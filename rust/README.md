# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/hypercast.svg)](https://crates.io/crates/hypercast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**The core itself: allocation-free parsers for scalars from untrusted text — booleans, the
full integer family, reals, UUIDs, and temporals — where every parse returns a verdict (the
value, or a closed reason plus the offending byte span), never a panic and never an
allocation.**

`str::parse` gives you the type's grammar; untrusted text doesn't speak it. This crate's
doors take what sources actually send — `yes`/`on`/`enabled`, `(1,234)` accounting
negatives, eurozone separators, `0xFF` radix prefixes, `urn:uuid:` prefixes, protobuf
`3.5s` durations — each lenience individually declared by the caller through
`NumFormat`, never guessed. Faults are byte spans into your own input; nothing is
captured or formatted on the error path, which is what makes the allocation-free claim
hold on failures too (asserted by a counting `#[global_allocator]` in
`tests/allocation_free.rs`, not a doc comment).

```rust
use hypercast::{cast_i32, cast_timestamp, NumFormat};

let verdict = cast_i32("(1,234)", &NumFormat::INVARIANT);
// Ok(-1234) — accounting parentheses, declared not guessed

let ts = cast_timestamp(b"2026-01-02T15:04:05.123456789+05:00");
// Ok(Timestamp { seconds, nanos }) — protobuf's dual-integer form, normalized to UTC
```

The crate is simultaneously the engine under every binding in this repo — built as a
`cdylib` (`libhypercast`) with 20 `cast_*` C-ABI exports, dlopen'd or linked by the
C#/Java/Go/Swift/Ruby/PHP/Python bindings, all held to byte-identical verdicts by the
shared `corpus/*.json` conformance vectors — and an ordinary `rlib` for plain Rust use.
Zero runtime dependencies either way.

## `no_std`

The parsing core touches nothing outside `core` — no `String`, no `Vec`, no allocator at
all — so `default-features = false` gives a genuine `#![no_std]` rlib:

```toml
hypercast = { version = "...", default-features = false }
```

Proven against a real bare-metal target rather than asserted:
`cargo check --no-default-features --target thumbv7em-none-eabi` builds clean.

The `std` feature is on by default and stays that way for the shared-library build, because
a `cdylib` is a final linked artifact and needs a `#[panic_handler]` that only `std`
supplies — declaring `#![no_std]` unconditionally fails the release build outright. A
bare-metal consumer brings its own panic handler, as such a consumer must anyway.

## Excel date serials

`cast_excel_serial` reads a spreadsheet's own date encoding — days since the workbook's
epoch, fraction as time of day — under a caller-declared `ExcelEpoch`, because nothing in
a cell says which system it is:

```rust
use hypercast::{cast_excel_serial, ExcelEpoch};

cast_excel_serial("45292.75", ExcelEpoch::Y1900);
// Ok(Timestamp { .. }) — 2024-01-01T18:00:00Z
```

The 1900 system contains a **February 29th that never existed**: Lotus 1-2-3 wrongly treated
1900 as a leap year, Excel copied the bug for file compatibility, and serial `60` has named
that phantom day ever since. This door rejects it as `Malformed` — the same verdict
`cast_date` already gives the text `1900-02-29` — so both doors agree that day is not a
date. Every serial above 60 is therefore shifted one day against a naive count, which is
precisely the arithmetic hand-rolled conversions get wrong. `ExcelEpoch::Y1904` (legacy
Macintosh workbooks, still selectable today) has no phantom anywhere in it.

## Optional native-extension features

Two additive cargo features link this same core straight into an interpreter as a real
native extension — one crate, two extra entry points, instead of satellite crates
path-depending back here:

```sh
cargo build --release                    # the plain cdylib + rlib every FFI binding uses
cargo build --release --features python  # the CPython extension module (PyO3, abi3-py310)
cargo build --release --features ruby    # the Ruby extension (Magnus)
```

Only one feature is ever enabled per build invocation — each produces a different C entry
point (`PyInit__native`, `Init_hypercast_native`) under the same crate. On macOS the
crate's own `.cargo/config.toml` supplies the `-undefined dynamic_lookup` link flag an
extension module needs (the host runtime's symbols resolve at load time, not link time).

**Local dev trap worth knowing:** all three builds write the *same* file —
`target/release/libhypercast.so` — so a `--features python` build (or a `maturin build` in
`python/`, which is one) silently replaces the plain cdylib that every other binding's dev
loop loads. The extension build still exports all 19 `cast_*` symbols, but it also carries
~95 undefined `Py*` symbols that only resolve inside a CPython process, so the next
`./gradlew test` or `dotnet test` fails at native load with something unhelpful about a
missing symbol. Nothing is broken; a plain `cargo build --release` puts it back. CI never
hits this — each leg builds in its own job.

## WebAssembly

The full test suite — unit tests, the allocation proof, and all twelve corpus replays —
passes under `wasmtime` on `wasm32-wasip1`: no clock, no randomness, no dependencies to
stub. CI also builds the `wasm32-unknown-emscripten` staticlib the C# binding's
browser-wasm packaging consumes, on every PR.

## Benchmarks

`cargo bench` — Criterion, `rust/benches/cast_benchmarks.rs`. Measured on linux-arm64
against Rust's own best-in-class. In-process, with no FFI boundary in the way, these are
the doors' raw cost:

| Door | HyperCast | Closest Rust parser |
| --- | ---: | --- |
| `cast_date_ordered` (`1/7/2026`) | 17.8 ns | no stdlib parser takes it |
| `cast_uuid` (D format) | 15.8 ns | 11.8 ns — `uuid` crate |
| `cast_i64` | 15.5 ns | 9.7 ns — `str::parse` |
| `cast_f64` | 26.6 ns | 14.6 ns — `str::parse` |
| `cast_datetime` (`1/7/2026 3:04 PM`) | 30.1 ns | no stdlib parser takes it |
| `cast_timestamp` (RFC 3339) | 30.3 ns | 21.9 ns — `time` crate |
| `cast_datetime` (ISO) | 34.6 ns | — |
| `cast_duration` (ISO 8601) | 42.1 ns | no stdlib parser takes it |

Separator detection costs one extra scan and nothing more: `1.234.567,89` under
`NumFormat::DETECT` is 71.1 ns against 59.9 ns for the same text under a declared eurozone
format — ~11 ns, and invisible behind any FFI boundary (the Java and Swift bindings measure
detection as free at their crossing).

**Correction, and the reason this file carries a table instead of a boast:** an earlier
version of this README claimed `cast_uuid` beat the `uuid` crate (15.4 vs 17.4 ns). It
doesn't anymore — our number is unchanged, and `uuid` 1.26 got materially faster. Against
in-process Rust parsers these doors trade raw speed for what they return (a verdict with a
span, not a panic or a bare `Option`) and what they accept (five `Guid` text forms, declared
grouping and separators, three duration grammars). The speed story belongs to the *bindings*,
where the competition is culture-machinery parsers rather than `str::parse`. Plain-shaped
input still takes allocation-free fast lanes; only text that actually uses the forgiveness
pays for it.

## Install

```sh
cargo add hypercast
```

Zero runtime dependencies. `default-features = false` gives the `#![no_std]` rlib described
above; the default build additionally produces the `cdylib` every other binding loads.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.
