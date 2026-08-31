package io.github.skunkwerkx.hypercast;

import java.time.chrono.IsoChronology;
import java.time.format.DateTimeFormatterBuilder;
import java.time.format.FormatStyle;
import java.util.Locale;

/**
 * The caller-declared field order of a separated calendar date. There is no guessing —
 * {@code 1/7/2026} is January 7th or July 1st only because the caller said which (en-US
 * short dates are month-first, en-GB and most of the world day-first, ISO year-first), the
 * same declare-don't-sniff stance {@link NumFormat} takes for numeric notation and
 * {@link UnixPrecision} takes for epoch magnitude. Values match the native core's
 * discriminants.
 */
public enum DateOrder {
    /** Year, month, day — ISO's order with any accepted separator. */
    YEAR_MONTH_DAY(1),
    /** Month, day, year — the en-US short-date order ({@code 1/7/2026} is January 7th). */
    MONTH_DAY_YEAR(2),
    /** Day, month, year — the en-GB/most-of-the-world order ({@code 1/7/2026} is July 1st). */
    DAY_MONTH_YEAR(3);

    private final int code;

    DateOrder(int code) {
        this.code = code;
    }

    int code() {
        return code;
    }

    /**
     * Derives the field order from a locale's own short-date pattern: the first of
     * {@code y}/{@code M}/{@code d} to appear decides (quoted literals skipped). Prefer
     * declaring explicitly when the text's origin is known — a locale is process/user
     * state; the text's dialect is a property of the text.
     *
     * @param locale the locale whose short-date pattern declares the order
     * @return the locale's field order
     * @throws IllegalArgumentException if the pattern names none of y/M/d — a malformed
     *     locale, not a data verdict
     */
    public static DateOrder from(Locale locale) {
        String pattern = DateTimeFormatterBuilder.getLocalizedDateTimePattern(
                FormatStyle.SHORT, null, IsoChronology.INSTANCE, locale);
        boolean inLiteral = false;
        for (int i = 0; i < pattern.length(); i++) {
            char ch = pattern.charAt(i);
            if (ch == '\'') {
                inLiteral = !inLiteral;
                continue;
            }
            if (inLiteral) {
                continue;
            }
            switch (ch) {
                case 'y', 'u' -> {
                    return YEAR_MONTH_DAY;
                }
                case 'M', 'L' -> {
                    return MONTH_DAY_YEAR;
                }
                case 'd' -> {
                    return DAY_MONTH_YEAR;
                }
                default -> { }
            }
        }
        throw new IllegalArgumentException(
                "Short date pattern '" + pattern + "' names none of y/M/d.");
    }
}
