# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Go Reference](https://pkg.go.dev/badge/github.com/SkunkWerkx/HyperCast/go.svg)](https://pkg.go.dev/github.com/SkunkWerkx/HyperCast/go)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**Go's own union idiom — `(value, *Fault)` — carrying the verdict of every cast: the value,
or a closed reason plus the exact byte span that offended. `*Fault` implements `error` for
composition, but the doors never panic on input; a panic here means a caller bug, never
data.**

Allocation-lean scalar casts — booleans, the full integer family, reals, UUIDs, temporals —
calling directly into the native `libhypercast` Rust core. Two backends, chosen
automatically by build tag, same public API either way: real cgo on darwin/linux
(`backend_cgo.go`) — 3.5-4.8x faster per call, see Benchmarks — and
[purego](https://github.com/ebitengine/purego) (`backend_purego.go`) — dlopen/dlsym plus
per-arch call trampolines, no cgo and no C compiler required — everywhere else, including
Windows unconditionally and any darwin/linux build with `CGO_ENABLED=0` (which, per Go's
own defaults, includes every cross-compile). Bundles a native build for every supported
platform via `go:embed` and picks the right one at runtime.

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

## Why not `strconv` / `time.Parse`?

1. **Verdicts with location** — a closed reason plus the offending span, against
   `strconv.NumError`'s wrapped string.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, radix prefixes, all five .NET `Guid` text forms plus
   `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other
   binding, held by the shared corpus (22 tests green on both backends, full twelve-file
   corpus replay).

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
