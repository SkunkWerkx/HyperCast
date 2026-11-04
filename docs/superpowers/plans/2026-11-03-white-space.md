# Unicode white space in the doors: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every HyperCast door treats the 25 Unicode `White_Space` code points as white space: numeric doors ignore it outside the numeral, other doors trim it at the edges, declarations that could never match are refused. Every binding's validators, span calculations, bridges and replayers move with the core.

**Architecture:** One byte-level scanner in `rust/src/text.rs`, plus a public `hypercast::is_white_space(char)`, replaces every ASCII-whitespace site in the core. A new `corpus/formats.json` carries declaration refusals and the white-space table to all 12 replayers, which is what holds each binding's restated rules to the core's.

**Tech Stack:** Rust 2024 (`no_std`, no `alloc`, `cargo no-panic`), each binding's toolchain (CLAUDE.md, "Local loop"), criterion for the benchmark receipt.

**Spec:** `docs/superpowers/specs/2026-11-03-white-space-design.md`. Read it first: it is the authority on every rule, and its "Every binding" section names each site. This plan doesn't restate the site tables; it assigns them to tasks.

## Global Constraints

- Worktree `.worktrees/HyperCast`, branch `constraints_from_raw` on `main` (`446b4f7`), the same branch as item 1. **The owner drives git: no commit, push or PR.** Every task ends with its work verified and left in the tree.
- **HyperTabular isn't touched.** It follows at the companion pin (spec, "Grouping positions and HyperTabular").
- The approved parsing rules don't change. Where code and spec disagree, the spec wins; a spec gap is a ledgered ruling.
- The core stays `no_std`, allocation-free and panic-free. No new dependency.
- White space means exactly the 25 code points in `text.rs`'s `WHITE_SPACE` (Unicode 17.0.0). Only complete, valid UTF-8 encodings count.
- New public API, and the only one: `pub fn is_white_space(c: char) -> bool`, re-exported from `lib.rs`.
- The space-like group separators are exactly `' '` (U+0020), U+00A0 and U+202F. Declaring any of them accepts all three.
- Message wording: "white space" replaces "ASCII whitespace" (or "ASCII ... whitespace"). A decimal separator's new refusal reads "<what> must not contain white space", in the binding's existing style.
- Check order in every validator: length, then digit, then equal separators, then white space. The existing messages for those earlier rules stand.
- **Replayers assert the winning rule.** A binding whose refusals are diagnostic (an error or fault with a message: Python, Ruby on both backends, PHP, TypeScript, R, Go, C# and Java) maps each `formats.json` `rule` to its own message for that rule. It asserts that a refused row raises that rule's message, so a binding that checks equal separators after white space fails the ordering rows. Swift (`isDeclarable…` booleans), C++ (`std::nullopt`) and the Rust and C ABI checks stay boolean.
- **The culture-sweep stop condition:** a real locale that newly throws, or newly loses a symbol, stops the work. Report the locale, the exact symbol's code points and the rule it breaks to the owner. Don't work around it.
- Bindings build and test against this tree's own core, as CLAUDE.md's "Local loop" describes for each.

## Review Focus

1. **Spaces everywhere around a currency symbol and sign** (`"€ - 5"`, `"- € 5"`, `"5 € -"` with the currency and trailing-sign flags): each reads as −5, because none of those spaces is inside the numeral. Test in Task 2.
2. **A cut UTF-8 sequence at an edge** (`"12\xC2"`, `"\xA012"`): `Malformed`, with a span covering the stray byte, never trimmed. Test in Task 1 (scanner) and Task 2 (door).
3. **Parentheses holding only white space** (`"( )"`, `"(\u00A0)"`): `Malformed` over the whole input, not `Empty`. Test in Task 2.
4. **A percent at a door that doesn't read percent** (`"12 %"` at `cast_i32`): still `Malformed`. White space before `%` is ignored only where `%` is the door's to read. Test in Task 2.
5. **`SEPARATOR_DETECT` with a space between digits** (`"1\u202F234,5"` under detection, with no space-like group declared): `Malformed`. Detection resolves `.` and `,` roles only, and U+202F isn't grouping unless declared. Test in Task 2.

---

### Task 1: The scanner and the public predicate

**Files:**
- Modify: `rust/src/text.rs` (beside `WHITE_SPACE`, `is_white_space`), `rust/src/lib.rs` (re-export), `rust/src/verdict.rs:949-956` (`trim`)
- Test: `rust/src/tests.rs`

**Interfaces:**
- Produces:
  - `pub fn is_white_space(c: char) -> bool` (public);
  - `pub(crate) fn white_space_len(bytes: &[u8]) -> usize`: the byte length of the white-space code point starting `bytes`, or `0`;
  - `pub(crate) fn white_space_len_back(bytes: &[u8]) -> usize`: the same for the one ending `bytes`;
  - `pub(crate) fn skip_white_space(bytes: &[u8]) -> usize` and `pub(crate) fn skip_white_space_back(bytes: &[u8]) -> usize`: the length of the run;
  - `pub(crate) fn contains_white_space(bytes: &[u8]) -> bool`.
  - `verdict::trim` keeps its signature, `(&[u8], usize)`.

- [ ] **Step 1: Write the failing tests** in `tests.rs`:
  - `white_space_scanner_matches_the_table`: for each of the 25 chars in `WHITE_SPACE`, its UTF-8 encoding gives `white_space_len == len_utf8()`, `white_space_len_back` likewise, and `is_white_space` is true.
  - For U+200B, U+FEFF, U+180E, U+0000, U+00AD and `'a'`: `0` and `false`.
  - A lone `0xC2`, `0xE2 0x80` (cut U+202F) and an overlong `0xC0 0xA0` give `0` from both ends.
  - `trim_strips_unicode_edges`: `trim("\u{A0}\u{3000} 12\u{202F}\n".as_bytes())` is `(b"12", 6)`; `trim(b"\x0B12")` is `(b"12", 1)` (vertical tab, which today's trim keeps); `trim(b"12\xC2")` is `(b"12\xC2", 0)`.
- [ ] **Step 2: Run and see them fail.** In `rust/`, run `cargo test --lib white_space` and `cargo test --lib trim_strips` as two commands (Cargo takes one name filter). Expected: unresolved names.
- [ ] **Step 3: Implement.** Make `is_white_space` public. The scanners take an ASCII fast path (one byte, the five ASCII controls, U+000B and space) and decode only after lead bytes `C2`, `E1`, `E2` or `E3`, matching the exact encodings in the table. `verdict::trim` uses `skip_white_space` and `skip_white_space_back`. Its doc comment says "Unicode `White_Space`".
- [ ] **Step 4: Run and see them pass**: the same two commands, then `cargo test`. Expected: the new tests pass. Corpus rows may move, because every door's trim changed. List each row that moves in the ledger: Task 2 and Task 3 own them, and a move that the spec doesn't predict is a bug.

### Task 2: The numeric grammar

**Files:**
- Modify: `rust/src/integer.rs` (`frame` :73-147: `strip_percent`'s callers :84 and :88, the leading sign :119-128, the trailing sign :138, `strip_parens` :202, `strip_currency` :241 and :262; the group match at :467), `rust/src/real.rs` (group match :240), `rust/src/verdict.rs` (a group-match helper on `NumFormat`, below)
- Test: `corpus/integer.json`, `corpus/real.json`, `corpus/decimal.json`, `corpus/constraints.json` (one checked row per placement), `rust/tests/fault_span_invariant.rs` (input list)

**Interfaces:**
- Consumes: Task 1's scanners.
- Produces: `NumFormat::group_at(&self, text: &[u8], at: usize, currency: bool) -> usize`, the byte length of a group separator at `at` (`0` if none). A space-like declared separator matches any of the three; anything else matches exactly. Both group sites use it.

- [ ] **Step 1: Write the failing corpus rows**, in the files' existing shapes, at the `i32`, `f64` and `decimal` doors:
  - Each of U+00A0, U+202F, U+2007, U+3000, U+0085 and U+000B at the edges, after a leading sign, before a trailing sign, either side of a currency symbol, inside parentheses, and before `%`: value rows.
  - The same code points between two digits with no space-like group declared: `malformed`, with the span of the run.
  - `"1\u202F234,56\u00A0€"` with `'\u202F'` group, `','` decimal and `€` currency; `"1\u202F234\u202F€"` and `"1\u202F234\u202F%"` (the boundary rule).
  - For each declared space-like group, input grouped with each of the other two, and a mix. U+2007 or U+3000 between digits under those declarations is `malformed`. A declared `'.'` group stays exact.
  - Review Focus 1–5, as rows.
  - Groups are well-formed only (`1 234 567`), per the spec.
- [ ] **Step 2: Run and see them fail.** `cargo test --test conformance` in `rust/`. Expected: the new value rows fail as `malformed`.
- [ ] **Step 3: Implement.**
  - Every ASCII trim in `frame` becomes the scanner. The leading sign gains `skip_white_space` after it.
  - `strip_parens`, `strip_currency` and the percent strip use `verdict::trim`, which is now Unicode.
  - Both group sites call `group_at` and advance by its length. The numeral's digit, decimal and exponent scans are untouched.
  - A space-like group not followed by a digit is left for the trim (it is outside the numeral).
- [ ] **Step 4: Run and see them pass,** then `cargo test` (the span sweep included) and `cargo test --test allocation_free`. Expected: all pass.

### Task 3: The other doors and the day-period gap

**Files:**
- Modify: `rust/src/temporal.rs` (the leading designator gap :597, the trailing gap :629)
- Test: `corpus/date.json`, `time.json`, `datetime.json`, `timestamp.json`, `duration.json`, `boolean.json`, `uuid.json`, `char.json`, `unix.json`, `excel_serial.json`

**Interfaces:**
- Consumes: Task 1's `skip_white_space`.

- [ ] **Step 1: Write the failing rows:**
  - Each door with U+00A0 and U+3000 at both edges: the value, with the span tests unchanged.
  - Input that is only white space: `empty`.
  - `cast_char("\u3000")`: U+3000, verbatim.
  - `cast_char("\u00A0U+0041\u202F")`: `'A'`.
  - `unix` and `excel_serial` given `"- 12"`: `malformed`, per the builder's call in the spec.
  - `3:00\u202FPM`, `3:00\u00A0PM`, `3:00  PM` and a leading designator followed by U+202F, at `time` and `datetime`, with and without declared designators: values.
  - U+202F inside a date (`2026\u202F01-07`): `malformed`.
- [ ] **Step 2: Run and see them fail.** `cargo test --test conformance`. Expected: the gap rows fail; most edge rows already pass after Task 1. Record which in the ledger.
- [ ] **Step 3: Implement.** Both gaps skip any white-space run where they accepted one optional `b' '`.
- [ ] **Step 4: Run and see them pass,** then `cargo test`.

### Task 4: The core's declaration checks

**Files:**
- Modify: `rust/src/verdict.rs` (`Symbol::check` :199-216, `CurrencySymbol::new`/`check` :350-390, `NumFormat::check` :692-724 and its docs :688), `rust/src/temporal.rs` (`DayPeriods::check` :401-406 and docs :376, :399), `rust/src/abi.rs` (docs :166, :286), `include/hypercast.h` and the Swift bundle's copy (regenerated)
- Test: `rust/src/tests.rs` (:753, :838-843; :842 flips)

**Interfaces:**
- Consumes: Task 1's `contains_white_space` and the scanners.

- [ ] **Step 1: Write the failing unit tests:**
  - `NumFormat` with a minus sign, plus sign, percent or currency symbol containing U+00A0, U+202F or ASCII space: `check()` false.
  - A `decimal_sep` or `currency_decimal_sep` of `"\u{A0}"`, `"\u{202F}"`, `" "` or `",\u{A0}"`: false.
  - The same characters as `group_sep` and `currency_group_sep`: true.
  - `DayPeriods` with `"\u{202F}PM"` or `"PM\u{A0}"`: false; `"a.\u{202F}m."`: true.
  - `tests.rs:842`'s vertical-tab case now expects false (update the assertion).
- [ ] **Step 2: Run and see them fail.** `cargo test --lib`.
- [ ] **Step 3: Implement** with the check order from Global Constraints. Fix every doc comment that says "ASCII whitespace". Then `HYPERCAST_BLESS=1 cargo test --test header` regenerates both headers; read the diff (doc lines only).
- [ ] **Step 4: Run and see them pass,** then `cargo test`, `cargo clippy --all-targets -- -D warnings`, and the no-panic proof plus canary in the Linux container (item 1's commands in CLAUDE.md).
- [ ] **Step 5: Measure the completed core.** The core's parsing and declaration changes are all in after this task.
  - Make a scratch checkout of the base: `git worktree add <tmp>/hc-bench-base 446b4f7`. It's a separate directory, so the uncommitted work here is never stashed or touched.
  - Run `cargo bench --bench cast_benchmarks` there and here, on the same machine, back to back.
  - Keep both outputs for Task 11's receipt, then `git worktree remove` the scratch checkout.

### Task 5: `corpus/formats.json`, the Rust and C++ replayers, and the C smokes

**Files:**
- Create: `corpus/formats.json`
- Modify: `rust/tests/conformance.rs` (a new `formats()` test), `c/smoke/corpus.cpp` (beside its declarations replay, :931), `c/smoke/main.cpp` (:126-128, :154) and `c/smoke/main.c` (:82-84), `.github/scripts/cdylib_smoke.py` (:305, :311), `.github/hyper.json` (`r_corpus`), `r/tools/vendor.sh:31` (its corpus list)

**Interfaces:**
- Produces: the vector shapes every later replayer reads:
  - `{"kind":"white_space","code_point":"U+00A0","white_space":true}`;
  - `{"kind":"num_format","format":{…},"expect":"ok"|"refused","rule":"<rule>"}` (a refused row only carries `rule`), whose `format` uses `conformance.rs` `format_of`'s field names (`decimal_sep`, `group_sep`, `flags`, `currency`, `currency_decimal_sep`, `currency_group_sep`, `minus_sign`, `plus_sign`, `percent`, `digit_zero`);
  - `{"kind":"day_periods","am":"…","pm":"…","expect":…,"rule":"<rule>"}`.
  - `<rule>` is one of `length`, `digit`, `equal_separators`, `white_space` and `designator_edge`: the rule that refuses the declaration, which for the ordering rows is the first that applies in the Global Constraints order.

- [ ] **Step 1: Write `formats.json`:**
  - All 25 white-space code points true, and U+200B, U+FEFF, U+180E, U+0000, U+00AD false.
  - Each newly refused declaration (sign, percent and currency symbol with U+00A0, U+202F and ASCII space; `decimal_sep` and `currency_decimal_sep` with each), and the still-accepted ones (each space-like group, plain and currency; `"a.\u202Fm."`).
  - The ordering cases: five U+00A0 as `decimal_sep` gives the length rule; U+00A0 for both separators gives "must differ".
  - Day periods with white space at either edge refused, inside accepted.
- [ ] **Step 2: Write the Rust replayer** `formats()`:
  - It checks `is_white_space` against every `white_space` row, and `NumFormat::check` / `DayPeriods::check` against every declaration row.
  - It checks the C ABI too: `cast_i32` (`cast_datetime` for periods) with the format and empty input gives `-1` for refused, and not `-1` for ok.
  - It asserts each kind appears.
- [ ] **Step 3: Run and see it fail.** `cargo test --test conformance formats` before Task 4's code would fail. Since Task 4 has landed, show the failure by temporarily reverting one Task 4 check (for example the decimal-separator rule) and running it. Expected: that row fails. Restore it.
- [ ] **Step 4: The C++ replayer and the C smokes.** `corpus.cpp` replays the declaration rows through `NumFormat::declare` / `DateFormat::declare` (`std::nullopt` for refused), and skips the `white_space` rows with a comment (it delegates to the core). The smokes gain a U+00A0 decimal separator (refused) and a `"\u202FPM"` designator (refused). Add `formats.json` to `r_corpus` and to `vendor.sh`'s list. Then run `python3 -m unittest discover -s .github/scripts/tests`, which holds `r_corpus` to the corpus directory, and the C/C++ smokes per CLAUDE.md.
- [ ] **Step 5: Run** `cargo test`. Expected: all pass.

### Task 6: Python and TypeScript

**Files:**
- Modify: `rust/src/python_ext.rs` (the spec's sites; `declare` :471 backstop), `python/tests/test_cast.py`, `python/tests/test_corpus.py`, `typescript/src/lib.rs` (sites; :716 backstop), `typescript/tests/declarations.test.ts`, `typescript/tests/corpus.test.ts`, `typescript/browser-smoke/main.js` (import and replay `formats.json` as it does `declarations.json`, :17, :242, :403), `typescript/binding.js` and `binding.d.ts` (regenerated by `npm run build`)

**Interfaces:**
- Consumes: `hypercast::is_white_space` (both are Rust; use it, don't restate it).

- [ ] **Step 1: Write the failing tests.**
  - The replayers map each `rule` to the binding's message and assert it on every refused row (Global Constraints, "Replayers assert the winning rule"), the ordering rows included.
  - Each corpus replayer gains `formats.json`'s declaration rows (refused or accepted, through the binding's public constructor). Its `white_space` rows are skipped with a comment: these bindings call the core's `is_white_space` rather than restating it, and the Rust replayer holds that predicate.
  - Message tests: `test_cast.py:197-203, :547-551, :699-705`; `declarations.test.ts:22-24, :103, :106` move to the new wording, and gain a decimal-separator refusal each.
  - `test_cast.py:391-394` keeps its length message.
- [ ] **Step 2: Run and see them fail.** Per CLAUDE.md: `maturin develop` and `pytest` in `python/`; `npm run build && npm test` in `typescript/`.
- [ ] **Step 3: Implement.** The validators use `is_white_space` with the spec's rules and order. `debug_assert!(resolved.check())` becomes a check in every build that raises each binding's declaration error, worded "a format the core refuses".
- [ ] **Step 4: Release backstop proof.** Factor each backstop into a function taking the resolved `NumFormat` (`fn refuse_unless_checked(resolved: &NumFormat) -> Result<(), …>`). Give it a Rust `#[test]` with a `NumFormat` built directly from the core's public fields, holding a white-space `decimal_sep` that skips the per-field validators: `Err`. Then run that test with `--release` to show it isn't a debug assertion.
- [ ] **Step 5: The culture sweep (Python).** A test feeds `from_localeconv` every locale `locale -a` lists (skipping any that `setlocale` refuses) and requires no throw. Run it on the base and on the change (the audit), and record any difference. Per the stop condition, a new throw stops the work.
- [ ] **Step 6: Run and see them pass,** then `npm run test:bun`, `npm run lint`, the WASI fallback (`npm run build:wasi`, `NAPI_RS_FORCE_WASI=error npm test`), and Python's lint, per CLAUDE.md.

### Task 7: Ruby (both backends)

**Files:**
- Modify: `ruby/lib/hypercast.rb` (`declared` :244-261 and its flush regex; `designator` :1193-1203), `rust/src/ruby_ext.rs` (`designator` :578-608; message :604), `ruby/spec/cast_spec.rb` (:232, :253-254, :352-353, :444-445, :456-458), `ruby/spec/corpus_spec.rb`

**Interfaces:**
- Consumes: `hypercast::is_white_space` (Magnus). `hypercast.rb` restates the 25 code points as one regex character class, held to `formats.json`'s `white_space` rows.

- [ ] **Step 1: Write the failing tests.**
  - On both backends the replayer maps each `rule` to `hypercast.rb`'s or the Magnus message and asserts it on every refused row (Global Constraints), the ordering rows included.
  - `corpus_spec.rb` replays `formats.json` on both backends: the declaration rows, plus the `white_space` rows against `hypercast.rb`'s restated class.
  - The spec messages move to the new wording.
  - The digit and white-space rules get distinct messages ("must not contain an ASCII digit" / "must not contain white space"). `:352` matches the white-space one, and `:353` (`"+1"`) the digit one.
  - `:253-254` keeps "must differ".
- [ ] **Step 2: Run and see them fail.** `rake` in `ruby/`, on both backends (CLAUDE.md's Ruby commands).
- [ ] **Step 3: Implement** with the spec's order. Decimal and currency decimal separators gain the white-space refusal in `declared`.
- [ ] **Step 4: Run and see them pass** on both backends, then the Ruby lint (`rubocop`, and silent under `-w`).

### Task 8: PHP, its V1 divergences, and its culture sweep

**Files:**
- Modify: `rust/src/php_ext/num_format.rs` (`check_symbol` :335-352, `validate` :271-321), `rust/src/php_ext/values.rs` (`check_designator` :468-492), `php/tests/CastTest.php` (:387-405), `php/tests/InteropTest.php:81` (`', '` becomes a refusal case, and the test keeps its intent with a non-space separator), `php/tests/CorpusTest.php`, `php/tests/V1ParityTest.php`, `php/stubs/hypercast.stub.php` (regenerated if any signature moved; it shouldn't)
- Create: `php/tests/fixtures/v1/divergences.json`

**Interfaces:**
- Consumes: `hypercast::is_white_space`.
- Produces: `divergences.json` is an object keyed `"<fixture>:<key>"` (for example `"formatters:fr_FR CURRENCY"`), each value `{"answer": <the new recorded answer>, "reason": "<one sentence citing the spec>"}`.

- [ ] **Step 1: Write the failing tests.**
  - The replayer maps each `rule` to the extension's message and asserts it on every refused row (Global Constraints), the ordering rows included.
  - `CorpusTest.php` replays `formats.json`'s declaration rows, skipping the `white_space` rows with a comment (PHP calls the core's predicate). `CastTest.php` moves to the new wording.
  - `V1ParityTest` reads `divergences.json`: a covered key is compared with `answer` instead of 1.0's.
  - A new `testEveryDivergenceIsUsed` fails on an entry no key touched.
- [ ] **Step 2: Run and see them fail.** `cargo php-ext` in `rust/`, then `composer test` in `php/` with HyperCast's extension, per CLAUDE.md.
- [ ] **Step 3: Implement the validators.** Then run `V1ParityTest` and collect every key that differs from 1.0.
- [ ] **Step 4: Sort the parity failures.**
  - Each is either a white-space consequence (an expected CURRENCY flip in the spec's list, or a message rewording) or a bug.
  - Add an entry for each consequence with its reason. Fix a bug in the code.
  - A flip in a locale the spec doesn't predict needs its sample's code points in the ledger to show the white space that caused it.
- [ ] **Step 5: The culture sweep.** A test (`#[RequiresPhpExtension('intl')]`) walks `ResourceBundle::getLocales('')` through `fromNumberFormatter` for the decimal, percent and currency styles, and requires no throw. Run it on the base and on the change (the audit). The stop condition applies.
- [ ] **Step 6: Run and see them pass,** then `HYPERCAST_CHECK_STUBS=1 composer test` and PHP's lint.

### Task 9: C# and Java

**Files:**
- Modify (C#): `csharp/HyperCast/NumFormat.cs` (`Declarable` :292-316, `Encode`/`Check` :406-492), `DateFormat.cs` (:123-156), `Cast.Char.cs` (`Beyond` :64-75); tests `InteropTests.cs`, `BridgeTests.cs` (:27-37 test-side rule; :252-337), `CastTests.cs:304-321`, `CorpusTests.cs` (:175-177 test-side trim; new `formats.json` replay)
- Modify (Java): `NumFormat.java` (`checkSymbol` :457-482; separators :161, :167), `DateTimeFormat.java` (:112-138), `Cast.java` (:394-411); tests `CastTest.java` (:84-88, :174-176, :340-386, :496-556), `CorpusTest.java` (:154-159; `formats.json`)

**Interfaces:**
- Each language restates the 25 code points once (C#: one `static bool IsWhiteSpace(int codePoint)` plus a UTF-8 byte scanner for `Beyond`; Java: the same in a package-private `WhiteSpace` class). Both are held to `formats.json`'s `white_space` rows.

- [ ] **Step 1: Write the failing tests.**
  - Both replayers map each `rule` to their exception message and assert it on every refused row (Global Constraints), the ordering rows included.
  - Each corpus replayer gains `formats.json` (its rows: white-space and declarations), and its test-side trim uses the restated table.
  - A char-door case with a U+00A0 or U+3000 edge around a supplementary scalar checks the `Beyond` span (C#), and the same for `Cast.java:394` (Java).
  - Message tests move to the new wording.
- [ ] **Step 2: Run and see them fail.** `dotnet test` in `csharp/`; `./gradlew test testFloor checkSuiteParity` in `java/` (CLAUDE.md's flags, JDK 21 path).
- [ ] **Step 3: Implement** with the spec's order and messages.
- [ ] **Step 4: The culture sweeps.** C#'s `BridgeTests` every-culture test asserts which symbols stay undeclared. Java's `CastTest.java:174-176` requires no throw for every JDK locale. Run both on the base and on the change (the audit). The stop condition applies.
- [ ] **Step 5: Run and see them pass,** then each binding's formatter check (CLAUDE.md, "Formatters").

### Task 10: Swift, Go and R

**Files:**
- Modify (Swift): `swift/Sources/HyperCast/NumFormat.swift` (:154-190, :235-268 including `isAsciiWhitespace`, :472-512, the bridges :318-344, :528), `Cast.swift:327` (doc); tests `Tests/HyperCastTests/CorpusTests.swift` (`formats.json` via the `isDeclarable…` predicates), `CastTests.swift`
- Modify (Go): `go/interop.go` (:217-268, :312-371, :404-427), `go/cast.go` (docs); tests `go/interop_test.go`, `go/cast_test.go` (:135, :503), `go/corpus_test.go`
- Modify (R): `r/R/num_format.R` (:25-63, :153-169), `r/R/doors.R` (:292-293), `r/src/hypercast.c` (:691-756 `ascii_space`/`nul_scalar`); tests `r/tests/testthat/test-api.R` (:94-101, :177, :217), `test-corpus.R` (:203-207; `formats.json`)

**Interfaces:**
- Each restates the table once (Swift: a `static func isWhiteSpace(_:Unicode.Scalar)`; Go: `isWhiteSpace(r rune) bool`; R: one vector of the 25 code points in `num_format.R` plus a C byte scanner in `hypercast.c`), each held to `formats.json`.

- [ ] **Step 1: Write the failing tests.**
  - Go's replayer maps each `rule` to `fault()`'s message, and R's to its error, asserting it on every refused row, the ordering rows included. Swift stays boolean (Global Constraints).
  - Each corpus replayer gains `formats.json`.
  - Swift replays declarations through `isDeclarable…`, since its constructors trap.
  - R's U+0000 span case gains a U+00A0 edge.
  - Message tests move.
- [ ] **Step 2: Run and see them fail.** Swift: `.github/scripts/local-core.sh` then `swift test`. Go: `go generate` in `go/` (the core changed), then `go test ./...`. R: `r/tools/vendor.sh`, `Rscript -e 'devtools::test("r")'`.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: The Swift culture sweep.** `Locale.availableIdentifiers` through `from(formatter:)` asserts which symbols stay undeclared, run on the base and on the change (the audit). The stop condition applies.
- [ ] **Step 5: Run and see them pass,** then the Go regeneration check (`HYPERCAST_CHECK_CORE=1 go test -count=1 -run 'TestCoreIsCurrent|…' .`, per CLAUDE.md), the TinyGo and wasm legs, `R CMD check --as-cran` on the built tarball, and each formatter.

### Task 11: Docs, the CHANGELOG, the receipt and the sweep

**Files:**
- Modify:
  - every doc site the spec lists: the binding READMEs, `docs/bindings.md` (:261, :286-288, :298-299, :308, :345), `rust/src/lib.rs:47`, `rust/README.md:89`, and the public API doc comments named in the inventory;
  - `CHANGELOG.md`;
  - the benchmark table that covers the plain doors (`rust/README.md` "Benchmarks").

- [ ] **Step 1: CHANGELOG "Changed"**, listing every package, per the spec:
  - values that now read;
  - the newly refused declarations, each named;
  - the space-like group equivalence;
  - the day-period gap;
  - `hypercast::is_white_space` under "Added" *(Rust)*.
- [ ] **Step 2: Docs.** Replace each "ASCII whitespace" with the rule as it now stands, and add the decimal-separator rule where the separator rules are listed. README examples are verified by hand (CLAUDE.md).
- [ ] **Step 3: The receipt.** If any task after Task 4 changed a file under `rust/src` outside the binding modules (`python_ext.rs`, `ruby_ext.rs`, `php_ext/`), rerun Task 4 Step 5's comparison first. Then put the before and after numbers in the benchmark table as a dated row, with the machine named and any slowdown printed beside the gains (CLAUDE.md, "Claims are measured").
- [ ] **Step 4: The audit report.** One ledger line per culture sweep: base result, change result, and the difference (expected: none).
- [ ] **Step 5: The sweep.**
  - In `rust/`: fmt, the CI clippy set, `cargo test`, the wasm32 suite, the no_std proofs, MSRV 1.89, and `cargo no-panic` plus the canary in the container.
  - At the root: `check_exports.py` and `.github/scripts/tests`.
  - Then every binding's suite once more from a clean tree, after `.github/scripts/local-clean.sh`.
  - Report what only CI proves: Android, iOS, Blazor, s390x, Windows PHP, and the culture data of platforms not run locally.
