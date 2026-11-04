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

// SymbolMaxBytes is the inline capacity of the ABI's separators, signs and percent symbol,
// in UTF-8 bytes: enough for every separator a real culture declares (one or two characters).
const SymbolMaxBytes = 8

// CurrencyMaxBytes is the inline capacity of the ABI's currency symbol, in UTF-8 bytes.
const CurrencyMaxBytes = 16

// DesignatorMaxBytes is the inline capacity of the ABI's day-period designators, in UTF-8
// bytes.
const DesignatorMaxBytes = 16

// RawSymbol8 is a separator, sign or percent symbol as it crosses the ABI: its UTF-8 bytes
// held inline (zero-padded) and their count, 9 bytes with no alignment. A zero length
// declares none.
type RawSymbol8 struct {
	Bytes [SymbolMaxBytes]byte
	Len   uint8
}

// RawSymbol16 is a currency symbol or a day-period designator as it crosses the ABI, the
// same shape at 16 bytes of capacity: 17 bytes with no alignment.
type RawSymbol16 struct {
	Bytes [CurrencyMaxBytes]byte
	Len   uint8
}

// String is the symbol's text — empty when none is declared.
func (s RawSymbol8) String() string { return string(s.Bytes[:s.Len]) }

// String is the symbol's text — empty when none is declared.
func (s RawSymbol16) String() string { return string(s.Bytes[:s.Len]) }

// RawNumFormat is NumFormat as it crosses the ABI — 92 bytes, 4-aligned, the core reading
// the caller's struct in place: a size word (RawNumFormatSize, so the layout can grow
// without another coordinated change), the flags, the native digits' zero code point (0
// for none), then every symbol inline, its bytes followed by its length. Byte offsets:
// Flags 4, DigitZero 8, DecimalSep 12, GroupSep 21, CurrencyDecimalSep 30, CurrencyGroupSep
// 39, MinusSign 48, PlusSign 57, Percent 66, Currency 75. NumFormat.Raw builds one.
type RawNumFormat struct {
	Size               uint32
	Flags              uint32
	DigitZero          uint32
	DecimalSep         RawSymbol8
	GroupSep           RawSymbol8
	CurrencyDecimalSep RawSymbol8
	CurrencyGroupSep   RawSymbol8
	MinusSign          RawSymbol8
	PlusSign           RawSymbol8
	Percent            RawSymbol8
	Currency           RawSymbol16
}

// RawNumFormatSize is the size word a RawNumFormat carries: its own 92 bytes.
const RawNumFormatSize = uint32(unsafe.Sizeof(RawNumFormat{}))

// RawDayPeriods is a DateFormat's designators as they cross the ABI beside the declared
// DateOrder — 40 bytes, 4-aligned: a size word (RawDayPeriodsSize), the AM designator
// inline at byte 4 and the PM one at 21, each its bytes followed by its length, and two
// bytes of padding. A zero length declares nothing beside the English marker; where the
// core takes a pointer to one, null declares nothing at all. DateFormat.Raw builds one.
type RawDayPeriods struct {
	Size uint32
	AM   RawSymbol16
	PM   RawSymbol16
	_    [2]byte
}

// RawDayPeriodsSize is the size word a RawDayPeriods carries: its own 40 bytes.
const RawDayPeriodsSize = uint32(unsafe.Sizeof(RawDayPeriods{}))

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

// RawConstraints is a declaration of validation constraints as it crosses the ABI, which the
// checked doors read in place: 72 bytes, 8-aligned, pointer-free, so the layout is the same
// on 32-bit and 64-bit targets. Byte offsets: Size 0, Flags 4, Rounding 8, MinLength 12,
// MaxLength 16, Reserved 20, Lower 24, Upper 40, MultipleOf 56, each decimal in Decimal's
// own layout. An enum list travels beside it (Constraints.PackedEnum). A Size other than
// RawConstraintsSize, an unknown flag bit, an unknown rounding code, a nonzero Reserved, a
// decimal sign byte other than 0 or 1, or a non-empty enum list without RawConstraintsHasEnum
// is a malformed layout. Constraints.Raw hands one over, from a declaration the core has
// already judged.
type RawConstraints struct {
	// Size is the layout's size as the caller built it: RawConstraintsSize.
	Size uint32
	// Flags says which constraints are declared: the RawConstraintsHas* bits,
	// RawConstraintsHasEnum among them, and RawConstraintsTrim.
	Flags uint32
	// Rounding is the Rounding code: 0 strict, 1 midpoint, 2 floor, 3 ceiling.
	Rounding uint32
	// MinLength is the fewest code points the text may hold.
	MinLength uint32
	// MaxLength is the most code points the text may hold.
	MaxLength uint32
	// Reserved is always 0.
	Reserved uint32
	// Lower is the minimum, or the exclusive minimum.
	Lower Decimal
	// Upper is the maximum, or the exclusive maximum.
	Upper Decimal
	// MultipleOf is the step every value must be a multiple of.
	MultipleOf Decimal
}

