# HyperCast — notes for agents

A `no_std`, allocation-free Rust core (`rust/`) of casting "doors", reached over a C ABI, as a
native extension, or, for Go, as Go source translated from its WebAssembly build, from nine
other languages, and from C and C++ through the release's C archives. `README.md` and
`docs/roadmap.md` have the why, each binding's README has its mechanics, and all CI and release
logic lives in the Forge (`SkunkWerkx/.github`). This file holds what the code alone won't
tell you.

## The contract

- A door returns a value or a fault (`Empty` / `Malformed` / `OutOfRange` plus a byte
  span). It never guesses: date order, epoch, Unix precision, separators and culture are
  all declared by the caller, and culture never lives in the core.
- A checked door (a door given `Constraints`: bounds, `multipleOf` with a rounding,
  lengths, `enum`, `trim`) returns a value, flagged when rounding adjusted it, or a
  rejection: the door's fault, or a `Violation` naming the OpenAPI keyword. Two arms, never
  a third. A declaration the core refuses is a contract violation and never means "no
  rules". Zero carries no sign: no door delivers `-0`.
- No binding throws for data. Every door hands back the verdict union in its language's own
  idiom (C#'s `Verdict<T>`, Java's sealed interface, Swift's enum, TypeScript's `ok`
  discriminant, R's `hypercast_verdict` rows, C++'s `noexcept` `Verdict<T>`, the C return
  code with `out` or `fault`); an exception is only ever a caller's contract violation, as
  each binding's README says.
- `corpus/*.json` is replayed byte for byte by the Rust suite and every binding (Ruby on
  both backends, C++ through `c/smoke/corpus.cpp`). A behaviour change is a corpus change,
  and every binding must agree. A new corpus file also joins `r_corpus` in
  `.github/hyper.json` (a test holds them equal).
- Fault and violation spans always satisfy `offset + len <= input.len()`, proven over every truncation
  of every corpus input (`rust/tests/fault_span_invariant.rs`); bindings slice with them.

## Proofs to keep green

- `cargo no-panic` proves at link time that no export can panic. That is why
  `rust/src/float.rs` is the crate's own float parser: `core`'s keeps bounds checks the
  optimizer cannot remove. Don't route parsing back through `core`'s. Rationale: the
  `float.rs` header and the Forge README ledger, "A float parser the proof accepts".
  `java/jni/examples/no_panic.rs` proves the same for every JNI entry point.
- `rust/tests/allocation_free.rs` demands zero allocations.

## Downstream: HyperTabular

HyperTabular links this crate with `default-features = false`, which turns off the
`exports` feature so both libraries can sit in one program (the `exports` comment in
`rust/Cargo.toml`). Changes to the doors, their types or features reach HyperTabular, and
a HyperCast release means bumping HyperCast in every one of HyperTabular's manifests. Its PHP
side uses `HyperCast\Interop` and the value classes, which the PIE extension now provides:
it requires `ext-hypercast`, not a Composer package.

## Claims are measured

Benchmark figures are dated, machine-specific receipts; losses are printed next to wins, and
corrections are published rather than quietly edited. Go's contention, rejected-cast and
civil date-time numbers are losses against the cgo binding it replaced and stay printed beside
its wins. Don't adjust numbers without re-running the harness, and don't drop a loss.

## Lists that move together

