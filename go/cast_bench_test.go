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
