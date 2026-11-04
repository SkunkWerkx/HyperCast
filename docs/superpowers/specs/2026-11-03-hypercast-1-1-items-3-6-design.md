# HyperCast 1.1: strict grouping, fault detail, date shapes and cleanup (items 3–6)

Date: 2026-11-03 · Size: a project (four items, one spec, one plan, one implementation) · Scope:
HyperCast's core, its C ABI, all ten bindings and the C++ header, the corpus, docs. HyperTabular
follows at the companion pin.

## Intent

These are the last HyperCast changes before 1.1 lands, gets one `purpose=board` run, and is pinned
once as HyperTabular's companion. Items 1 (`Constraints::from_raw`/`pack_enum`) and 2 (Unicode white
space) are on the same branch. The four items here are specified together because they interlock:
item 4's detail codes name failures items 3 and 5 introduce, and item 3 settles where item 2's
space-like group separators may sit. They are planned as one plan, with one task per binding
covering all of them, and implemented in one fan-out.

The origin is `CRAN_CHECKLIST.md`, items 3 (strict grouping), 7 (fault detail) and 8 (date shapes),
plus the release review's cleanup list and the backlog of deferred minors from items 1 and 2.

## Sequencing and constraints

- Implementation starts from item 2's merged and reviewed result in the main worktree, on the same
  branch (`constraints_from_raw`). The owner drives git: nothing is committed by agents.
- The core stays `no_std`, allocation-free and panic-free (`cargo no-panic` + the canary). No new
  dependency. Every change reaches every binding with parity (`CLAUDE.md`).
- **The 1.0 C interfaces keep their existing behaviour**: the 31 exports (32 symbols with
  `hypercast_version`), `Fault`, `RawFault`, `RawNumFormat`'s layout, null-format defaults, and the
  Rust struct literals over public types. New behaviour arrives through explicitly new
  interfaces, and the bindings move onto them together.
- Claims are measured: dated benchmark receipts, losses printed (`CLAUDE.md`).

---

## Item 3: strict grouping

### Policy and versioning (owner's decisions)

- **A two-bit policy field in `NumFormat.flags`**, bits 8–9 (no layout change):
  `00` legacy, `01` strict Western, `10` strict Indian, `11` refused. Named constants:
  `GROUPING_LEGACY` (0), `GROUPING_WESTERN`, `GROUPING_INDIAN`, and a `GROUPING_POLICY` mask.
- **From 1.1, `NumFormat::check` refuses policy `11` and any flag bit 1.1 doesn't define.** 1.0
  ignored unknown bits; this closes that for the future. Where it applies (owner's decision,
  revising the earlier proposal to apply it to the old exports too):
  - **The 1.0 C exports ignore bits 8 and above and always parse with legacy grouping**, so they
    behave exactly as in 1.0, stray bits included. Their `check` keeps 1.0's rules.
  - **The new detailed exports and every binding validator** enforce the 1.1 checks (policy `11`
    and unknown bits refused).
  - **The existing Rust doors honour the valid policies.** Their unchecked-format contract requires
    a format that passes `NumFormat::check`; behaviour for one that doesn't is unspecified (never
    promised to be legacy).
  - **The new Rust `detailed` doors take validated values** (below), so an invalid declaration
    can't reach them.
- **Setters replace the two-bit field**, never OR a second policy into it (a Rust `const fn
  with_grouping(self, policy)` and each binding's equivalent).
- **Old interfaces keep legacy:** the C ABI with a null format, a 1.0 layout with no policy bits,
  Rust `NumFormat::new(…, flags)` (policy is whatever `flags` says; none = legacy), and Rust
  `NumFormat::INVARIANT`. `ALL` stays a leniency mask and never carries a policy.
- **New: `NumFormat::STRICT`**: invariant separators, `ALL`, strict Western.
- **New-interface defaults:** every binding's 1.1 `NumFormat` constructor defaults to strict
  Western unless the caller declares Indian or legacy. **The bindings' `Invariant` constants
  become strict Western** (C# `NumFormat.Invariant`, Java `INVARIANT`, Swift `.invariant`, C++
  `NumFormat::invariant()`, Go `Invariant`, and so on). **Constructors take the policy separately
  from the leniency flags** (a style mask never carries a policy), and `withStyles(…)` and its
  equivalents preserve the current policy. Go's zero-value struct keeps policy `00`, legacy, while
  Go's `Invariant` explicitly selects strict Western. **Corpus rows that declare no policy keep
  legacy semantics** (the core and C ABI meaning); every new row declares its policy. A culture bridge declares what the
  platform's number and currency patterns say (Owner decision 1 and its revision: strict
  Western or Indian when every grouping pattern agrees on one, legacy otherwise). R's
  constructors and HyperTabular's new column declarations (at the pin) default to strict Western.
- **Strict policies don't enable grouping**: with `GROUPING` off, group separators stay forbidden
  under every policy.
- **Separator detection stays independent**: no grouping policy resolves an ambiguous `1,234`
  under `SEPARATOR_DETECT`.

### The strict rules

