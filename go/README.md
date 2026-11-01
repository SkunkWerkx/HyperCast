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

The Rust core is one `wasm32-wasip1` module of about 150 KB that imports nothing at all:
parsing needs no clock, no randomness and no I/O. wasm2go translates it into about 1 MB of
Go source with no dependencies beyond the standard library, and that file is committed and
compiled with the rest of the package. The binding keeps an *instance* of it: the
translated module and a scratch region in the module's linear memory, where a door's text,
its `NumFormat` and its day periods are copied in and its value and fault span copied out.
Every door is an ordinary Go call into that code; nothing crosses a foreign boundary.

wasm2go's generated code bounds-checks every memory access and keeps the call stack in Go,
so a core bug, a guest stack overflow included, is a Go panic and never a corrupted heap. The
guest's stack is 64 KiB: regenerating the core at halving stack sizes and running the whole
suite, the corpus replay included, at each, 4096 bytes is the smallest size that passes
everything and 2048 the first that traps (a bounds panic from the generated code, never a
wrong verdict), so the shipped size is a 16x margin over the tested paths, measured rather
than proved. A call that panics inside the core abandons the instance it ran on, whose guest
stack it left mid-call, rather than hand it to the next caller.

wasm2go is pre-1.0 (v0.4.19 today) with one maintainer. It is pinned in a tool module of its
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
once is 19.7 ns a call in aggregate on the Apple M6, against 6.9 ns for the cgo binding this
replaced, which took no lock at all — **2.9x slower** ([Benchmarks](#benchmarks)). A single
goroutine's calls are faster than they were.

A fresh instance is 3 WebAssembly pages of linear memory, 192 KiB: the 64 KiB stack and the
core's data in 2 pages, plus one page of scratch.

## `MaxText`

A door's text is copied into the instance's scratch. The base scratch holds 65,376 bytes; a
longer text runs on a one-off instance sized for it and dropped after the call, so neither
the primary nor the pool keeps the pages, and that instance is the call's one allocation. The
core reads the text through a slice whose length may not exceed the guest's `isize::MAX`,
2^31 − 1 on wasm32, whatever the host's word size, so a door takes a text of at most
`MaxText` bytes: the smaller of that limit and what the guest and host can address beside the
module's own pages. On a 64-bit host that is 2,147,483,647. A longer text panics, as a caller
bug, before anything is allocated.

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
`AllStyles` and nothing else declared; `Detect` adds `SeparatorDetect`. `AllStyles` is
every lenience including `CurrencySymbol` (and excluding `SeparatorDetect` and
`TrailingSign`), so keyed literals that predate a field keep compiling and keep their
meaning.

| Field | Declares | Read |
| --- | --- | --- |
| `DecimalSep`, `GroupSep` | the number separators — 1 to 8 UTF-8 bytes each with no ASCII digit (`"."`, `","`, `" "`, `", "`), different from each other; the only required fields | always |
| `Styles` | the `NumStyles` leniences | — |
| `Currency` | the currency symbol (`"$"`, `"€"`, `"kr."`), at most 16 bytes | under `CurrencySymbol` |
| `CurrencyDecimalSep`, `CurrencyGroupSep` | the separators of a currency amount, where a culture points and groups money differently from numbers; empty keeps the number separator | when `Currency` is declared and found in the text; ignored under `SeparatorDetect`, whose detection governs the whole token |
| `MinusSign`, `PlusSign` | a further spelling of the sign (`"−"`, U+2212, in Estonian and Basque text) | wherever ASCII `-`/`+` are |
| `Percent` | a further spelling of the percent symbol (`"٪"`, `"％"`) | as a trailing `%` is, under `Percent` — the real and decimal doors; the integer doors never read a percent |
| `DigitZero` | native digits: the ten code points from this zero up (`'٠'` for the Arabic-Indic digits, `'０'` for fullwidth); `0` declares none | wherever a digit is — integer part, fraction, exponent; a `0x`/`0b` body stays ASCII |

The rule for every field past the separators: **the ASCII spelling is always read, and a
declared symbol is a further spelling read beside it.** Nothing that parsed before a symbol
was declared stops parsing once it is: `"-5"` is -5 under a format declaring `"−"`, and so
is `"−5"`; `"50%"` is 0.5 under a format declaring `"٪"`, and so is `"50٪"`; `"123"` is 123
under a format declaring `'٠'`, and so are `"١٢٣"` and `"12٣"`. Two signs in any spelling
are `Malformed` at the later one. Under `CurrencySymbol` with a `Currency` declared, the
symbol is accepted once, whole, at either edge of the numeric body: leading, before or
after a sign (`$5`, `-$5`, `$ -5`, `€−5`), or trailing (`5 €`, `1.234,50 kr.`), with
optional ASCII whitespace between symbol and digits; accounting parentheses wrap symbol and
digits together (`($5)`). It never takes part in the digit scan, so a symbol containing a
separator character (`kr.` under `.` grouping) is fine. A symbol declared while the style
is off is `Malformed` at the symbol; the style with no symbol declared is a no-op.

`TrailingSign` is the one style that is not in `AllStyles`: it permits the sign after the
digits (`5-`, `5 -`, `5+`, `5 €-`, `5- €`, `2.5%-`), in ASCII or the declared spelling,
once, never beside a leading sign and never inside parentheses. A culture whose negative
pattern is `n-` or `n -` sets it; everything else leaves it off, so `"5-"` stays
`Malformed` under `Invariant`.

A format the core would refuse is a caller bug and panics, never a verdict — the way equal
separators always have: a number separator that is empty or longer than 8 bytes, equal
number separators or equal separators in force for a currency amount, any symbol that is
not valid UTF-8 or carries an ASCII digit, a sign, percent or currency symbol carrying ASCII
whitespace, a `Currency` longer than 16 bytes, or a `DigitZero` that is ASCII or starts no
Unicode digit block. `NumFormat.Raw()` reports the same check as an `error` for code that
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
matched byte for byte, after the clock (`3:04 nachm.`, optional space before) or ahead of
it (`오후 3:04`, optional space after), and the English pair is read whatever is declared.
A `DateFormat` declaring no designators reads exactly what `DateTime` reads, and
`DateTime` itself is unchanged. Only the civil date-time door reads them:
`DateOnlyOrdered` has no time part.

A designator is at most 16 UTF-8 bytes, carries no ASCII digit, and neither starts nor
ends in ASCII whitespace (an inner space, as in Spanish `a. m.`, is fine); anything else
is a caller bug and panics, and `DateFormat.Raw()` reports it as an `error`.

```go
german := hypercast.DateFormat{Order: hypercast.DayMonthYear, AM: "vorm.", PM: "nachm."}
fmt.Println(hypercast.DateTimeWith("1.7.2026 3:04 nachm.", german)) // July 1st, 15:04

korean := hypercast.DateFormat{Order: hypercast.YearMonthDay, AM: "오전", PM: "오후"}
fmt.Println(hypercast.DateTimeWith("2026-07-01 오후 3:04", korean)) // the same wall clock
```

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
- `Valid()` on `UnixPrecision`, `DateOrder` and `ExcelEpoch`; `UnixPrecisionFromCode`,
  `DateOrderFromCode`, `ExcelEpochFromCode` and `ReasonFromCode` turn an ABI code back into
  the type, and `false` for any other value.
- `FaultFromCode(code, offset, length)` — the `*Fault` a nonzero verdict code and its span
  name; `false` when the code names no reason, a binding bug rather than data.
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
c`), then runs the wasm2go pinned in its own `go.mod`. The rustc is pinned as a
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
cgo binding it replaced, single calls are mostly faster, since no foreign call is paid any
more: `Bool` 2.3x, `DateOnlyOrdered` 1.7x, `I32` 1.3x. The losses are the civil date-time
doors (1.4–1.5x), a rejected cast (1.5x) and contention (2.9x), all in the table below. The
declared-order date door beats `time.Parse` with a layout, and the messy date-time door is
level with it. **0 B, 0 allocs on every door** on the success path, under stock Go and
TinyGo, for any text of up to 65,376 bytes ([`MaxText`](#maxtext)); a failed cast allocates
exactly its `*Fault`, and `allocs_test.go` holds both counts as tests. In Go specifically,
this binding earns its keep on the vocabulary, the closed error contract, and cross-language
agreement — not per-call speed. The tabular layer,
[HyperTabular](https://github.com/SkunkWerkx/HyperTabular), is where a per-call cost amortizes:
it calls once per chunk, not once per cell.

## Benchmarks

`go test -bench=. -benchmem ./...`. Apple M6 (Mac18,5, 12 cores), macOS, Go 1.27.3,
2026-11-01: six runs of each binding interleaved in one session, benchstat medians with
benchstat's spread. The cgo column is the binding this one replaced, built from the same
`rust/` (its archive built by the Forge's `build-static-libs.sh`), with this same benchmark
file. Other work shared the machine during the session, so several rows are noisy, and the
spread is printed with them; differences benchstat did not call significant are marked ~.

| Door | translated core | cgo binding | | stdlib |
| --- | ---: | ---: | --- | ---: |
| `Bool` | 10.7 ns ±40% | 24.1 ns | 2.3x faster | 1.4 ns `strconv.ParseBool` |
| `I32` | 33.6 ns ±5% | 43.5 ns | 1.3x faster | 3.6 ns `strconv.Atoi` |
| `I32` (grouped) | 37.6 ns ±4% | 45.0 ns | 1.2x faster | — |
| `I32` (grouped), 12 goroutines, aggregate | 19.7 ns ±3% | 6.9 ns | **2.9x slower** | — |
| `I32` (grouped), `$` declared | 43.3 ns ±4% | 46.9 ns | 1.1x faster | — |
| `I32` (grouped), every symbol declared | 71.8 ns ±2% | 70.0 ns | ~ | — |
| `I32("1,234x567")`, rejected | 96.0 ns ±3%, 1 alloc | 64.3 ns, 1 alloc | **1.5x slower** | 14.6 ns, 2 allocs `strconv.Atoi("1234x567")` |
| `F64` | 42.8 ns ±38% | 50.2 ns | 1.2x faster | 14.9 ns `strconv.ParseFloat` |
| `F64` (`1.234.567,89`), declared | 41.2 ns ±11% | 49.9 ns | 1.2x faster | — |
| `F64` (`1.234.567,89`), `Detect` | 54.6 ns ±11% | 59.7 ns | 1.1x faster | — |
| `Uuid` | 19.7 ns ±93% | 28.1 ns | ~ | 11.8 ns `google/uuid.Parse` |
| `Timestamp` | 34.5 ns ±77% | 34.5 ns | ~ | 21.1 ns `time.Parse(RFC3339Nano)` |
| `Span` (ISO) | 30.9 ns ±173% | 29.0 ns | ~ | 27.2 ns `ParseDuration` (Go dialect — different grammar) |
| `TimeOfDay` | 20.4 ns ±166% | 28.2 ns | 1.4x faster | — |
| `DateTime` (`1/7/2026 3:04 PM`) | 49.0 ns ±160% | 34.2 ns | **1.4x slower** | 46.9 ns `time.Parse` w/ layout |
| `DateTimeWith`, `vorm.`/`nachm.` declared | 75.8 ns ±51% | 52.0 ns | **1.5x slower** | — |
| `DateOnlyOrdered` (`1/7/2026`) | 15.5 ns ±19% | 26.0 ns | 1.7x faster | 28.0 ns `time.Parse` w/ layout |

Every success row is 0 B and 0 allocations in both bindings. Separator detection costs
~13 ns here (54.6 against 41.2 ns for the same text under a declared eurozone format). A
declared symbol costs text that does not carry it: `I32("1,234,567")` is 37.6 ns under
`Invariant`, 43.3 ns under a format declaring `$`, and 71.8 ns under one declaring every
symbol there is; declare what the feed actually carries.

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
