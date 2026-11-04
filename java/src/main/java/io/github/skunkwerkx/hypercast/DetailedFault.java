package io.github.skunkwerkx.hypercast;

/**
 * The failure case of {@link DetailedVerdict}: the {@link Fault}'s reason and offending span,
 * and what failed ({@link FaultDetail}). Nothing is captured; the span points back into the
 * caller's input, in the units {@link Fault} documents (bytes for a {@code byte[]} or
 * {@link java.lang.foreign.MemorySegment} door, UTF-16 code units for a {@link String} door).
 *
 * <p>The same record is a checked door's parse failure: it is also a
 * {@link DetailedRejection}, so a checked door of the detailed path hands back exactly the
 * fault its plain door would.
 *
 * <p>A detail this binding does not name is not an error: {@link #detail()} keeps the code of
 * a later core's addition ({@link FaultDetail#isKnown()} is false), and the reason and span
 * stay usable. Equality is over every component, the raw code included.
 *
 * @param <T> the verdict's value type (phantom)
 * @param reason the closed-set conversion reason, never {@code null}
 * @param detail what failed, never {@code null}
 * @param offset offset of the offending span in the input: bytes for byte input, chars for a {@link String}
 * @param length length of the offending span in the same unit; zero for {@link CastFailure#EMPTY}
 */
public record DetailedFault<T>(CastFailure reason, FaultDetail detail, int offset, int length)
        implements DetailedVerdict<T>, DetailedRejection<T> {
    /** Rejects a {@code null} reason or detail up front: a fault with neither is a caller bug. */
    public DetailedFault {
        if (reason == null || detail == null) {
            throw new IllegalArgumentException("DetailedFault must carry a real reason and detail");
        }
    }

    /**
     * The same failure without its detail, as the plain door's {@link Fault}.
     *
     * @return the fault the plain door of {@link Cast} returns for this input
     */
    public Fault<T> toFault() {
        return new Fault<>(reason, offset, length);
    }
}
