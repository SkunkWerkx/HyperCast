package hypercast

// The validation constraints: a door-bound Constraints[V], judged once by the core when it is
// built (cast_constraints_check, its decimal text converted by cast_declared_decimal), and
// the checked doors that judge a value by it after it parses (cast_checked,
// cast_checked_from_f64), plus the text door (cast_text). Every rule and every refusal is the
// core's; this file only lays the declaration out and reads the verdict back. Spans are
// UTF-8 bytes, as the core counts them and as every Fault here already is, so nothing is
// remapped.

import (
	"encoding/binary"
	"fmt"
	"math"
	"strconv"
)

// CheckedTarget is the closed set of types a Constraints can be bound to, one per checked
// door: every Number (the integer, real and decimal doors) and string (the text door).
type CheckedTarget interface {
	Number | string
}

// Rounding is what a value that is not a multiple of the declared MultipleOf becomes —
// the native core's codes, verbatim. Every mode but Strict needs a MultipleOf, and rounding
// happens on the exact decimal, before the bounds judge the result.
type Rounding uint32

const (
	// Strict makes a value that is not a multiple a KeywordMultipleOf violation. The
	// default.
	Strict Rounding = 0
	// Midpoint rounds to the nearest multiple, a tie to the even one.
	Midpoint Rounding = 1
	// Floor rounds down, toward negative infinity.
	Floor Rounding = 2
	// Ceiling rounds up, toward positive infinity.
	Ceiling Rounding = 3
)

// String names the mode in lower case: "strict", "midpoint", "floor", "ceiling".
func (r Rounding) String() string {
	switch r {
	case Strict:
		return "strict"
	case Midpoint:
		return "midpoint"
	case Floor:
		return "floor"
	case Ceiling:
		return "ceiling"
	default:
		return fmt.Sprintf("Rounding(%d)", uint32(r))
	}
}

// Keyword is the rule a Violation broke, carried as the native core's code and spelled the
// OpenAPI way by String ("exclusiveMinimum"). Open to additions: format and pattern may join
// in a minor release, so a switch over keywords needs a default arm, and a code this binding
// does not name yet is kept, its String "keyword N".
//
//	switch violation.Keyword {
//	case hypercast.KeywordMinimum, hypercast.KeywordMaximum:
//		return "out of bounds"
//	case hypercast.KeywordEnum:
//		return "not one of the allowed values"
//	default: // keywords are open
//		return "breaks " + violation.Keyword.String()
//	}
type Keyword uint32

const (
	// KeywordMinimum is minimum: the value is below it.
	KeywordMinimum Keyword = 1 + iota
	// KeywordExclusiveMinimum is exclusiveMinimum: the value is at or below it.
	KeywordExclusiveMinimum
	// KeywordMaximum is maximum: the value is above it.
	KeywordMaximum
	// KeywordExclusiveMaximum is exclusiveMaximum: the value is at or above it.
	KeywordExclusiveMaximum
	// KeywordMultipleOf is multipleOf: the value is not a multiple, under Strict, or the
	// multiple a rounding gives lies past what the door's type holds.
	KeywordMultipleOf
	// KeywordMinLength is minLength: the text has fewer code points.
	KeywordMinLength
	// KeywordMaxLength is maxLength: the text has more code points.
	KeywordMaxLength
	// KeywordEnum is enum: the value matches no entry.
	KeywordEnum
)

// String is the OpenAPI spelling: "minimum", "exclusiveMinimum", "maximum",
// "exclusiveMaximum", "multipleOf", "minLength", "maxLength", "enum"; "keyword N" for a code
// this binding does not name.
func (k Keyword) String() string {
	switch k {
	case KeywordMinimum:
		return "minimum"
	case KeywordExclusiveMinimum:
		return "exclusiveMinimum"
	case KeywordMaximum:
		return "maximum"
	case KeywordExclusiveMaximum:
		return "exclusiveMaximum"
	case KeywordMultipleOf:
		return "multipleOf"
	case KeywordMinLength:
		return "minLength"
	case KeywordMaxLength:
		return "maxLength"
	case KeywordEnum:
		return "enum"
	default:
		return fmt.Sprintf("keyword %d", uint32(k))
	}
}

