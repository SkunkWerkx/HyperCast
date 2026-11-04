package io.github.skunkwerkx.hypercast;

/**
 * The outcome of a checked cast through the detailed path ({@link DetailedCast}'s doors with
 * declared {@link Constraints}): exactly one of {@link DetailedChecked} (the value obeys every
 * rule) or a {@link DetailedRejection}, itself exactly one of {@link DetailedFault} (it did
 * not parse, with its {@link FaultDetail}) or {@link DetailedViolation} (it parsed and broke a
 * declared rule). A sealed interface beside {@link CheckedVerdict}, which the checked doors of
 * {@link Cast} keep returning, unchanged.
 *
 * <p>Both levels are sealed, so either switch is exhaustive with no {@code default} arm.
 *
 * @param <T> the door's value type: a number, or {@link String} at the text door
 */
public sealed interface DetailedCheckedVerdict<T> permits DetailedChecked, DetailedRejection {}
