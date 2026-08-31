package io.github.skunkwerkx.hypercast;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.UUID;

/**
 * Allocation-lean scalar casts — booleans, numerics, UUIDs, temporals — calling directly
 * into the native {@code libhypercast} shared library via the Java Foreign Function &amp;
 * Memory API (stable since JDK 22 / JEP 454). Every door returns a {@link Verdict}: the
 * value, or a {@link Fault} with a closed reason and the offending byte span. Never throws
 * on bad input — the only exceptions here are caller bugs (a malformed {@link NumFormat}),
 * never data.
 *
 * <p>Door names mirror the native ABI ({@code i32}, {@code f64}, {@code timestamp}, …) so
 * the polyglot surface reads identically across bindings. Each door takes a {@link String}
 * (UTF-8-encoded into a confined {@link Arena} scratch segment) or raw UTF-8 {@code byte[]}
 * — the native contract, and the form whose {@link Fault} offsets need no mapping.
 *
 * <p>Temporal doors come out at {@code java.time}'s full fidelity: {@link Instant},
 * {@link LocalTime}, and {@link Duration} all carry nanoseconds, so unlike the C# binding
 * (ticks) nothing truncates — the JVM is the one platform that keeps every digit the core
 * parses.
 *
 * <p>The native library rides inside the jar under {@code /native/{rid}/} and is picked by
 * platform at runtime (see {@link NativePlatform}); the beyond-scalar cost per call is the
 * verdict record and the arena scratch — the Rust core itself never allocates.
 */
public final class Cast {
    private Cast() {}

    private static final Linker LINKER = Linker.nativeLinker();
    private static final SymbolLookup LOOKUP = loadLibrary();

    // (ptr, len, out, fault) -> code — the culture-insensitive doors.
    private static final FunctionDescriptor PLAIN = FunctionDescriptor.of(
            ValueLayout.JAVA_INT,
            ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.ADDRESS);
    // (ptr, len, format, out, fault) -> code — the numeric doors.
    private static final FunctionDescriptor NUMERIC = FunctionDescriptor.of(
            ValueLayout.JAVA_INT,
            ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.ADDRESS,
            ValueLayout.ADDRESS);
    // (ptr, len, precision, out, fault) -> code — the Unix door.
    private static final FunctionDescriptor UNIX = FunctionDescriptor.of(
            ValueLayout.JAVA_INT,
            ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT, ValueLayout.ADDRESS,
            ValueLayout.ADDRESS);

    private static final MethodHandle CAST_BOOL = handle("cast_bool", PLAIN);
    private static final MethodHandle CAST_I8 = handle("cast_i8", NUMERIC);
    private static final MethodHandle CAST_I16 = handle("cast_i16", NUMERIC);
    private static final MethodHandle CAST_I32 = handle("cast_i32", NUMERIC);
    private static final MethodHandle CAST_I64 = handle("cast_i64", NUMERIC);
    private static final MethodHandle CAST_U8 = handle("cast_u8", NUMERIC);
    private static final MethodHandle CAST_U16 = handle("cast_u16", NUMERIC);
    private static final MethodHandle CAST_U32 = handle("cast_u32", NUMERIC);
    private static final MethodHandle CAST_U64 = handle("cast_u64", NUMERIC);
    private static final MethodHandle CAST_F32 = handle("cast_f32", NUMERIC);
    private static final MethodHandle CAST_F64 = handle("cast_f64", NUMERIC);
    private static final MethodHandle CAST_UUID = handle("cast_uuid", PLAIN);
    private static final MethodHandle CAST_TIMESTAMP = handle("cast_timestamp", PLAIN);
    private static final MethodHandle CAST_UNIX = handle("cast_unix", UNIX);
    private static final MethodHandle CAST_DATE = handle("cast_date", PLAIN);
    // cast_date_ordered and cast_datetime share the unix ABI shape (ptr, len, u32, out, fault).
    private static final MethodHandle CAST_DATE_ORDERED = handle("cast_date_ordered", UNIX);
    private static final MethodHandle CAST_DATETIME = handle("cast_datetime", UNIX);
    private static final MethodHandle CAST_TIME = handle("cast_time", PLAIN);
    private static final MethodHandle CAST_DURATION = handle("cast_duration", PLAIN);

    private static MethodHandle handle(String symbol, FunctionDescriptor descriptor) {
        return LINKER.downcallHandle(LOOKUP.find(symbol).orElseThrow(), descriptor);
    }

