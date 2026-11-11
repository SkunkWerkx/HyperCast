package io.github.skunkwerkx.hypercast;

/**
 * The failure case of {@link DetailedCheckedVerdict}: exactly one of {@link DetailedFault}, the
 * very record every plain detailed door returns (the input is not the type), or
 * {@link DetailedViolation} (it was cast to the type and the value, which it carries, broke a
 * declared rule: no detail beyond its {@link Keyword}).
 *
 * @param <T> the door's value type: a {@link DetailedViolation} carries one, a
 *     {@link DetailedFault} none
 */
public sealed interface DetailedRejection<T> extends DetailedCheckedVerdict<T>
        permits DetailedFault, DetailedViolation {}
