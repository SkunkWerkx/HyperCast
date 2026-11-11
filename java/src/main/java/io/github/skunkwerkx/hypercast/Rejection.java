package io.github.skunkwerkx.hypercast;

/**
 * The failure case of {@link CheckedVerdict}: exactly one of {@link Fault}, the very record
 * every plain door returns (the input is not the type), or {@link Violation} (it was cast to the
 * type and the value, which it carries, broke a declared rule). The two are the only kinds of failure: parsing keeps its own lexicon, and a
 * rule is never a fourth {@link CastFailure}.
 *
 * @param <T> the door's value type: a {@link Violation} carries one, a {@link Fault} none
 */
public sealed interface Rejection<T> extends CheckedVerdict<T> permits Fault, Violation {}
