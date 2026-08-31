package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalTime;
import java.util.Locale;
import java.util.UUID;
import org.junit.jupiter.api.Test;

/**
 * Binding-level behavior the corpus can't express: the sealed union's compiler-checked
 * consumption, locale-to-{@link NumFormat} bridging, {@code java.time} fidelity mapping
 * (full nanoseconds — the JVM keeps every digit the core parses), and the caller-bug
 * guards.
 */
final class CastTest {
    @Test
    void verdictSwitchIsExhaustiveWithTwoArms() {
        // This compiling at all is the feature: Verdict is sealed, so these two arms with
        // no default are proven exhaustive by javac.
        String rendered = switch (Cast.i32("42", NumFormat.INVARIANT)) {
            case Success<Integer> success -> "ok " + success.value();
            case Fault<Integer> fault -> "fault " + fault.reason();
        };
        assertEquals("ok 42", rendered);
    }

    @Test
    void verdictPatternMatchesCaseTypesDirectly() {
        assertTrue(Cast.bool("yes") instanceof Success<Boolean>(Boolean value) && value);
        assertTrue(Cast.bool("maybe") instanceof Fault<Boolean> fault
                && fault.reason() == CastFailure.MALFORMED);
    }

    @Test
    void numFormatBridgesFromARealLocale() {
        NumFormat german = NumFormat.from(Locale.GERMANY);
        assertEquals(',', german.decimalSeparator());
        assertEquals('.', german.groupSeparator());
        assertEquals(new Success<>(1234.5), Cast.f64("1.234,5", german));
    }

    @Test
    void stringDoorsTranscodeNonAsciiSeparators() {
        NumFormat french = new NumFormat(',', ' ', NumFormat.STYLE_ALL);
        assertEquals(new Success<>(1234.5), Cast.f64("1 234,5", french));
    }

    @Test
    void faultSpanPointsAtTheOffendingByte() {
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 4, 1), Cast.i32("  12x4", NumFormat.INVARIANT));
    }

    @Test
    void uuidAgreesWithThePlatformsOwnParser() {
        String text = "01020304-0506-0708-090a-0b0c0d0e0f10";
        assertEquals(new Success<>(UUID.fromString(text)), Cast.uuid(text));
        assertEquals(new Success<>(UUID.fromString(text)), Cast.uuid("urn:uuid:" + text));
    }

    @Test
    void timestampKeepsFullNanosecondFidelity() {
        // The core's ninth fractional digit survives — no tick truncation on the JVM.
        assertEquals(
                new Success<>(Instant.parse("2026-01-02T15:04:05.123456789Z")),
                Cast.timestamp("2026-01-02T15:04:05.123456789Z"));
        // Offset input normalizes to UTC — 15:04:05+05:00 is 10:04:05Z.
        assertEquals(
                new Success<>(Instant.parse("2026-01-02T10:04:05Z")),
                Cast.timestamp("2026-01-02T15:04:05+05:00"));
    }

    @Test
    void unixMapsTheDeclaredPrecision() {
        assertEquals(new Success<>(Instant.ofEpochSecond(-1)), Cast.unix("-1", UnixPrecision.SECONDS));
        assertEquals(
                new Success<>(Instant.ofEpochMilli(1_700_000_000_123L)),
                Cast.unix("1700000000123", UnixPrecision.MILLISECONDS));
        assertEquals(
                new Success<>(Instant.ofEpochSecond(1_700_000_000L, 123_456_789L)),
                Cast.unix("1700000000123456789", UnixPrecision.NANOSECONDS));
    }

    @Test
    void dateTimeAndDurationMapToJavaTimeTypes() {
        assertEquals(new Success<>(LocalDate.of(2026, 1, 2)), Cast.date("2026-01-02"));
        assertEquals(new Success<>(LocalTime.of(15, 4, 5, 123_456_789)), Cast.time("15:04:05.123456789"));
        assertEquals(new Success<>(Duration.ofHours(30)), Cast.duration("P1DT6H"));
        assertEquals(new Success<>(Duration.ofMillis(-1500)), Cast.duration("-1.5s"));
        assertEquals(new Success<>(Duration.ofMinutes(90)), Cast.duration("01:30"));
    }

    @Test
    void unsignedDoorsWidenToTheirDocumentedCarriers() {
        assertEquals(new Success<>(255), Cast.u8("255", NumFormat.INVARIANT));
        assertEquals(new Success<>(65535), Cast.u16("65535", NumFormat.INVARIANT));
        assertEquals(new Success<>(4_294_967_295L), Cast.u32("4294967295", NumFormat.INVARIANT));
        // u64::MAX arrives as the two's-complement bit pattern.
        assertEquals(new Success<>(-1L), Cast.u64("18446744073709551615", NumFormat.INVARIANT));
        assertEquals("18446744073709551615",
                switch (Cast.u64("18446744073709551615", NumFormat.INVARIANT)) {
                    case Success<Long> s -> Long.toUnsignedString(s.value());
                    case Fault<Long> f -> f.toString();
                });
    }

    @Test
    void optionalPresentsEmptyAsAbsentAndEverythingElseVerbatim() {
        assertTrue(Cast.optional(Cast.i32("   ", NumFormat.INVARIANT)).isEmpty());
        assertEquals(new Success<>(42), Cast.optional(Cast.i32("42", NumFormat.INVARIANT)).orElseThrow());
        assertInstanceOf(Fault.class, Cast.optional(Cast.i32("abc", NumFormat.INVARIANT)).orElseThrow());
    }

    @Test
    void equalSeparatorsAreACallerBugNotAVerdict() {
        assertThrows(IllegalArgumentException.class, () -> new NumFormat('.', '.', NumFormat.STYLE_ALL));
    }
    @Test
    void dateOrderDisambiguatesLikeTheCulturesDo() {
        // The canonical ambiguity: 1/7/2026 is January 7th under en-US's month-first short
        // dates and July 1st under en-GB's day-first ones — resolved only by declaration,
        // with the declaration derived from the real locales' own short-date patterns.
        DateOrder enUs = DateOrder.from(Locale.US);
        DateOrder enGb = DateOrder.from(Locale.UK);
        assertEquals(DateOrder.MONTH_DAY_YEAR, enUs);
        assertEquals(DateOrder.DAY_MONTH_YEAR, enGb);
        assertEquals(new Success<>(LocalDate.of(2026, 1, 7)), Cast.date("1/7/2026", enUs));
        assertEquals(new Success<>(LocalDate.of(2026, 7, 1)), Cast.date("1/7/2026", enGb));
        // Undeclared, the door stays strict ISO — the ambiguity is never guessed at.
        assertEquals(new Fault<LocalDate>(CastFailure.MALFORMED, 0, 8), Cast.date("1/7/2026"));
    }

}
