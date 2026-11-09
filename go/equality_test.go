package hypercast

import (
	"testing"
	"time"
)

// The declarations are comparable values: == is true for two that declare the same thing,
// however each was made, as the core's equality is, and each keys a map.
func TestDeclarationsCompareByValue(t *testing.T) {
	formats := map[NumFormat]int{}
	formats[NewNumFormat(".", ",", AllStyles)]++
	formats[Invariant]++
	formats[NumFormat{DecimalSep: ".", GroupSep: ",", Styles: AllStyles}]++
	formats[Invariant.WithGrouping(GroupingIndian).WithGrouping(GroupingWestern)]++
	if len(formats) != 1 || formats[Invariant] != 4 {
		t.Fatalf("equal formats are %d keys: %v", len(formats), formats)
	}
	for _, other := range []NumFormat{
		Invariant.WithGrouping(GroupingIndian),
		NewNumFormat(",", ".", AllStyles),
		NewNumFormat(".", ",", Grouping),
		{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, Currency: "$"},
		{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, MinusSign: "−"},
		{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, DigitZero: '٠'},
	} {
		if other == Invariant {
			t.Fatalf("%+v equals Invariant", other)
		}
	}

	german := DateFormat{Order: DayMonthYear, AM: "vorm.", PM: "nachm."}
	if german != (DateFormat{Order: DayMonthYear, AM: "vorm.", PM: "nachm."}) ||
		german == (DateFormat{Order: MonthDayYear, AM: "vorm.", PM: "nachm."}) {
		t.Fatal("a DateFormat compares by its order and designators")
	}

	entries := make([]MonthName, 0, 12)
	for m := time.January; m <= time.December; m++ {
		entries = append(entries, MonthName{Month: m, Name: "m" + string(rune('a'+int(m)))})
	}
	names, err := NewMonthNames(entries...)
	if err != nil {
		t.Fatal(err)
	}
	again, _ := NewMonthNames(entries...)
	options := DateOptions{Shapes: CompactDate, Pivot: 2029, Months: names}
	if options != (DateOptions{Shapes: CompactDate, Pivot: 2029, Months: again}) {
		t.Fatal("options holding the same vocabulary differ")
	}
	reversed := make([]MonthName, len(entries))
	for i, e := range entries {
		reversed[len(entries)-1-i] = e
	}
	other, _ := NewMonthNames(reversed...)
	if options == (DateOptions{Shapes: CompactDate, Pivot: 2029, Months: other}) ||
		options == (DateOptions{Shapes: CompactDate, Pivot: 2029}) {
		t.Fatal("options holding another vocabulary or none are equal")
	}
}
