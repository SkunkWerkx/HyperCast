package io.github.skunkwerkx.hypercast;

import io.github.skunkwerkx.hypercast.interop.NativeValues;
import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.ByteOrder;
import org.graalvm.polyglot.Context;
import org.graalvm.polyglot.Source;
import org.graalvm.polyglot.Value;
import org.graalvm.polyglot.io.ByteSequence;

/**
 * The Rust core as a {@code wasm32-wasip1} module, run inside the JVM by
 * <a href="https://www.graalvm.org/webassembly/">GraalWasm</a>. No native binary, no
 * {@code java.lang.foreign} downcall: the same twenty-six {@code cast_*} exports (and the
 * {@code hypercast_version} probe) {@link Cast} downcalls into natively are called through
 * the polyglot API instead, on the module bundled at
 * {@code /native/wasm32-wasip1/hypercast.wasm}. The typed doors take a {@code double}, which
 * crosses as a wasm {@code f64} with nothing to stage.
 *
 * <p><b>Memory protocol.</b> A wasm guest only sees its own linear memory, so nothing here
 * can hand the core a pointer into a Java array the way the FFM path pins a {@code byte[]}.
 * The input text is copied into a grow-only guest buffer, and the four buffers the doors
 * read or fill — the 16-byte out-value, the 8-byte fault span, the 92-byte {@link NumFormat}
 * and the 40-byte day-period designators of a {@link DateTimeFormat} — are guest
 * allocations made once at load. All of it comes from the module's own exported
 * {@code malloc} (wasi-libc's, which is also what Rust's allocator sits on for this target)
 * and is read back out through the exported {@code memory} into the caller's own scratch
 * segments. Using the guest allocator rather than a host-picked offset is load-bearing:
 * dlmalloc claims the tail of the initial memory on first use, and HyperUuid observed a
 * buffer written at a "free-looking" offset past the data segments corrupted by the very
 * next allocation.
 *
 * <p><b>Threading.</b> One {@link Context} and one module instance serve the whole process,
 * and every call is serialized on this object's monitor — a polyglot context does not
 * permit concurrent multi-threaded access. The FFM path has no such lock; that is one of
 * the real costs of this backend, alongside the per-call price measured in the README.
 *
 * <p><b>WASI.</b> The module imports four {@code wasi_snapshot_preview1} functions
 * ({@code environ_get}, {@code environ_sizes_get}, {@code fd_write} and {@code proc_exit},
 * from wasi-libc's startup and panic paths; the core itself needs no clock and no entropy).
 * GraalWasm's built-in preview 1 implementation supplies them
 * ({@code wasm.Builtins=wasi_snapshot_preview1}); nothing is preopened, so the guest sees
 * no files, no environment and no arguments.
 *
 * <p>Instantiated by name from {@link Cast} so that {@code org.graalvm.polyglot} is only
 * ever loaded when this backend was selected.
 */
final class WasmBackend implements Backend {
    static final String RESOURCE_PATH = "/native/wasm32-wasip1/hypercast.wasm";

    // What a consumer who selected this backend without its optional dependencies is told.
    // A compile-time constant, so Cast can name it without loading this class.
    static final String GRAALWASM_MISSING = "hypercast: the wasm backend needs GraalWasm on the "
            + "classpath — add org.graalvm.polyglot:polyglot and org.graalvm.polyglot:wasm "
            + "(the latter is a POM-type dependency)";

    private static final ByteOrder BIG_ENDIAN = ByteOrder.BIG_ENDIAN;
    // Eight input bytes at a time, in the order they sit in the caller's segment: read
    // big-endian, written big-endian, so the guest sees the same byte sequence.
    private static final ValueLayout.OfLong BE_LONG_UNALIGNED = ValueLayout.JAVA_LONG_UNALIGNED.withOrder(BIG_ENDIAN);

    private final Context context;
    private final Value memory;

    // Each export resolved once and indexed by Door ordinal. HyperUuid measured this under
    // GraalVM's JIT: a cached Value's execute() is the cheapest shape of the call, against
    // more than twice the cost for invokeMember by name — the name lookup is what costs,
    // not the wasm.
    private final Value mallocFn;
    private final Value freeFn;
    private final Value versionFn;
    private final Value[] doors = new Value[Door.values().length];

    // Sixteen bytes read back from the guest per door; guarded by the same monitor as
    // every call, so one array serves the process.
    private final byte[] readback = new byte[16];

