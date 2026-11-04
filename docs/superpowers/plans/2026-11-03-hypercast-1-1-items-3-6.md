# HyperCast 1.1 items 3–6: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Strict grouping, fault detail, opt-in date shapes and the cleanup backlog, in HyperCast's core, its C ABI, all ten bindings and the C++ header.

**Architecture:** The core gains a `Detail` on every failure path, carried through one parse that both the old doors (projected to `Fault`) and a new `detailed` module return. Grouping policy lives in two `NumFormat.flags` bits. Date shapes arrive through a validated `DateOptions` value that only the detailed paths take. Four door-dispatched C exports carry all of it. Each binding then moves onto the detailed path in one task, compatibly with its 1.0 surface.

**Tech Stack:** Rust 2024 (`no_std`, no `alloc`, `cargo no-panic`), cbindgen, and each binding's toolchain (`CLAUDE.md`, "Local loop").

**Spec:** `docs/superpowers/specs/2026-11-03-hypercast-1-1-items-3-6-design.md`. Every rule, code, contract and owner decision is there; this plan assigns them to tasks and fixes the names.

## Global Constraints

- **Branch and git:** worktree `.worktrees/HyperCast`, branch `constraints_from_raw`, starting from item 2's merged, reviewed result. **No commits, stages, stashes or branches by agents.** The owner drives git.
- **Core properties:** the core stays `no_std`, allocation-free and panic-free (`cargo no-panic` plus the `hypercast_canary` canary in a Linux container). No new dependency.
- **1.0 interfaces unchanged:** the 32 existing exported symbols, `Fault`, `RawFault`, `RawNumFormat`'s layout, null-format defaults, and Rust struct literals over public types. **The old C exports ignore `flags` bits 8 and up and always parse with legacy grouping.**
- **Grouping policy:** bits 8–9 of `flags`. `GROUPING_LEGACY = 0`, `GROUPING_WESTERN = 1 << 8`, `GROUPING_INDIAN = 2 << 8`, `GROUPING_POLICY = 3 << 8`. Policy `3 << 8` is refused, and so is any undefined bit, by the 1.1 `check` that the detailed paths and binding validators run.
- **Detail codes:** the spec's table, numbers 1–32, never renumbered. `0` means "no detail" (a checked violation only).
- **Door codes:** existing, plus `Door::Char = 23`. `param` uses the existing ABI codes (Unix precision 1–4, date order, Excel epoch), and must be 0 where a door declares nothing.
- **Exports:** 32 symbols become 36: `cast_detailed`, `cast_detailed_from_f64`, `cast_checked_detailed`, `cast_checked_detailed_from_f64`. Every "Lists that move together" site moves, and `check_exports.py` must pass.
- **Layouts:**
  - `RawFaultDetail { size, offset, len, detail }`: 16 bytes, accepted when `size` ≥ 16 and a multiple of 4; the core writes bytes 4–16 only.
  - `RawDateOptions { size, flags, pivot }`: accepted at size 8 or 12; above 12 refused.
  - **Vocabulary entry:** a month byte, then a little-endian `u32` length, then the bytes.
- **Validation before writes:** every validation runs before any output write, and refusal returns −1 with nothing written.
- **Benchmarks:** claims are measured, in dated receipts with losses printed. Benchmarks run only when nothing else heavy is running.
- **The culture-sweep stop condition still applies.** A bridge that newly throws or loses a symbol stops the task. So does a bridge whose number and currency patterns need different grouping policies (spec, owner decision 1).

## Review Focus

1. **Native digits with a space-like group separator under strict Western:**
   - with native Arabic-Indic digits (`٠`, U+0660) declared and `U+202F` as the group separator, `١ ٢٣٤` (U+202F between) reads as 1234;
   - `١٢ ٣٤` is `Malformed`, with `group_size` spanning ` ٣٤`, counted in digits, not bytes.

   Test in Task 2.
2. **A dotted month name, the `.` separator and a pivot together:** with `janv.` declared, pivot 2029, order DMY, `7.janv..26` reads as 2026-01-07, and `7.janv.26` is `Malformed`. Test in Task 3.
3. **A door valid for one export passed to another:**
   - `cast_detailed_from_f64` with door `Text` (18) returns −1, with `out` and `fault` sentinels untouched;
   - `cast_detailed` with door 24 returns −1 likewise.

   Test in Task 4.
