package io.github.skunkwerkx.hypercast;

import java.nio.charset.StandardCharsets;
import java.text.DateFormatSymbols;
import java.util.Locale;
import java.util.Objects;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Caller-declared notation for the civil date-time door ({@link Cast#dateTime(String,
 * DateTimeFormat)}): the {@link DateOrder} of the date part, and the day-period designators
 * a culture writes for the two halves of a 12-hour clock — {@code 오전}/{@code 오후},
 * {@code a. m.}/{@code p. m.}, {@code 午前}/{@code 午後}. The native core carries no culture
 * data; this is the parallel of {@link NumFormat} for the one temporal door that reads
 * culture-sensitive text, and {@link #from(Locale)} is its bridge from the JDK's own
 * {@link DateFormatSymbols}.
 *
 * <p>A declared designator is matched exactly, after the clock ({@code 3:04 nachm.}, with an
 * optional space before it) or ahead of it ({@code 오후 3:04}, an optional space after), and
 * is read <em>beside</em> the English {@code AM}/{@code PM}, which the core always reads,
 * ASCII case-insensitively, in both positions. The {@link DateOrder} overloads of the door
 * declare none, which is exactly what they always read.
 *
 * @param order the declared field order of the date part
 * @param amDesignator the designator of the first half of the day: 1 to 16 UTF-8 bytes, no
 *     ASCII digit, no ASCII whitespace at either end (a space inside, as in {@code a. m.}, is
 *     fine); {@code ""} declares none beside {@code AM}
 * @param pmDesignator the designator of the second half, likewise; {@code ""} declares none
 *     beside {@code PM}
 */
public record DateTimeFormat(DateOrder order, String amDesignator, String pmDesignator) {
    // The inline capacity the core's RawDayPeriods holds each designator in, in UTF-8 bytes.
    private static final int DESIGNATOR_MAX_BYTES = 16;

    /**
     * Validates the designators up front, by the core's own rules — whole code points, at
     * most 16 UTF-8 bytes, no ASCII digit, no ASCII whitespace at either end, which the
     * grammar around a designator reads itself — so a malformed one fails loudly as the
     * caller bug it is, never as a verdict.
     *
     * @throws IllegalArgumentException for a designator the core would refuse
     * @throws NullPointerException if the order or a designator is null
     */
    public DateTimeFormat {
        Objects.requireNonNull(order, "order");
        String refused = refusal("AM designator", amDesignator);
        if (refused == null) {
            refused = refusal("PM designator", pmDesignator);
        }
        if (refused != null) {
            throw new IllegalArgumentException(refused);
        }
    }

    /**
     * Declares the order and no designators: the English {@code AM}/{@code PM} only, which is
     * what the door's {@link DateOrder} overloads read.
     *
     * @param order the declared field order of the date part
     */
    public DateTimeFormat(DateOrder order) {
        this(order, "", "");
    }

    /** True when no designator is declared — what crosses to the core as a null pointer. */
    boolean englishOnly() {
        return amDesignator.isEmpty() && pmDesignator.isEmpty();
    }

    /**
     * Derives the notation from a locale's own data: the field order of its short-date
     * pattern, as {@link DateOrder#from(Locale)} reads it, and the designators of its
     * {@link DateFormatSymbols#getAmPmStrings()}. A designator the core reads anyway — the
     * English pair, in any case — is left undeclared, since declaring it would change
     * nothing; so is one the core's layout cannot hold (over 16 UTF-8 bytes, which the
     * JDK's data has for a hundred-odd locales: Nepali's {@code पूर्वाह्न} is 27), rather
     * than refusing the locale — the text then reads as it always did, under the English
     * pair alone. Prefer declaring explicitly when the text's origin is known: a locale is
     * process/user state; the text's dialect is a property of the text.
     *
     * @param locale the locale to derive from, never null
     * @return the locale's field order and designators
     * @throws NullPointerException if {@code locale} is null
     * @throws IllegalArgumentException if the locale's short-date pattern names none of
     *     y/M/d — a malformed locale, not a data verdict
     */
    public static DateTimeFormat from(Locale locale) {
        Objects.requireNonNull(locale, "locale");
        DateTimeFormat cached = BY_LOCALE.get(locale);
        if (cached != null) {
            return cached;
        }
        String[] periods = DateFormatSymbols.getInstance(locale).getAmPmStrings();
        DateTimeFormat derived =
                new DateTimeFormat(DateOrder.from(locale), declared(periods[0], "AM"), declared(periods[1], "PM"));
        // Bounded by clearing rather than evicting, as NumFormat's cache is.
        if (BY_LOCALE.size() >= LOCALE_CACHE_LIMIT) {
            BY_LOCALE.clear();
        }
        BY_LOCALE.put(locale, derived);
        return derived;
    }

    private static String declared(String designator, String english) {
        return designator.equalsIgnoreCase(english) || refusal("", designator) != null ? "" : designator;
    }

    // Why the core would refuse a designator, or null when it would not: whole code points,
    // within the field's capacity, no ASCII digit (a digit is a digit before it is anything
    // else), and no ASCII whitespace (space, \t, \n, \f, \r) at either end. Empty passes —
    // nothing declared.
    private static String refusal(String what, String designator) {
        Objects.requireNonNull(designator, what);
        if (designator.isEmpty()) {
            return null;
        }
        if (!StandardCharsets.UTF_8.newEncoder().canEncode(designator)) {
            return what + " must be whole code points, not surrogate halves";
        }
        int bytes = designator.getBytes(StandardCharsets.UTF_8).length;
        if (bytes > DESIGNATOR_MAX_BYTES) {
            return what + " must be at most " + DESIGNATOR_MAX_BYTES + " UTF-8 bytes; '" + designator + "' is " + bytes;
        }
        for (int i = 0; i < designator.length(); i++) {
            char c = designator.charAt(i);
            if (c >= '0' && c <= '9') {
                return what + " must not contain an ASCII digit; got '" + designator + "'";
            }
        }
        if (asciiWhitespace(designator.charAt(0)) || asciiWhitespace(designator.charAt(designator.length() - 1))) {
            return what + " must not start or end with ASCII whitespace; got '" + designator + "'";
        }
        return null;
    }

    private static boolean asciiWhitespace(char c) {
        return c == ' ' || c == '\t' || c == '\n' || c == '\f' || c == '\r';
    }

    /** How many locales {@link #from(Locale)} keeps derived formats for. */
    private static final int LOCALE_CACHE_LIMIT = 64;

    /**
     * {@link #from(Locale)}'s derived formats, by locale: the short-date pattern lookup and
     * {@link DateFormatSymbols#getInstance} both cost on every call, and the {@code Locale}
     * overloads of the door call {@code from} on every cast.
     */
    private static final ConcurrentHashMap<Locale, DateTimeFormat> BY_LOCALE = new ConcurrentHashMap<>();
}
