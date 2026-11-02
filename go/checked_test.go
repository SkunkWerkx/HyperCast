package hypercast

import (
	"errors"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"reflect"
	"regexp"
	"strconv"
	"strings"
	"testing"
	"time"
	"unsafe"

	"github.com/google/uuid"
)

// The plain doors keep exactly their shape, (value, *Fault): each signature is pinned at
// compile time, so a change to one fails the build of this suite.
var (
	_ func(string) (bool, *Fault)                       = Bool[string]
	_ func(string) (rune, *Fault)                       = Char[string]
	_ func(string, NumFormat) (int8, *Fault)            = I8[string]
	_ func(string, NumFormat) (int16, *Fault)           = I16[string]
	_ func(string, NumFormat) (int32, *Fault)           = I32[string]
	_ func(string, NumFormat) (int64, *Fault)           = I64[string]
	_ func(string, NumFormat) (uint8, *Fault)           = U8[string]
	_ func(string, NumFormat) (uint16, *Fault)          = U16[string]
	_ func(string, NumFormat) (uint32, *Fault)          = U32[string]
	_ func(string, NumFormat) (uint64, *Fault)          = U64[string]
	_ func(string, NumFormat) (float32, *Fault)         = F32[string]
	_ func(string, NumFormat) (float64, *Fault)         = F64[string]
	_ func(string, NumFormat) (Decimal, *Fault)         = Exact[string]
	_ func(string, NumFormat) (int32, *Fault)           = Numeric[int32, string]
	_ func(string) (uuid.UUID, *Fault)                  = Uuid[string]
	_ func(string) (time.Time, *Fault)                  = Timestamp[string]
	_ func(string, UnixPrecision) (time.Time, *Fault)   = Unix[string]
	_ func(string, ExcelEpoch) (time.Time, *Fault)      = ExcelSerial[string]
	_ func(string) (Date, *Fault)                       = DateOnly[string]
	_ func(string, DateOrder) (Date, *Fault)            = DateOnlyOrdered[string]
	_ func(string, DateOrder) (CivilDateTime, *Fault)   = DateTime[string]
	_ func(string, DateFormat) (CivilDateTime, *Fault)  = DateTimeWith[string]
	_ func(string) (time.Duration, *Fault)              = TimeOfDay[string]
	_ func(string) (Duration, *Fault)                   = Span[string]
	_ func(float64) (Decimal, *Fault)                   = ExactFromFloat64
	_ func(float64, ExcelEpoch) (CivilDateTime, *Fault) = ExcelSerialFromFloat64
	_ func(float64) (time.Duration, *Fault)             = ExcelTime
	_ func(float64) (Duration, *Fault)                  = ExcelDuration
	_ func(string) (string, *Fault)                     = String[string]
)

// And Fault keeps exactly its three fields and the reasons their codes. A change here is a
// breaking change, not an addition.
func TestThePlainShapesAreUnchanged(t *testing.T) {
	fault := reflect.TypeOf(Fault{})
	want := []struct {
		name string
		typ  reflect.Type
	}{
		{"Reason", reflect.TypeOf(CastFailure(0))},
		{"Offset", reflect.TypeOf(0)},
		{"Length", reflect.TypeOf(0)},
	}
	if fault.NumField() != len(want) {
		t.Fatalf("Fault has %d fields, want %d", fault.NumField(), len(want))
	}
	for i, field := range want {
		if got := fault.Field(i); got.Name != field.name || got.Type != field.typ {
			t.Errorf("Fault field %d is %s %s, want %s %s", i, got.Name, got.Type, field.name, field.typ)
		}
	}
	if Empty != 1 || Malformed != 2 || OutOfRange != 3 {
		t.Errorf("CastFailure codes moved: %d %d %d", Empty, Malformed, OutOfRange)
	}
	// A *Fault is a Rejection, so a parse failure reads the same at a plain and a checked door.
	var _ Rejection = (*Fault)(nil)
}

// header finds include/hypercast.h the way corpus finds corpus/: beside the mounted corpus
// on WASI, else up from the test directory. The iOS simulator carries only testdata, where
// the header is not to be had.
func header(t *testing.T) string {
	t.Helper()
	candidates := []string{}
	if dir := os.Getenv("HYPERCAST_CORPUS"); dir != "" {
		candidates = append(candidates, filepath.Join(dir, "..", "include", "hypercast.h"))
	}
	if dir, err := os.Getwd(); err == nil {
		for {
			candidates = append(candidates, filepath.Join(dir, "include", "hypercast.h"))
			parent := filepath.Dir(dir)
			if parent == dir {
				break
			}
			dir = parent
		}
	}
	for _, path := range candidates {
		if data, err := os.ReadFile(path); err == nil {
			return string(data)
		}
	}
	t.Skip("include/hypercast.h is not reachable from here")
	return ""
}