// Violation is a value that parsed and broke a declared rule: the Keyword, the Value as the
// door cast it, and the value's span, as byte offsets into the input like a Fault's. The
// input is first cast to the type (a *Fault when it is not one), then judged; a violation is
// a value of the type, so it carries that value in V, exactly as cast — never rounded, since
// MultipleOf's rounding is part of the judgement, not of the value. At the text door the
// Value is the text judged (trimmed when Trim says so), not an enum entry. A from-float64
// door has no text to point into, so its span is empty.
type Violation[V any] struct {
	Keyword Keyword
	Value   V
	Offset  int
	Length  int
}

// Error renders the violation as "hypercast: breaks {keyword} at byte {start}..{end}", the
// span as a half-open byte range into the input. A nil *Violation renders as "<nil>", as a
// nil *Fault does.
func (v *Violation[V]) Error() string {
	if v == nil {
		return "<nil>"
	}
	return fmt.Sprintf("hypercast: breaks %s at byte %d..%d", v.Keyword, v.Offset, v.Offset+v.Length)
}

// Rejection is why a checked door gave no value: closed, exactly one of two — a *Fault, the
// very type every plain door returns (it did not parse), or a *Violation[V] of the door's
// type V (it parsed and broke a declared rule, and carries the value). A parse failure keeps its own closed lexicon, and a broken rule is a
// different type, not a fourth CastFailure. Nil is success. Sealed: nothing outside this
// package implements it, so a type switch over the two needs no default arm.
//
//	switch r := rejection.(type) {
//	case nil:
//		// success
//	case *hypercast.Fault:
//		// r.Reason, r.Offset, r.Length
//	case *hypercast.Violation[int32]: // V, the door's type
//		// r.Keyword, r.Value, r.Offset, r.Length
//	}
type Rejection interface {
	error
	rejection()
}

func (*Fault) rejection()        {}
func (*Violation[V]) rejection() {}

// Checked is what a checked door delivers on success: the value (rounded to the declared
// MultipleOf when the declaration rounds; at the text door with an enum, the caller's own
// declared entry), whether rounding changed it, and at the text door the index of the enum
// entry it matched — -1 otherwise, a numeric enum included, where the value is the answer.
// Every Checked a door returns, a rejection's zero value included, carries -1 when no entry
// matched.
type Checked[V any] struct {
	Value    V
	Adjusted bool
	// EnumIndex is the index of the text enum entry the value matched, or -1.
	EnumIndex int
}

// ConstraintValue is what a bound, a MultipleOf or an enum entry is declared as: text, an
// exact Decimal, or a Go integer — never a binary float, so 0.1 cannot be declared inexactly
// by accident (write "0.1"). Text at a numeric door is a decimal in invariant notation (an
// optional sign, digits with an optional '.' and fraction, an optional exponent: "0.000001",
// "-90", "1e-6"), converted exactly by the native core's own conversion, as a Decimal is
// through its String and an integer through its decimal digits: a value that does not fit 96
// bits at a scale of 28 or less is refused rather than rounded. Text at the text door is the
// enum entry itself.
type ConstraintValue interface {
	string | Decimal | int | int8 | int16 | int32 | int64 | uint | uint8 | uint16 | uint32 | uint64
}

// ruleKind is which keyword a Rule declares; the zero kind is a zero Rule.
type ruleKind uint8

const (
	ruleMinimum ruleKind = 1 + iota
	ruleExclusiveMinimum
	ruleMaximum
	ruleExclusiveMaximum
	ruleMultipleOf
	ruleRounding
	ruleMinLength
	ruleMaxLength
	ruleTrim
	ruleEnum
	ruleCaseSensitive
)

func (k ruleKind) String() string {
	return [...]string{"", "minimum", "exclusiveMinimum", "maximum", "exclusiveMaximum", "multipleOf",
		"rounding", "minLength", "maxLength", "trim", "enum", "caseSensitive"}[k]
}

// Rule is one keyword of a declaration, built by Minimum, ExclusiveMinimum, Maximum,
// ExclusiveMaximum, MultipleOf, WithRounding, MinLength, MaxLength, Trim, CaseSensitive or
// Enum and handed to NewConstraints. The zero Rule declares nothing and is refused.
type Rule struct {
	kind     ruleKind
	text     string   // a bound or step, as text
	length   int      // a length
	rounding Rounding // a rounding
	entries  []string // the enum entries, as text
	numbers  bool     // the enum entries were declared as numbers
}

