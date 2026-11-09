package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.text.DecimalFormat;
import java.text.DecimalFormatSymbols;
import java.text.NumberFormat;
import java.util.List;
import java.util.Locale;
import org.junit.jupiter.api.Test;

/**
 * Grouping: the policy a {@link NumFormat} declares apart from its styles, what the
 * bridge reads from the platform's own pattern, and the core's rules at the doors.
 */
final class GroupingTest {
    @Test
    void theDefaultIsWesternAndIndianIsExplicit() {
        assertEquals(Grouping.WESTERN, NumFormat.INVARIANT.grouping());
        assertEquals(Grouping.WESTERN, NumFormat.DETECT.grouping());
        assertEquals(Grouping.WESTERN, new NumFormat(".", ",", NumFormat.STYLE_ALL, Grouping.WESTERN).grouping());
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
                Grouping.WESTERN,
                format.withGrouping(Grouping.INDIAN)
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
        // Bit 8 is retired, a stale word holding it is refused wherever it is declared, and every
        // defined policy round-trips through its ABI bits.
        assertThrows(IllegalArgumentException.class, () -> Grouping.of(1 << 8));
        assertThrows(IllegalArgumentException.class, () -> Grouping.of(3 << 8));
        for (int stale : new int[] {351, 863, 1 << 8, 3 << 8}) {
            assertThrows(IllegalArgumentException.class, () -> new NumFormat(".", ",", stale));
            assertThrows(IllegalArgumentException.class, () -> NumFormat.INVARIANT.withStyles(stale));
            assertThrows(
                    IllegalArgumentException.class, () -> new NumFormat(".", ",", stale, "", "", "", "", "", "", 0));
        }
        assertThrows(IllegalArgumentException.class, () -> Grouping.of(1));
        for (Grouping grouping : Grouping.values()) {
            assertEquals(grouping, Grouping.of(grouping.code()));
        }
        assertEquals(0, Grouping.WESTERN.code());
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
        NumFormat original = western.withGrouping(Grouping.INDIAN);
        NumFormat copied = copy(original);
        assertEquals(Grouping.WESTERN, copied.grouping(), "an Indian format copies as Western");
        assertEquals(western, copied);
        // Every other component survives, and the policy is restored the documented way.
        assertEquals(original, copied.withGrouping(original.grouping()));
        // A bridged format can be Indian, so it copies as Western too.
        NumFormat bridged = NumFormat.from(new Patterned("#,##,##0.###"), new Patterned("\u00a4#,##,##0.00"));
        assertEquals(Grouping.INDIAN, bridged.grouping());
        assertEquals(Grouping.WESTERN, copy(bridged).grouping());
        assertEquals(bridged, copy(bridged).withGrouping(bridged.grouping()));
    }

    @Test
    void theStoredPolicyAcceptsOnlyWhatAFormatOfTheClassCouldHold() {
        // Bits 16 and 17 of the canonical constructor's styles: 0 Western, 2 Indian.
        int all = NumFormat.STYLE_ALL;
        assertEquals(Grouping.WESTERN, new NumFormat(".", ",", all, "", "", "", "", "", "", 0).grouping());
        assertEquals(Grouping.INDIAN, new NumFormat(".", ",", all | 2 << 16, "", "", "", "", "", "", 0).grouping());
        // 1 and 3 name no policy; any other bit that is no style is refused, as it always was.
        for (int field : new int[] {1, 3}) {
            IllegalArgumentException refused = assertThrows(
                    IllegalArgumentException.class,
                    () -> new NumFormat(".", ",", all | field << 16, "", "", "", "", "", "", 0));
            assertTrue(refused.getMessage().contains("names no grouping policy"), refused.getMessage());
        }
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
        NumFormat base = NumFormat.INVARIANT.withGrouping(Grouping.INDIAN).withCurrencySymbol("$");
        NumFormat swapped = base.withSeparators(",", ".");
        assertEquals(",", swapped.decimalSeparator());
        assertEquals(".", swapped.groupSeparator());
        assertEquals(Grouping.INDIAN, swapped.grouping());
        assertEquals("$", swapped.currencySymbol());
        assertEquals(base, swapped.withSeparators(".", ","));
        assertEquals(new Success<>(1234.5), Cast.f64("1.234,5", swapped));
        assertThrows(IllegalArgumentException.class, () -> base.withSeparators(",", ","));
        assertThrows(IllegalArgumentException.class, () -> base.withSeparators("", ","));
        assertThrows(NullPointerException.class, () -> base.withSeparators(null, ","));
    }

