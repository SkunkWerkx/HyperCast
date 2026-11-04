package hypercast

import (
	"errors"
	"math"
	"strings"
	"testing"
	"time"
)

// The 32 published detail codes and their names, restated from the spec's table
// (rust/src/detail.rs): a renumbered or renamed code fails here.
var publishedDetails = []string{
	"blank", "unexpected_character", "digit_expected", "delimiter_expected", "trailing_text",
	"no_digits", "sign_misplaced", "parentheses_unbalanced", "mark_repeated", "group_misplaced",
	"group_size", "group_in_fraction", "ambiguous_separator", "fraction_not_allowed",
	"field_width", "wrong_length", "component_missing", "not_recognized", "unknown_unit",
	"invalid_utf8", "too_long", "not_finite", "not_whole", "negative_not_allowed", "too_large",
	"too_small", "too_precise", "date_field_out_of_range", "time_field_out_of_range",
	"outside_window", "nonexistent_date", "not_a_scalar",
}

func TestDetailCodesAndNames(t *testing.T) {
	for i, name := range publishedDetails {
		d := Detail(i + 1)
		if d.String() != name || !d.Known() {
			t.Errorf("Detail(%d) = %q known=%v, want %q", i+1, d, d.Known(), name)
		}
	}
	for _, pin := range []struct {
		d    Detail
		code uint32
	}{{DetailBlank, 1}, {DetailGroupSize, 11}, {DetailNotFinite, 22}, {DetailNotAScalar, 32}} {
		if uint32(pin.d) != pin.code {
			t.Errorf("%v is %d, want %d", pin.d, uint32(pin.d), pin.code)
		}
	}
	if Detail(0).String() != "none" || Detail(0).Known() {
		t.Errorf("Detail(0) = %q known=%v", Detail(0), Detail(0).Known())
	}
}

// A code this binding does not name yet (a later minor release may publish it) decodes
// without a panic: the reason and span stay usable, and the code is kept.
func TestUnknownDetailDecodes(t *testing.T) {
	f, ok := FailureFromCode(2, 1, 3, 999)
	if !ok {
		t.Fatal("FailureFromCode refused a malformed verdict carrying detail 999")
	}
	if f.Reason != Malformed || f.Offset != 1 || f.Length != 3 || f.Detail != 999 {
		t.Fatalf("got %+v", f)
	}
	if f.Detail.Known() || f.Detail.String() != "detail 999" {
		t.Fatalf("Detail(999) = %q known=%v", f.Detail, f.Detail.Known())
	}
	if f.Error() != "hypercast: malformed (detail 999) at byte 1..4" {
		t.Fatalf("Error() = %q", f.Error())
	}
	if *f.Fault() != (Fault{Malformed, 1, 3}) {
		t.Fatalf("Fault() = %v", f.Fault())
	}
	// The path every detailed door takes, over the RawFaultDetail as the core writes it.
	got := failedDetailed(3, &rawFault{Offset: 4, Length: 2, Detail: 999})
	if got.Reason != OutOfRange || got.Offset != 4 || got.Length != 2 || got.Detail != 999 {
		t.Fatalf("failedDetailed: %+v", got)
	}
	if _, ok := FailureFromCode(0, 0, 0, 1); ok {
		t.Fatal("code 0 is a success, not a failure")
	}
}