// RawConstraintsSize is the size word a RawConstraints carries: its own 72 bytes.
const RawConstraintsSize = uint32(unsafe.Sizeof(RawConstraints{}))

// The flags of RawConstraints: which of its fields are read.
const (
	// RawConstraintsHasMinimum means Lower is the inclusive minimum.
	RawConstraintsHasMinimum uint32 = 1 << iota
	// RawConstraintsHasExclusiveMinimum means Lower is the exclusiveMinimum.
	RawConstraintsHasExclusiveMinimum
	// RawConstraintsHasMaximum means Upper is the inclusive maximum.
	RawConstraintsHasMaximum
	// RawConstraintsHasExclusiveMaximum means Upper is the exclusiveMaximum.
	RawConstraintsHasExclusiveMaximum
	// RawConstraintsHasMultipleOf means MultipleOf and Rounding are read.
	RawConstraintsHasMultipleOf
	// RawConstraintsHasMinLength means MinLength is read.
	RawConstraintsHasMinLength
	// RawConstraintsHasMaxLength means MaxLength is read.
	RawConstraintsHasMaxLength
	// RawConstraintsTrim means text is trimmed of Unicode White_Space before it is judged.
	RawConstraintsTrim
	// RawConstraintsHasEnum means an enum is declared: the list beside the layout is read,
	// and an empty one is refused.
	RawConstraintsHasEnum
)

// RawCheck is what a checked door reports beside its value, as the core writes it: 12 bytes,
// 4-aligned.
type RawCheck struct {
	// Keyword is, on a violation, the Keyword code broken; 0 otherwise.
	Keyword uint32
	// Adjusted is, on success, 1 when rounding changed the value, else 0.
	Adjusted uint32
	// EnumIndex is, on a success at the text door that matched an enum entry, its index;
	// math.MaxUint32 otherwise.
	EnumIndex uint32
}

// RawCheckSize is a RawCheck's size: 12 bytes.
const RawCheckSize = uint32(unsafe.Sizeof(RawCheck{}))

// RawSpan is a span of the caller's own input as the core writes it, 8 bytes, 4-aligned:
// what the text door writes in place of a copy.
type RawSpan struct {
	// Offset is the byte offset where the span starts.
	Offset uint32
	// Len is the span's length in bytes.
	Len uint32
}

// RawSpanSize is a RawSpan's size: 8 bytes.
const RawSpanSize = uint32(unsafe.Sizeof(RawSpan{}))

// asciiWhiteSpace reports an ASCII white-space byte: \t, \n, \v, \f, \r and space. Small
// enough to inline, since the symbol checks run it on every cast.
func asciiWhiteSpace(b byte) bool {
	return b == ' ' || b-'\t' <= '\r'-'\t'
}

// isWhiteSpace reports whether r has the Unicode White_Space property: the 25 code points
// of PropList.txt, which is what the core trims and what a numeral may not contain. U+200B,
// U+FEFF and U+180E are not among them. The table is the core's (rust/src/text.rs
// WHITE_SPACE), held to corpus/formats.json by the corpus replay.
func isWhiteSpace(r rune) bool {
	if r < 0x80 {
		return r >= 0 && asciiWhiteSpace(byte(r))
	}
	switch r {
	case 0x85, 0xA0, 0x1680, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000:
		return true
	}
	return r >= 0x2000 && r <= 0x200A
}

// spaces is where a declared symbol may not carry white space.
type spaces uint8

