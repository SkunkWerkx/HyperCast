package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.skunkwerkx.hypercast.interop.NativeValues;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.util.List;
import java.util.Optional;
import java.util.OptionalInt;
import org.junit.jupiter.api.Test;

/**
 * The detailed path: {@link DetailedCast}'s answers are {@link Cast}'s plus the core's detail,
 * through every input form, and an unpublished detail code is a usable failure, not an error.
 */
final class DetailedCastTest {
    @Test
    void theSwitchIsExhaustiveWithTwoArms() {
        // This compiling at all is the feature: DetailedVerdict is sealed over two records.
        String rendered = switch (DetailedCast.i32("12,34", NumFormat.INVARIANT)) {
            case DetailedSuccess<Integer> success -> "ok " + success.value();
            case DetailedFault<Integer> fault -> fault.reason() + " " + fault.detail();
        };
        assertEquals("MALFORMED group_size", rendered);
        String checked = switch (DetailedCast.i32(
                "101", NumFormat.INVARIANT, Constraints.i32().maximum("100").build())) {
            case DetailedChecked<Integer> ok -> "ok " + ok.value();
            case DetailedFault<Integer> fault -> "fault " + fault.detail();
            case DetailedViolation<Integer> violation -> "breaks " + violation.keyword();
        };
        assertEquals("breaks maximum", checked);
    }

    @Test
    void aFailureCarriesTheCoresDetailThroughEveryInputForm() {
        DetailedFault<Integer> expected = new DetailedFault<>(CastFailure.MALFORMED, FaultDetail.GROUP_SIZE, 2, 3);
        byte[] bytes = "12,34".getBytes(StandardCharsets.UTF_8);
        assertEquals(expected, DetailedCast.i32("12,34", NumFormat.INVARIANT));
        assertEquals(expected, DetailedCast.i32(bytes, NumFormat.INVARIANT));
        assertEquals(expected, DetailedCast.i32(MemorySegment.ofArray(bytes), NumFormat.INVARIANT));
        assertEquals(expected, DetailedCast.i32("12,34", java.util.Locale.US));
        // The plain door answers the same fault, without the detail.
        assertEquals(expected.toFault(), Cast.i32("12,34", NumFormat.INVARIANT));
        // The detail is independent of the reason: out of range and malformed share codes where the
        // meaning is the same.
        assertEquals(
                new DetailedFault<Byte>(CastFailure.OUT_OF_RANGE, FaultDetail.TOO_LARGE, 0, 3),
                DetailedCast.i8("128", NumFormat.INVARIANT));
        assertEquals(new DetailedFault<Boolean>(CastFailure.EMPTY, FaultDetail.BLANK, 0, 0), DetailedCast.bool(""));
    }

    @Test
    void aStringDoorRebasesTheSpanAndKeepsTheDetail() {
        // Arabic-Indic digits are two UTF-8 bytes and one char each: the span is in chars.
        NumFormat arabic = NumFormat.INVARIANT.withDigitZero(0x0660);
        DetailedFault<Integer> fault =
                assertInstanceOf(DetailedFault.class, DetailedCast.i32("\u0661\u0662,\u0663\u0664", arabic));
        assertEquals(FaultDetail.GROUP_SIZE, fault.detail());
        assertEquals(2, fault.offset());
        assertEquals(3, fault.length());
        DetailedFault<Integer> bytes = assertInstanceOf(
                DetailedFault.class,
                DetailedCast.i32("\u0661\u0662,\u0663\u0664".getBytes(StandardCharsets.UTF_8), arabic));
        assertEquals(FaultDetail.GROUP_SIZE, bytes.detail());
        assertEquals(4, bytes.offset());
        assertEquals(5, bytes.length());
    }

    @Test
    void theGroupingPolicyDecidesAndLegacyKeepsTheOldRule() {
        assertEquals(new DetailedSuccess<>(1234), DetailedCast.i32("1,234", NumFormat.INVARIANT));
        assertEquals(
                new DetailedSuccess<>(1234),
                DetailedCast.i32("12,34", NumFormat.INVARIANT.withGrouping(Grouping.LEGACY)));
        NumFormat indian = NumFormat.INVARIANT.withGrouping(Grouping.INDIAN);
        assertEquals(new DetailedSuccess<>(1234567), DetailedCast.i32("12,34,567", indian));
        assertEquals(
                new DetailedFault<Integer>(CastFailure.MALFORMED, FaultDetail.GROUP_SIZE, 1, 4),
                DetailedCast.i32("1,234,567", indian));
        // A strict policy does not enable grouping: with the style off a separator is forbidden.
        NumFormat off = NumFormat.INVARIANT.withStyles(NumFormat.STYLE_ALL & ~NumFormat.STYLE_GROUPING);
        assertEquals(FaultDetail.UNEXPECTED_CHARACTER, ((DetailedFault<?>) DetailedCast.i32("1,234", off)).detail());
    }

