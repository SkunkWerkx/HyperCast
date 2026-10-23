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

- The Forge is called at master, deliberately unpinned: a Forge merge reaches every consumer
  at once, and breakage it causes is fixed forward. `release.yml` resolves master to one
  commit per run (`find-ci-run`'s `forge_sha`), because the package inventory records it and
  recovery requires it unchanged. Don't merge Forge changes while a release is in flight.
- Third-party actions are pinned to a commit with the version in a trailing comment, and
  `.github/dependabot.yml` moves them with every other ecosystem in one monthly pull request.
  It ignores the Forge and `dtolnay/rust-toolchain` (branch heads, refreshed by hand).
- This repo conforms to the Forge; it doesn't add opt-outs (`ci.yml` header).
- Registry versions can never be replaced. Release and recovery: `.github/RELEASING.md`
  and the Forge's `docs/release-runbook.md`.

## Local loop

Run `cargo test` and `cargo no-panic` in `rust/`. Build `cargo cdylib` before a binding's
tests. Go, Swift and PHP otherwise test against the committed binaries;
`.github/scripts/local-core.sh` builds them from the checkout. A `--features python` or
`ruby` build overwrites the plain library in `target/release`; run `cargo cdylib` again.

## Working with the owner

- The owner drives git. Don't commit, push, tag, open pull requests or dispatch workflows
  unless asked for that step by name; a go on the edits is not a go on git. Leave the work
  in the working tree, verified, and report it.
- Agreeing that an approach is sound, or stating a preference, is a design decision, not
  an instruction to start. Reading to scope the work is fine; edits wait for the go. During
  a release the owner may be acting in parallel: `git fetch` and compare with origin before
  editing.
- Prove everything locally first, installing toolchains under `~` as needed. Every push to
  a PR branch runs the full matrix, so only what cannot run locally goes to CI, and the
  report says which parts those were.
- Fix stale docs, comments and links on sight; that permission is standing. READMEs state
  the current state only, history goes in the CHANGELOG, and README examples are verified
  by hand in the same pass. No CI step for docs.
- A change reaches all seven bindings and the sibling repos in the same pass, with parity
  in behaviour, API and wording. Where a binding can't follow, say why for that binding.
  Never leave a repo half-edited.
- Pre-1.0, a breaking change is a plain CHANGELOG entry under Unreleased; no semver
  callout, no upgrade hand-wringing.
- Install snippets keep `<version>` (Go: `@latest`); a concrete version there is the bug.
- Drafting release notes includes the pre-publish fixes: rename `[Unreleased]`, add the
  bullet for every package the diff touched, then draft from the fixed changelog.
- Never report commit, push or release state from memory; check with git or gh in the same
  turn, or leave it out.
- A verification counts only when its negative control fails. Copy the flags the repo's own
  alias or CI step uses before trusting a check.
- Formatters and their CI checks cover src, tests, benchmarks and smoke tests in every
  language.
- Tags are lightweight and unsigned (`git -c tag.gpgsign=false tag`); commits are always
  signed.
- Remove agent worktrees and their branches once the work is in the main checkout;
  `.claude/` is untracked and `git add` picks it up.
