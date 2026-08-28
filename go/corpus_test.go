package hypercast

// Replays the shared conformance corpus (corpus/*.json at the repository root) — the same
// files every other binding's suite replays. Fault spans are byte offsets into the UTF-8
// input, which is exactly what these doors receive, so span assertions hold verbatim.

import (
	"encoding/json"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"testing"
	"time"

	"github.com/google/uuid"
)

type vector struct {
	Input     string          `json:"input"`
	Expect    string          `json:"expect"`
	Type      string          `json:"type"`
	Value     json.RawMessage `json:"value"`
	Format    *formatSpec     `json:"format"`
	FaultSpan []int           `json:"fault"`
	Precision uint32          `json:"precision"`
	Seconds   *int64          `json:"seconds"`
	Nanos     int64           `json:"nanos"`
	Year      *int            `json:"year"`
	Month     int             `json:"month"`
	Day       int             `json:"day"`
}

type formatSpec struct {
	DecimalSep string `json:"decimal_sep"`
	GroupSep   string `json:"group_sep"`
	Flags      uint32 `json:"flags"`
}

func corpus(t *testing.T, name string) []vector {
	t.Helper()
	dir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for ; dir != "/"; dir = filepath.Dir(dir) {
		candidate := filepath.Join(dir, "corpus", name)
		if data, err := os.ReadFile(candidate); err == nil {
			var vectors []vector
			if err := json.Unmarshal(data, &vectors); err != nil {
				t.Fatalf("parsing %s: %v", candidate, err)
			}
			return vectors
		}
	}
	t.Fatal("corpus directory not found")
	return nil
}

func (v *vector) format() NumFormat {
	if v.Format == nil {
		return Invariant
	}
	return NumFormat{
		DecimalSep: []rune(v.Format.DecimalSep)[0],
		GroupSep:   []rune(v.Format.GroupSep)[0],
		Styles:     NumStyles(v.Format.Flags),
	}
}

var expectedReason = map[string]CastFailure{
	"empty":        Empty,
	"malformed":    Malformed,
	"out_of_range": OutOfRange,
}

// assertVerdict checks Go's union idiom the way callers consume it: nil fault is the
// success case, a non-nil fault carries the reason and span.
func assertVerdict[V comparable](t *testing.T, domain string, v *vector, value V, fault *Fault, expected V) {
	t.Helper()
	if fault == nil {
		if v.Expect != "ok" {
			t.Errorf("%s: %q unexpectedly parsed to %v", domain, v.Input, value)
			return
		}
		if value != expected {
			t.Errorf("%s: %q -> %v, want %v", domain, v.Input, value, expected)
		}
		return
	}
	reason, known := expectedReason[v.Expect]
	if !known {
		t.Errorf("%s: %q expected %q but faulted: %v", domain, v.Input, v.Expect, fault)
		return
	}
	if fault.Reason != reason {
		t.Errorf("%s: %q -> %v, want %v", domain, v.Input, fault.Reason, reason)
	}
	if v.FaultSpan != nil && (fault.Offset != v.FaultSpan[0] || fault.Length != v.FaultSpan[1]) {
		t.Errorf("%s: %q fault span (%d,%d), want (%d,%d)",
			domain, v.Input, fault.Offset, fault.Length, v.FaultSpan[0], v.FaultSpan[1])
	}
}

func TestBooleanCorpus(t *testing.T) {
	for _, v := range corpus(t, "boolean.json") {
		var expected bool
		if v.Value != nil {
			expected = string(v.Value) == "true"
		}
		value, fault := Bool(v.Input)
		assertVerdict(t, "boolean", &v, value, fault, expected)
	}
}

func TestIntegerCorpus(t *testing.T) {
	for _, v := range corpus(t, "integer.json") {
		format := v.format()
		switch v.Type {
		case "i8":
			value, fault := I8(v.Input, format)
			assertVerdict(t, "integer", &v, value, fault, int8(intValue(t, &v)))
		case "i16":
			value, fault := I16(v.Input, format)
			assertVerdict(t, "integer", &v, value, fault, int16(intValue(t, &v)))
		case "i32":
			value, fault := I32(v.Input, format)
			assertVerdict(t, "integer", &v, value, fault, int32(intValue(t, &v)))
		case "i64":
			value, fault := I64(v.Input, format)
			assertVerdict(t, "integer", &v, value, fault, intValue(t, &v))
		case "u8":
			value, fault := U8(v.Input, format)
			assertVerdict(t, "integer", &v, value, fault, uint8(uintValue(t, &v)))
		case "u16":
			value, fault := U16(v.Input, format)
			assertVerdict(t, "integer", &v, value, fault, uint16(uintValue(t, &v)))
		case "u32":
			value, fault := U32(v.Input, format)
			assertVerdict(t, "integer", &v, value, fault, uint32(uintValue(t, &v)))
		case "u64":
			value, fault := U64(v.Input, format)
			assertVerdict(t, "integer", &v, value, fault, uintValue(t, &v))
		default:
			t.Fatalf("integer: unknown type %q", v.Type)
		}
	}
}