const (
	// spaceAnywhere places no limit: a group separator, whose white space the input's own
	// spaces stand for.
	spaceAnywhere spaces = iota
	// spaceNowhere refuses white space anywhere in the symbol: signs, percent symbols and
	// decimal separators, which sit flush against the digits.
	spaceNowhere
	// spaceAtEdges refuses it at either edge only: a currency symbol, whose interior may
	// hold a gap ("F CFA"), since the grammar trims around the symbol.
	spaceAtEdges
)

// symbolOK is the core's rule for one declared symbol, decided without naming the fault:
// at most capacity UTF-8 bytes, valid UTF-8, no ASCII digit, and white space only where
// space allows it. An empty symbol passes: the caller decides whether that is "none
// declared" or a missing separator. This runs on every cast, so the one-byte symbol
// nearly every format declares is decided inline, and the rest in one pass over the bytes
// with no call out for ASCII.
func symbolOK(symbol string, capacity int, space spaces) bool {
	if len(symbol) == 1 {
		b := symbol[0]
		return b < 0x80 && !(b >= '0' && b <= '9') && !(space != spaceAnywhere && asciiWhiteSpace(b))
	}
	return len(symbol) == 0 || symbolOKSlow(symbol, capacity, space)
}

func symbolOKSlow(symbol string, capacity int, space spaces) bool {
	if len(symbol) > capacity {
		return false
	}
	ascii := true
	for i := 0; i < len(symbol); i++ {
		b := symbol[i]
		if b >= '0' && b <= '9' || space == spaceNowhere && asciiWhiteSpace(b) {
			return false
		}
		ascii = ascii && b < 0x80
	}
	if !ascii && !(utf8.ValidString(symbol) && !(space == spaceNowhere && hasWhiteSpace(symbol))) {
		return false
	}
	return !(space == spaceAtEdges && whiteSpaceAtAnEdge(symbol))
}

// hasWhiteSpace reports whether a white-space code point occurs in text.
func hasWhiteSpace(text string) bool {
	for _, r := range text {
		if isWhiteSpace(r) {
			return true
		}
	}
	return false
}

// symbolFault names what symbolOK refused, as the doors' panic text. The rules apply in
// the core's order: length, UTF-8, digit, then white space.
func symbolFault(what, symbol string, capacity int, space spaces) error {
	if len(symbol) > capacity {
		return fmt.Errorf("%s %q exceeds %d UTF-8 bytes", what, symbol, capacity)
	}
	if !utf8.ValidString(symbol) {
		return fmt.Errorf("%s %q is not valid UTF-8", what, symbol)
	}
	for i := 0; i < len(symbol); i++ {
		if b := symbol[i]; b >= '0' && b <= '9' {
			return fmt.Errorf("%s %q must not contain an ASCII digit", what, symbol)
		}
	}
	switch space {
	case spaceNowhere:
		return whiteSpaceFault(what, symbol)
	case spaceAtEdges:
		if whiteSpaceAtAnEdge(symbol) {
			return fmt.Errorf("%s %q must not start or end with white space", what, symbol)
		}
	}
	return nil
}

// whiteSpaceFault names the white space symbolOK refused in a symbol, nil when there is none.
func whiteSpaceFault(what, symbol string) error {
	if hasWhiteSpace(symbol) {
		return fmt.Errorf("%s %q must not contain white space", what, symbol)
	}
	return nil
}

// symbol8 and symbol16 lay a symbol valid has passed into its slot: a byte loop rather
// than copy, which is a runtime call for every slot — eight of them on every cast, most
// of them empty.
func symbol8(raw *RawSymbol8, symbol string) {
	for i := 0; i < len(symbol) && i < SymbolMaxBytes; i++ {
		raw.Bytes[i] = symbol[i]
	}
	raw.Len = uint8(len(symbol))
}

func symbol16(raw *RawSymbol16, symbol string) {
	for i := 0; i < len(symbol) && i < CurrencyMaxBytes; i++ {
		raw.Bytes[i] = symbol[i]
	}
	raw.Len = uint8(len(symbol))
}

// currencySeparators are the separators in force for a currency amount, each falling back
// to its number counterpart.
func (f NumFormat) currencySeparators() (decimal, group string) {
	decimal, group = f.CurrencyDecimalSep, f.CurrencyGroupSep
	if decimal == "" {
		decimal = f.DecimalSep
	}
	if group == "" {
		group = f.GroupSep
	}
	return decimal, group
}

