package io.github.skunkwerkx.hypercast;

import java.util.Locale;

/**
 * Opt-in date and time shapes for {@link DetailedCast}'s date doors. Every door keeps today's
 * shape rules unless it is handed options: a caller that relies on strict RFC 3339 refusing a
 * space separator or {@code +00} is unaffected. This deliberately differs from white-space
 * tolerance, which was a widening every door took: not every uniquely interpretable spelling
 * becomes accepted.
 *
 * <table class="striped">
 * <caption>The options</caption>
 * <tr><th>Option<th>Door<th>Accepts
 * <tr><td>{@link #COMPACT_DATE}<td>{@code date}<td>{@code 20260107}: eight ASCII digits
 * <tr><td>{@link #SPACE_SEPARATOR}<td>{@code timestamp}<td>a single space in place of {@code T}
 * <tr><td>{@link #HOUR_OFFSET}<td>{@code timestamp}<td>{@code ±hh} offsets ({@code +00}, {@code -05})
 * <tr><td>the {@link #pivot()}<td>{@code date} with an order, {@code dateTime}<td>two-digit years
 * <tr><td>{@link MonthNames}<td>{@code date} with an order, {@code dateTime}<td>declared month names
 * </table>
 *
 * <p>{@link #COMMON_FORMS} turns on the three flags together, never a pivot or a vocabulary,
 * and works unchanged at every door: an option that means nothing to a door is ignored there,
 * while a flag bit this binding does not define is refused.
 *
 * <p>The pivot is the last year of a 100-year window: a two-digit {@code yy} reads as the one
 * year in {@code [pivot−99, pivot]} that ends in {@code yy}. It is {@code 100} to {@code 9999}
 * (so the window stays within years 1 to 9999), or {@code 0} for none, the default; four-digit
 * years are unaffected. There is no default pivot, so a two-digit year stays an error until
 * the caller chooses one.
 *
 * @param flags bitwise OR of {@link #COMPACT_DATE}, {@link #SPACE_SEPARATOR} and {@link #HOUR_OFFSET}
 * @param pivot the last year of the two-digit-year window, 100 to 9999, or 0 for none
 */
public record DateOptions(int flags, int pivot) {
    /** {@code yyyyMMdd}: eight ASCII digits as a date. Read by the {@code date} door. */
    public static final int COMPACT_DATE = 1;

    /** A single space in place of {@code T} between the date and the time. Read by the {@code timestamp} door. */
    public static final int SPACE_SEPARATOR = 1 << 1;

    /** {@code ±hh} offsets, {@code +00} and {@code -05}, beside {@code ±hh:mm}. Read by the {@code timestamp} door. */
    public static final int HOUR_OFFSET = 1 << 2;

    private static final int DEFINED = COMPACT_DATE | SPACE_SEPARATOR | HOUR_OFFSET;

    /** No options: today's shape rules, no pivot. What a door reads when handed nothing. */
    public static final DateOptions NONE = new DateOptions(0, 0);

    /**
     * {@link #COMPACT_DATE}, {@link #SPACE_SEPARATOR} and {@link #HOUR_OFFSET} together, with
     * no pivot: the common forms feeds send, safe to hand to any date door.
     */
    public static final DateOptions COMMON_FORMS = new DateOptions(DEFINED, 0);

    /**
     * Validates by the core's own rules, so a declaration the core would refuse fails loudly as
     * the caller bug it is, never as a verdict: only the defined flag bits, then a pivot that is
     * 0 or 100 to 9999.
     *
     * @throws IllegalArgumentException for an undefined flag bit or a pivot out of range
     */
    public DateOptions {
        if ((flags & ~DEFINED) != 0) {
            throw new IllegalArgumentException("Date option flags must be COMPACT_DATE, SPACE_SEPARATOR and "
                    + "HOUR_OFFSET only; 0x" + Integer.toHexString(flags).toUpperCase(Locale.ROOT)
                    + " has undefined bits");
        }
        if (pivot != 0 && (pivot < 100 || pivot > 9999)) {
            throw new IllegalArgumentException("Pivot must be 0 (none) or 100 to 9999; got " + pivot);
        }
    }

    /**
     * Declares the flags and no pivot.
     *
     * @param flags bitwise OR of the option flags
     */
    public DateOptions(int flags) {
        this(flags, 0);
    }

    /**
     * The same options with a two-digit-year pivot.
     *
     * @param pivot the last year of the 100-year window, 100 to 9999, or 0 for none
     * @return the options with {@code pivot} in place of this one's
     */
    public DateOptions withPivot(int pivot) {
        return new DateOptions(flags, pivot);
    }

    /**
     * The same options under other flags.
     *
     * @param flags bitwise OR of the option flags
     * @return the options with {@code flags} in place of this one's
     */
    public DateOptions withFlags(int flags) {
        return new DateOptions(flags, pivot);
    }

    /** True when nothing is declared: what crosses to the core as a null pointer. */
    boolean isNone() {
        return flags == 0 && pivot == 0;
    }
}
