package io.github.skunkwerkx.hypercast;

import java.nio.charset.StandardCharsets;
import java.text.DecimalFormatSymbols;
import java.util.Locale;

/**
 * Caller-declared numeric notation for the integer, real and decimal doors. The native core
 * carries no culture data — this is the bridge from Java's locale machinery to the core's
 * declared fields. A call site parsing culture-sensitive text declares its format out loud
 * ({@link #INVARIANT}, or {@link #from(Locale)}) — there is no defaulting overload, the
 * same stance Svartalfheim took with {@code IFormatProvider} and the C# binding keeps.
 *
 * <p>The currency symbol — the one notation that truly needs culture tables — is declared,
 * never looked up: {@link #from(Locale)} fills it from the locale's own symbols, and the
 * doors honor it only while {@link #STYLE_CURRENCY} is set. Separators are single UTF-16
 * code units, which covers every separator any real locale declares ({@code .} {@code ,}
 * U+00A0, U+202F, …).
 *
 * @param decimalSeparator the declared decimal separator
 * @param groupSeparator the declared digit-group separator; must differ from {@code decimalSeparator}
 * @param styles bitwise OR of the {@code STYLE_*} flags
 * @param currencySymbol the declared currency symbol — at most 16 UTF-8 bytes, with no ASCII
 *     digit or ASCII whitespace; {@code ""} declares none
 */
public record NumFormat(char decimalSeparator, char groupSeparator, int styles, String currencySymbol) {
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
     * an i8). Integer doors only.
     */
    public static final int STYLE_RADIX_PREFIXES = 1 << 3;
    /** Permit a trailing {@code %}, dividing by 100 ({@code 50%} is 0.5). Real and decimal doors only. */
    public static final int STYLE_PERCENT = 1 << 4;
    /**
     * Resolve the {@code .}/{@code ,} roles per input from structure instead of the
     * declared separators (which are ignored while this style is set). Detection, not
     * sniffing: a repeated separator is grouping ({@code 1.234.567,89}); with both present
     * the rightmost is the decimal; a single separator with a non-3-digit right run is the
     * decimal ({@code 3,1415}); with exactly 3 digits right, only a {@code 0} integer part
     * proves decimal ({@code 0,785}). Genuinely ambiguous input ({@code 12.185},
     * {@code 1,000}) is {@link CastFailure#MALFORMED} at the separator, never guessed.
     */
    public static final int STYLE_SEPARATOR_DETECT = 1 << 5;
    /**
     * Permit the declared {@link #currencySymbol()} at either edge of the numeric body —
     * leading ({@code $5}, {@code -$5}, {@code $ -5}) or trailing ({@code 5 €},
     * {@code 1.234,50 kr.}), once, with optional ASCII whitespace between symbol and digits;
     * accounting parentheses wrap the symbol along with the digits ({@code ($5)}). A symbol
     * declared without this style is {@link CastFailure#MALFORMED} where it appears; this
     * style with no symbol declared matches nothing and changes nothing.
     */
    public static final int STYLE_CURRENCY = 1 << 6;

    /**
     * Every lenience on. {@link #STYLE_SEPARATOR_DETECT} is a separator <em>policy</em>, not
     * a lenience, and is deliberately not included.
     */
    public static final int STYLE_ALL = STYLE_GROUPING
            | STYLE_PARENTHESES
            | STYLE_EXPONENT
            | STYLE_RADIX_PREFIXES
            | STYLE_PERCENT
            | STYLE_CURRENCY;

    /**
     * The invariant profile — {@code .} decimal, {@code ,} grouping, every lenience on, no
     * currency symbol declared.
     */
    public static final NumFormat INVARIANT = new NumFormat('.', ',', STYLE_ALL);

    /**
     * The detection profile — every lenience on, {@code .}/{@code ,} roles resolved per
     * input by {@link #STYLE_SEPARATOR_DETECT}'s structural rules.
     */
    public static final NumFormat DETECT = new NumFormat('.', ',', STYLE_ALL | STYLE_SEPARATOR_DETECT);

    // The inline capacity the core's RawNumFormat holds the symbol in, in UTF-8 bytes.
    private static final int CURRENCY_SYMBOL_MAX_BYTES = 16;

    /**
     * Validates the declared separators up front — distinct, and whole code points — and the
     * currency symbol — at most 16 UTF-8 bytes, no ASCII digit or ASCII whitespace, which
     * would collide with the digit scan and the trimming the doors do around it — so a
     * malformed format fails loudly as the caller bug it is, never as a verdict.
     */
    public NumFormat {
        if (decimalSeparator == groupSeparator) {
            throw new IllegalArgumentException(
                    "Decimal and group separators must differ; both are '" + decimalSeparator + "'");
        }
        if (Character.isSurrogate(decimalSeparator) || Character.isSurrogate(groupSeparator)) {
            throw new IllegalArgumentException("Separators must be whole code points, not surrogate halves");
        }
        if (!StandardCharsets.UTF_8.newEncoder().canEncode(currencySymbol)) {
            throw new IllegalArgumentException("Currency symbol must be whole code points, not surrogate halves");
        }
        int bytes = currencySymbol.getBytes(StandardCharsets.UTF_8).length;
        if (bytes > CURRENCY_SYMBOL_MAX_BYTES) {
            throw new IllegalArgumentException("Currency symbol must be at most " + CURRENCY_SYMBOL_MAX_BYTES
                    + " UTF-8 bytes; '" + currencySymbol + "' is " + bytes);
        }
        for (int i = 0; i < currencySymbol.length(); i++) {
            char c = currencySymbol.charAt(i);
            // The core's own rule: ASCII digits and ASCII whitespace (space, \t, \n, \f, \r).
            if (c >= '0' && c <= '9' || c == ' ' || c == '\t' || c == '\n' || c == '\f' || c == '\r') {
                throw new IllegalArgumentException(
                        "Currency symbol must not contain an ASCII digit or whitespace; got '" + currencySymbol + "'");
            }
        }
    }

    /**
     * Declares the separators and styles with no currency symbol.
     *
     * @param decimalSeparator the declared decimal separator
     * @param groupSeparator the declared digit-group separator; must differ from {@code decimalSeparator}
     * @param styles bitwise OR of the {@code STYLE_*} flags
     */
    public NumFormat(char decimalSeparator, char groupSeparator, int styles) {
        this(decimalSeparator, groupSeparator, styles, "");
    }

    /**
     * Derives a format from a locale's number formatting symbols — separators and currency
     * symbol — with every lenience on.
     *
     * @param locale the locale to derive from, never null
     * @return the locale's separators and currency symbol with every lenience style enabled
     */
    public static NumFormat from(Locale locale) {
        DecimalFormatSymbols symbols = DecimalFormatSymbols.getInstance(locale);
        return new NumFormat(
                symbols.getDecimalSeparator(),
                symbols.getGroupingSeparator(),
                STYLE_ALL,
                symbols.getCurrencySymbol());
    }
}
