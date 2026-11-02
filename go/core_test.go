package hypercast

// Portable tests of the translated-core backend: no build tag and no os/exec, so they run
// under stock Go on every port, under TinyGo and in the browser page's runtimes.

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"math"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"
	"unsafe"

	"github.com/SkunkWerkx/HyperCast/go/internal/core"
	"github.com/google/uuid"
)

// withFreshInstances replaces the primary with a new instance and the pool with an empty one
// (no call may be in flight), returns the instance count after that, and restores both on
// cleanup. Tests assert deltas against the returned baseline, never absolute counts.
func withFreshInstances(t testing.TB) (baseline int64) {
	t.Helper()
	old := primary()
	thePrimary = mustNewInstance()
	resetPool()
	t.Cleanup(func() {
		thePrimary = old
		resetPool()
	})
	return instancesCreated.Load()
}

func TestTheModuleIsItsOwnTwoPages(t *testing.T) {
	if got := len(*core.New().Xmemory().Slice()); got != modulePages*pageSize {
		t.Fatalf("a fresh module holds %d bytes, want modulePages (%d) pages", got, modulePages)
	}
	in := mustNewInstance()
	if in.scratch != modulePages*pageSize || in.pages != basePages || len(in.mem()) != (modulePages+int(basePages))*pageSize {
		t.Fatalf("a fresh instance: scratch at %d, %d pages, %d bytes", in.scratch, in.pages, len(in.mem()))
	}
}

func TestScratchPagesFor(t *testing.T) {
	cases := []struct {
		n     uint64
		pages uint32
		ok    bool
	}{
		{0, 1, true},
		{uint64(baseText), 1, true},
		{uint64(baseText) + 1, 2, true},
		{uint64(hostMaxPages-modulePages)*pageSize - textOffset, hostMaxPages - modulePages, true},
		{uint64(hostMaxPages-modulePages)*pageSize - textOffset + 1, 0, false},
		{math.MaxUint64 - textOffset, 0, false},
	}
	for _, c := range cases {
		pages, ok := scratchPagesFor(c.n)
		if ok != c.ok || (ok && pages != c.pages) {
			t.Errorf("scratchPagesFor(%d) = %d, %v; want %d, %v", c.n, pages, ok, c.pages, c.ok)
		}
	}
}

func TestMaxTextIsTheSmallerBound(t *testing.T) {
	if strconv.IntSize == 64 && MaxText != 1<<31-1 {
		t.Fatalf("MaxText = %d on a 64-bit host, want %d", MaxText, 1<<31-1)
	}
	if _, ok := scratchPagesFor(MaxText); !ok {
		t.Fatal("a text of MaxText bytes does not fit the scratch")
	}
}

func TestATextPastMaxTextPanicsBeforeAnythingIsMade(t *testing.T) {
	base := withFreshInstances(t)
	defer func() {
		r := recover()
		message, _ := r.(string)
		for _, want := range []string{"MaxText", strconv.Itoa(MaxText + 1), strconv.Itoa(MaxText)} {
			if !strings.Contains(message, want) {
				t.Fatalf("panic %q does not name %s", r, want)
			}
		}
		if d := instancesCreated.Load() - base; d != 0 {
			t.Fatalf("a refused text created %d instances", d)
		}
	}()
	acquireFor(uintptr(MaxText) + 1)
	t.Fatal("no panic")
}

// A checked door's text and declaration together are bounded by MaxText, and the panic says
// so, naming the text's own length and the declaration's apart from their sum.
func TestACheckedTextPastMaxTextNamesItsDeclaration(t *testing.T) {
	base := withFreshInstances(t)
	n := uintptr(MaxText) - 10
	extra := declarationAt(n) - textOffset - n + rawConstraintsBytes + rawCheckBytes
	defer func() {
		message, _ := recover().(string)
		want := fmt.Sprintf("hypercast: a text of %d bytes with a %d-byte declaration past it (%d bytes in all) exceeds MaxText (%d)",
			n, extra, n+extra, MaxText)
		if message != want {
			t.Fatalf("panic %q, want %q", message, want)
		}
		if d := instancesCreated.Load() - base; d != 0 {
			t.Fatalf("a refused text created %d instances", d)
		}
	}()
	acquireChecked(n, &declaration{raw: RawConstraints{Size: RawConstraintsSize}})
	t.Fatal("no panic")
}

