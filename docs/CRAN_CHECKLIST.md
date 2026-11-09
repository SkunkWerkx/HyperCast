# HyperCast & HyperTabular: 1.1 Release and CRAN Submission Gate

Oct 31, 2026 · @Buvy

_Status 2026-11-05: HyperCast's rows 3, 7, 8 and 10 are in the core, the corpus and the bindings, proved by the full-board run [42152914672](https://github.com/SkunkWerkx/HyperCast/actions/runs/42152914672) at commit `9fbe8b2`, the companion pin in HyperTabular's `.github/hyper.json`. Its R type table is [`r/README.md`, Values](../r/README.md#values). Status 2026-11-09: the HyperTabular rows and contracts are implemented in both repos' uncommitted `cran_1_1` working trees, and their boxes are ticked on that evidence, except those left open with a reason. Every such tick, and every R CMD check row in [`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), is **candidate** class, and a box whose only evidence is candidate class is not closed for submission until the work is committed and the tag run's preserved tarballs are checked. Each box names its repo: **HyperCast**, **HyperTabular** or **both**. Ticked boxes carry their evidence in italics._

All twelve features below remain required for 1.1, with binding parity. This is our release gate, not a claim that CRAN requires feature parity with readr or readxl. CRAN acceptance also requires safe R integration and verification of the submitted packages.

HyperUuid's shared binding, build, packaging and review improvements are carried into both packages as each box below says; CRAN's feedback on HyperUuid is a gate of its own (see CRAN gates). This document records the additional work specific to HyperCast and HyperTabular. The assessment behind it was a source review, not a new test run or an R CMD check; unchecked items require implementation and evidence before they can be closed.

## Parity rule and order

**The rule:** a feature is done when its shared behavior is implemented once, the corpus pins it, and every binding exposes it in its own idiom. Parsing belongs in the core; allocation, host representations and presentation belong in the bindings. A binding that can't follow says why, in its README.

**Bindings in scope:** Rust, C#, Java, Go, Swift, Python, Ruby, PHP, TypeScript, R, C and C++. TypeScript, R, C and C++ follow the same rule as the rest.

**Order:**

1. Fix quote handling and unsafe text handling; settle grouping defaults and uncached-formula semantics. Nothing ships with known silent-data errors.
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
| 3 | Grouping policies | HyperCast | Shared validation for integer, float and decimal; declared Western (3) or Indian (2+3) grouping | Grouping policy on each `NumFormat` | Western by default, with declared separators/pattern |
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

- [x] **HyperTabular, quotes:** scanner and cell extraction agree on when a quote is structural. Test every scanner engine, buffer/block splits, odd stray quotes, escaped quotes and multiline records. No record may disappear or merge silently.
  _Task 3. HyperTabular's `rust/tests/kernel_delimited.rs` (`quotes_decide_boundaries_only`, `quotes_leave_unescaping_as_it_was`, `quotes_at_every_split`, `quotes_cross_block_edges`, `an_unclosed_quoted_cell_still_fails`, and `the_scanner_matches_a_byte_at_a_time_reference` over 20,000 seeded strings) and `delimited_windows.rs` (`a_stray_quote_loses_nothing_at_a_seam`), run on all six engines, the x86 ones under emulation; eight added `corpus/delimited.json` cases, no existing one changed, replayed by every binding; the rule in `docs/delimited.md`. Candidate (uncommitted working tree), not closed for submission; real x86 and s390x hardware are CI's._
- [x] **HyperCast, grouping:** strict validation does not infer locale. `1,234` remains ambiguous without declared separators. Test grouping across numeric doors, currency separators and native digits. (Amended 2026-11-06: this first said to preserve the documented legacy mode. The owner removed that mode, see [`docs/grouping.md`](grouping.md).)
  _Western grouping is the R default and `grouping = "indian"` is declared; `12,34` and the zero-led `0,125` are `malformed` with detail `group_size` under both policies, and `1,23,456` under Western ([`r/README.md`](../r/README.md)); `corpus/` pins it and the run above is green._
- [x] **both, text:** define BOM handling, malformed sequences, truncated UTF-16 code units and embedded NUL behavior. Never silently replace invalid input. Validate headers as well as values before making host strings.
  _Tasks 2, 4 and 6. HyperTabular's `docs/delimited.md` ("What is fixed here", "Decoding a declared encoding") defines all four, with nothing replaced; `rust/tests/kernel_delimited.rs` pins them (`invalid_utf8_is_malformed_on_text`, `invalid_utf8_in_a_header_is_a_failure`, `utf8_validity_is_chunk_invariant`, `a_bom_does_not_count_as_a_line`, `utf16_rejects_a_lone_surrogate`, `a_truncated_final_unit_fails_only_at_last`, `transcode_is_chunk_invariant`, `embedded_nul_passes`), and `kernel_workbook.rs` the workbook header and cell; HyperCast's side is D7 and D8 below. Candidate (uncommitted working trees), not closed for submission._
- [x] **HyperTabular, input ordering:** define decoding, skip/header selection, tokenization/unescaping, missing-token matching and casting order. Specify physical lines versus logical records, per-column token overrides and whether quoted tokens match.
  _Tasks 5 to 7. HyperTabular's `docs/delimited.md`, "What a read does, in order": `skip` counts physical lines, `header_row` records, a column's token list replaces the dialect's, and a quoted cell is never a token; `docs/workbook.md`, "Reading part of a sheet", for sheets. Pinned by `kernel_delimited.rs` (`skip_past_the_end_reads_nothing`, `header_row_discards_records`, `tokens_meet_blanks_and_padding`, `a_quoted_token_is_text`) and `kernel_workbook_rules.rs` (`sheet_skip_and_header_row_count_inside_the_range`). Candidate (uncommitted working tree), not closed for submission._
- [x] **HyperTabular, formula state:** preserve uncached formulas separately from blanks and cached formula errors in each supported workbook format. Formula evaluation is outside scope.
  _Task 8. XLSX and ODS mark an uncached formula `Empty` at origin `uncached`; blanks and cached errors are unchanged; Numbers holds no such cell, and `docs/workbook.md` says why. `kernel_workbook_rules.rs` (`uncached_formula_is_marked`, `an_ods_formula_at_the_edges_of_having_a_value` and the rest) and the `uncached.*` corpus packages. By the owner's decision for this release, a row of only uncached formulas is filtered by `skip_empty_rows` as 1.0 did, documented with `skip_empty_rows=false` as the lossless route. Candidate (uncommitted working tree), not closed for submission._
- [x] **HyperCast, trimming and dates:** preserve original byte offsets after NBSP trimming and the char door's single-whitespace behavior. Pin pivots, month vocabularies and new timestamp forms in the corpus.
  _`r/README.md` documents the `pivot` for two-digit years, the month vocabulary and U+00A0 as `White_Space` trimmed at both ends, with `cast_char()` given exactly one character keeping it; `corpus/date.json` and `corpus/char.json` pin them. `corpus/timestamp.json` pins U+00A0 trimmed from both ends and, in `2026-01-07\u00a015:04:05Z`, a fault at byte offset 10, length 2, the two bytes of U+00A0; `corpus/constraints.json` pins an offset after trimming, `\u00a0Mail` violating at byte offset 2, length 4._
- [ ] **both, ABI:** preserve 1.0 exports and layouts. Version or extend the affected fault, format, dialect, column and workbook interfaces explicitly; do not append fields to an existing C layout in place. Specify old-caller defaults and unknown-option handling. Review Rust API compatibility too.
  _Open: HyperTabular's side holds (Tasks 5 to 11: new size-prefixed structs and `_with` exports, 1.0 layouts pinned by `offset_of!` and the header test, sizes the core doesn't read refused, `cargo semver-checks` against 1.0.0 clean at Task 11, behavioural breaks D1, D2 and D5 in the CHANGELOG), but HyperCast's Rust API was reviewed by reading only in Task 13d; `cargo semver-checks` against 1.0.0 now ran on both final trees (2026-11-09, candidate): HyperCast 202 pass, its negative control (unexporting `cast_bool`) fails as major; HyperTabular 201 pass, 0 fail, 1 major-level warning, `State.failure` reordered in the `repr(C)` delimited state, which is Task 5's 64-to-80-byte state (opaque to callers, 1.0 `_init` still takes 64-byte blocks, in the CHANGELOG). Open only for the owner to accept that warning._

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
  _HyperCast's type table is [`r/README.md`, Values](../r/README.md#values), one R type per door with exact columns beside it; U+0000 and text R can't hold are D7 and D8 below._
