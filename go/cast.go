// Package hypercast provides allocation-lean scalar casts — booleans, numerics, UUIDs,
// temporals — by calling the Rust core compiled as Go source: the core built for
// wasm32-wasip1 and translated by wasm2go into internal/core (core.go has the backend). No
// cgo and no C compiler: it builds for every GOOS and GOARCH Go has, under stock Go and
// TinyGo alike, the browser and WASI included.
//
// Every door returns Go's own union idiom: (value, *Fault), where a nil fault is the success
// case and a non-nil one carries the closed reason plus the offending byte span. Never an
// error for bad data in the exception sense — *Fault implements error for composition, but
// the doors never panic on input; a panic here means a caller bug (a malformed NumFormat or
// an undefined enum value), never data.
//
// A value that parsed can still break a business rule. The checked doors — Check,
// CheckFromFloat64 and CheckString — judge it, in the core, by a door-bound Constraints[V]
// declared with the OpenAPI Schema Object's keywords, and return (Checked[V], Rejection): a
// nil Rejection is success, otherwise it is the door's own *Fault or a *Violation, the keyword
// broken and its byte span.
//
// Door names follow the native ABI's (cast_i32 is I32, cast_timestamp is Timestamp) so the
// polyglot surface reads the same across bindings, except where Go already owns the word:
// Exact and Span return the Decimal and Duration types their ABI names would collide with,
// and DateOnly, DateOnlyOrdered and TimeOfDay say what cast_date, cast_date_ordered and
// cast_time return. Doors are generic over string | []byte — either is copied once into the
// core's memory, and nothing else of it is touched.
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
	"math/big"
	"math/bits"
	"strconv"
	"time"
	"unsafe"

	"github.com/google/uuid"
)

// CastFailure is the closed set of reasons a cast can fail — the native core's verdict
// codes, verbatim.
type CastFailure int32

const (
	// Empty means required input was empty or white space.
	Empty CastFailure = 1
	// Malformed means input was present but not recognizable as the target type.
	Malformed CastFailure = 2
	// OutOfRange means well-formed but outside the target's range — "256" for a U8,
	// 1e400 for an F64.
	OutOfRange CastFailure = 3
)

// String names the reason in lower case — "empty", "malformed", "out of range" — the form
// Fault.Error embeds.
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

