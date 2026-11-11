# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Packagist](https://img.shields.io/packagist/v/skunkwerkx/hypercast.svg)](https://packagist.org/packages/skunkwerkx/hypercast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**A real `Success|Fault` union type on every door — the value, or a closed backed-enum
reason plus the exact byte span that offended — from `hypercast`, a PHP extension installed
prebuilt by [PIE](https://github.com/php/pie): no compiler, no Composer dependency, no FFI.**

Allocation-lean scalar casts — booleans, characters, the full integer family, reals, exact
decimals, UUIDs, temporals — from the Rust core, built into a Zend extension with
[ext-php-rs](https://ext-php.rs). The whole API is the extension's: the classes below exist
once it is loaded, with no PHP code in your `vendor/`. PHP 8.3 is the floor, and every
supported platform and PHP minor gets a prebuilt, attested zip (see [Install](#install)).
Both verdict classes are `final` and every door's return type declares the union, which is
as closed as PHP's type system can state it.

```php
<?php

use HyperCast\{Cast, NumFormat, Success, Fault};

$verdict = Cast::i32('(1,234)', NumFormat::invariant());
echo match (true) {
    $verdict instanceof Success => "got {$verdict->value}",          // -1234
    $verdict instanceof Fault => "{$verdict->reason->name} at byte {$verdict->offset}",
}, "\n";

$timestamp = Cast::timestamp('2026-01-02T15:04:05.123456789+05:00');
echo match (true) {
    $timestamp instanceof Success => 'timestamp ' . $timestamp->value->format('Y-m-d\TH:i:s.uP'), // UTC, microseconds
    $timestamp instanceof Fault => "{$timestamp->reason->name} at byte {$timestamp->offset}",
}, "\n";
```

Door names mirror the native ABI (`i32`, `f64`, `timestamp`, …); PHP strings are raw
bytes, so inputs cross verbatim and fault offsets need no mapping. PHP-flavored fidelity,
stated honestly: `int` is 64-bit signed, so u64 carries the two's-complement bit pattern
(render with `sprintf('%u', ...)`); `DateTimeImmutable` tops out at microseconds, so the
core's nanoseconds truncate by three digits; durations come back as the protobuf pair
(`Duration`) because `DateInterval` can't carry them; decimals come back as the core's
exact triple (`Decimal`) because PHP has no decimal type at all.

## Install

```sh
pie install skunkwerkx/hypercast
```

[PIE](https://github.com/php/pie), the PHP Foundation's extension installer, resolves the
package on [Packagist](https://packagist.org/packages/skunkwerkx/hypercast), downloads the zip
built for your platform and PHP minor from the GitHub Release, installs it into the
extension directory and enables it. Nothing compiles: the package has no source build, so where
no zip fits, PIE stops with its own error that no matching asset exists. `composer require`
does not install an extension; to have Composer check that it is loaded, put
`"ext-hypercast": "*"` in your project's `require`.

The extension needs no other extension. ext-intl is optional, needed only to pass a
`\NumberFormatter` ([below](#from-an-intl-numberformatter)), and ext-ffi only for the two
[interop](#interop-building-on-hypercasts-c-abi) writers that fill an `FFI\CData`.

PIE itself needs:

- PHP 8.1 or newer to run, with the `iconv` and `openssl` extensions: the phar's requirement
  checker stops without them.
- The `zip` extension or an `unzip` binary, to unpack the download.
- On macOS, the Xcode Command Line Tools (`xcode-select --install`). PIE tells macOS's libc
  from glibc with `otool`; without it, it looks for a glibc zip, which does not exist for
  macOS. Homebrew's PHP needs them already.

### Platforms and PHP versions

Every platform and minor ships twice: NTS (non-thread-safe), the build every distribution's
`php` CLI and FPM packages ship, and ZTS (thread-safe), the build FrankenPHP, Apache's threaded
MPMs and the thread-safe Windows downloads run. PIE picks the one that matches the PHP it
installs into. 36 zips a release, each one load-checked into the PHP it is named for, and run
through the full suite there, before it is packed:

| Platform | PHP 8.3 | PHP 8.4 | PHP 8.5 | Zip on the release (NTS, ZTS) |
| --- | :---: | :---: | :---: | --- |
| Linux x64, arm64 (glibc 2.34 or newer) | ✅ | ✅ | ✅ | `php_hypercast-v<version>_php<minor>-{x86_64,arm64}-linux-glibc-{nts,zts}.zip` |
| Linux x64, arm64 (musl: Alpine) | ✅ | ✅ | ✅ | `php_hypercast-v<version>_php<minor>-{x86_64,arm64}-linux-musl-{nts,zts}.zip` |
| macOS arm64 | ✅ | ✅ | ✅ | `php_hypercast-v<version>_php<minor>-arm64-darwin-bsdlibc-{nts,zts}.zip` |
| Windows x64 | ✅ | ✅ | ✅ | `php_hypercast-v<version>-<minor>-{nts,ts}-{vs16,vs17}-x86_64.zip` (VS16 for 8.3, VS17 from 8.4) |

On a thread-safe PHP the extension loads at startup only, from the `extension=hypercast` line PIE
writes to php.ini (or `docker-php-ext-enable hypercast` in a Docker image): a `dl()` there stops the
script with a warning naming that line. On an NTS PHP `dl()` loads it as before.

glibc 2.34 means Debian 12, Ubuntu 22.04, RHEL 9, Amazon Linux 2023 or newer. An extension is
tied to one PHP minor, so a zip built for 8.4 is refused by 8.5; PIE picks by the PHP it runs
against. Windows on ARM hardware takes the x64 zip, because PHP itself is an x64 process there:
PHP has never shipped a native Windows ARM64 build. Intel macOS has no zip: Homebrew bottles
no PHP for it any more, and nothing here compiles a PHP to build against. A 32-bit PHP is not
supported.

### Checking it is loaded

Without the extension the classes don't exist, so `extension_loaded('hypercast')` is the
check for a consumer with a fallback:

```php
use HyperCast\{Cast, NumFormat};

$text = '1,234.50';
$total = extension_loaded('hypercast')
    ? Cast::decimal($text, NumFormat::invariant())
    : $legacyParser->parse($text); // $legacyParser: the consumer's own fallback
```

`Cast::nativeVersion()` returns the extension's own `"major.minor.patch"`, the same string
as `phpversion('hypercast')`, which is the version of the core it links. `php -m` lists
`hypercast` when it is loaded.

### Stubs for IDEs and static analysis

An extension's classes have no PHP source for an IDE or an analyser to read, so the repository
carries a stub, [`php/stubs/hypercast.stub.php`](stubs/hypercast.stub.php): every class,
method, constant and enum case with its types and doc comment, regenerated from the extension
and compared in CI. PIE installs only the extension, so fetch the stub from the release's tag:

```sh
curl -LO https://raw.githubusercontent.com/SkunkWerkx/HyperCast/v<version>/php/stubs/hypercast.stub.php
```

PhpStorm indexes it like any PHP file in the project: copy it in, or add its directory under
*Settings | PHP | Include Path*. PHPStan needs it under `scanFiles`, which is how it discovers
classes that have no source (`stubFiles` alone only overrides classes it has already found, and
leaves these unknown):

```neon
parameters:
    scanFiles:
        - stubs/hypercast.stub.php
```

## 1.1: grouping policies, fault detail, date options

Three changes in 1.1, each additive in its API and one of them deliberate in its answers.

- **Grouping is Western by default.** `NumFormat::invariant()`, `detect()` and `new NumFormat(...)`
  declare Western grouping: groups of exactly three digits counted from the decimal
  separator, the leftmost one to three and not starting with a zero. `1,23,456` is a `Malformed`
  fault, spanning `,23`, and so are `12,34` and `0,125`, spanning `,34` and `0`; `1,234` and `1234`
  read. The policy is the last constructor argument, `grouping`, and a style mask never carries it
  (`flags` takes only the styles, 0 through 255):

  ```php
  new NumFormat('.', ',', NumFormat::ALL);                                       // Western
  new NumFormat('.', ',', NumFormat::ALL, grouping: NumFormat::GROUPING_INDIAN); // 1,23,456
  $format->withGrouping(NumFormat::GROUPING_WESTERN);                            // replaces the policy
  ```

  `GROUPING_WESTERN` (0) and `GROUPING_INDIAN` (512) are what the core reads from the packed flags
  word, and `GROUPING_POLICY` (768) is the field's mask, not a policy. Bit 8 (256) is retired
  permanently and never reused: the constructor, `withGrouping()` and `unserialize()` refuse 256 and
  768. A `NumFormat` unserialized from 1.0 carries no policy and is Western. Its `flags` must hold
  styles alone: `unserialize` refuses bit 8 (`351`, and `-1`, which 1.0 read as every style), the
  Indian bit (`607`, since the policy is the `grouping` property) and anything past them (`1119`),
  though 1.0 ignored those bits. A policy doesn't turn grouping on: with `GROUPING` off a group
  separator stays forbidden.

  The bridges follow the platform's number pattern: `fromNumberFormatter()` the ICU ext-intl links,
  `fromLocaleconv()` the C library. A group of three then groups of two is Indian (`en_IN`), and
  anything else Western. On ICU, `ml`, `mr` and `or` have an Indian number pattern and the currency
  pattern `¤#,##0.00` on every version measured (66.1 to 79.1), and so does `ta` on 79.1 (`¤ #,##0.00`;
  on 66.1 to 78.3 its currency is Indian too). A Western `₹1,234,567.50` through such a format is
  `Malformed` until the column declares `withGrouping(NumFormat::GROUPING_WESTERN)`. The reverse is
  `mr_IN` on glibc, `[3, 0]` for numbers and `[3, 2, 0]` for currency: its bridge is Western, and its
  own `₹12,34,567.50` is `Malformed` until the column declares
  `withGrouping(NumFormat::GROUPING_INDIAN)`. A shape neither policy reads (`tok` on ICU, `cmn_TW` and
  `unm_US` on glibc) is `Malformed` under a declared format, `invariant()` or a bridge, never misread,
  and its ungrouped numerals read, while `NumFormat::detect()` infers by the delimiter rule, so
  `1,2345` is 1.2345 there:
  [docs/grouping.md](https://github.com/SkunkWerkx/HyperCast/blob/master/docs/grouping.md#parked).
  One format has one `GROUPING` flag for both: it is cleared only when neither pattern groups (the C
  locale); when just one does not, the flag stays and that pattern's values stay valid ungrouped,
  since a format cannot forbid currency grouping on its own. The rule is fixed, never
  inferred from the input. `fromNumberFormatter()` reads the formatter's own pattern (its
  secondary size of zero repeats the primary: `#,##0` is groups of three) and its locale's
  pattern for the other of the number and currency styles, made with `NumberFormatter::create()`
  from `getLocale()`, so a customised formatter is judged by what it was set to; a percent
  pattern is outside the rule. `fromLocaleconv()` reads `grouping` and `mon_grouping` (the last
  size repeats; `[3, 2]` is Indian, empty is none); an array with neither key says nothing, so it
  is Western.

- **Fault detail** rides a result path of its own. `Fault` is `final readonly` and PHP compares
  objects property by property (`==`, `assertEquals`, `serialize`), so a `detail` on it would
  change what every existing fault equals. `HyperCast\DetailedCast` has every `Cast` door under
  the same name and parameters and returns a `Success` or a `DetailedFault`: the same reason,
  offset and length, the raw `code` the core reported, and `detail`, the `FaultDetail` case
  that names it (`FaultDetail::GroupSize`). A code this binding does not name (a newer
  core's, or one constructed: `new DetailedFault(CastFailure::Malformed, 0, 1, 999)`) is
  `FaultDetail::Unknown` with the code kept, never an error, so a `match` over details wants a
  `default` arm. `$fault->toFault()` is the plain `Fault`. A checked door's `Violation` carries
  no detail.

- **Date options** are opt-in: no door changes unless it is handed options. `DateOptions` holds
  flags (`COMPACT_DATE`: `20260107` at `date`; `SPACE_SEPARATOR` and `HOUR_OFFSET` at
  `timestamp`; `COMMON_FORMS` is all three, with no pivot) and a pivot (the last year of a
  100-year window, 100 through 9999) for two-digit years at `date` with an order and at
  `datetime`. `MonthNames` is a caller-built vocabulary, `[month => name or list of names]`,
  read by the same two doors in the month field the order selects, matched under simple case
  folding with the longest name followed by the separator winning; punctuation is part of a
  name (`janv.` with `.` as the separator reads `7.janv..2026`). Both are trailing optional
  parameters of `Cast` and `DetailedCast`: `Cast::date($text, $order, $options, $months)`,
  `Cast::datetime($text, $order, $periods, $options, $months)`, `Cast::timestamp($text,
  $options)`.

**FFI consumers and a 1.0 core.** A core older than 1.1 reads every
format by 1.0's rule, `0,125` as 125 and `1,23,456` as 123456; the 1.0 C exports on a 1.1 core read every
format Western: an Indian format written by
`Interop\NativeValues::writeFormat()` must go to the detailed exports (`cast_detailed` and its
siblings), which read the policy. The guard is opt-in: `writeFormat($format, $target, $ffi)` takes
the `FFI` handle the core was loaded with and refuses any format unless that handle declares
`cast_detailed` (a 1.0 library lacks it) and `hypercast_version()`, and the core's version has
this extension's major and a minor no older (the patch is ignored; `NativeValues::compatible()` is
that rule). Without a handle there is no check: the caller owns it. The extension's own core is
linked in, so nothing else can meet an old one.

## Doors

| Door | Value on `Success` |
| --- | --- |
| `Cast::bool` | `bool` |
| `Cast::char` (`A`, `U+00E9`, `&#233;`, `0x41`) | `string` — the one scalar, UTF-8-encoded |
| `Cast::i8` … `Cast::i64`, `Cast::u8` … `Cast::u64` | `int` (u64 as the bit pattern) |
| `Cast::f32`, `Cast::f64` | `float` |
| `Cast::decimal` | `Decimal` — exact sign, 96-bit magnitude, base-10 scale |
| `Cast::uuid`, `Cast::uuidBytes` | canonical hyphenated string, or the 16 raw bytes |
| `Cast::timestamp`, `Cast::unix`, `Cast::excelSerial` | `DateTimeImmutable` (UTC) |
| `Cast::date`, `Cast::datetime` | `DateTimeImmutable` (UTC label, no zone read) |
| `Cast::time` | `int` nanoseconds since midnight |
| `Cast::duration` | `Duration` (the protobuf pair) |
| `Cast::decimalFromFloat`, `Cast::excelSerialFromFloat`, `Cast::excelTime`, `Cast::excelDuration` | as `decimal`, `datetime`, `time` and `duration` — read from a `float` a workbook already holds; the decimal is the shortest that names it |
| `Cast::text` | `string` — the input itself, once it is valid UTF-8; nothing trimmed |
| `Cast::checked`, `Cast::checkedFromFloat`, `Cast::checkedText` | a `Checked`, `Fault` or `Violation`: the value judged by declared `Constraints` ([below](#validation-constraints)) |

White space is Unicode's `White_Space`, the 25 code points (ASCII, U+0085, U+00A0, U+2000 to
U+200A, U+202F, U+3000 among them). Every door but `Cast::text` ignores it at both ends (`Cast::char` given exactly one character keeps it), and the
numeric doors also after a leading sign, before a trailing sign or a percent symbol, on either
side of a currency symbol and inside accounting parentheses (`"- 12"`, `"( 12 )"`, `"12,5 %"`,
`"1 234,56 €"`, the last two under a declared comma decimal separator, the last with a space group separator and `€` declared); between digits it is `Malformed`, unless a space-like group separator is
declared. An ASCII space, U+00A0 or U+202F declared as the group separator makes the input's
choice of those three immaterial (Excel writes U+00A0, ICU's `fr` U+202F, an older system a
space), a space-like separator with no digit after it is just white space, and any other
separator stays exact. Input of nothing but white space is `Empty`.

Every numeric door takes a `NumFormat` — `NumFormat::invariant()`, `NumFormat::detect()`
(the `.`/`,` roles resolved per input from structure, ambiguous input a `Malformed` fault),
or a constructed one — or an intl `\NumberFormatter`
([below](#from-an-intl-numberformatter)), and `Cast::optional()` presents an `Empty` fault as `null`. Context
the text cannot carry is declared, never guessed: through backed enums, and through
`DayPeriods` for the words a culture's 12-hour clock uses ([below](#day-periods)):

```php
use HyperCast\{Cast, DateOrder, DayPeriods, ExcelEpoch, UnixPrecision};

Cast::unix('1767348245123', UnixPrecision::Milliseconds);
Cast::excelSerial('45292.75', ExcelEpoch::Y1900);        // 2024-01-01T18:00:00Z
Cast::date('1/7/2026', DateOrder::Mdy);                  // January 7th; Dmy makes it July 1st
Cast::date('2026-01-07');                                // no order: the strict ISO door
Cast::datetime('1/7/2026 3:04 PM', DateOrder::Mdy);
Cast::datetime('1/7/2026 3:04 nachm.', DateOrder::Dmy, new DayPeriods('vorm.', 'nachm.'));
```

### Decimal

`Cast::decimal` parses under the same grammar and `NumFormat` as the real doors but never
rounds: only exact trailing zeros in the fraction are dropped, so the scale is canonical
(`"1.10"`, `"1.1"` and `"1.1000"` are all magnitude `11` at scale `1`; zero is always scale
`0`), and text carrying more precision than 96 bits and 28 places can hold is an
`OutOfRange` fault, not an approximation. `Decimal` is a zero-dependency readonly carrier — `string $magnitude`
(decimal digits, since the magnitude outgrows PHP's signed `int`), `int $scale`,
`bool $negative` — whose `__toString()` renders the canonical text; hand that string to
bcmath, GMP or ext-decimal when arithmetic is wanted, or take `toFloat()` when a lossy
float is enough.

```php
$verdict = Cast::decimal('(1,234.50)', NumFormat::invariant());
echo $verdict->value;                    // -1234.5
echo $verdict->value->magnitude;         // 12345
```

### Currency symbols

`NumFormat` takes an optional fourth argument, the currency symbol — up to 16 bytes of
UTF-8 (`$`, `€`, `kr.`, `CHF`, `R$`, `руб.`, CLDR's `F CFA`) with no ASCII digit and no white space
at either edge (inside is fine, and an ASCII space, U+00A0 and U+202F there match each other), or
the constructor throws as the caller bug it is. With `NumFormat::CURRENCY` set (it is part of
`NumFormat::ALL`) the symbol is accepted once, leading (`$5`, `-$5`, `$ -5`) or trailing
(`5 €`, `1.234,50 kr.`), with optional white space between it and the digits; accounting
parentheses wrap symbol and digits together (`($5)`). A symbol declared with the flag off is
a `Malformed` fault at the symbol, never silently ignored; the flag with no symbol matches
nothing. Every integer, real and decimal door honors it.

```php
$usd = new NumFormat('.', ',', NumFormat::ALL, '$');
Cast::i32('($1,234)', $usd);             // Success(-1234)
Cast::decimal('$ 19.99', $usd);          // Success(Decimal 19.99)
```

### The rest of a culture's notation

`NumFormat` declares everything else a culture's number text can carry, each as an
optional named argument after the currency symbol, `''` (nothing declared) by default. One
rule covers all of them: **the ASCII spelling is always read, and a declared symbol is a
further spelling read beside it** — so declaring a symbol never stops anything that parsed
before from parsing. A symbol the core cannot carry (an ASCII digit inside one, white space
in a sign, percent symbol or decimal separator, a currency pair that collides, a digit zero
that starts no digit block) is an `InvalidArgumentException` from the constructor, the caller bug it is.

| Argument | What it declares |
| --- | --- |
| `minusSign`, `plusSign` | A further spelling of each sign — U+2212 for Estonian, Swedish and Finnish; a bidi mark and a hyphen for Arabic and Hebrew — read wherever ASCII `-`/`+` are: ahead of the digits, before or after a leading currency symbol. Two signs is `Malformed` at the later one. |
| `currencyDecimalSep`, `currencyGroupSep` | The separators a currency amount is read with: in force when `CURRENCY` is set, a symbol is declared *and* the symbol is found in the text. Each falls back to its number counterpart; under `SEPARATOR_DETECT` the structure governs the whole token and these are ignored. Austrian German groups a number with a no-break space and an amount with `.`. |
| `percent` | A further spelling of `%` (`٪`, `％`), read as a trailing `%` is under `PERCENT`: the real and decimal doors only, since an integer door never reads a percent. |
| `digitZero` | The zero of a set of native digits — `٠` (U+0660) for the Arabic-Indic digits, `۰` (U+06F0) for the extended ones, `０` (U+FF10) for the fullwidth ones. The ten code points from it are read beside `0`–`9` everywhere digits are (integer part, fraction, exponent), and the two may mix; radix-prefixed (`0x`, `0b`) digits stay ASCII. |

The two number separators may each be 1 to 8 bytes of UTF-8 with no ASCII digit —
for a culture that groups with `, ` or `' `. And the flags gain `NumFormat::TRAILING_SIGN`,
which permits a sign after the digits (`5-`, `5 -`, `5+`, `5 €-`, `5- €`, `2.5%-`), never
beside a leading sign nor inside parentheses. It is deliberately not part of
`NumFormat::ALL`: the locale factories below set it from the locale's own negative pattern,
and a hand-built format declares it when its text speaks that way.

```php
$estonian = new NumFormat(',', "\u{A0}", NumFormat::ALL | NumFormat::TRAILING_SIGN, '€', minusSign: "\u{2212}");
Cast::f64("\u{2212}1\u{A0}234,5", $estonian);     // Success(-1234.5): the declared minus
Cast::f64("-1\u{A0}234,5", $estonian);            // Success(-1234.5): ASCII is always read
Cast::f64("1\u{A0}234,5 €\u{2212}", $estonian);   // Success(-1234.5): the sign after the amount

$austrian = new NumFormat(',', "\u{A0}", NumFormat::ALL, '€', currencyGroupSep: '.');
Cast::f64('€ 1.234,50', $austrian);              // Success(1234.5): an amount groups with '.'
Cast::f64('1.234,50', $austrian);                // Fault Malformed at byte 1: a plain number does not

$egyptian = new NumFormat("\u{66B}", "\u{66C}", NumFormat::ALL, digitZero: "\u{660}", percent: "\u{66A}");
Cast::f64('١٬٢٣٤٫٥', $egyptian);                 // Success(1234.5)
Cast::decimal('٥٠٪', $egyptian);                 // Success(Decimal 0.5)
```

### Day periods

`Cast::datetime` takes a `DayPeriods` beside the `DateOrder`: the words a culture writes
for the two halves of a 12-hour clock, declared the same way the order is. A declared
designator is matched byte for byte after the clock (`3:04 nachm.`) or ahead of it
(`오후 3:04`), with any run of white space or none between designator and clock (so the
`3:04\u202fPM` that ICU 72 and later write for `en_US` reads), and the English `AM`/`PM`
are always read in both places, ASCII case-insensitively — so a declared pair adds
spellings and takes none away. Each designator is up to 16 bytes of UTF-8 with no ASCII
digit and no white space at either end (an internal space is fine: Spanish's `a. m.`);
anything else is an `InvalidArgumentException`. Only the datetime door reads them —
`Cast::date` has no time part — and a two-argument call reads the English pair only.

```php
use HyperCast\{DateOrder, DayPeriods};

$german = new DayPeriods('vorm.', 'nachm.');
Cast::datetime('1/7/2026 3:04 nachm.', DateOrder::Dmy, $german);   // 2026-07-01 15:04
Cast::datetime('1/7/2026 3:04 PM', DateOrder::Dmy, $german);       // 15:04 as well: AM/PM are always read
Cast::datetime('2026-01-07 오후 3:04', DateOrder::Ymd, new DayPeriods('오전', '오후'));   // ahead of the clock
Cast::datetime('1/7/2026 3:04 nachm.', DateOrder::Dmy);            // Fault Malformed at byte 13: nothing declared
```

Like `DateOrder`, `DayPeriods` has no locale bridge: PHP's stdlib carries no day-period
data, and ext-intl's `IntlDateFormatter` formats a period without exposing it as a symbol
— declare the pair the text actually speaks.

### Booleans stay English

`Cast::bool`'s lexicon is English-only, by decision rather than omission: no platform
carries culture data for its yes/no words — ICU has none to give a `NumberFormatter`'s
counterpart — so no bridge could fill such a field, and the core has none. A caller that
reads a culture's words maps them before the door.

### From locale data

`NumFormat::fromLocaleconv(?array $conv = null)` is the platform-data factory the other
bindings carry (C# `From(CultureInfo)`, Java `from(Locale)`, Swift `from(locale:)`, Ruby
`from_i18n`, Python `from_localeconv`): it
reads `decimal_point`, `thousands_sep` and `currency_symbol` from the given array, or from
`localeconv()` when null, defaulting to `.`, `,` and no symbol wherever a field is empty,
every lenience on. An empty separator never collides with the declared one: a comma-decimal
locale that reports no thousands separator gets `.` for grouping, not a second `,`. The
rest of what `localeconv()` carries comes over too: `mon_decimal_point` and
`mon_thousands_sep` become the currency separators where they differ from the number ones;
`negative_sign` and `positive_sign` are declared as further spellings of the signs where
they are not the ASCII ones (the ASCII signs stay readable beside them); and an
`n_sign_posn` of 2 or 4 — the sign after the amount — sets `TRAILING_SIGN`. `localeconv()`
has no percent symbol and no native digits, so those stay undeclared. PHP's
`localeconv()` reflects `setlocale(LC_NUMERIC | LC_MONETARY)`
*process* state — shared across every request in the worker — so a caller that knows its
notation should declare it explicitly; the factory is for the caller that genuinely wants
whatever the process locale says.

```php
$format = NumFormat::fromLocaleconv([
    'decimal_point' => ',', 'thousands_sep' => '.', 'currency_symbol' => '€',
    'negative_sign' => "\u{2212}", 'n_sign_posn' => 2,
]);
Cast::f64('1.234,50 €', $format);        // Success(1234.5)
Cast::f64("1.234,50 €\u{2212}", $format); // Success(-1234.5): the locale's minus, after the amount
```

### From an intl `NumberFormatter`

Every numeric door (the eight integers, `f32`, `f64`, `decimal`) takes an intl
`\NumberFormatter` in place of a `NumFormat`: PHP's per-object locale formatting, with none
of `localeconv()`'s process state. The door declares everything ICU's data for the locale
says — the decimal and grouping separators and the currency symbol; the monetary separators
(`MONETARY_SEPARATOR_SYMBOL`, `MONETARY_GROUPING_SEPARATOR_SYMBOL`) as the currency
separators where they differ from the number ones; the minus, plus and percent symbols
where they are not the ASCII ones; the zero digit (`ZERO_DIGIT_SYMBOL`) as the native digits
where it is not `0`; and `TRAILING_SIGN` from a negative pattern with a suffix and no prefix
(`NEGATIVE_SUFFIX`, `NEGATIVE_PREFIX`) — with every lenience on, the same as
`NumFormat::fromNumberFormatter()`, which derives the `NumFormat` itself:

```php
$fr = new \NumberFormatter('fr_FR', \NumberFormatter::DECIMAL);
Cast::f64("1\u{202F}234,5", $fr);         // Success(1234.5): ICU groups French with U+202F
Cast::i32('1 234', $fr);                 // Fault Malformed: a plain space is not U+202F
NumFormat::fromNumberFormatter($fr);     // NumFormat(',', "\u{202F}", NumFormat::ALL, '€')

$sv = new \NumberFormatter('sv_SE', \NumberFormatter::DECIMAL);
Cast::f64($sv->format(-1234.5), $sv);    // Success(-1234.5): ICU's U+2212 minus, declared
$ar = new \NumberFormatter('ar_EG', \NumberFormatter::DECIMAL);
Cast::f64($ar->format(1234.5), $ar);     // Success(1234.5): Arabic-Indic digits and separators
```

The formatter is read on every call: its ten symbols and two negative-pattern attributes,
because a formatter is mutable (`setSymbol()`, `setPattern()`) and a changed one is declared
again. Those twelve intl calls are what the formatter path costs over a hoisted `NumFormat`
— about four times the call in a run for 0.9.0, the FFI package (2.4 µs against 0.6 µs, with
the grouped `Cast::i32` at 645 ns, separate from the Benchmarks table) — so derive the
`NumFormat` once with `NumFormat::fromNumberFormatter()` on a hot path. Symbols the core cannot carry (a separator
of more than 8 bytes, an over-long currency symbol) are an `InvalidArgumentException`, as
through the constructor.

ext-intl is optional: the extension does not depend on it, and without it the doors take a
`NumFormat` exactly as before. CI's Alpine suites run without it and cover that.

The packed format the core reads is 92 bytes — a size word, the flags, the digit zero's
code point, then each symbol as its bytes and its length; `NumFormat::packed()` is exactly
those bytes, and the core reads a door's format from that layout. A `DayPeriods` is laid out
the same way — 40 bytes, `DayPeriods::packed()`. Construct a format once and hoist it: the
constructor validates every symbol.

## Validation constraints

A value that parsed can still break a business rule. `Constraints` declares the rule with the
OpenAPI Schema Object's keywords, bound to one `Door` (`Door::F64`, `Door::Decimal`,
`Door::Text`, …), and a checked door judges the value by it, in the core and in exact decimal
arithmetic:

```php
use HyperCast\{Cast, Checked, Constraints, Decimal, Door, Fault, Keyword, NumFormat, Rounding, Violation};

// Latitude: a multiple of 0.000001 between -90 and 90, finer input rounded half to even.
$latitude = new Constraints(Door::F64, minimum: '-90', maximum: '90', multipleOf: '0.000001', rounding: Rounding::Midpoint);
// Euro cash: a multiple of 0.05, and anything else is wrong.
$euroCash = new Constraints(Door::Decimal, multipleOf: new Decimal('5', 2, false));
// A sex code: one of three, whatever case and padding it arrived in.
$sex = new Constraints(Door::Text, trim: true, enum: ['Female', 'Male', 'Other']);

$euro = new NumFormat(',', '.', NumFormat::ALL, '€');
Cast::checked('45.1234567', NumFormat::invariant(), $latitude);  // Checked(45.123457, adjusted: true)
Cast::checked('-90.5', NumFormat::invariant(), $latitude);       // Violation(Keyword::Minimum, 0, 5)
Cast::checkedFromFloat(45.1234567, $latitude);                   // Checked(45.123457, adjusted: true)
Cast::checked('€1.234,55', $euro, $euroCash);                    // Checked(Decimal 1234.55)
Cast::checked('€12,37', $euro, $euroCash);                       // Violation(Keyword::MultipleOf, 0, 8)
Cast::checked('12,3x', $euro, $euroCash);                        // Fault Malformed at byte 4

function describe(Checked|Fault|Violation $verdict): string
{
    return match (true) {
        $verdict instanceof Checked => "{$verdict->value}, entry {$verdict->enumIndex}",  // ' MALE ': Male, entry 1
        $verdict instanceof Fault => "{$verdict->reason->name} at byte {$verdict->offset}", // '   ': Empty at byte 0
        $verdict instanceof Violation => match ($verdict->keyword) {
            Keyword::Enum => "not a sex code: bytes {$verdict->offset}+{$verdict->length}", // 'Femal': bytes 0+5
            default => "breaks {$verdict->keywordName()}",                                 // keywords are open
        },
    };
}

echo describe(Cast::checkedText(' MALE ', $sex));               // Male, entry 1
```

A checked door answers a `Checked` — `$value`, `$adjusted` (true only when rounding changed it)
and, at `Door::Text` with an `enum`, `$enumIndex`, the index of the entry it matched (`null`
otherwise, an integer `enum` included, where the value is the answer) — or one of two
rejections: the plain doors' own `Fault` (it did not parse: the very class) or a `Violation` (it
parsed and broke a rule: `$keyword` and the byte span, `$offset` and `$length`, as a fault's).
All three are `final readonly`, and every checked door declares `Checked|Fault|Violation`.
`Cast::optional()` takes a checked verdict as it takes a plain one: an `Empty` fault is `null`,
and a `Checked` or any other rejection, a `Violation` included, comes back as it was. A PHP
string is bytes, so text that is not UTF-8 (a lone surrogate's three bytes, a stray byte) reaches
every door as it is: `Malformed` at its bytes, the same fault at a plain door as at a checked
one. `Keyword` is an int-backed enum of the eight
keywords, its values the core's codes; PHP enums are closed, so it is open the other way:
`format` and `pattern` may join in a minor release, and a `Violation` carries a code this
binding does not name yet as the bare `int` (`Violation::$keyword` is `Keyword|int`). So a
`match` over keywords needs its `default` arm. `$violation->keywordName()` is the OpenAPI
spelling (`exclusiveMinimum`, the case's name with its first letter lowered), or `keyword 9`.
The plain doors and their `Success|Fault` are unchanged.

The integer doors take `minimum`, `exclusiveMinimum`, `maximum`, `exclusiveMaximum`,
`multipleOf` with a `rounding` (`Rounding::Strict`, the default, then `Midpoint`, half to even,
`Floor` and `Ceiling`) and `enum`; `Door::F32`, `Door::F64` and `Door::Decimal` the same without
`enum`; `Door::Text` takes `minLength`, `maxLength` (in code points), `trim` (Unicode
`White_Space`, U+00A0 included) and `enum`, matched under Unicode simple case folding and
delivering the caller's own declared string and its index. The constructor takes every keyword at
every door, by name, and the core alone refuses one its door does not take. The first failure
wins: the door parses, the text is trimmed, `multipleOf` rounds or refuses, then the bounds, the
lengths and `enum` judge the result. A float door also judges the float it delivers against each
bound read as the door reads it: `'1e-400'` at `Door::F64` reads as `0`, so it breaks
`exclusiveMinimum: '0'`. Bounds, `multipleOf` and integer entries are an `int`, a `string` in
invariant notation (`'1e-6'` is `0.000001`) or a `Decimal`, each converted exactly by the core
and kept as the `Decimal` it read; never a `float`, so `0.1` cannot be declared inexactly. The
constructor checks its arguments exactly, in weak mode too: a `float` bound is a `TypeError`
rather than a string PHP made of it, as is a `bool` length (on a release build of PHP; a debug
build coerces first). `Cast::checked($text, $format,
$constraints)` is every numeric door's checked form, and `Cast::checkedFromFloat($value,
$constraints)` judges a number a source already decoded on the shortest decimal that names it
(`0.1 + 0.2` breaks `multipleOf: '0.1'` under `Strict`, and rounds to `0.3` under `Midpoint`),
with an empty span. `Cast::checkedText($text, $constraints)` is the text door's, and
`Cast::text($text)` the plain one.

A declaration is judged once, by the core, when it is built, and one that cannot mean anything
at its door is the caller's bug: the constructor throws an `\InvalidArgumentException` whose code
is the core's refusal code and whose message is the rule (`new Constraints(Door::U8, minimum:
'-1')` throws code 8, "Constraints refused (code 8): a bound, step or enum entry the door cannot
hold."). An `enum` left `null` is none, while `enum: []` declares one no value could match and
is refused (code 12), as is a text entry no input can match (code 13: an empty one, or under
`trim` one with leading or trailing white space). The door is the constructor's one required
argument, so no `Constraints` means "no rules" by accident; `new Constraints(Door::F64)`
declares none on purpose, and one whose properties were never written (`ReflectionClass::newInstanceWithoutConstructor()`) is the
engine's `Error` at the first door that reads it. Handing a declaration to the other kind of
door (text constraints to `checked()`, numeric ones to `checkedText()`) is an
`\InvalidArgumentException` too, and after that a checked cast judges only data and throws
nothing for it. A `Constraints` is an ordinary readonly object: it compares, clones and
serializes as PHP's own, and a door lays out its properties again on every call, as it packs a
`NumFormat`, so even one `unserialize()` built is judged by the core.

## Interop: building on HyperCast's C ABI

For a package that carries HyperCast's verdicts across a C ABI of its own, as HyperTabular
does: it reads the core's fields out of its own buffers and needs the values `Cast` would
have returned. `HyperCast\Interop` holds the code the doors themselves use, so the two cannot
drift. Its two writers take an `FFI\CData`, so they need ext-ffi; nothing else does.

- `NativeValues` — the value builders `instant($seconds, $nanos)`,
  `date($year, $month, $day)`, `civil($year, $month, $day, $nanosOfDay)` and `uuid($bytes)`;
  `writeFormat(NumFormat, FFI\CData)`, which fills the core's 92-byte format struct as every
  numeric door does, and `writeDayPeriods(DayPeriods, FFI\CData)`, its 40-byte day-periods
  struct as the datetime door does; `fault($code, $offset, $length)`, a `\LogicException` for a code that
  names no reason; and `version($packed)`, a `*_version()` word as `"major.minor.patch"`. A
  decimal is built by `Decimal::fromLimbs()` and a span by `new Duration(...)`, both public
  already.
- `NativePlatform` — for a package that ships a SkunkWerkx core's builds under
  `native/{rid}/` and loads them with ext-ffi, parameterized by the library's base name:
  `ridAndLibraryName('hypertabular')` is this process's `native/{rid}/` directory and file
  name, `resolve()` the same table as a pure function, and
  `libraryPath($baseName, $sourceDir, $override)` the library to load — the file the
  `$override` environment variable names, else the staged build, else the in-repo cargo
  build.

A `Constraints` crosses the same way: `packed()` is the 72-byte `RawConstraints` layout
(`Constraints::PACKED_BYTES`) and `packedEnum()` the `enum` list beside it, in the native byte
order the core reads in place — the size, flags, rounding code, lengths and a reserved zero as
`u32`s, then the lower bound, upper bound and `multipleOf` as 16-byte decimals; integer entries
as such decimals, text entries a little-endian `u32` byte length followed by the bytes — what
the core's checked C exports read, the list only when the flags carry `HAS_ENUM` (256), which
any declared `enum` sets. The extension itself calls the core's Rust API, not the C
ABI's 36 exports (1.1's four detailed ones included).

## Why not `filter_var` / `DateTimeImmutable::createFromFormat`?

1. **Verdicts with location** — `filter_var` hands back `false` (indistinguishable from a
   parsed `false`, famously); a `Fault` is a reason and a span.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, radix prefixes, all five .NET `Guid` text forms plus
   `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (every corpus file replayed by phpunit, with byte-exact fault
   spans).
4. **Measured, not promised** — 1.0's FFI package was level with the platform's own parser on
   a timestamp and behind it on the shapes PHP has a dedicated function for; the extension
   has not been measured yet, and [Benchmarks](#benchmarks) below says what those figures are.

**The honest trade-off:** an extension to install, one build per PHP minor, and a native
call per door — for plain invariant integers, `(int)` casts and `ctype_digit` are the
reasonable choice.

## Benchmarks

**These figures predate the extension.** They measured 1.0's FFI package (recorded on
2026-10-01), and are kept until
the extension is measured on the same machine; the benchmarks in `php/bench/` now run against
the extension. phpbench (`XDEBUG_MODE=off vendor/bin/phpbench run --report=aggregate --retry-threshold=5`,
linux-x64 on an Intel Core i9-11900H, PHP 8.5; the threshold repeats a case until its
iterations agree within 5%):

| Door | HyperCast | PHP's own | Verdict |
| --- | ---: | ---: | --- |
| `Cast::timestamp` vs `new DateTimeImmutable` | 538 ns | 560 ns | level |
| `Cast::datetime` (`1/7/2026 3:04 PM`) vs `DateTimeImmutable::createFromFormat` | 673 ns | 439 ns | 1.5x slower |
| `Cast::duration` vs `new DateInterval` | 413 ns | 149 ns | 2.8x slower |
| `Cast::i32` vs `intval` | 340 ns | 22 ns | a cast, not a parser — no contest |
| `Cast::f64` vs `floatval` | 354 ns | 22 ns | the same |
| `Cast::i32` (grouped) | 352 ns | — | |
| `Cast::bool` | 273 ns | — | |
| `Cast::uuid` | 502 ns | — | |
| `Cast::date` (declared order) | 578 ns | — | |

Read it as a floor, not a race. Every door cost 270-680 ns there, and nearly all of that was
the `ext-ffi` call and the PHP around it rather than the parse, which the core finishes in tens
of nanoseconds. PHP's own date functions are C running inside the engine with no boundary to
cross, and how they compare depends on the machine (this one runs them at 440-560 ns). What a door
buys for its few hundred nanoseconds is what `intval` and `createFromFormat` do not return
— a verdict with a reason and a span instead of `0` or `false`, a format declared per call,
and the same answer in every other HyperCast binding.

A declared currency symbol is lost in the call's own cost: in a run for 0.9.0, after the
table's, where
`Cast::i32` (grouped) took 449 ns, the same text under `NumFormat::fromNumberFormatter` for
en_US, `$` included, took 472 ns, inside that run's ±3%. Rejecting `1,234x567` took 684 ns
there. PHP's validating parse, `filter_var('1234x567', FILTER_VALIDATE_INT)`, fails in
30 ns, and returns `false` with no reason or span.

When bytes are the destination — a `BINARY(16)` column bind, a wire format —
`Cast::uuidBytes` returns the sixteen RFC-ordered octets as a binary string and skips the
hex encoding and hyphen assembly `Cast::uuid` does to render the canonical form.

Benchmark forensics worth knowing: a loaded Xdebug inflates everything uniformly ~14x, so
every recorded number is taken with `XDEBUG_MODE=off`. Reproduce, in `php/` of a checkout
with the extension built (see [Development](#development)):
`composer install && XDEBUG_MODE=off composer bench -- --report=aggregate --retry-threshold=5`.

## WebAssembly: not shipped, and unproven for these classes

The extension is not built for the browser, and the recipe below has not been run with these
classes: they import SPL's and the engine's exception classes and `zend_post_startup_cb` from
PHP, which a side module has to resolve from WordPress Playground's main module, and that is
untested. What is proven is the raw build of the same crate that came before them, flat
functions returning packed verdict arrays in place of the classes, loaded as a side module into
Playground's prebuilt PHP for the browser (`@php-wasm/web`, and `@php-wasm/node` for node),
with no custom PHP build: verified under node and in headless Chromium against PHP 8.5.10
(`@php-wasm/*` 3.1.56, October 2026) for the version probe, the boolean, integer, real, UUID,
date and duration doors, a fault span, an out-of-range verdict, the 64-bit `u64` pattern and
the contract-violation exception. HyperUuid proved the same route first, and the recipe below
is the one it found. Shipping it would cost one module per supported PHP minor (each must match
its PHP version exactly), a ~4 GB Docker image in CI to build them, and a distribution channel
still to choose.

What it runs on, and the two upstream problems found on the way:

- **Playground's JSPI builds only.** Its Asyncify build cannot load third-party extensions.
- **A 64-bit `zend_long` only.** Playground builds PHP with `-DZEND_ENABLE_ZVAL_LONG64
  -D__x86_64__`, so `PHP_INT_SIZE` is 8. On a 32-bit PHP ext-php-rs does not compile
  ([ext-php-rs#800](https://github.com/extphprs/ext-php-rs/issues/800), five one-line
  fixes), and the `i64`/`u64` doors' values would not fit a PHP int there anyway.
- **Playground's `@php-wasm/compile-extension` drops four flags for Rust's C code**
  ([wordpress-playground#4377](https://github.com/WordPress/wordpress-playground/issues/4377)):
  without them the module fails to load with `bad export type for '__THREW__'`. The recipe
  passes them itself. Its README's "nightly and `-Zbuild-std`" requirement is stale: stable
  Rust (1.99 here) works.

**The recipe** (Rust stable with `wasm32-unknown-emscripten`, Docker, node):

1. Build Playground's extension image for each PHP minor, and copy its PHP headers out:

   ```sh
   npm i @php-wasm/compile-extension @php-wasm/node @php-wasm/universal
   node node_modules/@php-wasm/compile-extension/cli.js --prepare-image --php-versions 8.5
   docker cp <that image's container>:/usr/local/include/php ./php-include
   ```

2. ext-php-rs runs `php -i` and `php-config` to find the PHP it builds for, and the host PHP
   cannot stand in for the wasm one, so give it two scripts that describe the target:

   ```sh
   # fake-php-i.sh
   printf 'PHP Version => 8.5.10\nPHP API => 20250925\nDebug Build => no\nThread Safety => disabled\n'
   # fake-php-config.sh — DIR is ./php-include from step 1, absolute
   case "$1" in
     --includes) echo "-I$DIR -I$DIR/main -I$DIR/TSRM -I$DIR/Zend -I$DIR/ext -I$DIR/ext/date/lib";;
     --version) echo 8.5.10;; *) exit 1;; esac
   ```

3. Build the extension as a static library for the side module to wrap, from `rust/`, with
   Emscripten's environment sourced (the toolchain the image uses is fine):

   ```sh
   SYSROOT="$EMSDK/upstream/emscripten/cache/sysroot"
   PHP=./fake-php-i.sh PHP_CONFIG=./fake-php-config.sh \
   BINDGEN_EXTRA_CLANG_ARGS_wasm32_unknown_emscripten="--sysroot=$SYSROOT -DZEND_ENABLE_ZVAL_LONG64 -D__x86_64__" \
   CFLAGS_wasm32_unknown_emscripten="-fPIC -DZEND_ENABLE_ZVAL_LONG64 -D__x86_64__ -sSUPPORT_LONGJMP=wasm -fwasm-exceptions" \
   RUSTFLAGS="-C relocation-model=pic -C panic=abort" \
   cargo rustc --release --target wasm32-unknown-emscripten --crate-type staticlib --features php
   ```

   Not `EXT_PHP_RS_STATIC_EXT` — that is for linking into PHP itself, not a side module.

4. Wrap it as a side module. The extension directory needs only a `config.m4` and an empty C
   file, since `get_module` comes from the Rust archive:

   ```m4
   PHP_ARG_ENABLE([hypercast], [whether to enable hypercast], [AS_HELP_STRING([--enable-hypercast], [Enable hypercast])], [no])
   if test "$PHP_HYPERCAST" != "no"; then
     PHP_NEW_EXTENSION([hypercast], [hypercast_stub.c], [$ext_shared])
   fi
   ```

   ```sh
   node node_modules/@php-wasm/compile-extension/cli.js --source ./ext --name hypercast \
     --php-versions 8.5 --extra-ldflags /build/libhypercast.a --out dist
   ```

   That writes `hypercast-php8.5-jspi.so` (258 KB; 85 KB gzipped) and `manifest.json`.

5. Load it into the runtime and call it:

   ```js
   import { loadNodeRuntime } from '@php-wasm/node';   // or @php-wasm/web in a page
   import { PHP } from '@php-wasm/universal';
   const php = new PHP(await loadNodeRuntime('8.5', {
     emscriptenOptions: { processId: 1 },
     extensions: [{ source: { format: 'manifest', manifestUrl: 'dist/manifest.json' } }],
   }));
   const r = await php.run({ code: '<?php var_dump(HyperCast\\Cast::bool("yes"));' });
   ```

Rolling it out would mean a CI job per PHP minor (steps 1–5, about ten minutes each cold,
then the same calls in headless Chrome) and somewhere to publish the modules and manifests. The forge's
[levers not pulled](https://github.com/SkunkWerkx/.github#levers-deliberately-not-pulled)
table carries the decision for both packages.

**Running the core as wasm inside PHP** is not built: there is no maintained wasm engine PHP
can embed, so there is nothing to stand that on. The root
README's [WebAssembly section](../README.md#webassembly) tracks both directions for every
binding.

## Verifying provenance

PIE does not check attestations on what it downloads, so verify the zip yourself before you
trust it. Each one is named by the release attestation, signed by this repository's
`release.yml` at the tag, with the release inventory as its predicate:

```sh
gh release download v<version> --repo SkunkWerkx/HyperCast \
  --pattern 'php_hypercast-v<version>_php8.5-x86_64-linux-glibc-nts.zip'
gh attestation verify php_hypercast-v<version>_php8.5-x86_64-linux-glibc-nts.zip \
  --repo SkunkWerkx/HyperCast --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

Each zip also carries build provenance from `hyper-build-php-extension.yml` in
`SkunkWerkx/.github`, the workflow that built, tested and packed it at the release's
preparation commit, so that check needs `--signer-repo` beside `--repo`:

```sh
gh attestation verify <zip> --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
```

Get the signer wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which
reads like a bad signature but is only an identity mismatch. The root README's
[Provenance](../README.md#provenance) section has the whole picture: the inventory, which
artifacts are signed from where, and the gates every byte passes before publication.

## Development

The Composer package Packagist serves is the root [`composer.json`](../composer.json), a
`php-ext` package that PIE reads for its `php-ext` block alone: Packagist requires the manifest
at the top of the git repository it watches. This directory's `composer.json` is the
development project, never published: PHPUnit, PHPBench and PHP_CodeSniffer. To build the
extension from a checkout and run the suite against it (a Rust toolchain and PHP's development
headers, `php-config` on `PATH`):

```sh
git clone https://github.com/SkunkWerkx/HyperCast
cd HyperCast/rust
cargo php-ext                  # target/php/release/libhypercast.so (.dylib on macOS)
cd ../php
composer install
composer test                  # PHPUnit with -d extension= pointing at that build
composer bench                 # PHPBench, the same way
```

To load the build by hand, name the file for your platform:

```sh
# Linux
php -d extension=$PWD/../rust/target/php/release/libhypercast.so -r 'var_dump(HyperCast\Cast::bool("yes"));'
# macOS
php -d extension=$PWD/../rust/target/php/release/libhypercast.dylib -r 'var_dump(HyperCast\Cast::bool("yes"));'
```

`HYPERCAST_PHP_EXTENSION` names another build for `composer test` and `composer bench`.
`HYPERCAST_CHECK_STUBS=1 composer test` also regenerates the stub and compares it with the
committed one (`php tools/generate-stubs.php` rewrites it; it needs the cargo-php version the
script names, on Linux or macOS). `vendor/bin/phpcs` checks the tests, the benchmarks and the
tools. The parity tests compare the extension with 1.0's own recordings under
`tests/fixtures/v1/`, which `tools/capture-v1-fixtures.php` made from the v1.0.0 tag; they are
never edited.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
