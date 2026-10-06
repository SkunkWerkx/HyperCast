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
`cdylib` (`libhypercast`) with one `cast_*` C-ABI export per door plus `hypercast_version`, dlopen'd or linked by the
C#/Java/Go/Swift/Ruby/PHP/Python bindings, all held to byte-identical verdicts by the
shared `corpus/*.json` conformance vectors — and an ordinary `rlib` for plain Rust use.
Zero runtime dependencies either way.

## `no_std`

The parsing core touches nothing outside `core` — so `default-features = false` gives a
genuine `#![no_std]` rlib, on every target: your own machine, `wasm32-unknown-unknown`, and
bare metal.

```sh
cargo add hypercast --no-default-features
```

**And no `alloc`, either.** The crate never declares `extern crate alloc`, so there is no
`String`, no `Vec`, no `Box` anywhere in it and nothing for a `#[global_allocator]` to serve.
A bare-metal consumer supplies a `#[panic_handler]` and stops there — no allocator, no
scratch buffer, no hidden heap in a batch path, because there is no batch path. The doors
write into buffers you already own; that is the same property the counting-allocator test
(`tests/allocation_free.rs`) asserts at runtime, stated here as a dependency fact rather than
a benchmark result.

Both halves are guarded in CI (`check-no-std`), not left to convention:
`cargo check --no-default-features --target thumbv7em-none-eabi` compiles the crate for a
real Cortex-M target, a `default-features = false` consumer is built for the host and for
`wasm32-unknown-unknown`, and the job additionally fails if `extern crate alloc` ever appears
in the core. Nothing else in the pipeline would notice any of these regressions — every other
cargo invocation builds the default `std` configuration, where a stray `use std::` or a `Vec`
compiles perfectly cleanly.

The `std` feature is on by default for the crates.io consumer, the tests and the native
extensions. The artifacts this repository ships leave it out: the static libraries and the
shared library every FFI binding loads (`cargo cdylib`, below) are all `#![no_std]`, each
bringing the abort-on-panic handler `std` would otherwise supply — which takes the linux-x64
shared library from 437 KB to 104 KB with the same exports and the same code behind them.
The shared library is not one of the manifest's crate types, so cargo never builds it for a
consumer of the crate.

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
that phantom day ever since. This door rejects it as `OutOfRange` — the same verdict
`cast_date` gives the text `1900-02-29` — so both doors agree that day does not exist. Every serial above 60 is therefore shifted one day against a naive count, which is
precisely the arithmetic hand-rolled conversions get wrong. `ExcelEpoch::Y1904` (legacy
Macintosh workbooks, still selectable today) has no phantom anywhere in it.

A workbook reader holds the number, not its text. `excel_serial` is the same rules for the
`f64` a file stores, returning the zone-less wall clock the cell holds:

```rust
use hypercast::{excel_serial, ExcelEpoch};

let civil = excel_serial(45292.75, ExcelEpoch::Y1900);
// Ok(CivilDateTime { .. }) — 2024-01-01 18:00:00, no zone
civil.map(|wall| wall.assume_utc());
// the Timestamp the text door returns, because the caller said UTC
```

## Numbers a workbook already holds

The same holds for every other door. A numeric cell is a double, and each text door has a
typed twin that reads one: `i32_from_f64` and the other integer widths, `f32_from_f64`,
`bool_from_f64`, `decimal_from_f64`, `unix_from_f64`, and, for serials, `excel_time` (the
fraction as a time of day) and `excel_duration` (a span of days). A double is read as the
shortest decimal that rounds back to it, the digits a spreadsheet writes for it, so no twin
invents precision the file never had:

```rust
use hypercast::{decimal_from_f64, i32_from_f64, Reason};

decimal_from_f64(2.5).map(|d| d.to_string());       // Ok("2.5")
decimal_from_f64(0.1 + 0.2).map(|d| d.to_string()); // Ok("0.30000000000000004")
i32_from_f64(2.5);                                  // Err(Reason::Malformed), never rounded
```

## Optional native-extension features

Three additive cargo features link this same core straight into an interpreter as a real
native extension — one crate, three extra entry points, instead of satellite crates
path-depending back here:

