# HyperCast & HyperTabular: 1.1 Release and CRAN Submission Gate

Oct 31, 2026 · @Buvy

All twelve features below remain required for 1.1, with binding parity. This is our release gate, not a claim that CRAN requires feature parity with readr or readxl. CRAN acceptance also requires safe R integration and verification of the submitted packages.

Assume HyperUuid's shared binding, build, packaging and review improvements will be ported. This document records the additional work specific to HyperCast and HyperTabular. The assessment behind it was a source review, not a new test run or an R CMD check; unchecked items require implementation and evidence before they can be closed.

## Parity rule and order

**The rule:** a feature is done when its shared behavior is implemented once, the corpus pins it, and every binding exposes it in its own idiom. Parsing belongs in the core; allocation, host representations and presentation belong in the bindings. A binding that can't follow says why, in its README.

**Bindings in scope:** Rust, C#, Java, Go, Swift, Python, Ruby, PHP, TypeScript, R, C and C++. TypeScript, R, C and C++ arrive through the HyperUuid cascade.

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

- [ ] **Quotes:** scanner and cell extraction agree on when a quote is structural. Test every scanner engine, buffer/block splits, odd stray quotes, escaped quotes and multiline records. No record may disappear or merge silently.
- [ ] **Grouping:** strict validation does not infer locale. `1,234` remains ambiguous without declared separators. Test grouping across numeric doors, currency separators and native digits; preserve the documented legacy mode.
- [ ] **Text:** define BOM handling, malformed sequences, truncated UTF-16 code units and embedded NUL behavior. Never silently replace invalid input. Validate headers as well as values before making host strings.
- [ ] **Input ordering:** define decoding, skip/header selection, tokenization/unescaping, missing-token matching and casting order. Specify physical lines versus logical records, per-column token overrides and whether quoted tokens match.
- [ ] **Formula state:** preserve uncached formulas separately from blanks and cached formula errors in each supported workbook format. Formula evaluation is outside scope.
- [ ] **Trimming and dates:** preserve original byte offsets after NBSP trimming and the char door's single-whitespace behavior. Pin pivots, month vocabularies and new timestamp forms in the corpus.
- [ ] **ABI:** preserve 1.0 exports and layouts. Version or extend the affected fault, format, dialect, column and workbook interfaces explicitly; do not append fields to an existing C layout in place. Specify old-caller defaults and unknown-option handling. Review Rust API compatibility too.

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
| char | `character` except U+0000 | `string` | R: a code-point/raw alternative or a documented conversion fault for U+0000; never silently drop it |

- [ ] **Representation decisions recorded:** select concrete classes/return types, constructors, missing-value representation and conversion behavior before implementing these APIs. Exact output must remain available for every supported core value, or the binding must explicitly report its representation limitation.
- [ ] **Missingness:** distinguish input `NA`, declared missing tokens, blanks, ragged-row padding, malformed values and uncached formulas. Ordinary declared missingness is not a parse problem; preserve provenance where callers need to distinguish the cases.
- [ ] **Conversion faults:** host representation failures are distinguishable from parser failures. They must not be reported as if the core rejected a valid value.

**Cost:** eager R text conversion creates/interns strings and may dominate a read. Measure before promising speed; ALTREP is a possible later optimization, not a submission requirement.

## HyperTabular reader and diagnostic contracts

- [ ] **Coordinates:** define source row versus output row, source column versus projected column, sheet identity and index bases. Preserve coordinates through skip, header selection, ranges, blank-row filtering and batches. Existing `Batch::line()` supplies source lines/rows but does not settle the whole contract.
- [ ] **Spans:** identify whether a span indexes original file bytes, decoded cell bytes or unescaped text. Decoding and unescaping change offsets; retain mappings if original-file spans are promised. Define unavailable spans for typed workbook cells and host conversion faults.
- [ ] **Ownership:** returned R vectors and diagnostics remain valid after the next batch, reader close and garbage collection. Never expose reused buffer storage without a lifetime mechanism that makes it safe.
- [ ] **Cleanup:** define explicit close, finalization and use-after-close behavior. Release files and native buffers on success, errors and interrupts; cleanup must be safe when called more than once.
- [ ] **Interrupts:** check at bounded safe points during long reads and conversions. HyperUuid's stack/R-owned-buffer assumptions do not cover growing arenas or file handles. Keep R error/interrupt long jumps out of live Rust frames and arrange cleanup for native resources.
- [ ] **Limits:** check batch sizes, allocation arithmetic, core span limits and growth requests before allocation. Define oversized-record/workbook errors and cancellation behavior; no unchecked narrowing of lengths or coordinates.
- [ ] **Headers:** define empty/duplicate name handling and preserve original names alongside any repaired names. Named plans must not silently select an unintended duplicate.

