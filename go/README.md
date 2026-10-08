# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Go Reference](https://pkg.go.dev/badge/github.com/SkunkWerkx/HyperCast/go.svg)](https://pkg.go.dev/github.com/SkunkWerkx/HyperCast/go)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**Go's own union idiom — `(value, *Fault)` — carrying the verdict of every cast: the value,
or a closed reason plus the exact byte span that offended. `*Fault` implements `error` for
composition, but the doors never panic on input; a panic here means a caller bug, never
data.**

Allocation-lean scalar casts — booleans, the full integer family, reals, exact decimals,
UUIDs, temporals — calling directly into the native `hypercast` Rust core, **linked into
your binary through cgo** (`backend_static.go`). The core is a static library on the link
line: nothing is embedded, nothing is extracted, nothing is `dlopen`ed, nothing can fail to
load, and the binary runs from a read-only filesystem or a `scratch` image. `go get` is the
whole install; a C compiler at build time is the one requirement (see
[Requirements](#requirements)). [TinyGo](#in-the-browser-tinygo) links the same core into
WebAssembly, browser included.

```go
import hypercast "github.com/SkunkWerkx/HyperCast/go"

value, fault := hypercast.I32("(1,234)", hypercast.Invariant)
if fault != nil {
    log.Printf("%s at byte %d", fault.Reason, fault.Offset)
}
// value == -1234, accounting negative

ts, fault := hypercast.Timestamp("2026-01-02T15:04:05.123456789+05:00")
// a UTC time.Time at full nanosecond fidelity
```

## Install

```sh
go get github.com/SkunkWerkx/HyperCast/go
```

Requires Go 1.26 or later (the `go` directive in `go.mod`). The import path ends in `/go`
while the package is named `hypercast`, so spell the name out in the import, as above —
`goimports` adds it for you, but a hand-written `import "github.com/SkunkWerkx/HyperCast/go"`
reads as if the package were called `go`.

Go modules have no separate registry — `go get` resolves straight from a git tag, and because
this module lives in a monorepo subdirectory its tags are prefixed (`go/vX.Y.Z`). The core's
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

Every other build fails at compile time, by name:

```
undefined: hypercast_needs_cgo_and_a_C_compiler_on_linux_darwin_or_windows_amd64_arm64__set_CGO_ENABLED_1__for_wasm_build_with_TinyGo
```

That is `CGO_ENABLED=0` (which is also Go's default for a cross-compile — see
[Building and cross-compiling](#building-and-cross-compiling)), any OS or architecture
outside the ones above (Android and the iOS simulator on an Intel Mac among them, which
Go's own rules would otherwise count as Linux and macOS), and stock Go compiled to
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
in headless Chrome: every ABI shape — plain, numeric with a format and a currency symbol,
discriminated — a fault with its span, and the core's version against `rust/Cargo.toml`.

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
v, fault := hypercast.Scalar[time.Time]("2026-01-02T15:04:05+05:00", hypercast.Invariant)
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
    DecimalSep: '.', GroupSep: ',', Styles: hypercast.AllStyles, Currency: "$",
})
// d == Decimal{Lo: 12345, Scale: 1, Negative: true}; d.String() == "-1234.5"
```

### NumFormat and currency symbols

Every numeric door takes a `NumFormat` — the caller's declared notation, never a guess:
the decimal and group separators, the `NumStyles` lenience flags, and an optional
`Currency` symbol. `Invariant` is `'.'`/`','` with `AllStyles` and no symbol;
`Detect` adds `SeparatorDetect`. `AllStyles` is every lenience including `CurrencySymbol`
(and excluding `SeparatorDetect`), so keyed literals that predate the symbol keep compiling
and keep their meaning.

With `CurrencySymbol` set and a `Currency` declared, the symbol is accepted once, whole, at
either edge of the numeric body: leading, before or after a sign (`$5`, `-$5`, `$ -5`), or
trailing (`5 €`, `1.234,50 kr.`), with optional ASCII whitespace between symbol and digits;
accounting parentheses wrap symbol and digits together (`($5)`). It never takes part in
the digit scan, so a symbol containing a separator character (`kr.` under `.` grouping) is
fine. A symbol declared while the style is off is `Malformed` at the symbol; the style with
no symbol declared is a no-op. The symbol is at most 16 UTF-8 bytes and may not contain an
ASCII digit or ASCII whitespace — anything else is a caller bug and panics, the way equal
separators do.

```go
euros := hypercast.NumFormat{DecimalSep: ',', GroupSep: '.', Styles: hypercast.AllStyles, Currency: "€"}
value, fault := hypercast.F64("€ 1.234,50", euros) // 1234.5
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
log.Printf("hypercast core %s", hypercast.NativeVersion())
```

`ErrNativeUnavailable` is deprecated: nothing returns it or panics with it any more, and it
stays only so code that tests for it keeps compiling.
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
- `RawNumFormat` — the 32 bytes a format crosses as, with the symbol inline up to
  `CurrencyMaxBytes` (16). `NumFormat.Raw()` builds one, returning the doors' check as an
  error instead of their panic.
- `Valid()` on `UnixPrecision`, `DateOrder` and `ExcelEpoch`; `UnixPrecisionFromCode`,
  `DateOrderFromCode`, `ExcelEpochFromCode` and `ReasonFromCode` turn an ABI code back into
  the type, and `false` for any other value.
- `FaultFromCode(code, offset, length)` — the `*Fault` a nonzero verdict code and its span
  name; `false` when the code names no reason, a binding bug rather than data.
- `FormatVersion(packed)` — a `*_version()` export's `major<<16 | minor<<8 | patch` word as
  `"major.minor.patch"`.

```go
raw, err := hypercast.NumFormat{DecimalSep: ',', GroupSep: '.', Styles: hypercast.AllStyles, Currency: "€"}.Raw()
// raw crosses to the other library's C ABI; its verdicts come back as codes and spans:
fault, ok := hypercast.FaultFromCode(code, offset, length)
```

## Platforms

| Platform | Archive linked | Also links |
| --- | --- | --- |
| Linux x64 / arm64, glibc and musl (Alpine) | `staticlib/linux_amd64`, `staticlib/linux_arm64` | the C library |
| macOS x64 / arm64 | `staticlib/darwin_amd64`, `staticlib/darwin_arm64` | the C library |
| Windows x64 / arm64 | `staticlib/windows_amd64`, `staticlib/windows_arm64` | nothing (the C runtime) |
| iOS arm64 — device, simulator | `staticlib/ios_arm64`, `staticlib/iossimulator_arm64` | the C library |
| Mac Catalyst arm64 / x64 | `staticlib/maccatalyst_arm64`, `staticlib/maccatalyst_amd64` | the C library |
| WebAssembly under [TinyGo](#in-the-browser-tinygo) — browser, WASI | `staticlib/wasm` | nothing |

Each build names one archive on its link line and that is all it takes from this module:
the binary carries the core for its own platform, writes nothing to disk, and starts
without touching the filesystem.

**One archive serves glibc and musl.** cgo has no build constraint that tells the two C
libraries apart, so the Linux archives are the core built for the musl target, which asks
the C library for nothing glibc and musl do not both have (`memcpy`, `memset`, `bcmp`,
`abort`).

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
fetched with `go get` has no such archive, so the tag fails at link time there.

This repo's own CI runs `go test ./...` natively, never cross-compiled, on every leg —
Linux and Windows on x64 and arm64, macOS on arm64 — and on Alpine. The iOS simulator run
in `test-apple-mobile` is the one exception; see [iOS and Mac Catalyst](#ios-and-mac-catalyst).

## Why not `strconv` / `time.Parse`?

1. **Verdicts with location** — a closed reason plus the offending span, against
   `strconv.NumError`'s wrapped string.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators and currency symbols, radix prefixes, all five .NET
   `Guid` text forms plus `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other
   binding, held by the shared corpus (full corpus replay on every platform, and in the
   browser under TinyGo).

**The honest trade-off, stated as plainly as the wins elsewhere: every Go door but one
loses per-call to Go's stdlib.** Go's parsers are simply excellent (`time.Parse(RFC3339Nano)`
at ~44 ns, `strconv.Atoi` at ~7 ns), and every HyperCast call pays a cgo crossing of about
50 ns. The exception is the messy date-time door, which beats `time.Parse` with a layout by
1.3x because that is the one stdlib path slow enough to absorb the crossing. The crossing
is all a door pays: any Go pointer handed to cgo escapes to the heap, so the C shims in
`backend_static.go` declare the out-value, fault span and format on their own stack and
return the verdict by value as one struct, and no Go pointer crosses but the input bytes —
**0 B, 0 allocs on every door** on the success path. (A failed cast allocates exactly its
`*Fault`; `allocs_test.go` holds both counts as tests.) In Go specifically, this binding
earns its keep on the vocabulary, the closed error contract, and cross-language agreement —
not per-call speed. The batch/tabular layer (round three) is where the crossing amortizes
to zero.

## Benchmarks

`go test -bench=. -benchmem ./...`. Measured on linux-x64 (an Intel Core i9-11900H, go1.27.1),
one session, median of three runs:

| Door | HyperCast | stdlib |
| --- | ---: | ---: |
| `Timestamp` | **73 ns, 0 allocs** | 45 ns `time.Parse(RFC3339Nano)` |
| `I32` | **74 ns, 0 allocs** | 7 ns `strconv.Atoi` |
| `I32` (grouped) | **82 ns, 0 allocs** | — |
| `F64` | **82 ns, 0 allocs** | 24 ns `strconv.ParseFloat` |
| `Uuid` | **62 ns, 0 allocs** | 26 ns `google/uuid.Parse` |
| `Span` (ISO) | **62 ns, 0 allocs** | 60 ns `ParseDuration` (Go dialect — different grammar) |
| `Bool` | **48 ns, 0 allocs** | 2 ns `strconv.ParseBool` |
| `TimeOfDay` | **56 ns, 0 allocs** | — |
| `DateTime` (`1/7/2026 3:04 PM`) | **65 ns, 0 allocs** | 97 ns `time.Parse` w/ layout |
| `DateOnlyOrdered` (`1/7/2026`) | **57 ns, 0 allocs** | 57 ns `time.Parse` w/ layout |

Separator detection costs ~6 ns: `1.234.567,89` under `Detect` is 95 ns against 90 ns for
the same text under a declared eurozone format.

## Verifying build provenance

Go has no package registry to attest — `go get` resolves straight from the `go/vX.Y.Z` git
tag against this repo. The static libraries committed under `go/staticlib/` (staged by
`stage-native-binaries.yml`, which verifies each one before committing it) each carry their
own build-provenance attestation from the build in `SkunkWerkx/.github` — so verifying
needs `--signer-repo` alongside `--repo`, or `gh` reports a bare `verifying with issuer
"sigstore.dev"` that reads like a bad signature but is only an identity mismatch:

```sh
gh attestation verify go/staticlib/linux_amd64/libhypercast.a \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
```

See [csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance)
for more on why `--signer-repo` is needed for some artifacts here and not others.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
