package io.github.skunkwerkx.hypercast;

import io.github.skunkwerkx.hypercast.interop.NativePlatform;
import io.github.skunkwerkx.hypercast.interop.NativeValues;
import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.math.BigDecimal;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.UUID;

/**
 * The FFM path to the core: the JDK 25 and later half of the multi-release jar. Every call
 * goes straight to the library's C export through a {@link Linker.Option#critical(boolean)
 * critical} downcall and returns the core's verdict code raw; {@link Cast} builds the
 * verdict. The JDK 21 base tree has a class of the same name and the same members that
 * reaches the core through JNI instead.
 *
 * <p>The input crosses without a copy: every text downcall is linked
 * {@code critical(true)}, so the caller's own heap array or segment is handed to the native
 * side directly — sound because each door is a short, non-blocking parse over those bytes
 * that never calls back into Java, which is the exact profile that option exists for.
 */
final class NativeCore {
    private NativeCore() {}

    /**
     * The downcall handles: one per ABI shape the core exports, none of them bound to an
     * address. Each takes the export's address as its leading argument, which {@link Core}
     * looks up once the library is loaded.
     *
     * <p>They live apart from {@link Core} for GraalVM Native Image. An image can only compile
     * a call through a {@code MethodHandle} that is already a constant when the image is
     * built; a handle created at run time — which is what binding one to a symbol's address
     * forces, since the address does not exist until the library is loaded — is invoked
     * through the image's method-handle interpreter instead, at microseconds a call (measured:
     * 8-9 µs a door, against 14-51 ns on the JVM). Nothing in this class needs the library, so
     * {@code META-INF/native-image/.../native-image.properties} has it initialized at image
     * build time, and the handles are constants in the image. On the JVM the split changes
     * nothing: the class initializes on the first door, and the JIT folds a
     * {@code static final} handle either way.
     */
    private static final class Downcalls {
        private Downcalls() {}

        private static final Linker LINKER = Linker.nativeLinker();

        // critical(true) is what lets a heap segment (MemorySegment.ofArray over the caller's
        // byte[]) cross without being copied into native memory first: the array is pinned for
        // the duration of the call instead. The contract in exchange — the callee must be short,
        // must not block, and must never upcall into Java — is exactly what every door is: a
        // bounded parse over the bytes it was handed, with no callbacks and no allocation.
        private static final Linker.Option CRITICAL = Linker.Option.critical(true);

