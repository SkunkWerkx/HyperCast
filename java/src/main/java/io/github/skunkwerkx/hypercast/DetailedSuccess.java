package io.github.skunkwerkx.hypercast;

/**
 * The success case of {@link DetailedVerdict}: a cast value, as {@link Success} carries it.
 *
 * @param <T> the cast value's type
 * @param value the cast value, never null
 */
public record DetailedSuccess<T>(T value) implements DetailedVerdict<T> {}