- [x] **HyperTabular, representation decisions recorded:** table columns use the same classes, missing-value representation and conversion rules as HyperCast's standalone casts, and a table cell and a standalone cast agree.
  _Task 21. HyperTabular's `r/README.md`, "The verdict", takes HyperCast's type table unchanged; `r/tests/testthat/test-values.R`, "standalone casts and table columns agree, NUL and invalid UTF-8 included", holds every door's table column identical to its `hypercast::cast_*()` on the same bytes. Candidate (uncommitted working tree; [`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md) rows 7 and 8), not closed for submission._
- [x] **HyperCast, D7 and D8:** `cast_char()` gains a `code_point` column and keeps U+0000 `ok` with `NA` as its `value`; text R can't hold (an embedded NUL, or over `INT_MAX` bytes) is `ok` with `NA` on `cast_text()`, with `raw = TRUE` for a list of raw vectors. `r/README.md` explains that `ok` alone does not guarantee a usable character value.
  _`r/tests/testthat/test-api.R`, `test-constraints.R` and `test-corpus.R`; `corpus/char.json` now replays with no row set aside, and a new `corpus/constraints.json` row pins NUL inside text in every binding. Past `INT_MAX` needs 2 GiB, so that test runs only under `HYPERCAST_TEST_LARGE=1`: run locally on 2026-11-05, on arm64 macOS. `R CMD check --as-cran` on that tree's tarball (487,300 bytes), rerun after `raw = TRUE` on an enum match moved to the bytes the core matched, same day: `Status: 1 NOTE`, the new submission._