func TestDetailedDoorsReportTheDetail(t *testing.T) {
	_, f := I32Detailed("12,34", Invariant)
	if f == nil || *f != (Failure{Reason: Malformed, Detail: DetailGroupSize, Offset: 2, Length: 3}) {
		t.Fatalf("I32Detailed 12,34: %+v", f)
	}
	if _, f := BoolDetailed("maybe"); f == nil || f.Detail != DetailNotRecognized {
		t.Fatalf("BoolDetailed: %+v", f)
	}
	if _, f := CharDetailed(""); f == nil || f.Detail != DetailBlank || f.Reason != Empty {
		t.Fatalf("CharDetailed: %+v", f)
	}
	if v, f := CharDetailed("U+1F600"); f != nil || v != 0x1F600 {
		t.Fatalf("CharDetailed U+1F600: %v %+v", v, f)
	}
	if _, f := ExactFromFloat64Detailed(math.NaN()); f == nil || f.Detail != DetailNotFinite || f.Reason != Malformed {
		t.Fatalf("ExactFromFloat64Detailed(NaN): %+v", f)
	}
	if _, f := ExcelSerialFromFloat64Detailed(60, Excel1900); f == nil || f.Detail != DetailNonexistentDate {
		t.Fatalf("serial 60: %+v", f)
	}
	if _, f := StringDetailed("\xff"); f == nil || f.Detail != DetailInvalidUTF8 {
		t.Fatalf("StringDetailed: %+v", f)
	}
	if v, f := StringDetailed("abc"); f != nil || v != "abc" {
		t.Fatalf("StringDetailed abc: %q %+v", v, f)
	}
	if v, f := NumericDetailed[uint8]("255", Invariant); f != nil || v != 255 {
		t.Fatalf("NumericDetailed: %v %+v", v, f)
	}
	if _, f := ScalarDetailed[uint8]("256", Invariant); f == nil || f.Detail != DetailTooLarge {
		t.Fatalf("ScalarDetailed 256: %+v", f)
	}
	if _, f := UnixDetailed("253402300800", Seconds); f == nil || f.Detail != DetailOutsideWindow {
		t.Fatalf("UnixDetailed: %+v", f)
	}
	// The plain door is the same parse, projected: the reason and span agree.
	for _, text := range []string{"12,34", "abc", "", "1e", "(-$5)", "99999999999"} {
		_, fault := I32(text, Invariant)
		_, failure := I32Detailed(text, Invariant)
		if (fault == nil) != (failure == nil) || fault != nil && *fault != *failure.Fault() {
			t.Errorf("%q: I32 %v, I32Detailed %+v", text, fault, failure)
		}
	}
}

func TestOptionalDetailed(t *testing.T) {
	if v, f := OptionalDetailed(I32Detailed("  ", Invariant)); v != nil || f != nil {
		t.Fatalf("blank: %v %v", v, f)
	}
	if v, f := OptionalDetailed(I32Detailed("7", Invariant)); f != nil || *v != 7 {
		t.Fatalf("7: %v %v", v, f)
	}
	if _, f := OptionalDetailed(I32Detailed("x", Invariant)); f == nil || f.Detail != DetailUnexpectedCharacter {
		t.Fatalf("x: %v", f)
	}
	c := MustConstraints[int32](Maximum(5))
	if v, r := OptionalCheckedDetailed(CheckDetailed("", Invariant, c)); v != nil || r != nil {
		t.Fatalf("checked blank: %v %v", v, r)
	}
	if _, r := OptionalCheckedDetailed(CheckDetailed("6", Invariant, c)); r == nil {
		t.Fatal("6 passed a maximum of 5")
	}
}

func TestCheckedDetailed(t *testing.T) {
	c := MustConstraints[int32](Maximum(100))
	_, r := CheckDetailed("1x", Invariant, c)
	switch r := r.(type) {
	case *Failure:
		if r.Detail != DetailUnexpectedCharacter || r.Offset != 1 || r.Length != 1 {
			t.Fatalf("1x: %+v", r)
		}
	default:
		t.Fatalf("1x: %v, want a *Failure", r)
	}
	_, r = CheckDetailed("101", Invariant, c)
	if v, ok := r.(*Violation); !ok || *v != (Violation{KeywordMaximum, 0, 3}) {
		t.Fatalf("101: %v", r)
	}
	if got, r := CheckDetailed("12,34", Invariant.WithGrouping(GroupingLegacy), c); r == nil || got.EnumIndex != -1 {
		t.Fatalf("legacy 1234 against 100: %v %v", got, r)
	}
	if got, r := CheckFromFloat64Detailed(math.Inf(1), c); r == nil || got.EnumIndex != -1 {
		t.Fatalf("Inf: %v", r)
	} else if f, ok := r.(*Failure); !ok || f.Detail != DetailNotFinite {
		t.Fatalf("Inf: %v", r)
	}
	words := MustConstraints[string](Enum("Yes", "No"))
	if got, r := CheckStringDetailed("yes", words); r != nil || got.Value != "Yes" || got.EnumIndex != 0 {
		t.Fatalf("yes: %v %v", got, r)
	}
	if _, r := CheckStringDetailed("", words); r == nil || r.(*Failure).Detail != DetailBlank {
		t.Fatalf("blank: %v", r)
	}
}

