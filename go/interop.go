package hypercast

import (
	"fmt"
	"time"
	"unicode/utf8"
	"unsafe"
)

// The native core's C ABI, for a module that carries HyperCast's verdicts across a C ABI of
// its own (HyperTabular does): the layouts the core writes, the conversions every door here
// applies to them, the checked codes of the declared options, and the packed version word.
// A value read out of another library's buffer through these is the value the door of the
// same name would have returned.
//
// Decimal and Duration need no raw twin: each is laid out exactly as the core writes one
// (checked at compile time below), so a buffer of the core's decimals or durations can be
// viewed as a []Decimal or []Duration in place. The core writes a decimal's sign as a 0 or
// 1 byte, which is what a Go bool is.

// CurrencyMaxBytes is the inline capacity of the ABI's currency symbol, in UTF-8 bytes.
const CurrencyMaxBytes = 16

// RawNumFormat is NumFormat as it crosses the ABI — 32 bytes, 4-aligned: the separators as
// code points, the flags, and the currency symbol as CurrencyLen UTF-8 bytes held inline
// (zero-padded; a zero length declares none). NumFormat.Raw builds one.
type RawNumFormat struct {
	DecimalSep  uint32
	GroupSep    uint32
	Flags       uint32
	CurrencyLen uint32
	Currency    [CurrencyMaxBytes]byte
}

// RawTimestamp is an instant as the core writes it — protobuf's Timestamp: whole seconds
// from the Unix epoch and the nanoseconds after them. 16 bytes.
type RawTimestamp struct {
	Seconds int64
	Nanos   int32
	_       int32 // tail padding, matching the repr(C) layout's 16-byte size
}

// Time is the instant at UTC, at full nanosecond fidelity: the Timestamp, Unix and
// ExcelSerial doors' value.
func (r RawTimestamp) Time() time.Time { return time.Unix(r.Seconds, int64(r.Nanos)).UTC() }

// RawDate is a calendar date as the core writes it. 4 bytes.
type RawDate struct {
	Year  uint16
	Month uint8
	Day   uint8
}

// Date is the date: the DateOnly doors' value.
func (r RawDate) Date() Date {
	return Date{Year: int(r.Year), Month: time.Month(r.Month), Day: int(r.Day)}
}

// RawCivil is a zone-less wall clock as the core writes it: a date and the nanoseconds
// since its midnight. 16 bytes.
type RawCivil struct {
	Year  uint16
	Month uint8
	Day   uint8
	_     [4]byte // padding before the u64, matching the repr(C) layout
	Nanos uint64
}

// CivilDateTime is the wall clock: the DateTime doors' value.
func (r RawCivil) CivilDateTime() CivilDateTime {
	return CivilDateTime{
		Date:      Date{Year: int(r.Year), Month: time.Month(r.Month), Day: int(r.Day)},
		TimeOfDay: time.Duration(r.Nanos),
	}
}

// Raw validates the format and lays it out as the core reads it — what every numeric door
// does with its format argument, as an error rather than the doors' panic: equal
// separators, a separator that is no Unicode scalar value, or a currency symbol longer than
// CurrencyMaxBytes, not UTF-8, or carrying an ASCII digit or whitespace.
func (f NumFormat) Raw() (RawNumFormat, error) {
	if f.DecimalSep == f.GroupSep {
		return RawNumFormat{}, fmt.Errorf("decimal and group separators must differ; both are %q", f.DecimalSep)
	}
	if !utf8.ValidRune(f.DecimalSep) || !utf8.ValidRune(f.GroupSep) {
		return RawNumFormat{}, fmt.Errorf("separators %q and %q must be Unicode scalar values", f.DecimalSep, f.GroupSep)
	}
	if len(f.Currency) > CurrencyMaxBytes {
		return RawNumFormat{}, fmt.Errorf("currency symbol %q exceeds %d UTF-8 bytes", f.Currency, CurrencyMaxBytes)
	}
	if !utf8.ValidString(f.Currency) {
		return RawNumFormat{}, fmt.Errorf("currency symbol %q is not valid UTF-8", f.Currency)
	}
	for i := 0; i < len(f.Currency); i++ {
		// The core's rule verbatim: an ASCII digit or ASCII whitespace (space, \t, \n, \f,
		// \r) would collide with the digit scan and the trimming around the symbol.
		switch b := f.Currency[i]; {
		case b >= '0' && b <= '9', b == ' ', b == '\t', b == '\n', b == '\f', b == '\r':
			return RawNumFormat{}, fmt.Errorf("currency symbol %q must not contain an ASCII digit or whitespace", f.Currency)
		}
	}
	raw := RawNumFormat{DecimalSep: uint32(f.DecimalSep), GroupSep: uint32(f.GroupSep), Flags: uint32(f.Styles)}
	raw.CurrencyLen = uint32(copy(raw.Currency[:], f.Currency))
	return raw, nil
}

