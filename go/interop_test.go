package hypercast

import (
	"encoding/binary"
	"strings"
	"testing"
	"time"
	"unicode"
	"unsafe"
)

// The exported interop surface a module carrying HyperCast's verdicts across its own C ABI
// reads with: each raw value presents as the door of the same name presents it, each code
// decodes to exactly the constant it names, and the format check errs where the doors panic.

func TestCodesDecodeToTheConstantTheyNameAndNothingElseDecodes(t *testing.T) {
	for code := uint32(1); code <= 4; code++ {
		if p, ok := UnixPrecisionFromCode(code); !ok || uint32(p) != code {
			t.Errorf("UnixPrecisionFromCode(%d) = %v, %v", code, p, ok)
		}
	}
	for code := uint32(1); code <= 3; code++ {
		if o, ok := DateOrderFromCode(code); !ok || uint32(o) != code {
			t.Errorf("DateOrderFromCode(%d) = %v, %v", code, o, ok)
		}
		if r, ok := ReasonFromCode(code); !ok || uint32(r) != code {
			t.Errorf("ReasonFromCode(%d) = %v, %v", code, r, ok)
		}
	}
	for code := uint32(1); code <= 2; code++ {
		if e, ok := ExcelEpochFromCode(code); !ok || uint32(e) != code {
			t.Errorf("ExcelEpochFromCode(%d) = %v, %v", code, e, ok)
		}
	}
	for _, outside := range []uint32{0, 5, 1 << 31, ^uint32(0)} {
		if _, ok := UnixPrecisionFromCode(outside); ok {
			t.Errorf("UnixPrecisionFromCode(%d) decoded", outside)
		}
		if _, ok := DateOrderFromCode(outside); ok {
			t.Errorf("DateOrderFromCode(%d) decoded", outside)
		}
		if _, ok := ExcelEpochFromCode(outside); ok {
			t.Errorf("ExcelEpochFromCode(%d) decoded", outside)
		}
		if _, ok := ReasonFromCode(outside); ok {
			t.Errorf("ReasonFromCode(%d) decoded", outside)
		}
	}
	if _, ok := ExcelEpochFromCode(3); ok {
		t.Error("ExcelEpochFromCode(3) decoded")
	}
	if _, ok := DateOrderFromCode(4); ok {
		t.Error("DateOrderFromCode(4) decoded")
	}
}

func TestAFaultKeepsItsSpanAndACodeThatNamesNoReasonIsRefused(t *testing.T) {
	f, ok := FaultFromCode(2, 3, 4)
	if !ok || *f != (Fault{Reason: Malformed, Offset: 3, Length: 4}) {
		t.Errorf("FaultFromCode(2, 3, 4) = %v, %v", f, ok)
	}
	for _, code := range []uint32{0, 7} {
		if f, ok := FaultFromCode(code, 0, 0); ok || f != nil {
			t.Errorf("FaultFromCode(%d) = %v, %v", code, f, ok)
		}
	}
}

func TestRawValuesPresentAsTheDoorsPresentThem(t *testing.T) {
	want, fault := Timestamp("2026-01-02T15:04:05.123456789Z")
	if fault != nil {
		t.Fatal(fault)
	}
	if got := (RawTimestamp{Seconds: 1_767_366_245, Nanos: 123_456_789}).Time(); !got.Equal(want) || got.Location() != time.UTC {
		t.Errorf("RawTimestamp.Time() = %v, want %v", got, want)
	}
	if got := (RawDate{Year: 2026, Month: 1, Day: 7}).Date(); got != (Date{Year: 2026, Month: time.January, Day: 7}) {
		t.Errorf("RawDate.Date() = %v", got)
	}
	civil, fault := DateTime("2026-01-07 15:04:05.123456789", YearMonthDay)
	if fault != nil {
		t.Fatal(fault)
	}
	if got := (RawCivil{Year: 2026, Month: 1, Day: 7, Nanos: 54_245_123_456_789}).CivilDateTime(); got != civil {
		t.Errorf("RawCivil.CivilDateTime() = %v, want %v", got, civil)
	}
	if FormatVersion(0x00_06_02) != "0.6.2" || FormatVersion(0x01_02_03) != "1.2.3" {
		t.Error("FormatVersion")
	}
}

func TestADecimalAndADurationAreTheCoresBytes(t *testing.T) {
	// {u64 lo, u32 hi, u8 scale, u8 negative} and {i64 seconds, i32 nanos}, viewed in place,
	// the words in the host's order, as a native library on this host writes them.
	var buffer [2]uint64
	raw := unsafe.Slice((*byte)(unsafe.Pointer(&buffer)), 16)
	binary.NativeEndian.PutUint64(raw, 12345)
	raw[12], raw[13] = 2, 1
	if got := *(*Decimal)(unsafe.Pointer(&buffer)); got.String() != "-123.45" {
		t.Errorf("decimal view = %v", got)
	}
}

