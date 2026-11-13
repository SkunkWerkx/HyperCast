# HyperCast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Swift Package](https://img.shields.io/github/v/tag/SkunkWerkx/HyperCast?label=swift%20package&sort=semver)](https://github.com/SkunkWerkx/HyperCast/tags)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**`Verdict<T>` is a real Swift `enum`, so an exhaustive `switch` over it is
*compiler-mandatory* — not an opt-in analyzer flag, not a review convention. The value, or a
closed reason plus the exact byte span that offended.**

Allocation-lean scalar casts — booleans, characters, the full integer family, reals, exact
decimals, UUIDs, temporals — calling directly into the native `hypercast` Rust core,
no shim layer. On every platform it supports — Linux
(glibc and musl), macOS, Windows, iOS, Mac Catalyst, Android and WebAssembly — the core is linked into your executable
as a static library, so there is nothing to load at run time and nothing to deploy beside
it.

```swift
import HyperCast

switch Cast.i32("(1,234)", format: .invariant) {
case .success(let value): print("got \(value)")  // -1234, accounting negative
case .fault(let fault): print("\(fault.reason) at byte \(fault.offset): \(fault.detail?.name ?? "")")
}  // no default: the compiler mandates both arms, and only both arms

switch Cast.timestamp("2026-01-02T15:04:05.123456789+05:00") {
case .success(let value): print("timestamp \(value)")  // a Foundation Date, normalized to UTC
case .fault(let fault): print("\(fault.reason) at byte \(fault.offset)")
}
```