4. **An old C export given a 1.1 flag:** `cast_i32("12,34", format with GROUPING_WESTERN set)` succeeds with 1234. The old export ignores bits 8 and up. Test in Task 4.
5. **A strict format with grouping off:** `NumFormat::STRICT` with `GROUPING` cleared refuses `1,234` (group separator forbidden) through the detailed path. Test in Task 2.

---

### Task 1: `Detail`, `Failure`, the one-parse plumbing and the `detailed` module

**Files:**
- Create: `rust/src/detail.rs` (`Detail`, `Failure`), `rust/src/detailed.rs` (the module)
- Modify: every fault-producing module (spec Appendix B: `boolean.rs`, `character.rs`, `checked.rs`, `decimal.rs`, `integer.rs`, `real.rs`, `temporal.rs`, `text.rs`, `typed.rs`, `uuid.rs`, `verdict.rs`), `lib.rs`
- Test: `corpus/*.json` (a `detail` field on every failure row), `rust/tests/conformance.rs`, `rust/tests/fault_span_invariant.rs`, `rust/src/tests.rs`

**Interfaces:**
- Produces:
  - **The authoritative per-site table** (spec: "a test-checked table"). Every fault construction names a `Site`: a `pub(crate)` enum with one variant per site, doc-commented with its file and condition. A `pub(crate) const SITES: &[(Site, Reason, Detail)]` gives each site's reason and detail, and `docs/fault-sites.md` is generated from it by a test, never hand-edited.
  - `#[non_exhaustive] pub enum Detail { Blank = 1, …, NotAScalar = 32 }`, in the spec's order, with `Detail::code(self) -> u32` and `Detail::from_code(u32) -> Option<Detail>`;
  - `#[non_exhaustive] pub struct Failure { pub reason: Reason, pub detail: Detail, pub offset: u32, pub len: u32 }`, with `Failure::fault(self) -> Fault`;
  - `#[non_exhaustive] pub struct ValidNumFormat`, built by `NumFormat::validate(self) -> Result<ValidNumFormat, DeclarationError>`, where `DeclarationError` is a `#[non_exhaustive]` enum, 1.0 rules for now (Task 2 adds the policy rules);
  - `pub mod detailed`: one function per existing door with the same name, returning `Result<T, Failure>`. Numeric doors take `&ValidNumFormat`, not `&NumFormat`. Checked doors return `Result<Checked<T>, detailed::Rejection>`, an enum with `Failure(Failure)` and `Violation(Violation)`. The typed doors are covered too.
  - Every existing door is `detailed::…(…).map_err(Failure::fault)`, or the equivalent one-parse projection.

- [ ] **Step 1: Write the failing corpus rows.**
  - Add `"detail": "<code name>"` to every failure row in every corpus file.
  - Add the spec's overlap vectors, the pinned-span exceptions, the non-finite rows (`NaN`, `-inf`, `Infinity`, `+INF`, `nana`, `infinite` at `i32`/`f64`/`decimal`; typed NaN and ∞; `1e400` at `f64` and `1e39` at `f32` as `too_large`), the UUID precedence row (bad hex at byte 0 plus a bad hyphen at byte 8 reports byte 8), and a row for each of the 23 splits.
  - The `(-$5)` row expects the new sign span.
