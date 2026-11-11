package io.github.skunkwerkx.hypercast;

/**
 * The other failure case of {@link DetailedRejection}: the input was cast to the door's type and
 * the value broke a declared rule, as {@link Violation} says, carrying the value as cast. The
 * {@link Keyword} says which rule; a violation has no {@link FaultDetail}, because nothing about
 * the text was wrong.
 *
 * @param <T> the door's value type: a number, or {@link String} at the text door
 * @param keyword the rule broken, never {@code null}
 * @param value the value as cast (never rounded; at the text door the judged text), never
 *     {@code null}
 * @param offset offset of the value in the input: bytes for byte input, chars for a {@link String}
 * @param length length of the value in the same unit
 */
public record DetailedViolation<T>(Keyword keyword, T value, int offset, int length) implements DetailedRejection<T> {
    /** Refuses a {@code null} keyword or value up front: a violation names its rule and its value. */
    public DetailedViolation {
        if (keyword == null || value == null) {
            throw new IllegalArgumentException("DetailedViolation must carry a keyword and a value");
        }
    }

    /**
     * The same violation as {@link Cast}'s checked doors return it.
     *
     * @return the {@link Violation}
     */
    public Violation<T> toViolation() {
        return new Violation<>(keyword, value, offset, length);
    }
}
