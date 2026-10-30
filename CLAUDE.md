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

- Exports: `rust/src/ffi.rs` and `abi.rs`, every binding's declarations, the sites in
  `.github/scripts/check_exports.py` and `cdylib_smoke.py`, and the prefixes in
  `.github/hyper.json` (the Forge's symbol check reads them); `check_exports.py` fails on
  drift. The Blazor archive needs no export list: the SDK's pinvoke table links every
  P/Invoke by symbol. The `RawNumFormat` and
  `RawDayPeriods` layouts are pinned by `offset_of!` asserts in `rust/src/abi.rs` and
  restated in every binding.
- Ruby ABIs: `ABIS` in `ruby/Rakefile`, `ruby_compat_version` in `ci.yml`, and in the Forge
  the `ruby.abis` default in `hyper_config.py` (an `.github/hyper.json` override if this repo
  ever differs) and the `ruby_wasm_abis` default. Fiddle
  covers every Ruby and platform outside them and must never be restricted. Ruby 3.4
  reaches EOL on 2028-03-31: then drop it from those lists and clear
  `ruby_compat_version`.
- The .NET floor: `TargetFrameworks` in `csharp/Directory.Build.props` (the Apple and Android
  smoke tests, the gate probe and the benchmarks set their own), ci.yml's
  `csharp_aot_frameworks` and `csharp_extra_runtime`, the `HYPERCAST003` gate and the
  `build/net10.0/` folder name (which the smoke tests and `csharp/HyperCast.PlatformGateTest`
  import by path), `HyperCast/Polyfill.cs` and the remarks on `Verdict<T>`,
  `test-nuget-package`'s layout step (its `lib/` and `build/` paths and the
  no-`build/net11.0` check), its AOT loop (`net10.0 net11.0`) and control (`-f net10.0`), the
  needles `test_workflow_contracts.py` pins for that job, its floor step (a net9.0 restore
  must fail with NU1202) and its gate step, the Apple and Android probe steps that hard-code
  `net10.0-*`, the WasmSmokeTest's per-framework references and `check.sh`'s net10 legs and
  header, `wasm-tools-net10` in the `HYPERCAST002` text and both READMEs, the Forge
  `test-blazor` job's .NET 10 SDK and `wasm-tools-net10` steps (keyed on the wasm pin, not
  the floor), and the floor sentences in `csharp/README.md` and `README.md`. .NET 10 reaches
  EOL on 2028-11-14: then drop it from all of them, and the targets folder moves to the new
  lowest TFM. `Match` and `Switch` on `Verdict<T>` stay until a major version removes them;
  they are public API. ## Never hand-edit
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

Rust's one exception is the Blazor archive: `wasm.toolchain` in `.github/hyper.json` pins a
dated nightly, because only `-Zbuild-std` leaves out the prebuilt sysroot's legacy exception
handling and the features .NET 10's wasm-opt rejects (`rust/.cargo/config.toml` says why).
Moving the pin is its own PR, re-proven by CI's guard and Blazor legs, and it matches
HyperUuid's. When build-std stabilizes, the pin goes and `cargo wasm-staticlib` returns to
stable. Everything else in `rust/` stays on stable.

## Forge coupling and releases

- The Forge is called at master, deliberately unpinned: a Forge merge reaches every consumer
  at once, and breakage it causes is fixed forward. `release.yml` resolves master to one
  commit per run (`find-ci-run`'s `forge_sha`), checks the Forge out at it as `.hyperforge`
  and runs its composite actions from there (`release-guard`, `pack-gems`, `verify-wheels`,
  `release-gate`, `publish-nuget`, `publish-rubygems`, `publish-pypi`, `finalize-release`);
  the five reusable workflows (NuGet, crate and Maven packing; crate and Maven publishing)
  are called at master and resolved when the run is queued. The inventory records the
  commit and recovery requires it unchanged. Don't merge Forge changes while a release
  is in flight.
- `prepare-release.yml`, `stage-native-binaries.yml` and ci.yml's `check-core` are one
  `uses:` each of a Forge reusable workflow; this repo keeps triggers, permissions and gates.
  `.github/hyper.json` is what the Forge knows about this project (crate and project names,
  the export rule, the cdylib smoke script; the ABI probe is `.github/scripts/abi_probe.c`,
  the Forge's default path; every other name is derived in the Forge's `hyper_config.py`). `.github/scripts/tests` holds the workflow shape contracts
  and the probe tests; the generic release-guard tests live in the Forge.
- The release is two acts at one tag: the tag push packs, attests (build provenance and the
  release attestation over every shipped file) and preserves without uploading; `gh workflow
  run release.yml --ref vX.Y.Z -f prepared_run_id=<tag run>` publishes from the preserved
  bytes. The finalizer writes the GitHub Release from the CHANGELOG's `## [X.Y.Z]` section,
  and the tag run refuses a CHANGELOG without it.
- Third-party actions are pinned to a commit with the version in a trailing comment, and
  `.github/dependabot.yml` moves them with every other ecosystem in one monthly pull request.
  It ignores the Forge and `dtolnay/rust-toolchain` (branch heads, refreshed by hand).
- This repo conforms to the Forge; it doesn't add opt-outs (`ci.yml` header).
- Registry versions can never be replaced. Release and recovery: `.github/RELEASING.md`
  and the Forge's `docs/release-runbook.md`. Recover by re-dispatching from the preserved
  packages, never by retagging or rebuilding.

## Local loop

Run `cargo test` and `cargo no-panic` in `rust/`. Build `cargo cdylib` before a binding's
tests.

Go, Swift and PHP otherwise run against the committed binaries, which are refreshed only at
release and so lag every core change: build the checkout's own with
`.github/scripts/local-core.sh` (`FORGE=<path>` if the Forge checkout is neither `../.github`
nor `../HyperForge`), then `go test -tags hypercast_local ./...`,
`HYPERCAST_LOCAL_CORE=1 swift test`, and PHP with `HYPERCAST_NATIVE_LIBRARY`. Outside CI the
three suites refuse to run against a committed core older than `rust/`; CI builds each leg's
core before its suites, so it never sees the committed ones. Go caches a pass from before a
core change; `-count=1` runs the check again.

The Blazor archive needs the pinned nightly with its sources: `rustup toolchain install
<pin> --component rust-src --target wasm32-unknown-emscripten`, emsdk on PATH, then `cargo
+<pin> wasm-staticlib` in `rust/`; the Forge's `wasm_archive_guard.py` checks the result.

A raw `--features python` or
`ruby` build in `rust/` overwrites the plain library in `target/release`; run `cargo cdylib`
again. maturin in `python/` and the `ruby-ext`/`php-ext` aliases build elsewhere.

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

## Open follow-ups (as of 2026-10-23)

- 1.0.0 is the first release on the two-step flow. What only it can prove: the composite
  actions in a real job, PyPI Trusted Publishing, nuget.org's finalized bytes, the Maven
  confirmation wait, the finalizer. Afterwards the Forge drops its schema-1 inventory paths.
- Feature follow-ups live in the roadmap, not here.
