# Validation constraints — design

Status: design, approved in conversation 2026-11-01; awaiting written review.
Scope: HyperCast core, all ten bindings and the C++ header, then HyperTabular.

## Purpose

HyperTabular parses each cell into a typed value or a fault. An actuary loading data needs a
second judgement as well: the value parsed, but does it obey the business rule? Latitude is a
number that is a multiple of `0.000001` between `-90` and `90`. A euro cash amount is a multiple
of `0.05`, and anything else is wrong. A sex code is one of `Female`, `Male`, `Other`, whatever
case it arrived in.

This design adds that second judgement, using the vocabulary of the OpenAPI Schema Object. It
is declared by the caller, judged in the core, and returned in every binding's existing verdict
idiom. The world is full of bad data, so the judgement is forgiving about spelling (trimming,
case) and exact about values (decimal arithmetic, never binary floating point).

HyperCast is the governor of scalar values wherever they come from: a JSON property, an XML
attribute or element, a delimited field, a spreadsheet cell. HyperTabular is the first consumer,
not the owner. So the judgement, the constraint layout and the door numbering all live in
HyperCast, where any parser (a JSON or XML library, someone's own file reader) can reach them
directly. HyperTabular only adds per-column storage for them.

## Vocabulary and its source

The keywords are OpenAPI's. [OpenAPI 3.2.1](https://spec.openapis.org/oas/v3.2.1.html) (10
September 2026) defines the Schema Object as "a superset of the JSON Schema Specification Draft
2020-12" and takes its data types from JSON Schema Validation 2020-12, so the keywords mean what
that Validation vocabulary says (as they do in 3.1 and 3.2.0). Semantics follow JSON
Schema 2020-12 except where this document deviates on purpose; each deviation is listed in
[Deviations from JSON Schema](#deviations-from-json-schema). Callers never hand the core a schema
document: the core does not parse JSON (it allocates nothing). Each binding declares constraints
in its own idiom and maps them through the ABI. A schema-reading helper can be added in a
binding later without touching the core.

This design covers sub-projects 1 and 2 of 4:

1. Numeric: `minimum`, `exclusiveMinimum`, `maximum`, `exclusiveMaximum`, `multipleOf` with a
   rounding mode, and integer `enum`.
2. Text: `minLength`, `maxLength`, `enum`, and a `trim` option.
3. `format` (later, own design). OpenAPI 3.2.1 treats `format` as a non-validating annotation
   by default and makes support for each format optional, so the set HyperCast validates is its
   own declared choice. Several formats map to existing doors (`date-time`, `date`, `time`,
   `duration`, `uuid`, `int32`, `int64`, `float`, `double`); `email`, `uri`, `hostname`, `ipv4`
   and `ipv6` would be new grammars.
4. `pattern` (later, spike first). No existing crate supports run-time patterns, `no_std`
   without `alloc`, a `cargo no-panic` proof and ECMA-262 syntax together. The intended shape is
   a Pike VM compiled into a caller-supplied buffer, which matches in linear time with bounded
   state. It would accept the ECMA-262 subset JSON Schema recommends and refuse backreferences
   and lookaround. JSON Schema's `pattern` is unanchored, which every README must state.

Both later sub-projects extend `RawConstraints` at its end, using the size tag, without a bump.

## Semantics

### Which doors take which keywords

| Doors | Keywords |
| --- | --- |
| `I8` to `I64`, `U8` to `U64` | `minimum`, `exclusiveMinimum`, `maximum`, `exclusiveMaximum`, `multipleOf` + rounding, `enum` |
| `F32`, `F64`, `Decimal` | `minimum`, `exclusiveMinimum`, `maximum`, `exclusiveMaximum`, `multipleOf` + rounding |
| `Text` | `minLength`, `maxLength`, `enum`, `trim` |
| every other door | none |

`enum` on a number is restricted to the integer doors: an enumeration's underlying value is
integral in every language that has one.

### Refused declarations (contract violations)

A declaration that cannot mean anything is the caller's error. It is refused before any input
is judged: the C ABI returns `-1`, and each binding raises its normal contract-violation error
when the `Constraints` value is built. It is never a verdict. The refused declarations are:

- a constraint on a door outside the table above, or a keyword the door does not take;
- `minimum` together with `exclusiveMinimum`, or `maximum` together with `exclusiveMaximum`;
- a lower bound and an upper bound where the lower is not strictly below the upper, whichever
  kinds they are (`minimum: 5, maximum: 5` and `minimum: 5, exclusiveMaximum: 5` are both
  refused);
- `multipleOf` that is zero or negative;
- a rounding mode other than `strict` without `multipleOf`;
- `minLength > maxLength` (equal is allowed: fixed-width codes such as ISO country codes);
- a bound, `multipleOf` or integer `enum` entry with a fractional part on an integer door, or
  one outside the declared door's range (for example `minimum: -1` on `U8`);
- a text `enum` entry that is not valid UTF-8;
- duplicate `enum` entries (for text, duplicates after case folding). Bindings refuse this
  when they build `Constraints`. The core does not repeat the O(n²) check on each call: there
  the first matching entry wins.

### Order of judgement

The first failure wins.

1. **The door parses**, exactly as today. `Empty`, `Malformed` and `OutOfRange` keep their
   meanings and their spans, and constraints are not consulted.
2. **Trim** (text, when `trim` is declared): leading and trailing Unicode `White_Space` code
   points are removed. That is the 25 code points of the property, U+00A0 included, not the
   ASCII-only set the numeric doors trim. Trimming everything leaves empty text, which is
   `Empty`.
3. **`multipleOf`** (numeric), judged on the exact decimal value (see
   [Exactness](#exactness)):
   - `strict`: a value that is not an exact multiple is a violation of `multipleOf`.
   - `midpoint`: rounds to the nearest multiple, with ties to the even multiple (IEEE 754's
     default, banker's rounding).
   - `floor`: rounds to the multiple towards negative infinity.
   - `ceiling`: rounds to the multiple towards positive infinity.
   - If the rounded value differs from the value read, the success is marked `adjusted`.
4. **Bounds** (numeric), on the rounded value, in the order `minimum`, `exclusiveMinimum`,
   `maximum`, `exclusiveMaximum`. A breach is a violation of that keyword.
5. **Lengths** (text), on the trimmed value: `minLength`, then `maxLength`, counted in Unicode
   code points (not bytes, not UTF-16 units). A breach is a violation of that keyword.
6. **`enum`**:
   - Integer doors: the rounded value must equal an entry. No entry matching is
     a violation of `enum`.
   - Text: the trimmed value must equal an entry under Unicode simple case folding (one code
     point to one code point, as .NET's `OrdinalIgnoreCase` and Java's `equalsIgnoreCase`).
     Full folding (`STRASSE` to `Straße`) is not done. A match delivers the **declared entry**
     and its zero-based index, so `"MALE"` against `["Female", "Male", "Other"]` gives `"Male"`
     and `1`. No entry matching is a violation of `enum`.
7. **Delivery** in the door's own type, through the door's own conversion. If rounding takes an
   integer past its type (`127` rounded up to a multiple of 5 in an `I8`), that is `OutOfRange`.
   `F32` and `F64` deliver the rounded decimal converted to the nearest float by the crate's
   own conversion (`float.rs`).

Only rounding marks a success `adjusted`. Trimming and case canonicalization lose no
information about the value and are not flagged: flagging every padded cell of a fixed-width
export would bury the flags that matter.

### Exactness

Numeric judgement never uses binary floating point. Bounds and `multipleOf` are declared as
exact decimals (`Decimal`: a 96-bit magnitude, a scale up to 28, a sign). The input is judged
as the decimal text it is, at any length:

- `multipleOf = m · 10^-s`. A value `v` is a multiple when `v · 10^s` is an integer divisible by
  `m`. Any nonzero digit past scale `s` means the value is not a multiple. The remainder mod `m`
  is computed digit by digit (Horner's method), so the size of `v` is unbounded and nothing is
  allocated.
- Rounding adds or subtracts at most one step from the truncated multiple. For `midpoint` the
  tie test compares twice the remainder with `m`, plus a sticky flag for nonzero digits past
  scale `s`.
- Bounds compare sign, then the integer digits, then the fraction digits, digit by digit,
  without converting either side.
- A number that arrives already decoded (a JSON parser's `f64`, a spreadsheet's numeric cell)
  has no text. It is judged on the shortest round-trip decimal of its `f64`, through
  `cast_checked_from_f64`. That is the text HyperCast's typed doors already say an `f64` is,
  and the text HyperTabular says a workbook's numeric cell is (`docs/design.md`, the typed-cell
  table), so HyperTabular's workbook path calls the same function.

## The C ABI

Existing shapes are unchanged: `RawFault {offset: u32, len: u32}` (8 bytes), `Decimal {lo: u64,
hi: u32, scale: u8, negative: bool}` (16 bytes), `RawNumFormat` (92 bytes), return codes `0`
ok, `1` empty, `2` malformed, `3` out of range, `-1` contract violation.

### New verdict code, and why `Reason` does not change

HyperCast is at 1.0.0 and this ships as 1.1.0, so every change is additive. `Reason` (not
`#[non_exhaustive]`) and `Fault` stay exactly as they are: a new member would stop every
exhaustive match over them compiling (Rust, Swift, Java's sealed switches, TypeScript's `never`
checks, C# under CS8509-as-error), though no existing door could return it.

At the C ABI, return code `4` means a **violation**: the value parsed and breaks a declared
rule. Only the three checked exports return it, and `RawCheck.keyword` names the rule. Codes
`1` to `3` keep their meaning and remain the parsing vocabulary ("this is not a value of the
type"); a violation is the schema vocabulary ("it is a value, and it breaks your rule"). The two
never share a type above the C ABI ([Bindings](#bindings)).

### `RawConstraints`

Size-tagged like `RawNumFormat`, pointer-free so its layout is identical on 32-bit and 64-bit
targets and Go's translated core takes it as bytes. 72 bytes, 8-aligned, pinned by `offset_of!`
asserts in `rust/src/abi.rs` and carried in the header as `HyperCastRawConstraints_SIZE`.

```c
typedef struct {
    uint32_t size;          // bytes the caller built; 0 = no constraints
    uint32_t flags;         // see below
    uint32_t rounding;      // 0 strict, 1 midpoint (half-even), 2 floor, 3 ceiling
    uint32_t min_length;    // read when HAS_MIN_LENGTH
    uint32_t max_length;    // read when HAS_MAX_LENGTH
    uint32_t _reserved;     // must be 0
    Decimal  lower;         // minimum or exclusiveMinimum, per flags
    Decimal  upper;         // maximum or exclusiveMaximum, per flags
    Decimal  multiple_of;   // read when HAS_MULTIPLE_OF
} HyperCastRawConstraints;
```

Flags: `HAS_MINIMUM` (1), `HAS_EXCLUSIVE_MINIMUM` (2), `HAS_MAXIMUM` (4),
`HAS_EXCLUSIVE_MAXIMUM` (8), `HAS_MULTIPLE_OF` (16), `HAS_MIN_LENGTH` (32), `HAS_MAX_LENGTH`
(64), `TRIM` (128). An unknown flag bit, a nonzero `_reserved` or an unknown `size` is a
contract violation.

### `RawCheck`

What `cast_checked` reports beyond the value. 12 bytes, 4-aligned, nullable.

```c
typedef struct {
    uint32_t keyword;     // on code 4: 1 minimum, 2 exclusiveMinimum, 3 maximum,
                          // 4 exclusiveMaximum, 5 multipleOf, 6 minLength, 7 maxLength, 8 enum;
                          // 0 otherwise. 9 format and 10 pattern are reserved.
    uint32_t adjusted;    // on success: 1 when rounding changed the value, else 0
    uint32_t enum_index;  // on success: the matched entry; UINT32_MAX when no enum is declared
} HyperCastRawCheck;
```

### The enum list

Passed as `enum_ptr`/`enum_len` (bytes) beside `RawConstraints`, so the struct stays
pointer-free. `enum_len == 0` means no `enum`. It is read according to the door:

- Integer doors: an array of `Decimal` at scale 0, `enum_len / 16` entries, which covers every
  `i64` and `u64` exactly.
- `Text`: packed entries, each a little-endian `u32` byte length followed by that many UTF-8
  bytes, with no padding.

### Exports

Three new exports, 27 to 30. All carry `no_panic` and allocate nothing.

```c
// The constrained door. `door` and `param` are the door numbering below (4 I32,
// 11 F64, 18 Text, 19 Decimal, ...); `out` is the type that door writes
// (`RawSpan` for Text). `format` is null for the invariant notation and ignored by Text.
int32_t cast_checked(uint32_t door, uint32_t param,
                     const HyperCastRawNumFormat *format,
                     const HyperCastRawConstraints *constraints,
                     const uint8_t *enum_ptr, size_t enum_len,
                     const uint8_t *ptr, size_t len,
                     void *out, HyperCastRawFault *fault, HyperCastRawCheck *check);

// The constrained door for a number a source already decoded (a JSON parser's f64,
// a spreadsheet cell): the same judgement on the value's shortest round-trip decimal,
// the text HyperCast's typed doors already say an f64 is. `door` is a numeric door;
// a typed value has no text, so faults carry a zero span, as the typed doors' do.
// NaN is Malformed and an infinity OutOfRange, as `cast_decimal_from_f64` judges them.
int32_t cast_checked_from_f64(uint32_t door, double value,
                              const HyperCastRawConstraints *constraints,
                              const uint8_t *enum_ptr, size_t enum_len,
                              void *out, HyperCastRawFault *fault, HyperCastRawCheck *check);

// Text as a door: validates UTF-8 (invalid bytes are Malformed with their span) and
// writes the span of the text; empty input is Empty. Not trimmed.
int32_t cast_text(const uint8_t *ptr, size_t len, HyperCastRawSpan *out,
                  HyperCastRawFault *fault);
```

`cast_checked` takes 11 arguments, under the 12 Mono's interpreter passes. `RawSpan` is
`{offset: u32, len: u32}`, an index into the caller's input, which the trim narrows. With a
text `enum`, the span is the matched input and `enum_index` names the entry. Each binding
returns its own declared string for that index, not a copy.

The door numbering moves from HyperTabular into HyperCast (`rust/src/abi.rs`), unchanged, so
any parser can name a door and both crates share one definition. HyperTabular's `Door` becomes
a re-export of HyperCast's.

`RawConstraints` is validated on every call, as `RawNumFormat` is today. Only the duplicate
`enum` check is left to the bindings.

A violation spans the whole value, the span `OutOfRange` would cover (after trimming, for
text), written to `fault` as every other failure's span is. An adjusted success carries no
span: the input span is the caller's own input.

## Bindings

### The verdict: two arms, context under each

No third state, and nothing existing changes shape. The plain doors keep returning today's
verdict (`Verdict<T>` = success or `Fault`, in each binding's idiom). The checked doors return
a new two-arm verdict of their own, in Rust:

```rust
Result<Checked<T>, Rejection>

pub struct Checked<T> { pub value: T, pub adjusted: bool, pub enum_index: Option<u32> }

pub enum Rejection {           // closed: these two are the only kinds of failure
    Fault(Fault),              // it did not parse: today's Fault, unchanged
    Violation(Violation),      // it parsed and broke a declared rule
}

pub struct Violation { pub keyword: Keyword, pub offset: u32, pub len: u32 }

#[non_exhaustive]              // format and pattern join later, in a minor release
pub enum Keyword { Minimum, ExclusiveMinimum, Maximum, ExclusiveMaximum,
                   MultipleOf, MinLength, MaxLength, Enum }
```

- **Success** is `Checked<T>`: the value, `adjusted` (true only when rounding changed it), and
  for a text `enum` the matched index, with the caller's declared entry as the value.
- **Failure** is `Rejection`: either the existing `Fault`, so a parse failure is literally the
  type every door already returns, or a `Violation`, a keyword and a span. Parsing failures keep
  their own lexicon, and a rule failure is a different type, not a fifth reason.
- **Each binding** builds the same shape in its idiom: C# a `Checked<T>` | `Rejection` union
  beside `Verdict<T>`, with `Rejection` itself a `Fault` | `Violation` union; Java a sealed
  interface pair; Swift `Result`-shaped enums; TypeScript `{ ok: true, value, adjusted,
  enumIndex }` or `{ ok: false, fault }` / `{ ok: false, violation }`; R `hypercast_verdict`
  rows with `violation` and `keyword` columns that the plain doors leave `NA`; C++ a
  `noexcept` `Checked<T>` beside `Verdict<T>`; Go, Python, Ruby and PHP likewise.
- **`Keyword` stays open to additions** in each binding's idiom (`#[non_exhaustive]` in Rust, a
  `RawRepresentable` struct with static members in Swift, string constants in TypeScript), and
  every README says a match over keywords needs a default arm, so `format` and `pattern` can
  arrive in a minor release. Keywords use the OpenAPI spelling (`exclusiveMinimum`) in each
  language's casing convention.

### Declaring constraints

One immutable `Constraints` value per binding, validated once when built. Every refusal in
[Refused declarations](#refused-declarations-contract-violations), duplicates included,
surfaces there as the binding's normal contract-violation error. The shape follows each
binding's idiom: a C# `record` with init properties; a Java builder; a Swift struct; a Python
frozen dataclass with keyword arguments; a TypeScript object literal type; an R constructor
`hypercast_constraints()`; a Go struct; a PHP readonly class; a Ruby keyword-argument
constructor; C++ a struct with a validating factory. The rounding mode is each language's enum
with `Strict`, `Midpoint`, `Floor`, `Ceiling`.

Bounds and `multipleOf` accept the language's exact decimal type (`decimal`, `BigDecimal`,
`decimal.Decimal`, `Foundation.Decimal`, ...) or a decimal string (`"0.000001"`). They never
accept a binary float, so `0.1` cannot be declared inexactly by accident. Where a language has
no exact decimal, the string form is the only form.

Constrained overloads join the existing doors (C# `Cast.Double(text, format, constraints)`,
and the equivalent in each binding), and `Cast.Text(text, constraints)` and its equivalents are
new. So are typed overloads over a number the caller already holds (C#
`Cast.Double(double, constraints)`, `Cast.Decimal(double, constraints)`, an integer door over a
`double`), beside the existing typed doors and named the way each binding already names those. Fault
and violation spans remap to the caller's units exactly as today's fault spans do (C# and Java
chars, Python and Ruby code points, R's `substr()` units).

No binding is expected to be unable to follow. Go's translated core is regenerated.

## HyperTabular

- `ColumnSpec` gains `enum_offset: u32` and `enum_len: u32`, an index into one shared enum
  buffer, then `RawConstraints` inline. 104 bytes become 184, 8-aligned.
- The enum buffer travels in the existing `Buffers` struct, so neither fill export gains an
  argument; both stay under Mono's twelve.
- The spec is validated once when the reader opens (duplicates included). Each cell is then
  judged in place, through the same Rust function `cast_checked` calls, with no crossing.
- `CellVerdict` stays 12 bytes, and a plain column's verdicts are bit for bit what they are
  today. `reason` keeps codes `0` to `3`; a constrained column also writes `4` and packs the
  rest into the same word: `reason = code | keyword << 8 | adjusted << 16`. That is integer
  arithmetic, not a byte overlay, so it reads the same on s390x. It is pinned by tests and
  restated by every binding. This layout is internal to HyperTabular's core and its bindings,
  which ship together.
- The public batch API is additive: a plain column hands out today's `Verdict<T>`, and a
  constrained column hands out HyperCast's checked verdict (`Checked<T>` or `Rejection`) through
  new accessors beside today's, so HyperTabular follows in a minor release as well.
- For a text column with an `enum`, the value slot holds the matched index in place of the
  span. The binding hands back the declared entry, and the cell's raw text stays available
  through `Batch::raw`. For an integer `enum`, the value is the answer and no index is stored.

## Deviations from JSON Schema

| Point | JSON Schema 2020-12 | Here | Why |
| --- | --- | --- | --- |
| `enum` on numbers | any JSON value | integer doors only | enumerations are integral in every language that has them |
| `enum` on text | exact, case sensitive | Unicode simple case folding, delivers the declared entry and its index | forgiving input, canonical output, so the downstream map to an integer needs no second comparison |
| Whitespace | significant | optional `trim` (Unicode `White_Space`) before judgement | padded fixed-width and pasted data |
| `multipleOf` | strict only | `strict`, `midpoint`, `floor`, `ceiling`; rounding marks the success `adjusted` | latitude and longitude accept the precision loss, cash rounding does not |
| `minimum` with `exclusiveMinimum` | both allowed | refused | two lower bounds is a declaration mistake |
| lower ≥ upper | allowed, nothing validates | refused | a column that admits one value, or none, is a mistake |
| What is an integer | defined mathematically: `1.0` is an integer | the integer doors keep their own grammar, so `1.0` is `Malformed` there; it is read by `Decimal` or the float doors | constraints add a judgement after the door and never change what a door parses |
| `multipleOf` arithmetic | implementation-defined float issues are common | exact decimal at any length | `0.000001` has no binary float |

## Proof

- **Corpus.** A new `corpus/constraints.json`, replayed byte for byte by the Rust suite and
  every binding (Ruby on both backends, C++ through `c/smoke/corpus.cpp`), and added to
  `r_corpus` in `.github/hyper.json`. Each vector gives the door, param, format, constraints,
  enum list and input, then expects the value with `adjusted` and `enum_index`, a fault's
  reason and span, or a violation's keyword and span. Coverage includes every keyword at and around its boundary, each rounding
  mode at ties and on negative values, rounding that takes an integer past its type, inputs
  longer than 28 digits, trimmed and untrimmed text, U+00A0, case-insensitive matches,
  characters whose simple and full folding differ, and invalid UTF-8.
- **Refusals.** The same file holds declarations that must be refused (every item in
  [Refused declarations](#refused-declarations-contract-violations)). The core and every
  binding's `Constraints` builder must refuse the same set.
- **Existing proofs extended.** `fault_span_invariant.rs` sweeps every truncation of every new
  input through `cast_checked` and `cast_text`. `allocation_free.rs` counts zero for all three
  new exports. `cargo no-panic` covers all three and `java/jni/examples/no_panic.rs` their JNI
  entry points.
- **Typed rows.** `corpus/constraints.json` carries `from_f64` rows as `corpus/typed.json`
  does: the same constraints over an `f64`, including values whose shortest round-trip decimal
  sits exactly on a bound or a rounding tie, NaN and the infinities.
- **Independent exactness check.** A test-only dev-dependency on an arbitrary-precision decimal
  checks the digit-by-digit remainder, rounding and bound comparisons against it on generated
  inputs. The core keeps no dependencies.
- **The case-folding table** is generated from a pinned Unicode version's `CaseFolding.txt`
  (statuses `C` and `S`). A test regenerates it and fails on any difference, and it joins
  "Never hand-edit" in `CLAUDE.md`.
- **Negative controls.** Each check above is shown to fail when what it checks is broken.
- **Measured.** A constrained cast against a plain one through the C ABI, and a constrained
  column in HyperTabular's million-row harness, recorded as dated receipts with any loss printed
  beside the wins.

## Rollout

1. **HyperCast, in one pass:**
   - The core: `cast_checked`, `cast_checked_from_f64`, `cast_text`, the folding table, the
     door numbering and `corpus/constraints.json`.
   - All ten bindings, `include/hypercast.hpp` and `c/smoke/main.c`.
   - Every site in "Lists that move together" (`check_exports.py` enforces it).
   - Regenerated outputs: the cbindgen header, Go's translated core, the napi output and the
     PHP stub.
   - READMEs (the export count among them) and a CHANGELOG entry under Unreleased.
   - HyperUuid has no numeric or text doors and does not follow.
2. **HyperCast 1.1.0**, a minor release: every change above is additive.
3. **HyperTabular, a minor release:** `ColumnSpec`, `CellVerdict`, the enum buffer, the
   checked accessors and every binding, plus the HyperCast bump in every manifest.
4. **Later, each with its own design:** `format`, then the `pattern` spike.