func TestGroupingPolicy(t *testing.T) {
	f := NewNumFormat(".", ",", AllStyles)
	if f.GroupingPolicy() != GroupingWestern || f.Styles&^GroupingPolicyMask != AllStyles {
		t.Fatalf("NewNumFormat: %+v", f)
	}
	if f != Invariant {
		t.Fatalf("NewNumFormat(\".\", \",\", AllStyles) = %+v, want Invariant", f)
	}
	indian := f.WithGrouping(GroupingIndian)
	if indian.GroupingPolicy() != GroupingIndian {
		t.Fatalf("WithGrouping(Indian) = %v", indian.GroupingPolicy())
	}
	// The policy is replaced, never OR'ed: Indian then Western is Western, not policy 3.
	if back := indian.WithGrouping(GroupingWestern); back != f {
		t.Fatalf("Indian then Western: %+v", back)
	}
	if v, fault := I32("12,34,567", indian); fault != nil || v != 1234567 {
		t.Fatalf("Indian 12,34,567: %v %v", v, fault)
	}
	if _, fault := I32("1,234,567", indian); fault == nil || *fault != (Fault{Malformed, 1, 4}) {
		t.Fatalf("Indian 1,234,567: %v", fault)
	}
	// WithStyles keeps the policy.
	styled := indian.WithStyles(Grouping | Parentheses)
	if styled.GroupingPolicy() != GroupingIndian || styled.Styles&^GroupingPolicyMask != Grouping|Parentheses {
		t.Fatalf("WithStyles: %+v", styled)
	}
	for _, p := range []GroupingPolicy{GroupingLegacy, GroupingWestern, GroupingIndian} {
		if p.String() == "" || strings.HasPrefix(p.String(), "GroupingPolicy(") {
			t.Errorf("%d has no name", uint32(p))
		}
	}
	for name, call := range map[string]func(){
		"NewNumFormat with a policy in styles": func() { NewNumFormat(".", ",", AllStyles|NumStyles(GroupingIndian)) },
		"WithStyles with a policy":             func() { f.WithStyles(GroupingPolicyMask) },
		"WithGrouping(3 << 8)":                 func() { f.WithGrouping(GroupingPolicy(3 << 8)) },
		"WithGrouping(1)":                      func() { f.WithGrouping(GroupingPolicy(1)) },
	} {
		if panics(call) == "" {
			t.Errorf("%s did not panic", name)
		}
	}
}

// Review focus 1 and 5: native digits with a space-like group separator under strict Western,
// and a strict format with grouping off.
func TestStrictGroupingReviewFocus(t *testing.T) {
	arabic := NewNumFormat(".", "\u202f", AllStyles)
	arabic.DigitZero = '٠'
	if v, f := I32Detailed("١\u202f٢٣٤", arabic); f != nil || v != 1234 {
		t.Fatalf("١ ٢٣٤: %v %+v", v, f)
	}
	text := "١٢\u202f٣٤"
	_, f := I32Detailed(text, arabic)
	if f == nil || f.Reason != Malformed || f.Detail != DetailGroupSize || text[f.Offset:f.Offset+f.Length] != "\u202f٣٤" {
		t.Fatalf("١٢ ٣٤: %+v", f)
	}
	ungrouped := Invariant.WithStyles(AllStyles &^ Grouping)
	if _, f := I32Detailed("1,234", ungrouped); f == nil || f.Reason != Malformed {
		t.Fatalf("strict without Grouping read 1,234: %+v", f)
	}
}

func TestNumFormatRefusesPolicyThreeAndUndefinedBits(t *testing.T) {
	for _, c := range []struct {
		styles NumStyles
		want   string
	}{
		{AllStyles | GroupingPolicyMask, "names no grouping policy"},
		{AllStyles | 1<<10, "does not define"},
		{1 << 31, "does not define"},
	} {
		f := NumFormat{DecimalSep: ".", GroupSep: ",", Styles: c.styles}
		if f.valid() {
			t.Errorf("valid() accepted %#x", uint32(c.styles))
		}
		if _, err := f.Raw(); err == nil || !strings.Contains(err.Error(), c.want) {
			t.Errorf("Raw() of %#x: %v, want %q", uint32(c.styles), err, c.want)
		}
		if text := panics(func() { I32Detailed("5", f) }); !strings.Contains(text, c.want) {
			t.Errorf("I32Detailed with %#x panicked %q", uint32(c.styles), text)
		}
	}
}

