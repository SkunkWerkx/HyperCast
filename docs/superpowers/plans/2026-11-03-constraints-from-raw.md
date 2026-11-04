# Constraints to and from the C ABI form: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give HyperCast's public Rust API `Constraints::from_raw` (ABI form → typed declaration) and `Constraints::pack_enum` (typed declaration → packed `enum`), with a representability refusal, a no-panic harness, and an opt-in CI canary in the Forge that proves the harness is really checked.

**Architecture:** `from_raw` ends in the existing private `Constraints::checked`, so `check` stays the only judge. `pack_enum` writes the layout `Entries::packed_decimal` / `next_packed` already read. A `no-panic`-feature-only module links `#[no_panic]` wrappers over both into the throwaway no-panic cdylib, and a `--cfg hypercast_canary` function there must make that build fail, which the Forge's new `no_panic_canary` input checks on every core-checks run.

**Tech Stack:** Rust 2024 (`no_std`, no `alloc`, `cargo no-panic`), wasmtime for the wasm32 suite, Python 3 `unittest` in the Forge, GitHub Actions reusable workflows.

**Spec:** `docs/superpowers/specs/2026-11-03-constraints-from-raw-design.md` (HyperCast). Read it first; it is the authority on behaviour.

## Global Constraints

- Three checkouts: HyperCast (`/Users/buvy/Developer/SkunkWerkx/HyperCast`), the Forge (`../HyperForge`), and nothing in HyperTabular. Both have unrelated uncommitted changes of the owner's (HyperCast on `napi10`, the Forge's `release-gate` / `release-prepare` actions): don't touch them, and `git fetch` and compare with origin before editing.
- **The owner drives git.** No commit, push, branch or PR. Each task ends with the work left in the tree, verified, and a line in the report. (This replaces the skill's "Commit" steps.)
- The core stays `no_std`, never names `alloc`, allocates nothing, and cannot panic. No new dependency, dev-dependencies included.
- No C ABI change: the export count stays 31, `check_exports.py` and every binding are untouched, and `rust/src/ffi.rs` is not edited.
- `pack_enum` returns `Option<usize>`: `Some(len)` is the packed length whether or not `out` was long enough. It writes only when `out.len() >= len`. `None` means the total overflows `usize`, and nothing is written.
- A text `enum` entry longer than `u32::MAX` bytes is `Refusal::NotHeldByDoor` (code 8), from the shared `check`.
- The packed integer entry is 16 bytes: `lo` (8, native endian), `hi` (4, native endian), `scale` (1, written `0`), sign (1, `0` or `1`), 2 bytes written `0`. A packed text entry is a little-endian `u32` byte length, then the bytes, no padding.
- The canary's name and cfg are both `hypercast_canary`. The Forge input is `no_panic_canary`, a string, default `""`.
- The no-panic diagnostic line to match is ``detected panic in function `<name>` `` (no-panic 0.1.x: `ERROR[no-panic]: detected panic in function \`{}\``).
- `cargo no-panic` passes vacuously on macOS. Every no-panic run in this plan happens in a Linux container (`rust:latest`), per `CLAUDE.md`.

## Review Focus

1. **`HAS_ENUM` with empty bytes, and bytes without `HAS_ENUM`**: `from_raw` must refuse each exactly as `check_constraints` does (`EmptyEnum` / `MalformedLayout`), never accept them. Test in Task 3.
2. **The wrong door's form**: `from_raw::<i32>` with a text keyword or packed text bytes, and the text `from_raw` with `minimum`, must refuse `KeywordNotForDoor` (or what `check_constraints` says for that door), never read text as decimals. Test in Task 3.
3. **A `size` other than 72**: `MalformedLayout` before anything else. Test in Task 3.
4. **Integer entries at the type edges** (`i64::MIN`, `u64::MAX`, negative values): pack and round-trip with the right sign byte and `hi` word. Test in Task 4.
5. **Byte-for-byte identity**: `pack_enum` on a `Constraints` built by `from_raw` returns exactly the bytes it was given, so HyperTabular can pass either through. Test in Task 4.

---

### Task 1: The Forge's opt-in no-panic canary

**Files:**
- Create: `../HyperForge/.github/scripts/no_panic_canary.py`
- Create: `../HyperForge/.github/scripts/test_no_panic_canary.py`
- Create: `../HyperForge/.github/scripts/fixtures/no_panic_canary/{canary_only,canary_and_other,unrelated_error}.log`
- Modify: `../HyperForge/.github/workflows/hyper-core-checks.yml` (the inputs near line 32, a step after "the public API and the C exports link with no panic path", near line 349)
- Modify: `../HyperForge/.github/scripts/test_hyper_core_checks.py` (`ORDER`, `test_inputs_are_check_cores_minus_binding_lints_plus_c_lint`)

**Interfaces:**
- Produces: the workflow input `no_panic_canary` (string, default `""`), which Task 5 sets in HyperCast's `ci.yml`.
- Produces: `no_panic_canary.py --name <fn> [--root <dir>] -- <command...>`, which runs `<command>` in `<root>` (default `.`) with `RUSTFLAGS="--cfg <fn>"` and exits `0` only for the canary-only verdict.

- [ ] **Step 1: Write the fixtures and failing tests.** The three logs are recorded or hand-written no-panic outputs:
  - `canary_only.log` contains ``ERROR[no-panic]: detected panic in function `hypercast_canary` ``;
  - `canary_and_other.log` contains that line plus one naming `cast_text`;
  - `unrelated_error.log` contains a rustc `error[E0425]` and no no-panic line.

  `test_no_panic_canary.py` (unittest, like its neighbours) tests a pure function `judge(returncode: int, log: str, name: str) -> tuple[bool, str]`:
  - `judge(0, '', 'hypercast_canary')` is `(False, …)`, and the message contains `passed the proof`.
  - `judge(101, canary_only, 'hypercast_canary')` is `(True, …)`.
  - `judge(101, canary_and_other, …)` is `(False, …)`, and the message names `cast_text` and not `hypercast_canary`.
  - `judge(101, unrelated_error, …)` is `(False, …)`, and the message contains `for some other reason` and the log's last line.
  - `main(['--name', 'x', '--', sys.executable, '-c', 'raise SystemExit(0)'])` returns non-zero, proving `main` runs the command and passes its result to `judge`.
  - A command that prints the canary line and exits 101 makes `main` return `0`, and the child saw `RUSTFLAGS == '--cfg x'`. The child prints its `RUSTFLAGS` and the test asserts on it.
- [ ] **Step 2: Run them and see them fail.** `python3 -m unittest discover -s .github/scripts -p 'test_no_panic_canary.py' -v` in `../HyperForge`. (`python3 -m unittest <path>` fails with `ValueError: Empty module name` and never reaches the tests.) Expected: an import error for `no_panic_canary`.
- [ ] **Step 3: Implement `no_panic_canary.py`**: `judge` as above, plus `main(argv) -> int`. `main` runs the command with `subprocess.run`, `env = os.environ | {'RUSTFLAGS': f'--cfg {name}'}`, and stdout and stderr merged. It prints the message, and on failure the log's last 20 lines. The module docstring says why the check exists: a proof that has never rejected anything hasn't been shown to check anything.
- [ ] **Step 4: Run them and see them pass.** Same command. Expected: all pass.
- [ ] **Step 5: Write the failing contract tests** in `test_hyper_core_checks.py`:
  - `ORDER` gains `'the canary panics'` right after `'no panic path'`.
  - The inputs test's expected set becomes `(set(old) - BINDING_LINTS) | {'c_lint', 'no_panic_canary'}`.
  - A new `test_no_panic_canary_is_opt_in` asserts: the input is type `string` with default `''`; the step whose name contains `the canary panics` has an `if` containing `inputs.no_panic`, `inputs.no_panic_canary != ''` and `fromJSON(inputs.no_panic_runners)`; its `working-directory` is `rust`; and its `run` invokes `../.hyperforge/.github/scripts/no_panic_canary.py` with `--name "${{ inputs.no_panic_canary }}"` and `-- bash -c "${{ inputs.no_panic_command }}"` (or an equivalent shape that runs the same command string).
- [ ] **Step 6: Run them and see them fail.** `python3 -m unittest discover -s .github/scripts -p 'test_hyper_core_checks.py' -v`. Expected: failures in `ORDER`, the inputs test and the new test. After Step 7, the same command must pass.
- [ ] **Step 7: Add the input and the step** to `hyper-core-checks.yml`. The input's description says what it names, that the same name is the `--cfg`, and that the canary build runs with only that cfg in `RUSTFLAGS` (which replaces the config's rustflags). The step is named `the canary panics, and is the only function the no-panic proof names`. Above it goes a comment in the file's voice: the normal build passed in the step before, and this one must fail, naming only the canary.
- [ ] **Step 8: Run the whole Forge suite.** `python3 -m unittest discover -s .github/scripts -p 'test_*.py' -v`. Expected: all pass. Then `actionlint` on the workflow if it's installed (`validate.yml` runs it in CI).
- [ ] **Step 9: Report**: the files changed in the Forge, left uncommitted. It merges before Task 5's HyperCast change and is a no-op until a crate sets the input.

