package hypercast

// The README's zero-allocation claim, held as tests rather than as a benchmark somebody
// has to remember to read. The by-value shims in backend_static.go are what make these zero.

import "testing"

func assertAllocs(t *testing.T, name string, want float64, f func()) {
	t.Helper()
	if got := testing.AllocsPerRun(100, f); got != want {
		t.Errorf("%s: %v allocs per call, want %v", name, got, want)
	}
}

// succeed fails the test if a door that should have accepted its input did not — an
// allocation count for the wrong path would prove nothing.
func succeed(t *testing.T, name string, fault *Fault) {
	t.Helper()
	if fault != nil {
		t.Fatalf("%s: %v", name, fault)
	}
}

func TestEveryDoorIsAllocationFreeOnSuccess(t *testing.T) {
	doors := []struct {
		name string
		call func() *Fault
	}{
		{"Bool", func() *Fault { _, f := Bool("true"); return f }},
		{"I8", func() *Fault { _, f := I8("-128", Invariant); return f }},
		{"I16", func() *Fault { _, f := I16("-32,768", Invariant); return f }},
		{"I32", func() *Fault { _, f := I32("1,234,567", Invariant); return f }},
		{"I64", func() *Fault { _, f := I64("9223372036854775807", Invariant); return f }},
		{"U8", func() *Fault { _, f := U8("255", Invariant); return f }},
		{"U16", func() *Fault { _, f := U16("0xFFFF", Invariant); return f }},
		{"U32", func() *Fault { _, f := U32("4,294,967,295", Invariant); return f }},
		{"U64", func() *Fault { _, f := U64("18446744073709551615", Invariant); return f }},
		{"F32", func() *Fault { _, f := F32("1.5e3", Invariant); return f }},
		{"F64", func() *Fault { _, f := F64("12345.6789", Invariant); return f }},
		{"F64 (Detect)", func() *Fault { _, f := F64("1.234.567,89", Detect); return f }},
		{"Exact", func() *Fault { _, f := Exact("1,234.50", Invariant); return f }},
		{"Uuid", func() *Fault { _, f := Uuid("01020304-0506-0708-090a-0b0c0d0e0f10"); return f }},
		{"Timestamp", func() *Fault { _, f := Timestamp("2026-01-02T15:04:05.123456789Z"); return f }},
		{"Unix", func() *Fault { _, f := Unix("1700000000", Seconds); return f }},
		{"ExcelSerial", func() *Fault { _, f := ExcelSerial("45292.75", Excel1900); return f }},
		{"DateOnly", func() *Fault { _, f := DateOnly("2026-01-07"); return f }},
		{"DateOnlyOrdered", func() *Fault { _, f := DateOnlyOrdered("1/7/2026", MonthDayYear); return f }},
		{"DateTime", func() *Fault { _, f := DateTime("1/7/2026 3:04 PM", MonthDayYear); return f }},
		{"TimeOfDay", func() *Fault { _, f := TimeOfDay("15:04:05.123456789"); return f }},
		{"Span", func() *Fault { _, f := Span("PT1H30M15.5S"); return f }},
		{"ExactFromFloat64", func() *Fault { _, f := ExactFromFloat64(0.1 + 0.2); return f }},
		{"ExcelSerialFromFloat64", func() *Fault { _, f := ExcelSerialFromFloat64(45292.75, Excel1900); return f }},
		{"ExcelTime", func() *Fault { _, f := ExcelTime(0.75); return f }},
		{"ExcelDuration", func() *Fault { _, f := ExcelDuration(1.5); return f }},
	}
	for _, door := range doors {
		succeed(t, door.name, door.call())
		assertAllocs(t, door.name, 0, func() { door.call() })
	}
}

// A declared currency symbol rides inside the NumFormat by value, so it costs nothing either.
func TestCurrencyFormatIsAllocationFree(t *testing.T) {
	dollars := NumFormat{DecimalSep: '.', GroupSep: ',', Styles: AllStyles, Currency: "$"}
	_, fault := Exact("($1,234.50)", dollars)
	succeed(t, "Exact with currency", fault)
	assertAllocs(t, "Exact with currency", 0, func() { Exact("($1,234.50)", dollars) })
}

// []byte crosses the same way string does: a pointer into memory that already exists.
func TestByteInputIsAllocationFree(t *testing.T) {
	text := []byte("1,234,567")
	_, fault := I32(text, Invariant)
	succeed(t, "I32([]byte)", fault)
	assertAllocs(t, "I32([]byte)", 0, func() { I32(text, Invariant) })
}

// Numeric's dispatch switches on a pointer to the out-value precisely so that it adds
// nothing to the door it forwards to.
func TestNumericDispatchIsAllocationFree(t *testing.T) {
	assertAllocs(t, "Numeric[int32]", 0, func() { Numeric[int32]("1,234,567", Invariant) })
	assertAllocs(t, "Numeric[int64]", 0, func() { Numeric[int64]("9223372036854775807", Invariant) })
	assertAllocs(t, "Numeric[float64]", 0, func() { Numeric[float64]("12345.6789", Invariant) })
	assertAllocs(t, "Numeric[Decimal]", 0, func() { Numeric[Decimal]("1,234.50", Invariant) })
}

// The zero is the success path's. A failed cast returns a *Fault, and that pointer is the
// one allocation it makes.
func TestAFaultIsTheOnlyAllocationOnFailure(t *testing.T) {
	if _, fault := I32("12x4", Invariant); fault == nil {
		t.Fatal("expected a fault")
	}
	assertAllocs(t, "I32 (malformed)", 1, func() { I32("12x4", Invariant) })
	assertAllocs(t, "Timestamp (malformed)", 1, func() { Timestamp("not a timestamp") })
}
