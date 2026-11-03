package io.github.skunkwerkx.hypercast;

/**
 * The rule a {@link Violation} broke: the native core's code, and its OpenAPI spelling as
 * {@link #name()} ({@code exclusiveMinimum}, not {@code EXCLUSIVE_MINIMUM}).
 *
 * <p>Open to additions: {@code format} and {@code pattern} may join in a minor release, so a
 * keyword is a value with constants rather than an enum, and code that branches on one needs a
 * default arm. Compare with {@code equals}, or switch on {@link #name()}, where {@code javac}
 * demands the {@code default}:
 *
 * {@snippet :
 * String why = switch (violation.keyword().name()) {
 *     case "minimum", "maximum" -> "out of bounds";
 *     case "multipleOf" -> "not a whole step";
 *     default -> "breaks " + violation.keyword();
 * };
 * }
 *
 * <p>A keyword this binding does not name yet keeps its code, and its name is
 * {@code keyword N}.
 *
 * @param code the native core's code: 1 {@code minimum} to 8 {@code enum}
 */
public record Keyword(int code) {
    /** {@code minimum}: the value is below it. */
    public static final Keyword MINIMUM = new Keyword(1);

    /** {@code exclusiveMinimum}: the value is at or below it. */
    public static final Keyword EXCLUSIVE_MINIMUM = new Keyword(2);

    /** {@code maximum}: the value is above it. */
    public static final Keyword MAXIMUM = new Keyword(3);

    /** {@code exclusiveMaximum}: the value is at or above it. */
    public static final Keyword EXCLUSIVE_MAXIMUM = new Keyword(4);

    /** {@code multipleOf}: the value is not a multiple, under {@link Rounding#STRICT}. */
    public static final Keyword MULTIPLE_OF = new Keyword(5);

    /** {@code minLength}: the text has fewer code points. */
    public static final Keyword MIN_LENGTH = new Keyword(6);

    /** {@code maxLength}: the text has more code points. */
    public static final Keyword MAX_LENGTH = new Keyword(7);

    /** {@code enum}: the value matches no entry. */
    public static final Keyword ENUM = new Keyword(8);

    /** Refuses a code that names no keyword: 0 is "none" at the C ABI, and codes are positive. */
    public Keyword {
        if (code <= 0) {
            throw new IllegalArgumentException("keyword code " + code + " names no keyword");
        }
    }

    /**
     * The OpenAPI spelling.
     *
     * @return {@code minimum}, {@code exclusiveMinimum}, {@code maximum},
     *     {@code exclusiveMaximum}, {@code multipleOf}, {@code minLength}, {@code maxLength},
     *     {@code enum}, or {@code keyword N} for a code this binding does not name
     */
    public String name() {
        return switch (code) {
            case 1 -> "minimum";
            case 2 -> "exclusiveMinimum";
            case 3 -> "maximum";
            case 4 -> "exclusiveMaximum";
            case 5 -> "multipleOf";
            case 6 -> "minLength";
            case 7 -> "maxLength";
            case 8 -> "enum";
            default -> "keyword " + code;
        };
    }

    /**
     * Renders {@link #name()}.
     *
     * @return the OpenAPI spelling
     */
    @Override
    public String toString() {
        return name();
    }
}
