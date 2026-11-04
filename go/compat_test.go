package hypercast_test

// The 1.0 surface, written as 1.0 code wrote it: unkeyed literals of every exported struct,
// equality, the sealed Rejection switched on with no default arm, and the 1.0 doors' (value,
// *Fault) pair. 1.1 adds no field to any of these types (the detail travels on the separate
// *Failure path), so all of it compiles and behaves as before; the one behaviour that moves
// is the strict default of Invariant and Detect, pinned at the end.

import (
	"bytes"
	"encoding/gob"
	"encoding/json"
	"testing"
	"time"

	"github.com/SkunkWerkx/HyperCast/go"
)

func TestOneDotZeroLiteralsStillCompileAndCompare(t *testing.T) {
	legacy := hypercast.NumFormat{".", ",", hypercast.AllStyles, "", "", "", "", "", "", 0}
	if legacy != (hypercast.NumFormat{DecimalSep: ".", GroupSep: ",", Styles: hypercast.AllStyles}) {
		t.Fatal("an unkeyed NumFormat differs from its keyed twin")
	}
	fault := hypercast.Fault{hypercast.Malformed, 2, 3}
	if fault != (hypercast.Fault{Reason: hypercast.Malformed, Offset: 2, Length: 3}) {
		t.Fatal("Fault equality moved")
	}
	violation := hypercast.Violation{hypercast.KeywordMaximum, 0, 2}
	if violation != (hypercast.Violation{Keyword: hypercast.KeywordMaximum, Length: 2}) {
		t.Fatal("Violation equality moved")
	}
	checked := hypercast.Checked[int32]{5, false, -1}
	if checked != (hypercast.Checked[int32]{Value: 5, EnumIndex: -1}) {
		t.Fatal("Checked equality moved")
	}
	_ = hypercast.DateFormat{hypercast.DayMonthYear, "", ""}
	_ = hypercast.Date{2026, time.January, 7}
	_ = hypercast.CivilDateTime{hypercast.Date{2026, time.January, 7}, time.Hour}
	_ = hypercast.Duration{1, 2}
	_ = hypercast.Decimal{1, 0, 0, false}
	_ = hypercast.RawNumFormat{hypercast.RawNumFormatSize, 0, 0, hypercast.RawSymbol8{}, hypercast.RawSymbol8{},
		hypercast.RawSymbol8{}, hypercast.RawSymbol8{}, hypercast.RawSymbol8{}, hypercast.RawSymbol8{},
		hypercast.RawSymbol8{}, hypercast.RawSymbol16{}}
	_ = hypercast.RawCheck{0, 0, 0}
	_ = hypercast.RawSpan{0, 0}
}

func TestOneDotZeroDoorsKeepTheirVerdicts(t *testing.T) {
	// A format written as a 1.0 literal declares no policy: legacy, as 1.0 parsed it.
	legacy := hypercast.NumFormat{".", ",", hypercast.AllStyles, "", "", "", "", "", "", 0}
	if v, fault := hypercast.I32("12,34", legacy); fault != nil || v != 1234 {
		t.Fatalf("legacy literal: %v %v, want 1234", v, fault)
	}
	if _, fault := hypercast.I32("12x4", legacy); fault == nil || *fault != (hypercast.Fault{hypercast.Malformed, 2, 1}) {
		t.Fatalf("12x4: %v, want malformed at 2..3", fault)
	}
	// The zero value's Styles is policy 00.
	var zero hypercast.NumFormat
	zero.DecimalSep, zero.GroupSep = ".", ","
	zero.Styles = hypercast.Grouping
	if v, fault := hypercast.I32("1,2,3,4", zero); fault != nil || v != 1234 {
		t.Fatalf("zero-value styles: %v %v, want 1234", v, fault)
	}

	cents := hypercast.MustConstraints[int32](hypercast.Maximum(100))
	_, rejection := hypercast.Check("101", legacy, cents)
	switch r := rejection.(type) {
	case nil:
		t.Fatal("101 passed a maximum of 100")
	case *hypercast.Fault:
		t.Fatalf("101 faulted: %v", r)
	case *hypercast.Violation:
		if *r != (hypercast.Violation{hypercast.KeywordMaximum, 0, 3}) {
			t.Fatalf("violation %v", r)
		}
	}
}

// A NumFormat stored by 1.0 code (encoding/json and encoding/gob carry its exported fields)
// keeps 1.0's meaning when it is read back: its Styles carry no policy bits, so it is legacy.
func TestOneDotZeroStoredFormatsStayLegacy(t *testing.T) {
	// What json.Marshal(hypercast.Invariant) wrote under 1.0, where AllStyles was 95.
	payload := `{"DecimalSep":".","GroupSep":",","Styles":95,"Currency":"","CurrencyDecimalSep":"",` +
		`"CurrencyGroupSep":"","MinusSign":"","PlusSign":"","Percent":"","DigitZero":0}`
	var stored hypercast.NumFormat
	if err := json.Unmarshal([]byte(payload), &stored); err != nil {
		t.Fatal(err)
	}
	if stored.GroupingPolicy() != hypercast.GroupingLegacy {
		t.Fatalf("a 1.0 JSON payload reads as %v, want legacy", stored.GroupingPolicy())
	}
	if v, fault := hypercast.I32("12,34", stored); fault != nil || v != 1234 {
		t.Fatalf("a 1.0 JSON payload: %v %v, want 1234", v, fault)
	}

	var buf bytes.Buffer
	old := hypercast.NumFormat{DecimalSep: ".", GroupSep: ",", Styles: hypercast.AllStyles}
	if err := gob.NewEncoder(&buf).Encode(old); err != nil {
		t.Fatal(err)
	}
	var back hypercast.NumFormat
	if err := gob.NewDecoder(&buf).Decode(&back); err != nil {
		t.Fatal(err)
	}
	if back != old || back.GroupingPolicy() != hypercast.GroupingLegacy {
		t.Fatalf("gob round trip %+v, want %+v (legacy)", back, old)
	}
}

// The one deliberate change: Invariant and Detect select strict Western grouping, so 12,34
// moves from 1234 to Malformed, spanning the short group with its separator.
func TestInvariantIsStrictWestern(t *testing.T) {
	if _, fault := hypercast.I32("12,34", hypercast.Invariant); fault == nil || *fault != (hypercast.Fault{hypercast.Malformed, 2, 3}) {
		t.Fatalf("Invariant 12,34: %v, want malformed at 2..5", fault)
	}
	if v, fault := hypercast.I32("1,234,567", hypercast.Invariant); fault != nil || v != 1234567 {
		t.Fatalf("Invariant 1,234,567: %v %v", v, fault)
	}
	if hypercast.Detect.GroupingPolicy() != hypercast.GroupingWestern {
		t.Fatalf("Detect is %v, want western", hypercast.Detect.GroupingPolicy())
	}
	legacy := hypercast.Invariant.WithGrouping(hypercast.GroupingLegacy)
	if v, fault := hypercast.I32("12,34", legacy); fault != nil || v != 1234 {
		t.Fatalf("Invariant.WithGrouping(GroupingLegacy) 12,34: %v %v, want 1234", v, fault)
	}
}
