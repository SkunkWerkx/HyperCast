# HyperCast & HyperTabular: 1.1 Release and CRAN Submission Gate

Oct 31, 2026 · @Buvy

_Status 2026-11-05: HyperCast's rows 3, 7, 8 and 10 are in the core, the corpus and the bindings, proved by the full-board run [42152914672](https://github.com/SkunkWerkx/HyperCast/actions/runs/42152914672) at commit `9fbe8b2`, the companion pin in HyperTabular's `.github/hyper.json`. Its R type table is [`r/README.md`, Values](../r/README.md#values). The HyperTabular rows and contracts are not closed, and the R and packaging gates below stay open until the submitted tarballs are checked. Each box names its repo: **HyperCast**, **HyperTabular** or **both**. Ticked boxes carry their evidence in italics._

All twelve features below remain required for 1.1, with binding parity. This is our release gate, not a claim that CRAN requires feature parity with readr or readxl. CRAN acceptance also requires safe R integration and verification of the submitted packages.

HyperUuid's shared binding, build, packaging and review improvements are carried into both packages as each box below says; CRAN's feedback on HyperUuid is a gate of its own (see CRAN gates). This document records the additional work specific to HyperCast and HyperTabular. The assessment behind it was a source review, not a new test run or an R CMD check; unchecked items require implementation and evidence before they can be closed.

## Parity rule and order

**The rule:** a feature is done when its shared behavior is implemented once, the corpus pins it, and every binding exposes it in its own idiom. Parsing belongs in the core; allocation, host representations and presentation belong in the bindings. A binding that can't follow says why, in its README.

**Bindings in scope:** Rust, C#, Java, Go, Swift, Python, Ruby, PHP, TypeScript, R, C and C++. TypeScript, R, C and C++ follow the same rule as the rest.

**Order:**

1. Fix quote handling and unsafe text handling; settle strict grouping defaults and uncached-formula semantics. Nothing ships with known silent-data errors.
2. Settle exact host representations, diagnostics, options and ABI evolution before implementing their bindings.
3. Complete all twelve features and the reader contracts below, with corpus and binding parity.
4. Verify the panic boundary, R resource lifecycle and exact submission tarballs.
5. Release 1.1. Submit HyperCast first; submit HyperTabular after its declared R dependencies are available on CRAN.

## Core features, with binding parity

Quote handling is a correctness fix. Safe text conversion, explicit numeric interpretation and distinguishing uncached formulas are submission priorities. Strict rejection of ragged rows is a valid existing policy; permissive ragged reads and the remaining format conveniences are adoption requirements we choose to include in 1.1.

| # | Feature | Repo | Core change | Every binding exposes | R shape |
| --- | --- | --- | --- | --- | --- |
| 1 | Stray `"` stays literal | HyperTabular | A quote is a quote only at a cell's start or inside an open quoted cell; odd-count corpus vectors | Nothing new: same reads, right answers | Same |
| 2 | Ragged rows | HyperTabular | Declared policy: reject or pad short rows; reject, report or explicitly truncate long rows; retain expected/actual widths | Ragged policies on the dialect | `fill =` plus explicit long-row policy |
| 3 | Strict grouping | HyperCast | Shared validation for integer, float and decimal; declared Western (3) or Indian (2+3) grouping | Strict flag and grouping pattern on each `NumFormat` | Strict by default, with declared separators/pattern |
| 4 | Skip, header row, range | HyperTabular | Leading rows to skip and a header row index, delimited and sheets; a cell range for sheets | Options on the dialect and `SheetOptions` | `skip =`, `range = "A5:F200"` |
| 5 | Missing-value tokens | HyperTabular | A declared token list read as `Empty` | A tokens option per dialect and per column | `na = c("", "NA")` |
| 6 | Declared encoding | HyperTabular | Validate UTF-8; decode declared Windows-1252, Latin-1 and UTF-16LE before tokenization using caller-owned buffers | Encoding and malformed-input policy | `encoding =`; no invalid bytes marked UTF-8 |
| 7 | Fault detail | HyperCast | A detail code beside the reason, through new exports so the 1.0 ABI holds | A `detail` on every fault, in each binding's union | A `detail` column in `problems()` |
| 8 | Date and time shapes | HyperCast | `20260107`; two-digit years with a declared pivot; declared month-name vocabulary; space-separated timestamps and `+00` offsets | The new options on the date doors | Arguments on the date casts |
| 9 | Uncached formulas | HyperTabular | A signal distinct from a blank cell | A way to tell the two apart | A `problems()` row |
| 10 | Trim no-break space | HyperCast | Trim U+00A0 at the edges | Nothing new | Same |
| 11 | Problems summary | HyperTabular | Preserve verdict detail and source coordinates; shared iteration or caller-owned output, without an allocated list in the kernel | A batch-level faults view/list beside per-cell verdicts | `problems()` |
| 12 | Plan suggester | HyperTabular | Bounded sampling and shared suggestion logic; explicit unresolved/ambiguous results; never applied automatically | A `suggest` call returning a plan to inspect and pass back | `suggest_plan()` printing a `col_types` spec |

Each row lands as one coordinated change: core, corpus vectors, then every binding, as the char door did.

### Core acceptance details

- [ ] **HyperTabular, quotes:** scanner and cell extraction agree on when a quote is structural. Test every scanner engine, buffer/block splits, odd stray quotes, escaped quotes and multiline records. No record may disappear or merge silently.
- [x] **HyperCast, grouping:** strict validation does not infer locale. `1,234` remains ambiguous without declared separators. Test grouping across numeric doors, currency separators and native digits; preserve the documented legacy mode.
  _Strict Western grouping is the R default and `grouping = "indian"` is declared; `12,34` is `malformed` with detail `group_size` ([`r/README.md`](../r/README.md)); `corpus/` pins it and the run above is green._
- [ ] **both, text:** define BOM handling, malformed sequences, truncated UTF-16 code units and embedded NUL behavior. Never silently replace invalid input. Validate headers as well as values before making host strings.
- [ ] **HyperTabular, input ordering:** define decoding, skip/header selection, tokenization/unescaping, missing-token matching and casting order. Specify physical lines versus logical records, per-column token overrides and whether quoted tokens match.
- [ ] **HyperTabular, formula state:** preserve uncached formulas separately from blanks and cached formula errors in each supported workbook format. Formula evaluation is outside scope.
- [x] **HyperCast, trimming and dates:** preserve original byte offsets after NBSP trimming and the char door's single-whitespace behavior. Pin pivots, month vocabularies and new timestamp forms in the corpus.
  _`r/README.md` documents the `pivot` for two-digit years, the month vocabulary and U+00A0 as `White_Space` trimmed at both ends, with `cast_char()` given exactly one character keeping it; `corpus/date.json` and `corpus/char.json` pin them._
- [ ] **both, ABI:** preserve 1.0 exports and layouts. Version or extend the affected fault, format, dialect, column and workbook interfaces explicitly; do not append fields to an existing C layout in place. Specify old-caller defaults and unknown-option handling. Review Rust API compatibility too.

## R and TypeScript bindings

R users will compare this with readr, vroom, `data.table::fread` and readxl, so it takes their shapes and sells the verdicts, not raw speed.

**Shape**

- A `col_types`-style plan.
- Faults in a `problems()` table with defined source coordinates, reason, detail and span semantics.
- A plan suggester that prints a plan for the user to pass back, never applied silently.
- Vectorised casts: character in, typed vector out, `NA` for a fault, with diagnostics retained. Standalone casts and table columns use the same conversion rules.

**Type decisions for both (settle before any code)**

| Value | R | TypeScript | Decision to make |
| --- | --- | --- | --- |
| i32 | `integer`, whose `NA` is `INT_MIN` | `number`, exact | R: preserve the full range as double, or report the sentinel collision as a binding conversion fault; do not narrow the core's i32 domain |
| i64 | no native exact scalar type | `bigint` | R: exact character/class output or `bit64::integer64` with an explicit missing-sentinel policy; lossy double conversion is opt-in |
| u64 | no native exact scalar type | `bigint` | R: exact character/class output; signed `bit64::integer64` cannot cover u64 |
| u32 | double preserves the full range | `number`, exact | R: double; avoid integer overflow and sentinel collisions |
| decimal | exact character/class output | exact string/class output | Preserve the core's exact decimal value; numeric conversion is explicit and documents loss |
| timestamp | exact representation plus optional `POSIXct` conversion | `{seconds, nanos}`; optional `Date`/`Temporal` conversion | Define rounding, range and precision loss; double seconds do not guarantee microsecond precision across the full domain |
| civil datetime | date plus exact time-of-day representation | civil fields or suitable `Temporal` representation | No implicit UTC/local zone; conversion to an instant requires a zone and daylight-saving ambiguity/gap policy |
| time of day | exact representation plus optional `hms`/`difftime` conversion | nanoseconds as `bigint` or a pair | Document any precision loss in convenience conversions |
| date | `Date` | no date-only type | TypeScript: `{year, month, day}` or `Temporal.PlainDate` |
| duration | exact representation plus optional `difftime` conversion | `{seconds, nanos}` | Preserve sign and nanos; convenience conversion is explicit |
| UUID | `character` | `string` | Direct |
| char | `character` except U+0000 | `string` | R: U+0000 is `ok` with `NA` as its `value`, plus a `code_point` column (HyperTabular D7); never silently dropped. D8 applies the same rule to text R can't hold, with `raw = TRUE` for the exact bytes |

- [x] **HyperCast, representation decisions recorded:** select concrete classes/return types, constructors, missing-value representation and conversion behavior before implementing these APIs. Exact output must remain available for every supported core value, or the binding must explicitly report its representation limitation.
  _HyperCast's type table is [`r/README.md`, Values](../r/README.md#values), one R type per door with exact columns beside it. The U+0000 row waits on D7 and D8, which are their own box below._
- [ ] **HyperTabular, representation decisions recorded:** table columns use the same classes, missing-value representation and conversion rules as HyperCast's standalone casts, and a table cell and a standalone cast agree.
- [ ] **both, D7 and D8:** `cast_char()` gains a `code_point` column and keeps U+0000 `ok` with `NA` as its `value`; text R can't hold (an embedded NUL, or over `INT_MAX` bytes) is `ok` with `NA` on `cast_text()` and `col_text()`, with `raw = TRUE` for a list of raw vectors; HyperTabular's `problems()` adds a `host_conversion` row. `r/README.md` explains that `ok` alone does not guarantee a usable character value.
- [ ] **both, missingness:** distinguish input `NA`, declared missing tokens, blanks, ragged-row padding, malformed values and uncached formulas. Ordinary declared missingness is not a parse problem; preserve provenance where callers need to distinguish the cases.
- [ ] **both, conversion faults:** host representation failures are distinguishable from parser failures. They must not be reported as if the core rejected a valid value.

**Cost:** eager R text conversion creates/interns strings and may dominate a read. Measure before promising speed; ALTREP is a possible later optimization, not a submission requirement.

## HyperTabular reader and diagnostic contracts

- [ ] **HyperTabular, coordinates:** define source row versus output row, source column versus projected column, sheet identity and index bases. Preserve coordinates through skip, header selection, ranges, blank-row filtering and batches. Existing `Batch::line()` supplies source lines/rows but does not settle the whole contract.
- [ ] **HyperTabular, spans:** identify whether a span indexes original file bytes, decoded cell bytes or unescaped text. Decoding and unescaping change offsets; retain mappings if original-file spans are promised. Define unavailable spans for typed workbook cells and host conversion faults.
- [ ] **HyperTabular, ownership:** returned R vectors and diagnostics remain valid after the next batch, reader close and garbage collection. Never expose reused buffer storage without a lifetime mechanism that makes it safe.
- [ ] **HyperTabular, cleanup:** define explicit close, finalization and use-after-close behavior. Release files and native buffers on success, errors and interrupts; cleanup must be safe when called more than once.
- [ ] **HyperTabular, interrupts:** check at bounded safe points during long reads and conversions. HyperUuid's stack/R-owned-buffer assumptions do not cover growing arenas or file handles. Keep R error/interrupt long jumps out of live Rust frames and arrange cleanup for native resources.
- [ ] **HyperTabular, limits:** check batch sizes, allocation arithmetic, core span limits and growth requests before allocation. Define oversized-record/workbook errors and cancellation behavior; no unchecked narrowing of lengths or coordinates.
- [ ] **HyperTabular, headers:** define empty/duplicate name handling and preserve original names alongside any repaired names. Named plans must not silently select an unintended duplicate.

- [ ] **HyperTabular, HyperCast rows 3, 7 and 8 reach it:** strict grouping and the `NumFormat` pattern, the fault `detail` and the new date shapes are exposed by HyperTabular's plan columns, `problems()` and corpus, in every binding, not only in HyperCast's own casts. Row 10 (NBSP trimming) needs no surface.

## CRAN gates

These gates are additional to the shared HyperUuid packaging work. CRAN review remains discretionary; passing this list is our submission criterion, not a guarantee of acceptance.

- [x] **both, R dependency architecture:** record whether HyperTabular imports HyperCast's R functions/classes, uses a registered native API, or vendors its Rust crate independently. Declare actual R dependencies and compatible versions; preserve crate attribution either way.
  _HyperTabular's `r/DESCRIPTION` has `Imports: hypercast (>= 1.1.0)`, and `r/tools/vendor.sh` vendors the core and the `hypercast` crate into `src/rust/vendor.tar.xz`, with `inst/AUTHORS` generated from it._
- [ ] **both, submission order:** HyperCast goes first. HyperTabular declares an R dependency on it, so HyperCast must be available on CRAN before HyperTabular's submission; budget the extra review cycle.
- [ ] **both, lockstep.** Every HyperCast update is checked against HyperTabular, and a break blocks the update until both are fixed. The Rust lockstep already works this way; the R side needs the guard and the reverse-dependency step below.
- [ ] **HyperCast, R-level compatibility guard:** a check that HyperTabular's R package still loads and passes against the HyperCast it imports, and an explicit failure when the versions disagree.
- [ ] **both, reverse dependency:** a scripted step that checks hypertabular against a candidate hypercast, run before any hypercast submission.
- [ ] **both, HyperUuid's feedback is a gate.** Carry every reviewer comment into both packages before submission, or record that CRAN said there are none. The owner's answer decides how this closes. The shared checklist is [HyperUuid/docs/CRAN_CHECKLIST.md](../../HyperUuid/docs/CRAN_CHECKLIST.md).
- [ ] **both, tarball size and fixtures:** measure the built archive against CRAN's 5 MB guideline and the 10 MB ceiling. Keep a deliberate fixture manifest covering all shipped features, with `r_corpus` extended by the new fixtures; a missing required fixture fails the tests and is not skipped. Verify the actual R dependency/vendor tree.
- [ ] **both, panic boundary:** rerun the no-panic proof for affected exports, including the new ones, and record its scope and toolchain. The current `no_std` artifacts abort on panic; `catch_unwind` cannot catch an abort. A C shim over proven exports may retain HyperUuid's architecture. Do not claim the kernel proof covers the whole wrapper. Inspect the actual R shared library and explain relevant compiled-code findings in `cran-comments.md`.
- [ ] **both, `rchk`:** run on the built packages; record the result in `cran-comments.md`.
- [ ] **both, `gctorture`:** the R lifecycle tests run under `gctorture`.
- [ ] **both, valgrind:** run the tests and examples under valgrind.
- [ ] **both, UBSAN and ASAN:** build and run the tests with the sanitizers.
- [ ] **both, LTO build:** build and check with link-time optimisation on, as CRAN's checks do.
- [ ] **both, noSuggests check:** `R CMD check` with `_R_CHECK_DEPENDS_ONLY_` set, so no suggested package is needed.
- [ ] **both, C and ISO-8859-15 locales:** run the tests in each.
- [ ] **both, check time:** measure `R CMD check` for each package against CRAN's ten-minute norm.
- [ ] **HyperTabular, `cran-comments.md`:** brought up to HyperCast's level: R-devel, the R 4.2 floor, Windows, the r-oldrel section and the Unicode sentence.
- [ ] **both, r-oldrel (D9):** the Rust floor stays 1.89, so the two r-oldrel macOS flavors (rustc 1.84.1) keep failing at `configure`, because both crates are edition 2024 and need Rust 1.85. Explain it in each `cran-comments.md`; revisit only if CRAN's HyperUuid feedback says otherwise.
- [ ] **both, win-builder, mac-builder, rhub and checktor:** run on the exact tarballs. Those that upload or email the maintainer are the owner's to run.
- [ ] **both, release cadence.** CRAN asks for updates no more than every one to two months, so R ships a batch of releases, not every tag.

## Evidence required to close the gate

- [ ] **both, core regression evidence:** corpus vectors cover every changed behavior and boundary; every scanner engine and binding agrees. Preserve allocation-free and no-panic checks and fault-span invariants.
- [ ] **both, R value boundaries:** test integer extrema/sentinels, values around 2^53, decimal precision, temporal extremes, civil versus instant conversions, U+0000, invalid encodings and missingness. Verify standalone casts and table columns agree.
- [ ] **HyperTabular, reader lifecycle:** test interrupted/erroring reads, repeated close, garbage collection, retained batches, header edge cases and coordinates after filtering/ranges. Exercise large-input limits with bounded fixtures.
- [ ] **both, ABI and coexistence:** test old supported interfaces, new diagnostics/options, and both R packages loaded in either order. Verify symbols and the declared dependency architecture on supported platforms.
- [ ] **both, submission artifacts:** run `R CMD check --as-cran` on the exact source tarballs with current R-devel, plus Windows and macOS coverage. Resolve errors/warnings and explain remaining NOTEs. Record environments, commands, results, archive sizes and fixture coverage in `cran-comments.md`; inherited HyperUuid results do not substitute for these packages' checks.

## References

- [CRAN Repository Policy](https://cran.r-project.org/web/packages/policies.html) — dependencies, size, check resources and submission expectations.
- [Writing R Extensions](https://cran.r-project.org/doc/manuals/r-release/R-exts.html) — native interfaces, memory ownership, cleanup and interrupts.
- [bit64 documentation](https://cran.r-project.org/web/packages/bit64/refman/bit64.html) — signed integer64 semantics and conversions.
