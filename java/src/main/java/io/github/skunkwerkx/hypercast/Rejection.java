package io.github.skunkwerkx.hypercast;

/**
 * The failure case of {@link CheckedVerdict}: exactly one of {@link Fault}, the very record
 * every plain door returns (the input did not parse), or {@link Violation} (it parsed and broke
 * a declared rule). The two are the only kinds of failure: parsing keeps its own lexicon, and a
 * rule is never a fourth {@link CastFailure}.
 *
 * @param <T> the door's value type (phantom: a rejection carries no value)
 */
public sealed interface Rejection<T> extends CheckedVerdict<T> permits Fault, Violation {}