// constraintText is a ConstraintValue as the text the core converts: the text itself, a
// Decimal's String, an integer's decimal digits. It reports whether the value was a number.
func constraintText[B ConstraintValue](value B) (string, bool) {
	switch v := any(value).(type) {
	case string:
		return v, false
	case Decimal:
		return v.String(), true
	case int:
		return strconv.FormatInt(int64(v), 10), true
	case int8:
		return strconv.FormatInt(int64(v), 10), true
	case int16:
		return strconv.FormatInt(int64(v), 10), true
	case int32:
		return strconv.FormatInt(int64(v), 10), true
	case int64:
		return strconv.FormatInt(v, 10), true
	case uint:
		return strconv.FormatUint(uint64(v), 10), true
	case uint8:
		return strconv.FormatUint(uint64(v), 10), true
	case uint16:
		return strconv.FormatUint(uint64(v), 10), true
	case uint32:
		return strconv.FormatUint(uint64(v), 10), true
	case uint64:
		return strconv.FormatUint(v, 10), true
	default:
		panic("unreachable: ConstraintValue admits no other type")
	}
}

func bound[B ConstraintValue](kind ruleKind, value B) Rule {
	text, _ := constraintText(value)
	return Rule{kind: kind, text: text}
}

// Minimum declares the smallest value allowed.
func Minimum[B ConstraintValue](value B) Rule { return bound(ruleMinimum, value) }

// ExclusiveMinimum declares a value every value must be above.
func ExclusiveMinimum[B ConstraintValue](value B) Rule { return bound(ruleExclusiveMinimum, value) }

// Maximum declares the largest value allowed.
func Maximum[B ConstraintValue](value B) Rule { return bound(ruleMaximum, value) }

// ExclusiveMaximum declares a value every value must be below.
func ExclusiveMaximum[B ConstraintValue](value B) Rule { return bound(ruleExclusiveMaximum, value) }

// MultipleOf declares the positive step every value must be a multiple of; WithRounding
// says what a value that is not one becomes.
func MultipleOf[B ConstraintValue](step B) Rule { return bound(ruleMultipleOf, step) }

// WithRounding declares what a value that is not a multiple of the declared MultipleOf
// becomes. Strict, the mode when none is declared, makes it a violation; any other needs a
// MultipleOf.
func WithRounding(rounding Rounding) Rule { return Rule{kind: ruleRounding, rounding: rounding} }

// MinLength declares the fewest code points the text may hold.
func MinLength(n int) Rule { return Rule{kind: ruleMinLength, length: n} }

// MaxLength declares the most code points the text may hold.
func MaxLength(n int) Rule { return Rule{kind: ruleMaxLength, length: n} }

// Trim declares that the text is trimmed of leading and trailing Unicode White_Space (U+00A0
// included) before it is judged.
func Trim() Rule { return Rule{kind: ruleTrim} }

// CaseSensitive declares that a text Enum matches byte for byte instead of under Unicode
// simple case folding, for a vocabulary whose case carries meaning ("mm" and "Mm"). A
// HyperCast extension of the text door, like Trim.
func CaseSensitive() Rule { return Rule{kind: ruleCaseSensitive} }

// Enum declares the values allowed: numbers at a numeric door (as numbers or their text),
// compared exactly to the value as cast, so an entry the door's type cannot hold (a fraction
// at an integer door, say) never matches; strings at the text door, matched under Unicode
// simple case folding unless CaseSensitive is declared — a match delivers the entry as
// declared, and its index; numbers at the text door are valid entries no string equals. Entries may repeat, the first match winning. Leaving Enum out
// declares no enum; Enum with no entries declares one that no value matches, so every value
// breaks it.
func Enum[E ConstraintValue](entries ...E) Rule {
	r := Rule{kind: ruleEnum, entries: make([]string, len(entries))}
	for i, entry := range entries {
		r.entries[i], r.numbers = constraintText(entry)
	}
	return r
}

// declaration is a Constraints as the core reads it: the layout, and the enum list beside it
// (numeric entries as Decimals, text entries packed with the declared strings kept).
type declaration struct {
	raw        RawConstraints
	decimals   []Decimal
	packedText []byte
	texts      []string
	// The enum as declared, for Enum: each entry's text, and whether they were numbers.
	enum        []string
	enumNumbers bool
}

// enumBytes is the enum list's length in bytes as it crosses.
func (d *declaration) enumBytes() int {
	if d.decimals != nil {
		return 16 * len(d.decimals)
	}
	return len(d.packedText)
}

