package hypercast

// The detailed doors: every door again, returning a *Failure that names what failed (a
// Detail) beside the reason and span, and the detailed date doors taking DateOptions. Go
// cannot add a field to Fault without breaking unkeyed literals and equality, so the detail
// travels on this separate path and the 1.0 doors stay exactly as they were; both are the
// same parse in the core, so a plain door's *Fault is always the *Failure's Fault().

import (
	"fmt"
	"time"

	"github.com/google/uuid"
)

// Detail names what failed, beside the broad CastFailure: a small shared vocabulary, the
// native core's codes verbatim, mainly for people diagnosing rejected data. Published codes
// keep their numbers and meanings; new codes may appear in a minor release, so a switch over
// details needs a default arm, and a code this binding does not name yet is kept, its String
// "detail N" and Known false. A detail never suggests a retry, and the Failure's Reason and
// span stay usable whatever the code.
type Detail uint32

// The published details, numbered as the core numbers them.
const (
	// DetailBlank is empty input, or only white space.
	DetailBlank Detail = 1 + iota
	// DetailUnexpectedCharacter is a character the position does not allow, where it allows
	// several things (a letter in a numeral, an undeclared symbol).
	DetailUnexpectedCharacter
	// DetailDigitExpected is a numeral or digit field under way, or a fixed position needing a
	// digit, meeting another character or the end ("0xG", "1e", "2026-0x-07").
	DetailDigitExpected
	// DetailDelimiterExpected is a required mark ('-', ':', 'T', '}', "0x", ';', a UUID
	// hyphen) missing or different.
	DetailDelimiterExpected
	// DetailTrailingText is a complete value followed by more text.
	DetailTrailingText
	// DetailNoDigits is a numeral absent entirely: only a sign, a currency or percent affix,
	// a bare radix prefix, or a protobuf duration's bare 's'.
	DetailNoDigits
	// DetailSignMisplaced is a sign where none may stand: a second sign, inside parentheses,
	// mid-token, leading plus trailing.
	DetailSignMisplaced
	// DetailParenthesesUnbalanced is '(' without ')', or "( )".
	DetailParenthesesUnbalanced
	// DetailMarkRepeated is a once-only mark appearing twice (a second decimal separator).
	DetailMarkRepeated
	// DetailGroupMisplaced is a group separator not between digits, or an empty group.
	DetailGroupMisplaced
	// DetailGroupSize is a group its GroupingPolicy refuses: the wrong digit count, or a
	// first group that starts with a zero.
	DetailGroupSize
	// DetailGroupInFraction is grouping after the decimal separator or in an exponent, under
	// every GroupingPolicy.
	DetailGroupInFraction
	// DetailAmbiguousSeparator is SeparatorDetect unable to tell '.' from ','.
	DetailAmbiguousSeparator
	// DetailFractionNotAllowed is a fraction where the type is whole.
	DetailFractionNotAllowed
	// DetailFieldWidth is a delimited field with the wrong number of digits.
	DetailFieldWidth
	// DetailWrongLength is a fixed-shape token of the wrong total length (UUID, ISO date).
	DetailWrongLength
	// DetailComponentMissing is a required part absent (a zone, seconds, a duration unit).
	DetailComponentMissing
	// DetailNotRecognized is none of the door's accepted words or values (a boolean word, a
	// month name the vocabulary lacks).
	DetailNotRecognized
	// DetailUnknownUnit is an unknown or misplaced duration unit.
	DetailUnknownUnit
	// DetailInvalidUTF8 is bytes at the failure site that do not decode as UTF-8.
	DetailInvalidUTF8
	// DetailTooLong is input past an implementation limit (a 256-byte numeral).
	DetailTooLong
	// DetailNotFinite is a non-finite value: a NaN or infinity spelling, or a float64 one.
	DetailNotFinite
	// DetailNotWhole is a float64 with a fraction at a whole-number door.
	DetailNotWhole
	// DetailNegativeNotAllowed is a negative where the type has none (unsigned, a serial).
	DetailNegativeNotAllowed
	// DetailTooLarge is a magnitude past what the target type represents.
	DetailTooLarge
	// DetailTooSmall is a value below a signed type's minimum.
	DetailTooSmall
	// DetailTooPrecise is more fractional digits than the type holds (a scale past 28).
	DetailTooPrecise
	// DetailDateFieldOutOfRange is year 0, month 13, a day past the month's end.
	DetailDateFieldOutOfRange
	// DetailTimeFieldOutOfRange is an hour, minute, second or offset out of range.
	DetailTimeFieldOutOfRange
	// DetailOutsideWindow is a representable instant or duration past the supported window
	// (years 0001 to 9999, ±10,000 years).
	DetailOutsideWindow
	// DetailNonexistentDate is a day that never existed: Excel's phantom 1900-02-29.
	DetailNonexistentDate
	// DetailNotAScalar is a code point past U+10FFFF, or a surrogate.
	DetailNotAScalar
)

