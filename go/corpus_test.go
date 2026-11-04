package hypercast

// Replays the shared conformance corpus (corpus/*.json at the repository root) — the same
// files every other binding's suite replays. Fault spans are byte offsets into the UTF-8
// input, which is exactly what these doors receive, so span assertions hold verbatim. Every
// row replays through its detailed door too, which must name the row's detail (by name and by
// code) on every failure; a row declaring date options replays through the detailed door
// alone, the only one that takes them.

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
	// What a failure row says failed, by the core's name.
	Detail string `json:"detail"`
	// A date door row's opt-in shapes, pivot and month vocabulary.
	Options *optionsSpec `json:"options"`
	// A formats.json date_options row: its shapes by name or as raw bits, and its pivot; a
	// month_names row: its entries, and bytes appended to their packed form.
	Shapes      []string `json:"flags"`
	FlagBits    *uint32  `json:"flag_bits"`
	Pivot       int      `json:"pivot"`
	Months      [][]any  `json:"months"`
	AppendBytes []int    `json:"append_bytes"`
}

// A row's date options; absent keys declare nothing.
type optionsSpec struct {
	Shapes []string `json:"flags"`
	Pivot  int      `json:"pivot"`
	Months [][]any  `json:"months"`
}

// The corpus spells each shape by name; this is the binding's own constant for it.
var corpusShapes = map[string]DateShapes{
	"compact_date": CompactDate, "space_separator": SpaceSeparator, "hour_offset": HourOffset, "common_forms": CommonForms,
}

func shapesOf(t *testing.T, names []string) DateShapes {
	t.Helper()
	var shapes DateShapes
	for _, name := range names {
		shape, ok := corpusShapes[name]
		if !ok {
			t.Fatalf("unknown date shape %q", name)
		}
		shapes |= shape
	}
	return shapes
}

// monthEntries is a row's [month, name] pairs as MonthName entries.
func monthEntries(t *testing.T, months [][]any) []MonthName {
	t.Helper()
	names := make([]MonthName, len(months))
	for i, pair := range months {
		month, okMonth := pair[0].(float64)
		name, okName := pair[1].(string)
		if len(pair) != 2 || !okMonth || !okName {
			t.Fatalf("bad month entry %v", pair)
		}
		names[i] = MonthName{Month: time.Month(month), Name: name}
	}
	return names
}

// options is the row's DateOptions, built through the public API; the zero DateOptions when
// the row declares none.
func (v *vector) options(t *testing.T) DateOptions {
	t.Helper()
	if v.Options == nil {
		return DateOptions{}
	}
	options := DateOptions{Shapes: shapesOf(t, v.Options.Shapes), Pivot: v.Options.Pivot}
	if v.Options.Months != nil {
		months, err := NewMonthNames(monthEntries(t, v.Options.Months)...)
		if err != nil {
			t.Fatalf("%q: the row's vocabulary was refused: %v", v.Input, err)
		}
		options.Months = months
	}
	return options
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
	// The grouping policy by name, replacing any policy bits in Flags; absent keeps them.
	Grouping string `json:"grouping"`
}

// The corpus spells each grouping policy by name; this is the binding's own constant for it.
var corpusGrouping = map[string]GroupingPolicy{"legacy": GroupingLegacy, "western": GroupingWestern, "indian": GroupingIndian}

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