// Error renders the verdict as "hypercast: {reason} at byte {start}..{end}", the span as a
// half-open byte range into the input. A nil *Fault — a door's success, printed — renders as
// "<nil>", what fmt prints for one under stock Go after recovering the nil dereference.
// TinyGo's wasm targets cannot recover a panic, so without this the same print trapped there.
func (f *Fault) Error() string {
	if f == nil {
		return "<nil>"
	}
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

// rawFault is a RawFaultDetail as core.go reads it back: the span and the detail, without the
// size word the core never writes.
type rawFault struct {
	Offset uint32
	Length uint32
	Detail uint32
}

// The verdict as core.go hands it back, by value: 16 bytes of out-value (8-aligned, the
// widest door's size — every door reads its own prefix through a typed view), the core's
// code, and the fault's span and detail.
type result struct {
	out   [2]uint64
	code  int32
	fault rawFault
}

// A typed view of the out-value; V is one of the repr(C) shapes the core writes. The bytes
// are the guest's, little-endian, so they are reinterpreted in place only on a
// little-endian host; a big-endian one decodes them field by field (core.go).
func read[V any](r *result) V {
	if bigEndian {
		return decodeLE[V]((*[16]byte)(unsafe.Pointer(&r.out)))
	}
	return *(*V)(unsafe.Pointer(&r.out))
}

// The core's Decimal as it crosses the ABI: lo at 0, hi at 8, scale at 12, negative (0/1)
// at 13, two tail bytes of padding — 16 bytes with 8-byte alignment.
type rawDecimal struct {
	Lo       uint64
	Hi       uint32
	Scale    uint8
	Negative uint8
	_        [2]byte
}

// Each shape above at the size rust/src/abi.rs pins for it, checked at compile time: an
// array of any other length is another type, so a drift on this side fails the build. The
// out-value holds the widest of them.
var (
	_ [12]byte = [unsafe.Sizeof(rawFault{})]byte{}
	_ [16]byte = [unsafe.Sizeof(rawDecimal{})]byte{}
	_ [16]byte = [unsafe.Sizeof(result{}.out)]byte{}
)

// NumStyles are the lenience flags of NumFormat — bit-for-bit the native core's flags.
type NumStyles uint32

const (
	// Grouping permits the group separator between digits, its groups judged by the
	// format's GroupingPolicy.
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

	// CurrencySymbol permits the format's declared Currency once, at either edge of the
	// numeric body: leading, before or after a sign ("$5", "-$5", "$ -5"), or trailing
	// ("5 €", "1.234,50 kr."), with optional white space between symbol and digits;
	// accounting parentheses wrap symbol and digits together ("($5)"). With no Currency
	// declared the flag matches nothing and changes nothing. A Currency declared while
	// this flag is off is Malformed at the symbol. Every integer, real and decimal door.
	CurrencySymbol NumStyles = 1 << 6

	// TrailingSign permits the sign after the digits ("5-", "5 -", "5+", "5 €-", "5- €",
	// "2.5%-"), the way a culture whose negative pattern is "n-" writes it: ASCII '-'/'+'
	// or the declared MinusSign/PlusSign, once, with optional white space before it,
	// and never beside a leading sign or inside accounting parentheses. Not part of
	// AllStyles: a caller sets it from the culture's own negative pattern.
	TrailingSign NumStyles = 1 << 7

	// AllStyles is every lenience on but TrailingSign, which is declared from a culture's
	// negative pattern rather than assumed. It never carries a grouping policy.
	AllStyles = Grouping | Parentheses | Exponent | RadixPrefixes | Percent | CurrencySymbol

	// GroupingPolicyMask is the two bits of Styles, 8 and 9, that hold the GroupingPolicy rather
	// than a lenience: NumFormat.GroupingPolicy reads them, and NewNumFormat, WithGrouping and
	// WithStyles keep the two apart. Bit 8 is retired permanently and never reused, so a Styles
	// holding it names no policy, an old 351 included, and a future policy takes a new bit from
	// bit 10 up. Any bit above them is a caller bug too.
	GroupingPolicyMask NumStyles = 3 << 8

	// definedStyles is every bit a NumFormat's Styles may carry.
	definedStyles = AllStyles | SeparatorDetect | TrailingSign | NumStyles(GroupingIndian)
)

// GroupingPolicy is how the Grouping lenience judges the groups between a numeral's group
// separators — the native core's policy value, verbatim, held in a NumFormat's Styles
// (GroupingPolicyMask). A policy never enables grouping: with Grouping off, a group separator
// stays Malformed under every policy, and an ungrouped numeral is valid under every policy. No
// policy resolves an ambiguous "1,234" under SeparatorDetect.
//
// Shapes neither policy reads ("1,2345,678", "1 23 45 67 89") are Malformed under a declared format,
// and SeparatorDetect infers them instead (it can read "1,2345" as 1.2345): docs/grouping.md
// says why and what adding one takes.
type GroupingPolicy uint32

const (
	// GroupingWestern requires groups of exactly three digits counted leftward from the
	// decimal separator (or the numeral's end), the leftmost group one to three and not
	// starting with a zero: "1,234,567" reads, and "1,23,456", "12,34" and "0,125" are
	// Malformed. Once a numeral uses a group separator every group must conform, and no group
	// separator may follow the decimal separator or sit in an exponent. Native digits and the
	// space-like separators count as digits and separators; a currency amount's group
	// separator follows the same rule. It sets no bit, so a NumFormat literal that declares no
	// policy, the zero value included, and every format stored by 1.0 code is Western.
	GroupingWestern GroupingPolicy = 0
	// GroupingIndian requires the group next to the decimal separator to hold three digits
	// and every group left of it two, the leftmost one or two: "12,34,567" reads and
	// "1,234,567" is Malformed. Only when declared; the rest as GroupingWestern.
	GroupingIndian GroupingPolicy = 2 << 8
)

// String names the policy in lower case — "western", "indian" — and "GroupingPolicy(N)" for
// bits that name none.
func (p GroupingPolicy) String() string {
	switch p {
	case GroupingWestern:
		return "western"
	case GroupingIndian:
		return "indian"
	default:
		return fmt.Sprintf("GroupingPolicy(%d)", uint32(p))
	}
}

// valid reports whether p is one of the two policies.
func (p GroupingPolicy) valid() bool {
	return p == GroupingWestern || p == GroupingIndian
}

// UnmarshalJSON reads a Styles number as encoding/json would, and refuses what no NumFormat may
// hold where it is decoded: the retired bit 8 (a stored 351 or 863) and any bit past the defined
// ones (1119). Bit 9 is Indian, this binding's own encoding of the policy. encoding/gob has no such
// hook for a stream written without one, which every 1.0 stream is, and no other decoder calls this
// one: a bad Styles read from those is refused by the first door (a panic) or by Raw.
func (s *NumStyles) UnmarshalJSON(data []byte) error {
	if string(data) == "null" {
		return nil
	}
	bits, err := strconv.ParseUint(string(data), 10, 32)
	if err != nil {
		return fmt.Errorf("hypercast: Styles %s is not a uint32", data)
	}
	if NumStyles(bits)&retiredPolicyBit != 0 {
		return fmt.Errorf("hypercast: Styles %#x names no grouping policy: bit 8 is retired", bits)
	}
	if NumStyles(bits)&^definedStyles != 0 {
		return fmt.Errorf("hypercast: Styles %#x holds a bit that is not defined", bits)
	}
	*s = NumStyles(bits)
	return nil
}

// retiredPolicyBit is bit 8, refused wherever a Styles is judged.
const retiredPolicyBit NumStyles = 1 << 8

// NumFormat is the caller-declared numeric notation for the integer, real and decimal
// doors. The core carries no culture data, and neither does Go's standard library, so
// there is nothing here to bridge from — a call site parsing culture-sensitive text
// declares its format out loud (Invariant, or a literal filled from whatever culture data
// the application holds); there is no default, the same stance every binding in this repo
// takes. The two number separators are the only required fields; every other field
// declares a further spelling the core reads beside the ASCII one, so a format that
// declares nothing else reads exactly what Invariant reads, and nothing that parsed
// before a symbol was declared stops parsing once it is.
//
// Styles carries the lenience flags and the GroupingPolicy. A literal that sets no policy, the
// zero value included, NewNumFormat, Invariant, Detect and a format stored by 1.0 code
// (encoding/json, encoding/gob) are all GroupingWestern; WithGrouping replaces the policy and
// WithStyles the flags, each keeping the other.
//
// A format the core would refuse is a caller bug and panics, never a verdict: Styles naming
// no grouping policy (holding the retired bit 8) or setting a bit no style or policy defines, a number separator that is empty or longer than SymbolMaxBytes, equal number
// separators (or
// equal separators in force for a currency amount), any symbol that is not valid UTF-8 or
// carries an ASCII digit, a sign, percent symbol or decimal separator (plain or currency)
// carrying white space, a Currency longer than CurrencyMaxBytes or starting or ending in
// white space, or a DigitZero that starts no Unicode digit block. White space is Unicode's
// White_Space property; a group separator may carry it, and ' ', U+00A0 and U+202F declared
// as a group separator stand for each other in the input; so do they inside a Currency,
// which may hold a gap of its own ("F CFA"). NumFormat.Raw reports the same check as an
// error.
type NumFormat struct {
	// DecimalSep is the decimal separator ('.' invariant, ',' in eurozone text): 1 to
	// SymbolMaxBytes bytes of UTF-8 with no ASCII digit and no white space. Required.
	DecimalSep string
	// GroupSep is the digit-group separator (',' invariant, '.', ' ' or ", " elsewhere),
	// likewise, but white space is fine; must differ from DecimalSep. Required.
	GroupSep string
	Styles   NumStyles
	// Currency is the symbol the CurrencySymbol style accepts ("$", "€", "kr.", "CHF") —
	// matched whole at the edges of the numeric body, never inside the digit scan, so a
	// symbol containing a separator character is fine. White space may sit inside it
	// ("F CFA") but not at either edge. Empty declares none.
	Currency string
	// CurrencyDecimalSep is the decimal separator of a currency amount, read in place of
	// DecimalSep when CurrencySymbol is set, Currency is declared and the symbol is found
	// in the text — the way a culture's currency pattern may group and point differently
	// from its number pattern. Empty keeps DecimalSep. Ignored under SeparatorDetect, whose
	// detection governs the whole token.
	CurrencyDecimalSep string
	// CurrencyGroupSep is the group separator of a currency amount, likewise; empty keeps
	// GroupSep.
	CurrencyGroupSep string
	// MinusSign is a further spelling of the minus sign ("−", U+2212, in Estonian and
	// Basque text), read wherever ASCII '-' is: ahead of the digits, before or after a
	// leading currency symbol, and after the digits under TrailingSign. ASCII '-' is always
	// read; empty declares no other.
	MinusSign string
	// PlusSign is a further spelling of the plus sign, likewise beside ASCII '+'.
	PlusSign string
	// Percent is a further spelling of the percent symbol ("٪" in Arabic-script text, "％"
	// fullwidth), read as a trailing '%' is while Percent is set — the real and decimal
	// doors; the integer doors never read a percent. ASCII '%' is always read.
	Percent string
	// DigitZero declares native digits beside ASCII 0–9: the ten code points from this zero
	// up ('٠', U+0660, declares the Arabic-Indic digits; '０', U+FF10, the fullwidth ones),
	// read wherever a digit is — integer part, fraction, exponent; a radix-prefixed (0x/0b)
	// body stays ASCII. 0 declares none. An ASCII zero is a caller bug, as is a code point
	// that starts no Unicode digit block.
	DigitZero rune
}

// Invariant is the invariant profile — '.' decimal, ',' grouping, every lenience on, Western
// grouping, no currency symbol or other spelling declared: NewNumFormat(".", ",", AllStyles),
// and the literal NumFormat{DecimalSep: ".", GroupSep: ",", Styles: AllStyles}.
// Invariant.WithGrouping(GroupingIndian) reads "1,23,456".
var Invariant = NewNumFormat(".", ",", AllStyles)

// Detect is the detection profile — every lenience on, './,' roles resolved per input by
// SeparatorDetect's structural rules, Western grouping.
var Detect = NewNumFormat(".", ",", AllStyles|SeparatorDetect)

// NewNumFormat is a format with the two number separators and the lenience flags, under Western
// grouping; WithGrouping declares GroupingIndian instead, and the further spellings are the
// result's fields to fill. styles carrying a bit of GroupingPolicyMask is a caller bug and panics:
// the policy is declared apart from the flags.
func NewNumFormat(decimalSep, groupSep string, styles NumStyles) NumFormat {
	if styles&GroupingPolicyMask != 0 {
		panic(fmt.Sprintf("hypercast: NewNumFormat styles %#x carry grouping policy bits; declare the policy with WithGrouping", uint32(styles)))
	}
	return NumFormat{DecimalSep: decimalSep, GroupSep: groupSep, Styles: styles}
}

// GroupingPolicy is the format's grouping policy: the bits of Styles under
// GroupingPolicyMask. GroupingWestern for a format that sets none.
func (f NumFormat) GroupingPolicy() GroupingPolicy {
	return GroupingPolicy(f.Styles & GroupingPolicyMask)
}

// WithGrouping is the format under policy, which replaces the current one (never combines
// with it); every lenience flag and symbol is kept. A value other than GroupingWestern and
// GroupingIndian, 256 and 768 included, is a caller bug and panics.
func (f NumFormat) WithGrouping(policy GroupingPolicy) NumFormat {
	if !policy.valid() {
		panic(fmt.Sprintf("hypercast: undefined %v", policy))
	}
	f.Styles = f.Styles&^GroupingPolicyMask | NumStyles(policy)
	return f
}

// WithStyles is the format with styles as its lenience flags, which replace the current ones;
// the grouping policy and every symbol are kept. styles carrying a bit of GroupingPolicyMask is
// a caller bug and panics, as in NewNumFormat.
func (f NumFormat) WithStyles(styles NumStyles) NumFormat {
	if styles&GroupingPolicyMask != 0 {
		panic(fmt.Sprintf("hypercast: WithStyles styles %#x carry grouping policy bits; declare the policy with WithGrouping", uint32(styles)))
	}
	f.Styles = f.Styles&GroupingPolicyMask | styles
	return f
}

// raw is Raw for the doors, which treat a malformed format as the caller bug it is,
// written into the door's own local.
func (f NumFormat) raw(raw *RawNumFormat) {
	if !f.valid() {
		panic("hypercast: " + f.fault().Error())
	}
	f.layout(raw)
}

// Decimal is an exact decimal — a sign, a 96-bit unsigned magnitude, and a base-10 scale —
// the shape .NET's decimal stores natively and every arbitrary-precision decimal builds from
// directly. The value is (-1)^Negative × (Hi·2^64 + Lo) × 10^-Scale, Scale 0..=28. Never
// rounded: the Exact door reports text carrying more precision than that as OutOfRange
// rather than silently approximating. Canonical: exact trailing zeros in the fraction are
// trimmed so the scale is minimal ("1.10", "1.1" and "1.1000" are all magnitude 11, scale
// 1; zero is scale 0), and zero is never negative. Comparable, so two Decimals are ==
// exactly when their values agree.
type Decimal struct {
	// Lo is the low 64 bits of the magnitude.
	Lo uint64
	// Hi is the high 32 bits of the magnitude; the magnitude never exceeds 2^96 - 1.
	Hi uint32
	// Scale is the number of places the magnitude is shifted right, 0..=28.
	Scale uint8
	// Negative is true for a negative value; zero is never negative.
	Negative bool
}

// String renders the canonical text form — "1234.5", "-0.025", "0" — the same string the
// conformance corpus pins as value.
func (d Decimal) String() string {
	// 29 digits cover 2^96 - 1; one more for the leading zero when scale >= digit count,
	// one for the point, one for the sign.
	var buf [40]byte
	n := len(buf)
	hi, lo := uint64(d.Hi), d.Lo
	digits := 0
	for digits == 0 || hi != 0 || lo != 0 {
		// 128-by-64 division by 10 in two halves, the remainder of the high half carried
		// down: bits.Div64 requires its high word below the divisor, and hi < 2^32 < 10·2^64.
		qHi, r := bits.Div64(0, hi, 10)
		qLo, r := bits.Div64(r, lo, 10)
		hi, lo = qHi, qLo
		n--
		buf[n] = byte('0' + r)
		digits++
	}
	scale := int(d.Scale)
	for digits <= scale {
		n--
		buf[n] = '0'
		digits++
	}
	if scale > 0 {
		point := len(buf) - scale
		copy(buf[n-1:point-1], buf[n:point])
		n--
		buf[point-1] = '.'
	}
	if d.Negative {
		n--
		buf[n] = '-'
	}
	return string(buf[n:])
}

// Rat is the exact value as a big.Rat — magnitude over 10^Scale, signed.
func (d Decimal) Rat() *big.Rat {
	magnitude := new(big.Int).SetUint64(uint64(d.Hi))
	magnitude.Lsh(magnitude, 64)
	magnitude.Or(magnitude, new(big.Int).SetUint64(d.Lo))
	if d.Negative {
		magnitude.Neg(magnitude)
	}
	denominator := new(big.Int).Exp(big.NewInt(10), big.NewInt(int64(d.Scale)), nil)
	return new(big.Rat).SetFrac(magnitude, denominator)
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

// DateFormat is the caller-declared notation of a zone-less civil date-time for
// DateTimeWith: the field order, and the words the culture writes for the two halves of
// a 12-hour clock ("vorm."/"nachm.", "오전"/"오후", "a. m."/"p. m."), the way NumFormat
// declares a numeric notation. Declared, never looked up — Go's standard library carries
// no culture data, so an application fills these from its own. The English AM/PM are read
// whatever is declared, ASCII case-insensitively; a declared designator is matched byte
// for byte, after the clock ("3:04 nachm.") or ahead of it ("오후 3:04"), with any run of
// white space (or none) between designator and clock, so "3:04\u202fPM" reads. Empty designators declare nothing, which is exactly
// what DateTime reads.
//
// A designator the core would refuse is a caller bug and panics, never a verdict: longer
// than DesignatorMaxBytes of UTF-8, not valid UTF-8, carrying an ASCII digit, or starting
// or ending in white space (the grammar around it reads that; an inner space, as in
// "a. m.", is fine). DateFormat.Raw reports the same check as an error.
type DateFormat struct {
	Order DateOrder
	// AM is the designator of the first half of the day; empty declares none beside AM.
	AM string
	// PM is the designator of the second half; empty declares none beside PM.
	PM string
}

// raw is Raw for the door, which treats a malformed format as the caller bug it is,
// written into the door's own local.
func (f DateFormat) raw(raw *RawDayPeriods) {
	if !f.valid() {
		panic("hypercast: " + f.fault().Error())
	}
	f.layout(raw)
}

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
		panic("hypercast: the core reported a contract violation — a binding bug, please report it")
	}
	f, ok := FaultFromCode(uint32(code), fault.Offset, fault.Length)
	if !ok {
		panic(fmt.Sprintf("hypercast: the core returned unknown verdict code %d", code))
	}
	return f
}

