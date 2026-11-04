package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.text.DecimalFormat;
import java.text.DecimalFormatSymbols;
import java.text.NumberFormat;
import java.util.List;
import java.util.Locale;
import org.junit.jupiter.api.Test;

/**
 * Strict grouping: the policy a {@link NumFormat} declares apart from its styles, what the
 * bridge reads from the platform's own pattern, and the core's rules at the doors.
 */
final class GroupingTest {
    @Test
    void theDefaultIsStrictWesternAndLegacyAndIndianAreExplicit() {
        assertEquals(Grouping.WESTERN, NumFormat.INVARIANT.grouping());
        assertEquals(Grouping.WESTERN, NumFormat.DETECT.grouping());
        assertEquals(Grouping.WESTERN, new NumFormat(".", ",", NumFormat.STYLE_ALL, Grouping.WESTERN).grouping());
        assertEquals(Grouping.LEGACY, new NumFormat(".", ",", NumFormat.STYLE_ALL, Grouping.LEGACY).grouping());
        assertEquals(Grouping.INDIAN, new NumFormat('.', ',', NumFormat.STYLE_ALL, Grouping.INDIAN).grouping());
        assertEquals(
                Grouping.INDIAN, new NumFormat(".", ",", NumFormat.STYLE_ALL, "\u20b9", Grouping.INDIAN).grouping());
        assertEquals(
                new NumFormat(".", ",", NumFormat.STYLE_ALL, "$"),
                new NumFormat(".", ",", NumFormat.STYLE_ALL, "$", Grouping.WESTERN));
    }

    @Test
    void aPolicyReplacesAndNeverJoinsTheOneBeforeIt() {
        NumFormat format = NumFormat.INVARIANT;
        assertEquals(Grouping.INDIAN, format.withGrouping(Grouping.INDIAN).grouping());
        assertEquals(
                Grouping.LEGACY,
                format.withGrouping(Grouping.INDIAN)
                        .withGrouping(Grouping.LEGACY)
                        .grouping());
        assertEquals(
                Grouping.WESTERN,
                format.withGrouping(Grouping.LEGACY)
                        .withGrouping(Grouping.WESTERN)
                        .grouping());
        for (Grouping grouping : Grouping.values()) {
            // One policy in the ABI word, whatever was declared before: bits 8 and 9 are the policy.
            int flags =
                    format.withGrouping(Grouping.INDIAN).withGrouping(grouping).flags();
            assertEquals(grouping.code(), flags & Grouping.POLICY_MASK);
            assertEquals(NumFormat.STYLE_ALL, flags & ~Grouping.POLICY_MASK);
        }
    }

    @Test
    void aStyleMaskNeverCarriesAPolicyOrAnUndefinedBit() {
        assertThrows(
                IllegalArgumentException.class,
                () -> new NumFormat(".", ",", NumFormat.STYLE_ALL | Grouping.INDIAN.code(), Grouping.WESTERN));
        assertThrows(IllegalArgumentException.class, () -> new NumFormat(".", ",", 1 << 10, Grouping.WESTERN));
        assertThrows(IllegalArgumentException.class, () -> new NumFormat(".", ",", 1 << 31));
        assertThrows(IllegalArgumentException.class, () -> NumFormat.INVARIANT.withStyles(1 << 8));
        assertThrows(NullPointerException.class, () -> NumFormat.INVARIANT.withGrouping(null));
        assertThrows(NullPointerException.class, () -> new NumFormat(".", ",", 0, (Grouping) null));
        // Policy 3 names none; every defined policy round-trips through its ABI bits.
        assertThrows(IllegalArgumentException.class, () -> Grouping.of(3 << 8));
        assertThrows(IllegalArgumentException.class, () -> Grouping.of(1));
        for (Grouping grouping : Grouping.values()) {
            assertEquals(grouping, Grouping.of(grouping.code()));
        }
        assertEquals(0, Grouping.LEGACY.code());
        assertEquals(1 << 8, Grouping.WESTERN.code());
        assertEquals(2 << 8, Grouping.INDIAN.code());
        assertEquals(NumFormat.STYLE_ALL, NumFormat.INVARIANT.styles());
        assertTrue(NumFormat.INVARIANT.toString().endsWith("grouping=WESTERN]"));
    }