- **Western:** groups of exactly three digits counted leftward from the decimal separator (or the
  numeral's end); the leftmost group has one to three digits.
- **Indian** (only when declared): the group next to the decimal separator has three digits, every
  group to its left has two, the leftmost one or two.
- **Ungrouped numerals stay valid** under both.
- **Once a numeral uses a group separator, every group must conform**: `1234,567` is `Malformed`.
- **No grouping after the decimal separator or in an exponent**, under either strict policy.
- The same rules apply at the integer, float and decimal doors (and their checked twins), to the
  currency group separator, to native digits, and to item 2's interchangeable space-like
  separators. Digits are counted as digits (native digits included), never bytes.
- **Legacy** keeps today's rule: a group separator anywhere between two digits.

### Spans and the winner (owner-approved)

- An invalid group spans its original bytes **including its preceding separator**: `,34` in
  `12,34`.
- An oversized leftmost group has no preceding separator: `1234` in `1234,567`.
- A leading, trailing or repeated separator that creates an empty group spans **that separator**.
- **The leftmost invalid group in reading order wins** when several are invalid; the corpus pins
  it. This is the new check's own rule; elsewhere precedence is each door's existing order
  (item 4).

### The downgrade guard (owner's decision)

Policy bits select behaviour; they can't negotiate a library version, and a 1.0 core silently
ignores them. So:

- `include/hypercast.hpp` checks the loaded core's `hypercast_version()` at runtime and refuses a
  core below 1.1 before any strict policy is used (alongside item 6's compile-time version macro).
- The C header's docs state the requirement for direct C callers, with a checked example in
  `c/smoke/`.
- Every binding's existing load-time version check is verified to reject a 1.0 core; one that
  doesn't is fixed.
- **An explicit downgrade test**: a strict request against a 1.0 core is rejected by the guarded
  interface, never parsed in legacy mode. (A 1.0 library is available from the v1.0.0 GitHub
  Release or a build of the tag.)

### CHANGELOG

Called out prominently under "Changed": the new default, with `12,34` moving from a value to
`Malformed` through the binding constructors and bridges, and the escape hatch (`GROUPING_LEGACY`
and each binding's legacy option).

---

## Item 4: fault detail

### Purpose and stability contract (owner's decisions)

`detail` is mainly for people diagnosing rejected data (R's `problems()`, HyperTabular's summaries),
as a small shared vocabulary with precise meanings. Programs may inspect it, under this contract:

- Published codes keep their numeric values and meanings.
- New codes may appear in minor releases; consumers must tolerate unknown codes.
- `reason` stays the broad classification; an unknown detail still leaves a usable reason and span.
- Codes describe **what failed**, never a suggested retry or an internal parser location.
- Equivalent failures across doors share a code where the meaning is genuinely the same.
- **Every failure path is classified**; there is no "other" bucket. Code `0` is reserved for "no
  detail" (used only by a checked violation, below) and is never a vocabulary code.

### The vocabulary

The detail is independent of the reason. Where today's reason for a failure differs by door (∞ is
`OutOfRange` at a float door and `Malformed` at an integer door), the reason stays and the detail is
shared. The "typical span" column describes the usual shape; it is **not** a contract (below).

| # | Code | Meaning | Typical span |
|---|---|---|---|
| 1 | `blank` | empty, or only white space | empty |
| 2 | `unexpected_character` | a character the position doesn't allow, where the position allows several things (a letter in a numeral, an undeclared symbol) | that character |
| 3 | `digit_expected` | a numeral or digit field is under way, or a fixed position requires a digit, and another character or the end is found (`0xG`, `1e`, `1e+`, `2026-0x-07`) | that character; at the end, empty at the end |
| 4 | `delimiter_expected` | a required mark (`-` `:` `T` `}` `0x` `;`, a UUID hyphen) is missing or different | the character found |
| 5 | `trailing_text` | the value is complete, but more text follows | the first extra character |
| 6 | `no_digits` | the numeral is absent entirely: only a sign, a currency or percent affix, a bare radix prefix (`0x`, `&H`, `0b`) or a protobuf duration's bare `s` | the token |
| 7 | `sign_misplaced` | a sign where none may stand: a second sign, inside parentheses, mid-token, leading plus trailing | that sign |
| 8 | `parentheses_unbalanced` | `(` without `)`, or `( )` | `(` to the end |
| 9 | `mark_repeated` | a once-only mark appears twice (a second decimal separator, a second `T`) | the second mark |
| 10 | `group_misplaced` | a group separator not between digits, or an empty group | that separator |
| 11 | `group_size` | item 3: a group with the wrong digit count | per item 3 |
| 12 | `group_in_fraction` | item 3: grouping after the decimal separator or in an exponent | that separator |
| 13 | `ambiguous_separator` | separator detection can't tell `.` from `,` | the separator |
| 14 | `fraction_not_allowed` | a fraction where the type is whole (integer door, Unix count, negative exponent at an integer door, a fraction on a non-seconds duration unit) | the separator, or the fraction |
| 15 | `field_width` | a delimited field has the wrong number of digits (a two-digit year without a pivot, a 3-digit month, an over-long UUID component) | the field's digits |
| 16 | `wrong_length` | a fixed-shape token, recognised by its total length, has the wrong length (UUID, ISO date) | the token |
| 17 | `component_missing` | a required value-level part is absent (zone, seconds, a duration unit, any duration component, a bare `GUID:`, a bare `P`) | the token, or where it should start |
| 18 | `not_recognized` | not one of the door's accepted words or values (a boolean word, `bool_from_f64` not 0 or 1, an item-5 month name) | the word; empty for typed doors |
| 19 | `unknown_unit` | an unknown or misplaced duration unit | the unit |
| 20 | `invalid_utf8` | the bytes at the winning failure site don't decode as UTF-8 | the bad sequence |
| 21 | `too_long` | past an implementation limit (a 256-byte numeral, an 18-digit run, a span past `u32::MAX`) | the token, or the first digit past the limit |
| 22 | `not_finite` | a non-finite value: a textual NaN/infinity spelling at a numeric door, or a typed NaN/∞ where it can't be represented | the existing span (see "Non-finite values") |
| 23 | `not_whole` | a non-integral double at a whole-number typed door | empty |
| 24 | `negative_not_allowed` | negative where the type has no negatives (unsigned, an Excel serial) | the numeral, or the sign |
| 25 | `too_large` | the magnitude exceeds what the target type represents (overflow, a finite double past `f32::MAX`, a rounded multiple that no longer fits) | the numeral; empty for typed |
| 26 | `too_small` | below a signed type's minimum | same |
| 27 | `too_precise` | more fractional digits than the type holds (decimal scale above 28) | same |
| 28 | `date_field_out_of_range` | year 0, month 13, a day past the month's end | that field's digits |
| 29 | `time_field_out_of_range` | hour, minute, second or offset out of range | that field's digits |
| 30 | `outside_window` | a representable instant or duration past the supported window (0001–9999, ±10,000 years) | the token, or empty |
| 31 | `nonexistent_date` | a day that never existed: Excel's phantom 1900-02-29 (serial 60) | the token, or empty |
| 32 | `not_a_scalar` | a code point above U+10FFFF, or a surrogate | the token |

### Boundaries (owner-approved; corpus-pinned)

- **Precedence is each door's existing validation order.** The detailed result is the same parse,
  so it reports exactly the fault the door reports today: UUID checks hyphens before hex digits, so
  a bad hex digit at byte 0 plus a bad hyphen at byte 8 still reports byte 8. Structure before
  value, and the first field in reading order, are how today's doors happen to behave, not new
  rules. Item 3's leftmost-invalid-group rule is the one new precedence.
- **Spans are not changed by this item**: the old and detailed entry points always agree, because
  they project one parse. Spans that differ from the typical shape keep their values and are pinned
  (`cast_char("U+")` spans the whole token; `not_finite` spans the token for text and is empty for
  typed doors; an incomplete exponent spans the `e`). **The one deliberate span change** is
  `(-$5)`: today it spans the digit `5`; it will span the misplaced sign. Regression row plus
  CHANGELOG entry.
- **`invalid_utf8` is classified at the winning failure site only.** If the bytes where the door
  already fails don't decode as UTF-8, the detail is `invalid_utf8` instead of the code the
  position would otherwise get. No new validation pass, and no later encoding fault ever replaces
  an earlier grammar fault. `cast_text` (whose grammar is UTF-8 validation) is unchanged.
- **`no_digits` vs `digit_expected` vs `component_missing`**: as the table says.
- **Overlap vectors**:
  - `field_width` vs `wrong_length`: `cast_date("2026-1-07")` → `wrong_length`; a 35-character UUID
    → `wrong_length`; `date_ordered("26/01/07")` → `field_width`; `date_ordered("2026/1/123")` →
    `field_width`.
  - `unexpected_character` vs `digit_expected`: `0xG` → `digit_expected`; `1e` → `digit_expected`;
    `12a` and `abc` at `i32` → `unexpected_character`.
  - `too_large` vs `outside_window`: `i8 "128"` → `too_large`; a Unix count overflowing i128 →
    `too_large`; Unix `253402300800` → `outside_window`; `9999-12-31T23:00:00-05:00` →
    `outside_window`.
- **The sites that lump distinct failures today** (23, inventoried 2026-11-03) are split so each
  gets its honest code; splitting adds a detail and never changes which fault wins, its reason or
  its span.
- **Non-finite values** (pinned; reasons and spans exactly as today):
  - **Textual spellings at the numeric doors:** when the token, after the numeric frame (sign,
    currency, parentheses, white space) is stripped, is exactly `nan`, `inf` or `infinity` under
    ASCII case-insensitive comparison, the detail is `not_finite`. The reason stays `Malformed`,
    and the span stays what today's catch-all reports (the first letter). Any other letters stay
    `unexpected_character`. Rows: `NaN`, `-inf`, `Infinity`, `+INF` at `i32`, `f64` and
    `decimal`, plus `nana` and `infinite` as `unexpected_character`.
  - **Typed NaN/∞** (the `*_from_f64` doors, `checked_from_f64`): `not_finite`. The reason stays
    as today: NaN `Malformed` everywhere; ∞ `OutOfRange` at the float and decimal doors and
    `Malformed` at the integer, Unix and Excel doors. The span stays empty.
  - **Finite numeric text that overflows** (`1e400` at `f64`, `1e39` at `f32`): `too_large`,
    `OutOfRange`, spanning the trimmed numeral as today. It's a finite spelling, never
    `not_finite`.
- **Underflow stays unchanged** (`1e-400` reads as `0.0`; owner's decision). There is no underflow
  code.

### Rust shape (owner-approved)

- `#[non_exhaustive] pub struct Failure { pub reason: Reason, pub detail: Detail, pub offset: u32,
  pub len: u32 }` and `#[non_exhaustive] pub enum Detail` with explicit discriminants (the table's
  numbers), `Detail::code(self) -> u32` and `Detail::from_code(u32) -> Option<Detail>`.
- A `hypercast::detailed` module mirrors every door by name (`detailed::cast_i32`, …,
  `detailed::cast_checked`, the typed `*_from_f64` doors) returning `Result<T, Failure>`; the
  checked ones return `Result<Checked<T>, detailed::Rejection>` with `Failure(Failure)` and
  `Violation(Violation)`.
- The existing doors call the same parse and project `Failure` to `Fault`. `Fault`, `RawFault`
  and every existing signature are untouched.
- **The `detailed` doors take validated declarations** (owner's decision). A fallible validation
  produces them: for example `NumFormat::validate(self) -> Result<ValidNumFormat, Refusal-like
  error>` and the same for date options with their vocabulary. An invalid declaration is a
  declaration error at validation time, kept separate from `Failure`, whose reason always
  describes rejected input data. `Result<T, Failure>` never represents a bad declaration.

### C exports (approach B, owner's choice)

Four new exports, using `Door`'s existing codes:

- `cast_detailed(door, param, format, periods, date_options, vocab_ptr, vocab_len, ptr, len, out,
  fault)` (11 arguments; the date-option arguments are item 5's).
- `cast_detailed_from_f64(door, param, value, out, fault)`.
- `cast_checked_detailed(door, param, format, constraints, enum_ptr, enum_len, ptr, len, out,
  fault, check)` (11 arguments).
- `cast_checked_detailed_from_f64(door, param, constraints, enum_ptr, enum_len, value, out, fault,
  check)`.

Export count: 32 symbols → 36. Every "Lists that move together" site moves.

**Dispatch contract:**

- `door` is `Door`'s existing ABI code, with **`Door::Char = 23` added** (`DateTime` stays 21,
  `ExcelSerial` 22): 23 text-dispatched doors.
- `param` reuses the existing ABI codes (Unix precision 1–4, date order, Excel epoch) and must be
  0 for every door that declares nothing.
- `cast_detailed` accepts all 23 text doors. `out` receives exactly the type that door's existing
  export writes, and need not be aligned (written with `write_unaligned`).
- `cast_detailed_from_f64` accepts exactly the doors with a typed **C** export today: `Decimal`
  (`decimal_from_f64`), `ExcelSerial(e)`, `Time` (`excel_time`) and `Duration`
  (`excel_duration`). The integer, `f32`, `bool` and Unix typed doors stay Rust-only (the Rust
  `detailed` module covers them); no binding gains a number-input door. The checked pair accepts
  exactly `cast_checked`'s and `cast_checked_from_f64`'s doors.
- `format` is read only by the numeric doors (null means `INVARIANT`, legacy); `periods` only by
  `DateTime` (null means English); `date_options`/`vocab` only by the date doors item 5 names.
  **Pointers a door ignores are never dereferenced or validated**; only relevant layouts can cause
  a rejection.
- **Every validation happens before any output is written**: an unknown door, a door outside the
  export's set, a bad `param`, a refused layout, or a bad fault capacity returns −1 and writes
  nothing.

**`RawFaultDetail` capacity:**

- Layout v1: `{ size: u32, offset: u32, len: u32, detail: u32 }`, 16 bytes, 4-byte aligned. The
  caller writes `size`; the core never writes it.
- Accepted: `size ≥ 16` and a multiple of 4. Smaller returns −1 and nothing is written.
- A larger size (a newer caller) is accepted; this core writes only bytes 4–16 and never a byte
  past 16. A newer caller initialises any fields beyond those to their neutral value itself
  (documented beside the layout).
- A null `fault` is allowed: nothing is written and the return code is still correct.
- Written only on codes 1–3, and on 4 for the checked pair; never on 0 or −1.

**Checked results:**

| Return | `out` | `fault` | `check` |
|---|---|---|---|
| 0, a value | written | not written | `keyword` 0, `adjusted`, `enum_index` |
| 1–3, a parse failure | not written | `offset`, `len`, `detail` (≥ 1) | not written |
| 4, a violation | not written | `offset`, `len` of the value; **`detail` 0** | `keyword` (1–8), `adjusted` 0, `enum_index` `u32::MAX` |
| −1 | nothing written | nothing written | nothing written |

**Binding compatibility (owner's decision, approach B):** every **detailed failure** exposes
`detail`; compatibility takes precedence over putting it on every existing fault type.

- Each binding preserves its existing constructors, patterns and deconstruction, and
  equality/hash behaviour. A detail must never silently change equality of an existing fault type.
- Where the language allows a compatible extension (an extra non-positional property with the old
  constructors and deconstructors kept; keyword-only arguments; a new R column), the existing type
  carries it, with equality unchanged.
- Where it doesn't, the binding provides a **separate detailed result path** and leaves the
  existing API exactly as it is:
  - **Go:** adding any field to an exported struct breaks unkeyed literals, so the existing types
    stay as they are.
  - **Java:** `Verdict` and `Rejection` are sealed unions; a new variant would break exhaustive
    switches, even with the old record unchanged.

  Some bindings will need additional methods; that is accepted.
- Each binding's task demonstrates compatibility: a test compiled against the 1.0 surface
  (constructors, patterns or deconstruction, equality) still compiles and passes.

**Unknown details in the bindings:** every detailed failure carries the raw numeric code plus a
named value where its idiom has one, with an "unknown" member keeping the code. Decoding an
unknown code never throws, panics or rejects the result; each binding tests it with a constructed
`RawFaultDetail` carrying an unpublished code.

**Compatibility wording:** the detailed exports may change while unpublished; their signatures are
fixed once 1.1 is released.

---

## Item 5: date and time shapes

### All opt-in (owner's decision)

The existing doors and their detailed twins keep today's shape rules unless the caller passes date
options. **This deliberately differs from item 2:** whitespace tolerance was an approved widening,
not a general policy that every uniquely interpretable spelling becomes accepted. A caller using
strict RFC 3339 may rely on a space separator or `+00` being refused.

### Options

| Option | Door | Accepts |
|---|---|---|
| `COMPACT_DATE` | `cast_date` | `20260107` (`yyyyMMdd`, eight ASCII digits) |
| `SPACE_SEPARATOR` | `cast_timestamp` | a single space in place of `T` |
| `HOUR_OFFSET` | `cast_timestamp` | `±hh` offsets (`+00`, `-05`) |
| pivot | `cast_date_ordered`, `cast_datetime` | two-digit years |
| month vocabulary | `cast_date_ordered`, `cast_datetime` | declared month names in the month field |

- **`DateOptions::COMMON_FORMS`** turns on `COMPACT_DATE`, `SPACE_SEPARATOR` and `HOUR_OFFSET`
  together. It never supplies a pivot or a vocabulary. It works unchanged at every applicable date
  door: a known option irrelevant to a door is ignored; **an unknown flag bit is refused**.
- **Pivot:** the last year of a 100-year window; a two-digit `yy` maps to the one year in
  `[pivot−99, pivot]` ending in `yy`. Range **100–9999**, so the window stays within years 1–9999;
  `0` means absent. Four-digit years are unaffected. No default.
- **Month names, Excel style only:** a declared name replaces the numeric month field in the
  position the declared field order selects, with the existing repeated `/`, `-` or `.` separator.
  `07-Jan-26` reads with a pivot; `7-Jan-2026` without one. Prose layouts, optional commas and
  automatic reordering are deferred; a vocabulary never overrides the declared order.
- **Matching:** simple case folding (the text `enum`'s folding). The longest declared entry wins
  **among those followed by the required separator** (or the end, when the month is the last
  field). Aliases are separate entries (`Sep`, `Sept`). Punctuation is part of the name and never
  stripped: with `janv.` declared and `.` as the separator, `7.janv..2026` reads, while
  `7.janv.2026` is `Malformed` unless `janv` is also declared. Corpus rows pin both.

### Declarations

- **Vocabulary rules:** each entry maps to a month 1–12; every month has at least one entry; no
  entry is empty, longer than 64 bytes, or contains an **ASCII digit (`0`–`9`)**, `/`, `-` or any
  white space; `.` is allowed anywhere; two entries that fold to the same spelling are refused,
  even for the same month. Refusals are the caller's contract violation (the export's −1, each
  binding's declaration error).
- **Packed layout:** each entry is a month byte (1–12), a little-endian `u32` byte length, then
  that many UTF-8 bytes, with no padding. A null pointer with length 0 means no vocabulary.
  Refused: a cut-short entry or trailing bytes; invalid UTF-8; a zero or over-long name; a month
  outside 1–12 or a month without an entry; a forbidden character; a folded duplicate. Length
  arithmetic is checked. The caller owns the buffer; the core borrows it for the call only.
- **`RawDateOptions`:** `{ size, flags, pivot }`, three `u32`s, 12 bytes, 4-byte aligned.
  - **Null `date_options`** means no options: today's shape rules, no pivot.
  - **Accepted sizes:** 8, the size and flags words, through 12, the whole layout, in whole
    `u32` words, so 8 or 12. Anything else returns −1 before any write: below 8, not a multiple
    of 4, or **above 12** (a layout this core doesn't know is refused, as `RawNumFormat` refuses
    a size above its own).
  - **A shorter layout** (size 8) is read over zeros, so its pivot is 0, meaning absent.
  - The core never writes it. `flags` must hold only the defined option bits, and `pivot` must
    be 0 or 100–9999; otherwise −1.
- **The vocabulary pointer and length:**
  - **The vocabulary is independent of `date_options`.** It may accompany null options, which
    enables month names and nothing else.
  - **A length of 0 means no vocabulary**, whatever the pointer. A non-null pointer with length 0
    is not dereferenced.
  - **A null pointer with a nonzero length** returns −1, checked before any dereference.
  - **A door that ignores the vocabulary** (any door but `cast_date_ordered` and `cast_datetime`)
    never looks at either value.
- **Validated before any output is written, on every call** (the core keeps no state); the
  benchmark measures that cost.
- **Delivery:** only the detailed exports and Rust's `detailed` doors take date options. The 1.0
  exports never see the new shapes.

---

## Item 6: cleanup and the deferred-minor backlog

### The four cleanup items

1. **`Door` docs:** `lib.rs`'s door list (now including `Char`); `Door::Text`'s doc rewritten for
   HyperCast.
2. **C++ version macro** in `hypercast.hpp`, alongside item 3's runtime check.
3. **CHANGELOG formatting:** the mid-bullet `*(repository only)*` tag; stray blank lines splitting
   bullets.
4. **Stale links:** `csharp/README.md`'s `php/src/native/`; `CRAN_CHECKLIST.md`'s link to
   `../HyperUuid/CRAN_CHECKLIST.md`.

### The backlog (owner's decision)

The deferred minors recorded during items 1 and 2 (the Appendix, plus whatever item 2's final review
records) are item 6's checklist. **Triage before editing**, then each entry ends as one of:

- **fixed and verified**: stale docs and comments, broken links, small duplication, test gaps
  within the agreed behaviour, and message ordering that violates the approved ordering contract.
  A test-strength fix gets a negative control that fails without it.
- **superseded** by another change (named).
- **deferred**, with a reason.

Anything that would change public behaviour, expand an API or need a broader refactor is brought to
the owner instead of fixed. The final whole-change review verifies the dispositions; it doesn't
inherit an untriaged backlog.

**Ordering contract for the triage** (builder's call, stated so "violates" is decidable): the
approved order (length → digit → equal separators → white space) is rule-major across all fields
of one declaration. Every field's length and digit checks run before any equal-separator check,
which runs before any white-space check. A binding that reports a later rule while an earlier rule
also fails in another field violates it.

---

## Every binding

Each binding implements items 3, 4 and 5 in one task:

- **Item 3:** its `NumFormat` constructor defaults to strict Western, with an explicit legacy and
  Indian option (replacing, never OR-ing, the policy); bridges follow Owner decision 1 as revised
  (Western/Indian when all grouping patterns agree, legacy otherwise); the C ABI
  layout carries the bits; its version check rejects a 1.0 core; the downgrade test.
- **Item 4:** its detailed failures expose the raw `detail` code plus a named value with an
  "unknown" member, compatibly or through a separate detailed path as "Binding compatibility" says;
  its doors use the detailed exports (or, for Rust-built bindings, the `detailed` doors); its
  replayers assert `detail` on every failure row; its compatibility test.
- **Item 5:** a `DateOptions` type (flags, pivot, `COMMON_FORMS`) and a month-vocabulary type,
  accepted by its date doors.
- Messages, docs and tests follow, as in item 2.

C and C++ get the header changes, the runtime version guard and the smoke tests.

## Proof

- **Corpus:**
  - grouping rows (each policy, each door, native digits, currency separators, space-like
    separators, the span rules, the leftmost-winner rows);
  - `detail` on every existing and new failure row, plus the overlap vectors, the pinned-span
    exceptions and the `(-$5)` change;
  - date-option rows (each option, the profile, pivots at the window edges, month names including
    `janvier`, aliases, dotted names with `.`, folded matches, refused vocabularies);
  - `formats.json` rows for the new grouping refusals (policy `11`, unknown flag bits).
- **Differential tests (Rust, over every corpus file, the C ABI linked from the rlib):**
  - each text door's old export vs `cast_detailed`, **with legacy grouping policy (`00`) and no
    date options**. Agreement is required only there: strict policies differ by design (`12,34`
    succeeds through an old export, which always parses legacy, and fails through a strict
    detailed call);
  - the four typed C exports vs `cast_detailed_from_f64`, and the Rust-only typed doors vs their
    `detailed` twins;
  - `cast_checked`/`cast_checked_from_f64` vs their detailed pair;
  - the Rust doors vs `detailed::*` projected to `Fault`.

  They compare **defined values only**: fields of `Decimal`, `Timestamp`, `Duration` and
  `CivilDateTime` (never padding), floating-point bits where they apply, byte arrays as bytes.
  Fault and check fields are compared only when the return code writes them; otherwise sentinel
  contents must be untouched.
- **Tested separately, not differentially:** the strict grouping policies (the C ABI detailed path
  against the Rust strict path, and both against the corpus's strict rows), and the date-option
  forms.
- **Sentinel tests:** invalid fault sizes and combinations, larger fault buffers with untouched
  tails, null fault pointers, deliberately unaligned `out`, ignored pointers left undereferenced
  (pass a poisoned pointer to a door that ignores it).
- **The downgrade test** (item 3) and **each binding's unknown-detail decode test** (item 4).
- **`fault_span_invariant.rs`, `allocation_free.rs`, `cargo no-panic` and the canary** cover every
  new export and door.
- **Measured:**
  - the Rust doors (`cast_benchmarks`) before and after;
  - a C-ABI microbenchmark comparing each old export with `cast_detailed` on the same input, plain
    and failing;
  - every binding's existing benchmark harness before and after, on one machine;
  - the per-call vocabulary validation cost.

  Dated receipts, losses printed.

## Approaches considered

| Item | Considered | Chosen | Why |
|---|---|---|---|
| 3 | A: policy bits in `flags`; B: a second `RawNumFormat` layout (size 96) | A | no layout or Rust-literal change; the downgrade gap is closed by the runtime version guard |
| 3 | `INVARIANT` strict vs legacy | legacy, plus `STRICT` | keeps the 1.0 Rust constant's meaning |
| 4 | A: a detailed twin per door; B: four door-dispatched exports; C: detail on demand (re-parse) | B | one parse, a small ABI that new doors don't grow, follows `cast_checked` |
| 5 | always-on unambiguous shapes vs all opt-in | opt-in | preserves each door's validation contract |
| 5 | Excel-style month names vs prose layouts | Excel-style | no second date grammar this release |

## Owner decisions on the former open questions

1. **Bridges declare Indian grouping when the platform's actual number pattern specifies it** (for
   example CLDR `#,##,##0.###`). They read the pattern itself, never the locale name or the input.
   Otherwise they declare strict Western. A platform whose number and currency patterns need
   different policies, which one `NumFormat` policy field can't represent, is brought back to the
   owner, not resolved in the bridge.

   **Revision (owner, 2026-11-03, after the culture sweeps):** a bridge never knowingly rejects
   its platform's own output. It reads each platform's *complete* number and currency patterns (a
   secondary size of zero may mean "repeat the primary size", not "grouping disabled") and, across
   the single shared policy and `GROUPING` flag:
   - every grouping-enabled pattern Western → strict Western;
   - every grouping-enabled pattern Indian → strict Indian;
   - any enabled pattern of another shape (e.g. Toki Pona's groups of 2), or enabled patterns that
     disagree (e.g. ICU `ml`/`ta`/`mr`/`or`: number `#,##,##0.###`, currency `¤#,##0.00`) →
     **legacy**;
   - `GROUPING` is cleared only when neither number nor currency formatting groups; if just one
     disables grouping (e.g. `en-BG` currency), the flag stays for the other, and ungrouped values
     remain valid. The current format can't prohibit currency grouping independently; that is
     documented.

   The fallback is deterministic and documented, and never inferred from input. Each bridge tests
   these actual patterns and mixed number/currency cases, and verifies it reads representative
   platform-formatted values. This replaces the "brought back to the owner" clause above for
   disagreeing patterns.
2. **Month vocabularies are caller-built in this release.** No platform vocabulary builder:
   aliases, abbreviations and grammatical forms (`janvier` vs `de janeiro`) are decisions beyond the
   approved scope.

## Left to the builder

Internal module layout, the order of tasks within the plan, test file layout, doc wording, the
benchmark harness for the C-ABI microbenchmark, and the binding-side spelling of each new type
(within each binding's existing idiom).

## Appendix: the deferred-minor backlog (as of 2026-11-03, before item 2's final review)

From item 1 (constraints from/to the ABI form):
1. `constraints.rs`: `packed_len` sits between `Entries::packed`'s doc comment and `fn packed`.
2. `constraints.rs` check step 8 walks a `PackedText` list's length prefixes on every checked call.
3. `no_panic_canary.py` (Forge) emits a multi-line `::error::` annotation.
4. `no_panic_harness.rs` `items()` reads a null pointer with `len > 0` as empty; the Safety comment
   doesn't say so.

From item 2 (white space):

5. `text.rs:87` doc says "what every door trims".
6. Scanner coverage: lone 0x1C–0x1F/0x85 rejection; the backward scan stopping before a multi-byte
   match (`a\xC2\xA0`); trailing `\x0B` in `trim`.
7. `temporal.rs:581` long doc line; no gap row rejecting a non-white-space byte (`3:04\u200BPM`); no
   row for an unused gap run before non-edge content; `datetime.json:71` pins no fault span.
8. `verdict.rs` restates the ASCII white-space set; no `NumFormat`-level test with white space in
   `currency`; `tests.rs` order comments claim an order a bool can't show.
9. Space-like encodings hardcoded twice in `verdict.rs` (since partly shared by C1; verify);
   `group_at` doesn't gate on `GROUPING`; `round_trip.rs` reference trims edges with `trim_ascii`.
10. `corpus.cpp` new replayer drops `digit_zero`; `conformance.rs` reads a missing `expect` as ok for
    declaration rows; rule labels unverified by core/C++; no U+3000 currency-group row.
11. Padded input (white space at an edge) ~1.5× the old trim: a known, measured loss.
12. TypeScript: `lib.rs` restates `CurrencySymbol` length/digit (partly fixed in the currency
    revision); truncated regexes in `declarations.test.ts`; length and digit share one message;
    backstop wiring untested end to end.
13. Ruby: unanchored digit-message regexes; number-pair equality checked before declaring
    currency/sign fields; designator regex rebuilt per call on Fiddle; day-period ok rows only
    smoke-tested; README reflow; untracked native bundle (confirm gitignored).
14. R: the C scanner test replays only true code points; length/digit share a message;
    `cran-comments.md` PASS count measured with 17 files beside "eighteen".
15. Go: long comment; the separators' two-step white-space check deserves a comment; a
    double-negative condition in `interop.go`.
16. C1: `tests.rs` "`F\u{C2}CFA`" isn't a lone 0xC2 byte; the slow currency path runs on every miss
    even without trio characters (perf); a long doc line in `conformance.rs`; no unit test for a
    `€ X`-style symbol.
17. Swift: the replayer's `declares` restates the constructor preconditions; the sweep test checks
    agreement with predicates, not a recorded set, and skips `DateFormat.from`; a group==decimal
    guard gap; data-dependent `fr_BF`/`fr_SN` assertions; the nine-NBSP length case also hits the
    edge rule; doc wraps; `Verdict.swift:5` "whitespace".
18. Java: a long javadoc line; fully qualified `Set`/`TreeSet` and an `int[3]` counter in
    `CorpusTest`.
19. C#: `Check`'s inline ASCII range restates part of the table; cross-field order in
    `ToRaw`/`Transcode`; `ThrowWhiteSpaceAtEdge` duplicated in `DateFormat`; long doc lines.
20. Python: `currency_symbol` blames the edge rule whenever length/digit don't match; TypeScript
    `binding.d.ts` ragged wrap; Ruby ragged comment; Go double negative.
21. PHP: the digit message "an ASCII digit or whitespace" is stale (rewording needs `coercion.json`
    divergence support: owner decision); `CorpusTest.php:716` case-sensitive `'Group'`; formatter
    divergences only compared with `intl`; duplicated edge closure.
22. Plan-mandated: the Python and TypeScript release-backstop `#[test]`s need special linking, so
    CI's plain `cargo test` never runs them; wire them into CI (a Forge change) or document them.

## Appendix B: the fault-site inventory (2026-11-03)

177 fault sites: 17 `Empty` (each `blank`), 160 `Malformed`/`OutOfRange`. The full per-site list was
produced by a read-only inventory of `rust/src` (boolean, character, checked, decimal, integer, real,
temporal, text, typed, uuid, verdict). The plan's first core task re-derives the per-site table into
the code from this appendix's classes and splits. **The 23 sites that lump distinct failures and
must be split** (each split adds a detail and never changes which fault wins, its reason or its
span):

1. `integer.rs` catch-all (~:517) and `real.rs` catch-all (~:275): a second leading sign and a
   mid-token sign (`sign_misplaced`); a textual NaN/infinity spelling (`not_finite`, per
   "Non-finite values"); other letters and undeclared symbols (`unexpected_character`); `%` at an
   integer door and `e` with exponents off (`unexpected_character`); interior white space
   (`unexpected_character`).
2. `integer.rs` range sites (~:414, :447, :538) and `typed.rs` (~:74): a negative at an unsigned
   door (`negative_not_allowed`) vs above the maximum (`too_large`) vs below the signed minimum
   (`too_small`).
3. `decimal.rs` (~:180, :242) and `typed.rs` (~:152): over 96 bits (`too_large`) vs scale over 28
   (`too_precise`).
4. `typed.rs` (~:99): an ∞ input (`not_finite`) vs a finite value past `f32::MAX` (`too_large`).
5. `typed.rs` `whole()` (~:49): NaN or ∞ (`not_finite`) vs not whole (`not_whole`).
6. `typed.rs` (~:115): NaN or ∞ (`not_finite`) vs any other non-0/1 value (`not_recognized`).
7. `temporal.rs` `split_serial` (~:960): NaN or ∞ (`not_finite`) vs negative
   (`negative_not_allowed`).
8. `temporal.rs` Excel day checks (~:936, :1227): phantom serial 60 (`nonexistent_date`) vs below
   the first serial or past 9999-12-31 (`outside_window`); `:1227` also i64 overflow (`too_large`).
9. `temporal.rs` (~:1197): a sign on an Excel serial (`negative_not_allowed`) vs no digit
   (`digit_expected`).
10. `temporal.rs` (~:862): a fraction at a Unix count (`fraction_not_allowed`) vs another character
    (`unexpected_character`).
11. `temporal.rs` (~:1222, :1479): trailing text (`trailing_text`) vs an unexpected character
    (`unexpected_character`).
12. `temporal.rs` field runs (~:440, :604, :1405, :1424, :1436): zero digits (`digit_expected`) vs too
    many (`field_width`).
13. `temporal.rs` (~:477): the second separator missing (`delimiter_expected`) vs different from the
    first (`delimiter_expected`, same code, distinct rows).
14. `temporal.rs` (~:288): input too short (`wrong_length`) vs a non-digit in the year
    (`digit_expected`).
15. `temporal.rs` whole-token length checks (~:335, :753): `wrong_length`, with time-bearing,
    date-only, separated and trailing-text inputs pinned by rows.
16. `temporal.rs` (~:762): seconds missing (`component_missing`) vs a malformed time shape
    (`digit_expected`/`delimiter_expected` at the offending byte, span unchanged).
17. `temporal.rs` (~:1465): not a duration at all vs a missing `s`; both `component_missing` unless
    the winning byte says otherwise.
18. `character.rs` (~:64): not a code-point spelling (`unexpected_character`) vs a missing digit after
    a prefix (`digit_expected`) vs the input ending (`digit_expected`, whole-token span kept).
19. `character.rs` (~:70): a missing `;` (`delimiter_expected`) vs a stray character
    (`unexpected_character`).
20. `character.rs` (~:75): `not_a_scalar` for both (one code, distinct rows).
21. `uuid.rs` (~:102): generic "not a UUID" vs a D-format wrong length; both `wrong_length`, pinned by
    rows.
22. `integer.rs` (~:118): the `(-$5)` span fix (the deliberate span change).
23. The 8 defensive/unreachable sites (`real.rs:98`, `decimal.rs:193/215/222`, `uuid.rs:128/189`,
    `temporal.rs:1233`, `typed.rs:54`) each get the code their condition means.

Line numbers are as of 2026-11-03, before items 3–5 move code. The plan's core task produces the
authoritative per-site mapping as a test-checked table, and the corpus pins every class.
