package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;

import java.util.HashSet;
import java.util.List;
import org.junit.jupiter.api.Test;

/**
 * A declaration equals one that declares the same thing, as the Rust core's {@code ==} holds,
 * and hashes alike, however it was built.
 */
final class EqualityTest {
    @Test
    void aFormatEqualsOneDeclaringTheSameThing() {
        NumFormat declared = new NumFormat(".", ",", NumFormat.STYLE_ALL);
        NumFormat roundTrip = NumFormat.INVARIANT.withGrouping(Grouping.INDIAN).withGrouping(Grouping.WESTERN);
        NumFormat respelled = NumFormat.INVARIANT.withSeparators(".", ",");
        assertEquals(1, new HashSet<>(List.of(NumFormat.INVARIANT, declared, roundTrip, respelled)).size());
        for (NumFormat same : new NumFormat[] {declared, roundTrip, respelled}) {
            assertEquals(NumFormat.INVARIANT, same);
            assertEquals(NumFormat.INVARIANT.hashCode(), same.hashCode());
        }
        assertNotEquals(NumFormat.INVARIANT, NumFormat.INVARIANT.withGrouping(Grouping.INDIAN));
        assertNotEquals(NumFormat.INVARIANT, NumFormat.DETECT);
        assertNotEquals(NumFormat.INVARIANT, NumFormat.INVARIANT.withCurrencySymbol("$"));
        assertNotEquals(NumFormat.INVARIANT, NumFormat.INVARIANT.withDigitZero(0x0660));
    }

    @Test
    void dateDeclarationsEqualOnesDeclaringTheSameThing() {
        assertEquals(new DateTimeFormat(DateOrder.DAY_MONTH_YEAR), new DateTimeFormat(DateOrder.DAY_MONTH_YEAR));
        assertNotEquals(new DateTimeFormat(DateOrder.DAY_MONTH_YEAR), new DateTimeFormat(DateOrder.MONTH_DAY_YEAR));
        DateOptions options = new DateOptions(0).withPivot(2050);
        assertEquals(options, new DateOptions(0).withPivot(2050));
        assertEquals(options.hashCode(), new DateOptions(0).withPivot(2050).hashCode());
        assertNotEquals(options, new DateOptions(0).withPivot(2049));
        MonthNames january = months("janvier");
        MonthNames same = MonthNames.ofPacked(january.packed());
        assertEquals(january, same);
        assertEquals(january.hashCode(), same.hashCode());
        assertNotEquals(january, months("january"));
    }

    private static MonthNames months(String first) {
        MonthNames.Builder builder = MonthNames.builder().add(1, first);
        for (int month = 2; month <= 12; month++) {
            builder.add(month, "m" + (char) ('a' + month));
        }
        return builder.build();
    }
}