// Each layout this binding restates is the size the generated header carries.
func TestTheLayoutsAreTheHeadersSizes(t *testing.T) {
	text := header(t)
	for name, size := range map[string]uint32{
		"HyperCastRawNumFormat_SIZE":   RawNumFormatSize,
		"HyperCastRawDayPeriods_SIZE":  RawDayPeriodsSize,
		"HyperCastRawConstraints_SIZE": RawConstraintsSize,
		"HyperCastRawCheck_SIZE":       RawCheckSize,
		"HyperCastRawSpan_SIZE":        RawSpanSize,
	} {
		m := regexp.MustCompile(`#define ` + name + ` (\d+)`).FindStringSubmatch(text)
		if m == nil {
			t.Errorf("the header defines no %s", name)
			continue
		}
		if want, _ := strconv.ParseUint(m[1], 10, 32); uint32(want) != size {
			t.Errorf("%s is %s, this binding's layout is %d", name, m[1], size)
		}
	}
	for flag, value := range map[string]uint32{
		"HAS_MINIMUM": RawConstraintsHasMinimum, "HAS_EXCLUSIVE_MINIMUM": RawConstraintsHasExclusiveMinimum,
		"HAS_MAXIMUM": RawConstraintsHasMaximum, "HAS_EXCLUSIVE_MAXIMUM": RawConstraintsHasExclusiveMaximum,
		"HAS_MULTIPLE_OF": RawConstraintsHasMultipleOf, "HAS_MIN_LENGTH": RawConstraintsHasMinLength,
		"HAS_MAX_LENGTH": RawConstraintsHasMaxLength, "TRIM": RawConstraintsTrim,
		"HAS_ENUM": RawConstraintsHasEnum,
	} {
		want := fmt.Sprintf("#define HyperCastRawConstraints_%s %d\n", flag, value)
		if !strings.Contains(text, want) {
			t.Errorf("the header does not carry %q", strings.TrimSpace(want))
		}
	}
}

func refusalOf(t *testing.T, err error) *refusal {
	t.Helper()
	var r *refusal
	if !errors.As(err, &r) {
		t.Fatalf("got %v, want a refusal", err)
	}
	return r
}

// One message per core code, worded as the Rust core's Refusal displays it.
func TestEachRefusalCodeHasItsOwnMessage(t *testing.T) {
	want := map[int32]string{
		1:  "a constraint the door does not take",
		2:  "minimum together with exclusiveMinimum",
		3:  "maximum together with exclusiveMaximum",
		4:  "the lower bound is not below the upper bound",
		5:  "multipleOf is not positive",
		6:  "a rounding without multipleOf",
		7:  "minLength is above maxLength",
		8:  "a bound, step or enum entry the door cannot hold",
		9:  "an enum entry that is not UTF-8",
		10: "a duplicate enum entry",
		11: "a malformed constraints layout",
		12: "an enum with no entries",
		13: "a text enum entry no input can match",
		14: "a rule this binding does not name",
	}
	for code, rule := range want {
		if got := refused(code).Error(); got != fmt.Sprintf("hypercast: Constraints refused (code %d): %s.", code, rule) {
			t.Errorf("code %d: %q", code, got)
		}
	}
	defer func() {
		if p := recover(); p != "hypercast: Constraints refused (code 8): a bound, step or enum entry the door cannot hold." {
			t.Errorf("MustConstraints[uint8](Minimum(-1)) panicked with %v", p)
		}
	}()
	MustConstraints[uint8](Minimum(-1))
}

// A Go string can hold any bytes, so a text entry that is not UTF-8 reaches the core, which
// refuses it as 9; the same bytes as input are Malformed with their span.
func TestInvalidUTF8ReachesTheCore(t *testing.T) {
	_, err := NewConstraints[string](Enum("Male", "F\xffmale"))
	if r := refusalOf(t, err); r.code != 9 {
		t.Errorf("an invalid UTF-8 entry refused with %d, want 9", r.code)
	}
	sex := MustConstraints[string](Enum("Female", "Male"))
	_, rejection := CheckString("M\xe2\x82le", sex)
	if fault, ok := rejection.(*Fault); !ok || *fault != (Fault{Reason: Malformed, Offset: 1, Length: 2}) {
		t.Errorf("invalid UTF-8 input -> %v, want malformed at 1+2", rejection)
	}
	if _, fault := String("ab\xffc"); fault == nil || *fault != (Fault{Reason: Malformed, Offset: 2, Length: 1}) {
		t.Errorf(`String("ab\xffc") -> %v, want malformed at 2+1`, fault)
	}
}

