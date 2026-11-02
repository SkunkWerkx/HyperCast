package io.github.skunkwerkx.hypercast;

import io.github.skunkwerkx.hypercast.interop.NativeValues;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.Optional;
import java.util.OptionalInt;

/**
 * A declaration of validation constraints, the OpenAPI Schema Object's keywords, bound to one
 * door: {@code Constraints.f64()} builds one for {@link Cast#f64(String, NumFormat, Constraints)},
 * {@code Constraints.text()} one for {@link Cast#text(String, Constraints)}. Immutable, and
 * judged once, by the native core, when it is built: a declaration that cannot mean anything at
 * its door is refused there as a caller bug, and every checked cast after that only judges data.
 *
 * {@snippet :
 * // Latitude: a multiple of 0.000001 between -90 and 90, finer input rounded half to even.
 * Constraints<Double> latitude = Constraints.f64()
 *         .minimum("-90").maximum("90")
 *         .multipleOf("0.000001").rounding(Rounding.MIDPOINT)
 *         .build();
 * }
 *
 * <p>The integer doors ({@link #i8()} to {@link #u64()}) take {@code minimum},
 * {@code exclusiveMinimum}, {@code maximum}, {@code exclusiveMaximum}, {@code multipleOf} with a
 * {@link Rounding}, and {@code enum}; {@link #f32()}, {@link #f64()} and {@link #decimal()} the
 * same but {@code enum}; {@link #text()} {@code minLength}, {@code maxLength}, {@code trim} and
 * {@code enum}. Every builder offers every keyword, and the core alone refuses one its door does
 * not take. Bounds and steps are a {@link BigDecimal} or its text in invariant notation
 * ({@code "0.000001"}, {@code "-90"}, {@code "1e-6"}), converted exactly by the core's own
 * conversion, never a {@code double}, so {@code 0.1} cannot be declared inexactly.
 *
 * <p>The value is judged in the core, in this order, the first failure winning: the door parses
 * (a {@link Fault}, exactly as the plain door's); text is trimmed of Unicode
 * {@code White_Space} when {@code trim} says so; {@code multipleOf}, on the exact decimal,
 * rounding when declared; the bounds, on the rounded value; the lengths, in code points;
 * {@code enum} (text under Unicode simple case folding, delivering the declared entry). A broken
 * rule is a {@link Violation}.
 *
 * <p>A declaration is bound to the door it was built for. The type parameter catches most
 * mismatches at compile time; doors that share a Java type ({@code u8}, {@code u16} and
 * {@code i32} all deliver an {@link Integer}) are told apart when the cast is made, and handing a
 * declaration to another door is an {@link IllegalArgumentException}.
 *
 * @param <T> the door's value type
 */
public final class Constraints<T> {
    // The core's door codes, which the checked exports take.
    static final int I8 = 2;
    static final int I16 = 3;
    static final int I32 = 4;
    static final int I64 = 5;
    static final int U8 = 6;
    static final int U16 = 7;
    static final int U32 = 8;
    static final int U64 = 9;
    static final int F32 = 10;
    static final int F64 = 11;
    static final int TEXT = 18;
    static final int DECIMAL = 19;

    // The layout and one packed integer entry, as interop's NativeValues sizes them.
    private static final int RAW_BYTES = (int) NativeValues.CONSTRAINTS_BYTES;
    private static final int DECIMAL_BYTES = (int) NativeValues.DECIMAL_BYTES;

    // The flag bits, restated from the header; package-private so the suite pins them to it.
    static final int HAS_MINIMUM = 1;
    static final int HAS_EXCLUSIVE_MINIMUM = 2;
    static final int HAS_MAXIMUM = 4;
    static final int HAS_EXCLUSIVE_MAXIMUM = 8;
    static final int HAS_MULTIPLE_OF = 16;
    static final int HAS_MIN_LENGTH = 32;
    static final int HAS_MAX_LENGTH = 64;
    static final int TRIM = 128;
    static final int HAS_ENUM = 256;