    @Test
    void aViolationNamesItsKeywordAndNoDetail() {
        Constraints<Integer> percent =
                Constraints.i32().minimum("0").maximum("100").build();
        DetailedCheckedVerdict<Integer> verdict = DetailedCast.i32(" 101 ", NumFormat.INVARIANT, percent);
        assertEquals(new DetailedViolation<Integer>(Keyword.MAXIMUM, 1, 3), verdict);
        assertEquals(
                new Violation<Integer>(Keyword.MAXIMUM, 1, 3), ((DetailedViolation<Integer>) verdict).toViolation());
        assertEquals(
                new DetailedChecked<>(7, false, OptionalInt.empty()),
                DetailedCast.i32("7", NumFormat.INVARIANT, percent));
        // A parse failure inside a checked door keeps its detail, and the span of its units.
        assertEquals(
                new DetailedFault<Integer>(CastFailure.MALFORMED, FaultDetail.UNEXPECTED_CHARACTER, 1, 1),
                DetailedCast.i32("1x", NumFormat.INVARIANT, percent));
        // From a double: not_finite, an empty span.
        assertEquals(
                new DetailedFault<Integer>(CastFailure.MALFORMED, FaultDetail.NOT_FINITE, 0, 0),
                DetailedCast.i32FromDouble(Double.NaN, percent));
        assertEquals(new DetailedViolation<Integer>(Keyword.MAXIMUM, 0, 0), DetailedCast.i32FromDouble(101, percent));
        // The text door's enum hands back the caller's own entry.
        Constraints<String> sex =
                Constraints.text().trim(true).enumOf("Female", "Male").build();
        assertEquals(new DetailedChecked<>("Male", false, OptionalInt.of(1)), DetailedCast.text("  MALE ", sex));
        assertEquals(new DetailedViolation<String>(Keyword.ENUM, 2, 5), DetailedCast.text("  Other ", sex));
    }

    @Test
    void theTextDoorNamesInvalidUtf8() {
        DetailedFault<String> fault = assertInstanceOf(DetailedFault.class, DetailedCast.text("a\uD800b"));
        assertEquals(CastFailure.MALFORMED, fault.reason());
        assertEquals(FaultDetail.INVALID_UTF8, fault.detail());
        assertEquals(1, fault.offset());
        assertEquals(1, fault.length());
        assertEquals(new DetailedFault<String>(CastFailure.EMPTY, FaultDetail.BLANK, 0, 0), DetailedCast.text(""));
        assertEquals(new DetailedSuccess<>("a b"), DetailedCast.text("a b"));
    }

    @Test
    void aSupplementaryScalarIsTooLargeForAChar() {
        DetailedFault<Character> fault = assertInstanceOf(DetailedFault.class, DetailedCast.character("  U+1F600 "));
        assertEquals(CastFailure.OUT_OF_RANGE, fault.reason());
        assertEquals(FaultDetail.TOO_LARGE, fault.detail());
        assertEquals(2, fault.offset());
        assertEquals(7, fault.length());
        assertEquals(new DetailedSuccess<>('A'), DetailedCast.character("A"));
        assertEquals(new DetailedSuccess<>('\u00e9'), DetailedCast.character("&#233;"));
    }

    @Test
    void optionalPresentsBlankAsAbsent() {
        assertEquals(Optional.empty(), DetailedCast.optional(DetailedCast.bool("  ")));
        DetailedVerdict<Boolean> maybe = DetailedCast.bool("maybe");
        assertEquals(Optional.of(maybe), DetailedCast.optional(maybe));
        assertEquals(
                Optional.empty(),
                DetailedCast.optional(DetailedCast.i32(
                        "", NumFormat.INVARIANT, Constraints.i32().build())));
        DetailedFault<Boolean> fault = new DetailedFault<>(CastFailure.EMPTY, FaultDetail.BLANK, 0, 0);
        assertEquals(Optional.empty(), DetailedCast.optional(fault));
    }

    // --- an unpublished detail code ---

