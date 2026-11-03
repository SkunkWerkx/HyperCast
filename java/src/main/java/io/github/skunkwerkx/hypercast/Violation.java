package io.github.skunkwerkx.hypercast;

/**
 * The other failure case of {@link Rejection}: the value parsed and broke a declared rule. The
 * keyword says which, and the span covers the value (after trimming, for text) in the same
 * units as a {@link Fault}'s: bytes through a {@code byte[]} or
 * {@link java.lang.foreign.MemorySegment} door, UTF-16 code units through a {@link String} door,
 * and an empty span from a door that read a {@code double}.
 *
 * <p>Generic only so it can inhabit {@code Rejection<T>}; the type parameter is phantom.
 *
 * @param <T> the door's value type (phantom)
 * @param keyword the rule broken, never {@code null}
 * @param offset offset of the value in the input: bytes for byte input, chars for a {@link String}
 * @param length length of the value in the same unit
 */
public record Violation<T>(Keyword keyword, int offset, int length) implements Rejection<T> {
    /** Refuses a {@code null} keyword up front: a violation that names no rule is a caller bug. */
    public Violation {
        if (keyword == null) {
            throw new IllegalArgumentException("Violation must name a keyword");
        }
    }
}