// Texts either side of the base scratch's end: whitespace the core trims, around a value
// it reads, and a fault whose span lies past the first page. The longer one runs on a one-off
// instance, and the primary keeps its one page.
func TestTextsAtTheScratchBoundary(t *testing.T) {
	base := withFreshInstances(t)
	for i, n := range []int{baseText, baseText + 1, 3*pageSize + 5} {
		pad := strings.Repeat(" ", n-2)
		v, fault := I32(pad+"42", Invariant) // n bytes, and so is the faulting text below
		if fault != nil || v != 42 {
			t.Fatalf("I32 of %d bytes = %d, %v", n, v, fault)
		}
		_, fault = I32([]byte(pad+"4x"), Invariant)
		if fault == nil || fault.Reason != Malformed || fault.Offset != n-1 || fault.Length != 1 {
			t.Fatalf("I32 of %d bytes ending in 4x: %v, want malformed at %d..%d", n, fault, n-1, n)
		}
		if pages := primary().pages; pages != basePages {
			t.Fatalf("after a text of %d bytes the primary has %d scratch pages", n, pages)
		}
		want := int64(0)
		if i > 0 {
			want = int64(2 * i) // two one-off instances per longer size
		}
		if got := instancesCreated.Load() - base; got != want {
			t.Fatalf("after texts of %d bytes: %d instances created, want %d", n, got, want)
		}
	}
}

// A declared format and day periods cross beside a long text on a one-off instance too.
func TestAOneOffInstanceCarriesTheFormatAndThePeriods(t *testing.T) {
	pad := strings.Repeat(" ", baseText)
	euros := NumFormat{DecimalSep: ",", GroupSep: ".", Styles: AllStyles, Currency: "€"}
	if v, fault := F64(pad+"€ 1.234,50", euros); fault != nil || v != 1234.5 {
		t.Fatalf("F64 = %v, %v", v, fault)
	}
	german := DateFormat{Order: DayMonthYear, AM: "vorm.", PM: "nachm."}
	got, fault := DateTimeWith(pad+"1.7.2026 3:04 nachm.", german)
	want, fault2 := DateTimeWith("1.7.2026 3:04 nachm.", german)
	if fault != nil || fault2 != nil || got != want || got.TimeOfDay != 15*time.Hour+4*time.Minute {
		t.Fatalf("DateTimeWith = %v, %v; short text %v, %v", got, fault, want, fault2)
	}
}

// The scratch is reused, so a call must never read a value an earlier one left there: a
// fault after a success reports its own span, and a success after a fault its own value.
func TestNothingCarriesOverBetweenCalls(t *testing.T) {
	withFreshInstances(t)
	if v, fault := I64("9223372036854775807", Invariant); fault != nil || v != math.MaxInt64 {
		t.Fatalf("I64 = %d, %v", v, fault)
	}
	if v, fault := U8("7", Invariant); fault != nil || v != 7 {
		t.Fatalf("U8 = %d, %v", v, fault)
	}
	if _, fault := U8("1x", Invariant); fault == nil || fault.Offset != 1 || fault.Length != 1 {
		t.Fatalf("U8(1x) = %v", fault)
	}
	if _, fault := Bool(""); fault == nil || fault.Reason != Empty || fault.Offset != 0 || fault.Length != 0 {
		t.Fatalf("Bool(\"\") = %v", fault)
	}
	if v, fault := Bool("no"); fault != nil || v {
		t.Fatalf("Bool(no) = %v, %v", v, fault)
	}
}

