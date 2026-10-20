package hypercast

// Each pair pits a HyperCast door (cgo crossing included) against the closest stdlib /
// lingua-franca parse — mirrors rust/benches, csharp/HyperCast.Benchmarks, and
// java/benchmarks. Where grammars differ the label says so: strconv has no grouping knob
// (the ungrouped Cast row is the like-for-like), and time.ParseDuration speaks Go's own
// "1h30m" dialect, not ISO 8601 — printed for scale, not equivalence.
// Run: go test -bench=. -benchmem

import (
	"strconv"
	"testing"
	"time"

	"github.com/google/uuid"
)

const (
	benchBool      = "true"
	benchInt       = "1234567"
	benchIntGroup  = "1,234,567"
	benchFloat     = "12345.6789"
	benchUuid      = "01020304-0506-0708-090a-0b0c0d0e0f10"
	benchTimestamp = "2026-01-02T15:04:05.123456789Z"
	benchIsoSpan   = "PT1H30M15.5S"
	benchGoSpan    = "1h30m15.5s"
)

func BenchmarkCastBool(b *testing.B) {
	for b.Loop() {
		Bool(benchBool)
	}
}

func BenchmarkStdParseBool(b *testing.B) {
	for b.Loop() {
		_, _ = strconv.ParseBool(benchBool)
	}
}

func BenchmarkCastI32(b *testing.B) {
	for b.Loop() {
		I32(benchInt, Invariant)
	}
}

func BenchmarkCastI32Grouped(b *testing.B) {
	for b.Loop() {
		I32(benchIntGroup, Invariant)
	}
}

// A declared currency symbol the text does not carry: what it costs over Invariant.
func BenchmarkCastI32GroupedDeclaredCurrency(b *testing.B) {
	dollars := NumFormat{DecimalSep: ".", GroupSep: ",", Styles: AllStyles, Currency: "$"}
	for b.Loop() {
		I32(benchIntGroup, dollars)
	}
}

// Every symbol the layout carries declared, on text that carries none of them: what a
// fully bridged culture costs over Invariant.
func BenchmarkCastI32GroupedDeclaredEverything(b *testing.B) {
	everything := NumFormat{
		DecimalSep: ".", GroupSep: ",", Styles: AllStyles | TrailingSign, Currency: "$",
		CurrencyDecimalSep: ".", CurrencyGroupSep: ",", MinusSign: "−", PlusSign: "＋", Percent: "٪", DigitZero: '٠',
	}
	for b.Loop() {
		I32(benchIntGroup, everything)
	}
}

// The rejection path, against strconv's own failure on the same mistake.
func BenchmarkCastI32Malformed(b *testing.B) {
	for b.Loop() {
		I32("1,234x567", Invariant)
	}
}

func BenchmarkStdAtoiMalformed(b *testing.B) {
	for b.Loop() {
		_, _ = strconv.Atoi("1234x567")
	}
}

func BenchmarkStdAtoi(b *testing.B) {
	// No grouping knob exists in strconv at any price — ungrouped input only.
	for b.Loop() {
		_, _ = strconv.Atoi(benchInt)
	}
}

func BenchmarkCastF64(b *testing.B) {
	for b.Loop() {
		F64(benchFloat, Invariant)
	}
}

func BenchmarkStdParseFloat(b *testing.B) {
	for b.Loop() {
		_, _ = strconv.ParseFloat(benchFloat, 64)
	}
}

func BenchmarkCastUuid(b *testing.B) {
	for b.Loop() {
		Uuid(benchUuid)
	}
}

func BenchmarkGoogleUuidParse(b *testing.B) {
	for b.Loop() {
		_, _ = uuid.Parse(benchUuid)
	}
}

func BenchmarkCastTimestamp(b *testing.B) {
	for b.Loop() {
		Timestamp(benchTimestamp)
	}
}

func BenchmarkStdTimeParseRFC3339(b *testing.B) {
	for b.Loop() {
		_, _ = time.Parse(time.RFC3339Nano, benchTimestamp)
	}
}

func BenchmarkCastSpanIso(b *testing.B) {
	for b.Loop() {
		Span(benchIsoSpan)
	}
}

func BenchmarkStdParseDurationGoDialect(b *testing.B) {
	// Go's own duration dialect, not ISO 8601 — scale reference, not equivalence.
	for b.Loop() {
		_, _ = time.ParseDuration(benchGoSpan)
	}
}

func BenchmarkCastTimeOfDay(b *testing.B) {
	// No stdlib time-of-day parser exists to pair against.
	for b.Loop() {
		TimeOfDay("15:04:05.123456789")
	}
}

func BenchmarkCastDateTimeMessy(b *testing.B) {
	// No stdlib parser takes "1/7/2026 3:04 PM" without a layout; time.Parse with the
	// equivalent layout is the honest pairing.
	for b.Loop() {
		DateTime("1/7/2026 3:04 PM", MonthDayYear)
	}
}

// The same text under declared day periods it does not use: what the periods cost.
func BenchmarkCastDateTimeMessyDeclaredPeriods(b *testing.B) {
	german := DateFormat{Order: MonthDayYear, AM: "vorm.", PM: "nachm."}
	for b.Loop() {
		DateTimeWith("1/7/2026 3:04 PM", german)
	}
}

func BenchmarkStdTimeParseUSLayout(b *testing.B) {
	for b.Loop() {
		_, _ = time.Parse("1/2/2006 3:04 PM", "1/7/2026 3:04 PM")
	}
}

func BenchmarkCastDateOrdered(b *testing.B) {
	for b.Loop() {
		DateOnlyOrdered("1/7/2026", MonthDayYear)
	}
}

func BenchmarkStdTimeParseUSDateLayout(b *testing.B) {
	for b.Loop() {
		_, _ = time.Parse("1/2/2006", "1/7/2026")
	}
}

func BenchmarkCastF64Detect(b *testing.B) {
	for b.Loop() {
		F64("1.234.567,89", Detect)
	}
}

func BenchmarkCastF64Declared(b *testing.B) {
	declared := NumFormat{DecimalSep: ",", GroupSep: ".", Styles: AllStyles}
	for b.Loop() {
		F64("1.234.567,89", declared)
	}
}
