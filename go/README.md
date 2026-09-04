# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Go Reference](https://pkg.go.dev/badge/github.com/SkunkWerkx/HyperCast/go.svg)](https://pkg.go.dev/github.com/SkunkWerkx/HyperCast/go)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**Go's own union idiom — `(value, *Fault)` — carrying the verdict of every cast: the value,
or a closed reason plus the exact byte span that offended. `*Fault` implements `error` for
composition, but the doors never panic on input; a panic here means a caller bug, never
data.**

Allocation-lean scalar casts — booleans, the full integer family, reals, exact decimals,
UUIDs, temporals — calling directly into the native `libhypercast` Rust core. Two native backends, chosen
automatically by build tag, same public API either way: real cgo on darwin/linux
(`backend_cgo.go`) — 3.5-4.8x faster per call, see Benchmarks — and
[purego](https://github.com/ebitengine/purego) (`backend_purego.go`) — dlopen/dlsym plus
per-arch call trampolines, no cgo and no C compiler required — everywhere else, including
Windows unconditionally and any darwin/linux build with `CGO_ENABLED=0` (which, per Go's
own defaults, includes every cross-compile). Bundles a native build for every supported
platform via `go:embed` and picks the right one at runtime. A third backend, opt-in behind
`-tags hypercast_wasm`, runs the same core as a WebAssembly module inside the process
through wasmtime-go instead of dlopen'ing anything — see
[WebAssembly (wasmtime-go)](#webassembly-wasmtime-go).

```go
import "github.com/SkunkWerkx/HyperCast/go"

value, fault := hypercast.I32("(1,234)", hypercast.Invariant)
if fault != nil {
    log.Printf("%s at byte %d", fault.Reason, fault.Offset)
}
// value == -1234, accounting negative

ts, fault := hypercast.Timestamp("2026-01-02T15:04:05.123456789+05:00")
// a UTC time.Time at full nanosecond fidelity
```

Doors are generic over `string | []byte` — both cross zero-copy (the core only reads).
`Uuid` returns [`google/uuid`](https://pkg.go.dev/github.com/google/uuid)'s `uuid.UUID`
(RFC 9562 order is exactly its own layout). Go-flavored fidelity, stated honestly both
ways: `time.Time` carries full nanoseconds across the whole 0001–9999 window and
time-of-day comes back nanosecond-exact, but `time.Duration`'s int64-nanosecond ceiling
(±292 years) sits far below the core's ±10,000-year duration window, so `Span` returns
the protobuf pair (`Duration{Seconds, Nanos}`) with a checked `AsDuration()` converter
rather than silently wrapping.

## Doors

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

Plus two entry points that are not doors:

- `Available()` — `true` when the native library (or, under `-tags hypercast_wasm`, the
  wasm module) loaded and exports the ABI this binding was built against: every symbol
  resolved and `hypercast_version` answered. Probed once and cached; a `false` is permanent
  for the process. It is the one call that never panics on a load failure — a consumer
  keeping a fallback for a platform this module does not cover gates on it instead of
  recovering around its first cast.
- `NativeVersion()` — `"major.minor.patch"` as the loaded core reports it, so a mismatch
  against the version this module was built for can be named before the first cast. Panics
  if the library did not load, like every door; `Available()` is the safe probe.

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

## Why not `strconv` / `time.Parse`?

1. **Verdicts with location** — a closed reason plus the offending span, against
   `strconv.NumError`'s wrapped string.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators and currency symbols, radix prefixes, all five .NET
   `Guid` text forms plus `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other
   binding, held by the shared corpus (the whole suite green on all three backends, full
   thirteen-file corpus replay).

**The honest trade-off, stated as plainly as the wins elsewhere: every Go door loses
per-call to Go's stdlib.** Go's parsers are simply excellent (`time.Parse(RFC3339Nano)` at
~67 ns, `strconv.Atoi` at ~11 ns), and every HyperCast call pays a cgo crossing. It no
longer pays a heap allocation on top: 0.1.0's doors passed `&out` and `&fault` into the
foreign call, and any Go pointer handed to cgo escapes to the heap — which the README then
called "a floor for this call shape". It was a floor for *that* shape, not for the ABI.
The C shims now declare the out-value, fault span and format on their own stack and return
the verdict by value as one struct, so no Go pointer crosses at all: **0 B, 0 allocs on
every door**, and 30–50% off each one. In Go specifically, this binding still earns its
keep on the vocabulary, the closed error contract, and cross-language agreement — not
per-call speed. The batch/tabular layer (round three) is where the crossing amortizes to
zero.

## Benchmarks

`go test -bench=. -benchmem ./...` for cgo, the same with `CGO_ENABLED=0` for purego.
Measured on the same linux-arm64 machine, same session, median of three runs — 0.1.0's
pointer-passing doors against 0.2.0's by-value shims:

| Door | cgo 0.1.0 | cgo 0.2.0 | purego | stdlib |
| --- | ---: | ---: | ---: | ---: |
| `Timestamp` | 174 ns, 2 allocs | **112 ns, 0 allocs** | 596 ns, 5 allocs | 67 ns `time.Parse(RFC3339Nano)` |
| `I32` | 172 ns, 3 allocs | **87 ns, 0 allocs** | — | 11 ns `strconv.Atoi` |
| `I32` (grouped) | 224 ns, 3 allocs | **125 ns, 0 allocs** | 652 ns, 6 allocs | — |
| `F64` | 218 ns, 3 allocs | **106 ns, 0 allocs** | — | 39 ns `strconv.ParseFloat` |
| `Uuid` | 155 ns, 2 allocs | **99 ns, 0 allocs** | 590 ns, 5 allocs | 35 ns `google/uuid.Parse` |
| `Span` (ISO) | 172 ns, 2 allocs | **120 ns, 0 allocs** | — | 71 ns `ParseDuration` (Go dialect — different grammar) |
| `Bool` | 111 ns, 1 alloc | **78 ns, 0 allocs** | 560 ns, 5 allocs | 4 ns `strconv.ParseBool` |
| `TimeOfDay` | 115 ns, 1 alloc | **81 ns, 0 allocs** | — | — |
| `DateTime` (`1/7/2026 3:04 PM`) | 173 ns, 2 allocs | **122 ns, 0 allocs** | — | 135 ns `time.Parse` w/ layout |
| `DateOnlyOrdered` (`1/7/2026`) | 129 ns, 1 alloc | **95 ns, 0 allocs** | — | 78 ns `time.Parse` w/ layout |

The purego column barely moves, as expected: its allocations are the trampoline's own
argument boxing, not the out-params, and its doors fill the same by-value result through
pointers because that cost is already paid. Separator detection costs ~9 ns: `1.234.567,89`
under `Detect` is 172 ns against 164 ns for the same text under a declared eurozone format
(cgo backend).

cgo's 4.5-7x per-call win over purego is why it stays the default wherever it's
available; purego's zero-toolchain story is why it carries Windows, `CGO_ENABLED=0`, and
every cross-compile automatically. One caveat inherited with cgo-by-default: a *native*
darwin/linux build on a machine with no C compiler at all (distroless-style container,
macOS without Xcode CLT) now fails to build — `CGO_ENABLED=0 go build ./...` forces the
purego fallback anywhere.

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
- **A deployment that must not write an executable to a temp file.** The native backends
  have to (see `native_extract.go`); this one instantiates the module straight from the
  embedded bytes.

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

Measured on linux-arm64 (WSL2, go1.27), `go test -bench=BenchmarkCast -benchmem` with and
without the tag, same session:

| Door | cgo | wasmtime-go |
| --- | ---: | ---: |
| `Bool` | 82 ns, 0 allocs | 3.4 µs, 14 allocs |
| `I32` | 84 ns, 0 allocs | 3.4 µs, 16 allocs |
| `F64` | 105 ns, 0 allocs | 3.7 µs, 16 allocs |
| `Uuid` | 99 ns, 0 allocs | 3.5 µs, 14 allocs |
| `Timestamp` | 101 ns, 0 allocs | 3.8 µs, 14 allocs |
| `DateTime` (`1/7/2026 3:04 PM`) | 110 ns, 0 allocs | 3.4 µs, 15 allocs |
| `Span` (ISO) | 119 ns, 0 allocs | 3.6 µs, 14 allocs |

Roughly 35x the native crossing per door, and the allocations are wasmtime-go's own
per-call argument boxing, not this module's. Unlike HyperUuid, there is no batch door here
to amortize that behind — a per-cell workload pays it per cell — which is exactly the shape
round three's chunk layer exists to change. Until then this backend is a portability answer,
not a performance one.

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

## Install

```sh
go get github.com/SkunkWerkx/HyperCast/go
```

Go modules have no separate registry — `go get` resolves straight from a git tag, and because
this module lives in a monorepo subdirectory its tags are prefixed (`go/vX.Y.Z`). The native
libraries under `native/{rid}/` are committed to git and kept fresh by
`stage-native-binaries.yml`: a `go:embed` consumer has no packing step, so whatever is
literally in the tree at the resolved tag is what gets embedded (see `native/README.md`).

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.
