# HyperCast — notes for agents

A `no_std`, allocation-free Rust core (`rust/`) of casting "doors" behind a C ABI, called
from seven other languages. `README.md` and `docs/roadmap.md` have the why, each binding's
README has its mechanics, and all CI and release logic lives in the Forge
(`SkunkWerkx/.github`). This file holds what the code alone won't tell you.

## The contract

- A door returns a value or a fault (`Empty` / `Malformed` / `OutOfRange` plus a byte
  span). It never guesses: date order, epoch, Unix precision, separators and culture are
  all declared by the caller, and culture never lives in the core.
- `corpus/*.json` is replayed byte for byte by the Rust suite and every binding (Ruby on
  both backends). A behaviour change is a corpus change, and every binding must agree.
- Fault spans always satisfy `offset + len <= input.len()`, proven over every truncation
  of every corpus input (`rust/tests/fault_span_invariant.rs`); bindings slice with them.

## Proofs to keep green

- `cargo no-panic` proves at link time that no export can panic. That is why
  `rust/src/float.rs` is the crate's own float parser: `core`'s keeps bounds checks the
  optimizer cannot remove. Don't route parsing back through `core`'s. Rationale: the
  `float.rs` header and the Forge README ledger, "A float parser the proof accepts".
- `rust/tests/allocation_free.rs` demands zero allocations.

## Downstream: HyperTabular

HyperTabular links this crate with `default-features = false`, which turns off the
`exports` feature so both libraries can sit in one program (the `exports` comment in
`rust/Cargo.toml`). Changes to the doors, their types or features reach HyperTabular, and
a HyperCast release means bumping HyperCast in every one of HyperTabular's manifests.

## Lists that move together

- Exports: `rust/src/ffi.rs` and every binding's declarations;
  `.github/scripts/check_exports.py` fails on drift. The `RawNumFormat` and
  `RawDayPeriods` layouts are pinned by `offset_of!` asserts in `rust/src/abi.rs` and
  restated in every binding.
- Ruby ABIs: `ABIS` in `ruby/Rakefile`, both `ABIS=(…)` in `release.yml`,
  `ruby_compat_version` in `ci.yml`, and the Forge's `ruby_wasm_abis` default. Fiddle
  covers every Ruby and platform outside them and must never be restricted. Ruby 3.4
  reaches EOL on 2028-03-31: then drop it from those lists and clear
  `ruby_compat_version`.

## Never hand-edit

- `go/staticlib/`, the Swift artifact bundle and `php/src/native/` are CI-built binaries
  committed by `stage-native-binaries.yml`.
- Versions: bump through `prepare-release.yml`. The libraries embed their version.

## Claims are measured

Benchmark figures are dated, machine-specific receipts; losses are printed, and corrections
are published rather than quietly edited. Don't adjust numbers without re-running.

## Toolchains and floors

Each language's primary toolchain is its current release; check endoflife.date rather than
assuming. Exceptions must be commented where pinned: a real platform constraint, or an
older floor shipped beside the primary and dropped at its EOL. Floors match HyperUuid and
HyperTabular and move together.

## Forge coupling and releases

- The Forge is pinned to one exact commit in `ci.yml`, `release.yml` and `RELEASING.md`.
  Repin all of them, and HyperUuid and HyperTabular with them, as one reviewed change. This
  repo conforms to the Forge; it doesn't add opt-outs (`ci.yml` header).
- Registry versions can never be replaced. Release and recovery: `.github/RELEASING.md`
  and the Forge's `docs/release-runbook.md`.

## Local loop

Run `cargo test` and `cargo no-panic` in `rust/`. Build `cargo cdylib` before a binding's
tests. Go, Swift and PHP otherwise test against the committed binaries;
`.github/scripts/local-core.sh` builds them from the checkout. A `--features python` or
`ruby` build overwrites the plain library in `target/release`; run `cargo cdylib` again.

## Working with the owner

Don't open pull requests or push unless asked.
