package io.github.skunkwerkx.hypercast;

import java.util.Locale;

/**
 * How a {@link NumFormat} judges digit groups: the policy field the core reads from the
 * declared format, apart from the leniency {@code STYLE_*} flags (a style mask never carries
 * one). It matters only where {@link NumFormat#STYLE_GROUPING} lets a group separator in at
 * all; with that style off a group separator is refused under every policy.
 *
 * <p>Ungrouped numerals are valid under both, and a numeral that uses a group separator must
 * conform in every group. A group is judged as a group ({@link FaultDetail#GROUP_SIZE}), counted
 * in digits (native digits included), never in bytes, and a first group can't start with a zero:
 * no grouping after the decimal separator or in an exponent ({@link FaultDetail#GROUP_IN_FRACTION}).
 *
 * <p>{@code 1,23,456} and {@code 12,34} were values under 1.0, which read a group separator
 * anywhere between two digits, and are {@link CastFailure#MALFORMED} under {@link #WESTERN} now;
 * declare {@link #INDIAN} for {@code 1,23,456}. Shapes neither policy reads: {@code docs/grouping.md}.
 */
public enum Grouping {
    /**
     * Western: groups of exactly three digits counted leftward from the decimal separator (or
     * the end of the numeral); the leftmost group has one to three and doesn't start with a
     * zero. {@code 1,234,567} reads and {@code 0,125} is {@link CastFailure#MALFORMED}. The
     * default of every {@link NumFormat} constructor and of {@link NumFormat#INVARIANT}.
     */
    WESTERN(0),

    /**
     * Indian, only when declared: the group next to the decimal separator has three digits,
     * every group to its left has two, the leftmost one or two. {@code 12,34,567}.
     */
    INDIAN(2 << 8);

    /**
     * The policy field of the ABI's {@code flags} word, bits 8 and 9. Bit 8 is retired permanently
     * and never reused, so {@code 1 << 8} and {@code 3 << 8} name nothing; a future policy takes a
     * new bit from bit 10 up.
     */
    static final int POLICY_MASK = 3 << 8;

    private final int code;

    Grouping(int code) {
        this.code = code;
    }

    /**
     * This policy's bits in the native {@code flags} word: 0 or {@code 2 << 8}.
     *
     * @return the ABI policy bits
     */
    public int code() {
        return code;
    }

    /**
     * The policy whose ABI bits are {@code bits}, for a library that holds a {@code flags}
     * word from another source (a corpus, a configuration) and reads its policy field.
     *
     * @param bits the policy field alone: 0 or {@code 2 << 8}
     * @return the policy
     * @throws IllegalArgumentException for {@code 1 << 8} and {@code 3 << 8}, which hold the retired
     *     bit and the core refuses, or any bit outside the policy field
     */
    public static Grouping of(int bits) {
        return switch (bits) {
            case 0 -> WESTERN;
            case 2 << 8 -> INDIAN;
            case 1 << 8, 3 << 8 ->
                throw new IllegalArgumentException(
                        "Grouping policy bits hold bit 8, which names no grouping policy; the core refuses them");
            default ->
                throw new IllegalArgumentException("Grouping policy bits must be 0 or 2 << 8; got 0x"
                        + Integer.toHexString(bits).toUpperCase(Locale.ROOT));
        };
    }
}