func janvVocabulary(t *testing.T) MonthNames {
	t.Helper()
	names, err := NewMonthNames(
		MonthName{time.January, "janv."}, MonthName{time.February, "févr."}, MonthName{time.March, "mars"},
		MonthName{time.April, "avr."}, MonthName{time.May, "mai"}, MonthName{time.June, "juin"},
		MonthName{time.July, "juil."}, MonthName{time.August, "août"}, MonthName{time.September, "sept."},
		MonthName{time.October, "oct."}, MonthName{time.November, "nov."}, MonthName{time.December, "déc."},
	)
	if err != nil {
		t.Fatal(err)
	}
	return names
}

// Review focus 2: a dotted month name, the '.' separator and a pivot together.
func TestDateOptionsReviewFocus(t *testing.T) {
	options := DateOptions{Pivot: 2029, Months: janvVocabulary(t)}
	if d, f := DateOnlyOrderedDetailed("7.janv..26", DayMonthYear, options); f != nil || d != (Date{2026, time.January, 7}) {
		t.Fatalf("7.janv..26: %v %+v", d, f)
	}
	if _, f := DateOnlyOrderedDetailed("7.janv.26", DayMonthYear, options); f == nil || f.Reason != Malformed {
		t.Fatalf("7.janv.26: %+v", f)
	}
}

func TestDateOptionsAtEachDoor(t *testing.T) {
	if d, f := DateOnlyDetailed("20260107", DateOptions{Shapes: CompactDate}); f != nil || d != (Date{2026, time.January, 7}) {
		t.Fatalf("compact: %v %+v", d, f)
	}
	if _, f := DateOnlyDetailed("20260107", DateOptions{}); f == nil {
		t.Fatal("20260107 read without CompactDate")
	}
	ts, f := TimestampDetailed("2026-01-07 15:04:05+00", DateOptions{Shapes: CommonForms})
	if f != nil || !ts.Equal(time.Date(2026, 1, 7, 15, 4, 5, 0, time.UTC)) {
		t.Fatalf("common forms timestamp: %v %+v", ts, f)
	}
	if _, f := TimestampDetailed("2026-01-07 15:04:05+00", DateOptions{}); f == nil {
		t.Fatal("a space and +00 read without options")
	}
	months := MustMonthNames(
		MonthName{1, "Jan"}, MonthName{2, "Feb"}, MonthName{3, "Mar"}, MonthName{4, "Apr"},
		MonthName{5, "May"}, MonthName{6, "Jun"}, MonthName{7, "Jul"}, MonthName{8, "Aug"},
		MonthName{9, "Sep"}, MonthName{9, "Sept"}, MonthName{10, "Oct"}, MonthName{11, "Nov"},
		MonthName{12, "Dec"},
	)
	c, f := DateTimeDetailed("07-Jan-26 3:04 PM", DateFormat{Order: DayMonthYear}, DateOptions{Pivot: 2029, Months: months})
	if f != nil || c != (CivilDateTime{Date{2026, time.January, 7}, 15*time.Hour + 4*time.Minute}) {
		t.Fatalf("07-Jan-26 3:04 PM: %v %+v", c, f)
	}
	// The vocabulary is independent of the options' bits and pivot.
	if d, f := DateOnlyOrderedDetailed("7-Sept-2026", DayMonthYear, DateOptions{Months: months}); f != nil || d != (Date{2026, time.September, 7}) {
		t.Fatalf("7-Sept-2026: %v %+v", d, f)
	}
	// With no options, the 1.0 doors and their detailed twins agree.
	for _, text := range []string{"2026-01-07", "2026-1-07", "", "20260107"} {
		d, fault := DateOnly(text)
		dd, failure := DateOnlyDetailed(text, DateOptions{})
		if d != dd || (fault == nil) != (failure == nil) || fault != nil && *fault != *failure.Fault() {
			t.Errorf("%q: DateOnly %v %v, DateOnlyDetailed %v %+v", text, d, fault, dd, failure)
		}
	}
}