    private final int door;
    private final String doorName;
    private final byte[] raw;
    private final byte[] packed;
    private final ByteBuffer entries;
    private final String[] texts;
    private final BigDecimal minimum;
    private final BigDecimal exclusiveMinimum;
    private final BigDecimal maximum;
    private final BigDecimal exclusiveMaximum;
    private final BigDecimal multipleOf;
    private final Rounding rounding;
    private final Integer minLength;
    private final Integer maxLength;
    private final boolean trim;

    private Constraints(Builder<T> builder) {
        door = builder.door;
        doorName = builder.doorName;
        rounding = builder.rounding;
        minLength = builder.minLength;
        maxLength = builder.maxLength;
        trim = builder.trim;
        minimum = declared(builder.minimum);
        exclusiveMinimum = declared(builder.exclusiveMinimum);
        maximum = declared(builder.maximum);
        exclusiveMaximum = declared(builder.exclusiveMaximum);
        multipleOf = declared(builder.multipleOf);
        int flags = (minimum != null ? HAS_MINIMUM : 0)
                | (exclusiveMinimum != null ? HAS_EXCLUSIVE_MINIMUM : 0)
                | (maximum != null ? HAS_MAXIMUM : 0)
                | (exclusiveMaximum != null ? HAS_EXCLUSIVE_MAXIMUM : 0)
                | (multipleOf != null ? HAS_MULTIPLE_OF : 0)
                | (minLength != null ? HAS_MIN_LENGTH : 0)
                | (maxLength != null ? HAS_MAX_LENGTH : 0)
                | (trim ? TRIM : 0)
                | (builder.entries != null ? HAS_ENUM : 0);
        raw = new byte[RAW_BYTES];
        ByteBuffer layout = ByteBuffer.wrap(raw).order(ByteOrder.nativeOrder());
        layout.putInt(RAW_BYTES)
                .putInt(flags)
                .putInt(rounding.code())
                .putInt(minLength != null ? minLength : 0)
                .putInt(maxLength != null ? maxLength : 0)
                .putInt(0);
        pack(minimum != null ? minimum : exclusiveMinimum, layout);
        pack(maximum != null ? maximum : exclusiveMaximum, layout);
        pack(multipleOf, layout);
        if (builder.entries == null) {
            packed = new byte[0];
            texts = null;
        } else if (door == TEXT) {
            texts = new String[builder.entries.size()];
            List<byte[]> encoded = new ArrayList<>(texts.length);
            int size = 0;
            for (int i = 0; i < texts.length; i++) {
                if (!(builder.entries.get(i) instanceof String entry)) {
                    throw new IllegalArgumentException("Text enum entries are strings; entry " + i + " is the number "
                            + builder.entries.get(i) + ".");
                }
                texts[i] = entry;
                byte[] utf8 = Cast.wtf8(entry);
                encoded.add(utf8);
                size = Math.addExact(size, Math.addExact(4, utf8.length));
            }
            packed = new byte[size];
            ByteBuffer list = ByteBuffer.wrap(packed).order(ByteOrder.LITTLE_ENDIAN);
            for (byte[] utf8 : encoded) {
                list.putInt(utf8.length).put(utf8);
            }
        } else {
            texts = null;
            packed = new byte[Math.multiplyExact(builder.entries.size(), DECIMAL_BYTES)];
            ByteBuffer list = ByteBuffer.wrap(packed).order(ByteOrder.nativeOrder());
            for (Object entry : builder.entries) {
                pack(declared(entry.toString()), list);
            }
        }
        // The list lives in native memory for as long as this declaration does, so neither path
        // copies or pins it on a call: JNI reads its address, FFM wraps it as a segment.
        entries = packed.length == 0
                ? null
                : ByteBuffer.allocateDirect(packed.length).put(0, packed);
        int code = NativeCore.constraintsCheck(door, raw, entries);
        if (code != 0) {
            throw new IllegalArgumentException(refusal(code));
        }
    }

