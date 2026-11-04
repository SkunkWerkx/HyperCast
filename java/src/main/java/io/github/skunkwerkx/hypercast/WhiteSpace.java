package io.github.skunkwerkx.hypercast;

import java.util.function.IntUnaryOperator;

/**
 * The core's notion of white space, restated once: the 25 code points with the Unicode
 * {@code White_Space} property (PropList.txt, Unicode 17.0.0), which every door trims at the
 * edges. U+200B, U+FEFF and U+180E are not among them. {@code corpus/formats.json}'s
 * {@code white_space} rows hold {@link #is(int)} to the core's table.
 */
final class WhiteSpace {
    private WhiteSpace() {}

    /** Whether {@code codePoint} has the {@code White_Space} property. */
    static boolean is(int codePoint) {
        return switch (codePoint) {
            case 0x0009, 0x000A, 0x000B, 0x000C, 0x000D, 0x0020, 0x0085, 0x00A0, 0x1680 -> true;
            case 0x2028, 0x2029, 0x202F, 0x205F, 0x3000 -> true;
            default -> codePoint >= 0x2000 && codePoint <= 0x200A;
        };
    }

    /** Whether any code point of {@code text} is white space. */
    static boolean contains(String text) {
        return text.codePoints().anyMatch(WhiteSpace::is);
    }

    /** Whether {@code text} starts or ends with white space; empty text does neither. */
    static boolean atEdge(String text) {
        return !text.isEmpty() && (is(text.codePointAt(0)) || is(text.codePointBefore(text.length())));
    }

    /**
     * The byte length of the white-space code point that starts the UTF-8 bytes
     * {@code [at, end)}, or {@code 0}; overlong and cut encodings match nothing, as in the
     * core.
     */
    static int lengthAt(IntUnaryOperator byteAt, int at, int end) {
        if (at >= end) {
            return 0;
        }
        int first = byteAt.applyAsInt(at) & 0xFF;
        if (first >= 0x09 && first <= 0x0D || first == 0x20) {
            return 1;
        }
        int width = first == 0xC2 ? 2 : first >= 0xE1 && first <= 0xE3 ? 3 : 0;
        if (width == 0 || end - at < width) {
            return 0;
        }
        return isEncoded(byteAt, at, width) ? width : 0;
    }

    /**
     * The byte length of the white-space code point that ends the UTF-8 bytes
     * {@code [start, end)}, or {@code 0}.
     */
    static int lengthBefore(IntUnaryOperator byteAt, int start, int end) {
        if (end <= start) {
            return 0;
        }
        int last = byteAt.applyAsInt(end - 1) & 0xFF;
        if (last >= 0x09 && last <= 0x0D || last == 0x20) {
            return 1;
        }
        for (int width = 2; width <= 3 && end - width >= start; width++) {
            if (isEncoded(byteAt, end - width, width)) {
                return width;
            }
        }
        return 0;
    }

    // Whether the `width` bytes from `at` are, exactly, the UTF-8 encoding of a non-ASCII
    // white-space code point.
    private static boolean isEncoded(IntUnaryOperator byteAt, int at, int width) {
        int b0 = byteAt.applyAsInt(at) & 0xFF;
        int b1 = byteAt.applyAsInt(at + 1) & 0xFF;
        if (width == 2) {
            return b0 == 0xC2 && (b1 == 0x85 || b1 == 0xA0);
        }
        int b2 = byteAt.applyAsInt(at + 2) & 0xFF;
        return switch (b0) {
            case 0xE1 -> b1 == 0x9A && b2 == 0x80;
            case 0xE2 ->
                b1 == 0x80 && (b2 >= 0x80 && b2 <= 0x8A || b2 == 0xA8 || b2 == 0xA9 || b2 == 0xAF)
                        || b1 == 0x81 && b2 == 0x9F;
            case 0xE3 -> b1 == 0x80 && b2 == 0x80;
            default -> false;
        };
    }
}