- [x] **HyperTabular, D8:** text R can't hold is `ok` with `NA` on `col_text()`, with `raw = TRUE` for a list of raw vectors, agreeing with `cast_text()`; `problems()` adds a `host_conversion` row. Its docs say what `ok` with `NA` means in HyperCast's terms.
  _Task 21. HyperTabular's `r/README.md`, "The verdict" and "Problems", in HyperCast's words; `r/tests/testthat/test-api.R` (a NUL cell: `ok`, `NA`, its bytes under `raw = TRUE`, and a `host_conversion` row) and `test-values.R` (`col_text()` against `cast_text()`, `raw = TRUE` included). Candidate (uncommitted working tree; [`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md) rows 7 and 8), not closed for submission._
- [x] **both, missingness:** distinguish input `NA`, declared missing tokens, blanks, ragged-row padding, malformed values and uncached formulas. Ordinary declared missingness is not a parse problem; preserve provenance where callers need to distinguish the cases.
  _Tasks 2, 5, 8 and 21. HyperCast's `r/README.md`, Values: a missing input is `ok` `NA`. HyperTabular's `origin` (`cell`, `token`, `padded`, `uncached`) beside `ok` and `reason`; `problems()` leaves out blanks, tokens and padding. `r/tests/testthat/test-values.R`, "missingness is told apart through ok, reason and origin". Candidate (uncommitted working trees), not closed for submission._
- [x] **both, conversion faults:** host representation failures are distinguishable from parser failures. They must not be reported as if the core rejected a valid value.
  _Tasks 2 and 21 (D8): text R can't hold is `ok` with `NA` on `cast_text()` and `col_text()`, no longer `out_of_range`, and HyperTabular's `problems()` names it `host_conversion` with no span. HyperCast's `r/tests/testthat/test-api.R` and `test-constraints.R`; HyperTabular's `test-api.R` and `test-values.R`. Candidate (uncommitted working trees), not closed for submission._

**Cost:** eager R text conversion creates/interns strings and may dominate a read. Measure before promising speed; ALTREP is a possible later optimization, not a submission requirement.

## HyperTabular reader and diagnostic contracts

- [ ] **HyperTabular, coordinates:** define source row versus output row, source column versus projected column, sheet identity and index bases. Preserve coordinates through skip, header selection, ranges, blank-row filtering and batches. Existing `Batch::line()` supplies source lines/rows but does not settle the whole contract.
  _Open: Tasks 7, 9 and 21 define and pin the rest (`docs/design.md`, "Problems": `line`, `row`, plan and source columns, from zero, a sheet's own column wherever a range starts; R's from 1 in `problems()`; corpus `problems` cases under skip, header row, range and filter), but the scanner counts a lone `\r` inside a quoted cell as no line while it counts one outside quotes, so each such cell makes later `line` values one lower than "the record's first physical line" (reported to the owner in Task 6, unchanged and undocumented); and `header_row` counting sheet rows after the empty-row filter departs from the spec's sentence, which the owner is to amend (Task 7)._
- [x] **HyperTabular, spans:** identify whether a span indexes original file bytes, decoded cell bytes or unescaped text. Decoding and unescaping change offsets; retain mappings if original-file spans are promised. Define unavailable spans for typed workbook cells and host conversion faults.
  _Tasks 4, 6, 9 and 21. A cell's span indexes its decoded, unescaped text, and only `Failure::ENCODING`'s `byte` the original input (HyperTabular's `docs/delimited.md`, steps 7 and "Decoding a declared encoding"); a typed workbook cell's covers its text form (`docs/design.md`, "Verdicts and fault spans"); a host conversion has none (`r/R/problems.R`). `r/tests/testthat/test-api.R`, "a fault's span indexes raw in the units substr() counts", and every binding's corpus replay of spans. Candidate (uncommitted working tree), not closed for submission._
- [x] **HyperTabular, ownership:** returned R vectors and diagnostics remain valid after the next batch, reader close and garbage collection. Never expose reused buffer storage without a lifetime mechanism that makes it safe.
  _Task 21. Every vector is R's own copy; `r/tests/testthat/test-lifecycle.R` ("a batch stays good after its reader is closed and collected", "garbage collected at every allocation, a read is whole") and `test-api.R` ("a batch ... stays good after the reader moves on"), under `gctorture` too ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), rows 12 and 13). Candidate, not closed for submission._
- [x] **HyperTabular, cleanup:** define explicit close, finalization and use-after-close behavior. Release files and native buffers on success, errors and interrupts; cleanup must be safe when called more than once.
  _Task 21. `close()` on readers, sheets and workbooks, safe twice, a read after it an R error, finalizers as the net (HyperTabular's `r/README.md`, "Reading"); `test-lifecycle.R` ("close is safe twice, and a read after it is an error", "a reader whose read ends early closes the connection it opened") and `test-api.R` (connections); valgrind with 0 bytes definitely lost ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), row 19). Candidate, not closed for submission._
- [x] **HyperTabular, interrupts:** check at bounded safe points during long reads and conversions. HyperUuid's stack/R-owned-buffer assumptions do not cover growing arenas or file handles. Keep R error/interrupt long jumps out of live Rust frames and arrange cleanup for native resources.
  _Task 21. `R_CheckUserInterrupt` at each batch and every 2^20 elements, in C between core calls, native memory in external pointers (`r/src/hypertabular.c`); `test-lifecycle.R`, "an interrupt during a read hands every row over once, numbered right, or refuses", run with `NOT_CRAN=true` ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), row 8). A 950-trial storm lost a batch only in the caller's own gap, about one interrupt in 6,000, visible as a gap in `row`: a controller ruling the owner may overrule. Candidate, not closed for submission._
- [x] **HyperTabular, limits:** check batch sizes, allocation arithmetic, core span limits and growth requests before allocation. Define oversized-record/workbook errors and cancellation behavior; no unchecked narrowing of lengths or coordinates.
  _Tasks 18b and 21. The core's `hypertabular_batch_check` refuses a size past 4 GiB, or a zero, before anything is allocated, in checked arithmetic (HyperTabular's `docs/design.md`, "Errors"; `rust/tests/batch_size.rs`); an oversized record stays `row_too_long` and a workbook `TOO_LARGE`; `r/tests/testthat/test-values.R`, "sizes are the core's to refuse, before anything is allocated". Cancellation is the interrupts box above. Candidate (uncommitted working tree), not closed for submission._
- [x] **HyperTabular, headers:** define empty/duplicate name handling and preserve original names alongside any repaired names. Named plans must not silently select an unintended duplicate.
  _Tasks 11 to 21 (D5): every binding's strict resolver and R's named plan refuse a duplicate, naming both positions, and optional lookups keep the first match, documented. R repairs names on presentation and keeps `original_names`. `test-values.R` ("a named plan refuses a duplicate", "repeated output names are made unique, and the originals kept", "a header's edge cases read as the source holds them"). Candidate (uncommitted working tree), not closed for submission._

- [x] **HyperTabular, HyperCast rows 3, 7 and 8 reach it:** grouping policies and the `NumFormat` pattern, the fault `detail` and the new date shapes are exposed by HyperTabular's plan columns, `problems()` and corpus, in every binding, not only in HyperCast's own casts. Row 10 (NBSP trimming) needs no surface.
  _Tasks 5, 7, 9, 13e and 12 to 21: a column's format and date `options` on every binding's plan, `detail` on its faults and in R's `problems()`; the corpus pins flags 95 and 607, the `grouping.*` packages, `detail` and `options`, and every binding's replay passed at gates 21b and 21c. `test-values.R`, "an Indian column beside a Western one reads each as declared". Candidate (uncommitted working trees), not closed for submission._

## CRAN gates

These gates are additional to the shared HyperUuid packaging work. CRAN review remains discretionary; passing this list is our submission criterion, not a guarantee of acceptance.

- [x] **both, R dependency architecture:** record whether HyperTabular imports HyperCast's R functions/classes, uses a registered native API, or vendors its Rust crate independently. Declare actual R dependencies and compatible versions; preserve crate attribution either way.
  _HyperTabular's `r/DESCRIPTION` has `Imports: hypercast (>= 1.1.0)`, and `r/tools/vendor.sh` vendors the core and the `hypercast` crate into `src/rust/vendor.tar.xz`, with `inst/AUTHORS` generated from it._
- [ ] **both, submission order:** HyperCast goes first. HyperTabular declares an R dependency on it, so HyperCast must be available on CRAN before HyperTabular's submission; budget the extra review cycle.
  _Open: the owner's, at submission; nothing is submitted yet._
- [x] **both, lockstep.** Every HyperCast update is checked against HyperTabular, and a break blocks the update until both are fixed. The Rust lockstep already works this way; the R side needs the guard and the reverse-dependency step below.
  _The guard and the reverse-dependency step below are both in place; HyperTabular's `RELEASING.md` runs the step before every HyperCast submission._
- [x] **HyperCast, R-level compatibility guard:** a check that HyperTabular's R package still loads and passes against the HyperCast it imports, and an explicit failure when the versions disagree.
  _hypertabular checks the layout every hypercast packs and stops with "install versions of the two that agree" when it reads none of it; a hypercast doctored to pack 96 bytes fails hypertabular's examples and tests with that message ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), row 25), and the 1.1.0 candidate passes (row 24)._
- [x] **both, reverse dependency:** a scripted step that checks hypertabular against a candidate hypercast, run before any hypercast submission.
  _HyperTabular's `.github/scripts/r_revdep_check.sh <hypercast tarball> <hypertabular tarball>`, in its `RELEASING.md` step 6: `Status: OK` against the candidate, exit 1 against the doctored one ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), rows 24 and 25)._
- [ ] **both, HyperUuid's feedback is a gate.** Carry every reviewer comment into both packages before submission, or record that CRAN said there are none. The owner's answer decides how this closes. The shared checklist is [HyperUuid/docs/CRAN_CHECKLIST.md](../../HyperUuid/docs/CRAN_CHECKLIST.md).
  _Open: waits on the owner's answer from CRAN on HyperUuid, its word on D9 included._
- [x] **both, tarball size and fixtures:** measure the built archive against CRAN's 5 MB guideline and the 10 MB ceiling. Keep a deliberate fixture manifest covering all shipped features, with `r_corpus` extended by the new fixtures; a missing required fixture fails the tests and is not skipped. Verify the actual R dependency/vendor tree.
  _524,288 and 1,417,147 bytes, under 5 MB; hypertabular unpacks to 8.2 MB, 6.9 MB of it the corpus. `r_corpus` names every file the replay reads, and a stale entry fails its test (row 26); `dates.ods` removed fails the replay, not skipped (row 14). The vendor tree is the core, hypercast 1.1.0 and simdutf8 0.1.5 ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md))._
- [x] **both, panic boundary:** rerun the no-panic proof for affected exports, including the new ones, and record its scope and toolchain. The current `no_std` artifacts abort on panic; `catch_unwind` cannot catch an abort. A C shim over proven exports may retain HyperUuid's architecture. Do not claim the kernel proof covers the whole wrapper. Inspect the actual R shared library and explain relevant compiled-code findings in `cran-comments.md`.
  _`cargo no-panic` over all 75 hypercast and 23 hypertabular exports with rustc 1.99.1, simdutf8 linked, each canary refused by name; `nm` on both built `.so` files; scope (the Rust core, not the C shims) in both `cran-comments.md` ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), rows 21 to 23)._
- [x] **both, `rchk`:** run on the built packages; record the result in `cran-comments.md`.
  _no function reported on round 1, after the shim fixes its round 0 findings led to; a dropped `PROTECT` is reported ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), rows 1, 2, 4, 5); in both `cran-comments.md`._
- [x] **both, `gctorture`:** the R lifecycle tests run under `gctorture`.
  _hypertabular's lifecycle tests under `gctorture(TRUE)`, its value tests and hypercast's API and no-throw tests under `gctorture2(20)` ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), rows 12 and 13)._
- [x] **both, valgrind:** run the tests and examples under valgrind.
  _tests and examples of both, 0 errors and 0 bytes definitely lost; its control is reported ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), rows 19 and 27)._
- [x] **both, UBSAN and ASAN:** build and run the tests with the sanitizers.
  _R-devel built with ASan, the shims with `-fsanitize=address,undefined`, the Rust archive plain and under nightly's `-Zsanitizer=address`: tests and examples of both with no report; both controls reported ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), rows 30 and 31)._
- [x] **both, LTO build:** build and check with link-time optimisation on, as CRAN's checks do.
  _`--use-LTO` with GCC 15.2 and `-Werror=odr -Werror=lto-type-mismatch`: `Status: OK` each ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), row 20)._
- [x] **both, noSuggests check:** `R CMD check` with `_R_CHECK_DEPENDS_ONLY_` set, so no suggested package is needed.
  _`Status: OK` each ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), row 9)._
- [x] **both, C and ISO-8859-15 locales:** run the tests in each.
  _`Status: OK` each ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), rows 10 and 11)._
- [x] **both, check time:** measure `R CMD check` for each package against CRAN's ten-minute norm.
  _36 s to 116 s across environments, the longest R-devel under emulation ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), row 29)._
- [x] **HyperTabular, `cran-comments.md`:** brought up to HyperCast's level: R-devel, the R 4.2 floor, Windows, the r-oldrel section and the Unicode sentence.
  _environments, results, memory and native-code checks, check time, the Unicode sentence, D8 and the r-oldrel section, quoting candidate rows of [`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md); Windows is still to be confirmed on win-builder._
- [x] **both, r-oldrel (D9):** the Rust floor stays 1.89, so the two r-oldrel macOS flavors (rustc 1.84.1) keep failing at `configure`, because both crates are edition 2024 and need Rust 1.85. Explain it in each `cran-comments.md`; revisit only if CRAN's HyperUuid feedback says otherwise.
  _both `cran-comments.md` explain it; `configure` refuses rustc 1.84.1 with each message ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), row 16)._
