package hypercast

// The opt-in date shapes: DateOptions (the shape bits, a two-digit-year pivot and a month
// vocabulary) for the detailed date doors, laid out as the core reads them and judged before
// any call. Only the detailed doors take them: the 1.0 doors keep each door's own shapes.

import (
	"encoding/binary"
	"fmt"
	"strings"
	"time"
	"unicode/utf8"
)

// DateShapes are the opt-in spellings of DateOptions — bit-for-bit the native core's option
// flags. Each is read by the door it names and ignored by the rest, so one DateOptions works
// at every date door.
type DateShapes uint32

const (
	// CompactDate reads yyyyMMdd, eight ASCII digits ("20260107"), at DateOnlyDetailed.
	CompactDate DateShapes = 1 << iota
	// SpaceSeparator reads a single space in place of the 'T' at TimestampDetailed.
	SpaceSeparator
	// HourOffset reads ±hh offsets ("+00", "-05") at TimestampDetailed.
	HourOffset

	// CommonForms is CompactDate, SpaceSeparator and HourOffset together. It supplies no
	// pivot and no vocabulary.
	CommonForms = CompactDate | SpaceSeparator | HourOffset
)

// MaxMonthNameBytes is the longest month name a MonthNames vocabulary takes, in UTF-8 bytes.
const MaxMonthNameBytes = 64

// DateOptions are the shapes a detailed date door reads beyond its own, all opt-in: the zero
// DateOptions reads exactly what the 1.0 door reads. Shapes are read by the doors they name;
// Pivot and Months by DateOnlyOrderedDetailed and DateTimeDetailed only. Months is independent
// of the rest: with nothing else declared it opens month names and nothing else.
//
// Options the core would refuse are a caller bug and panics at the door, never a verdict:
// Shapes setting a bit DateShapes does not define, or a Pivot other than 0 or 100 through
// 9999. DateOptions.Raw reports the same check as an error. Build it with field names: a
// later release may add a field.
type DateOptions struct {
	_ struct{}
	// Shapes are the opt-in spellings: CompactDate, SpaceSeparator, HourOffset, CommonForms.
	Shapes DateShapes
	// Pivot is the last year of the 100-year window a two-digit year is read in: with 2029,
	// "26" is 2026 and "30" is 1930. 0 declares none, so a two-digit year stays Malformed;
	// otherwise 100 through 9999, which keeps the window inside years 1 to 9999. A four-digit
	// year is read as written.
	Pivot int
	// Months is the vocabulary a month field may be written in ("07-Jan-26", "7.janv..2026"),
	// the zero MonthNames for none.
	Months MonthNames
}

// valid is the check every detailed date door runs on its options; fault names what it
// refused.
func (o *DateOptions) valid() bool {
	return o.Shapes&^CommonForms == 0 && (o.Pivot == 0 || o.Pivot >= 100 && o.Pivot <= 9999)
}

// fault names why valid refused the options, as the doors' panic text; nil when it did not.
// The shape bits are judged first, as the core judges them.
func (o *DateOptions) fault() error {
	if extra := o.Shapes &^ CommonForms; extra != 0 {
		return fmt.Errorf("DateOptions Shapes set bits HyperCast does not define: %#x", uint32(extra))
	}
	if o.Pivot != 0 && (o.Pivot < 100 || o.Pivot > 9999) {
		return fmt.Errorf("DateOptions pivot %d is neither 0 nor 100 through 9999", o.Pivot)
	}
	return nil
}

// layout writes the options as the core reads them into raw, for options valid has passed.
func (o *DateOptions) layout(raw *RawDateOptions) {
	raw.Size = RawDateOptionsSize
	raw.Flags = uint32(o.Shapes)
	raw.Pivot = uint32(o.Pivot)
}

// Raw validates the options and lays the shape bits and the pivot out as the core reads them
// — what every detailed date door does with its options, as an error rather than the doors'
// panic. The vocabulary crosses beside them, as Months.Packed.
func (o DateOptions) Raw() (RawDateOptions, error) {
	if !o.valid() {
		return RawDateOptions{}, o.fault()
	}
	var raw RawDateOptions
	o.layout(&raw)
	return raw, nil
}

// dateCall is a date door under its options (nil, or a zero DateOptions, for none, which
// crosses as null): the options panic here when the core would refuse them, and the month
// vocabulary crosses only to the doors that read one.
func dateCall[T Text](door, param uint32, text T, periods *RawDayPeriods, options *DateOptions) result {
	ptr, length := textPtr(text)
	if options == nil || *options == (DateOptions{}) {
		return callDetailed(door, param, ptr, length, nil, periods, nil, "")
	}
	if !options.valid() {
		panic("hypercast: " + options.fault().Error())
	}
	var raw RawDateOptions
	options.layout(&raw)
	vocab := ""
	if door == doorDateOrdered || door == doorDateTime {
		vocab = options.Months.packed
	}
	return callDetailed(door, param, ptr, length, nil, periods, &raw, vocab)
}

// MonthName is one entry of a month vocabulary: a name, and the month it stands for. Aliases
// are separate entries ({time.September, "Sep"} and {time.September, "Sept"}).
type MonthName struct {
	Month time.Month
	Name  string
}

// MonthNames is a month vocabulary, caller-built and judged once by NewMonthNames: the names
// a month field may be written in, each standing for its month, under the field order the door
// declares ("07-Jan-26" under DayMonthYear) and with the date's own repeated '/', '-' or '.'
// separator. Names match under Unicode simple case folding, the longest declared name followed
// by the separator (or the end) winning. Punctuation is part of a name and never stripped: with
// "janv." declared and '.' as the separator, "7.janv..2026" reads, while "7.janv.2026" does not
// unless "janv" is also declared. The zero MonthNames declares no vocabulary. Comparable:
// two vocabularies are == when they hold the same entries in the same order.
type MonthNames struct {
	// packed is the vocabulary as the core reads it: each entry a month byte, a little-endian
	// 32-bit byte length, then the name's bytes.
	packed string
}