// Constraints is a declaration of validation constraints — the OpenAPI Schema Object's
// keywords, as OpenAPI 3.2 (JSON Schema 2020-12) reads them — bound to the door its type
// names: Constraints[float64] for Check's F64 door, Constraints[Decimal] for Exact's,
// Constraints[string] for CheckString. Immutable, and judged once, by the native core, when
// NewConstraints or MustConstraints builds it: every valid OpenAPI declaration is accepted,
// one no value can satisfy included, and only one the core cannot read is refused there;
// every checked cast after that only judges data. There is no default: the zero Constraints was never judged,
// so handing one to a checked door (or asking it for Raw or PackedEnum) is a caller bug and
// panics, as a zero NumFormat does. MustConstraints[V]() with no rules is the declaration of
// none.
//
// The numeric doors take Minimum, ExclusiveMinimum, Maximum, ExclusiveMaximum (any of them
// together, each judged on its own), MultipleOf with WithRounding, and Enum; the text door
// MinLength, MaxLength, Trim, CaseSensitive and Enum. A keyword for another type (MinLength
// at a numeric door, Minimum at the text door) is ignored, as OpenAPI ignores it. The value is
// judged in the core, in this order, the first failure winning: the door parses (a *Fault,
// exactly as the plain door's); text is trimmed when Trim says so; MultipleOf, on the exact
// decimal, rounding when declared (a multiple past what the type holds breaks MultipleOf);
// Minimum, ExclusiveMinimum, Maximum, ExclusiveMaximum, on the rounded value; the lengths,
// in code points; Enum (text under Unicode simple case folding unless CaseSensitive,
// delivering the declared entry). A broken rule is a *Violation[V] carrying the value.
type Constraints[V CheckedTarget] struct {
	d declaration
}

// doorOf is the core's door code for V's checked door (core.go has the codes).
func doorOf[V CheckedTarget]() uint32 {
	var v V
	switch any(&v).(type) {
	case *int8:
		return doorI8
	case *int16:
		return doorI16
	case *int32:
		return doorI32
	case *int64:
		return doorI64
	case *uint8:
		return doorU8
	case *uint16:
		return doorU16
	case *uint32:
		return doorU32
	case *uint64:
		return doorU64
	case *float32:
		return doorF32
	case *float64:
		return doorF64
	case *string:
		return doorText
	case *Decimal:
		return doorDecimal
	default:
		panic("unreachable: CheckedTarget admits no other type")
	}
}

// refusal is a declaration NewConstraints refused: the core's refusal code, or 0 for a
// rule this binding enforces before the core sees the declaration.
type refusal struct {
	code    int32
	message string
}

func (r *refusal) Error() string { return "hypercast: " + r.message }

// refused is the core's refusal code as the error NewConstraints returns: one message per
// code, worded as the Rust core's Refusal displays it.
func refused(code int32) *refusal {
	var rule string
	switch code {
	case 1:
		rule = "a constraint the door does not take"
	case 5:
		rule = "multipleOf is not positive"
	case 6:
		rule = "a rounding without multipleOf"
	case 8:
		rule = "a bound, step or enum entry the door cannot hold"
	case 9:
		rule = "an enum entry that is not UTF-8"
	case 11:
		rule = "a malformed constraints layout"
	default:
		rule = "a rule this binding does not name"
	}
	return &refusal{code: code, message: fmt.Sprintf("Constraints refused (code %d): %s.", code, rule)}
}