    /**
     * Declares constraints for {@link Cast#i8(String, NumFormat, Constraints)}.
     *
     * @return a builder for the {@code i8} door
     */
    public static Builder<Byte> i8() {
        return new Builder<>(I8, "i8");
    }

    /**
     * Declares constraints for {@link Cast#i16(String, NumFormat, Constraints)}.
     *
     * @return a builder for the {@code i16} door
     */
    public static Builder<Short> i16() {
        return new Builder<>(I16, "i16");
    }

    /**
     * Declares constraints for {@link Cast#i32(String, NumFormat, Constraints)}.
     *
     * @return a builder for the {@code i32} door
     */
    public static Builder<Integer> i32() {
        return new Builder<>(I32, "i32");
    }

    /**
     * Declares constraints for {@link Cast#i64(String, NumFormat, Constraints)}.
     *
     * @return a builder for the {@code i64} door
     */
    public static Builder<Long> i64() {
        return new Builder<>(I64, "i64");
    }

    /**
     * Declares constraints for {@link Cast#u8(String, NumFormat, Constraints)}.
     *
     * @return a builder for the {@code u8} door
     */
    public static Builder<Integer> u8() {
        return new Builder<>(U8, "u8");
    }

    /**
     * Declares constraints for {@link Cast#u16(String, NumFormat, Constraints)}.
     *
     * @return a builder for the {@code u16} door
     */
    public static Builder<Integer> u16() {
        return new Builder<>(U16, "u16");
    }

    /**
     * Declares constraints for {@link Cast#u32(String, NumFormat, Constraints)}.
     *
     * @return a builder for the {@code u32} door
     */
    public static Builder<Long> u32() {
        return new Builder<>(U32, "u32");
    }

    /**
     * Declares constraints for {@link Cast#u64(String, NumFormat, Constraints)}, whose value is
     * the two's-complement bit pattern of the unsigned 64-bit number, as the plain door's is.
     *
     * @return a builder for the {@code u64} door
     */
    public static Builder<Long> u64() {
        return new Builder<>(U64, "u64");
    }

    /**
     * Declares constraints for {@link Cast#f32(String, NumFormat, Constraints)}.
     *
     * @return a builder for the {@code f32} door
     */
    public static Builder<Float> f32() {
        return new Builder<>(F32, "f32");
    }

    /**
     * Declares constraints for {@link Cast#f64(String, NumFormat, Constraints)}.
     *
     * @return a builder for the {@code f64} door
     */
    public static Builder<Double> f64() {
        return new Builder<>(F64, "f64");
    }

    /**
     * Declares constraints for {@link Cast#decimal(String, NumFormat, Constraints)}.
     *
     * @return a builder for the {@code decimal} door
     */
    public static Builder<BigDecimal> decimal() {
        return new Builder<>(DECIMAL, "decimal");
    }

    /**
     * Declares constraints for {@link Cast#text(String, Constraints)}.
     *
     * @return a builder for the text door
     */
    public static Builder<String> text() {
        return new Builder<>(TEXT, "text");
    }

    /**
     * The declared {@code minimum}, as the core converted it (trailing zeros dropped).
     *
     * @return the minimum, or empty
     */
    public Optional<BigDecimal> minimum() {
        return Optional.ofNullable(minimum);
    }

    /**
     * The declared {@code exclusiveMinimum}, as the core converted it.
     *
     * @return the exclusive minimum, or empty
     */
    public Optional<BigDecimal> exclusiveMinimum() {
        return Optional.ofNullable(exclusiveMinimum);
    }

    /**
     * The declared {@code maximum}, as the core converted it.
     *
     * @return the maximum, or empty
     */
    public Optional<BigDecimal> maximum() {
        return Optional.ofNullable(maximum);
    }

    /**
     * The declared {@code exclusiveMaximum}, as the core converted it.
     *
     * @return the exclusive maximum, or empty
     */
    public Optional<BigDecimal> exclusiveMaximum() {
        return Optional.ofNullable(exclusiveMaximum);
    }