// What this binding refuses before the core sees the declaration, each with its own words.
func TestTheBindingsOwnRefusals(t *testing.T) {
	for _, c := range []struct {
		name  string
		err   error
		words string
	}{
		{"zero Rule", second(NewConstraints[int32](Rule{})), "a zero Rule declares nothing"},
		{"twice", second(NewConstraints[int32](Minimum(1), Minimum(2))), "minimum is declared twice."},
		{"rounding twice", second(NewConstraints[int32](WithRounding(Floor), WithRounding(Floor))), "rounding is declared twice."},
		{"number at text", second(NewConstraints[string](Enum(1, 2))), "Text enum entries are strings; entry 0 is the number 1."},
		{"negative length", second(NewConstraints[string](MinLength(-1))), "minLength must be from 0 to 4294967295; it is -1."},
	} {
		r := refusalOf(t, c.err)
		if r.code != 0 || !strings.Contains(r.Error(), c.words) || !strings.HasPrefix(r.Error(), "hypercast: ") {
			t.Errorf("%s: %v (code %d), want %q", c.name, r, r.code, c.words)
		}
	}
	// Control: the same keywords, declared once each and right, are accepted.
	if _, err := NewConstraints[string](MinLength(0), Enum("a")); err != nil {
		t.Errorf("a sound declaration refused: %v", err)
	}
}

func second[A, B any](_ A, b B) B { return b }

// Bounds take text, a Decimal or a Go integer, all through the core's one conversion; the
// accessors hand back what the core holds.
func TestBoundsTakeTextDecimalsAndIntegers(t *testing.T) {
	step, _ := Exact("0.05", Invariant)
	c := MustConstraints[Decimal](Minimum(-90), ExclusiveMaximum(uint8(90)), MultipleOf(step), WithRounding(Midpoint))
	if m, ok := c.Minimum(); !ok || m.String() != "-90" {
		t.Errorf("Minimum() = %v %v", m, ok)
	}
	if m, ok := c.ExclusiveMaximum(); !ok || m.String() != "90" {
		t.Errorf("ExclusiveMaximum() = %v %v", m, ok)
	}
	if _, ok := c.Maximum(); ok {
		t.Error("Maximum() declared")
	}
	if _, ok := c.ExclusiveMinimum(); ok {
		t.Error("ExclusiveMinimum() declared")
	}
	if m, ok := c.MultipleOf(); !ok || m != step {
		t.Errorf("MultipleOf() = %v %v", m, ok)
	}
	if c.Rounding() != Midpoint || c.Trim() {
		t.Errorf("Rounding() %v, Trim() %v", c.Rounding(), c.Trim())
	}
	raw := c.Raw()
	if raw.Size != RawConstraintsSize || raw.Flags != RawConstraintsHasMinimum|RawConstraintsHasExclusiveMaximum|RawConstraintsHasMultipleOf || raw.Rounding != 1 {
		t.Errorf("Raw() = %+v", raw)
	}
	text := MustConstraints[string](MinLength(1), MaxLength(3), Trim(), Enum("ab", "c"))
	if n, ok := text.MinLength(); !ok || n != 1 {
		t.Errorf("MinLength() = %d %v", n, ok)
	}
	if n, ok := text.MaxLength(); !ok || n != 3 || !text.Trim() {
		t.Errorf("MaxLength() = %d %v, Trim() %v", n, ok, text.Trim())
	}
	if got := text.PackedEnum(); string(got) != "\x02\x00\x00\x00ab\x01\x00\x00\x00c" {
		t.Errorf("PackedEnum() = %q", got)
	}
	ints := MustConstraints[int64](Enum(7, -3))
	packed := ints.PackedEnum()
	if len(packed) != 32 || *(*Decimal)(unsafe.Pointer(&packed[16])) != (Decimal{Lo: 3, Negative: true}) {
		t.Errorf("PackedEnum() = %v", packed)
	}
	if MustConstraints[int64]().PackedEnum() != nil || MustConstraints[int64]().Raw().Size != RawConstraintsSize {
		t.Error("an empty declaration packs an enum or no size")
	}
}

