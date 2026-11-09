package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.List;
import java.util.Locale;
import java.util.OptionalInt;
import org.junit.jupiter.api.Test;

/**
 * 1.0's surface, written exactly as 1.0 code writes it: constructors, record patterns and
 * deconstruction, equality and hash, and exhaustive switches with no {@code default}. Nothing
 * here may need an edit for the detailed path, grouping policies or the date options: this file
 * compiling is half the proof, and the assertions are the other half.
 */
final class CompatibilityTest {
    @Test
    void everyNumFormatConstructorOf10StillCompilesAndDeclaresWhatItDid() {
        NumFormat three = new NumFormat(".", ",", NumFormat.STYLE_ALL);
        NumFormat four = new NumFormat(".", ",", NumFormat.STYLE_ALL, "$");
        NumFormat chars = new NumFormat('.', ',', NumFormat.STYLE_ALL);
        NumFormat charsAndCurrency = new NumFormat('.', ',', NumFormat.STYLE_ALL, "$");
        NumFormat all = new NumFormat(".", ",", NumFormat.STYLE_ALL, "$", "", "", "\u2212", "+", "\u066a", 0x0660);
        assertEquals(three, chars);
        assertEquals(four, charsAndCurrency);
        assertEquals(three, NumFormat.INVARIANT);
        assertEquals(three.hashCode(), NumFormat.INVARIANT.hashCode());
        assertEquals("$", all.currencySymbol());
        assertEquals("\u2212", all.minusSign());
        assertEquals(0x0660, all.digitZero());
        // Every one of them declares Western grouping now, not 1.0's rule.
        for (NumFormat format :
                List.of(three, four, chars, charsAndCurrency, all, NumFormat.INVARIANT, NumFormat.DETECT)) {
            assertEquals(Grouping.WESTERN, format.grouping(), format.toString());
        }
    }

    @Test
    void theTenComponentRecordPatternStillDeconstructs() {
        NumFormat format = new NumFormat(",", ".", NumFormat.STYLE_ALL | NumFormat.STYLE_TRAILING_SIGN, "\u20ac")
                .withSigns("\u2212", "")
                .withDigitZero(0x0660)
                .withGrouping(Grouping.INDIAN);
        // Ten components, as 1.0 declared them; the mask is the styles alone.
        if (!(format
                instanceof
                NumFormat(
                        String decimal,
                        String group,
                        int styles,
                        String currency,
                        String currencyDecimal,
                        String currencyGroup,
                        String minus,
                        String plus,
                        String percent,
                        int zero))) {
            throw new AssertionError("NumFormat no longer deconstructs into its ten components");
        }
        assertEquals(",", decimal);
        assertEquals(".", group);
        assertEquals(NumFormat.STYLE_ALL | NumFormat.STYLE_TRAILING_SIGN, styles);
        assertEquals("\u20ac", currency);
        assertEquals("", currencyDecimal);
        assertEquals("", currencyGroup);
        assertEquals("\u2212", minus);
        assertEquals("", plus);
        assertEquals("", percent);
        assertEquals(0x0660, zero);
        // 1.0's accessors answer what they did, and rebuilding from them keeps every style.
        assertEquals(styles, format.styles());
        NumFormat rebuilt = new NumFormat(
                decimal, group, styles, currency, currencyDecimal, currencyGroup, minus, plus, percent, zero);
        assertEquals(format.withGrouping(Grouping.WESTERN), rebuilt);
    }

    @Test
    void equalityAndHashAreOverTheComponentsAndTheNewPolicy() {
        NumFormat western = new NumFormat(".", ",", NumFormat.STYLE_ALL);
        assertEquals(western, new NumFormat('.', ',', NumFormat.STYLE_ALL));
        assertEquals(western.hashCode(), new NumFormat('.', ',', NumFormat.STYLE_ALL).hashCode());
        // The policy is part of what a format is: Indian is another format.
        assertNotEquals(western, western.withGrouping(Grouping.INDIAN));
        assertEquals(western, western.withGrouping(Grouping.INDIAN).withGrouping(Grouping.WESTERN));
        assertNotEquals(western, new NumFormat(".", ",", NumFormat.STYLE_ALL & ~NumFormat.STYLE_PERCENT));
        assertEquals(
                western,
                NumFormat.from(new java.text.DecimalFormat(
                                "#,##0.###", new java.text.DecimalFormatSymbols(Locale.ROOT)))
                        .withStyles(NumFormat.STYLE_ALL)
                        .withCurrencySymbol("")
                        .withCurrencySeparators("", ""));
    }