    /**
     * The declared {@code multipleOf}, as the core converted it.
     *
     * @return the step, or empty
     */
    public Optional<BigDecimal> multipleOf() {
        return Optional.ofNullable(multipleOf);
    }

    /**
     * What a value that is not a multiple of {@link #multipleOf()} becomes.
     *
     * @return the rounding, {@link Rounding#STRICT} unless declared
     */
    public Rounding rounding() {
        return rounding;
    }

    /**
     * The declared {@code minLength}, in code points.
     *
     * @return the fewest code points, or empty
     */
    public OptionalInt minLength() {
        return minLength != null ? OptionalInt.of(minLength) : OptionalInt.empty();
    }

    /**
     * The declared {@code maxLength}, in code points.
     *
     * @return the most code points, or empty
     */
    public OptionalInt maxLength() {
        return maxLength != null ? OptionalInt.of(maxLength) : OptionalInt.empty();
    }

    /**
     * Whether text is trimmed of Unicode {@code White_Space} before it is judged.
     *
     * @return {@code true} when {@code trim} was declared
     */
    public boolean trim() {
        return trim;
    }

    /**
     * The declaration in the native core's 72-byte {@code RawConstraints} layout, in this
     * platform's byte order, for a library that judges values across a C ABI of its own; the
     * {@code enum} list travels beside it, as {@link #packedEnum()}.
     *
     * @return a copy of the layout's bytes
     */
    public byte[] toRaw() {
        return raw.clone();
    }

    /**
     * The {@code enum} list as the native core reads it beside {@link #toRaw()} (whose flag
     * {@code 256} says whether one was declared), empty for none:
     * at an integer door, 16-byte {@code Decimal}s in this platform's byte order; at the text
     * door, each entry a little-endian 32-bit byte length followed by its UTF-8 bytes.
     *
     * @return a copy of the list's bytes
     */
    public byte[] packedEnum() {
        return packed.clone();
    }

    /** The layout itself, for the native calls; never handed out. */
    byte[] raw() {
        return raw;
    }

    /** The {@code enum} list in native memory, or {@code null} for none. */
    ByteBuffer entries() {
        return entries;
    }

    /**
     * Refuses this declaration at any door but {@code door}: a caller handing one door's
     * constraints to another.
     */
    Constraints<T> require(int door, String name) {
        if (this.door != door) {
            throw new IllegalArgumentException(
                    "Constraints built for the " + doorName + " door were passed to the " + name + " door");
        }
        return this;
    }

    /** The declared text entry at {@code index}, the caller's own string. */
    String entry(int index) {
        if (texts == null || index < 0 || index >= texts.length) {
            throw new IllegalStateException("libhypercast matched enum entry " + index
                    + ", which was never declared — a binding bug, please report it");
        }
        return texts[index];
    }

    // A bound, step or integer entry's text through the core's own conversion
    // (cast_declared_decimal): the exact decimal, or the core's refusal 8.
    private static BigDecimal declared(String text) {
        if (text == null) {
            return null;
        }
        NativeCore.Scratch scratch = NativeCore.scratch();
        int code = NativeCore.declaredDecimal(scratch, Cast.wtf8(text));
        return switch (code) {
            case 0 -> NativeCore.decimal(scratch);
            case 8 -> throw new IllegalArgumentException(refusal(8));
            default ->
                throw new IllegalStateException(
                        "libhypercast reported a contract violation — a binding bug, please report it");
        };
    }

    // A decimal's 16 bytes, {lo, hi, scale, negative} and two of padding, at the buffer's
    // position: the triple the core converted, as it handed it over. Absent is all zero.
    private static void pack(BigDecimal value, ByteBuffer into) {
        if (value == null) {
            into.putLong(0).putInt(0).putInt(0);
            return;
        }
        BigInteger magnitude = value.unscaledValue().abs();
        into.putLong(magnitude.longValue())
                .putInt(magnitude.shiftRight(64).intValue())
                .put((byte) value.scale())
                .put((byte) (value.signum() < 0 ? 1 : 0))
                .putShort((short) 0);
    }

