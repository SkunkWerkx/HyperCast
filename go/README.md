# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Go Reference](https://pkg.go.dev/badge/github.com/SkunkWerkx/HyperCast/go.svg)](https://pkg.go.dev/github.com/SkunkWerkx/HyperCast/go)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**Go's own union idiom — `(value, *Fault)` — carrying the verdict of every cast: the value,
or a closed reason plus the exact byte span that offended. `*Fault` implements `error` for
composition, but the doors never panic on input; a panic here means a caller bug, never
data.**

Allocation-lean scalar casts — booleans, characters, the full integer family, reals, exact
decimals, UUIDs, temporals — from the `hypercast` Rust core **compiled as Go source**: the
core built for `wasm32-wasip1`, translated to one Go file by
[wasm2go](https://github.com/ncruces/wasm2go) and committed as
[`internal/core/core.go`](internal/core/core.go). The Go compiler builds it like any other
package, so there is no cgo, no C compiler, no archive in git, nothing embedded, nothing
extracted, nothing `dlopen`ed and nothing that can fail to load. `go get` is the whole
install, `CGO_ENABLED=0` is fine, and the module builds for every GOOS and GOARCH Go has,
the browser and WASI included, under stock Go and TinyGo alike.

```go
package main

import (
	"fmt"

	hypercast "github.com/SkunkWerkx/HyperCast/go"
)

func main() {
	value, fault := hypercast.I32("(1,234)", hypercast.Invariant)
	if fault != nil {
		fmt.Printf("%s at byte %d\n", fault.Reason, fault.Offset)
	} else {
		fmt.Println("got", value) // -1234, accounting negative
	}

	ts, fault := hypercast.Timestamp("2026-01-02T15:04:05.123456789+05:00")
	if fault != nil {
		fmt.Printf("%s at byte %d\n", fault.Reason, fault.Offset)
	} else {
		fmt.Println("timestamp", ts) // a UTC time.Time at full nanosecond fidelity
	}
}
```

## Install

```sh
go get github.com/SkunkWerkx/HyperCast/go@latest
```

Requires Go 1.26 or later (the `go` directive in `go.mod`); no C compiler, and `CGO_ENABLED`
makes no difference. The import path ends in `/go`
while the package is named `hypercast`, so spell the name out in the import, as above —
`goimports` adds it for you, but a hand-written `import "github.com/SkunkWerkx/HyperCast/go"`
reads as if the package were called `go`.

Go modules have no separate registry — `go get` resolves straight from a git tag, and because
this module lives in a monorepo subdirectory its tags are prefixed (`go/v<version>`). What
`go get` fetches is source, the translated core included, so nothing is staged for a release
but the core regenerated at the new version. `github.com/google/uuid` is the module's only
dependency.

## What this binding is

The Rust core is one `wasm32-wasip1` module of about 430 KB that imports nothing at all:
parsing needs no clock, no randomness and no I/O. wasm2go translates it into about 3.4 MB of
Go source with no dependencies beyond the standard library, and that file is committed and
compiled with the rest of the package. The binding keeps an *instance* of it: the
translated module and a scratch region in the module's linear memory, where a door's text,
its `NumFormat` and its day periods are copied in and its value and fault span copied out.
Every door is an ordinary Go call into that code; nothing crosses a foreign boundary.

wasm2go's generated code bounds-checks every memory access ([`internal/core/bounds_test.go`](internal/core/bounds_test.go) pins the checks) and keeps the call stack in Go,
so a core bug, a guest stack overflow included, is a Go panic and never a corrupted heap. The
guest's stack is 64 KiB: regenerating the core at smaller stack sizes and running the whole
suite, the corpus replay included, at each, 4608 bytes is the smallest size that passes
everything and 4096 the first that traps (a bounds panic from the generated code, never a
wrong verdict), so the shipped size is a 14x margin over the tested paths, measured rather
than proved. A call that panics inside the core abandons the instance it ran on, whose guest
stack it left mid-call, rather than hand it to the next caller.

wasm2go is pre-1.0 (v0.4.21 today) with one maintainer. It is pinned in a tool module of its
own, [`internal/wasm2go/`](internal/wasm2go), so it never enters your module graph, and
because its output is committed, a dead wasm2go would cost this repository its regeneration
step and cost a consumer nothing: the binding keeps working from the committed file.

The guest is little-endian whatever the host is. Every word the binding writes into the
scratch or reads out of it is in the guest's order, and an out-value is reinterpreted in
place only on a little-endian host; a big-endian one (`s390x`, `ppc64`, `mips64`) decodes it
field by field.

## One primary and a pool

The core keeps no state between calls, so any instance serves any door. A call runs on one
*primary* instance when its lock is free, and otherwise on an instance from a `sync.Pool`,
created on demand. The primary owns nothing a pool could not hold; it is there because
`sync.Pool` drops what it holds across two GCs, and a pool alone would make a quiet
single-goroutine caller rebuild an instance after every other collection, an allocation on a
path that otherwise has none. The pool bounds nothing (it is per-P, cleared across two GCs),
so the number of live instances tracks the goroutines concurrently inside a door, about
`GOMAXPROCS`. The primary is created on first use; importing the package costs nothing.
Under TinyGo the scheduler is cooperative and a call never yields, so every call runs on the
primary.

What that costs under contention is the binding's one large loss: `I32` on 12 goroutines at
once is 20.4 ns a call in aggregate on the Apple M6, against 6.8 ns for the cgo binding this
replaced, which took no lock at all — **3.0x slower** ([Benchmarks](#benchmarks)). A single
goroutine's calls range from 1.5x faster than they were to 1.7x slower.

A fresh instance is 3 WebAssembly pages of linear memory, 192 KiB: the 64 KiB stack and the
core's data in 2 pages, plus one page of scratch.

## `MaxText`

A door's text is copied into the instance's scratch. The base scratch holds 65,360 bytes; a
longer text runs on a one-off instance sized for it and dropped after the call, so neither
the primary nor the pool keeps the pages, and that instance is the call's one allocation. The
core reads the text through a slice whose length may not exceed the guest's `isize::MAX`,
2^31 − 1 on wasm32, whatever the host's word size, so a door takes a text of at most
`MaxText` bytes: the smaller of that limit and what the guest and host can address beside the
module's own pages. On a 64-bit host that is 2,147,483,647. A longer text panics, as a caller
bug, before anything is allocated. A checked door lays its declaration (84 bytes and its
`enum` list), and a detailed date door its month vocabulary, out past its text in the same
scratch, so for them the text and what follows it together are what the base scratch must
hold and what `MaxText` bounds.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.

## Doors

Doors are generic over `string | []byte` — either is copied once into the core's memory,
and nothing else of it is touched.
`Uuid` returns [`google/uuid`](https://pkg.go.dev/github.com/google/uuid)'s `uuid.UUID`
(RFC 9562 order is exactly its own layout). Go-flavored fidelity, stated honestly both
ways: `time.Time` carries full nanoseconds across the whole 0001–9999 window and
time-of-day comes back nanosecond-exact, but `time.Duration`'s int64-nanosecond ceiling
(±292 years) sits far below the core's ±10,000-year duration window, so `Span` returns
the protobuf pair (`Duration{Seconds, Nanos}`) with a checked `AsDuration()` converter
rather than silently wrapping.

| Door | Returns | Declares |
| --- | --- | --- |
| `Bool` | `bool` | — |
| `Char` | `rune` — one character verbatim, or a declared code point (`U+00E9`, `0xE9`, `&H41`, `&#233;`, `&#x41;`, decimal `233`) | — |
| `I8` `I16` `I32` `I64` `U8` `U16` `U32` `U64` | the Go integer | `NumFormat` |
| `F32` `F64` | `float32` / `float64` | `NumFormat` |
| `Exact` | `Decimal` — sign, 96-bit magnitude, scale 0..=28 | `NumFormat` |
| `Uuid` | `uuid.UUID` | — |
| `Timestamp` | UTC `time.Time` | — |
| `Unix` | UTC `time.Time` | `UnixPrecision` |
| `ExcelSerial` | UTC `time.Time` | `ExcelEpoch` |
| `DateOnly` | `Date` | — |
| `DateOnlyOrdered` | `Date` | `DateOrder` |
| `DateTime` | `CivilDateTime` | `DateOrder` |
| `DateTimeWith` | `CivilDateTime` | `DateFormat` — the order plus the culture's AM/PM designators |
| `TimeOfDay` | `time.Duration` since midnight | — |
| `Span` | `Duration{Seconds, Nanos}` | — |
| `ExactFromFloat64` | `Decimal`, the shortest that names the `float64` (`0.1` is one tenth) | — |
| `ExcelSerialFromFloat64` | `CivilDateTime` | `ExcelEpoch` |
| `ExcelTime` | `time.Duration` since midnight | — |
| `ExcelDuration` | `Duration{Seconds, Nanos}` | — |
| `String` | `string` — the text itself, once it is valid UTF-8 | — |

The checked doors, `Check`, `CheckFromFloat64` and `CheckString`, judge a value by declared
constraints and are under [Validation constraints](#validation-constraints). Every door has
a detailed twin (`I32Detailed`, `DateOnlyDetailed`, `CheckDetailed`, …) that also names what
failed, and the detailed date doors take opt-in shapes: [Detailed doors](#detailed-doors-what-failed)
and [Date options](#date-options-compact-dates-two-digit-years-and-month-names).

`Optional` presents a door's two results with Go's absent, a nil pointer: `age, fault :=
hypercast.Optional(hypercast.I32(text, hypercast.Invariant))` is `nil, nil` for an `Empty`
fault, a pointer to the value on success, and every other fault untouched. `OptionalChecked`
does the same for a checked door's `(Checked[V], Rejection)`: only an `Empty` fault is absent,
and a `*Violation[V]` flows through.

Plus three entry points that are not doors — `Available()`, `LoadError()` and
`NativeVersion()` — covered under
[The core](#the-core-available-loaderror-nativeversion) below.

### Numeric — one door generic over the target

A caller that is itself generic over the target type would otherwise write the eleven-way
door switch. `Numeric[V Number, T Text]` writes it once: `Number` is the closed set of
types the numeric doors return — `int8` … `int64`, `uint8` … `uint64`, `float32`,
`float64`, `Decimal` — and `Numeric[int32]` is `I32`, `Numeric[Decimal]` is `Exact`, verdict
for verdict. `T` is inferred from the argument. An unsupported `V` is impossible by
construction: the constraint is exactly the door list, so the compiler rejects anything
else at the call site.

```go
func column[V hypercast.Number](cells []string, format hypercast.NumFormat) ([]V, *hypercast.Fault) {
	out := make([]V, 0, len(cells))
	for _, cell := range cells {
		v, fault := hypercast.Numeric[V](cell, format)
		if fault != nil {
			return nil, fault
		}
		out = append(out, v)
	}
	return out, nil
}
```

### Scalar — one door generic over every target

`Scalar[V ScalarTarget, T Text]` widens `Numeric` to every target with a door of its
own: each `Number`, plus `bool` (`Bool`), `uuid.UUID` (`Uuid`), `time.Time` (`Timestamp` —
RFC 3339, zone mandatory, normalized to UTC), `Date` (`DateOnly`, strict `yyyy-MM-dd`) and
`Duration` (`Span`). The `NumFormat` argument is read by the numeric doors only. The
verdicts are the concrete door's, and as with `Numeric` an unsupported `V` fails to
compile. Three types are left out on purpose: `rune` is `int32`, so `Scalar[int32]` is the
integer door and a character is `Char`'s; `time.Duration` is what `TimeOfDay` returns, but
every Go reader takes it for a span; and `CivilDateTime` needs a declared `DateOrder`.

```go
import "time"

v, fault := hypercast.Scalar[time.Time]("2026-01-02T15:04:05+05:00", hypercast.Invariant)
fmt.Println(v, fault) // 2026-01-02 10:04:05 +0000 UTC <nil>
```

### Exact decimals

`Exact` is the decimal door — named that way because the result type already owns the
identifier `Decimal`, the same reason `Span` returns a `Duration`. It reads the real doors'
grammar under the same `NumFormat` but never rounds: `Decimal{Lo, Hi, Scale, Negative}` is
the .NET `decimal` shape, (−1)^Negative × (Hi·2⁶⁴ + Lo) × 10⁻ˢᶜᵃˡᵉ, and text carrying more
than 96 bits or 28 places is `OutOfRange`, not approximated. The result is canonical:
exact trailing zeros in the fraction are trimmed so the scale is minimal, so `"1.10"`,
`"1.1"` and `"1.1000"` are all magnitude 11 at scale 1 and `String()` renders each as
`"1.1"`; `Rat()` hands over the exact value as a `*big.Rat`. Zero is never negative and
always scale 0.

```go
d, fault := hypercast.Exact("($1,234.50)", hypercast.NumFormat{
	DecimalSep: ".", GroupSep: ",", Styles: hypercast.AllStyles, Currency: "$",
})
fmt.Println(d, fault) // -1234.5 <nil>
// d == Decimal{Lo: 12345, Scale: 1, Negative: true}; d.String() == "-1234.5"
```

### NumFormat: separators, currency, and the culture's own symbols

Every numeric door takes a `NumFormat` — the caller's declared notation, never a guess.
Go's standard library carries no culture data, so there is no `From(culture)` bridge here
as the C#, Java and Swift bindings have; the struct literal is the whole API, and an
application fills it from whatever culture data it holds. `Invariant` is `"."`/`","` with
`AllStyles`, Western grouping and nothing else declared; `Detect` adds
`SeparatorDetect`. `AllStyles` is every lenience including `CurrencySymbol` (and excluding
`SeparatorDetect` and `TrailingSign`), so keyed literals that predate a field keep compiling
and keep their meaning. `NewNumFormat(decimalSep, groupSep, styles)` builds a format under
Western grouping, and the further fields are the result's to fill.

| Field | Declares | Read |
| --- | --- | --- |
| `DecimalSep`, `GroupSep` | the number separators — 1 to 8 UTF-8 bytes each with no ASCII digit (`"."`, `","`, `" "`, `", "`), different from each other; the only required fields | always |
| `Styles` | the `NumStyles` leniences, and in bits 8 and 9 the `GroupingPolicy` | — |
| `Currency` | the currency symbol (`"$"`, `"€"`, `"kr."`), at most 16 bytes | under `CurrencySymbol` |
| `CurrencyDecimalSep`, `CurrencyGroupSep` | the separators of a currency amount, where a culture points and groups money differently from numbers; empty keeps the number separator | when `Currency` is declared and found in the text; ignored under `SeparatorDetect`, whose detection governs the whole token |
| `MinusSign`, `PlusSign` | a further spelling of the sign (`"−"`, U+2212, in Estonian and Basque text) | wherever ASCII `-`/`+` are |
| `Percent` | a further spelling of the percent symbol (`"٪"`, `"％"`) | as a trailing `%` is, under `Percent` — the real and decimal doors; the integer doors never read a percent |
| `DigitZero` | native digits: the ten code points from this zero up (`'٠'` for the Arabic-Indic digits, `'０'` for fullwidth); `0` declares none | wherever a digit is — integer part, fraction, exponent; a `0x`/`0b` body stays ASCII |

White space is Unicode's `White_Space`, the 25 code points (ASCII, U+0085, U+00A0, U+2000 to
U+200A, U+202F, U+3000 among them). Every door ignores it at both ends (`Char` given exactly
one character keeps it), and the numeric doors also after a leading sign, before a trailing sign or a percent symbol, on
either side of a currency symbol and inside accounting parentheses (`"- 12"`, `"( 12 )"`,
`"12,5 %"`, the percent one under a declared comma decimal separator); between digits it is `Malformed`, unless a space-like group separator is
declared: an ASCII space, U+00A0 or U+202F declared as the group separator makes the input's
choice of those three immaterial, and a space-like separator with no digit after it is just
white space. Input of nothing but white space is `Empty`. A day period may be set off from
the clock by any run of white space (`3:04\u202fPM`).

**Grouping policy.** `Grouping` permits a group separator between digits, and the format's
`GroupingPolicy` judges the groups. `GroupingWestern` wants groups of exactly three digits
counted from the decimal separator, the leftmost one to three (`1,234,567`; `1,23,456` is
`Malformed`, spanning `,23`); `GroupingIndian` wants three next to the decimal separator and
twos to the left (`12,34,567`). Under both a first group can't start with a zero (`0,125` is `Malformed`,
spanning `0`) and the last group is three digits (`12,34` is `Malformed`), and a lone separator
can't follow more than three digits (`1234,567`). Once a numeral uses a
group separator every group must conform; no group may follow the decimal separator or sit in an exponent, digits are counted
as digits (native ones included) and the space-like separators and the currency group
separator follow the same rule; an ungrouped numeral is always fine, and with `Grouping` off a
separator stays `Malformed` under both. Western is no bits, so a literal that sets no policy, the zero value and a format
1.0 code stored with `encoding/json` or `encoding/gob` are Western, as `NewNumFormat`, `Invariant` and `Detect` are. Indian lives in `Styles` (the
`GroupingPolicyMask` bits): `WithGrouping` replaces the policy (it never combines two) and `WithStyles` replaces the
leniences, each keeping the other. Bit 8 (256) is retired permanently and never reused. `WithGrouping` and JSON
decoding refuse it, and JSON any bit past the policy; a struct literal or another decoder (`gob`, `encoding/xml`) can't, so `Raw()` returns an error and the first door panics on it, never reading a cell. A shape neither policy
reads is `Malformed` under a declared format or `Invariant`, never misread, while `Detect` infers by the delimiter rule, so `1,2345` is 1.2345 there: [docs/grouping.md](https://github.com/SkunkWerkx/HyperCast/blob/master/docs/grouping.md#parked).

```go
_, fault := hypercast.I32("1,23,456", hypercast.Invariant)
fmt.Println(fault) // hypercast: malformed at byte 1..4
v, _ := hypercast.I32("1,23,456", hypercast.Invariant.WithGrouping(hypercast.GroupingIndian))
fmt.Println(v) // 123456
```

The rule for every field past the separators: **the ASCII spelling is always read, and a
declared symbol is a further spelling read beside it.** Nothing that parsed before a symbol
was declared stops parsing once it is: `"-5"` is -5 under a format declaring `"−"`, and so
is `"−5"`; `"50%"` is 0.5 under a format declaring `"٪"`, and so is `"50٪"`; `"123"` is 123
under a format declaring `'٠'`, and so are `"١٢٣"` and `"12٣"`. Two signs in any spelling
are `Malformed` at the later one. Under `CurrencySymbol` with a `Currency` declared, the
symbol is accepted once, whole, at either edge of the numeric body: leading, before or
after a sign (`$5`, `-$5`, `$ -5`, `€−5`), or trailing (`5 €`, `1.234,50 kr.`), with
optional white space between symbol and digits; accounting parentheses wrap symbol and
digits together (`($5)`). It never takes part in the digit scan, so a symbol containing a
separator character (`kr.` under `.` grouping) is fine. A symbol declared while the style
is off is `Malformed` at the symbol; the style with no symbol declared is a no-op.

`TrailingSign` is the one style that is not in `AllStyles`: it permits the sign after the
digits (`5-`, `5 -`, `5+`, `5 €-`, `5- €`, `2.5%-`), in ASCII or the declared spelling,
once, never beside a leading sign and never inside parentheses. A culture whose negative
pattern is `n-` or `n -` sets it; everything else leaves it off, so `"5-"` stays
`Malformed` under `Invariant`.

A format the core would refuse is a caller bug and panics, never a verdict — the way equal
separators always have: `Styles` with both policy bits set (no policy) or a bit no style or
policy defines, a number separator that is empty or longer than 8 bytes, equal
number separators or equal separators in force for a currency amount, any symbol that is
not valid UTF-8 or carries an ASCII digit, a sign or percent symbol with white space in it, a
decimal separator (number or currency) with white space in it, a `Currency` longer than 16
bytes or starting or ending in white space (`F CFA` is fine, and there an ASCII space,
U+00A0 and U+202F read as each other), or a `DigitZero` that is ASCII or starts no Unicode
digit block. `NumFormat.Raw()` reports the same check as an `error` for code that
would rather not panic.

```go
estonian := hypercast.NumFormat{
	DecimalSep: ",", GroupSep: " ", Styles: hypercast.AllStyles | hypercast.TrailingSign,
	Currency: "€", MinusSign: "−",
}
fmt.Println(hypercast.F64("−1 234,5 €", estonian)) // -1234.5 <nil>; so is "1 234,5 €−", and "-1234,5"

arabic := hypercast.NumFormat{DecimalSep: "٫", GroupSep: "٬", Styles: hypercast.AllStyles, DigitZero: '٠', Percent: "٪"}
fmt.Println(hypercast.F64("١٢٫٥٪", arabic)) // 0.125 <nil>

euros := hypercast.NumFormat{DecimalSep: ",", GroupSep: ".", Styles: hypercast.AllStyles, Currency: "€"}
fmt.Println(hypercast.F64("€ 1.234,50", euros)) // 1234.5 <nil>
```

The boolean lexicon stays English (`true`/`yes`/`on`/`enabled`/… and their negatives): no
platform has culture data for it, so no binding bridges one — a caller maps a culture's
words before the door.

### DateFormat: the culture's day periods

`DateTime` reads a 12-hour clock's marker as the English `AM`/`PM`, ASCII
case-insensitively, after the clock or ahead of it. `DateTimeWith` takes a `DateFormat`
instead of the bare `DateOrder` — the same declarative shape as `NumFormat`: the order, and
the words the culture writes for the two halves of the day. A declared designator is
matched byte for byte, after the clock (`3:04 nachm.`) or ahead of it (`오후 3:04`), with any
run of white space or none between designator and clock (so the `3:04\u202fPM` that ICU 72 and
later write for `en_US` reads), and the English pair is read whatever is declared.
A `DateFormat` declaring no designators reads exactly what `DateTime` reads, and
`DateTime` itself is unchanged. Only the civil date-time door reads them:
`DateOnlyOrdered` has no time part.

A designator is at most 16 UTF-8 bytes, carries no ASCII digit, and neither starts nor
ends in white space (an inner space, as in Spanish `a. m.`, is fine); anything else
is a caller bug and panics, and `DateFormat.Raw()` reports it as an `error`.

```go
german := hypercast.DateFormat{Order: hypercast.DayMonthYear, AM: "vorm.", PM: "nachm."}
fmt.Println(hypercast.DateTimeWith("1.7.2026 3:04 nachm.", german)) // July 1st, 15:04

korean := hypercast.DateFormat{Order: hypercast.YearMonthDay, AM: "오전", PM: "오후"}
fmt.Println(hypercast.DateTimeWith("2026-07-01 오후 3:04", korean)) // the same wall clock
```

## Detailed doors: what failed

Every door has a detailed twin, named with `Detailed` (`BoolDetailed`, `I32Detailed`,
`NumericDetailed`, `ScalarDetailed`, `ExactFromFloat64Detailed`, `StringDetailed`, …): the
same parse in the core, returning a `*Failure` where the door returns a `*Fault`. A
`Failure` carries the `Fault`'s `Reason`, `Offset` and `Length` and a `Detail` naming what
failed, from a small shared vocabulary of 32 codes (`DetailBlank`, `DetailGroupSize`,
`DetailNotFinite`, `DetailOutsideWindow`, …), whose `String()` is the core's snake-case name.
`Fault()` projects it back to what the plain door returns. Go cannot give `Fault` a field
without breaking unkeyed literals and equality, so the detail travels on this separate path
and `Fault` keeps its three fields.

The vocabulary is open: published codes keep their numbers and meanings, but a minor release
may add one, so a `switch` over details needs a `default` arm. A code this binding doesn't name
is kept, `Known()` false and `String()` `"detail N"`, and the reason and span stay usable.
`CheckDetailed`, `CheckFromFloat64Detailed` and `CheckStringDetailed` return a
`DetailedRejection`, a `*Failure` or a `*Violation[V]` (sealed, as `Rejection` is), and
`OptionalDetailed` and `OptionalCheckedDetailed` present them as `Optional` does.

```go
_, failure := hypercast.I32Detailed("12,34", hypercast.Invariant)
fmt.Println(failure) // hypercast: malformed (group_size) at byte 2..5
switch failure.Detail {
case hypercast.DetailGroupSize, hypercast.DetailGroupMisplaced:
	fmt.Println("check the grouping")
default: // details are open
	fmt.Println(failure.Detail)
}
```

## Date options: compact dates, two-digit years and month names

The detailed date doors take a `DateOptions`, every part of it opt-in: the zero
`DateOptions` reads exactly what the plain door reads, and the plain doors never read the
shapes below.

| Option | Door | Reads |
| --- | --- | --- |
| `CompactDate` | `DateOnlyDetailed` | `20260107` (eight ASCII digits) |
| `SpaceSeparator` | `TimestampDetailed` | one space in place of `T` |
| `HourOffset` | `TimestampDetailed` | `±hh` offsets (`+00`, `-05`) |
| `Pivot` | `DateOnlyOrderedDetailed`, `DateTimeDetailed` | a two-digit year, in the 100 years ending at the pivot (100 to 9999; 0 for none) |
| `Months` | `DateOnlyOrderedDetailed`, `DateTimeDetailed` | a declared month name in the month field |

`CommonForms` is the three shapes together and supplies no pivot or vocabulary; a shape a door
doesn't read is ignored there, so one `DateOptions` serves every date door. `DateTimeDetailed`
takes a `DateFormat`, so it is `DateTime` and `DateTimeWith` at once. A shape bit that isn't
defined or a pivot outside 0 and 100 to 9999 is a caller bug and panics, and
`DateOptions.Raw()` reports it as an `error`; build a `DateOptions` with field names.

`MonthNames` is a caller-built vocabulary, judged once by `NewMonthNames` (or
`MustMonthNames`): each `MonthName` maps a name to its month, aliases are separate entries,
and every month needs one. A name is 1 to 64 UTF-8 bytes with no ASCII digit, `/`, `-` or
white space (`.` is fine), and no two names may fold to the same spelling, even for one month.
A name replaces the numeric month field the declared order selects, with the date's own
separator, matched under Unicode simple case folding, the longest name followed by the
separator winning; punctuation is part of the name, so with `janv.` declared `7.janv..2026`
reads and `7.janv.2026` doesn't. The vocabulary works with no other option set.

```go
months := hypercast.MustMonthNames(
	hypercast.MonthName{Month: time.January, Name: "Jan"}, /* … one or more per month … */
	hypercast.MonthName{Month: time.December, Name: "Dec"},
)
date, _ := hypercast.DateOnlyOrderedDetailed("07-Jan-26", hypercast.DayMonthYear,
	hypercast.DateOptions{Pivot: 2029, Months: months})
fmt.Println(date) // {2026 January 7}
```

## Validation constraints

A value that parsed can still break a business rule. A `Constraints[V]` declares the rule
with the OpenAPI Schema Object's keywords, bound by its type to one door
(`Constraints[float64]` for `F64`, `Constraints[Decimal]` for `Exact`, `Constraints[string]`
for the text door, …), and the checked doors judge the value by it, in the core and in exact
decimal arithmetic:

```go
// Latitude: a multiple of 0.000001 between -90 and 90, finer input rounded half to even.
latitude := hypercast.MustConstraints[float64](
	hypercast.Minimum(-90), hypercast.Maximum(90),
	hypercast.MultipleOf("0.000001"), hypercast.WithRounding(hypercast.Midpoint))
// Euro cash: a multiple of 0.05, and anything else is wrong.
euroCash := hypercast.MustConstraints[hypercast.Decimal](hypercast.MultipleOf("0.05"))
// A sex code: one of three, whatever case and padding it arrived in.
sex := hypercast.MustConstraints[string](hypercast.Trim(), hypercast.Enum("Female", "Male", "Other"))

euro := hypercast.NumFormat{DecimalSep: ",", GroupSep: ".", Styles: hypercast.AllStyles, Currency: "€"}
fmt.Println(hypercast.Check("45.1234567", hypercast.Invariant, latitude)) // {45.123457 true -1} <nil>
fmt.Println(hypercast.Check("-90.5", hypercast.Invariant, latitude))      // {0 false -1} hypercast: breaks minimum at byte 0..5
fmt.Println(hypercast.CheckFromFloat64(45.1234567, latitude))             // {45.123457 true -1} <nil>
fmt.Println(hypercast.Check("€1.234,55", euro, euroCash))                 // {1234.55 false -1} <nil>
fmt.Println(hypercast.Check("€12,37", euro, euroCash))                    // {0 false -1} hypercast: breaks multipleOf at byte 0..8
fmt.Println(hypercast.Check("12,3x", euro, euroCash))                     // {0 false -1} hypercast: malformed at byte 4..5

checked, rejection := hypercast.CheckString(" MALE ", sex)
switch r := rejection.(type) {
case nil:
	fmt.Printf("%s, entry %d\n", checked.Value, checked.EnumIndex) // Male, entry 1
case *hypercast.Fault:
	fmt.Printf("%s at byte %d\n", r.Reason, r.Offset) // empty at byte 0, for "   "
case *hypercast.Violation[string]:
	switch r.Keyword {
	case hypercast.KeywordEnum:
		fmt.Printf("not a sex code: %q, bytes %d+%d\n", r.Value, r.Offset, r.Length) // for "Femal": "Femal", bytes 0+5
	default: // keywords are open
		fmt.Println("breaks", r.Keyword)
	}
}
```

A checked door returns `(Checked[V], Rejection)`, beside the plain doors' `(value, *Fault)`.
`Checked[V]` is the `Value`, `Adjusted` (true only
when rounding changed it) and `EnumIndex`, the matched entry at the text door and `-1`
otherwise (a numeric `enum` has no index: the value is the answer). A nil `Rejection` is
success; otherwise it is one of two pointer types, sealed so nothing else implements it: the
plain doors' own `*Fault` (the input is not the type: the very type a plain door returns, so
a parse failure reads the same at either door) or a `*Violation[V]` (a value of the type
that broke a rule: its `Keyword`, the `Value` exactly as the door cast it, and the value's
span, in bytes like a fault's). The `Value` is never the rounded one, since `MultipleOf`'s
rounding is part of the judgement; at the text door it is the text judged (trimmed under
`Trim`), not an `Enum` entry. `V` is the door's type, so a type switch names it:
`case *hypercast.Violation[int32]:`. Both implement `error`. `Keyword` is a code with the OpenAPI
names as its `String` (`exclusiveMinimum`) and the constants `KeywordMinimum` …
`KeywordEnum`; `format` and `pattern` may join in a minor release, so a switch over keywords
needs its `default` arm, and a code this binding does not know yet is kept (`keyword 9`).

`Check[V Number, T Text]` is every numeric door's checked form, its door picked by the
`Constraints` it is handed (`V` is inferred from it, `T` from the text), over a `string` or a
`[]byte` with a `NumFormat`; `CheckFromFloat64` judges a number a source already decoded on
the shortest decimal that names it (`0.1 + 0.2` breaks `MultipleOf("0.1")` under `Strict`,
and rounds to `0.3` under `Midpoint`), with an empty span; `CheckString` is the text door's.
The numeric doors take `Minimum`, `ExclusiveMinimum`, `Maximum`, `ExclusiveMaximum` (any of
them together, each judged on its own, as OpenAPI 3.2 reads them), `MultipleOf` with
`WithRounding` (`Strict`, the default, then `Midpoint`, half to even, `Floor` and `Ceiling`)
and `Enum`, its entries compared exactly to the value; the text door takes `MinLength`,
`MaxLength` (in code points), `Trim` (Unicode `White_Space`, U+00A0 included),
`CaseSensitive` and `Enum`, matched under Unicode simple case folding (byte for byte under
`CaseSensitive`, for a vocabulary like `mm` and `Mm`) and delivering the caller's own
declared string and its index (`Enum` with numbers there declares valid entries no string
equals). `NewConstraints` takes any rule at any door, and the core
alone judges it: a keyword for another type (`MinLength` at a number, `Minimum` at text) is
ignored, as OpenAPI ignores it. The first failure wins: the door parses, the text is
trimmed, `MultipleOf` rounds or is broken (a multiple past what the type holds breaks it
too: `127` at `int8` under `MultipleOf(5)` and `Ceiling` carries `127`), then `Minimum`,
`ExclusiveMinimum`, `Maximum`, `ExclusiveMaximum`, the lengths and `Enum` judge the result. A float door also judges the float it delivers against
each bound read as the door reads it: `"1e-400"` at `float64` reads as `0`, so it breaks
`ExclusiveMinimum("0")`. Bounds, steps and numeric entries are text in invariant notation, a
`Decimal` or a Go integer, all converted exactly by the core (`"1e-6"` is `0.000001`, and
`"1e-29"`, past the 28 places a core decimal holds, is refused rather than rounded); a
`float64` does not compile there, so `0.1` cannot be declared inexactly.

A declaration is judged once, by the core, when it is built, and every valid OpenAPI
declaration is accepted, one no value can satisfy included: crossed or equal bounds, an
inclusive and an exclusive bound together, `MinLength` above `MaxLength`, repeated entries
(the first match wins), entries or bounds the door's type cannot reach. `Enum()` with no
entries declares an `enum` every value breaks; leaving `Enum` out declares none. What the
core refuses is what it cannot read: `Trim` or `CaseSensitive` at a numeric door, or
`WithRounding` beside `MultipleOf` at the text door (code 1); a `MultipleOf` not positive
(5); a rounding without `MultipleOf` (6); text that is no decimal, a decimal past 96 bits or
28 places, or a rounding at an integer door with a fractional step (8); an `Enum` entry that
is not UTF-8 (9). `NewConstraints` returns a refusal as an `error` worded from the rule it
broke, for a declaration read at run time; `MustConstraints`, for one written in code,
panics with the same text, as a door does on a `NumFormat` the core would refuse
(`MustConstraints[float64](hypercast.MultipleOf("-0.05"))` panics with "hypercast:
Constraints refused (code 5): multipleOf is not positive."). A Go call can say a few things
no OpenAPI declaration can, and this binding refuses those itself: a keyword declared twice,
a zero `Rule`, a length below zero or past `math.MaxUint32`. Numbers as the text door's
entries are valid and never match, as JSON Schema's exact equality says. After that a checked cast judges only data and
panics on nothing for it. There is no default, as there is none for `NumFormat`: a zero
`Constraints[V]` was never judged, so handing one to a checked door is a caller bug and
panics; `MustConstraints[V]()` with no rules is the declaration of none.

`String` is the plain text door: the input itself when it is valid UTF-8, untrimmed
(`String("  Male ")` is `"  Male "`). A Go string can hold any bytes, so a `string` and a
`[]byte` input are judged alike, at every door, plain or checked: invalid UTF-8 is
`Malformed` at its first invalid sequence,
and an `Enum` entry that is not UTF-8 is refused (code 9). Go's string model has no lone
surrogate: its UTF-8 encoding is just invalid bytes, judged as any others are. A `string`
input comes back as the caller's own string (a substring of it when trimmed) and an `Enum`
match as the caller's own entry, so the checked doors make no allocation on success; a
`[]byte` input to `String` or `CheckString` is copied into its result. `Constraints.Raw()`
and `PackedEnum()` hand a declaration over in the core's own 104-byte layout and `enum` list,
for a library judging values across a C ABI of its own. `Constraints.Enum()` reads the
declared `enum` back: each entry as `Enum` rendered it (a number as its decimal text), whether
they were declared as numbers, and `ok` false when no `enum` is declared; an empty one, which
every value violates, is `ok` with no entries.

## The core: `Available`, `LoadError`, `NativeVersion`

The core is compiled into the binary, so there is nothing to load and nothing that can fail,
and no door can panic for want of a library. The three probes every binding carries are here
so code written against them works unchanged:

- `Available()` — always `true`.
- `LoadError()` — always `nil`.
- `NativeVersion()` — `"major.minor.patch"` as the core reports it about itself, a literal
  compiled into the translated file from the crate's version rather than read from this
  module, so a deployment can confirm which core went into the binary. It cannot fail.

```go
import "log"

log.Printf("hypercast core %s", hypercast.NativeVersion())
```

[HyperUuid](https://github.com/SkunkWerkx/HyperUuid)'s Go binding has the same three entry
points.

Nor can a binding meet an older core. The core is part of this package, regenerated with the
crate (`TestCoreIsCurrent`, and `TestRegenerationCarriesTheCrateVersion` for the version it
carries), and every door crosses through the core's detailed exports, the only ones that read a
grouping policy: a 1.0 core, which would read `1,23,456` as 123456 under any format, put in its place
doesn't build. `HYPERCAST_DOWNGRADE_ROOT=<a v1.0.0 checkout> go test -run
TestADowngradedCoreDoesNotBuild .` translates the 1.0 crate and shows the build failing on the
missing detailed exports.

## Interop: building on HyperCast's C ABI

For a module that carries HyperCast's verdicts across a C ABI of its own (HyperTabular
does): the layouts the core writes, the conversions every door applies to them,
the checked codes of the declared options, and the packed version word — the same code the
doors run, so a value read out of another library's buffer is the value the door of the
same name would have returned.

- `RawTimestamp` (16 bytes) → `.Time()`, `RawDate` (4) → `.Date()`, `RawCivil` (16) →
  `.CivilDateTime()`. `Decimal` and `Duration` need no raw twin: each is laid out exactly as
  the core writes one, checked at compile time, so a buffer of them can be viewed as a
  `[]Decimal` or `[]Duration` in place — `Duration` on a 64-bit host only, since the 32-bit
  ports (`386`, `arm`, `mips`) align an `int64` to 4 bytes and make it 12.
- `RawNumFormat` — the 92 bytes a format crosses as, which the core reads in place: a
  size word, the flags, the digit zero's code point, then every symbol inline as its bytes
  followed by its length — `RawSymbol8` (9 bytes; `SymbolMaxBytes` is 8) for the
  separators, signs and percent symbol, `RawSymbol16` (17 bytes; `CurrencyMaxBytes` is 16)
  for the currency symbol. `NumFormat.Raw()` builds one, returning the doors' check as an
  error instead of their panic.
- `RawDayPeriods` — the 40 bytes a `DateFormat`'s designators cross as beside the order: a
  size word and the AM and PM designators as `RawSymbol16`s (`DesignatorMaxBytes` is 16);
  a zero size is what crosses as null, nothing declared. `DateFormat.Raw()` builds one.
- `RawConstraints` — the 104 bytes, 8-aligned, a `Constraints` crosses as
  (`RawConstraintsSize`): a size word, the flags (`RawConstraintsHasMinimum` …
  `RawConstraintsHasEnum`, `RawConstraintsTrim`, `RawConstraintsCaseSensitive`), the rounding
  code, the two lengths, a reserved word, then `Minimum`, `Maximum`, `MultipleOf`,
  `ExclusiveMinimum` and `ExclusiveMaximum` as `Decimal`s, each bound in a field of its own.
  `Constraints.Raw()` builds one, and `Constraints.PackedEnum()` is its `enum` list beside it
  (16-byte `Decimal`s in the host's byte order at a numeric door, a little-endian `uint32`
  length then the bytes per entry at the text door). `RawCheck` (12 bytes) is what a checked
  door reports beside its value (written on a violation as on a success), and
  `RawSpan` (8) what the text door writes; each layout is held to the generated header's
  size by the suite.
- `Valid()` on `UnixPrecision`, `DateOrder` and `ExcelEpoch`; `UnixPrecisionFromCode`,
  `DateOrderFromCode`, `ExcelEpochFromCode` and `ReasonFromCode` turn an ABI code back into
  the type, and `false` for any other value.
- `FaultFromCode(code, offset, length)` — the `*Fault` a nonzero verdict code and its span
  name; `false` when the code names no reason, a binding bug rather than data.
  `FailureFromCode(code, offset, length, detail)` is the `*Failure` a detailed export's
  `RawFaultDetail` (16 bytes: a size word the caller writes, the span, the `Detail` code)
  names, keeping a code this binding doesn't know.
- `RawDateOptions` (12 bytes: a size word, the `DateShapes` bits, the pivot) — what a
  `DateOptions` crosses as; `DateOptions.Raw()` builds one, and `MonthNames.Packed()` is the
  vocabulary beside it (each entry a month byte, a little-endian `uint32` length, the name's
  bytes). `NumFormat.Raw()`'s `Flags` carry `GroupingIndian`, which only
  the core's detailed exports read: the 1.0 exports mask flags to 8 bits and read Western.
- `FormatVersion(packed)` — a `*_version()` export's `major<<16 | minor<<8 | patch` word as
  `"major.minor.patch"`.

```go
import (
	"log"
	"unsafe"
)

raw, err := hypercast.NumFormat{DecimalSep: ",", GroupSep: ".", Styles: hypercast.AllStyles, Currency: "€"}.Raw()
if err != nil {
	log.Fatal(err)
}
fmt.Println(unsafe.Sizeof(raw)) // 92: the bytes that cross to the other library's C ABI
// Its verdicts come back as codes and spans; code 2 at byte 3, length 1, is a malformed byte:
if fault, ok := hypercast.FaultFromCode(2, 3, 1); ok {
	fmt.Println(fault) // hypercast: malformed at byte 3..4
}
```

## Platforms

Every GOOS and GOARCH Go has, with `CGO_ENABLED=0`, because the core is Go source. CI
cross-compiles and vets the package for these 23 pairs on every pull request that touches
the Go binding, the core or the workflows, and on every release-mode run:

| GOOS | GOARCH |
| --- | --- |
| `linux` | `amd64`, `386`, `arm`, `arm64`, `riscv64`, `ppc64le`, `s390x`, `loong64`, `mips64le` |
| `windows` | `amd64`, `arm64`, `386` |
| `darwin` | `amd64`, `arm64` |
| `freebsd`, `openbsd`, `netbsd`, `illumos`, `plan9` | `amd64` |
| `aix` | `ppc64` |
| `android` | `arm64` |
| `js`, `wasip1` | `wasm` |

and runs the whole suite natively on Linux and Windows on x64 and arm64, on macOS on arm64
(Intel macOS is built by the sweep, not run), on Alpine (musl x64 and arm64), in an iOS
simulator, in an Android emulator, and on the three WebAssembly hosts below. No big-endian
host runs in CI; the suite, corpus replay included, passed on `linux/s390x` under emulation
when the backend landed (2026-11-01), and failed there with the big-endian decoding turned
off. A binary carries the core like any package, writes nothing to disk and starts without
touching the filesystem; glibc or musl makes no difference, since a cgo-free Go binary takes
nothing from the C library, and a `scratch` image runs it as it is.

### iOS and Mac Catalyst

`GOOS=ios GOARCH=arm64` builds the package as it builds any other. What an iOS executable
needs is Xcode's linker, because Go's `ios/arm64` port links externally; that is Go's rule
for the platform, nothing of this binding's, and it is what
[`gomobile`](https://pkg.go.dev/golang.org/x/mobile/cmd/gomobile) arranges. For the
simulator, Go's own `misc/ios/clangwrap.sh` is the `CC`:

```sh
CGO_ENABLED=1 CC="$(go env GOROOT)/misc/ios/clangwrap.sh" GOOS=ios GOARCH=arm64 go build ./...
```

The device, the simulator and Mac Catalyst (gomobile's `maccatalyst` tag) build the same
package; nothing here selects on a tag. CI runs this suite in an iOS simulator through Go's
`misc/ios/go_ios_exec.go`, with the corpus staged as `testdata/corpus` since that wrapper
carries nothing above the module into the simulator, and builds it for a device with the
iPhoneOS SDK's clang. Mac Catalyst is not run from Go there.

### Android

`GOOS=android GOARCH=arm64 go build ./...` needs no NDK: Go links it internally.
`android/amd64` (the emulator) is a platform Go itself links externally, so it takes a C
linker for that reason alone, the NDK's clang as `CC` with `CGO_ENABLED=1`, as any Go program
for it does; nothing of the core's is linked either way, and page alignment is the linker's
business (NDK r28 and later align to 16 KB pages by default). CI cross-builds this whole
suite for `android/amd64` and runs it in an x86_64 emulator whose image uses 16 KB pages
(`.github/scripts/android_build_suite.sh` and `android_device_test.sh`, which run the same
way against a local emulator); `android/arm64` is built in the cross-compile sweep.

### WebAssembly

The translated core is plain Go, so it builds for the browser and for WASI under stock Go
and TinyGo alike, with no archive and no cgo:

```sh
GOOS=js GOARCH=wasm go build -o main.wasm .          # stock Go, the browser or Node
cp "$(go env GOROOT)/lib/wasm/wasm_exec.js" .

tinygo build -target=wasm -no-debug -opt=z -o main.wasm .   # TinyGo, the browser
cp "$(tinygo env TINYGOROOT)/targets/wasm_exec.js" .
```

```html
<script src="wasm_exec.js"></script>
<script>
  const go = new Go();
  WebAssembly.instantiateStreaming(fetch("main.wasm"), go.importObject)
    .then(result => go.run(result.instance));
</script>
```

Use the `wasm_exec.js` of the toolchain that built the module. `GOOS=wasip1 GOARCH=wasm` and
`tinygo build -target=wasip1` do the same for a WASI runtime (wasmtime, for one). The core
imports nothing, so the page loads one module and that is all. The binding, its dependencies (`time`,
`math/big`, `google/uuid`) and the core add about 191 KB to a TinyGo `-opt=z -no-debug`
browser build, 953 KB to a stock Go `js/wasm` one and 631 KB to a native darwin/arm64 one,
each built with `-ldflags='-s -w'` where the toolchain takes it, on top of `fmt` and the
runtime (2026-11-01, TinyGo 0.44, Go 1.27.3). No WebAssembly timings are published yet.

CI runs the whole suite three ways on every pull request that touches the Go binding, the
core or the workflows: under TinyGo on wasip1 under wasmtime (with the checkout mounted and
the corpus found through `HYPERCAST_CORPUS`, and the same binary run without the mount as a
negative control), under stock Go on `js/wasm` under Node and on `wasip1/wasm` under
wasmtime. It also builds [`internal/tinygosmoke`](internal/tinygosmoke) with TinyGo for the
browser and runs it in headless Chrome: every ABI shape — plain, numeric with a format (a
currency symbol, and every other symbol the layout carries), discriminated, the civil
date-time door with declared day periods — a fault with its span, and the core's version
against `rust/Cargo.toml`.

## Building and regenerating

Nothing to build: `go test ./...` runs on the committed core. The core is regenerated only
after a change to `rust/` or to the pins:

```sh
rustup toolchain install 1.99.1 --profile minimal --target wasm32-wasip1
go generate     # in go/: builds rust/ for wasm32-wasip1 and rewrites internal/core/core.go
```

[`internal/wasm2go`](internal/wasm2go) is the generator: it builds the crate's `cargo cdylib`
with the rustc named in its `rust-toolchain.toml` (1.99.1), a 64 KiB guest stack
(`-C link-arg=-zstack-size=65536`) and wasi-libc for `memcmp`, `memcpy` and `memset` (`-l
c`), then runs the wasm2go pinned in its own `go.mod` with `-unsafe`, which replaces the memory helpers with `unsafe.Add` access behind a check of
the access's last byte (CPUs without unaligned access keep the `encoding/binary` path). The rustc is pinned as a
reproducibility constraint, not a floor: the wasm's bytes follow rustc's codegen, so one named
toolchain makes the committed file reproducible, and a bump is a deliberate
regenerate-and-commit. `HYPERCAST_CHECK_CORE=1 go test -count=1 -run
'TestCoreIsCurrent|TestRegenerationCarriesTheCrateVersion' .` regenerates into a temporary
file and diffs it against the committed one (without the variable the test skips when the
toolchain is missing); CI runs it on every pull request that touches the Go binding, the
core or the workflows, and on every release-mode run, and release preparation regenerates
the core at the new version, since the version is a literal in the file.

## Why not `strconv` / `time.Parse`?

1. **Verdicts with location** — a closed reason plus the offending span, against
   `strconv.NumError`'s wrapped string.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, signs, currency symbols, percent symbols and native
   digits, a culture's own AM/PM, radix prefixes, all five .NET `Guid` text forms plus
   `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other
   binding, held by the shared corpus (full corpus replay on every platform, and in the
   browser and on WASI under TinyGo and stock Go).

**The honest trade-off, stated as plainly as the wins elsewhere: most Go doors still lose
per-call to Go's stdlib.** Go's parsers are simply excellent (`time.Parse(RFC3339Nano)` at
~21 ns, `strconv.Atoi` at ~4 ns on the machine below), and every HyperCast call copies its
text into the core's memory and runs the core as translated, bounds-checked Go. Against the
cgo binding it replaced, it is mixed: `Bool` 1.5x and `Uuid` 1.2x faster, `I32`, `TimeOfDay` and
`DateOnlyOrdered` 1.1x faster, since no foreign call is paid any more. The losses are contention
(3.0x), a rejected cast (1.7x), a declaration carrying every symbol and the messy date-time door
with declared periods (1.3x and 1.5x), and 1.1x to 1.2x on the grouped integers, `Detect`,
`Timestamp`, the ISO span door and the plain date-time door, all in the table below. The
declared-order date door and the messy date-time door both beat `time.Parse` with a layout. **0 B, 0 allocs on every door** on the success path, under stock Go and
TinyGo, for any text of up to 65,360 bytes ([`MaxText`](#maxtext)), the checked doors
included (a checked door's declaration shares that scratch with its text), except that a
`[]byte` input to the text doors is copied into the `string` they return; a failed cast
allocates exactly its `*Fault`, `*Failure` or `*Violation[V]` (and, at the text door, the copy of a `[]byte` input's text it carries), and `allocs_test.go` holds both counts as
tests. In Go specifically,
this binding earns its keep on the vocabulary, the closed error contract, and cross-language
agreement — not per-call speed. The tabular layer,
[HyperTabular](https://github.com/SkunkWerkx/HyperTabular), is where a per-call cost amortizes:
it calls once per chunk, not once per cell.

## Benchmarks

`go test -bench=. -benchmem ./...`. Apple M6 (Mac18,5, 12 cores), macOS 27.2, Go 1.27.3,
2026-11-05, on the core with the checked doors and the white-space trim: six runs of each binding interleaved in one
session, benchstat medians with benchstat's spread. The cgo column is the binding this one
replaced (its tree as it stood before the translated core), linked against a static archive
built from the same `rust/` (`cargo staticlib`), with this same benchmark file. Other work
shared the machine (load average 2.3 to 3.5); differences benchstat did not call significant
are marked ~.

| Door | translated core | cgo binding | | stdlib |
| --- | ---: | ---: | --- | ---: |
| `Bool` | 15.3 ns ±2% | 23.3 ns | 1.5x faster | 1.4 ns `strconv.ParseBool` |
| `I32` | 40.4 ns ±1% | 42.9 ns | 1.1x faster | 3.6 ns `strconv.Atoi` |
| `I32` (grouped) | 49.4 ns ±1% | 44.2 ns | **1.1x slower** | — |
| `I32` (grouped), 12 goroutines, aggregate | 20.4 ns ±5% | 6.8 ns | **3.0x slower** | — |
| `I32` (grouped), `$` declared | 50.0 ns ±2% | 46.5 ns | **1.1x slower** | — |
| `I32` (grouped), every symbol declared | 92.8 ns ±2% | 70.4 ns | **1.3x slower** | — |
| `I32("1,234x567")`, rejected | 107 ns ±1%, 1 alloc | 64.2 ns, 1 alloc | **1.7x slower** | 14.6 ns, 2 allocs `strconv.Atoi("1234x567")` |
| `F64` | 50.5 ns ±1% | 50.2 ns | ~ | 14.4 ns `strconv.ParseFloat` |
| `F64` (`1.234.567,89`), declared | 50.4 ns ±2% | 50.1 ns | ~ | — |
| `F64` (`1.234.567,89`), `Detect` | 69.6 ns ±1% | 58.0 ns | **1.2x slower** | — |
| `Uuid` | 22.8 ns ±2% | 28.0 ns | 1.2x faster | 12.2 ns `google/uuid.Parse` |
| `Timestamp` | 40.5 ns ±2% | 33.8 ns | **1.2x slower** | 20.8 ns `time.Parse(RFC3339Nano)` |
| `Span` (ISO) | 32.7 ns ±2% | 28.6 ns | **1.1x slower** | 27.8 ns `ParseDuration` (Go dialect — different grammar) |
| `TimeOfDay` | 24.9 ns ±2% | 26.9 ns | 1.1x faster | — |
| `DateTime` (`1/7/2026 3:04 PM`) | 39.2 ns ±1% | 31.4 ns | **1.2x slower** | 46.6 ns `time.Parse` w/ layout |
| `DateTimeWith`, `vorm.`/`nachm.` declared | 72.9 ns ±3% | 48.6 ns | **1.5x slower** | — |
| `DateOnlyOrdered` (`1/7/2026`) | 21.8 ns ±2% | 24.9 ns | 1.1x faster | 28.1 ns `time.Parse` w/ layout |

Every success row is 0 B and 0 allocations in both bindings. Separator detection costs
~19 ns here (69.6 against 50.4 ns for the same text under a declared eurozone format). A
declared symbol costs text that does not carry it: `I32("1,234,567")` is 49.4 ns under
`Invariant`, 50.0 ns under a format declaring `$`, and 92.8 ns under one declaring every
symbol there is; declare what the feed actually carries.

`-unsafe` is measured apart: the same wasm translated with and without it, six interleaved runs
in one session on the same day, makes every door 2% to 37% faster except the contended one, which
is level (`DateOnlyOrdered` 22.1 ns against 35.2, `Bool` 15.6 against 17.2). `google/uuid.Parse`,
which never touches the core, read 8% slower in the flag's binaries in every run: that is code
layout rather than the flag, and it is the size of effect to allow for in any small row here.

The table that follows **predates the translated core**: it measured the cgo binding for
0.9.0 on linux-x64 (an Intel Core i9-11900H, go1.27.2), one session, median of five runs,
and has not been re-run since.

| Door | HyperCast (cgo, 0.9.0) | stdlib |
| --- | ---: | ---: |
| `Timestamp` | 90 ns, 0 allocs | 58 ns `time.Parse(RFC3339Nano)` |
| `I32` | 103 ns, 0 allocs | 9 ns `strconv.Atoi` |
| `I32` (grouped) | 108 ns, 0 allocs | — |
| `F64` | 121 ns, 0 allocs | 30 ns `strconv.ParseFloat` |
| `Uuid` | 76 ns, 0 allocs | 33 ns `google/uuid.Parse` |
| `Span` (ISO) | 78 ns, 0 allocs | 73 ns `ParseDuration` (Go dialect — different grammar) |
| `Bool` | 60 ns, 0 allocs | 3 ns `strconv.ParseBool` |
| `TimeOfDay` | 69 ns, 0 allocs | — |
| `DateTime` (`1/7/2026 3:04 PM`) | 84 ns, 0 allocs | 118 ns `time.Parse` w/ layout |
| `DateOnlyOrdered` (`1/7/2026`) | 67 ns, 0 allocs | 72 ns `time.Parse` w/ layout |

## Verifying build provenance

Go has no package registry to attest, and this module ships no binary: `go get` resolves
straight from the `go/v<version>` git tag against this repo, and what it fetches is source,
the generated core included. That file's provenance is reproducible generation, claimed
exactly: `internal/core/core.go` is the deterministic output of wasm2go over the crate at that
tag built with the pinned rustc, CI regenerates it with those pins on every pull request that
touches the Go binding, the core or the workflows, and on every release-mode run, and fails
on any diff, and anyone can repeat it with `go generate` in a checkout
([Building and regenerating](#building-and-regenerating)). No attestation is claimed for the
intermediate wasm, which is not a release asset. The root README's
[Provenance](../README.md#provenance) section has the picture for the artifacts the release
does ship: the inventory, which are signed from where, and the gates every byte passes before
publication.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
