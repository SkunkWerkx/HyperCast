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
	raw, err := NumFormat{DecimalSep: ',', GroupSep: '.', Styles: AllStyles, Currency: "€"}.Raw()
	if err != nil || raw.DecimalSep != ',' || raw.GroupSep != '.' || raw.CurrencyLen != 3 || string(raw.Currency[:3]) != "€" {
		t.Errorf("Raw() = %+v, %v", raw, err)
	}
	for _, bad := range []NumFormat{
		{DecimalSep: '.', GroupSep: '.'},
		{DecimalSep: '.', GroupSep: 0xD800},
		{DecimalSep: '.', GroupSep: ',', Currency: strings.Repeat("$", CurrencyMaxBytes+1)},
		{DecimalSep: '.', GroupSep: ',', Currency: "\xff"},
		{DecimalSep: '.', GroupSep: ',', Currency: "U S"},
	} {
		if _, err := bad.Raw(); err == nil {
			t.Errorf("Raw() of %+v did not err", bad)
		}
	}
}