var detailNames = [...]string{
	"none", "blank", "unexpected_character", "digit_expected", "delimiter_expected",
	"trailing_text", "no_digits", "sign_misplaced", "parentheses_unbalanced", "mark_repeated",
	"group_misplaced", "group_size", "group_in_fraction", "ambiguous_separator",
	"fraction_not_allowed", "field_width", "wrong_length", "component_missing",
	"not_recognized", "unknown_unit", "invalid_utf8", "too_long", "not_finite", "not_whole",
	"negative_not_allowed", "too_large", "too_small", "too_precise", "date_field_out_of_range",
	"time_field_out_of_range", "outside_window", "nonexistent_date", "not_a_scalar",
}

// String is the core's name for the detail, in snake case ("group_size"); "none" for 0, which
// no failure carries; "detail N" for a code this binding does not name.
func (d Detail) String() string {
	if uint64(d) < uint64(len(detailNames)) {
		return detailNames[d]
	}
	return fmt.Sprintf("detail %d", uint32(d))
}

// Known reports whether this binding names the code: 1 through 32.
func (d Detail) Known() bool { return d >= DetailBlank && d <= DetailNotAScalar }

// Failure is a detailed door's failure case: the Fault's reason and byte span, and the Detail
// naming what failed. Every 1.0 door's *Fault is the same parse without the detail
// (Failure.Fault). Build one with field names: a later release may add a field.
type Failure struct {
	_      struct{}
	Reason CastFailure
	Detail Detail
	Offset int
	Length int
}

// Error renders the failure as "hypercast: {reason} ({detail}) at byte {start}..{end}". A nil
// *Failure renders as "<nil>", as a nil *Fault does.
func (f *Failure) Error() string {
	if f == nil {
		return "<nil>"
	}
	return fmt.Sprintf("hypercast: %s (%s) at byte %d..%d", f.Reason, f.Detail, f.Offset, f.Offset+f.Length)
}

// Fault is the failure as the 1.0 doors report it: the reason and span, without the detail.
func (f *Failure) Fault() *Fault {
	return &Fault{Reason: f.Reason, Offset: f.Offset, Length: f.Length}
}

// DetailedRejection is why a detailed checked door gave no value: a *Failure (it did not
// parse) or a *Violation (it parsed and broke a declared rule), as Rejection is for the plain
// checked doors. Nil is success. Sealed: nothing outside this package implements it, so a type
// switch over the two needs no default arm.
type DetailedRejection interface {
	error
	detailedRejection()
}

func (*Failure) detailedRejection()   {}
func (*Violation) detailedRejection() {}

func failedDetailed(code int32, fault *rawFault) *Failure {
	if code == -1 {
		panic("hypercast: the core reported a contract violation — a binding bug, please report it")
	}
	f, ok := FailureFromCode(uint32(code), fault.Offset, fault.Length, fault.Detail)
	if !ok {
		panic(fmt.Sprintf("hypercast: the core returned unknown verdict code %d", code))
	}
	return f
}

