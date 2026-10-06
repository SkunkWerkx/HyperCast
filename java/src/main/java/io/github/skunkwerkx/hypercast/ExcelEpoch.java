package io.github.skunkwerkx.hypercast;

import java.util.Optional;

/**
 * The date system an Excel serial number is expressed in. Spreadsheets carry no marker for
 * this — it is a workbook-level setting — so the caller states it, the same way
 * {@link UnixPrecision} and {@link DateOrder} are declared rather than guessed. Values match
 * the native core's discriminants.
 */
public enum ExcelEpoch {
    /**
     * The 1900 system (the Windows default): serial {@code 1} is 1900-01-01, and serial
     * {@code 60} is a February 29th that never existed.
     */
    Y1900(1),
    /**
     * The 1904 system (legacy Macintosh workbooks, still selectable today): serial
     * {@code 0} is 1904-01-01, with no phantom day anywhere in it.
     */
    Y1904(2);

    private final int code;

    ExcelEpoch(int code) {
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
     * @param code the discriminant: 1 the 1900 system, 2 the 1904 system
     * @return the constant, or empty for any other value
     */
    public static Optional<ExcelEpoch> fromCode(int code) {
        for (ExcelEpoch member : values()) {
            if (member.code == code) {
                return Optional.of(member);
            }
        }
        return Optional.empty();
    }
}