// digitZeroOK is the core's rule for the digit zero: 0, or a code point outside ASCII
// that starts a run of ten digits sharing its UTF-8 prefix (every Unicode digit block
// does; one whose value modulo 64 exceeds 54 does not).
func (f NumFormat) digitZeroOK() bool {
	return f.DigitZero == 0 || f.DigitZero >= 0x80 && utf8.ValidRune(f.DigitZero) && f.DigitZero%64 <= 54
}

// valid is the check every numeric door runs on its format, decided without naming the
// fault — the fast path, since it runs on every cast; fault names what it refused.
func (f NumFormat) valid() bool {
	if f.DecimalSep == "" || f.GroupSep == "" || f.DecimalSep == f.GroupSep {
		return false
	}
	if !symbolOK(f.DecimalSep, SymbolMaxBytes, spaceNowhere) || !symbolOK(f.GroupSep, SymbolMaxBytes, spaceAnywhere) {
		return false
	}
	// Everything below is declared by few formats and skipped in one test by the rest.
	if f.Currency == "" && f.CurrencyDecimalSep == "" && f.CurrencyGroupSep == "" &&
		f.MinusSign == "" && f.PlusSign == "" && f.Percent == "" && f.DigitZero == 0 {
		return true
	}
	if !symbolOK(f.CurrencyDecimalSep, SymbolMaxBytes, spaceNowhere) || !symbolOK(f.CurrencyGroupSep, SymbolMaxBytes, spaceAnywhere) {
		return false
	}
	if decimal, group := f.currencySeparators(); decimal == group {
		return false
	}
	return symbolOK(f.MinusSign, SymbolMaxBytes, spaceNowhere) && symbolOK(f.PlusSign, SymbolMaxBytes, spaceNowhere) &&
		symbolOK(f.Percent, SymbolMaxBytes, spaceNowhere) && symbolOK(f.Currency, CurrencyMaxBytes, spaceAtEdges) && f.digitZeroOK()
}

// fault names why valid refused the format, as the doors' panic text; nil when it did not.
// The rules apply in the core's order: each symbol's length, UTF-8 and digit, then equal
// separators, then white space.
func (f NumFormat) fault() error {
	if f.DecimalSep == "" || f.GroupSep == "" {
		return fmt.Errorf("decimal separator %q and group separator %q must both be declared", f.DecimalSep, f.GroupSep)
	}
	for _, symbol := range []struct{ what, text string }{
		{"decimal separator", f.DecimalSep}, {"group separator", f.GroupSep},
	} {
		if err := symbolFault(symbol.what, symbol.text, SymbolMaxBytes, spaceAnywhere); err != nil {
			return err
		}
	}
	if f.DecimalSep == f.GroupSep {
		return fmt.Errorf("decimal and group separators must differ; both are %q", f.DecimalSep)
	}
	if err := whiteSpaceFault("decimal separator", f.DecimalSep); err != nil {
		return err
	}
	for _, symbol := range []struct{ what, text string }{
		{"currency decimal separator", f.CurrencyDecimalSep}, {"currency group separator", f.CurrencyGroupSep},
	} {
		if err := symbolFault(symbol.what, symbol.text, SymbolMaxBytes, spaceAnywhere); err != nil {
			return err
		}
	}
	if decimal, group := f.currencySeparators(); decimal == group {
		return fmt.Errorf("currency decimal and group separators must differ; both are %q", decimal)
	}
	if err := whiteSpaceFault("currency decimal separator", f.CurrencyDecimalSep); err != nil {
		return err
	}
	for _, symbol := range []struct {
		what     string
		text     string
		capacity int
		space    spaces
	}{
		{"minus sign", f.MinusSign, SymbolMaxBytes, spaceNowhere},
		{"plus sign", f.PlusSign, SymbolMaxBytes, spaceNowhere},
		{"percent symbol", f.Percent, SymbolMaxBytes, spaceNowhere},
		{"currency symbol", f.Currency, CurrencyMaxBytes, spaceAtEdges},
	} {
		if err := symbolFault(symbol.what, symbol.text, symbol.capacity, symbol.space); err != nil {
			return err
		}
	}
	if !f.digitZeroOK() {
		return fmt.Errorf("digit zero %q starts no Unicode digit block", f.DigitZero)
	}
	return nil
}

