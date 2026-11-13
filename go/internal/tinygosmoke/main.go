// Command tinygosmoke is the check CI runs in headless Chrome: this module compiled by TinyGo
// for the browser (`tinygo build -target=wasm`), loaded by index.html beside it through
// TinyGo's own wasm_exec.js. Nothing is linked: the core is the Go translation in
// internal/core, and it imports nothing from the host. It prints one PASS or FAIL line per
// check and DONE at the end; the forge's hyper-build-wasm.yml fails the run on any FAIL or a
// missing DONE, which is also what a trap part-way through looks like.
//
// Every ABI shape crosses at least once — a plain door (Bool, Uuid, Timestamp, DateOnly,
// TimeOfDay, Span), a numeric one with a format (I32, F64, Exact with a currency symbol,
// and a format declaring every symbol the 92-byte layout carries), a discriminated one
// (Unix, ExcelSerial, DateTime), the day periods beside the order (DateTimeWith), the text
// door (String), and the 104-byte constraints layout with an integer and a text enum list
// beside it (Check, CheckFromFloat64, CheckString, NewConstraints's own judgement) — and so
// do a fault and a violation with their spans, each copied into the translated core's memory and read back
// out of it.
//
// The expected core version comes from rust/Cargo.toml at build time
// (`-ldflags "-X main.wantVersion=..."`), so the page proves the translated core is the one
// this commit's rust/ builds, not a stale one. Stock Go builds this too, so `go vet ./...`
// covers it like any other package; it is internal, so it is nothing a consumer can import.
package main

import (
	"fmt"
	"time"

	hypercast "github.com/SkunkWerkx/HyperCast/go"
)

// wantVersion is rust/Cargo.toml's version, set by the build's -ldflags -X.
var wantVersion string

func check(ok bool, what string) {
	if ok {
		fmt.Println("PASS", what)
	} else {
		fmt.Println("FAIL", what)
	}
}