// OptionalDetailed presents a detailed door's verdict optionally, as Optional does a plain
// one's: an Empty failure is absent, (nil, nil); a value is a pointer to it; every other
// failure flows through.
func OptionalDetailed[V any](value V, failure *Failure) (*V, *Failure) {
	if failure != nil {
		if failure.Reason == Empty {
			return nil, nil
		}
		return nil, failure
	}
	return &value, nil
}

// OptionalCheckedDetailed presents a detailed checked door's verdict optionally, as
// OptionalChecked does a plain one's.
func OptionalCheckedDetailed[V any](checked Checked[V], rejection DetailedRejection) (*Checked[V], DetailedRejection) {
	if rejection != nil {
		if failure, ok := rejection.(*Failure); ok && failure.Reason == Empty {
			return nil, nil
		}
		return nil, rejection
	}
	return &checked, nil
}

// BoolDetailed is Bool, reporting a failure's detail.
func BoolDetailed[T Text](text T) (bool, *Failure) {
	r := textCall(doorBool, text)
	if r.code != 0 {
		return false, failedDetailed(r.code, &r.fault)
	}
	return read[uint8](&r) != 0, nil
}

// CharDetailed is Char, reporting a failure's detail.
func CharDetailed[T Text](text T) (rune, *Failure) {
	r := textCall(doorChar, text)
	if r.code != 0 {
		return 0, failedDetailed(r.code, &r.fault)
	}
	return rune(read[uint32](&r)), nil
}

func numericDetailed[T Text, V any](door uint32, text T, format NumFormat) (V, *Failure) {
	r := numericCall(door, text, format)
	if r.code != 0 {
		var zero V
		return zero, failedDetailed(r.code, &r.fault)
	}
	return read[V](&r), nil
}

// I8Detailed is I8, reporting a failure's detail.
func I8Detailed[T Text](text T, format NumFormat) (int8, *Failure) {
	return numericDetailed[T, int8](doorI8, text, format)
}

// I16Detailed is I16, reporting a failure's detail.
func I16Detailed[T Text](text T, format NumFormat) (int16, *Failure) {
	return numericDetailed[T, int16](doorI16, text, format)
}

// I32Detailed is I32, reporting a failure's detail.
func I32Detailed[T Text](text T, format NumFormat) (int32, *Failure) {
	return numericDetailed[T, int32](doorI32, text, format)
}

// I64Detailed is I64, reporting a failure's detail.
func I64Detailed[T Text](text T, format NumFormat) (int64, *Failure) {
	return numericDetailed[T, int64](doorI64, text, format)
}

// U8Detailed is U8, reporting a failure's detail.
func U8Detailed[T Text](text T, format NumFormat) (uint8, *Failure) {
	return numericDetailed[T, uint8](doorU8, text, format)
}

// U16Detailed is U16, reporting a failure's detail.
func U16Detailed[T Text](text T, format NumFormat) (uint16, *Failure) {
	return numericDetailed[T, uint16](doorU16, text, format)
}

// U32Detailed is U32, reporting a failure's detail.
func U32Detailed[T Text](text T, format NumFormat) (uint32, *Failure) {
	return numericDetailed[T, uint32](doorU32, text, format)
}

// U64Detailed is U64, reporting a failure's detail.
func U64Detailed[T Text](text T, format NumFormat) (uint64, *Failure) {
	return numericDetailed[T, uint64](doorU64, text, format)
}

// F32Detailed is F32, reporting a failure's detail.
func F32Detailed[T Text](text T, format NumFormat) (float32, *Failure) {
	return numericDetailed[T, float32](doorF32, text, format)
}

// F64Detailed is F64, reporting a failure's detail.
func F64Detailed[T Text](text T, format NumFormat) (float64, *Failure) {
	return numericDetailed[T, float64](doorF64, text, format)
}

// ExactDetailed is Exact, reporting a failure's detail.
func ExactDetailed[T Text](text T, format NumFormat) (Decimal, *Failure) {
	out, failure := numericDetailed[T, rawDecimal](doorDecimal, text, format)
	if failure != nil {
		return Decimal{}, failure
	}
	return Decimal{Lo: out.Lo, Hi: out.Hi, Scale: out.Scale, Negative: out.Negative != 0}, nil
}