Door names mirror the native ABI (`i32`, `f64`, `decimal`, `timestamp`, …); every door also
takes raw UTF-8 `[UInt8]` for callers already holding bytes. Swift-flavored fidelity:
an instant is a `Date`, a `Double` of seconds, so it holds under a microsecond today and about
30 µs at the window's end, and never rounds into the next second. `Duration` is attosecond-backed, so the duration door keeps every nanosecond the core
parses; the `Duration` presentation is also why `Package.swift` carries a `.macOS(.v13)`
floor, and `.iOS(.v16)` and `.macCatalyst(.v16)` beside it (Linux has no availability
gates — these only set the Darwin deployment targets).
`Cast.decimal` presents Foundation's `Decimal`, whose 38-digit mantissa holds every value
the core's 96-bit, 28-place decimal produces exactly — `0.1` is one tenth, `50%` is exactly
`0.5`, and excess precision is `outOfRange` rather than rounded. The core's result is
canonical — exact trailing fraction zeros are trimmed, so `1.10`, `1.1` and `1.1000` are all
magnitude 11 at scale 1 — which is exactly what `Decimal` represents, so nothing is lost
between the core and the presentation. `Cast.nativeVersion()` reports the linked core's
own `major.minor.patch` (`hypercast_version`), so a caller can prove the binary it resolved
is the one this binding was built against before the first cast — see
[Linking and deployment](#linking-and-deployment). A caller that is itself generic over its target uses
`Cast.numeric<T>(_:format:)`, resolved statically over the closed `NumericCastTarget` set —
exactly the eleven numeric targets (`Int8`…`Int64`, `UInt8`…`UInt64`, `Float`, `Double`,
`Decimal`), each routed to its own door; any other `T` is a compile error, not a runtime
one.

```swift
func column<T: NumericCastTarget>(_ cells: [String], as _: T.Type, format: NumFormat) -> [Verdict<T>] {
    cells.map { Cast.numeric($0, format: format) }
}
```

`Cast.scalar<T>(_:format:)` widens that to every door a type alone can choose, over the
closed `ScalarCastTarget` set: `Bool`, the eleven numeric targets, `UUID`, `Date` (the
RFC 3339 `timestamp` door, zone mandatory), `Duration` and `Unicode.Scalar` (the `char`
door). `format` is read by the numeric doors only. `DateComponents` is deliberately not a
target: the `date`, `time` and `dateTime` doors all return it, so the type cannot say which
was meant, and a caller names that door instead.

`Cast.char` reads one `Unicode.Scalar`: an input that is exactly one scalar is taken
verbatim before any trimming (`" "` is a space, `"6"` the digit six), and otherwise the
trimmed text must be one code-point spelling — `65`, `U+0041`, `0x41`, `&H41`, `&#65;`,
`&#x41;` — with a spelling past `U+10FFFF` or naming a surrogate `outOfRange`. Every scalar
fits `Unicode.Scalar`, so nothing narrows.

## Numbers a workbook already holds

A spreadsheet stores a numeric cell as a `Double`, and four plain doors read one directly, with no
text in between (the checked `…FromDouble` doors are under [Validation constraints](#validation-constraints)): `Cast.decimalFromDouble` (the shortest decimal that names the double, the
digits the file holds — `0.1` is one tenth), `Cast.excelSerialFromDouble(_:epoch:)` (the
`DateComponents` `dateTime` returns, under a declared `ExcelEpoch`), `Cast.excelTime` (a
serial's fraction, as `time` returns it) and `Cast.excelDuration` (days as a `Duration`). A
typed door's `Fault` has no span: its offset and length are 0.

## Validation constraints

Untrusted input is judged in two steps: first it is cast to the type (a `Fault` when it is not
one), then the value is validated by OpenAPI 3.2's rules. `Constraints<T>` declares the rules
with the OpenAPI Schema Object's keywords, bound by its type to one door (`Constraints<Double>` for `f64`,
`Constraints<Decimal>` for `decimal`, `Constraints<String>` for `text`, …), and the checked
overload of that door judges the value by it, in the core and in exact decimal arithmetic:

```swift
import Foundation
import HyperCast

// Latitude: a multiple of 0.000001 between -90 and 90, finer input rounded half to even.
let latitude = Constraints<Double>(minimum: -90, maximum: 90, multipleOf: "0.000001", rounding: .midpoint)
// Euro cash: a multiple of 0.05, and anything else is wrong.
let euroCash = Constraints<Decimal>(multipleOf: ConstraintValue(Decimal(string: "0.05")!))
// A sex code: one of three, whatever case and padding it arrived in.
let sex = Constraints<String>(trim: true, enum: ["Female", "Male", "Other"])

let euro = NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all, currencySymbol: "€")
Cast.f64("45.1234567", format: .invariant, constraints: latitude)  // .success(Checked(value: 45.123457, adjusted: true, …))
Cast.f64("-90.5", format: .invariant, constraints: latitude)  // .rejection(.violation(minimum, value: -90.5, offset: 0, length: 5))
Cast.f64FromDouble(45.1234567, constraints: latitude)  // .success(Checked(value: 45.123457, adjusted: true, …))
Cast.decimal("€1.234,55", format: euro, constraints: euroCash)  // .success(Checked(value: 1234.55, adjusted: false, …))
Cast.decimal("€12,37", format: euro, constraints: euroCash)  // .rejection(.violation(multipleOf, value: 12.37, offset: 0, length: 8))
Cast.decimal("12,3x", format: euro, constraints: euroCash)  // .rejection(.fault(malformed, offset: 4, length: 1))

let message =
    switch Cast.text(" MALE ", constraints: sex) {
    case .success(let checked): "\(checked.value), entry \(checked.enumIndex!)"  // Male, entry 1
    case .rejection(.fault(let fault)): "\(fault.reason) at byte \(fault.offset)"  // empty at byte 0, for "   "
    case .rejection(.violation(let violation)):
        switch violation.keyword {
        case .enum: "not a sex code: '\(violation.value)'"  // for " Femal ": not a sex code: 'Femal'
        default: "breaks \(violation.keyword)"  // keywords are open
        }
    }
```

A checked door answers in a `CheckedVerdict<T>`, an enum beside `Verdict<T>` (which every plain
door keeps returning, unchanged): `.success(Checked<T>)`, the `value`, `adjusted` (true only
when rounding changed it) and, at the text door with an `enum`, the matched `enumIndex`; or
`.rejection(Rejection<T>)`, itself an enum of the plain doors' own `.fault(Fault)` (the input is
not the type: the very type, so a cast failure reads the same at either door) and
`.violation(Violation<T>)` (a value of the type that broke a rule: its `keyword`, the `value`
exactly as the door cast it, and the value's span, in UTF-8 bytes like a fault's). The violation's
`value` is never the rounded one, since a `multipleOf` rounding is part of the judgement, and at
the text door it is the judged text (trimmed under `trim`), never an `enum` entry.
A switch over the three is exhaustive with no `default`. `Keyword` is a `RawRepresentable`
struct with static members (`.minimum` … `.enum`, each with its OpenAPI `name`, such as
`exclusiveMinimum`), not an enum: `format` and `pattern` may join in a minor release, so a
switch over keywords needs its `default` arm, and a code this binding does not know yet is kept
(`keyword 9`).

The numeric doors (the integers, `f32`, `f64` and `decimal`) read `minimum`, `exclusiveMinimum`,
`maximum` and `exclusiveMaximum` (four independent keywords, any of them together),
`multipleOf` with a `Rounding` (`.strict`, the default, then `.midpoint`, half to even, `.floor`
and `.ceiling`) and `enum` (decimal entries compared exactly to the value); the text door reads
`minLength`, `maxLength` (in code points) and `enum`, matched under Unicode simple case folding
and delivering the caller's own declared string and its index (a number among its entries is
valid and never matches, as JSON Schema's exact equality says), with two extensions of its own:
`trim` (Unicode `White_Space`, U+00A0 included) and `caseSensitive` (an `enum` matched byte for
byte, for a vocabulary such as `mm` and `Mm`). As in JSON Schema, a keyword for the other type is
ignored. The first failure wins: the door casts, the text is trimmed, `multipleOf` rounds or
refuses, then the bounds, the lengths and `enum` judge the result; a rounding past what the type
holds (`127` at `Int8`, `multipleOf: 5`, `.ceiling`) breaks `multipleOf`. A float door also
judges the float it delivers against each bound read as the door reads it: `"1e-400"` at
`Double` reads as `0`, so it breaks `exclusiveMinimum: "0"`. Bounds and steps are
`ConstraintValue`s: an integer literal, a `Decimal`, or text in invariant notation, all
converted exactly by the core (`"1e-6"` is `0.000001`, and `"1e-29"`, past the 28 places a core
decimal holds, is refused rather than rounded). There is no float literal and no conversion from
`Double`, so `0.1` cannot be declared inexactly; `Decimal(0.1)` itself goes through a `Double`,
so write `"0.1"` or `Decimal(string: "0.1")`. Every numeric door has its checked overloads over
a `String`, `[UInt8]` and `UnsafeRawBufferPointer` with a `NumFormat`, and `i32FromDouble`,
`f64FromDouble`, `decimalFromDouble` and the rest for a number a source already decoded, judged
on the shortest decimal that names it (`0.1 + 0.2` breaks `multipleOf: "0.1"` under `.strict`,
and rounds to `0.3` under `.midpoint`), with an empty span.

A declaration is judged once, by the core, when it is built. Every valid OpenAPI 3.2 declaration
is accepted, unsatisfiable ones included: crossed bounds, `minLength` above `maxLength`, a bound
past the type's range, duplicate entries (the first match wins), and `enum: []`, which no value
matches (an `enum` left `nil` is none). What the core refuses is the caller's bug, and the
initializer's precondition fails, as `NumFormat`'s does, with the rule as its message
(`Constraints<Decimal>(multipleOf: "1e-29")` traps with "Constraints refused (code 8): a bound,
step or enum entry the door cannot hold."): `trim` or `caseSensitive` at a numeric door, a
rounding with `multipleOf` at the text door (code 1), a `multipleOf` not positive (5), a rounding
without `multipleOf` (6), a decimal past 96 bits or 28 places, or a rounding at an integer door
with a fractional `multipleOf` (8). After that a checked cast judges only data and traps on
nothing for it. `Cast.text` without constraints is the plain text
door, a `Verdict<String>`: the input itself when it is valid text (`Cast.text("  Male ")` is
`.success("  Male ")`, untrimmed); through `[UInt8]`, invalid UTF-8 is `.malformed` at its first
invalid sequence, as at every plain door, which hands its bytes to the core unchanged. A Swift
`String` cannot hold a lone surrogate, so none can reach the core from one. `Cast.optional` takes
a checked verdict as it takes a plain one: an `.empty` fault is `nil`, and every other rejection,
a violation included, flows through. `Interop.rawConstraints(_:)` and `Interop.packedEnum(_:)` hand a declaration over in the
core's own 104-byte layout and `enum` list, for a library judging values across a C ABI of its
own. A declaration reads back through properties of its keyword names; `` `enum` `` is the list as
passed, `nil` when none is declared and empty for an empty one, which every value violates.

## Why not `Int32("...")` / `ISO8601FormatStyle`?

1. **Verdicts with location** — Swift's failable initializers return `nil` with no reason
   and no span; formatters throw.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, signs, percent symbols and native digits, radix
   prefixes, a culture's own `AM`/`PM`, all five .NET `Guid` text forms plus `urn:uuid:`
   prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (every corpus file replayed, with byte-exact fault spans).
4. **Faster on the culture-machinery doors, and allocation-free** — numbers from
   ordo-one's package-benchmark (linux-x64 on an Intel Core i9-11900H, p50, `swift package
   benchmark run` in `Benchmarks/`, Swift 6.3.3), Foundation's closest parser measured in
   the same run:

   | Door | HyperCast | mallocs/call | Foundation, same run |
   | --- | ---: | :---: | ---: |
   | `Cast.dateTime` (messy civil) | **82 ns** | 0 | 33 µs `DateFormatter` (`M/d/yyyy h:mm a`, hoisted), 103 mallocs |
   | `Cast.uuid` | **31 ns** | 0 | 479 ns `UUID(uuidString:)` — 15x |
   | `Cast.timestamp` | **37 ns** | 0 | 627 ns `Date.ISO8601FormatStyle` — 17x |
   | `Cast.decimal` | **39 ns** | 0 | 419 ns `Decimal(string:)` — 11x |
   | `Cast.f64` | **30 ns** | 0 | 68 ns `Double(String)` — 2.3x |
   | `Cast.i32` | 20 ns | 0 | 7 ns `Int(String)` — honest loss, see below |
   | `Cast.i32` (grouped) | 26 ns | 0 | — |
   | `Cast.date` (declared order) | 65 ns | 0 | — |
   | `Cast.duration` | 33 ns | 0 | — |
   | `Cast.bool` | 15 ns | 0 | — |

   **What makes the doors this cheap** is the carrier, not the parse: nothing is allocated
   around the call. The input crosses as a view of the string's own UTF-8 (`withUTF8`), the
   scratch is a tuple of fixed-width integers on the stack, and the call is a direct call to
   a linked-in symbol.

   Two things to read straight: `Int(String)` at 7 ns is still faster than the invariant
   integer door, as it should be — a stdlib integer parse with no grouping, no parens and
   no radix prefixes is the floor, and the door only wins once the text needs any of those.
   And the `DateFormatter` control really does measure tens of microseconds and 103
   mallocs on this toolchain; the door's own numbers are the claim here, the Foundation
   figure is reported as measured.

   Separator detection shows its true cost, because the carrier is thin enough to see it:
   `1.234.567,89` under `.detect` is 54 ns against 31 ns for the same text under a declared
   eurozone format.

   A locale's format declares its currency symbol whether or not the text carries one, and
   that costs about 3 ns: in the run that measured it for 0.9.0, `1,234,567` is 28 ns under
   `.invariant` (the table's grouped row, timed again) and 31 ns under
   `NumFormat.from(locale:)` for en_US, `$` included. Rejecting `1,234x567` takes 52 ns with
   no allocation. `Int("1234x567")` fails in about 1 ns, at the first character it does not
   take, where the door reads on to report the fault's reason and span.

**The honest trade-off:** a native dependency — a prebuilt static library per platform,
linked into your executable — and a call across the C ABI per cast. For plain invariant
integers, `Int32("...")` is the reasonable choice.

A fault also says what failed in its span: `fault.detail` is a `FaultDetail`, the core's
shared vocabulary (`.groupSize`, `.digitExpected`, `.tooLarge`, …, 32 published codes), and
`fault.detailCode` its raw number. The reason stays the broad classification and the detail
never changes it or the span. `FaultDetail` is an open struct, like `Keyword`: any code
decodes without trapping and keeps its `rawValue`, `name` is `nil` for a code this binding
doesn't publish (and `isPublished` false), the fault around it is still whole, a code a later
release names becomes the equal member, and a `switch` over a detail needs a `default`.
`Fault`'s equality is its reason and span, as it always was, so a fault you built with
`Fault(reason:offset:length:)` (no detail, `detail == nil`) equals the door's for the same
failure; compare `detailCode` to compare details. A violation of a declared rule names a
keyword, not a detail.

Every door also takes an `UnsafeRawBufferPointer` — the primitive the `String` and
`[UInt8]` forms wrap — so a caller already holding a buffer (a mapped file, one field of a
delimited line) casts a slice of it without copying anything out first.

## Declared formats and currency

The numeric doors take a `NumFormat` — declared separators plus `NumStyles` lenience flags
— with no default argument: `.invariant` (`.` decimal, `,` grouping, every lenience on,
Western grouping) or `NumFormat.from(locale:)`, which reads the locale's own symbols. The core carries no culture
table, so everything a culture spells differently is declared, never looked up — and every
declared spelling is read *beside* the ASCII one, which the core reads whatever is declared.
A declared minus sign does not stop `-` being read, a declared percent symbol does not stop
`%`, native digits do not stop `0`–`9`: nothing that parsed before stops parsing, and a
format that declares nothing reads exactly what it always did. `NumFormat`, `NumStyles`,
`DateFormat`, `DateOptions` and `MonthNames` are `Hashable` values, equal when they declare the
same thing.

White space is Unicode's `White_Space`, the 25 code points (ASCII, U+0085, U+00A0, U+2000 to
U+200A, U+202F, U+3000 among them). Every door but `Cast.text`, which trims only under `trim: true`, ignores it at both ends (`Cast.char` given exactly one character keeps it), and the
numeric doors also after a leading sign, before a trailing sign or a percent symbol, on either
side of a currency symbol and inside accounting parentheses (`"- 12"`, `"( 12 )"`, `"12,5 %"`,
`"1 234,56 €"`, the last two under a declared comma decimal separator, the last with a space group separator and `€` declared); between digits it is `.malformed`, unless a space-like group separator is
declared. An ASCII space, U+00A0 or U+202F declared as the group separator makes the input's
choice of those three immaterial (Excel writes U+00A0, ICU's `fr` U+202F, an older system a
space), a space-like separator with no digit after it is just white space, and any other
separator stays exact. Input of nothing but white space is `.empty`.

- **Grouping.** `NumFormat` takes the policy beside the styles, `grouping:`, and it defaults
  to `.western`: groups of exactly three digits counted from the decimal separator (the
  leftmost one to three, not starting with a zero), so `1,234,567` reads and `1,23,456` and
  `1234,567` are `malformed` at the group (`.groupSize`, spanning the group and its
  separator: `,23`), as is `0,125` (spanning `0`). `.indian`
  reads `1,23,456` and `12,34,567` and refuses `1,234,567`. A numeral that uses no separator is valid under both,
  grouping after the decimal separator or in an exponent is never allowed
  (`.groupInFraction`), and the policy only judges groups: with `.grouping` off a group
  separator stays forbidden, and `.separatorDetect` resolves its own `1,234`. The policy
  is separate from the styles (a `NumStyles` never carries it, and one that sets a bit the
  core doesn't define is a precondition failure; bit 8, 256, is retired permanently and never reused); `withStyles(_:)` keeps the policy,
  `withGrouping(_:)` replaces it. `NumFormat.from(locale:)` and `from(formatter:)` follow the
  number pattern (`groupingSize`, `secondaryGroupingSize` where 0 repeats the primary size):
  three then twos is `.indian`, anything else `.western`. The patterns are Foundation's, and they vary:
  on macOS 27.2 `en_IN`, `hi_IN`, `ml`, `mr`, `or` and `ta` are three then twos for numbers and currency
  both, and on Linux (swift-foundation, Swift 6.4.2) all of them are threes, so declare
  `.withGrouping(.indian)` there. A Western amount through a format whose numbers are Indian is `malformed`
  until the column says `.withGrouping(.western)`.
  A shape neither policy reads (Toki Pona's groups of two) is `malformed` under a declared format, `.invariant` or a bridge, never misread, while `.detect` infers by the delimiter rule, so `1,2345` is 1.2345 there: [docs/grouping.md](https://github.com/SkunkWerkx/HyperCast/blob/master/docs/grouping.md#parked). `.grouping` is cleared only
  when neither the number nor the currency pattern groups (`en_US_POSIX`); if just one does not (`en_BG`'s currency) the
  flag stays, so an ungrouped value stays valid — a format can't forbid grouping in a
  currency amount alone. The rule depends on the patterns only, never on the input.
- **Separators** (`decimalSeparator`, `groupSeparator`) are short strings: one or two
  characters, up to 8 UTF-8 bytes, no ASCII digit — a comma and a space, a narrow no-break
  space — and never equal to each other. The group separator may be white space (Unicode
  `White_Space`, U+00A0 and U+202F included), and a group declared as a space, U+00A0 or
  U+202F reads as any of the three; a decimal separator, the currency one included, may
  not contain any, since a numeral admits white space only as a group.
- **Currency.** With `.currency` in the styles (part of `.all`) the declared
  `currencySymbol` is accepted once at either edge of the number — leading, before or after
  a sign (`$5`, `-$5`, `$ -5`), or trailing (`5 €`, `1.234,50 kr.`) — with optional
  white space between symbol and digits, and accounting parentheses wrap the two together
  (`($5)`). A symbol declared with the flag off is `malformed` at the symbol; the flag with
  no symbol declared changes nothing. When the symbol is in the text, the digits are read
  with `currencyDecimalSeparator` and `currencyGroupSeparator` where those are declared (a
  decimal comma in numbers and a decimal point in prices, as `en_NL` writes them); empty
  keeps the number separators. Under `.separatorDetect` detection governs the whole token
  and the currency separators are ignored.
- **Signs.** `minusSign` and `plusSign` are read wherever ASCII `-` and `+` are — ahead of
  the digits, before or after a leading currency symbol (U+2212 in Estonian and Basque).
  Two signs in one number, in either spelling, are `malformed` at the second.
  `.trailingSign` reads the sign after the digits (`5-`, `5 -`, `5 €-`, `2.5%-`), never
  beside a leading sign or inside parentheses. It is opt-in and deliberately not in `.all`:
  a trailing sign is a culture's own negative pattern, and `from(locale:)` declares it from
  the locale's rather than assuming it.
- **Percent.** `percentSymbol` is read as a trailing `%` is while `.percent` is set (`٪` in
  Arabic-script locales, `％` fullwidth). Real and decimal doors only; an integer door never
  reads a percent.
- **Native digits.** `digitZero` declares the ten code points from it up, read beside
  `0`–`9` everywhere digits are — integer part, fraction, exponent (U+0660 for the
  Arabic-Indic `٠`–`٩`, U+FF10 for the fullwidth digits); a radix-prefixed body stays ASCII.

Every rule is a precondition on the initializer, a caller bug the same way equal separators
are: a sign or percent symbol is 1 to 8 bytes with no ASCII digit or white space, the currency
symbol at most 16 with no ASCII digit and no white space at either end (inside is fine, as
in `F CFA`), a decimal separator no white space, the digit zero non-ASCII and the start of a digit block.
The core checks the same rules and reports a violation as a contract error, which no
well-typed call here can reach.

```swift
import Foundation

let danish = NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all, currencySymbol: "kr.")
switch Cast.decimal("(1.234,50 kr.)", format: danish) {
case .success(let amount): print(amount)  // -1234.5, exact — no double was ever formed
case .fault(let fault): print("\(fault.reason) at byte \(fault.offset)")
}
let enUs = NumFormat.from(locale: Locale(identifier: "en_US"))  // "$", from the locale's own data
Cast.i32("-$5", format: enUs)  // .success(-5)
let estonian = NumFormat.from(locale: Locale(identifier: "et_EE"))  // a U+2212 minus, no-break-space groups
Cast.i32("−1\u{00A0}234", format: estonian)  // .success(-1234); "-1234" reads too
let egyptian = NumFormat.from(locale: Locale(identifier: "ar_EG"))  // Arabic-Indic digits, ٫ and ٬, its own percent
Cast.decimal("١٢٫٥", format: egyptian)  // .success(12.5)
let trailing = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: [.all, .trailingSign])
Cast.i32("5-", format: trailing)  // .success(-5); malformed under .invariant
```

`NumFormat.from(locale:)` reads what Foundation's own number parsing reads, through a
`NumberFormatter` in the decimal style and `Locale.numberingSystem`: the separators and the
currency symbol, `minusSign`, `plusSign` and `percentSymbol`, `currencyDecimalSeparator` and
`currencyGroupingSeparator`, a non-empty `negativeSuffix` with no `negativePrefix` as
`.trailingSign` (the `n-` and `n -` patterns), and the numbering system's zero for the
digits (`arab`, `arabext`, `fullwide`, `thai`, `mymr`, `khmr`, `laoo`, `tibt`, `mong` and
the Indic scripts; `latn` declares none). A symbol the locale spells as the ASCII default is
not declared, since that is read anyway, nor is one the native field cannot carry — a locale
is process/user state and no cast should trap on it. ICU's symbols are declared as spelled,
bidi marks included (an Arabic-script locale's minus is an Arabic letter mark then `-`), so
a locale's own formatted output parses under its format, and so does the plain ASCII
spelling. `NumFormat.from(formatter:)` does the same from a `NumberFormatter` you already
configured, overrides and all.

The boolean door stays English (`true`/`false`, `yes`/`no`, `on`/`off` and the rest):
no platform carries a lexicon of boolean words, so there is nothing a bridge could fill a
declared one from, and `ja` or `oui` is a caller's mapping ahead of the door, not a
format's.

### Date options and month names

The date doors keep their strict shapes unless the caller opens more, with a `DateOptions`
(`Cast.timestamp(_:options:)`, `date(_:options:)`, `date(_:order:options:)`,
`dateTime(_:order:options:)`, `dateTime(_:format:options:)`, each over `String`, `[UInt8]`
and `UnsafeRawBufferPointer`): `.compactDate` (`20260107` at the date door),
`.spaceSeparator` and `.hourOffset` (`2026-01-07 03:04:05+00` at the timestamp door),
`DateOptions.commonForms` for all three (no pivot, no month names), a `pivot` (100 through
9999, the last year of a 100-year window: `26` is 2026 under 2029, `30` is 1930; `nil` or 0 declares none) for
two-digit years at the order doors, and `months`, a `MonthNames`. A door reads only the
options that are its own. `MonthNames` is built by the caller from `(month, name)` entries
(`Jan`, `Sept`, `janv.`): each name 1 to 64 bytes with no ASCII digit, `/`, `-` or white
space, every month named, no two names equal under simple case folding, or the
initializer's precondition fails. Names are read in the month field in the position `order`
selects (`07-Jan-26`), longest first among those followed by the date's own separator, with
their punctuation kept: with `janv.` declared and `.` as the separator, `7.janv..26` reads
and `7.janv.26` is `malformed`. The vocabulary is checked by the core on every call, so
build the `DateOptions` once.

### Day periods

`Cast.dateTime` also takes a `DateFormat` — the `DateOrder` plus the day-period designators
a culture writes beside a 12-hour clock, `amSymbol` and `pmSymbol` — beside the `order:`
overload, which is the same door with nothing declared. A declared designator is
read after the clock (`3:04 nachm.`) or ahead of it (`오후 3:04`), with any run of white space
or none between (so the `3:04\u202fPM` Apple platforms write for `en_US` reads), by
its exact bytes, beside the English `AM`/`PM`, which are read whatever is declared, ASCII
case-insensitively, after the clock or ahead of it. A designator is 1 to 16
UTF-8 bytes with no ASCII digit and no white space at either end (a space inside is
fine: Spanish writes `a. m.`). `DateFormat.from(locale:)` reads the order off the locale's
short-date pattern, as `DateOrder.from(locale:)` does, and the designators off a
`DateFormatter`'s `amSymbol` and `pmSymbol`, both or neither: a locale whose designators
exceed the 16-byte field declares none and reads English only.

```swift
import Foundation

