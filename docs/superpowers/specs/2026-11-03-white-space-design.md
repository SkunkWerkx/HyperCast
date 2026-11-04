# Unicode white space in the doors (HyperCast 1.1.0)

Date: 2026-11-03 · Size: a project (stepped up from a small change once the binding inventory
showed every binding restating the rules) · Scope: the core, a new corpus file and all 12
replayers, every binding's declaration validators, span calculations, locale bridges and docs.
One additive public function (below); no other API change.

## Intent

Data pasted from Excel, Word and the web carries Unicode spaces: U+00A0 at a cell's edge, and
French and Swiss numbers written `12,5 %` and `1 234,56 €` with U+202F or U+00A0. Every door
trims only ASCII whitespace today, so all of those come back `Malformed`. This is item 10 of
`CRAN_CHECKLIST.md`, which the owner widened from "trim U+00A0" to the following.

## What changes, by door

"White space" means the 25 code points with the Unicode `White_Space` property: the table
`text.rs` already holds for the checked text door's `trim`. Only a complete, valid UTF-8
encoding of one counts, so a stray `C2` or a non-UTF-8 edge is never white space. Spans keep
indexing the caller's original bytes, and nothing is copied.

- **Numeric doors** (the eight integers, `f32`, `f64`, `Decimal`, and their checked twins):
  - **The numeral is atomic**: digits, the declared group and decimal separators, and an
    exponent, with no white space inside. The one exception is a declared space-like group
    separator (`' '`, U+00A0, U+202F, or any declared group separator that is white space),
    which is grouping. Its positions are item 3's (strict grouping) to judge. `"12 34"` stays
    `Malformed` unless such a separator is declared.
  - **The space-like group separators stand for each other** (owner's decision). Declaring
    any of `' '`, U+00A0 or U+202F as the group separator (plain or currency) accepts all
    three in the input, and only those three. A declared non-space group separator stays
    exact. Real French data mixes them: CLDR and ICU (the PHP, Java and Swift bridges) declare
    U+202F, Excel and Word write U+00A0, older systems an ASCII space.
  - **A space-like group separator not followed by a digit is not grouping.** It is white
    space outside the numeral. So with U+202F declared as the group separator,
    `"1\u202F234\u202F€"` reads as 1234 €: the second U+202F precedes the currency symbol, not
    a digit group.
  - **Outside the numeral, white space of any kind and any amount is ignored**, ASCII
    included, so `"- 12"`, which is `Malformed` today, reads too (owner: numeric doors remove
    all white space): at the edges,
    after a leading sign, before a trailing sign, on either side of a currency symbol, inside
    accounting parentheses, and before a percent symbol.
  - So `"-\u00A012"`, `"( 12 )"`, `"12,5\u202F%"` and `"1\u202F234,56\u00A0€"` (with
    `'\u202F'` declared as the group separator and `€` as the currency) read.
- **Every other text door** (dates, times, timestamps, durations, Excel serials as text, Unix
  counts, UUID, boolean, char): leading and trailing white space is always trimmed, with no
  opt-in. Interior spaces stay structural: space-separated timestamps are item 8's.
  - **One exception, the day-period gap** (owner's decision): between a time and a trailing
    AM/PM designator, and after a leading one, any run of white space is accepted where one
    optional ASCII space is today (`temporal.rs:597, :629`). So `3:00\u202FPM`, as ICU 72+,
    JDK 20+ and Apple platforms format en_US times, reads. Designators can't have white space
    at their edges, so without this a caller could never declare that shape.
  - **Unix counts and Excel serials as text are edge-trimmed only** (builder's call). They take
    no `NumFormat` grammar (no currency, percent or parentheses), so `"- 12"` stays `Malformed`
    there while it reads at the numeric doors.
  `cast_char` given exactly one character stays verbatim, so `"\u3000"` is U+3000, as `" "`
  is a space today.
- **Input that is nothing but white space** is `Empty` at every door that trims.
- **Text** (`cast_text`, `cast_text_checked`): unchanged. Trimming stays the caller's opt-in
  `trim`, which already uses this set.
- **Declarations** (`decimal_for_declaration`, bounds, steps): unchanged and strict.
  `"1\u00A0"` is still refused.

## Declarations follow the input

A symbol the input can never match is refused at declaration. Since white space around symbols
is now ignored:

- A sign or percent symbol containing any white space is refused (today: only ASCII white
  space). These sit flush against the digits.
- **A currency symbol with white space at either edge is refused; inside, white space is
  allowed** (revised 2026-11-03, see "Revision: currency symbols").
- **A decimal separator containing any white space is refused**, `decimal_sep` and
  `currency_decimal_sep` alike (today `NumFormat::check` accepts one through
  `decimal_sep.check(false)` and `currency_decimal_sep.check_optional(false)`). The numeral
  admits white space only as a declared group separator, so such a declaration could never
  match. No CLDR locale uses one, and allowing it as a second exception would make `"1 5"`
  ambiguous between a fraction and two groups whenever the group separator is space-like too.
  Group separators, plain and currency, may still be or contain white space: that is the
  structural exception above.
- A day-period designator with white space at either end is refused (today: only ASCII). Inside
  stays allowed (Spanish `a. m.`, or `a.\u202Fm.`).
- The core's checks (`Symbol::check`, `CurrencySymbol::new`/`check`, `DayPeriods::check`, and
  `NumFormat::check` for the two decimal separators) and
  every binding-side validator that restates them change together, with "white space" for
  "ASCII whitespace" in their messages. The sites are listed under "Every binding".

## Every binding

The rules above are the core's. The inventory (2026-11-03) found that every binding but C and
C++ restates the declaration rules with ASCII hard-coded, and four re-implement the trim. Each
must move with the core in the same pass, or the binding breaks in one of two ways: it accepts
a declaration the core refuses, which surfaces as a "contract violation, binding bug" at cast
time, or in Python and TypeScript (whose doors call core Rust without re-checking in release)
it passes silently; or it computes a span the core didn't.

### One definition, shared or restated

- **A new public predicate**, `hypercast::is_white_space(c: char) -> bool`: true for exactly
  the 25 `White_Space` code points. It is additive and stable (a Unicode property). The core
  uses it, and so do the Rust-built bindings: Python, Ruby's Magnus backend and PHP inside the
  crate, and TypeScript through the public API.
- **Bindings in another language restate the table**: C#, Java, Swift, Go, R (R code and its
  C shim), and Ruby's Fiddle backend (`hypercast.rb`). Each restated table is held to the
  corpus's list of white-space code points (below), so a drifted table fails that binding's
  own corpus replay. Swift's current `isAsciiWhitespace`, which counts `\v` unlike the core,
  goes.
- **C and C++** delegate to the core (`hpp` `declare()` probes a door), so they get the new
  refusals with no change. Their smoke tests gain the new refusals.

### Declaration validators, in release builds too

Every binding-side validator refuses exactly what the core refuses: a sign or percent symbol
containing white space; a currency symbol or a designator with white space at either end (see
"Revision: currency symbols"); a decimal or
currency decimal separator containing white space. Each uses its binding's existing refusal
idiom and message style, with "white space" for "ASCII whitespace". The sites, from the
inventory:

| Binding | Validator sites |
| --- | --- |
| Python | `python_ext.rs` `optional` (:487), `separator` for both decimal separators (:455-458), `designator` (:536) |
| Ruby | `hypercast.rb` `declared` (:244, the flush regex) and `designator` (:1193); `ruby_ext.rs` `designator` (:578) |
| PHP | `php_ext/num_format.rs` `check_symbol` (:335) with the decimal separators (:272, :281-286); `php_ext/values.rs` `check_designator` (:468) |
| TypeScript | `typescript/src/lib.rs` `optional_symbol` (:623), `separator` for both decimal separators, `designator` (:841) |
| R | `r/R/num_format.R` `symbol_bytes` (:48) and the decimal separators (:153, :159); `r/R/doors.R` designators (:292) |
| Go | `go/interop.go` `symbolOK`/`symbolOKSlow`/`symbolFault` (:229-268), `NumFormat.valid`/`fault` (:312-371), `designatorOK`/`DateFormat.fault` (:404-427) |
| Swift | `NumFormat.swift` `isDeclarable(separator:)`/`(sign:)`/`(currencySymbol:)`/`(designator:)` (:235-268, :508) and the preconditions (:154-190, :472-479) |
| C# | `NumFormat.cs` `Declarable` (:292) and `Encode`/`Check` (:406-484); `DateFormat.cs` `Declared`/`Encode` (:123-156) |
| Java | `NumFormat.java` `checkSymbol` (:457) with the decimal separators (:161, :167); `DateTimeFormat.java` `refusal` (:112-138) |

**Release builds:** Python's `declare` (`python_ext.rs:471`) and TypeScript's
(`typescript/src/lib.rs:716`) end in a `debug_assert!(resolved.check())`, so a validator that
lags the core goes unnoticed in release. Each becomes a real check in every build, raising that
binding's declaration error ("a format the core refuses") when it fails. The per-field
validators give the specific message first; this is the backstop.

**Ordering:** where a declaration breaks two rules, existing tests pin which message wins (for
example Python's `test_cast.py:391`, five U+00A0 as a decimal separator, expects the length
message; Ruby's `cast_spec.rb:253`, U+00A0 decimal and group separators, expects "must
differ"). The white-space rule is checked after the length, digit and equal-separator rules, so
those messages stand.

### Spans the bindings compute

Each of these trims the input itself to report a span, and moves to the same white-space set:

- Java `Cast.java:394-411` (a supplementary scalar a `char` can't hold: out of range over the
  trimmed token).
- C# `Cast.Char.cs:64-75` `Beyond` (the same).
- R `r/src/hypercast.c:691-756` `ascii_space`/`nul_scalar` (U+0000's out-of-range span).
- The corpus replayers that trim on the test side: C# `CorpusTests.cs:175-177`, Java
  `CorpusTest.java:154-159`, R `test-corpus.R:203-207`.

Each gets a corpus-driven case: a char-door input whose edge is U+00A0 or U+3000 and whose
expected span excludes it.

### The new corpus file

`corpus/formats.json`, a new vector shape, added to `r_corpus` in `.github/hyper.json` (whose
test holds the list) and replayed by all 12 replayers:

- `rust/tests/conformance.rs`, `c/smoke/corpus.cpp`, and the C#, Java, Swift, Go, Python, Ruby
  (both backends), PHP, TypeScript and R suites.

It holds three kinds of vector, using the field names the existing vectors' `format`, `am` and
`pm` already use:

- `{"kind": "white_space", "code_point": "U+00A0", "white_space": true}`: all 25 code points
  true, and near misses false (U+200B, U+FEFF, U+180E, U+0000, U+00AD). Every binding holds its
  predicate or table to these.
- `{"kind": "num_format", "format": {…}, "expect": "ok" | "refused"}`: each newly refused case
  (a sign and percent symbol containing U+00A0, U+202F and ASCII space; a currency symbol with
  each at an edge, rule `currency_edge`; a decimal and currency decimal separator of each), each
  still-accepted one (a white-space group separator, plain and currency; a currency symbol with
  inner white space such as `F CFA`; a designator with an inner space), and the ordering cases
  above. The rule names are `length`, `digit`, `equal_separators`, `white_space`,
  `designator_edge` and, since the revision, `currency_edge`.
- `{"kind": "day_periods", "am": "…", "pm": "…", "expect": "ok" | "refused"}`: designators with
  white space at either end, refused; inside, accepted.

A replayer constructs each declaration through its binding's public API and expects that
binding's refusal idiom: an exception in C#, Java, Python, Ruby, PHP and R; an `Outcome` error
in TypeScript; a `fault()`/`valid()` answer in Go; `isDeclarable…` returning false in Swift
(whose constructors trap); `std::nullopt` in C++. The Rust replayer checks both
`NumFormat::check`/`DayPeriods::check` and the C ABI (a door's `-1`).

The existing corpora gain the parsing vectors under Proof, so every replayer sees them with no
new shape.

### Locale bridges

The owner's decision: each bridge keeps its current failure policy. C# and Swift leave an
undeclarable platform symbol undeclared. Java's `NumFormat.from(Locale)`, PHP's
`fromLocaleconv`/`fromNumberFormatter`, Python's `from_localeconv` and Ruby's `from_i18n`
throw.

- **The before-and-after audit** (builder's call). During implementation each sweep is run on
  the base commit and on the change, and the two are compared. That comparison is what finds a
  locale that "newly fails"; no per-platform snapshot is committed (.NET's ICU and NLS, and
  Swift's macOS and Linux, ship different data).
- **Culture sweeps.** Each bridge has, or gains, a test that walks every locale the platform
  ships through it: C# `BridgeTests.cs:320-337` (every culture), Java `CastTest.java:174-176`
  (every JDK locale), Swift (`Locale.availableIdentifiers` through `from(formatter:)`), PHP
  (`ResourceBundle::getLocales('')` through `fromNumberFormatter`, under `intl`). A throwing
  bridge's sweep requires no throw. A dropping bridge's sweep asserts which symbols it leaves
  undeclared. Python's `from_localeconv` reads only the process's locale, so it is swept over
  whatever locales the machine has (`locale -a`; often only C and POSIX on CI), with no Forge
  change (builder's call). Ruby has no sweep: `from_i18n` takes the caller's own hashes, so
  there is no platform data to walk (builder's call).
- **A failing sweep stops the work.** It is not allow-listed: the finding goes to the owner
  first, as below.
- **Any real locale that newly fails** (it throws where it didn't, or loses a symbol it kept) is
  a finding for the owner, reported with the exact platform-provided symbol (code points), the
  locale, and the rule it breaks. It is not worked around in the bridge.

### PHP's V1 parity fixtures

`php/tests/fixtures/v1/` holds 1.0's own recordings, and `V1ParityTest` requires every answer to
match exactly. It has no way to express an intended change, so it gains one (the builder's
design, recorded as such):

- **`php/tests/fixtures/v1/divergences.json`**: each entry names the fixture, the key, the new
  answer, and the reason. For example `formatters.json` `"fr_FR CURRENCY"`, the currency sample
  that 1.0 answered `Malformed` because of the U+00A0 before `€`, now `Success`, citing this
  spec.
- `V1ParityTest` compares against 1.0's answer unless an entry covers the key, and then against
  the entry's new answer, so a divergence is pinned, not skipped. A second test fails on any
  entry that no key used, so stale entries can't pile up.
- The inventory's expected flips are the CURRENCY samples of de_DE, de_AT, fr_FR, fr_CH,
  sv_SE, nl_NL, es_ES, pt_BR, ru_RU and et_EE (`formatters.json`); the bidi-mark locales
  (ar_EG, fa_IR, he_IL) are expected not to move. Each actual flip gets an entry. A flip that
  isn't a white-space consequence is a bug, not a divergence.
- The `formats.json` message fixtures (`:16, :26, :27, :48, :49`) and `formatters.json:341-344`
  change wording ("white space"), each an entry with that reason.

### Tests that pin the old behaviour

They change with the rule, each named in the plan. From the inventory: PHP
`InteropTest.php:81` (`', '` as a decimal separator, now refused); the core's
`tests.rs:842` (vertical tab, now refused); the message regexes in Ruby `cast_spec.rb:352-353`,
TypeScript `declarations.test.ts:22`, R `test-api.R:177`, Go, Java and C#; C#'s
`BridgeTests.cs:27-37` test-side designator rule.

### Grouping positions and HyperTabular

- **Grouping vectors use only well-formed groups** (`1 234 567`) (builder's call). Where a
  group may sit is item 3's (strict grouping), which comes next. This change pins no loose
  position such as `12\u202F34`.
- **HyperTabular follows at the companion pin** (owner's sequencing: HyperCast's 1.1 work lands,
  then one board run and one pin). Its regenerated header comments, the restated symbol rule
  in `swift/Sources/HyperTabular/Column.swift:193`, and its corpus and CHANGELOG are checked
  and changed with the HyperCast bump there, not in this pass.

## Revision: currency symbols (2026-11-03, owner's decision)

This revises the original rule, which refused any currency symbol *containing* white space.
The culture sweeps found that rule broke real locales: CLDR's West African CFA franc symbol
is `F CFA`, written `U+0046 U+202F U+0043 U+0046 U+0041`, used by 23 ICU locales (PHP's
`fromNumberFormatter`), 32 JDK locales (Java's `NumFormat.from`), 23 .NET cultures and 23
Apple locales (Côte d'Ivoire, Senegal, Mali, Burkina Faso, Niger, Benin, Togo, Guinea-Bissau).
The bases of all four accepted it. The original reason for the refusal, "a symbol the input
can never match", holds only for white space at a symbol's edges, which is ignored around it.

The revised rule:

- **Declaration.** A currency symbol with white space at either edge is refused (rule
  `currency_edge` in `corpus/formats.json`). White space inside it is allowed. This applies to
  `CurrencySymbol::new`/`check` and to every binding's validator. The message names the edge:
  "<what> must not start or end with white space", in each binding's style.
- **Matching.** Inside a declared currency symbol, ASCII space, U+00A0 and U+202F match each
  other in the input, one character for one character. So `F CFA` declared with U+202F reads
  `1 000 F CFA` written with any of the three. Every other interior character, other white
  space included (U+2009, U+3000, …), stays exact. Runs aren't collapsed (`F  CFA`, two
  spaces, doesn't match a one-space symbol), and white space is never optional (`FCFA` doesn't
  match `F CFA`).
- **Spans** keep indexing the caller's original bytes. A matched symbol's byte length is its
  length in the input, which differs from the declaration's when, for example, U+202F (3
  bytes) is matched by an ASCII space (1 byte).
- **Signs and percent symbols are unchanged**: any white space in them is refused. They sit
  flush against the digits, and no sweep has shown a real one with white space.

Proof added:
- corpus rows at the integer, float and decimal doors: `F CFA` declared with U+202F, reading
  input with each of the three spaces (with and without a space-like group separator), and
  the same with the symbol declared with ASCII space and with U+00A0;
- non-equivalent interior white space (U+2009, U+3000 in the input against a U+202F
  declaration) is `Malformed`; a symbol declared with U+2009 inside matches only U+2009;
- two spaces and no space against a one-space symbol are `Malformed`;
- spans: faults after a symbol matched by a different-length space point at the right
  original bytes;
- `formats.json`: `F CFA` with each of the three spaces accepted, and with white space at the
  leading or trailing edge refused (`currency_edge`);
- every binding's validator accepts `F CFA` with each space and refuses edges, with its
  message;
- the culture sweeps of PHP, Java, C# and Swift are rerun on base and change, and must show no
  difference.

## Approaches

| | What | Trade-off |
| --- | --- | --- |
| **A (chosen)** | One byte-level scanner in `text.rs` (`white_space_len(bytes) -> usize`: the bytes of the white-space code point starting the slice, `0` if none, plus the mirror for a slice's end), with an ASCII fast path that decodes only after a lead byte of `C2`, `E1`, `E2` or `E3`. `verdict::trim` and every numeric-grammar site that skips ASCII white space today call it, and so do the declaration checks. | Several grammar sites change, each needing corpus vectors. One definition of white space. No copying, so no span remapping. |
| B | Strip white space into a scratch buffer, then parse with the ASCII grammar. | The core can't allocate and numeric input is unbounded, so no fixed buffer suffices. Every span would need remapping. Dropped. |
| C | Unicode at the edges only, the grammar left ASCII. | Leaves `12,5\u202F%` broken. Dropped. |

The owner chose A.

## Proof

- **Corpus vectors**, replayed by every binding:
  - each numeric door with U+00A0, U+202F, U+2007, U+3000, U+0085 and U+000B at each allowed
    place;
  - the same between digits without a space-like group separator (`Malformed`, with its
    span);
  - a declared U+202F group separator, including one directly before a currency symbol or `%`
    (outside the numeral, ignored);
  - each of `' '`, U+00A0 and U+202F declared, reading input grouped with each of the other
    two and with a mix; U+2007 or U+3000 between digits under the same declarations
    (`Malformed`, not in the set); a declared non-space separator still exact;
  - `3:00\u202FPM`, `3:00\u00A0PM`, `3:00  PM` and a leading designator followed by U+202F, at
    the time and datetime doors, with and without declared designators;
  - each non-numeric door's edges;
  - white space alone (`Empty`);
  - `cast_char` with one white-space character;
  - a lone `C2` and an overlong encoding at an edge (not white space);
  - refusals for each widened declaration check: a sign or percent symbol with white space, a
    currency symbol with white space at an edge (never inside; see the revision), a designator
    with white space at an edge, and a `decimal_sep` or
    `currency_decimal_sep` containing white space (U+00A0, U+202F, ASCII space), in the new
    `corpus/formats.json` ("Every binding");
  - a white-space group separator, plain and currency, still accepted, in the same file.
- **Binding validation**, span calculations, the 12 replayers, release-build checks, culture
  sweeps and the V1 parity divergences: as "Every binding" sets out.
- **`fault_span_invariant.rs`** sweeps the new inputs.
- **`allocation_free.rs`** and **`cargo no-panic`** stay green.
- **A unit test** holds the scanner to `text.rs`'s `WHITE_SPACE` table, every code point's
  encoding at both ends, so the two can't drift.
- **Measured:** the plain doors' benchmark before and after on one machine, recorded as a dated
  receipt with any slowdown printed.

## Docs

- A CHANGELOG "Changed" entry listing every package: values that were `Malformed` now read,
  and declarations that were accepted are now refused. The entry names each newly refused
  case: a sign or percent symbol containing white space, a currency symbol with white space at
  either edge (the revision below), a designator with white
  space at an edge, and a decimal separator (plain or currency) containing white space.
- The binding READMEs' whitespace sentences.
- `docs/` where it says "ASCII whitespace".

## Left to the builder

The scanner's exact name and signatures, test layout, and the benchmark harness to use (the
crate's existing criterion benches).