    // Guest addresses allocated once for the process: the out-value (16 bytes covers the
    // widest door; malloc's alignment satisfies the i64 the temporal doors write), the
    // fault span, the NumFormat the numeric doors read, and the day periods the civil
    // date-time door reads.
    private final int outPtr;
    private final int faultPtr;
    private final int formatPtr;
    private final int periodsPtr;

    // Host-side images of the two declared layouts, written by the same interop code the
    // native path uses and copied into the guest byte for byte: one layout implementation,
    // not two.
    private final Arena images = Arena.ofAuto();
    private final MemorySegment formatImage = images.allocate(NativeValues.FORMAT_BYTES, 4);
    private final MemorySegment periodsImage = images.allocate(NativeValues.DAY_PERIODS_BYTES, 4);

    // Grow-only guest buffer the input text is copied into.
    private int inPtr;
    private int inCapacity;

    // The format currently written at formatPtr, and the designators at periodsPtr — the
    // same identity memos Cast's per-thread scratch keeps, valid because both records are
    // immutable and reused in practice.
    private NumFormat formatKey;
    private DateTimeFormat periodsKey;

    WasmBackend() {
        byte[] module;
        try (InputStream in = WasmBackend.class.getResourceAsStream(RESOURCE_PATH)) {
            if (in == null) {
                throw new IllegalStateException(
                        RESOURCE_PATH + " classpath resource not found (this jar was built without the wasm module)");
            }
            module = in.readAllBytes();
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        try {
            context = Context.newBuilder("wasm")
                    .option("wasm.Builtins", "wasi_snapshot_preview1")
                    .build();
        } catch (IllegalArgumentException | IllegalStateException noWasmLanguage) {
            // The polyglot API is on the classpath but nothing behind it can run wasm: the
            // half-added dependency, org.graalvm.polyglot:polyglot without :wasm.
            throw new IllegalStateException(GRAALWASM_MISSING, noWasmLanguage);
        }
        Value instance = context.eval(Source.newBuilder("wasm", ByteSequence.create(module), "hypercast")
                        .buildLiteral())
                .newInstance();
        Value exports = instance.getMember("exports");
        memory = exports.getMember("memory");
        mallocFn = export(exports, "malloc");
        freeFn = export(exports, "free");
        versionFn = export(exports, "hypercast_version");
        for (Door door : Door.values()) {
            doors[door.ordinal()] = export(exports, door.symbol());
        }
        outPtr = malloc(16);
        faultPtr = malloc(8);
        formatPtr = malloc((int) NativeValues.FORMAT_BYTES);
        periodsPtr = malloc((int) NativeValues.DAY_PERIODS_BYTES);
    }

    private static Value export(Value exports, String name) {
        Value fn = exports.getMember(name);
        if (fn == null || !fn.canExecute()) {
            throw new IllegalStateException("hypercast: wasm module does not export " + name);
        }
        return fn;
    }

    @Override
    public String name() {
        return "wasm";
    }

    // ---- guest memory -------------------------------------------------------------------

    private int malloc(int size) {
        int ptr = mallocFn.execute(size).asInt();
        if (ptr == 0) {
            throw new IllegalStateException("hypercast: wasm guest malloc(" + size + ") returned NULL");
        }
        return ptr;
    }

    /**
     * Copies the caller's input into the guest and returns its address — 0, the ABI's NULL,
     * for empty input, which the core never dereferences.
     */
    private int stageInput(MemorySegment in, long len) {
        if (len == 0) {
            return 0;
        }
        if (len > Integer.MAX_VALUE) {
            throw new IllegalArgumentException(
                    "hypercast: an input of " + len + " bytes cannot fit the wasm32 address space");
        }
        int n = (int) len;
        if (n > inCapacity) {
            if (inPtr != 0) {
                freeFn.execute(inPtr);
                inPtr = 0;
                inCapacity = 0;
            }
            inPtr = malloc(n);
            inCapacity = n;
        }
        copyIn(in, n, inPtr);
        return inPtr;
    }

    // Copies the first n bytes of a host segment into the guest at ptr, in the order they
    // sit. The polyglot buffer interface has a bulk read but no bulk write; eight bytes at a
    // time is the widest single write it offers.
    private void copyIn(MemorySegment from, int n, int ptr) {
        int i = 0;
        for (; i + 8 <= n; i += 8) {
            memory.writeBufferLong(BIG_ENDIAN, ptr + i, from.get(BE_LONG_UNALIGNED, i));
        }
        for (; i < n; i++) {
            memory.writeBufferByte(ptr + i, from.get(ValueLayout.JAVA_BYTE, i));
        }
    }

    private int stageFormat(NumFormat format) {
        if (formatKey != format) {
            // The whole layout every time, zero past each declared symbol: the memo means a
            // shorter symbol after a longer one must not leave the tail behind. wasm memory
            // is little-endian by specification and so is every platform this jar supports,
            // so the image's three words copy in as they are.
            NativeValues.writeFormat(format, formatImage, 0);
            copyIn(formatImage, (int) NativeValues.FORMAT_BYTES, formatPtr);
            formatKey = format;
        }
        return formatPtr;
    }

    // 0, the ABI's NULL, when nothing is declared: the core then reads exactly what the
    // DateOrder overloads always read.
    private int stagePeriods(DateTimeFormat format) {
        if (format == null || format.englishOnly()) {
            return 0;
        }
        if (periodsKey != format) {
            NativeValues.writeDayPeriods(format, periodsImage, 0);
            copyIn(periodsImage, (int) NativeValues.DAY_PERIODS_BYTES, periodsPtr);
            periodsKey = format;
        }
        return periodsPtr;
    }

    /**
     * Reads the verdict back out of the guest into the caller's segments: the out-value on
     * success, the fault span on a failure code, nothing on a contract violation — exactly
     * what the native call leaves behind. wasm memory is little-endian by specification and
     * so is every platform this jar supports, so the bytes copy straight into the
     * native-order layouts {@link Cast}'s readers use.
     */
    private int finish(int code, MemorySegment out, MemorySegment fault) {
        if (code == 0) {
            memory.readBuffer(outPtr, readback, 0, 16);
            MemorySegment.copy(readback, 0, out, ValueLayout.JAVA_BYTE, 0, 16);
        } else if (code > 0) {
            memory.readBuffer(faultPtr, readback, 0, 8);
            MemorySegment.copy(readback, 0, fault, ValueLayout.JAVA_BYTE, 0, 8);
        }
        return code;
    }

    // ---- the six ABI shapes, and the version probe --------------------------------------

    @Override
    public synchronized int plain(Door door, MemorySegment in, long len, MemorySegment out, MemorySegment fault) {
        int input = stageInput(in, len);
        int code = doors[door.ordinal()]
                .execute(input, (int) len, outPtr, faultPtr)
                .asInt();
        return finish(code, out, fault);
    }

    @Override
    public synchronized int numeric(
            Door door, MemorySegment in, long len, NumFormat format, MemorySegment out, MemorySegment fault) {
        int input = stageInput(in, len);
        int code = doors[door.ordinal()]
                .execute(input, (int) len, stageFormat(format), outPtr, faultPtr)
                .asInt();
        return finish(code, out, fault);
    }

    @Override
    public synchronized int declared(
            Door door, MemorySegment in, long len, int discriminant, MemorySegment out, MemorySegment fault) {
        int input = stageInput(in, len);
        int code = doors[door.ordinal()]
                .execute(input, (int) len, discriminant, outPtr, faultPtr)
                .asInt();
        return finish(code, out, fault);
    }

    @Override
    public synchronized int civil(
            Door door,
            MemorySegment in,
            long len,
            int order,
            DateTimeFormat format,
            MemorySegment out,
            MemorySegment fault) {
        int input = stageInput(in, len);
        int code = doors[door.ordinal()]
                .execute(input, (int) len, order, stagePeriods(format), outPtr, faultPtr)
                .asInt();
        return finish(code, out, fault);
    }

    @Override
    public synchronized int typed(Door door, double value, MemorySegment out, MemorySegment fault) {
        int code = doors[door.ordinal()].execute(value, outPtr, faultPtr).asInt();
        return finish(code, out, fault);
    }

    @Override
    public synchronized int typedDeclared(
            Door door, double value, int discriminant, MemorySegment out, MemorySegment fault) {
        int code = doors[door.ordinal()]
                .execute(value, discriminant, outPtr, faultPtr)
                .asInt();
        return finish(code, out, fault);
    }

    @Override
    public synchronized int version() {
        return versionFn.execute().asInt();
    }
}
