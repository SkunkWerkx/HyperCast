# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/hypercast.svg)](https://pypi.org/project/hypercast/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**`match`/`case` over a two-case verdict — the value, or a closed reason plus the exact
byte span that offended — with the Rust core linked straight into CPython as a native
extension. No dlopen, no ctypes marshalling, no runtime bridge.**

Allocation-lean scalar casts — booleans, characters, the full integer family, reals, exact
decimals, UUIDs, temporals.
The PyO3 extension (`hypercast._native`) is the backend every wheel ships — a door is an
ordinary `METH_FASTCALL` extension call into a direct Rust call, and the wheel maturin
builds is the whole package, with no ctypes fallback. Python 3.11 is the floor,
the oldest version still in upstream support (`match`/`case` is the consumption idiom).

## Install

```sh
pip install hypercast
```

Real platform-specific wheels, so on every platform in the table below it lands at native
speed with nothing to compile — no compiler needed and no dependencies at all; the PyO3 extension *is* the package. Each
platform gets two wheels, both stable-ABI, which between them cover every CPython from the
3.11 floor up, with the GIL or without it:

| Wheel | Interpreters |
| --- | --- |
| `cp311-abi3` | CPython 3.11 and newer, with the GIL |
| `cp315-abi3.abi3t` | CPython 3.15 and newer, free-threaded or not ([PEP 803](https://peps.python.org/pep-0803/)) |

on each of eight platforms, plus a seventeenth wheel for Pyodide in the browser
([below](#in-the-browser-pyodide)):

| | x64 | arm64 |
| --- | :---: | :---: |
| Linux, glibc 2.28 or newer (`manylinux_2_28`) | ✓ | ✓ |
| Linux, musl 1.2 or newer (`musllinux_1_2` — Alpine) | ✓ | ✓ |
| macOS | ✓ | ✓ |
| Windows | ✓ | ✓ |

Platforms with a wheel (the table above) install with no compiler, nothing to compile and
no dependencies. Elsewhere pip falls back to building the same extension from the source
distribution, `hypercast-<version>.tar.gz`, which needs Rust 1.89 or newer and a C linker; CI
proves that build on x86-64 glibc 2.17 and on musl, with CPython 3.11 at exactly Rust 1.89.
pip's build isolation fetches `maturin`. s390x (big-endian): CI runs the core's test suite,
byte-exact corpus included, on s390x under QEMU; the extension itself has not run there.

Free-threaded CPython older than 3.15 has no stable ABI,
so its wheel would serve one interpreter version, too narrow an ABI to ship; free-threading
starts at 3.15, where `abi3t` covers it.

On Alpine the musl wheel needs nothing beyond the base image — it carries its own copy of
`libgcc_s` — and the whole suite passes on a bare `python:3.14-alpine`.

**Free-threaded CPython runs it without the GIL.** The extension declares it needs no GIL, so a
free-threaded interpreter keeps the GIL off after `import hypercast` (no `RuntimeWarning`), and
every door is safe to call from any number of threads at once: the verdict types and
`NumFormat` are immutable, the objects a door builds its values with are bound once at import
and only read after, and a door reads its `str` or `bytes` argument in place, both immutable.
The bindings are written once, for free-threading; a GIL build runs the same code. No door
releases the GIL on a GIL build: each is tens to hundreds of nanoseconds of pure Rust, less
than the handoff would cost. CI runs the whole suite on free-threaded 3.15 on glibc Linux,
Apple silicon macOS and Windows, x64 and arm64, and installs each wheel on its own
interpreters, free-threaded ones included, on all eight platforms before it is published.

The package is typed: it ships `py.typed` and a stub for the extension module, so mypy and
pyright see each door's own `Success[...] | Fault` rather than `Any` — which is what makes
the exhaustiveness check below a real one.

## Usage

```python
import hypercast

match hypercast.cast_i32("(1,234)", hypercast.NumFormat.INVARIANT):
    case hypercast.Success(value):
        print("got", value)  # -1234, accounting negative
    case hypercast.Fault(reason, offset):
        print(reason.name, "at char", offset)

match hypercast.cast_timestamp("2026-01-02T15:04:05.123456789+05:00"):
    case hypercast.Success(value):
        print("timestamp", value)  # an aware UTC datetime, to the microsecond
    case hypercast.Fault(reason, offset):
        print(reason.name, "at char", offset)
```

Door names mirror the native ABI (`cast_i32`, `cast_f64`, `cast_decimal`, `cast_timestamp`,
…); inputs are `str` or `bytes`, both zero-copy views, and a `Fault`'s span comes back in
the caller's own units — byte offsets for `bytes`, code-point offsets for `str` — so
`text[offset:offset + length]` slices the offending text back out of whatever you passed,
no mapping needed. Exhaustiveness is the type checker's job here —
pair the two cases with `typing.assert_never` under mypy/pyright for the compile-time
guarantee the static bindings get natively. Python-flavored fidelity, stated honestly:
`int` is unbounded, so `cast_u64` returns the true unsigned value with no bit-pattern
games; `cast_decimal` returns an exact, canonical `decimal.Decimal`; `datetime`'s
resolution is microseconds, so the core's nanoseconds truncate by three digits on the
temporal doors — `datetime`'s own ceiling, not the parser's, and said out loud rather than
discovered later.

The exhaustiveness check, spelled out — `Verdict[int]` is `Success[int] | Fault`, what
`cast_i32` returns, and the fall-through arm stops type-checking the moment either case
above it goes missing (`assert_never` is in `typing` on every supported Python):

```python
from typing import assert_never


def describe(verdict: hypercast.Verdict[int]) -> str:
    match verdict:
        case hypercast.Success(value):
            return f"got {value}"  # value: int
        case hypercast.Fault(reason, offset, length):
            return f"{reason.name} at {offset}"
        case _:
            assert_never(verdict)
```

### The doors

Every door takes the text first — `str` or `bytes` — and returns `Success(value)` or
`Fault(reason, offset, length)`:

| Door | Reads | `Success.value` |
| --- | --- | --- |
| `cast_bool(text)` | the natural-language boolean lexicon | `bool` |
| `cast_char(text)` | one character verbatim, or a declared code point (`65`, `U+0041`, `0x41`, `&H41`, `&#65;`, `&#x41;`) | `str` of length 1 |
| `cast_i8` `cast_i16` `cast_i32` `cast_i64` `(text, fmt)` | a signed integer under a declared `NumFormat` | `int` |
| `cast_u8` `cast_u16` `cast_u32` `cast_u64` `(text, fmt)` | an unsigned integer under a declared `NumFormat` | `int` |
| `cast_f32` `cast_f64` `(text, fmt)` | a real under a declared `NumFormat` | `float` |
| `cast_decimal(text, fmt)` | an exact decimal under a declared `NumFormat` | `decimal.Decimal` |
| `cast_uuid(text)` | every .NET `Guid` form, plus `urn:uuid:`-style prefixes | `uuid.UUID` |
| `cast_timestamp(text)` | an RFC 3339 instant | `datetime`, aware, UTC |
| `cast_unix(text, precision)` | an integer Unix-epoch value in a declared `UnixPrecision` | `datetime`, aware, UTC |
| `cast_excel_serial(text, epoch)` | an Excel date serial in a declared `ExcelEpoch` | `datetime`, aware, UTC |
| `cast_date(text)` | a strict ISO `yyyy-MM-dd` date | `date` |
| `cast_date(text, order)` | a separated date in a declared `DateOrder` | `date` |
| `cast_datetime(text, order, *, am="", pm="")` | a zone-less civil date-time in a declared `DateOrder`, a culture's day periods declared beside the English `AM`/`PM` | `datetime`, naive |
| `cast_time(text)` | an ISO 24-hour time of day | `time` |
| `cast_duration(text)` | ISO 8601, the invariant colon form, or protobuf JSON seconds | `timedelta` |
| `cast_decimal_from_float(value)` | a `float` as the shortest decimal that names it (`0.1` is one tenth) | `decimal.Decimal` |
| `cast_excel_serial_from_float(value, epoch)` | an Excel serial `float` in a declared `ExcelEpoch` | `datetime`, naive |
| `cast_excel_time(value)` | the fraction of an Excel serial `float` | `time` |
| `cast_excel_duration(value)` | a `float` count of days | `timedelta` |
| `cast_text(text)` | the text itself, once it is valid text (`bytes` UTF-8, a `str` with no lone surrogate), untrimmed | `str` |

Nothing about the text is guessed; what a door cannot know from the text, the caller
declares, as an `IntEnum`:

- **`UnixPrecision`** — `SECONDS`, `MILLISECONDS`, `MICROSECONDS`, `NANOSECONDS`: the unit of
  a Unix-epoch value, never inferred from its magnitude.
- **`ExcelEpoch`** — `Y1900` (the Windows default, phantom February 29th 1900 included) or
  `Y1904` (the legacy Macintosh system): a workbook-level setting no serial number carries.
- **`DateOrder`** — `YEAR_MONTH_DAY`, `MONTH_DAY_YEAR`, `DAY_MONTH_YEAR`: `"1/7/2026"` is
  January 7th or July 1st only because the caller said which. Without one, `cast_date`
  stays strict ISO.

```python
from hypercast import DateOrder, ExcelEpoch, UnixPrecision

hypercast.cast_unix("1700000000123", UnixPrecision.MILLISECONDS)
# Success(value=datetime.datetime(2023, 11, 14, 22, 13, 20, 123000, tzinfo=datetime.timezone.utc))
hypercast.cast_excel_serial("45000", ExcelEpoch.Y1900)
# Success(value=datetime.datetime(2023, 3, 15, 0, 0, tzinfo=datetime.timezone.utc))
hypercast.cast_date("1/7/2026", DateOrder.DAY_MONTH_YEAR)
# Success(value=datetime.date(2026, 7, 1))
hypercast.cast_datetime("1/7/2026 3:04 PM", DateOrder.MONTH_DAY_YEAR)
# Success(value=datetime.datetime(2026, 1, 7, 15, 4))
hypercast.cast_datetime("2026-01-07 오후 3:04", DateOrder.YEAR_MONTH_DAY, am="오전", pm="오후")
# Success(value=datetime.datetime(2026, 1, 7, 15, 4))
hypercast.cast_duration("PT1H30M")
# Success(value=datetime.timedelta(seconds=5400))
```

`cast_datetime` reads the English `AM`/`PM` whatever is declared — ASCII, either case, after
the clock (`3:04 PM`) or ahead of it (`PM 3:04`). A culture's own designators are declared
beside them as the keyword-only `am` and `pm` — German `vorm.`/`nachm.`, Korean `오전`/`오후`,
Spanish `a. m.`/`p. m.` — and read exactly as spelled, after the clock with an optional space
before (`3:04 nachm.`) or ahead of it with an optional space after (`오후 3:04`). Nothing is
guessed from the text: undeclared, `nachm.` is the first offending character of a `MALFORMED`
fault. A designator is 1 to 16 UTF-8 bytes with no ASCII digit and no whitespace at either
end (inside is fine, as in `a. m.`); anything else is a `ValueError` at the call, a caller's
bug like a `NumFormat` that cannot be.

A `Fault`'s `reason` is a `CastFailure` — `EMPTY` (nothing there, or only whitespace),
`MALFORMED` (present, but not the target type) or `OUT_OF_RANGE` (well-formed, outside the
target's range) — compared with `is`. `hypercast.optional(verdict)` presents an `EMPTY`
fault as `None` and passes everything else through, for the column where blank means
absent:

```python
hypercast.optional(hypercast.cast_i32("   ", hypercast.NumFormat.INVARIANT))  # None
# Fault(reason=<CastFailure.MALFORMED: 2>, offset=0, length=1), passed through:
hypercast.optional(hypercast.cast_i32("abc", hypercast.NumFormat.INVARIANT))
```

Bad data is always a `Fault`. What raises is a caller's bug: `TypeError` for an argument of the wrong type (text that is neither `str` nor
`bytes`, a format that is not a `NumFormat`, a `float` bound), `ValueError` for a declaration
that names no member of its enum, a `NumFormat` or a `Constraints` that cannot be (below) or a
day-period designator that cannot be, `OverflowError` for an integer
too wide for the 32 unsigned bits a flag set or a declaration is. The one exception that is
about data: a `str` holding a lone surrogate has no UTF-8 form, so at the doors in the table
above, `cast_text` aside, it is a `UnicodeEncodeError` before the door runs. `cast_text` and
the checked doors ([below](#validation-constraints)) read it with the surrogate kept, never
replaced, as `MALFORMED` text at its own code point.

See [the repo root README](../README.md) for the receipts and the state of every other
language binding.

## Declared formats: separators, lenience flags, currency, signs, digits

Every numeric door — the integer family, `cast_f32`/`cast_f64`, and `cast_decimal` — takes
a `NumFormat`: the decimal and group separators (1 to 8 UTF-8 bytes each: `,` and `.`, a
no-break space, the `, ` some feeds carry), a bitwise OR of the lenience flags (`GROUPING`,
`PARENTHESES`, `EXPONENT`, `RADIX_PREFIXES`, `PERCENT`, `CURRENCY`; `ALL` is all six —
`TRAILING_SIGN` is opted into separately, from a culture's negative pattern, and
`SEPARATOR_DETECT` is a separator *policy*, not a lenience, opted into separately too), an
optional currency symbol, and then, as keyword arguments, what a culture spells differently:
`currency_decimal_sep` and `currency_group_sep`, the separators a currency amount is read
with; `minus_sign`, `plus_sign` and `percent`, further spellings of the signs and the
percent symbol; and `digit_zero`, the zero of a set of native digits. `NumFormat.INVARIANT`
is `.`/`,` with every lenience on and nothing else declared; `NumFormat.DETECT` is the same
with `SEPARATOR_DETECT` added, so the `.`/`,` roles are resolved per input from its
structure (`1.234.567,89` and `1,234,567.89` both read) and an undecidable one is a
`MALFORMED` fault, never a guess; `NumFormat.from_localeconv()` bridges
`locale.localeconv()` ([below](#the-locale-bridge)). A format is immutable and reads back
through `decimal_sep`, `group_sep`, `flags`, `currency` and the keyword fields by their
own names, each `""` when it declares nothing. Unlike the C#, Java, Swift and PHP doors,
these take no platform format object (Ruby's take Rails' words as keywords instead): Python's `locale` is process-global, and
`from_localeconv()` already reads it.

A currency symbol is declared, never guessed. With `CURRENCY` set and a symbol declared, the
symbol is accepted once, leading (before or after the sign: `$5`, `-$5`, `$ -5`) or trailing
(`5 €`, `1.234,50 kr.`), with optional whitespace between it and the digits, and accounting
parentheses wrap the symbol along with the digits (`($5)`). Declared but with the flag off,
the symbol is simply the first offending byte of a `MALFORMED` fault; with no symbol declared
the flag matches nothing. A symbol is 1 to 16 UTF-8 bytes with no ASCII digit or whitespace —
anything else is a `ValueError` at construction, a caller bug like equal separators.

```python
dollars = hypercast.NumFormat(".", ",", hypercast.NumFormat.ALL, "$")
hypercast.cast_i32("($1,234)", dollars)  # Success(value=-1234)
hypercast.cast_decimal("$1,234.50", dollars)  # Success(value=Decimal('1234.5'))

krone = hypercast.NumFormat(",", ".", hypercast.NumFormat.ALL, currency="kr.")
hypercast.cast_decimal("1.234,50 kr.", krone)  # Success(value=Decimal('1234.5'))
```

The one rule behind every declared symbol: **the ASCII spelling is always read, and a
declared symbol is a further spelling read beside it** — so nothing that parsed before a
declaration stops parsing after it. What each field adds:

- **Signs** — `minus_sign` and `plus_sign` are read wherever ASCII `-` and `+` are: ahead of
  the digits, before or after a leading currency symbol (`−5`, `€−5`, `−€5` with U+2212, as
  Estonian and Basque write it). Two signs are `MALFORMED` at the second.
- **`TRAILING_SIGN`** — the sign after the digits, as a culture whose negative pattern is
  `n-` writes it: `5-`, `5 -`, `5+`, `5 €-`, `5- €`, `2.5%-`, `2.5-%`; ASCII or declared,
  never beside a leading sign and never inside accounting parentheses. Not in `ALL`:
  `from_localeconv()` sets it from the locale's sign position, and you set it from yours.
- **Currency separators** — with `CURRENCY` set, a symbol declared, *and* the symbol found
  in the text (either edge), the digits are read with `currency_decimal_sep` and
  `currency_group_sep`, each falling back to its number counterpart when not declared —
  the way the BCL switches to a culture's `CurrencyDecimalSeparator` when it sees the
  symbol (Swiss German groups a number on `'` and writes money with a `,` decimal). A plain
  number keeps the number separators. Under `SEPARATOR_DETECT` detection governs the whole
  token and the currency separators are ignored.
- **Percent** — `percent` is read exactly as a trailing ASCII `%` is, under `PERCENT`, on
  the real and decimal doors (`50٪` is `0.5`); the integer doors never read a percent.
- **Native digits** — the ten code points from `digit_zero` (`٠` U+0660 declares the
  Arabic-Indic digits, `０` U+FF10 the fullwidth ones) are digits everywhere ASCII `0`–`9`
  are: integer part, fraction, exponent, mixed with ASCII in one token. Radix-prefixed
  (`0x`, `0b`) digits stay ASCII.

```python
estonian = hypercast.NumFormat(",", "\u00a0", hypercast.NumFormat.ALL, "€", minus_sign="−")
hypercast.cast_f64("−1\u00a0234,5", estonian)  # Success(value=-1234.5)
hypercast.cast_f64("-1\u00a0234,5", estonian)  # Success(value=-1234.5) — ASCII is always read

swiss = hypercast.NumFormat(
    ".",
    "'",
    hypercast.NumFormat.ALL | hypercast.NumFormat.TRAILING_SIGN,
    "CHF",
    currency_decimal_sep=",",
)
hypercast.cast_decimal("CHF 1'234,50", swiss)  # Success(value=Decimal('1234.5'))
# No symbol, so the number separators: Success(value=Decimal('1234.5'))
hypercast.cast_decimal("1'234.50", swiss)
hypercast.cast_decimal("1'234,50 CHF-", swiss)  # Success(value=Decimal('-1234.5'))

arabic = hypercast.NumFormat("٫", "٬", hypercast.NumFormat.ALL, percent="٪", digit_zero="٠")
hypercast.cast_f64("١٬٢٣٤٫٥", arabic)  # Success(value=1234.5)
hypercast.cast_decimal("٥٠٪", arabic)  # Success(value=Decimal('0.5'))
```

What the core refuses to carry is refused at construction, as a `ValueError` — the C ABI's
own contract rules, so a `NumFormat` that exists is one the core reads: a separator, sign
or percent symbol that is not 1 to 8 UTF-8 bytes or contains an ASCII digit, equal
separators (the number pair, or the pair in force for a currency amount, counting what each
falls back to), a sign or percent symbol with ASCII whitespace in it, a currency symbol over
16 UTF-8 bytes or carrying an ASCII digit or whitespace, a digit zero that is ASCII or starts no Unicode digit block.

Not declared, by decision: `cast_bool`'s lexicon stays English-only. No platform carries
culture data for a boolean vocabulary, so no bridge could fill such a field — a caller maps
a culture's words before the door.

`cast_decimal` is the real doors' grammar with an exact result: a `decimal.Decimal` built
from the core's sign, 96-bit magnitude and base-10 scale, never a float in between. The
result is canonical: exact trailing zeros in the fraction are trimmed, so `"0.1"` is one
tenth, `"50%"` is `Decimal('0.5')`, `"1.10"`, `"1.1"` and `"1.1000"` are all `Decimal('1.1')`
(`as_tuple()` gives digits `(1, 1)`, exponent `-1`), and zero is `Decimal('0')`, never negative.
Text past 96 bits of magnitude or 28 nonzero places is `OUT_OF_RANGE` — nothing but a zero is
ever dropped, never rounded, the one thing a caller who reached for a decimal is entitled to
assume.

### The locale bridge

`NumFormat.from_localeconv()` reads `locale.localeconv()` — or a dict shaped like it — and
fills every field the C locale has a column for: `decimal_point` and `thousands_sep` as the
separators, `currency_symbol`, `mon_decimal_point` and `mon_thousands_sep` as the currency
separators, `negative_sign` and `positive_sign` as the signs (U+2212 for Estonian and Basque;
an ASCII sign is already read, so it declares nothing), and an `n_sign_posn` of `2` or `4` —
the sign after the amount — as `TRAILING_SIGN`. `localeconv()` has no percent symbol and no
digits, so those stay ASCII. A separator the locale leaves empty takes its default (`.`/`,`
for the number pair, the number pair for the currency pair) unless the other separator
already holds that character, in which case it takes the other of the pair — a comma-decimal
locale with no thousands separator groups on `.` rather than colliding. Every lenience is on;
narrow the flags, or construct a `NumFormat` outright, for anything stricter.

```python
# With no argument, the process's own locale; here the fields et_EE.UTF-8 reports.
fmt = hypercast.NumFormat.from_localeconv(
    {"decimal_point": ",", "thousands_sep": "\xa0", "currency_symbol": "€", "negative_sign": "−"}
)
fmt  # NumFormat(',', '\xa0', 95, '€', minus_sign='−')
hypercast.cast_f64("−1\xa0234,5", fmt)  # Success(value=-1234.5)
```

`hypercast.native_version()` names the core actually loaded, `"major.minor.patch"`, decoded
from the same packed `hypercast_version` export every other binding probes.

## Validation constraints

A value that parsed can still break a business rule. `Constraints` declares the rule with the
OpenAPI Schema Object's keywords, bound to one `Door` (`Door.F64`, `Door.DECIMAL`, `Door.TEXT`,
…), and a checked door judges the value by it, in the core and in exact decimal arithmetic:

```python
from decimal import Decimal

import hypercast
from hypercast import Checked, Constraints, Door, Fault, Keyword, NumFormat, Rounding, Violation

# Latitude: a multiple of 0.000001 between -90 and 90, finer input rounded half to even.
latitude = Constraints(
    Door.F64, minimum="-90", maximum="90", multiple_of="0.000001", rounding=Rounding.MIDPOINT
)
# Euro cash: a multiple of 0.05, and anything else is wrong.
euro_cash = Constraints(Door.DECIMAL, multiple_of=Decimal("0.05"))
# A sex code: one of three, whatever case and padding it arrived in.
sex = Constraints(Door.TEXT, trim=True, enum=["Female", "Male", "Other"])

euro = NumFormat(",", ".", NumFormat.ALL, "€")
hypercast.cast_checked("45.1234567", NumFormat.INVARIANT, latitude)
# Checked(value=45.123457, adjusted=True, enum_index=None)
hypercast.cast_checked("-90.5", NumFormat.INVARIANT, latitude)
# Violation(keyword=Keyword.MINIMUM, offset=0, length=5)
hypercast.cast_checked_from_float(45.1234567, latitude)
# Checked(value=45.123457, adjusted=True, enum_index=None)
hypercast.cast_checked("€1.234,55", euro, euro_cash)
# Checked(value=Decimal('1234.55'), adjusted=False, enum_index=None)
hypercast.cast_checked("€12,37", euro, euro_cash)
# Violation(keyword=Keyword.MULTIPLE_OF, offset=0, length=6)
hypercast.cast_checked("12,3x", euro, euro_cash)
# Fault(reason=<CastFailure.MALFORMED: 2>, offset=4, length=1)


def describe(verdict: hypercast.CheckedVerdict[str]) -> str:
    match verdict:
        case Checked(value, _, enum_index):
            return f"{value}, entry {enum_index}"  # " MALE " -> Male, entry 1
        case Fault(reason, offset):
            return f"{reason.name} at char {offset}"  # "   " -> EMPTY at char 0
        case Violation(Keyword.ENUM, offset, length):
            return f"not a sex code: chars {offset}+{length}"  # "Femal" -> chars 0+5
        case Violation(keyword):
            return f"breaks {keyword.name}"  # keywords are open


describe(hypercast.cast_text(" MALE ", sex))  # 'Male, entry 1'
```

A checked door answers `Checked(value, adjusted, enum_index)` — the value, `adjusted` (true only
when rounding changed it) and, at `Door.TEXT` with an `enum`, the index of the entry it matched
(`None` otherwise, an integer `enum` included, where the value is the answer) — or a
`Rejection`, which is `Fault | Violation`: the plain doors' own `Fault` (it did not parse: the
very type, so a parse failure reads the same at either door) or a `Violation(keyword, offset,
length)` (it parsed and broke a rule: the keyword and the value's span, in the same units as a
fault's, code points for `str` and bytes for `bytes`). `CheckedVerdict[T]` is
`Checked[T] | Fault | Violation`, and a `match` over the three type-checks as exhaustive with
`assert_never`. `Keyword` is a class with constants (`Keyword.MINIMUM` … `Keyword.ENUM`), not
an enum: its `name` is the OpenAPI spelling (`exclusiveMinimum`), and `format` and `pattern` may
join in a minor release, a code this binding does not name yet arriving as `Keyword(9)`, named
`"keyword 9"`. So a `match` over keywords needs its default arm (the `Violation(keyword)` case
above). The plain doors and their `Success | Fault` are unchanged.

The integer doors take `minimum`, `exclusive_minimum`, `maximum`, `exclusive_maximum`,
`multiple_of` with a `rounding` (`Rounding.STRICT`, the default, then `MIDPOINT`, half to even,
`FLOOR` and `CEILING`) and `enum`; `Door.F32`, `Door.F64` and `Door.DECIMAL` the same without
`enum`; `Door.TEXT` takes `min_length`, `max_length` (in code points), `trim` (Unicode
`White_Space`, U+00A0 included) and `enum`, matched under Unicode simple case folding and
delivering the caller's own declared `str` and its index. The constructor takes every keyword
at every door, and the core alone refuses one its door does not take. The first failure wins:
the door parses, the text is trimmed, `multiple_of` rounds or refuses, then the bounds, the
lengths and `enum` judge the result. A float door also judges the float it delivers against
each bound read as the door reads it: `"1e-400"` at `Door.F64` reads as `0`, so it breaks
`exclusive_minimum="0"`. Bounds, steps and integer entries are a `decimal.Decimal`, an `int` or
a `str` in invariant notation, all converted exactly by the core (`"1e-6"` is `0.000001`, and
`Decimal("1E-29")`, past the 28 places a core decimal holds, is refused rather than rounded),
never a `float`, so `0.1` cannot be declared inexactly: a `float` bound is a `TypeError`.
`cast_checked(text, fmt, constraints)` is every numeric door's checked form over `str` or
`bytes`, and `cast_checked_from_float(value, constraints)` judges a number a source already
decoded on the shortest decimal that names it (`0.1 + 0.2` breaks `multiple_of="0.1"` under
`STRICT`, and rounds to `0.3` under `MIDPOINT`), with an empty span. `cast_text(text,
constraints)` is the text door's.

A declaration is judged once, by the core, when it is built, and one that cannot mean anything
at its door is the caller's bug: the constructor raises `ValueError`, its message the rule
(`Constraints(Door.U8, minimum="-1")` raises "Constraints refused (code 8): a bound, step or
enum entry the door cannot hold."). An `enum` left `None` is none, while `enum=[]` declares
one no value could match and is refused (code 12), as is a text entry no input can match
(code 13: an empty one, or under `trim` one with leading or trailing white space). There is no
declaration without a door, so no `Constraints`
means "no rules" by accident; `Constraints(Door.F64)` declares none on purpose. Handing a
declaration to the other kind of door (text constraints to `cast_checked`, numeric ones to
`cast_text`) is a `ValueError` too, and after that a checked cast judges only data and raises
nothing for it. `cast_constraints_check(door, **keywords)` answers the same refusal as its code,
`0` to `13`, without raising, and `cast_declared_decimal(text)` is the conversion every bound
crosses. A lone surrogate in a `str` is `MALFORMED` at its own code point, never replaced, and in
an `enum` entry it is refused as an entry that is not UTF-8 (code 9). A `Constraints` is
immutable and hashable, and its `repr` is the call that builds it again.

## Interop: building on HyperCast's C ABI

For a library that carries HyperCast's verdicts across a C ABI of its own — HyperTabular's,
for one — and has to hand its native layer a format in the core's own layout
rather than a copy of it. `NumFormat.packed` is those 92 bytes, the size-tagged layout the
core reads in place: the size (`92`), the flags and the native digits' zero code point (`0`
for none) as little-endian `u32`s at offsets 0, 4 and 8; then each symbol as its UTF-8
zero-padded to its field, followed by its byte length — 8 + 1 bytes each for the decimal
separator (offset 12), group separator (21), currency decimal separator (30), currency group
separator (39), minus sign (48), plus sign (57) and percent symbol (66), and 16 + 1 for the
currency symbol (75) — the same bytes as the Rust crate's `RawNumFormat::to_le_bytes`. A
symbol declared empty is all zeros. A format's `repr` is the constructor call that builds it
again — the four positional arguments, then only the keywords that declare something — so one
can be logged and restored:

```python
fmt = hypercast.NumFormat(".", ",", hypercast.NumFormat.ALL, "$")
fmt  # NumFormat('.', ',', 95, '$')
fmt.packed  # the 92 bytes:
# b'\\\x00\x00\x00' + b'_\x00\x00\x00' + bytes(4)         size 92, flags 95, no native digits
#   + b'.' + bytes(7) + b'\x01' + b',' + bytes(7) + b'\x01'  the separators, each its UTF-8 then its length
#   + bytes(45)                                            five symbols declared empty
#   + b'$' + bytes(15) + b'\x01'                           the currency symbol
```

A `Constraints` crosses the same way: `packed` is the 72-byte `RawConstraints` layout and
`packed_enum` the `enum` list beside it, in the native byte order the core reads in place (the
size, flags, rounding code, lengths and a reserved zero as `u32`s, then the lower bound, upper
bound and `multiple_of` as 16-byte decimals; integer entries as such decimals, text entries a
little-endian `u32` byte length followed by the UTF-8), which the core reads only when the
flags carry `HAS_ENUM` (256), set whenever an `enum` is declared. The extension calls the
core's Rust API directly rather than its C ABI, with a door for each of the core's 32 exports
(`hypercast_version` as `native_version`, `cast_date_ordered` as `cast_date`'s order).

The values themselves are built on the Rust side: an extension module of its own that
presents them enables the crate's `python-values` feature and uses
`hypercast::python::Values`, the conversions this package's own extension makes, so a
`datetime`, `decimal.Decimal` or `uuid.UUID` it hands out is the object `cast_*` would have
returned (see [rust/README.md](../rust/README.md#interop-building-on-hypercasts-c-abi)).

## In the browser (Pyodide)

The same PyO3 extension, compiled for Pyodide's Emscripten target, is published to PyPI as
`hypercast-<version>-cp311-abi3-pyemscripten_2026_0_wasm32.whl` (about 150 KB), so micropip
finds it the way pip finds the native wheels:

```python
import micropip

await micropip.install("hypercast")

import hypercast

hypercast.cast_i32("(1,234)", hypercast.NumFormat.INVARIANT)  # Success(value=-1234)
hypercast.cast_timestamp("2026-01-02T10:04:05Z")
```

It has the same API and no JavaScript bridge: every door is the same Rust call it is in
CPython, and the verdicts are bit-for-bit the same. CI installs the wheel into Pyodide and runs this package's whole pytest
suite in it twice, under Node and in headless Chrome, shared corpus replay included.

The wheel is for **Pyodide 314.x** (Python 3.14, platform `pyemscripten_2026_0`). It is
`abi3`, but a Pyodide ABI is one Python minor built with one exact Emscripten, so each
Pyodide ABI year needs its own wheel: earlier Pyodide lines (0.29.x and older) find no wheel,
and the next line is covered by the release that adds its build.

## Why not `int()` / `fromisoformat` / `dateutil`?

1. **Verdicts, not exceptions** — bad data is the expected case for untrusted text, and a
   `Fault` is a reason plus a span, not a `ValueError` to catch and regex.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, currency symbols, signs, percent symbols and native
   digits, radix prefixes, a culture's day periods, all five .NET `Guid` text forms plus
   `urn:uuid:` prefixes, protobuf JSON durations — with each lenience individually
   declared, never guessed.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (the whole suite green, in CPython and in Pyodide, the full
   corpus replayed).
4. **Native-extension speed** — the escape from the interpreted tier is this binding's own
   receipt: the cost of calling native code from Python is *ctypes*, not the call
   (~1 µs of interpreted marshalling per call, measured). The numbers are under
   [Benchmarks](#benchmarks).

**The honest trade-off:** for plain invariant integers `int()` still wins — it's a
C-accelerated builtin with no boundary to cross. These doors earn their keep on the
culture-machinery parsers, the closed error contract, and cross-language agreement.

## Benchmarks

Measured with pyperf on linux-x64 (an Intel Core
i9-11900H), CPython 3.14.7 as Fedora builds it, each door beside the closest thing the
stdlib has:

| Door | HyperCast | stdlib | Verdict |
| --- | ---: | ---: | --- |
| `cast_datetime("1/7/2026 3:04 PM", MONTH_DAY_YEAR)` | 230 ns | 4.65 µs `datetime.strptime` | **20x faster** |
| `cast_date("1/7/2026", MONTH_DAY_YEAR)` | 201 ns | 3.51 µs `datetime.strptime` | **17x faster** |
| `cast_uuid` | 262 ns | 798 ns `uuid.UUID()` | **3.0x faster** |
| `cast_timestamp` | 252 ns | 146 ns `datetime.fromisoformat` | 1.7x slower |
| `cast_f64` | 131 ns | 72 ns `float()` | 1.8x slower |
| `cast_i32` | 103 ns | 56 ns `int()` | 1.8x slower |
| `cast_i32` (grouped) | 116 ns | — | |
| `cast_bool` | 89 ns | — | |
| `cast_duration` (ISO) | 262 ns | — | |

The messy-feed doors are where the gap is widest, because `strptime` is the only stdlib
parser that accepts their input at all — and it is *slow*. The three losses are each to a C
builtin that reads one shape and has no boundary to cross: `int()` and `float()` on plain
numbers, and `fromisoformat` on a timestamp. What a door returns for the difference is a
verdict rather than an exception, and the forgiveness — grouping, declared separators, a
currency symbol — at ~115-160 ns for grammar the stdlib doesn't sell at any price.

Past the parse, what a door costs is the Python object it hands back, so those are built
the cheapest way the stable ABI allows: a `uuid.UUID` is allocated and its slots set through
the C API with no `__init__` (the core has already validated the text); a `datetime`, `date`
or `time` comes from the packed form the `datetime` module's own pickling uses; and a
`timedelta` from subtracting two datetimes, which is several times cheaper than its
constructor. All of it is inside the stable ABI, so one `abi3` wheel still covers every
CPython from 3.11 with the GIL, and one `abi3t` wheel every CPython from 3.15.

Separator detection costs ~15 ns: `1.234.567,89` under `NumFormat.DETECT` is 157 ns
against 142 ns for the same text under a declared eurozone format.

A declared currency symbol costs nothing measurable here: in a separate run for 0.9.0,
`cast_i32("1,234,567")` is 104 ns under `NumFormat.INVARIANT` and
under a format declaring `$`. Rejection is where the verdict
pays off. `cast_i32("1,234x567")` returns its `Fault` in 140 ns, while `int("1234x567")`
takes 645 ns to raise its `ValueError` and have it caught: 4.6x.

The culture fields are free until used: a `NumFormat` is resolved once, at construction, and
a door hands the core the same pre-resolved struct whatever it declares, so signs, percent,
currency separators and native digits cost a door nothing on text that does not carry them.
`cast_datetime`'s keyword-only `am` and `pm` are the one declaration parsed per call: with
nothing declared the door takes the core's English path and pays about 15 ns for the
keyword signature (measured before and after on the same build, interleaved), and declaring
a pair costs about 75 ns more — two keyword arguments extracted and checked against the
designator rules on every call.

Reproduce: `maturin develop --release` (from `python/`, inside a virtualenv — `pyproject.toml`
already points maturin at `../rust/Cargo.toml` and the `python` feature) to build the release
extension, then `pip install pyperf` and `python bench_cast.py --fast`.

## Verifying provenance

Every wheel PyPI serves, the Pyodide one included, and the sdist verify from the release tag alone: the
release attestation, signed by this repository's `release.yml` at the tag, names every one of
them, with the release inventory as its predicate.

```sh
pip download hypercast==<version> --no-deps -d .
curl -sLO "$(curl -s https://pypi.org/pypi/hypercast/<version>/json |
  jq -r '.urls[] | select(.packagetype=="sdist") | .url')"
for f in hypercast-<version>*; do
  gh attestation verify "$f" --repo SkunkWerkx/HyperCast \
    --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
done
```

Each wheel and the sdist also carry build provenance where each was built, installed and attested:
`hyper-build-wheels.yml` in `SkunkWerkx/.github`, at the preparation commit on master the
release was prepared from (`release.yml` verifies each wheel and the sdist against it before publishing it
unchanged). That check names the signer, or uses `--owner` for both:

```sh
for f in hypercast-<version>*; do
  gh attestation verify "$f" \
    --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
  # or: gh attestation verify "$f" --owner SkunkWerkx
done
```

Both are the GitHub/Sigstore transparency-log route, checked with `gh attestation verify`, the
same route every other artifact in this project uses. Separately, PyPI itself holds a
[PEP 740](https://peps.python.org/pep-0740/) publish attestation for every wheel and the sdist, filed by
`gh-action-pypi-publish` at publication and shown on each file's PyPI page, which PyPI-side
tooling checks on its own. The root README's [Provenance](../README.md#provenance) section has
the whole picture.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
