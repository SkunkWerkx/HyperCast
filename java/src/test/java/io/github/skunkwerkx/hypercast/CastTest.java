package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.text.DecimalFormat;
import java.text.DecimalFormatSymbols;
import java.text.NumberFormat;
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
        assertTrue(Cast.bool("maybe") instanceof Fault<Boolean> fault && fault.reason() == CastFailure.MALFORMED);
    }

    @Test
    void characterReadsVerbatimOrDeclaredCodePoints() {
        assertEquals(new Success<>('A'), Cast.character("A"));
        assertEquals(new Success<>(' '), Cast.character(" "));
        assertEquals(new Success<>('\u00e9'), Cast.character("&#233;"));
        assertEquals(new Success<>('\u00e9'), Cast.character("  u+00E9  ".getBytes(StandardCharsets.UTF_8)));
        assertEquals(new Fault<Character>(CastFailure.EMPTY, 0, 0), Cast.character(""));
        assertEquals(new Fault<Character>(CastFailure.MALFORMED, 2, 1), Cast.character("65x"));
    }

    @Test
    void characterKeepsALoneSurrogateVerbatim() {
        // One UTF-16 code unit is the char as-is, never transcoded to U+FFFD on the way in.
        assertEquals(new Success<>('\uD800'), Cast.character("\uD800"));
        assertEquals(new Success<>('\uDFFF'), Cast.character("\uDFFF"));
        // Spelled as a code point, a surrogate names no scalar: the core's out-of-range.
        assertEquals(new Fault<Character>(CastFailure.OUT_OF_RANGE, 0, 6), Cast.character("U+D800"));
    }

    @Test
    void characterFaultsASupplementaryScalarInTheCallersUnits() {
        // "😀" is two chars in a String, four bytes in UTF-8: the span covers it whole in each.
        assertEquals(new Fault<Character>(CastFailure.OUT_OF_RANGE, 0, 2), Cast.character("\uD83D\uDE00"));
        assertEquals(
                new Fault<Character>(CastFailure.OUT_OF_RANGE, 0, 4),
                Cast.character("\uD83D\uDE00".getBytes(StandardCharsets.UTF_8)));
        // The trimmed edges stay outside the span.
        assertEquals(new Fault<Character>(CastFailure.OUT_OF_RANGE, 2, 7), Cast.character("  U+1F600 "));
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment text = arena.allocateFrom("\t&#x1F600;\n");
            assertEquals(
                    new Fault<Character>(CastFailure.OUT_OF_RANGE, 1, 9),
                    Cast.character(text.asSlice(0, text.byteSize() - 1)));
        }
    }

    @Test
    void numFormatBridgesFromARealLocale() {
        NumFormat german = NumFormat.from(Locale.GERMANY);
        assertEquals(",", german.decimalSeparator());
        assertEquals(".", german.groupSeparator());
        assertEquals(new Success<>(1234.5), Cast.f64("1.234,5", german));
        // What the core reads anyway is left undeclared: an ASCII minus and percent, a '0'
        // zero digit, monetary separators equal to the number ones, and no plus sign at all.
        assertEquals(new NumFormat(",", ".", NumFormat.STYLE_ALL, "€"), german);
    }

    @Test
    void everyDeclaredSymbolIsReadBesideTheAsciiOne() {
        // Nothing that parsed before stops parsing: the ASCII spelling stays read beside the
        // declared one, in every field.
        NumFormat minus = NumFormat.INVARIANT.withSigns("\u2212", "");
        assertEquals(new Success<>(-5), Cast.i32("\u22125", minus));
        assertEquals(new Success<>(-5), Cast.i32("-5", minus));
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 0, 1), Cast.i32("\u22125", NumFormat.INVARIANT));
        NumFormat percent = NumFormat.INVARIANT.withPercentSymbol("\u066a");
        assertEquals(new Success<>(0.5), Cast.f64("50\u066a", percent));
        assertEquals(new Success<>(0.5), Cast.f64("50%", percent));
        NumFormat digits = NumFormat.INVARIANT.withDigitZero(0x0660);
        assertEquals(new Success<>(123), Cast.i32("\u0661\u0662\u0663", digits));
        assertEquals(new Success<>(123), Cast.i32("12\u0663", digits));
        assertEquals(new Success<>(1000), Cast.i32("\u0661e\u0663", digits));
        // A trailing sign is opt-in, never assumed.
        NumFormat trailing = NumFormat.INVARIANT.withStyles(NumFormat.STYLE_ALL | NumFormat.STYLE_TRAILING_SIGN);
        assertEquals(new Success<>(-5), Cast.i32("5-", trailing));
        assertEquals(new Success<>(-5), Cast.i32("5 -", trailing));
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 1, 1), Cast.i32("5-", NumFormat.INVARIANT));
        // The currency separators are read when the symbol is in the text, the number ones otherwise.
        NumFormat swiss = new NumFormat(",", ".", NumFormat.STYLE_ALL, "€").withCurrencySeparators(".", ",");
        assertEquals(new Success<>(1234.5), Cast.f64("€1,234.50", swiss));
        assertEquals(new Success<>(1234.5), Cast.f64("1.234,50", swiss));
        assertEquals(new Fault<Double>(CastFailure.MALFORMED, 5, 1), Cast.f64("1,234.50", swiss));
        // A separator may be two characters.
        NumFormat spaced = new NumFormat(".", ", ", NumFormat.STYLE_ALL);
        assertEquals(new Success<>(1234), Cast.i32("1, 234", spaced));
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 1, 1), Cast.i32("1,234", spaced));
    }

    @Test
    void theLocaleBridgeFillsEveryField() {
        // Estonian writes its minus as U+2212; ASCII '-' stays read beside it.
        NumFormat estonian = NumFormat.from(Locale.forLanguageTag("et-EE"));
        assertEquals("\u2212", estonian.minusSign());
        assertEquals(new Success<>(-5), Cast.i32("\u22125", Locale.forLanguageTag("et-EE")));
        assertEquals(new Success<>(-5), Cast.i32("-5", Locale.forLanguageTag("et-EE")));
        // Swiss French numbers take a decimal comma, Swiss French currency a decimal point.
        Locale swissFrench = Locale.forLanguageTag("fr-CH");
        NumFormat swiss = NumFormat.from(swissFrench);
        assertEquals(",", swiss.decimalSeparator());
        assertEquals(".", swiss.currencyDecimalSeparator());
        assertEquals("", swiss.currencyGroupSeparator());
        assertEquals("CHF", swiss.currencySymbol());
        assertEquals(new Success<>(1234.5), Cast.f64("CHF 1\u202f234.50", swissFrench));
        assertEquals(new Success<>(1234.5), Cast.f64("1\u202f234,50", swissFrench));
        // Austrian German groups numbers with a no-break space and currency with a dot.
        assertEquals(".", NumFormat.from(Locale.forLanguageTag("de-AT")).currencyGroupSeparator());
        assertEquals(new Success<>(1234.5), Cast.f64("€ 1.234,50", Locale.forLanguageTag("de-AT")));
        // Egyptian Arabic: native digits, its own separators and percent symbol.
        Locale egypt = Locale.forLanguageTag("ar-EG");
        NumFormat arabic = NumFormat.from(egypt);
        assertEquals(0x0660, arabic.digitZero());
        assertEquals("\u066a", arabic.percentSymbol());
        assertEquals("\u066b", arabic.decimalSeparator());
        assertEquals("\u066c", arabic.groupSeparator());
        assertEquals(new Success<>(1234.5), Cast.f64("\u0661\u066c\u0662\u0663\u0664\u066b\u0665", egypt));
        assertEquals(new Success<>(new BigDecimal("0.5")), Cast.decimal("50\u066a", egypt));
        assertEquals(new Success<>(12), Cast.i32("12", egypt));
        // The JDK's own data writes no locale's negative with a suffix alone, so the trailing
        // sign comes from a DecimalFormat the caller shaped — the same bridge from(Locale) runs.
        assertEquals(NumFormat.STYLE_ALL, NumFormat.from(Locale.US).styles());
        DecimalFormat suffixed = (DecimalFormat) NumberFormat.getInstance(Locale.US);
        suffixed.setNegativePrefix("");
        suffixed.setNegativeSuffix("-");
        NumFormat trailing = NumFormat.from(suffixed);
        assertEquals(NumFormat.STYLE_ALL | NumFormat.STYLE_TRAILING_SIGN, trailing.styles());
        assertEquals(new Success<>(-1234), Cast.i32("1,234-", trailing));
        assertEquals(new Success<>(-1234), Cast.i32("-1,234", trailing));
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 5, 1), Cast.i32("1,234-", Locale.US));
        // Every locale the JDK knows derives: nothing in its data is a contract violation.
        for (Locale locale : Locale.getAvailableLocales()) {
            assertDoesNotThrow(() -> NumFormat.from(locale), locale.toLanguageTag());
        }
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
        assertEquals(new Success<>(Instant.parse("2026-01-02T10:04:05Z")), Cast.timestamp("2026-01-02T15:04:05+05:00"));
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
        assertEquals(
                "18446744073709551615",
                switch (Cast.u64("18446744073709551615", NumFormat.INVARIANT)) {
                    case Success<Long> s -> Long.toUnsignedString(s.value());
                    case Fault<Long> f -> f.toString();
                });
    }

    @Test
    void optionalPresentsEmptyAsAbsentAndEverythingElseVerbatim() {
        assertTrue(Cast.optional(Cast.i32("   ", NumFormat.INVARIANT)).isEmpty());
        assertEquals(
                new Success<>(42),
                Cast.optional(Cast.i32("42", NumFormat.INVARIANT)).orElseThrow());
        assertInstanceOf(
                Fault.class, Cast.optional(Cast.i32("abc", NumFormat.INVARIANT)).orElseThrow());
    }

    @Test
    void equalSeparatorsAreACallerBugNotAVerdict() {
        assertThrows(IllegalArgumentException.class, () -> new NumFormat('.', '.', NumFormat.STYLE_ALL));
        assertThrows(IllegalArgumentException.class, () -> new NumFormat(", ", ", ", NumFormat.STYLE_ALL));
        // The pair in force for a currency amount must differ too: a declared currency group
        // separator equal to the number decimal separator it is read beside.
        assertThrows(
                IllegalArgumentException.class,
                () -> new NumFormat(".", ",", NumFormat.STYLE_ALL, "€").withCurrencySeparators("", "."));
        assertThrows(
                IllegalArgumentException.class,
                () -> new NumFormat(".", ",", NumFormat.STYLE_ALL, "€").withCurrencySeparators("'", "'"));
    }

    @Test
    void malformedSymbolsAreACallerBugNotAVerdict() {
        // The core's rules, enforced before the crossing: a separator 1 to 8 UTF-8 bytes, a
        // sign or percent symbol at most 8 with no ASCII whitespace, every symbol whole code
        // points with no ASCII digit, and a digit zero that starts a digit block.
        assertThrows(IllegalArgumentException.class, () -> new NumFormat("", ",", NumFormat.STYLE_ALL));
        assertThrows(IllegalArgumentException.class, () -> new NumFormat(".", "", NumFormat.STYLE_ALL));
        assertThrows(IllegalArgumentException.class, () -> new NumFormat(".", "€€€", NumFormat.STYLE_ALL));
        assertThrows(IllegalArgumentException.class, () -> new NumFormat(".", "1", NumFormat.STYLE_ALL));
        assertThrows(IllegalArgumentException.class, () -> new NumFormat(".", "\ud83d", NumFormat.STYLE_ALL));
        assertThrows(IllegalArgumentException.class, () -> NumFormat.INVARIANT.withSigns("- ", ""));
        assertThrows(IllegalArgumentException.class, () -> NumFormat.INVARIANT.withSigns("", "+1"));
        assertThrows(IllegalArgumentException.class, () -> NumFormat.INVARIANT.withSigns("", "€€€"));
        assertThrows(IllegalArgumentException.class, () -> NumFormat.INVARIANT.withPercentSymbol(" %"));
        assertThrows(IllegalArgumentException.class, () -> NumFormat.INVARIANT.withCurrencySeparators("1", ""));
        assertThrows(IllegalArgumentException.class, () -> NumFormat.INVARIANT.withDigitZero('0'));
        assertThrows(IllegalArgumentException.class, () -> NumFormat.INVARIANT.withDigitZero(0x00bf));
        assertThrows(IllegalArgumentException.class, () -> NumFormat.INVARIANT.withDigitZero(0xd800));
        assertThrows(IllegalArgumentException.class, () -> NumFormat.INVARIANT.withDigitZero(0x110000));
        assertThrows(NullPointerException.class, () -> NumFormat.INVARIANT.withSigns(null, ""));
        // The widest real ones fit: two-character separators, a sign with a bidi mark, every
        // digit block Unicode defines, fullwidth and mathematical digits included.
        assertDoesNotThrow(() -> new NumFormat(".", "' ", NumFormat.STYLE_ALL).withSigns("\u200e\u2212", "\uff0b"));
        assertDoesNotThrow(() -> NumFormat.INVARIANT.withDigitZero(0xff10).withDigitZero(0x1d7ce));
    }

    @Test
    void currencySymbolComesFromTheLocaleAndIsHonoredOnlyWhenDeclared() {
        NumFormat us = NumFormat.from(Locale.US);
        assertEquals("$", us.currencySymbol());
        assertEquals(new Success<>(1234.5), Cast.f64("$1,234.50", us));
        assertEquals(new Success<>(-5), Cast.i32("-$5", us));
        assertEquals(new Success<>(new BigDecimal("-1234.5")), Cast.decimal("($1,234.50)", us));
        // A trailing symbol, eurozone style, with the locale's own separators.
        assertEquals(new Success<>(1234.5), Cast.f64("1.234,50 €", NumFormat.from(Locale.GERMANY)));
        // Declared but not permitted: the symbol is the malformed span, not the whole token.
        NumFormat declaredOnly = new NumFormat('.', ',', NumFormat.STYLE_ALL & ~NumFormat.STYLE_CURRENCY, "$");
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 0, 1), Cast.i32("$5", declaredOnly));
        // The three-field constructor still declares nothing, and INVARIANT never did.
        assertEquals("", NumFormat.INVARIANT.currencySymbol());
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 0, 1), Cast.i32("$5", NumFormat.INVARIANT));
    }

    @Test
    void malformedCurrencySymbolsAreACallerBugNotAVerdict() {
        // The core's own rule, enforced before the crossing: no ASCII digit, no ASCII
        // whitespace, at most 16 UTF-8 bytes, whole code points.
        assertThrows(IllegalArgumentException.class, () -> new NumFormat('.', ',', NumFormat.STYLE_ALL, "$1"));
        assertThrows(IllegalArgumentException.class, () -> new NumFormat('.', ',', NumFormat.STYLE_ALL, "US $"));
        assertThrows(IllegalArgumentException.class, () -> new NumFormat('.', ',', NumFormat.STYLE_ALL, "€".repeat(6)));
        assertThrows(IllegalArgumentException.class, () -> new NumFormat('.', ',', NumFormat.STYLE_ALL, "\ud83d"));
        // The widest real symbols fit: "€" ×5 is 15 bytes.
        assertEquals("€€€€€", new NumFormat('.', ',', NumFormat.STYLE_ALL, "€".repeat(5)).currencySymbol());
    }

    @Test
    void decimalIsExactAndCanonical() {
        // BigDecimal.equals is scale-sensitive, so these pin the scale as well as the value:
        // exact trailing fraction zeros are trimmed, so the scale is minimal.
        assertEquals(new Success<>(new BigDecimal("0.1")), Cast.decimal("0.1", NumFormat.INVARIANT));
        assertEquals(new Success<>(new BigDecimal("1.1")), Cast.decimal("1.10", NumFormat.INVARIANT));
        assertEquals(new Success<>(new BigDecimal("1.1")), Cast.decimal("1.1", NumFormat.INVARIANT));
        assertEquals(new Success<>(new BigDecimal("1.1")), Cast.decimal("1.1000", NumFormat.INVARIANT));
        assertEquals(new Success<>(new BigDecimal("12345.5")), Cast.decimal("12,345.50", NumFormat.INVARIANT));
        assertEquals(new Success<>(new BigDecimal("0.5")), Cast.decimal("50%", NumFormat.INVARIANT));
        // Integer zeros are never touched: 100 stays 100 / scale 0.
        assertEquals(new Success<>(new BigDecimal("100")), Cast.decimal("100", NumFormat.INVARIANT));
        assertEquals(new Success<>(new BigDecimal("-0.025")), Cast.decimal("(2.5)%", NumFormat.INVARIANT));
        assertEquals(new Success<>(new BigDecimal("2500")), Cast.decimal("2.5e3", NumFormat.INVARIANT));
        // The full 96 bits: 2^96 - 1 is the largest magnitude the core carries.
        assertEquals(
                new Success<>(new BigDecimal("79228162514264337593543950335")),
                Cast.decimal("79228162514264337593543950335", NumFormat.INVARIANT));
        assertEquals(
                new Fault<BigDecimal>(CastFailure.OUT_OF_RANGE, 0, 29),
                Cast.decimal("79228162514264337593543950336", NumFormat.INVARIANT));
        // Zero is never negative, whatever the text said, and is always scale 0.
        switch (Cast.decimal("-0.00", NumFormat.INVARIANT)) {
            case Success<BigDecimal> s -> {
                assertEquals(0, s.value().signum());
                assertEquals(0, s.value().scale());
            }
            case Fault<BigDecimal> f -> throw new AssertionError("zero faulted: " + f);
        }
    }

    @Test
    void nativeVersionIsThisBindingsOwn() {
        // The probe decodes the packed major.minor.patch of the core actually loaded — the
        // platform library or the wasm module — which must be the one this binding is built
        // against: build.gradle.kts hands its own version to the test JVM, and rust/Cargo.toml
        // moves with it.
        // A CI override may append a prerelease tag (a -ci.N suffix, per build.gradle.kts); the
        // core carries only major.minor.patch, so compare that much.
        var expected = System.getProperty("hypercast.version").split("-", 2)[0];
        assertEquals(expected, Cast.nativeVersion());
    }

    @Test
    void isAvailableAgreesWithNativeVersion() {
        // The suite only ever runs with a core staged, so the probe says so — and it says so
        // by the same crossing nativeVersion() makes, so the two cannot disagree.
        assertTrue(Cast.isAvailable());
        assertDoesNotThrow(Cast::nativeVersion);
    }

    @Test
    void stringDoorsReportCharOffsetsAndByteDoorsReportByteOffsets() {
        // The core faults in bytes. A String caller thinks in chars, so the span is rebased
        // for them and substring() names the offending text; the byte doors are the core's
        // own span, verbatim. ASCII input is identical either way and is never touched.
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 0, 1), Cast.i32("€x", NumFormat.INVARIANT));
        assertEquals(
                new Fault<Integer>(CastFailure.MALFORMED, 0, 3),
                Cast.i32("€x".getBytes(StandardCharsets.UTF_8), NumFormat.INVARIANT));
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 1, 1), Cast.i32("1€", NumFormat.INVARIANT));
        assertEquals(
                new Fault<Integer>(CastFailure.MALFORMED, 1, 3),
                Cast.i32("1€".getBytes(StandardCharsets.UTF_8), NumFormat.INVARIANT));
        // A supplementary code point is one UTF-8 sequence and two UTF-16 units.
        String astral = "12\uD83D\uDE00";
        switch (Cast.i32(astral, NumFormat.INVARIANT)) {
            case Fault<Integer> fault ->
                assertEquals("\uD83D\uDE00", astral.substring(fault.offset(), fault.offset() + fault.length()));
            case Success<Integer> success -> throw new AssertionError("emoji parsed: " + success);
        }
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 4, 1), Cast.i32("  12x4", NumFormat.INVARIANT));
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
    void dateTimeReadsADeclaredDesignatorBesideTheEnglishOnes() {
        // Korean designators, before the clock as Korean writes them, from the locale's own data.
        DateTimeFormat korean = DateTimeFormat.from(Locale.KOREA);
        assertEquals(new DateTimeFormat(DateOrder.YEAR_MONTH_DAY, "오전", "오후"), korean);
        assertEquals(new Success<>(LocalDateTime.of(2026, 1, 7, 15, 4)), Cast.dateTime("2026-01-07 오후 3:04", korean));
        assertEquals(new Success<>(LocalDateTime.of(2026, 1, 7, 0, 4)), Cast.dateTime("2026-01-07 오전 12:04", korean));
        // English stays read beside them, in either position; the order alone reads no Korean.
        assertEquals(new Success<>(LocalDateTime.of(2026, 1, 7, 15, 4)), Cast.dateTime("2026-01-07 3:04 pm", korean));
        assertEquals(new Success<>(LocalDateTime.of(2026, 1, 7, 15, 4)), Cast.dateTime("2026-01-07 PM 3:04", korean));
        assertEquals(
                new Fault<LocalDateTime>(CastFailure.MALFORMED, 11, 1),
                Cast.dateTime("2026-01-07 오후 3:04", DateOrder.YEAR_MONTH_DAY));
        // Spanish, with a space inside the designator; German designators declared by hand,
        // since the JDK's data for de-DE is the English pair, which from(Locale) leaves undeclared.
        // (The JDK's CLDR data spells them with a no-break space, which the core reads: only
        // ASCII whitespace at a designator's edges is refused.)
        Locale spain = Locale.forLanguageTag("es-ES");
        assertEquals(
                new DateTimeFormat(DateOrder.DAY_MONTH_YEAR, "a.\u00a0m.", "p.\u00a0m."), DateTimeFormat.from(spain));
        assertEquals(
                new Success<>(LocalDateTime.of(2026, 1, 7, 15, 4)), Cast.dateTime("7/1/2026 3:04 p.\u00a0m.", spain));
        assertEquals(
                new Success<>(LocalDateTime.of(2026, 1, 7, 15, 4)),
                Cast.dateTime("7/1/2026 3:04 p. m.", new DateTimeFormat(DateOrder.DAY_MONTH_YEAR, "a. m.", "p. m.")));
        assertEquals(new DateTimeFormat(DateOrder.DAY_MONTH_YEAR), DateTimeFormat.from(Locale.GERMANY));
        assertEquals(
                new Success<>(LocalDateTime.of(2026, 1, 7, 15, 4)), Cast.dateTime("7.1.2026 3:04 PM", Locale.GERMANY));
        DateTimeFormat german = new DateTimeFormat(DateOrder.DAY_MONTH_YEAR, "vorm.", "nachm.");
        assertEquals(new Success<>(LocalDateTime.of(2026, 1, 7, 15, 4)), Cast.dateTime("7.1.2026 3:04 nachm.", german));
        assertEquals(
                new Fault<LocalDateTime>(CastFailure.MALFORMED, 13, 1),
                Cast.dateTime("7.1.2026 3:04 nachm.", DateOrder.DAY_MONTH_YEAR));
        // A designator the layout cannot hold is left undeclared, not refused: Nepali's are
        // 27 and 21 UTF-8 bytes.
        assertEquals(
                new DateTimeFormat(DateOrder.from(Locale.forLanguageTag("ne-NP"))),
                DateTimeFormat.from(Locale.forLanguageTag("ne-NP")));
        for (Locale locale : Locale.getAvailableLocales()) {
            assertDoesNotThrow(() -> DateTimeFormat.from(locale), locale.toLanguageTag());
        }
    }

    @Test
    void dateTimeLocaleOverloadsAreTheBridgeExactly() {
        List<String> inputs =
                List.of("2026-01-07 오후 3:04", "2026-01-07 3:04 PM", "2026-01-07T15:04:05.5", "1/7/2026", "nope", "");
        for (Locale locale : List.of(Locale.KOREA, Locale.US, Locale.forLanguageTag("es-ES"))) {
            DateTimeFormat format = DateTimeFormat.from(locale);
            for (String text : inputs) {
                byte[] utf8 = text.getBytes(StandardCharsets.UTF_8);
                assertEquals(Cast.dateTime(text, format), Cast.dateTime(text, locale));
                assertEquals(Cast.dateTime(utf8, format), Cast.dateTime(utf8, locale));
                assertEquals(
                        Cast.dateTime(MemorySegment.ofArray(utf8), format),
                        Cast.dateTime(MemorySegment.ofArray(utf8), locale));
            }
        }
        assertSame(DateTimeFormat.from(Locale.KOREA), DateTimeFormat.from(Locale.KOREA));
        assertThrows(NullPointerException.class, () -> Cast.dateTime("1/7/2026", (Locale) null));
        assertThrows(NullPointerException.class, () -> Cast.dateTime("1/7/2026", (DateTimeFormat) null));
    }

    @Test
    void malformedDesignatorsAreACallerBugNotAVerdict() {
        // The core's rules, enforced before the crossing: at most 16 UTF-8 bytes, no ASCII
        // digit, no ASCII whitespace at either end — a space inside is fine.
        assertThrows(IllegalArgumentException.class, () -> new DateTimeFormat(DateOrder.MONTH_DAY_YEAR, "am", "pm1"));
        assertThrows(IllegalArgumentException.class, () -> new DateTimeFormat(DateOrder.MONTH_DAY_YEAR, " am", "pm"));
        assertThrows(IllegalArgumentException.class, () -> new DateTimeFormat(DateOrder.MONTH_DAY_YEAR, "am", "pm "));
        assertThrows(
                IllegalArgumentException.class,
                () -> new DateTimeFormat(DateOrder.MONTH_DAY_YEAR, "ante meridiem now", "pm"));
        assertThrows(
                IllegalArgumentException.class, () -> new DateTimeFormat(DateOrder.MONTH_DAY_YEAR, "\ud83d", "pm"));
        assertThrows(NullPointerException.class, () -> new DateTimeFormat(null, "am", "pm"));
        assertThrows(NullPointerException.class, () -> new DateTimeFormat(DateOrder.MONTH_DAY_YEAR, null, "pm"));
        assertDoesNotThrow(() -> new DateTimeFormat(DateOrder.MONTH_DAY_YEAR, "a. m.", "p. m."));
        assertDoesNotThrow(() -> new DateTimeFormat(DateOrder.MONTH_DAY_YEAR, "ante meridiem.", ""));
    }

    @Test
    void dateTimeReadsTheMessyCivilShapes() {
        // The AM/PM world, zone-less: LocalDateTime because the text named no zone —
        // fusing one is the caller's job, never the parser's guess.
        assertEquals(
                new Success<>(LocalDateTime.of(2026, 1, 7, 15, 4)),
                Cast.dateTime("1/7/2026 3:04 PM", DateOrder.from(Locale.US)));
        assertEquals(
                new Success<>(LocalDateTime.of(2026, 7, 1, 15, 4)),
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
                        assertEquals(new Success<>(mine), Cast.i32(Integer.toString(mine), NumFormat.INVARIANT));
                        assertEquals(
                                new Success<>(LocalDateTime.of(2026, 1, 7, 15, 4)),
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
    void longInputsStillCast() {
        // The input crosses as a pinned view of the caller's own array, whatever its size —
        // there is no staging buffer to outgrow — so a token far past any plausible scratch
        // size must cast identically, and short inputs after it must keep working.
        String padded = " ".repeat(600) + "1234" + " ".repeat(600);
        assertEquals(new Success<>(1234), Cast.i32(padded, NumFormat.INVARIANT));
        assertEquals(new Success<>(7), Cast.i32("7", NumFormat.INVARIANT));

        // Long, and genuinely malformed: the fault span still indexes the caller's input.
        String longJunk = "x".repeat(1_000);
        switch (Cast.i32(longJunk, NumFormat.INVARIANT)) {
            case Fault<Integer> fault ->
                assertTrue(
                        fault.offset() + fault.length() <= longJunk.length(),
                        "fault span escaped a " + longJunk.length() + "-byte input");
            case Success<Integer> success -> throw new AssertionError("junk parsed: " + success);
        }
    }

    @Test
    void memorySegmentDoorsCastASliceWithoutCopying() {
        // One buffer holding several values — the round-three shape — with each door handed
        // a heap slice, not a copied-out array. Fault spans index the slice, not the buffer.
        byte[] line = "1,234|2026-01-07T15:04:05Z|not-a-uuid|true".getBytes(java.nio.charset.StandardCharsets.UTF_8);
        MemorySegment whole = MemorySegment.ofArray(line);
        assertEquals(new Success<>(1234), Cast.i32(whole.asSlice(0, 5), NumFormat.INVARIANT));
        assertEquals(new Success<>(Instant.parse("2026-01-07T15:04:05Z")), Cast.timestamp(whole.asSlice(6, 20)));
        assertEquals(new Fault<>(CastFailure.MALFORMED, 0, 10), Cast.uuid(whole.asSlice(27, 10)));
        assertEquals(new Success<>(true), Cast.bool(whole.asSlice(38, 4)));
    }

    @Test
    void memorySegmentDoorsAcceptNativeMemoryAndEmptySlices() {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment off = arena.allocateFrom("42.5");
            // allocateFrom appends a NUL terminator; the door gets exactly the text's bytes.
            assertEquals(new Success<>(42.5), Cast.f64(off.asSlice(0, 4), NumFormat.INVARIANT));
            assertEquals(new Fault<>(CastFailure.EMPTY, 0, 0), Cast.f64(off.asSlice(0, 0), NumFormat.INVARIANT));
        }
    }

    @Test
    void uuidDoorReadsTheSixteenBytesInRfcOrder() {
        // Two big-endian long reads must produce the same UUID as the byte-at-a-time
        // decomposition did, including the top bit set in both halves.
        assertEquals(
                new Success<>(UUID.fromString("f1e2d3c4-b5a6-9788-8968-5a4b3c2d1e0f")),
                Cast.uuid("f1e2d3c4-b5a6-9788-8968-5a4b3c2d1e0f"));
    }

    /** {@code 1<group>234<decimal>5} spelled in the locale's own separators. */
    private static String grouped(Locale locale) {
        DecimalFormatSymbols symbols = DecimalFormatSymbols.getInstance(locale);
        return "1" + symbols.getGroupingSeparator() + "234" + symbols.getDecimalSeparator() + "5";
    }

    @Test
    void localeOverloadsReadTheLocalesOwnNotation() {
        // fr-FR groups with a no-break space (U+202F in current CLDR data), de-DE with a dot.
        for (Locale locale : List.of(Locale.US, Locale.GERMANY, Locale.FRANCE)) {
            String text = grouped(locale);
            byte[] utf8 = text.getBytes(StandardCharsets.UTF_8);
            assertEquals(new Success<>(1234.5), Cast.f64(text, locale), locale.toString());
            assertEquals(new Success<>(1234.5f), Cast.f32(text, locale), locale.toString());
            assertEquals(new Success<>(new BigDecimal("1234.5")), Cast.decimal(utf8, locale), locale.toString());
            try (Arena arena = Arena.ofConfined()) {
                MemorySegment segment =
                        arena.allocateFrom(text, StandardCharsets.UTF_8).asSlice(0, utf8.length);
                assertEquals(new Success<>(1234.5), Cast.f64(segment, locale), locale.toString());
            }
            String integer = text.substring(0, text.length() - 2);
            assertEquals(new Success<>(1234), Cast.i32(integer, locale), locale.toString());
            assertEquals(new Success<>(1234L), Cast.i64(integer.getBytes(StandardCharsets.UTF_8), locale));
        }
    }

    @Test
    void localeOverloadsAreTheBridgeExactly() {
        // Every overload is its NumFormat door under NumFormat.from(locale): same verdict, fault
        // spans included, success or not.
        List<String> inputs = List.of("1,234", "1.234", "(12)", "1e3", "0x7F", "50%", "€5", "12,5", "nope", "");
        for (Locale locale : List.of(Locale.US, Locale.GERMANY, Locale.FRANCE)) {
            NumFormat format = NumFormat.from(locale);
            for (String text : inputs) {
                byte[] utf8 = text.getBytes(StandardCharsets.UTF_8);
                assertEquals(Cast.i8(text, format), Cast.i8(text, locale));
                assertEquals(Cast.i16(utf8, format), Cast.i16(utf8, locale));
                assertEquals(Cast.i32(text, format), Cast.i32(text, locale));
                assertEquals(Cast.i64(text, format), Cast.i64(text, locale));
                assertEquals(Cast.u8(text, format), Cast.u8(text, locale));
                assertEquals(Cast.u16(text, format), Cast.u16(text, locale));
                assertEquals(Cast.u32(utf8, format), Cast.u32(utf8, locale));
                assertEquals(Cast.u64(text, format), Cast.u64(text, locale));
                assertEquals(Cast.f32(text, format), Cast.f32(text, locale));
                assertEquals(Cast.f64(utf8, format), Cast.f64(utf8, locale));
                assertEquals(Cast.decimal(text, format), Cast.decimal(text, locale));
            }
        }
    }

    @Test
    void aNullLocaleIsRefusedNotDefaulted() {
        assertThrows(NullPointerException.class, () -> Cast.i32("1", (Locale) null));
        assertThrows(NullPointerException.class, () -> Cast.decimal(new byte[] {'1'}, (Locale) null));
        assertThrows(NullPointerException.class, () -> NumFormat.from((Locale) null));
    }

    @Test
    void aLocalesFormatIsDerivedOnceAndShared() {
        assertSame(NumFormat.from(Locale.GERMANY), NumFormat.from(Locale.GERMANY));
        assertEquals(NumFormat.from(Locale.FRANCE), NumFormat.from(Locale.forLanguageTag("fr-FR")));
    }
}
