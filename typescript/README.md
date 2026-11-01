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

It runs on Node 22.13 or newer (`^22.13.0 || >=23.5.0`, the floor of the WebAssembly
runtime), or Bun. An install run with `--omit=optional`, or on a platform outside the eight,
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
there, or only whitespace; `malformed` is present but not the target type; `outOfRange` is
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

Every span satisfies `offset + length <= text.length` in those units. Text is read as given,
and surrounding whitespace is trimmed before it is judged, so the span still indexes what
was passed (`castI32('  12x  ')` is malformed at offset 4). A string with a lone surrogate
has no UTF-8 form; it is read with the surrogate replaced by U+FFFD, so it is `malformed`,
never thrown.

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
| `castTimestamp(text)` | an RFC 3339 instant | `Timestamp` |
| `castUnix(text, precision)` | an integer Unix-epoch value in a declared `UnixPrecision` | `Timestamp` |
| `castExcelSerial(text, epoch)` | an Excel date serial in a declared `ExcelEpoch` | `Timestamp` |
| `castDate(text, order?)` | a strict ISO `yyyy-MM-dd` date, or a separated date in a declared `DateOrder` | `CalendarDate` |
| `castDatetime(text, format)` | a zone-less civil date-time in a declared `DateOrder`, or a `DateFormat` that adds a culture's day periods | `CivilDateTime` |
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
and read exactly as spelled, after the clock with an optional space before or ahead of it with
an optional space after. Undeclared, `nachm.` is the first offending character of a
`malformed` fault. A designator is 1 to 16 UTF-8 bytes with no ASCII digit and no whitespace at
either end.

## Declared formats

Every numeric door takes an optional `NumFormat`; left out (or `null`), it is
`NumFormat.invariant()`: `.` and `,` with every lenience in `NumStyles.All` on and nothing else
declared. `NumFormat.detect()` adds `NumStyles.SeparatorDetect`, so the `.`/`,` roles are
resolved per input from its structure and an undecidable one is a `malformed` fault, never a
guess. Anything else is declared from a spec:

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

A spec (`NumFormatSpec`) has `decimalSep`, `groupSep` and `flags`, and optionally `currency`,
`currencyDecimalSep`, `currencyGroupSep`, `minusSign`, `plusSign`, `percent` and `digitZero`.
The flags are a bitwise OR of `NumStyles`: `Grouping`, `Parentheses`, `Exponent`,
`RadixPrefixes`, `Percent`, `Currency` (`All` is those six), and, opted into separately,
`TrailingSign` and `SeparatorDetect`. The rules are the core's and every binding's: the ASCII
spelling is always read, and a declared symbol is a further spelling read beside it; the
repository root README has them in full.

A declaration is a DU too, `Declaration<T> = Success<T> | Refusal`, where a `Refusal` is
`{ ok: false, message }` with the same message the Python binding raises. What the core refuses
to carry is refused here, never thrown: separators, signs or a percent symbol that are not 1
to 8 UTF-8 bytes or contain an ASCII digit, equal separators, a currency symbol over 16 bytes
or carrying a digit or whitespace, a digit zero that starts no Unicode digit block, flags
that are not a whole number from 0 to 2^32 - 1, a field of the wrong type, or a getter on the
spec that throws. A
`NumFormat` reads back through getters of the same names, each `''` when it declares nothing,
and `packed` is the 92-byte little-endian `RawNumFormat` the core reads in place, for a library
that hands it to a C ABI of its own. `DateFormat.declare({ order, am?, pm? })` follows the same
pattern. Neither class has a public constructor: `new NumFormat()` throws napi's "Class
contains no `constructor`" error, the one way to call this package wrongly that the types do
not stop; use `declare`, `invariant()` or `detect()`.

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

`tests/never_throws.test.ts` calls every door with some forty hostile values (symbols,
`bigint`s, detached buffers, Proxies whose traps throw, the classes' own prototypes) in every
position and every pair, and every declaration with each as its spec.

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
the exact floor, Node 22.13.0, and on the floating Node 22 and 24 images; the Node floor is
enforced (`engines`), not advisory.

**Big-endian hosts** (s390x, tested in `node:24-alpine` for `linux/s390x` under QEMU) are a
loss, printed here: the fallback loads and every check of the consumer smoke passes, but the
process never exits on its own. The WebAssembly runtime (`@emnapi/runtime` 2.0.0-beta.2
through `@napi-rs/wasm-runtime` 1.2.6) writes the module's threadsafe-function reference count
little-endian and reads it back host-endian, so the `MessagePort` that keeps the event loop
alive is never released. Nothing more is claimed for big-endian hosts than the smoke's checks.

**Bundlers.** esbuild and webpack cannot fold the loader's platform `require`s into one file:
webpack fails the build on them, and an esbuild bundle cannot load the module from a directory
with no `node_modules`. The form that works, for both, is the package external (esbuild
`--external:@skunkwerkx/hypercast`, webpack
`externals: { '@skunkwerkx/hypercast': 'commonjs @skunkwerkx/hypercast' }`) with it installed
beside the bundle.

**Browsers.** The fallback package's `browser` entry exports the same doors over a fetched
module (`import('@skunkwerkx/hypercast-wasm32-wasip1')` under a bundler that honours the
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

No figures are published for this binding yet. `npm run bench` (and `npm run bench:bun`, and
either under `NAPI_RS_FORCE_WASI=error`) runs `bench/bench.ts`, with
[mitata](https://github.com/evanwashere/mitata), against `parseInt`, `parseFloat` and
`Date.parse`; a figure goes here with its date and machine once it is taken.

## Local loop

From `typescript/`, with a Rust toolchain installed:

```sh
npm ci
npm run build       # the release addon for this machine, plus binding.js and binding.d.ts
npm test            # node --test: the corpus, never-throws, the verdict's types, the declarations
npm run test:bun    # bun test
npm run typecheck
npm run lint
npm run bench
```

The WebAssembly fallback needs the `wasm32-wasip1` target (`rustup target add wasm32-wasip1`):

```sh
npm run build:wasi                  # hypercast.wasm32-wasip1.wasm and the hypercast.wasip1* loaders
NAPI_RS_FORCE_WASI=error npm test   # the same suite on the fallback, failing rather than falling back
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
