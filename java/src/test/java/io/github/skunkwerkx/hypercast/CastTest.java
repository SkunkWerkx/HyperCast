package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
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

    @Test
    void dateTimeReadsTheMessyCivilShapes() {
        // The AM/PM world, zone-less: LocalDateTime because the text named no zone —
        // fusing one is the caller's job, never the parser's guess.
        assertEquals(new Success<>(LocalDateTime.of(2026, 1, 7, 15, 4)),
                Cast.dateTime("1/7/2026 3:04 PM", DateOrder.from(Locale.US)));
        assertEquals(new Success<>(LocalDateTime.of(2026, 7, 1, 15, 4)),
                Cast.dateTime("1/7/2026 3:04 PM", DateOrder.from(Locale.UK)));
        // A zone suffix is not this door's business — timestamp is the instant door.
        assertInstanceOf(Fault.class, Cast.dateTime("1/7/2026 15:04:05Z", DateOrder.MONTH_DAY_YEAR));
    }

    @Test
    void doorsStayCorrectAcrossThreadsOnPerThreadScratch() throws Exception {
        // The per-call confined arena became per-thread scratch (Cast.Scratch), which turns
        // "thread-confined" from a structural guarantee into a claim — so prove it. Each
        // thread casts values only it knows, thousands of times; any cross-thread bleed of
        // the shared out/fault/input segments surfaces as a wrong value or a stray fault.
        int threads = 8;
        int iterations = 2_000;
        ExecutorService pool = Executors.newFixedThreadPool(threads);
        List<Future<?>> running = new ArrayList<>();
        try {
            for (int t = 0; t < threads; t++) {
                int id = t;
                running.add(pool.submit(() -> {
                    for (int i = 0; i < iterations; i++) {
                        int mine = id * 1_000_000 + i;
                        assertEquals(new Success<>(mine),
                                Cast.i32(Integer.toString(mine), NumFormat.INVARIANT));
                        assertEquals(new Success<>(LocalDateTime.of(2026, 1, 7, 15, 4)),
                                Cast.dateTime("1/7/2026 3:04 PM", DateOrder.MONTH_DAY_YEAR));
                    }
                }));
            }
            for (Future<?> task : running) {
                task.get();
            }
        } finally {
            pool.shutdownNow();
        }
    }

    @Test
    void inputsPastTheScratchBufferStillCast() {
        // The scratch input buffer starts at 512 bytes and grows on demand; a token past
        // that boundary must cast identically, and the grown buffer must keep working for
        // the short inputs that follow it.
        String padded = " ".repeat(600) + "1234" + " ".repeat(600);
        assertEquals(new Success<>(1234), Cast.i32(padded, NumFormat.INVARIANT));
        assertEquals(new Success<>(7), Cast.i32("7", NumFormat.INVARIANT));

        // Long, and genuinely malformed: the fault span still indexes the caller's input.
        String longJunk = "x".repeat(1_000);
        switch (Cast.i32(longJunk, NumFormat.INVARIANT)) {
            case Fault<Integer> fault ->
                assertTrue(fault.offset() + fault.length() <= longJunk.length(),
                        "fault span escaped a " + longJunk.length() + "-byte input");
            case Success<Integer> success -> throw new AssertionError("junk parsed: " + success);
        }
    }

}