// Optional presents a plain door's verdict optionally, Go's absent being a nil pointer: an
// Empty fault is absent, (nil, nil); a value is a pointer to it, (&value, nil); every other
// fault flows through untouched, (nil, fault). A door's two results pass straight in:
//
//	age, fault := hypercast.Optional(hypercast.I32(text, hypercast.Invariant))
func Optional[V any](value V, fault *Fault) (*V, *Fault) {
	if fault != nil {
		if fault.Reason == Empty {
			return nil, nil
		}
		return nil, fault
	}
	return &value, nil
}

// Bool casts boolean text: true/false plus the conventions untrusted sources actually send
// (t/f, yes/no, y/n, 1/0, on/off, enabled/disabled, active/inactive, checked/unchecked,
// in/out), ASCII case-insensitive.
func Bool[T Text](text T) (bool, *Fault) {
	r := textCall(doorBool, text)
	if r.code != 0 {
		return false, failed(r.code, &r.fault)
	}
	return read[uint8](&r) != 0, nil
}

// Char casts one character: text that is exactly one UTF-8 scalar is that scalar verbatim,
// before any trimming (" " is a space, "6" is the digit six); otherwise, white space
// trimmed, exactly one declared code point — decimal 65, U+0041, 0x41, &H41, or an HTML
// numeric entity &#65; / &#x41; (the ; required), prefixes ASCII case-insensitive. A
// code point past U+10FFFF or a surrogate is OutOfRange; the rune returned is always a
// valid Unicode scalar.
func Char[T Text](text T) (rune, *Fault) {
	r := textCall(doorChar, text)
	if r.code != 0 {
		return 0, failed(r.code, &r.fault)
	}
	return rune(read[uint32](&r)), nil
}

