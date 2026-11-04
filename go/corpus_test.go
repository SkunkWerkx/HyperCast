package hypercast

// Replays the shared conformance corpus (corpus/*.json at the repository root) — the same
// files every other binding's suite replays. Fault spans are byte offsets into the UTF-8
// input, which is exactly what these doors receive, so span assertions hold verbatim.

import (
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"math/big"
	"os"
	"path/filepath"
	"strconv"
	"strings"
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
	Epoch     uint32          `json:"epoch"`
	Order     uint32          `json:"order"`
	NanosDay  uint64          `json:"nanos_of_day"`
	Seconds   *int64          `json:"seconds"`
	Nanos     int64           `json:"nanos"`
	Year      *int            `json:"year"`
	Month     int             `json:"month"`
	Day       int             `json:"day"`
	Magnitude string          `json:"magnitude"`
	Scale     uint8           `json:"scale"`
	Negative  bool            `json:"negative"`
	Door      string          `json:"door"`
	Bits      string          `json:"bits"`
	// A datetime vector's day-period designators, declared together or not at all.
	AM string `json:"am"`
	PM string `json:"pm"`
	// A formats.json row: its kind, and per kind the code point and its answer, or the
	// refusal's winning rule.
	Kind       string `json:"kind"`
	CodePoint  string `json:"code_point"`
	WhiteSpace *bool  `json:"white_space"`
	Rule       string `json:"rule"`
}

// A vector's declared format. Every key past the separators and flags is absent when the
// vector declares nothing for it, which is the empty string here — NumFormat's own "none".
type formatSpec struct {
	DecimalSep         string `json:"decimal_sep"`
	GroupSep           string `json:"group_sep"`
	Flags              uint32 `json:"flags"`
	Currency           string `json:"currency"`
	CurrencyDecimalSep string `json:"currency_decimal_sep"`
	CurrencyGroupSep   string `json:"currency_group_sep"`
	MinusSign          string `json:"minus_sign"`
	PlusSign           string `json:"plus_sign"`
	Percent            string `json:"percent"`
	DigitZero          string `json:"digit_zero"` // a one-character string
}

// corpus decodes corpus/<name> as the shape most files share.
func corpus(t *testing.T, name string) []vector {
	t.Helper()
	path, data := corpusFile(t, name)
	return decodeVectors(t, path, data)
}

// corpusFile reads corpus/<name>: from the directory HYPERCAST_CORPUS names when it is set (a
// WASI runtime sees only what is mounted, and the wasm job mounts the checkout and points
// here), else by walking up from the test directory to find it.
func corpusFile(t *testing.T, name string) (string, []byte) {
	t.Helper()
	if dir := os.Getenv("HYPERCAST_CORPUS"); dir != "" {
		path := filepath.Join(dir, name)
		data, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("corpus/%s not found at %s: %v", name, path, err)
		}
		return path, data
	}
	dir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for {
		// testdata/corpus is the copy CI stages for the iOS simulator: Go's go_ios_exec
		// carries the module's files and testdata directories into the app, and nothing
		// above the module, so the repository's corpus/ is not there to walk up to.
		for _, candidate := range []string{
			filepath.Join(dir, "corpus", name),
			filepath.Join(dir, "testdata", "corpus", name),
		} {
			if data, err := os.ReadFile(candidate); err == nil {
				return candidate, data
			}
		}
		// The root is its own parent on every OS — "/" here, "C:\" on Windows, where a
		// comparison against "/" never becomes true and the walk would never end.
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Fatalf("corpus/%s not found above the test directory", name)
			return "", nil
		}
		dir = parent
	}
}

func decodeVectors(t *testing.T, path string, data []byte) []vector {
	t.Helper()
	var vectors []vector
	if err := json.Unmarshal(data, &vectors); err != nil {
		t.Fatalf("parsing %s: %v", path, err)
	}
	if len(vectors) == 0 {
		t.Fatalf("%s is empty", path)
	}
	return vectors
}

