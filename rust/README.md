# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/hypercast.svg)](https://crates.io/crates/hypercast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**The core itself: allocation-free parsers for scalars from untrusted text — booleans, single
characters, the full integer family, reals, UUIDs, and temporals — where every parse returns a
verdict (the value, or a closed reason plus the offending byte span), never a panic and never
an allocation.**

`str::parse` gives you the type's grammar; untrusted text doesn't speak it. This crate's
doors take what sources actually send — `yes`/`on`/`enabled`, `(1,234)` accounting
negatives, eurozone separators, `0xFF` radix prefixes, `urn:uuid:` prefixes, protobuf
`3.5s` durations — each lenience individually declared by the caller through
`NumFormat`, never guessed. Faults are byte spans into your own input; nothing is
captured or formatted on the error path, which is what makes the allocation-free claim
hold on failures too (asserted by a counting `#[global_allocator]` in
`tests/allocation_free.rs`, not a doc comment).

```rust
use hypercast::{NumFormat, cast_i32, cast_timestamp};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match cast_i32("(1,234)", &NumFormat::INVARIANT) {
        // -1234 — accounting parentheses, declared not guessed
        Ok(value) => println!("got {value}"),
        Err(fault) => println!("{} at byte {}", fault.reason, fault.offset),
    }

    match cast_timestamp(b"2026-01-02T15:04:05.123456789+05:00") {
        // Timestamp { seconds, nanos } — protobuf's dual-integer form, normalized to UTC
        Ok(ts) => println!("timestamp {ts:?}"),
        Err(fault) => println!("{} at byte {}", fault.reason, fault.offset),
    }
    Ok(())
}
```

The crate is simultaneously the engine under every binding in this repo — built as a
`cdylib` (`libhypercast`) with 26 `cast_*` C-ABI exports (the text doors, plus
`decimal_from_f64`, `excel_serial_from_f64`, `excel_time` and `excel_duration`) and
`hypercast_version`, dlopen'd or linked by the
C#/Java/Go/Swift/Ruby/PHP/Python bindings, all held to byte-identical verdicts by the
shared `corpus/*.json` conformance vectors — and an ordinary `rlib` for plain Rust use.
Zero runtime dependencies either way.

## What a culture declares

Separators and a currency symbol are what most text needs, and what `NumFormat::new` and
`with_currency` declare. A culture declares more, and every one of those is a field too —
read *beside* the ASCII default, never instead of it, so a format that declares nothing
reads exactly as the invariant culture and a culture whose minus is U+2212 still reads the
hyphen its users type:

```rust
use hypercast::{CurrencySymbol, NativeDigits, NumFormat, Symbol, cast_f64, cast_i32};

let symbol = |text| Symbol::<8>::new(text).unwrap();

// Estonian: a minus of U+2212, and a hyphen still read.
let et = NumFormat::INVARIANT.with_signs(symbol("\u{2212}"), Symbol::NONE);
cast_i32("\u{2212}5", &et); // Ok(-5)

// A culture whose numbers and currency amounts use different separators (en-DE):
// the currency separators apply once the declared symbol is found in the text.
let en_de = NumFormat::new(',', '.', NumFormat::ALL)
    .with_currency(CurrencySymbol::new("€").unwrap())
    .with_currency_separators(symbol("."), symbol(","));
cast_f64("1.234,5", &en_de); // Ok(1234.5)
cast_f64("€1,234.50", &en_de); // Ok(1234.5)

// Arabic-script separators, digits and percent symbol.
let ar = NumFormat::with_separators(symbol("\u{66B}"), symbol("\u{66C}"), NumFormat::ALL)
    .with_digits(NativeDigits::new('\u{660}').unwrap())
    .with_percent(symbol("\u{66A}"));
cast_f64("\u{661}\u{66C}\u{662}\u{663}\u{664}\u{66B}\u{665}", &ar); // Ok(1234.5)
cast_f64("\u{665}\u{660}\u{66A}", &ar); // Ok(0.5)

// A sign after the digits, the way a culture with an `n-` pattern writes it: a lenience
// of its own, not in ALL, so a bridge sets it from the culture's negative pattern.
let trailing =
    NumFormat { flags: NumFormat::ALL | NumFormat::TRAILING_SIGN, ..NumFormat::INVARIANT };
cast_i32("5 -", &trailing); // Ok(-5)
```