// textCall is a door that reads its text and nothing else.
func textCall[T Text](door uint32, text T) result {
	ptr, length := textPtr(text)
	return callDetailed(door, 0, ptr, length, nil, nil, nil, "")
}

// numericCall is a numeric door under the declared format, which panics here when the core
// would refuse it.
func numericCall[T Text](door uint32, text T, format NumFormat) result {
	var raw RawNumFormat
	format.raw(&raw)
	ptr, length := textPtr(text)
	return callDetailed(door, 0, ptr, length, &raw, nil, nil, "")
}

func numericDoor[T Text, V any](door uint32, text T, format NumFormat) (V, *Fault) {
	r := numericCall(door, text, format)
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
	return numericDoor[T, int8](doorI8, text, format)
}

// I16 casts integer text to int16. Rules as I8.
func I16[T Text](text T, format NumFormat) (int16, *Fault) {
	return numericDoor[T, int16](doorI16, text, format)
}

// I32 casts integer text to int32. Rules as I8.
func I32[T Text](text T, format NumFormat) (int32, *Fault) {
	return numericDoor[T, int32](doorI32, text, format)
}

// I64 casts integer text to int64. Rules as I8.
func I64[T Text](text T, format NumFormat) (int64, *Fault) {
	return numericDoor[T, int64](doorI64, text, format)
}