    private static NumFormat copy(NumFormat f) {
        return new NumFormat(
                f.decimalSeparator(),
                f.groupSeparator(),
                f.styles(),
                f.currencySymbol(),
                f.currencyDecimalSeparator(),
                f.currencyGroupSeparator(),
                f.minusSign(),
                f.plusSign(),
                f.percentSymbol(),
                f.digitZero());
    }

    /**
     * The one deviation from the record copy rule, pinned: a copy through the canonical constructor
     * from {@code styles()} is exact for a Western format and takes the default policy for any other.
     */
    @Test
    void aCanonicalCopyIsExactButForANonDefaultPolicy() {
        NumFormat western = NumFormat.INVARIANT.withSigns("\u2212", "").withCurrencySymbol("$");
        assertEquals(western, copy(western));
        assertEquals(western.hashCode(), copy(western).hashCode());
        for (Grouping grouping : List.of(Grouping.LEGACY, Grouping.INDIAN)) {
            NumFormat original = western.withGrouping(grouping);
            NumFormat copy = copy(original);
            assertEquals(Grouping.WESTERN, copy.grouping(), "a " + grouping + " format copies as Western");
            assertEquals(western, copy);
            // Every other component survives, and the policy is restored the documented way.
            assertEquals(original, copy.withGrouping(original.grouping()));
        }
        // A bridged format can be legacy (Toki Pona's groups of two), so it copies as Western too.
        NumFormat bridged = NumFormat.from(new Patterned("#,#0.###"), new Patterned("\u00a4#,#0.00"));
        assertEquals(Grouping.LEGACY, bridged.grouping());
        assertEquals(Grouping.WESTERN, copy(bridged).grouping());
        assertEquals(bridged, copy(bridged).withGrouping(bridged.grouping()));
    }

    @Test
    void theStoredPolicyAcceptsOnlyWhatAFormatOfTheClassCouldHold() {
        // Bits 16 and 17 of the canonical constructor's styles: 0 Western, 1 legacy, 2 Indian.
        int all = NumFormat.STYLE_ALL;
        assertEquals(Grouping.WESTERN, new NumFormat(".", ",", all, "", "", "", "", "", "", 0).grouping());
        assertEquals(Grouping.LEGACY, new NumFormat(".", ",", all | 1 << 16, "", "", "", "", "", "", 0).grouping());
        assertEquals(Grouping.INDIAN, new NumFormat(".", ",", all | 2 << 16, "", "", "", "", "", "", 0).grouping());
        // 3 names no policy; any other bit that is no style is refused, as it always was.
        IllegalArgumentException three = assertThrows(
                IllegalArgumentException.class,
                () -> new NumFormat(".", ",", all | 3 << 16, "", "", "", "", "", "", 0));
        assertTrue(three.getMessage().contains("names no grouping policy"), three.getMessage());
        assertThrows(
                IllegalArgumentException.class,
                () -> new NumFormat(".", ",", all | 1 << 18, "", "", "", "", "", "", 0));
        assertThrows(
                IllegalArgumentException.class, () -> new NumFormat(".", ",", all | 1 << 8, "", "", "", "", "", "", 0));
        // A format's own word is 1.0's for the default and never leaks into styles().
        assertEquals(all, NumFormat.INVARIANT.withGrouping(Grouping.INDIAN).styles());
    }