let korean = DateFormat.from(locale: Locale(identifier: "ko_KR"))  // 오전 / 오후, year-month-day
Cast.dateTime("2026-01-07 오후 3:04", format: korean)  // .success(2026-01-07 15:04)
let german = DateFormat(order: .dayMonthYear, amSymbol: "vorm.", pmSymbol: "nachm.")
Cast.dateTime("1/7/2026 3:04 nachm.", format: german)  // .success(2026-07-01 15:04)
Cast.dateTime("1/7/2026 3:04 PM", format: german)  // .success too: English is always read
```

### Taking the `Locale` itself

Every numeric door, the generic `numeric` and `scalar` doors, and `dateTime` also take the
`Locale` itself, the type Foundation's own parsing is configured with:
`Cast.i32("1.234", locale: Locale(identifier: "de_DE"))` is `.success(1234)`, exactly
`Cast.i32("1.234", format: .from(locale: Locale(identifier: "de_DE")))`, and
`Cast.dateTime(text, locale:)` is the door under `DateFormat.from(locale:)`. The locale is
not optional, as Foundation's are not: pass `.current` or `.autoupdatingCurrent` to mean the
user's settings, since nothing here falls back to one. Deriving a format builds a
Foundation formatter — 50–60 µs and 71 allocations for a `NumberFormatter` on this
toolchain, where reading the separators alone was 270 ns and allocation-free — so the doors
cache formats by locale identifier, and a `locale:` call with an explicit identifier costs
150–250 ns over a prebuilt format: the lookup, and about 70 ns of telling the locale apart
from the user's. `.current` and `.autoupdatingCurrent` are never cached: the per-user
number-format overrides Apple platforms allow on them are not in the identifier, so those
two derive on every call — the user's settings are read when they are asked for — at 60–70
µs per call here, a thousand times the door. A hot loop over the user's locale derives once
with `NumFormat.from(locale:)` and passes the format, which is also the fastest form in any
loop. (The figures are the harness's `Cast.i32 grouped (locale: …)` and
`NumFormat.from(locale:)` rows, p50, from a separate run on the same machine as the table
above but with other work running. That run's own `Cast.i32 grouped` read 41 ns, against the
table's 26 ns, so read these figures against 41 ns.)

## Interop: building on HyperCast's C ABI

The `Interop` namespace is for a package that carries HyperCast's verdicts across a C ABI
of its own (HyperTabular does): it reads the value layouts the core writes
through exactly the conversions every `Cast` door applies, so a value read out of another
library's buffer is the value the door of the same name would have returned. Each reader
takes an `UnsafeRawBufferPointer` starting at one value, aligned for its widest field.

- Out-values: `Interop.scalar` → `Unicode.Scalar`, `decimal` → `Decimal`, `uuid` → `UUID`, `instant` → `Date` (a `Double`, which never rounds into the next second), `date`,
  `civil` and `time` → `DateComponents`, `duration` → `Duration`.
- `Interop.rawFormat(_:)` — a `NumFormat` as the core reads it in place, the 92-byte
  `Interop.RawNumFormat` tuple: a size word, the flags, the digit zero, then every symbol
  inline with its length after it, up to `Interop.symbolMaxBytes` (8) for a separator, a
  sign or the percent symbol and `Interop.currencyMaxBytes` (16) for the currency symbol.
- `Interop.rawDayPeriods(_:)` — a `DateFormat`'s designators as the core reads them, the
  40-byte `Interop.RawDayPeriods` tuple, each up to `Interop.designatorMaxBytes` (16).
- `Interop.rawConstraints(_:)` — a `Constraints` as the core reads it in place, the 104-byte,
  8-aligned `Interop.RawConstraints` tuple (`Interop.rawConstraintsSize`): a size word, the
  flags, the rounding code, the two lengths, a reserved word, then `minimum`, `maximum`,
  `multipleOf`, `exclusiveMinimum` and `exclusiveMaximum` in the layout `Interop.decimal` reads;
  `Interop.packedEnum(_:)` is its `enum` list beside it (16-byte native-endian decimals at a
  numeric door, a little-endian
  `UInt32` length then UTF-8 bytes per entry at the text door).
- `Interop.fault(code:offset:length:)` — the `Fault` a nonzero verdict code and its byte
  span name; any code other than 1–3 is a precondition failure, a binding bug.
  `Interop.fault(code:offset:length:detail:)` and `Interop.fault(code:faultDetail:)` (a
  detailed export's 16-byte `Interop.RawFaultDetail`: the capacity word, offset, length,
  detail) add the detail code, which an unknown one never refuses.
- `Interop.RawDateOptions` — the 12-byte date options (`Interop.rawDateOptionsSize`).
- `Interop.version(_:)` — a `*_version()` export's packed `major << 16 | minor << 8 | patch`
  word as `"major.minor.patch"`.

```swift
// buffer: the [UInt8] another library's C ABI wrote values into; offset: where one decimal starts
let amount = buffer.withUnsafeBytes { Interop.decimal(UnsafeRawBufferPointer(rebasing: $0[offset..<offset + 16])) }
let format = Interop.rawFormat(.invariant)  // 92 bytes, ready for the other library's ABI
```

## Requirements

- **Swift 6.2 or later.** The manifests declare `swift-tools-version:6.2`: the first release
  whose package manager can link a static library as a binary target (SE-0482), which is how
  the core reaches every platform. On Swift 6.4, CI runs the suite on Linux (glibc), macOS,
  Windows and WebAssembly, on an iOS simulator and as a Mac Catalyst process, and in an
  Android emulator, and on musl, whose static SDK has no XCTest, the smoke executable. On 6.2,
  in Swift's own containers, it runs the suite on glibc and the smoke executable on musl and
  WebAssembly. macOS and Windows are tested on 6.4 only.
- **Platforms.** Linux on glibc and on musl (Swift's static Linux SDK), macOS and Windows,
  each on x86_64 and arm64, WebAssembly (`wasm32-unknown-wasip1`), in WASI hosts and in
  the browser, and iOS, the iOS simulator and Mac Catalyst on arm64. The declared
  deployment floors are macOS 13, iOS 16 and Mac Catalyst 16, for `Duration`. Android on
  arm64 and x86_64 builds with the Swift SDK for Android (Swift 6.3 or later, API 28 or
  later); see [Linking and deployment](#linking-and-deployment).
- **Not supported: everything else.** tvOS, watchOS, visionOS, the iOS simulator
  and Mac Catalyst on Intel Macs, and any other architecture on the supported systems have
  no prebuilt core here, so the build stops at compile time with no `HyperCastCore` module
  (Swift Build first warns that the artifact bundle has no matching variant) — never at
  run time.

## Linking and deployment

There is nothing for you to do. The package declares the core as a SwiftPM binary target —
one static library per triple, in `HyperCastCore.artifactbundle.zip`, attached to the GitHub
Release and named in the manifest by URL and checksum — and SwiftPM downloads it, checks it,
and links the one for your target into your executable. There is no shared library to find
at run time, no resource bundle, and nothing to deploy beside the binary: a multi-stage
Dockerfile that
copies only the executable works, so does a fully static build with
`swift build --swift-sdk x86_64-swift-linux-musl`, and so does copying a macOS or Windows
executable on its own. Each triple's archive is about 410–580 KB (the Windows `.lib`s are
the largest; CI's build of 2026-11-13), and the linker keeps only what your executable reaches.

No door throws: bad data is a `Verdict`'s `Fault` (or a checked door's `Violation`), and the core is linked in, so there is
nothing to fail to load. `Cast.isAvailable` is always `true`; it keeps the availability
probe every binding carries.

The package needs the 1.1 core, and a 1.0 one can't be linked by mistake: the doors call the
core's detailed exports directly, a 1.0 core doesn't have them, so the build fails (to
compile, and to link) instead of a numeral being read by 1.0's rule. There is no run-time
version check to forget.

iOS, the iOS simulator and Mac Catalyst get the same archives from a second binary target,
`HyperCastCoreApple.xcframework`: an app for those is built by Xcode, which links a static
library out of an XCFramework and does not read a static-library artifact bundle. The
manifest declares it only on a Mac, where those platforms can be built at all, and both
targets define the one `HyperCastCore` module the binding imports. CI's `test-apple-mobile`
job runs the suite on an iOS simulator and as a Mac Catalyst process with `xcodebuild test`,
and builds the package for an iOS device.

Android uses the same artifact bundle: it carries the core for `aarch64-unknown-linux-android`
and `x86_64-unknown-linux-android`, and a package built with the
[Swift SDK for Android](https://www.swift.org/documentation/articles/swift-sdk-for-android-getting-started.html)
(`swift build --swift-sdk aarch64-unknown-linux-android28`; Swift 6.3 or later, API 28 or
later) links it like any other triple. Page alignment is the final link's, which the SDK
does with the NDK's linker; NDK r28 and later align to the 16 KB pages Android 15 devices
may use by default. CI cross-builds this suite with the SDK for x86_64 and runs it in an emulator
whose image uses 16 KB pages, through `.github/scripts/android_build_suite.sh` and
`android_device_test.sh`, which run the same way against a local emulator; the aarch64
build is linked. The Swift runtime on Android is shared libraries, which an app packages
the way the SDK's documentation describes; the core adds nothing to them.

In a checkout of this repository the core is build output, never committed:
`.github/scripts/local-core.sh` clears every binding's staged build artifact
(`.github/scripts/local-clean.sh`), builds the host's archive from the checkout's `rust/` into
`swift/HyperCastCore.artifactbundle/` (gitignored but for its `include/`), and then `swift test`
from `swift/` links it. CI writes each leg's archives there the same way. Without one the
package still resolves, and a build stops at `import HyperCastCore`. The root `Package.swift`
links that local bundle when it is there too, so CI's smoke tests build through the consumer's
manifest against the core the run built; a dependency's checkout never has one.

On Swift 6.3 with `--build-system swiftbuild` (opt-in there), a package that depends on
this one fails with `missing required module 'HyperCastCore'`: that release's Swift Build
drops a binary target's module map when it is reached through a product
([swift-build#1295](https://github.com/swiftlang/swift-build/pull/1295), fixed in 6.4).
The default build system of every release from 6.2 on is unaffected, and so is 6.4's
Swift Build (its default); 6.2's opt-in Swift Build predates static-library artifact
bundles altogether.

## WebAssembly

The binding compiles to WebAssembly from Swift 6.2: install swift.org's WebAssembly SDK
and build with it.

```sh
swift sdk install <the Wasm SDK URL and checksum from swift.org/install>
swift build --swift-sdk swift-6.4.0-RELEASE_wasm     # `swift sdk list` names yours
```

The core is linked in as a static library — the same binary target Linux uses, with a
`wasm32-unknown-wasip1` archive — so there is no module to load and no engine to embed.
The core needs neither a clock nor randomness, so it asks nothing of the host. CI runs the whole suite under WasmKit on Swift 6.4, and a smoke executable on 6.2, whose
XCTest does not start under WASI.

### In the browser

What the WebAssembly SDK builds is a plain `wasm32-wasip1` command module (it imports only
`wasi_snapshot_preview1`), so a browser runs it through a WASI shim — no JavaScriptKit and
no change to the package. With [`@bjorn3/browser_wasi_shim`](https://github.com/bjorn3/browser_wasi_shim):

```js
import { WASI, File, OpenFile, ConsoleStdout } from "@bjorn3/browser_wasi_shim";