// NewConstraints declares the rules and has the native core judge them for V's door, once.
// A refused declaration is an error, worded from the rule it broke ("hypercast: Constraints
// refused (code 8): a bound, step or enum entry the door cannot hold."), from the core's
// codes: 1 a keyword the door does not take (Trim or CaseSensitive at a numeric door, or
// WithRounding beside MultipleOf at the text door); 5 a MultipleOf not positive; 6 a rounding
// without MultipleOf; 8 a bound, step or entry no door can hold (text that is no decimal, or
// a decimal past 96 bits or scale 28), or WithRounding at an integer door with a fractional
// MultipleOf; 9 an enum entry that is not UTF-8. Everything else OpenAPI allows is accepted:
// crossed or equal bounds, an inclusive and an exclusive bound together, MinLength above
// MaxLength, repeated or no enum entries. This binding also refuses what a Go call can say
// but no OpenAPI declaration can: a keyword declared twice, a zero Rule and a length below
// zero or past math.MaxUint32. Numbers as the text door's entries are valid and never match,
// as JSON Schema's exact equality says. Use it for a declaration read
// at run time; MustConstraints is the form for one written in code.
func NewConstraints[V CheckedTarget](rules ...Rule) (Constraints[V], error) {
	door := doorOf[V]()
	d := declaration{raw: RawConstraints{Size: RawConstraintsSize}}
	var seen uint32
	for _, rule := range rules {
		if rule.kind == 0 {
			return Constraints[V]{}, &refusal{message: "a zero Rule declares nothing; build each with Minimum, Maximum, MultipleOf, Enum or another keyword."}
		}
		if seen&(1<<rule.kind) != 0 {
			return Constraints[V]{}, &refusal{message: fmt.Sprintf("%s is declared twice.", rule.kind)}
		}
		seen |= 1 << rule.kind
		var err *refusal
		switch rule.kind {
		case ruleMinimum:
			d.raw.Minimum, err = declared(rule.text)
			d.raw.Flags |= RawConstraintsHasMinimum
		case ruleExclusiveMinimum:
			d.raw.ExclusiveMinimum, err = declared(rule.text)
			d.raw.Flags |= RawConstraintsHasExclusiveMinimum
		case ruleMaximum:
			d.raw.Maximum, err = declared(rule.text)
			d.raw.Flags |= RawConstraintsHasMaximum
		case ruleExclusiveMaximum:
			d.raw.ExclusiveMaximum, err = declared(rule.text)
			d.raw.Flags |= RawConstraintsHasExclusiveMaximum
		case ruleMultipleOf:
			d.raw.MultipleOf, err = declared(rule.text)
			d.raw.Flags |= RawConstraintsHasMultipleOf
		case ruleRounding:
			d.raw.Rounding = uint32(rule.rounding)
		case ruleMinLength:
			d.raw.MinLength, err = length(rule.length, rule.kind)
			d.raw.Flags |= RawConstraintsHasMinLength
		case ruleMaxLength:
			d.raw.MaxLength, err = length(rule.length, rule.kind)
			d.raw.Flags |= RawConstraintsHasMaxLength
		case ruleTrim:
			d.raw.Flags |= RawConstraintsTrim
		case ruleCaseSensitive:
			d.raw.Flags |= RawConstraintsCaseSensitive
		case ruleEnum:
			err = d.pack(rule, door)
			d.raw.Flags |= RawConstraintsHasEnum
			d.enum = append(make([]string, 0, len(rule.entries)), rule.entries...)
			d.enumNumbers = rule.numbers
		}
		if err != nil {
			return Constraints[V]{}, err
		}
	}
	if code := callConstraintsCheck(door, &d); code != 0 {
		return Constraints[V]{}, refused(code)
	}
	return Constraints[V]{d: d}, nil
}

// MustConstraints is NewConstraints for a declaration written in code, where a refusal is a
// caller bug: it panics with NewConstraints's error text, as a door panics on a NumFormat the
// core would refuse.
func MustConstraints[V CheckedTarget](rules ...Rule) Constraints[V] {
	c, err := NewConstraints[V](rules...)
	if err != nil {
		panic(err.Error())
	}
	return c
}

// declared is a bound, step or numeric entry through the core's own conversion: the exact
// decimal, or the core's refusal 8.
func declared(text string) (Decimal, *refusal) {
	d, ok := declaredDecimal(text)
	if !ok {
		return Decimal{}, refused(8)
	}
	return d, nil
}

// declaredDecimal is the core's conversion of a declaration's decimal text
// (cast_declared_decimal), the one conversion every binding calls: the decimal, or false for
// text no door can hold (the core's 8).
func declaredDecimal(text string) (Decimal, bool) {
	ptr, length := textPtr(text)
	r := callDeclaredDecimal(ptr, length)
	switch r.code {
	case 0:
		raw := read[rawDecimal](&r)
		return Decimal{Lo: raw.Lo, Hi: raw.Hi, Scale: raw.Scale, Negative: raw.Negative != 0}, true
	case 8:
		return Decimal{}, false
	default:
		panic("hypercast: the core reported a contract violation — a binding bug, please report it")
	}
}

func length(n int, kind ruleKind) (uint32, *refusal) {
	if n < 0 || uint64(n) > math.MaxUint32 {
		return 0, &refusal{message: fmt.Sprintf("%s must be from 0 to %d; it is %d.", kind, uint32(math.MaxUint32), n)}
	}
	return uint32(n), nil
}

