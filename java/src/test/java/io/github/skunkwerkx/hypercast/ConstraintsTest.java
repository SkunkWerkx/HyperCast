package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.skunkwerkx.hypercast.interop.NativeValues;
import java.lang.foreign.MemorySegment;
import java.math.BigDecimal;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Optional;
import java.util.OptionalInt;
import org.junit.jupiter.api.Test;

/**
 * The checked doors' binding-side contract, beside the corpus replay: the declaration's
 * builder and its refusals, the door binding, lone surrogates, the caller's own enum instance,
 * the open keyword, and the README's examples as written.
 */
class ConstraintsTest {
    private static final Constraints<String> SEX =
            Constraints.text().trim(true).enumOf("Female", "Male", "Other").build();

    @Test
    void theReadmeExamplesAnswerAsDocumented() {
        Constraints<Double> latitude = Constraints.f64()
                .minimum("-90")
                .maximum("90")
                .multipleOf("0.000001")
                .rounding(Rounding.MIDPOINT)
                .build();
        Constraints<BigDecimal> euroCash =
                Constraints.decimal().multipleOf("0.05").build();
        NumFormat euro = new NumFormat(',', '.', NumFormat.STYLE_ALL, "€");

        assertEquals(
                new Checked<>(45.123457, true, OptionalInt.empty()),
                Cast.f64("45.1234567", NumFormat.INVARIANT, latitude));
        assertEquals(new Violation<Double>(Keyword.MINIMUM, 0, 5), Cast.f64("-90.5", NumFormat.INVARIANT, latitude));
        assertEquals(new Checked<>(45.123457, true, OptionalInt.empty()), Cast.f64FromDouble(45.1234567, latitude));
        assertEquals(
                new Checked<>(new BigDecimal("1234.55"), false, OptionalInt.empty()),
                Cast.decimal("€1.234,55", euro, euroCash));
        assertEquals(new Violation<BigDecimal>(Keyword.MULTIPLE_OF, 0, 6), Cast.decimal("€12,37", euro, euroCash));
        assertEquals(new Fault<BigDecimal>(CastFailure.MALFORMED, 4, 1), Cast.decimal("12,3x", euro, euroCash));

        assertEquals("Male, entry 1", describe(" MALE ", SEX));
        assertEquals("not a sex code: chars 0+5", describe("Femal", SEX));
        assertEquals("EMPTY at char 0", describe("   ", SEX));
    }

    // The README's switch, verbatim.
    private static String describe(String input, Constraints<String> sex) {
        return switch (Cast.text(input, sex)) {
            case Checked<String> ok -> ok.value() + ", entry " + ok.enumIndex().getAsInt();
            case Fault<String> f -> f.reason() + " at char " + f.offset();
            case Violation<String> v ->
                switch (v.keyword().name()) {
                    case "enum" -> "not a sex code: chars " + v.offset() + "+" + v.length();
                    default -> "breaks " + v.keyword(); // keywords are open
                };
        };
    }

    @Test
    void aMatchHandsBackTheCallersOwnEntry() {
        String male = "Male";
        Constraints<String> sex = Constraints.text().enumOf("Female", male).build();
        Checked<String> ok = (Checked<String>) Cast.text("mAlE", sex);
        assertSame(male, ok.value());
        assertEquals(OptionalInt.of(1), ok.enumIndex());
        assertSame(male, ((Checked<String>) Cast.text("MALE".getBytes(StandardCharsets.UTF_8), sex)).value());
    }

    @Test
    void theTextDoorHandsBackTheInputItselfWhenNothingWasTrimmed() {
        String input = "naïve";
        assertSame(input, ((Success<String>) Cast.text(input)).value());
        Constraints<String> anything = Constraints.text().build();
        assertSame(input, ((Checked<String>) Cast.text(input, anything)).value());
        Constraints<String> trimmed = Constraints.text().trim(true).build();
        assertEquals(new Checked<>("naïve", false, OptionalInt.empty()), Cast.text(" naïve ", trimmed));
        assertEquals(new Success<>("naïve"), Cast.text("naïve".getBytes(StandardCharsets.UTF_8)));
        assertEquals(
                new Success<>("naïve"), Cast.text(MemorySegment.ofArray("naïve".getBytes(StandardCharsets.UTF_8))));
        assertEquals(new Fault<String>(CastFailure.EMPTY, 0, 0), Cast.text(""));
    }

    @Test
    void lengthsCountCodePointsAndSpansCountChars() {
        Constraints<String> two = Constraints.text().maxLength(2).build();
        // A thumbs-up with a skin tone is two code points, four chars.
        assertTrue(Cast.text("👍🏽", two) instanceof Checked<String>);
        Constraints<String> one = Constraints.text().maxLength(1).build();
        assertEquals(new Violation<String>(Keyword.MAX_LENGTH, 0, 4), Cast.text("👍🏽", one));
        assertEquals(
                new Violation<String>(Keyword.MAX_LENGTH, 0, 8),
                Cast.text("👍🏽".getBytes(StandardCharsets.UTF_8), one));
    }

