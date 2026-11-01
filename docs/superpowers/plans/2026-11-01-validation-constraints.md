# Validation Constraints (HyperCast 1.1.0) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add OpenAPI-vocabulary constraints (bounds, `multipleOf` with rounding, lengths, `enum`, `trim`) to HyperCast's core, its C ABI, all ten bindings and the C++ header, as an additive 1.1.0.

**Architecture:** A checked door runs the existing door first, then judges the value with exact decimal arithmetic (never binary floating point) against a door-bound declaration. The result is a two-arm verdict: `Checked<T>` (value, `adjusted`, text `enum_index`) or `Rejection` (today's `Fault`, unchanged, or a new `Violation` with a keyword). Four new C exports carry it, and every binding wraps them idiomatically beside its untouched plain doors.

**Tech Stack:** Rust 2024 (`no_std`, no `alloc`, `cargo no-panic`), cbindgen, and each binding's own toolchain (C#, Java FFM and JNI, Swift, Go via wasm2go, Python via PyO3, Ruby via Magnus and Fiddle, PHP via ext-php-rs, TypeScript via napi-rs, R, C and C++).

**Spec:** `docs/superpowers/specs/2026-11-01-validation-constraints-design.md`. Read it before any task: it is the authority on semantics, refusals, layouts and codes. This plan only adds the decisions it leaves to implementation.

**Out of scope:** HyperTabular's implementation (its own spec and plan) and `format` and `pattern`. **Release gate:** HyperCast 1.1.0 is not published until HyperTabular's plan has been carried out against this working tree (Task 19). Any change HyperTabular forces lands here first, so no version number is spent on a fix.

## Global Constraints

- Additive only. `Reason`, `Fault`, every plain door's signature and verdict type, and R's `hypercast_verdict` columns do not change.
- The core stays `no_std`, never names `alloc`, allocates nothing (`rust/tests/allocation_free.rs`) and cannot panic (`cargo no-panic`). Scratch space lives on the stack with fixed bounds.
- No float in any judgement. Bounds and `multipleOf` are `Decimal`. Rounded floats are delivered through `float::parse` over canonical ASCII.
- Exports go from 27 to 31: `cast_checked`, `cast_checked_from_f64`, `cast_constraints_check` and `cast_text`.
- Layouts: `RawConstraints` is 72 bytes and 8-aligned, `RawCheck` 12 bytes and 4-aligned, `RawSpan` 8 bytes and 4-aligned, each pinned by `offset_of!` asserts and carried in the header as `*_SIZE`.
- Return codes: `0` ok, `1` empty, `2` malformed, `3` out of range, `4` violation, `-1` contract violation. `cast_constraints_check` returns `0` or a refusal code `1` to `11`, never `-1`.
- Keyword codes: `1` minimum, `2` exclusiveMinimum, `3` maximum, `4` exclusiveMaximum, `5` multipleOf, `6` minLength, `7` maxLength, `8` enum. `9` and `10` are reserved.
- Rounding codes: `0` strict, `1` midpoint (half to even), `2` floor, `3` ceiling.
- Flags: `HAS_MINIMUM` 1, `HAS_EXCLUSIVE_MINIMUM` 2, `HAS_MAXIMUM` 4, `HAS_EXCLUSIVE_MAXIMUM` 8, `HAS_MULTIPLE_OF` 16, `HAS_MIN_LENGTH` 32, `HAS_MAX_LENGTH` 64, `TRIM` 128.
- `enum_index` is `UINT32_MAX` except on a `Text` success that matched an entry.
- Case folding: `CaseFolding.txt` statuses `C` and `S` only, from Unicode 17.0.0, pinned. Trim: the 25 `White_Space` code points.
- Keywords are spelled the OpenAPI way (`exclusiveMinimum`), in each language's casing convention. Every `Keyword` type stays open to additions.
- **Git:** the owner drives git. Wherever a step would commit, the checkpoint is instead: run the task's tests and leave the work in the tree. Never commit, push or tag.
- Every check must be shown failing under a negative control before it counts (CLAUDE.md).
- Formatters cover src, tests, benchmarks and smoke tests in every language: run each binding's formatter and lint before the task is done.

## Review Focus

1. **Notation under `multipleOf`:** `$12.37`, `(12.35)`, `1.234,55` (declared separators), `5%` and native digits must be judged on the value the door reads (`5%` is `0.05`). Pinned in Task 5 (the corpus notation rows).
2. **Rounding to zero:** `-0.0000004` under `midpoint` with `multipleOf: 0.000001` must deliver positive zero, adjusted, never `-0`. Pinned in Task 2 (`negative_rounds_to_positive_zero`) and Task 5 (corpus).
3. **Already-decoded floats:** `0.1 + 0.2` (`0.30000000000000004`) under `multipleOf: 0.1` is a violation under `strict` and `0.3`, adjusted, under `midpoint`. A negative decoded value keeps its sign, and `-0.0` judges as zero. Pinned in Task 2 (`from_shortest_carries_the_sign`) and Task 5 (corpus `from_f64` rows).
4. **Type extremes:** `minimum: -9223372036854775808` on `I64` is accepted, `maximum: 18446744073709551616` on `U64` is refused (8), and `18446744073709551615` under `ceiling` with `multipleOf: 10` is `OutOfRange`. Pinned in Task 4 (`bounds_at_type_extremes`) and Task 5 (corpus).
5. **Text that is not one code point per character:** `👍🏽` counts 2 code points, `e` plus U+0301 counts 2, and whitespace-only input with `trim` is `Empty`. Pinned in Task 3 (`lengths_count_code_points`) and Task 5 (corpus, trimmed to nothing).

---

## Phase 1: the core (`rust/`)

### Task 1: The door numbering moves into HyperCast

**Files:**
- Create: `rust/src/door.rs`
- Modify: `rust/src/lib.rs` (add `mod door;`, `pub use door::Door;`, crate-doc bullet)

**Interfaces:**
- Produces: `pub enum Door` (`#[non_exhaustive]`, `Clone, Copy, Debug, PartialEq, Eq`), with the same 22 variants, docs, `const fn code(self) -> u32`, `const fn from_code(code: u32, param: u32) -> Option<Door>` and `const fn is_numeric(self) -> bool` as `HyperTabular/rust/src/kernel/door.rs`, copied verbatim (including `Door::Text` = 18).

- [ ] **Step 1: Write the failing tests** in `door.rs`'s `#[cfg(test)] mod tests`:
  - `codes_round_trip`: for every variant (with each param value for 14, 20, 21 and 22), `Door::from_code(d.code(), param) == Some(d)`.
  - `unknown_codes_refused`: `from_code(0, 0)`, `from_code(23, 0)`, `from_code(14, 5)`, `from_code(20, 0)` and `from_code(22, 3)` are all `None`.
  - `numeric_doors`: `is_numeric` is true exactly for codes 2 to 11 and 19.
- [ ] **Step 2: Run** `cargo test --lib door` in `rust/`. Expected: compile failure (no `door` module).
- [ ] **Step 3: Implement `door.rs`** as the copy described above. Point the module doc at the spec's reason: any parser can name a door.
- [ ] **Step 4: Run** `cargo test --lib door`. Expected: PASS.
- [ ] **Step 5: Checkpoint.** Run `cargo fmt --check` and `cargo clippy --all-targets`. Leave the work in the tree.

### Task 2: Exact decimal judgement

**Files:**
- Create: `rust/src/exact.rs`
- Modify: `rust/Cargo.toml` (dev-dependency `bigdecimal = "0.4"`, commented as test-only, like the others), `rust/src/real.rs` and `rust/src/decimal.rs` (one shared routing decision, and `normalize` refactored onto a new streaming `walk`)

**Interfaces:**
- Consumes: `Decimal`, `typed::shortest_digits`, and `Rounding` (define it here, in `exact.rs`, and re-export it from `constraints.rs` in Task 4).
- Produces:
  - `pub(crate) enum Route { Plain, Engine }` and `pub(crate) fn route<'f>(text: &[u8], start: usize, format: &'f NumFormat, resolved: &'f mut NumFormat) -> Result<(Route, &'f NumFormat), Fault>` in `real.rs`. This is the decision the real and decimal doors make today, moved out so the doors and the exact view share it: resolve `SEPARATOR_DETECT` into `resolved` (`format.resolve_detected`), then take `Plain` when `reads_plain(format) && is_plain(text, format)`, which the doors try before the lane and before any currency stripping, so `.5` under a symbol of `.` is one half. Otherwise take `Engine`. The lenient lane stays inside the doors as a fast path for `Engine` tokens: it reads only shapes the engine reads, to the same value (`real.rs`'s lane-parity test holds it to that).
  - Both doors call `route` in place of their own copies of that decision. The plain-door suites and `conformance.rs` must stay green, unchanged.
  - `pub(crate) fn walk(text: &[u8], start: usize, format: &NumFormat, percent_door: bool, emit: &mut impl FnMut(Token) -> bool) -> Result<(), Fault>` in `real.rs`, with `pub(crate) enum Token { Negative, Digit(u8), Point, ExponentSign(u8), ExponentDigit(u8), Percent }`. It is the engine grammar `normalize` reads today (signs, parentheses, currency symbol, separators, native digits, exponent, percent), streamed with no buffer and no length limit. The exponent's sign and digits are streamed as written, leading zeros included, and never collapsed into an integer here. `emit` returning `false` stops the walk early. `normalize` becomes `walk` writing into its 256-byte buffer, keeping its exact output bytes.
  - `pub(crate) struct Exact<'a>`: a cloneable view of an exact decimal that never copies its digits. Its source is either the caller's text under a format (re-walked on demand) or ASCII digits in a buffer. It knows `negative`, the position of the decimal point relative to its first significant digit (leading zeros skipped), and yields significant digits most significant first. Zero is never negative.
  - Constructors: `Exact::from_text(text: &'a [u8], start: usize, format: &'a NumFormat, percent_door: bool) -> Result<Exact<'a>, Fault>`. It calls `route` exactly as the door did. Under `Plain` it reads the caller's trimmed text in place as the plain shape (`[-+]digits[.digits][e[+-]digits]`). Under `Engine` it streams `walk` under the **resolved** format: a first pass finds the sign, the point position, the exponent and percent (as −2), and a second pass yields the digits. The view's **effective position** is the power of ten of its first significant digit: the mantissa's position (from the decimal point and the leading zeros skipped, at most the input's length `L` in magnitude) plus the explicit exponent (and −2 for percent). It is computed in `i64`, never by clamping the exponent alone, because a long mantissa can cancel a long exponent. The explicit exponent accumulates from its streamed digits saturating at ±(`L` + `C`), with `C` = 1,000,000. The sum is then clamped to ±`C`. The proof: if the true exponent is beyond ±(`L` + `C`), the true effective position is beyond ±`C` on the same side as the clamped one (the mantissa moves it by at most `L`), and every value whose effective position is beyond ±`C` judges identically. Below −`C` the value is a nonzero fraction smaller than any step or bound can resolve, since both have a scale of at most 28. Above +`C` no door has accepted it: an `f64` stops at 309 integer digits. `Exact::from_integer(value: i128, buf: &mut [u8; 40])`; `Exact::from_decimal(d: &Decimal, buf: &mut [u8; 40])`; `Exact::from_shortest(negative: bool, digits: Option<&ShortestDigits>) -> Exact`, where `None` is zero and a zero is never negative, so `-0.0` and `0.0` are the same value. `ShortestDigits` carries only the magnitude, which is why the sign is passed in.
  - `pub(crate) fn compare(value: &Exact, bound: &Decimal) -> Ordering` and `pub(crate) fn compare_decimals(a: &Decimal, b: &Decimal) -> Ordering`.
  - `pub(crate) enum Snap { Exact, NotMultiple, Rounded(Rounded) }` from `pub(crate) fn snap(value: &Exact, step: &Decimal, mode: Rounding) -> Snap`.
  - `pub(crate) struct Rounded { buf: [u8; 340], len: u16, exponent: i32, negative: bool }` with `as_exact(&self) -> Exact`, `to_i128(&self) -> Option<i128>`, `to_decimal(&self) -> Option<Decimal>` (`None` past 96 bits or a scale of 28) and `write_canonical(&self, out: &mut [u8; 350]) -> usize` (for `float::parse`). 340 digits suffice: `Rounded` exists only after the door accepted the value, so its integer part is at most 309 digits (an `f64`), its fraction at most the step's 28, plus one for a carry.

- [ ] **Step 1: Write the failing tests** in `exact.rs`:
  - `midpoint_with_an_odd_step` (the spec's counterexample): step 3 and `midpoint`. `1.4` gives `0`, `1.5` gives `0`, `1.6` gives `3`, `-1.4` gives `0`, `-1.5` gives `0` and `-1.6` gives `-3`. All are `Rounded` except where the value is already a multiple.
  - `midpoint_ties_go_to_even_quotients`: step `0.05`. `0.025` gives `0`, `0.075` gives `0.10`, `0.125` gives `0.10`, `-0.075` gives `-0.10`.
  - `floor_and_ceiling_follow_the_number_line`: step `0.05`. `12.37` gives floor `12.35` and ceiling `12.40`. `-12.37` gives floor `-12.40` and ceiling `-12.35`.
  - `strict_reports_not_multiple`: step `0.05`. `12.37` is `NotMultiple`, `12.35` is `Exact`, and so is `12.3500000`.
  - `latitude`: step `0.000001`, `midpoint`. `45.1234567` gives `45.123457`, `45.1234565` gives `45.123456` (tie, even quotient), `89.9999996` gives `90.000000` and `45.123456` is `Exact`.
  - `discarded_digits_far_past_the_step`: step `0.1`, `strict`. `0.1000000000000000000000000000000000001` (40 fractional digits) is `NotMultiple`. Under `floor` it gives `0.1`.
  - `negative_rounds_to_positive_zero`: step `0.000001`, `midpoint`. `-0.0000004` gives `Rounded` with `negative == false` and a zero value.
  - `compare_is_exact`: `90.0000000000000000001` is `Greater` than `90`, `-90` is `Equal` to `-90.000`, and `1e3` (from text) is `Equal` to `1000`.
  - `long_input_past_normalize`: an invariant `0.1` followed by 300 zeros and a `1` (longer than `normalize`'s 256 bytes, read through `from_text`), with step `0.1`: `strict` is `NotMultiple` and `floor` gives `0.1`.
  - `normalize_output_is_pinned`: written **before** the refactor, from today's `normalize`, as fixed expected bytes. `1e+003` gives `1e+003`; `1.234,5` under `,`/`.` gives `1234.5`; `(1,234.50)` gives `-1234.50`; `1E-0000000000000000000000000000000000000000007` keeps every zero; and an exponent of `-` followed by 40 nines is written as given, not overflowed. After the refactor, the same bytes must come out. This is independent of `walk`, so a regression in `walk` that `normalize` inherits still fails.
  - `exponent_saturates`: `1e-99999999999999999999999999` with step `0.01`: `strict` is `NotMultiple`, `floor` gives `0`, and it compares `Greater` than `0` and `Less` than `0.01`.
  - `mantissa_cancels_exponent` (generated at run time, not in the corpus: the inputs are about 1 MB). `"0."` + 1,000,000 zeros + `"1e1000001"`, and `"1"` + 1,000,000 zeros + `"e-1000000"`, through the plain `f64` door and through `Exact::from_text`: the door gives `1.0`, `compare` against `1` is `Equal`, and `snap` with step `1` under `strict` is `Exact`. Also `"0."` + 1,000,000 zeros + `"1e999999"` gives `0.01` from the door and compares `Equal` to `0.01`.
  - `exact_agrees_with_the_door`: for every accepted input in `real.json` (f64 rows), `decimal.json` and `integer.json`, the value of `Exact::from_text` (or `from_integer`) equals the door's own value. Decimals compare with `compare`; floats convert the view through `write_canonical` and `float::parse` and must equal the door's `f64` bit for bit. This is what catches a view that reads differently from its door.
  - `route_precedence`: `.5` on the decimal door under a declared currency symbol `.` gives `0.5` from the door and from `Exact`. `1.234,56` and `12,5` under `SEPARATOR_DETECT` give `1234.56` and `12.5` from both.
  - `from_shortest_carries_the_sign`: `from_shortest(true, shortest_digits(12.37))` with step `0.05` gives floor `-12.40` and ceiling `-12.35`. `from_shortest(true, None)` and `from_shortest(false, None)` are both non-negative zero.
  - `agrees_with_bigdecimal`: 20,000 generated cases from a fixed-seed LCG. Each draws a value of up to 40 digits with a scale up to 30, a step whose magnitude is below 2^96 with a scale up to 28, and a mode. `snap` and `compare` must agree with `bigdecimal` computing `(v / step)` rounded by the mode, times the step.
- [ ] **Step 2: Run** `cargo test --lib exact`. Expected: compile failure.
- [ ] **Step 3: Implement.** The remainder is Horner's method over `v · 10^s` digit by digit, mod `m` (`m < 2^96`, so a `u128` accumulator times 10 plus a digit cannot overflow). The spec's midpoint rule is the algorithm, verbatim. Its inputs are `d = 2r − m`, the first discarded digit and a sticky flag for the rest:

  ```text
  d >= 1                      -> up
  d <= -2                     -> down
  d == 0                      -> f == 0 ? tie : up
  d == -1                     -> first discarded digit > 5, or == 5 with sticky -> up
                                 == 5 without sticky -> tie; < 5 -> down
  tie                         -> the multiple whose quotient is even
  ```

  "Up" and "down" are in magnitude, and the sign is applied after. Floor and ceiling pick by sign: floor goes away from zero when negative and not a multiple, ceiling when positive. `Rounded` is the value truncated at scale `s`, minus `r · 10^-s`, plus one step when rounding up in magnitude. It is built in the stack buffer. A zero result is never negative.
- [ ] **Step 4: Run** `cargo test --lib exact`. Expected: PASS.
- [ ] **Step 5: Negative controls.** Temporarily drop the `d == -1` branch, so it falls through to "down". Confirm `midpoint_with_an_odd_step` and `agrees_with_bigdecimal` fail, then restore it. Make `Exact::from_text` skip `route` and always `walk` the unresolved format; confirm `route_precedence` and `exact_agrees_with_the_door` fail, then restore it. Collapse the exponent to `i32` with wrapping arithmetic; confirm `exponent_saturates` fails, then restore it. Clamp the explicit exponent to ±`C` before adding the mantissa's position; confirm `mantissa_cancels_exponent` fails, then restore it.
- [ ] **Step 6: Checkpoint.** Also run `cargo test --target wasm32-wasip1 --lib exact` (the runner is in CLAUDE.md), since dev-dependencies must build there.

### Task 3: Text: the door, trimming, lengths and folding

**Files:**
- Create: `rust/src/text.rs`, `rust/src/fold_table.rs` (generated), `rust/unicode/CaseFolding-17.0.0.txt` (the Unicode Character Database file, unmodified, with its licence header), `rust/tests/fold_table.rs`
- Modify: `rust/src/lib.rs`, and `CLAUDE.md` ("Never hand-edit" gains `rust/src/fold_table.rs` and its regeneration command)

**Interfaces:**
- Produces:
  - `pub fn cast_text(input: &[u8]) -> Result<&str, Fault>`: invalid UTF-8 is `Malformed`, with the span of the first invalid sequence. Empty input is `Empty`. The text is not trimmed.
  - `pub(crate) fn trim_white_space(text: &str) -> (&str, usize)`: the 25 `White_Space` code points, returning the trimmed text and its byte offset.
  - `pub(crate) fn code_points(text: &str) -> u32`.
  - `pub(crate) fn fold(c: char) -> char`, a binary search of `fold_table::FOLD: [(u32, u32); N]`.
  - `pub(crate) fn eq_folded(a: &[u8], b: &[u8]) -> bool`: both valid UTF-8, compared by code point, without allocating.

- [ ] **Step 1: Write the failing tests:**
  - In `rust/tests/fold_table.rs`, `fold_table_is_current`: regenerate the table from `rust/unicode/CaseFolding-17.0.0.txt` (statuses `C` and `S` only) and compare it with `rust/src/fold_table.rs`. Rewrite the file when `HYPERCAST_BLESS=1`, as `tests/header.rs` does.
  - In `text.rs`:
    - `cast_text_validates_utf8`: `b"ab\xFFc"` is `Malformed` at span `(2, 1)`, `b""` is `Empty` and `b" a "` gives `" a "`.
    - `trim_covers_white_space`: `"\u{00A0}Male\u{3000}"` trims to `"Male"` at offset 2, and `"\u{200B}x"` is unchanged (U+200B isn't `White_Space`).
    - `lengths_count_code_points`: `"👍🏽"` is 2, `"e\u{301}"` is 2, `"ab"` is 2.
    - `folding_is_simple_and_not_turkic`: `eq_folded("MALE", "male")` and `eq_folded("ΣΑΣ", "σας")` hold. `eq_folded("I", "ı")`, `eq_folded("i", "İ")` and `eq_folded("STRASSE", "straße")` do not.
- [ ] **Step 2: Run** `cargo test --test fold_table` and `cargo test --lib text`. Expected: failures (no table, no module).
- [ ] **Step 3: Implement** `text.rs`. Generate the table with `HYPERCAST_BLESS=1 cargo test --test fold_table`. Its header names the source file and the regeneration command.
- [ ] **Step 4: Run** both. Expected: PASS.
- [ ] **Step 5: Negative control.** Edit one row of `fold_table.rs` by hand, confirm `fold_table_is_current` fails, and regenerate.
- [ ] **Step 6: Checkpoint.**

### Task 4: Declarations, refusals and the ABI layouts

**Files:**
- Create: `rust/src/constraints.rs`
- Modify: `rust/src/abi.rs` (`RawConstraints`, `RawCheck`, `RawSpan` and their `offset_of!` asserts), `rust/src/lib.rs` (re-exports)

**Interfaces:**
- Consumes: `Door` (Task 1), `Decimal` (`verdict.rs`).
- Produces:
  - `#[repr(C)] pub struct RawConstraints { pub size: u32, pub flags: u32, pub rounding: u32, pub min_length: u32, pub max_length: u32, pub _reserved: u32, pub lower: Decimal, pub upper: Decimal, pub multiple_of: Decimal }`, with `SIZE: u32 = 72`, `ZERO`, and the flag constants as associated `u32` consts named as in Global Constraints.
  - `#[repr(C)] pub struct RawCheck { pub keyword: u32, pub adjusted: u32, pub enum_index: u32 }` (`SIZE = 12`) and `#[repr(C)] pub struct RawSpan { pub offset: u32, pub len: u32 }` (`SIZE = 8`).
  - `pub enum Rounding { Strict = 0, Midpoint = 1, Floor = 2, Ceiling = 3 }`, `pub enum Bound { Inclusive(Decimal), Exclusive(Decimal) }`, `#[non_exhaustive] pub enum Keyword { Minimum = 1, ExclusiveMinimum, Maximum, ExclusiveMaximum, MultipleOf, MinLength, MaxLength, Enum }` with `const fn code(self) -> u32`.
  - `#[repr(u8)] pub enum Refusal { KeywordNotForDoor = 1, MinimumWithExclusive, MaximumWithExclusive, LowerNotBelowUpper, MultipleOfNotPositive, RoundingWithoutMultipleOf, MinLengthAboveMax, NotHeldByDoor, EnumNotUtf8, DuplicateEnum, MalformedLayout }` with `code()` and `Display`.
  - `pub struct NumericRules { pub lower: Option<Bound>, pub upper: Option<Bound>, pub multiple_of: Option<(Decimal, Rounding)> }` and `pub struct TextRules { pub min_length: Option<u32>, pub max_length: Option<u32>, pub trim: bool }`.
  - Sealed `pub trait Numeric: Copy` for `i8 … u64, f32, f64, Decimal` with `const DOOR: Door`, and `pub struct Text;` as a marker.
  - `pub struct Constraints<'e, T>` (door in the type), built by `Constraints::<T: Numeric>::numeric(rules: NumericRules, entries: &'e [i128]) -> Result<Self, Refusal>` and `Constraints::<Text>::text(rules: TextRules, entries: &'e [&'e str]) -> Result<Self, Refusal>`. The method `raw(&self) -> RawConstraints` gives its ABI form.
  - `pub(crate) enum Entries<'e> { None, Integers(&'e [i128]), Text(&'e [&'e str]), PackedIntegers(&'e [u8]), PackedText(&'e [u8]) }` with `len()`, `integer(i) -> Option<i128>` and `text(i) -> Option<&'e [u8]>` (packed text is a little-endian `u32` length plus bytes; packed integers are 16-byte `Decimal`s at scale 0).
  - `pub fn check_constraints(door: Door, raw: &RawConstraints, entries: &[u8]) -> Result<(), Refusal>` (the full check, duplicates included) and `pub(crate) fn check_without_duplicates(door, raw, entries: &Entries) -> Result<(), Refusal>`, which checked doors run per call. The first is the second plus the duplicate pass.

- [ ] **Step 1: Write the failing tests** in `constraints.rs`:
  - `layouts_are_pinned`: `size_of::<RawConstraints>() == 72`, `align_of == 8`, `offset_of!(lower) == 24`, `(upper) == 40`, `(multiple_of) == 56`; `RawCheck` is 12 bytes, align 4; `RawSpan` is 8 bytes, align 4.
  - `each_refusal_has_its_code`: one table-driven case per `Refusal`, built through `check_constraints` from a `RawConstraints`, asserting the exact variant. The cases: `HAS_MIN_LENGTH` on `I32` gives 1; `HAS_MINIMUM|HAS_EXCLUSIVE_MINIMUM` gives 2; `HAS_MAXIMUM|HAS_EXCLUSIVE_MAXIMUM` gives 3; minimum 5 with maximum 5 gives 4, and so does minimum 5 with exclusiveMaximum 5; `multipleOf` 0 gives 5, and so does −0.05; rounding 1 without `HAS_MULTIPLE_OF` gives 6; min length 3 with max length 2 on `Text` gives 7; minimum −1 on `U8` gives 8, as do maximum 256 on `U8`, minimum −129 on `I8` and minimum 0.5 or `multipleOf` 0.5 on `I32`; packed text entry `[0xFF]` gives 9; `["Male", "MALE"]` gives 10; size 71 gives 11, as do flag bit 256 and `_reserved = 1`.
  - `refusal_applies_to_the_door`: minimum −1 is accepted on `I8` and refused on `U8`. `HAS_MIN_LENGTH` is accepted on `Text` and refused on `F64`. An integer `enum` on `F64` gives 1. Any constraint on `Bool` gives 1.
  - `bounds_at_type_extremes`: minimum `-9223372036854775808` on `I64` is accepted. Maximum `18446744073709551616` on `U64` gives `NotHeldByDoor`.
  - `turkic_pairs_are_not_duplicates`: `["I", "ı"]` and `["i", "İ"]` are accepted, and `["I", "i"]` gives `DuplicateEnum` (Task 3's `eq_folded`).
  - `equal_lengths_allowed`: min length 2 with max length 2 on `Text` is accepted.
  - `float_doors_hold_every_declarable_bound`: maximum `79228162514264337593543950335` (the largest `Decimal`) and minimum of its negative are accepted on `F32`. No `Decimal` exceeds `f32`'s range, so code 8 never fires for a float door on range.
  - `declaration_conversion`: `pub fn decimal_for_declaration(text: &str) -> Result<Decimal, Refusal>` (in `constraints.rs`, invariant notation, used by the corpus replay and mirrored by every binding) turns `"3.5e38"`, `"1e-29"` and `"0.00000000000000000000000000001"` into `NotHeldByDoor`, because they don't fit `Decimal`'s 96 bits or a scale of 28. `"0.000001"` and `"-90"` convert.
- [ ] **Step 2: Run** `cargo test --lib constraints`. Expected: compile failure.
- [ ] **Step 3: Implement** the types and both check functions. Order the checks as `Refusal`'s codes so that the first rule broken is the one returned. Compare bounds with Task 2's `exact::compare_decimals`, and detect duplicates with Task 3's `eq_folded`.
- [ ] **Step 4: Run** `cargo test --lib constraints`. Expected: PASS.
- [ ] **Step 5: Checkpoint**, as in Task 1.

### Task 5: Checked doors in Rust, and the corpus

**Files:**
- Create: `rust/src/checked.rs`, `corpus/constraints.json`
- Modify: `rust/src/lib.rs` (re-exports, crate-doc bullets), `rust/tests/conformance.rs` (replay), `.github/hyper.json` (`r_corpus` gains `constraints.json`)

**Interfaces:**
- Consumes: Tasks 1 to 4.
- Produces:
  - `pub struct Checked<T> { pub value: T, pub adjusted: bool, pub enum_index: Option<u32> }`.
  - `pub enum Rejection { Fault(Fault), Violation(Violation) }`.
  - `pub struct Violation { pub keyword: Keyword, pub offset: u32, pub len: u32 }`.
  - `pub fn cast_checked<T: Numeric>(input: impl AsRef<[u8]>, format: &NumFormat, constraints: &Constraints<'_, T>) -> Result<Checked<T>, Rejection>`.
  - `pub fn checked_from_f64<T: Numeric>(value: f64, constraints: &Constraints<'_, T>) -> Result<Checked<T>, Rejection>`.
  - `pub fn cast_text_checked<'a>(input: &'a [u8], constraints: &Constraints<'a, Text>) -> Result<Checked<&'a str>, Rejection>`.
  - `pub(crate) fn judge_numeric(door: Door, input: &[u8], format: &NumFormat, raw: &RawConstraints, entries: &Entries, out: *mut u8) -> Result<CheckOutcome, Rejection>` and `judge_text(...)`, both shared with Task 6's exports. `CheckOutcome { adjusted: bool, enum_index: Option<u32>, span: (u32, u32) }`.

**Corpus schema.** An array, one object per vector:
- `door`: `i8` … `u64`, `f32`, `f64`, `decimal` or `text`.
- Optionally `format`, as in `integer.json`.
- `constraints`: an object with `minimum`, `exclusiveMinimum`, `maximum`, `exclusiveMaximum` and `multipleOf` (decimal strings), `rounding` (`strict`/`midpoint`/`floor`/`ceiling`), `minLength`, `maxLength`, `trim` and `enum` (strings for text, integers as strings for integer doors).
- Exactly one of: `input` (text); `input_bytes` (an array of byte values, for input that is not valid UTF-8, which a JSON string cannot carry); or `from_f64` (a number, `"-0"`, `"NaN"`, `"Infinity"` or `"-Infinity"`).
- `expect`: `ok`, `empty`, `malformed`, `out_of_range`, `violation` or `refused`.
- On `ok`: `value` (a number for ints and floats, a string for decimals and text), `adjusted` (bool) and `enum_index` (an integer or absent).
- On every failure (`empty`, `malformed`, `out_of_range`, `violation`): `fault: [offset, len]`, always present. It is `[0, 0]` for `empty` and for every `from_f64` row, and a byte span into `input` or `input_bytes` otherwise. A `violation` also has `keyword` (OpenAPI spelling).
- On `refused`: `refusal`, with no input. Only codes 1 to 8 and 10 appear: they are the declarations a binding can express. Code 8 includes a bound or step whose decimal string does not fit `Decimal`, refused by the conversion (`decimal_for_declaration`) before the core is asked.
- **Replay rules:** `input_bytes` rows are replayed by every binding with a byte-input door; a binding without one skips them and names why in its test. Codes 9 (an `enum` entry that is not UTF-8) and 11 (a malformed layout: an unknown size, flag bit or reserved word) cannot be written through any binding's idiomatic API, so they are ABI-only tests (Task 6). Bindings whose string type can hold a lone surrogate (C#, Java, TypeScript, Python) also test that such an entry raises refusal 9.

- [ ] **Step 1: Write `corpus/constraints.json`.** It must hold at least:
  - Every keyword below, at and above each boundary, on an integer door, a float door and `decimal`.
  - The rounding vectors of Task 2 (the odd step, ties, floor and ceiling on negatives, latitude, positive zero).
  - Euro cash: `12.37` with `0.05` under `strict` is a violation of `multipleOf`, span `[0,5]`; `12.35` is ok.
  - `127` on `i8` under `ceiling` with `multipleOf: 5` is `out_of_range`.
  - Every `refused` code 1 to 8 and 10, with the cases from Task 4, including `"maximum": "3.5e38"` on `f32` (8, by conversion).
  - Text: `trim`, U+00A0, `"MALE"` against `["Female","Male","Other"]` giving `"Male"` and index 1, the Turkic pairs, `"   "` with `trim` giving `empty` (`[0,0]`), and `input_bytes: [97, 98, 255, 99]` giving `malformed` `[2,1]`.
  - `from_f64`: `0.30000000000000004` with `0.1`, under `strict` (a violation) and `midpoint` (`0.3`, adjusted); NaN (malformed); `Infinity` (out of range); `45.1234567` under latitude; `-12.37` with `0.05` under `floor` (`-12.4`, adjusted) and `ceiling` (`-12.35`, adjusted); and both signed zeros, `0` and `"-0"`, which are ok under `minimum: 0` (not adjusted) and a violation of `exclusiveMinimum` under `exclusiveMinimum: 0`.
  - Long input: an invariant `f64` input of `0.1`, then 300 zeros, then `1`, with `multipleOf: 0.1`: a violation under `strict` (span covering the whole input) and ok with value `0.1`, adjusted, under `floor`.
  - Notation (Review Focus 1): `$12.37` with a declared `$` under `strict` `0.05` is a violation; `5%` on `f64` with `multipleOf: 0.01` is ok, with value `0.05`; `(12.35)` is ok, with value `-12.35`.
  - Routing (each agrees with the plain door's value): `.5` on `decimal` and `f64` under a declared currency symbol `.` with `multipleOf: 0.5` is ok, value `0.5`, not adjusted. Under `SEPARATOR_DETECT`, `1.234,56` with `multipleOf: 0.01` is ok, value `1234.56`; `12,5` with `multipleOf: 0.5` is ok, value `12.5`; and `12,37` with `multipleOf: 0.05` is a violation of `multipleOf` under `strict`.
  - Type extremes (Review Focus 4).
  - Integer `enum`: `20` against `["10","20"]` is ok with no `enum_index`; `30` is a violation of `enum`.
  - Judgement order: `30` on `i32` with `maximum: 25` and `enum: ["10","20"]` is a violation of `maximum`, not `enum`.
  - Trimmed to nothing: `"   "` on `text` with `trim` is `empty` (Review Focus 5).
- [ ] **Step 2: Write the failing replay** in `rust/tests/conformance.rs`: `fn constraints()` replays every vector through the public Rust API and asserts every field above, the `fault` span on every failure included. Refusals convert their decimal strings with `decimal_for_declaration` (a conversion refusal is the row's answer), then go through `check_constraints`, using the vector's door and the packed form of its entries.
- [ ] **Step 3: Run** `cargo test --test conformance constraints`. Expected: compile failure.
- [ ] **Step 4: Implement `checked.rs`.** It follows the spec's "Order of judgement" exactly. The plain door runs first. For floats and `decimal`, the `Exact` view is then `Exact::from_text` over the trimmed text, which streams and so reads every input the door accepted, however long (never `normalize`, which stops at 256 bytes). A rounded float goes through `Rounded::write_canonical` and `float::parse::<T>`, and an infinity is `OutOfRange`. A violation's span is the door's trimmed span (for text, after `TRIM`). `checked_from_f64` builds its view with `Exact::from_shortest(value.is_sign_negative(), shortest_digits(value).as_ref())`. NaN is `Malformed` and an infinity is `OutOfRange`, both with a zero span.
- [ ] **Step 5: Run** `cargo test`. Expected: PASS, the whole suite included.
- [ ] **Step 6: Negative controls.** Flip one corpus vector's expected `adjusted` and confirm the replay fails, then restore it. Move the `enum` judgement ahead of the bounds and confirm the order vector fails (`30` on `i32` with `maximum: 25` and `enum: ["10","20"]` expects a violation of `maximum`), then restore it.
- [ ] **Step 7: Checkpoint.**

### Task 6: The four C exports, the header and the proofs

**Files:**
- Modify: `rust/src/ffi.rs` (four exports and the module doc), `rust/Cargo.toml` (the `exports` feature comment: 30 `cast_*` symbols plus `hypercast_version`), `include/hypercast.h` and the Swift bundle's copy (regenerated), `rust/tests/allocation_free.rs`, `rust/tests/fault_span_invariant.rs`, `rust/cbindgen.toml` (only if the new consts need listing), `.github/scripts/cdylib_smoke.py`, `c/smoke/main.c`
- Test: the files above

**Interfaces:**
- Consumes: `judge_numeric`, `judge_text`, `check_constraints` and `cast_text` (Tasks 3 to 5).
- Produces the four C signatures exactly as the spec's "Exports" block gives them. `out` is written only on success. `fault` is written on codes 1 to 4. `check` (nullable) is written on 0 and 4: `keyword` on 4, `adjusted` and `enum_index` on 0. A null `constraints`, a `size` of 0 or an unknown door is `-1`. `cast_checked` with door 18 writes a `RawSpan`, which for an `enum` match is the matched input.

- [ ] **Step 1: Write the failing tests:**
  - `allocation_free.rs`: every new export over a sample of the constraints corpus counts zero allocations.
  - `fault_span_invariant.rs`: every truncation of every constraints-corpus input, through `cast_checked` and `cast_text`, satisfies `offset + len <= input.len()` on codes 1 to 4.
  - `ffi.rs` tests: `checked_reports_through_raw_check` (a latitude vector gives code 0, `adjusted == 1`, `enum_index == u32::MAX`), `violation_is_code_4` (`91` gives `keyword == 3`), `constraints_check_codes` (one call per refusal code, including the ABI-only 9, a packed text entry `[1,0,0,0,0xFF]`, and 11, with size 71, flag bit 256 and `_reserved = 1` each), and `null_and_zero_size_are_contract_violations`.
  - `c/smoke/main.c`: call all four exports once, with a correct expectation each.
- [ ] **Step 2: Run** `cargo test`. Expected: failures (the exports are missing).
- [ ] **Step 3: Implement the exports** with `#[unsafe(no_mangle)]` and `#[cfg_attr(feature = "no-panic", no_panic::no_panic)]`, in the existing `ffi.rs` style. Regenerate the header with `HYPERCAST_BLESS=1 cargo test --test header`.
- [ ] **Step 4: Run** `cargo test`, then `cargo no-panic` in a Linux container (GNU ld), then `cargo cdylib` and `python3 .github/scripts/cdylib_smoke.py`. Expected: PASS. `no-panic` names all four.
- [ ] **Step 5: Run** `python3 .github/scripts/check_exports.py`. Expected: FAIL, listing the four exports missing from every binding site. That is the to-do list for Phases 2 and 3, and it must be green at the end of Phase 3.
- [ ] **Step 6: Negative control.** Insert an `unwrap()` reachable from `cast_checked`, confirm `cargo no-panic` fails, and remove it.
- [ ] **Step 7: Checkpoint.**

### Task 7: Benchmarks

**Files:**
- Modify: `rust/benches/cast_benchmarks.rs`, `docs/benchmarks.md`

- [ ] **Step 1: Add criterion cases:** `cast_f64` against `cast_checked::<f64>` on `45.1234567` (latitude, `midpoint`); `cast_decimal` against `cast_checked::<Decimal>` on `12.37` (`strict`, `0.05`); and `cast_text` against `cast_text_checked` with a 3-entry `enum` and `trim`.
- [ ] **Step 2: Run** `cargo bench --bench cast_benchmarks -- checked` and record a dated receipt in `docs/benchmarks.md` (machine, date, figures), with any loss printed beside the wins.
- [ ] **Step 3: Checkpoint.**

---

## Phase 2 and 3: the bindings

Every binding task has the same deliverable, in its own idiom. The spec's "Bindings" section names each binding's shape. Each task:

1. **Wires the four exports** into the binding's native layer. Its site in `check_exports.py` must then be clean.
2. **Adds a door-bound `Constraints`** whose constructor calls `cast_constraints_check` and raises the binding's existing contract-violation error with a message worded from the refusal code (one message per code, shared within the binding). Bounds and `multipleOf` accept the language's exact decimal, or a string, never a float.
3. **Adds the checked verdict** (`Checked<T>` or `Rejection` = `Fault` | `Violation`) and an open `Keyword`.
4. **Adds constrained overloads** for the integer, float and decimal doors (text and `from_f64` forms), plus `Text` and constrained `Text`. A text `enum` match returns the caller's own declared string, by index.
5. **Remaps violation spans** to the binding's caller units where its fault spans already are.
6. **Replays `corpus/constraints.json` in full**, refusals and `from_f64` rows included, in the binding's corpus test.
7. **Pins the plain shapes** with a test that fails if the plain verdict or fault type changed shape (for R, the exact `hypercast_verdict` column names and order).
8. **Documents it** in the binding's README: the `Constraints` example (latitude, euro cash, sex code), verified by hand, the keyword match needing a default arm, and the export count.

Steps for every binding task:

- [ ] **Step 1:** Add the constraints replay and the plain-shape pin to the corpus test. Run the suite and expect it to fail.
- [ ] **Step 2:** Implement points 1 to 5.
- [ ] **Step 3:** Run the suite, lint and formatter with the command in the table. Expect PASS.
- [ ] **Step 4: Negative control.** Change the binding's `midpoint` mapping to send code 2 (floor), confirm the replay fails, and restore it.
- [ ] **Step 5:** Update the README, then checkpoint.

| Task | Binding | Native layer | Types and doors | Corpus test | Local run (CLAUDE.md "Local loop") |
| --- | --- | --- | --- | --- | --- |
| 8 | C# | `csharp/HyperCast/*.cs`: all three `LibraryImport` declarations | `Constraints<T>`, `Checked<T>`, `Rejection`, `Violation`, `Keyword` (a readonly struct with static members), `Rounding`; `Cast.*` overloads | `csharp/HyperCast.Tests/CorpusTests.cs` | `cargo cdylib`, then `dotnet test` (both TFMs), and the AOT smoke test |
| 9 | Java | both `NativeCore` trees (FFM in `src/main/java25/`, JNI base), `java/jni/src/lib.rs` (extern block and `RegisterNatives` table), `java/jni/examples/no_panic.rs` | `Constraints` (a factory per door), sealed `CheckedVerdict<T>` and `Rejection` | `java/src/test/java/io/github/skunkwerkx/hypercast/CorpusTest.java` | `cargo build --release --locked --lib` in `java/jni/`, then `./gradlew test testFloor checkSuiteParity javadoc` |
| 10 | Swift | `swift/Sources/HyperCast/Cast.swift` over the bundled header | `Constraints<T>`, enum `CheckedVerdict<T>`, `Rejection`; `Keyword` as a `RawRepresentable` struct | `swift/Tests/HyperCastTests/CorpusTests.swift` | `.github/scripts/local-core.sh`, then `swift test` |
| 11 | Go | `go generate` (regenerated `go/internal/core/core.go`), `go/core.go` | `Constraints[V]`, `Checked[V]`, `Rejection` | `go/corpus_test.go` | `go test ./...`, the regeneration check, the TinyGo and `GOOS=js`/`wasip1` legs |
| 12 | Python | `rust/src/python_ext.rs`, `python/src/hypercast/_native.pyi`, `__init__.py` | frozen dataclasses `Constraints`, `Checked`, `Violation`; `Rejection` as `Fault \| Violation` | `python/tests/test_corpus.py` | maturin develop in `python/`, then `pytest` |
| 13 | Ruby | `rust/src/ruby_ext.rs` (Magnus), `ruby/lib/hypercast/runtime.rb` (Fiddle), `ruby/lib/hypercast.rb` | `HyperCast::Constraints`, `Checked`, `Violation` | `ruby/spec/corpus_spec.rb`, on both backends | `cargo ruby-ext`, then `rake spec` for each backend |
| 14 | PHP | `rust/src/php_ext/` (every body starts with `guard(`), the regenerated `php/stubs/hypercast.stub.php` | readonly classes `Constraints`, `Checked`, `Violation`; `Rounding` and `Keyword` enums | `php/tests/CorpusTest.php` | `cargo php-ext`, then `HYPERCAST_CHECK_STUBS=1 composer test` |
| 15 | TypeScript | `typescript/src/*.rs` (every `#[napi]` carries `catch_unwind`), regenerated `binding.js`/`binding.d.ts` | `Constraints` object type, `{ ok, value, adjusted, enumIndex }` or `{ ok: false, fault }` / `{ ok: false, violation }` | `typescript/tests/corpus.test.ts` | `npm ci && npm run build && npm test`, `npm run test:bun`, `npm run lint`, `npm run build:wasi` and `NAPI_RS_FORCE_WASI=error npm test` |
| 16 | R | `r/src/hypercast.c`, `r/R/doors.R`, `r/R/verdict.R` | `hypercast_constraints()`, `hypercast_checked_verdict` with `faults()`, `optional()` and `violations()` methods; `hypercast_verdict` untouched | `r/tests/testthat/test-corpus.R` | `r/tools/vendor.sh`, then `devtools::test("r")` and `R CMD check --as-cran` |
| 17 | C and C++ | `include/hypercast.hpp` (hand-written; `::`-qualified calls), `c/smoke/corpus.cpp`, `c/smoke/main.c` (Task 6) | `noexcept` `hypercast::Checked<T>`, `Rejection`, a `Constraints` factory returning a `Verdict`-like result | `c/smoke/corpus.cpp` | the `c_package.py assemble` command in CLAUDE.md, then `SMOKE_CXX=1 c/smoke/run.sh` |

Notes per binding that the table can't hold:
- **Java:** a `Constraints` value built for one door and passed to another is `IllegalArgumentException`, the binding's contract-violation error. Every new JNI entry point carries `no_panic` (`jni_no_panic` enforces it).
- **TypeScript and R:** dynamic types, so the door is a constructor argument the value keeps, and a mismatch raises the binding's contract-violation error.
- **Ruby and Python:** fault and violation spans count code points. R counts `substr()` units. C# and Java count chars.
- **Go:** after `go generate`, run `HYPERCAST_CHECK_CORE=1 go test -count=1 -run 'TestCoreIsCurrent|TestRegenerationCarriesTheCrateVersion' .`.

## Phase 4: close-out

### Task 18: Lists, docs and the whole-repo checks

**Files:**
- Modify: `.github/scripts/check_exports.py` only if a site's regex can't see a new export, `.github/scripts/tests/test_check_exports.py` if it pins counts, the READMEs (root and every binding: the export count of 31 and the constraints section), `CHANGELOG.md` (one `### Added` entry under Unreleased, listing every package touched), `docs/roadmap.md` (the sub-project order: `format`, then the `pattern` spike), `CLAUDE.md` (the "Lists that move together" exports bullet gains `cast_constraints_check`'s binding-side constructors; the fold table under "Never hand-edit", from Task 3)

- [ ] **Step 1: Run** `python3 .github/scripts/check_exports.py` and `python3 -m pytest .github/scripts/tests`. Expected: PASS. Show the negative control by removing one binding's `cast_text` declaration: it must fail.
- [ ] **Step 2: Run** `.github/scripts/local-clean.sh`, then every binding's local run from the table in one sweep, from a clean tree.
- [ ] **Step 3:** Hand-verify every README example.
- [ ] **Step 4:** Report to the owner: what ran locally, what only CI can prove (the Android, iOS, Blazor, s390x and Windows PHP legs), and the working-tree diff summary. Leave everything uncommitted.

### Task 19: Release gate: HyperTabular consumes the unreleased core

**Files:** none in this repo unless HyperTabular forces a change. In that case the change goes back into Phases 1 to 3 here, with its tests, before anything is published.

- [ ] **Step 1:** Write HyperTabular's own spec and plan from this spec's "HyperTabular" section (brainstorming, then writing-plans, in the HyperTabular repo).
- [ ] **Step 2:** Carry out that plan against this checkout, never the registry:
  - Rust: `[patch.crates-io] hypercast = { path = "../HyperCast/rust" }` in HyperTabular's workspace, uncommitted, and removed before its release.
  - Bindings: local builds of each HyperCast binding (CLAUDE.md "Local loop"), referenced locally the way each HyperTabular binding can (a NuGet local feed, `mavenLocal()`, a Swift path dependency, a Go `replace`, editable Python, a Ruby path gem, a PHP local extension build, npm `file:` and an R local install).
- [ ] **Step 3:** For every friction HyperTabular hits in HyperCast's API or ABI (a missing accessor, a layout it needs to restate, a Rust signature that won't compose with `Batch`), fix it here, re-run the affected HyperCast tasks' tests, and record it in HyperCast's CHANGELOG entry.
- [ ] **Step 4:** Only when HyperTabular's suite is green on every binding against the working tree is HyperCast ready for `prepare-release.yml`. That release is the owner's step.