// pack lays the enum list out for the door: at the text door each entry a little-endian
// 32-bit byte length then its bytes, the declared strings kept; elsewhere Decimals, each
// through the core's conversion. No entries crosses as an empty list, under
// RawConstraintsHasEnum: an enum no value matches.
func (d *declaration) pack(rule Rule, door uint32) *refusal {
	if len(rule.entries) == 0 {
		return nil
	}
	if door != doorText {
		d.decimals = make([]Decimal, len(rule.entries))
		for i, entry := range rule.entries {
			var err *refusal
			if d.decimals[i], err = declared(entry); err != nil {
				return err
			}
		}
		return nil
	}
	if rule.numbers {
		// A number is a valid entry that no string equals (JSON Schema's enum is exact
		// equality across types): each is packed as an empty entry, which no input matches
		// (empty input is the door's Empty fault), so the list keeps its length and indexes.
		d.packedText = make([]byte, 4*len(rule.entries))
		d.texts = make([]string, len(rule.entries))
		return nil
	}
	size := 0
	for i, entry := range rule.entries {
		if uint64(len(entry)) > math.MaxUint32 {
			return &refusal{message: fmt.Sprintf("Enum entry %d is longer than %d UTF-8 bytes.", i, uint32(math.MaxUint32))}
		}
		size += 4 + len(entry)
	}
	d.packedText = make([]byte, 0, size)
	for _, entry := range rule.entries {
		d.packedText = binary.LittleEndian.AppendUint32(d.packedText, uint32(len(entry)))
		d.packedText = append(d.packedText, entry...)
	}
	d.texts = rule.entries
	return nil
}

// declaration is what a checked call hands the core. The zero Constraints was never judged
// by the core, so it is a caller bug, never a declaration of nothing.
func (c *Constraints[V]) declaration() *declaration {
	if c.d.raw.Size == 0 {
		var v V
		panic(fmt.Sprintf("hypercast: a zero Constraints[%T]; build it with NewConstraints or MustConstraints", v))
	}
	return &c.d
}

func (c Constraints[V]) bound(flag uint32, value Decimal) (Decimal, bool) {
	if c.d.raw.Flags&flag == 0 {
		return Decimal{}, false
	}
	return value, true
}

// Enum is the declared enum: each entry as Enum rendered it (a number as its decimal text), in
// order, duplicates kept; numbers reports that the entries were declared as numbers; and ok is
// false when no enum is declared. An empty enum, which every value violates, is ok with no
// entries. The slice is a fresh copy each call, so changing it changes nothing here.
func (c Constraints[V]) Enum() (entries []string, numbers bool, ok bool) {
	if c.d.raw.Flags&RawConstraintsHasEnum == 0 {
		return nil, false, false
	}
	return append(make([]string, 0, len(c.d.enum)), c.d.enum...), c.d.enumNumbers, true
}

// Minimum is the declared minimum, and whether one is declared.
func (c Constraints[V]) Minimum() (Decimal, bool) {
	return c.bound(RawConstraintsHasMinimum, c.d.raw.Minimum)
}

// ExclusiveMinimum is the declared exclusiveMinimum, and whether one is declared.
func (c Constraints[V]) ExclusiveMinimum() (Decimal, bool) {
	return c.bound(RawConstraintsHasExclusiveMinimum, c.d.raw.ExclusiveMinimum)
}

// Maximum is the declared maximum, and whether one is declared.
func (c Constraints[V]) Maximum() (Decimal, bool) {
	return c.bound(RawConstraintsHasMaximum, c.d.raw.Maximum)
}

// ExclusiveMaximum is the declared exclusiveMaximum, and whether one is declared.
func (c Constraints[V]) ExclusiveMaximum() (Decimal, bool) {
	return c.bound(RawConstraintsHasExclusiveMaximum, c.d.raw.ExclusiveMaximum)
}

// MultipleOf is the declared multipleOf, and whether one is declared.
func (c Constraints[V]) MultipleOf() (Decimal, bool) {
	return c.bound(RawConstraintsHasMultipleOf, c.d.raw.MultipleOf)
}

// Rounding is what a value that is not a multiple of MultipleOf becomes.
func (c Constraints[V]) Rounding() Rounding { return Rounding(c.d.raw.Rounding) }

