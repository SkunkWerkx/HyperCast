// Command tinygosmoke is the check CI runs in headless Chrome: this module compiled by TinyGo
// for the browser (`tinygo build -target=wasm`), with the core linked in from
// staticlib/wasm, loaded by index.html beside it through TinyGo's own wasm_exec.js. It
// prints one PASS or FAIL line per check and DONE at the end; the forge's
// hyper-build-wasm.yml fails the run on any FAIL or a missing DONE, which is also what a
// trap part-way through looks like.
//
// Every ABI shape crosses at least once — a plain door (Bool, Uuid, Timestamp, DateOnly,
// TimeOfDay, Span), a numeric one with a format (I32, F64, Exact with a currency symbol), a
// discriminated one (Unix, ExcelSerial, DateTime) — and so does a fault with its span,
// since TinyGo hands the core pointers into Go memory for the out-value and the fault
// rather than the by-value shims the cgo backend uses.
//
// The expected core version comes from rust/Cargo.toml at build time
// (`-ldflags "-X main.wantVersion=..."`), so the page proves the archive that was linked is
// the one this commit's core builds, not a stale one. Stock Go builds this too, natively
// under cgo, so `go vet ./...` covers it like any other package; it is internal, so it is
// nothing a consumer can import.
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

	dollars := hypercast.NumFormat{DecimalSep: '.', GroupSep: ',', Styles: hypercast.AllStyles, Currency: "$"}
	d, fault := hypercast.Exact("($1,234.50)", dollars)
	check(fault == nil && d.String() == "-1234.5" && d.Scale == 1 && d.Negative, "Exact with a currency symbol")

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

	fmt.Println("DONE")
}
