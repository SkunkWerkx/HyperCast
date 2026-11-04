package io.github.skunkwerkx.hypercast;

/**
 * The outcome of a cast through the detailed path ({@link DetailedCast}): exactly one of
 * {@link DetailedSuccess} or {@link DetailedFault}, the failure carrying the core's
 * {@link FaultDetail} beside its reason and span. A sealed interface beside {@link Verdict},
 * which {@link Cast} keeps returning, unchanged: {@code Verdict} is sealed over {@link Success}
 * and {@link Fault}, so a third case, or a detail on {@code Fault}, would break the exhaustive
 * switches written against it. The detailed path is the same parse; it answers with the same
 * value, reason and span, plus the detail.
 *
 * {@snippet :
 * String rendered = switch (DetailedCast.i32("12,34", NumFormat.INVARIANT)) {
 *     case DetailedSuccess<Integer> s -> "got " + s.value();
 *     case DetailedFault<Integer> f -> f.detail() + " at char " + f.offset();
 * };
 * }
 *
 * @param <T> the cast value's type
 */
public sealed interface DetailedVerdict<T> permits DetailedSuccess, DetailedFault {}