```sh
cargo cdylib                    # the plain shared library every FFI binding uses (no_std)
cargo rustc --release --crate-type cdylib --features python  # the CPython extension module (PyO3, abi3-py311)
cargo ruby-ext                  # the Ruby extension (Magnus), in target/ruby/release/ (ruby/'s `rake native:dev` runs this and stages the result)
cargo php-ext                   # the Zend extension (ext-php-rs) — benchmark spike only, in target/php/release/
```

`cargo cdylib` is an alias in `.cargo/config.toml` for `cargo rustc --release --crate-type
cdylib` with the `cdylib` feature in place of `std` and panics set to abort. The manifest
declares only the rlib, so a plain `cargo build` produces no shared library; the crate type
is named per invocation, which is what keeps it out of every consumer's build. The
extensions are built with `std` and unwinding, so a panic in one still reaches the host as
an exception — which is why the python one is spelled out rather than run through the
alias.

The `php` one is not a shipped backend. PHP's ext-ffi crossing measured ~105 ns — already
extension-class, which is why Python and Ruby got a native backend and PHP didn't — and
this spike exists to keep that reasoning checkable against real numbers rather than
asserted, exactly as in HyperUuid. CI builds and load-checks it on every darwin/linux leg
so it cannot bit-rot; no phpunit runs against it, and the Composer package never loads it.