    @Test
    void everyComponentHasAWitherIncludingTheSeparators() {
        NumFormat base = NumFormat.INVARIANT.withGrouping(Grouping.LEGACY).withCurrencySymbol("$");
        NumFormat swapped = base.withSeparators(",", ".");
        assertEquals(",", swapped.decimalSeparator());
        assertEquals(".", swapped.groupSeparator());
        assertEquals(Grouping.LEGACY, swapped.grouping());
        assertEquals("$", swapped.currencySymbol());
        assertEquals(base, swapped.withSeparators(".", ","));
        assertEquals(new Success<>(1234.5), Cast.f64("1.234,5", swapped));
        assertThrows(IllegalArgumentException.class, () -> base.withSeparators(",", ","));
        assertThrows(IllegalArgumentException.class, () -> base.withSeparators("", ","));
        assertThrows(NullPointerException.class, () -> base.withSeparators(null, ","));
    }

    @Test
    void theStrictRulesAtTheDoors() {
        NumFormat western = NumFormat.INVARIANT;
        NumFormat indian = western.withGrouping(Grouping.INDIAN);
        NumFormat legacy = western.withGrouping(Grouping.LEGACY);
        // Western: three digits, counted leftward from the decimal separator; ungrouped is valid.
        assertEquals(new Success<>(1234567L), Cast.i64("1,234,567", western));
        assertEquals(new Success<>(1234567L), Cast.i64("1234567", western));
        assertEquals(new Fault<Long>(CastFailure.MALFORMED, 2, 3), Cast.i64("12,34", western));
        // Once a numeral uses a group separator every group must conform.
        assertEquals(new Fault<Long>(CastFailure.MALFORMED, 0, 4), Cast.i64("1234,567", western));
        assertEquals(new Success<>(1234567.5), Cast.f64("1,234,567.5", western));
        // No grouping after the decimal separator or in an exponent.
        assertEquals(CastFailure.MALFORMED, ((Fault<Double>) Cast.f64("1.234,5", western)).reason());
        assertEquals(
                FaultDetail.GROUP_IN_FRACTION,
                ((DetailedFault<Double>) DetailedCast.f64("1.5e1,000", western)).detail());
        // Indian: three beside the decimal separator, two to its left, the leftmost one or two.
        assertEquals(new Success<>(1234567L), Cast.i64("12,34,567", indian));
        assertEquals(new Success<>(1234L), Cast.i64("1,234", indian));
        assertEquals(CastFailure.MALFORMED, ((Fault<Long>) Cast.i64("123,456", indian)).reason());
        assertEquals(new Success<>(12345678.9), Cast.f64("1,23,45,678.9", indian));
        // Legacy: a separator anywhere between two digits.
        assertEquals(new Success<>(1234L), Cast.i64("12,34", legacy));
        assertEquals(new Success<>(1234567L), Cast.i64("1234,567", legacy));
        // The decimal door and a currency amount, with the currency symbol's own separators.
        assertEquals(
                new Success<>(new java.math.BigDecimal("1234567.5")),
                Cast.decimal("$1,234,567.50", western.withCurrencySymbol("$")));
        assertEquals(
                CastFailure.MALFORMED,
                ((Fault<java.math.BigDecimal>) Cast.decimal("$12,34", western.withCurrencySymbol("$"))).reason());
    }

    @Test
    void nativeDigitsAndSpaceLikeSeparatorsAreCountedAsDigits() {
        NumFormat arabic = new NumFormat(".", "\u202f", NumFormat.STYLE_ALL).withDigitZero(0x0660);
        // U+202F between native digits: one thousand two hundred thirty-four.
        assertEquals(new Success<>(1234), Cast.i32("\u0661\u202f\u0662\u0663\u0664", arabic));
        // Two digits and then three-digit group size wrong: counted in digits, not bytes.
        DetailedFault<Integer> fault =
                (DetailedFault<Integer>) DetailedCast.i32("\u0661\u0662\u202f\u0663\u0664", arabic);
        assertEquals(FaultDetail.GROUP_SIZE, fault.detail());
        assertEquals(2, fault.offset());
        assertEquals(3, fault.length());
        // The interchangeable space-like separators stand for each other under the strict rules.
        NumFormat space = new NumFormat(",", " ", NumFormat.STYLE_ALL);
        assertEquals(new Success<>(1234567), Cast.i32("1\u00a0234\u202f567", space));
    }

