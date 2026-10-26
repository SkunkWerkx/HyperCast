# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Go Reference](https://pkg.go.dev/badge/github.com/SkunkWerkx/HyperCast/go.svg)](https://pkg.go.dev/github.com/SkunkWerkx/HyperCast/go)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**Go's own union idiom — `(value, *Fault)` — carrying the verdict of every cast: the value,
or a closed reason plus the exact byte span that offended. `*Fault` implements `error` for
composition, but the doors never panic on input; a panic here means a caller bug, never
data.**

Allocation-lean scalar casts — booleans, characters, the full integer family, reals, exact
decimals, UUIDs, temporals — calling directly into the native `hypercast` Rust core, **linked into
your binary through cgo** (`backend_static.go`). The core is a static library on the link
line: nothing is embedded, nothing is extracted, nothing is `dlopen`ed, nothing can fail to
load, and the binary runs from a read-only filesystem or a `scratch` image. `go get` is the
whole install; a C compiler at build time is the one requirement (see
[Requirements](#requirements)). [TinyGo](#in-the-browser-tinygo) links the same core into
WebAssembly, browser included.

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

Requires Go 1.26 or later (the `go` directive in `go.mod`). The import path ends in `/go`
while the package is named `hypercast`, so spell the name out in the import, as above —
`goimports` adds it for you, but a hand-written `import "github.com/SkunkWerkx/HyperCast/go"`
reads as if the package were called `go`.

Go modules have no separate registry — `go get` resolves straight from a git tag, and because
this module lives in a monorepo subdirectory its tags are prefixed (`go/v<version>`). The core's
static libraries under `staticlib/` are committed to git and restaged for each
release by `stage-native-binaries.yml`: a `go get` consumer has no packing step, so whatever is
literally in the tree at the resolved tag is what gets linked (see `staticlib/README.md`).
`github.com/google/uuid` is the module's only dependency.

## Requirements

cgo, and so a C compiler wherever the module is **built** — nothing at run time:

| Building on / for | C compiler |
| --- | --- |
| Linux x64 / arm64, glibc or musl | gcc or clang (`build-essential`; on Alpine, `apk add build-base`) |
| macOS x64 / arm64 | the Xcode command-line tools (`xcode-select --install`) |
| Windows x64 | MinGW-w64 gcc |
| Windows arm64 | [llvm-mingw](https://github.com/mstorsjo/llvm-mingw) |
| iOS, the iOS simulator, Mac Catalyst | Xcode's clang for that platform's SDK; see [iOS and Mac Catalyst](#ios-and-mac-catalyst) |
| Android arm64 / x64 | the NDK's clang for that ABI as `CC`; see [Android](#android) |

Every other build fails at compile time, by name:

```
undefined: hypercast_needs_cgo_and_a_C_compiler_on_linux_darwin_or_windows_amd64_arm64__set_CGO_ENABLED_1__for_wasm_build_with_TinyGo
```

That is `CGO_ENABLED=0` (which is also Go's default for a cross-compile — see
[Building and cross-compiling](#building-and-cross-compiling)), any OS or architecture
outside the ones above (the iOS simulator on an Intel Mac among them, which Go's own rules
would otherwise count as macOS), and stock Go compiled to
WebAssembly (`GOOS=wasip1`, `GOOS=js`): Go's wasm toolchain links Go code only, with no cgo, so a foreign library has nowhere to go.
For WebAssembly, build with [TinyGo](#in-the-browser-tinygo), which links the core there,
browser included.

## In the browser (TinyGo)

[TinyGo](https://tinygo.org) 0.42 or later compiles this module to WebAssembly with the core
linked in, the way the C# package does for Blazor: TinyGo links with `wasm-ld` and has cgo,
so `backend_tinygo.go` names `staticlib/wasm/libhypercast.a` — the core built for
`wasm32-wasip1` — on its link line. No code changes and no extra files to ship: the core is
inside your `.wasm`.

```sh
tinygo build -target=wasm -no-debug -opt=z -o main.wasm .
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

Use TinyGo's own `wasm_exec.js`, not stock Go's. The core itself imports nothing — parsing
needs no clock, no randomness and no I/O — so the page loads one module and that is all.
`-target=wasip1` works the same way under a WASI runtime (wasmtime, for one). CI builds
[`internal/tinygosmoke`](internal/tinygosmoke) this way on every pull request and runs it
in headless Chrome: every ABI shape — plain, numeric with a format (a currency symbol, and
every other symbol the layout carries), discriminated, the civil date-time door with
declared day periods — a fault with its span, and the core's version against
`rust/Cargo.toml`.

TinyGo's cgo is narrower than Go's — no build constraints on `#cgo` lines, no `${SRCDIR}`,
no C structs by value — which is why the browser build has a backend file of its own
rather than more lines in `backend_static.go`; the file's header has the details. The one
that changes how a call crosses is the last: `backend_static.go`'s shims hand the verdict
back by value, so under TinyGo each door instead passes the core pointers to its result's
out-value and fault span, the way the core's exports take them anyway.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.

## Doors

Doors are generic over `string | []byte` — both cross zero-copy (the core only reads).
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
[The native library](#the-native-library-available-loaderror-nativeversion) below.

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

## The native library: `Available`, `LoadError`, `NativeVersion`

The core is part of the binary, so there is nothing to load and nothing that can fail, and
no door can panic for want of the library. The three probes every binding carries are here
so code written against them works unchanged:

- `Available()` — always `true`.
- `LoadError()` — always `nil`.
- `NativeVersion()` — `"major.minor.patch"` as the linked core reports it about itself, read
  through the ABI rather than from this module, so a deployment can confirm which build of
  the archive went into the binary. It cannot fail.

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
  `[]Decimal` or `[]Duration` in place.
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

| Platform | Archive linked | Also links |
| --- | --- | --- |
| Linux x64 / arm64, glibc and musl (Alpine) | `staticlib/linux_amd64`, `staticlib/linux_arm64` | the C library |
| macOS x64 / arm64 | `staticlib/darwin_amd64`, `staticlib/darwin_arm64` | the C library |
| Windows x64 / arm64 | `staticlib/windows_amd64`, `staticlib/windows_arm64` | nothing (the C runtime) |
| iOS arm64 — device, simulator | `staticlib/ios_arm64`, `staticlib/iossimulator_arm64` | the C library |
| Mac Catalyst arm64 / x64 | `staticlib/maccatalyst_arm64`, `staticlib/maccatalyst_amd64` | the C library |
| [Android](#android) arm64 / x64 (API 21+) | `staticlib/android_arm64`, `staticlib/android_amd64` | Bionic, the C library |
| WebAssembly under [TinyGo](#in-the-browser-tinygo) — browser, WASI | `staticlib/wasm` | nothing |

Each build names one archive on its link line and that is all it takes from this module:
the binary carries the core for its own platform, writes nothing to disk, and starts
without touching the filesystem.

**One archive serves glibc and musl.** cgo has no build constraint that tells the two C
libraries apart, so the Linux archives are the core built for the musl target, which asks
the C library for nothing glibc and musl do not both have (`memcpy`, `memset`, `bcmp`).

**Windows links the MSVC archive**, the same bytes the C# package's Native AOT publish
links. MinGW's linker reads MSVC's COFF objects, and what the archive asks of the C runtime
resolves against `msvcrt.dll` through MinGW's own import library, so the link line names
nothing else.

### iOS and Mac Catalyst

Go builds for all three as `GOOS=ios`, with cgo and the platform's own clang, which is what
[`gomobile`](https://pkg.go.dev/golang.org/x/mobile/cmd/gomobile) arranges and what Go's own
`misc/ios/clangwrap.sh` does for the simulator. A Mach-O object says which platform it was
built for and the linker refuses a mismatch, so each has its own archive, and since the
three are one `GOOS`/`GOARCH` pair to Go, build tags choose between them:

| Building for | Tags | Archive |
| --- | --- | --- |
| An iOS device | none | `staticlib/ios_arm64` |
| The iOS simulator, Apple silicon | `iossimulator` | `staticlib/iossimulator_arm64` |
| Mac Catalyst | `maccatalyst` | `staticlib/maccatalyst_arm64`, `staticlib/maccatalyst_amd64` |

`gomobile` sets `maccatalyst` itself for that target. Nothing sets `iossimulator`: gomobile
builds a device and a simulator with the same tags, so a simulator build passes
`-tags iossimulator` by hand, and a build that omits it links the device archive and fails
at the simulator link. A `gomobile bind` covering both in one invocation gives them one set
of tags, so they take two invocations. The simulator on an Intel Mac has no archive and is
a compile error.

CI checks that every platform and tag combination selects its own archive
(`.github/scripts/check_go_archives.sh`), runs this suite in an iOS simulator through Go's
`misc/ios/go_ios_exec.go`, with the corpus staged as `testdata/corpus` since that wrapper
carries nothing above the module into the simulator, and links a device build. Mac Catalyst
is not linked from Go there; its archives are the ones the Swift and C# bindings link and
run in the same job.

### Android

`GOOS=android` with cgo, the NDK's clang as `CC`, as `gomobile` arranges and as any cgo
package on Android needs:

```shell
CC=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang \
  GOOS=android GOARCH=arm64 CGO_ENABLED=1 go build ./...
```

(`x86_64-linux-android21-clang` and `GOARCH=amd64` for the emulator.) `GOOS=android` also
satisfies Go's `linux` constraint, so the Linux link lines exclude it and it takes its own
archives, built for Android against Bionic. Page alignment is the final link's: NDK r28 and
later align to the 16 KB pages Android 15 devices may use by default, and an older NDK
needs `-extldflags=-Wl,-z,max-page-size=16384`.

CI checks that both select their own archive (`.github/scripts/check_go_archives.sh`),
cross-compiles this whole suite for `android/amd64`, and runs it in an x86_64 emulator whose
image uses 16 KB pages (`.github/scripts/android_build_suite.sh` and
`android_device_test.sh`, which run the same way against a local emulator); `android/arm64`
is linked.

### Deploying

- **A fully static Linux binary** — `go build -ldflags '-linkmode external -extldflags
  -static'` gives a binary with no dependencies at all, which runs in an empty, read-only
  `scratch` container. It needs a static C library to link against; Alpine's musl has one.
- **Shipping to Alpine** — build on Alpine (`apk add build-base`), or link fully statically
  as above. An ordinary dynamically linked build from a glibc machine needs the builder's
  glibc, which a bare Alpine image does not have.
- **Anywhere else** — the binary needs only the C library it was linked against.

## Building and cross-compiling

`CGO_ENABLED` defaults to `1` on a native build and to `0` the moment `GOOS`/`GOARCH`
differ from the host:

```
$ go env CGO_ENABLED                # native
1
$ GOARCH=arm64 go env CGO_ENABLED   # cross, same OS, different arch
0
$ GOOS=windows go env CGO_ENABLED   # cross, different OS
0
```

So a plain cross-compile lands on the compile error above. Cross-compiling takes a cross
C compiler and `CGO_ENABLED=1` set explicitly:

```sh
CC=x86_64-w64-mingw32-gcc GOOS=windows GOARCH=amd64 CGO_ENABLED=1 go build ./...
CC=aarch64-linux-gnu-gcc  GOOS=linux   GOARCH=arm64 CGO_ENABLED=1 go build ./...
```

A native build with no C compiler installed fails in `runtime/cgo` with `C compiler "gcc"
not found` — install the one for your platform from [Requirements](#requirements).
GitHub's `ubuntu-latest` and `macos-latest` runner images ship one by default (`gcc` and the
Xcode command-line tools' `clang`).

The `hypercast_local` build tag is for working on the core in a checkout of this repository:
`go test -tags hypercast_local ./...` links the archive `.github/scripts/local-core.sh` builds
from the checkout, under `rust/target/local-core/`, in place of the committed one. A module
fetched with `go get` has no such archive, so the tag fails at link time there. Without the
tag, outside CI, the suite refuses to run when the checkout's `rust/` differs from the commit
the committed archives were built from (`-count=1` after a core change, since `go test`
replays a cached pass).

This repo's own CI runs `go test ./...` natively, never cross-compiled, on every leg —
Linux and Windows on x64 and arm64, macOS on arm64 — and on Alpine. The iOS simulator run
in `test-apple-mobile` and the Android emulator run in `test-android` are the two
exceptions; see [iOS and Mac Catalyst](#ios-and-mac-catalyst) and [Android](#android).

## Why not `strconv` / `time.Parse`?

1. **Verdicts with location** — a closed reason plus the offending span, against
   `strconv.NumError`'s wrapped string.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, signs, currency symbols, percent symbols and native
   digits, a culture's own AM/PM, radix prefixes, all five .NET `Guid` text forms plus
   `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other
   binding, held by the shared corpus (full corpus replay on every platform, and in the
   browser under TinyGo).

**The honest trade-off, stated as plainly as the wins elsewhere: every Go door but the two
declared-order date doors loses per-call to Go's stdlib.** Go's parsers are simply excellent
(`time.Parse(RFC3339Nano)` at ~58 ns, `strconv.Atoi` at ~9 ns), and every HyperCast call
pays a cgo crossing of about 50 ns. The exceptions are the messy date-time door, which beats
`time.Parse` with a layout by 1.4x, and the declared-order date door, which edges it:
`time.Parse` with a layout is the one stdlib path slow enough to absorb the crossing. The
crossing is all a door pays: any Go pointer handed to cgo escapes to the heap, so the format and
the day periods cross by value, the C shims in `backend_static.go` declare them, the
out-value and the fault span on their own stack and return the verdict by value as one
struct, and no Go pointer crosses but the input bytes —
**0 B, 0 allocs on every door** on the success path. (A failed cast allocates exactly its
`*Fault`; `allocs_test.go` holds both counts as tests.) In Go specifically, this binding
earns its keep on the vocabulary, the closed error contract, and cross-language agreement —
not per-call speed. The tabular layer, [HyperTabular](https://github.com/SkunkWerkx/HyperTabular),
is where the crossing amortizes to zero: it crosses once per chunk, not once per cell.

## Benchmarks

`go test -bench=. -benchmem ./...`. Measured for 0.9.0 on linux-x64 (an Intel Core i9-11900H,
go1.27.2), one session, median of five runs:

| Door | HyperCast | stdlib |
| --- | ---: | ---: |
| `Timestamp` | **90 ns, 0 allocs** | 58 ns `time.Parse(RFC3339Nano)` |
| `I32` | **103 ns, 0 allocs** | 9 ns `strconv.Atoi` |
| `I32` (grouped) | **108 ns, 0 allocs** | — |
| `F64` | **121 ns, 0 allocs** | 30 ns `strconv.ParseFloat` |
| `Uuid` | **76 ns, 0 allocs** | 33 ns `google/uuid.Parse` |
| `Span` (ISO) | **78 ns, 0 allocs** | 73 ns `ParseDuration` (Go dialect — different grammar) |
| `Bool` | **60 ns, 0 allocs** | 3 ns `strconv.ParseBool` |
| `TimeOfDay` | **69 ns, 0 allocs** | — |
| `DateTime` (`1/7/2026 3:04 PM`) | **84 ns, 0 allocs** | 118 ns `time.Parse` w/ layout |
| `DateOnlyOrdered` (`1/7/2026`) | **67 ns, 0 allocs** | 72 ns `time.Parse` w/ layout |

Separator detection costs ~16 ns: `1.234.567,89` under `Detect` is 141 ns against 125 ns
for the same text under a declared eurozone format.

A format is checked and laid out on every call — the two separators in a few nanoseconds,
every further symbol a little more — and the whole 92-byte layout crosses by value, so a
declared symbol costs text that does not carry it: `I32("1,234,567")` is 108 ns under
`Invariant`, 121 ns under a format declaring `$`, and 204 ns under one declaring every
symbol there is (currency separators, both signs, percent, native digits). Declare what the
feed actually carries. Day periods are the same story: `DateTime("1/7/2026 3:04 PM")` is
84 ns, and `DateTimeWith` under a format declaring `vorm.`/`nachm.` reads the same text in
135 ns. Rejection allocates where success does not: `I32("1,234x567")` takes 171 ns and one
allocation, the `*Fault` it returns, against 49 ns and two allocations for `strconv.Atoi`
failing on `1234x567`.

## Verifying build provenance

Go has no package registry to attest: `go get` resolves straight from the `go/v<version>`
git tag against this repo, and what it fetches is the tree at that tag, the static libraries
under `go/staticlib/` included. Each of them verifies from the release tag alone: the release
attestation, signed by this repository's `release.yml` at the root tag, names every one of
them, with the release inventory as its predicate. The same file sits in the module cache at
`$(go env GOMODCACHE)/github.com/!skunk!werkx/!hyper!cast/go@v<version>/staticlib/`.

```sh
gh attestation verify go/staticlib/linux_amd64/libhypercast.a --repo SkunkWerkx/HyperCast \
  --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

Each archive also carries build provenance from `hyper-build-static.yml` in
`SkunkWerkx/.github`, at the preparation commit the tagged commit was staged from
(`stage-native-binaries.yml` verifies that before committing a byte, and the staging commit's
`Release-Source-SHA` and `Release-CI-Run-ID` trailers name the commit and the run), so that
check needs `--signer-repo` beside `--repo`:

```sh
gh attestation verify go/staticlib/linux_amd64/libhypercast.a \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
```

Get the signer wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which
reads like a bad signature but is only an identity mismatch. The root README's
[Provenance](../README.md#provenance) section has the whole picture: the inventory, which
artifacts are signed from where, and the gates every byte passes before publication.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
