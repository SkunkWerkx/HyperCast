package io.github.skunkwerkx.hypercast;

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

    int code() {
        return code;
    }
}