    // --- the bridge reads the platform's patterns, never the locale's name ---

    @Test
    void aPatternHasTheShapeItGroupsIn() {
        assertEquals(NumFormat.Shape.INDIAN, NumFormat.shapeOf("#,##,##0.###"));
        assertEquals(NumFormat.Shape.INDIAN, NumFormat.shapeOf("\u00a4#,##,##0.00"));
        assertEquals(NumFormat.Shape.INDIAN, NumFormat.shapeOf("\u00a4 #,##,##0.00;(\u00a4 #,##,##0.00)"));
        assertEquals(NumFormat.Shape.INDIAN, NumFormat.shapeOf("#,##,##,##0"));
        assertEquals(NumFormat.Shape.INDIAN, NumFormat.shapeOf("'#'#,##,##0.00"));
        // A single separator has no secondary size of its own: it repeats the primary.
        assertEquals(NumFormat.Shape.WESTERN, NumFormat.shapeOf("#,##0.###"));
        assertEquals(NumFormat.Shape.WESTERN, NumFormat.shapeOf("#,##0.00 \u00a4"));
        assertEquals(NumFormat.Shape.WESTERN, NumFormat.shapeOf("#,###"));
        assertEquals(NumFormat.Shape.WESTERN, NumFormat.shapeOf("#,##,###,##0"));
        // Groups of two (Toki Pona), and any other size or secondary, are another shape.
        assertEquals(NumFormat.Shape.OTHER, NumFormat.shapeOf("#,#0.###"));
        assertEquals(NumFormat.Shape.OTHER, NumFormat.shapeOf("\u00a4#,#0.00"));
        assertEquals(NumFormat.Shape.OTHER, NumFormat.shapeOf("#,##"));
        assertEquals(NumFormat.Shape.OTHER, NumFormat.shapeOf("#,####"));
        assertEquals(NumFormat.Shape.OTHER, NumFormat.shapeOf("#,##,##"));
        assertEquals(NumFormat.Shape.OTHER, NumFormat.shapeOf("#,####,##0"));
        // No separator is no grouping, and a size of 0 (en-US-POSIX) writes none.
        assertEquals(NumFormat.Shape.NONE, NumFormat.shapeOf("0.###"));
        assertEquals(NumFormat.Shape.NONE, NumFormat.shapeOf("#0.######"));
        assertEquals(NumFormat.Shape.NONE, NumFormat.shapeOf("#0.00"));
        assertEquals(NumFormat.Shape.NONE, NumFormat.shapeOf(""));
        // A quoted comma is text, not a separator; the negative subpattern is not read.
        assertEquals(NumFormat.Shape.NONE, NumFormat.shapeOf("'#,##,##0'#0"));
        assertEquals(NumFormat.Shape.WESTERN, NumFormat.shapeOf("#,##0.00;#,##,##0.00"));
    }

    /** A DecimalFormat whose pattern is exactly {@code pattern}, as a platform with a richer pattern reports it. */
    private static final class Patterned extends DecimalFormat {
        private final String pattern;

        Patterned(String pattern) {
            super("#,##0.###", DecimalFormatSymbols.getInstance(Locale.US));
            this.pattern = pattern;
        }

        @Override
        public String toPattern() {
            return pattern;
        }
    }

    private static NumFormat bridge(String number, String currency) {
        return NumFormat.from(new Patterned(number), new Patterned(currency));
    }

