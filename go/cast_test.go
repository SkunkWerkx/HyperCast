package hypercast

// Binding-level behavior the corpus can't express: the (value, *Fault) idiom, zero-copy
// string and []byte agreement, Go-type fidelity (full-nanosecond time.Time, the Duration
// pair and its checked converter), and the caller-bug guards.

import (
	"testing"
	"time"

	"github.com/google/uuid"
)

func TestFaultSpanPointsAtTheOffendingByte(t *testing.T) {
	_, fault := I32("  12x4", Invariant)
	if fault == nil || fault.Reason != Malformed || fault.Offset != 4 || fault.Length != 1 {
		t.Fatalf("got %v", fault)
	}
}

func TestFaultImplementsError(t *testing.T) {
	_, fault := I32("abc", Invariant)
	var err error = fault
	if err.Error() != "hypercast: malformed at byte 0..1" {
		t.Fatalf("got %q", err.Error())
	}
}

func TestStringAndBytesDoorsAgree(t *testing.T) {
	fromString, faultString := Bool("yes")
	fromBytes, faultBytes := Bool([]byte("yes"))
	if faultString != nil || faultBytes != nil || fromString != true || fromBytes != true {
		t.Fatalf("got %v/%v %v/%v", fromString, faultString, fromBytes, faultBytes)
	}
}

func TestDeclaredSeparators(t *testing.T) {
	eurozone := NumFormat{DecimalSep: ',', GroupSep: '.', Styles: AllStyles}
	value, fault := F64("1.234,5", eurozone)
	if fault != nil || value != 1234.5 {
		t.Fatalf("got %v %v", value, fault)
	}
	french := NumFormat{DecimalSep: ',', GroupSep: ' ', Styles: AllStyles}
	value, fault = F64("1 234,5", french)
	if fault != nil || value != 1234.5 {
		t.Fatalf("got %v %v", value, fault)
	}
}

func TestUuidAgreesWithGoogleUuid(t *testing.T) {
	const text = "01020304-0506-0708-090a-0b0c0d0e0f10"
	value, fault := Uuid("urn:uuid:" + text)
	if fault != nil || value != uuid.MustParse(text) {
		t.Fatalf("got %v %v", value, fault)
	}
}

func TestTimestampKeepsFullNanosecondFidelity(t *testing.T) {
	value, fault := Timestamp("2026-01-02T15:04:05.123456789+05:00")
	expected := time.Date(2026, 1, 2, 10, 4, 5, 123456789, time.UTC)
	if fault != nil || !value.Equal(expected) {
		t.Fatalf("got %v %v", value, fault)
	}
}

func TestDurationPairAndCheckedConverter(t *testing.T) {
	value, fault := Span("PT1.5S")
	if fault != nil || value != (Duration{Seconds: 1, Nanos: 500_000_000}) {
		t.Fatalf("got %v %v", value, fault)
	}
	std, ok := value.AsDuration()
	if !ok || std != 1500*time.Millisecond {
		t.Fatalf("got %v %v", std, ok)
	}
	// The core's ±10,000-year window exceeds time.Duration's ±292-year ceiling — the
	// pair holds it, the converter reports it honestly.
	huge, fault := Span("315576000000s")
	if fault != nil || huge.Seconds != 315_576_000_000 {
		t.Fatalf("got %v %v", huge, fault)
	}
	if _, ok := huge.AsDuration(); ok {
		t.Fatal("expected AsDuration to report out-of-range")
	}
}

func TestEqualSeparatorsPanic(t *testing.T) {
	defer func() {
		if recover() == nil {
			t.Fatal("expected panic")
		}
	}()
	I32("42", NumFormat{DecimalSep: '.', GroupSep: '.', Styles: AllStyles})
}

func TestDateOrderDisambiguatesLikeTheCulturesDo(t *testing.T) {
	// 1/7/2026 is January 7th under en-US's month-first short dates and July 1st under
	// en-GB's day-first ones — resolved only by what the caller declared.
	value, fault := DateOnlyOrdered("1/7/2026", MonthDayYear)
	if fault != nil || value != (Date{Year: 2026, Month: time.January, Day: 7}) {
		t.Fatalf("MonthDayYear: %v, %v", value, fault)
	}
	value, fault = DateOnlyOrdered("1/7/2026", DayMonthYear)
	if fault != nil || value != (Date{Year: 2026, Month: time.July, Day: 1}) {
		t.Fatalf("DayMonthYear: %v, %v", value, fault)
	}
	// Undeclared, the strict door stays strict — the ambiguity is never guessed at.
	if _, fault := DateOnly("1/7/2026"); fault == nil || fault.Reason != Malformed {
		t.Fatalf("undeclared slash date should be Malformed, got %v", fault)
	}
}

func TestDateTimeReadsTheMessyCivilShapes(t *testing.T) {
	// The AM/PM world, zone-less: the value is a CivilDateTime because the text named no
	// zone — fusing one is the caller's job, never the parser's guess.
	value, fault := DateTime("1/7/2026 3:04 PM", MonthDayYear)
	want := CivilDateTime{
		Date:      Date{Year: 2026, Month: time.January, Day: 7},
		TimeOfDay: 15*time.Hour + 4*time.Minute,
	}
	if fault != nil || value != want {
		t.Fatalf("MonthDayYear: %v, %v", value, fault)
	}
	value, fault = DateTime("1/7/2026 3:04 PM", DayMonthYear)
	if fault != nil || value.Date.Month != time.July || value.Date.Day != 1 {
		t.Fatalf("DayMonthYear: %v, %v", value, fault)
	}
	// A zone suffix is not this door's business — Timestamp is the instant door.
	if _, fault := DateTime("1/7/2026 15:04:05Z", MonthDayYear); fault == nil || fault.Reason != Malformed {
		t.Fatalf("zoned text should be Malformed through the civil door, got %v", fault)
	}
}
