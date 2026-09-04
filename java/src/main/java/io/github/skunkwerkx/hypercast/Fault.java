package io.github.skunkwerkx.hypercast;

/**
 * The failure case of {@link Verdict}: a closed conversion reason plus the offending span,
 * pointing back into the caller's own input. Nothing is captured — rendering a diagnostic
 * (slicing the offending text out of the input) is the caller's choice, paid only when
 * actually rendering.
 *
 * <p>{@code offset} and {@code length} are in the input's own unit. Through a {@code byte[]}
 * or {@link java.lang.foreign.MemorySegment} door they are byte offsets into the UTF-8 the
 * core received, verbatim. Through a {@link String} door they are UTF-16 code-unit offsets
 * into that string — the core's byte span rebased, so
 * {@code text.substring(offset, offset + length)} is the offending text whatever the input
 * contained; for ASCII the two units coincide and nothing is rebased.
 *
 * <p>Generic only so it can inhabit {@code Verdict<T>} — the type parameter is phantom;
 * a fault carries no value.
 *
 * @param <T> the verdict's value type (phantom)
 * @param reason the closed-set conversion reason, never {@code null}
 * @param offset offset of the offending span in the input: bytes for byte input, chars for a {@link String}
 * @param length length of the offending span in the same unit; zero for {@link CastFailure#EMPTY}
 */
public record Fault<T>(CastFailure reason, int offset, int length) implements Verdict<T> {
    /** Rejects a {@code null} reason up front — a fault with no reason is a caller bug. */
    public Fault {
        if (reason == null) {
            throw new IllegalArgumentException("Fault must carry a real reason");
        }
    }
}
