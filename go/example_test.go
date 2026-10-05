package hypercast_test

import (
	"fmt"
	"log"

	// The import path ends in /go, so the package name has to be spelled out.
	hypercast "github.com/SkunkWerkx/HyperCast/go"
)

func ExampleI32() {
	// Accounting parentheses and grouping, under the declared invariant format.
	value, fault := hypercast.I32("(1,234)", hypercast.Invariant)
	if fault != nil {
		log.Fatal(fault)
	}
	fmt.Println(value)
	// Output: -1234
}

func ExampleI32_fault() {
	// A verdict, not an exception: the closed reason and the byte span that offended.
	_, fault := hypercast.I32("  12x4", hypercast.Invariant)
	fmt.Println(fault.Reason, fault.Offset, fault.Length)
	fmt.Println(fault)

	_, fault = hypercast.U8("256", hypercast.Invariant)
	fmt.Println(fault.Reason == hypercast.OutOfRange)
	// Output:
	// malformed 4 1
	// hypercast: malformed at byte 4..5
	// true
}

func ExampleBool() {
	yes, _ := hypercast.Bool("yes")
	off, _ := hypercast.Bool([]byte("OFF")) // string and []byte both cross zero-copy
	_, fault := hypercast.Bool("maybe")
	fmt.Println(yes, off, fault.Reason)
	// Output: true false malformed
}

func ExampleF64() {
	// The caller declares the notation; nothing is guessed from the input.
	euros := hypercast.NumFormat{DecimalSep: ',', GroupSep: '.', Styles: hypercast.AllStyles, Currency: "€"}
	value, fault := hypercast.F64("€ 1.234,50", euros)
	if fault != nil {
		log.Fatal(fault)
	}
	fmt.Println(value)
	// Output: 1234.5
}

func ExampleF64_detect() {
	// Detect resolves the separators' roles from the input's own structure — and refuses
	// to guess when the structure does not settle it.
	value, _ := hypercast.F64("1.234.567,89", hypercast.Detect)
	_, fault := hypercast.F64("1,000", hypercast.Detect)
	fmt.Println(value, fault.Reason)
	// Output: 1.23456789e+06 malformed
}

func ExampleExact() {
	dollars := hypercast.NumFormat{DecimalSep: '.', GroupSep: ',', Styles: hypercast.AllStyles, Currency: "$"}
	d, fault := hypercast.Exact("($1,234.50)", dollars)
	if fault != nil {
		log.Fatal(fault)
	}
	// Never rounded, and canonical: the trailing zero is trimmed.
	fmt.Println(d, d.Scale, d.Negative)
	// Output: -1234.5 1 true
}

func ExampleNumeric() {
	// One door generic over the target: Numeric[int32] is I32, Numeric[Decimal] is Exact.
	n, _ := hypercast.Numeric[int32]("42", hypercast.Invariant)
	f, _ := hypercast.Numeric[float64]("50%", hypercast.Invariant)
	d, _ := hypercast.Numeric[hypercast.Decimal]("1.10", hypercast.Invariant)
	fmt.Println(n, f, d)
	// Output: 42 0.5 1.1
}

func ExampleUuid() {
	id, fault := hypercast.Uuid("urn:uuid:01020304-0506-0708-090a-0b0c0d0e0f10")
	if fault != nil {
		log.Fatal(fault)
	}
	fmt.Println(id)
	// Output: 01020304-0506-0708-090a-0b0c0d0e0f10
}

func ExampleTimestamp() {
	// RFC 3339, zone mandatory, normalized to UTC at full nanosecond fidelity.
	ts, fault := hypercast.Timestamp("2026-01-02T15:04:05.123456789+05:00")
	if fault != nil {
		log.Fatal(fault)
	}
	fmt.Println(ts)
	// Output: 2026-01-02 10:04:05.123456789 +0000 UTC
}

func ExampleUnix() {
	// The unit is declared, never inferred from the magnitude.
	fromSeconds, _ := hypercast.Unix("1700000000", hypercast.Seconds)
	fromMillis, _ := hypercast.Unix("1700000000000", hypercast.Milliseconds)
	fmt.Println(fromSeconds, fromSeconds.Equal(fromMillis))
	// Output: 2023-11-14 22:13:20 +0000 UTC true
}

func ExampleExcelSerial() {
	ts, fault := hypercast.ExcelSerial("45292.75", hypercast.Excel1900)
	if fault != nil {
		log.Fatal(fault)
	}
	fmt.Println(ts)
	// Output: 2024-01-01 18:00:00 +0000 UTC
}

func ExampleExactFromFloat64() {
	// The digits a spreadsheet writes for the double, not the binary fraction nearest it.
	// (Two variables, because Go adds the constants 0.1 + 0.2 exactly, to 0.3.)
	a, b := 0.1, 0.2
	d, fault := hypercast.ExactFromFloat64(a + b)
	if fault != nil {
		log.Fatal(fault)
	}
	fmt.Println(d)
	// Output: 0.30000000000000004
}

func ExampleExcelSerialFromFloat64() {
	civil, fault := hypercast.ExcelSerialFromFloat64(45292.75, hypercast.Excel1900)
	if fault != nil {
		log.Fatal(fault)
	}
	fmt.Println(civil.Date.Year, civil.Date.Month, civil.Date.Day, civil.TimeOfDay)
	// Output: 2024 January 1 18h0m0s
}

func ExampleDateOnlyOrdered() {
	// The same text is January 7th or July 1st only because the caller said which.
	us, _ := hypercast.DateOnlyOrdered("1/7/2026", hypercast.MonthDayYear)
	gb, _ := hypercast.DateOnlyOrdered("1/7/2026", hypercast.DayMonthYear)
	fmt.Println(us.Month, us.Day)
	fmt.Println(gb.Month, gb.Day)
	// Output:
	// January 7
	// July 1
}

func ExampleDateTime() {
	// Zone-less text names a wall-clock date and time, not an instant — so that is what
	// comes back.
	civil, fault := hypercast.DateTime("1/7/2026 3:04 PM", hypercast.MonthDayYear)
	if fault != nil {
		log.Fatal(fault)
	}
	fmt.Println(civil.Date.Year, civil.Date.Month, civil.Date.Day, civil.TimeOfDay)
	// Output: 2026 January 7 15h4m0s
}

func ExampleTimeOfDay() {
	sinceMidnight, fault := hypercast.TimeOfDay("15:04:05.5")
	if fault != nil {
		log.Fatal(fault)
	}
	fmt.Println(sinceMidnight)
	// Output: 15h4m5.5s
}

func ExampleSpan() {
	span, fault := hypercast.Span("PT1H30M15.5S")
	if fault != nil {
		log.Fatal(fault)
	}
	fmt.Println(span.Seconds, span.Nanos)

	// The checked converter: time.Duration tops out near 292 years, the core's window
	// does not.
	std, ok := span.AsDuration()
	fmt.Println(std, ok)
	// Output:
	// 5415 500000000
	// 1h30m15.5s true
}

func ExampleNativeVersion() {
	// The version the linked core reports about itself, "major.minor.patch".
	log.Printf("hypercast core %s", hypercast.NativeVersion())
}