    @Test
    void theGroupingPatternsThatGroupDecideTheOnePolicyAndTheOneFlag() {
        int all = NumFormat.STYLE_ALL;
        // Both Western: strict Western. Both Indian: strict Indian.
        assertEquals(Grouping.WESTERN, bridge("#,##0.###", "\u00a4#,##0.00").grouping());
        assertEquals(
                Grouping.INDIAN, bridge("#,##,##0.###", "\u00a4#,##,##0.00").grouping());
        // Western and Indian disagree: legacy, which reads both.
        assertEquals(Grouping.LEGACY, bridge("#,##,##0.###", "\u00a4#,##0.00").grouping());
        assertEquals(Grouping.LEGACY, bridge("#,##0.###", "\u00a4#,##,##0.00").grouping());
        // Any pattern of another shape: legacy, and either order.
        assertEquals(Grouping.LEGACY, bridge("#,#0.###", "\u00a4#,#0.00").grouping());
        assertEquals(Grouping.LEGACY, bridge("#,##0.###", "\u00a4#,#0.00").grouping());
        assertEquals(Grouping.LEGACY, bridge("#,#0.###", "\u00a4#,##0.00").grouping());
        // Only one groups: its shape decides, and the flag stays for it (ungrouped values stay valid).
        NumFormat indianAndNone = bridge("#,##,##0.###", "\u00a40.00");
        assertEquals(Grouping.INDIAN, indianAndNone.grouping());
        assertEquals(all, indianAndNone.styles());
        NumFormat noneAndWestern = bridge("0.###", "\u00a4#,##0.00");
        assertEquals(Grouping.WESTERN, noneAndWestern.grouping());
        assertEquals(all, noneAndWestern.styles());
        assertEquals(Grouping.LEGACY, bridge("#,#0.###", "0.00").grouping());
        assertEquals(all, bridge("#,#0.###", "0.00").styles());
        // Neither groups: GROUPING is cleared, and a separator is then refused.
        NumFormat none = bridge("#0.######", "\u00a4#0.00");
        assertEquals(all & ~NumFormat.STYLE_GROUPING, none.styles());
        assertEquals(new Success<>(1234.5), Cast.f64("1234.5", none));
        assertEquals(CastFailure.MALFORMED, ((Fault<Double>) Cast.f64("1,234.5", none)).reason());
        // A format of one pattern reads that pattern alone.
        assertEquals(
                Grouping.INDIAN, NumFormat.from(new Patterned("#,##,##0.###")).grouping());
        assertEquals(Grouping.LEGACY, NumFormat.from(new Patterned("#,#0.###")).grouping());
        assertEquals(
                all & ~NumFormat.STYLE_GROUPING,
                NumFormat.from(new Patterned("0.###")).styles());
        assertEquals(Grouping.WESTERN, NumFormat.from(new Patterned("0.###")).grouping());
        // The rule never looks at input: the same patterns give the same, equal format.
        assertEquals(bridge("#,#0.###", "\u00a4#,#0.00"), bridge("#,#0.###", "\u00a4#,#0.00"));
    }

    /** A platform whose DecimalFormat reports the Indian grouping the JDK's own cannot keep. */
    private static final class IndianPattern extends DecimalFormat {
        IndianPattern() {
            super("#,##0.###", DecimalFormatSymbols.getInstance(Locale.US));
        }

        @Override
        public String toPattern() {
            return "#,##,##0.###";
        }
    }

    @Test
    void theBridgeDeclaresWhatTheFormatsPatternSays() {
        NumFormat derived = NumFormat.from(new IndianPattern());
        assertEquals(Grouping.INDIAN, derived.grouping());
        assertEquals(new Success<>(12345678L), Cast.i64("1,23,45,678", derived));
        assertEquals(CastFailure.MALFORMED, ((Fault<Long>) Cast.i64("12,345,678", derived)).reason());
        NumFormat western = NumFormat.from(new DecimalFormat("#,##0.###", DecimalFormatSymbols.getInstance(Locale.US)));
        assertEquals(Grouping.WESTERN, western.grouping());
        assertEquals(new Success<>(12345678L), Cast.i64("12,345,678", western));
        // Toki Pona groups by two with a no-break space; legacy reads that, and strict Western cannot.
        NumFormat twos = NumFormat.from(new Patterned("#,#0.###"));
        assertEquals(new Success<>(12345678L), Cast.i64("12,34,56,78", twos));
    }

