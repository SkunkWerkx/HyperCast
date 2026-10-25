package io.github.skunkwerkx.hypercast;

import java.nio.charset.StandardCharsets;
import java.text.DecimalFormat;
import java.text.DecimalFormatSymbols;
import java.text.NumberFormat;
import java.util.Locale;
import java.util.Objects;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Caller-declared numeric notation for the integer, real and decimal doors. The native core
 * carries no culture data — this is the bridge from Java's locale machinery to the core's
 * declared fields. A call site parsing culture-sensitive text declares its format out loud
 * ({@link #INVARIANT}, or {@link #from(Locale)}) — there is no defaulting overload, the
 * same stance Svartalfheim took with {@code IFormatProvider} and the C# binding keeps.
 *
 * <p>Past the two number separators and the styles, every field is a further spelling read
 * <em>beside</em> the ASCII one the core always reads: a declared minus sign beside
 * {@code -}, a plus sign beside {@code +}, a percent symbol beside {@code %}, a block of
 * native digits beside {@code 0}–{@code 9}, the currency separators beside the number ones
 * for a currency amount. Nothing that parsed under a sparser format stops parsing under a
 * fuller one, and every field defaults to declaring nothing, which reads exactly as the
 * invariant culture. {@link #from(Locale)} fills all of them from the locale's own symbols,
 * and the doors honor the currency symbol only while {@link #STYLE_CURRENCY} is set. A
 * separator is 1 to 8 UTF-8 bytes with no ASCII digit, which covers every separator
 * any real locale declares ({@code .}, {@code ,}, U+00A0, U+202F, an apostrophe and a space).
 *
 * <p>The boolean door's lexicon is not here, and stays English by decision: no platform
 * carries culture data for its yes/no words, so no bridge could fill one. A caller maps its
 * own words before the door.
 *
 * @param decimalSeparator the declared decimal separator: 1 to 8 UTF-8 bytes, no ASCII digit
 * @param groupSeparator the declared digit-group separator, likewise; must differ from
 *     {@code decimalSeparator}
 * @param styles bitwise OR of the {@code STYLE_*} flags
 * @param currencySymbol the declared currency symbol — at most 16 UTF-8 bytes, with no ASCII
 *     digit or ASCII whitespace; {@code ""} declares none
 * @param currencyDecimalSeparator the decimal separator of a currency amount, read in place
 *     of {@code decimalSeparator} when the declared symbol is in the text (a decimal point in
 *     Swiss French currency, a decimal comma in its numbers); {@code ""} keeps
 *     {@code decimalSeparator}
 * @param currencyGroupSeparator the group separator of a currency amount, likewise; {@code ""}
 *     keeps {@code groupSeparator}; the pair in force for a currency amount must differ
 * @param minusSign a further spelling of the minus sign, read wherever ASCII {@code -} is
 *     (U+2212 in Estonian and Basque): at most 8 UTF-8 bytes, no ASCII digit or whitespace;
 *     {@code ""} declares none
 * @param plusSign a further spelling of the plus sign, likewise beside ASCII {@code +}
 * @param percentSymbol a further spelling of the percent symbol, read as a trailing
 *     {@code %} is while {@link #STYLE_PERCENT} is set ({@code ٪} in Arabic-script locales);
 *     {@code ""} declares none
 * @param digitZero the code point of the zero of a native digit block, whose ten digits are
 *     read beside ASCII {@code 0}–{@code 9} wherever digits are (U+0660 for the Arabic-Indic
 *     digits, U+FF10 for the fullwidth ones — the way {@link DecimalFormatSymbols#getZeroDigit()}
 *     names a locale's digits); {@code 0} declares none
 */
public record NumFormat(
        String decimalSeparator,
        String groupSeparator,
        int styles,
        String currencySymbol,
        String currencyDecimalSeparator,
        String currencyGroupSeparator,
        String minusSign,
        String plusSign,
        String percentSymbol,
        int digitZero) {
    /**
     * Permit the declared group separator between digits. Group sizes are not validated —
     * a separator must simply sit between two digits (HyperCast's own deterministic rule).
     */
    public static final int STYLE_GROUPING = 1;
    /** Permit accounting parentheses as negation: {@code (1,234)} is -1234. */
    public static final int STYLE_PARENTHESES = 1 << 1;
    /**
     * Permit exponent notation: {@code 1e3}, {@code 2.5E-3}. Integer doors reject a negative
     * exponent — a decimal point is never accepted on an integer, in any disguise.
     */
    public static final int STYLE_EXPONENT = 1 << 2;
    /**
     * Permit the culture-insensitive radix prefixes {@code 0x}/{@code &H} (hex) and
     * {@code 0b} (binary), read as the two's-complement bit pattern ({@code 0xFF} is -1 for
     * an i8). Integer doors only. The digits after a prefix are ASCII whatever
     * {@link #digitZero()} declares.
     */
    public static final int STYLE_RADIX_PREFIXES = 1 << 3;
    /**
     * Permit a trailing {@code %} — or the declared {@link #percentSymbol()} — dividing by
     * 100 ({@code 50%} is 0.5). Real and decimal doors only.
     */
    public static final int STYLE_PERCENT = 1 << 4;
    /**
     * Resolve the {@code .}/{@code ,} roles per input from structure instead of the
     * declared separators (which are ignored while this style is set, the currency ones
     * included — detection governs the whole token). Detection, not sniffing: a repeated
     * separator is grouping ({@code 1.234.567,89}); with both present the rightmost is the
     * decimal; a single separator with a non-3-digit right run is the decimal
     * ({@code 3,1415}); with exactly 3 digits right, only a {@code 0} integer part proves
     * decimal ({@code 0,785}). Genuinely ambiguous input ({@code 12.185}, {@code 1,000}) is
     * {@link CastFailure#MALFORMED} at the separator, never guessed.
     */
    public static final int STYLE_SEPARATOR_DETECT = 1 << 5;
    /**
     * Permit the declared {@link #currencySymbol()} at either edge of the numeric body —
     * leading ({@code $5}, {@code -$5}, {@code $ -5}) or trailing ({@code 5 €},
     * {@code 1.234,50 kr.}), once, with optional ASCII whitespace between symbol and digits;
     * accounting parentheses wrap the symbol along with the digits ({@code ($5)}). A symbol
     * declared without this style is {@link CastFailure#MALFORMED} where it appears; this
     * style with no symbol declared matches nothing and changes nothing. With the symbol in
     * the text, the digits are read with the currency separators where those are declared.
     */
    public static final int STYLE_CURRENCY = 1 << 6;
    /**
     * Permit the sign after the digits ({@code 5-}, {@code 5 -}, {@code 5 €-}, {@code 5- €},
     * {@code 2.5%-}), the way a locale whose negative pattern is {@code n-} writes it: ASCII
     * {@code -}/{@code +} or the declared signs, once, with optional ASCII whitespace before
     * it, and never beside a leading sign or inside accounting parentheses. Not part of
     * {@link #STYLE_ALL}: {@link #from(Locale)} sets it from the locale's own negative
     * pattern rather than assuming it.
     */
    public static final int STYLE_TRAILING_SIGN = 1 << 7;

    /**
     * Every default lenience on. {@link #STYLE_SEPARATOR_DETECT} is a separator
     * <em>policy</em>, not a lenience, and {@link #STYLE_TRAILING_SIGN} is declared from a
     * locale's pattern rather than assumed; neither is included.
     */
    public static final int STYLE_ALL =
            STYLE_GROUPING | STYLE_PARENTHESES | STYLE_EXPONENT | STYLE_RADIX_PREFIXES | STYLE_PERCENT | STYLE_CURRENCY;

    /**
     * The invariant profile — {@code .} decimal, {@code ,} grouping, every default lenience
     * on, nothing else declared.
     */
    public static final NumFormat INVARIANT = new NumFormat(".", ",", STYLE_ALL);

    /**
     * The detection profile — every default lenience on, {@code .}/{@code ,} roles resolved
     * per input by {@link #STYLE_SEPARATOR_DETECT}'s structural rules.
     */
    public static final NumFormat DETECT = new NumFormat(".", ",", STYLE_ALL | STYLE_SEPARATOR_DETECT);

    // The inline capacity the core's RawNumFormat holds each symbol in, in UTF-8 bytes: a
    // separator, a sign or the percent symbol, and the wider currency symbol.
    private static final int SYMBOL_MAX_BYTES = 8;
    private static final int CURRENCY_SYMBOL_MAX_BYTES = 16;

    /**
     * Validates every field up front, by the core's own rules, so a malformed format fails
     * loudly as the caller bug it is, never as a verdict: the number separators 1 to 8 UTF-8
     * bytes each and distinct; the currency separators the same or empty, and the pair in
     * force for a currency amount distinct; the signs and the percent symbol at most 8 bytes
     * and the currency symbol at most 16, with no ASCII whitespace, which the doors trim
     * around them; every symbol whole code points with no ASCII digit, which the digit scan
     * would read first; and the digit zero the start of a block ten digits can be read from.
     *
     * @throws IllegalArgumentException for any field the core would refuse
     * @throws NullPointerException if any symbol is null
     */
    public NumFormat {
        checkSymbol("Decimal separator", decimalSeparator, SYMBOL_MAX_BYTES, true, false);
        checkSymbol("Group separator", groupSeparator, SYMBOL_MAX_BYTES, true, false);
        if (decimalSeparator.equals(groupSeparator)) {
            throw new IllegalArgumentException(
                    "Decimal and group separators must differ; both are '" + decimalSeparator + "'");
        }
        checkSymbol("Currency decimal separator", currencyDecimalSeparator, SYMBOL_MAX_BYTES, false, false);
        checkSymbol("Currency group separator", currencyGroupSeparator, SYMBOL_MAX_BYTES, false, false);
        String currencyDecimal = currencyDecimalSeparator.isEmpty() ? decimalSeparator : currencyDecimalSeparator;
        String currencyGroup = currencyGroupSeparator.isEmpty() ? groupSeparator : currencyGroupSeparator;
        if (currencyDecimal.equals(currencyGroup)) {
            throw new IllegalArgumentException(
                    "A currency amount's decimal and group separators must differ; both are '" + currencyDecimal + "'");
        }
        checkSymbol("Minus sign", minusSign, SYMBOL_MAX_BYTES, false, true);
        checkSymbol("Plus sign", plusSign, SYMBOL_MAX_BYTES, false, true);
        checkSymbol("Percent symbol", percentSymbol, SYMBOL_MAX_BYTES, false, true);
        checkSymbol("Currency symbol", currencySymbol, CURRENCY_SYMBOL_MAX_BYTES, false, true);
        // The core's rule: every decimal digit block Unicode defines starts at a code point
        // whose ten successors share its UTF-8 prefix, which is what lets a digit be read from
        // its last byte — a zero whose value modulo 64 is past 54 starts no such block, and an
        // ASCII zero declares nothing the core does not read already.
        if (digitZero != 0
                && (digitZero < 0x80
                        || digitZero % 64 > 54
                        || !Character.isValidCodePoint(digitZero)
                        || Character.isBmpCodePoint(digitZero) && Character.isSurrogate((char) digitZero))) {
            throw new IllegalArgumentException("Digit zero must be the zero of a Unicode decimal digit block; got U+"
                    + Integer.toHexString(digitZero).toUpperCase(Locale.ROOT));
        }
    }

    /**
     * Declares the separators and styles with no currency symbol and nothing else.
     *
     * @param decimalSeparator the declared decimal separator
     * @param groupSeparator the declared digit-group separator; must differ from {@code decimalSeparator}
     * @param styles bitwise OR of the {@code STYLE_*} flags
     */
    public NumFormat(String decimalSeparator, String groupSeparator, int styles) {
        this(decimalSeparator, groupSeparator, styles, "");
    }

    /**
     * Declares the separators, styles and currency symbol, and nothing else.
     *
     * @param decimalSeparator the declared decimal separator
     * @param groupSeparator the declared digit-group separator; must differ from {@code decimalSeparator}
     * @param styles bitwise OR of the {@code STYLE_*} flags
     * @param currencySymbol the declared currency symbol; {@code ""} declares none
     */
    public NumFormat(String decimalSeparator, String groupSeparator, int styles, String currencySymbol) {
        this(decimalSeparator, groupSeparator, styles, currencySymbol, "", "", "", "", "", 0);
    }

    /**
     * Declares one-character separators and styles with no currency symbol and nothing else.
     *
     * @param decimalSeparator the declared decimal separator
     * @param groupSeparator the declared digit-group separator; must differ from {@code decimalSeparator}
     * @param styles bitwise OR of the {@code STYLE_*} flags
     */
    public NumFormat(char decimalSeparator, char groupSeparator, int styles) {
        this(String.valueOf(decimalSeparator), String.valueOf(groupSeparator), styles, "");
    }

    /**
     * Declares one-character separators, styles and the currency symbol, and nothing else.
     *
     * @param decimalSeparator the declared decimal separator
     * @param groupSeparator the declared digit-group separator; must differ from {@code decimalSeparator}
     * @param styles bitwise OR of the {@code STYLE_*} flags
     * @param currencySymbol the declared currency symbol; {@code ""} declares none
     */
    public NumFormat(char decimalSeparator, char groupSeparator, int styles, String currencySymbol) {
        this(String.valueOf(decimalSeparator), String.valueOf(groupSeparator), styles, currencySymbol);
    }

    /**
     * The same format under other styles — {@code INVARIANT.withStyles(STYLE_ALL | STYLE_TRAILING_SIGN)}
     * opts into a trailing sign, {@code withStyles(STYLE_ALL & ~STYLE_RADIX_PREFIXES)} out of
     * radix prefixes.
     *
     * @param styles bitwise OR of the {@code STYLE_*} flags
     * @return the format with {@code styles} in place of this one's
     */
    public NumFormat withStyles(int styles) {
        return new NumFormat(
                decimalSeparator,
                groupSeparator,
                styles,
                currencySymbol,
                currencyDecimalSeparator,
                currencyGroupSeparator,
                minusSign,
                plusSign,
                percentSymbol,
                digitZero);
    }

    /**
     * The same format with the currency symbol declared; pair it with {@link #STYLE_CURRENCY}
     * (part of {@link #STYLE_ALL}) for the symbol to be honored.
     *
     * @param currencySymbol the declared currency symbol; {@code ""} declares none
     * @return the format with {@code currencySymbol} in place of this one's
     */
    public NumFormat withCurrencySymbol(String currencySymbol) {
        return new NumFormat(
                decimalSeparator,
                groupSeparator,
                styles,
                currencySymbol,
                currencyDecimalSeparator,
                currencyGroupSeparator,
                minusSign,
                plusSign,
                percentSymbol,
                digitZero);
    }

    /**
     * The same format with the separators a currency amount is read with, each {@code ""}
     * keeping the number separator it stands in for.
     *
     * @param currencyDecimalSeparator the decimal separator of a currency amount
     * @param currencyGroupSeparator the group separator of a currency amount
     * @return the format with the currency separators in place of this one's
     */
    public NumFormat withCurrencySeparators(String currencyDecimalSeparator, String currencyGroupSeparator) {
        return new NumFormat(
                decimalSeparator,
                groupSeparator,
                styles,
                currencySymbol,
                currencyDecimalSeparator,
                currencyGroupSeparator,
                minusSign,
                plusSign,
                percentSymbol,
                digitZero);
    }

    /**
     * The same format with further spellings of the minus and plus signs declared beside the
     * ASCII ones, each {@code ""} declaring none.
     *
     * @param minusSign the further minus sign
     * @param plusSign the further plus sign
     * @return the format with the signs in place of this one's
     */
    public NumFormat withSigns(String minusSign, String plusSign) {
        return new NumFormat(
                decimalSeparator,
                groupSeparator,
                styles,
                currencySymbol,
                currencyDecimalSeparator,
                currencyGroupSeparator,
                minusSign,
                plusSign,
                percentSymbol,
                digitZero);
    }

    /**
     * The same format with a further spelling of the percent symbol declared beside ASCII
     * {@code %}.
     *
     * @param percentSymbol the further percent symbol; {@code ""} declares none
     * @return the format with {@code percentSymbol} in place of this one's
     */
    public NumFormat withPercentSymbol(String percentSymbol) {
        return new NumFormat(
                decimalSeparator,
                groupSeparator,
                styles,
                currencySymbol,
                currencyDecimalSeparator,
                currencyGroupSeparator,
                minusSign,
                plusSign,
                percentSymbol,
                digitZero);
    }

    /**
     * The same format with a block of native digits declared beside ASCII {@code 0}–{@code 9}.
     *
     * @param digitZero the code point of the block's zero; {@code 0} declares none
     * @return the format with {@code digitZero} in place of this one's
     */
    public NumFormat withDigitZero(int digitZero) {
        return new NumFormat(
                decimalSeparator,
                groupSeparator,
                styles,
                currencySymbol,
                currencyDecimalSeparator,
                currencyGroupSeparator,
                minusSign,
                plusSign,
                percentSymbol,
                digitZero);
    }

    /**
     * Derives a format from a locale's own number formatting data, every field filled: the
     * separators, currency symbol, monetary separators, minus sign, percent symbol and zero
     * digit of its {@link DecimalFormatSymbols}, with every default lenience on, plus
     * {@link #STYLE_TRAILING_SIGN} when the locale's {@link DecimalFormat} writes a negative
     * number with a suffix and no prefix ({@code 5-}). A symbol the core reads anyway — the
     * ASCII minus and percent, a {@code 0} zero digit, a monetary separator equal to the
     * number one — is left undeclared, since declaring it would change nothing; the plus sign
     * is left undeclared because the symbols carry none. Nothing is ever taken away: ASCII
     * {@code -}, {@code +}, {@code %} and {@code 0}–{@code 9} are read beside whatever the
     * locale declares, so a locale whose {@code getMinusSign()} is U+2212 (Estonian, Basque)
     * reads both {@code −5} and {@code -5}. Narrow the styles, or declare a {@code NumFormat}
     * outright, for anything stricter.
     *
     * @param locale the locale to derive from, never null
     * @return the locale's notation, every field filled, with every default lenience style enabled
     * @throws NullPointerException if {@code locale} is null
     */
    public static NumFormat from(Locale locale) {
        Objects.requireNonNull(locale, "locale");
        NumFormat cached = BY_LOCALE.get(locale);
        if (cached != null) {
            return cached;
        }
        // NumberFormat.getInstance is a DecimalFormat for every locale the JDK knows; a
        // formatting provider that supplies something else still yields its symbols, with no
        // negative pattern to read a trailing sign from.
        NumFormat derived = NumberFormat.getInstance(locale) instanceof DecimalFormat pattern
                ? from(pattern)
                : derive(DecimalFormatSymbols.getInstance(locale), STYLE_ALL);
        // Bounded by clearing rather than evicting: a process casts under a handful of
        // locales, and one that cycles through more than this many only pays the derivation
        // again. A NumFormat is an immutable record, so one instance is shared by every caller.
        if (BY_LOCALE.size() >= LOCALE_CACHE_LIMIT) {
            BY_LOCALE.clear();
        }
        BY_LOCALE.put(locale, derived);
        return derived;
    }

    /**
     * Derives a format from a {@link DecimalFormat}'s own symbols and negative pattern —
     * exactly what {@link #from(Locale)} does with {@code NumberFormat.getInstance(locale)},
     * for a format the caller built or customized instead: its
     * {@link DecimalFormat#getDecimalFormatSymbols() symbols} fill the fields as a locale's
     * do, and a {@link DecimalFormat#getNegativeSuffix() negative suffix} with no
     * {@link DecimalFormat#getNegativePrefix() negative prefix} sets
     * {@link #STYLE_TRAILING_SIGN}. Derived on every call, never cached.
     *
     * @param format the format to derive from, never null
     * @return the format's notation, every field filled, with every default lenience style enabled
     * @throws NullPointerException if {@code format} is null
     */
    public static NumFormat from(DecimalFormat format) {
        Objects.requireNonNull(format, "format");
        int styles = STYLE_ALL;
        if (!format.getNegativeSuffix().isEmpty() && format.getNegativePrefix().isEmpty()) {
            styles |= STYLE_TRAILING_SIGN;
        }
        return derive(format.getDecimalFormatSymbols(), styles);
    }

    private static NumFormat derive(DecimalFormatSymbols symbols, int styles) {
        String decimalSeparator = String.valueOf(symbols.getDecimalSeparator());
        String groupSeparator = String.valueOf(symbols.getGroupingSeparator());
        return new NumFormat(
                decimalSeparator,
                groupSeparator,
                styles,
                symbols.getCurrencySymbol(),
                declared(symbols.getMonetaryDecimalSeparator(), decimalSeparator),
                declared(symbols.getMonetaryGroupingSeparator(), groupSeparator),
                declared(symbols.getMinusSign(), "-"),
                "",
                declared(symbols.getPercent(), "%"),
                symbols.getZeroDigit() == '0' ? 0 : symbols.getZeroDigit());
    }

    // A symbol the core reads whether or not it is declared — the ASCII default, or the
    // number separator a currency separator would stand in for — is left undeclared.
    private static String declared(char symbol, String implied) {
        String text = String.valueOf(symbol);
        return text.equals(implied) ? "" : text;
    }

    // The core's rules for one symbol: whole code points, within the field's capacity, no
    // ASCII digit (a digit is a digit before it is anything else, so such a symbol could
    // never be matched), and — for the symbols that sit flush against the digits — no ASCII
    // whitespace (space, \t, \n, \f, \r), which the doors trim around them. The two number
    // separators must be declared; every other symbol may be empty.
    private static void checkSymbol(String what, String symbol, int maxBytes, boolean required, boolean flush) {
        Objects.requireNonNull(symbol, what);
        if (symbol.isEmpty()) {
            if (required) {
                throw new IllegalArgumentException(what + " must not be empty");
            }
            return;
        }
        if (!StandardCharsets.UTF_8.newEncoder().canEncode(symbol)) {
            throw new IllegalArgumentException(what + " must be whole code points, not surrogate halves");
        }
        int bytes = symbol.getBytes(StandardCharsets.UTF_8).length;
        if (bytes > maxBytes) {
            throw new IllegalArgumentException(
                    what + " must be at most " + maxBytes + " UTF-8 bytes; '" + symbol + "' is " + bytes);
        }
        for (int i = 0; i < symbol.length(); i++) {
            char c = symbol.charAt(i);
            if (c >= '0' && c <= '9') {
                throw new IllegalArgumentException(what + " must not contain an ASCII digit; got '" + symbol + "'");
            }
            if (flush && (c == ' ' || c == '\t' || c == '\n' || c == '\f' || c == '\r')) {
                throw new IllegalArgumentException(what + " must not contain ASCII whitespace; got '" + symbol + "'");
            }
        }
    }

    /** How many locales {@link #from(Locale)} keeps derived formats for. */
    private static final int LOCALE_CACHE_LIMIT = 64;

    /**
     * {@link #from(Locale)}'s derived formats, by locale. {@link NumberFormat#getInstance(Locale)}
     * and {@link DecimalFormatSymbols#getInstance} build new objects on every call, and the
     * {@code Locale} overloads of the numeric doors call {@code from} on every cast.
     */
    private static final ConcurrentHashMap<Locale, NumFormat> BY_LOCALE = new ConcurrentHashMap<>();
}