    // One message per refusal code, worded as the Rust core's Refusal displays it.
    static String refusal(int code) {
        String rule = switch (code) {
            case 1 -> "a constraint the door does not take";
            case 2 -> "minimum together with exclusiveMinimum";
            case 3 -> "maximum together with exclusiveMaximum";
            case 4 -> "the lower bound is not below the upper bound";
            case 5 -> "multipleOf is not positive";
            case 6 -> "a rounding without multipleOf";
            case 7 -> "minLength is above maxLength";
            case 8 -> "a bound, step or enum entry the door cannot hold";
            case 9 -> "an enum entry that is not UTF-8";
            case 10 -> "a duplicate enum entry";
            case 11 -> "a malformed constraints layout";
            case 12 -> "an enum with no entries";
            case 13 -> "a text enum entry no input can match";
            default -> "a rule this binding does not name";
        };
        return "Constraints refused (code " + code + "): " + rule + ".";
    }

    /**
     * Collects one door's keywords; {@link #build()} has the core judge them. Every keyword is
     * offered at every door, so the core alone decides which a door takes. Setting a keyword
     * twice keeps the last value. Not thread-safe; the {@link Constraints} it builds is
     * immutable and safe to share.
     *
     * @param <T> the door's value type
     */
    public static final class Builder<T> {
        private final int door;
        private final String doorName;
        private String minimum;
        private String exclusiveMinimum;
        private String maximum;
        private String exclusiveMaximum;
        private String multipleOf;
        private Rounding rounding = Rounding.STRICT;
        private Integer minLength;
        private Integer maxLength;
        private boolean trim;
        private List<Object> entries;

        private Builder(int door, String doorName) {
            this.door = door;
            this.doorName = doorName;
        }

        /**
         * The smallest value allowed.
         *
         * @param value an exact decimal
         * @return this builder
         */
        public Builder<T> minimum(BigDecimal value) {
            minimum = Objects.requireNonNull(value, "minimum").toString();
            return this;
        }

        /**
         * The smallest value allowed.
         *
         * @param value a decimal in invariant notation ({@code "-90"}, {@code "1e-6"})
         * @return this builder
         */
        public Builder<T> minimum(String value) {
            minimum = Objects.requireNonNull(value, "minimum");
            return this;
        }

        /**
         * A value every value must be above.
         *
         * @param value an exact decimal
         * @return this builder
         */
        public Builder<T> exclusiveMinimum(BigDecimal value) {
            exclusiveMinimum = Objects.requireNonNull(value, "exclusiveMinimum").toString();
            return this;
        }

        /**
         * A value every value must be above.
         *
         * @param value a decimal in invariant notation
         * @return this builder
         */
        public Builder<T> exclusiveMinimum(String value) {
            exclusiveMinimum = Objects.requireNonNull(value, "exclusiveMinimum");
            return this;
        }

        /**
         * The largest value allowed.
         *
         * @param value an exact decimal
         * @return this builder
         */
        public Builder<T> maximum(BigDecimal value) {
            maximum = Objects.requireNonNull(value, "maximum").toString();
            return this;
        }

        /**
         * The largest value allowed.
         *
         * @param value a decimal in invariant notation
         * @return this builder
         */
        public Builder<T> maximum(String value) {
            maximum = Objects.requireNonNull(value, "maximum");
            return this;
        }

        /**
         * A value every value must be below.
         *
         * @param value an exact decimal
         * @return this builder
         */
        public Builder<T> exclusiveMaximum(BigDecimal value) {
            exclusiveMaximum = Objects.requireNonNull(value, "exclusiveMaximum").toString();
            return this;
        }

        /**
         * A value every value must be below.
         *
         * @param value a decimal in invariant notation
         * @return this builder
         */
        public Builder<T> exclusiveMaximum(String value) {
            exclusiveMaximum = Objects.requireNonNull(value, "exclusiveMaximum");
            return this;
        }