// The zero Constraints was never judged: a checked door, Raw and PackedEnum panic on it as a
// caller bug, and the explicit declaration of no rules is MustConstraints with none.
func TestTheZeroConstraintsIsACallerBug(t *testing.T) {
	const want = "hypercast: a zero Constraints[%s]; build it with NewConstraints or MustConstraints"
	for _, c := range []struct {
		name, typ string
		call      func()
	}{
		{"Check", "float64", func() { Check("-12.5", Invariant, Constraints[float64]{}) }},
		{"CheckFromFloat64", "hypercast.Decimal", func() { CheckFromFloat64(1, Constraints[Decimal]{}) }},
		{"CheckString", "string", func() { CheckString("Male", Constraints[string]{}) }},
		{"Raw", "int32", func() { (Constraints[int32]{}).Raw() }},
		{"PackedEnum", "int32", func() { (Constraints[int32]{}).PackedEnum() }},
	} {
		func() {
			defer func() {
				if p := recover(); p != fmt.Sprintf(want, c.typ) {
					t.Errorf("%s on the zero Constraints panicked with %v", c.name, p)
				}
			}()
			c.call()
		}()
	}
	got, rejection := Check("-12.5", Invariant, MustConstraints[float64]())
	if rejection != nil || got != (Checked[float64]{Value: -12.5, EnumIndex: -1}) {
		t.Errorf("Check under no rules -> %v %v", got, rejection)
	}
	text, rejection := CheckString("  Male ", MustConstraints[string]())
	if rejection != nil || text.Value != "  Male " {
		t.Errorf("CheckString under no rules -> %v %v", text, rejection)
	}
}

// Success is an untyped nil Rejection, so `rejection == nil` is the test; a failure is
// exactly one of the two pointer types, and its Checked carries no entry.
func TestARejectionIsAFaultOrAViolation(t *testing.T) {
	small := MustConstraints[int32](Maximum(10))
	if _, rejection := Check("7", Invariant, small); rejection != nil {
		t.Errorf("success carries %#v", rejection)
	}
	got, rejection := Check("11", Invariant, small)
	if v, ok := rejection.(*Violation); !ok || *v != (Violation{Keyword: KeywordMaximum, Offset: 0, Length: 2}) || got.EnumIndex != -1 {
		t.Errorf("Check(11) -> %v %#v", got, rejection)
	}
	if rejection.Error() != "hypercast: breaks maximum at byte 0..2" {
		t.Errorf("Error() = %q", rejection.Error())
	}
	if _, rejection := Check("1x", Invariant, small); rejection.Error() != "hypercast: malformed at byte 1..2" {
		t.Errorf("Check(1x) -> %v", rejection)
	}
	if (*Violation)(nil).Error() != "<nil>" {
		t.Error("a nil *Violation does not render as <nil>")
	}
}

func TestKeywordsAreOpen(t *testing.T) {
	names := []string{"minimum", "exclusiveMinimum", "maximum", "exclusiveMaximum", "multipleOf", "minLength", "maxLength", "enum"}
	for i, name := range names {
		if got := Keyword(i + 1).String(); got != name {
			t.Errorf("Keyword(%d) = %q, want %q", i+1, got, name)
		}
	}
	if KeywordEnum != 8 || Keyword(9).String() != "keyword 9" || Keyword(0).String() != "keyword 0" {
		t.Error("an unknown keyword is not kept as its code")
	}
	for r, name := range map[Rounding]string{Strict: "strict", Midpoint: "midpoint", Floor: "floor", Ceiling: "ceiling", 7: "Rounding(7)"} {
		if r.String() != name {
			t.Errorf("Rounding %d is %q, want %q", uint32(r), r, name)
		}
	}
	if Strict != 0 || Midpoint != 1 || Floor != 2 || Ceiling != 3 {
		t.Error("the rounding codes moved")
	}
}

// A text enum match is the caller's own declared string, not a copy of the input; without
// an enum the value is the caller's input itself, trimmed by slicing.
func TestATextMatchIsTheCallersOwnString(t *testing.T) {
	declared := strings.Repeat("Male", 1)
	sex := MustConstraints[string](Trim(), Enum("Female", declared, "Other"))
	got, rejection := CheckString(" MALE ", sex)
	if rejection != nil || got.Value != "Male" || got.EnumIndex != 1 || unsafe.StringData(got.Value) != unsafe.StringData(declared) {
		t.Errorf("CheckString(\" MALE \") -> %+v %v, want the declared entry 1", got, rejection)
	}
	input := "  padded  "
	trimmed, _ := CheckString(input, MustConstraints[string](Trim()))
	if trimmed.Value != "padded" || unsafe.StringData(trimmed.Value) != unsafe.StringData(input[2:]) {
		t.Errorf("a trimmed string is %q, not a slice of the input", trimmed.Value)
	}
	if whole, _ := String(input); unsafe.StringData(whole) != unsafe.StringData(input) {
		t.Error("String copied a string input")
	}
	if _, fault := String(""); fault == nil || fault.Reason != Empty {
		t.Errorf(`String("") -> %v, want empty`, fault)
	}
	if b, fault := String([]byte("bytes")); fault != nil || b != "bytes" {
		t.Errorf("String([]byte) -> %q %v", b, fault)
	}
	_, rejection = CheckString("Femal", sex)
	if v, ok := rejection.(*Violation); !ok || *v != (Violation{Keyword: KeywordEnum, Offset: 0, Length: 5}) {
		t.Errorf(`CheckString("Femal") -> %v`, rejection)
	}
}

