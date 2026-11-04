package io.github.skunkwerkx.hypercast;

import java.util.Optional;

/**
 * What failed, as the native core names it: a small shared vocabulary, finer than the
 * {@link CastFailure reason} and independent of it ({@code digit_expected}, {@code group_size},
 * {@code too_large}, {@code not_finite}), read from a {@link DetailedFault}. It is for people
 * diagnosing rejected data, and for a program that wants to branch on it, under this
 * contract:
 *
 * <ul>
 *   <li>a published code keeps its number and its meaning;
 *   <li>new codes may appear in a minor release, so a consumer tolerates a code it does not
 *       know: this record keeps the code of one this binding does not name, and
 *       {@link #name()} is {@code detail N};
 *   <li>the {@link CastFailure reason} stays the broad classification, and an unknown detail
 *       still leaves a usable reason and span.
 * </ul>
 *
 * <p>Open to additions for that reason, as {@link Keyword} is: a record with constants rather
 * than an enum, so code that branches on one needs a default arm. Compare with {@code equals},
 * or switch on {@link #name()}, where {@code javac} demands the {@code default}:
 *
 * {@snippet :
 * String why = switch (fault.detail().name()) {
 *     case "group_size", "group_misplaced" -> "thousands separators are wrong";
 *     case "too_large", "too_small" -> "out of range for the type";
 *     default -> "rejected: " + fault.detail();
 * };
 * }
 *
 * <p>The vocabulary is {@code corpus/formats.json}'s own and the Rust core's
 * {@code Detail}: the same names and numbers in every binding.
 *
 * @param code the native core's code, read as the unsigned 32-bit word it is: 1 {@code blank} to 32
 *     {@code not_a_scalar}, and any other value a later core publishes (a value above
 *     {@link Integer#MAX_VALUE} reads negative here, and 0, which a failure never carries, is kept
 *     the same way, as a detail this binding does not name)
 */
public record FaultDetail(int code) {
    /** {@code blank}: empty, or only white space. */
    public static final FaultDetail BLANK = new FaultDetail(1);

    /** {@code unexpected_character}: a character the position does not allow. */
    public static final FaultDetail UNEXPECTED_CHARACTER = new FaultDetail(2);

    /** {@code digit_expected}: a numeral or digit field is under way and another character, or the end, is found. */
    public static final FaultDetail DIGIT_EXPECTED = new FaultDetail(3);

    /** {@code delimiter_expected}: a required mark ({@code -}, {@code :}, {@code T}, {@code 0x}, {@code ;}) is missing or different. */
    public static final FaultDetail DELIMITER_EXPECTED = new FaultDetail(4);

    /** {@code trailing_text}: the value is complete, but more text follows. */
    public static final FaultDetail TRAILING_TEXT = new FaultDetail(5);

    /** {@code no_digits}: the numeral is absent entirely, only a sign, an affix or a bare radix prefix. */
    public static final FaultDetail NO_DIGITS = new FaultDetail(6);

    /** {@code sign_misplaced}: a sign where none may stand. */
    public static final FaultDetail SIGN_MISPLACED = new FaultDetail(7);

    /** {@code parentheses_unbalanced}: {@code (} without {@code )}, or {@code ( )}. */
    public static final FaultDetail PARENTHESES_UNBALANCED = new FaultDetail(8);

    /** {@code mark_repeated}: a once-only mark appears twice. */
    public static final FaultDetail MARK_REPEATED = new FaultDetail(9);

    /** {@code group_misplaced}: a group separator not between digits, or an empty group. */
    public static final FaultDetail GROUP_MISPLACED = new FaultDetail(10);

    /** {@code group_size}: a group with the wrong digit count under a strict {@link Grouping}. */
    public static final FaultDetail GROUP_SIZE = new FaultDetail(11);

    /** {@code group_in_fraction}: grouping after the decimal separator or in an exponent. */
    public static final FaultDetail GROUP_IN_FRACTION = new FaultDetail(12);

    /** {@code ambiguous_separator}: separator detection cannot tell {@code .} from {@code ,}. */
    public static final FaultDetail AMBIGUOUS_SEPARATOR = new FaultDetail(13);

    /** {@code fraction_not_allowed}: a fraction where the type is whole. */
    public static final FaultDetail FRACTION_NOT_ALLOWED = new FaultDetail(14);

    /** {@code field_width}: a delimited field has the wrong number of digits. */
    public static final FaultDetail FIELD_WIDTH = new FaultDetail(15);

    /** {@code wrong_length}: a fixed-shape token, recognised by its total length, has the wrong length. */
    public static final FaultDetail WRONG_LENGTH = new FaultDetail(16);

    /** {@code component_missing}: a required value-level part is absent. */
    public static final FaultDetail COMPONENT_MISSING = new FaultDetail(17);

    /** {@code not_recognized}: not one of the door's accepted words or values. */
    public static final FaultDetail NOT_RECOGNIZED = new FaultDetail(18);

    /** {@code unknown_unit}: an unknown or misplaced duration unit. */
    public static final FaultDetail UNKNOWN_UNIT = new FaultDetail(19);

    /** {@code invalid_utf8}: the bytes at the failure do not decode as UTF-8. */
    public static final FaultDetail INVALID_UTF8 = new FaultDetail(20);

    /** {@code too_long}: past an implementation limit. */
    public static final FaultDetail TOO_LONG = new FaultDetail(21);