    @Test
    void theJdksOwnPatternForAnIndianLocaleIsWesternSoThatIsWhatTheBridgeDeclares() {
        // DecimalFormat keeps one grouping size, the last interval: the JDK formats en-IN as
        // 12,345,678 and its pattern says #,##0.###, so strict Western is right for what the
        // platform writes. CLDR-style Indian output is read by declaring it.
        Locale india = Locale.forLanguageTag("en-IN");
        DecimalFormat pattern = (DecimalFormat) NumberFormat.getInstance(india);
        assertEquals(NumFormat.Shape.WESTERN, NumFormat.shapeOf(pattern.toPattern()));
        assertEquals("12,345,678", pattern.format(12345678L));
        assertEquals(Grouping.WESTERN, NumFormat.from(india).grouping());
        assertEquals(new Success<>(12345678L), Cast.i64("12,345,678", india));
        assertEquals(
                new Success<>(12345678L),
                Cast.i64("1,23,45,678", NumFormat.from(india).withGrouping(Grouping.INDIAN)));
    }

    /** What the rule says about a locale's own two patterns, computed independently of {@code from}. */
    private static void assertTheRule(Locale locale) {
        DecimalFormat number = (DecimalFormat) NumberFormat.getInstance(locale);
        DecimalFormat currency = (DecimalFormat) NumberFormat.getCurrencyInstance(locale);
        java.util.Set<NumFormat.Shape> grouping = new java.util.TreeSet<>();
        for (DecimalFormat format : List.of(number, currency)) {
            NumFormat.Shape shape = NumFormat.shapeOf(format.toPattern());
            if (shape != NumFormat.Shape.NONE) {
                grouping.add(shape);
            }
        }
        NumFormat format = NumFormat.from(locale);
        String why = locale.toLanguageTag() + ": " + number.toPattern() + " / " + currency.toPattern();
        Grouping expected = grouping.isEmpty()
                ? Grouping.WESTERN
                : grouping.equals(java.util.Set.of(NumFormat.Shape.WESTERN))
                        ? Grouping.WESTERN
                        : grouping.equals(java.util.Set.of(NumFormat.Shape.INDIAN)) ? Grouping.INDIAN : Grouping.LEGACY;
        assertEquals(expected, format.grouping(), why);
        assertEquals(grouping.isEmpty(), (format.styles() & NumFormat.STYLE_GROUPING) == 0, why);
    }

    /**
     * The bridge's format reads what the platform wrote: the locale's own number and currency
     * output, at magnitudes from zero to a billion, negative included.
     */
    private static void assertReadsItsOwnOutput(Locale locale) {
        NumFormat format = NumFormat.from(locale);
        NumberFormat number = NumberFormat.getInstance(locale);
        number.setMaximumFractionDigits(3);
        NumberFormat currency = NumberFormat.getCurrencyInstance(locale);
        for (double value : new double[] {0, 1234.5, -1234.5, 1234567.891, 12345678.9, 1e9}) {
            String shown = number.format(value);
            assertEquals(
                    new Success<>(new java.math.BigDecimal(Double.toString(value))
                            .setScale(3, java.math.RoundingMode.HALF_EVEN)
                            .doubleValue()),
                    Cast.f64(shown, format),
                    locale.toLanguageTag() + " number '" + shown + "'");
            String money = currency.format(value);
            java.math.BigDecimal want = java.math.BigDecimal.valueOf(value)
                    .setScale(currency.getMaximumFractionDigits(), java.math.RoundingMode.HALF_EVEN)
                    .stripTrailingZeros();
            Verdict<java.math.BigDecimal> read = Cast.decimal(money, format);
            assertTrue(
                    read instanceof Success<java.math.BigDecimal> ok
                            && ok.value().compareTo(want) == 0,
                    locale.toLanguageTag() + " currency '" + money + "' read " + read);
        }
    }