    @Test
    void withStylesAndTheOtherWithersKeepThePolicy() {
        for (Grouping grouping : Grouping.values()) {
            NumFormat format = NumFormat.INVARIANT.withGrouping(grouping);
            assertEquals(
                    grouping,
                    format.withStyles(NumFormat.STYLE_ALL & ~NumFormat.STYLE_RADIX_PREFIXES)
                            .grouping());
            assertEquals(grouping, format.withCurrencySymbol("$").grouping());
            assertEquals(grouping, format.withCurrencySeparators(".", ",").grouping());
            assertEquals(grouping, format.withSigns("\u2212", "+").grouping());
            assertEquals(grouping, format.withPercentSymbol("\u066a").grouping());
            assertEquals(grouping, format.withDigitZero(0x0660).grouping());
        }
    }

    @Test
    void theVerdictTypesKeepTheirPatternsAndEquality() {
        // Record patterns, as 1.0 code matches them.
        Verdict<Integer> success = Cast.i32("42", NumFormat.INVARIANT);
        assertTrue(success instanceof Success<Integer>(Integer value) && value == 42);
        Verdict<Integer> fault = Cast.i32("4x", NumFormat.INVARIANT);
        assertTrue(fault instanceof Fault<Integer>(CastFailure reason, int offset, int length)
                && reason == CastFailure.MALFORMED
                && offset == 1
                && length == 1);
        // Exhaustive with two arms and no default.
        String rendered = switch (fault) {
            case Success<Integer> s -> "ok";
            case Fault<Integer> f -> "fault " + f.reason();
        };
        assertEquals("fault MALFORMED", rendered);
        // Equality and hash: a Fault is its three components, whatever the door knows beside them.
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 1, 1), fault);
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 1, 1).hashCode(), fault.hashCode());
        assertEquals(new Success<>(42), success);
        // The same input through the detailed path is equal as a plain fault, never as itself.
        DetailedFault<Integer> detailed = (DetailedFault<Integer>) DetailedCast.i32("4x", NumFormat.INVARIANT);
        assertEquals(fault, detailed.toFault());
        assertFalse(fault.equals(detailed));
    }

    @Test
    void theCheckedTypesKeepTheirPatternsAndEquality() {
        Constraints<Integer> rule = Constraints.i32().maximum("10").build();
        CheckedVerdict<Integer> ok = Cast.i32("7", NumFormat.INVARIANT, rule);
        assertTrue(ok instanceof Checked<Integer>(Integer value, boolean adjusted, OptionalInt index)
                && value == 7
                && !adjusted
                && index.isEmpty());
        CheckedVerdict<Integer> over = Cast.i32("70", NumFormat.INVARIANT, rule);
        assertTrue(over instanceof Violation<Integer>(Keyword keyword, int offset, int length)
                && keyword.equals(Keyword.MAXIMUM)
                && offset == 0
                && length == 2);
        // Three arms, no default, over both levels of the sealed hierarchy.
        String rendered = switch (Cast.i32("x", NumFormat.INVARIANT, rule)) {
            case Checked<Integer> c -> "checked";
            case Fault<Integer> f -> "fault";
            case Violation<Integer> v -> "violation";
        };
        assertEquals("fault", rendered);
        Rejection<Integer> rejection = (Rejection<Integer>) over;
        String level = switch (rejection) {
            case Fault<Integer> f -> "fault";
            case Violation<Integer> v -> "violation";
        };
        assertEquals("violation", level);
        assertEquals(new Violation<Integer>(Keyword.MAXIMUM, 0, 2), over);
        assertEquals(new Checked<>(7, false, OptionalInt.empty()), ok);
    }

    private static final int DOORS = 159;

    /**
     * NumFormat was never Serializable in 1.0 (its tag v1.0.0 declares a plain record), so no 1.0 stream
     * of one exists; it stays so, and a change that makes it Serializable must refuse a stream
     * holding a retired policy and read one without a policy as Western, and add that test.
     */
    @Test
    void numFormatIsNotSerializableAsIn10() throws Exception {
        assertFalse(java.io.Serializable.class.isAssignableFrom(NumFormat.class));
        assertThrows(
                java.io.NotSerializableException.class,
                () -> new java.io.ObjectOutputStream(java.io.OutputStream.nullOutputStream())
                        .writeObject(NumFormat.INVARIANT));
    }

    /** Every public door of Cast has a detailed twin: same name, same parameters. */
    @Test
    void everyPublicDoorHasADetailedTwinWithTheSameParameters() throws Exception {
        int doors = 0;
        for (java.lang.reflect.Method door : Cast.class.getDeclaredMethods()) {
            if (!java.lang.reflect.Modifier.isPublic(door.getModifiers())
                    || !java.lang.reflect.Modifier.isStatic(door.getModifiers())
                    || List.of("isAvailable", "nativeVersion", "optional").contains(door.getName())) {
                continue;
            }
            java.lang.reflect.Method twin = DetailedCast.class.getMethod(door.getName(), door.getParameterTypes());
            assertTrue(java.lang.reflect.Modifier.isStatic(twin.getModifiers()), twin.toGenericString());
            assertEquals(
                    List.of(DetailedVerdict.class, DetailedCheckedVerdict.class)
                            .get(door.getReturnType() == CheckedVerdict.class ? 1 : 0),
                    twin.getReturnType(),
                    twin.toGenericString());
            doors++;
        }
        // A new Cast door without a twin fails above; a removed one changes this count on purpose.
        assertEquals(DOORS, doors);
    }

    @Test
    void theSealedHierarchiesAreExactlyAsTheyWere() {
        assertEquals(List.of(Success.class, Fault.class), List.of(Verdict.class.getPermittedSubclasses()));
        assertEquals(List.of(Checked.class, Rejection.class), List.of(CheckedVerdict.class.getPermittedSubclasses()));
        assertEquals(List.of(Fault.class, Violation.class), List.of(Rejection.class.getPermittedSubclasses()));
        assertEquals(List.of(Verdict.class), List.of(Success.class.getInterfaces()));
        assertEquals(List.of(Verdict.class, Rejection.class), List.of(Fault.class.getInterfaces()));
        assertEquals(List.of(Rejection.class), List.of(Violation.class.getInterfaces()));
        assertEquals(List.of(CheckedVerdict.class), List.of(Checked.class.getInterfaces()));
        assertEquals(3, Fault.class.getRecordComponents().length);
        assertEquals(
                List.of(1, 2, 3),
                List.of(CastFailure.EMPTY.code(), CastFailure.MALFORMED.code(), CastFailure.OUT_OF_RANGE.code()));
    }

    @Test
    void anInputThatWasValidUnder10StillReadsAndTheChangedDefaultIsTheDocumentedOne() {
        // Ungrouped numerals and well-formed groups read as they did.
        assertEquals(new Success<>(1234), Cast.i32("1234", NumFormat.INVARIANT));
        assertEquals(new Success<>(1234567), Cast.i32("1,234,567", NumFormat.INVARIANT));
        assertEquals(new Success<>(1234.5), Cast.f64("1.234,5", NumFormat.from(Locale.GERMANY)));
        // The deliberate change: 12,34 was 1234 and is Malformed under every policy, and 1,23,456
        // was 123456 and is Malformed until Indian is declared.
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 2, 3), Cast.i32("12,34", NumFormat.INVARIANT));
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 1, 3), Cast.i32("1,23,456", NumFormat.INVARIANT));
        assertEquals(new Success<>(123456), Cast.i32("1,23,456", NumFormat.INVARIANT.withGrouping(Grouping.INDIAN)));
    }
}
