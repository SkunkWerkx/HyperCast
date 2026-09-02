// Package hypercast provides allocation-lean scalar casts — booleans, numerics, UUIDs,
// temporals — calling directly into the native libhypercast shared library. Every door
// returns Go's own union idiom: (value, *Fault), where a nil fault is the success case and
// a non-nil one carries the closed reason plus the offending byte span. Never an error for
// bad data in the exception sense — *Fault implements error for composition, but the doors
// never panic on input; a panic here means a caller bug (a malformed NumFormat), never data.
//
// Door names mirror the native ABI (I32, F64, Timestamp, ...) so the polyglot surface reads
// identically across bindings. Doors are generic over string | []byte — both cross
// zero-copy (the core only reads).
//
// Go-flavored fidelity, stated honestly both ways: time.Time carries full nanoseconds
// across the whole 0001–9999 window, and time-of-day comes back as a time.Duration since
// midnight, also nanosecond-exact. But time.Duration's int64-nanosecond ceiling (±292
// years) sits far below the core's ±10,000-year duration window, so the duration door
// returns the protobuf pair (Duration{Seconds, Nanos}) with a checked AsDuration()
// converter rather than silently wrapping.
package hypercast

import (
	"fmt"
	"runtime"
	"time"
	"unsafe"

	"github.com/google/uuid"
)

// CastFailure is the closed set of reasons a cast can fail — the native core's verdict
// codes, verbatim.
type CastFailure int32

const (
	// Empty means required input was empty or whitespace.
	Empty CastFailure = 1
	// Malformed means input was present but not recognizable as the target type.
	Malformed CastFailure = 2
	// OutOfRange means well-formed but outside the target's range — "256" for a U8,
	// 1e400 for an F64.
	OutOfRange CastFailure = 3
)

func (f CastFailure) String() string {
	switch f {
	case Empty:
		return "empty"
	case Malformed:
		return "malformed"
	case OutOfRange:
		return "out of range"
	default:
		return fmt.Sprintf("CastFailure(%d)", int32(f))
	}
}

// Fault is the failure case of a cast: a closed reason plus the offending span, as byte
// offsets into the input (identical to what the door received — string and []byte inputs
// are both raw UTF-8 here). Implements error so verdicts compose with Go's error plumbing,
// but nothing is captured or formatted until Error() is actually called.
type Fault struct {
	Reason CastFailure
	Offset int
	Length int
}

func (f *Fault) Error() string {
	return fmt.Sprintf("hypercast: %s at byte %d..%d", f.Reason, f.Offset, f.Offset+f.Length)
}

// Text constrains the door inputs: Go's two spellings of raw UTF-8.
type Text interface {
	string | []byte
}

func textPtr[T Text](text T) (unsafe.Pointer, uintptr) {
	switch v := any(text).(type) {
	case string:
		if len(v) == 0 {
			return nil, 0
		}
		return unsafe.Pointer(unsafe.StringData(v)), uintptr(len(v))
	case []byte:
		if len(v) == 0 {
			return nil, 0
		}
		return unsafe.Pointer(unsafe.SliceData(v)), uintptr(len(v))
	default:
		panic("unreachable")
	}
}

type rawFault struct {
	Offset uint32
	Length uint32
}

// The verdict as the backends hand it back, by value: 16 bytes of out-value (8-aligned,
// the widest door's size — every door reads its own prefix through a typed view), the
// native code, and the fault span. Returning this rather than filling caller pointers is
// what keeps the cgo backend's doors allocation-free — see backend_cgo.go.
type result struct {
	out   [2]uint64
	code  int32
	fault rawFault
}

// A typed view of the out-value; V is one of the repr(C) shapes the core writes.
func read[V any](r *result) V {
	return *(*V)(unsafe.Pointer(&r.out))
}

type rawNumFormat struct {
	DecimalSep uint32
	GroupSep   uint32
	Flags      uint32
}

type rawTimestamp struct {
	Seconds int64
	Nanos   int32
	_       int32 // tail padding, matching the repr(C) layout's 16-byte size
}

type rawDate struct {
	Year  uint16
	Month uint8
	Day   uint8
}

type rawCivil struct {
	Year  uint16
	Month uint8
	Day   uint8
	_     [4]byte // tail padding before the u64, matching the repr(C) layout
	Nanos uint64
}