// NumericDetailed is Numeric, reporting a failure's detail: V's own detailed door.
func NumericDetailed[V Number, T Text](text T, format NumFormat) (V, *Failure) {
	var out V
	var failure *Failure
	switch p := any(&out).(type) {
	case *int8:
		*p, failure = I8Detailed(text, format)
	case *int16:
		*p, failure = I16Detailed(text, format)
	case *int32:
		*p, failure = I32Detailed(text, format)
	case *int64:
		*p, failure = I64Detailed(text, format)
	case *uint8:
		*p, failure = U8Detailed(text, format)
	case *uint16:
		*p, failure = U16Detailed(text, format)
	case *uint32:
		*p, failure = U32Detailed(text, format)
	case *uint64:
		*p, failure = U64Detailed(text, format)
	case *float32:
		*p, failure = F32Detailed(text, format)
	case *float64:
		*p, failure = F64Detailed(text, format)
	case *Decimal:
		*p, failure = ExactDetailed(text, format)
	default:
		panic("unreachable: Number admits no other type")
	}
	if failure != nil {
		var zero V
		return zero, failure
	}
	return out, nil
}

// ScalarDetailed is Scalar, reporting a failure's detail: V's own detailed door, with no date
// options at the Timestamp and DateOnly doors.
func ScalarDetailed[V ScalarTarget, T Text](text T, format NumFormat) (V, *Failure) {
	var out V
	var failure *Failure
	switch p := any(&out).(type) {
	case *bool:
		*p, failure = BoolDetailed(text)
	case *uuid.UUID:
		*p, failure = UuidDetailed(text)
	case *time.Time:
		*p, failure = TimestampDetailed(text, DateOptions{})
	case *Date:
		*p, failure = DateOnlyDetailed(text, DateOptions{})
	case *Duration:
		*p, failure = SpanDetailed(text)
	default:
		return numericScalarDetailed[V](text, format)
	}
	if failure != nil {
		var zero V
		return zero, failure
	}
	return out, nil
}

// numericScalarDetailed is ScalarDetailed for the Number targets.
func numericScalarDetailed[V ScalarTarget, T Text](text T, format NumFormat) (V, *Failure) {
	var out V
	var failure *Failure
	switch p := any(&out).(type) {
	case *int8:
		*p, failure = I8Detailed(text, format)
	case *int16:
		*p, failure = I16Detailed(text, format)
	case *int32:
		*p, failure = I32Detailed(text, format)
	case *int64:
		*p, failure = I64Detailed(text, format)
	case *uint8:
		*p, failure = U8Detailed(text, format)
	case *uint16:
		*p, failure = U16Detailed(text, format)
	case *uint32:
		*p, failure = U32Detailed(text, format)
	case *uint64:
		*p, failure = U64Detailed(text, format)
	case *float32:
		*p, failure = F32Detailed(text, format)
	case *float64:
		*p, failure = F64Detailed(text, format)
	case *Decimal:
		*p, failure = ExactDetailed(text, format)
	default:
		panic("unreachable: ScalarTarget admits no other type")
	}
	if failure != nil {
		var zero V
		return zero, failure
	}
	return out, nil
}

// UuidDetailed is Uuid, reporting a failure's detail.
func UuidDetailed[T Text](text T) (uuid.UUID, *Failure) {
	r := textCall(doorUuid, text)
	if r.code != 0 {
		return uuid.UUID{}, failedDetailed(r.code, &r.fault)
	}
	return read[uuid.UUID](&r), nil
}

// TimestampDetailed is Timestamp under options, reporting a failure's detail: SpaceSeparator
// reads "2026-01-07 15:04:05Z", HourOffset "2026-01-07T15:04:05+00"; the zero DateOptions
// reads what Timestamp reads. Pivot and Months are not read here.
func TimestampDetailed[T Text](text T, options DateOptions) (time.Time, *Failure) {
	r := instantCall(text, 0, &options)
	if r.code != 0 {
		return time.Time{}, failedDetailed(r.code, &r.fault)
	}
	return read[RawTimestamp](&r).Time(), nil
}