const wasi = new WASI(["MyTool"], [], [
  new OpenFile(new File([])),                       // stdin
  ConsoleStdout.lineBuffered(console.log),          // stdout
  ConsoleStdout.lineBuffered(console.error),        // stderr
]);
const { instance } = await WebAssembly.instantiateStreaming(
  fetch("MyTool.wasm"), { wasi_snapshot_preview1: wasi.wasiImport });
const exitCode = wasi.start(instance);
```

CI builds the smoke executable for WebAssembly on Swift 6.4 and runs it this way in headless
Chrome on every run off a pull request, failing unless it exits 0. An executable that imports Foundation is
large (tens of MB, most of it ICU data); `-c release` and `wasm-opt` bring that down.

The other direction — running the core as wasm *inside* a native Swift process — is not
built: no wasm engine ships as a Swift package with a stable API, and nothing here needs one, since every platform this binding
supports has the core natively. The root README's
[WebAssembly section](../README.md#webassembly) tracks both directions for every binding.

## Verifying provenance

There is no separate package registry to attest here: SwiftPM resolves a git tag directly
against this repo, and downloads the two zips the tag's `Package.swift` names from that
release's assets, refusing either if its SHA-256 differs from the checksum written there. Each
zip verifies from the release tag alone: the release attestation, signed by this repository's
`release.yml` at the tag, names both, with the release inventory as its predicate.

```sh
gh release download v<version> --repo SkunkWerkx/HyperCast --pattern 'HyperCastCore*.zip'
gh attestation verify HyperCastCore.artifactbundle.zip \
  --repo SkunkWerkx/HyperCast --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