    @Test
    void theRulesAtTheDoors() {
        NumFormat western = NumFormat.INVARIANT;
        NumFormat indian = western.withGrouping(Grouping.INDIAN);
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
        // Each policy refuses the other's shapes, and no shape is open.
        assertEquals(new Fault<Long>(CastFailure.MALFORMED, 1, 3), Cast.i64("1,23,456", western));
        assertEquals(new Fault<Long>(CastFailure.MALFORMED, 0, 4), Cast.i64("1234,567,890", western));
        assertEquals(new Fault<Long>(CastFailure.MALFORMED, 0, 1), Cast.i64("0,125", western));
        assertEquals(new Fault<Long>(CastFailure.MALFORMED, 1, 4), Cast.i64("1,234,567", indian));
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
        // The interchangeable space-like separators stand for each other under both policies.
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
        // The number pattern decides: Indian is Indian, anything else Western.
        assertEquals(Grouping.WESTERN, bridge("#,##0.###", "\u00a4#,##0.00").grouping());
        assertEquals(
                Grouping.INDIAN, bridge("#,##,##0.###", "\u00a4#,##,##0.00").grouping());
        assertEquals(Grouping.INDIAN, bridge("#,##,##0.###", "\u00a4#,##0.00").grouping());
        assertEquals(Grouping.WESTERN, bridge("#,##0.###", "\u00a4#,##,##0.00").grouping());
        // Another shape is Western, in either order.
        assertEquals(Grouping.WESTERN, bridge("#,#0.###", "\u00a4#,#0.00").grouping());
        assertEquals(Grouping.WESTERN, bridge("#,##0.###", "\u00a4#,#0.00").grouping());
        assertEquals(Grouping.WESTERN, bridge("#,#0.###", "\u00a4#,##0.00").grouping());
        assertEquals(Grouping.INDIAN, bridge("#,##,##0.###", "\u00a4#,#0.00").grouping());
        // Only one groups: the number pattern still decides, and the flag stays (ungrouped values stay valid).
        NumFormat indianAndNone = bridge("#,##,##0.###", "\u00a40.00");
        assertEquals(Grouping.INDIAN, indianAndNone.grouping());
        assertEquals(all, indianAndNone.styles());
        NumFormat noneAndIndian = bridge("0.###", "\u00a4#,##,##0.00");
        assertEquals(Grouping.WESTERN, noneAndIndian.grouping());
        assertEquals(all, noneAndIndian.styles());
        assertEquals(Grouping.WESTERN, bridge("#,#0.###", "0.00").grouping());
        assertEquals(all, bridge("#,#0.###", "0.00").styles());
        // Neither groups: GROUPING is cleared, and a separator is then refused.
        NumFormat none = bridge("#0.######", "\u00a4#0.00");
        assertEquals(all & ~NumFormat.STYLE_GROUPING, none.styles());
        assertEquals(new Success<>(1234.5), Cast.f64("1234.5", none));
        assertEquals(CastFailure.MALFORMED, ((Fault<Double>) Cast.f64("1,234.5", none)).reason());
        // A format of one pattern reads that pattern alone.
        assertEquals(
                Grouping.INDIAN, NumFormat.from(new Patterned("#,##,##0.###")).grouping());
        assertEquals(Grouping.WESTERN, NumFormat.from(new Patterned("#,#0.###")).grouping());
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
        // Toki Pona groups by two; the bridge declares Western, and that shape is refused, never misread.
        NumFormat twos = NumFormat.from(new Patterned("#,#0.###"));
        assertEquals(new Fault<Long>(CastFailure.MALFORMED, 2, 3), Cast.i64("12,34,56,78", twos));
    }