func TestDateOptionsRefusals(t *testing.T) {
	for _, c := range []struct {
		options DateOptions
		want    string
	}{
		{DateOptions{Shapes: 8}, "does not define"},
		{DateOptions{Pivot: 99}, "pivot"},
		{DateOptions{Pivot: 10000}, "pivot"},
		{DateOptions{Pivot: -1}, "pivot"},
		{DateOptions{Shapes: 8, Pivot: 99}, "does not define"},
	} {
		if _, err := c.options.Raw(); err == nil || !strings.Contains(err.Error(), c.want) {
			t.Errorf("Raw() of %+v: %v, want %q", c.options, err, c.want)
		}
		if text := panics(func() { DateOnlyDetailed("2026-01-07", c.options) }); !strings.Contains(text, c.want) {
			t.Errorf("DateOnlyDetailed with %+v panicked %q", c.options, text)
		}
	}
	raw, err := DateOptions{Shapes: CommonForms, Pivot: 2029}.Raw()
	if err != nil || raw != (RawDateOptions{Size: RawDateOptionsSize, Flags: 7, Pivot: 2029}) {
		t.Fatalf("Raw(): %+v %v", raw, err)
	}
}

func TestMonthNamesRefusals(t *testing.T) {
	full := []MonthName{
		{1, "Jan"}, {2, "Feb"}, {3, "Mar"}, {4, "Apr"}, {5, "May"}, {6, "Jun"},
		{7, "Jul"}, {8, "Aug"}, {9, "Sep"}, {10, "Oct"}, {11, "Nov"}, {12, "Dec"},
	}
	with := func(extra ...MonthName) []MonthName { return append(append([]MonthName(nil), full...), extra...) }
	for _, c := range []struct {
		names []MonthName
		want  string
	}{
		{nil, "has no name"},
		{full[1:], "has no name"},
		{with(MonthName{13, "x"}), "1 through 12"},
		{with(MonthName{1, ""}), "1 to 64"},
		{with(MonthName{1, "J1"}), "must not contain"},
		{with(MonthName{1, "J n"}), "must not contain"},
		{with(MonthName{2, "JAN"}), "fold to the same spelling"},
		{with(MonthName{10, "\u212a"}, MonthName{11, "k"}), "fold to the same spelling"},
		// Rules apply in reading order: the duplicate at entry 12 wins over the digit after it.
		{with(MonthName{2, "jan"}, MonthName{3, "M4r"}), "fold to the same spelling"},
	} {
		_, err := NewMonthNames(c.names...)
		if err == nil || !strings.Contains(err.Error(), c.want) {
			t.Errorf("NewMonthNames(%v): %v, want %q", c.names, err, c.want)
		}
		if text := panics(func() { MustMonthNames(c.names...) }); err != nil && text != err.Error() {
			t.Errorf("MustMonthNames(%v) panicked %q, want %q", c.names, text, err)
		}
	}
	names, err := NewMonthNames(with(MonthName{10, "ß"}, MonthName{10, "ss"})...)
	if err != nil {
		t.Fatalf("ß and ss are distinct under simple folding: %v", err)
	}
	packed := names.Packed()
	if packed[0] != 1 || packed[1] != 3 || string(packed[5:8]) != "Jan" {
		t.Fatalf("Packed() starts %v", packed[:8])
	}
	if again, err := monthNamesFromPacked(packed); err != nil || again != names {
		t.Fatalf("packed round trip: %v", err)
	}
	if _, err := monthNamesFromPacked(append(packed, 3, 5)); err == nil || !strings.Contains(err.Error(), "cut short") {
		t.Fatalf("cut short: %v", err)
	}
}

func TestDetailedAllocations(t *testing.T) {
	months := janvVocabulary(t)
	assertAllocs(t, "I32Detailed", 0, func() { I32Detailed("1,234,567", Invariant) })
	assertAllocs(t, "DateOnlyOrderedDetailed (vocabulary)", 0, func() {
		DateOnlyOrderedDetailed("7.janv..2026", DayMonthYear, DateOptions{Months: months})
	})
	assertAllocs(t, "TimestampDetailed (options)", 0, func() {
		TimestampDetailed("2026-01-07 15:04:05+00", DateOptions{Shapes: CommonForms})
	})
	assertAllocs(t, "I32Detailed (malformed)", 1, func() { I32Detailed("12x4", Invariant) })
}

func TestFailureIsADetailedRejection(t *testing.T) {
	var r DetailedRejection = &Failure{Reason: Empty, Detail: DetailBlank}
	var f *Failure
	if !errors.As(r, &f) || f.Detail != DetailBlank {
		t.Fatal("errors.As")
	}
	var nilFailure *Failure
	if nilFailure.Error() != "<nil>" {
		t.Fatal("a nil *Failure prints <nil>")
	}
}