// decodeLE is what read does on a big-endian host. On a little-endian one the in-place view
// is the core's own layout, so every shape a door reads must decode to exactly that view.
func TestDecodeLEMatchesTheInPlaceView(t *testing.T) {
	if bigEndian {
		t.Skip("the in-place view is the reference only on a little-endian host")
	}
	var b [16]byte
	for i := range b {
		b[i] = byte(0xA5 ^ i*37)
	}
	r := result{}
	copy((*[16]byte)(unsafe.Pointer(&r.out))[:], b[:])
	check := func(name string, ok bool) {
		t.Helper()
		if !ok {
			t.Errorf("%s: decodeLE differs from the in-place view", name)
		}
	}
	check("uint8", decodeLE[uint8](&b) == *(*uint8)(unsafe.Pointer(&r.out)))
	check("int8", decodeLE[int8](&b) == *(*int8)(unsafe.Pointer(&r.out)))
	check("uint16", decodeLE[uint16](&b) == *(*uint16)(unsafe.Pointer(&r.out)))
	check("int16", decodeLE[int16](&b) == *(*int16)(unsafe.Pointer(&r.out)))
	check("uint32", decodeLE[uint32](&b) == *(*uint32)(unsafe.Pointer(&r.out)))
	check("int32", decodeLE[int32](&b) == *(*int32)(unsafe.Pointer(&r.out)))
	check("uint64", decodeLE[uint64](&b) == *(*uint64)(unsafe.Pointer(&r.out)))
	check("int64", decodeLE[int64](&b) == *(*int64)(unsafe.Pointer(&r.out)))
	check("float32", math.Float32bits(decodeLE[float32](&b)) == *(*uint32)(unsafe.Pointer(&r.out)))
	check("float64", math.Float64bits(decodeLE[float64](&b)) == *(*uint64)(unsafe.Pointer(&r.out)))
	check("rawDecimal", decodeLE[rawDecimal](&b) == *(*rawDecimal)(unsafe.Pointer(&r.out)))
	check("RawTimestamp", decodeLE[RawTimestamp](&b) == *(*RawTimestamp)(unsafe.Pointer(&r.out)))
	check("RawDate", decodeLE[RawDate](&b) == *(*RawDate)(unsafe.Pointer(&r.out)))
	check("RawCivil", decodeLE[RawCivil](&b) == *(*RawCivil)(unsafe.Pointer(&r.out)))
	check("RawSpan", decodeLE[RawSpan](&b) == *(*RawSpan)(unsafe.Pointer(&r.out)))
	check("uuid.UUID", decodeLE[uuid.UUID](&b) == *(*uuid.UUID)(unsafe.Pointer(&r.out)))
	// Control: a shape decodeLE does not know is refused, not silently zeroed.
	func() {
		defer func() {
			if recover() == nil {
				t.Error("decodeLE[string] did not panic")
			}
		}()
		decodeLE[string](&b)
	}()
}

// The words a format and the day periods carry are written in the guest's order whatever the
// host's: the bytes in the scratch are the little-endian encoding of each word.
func TestTheFormatAndPeriodsWordsAreLittleEndianInTheScratch(t *testing.T) {
	withFreshInstances(t)
	in, how := acquire()
	defer release(in, how)
	var raw RawNumFormat
	NumFormat{DecimalSep: ",", GroupSep: ".", Styles: AllStyles | TrailingSign, DigitZero: '٠'}.raw(&raw)
	in.loadFormat(&raw)
	got := in.mem()[in.scratch+formatOffset:]
	for i, word := range []uint32{raw.Size, raw.Flags, raw.DigitZero} {
		if v := binary.LittleEndian.Uint32(got[4*i:]); v != word {
			t.Errorf("format word %d is %#x in the scratch, want %#x", i, v, word)
		}
	}
	if !bytes.Equal(got[12:92], (*[92]byte)(unsafe.Pointer(&raw))[12:]) {
		t.Error("the format's symbols differ in the scratch")
	}
	var periods RawDayPeriods
	DateFormat{Order: DayMonthYear, AM: "vorm.", PM: "nachm."}.raw(&periods)
	in.loadPeriods(&periods)
	got = in.mem()[in.scratch+periodsOffset:]
	if v := binary.LittleEndian.Uint32(got); v != periods.Size {
		t.Errorf("the periods' size word is %#x in the scratch, want %#x", v, periods.Size)
	}
}