Only one feature is ever enabled per build invocation — each produces a different C entry
point (`PyInit__native`, `Init_hypercast_native`, PHP's module struct) under the same
crate. On macOS the
crate's own `.cargo/config.toml` supplies the `-undefined dynamic_lookup` link flag an
extension module needs (the host runtime's symbols resolve at load time, not link time).

**Local dev trap worth knowing:** all three builds write the *same* file —
`target/release/libhypercast.so` — so a `--features python` build (or a `maturin build` in
`python/`, which is one) silently replaces the plain cdylib that every other binding's dev
loop loads. The extension build still exports every `cast_*` symbol, but it also carries
~95 undefined `Py*` symbols that only resolve inside a CPython process, so the next
`./gradlew test` or `dotnet test` fails at native load with something unhelpful about a
missing symbol. Nothing is broken; a plain `cargo cdylib` puts it back. Locally,
the `cargo ruby-ext` and `cargo php-ext` aliases in `.cargo/config.toml` avoid it by building into
`target/ruby/` and `target/php/`, and `python/.cargo/config.toml` does the same for maturin
(`python/target/`). CI hits
exactly this ordering — the forge's single per-platform job builds the PyO3 extension
before it tests C# and Java — which is why both bindings' dev-loop staging yields whenever
CI has already placed the library explicitly (`runtimes/<rid>/native/`,
`src/main/resources/native/<rid>/`); the first collapsed-job run failed every Linux leg on
`undefined symbol: PyExc_SystemError` before that gate existed.

## Proven panic-free, and proven right

Every C export is checked at link time: the `no-panic` feature wraps each one in dtolnay's
[`#[no_panic]`](https://docs.rs/no-panic), and `cargo no-panic` (an alias in
`.cargo/config.toml`) links a release build that fails, naming the export, if the optimizer
left any panic path in it. CI runs it on every PR.

To run the same proof from your own crate, turn on fat LTO in its release profile. Cargo
ignores a dependency's profile, so a consumer's stock `--release` build compiles this crate
separately, leaves the panic paths across that boundary in, and fails the link for most of
the exports; thin LTO and `codegen-units = 1` are not enough:

```toml
[dependencies]
hypercast = { version = "0.6", features = ["no-panic"] }

[profile.release]
lto = true
```

no-panic reports a surviving panic path as an undefined symbol. An executable fails to link
on one, but a Linux shared library is allowed undefined symbols, so a `cdylib` passes
silently unless the link forbids them, which is what this repository's own `cargo no-panic`
does:

```sh
RUSTFLAGS="-C link-arg=-Wl,--no-undefined" cargo build --release
```

Panic-freedom says nothing about whether a door reads the right value, so
`tests/round_trip.rs` checks that too: values format to text and cast back to themselves,
and every one-character edit of a valid string (replaced, removed or inserted, multi-byte
UTF-8 included) casts exactly when a deliberately naive reference parser says it should,
to the same value — about 34,000 strings across the UUID, boolean, integer (each through
all eight widths) and strict date doors — and 10,000 sampled floats, extremes and
subnormals included, survive Rust's own shortest round-trip formatting through
`cast_f32`/`cast_f64` to the same bits. The doors
whose grammar *is* the lenience (grouping, currency, separator detection, the separated
temporal forms, durations) are pinned by the shared corpus instead.

## WebAssembly

The full test suite — unit tests, the allocation proof, and every corpus replay —
passes under `wasmtime` on `wasm32-wasip1`: no clock, no randomness, no dependencies to
stub. CI also builds the `wasm32-unknown-emscripten` staticlib the C# binding's
browser-wasm packaging consumes, on every PR — `cargo wasm-staticlib`, which leaves Rust's
standard library out of it (the `staticlib` feature supplies the panic handler in its
place), so it can be linked into one Blazor app beside HyperUuid's.

**In the browser, from Rust.** The crate needs nothing from a consumer on
`wasm32-unknown-unknown`: the core reads no clock and no entropy, so there is no `getrandom`
backend to switch on and no time to hand in — `default-features = false` or not, it builds
for the browser as it is. `rust/browser-test/` depends on it the way a browser consumer does
and runs the boolean, integer, real, decimal, UUID, timestamp, date and duration doors plus
the version export inside a real browser; CI runs it in headless Chrome on every PR
(`wasm-pack test --headless --chrome` there).

One wasm build of this crate is not left to the consumer, because the Java binding in this
repo ships it: the `cdylib` for `wasm32-wasip1`, built from inside this directory so that
`.cargo/config.toml` applies —

```sh
cargo wasm-module
# rust/target/wasm32-wasip1/release/hypercast.wasm
```

Unlike the native library, the module is built with `std`: its allocator exports come from
wasi-libc by way of `std`'s allocator, and a `no_std` module does not link wasi-libc at all.

That config adds two linker flags for this target only, `--export=malloc` and
`--export=free`, so the module's exports are the `cast_*` functions and `hypercast_version` from `ffi.rs`
plus wasi-libc's allocator. A wasm host cannot hand this library a pointer into its own
memory, so its embedder — GraalWasm inside the Java binding — copies the input text into a
guest buffer, points the door at guest buffers for the out-value, the fault span and the
`NumFormat`, and reads the result back out of the exported `memory`. The exported allocator is what makes that safe: dlmalloc claims
the tail of the initial linear memory on its first use, so a host-chosen offset past the
data segments is not free, and HyperUuid observed a buffer written there corrupted by the
next allocation. The module imports four `wasi_snapshot_preview1` functions — wasi-libc's
`environ_get`, `environ_sizes_get`, `fd_write` and `proc_exit`, from its startup and panic
paths — and nothing else: no clock and no entropy, because the core is pure computation
over the bytes it is handed. `ffi.rs` itself is untouched by any of this; on every native
target the C ABI is still exactly the same exports.

## Benchmarks

`cargo bench` — Criterion, `rust/benches/cast_benchmarks.rs`. Measured on linux-x64 (an
Intel Core i9-11900H) against Rust's own best-in-class. In-process, with no FFI boundary in the way, these are
the doors' raw cost:

| Door | HyperCast | Closest Rust parser |
| --- | ---: | --- |
| `cast_date_ordered` (`1/7/2026`) | 8.5 ns | no stdlib parser takes it |
| `cast_uuid` (D format) | 12.8 ns | 11.1 ns — `uuid` crate |
| `cast_i64` | 8.7 ns | 7.1 ns — `str::parse` |
| `cast_f64` | 13.0 ns | 10.5 ns — `str::parse` |
| `cast_decimal` (`12345.6789`) | 13.6 ns | no stdlib parser — an exact `u96`+scale, never rounded |
| `cast_decimal` (`$12,345.67`, declared `$`) | 18.1 ns | the currency symbol and grouping, read in one pass: what a culture-shaped feed actually costs |
| `cast_datetime` (`1/7/2026 3:04 PM`) | 13.9 ns | no stdlib parser takes it |
| `cast_timestamp` (RFC 3339) | 14.5 ns | 17.1 ns — `time` crate |
| `cast_datetime` (ISO) | 13.3 ns | — |
| `cast_duration` (ISO 8601) | 23.6 ns | no stdlib parser takes it |

Separator detection costs one extra scan and nothing more: `1.234.567,89` under
`NumFormat::DETECT` is 29.0 ns against 19.2 ns for the same text under a declared eurozone
format — ~10 ns, and invisible behind any FFI boundary (the Java and Swift bindings measure
detection as free at their crossing).

Grouping, a currency symbol and accounting parentheses each used to send a number through
the full normalize-then-parse engine. A second fast lane (`src/lane.rs`) reads them in one
pass, so a money-shaped value costs a few nanoseconds more than a plain one rather than
several times as much — under a declared `$` and `,`:

| Shape | `cast_i64` | `cast_decimal` | `cast_f64` |
| --- | ---: | ---: | ---: |
| plain (`12345.67`) | 6.7 ns | 11.3 ns | 12.2 ns |
| grouped (`12,345.67`) | 12.8 ns | 21.5 ns | 21.6 ns |
| grouped, with the symbol (`$12,345.67`) | 12.8 ns | 19.0 ns | 20.4 ns |
| the same in parentheses (`($12,345.67)`) | 13.0 ns | 18.7 ns | 21.0 ns |

(The integer door reads the same four shapes of `1234567`.)

**Correction, and the reason this file carries a table instead of a boast:** an earlier
version of this README claimed `cast_uuid` beat the `uuid` crate (15.4 vs 17.4 ns). It
doesn't anymore — `uuid` 1.26 got materially faster, and our SWAR decode (15.9 → 12.8 ns)
narrowed the gap without closing it. `cast_timestamp` is the one door ahead of its Rust
counterpart, and only since its field readers were inlined (26.6 → 14.5 ns). Against
in-process Rust parsers these doors trade raw speed for what they return (a verdict with a
span, not a panic or a bare `Option`) and what they accept (five `Guid` text forms, declared
grouping and separators, three duration grammars). The speed story belongs to the *bindings*,
where the competition is culture-machinery parsers rather than `str::parse`. Plain-shaped
input still takes allocation-free fast lanes; only text that actually uses the forgiveness
pays for it.

## Verifying provenance

The published `.crate` carries a GitHub build-provenance attestation, but not one signed by
this repo directly — `release.yml`'s `pack-crates` job hands off to a reusable workflow
(`hyper-publish-crate.yml`) that physically lives in `SkunkWerkx/.github`, and that's the
identity Fulcio records as the signer. `--repo` alone isn't enough; add `--signer-repo`,
or use `--owner` in place of both:

```sh
curl -LO https://static.crates.io/crates/hypercast/hypercast-X.Y.Z.crate
gh attestation verify hypercast-X.Y.Z.crate \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
# or: gh attestation verify hypercast-X.Y.Z.crate --owner SkunkWerkx
```

The crate is packaged and attested *before* `cargo publish` runs, so an attestation failure
stops the release while it is still reversible — a crates.io version can be yanked but never
deleted or reused. One attestation covers the bytes a consumer downloads: a `.crate` is
byte-identical wherever `cargo package --locked` produces it, and cargo verifies every
download against the index checksum, so crates.io cannot rewrite it the way nuget.org
rewrites a `.nupkg`.

Get the signer-repo wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`,
which reads like a bad signature but is only an identity mismatch — see
[csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance) for the
full breakdown of which artifacts in this project are signed from which repo and why.

## Install

```sh
cargo add hypercast
```

Requires Rust 1.88 or later (`rust-version` in the manifest): the parsers use `let` chains,
stable since 1.88, above edition 2024's own 1.85 floor. CI builds the library and the
`no_std` shared library on exactly that toolchain on every PR.

Zero runtime dependencies. `default-features = false` gives the `#![no_std]` rlib described
above. The `cdylib` every other binding loads is built from this repository with
`cargo cdylib`, and is never part of a consumer's build.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