    /** {@code not_finite}: a non-finite value. */
    public static final FaultDetail NOT_FINITE = new FaultDetail(22);

    /** {@code not_whole}: a non-integral double at a whole-number typed door. */
    public static final FaultDetail NOT_WHOLE = new FaultDetail(23);

    /** {@code negative_not_allowed}: negative where the type has no negatives. */
    public static final FaultDetail NEGATIVE_NOT_ALLOWED = new FaultDetail(24);

    /** {@code too_large}: the magnitude exceeds what the target type represents. */
    public static final FaultDetail TOO_LARGE = new FaultDetail(25);

    /** {@code too_small}: below a signed type's minimum. */
    public static final FaultDetail TOO_SMALL = new FaultDetail(26);

    /** {@code too_precise}: more fractional digits than the type holds. */
    public static final FaultDetail TOO_PRECISE = new FaultDetail(27);

    /** {@code date_field_out_of_range}: year 0, month 13, a day past the month's end. */
    public static final FaultDetail DATE_FIELD_OUT_OF_RANGE = new FaultDetail(28);

    /** {@code time_field_out_of_range}: an hour, minute, second or offset out of range. */
    public static final FaultDetail TIME_FIELD_OUT_OF_RANGE = new FaultDetail(29);

    /** {@code outside_window}: a representable instant or duration past the supported window. */
    public static final FaultDetail OUTSIDE_WINDOW = new FaultDetail(30);

    /** {@code nonexistent_date}: a day that never existed, Excel's phantom 1900-02-29. */
    public static final FaultDetail NONEXISTENT_DATE = new FaultDetail(31);

    /** {@code not_a_scalar}: a code point above U+10FFFF, or a surrogate. */
    public static final FaultDetail NOT_A_SCALAR = new FaultDetail(32);

    private static final String[] NAMES = {
        "blank",
        "unexpected_character",
        "digit_expected",
        "delimiter_expected",
        "trailing_text",
        "no_digits",
        "sign_misplaced",
        "parentheses_unbalanced",
        "mark_repeated",
        "group_misplaced",
        "group_size",
        "group_in_fraction",
        "ambiguous_separator",
        "fraction_not_allowed",
        "field_width",
        "wrong_length",
        "component_missing",
        "not_recognized",
        "unknown_unit",
        "invalid_utf8",
        "too_long",
        "not_finite",
        "not_whole",
        "negative_not_allowed",
        "too_large",
        "too_small",
        "too_precise",
        "date_field_out_of_range",
        "time_field_out_of_range",
        "outside_window",
        "nonexistent_date",
        "not_a_scalar"
    };

    /**
     * The detail whose native code is {@code code}. Never throws: a code this binding does not
     * name is kept as it is.
     *
     * @param code the native code, as the unsigned word the core wrote
     * @return the constant for a published code, otherwise a detail that keeps the code
     */
    public static FaultDetail of(int code) {
        return code >= 1 && code <= NAMES.length ? PUBLISHED[code - 1] : new FaultDetail(code);
    }

    /**
     * The detail whose published name, in the core's {@code snake_case}, is {@code name}.
     *
     * @param name the name, {@code group_size} say
     * @return the detail, or empty for a name this binding does not know
     */
    public static Optional<FaultDetail> fromName(String name) {
        for (int i = 0; i < NAMES.length; i++) {
            if (NAMES[i].equals(name)) {
                return Optional.of(PUBLISHED[i]);
            }
        }
        return Optional.empty();
    }

    /**
     * Whether this binding names the code: false for one a later core publishes.
     *
     * @return {@code true} for a published code this binding knows
     */
    public boolean isKnown() {
        return code >= 1 && code <= NAMES.length;
    }

    /**
     * The core's spelling of the detail.
     *
     * @return {@code blank}, {@code unexpected_character}, … {@code not_a_scalar}, or
     *     {@code detail N} for a code this binding does not name (N unsigned)
     */
    public String name() {
        return isKnown() ? NAMES[code - 1] : "detail " + Integer.toUnsignedString(code);
    }

    /**
     * Renders {@link #name()}.
     *
     * @return the core's spelling
     */
    @Override
    public String toString() {
        return name();
    }

    private static final FaultDetail[] PUBLISHED = {
        BLANK,
        UNEXPECTED_CHARACTER,
        DIGIT_EXPECTED,
        DELIMITER_EXPECTED,
        TRAILING_TEXT,
        NO_DIGITS,
        SIGN_MISPLACED,
        PARENTHESES_UNBALANCED,
        MARK_REPEATED,
        GROUP_MISPLACED,
        GROUP_SIZE,
        GROUP_IN_FRACTION,
        AMBIGUOUS_SEPARATOR,
        FRACTION_NOT_ALLOWED,
        FIELD_WIDTH,
        WRONG_LENGTH,
        COMPONENT_MISSING,
        NOT_RECOGNIZED,
        UNKNOWN_UNIT,
        INVALID_UTF8,
        TOO_LONG,
        NOT_FINITE,
        NOT_WHOLE,
        NEGATIVE_NOT_ALLOWED,
        TOO_LARGE,
        TOO_SMALL,
        TOO_PRECISE,
        DATE_FIELD_OUT_OF_RANGE,
        TIME_FIELD_OUT_OF_RANGE,
        OUTSIDE_WINDOW,
        NONEXISTENT_DATE,
        NOT_A_SCALAR
    };
}
