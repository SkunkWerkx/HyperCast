package io.github.skunkwerkx.hypercast;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import org.graalvm.polyglot.Context;
import org.graalvm.polyglot.Source;
import org.graalvm.polyglot.Value;
import org.graalvm.polyglot.io.ByteSequence;

/**
 * The Rust core as a {@code wasm32-wasip1} module, run inside the JVM by
 * <a href="https://www.graalvm.org/webassembly/">GraalWasm</a>. No native binary, no
 * {@code java.lang.foreign} downcall: the same twenty-one {@code cast_*} exports (and the
 * {@code hypercast_version} probe) {@link Cast} downcalls into natively are called through
 * the polyglot API instead, on the module bundled at
 * {@code /native/wasm32-wasip1/hypercast.wasm}.
 *
 * <p><b>Memory protocol.</b> A wasm guest only sees its own linear memory, so nothing here
 * can hand the core a pointer into a Java array the way the FFM path pins a {@code byte[]}.
 * The input text is copied into a grow-only guest buffer, and the three out-params the doors
 * fill — the 16-byte out-value, the 8-byte fault span, the 32-byte {@link NumFormat} — are
 * guest allocations made once at load. All of it comes from the module's own exported
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

    private static final ByteOrder BIG_ENDIAN = ByteOrder.BIG_ENDIAN;
    private static final ByteOrder LITTLE_ENDIAN = ByteOrder.LITTLE_ENDIAN;
    // Eight input bytes at a time, in the order they sit in the caller's segment: read
    // big-endian, written big-endian, so the guest sees the same byte sequence.
    private static final ValueLayout.OfLong BE_LONG_UNALIGNED =
            ValueLayout.JAVA_LONG_UNALIGNED.withOrder(BIG_ENDIAN);

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
    // fault span, and the NumFormat the numeric doors read.
    private final int outPtr;
    private final int faultPtr;
    private final int formatPtr;

    // Grow-only guest buffer the input text is copied into.
    private int inPtr;
    private int inCapacity;

    // The format currently written at formatPtr — the same identity memo Cast's per-thread
    // scratch keeps, valid because NumFormat is immutable and reused in practice.
    private NumFormat formatKey;

    WasmBackend() {
        byte[] module;
        try (InputStream in = WasmBackend.class.getResourceAsStream(RESOURCE_PATH)) {
            if (in == null) {
                throw new IllegalStateException(RESOURCE_PATH
                        + " classpath resource not found (this jar was built without the wasm module)");
            }
            module = in.readAllBytes();
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        context = Context.newBuilder("wasm")
                .option("wasm.Builtins", "wasi_snapshot_preview1")
                .build();
        Value instance = context.eval(Source.newBuilder("wasm", ByteSequence.create(module), "hypercast").buildLiteral())
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
        formatPtr = malloc(32);
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
        // The polyglot buffer interface has a bulk read but no bulk write; eight bytes at a
        // time is the widest single write it offers.
        int i = 0;
        for (; i + 8 <= n; i += 8) {
            memory.writeBufferLong(BIG_ENDIAN, inPtr + i, in.get(BE_LONG_UNALIGNED, i));
        }
        for (; i < n; i++) {
            memory.writeBufferByte(inPtr + i, in.get(ValueLayout.JAVA_BYTE, i));
        }
        return inPtr;
    }

    private int stageFormat(NumFormat format) {
        if (formatKey != format) {
            memory.writeBufferInt(LITTLE_ENDIAN, formatPtr, format.decimalSeparator());
            memory.writeBufferInt(LITTLE_ENDIAN, formatPtr + 4, format.groupSeparator());
            memory.writeBufferInt(LITTLE_ENDIAN, formatPtr + 8, format.styles());
            byte[] symbol = format.currencySymbol().getBytes(StandardCharsets.UTF_8);
            memory.writeBufferInt(LITTLE_ENDIAN, formatPtr + 12, symbol.length);
            // All sixteen symbol bytes every time, zero past the declared length: the memo
            // means a shorter symbol after a longer one must not leave the tail behind.
            for (int i = 0; i < 16; i++) {
                memory.writeBufferByte(formatPtr + 16 + i, i < symbol.length ? symbol[i] : (byte) 0);
            }
            formatKey = format;
        }
        return formatPtr;
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

    // ---- the three ABI shapes, and the version probe ------------------------------------

    @Override
    public synchronized int plain(Door door, MemorySegment in, long len, MemorySegment out, MemorySegment fault) {
        int input = stageInput(in, len);
        int code = doors[door.ordinal()].execute(input, (int) len, outPtr, faultPtr).asInt();
        return finish(code, out, fault);
    }

    @Override
    public synchronized int numeric(
            Door door, MemorySegment in, long len, NumFormat format, MemorySegment out, MemorySegment fault) {
        int input = stageInput(in, len);
        int code = doors[door.ordinal()].execute(input, (int) len, stageFormat(format), outPtr, faultPtr).asInt();
        return finish(code, out, fault);
    }

    @Override
    public synchronized int declared(
            Door door, MemorySegment in, long len, int discriminant, MemorySegment out, MemorySegment fault) {
        int input = stageInput(in, len);
        int code = doors[door.ordinal()].execute(input, (int) len, discriminant, outPtr, faultPtr).asInt();
        return finish(code, out, fault);
    }

    @Override
    public synchronized int version() {
        return versionFn.execute().asInt();
    }
}