// NumStyles are the lenience flags of NumFormat — bit-for-bit the native core's flags.
type NumStyles uint32

const (
	// Grouping permits the group separator between digits (sizes not validated — between
	// digits is the rule).
	Grouping NumStyles = 1 << iota
	// Parentheses permits accounting negation: (1,234) is -1234.
	Parentheses
	// Exponent permits exponent notation. Integer doors reject a negative exponent.
	Exponent
	// RadixPrefixes permits 0x/&H/0b two's-complement prefixes (0xFF is -1 for an I8).
	RadixPrefixes
	// Percent permits a trailing %, dividing by 100. Real doors only.
	Percent
	// SeparatorDetect resolves the './,' roles per input from structure instead of the
	// declared separators (which are ignored while this flag is set). Detection, not
	// sniffing: a repeated separator is grouping (1.234.567,89); with both present the
	// rightmost is the decimal; a single separator with a non-3-digit right run is the
	// decimal (3,1415); with exactly 3 digits right, only a 0 integer part proves decimal
	// (0,785). Genuinely ambiguous input (12.185, 1,000) is Malformed at the separator,
	// never guessed.
	SeparatorDetect NumStyles = 1 << 5

	// AllStyles is every lenience on.
	AllStyles = Grouping | Parentheses | Exponent | RadixPrefixes | Percent
)

// NumFormat is the caller-declared numeric notation for the integer and real doors. The
// core carries no culture data — a call site parsing culture-sensitive text declares its
// format out loud (Invariant, or a literal); there is no default, the same stance every
// binding in this repo takes. Equal separators are a caller bug and panic, never a verdict.
type NumFormat struct {
	DecimalSep rune
	GroupSep   rune
	Styles     NumStyles
}

// Invariant is the invariant profile — '.' decimal, ',' grouping, every lenience on.
var Invariant = NumFormat{DecimalSep: '.', GroupSep: ',', Styles: AllStyles}

// Detect is the detection profile — every lenience on, './,' roles resolved per input by
// SeparatorDetect's structural rules.
var Detect = NumFormat{DecimalSep: '.', GroupSep: ',', Styles: AllStyles | SeparatorDetect}

func (f NumFormat) raw() rawNumFormat {
	if f.DecimalSep == f.GroupSep {
		panic(fmt.Sprintf("hypercast: decimal and group separators must differ; both are %q", f.DecimalSep))
	}
	return rawNumFormat{DecimalSep: uint32(f.DecimalSep), GroupSep: uint32(f.GroupSep), Flags: uint32(f.Styles)}
}

// UnixPrecision is the declared unit of a Unix-epoch value — no magnitude guessing, ever.
type UnixPrecision uint32

// The declared units a Unix-epoch value can carry — the native core's codes, verbatim.
const (
	Seconds      UnixPrecision = 1
	Milliseconds UnixPrecision = 2
	Microseconds UnixPrecision = 3
	Nanoseconds  UnixPrecision = 4
)

// ExcelEpoch is the date system an Excel serial number is expressed in. Spreadsheets carry
// no marker for this — it is a workbook-level setting — so the caller states it, the same
// way UnixPrecision and DateOrder are declared rather than guessed.
type ExcelEpoch uint32

// The declared Excel date systems — the native core's codes, verbatim.
const (
	// Excel1900 is the Windows default: serial 1 is 1900-01-01, and serial 60 is a
	// February 29th that never existed.
	Excel1900 ExcelEpoch = 1
	// Excel1904 is the legacy Macintosh system, still selectable today: serial 0 is
	// 1904-01-01, with no phantom day anywhere in it.
	Excel1904 ExcelEpoch = 2
)

// DateOrder is the caller-declared field order of a separated calendar date — no
// guessing, ever: "1/7/2026" is January 7th (MonthDayYear, the en-US order) or July 1st
// (DayMonthYear, the en-GB order) only because the caller said which.
type DateOrder uint32

// The declared field orders — the native core's codes, verbatim.
const (
	YearMonthDay DateOrder = 1
	MonthDayYear DateOrder = 2
	DayMonthYear DateOrder = 3
)

