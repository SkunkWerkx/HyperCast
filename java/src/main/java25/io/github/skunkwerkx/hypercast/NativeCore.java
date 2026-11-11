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
import java.nio.ByteBuffer;
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
 * <p>Every door crosses through one of the four detailed exports ({@code cast_detailed} and its
 * three siblings), which alone take the grouping policy a {@link NumFormat} declares, date
 * options and a month vocabulary, and report what failed. Every export is looked up in
 * {@link Core}, the four detailed ones included, so a 1.0 core, which would ignore a declared
 * grouping policy, fails the load rather than parse by 1.0's rule; the 1.0 exports are looked up
 * too, and nothing calls them.
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

        // (door, param, format, periods, date_options, vocab_ptr, vocab_len, ptr, len, out,
        // fault) -> code — cast_detailed: every text door, with whatever it declares.
        private static final MethodHandle DETAILED = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS,
                        ValueLayout.JAVA_LONG,
                        ValueLayout.ADDRESS,
                        ValueLayout.JAVA_LONG,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS),
                CRITICAL);
        // (door, param, value, out, fault) -> code — cast_detailed_from_f64: the typed doors,
        // which read a double the caller holds.
        private static final MethodHandle DETAILED_FROM_F64 = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_DOUBLE,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS),
                CRITICAL);
        // (door, param, format, constraints, enum_ptr, enum_len, ptr, len, out, fault, check)
        // -> code — cast_checked_detailed: a numeric door or Text, judged by a declaration.
        private static final MethodHandle CHECKED = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS,
                        ValueLayout.JAVA_LONG,
                        ValueLayout.ADDRESS,
                        ValueLayout.JAVA_LONG,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS),
                CRITICAL);
        // (door, param, constraints, enum_ptr, enum_len, value, out, fault, check) -> code —
        // cast_checked_detailed_from_f64: the checked door over a double a source already decoded.
        private static final MethodHandle CHECKED_FROM_F64 = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS,
                        ValueLayout.JAVA_LONG,
                        ValueLayout.JAVA_DOUBLE,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS),
                CRITICAL);
        // (door, param, constraints, enum_ptr, enum_len) -> code — a declaration judged once.
        private static final MethodHandle CONSTRAINTS_CHECK = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT,
                        ValueLayout.JAVA_INT,
                        ValueLayout.ADDRESS,
                        ValueLayout.ADDRESS,
                        ValueLayout.JAVA_LONG),
                CRITICAL);
        // (ptr, len, out) -> code — a declared decimal through the core's conversion.
        private static final MethodHandle DECLARED_DECIMAL = LINKER.downcallHandle(
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS),
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
        private static final MemorySegment CAST_CHECKED = export("cast_checked");
        private static final MemorySegment CAST_CHECKED_FROM_F64 = export("cast_checked_from_f64");
        private static final MemorySegment CAST_CONSTRAINTS_CHECK = export("cast_constraints_check");
        private static final MemorySegment CAST_TEXT = export("cast_text");
        private static final MemorySegment CAST_DECLARED_DECIMAL = export("cast_declared_decimal");
        private static final MemorySegment CAST_DETAILED = export("cast_detailed");
        private static final MemorySegment CAST_DETAILED_FROM_F64 = export("cast_detailed_from_f64");
        private static final MemorySegment CAST_CHECKED_DETAILED = export("cast_checked_detailed");
        private static final MemorySegment CAST_CHECKED_DETAILED_FROM_F64 = export("cast_checked_detailed_from_f64");

        private static MemorySegment export(String symbol) {
            return LOOKUP.find(symbol)
                    .orElseThrow(() -> new java.util.NoSuchElementException("libhypercast has no export " + symbol
                            + ": a core older than this binding (a 1.0 core has none of the cast_detailed* exports)"));
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
        // A RawFaultDetail: the caller sets `size` once, the core never writes it.
        private final MemorySegment fault = fixed.allocate(NativeValues.FAULT_DETAIL_BYTES, 4);
        private final MemorySegment formatSegment = fixed.allocate(NativeValues.FORMAT_BYTES, 4);
        private NumFormat formatKey;
        private final MemorySegment periodsSegment = fixed.allocate(NativeValues.DAY_PERIODS_BYTES, 4);
        private DateTimeFormat periodsKey;
        // A checked door's report, and the declaration it was judged by: the 104-byte layout
        // copied into aligned memory and the enum list's segment, under the same identity memo
        // the format keeps.
        private final MemorySegment check = fixed.allocate(NativeValues.CHECK_BYTES, 4);
        private final MemorySegment constraintsSegment = fixed.allocate(NativeValues.CONSTRAINTS_BYTES, 8);
        private Constraints<?> constraintsKey;
        private MemorySegment entriesSegment = MemorySegment.NULL;

        private final MemorySegment dateOptionsSegment = fixed.allocate(NativeValues.DATE_OPTIONS_BYTES, 4);
        private DateOptions dateOptionsKey;

        private Scratch() {
            fault.set(ValueLayout.JAVA_INT, 0, (int) NativeValues.FAULT_DETAIL_BYTES);
        }

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

        // The declaration's layout, and its enum list (NULL for none) left in entriesSegment.
        MemorySegment constraints(Constraints<?> declared) {
            if (constraintsKey != declared) {
                byte[] raw = declared.raw();
                MemorySegment.copy(raw, 0, constraintsSegment, ValueLayout.JAVA_BYTE, 0, raw.length);
                ByteBuffer entries = declared.entries();
                entriesSegment = entries == null ? MemorySegment.NULL : MemorySegment.ofBuffer(entries);
                constraintsKey = declared;
            }
            return constraintsSegment;
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

        // NULL when nothing is declared, which the core reads as no options; otherwise the
        // layout, under the same identity memo.
        MemorySegment dateOptions(DateOptions declared) {
            if (declared == null || declared.isNone()) {
                return MemorySegment.NULL;
            }
            if (dateOptionsKey != declared) {
                NativeValues.writeDateOptions(declared.flags(), declared.pivot(), dateOptionsSegment, 0);
                dateOptionsKey = declared;
            }
            return dateOptionsSegment;
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

    // Java's spans are ints, so a segment longer than Integer.MAX_VALUE bytes is out_of_range
    // (too_long) at every door, spanning the most a span holds, before the core is asked: no
    // value or fault it could report past there would fit, and nothing past there is read.
    private static boolean tooLong(MemorySegment in) {
        return in.byteSize() > Integer.MAX_VALUE;
    }

    private static int tooLong(Scratch s) {
        s.fault.set(ValueLayout.JAVA_INT, 4, 0);
        s.fault.set(ValueLayout.JAVA_INT, 8, Integer.MAX_VALUE);
        s.fault.set(ValueLayout.JAVA_INT, 12, TOO_LONG);
        return OUT_OF_RANGE;
    }

    private static final int OUT_OF_RANGE = 3;
    private static final int TOO_LONG = 21;

    // --- calls: each returns the core's code raw ---

    // Every text door is cast_detailed: `door` is the core's code and `param` what it declares.
    // A layout that is not declared crosses as NULL, which the core reads as none; each
    // declaration is read only at the doors it is for, and a pointer a door ignores is never
    // looked at.
    private static int detailed(
            Scratch s,
            int door,
            int param,
            MemorySegment format,
            MemorySegment periods,
            MemorySegment dateOptions,
            MemorySegment vocabulary,
            MemorySegment in,
            long len) {
        // The export outside the try, in every call: a core that will not load surfaces as the
        // LinkageError it is, not wrapped as a downcall failure.
        MemorySegment export = Core.CAST_DETAILED;
        try {
            return (int) Downcalls.DETAILED.invokeExact(
                    export,
                    door,
                    param,
                    format,
                    periods,
                    dateOptions,
                    vocabulary,
                    vocabulary.byteSize(),
                    in,
                    len,
                    s.out,
                    s.fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: downcall failed unexpectedly", t);
        }
    }

    /** A text door with nothing declared beside the text. */
    static int plain(Scratch s, int door, byte[] in) {
        return detailed(
                s,
                door,
                0,
                MemorySegment.NULL,
                MemorySegment.NULL,
                MemorySegment.NULL,
                MemorySegment.NULL,
                input(in),
                in.length);
    }

    static int plain(Scratch s, int door, MemorySegment in) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return detailed(
                s,
                door,
                0,
                MemorySegment.NULL,
                MemorySegment.NULL,
                MemorySegment.NULL,
                MemorySegment.NULL,
                input(in),
                in.byteSize());
    }

    /** A numeric door, under the declared format. */
    static int numeric(Scratch s, int door, byte[] in, NumFormat format) {
        return detailed(
                s,
                door,
                0,
                s.format(format),
                MemorySegment.NULL,
                MemorySegment.NULL,
                MemorySegment.NULL,
                input(in),
                in.length);
    }

    static int numeric(Scratch s, int door, MemorySegment in, NumFormat format) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return detailed(
                s,
                door,
                0,
                s.format(format),
                MemorySegment.NULL,
                MemorySegment.NULL,
                MemorySegment.NULL,
                input(in),
                in.byteSize());
    }

    /** A text door with one declared code: the Unix unit, the Excel epoch, the date order. */
    static int declared(Scratch s, int door, byte[] in, int code) {
        return detailed(
                s,
                door,
                code,
                MemorySegment.NULL,
                MemorySegment.NULL,
                MemorySegment.NULL,
                MemorySegment.NULL,
                input(in),
                in.length);
    }

    static int declared(Scratch s, int door, MemorySegment in, int code) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return detailed(
                s,
                door,
                code,
                MemorySegment.NULL,
                MemorySegment.NULL,
                MemorySegment.NULL,
                MemorySegment.NULL,
                input(in),
                in.byteSize());
    }

    /** The civil date-time door: the order, and the designators or {@code null} for none. */
    static int civil(Scratch s, byte[] in, int order, DateTimeFormat format) {
        return detailed(
                s,
                Door.DATETIME,
                order,
                MemorySegment.NULL,
                s.periods(format),
                MemorySegment.NULL,
                MemorySegment.NULL,
                input(in),
                in.length);
    }

    static int civil(Scratch s, MemorySegment in, int order, DateTimeFormat format) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return detailed(
                s,
                Door.DATETIME,
                order,
                MemorySegment.NULL,
                s.periods(format),
                MemorySegment.NULL,
                MemorySegment.NULL,
                input(in),
                in.byteSize());
    }

    /**
     * A date door under {@link DateOptions} and a month vocabulary, either {@code null} for
     * none: the timestamp, date, ordered date and civil date-time doors. {@code param} is the
     * order at the last two; {@code format} the designators at the civil door only.
     */
    static int dated(
            Scratch s, int door, int param, byte[] in, DateTimeFormat format, DateOptions options, MonthNames months) {
        return detailed(
                s,
                door,
                param,
                MemorySegment.NULL,
                s.periods(format),
                s.dateOptions(options),
                vocabulary(months),
                input(in),
                in.length);
    }

    static int dated(
            Scratch s,
            int door,
            int param,
            MemorySegment in,
            DateTimeFormat format,
            DateOptions options,
            MonthNames months) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return detailed(
                s,
                door,
                param,
                MemorySegment.NULL,
                s.periods(format),
                s.dateOptions(options),
                vocabulary(months),
                input(in),
                in.byteSize());
    }

    // The packed vocabulary in native memory, or NULL for none.
    private static MemorySegment vocabulary(MonthNames months) {
        return months == null ? MemorySegment.NULL : MemorySegment.ofBuffer(months.vocabulary());
    }

    /** A typed door: a {@code double} the caller holds. */
    static int typed(Scratch s, int door, double value) {
        return typedDeclared(s, door, value, 0);
    }

    /** A typed door with one declared code: the Excel serial and its epoch. */
    static int typedDeclared(Scratch s, int door, double value, int code) {
        MemorySegment export = Core.CAST_DETAILED_FROM_F64;
        try {
            return (int) Downcalls.DETAILED_FROM_F64.invokeExact(export, door, code, value, s.out, s.fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: downcall failed unexpectedly", t);
        }
    }

    /**
     * A checked door: {@code door} is the core's door code ({@link Constraints#I32},
     * {@link Constraints#TEXT}, ...), {@code format} the declared notation or {@code null} (the
     * text door reads none), {@code constraints} the declaration already bound to the door.
     */
    static int checked(Scratch s, int door, byte[] in, NumFormat format, Constraints<?> constraints) {
        return checked(s, door, input(in), in.length, format, constraints);
    }

    static int checked(Scratch s, int door, MemorySegment in, NumFormat format, Constraints<?> constraints) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return checked(s, door, input(in), in.byteSize(), format, constraints);
    }

    private static int checked(
            Scratch s, int door, MemorySegment in, long len, NumFormat format, Constraints<?> constraints) {
        MemorySegment export = Core.CAST_CHECKED_DETAILED;
        MemorySegment notation = format == null ? MemorySegment.NULL : s.format(format);
        MemorySegment declared = s.constraints(constraints);
        MemorySegment entries = s.entriesSegment;
        try {
            return (int) Downcalls.CHECKED.invokeExact(
                    export, door, 0, notation, declared, entries, entries.byteSize(), in, len, s.out, s.fault, s.check);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: downcall failed unexpectedly", t);
        }
    }

    /** A checked door over a {@code double} a source already decoded. */
    static int checkedFromF64(Scratch s, int door, double value, Constraints<?> constraints) {
        MemorySegment export = Core.CAST_CHECKED_DETAILED_FROM_F64;
        MemorySegment declared = s.constraints(constraints);
        MemorySegment entries = s.entriesSegment;
        try {
            return (int) Downcalls.CHECKED_FROM_F64.invokeExact(
                    export, door, 0, declared, entries, entries.byteSize(), value, s.out, s.fault, s.check);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: downcall failed unexpectedly", t);
        }
    }

    /**
     * The core's judgement of a month vocabulary, 1 when it is whole and -1 when the core
     * refuses it: the vocabulary is judged before the (empty) input is read, so an acceptable
     * one reads nothing and answers {@code empty}.
     */
    static int monthNamesCheck(ByteBuffer vocabulary) {
        return detailed(
                scratch(),
                Door.DATE_ORDERED,
                1,
                MemorySegment.NULL,
                MemorySegment.NULL,
                MemorySegment.NULL,
                MemorySegment.ofBuffer(vocabulary),
                MemorySegment.NULL,
                0);
    }

    /** The core's judgement of a declaration: 0, or the refusal code. */
    static int constraintsCheck(int door, byte[] raw, ByteBuffer entries) {
        MemorySegment export = Core.CAST_CONSTRAINTS_CHECK;
        // Once per declaration, so a confined arena of its own rather than the scratch.
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment declared = arena.allocate(NativeValues.CONSTRAINTS_BYTES, 8);
            MemorySegment.copy(raw, 0, declared, ValueLayout.JAVA_BYTE, 0, raw.length);
            MemorySegment list = entries == null ? MemorySegment.NULL : MemorySegment.ofBuffer(entries);
            return (int) Downcalls.CONSTRAINTS_CHECK.invokeExact(export, door, 0, declared, list, list.byteSize());
        } catch (Throwable t) {
            throw new AssertionError("hypercast: downcall failed unexpectedly", t);
        }
    }

    /** The text door: the span of valid UTF-8 input. */
    static int text(Scratch s, byte[] in) {
        return plain(s, Door.TEXT, in);
    }

    static int text(Scratch s, MemorySegment in) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return plain(s, Door.TEXT, in);
    }

    /** The core's conversion of a declared decimal: 0 with the value, or 8. */
    static int declaredDecimal(Scratch s, byte[] utf8) {
        MemorySegment export = Core.CAST_DECLARED_DECIMAL;
        try {
            return (int) Downcalls.DECLARED_DECIMAL.invokeExact(export, input(utf8), (long) utf8.length, s.out);
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

    /** The text door's span: the byte offset of the (trimmed) text. */
    static int spanOffset(Scratch s) {
        return s.out.get(ValueLayout.JAVA_INT, 0);
    }

    /** The text door's span: the byte length of the (trimmed) text. */
    static int spanLength(Scratch s) {
        return s.out.get(ValueLayout.JAVA_INT, 4);
    }

    /** A checked door's keyword code on a violation; 0 otherwise. */
    static int checkKeyword(Scratch s) {
        return s.check.get(ValueLayout.JAVA_INT, 0);
    }

    /** Whether a checked door's rounding changed the value. */
    static boolean checkAdjusted(Scratch s) {
        return s.check.get(ValueLayout.JAVA_INT, 4) != 0;
    }

    /** The text {@code enum} entry a checked door matched, or -1 for none. */
    static int checkEnumIndex(Scratch s) {
        return s.check.get(ValueLayout.JAVA_INT, 8);
    }

    // The RawFaultDetail: size, then offset, length and detail.
    static int faultOffset(Scratch s) {
        return s.fault.get(ValueLayout.JAVA_INT, 4);
    }

    static int faultLength(Scratch s) {
        return s.fault.get(ValueLayout.JAVA_INT, 8);
    }

    /** The detail code of the fault the last failed call wrote: 1 or more, or 0 for a violation. */
    static int faultDetail(Scratch s) {
        return s.fault.get(ValueLayout.JAVA_INT, 12);
    }
}