    @Test
    void theJdksOwnPatternForAnIndianLocaleIsWesternSoThatIsWhatTheBridgeDeclares() {
        // DecimalFormat keeps one grouping size, the last interval: the JDK formats en-IN as
        // 12,345,678 and its pattern says #,##0.###, so Western is right for what the
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
        NumFormat.Shape numberShape = NumFormat.shapeOf(number.toPattern());
        boolean groups =
                numberShape != NumFormat.Shape.NONE || NumFormat.shapeOf(currency.toPattern()) != NumFormat.Shape.NONE;
        NumFormat format = NumFormat.from(locale);
        String why = locale.toLanguageTag() + ": " + number.toPattern() + " / " + currency.toPattern();
        assertEquals(
                numberShape == NumFormat.Shape.INDIAN ? Grouping.INDIAN : Grouping.WESTERN, format.grouping(), why);
        assertEquals(!groups, (format.styles() & NumFormat.STYLE_GROUPING) == 0, why);
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
        // Where the platform really writes groups of two, the bridge is Western.
        DecimalFormat tok = (DecimalFormat) NumberFormat.getInstance(Locale.forLanguageTag("tok"));
        if (NumFormat.shapeOf(tok.toPattern()) == NumFormat.Shape.OTHER) {
            assertEquals(
                    Grouping.WESTERN,
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
                "en-US", "en-GB", "de-DE", "de-CH", "fr-FR", "fr-CH", "es-ES", "pt-BR", "ja-JP", "ko-KR", "hi-IN",
                "en-IN", "mr-IN", "ml-IN", "ta-IN", "en-BG")) {
            assertReadsItsOwnOutput(Locale.forLanguageTag(tag));
        }
        assertReadsItsOwnOutput(Locale.of("en", "US", "POSIX"));
    }

    @Test
    void everyLocalesBridgeFollowsTheRuleAndReadsThePatternsOfItsPolicy() {
        int seen = 0;
        double[] values = {0, 1234.5, -1234.5, 1234567.891, 12345678.9, 1e9};
        for (Locale locale : Locale.getAvailableLocales()) {
            assertTheRule(locale);
            // Whatever the platform wrote in a pattern of the bridge's own policy is never refused
            // for its groups; a pattern of another shape is refused, never misread.
            NumFormat bridge = NumFormat.from(locale);
            NumFormat.Shape policy =
                    bridge.grouping() == Grouping.INDIAN ? NumFormat.Shape.INDIAN : NumFormat.Shape.WESTERN;
            DecimalFormat number = (DecimalFormat) NumberFormat.getInstance(locale);
            number.setMaximumFractionDigits(3);
            DecimalFormat currency = (DecimalFormat) NumberFormat.getCurrencyInstance(locale);
            for (double value : values) {
                for (DecimalFormat writer : List.of(number, currency)) {
                    NumFormat.Shape shape = NumFormat.shapeOf(writer.toPattern());
                    if (shape != policy && shape != NumFormat.Shape.NONE) {
                        continue;
                    }
                    String shown = writer.format(value);
                    if (DetailedCast.decimal(shown, bridge) instanceof DetailedFault<?> fault) {
                        assertNotEquals(
                                FaultDetail.GROUP_SIZE,
                                fault.detail(),
                                locale.toLanguageTag() + " '" + shown + "' under " + bridge.grouping());
                    }
                }
            }
            seen++;
        }
        assertTrue(seen > 100, "the JDK knows a locale sweep's worth of locales");
    }
}
