package io.github.skunkwerkx.hypercast;

import java.util.OptionalInt;

/**
 * The success case of {@link CheckedVerdict}: a value that obeys every declared rule.
 *
 * @param <T> the door's value type: a number, or {@link String} at the text door
 * @param value the value, rounded to its {@code multipleOf} when a {@link Rounding} said so;
 *     at the text door, the text (trimmed when declared), and with an {@code enum} the
 *     caller's own declared entry, the same instance, whatever case the input arrived in
 * @param adjusted {@code true} only when rounding changed the value; trimming and case never
 *     set it
 * @param enumIndex at the text door with an {@code enum}, the zero-based index of the entry
 *     matched; empty otherwise, an integer {@code enum} included (the value is the answer)
 */
public record Checked<T>(T value, boolean adjusted, OptionalInt enumIndex) implements CheckedVerdict<T> {
    /** Refuses a {@code null} value or index: a success always carries both. */
    public Checked {
        if (value == null || enumIndex == null) {
            throw new IllegalArgumentException("Checked must carry a value and an enumIndex (empty for none)");
        }
    }
}
