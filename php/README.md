# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Packagist](https://img.shields.io/packagist/v/skunkwerkx/hypercast.svg)](https://packagist.org/packages/skunkwerkx/hypercast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**A real `Success|Fault` union type on every door — the value, or a closed backed-enum
reason plus the exact byte span that offended — over PHP's own built-in ext-ffi. Zero
Composer runtime dependencies, no extension to compile, no runtime bridge.**

Allocation-lean scalar casts — booleans, characters, the full integer family, reals, exact
decimals, UUIDs, temporals — calling directly into the native `libhypercast` Rust core. PHP 8.2 is
the floor (readonly classes, enums); both verdict classes are `final` and every door's
return type declares the union, which is as closed as PHP's type system can state it.
Bundles a native build for every supported platform (see [Requirements](#requirements))
and picks the right one at runtime.

```php
use HyperCast\{Cast, NumFormat, Success, Fault};

$verdict = Cast::i32('(1,234)', NumFormat::invariant());
echo match (true) {
    $verdict instanceof Success => "got {$verdict->value}",          // -1234
    $verdict instanceof Fault => "{$verdict->reason->name} at byte {$verdict->offset}",
};
```

Door names mirror the native ABI (`i32`, `f64`, `timestamp`, …); PHP strings are raw
bytes, so inputs cross verbatim and fault offsets need no mapping. PHP-flavored fidelity,
stated honestly: `int` is 64-bit signed, so u64 carries the two's-complement bit pattern
(render with `sprintf('%u', ...)`); `DateTimeImmutable` tops out at microseconds, so the
core's nanoseconds truncate by three digits; durations come back as the protobuf pair
(`Duration`) because `DateInterval` can't carry them; decimals come back as the core's
exact triple (`Decimal`) because PHP has no decimal type at all.

## Requirements

- **PHP 8.2 or later**, 64-bit.
- **`ext-ffi`**, loaded and permitted for your SAPI — see [Enabling FFI](#enabling-ffi)
  below. No other extension is required (not even mbstring), and there is no Composer
  dependency; ext-intl is an optional `suggest`, needed only to pass a `\NumberFormatter`
  ([below](#from-an-intl-numberformatter)).
- **A supported platform.** The package bundles one native library per platform and picks
  at load:

  | Platform | Bundled library |
  | --- | --- |
  | Linux x64 / arm64, glibc 2.34 or newer | `linux-x64`, `linux-arm64` |
  | Linux x64 / arm64, musl (Alpine) | `linux-musl-x64`, `linux-musl-arm64` |
  | macOS x64 / arm64 | `osx-x64`, `osx-arm64` |
  | Windows x64 | `win-x64` |

  glibc 2.34 means Debian 12, Ubuntu 22.04, RHEL 9, Amazon Linux 2023 or newer; an older
  glibc fails when the library loads. musl is detected from the running process, so an
  Alpine image needs nothing extra. Windows on ARM hardware loads the x64 library, because
  PHP itself is an x64 process there — PHP has never shipped a native Windows ARM64 build.
  Anything else (a 32-bit PHP, another architecture, another OS family) is a clear
  unsupported-platform error rather than a wrong-library load.

- **About 3.9 MB on disk.** The Composer package is GitHub's archive of the release tag,
  trimmed to PHP by `.gitattributes`, except for `go/`, which is over half of it: mostly
  the Go module's static libraries, which PHP never loads. The Go module proxy builds its
  module from the same archive, so leaving `go/` out would break the Go module, and moving
  PHP to a repository of its own is not worth it for the users it has. This will stay as it
  is unless PHP sees wide adoption.

### Enabling FFI

`ext-ffi` ships with PHP, but the `ffi.enable` ini setting decides who may use it, and its
default is `preload`:

| `ffi.enable` | CLI | Web SAPIs (FPM, Apache, `php -S`) |
| --- | --- | --- |
| `preload` (the default) | works | works only from preloaded code |
| `1` | works | works |
| `0` | refused | refused |

So the CLI works out of the box, and a web SAPI needs one of two things in `php.ini`
(`ffi.enable` is a system-level setting — `ini_set()` and per-directory overrides cannot
change it):

- `ffi.enable=1`, which permits FFI to every script the server runs; or
- keep the default and preload this package, which permits FFI to it alone:

  ```ini
  opcache.preload=/path/to/your/preload.php
  ; opcache.preload_user=www-data   ; required when the server starts as root
  ```

  ```php
  // preload.php
  $src = __DIR__ . '/vendor/skunkwerkx/hypercast/php/src';
  foreach ([...glob("$src/*.php"), ...glob("$src/Interop/*.php")] as $file) {
      opcache_compile_file($file);
  }
  ```

  PHP has no preloading on Windows; use `ffi.enable=1` there.

Without either, the first call throws `FFI\Exception: FFI API is restricted by "ffi.enable"
configuration directive`. Under a web SAPI the library is bound once per request — PHP's
statics reset between requests — while the operating system keeps it mapped for the
worker's lifetime.

## Checking availability

`Cast::isAvailable()` answers whether the native library can be used at all, and never
throws: a missing `ext-ffi`, an `ffi.enable` that restricts FFI for this SAPI, a missing or
unloadable library, an unsupported platform, and a stale library lacking a symbol this
binding declares all answer `false`. It attempts the same load every door makes and caches
the answer for the request, so it is what a consumer with a fallback gates on:

```php
$total = Cast::isAvailable()
    ? Cast::decimal($text, $format)
    : $legacyParser->parse($text);
```

`Cast::nativeVersion()` returns the loaded library's own `"major.minor.patch"` — a
zero-argument probe the core exports, so a host can prove the `libhypercast` it resolved is
the one this binding was written against before making the first cast. It throws exactly
where `isAvailable()` answers `false`. If you catch instead of asking, catch `\Throwable`:
ext-ffi reports its own failures as `\Error`s (`FFI\Exception` extends `\Error`, and a
missing extension is a plain `Error: Class "FFI" not found`), which `catch (\Exception)`
does not see.

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
UTF-8 (`$`, `€`, `kr.`, `CHF`, `R$`, `руб.`) with no ASCII digit or whitespace, or the
constructor throws as the caller bug it is. With `NumFormat::CURRENCY` set (it is part of
`NumFormat::ALL`) the symbol is accepted once, leading (`$5`, `-$5`, `$ -5`) or trailing
(`5 €`, `1.234,50 kr.`), with optional whitespace between it and the digits; accounting
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
before from parsing. A symbol the core cannot carry (an ASCII digit inside one, whitespace
in a sign, a currency pair that collides, a digit zero that starts no digit block) is an
`InvalidArgumentException` from the constructor, the caller bug it is.

| Argument | What it declares |
| --- | --- |
| `minusSign`, `plusSign` | A further spelling of each sign — U+2212 for Estonian, Swedish and Finnish; a bidi mark and a hyphen for Arabic and Hebrew — read wherever ASCII `-`/`+` are: ahead of the digits, before or after a leading currency symbol. Two signs is `Malformed` at the later one. |
| `currencyDecimalSep`, `currencyGroupSep` | The separators a currency amount is read with: in force when `CURRENCY` is set, a symbol is declared *and* the symbol is found in the text. Each falls back to its number counterpart; under `SEPARATOR_DETECT` the structure governs the whole token and these are ignored. Austrian German groups a number with a no-break space and an amount with `.`. |
| `percent` | A further spelling of `%` (`٪`, `％`), read as a trailing `%` is under `PERCENT`: the real and decimal doors only, since an integer door never reads a percent. |
| `digitZero` | The zero of a set of native digits — `٠` (U+0660) for the Arabic-Indic digits, `۰` (U+06F0) for the extended ones, `０` (U+FF10) for the fullwidth ones. The ten code points from it are read beside `0`–`9` everywhere digits are (integer part, fraction, exponent), and the two may mix; radix-prefixed (`0x`, `0b`) digits stay ASCII. |

The two number separators may be one or two characters each — up to 8 bytes of UTF-8 —
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
designator is matched byte for byte after the clock (`3:04 nachm.`, an optional space
before it) or ahead of it (`오후 3:04`, an optional space after), and the English `AM`/`PM`
are always read in both places, ASCII case-insensitively — so a declared pair adds
spellings and takes none away. Each designator is up to 16 bytes of UTF-8 with no ASCII
digit and no ASCII whitespace at either end (an internal space is fine: Spanish's `a. m.`);
anything else is an `InvalidArgumentException`. Only the datetime door reads them —
`Cast::date` has no time part — and the two-argument call is exactly what it was.

```php
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

The conversion is remembered per formatter, so passing the same formatter call after call
converts it once; but its ten symbols and two negative-pattern attributes are still read on
every call, because a formatter is mutable (`setSymbol()`, `setPattern()`) and a changed one
is declared again, and those twelve intl calls are what the formatter path costs over a
hoisted `NumFormat` — about four times the call (2.4 µs against 0.6 µs in the run that
measured the grouped `Cast::i32` at 645 ns), so derive the `NumFormat` once with
`NumFormat::fromNumberFormatter()` on a hot path. Symbols the core cannot carry (a separator
of more than 8 bytes, an over-long currency symbol) are an `InvalidArgumentException`, as
through the constructor.

ext-intl is optional: it is a `suggest` in `composer.json`, nothing in the package loads it,
and without it the doors take a `NumFormat` exactly as before. CI's Alpine suite runs with
ext-ffi alone and covers that.

The packed format the core reads is 92 bytes — a size word, the flags, the digit zero's
code point, then each symbol as its bytes and its length; `NumFormat::packed()` is exactly
those bytes. `Cast` writes it once per distinct `NumFormat` instance, so hoist a format
rather than constructing one per call. A `DayPeriods` is written the same way — 40 bytes,
`DayPeriods::packed()` — once per distinct instance.

## Interop: building on HyperCast's C ABI

For a package that carries HyperCast's verdicts across a C ABI of its own, as HyperTabular
does: it reads the core's fields out of its own buffers and needs the values `Cast` would
have returned. `HyperCast\Interop` holds the code the doors themselves use, so the two cannot
drift:

- `NativeValues` — the value builders `instant($seconds, $nanos)`,
  `date($year, $month, $day)`, `civil($year, $month, $day, $nanosOfDay)` and `uuid($bytes)`;
  `writeFormat(NumFormat, FFI\CData)`, which fills the core's 92-byte format struct as every
  numeric door does, and `writeDayPeriods(DayPeriods, FFI\CData)`, its 40-byte day-periods
  struct as the datetime door does; `fault($code, $offset, $length)`, a `\LogicException` for a code that
  names no reason; and `version($packed)`, a `*_version()` word as `"major.minor.patch"`. A
  decimal is built by `Decimal::fromLimbs()` and a span by `new Duration(...)`, both public
  already.
- `NativePlatform` — parameterized by the library's base name:
  `ridAndLibraryName('hypertabular')` is this process's `native/{rid}/` directory and file
  name, `resolve()` the same table as a pure function, and
  `libraryPath($baseName, $sourceDir, $override)` the library to load — the file the
  `$override` environment variable names, else the staged build, else the in-repo cargo
  build — exactly as `Cast` finds `libhypercast` (its variable is
  `HYPERCAST_NATIVE_LIBRARY`).

## Why not `filter_var` / `DateTimeImmutable::createFromFormat`?

1. **Verdicts with location** — `filter_var` hands back `false` (indistinguishable from a
   parsed `false`, famously); a `Fault` is a reason and a span.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, radix prefixes, all five .NET `Guid` text forms plus
   `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (every corpus file replayed by phpunit, with byte-exact fault
   spans).
4. **Level with the platform's own parser on a timestamp** — and behind it on the shapes
   PHP has a dedicated function for; both are printed under [Benchmarks](#benchmarks) below.

**The honest trade-off:** a native library shipped inside the package and an FFI call per
door — for plain invariant integers, `(int)` casts and `ctype_digit` are the reasonable
choice.

## Benchmarks

phpbench (`XDEBUG_MODE=off vendor/bin/phpbench run --report=aggregate --retry-threshold=5`,
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

Read it as a floor, not a race. Every door costs 270-680 ns, and nearly all of that is the
`ext-ffi` call and the PHP around it rather than the parse, which the core finishes in tens
of nanoseconds. PHP's own date functions are C running inside the engine with no boundary to
cross, and how they compare depends on the machine (this one runs them at 440-560 ns). What a door
buys for its few hundred nanoseconds is what `intval` and `createFromFormat` do not return
— a verdict with a reason and a span instead of `0` or `false`, a format declared per call,
and the same answer in six other languages.

No new mechanism was needed to get here — the ext-ffi call floor is already extension-class,
so the work was a wrapper diet: flat doors (one FFI call, no closure indirection), typed
cdef structs read as fields, static scratch `CData` with pre-taken addresses (PHP's request
model makes static scratch safe), and `createFromTimestamp`/`setMicrosecond` on PHP 8.4+
instead of a date-string parse. Separator detection costs ~54 ns (441 ns vs 387 ns
declared).

A declared currency symbol is lost in the call's own cost: in a later run where
`Cast::i32` (grouped) took 449 ns, the same text under `NumFormat::fromNumberFormatter` for
en_US, `$` included, took 472 ns, inside that run's ±3%. Rejecting `1,234x567` took 684 ns
there. PHP's validating parse, `filter_var('1234x567', FILTER_VALIDATE_INT)`, fails in
30 ns, and returns `false` with no reason or span.

When bytes are the destination — a `BINARY(16)` column bind, a wire format —
`Cast::uuidBytes` returns the sixteen RFC-ordered octets as a binary string and skips the
hex encoding and hyphen assembly `Cast::uuid` does to render the canonical form. The eight
integer doors are now written out flat like the real doors — one literal FFI call each, no
shared helper doing a dynamic symbol lookup and a string match to find the width's
sign-extension shift.

Benchmark forensics worth knowing: PHP read 20x slow until a loaded Xdebug was caught
inflating everything uniformly ~14x — `XDEBUG_MODE=off` for every recorded number.

### The native extension spike

**The `skunkwerkx/hypercast` Composer package (see Install below) is `ext-ffi` only** —
chosen because it needs zero compilation to install.

The same Rust core also links straight into a real Zend extension via
[`ext-php-rs`](https://ext-php.rs) (`rust/src/php_ext.rs`, gated behind the crate's `php`
Cargo feature) — the same move Python (PyO3) and Ruby (Magnus) get a shipped native backend
for, exposing every door at the raw layer this package's own FFI calls sit at. PHP's didn't
ship, for two reasons. The mechanism was never the bottleneck here the way ctypes and Fiddle
were: the `ext-ffi` crossing measured ~105 ns, so what a Zend extension removes is the
PHP-level wrapper around the call, not the call. And a Zend extension is pinned to one PHP
ABI per build — the API number plus NTS or ZTS — with no Windows build on stable Rust, so
shipping it means a binary per PHP version where the `ext-ffi` package ships one library per
platform. CI builds the extension on every Linux and macOS leg, load-checks it, and uploads
and attests the result, so it cannot silently bit-rot; no `phpunit` runs against it and
nothing in the Composer package loads it. HyperUuid carries the same spike on the same terms
and measured it: 1.4-2x on single calls, where that wrapper is a large share of the time, and
nothing on batches ([its PHP README](https://github.com/SkunkWerkx/HyperUuid/tree/master/php#the-native-extension-spike)
has the table). No measurement of this repo's spike is recorded, so no number is claimed
here. If you want to try it anyway, here's how to build and load it yourself:

1. **Prerequisites:** a Rust toolchain ([rustup](https://rustup.rs)) and PHP's development
   headers (the `php-dev` / `php8.5-dev` / `php-devel` package for your distro — `ext-php-rs`'s
   build script needs these to link against `libphp`).
2. **Build it** with the `php` feature, not the plain default build — that produces the
   `ext-ffi` binding's cdylib, a different entry point from the same crate; don't load both
   at once. `cargo php-ext` is an alias in `rust/.cargo/config.toml` that builds into its own
   `target/php/` directory, so it can't overwrite the plain cdylib the other bindings load:
   ```sh
   git clone https://github.com/SkunkWerkx/HyperCast
   cd HyperCast/rust
   cargo php-ext
   ```
   Produces `target/php/release/libhypercast.so` (`.dylib` on macOS; Windows isn't supported —
   `ext-php-rs`'s Windows path needs a nightly-only Rust feature, so every CI leg here builds
   Linux/macOS only).
3. **Load it** — either add `extension=/absolute/path/to/target/php/release/libhypercast.so` to
   `php.ini`, or pass it ad hoc: `php -d extension=/absolute/path/to/target/php/release/libhypercast.so your_script.php`.
   Verify with `php -m | grep hypercast`.
4. **Call it.** This extension is a benchmark spike, not a polished second backend, so it
   exposes flat `hypercast_native_*` functions taking the raw text and returning a packed
   verdict array, not this package's `Cast` API: `[0, ...value]` on success, or
   `[reason, offset, length]` on a fault. A numeric door takes its format as the 92 packed
   bytes `NumFormat::packed()` builds — the same bytes `Cast` hands the `ext-ffi` path — and
   the datetime door its day-period designators as two strings, both empty for `AM`/`PM`
   alone:
   ```php
   hypercast_native_cast_bool("yes");                  // [0, true]
   hypercast_native_cast_bool("maybe");                // [2, 0, 5] — reason, offset, length
   hypercast_native_cast_u64("42", NumFormat::invariant()->packed());             // [0, 42]
   hypercast_native_cast_datetime("1/7/2026 3:04 nachm.", 3, "vorm.", "nachm."); // [0, 2026, 7, 1, 54240000000000]
   ```
   See [`rust/src/php_ext.rs`](../rust/src/php_ext.rs) for the full function list — one
   `hypercast_native_cast_*` per door, plus `hypercast_native_version`, which returns the
   packed integer (`major << 16 | minor << 8 | patch`) rather than `Cast::nativeVersion()`'s
   string.

The same extension is also the route to PHP in the browser, documented under
[WebAssembly](#webassembly).

## WebAssembly

**In the browser: proven, not shipped.** The [native extension spike](#the-native-extension-spike)
runs inside WordPress Playground's prebuilt PHP for the browser (`@php-wasm/web`, and
`@php-wasm/node` for node) as a side module — no custom PHP build — verified under node and in
headless Chromium against PHP 8.5.10 (`@php-wasm/*` 3.1.56, October 2026): the version probe,
the boolean, integer, real, UUID, date and duration doors, a fault span, an out-of-range
verdict, the 64-bit `u64` pattern and the contract-violation exception. HyperUuid proved the
same route first with its own extension, and the recipe below is the one it found. It is not
shipped, in either package, because of what shipping it costs, not because it does not work:
one module per supported PHP minor (each must match its PHP version exactly), a ~4 GB Docker
image in CI to build them, and a distribution channel still to choose, for a binding whose
Composer package is FFI only.

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
   const r = await php.run({ code: '<?php echo json_encode(hypercast_native_cast_bool("yes"));' });
   ```

   The functions are the spike's `hypercast_native_*` packed-verdict API; a browser rollout
   would wrap them the way `Cast` wraps the FFI calls.

Rolling it out would mean a CI job per PHP minor (steps 1–5, about ten minutes each cold,
then the same calls in headless Chrome) and somewhere to publish the modules and manifests. The forge's
[levers not pulled](https://github.com/SkunkWerkx/.github#levers-deliberately-not-pulled)
table carries the decision for both packages.

**Running the core as wasm inside PHP** is not built: there is no maintained wasm engine PHP
can embed, so there is nothing to stand that on. The root
README's [WebAssembly section](../README.md#webassembly) tracks both directions for every
binding.

## Verifying provenance

Packagist has nothing of its own to attest: there is no packed artifact, just a git tag it
resolves against this repo. What is worth checking is the native libraries committed under
`php/src/native/`, and each verifies from the release tag alone: the release attestation,
signed by this repository's `release.yml` at the tag, names every one of them, with the
release inventory as its predicate.

```sh
composer require skunkwerkx/hypercast:<version>
gh attestation verify vendor/skunkwerkx/hypercast/php/src/native/linux-x64/libhypercast.so \
  --repo SkunkWerkx/HyperCast --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

Each library also carries build provenance from `hyper-build-native.yml` in
`SkunkWerkx/.github`, at the preparation commit the tagged commit was staged from
(`stage-native-binaries.yml` verifies that before committing a byte, and the staging commit's
`Release-Source-SHA` and `Release-CI-Run-ID` trailers name the commit and the run), so that
check needs `--signer-repo` beside `--repo`:

```sh
gh attestation verify vendor/skunkwerkx/hypercast/php/src/native/linux-x64/libhypercast.so \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
```

Get the signer wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which
reads like a bad signature but is only an identity mismatch. The root README's
[Provenance](../README.md#provenance) section has the whole picture: the inventory, which
artifacts are signed from where, and the gates every byte passes before publication.

## Install

```sh
composer require skunkwerkx/hypercast
```

Published to [Packagist](https://packagist.org/packages/skunkwerkx/hypercast) — no extra
repository configuration needed. See [Requirements](#requirements) for the PHP floor,
`ffi.enable` and the supported platforms.

There are two `composer.json` files in this repo: this directory's own (what CI actually
`composer install`s/tests against) and a second one at [the repo root](../composer.json),
which exists because Packagist requires `composer.json` at the top of the git repository it
watches, with no subdirectory support. Its `autoload` PSR-4 mapping points into `php/src/`.
Keep both in sync by hand when `require`/`autoload` change here.

The native libraries under `src/native/{rid}/` are committed to git, not built by Packagist —
Packagist has no packing step, so the git tree at the tag *is* the package. They are restaged
for each release by `stage-native-binaries.yml`; see `src/native/README.md`.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