        /**
         * The positive step every value must be a multiple of.
         *
         * @param value an exact decimal
         * @return this builder
         */
        public Builder<T> multipleOf(BigDecimal value) {
            multipleOf = Objects.requireNonNull(value, "multipleOf").toString();
            return this;
        }

        /**
         * The positive step every value must be a multiple of.
         *
         * @param value a decimal in invariant notation ({@code "0.05"})
         * @return this builder
         */
        public Builder<T> multipleOf(String value) {
            multipleOf = Objects.requireNonNull(value, "multipleOf");
            return this;
        }

        /**
         * What a value that is not a multiple becomes; anything but {@link Rounding#STRICT}
         * needs {@code multipleOf}.
         *
         * @param value the rounding
         * @return this builder
         */
        public Builder<T> rounding(Rounding value) {
            rounding = Objects.requireNonNull(value, "rounding");
            return this;
        }

        /**
         * The fewest code points the text may hold.
         *
         * @param value at least 0
         * @return this builder
         * @throws IllegalArgumentException {@code value} is negative
         */
        public Builder<T> minLength(int value) {
            minLength = length(value, "minLength");
            return this;
        }

        /**
         * The most code points the text may hold.
         *
         * @param value at least 0
         * @return this builder
         * @throws IllegalArgumentException {@code value} is negative
         */
        public Builder<T> maxLength(int value) {
            maxLength = length(value, "maxLength");
            return this;
        }

        /**
         * Whether to trim leading and trailing Unicode {@code White_Space} (U+00A0 included)
         * from the text before judging it.
         *
         * @param value {@code true} to trim
         * @return this builder
         */
        public Builder<T> trim(boolean value) {
            trim = value;
            return this;
        }

        /**
         * The values allowed, distinct: at the text door, the strings themselves, matched under
         * Unicode simple case folding, a match delivering the entry as declared; at an integer
         * door, whole numbers in invariant notation. Not calling it declares no {@code enum};
         * calling it with no entries ({@code enumOf(new String[0])}) declares one that no value
         * could match, which the core refuses.
         *
         * @param values the entries
         * @return this builder
         */
        public Builder<T> enumOf(String... values) {
            entries = entries(values, "enum");
            return this;
        }

        /**
         * The values allowed at an integer door, distinct whole numbers. The text door takes
         * strings only.
         *
         * @param values the entries
         * @return this builder
         */
        public Builder<T> enumOf(BigDecimal... values) {
            entries = entries(values, "enum");
            return this;
        }

        /**
         * Has the native core judge the declaration for this door.
         *
         * @return the immutable declaration
         * @throws IllegalArgumentException the core refused it, a contract violation rather than
         *     a data verdict, the message naming the rule: a keyword the door does not take;
         *     {@code minimum} with {@code exclusiveMinimum}, or {@code maximum} with
         *     {@code exclusiveMaximum}; a lower bound not below the upper; a {@code multipleOf}
         *     not positive; a rounding without {@code multipleOf}; {@code minLength} above
         *     {@code maxLength}; a bound, step or entry the door cannot hold (a fraction or out
         *     of range at an integer door, or text that is no decimal or does not fit one); an
         *     entry that is not UTF-8 (a lone surrogate); duplicate entries; an {@code enum}
         *     with no entries; a text entry no input can match (an empty one, or under
         *     {@code trim} one with leading or trailing white space). Also a
         *     {@link BigDecimal} entry at the text door
         */
        public Constraints<T> build() {
            return new Constraints<>(this);
        }

        private static Integer length(int value, String name) {
            if (value < 0) {
                throw new IllegalArgumentException(name + " is " + value + "; a length is at least 0");
            }
            return value;
        }

        private static List<Object> entries(Object[] values, String name) {
            Objects.requireNonNull(values, name);
            List<Object> list = new ArrayList<>(values.length);
            for (int i = 0; i < values.length; i++) {
                list.add(Objects.requireNonNull(values[i], name + " entry " + i));
            }
            return list;
        }
    }
}