Separators are `Symbol<8>`s — 1 to 8 UTF-8 bytes with no ASCII digit, so a culture
whose group separator is a narrow no-break space or an LTR mark and a comma declares it
whole. The civil date-time door takes the same declaration for the two halves of a 12-hour
clock: `cast_datetime_with(text, order, &DayPeriods)` reads a culture's own designators
after the clock (`3:04 nachm.`) or ahead of it (`오후 3:04`), beside `AM`/`PM`:

```rust
use hypercast::{DateOrder, DayPeriods, Symbol, cast_datetime_with};

let de = DayPeriods::new(Symbol::new("vorm.").unwrap(), Symbol::new("nachm.").unwrap());
cast_datetime_with("1.7.2026 3:04 nachm.", DateOrder::DayMonthYear, &de);
// Ok(CivilDateTime { .. }) — 2026-07-01 15:04, no zone
```

The boolean door is deliberately not on this list: no platform's culture data carries a
boolean lexicon, so nothing could fill one, and `cast_bool` stays English. Every row here
is pinned by `corpus/`, and `docs/roadmap.md` has the sweep that built them.

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
shared library the loading bindings open (`cargo cdylib`, below) are all `#![no_std]`, each
bringing the abort-on-panic handler `std` would otherwise supply — which takes the linux-x64
shared library from 425 KB to 160 KB with the same exports and the same code behind them
(425,048 and 160,488 bytes, `cargo rustc --release --crate-type cdylib` against `cargo
cdylib`, rustc 1.99.1 in `rust:latest`, at commit `0ee2caa` on 2026-10-25; the CI-built
library committed for PHP is 160,512 bytes, the difference being the C compiler's version
string).
The shared library is not one of the manifest's crate types, so cargo never builds it for a
consumer of the crate.

## Excel date serials

`cast_excel_serial` reads a spreadsheet's own date encoding — days since the workbook's
epoch, fraction as time of day — under a caller-declared `ExcelEpoch`, because nothing in
a cell says which system it is:

```rust
use hypercast::{ExcelEpoch, cast_excel_serial};

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
use hypercast::{ExcelEpoch, excel_serial};

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
use hypercast::{decimal_from_f64, i32_from_f64};

decimal_from_f64(2.5).map(|d| d.to_string()); // Ok("2.5")
decimal_from_f64(0.1 + 0.2).map(|d| d.to_string()); // Ok("0.30000000000000004")
i32_from_f64(2.5); // Err(Reason::Malformed), never rounded
```

Above 2^53, a double's shortest decimal integer can differ from its exact binary
integer. Typed integer doors use those shortest decimal digits, just as their text twins
would, rather than switching to binary-integer semantics:

```rust
assert_eq!(hypercast::u64_from_f64(9_223_372_036_854_775_808.0), Ok(9_223_372_036_854_776_000));
```

## One character

`cast_char` reads one Unicode scalar. Text that is exactly one character is taken verbatim,
checked before trimming, so `" "` is a space and `"6"` the digit six; anything longer is one
declared code point: `65`, `U+0041`, `0x41`, `&H41`, or `&#65;` / `&#x41;` (the `;`
required). A surrogate or anything past `U+10FFFF` is `OutOfRange`. It takes no `NumFormat`,
since nothing in the grammar is cultural:

```rust
use hypercast::cast_char;

cast_char("U+00E9"); // Ok('é')
cast_char(" "); // Ok(' '), verbatim rather than Empty
cast_char("U+D800"); // Err: OutOfRange, a surrogate
```

## Optional native-extension features

Three additive cargo features link this same core straight into an interpreter as a real
native extension — one crate, three extra entry points, instead of satellite crates
path-depending back here:

```sh
cargo cdylib                    # the plain shared library C#, Java, PHP and Ruby's Fiddle backend load (no_std)
cargo rustc --release --crate-type cdylib --features python  # the CPython extension module (PyO3: abi3, or abi3t for a free-threaded 3.15+ PYO3_PYTHON)
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
asserted, exactly as in HyperUuid. CI builds and load-checks it on linux-x64, linux-arm64
and osx-arm64 in every release-mode run (weekly and release, not pull requests) so it cannot
bit-rot; no phpunit runs against it, and the Composer package never loads it.

Only one feature is ever enabled per build invocation — each produces a different C entry
point (`PyInit__native`, `Init_hypercast_native`, PHP's module struct) under the same
crate. On macOS the
crate's own `.cargo/config.toml` supplies the `-undefined dynamic_lookup` link flag an
extension module needs (the host runtime's symbols resolve at load time, not link time).

**Local dev trap worth knowing:** a plain `--features` build of any of the three writes the
*same* file as `cargo cdylib`, `target/release/libhypercast.so`, so a `--features python` build (or a `maturin build` in
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
`src/main/resources/native/<rid>/`).

## Interop: building on HyperCast's C ABI

For a crate that links this one as an rlib and exports a C ABI of its own carrying
HyperCast's verdicts — HyperTabular's native layer, for one. The value types
(`Decimal`, `Timestamp`, `Date`, `CivilDateTime`, `Duration`) are `#[repr(C)]` and are the
out-values themselves; the rest is what turns the ABI's codes and format back into them,
the same code the doors and `ffi.rs` use:

- **The `exports` feature**, on by default, is the 26 `cast_*` symbols and
  `hypercast_version` as a symbol. A `#[no_mangle]` item is exported from whatever library
  the crate ends up in, so a crate building its own `cdylib` or `staticlib` takes
  `default-features = false`, naming `std` again if it wants it — otherwise its library
  carries all 26, and its static archive cannot be linked into one program with
  `libhypercast.a` (`multiple definition of cast_bool`). `hypercast_version()` stays a Rust
  function either way.
- **`RawNumFormat`** is `NumFormat` as it crosses the ABI: a size word, then the format
  itself, byte for byte — 92 bytes, 4-aligned, every symbol its UTF-8 bytes then a length
  byte (`NumFormat` is `#[repr(C)]` for exactly this; `abi.rs` has the offset table). The
  core reads a caller's struct in place, so a cast pays for a check of it and no copy.
  `RawNumFormat::from(format)` builds it, `to_le_bytes`/`from_le_bytes` are its exact
  bytes (what Python's and Ruby's `NumFormat.packed` are), and `resolve()` returns the
  `&NumFormat` or `None` for a contract violation — a size the core does not read, or a
  format `NumFormat::check` refuses. `RawDayPeriods` is the same shape around
  `DayPeriods`, 40 bytes, for `cast_datetime`.
- **`from_code`** — a `const fn` on `Reason`, `UnixPrecision`, `ExcelEpoch` and `DateOrder`
  — reads a verdict code or a declared option back, `None` for any code that names none
  (`Reason::from_code(0)`, success, included).
- **The `python-values` feature** exposes `hypercast::python::Values`, the conversions this
  crate's own extension uses (`Values::import`, then `instant`, `date`, `civil`, `time`,
  `duration`, `decimal`, `uuid`), so another PyO3 extension hands out the same `datetime`,
  `decimal.Decimal` and `uuid.UUID` objects `hypercast.cast_*` would. The `python` feature
  builds on it. Its types are PyO3 0.30's, so the extension using it must be on PyO3 0.30
  too (Cargo links one PyO3 per build). `Values` is `Sync` and only read once built, so a
  free-threaded extension can share one across threads.

```toml
[dependencies]
hypercast = { version = "<version>", default-features = false, features = ["std"] }
```

```rust
use hypercast::{NumFormat, RawNumFormat, Reason};

let raw = RawNumFormat::from(NumFormat::new(',', '.', NumFormat::ALL));
assert_eq!(RawNumFormat::from_le_bytes(raw.to_le_bytes()).resolve(), raw.resolve());
assert_eq!(Reason::from_code(3), Some(Reason::OutOfRange));
```

`Decimal`'s fields are public, since it is an out-value itself. Cast results are canonical,
with scale `0..=28`, no trailing fraction zeros and no negative zero. Directly constructed
values can be noncanonical; `Display` preserves every declared scale byte and sign,
without clamping or allocation. Rust's generic `Symbol<N>` accepts at most `min(N, 255)`
UTF-8 bytes because its ABI length is one byte; the 8- and 16-byte culture symbols use
their existing limits and layouts.

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
hypercast = { version = "<version>", features = ["no-panic"] }

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
passes under `wasmtime` on `wasm32-wasip1`, in CI's `test-wasi` job on every pull request:
no clock, no randomness, no dependencies to stub. CI also builds the `wasm32-unknown-emscripten` staticlib the C# binding's
browser-wasm packaging consumes, on every PR — `cargo +<pin> wasm-staticlib`, which leaves
Rust's standard library out of it (the `staticlib` feature supplies the panic handler in its
place), so it can be linked into one Blazor app beside HyperUuid's. It is the one build on a
nightly, the dated one `.github/hyper.json` pins as `wasm.toolchain`, installed with
`rust-src`: `-Zbuild-std` rebuilds core and compiler_builtins for the plain wasm feature set
both .NET 10 and .NET 11 load, without the prebuilt sysroot's legacy exception handling
(`.cargo/config.toml` has the detail).

