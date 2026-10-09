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
import java.lang.reflect.InvocationTargetException;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.Locale;
import java.util.UUID;

/**
 * Allocation-lean scalar casts — booleans, numerics, UUIDs, temporals — calling directly
 * into the native {@code libhypercast} shared library via the Java Foreign Function &amp;
 * Memory API (JEP 454; this binding's floor is JDK 25). Every door returns a {@link Verdict}: the
 * value, or a {@link Fault} with a closed reason and the offending byte span. Never throws
 * on bad input — the only exceptions here are caller bugs (a malformed {@link NumFormat}),
 * never data.
 *
 * <p>Door names mirror the native ABI ({@code i32}, {@code f64}, {@code timestamp}, …) so
 * the polyglot surface reads identically across bindings. Each door takes a {@link String}
 * (UTF-8-encoded into a fresh {@code byte[]}), raw UTF-8 {@code byte[]} — the native
 * contract, and the form whose {@link Fault} offsets need no mapping — or a
 * {@link MemorySegment} view of UTF-8 bytes, heap or native, for a caller that already
 * holds one buffer of many values (a mapped file, a direct buffer, one line of a CSV) and
 * wants to cast a slice of it without copying it out first.
 *
 * <p>Temporal doors come out at {@code java.time}'s full fidelity: {@link Instant},
 * {@link LocalTime}, and {@link Duration} all carry nanoseconds, so unlike the C# binding
 * (ticks) nothing truncates — the JVM is the one platform that keeps every digit the core
 * parses.
 *
 * <p>The native library rides inside the jar under {@code /native/{rid}/} and is picked by
 * platform at runtime (see {@link NativePlatform}). The input crosses without a copy: every
 * downcall is linked with {@link Linker.Option#critical(boolean) critical(true)}, so the
 * caller's own heap array or segment is handed to the native side directly — sound because
 * each door is a short, non-blocking parse over those bytes that never calls back into
 * Java, which is the exact profile that option exists for. What remains per call is the
 * verdict record, the boxed value it carries, and (for the {@link String} doors) the UTF-8
 * encode; the Rust core itself never allocates.
 *
 * <p>The same core also ships inside this jar as a {@code wasm32-wasip1} module, run by
 * <a href="https://www.graalvm.org/webassembly/">GraalWasm</a> when {@link #BACKEND_PROPERTY}
 * says so, when no native build exists for the running platform, or when the bundled one
 * will not load. That path needs
 * {@code org.graalvm.polyglot:polyglot} and {@code org.graalvm.polyglot:wasm} on the
 * classpath (optional dependencies, never pulled in transitively), serializes every call on
 * one lock, and costs several times a native downcall per door; {@link #backend()} reports
 * which path is active. Everything else — every door, every verdict, every exception and
 * message — is identical between the two.
 *
 * <p>Nothing loads until the first door is called. A consumer with a managed fallback gates
 * on {@link #isAvailable()} first — the one probe that never throws — because the doors do
 * not fall back: a core that failed to load is thrown from every door as the failure it was.
 */
public final class Cast {
    private Cast() {}

