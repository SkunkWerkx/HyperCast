package io.github.skunkwerkx.hypercast;

import java.util.Optional;

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

    /**
     * The native verdict code ({@code 0} is "Ok" at the ABI and is never a failure).
     *
     * @return the native failure code this constant maps
     */
    public int code() {
        return code;
    }

    /**
     * The reason whose native verdict code is {@code code}.
     *
     * @param code the verdict code: 1 empty, 2 malformed, 3 out of range
     * @return the reason, or empty for any other value — {@code 0}, success, included
     */
    public static Optional<CastFailure> fromCode(int code) {
        return switch (code) {
            case 1 -> Optional.of(EMPTY);
            case 2 -> Optional.of(MALFORMED);
            case 3 -> Optional.of(OUT_OF_RANGE);
            default -> Optional.empty();
        };
    }
}
