# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)

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
   binding, held by the shared corpus (17 tests green on both backends, full nine-file
   corpus replay).

**The honest trade-off, stated as plainly as the wins elsewhere: every Go door loses
per-call to Go's stdlib.** Go's parsers are simply excellent (`time.Parse(RFC3339Nano)` at
~67 ns, `strconv.Atoi` at ~11 ns), and every HyperCast call pays a cgo crossing plus a
structural heap allocation (any pointer crossing into opaque foreign code is excluded from
escape analysis — a floor for this call shape, not an FFI-library choice). In Go
specifically, this binding earns its keep on the vocabulary, the closed error contract,
and cross-language agreement — not per-call speed. The batch/tabular layer (round three)
is where the crossing amortizes to zero.

## Benchmarks

`go test -bench=. -benchmem ./...` for cgo, the same with `CGO_ENABLED=0` for purego.
Measured on the same linux-arm64 machine, same run:

| Door | cgo | purego | stdlib |
| --- | ---: | ---: | ---: |
| `Timestamp` | 175 ns, 2 allocs | 619 ns, 6 allocs | 67 ns `time.Parse(RFC3339Nano)` |
| `I32` | 177 ns, 3 allocs | 613 ns, 7 allocs | 11 ns `strconv.Atoi` |
| `F64` | 206 ns, 3 allocs | 617 ns, 7 allocs | 39 ns `strconv.ParseFloat` |
| `Uuid` | 148 ns, 2 allocs | 599 ns, 6 allocs | 35 ns `google/uuid.Parse` |
| `Span` (ISO) | 172 ns, 2 allocs | 623 ns, 6 allocs | 71 ns `ParseDuration` (Go dialect — different grammar) |
| `Bool` | 115 ns, 1 alloc | 553 ns, 5 allocs | 4 ns `strconv.ParseBool` |

cgo's 3.5-4.8x per-call win over purego is why it stays the default wherever it's
available; purego's zero-toolchain story is why it carries Windows, `CGO_ENABLED=0`, and
every cross-compile automatically. One caveat inherited with cgo-by-default: a *native*
darwin/linux build on a machine with no C compiler at all (distroless-style container,
macOS without Xcode CLT) now fails to build — `CGO_ENABLED=0 go build ./...` forces the
purego fallback anywhere.

## Install

```sh
go get github.com/SkunkWerkx/HyperCast/go
```

Go modules have no separate registry — `go get` resolves straight from a git tag (this
module lives in a monorepo subdirectory, so its tags are prefixed: `go/v0.1.0`). No tags
exist yet; the native libraries under `native/{rid}/` are committed to git by
`stage-native-binaries.yml` (a `go:embed` consumer has no packing step — see
`native/README.md`), and the first `go/v*` tag is the publish. Until then: clone the repo,
`cargo build --release` in `rust/`, copy the library into `native/{rid}/`, and
`go test ./...`.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.