- [ ] **both, win-builder, mac-builder, rhub and checktor:** run on the exact tarballs. Those that upload or email the maintainer are the owner's to run.
  _Open: none has run; each uploads or emails the maintainer, so all are the owner's, on the tag run's preserved tarballs._
- [ ] **both, release cadence.** CRAN asks for updates no more than every one to two months, so R ships a batch of releases, not every tag.
  _Open: a standing practice, kept from the first acceptance on; nothing to prove before it._

## Evidence required to close the gate

- [x] **both, core regression evidence:** corpus vectors cover every changed behavior and boundary; every scanner engine and binding agrees. Preserve allocation-free and no-panic checks and fault-span invariants.
  _Tasks 3 to 22b and gates 21b and 21c: `corpus/delimited.json` (123 cases) and `corpus/workbook.json` (398 cases, 41 packages) in HyperTabular and HyperCast's `corpus/` pin each change, with no existing case's values changed; every binding's replay and suite passed, a 13,520-case sampler differential across the bindings found no difference, and all six scanner engines ran (x86 under emulation). HyperTabular's `rust/check-core.sh` and HyperCast's `cargo no-panic` hold with simdutf8 linked, canaries refused ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), rows 22, 23 and 28); the allocation-counting tests pass. Candidate (uncommitted working trees), not closed for submission; real x86, s390x, Windows and the mobile legs are CI's._