// Date is a calendar date with no time or zone — the protobuf google.type.Date fields.
// Go has no standard date-only type; this keeps the core's digits exactly.
type Date struct {
	Year  int
	Month time.Month
	Day   int
}

// CivilDateTime is a wall-clock date and time with no zone — exactly what zone-less text
// like "1/7/2026 3:04 PM" actually names. Deliberately not a time.Time: without a zone
// there is no instant, and inventing one (assuming UTC, say) would be a silent value error
// of up to ±14 hours. Fuse a zone yourself when you know it:
// time.Date(d.Date.Year, d.Date.Month, d.Date.Day, 0, 0, 0, 0, loc).Add(d.TimeOfDay).
type CivilDateTime struct {
	Date      Date
	TimeOfDay time.Duration
}

// Duration is the protobuf pair — whole seconds plus same-signed nanoseconds. Returned
// instead of time.Duration because the core's ±10,000-year window exceeds time.Duration's
// int64-nanosecond ceiling (±292 years); AsDuration converts when the value fits.
type Duration struct {
	Seconds int64
	Nanos   int32
}

// AsDuration converts to time.Duration, reporting false when the value exceeds
// time.Duration's ±292-year representable range.
func (d Duration) AsDuration() (time.Duration, bool) {
	const maxSeconds = int64(1<<63-1) / int64(time.Second)
	if d.Seconds > maxSeconds || d.Seconds < -maxSeconds {
		return 0, false
	}
	return time.Duration(d.Seconds)*time.Second + time.Duration(d.Nanos), true
}

func failed(code int32, fault *rawFault) *Fault {
	if code == -1 {
		panic("hypercast: libhypercast reported a contract violation — a binding bug, please report it")
	}
	return &Fault{Reason: CastFailure(code), Offset: int(fault.Offset), Length: int(fault.Length)}
}

func mustLoad() {
	if err := ensureLoaded(); err != nil {
		panic(err)
	}
}

// Bool casts boolean text: true/false plus the conventions untrusted sources actually send
// (t/f, yes/no, y/n, 1/0, on/off, enabled/disabled, active/inactive, checked/unchecked,
// in/out), ASCII case-insensitive.
func Bool[T Text](text T) (bool, *Fault) {
	mustLoad()
	ptr, length := textPtr(text)
	r := callPlain(symBool, ptr, length)
	runtime.KeepAlive(text)
	if r.code != 0 {
		return false, failed(r.code, &r.fault)
	}
	return read[uint8](&r) != 0, nil
}

func numericDoor[T Text, V any](sym *numericSymbol, text T, format NumFormat) (V, *Fault) {
	// sym is an address, not a value: the caller evaluates it before mustLoad has
	// resolved the symbols, so the dereference must happen after.
	mustLoad()
	raw := format.raw()
	ptr, length := textPtr(text)
	r := callNumeric(*sym, ptr, length, raw)
	runtime.KeepAlive(text)
	if r.code != 0 {
		var zero V
		return zero, failed(r.code, &r.fault)
	}
	return read[V](&r), nil
}

// I8 casts integer text under the declared format: the type's own range, declared
// grouping, accounting parentheses, non-negative exponent (1e3 is 1000; a decimal point is
// never accepted), and 0x/&H/0b two's-complement radix prefixes (0xFF is -1). The other
// integer doors share these rules at their own widths.
func I8[T Text](text T, format NumFormat) (int8, *Fault) {
	return numericDoor[T, int8](&symI8, text, format)
}

// I16 casts integer text to int16. Rules as I8.
func I16[T Text](text T, format NumFormat) (int16, *Fault) {
	return numericDoor[T, int16](&symI16, text, format)
}

// I32 casts integer text to int32. Rules as I8.
func I32[T Text](text T, format NumFormat) (int32, *Fault) {
	return numericDoor[T, int32](&symI32, text, format)
}

// I64 casts integer text to int64. Rules as I8.
func I64[T Text](text T, format NumFormat) (int64, *Fault) {
	return numericDoor[T, int64](&symI64, text, format)
}

// U8 casts integer text to uint8. Rules as I8.
func U8[T Text](text T, format NumFormat) (uint8, *Fault) {
	return numericDoor[T, uint8](&symU8, text, format)
}

// U16 casts integer text to uint16. Rules as I8.
func U16[T Text](text T, format NumFormat) (uint16, *Fault) {
	return numericDoor[T, uint16](&symU16, text, format)
}

