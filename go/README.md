# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Go Reference](https://pkg.go.dev/badge/github.com/SkunkWerkx/HyperCast/go.svg)](https://pkg.go.dev/github.com/SkunkWerkx/HyperCast/go)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**Go's own union idiom — `(value, *Fault)` — carrying the verdict of every cast: the value,
or a closed reason plus the exact byte span that offended. `*Fault` implements `error` for
composition, but the doors never panic on input; a panic here means a caller bug, never
data.**

Allocation-lean scalar casts — booleans, the full integer family, reals, exact decimals,
UUIDs, temporals — calling directly into the native `hypercast` Rust core. Which way is
chosen by the build, with the same public API every time:

- **cgo on Linux and macOS links the core in** (`backend_static.go`). The core is a static
  library on the link line: nothing is embedded, nothing is extracted, nothing is
  `dlopen`ed, and the binary runs from a read-only filesystem or a `scratch` image.
- **Everywhere else loads it** through [purego](https://github.com/ebitengine/purego)
  (`backend_purego.go`) — no cgo and no C compiler required. That is Windows always, and
  any build with `CGO_ENABLED=0`, which per Go's own defaults includes every cross-compile.
  This build embeds a shared library for every supported platform via `go:embed` and picks
  one at run time.
- **`-tags hypercast_wasm`** runs the same core as a WebAssembly module inside the process
  through wasmtime-go — see [WebAssembly (wasmtime-go)](#webassembly-wasmtime-go).

cgo is also 5-8x faster per call than purego; see Benchmarks.

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
this module lives in a monorepo subdirectory its tags are prefixed (`go/vX.Y.Z`). The native
binaries — the shared libraries under `native/{rid}/` and the static ones under
`staticlib/{goos}_{goarch}/` — are committed to git and kept fresh by
`stage-native-binaries.yml`: a `go get` consumer has no packing step, so whatever is
literally in the tree at the resolved tag is what gets linked or embedded (see
`native/README.md` and `staticlib/README.md`).

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

In a cgo build on Linux or macOS the core is part of the binary, so there is nothing to
load and nothing that can fail: `Available()` is always `true`, `LoadError()` always `nil`,
and no door can panic for want of the library. Every other build loads the core once, on
first use, and caches the outcome for the life of the process. A door returns
`(value, *Fault)`, and a `*Fault` is a verdict about the input — so
a library that never loaded has nowhere to go but a panic, and every door panics with an
error wrapping `ErrNativeUnavailable` around the specific reason: an unsupported platform,
no embedded build for it, a failed extraction or `dlopen`, a core that doesn't export the
ABI this binding was built against. Three entry points that are not doors probe the same
outcome up front, so a consumer keeping a fallback for a platform this module does not
cover gates on them instead of recovering around its first cast:

- `Available()` — `true` when the native library (or, under `-tags hypercast_wasm`, the
  wasm module) loaded and exports the ABI this binding was built against: every symbol
  resolved and `hypercast_version` answered. A `false` is permanent for the process. Never
  panics.
- `LoadError()` — `nil` when it loaded; otherwise the error the doors would panic with,
  reason included. Never panics.
- `NativeVersion()` — `"major.minor.patch"` as the loaded core reports it about itself, so
  a mismatch against the version this module was built for can be named before the first
  cast. Panics if the library did not load, like every door; the other two are the safe
  probes.

```go
if err := hypercast.LoadError(); err != nil {
	// errors.Is(err, hypercast.ErrNativeUnavailable) == true, and the message says why:
	//   hypercast: native library unavailable: dlopen failed: libgcc_s.so.1: cannot open
	//   shared object file: No such file or directory
	log.Fatal(err)
}
log.Printf("hypercast core %s", hypercast.NativeVersion())
```

[HyperUuid](https://github.com/SkunkWerkx/HyperUuid)'s Go binding has the same three entry
points and the same `ErrNativeUnavailable`, with one difference that follows from its API:
its functions already return `error`, so they return that error where these doors panic
with it.

Where the core is loaded — purego, or cgo with `-tags hypercast_dynamic` — loading means
extracting the embedded library to a temp file and `dlopen`ing it from there
(`native_extract.go`). The file is created in `os.TempDir()` — `TMPDIR` moves it — once per
process, and is not removed at exit. A build that links the core in does none of that.

## Platforms

| Platform | cgo build | `CGO_ENABLED=0` (purego) |
| --- | --- | --- |
| Linux x64 / arm64, glibc | linked in: `staticlib/linux_amd64`, `staticlib/linux_arm64` | loads `native/linux-x64`, `native/linux-arm64` — needs glibc 2.34 or newer, and `libgcc_s.so.1` |
| Linux x64 / arm64, musl (Alpine) | linked in, the same two archives | loads `native/linux-musl-x64`, `native/linux-musl-arm64` — needs musl libc, nothing else |
| macOS x64 / arm64 | linked in: `staticlib/darwin_amd64`, `staticlib/darwin_arm64` | loads `native/osx-x64`, `native/osx-arm64` |
| Windows x64 / arm64 | purego regardless of cgo | loads `native/win-x64`, `native/win-arm64` |

Anything else — another OS, or an architecture such as 386 or riscv64 — is reported as
`unsupported platform {GOOS}/{GOARCH}` inside `ErrNativeUnavailable`, never guessed at.

**Linked in.** A cgo build names one static library on its link line and that is all it
takes from this module: the binary carries the core for its own platform, where a build
that loads carries every platform's shared library — 3.2 MB against 6.7 MB for a program
that does nothing else. It needs nothing at run time beyond the C library it was linked
against, so `-ldflags '-linkmode external -extldflags -static'` gives a binary with no
dependencies at all, which runs in an empty, read-only container. One archive serves glibc
and musl alike: cgo has no build constraint that tells them apart, and the archive asks the
C library for nothing both do not have. CI runs the suite against it on Debian and on
Alpine, on both architectures.

**Loaded, on glibc.** The glibc shared libraries reference symbols up to `GLIBC_2.34`, which
is Debian 12, Ubuntu 22.04, RHEL 9 and Amazon Linux 2023 or later; on an older glibc the
load fails with the loader's own `version ... not found`. They also link `libgcc_s.so.1`,
which every mainstream glibc distribution ships and a minimal image may not: an image with
glibc but no `libgcc_s.so.1` (`gcr.io/distroless/base`, for one) fails the load with
`libgcc_s.so.1: cannot open shared object file`. None of this applies to a build that links
the core in.

**Loaded, on musl.** Which Linux shared library is loaded is decided at run time, by what the process is
actually running on: if `/proc/self/maps` shows a musl loader mapped (`ld-musl-*` or
`libc.musl-*`), the `linux-musl-*` build is used; otherwise, or if the file can't be read,
the glibc one. The musl builds depend on musl libc alone — no `libgcc`, no `gcompat`. What
that means for a build:

- **Built on Alpine** — every backend works. The default cgo build needs a C compiler
  (`apk add build-base`) at build time only, and links the core in. `CGO_ENABLED=0`
  (purego) needs none.
- **Built on a glibc machine, shipped to Alpine** — build with `CGO_ENABLED=0`, or link
  fully statically; an ordinary cgo build links the builder's glibc, which a bare Alpine
  image does not have. With `CGO_ENABLED=0`, Go's linker still writes glibc's loader into
  the binary by default, so on a bare Alpine image
  it fails to start (`not found`, exit 127) before this module is ever reached. Name
  musl's loader instead and it runs:
  `CGO_ENABLED=0 go build -ldflags '-I /lib/ld-musl-x86_64.so.1' ./...` (arm64's loader
  is `/lib/ld-musl-aarch64.so.1`). Installing `gcompat` in the image also works, and the
  musl build is still the one selected.
- **The wasm backend** (`-tags hypercast_wasm`) does not link on musl: wasmtime-go's
  precompiled static library targets glibc.
- A version of this module from before the musl builds were added has no `linux-musl-*`
  directory to embed; there, a musl process gets `ErrNativeUnavailable` naming the
  missing file rather than a failed `dlopen` of the glibc build.

## cgo on darwin/linux, purego everywhere else

The backend is chosen entirely at compile time, by build tag, with the same public API
either way — no code changes for a consumer:

| Build | Backend | File |
| --- | --- | --- |
| darwin/linux with cgo enabled, amd64 or arm64 | cgo, core linked in | `backend_static.go` |
| the same, with `-tags hypercast_dynamic` | cgo, shared library loaded | `backend_cgo.go` |
| Windows; any build with `CGO_ENABLED=0`; every cross-compile | purego | `backend_purego.go` (`//go:build !(cgo && (darwin \|\| linux)) && !hypercast_wasm`) |
| `-tags hypercast_wasm` | wasmtime-go | `backend_wasmtime.go` (`//go:build hypercast_wasm`) |

**cgo wherever it is available**, because a scalar parser pays the crossing on every call
and purego's trampoline allocates on each one — the Benchmarks section has both columns.

**Linked in by default.** `backend_static.go` takes each door's address from a symbol the
linker resolved; `backend_cgo.go` takes it from `dlsym`, the way every cgo build did through
0.3.0, and is kept behind `-tags hypercast_dynamic` for a build that has to pick the core
up at run time rather than at link time. The two share their C shims and cost the same per
call; it is the loading that differs.

**Windows stays on purego unconditionally**, even when `CGO_ENABLED=1`: a cgo build there
needs a MinGW-class C toolchain, and the mainline MinGW-w64 distribution has no arm64
support at all.

**Cross-compiles land on purego automatically.** Go disables cgo by default the moment
`GOOS`/`GOARCH` differ from the host, so `GOOS=linux GOARCH=arm64 go build` from an amd64
machine needs no C cross-compiler and no action from a consumer.

**The caveat cgo-by-default brings:** a *native* darwin/linux build on a machine with no C
compiler at all (a distroless-style build container, macOS without Xcode Command Line
Tools) fails to build, because `CGO_ENABLED` defaults to `1` there whether or not a
compiler is present. `CGO_ENABLED=0 go build ./...` forces the purego backend anywhere.
[HyperUuid's Go README](https://github.com/SkunkWerkx/HyperUuid/tree/master/go#cgo-on-darwinlinux-purego-everywhere-else)
has the longer account of how this split was arrived at.

## WebAssembly (wasmtime-go)

The root README's WebAssembly table lists Go as a **structural** blocker, and that row is
still true: it is about compiling *this module* to wasm, and neither `cgo` nor `purego`
has a wasm target. This section is the inverse direction — the Rust core compiled to
`wasm32-wasip1` and run *inside* an ordinary Go process by
[wasmtime-go](https://github.com/bytecodealliance/wasmtime-go), with no native shared
library dlopen'd at all. Same public API, same suite, third backend:

```shell
go build -tags hypercast_wasm ./...
go test  -tags hypercast_wasm ./...
```

`backend_wasmtime.go` is gated on the `hypercast_wasm` tag and the other two backends
are gated on its absence, so exactly one is ever compiled in. It is opt-in only — never
selected automatically — because it is the right answer to two specific questions and a
worse answer to every other one:

- **A platform this module ships no native build for.** The embedded
  `native/wasm32-wasip1/hypercast.wasm` is one artifact for every OS and architecture
  wasmtime itself runs on; `currentTarget()` and the per-RID shared libraries are not
  consulted.
- **A deployment that must not write an executable to a temp file, and cannot use cgo.** The
  purego backend has to (see `native_extract.go`); this one instantiates the module straight
  from the embedded bytes. Where cgo is available the default build already writes nothing:
  it links the core in.

Two costs, stated plainly:

**It is cgo throughout.** wasmtime-go links wasmtime's precompiled static library through
its C API, so a build with this tag needs a working C toolchain on every platform,
Windows included — which is exactly the story `backend_purego.go` exists to avoid (see
"cgo on darwin/linux, purego everywhere else" above). It is also a `require` in
`go.mod` regardless of tag, because Go has no tag-conditional requirements; it lands in
every consumer's module graph and `go.sum`, and compiles into a binary only with the tag.

**Every call crosses into a wasm guest, serialized under a mutex.** A wasmtime `Store`
is not safe for concurrent use, so one process-wide instance takes a lock per call. A
wasm guest sees only its own linear memory, so nothing is handed over by pointer either:
the input is copied into a grow-only guest buffer, the out-value, fault span and
`NumFormat` live in guest allocations made once at load — all from the module's own
exported `malloc`, never a host-picked offset, because dlmalloc claims the tail of the
initial memory on first use and HyperUuid observed a buffer written there corrupted by the
guest's next allocation — and the verdict is copied back out. The by-value `result` the
doors read is the same one the cgo shims return, so `cast.go` does not know which backend
it got.

Measured on linux-x64 (an Intel Core i9-11900H, go1.27), `go test -bench=BenchmarkCast
-benchmem` with and without the tag, same session:

| Door | cgo | wasmtime-go |
| --- | ---: | ---: |
| `Bool` | 51 ns, 0 allocs | 2.6 µs, 14 allocs |
| `I32` | 73 ns, 0 allocs | 2.9 µs, 16 allocs |
| `F64` | 95 ns, 0 allocs | 2.7 µs, 16 allocs |
| `Uuid` | 71 ns, 0 allocs | 2.5 µs, 14 allocs |
| `Timestamp` | 77 ns, 0 allocs | 2.6 µs, 14 allocs |
| `DateTime` (`1/7/2026 3:04 PM`) | 73 ns, 0 allocs | 2.9 µs, 15 allocs |
| `Span` (ISO) | 80 ns, 0 allocs | 2.6 µs, 14 allocs |

Roughly 35x the native crossing per door, and the allocations are wasmtime-go's own
per-call argument boxing, not this module's. Unlike HyperUuid, there is no batch door here
to amortize that behind — a per-cell workload pays it per cell — which is exactly the shape
round three's chunk layer exists to change. Until then this backend is a portability answer,
not a performance one.

## Why not `strconv` / `time.Parse`?

1. **Verdicts with location** — a closed reason plus the offending span, against
   `strconv.NumError`'s wrapped string.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators and currency symbols, radix prefixes, all five .NET
   `Guid` text forms plus `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other
   binding, held by the shared corpus (the whole suite green on all three backends, full
   corpus replay).

**The honest trade-off, stated as plainly as the wins elsewhere: every Go door but one
loses per-call to Go's stdlib.** Go's parsers are simply excellent (`time.Parse(RFC3339Nano)`
at ~44 ns, `strconv.Atoi` at ~7 ns), and every HyperCast call pays a cgo crossing of about
50 ns. The exception is the messy date-time door, which beats `time.Parse` with a layout by
1.3x because that is the one stdlib path slow enough to absorb the crossing. It no
longer pays a heap allocation on top: 0.1.0's doors passed `&out` and `&fault` into the
foreign call, and any Go pointer handed to cgo escapes to the heap — which the README then
called "a floor for this call shape". It was a floor for *that* shape, not for the ABI.
The C shims now declare the out-value, fault span and format on their own stack and return
the verdict by value as one struct, so no Go pointer crosses at all: **0 B, 0 allocs on
every door** on the success path, and 30–50% off each one. (A failed cast allocates
exactly its `*Fault`; `allocs_test.go` holds both counts as tests.) In Go specifically, this binding still earns its
keep on the vocabulary, the closed error contract, and cross-language agreement — not
per-call speed. The batch/tabular layer (round three) is where the crossing amortizes to
zero.

## Benchmarks

`go test -bench=. -benchmem ./...` for cgo with the core linked in, `-tags
hypercast_dynamic` for cgo loading it, `CGO_ENABLED=0` for purego. Measured on linux-x64
(an Intel Core i9-11900H, go1.27), one session, median of three runs:

| Door | cgo, linked in (the default) | cgo, loading | purego | stdlib |
| --- | ---: | ---: | ---: | ---: |
| `Timestamp` | **77 ns, 0 allocs** | 82 ns, 0 allocs | 466 ns, 5 allocs | 44 ns `time.Parse(RFC3339Nano)` |
| `I32` | **73 ns, 0 allocs** | 74 ns, 0 allocs | 488 ns, 6 allocs | 7 ns `strconv.Atoi` |
| `I32` (grouped) | **83 ns, 0 allocs** | 80 ns, 0 allocs | 496 ns, 6 allocs | — |
| `F64` | **95 ns, 0 allocs** | 95 ns, 0 allocs | 508 ns, 6 allocs | 25 ns `strconv.ParseFloat` |
| `Uuid` | **71 ns, 0 allocs** | 69 ns, 0 allocs | 442 ns, 5 allocs | 27 ns `google/uuid.Parse` |
| `Span` (ISO) | **80 ns, 0 allocs** | 87 ns, 0 allocs | 474 ns, 5 allocs | 61 ns `ParseDuration` (Go dialect — different grammar) |
| `Bool` | **51 ns, 0 allocs** | 53 ns, 0 allocs | 426 ns, 5 allocs | 2 ns `strconv.ParseBool` |
| `TimeOfDay` | **66 ns, 0 allocs** | 61 ns, 0 allocs | 443 ns, 5 allocs | — |
| `DateTime` (`1/7/2026 3:04 PM`) | **73 ns, 0 allocs** | 79 ns, 0 allocs | 499 ns, 6 allocs | 95 ns `time.Parse` w/ layout |
| `DateOnlyOrdered` (`1/7/2026`) | **70 ns, 0 allocs** | 70 ns, 0 allocs | 483 ns, 6 allocs | 56 ns `time.Parse` w/ layout |

Linking the core in and loading it cost the same per call, within the noise of three runs;
what linking changes is the binary and its start-up, not the crossing. purego's allocations
are the trampoline's own argument boxing, not the out-params — its doors fill the same
by-value result through pointers, because that cost is already paid. Separator detection
costs ~13 ns: `1.234.567,89` under `Detect` is 117 ns against 104 ns for the same text under
a declared eurozone format (cgo backend).

cgo's 5-8x per-call win over purego is why it stays the default wherever it's
available; purego's zero-toolchain story is why it carries Windows, `CGO_ENABLED=0`, and
every cross-compile automatically. One caveat inherited with cgo-by-default: a *native*
darwin/linux build on a machine with no C compiler at all (distroless-style container,
macOS without Xcode CLT) now fails to build — `CGO_ENABLED=0 go build ./...` forces the
purego fallback anywhere.

## Verifying build provenance

Go has no package registry to attest — `go get` resolves straight from the `go/vX.Y.Z` git
tag against this repo. The native libraries committed under `go/native/` (staged by
`stage-native-binaries.yml`) each carry their own build-provenance attestation from
`hyper-build-native.yml`, which physically lives in `SkunkWerkx/.github` — so verifying
needs `--signer-repo` alongside `--repo`, or `gh` reports a bare `verifying with issuer
"sigstore.dev"` that reads like a bad signature but is only an identity mismatch:

```sh
gh attestation verify go/native/linux-x64/libhypercast.so \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
```

See [csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance)
for more on why `--signer-repo` is needed for some artifacts here and not others.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