- [x] **both, R value boundaries:** test integer extrema/sentinels, values around 2^53, decimal precision, temporal extremes, civil versus instant conversions, U+0000, invalid encodings and missingness. Verify standalone casts and table columns agree.
  _Tasks 2 and 21: HyperTabular's `r/tests/testthat/test-values.R` runs every one of these through a table column and `hypercast::cast_*()` and holds them identical, with civil against instant and missingness in their own tests; HyperCast's `test-api.R` and `test-constraints.R` hold its side, NUL and past `INT_MAX` (D7, D8 above). Candidate ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), rows 6 to 8 and 13), not closed for submission._
- [x] **HyperTabular, reader lifecycle:** test interrupted/erroring reads, repeated close, garbage collection, retained batches, header edge cases and coordinates after filtering/ranges. Exercise large-input limits with bounded fixtures.
  _Task 21: `r/tests/testthat/test-lifecycle.R` (interrupts, repeated close, a read ending early, re-entry, `gctorture`, retained batches), `test-values.R` (header edge cases; sizes refused before allocation, no large fixture) and `test-corpus.R`, whose `problems` cases carry `line` and `source_column` under skip, header row, range and the empty-row filter. Candidate ([`CRAN_EVIDENCE.md`](CRAN_EVIDENCE.md), rows 8 and 12), not closed for submission._