func TestAFormatErrsWhereTheDoorsPanic(t *testing.T) {
	raw, err := NumFormat{DecimalSep: ",", GroupSep: ".", Styles: AllStyles, Currency: "€"}.Raw()
	if err != nil || raw.Size != 92 || raw.Flags != uint32(AllStyles) || raw.DigitZero != 0 ||
		raw.DecimalSep.String() != "," || raw.GroupSep.String() != "." || raw.Currency.Len != 3 || raw.Currency.String() != "€" ||
		raw.CurrencyDecimalSep.Len != 0 || raw.MinusSign.Len != 0 || raw.Percent.Len != 0 {
		t.Errorf("Raw() = %+v, %v", raw, err)
	}
	// Every new field lands in its own slot, the digit zero as its code point.
	full := NumFormat{
		DecimalSep: "٫", GroupSep: "٬", Styles: AllStyles | TrailingSign, Currency: "kr.",
		CurrencyDecimalSep: ".", CurrencyGroupSep: ", ", MinusSign: "−", PlusSign: "＋", Percent: "٪", DigitZero: '٠',
	}
	raw, err = full.Raw()
	if err != nil || raw.Flags != uint32(AllStyles|TrailingSign) || raw.DigitZero != 0x660 ||
		raw.DecimalSep.String() != "٫" || raw.GroupSep.String() != "٬" ||
		raw.CurrencyDecimalSep.String() != "." || raw.CurrencyGroupSep.String() != ", " ||
		raw.MinusSign.String() != "−" || raw.PlusSign.String() != "＋" || raw.Percent.String() != "٪" || raw.Currency.String() != "kr." {
		t.Errorf("Raw() = %+v, %v", raw, err)
	}
	for _, bad := range []NumFormat{
		{DecimalSep: ".", GroupSep: "."},
		{DecimalSep: "", GroupSep: ","},
		{DecimalSep: ".", GroupSep: ""},
		{DecimalSep: ".", GroupSep: "\xff"},
		{DecimalSep: ".", GroupSep: strings.Repeat(",", SymbolMaxBytes+1)},
		{DecimalSep: ".", GroupSep: ",1"},
		{DecimalSep: ".", GroupSep: ",", CurrencyDecimalSep: ","},
		{DecimalSep: ".", GroupSep: ",", CurrencyGroupSep: "."},
		{DecimalSep: ".", GroupSep: ",", CurrencyDecimalSep: "'", CurrencyGroupSep: "'"},
		{DecimalSep: ".", GroupSep: ",", CurrencyGroupSep: strings.Repeat("'", SymbolMaxBytes+1)},
		{DecimalSep: ".", GroupSep: ",", MinusSign: "- "},
		{DecimalSep: ".", GroupSep: ",", MinusSign: "\xe2\x88"},
		{DecimalSep: ".", GroupSep: ",", PlusSign: "+1"},
		{DecimalSep: ".", GroupSep: ",", Percent: "\t%"},
		{DecimalSep: ".", GroupSep: ",", Percent: strings.Repeat("%", SymbolMaxBytes+1)},
		{DecimalSep: ".", GroupSep: ",", Currency: strings.Repeat("$", CurrencyMaxBytes+1)},
		{DecimalSep: ".", GroupSep: ",", Currency: "\xff"},
		{DecimalSep: ".", GroupSep: ",", Currency: " F CFA"},
		{DecimalSep: ".", GroupSep: ",", Currency: "F CFA "},
		{DecimalSep: ".", GroupSep: ",", Currency: "\u00a0F CFA"},
		{DecimalSep: ".", GroupSep: ",", Currency: "F\u00a0CFA\u00a0"},
		{DecimalSep: ".", GroupSep: ",", Currency: "\u202fF"},
		{DecimalSep: ".", GroupSep: ",", Currency: "F\u3000"},
		{DecimalSep: ".", GroupSep: ",", Currency: "F\u0085"},
		{DecimalSep: ".", GroupSep: ",", Currency: "\u00a0"},
		{DecimalSep: ".", GroupSep: ",", DigitZero: '0'},
		{DecimalSep: ".", GroupSep: ",", DigitZero: 0xD800},
		{DecimalSep: ".", GroupSep: ",", DigitZero: 0x110000},
		{DecimalSep: ".", GroupSep: ",", DigitZero: 0x6F7}, // ۷: modulo 64 past 54, no digit block starts there
		{DecimalSep: ".", GroupSep: ",", MinusSign: "\u00a0"},
		{DecimalSep: ".", GroupSep: ",", PlusSign: "A\u202fB"},
		{DecimalSep: ".", GroupSep: ",", Percent: "\v"},
		{DecimalSep: ".", GroupSep: ",", Percent: "\u3000"},
		{DecimalSep: ".", GroupSep: ",", Currency: "€\u2009"},
		{DecimalSep: "\u00a0", GroupSep: ","},
		{DecimalSep: "x ", GroupSep: ","},
		{DecimalSep: ",", GroupSep: ".", CurrencyDecimalSep: "\u202f"},
		{DecimalSep: ",", GroupSep: ".", CurrencyDecimalSep: "x\u00a0"},
	} {
		if _, err := bad.Raw(); err == nil {
			t.Errorf("Raw() of %+v did not err", bad)
		}
	}
	// White space inside a currency symbol is fine: F CFA with each of the three spaces, and
	// the other white space too.
	for _, gap := range []string{" ", "\u00a0", "\u202f", "\u2009", "\u3000", "\t"} {
		if _, err := (NumFormat{DecimalSep: ".", GroupSep: ",", Currency: "F" + gap + "CFA"}).Raw(); err != nil {
			t.Errorf("Raw() of the currency symbol F%qCFA: %v", gap, err)
		}
	}
	// A group separator may carry white space (French grouping, ", "); a sign, the percent
	// symbol, the currency symbol and a decimal separator may not.
	for _, group := range []string{" ", "\u00a0", "\u202f", "\u2009", "\u3000"} {
		if _, err := (NumFormat{DecimalSep: ",", GroupSep: group, CurrencyGroupSep: ", "}).Raw(); err != nil {
			t.Errorf("Raw() of the group separator %q: %v", group, err)
		}
		if _, err := (NumFormat{DecimalSep: ".", GroupSep: ",", CurrencyDecimalSep: ",", CurrencyGroupSep: group}).Raw(); err != nil {
			t.Errorf("Raw() of the currency group separator %q: %v", group, err)
		}
	}
}