// UnixDetailed is Unix, reporting a failure's detail.
func UnixDetailed[T Text](text T, precision UnixPrecision) (time.Time, *Failure) {
	if precision < Seconds || precision > Nanoseconds {
		panic(fmt.Sprintf("hypercast: undefined UnixPrecision %d", precision))
	}
	r := instantCall(text, precision, nil)
	if r.code != 0 {
		return time.Time{}, failedDetailed(r.code, &r.fault)
	}
	return read[RawTimestamp](&r).Time(), nil
}

// ExcelSerialDetailed is ExcelSerial, reporting a failure's detail.
func ExcelSerialDetailed[T Text](text T, epoch ExcelEpoch) (time.Time, *Failure) {
	if epoch != Excel1900 && epoch != Excel1904 {
		panic(fmt.Sprintf("hypercast: undefined ExcelEpoch %d", epoch))
	}
	r := excelSerialCall(text, epoch)
	if r.code != 0 {
		return time.Time{}, failedDetailed(r.code, &r.fault)
	}
	return read[RawTimestamp](&r).Time(), nil
}

// DateOnlyDetailed is DateOnly under options, reporting a failure's detail: CompactDate reads
// "20260107"; the zero DateOptions reads what DateOnly reads. Pivot and Months are not read
// here.
func DateOnlyDetailed[T Text](text T, options DateOptions) (Date, *Failure) {
	r := dateCall(doorDate, 0, text, nil, &options)
	if r.code != 0 {
		return Date{}, failedDetailed(r.code, &r.fault)
	}
	return read[RawDate](&r).Date(), nil
}

// DateOnlyOrderedDetailed is DateOnlyOrdered under options, reporting a failure's detail: a
// Pivot reads a two-digit year ("07/01/26"), and Months a month name in the month field the
// order selects ("7-Jan-2026"); the zero DateOptions reads what DateOnlyOrdered reads.
func DateOnlyOrderedDetailed[T Text](text T, order DateOrder, options DateOptions) (Date, *Failure) {
	if order < YearMonthDay || order > DayMonthYear {
		panic(fmt.Sprintf("hypercast: undefined DateOrder %d", order))
	}
	r := dateCall(doorDateOrdered, uint32(order), text, nil, &options)
	if r.code != 0 {
		return Date{}, failedDetailed(r.code, &r.fault)
	}
	return read[RawDate](&r).Date(), nil
}

// DateTimeDetailed is DateTimeWith under options, reporting a failure's detail: the date part
// reads Pivot and Months as DateOnlyOrderedDetailed does ("07-Jan-26 3:04 PM"). A format
// declaring no designators reads what DateTime reads, and the zero DateOptions adds nothing.
func DateTimeDetailed[T Text](text T, format DateFormat, options DateOptions) (CivilDateTime, *Failure) {
	if format.Order < YearMonthDay || format.Order > DayMonthYear {
		panic(fmt.Sprintf("hypercast: undefined DateOrder %d", format.Order))
	}
	var periods RawDayPeriods
	format.raw(&periods)
	r := dateCall(doorDateTime, uint32(format.Order), text, &periods, &options)
	if r.code != 0 {
		return CivilDateTime{}, failedDetailed(r.code, &r.fault)
	}
	return read[RawCivil](&r).CivilDateTime(), nil
}

// TimeOfDayDetailed is TimeOfDay, reporting a failure's detail.
func TimeOfDayDetailed[T Text](text T) (time.Duration, *Failure) {
	r := textCall(doorTime, text)
	if r.code != 0 {
		return 0, failedDetailed(r.code, &r.fault)
	}
	return time.Duration(read[uint64](&r)), nil
}

// SpanDetailed is Span, reporting a failure's detail.
func SpanDetailed[T Text](text T) (Duration, *Failure) {
	r := textCall(doorDuration, text)
	if r.code != 0 {
		return Duration{}, failedDetailed(r.code, &r.fault)
	}
	out := read[RawTimestamp](&r)
	return Duration{Seconds: out.Seconds, Nanos: out.Nanos}, nil
}

