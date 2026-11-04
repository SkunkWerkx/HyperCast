package io.github.skunkwerkx.hypercast;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;

/**
 * A caller-built month vocabulary for {@link DetailedCast}'s date doors: the names a month
 * field may be written in ({@code Jan}, {@code janvier}), Excel style only. A declared name
 * replaces the numeric month field in the position the declared {@link DateOrder} selects,
 * with the same repeated {@code /}, {@code -} or {@code .} separator: {@code 07-Jan-26} reads
 * under day-month-year with a {@link DateOptions#pivot() pivot}, and {@code 7-Jan-2026}
 * without one. Prose layouts, optional commas and reordering are not read, and a vocabulary
 * never overrides the declared order. The vocabulary is independent of {@link DateOptions}: it
 * may accompany {@link DateOptions#NONE}, which enables month names and nothing else.
 *
 * <p>Matching folds case with simple case folding, as a text {@code enum} does; the longest
 * declared entry wins among those followed by the required separator. Aliases are separate
 * entries ({@code Sep}, {@code Sept}). Punctuation is part of the name and never stripped:
 * with {@code janv.} declared and {@code .} as the separator, {@code 7.janv..2026} reads and
 * {@code 7.janv.2026} is {@link CastFailure#MALFORMED} unless {@code janv} is declared too.
 *
 * <p>The platform builds none, in this release: aliases, abbreviations and grammatical forms
 * ({@code janvier} against {@code de janeiro}) are the caller's decisions.
 *
 * <p>Validated up front by the core's own rules, so a refused vocabulary fails loudly as the
 * caller bug it is, never as a verdict: each entry maps to a month 1 to 12; every month has at
 * least one entry; no entry is empty, longer than 64 UTF-8 bytes, or contains an ASCII digit,
 * {@code /}, {@code -} or any white space ({@code .} is allowed anywhere); and no two entries
 * fold to the same spelling, even for the same month.
 */
public final class MonthNames {
    private static final int MAX_NAME_BYTES = 64;

    private final byte[] packed;
    private final ByteBuffer vocabulary;

    private MonthNames(byte[] packed) {
        boolean[] present = validate(packed);
        // Simple case folding is the core's table; Java restates none of it, so a folded
        // duplicate is the one rule the core judges, once, here. The core judges a duplicate
        // entry by entry, before it asks that every month has a name, so a vocabulary short of
        // a month is judged completed with a name no entry can fold to: the core then refuses
        // only for a duplicate, which wins over the missing month, as it does there.
        byte[] judged = complete(packed, present);
        ByteBuffer judging = ByteBuffer.allocateDirect(judged.length).put(0, judged);
        if (NativeCore.monthNamesCheck(judging) != 1) {
            throw new IllegalArgumentException(
                    "Two month names fold to the same spelling (simple case folding), even for the same month");
        }
        for (int month = 1; month <= 12; month++) {
            if (!present[month]) {
                throw new IllegalArgumentException("Every month needs at least one name; month " + month + " has none");
            }
        }
        this.packed = packed;
        this.vocabulary = judging;
    }

    // The vocabulary with one stand-in entry for each month that has none: a control character
    // and a letter, which no declared name can fold to (no letter that case folding maps from
    // another character, K and s, is used) unless it starts with that same control character,
    // and then the next one is tried.
    private static byte[] complete(byte[] packed, boolean[] present) {
        boolean missing = false;
        for (int month = 1; month <= 12; month++) {
            missing |= !present[month];
        }
        if (!missing) {
            return packed;
        }
        String taken = decodedLead(packed);
        char lead = 1;
        while (lead < 32 && taken.indexOf(lead) >= 0) {
            lead++;
        }
        if (lead == 32) {
            // Every control character leads a declared name: no stand-in is safe, so the
            // vocabulary is judged as it is.
            return packed;
        }
        java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
        out.writeBytes(packed);
        String letters = "abcdefghijlm";
        for (int month = 1; month <= 12; month++) {
            if (!present[month]) {
                byte[] name = (String.valueOf(lead) + letters.charAt(month - 1)).getBytes(StandardCharsets.UTF_8);
                out.write(month);
                out.writeBytes(ByteBuffer.allocate(4)
                        .order(ByteOrder.LITTLE_ENDIAN)
                        .putInt(name.length)
                        .array());
                out.writeBytes(name);
            }
        }
        return out.toByteArray();
    }

    // The leading character of every declared name, for choosing a stand-in lead no name uses.
    private static String decodedLead(byte[] packed) {
        StringBuilder leads = new StringBuilder();
        ByteBuffer in = ByteBuffer.wrap(packed).order(ByteOrder.LITTLE_ENDIAN);
        while (in.remaining() >= 5) {
            in.get();
            int length = in.getInt();
            if (length > 0 && length <= in.remaining()) {
                leads.append((char) (in.get(in.position()) & 0xFF));
            }
            in.position(in.position() + length);
        }
        return leads.toString();
    }

    /**
     * A builder for a vocabulary: {@link Builder#add(int, String)} one entry per spelling.
     *
     * @return an empty builder
     */
    public static Builder builder() {
        return new Builder();
    }

