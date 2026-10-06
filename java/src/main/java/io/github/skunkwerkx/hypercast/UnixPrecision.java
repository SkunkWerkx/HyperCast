package io.github.skunkwerkx.hypercast;

import java.util.Optional;

/**
 * The declared unit of a Unix-epoch value. There is no magnitude guessing — the caller
 * states the unit, so a bare number is never silently interpreted as seconds or
 * milliseconds. Values match the native core's discriminants.
 */
public enum UnixPrecision {
    /** Seconds since 1970-01-01T00:00:00Z. */
    SECONDS(1),
    /** Milliseconds since the epoch. */
    MILLISECONDS(2),
    /** Microseconds since the epoch. */
    MICROSECONDS(3),
    /** Nanoseconds since the epoch. */
    NANOSECONDS(4);

    private final int code;

    UnixPrecision(int code) {
        this.code = code;
    }

    /**
     * The native core's discriminant for this constant — what a library declaring this
     * option across its own C ABI passes.
     *
     * @return the ABI discriminant
     */
    public int code() {
        return code;
    }

    /**
     * The constant whose ABI discriminant is {@code code}.
     *
     * @param code the discriminant: 1 seconds … 4 nanoseconds
     * @return the constant, or empty for any other value
     */
    public static Optional<UnixPrecision> fromCode(int code) {
        for (UnixPrecision member : values()) {
            if (member.code == code) {
                return Optional.of(member);
            }
        }
        return Optional.empty();
    }
}