// Every door from many goroutines at once, long texts among them: each verdict is its own.
func TestDoorsFromManyGoroutines(t *testing.T) {
	var wg sync.WaitGroup
	long := strings.Repeat(" ", baseText) + "-5"
	for g := range 8 {
		wg.Add(1)
		go func(g int) {
			defer wg.Done()
			for i := range 300 {
				n := g*1000 + i
				text := strconv.Itoa(n)
				if i%50 == 0 {
					text = long
					n = -5
				}
				v, fault := I64(text, Invariant)
				if fault != nil || v != int64(n) {
					t.Errorf("I64(%q) = %d, %v", text[max(0, len(text)-8):], v, fault)
					return
				}
				if _, fault := U8(text+"x", Invariant); fault == nil || fault.Offset+fault.Length > len(text)+1 {
					t.Errorf("U8(%q) = %v", text[max(0, len(text)-8):]+"x", fault)
					return
				}
			}
		}(g)
	}
	wg.Wait()
}

// The checked doors from many goroutines at once, each with declarations and enum lists of
// its own, while the primary is held, so every call runs on a pooled instance (or a one-off
// for the long texts): each verdict is its own goroutine's, the declaration laid out past
// each instance's text included.
func TestCheckedDoorsFromManyGoroutines(t *testing.T) {
	type declared struct {
		codes Constraints[int32]
		step  Constraints[float64]
		words Constraints[string]
	}
	var all [8]declared
	for g := range all {
		all[g] = declared{
			codes: MustConstraints[int32](Enum(g, g+100, g+200), Maximum(g+200)),
			step:  MustConstraints[float64](MultipleOf(fmt.Sprintf("0.%0*d", g+1, 5)), WithRounding(Floor)),
			words: MustConstraints[string](Trim(), Enum(fmt.Sprintf("g%d-alpha", g), fmt.Sprintf("g%d-beta", g))),
		}
	}
	base := withFreshInstances(t)
	thePrimary.mu.Lock()
	defer thePrimary.mu.Unlock()
	long := strings.Repeat(" ", baseText)
	var wg sync.WaitGroup
	for g := range all {
		wg.Add(1)
		go func(g int) {
			defer wg.Done()
			codes, step, words := all[g].codes, all[g].step, all[g].words
			for i := range 200 {
				pad := ""
				if i%50 == 0 {
					pad = long
				}
				want := int32(g + 100*(i%3))
				got, rejection := Check(pad+strconv.Itoa(int(want)), Invariant, codes)
				if rejection != nil || got.Value != want {
					t.Errorf("goroutine %d: Check(%d) = %v, %v", g, want, got, rejection)
					return
				}
				if _, rejection := Check(strconv.Itoa(g+1), Invariant, codes); rejection == nil {
					t.Errorf("goroutine %d: %d matched its enum", g, g+1)
					return
				}
				f, rejection := Check("1", Invariant, step)
				if rejection != nil || f.Value != 1 || f.Adjusted {
					t.Errorf("goroutine %d: Check(1) by step 0.%0*d = %v, %v", g, g+1, 5, f, rejection)
					return
				}
				entry := fmt.Sprintf("g%d-beta", g)
				w, rejection := CheckString(pad+strings.ToUpper(entry)+" ", words)
				if rejection != nil || w.Value != entry || w.EnumIndex != 1 {
					t.Errorf("goroutine %d: CheckString(%s) = %+v, %v", g, entry, w, rejection)
					return
				}
				if _, rejection := CheckString(fmt.Sprintf("g%d-beta", (g+1)%8), words); rejection == nil {
					t.Errorf("goroutine %d: another goroutine's entry matched", g)
					return
				}
			}
		}(g)
	}
	wg.Wait()
	if d := instancesCreated.Load() - base; d < 1 {
		t.Errorf("the checked doors created %d instances with the primary held: the pool went unused", d)
	}
}
