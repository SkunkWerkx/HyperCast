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
 * <p>How digit groups are judged is the {@link Grouping}, declared apart from the styles and
 * {@link Grouping#WESTERN Western} unless declared otherwise: {@code 1,234,567} reads, {@code
 * 1,23,456} and {@code 12,34} are {@link CastFailure#MALFORMED}. {@code withGrouping(Grouping.INDIAN)}
 * and {@code withGrouping(Grouping.WESTERN)} choose either on any format. Every constructor and
 * {@link #INVARIANT} default to Western; {@link #from(Locale)} declares what the platform's own
 * number pattern says: {@link Grouping#INDIAN} when it is three, then twos, anything else
 * {@link Grouping#WESTERN}, with {@link #STYLE_GROUPING} cleared only when neither the number nor
 * the currency pattern groups. A shape no policy names is refused, never misread: {@code
 * docs/grouping.md}.
 *
 * <p><b>The one deviation from the record copy rule.</b> A record is normally copied by handing
 * its components back to the canonical constructor ({@code new NumFormat(f.decimalSeparator(),
 * ..., f.styles(), ...)}, a deconstruct-and-rebuild, a serialization library). That copy is exact
 * for every field but one: {@link #styles()} answers the {@code STYLE_*} bits alone, because 1.0's
 * ten components are the ones this record keeps (an eleventh would break 1.0's ten-binder record
 * pattern), so the copy does not carry the {@link Grouping} and takes the default,
 * {@link Grouping#WESTERN}. A copy of a Western format is equal to it; a copy of a
 * {@link Grouping#INDIAN} one is Western, so copy with {@link #withGrouping(Grouping)}, the
 * {@code with*} methods (every component has one) or {@code format.withGrouping(copy.grouping())}.
 * The policy lives inside the {@code styles} component of the canonical constructor, in bits 16
 * and 17, as an internal encoding that callers must not set: the constructor accepts only the
 * values a format of this class holds (0 Western, 2 Indian) and refuses the rest.
 *
 * <p>The boolean door's lexicon is not here, and stays English by decision: no platform
 * carries culture data for its yes/no words, so no bridge could fill one. A caller maps its
 * own words before the door.
 *
 * @param decimalSeparator the declared decimal separator: 1 to 8 UTF-8 bytes, no ASCII digit
 *     and no white space
 * @param groupSeparator the declared digit-group separator, likewise but it may be or contain
 *     white space (U+0020, U+00A0 and U+202F stand for each other); must differ from
 *     {@code decimalSeparator}
 * @param styles bitwise OR of the {@code STYLE_*} flags (bits 0 to 7). The {@link Grouping} is
 *     not a style: declare it beside the mask ({@link #NumFormat(String, String, int, Grouping)},
 *     {@link #withGrouping(Grouping)}), where it defaults to {@link Grouping#WESTERN}. As a
 *     component of this record the mask is the stored word, which also holds the policy in bits
 *     16 and 17 (0 Western, 2 Indian; an internal encoding callers must not set, and 1 and 3
 *     are refused, as is any other bit, so a stale ABI word such as 351 is refused here), so {@link #styles()} answers the {@code STYLE_*} bits alone and
 *     {@link #grouping()} the policy; a record rebuilt through the canonical constructor from
 *     {@code styles()} keeps every style and takes the default policy (see the class
 *     description's copy rule)
 * @param currencySymbol the declared currency symbol — at most 16 UTF-8 bytes, with no ASCII
 *     digit and no white space at either end (inside is fine); {@code ""} declares none
 * @param currencyDecimalSeparator the decimal separator of a currency amount, read in place
 *     of {@code decimalSeparator} when the declared symbol is in the text (a decimal point in
 *     Swiss French currency, a decimal comma in its numbers); {@code ""} keeps
 *     {@code decimalSeparator}
 * @param currencyGroupSeparator the group separator of a currency amount, likewise; {@code ""}
 *     keeps {@code groupSeparator}; the pair in force for a currency amount must differ
 * @param minusSign a further spelling of the minus sign, read wherever ASCII {@code -} is
 *     (U+2212 in Estonian and Basque): at most 8 UTF-8 bytes, no ASCII digit or white space;
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
     * Permit the declared group separator between digits. How the groups are judged is the
     * {@link Grouping}: under the default {@link Grouping#WESTERN} every group must be three
     * digits, under {@link Grouping#INDIAN} three beside the decimal separator and twos beyond.
     * Without this style a group separator is refused under every policy.
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
     * {@code 1.234,50 kr.}), once, with optional white space between symbol and digits;
     * accounting parentheses wrap the symbol along with the digits ({@code ($5)}). A symbol
     * declared without this style is {@link CastFailure#MALFORMED} where it appears; this
     * style with no symbol declared matches nothing and changes nothing. With the symbol in
     * the text, the digits are read with the currency separators where those are declared.
     */
    public static final int STYLE_CURRENCY = 1 << 6;
    /**
     * Permit the sign after the digits ({@code 5-}, {@code 5 -}, {@code 5 €-}, {@code 5- €},
     * {@code 2.5%-}), the way a locale whose negative pattern is {@code n-} writes it: ASCII
     * {@code -}/{@code +} or the declared signs, once, with optional white space before
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
     * on, {@link Grouping#WESTERN} grouping, nothing else declared. As of 1.1 this rejects
     * {@code 12,34} and {@code 1,23,456}, which 1.0 read as 1234 and 123456:
     * {@code INVARIANT.withGrouping(Grouping.INDIAN)} refuses the first too, and reads the second.
     */
    public static final NumFormat INVARIANT = new NumFormat(".", ",", STYLE_ALL);

    /**
     * The detection profile — every default lenience on, {@code .}/{@code ,} roles resolved
     * per input by {@link #STYLE_SEPARATOR_DETECT}'s structural rules,
     * {@link Grouping#WESTERN} grouping (no policy resolves an ambiguous {@code 1,234}).
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
     * and the currency symbol at most 16, with no white space in a sign or the percent
     * symbol, which the doors trim around them, and in neither decimal separator, since the
     * numeral admits white space only as a group separator; the currency symbol with none at
     * either end, though inside is fine (CLDR's {@code F CFA}); every symbol whole code points with no ASCII digit,
     * which the digit scan would read first; the digit zero the start of a block ten digits
     * can be read from; and the styles only the defined {@code STYLE_*} bits. A declaration
     * breaking two rules reports the first of: length, digit, equal separators, white space.
     *
     * <p>The canonical constructor is 1.0's, and declares {@link Grouping#WESTERN}: its
     * {@code styles} is the {@code STYLE_*} mask, which may also carry the stored policy of a
     * format this record itself handed out (not a spelling to write by hand: declare a
     * {@link Grouping} with {@link #NumFormat(String, String, int, Grouping)} or
     * {@link #withGrouping(Grouping)}).
     *
     * @throws IllegalArgumentException for any field the core would refuse
     * @throws NullPointerException if any symbol is null
     */
    public NumFormat {
        styles = stored(styles);
        checkSymbol("Decimal separator", decimalSeparator, SYMBOL_MAX_BYTES, true);
        checkSymbol("Group separator", groupSeparator, SYMBOL_MAX_BYTES, true);
        if (decimalSeparator.equals(groupSeparator)) {
            throw new IllegalArgumentException(
                    "Decimal and group separators must differ; both are '" + decimalSeparator + "'");
        }
        checkSymbol("Currency decimal separator", currencyDecimalSeparator, SYMBOL_MAX_BYTES, false);
        checkSymbol("Currency group separator", currencyGroupSeparator, SYMBOL_MAX_BYTES, false);
        String currencyDecimal = currencyDecimalSeparator.isEmpty() ? decimalSeparator : currencyDecimalSeparator;
        String currencyGroup = currencyGroupSeparator.isEmpty() ? groupSeparator : currencyGroupSeparator;
        if (currencyDecimal.equals(currencyGroup)) {
            throw new IllegalArgumentException(
                    "A currency amount's decimal and group separators must differ; both are '" + currencyDecimal + "'");
        }
        checkSymbol("Minus sign", minusSign, SYMBOL_MAX_BYTES, false);
        checkSymbol("Plus sign", plusSign, SYMBOL_MAX_BYTES, false);
        checkSymbol("Percent symbol", percentSymbol, SYMBOL_MAX_BYTES, false);
        checkSymbol("Currency symbol", currencySymbol, CURRENCY_SYMBOL_MAX_BYTES, false);
        // White space comes last, so an earlier rule's message stands. A group separator may
        // be or contain it. No decimal separator, sign or percent symbol can: the doors trim it
        // around a sign and the percent symbol, and a numeral admits it only as a group
        // separator. The currency symbol may hold it inside (CLDR's "F CFA"), not at an edge.
        checkNoWhiteSpace("Decimal separator", decimalSeparator);
        checkNoWhiteSpace("Currency decimal separator", currencyDecimalSeparator);
        checkNoWhiteSpace("Minus sign", minusSign);
        checkNoWhiteSpace("Plus sign", plusSign);
        checkNoWhiteSpace("Percent symbol", percentSymbol);
        if (WhiteSpace.atEdge(currencySymbol)) {
            throw new IllegalArgumentException(
                    "Currency symbol must not start or end with white space; got '" + currencySymbol + "'");
        }
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
     * Declares the separators, styles and grouping policy, with no currency symbol and nothing
     * else. The policy is declared apart from the styles, which never carry one.
     *
     * @param decimalSeparator the declared decimal separator
     * @param groupSeparator the declared digit-group separator; must differ from {@code decimalSeparator}
     * @param styles bitwise OR of the {@code STYLE_*} flags, and nothing else
     * @param grouping how digit groups are judged
     * @throws IllegalArgumentException if {@code styles} carries a bit that is no style
     * @throws NullPointerException if {@code grouping} is null
     */
    public NumFormat(String decimalSeparator, String groupSeparator, int styles, Grouping grouping) {
        this(decimalSeparator, groupSeparator, styles, "", grouping);
    }

    /**
     * Declares the separators, styles, currency symbol and grouping policy, and nothing else.
     *
     * @param decimalSeparator the declared decimal separator
     * @param groupSeparator the declared digit-group separator; must differ from {@code decimalSeparator}
     * @param styles bitwise OR of the {@code STYLE_*} flags, and nothing else
     * @param currencySymbol the declared currency symbol; {@code ""} declares none
     * @param grouping how digit groups are judged
     * @throws IllegalArgumentException if {@code styles} carries a bit that is no style
     * @throws NullPointerException if {@code grouping} is null
     */
    public NumFormat(
            String decimalSeparator, String groupSeparator, int styles, String currencySymbol, Grouping grouping) {
        this(decimalSeparator, groupSeparator, join(styles, grouping), currencySymbol, "", "", "", "", "", 0);
    }

    /**
     * Declares one-character separators, styles and grouping policy, with no currency symbol
     * and nothing else.
     *
     * @param decimalSeparator the declared decimal separator
     * @param groupSeparator the declared digit-group separator; must differ from {@code decimalSeparator}
     * @param styles bitwise OR of the {@code STYLE_*} flags, and nothing else
     * @param grouping how digit groups are judged
     * @throws IllegalArgumentException if {@code styles} carries a bit that is no style
     * @throws NullPointerException if {@code grouping} is null
     */
    public NumFormat(char decimalSeparator, char groupSeparator, int styles, Grouping grouping) {
        this(String.valueOf(decimalSeparator), String.valueOf(groupSeparator), styles, "", grouping);
    }

    /**
     * The {@code STYLE_*} flags alone, as 1.0's accessor answered: the {@link Grouping} is
     * {@link #grouping()}, never a bit of this mask.
     *
     * @return bitwise OR of the {@code STYLE_*} flags
     */
    public int styles() {
        return styles & STYLE_BITS;
    }

    /**
     * How this format judges digit groups.
     *
     * @return the policy; {@link Grouping#WESTERN} unless declared otherwise
     */
    public Grouping grouping() {
        return switch (styles >>> POLICY_SHIFT) {
            case INDIAN_FIELD -> Grouping.INDIAN;
            default -> Grouping.WESTERN;
        };
    }

    /**
     * The {@code flags} word as the native core reads it: the {@code STYLE_*} bits and the
     * {@link Grouping#code() policy}, Indian's bit 9 or nothing for Western. For a library that carries a format across
     * the C ABI of its own, with {@link io.github.skunkwerkx.hypercast.interop.NativeValues}.
     *
     * @return the ABI flags
     */
    public int flags() {
        return styles() | grouping().code();
    }

    /**
     * The same format under another grouping policy, which replaces this one's.
     * {@code INVARIANT.withGrouping(Grouping.INDIAN)} reads {@code 1,23,456}.
     *
     * @param grouping how digit groups are judged
     * @return the format with {@code grouping} in place of this one's
     * @throws NullPointerException if {@code grouping} is null
     */
    public NumFormat withGrouping(Grouping grouping) {
        return withStored(join(styles(), grouping));
    }

    /**
     * The same format under other styles — {@code INVARIANT.withStyles(STYLE_ALL | STYLE_TRAILING_SIGN)}
     * opts into a trailing sign, {@code withStyles(STYLE_ALL & ~STYLE_RADIX_PREFIXES)} out of
     * radix prefixes. The grouping policy is kept.
     *
     * @param styles bitwise OR of the {@code STYLE_*} flags, and nothing else
     * @return the format with {@code styles} in place of this one's
     * @throws IllegalArgumentException if {@code styles} carries a bit that is no style
     */
    public NumFormat withStyles(int styles) {
        return withStored(join(styles, grouping()));
    }

    private NumFormat withStored(int stored) {
        return new NumFormat(
                decimalSeparator,
                groupSeparator,
                stored,
                currencySymbol,
                currencyDecimalSeparator,
                currencyGroupSeparator,
                minusSign,
                plusSign,
                percentSymbol,
                digitZero);
    }

    /**
     * The same format with other number separators, so that every component has a wither.
     * Validated as the constructor does: 1 to 8 UTF-8 bytes each, distinct, no ASCII digit, no
     * white space in the decimal separator (a currency pair that was distinct must stay so).
     *
     * @param decimalSeparator the declared decimal separator
     * @param groupSeparator the declared digit-group separator; must differ from {@code decimalSeparator}
     * @return the format with the separators in place of this one's
     */
    public NumFormat withSeparators(String decimalSeparator, String groupSeparator) {
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
     * number with a suffix and no prefix ({@code 5-}), and the grouping its number and currency
     * patterns declare (below). A symbol the core reads anyway — the ASCII minus and percent, a
     * {@code 0} zero digit, a monetary separator equal to the number one — is left undeclared,
     * since declaring it would change nothing; the plus sign is left undeclared because the
     * symbols carry none. Nothing is ever taken away: ASCII {@code -}, {@code +}, {@code %}
     * and {@code 0}–{@code 9} are read beside whatever the locale declares, so a locale whose
     * {@code getMinusSign()} is U+2212 (Estonian, Basque) reads both {@code −5} and
     * {@code -5}. Narrow the styles, or declare a {@code NumFormat} outright, for anything
     * stricter.
     *
     * <p><b>The grouping rule.</b> One format has one {@link Grouping} and one
     * {@link #STYLE_GROUPING} flag, shared by number and currency values. The policy follows the
     * platform's number pattern ({@link NumberFormat#getInstance(Locale)}), never the locale's name
     * and never the input:
     *
     * <ul>
     *   <li>a number pattern of three, then twos is {@link Grouping#INDIAN};
     *   <li>anything else is {@link Grouping#WESTERN}: groups of three, no grouping, or another
     *       shape (Toki Pona's groups of two, {@code #,#0.###}), whose own grouped output is then
     *       refused, never misread ({@code docs/grouping.md});
     *   <li>{@link #STYLE_GROUPING} is cleared only when neither the number nor the currency
     *       pattern ({@link NumberFormat#getCurrencyInstance(Locale)}) groups. When just one does
     *       not, the style stays for the other and an ungrouped value stays valid; the format
     *       cannot prohibit grouping of currency amounts alone.
     * </ul>
     *
     * A currency pattern that differs from the number's is refused through the bridge's format
     * until the column declares its own policy with {@link #withGrouping(Grouping)}.
     *
     * The rule is deterministic: the same patterns give the same format. {@link DecimalFormat}
     * keeps one grouping size, the last interval, with no secondary size, so the JDK's own pattern
     * for an Indian locale reads {@code #,##0.###}, the JDK writes {@code 12,345,678} for it, and
     * Western is right for what the platform produces. Text in CLDR's Indian grouping
     * ({@code 12,34,567}) is read with {@code from(locale).withGrouping(Grouping.INDIAN)}. The
     * data is the JDK's own CLDR tables, never ICU.
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
        // pattern to read a trailing sign or a grouping from.
        NumFormat derived = NumberFormat.getInstance(locale) instanceof DecimalFormat number
                ? from(
                        number,
                        NumberFormat.getCurrencyInstance(locale) instanceof DecimalFormat currency ? currency : null)
                : derive(DecimalFormatSymbols.getInstance(locale), STYLE_ALL, Grouping.WESTERN);
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
     * Derives a format from a {@link DecimalFormat}'s own symbols and pattern: its
     * {@link DecimalFormat#getDecimalFormatSymbols() symbols} fill the fields as a locale's do, a
     * {@link DecimalFormat#getNegativeSuffix() negative suffix} with no
     * {@link DecimalFormat#getNegativePrefix() negative prefix} sets
     * {@link #STYLE_TRAILING_SIGN}, and {@link DecimalFormat#toPattern() its pattern} declares
     * the grouping by {@link #from(Locale)}'s rule, with this one pattern the only one read, and
     * {@link #STYLE_GROUPING} cleared when the pattern does not group. {@link DecimalFormat} keeps
     * one grouping size, so {@code new DecimalFormat("#,##,##0.###")} has already lost the
     * secondary size and its pattern reads {@code #,##0.###}: this is Western for every format the
     * JDK can hold. Declare Indian with {@link #withGrouping(Grouping)}. Derived on every call,
     * never cached.
     *
     * @param format the format to derive from, never null
     * @return the format's notation, every field filled, with every default lenience style enabled
     * @throws NullPointerException if {@code format} is null
     */
    public static NumFormat from(DecimalFormat format) {
        return from(format, null);
    }

    /**
     * Derives a format from a number {@link DecimalFormat} and a currency one, as
     * {@link #from(Locale)} does with a locale's two: the symbols, trailing sign and
     * policy are the number format's, and {@link #STYLE_GROUPING} goes only when neither groups.
     * Derived on every call, never cached.
     *
     * @param number the number format, never null
     * @param currency the currency format; {@code null} reads the number pattern alone
     * @return the formats' notation, every field filled, with every default lenience style enabled
     * @throws NullPointerException if {@code number} is null
     */
    public static NumFormat from(DecimalFormat number, DecimalFormat currency) {
        Objects.requireNonNull(number, "number");
        int styles = STYLE_ALL;
        if (!number.getNegativeSuffix().isEmpty() && number.getNegativePrefix().isEmpty()) {
            styles |= STYLE_TRAILING_SIGN;
        }
        Shape shape = shapeOf(number.toPattern());
        if (shape == Shape.NONE && (currency == null || shapeOf(currency.toPattern()) == Shape.NONE)) {
            styles &= ~STYLE_GROUPING;
        }
        return derive(number.getDecimalFormatSymbols(), styles, groupingOf(shape));
    }

    /** How one {@link DecimalFormat} pattern groups the integer part of its positive subpattern. */
    enum Shape {
        /** No group separator: grouping is off (a size of 0 included). */
        NONE,
        /** Groups of three. */
        WESTERN,
        /** Three beside the decimal separator, then twos. */
        INDIAN,
        /** Any other size (Toki Pona's groups of two). */
        OTHER
    }

    /**
     * The shape of a {@link DecimalFormat} pattern, read from its integer part (quoted text
     * skipped, the negative subpattern ignored). The last interval is the primary group and the
     * one before it the secondary, and a single separator has no secondary of its own, so it
     * repeats the primary ({@code #,##0} is three then three): three then three is Western, three
     * then two ({@code #,##,##0.###}, {@code ¤ #,##,##0.00}) Indian, no separator none, and
     * anything else other.
     */
    static Shape shapeOf(String pattern) {
        StringBuilder integer = new StringBuilder();
        boolean quoted = false;
        boolean inNumber = false;
        for (int i = 0; i < pattern.length(); i++) {
            char c = pattern.charAt(i);
            if (c == '\'') {
                quoted = !quoted;
                if (inNumber) {
                    break;
                }
            } else if (quoted) {
                continue;
            } else if (c == ';') {
                break;
            } else if (c == '#' || c == '0' || c == ',') {
                inNumber = true;
                integer.append(c);
            } else if (inNumber) {
                break;
            }
        }
        String[] groups = integer.toString().split(",", -1);
        int n = groups.length;
        if (n < 2) {
            return Shape.NONE;
        }
        int primary = groups[n - 1].length();
        int secondary = n >= 3 ? groups[n - 2].length() : primary;
        if (primary == 3 && secondary == 3) {
            return Shape.WESTERN;
        }
        return primary == 3 && secondary == 2 ? Shape.INDIAN : Shape.OTHER;
    }

    /** The {@link Grouping} a number pattern's shape declares: Indian for Indian, else Western. */
    static Grouping groupingOf(Shape number) {
        return number == Shape.INDIAN ? Grouping.INDIAN : Grouping.WESTERN;
    }

    private static NumFormat derive(DecimalFormatSymbols symbols, int styles, Grouping grouping) {
        String decimalSeparator = String.valueOf(symbols.getDecimalSeparator());
        String groupSeparator = String.valueOf(symbols.getGroupingSeparator());
        return new NumFormat(
                decimalSeparator,
                groupSeparator,
                join(styles, grouping),
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

    // The styles component is the stored word: the STYLE_* bits (0 to 7) and, in bits 16 and 17,
    // the policy as a field: 0 Western, 2 Indian, and 1 and 3 never. Western is 0, so a 1.0
    // constructor call, which carries no policy, means Western and its word is 1.0's own. The
    // ABI word is flags(), and bits 8 and 9 are never stored, so a stale ABI 351 or 863 here is an
    // undefined bit.
    private static final int STYLE_BITS = 0xFF;
    private static final int POLICY_SHIFT = 16;
    private static final int POLICY_FIELD = 3 << POLICY_SHIFT;
    private static final int WESTERN_FIELD = 0;
    private static final int INDIAN_FIELD = 2;

    // The canonical constructor's word: styles and a stored policy that a format of this class
    // could hold, and nothing else.
    private static int stored(int styles) {
        if ((styles & ~(STYLE_BITS | POLICY_FIELD)) != 0) {
            throw undefinedStyles(styles);
        }
        int field = (styles & POLICY_FIELD) >>> POLICY_SHIFT;
        if (field != WESTERN_FIELD && field != INDIAN_FIELD) {
            throw new IllegalArgumentException("The stored policy (bits 16 and 17 of the styles component, an "
                    + "internal encoding) is 0 Western or 2 Indian; " + field + " names no grouping policy");
        }
        return styles;
    }

    // A style mask and a policy declared apart: the mask may carry no bit that is no style.
    private static int join(int styles, Grouping grouping) {
        Objects.requireNonNull(grouping, "grouping");
        if ((styles & ~STYLE_BITS) != 0) {
            throw undefinedStyles(styles);
        }
        int field = switch (grouping) {
            case WESTERN -> WESTERN_FIELD;
            case INDIAN -> INDIAN_FIELD;
        };
        return styles | field << POLICY_SHIFT;
    }

    private static IllegalArgumentException undefinedStyles(int styles) {
        return new IllegalArgumentException("Styles must be STYLE_* flags only (the grouping policy is declared as a "
                + "Grouping, never in the mask); 0x"
                + Integer.toHexString(styles).toUpperCase(Locale.ROOT)
                + " has undefined bits");
    }

    // The core's rules for one symbol, short of white space: whole code points, within the
    // field's capacity, and no ASCII digit (a digit is a digit before it is anything else,
    // so such a symbol could never be matched). The two number separators must be declared;
    // every other symbol may be empty.
    private static void checkSymbol(String what, String symbol, int maxBytes, boolean required) {
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
        }
    }

    private static void checkNoWhiteSpace(String what, String symbol) {
        if (WhiteSpace.contains(symbol)) {
            throw new IllegalArgumentException(what + " must not contain white space; got '" + symbol + "'");
        }
    }

    /**
     * Renders every field and the {@link #grouping() grouping}, with the styles as
     * {@link #styles()} answers them.
     *
     * @return a description of the format
     */
    @Override
    public String toString() {
        return "NumFormat[decimalSeparator=" + decimalSeparator + ", groupSeparator=" + groupSeparator + ", styles="
                + styles() + ", currencySymbol=" + currencySymbol + ", currencyDecimalSeparator="
                + currencyDecimalSeparator + ", currencyGroupSeparator=" + currencyGroupSeparator + ", minusSign="
                + minusSign + ", plusSign=" + plusSign + ", percentSymbol=" + percentSymbol + ", digitZero="
                + digitZero + ", grouping=" + grouping() + "]";
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
