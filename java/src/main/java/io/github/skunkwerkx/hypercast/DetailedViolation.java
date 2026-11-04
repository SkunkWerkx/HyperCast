package io.github.skunkwerkx.hypercast;

/**
 * The other failure case of {@link DetailedRejection}: the value parsed and broke a declared
 * rule, as {@link Violation} says. The {@link Keyword} says which rule; a violation has no
 * {@link FaultDetail}, because nothing about the text was wrong.
 *
 * @param <T> the door's value type (phantom)
 * @param keyword the rule broken, never {@code null}
 * @param offset offset of the value in the input: bytes for byte input, chars for a {@link String}
 * @param length length of the value in the same unit
 */
public record DetailedViolation<T>(Keyword keyword, int offset, int length) implements DetailedRejection<T> {
    /** Refuses a {@code null} keyword up front: a violation that names no rule is a caller bug. */
    public DetailedViolation {
        if (keyword == null) {
            throw new IllegalArgumentException("DetailedViolation must name a keyword");
        }
    }

    /**
     * The same violation as {@link Cast}'s checked doors return it.
     *
     * @return the {@link Violation}
     */
    public Violation<T> toViolation() {
        return new Violation<>(keyword, offset, length);
    }
}