- Exports: `rust/src/ffi.rs` (cbindgen generates `include/hypercast.h` from it), C#'s three
  `LibraryImport` declarations (`"hypercast"`, `"*"`, `"__Internal"`), the sites in
  `.github/scripts/check_exports.py`, among them Go's two (`go/internal/core/core.go`, every
  export translated, which `go generate` rewrites, and `go/core.go`, every export called),
  R's C shim `r/src/hypercast.c`, TypeScript's generated `typescript/binding.d.ts` (every
  `#[napi]` function in `typescript/src/lib.rs` carries `catch_unwind`, and the one
  `#[napi(module_exports)]` hook, which replaces `castText` so every entry counts its
  arguments, calls it in its body; `check_exports.py` enforces both and
  `test_check_exports.py` pins them), Java's two `NativeCore`
  trees, `java/jni/src/lib.rs` (its extern block and its `RegisterNatives` table) and its
  `examples/no_panic.rs` (every JNI entry point carries `no_panic`, enforced by
  `jni_no_panic`), PHP's `rust/src/php_ext/` and its committed stub (not a C ABI site: every
  PHP-callable body starts with `guard(`, which `php_guard` enforces), the C++ header
  `include/hypercast.hpp` (hand-written; its `::`-qualified calls are a site) and the C smoke
  consumer `c/smoke/main.c`, `cdylib_smoke.py`, the prefixes in `.github/hyper.json` (the
  Forge's symbol check reads them), the AOT and wasm smoke tests, and the export count in the
  READMEs. `check_exports.py` fails on drift. The Blazor archive needs no export list: the
  SDK's pinvoke table links every P/Invoke by symbol. The `RawNumFormat` and `RawDayPeriods`
  layouts are pinned by `offset_of!` asserts in `rust/src/abi.rs`, carried by the header as
  `HyperCastRawNumFormat_SIZE` and `HyperCastRawDayPeriods_SIZE`, and restated in every
  binding.
- The shared library's own name is set in three places that must agree: `rust/build.rs`
  (install name `@rpath/libhypercast.dylib`, soname `libhypercast.so`; `cdylib` feature only),
  `IMPORTED_SONAME` in `c/cmake/hypercastConfig.cmake`, and the `needed` check in
  `c/smoke/run.sh`.
- The C archives: `C_RIDS` in the Forge's `c_package.py`, the platform list in `c/README.md`
  and the C row of the README's platform table.
- Ruby ABIs: `ABIS` in `ruby/Rakefile`, `ruby_compat_version` in `ci.yml`, and in the Forge
  the `ruby.abis` default in `hyper_config.py` (an `.github/hyper.json` override if this repo
  ever differs) and the `ruby_wasm_abis` default. Fiddle covers every Ruby and platform
  outside them and must never be restricted. Ruby 3.4 reaches EOL on 2028-03-31: then drop it
  from those lists and clear `ruby_compat_version`.
- npm's ten packages: the root `@skunkwerkx/hypercast`, eight `@skunkwerkx/hypercast-<suffix>`,
  one per RID, mapped by `NODE_PLATFORMS` in the Forge's `hyper_config.py` and listed as
  `napi.targets` in `typescript/package.json`, and the WebAssembly fallback
  `@skunkwerkx/hypercast-wasm32-wasip1` (the last `napi.targets` entry). The root's
  `optionalDependencies` are injected at pack time, never committed. The first publish of
  every package, a new platform's included, is the owner's, by hand with a granular token: npm
  Trusted Publishing attaches only to a package that exists, and `release.yml` is registered as
  its trusted publisher after.
- The Node floor: `engines.node` (`^22.13.0 || >=23.5.0`, the WASI runtime's floor) in
  `typescript/package.json`, the same string in the Forge's `npm_package.py`, its
  `node_floor_versions` default, `FLOOR_IMAGE`'s default `node:22.13.0-alpine` and
  `LOCKFILE_NPM` in `typescript/consumer-tests/run.sh`, and the floor sentences in
  `typescript/README.md`, `README.md` and the CHANGELOG. Node 22 reaches EOL on 2027-04-30:
  then drop it from all of them.
- The JDK floor: `jdkFloor` (`21`) in `java/build.gradle.kts`, `jdk_floor_version` in
  `ci.yml`, and the floor sentences in `java/README.md` and the README. JDK 21 reaches EOL at
  Temurin on 2029-12-31: then the floor is 25, and the JNI base, `java/jni/`, `java_jni`,
  `lint_java_jni`, the JNI sites in `check_exports.py` and `checkFloorApi` go, the compile goes
  back to `--release`, and the FFM `NativeCore` moves back into the base.
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
  `test-blazor` job's .NET 10 SDK and `wasm-tools-net10` steps (unconditional, so nothing
  else prompts them), and the floor sentences in `csharp/README.md` and `README.md`. .NET 10
  reaches EOL on 2028-11-14: then drop it from all of them, and the targets folder moves to the
  new lowest TFM. `Match` and `Switch` on `Verdict<T>` stay until a major version removes
  them; they are public API.
- The PHP floor: `php` in `composer.json` and `php/composer.json`, `php_floor_version` in
  `ci.yml` (an explicit input, since the Forge's 8.2 default still serves HyperTabular), the
  lowest of the PHP minors below, and the floor sentences in `php/README.md` and the root
  README. PHP 8.3 reaches EOL on 2027-12-31: then drop it from all of them.
- The PHP minors the extension is built for (8.3, 8.4, 8.5): the Forge's `php_versions`
  default, `MINORS`, `WINDOWS_COMPILERS` and `WINDOWS_TOOLSETS` in its `php_ext_package.py`,
  the `minors` default in `hyper-pie-acceptance.yml`, and the platform and minor table in
  `php/README.md`. A new minor is 6 more zips on the release; the count (18 today) is in
  `php/README.md`, `.github/RELEASING.md` and the CHANGELOG. No Intel macOS zip.
- The R wrapper's core pin: `hypercast = { version = "=X.Y.Z"` in `r/src/rust/Cargo.toml` is a
  version file the Forge bumps with `r/DESCRIPTION`; `r/tools/vendor.sh` refuses a pin that
  differs from `rust/Cargo.toml`. `r/inst/AUTHORS` is regenerated by `vendor.sh`; never edit
  it by hand.
- The Python sdist: built in the Forge's `hyper-build-wheels.yml` because `.github/hyper.json`
  says `sdist: true`; one more pypi inventory file beside the wheels. Its Rust floor is
  `rust-version`, proven at exactly that toolchain by the build job.
- The Go core's regeneration pins: the rustc in `go/internal/wasm2go/rust-toolchain.toml` (a
  reproducibility constraint, not a floor) and wasm2go's version in its `go.mod`, each moved
  by regenerating and committing the core. Go's floor is `go` in `go/go.mod` (`1.26`) and the
  floor sentence in `go/README.md`.

## Never hand-edit

- `include/hypercast.h` and the Swift bundle's copy are generated by cbindgen from
  `rust/src/ffi.rs` and the types its exports take: `HYPERCAST_BLESS=1 cargo test --test header`
  in `rust/` rewrites both, and the test fails when either differs.
- `rust/src/fold_table.rs` is the simple case folding (`C` and `S` rows) generated from
  `rust/unicode/CaseFolding-17.0.0.txt`, which stays as Unicode published it:
  `HYPERCAST_BLESS=1 cargo test --test fold_table` in `rust/` rewrites it, and the test fails
  when it differs.
- `go/internal/core/core.go` is wasm2go's translation of the core's `wasm32-wasip1` build:
  `go generate` in `go/` rewrites it with the rustc pinned in
  `go/internal/wasm2go/rust-toolchain.toml` (`rustup toolchain install <pin> --target
  wasm32-wasip1`), and `HYPERCAST_CHECK_CORE=1 go test -count=1 -run
  'TestCoreIsCurrent|TestRegenerationCarriesTheCrateVersion' .` diffs it. Any change under
  `rust/src` that reaches the cdylib means regenerating it; `prepare-release.yml` regenerates
  it at the version (the version is a literal in it).
- `typescript/binding.js` and `binding.d.ts` are napi-rs output: `npm run build` in
  `typescript/` rewrites them.
- `php/stubs/hypercast.stub.php` is generated from the extension by `php
  tools/generate-stubs.php` in `php/` (Linux or macOS), with cargo-php pinned to the
  `CARGO_PHP_VERSION` that script names (0.2.1); `StubTest` regenerates and compares it when
  `HYPERCAST_CHECK_STUBS=1`.
- The root `Package.swift`'s release version and the two Swift zips' checksums are written by
  `ci.yml`'s `stage-release` job in the preparation run. No binary is committed anywhere.
- Versions: bump through `prepare-release.yml`. The libraries embed their version.

## Toolchains and floors

Each language's primary toolchain is its current release; check endoflife.date rather than
assuming. Exceptions must be commented where pinned: a real platform constraint, or an
older floor shipped beside the primary and dropped at its EOL. Floors match HyperUuid and
HyperTabular and move together; PHP's 8.3 reaches HyperTabular when it is ported.

Rust has two exceptions, both on stable with `RUSTC_BOOTSTRAP=1`, which the Forge sets on
exactly two build steps and nowhere else; never set it more widely. Neither has a stability
promise, so every stable bump is re-proven by the legs that use it.

- The Blazor archive (`hyper-build-wasm.yml`) is built with `-Zbuild-std`, because only that
  leaves out the prebuilt sysroot's legacy exception handling and the features .NET 10's
  wasm-opt rejects (`rust/.cargo/config.toml` says why). When build-std stabilizes, the
  variable goes from that step.
- The Windows PHP extension (`hyper-build-php-extension.yml`'s Windows build step): ext-php-rs
  needs the unstable `abi_vectorcall` there (rust-lang/rust#124485, the `cfg_attr` at the top
  of `rust/src/lib.rs`). When the ABI stabilizes, the variable goes from that step.

## Forge coupling and releases

- The Forge is called at master, deliberately unpinned: a Forge merge reaches every consumer
  at once, and breakage it causes is fixed forward. Each run resolves master to one commit
  (`ci.yml`'s `forge` job, `release.yml`'s `find-ci-run`), checks the Forge out at it as
  `.hyperforge` and runs its composite actions from there: in `ci.yml`'s release tail
  `pack-gems`, `pack-npm`, `pack-r`, `pack-c`, `verify-wheels`, `release-prepare` and
  `stage-release`; in `release.yml` `tag-guard`, `tag-gate`, `publish-nuget`,
  `publish-rubygems`, `publish-npm`, `publish-pypi` and `finalize-release`. The six reusable
  workflows (NuGet, crate and Maven packing in the tail; crate and Maven publishing and the
  PIE acceptance, `release.yml`'s last job) are called at master and resolved when the run is
  queued. The inventory records the commit that packed, and the tag run requires its own
  Forge to descend from it. Don't merge Forge changes while a preparation is in flight.
- The board is one Forge workflow per concern: `core` (`hyper-core.yml`, the seven core legs,
  each with its cdylib export check and C smoke), `core-checks` (`hyper-core-checks.yml`, the
  static checks on three hosts, s390x included), `lint-<binding>` (one job on
  `ubuntu-26.04-arm`) and `test-<binding>` (the binding's suite on its hosts), gated on what
  the pull request touched (`changes`, from `.github/scripts/pr_changes.py`; `pr-board` is the
  one status to read, red when a job failed or was skipped while its gate said run). Off a
  pull request nothing is gated and the whole release-mode board runs. A new binding is one
  `lint-` and one `test-` caller, one class in `pr_changes.py` and one gate in `pr_board.py`
  (its tests hold the three together).
- `prepare-release.yml` and ci.yml's `core-checks` are one `uses:` each of a Forge reusable
  workflow; this repo keeps triggers, permissions and gates. `.github/hyper.json` is what the
  Forge knows about this project (crate and project names, the export rule, the cdylib smoke
  script, the sdist, `swift_remote` and `c_assets` switches, `r_corpus`, and
  `big_endian_control`, the corpus vector `core-checks` corrupts on s390x to prove the suite
  reads the corpus there and the test that must then fail; the ABI probe is
  `.github/scripts/abi_probe.c`, the Forge's default path; every other name is derived in the
  Forge's `hyper_config.py`). `.github/scripts/tests` holds the workflow shape contracts and
  the probe, export, board and text-file tests; the generic release-guard tests live in the
  Forge.
- The release is two acts: `prepare-release.yml` pushes A, and that run's release-mode
  `ci.yml` packs every registry input, attests each package and the inventory at A, preserves
  `release-packages` for 90 days, runs CodeQL on A and commits B; the tag push at B verifies
  the preserved bytes, signs the release attestation from the tag and publishes. `gh workflow
  run release.yml --ref vX.Y.Z -f publication_run_id=<tag run>` replays it with the receipts
  (every release runs that once, as its no-op recovery proof). The finalizer writes the GitHub
  Release from the CHANGELOG's `## [X.Y.Z]` section; `prepare-release.yml` dates that heading,
  sets `r/NEWS.md`'s, and refuses a CHANGELOG without it or with a placeholder left in it.
- Third-party actions are pinned to a commit with the version in a trailing comment, and
  `.github/dependabot.yml` moves them with every other ecosystem in one monthly pull request.
  It ignores the Forge and `dtolnay/rust-toolchain` (branch heads, refreshed by hand).
- This repo conforms to the Forge; it doesn't add opt-outs (`ci.yml` header).
- Registry versions can never be replaced. Release and recovery: `.github/RELEASING.md`
  and the Forge's `docs/release-runbook.md`. Recover by re-dispatching from the preserved
  packages, never by retagging or rebuilding. CRAN submission is by hand, from the R tarball
  the tag run preserved.

## Local loop

The PR board runs a binding's lint and suite only when its tree, the core (`rust/`,
`include/`, `c/`, `corpus/`), or `.github/` changed; `python3 .github/scripts/pr_changes.py`
classifies a `filename<TAB>previous_filename` list from stdin the way the `changes` job does
(`--all` is the release board), and `python3 .github/scripts/pr_board.py` with `NEEDS`,
`CHANGES` and `GITHUB_EVENT_NAME` set gives the verdict.

Run `cargo test` and `cargo no-panic` (GNU ld: Linux, or a Linux container) in `rust/`. Build
`cargo cdylib` before a binding's tests. A raw `cargo rustc --release --crate-type cdylib
--features python` (or `ruby`, or `php`) in `rust/` overwrites the same `target/release`
library, and the other bindings then fail on missing symbols until `cargo cdylib` runs again;
maturin in `python/` and the `ruby-ext`/`php-ext` aliases build elsewhere and don't.

Swift has no core until one is built: `.github/scripts/local-core.sh` builds the checkout's own
into the gitignored `swift/HyperCastCore.artifactbundle/` (it finds `../HyperForge` on its own;
`FORGE=<path>` otherwise) and stages the same archive in `csharp/HyperCast/staticlibs/<rid>/`
for a local Native AOT publish, then `swift test`. It starts with
`.github/scripts/local-clean.sh`, which removes every binding's staged build artifact and names
what rebuilds each, so nothing a suite loads predates the core being built; run it alone before
any local build after a `rust/` change. CI builds each leg's core into an empty tree.

Go needs nothing built: `go test ./...` in `go/` runs on the committed translated core
(`CGO_ENABLED=0`, as CI runs it). After a change in `rust/`, `go generate` in `go/`, then the
regeneration check above (`-count=1`, since `go test` replays a cached pass). The wasm legs:
`tinygo test -target=wasip1 -c -o suite.wasm .`, then `wasmtime run --dir "$PWD/..::/repo"
--env HYPERCAST_CORPUS=/repo/corpus --env HYPERCAST_CRATE_VERSION=<version> suite.wasm
-test.v`; and `GOOS=js` or `GOOS=wasip1` with `GOARCH=wasm` `go test ./...` with `$(go env
GOROOT)/lib/wasm` on PATH.

PHP is the extension built from `rust/`: `cargo php-ext` there, then `composer test` in `php/`
(`composer bench` for the benchmarks), loading `rust/target/php/release/` with `-d
extension=`, or the build `HYPERCAST_PHP_EXTENSION` names; `HYPERCAST_CHECK_STUBS=1 composer
test` runs the stub check (cargo-php and a C compiler needed).

Java's JNI path needs its own library: `cargo build --release --locked --lib` in `java/jni/`,
after `cargo cdylib`. Gradle stages both and refuses a stale one. Then, from `java/`,
`./gradlew test testFloor checkSuiteParity javadoc` with a JDK 21 passed through
`-Porg.gradle.java.installations.paths=<jdk21>`.

TypeScript builds its own addon, a napi-rs crate path-depending on `rust/`: from
`typescript/`, `npm ci && npm run build && npm test`, then `npm run test:bun` and `npm run
lint`. The WebAssembly fallback (`rustup target add wasm32-wasip1`): `npm run build:wasi`, then
`NAPI_RS_FORCE_WASI=error npm test`. The consumer tests are `typescript/consumer-tests/README.md`
(Verdaccio on port 4873; `floor` and `s390x` need Docker).

R builds the core from its vendor tarball, never from `rust/` directly: `r/tools/vendor.sh`
resolves the pinned version from the checkout's `rust/`, vendors the core into
`r/src/rust/vendor.tar.xz`, derives the wrapper's lock, stages the header and the corpus and
rewrites `inst/AUTHORS` (all gitignored but `AUTHORS`), and has to run again after any change in
`rust/`, `include/` or `corpus/`. Then, from the repo root, `Rscript -e 'devtools::test("r")'`
and `R CMD check --as-cran` on the tarball `R CMD build r` makes; lint against a scratch install
(`R CMD INSTALL --library=<dir> r`, then `R_LIBS=<dir>`).

C builds an archive from local libraries with the Forge's packer, then the smoke consumers
against it (CMake and pkg-config on PATH), after `cargo cdylib` and `local-core.sh`:

```sh
python3 ../HyperForge/.github/scripts/c_package.py assemble --rid osx-arm64 --out <dir> \
  --static swift/HyperCastCore.artifactbundle/arm64-apple-macosx/libhypercast.a \
  --shared rust/target/release/libhypercast.dylib
SMOKE_CXX=1 c/smoke/run.sh <dir>/hypercast-<version>-osx-arm64 <version>
```

`SMOKE_ARCH=x86_64` runs an osx-x64 archive under Rosetta, and `c/smoke/apple-mobile.sh <dir>
<version>` takes the four Apple mobile archives.

The Blazor archive needs stable's sources: `rustup component add rust-src` and `rustup target
add wasm32-unknown-emscripten`, emsdk on PATH, then `RUSTC_BOOTSTRAP=1 cargo wasm-staticlib`
in `rust/`, after deleting `rust/target/.rustc_info.json` and
`rust/target/wasm32-unknown-emscripten/` (cargo caches rustc's cfg without regard to the
variable, and a cache from a plain cargo run for that target builds a different archive); the
Forge's `wasm_archive_guard.py` checks the result.

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
- A change reaches every binding (C++'s header included) and the sibling repos in the same
  pass, with parity in behaviour, API and wording. Where a binding can't follow, say why for
  that binding. Never leave a repo half-edited.
- Pre-1.0, a breaking change is a plain CHANGELOG entry under Unreleased; no semver
  callout, no upgrade hand-wringing.
- Install snippets keep `<version>` (Go: `@latest`); a concrete version there is the bug.
- Drafting release notes includes the pre-publish fixes: rename `[Unreleased]`, add the
  bullet for every package the diff touched, and set `r/NEWS.md`'s heading to the version,
  then draft from the fixed changelog.
- Never report commit, push or release state from memory; check with git or gh in the same
  turn, or leave it out.
- A verification counts only when its negative control fails. Copy the flags the repo's own
  alias or CI step uses before trusting a check.
- Formatters and their CI checks cover src, tests, benchmarks and smoke tests in every
  language.
- Tags are annotated and signed: `git tag -m "HyperCast vX.Y.Z" vX.Y.Z <B>` (`tag.gpgsign` is on,
  so the tag carries the owner's SSH signature); commits are always signed.
- Remove agent worktrees and their branches once the work is in the main checkout;
  `.claude/` is untracked and `git add` picks it up.

## Open follow-ups (as of 2026-11-01)

- The next release is the first with npm, CRAN, C archives, PIE zips, the Swift zips and the
  JNI jar. What only it can prove: npm publishing under OIDC (after the owner's `0.0.1`
  placeholders and trusted-publisher registration on all ten names), the hand CRAN submission,
  the PIE install from Packagist, the Swift zips resolving by URL and checksum, and the first
  Maven Central publish of the multi-release jar.
- TypeScript has no locale bridge yet (C#, Java and Swift do); `Intl.NumberFormat` is the
  natural source.
- Feature follow-ups live in the roadmap, not here.