- [ ] **Step 2: Make `conformance.rs` replay through `detailed`.** On every row, assert the detail, and assert that the old door equals the detailed result projected to `Fault`. Run `cargo test --test conformance` in `rust/`. Expected: fails to compile (no `detailed`).
- [ ] **Step 3: Implement.**
  - Thread `Detail` through each fault site. Every site constructs a `Failure` with its honest code, and the 23 lumps split per spec Appendix B.
  - Splitting never changes the winner, its reason or its span. The one exception is `(-$5)`.
  - A textual NaN/infinity spelling gets `not_finite` only where today's catch-all fires, reason and span unchanged.
  - `invalid_utf8` is decided at the winning site, by decoding the bytes there. No new validation pass.
  - Add a unit test `every_detail_round_trips` over `Detail::from_code(code()).`
  - **The coverage check, `every_site_is_reached`:**
    - **The recorder is scoped to this replay, never process-global.** Under `cfg(test)` only, each `Site` firing records into a **thread-local** set, `std::thread_local!` in the test build (the shipped `no_std` core is unchanged). The test **resets** it at the start of its replay and reads it at the end, so a site reached by another unit test running concurrently on another thread can't count.
    - **The replay lives in the instrumented unit-test build:** a `#[cfg(test)]` test inside the crate (`rust/src/tests.rs` or a `cfg(test)` module), reading the corpus files from there. Integration tests under `rust/tests/` compile the library without `cfg(test)` and can't see the recorder.
    - The test replays every corpus file and asserts that every `Site` fired, **except** those listed in `SITES` as `unreachable`, each with its reason. The 8 defensive sites from spec Appendix B belong here.
    - It also asserts that every `Site` appears exactly once in `SITES`, and that the detail each firing produced equals the table's.
    - Then `docs_fault_sites_is_current` regenerates `docs/fault-sites.md` and fails on any difference (`HYPERCAST_BLESS=1` rewrites it, like the header test).
    - **Negative control, with the whole unit-test suite running** (plain `cargo test --lib`, default parallel threads): remove one corpus row that is the only one reaching a site, and `every_site_is_reached` fails naming that site, even though other tests reach it on their own threads. Restore the row and record the failure.
