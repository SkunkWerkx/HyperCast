package io.github.skunkwerkx.hypercast;

/**
 * The failure case of {@link DetailedCheckedVerdict}: exactly one of {@link DetailedFault}, the
 * very record every plain detailed door returns (the input did not parse), or
 * {@link DetailedViolation} (it parsed and broke a declared rule, which has no detail beyond
 * its {@link Keyword}).
 *
 * @param <T> the door's value type (phantom: a rejection carries no value)
 */
public sealed interface DetailedRejection<T> extends DetailedCheckedVerdict<T>
        permits DetailedFault, DetailedViolation {}