// layout writes the format as the core reads it into raw, zero to begin with, for a
// format valid has passed. Into the caller's own local rather than returned: the doors
// hand the backends a pointer to it, so the 92 bytes are copied once, into the C call.
func (f NumFormat) layout(raw *RawNumFormat) {
	raw.Size = RawNumFormatSize
	raw.Flags = uint32(f.Styles)
	raw.DigitZero = uint32(f.DigitZero)
	symbol8(&raw.DecimalSep, f.DecimalSep)
	symbol8(&raw.GroupSep, f.GroupSep)
	symbol8(&raw.CurrencyDecimalSep, f.CurrencyDecimalSep)
	symbol8(&raw.CurrencyGroupSep, f.CurrencyGroupSep)
	symbol8(&raw.MinusSign, f.MinusSign)
	symbol8(&raw.PlusSign, f.PlusSign)
	symbol8(&raw.Percent, f.Percent)
	symbol16(&raw.Currency, f.Currency)
}

// Raw validates the format and lays it out as the core reads it — what every numeric door
// does with its format argument, as an error rather than the doors' panic. The core's
// rules verbatim: each number separator one to SymbolMaxBytes UTF-8 bytes with no ASCII
// digit, the two different; each currency separator the same or empty, and the pair in
// force for a currency amount different; each decimal separator, plain and currency, free
// of white space (a group separator may carry it); each sign and the percent symbol the
// same or empty, with no white space; the currency symbol at most CurrencyMaxBytes, no
// digit, no white space at either edge (inside is fine: "F CFA"); the digit zero 0, or a
// code point outside ASCII that starts a run of ten digits sharing its UTF-8 prefix (every
// Unicode digit block does; one whose value modulo 64 exceeds 54 does not).
func (f NumFormat) Raw() (raw RawNumFormat, err error) {
	if !f.valid() {
		return RawNumFormat{}, f.fault()
	}
	f.layout(&raw)
	return raw, nil
}

// designatorOK is the core's rule for one day-period designator, decided without naming
// the fault: empty, or a symbol with no white space at either end.
func designatorOK(designator string) bool {
	return symbolOK(designator, DesignatorMaxBytes, spaceAtEdges)
}

// whiteSpaceAtAnEdge reports whether text starts or ends in a white-space code point.
func whiteSpaceAtAnEdge(text string) bool {
	if text == "" {
		return false
	}
	first, _ := utf8.DecodeRuneInString(text)
	last, _ := utf8.DecodeLastRuneInString(text)
	return isWhiteSpace(first) || isWhiteSpace(last)
}

// valid is the check DateTimeWith runs on its format's designators, the fast path; fault
// names what it refused.
func (f DateFormat) valid() bool {
	return designatorOK(f.AM) && designatorOK(f.PM)
}

// fault names why valid refused the designators, as the door's panic text; nil when it
// did not.
func (f DateFormat) fault() error {
	for _, designator := range []struct{ what, text string }{{"AM designator", f.AM}, {"PM designator", f.PM}} {
		if err := symbolFault(designator.what, designator.text, DesignatorMaxBytes, spaceAnywhere); err != nil {
			return err
		}
		if whiteSpaceAtAnEdge(designator.text) {
			return fmt.Errorf("%s %q must not start or end in white space", designator.what, designator.text)
		}
	}
	return nil
}

// layout writes the designators as the core reads them into raw, zero to begin with, for
// a format valid has passed.
func (f DateFormat) layout(raw *RawDayPeriods) {
	raw.Size = RawDayPeriodsSize
	symbol16(&raw.AM, f.AM)
	symbol16(&raw.PM, f.PM)
}

