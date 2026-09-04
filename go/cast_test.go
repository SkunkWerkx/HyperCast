package hypercast

// Binding-level behavior the corpus can't express: the (value, *Fault) idiom, zero-copy
// string and []byte agreement, Go-type fidelity (full-nanosecond time.Time, the Duration
// pair and its checked converter), and the caller-bug guards.

import (
	"math"
	"math/big"
	"os"
	"path/filepath"
	"strings"
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

func TestCurrencySymbolAtEitherEdge(t *testing.T) {
	dollars := NumFormat{DecimalSep: '.', GroupSep: ',', Styles: AllStyles, Currency: "$"}
	for _, c := range []struct {
		text string
		want int32
	}{
		{"$1,234", 1234}, {"-$5", -5}, {"$ -5", -5}, {"($5)", -5}, {"5 $", 5},
	} {
		value, fault := I32(c.text, dollars)
		if fault != nil || value != c.want {
			t.Errorf("%q: got %v %v, want %d", c.text, value, fault, c.want)
		}
	}
	// The symbol is matched whole and never inside the digit scan, so a symbol that
	// contains the grouping separator is fine.
	danish := NumFormat{DecimalSep: ',', GroupSep: '.', Styles: AllStyles, Currency: "kr."}
	value, fault := F64("1.234,50 kr.", danish)
	if fault != nil || value != 1234.5 {
		t.Fatalf("got %v %v", value, fault)
	}
}

func TestCurrencyDeclaredButStyleOffIsMalformedAtTheSymbol(t *testing.T) {
	format := NumFormat{DecimalSep: '.', GroupSep: ',', Styles: AllStyles &^ CurrencySymbol, Currency: "$"}
	_, fault := I32("$5", format)
	if fault == nil || fault.Reason != Malformed || fault.Offset != 0 || fault.Length != 1 {
		t.Fatalf("got %v", fault)
	}
	// The flag with nothing declared is a no-op, and Invariant declares nothing.
	if _, fault := I32("$5", Invariant); fault == nil || fault.Reason != Malformed {
		t.Fatalf("undeclared symbol should be Malformed, got %v", fault)
	}
}

func TestInvalidCurrencySymbolsPanic(t *testing.T) {
	for _, symbol := range []string{"$1", "US $", "\t€", "seventeen-bytes!!", "\xff"} {
		func() {
			defer func() {
				if recover() == nil {
					t.Errorf("%q: expected panic", symbol)
				}
			}()
			I32("5", NumFormat{DecimalSep: '.', GroupSep: ',', Styles: AllStyles, Currency: symbol})
		}()
	}
	// Exactly 16 bytes is the ceiling, not past it.
	sixteen := NumFormat{DecimalSep: '.', GroupSep: ',', Styles: AllStyles, Currency: "sixteen-bytes-ok"}
	if value, fault := I32("sixteen-bytes-ok 5", sixteen); fault != nil || value != 5 {
		t.Fatalf("got %v %v", value, fault)
	}
}

func TestExactIsExact(t *testing.T) {
	// Canonical: exact trailing zeros are trimmed, so the scale is minimal and 1.10, 1.1
	// and 1.1000 are all magnitude 11 at scale 1.
	for _, text := range []string{"1.10", "1.1", "1.1000"} {
		value, fault := Exact(text, Invariant)
		if fault != nil || value != (Decimal{Lo: 11, Scale: 1}) || value.String() != "1.1" {
			t.Fatalf("%q: got %v %v", text, value, fault)
		}
	}
	// 0.1 is exactly 1/10 — no binary approximation anywhere.
	value, fault := Exact("0.1", Invariant)
	if fault != nil || value.Rat().Cmp(big.NewRat(1, 10)) != 0 {
		t.Fatalf("got %v %v", value, fault)
	}
	// Negative zero collapses: zero is never negative and always scale 0.
	value, fault = Exact("-0.00", Invariant)
	if fault != nil || value != (Decimal{}) || value.String() != "0" {
		t.Fatalf("got %v %v", value, fault)
	}
	// Accounting negative, currency, and the sign in String().
	value, fault = Exact("($1,234.50)", NumFormat{DecimalSep: '.', GroupSep: ',', Styles: AllStyles, Currency: "$"})
	if fault != nil || value != (Decimal{Lo: 12345, Scale: 1, Negative: true}) || value.String() != "-1234.5" {
		t.Fatalf("got %v %v", value, fault)
	}
	// The high word is live: 2^96 - 1 is the ceiling, one more is OutOfRange, not rounded.
	value, fault = Exact("79228162514264337593543950335", Invariant)
	if fault != nil || value != (Decimal{Lo: math.MaxUint64, Hi: math.MaxUint32}) {
		t.Fatalf("got %v %v", value, fault)
	}
	if value.String() != "79228162514264337593543950335" {
		t.Fatalf("got %q", value.String())
	}
	if _, fault := Exact("79228162514264337593543950336", Invariant); fault == nil || fault.Reason != OutOfRange {
		t.Fatalf("expected OutOfRange, got %v", fault)
	}
	if _, fault := Exact("1e-29", Invariant); fault == nil || fault.Reason != OutOfRange {
		t.Fatalf("scale 29 should be OutOfRange, got %v", fault)
	}
}

func TestNativeVersionMatchesTheCrate(t *testing.T) {
	// The expectation is the crate's own manifest, walked up to the way corpus_test.go finds
	// corpus/, so a release bump never leaves a stale literal here.
	dir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for {
		text, readErr := os.ReadFile(filepath.Join(dir, "rust", "Cargo.toml"))
		if readErr == nil {
			for _, line := range strings.Split(string(text), "\n") {
				line = strings.TrimRight(line, "\r")
				if rest, ok := strings.CutPrefix(line, "version = \""); ok {
					want := rest[:strings.IndexByte(rest, '"')]
					if got := NativeVersion(); got != want {
						t.Fatalf("got %q, rust/Cargo.toml says %q", got, want)
					}
					return
				}
			}
			t.Fatal("no version line in rust/Cargo.toml")
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Fatal("rust/Cargo.toml not found above the test directory")
		}
		dir = parent
	}
}

func TestAvailableIsTheNonPanickingProbe(t *testing.T) {
	if !Available() {
		t.Fatal("the embedded core should load on every CI leg")
	}
	// Cached: the second answer is the same one, not a second probe.
	if !Available() {
		t.Fatal("availability flipped between probes")
	}
}

// numericTarget pins Numeric[V] to its concrete door for one target: the same verdict,
// value for value, fault for fault.
func numericTarget[V Number](t *testing.T, text string, door func(string, NumFormat) (V, *Fault)) {
	t.Helper()
	got, gotFault := Numeric[V](text, Invariant)
	want, wantFault := door(text, Invariant)
	if gotFault != nil || wantFault != nil {
		t.Fatalf("%T %q: unexpected fault %v / %v", want, text, gotFault, wantFault)
	}
	if got != want {
		t.Fatalf("%T %q: Numeric gave %v, door gave %v", want, text, got, want)
	}
}

func TestNumericDispatchesToEveryDoor(t *testing.T) {
	numericTarget[int8](t, "-128", I8[string])
	numericTarget[int16](t, "-32,768", I16[string])
	numericTarget[int32](t, "(2,147,483,648)", I32[string])
	numericTarget[int64](t, "9223372036854775807", I64[string])
	numericTarget[uint8](t, "255", U8[string])
	numericTarget[uint16](t, "0xFFFF", U16[string])
	numericTarget[uint32](t, "4,294,967,295", U32[string])
	numericTarget[uint64](t, "18446744073709551615", U64[string])
	numericTarget[float32](t, "1.5e3", F32[string])
	numericTarget[float64](t, "50%", F64[string])
	numericTarget[Decimal](t, "1,234.50", Exact[string])
	// T is inferred from the argument; []byte crosses the same way.
	if value, fault := Numeric[int32]([]byte("42"), Invariant); fault != nil || value != 42 {
		t.Fatalf("got %v %v", value, fault)
	}
}

func TestNumericPassesTheFaultThrough(t *testing.T) {
	value, fault := Numeric[int32]("  12x4", Invariant)
	if value != 0 || fault == nil || fault.Reason != Malformed || fault.Offset != 4 || fault.Length != 1 {
		t.Fatalf("got %v %v", value, fault)
	}
	if _, fault := Numeric[uint8]("256", Invariant); fault == nil || fault.Reason != OutOfRange {
		t.Fatalf("got %v", fault)
	}
	if d, fault := Numeric[Decimal]("1e29", Invariant); d != (Decimal{}) || fault == nil || fault.Reason != OutOfRange {
		t.Fatalf("got %v %v", d, fault)
	}
	if _, fault := Numeric[float64]("", Invariant); fault == nil || fault.Reason != Empty {
		t.Fatalf("got %v", fault)
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