**In the browser, from Rust.** The crate needs nothing from a consumer on
`wasm32-unknown-unknown`: the core reads no clock and no entropy, so there is no `getrandom`
backend to switch on and no time to hand in — `default-features = false` or not, it builds
for the browser as it is. `rust/browser-test/` depends on it the way a browser consumer does
and runs the boolean, integer, real, decimal, UUID, timestamp, date and duration doors plus
the version export inside a real browser; CI runs it in headless Chrome on every PR
(`wasm-pack test --headless --chrome` there).

The `wasm32-wasip1` static archive that TinyGo and Swift's WebAssembly SDK link is the same
`no_std` build as every other archive, and it imports nothing from WASI: no clock and no
entropy, because the core is pure computation over the bytes it is handed. Its only
undefined symbols are `memcmp`, the 128-bit arithmetic helpers and the stack pointer, which
the consumer's linker (wasi-libc, compiler-rt) supplies.

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
format — ~10 ns, small beside any FFI crossing (about 7 ns in C# and Java, more in Swift,
whose carrier is thinner).

A declared currency symbol costs text that does not carry one about 2 ns: `1,234,567` is
13.3 ns under `NumFormat::INVARIANT` and 15.1 ns under a format declaring `$`. Rejection
costs more than acceptance. `cast_i32` faults `1,234x567` in 37.3 ns, against 3.3 ns for
`str::parse` failing on `1234x567`: the fast lanes read well-formed shapes only, so a fault
falls through to the full engine, which rescans the text to report its reason and span.

Grouping, a currency symbol and accounting parentheses do not send a number through the
full normalize-then-parse engine: a second fast lane (`src/lane.rs`) reads them in one
pass, so a money-shaped value costs a few nanoseconds more than a plain one rather than
several times as much — under a declared `$` and `,`:

| Shape | `cast_i64` | `cast_decimal` | `cast_f64` |
| --- | ---: | ---: | ---: |
| plain (`12345.67`) | 6.7 ns | 11.3 ns | 12.2 ns |
| grouped (`12,345.67`) | 12.8 ns | 21.5 ns | 21.6 ns |
| grouped, with the symbol (`$12,345.67`) | 12.8 ns | 19.0 ns | 20.4 ns |
| the same in parentheses (`($12,345.67)`) | 13.0 ns | 18.7 ns | 21.0 ns |

(The integer door reads the same four shapes of `1234567`.)

**Why this file carries a table instead of a boast:** `cast_uuid` is 12.8 ns against the
`uuid` crate's 11.1 ns, and `cast_timestamp` is the one door ahead of its Rust counterpart
(14.5 ns against the `time` crate's 17.1). Against
in-process Rust parsers these doors trade raw speed for what they return (a verdict with a
span, not a panic or a bare `Option`) and what they accept (five `Guid` text forms, declared
grouping and separators, three duration grammars). The speed story belongs to the *bindings*,
where the competition is culture-machinery parsers rather than `str::parse`. Plain-shaped
input still takes allocation-free fast lanes; only text that actually uses the forgiveness
pays for it.

## Verifying provenance

The published `.crate` verifies from the release tag alone: the release attestation, signed by
this repository's `release.yml` at the tag, names it among every file the release ships, with
the release inventory as its predicate.

```sh
curl -LO https://static.crates.io/crates/hypercast/hypercast-<version>.crate
gh attestation verify hypercast-<version>.crate --repo SkunkWerkx/HyperCast \
  --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

Its build provenance is the second check. The crate is packed by `hyper-pack-crate.yml`, a
reusable workflow that lives in `SkunkWerkx/.github`, and that is the identity Fulcio records
as the signer, so `--repo` alone is not enough there: add `--signer-repo`, or use `--owner` in
place of both.

```sh
gh attestation verify hypercast-<version>.crate \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
# or: gh attestation verify hypercast-<version>.crate --owner SkunkWerkx
```

Get the signer wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which
reads like a bad signature but is only an identity mismatch. The root README's
[Provenance](../README.md#provenance) section has the whole picture: the inventory, which
artifacts are signed from where, and the gates every byte passes before publication.

## Install

```sh
cargo add hypercast
```

Requires Rust 1.88 or later (`rust-version` in the manifest): the parsers use `let` chains,
stable since 1.88, above edition 2024's own 1.85 floor. CI checks the library and the
`no_std` shared library on exactly that toolchain on every PR.

Zero runtime dependencies. `default-features = false` gives the `#![no_std]` rlib described
above. The `cdylib` the loading bindings open is built from this repository with
`cargo cdylib`, and is never part of a consumer's build.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