func TestADateFormatErrsWhereTheDoorPanics(t *testing.T) {
	raw, err := DateFormat{Order: DayMonthYear, AM: "vorm.", PM: "nachm."}.Raw()
	if err != nil || raw.Size != 40 || raw.AM.String() != "vorm." || raw.PM.String() != "nachm." {
		t.Errorf("Raw() = %+v, %v", raw, err)
	}
	// Nothing declared is a valid layout too — DateTimeWith reads it as DateTime does — and
	// an inner space is fine (Spanish "a. m.").
	if raw, err := (DateFormat{Order: YearMonthDay}).Raw(); err != nil || raw.Size != 40 || raw.AM.Len != 0 || raw.PM.Len != 0 {
		t.Errorf("Raw() of nothing declared = %+v, %v", raw, err)
	}
	for _, inner := range []DateFormat{
		{Order: MonthDayYear, AM: "a. m.", PM: "p. m."},
		{Order: MonthDayYear, AM: "a.\u202fm.", PM: "p.\u202fm."},
		{Order: MonthDayYear, AM: "a.\u00a0m.", PM: "p.\u00a0m."},
	} {
		if _, err := inner.Raw(); err != nil {
			t.Errorf("Raw() of an inner space in %+v: %v", inner, err)
		}
	}
	for _, bad := range []DateFormat{
		{AM: " vorm.", PM: "nachm."},
		{AM: "vorm.", PM: "nachm. "},
		{AM: "vorm.", PM: "nachm.\t"},
		{AM: "vorm.", PM: "nachm.\v"},
		{AM: "\u00a0vorm.", PM: "nachm."},
		{AM: "vorm.", PM: "nachm.\u202f"},
		{AM: "\u3000", PM: "nachm."},
		{AM: "am1", PM: "pm"},
		{AM: "\xff", PM: "pm"},
		{AM: "am", PM: strings.Repeat("p", DesignatorMaxBytes+1)},
	} {
		if _, err := bad.Raw(); err == nil {
			t.Errorf("Raw() of %+v did not err", bad)
		}
	}
}