// Valid reports whether p is a defined precision, the ones a door accepts.
func (p UnixPrecision) Valid() bool { return p >= Seconds && p <= Nanoseconds }

// Valid reports whether o is a defined order, the ones a door accepts.
func (o DateOrder) Valid() bool { return o >= YearMonthDay && o <= DayMonthYear }

// Valid reports whether e is a defined date system, the ones a door accepts.
func (e ExcelEpoch) Valid() bool { return e == Excel1900 || e == Excel1904 }

// UnixPrecisionFromCode is the precision whose ABI discriminant is code (1 seconds … 4
// nanoseconds), and false for any other value.
func UnixPrecisionFromCode(code uint32) (UnixPrecision, bool) {
	p := UnixPrecision(code)
	return p, p.Valid()
}

// DateOrderFromCode is the order whose ABI discriminant is code (1 year-month-day, 2
// month-day-year, 3 day-month-year), and false for any other value.
func DateOrderFromCode(code uint32) (DateOrder, bool) {
	o := DateOrder(code)
	return o, o.Valid()
}

// ExcelEpochFromCode is the date system whose ABI discriminant is code (1 the 1900 system,
// 2 the 1904 system), and false for any other value.
func ExcelEpochFromCode(code uint32) (ExcelEpoch, bool) {
	e := ExcelEpoch(code)
	return e, e.Valid()
}

// ReasonFromCode is the reason whose verdict code is code (1 empty, 2 malformed, 3 out of
// range), and false for any other value — 0, the code for success, included.
func ReasonFromCode(code uint32) (CastFailure, bool) {
	r := CastFailure(code)
	return r, r >= Empty && r <= OutOfRange
}

// FaultFromCode is the fault a nonzero verdict code and its byte span name, and false when
// the code names no reason — a binding bug, not data.
func FaultFromCode(code, offset, length uint32) (*Fault, bool) {
	reason, ok := ReasonFromCode(code)
	if !ok {
		return nil, false
	}
	return &Fault{Reason: reason, Offset: int(offset), Length: int(length)}, true
}

// FormatVersion renders a native library's packed version word — major<<16 | minor<<8 |
// patch, as a *_version() export returns it — as "major.minor.patch".
func FormatVersion(packed uint32) string {
	return fmt.Sprintf("%d.%d.%d", packed>>16, (packed>>8)&0xFF, packed&0xFF)
}

// The raw shapes at the sizes rust/src/abi.rs pins for them, and Decimal and Duration at
// the core's layout, field by field: an array of any other length is another type, so a
// drift on this side fails the build.
var (
	_ [32]byte = [unsafe.Sizeof(RawNumFormat{})]byte{}
	_ [16]byte = [unsafe.Sizeof(RawTimestamp{})]byte{}
	_ [4]byte  = [unsafe.Sizeof(RawDate{})]byte{}
	_ [16]byte = [unsafe.Sizeof(RawCivil{})]byte{}
	_ [8]byte  = [unsafe.Offsetof(RawCivil{}.Nanos)]byte{}

	_ [16]byte = [unsafe.Sizeof(Decimal{})]byte{}
	_ [0]byte  = [unsafe.Offsetof(Decimal{}.Lo)]byte{}
	_ [8]byte  = [unsafe.Offsetof(Decimal{}.Hi)]byte{}
	_ [12]byte = [unsafe.Offsetof(Decimal{}.Scale)]byte{}
	_ [13]byte = [unsafe.Offsetof(Decimal{}.Negative)]byte{}
	_ [16]byte = [unsafe.Sizeof(Duration{})]byte{}
	_ [0]byte  = [unsafe.Offsetof(Duration{}.Seconds)]byte{}
	_ [8]byte  = [unsafe.Offsetof(Duration{}.Nanos)]byte{}
)
