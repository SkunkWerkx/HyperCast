package io.github.skunkwerkx.hypercast;

import java.util.OptionalInt;

/**
 * The success case of {@link DetailedCheckedVerdict}: a value that obeys every declared rule,
 * as {@link Checked} carries it.
 *
 * @param <T> the door's value type: a number, or {@link String} at the text door
 * @param value the value, rounded to its {@code multipleOf} when a {@link Rounding} said so; at
 *     the text door, the text, or with an {@code enum} the caller's own declared entry
 * @param adjusted {@code true} only when rounding changed the value
 * @param enumIndex at the text door with an {@code enum}, the zero-based index of the entry
 *     matched; empty otherwise
 */
public record DetailedChecked<T>(T value, boolean adjusted, OptionalInt enumIndex)
        implements DetailedCheckedVerdict<T> {
    /** Refuses a {@code null} value or index: a success always carries both. */
    public DetailedChecked {
        if (value == null || enumIndex == null) {
            throw new IllegalArgumentException("DetailedChecked must carry a value and an enumIndex (empty for none)");
        }
    }
}
