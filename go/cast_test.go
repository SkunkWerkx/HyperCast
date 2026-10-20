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

// A door's success is a nil *Fault, and printing it must not dereference it: stock Go's fmt
// recovers that panic, but TinyGo's wasm targets cannot recover and trapped.
func TestNilFaultErrorDoesNotPanic(t *testing.T) {
	var fault *Fault
	if got := fault.Error(); got != "<nil>" {
		t.Fatalf("got %q", got)
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
	eurozone := NumFormat{DecimalSep: ",", GroupSep: ".", Styles: AllStyles}
	value, fault := F64("1.234,5", eurozone)
	if fault != nil || value != 1234.5 {
		t.Fatalf("got %v %v", value, fault)
	}
	french := NumFormat{DecimalSep: ",", GroupSep: " ", Styles: AllStyles}
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
	dollars := NumFormat{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, Currency: "$"}
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
	danish := NumFormat{DecimalSep: ",", GroupSep: ".", Styles: AllStyles, Currency: "kr."}
	value, fault := F64("1.234,50 kr.", danish)
	if fault != nil || value != 1234.5 {
		t.Fatalf("got %v %v", value, fault)
	}
}

func TestCurrencyDeclaredButStyleOffIsMalformedAtTheSymbol(t *testing.T) {
	format := NumFormat{DecimalSep: ".", GroupSep: ",", Styles: AllStyles &^ CurrencySymbol, Currency: "$"}
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
			I32("5", NumFormat{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, Currency: symbol})
		}()
	}
	// Exactly 16 bytes is the ceiling, not past it.
	sixteen := NumFormat{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, Currency: "sixteen-bytes-ok"}
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
	value, fault = Exact("($1,234.50)", NumFormat{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, Currency: "$"})
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
	// corpus/, so a release bump never leaves a stale literal here. A device run (Android,
	// through .github/scripts/android_device_test.sh) has no source tree and is handed it.
	if want := os.Getenv("HYPERCAST_CRATE_VERSION"); want != "" {
		if got := NativeVersion(); got != want {
			t.Fatalf("got %q, HYPERCAST_CRATE_VERSION says %q", got, want)
		}
		return
	}
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