// format is the row's NumFormat. A row declaring none means the core's invariant format,
// legacy grouping (the C ABI's null format), which is Invariant under GroupingLegacy; a row
// declaring no policy keeps the policy bits of its flags, none being legacy.
func (v *vector) format() NumFormat {
	if v.Format == nil {
		return Invariant.WithGrouping(GroupingLegacy)
	}
	var zero rune
	if v.Format.DigitZero != "" {
		zero = []rune(v.Format.DigitZero)[0]
	}
	format := NumFormat{
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
	if v.Format.Grouping != "" {
		policy, ok := corpusGrouping[v.Format.Grouping]
		if !ok {
			panic(fmt.Sprintf("unknown grouping %q", v.Format.Grouping))
		}
		format = format.WithGrouping(policy)
	}
	return format
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

// assertDetailed checks a detailed door's verdict as assertVerdict checks a plain one's, and
// on a failure the row's detail, by the core's name and by its code (publishedDetails restates
// the table).
func assertDetailed[V comparable](t *testing.T, domain string, v *vector, value V, failure *Failure, expected V) {
	t.Helper()
	var fault *Fault
	if failure != nil {
		fault = failure.Fault()
	}
	assertVerdict(t, domain+" (detailed)", v, value, fault, expected)
	if failure == nil {
		return
	}
	assertDetail(t, domain, v.Input, v.Detail, failure.Detail)
}

// assertDetail checks a failure's detail against the row's, which every failure row declares.
func assertDetail(t *testing.T, domain, input, want string, got Detail) {
	t.Helper()
	if want == "" {
		t.Errorf("%s: %q failed (%s) but the row declares no detail", domain, input, got)
		return
	}
	code := 0
	for i, name := range publishedDetails {
		if name == want {
			code = i + 1
		}
	}
	if code == 0 {
		t.Fatalf("%s: %q declares unpublished detail %q", domain, input, want)
	}
	if got.String() != want || uint32(got) != uint32(code) {
		t.Errorf("%s: %q detail %s (%d), want %s (%d)", domain, input, got, uint32(got), want, code)
	}
}

// replay replays a row through its plain door (skipped when the row declares date options,
// which only the detailed door takes) and its detailed door.
func replay[V comparable](t *testing.T, domain string, v *vector, expected V, plain func() (V, *Fault), detailed func() (V, *Failure)) {
	t.Helper()
	if v.Options == nil {
		value, fault := plain()
		assertVerdict(t, domain, v, value, fault, expected)
	}
	value, failure := detailed()
	assertDetailed(t, domain, v, value, failure, expected)
}

func TestCorpusBoolean(t *testing.T) {
	for _, v := range corpus(t, "boolean.json") {
		var expected bool
		if v.Value != nil {
			expected = string(v.Value) == "true"
		}
		replay(t, "boolean", &v, expected,
			func() (bool, *Fault) { return Bool(v.Input) },
			func() (bool, *Failure) { return BoolDetailed(v.Input) })
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
		replay(t, "char", &v, expected,
			func() (rune, *Fault) { return Char(v.Input) },
			func() (rune, *Failure) { return CharDetailed(v.Input) })
		replay(t, "char", &v, expected,
			func() (rune, *Fault) { return Char([]byte(v.Input)) },
			func() (rune, *Failure) { return CharDetailed([]byte(v.Input)) })
	}
}

func TestCorpusInteger(t *testing.T) {
	for _, v := range corpus(t, "integer.json") {
		format := v.format()
		switch v.Type {
		case "i8":
			replay(t, "integer", &v, int8(intValue(t, &v)),
				func() (int8, *Fault) { return I8(v.Input, format) },
				func() (int8, *Failure) { return I8Detailed(v.Input, format) })
		case "i16":
			replay(t, "integer", &v, int16(intValue(t, &v)),
				func() (int16, *Fault) { return I16(v.Input, format) },
				func() (int16, *Failure) { return I16Detailed(v.Input, format) })
		case "i32":
			replay(t, "integer", &v, int32(intValue(t, &v)),
				func() (int32, *Fault) { return I32(v.Input, format) },
				func() (int32, *Failure) { return I32Detailed(v.Input, format) })
		case "i64":
			replay(t, "integer", &v, intValue(t, &v),
				func() (int64, *Fault) { return I64(v.Input, format) },
				func() (int64, *Failure) { return I64Detailed(v.Input, format) })
		case "u8":
			replay(t, "integer", &v, uint8(uintValue(t, &v)),
				func() (uint8, *Fault) { return U8(v.Input, format) },
				func() (uint8, *Failure) { return U8Detailed(v.Input, format) })
		case "u16":
			replay(t, "integer", &v, uint16(uintValue(t, &v)),
				func() (uint16, *Fault) { return U16(v.Input, format) },
				func() (uint16, *Failure) { return U16Detailed(v.Input, format) })
		case "u32":
			replay(t, "integer", &v, uint32(uintValue(t, &v)),
				func() (uint32, *Fault) { return U32(v.Input, format) },
				func() (uint32, *Failure) { return U32Detailed(v.Input, format) })
		case "u64":
			replay(t, "integer", &v, uintValue(t, &v),
				func() (uint64, *Fault) { return U64(v.Input, format) },
				func() (uint64, *Failure) { return U64Detailed(v.Input, format) })
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
			replay(t, "real", &v, math.Float32bits(float32(expected)),
				func() (uint32, *Fault) { value, fault := F32(v.Input, format); return math.Float32bits(value), fault },
				func() (uint32, *Failure) { value, f := F32Detailed(v.Input, format); return math.Float32bits(value), f })
		case "f64":
			replay(t, "real", &v, math.Float64bits(expected),
				func() (uint64, *Fault) { value, fault := F64(v.Input, format); return math.Float64bits(value), fault },
				func() (uint64, *Failure) { value, f := F64Detailed(v.Input, format); return math.Float64bits(value), f })
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
		detailed, failure := ExactDetailed(v.Input, v.format())
		assertDetailed(t, "decimal", &v, detailed, failure, expected)
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
		replay(t, "uuid", &v, expected,
			func() (uuid.UUID, *Fault) { return Uuid(v.Input) },
			func() (uuid.UUID, *Failure) { return UuidDetailed(v.Input) })
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
		replay(t, "timestamp", &v, expectedInstant(&v),
			func() (time.Time, *Fault) { return Timestamp(v.Input) },
			func() (time.Time, *Failure) { return TimestampDetailed(v.Input, v.options(t)) })
	}
}

func TestCorpusUnix(t *testing.T) {
	for _, v := range corpus(t, "unix.json") {
		replay(t, "unix", &v, expectedInstant(&v),
			func() (time.Time, *Fault) { return Unix(v.Input, UnixPrecision(v.Precision)) },
			func() (time.Time, *Failure) { return UnixDetailed(v.Input, UnixPrecision(v.Precision)) })
	}
}

func TestCorpusExcelSerial(t *testing.T) {
	for _, v := range corpus(t, "excel_serial.json") {
		replay(t, "excel_serial", &v, expectedInstant(&v),
			func() (time.Time, *Fault) { return ExcelSerial(v.Input, ExcelEpoch(v.Epoch)) },
			func() (time.Time, *Failure) { return ExcelSerialDetailed(v.Input, ExcelEpoch(v.Epoch)) })
	}
}

func TestCorpusDate(t *testing.T) {
	for _, v := range corpus(t, "date.json") {
		var expected Date
		if v.Year != nil {
			expected = Date{Year: *v.Year, Month: time.Month(v.Month), Day: v.Day}
		}
		replay(t, "date", &v, expected,
			func() (Date, *Fault) { return DateOnly(v.Input) },
			func() (Date, *Failure) { return DateOnlyDetailed(v.Input, v.options(t)) })
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
		detailed, failure := DateTimeDetailed(v.Input, format, v.options(t))
		assertDetailed(t, "datetime", &v, detailed, failure, expected)
		if v.Options != nil {
			continue
		}
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
		replay(t, "date_order", &v, expected,
			func() (Date, *Fault) { return DateOnlyOrdered(v.Input, DateOrder(v.Order)) },
			func() (Date, *Failure) { return DateOnlyOrderedDetailed(v.Input, DateOrder(v.Order), v.options(t)) })
	}
}

func TestCorpusTime(t *testing.T) {
	for _, v := range corpus(t, "time.json") {
		var expected time.Duration
		if v.Expect == "ok" {
			expected = time.Duration(v.Nanos)
		}
		replay(t, "time", &v, expected,
			func() (time.Duration, *Fault) { return TimeOfDay(v.Input) },
			func() (time.Duration, *Failure) { return TimeOfDayDetailed(v.Input) })
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
		replay(t, "duration", &v, expected,
			func() (Duration, *Fault) { return Span(v.Input) },
			func() (Duration, *Failure) { return SpanDetailed(v.Input) })
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
			detailed, failure := ExactFromFloat64Detailed(value)
			assertDetailed(t, "typed decimal", &v, detailed, failure, expected)
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
			detailed, failure := ExcelSerialFromFloat64Detailed(value, ExcelEpoch(v.Epoch))
			assertDetailed(t, "typed excel_serial", &v, detailed, failure, expected)
		case "excel_time":
			var got time.Duration
			got, fault = ExcelTime(value)
			assertVerdict(t, "typed excel_time", &v, got, fault, time.Duration(v.Nanos))
			detailed, failure := ExcelTimeDetailed(value)
			assertDetailed(t, "typed excel_time", &v, detailed, failure, time.Duration(v.Nanos))
		case "excel_duration":
			var expected Duration
			if v.Seconds != nil {
				expected = Duration{Seconds: *v.Seconds, Nanos: int32(v.Nanos)}
			}
			var got Duration
			got, fault = ExcelDuration(value)
			assertVerdict(t, "typed excel_duration", &v, got, fault, expected)
			detailed, failure := ExcelDurationDetailed(value)
			assertDetailed(t, "typed excel_duration", &v, detailed, failure, expected)
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
	Detail    string          `json:"detail"`
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
func assertChecked[V any](t *testing.T, v *constraintsVector, how string, got Checked[V], rejection error, same func(V) string) {
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
		if failure, detailed := rejection.(*Failure); detailed {
			fault, ok = failure.Fault(), true
			assertDetail(t, "constraints ("+how+")", v.String(), v.Detail, failure.Detail)
		}
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
		assertChecked(t, v, "from float64", got, asError(rejection), same)
		got, detailed := CheckFromFloat64Detailed(v.fromF64(t), constraints)
		assertChecked(t, v, "from float64, detailed", got, asError(detailed), same)
	case v.Input != nil:
		got, rejection := Check(*v.Input, format, constraints)
		assertChecked(t, v, "string", got, asError(rejection), same)
		got, rejection = Check([]byte(*v.Input), format, constraints)
		assertChecked(t, v, "bytes", got, asError(rejection), same)
		got, detailed := CheckDetailed(*v.Input, format, constraints)
		assertChecked(t, v, "string, detailed", got, asError(detailed), same)
	case v.InputBytes != nil:
		got, rejection := Check(v.bytes(), format, constraints)
		assertChecked(t, v, "bytes", got, asError(rejection), same)
		got, detailed := CheckDetailed(v.bytes(), format, constraints)
		assertChecked(t, v, "bytes, detailed", got, asError(detailed), same)
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
	assertChecked(t, v, "string", got, asError(rejection), same)
	got, rejection = CheckString(text, constraints)
	assertChecked(t, v, "bytes", got, asError(rejection), same)
	got, detailed := CheckStringDetailed(text, constraints)
	assertChecked(t, v, "bytes, detailed", got, asError(detailed), same)
}

// asError is a checked door's rejection as a plain error, nil for none (a nil interface of
// either rejection type, never a typed nil).
func asError[R interface {
	error
	comparable
}](rejection R) error {
	var none R
	if rejection == none {
		return nil
	}
	return rejection
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
	"grouping_policy":  "names no grouping policy",
	"unknown_flag":     "does not define",
	// DateOptions.
	"flags": "does not define",
	"pivot": "is neither 0 nor 100 through 9999",
	// MonthNames.
	"layout":        "is cut short",
	"month":         "must be 1 through 12",
	"utf8":          "is not valid UTF-8",
	"name_length":   "must be 1 to 64 UTF-8 bytes",
	"character":     "must not contain an ASCII digit, '/', '-' or white space",
	"month_missing": "has no name",
	"duplicate":     "fold to the same spelling",
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
// message, errs from Raw(), and panics at the door; an accepted one does none of them. A month
// vocabulary is refused by NewMonthNames (or, for the rows that append raw bytes, by the same
// judgement over the packed form) with the rule's message, and MustMonthNames panics with it.
func TestCorpusFormats(t *testing.T) {
	var whiteSpace, numFormats, dayPeriods, dateOptions, monthNames int
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
		case "date_options":
			dateOptions++
			options := DateOptions{Shapes: shapesOf(t, v.Shapes), Pivot: v.Pivot}
			if v.FlagBits != nil {
				options.Shapes = DateShapes(*v.FlagBits)
			}
			label = fmt.Sprintf("%+v", options)
			if options.valid() != (v.Expect == "ok") {
				t.Errorf("formats: valid() of %s = %v, want %s", label, options.valid(), v.Expect)
			}
			refusal = options.fault()
			if _, err := options.Raw(); (err == nil) != (v.Expect == "ok") {
				t.Errorf("formats: Raw() of %s = %v, want %s", label, err, v.Expect)
			}
			door = func() { DateOnlyOrderedDetailed("2026-01-07", YearMonthDay, options) }
		case "month_names":
			monthNames++
			entries := monthEntries(t, v.Months)
			label = fmt.Sprintf("%v %v", v.Months, v.AppendBytes)
			if v.AppendBytes == nil {
				_, refusal = NewMonthNames(entries...)
				door = func() { MustMonthNames(entries...) }
			} else {
				packed := MonthNames{}
				if base, err := NewMonthNames(entries...); err != nil {
					t.Fatalf("formats: %s: the base vocabulary was refused: %v", label, err)
				} else {
					packed = base
				}
				bytes := packed.Packed()
				for _, b := range v.AppendBytes {
					bytes = append(bytes, byte(b))
				}
				_, refusal = monthNamesFromPacked(bytes)
				door = func() {
					if _, err := monthNamesFromPacked(bytes); err != nil {
						panic(err.Error())
					}
				}
			}
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
	if whiteSpace < 30 || numFormats == 0 || dayPeriods == 0 || dateOptions == 0 || monthNames == 0 {
		t.Errorf("formats: replayed %d white_space, %d num_format, %d day_periods, %d date_options and %d month_names rows",
			whiteSpace, numFormats, dayPeriods, dateOptions, monthNames)
	}
}