Each zip also carries build provenance from `hyper-build-static.yml` in `SkunkWerkx/.github`,
at the preparation commit the tagged commit was staged from (the preparation run's
`stage-release` job verifies that before it writes a checksum into `Package.swift`, and the tag
run again before it signs the release attestation), so that check needs
`--signer-repo` beside `--repo`:

```sh
gh attestation verify HyperCastCoreApple.xcframework.zip \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
```

Get the signer wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which
reads like a bad signature but is only an identity mismatch. The root README's
[Provenance](../README.md#provenance) section has the whole picture: the inventory, which
artifacts are signed from where, and the gates every byte passes before publication.

## Install

Add the package URL as a dependency:

```
https://github.com/SkunkWerkx/HyperCast
```

In Xcode that's File ▸ Add Package Dependencies; in a `Package.swift` it's a `.package(url:)`
entry with whatever version requirement suits you, plus the product on each target that
uses it:

```swift
dependencies: [
    .package(url: "https://github.com/SkunkWerkx/HyperCast", from: "<version>"),   // the tag on the badge above
],
targets: [
    .target(
        name: "MyTarget",
        dependencies: [.product(name: "HyperCast", package: "HyperCast")]
    ),
]
```

SwiftPM resolves the newest release that satisfies the requirement, so there is no version
to copy from here and none to go stale.

SwiftPM has no separate registry to publish to — `.package(url:from:)` resolves straight from
a git tag, which *is* the complete publish story here rather than a placeholder for one. It
requires `Package.swift` at the repository root with no monorepo-subdirectory support, which
is why [the root's own `Package.swift`](../Package.swift) exists, with its targets pointed at
the real sources under `swift/` via `path:`. The native binaries are not in git: the manifest
names two zips on the GitHub Release by URL and checksum, the static-library artifact bundle
(`hypercast.lib` for the two Windows triples, `libhypercast.a` for the rest) and, on a Mac, the
XCFramework with the iOS and Mac Catalyst slices, and SwiftPM downloads the ones the resolved
tag names.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