// U32 casts integer text to uint32. Rules as I8.
func U32[T Text](text T, format NumFormat) (uint32, *Fault) {
	return numericDoor[T, uint32](&symU32, text, format)
}

// U64 casts integer text to uint64 — natively unsigned, no widening games. Rules as I8.
func U64[T Text](text T, format NumFormat) (uint64, *Fault) {
	return numericDoor[T, uint64](&symU64, text, format)
}

// F32 casts real text under the declared format: finite values only (NaN/Infinity literals
// are Malformed, overflow to infinity is OutOfRange), declared separators and grouping,
// accounting parentheses, exponent, and trailing percent (50% is 0.5).
func F32[T Text](text T, format NumFormat) (float32, *Fault) {
	return numericDoor[T, float32](&symF32, text, format)
}

// F64 casts real text to float64. Rules as F32.
func F64[T Text](text T, format NumFormat) (float64, *Fault) {
	return numericDoor[T, float64](&symF64, text, format)
}

// Uuid casts UUID text — all five .NET Guid formats (D/N/B/P/X) plus
// urn:uuid:/GUID:/UUID: prefixes — to a uuid.UUID (RFC 9562 byte order, which is exactly
// uuid.UUID's own layout).
func Uuid[T Text](text T) (uuid.UUID, *Fault) {
	mustLoad()
	ptr, length := textPtr(text)
	r := callPlain(symUuid, ptr, length)
	runtime.KeepAlive(text)
	if r.code != 0 {
		return uuid.UUID{}, failed(r.code, &r.fault)
	}
	return read[uuid.UUID](&r), nil
}

// instantDoor serves both instant-shaped doors: a zero precision means the RFC 3339 door
// (symTimestamp, read after mustLoad has resolved it), anything else the Unix-epoch door.
func instantDoor[T Text](text T, precision UnixPrecision) (time.Time, *Fault) {
	mustLoad()
	ptr, length := textPtr(text)
	var r result
	if precision == 0 {
		r = callPlain(symTimestamp, ptr, length)
	} else {
		r = callUnix(ptr, length, uint32(precision))
	}
	runtime.KeepAlive(text)
	if r.code != 0 {
		return time.Time{}, failed(r.code, &r.fault)
	}
	out := read[rawTimestamp](&r)
	return time.Unix(out.Seconds, int64(out.Nanos)).UTC(), nil
}

// Timestamp casts an RFC 3339 instant — zone mandatory — to a UTC time.Time at full
// nanosecond fidelity across the whole 0001–9999 window.
func Timestamp[T Text](text T) (time.Time, *Fault) {
	return instantDoor(text, 0)
}

// Unix casts an integer Unix-epoch value under a caller-declared unit to a UTC time.Time.
// An undefined precision is a caller bug and panics, never a verdict.
func Unix[T Text](text T, precision UnixPrecision) (time.Time, *Fault) {
	if precision < Seconds || precision > Nanoseconds {
		panic(fmt.Sprintf("hypercast: undefined UnixPrecision %d", precision))
	}
	return instantDoor(text, precision)
}

// ExcelSerial casts an Excel date serial under a caller-declared ExcelEpoch to a UTC
// time.Time. The whole part counts days from the system's own day zero and the fraction is
// the time of day, so "45292.75" is 2024-01-01T18:00:00Z. A spreadsheet cell carries no
// zone and none is invented.
//
// The 1900 system contains a day that never existed: serial 60 is 1900-02-29, kept
// deliberately because Lotus 1-2-3 wrongly treated 1900 as a leap year and Excel copied the
// bug for file compatibility. It is Malformed here — the same verdict DateOnly gives the
// text "1900-02-29" — so every serial above it is shifted one day against a naive count,
// which is the arithmetic hand-rolled conversions get wrong.
//
// An undefined epoch is a caller bug and panics, never a verdict.
func ExcelSerial[T Text](text T, epoch ExcelEpoch) (time.Time, *Fault) {
	if epoch != Excel1900 && epoch != Excel1904 {
		panic(fmt.Sprintf("hypercast: undefined ExcelEpoch %d", epoch))
	}
	mustLoad()
	ptr, length := textPtr(text)
	r := callExcelSerial(ptr, length, uint32(epoch))
	runtime.KeepAlive(text)
	if r.code != 0 {
		return time.Time{}, failed(r.code, &r.fault)
	}
	out := read[rawTimestamp](&r)
	return time.Unix(out.Seconds, int64(out.Nanos)).UTC(), nil
}

