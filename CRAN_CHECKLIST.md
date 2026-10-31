# HyperCast & HyperTabular: CRAN Adoption Plan

Oct 31, 2026 · @Buvy

1.1 closes every known gap: each feature below lands in the Rust core once and in every binding in the same pass, R and TypeScript included.

## Parity rule and order

**The rule:** a feature is done when the core has it, the corpus pins it, and every binding exposes it in its own idiom. A binding that can't follow says why, in its README.

**Bindings in scope:** Rust, C#, Java, Go, Swift, Python, Ruby, PHP, TypeScript, R, C and C++. TypeScript, R, C and C++ arrive with tomorrow's cascade from HyperUuid.

**Order:**

1. Wrong-data fixes (features 1-3). Nothing ships before these.
2. Every other row of the core features table, all twelve, in 1.1.
3. R and TypeScript type decisions, then those two bindings.
4. Every binding's glue catches unwinds at the boundary (TypeScript's check already enforces it; extend it to the rest).
5. 1.1 releases; HyperCast goes to CRAN, then HyperTabular.

## Core features, with binding parity

The first three are wrong-data fixes; the rest are what adoption needs.

| # | Feature | Repo | Core change | Every binding exposes | R shape |
| --- | --- | --- | --- | --- | --- |
| 1 | Stray `"` stays literal | HyperTabular | A quote is a quote only at a cell's start or inside an open quoted cell; odd-count corpus vectors | Nothing new: same reads, right answers | Same |
| 2 | Ragged rows | HyperTabular | Declared policy: pad short rows with `Empty`, long rows a row fault or truncate | A ragged option on the dialect | `fill =` argument |
| 3 | Strict grouping | HyperCast | Validate group sizes (3, and Indian 2+3) under a new flag | The flag on each `NumFormat` | On by default |
| 4 | Skip, header row, range | HyperTabular | Leading rows to skip and a header row index, delimited and sheets; a cell range for sheets | Options on the dialect and `SheetOptions` | `skip =`, `range = "A5:F200"` |
| 5 | Missing-value tokens | HyperTabular | A declared token list read as `Empty` | A tokens option per dialect and per column | `na = c("", "NA")` |
| 6 | Declared encoding | HyperTabular | Windows-1252, Latin-1, UTF-16LE decoded to UTF-8; code-page tables fit `no_std` | An encoding option | `encoding =` |
| 7 | Fault detail | HyperCast | A detail code beside the reason, through new exports so the 1.0 ABI holds | A `detail` on every fault, in each binding's union | A `detail` column in `problems()` |
| 8 | Date and time shapes | HyperCast | `20260107`; two-digit years with a declared pivot; month names; space-separated RFC 3339 and `+00` offsets | The new options on the date doors | Arguments on the date casts |
| 9 | Uncached formulas | HyperTabular | A signal distinct from a blank cell | A way to tell the two apart | A `problems()` row |
| 10 | Trim no-break space | HyperCast | Trim U+00A0 at the edges | Nothing new | Same |
| 11 | Problems summary | HyperTabular | Each batch lists its faults: row, column, reason, detail, span | A batch-level faults list beside the per-cell verdicts | `problems()` |
| 12 | Plan suggester | HyperTabular | One export that reads a sample and proposes a plan, never applied | A `suggest` call returning a plan to inspect and pass back | `suggest_plan()` printing a `col_types` spec |

Each row lands as one coordinated change: core, corpus vectors, then every binding, as the char door did.

## R and TypeScript bindings

R users will compare this with readr, vroom, `data.table::fread` and readxl, so it takes their shapes and sells the verdicts, not raw speed.

**Shape**

- A `col_types`-style plan.
- Faults in a `problems()` table: row, column, reason, detail, span.
- A plan suggester that prints a plan for the user to pass back, never applied silently.
- Vectorised casts: character in, typed vector out, `NA` for a fault.

**Type decisions for both (settle before any code)**

| Value | R | TypeScript | Decision to make |
| --- | --- | --- | --- |
| i32 | `integer`, whose `NA` is `INT_MIN` | `number`, exact | R: `-2147483648` becomes `OutOfRange`, or i32 comes back as double |
| i64, u64 | no native type | `number` is inexact above 2^53 | R: `bit64::integer64` or double. TypeScript: `bigint` |
| u32 above 2^31 | no native type | `number`, exact | R: double |
| decimal | no native type | no native type | Double, string, or a class or library: one rule for both |
| timestamp | `POSIXct`, about a microsecond | `Date` is millisecond-precise | R: truncate to microseconds and say so. TypeScript: the `{seconds, nanos}` pair, `Temporal` where available |
| time of day | nothing in base R | nothing native | R: `hms` or `difftime`. TypeScript: nanoseconds as `bigint` or the pair |
| date | `Date` | no date-only type | TypeScript: `{year, month, day}` or `Temporal.PlainDate` |
| duration | `difftime` | no native type | TypeScript: the `{seconds, nanos}` pair |
| UUID, char | `character` | `string` | Direct |

**Cost:** every text cell becomes a string in R's global cache, which dominates a read as it does in Ruby and Python. vroom avoids this with ALTREP; measure before promising speed.

## CRAN gates

The dependency tree is honest everywhere: HyperTabular's R package depends on HyperCast's, as its crate does.

- [ ] **HyperCast is accepted on CRAN first.** HyperTabular can't be submitted until it is: one extra review cycle in the schedule.
- [ ] **Plan for the lockstep.** Every HyperCast update is checked against HyperTabular, and a break blocks the update until both are fixed. The Rust lockstep already works this way.
- [ ] **Corpus subset in the tarball.** HyperTabular's corpus is 3 MB against CRAN's 5 MB guideline; ship a subset and skip cases whose files are missing. Vendored crates are small: HyperCast has none, HyperTabular only `hypercast`.
- [ ] **Panic-free, said out loud.** Both cores are proven panic-free; say so in `cran-comments.md`, and catch unwinds in the R glue anyway.
- [ ] **Release cadence.** CRAN asks for updates no more than every one to two months, so R ships a batch of releases, not every tag.
- [ ] **HyperUuid's lessons first.** Carry every reviewer comment HyperUuid gets into both packages before either is submitted. The full list of findings is in `HyperUuid/CRAN_CHECKLIST.md`.