    @Test
    void theRuleHoldsForTheActualPatternsOfTheLocalesTheTaskNames() {
        // tok (Toki Pona, groups of two, on JDK 25), en-US-POSIX (grouping size 0), en-BG, en-IN
        // and the ICU ml/ta/mr/or where this JDK's data differs, whichever of them this JDK has:
        // the rule is checked against each one's actual patterns, whatever they are here.
        for (Locale locale : List.of(
                Locale.forLanguageTag("tok"),
                Locale.forLanguageTag("tok-001"),
                Locale.forLanguageTag("tok-Latn-001"),
                Locale.of("en", "US", "POSIX"),
                Locale.forLanguageTag("en-BG"),
                Locale.forLanguageTag("en-IN"),
                Locale.forLanguageTag("hi-IN"),
                Locale.forLanguageTag("ml-IN"),
                Locale.forLanguageTag("ta-IN"),
                Locale.forLanguageTag("mr-IN"),
                Locale.forLanguageTag("or-IN"))) {
            assertTheRule(locale);
        }
        // Where the platform really writes groups of two, the bridge is legacy and reads them.
        DecimalFormat tok = (DecimalFormat) NumberFormat.getInstance(Locale.forLanguageTag("tok"));
        if (NumFormat.shapeOf(tok.toPattern()) == NumFormat.Shape.OTHER) {
            assertEquals(
                    Grouping.LEGACY,
                    NumFormat.from(Locale.forLanguageTag("tok")).grouping());
        }
        // Where it writes no grouping at all, GROUPING is cleared.
        DecimalFormat posix = (DecimalFormat) NumberFormat.getInstance(Locale.of("en", "US", "POSIX"));
        if (NumFormat.shapeOf(posix.toPattern()) == NumFormat.Shape.NONE) {
            assertEquals(0, NumFormat.from(Locale.of("en", "US", "POSIX")).styles() & NumFormat.STYLE_GROUPING);
        }
    }

    @Test
    void representativeLocalesReadTheirOwnNumberAndCurrencyOutput() {
        for (String tag : List.of(
                "en-US",
                "en-GB",
                "de-DE",
                "de-CH",
                "fr-FR",
                "fr-CH",
                "es-ES",
                "pt-BR",
                "ja-JP",
                "ko-KR",
                "hi-IN",
                "en-IN",
                "mr-IN",
                "ml-IN",
                "ta-IN",
                "en-BG",
                "tok",
                "tok-001",
                "tok-Latn-001")) {
            assertReadsItsOwnOutput(Locale.forLanguageTag(tag));
        }
        assertReadsItsOwnOutput(Locale.of("en", "US", "POSIX"));
    }

    @Test
    void everyLocalesBridgeFollowsTheRuleAndNeverRejectsWhatLegacyGroupingWouldRead() {
        int seen = 0;
        double[] values = {0, 1234.5, -1234.5, 1234567.891, 12345678.9, 1e9};
        for (Locale locale : Locale.getAvailableLocales()) {
            assertTheRule(locale);
            // The invariant: whatever the platform wrote that legacy grouping (the 1.0 rule, with
            // GROUPING on) reads, the bridge's own format reads too.
            NumFormat bridge = NumFormat.from(locale);
            NumFormat legacy =
                    bridge.withGrouping(Grouping.LEGACY).withStyles(bridge.styles() | NumFormat.STYLE_GROUPING);
            NumberFormat number = NumberFormat.getInstance(locale);
            number.setMaximumFractionDigits(3);
            NumberFormat currency = NumberFormat.getCurrencyInstance(locale);
            for (double value : values) {
                for (String shown : List.of(number.format(value), currency.format(value))) {
                    if (Cast.decimal(shown, legacy) instanceof Success<java.math.BigDecimal> ok) {
                        assertEquals(
                                ok,
                                Cast.decimal(shown, bridge),
                                locale.toLanguageTag() + " '" + shown + "' under " + bridge.grouping());
                    }
                }
            }
            seen++;
        }
        assertTrue(seen > 100, "the JDK knows a locale sweep's worth of locales");
    }
}
