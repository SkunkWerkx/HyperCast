package io.github.skunkwerkx.hypercast;

import java.lang.foreign.MemorySegment;

/**
 * The one seam between {@link Cast}'s public surface and a non-FFM way of reaching the Rust
 * core. The FFM downcalls are not behind this: they stay inlined in {@code Cast} exactly as
 * before, and every door there only checks one {@code static final} reference against
 * {@code null} before taking its native path. This exists so the wasm implementation can be
 * a separate class ({@link WasmBackend}) that is never loaded, and whose GraalWasm
 * dependency is never touched, unless it was actually selected — the reference {@code Cast}
 * holds is typed as this interface, and the implementation is instantiated by name.
 *
 * <p>Three of the methods are the three ABI shapes in {@code ffi.rs}, one level below the
 * verdict: each takes the caller's input bytes and the caller's own out-value and fault-span
 * segments (the per-thread scratch {@code Cast} already keeps), performs the crossing, fills
 * the segments exactly as the native call would have, and returns the verdict code. Folding
 * the code and the bytes into a {@link Verdict} stays in {@code Cast}, so the readers, the
 * exceptions and the messages are one implementation for both paths — which is what lets the
 * whole test suite run unchanged against either. The fourth is the core's version probe,
 * which takes nothing and cannot fail.
 */
interface Backend {
    /** A short, stable name for diagnostics and tests: {@code "wasm"}. */
    String name();

    /** The {@code (ptr, len, out, fault)} shape: the culture-insensitive doors. */
    int plain(Door door, MemorySegment in, long len, MemorySegment out, MemorySegment fault);

    /** The {@code (ptr, len, format, out, fault)} shape: the integer, real and decimal doors. */
    int numeric(Door door, MemorySegment in, long len, NumFormat format, MemorySegment out, MemorySegment fault);

    /**
     * The {@code (ptr, len, u32, out, fault)} shape: the doors that take a caller-declared
     * precision, epoch, or field order.
     */
    int declared(Door door, MemorySegment in, long len, int discriminant, MemorySegment out, MemorySegment fault);

    /** The {@code hypercast_version} export: the core's version, packed {@code major << 16 | minor << 8 | patch}. */
    int version();
}