- [ ] **Step 4: Run and see them pass.**
  - `cargo test`: the span sweep and `allocation_free` included.
  - `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings`.
  - The no-panic proof plus canary in the container. (The harness coverage of the new paths is Task 4 Step 4's, once the exports exist.)

### Task 2: Strict grouping

**Files:**
- Modify: `rust/src/verdict.rs` (the constants, `with_grouping`, `NumFormat::STRICT`, `check`/`validate` rules), `rust/src/integer.rs` and `rust/src/real.rs` (the grouping engine at both group sites), **`rust/src/lane.rs` (`scan`'s grouped-input fast path, which runs before both engines at the integer, real and decimal doors)**, `rust/src/ffi.rs` (old exports mask bits ≥ 8 to legacy)
- Test: `corpus/integer.json`, `real.json`, `decimal.json`, `constraints.json`, `formats.json`, `rust/tests/conformance.rs` (`format_of` reads a `"grouping": "legacy" | "western" | "indian"` field; absent means legacy), `rust/src/tests.rs`

**Interfaces:**
- Consumes: Task 1's `Detail` (`GroupSize`, `GroupInFraction`, `GroupMisplaced`) and `ValidNumFormat`.
- Produces:
  - `pub const GROUPING_LEGACY: u32 = 0; GROUPING_WESTERN: u32 = 1 << 8; GROUPING_INDIAN: u32 = 2 << 8; GROUPING_POLICY: u32 = 3 << 8;` on `NumFormat`;
  - `pub const fn with_grouping(self, policy: u32) -> NumFormat`, which **replaces** the field;
  - `pub const STRICT: NumFormat`: invariant separators, `ALL`, `GROUPING_WESTERN`.

- [ ] **Step 1: Write the failing rows and tests.**
  - Western and Indian accept and refuse cases at all three doors and their checked twins: currency separators, native digits, space-like separators, ungrouped numerals.
  - `1234,567` gives `group_size` spanning `1234`; `12,34` gives `,34`.
  - Leading, trailing and doubled separators give `group_misplaced` on the separator.
  - Grouping in the fraction and in the exponent gives `group_in_fraction`.
  - Several invalid groups: the leftmost wins.
  - `1,234` under `SEPARATOR_DETECT` with a strict policy stays ambiguous.
  - Review Focus 1 and 5.
  - `formats.json`: policy `3 << 8` and an undefined bit refused (new rule names `grouping_policy` and `unknown_flag`), each policy accepted.
  - Unit tests: `with_grouping` replaces, never ORs; `STRICT`'s fields; `INVARIANT` stays legacy.
  - **Fast-path agreement:** `lane_and_engine_agree_on_grouping`. For each policy, a set of valid and invalid Western and Indian groupings (`1,234,567`, `12,34,567`, `1234,567`, `12,34`, `,123`, `1,,234`, a fraction group, and native-digit and space-like variants) goes through each door with the lenient lane on and off (the existing `*_engine_only` test hooks). Both paths give the same verdict, detail and span, and the lane accepts strict input only when it validated the groups.
- [ ] **Step 2: Run and see them fail** (`cargo test`).
- [ ] **Step 3: Implement.**
  - The rules as the spec states them: counting digits, not bytes; strict policies don't enable grouping.
  - **`lane::scan` validates the group sizes for the strict policies:** Western three, and Indian three then twos, with the leftmost group's bounds. The bindings now default to strict Western, so falling through would put every grouped token on the slow engine. Any shape the lane can't validate falls through to the engine, which is authoritative. Legacy keeps today's lane behaviour.
  - `check`'s 1.1 rules apply in `validate` and to the detailed exports.
  - **The old exports' `resolve_format` masks `flags` to bits 0–7 before parsing.**
  - The Rust existing doors honour policies `01` and `10`.
  - Docs on each constant and on `check`, `validate` and `with_grouping`.
- [ ] **Step 4: Run and see them pass,** then the Task 1 Step 4 checks.

### Task 3: Date options

**Files:**
- Create: `rust/src/date_options.rs`
- Modify: `rust/src/temporal.rs` (compact date, space separator, hour offset, pivot, month names), `rust/src/detailed.rs` (date doors take `Option<&ValidDateOptions>`), `lib.rs`
- Test: `corpus/date.json`, `timestamp.json`, `date_order.json`, `datetime.json` (rows gain `"options": {"flags": […], "pivot": n, "months": [[month, "name"], …]}`), `rust/tests/conformance.rs`, `rust/src/tests.rs`

**Interfaces:**
- Consumes: Task 1's `Detail` (`NotRecognized`, `FieldWidth`, …).
- Produces:
  - `pub struct DateOptions { … }`, with flags `COMPACT_DATE = 1`, `SPACE_SEPARATOR = 2`, `HOUR_OFFSET = 4`, `pub const COMMON_FORMS: DateOptions` (all three, no pivot), and `with_pivot(u16)`;
  - `pub struct MonthNames<'a>` over the packed buffer (layout per Global Constraints), built by `MonthNames::from_packed(&'a [u8]) -> Result<MonthNames<'a>, DeclarationError>`, and a const-friendly builder for Rust callers;
  - `#[non_exhaustive] pub struct ValidDateOptions<'a>`, built by `DateOptions::validate(self, months: Option<MonthNames<'a>>) -> Result<ValidDateOptions<'a>, DeclarationError>`;
  - the packed-buffer packer `MonthNames::pack(&self, out: &mut [u8]) -> Option<usize>`, the same contract as `pack_enum`.

- [ ] **Step 1: Write the failing rows and tests.**
  - Each option at its door, and the profile at every applicable door.
  - Irrelevant known options ignored; an unknown flag bit refused.
  - Pivots 100, 2029 and 9999. Pivot 99 and pivot 10000 refused.
  - `07-Jan-26` with a pivot; `7-Jan-2026` without one; `26/01/07` without a pivot gives `field_width`.
  - Month names: `janvier`; the `Sep`/`Sept` aliases; folded matches (`JAN`); longest-eligible matching; dotted names with `.` (Review Focus 2); an unknown name gives `not_recognized`.
  - Refused vocabularies, one per spec rule: a cut-short entry, trailing bytes, invalid UTF-8, an empty or 65-byte name, month 0 or 13, a missing month, an ASCII digit, `/`, `-`, white space, a folded duplicate.
  - With no options, every existing row is unchanged.
- [ ] **Step 2: Run and see them fail.**
- [ ] **Step 3: Implement** per the spec's Item 5. The vocabulary is validated in full by `validate`. The detailed date doors take the validated value; the old doors never see options.
- [ ] **Step 4: Run and see them pass,** then the Task 1 Step 4 checks.

### Task 4: The four C exports, `Door::Char`, and the differential and sentinel proofs

**Files:**
- Modify: `rust/src/ffi.rs` (the four exports), `rust/src/abi.rs` (`RawFaultDetail`, `RawDateOptions` with `offset_of!` pins and `SIZE`/`MIN_SIZE`), `rust/src/door.rs` (`Char = 23`, docs: item 6 cleanup 1), `include/hypercast.h` and the Swift copy (regenerated)
- Create: `rust/tests/detailed_abi.rs` (differential plus sentinel tests)
- Modify: `rust/tests/allocation_free.rs`, `.github/scripts/check_exports.py` (only if its pattern misses the new exports), the CLAUDE.md export bullet

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces, with the exact signatures from the spec's "C exports" section:
  - `cast_detailed(door: u32, param: u32, format: *const RawNumFormat, periods: *const RawDayPeriods, date_options: *const RawDateOptions, vocab_ptr: *const u8, vocab_len: usize, ptr: *const u8, len: usize, out: *mut c_void, fault: *mut RawFaultDetail) -> i32`;
  - `cast_detailed_from_f64(door, param, value: f64, out, fault) -> i32`, for the four doors only;
  - `cast_checked_detailed(door, param, format, constraints, enum_ptr, enum_len, ptr, len, out, fault, check: *mut RawCheck) -> i32`;
  - `cast_checked_detailed_from_f64(door, param, constraints, enum_ptr, enum_len, value, out, fault, check) -> i32`.

- [ ] **Step 1: Write the failing tests** in `detailed_abi.rs`.
  - **Differential, over every corpus file:**
    - each text door's old export vs `cast_detailed`, with legacy policy and no date options;
    - the four typed C exports vs `cast_detailed_from_f64`;
    - `cast_checked`/`cast_checked_from_f64` vs their detailed pair.

    Compare defined values field by field: never padding, floats by bits, byte arrays as bytes. Compare fault/check fields only on the codes that write them, and otherwise assert that the sentinels are untouched.
  - **Sentinels:**
    - fault sizes 0, 12, 15, 17 refused, with nothing written;
    - size 32 accepted, with bytes 16–32 untouched;
    - a null fault;
    - an unaligned `out`;
    - poisoned ignored pointers: a dangling `format` at a date door, and a dangling `periods`, `date_options` and vocabulary at `i32`;
    - an unknown door, a door outside the export's set, a nonzero `param` where none is declared, a bad precision, order or epoch code;
    - `RawDateOptions` sizes 4, 10, 16 refused; size 8 read with no pivot;
    - a null vocabulary pointer with a nonzero length refused; a non-null pointer with length 0 treated as no vocabulary;
    - Review Focus 3 and 4.
  - **The checked table:** code 4 writes `detail` 0, and `RawCheck { keyword, adjusted: 0, enum_index: u32::MAX }`.
  - **`Door::Char`:** `cast_detailed` with door 23 matches `cast_char`.
- [ ] **Step 2: Run and see them fail** (no exports yet).
- [ ] **Step 3: Implement.**
  - The exports validate everything first, then dispatch to `detailed`, then write.
  - Every export carries `#[no_panic]` under the feature.
  - Regenerate the headers with `HYPERCAST_BLESS=1 cargo test --test header`.
  - `allocation_free` counts zero for all four exports.
- [ ] **Step 4: No-panic coverage of every new path.** In `rust/src/no_panic_harness.rs`, add a table comment listing which new paths the four `#[no_panic]` exports instantiate:
  - every `detailed` text door at the types the exports call;
  - the four typed C doors;
  - the checked pair;
  - `NumFormat::validate`, `DateOptions::validate` and `MonthNames::from_packed` as the exports call them.

  Then add `#[no_panic]` wrappers for **every Rust-only path the exports don't reach**:
  - the `detailed` typed doors the C ABI lacks (the eight integers, `f32`, `bool`, Unix from `f64`);
  - `MonthNames::pack`, and the Rust-side `MonthNames` builder;
  - `NumFormat::with_grouping`;
  - `Detail::from_code` and `Failure::fault`;
  - any `detailed` door whose generic instantiation no export uses.

  **Panic-injection control in a Rust-only path:** a temporary bare index in `MonthNames::pack` must make `cargo no-panic` fail, naming that wrapper. Remove it and record the failure. The general canary only proves the linker check works.
- [ ] **Step 5: Run and see them pass,** plus `python3 .github/scripts/check_exports.py`, which will fail on every binding until Tasks 5–14 declare the exports. Record which bindings it names; the binding tasks own the fix. Then the Task 1 Step 4 checks.
- [ ] **Step 6: Measure the Rust side,** before and after on a quiet machine, with a scratch base checkout at `446b4f7` and another at the item-2 tree as recorded in the ledger:
  - `cargo bench --bench cast_benchmarks`;
  - a new `rust/benches/detailed_abi.rs` comparing each old export with `cast_detailed` on the same plain and failing inputs;
  - the vocabulary-validation cost.

  Keep the raw outputs for Task 16. (This runs while nothing else does, before Tasks 5–14 fan out.)

### Tasks 5–14: one task per binding (parallel after Task 4)

**Isolation:** each binding task runs in its **own scratch worktree** (`.worktrees/scratch/<binding>`, seeded with the main worktree's state after Task 4), never in the shared checkout. `local-core.sh` runs `local-clean.sh`, which deletes every binding's staged artifacts, and builds write `target/`, `.build/`, the artifact bundle and the staged native libraries. Separate worktrees give each task its own copies of all of them, so one task's build or clean can't remove another's. The controller merges each binding's diff back after its review. **Benchmarks aren't run inside these parallel tasks** (the machine is loaded); each task only records the exact command its harness takes. Task 16 runs them serially on a quiet machine.

Each binding task implements the spec's "Every binding" section, items 3, 4 and 5, in one pass. The shared requirements, which every one of these tasks includes:

- **Grouping:**
  - the constructor takes the policy separately from the leniency flags, defaulting to strict Western;
  - an explicit legacy and Indian option that replaces the policy;
  - `withStyles` (or the equivalent) preserves the policy;
  - the binding's `Invariant` constant is strict Western;
  - validators enforce policy `3 << 8` and undefined bits;
  - **bridges read the platform's actual number pattern**: Indian when it is Indian, strict Western otherwise. Number and currency patterns that disagree trigger the stop condition.
- **Version guard:** the binding's load-time version check rejects a core below 1.1. **A downgrade test** shows a strict request against a 1.0 core is rejected, never parsed legacy (a 1.0 library from the v1.0.0 Release or a build of the tag).
- **Detail:**
  - every detailed failure exposes the raw code and a named value with an "unknown" member;
  - compatible extension or a separate detailed path, per the spec's "Binding compatibility";
  - **a compatibility test** shows 1.0-style constructors, patterns or deconstruction, and equality/hash still compile and behave the same;
  - **an unknown-detail test** decodes a constructed fault carrying an unpublished code.
- **Detailed exports:** doors route through the detailed exports, or the Rust `detailed` doors for the Rust-built bindings. All four new exports are declared at every site `check_exports.py` names.
- **Dates:** a `DateOptions` type with flags, pivot and `COMMON_FORMS`, and a month-vocabulary type that packs the spec's layout, accepted by the binding's date doors.
- **Corpus:** the replayers assert `detail` on every failure row, read the `grouping` and `options` row fields, and replay `formats.json`'s new rules.
- **Benchmarks:** record in the report the command that runs the binding's existing harness (Task 16 runs it serially, before and after).
- **Wording:** messages, docs and README follow, in the binding's existing style.

| Task | Binding | Sites to start from |
|---|---|---|
| 5 | Python | `rust/src/python_ext.rs`, `python/` (`from_localeconv`: `localeconv()['grouping']` `[3, 2, 0]` means Indian) |
| 6 | TypeScript | `typescript/src/lib.rs`, `typescript/tests`, `browser-smoke/main.js`, `binding.js`/`.d.ts` regenerated |
| 7 | Ruby | `ruby/lib/hypercast.rb` (Fiddle), `rust/src/ruby_ext.rs` (Magnus); no platform pattern (`from_i18n` takes caller hashes) |
| 8 | PHP | `rust/src/php_ext/`, `php/` (`fromNumberFormatter`/`fromLocaleconv` patterns), V1 parity divergences for any new 1.0-answer change, the stub check |
| 9 | C# | `csharp/HyperCast/` (`NumberFormatInfo.NumberGroupSizes` / `CurrencyGroupSizes`) |
| 10 | Java | `java/` (FFM and JNI: `DecimalFormat` pattern; JNI `Native` table and `java/jni/src/lib.rs`, its `no_panic` example) |
| 11 | Swift | `swift/Sources/HyperCast/` (`NumberFormatter` grouping size and secondary grouping size) |
| 12 | Go | `go/interop.go`, `go/cast.go`; regenerate the translated core (`go generate`) and its check |
| 13 | R | `r/R/`, `r/src/hypercast.c`, `r/tools/vendor.sh` (also strict by default per the spec); a `detail` column on verdicts per the spec |
| 14 | C and C++ | `include/hypercast.hpp` (the runtime `hypercast_version()` guard, the compile-time version macro from item 6 cleanup 2, detailed wrappers), `c/smoke/` (the checked direct-C example and the downgrade test), `c/smoke/corpus.cpp` |

Steps for each:
- [ ] **Step 1: Write the failing tests** named above, in the binding's suite.
- [ ] **Step 2: Run and see them fail** (`CLAUDE.md` "Local loop").
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run and see them pass,** plus the formatter, lint, and the culture sweep on base and change where a bridge exists. The base build goes in its own scratch checkout, never the shared one.

### Task 15: Item 6, the cleanup items and the backlog

**Files:** as each entry needs. Spec "Item 6" and Appendix A, plus every `minor (deferred)` line item 2's final review records in its ledger.

- [ ] **Step 1: Triage before editing.** Write `.superpowers/sdd/<plan>/backlog-triage.md` with one row per entry and a proposed disposition: fix, superseded (naming the change), defer (with a reason), or **bring back** (it would change public behaviour, expand an API or need a broader refactor). Apply the spec's rule-major ordering contract to the message-order entries (C#, Ruby, PHP). Stop with status NEEDS_CONTEXT if any row is "bring back"; the controller takes those to the owner.
- [ ] **Step 2: The four cleanup items:** the `Door` docs (with `Char`), the C++ version macro if Task 14 didn't land it, the CHANGELOG formatting, and the stale links in `csharp/README.md` and `CRAN_CHECKLIST.md`.
- [ ] **Step 3: Fix every "fix" row.** A test-strength fix gets a negative control that fails without the fix, recorded in the report.
- [ ] **Step 4: Verify.** Run each touched binding's suite and the core suite, then fill in the final disposition of every row in the triage file.

### Task 16: Docs, the CHANGELOG, receipts and the sweep

- [ ] **Step 1: CHANGELOG (Unreleased).**
  - **Under "Changed":** the strict-grouping default, prominently, with `12,34` and the legacy escape hatch; the `(-$5)` span change; the binding `Invariant` constants; and the 1.1 flag checks on the new paths.
  - **Under "Added":** `Detail`, `Failure`, the `detailed` module, the four exports, `RawFaultDetail`, `RawDateOptions`, `DateOptions`/`COMMON_FORMS`, month vocabularies, `NumFormat::STRICT`/`with_grouping`, and `Door::Char`.
  - Every package listed.
- [ ] **Step 2: Docs.** The READMEs, `docs/bindings.md`, the export count (36), and the C header's direct-caller version requirement. README examples are verified by hand.
- [ ] **Step 3: Receipts, run serially on a quiet machine** (check `uptime` and record the load):
  - every binding's harness, using the commands Tasks 5–14 recorded, before (a scratch checkout of the item-2 tree) and after (the merged tree), one binding at a time;
  - Task 4 Step 6's Rust measurements, rerun if any core file changed since.

  Dated rows, machine named, losses printed.
- [ ] **Step 4: The full sweep from a clean tree** (`.github/scripts/local-clean.sh`):
  - **Core:** fmt, the CI clippy set, `cargo test`, the wasm32 suite, the no_std proofs, MSRV 1.89, and no-panic plus canary.
  - **Repo scripts:** `check_exports.py` and `.github/scripts/tests`.
  - **Every binding's suite,** the C/C++ smokes included.

  Report what only CI proves.