// ExactFromFloat64Detailed is ExactFromFloat64, reporting a failure's detail.
func ExactFromFloat64Detailed(value float64) (Decimal, *Failure) {
	r := callDetailedFromF64(doorDecimal, 0, value)
	if r.code != 0 {
		return Decimal{}, failedDetailed(r.code, &r.fault)
	}
	out := read[rawDecimal](&r)
	return Decimal{Lo: out.Lo, Hi: out.Hi, Scale: out.Scale, Negative: out.Negative != 0}, nil
}

// ExcelSerialFromFloat64Detailed is ExcelSerialFromFloat64, reporting a failure's detail.
func ExcelSerialFromFloat64Detailed(value float64, epoch ExcelEpoch) (CivilDateTime, *Failure) {
	if epoch != Excel1900 && epoch != Excel1904 {
		panic(fmt.Sprintf("hypercast: undefined ExcelEpoch %d", epoch))
	}
	r := callDetailedFromF64(doorExcelSerial, uint32(epoch), value)
	if r.code != 0 {
		return CivilDateTime{}, failedDetailed(r.code, &r.fault)
	}
	return read[RawCivil](&r).CivilDateTime(), nil
}

// ExcelTimeDetailed is ExcelTime, reporting a failure's detail.
func ExcelTimeDetailed(value float64) (time.Duration, *Failure) {
	r := callDetailedFromF64(doorTime, 0, value)
	if r.code != 0 {
		return 0, failedDetailed(r.code, &r.fault)
	}
	return time.Duration(read[uint64](&r)), nil
}

// ExcelDurationDetailed is ExcelDuration, reporting a failure's detail.
func ExcelDurationDetailed(value float64) (Duration, *Failure) {
	r := callDetailedFromF64(doorDuration, 0, value)
	if r.code != 0 {
		return Duration{}, failedDetailed(r.code, &r.fault)
	}
	out := read[RawTimestamp](&r)
	return Duration{Seconds: out.Seconds, Nanos: out.Nanos}, nil
}

// StringDetailed is String, reporting a failure's detail.
func StringDetailed[T Text](text T) (string, *Failure) {
	r := textCall(doorText, text)
	if r.code != 0 {
		return "", failedDetailed(r.code, &r.fault)
	}
	return slice(text, read[RawSpan](&r)), nil
}

// rejectedDetailed is the DetailedRejection a nonzero checked code names.
func rejectedDetailed(r *result, c RawCheck) DetailedRejection {
	if r.code != 4 {
		return failedDetailed(r.code, &r.fault)
	}
	return violated(r, c)
}

// CheckDetailed is Check, reporting a parse failure's detail.
func CheckDetailed[V Number, T Text](text T, format NumFormat, constraints Constraints[V]) (Checked[V], DetailedRejection) {
	var raw RawNumFormat
	format.raw(&raw)
	ptr, length := textPtr(text)
	r, c := callChecked(doorOf[V](), ptr, length, &raw, constraints.declaration())
	if r.code != 0 {
		return Checked[V]{EnumIndex: -1}, rejectedDetailed(&r, c)
	}
	return checkedValue[V](&r, c), nil
}

// CheckFromFloat64Detailed is CheckFromFloat64, reporting a failure's detail.
func CheckFromFloat64Detailed[V Number](value float64, constraints Constraints[V]) (Checked[V], DetailedRejection) {
	r, c := callCheckedFromF64(doorOf[V](), value, constraints.declaration())
	if r.code != 0 {
		return Checked[V]{EnumIndex: -1}, rejectedDetailed(&r, c)
	}
	return checkedValue[V](&r, c), nil
}

// CheckStringDetailed is CheckString, reporting a parse failure's detail.
func CheckStringDetailed[T Text](text T, constraints Constraints[string]) (Checked[string], DetailedRejection) {
	ptr, length := textPtr(text)
	d := constraints.declaration()
	r, c := callChecked(doorText, ptr, length, nil, d)
	if r.code != 0 {
		return Checked[string]{EnumIndex: -1}, rejectedDetailed(&r, c)
	}
	return checkedText(text, d, &r, c), nil
}