// NewMonthNames judges the entries as the core does and returns the vocabulary, or an error
// naming the first rule an entry breaks, in reading order: a month outside January to
// December, a name that is empty or longer than MaxMonthNameBytes, a name holding an ASCII
// digit, '/', '-' or any white space ('.' is fine anywhere), or a name that folds to the same
// spelling as an earlier one (even for the same month: "Jan" and "JAN"); then a month with no
// name. Use it for a vocabulary read at run time; MustMonthNames is the form for one written in
// code.
func NewMonthNames(names ...MonthName) (MonthNames, error) {
	size := 0
	for _, n := range names {
		size += 5 + len(n.Name)
	}
	packed := make([]byte, 0, size)
	for _, n := range names {
		month := byte(0) // an undefined month, whatever its value: judge names the rule
		if n.Month >= time.January && n.Month <= time.December {
			month = byte(n.Month)
		}
		packed = append(packed, month)
		packed = binary.LittleEndian.AppendUint32(packed, uint32(len(n.Name)))
		packed = append(packed, n.Name...)
	}
	return judgeMonthNames(string(packed), names)
}

// MustMonthNames is NewMonthNames for a vocabulary written in code, where a refusal is a
// caller bug: it panics with NewMonthNames's error text.
func MustMonthNames(names ...MonthName) MonthNames {
	m, err := NewMonthNames(names...)
	if err != nil {
		panic(err.Error())
	}
	return m
}

// monthNamesFromPacked judges a vocabulary already packed (the corpus's cut-short and
// non-UTF-8 rows, which no MonthName can spell).
func monthNamesFromPacked(packed []byte) (MonthNames, error) {
	return judgeMonthNames(string(packed), nil)
}

// le32 is the little-endian 32-bit word at text[at:at+4].
func le32(text string, at int) uint32 {
	return uint32(text[at]) | uint32(text[at+1])<<8 | uint32(text[at+2])<<16 | uint32(text[at+3])<<24
}

// judgeMonthNames is the core's judgement of a packed vocabulary, rule by rule in the core's
// order (each entry's layout, month, UTF-8, length, characters and fold against every earlier
// name; then each month's entry), naming the rule in the error, with entry i's month as
// declared[i] spells it when the entries were declared rather than packed. The core judges
// the bytes again on every call; a vocabulary it would refuse after these rules passed is
// reported here too, so no door ever sends one.
func judgeMonthNames(packed string, declared []MonthName) (MonthNames, error) {
	var names []string
	var seen uint16
	for at, i := 0, 0; at < len(packed); i++ {
		if len(packed)-at < 5 {
			return MonthNames{}, fmt.Errorf("MonthNames entry %d is cut short", i)
		}
		length := le32(packed, at+1)
		if uint64(length) > uint64(len(packed)-at-5) {
			return MonthNames{}, fmt.Errorf("MonthNames entry %d is cut short", i)
		}
		m := packed[at]
		name := packed[at+5 : at+5+int(length)]
		at += 5 + int(length)
		if m < 1 || m > 12 {
			month := int(m)
			if declared != nil {
				month = int(declared[i].Month)
			}
			return MonthNames{}, fmt.Errorf("MonthNames entry %d's month %d must be 1 through 12", i, month)
		}
		if !utf8.ValidString(name) {
			return MonthNames{}, fmt.Errorf("MonthNames entry %d's name %q is not valid UTF-8", i, name)
		}
		if name == "" || len(name) > MaxMonthNameBytes {
			return MonthNames{}, fmt.Errorf("MonthNames entry %d's name %q must be 1 to %d UTF-8 bytes", i, name, MaxMonthNameBytes)
		}
		if strings.ContainsFunc(name, func(r rune) bool { return r >= '0' && r <= '9' || r == '/' || r == '-' || isWhiteSpace(r) }) {
			return MonthNames{}, fmt.Errorf("MonthNames entry %d's name %q must not contain an ASCII digit, '/', '-' or white space", i, name)
		}
		for j, earlier := range names {
			if strings.EqualFold(name, earlier) {
				return MonthNames{}, fmt.Errorf("MonthNames entries %d (%q) and %d (%q) fold to the same spelling", j, earlier, i, name)
			}
		}
		names = append(names, name)
		seen |= 1 << m
	}
	for m := time.January; m <= time.December; m++ {
		if seen&(1<<m) == 0 {
			return MonthNames{}, fmt.Errorf("MonthNames: %s has no name", m)
		}
	}
	if !coreAcceptsVocabulary(packed) {
		return MonthNames{}, fmt.Errorf("MonthNames: two names fold to the same spelling under the core's case folding")
	}
	return MonthNames{packed: packed}, nil
}

// coreAcceptsVocabulary asks the core to judge the vocabulary: a call to the date-ordered
// door over empty text, which reports Empty once the vocabulary passes and is refused (-1)
// otherwise. Go's strings.EqualFold and the core fold under the same Unicode (17.0), so this
// only backs the rules above up.
func coreAcceptsVocabulary(packed string) bool {
	return callDetailed(doorDateOrdered, uint32(YearMonthDay), nil, 0, nil, nil, nil, packed).code != -1
}

// Packed is the vocabulary as the core reads it beside RawDateOptions, a fresh copy, nil for
// none: each entry its month byte (1 to 12), a little-endian 32-bit byte length, then the
// name's UTF-8 bytes, with no padding, in the order NewMonthNames was given them.
func (m MonthNames) Packed() []byte {
	if m.packed == "" {
		return nil
	}
	return []byte(m.packed)
}