// MinLength is the declared minLength, and whether one is declared.
func (c Constraints[V]) MinLength() (int, bool) {
	return int(c.d.raw.MinLength), c.d.raw.Flags&RawConstraintsHasMinLength != 0
}

// MaxLength is the declared maxLength, and whether one is declared.
func (c Constraints[V]) MaxLength() (int, bool) {
	return int(c.d.raw.MaxLength), c.d.raw.Flags&RawConstraintsHasMaxLength != 0
}

// Trim is whether text is trimmed of Unicode White_Space before it is judged.
func (c Constraints[V]) Trim() bool { return c.d.raw.Flags&RawConstraintsTrim != 0 }

// CaseSensitive is whether a text enum matches byte for byte rather than under Unicode
// simple case folding.
func (c Constraints[V]) CaseSensitive() bool { return c.d.raw.Flags&RawConstraintsCaseSensitive != 0 }

// Raw is the declaration laid out as the core reads it — what every checked door hands the
// core, beside PackedEnum — for a library judging values across a C ABI of its own. The core
// judged it when it was declared. The zero Constraints panics, as a checked door does.
func (c Constraints[V]) Raw() RawConstraints { return c.declaration().raw }

// PackedEnum is the declaration's enum list as the core reads it beside Raw, a fresh copy, nil
// for none: at a numeric door 16-byte Decimals in the host's byte order, at the text door
// each entry a little-endian 32-bit byte length followed by its bytes. The zero Constraints
// panics, as a checked door does.
func (c Constraints[V]) PackedEnum() []byte {
	d := c.declaration()
	if d.decimals == nil {
		if d.packedText == nil {
			return nil
		}
		return append([]byte(nil), d.packedText...)
	}
	out := make([]byte, 16*len(d.decimals))
	for i, entry := range d.decimals {
		dst := out[16*i:]
		binary.NativeEndian.PutUint64(dst, entry.Lo)
		binary.NativeEndian.PutUint32(dst[8:], entry.Hi)
		dst[12] = entry.Scale
		if entry.Negative {
			dst[13] = 1
		}
	}
	return out
}

// The checked doors.

// violated is the *Violation a code 4 names: the keyword the check carries, the value as
// cast, and its span, UTF-8 bytes of the input.
func violated[V any](r *result, c RawCheck, value V) *Violation[V] {
	if c.Keyword == 0 {
		panic("hypercast: the core reported a violation of no keyword — a binding bug, please report it")
	}
	return &Violation[V]{Keyword: Keyword(c.Keyword), Value: value, Offset: int(r.fault.Offset), Length: int(r.fault.Length)}
}

// checkedNumber reads a numeric checked door's verdict: 4 a violation carrying the value as
// cast, 1 to 3 the door's own fault, both spanning UTF-8 bytes of the input.
func checkedNumber[V Number](r *result, c RawCheck) (Checked[V], Rejection) {
	switch r.code {
	case 0:
		return checkedValue[V](r, c), nil
	case 4:
		return Checked[V]{EnumIndex: -1}, violated(r, c, numberOut[V](r))
	default:
		return Checked[V]{EnumIndex: -1}, failed(r.code, &r.fault)
	}
}

// numberOut is the value a numeric checked door wrote to out, on a success or a violation.
func numberOut[V Number](r *result) V {
	var out V
	if p, ok := any(&out).(*Decimal); ok {
		raw := read[rawDecimal](r)
		*p = Decimal{Lo: raw.Lo, Hi: raw.Hi, Scale: raw.Scale, Negative: raw.Negative != 0}
	} else {
		out = read[V](r)
	}
	return out
}

// checkedValue is a numeric checked door's success.
func checkedValue[V Number](r *result, c RawCheck) Checked[V] {
	return Checked[V]{Value: numberOut[V](r), Adjusted: c.Adjusted != 0, EnumIndex: -1}
}

// Check casts numeric text to V under the declared format, as V's own door does (Check with
// a Constraints[int32] is I32, with a Constraints[Decimal] Exact), then judges the value by
// constraints. The Rejection is nil on success, else the door's own *Fault or a
// *Violation[V] carrying the value as cast, both spanning bytes of the input. V is inferred from constraints and T from text:
// hypercast.Check("45.5", hypercast.Invariant, latitude).
func Check[V Number, T Text](text T, format NumFormat, constraints Constraints[V]) (Checked[V], Rejection) {
	var raw RawNumFormat
	format.raw(&raw)
	ptr, length := textPtr(text)
	r, c := callChecked(doorOf[V](), ptr, length, &raw, constraints.declaration())
	return checkedNumber[V](&r, c)
}