    /**
     * Name of the system property that picks the interop path: {@code "native"} for the FFM
     * downcalls into the bundled platform library, {@code "wasm"} for the bundled
     * {@code wasm32-wasip1} module run by GraalWasm. Unset means native when this platform's
     * library is bundled and loads, wasm otherwise.
     */
    public static final String BACKEND_PROPERTY = "hypercast.backend";

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
     * nothing: the class initializes on the first native-path door, and the JIT folds a
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
     * The loaded core: which path won, the library it resolved to, and the address of every
     * export in it — all {@code static final}, all resolved in this holder's own class init.
     * A holder rather than fields on {@code Cast} itself so that nothing loads until the
     * first door (or {@link Cast#backend()}/{@link Cast#nativeVersion()}) touches it, and so
     * that {@link Cast#isAvailable()} can observe a load failure without {@code Cast} having failed
     * to initialize: a door after a failed load throws the {@link NoClassDefFoundError} for
     * this class, exactly as it used to throw it for {@code Cast}.
     */
    private static final class Core {
        private Core() {}

        /** The library's base name: {@code libhypercast.so}, {@code hypercast.dll}. */
        private static final String LIBRARY = "hypercast";

        /**
         * Non-null only when the wasm path was selected — see the static block below. Every door
         * checks this one {@code static final} against {@code null} before its FFM path; the JIT
         * folds that check away, so the native path costs exactly what it did before a second
         * backend existed.
         */
        private static final Backend WASM;

        // Null on the wasm path: there is no library to look symbols up in, and the native
        // linker is never asked for (Downcalls stays uninitialized) — a platform the JDK has
        // no linker for can still run the module.
        private static final SymbolLookup LOOKUP;

        /*
         * Decides the interop path once, at class init, and never again. BACKEND_PROPERTY set
         * to "wasm" forces the GraalWasm backend; "native" forces FFM, and fails loudly when
         * this platform has no bundled library or the library will not load. Unset takes FFM
         * when this platform's native library is bundled and loads, and the wasm module
         * otherwise — an OS, architecture or C library this jar ships no native build for
         * still works, just through the module, and so does a bundled library that will not
         * open (a temp directory mounted noexec, say). When that last fallback cannot start
         * either, the failure thrown is the native one, with the wasm one suppressed on it.
         */
        static {
            String choice = System.getProperty(BACKEND_PROPERTY);
            if (choice != null && !"native".equals(choice) && !"wasm".equals(choice)) {
                throw new IllegalStateException(
                        BACKEND_PROPERTY + " must be \"native\" or \"wasm\"; got \"" + choice + "\"");
            }
            NativePlatform.Target target = NativePlatform.current(LIBRARY);
            Backend wasm = null;
            SymbolLookup lookup = null;
            if ("wasm".equals(choice)) {
                wasm = startWasm(null);
            } else if ("native".equals(choice)) {
                lookup = NativePlatform.load(Cast.class, LIBRARY, target);
            } else if (target == null || Cast.class.getResource(target.resourcePath()) == null) {
                wasm = startWasm(NativePlatform.missing(LIBRARY, target));
            } else {
                try {
                    lookup = NativePlatform.load(Cast.class, LIBRARY, target);
                } catch (RuntimeException | LinkageError nativeFailure) {
                    try {
                        wasm = startWasm("the bundled native library would not load (" + nativeFailure + ")");
                    } catch (RuntimeException wasmFailure) {
                        nativeFailure.addSuppressed(wasmFailure);
                        throw nativeFailure;
                    }
                }
            }
            WASM = wasm;
            LOOKUP = lookup;
        }

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

        // Null when the wasm backend is active — the addresses above are then never used, and
        // there is no library to look symbols up in.
        private static MemorySegment export(String symbol) {
            return LOOKUP == null ? null : LOOKUP.find(symbol).orElseThrow();
        }

        /**
         * Starts the GraalWasm backend. {@code nativeUnavailable} is why the native path was
         * not taken, or {@code null} when wasm was asked for by name — it only shapes the
         * message of a failure here.
         *
         * <p>{@link WasmBackend} is instantiated by name so that {@code org.graalvm.polyglot} is
         * never loaded unless it is actually going to be used: it is a {@code compileOnly}
         * dependency of this jar, present at runtime only if the consumer added it.
         */
        private static Backend startWasm(String nativeUnavailable) {
            if (Cast.class.getResource(WasmBackend.RESOURCE_PATH) == null) {
                throw new IllegalStateException(
                        nativeUnavailable == null
                                ? WasmBackend.RESOURCE_PATH + " classpath resource not found (this jar was built "
                                        + "without the wasm module)"
                                : nativeUnavailable + ", and " + WasmBackend.RESOURCE_PATH + " is not bundled either");
            }
            try {
                return (Backend) Class.forName(Cast.class.getPackageName() + ".WasmBackend")
                        .getDeclaredConstructor()
                        .newInstance();
            } catch (ReflectiveOperationException | LinkageError e) {
                // The constructor is where GraalWasm is first touched, so its absence arrives
                // wrapped: newInstance hands back whatever the constructor threw — an Error
                // included — inside an InvocationTargetException.
                Throwable cause = e instanceof InvocationTargetException && e.getCause() != null ? e.getCause() : e;
                if (cause instanceof NoClassDefFoundError) {
                    throw new IllegalStateException(
                            WasmBackend.GRAALWASM_MISSING
                                    + (nativeUnavailable == null
                                            ? ""
                                            : "; wasm was selected because " + nativeUnavailable),
                            cause);
                }
                if (cause instanceof RuntimeException re) {
                    throw re;
                }
                throw new IllegalStateException("hypercast: could not start the wasm backend", cause);
            }
        }
    }

    /**
     * Which interop path this process is using: {@code "native"} (FFM downcalls into the
     * bundled platform library) or {@code "wasm"} (the bundled {@code wasm32-wasip1} module run
     * by GraalWasm). Decided once, when the core is first loaded; see
     * {@link #BACKEND_PROPERTY}.
     *
     * @return {@code "native"} or {@code "wasm"}
     */
    public static String backend() {
        return Core.WASM == null ? "native" : Core.WASM.name();
    }

    /**
     * Whether the core resolved: the bundled platform library — or, on the wasm path, the
     * module — loaded, and every export this binding was built against was found in it.
     * Probed once, on first call, and cached; never throws. A library that will not open, a
     * jar built without a core for this platform, GraalWasm absent when the wasm path was
     * selected, an export missing from an older core — all come back {@code false}. This is
     * what a consumer with a managed fallback gates on before the first door: the doors
     * themselves throw the load failure they hit rather than fall back, because a door that
     * quietly answered from somewhere else would be a verdict lie. {@code true} exactly when
     * {@link #nativeVersion()} succeeds.
     *
     * @return {@code true} when the core is loaded and callable
     */
    public static boolean isAvailable() {
        return Availability.AVAILABLE;
    }

    // Its own holder so the answer is computed once and cached without Cast's own init
    // depending on the load. The probe is the version export: the cheapest crossing there
    // is, and reaching it proves Core initialized — every handle resolved.
    private static final class Availability {
        static final boolean AVAILABLE = probe();

        private Availability() {}

        private static boolean probe() {
            try {
                nativeVersion();
                return true;
            } catch (RuntimeException | LinkageError unavailable) {
                // Core failing to initialize surfaces as ExceptionInInitializerError (and
                // NoClassDefFoundError on every later touch) — LinkageErrors, whatever was
                // underneath: the loader's own IllegalStateException, an unopenable
                // library's IllegalArgumentException, a missing export's
                // NoSuchElementException. Nothing else is expected, and nothing else is
                // swallowed.
                return false;
            }
        }
    }

    /**
     * The version of the core this process actually loaded — the bundled platform library
     * or the wasm module — as {@code major.minor.patch}, decoded from the core's own
     * {@code hypercast_version} export. The probe a host uses to prove the library it
     * resolved is the one this binding was built against, before making the first cast.
     * Takes nothing and touches nothing; the only way it fails is the core not having
     * loaded, which it reports as the load failure itself.
     *
     * @return the loaded core's version as {@code "major.minor.patch"}
     */
    public static String nativeVersion() {
        int packed;
        if (Core.WASM != null) {
            packed = Core.WASM.version();
        } else {
            try {
                packed = (int) Downcalls.VERSION.invokeExact(Core.HYPERCAST_VERSION);
            } catch (Throwable t) {
                throw new AssertionError("hypercast: hypercast_version downcall failed unexpectedly", t);
            }
        }
        return NativeValues.version(packed);
    }

    // The five ABI shapes, each one line on the wasm path and one downcall on the native
    // one. Core.WASM is a static final, so the JIT folds the null check away on the FFM
    // path. Each shape has one handle, a static final constant in Downcalls; what a door
    // names is the address of its export, which is the handle's leading argument — a direct
    // downcall, exactly as before a second backend existed.
    private static int plain(
            MemorySegment export, Door door, MemorySegment in, long len, MemorySegment out, MemorySegment fault) {
        if (Core.WASM != null) {
            return Core.WASM.plain(door, in, len, out, fault);
        }
        try {
            return (int) Downcalls.PLAIN.invokeExact(export, in, len, out, fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: " + door.symbol() + " downcall failed unexpectedly", t);
        }
    }

    private static int numeric(
            MemorySegment export, Door door, MemorySegment in, long len, NumFormat format, Scratch scratch) {
        if (Core.WASM != null) {
            return Core.WASM.numeric(door, in, len, format, scratch.out, scratch.fault);
        }
        try {
            return (int)
                    Downcalls.NUMERIC.invokeExact(export, in, len, scratch.format(format), scratch.out, scratch.fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: " + door.symbol() + " downcall failed unexpectedly", t);
        }
    }

    private static int declared(
            MemorySegment export,
            Door door,
            MemorySegment in,
            long len,
            int discriminant,
            MemorySegment out,
            MemorySegment fault) {
        if (Core.WASM != null) {
            return Core.WASM.declared(door, in, len, discriminant, out, fault);
        }
        try {
            return (int) Downcalls.DECLARED.invokeExact(export, in, len, discriminant, out, fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: " + door.symbol() + " downcall failed unexpectedly", t);
        }
    }

    private static int typed(MemorySegment export, Door door, double value, MemorySegment out, MemorySegment fault) {
        if (Core.WASM != null) {
            return Core.WASM.typed(door, value, out, fault);
        }
        try {
            return (int) Downcalls.TYPED.invokeExact(export, value, out, fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: " + door.symbol() + " downcall failed unexpectedly", t);
        }
    }

    private static int typedDeclared(
            MemorySegment export, Door door, double value, int discriminant, MemorySegment out, MemorySegment fault) {
        if (Core.WASM != null) {
            return Core.WASM.typedDeclared(door, value, discriminant, out, fault);
        }
        try {
            return (int) Downcalls.TYPED_DECLARED.invokeExact(export, value, discriminant, out, fault);
        } catch (Throwable t) {
            throw new AssertionError("hypercast: " + door.symbol() + " downcall failed unexpectedly", t);
        }
    }

    /**
     * Per-thread scratch for the downcall out-params. Every door used to open its own
     * {@link Arena#ofConfined()} — a fresh native allocation plus a scope teardown on every
     * single cast, measured at roughly 100 ns and the dominant cost of the lean doors.
     * Confined arenas are thread-confined by design, so the replacement is thread-confined
     * too: one {@link ThreadLocal} holding segments that live as long as the thread does.
     *
     * <p>{@code out} is sized for the widest out-param (16 bytes — a protobuf timestamp, a
     * civil date-time or a decimal) and each door reads only its own prefix, after the
     * native side has written it; a failing call never reads it at all. Nothing is shared
     * between threads, so no door needs locking, and doors never nest, so no call can
     * observe another's scratch mid-flight. Under virtual threads that is one small off-heap
     * footprint per <em>virtual</em> thread, not per carrier — a few dozen bytes each, but
     * scaling with however many a server runs.
     *
     * <p>There is deliberately no input buffer here any more. The input used to be copied
     * into a per-thread native staging segment on every call; the downcalls are now linked
     * {@code critical(true)}, so the caller's own array crosses as a pinned heap segment and
     * the copy — and the growable buffer behind it — went with it.
     */
    private static final class Scratch {
        // Explicit alignment rather than allocate(size)'s implicit 1: the temporal doors
        // read JAVA_LONG out of `out`, which a 1-byte-aligned segment rejects outright.
        private final Arena fixed = Arena.ofAuto();
        final MemorySegment out = fixed.allocate(16, 8);
        final MemorySegment fault = fixed.allocate(NativeValues.FAULT_BYTES, 4);
        private final MemorySegment formatSegment = fixed.allocate(NativeValues.FORMAT_BYTES, 4);
        private NumFormat formatKey;

        MemorySegment format(NumFormat declared) {
            // Formats are reused constants in practice (INVARIANT, DETECT, a per-locale
            // instance), so an identity check skips the stores — and the symbol's UTF-8
            // encode — on the overwhelming majority of calls; the same memo the Python and
            // Ruby bindings keep.
            if (formatKey != declared) {
                // The whole 16-byte symbol field is written, zero past the symbol: the memo
                // means a shorter symbol after a longer one must not leave the tail behind.
                NativeValues.writeFormat(declared, formatSegment, 0);
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
        return NativeValues.fault(code, fault.get(ValueLayout.JAVA_INT, 0), fault.get(ValueLayout.JAVA_INT, 4));
    }

    private static byte[] utf8(String text) {
        return text.getBytes(StandardCharsets.UTF_8);
    }

    // A fault's span is byte offsets into the UTF-8 the core saw. For a String door that is
    // the wrong unit whenever the encoding came out longer than the String — non-ASCII input
    // — so the span is rebased to UTF-16 code units, and text.substring(offset, offset +
    // length) names the same characters the core faulted at. Length equality is the whole
    // ASCII check; the walk below runs only on a non-ASCII failure.
    private static <T> Verdict<T> chars(Verdict<T> verdict, String text, byte[] utf8) {
        if (utf8.length == text.length() || !(verdict instanceof Fault<T> fault)) {
            return verdict;
        }
        int start = charIndex(utf8, fault.offset());
        int end = charIndex(utf8, fault.offset() + fault.length());
        return new Fault<>(fault.reason(), start, end - start);
    }

    // How many UTF-16 code units the first byteOffset UTF-8 bytes decode to: one per ASCII
    // or lead byte, two for the lead of a four-byte sequence (a supplementary code point is
    // a surrogate pair), none for a continuation byte.
    private static int charIndex(byte[] utf8, int byteOffset) {
        int units = 0;
        for (int i = 0, n = Math.min(byteOffset, utf8.length); i < n; i++) {
            int b = utf8[i] & 0xFF;
            if ((b & 0xC0) != 0x80) {
                units += b >= 0xF0 ? 2 : 1;
            }
        }
        return units;
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
        byte[] utf8 = utf8(text);
        return chars(bool(utf8), text, utf8);
    }

    /**
     * See {@link #bool(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Boolean> bool(byte[] utf8) {
        return boolDoor(input(utf8), utf8.length);
    }

    /**
     * See {@link #bool(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Boolean> bool(MemorySegment utf8) {
        return boolDoor(input(utf8), utf8.byteSize());
    }

    private static Verdict<Boolean> boolDoor(MemorySegment in, long len) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code = plain(Core.CAST_BOOL, Door.BOOL, in, len, out, fault);
        return code == 0 ? new Success<>(out.get(ValueLayout.JAVA_BYTE, 0) != 0) : failed(code, fault);
    }

    // --- char ---

    /**
     * Casts char text: one character, either verbatim — the input is exactly one character,
     * read <em>before</em> trimming, so {@code " "} is a space and {@code "6"} is the digit
     * six — or a declared code point, ASCII-whitespace-trimmed and ASCII case-insensitive on
     * the prefix: decimal {@code 65}, {@code U+0041}, {@code 0x41}, {@code &H41}, or an HTML
     * numeric entity {@code &#65;} / {@code &#x41;} (the closing {@code ;} required).
     * Culture-insensitive — no {@link NumFormat}.
     *
     * <p>A code point past {@code U+10FFFF} or in the surrogate range is
     * {@link CastFailure#OUT_OF_RANGE}, as is any scalar above {@code U+FFFF}: a supplementary
     * character is a surrogate pair, not one {@code char}, so it faults over the trimmed input
     * rather than being split. A {@code String} of exactly one UTF-16 code unit is that
     * {@code char} as-is, a lone surrogate included, without crossing into the core.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Character> character(String text) {
        if (text.length() == 1) {
            return new Success<>(text.charAt(0));
        }
        byte[] utf8 = utf8(text);
        return chars(character(utf8), text, utf8);
    }

    /**
     * See {@link #character(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Character> character(byte[] utf8) {
        return charDoor(input(utf8), utf8.length);
    }

    /**
     * See {@link #character(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Character> character(MemorySegment utf8) {
        return charDoor(input(utf8), utf8.byteSize());
    }

    private static Verdict<Character> charDoor(MemorySegment in, long len) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code = plain(Core.CAST_CHAR, Door.CHAR, in, len, out, fault);
        if (code != 0) {
            return failed(code, fault);
        }
        int scalar = out.get(ValueLayout.JAVA_INT, 0);
        if (scalar <= Character.MAX_VALUE) {
            return new Success<>((char) scalar);
        }
        // A supplementary scalar the core accepted but one char cannot hold: out of range
        // over the token the core read, the input less its ASCII whitespace edges.
        long start = 0;
        long end = len;
        while (start < end && asciiWhitespace(in.get(ValueLayout.JAVA_BYTE, start))) {
            start++;
        }
        while (end > start && asciiWhitespace(in.get(ValueLayout.JAVA_BYTE, end - 1))) {
            end--;
        }
        return new Fault<>(CastFailure.OUT_OF_RANGE, (int) start, (int) (end - start));
    }

    // The core's trim set: Rust's u8::is_ascii_whitespace (no vertical tab).
    private static boolean asciiWhitespace(byte b) {
        return b == ' ' || b == '\t' || b == '\n' || b == '\f' || b == '\r';
    }

    // --- integers ---

    private interface IntReader<T> {
        T read(MemorySegment out);
    }

    private static <T> Verdict<T> numeric(
            MemorySegment export, Door door, MemorySegment in, long len, NumFormat format, IntReader<T> reader) {
        Scratch scratch = SCRATCH.get();
        int code = numeric(export, door, in, len, format, scratch);
        return code == 0 ? new Success<>(reader.read(scratch.out)) : failed(code, scratch.fault);
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
        byte[] utf8 = utf8(text);
        return chars(i8(utf8, format), text, utf8);
    }

    /**
     * See {@link #i8(String, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Byte> i8(String text, Locale locale) {
        return i8(text, NumFormat.from(locale));
    }

    /**
     * See {@link #i8(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Byte> i8(byte[] utf8, NumFormat format) {
        return numeric(
                Core.CAST_I8, Door.I8, input(utf8), utf8.length, format, out -> out.get(ValueLayout.JAVA_BYTE, 0));
    }

    /**
     * See {@link #i8(byte[], NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Byte> i8(byte[] utf8, Locale locale) {
        return i8(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #i8(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Byte> i8(MemorySegment utf8, NumFormat format) {
        return numeric(
                Core.CAST_I8, Door.I8, input(utf8), utf8.byteSize(), format, out -> out.get(ValueLayout.JAVA_BYTE, 0));
    }

    /**
     * See {@link #i8(MemorySegment, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Byte> i8(MemorySegment utf8, Locale locale) {
        return i8(utf8, NumFormat.from(locale));
    }

    /**
     * Casts integer text to a signed 16-bit value. Notation rules as {@link #i8(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Short> i16(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(i16(utf8, format), text, utf8);
    }

    /**
     * See {@link #i16(String, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Short> i16(String text, Locale locale) {
        return i16(text, NumFormat.from(locale));
    }

    /**
     * See {@link #i16(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Short> i16(byte[] utf8, NumFormat format) {
        return numeric(
                Core.CAST_I16, Door.I16, input(utf8), utf8.length, format, out -> out.get(ValueLayout.JAVA_SHORT, 0));
    }

    /**
     * See {@link #i16(byte[], NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Short> i16(byte[] utf8, Locale locale) {
        return i16(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #i16(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Short> i16(MemorySegment utf8, NumFormat format) {
        return numeric(
                Core.CAST_I16,
                Door.I16,
                input(utf8),
                utf8.byteSize(),
                format,
                out -> out.get(ValueLayout.JAVA_SHORT, 0));
    }

    /**
     * See {@link #i16(MemorySegment, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Short> i16(MemorySegment utf8, Locale locale) {
        return i16(utf8, NumFormat.from(locale));
    }

    /**
     * Casts integer text to a signed 32-bit value. Notation rules as {@link #i8(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> i32(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(i32(utf8, format), text, utf8);
    }

    /**
     * See {@link #i32(String, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> i32(String text, Locale locale) {
        return i32(text, NumFormat.from(locale));
    }

    /**
     * See {@link #i32(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> i32(byte[] utf8, NumFormat format) {
        return numeric(
                Core.CAST_I32, Door.I32, input(utf8), utf8.length, format, out -> out.get(ValueLayout.JAVA_INT, 0));
    }

    /**
     * See {@link #i32(byte[], NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> i32(byte[] utf8, Locale locale) {
        return i32(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #i32(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> i32(MemorySegment utf8, NumFormat format) {
        return numeric(
                Core.CAST_I32, Door.I32, input(utf8), utf8.byteSize(), format, out -> out.get(ValueLayout.JAVA_INT, 0));
    }

    /**
     * See {@link #i32(MemorySegment, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> i32(MemorySegment utf8, Locale locale) {
        return i32(utf8, NumFormat.from(locale));
    }

    /**
     * Casts integer text to a signed 64-bit value. Notation rules as {@link #i8(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> i64(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(i64(utf8, format), text, utf8);
    }

    /**
     * See {@link #i64(String, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> i64(String text, Locale locale) {
        return i64(text, NumFormat.from(locale));
    }

    /**
     * See {@link #i64(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> i64(byte[] utf8, NumFormat format) {
        return numeric(
                Core.CAST_I64, Door.I64, input(utf8), utf8.length, format, out -> out.get(ValueLayout.JAVA_LONG, 0));
    }

    /**
     * See {@link #i64(byte[], NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> i64(byte[] utf8, Locale locale) {
        return i64(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #i64(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> i64(MemorySegment utf8, NumFormat format) {
        return numeric(
                Core.CAST_I64,
                Door.I64,
                input(utf8),
                utf8.byteSize(),
                format,
                out -> out.get(ValueLayout.JAVA_LONG, 0));
    }

    /**
     * See {@link #i64(MemorySegment, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> i64(MemorySegment utf8, Locale locale) {
        return i64(utf8, NumFormat.from(locale));
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
        byte[] utf8 = utf8(text);
        return chars(u8(utf8, format), text, utf8);
    }

    /**
     * See {@link #u8(String, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> u8(String text, Locale locale) {
        return u8(text, NumFormat.from(locale));
    }

    /**
     * See {@link #u8(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u8(byte[] utf8, NumFormat format) {
        return numeric(
                Core.CAST_U8,
                Door.U8,
                input(utf8),
                utf8.length,
                format,
                out -> Byte.toUnsignedInt(out.get(ValueLayout.JAVA_BYTE, 0)));
    }

    /**
     * See {@link #u8(byte[], NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> u8(byte[] utf8, Locale locale) {
        return u8(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #u8(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u8(MemorySegment utf8, NumFormat format) {
        return numeric(
                Core.CAST_U8,
                Door.U8,
                input(utf8),
                utf8.byteSize(),
                format,
                out -> Byte.toUnsignedInt(out.get(ValueLayout.JAVA_BYTE, 0)));
    }

    /**
     * See {@link #u8(MemorySegment, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> u8(MemorySegment utf8, Locale locale) {
        return u8(utf8, NumFormat.from(locale));
    }

    /**
     * Casts integer text to an unsigned 16-bit value, widened to {@code int} ({@code 0..65535}).
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u16(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(u16(utf8, format), text, utf8);
    }

    /**
     * See {@link #u16(String, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> u16(String text, Locale locale) {
        return u16(text, NumFormat.from(locale));
    }

    /**
     * See {@link #u16(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u16(byte[] utf8, NumFormat format) {
        return numeric(
                Core.CAST_U16,
                Door.U16,
                input(utf8),
                utf8.length,
                format,
                out -> Short.toUnsignedInt(out.get(ValueLayout.JAVA_SHORT, 0)));
    }

    /**
     * See {@link #u16(byte[], NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> u16(byte[] utf8, Locale locale) {
        return u16(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #u16(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u16(MemorySegment utf8, NumFormat format) {
        return numeric(
                Core.CAST_U16,
                Door.U16,
                input(utf8),
                utf8.byteSize(),
                format,
                out -> Short.toUnsignedInt(out.get(ValueLayout.JAVA_SHORT, 0)));
    }

    /**
     * See {@link #u16(MemorySegment, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> u16(MemorySegment utf8, Locale locale) {
        return u16(utf8, NumFormat.from(locale));
    }

    /**
     * Casts integer text to an unsigned 32-bit value, widened to {@code long}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u32(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(u32(utf8, format), text, utf8);
    }

    /**
     * See {@link #u32(String, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> u32(String text, Locale locale) {
        return u32(text, NumFormat.from(locale));
    }

    /**
     * See {@link #u32(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u32(byte[] utf8, NumFormat format) {
        return numeric(
                Core.CAST_U32,
                Door.U32,
                input(utf8),
                utf8.length,
                format,
                out -> Integer.toUnsignedLong(out.get(ValueLayout.JAVA_INT, 0)));
    }

    /**
     * See {@link #u32(byte[], NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> u32(byte[] utf8, Locale locale) {
        return u32(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #u32(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u32(MemorySegment utf8, NumFormat format) {
        return numeric(
                Core.CAST_U32,
                Door.U32,
                input(utf8),
                utf8.byteSize(),
                format,
                out -> Integer.toUnsignedLong(out.get(ValueLayout.JAVA_INT, 0)));
    }

    /**
     * See {@link #u32(MemorySegment, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> u32(MemorySegment utf8, Locale locale) {
        return u32(utf8, NumFormat.from(locale));
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
        byte[] utf8 = utf8(text);
        return chars(u64(utf8, format), text, utf8);
    }

    /**
     * See {@link #u64(String, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> u64(String text, Locale locale) {
        return u64(text, NumFormat.from(locale));
    }

    /**
     * See {@link #u64(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u64(byte[] utf8, NumFormat format) {
        return numeric(
                Core.CAST_U64, Door.U64, input(utf8), utf8.length, format, out -> out.get(ValueLayout.JAVA_LONG, 0));
    }

    /**
     * See {@link #u64(byte[], NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> u64(byte[] utf8, Locale locale) {
        return u64(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #u64(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u64(MemorySegment utf8, NumFormat format) {
        return numeric(
                Core.CAST_U64,
                Door.U64,
                input(utf8),
                utf8.byteSize(),
                format,
                out -> out.get(ValueLayout.JAVA_LONG, 0));
    }

    /**
     * See {@link #u64(MemorySegment, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> u64(MemorySegment utf8, Locale locale) {
        return u64(utf8, NumFormat.from(locale));
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
        byte[] utf8 = utf8(text);
        return chars(f32(utf8, format), text, utf8);
    }

    /**
     * See {@link #f32(String, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Float> f32(String text, Locale locale) {
        return f32(text, NumFormat.from(locale));
    }

    /**
     * See {@link #f32(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Float> f32(byte[] utf8, NumFormat format) {
        return numeric(
                Core.CAST_F32, Door.F32, input(utf8), utf8.length, format, out -> out.get(ValueLayout.JAVA_FLOAT, 0));
    }

    /**
     * See {@link #f32(byte[], NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Float> f32(byte[] utf8, Locale locale) {
        return f32(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #f32(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Float> f32(MemorySegment utf8, NumFormat format) {
        return numeric(
                Core.CAST_F32,
                Door.F32,
                input(utf8),
                utf8.byteSize(),
                format,
                out -> out.get(ValueLayout.JAVA_FLOAT, 0));
    }

    /**
     * See {@link #f32(MemorySegment, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Float> f32(MemorySegment utf8, Locale locale) {
        return f32(utf8, NumFormat.from(locale));
    }

    /**
     * Casts real text to {@code double}. Notation rules as {@link #f32(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Double> f64(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(f64(utf8, format), text, utf8);
    }

    /**
     * See {@link #f64(String, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Double> f64(String text, Locale locale) {
        return f64(text, NumFormat.from(locale));
    }

    /**
     * See {@link #f64(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Double> f64(byte[] utf8, NumFormat format) {
        return numeric(
                Core.CAST_F64, Door.F64, input(utf8), utf8.length, format, out -> out.get(ValueLayout.JAVA_DOUBLE, 0));
    }

    /**
     * See {@link #f64(byte[], NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Double> f64(byte[] utf8, Locale locale) {
        return f64(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #f64(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Double> f64(MemorySegment utf8, NumFormat format) {
        return numeric(
                Core.CAST_F64,
                Door.F64,
                input(utf8),
                utf8.byteSize(),
                format,
                out -> out.get(ValueLayout.JAVA_DOUBLE, 0));
    }

    /**
     * See {@link #f64(MemorySegment, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Double> f64(MemorySegment utf8, Locale locale) {
        return f64(utf8, NumFormat.from(locale));
    }

    // --- decimal ---

    /**
     * Casts decimal text to a {@link BigDecimal}, exactly and canonically: no {@code double}
     * is ever formed on the way, so {@code 0.1} is one tenth and {@code 50%} is exactly
     * {@code 0.5}; exact trailing zeros in the fraction are trimmed, so the scale is minimal
     * ({@code 1.10}, {@code 1.1} and {@code 1.1000} all come out with a scale of 1, and
     * {@code 100} stays {@code 100} — integer zeros are never touched). Zero is scale 0 and
     * never negative. Notation rules as {@link #f32(String, NumFormat)}. The door never
     * rounds — nothing but a zero is ever dropped, so a magnitude past
     * 2<sup>96</sup>&minus;1, or more fractional precision than 28 places can hold once exact
     * trailing zeros are shed, is {@link CastFailure#OUT_OF_RANGE}, not a silent
     * approximation.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<BigDecimal> decimal(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(decimal(utf8, format), text, utf8);
    }

    /**
     * See {@link #decimal(String, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<BigDecimal> decimal(String text, Locale locale) {
        return decimal(text, NumFormat.from(locale));
    }

    /**
     * See {@link #decimal(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<BigDecimal> decimal(byte[] utf8, NumFormat format) {
        return numeric(Core.CAST_DECIMAL, Door.DECIMAL, input(utf8), utf8.length, format, Cast::readDecimal);
    }

    /**
     * See {@link #decimal(byte[], NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<BigDecimal> decimal(byte[] utf8, Locale locale) {
        return decimal(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #decimal(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<BigDecimal> decimal(MemorySegment utf8, NumFormat format) {
        return numeric(Core.CAST_DECIMAL, Door.DECIMAL, input(utf8), utf8.byteSize(), format, Cast::readDecimal);
    }

    /**
     * See {@link #decimal(MemorySegment, NumFormat)}, with the notation the locale's: its decimal and
     * group separators and currency symbol, every lenience on, as {@link NumFormat#from(Locale)}
     * derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<BigDecimal> decimal(MemorySegment utf8, Locale locale) {
        return decimal(utf8, NumFormat.from(locale));
    }

    private static BigDecimal readDecimal(MemorySegment out) {
        return NativeValues.decimal(out, 0);
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
        byte[] utf8 = utf8(text);
        return chars(uuid(utf8), text, utf8);
    }

    /**
     * See {@link #uuid(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<UUID> uuid(byte[] utf8) {
        return uuidDoor(input(utf8), utf8.length);
    }

    /**
     * See {@link #uuid(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<UUID> uuid(MemorySegment utf8) {
        return uuidDoor(input(utf8), utf8.byteSize());
    }

    private static Verdict<UUID> uuidDoor(MemorySegment in, long len) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code = plain(Core.CAST_UUID, Door.UUID, in, len, out, fault);
        if (code != 0) {
            return failed(code, fault);
        }
        return new Success<>(NativeValues.uuid(out, 0));
    }

    // --- temporals ---

    // A zero precision means the RFC 3339 door's plain shape; anything else is a declared
    // unit or epoch on the unix shape.
    private static Verdict<Instant> instantDoor(
            MemorySegment export, Door door, MemorySegment in, long len, int precision) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code = precision == 0
                ? plain(export, door, in, len, out, fault)
                : declared(export, door, in, len, precision, out, fault);
        return code == 0 ? new Success<>(NativeValues.instant(out, 0)) : failed(code, fault);
    }

    /**
     * Casts an RFC 3339 instant — {@code yyyy-MM-ddTHH:mm:ss[.f+](Z|±hh:mm)}, zone
     * <b>mandatory</b> — to an {@link Instant}, normalized to UTC at full nanosecond
     * fidelity. A zone-less or space-separated form is {@link CastFailure#MALFORMED}; an
     * instant outside 0001-01-01 to 9999-12-31 UTC is {@link CastFailure#OUT_OF_RANGE}.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> timestamp(String text) {
        byte[] utf8 = utf8(text);
        return chars(timestamp(utf8), text, utf8);
    }

    /**
     * See {@link #timestamp(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> timestamp(byte[] utf8) {
        return instantDoor(Core.CAST_TIMESTAMP, Door.TIMESTAMP, input(utf8), utf8.length, 0);
    }

    /**
     * See {@link #timestamp(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> timestamp(MemorySegment utf8) {
        return instantDoor(Core.CAST_TIMESTAMP, Door.TIMESTAMP, input(utf8), utf8.byteSize(), 0);
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
        byte[] utf8 = utf8(text);
        return chars(unix(utf8, precision), text, utf8);
    }

    /**
     * See {@link #unix(String, UnixPrecision)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param precision the declared unit of the epoch value
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> unix(byte[] utf8, UnixPrecision precision) {
        return instantDoor(Core.CAST_UNIX, Door.UNIX, input(utf8), utf8.length, precision.code());
    }

    /**
     * See {@link #unix(String, UnixPrecision)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @param precision the declared unit of the epoch value
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> unix(MemorySegment utf8, UnixPrecision precision) {
        return instantDoor(Core.CAST_UNIX, Door.UNIX, input(utf8), utf8.byteSize(), precision.code());
    }

    /**
     * Casts an Excel date serial under a caller-declared {@link ExcelEpoch} to an
     * {@link Instant}. The whole part counts days from the system's own day zero and the
     * fraction is the time of day, so {@code 45292.75} is 2024-01-01T18:00:00Z. A
     * spreadsheet cell carries no zone and none is invented.
     *
     * <p>The 1900 system contains a day that never existed: serial {@code 60} is
     * 1900-02-29, kept deliberately because Lotus 1-2-3 wrongly treated 1900 as a leap year
     * and Excel copied the bug for file compatibility. It is {@link CastFailure#OUT_OF_RANGE}
     * here — the same verdict {@link #date(String)} gives the text {@code 1900-02-29} — so
     * every serial above it is shifted one day against a naive count, which is the
     * arithmetic hand-rolled conversions get wrong.
     *
     * @param text the text to cast
     * @param epoch the declared date system
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> excelSerial(String text, ExcelEpoch epoch) {
        byte[] utf8 = utf8(text);
        return chars(excelSerial(utf8, epoch), text, utf8);
    }

    /**
     * See {@link #excelSerial(String, ExcelEpoch)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param epoch the declared date system
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> excelSerial(byte[] utf8, ExcelEpoch epoch) {
        return instantDoor(Core.CAST_EXCEL_SERIAL, Door.EXCEL_SERIAL, input(utf8), utf8.length, epoch.code());
    }

    /**
     * See {@link #excelSerial(String, ExcelEpoch)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @param epoch the declared date system
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> excelSerial(MemorySegment utf8, ExcelEpoch epoch) {
        return instantDoor(Core.CAST_EXCEL_SERIAL, Door.EXCEL_SERIAL, input(utf8), utf8.byteSize(), epoch.code());
    }

    /**
     * Casts a strict ISO 8601 {@code yyyy-MM-dd} calendar date to a {@link LocalDate}.
     * Anything time-bearing or non-ISO is {@link CastFailure#MALFORMED}; year 0000, month
     * 00 or 13+, or a day the month does not have is {@link CastFailure#OUT_OF_RANGE}, at
     * that field.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(String text) {
        byte[] utf8 = utf8(text);
        return chars(date(utf8), text, utf8);
    }

    /**
     * See {@link #date(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(byte[] utf8) {
        return dateDoor(input(utf8), utf8.length);
    }

    /**
     * See {@link #date(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(MemorySegment utf8) {
        return dateDoor(input(utf8), utf8.byteSize());
    }

    private static Verdict<LocalDate> dateDoor(MemorySegment in, long len) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code = plain(Core.CAST_DATE, Door.DATE, in, len, out, fault);
        return code == 0 ? new Success<>(NativeValues.date(out, 0)) : failed(code, fault);
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
        byte[] utf8 = utf8(text);
        return chars(date(utf8, order), text, utf8);
    }

    /**
     * See {@link #date(String, DateOrder)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(byte[] utf8, DateOrder order) {
        return dateOrderedDoor(input(utf8), utf8.length, order);
    }

    /**
     * See {@link #date(String, DateOrder)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(MemorySegment utf8, DateOrder order) {
        return dateOrderedDoor(input(utf8), utf8.byteSize(), order);
    }

    private static Verdict<LocalDate> dateOrderedDoor(MemorySegment in, long len, DateOrder order) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code = declared(Core.CAST_DATE_ORDERED, Door.DATE_ORDERED, in, len, order.code(), out, fault);
        return code == 0 ? new Success<>(NativeValues.date(out, 0)) : failed(code, fault);
    }

    /**
     * Casts a zone-less civil date-time — the shape untrusted feeds actually send
     * ({@code 1/7/2026 3:04 PM}, {@code 2026-01-07 15:04:05}) — under the caller-declared
     * {@link DateOrder} to a {@link LocalDateTime} at full nanosecond fidelity. The date
     * part follows {@link #date(String, DateOrder)}'s grammar; the optional time part (one
     * space or {@code T} after the date) is 24-hour {@code h:mm[:ss[.f+]]} or 12-hour
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
        byte[] utf8 = utf8(text);
        return chars(dateTime(utf8, order), text, utf8);
    }

    /**
     * See {@link #dateTime(String, DateOrder)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDateTime> dateTime(byte[] utf8, DateOrder order) {
        return dateTimeDoor(input(utf8), utf8.length, order);
    }

    /**
     * See {@link #dateTime(String, DateOrder)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDateTime> dateTime(MemorySegment utf8, DateOrder order) {
        return dateTimeDoor(input(utf8), utf8.byteSize(), order);
    }

    private static Verdict<LocalDateTime> dateTimeDoor(MemorySegment in, long len, DateOrder order) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code = declared(Core.CAST_DATETIME, Door.DATETIME, in, len, order.code(), out, fault);
        return code == 0 ? new Success<>(NativeValues.civil(out, 0)) : failed(code, fault);
    }

    /**
     * Casts an ISO 8601 24-hour time-of-day — {@code HH:mm}, {@code HH:mm:ss}, or
     * {@code HH:mm:ss.f+} — to a {@link LocalTime} at full nanosecond fidelity,
     * {@code 00:00} through {@code 23:59:59.999999999}. A well-formed hour past 23 or minute
     * or second past 59 ({@code 24:00}, {@code 15:04:60}) is
     * {@link CastFailure#OUT_OF_RANGE}, at that field.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalTime> time(String text) {
        byte[] utf8 = utf8(text);
        return chars(time(utf8), text, utf8);
    }

    /**
     * See {@link #time(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalTime> time(byte[] utf8) {
        return timeDoor(input(utf8), utf8.length);
    }

    /**
     * See {@link #time(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalTime> time(MemorySegment utf8) {
        return timeDoor(input(utf8), utf8.byteSize());
    }

    private static Verdict<LocalTime> timeDoor(MemorySegment in, long len) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code = plain(Core.CAST_TIME, Door.TIME, in, len, out, fault);
        return code == 0 ? new Success<>(NativeValues.time(out, 0)) : failed(code, fault);
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
        byte[] utf8 = utf8(text);
        return chars(duration(utf8), text, utf8);
    }

    /**
     * See {@link #duration(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Duration> duration(byte[] utf8) {
        return durationDoor(input(utf8), utf8.length);
    }

    /**
     * See {@link #duration(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Duration> duration(MemorySegment utf8) {
        return durationDoor(input(utf8), utf8.byteSize());
    }

    private static Verdict<Duration> durationDoor(MemorySegment in, long len) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        MemorySegment fault = scratch.fault;
        int code = plain(Core.CAST_DURATION, Door.DURATION, in, len, out, fault);
        return code == 0 ? new Success<>(NativeValues.duration(out, 0)) : failed(code, fault);
    }

    // --- typed doors: a number the caller already holds ---

    /**
     * Reads a number the caller already holds — the {@code double} a workbook stores for a
     * numeric cell — as the exact {@link BigDecimal} it names: the shortest decimal that rounds
     * back to the double, the digits a spreadsheet writes for it. So {@code 0.1} is one tenth,
     * not the binary fraction {@code new BigDecimal(0.1)} spells out, and {@code 0.1 + 0.2} is
     * {@code 0.30000000000000004}. The result is canonical, as
     * {@link #decimal(String, NumFormat)}'s is. {@code NaN} is {@link CastFailure#MALFORMED};
     * an infinity, a magnitude past 2<sup>96</sup>&minus;1 or more than 28 places is
     * {@link CastFailure#OUT_OF_RANGE} — the digits are never cut to fit. A typed door's
     * {@link Fault} has no span: its offset and length are 0.
     *
     * @param value the number to read
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<BigDecimal> decimalFromDouble(double value) {
        Scratch scratch = SCRATCH.get();
        int code = typed(Core.CAST_DECIMAL_FROM_F64, Door.DECIMAL_FROM_F64, value, scratch.out, scratch.fault);
        return code == 0 ? new Success<>(readDecimal(scratch.out)) : failed(code, scratch.fault);
    }

    /**
     * Reads an Excel date serial the caller already holds as a {@code double} under a declared
     * {@link ExcelEpoch} — the twin of {@link #excelSerial(String, ExcelEpoch)} for a workbook
     * reader that has the cell's number and no text. The result is the zone-less wall clock
     * the cell holds, a {@link LocalDateTime} as {@link #dateTime(String, DateOrder)} returns,
     * its fraction snapped: the time with the fewest fractional-second digits that the same double
     * stores, so Excel's 23:59:59 is read on the second rather than the nanoseconds of float
     * noise the double carries. The 1900 system's phantom serial
     * {@code 60}, a serial below the system's first day and one past 9999-12-31 are
     * {@link CastFailure#OUT_OF_RANGE}; a negative, NaN or infinite serial is
     * {@link CastFailure#MALFORMED}.
     *
     * @param value the serial
     * @param epoch the declared date system
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDateTime> excelSerialFromDouble(double value, ExcelEpoch epoch) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        int code = typedDeclared(
                Core.CAST_EXCEL_SERIAL_FROM_F64, Door.EXCEL_SERIAL_FROM_F64, value, epoch.code(), out, scratch.fault);
        return code == 0 ? new Success<>(NativeValues.civil(out, 0)) : failed(code, scratch.fault);
    }

    /**
     * Reads the fraction of an Excel serial the caller already holds as a {@code double} as a
     * {@link LocalTime} at full nanosecond fidelity: {@code 0.75} and {@code 45292.75} are
     * both 18:00. Snapped as the serial door snaps, and a fraction that snaps to a whole day is
     * midnight. A negative, NaN or infinite serial is {@link CastFailure#MALFORMED}; one
     * past 9999-12-31 is {@link CastFailure#OUT_OF_RANGE}.
     *
     * @param value the serial
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalTime> excelTime(double value) {
        Scratch scratch = SCRATCH.get();
        int code = typed(Core.CAST_EXCEL_TIME, Door.EXCEL_TIME, value, scratch.out, scratch.fault);
        return code == 0 ? new Success<>(NativeValues.time(scratch.out, 0)) : failed(code, scratch.fault);
    }

    /**
     * Reads a number of days the caller already holds as a {@code double} — what an
     * elapsed-time format ({@code [h]:mm:ss}) stores — as a {@link Duration} at full
     * nanosecond fidelity: {@code 1.5} is a day and twelve hours, and a negative span is
     * negative, its size snapped as a serial's time. NaN or an infinity is
     * {@link CastFailure#MALFORMED}; beyond ±10,000 years is {@link CastFailure#OUT_OF_RANGE}.
     *
     * @param value the number of days
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Duration> excelDuration(double value) {
        Scratch scratch = SCRATCH.get();
        MemorySegment out = scratch.out;
        int code = typed(Core.CAST_EXCEL_DURATION, Door.EXCEL_DURATION, value, out, scratch.fault);
        return code == 0 ? new Success<>(NativeValues.duration(out, 0)) : failed(code, scratch.fault);
    }
}