// The layouts byte for byte, as rust/src/abi.rs pins them: the size word first, each
// symbol's bytes then its length, nothing between one field and the next.
func TestTheRawLayoutsAreTheCoresBytes(t *testing.T) {
	raw, err := NumFormat{
		DecimalSep: ",", GroupSep: ". ", Styles: AllStyles | TrailingSign, Currency: "€",
		CurrencyDecimalSep: ".", CurrencyGroupSep: ",", MinusSign: "−", PlusSign: "+", Percent: "٪", DigitZero: '０',
	}.Raw()
	if err != nil {
		t.Fatal(err)
	}
	bytes := unsafe.Slice((*byte)(unsafe.Pointer(&raw)), unsafe.Sizeof(raw))
	word := func(at int) uint32 { return binary.NativeEndian.Uint32(bytes[at:]) }
	symbol := func(at, capacity int) string { return string(bytes[at : at+int(bytes[at+capacity])]) }
	if len(bytes) != 92 || word(0) != 92 || word(4) != uint32(AllStyles|TrailingSign) || word(8) != 0xFF10 {
		t.Errorf("head = %v", bytes[:12])
	}
	for _, field := range []struct {
		at, capacity int
		want         string
	}{
		{12, 8, ","}, {21, 8, ". "}, {30, 8, "."}, {39, 8, ","}, {48, 8, "−"}, {57, 8, "+"}, {66, 8, "٪"}, {75, 16, "€"},
	} {
		if got := symbol(field.at, field.capacity); got != field.want {
			t.Errorf("symbol at %d = %q, want %q", field.at, got, field.want)
		}
	}
	periods, err := DateFormat{Order: YearMonthDay, AM: "오전", PM: "오후"}.Raw()
	if err != nil {
		t.Fatal(err)
	}
	bytes = unsafe.Slice((*byte)(unsafe.Pointer(&periods)), unsafe.Sizeof(periods))
	if len(bytes) != 40 || word(0) != 40 || symbol(4, 16) != "오전" || symbol(21, 16) != "오후" || bytes[38] != 0 || bytes[39] != 0 {
		t.Errorf("periods = %v", bytes)
	}
}

// The messages name the rule that broke: "white space" where this binding once said "ASCII
// whitespace", and the older rules win where a declaration breaks two.
func TestAFormatsFaultNamesTheRuleThatBrokeFirst(t *testing.T) {
	for _, c := range []struct {
		format NumFormat
		want   string
	}{
		{NumFormat{DecimalSep: ".", GroupSep: ",", MinusSign: "\u00a0"}, `minus sign "\u00a0" must not contain white space`},
		{NumFormat{DecimalSep: ".", GroupSep: ",", Currency: "€ "}, `currency symbol "€ " must not start or end with white space`},
		{NumFormat{DecimalSep: ".", GroupSep: ",", Currency: "\u202fF CFA"}, `currency symbol "\u202fF CFA" must not start or end with white space`},
		{NumFormat{DecimalSep: ".", GroupSep: ",", Currency: "F1 "}, `currency symbol "F1 " must not contain an ASCII digit`},
		{NumFormat{DecimalSep: "\u202f", GroupSep: ","}, `decimal separator "\u202f" must not contain white space`},
		{NumFormat{DecimalSep: ",", GroupSep: ".", CurrencyDecimalSep: "\u00a0"}, `currency decimal separator "\u00a0" must not contain white space`},
		{NumFormat{DecimalSep: "\u00a0", GroupSep: "\u00a0"}, `decimal and group separators must differ; both are "\u00a0"`},
		{NumFormat{DecimalSep: "\u00a01", GroupSep: ","}, `decimal separator "\u00a01" must not contain an ASCII digit`},
		{NumFormat{DecimalSep: "1", GroupSep: "1"}, `decimal separator "1" must not contain an ASCII digit`},
		{NumFormat{DecimalSep: ",", GroupSep: ".", CurrencyDecimalSep: "\u00a0", CurrencyGroupSep: "\u00a0"}, `currency decimal and group separators must differ; both are "\u00a0"`},
	} {
		err := c.format.fault()
		if err == nil || err.Error() != c.want {
			t.Errorf("fault() of %+v = %v, want %s", c.format, err, c.want)
		}
	}
	if err := (DateFormat{AM: "AM\u00a0"}).fault(); err == nil || err.Error() != `AM designator "AM\u00a0" must not start or end in white space` {
		t.Errorf("fault() of a trailing NBSP designator = %v", err)
	}
}

// isWhiteSpace is the 25 code points of Unicode's White_Space property and no other, over
// every code point.
func TestIsWhiteSpaceIsTheUnicodePropertyAndNothingElse(t *testing.T) {
	count := 0
	for r := rune(0); r <= unicode.MaxRune; r++ {
		got := isWhiteSpace(r)
		if got != unicode.Is(unicode.White_Space, r) {
			t.Errorf("isWhiteSpace(%U) = %v", r, got)
		}
		if got {
			count++
		}
	}
	if count != 25 {
		t.Errorf("%d white-space code points, want 25", count)
	}
}
