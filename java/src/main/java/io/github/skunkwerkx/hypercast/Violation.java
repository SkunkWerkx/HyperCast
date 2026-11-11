package io.github.skunkwerkx.hypercast;

/**
 * The other failure case of {@link Rejection}: the input was cast to the door's type and the
 * value broke a declared rule. The value is carried exactly as the door cast it, never as a
 * {@link Rounding} would have moved it (rounding is part of the judgement, not of the value):
 * at the text door, the judged text (after {@code trim}), the input itself rather than an
 * {@code enum} entry. The keyword says which rule, and the span covers the value in the same
 * units as a {@link Fault}'s: bytes through a {@code byte[]} or
 * {@link java.lang.foreign.MemorySegment} door, UTF-16 code units through a {@link String} door,
 * and an empty span from a door that read a {@code double}.
 *
 * @param <T> the door's value type: a number, or {@link String} at the text door
 * @param keyword the rule broken, never {@code null}
 * @param value the value as cast, never {@code null}
 * @param offset offset of the value in the input: bytes for byte input, chars for a {@link String}
 * @param length length of the value in the same unit
 */
public record Violation<T>(Keyword keyword, T value, int offset, int length) implements Rejection<T> {
    /** Refuses a {@code null} keyword or value up front: a violation names its rule and its value. */
    public Violation {
        if (keyword == null || value == null) {
            throw new IllegalArgumentException("Violation must carry a keyword and a value");
        }
    }
}
