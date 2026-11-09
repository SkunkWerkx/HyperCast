package hypercast_test

// The 1.0 surface, written as 1.0 code wrote it: unkeyed literals of every exported struct,
// equality, the sealed Rejection switched on with no default arm, and the 1.0 doors' (value,
// *Fault) pair. 1.1 adds no field to any of these types (the detail travels on the separate
// *Failure path), so all of it compiles and behaves as before; the one behaviour that moves
// is that every format reads Western unless it declares Indian, pinned at the end.

import (
	"bytes"
	"encoding/gob"
	"encoding/json"
	"testing"
	"time"

	"github.com/SkunkWerkx/HyperCast/go"
)

func TestOneDotZeroLiteralsStillCompileAndCompare(t *testing.T) {
	literal := hypercast.NumFormat{".", ",", hypercast.AllStyles, "", "", "", "", "", "", 0}
	if literal != (hypercast.NumFormat{DecimalSep: ".", GroupSep: ",", Styles: hypercast.AllStyles}) {
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
	// A format written as a 1.0 literal declares no policy: Western, like Invariant.
	literal := hypercast.NumFormat{".", ",", hypercast.AllStyles, "", "", "", "", "", "", 0}
	if literal != hypercast.Invariant {
		t.Fatalf("a 1.0 literal is %+v, want Invariant", literal)
	}
	if _, fault := hypercast.I32("1,23,456", literal); fault == nil || *fault != (hypercast.Fault{hypercast.Malformed, 1, 3}) {
		t.Fatalf("literal 1,23,456: %v, want malformed at 1..4", fault)
	}
	if _, fault := hypercast.I32("12x4", literal); fault == nil || *fault != (hypercast.Fault{hypercast.Malformed, 2, 1}) {
		t.Fatalf("12x4: %v, want malformed at 2..3", fault)
	}
	// The zero value's Styles sets no policy.
	var zero hypercast.NumFormat
	zero.DecimalSep, zero.GroupSep = ".", ","
	zero.Styles = hypercast.Grouping
	if v, fault := hypercast.I32("1,234,567", zero); fault != nil || v != 1234567 {
		t.Fatalf("zero-value styles: %v %v, want 1234567", v, fault)
	}
	if _, fault := hypercast.I32("1,2,345", zero); fault == nil || *fault != (hypercast.Fault{hypercast.Malformed, 1, 2}) {
		t.Fatalf("zero-value styles 1,2,345: %v, want malformed at 1..3", fault)
	}

	cents := hypercast.MustConstraints[int32](hypercast.Maximum(100))
	_, rejection := hypercast.Check("101", literal, cents)
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

// A NumFormat stored by 1.0 code (encoding/json and encoding/gob carry its exported fields) reads
// back with what its Styles say: no policy bits, so Western.
func TestOneDotZeroStoredFormatsAreWestern(t *testing.T) {
	// What json.Marshal(hypercast.Invariant) wrote under 1.0, where AllStyles was 95.
	payload := `{"DecimalSep":".","GroupSep":",","Styles":95,"Currency":"","CurrencyDecimalSep":"",` +
		`"CurrencyGroupSep":"","MinusSign":"","PlusSign":"","Percent":"","DigitZero":0}`
	var stored hypercast.NumFormat
	if err := json.Unmarshal([]byte(payload), &stored); err != nil {
		t.Fatal(err)
	}
	if stored != hypercast.Invariant {
		t.Fatalf("a 1.0 JSON payload reads as %+v, want Invariant", stored)
	}
	// JSON refuses a stale policy where it is decoded, and a payload with no Styles stays as it was.
	for _, stale := range []string{"351", "863", "256", "768", "1024", "1119"} {
		var bad hypercast.NumFormat
		if err := json.Unmarshal([]byte(`{"DecimalSep":".","GroupSep":",","Styles":`+stale+`}`), &bad); err == nil {
			t.Fatalf("a JSON Styles of %s decoded", stale)
		}
	}
	// Bit 9 is Go's own encoding of Indian, so a stored 607 is an Indian format.
	var indian hypercast.NumFormat
	if err := json.Unmarshal([]byte(`{"DecimalSep":".","GroupSep":",","Styles":607}`), &indian); err != nil {
		t.Fatal(err)
	} else if indian.GroupingPolicy() != hypercast.GroupingIndian {
		t.Fatalf("a JSON Styles of 607 reads as %v", indian.Styles)
	}
	for _, malformed := range []string{`"95"`, "-1", "1.5", "4294967296", "true"} {
		var bad hypercast.NumFormat
		if err := json.Unmarshal([]byte(`{"DecimalSep":".","GroupSep":",","Styles":`+malformed+`}`), &bad); err == nil {
			t.Fatalf("a JSON Styles of %s decoded", malformed)
		}
	}
	var none hypercast.NumFormat
	if err := json.Unmarshal([]byte(`{"DecimalSep":".","GroupSep":",","Styles":null}`), &none); err != nil || none.Styles != 0 {
		t.Fatalf("a null Styles: %+v %v", none, err)
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
	if back != hypercast.Invariant {
		t.Fatalf("gob round trip %+v, want Invariant", back)
	}
}

// The one deliberate change: every format reads Western unless it declares Indian, so 12,34 and
// 1,23,456 move to Malformed, spanning the short group with its separator.
func TestInvariantIsWestern(t *testing.T) {
	if _, fault := hypercast.I32("12,34", hypercast.Invariant); fault == nil || *fault != (hypercast.Fault{hypercast.Malformed, 2, 3}) {
		t.Fatalf("Invariant 12,34: %v, want malformed at 2..5", fault)
	}
	if v, fault := hypercast.I32("1,234,567", hypercast.Invariant); fault != nil || v != 1234567 {
		t.Fatalf("Invariant 1,234,567: %v %v", v, fault)
	}
	if hypercast.Detect.GroupingPolicy() != hypercast.GroupingWestern {
		t.Fatalf("Detect is %v, want western", hypercast.Detect.GroupingPolicy())
	}
	indian := hypercast.Invariant.WithGrouping(hypercast.GroupingIndian)
	if v, fault := hypercast.I32("1,23,456", indian); fault != nil || v != 123456 {
		t.Fatalf("Invariant.WithGrouping(GroupingIndian) 1,23,456: %v %v, want 123456", v, fault)
	}
	for _, text := range []string{"12,34", "1,2", "0,125", "1234,567", "1,234,567"} {
		if _, fault := hypercast.I32(text, indian); fault == nil {
			t.Fatalf("Invariant.WithGrouping(GroupingIndian) reads %s", text)
		}
	}
}