func intValue(t *testing.T, v *vector) int64 {
	t.Helper()
	if v.Value == nil {
		return 0
	}
	parsed, err := strconv.ParseInt(string(v.Value), 10, 64)
	if err != nil {
		t.Fatalf("integer: %q bad expected value: %v", v.Input, err)
	}
	return parsed
}

func uintValue(t *testing.T, v *vector) uint64 {
	t.Helper()
	if v.Value == nil {
		return 0
	}
	parsed, err := strconv.ParseUint(string(v.Value), 10, 64)
	if err != nil {
		t.Fatalf("integer: %q bad expected value: %v", v.Input, err)
	}
	return parsed
}

func TestRealCorpus(t *testing.T) {
	for _, v := range corpus(t, "real.json") {
		format := v.format()
		var expected float64
		if v.Value != nil {
			parsed, err := strconv.ParseFloat(string(v.Value), 64)
			if err != nil {
				t.Fatalf("real: %q bad expected value: %v", v.Input, err)
			}
			expected = parsed
		}
		switch v.Type {
		case "f32":
			value, fault := F32(v.Input, format)
			assertVerdict(t, "real", &v, value, fault, float32(expected))
		case "f64":
			value, fault := F64(v.Input, format)
			assertVerdict(t, "real", &v, value, fault, expected)
		default:
			t.Fatalf("real: unknown type %q", v.Type)
		}
	}
}

func TestUuidCorpus(t *testing.T) {
	for _, v := range corpus(t, "uuid.json") {
		var expected uuid.UUID
		if v.Value != nil {
			var hex string
			if err := json.Unmarshal(v.Value, &hex); err != nil {
				t.Fatal(err)
			}
			parsed, err := uuid.Parse(hex)
			if err != nil {
				t.Fatalf("uuid: %q bad expected value: %v", v.Input, err)
			}
			expected = parsed
		}
		value, fault := Uuid(v.Input)
		assertVerdict(t, "uuid", &v, value, fault, expected)
	}
}

func expectedInstant(v *vector) time.Time {
	if v.Seconds == nil {
		return time.Time{}
	}
	return time.Unix(*v.Seconds, v.Nanos).UTC()
}

func TestTimestampCorpus(t *testing.T) {
	for _, v := range corpus(t, "timestamp.json") {
		value, fault := Timestamp(v.Input)
		assertVerdict(t, "timestamp", &v, value, fault, expectedInstant(&v))
	}
}

func TestUnixCorpus(t *testing.T) {
	for _, v := range corpus(t, "unix.json") {
		value, fault := Unix(v.Input, UnixPrecision(v.Precision))
		assertVerdict(t, "unix", &v, value, fault, expectedInstant(&v))
	}
}

func TestDateCorpus(t *testing.T) {
	for _, v := range corpus(t, "date.json") {
		var expected Date
		if v.Year != nil {
			expected = Date{Year: *v.Year, Month: time.Month(v.Month), Day: v.Day}
		}
		value, fault := DateOnly(v.Input)
		assertVerdict(t, "date", &v, value, fault, expected)
	}
}

func TestTimeCorpus(t *testing.T) {
	for _, v := range corpus(t, "time.json") {
		var expected time.Duration
		if v.Expect == "ok" {
			expected = time.Duration(v.Nanos)
		}
		value, fault := TimeOfDay(v.Input)
		assertVerdict(t, "time", &v, value, fault, expected)
	}
}

func TestDurationCorpus(t *testing.T) {
	for _, v := range corpus(t, "duration.json") {
		var expected Duration
		if v.Seconds != nil {
			if v.Nanos < math.MinInt32 || v.Nanos > math.MaxInt32 {
				t.Fatalf("duration: %q corpus nanos out of i32", v.Input)
			}
			expected = Duration{Seconds: *v.Seconds, Nanos: int32(v.Nanos)}
		}
		value, fault := Span(v.Input)
		assertVerdict(t, "duration", &v, value, fault, expected)
	}
}