// U8 casts integer text to uint8. Rules as I8.
func U8[T Text](text T, format NumFormat) (uint8, *Fault) {
	return numericDoor[T, uint8](doorU8, text, format)
}

// U16 casts integer text to uint16. Rules as I8.
func U16[T Text](text T, format NumFormat) (uint16, *Fault) {
	return numericDoor[T, uint16](doorU16, text, format)
}

// U32 casts integer text to uint32. Rules as I8.
func U32[T Text](text T, format NumFormat) (uint32, *Fault) {
	return numericDoor[T, uint32](doorU32, text, format)
}

// U64 casts integer text to uint64 — natively unsigned, no widening games. Rules as I8.
func U64[T Text](text T, format NumFormat) (uint64, *Fault) {
	return numericDoor[T, uint64](doorU64, text, format)
}

// F32 casts real text under the declared format: finite values only (NaN/Infinity literals
// are Malformed, overflow to infinity is OutOfRange), declared separators and grouping,
// accounting parentheses, exponent, and trailing percent (50% is 0.5).
func F32[T Text](text T, format NumFormat) (float32, *Fault) {
	return numericDoor[T, float32](doorF32, text, format)
}

// F64 casts real text to float64. Rules as F32.
func F64[T Text](text T, format NumFormat) (float64, *Fault) {
	return numericDoor[T, float64](doorF64, text, format)
}