## CRAN gates

These gates are additional to the shared HyperUuid packaging work. CRAN review remains discretionary; passing this list is our submission criterion, not a guarantee of acceptance.

- [ ] **R dependency architecture:** record whether HyperTabular imports HyperCast's R functions/classes, uses a registered native API, or vendors its Rust crate independently. A Cargo dependency alone does not require an R package dependency. Declare actual R dependencies and compatible versions; preserve crate attribution either way.
- [ ] **Submission order:** HyperCast goes first. If HyperTabular declares an R dependency on it, that dependency must be available on CRAN before HyperTabular's submission; budget the extra review cycle.
- [ ] **Plan for the lockstep.** Every HyperCast update is checked against HyperTabular, and a break blocks the update until both are fixed. The Rust lockstep already works this way.
- [ ] **Tarball size and fixtures:** measure the built archive. CRAN's general data/documentation guideline is 5 MB; source tarballs should if possible stay below 10 MB. Keep a deliberate fixture manifest covering all shipped features; missing required fixtures fail tests. Do not broadly skip absent files. Verify the actual R dependency/vendor tree rather than assuming the plain core's tree.
- [ ] **Panic boundary:** rerun the no-panic proof for affected exports and record its scope/toolchain. The current `no_std` artifacts abort on panic; `catch_unwind` cannot catch an abort. A C shim over proven exports may retain HyperUuid's architecture. Any allocating Rust wrapper needs separate containment of unwind-capable code before an FFI boundary. Do not claim the kernel proof covers the whole wrapper. Inspect the actual R shared library and explain relevant compiled-code findings in `cran-comments.md`.
- [ ] **Release cadence.** CRAN asks for updates no more than every one to two months, so R ships a batch of releases, not every tag.
- [ ] **HyperUuid's lessons first.** Carry every applicable reviewer comment into both packages before submission. The shared checklist is [HyperUuid/CRAN_CHECKLIST.md](../HyperUuid/CRAN_CHECKLIST.md).

## Evidence required to close the gate

- [ ] **Core regression evidence:** corpus vectors cover every changed behavior and boundary; every scanner engine and binding agrees. Preserve allocation-free and no-panic checks and fault-span invariants.
- [ ] **R value boundaries:** test integer extrema/sentinels, values around 2^53, decimal precision, temporal extremes, civil versus instant conversions, U+0000, invalid encodings and missingness. Verify standalone casts and table columns agree.
- [ ] **Reader lifecycle:** test interrupted/erroring reads, repeated close, garbage collection, retained batches, header edge cases and coordinates after filtering/ranges. Exercise large-input limits with bounded fixtures.
- [ ] **ABI and coexistence:** test old supported interfaces, new diagnostics/options, and both R packages loaded in either order. Verify symbols and the declared dependency architecture on supported platforms.
- [ ] **Submission artifacts:** run `R CMD check --as-cran` on the exact source tarballs with current R-devel, plus Windows and macOS coverage. Resolve errors/warnings and explain remaining NOTEs. Record environments, commands, results, archive sizes and fixture coverage in `cran-comments.md`; inherited HyperUuid results do not substitute for these packages' checks.

## References

- [CRAN Repository Policy](https://cran.r-project.org/web/packages/policies.html) — dependencies, size, check resources and submission expectations.
- [Writing R Extensions](https://cran.r-project.org/doc/manuals/r-release/R-exts.html) — native interfaces, memory ownership, cleanup and interrupts.
- [bit64 documentation](https://cran.r-project.org/web/packages/bit64/refman/bit64.html) — signed integer64 semantics and conversions.