// DateOnly casts a strict ISO 8601 yyyy-MM-dd calendar date.
func DateOnly[T Text](text T) (Date, *Fault) {
	mustLoad()
	ptr, length := textPtr(text)
	r := callPlain(symDate, ptr, length)
	runtime.KeepAlive(text)
	if r.code != 0 {
		return Date{}, failed(r.code, &r.fault)
	}
	out := read[rawDate](&r)
	return Date{Year: int(out.Year), Month: time.Month(out.Month), Day: int(out.Day)}, nil
}

// DateOnlyOrdered casts a separated calendar date — three digit fields joined by one
// consistent separator (/, -, or .) — under the caller-declared DateOrder. The year field
// is four digits wherever the order puts it (two-digit years mean century guessing, which
// never happens here); an undefined order is a caller bug and panics, never a verdict.
// The strict DateOnly door keeps rejecting every separated form.
func DateOnlyOrdered[T Text](text T, order DateOrder) (Date, *Fault) {
	if order < YearMonthDay || order > DayMonthYear {
		panic(fmt.Sprintf("hypercast: undefined DateOrder %d", order))
	}
	mustLoad()
	ptr, length := textPtr(text)
	r := callDateOrdered(ptr, length, uint32(order))
	runtime.KeepAlive(text)
	if r.code != 0 {
		return Date{}, failed(r.code, &r.fault)
	}
	out := read[rawDate](&r)
	return Date{Year: int(out.Year), Month: time.Month(out.Month), Day: int(out.Day)}, nil
}

// DateTime casts a zone-less civil date-time — the shape untrusted feeds actually send
// ("1/7/2026 3:04 PM", "2026-01-07 15:04:05") — under the caller-declared DateOrder. The
// date part follows DateOnlyOrdered's grammar; the optional time part (one space or 'T'
// after the date) is 24-hour h:mm[:ss[.f]] or 12-hour with an AM/PM marker; absent, the
// time is midnight. No zone is read and none is invented — Timestamp stays the strict
// RFC 3339 instant door. An undefined order is a caller bug and panics, never a verdict.
func DateTime[T Text](text T, order DateOrder) (CivilDateTime, *Fault) {
	if order < YearMonthDay || order > DayMonthYear {
		panic(fmt.Sprintf("hypercast: undefined DateOrder %d", order))
	}
	mustLoad()
	ptr, length := textPtr(text)
	r := callDateTime(ptr, length, uint32(order))
	runtime.KeepAlive(text)
	if r.code != 0 {
		return CivilDateTime{}, failed(r.code, &r.fault)
	}
	out := read[rawCivil](&r)
	return CivilDateTime{
		Date:      Date{Year: int(out.Year), Month: time.Month(out.Month), Day: int(out.Day)},
		TimeOfDay: time.Duration(out.Nanos),
	}, nil
}

// TimeOfDay casts an ISO 24-hour time-of-day to a time.Duration since midnight —
// nanosecond-exact, and comfortably inside time.Duration's range.
func TimeOfDay[T Text](text T) (time.Duration, *Fault) {
	mustLoad()
	ptr, length := textPtr(text)
	r := callPlain(symTime, ptr, length)
	runtime.KeepAlive(text)
	if r.code != 0 {
		return 0, failed(r.code, &r.fault)
	}
	return time.Duration(read[uint64](&r)), nil
}

// Span casts a duration (ISO 8601 fixed components, invariant colon form, or protobuf JSON
// seconds) to the protobuf pair — see Duration for why not time.Duration directly.
func Span[T Text](text T) (Duration, *Fault) {
	mustLoad()
	ptr, length := textPtr(text)
	r := callPlain(symDuration, ptr, length)
	runtime.KeepAlive(text)
	if r.code != 0 {
		return Duration{}, failed(r.code, &r.fault)
	}
	out := read[rawTimestamp](&r)
	return Duration{Seconds: out.Seconds, Nanos: out.Nanos}, nil
}
