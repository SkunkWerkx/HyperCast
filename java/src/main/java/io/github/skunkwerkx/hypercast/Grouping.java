package io.github.skunkwerkx.hypercast;

import java.util.Locale;

/**
 * How a {@link NumFormat} judges digit groups: the policy field the core reads from the
 * declared format, apart from the leniency {@code STYLE_*} flags (a style mask never carries
 * one). It matters only where {@link NumFormat#STYLE_GROUPING} lets a group separator in at
 * all; with that style off a group separator is refused under every policy.
 *
 * <p>Ungrouped numerals are valid under all three, and a numeral that uses a group separator
 * must conform in every group. Under a strict policy a group is judged as a group
 * ({@link FaultDetail#GROUP_SIZE}), counted in digits (native digits included), never in
 * bytes: no grouping after the decimal separator or in an exponent
 * ({@link FaultDetail#GROUP_IN_FRACTION}).
 *
 * <p>This is the 1.1 default's change: {@code 12,34} was the value 1234 under 1.0, which read a
 * group separator anywhere between two digits, and is {@link CastFailure#MALFORMED} under the
 * default {@link #WESTERN}. {@link #LEGACY} keeps 1.0's rule.
 */
public enum Grouping {
    /**
     * 1.0's rule: a group separator anywhere between two digits, whatever the group sizes.
     * What {@code NumFormat.INVARIANT} and every constructor declared before 1.1.
     */
    LEGACY(0),

    /**
     * Western: groups of exactly three digits counted leftward from the decimal separator (or
     * the end of the numeral); the leftmost group has one to three. {@code 1,234,567}. The
     * default of every {@link NumFormat} constructor and of {@link NumFormat#INVARIANT}.
     */
    WESTERN(1 << 8),

    /**
     * Indian, only when declared: the group next to the decimal separator has three digits,
     * every group to its left has two, the leftmost one or two. {@code 12,34,567}.
     */
    INDIAN(2 << 8);

    /** The policy field of the ABI's {@code flags} word, bits 8 and 9. */
    static final int POLICY_MASK = 3 << 8;

    private final int code;

    Grouping(int code) {
        this.code = code;
    }

    /**
     * This policy's bits in the native {@code flags} word: 0, {@code 1 << 8} or {@code 2 << 8}.
     * The core refuses {@code 3 << 8}.
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
     * @param bits the policy field alone: 0, {@code 1 << 8} or {@code 2 << 8}
     * @return the policy
     * @throws IllegalArgumentException for {@code 3 << 8}, which the core refuses, or any bit
     *     outside the policy field
     */
    public static Grouping of(int bits) {
        return switch (bits) {
            case 0 -> LEGACY;
            case 1 << 8 -> WESTERN;
            case 2 << 8 -> INDIAN;
            case 3 << 8 ->
                throw new IllegalArgumentException(
                        "Grouping policy 3 (bits 8 and 9 both set) names no grouping policy; the core refuses it");
            default ->
                throw new IllegalArgumentException("Grouping policy bits must be 0, 1 << 8 or 2 << 8; got 0x"
                        + Integer.toHexString(bits).toUpperCase(Locale.ROOT));
        };
    }
}
