package io.github.buvinghausen.hypercast;

/**
 * The closed set of reasons a cast can fail — the native core's verdict codes, verbatim.
 * Adding a member is a deliberate breaking change: every exhaustive switch over this enum
 * must be updated, in every binding at once.
 */
public enum CastFailure {
    /** Required input was empty or whitespace. The {@code *Optional} presentation surfaces this as absent. */
    EMPTY(1),

    /** Input was present but not recognizable as the target type. */
    MALFORMED(2),

    /**
     * Input was well-formed but the value falls outside the target's representable range —
     * {@code "256"} for a u8, a timestamp past 9999-12-31, {@code 1e400} for an f64.
     */
    OUT_OF_RANGE(3);

    private final int code;

    CastFailure(int code) {
        this.code = code;
    }

    /** The native verdict code ({@code 0} is "Ok" at the ABI and is never a failure). */
    public int code() {
        return code;
    }

    static CastFailure fromCode(int code) {
        return switch (code) {
            case 1 -> EMPTY;
            case 2 -> MALFORMED;
            case 3 -> OUT_OF_RANGE;
            default -> throw new IllegalStateException("libhypercast returned unknown verdict code " + code);
        };
    }
}