func TestAvailableAndLoadErrorAreConstant(t *testing.T) {
	// The core is linked in, so both are constants; this pins that they stay that way.
	if !Available() || LoadError() != nil {
		t.Fatalf("Available() = %v, LoadError() = %v; a linked core is always available", Available(), LoadError())
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

// scalarTarget pins Scalar[V] to its concrete door for one target, on a success and on a
// failure: the same value, or the same fault with the same span.
func scalarTarget[V ScalarTarget](t *testing.T, good, bad string, door func(string, NumFormat) (V, *Fault)) {
	t.Helper()
	got, gotFault := Scalar[V](good, Invariant)
	want, wantFault := door(good, Invariant)
	if gotFault != nil || wantFault != nil {
		t.Fatalf("%T %q: unexpected fault %v / %v", want, good, gotFault, wantFault)
	}
	if got != want {
		t.Fatalf("%T %q: Scalar gave %v, door gave %v", want, good, got, want)
	}
	_, gotFault = Scalar[V](bad, Invariant)
	_, wantFault = door(bad, Invariant)
	if gotFault == nil || wantFault == nil || *gotFault != *wantFault {
		t.Fatalf("%T %q: Scalar faulted %v, door faulted %v", want, bad, gotFault, wantFault)
	}
}

// ignoring adapts a door that takes no format to the shape scalarTarget compares against.
func ignoring[V any](door func(string) (V, *Fault)) func(string, NumFormat) (V, *Fault) {
	return func(text string, _ NumFormat) (V, *Fault) { return door(text) }
}

func TestScalarDispatchesToEveryDoor(t *testing.T) {
	scalarTarget(t, "enabled", "maybe", ignoring(Bool[string]))
	scalarTarget(t, "{01020304-0506-0708-090a-0b0c0d0e0f10}", "01020304-0506", ignoring(Uuid[string]))
	scalarTarget(t, "2026-01-02T15:04:05.123456789+05:00", "2026-01-02T15:04:05", ignoring(Timestamp[string]))
	scalarTarget(t, "2024-02-29", "2023-02-29", ignoring(DateOnly[string]))
	scalarTarget(t, "P1DT6H30M15.5S", "P1Y", ignoring(Span[string]))
	scalarTarget(t, "-128", "-129", I8[string])
	scalarTarget(t, "-32,768", "32768", I16[string])
	scalarTarget(t, "(2,147,483,648)", "12x4", I32[string])
	scalarTarget(t, "9223372036854775807", "9223372036854775808", I64[string])
	scalarTarget(t, "255", "256", U8[string])
	scalarTarget(t, "0xFFFF", "-1", U16[string])
	scalarTarget(t, "4,294,967,295", "4,294,967,296", U32[string])
	scalarTarget(t, "18446744073709551615", "", U64[string])
	scalarTarget(t, "1.5e3", "NaN", F32[string])
	scalarTarget(t, "50%", "1e400", F64[string])
	scalarTarget(t, "1,234.50", "1.2.3", Exact[string])
	// The format reaches the numeric doors and nothing else.
	eurozone := NumFormat{DecimalSep: ",", GroupSep: ".", Styles: AllStyles}
	if value, fault := Scalar[float64]("1.234,5", eurozone); fault != nil || value != 1234.5 {
		t.Fatalf("Scalar[float64] under a declared format: %v %v", value, fault)
	}
	if value, fault := Scalar[bool]([]byte("on"), eurozone); fault != nil || !value {
		t.Fatalf("Scalar[bool]([]byte): %v %v", value, fault)
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
	I32("42", NumFormat{DecimalSep: ".", GroupSep: ".", Styles: AllStyles})
}

// Every field past the separators declares a further spelling read beside the ASCII one,
// so the ASCII form keeps parsing under every declaration below.

func TestDeclaredSignsAreReadBesideASCII(t *testing.T) {
	estonian := NumFormat{DecimalSep: ",", GroupSep: " ", Styles: AllStyles, MinusSign: "−", Currency: "€"}
	for _, c := range []struct {
		text string
		want int32
	}{
		{"−5", -5}, {"-5", -5}, {"−€5", -5}, {"€−5", -5}, {"€ -5", -5},
	} {
		value, fault := I32(c.text, estonian)
		if fault != nil || value != c.want {
			t.Errorf("%q: got %v %v, want %d", c.text, value, fault, c.want)
		}
	}
	// Two signs, in either spelling, is Malformed at the later one — a byte span.
	if _, fault := I32("−-5", estonian); fault == nil || fault.Reason != Malformed || fault.Offset != 3 || fault.Length != 1 {
		t.Errorf("−-5: got %v", fault)
	}
	if _, fault := I32("-−5", estonian); fault == nil || fault.Reason != Malformed || fault.Offset != 1 || fault.Length != 3 {
		t.Errorf("-−5: got %v", fault)
	}
	// Undeclared, U+2212 is no sign at all.
	if _, fault := I32("−5", Invariant); fault == nil || fault.Reason != Malformed {
		t.Errorf("undeclared minus: got %v", fault)
	}
	fullwidth := NumFormat{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, PlusSign: "＋"}
	if value, fault := F64("＋2.5", fullwidth); fault != nil || value != 2.5 {
		t.Errorf("＋2.5: got %v %v", value, fault)
	}
}

func TestTrailingSignIsOptIn(t *testing.T) {
	// Not in AllStyles: a culture whose negative pattern is "n-" declares it.
	if _, fault := I32("5-", Invariant); fault == nil || fault.Reason != Malformed {
		t.Errorf("5- under AllStyles: got %v", fault)
	}
	trailing := NumFormat{DecimalSep: ".", GroupSep: ",", Styles: AllStyles | TrailingSign, Currency: "€", MinusSign: "−"}
	for _, c := range []struct {
		text string
		want int32
	}{
		{"5-", -5}, {"5 -", -5}, {"5+", 5}, {"5 €-", -5}, {"5- €", -5}, {"€5-", -5}, {"5 €−", -5}, {"-5", -5},
	} {
		value, fault := I32(c.text, trailing)
		if fault != nil || value != c.want {
			t.Errorf("%q: got %v %v, want %d", c.text, value, fault, c.want)
		}
	}
	// Never beside a leading sign, never inside parentheses.
	for _, text := range []string{"-5-", "(5-)", "-5 €-"} {
		if _, fault := I32(text, trailing); fault == nil || fault.Reason != Malformed {
			t.Errorf("%q: got %v", text, fault)
		}
	}
	if value, fault := F64("2.5%-", trailing); fault != nil || value != -0.025 {
		t.Errorf("2.5%%-: got %v %v", value, fault)
	}
	if value, fault := Exact("2.5−", trailing); fault != nil || value.String() != "-2.5" {
		t.Errorf("2.5−: got %v %v", value, fault)
	}
}

func TestCurrencySeparatorsApplyWhenTheSymbolIsPresent(t *testing.T) {
	// A culture that points and groups a currency amount the other way round from a number.
	format := NumFormat{
		DecimalSep: ",", GroupSep: ".", Styles: AllStyles, Currency: "€",
		CurrencyDecimalSep: ".", CurrencyGroupSep: ",",
	}
	for _, c := range []struct {
		text string
		want float64
	}{
		{"€1,234.50", 1234.5}, {"€ 1,234.50", 1234.5}, {"1,234.50 €", 1234.5}, {"1.234,50", 1234.5},
	} {
		value, fault := F64(c.text, format)
		if fault != nil || value != c.want {
			t.Errorf("%q: got %v %v, want %v", c.text, value, fault, c.want)
		}
	}
	// Without the symbol the number separators govern; with it, the currency ones do.
	if _, fault := F64("1,234.50", format); fault == nil || fault.Reason != Malformed || fault.Offset != 5 {
		t.Errorf("1,234.50 without the symbol: got %v", fault)
	}
	if _, fault := I32("€1.234", format); fault == nil || fault.Reason != Malformed || fault.Offset != 4 {
		t.Errorf("€1.234 with the symbol: got %v", fault)
	}
	// Under SeparatorDetect detection governs the whole token and the currency separators
	// are ignored.
	detect := format
	detect.Styles |= SeparatorDetect
	if value, fault := F64("€1.234.567,89", detect); fault != nil || value != 1234567.89 {
		t.Errorf("detect: got %v %v", value, fault)
	}
}

func TestDeclaredPercentIsReadBesideASCII(t *testing.T) {
	arabic := NumFormat{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, Percent: "٪"}
	for _, text := range []string{"50٪", "50%"} {
		if value, fault := F64(text, arabic); fault != nil || value != 0.5 {
			t.Errorf("%q: got %v %v", text, value, fault)
		}
	}
	if value, fault := Exact("(2.5)٪", arabic); fault != nil || value.String() != "-0.025" {
		t.Errorf("(2.5)٪: got %v %v", value, fault)
	}
	// The integer doors never read a percent, declared or not.
	if _, fault := I32("50٪", arabic); fault == nil || fault.Reason != Malformed {
		t.Errorf("I32 50٪: got %v", fault)
	}
}

func TestNativeDigitsAreReadBesideASCII(t *testing.T) {
	arabic := NumFormat{DecimalSep: "٫", GroupSep: "٬", Styles: AllStyles, DigitZero: '٠'}
	if value, fault := I32("١٢٣", arabic); fault != nil || value != 123 {
		t.Errorf("١٢٣: got %v %v", value, fault)
	}
	if value, fault := I32("-12٣", arabic); fault != nil || value != -123 {
		t.Errorf("-12٣: got %v %v", value, fault)
	}
	if value, fault := F64("١٬٢٣٤٫٥", arabic); fault != nil || value != 1234.5 {
		t.Errorf("١٬٢٣٤٫٥: got %v %v", value, fault)
	}
	if value, fault := F64("١٢٫٥e١", arabic); fault != nil || value != 125 {
		t.Errorf("exponent: got %v %v", value, fault)
	}
	// A radix-prefixed body stays ASCII.
	if _, fault := I32("0x١", arabic); fault == nil || fault.Reason != Malformed {
		t.Errorf("0x١: got %v", fault)
	}
	fullwidth := NumFormat{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, DigitZero: '０', Percent: "％"}
	if value, fault := F64("５０％", fullwidth); fault != nil || value != 0.5 {
		t.Errorf("５０％: got %v %v", value, fault)
	}
	// Undeclared, the digits are no digits.
	if _, fault := I32("١٢٣", Invariant); fault == nil || fault.Reason != Malformed {
		t.Errorf("undeclared digits: got %v", fault)
	}
}

func TestATwoCharacterSeparator(t *testing.T) {
	spaced := NumFormat{DecimalSep: ".", GroupSep: ", ", Styles: AllStyles}
	if value, fault := I32("1, 234", spaced); fault != nil || value != 1234 {
		t.Errorf("1, 234: got %v %v", value, fault)
	}
	if _, fault := I32("1,234", spaced); fault == nil || fault.Reason != Malformed || fault.Offset != 1 {
		t.Errorf("1,234 under \", \": got %v", fault)
	}
	swiss := NumFormat{DecimalSep: ".", GroupSep: "' ", Styles: AllStyles}
	if value, fault := F64("1' 234.5", swiss); fault != nil || value != 1234.5 {
		t.Errorf("1' 234.5: got %v %v", value, fault)
	}
}

func TestAFormatTheCoreWouldRefusePanics(t *testing.T) {
	for _, bad := range []NumFormat{
		{DecimalSep: "", GroupSep: ",", Styles: AllStyles},
		{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, CurrencyDecimalSep: ","},
		{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, MinusSign: "- "},
		{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, Percent: "%1"},
		{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, DigitZero: '0'},
		{DecimalSep: ".", GroupSep: "123456789", Styles: AllStyles},
	} {
		func() {
			defer func() {
				if recover() == nil {
					t.Errorf("%+v: expected panic", bad)
				}
			}()
			I32("5", bad)
		}()
	}
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
	// The marker is read ahead of the clock too.
	value, fault = DateTime("1/7/2026 PM 3:04", MonthDayYear)
	if fault != nil || value != want {
		t.Fatalf("marker ahead: %v, %v", value, fault)
	}
}

func TestDateTimeWithReadsTheCulturesDayPeriods(t *testing.T) {
	german := DateFormat{Order: DayMonthYear, AM: "vorm.", PM: "nachm."}
	korean := DateFormat{Order: YearMonthDay, AM: "오전", PM: "오후"}
	spanish := DateFormat{Order: MonthDayYear, AM: "a. m.", PM: "p. m."}
	afternoon := 15*time.Hour + 4*time.Minute
	for _, c := range []struct {
		text   string
		format DateFormat
		want   CivilDateTime
	}{
		{"1/7/2026 3:04 nachm.", german, CivilDateTime{Date{2026, time.July, 1}, afternoon}},
		{"1/7/2026 3:04 vorm.", german, CivilDateTime{Date{2026, time.July, 1}, 3*time.Hour + 4*time.Minute}},
		{"1/7/2026 3:04 PM", german, CivilDateTime{Date{2026, time.July, 1}, afternoon}},
		{"2026-01-07 오후 3:04", korean, CivilDateTime{Date{2026, time.January, 7}, afternoon}},
		{"2026-01-07 오후3:04", korean, CivilDateTime{Date{2026, time.January, 7}, afternoon}},
		{"2026-01-07 오전 12:00", korean, CivilDateTime{Date{2026, time.January, 7}, 0}},
		{"1/7/2026 3:04 p. m.", spanish, CivilDateTime{Date{2026, time.January, 7}, afternoon}},
	} {
		value, fault := DateTimeWith(c.text, c.format)
		if fault != nil || value != c.want {
			t.Errorf("%q: got %v %v, want %v", c.text, value, fault, c.want)
		}
	}
	// A declared designator matches byte for byte; the English pair stays case-insensitive.
	if _, fault := DateTimeWith("1/7/2026 3:04 NACHM.", german); fault == nil || fault.Reason != Malformed {
		t.Errorf("NACHM.: got %v", fault)
	}
	if value, fault := DateTimeWith("1/7/2026 3:04 pm", german); fault != nil || value.TimeOfDay != afternoon {
		t.Errorf("pm: got %v %v", value, fault)
	}
	// DateTime reads English only: a culture's word is Malformed there, and DateTimeWith
	// with nothing declared is DateTime.
	if _, fault := DateTime("1/7/2026 3:04 nachm.", DayMonthYear); fault == nil || fault.Reason != Malformed {
		t.Errorf("DateTime nachm.: got %v", fault)
	}
	if _, fault := DateTimeWith("1/7/2026 3:04 nachm.", DateFormat{Order: DayMonthYear}); fault == nil || fault.Reason != Malformed {
		t.Errorf("DateTimeWith nothing declared, nachm.: got %v", fault)
	}
	// []byte crosses the same way.
	if value, fault := DateTimeWith([]byte("2026-01-07 오후 3:04"), korean); fault != nil || value.TimeOfDay != afternoon {
		t.Errorf("[]byte: got %v %v", value, fault)
	}
}

func TestADateFormatTheCoreWouldRefusePanics(t *testing.T) {
	for _, bad := range []DateFormat{
		{Order: DayMonthYear, AM: " vorm.", PM: "nachm."},
		{Order: DayMonthYear, AM: "vorm.", PM: "nachm1"},
		{Order: DayMonthYear, AM: "vorm.", PM: "seventeen-bytes!!"},
		{Order: 0, AM: "vorm.", PM: "nachm."},
	} {
		func() {
			defer func() {
				if recover() == nil {
					t.Errorf("%+v: expected panic", bad)
				}
			}()
			DateTimeWith("1/7/2026 3:04 nachm.", bad)
		}()
	}
}