// CheckFromFloat64 judges a number the caller already holds — a JSON parser's float64, a
// spreadsheet's numeric cell — at V's door by constraints, on its sign and the shortest
// decimal that names it: the judgement Check makes of that text (strconv.FormatFloat(value,
// 'g', -1, 64)'s digits). So 0.1 + 0.2 breaks MultipleOf("0.1") under Strict and rounds to
// 0.3 under Midpoint. -0.0 is zero, delivered unsigned. NaN is Malformed; an infinity is
// OutOfRange at a float or Decimal door and Malformed at an integer door. With no text to
// point into, every fault and violation has an empty span.
func CheckFromFloat64[V Number](value float64, constraints Constraints[V]) (Checked[V], Rejection) {
	r, c := callCheckedFromF64(doorOf[V](), value, constraints.declaration())
	return checkedNumber[V](&r, c)
}

// slice is the span of the input the core judged to be the text: a substring of a string
// input, which costs nothing, or a copy of a []byte input's bytes.
func slice[T Text](text T, span RawSpan) string {
	switch v := any(text).(type) {
	case string:
		return v[span.Offset : span.Offset+span.Len]
	case []byte:
		return string(v[span.Offset : span.Offset+span.Len])
	default:
		panic("unreachable")
	}
}

// OptionalChecked presents a checked door's verdict optionally, as Optional does a plain
// one's: an Empty *Fault is absent, (nil, nil); a success is a pointer to the Checked, (&checked,
// nil); every other rejection, a *Violation included, flows through untouched, (nil, rejection).
//
//	checked, rejection := hypercast.OptionalChecked(hypercast.Check(text, hypercast.Invariant, percent))
func OptionalChecked[V any](checked Checked[V], rejection Rejection) (*Checked[V], Rejection) {
	if rejection != nil {
		if fault, ok := rejection.(*Fault); ok && fault.Reason == Empty {
			return nil, nil
		}
		return nil, rejection
	}
	return &checked, nil
}

// String casts text as text: the input itself, once it is valid UTF-8, untrimmed. Empty
// input is Empty; invalid UTF-8 is Malformed, spanning the first invalid sequence (a Go
// string can hold any bytes, so a string input is judged too). A string input comes back as
// the caller's own string; a []byte input as a copy. Named for what it returns, since Text
// is the constraint every door's input satisfies.
func String[T Text](text T) (string, *Fault) {
	r := textCall(doorText, text)
	if r.code != 0 {
		return "", failed(r.code, &r.fault)
	}
	return slice(text, read[RawSpan](&r)), nil
}

// CheckString casts text as String does, then judges it by constraints: trimmed of Unicode
// White_Space when Trim says so, its length counted in code points, and an enum matched under
// Unicode simple case folding (byte for byte under CaseSensitive) — a match delivers the
// caller's own declared entry, and its index. Otherwise the value is the input judged
// (trimmed when declared), and a *Violation[string] carries that text. Spans are bytes into
// the input, as a fault's are.
func CheckString[T Text](text T, constraints Constraints[string]) (Checked[string], Rejection) {
	ptr, length := textPtr(text)
	d := constraints.declaration()
	r, c := callChecked(doorText, ptr, length, nil, d)
	switch r.code {
	case 0:
		return checkedText(text, d, &r, c), nil
	case 4:
		return Checked[string]{EnumIndex: -1}, violated(&r, c, slice(text, read[RawSpan](&r)))
	default:
		return Checked[string]{EnumIndex: -1}, failed(r.code, &r.fault)
	}
}

// checkedText is the text checked door's success: the declared entry it matched, or the
// input judged.
func checkedText[T Text](text T, d *declaration, r *result, c RawCheck) Checked[string] {
	if c.EnumIndex != math.MaxUint32 {
		if uint64(c.EnumIndex) >= uint64(len(d.texts)) {
			panic(fmt.Sprintf("hypercast: the core matched enum entry %d, which was never declared — a binding bug, please report it", c.EnumIndex))
		}
		return Checked[string]{Value: d.texts[c.EnumIndex], Adjusted: c.Adjusted != 0, EnumIndex: int(c.EnumIndex)}
	}
	return Checked[string]{Value: slice(text, read[RawSpan](r)), Adjusted: c.Adjusted != 0, EnumIndex: -1}
}
