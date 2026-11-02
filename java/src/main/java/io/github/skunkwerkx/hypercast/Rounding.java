package io.github.skunkwerkx.hypercast;

/**
 * How a value that is not a multiple of {@code multipleOf} is treated, judged on the exact
 * decimal the door read. Any mode but {@link #STRICT} needs a {@code multipleOf}; a rounded
 * success is marked {@link Checked#adjusted()}.
 */
public enum Rounding {
    /** A value that is not an exact multiple is a violation of {@code multipleOf}. The default. */
    STRICT(0),

    /** Rounds to the nearest multiple, ties to the even multiple (banker's rounding, IEEE 754's default). */
    MIDPOINT(1),

    /** Rounds to the multiple towards negative infinity. */
    FLOOR(2),

    /** Rounds to the multiple towards positive infinity. */
    CEILING(3);

    private final int code;

    Rounding(int code) {
        this.code = code;
    }

    /**
     * The native core's code for this mode.
     *
     * @return 0 strict, 1 midpoint, 2 floor, 3 ceiling
     */
    public int code() {
        return code;
    }
}
