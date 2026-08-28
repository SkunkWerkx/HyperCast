package io.github.buvinghausen.hypercast;

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

    private static MemorySegment input(Arena arena, byte[] utf8) {
        // len == 0 never dereferences the pointer, per the ABI contract.
        return utf8.length == 0 ? MemorySegment.NULL : arena.allocateFrom(ValueLayout.JAVA_BYTE, utf8);
    }

    private static MemorySegment format(Arena arena, NumFormat format) {
        MemorySegment segment = arena.allocate(12);
        segment.set(ValueLayout.JAVA_INT, 0, format.decimalSeparator());
        segment.set(ValueLayout.JAVA_INT, 4, format.groupSeparator());
        segment.set(ValueLayout.JAVA_INT, 8, format.styles());
        return segment;
    }

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
     */
    public static Verdict<Boolean> bool(String text) {
        return bool(utf8(text));
    }

    /** See {@link #bool(String)}; input as raw UTF-8 bytes. */
    public static Verdict<Boolean> bool(byte[] utf8) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = arena.allocate(ValueLayout.JAVA_BYTE);
            MemorySegment fault = arena.allocate(FAULT_BYTES);
            int code;
            try {
                code = (int) CAST_BOOL.invokeExact(input(arena, utf8), (long) utf8.length, out, fault);
            } catch (Throwable t) {
                throw new AssertionError("hypercast: cast_bool downcall failed unexpectedly", t);
            }
            return code == 0
                    ? new Success<>(out.get(ValueLayout.JAVA_BYTE, 0) != 0)
                    : failed(code, fault);
        }
    }

    // --- integers ---

    private interface IntReader<T> {
        T read(MemorySegment out);
    }

    private static <T> Verdict<T> numeric(
            MethodHandle door, String symbol, byte[] utf8, NumFormat format, long outBytes, IntReader<T> reader) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = arena.allocate(outBytes);
            MemorySegment fault = arena.allocate(FAULT_BYTES);
            int code;
            try {
                code = (int) door.invokeExact(
                        input(arena, utf8), (long) utf8.length, format(arena, format), out, fault);
            } catch (Throwable t) {
                throw new AssertionError("hypercast: " + symbol + " downcall failed unexpectedly", t);
            }
            return code == 0 ? new Success<>(reader.read(out)) : failed(code, fault);
        }
    }

    /**
     * Casts integer text to a signed 8-bit value under the declared format: the type's own
     * range, declared grouping, accounting parentheses, non-negative exponent ({@code 1e3}
     * is 1000; a decimal point is never accepted), and {@code 0x}/{@code &H}/{@code 0b}
     * two's-complement radix prefixes ({@code 0xFF} is -1).
     */
    public static Verdict<Byte> i8(String text, NumFormat format) {
        return i8(utf8(text), format);
    }

    /** See {@link #i8(String, NumFormat)}; input as raw UTF-8 bytes. */
    public static Verdict<Byte> i8(byte[] utf8, NumFormat format) {
        return numeric(CAST_I8, "cast_i8", utf8, format, 1, out -> out.get(ValueLayout.JAVA_BYTE, 0));
    }

    /** Casts integer text to a signed 16-bit value. Notation rules as {@link #i8(String, NumFormat)}. */
    public static Verdict<Short> i16(String text, NumFormat format) {
        return i16(utf8(text), format);
    }

    /** See {@link #i16(String, NumFormat)}; input as raw UTF-8 bytes. */
    public static Verdict<Short> i16(byte[] utf8, NumFormat format) {
        return numeric(CAST_I16, "cast_i16", utf8, format, 2, out -> out.get(ValueLayout.JAVA_SHORT, 0));
    }

    /** Casts integer text to a signed 32-bit value. Notation rules as {@link #i8(String, NumFormat)}. */
    public static Verdict<Integer> i32(String text, NumFormat format) {
        return i32(utf8(text), format);
    }

    /** See {@link #i32(String, NumFormat)}; input as raw UTF-8 bytes. */
    public static Verdict<Integer> i32(byte[] utf8, NumFormat format) {
        return numeric(CAST_I32, "cast_i32", utf8, format, 4, out -> out.get(ValueLayout.JAVA_INT, 0));
    }

    /** Casts integer text to a signed 64-bit value. Notation rules as {@link #i8(String, NumFormat)}. */
    public static Verdict<Long> i64(String text, NumFormat format) {
        return i64(utf8(text), format);
    }

    /** See {@link #i64(String, NumFormat)}; input as raw UTF-8 bytes. */
    public static Verdict<Long> i64(byte[] utf8, NumFormat format) {
        return numeric(CAST_I64, "cast_i64", utf8, format, 8, out -> out.get(ValueLayout.JAVA_LONG, 0));
    }

    /**
     * Casts integer text to an unsigned 8-bit value, widened to {@code int} ({@code 0..255})
     * — Java has no unsigned primitives. Notation rules as {@link #i8(String, NumFormat)}.
     */
    public static Verdict<Integer> u8(String text, NumFormat format) {
        return u8(utf8(text), format);
    }

    /** See {@link #u8(String, NumFormat)}; input as raw UTF-8 bytes. */
    public static Verdict<Integer> u8(byte[] utf8, NumFormat format) {
        return numeric(CAST_U8, "cast_u8", utf8, format, 1,
                out -> Byte.toUnsignedInt(out.get(ValueLayout.JAVA_BYTE, 0)));
    }

    /** Casts integer text to an unsigned 16-bit value, widened to {@code int} ({@code 0..65535}). */
    public static Verdict<Integer> u16(String text, NumFormat format) {
        return u16(utf8(text), format);
    }

    /** See {@link #u16(String, NumFormat)}; input as raw UTF-8 bytes. */
    public static Verdict<Integer> u16(byte[] utf8, NumFormat format) {
        return numeric(CAST_U16, "cast_u16", utf8, format, 2,
                out -> Short.toUnsignedInt(out.get(ValueLayout.JAVA_SHORT, 0)));
    }

    /** Casts integer text to an unsigned 32-bit value, widened to {@code long}. */
    public static Verdict<Long> u32(String text, NumFormat format) {
        return u32(utf8(text), format);
    }

    /** See {@link #u32(String, NumFormat)}; input as raw UTF-8 bytes. */
    public static Verdict<Long> u32(byte[] utf8, NumFormat format) {
        return numeric(CAST_U32, "cast_u32", utf8, format, 4,
                out -> Integer.toUnsignedLong(out.get(ValueLayout.JAVA_INT, 0)));
    }

    /**
     * Casts integer text to an unsigned 64-bit value, carried as {@code long}'s
     * two's-complement bit pattern — render with {@link Long#toUnsignedString(long)} and
     * compare with {@link Long#compareUnsigned(long, long)}.
     */
    public static Verdict<Long> u64(String text, NumFormat format) {
        return u64(utf8(text), format);
    }

    /** See {@link #u64(String, NumFormat)}; input as raw UTF-8 bytes. */
    public static Verdict<Long> u64(byte[] utf8, NumFormat format) {
        return numeric(CAST_U64, "cast_u64", utf8, format, 8, out -> out.get(ValueLayout.JAVA_LONG, 0));
    }

    // --- reals ---

    /**
     * Casts real text to {@code float} under the declared format: finite values only
     * ({@code NaN}/{@code Infinity} literals are {@link CastFailure#MALFORMED}, overflow to
     * infinity is {@link CastFailure#OUT_OF_RANGE}), declared separators and grouping,
     * accounting parentheses, exponent, and trailing percent ({@code 50%} is 0.5).
     */
    public static Verdict<Float> f32(String text, NumFormat format) {
        return f32(utf8(text), format);
    }

    /** See {@link #f32(String, NumFormat)}; input as raw UTF-8 bytes. */
    public static Verdict<Float> f32(byte[] utf8, NumFormat format) {
        return numeric(CAST_F32, "cast_f32", utf8, format, 4, out -> out.get(ValueLayout.JAVA_FLOAT, 0));
    }

    /** Casts real text to {@code double}. Notation rules as {@link #f32(String, NumFormat)}. */
    public static Verdict<Double> f64(String text, NumFormat format) {
        return f64(utf8(text), format);
    }

    /** See {@link #f64(String, NumFormat)}; input as raw UTF-8 bytes. */
    public static Verdict<Double> f64(byte[] utf8, NumFormat format) {
        return numeric(CAST_F64, "cast_f64", utf8, format, 8, out -> out.get(ValueLayout.JAVA_DOUBLE, 0));
    }

    // --- uuid ---

    /**
     * Casts UUID text to a {@link UUID}: every format .NET's {@code Guid} accepts (D, N, B,
     * P, X), after stripping a case-insensitive {@code urn:uuid:}/{@code GUID:}/{@code UUID:}
     * prefix. Culture-insensitive.
     */
    public static Verdict<UUID> uuid(String text) {
        return uuid(utf8(text));
    }

    /** See {@link #uuid(String)}; input as raw UTF-8 bytes. */
    public static Verdict<UUID> uuid(byte[] utf8) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = arena.allocate(16);
            MemorySegment fault = arena.allocate(FAULT_BYTES);
            int code;
            try {
                code = (int) CAST_UUID.invokeExact(input(arena, utf8), (long) utf8.length, out, fault);
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
    }

    // --- temporals ---

    /** Timestamp out-param: {@code {i64 seconds, i32 nanos}} (protobuf layout, 16 bytes with padding). */
    private static final long TIMESTAMP_BYTES = 16;

    private static Verdict<Instant> instantDoor(MethodHandle door, String symbol, byte[] utf8, int precision) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = arena.allocate(TIMESTAMP_BYTES);
            MemorySegment fault = arena.allocate(FAULT_BYTES);
            int code;
            try {
                code = precision == 0
                        ? (int) door.invokeExact(input(arena, utf8), (long) utf8.length, out, fault)
                        : (int) door.invokeExact(input(arena, utf8), (long) utf8.length, precision, out, fault);
            } catch (Throwable t) {
                throw new AssertionError("hypercast: " + symbol + " downcall failed unexpectedly", t);
            }
            return code == 0
                    ? new Success<>(Instant.ofEpochSecond(
                            out.get(ValueLayout.JAVA_LONG, 0), out.get(ValueLayout.JAVA_INT, 8)))
                    : failed(code, fault);
        }
    }

    /**
     * Casts an RFC 3339 instant — {@code yyyy-MM-ddTHH:mm:ss[.f{1..9}](Z|±hh:mm)}, zone
     * <b>mandatory</b> — to an {@link Instant}, normalized to UTC at full nanosecond
     * fidelity. A zone-less or space-separated form is {@link CastFailure#MALFORMED}; an
     * instant outside 0001-01-01 to 9999-12-31 UTC is {@link CastFailure#OUT_OF_RANGE}.
     */
    public static Verdict<Instant> timestamp(String text) {
        return timestamp(utf8(text));
    }

    /** See {@link #timestamp(String)}; input as raw UTF-8 bytes. */
    public static Verdict<Instant> timestamp(byte[] utf8) {
        return instantDoor(CAST_TIMESTAMP, "cast_timestamp", utf8, 0);
    }

    /**
     * Casts an integer Unix-epoch value under a caller-declared unit to an {@link Instant}.
     * Negatives (pre-1970) are allowed; a fractional or non-integer value is
     * {@link CastFailure#MALFORMED}; outside the 0001–9999 window is
     * {@link CastFailure#OUT_OF_RANGE}.
     */
    public static Verdict<Instant> unix(String text, UnixPrecision precision) {
        return unix(utf8(text), precision);
    }

    /** See {@link #unix(String, UnixPrecision)}; input as raw UTF-8 bytes. */
    public static Verdict<Instant> unix(byte[] utf8, UnixPrecision precision) {
        return instantDoor(CAST_UNIX, "cast_unix", utf8, precision.code());
    }

    /**
     * Casts a strict ISO 8601 {@code yyyy-MM-dd} calendar date to a {@link LocalDate}.
     * Anything time-bearing or non-ISO is {@link CastFailure#MALFORMED}; year 0000 is
     * {@link CastFailure#OUT_OF_RANGE}.
     */
    public static Verdict<LocalDate> date(String text) {
        return date(utf8(text));
    }

    /** See {@link #date(String)}; input as raw UTF-8 bytes. */
    public static Verdict<LocalDate> date(byte[] utf8) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = arena.allocate(4);
            MemorySegment fault = arena.allocate(FAULT_BYTES);
            int code;
            try {
                code = (int) CAST_DATE.invokeExact(input(arena, utf8), (long) utf8.length, out, fault);
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
    }

    /**
     * Casts an ISO 8601 24-hour time-of-day — {@code HH:mm}, {@code HH:mm:ss}, or
     * {@code HH:mm:ss.f{1..9}} — to a {@link LocalTime} at full nanosecond fidelity.
     * Midnight and {@code 23:59:59.999999999} are real clock readings, so this door has no
     * range failure.
     */
    public static Verdict<LocalTime> time(String text) {
        return time(utf8(text));
    }

    /** See {@link #time(String)}; input as raw UTF-8 bytes. */
    public static Verdict<LocalTime> time(byte[] utf8) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = arena.allocate(ValueLayout.JAVA_LONG);
            MemorySegment fault = arena.allocate(FAULT_BYTES);
            int code;
            try {
                code = (int) CAST_TIME.invokeExact(input(arena, utf8), (long) utf8.length, out, fault);
            } catch (Throwable t) {
                throw new AssertionError("hypercast: cast_time downcall failed unexpectedly", t);
            }
            return code == 0
                    ? new Success<>(LocalTime.ofNanoOfDay(out.get(ValueLayout.JAVA_LONG, 0)))
                    : failed(code, fault);
        }
    }

    /**
     * Casts a duration in any of three cleanly-partitioned shapes to a {@link Duration} at
     * full nanosecond fidelity: an ISO 8601 duration restricted to fixed components
     * ({@code P2W}, {@code P1DT6H30M15.5S} — years/months are not fixed durations and are
     * {@link CastFailure#MALFORMED}), the invariant colon form
     * ({@code [-][d.]hh:mm[:ss[.f]]}), or protobuf JSON seconds ({@code 3.5s}). Beyond
     * ±10,000 years is {@link CastFailure#OUT_OF_RANGE}.
     */
    public static Verdict<Duration> duration(String text) {
        return duration(utf8(text));
    }

    /** See {@link #duration(String)}; input as raw UTF-8 bytes. */
    public static Verdict<Duration> duration(byte[] utf8) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = arena.allocate(TIMESTAMP_BYTES);
            MemorySegment fault = arena.allocate(FAULT_BYTES);
            int code;
            try {
                code = (int) CAST_DURATION.invokeExact(input(arena, utf8), (long) utf8.length, out, fault);
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
}
