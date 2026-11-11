# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![npm](https://img.shields.io/npm/v/@skunkwerkx/hypercast.svg)](https://www.npmjs.com/package/@skunkwerkx/hypercast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**A discriminated union for every cast: `{ ok: true, value }` or `{ ok: false, fault }`, a
closed reason and the exact span that offended, narrowed on `ok`. No door throws, whatever it
is handed.**

Scalar casts from text: booleans, characters, the full integer family, reals, exact decimals,
UUIDs, temporals. A Node-API addon built with [napi-rs](https://napi.rs): the Rust core every
other HyperCast binding calls, linked straight into the `.node` file, with types for
TypeScript and the same API under Node and Bun, and a WebAssembly fallback where no addon
fits.

## Install

```sh
npm install @skunkwerkx/hypercast@<version>
```

`bun add @skunkwerkx/hypercast@<version>` installs the same packages, and Node runs the result.

The package itself is JavaScript and types; the core comes from one of nine companion
packages. The native addon is a platform package that npm picks for you through
`optionalDependencies`, so nothing compiles at install, one for each of eight platforms, as
`@skunkwerkx/hypercast-<platform>`. The ninth, `@skunkwerkx/hypercast-wasm32-wasip1`, is the
WebAssembly fallback: a plain dependency of the root, so it is always installed, and loaded
only when no native addon fits.

| | x64 | arm64 |
| --- | :---: | :---: |
| Linux, glibc (`linux-*-gnu`) | ✓ | ✓ |
| Linux, musl (`linux-*-musl`, Alpine) | ✓ | ✓ |
| macOS (`darwin-*`) | ✓ | ✓ |
| Windows (`win32-*-msvc`) | ✓ | ✓ |
| Everything else: the WebAssembly fallback (`wasm32-wasip1`) | ✓ | ✓ |

It runs on Node 22.14 or newer (`^22.14.0 || >=23.6.0`, the first releases with Node-API
10), or Bun. An install run with `--omit=optional`, or on a platform outside the eight,
gets the fallback instead of the addon ([below](#when-no-native-addon-fits)).

## Usage

```js
const { castI32, castTimestamp } = require('@skunkwerkx/hypercast')

const verdict = castI32('(1,234)')
if (verdict.ok) {
  console.log('got', verdict.value) // got -1234, accounting negative
} else {
  console.log(verdict.fault.reason, 'at', verdict.fault.offset)
}

console.log(castI32('12x'))
// { ok: false, fault: { reason: 'malformed', offset: 2, length: 1 } }
console.log(castTimestamp('2026-01-02T15:04:05.123456789+05:00'))
// { ok: true, value: { seconds: 1767348245, nanos: 123456789 } }
```

ESM works the same way, named imports included (`import { castI32 } from
'@skunkwerkx/hypercast'`).

Every door returns a `Verdict<T>`:

```ts
type Verdict<T> = Success<T> | Failure
interface Success<T> { ok: true; value: T }
interface Failure { ok: false; fault: Fault }
interface Fault { reason: CastFailure; offset: number; length: number }
type CastFailure = 'empty' | 'malformed' | 'outOfRange'
```

`ok` is the discriminant, so TypeScript reaches `value` only after `verdict.ok` and `fault`
only after `!verdict.ok`, and a `switch` over `fault.reason` is exhaustive over exactly three
strings (a `default` that assigns the reason to `never` type-checks). `empty` is nothing
there, or only white space; `malformed` is present but not the target type; `outOfRange` is
well-formed but outside the target's range. `CastFailure.Empty`, `.Malformed` and
`.OutOfRange` name the three strings for a caller who would rather not spell them.

```ts
import { castI32, type Fault, type Verdict } from '@skunkwerkx/hypercast'

function describe(fault: Fault): string {
  switch (fault.reason) {
    case 'empty':
      return 'absent'
    case 'malformed':
      return `malformed at ${fault.offset}`
    case 'outOfRange':
      return `out of range at ${fault.offset}`
    default: {
      const unreachable: never = fault.reason
      return unreachable
    }
  }
}

const verdict: Verdict<number> = castI32('12x')
console.log(verdict.ok ? verdict.value : describe(verdict.fault)) // malformed at 2
```

`optional(verdict)` presents an `empty` fault as `null` and passes everything else through,
for the column where blank means absent: `optional(castI32('   '))` is `null`,
`optional(castI32('abc'))` the `malformed` failure.

### Spans

The core reports a fault's span in UTF-8 bytes. The binding hands it back in the units of
what you passed, so `text.slice(offset, offset + length)` (or `subarray` on bytes) is the
offending text with no mapping:

- a `string` gets UTF-16 code units, JavaScript's own string index (`'😀x'` is malformed at
  offset 0, length 2, the emoji's two code units);
- a `Uint8Array` (a `Buffer` too) gets bytes, verbatim from the core (the same `'😀x'` as its
  UTF-8 is offset 0, length 4).

A `Uint8Array` over a `SharedArrayBuffer` is read once, into a copy, before the door judges it:
another thread writing that memory meanwhile changes nothing the verdict holds.

Every span satisfies `offset + length <= text.length` in those units. Text is read as given,
and surrounding white space is trimmed before it is judged, so the span still indexes what
was passed (`castI32('  12x  ')` is malformed at offset 4). A string with a lone surrogate
has no UTF-8 form, so it is `malformed`, never thrown: every door, plain or checked, reads it
as it is (WTF-8), so the fault is at the surrogate's own code unit and a lone surrogate never
matches a declared `'\ufffd'`, as a currency symbol or as an `enum` entry. Nor can one be
declared: in a `NumFormat` or `DateFormat` symbol it is a refusal naming the field (`currency
must be whole code points, not surrogate halves`), and as an `enum` entry refusal `9`.

### The doors

Every door takes the text first, as `Text` (`string | Uint8Array | undefined | null`), and
returns a `Verdict<T>`:

| Door | Reads | `value` |
| --- | --- | --- |
| `castBool(text)` | the natural-language boolean lexicon | `boolean` |
| `castChar(text)` | one character verbatim, or a declared code point (`65`, `U+0041`, `0x41`, `&H41`, `&#65;`, `&#x41;`) | `string` of one code point |
| `castI8` `castI16` `castI32` `castU8` `castU16` `castU32` `(text, format?)` | an integer under a declared `NumFormat` | `number` |
| `castI64` `castU64` `(text, format?)` | an integer under a declared `NumFormat` | `bigint` |
| `castF32` `castF64` `(text, format?)` | a real under a declared `NumFormat` | `number` (an `f32` widened exactly) |
| `castDecimal(text, format?)` | an exact decimal under a declared `NumFormat` | `string`, canonical (`'1234.5'`) |
| `castUuid(text)` | every .NET `Guid` form, plus `urn:uuid:`-style prefixes | `string`, lowercase hyphenated |
| `castTimestamp(text, options?)` | an RFC 3339 instant (a space for the `T` and `±hh` offsets with `DateOptions`) | `Timestamp` |
| `castUnix(text, precision)` | an integer Unix-epoch value in a declared `UnixPrecision` | `Timestamp` |
| `castExcelSerial(text, epoch)` | an Excel date serial in a declared `ExcelEpoch` | `Timestamp` |
| `castDate(text, order?, options?)` | a strict ISO `yyyy-MM-dd` date, or a separated date in a declared `DateOrder`; `DateOptions` add `20260107`, a pivot and month names | `CalendarDate` |
| `castDatetime(text, format, options?)` | a zone-less civil date-time in a declared `DateOrder`, or a `DateFormat` that adds a culture's day periods; `DateOptions` add a pivot and month names | `CivilDateTime` |
| `castTime(text)` | an ISO 24-hour time of day | `TimeOfDay` |
| `castDuration(text)` | ISO 8601, the invariant colon form, or protobuf JSON seconds | `Duration` |
| `castDecimalFromNumber(value)` | a `number` as the shortest decimal that names it (`0.1` is one tenth) | `string`, canonical |
| `castExcelSerialFromNumber(value, epoch)` | an Excel serial `number` in a declared `ExcelEpoch` | `CivilDateTime` |
| `castExcelTime(value)` | the fraction of an Excel serial `number` | `TimeOfDay` |
| `castExcelDuration(value)` | a `number` count of days | `Duration` |

The temporal values are plain objects at the core's nanosecond resolution, so nothing is
truncated to a `Date`'s milliseconds:

| Type | Fields |
| --- | --- |
| `Timestamp` | `seconds` (Unix epoch, UTC), `nanos` (0 to 999,999,999) |
| `Duration` | `seconds`, `nanos` (both carry the duration's sign: `-PT1.5S` is `-1` and `-500000000`) |
| `CalendarDate` | `year`, `month`, `day` |
| `TimeOfDay` | `hour`, `minute`, `second`, `nanos` |
| `CivilDateTime` | `year`, `month`, `day`, `hour`, `minute`, `second`, `nanos` |

A `Timestamp` is a `Date` as `new Date(seconds * 1000 + Math.floor(nanos / 1e6))` when
milliseconds are enough. Integers that fit a `number` exactly come back as one; `i64` and
`u64` come back as `bigint`, so `castU64('18446744073709551615')` is
`18446744073709551615n`. A decimal comes back as its canonical string (trailing fraction
zeros trimmed, zero never negative), which a decimal library takes without rounding.

Nothing about the text is guessed; what a door cannot know from the text, the caller
declares, as a numeric enum (plain objects, not `const enum`s, so they work under type
stripping):

- **`UnixPrecision`**: `Seconds`, `Milliseconds`, `Microseconds`, `Nanoseconds`, the unit of
  a Unix-epoch value, never inferred from its magnitude.
- **`ExcelEpoch`**: `Y1900` (the Windows default, phantom February 29th 1900 included) or
  `Y1904` (the legacy Macintosh system), a workbook-level setting no serial carries.
- **`DateOrder`**: `YearMonthDay`, `MonthDayYear`, `DayMonthYear`. `'1/7/2026'` is January
  7th or July 1st only because the caller said which. Without one, `castDate` stays strict
  ISO.

```js
const { castUnix, castExcelSerial, castDate, castDatetime, castDuration, DateFormat,
  DateOrder, ExcelEpoch, UnixPrecision } = require('@skunkwerkx/hypercast')

castUnix('1700000000123', UnixPrecision.Milliseconds)
// { ok: true, value: { seconds: 1700000000, nanos: 123000000 } }
castExcelSerial('45000', ExcelEpoch.Y1900)
// { ok: true, value: { seconds: 1678838400, nanos: 0 } }
castDate('1/7/2026', DateOrder.DayMonthYear)
// { ok: true, value: { year: 2026, month: 7, day: 1 } }
castDatetime('1/7/2026 3:04 PM', DateOrder.MonthDayYear)
// { ok: true, value: { year: 2026, month: 1, day: 7, hour: 15, minute: 4, second: 0, nanos: 0 } }
const korean = DateFormat.declare({ order: DateOrder.YearMonthDay, am: '오전', pm: '오후' })
if (korean.ok) castDatetime('2026-01-07 오후 3:04', korean.value)
// the same value
castDuration('PT1H30M')
// { ok: true, value: { seconds: 5400, nanos: 0 } }
```

`castDatetime` reads the English `AM`/`PM` whatever is declared: ASCII, either case, after the
clock (`3:04 PM`) or ahead of it (`PM 3:04`). A culture's own designators are declared beside
them in a `DateFormat` (German `vorm.`/`nachm.`, Korean `오전`/`오후`, Spanish `a. m.`/`p. m.`)
and read exactly as spelled, after the clock with white space before or ahead of it with
white space after: a run of any, so the `3:04\u202fPM` that ICU 72 and later write for `en_US`
reads as well as `3:04 PM`. Undeclared, `nachm.` is the first offending character of a
`malformed` fault. A designator is 1 to 16 UTF-8 bytes with no ASCII digit and no white space
at either end (inside is fine, as in `a. m.`).

White space is Unicode's `White_Space`, the 25 code points (ASCII, U+0085, U+00A0, U+2000 to
U+200A, U+202F, U+3000 among them), and every door but the plain `castText` ignores it at both ends.
The numeric doors also ignore it after a leading sign, before a trailing sign or a percent
symbol, on either side of a currency symbol and inside accounting parentheses (`'- 12'`,
`'( 12 )'`, `'12,5 %'`, the last under a declared comma decimal separator); between digits it is `malformed`, unless a space-like group
separator is declared (below). Text that is only white space is `empty`.

### Saying what failed

A fault says why (`reason`) and where (`offset`, `length`), and has exactly those three fields. The four detailed doors answer the same verdicts with a
`DetailedFault`, which adds what failed, by name and by the core's number:

| Door | Reads | Like |
| --- | --- | --- |
| `castDetailed(door, text, declared?, options?)` | any door the core names, `Door.Bool` to `Door.Char` | every text door above, and `castText` |
| `castDetailedFromNumber(door, value, declared?)` | `Door.Decimal`, `Door.ExcelSerial` (`declared` an `ExcelEpoch`), `Door.Time`, `Door.Duration` | the four `FromNumber` and `Excel` doors |
| `castCheckedDetailed(text, constraints, format?)` | a numeric or `Door.Text` `Constraints` | `castChecked` and `castText` with constraints |
| `castCheckedDetailedFromNumber(value, constraints)` | a numeric `Constraints` | `castCheckedFromNumber` |

```js
const { castDetailed, Door } = require('@skunkwerkx/hypercast')

castDetailed(Door.I32, '12x')
// { ok: false, fault: { reason: 'malformed', offset: 2, length: 1,
//                       detail: 'unexpectedCharacter', detailCode: 2 } }
```

`declared` is what the door declares: a `NumFormat` at the integer, real and decimal doors, a
`UnixPrecision` at `Door.Unix`, a `DateOrder` at `Door.DateOrdered`, a `DateOrder` or
`DateFormat` at `Door.DateTime`, an `ExcelEpoch` at `Door.ExcelSerial`, and nothing anywhere
else. `detail` is one of the names in `Detail` (`blank`, `unexpectedCharacter`, `groupSize`,
`tooLarge` and so on, 32 of them); `detailCode` is the core's number (1 to 32), which never
changes. Published codes keep their meaning, and new ones may arrive in a minor release, so a
`switch` over `detail` needs a `default` arm: a code this binding does not name yet is
`'unknown'`, with its number in `detailCode` and the reason and span as usable as ever. `'none'`
(code 0) is a fault with nothing to say: a contract violation, which is `malformed` at offset 0,
length 0 as at every door. Absent text is `empty` with the detail `blank`. A checked door's
violation is a rule the value broke, not a failure to read it, so it has no detail.

## Declared formats

Every numeric door takes an optional `NumFormat`; left out (or `null`), it is
`NumFormat.invariant()`: `.` and `,` with every lenience in `NumStyles.All` on and nothing else
declared, and Western grouping. `NumFormat.detect()` adds `NumStyles.SeparatorDetect`, so
the `.`/`,` roles are resolved per input from its structure and an undecidable one is a
`malformed` fault, never a guess (a grouping policy never settles one). Anything else is declared
from a spec:

```js
const { castI32, castDecimal, castF64, NumFormat, NumStyles } = require('@skunkwerkx/hypercast')

const dollars = NumFormat.declare({ decimalSep: '.', groupSep: ',', flags: NumStyles.All, currency: '$' })
if (dollars.ok) {
  castI32('($1,234)', dollars.value) // { ok: true, value: -1234 }
  castDecimal('$1,234.50', dollars.value) // { ok: true, value: '1234.5' }
}

const estonian = NumFormat.declare({
  decimalSep: ',',
  groupSep: ' ',
  flags: NumStyles.All,
  currency: '€',
  minusSign: '−',
})
if (estonian.ok) castF64('−1 234,5', estonian.value) // { ok: true, value: -1234.5 }

NumFormat.declare({ decimalSep: ',', groupSep: ',', flags: NumStyles.All })
// { ok: false, message: 'Decimal and group separators must differ; both are ","' }
```

### Grouping

A format declares how it reads groups of digits, apart from its styles, as a
`Grouping`, in `grouping`:

- **`Grouping.Western`** (0), the default (a declaration with no `grouping`, `invariant()` and
  `detect()`): groups of exactly three digits counted from the decimal separator, the leftmost
  of one to three, not starting with a zero. `1,234,567` reads; `1,23,456`, `12,34`, `1234,567` and `0,125`
  are `malformed` (`groupSize`).
- **`Grouping.Indian`** (512): three beside the decimal separator and twos beyond it, the leftmost of
  one or two, not starting with a zero (`1,23,456`, `12,34,567`). It refuses `1,234,567`.

`format.withGrouping(Grouping.Indian)` chooses either on a format already made, and answers with
a `Declaration`, as `declare` does: anything else, 256 and 768 included, is refused. Bit 8 is
retired permanently and never reused. Ungrouped numerals stay valid under both, and once a numeral groups every group must
conform. A policy does not turn grouping on: with `NumStyles.Grouping` off a group separator is
refused under both. Digits are counted as digits, native ones included, whatever
separator sits between the groups. A style mask holds no policy: `flags` with bit 8 or 9 set is
refused, naming `grouping`, and `NumFormat.flags` reads back the styles alone, with
`NumFormat.grouping` beside it. A door given no format reads `NumFormat.invariant()`. A shape
neither policy reads is `malformed` under a declared format or `invariant()`, never misread, while `detect()` infers by the delimiter rule, so `1,2345` is 1.2345 there: [docs/grouping.md](https://github.com/SkunkWerkx/HyperCast/blob/master/docs/grouping.md#parked).

A spec (`NumFormatSpec`) has `decimalSep`, `groupSep` and `flags`, and optionally `grouping`, `currency`,
`currencyDecimalSep`, `currencyGroupSep`, `minusSign`, `plusSign`, `percent` and `digitZero`.
The flags are a bitwise OR of `NumStyles`: `Grouping`, `Parentheses`, `Exponent`,
`RadixPrefixes`, `Percent`, `Currency` (`All` is those six), and, opted into separately,
`TrailingSign` and `SeparatorDetect`. The rules are the core's and every binding's: the ASCII
spelling is always read, and a declared symbol is a further spelling read beside it; declare
an ASCII space, U+00A0 or U+202F as the group separator and the input may use any of the three
(and mix them), while any other group separator stays exact; the repository root README has
them in full.

A declaration is a DU too, `Declaration<T> = Success<T> | Refusal`, where a `Refusal` is
`{ ok: false, message }` with the same message the Python binding raises. What the core refuses
to carry is refused here, never thrown: separators, signs or a percent symbol that are not 1
to 8 UTF-8 bytes or contain an ASCII digit, equal separators, a sign or percent symbol with
white space in it, a decimal separator (number or currency) with white space in it, a currency
symbol over 16 bytes, carrying a digit or starting or ending in white space (`'F CFA'` is
fine, and there an ASCII space, U+00A0 and U+202F read as each other), a digit zero that starts no Unicode digit block, flags
that are not a whole number from 0 to 2^32 - 1, a field of the wrong type, or a getter on the
spec that throws. A
`NumFormat` reads back through getters of the same names, each `''` when it declares nothing,
and `packed` is the 92-byte little-endian `RawNumFormat` the core reads in place, for a library
that hands it to a C ABI of its own. `DateFormat.declare({ order, am?, pm? })` follows the same
pattern. Neither class has a public constructor: `new NumFormat()` throws napi's "Class
contains no `constructor`" error, the one way to call this package wrongly that the types do
not stop; use `declare`, `invariant()` or `detect()`. JavaScript has no value equality to hook,
so `===` is identity; `a.equals(b)` is true when `b` declares the same thing, however each was
made, and `NumFormat`, `DateFormat`, `DateOptions` and `MonthNames` all have it.

## Date options

The date doors keep their strict shapes unless they are given options, so a caller who relies on
strict RFC 3339 can rely on a space separator or `+00` being refused. Options are opt-in, built
by `DateOptions.declare({ flags?, pivot?, months? })` (or `DateOptions.commonForms()`), and
passed as the last argument of `castDate`, `castTimestamp`, `castDatetime` and `castDetailed`:

| Option | Door | Accepts |
| --- | --- | --- |
| `DateStyles.CompactDate` | `castDate`, no order | `20260107` |
| `DateStyles.SpaceSeparator` | `castTimestamp` | one space in place of the `T` |
| `DateStyles.HourOffset` | `castTimestamp` | `±hh` offsets (`+00`, `-05`) |
| `pivot` | with an order, and `castDatetime` | two-digit years |
| `months` (a `MonthNames`) | with an order, and `castDatetime` | declared month names |

`DateStyles.CommonForms` (or `DateOptions.commonForms()`) turns the three styles on together; it
works at every date door, a style a door does not read is ignored there, and it never supplies a
pivot or month names. A `pivot` is the last year of a 100-year window: `yy` reads as the one year
from `pivot - 99` to `pivot` that ends in `yy`, so with 2029, `26` is 2026 and `30` is 1930. It
is 0 (none: a two-digit year is `fieldWidth`) or from 100 to 9999, and is refused rather than
narrowed. Month names are Excel style only (`07-Jan-26`): the name stands where the declared
order puts the month, between the date's own separators, and never reorders the fields.

```js
const { DateOptions, MonthNames, DateOrder, castDate } = require('@skunkwerkx/hypercast')

const months = MonthNames.declare([[1, 'janv.'], [2, 'févr.'], /* … every month … */ [12, 'déc.']])
if (months.ok) {
  const options = DateOptions.declare({ pivot: 2029, months: months.value })
  if (options.ok) castDate('7.janv..26', DateOrder.DayMonthYear, options.value)
  // { ok: true, value: { year: 2026, month: 1, day: 7 } }
}
```

`MonthNames.declare(entries)` takes `[month, name]` entries, aliases as separate entries
(`[9, 'Sep'], [9, 'Sept']`); `MonthNames.fromPacked(bytes)` takes the layout the C ABI does, each
entry a month byte, a little-endian `u32` length and the UTF-8, and `packed` reads it back. Names
match under simple case folding and the longest name followed by the date's separator wins;
punctuation is part of a name, so with `janv.` declared and `.` as the separator `7.janv..26`
reads and `7.janv.26` is `malformed` unless `janv` is declared too. Every month needs a name; a
name is 1 to 64 UTF-8 bytes with no ASCII digit, `/`, `-` or white space, and two that fold alike
are refused. A `MonthNames` passed alone (as `options`) opens month names and nothing else. A
refusal is `{ ok: false, message }` naming the rule; passing something else as `options` to a
date door is a contract violation, and a door that does not read options ignores them.

## Validation constraints

Untrusted input is judged in two steps. First it is cast to the door's type: a fault (`empty`,
`malformed`, `outOfRange`) means it is not that type. Then the value is validated by the
OpenAPI 3.2 Schema Object's keywords (JSON Schema 2020-12's): `minimum`, `exclusiveMinimum`,
`maximum`, `exclusiveMaximum`, `multipleOf` with a rounding, `minLength`, `maxLength` and
`enum`, with HyperCast's own `trim` and `caseSensitive` for text. A broken rule is a violation,
which carries the value as cast. A `Constraints` is declared for one door and judged once, by
the core, when declared; every checked cast after that judges only data.

```js
const { castChecked, castText, Constraints, Door, NumFormat, NumStyles, Rounding } =
  require('@skunkwerkx/hypercast')

const latitude = Constraints.declare({
  door: Door.F64,
  minimum: '-90',
  maximum: '90',
  multipleOf: '0.000001',
  rounding: Rounding.Midpoint,
})
if (latitude.ok) {
  castChecked('45.12345678', latitude.value)
  // { ok: true, value: 45.123457, adjusted: true, enumIndex: null }
  castChecked('91', latitude.value)
  // { ok: false, violation: { keyword: 'maximum', value: 91, offset: 0, length: 2 } }
}

const euro = NumFormat.declare({ decimalSep: ',', groupSep: '.', flags: NumStyles.All })
const cash = Constraints.declare({ door: Door.Decimal, minimum: '0', multipleOf: '0.05' })
if (euro.ok && cash.ok) {
  castChecked('12,35', cash.value, euro.value)
  // { ok: true, value: '12.35', adjusted: false, enumIndex: null }
  castChecked('12,37', cash.value, euro.value)
  // { ok: false, violation: { keyword: 'multipleOf', value: '12.37', offset: 0, length: 5 } }
}

const sex = Constraints.declare({ door: Door.Text, trim: true, enum: ['Female', 'Male', 'Other'] })
if (sex.ok) {
  castText(' MALE ', sex.value)
  // { ok: true, value: 'Male', adjusted: false, enumIndex: 1 }
  castText(' Mail ', sex.value)
  // { ok: false, violation: { keyword: 'enum', value: 'Mail', offset: 1, length: 4 } }
}

const units = Constraints.declare({ door: Door.Text, caseSensitive: true, enum: ['mm', 'Mm'] })
if (units.ok) {
  castText('Mm', units.value)
  // { ok: true, value: 'Mm', adjusted: false, enumIndex: 1 }
}

Constraints.declare({ door: Door.I32, trim: true })
// { ok: false, message: 'Constraints refused (code 1): a constraint the door does not take.' }
```

- **The declaration.** A `ConstraintsSpec` has the `door` (a `Door`: `I8` to `U64`, `F32`,
  `F64`, `Decimal`, `Text`) and its keywords: the four bounds, `multipleOf`, `rounding` (a
  `Rounding`: `Strict`, the default, `Midpoint`, half to even, `Floor`, `Ceiling`) and `enum`
  at the numeric doors; `minLength`, `maxLength` (in code points), `enum`, `trim` (Unicode
  `White_Space`) and `caseSensitive` at `Door.Text`. A text `enum` matches under simple case
  folding unless `caseSensitive: true`, which compares byte for byte, for a vocabulary whose
  case carries meaning (a `bigint` among its entries is valid and never matches); a numeric `enum` is compared exactly to the value (`'2.50'` matches
  `'2.5'`). A bound, step or numeric entry is a decimal string in invariant notation
  (`'0.000001'`, `'-90'`, `'1e-6'`) or a `bigint`, never a `number`: a `number` is a binary
  float, which holds most decimals only approximately, so `maximum: 0.1` is refused as a
  binding contract (and is a type error). A length is a whole `number`, never a `boolean`.
- **Every OpenAPI declaration is accepted.** Each bound is its own keyword, so `minimum` and
  `exclusiveMinimum` both apply when both are declared. A keyword for the other type
  (`minLength` at `Door.I32`, `minimum` at `Door.Text`) is ignored, as JSON Schema ignores
  it. A declaration no value satisfies (crossed bounds, `minLength` above `maxLength`,
  `enum: []`) is accepted, and every value then violates it; leaving `enum` out declares none.
  With duplicate entries the first match wins, and an entry no value can match (an empty
  text, a fraction at an integer door) never matches.
- **Refusals.** `Constraints.declare` answers a `Declaration<Constraints>`, never throws. A
  declaration the core refuses reads `Constraints refused (code N): <the rule>.`, `N` the code
  `castConstraintsCheck(spec)` answers: `1` a constraint the door does not take (`trim` or
  `caseSensitive` at a numeric door, a `rounding` with `multipleOf` at `Door.Text`), `5` a
  `multipleOf` not positive, `6` a rounding without `multipleOf`, `8` a bound, step or entry
  no door holds (past 28 places or 96 bits, or a rounding with a fractional step at an integer
  door), `9` a text entry with no UTF-8 form (a lone surrogate) and `11` a rounding code the
  core does not know. A spec the binding cannot read (a `number` bound, a `door` that is no
  `Door`, an `enum` that is no array, a `trim` that is no `boolean`) is refused with its own
  wording, and `castConstraintsCheck` answers `-1` for it. `castDeclaredDecimal(text)` is the
  conversion every bound crosses, as a `Declaration<string>` of its canonical text (`'1.50'`
  is `'1.5'`).
- **The checked doors.** `castChecked(text, constraints, format?)` reads numeric text through
  the door the declaration is bound to; `castCheckedFromNumber(value, constraints)` judges a
  `number` already decoded on the shortest decimal that names it (`0.1 + 0.2` breaks
  `multipleOf: '0.1'` unrounded, and rounds to `0.3` under `Midpoint`), every span empty;
  `castText(text, constraints)` judges text. Each answers a `CheckedVerdict<T>`: `{ ok: true,
  value, adjusted, enumIndex }`, or `{ ok: false, fault }` (the door's own `Failure`), or `{
  ok: false, violation: { keyword, value, offset, length } }`. `value` follows the door
  (`number`, `bigint` at `I64`/`U64`, canonical text at `Decimal`, a `string` at `Text`) in
  both. A success's `value` is rounded to `multipleOf` when the declaration rounds, `adjusted`
  true only when rounding changed it, and `enumIndex` is the index of the entry a text matched
  and `null` otherwise: a numeric `enum` is answered by the value. A text match delivers your
  own declared entry, so `' MALE '` is `'Male'`. A violation's `value` is the value exactly as
  the door cast it, never rounded (rounding is part of the judgement), and at `Door.Text` the
  judged text, trimmed under `trim`. A rounding past what the type holds is a `multipleOf`
  violation: `'127'` at `Door.I8` with `multipleOf: '5'` and `Ceiling` carries `127`. Spans
  are in your units, as a fault's are. Numbers are judged exactly, on the decimal the door
  read. A float door also judges the float it delivers against each bound read as the door
  reads it: `'1e-400'` at `Door.F64` reads as `0`, so it breaks `exclusiveMinimum: '0'`. Zero
  has one form: the core never delivers `-0`. Handing a declaration to another door's cast (a
  `Door.Text` one to `castChecked`, a numeric one to `castText`) is a contract violation,
  `malformed` with an empty span, as any wrong argument is.
- **Plain text.** `castText(text)` with no constraints is a plain door: the text itself once it
  is valid text, a `Verdict<string>`, `empty` for empty input, nothing trimmed; a `Uint8Array`
  past 4,294,967,295 bytes is `outOfRange` (detail `tooLong`), since a span cannot carry it.
  Plain or constrained is chosen by the number of arguments, not by the second one's value: a
  failed declaration's missing `.value` (`castText(text, undefined)`, or `null`) is the
  contract violation, never "no rules". The addon itself counts, so every entry, the
  WebAssembly fallback's browser entry included, behaves the same.
- **Keywords are open.** `violation.keyword` is the OpenAPI spelling (`Keyword.MultipleOf` is
  `'multipleOf'`), and `format` and `pattern` may join in a minor release, so a `switch` over
  keywords needs a `default` arm; a code this binding does not name yet arrives as
  `'keyword N'`.
- `optional(verdict)` reads a checked verdict too: an `empty` fault is `null`, a violation is
  not absent and comes back as it is. A `Constraints` reads back through getters of its keyword
  names, and `packed` and `packedEnum` are the 104-byte `RawConstraints` (each bound in its own
  slot: `minimum` at 24, `maximum` at 40, `multipleOf` at 56, `exclusiveMinimum` at 72,
  `exclusiveMaximum` at 88) and the `enum` list in the native byte order the core reads in
  place, the list read only when the flags carry `HAS_ENUM` (256), which any declared `enum`
  sets, and a text `enum` matched byte for byte when they carry `CASE_SENSITIVE` (512).

## Versions

The addon (and the WebAssembly fallback) is the core compiled in, not a library found at load
time, so a core older than 1.1 cannot be loaded under it: the detailed exports and the
grouping policy it calls are linked, and a build against a 1.0 core fails to compile. The plain
numeric doors call the core's Rust doors, which honour a format's policy; the checked doors and
the four detailed doors call the core's detailed exports, which do too. `index.js` also pairs
the addon with the package when it loads: unless `nativeVersion()` equals the package's version
and the addon has `castDetailed`, it is treated as not loaded (`isAvailable()` is `false`, every
door throws `ERR_HYPERCAST_UNAVAILABLE` with the mismatch as its `cause`), which catches a stale
platform package or an addon `NAPI_RS_NATIVE_LIBRARY_PATH` points at. `nativeVersion()` is the linked core's version, which the package version matches.

## What never throws, and the one thing that does

No door and no declaration throws for any argument, of any type, in any position. A call the
types would have refused still returns a verdict:

- text that is not a `string` or a `Uint8Array` (a number, an `ArrayBuffer`, a `Uint16Array`,
  a `DataView`, an object), a format that is not a `NumFormat` (or a `DateOrder` or
  `DateFormat` for `castDatetime`), an enum value that names no member, or a non-`number`
  given to a `FromNumber`/`Excel` door is `{ ok: false, fault: { reason: 'malformed', offset:
  0, length: 0 } }`: a contract violation, with an empty span because there is no text to
  point into. The declarations are read before the text, so absent text under a broken
  declaration is `malformed` too;
- `undefined` or `null` text is `empty`, offset 0, length 0, as blank text is.

`tests/never_throws.test.ts` calls every door, the checked ones included, with some fifty
hostile values (symbols, `bigint`s, detached buffers, Proxies whose traps throw, the classes'
own prototypes) in every position and every pair, and every declaration with each as its spec
and as each `Constraints` keyword.

The one error the package throws is `ERR_HYPERCAST_UNAVAILABLE`, from any door when no addon
and no fallback loaded: an installation problem, not an input ([below](#when-no-native-addon-fits)).
`isAvailable()` says up front, and `isHyperCastError(error)` recognizes it. On the native
addon every door also catches a Rust panic and throws it as napi's `GenericFailure` rather
than taking the process down; the core is proven to have none, so that is a backstop, and on
the fallback a panic aborts instead. An engine out of memory throws as it would anywhere.

## When no native addon fits

`require('@skunkwerkx/hypercast')` tries the platform package for the host first. When none
fits (the install ran with `--omit=optional`, or the platform is not one of the eight) it loads
the WebAssembly fallback instead: the same core compiled to `wasm32-wasip1`, run in-process
through Node's `node:wasi` by
[`@napi-rs/wasm-runtime`](https://www.npmjs.com/package/@napi-rs/wasm-runtime), from
`@skunkwerkx/hypercast-wasm32-wasip1`, which the root depends on outright. The API is the same,
the suite is the same, and `isAvailable()` is `true`. What differs:

- Node prints, once, at `require`:

  ```text
  (node:NNN) ExperimentalWarning: WASI is an experimental feature and might change at any time
  ```

  That is Node's own warning about `node:wasi`, not this package's;
  `node --disable-warning=ExperimentalWarning` silences it.
- The install carries the runtime's three dependencies (`@napi-rs/wasm-runtime`,
  `@emnapi/core`, `@emnapi/runtime`) and what they pull in, beside the 376,730-byte module.
- A Proxy whose prototype lookup throws (a `getPrototypeOf` trap that throws, or a revoked
  Proxy) makes a door or declaration throw under the fallback, a recorded loss: the runtime's
  handle lookup tests every argument `instanceof WeakRef`, which runs the trap, and its
  exception unwinds through the module before any Rust runs. The module stays sound and the
  next call answers. The native addon never looks at an argument's prototype, so there these
  are verdicts like any other value.
- Panics abort. `wasm32-wasip1` has no unwinder, so the fallback is built with
  `panic = "abort"`: a panic surfaces as `RuntimeError: unreachable` thrown from the call.
- Bun 1.4.4 cannot run it: its `node:wasi` has no reactor `initialize`, so Bun with no native
  addon has `isAvailable()` `false`. Bun with the addon passes the whole suite.

`NAPI_RS_FORCE_WASI=error` in the environment loads the fallback even where an addon is
installed and fails rather than falling back further; that is how CI runs the whole suite
against it, and how to try it locally. The consumer smoke runs from the packed tarballs on Alpine, at
the exact floor, Node 22.14.0, and on the floating Node 22 and 24 images; the Node floor is
enforced (`engines`), not advisory.

**Big-endian hosts** (s390x, tested in `node:24-alpine` for `linux/s390x` under QEMU) are a
loss, printed here: the fallback loads and every check of the consumer smoke passes, but the
process never exits on its own. The WebAssembly runtime (`@emnapi/runtime` 2.0.0-beta.3
through `@napi-rs/wasm-runtime` 1.2.7) writes the module's threadsafe-function reference count
little-endian and reads it back host-endian, so the `MessagePort` that keeps the event loop
alive is never released. Nothing more is claimed for big-endian hosts than the smoke's checks.

**Bundlers.** esbuild and webpack cannot fold the loader's platform `require`s into one file:
webpack fails the build on them, and an esbuild bundle cannot load the module from a directory
with no `node_modules`. The form that works, for both, is the package external (esbuild
`--external:@skunkwerkx/hypercast`, webpack
`externals: { '@skunkwerkx/hypercast': 'commonjs @skunkwerkx/hypercast' }`) with it installed
beside the bundle.

**Browsers.** The fallback package's `browser` entry exports the same doors, behaving
identically, over a fetched module (`import('@skunkwerkx/hypercast-wasm32-wasip1')` under a bundler that honours the
`browser` condition). `browser-smoke/` replays the whole corpus through it in headless Chrome,
from a Vite build served without cross-origin isolation. Firefox and Safari are not tested.

When the fallback is missing too (a lockfile that pinned neither package, say, or a
`node_modules` pruned by hand), `require` still succeeds, `isAvailable()` is `false`, the enums
are still there as frozen literals, and every door throws `ERR_HYPERCAST_UNAVAILABLE` with
napi's loader error as its `cause`, whose own `cause` chain names every attempt, native and
WebAssembly:

```js
const hypercast = require('@skunkwerkx/hypercast')

console.log(hypercast.isAvailable())
try {
  hypercast.castI32('1')
} catch (error) {
  console.log(error.code, error.cause.message)
}
```

With both packages removed, that prints:

```text
false
ERR_HYPERCAST_UNAVAILABLE Cannot find native binding. npm has a bug related to optional dependencies (https://github.com/npm/cli/issues/4828). Please try `npm i` again after removing both package-lock.json and node_modules directory.
```

## Bun

The same package runs under Bun on the native addon: `bun test tests/` runs this package's
whole suite. The WebAssembly fallback does not load on Bun 1.4.4 ([above](#when-no-native-addon-fits)).

## Benchmarks

No figures are published for this binding yet. `bun run bench` (and `bun run bench:bun`, and
either under `NAPI_RS_FORCE_WASI=error`) runs `bench/bench.ts`, with
[mitata](https://github.com/evanwashere/mitata), against `parseInt`, `parseFloat` and
`Date.parse`; a figure goes here with its date and machine once it is taken.

## Local loop

From `typescript/`, with a Rust toolchain installed:

```sh
bun install --frozen-lockfile
bun run build       # the release addon for this machine, plus binding.js and binding.d.ts
bun run test        # node --test: the corpus, never-throws, the verdict's types, the declarations, the constraints, grouping, dates, the detailed doors
bun run test:bun    # bun test
bun run typecheck
bun run lint
bun run bench
cargo test --release  # the Rust unit tests of the addon (detail decoding, the validation backstop)
```

The WebAssembly fallback needs the `wasm32-wasip1` target (`rustup target add wasm32-wasip1`):

```sh
bun run build:wasi                  # hypercast.wasm32-wasip1.wasm and the hypercast.wasip1* loaders
NAPI_RS_FORCE_WASI=error bun run test   # the same suite on the fallback, failing rather than falling back
```

`tests/corpus.test.ts` replays every `corpus/*.json` file, each vector as its UTF-8 bytes (byte
spans verbatim) and as a string (spans in UTF-16 code units); `HYPERCAST_CORPUS` points it at
another directory. Pass `--js binding.js --dts binding.d.ts` and `--platform` to any other
`napi build`, as both build scripts do: without the first two napi writes its generated loader
to `index.js` and `index.d.ts`, over this package's own, and without `--platform` the WASI build
deletes `binding.js` as a stale managed file.

What a consumer gets from the published tarballs is proven by installing them:
[`consumer-tests/`](consumer-tests/README.md) packs the packages, serves them from a local
registry and runs the install scenarios against them.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