- [ ] **both, ABI and coexistence:** test old supported interfaces, new diagnostics/options, and both R packages loaded in either order. Verify symbols and the declared dependency architecture on supported platforms.
  _Open: the tests exist (each binding calls its kept 1.0 exports; `test-values.R`, "the 1.0 exports read every format Western"; `test-lifecycle.R`, "both packages load in either order, in fresh processes", row 8), and symbols are checked on Linux and macOS only (row 21). Windows' symbols, through win-builder and the board's Windows core legs, are still to come._
- [ ] **both, submission artifacts:** run `R CMD check --as-cran` on the exact source tarballs with current R-devel, plus Windows and macOS coverage. Resolve errors/warnings and explain remaining NOTEs. Record environments, commands, results, archive sizes and fixture coverage in `cran-comments.md`; inherited HyperUuid results do not substitute for these packages' checks.
  _Open: every row so far is a candidate tarball built from a working tree (hypertabular's still versioned 1.0.0); the submission tarballs come from the tag run, and Windows (win-builder) has not run._

## References

- [CRAN Repository Policy](https://cran.r-project.org/web/packages/policies.html) — dependencies, size, check resources and submission expectations.
- [Writing R Extensions](https://cran.r-project.org/doc/manuals/r-release/R-exts.html) — native interfaces, memory ownership, cleanup and interrupts.
- [bit64 documentation](https://cran.r-project.org/web/packages/bit64/refman/bit64.html) — signed integer64 semantics and conversions.