    /**
     * A vocabulary from the core's packed layout, for a library that keeps one already: each
     * entry is a month byte (1 to 12), a little-endian {@code u32} byte length, then that many
     * UTF-8 bytes, with no padding.
     *
     * @param packed the packed entries, copied
     * @return the validated vocabulary
     * @throws IllegalArgumentException for a layout or entry the core refuses
     * @throws NullPointerException if {@code packed} is null
     */
    public static MonthNames ofPacked(byte[] packed) {
        return new MonthNames(Objects.requireNonNull(packed, "packed").clone());
    }

    /**
     * The vocabulary in the core's packed layout, as {@link #ofPacked(byte[])} takes it.
     *
     * @return a copy of the packed entries
     */
    public byte[] packed() {
        return packed.clone();
    }

    /** The vocabulary in native memory, for the native calls; never handed out. */
    ByteBuffer vocabulary() {
        return vocabulary;
    }

    /**
     * The vocabulary's layout is its identity.
     *
     * @param other the other object
     * @return {@code true} for a vocabulary with the same packed entries
     */
    @Override
    public boolean equals(Object other) {
        return other instanceof MonthNames names && Arrays.equals(packed, names.packed);
    }

    /**
     * Hashes the packed entries.
     *
     * @return the hash
     */
    @Override
    public int hashCode() {
        return Arrays.hashCode(packed);
    }

    /**
     * Describes the vocabulary by its size.
     *
     * @return {@code MonthNames[N bytes]}
     */
    @Override
    public String toString() {
        return "MonthNames[" + packed.length + " bytes]";
    }

    /** Collects entries for {@link MonthNames}; not thread-safe. */
    public static final class Builder {
        private final java.io.ByteArrayOutputStream entries = new java.io.ByteArrayOutputStream();

        private Builder() {}

        /**
         * Adds one spelling of a month.
         *
         * @param month the month, 1 (January) to 12
         * @param name the spelling, punctuation included; a lone surrogate is invalid UTF-8
         * @return this builder
         * @throws IllegalArgumentException if {@code month} is not 1 to 12
         * @throws NullPointerException if {@code name} is null
         */
        public Builder add(int month, String name) {
            if (month < 1 || month > 12) {
                throw monthRefusal(month);
            }
            byte[] utf8 = Cast.wtf8(Objects.requireNonNull(name, "name"));
            entries.write(month);
            entries.writeBytes(ByteBuffer.allocate(4)
                    .order(ByteOrder.LITTLE_ENDIAN)
                    .putInt(utf8.length)
                    .array());
            entries.writeBytes(utf8);
            return this;
        }

        /**
         * Builds the vocabulary, judging it by the core's own rules.
         *
         * @return the validated vocabulary
         * @throws IllegalArgumentException for a vocabulary the core would refuse: a month
         *     without an entry, an entry that is empty, too long, or holds a digit, {@code /},
         *     {@code -} or white space, or two entries that fold to the same spelling
         */
        public MonthNames build() {
            return new MonthNames(entries.toByteArray());
        }
    }

    private static IllegalArgumentException monthRefusal(int month) {
        return new IllegalArgumentException("A month name's month must be 1 to 12; got " + month);
    }

    // The core's rules in its order, entry by entry (layout, month, UTF-8, length, character),
    // and which months have a name. The two rules left, the folded duplicate and then the month
    // without a name, are judged by the constructor, in the core's order.
    private static boolean[] validate(byte[] packed) {
        boolean[] present = new boolean[13];
        ByteBuffer in = ByteBuffer.wrap(packed).order(ByteOrder.LITTLE_ENDIAN);
        while (in.hasRemaining()) {
            if (in.remaining() < 5) {
                throw layoutRefusal();
            }
            int month = in.get() & 0xFF;
            long length = Integer.toUnsignedLong(in.getInt());
            if (length > in.remaining()) {
                throw layoutRefusal();
            }
            byte[] utf8 = new byte[(int) length];
            in.get(utf8);
            if (month < 1 || month > 12) {
                throw monthRefusal(month);
            }
            String name;
            try {
                name = StandardCharsets.UTF_8
                        .newDecoder()
                        .onMalformedInput(CodingErrorAction.REPORT)
                        .onUnmappableCharacter(CodingErrorAction.REPORT)
                        .decode(ByteBuffer.wrap(utf8))
                        .toString();
            } catch (CharacterCodingException e) {
                throw new IllegalArgumentException("A month name must be valid UTF-8", e);
            }
            if (utf8.length == 0 || utf8.length > MAX_NAME_BYTES) {
                throw new IllegalArgumentException(
                        "A month name must be 1 to " + MAX_NAME_BYTES + " UTF-8 bytes; it is " + utf8.length);
            }
            name.codePoints().forEach(point -> {
                if (point >= '0' && point <= '9' || point == '/' || point == '-' || WhiteSpace.is(point)) {
                    throw new IllegalArgumentException(
                            "A month name must not contain an ASCII digit, '/', '-' or white space; got '" + name
                                    + "'");
                }
            });
            present[month] = true;
        }
        return present;
    }

    private static IllegalArgumentException layoutRefusal() {
        return new IllegalArgumentException("The packed month vocabulary is malformed: an entry is a month byte, a "
                + "little-endian u32 length and that many UTF-8 bytes, with nothing cut short or left over");
    }
}