func main() {
	version := hypercast.NativeVersion()
	fmt.Println("NativeVersion", version, "want", wantVersion)
	check(version != "" && version == wantVersion, "NativeVersion matches rust/Cargo.toml")

	yes, fault := hypercast.Bool("Yes")
	check(fault == nil && yes, "Bool")

	i, fault := hypercast.I32("(1,234)", hypercast.Invariant)
	check(fault == nil && i == -1234, "I32 accounting parentheses")

	_, fault = hypercast.I32("  12x4", hypercast.Invariant)
	fmt.Println("fault", fault)
	check(fault != nil && fault.Reason == hypercast.Malformed && fault.Offset == 4 && fault.Length == 1,
		"I32 fault reason and span")

	_, fault = hypercast.U8("256", hypercast.Invariant)
	check(fault != nil && fault.Reason == hypercast.OutOfRange, "U8 out of range")

	f, fault := hypercast.F64("50%", hypercast.Invariant)
	check(fault == nil && f == 0.5, "F64 percent")

	dollars := hypercast.NumFormat{DecimalSep: ".", GroupSep: ",", Styles: hypercast.AllStyles, Currency: "$"}
	d, fault := hypercast.Exact("($1,234.50)", dollars)
	check(fault == nil && d.String() == "-1234.5" && d.Scale == 1 && d.Negative, "Exact with a currency symbol")

	estonian := hypercast.NumFormat{
		DecimalSep: ",", GroupSep: " ", Styles: hypercast.AllStyles | hypercast.TrailingSign,
		Currency: "€", MinusSign: "−",
	}
	minus, fault := hypercast.F64("1 234,5 €−", estonian)
	check(fault == nil && minus == -1234.5, "F64 declared minus sign, trailing")

	everything := hypercast.NumFormat{
		DecimalSep: "٫", GroupSep: "٬", Styles: hypercast.AllStyles, Currency: "€",
		CurrencyDecimalSep: ".", CurrencyGroupSep: ",", MinusSign: "−", PlusSign: "＋", Percent: "٪", DigitZero: '٠',
	}
	digits, fault := hypercast.Exact("١٬٢٣٤٫٥٪", everything)
	check(fault == nil && digits.String() == "12.345", "Exact native digits and declared percent")
	euros, fault := hypercast.F64("€1,234.50", everything)
	check(fault == nil && euros == 1234.5, "F64 currency separators")

	n, fault := hypercast.Numeric[uint64]("0xFF", hypercast.Invariant)
	check(fault == nil && n == 255, "Numeric[uint64] radix prefix")

	id, fault := hypercast.Uuid("{6ba7b810-9dad-11d1-80b4-00c04fd430c8}")
	check(fault == nil && id.String() == "6ba7b810-9dad-11d1-80b4-00c04fd430c8", "Uuid braced")

	ts, fault := hypercast.Timestamp("2026-01-07T15:04:05.123456789+02:00")
	check(fault == nil && ts.Equal(time.Date(2026, 1, 7, 13, 4, 5, 123456789, time.UTC)), "Timestamp")

	unix, fault := hypercast.Unix("1767798245123", hypercast.Milliseconds)
	check(fault == nil && unix.UnixMilli() == 1767798245123, "Unix milliseconds")

	excel, fault := hypercast.ExcelSerial("45292.75", hypercast.Excel1900)
	check(fault == nil && excel.Equal(time.Date(2024, 1, 1, 18, 0, 0, 0, time.UTC)), "ExcelSerial 1900")

	date, fault := hypercast.DateOnly("2026-01-07")
	check(fault == nil && date == hypercast.Date{Year: 2026, Month: time.January, Day: 7}, "DateOnly")

	civil, fault := hypercast.DateTime("1/7/2026 3:04 PM", hypercast.MonthDayYear)
	check(fault == nil && civil.Date == hypercast.Date{Year: 2026, Month: time.January, Day: 7} &&
		civil.TimeOfDay == 15*time.Hour+4*time.Minute, "DateTime 12-hour")

	german := hypercast.DateFormat{Order: hypercast.DayMonthYear, AM: "vorm.", PM: "nachm."}
	civil, fault = hypercast.DateTimeWith("1/7/2026 3:04 nachm.", german)
	check(fault == nil && civil.Date == hypercast.Date{Year: 2026, Month: time.July, Day: 1} &&
		civil.TimeOfDay == 15*time.Hour+4*time.Minute, "DateTimeWith declared day periods")
	_, fault = hypercast.DateTime("1/7/2026 3:04 nachm.", hypercast.DayMonthYear)
	check(fault != nil && fault.Reason == hypercast.Malformed, "DateTime reads English only")

	tod, fault := hypercast.TimeOfDay("23:59:59.5")
	check(fault == nil && tod == 23*time.Hour+59*time.Minute+59*time.Second+500*time.Millisecond, "TimeOfDay")

	span, fault := hypercast.Span("PT1H30M15.5S")
	check(fault == nil && span.Seconds == 5415 && span.Nanos == 500000000, "Span")

	tenth, fifth := 0.1, 0.2 // variables: Go adds the constants 0.1 + 0.2 exactly, to 0.3
	fromDouble, fault := hypercast.ExactFromFloat64(tenth + fifth)
	check(fault == nil && fromDouble.String() == "0.30000000000000004", "ExactFromFloat64")

	serial, fault := hypercast.ExcelSerialFromFloat64(45292.75, hypercast.Excel1900)
	check(fault == nil && serial.Date == hypercast.Date{Year: 2024, Month: time.January, Day: 1} &&
		serial.TimeOfDay == 18*time.Hour, "ExcelSerialFromFloat64 1900")

	noon, fault := hypercast.ExcelTime(0.5)
	check(fault == nil && noon == 12*time.Hour, "ExcelTime")

	elapsed, fault := hypercast.ExcelDuration(1.5)
	check(fault == nil && elapsed.Seconds == 129600 && elapsed.Nanos == 0, "ExcelDuration")

	text, fault := hypercast.String("  Male ")
	check(fault == nil && text == "  Male ", "String")

	latitude := hypercast.MustConstraints[float64](
		hypercast.Minimum(-90), hypercast.Maximum(90),
		hypercast.MultipleOf("0.000001"), hypercast.WithRounding(hypercast.Midpoint))
	lat, rejection := hypercast.Check("45.1234567", hypercast.Invariant, latitude)
	check(rejection == nil && lat.Value == 45.123457 && lat.Adjusted, "Check rounds to multipleOf")
	_, rejection = hypercast.Check("-90.5", hypercast.Invariant, latitude)
	violation, ok := rejection.(*hypercast.Violation[float64])
	check(ok && violation.Keyword == hypercast.KeywordMinimum && violation.Value == -90.5 &&
		violation.Offset == 0 && violation.Length == 5, "Check violation keyword, value and span")
	lat, rejection = hypercast.CheckFromFloat64(tenth+fifth, hypercast.MustConstraints[float64](
		hypercast.MultipleOf("0.1"), hypercast.WithRounding(hypercast.Midpoint)))
	check(rejection == nil && lat.Value == 0.3 && lat.Adjusted, "CheckFromFloat64")

	codes := hypercast.MustConstraints[int32](hypercast.Enum(10, 20))
	_, rejection = hypercast.Check("30", hypercast.Invariant, codes)
	enumViolation, ok := rejection.(*hypercast.Violation[int32])
	check(ok && enumViolation.Keyword == hypercast.KeywordEnum && enumViolation.Value == 30, "Check integer enum")

	sex := hypercast.MustConstraints[string](hypercast.Trim(), hypercast.Enum("Female", "Male", "Other"))
	code, rejection := hypercast.CheckString(" MALE ", sex)
	check(rejection == nil && code.Value == "Male" && code.EnumIndex == 1, "CheckString text enum")

	units := hypercast.MustConstraints[string](hypercast.Trim(), hypercast.CaseSensitive(), hypercast.Enum("mm", "Mm"))
	_, rejection = hypercast.CheckString(" MM ", units)
	textViolation, ok := rejection.(*hypercast.Violation[string])
	check(ok && textViolation.Keyword == hypercast.KeywordEnum && textViolation.Value == "MM" &&
		textViolation.Offset == 1 && textViolation.Length == 2, "CheckString case-sensitive enum violation")

	_, err := hypercast.NewConstraints[int32](hypercast.MultipleOf(0))
	check(err != nil && err.Error() == "hypercast: Constraints refused (code 5): multipleOf is not positive.",
		"NewConstraints refusal")

	fmt.Println("DONE")
}