        // (ptr, len, out, fault) -> code — the culture-insensitive doors.
        private static final MethodHandle PLAIN = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT,
                        ValueLayout.ADDRESS,
                        ValueLayout.JAVA_LONG,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS),
                CRITICAL);
        // (ptr, len, format, out, fault) -> code — the numeric doors.
        private static final MethodHandle NUMERIC = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT,
                        ValueLayout.ADDRESS,
                        ValueLayout.JAVA_LONG,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS),
                CRITICAL);
        // (ptr, len, discriminant, out, fault) -> code — the Unix door, and every other door
        // that takes one declared u32: the Excel epoch, the date order.
        private static final MethodHandle DECLARED = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT,
                        ValueLayout.ADDRESS,
                        ValueLayout.JAVA_LONG,
                        ValueLayout.JAVA_INT,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS),
                CRITICAL);
        // (ptr, len, discriminant, periods, out, fault) -> code — the civil date-time door:
        // the declared order, and the day-period designators or NULL for none.
        private static final MethodHandle CIVIL = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT,
                        ValueLayout.ADDRESS,
                        ValueLayout.JAVA_LONG,
                        ValueLayout.JAVA_INT,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS),
                CRITICAL);
        // (value, out, fault) -> code — the typed doors, which read a double the caller holds.
        private static final MethodHandle TYPED = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT, ValueLayout.JAVA_DOUBLE, ValueLayout.ADDRESS, ValueLayout.ADDRESS),
                CRITICAL);
        // (value, discriminant, out, fault) -> code — the typed Excel serial door and its epoch.
        private static final MethodHandle TYPED_DECLARED = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_DOUBLE,
                        ValueLayout.JAVA_INT,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS),
                CRITICAL);
        // () -> packed version — the probe. Nothing crosses, so it is not linked critical.
        private static final MethodHandle VERSION = LINKER.downcallHandle(FunctionDescriptor.of(ValueLayout.JAVA_INT));
    }

    /**
     * The loaded core: the library it resolved to, and the address of every export in it —
     * all {@code static final}, all resolved in this holder's own class init. A holder so
     * that nothing loads until the first door (or {@link Cast#nativeVersion()}) touches it,
     * and so that {@link Cast#isAvailable()} can observe a load failure without {@code Cast}
     * having failed to initialize: a door after a failed load throws the
     * {@link NoClassDefFoundError} for this class.
     */
    private static final class Core {
        private Core() {}

        /** The library's base name: {@code libhypercast.so}, {@code hypercast.dll}. */
        private static final String LIBRARY = "hypercast";

        private static final SymbolLookup LOOKUP =
                NativePlatform.load(NativeCore.class, LIBRARY, NativePlatform.current(LIBRARY));

        // Where each export lives in the loaded library — the leading argument of the
        // Downcalls handle with its shape. Looked up here, once, so an export missing from an
        // older core fails this class's init (and isAvailable() says so) rather than a door.
        private static final MemorySegment CAST_BOOL = export("cast_bool");
        private static final MemorySegment CAST_I8 = export("cast_i8");
        private static final MemorySegment CAST_I16 = export("cast_i16");
        private static final MemorySegment CAST_I32 = export("cast_i32");
        private static final MemorySegment CAST_I64 = export("cast_i64");
        private static final MemorySegment CAST_U8 = export("cast_u8");
        private static final MemorySegment CAST_U16 = export("cast_u16");
        private static final MemorySegment CAST_U32 = export("cast_u32");
        private static final MemorySegment CAST_U64 = export("cast_u64");
        private static final MemorySegment CAST_F32 = export("cast_f32");
        private static final MemorySegment CAST_F64 = export("cast_f64");
        private static final MemorySegment CAST_DECIMAL = export("cast_decimal");
        private static final MemorySegment CAST_UUID = export("cast_uuid");
        private static final MemorySegment CAST_TIMESTAMP = export("cast_timestamp");
        private static final MemorySegment CAST_UNIX = export("cast_unix");
        private static final MemorySegment CAST_EXCEL_SERIAL = export("cast_excel_serial");
        private static final MemorySegment CAST_DATE = export("cast_date");
        private static final MemorySegment CAST_DATE_ORDERED = export("cast_date_ordered");
        private static final MemorySegment CAST_DATETIME = export("cast_datetime");
        private static final MemorySegment CAST_TIME = export("cast_time");
        private static final MemorySegment CAST_DURATION = export("cast_duration");
        private static final MemorySegment CAST_DECIMAL_FROM_F64 = export("cast_decimal_from_f64");
        private static final MemorySegment CAST_EXCEL_SERIAL_FROM_F64 = export("cast_excel_serial_from_f64");
        private static final MemorySegment CAST_EXCEL_TIME = export("cast_excel_time");
        private static final MemorySegment CAST_EXCEL_DURATION = export("cast_excel_duration");
        private static final MemorySegment CAST_CHAR = export("cast_char");
        private static final MemorySegment HYPERCAST_VERSION = export("hypercast_version");

        private static MemorySegment export(String symbol) {
            return LOOKUP.find(symbol).orElseThrow();
        }
    }

    /** Which path reached the core, for the diagnostics; loads nothing. */
    static String backend() {
        return "ffm";
    }

    /**
     * Per-thread scratch for the downcall out-params, in place of an
     * {@link Arena#ofConfined()} per door: that is a fresh native allocation plus a scope
     * teardown on every single cast, measured at roughly 100 ns and the dominant cost of a
     * lean door. A confined arena is thread-confined by design, so this scratch is too: one
     * {@link ThreadLocal} holding segments that live as long as the thread does.
     *
     * <p>{@code out} is sized for the widest out-param (16 bytes — a protobuf timestamp, a
     * civil date-time or a decimal) and each door reads only its own prefix, after the
     * native side has written it; a failing call never reads it at all. Nothing is shared
     * between threads, so no door needs locking, and doors never nest, so no call can
     * observe another's scratch mid-flight. Under virtual threads that is one small off-heap
     * footprint per <em>virtual</em> thread, not per carrier — a few dozen bytes each, but
     * scaling with however many a server runs.
     *
     * <p>There is deliberately no input buffer here: the downcalls are linked
     * {@code critical(true)}, so the caller's own array crosses as a pinned heap segment,
     * with no copy into a native staging segment and no growable buffer behind one.
     */
    static final class Scratch {
        // Explicit alignment rather than allocate(size)'s implicit 1: the temporal doors
        // read JAVA_LONG out of `out`, which a 1-byte-aligned segment rejects outright.
        private final Arena fixed = Arena.ofAuto();
        private final MemorySegment out = fixed.allocate(16, 8);
        private final MemorySegment fault = fixed.allocate(NativeValues.FAULT_BYTES, 4);
        private final MemorySegment formatSegment = fixed.allocate(NativeValues.FORMAT_BYTES, 4);
        private NumFormat formatKey;
        private final MemorySegment periodsSegment = fixed.allocate(NativeValues.DAY_PERIODS_BYTES, 4);
        private DateTimeFormat periodsKey;

        private Scratch() {}

        MemorySegment format(NumFormat declared) {
            // Formats are reused constants in practice (INVARIANT, DETECT, a per-locale
            // instance), so an identity check skips the stores — and each symbol's UTF-8
            // encode — on the overwhelming majority of calls; the same memo the Python and
            // Ruby bindings keep.
            if (formatKey != declared) {
                // The whole layout is written, zero past each symbol: the memo means a
                // shorter symbol after a longer one must not leave the tail behind.
                NativeValues.writeFormat(declared, formatSegment, 0);
                formatKey = declared;
            }
            return formatSegment;
        }

        // NULL when nothing is declared — the DateOrder overloads, or a DateTimeFormat with
        // no designators — which the core reads exactly as it always has; otherwise the
        // designators, under the same identity memo the format keeps.
        MemorySegment periods(DateTimeFormat declared) {
            if (declared == null || declared.englishOnly()) {
                return MemorySegment.NULL;
            }
            if (periodsKey != declared) {
                NativeValues.writeDayPeriods(declared, periodsSegment, 0);
                periodsKey = declared;
            }
            return periodsSegment;
        }
    }

    private static final ThreadLocal<Scratch> SCRATCH = ThreadLocal.withInitial(Scratch::new);

    /** This thread's scratch. */
    static Scratch scratch() {
        return SCRATCH.get();
    }

    /** The core's packed version, {@code major << 16 | minor << 8 | patch}. */
    static int coreVersion() {
        // Outside the try: a core that will not load is its own LinkageError, which
        // Cast.isAvailable() answers false on, never an AssertionError around it.
        MemorySegment export = Core.HYPERCAST_VERSION;
        try {
            return (int) Downcalls.VERSION.invokeExact(export);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: hypercast_version downcall failed unexpectedly", t);
        }
    }

    // A zero-copy view over the caller's array. len == 0 never dereferences the pointer,
    // per the ABI contract, so an empty input crosses as NULL rather than a zero-length
    // heap view.
    private static MemorySegment input(byte[] utf8) {
        return utf8.length == 0 ? MemorySegment.NULL : MemorySegment.ofArray(utf8);
    }

    private static MemorySegment input(MemorySegment utf8) {
        return utf8.byteSize() == 0 ? MemorySegment.NULL : utf8;
    }

    // --- calls: each returns the core's code raw ---

    /** A text door with nothing declared beside the text. */
    static int plain(Scratch s, int door, byte[] in) {
        return plain(s, door, input(in), in.length);
    }

    static int plain(Scratch s, int door, MemorySegment in) {
        return plain(s, door, input(in), in.byteSize());
    }

    private static int plain(Scratch s, int door, MemorySegment in, long len) {
        MemorySegment export = switch (door) {
            case Door.BOOL -> Core.CAST_BOOL;
            case Door.CHAR -> Core.CAST_CHAR;
            case Door.UUID -> Core.CAST_UUID;
            case Door.TIMESTAMP -> Core.CAST_TIMESTAMP;
            case Door.DATE -> Core.CAST_DATE;
            case Door.TIME -> Core.CAST_TIME;
            case Door.DURATION -> Core.CAST_DURATION;
            default -> throw Door.unknown(door);
        };
        try {
            return (int) Downcalls.PLAIN.invokeExact(export, in, len, s.out, s.fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: downcall failed unexpectedly", t);
        }
    }

    /** A numeric door, under the declared format. */
    static int numeric(Scratch s, int door, byte[] in, NumFormat format) {
        return numeric(s, door, input(in), in.length, format);
    }

    static int numeric(Scratch s, int door, MemorySegment in, NumFormat format) {
        return numeric(s, door, input(in), in.byteSize(), format);
    }

    private static int numeric(Scratch s, int door, MemorySegment in, long len, NumFormat format) {
        MemorySegment export = switch (door) {
            case Door.I8 -> Core.CAST_I8;
            case Door.I16 -> Core.CAST_I16;
            case Door.I32 -> Core.CAST_I32;
            case Door.I64 -> Core.CAST_I64;
            case Door.U8 -> Core.CAST_U8;
            case Door.U16 -> Core.CAST_U16;
            case Door.U32 -> Core.CAST_U32;
            case Door.U64 -> Core.CAST_U64;
            case Door.F32 -> Core.CAST_F32;
            case Door.F64 -> Core.CAST_F64;
            case Door.DECIMAL -> Core.CAST_DECIMAL;
            default -> throw Door.unknown(door);
        };
        try {
            return (int) Downcalls.NUMERIC.invokeExact(export, in, len, s.format(format), s.out, s.fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: downcall failed unexpectedly", t);
        }
    }

    /** A text door with one declared code: the Unix unit, the Excel epoch, the date order. */
    static int declared(Scratch s, int door, byte[] in, int code) {
        return declared(s, door, input(in), in.length, code);
    }

    static int declared(Scratch s, int door, MemorySegment in, int code) {
        return declared(s, door, input(in), in.byteSize(), code);
    }

    private static int declared(Scratch s, int door, MemorySegment in, long len, int code) {
        MemorySegment export = switch (door) {
            case Door.UNIX -> Core.CAST_UNIX;
            case Door.EXCEL_SERIAL -> Core.CAST_EXCEL_SERIAL;
            case Door.DATE_ORDERED -> Core.CAST_DATE_ORDERED;
            default -> throw Door.unknown(door);
        };
        try {
            return (int) Downcalls.DECLARED.invokeExact(export, in, len, code, s.out, s.fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: downcall failed unexpectedly", t);
        }
    }

    /** The civil date-time door: the order, and the designators or {@code null} for none. */
    static int civil(Scratch s, byte[] in, int order, DateTimeFormat format) {
        return civil(s, input(in), in.length, order, format);
    }

    static int civil(Scratch s, MemorySegment in, int order, DateTimeFormat format) {
        return civil(s, input(in), in.byteSize(), order, format);
    }

    private static int civil(Scratch s, MemorySegment in, long len, int order, DateTimeFormat format) {
        // The export outside the try, in every call: a core that will not load surfaces as the
        // LinkageError it is, not wrapped as a downcall failure.
        MemorySegment export = Core.CAST_DATETIME;
        try {
            return (int) Downcalls.CIVIL.invokeExact(export, in, len, order, s.periods(format), s.out, s.fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: downcall failed unexpectedly", t);
        }
    }

    /** A typed door: a {@code double} the caller holds. */
    static int typed(Scratch s, int door, double value) {
        MemorySegment export = switch (door) {
            case Door.DECIMAL_FROM_F64 -> Core.CAST_DECIMAL_FROM_F64;
            case Door.EXCEL_TIME -> Core.CAST_EXCEL_TIME;
            case Door.EXCEL_DURATION -> Core.CAST_EXCEL_DURATION;
            default -> throw Door.unknown(door);
        };
        try {
            return (int) Downcalls.TYPED.invokeExact(export, value, s.out, s.fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: downcall failed unexpectedly", t);
        }
    }

    /** A typed door with one declared code: the Excel serial and its epoch. */
    static int typedDeclared(Scratch s, int door, double value, int code) {
        if (door != Door.EXCEL_SERIAL_FROM_F64) {
            throw Door.unknown(door);
        }
        MemorySegment export = Core.CAST_EXCEL_SERIAL_FROM_F64;
        try {
            return (int) Downcalls.TYPED_DECLARED.invokeExact(export, value, code, s.out, s.fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: downcall failed unexpectedly", t);
        }
    }

    // --- readers: the value the last successful call on `s` produced, or its fault ---

    static boolean bool(Scratch s) {
        return s.out.get(ValueLayout.JAVA_BYTE, 0) != 0;
    }

    /** The Unicode scalar the char door read, {@code 0..0x10FFFF}. */
    static int scalar(Scratch s) {
        return s.out.get(ValueLayout.JAVA_INT, 0);
    }

    static byte i8(Scratch s) {
        return s.out.get(ValueLayout.JAVA_BYTE, 0);
    }

    static short i16(Scratch s) {
        return s.out.get(ValueLayout.JAVA_SHORT, 0);
    }

    static int i32(Scratch s) {
        return s.out.get(ValueLayout.JAVA_INT, 0);
    }

    static long i64(Scratch s) {
        return s.out.get(ValueLayout.JAVA_LONG, 0);
    }

    static int u8(Scratch s) {
        return Byte.toUnsignedInt(s.out.get(ValueLayout.JAVA_BYTE, 0));
    }

    static int u16(Scratch s) {
        return Short.toUnsignedInt(s.out.get(ValueLayout.JAVA_SHORT, 0));
    }

    static long u32(Scratch s) {
        return Integer.toUnsignedLong(s.out.get(ValueLayout.JAVA_INT, 0));
    }

    /** The two's-complement bit pattern, as Java has no unsigned 64-bit primitive. */
    static long u64(Scratch s) {
        return s.out.get(ValueLayout.JAVA_LONG, 0);
    }

    static float f32(Scratch s) {
        return s.out.get(ValueLayout.JAVA_FLOAT, 0);
    }

    static double f64(Scratch s) {
        return s.out.get(ValueLayout.JAVA_DOUBLE, 0);
    }

    static BigDecimal decimal(Scratch s) {
        return NativeValues.decimal(s.out, 0);
    }

    static UUID uuid(Scratch s) {
        return NativeValues.uuid(s.out, 0);
    }

    static Instant instant(Scratch s) {
        return NativeValues.instant(s.out, 0);
    }

    static LocalDate date(Scratch s) {
        return NativeValues.date(s.out, 0);
    }

    static LocalDateTime dateTime(Scratch s) {
        return NativeValues.civil(s.out, 0);
    }

    static LocalTime time(Scratch s) {
        return NativeValues.time(s.out, 0);
    }

    static Duration duration(Scratch s) {
        return NativeValues.duration(s.out, 0);
    }

    static int faultOffset(Scratch s) {
        return s.fault.get(ValueLayout.JAVA_INT, 0);
    }

    static int faultLength(Scratch s) {
        return s.fault.get(ValueLayout.JAVA_INT, 4);
    }
}
