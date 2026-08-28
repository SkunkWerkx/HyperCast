package io.github.buvinghausen.hypercast;

/**
 * The failure case of {@link Verdict}: a closed conversion reason plus the offending span,
 * pointing back into the caller's own input. Nothing is captured — rendering a diagnostic
 * (slicing the offending text out of the input) is the caller's choice, paid only when
 * actually rendering.
 *
 * <p>{@code offset} and {@code length} are byte offsets into the UTF-8 representation of
 * the input — exactly what the {@code byte[]} doors received, and identical to character
 * offsets whenever the input is ASCII (which scalar text almost always is). For non-ASCII
 * input passed through a {@link String} door, map accordingly before slicing.
 *
 * <p>Generic only so it can inhabit {@code Verdict<T>} — the type parameter is phantom;
 * a fault carries no value.
 *
 * @param <T> the verdict's value type (phantom)
 * @param reason the closed-set conversion reason, never {@code null}
 * @param offset byte offset of the offending span in the input's UTF-8 form
 * @param length byte length of the offending span; zero for {@link CastFailure#EMPTY}
 */
public record Fault<T>(CastFailure reason, int offset, int length) implements Verdict<T> {
    public Fault {
        if (reason == null) {
            throw new IllegalArgumentException("Fault must carry a real reason");
        }
    }
}
