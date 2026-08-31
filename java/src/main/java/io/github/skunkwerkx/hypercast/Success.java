package io.github.skunkwerkx.hypercast;

/**
 * The success case of {@link Verdict}: a cast value.
 *
 * @param <T> the cast value's type
 * @param value the cast value, never null
 */
public record Success<T>(T value) implements Verdict<T> {}