// Exact casts decimal text to a Decimal under the declared format — the same grammar and
// NumFormat as the real doors (declared separators and grouping, accounting parentheses,
// exponent, trailing percent, currency symbol) — but never rounded: a magnitude past
// 2^96 - 1 or a scale past 28 is OutOfRange, not approximated. Named Exact because the
// result type already owns the identifier Decimal, the way Span returns a Duration.
func Exact[T Text](text T, format NumFormat) (Decimal, *Fault) {
	out, fault := numericDoor[T, rawDecimal](doorDecimal, text, format)
	if fault != nil {
		return Decimal{}, fault
	}
	return Decimal{Lo: out.Lo, Hi: out.Hi, Scale: out.Scale, Negative: out.Negative != 0}, nil
}

// Number is the closed set of targets the numeric doors cast to — exactly the eleven
// types I8 through U64, F32, F64 and Exact return. It exists for Numeric: a caller that is
// itself generic over the target names the type once instead of writing the door switch.
type Number interface {
	int8 | int16 | int32 | int64 | uint8 | uint16 | uint32 | uint64 | float32 | float64 | Decimal
}

// Numeric casts numeric text to V under the declared format by dispatching to V's own
// door — Numeric[int32] is I32, Numeric[Decimal] is Exact — with T inferred from the
// argument: hypercast.Numeric[int32]("42", format). The rules and verdicts are the
// concrete door's, unchanged. An unsupported V is impossible by construction: Number lists
// exactly the types that have a door, so the switch below is exhaustive and the compiler
// rejects anything else at the call site.
func Numeric[V Number, T Text](text T, format NumFormat) (V, *Fault) {
	// Switching on a pointer to the out-value keeps the dispatch allocation-free: boxing
	// the returned value into an interface would heap-allocate every int64 and float64.
	var out V
	var fault *Fault
	switch p := any(&out).(type) {
	case *int8:
		*p, fault = I8(text, format)
	case *int16:
		*p, fault = I16(text, format)
	case *int32:
		*p, fault = I32(text, format)
	case *int64:
		*p, fault = I64(text, format)
	case *uint8:
		*p, fault = U8(text, format)
	case *uint16:
		*p, fault = U16(text, format)
	case *uint32:
		*p, fault = U32(text, format)
	case *uint64:
		*p, fault = U64(text, format)
	case *float32:
		*p, fault = F32(text, format)
	case *float64:
		*p, fault = F64(text, format)
	case *Decimal:
		*p, fault = Exact(text, format)
	default:
		panic("unreachable: Number admits no other type")
	}
	if fault != nil {
		var zero V
		return zero, fault
	}
	return out, nil
}

// Uuid casts UUID text — all five .NET Guid formats (D/N/B/P/X) plus
// urn:uuid:/GUID:/UUID: prefixes — to a uuid.UUID (RFC 9562 byte order, which is exactly
// uuid.UUID's own layout).
func Uuid[T Text](text T) (uuid.UUID, *Fault) {
	r := textCall(doorUuid, text)
	if r.code != 0 {
		return uuid.UUID{}, failed(r.code, &r.fault)
	}
	return read[uuid.UUID](&r), nil
}

// instantCall serves both instant-shaped doors: a zero precision means the RFC 3339 door
// under the date options (nil for none), anything else the Unix-epoch door.
func instantCall[T Text](text T, precision UnixPrecision, options *DateOptions) result {
	if precision == 0 {
		return dateCall(doorTimestamp, 0, text, nil, options)
	}
	ptr, length := textPtr(text)
	return callDetailed(doorUnix, uint32(precision), ptr, length, nil, nil, nil, "")
}

