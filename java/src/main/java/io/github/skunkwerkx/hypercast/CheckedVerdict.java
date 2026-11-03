package io.github.skunkwerkx.hypercast;

/**
 * The outcome of a checked cast, a door with declared {@link Constraints}: exactly one of
 * {@link Checked} (the value obeys every rule) or a {@link Rejection}, itself exactly one of
 * the plain doors' own {@link Fault} (it did not parse) or a {@link Violation} (it parsed and
 * broke a declared rule). A sealed interface beside {@link Verdict}, which every plain door
 * keeps returning, unchanged. Nothing is ever thrown for data.
 *
 * <p>Both levels are sealed, so either switch is exhaustive with no {@code default} arm:
 * over {@code Checked} and {@code Rejection}, or straight over the three records.
 *
 * {@snippet :
 * Constraints<Integer> percent = Constraints.i32().minimum("0").maximum("100").build();
 * String rendered = switch (Cast.i32(" 101 ", NumFormat.INVARIANT, percent)) {
 *     case Checked<Integer> ok -> "got " + ok.value();
 *     case Fault<Integer> f -> f.reason() + " at char " + f.offset();
 *     case Violation<Integer> v -> "breaks " + v.keyword() + " at char " + v.offset();
 * };
 * }
 *
 * @param <T> the door's value type: a number, or {@link String} at the text door
 */
public sealed interface CheckedVerdict<T> permits Checked, Rejection {}