    @Test
    void anUnknownDetailKeepsItsCodeAndTheFaultStaysUsable() {
        DetailedFault<Integer> fault = NativeValues.detailedFault(2, 3, 1, 999);
        assertEquals(CastFailure.MALFORMED, fault.reason());
        assertEquals(3, fault.offset());
        assertEquals(1, fault.length());
        assertEquals(999, fault.detail().code());
        assertFalse(fault.detail().isKnown());
        assertEquals("detail 999", fault.detail().name());
        assertEquals("detail 999", fault.detail().toString());
        // Equality keeps the raw code, so it never collapses two unknowns into one.
        assertEquals(fault, NativeValues.<Integer>detailedFault(2, 3, 1, 999));
        assertFalse(fault.equals(NativeValues.<Integer>detailedFault(2, 3, 1, 998)));
        // The plain fault is the same reason and span.
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 3, 1), fault.toFault());
        // Never an error to decode, and never a known name by accident.
        assertEquals(Optional.empty(), FaultDetail.fromName("detail 999"));
        assertEquals(FaultDetail.NOT_A_SCALAR, FaultDetail.of(32));
        assertEquals(33, FaultDetail.of(33).code());
        assertFalse(FaultDetail.of(33).isKnown());
    }

    @Test
    void aConstructedRawFaultDetailWithAnUnpublishedCodeDecodes() {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment raw = arena.allocate(NativeValues.FAULT_DETAIL_BYTES + 4, 4);
            raw.set(ValueLayout.JAVA_INT, 4, 16);
            raw.set(ValueLayout.JAVA_INT, 8, 5);
            raw.set(ValueLayout.JAVA_INT, 12, 2);
            raw.set(ValueLayout.JAVA_INT, 16, 999);
            DetailedFault<Long> fault = NativeValues.detailedFault(3, raw, 4);
            assertEquals(CastFailure.OUT_OF_RANGE, fault.reason());
            assertEquals(5, fault.offset());
            assertEquals(2, fault.length());
            assertEquals(999, fault.detail().code());
            assertFalse(fault.detail().isKnown());
            // And a published one in the same layout reads as its name.
            raw.set(ValueLayout.JAVA_INT, 16, 11);
            assertEquals(
                    FaultDetail.GROUP_SIZE,
                    NativeValues.<Long>detailedFault(2, raw, 4).detail());
        }
        // A reason that is none is a binding bug, not data; a detail code never is, whatever it is.
        assertThrows(IllegalStateException.class, () -> NativeValues.detailedFault(9, 0, 0, 1));
        DetailedFault<Integer> zero = NativeValues.detailedFault(2, 0, 1, 0);
        assertEquals(0, zero.detail().code());
        assertFalse(zero.detail().isKnown());
        assertEquals("detail 0", zero.detail().name());
        // The word is unsigned: 0xFFFFFFFF is a code, not a negative one.
        DetailedFault<Integer> high = NativeValues.detailedFault(2, 0, 1, 0xFFFFFFFF);
        assertEquals("detail 4294967295", high.detail().name());
        assertFalse(high.detail().isKnown());
        assertEquals(CastFailure.MALFORMED, high.reason());
    }

    @Test
    void theVocabularyIsThePublishedOne() {
        List<String> names = List.of(
                "blank",
                "unexpected_character",
                "digit_expected",
                "delimiter_expected",
                "trailing_text",
                "no_digits",
                "sign_misplaced",
                "parentheses_unbalanced",
                "mark_repeated",
                "group_misplaced",
                "group_size",
                "group_in_fraction",
                "ambiguous_separator",
                "fraction_not_allowed",
                "field_width",
                "wrong_length",
                "component_missing",
                "not_recognized",
                "unknown_unit",
                "invalid_utf8",
                "too_long",
                "not_finite",
                "not_whole",
                "negative_not_allowed",
                "too_large",
                "too_small",
                "too_precise",
                "date_field_out_of_range",
                "time_field_out_of_range",
                "outside_window",
                "nonexistent_date",
                "not_a_scalar");
        for (int i = 0; i < names.size(); i++) {
            FaultDetail detail = FaultDetail.of(i + 1);
            assertEquals(names.get(i), detail.name());
            assertEquals(i + 1, detail.code());
            assertTrue(detail.isKnown());
            assertEquals(Optional.of(detail), FaultDetail.fromName(names.get(i)));
        }
        // Decoding never throws: 0 and the unsigned high words are kept as details nobody named.
        assertFalse(FaultDetail.of(0).isKnown());
        assertFalse(FaultDetail.of(-1).isKnown());
        assertEquals(Optional.empty(), FaultDetail.fromName("detail 0"));
    }

    @Test
    void theDetailedTypesAreShapedLikeTheirPlainTwins() {
        assertEquals(
                List.of(DetailedSuccess.class, DetailedFault.class),
                List.of(DetailedVerdict.class.getPermittedSubclasses()));
        assertEquals(
                List.of(DetailedChecked.class, DetailedRejection.class),
                List.of(DetailedCheckedVerdict.class.getPermittedSubclasses()));
        assertEquals(
                List.of(DetailedFault.class, DetailedViolation.class),
                List.of(DetailedRejection.class.getPermittedSubclasses()));
        assertEquals(
                List.of(
                        "io.github.skunkwerkx.hypercast.CastFailure reason",
                        "io.github.skunkwerkx.hypercast.FaultDetail detail",
                        "int offset",
                        "int length"),
                java.util.Arrays.stream(DetailedFault.class.getRecordComponents())
                        .map(c -> c.getGenericType().getTypeName() + " " + c.getName())
                        .toList());
        assertThrows(IllegalArgumentException.class, () -> new DetailedFault<>(null, FaultDetail.BLANK, 0, 0));
        assertThrows(IllegalArgumentException.class, () -> new DetailedFault<>(CastFailure.EMPTY, null, 0, 0));
        assertThrows(IllegalArgumentException.class, () -> new DetailedViolation<>(null, 0, 0));
        assertThrows(IllegalArgumentException.class, () -> new DetailedChecked<>(null, false, OptionalInt.empty()));
    }

    // --- date options and month names ---

    @Test
    void everyDoorKeepsTodaysShapesUntilOptionsAreHandedOver() {
        assertEquals(FaultDetail.WRONG_LENGTH, ((DetailedFault<?>) DetailedCast.date("20260107")).detail());
        assertEquals(
                new DetailedSuccess<>(LocalDate.of(2026, 1, 7)),
                DetailedCast.date("20260107", DateOptions.COMMON_FORMS));
        assertEquals(
                FaultDetail.DELIMITER_EXPECTED,
                ((DetailedFault<?>) DetailedCast.timestamp("2026-01-07 15:04:05Z")).detail());
        assertEquals(
                new DetailedSuccess<>(Instant.parse("2026-01-07T15:04:05Z")),
                DetailedCast.timestamp("2026-01-07 15:04:05Z", DateOptions.COMMON_FORMS));
        assertEquals(
                new DetailedSuccess<>(Instant.parse("2026-01-07T15:04:05Z")),
                DetailedCast.timestamp("2026-01-07T15:04:05+00", DateOptions.COMMON_FORMS));
        // Each flag opens only its own shape: a flag that means nothing to a door is ignored.
        assertEquals(
                new DetailedSuccess<>(Instant.parse("2026-01-07T15:04:05Z")),
                DetailedCast.timestamp("2026-01-07 15:04:05Z", new DateOptions(DateOptions.SPACE_SEPARATOR)));
        assertTrue(
                DetailedCast.timestamp("2026-01-07T15:04:05+00", new DateOptions(DateOptions.SPACE_SEPARATOR))
                        instanceof DetailedFault<Instant>);
        assertEquals(
                new DetailedSuccess<>(LocalDate.of(2026, 1, 7)),
                DetailedCast.date("2026-01-07", DateOptions.COMMON_FORMS));
    }

    @Test
    void aPivotReadsATwoDigitYearAndNamesReadTheMonth() {
        DateOptions pivot = new DateOptions(0, 2029);
        assertEquals(
                new DetailedSuccess<>(LocalDate.of(2026, 1, 7)),
                DetailedCast.date("07/01/26", DateOrder.DAY_MONTH_YEAR, pivot));
        assertEquals(
                FaultDetail.FIELD_WIDTH,
                ((DetailedFault<?>) DetailedCast.date("07/01/26", DateOrder.DAY_MONTH_YEAR)).detail());
        MonthNames english = english();
        assertEquals(
                new DetailedSuccess<>(LocalDate.of(2026, 1, 7)),
                DetailedCast.date("07-Jan-26", DateOrder.DAY_MONTH_YEAR, pivot, english));
        // The vocabulary alone opens names and nothing else.
        assertEquals(
                new DetailedSuccess<>(LocalDate.of(2026, 1, 7)),
                DetailedCast.date("7-Jan-2026", DateOrder.DAY_MONTH_YEAR, DateOptions.NONE, english));
        assertEquals(
                new DetailedSuccess<>(LocalDateTime.of(2026, 1, 7, 15, 4)),
                DetailedCast.dateTime("07-Jan-26 3:04 PM", DateOrder.DAY_MONTH_YEAR, pivot, english));
        assertEquals(
                new DetailedSuccess<>(LocalDateTime.of(2026, 1, 7, 15, 4)),
                DetailedCast.dateTime(
                        "07-Jan-26 3:04 PM".getBytes(StandardCharsets.UTF_8),
                        new DateTimeFormat(DateOrder.DAY_MONTH_YEAR),
                        pivot,
                        english));
    }

    @Test
    void aDottedNameKeepsItsPunctuationAndTheSeparatorStillSeparates() {
        MonthNames.Builder builder = MonthNames.builder();
        String[] names = {
            "janv.",
            "f\u00e9vr.",
            "mars",
            "avr.",
            "mai",
            "juin",
            "juil.",
            "ao\u00fbt",
            "sept.",
            "oct.",
            "nov.",
            "d\u00e9c."
        };
        for (int i = 0; i < names.length; i++) {
            builder.add(i + 1, names[i]);
        }
        MonthNames french = builder.build();
        DateOptions pivot = new DateOptions(0, 2029);
        assertEquals(
                new DetailedSuccess<>(LocalDate.of(2026, 1, 7)),
                DetailedCast.date("7.janv..26", DateOrder.DAY_MONTH_YEAR, pivot, french));
        DetailedFault<LocalDate> fault = assertInstanceOf(
                DetailedFault.class, DetailedCast.date("7.janv.26", DateOrder.DAY_MONTH_YEAR, pivot, french));
        assertEquals(CastFailure.MALFORMED, fault.reason());
        assertEquals(FaultDetail.NOT_RECOGNIZED, fault.detail());
    }

    @Test
    void optionsAndVocabulariesAreValidatedByTheCoresOwnRules() {
        assertThrows(IllegalArgumentException.class, () -> new DateOptions(8));
        assertThrows(IllegalArgumentException.class, () -> new DateOptions(0, 99));
        assertThrows(IllegalArgumentException.class, () -> new DateOptions(0, 10000));
        assertEquals(0, DateOptions.NONE.pivot());
        assertEquals(7, DateOptions.COMMON_FORMS.flags());
        assertEquals(0, DateOptions.COMMON_FORMS.pivot());
        assertEquals(new DateOptions(1, 2029), DateOptions.NONE.withFlags(1).withPivot(2029));
        MonthNames english = english();
        assertEquals(english, MonthNames.ofPacked(english.packed()));
        assertEquals(english.hashCode(), MonthNames.ofPacked(english.packed()).hashCode());
        assertThrows(NullPointerException.class, () -> DetailedCast.date("1", DateOrder.YEAR_MONTH_DAY, null));
        assertThrows(
                NullPointerException.class,
                () -> DetailedCast.date("1", DateOrder.YEAR_MONTH_DAY, DateOptions.NONE, null));
        assertThrows(IllegalArgumentException.class, () -> MonthNames.builder().build());
    }

    private static MonthNames.Builder months(int upTo) {
        MonthNames.Builder builder = MonthNames.builder();
        String[] names = {"Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"};
        for (int i = 0; i < upTo; i++) {
            builder.add(i + 1, names[i]);
        }
        return builder;
    }

    @Test
    void aFoldedDuplicateWinsOverAMissingMonthAsInTheCore() {
        // The core judges a duplicate entry by entry, then asks that every month has a name.
        // Eleven months and a folded repeat of January: the duplicate is the rule that fails.
        IllegalArgumentException both = assertThrows(
                IllegalArgumentException.class, () -> months(11).add(2, "JAN").build());
        assertTrue(both.getMessage().contains("fold to the same spelling"), both.getMessage());
        assertThrows(
                IllegalArgumentException.class, () -> months(11).add(1, "jan").build());
        // Only a month missing: that rule, with its month named; nothing at all missing the lot.
        IllegalArgumentException missing =
                assertThrows(IllegalArgumentException.class, () -> months(11).build());
        assertTrue(missing.getMessage().contains("month 12 has none"), missing.getMessage());
        IllegalArgumentException none =
                assertThrows(IllegalArgumentException.class, () -> months(0).build());
        assertTrue(none.getMessage().contains("month 1 has none"), none.getMessage());
        // A duplicate in a whole vocabulary, as before.
        assertThrows(
                IllegalArgumentException.class, () -> months(12).add(5, "mAr").build());
        // The stand-in the check completes a short vocabulary with never collides with a name a
        // caller declares: this one is exactly what an unprotected stand-in for December would be.
        IllegalArgumentException odd = assertThrows(
                IllegalArgumentException.class,
                () -> months(11).add(1, "\u0001l").build());
        assertTrue(odd.getMessage().contains("month 12 has none"), odd.getMessage());
    }

    static MonthNames english() {
        MonthNames.Builder builder = MonthNames.builder();
        String[] names = {"Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"};
        for (int i = 0; i < names.length; i++) {
            builder.add(i + 1, names[i]);
        }
        return builder.add(9, "Sept").build();
    }
}