func (v *vector) format() NumFormat {
	if v.Format == nil {
		return Invariant
	}
	var zero rune
	if v.Format.DigitZero != "" {
		zero = []rune(v.Format.DigitZero)[0]
	}
	return NumFormat{
		DecimalSep:         v.Format.DecimalSep,
		GroupSep:           v.Format.GroupSep,
		Styles:             NumStyles(v.Format.Flags),
		Currency:           v.Format.Currency,
		CurrencyDecimalSep: v.Format.CurrencyDecimalSep,
		CurrencyGroupSep:   v.Format.CurrencyGroupSep,
		MinusSign:          v.Format.MinusSign,
		PlusSign:           v.Format.PlusSign,
		Percent:            v.Format.Percent,
		DigitZero:          zero,
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

func TestCorpusBoolean(t *testing.T) {
	for _, v := range corpus(t, "boolean.json") {
		var expected bool
		if v.Value != nil {
			expected = string(v.Value) == "true"
		}
		value, fault := Bool(v.Input)
		assertVerdict(t, "boolean", &v, value, fault, expected)
	}
}

// A char vector's value is the code point as an integer.
func TestCorpusChar(t *testing.T) {
	for _, v := range corpus(t, "char.json") {
		var expected rune
		if v.Value != nil {
			n, err := strconv.ParseInt(string(v.Value), 10, 32)
			if err != nil {
				t.Fatalf("char: %q value %s: %v", v.Input, v.Value, err)
			}
			expected = rune(n)
		}
		value, fault := Char(v.Input)
		assertVerdict(t, "char", &v, value, fault, expected)
		bytesValue, bytesFault := Char([]byte(v.Input))
		assertVerdict(t, "char", &v, bytesValue, bytesFault, expected)
	}
}

func TestCorpusInteger(t *testing.T) {
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

// A real vector's value is compared by its IEEE 754 bits.
func TestCorpusReal(t *testing.T) {
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
		// By bits: zero has one form, and -0.0 == 0.0 would let a signed zero through.
		switch v.Type {
		case "f32":
			value, fault := F32(v.Input, format)
			assertVerdict(t, "real", &v, math.Float32bits(value), fault, math.Float32bits(float32(expected)))
		case "f64":
			value, fault := F64(v.Input, format)
			assertVerdict(t, "real", &v, math.Float64bits(value), fault, math.Float64bits(expected))
		default:
			t.Fatalf("real: unknown type %q", v.Type)
		}
	}
}

func TestCorpusDecimal(t *testing.T) {
	for _, v := range corpus(t, "decimal.json") {
		var expected Decimal
		var text string
		if v.Expect == "ok" {
			magnitude, ok := new(big.Int).SetString(v.Magnitude, 10)
			if !ok || magnitude.Sign() < 0 || magnitude.BitLen() > 96 {
				t.Fatalf("decimal: %q bad expected magnitude %q", v.Input, v.Magnitude)
			}
			lo := new(big.Int).And(magnitude, new(big.Int).SetUint64(math.MaxUint64))
			hi := new(big.Int).Rsh(magnitude, 64)
			expected = Decimal{Lo: lo.Uint64(), Hi: uint32(hi.Uint64()), Scale: v.Scale, Negative: v.Negative}
			if err := json.Unmarshal(v.Value, &text); err != nil {
				t.Fatalf("decimal: %q bad expected value: %v", v.Input, err)
			}
		}
		value, fault := Exact(v.Input, v.format())
		assertVerdict(t, "decimal", &v, value, fault, expected)
		// The canonical text form is pinned alongside the triple: String() must agree.
		if fault == nil && value.String() != text {
			t.Errorf("decimal: %q String() = %q, want %q", v.Input, value.String(), text)
		}
	}
}

func TestCorpusUuid(t *testing.T) {
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

func TestCorpusTimestamp(t *testing.T) {
	for _, v := range corpus(t, "timestamp.json") {
		value, fault := Timestamp(v.Input)
		assertVerdict(t, "timestamp", &v, value, fault, expectedInstant(&v))
	}
}

func TestCorpusUnix(t *testing.T) {
	for _, v := range corpus(t, "unix.json") {
		value, fault := Unix(v.Input, UnixPrecision(v.Precision))
		assertVerdict(t, "unix", &v, value, fault, expectedInstant(&v))
	}
}

func TestCorpusExcelSerial(t *testing.T) {
	for _, v := range corpus(t, "excel_serial.json") {
		value, fault := ExcelSerial(v.Input, ExcelEpoch(v.Epoch))
		assertVerdict(t, "excel_serial", &v, value, fault, expectedInstant(&v))
	}
}

func TestCorpusDate(t *testing.T) {
	for _, v := range corpus(t, "date.json") {
		var expected Date
		if v.Year != nil {
			expected = Date{Year: *v.Year, Month: time.Month(v.Month), Day: v.Day}
		}
		value, fault := DateOnly(v.Input)
		assertVerdict(t, "date", &v, value, fault, expected)
	}
}

// A vector declaring designators replays through DateTimeWith; one declaring none through
// DateTime, and through DateTimeWith with nothing declared, which must agree with it.
func TestCorpusDateTime(t *testing.T) {
	declared := 0
	for _, v := range corpus(t, "datetime.json") {
		var expected CivilDateTime
		if v.Year != nil {
			expected = CivilDateTime{
				Date:      Date{Year: *v.Year, Month: time.Month(v.Month), Day: v.Day},
				TimeOfDay: time.Duration(v.NanosDay),
			}
		}
		format := DateFormat{Order: DateOrder(v.Order), AM: v.AM, PM: v.PM}
		if v.AM != "" || v.PM != "" {
			declared++
			value, fault := DateTimeWith(v.Input, format)
			assertVerdict(t, "datetime (declared periods)", &v, value, fault, expected)
			continue
		}
		value, fault := DateTime(v.Input, DateOrder(v.Order))
		assertVerdict(t, "datetime", &v, value, fault, expected)
		withValue, withFault := DateTimeWith(v.Input, format)
		if withValue != value || (withFault == nil) != (fault == nil) || (fault != nil && *withFault != *fault) {
			t.Errorf("datetime: %q DateTimeWith gave %v %v, DateTime gave %v %v", v.Input, withValue, withFault, value, fault)
		}
	}
	if declared == 0 {
		t.Error("datetime: no vector declared day periods")
	}
}

func TestCorpusDateOrder(t *testing.T) {
	for _, v := range corpus(t, "date_order.json") {
		var expected Date
		if v.Year != nil {
			expected = Date{Year: *v.Year, Month: time.Month(v.Month), Day: v.Day}
		}
		value, fault := DateOnlyOrdered(v.Input, DateOrder(v.Order))
		assertVerdict(t, "date_order", &v, value, fault, expected)
	}
}

func TestCorpusTime(t *testing.T) {
	for _, v := range corpus(t, "time.json") {
		var expected time.Duration
		if v.Expect == "ok" {
			expected = time.Duration(v.Nanos)
		}
		value, fault := TimeOfDay(v.Input)
		assertVerdict(t, "time", &v, value, fault, expected)
	}
}

func TestCorpusDuration(t *testing.T) {
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

// The typed doors, by bits: each float64 is rebuilt exactly from its IEEE 754 pattern — NaN
// and the infinities included, which JSON cannot spell. A typed door's Fault has no span.
func TestCorpusTyped(t *testing.T) {
	doors := map[string]bool{}
	for _, v := range corpus(t, "typed.json") {
		bits, err := strconv.ParseUint(v.Bits, 16, 64)
		if err != nil {
			t.Fatalf("typed: %q bad bits %q", v.Input, v.Bits)
		}
		value := math.Float64frombits(bits)
		doors[v.Door] = true
		var fault *Fault
		switch v.Door {
		case "decimal":
			var expected Decimal
			var text string
			if v.Expect == "ok" {
				magnitude, _ := new(big.Int).SetString(v.Magnitude, 10)
				lo := new(big.Int).And(magnitude, new(big.Int).SetUint64(math.MaxUint64))
				hi := new(big.Int).Rsh(magnitude, 64)
				expected = Decimal{Lo: lo.Uint64(), Hi: uint32(hi.Uint64()), Scale: v.Scale, Negative: v.Negative}
				if err := json.Unmarshal(v.Value, &text); err != nil {
					t.Fatalf("typed decimal: %q bad expected value: %v", v.Input, err)
				}
			}
			var got Decimal
			got, fault = ExactFromFloat64(value)
			assertVerdict(t, "typed decimal", &v, got, fault, expected)
			if fault == nil && got.String() != text {
				t.Errorf("typed decimal: %q String() = %q, want %q", v.Input, got.String(), text)
			}
		case "excel_serial":
			var expected CivilDateTime
			if v.Year != nil {
				expected = CivilDateTime{
					Date:      Date{Year: *v.Year, Month: time.Month(v.Month), Day: v.Day},
					TimeOfDay: time.Duration(v.NanosDay),
				}
			}
			var got CivilDateTime
			got, fault = ExcelSerialFromFloat64(value, ExcelEpoch(v.Epoch))
			assertVerdict(t, "typed excel_serial", &v, got, fault, expected)
		case "excel_time":
			var got time.Duration
			got, fault = ExcelTime(value)
			assertVerdict(t, "typed excel_time", &v, got, fault, time.Duration(v.Nanos))
		case "excel_duration":
			var expected Duration
			if v.Seconds != nil {
				expected = Duration{Seconds: *v.Seconds, Nanos: int32(v.Nanos)}
			}
			var got Duration
			got, fault = ExcelDuration(value)
			assertVerdict(t, "typed excel_duration", &v, got, fault, expected)
		default:
			t.Fatalf("typed: unknown door %q", v.Door)
		}
		if fault != nil && (fault.Offset != 0 || fault.Length != 0) {
			t.Errorf("typed %s: %q fault span %d+%d, want 0+0", v.Door, v.Input, fault.Offset, fault.Length)
		}
	}
	if len(doors) != 4 {
		t.Errorf("typed: replayed %d doors, want 4", len(doors))
	}
}

// A constraints.json vector: a declaration at a door, then the outcome of judging one input
// (text, raw bytes or an already-decoded float64) by it, or the core's refusal of the
// declaration itself.
type constraintsVector struct {
	Door        string          `json:"door"`
	Input       *string         `json:"input"`
	InputBytes  []int           `json:"input_bytes"`
	FromF64     json.RawMessage `json:"from_f64"`
	Format      *formatSpec     `json:"format"`
	Constraints struct {
		Minimum          *string  `json:"minimum,omitempty"`
		ExclusiveMinimum *string  `json:"exclusiveMinimum,omitempty"`
		Maximum          *string  `json:"maximum,omitempty"`
		ExclusiveMaximum *string  `json:"exclusiveMaximum,omitempty"`
		MultipleOf       *string  `json:"multipleOf,omitempty"`
		Rounding         string   `json:"rounding,omitempty"`
		MinLength        *int     `json:"minLength,omitempty"`
		MaxLength        *int     `json:"maxLength,omitempty"`
		Trim             bool     `json:"trim,omitempty"`
		Enum             []string `json:"enum,omitempty"`
	} `json:"constraints"`
	Expect    string          `json:"expect"`
	Value     json.RawMessage `json:"value"`
	Adjusted  bool            `json:"adjusted"`
	EnumIndex *int            `json:"enum_index"`
	Fault     []int           `json:"fault"`
	Keyword   string          `json:"keyword"`
	Refusal   int32           `json:"refusal"`
}

// The corpus spells each rounding mode by name; this is the binding's own constant for it.
var corpusRounding = map[string]Rounding{"strict": Strict, "midpoint": Midpoint, "floor": Floor, "ceiling": Ceiling}

// rules is the vector's declaration as the public Rule constructors build it, so the corpus
// exercises the binding's own conversion path.
func (v *constraintsVector) rules(t *testing.T) []Rule {
	t.Helper()
	c := &v.Constraints
	var rules []Rule
	for _, b := range []struct {
		text *string
		rule func(string) Rule
	}{
		{c.Minimum, Minimum[string]}, {c.ExclusiveMinimum, ExclusiveMinimum[string]},
		{c.Maximum, Maximum[string]}, {c.ExclusiveMaximum, ExclusiveMaximum[string]},
		{c.MultipleOf, MultipleOf[string]},
	} {
		if b.text != nil {
			rules = append(rules, b.rule(*b.text))
		}
	}
	if c.Rounding != "" {
		mode, ok := corpusRounding[c.Rounding]
		if !ok {
			t.Fatalf("constraints: unknown rounding %q", c.Rounding)
		}
		rules = append(rules, WithRounding(mode))
	}
	if c.MinLength != nil {
		rules = append(rules, MinLength(*c.MinLength))
	}
	if c.MaxLength != nil {
		rules = append(rules, MaxLength(*c.MaxLength))
	}
	if c.Trim {
		rules = append(rules, Trim())
	}
	if c.Enum != nil {
		rules = append(rules, Enum(c.Enum...))
	}
	return rules
}

func (v *constraintsVector) String() string {
	declared, _ := json.Marshal(v.Constraints)
	switch {
	case v.Input != nil:
		return fmt.Sprintf("%s %q %s", v.Door, *v.Input, declared)
	case v.InputBytes != nil:
		return fmt.Sprintf("%s bytes %v %s", v.Door, v.InputBytes, declared)
	case v.FromF64 != nil:
		return fmt.Sprintf("%s from_f64 %s %s", v.Door, v.FromF64, declared)
	default:
		return fmt.Sprintf("%s declaration %s", v.Door, declared)
	}
}

// fromF64 is the vector's already-decoded number: a JSON number, or the strings JSON cannot
// spell as one ("NaN", "Infinity", "-Infinity", "-0").
func (v *constraintsVector) fromF64(t *testing.T) float64 {
	t.Helper()
	var text string
	if json.Unmarshal(v.FromF64, &text) == nil {
		switch text {
		case "NaN":
			return math.NaN()
		case "Infinity":
			return math.Inf(1)
		case "-Infinity":
			return math.Inf(-1)
		case "-0":
			return math.Copysign(0, -1)
		}
		t.Fatalf("constraints: %v unknown from_f64 %q", v, text)
	}
	f, err := strconv.ParseFloat(string(v.FromF64), 64)
	if err != nil {
		t.Fatalf("constraints: %v bad from_f64: %v", v, err)
	}
	return f
}

// assertRefused checks a refused declaration: the error carries the core's code and the
// shared message, and MustConstraints panics with the same text.
func assertRefused[V CheckedTarget](t *testing.T, v *constraintsVector) {
	t.Helper()
	rules := v.rules(t)
	_, err := NewConstraints[V](rules...)
	var r *refusal
	if !errors.As(err, &r) {
		t.Errorf("constraints: %v declared, want refusal %d", v, v.Refusal)
		return
	}
	if r.code != v.Refusal {
		t.Errorf("constraints: %v refused with code %d, want %d", v, r.code, v.Refusal)
	}
	want := fmt.Sprintf("hypercast: Constraints refused (code %d): ", v.Refusal)
	if !strings.HasPrefix(err.Error(), want) || err.Error() != refused(v.Refusal).Error() {
		t.Errorf("constraints: %v refused as %q, want the code %d message", v, err, v.Refusal)
	}
	func() {
		defer func() {
			if p := recover(); p != err.Error() {
				t.Errorf("constraints: %v MustConstraints panicked with %v, want %q", v, p, err)
			}
		}()
		MustConstraints[V](rules...)
	}()
}

// assertChecked checks one checked verdict against the vector; same reports how a delivered
// value differs from the expected one ("" when it does not).
func assertChecked[V any](t *testing.T, v *constraintsVector, how string, got Checked[V], rejection Rejection, same func(V) string) {
	t.Helper()
	switch v.Expect {
	case "ok":
		if rejection != nil {
			t.Errorf("constraints: %v (%s) rejected: %v", v, how, rejection)
			return
		}
		if diff := same(got.Value); diff != "" {
			t.Errorf("constraints: %v (%s) %s", v, how, diff)
		}
		if got.Adjusted != v.Adjusted {
			t.Errorf("constraints: %v (%s) adjusted %v, want %v", v, how, got.Adjusted, v.Adjusted)
		}
		want := -1
		if v.EnumIndex != nil {
			want = *v.EnumIndex
		}
		if got.EnumIndex != want {
			t.Errorf("constraints: %v (%s) enum index %d, want %d", v, how, got.EnumIndex, want)
		}
		return
	case "violation":
		violation, ok := rejection.(*Violation)
		if !ok {
			t.Errorf("constraints: %v (%s) -> %v %v, want a %s violation", v, how, got, rejection, v.Keyword)
			return
		}
		if violation.Keyword.String() != v.Keyword {
			t.Errorf("constraints: %v (%s) broke %s, want %s", v, how, violation.Keyword, v.Keyword)
		}
		if violation.Offset != v.Fault[0] || violation.Length != v.Fault[1] {
			t.Errorf("constraints: %v (%s) violation span (%d,%d), want (%d,%d)",
				v, how, violation.Offset, violation.Length, v.Fault[0], v.Fault[1])
		}
	default:
		reason, known := expectedReason[v.Expect]
		if !known {
			t.Fatalf("constraints: %v unknown expect %q", v, v.Expect)
		}
		fault, ok := rejection.(*Fault)
		if !ok {
			t.Errorf("constraints: %v (%s) -> %v %v, want %s", v, how, got, rejection, reason)
			return
		}
		if fault.Reason != reason || fault.Offset != v.Fault[0] || fault.Length != v.Fault[1] {
			t.Errorf("constraints: %v (%s) -> %v, want %s at (%d,%d)", v, how, fault, reason, v.Fault[0], v.Fault[1])
		}
	}
	if got.EnumIndex != -1 {
		t.Errorf("constraints: %v (%s) a rejection's Checked carries enum index %d, want -1", v, how, got.EnumIndex)
	}
}

// replayNumber replays a numeric vector at V's door: declared through NewConstraints, then
// judged through Check (over the text as a string and as bytes) or CheckFromFloat64.
func replayNumber[V Number](t *testing.T, v *constraintsVector, same func(V) string) {
	t.Helper()
	if v.Expect == "refused" {
		assertRefused[V](t, v)
		return
	}
	constraints, err := NewConstraints[V](v.rules(t)...)
	if err != nil {
		t.Errorf("constraints: %v refused: %v", v, err)
		return
	}
	format := (&vector{Format: v.Format}).format()
	switch {
	case v.FromF64 != nil:
		got, rejection := CheckFromFloat64(v.fromF64(t), constraints)
		assertChecked(t, v, "from float64", got, rejection, same)
	case v.Input != nil:
		got, rejection := Check(*v.Input, format, constraints)
		assertChecked(t, v, "string", got, rejection, same)
		got, rejection = Check([]byte(*v.Input), format, constraints)
		assertChecked(t, v, "bytes", got, rejection, same)
	case v.InputBytes != nil:
		got, rejection := Check(v.bytes(), format, constraints)
		assertChecked(t, v, "bytes", got, rejection, same)
	default:
		t.Fatalf("constraints: %v has no input", v)
	}
}

func (v *constraintsVector) bytes() []byte {
	b := make([]byte, len(v.InputBytes))
	for i, x := range v.InputBytes {
		b[i] = byte(x)
	}
	return b
}

// replayText replays a text vector through CheckString: a Go string can hold any bytes, so
// an input_bytes vector replays as a string as well as as bytes.
func replayText(t *testing.T, v *constraintsVector) {
	t.Helper()
	if v.Expect == "refused" {
		assertRefused[string](t, v)
		return
	}
	constraints, err := NewConstraints[string](v.rules(t)...)
	if err != nil {
		t.Errorf("constraints: %v refused: %v", v, err)
		return
	}
	var want string
	if v.Expect == "ok" {
		if err := json.Unmarshal(v.Value, &want); err != nil {
			t.Fatalf("constraints: %v bad value: %v", v, err)
		}
	}
	same := func(got string) string {
		if got != want {
			return fmt.Sprintf("-> %q, want %q", got, want)
		}
		return ""
	}
	text := v.bytes()
	if v.Input != nil {
		text = []byte(*v.Input)
	}
	got, rejection := CheckString(string(text), constraints)
	assertChecked(t, v, "string", got, rejection, same)
	got, rejection = CheckString(text, constraints)
	assertChecked(t, v, "bytes", got, rejection, same)
}

// exactly compares an integer door's value with the vector's.
func exactly[V int8 | int16 | int32 | int64 | uint8 | uint16 | uint32 | uint64](t *testing.T, v *constraintsVector) func(V) string {
	return func(got V) string {
		if want := string(v.Value); fmt.Sprint(got) != want {
			return fmt.Sprintf("-> %v, want %s", got, want)
		}
		return ""
	}
}

// The checked doors over corpus/constraints.json: every row, refusals and from_f64 rows
// included, through the public NewConstraints and the checked doors. Floats are compared by
// bits, so a -0.0 delivered for zero fails. A lone surrogate does not exist in Go's string
// model (a string is bytes, and the surrogate's encoding is just invalid UTF-8, which the
// input_bytes rows already replay), so there is nothing further to skip.
func TestCorpusConstraints(t *testing.T) {
	path, data := corpusFile(t, "constraints.json")
	var vectors []constraintsVector
	if err := json.Unmarshal(data, &vectors); err != nil {
		t.Fatalf("parsing %s: %v", path, err)
	}
	replayed := map[string]int{}
	for i := range vectors {
		v := &vectors[i]
		replayed[v.Expect]++
		switch v.Door {
		case "i8":
			replayNumber(t, v, exactly[int8](t, v))
		case "i16":
			replayNumber(t, v, exactly[int16](t, v))
		case "i32":
			replayNumber(t, v, exactly[int32](t, v))
		case "i64":
			replayNumber(t, v, exactly[int64](t, v))
		case "u8":
			replayNumber(t, v, exactly[uint8](t, v))
		case "u16":
			replayNumber(t, v, exactly[uint16](t, v))
		case "u32":
			replayNumber(t, v, exactly[uint32](t, v))
		case "u64":
			replayNumber(t, v, exactly[uint64](t, v))
		case "f32":
			replayNumber(t, v, func(got float32) string {
				want, err := strconv.ParseFloat(string(v.Value), 32)
				if err != nil {
					t.Fatalf("constraints: %v bad value: %v", v, err)
				}
				if math.Float32bits(got) != math.Float32bits(float32(want)) {
					return fmt.Sprintf("-> %v (bits %08x), want %v (bits %08x)", got, math.Float32bits(got), float32(want), math.Float32bits(float32(want)))
				}
				return ""
			})
		case "f64":
			replayNumber(t, v, func(got float64) string {
				want, err := strconv.ParseFloat(string(v.Value), 64)
				if err != nil {
					t.Fatalf("constraints: %v bad value: %v", v, err)
				}
				if math.Float64bits(got) != math.Float64bits(want) {
					return fmt.Sprintf("-> %v (bits %016x), want %v (bits %016x)", got, math.Float64bits(got), want, math.Float64bits(want))
				}
				return ""
			})
		case "decimal":
			replayNumber(t, v, func(got Decimal) string {
				var want string
				if err := json.Unmarshal(v.Value, &want); err != nil {
					t.Fatalf("constraints: %v bad value: %v", v, err)
				}
				if got.String() != want {
					return fmt.Sprintf("-> %s, want %s", got, want)
				}
				return ""
			})
		case "text":
			replayText(t, v)
		default:
			t.Fatalf("constraints: unknown door %q", v.Door)
		}
	}
	for _, expect := range []string{"ok", "violation", "refused", "empty", "malformed", "out_of_range"} {
		if replayed[expect] == 0 {
			t.Errorf("constraints: no %q vector replayed", expect)
		}
	}
}

// A declarations.json vector: a bound's text (or raw bytes) and the exact Decimal the core's
// conversion makes of it, or its refusal.
type declarationVector struct {
	Input      *string `json:"input"`
	InputBytes []int   `json:"input_bytes"`
	Expect     string  `json:"expect"`
	Magnitude  string  `json:"magnitude"`
	Scale      uint8   `json:"scale"`
	Negative   bool    `json:"negative"`
	Value      string  `json:"value"`
	Refusal    int32   `json:"refusal"`
}

// The conversion every declaration's text goes through (cast_declared_decimal), over
// corpus/declarations.json: the raw triple, scale included, through the binding's wrapper,
// and again through the public path (a Minimum read back from its Constraints). A Go string
// holds any bytes, so the input_bytes rows replay too.
func TestCorpusDeclarations(t *testing.T) {
	path, data := corpusFile(t, "declarations.json")
	var vectors []declarationVector
	if err := json.Unmarshal(data, &vectors); err != nil {
		t.Fatalf("parsing %s: %v", path, err)
	}
	if len(vectors) == 0 {
		t.Fatalf("%s is empty", path)
	}
	for _, v := range vectors {
		var text string
		if v.Input != nil {
			text = *v.Input
		} else {
			b := make([]byte, len(v.InputBytes))
			for i, x := range v.InputBytes {
				b[i] = byte(x)
			}
			text = string(b)
		}
		got, ok := declaredDecimal(text)
		constraints, err := NewConstraints[Decimal](Minimum(text))
		switch v.Expect {
		case "ok":
			magnitude, parsed := new(big.Int).SetString(v.Magnitude, 10)
			if !parsed || magnitude.BitLen() > 96 {
				t.Fatalf("declarations: %q bad magnitude %q", text, v.Magnitude)
			}
			lo := new(big.Int).And(magnitude, new(big.Int).SetUint64(math.MaxUint64))
			hi := new(big.Int).Rsh(magnitude, 64)
			want := Decimal{Lo: lo.Uint64(), Hi: uint32(hi.Uint64()), Scale: v.Scale, Negative: v.Negative}
			if !ok || got != want {
				t.Errorf("declarations: %q -> %+v %v, want %+v", text, got, ok, want)
			}
			if ok && got.String() != v.Value {
				t.Errorf("declarations: %q String() = %q, want %q", text, got.String(), v.Value)
			}
			if minimum, declared := constraints.Minimum(); err != nil || !declared || minimum != want {
				t.Errorf("declarations: %q as a Minimum -> %+v %v %v, want %+v", text, minimum, declared, err, want)
			}
		case "refused":
			if ok {
				t.Errorf("declarations: %q -> %+v, want refusal %d", text, got, v.Refusal)
			}
			var r *refusal
			if !errors.As(err, &r) || r.code != v.Refusal {
				t.Errorf("declarations: %q as a Minimum -> %v, want refusal %d", text, err, v.Refusal)
			}
		default:
			t.Fatalf("declarations: %q unknown expect %q", text, v.Expect)
		}
	}
}

// What fault() says for each rule formats.json names: the text each refused row must carry,
// which also pins which rule won where a declaration breaks two.
var ruleMessage = map[string]string{
	"length":           " exceeds ",
	"digit":            "must not contain an ASCII digit",
	"equal_separators": "separators must differ",
	"white_space":      "must not contain white space",
	"designator_edge":  "must not start or end in white space",
	"currency_edge":    "must not start or end with white space",
}

// panics runs f and reports the panic's text, "" when it did not panic.
func panics(f func()) (text string) {
	defer func() {
		if r := recover(); r != nil {
			text = fmt.Sprint(r)
		}
	}()
	f()
	return ""
}

// corpus/formats.json: the white-space table, and the declarations the core accepts and
// refuses. A refused declaration is refused by valid(), named by fault() with the rule's
// message, errs from Raw(), and panics at the door; an accepted one does none of them.
func TestCorpusFormats(t *testing.T) {
	var whiteSpace, numFormats, dayPeriods int
	for _, v := range corpus(t, "formats.json") {
		var refusal error
		var door func()
		var label string
		switch v.Kind {
		case "white_space":
			whiteSpace++
			if v.WhiteSpace == nil || !strings.HasPrefix(v.CodePoint, "U+") {
				t.Fatalf("formats: bad white_space row %+v", v)
			}
			code, err := strconv.ParseUint(v.CodePoint[2:], 16, 32)
			if err != nil {
				t.Fatalf("formats: %s: %v", v.CodePoint, err)
			}
			if got := isWhiteSpace(rune(code)); got != *v.WhiteSpace {
				t.Errorf("formats: isWhiteSpace(%s) = %v, want %v", v.CodePoint, got, *v.WhiteSpace)
			}
			continue
		case "num_format":
			numFormats++
			format := v.format()
			label = fmt.Sprintf("%+v", format)
			if (format.valid()) != (v.Expect == "ok") {
				t.Errorf("formats: valid() of %s = %v, want %s", label, format.valid(), v.Expect)
			}
			refusal = format.fault()
			if _, err := format.Raw(); (err == nil) != (v.Expect == "ok") {
				t.Errorf("formats: Raw() of %s = %v, want %s", label, err, v.Expect)
			}
			door = func() { I32("5", format) }
		case "day_periods":
			dayPeriods++
			format := DateFormat{Order: YearMonthDay, AM: v.AM, PM: v.PM}
			label = fmt.Sprintf("%+v", format)
			if format.valid() != (v.Expect == "ok") {
				t.Errorf("formats: valid() of %s = %v, want %s", label, format.valid(), v.Expect)
			}
			refusal = format.fault()
			if _, err := format.Raw(); (err == nil) != (v.Expect == "ok") {
				t.Errorf("formats: Raw() of %s = %v, want %s", label, err, v.Expect)
			}
			door = func() { DateTimeWith("2026-01-02 03:04", format) }
		default:
			t.Fatalf("formats: unknown kind %q", v.Kind)
		}
		switch v.Expect {
		case "ok":
			if refusal != nil {
				t.Errorf("formats: %s: fault() = %v, want none", label, refusal)
			}
			if text := panics(door); text != "" {
				t.Errorf("formats: %s: the door panicked: %s", label, text)
			}
		case "refused":
			want, known := ruleMessage[v.Rule]
			if !known {
				t.Fatalf("formats: unknown rule %q", v.Rule)
			}
			if refusal == nil || !strings.Contains(refusal.Error(), want) {
				t.Errorf("formats: %s: fault() = %v, want the %s rule (%q)", label, refusal, v.Rule, want)
			}
			if text := panics(door); text == "" || !strings.Contains(text, want) {
				t.Errorf("formats: %s: the door panicked with %q, want the %s rule (%q)", label, text, v.Rule, want)
			}
		default:
			t.Fatalf("formats: unknown expect %q", v.Expect)
		}
	}
	if whiteSpace < 30 || numFormats == 0 || dayPeriods == 0 {
		t.Errorf("formats: replayed %d white_space, %d num_format and %d day_periods rows", whiteSpace, numFormats, dayPeriods)
	}
}