    @Test
    void aLoneSurrogateIsMalformedAtItsCharsNeverReplaced() {
        // Java's own encoder would make it '?', which is text; the core must see it as it is.
        String lone = "ab\uD800c";
        assertEquals(new Fault<String>(CastFailure.MALFORMED, 2, 1), Cast.text(lone));
        assertEquals(
                new Fault<String>(CastFailure.MALFORMED, 2, 1),
                Cast.text(lone, Constraints.text().build()));
        assertEquals(
                new Fault<String>(CastFailure.MALFORMED, 2, 1),
                Cast.text(lone, Constraints.text().enumOf("ab?c").build()));
        Constraints<Integer> any = Constraints.i32().build();
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 1, 1), Cast.i32("1\uDC00", NumFormat.INVARIANT, any));
        // A whole pair is one code point, four bytes, two chars: no fault.
        assertTrue(Cast.text("a👍", Constraints.text().build()) instanceof Checked<String>);
    }

    @Test
    void aLoneSurrogateInAnEntryIsRefusedAsNotUtf8() {
        IllegalArgumentException e = assertThrows(
                IllegalArgumentException.class,
                () -> Constraints.text().enumOf("ok", "\uDFFF").build());
        assertEquals(CorpusTest.refusalMessage(9), e.getMessage());
        // In a bound it is text no door can hold.
        e = assertThrows(
                IllegalArgumentException.class,
                () -> Constraints.i32().minimum("1\uD800").build());
        assertEquals(CorpusTest.refusalMessage(8), e.getMessage());
    }

    @Test
    void constraintsBuiltForOneDoorAreRefusedAtAnother() {
        Constraints<Integer> u8 = Constraints.u8().maximum("200").build();
        IllegalArgumentException e =
                assertThrows(IllegalArgumentException.class, () -> Cast.i32("1", NumFormat.INVARIANT, u8));
        assertEquals("Constraints built for the u8 door were passed to the i32 door", e.getMessage());
        assertThrows(IllegalArgumentException.class, () -> Cast.u16FromDouble(1, u8));
        assertThrows(
                IllegalArgumentException.class,
                () -> Cast.u16("1".getBytes(StandardCharsets.UTF_8), NumFormat.INVARIANT, u8));
        assertTrue(Cast.u8("1", NumFormat.INVARIANT, u8) instanceof Checked<Integer>);
    }

    @Test
    void aKeywordTheDoorDoesNotTakeIsTheCoresRefusal() {
        IllegalArgumentException e = assertThrows(
                IllegalArgumentException.class,
                () -> Constraints.i32().minLength(1).build());
        assertEquals(CorpusTest.refusalMessage(1), e.getMessage());
        e = assertThrows(
                IllegalArgumentException.class,
                () -> Constraints.f64().enumOf("1").build());
        assertEquals(CorpusTest.refusalMessage(1), e.getMessage());
        e = assertThrows(
                IllegalArgumentException.class,
                () -> Constraints.u8().minimum("-1").build());
        assertEquals("Constraints refused (code 8): a bound, step or enum entry the door cannot hold.", e.getMessage());
    }

    @Test
    void anEmptyEnumIsDeclaredAndRefused() {
        // No enumOf is no enum; enumOf with no entries is one no value could match.
        assertEquals(
                0,
                ByteBuffer.wrap(Constraints.i32().build().toRaw())
                        .order(ByteOrder.nativeOrder())
                        .getInt(4));
        assertEquals(
                CorpusTest.refusalMessage(12),
                assertThrows(
                                IllegalArgumentException.class,
                                () -> Constraints.i32()
                                        .enumOf(new BigDecimal[0])
                                        .build())
                        .getMessage());
        assertEquals(
                CorpusTest.refusalMessage(12),
                assertThrows(
                                IllegalArgumentException.class,
                                () -> Constraints.text().enumOf(new String[0]).build())
                        .getMessage());
        assertEquals(
                CorpusTest.refusalMessage(1),
                assertThrows(
                                IllegalArgumentException.class,
                                () -> Constraints.f64().enumOf(new String[0]).build())
                        .getMessage());
        assertEquals(
                CorpusTest.refusalMessage(13),
                assertThrows(
                                IllegalArgumentException.class,
                                () -> Constraints.text().enumOf("Male", "").build())
                        .getMessage());
        assertEquals(
                CorpusTest.refusalMessage(13),
                assertThrows(
                                IllegalArgumentException.class,
                                () -> Constraints.text()
                                        .trim(true)
                                        .enumOf(" Male ")
                                        .build())
                        .getMessage());
        Constraints<String> spaced = Constraints.text().enumOf(" Male ").build();
        assertEquals(
                Constraints.HAS_ENUM,
                ByteBuffer.wrap(spaced.toRaw()).order(ByteOrder.nativeOrder()).getInt(4));
        assertEquals("Constraints refused (code 14): a rule this binding does not name.", Constraints.refusal(14));
    }

    /**
     * The flag bits this binding restates, as literals on every leg, and the header's own
     * wherever the tree holds include/hypercast.h. The musl suite stages only rust/Cargo.toml,
     * corpus/ and java/, so there the literals are asserted alone.
     */
    @Test
    void theFlagsAreTheHeaders() throws java.io.IOException {
        java.util.Map<String, Integer> flags = java.util.Map.of(
                "HAS_MINIMUM", 1,
                "HAS_EXCLUSIVE_MINIMUM", 2,
                "HAS_MAXIMUM", 4,
                "HAS_EXCLUSIVE_MAXIMUM", 8,
                "HAS_MULTIPLE_OF", 16,
                "HAS_MIN_LENGTH", 32,
                "HAS_MAX_LENGTH", 64,
                "TRIM", 128,
                "HAS_ENUM", 256);
        java.util.Map<String, Integer> restated = java.util.Map.of(
                "HAS_MINIMUM", Constraints.HAS_MINIMUM,
                "HAS_EXCLUSIVE_MINIMUM", Constraints.HAS_EXCLUSIVE_MINIMUM,
                "HAS_MAXIMUM", Constraints.HAS_MAXIMUM,
                "HAS_EXCLUSIVE_MAXIMUM", Constraints.HAS_EXCLUSIVE_MAXIMUM,
                "HAS_MULTIPLE_OF", Constraints.HAS_MULTIPLE_OF,
                "HAS_MIN_LENGTH", Constraints.HAS_MIN_LENGTH,
                "HAS_MAX_LENGTH", Constraints.HAS_MAX_LENGTH,
                "TRIM", Constraints.TRIM,
                "HAS_ENUM", Constraints.HAS_ENUM);
        assertEquals(flags, restated);
        java.nio.file.Path header = null;
        for (java.nio.file.Path dir = java.nio.file.Path.of("").toAbsolutePath(); dir != null; dir = dir.getParent()) {
            java.nio.file.Path candidate = dir.resolve("include/hypercast.h");
            if (java.nio.file.Files.isRegularFile(candidate)) {
                header = candidate;
                break;
            }
        }
        if (header == null) {
            return; // no header in this tree (the musl suite's): the literals above stand alone
        }
        String text = java.nio.file.Files.readString(header);
        flags.forEach((name, value) -> assertTrue(
                text.contains("#define HyperCastRawConstraints_" + name + " " + value + "\n"),
                "the header does not carry " + name + " " + value));
    }

    @Test
    void boundsAreExactDecimalsConvertedByTheCore() {
        Constraints<BigDecimal> fine = Constraints.decimal()
                .minimum(new BigDecimal("1E-6"))
                .maximum(new BigDecimal("1.50"))
                .build();
        assertEquals(Optional.of(new BigDecimal("0.000001")), fine.minimum());
        // Trailing zeros dropped, as the core converts them.
        assertEquals(Optional.of(new BigDecimal("1.5")), fine.maximum());
        assertEquals(Optional.empty(), fine.multipleOf());
        assertEquals(Rounding.STRICT, fine.rounding());
        // 1e-29 is past the 28 places a Decimal holds: refused, never rounded to zero.
        IllegalArgumentException e = assertThrows(
                IllegalArgumentException.class,
                () -> Constraints.decimal().multipleOf(new BigDecimal("1E-29")).build());
        assertEquals(CorpusTest.refusalMessage(8), e.getMessage());
        e = assertThrows(
                IllegalArgumentException.class,
                () -> Constraints.decimal().multipleOf("0.1.2").build());
        assertEquals(CorpusTest.refusalMessage(8), e.getMessage());
    }

    @Test
    void integerEntriesAreDecimalsAndTextEntriesAreStrings() {
        Constraints<Integer> codes = Constraints.i32()
                .enumOf(new BigDecimal("10"), new BigDecimal("2E+1"))
                .build();
        assertEquals(new Checked<>(20, false, OptionalInt.empty()), Cast.i32("20", NumFormat.INVARIANT, codes));
        assertEquals(new Violation<Integer>(Keyword.ENUM, 0, 2), Cast.i32("30", NumFormat.INVARIANT, codes));
        assertEquals(32, codes.packedEnum().length);
        IllegalArgumentException e = assertThrows(
                IllegalArgumentException.class,
                () -> Constraints.text().enumOf(BigDecimal.ONE).build());
        assertEquals("Text enum entries are strings; entry 0 is the number 1.", e.getMessage());
        assertThrows(IllegalArgumentException.class, () -> Constraints.text().minLength(-1));
        assertThrows(NullPointerException.class, () -> Constraints.i32().minimum((String) null));
        assertThrows(NullPointerException.class, () -> Constraints.text().enumOf("a", null));
    }

    @Test
    void theLayoutIsTheCoresInThisPlatformsOrder() {
        Constraints<String> text = Constraints.text()
                .minLength(2)
                .maxLength(3)
                .trim(true)
                .enumOf("ab", "cde")
                .build();
        ByteBuffer raw = ByteBuffer.wrap(text.toRaw()).order(ByteOrder.nativeOrder());
        assertEquals(NativeValues.CONSTRAINTS_BYTES, text.toRaw().length);
        assertEquals(72, raw.getInt(0));
        assertEquals(32 | 64 | 128 | 256, raw.getInt(4));
        assertEquals(List.of(0, 2, 3, 0), List.of(raw.getInt(8), raw.getInt(12), raw.getInt(16), raw.getInt(20)));
        assertArrayEquals(new byte[] {2, 0, 0, 0, 'a', 'b', 3, 0, 0, 0, 'c', 'd', 'e'}, text.packedEnum());
        assertEquals(OptionalInt.of(2), text.minLength());
        assertEquals(OptionalInt.of(3), text.maxLength());
        assertTrue(text.trim());

        Constraints<Long> step = Constraints.i64()
                .exclusiveMinimum("-7")
                .multipleOf("0.50E+2")
                .rounding(Rounding.CEILING)
                .build();
        raw = ByteBuffer.wrap(step.toRaw()).order(ByteOrder.nativeOrder());
        assertEquals(2 | 16, raw.getInt(4));
        assertEquals(3, raw.getInt(8));
        assertEquals(
                List.of(7L, 0, (byte) 0, (byte) 1), List.of(raw.getLong(24), raw.getInt(32), raw.get(36), raw.get(37)));
        assertEquals(
                List.of(50L, 0, (byte) 0, (byte) 0),
                List.of(raw.getLong(56), raw.getInt(64), raw.get(68), raw.get(69)));
        // A copy each time: the declaration stays immutable.
        step.toRaw()[0] = 0;
        assertEquals(
                72, ByteBuffer.wrap(step.toRaw()).order(ByteOrder.nativeOrder()).getInt(0));
    }

    @Test
    void aKeywordIsOpenAndSpelledTheOpenApiWay() {
        assertEquals(
                List.of(
                        "minimum",
                        "exclusiveMinimum",
                        "maximum",
                        "exclusiveMaximum",
                        "multipleOf",
                        "minLength",
                        "maxLength",
                        "enum"),
                List.of(
                                Keyword.MINIMUM,
                                Keyword.EXCLUSIVE_MINIMUM,
                                Keyword.MAXIMUM,
                                Keyword.EXCLUSIVE_MAXIMUM,
                                Keyword.MULTIPLE_OF,
                                Keyword.MIN_LENGTH,
                                Keyword.MAX_LENGTH,
                                Keyword.ENUM)
                        .stream()
                        .map(Keyword::toString)
                        .toList());
        assertEquals(List.of(1, 8), List.of(Keyword.MINIMUM.code(), Keyword.ENUM.code()));
        assertEquals(Keyword.MULTIPLE_OF, new Keyword(5));
        assertEquals("keyword 9", new Keyword(9).name());
        assertThrows(IllegalArgumentException.class, () -> new Keyword(0));
        assertEquals(
                List.of(0, 1, 2, 3),
                List.of(Rounding.values()).stream().map(Rounding::code).toList());
    }

    @Test
    void zeroHasOneForm() {
        Constraints<Double> any = Constraints.f64().build();
        Checked<Double> zero = (Checked<Double>) Cast.f64FromDouble(-0.0, any);
        assertEquals(0L, Double.doubleToRawLongBits(zero.value()));
        assertFalse(zero.adjusted());
        Constraints<Double> step = Constraints.f64()
                .multipleOf("0.000001")
                .rounding(Rounding.MIDPOINT)
                .build();
        Checked<Double> rounded = (Checked<Double>) Cast.f64("-0.0000004", NumFormat.INVARIANT, step);
        assertEquals(0L, Double.doubleToRawLongBits(rounded.value()));
        assertTrue(rounded.adjusted());
    }

    @Test
    void aCheckedVerdictRefusesAnEmptyCase() {
        assertThrows(IllegalArgumentException.class, () -> new Checked<>(null, false, OptionalInt.empty()));
        assertThrows(IllegalArgumentException.class, () -> new Checked<>(1, false, null));
        assertThrows(IllegalArgumentException.class, () -> new Violation<Integer>(null, 0, 0));
    }
}