    // The library must outlive every downcall made through it, so it's loaded into the
    // JDK-provided global arena that lives for the process's lifetime.
    private static SymbolLookup loadLibrary() {
        String resourcePath = NativePlatform.resourcePath();
        try (InputStream resource = Cast.class.getResourceAsStream(resourcePath)) {
            if (resource == null) {
                throw new IllegalStateException(resourcePath
                        + " classpath resource not found (unsupported platform, or this jar was "
                        + "built without a native library for it)");
            }
            String libraryFileName = NativePlatform.current().libraryFileName();
            String extension = libraryFileName.substring(libraryFileName.lastIndexOf('.'));
            Path tmp = Files.createTempFile("hypercast", extension);
            tmp.toFile().deleteOnExit();
            Files.copy(resource, tmp, StandardCopyOption.REPLACE_EXISTING);
            return SymbolLookup.libraryLookup(tmp, Arena.global());
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    /** Fault span out-param: {@code {u32 offset, u32 length}}. */
    private static final long FAULT_BYTES = 8;

    /**
     * Per-thread scratch for the downcall out-params. Every door used to open its own
     * {@link Arena#ofConfined()} — a fresh native allocation plus a scope teardown on every
     * single cast, measured at roughly 100 ns and the dominant cost of the lean doors.
     * Confined arenas are thread-confined by design, so the replacement is thread-confined
     * too: one {@link ThreadLocal} holding segments that live as long as the thread does.
     *
     * <p>{@code out} is sized for the widest out-param (16 bytes — a protobuf timestamp or
     * a civil date-time) and each door reads only its own prefix, after the native side has
     * written it; a failing call never reads it at all. Nothing is shared between threads,
     * so no door needs locking, and doors never nest, so no call can observe another's
     * scratch mid-flight.
     */
    private static final class Scratch {
        /** Reused input staging buffer; grown only when a call actually outgrows it. */
        private static final long INITIAL_INPUT_BYTES = 512;

        // Explicit alignment rather than allocate(size)'s implicit 1: the temporal doors
        // read JAVA_LONG out of `out`, which a 1-byte-aligned segment rejects outright.
        private final Arena fixed = Arena.ofAuto();
        final MemorySegment out = fixed.allocate(16, 8);
        final MemorySegment fault = fixed.allocate(FAULT_BYTES, 4);
        private final MemorySegment formatSegment = fixed.allocate(12, 4);
        private NumFormat formatKey;

        // Deliberately a *separate* arena from `fixed`: a segment keeps its whole arena
        // alive, so sharing one would retain every buffer this thread ever outgrew.
        // Replacing this field on growth drops the old buffer for the collector.
        private Arena inputArena = Arena.ofAuto();
        private MemorySegment inputBuffer = inputArena.allocate(INITIAL_INPUT_BYTES, 1);

        MemorySegment input(byte[] utf8) {
            // len == 0 never dereferences the pointer, per the ABI contract.
            if (utf8.length == 0) {
                return MemorySegment.NULL;
            }
            if (utf8.length > inputBuffer.byteSize()) {
                inputArena = Arena.ofAuto();
                inputBuffer = inputArena.allocate(Long.highestOneBit(utf8.length - 1) << 1, 1);
            }
            MemorySegment.copy(utf8, 0, inputBuffer, ValueLayout.JAVA_BYTE, 0, utf8.length);
            return inputBuffer;
        }

        MemorySegment format(NumFormat declared) {
            // Formats are reused constants in practice (INVARIANT, DETECT, a per-locale
            // instance), so an identity check skips three stores on the overwhelming
            // majority of calls — the same memo the Python and Ruby bindings keep.
            if (formatKey != declared) {
                formatSegment.set(ValueLayout.JAVA_INT, 0, declared.decimalSeparator());
                formatSegment.set(ValueLayout.JAVA_INT, 4, declared.groupSeparator());
                formatSegment.set(ValueLayout.JAVA_INT, 8, declared.styles());
                formatKey = declared;
            }
            return formatSegment;
        }
    }

    private static final ThreadLocal<Scratch> SCRATCH = ThreadLocal.withInitial(Scratch::new);

    private static <T> Verdict<T> failed(int code, MemorySegment fault) {
        if (code == -1) {
            throw new IllegalStateException(
                    "libhypercast reported a contract violation — a binding bug, please report it");
        }
        return new Fault<>(
                CastFailure.fromCode(code),
                fault.get(ValueLayout.JAVA_INT, 0),
                fault.get(ValueLayout.JAVA_INT, 4));
    }

    private static byte[] utf8(String text) {
        return text.getBytes(StandardCharsets.UTF_8);
    }

    /**
     * Presents a verdict optionally: an {@link CastFailure#EMPTY} fault becomes absent
     * ({@link java.util.Optional#empty()}); every other outcome flows through untouched —
     * each binding maps absence to its platform's own idiom, and Java's is {@code Optional}.
     *
     * @param <T> the verdict's value type
     * @param verdict the verdict to present
     * @return empty for an EMPTY fault; the untouched verdict otherwise
     */
    public static <T> java.util.Optional<Verdict<T>> optional(Verdict<T> verdict) {
        return verdict instanceof Fault<T> fault && fault.reason() == CastFailure.EMPTY
                ? java.util.Optional.empty()
                : java.util.Optional.of(verdict);
    }

    // --- boolean ---

    /**
     * Casts boolean text: {@code true}/{@code false} plus the numeric and natural-language
     * conventions untrusted sources actually send ({@code t}/{@code f}, {@code yes}/{@code no},
     * {@code y}/{@code n}, {@code 1}/{@code 0}, {@code on}/{@code off},
     * {@code enabled}/{@code disabled}, {@code active}/{@code inactive},
     * {@code checked}/{@code unchecked}, {@code in}/{@code out}), ASCII case-insensitive.
     * Culture-insensitive — no {@link NumFormat}.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Boolean> bool(String text) {
        return bool(utf8(text));
    }

    /**
     * See {@link #bool(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Boolean> bool(byte[] utf8) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code;
        try {
            code = (int) CAST_BOOL.invokeExact(scratch.input(utf8), (long) utf8.length, out, fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: cast_bool downcall failed unexpectedly", t);
        }
        return code == 0
                ? new Success<>(out.get(ValueLayout.JAVA_BYTE, 0) != 0)
                : failed(code, fault);
    }

    // --- integers ---

    private interface IntReader<T> {
        T read(MemorySegment out);
    }

    private static <T> Verdict<T> numeric(
            MethodHandle door, String symbol, byte[] utf8, NumFormat format, IntReader<T> reader) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code;
        try {
            code = (int) door.invokeExact(
                    scratch.input(utf8), (long) utf8.length, scratch.format(format), out, fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: " + symbol + " downcall failed unexpectedly", t);
        }
        return code == 0 ? new Success<>(reader.read(out)) : failed(code, fault);
    }

    /**
     * Casts integer text to a signed 8-bit value under the declared format: the type's own
     * range, declared grouping, accounting parentheses, non-negative exponent ({@code 1e3}
     * is 1000; a decimal point is never accepted), and {@code 0x}/{@code &H}/{@code 0b}
     * two's-complement radix prefixes ({@code 0xFF} is -1).
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Byte> i8(String text, NumFormat format) {
        return i8(utf8(text), format);
    }

    /**
     * See {@link #i8(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Byte> i8(byte[] utf8, NumFormat format) {
        return numeric(CAST_I8, "cast_i8", utf8, format, out -> out.get(ValueLayout.JAVA_BYTE, 0));
    }

    /**
     * Casts integer text to a signed 16-bit value. Notation rules as {@link #i8(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Short> i16(String text, NumFormat format) {
        return i16(utf8(text), format);
    }

    /**
     * See {@link #i16(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Short> i16(byte[] utf8, NumFormat format) {
        return numeric(CAST_I16, "cast_i16", utf8, format, out -> out.get(ValueLayout.JAVA_SHORT, 0));
    }

    /**
     * Casts integer text to a signed 32-bit value. Notation rules as {@link #i8(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> i32(String text, NumFormat format) {
        return i32(utf8(text), format);
    }

    /**
     * See {@link #i32(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> i32(byte[] utf8, NumFormat format) {
        return numeric(CAST_I32, "cast_i32", utf8, format, out -> out.get(ValueLayout.JAVA_INT, 0));
    }

    /**
     * Casts integer text to a signed 64-bit value. Notation rules as {@link #i8(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> i64(String text, NumFormat format) {
        return i64(utf8(text), format);
    }

    /**
     * See {@link #i64(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> i64(byte[] utf8, NumFormat format) {
        return numeric(CAST_I64, "cast_i64", utf8, format, out -> out.get(ValueLayout.JAVA_LONG, 0));
    }

    /**
     * Casts integer text to an unsigned 8-bit value, widened to {@code int} ({@code 0..255})
     * — Java has no unsigned primitives. Notation rules as {@link #i8(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u8(String text, NumFormat format) {
        return u8(utf8(text), format);
    }

    /**
     * See {@link #u8(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u8(byte[] utf8, NumFormat format) {
        return numeric(CAST_U8, "cast_u8", utf8, format,
                out -> Byte.toUnsignedInt(out.get(ValueLayout.JAVA_BYTE, 0)));
    }

    /**
     * Casts integer text to an unsigned 16-bit value, widened to {@code int} ({@code 0..65535}).
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u16(String text, NumFormat format) {
        return u16(utf8(text), format);
    }

    /**
     * See {@link #u16(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u16(byte[] utf8, NumFormat format) {
        return numeric(CAST_U16, "cast_u16", utf8, format,
                out -> Short.toUnsignedInt(out.get(ValueLayout.JAVA_SHORT, 0)));
    }

    /**
     * Casts integer text to an unsigned 32-bit value, widened to {@code long}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u32(String text, NumFormat format) {
        return u32(utf8(text), format);
    }

    /**
     * See {@link #u32(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u32(byte[] utf8, NumFormat format) {
        return numeric(CAST_U32, "cast_u32", utf8, format,
                out -> Integer.toUnsignedLong(out.get(ValueLayout.JAVA_INT, 0)));
    }

    /**
     * Casts integer text to an unsigned 64-bit value, carried as {@code long}'s
     * two's-complement bit pattern — render with {@link Long#toUnsignedString(long)} and
     * compare with {@link Long#compareUnsigned(long, long)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u64(String text, NumFormat format) {
        return u64(utf8(text), format);
    }

    /**
     * See {@link #u64(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u64(byte[] utf8, NumFormat format) {
        return numeric(CAST_U64, "cast_u64", utf8, format, out -> out.get(ValueLayout.JAVA_LONG, 0));
    }

    // --- reals ---

    /**
     * Casts real text to {@code float} under the declared format: finite values only
     * ({@code NaN}/{@code Infinity} literals are {@link CastFailure#MALFORMED}, overflow to
     * infinity is {@link CastFailure#OUT_OF_RANGE}), declared separators and grouping,
     * accounting parentheses, exponent, and trailing percent ({@code 50%} is 0.5).
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Float> f32(String text, NumFormat format) {
        return f32(utf8(text), format);
    }

    /**
     * See {@link #f32(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Float> f32(byte[] utf8, NumFormat format) {
        return numeric(CAST_F32, "cast_f32", utf8, format, out -> out.get(ValueLayout.JAVA_FLOAT, 0));
    }

    /**
     * Casts real text to {@code double}. Notation rules as {@link #f32(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Double> f64(String text, NumFormat format) {
        return f64(utf8(text), format);
    }

    /**
     * See {@link #f64(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Double> f64(byte[] utf8, NumFormat format) {
        return numeric(CAST_F64, "cast_f64", utf8, format, out -> out.get(ValueLayout.JAVA_DOUBLE, 0));
    }

    // --- uuid ---

    /**
     * Casts UUID text to a {@link UUID}: every format .NET's {@code Guid} accepts (D, N, B,
     * P, X), after stripping a case-insensitive {@code urn:uuid:}/{@code GUID:}/{@code UUID:}
     * prefix. Culture-insensitive.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<UUID> uuid(String text) {
        return uuid(utf8(text));
    }

    /**
     * See {@link #uuid(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<UUID> uuid(byte[] utf8) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code;
        try {
            code = (int) CAST_UUID.invokeExact(scratch.input(utf8), (long) utf8.length, out, fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: cast_uuid downcall failed unexpectedly", t);
        }
        if (code != 0) {
            return failed(code, fault);
        }
        // RFC 9562 order is exactly UUID's msb/lsb decomposition — no swapping, unlike Guid.
        long msb = 0;
        long lsb = 0;
        for (int i = 0; i < 8; i++) {
            msb = (msb << 8) | (out.get(ValueLayout.JAVA_BYTE, i) & 0xFFL);
            lsb = (lsb << 8) | (out.get(ValueLayout.JAVA_BYTE, i + 8) & 0xFFL);
        }
        return new Success<>(new UUID(msb, lsb));
    }

    // --- temporals ---

    /** Timestamp out-param: {@code {i64 seconds, i32 nanos}} (protobuf layout, 16 bytes with padding). */
    private static final long TIMESTAMP_BYTES = 16;

    /** CivilDateTime out-param: {@code {u16 y, u8 m, u8 d, pad, u64 nanos-of-day}} (16 bytes). */
    private static final long CIVIL_BYTES = 16;

    private static Verdict<Instant> instantDoor(MethodHandle door, String symbol, byte[] utf8, int precision) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        MemorySegment in = scratch.input(utf8);
        int code;
        try {
            code = precision == 0
                    ? (int) door.invokeExact(in, (long) utf8.length, out, fault)
                    : (int) door.invokeExact(in, (long) utf8.length, precision, out, fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: " + symbol + " downcall failed unexpectedly", t);
        }
        return code == 0
                ? new Success<>(Instant.ofEpochSecond(
                        out.get(ValueLayout.JAVA_LONG, 0), out.get(ValueLayout.JAVA_INT, 8)))
                : failed(code, fault);
    }

    /**
     * Casts an RFC 3339 instant — {@code yyyy-MM-ddTHH:mm:ss[.f{1..9}](Z|±hh:mm)}, zone
     * <b>mandatory</b> — to an {@link Instant}, normalized to UTC at full nanosecond
     * fidelity. A zone-less or space-separated form is {@link CastFailure#MALFORMED}; an
     * instant outside 0001-01-01 to 9999-12-31 UTC is {@link CastFailure#OUT_OF_RANGE}.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> timestamp(String text) {
        return timestamp(utf8(text));
    }

    /**
     * See {@link #timestamp(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> timestamp(byte[] utf8) {
        return instantDoor(CAST_TIMESTAMP, "cast_timestamp", utf8, 0);
    }

    /**
     * Casts an integer Unix-epoch value under a caller-declared unit to an {@link Instant}.
     * Negatives (pre-1970) are allowed; a fractional or non-integer value is
     * {@link CastFailure#MALFORMED}; outside the 0001–9999 window is
     * {@link CastFailure#OUT_OF_RANGE}.
     *
     * @param text the text to cast
     * @param precision the declared unit of the epoch value
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> unix(String text, UnixPrecision precision) {
        return unix(utf8(text), precision);
    }

    /**
     * See {@link #unix(String, UnixPrecision)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param precision the declared unit of the epoch value
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> unix(byte[] utf8, UnixPrecision precision) {
        return instantDoor(CAST_UNIX, "cast_unix", utf8, precision.code());
    }

    /**
     * Casts a strict ISO 8601 {@code yyyy-MM-dd} calendar date to a {@link LocalDate}.
     * Anything time-bearing or non-ISO is {@link CastFailure#MALFORMED}; year 0000 is
     * {@link CastFailure#OUT_OF_RANGE}.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(String text) {
        return date(utf8(text));
    }

    /**
     * See {@link #date(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(byte[] utf8) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code;
        try {
            code = (int) CAST_DATE.invokeExact(scratch.input(utf8), (long) utf8.length, out, fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: cast_date downcall failed unexpectedly", t);
        }
        return code == 0
                ? new Success<>(LocalDate.of(
                        Short.toUnsignedInt(out.get(ValueLayout.JAVA_SHORT, 0)),
                        out.get(ValueLayout.JAVA_BYTE, 2),
                        out.get(ValueLayout.JAVA_BYTE, 3)))
                : failed(code, fault);
    }

    /**
     * Casts a separated calendar date — three digit fields joined by one consistent
     * separator ({@code /}, {@code -}, or {@code .}) — under the caller-declared
     * {@link DateOrder} to a {@link LocalDate}: {@code 1/7/2026} is January 7th or July 1st
     * only because {@code order} said which. The year field is four digits wherever the
     * order puts it (two-digit years mean century guessing, which never happens —
     * {@link CastFailure#MALFORMED}); the order-less {@link #date(String)} overload stays
     * strict ISO.
     *
     * @param text the text to cast
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(String text, DateOrder order) {
        return date(utf8(text), order);
    }

    /**
     * See {@link #date(String, DateOrder)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(byte[] utf8, DateOrder order) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code;
        try {
            code = (int) CAST_DATE_ORDERED.invokeExact(
                    scratch.input(utf8), (long) utf8.length, order.code(), out, fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: cast_date_ordered downcall failed unexpectedly", t);
        }
        return code == 0
                ? new Success<>(LocalDate.of(
                        Short.toUnsignedInt(out.get(ValueLayout.JAVA_SHORT, 0)),
                        out.get(ValueLayout.JAVA_BYTE, 2),
                        out.get(ValueLayout.JAVA_BYTE, 3)))
                : failed(code, fault);
    }

    /**
     * Casts a zone-less civil date-time — the shape untrusted feeds actually send
     * ({@code 1/7/2026 3:04 PM}, {@code 2026-01-07 15:04:05}) — under the caller-declared
     * {@link DateOrder} to a {@link LocalDateTime} at full nanosecond fidelity. The date
     * part follows {@link #date(String, DateOrder)}'s grammar; the optional time part (one
     * space or {@code T} after the date) is 24-hour {@code h:mm[:ss[.f{1..9}]]} or 12-hour
     * with an {@code AM}/{@code PM} marker; absent, the time is midnight. No zone is read
     * and none is invented — the text named no instant, which is exactly what
     * {@link LocalDateTime} says; fusing a zone is the caller's job
     * ({@link #timestamp(String)} stays the strict RFC 3339 instant door).
     *
     * @param text the text to cast
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDateTime> dateTime(String text, DateOrder order) {
        return dateTime(utf8(text), order);
    }

    /**
     * See {@link #dateTime(String, DateOrder)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDateTime> dateTime(byte[] utf8, DateOrder order) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code;
        try {
            code = (int) CAST_DATETIME.invokeExact(
                    scratch.input(utf8), (long) utf8.length, order.code(), out, fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: cast_datetime downcall failed unexpectedly", t);
        }
        return code == 0
                ? new Success<>(LocalDateTime.of(
                        LocalDate.of(
                                Short.toUnsignedInt(out.get(ValueLayout.JAVA_SHORT, 0)),
                                out.get(ValueLayout.JAVA_BYTE, 2),
                                out.get(ValueLayout.JAVA_BYTE, 3)),
                        LocalTime.ofNanoOfDay(out.get(ValueLayout.JAVA_LONG, 8))))
                : failed(code, fault);
    }

    /**
     * Casts an ISO 8601 24-hour time-of-day — {@code HH:mm}, {@code HH:mm:ss}, or
     * {@code HH:mm:ss.f{1..9}} — to a {@link LocalTime} at full nanosecond fidelity.
     * Midnight and {@code 23:59:59.999999999} are real clock readings, so this door has no
     * range failure.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalTime> time(String text) {
        return time(utf8(text));
    }

    /**
     * See {@link #time(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalTime> time(byte[] utf8) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code;
        try {
            code = (int) CAST_TIME.invokeExact(scratch.input(utf8), (long) utf8.length, out, fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: cast_time downcall failed unexpectedly", t);
        }
        return code == 0
                ? new Success<>(LocalTime.ofNanoOfDay(out.get(ValueLayout.JAVA_LONG, 0)))
                : failed(code, fault);
    }

    /**
     * Casts a duration in any of three cleanly-partitioned shapes to a {@link Duration} at
     * full nanosecond fidelity: an ISO 8601 duration restricted to fixed components
     * ({@code P2W}, {@code P1DT6H30M15.5S} — years/months are not fixed durations and are
     * {@link CastFailure#MALFORMED}), the invariant colon form
     * ({@code [-][d.]hh:mm[:ss[.f]]}), or protobuf JSON seconds ({@code 3.5s}). Beyond
     * ±10,000 years is {@link CastFailure#OUT_OF_RANGE}.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Duration> duration(String text) {
        return duration(utf8(text));
    }

    /**
     * See {@link #duration(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Duration> duration(byte[] utf8) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code;
        try {
            code = (int) CAST_DURATION.invokeExact(scratch.input(utf8), (long) utf8.length, out, fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: cast_duration downcall failed unexpectedly", t);
        }
        // Duration.ofSeconds normalizes the core's same-signed nanos adjustment correctly.
        return code == 0
                ? new Success<>(Duration.ofSeconds(
                        out.get(ValueLayout.JAVA_LONG, 0), out.get(ValueLayout.JAVA_INT, 8)))
                : failed(code, fault);
    }
}
