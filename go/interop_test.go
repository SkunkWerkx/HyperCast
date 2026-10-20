package hypercast

import (
	"strings"
	"testing"
	"time"
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
	// {u64 lo, u32 hi, u8 scale, u8 negative} and {i64 seconds, i32 nanos}, viewed in place.
	var buffer [2]uint64
	raw := unsafe.Slice((*byte)(unsafe.Pointer(&buffer)), 16)
	raw[0], raw[12], raw[13] = 0x39, 2, 1 // 0x3039 = 12345
	raw[1] = 0x30
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
		{DecimalSep: ".", GroupSep: ",", Currency: "U S"},
		{DecimalSep: ".", GroupSep: ",", DigitZero: '0'},
		{DecimalSep: ".", GroupSep: ",", DigitZero: 0xD800},
		{DecimalSep: ".", GroupSep: ",", DigitZero: 0x110000},
		{DecimalSep: ".", GroupSep: ",", DigitZero: 0x6F7}, // ۷: modulo 64 past 54, no digit block starts there
	} {
		if _, err := bad.Raw(); err == nil {
			t.Errorf("Raw() of %+v did not err", bad)
		}
	}
	// A separator may carry a space (French grouping, ", "); a sign may not.
	if _, err := (NumFormat{DecimalSep: ",", GroupSep: " ", CurrencyGroupSep: ", "}).Raw(); err != nil {
		t.Errorf("Raw() of a spaced separator: %v", err)
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
	if _, err := (DateFormat{Order: MonthDayYear, AM: "a. m.", PM: "p. m."}).Raw(); err != nil {
		t.Errorf("Raw() of an inner space: %v", err)
	}
	for _, bad := range []DateFormat{
		{AM: " vorm.", PM: "nachm."},
		{AM: "vorm.", PM: "nachm. "},
		{AM: "vorm.", PM: "nachm.\t"},
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
	word := func(at int) uint32 {
		return uint32(bytes[at]) | uint32(bytes[at+1])<<8 | uint32(bytes[at+2])<<16 | uint32(bytes[at+3])<<24
	}
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