// instantDoor serves both instant-shaped doors, as instantCall with no date options.
func instantDoor[T Text](text T, precision UnixPrecision) (time.Time, *Fault) {
	r := instantCall(text, precision, nil)
	if r.code != 0 {
		return time.Time{}, failed(r.code, &r.fault)
	}
	return read[RawTimestamp](&r).Time(), nil
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

// excelSerialCall is the Excel serial door over text, for an epoch the caller checked.
func excelSerialCall[T Text](text T, epoch ExcelEpoch) result {
	ptr, length := textPtr(text)
	return callDetailed(doorExcelSerial, uint32(epoch), ptr, length, nil, nil, nil, "")
}

// ExcelSerial casts an Excel date serial under a caller-declared ExcelEpoch to a UTC
// time.Time. The whole part counts days from the system's own day zero and the fraction is
// the time of day, so "45292.75" is 2024-01-01T18:00:00Z. A spreadsheet cell carries no
// zone and none is invented.
//
// The 1900 system contains a day that never existed: serial 60 is 1900-02-29, kept
// deliberately because Lotus 1-2-3 wrongly treated 1900 as a leap year and Excel copied the
// bug for file compatibility. It is OutOfRange here — the same verdict DateOnly gives the
// text "1900-02-29" — so every serial above it is shifted one day against a naive count,
// which is the arithmetic hand-rolled conversions get wrong.
//
// An undefined epoch is a caller bug and panics, never a verdict.
func ExcelSerial[T Text](text T, epoch ExcelEpoch) (time.Time, *Fault) {
	if epoch != Excel1900 && epoch != Excel1904 {
		panic(fmt.Sprintf("hypercast: undefined ExcelEpoch %d", epoch))
	}
	r := excelSerialCall(text, epoch)
	if r.code != 0 {
		return time.Time{}, failed(r.code, &r.fault)
	}
	return read[RawTimestamp](&r).Time(), nil
}

// DateOnly casts a strict ISO 8601 yyyy-MM-dd calendar date.
func DateOnly[T Text](text T) (Date, *Fault) {
	r := dateCall(doorDate, 0, text, nil, nil)
	if r.code != 0 {
		return Date{}, failed(r.code, &r.fault)
	}
	return read[RawDate](&r).Date(), nil
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
	r := dateCall(doorDateOrdered, uint32(order), text, nil, nil)
	if r.code != 0 {
		return Date{}, failed(r.code, &r.fault)
	}
	return read[RawDate](&r).Date(), nil
}

// DateTime casts a zone-less civil date-time — the shape untrusted feeds actually send
// ("1/7/2026 3:04 PM", "2026-01-07 15:04:05") — under the caller-declared DateOrder. The
// date part follows DateOnlyOrdered's grammar; the optional time part (one space or 'T'
// after the date) is 24-hour h:mm[:ss[.f]] or 12-hour with an AM/PM marker, after the
// clock or ahead of it; absent, the time is midnight. No zone is read and none is
// invented — Timestamp stays the strict RFC 3339 instant door. The markers are the English
// AM/PM only; DateTimeWith reads a culture's own beside them. An undefined order is a
// caller bug and panics, never a verdict.
func DateTime[T Text](text T, order DateOrder) (CivilDateTime, *Fault) {
	if order < YearMonthDay || order > DayMonthYear {
		panic(fmt.Sprintf("hypercast: undefined DateOrder %d", order))
	}
	r := dateCall(doorDateTime, uint32(order), text, nil, nil)
	if r.code != 0 {
		return CivilDateTime{}, failed(r.code, &r.fault)
	}
	return read[RawCivil](&r).CivilDateTime(), nil
}

// DateTimeWith is DateTime under a declared DateFormat: the same grammar under its Order,
// with its AM and PM designators read beside the English markers ("1/7/2026 3:04 nachm.",
// "2026-01-07 오후 3:04"). A format declaring no designators reads exactly what DateTime
// reads. An undefined order or a designator the core would refuse is a caller bug and
// panics, never a verdict.
func DateTimeWith[T Text](text T, format DateFormat) (CivilDateTime, *Fault) {
	if format.Order < YearMonthDay || format.Order > DayMonthYear {
		panic(fmt.Sprintf("hypercast: undefined DateOrder %d", format.Order))
	}
	var periods RawDayPeriods
	format.raw(&periods)
	r := dateCall(doorDateTime, uint32(format.Order), text, &periods, nil)
	if r.code != 0 {
		return CivilDateTime{}, failed(r.code, &r.fault)
	}
	return read[RawCivil](&r).CivilDateTime(), nil
}

// TimeOfDay casts an ISO 24-hour time-of-day to a time.Duration since midnight —
// nanosecond-exact, and comfortably inside time.Duration's range.
func TimeOfDay[T Text](text T) (time.Duration, *Fault) {
	r := textCall(doorTime, text)
	if r.code != 0 {
		return 0, failed(r.code, &r.fault)
	}
	return time.Duration(read[uint64](&r)), nil
}

// Span casts a duration (ISO 8601 fixed components, invariant colon form, or protobuf JSON
// seconds) to the protobuf pair — see Duration for why not time.Duration directly.
func Span[T Text](text T) (Duration, *Fault) {
	r := textCall(doorDuration, text)
	if r.code != 0 {
		return Duration{}, failed(r.code, &r.fault)
	}
	out := read[RawTimestamp](&r)
	return Duration{Seconds: out.Seconds, Nanos: out.Nanos}, nil
}

// ScalarTarget is the closed set of types Scalar casts to: every Number, plus bool,
// uuid.UUID, time.Time, Date and Duration — each the one type a single door returns
// without a declared argument. Three are left out on purpose:
//
//   - rune: it is int32, so Scalar[int32] is I32's integer door; call Char for a character.
//   - time.Duration: TimeOfDay returns one, but a Go reader takes time.Duration for a span,
//     and Span returns Duration because time.Duration cannot hold the core's window.
//   - CivilDateTime: zone-less date-times need a declared DateOrder; call DateTime.
type ScalarTarget interface {
	Number | bool | uuid.UUID | time.Time | Date | Duration
}

// Scalar casts text to V by dispatching to V's own door, the way Numeric does for the
// numeric targets: Scalar[bool] is Bool, Scalar[uuid.UUID] is Uuid, Scalar[time.Time] is
// Timestamp (RFC 3339, zone mandatory, normalized to UTC), Scalar[Date] is DateOnly
// (strict yyyy-MM-dd), Scalar[Duration] is Span, and every Number is Numeric. format is
// read only by the numeric doors. The rules and verdicts are the concrete door's,
// unchanged, and an unsupported V is a compile error: ScalarTarget lists exactly the types
// that have a door.
func Scalar[V ScalarTarget, T Text](text T, format NumFormat) (V, *Fault) {
	// A pointer to the out-value, as in Numeric, keeps the dispatch allocation-free.
	var out V
	var fault *Fault
	switch p := any(&out).(type) {
	case *bool:
		*p, fault = Bool(text)
	case *uuid.UUID:
		*p, fault = Uuid(text)
	case *time.Time:
		*p, fault = Timestamp(text)
	case *Date:
		*p, fault = DateOnly(text)
	case *Duration:
		*p, fault = Span(text)
	case *int8:
		*p, fault = I8(text, format)
	case *int16:
		*p, fault = I16(text, format)
	case *int32:
		*p, fault = I32(text, format)
	case *int64:
		*p, fault = I64(text, format)
	case *uint8:
		*p, fault = U8(text, format)
	case *uint16:
		*p, fault = U16(text, format)
	case *uint32:
		*p, fault = U32(text, format)
	case *uint64:
		*p, fault = U64(text, format)
	case *float32:
		*p, fault = F32(text, format)
	case *float64:
		*p, fault = F64(text, format)
	case *Decimal:
		*p, fault = Exact(text, format)
	default:
		panic("unreachable: ScalarTarget admits no other type")
	}
	if fault != nil {
		var zero V
		return zero, fault
	}
	return out, nil
}

// ExactFromFloat64 reads a number the caller already holds — the float64 a workbook stores
// for a numeric cell — as the exact Decimal it names: the shortest decimal that rounds back
// to the float64, the digits a spreadsheet writes for it. So 0.1 is one tenth, not the
// binary fraction nearest it, and 0.1 + 0.2 is 0.30000000000000004 — the same digits
// strconv.FormatFloat(v, 'g', -1, 64) prints. The result is canonical, as Exact's is. NaN is
// Malformed; an infinity, a magnitude past 2^96 - 1 or more than 28 places is OutOfRange —
// the digits are never cut to fit. A typed door's Fault has no span: Offset and Length are 0.
func ExactFromFloat64(value float64) (Decimal, *Fault) {
	r := callDetailedFromF64(doorDecimal, 0, value)
	if r.code != 0 {
		return Decimal{}, failed(r.code, &r.fault)
	}
	out := read[rawDecimal](&r)
	return Decimal{Lo: out.Lo, Hi: out.Hi, Scale: out.Scale, Negative: out.Negative != 0}, nil
}

// ExcelSerialFromFloat64 reads an Excel date serial the caller already holds as a float64
// under a caller-declared ExcelEpoch — the twin of ExcelSerial for a workbook reader that
// has the cell's number and no text. The result is the zone-less wall clock the cell holds,
// a CivilDateTime as DateTime returns, its fraction snapped: the time with the fewest
// fractional-second digits that the same float64 stores, so Excel's 23:59:59 is read on the
// second rather than the nanoseconds of float noise the double carries. The
// 1900 system's phantom serial 60, a serial below the system's first day and one past
// 9999-12-31 are OutOfRange; a negative, NaN or infinite serial is Malformed. An undefined
// epoch is a caller bug and panics, never a verdict.
func ExcelSerialFromFloat64(value float64, epoch ExcelEpoch) (CivilDateTime, *Fault) {
	if epoch != Excel1900 && epoch != Excel1904 {
		panic(fmt.Sprintf("hypercast: undefined ExcelEpoch %d", epoch))
	}
	r := callDetailedFromF64(doorExcelSerial, uint32(epoch), value)
	if r.code != 0 {
		return CivilDateTime{}, failed(r.code, &r.fault)
	}
	return read[RawCivil](&r).CivilDateTime(), nil
}

// ExcelTime reads the fraction of an Excel serial the caller already holds as a float64 as a
// time.Duration since midnight, as TimeOfDay returns: 0.75 and 45292.75 are both 18h.
// Snapped as ExcelSerialFromFloat64 snaps, and a fraction that snaps to a whole day is
// midnight.
// A negative, NaN or infinite serial is Malformed; one past 9999-12-31 is OutOfRange.
func ExcelTime(value float64) (time.Duration, *Fault) {
	r := callDetailedFromF64(doorTime, 0, value)
	if r.code != 0 {
		return 0, failed(r.code, &r.fault)
	}
	return time.Duration(read[uint64](&r)), nil
}

// ExcelDuration reads a number of days the caller already holds as a float64 — what an
// elapsed-time format ([h]:mm:ss) stores — as the protobuf pair Span returns: 1.5 is a day
// and twelve hours, and a negative span is negative, its size snapped as a serial's time. NaN
// or an infinity is Malformed; beyond ±10,000 years is OutOfRange.
func ExcelDuration(value float64) (Duration, *Fault) {
	r := callDetailedFromF64(doorDuration, 0, value)
	if r.code != 0 {
		return Duration{}, failed(r.code, &r.fault)
	}
	out := read[RawTimestamp](&r)
	return Duration{Seconds: out.Seconds, Nanos: out.Nanos}, nil
}