// A text, or an enum list, past the base scratch runs on a one-off instance carrying the
// declaration with it.
func TestALongTextOrEnumRunsOnAOneOffInstance(t *testing.T) {
	long := strings.Repeat("x", baseText+10)
	got, rejection := CheckString(long, MustConstraints[string](MaxLength(baseText+10)))
	if rejection != nil || len(got.Value) != len(long) {
		t.Errorf("a long text -> %d bytes, %v", len(got.Value), rejection)
	}
	if _, rejection := CheckString(long, MustConstraints[string](MaxLength(baseText))); rejection == nil {
		t.Error("a long text over maxLength was not refused")
	}
	// Few long entries, distinct from their first byte on: the core's duplicate search
	// compares every pair, which costs seconds under TinyGo for thousands of short ones.
	pad := strings.Repeat("x", 120)
	entries := make([]string, 0, 600)
	for i := range 600 {
		entries = append(entries, fmt.Sprintf("%03d-%s", i, pad))
	}
	many, err := NewConstraints[string](Enum(entries...))
	if err != nil {
		t.Fatal(err)
	}
	if len(many.PackedEnum()) <= baseText {
		t.Fatalf("the enum list is %d bytes, inside the base scratch", len(many.PackedEnum()))
	}
	if got, rejection := CheckString(strings.ToUpper(entries[599]), many); rejection != nil || got.EnumIndex != 599 || got.Value != entries[599] {
		t.Errorf("a long enum -> %+v %v", got, rejection)
	}
	// The text fits the base scratch, but not with the declaration past it: exactly one
	// one-off instance, and the door's own verdict (far past 96 bits).
	near := strings.Repeat("9", baseText-20)
	positive := MustConstraints[Decimal](Minimum(0))
	base := withFreshInstances(t)
	if _, rejection := Check(near, Invariant, positive); rejection == nil || rejection.(*Fault).Reason != OutOfRange {
		t.Errorf("a text at the scratch's edge -> %v", rejection)
	}
	if d := instancesCreated.Load() - base; d != 1 {
		t.Errorf("a text at the scratch's edge created %d instances, want the one-off", d)
	}
	// Control: the same text under the plain door fits the base scratch and creates none.
	base = instancesCreated.Load()
	if _, fault := Exact(near, Invariant); fault == nil || instancesCreated.Load() != base {
		t.Errorf("the plain door -> %v, %d instances", fault, instancesCreated.Load()-base)
	}
}

// From a float64: the shortest decimal that names it, judged as its text would be, and a
// span that is always empty.
func TestCheckFromFloat64JudgesTheShortestDecimal(t *testing.T) {
	// Variables, so the sum is float64 arithmetic: Go folds the constant 0.1 + 0.2 exactly.
	a, b := 0.1, 0.2
	tenths := MustConstraints[float64](MultipleOf("0.1"))
	if _, rejection := CheckFromFloat64(a+b, tenths); rejection == nil || rejection.(*Violation).Length != 0 {
		t.Errorf("0.1 + 0.2 under strict -> %v", rejection)
	}
	rounded := MustConstraints[float64](MultipleOf("0.1"), WithRounding(Midpoint))
	if got, rejection := CheckFromFloat64(a+b, rounded); rejection != nil || got.Value != 0.3 || !got.Adjusted {
		t.Errorf("0.1 + 0.2 under midpoint -> %v %v", got, rejection)
	}
	if got, _ := CheckFromFloat64(math.Copysign(0, -1), rounded); math.Float64bits(got.Value) != 0 || got.Adjusted {
		t.Errorf("-0.0 -> %v (bits %x), adjusted %v", got.Value, math.Float64bits(got.Value), got.Adjusted)
	}
	if _, rejection := CheckFromFloat64(math.NaN(), rounded); rejection == nil || rejection.(*Fault).Reason != Malformed {
		t.Errorf("NaN -> %v", rejection)
	}
}