// Raw validates the format's designators and lays them out as the core reads them — what
// DateTimeWith does with its format argument, as an error rather than the door's panic.
// The core's rules verbatim: each designator empty or at most DesignatorMaxBytes of valid
// UTF-8 with no ASCII digit and no white space at either end. The order is not
// checked here: it crosses beside the designators, and DateOrder.Valid reports it.
func (f DateFormat) Raw() (raw RawDayPeriods, err error) {
	if !f.valid() {
		return RawDayPeriods{}, f.fault()
	}
	f.layout(&raw)
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

// The raw shapes at the sizes and offsets rust/src/abi.rs pins for them, and Decimal and
// Duration at the core's layout, field by field: an array of any other length is another
// type, so a drift on this side fails the build.
var (
	_ [9]byte  = [unsafe.Sizeof(RawSymbol8{})]byte{}
	_ [17]byte = [unsafe.Sizeof(RawSymbol16{})]byte{}
	_ [92]byte = [unsafe.Sizeof(RawNumFormat{})]byte{}
	_ [0]byte  = [unsafe.Offsetof(RawNumFormat{}.Size)]byte{}
	_ [4]byte  = [unsafe.Offsetof(RawNumFormat{}.Flags)]byte{}
	_ [8]byte  = [unsafe.Offsetof(RawNumFormat{}.DigitZero)]byte{}
	_ [12]byte = [unsafe.Offsetof(RawNumFormat{}.DecimalSep)]byte{}
	_ [21]byte = [unsafe.Offsetof(RawNumFormat{}.GroupSep)]byte{}
	_ [30]byte = [unsafe.Offsetof(RawNumFormat{}.CurrencyDecimalSep)]byte{}
	_ [39]byte = [unsafe.Offsetof(RawNumFormat{}.CurrencyGroupSep)]byte{}
	_ [48]byte = [unsafe.Offsetof(RawNumFormat{}.MinusSign)]byte{}
	_ [57]byte = [unsafe.Offsetof(RawNumFormat{}.PlusSign)]byte{}
	_ [66]byte = [unsafe.Offsetof(RawNumFormat{}.Percent)]byte{}
	_ [75]byte = [unsafe.Offsetof(RawNumFormat{}.Currency)]byte{}
	_ [40]byte = [unsafe.Sizeof(RawDayPeriods{})]byte{}
	_ [4]byte  = [unsafe.Offsetof(RawDayPeriods{}.AM)]byte{}
	_ [21]byte = [unsafe.Offsetof(RawDayPeriods{}.PM)]byte{}
	_ [16]byte = [unsafe.Sizeof(RawTimestamp{})]byte{}
	_ [4]byte  = [unsafe.Sizeof(RawDate{})]byte{}
	_ [16]byte = [unsafe.Sizeof(RawCivil{})]byte{}
	_ [8]byte  = [unsafe.Offsetof(RawCivil{}.Nanos)]byte{}
	_ [72]byte = [unsafe.Sizeof(RawConstraints{})]byte{}
	_ [4]byte  = [unsafe.Offsetof(RawConstraints{}.Flags)]byte{}
	_ [8]byte  = [unsafe.Offsetof(RawConstraints{}.Rounding)]byte{}
	_ [12]byte = [unsafe.Offsetof(RawConstraints{}.MinLength)]byte{}
	_ [16]byte = [unsafe.Offsetof(RawConstraints{}.MaxLength)]byte{}
	_ [20]byte = [unsafe.Offsetof(RawConstraints{}.Reserved)]byte{}
	_ [24]byte = [unsafe.Offsetof(RawConstraints{}.Lower)]byte{}
	_ [40]byte = [unsafe.Offsetof(RawConstraints{}.Upper)]byte{}
	_ [56]byte = [unsafe.Offsetof(RawConstraints{}.MultipleOf)]byte{}
	_ [12]byte = [unsafe.Sizeof(RawCheck{})]byte{}
	_ [8]byte  = [unsafe.Sizeof(RawSpan{})]byte{}

	_ [16]byte = [unsafe.Sizeof(Decimal{})]byte{}
	_ [0]byte  = [unsafe.Offsetof(Decimal{}.Lo)]byte{}
	_ [8]byte  = [unsafe.Offsetof(Decimal{}.Hi)]byte{}
	_ [12]byte = [unsafe.Offsetof(Decimal{}.Scale)]byte{}
	_ [13]byte = [unsafe.Offsetof(Decimal{}.Negative)]byte{}
	// 16 bytes wherever an int64 is 8-aligned, as the core lays a Duration out; on the 32-bit
	// ports (386, arm, mips) an int64 is 4-aligned and a Duration is 12, so viewing the
	// core's bytes as a []Duration in place holds on a 64-bit host only.
	_ [16]byte = [unsafe.Sizeof(Duration{}) + 8 - unsafe.Alignof(Duration{}.Seconds)]byte{}
	_ [0]byte  = [unsafe.Offsetof(Duration{}.Seconds)]byte{}
	_ [8]byte  = [unsafe.Offsetof(Duration{}.Nanos)]byte{}
)