### Task 2: Refuse a text entry the packed form can't hold

**Files:**
- Modify: `rust/src/constraints.rs` (`check`, the text-entry pass near line 861; `Refusal::NotHeldByDoor`'s doc near line 125)
- Test: `rust/tests/constraints_raw.rs` (new; Cargo discovers it)

**Interfaces:**
- Produces: `check` refusing any text entry with `len() > u32::MAX as usize` as `Refusal::NotHeldByDoor`. Task 4's packer relies on it to write a `u32` length without a check of its own.

- [ ] **Step 1: Write the failing tests** in `rust/tests/constraints_raw.rs`, under `#[cfg(target_pointer_width = "64")]`. A helper `zeroed(len) -> &'static str` uses `std::alloc::alloc_zeroed` and **panics** if the allocator returns null, so a test never passes without running its assertion. The zero pages are mapped lazily, and NUL is valid UTF-8 and not `White_Space`, so the checker reads only the entry's edges; build the `&str` with `core::str::from_utf8(...).unwrap()`.
  - `text_entry_past_u32_is_not_held`: `Constraints::text(TextRules::new(), Some(&[big]))` with `big.len() == u32::MAX as usize + 1` is `Err(Refusal::NotHeldByDoor)`.
  - `text_entry_at_u32_is_held`: the same at exactly `u32::MAX as usize` is `Ok`.
  - Free each allocation at the end of its test.
- [ ] **Step 2: Run them and see them fail.** `cargo test --test constraints_raw` in `rust/`. Expected: `text_entry_past_u32_is_not_held` fails (`Ok` returned).
- [ ] **Step 3: Implement.** In `check`, refuse `NotHeldByDoor` for an entry over `u32::MAX` bytes. Put it in code order: it's refusal 8, so it comes before the `enum` refusals 9–13 that the doc comment on `check_constraints` orders. Add the case to `NotHeldByDoor`'s doc ("…or a text `enum` entry longer than `u32::MAX` bytes, which no input can match").
- [ ] **Step 4: Run them and see them pass,** then the whole suite: `cargo test` in `rust/`. Expected: all pass, corpus unchanged.

### Task 3: `Constraints::from_raw`

**Files:**
- Modify: `rust/src/constraints.rs` (the two `impl` blocks at lines 438 and 489)
- Modify: `rust/tests/conformance.rs` (`constraints()` at line 581, `numeric_row` / `text_row`)
- Test: `rust/tests/constraints_raw.rs`

**Interfaces:**
- Produces: `impl<'e, T: Numeric> Constraints<'e, T> { pub fn from_raw(raw: RawConstraints, entries: &'e [u8]) -> Result<Self, Refusal> }` and the same signature on `impl<'e> Constraints<'e, Text>`.

- [ ] **Step 1: Write the failing corpus replay.** In `constraints()`, for every vector, also run `from_raw` on the vector's door with the `(raw, entries)` that `declaration` already builds:
  - For a refused row, `from_raw(...).map(|_| ())` must equal `check_constraints`'s result: the same `Refusal`, duplicates included.
  - For an accepted row, `numeric_row` / `text_row` also judge the input through the `from_raw`-built value and assert the same `CheckedRow` as the builder-made one. Pass `&entries` in, so a text row borrows it.
- [ ] **Step 2: Write the failing edge tests** in `constraints_raw.rs` (Review Focus 1–3), each asserting `from_raw`'s result equals `check_constraints(door, &raw, bytes)`'s:
  - `HAS_ENUM` with `&[]` is `EmptyEnum`;
  - packed bytes without `HAS_ENUM` is `MalformedLayout`;
  - `from_raw::<i32>` with `HAS_MIN_LENGTH` is `KeywordNotForDoor`;
  - `from_raw::<i32>` with `HAS_ENUM` and packed *text* bytes gives whatever `check_constraints(Door::I32, …)` gives (assert equality, not a code);
  - the text `from_raw` with `HAS_MINIMUM` is `KeywordNotForDoor`;
  - `size: 71` is `MalformedLayout` for both.
- [ ] **Step 3: Run them and see them fail.** `cargo test --test conformance --test constraints_raw`. Expected: `from_raw` not found.
- [ ] **Step 4: Implement** both as `Self::checked(DOOR, raw, Entries::packed(DOOR, entries))`, where `DOOR` is `T::DOOR` or `Door::Text`. Each doc comment names the ABI form, says it's refused where `check_constraints` refuses, and has a doctest building latitude (`minimum -90`, `maximum 90`, `multipleOf 0.000001` midpoint) through `raw()` → `from_raw`.
- [ ] **Step 5: Run and see them pass,** then `cargo test` (doctests included). Expected: all pass.
- [ ] **Step 6: Negative control.** Temporarily make `checked` skip the duplicate pass (`check(door, &raw, &entries, false)`) and run `cargo test --test conformance`. Expected: it fails on a duplicate-refusal row. Put it back and record the failure in the report.

### Task 4: `Constraints::pack_enum`

**Files:**
- Modify: `rust/src/constraints.rs` (the `impl<'e, T> Constraints<'e, T>` at line 516; a private `fn packed_len(&self) -> Option<usize>` beside it)
- Modify: `rust/tests/conformance.rs`, `rust/tests/allocation_free.rs` (near line 204, beside `latitude`)
- Test: `rust/tests/constraints_raw.rs`, plus unit tests in `rust/src/tests.rs`

**Interfaces:**
- Consumes: Task 2's guarantee (no text entry over `u32::MAX` bytes) and Task 3's `from_raw`.
- Produces: `impl<'e, T> Constraints<'e, T> { pub fn pack_enum(&self, out: &mut [u8]) -> Option<usize> }`.

- [ ] **Step 1: Write the failing round trip** in `conformance.rs`. For every accepted row:
  - `pack_enum(&mut [])` on the builder-made value gives `Some(len)`; packing again into `vec![0; len]` gives `Some(len)`.
  - Those bytes pass `check_constraints(door, &c.raw(), &bytes)`.
  - `from_raw(c.raw(), &bytes)` judges the row's input to the same `CheckedRow`.
- [ ] **Step 2: Write the failing unit and edge tests:**
  - `pack_short_buffer_writes_nothing`: for each form (`Integers`, `Text`, a `from_raw`-built `PackedIntegers` and `PackedText`, and no `enum`), with `len` from `pack_enum(&mut [])`, every `out` of length `0..len` filled with `0xAA` comes back unchanged with `Some(len)`. An `out` longer than `len` has its tail left as `0xAA`.
  - `pack_no_enum_is_zero`: `Some(0)`.
  - `pack_integer_edges` (Review Focus 4): `Constraints::<i64>::numeric(…, Some(&[i64::MIN as i128, -1, 0, i64::MAX as i128]))` and `Constraints::<u64>` with `u64::MAX`. Each packed 16-byte chunk decodes (`lo`/`hi` native endian, byte 12 scale `0`, byte 13 sign, bytes 14–15 `0`) to the entry, and the round trip judges `"-9223372036854775808"` and `"18446744073709551615"` as `enum` members.
  - `pack_is_identity_for_packed` (Review Focus 5): for a `PackedText` and a `PackedIntegers` input, `pack_enum` returns exactly the bytes given to `from_raw`.
  - In `rust/src/tests.rs`, unit-test the sum on every host: factor it into the private `fn packed_text_len(lengths: impl Iterator<Item = usize>) -> Option<usize>` (`packed_len` uses it). `[usize::MAX - 4]` gives `Some(usize::MAX)`, while `[usize::MAX - 3]` and `[usize::MAX / 2, usize::MAX / 2]` give `None`.
  - `#[cfg(target_pointer_width = "32")] pack_total_overflows_on_32_bit` in `constraints_raw.rs`. Every allocation and slice stays below wasm32's `isize::MAX` (2³¹ − 1), the most Rust can validly build:
    - Allocate one `alloc_zeroed` buffer of `(1 << 30) + 4` bytes and set bytes `0..5` to `b"abcde"`. **Panic** if the allocation returns null; never return early.
    - Take five overlapping entries, `buf[i..i + (1 << 30)]` for `i` in `0..5`. Each starts with a different letter, so none is a duplicate, even under folding. Convert each with `core::str::from_utf8(...).unwrap()`.
    - Packed, they take `5 × (4 + 2³⁰) = 5·2³⁰ + 20` bytes, more than `u32::MAX`.
    - Assert that `Constraints::text(TextRules::new(), Some(&entries))` is `Ok`, and that `pack_enum(&mut [0xAA; 8])` is `None` with the buffer unchanged.
    - The report quotes this test's `ok` line from the wasmtime run. A 1 GiB allocation fits wasm32's 4 GiB memory; if wasmtime's limit refuses it, the test fails, and that failure is reported, not skipped.
  - `allocation_free.rs`: `from_raw` and `pack_enum` (into a stack array) count zero allocations, for the latitude declaration and the sexes text `enum` already there.
- [ ] **Step 3: Run them and see them fail.** `cargo test`. Expected: `pack_enum` not found.
- [ ] **Step 4: Implement** `pack_enum`. Use `packed_len` (checked sums: `16` per integer, `4 + len` per text, the packed slice's own length for the packed forms), then return early if `out` is shorter. Write integers in the layout under Global Constraints, from `i128` through the same `Decimal` fields `packed_decimal` reads (`magnitude` split into `lo`/`hi`, `negative` for a negative entry; the checker has already held each entry to its door). Write text as `u32::to_le_bytes(len)` then the bytes, and copy packed forms as they are. Index with `get`/`split_at_mut` checks, never a bare `[]` that could panic (the no-panic harness in Task 5 will catch any).
- [ ] **Step 5: Run and see them pass.** `cargo test`. Then the wasm32 suite as `CLAUDE.md` gives it: `cargo test --target wasm32-wasip1 --target-dir <dir>` with `CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime run --dir=<repo>::<repo> --dir=<dir>::<dir>"`. Expected: all pass, with `pack_total_overflows_on_32_bit` executed and listed as `ok` (not ignored, not filtered out).

### Task 5: The no-panic harness and the canary

**Files:**
- Create: `rust/src/no_panic_harness.rs`
- Modify: `rust/src/lib.rs` (beside `#[cfg(feature = "exports")] mod ffi;`, line 257)
- Modify: `rust/Cargo.toml` (`[lints.rust] unexpected_cfgs` check-cfg for `cfg(hypercast_canary)`, the way HyperTabular's `Cargo.toml:167` does it)
- Modify: `.github/workflows/ci.yml` (the `core-checks` `with:` block, line 196)

**Interfaces:**
- Consumes: Tasks 3–4's `from_raw` and `pack_enum`; Task 1's `no_panic_canary` input.

- [ ] **Step 1: Write the harness.** `#[cfg(feature = "no-panic")] mod no_panic_harness;` in `lib.rs`. In the module, non-generic `#[unsafe(no_mangle)] pub extern "C"` functions, each `#[no_panic::no_panic]`, prefixed `hypercast_harness_` (never `cast_`, so nothing reads them as exports). Every input arrives as a raw pointer and length, rebuilt with `core::slice::from_raw_parts`, so nothing folds away:
  - `from_raw` for each of the 11 `Numeric` types and `Text`, returning the refusal code or `0`;
  - `pack_enum` over each form: a `from_raw`-built `PackedIntegers` and `PackedText`, no `enum`, and the Rust-list forms built by `Constraints::numeric` (from a `*const i128`, `count`) and `Constraints::text` (from a `*const &str`, `count`; `#[allow(improper_ctypes_definitions)]` with a one-line why), with `out` as pointer and length, returning the length or `usize::MAX` for `None`.
  - The module doc says what the module is for, that it's compiled only into the throwaway no-panic build, and why it exists: the new methods are generic and no export calls them.
- [ ] **Step 2: Add the canary**: `#[cfg(hypercast_canary)]` `hypercast_canary(value: u32) -> u32`, `#[no_panic]`, `assert!(value != 12_345, "the canary")`, with HyperTabular's comment: a proof that has never rejected anything hasn't been shown to check anything. Add the check-cfg lint line to `Cargo.toml`.
- [ ] **Step 3: The proof passes.** In a Linux container (`rust:latest`, repo mounted), `cargo no-panic` in `rust/`. Expected: success.
- [ ] **Step 4: The canary fails, by name.** In the same container, from `rust/`, `python3 <forge>/.github/scripts/no_panic_canary.py --name hypercast_canary -- cargo no-panic`. Expected: exit 0, printing that it was rejected by name. Then run `RUSTFLAGS="--cfg hypercast_canary" cargo no-panic` directly and keep the line naming `hypercast_canary` for the report.
- [ ] **Step 5: The harness is really checked.** Temporarily add an `out[0] = 0;` bare index on an unchecked path in `pack_enum`. `cargo no-panic` must fail, naming a `hypercast_harness_` function. Remove it and record the failure in the report.
- [ ] **Step 6: Turn it on in CI.** Add `no_panic_canary: hypercast_canary` to `ci.yml`'s `core-checks` `with:`. Then run `python3 -m unittest discover -s .github/scripts/tests -v` in HyperCast in case a workflow-contract test pins that block, and update the test it names if one does.

### Task 6: Docs and the whole-repo sweep

**Files:**
- Modify: `CHANGELOG.md` (the Unreleased constraints entry), `rust/README.md` (a constraints section), `CLAUDE.md` (the no-panic bullet near line 34)

- [ ] **Step 1: CHANGELOG.** One line in the 1.1.0 constraints entry: `Constraints::from_raw` and `Constraints::pack_enum` convert a declaration to and from its C ABI form, for a parser that carries declarations across its own C ABI (HyperTabular), and a text `enum` entry over `u32::MAX` bytes is refused `NotHeldByDoor`. *(Rust)*
- [ ] **Step 2: `rust/README.md`.** A constraints section in the crates.io README's voice: the checked doors, a latitude example (`f64`, `minimum -90`, `maximum 90`, `multipleOf 0.000001` midpoint) showing `Checked { value, adjusted }` and a `Violation`, a text `enum`, and one paragraph on `from_raw` / `pack_enum`. Verify every example by hand: paste it into a scratch `examples/` file, `cargo run --example`, then delete the file.
- [ ] **Step 3: `CLAUDE.md`.** The no-panic bullet adds that `src/no_panic_harness.rs` links the generic public API that no export reaches, and that core-checks makes `--cfg hypercast_canary` fail by name (the Forge's `no_panic_canary` input).
- [ ] **Step 4: The sweep, from the HyperCast root:**
  - `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` in `rust/`;
  - `cargo test` and the wasm32 suite;
  - `cargo test --no-default-features` (the crate without its default features; its test harness uses std, so this is not the no_std proof);
  - the no_std proofs exactly as the Forge's `hyper-core-checks.yml` runs them (lines 288–332):
    - `cargo check --no-default-features --target thumbv7em-none-eabi` in `rust/`;
    - the scratch `#![no_std]` consumer (`hypercast = { path = …/rust, default-features = false }`, `pub use hypercast as core_under_test;`), built with `cargo build` on the host and with `--target wasm32-unknown-unknown`;
    - the `extern crate alloc` grep with the same excludes.

    Add the two targets with `rustup target add` if they're missing;
  - `python3 .github/scripts/check_exports.py` (export count 31) and `python3 -m unittest discover -s .github/scripts/tests -v`;
  - Task 5 Steps 3–4 again in the container.

  Expected: all green, nothing in any binding's tree changed (`git status` shows only `rust/`, `.github/workflows/ci.yml`, the three docs, and this plan and spec).
- [ ] **Step 5: Report to the owner**: what ran locally, with each negative control's failing output (Task 3 Step 6, Task 5 Steps 4–5); what only CI proves (the Forge step on a real core-checks run, after the Forge change merges); the merge order (Forge first, then HyperCast); and the diff summary for both repos, uncommitted.
