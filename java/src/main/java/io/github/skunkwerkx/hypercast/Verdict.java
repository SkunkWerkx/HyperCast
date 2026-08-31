package io.github.skunkwerkx.hypercast;

/**
 * The outcome of a cast: exactly one of {@link Success} or {@link Fault}, as Java's native
 * discriminated union — a sealed interface over two records. The failure reason travels in
 * the type, never as an exception.
 *
 * <p>Consume with an exhaustive switch over the case records — sealed means the compiler
 * proves both dispositions are handled and rejects a missing one, no {@code default} arm:
 *
 * {@snippet :
 * String rendered = switch (Cast.i32("(1,234)", NumFormat.INVARIANT)) {
 *     case Success<Integer> s -> "got " + s.value();
 *     case Fault<Integer> f -> f.reason() + " at byte " + f.offset();
 * };
 * }
 *
 * <p>The exact counterpart of the C# binding's {@code Verdict<T>} ({@code [Union]}/
 * {@code IUnion} on .NET 11) — each binding uses its platform's own union idiom over the
 * same native verdict codes.
 *
 * @param <T> the cast value's type
 */
public sealed interface Verdict<T> permits Success, Fault {}
