# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Maven Central](https://img.shields.io/maven-central/v/io.github.skunkwerkx/hypercast.svg)](https://central.sonatype.com/artifact/io.github.skunkwerkx/hypercast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**Java's own discriminated union — a `sealed interface` over two records — carrying the
verdict of every cast: the value, or a closed reason plus the exact byte span that
offended. A two-arm switch with no default is proven exhaustive by `javac`; an unhandled
disposition is a compile failure.**

Allocation-lean scalar casts — booleans, characters, the full integer family, reals, exact
decimals, UUIDs, temporals — calling the native `libhypercast` Rust core in-process: through
JNI on JDK 21 to 24, and through `java.lang.foreign` (FFM) downcalls on JDK 25 and later, from
one multi-release jar. The API, the verdicts, the exceptions and their messages are the same
on both. JDK 21 is the floor, the Verdict union's own requirement (sealed interface + record
patterns + exhaustive switch), and not 17: Temurin builds no JDK 17 for Windows arm64 or
Alpine (musl) arm64, so a 17 floor could not be tested on every platform this jar ships. JDK
22 to 24 take the JNI path; they are past end of life and not tested. The jar bundles a
native build for every supported platform (Linux glibc, Linux musl, macOS, Windows ×
x64/arm64) under `/native/{rid}/`, the core and the JNI library built from it, and picks the
right one at runtime, so a consumer adds one dependency and nothing else.

```java
package com.example;

import io.github.skunkwerkx.hypercast.Cast;
import io.github.skunkwerkx.hypercast.Fault;
import io.github.skunkwerkx.hypercast.NumFormat;
import io.github.skunkwerkx.hypercast.Success;
import java.time.Instant;

public class Main {
    public static void main(String[] args) {
        String message = switch (Cast.i32("(1,234)", NumFormat.INVARIANT)) {
            case Success<Integer> s -> "got " + s.value(); // -1234, accounting negative
            case Fault<Integer> f -> f.reason() + " at char " + f.offset();
        }; // no third case: javac checked
        System.out.println(message);

        String timestamp = switch (Cast.timestamp("2026-01-02T15:04:05.123456789+05:00")) {
            case Success<Instant> s -> "timestamp " + s.value(); // UTC, all nine fractional digits
            case Fault<Instant> f -> f.reason() + " at char " + f.offset();
        };
        System.out.println(timestamp);
    }
}
```

Door names mirror the native ABI (`i32`, `f64`, `timestamp`, …) so the polyglot surface
reads identically across bindings; every text door also takes raw UTF-8 `byte[]` for callers
already holding bytes. `NumFormat.from(Locale)` bridges Java's own locale machinery —
separators, signs, percent and currency symbols, native digits, the negative pattern — to
the caller-declared format the native side reads, `DateTimeFormat.from(Locale)` does the
same for a date-time's field order and day-period designators, and every culture-sensitive
door also takes the `Locale` itself.
JVM-flavored fidelity, stated proudly: `Instant`, `LocalTime`, and `Duration` keep all nine
fractional digits, so nothing the core parses is truncated on the way out — full nanosecond
precision, end to end — and `Cast.decimal` lands in a `BigDecimal` built straight from the
core's exact sign, magnitude and scale.

## Numbers a workbook already holds

A spreadsheet stores a numeric cell as a `double`, and four doors read one directly, with no
text in between: `Cast.decimalFromDouble` (the shortest decimal that names the double, the
digits the file holds — `0.1` is one tenth, not what `new BigDecimal(0.1)` spells out),
`Cast.excelSerialFromDouble` (a `LocalDateTime` under a declared `ExcelEpoch`),
`Cast.excelTime` (a serial's fraction as a `LocalTime`) and `Cast.excelDuration` (days as a
`Duration`). A typed door's `Fault` has no span: its offset and length are 0.

## Validation constraints

Untrusted input is judged at two levels: first it is cast to the type (a `Fault` says it is not
the type), then the value is validated against a business rule. `Constraints` declares the rule
with the OpenAPI 3.2 Schema Object's keywords, built for one door (`Constraints.f64()`,
`Constraints.decimal()`, `Constraints.text()`, …), and the checked overload of that door judges
the value by it, in the core and in exact decimal arithmetic:

```java
import io.github.skunkwerkx.hypercast.*;
import java.math.BigDecimal;

// Latitude: a multiple of 0.000001 between -90 and 90, finer input rounded half to even.
Constraints<Double> latitude = Constraints.f64()
        .minimum("-90").maximum("90")
        .multipleOf("0.000001").rounding(Rounding.MIDPOINT)
        .build();
// Euro cash: a multiple of 0.05, and anything else is wrong.
Constraints<BigDecimal> euroCash = Constraints.decimal().multipleOf(new BigDecimal("0.05")).build();
// A sex code: one of three, whatever case and padding it arrived in.
Constraints<String> sex = Constraints.text().trim(true).enumOf("Female", "Male", "Other").build();

NumFormat euro = new NumFormat(',', '.', NumFormat.STYLE_ALL, "€");
System.out.println(Cast.f64("45.1234567", NumFormat.INVARIANT, latitude)); // Checked[value=45.123457, adjusted=true, …]
System.out.println(Cast.f64("-90.5", NumFormat.INVARIANT, latitude));      // Violation[keyword=minimum, value=-90.5, offset=0, length=5]
System.out.println(Cast.f64FromDouble(45.1234567, latitude));              // Checked[value=45.123457, adjusted=true, …]
System.out.println(Cast.decimal("€1.234,55", euro, euroCash));             // Checked[value=1234.55, adjusted=false, …]
System.out.println(Cast.decimal("€12,37", euro, euroCash));                // Violation[keyword=multipleOf, value=12.37, offset=0, length=6]
System.out.println(Cast.decimal("12,3x", euro, euroCash));                 // Fault[reason=MALFORMED, offset=4, length=1]

String message = switch (Cast.text(" MALE ", sex)) {
    case Checked<String> ok -> ok.value() + ", entry " + ok.enumIndex().getAsInt(); // Male, entry 1
    case Fault<String> f -> f.reason() + " at char " + f.offset();                  // EMPTY at char 0, for "   "
    case Violation<String> v -> switch (v.keyword().name()) {
        case "enum" -> "not a sex code: " + v.value();                                // for " Femal ": Femal
        default -> "breaks " + v.keyword(); // keywords are open
    };
};
System.out.println(message);
```

A checked door answers in a `CheckedVerdict<T>`, a sealed interface beside `Verdict<T>` (which
every plain door keeps returning, unchanged): `Checked<T>`, the `value`, `adjusted` (true only
when rounding changed it) and, at the text door with an `enum`, the matched `enumIndex`; or a
`Rejection<T>`, itself sealed over the plain doors' own `Fault<T>` (the input is not the type:
the very record, so a parse failure reads the same at either door) and a `Violation<T>` (it was
cast and the value broke a rule: its `keyword`, the `value` exactly as the door cast it, never
rounded, which at the text door is the trimmed input rather than an `enum` entry, and the
value's span, in the same units as a fault's). A switch over
the three records is exhaustive with no `default`. `Keyword` is a record with constants
(`Keyword.MINIMUM` … `Keyword.ENUM`), not an enum: `format` and `pattern` may join in a minor
release, so a branch over keywords needs its default arm (a `switch` on `name()`, the OpenAPI
spelling such as `exclusiveMinimum`, demands one).

The numeric doors take `minimum`, `exclusiveMinimum`, `maximum`, `exclusiveMaximum`,
`multipleOf` with a `Rounding` (`STRICT`, the default, then `MIDPOINT`, half to even, `FLOOR`
and `CEILING`) and `enumOf`, whose entries are decimals compared exactly to the value (`"7"` and
`"7.0"` are one value; at an integer door a fractional or out-of-range entry matches nothing).
The text door takes `minLength`, `maxLength` (in code points) and `enumOf`, with two extensions
of HyperCast's own: `trim` (Unicode `White_Space`, U+00A0 included) and `caseSensitive`. A text
`enum` matches under Unicode simple case folding unless `caseSensitive(true)` says byte for byte
(for a vocabulary whose case carries meaning, `mm` and `Mm`), and a match delivers the caller's
own declared string and its index, the first entry that matches; `enumOf(BigDecimal...)` there
declares numbers, valid entries no string equals. The four bounds are four
independent keywords, as in OpenAPI: `minimum("0").exclusiveMinimum("0")` declares both, and a
value is held to each. Every builder offers every keyword, and as JSON Schema does, a door
ignores an OpenAPI keyword for another type (`minLength` at a number, `minimum` at text). The
first broken rule wins: the door casts, the text is trimmed, `multipleOf` rounds or is broken (a
rounding past what the type holds, `127` rounded up to a multiple of 5 at `i8`, breaks
`multipleOf`), then `minimum`, `exclusiveMinimum`, `maximum`, `exclusiveMaximum`, the lengths
and `enum` judge the result. A float door also
judges the float it delivers against each bound read as the door reads it: `"1e-400"` at
`Constraints.f64()` reads as `0`, so it breaks `.exclusiveMinimum("0")`. Bounds and steps are a
`BigDecimal` or its text in invariant notation, both converted exactly by the core (`"1e-6"` is
`0.000001`, and `new BigDecimal("1E-29")`, past the 28 places a core decimal holds, is refused
rather than rounded), never a `double`, so `0.1` cannot be declared inexactly. Every numeric door
has its checked overloads over a `String`, UTF-8 `byte[]` and `MemorySegment` with a `NumFormat`,
and `i32FromDouble`, `f64FromDouble`, `decimalFromDouble` and the rest for a number a source
already decoded, judged on the shortest decimal that names it (`0.1 + 0.2` breaks
`multipleOf("0.1")` under `STRICT`, and rounds to `0.3` under `MIDPOINT`), with an empty span.

A declaration is judged once, by the core, when it is built. Every declaration OpenAPI allows is
accepted, those no value can meet included: crossed or equal bounds, `minLength` above
`maxLength`, duplicate entries (the first match wins), and `enumOf(new String[0])`, an `enum`
with no entries, which every value then violates (leaving `enumOf` out declares no `enum`). A
declaration reads back through methods of its keyword names, and `enumValues()` is the `enum` as
passed to `enumOf`, in order, duplicates kept (empty when none is declared, an empty list for
`enumOf(new String[0])`). `door()` names the door the declaration is bound to, as the factory that
built it (`"u8"`, `"text"`): `Constraints<Integer>` alone is `i32`'s, `u8`'s or `u16`'s. What
the core cannot read is the caller's bug: `build()` throws `IllegalArgumentException`, its
message the rule. That is `trim` or `caseSensitive` at a numeric door, or a rounding with
`multipleOf` at the text door (code 1); a `multipleOf` not positive (5); a rounding without
`multipleOf` (6); a bound, step or entry no door can hold, or a rounding with a fractional
`multipleOf` at an integer door (8: `Constraints.decimal().multipleOf("1e-29").build()` throws
"Constraints refused (code 8): a bound, step or enum entry the door cannot hold."); and an entry
that is not UTF-8 (9). So does handing a declaration to another door: `u8`, `u16` and `i32` all
deliver an `Integer`, so the compiler cannot tell their `Constraints<Integer>` apart, and the
door does. After that a checked cast judges only data and throws nothing for it. `Cast.text` without constraints is the plain text door, a
`Verdict<String>`: the input itself when it is valid text. Through a `String`, a lone surrogate
is `MALFORMED` at its char at every door, plain or checked, never replaced (Java's own encoder
would make it `?`), and in an `enum` entry it is refused as an entry that is not UTF-8. On the
JNI path the `enum` list lives in a direct buffer the declaration holds, so a checked call
neither copies nor pins it. `Cast.optional` takes a checked verdict as it takes a plain one: an
`EMPTY` fault is `Optional.empty()`, and every other rejection, a `Violation` included, comes
back inside the `Optional`.

## One character

`Cast.character` (`char` being a keyword) reads one character: the input verbatim when it
is exactly one character, `" "` and `"6"` included, or a declared code point (`65`,
`U+00E9`, `0x41`, `&H41`, `&#233;`, `&#xE9;`). It returns a `Verdict<Character>`, and a
`char` holds only the Basic Multilingual Plane, so a supplementary character the core
accepts (`😀`, `U+1F600`) is `OUT_OF_RANGE` here, spanning the input less its white
space edges, not split into half a surrogate pair. That is the one verdict where this
binding departs from the core's by design. A `String` of exactly one UTF-16 code unit comes
back as that `char` without crossing into the core, a lone surrogate included.

## NumFormat: declared, never guessed

White space is Unicode's `White_Space`, the 25 code points (ASCII, U+0085, U+00A0, U+2000 to
U+200A, U+202F, U+3000 among them). Every door ignores it at both ends (`Cast.character` given
exactly one character keeps it), and the numeric doors also after a leading sign, before a
trailing sign or a percent symbol, on either side of a currency symbol and inside accounting
parentheses (`"- 12"`, `"( 12 )"`, `"12,5 %"`, `"1 234,56 €"`, the last two under a declared comma decimal separator, the last with a space group separator and `€` declared); between digits it is
`MALFORMED`, unless a space-like group separator is declared. An ASCII space, U+00A0 or U+202F
declared as the group separator makes the input's choice of those three immaterial (Excel
writes U+00A0, CLDR's `fr` U+202F, an older system a space), a space-like separator with no
digit after it is just white space, and any other separator stays exact. Input of nothing
but white space is `EMPTY`.

Every integer, real and decimal door takes a `NumFormat`: the two separators, the `STYLE_*`
lenience flags, a currency symbol, and everything else a culture spells differently — the
separators a currency amount is written with, further spellings of the minus and plus signs
and the percent symbol, and a block of native digits. One rule governs all of those: the
ASCII default is *always* read, and a declared symbol is read beside it. `-5` parses under
every format, `−5` (U+2212) too once that minus is declared, and nothing that parsed under
a sparser format stops parsing under a fuller one. `NumFormat.INVARIANT` is `.`/`,` with
every default lenience on, Western grouping (below) and nothing else declared; `NumFormat.from(Locale)` fills every
field from the locale's own `DecimalFormatSymbols`, so a US caller gets `$`, a German one
`€`, an Estonian one its U+2212 minus and an Egyptian one `٠`–`٩`, `٫`, `٬` and `٪`:

```java
import java.util.Locale;

NumFormat us = NumFormat.from(Locale.US);
Cast.f64("$1,234.50", us); // 1234.5
Cast.i32("($5)", us); // -5: parentheses wrap symbol and digits
Cast.decimal("1.234,50 €", NumFormat.from(Locale.GERMANY)); // 1234.5 — scale 1, exact
Cast.i32("−5", Locale.forLanguageTag("et-EE")); // -5: U+2212, and ASCII '-' still reads
Cast.f64("١٬٢٣٤٫٥", Locale.forLanguageTag("ar-EG")); // 1234.5: Arabic-Indic digits and separators
Cast.f64("CHF 1\u202F234.50", Locale.forLanguageTag("fr-CH")); // 1234.5: fr-CH currency takes a decimal point
```

Or hand the door the `Locale` itself: every numeric door, in every input form, has a
`Locale` overload that is exactly that door under `NumFormat.from(locale)`, the shape
`NumberFormat.getInstance(Locale)` already gives Java code:

```java
import java.util.Locale;

Cast.f64("1.234,5", Locale.GERMANY); // 1234.5
Cast.i32("1\u202F234", Locale.FRANCE); // 1234 — fr-FR groups with U+202F
Cast.f64("1234.5", Locale.getDefault(Locale.Category.FORMAT)); // the default, said out loud
```

A `null` locale is a `NullPointerException`, never a silent default: pass
`Locale.getDefault(Locale.Category.FORMAT)` to mean the JVM's. Deriving a format reads the
locale's `DecimalFormat` and `DecimalFormatSymbols`, which build new objects each time, so
`NumFormat.from` keeps each locale's format once derived (a bounded cache of 64) and the
overloads cost what the `NumFormat` doors do: in a run for 0.9.0, a grouped `Cast.i32`
measured 45.6 ± 5.5 ns through `Locale.US` against 50.2 ± 11.8 ns through
`NumFormat.INVARIANT` — the lookup is
inside the noise — where deriving the format on every call, every field of it, measured
1199.0 ± 169.4 ns and 3.0 KB (JMH, linux-x64, Temurin 25).

The currency symbol is matched whole, once, at either edge of the numeric body — leading
(`$5`, `-$5`, `$ -5`) or trailing (`5 €`, `1.234,50 kr.`) with optional white space between
it and the digits — and only while `STYLE_CURRENCY` (part of `STYLE_ALL`) is set: declared
without it, the symbol is the `MALFORMED` span. With the symbol in the text, the digits are
read with the currency separators where the format declares them — Swiss French writes a
decimal comma in numbers and a decimal point in currency, Austrian German groups numbers
with a no-break space and currency with a dot, and `from(Locale)` fills them from
`getMonetaryDecimalSeparator()` and `getMonetaryGroupingSeparator()` where they differ from
the number separators; under `STYLE_SEPARATOR_DETECT` detection governs the whole token and
they are ignored. The three-argument constructors declare no symbol.

Declare a format outright with the `String` constructors and the `with*` methods: a
separator is 1 to 8 UTF-8 bytes with no ASCII digit, so `new NumFormat(".", ", ",
NumFormat.STYLE_ALL)` takes a comma-and-space group separator, and the `char` constructors
remain for the one-character case — and
`NumFormat.INVARIANT.withSigns("−", "").withPercentSymbol("٪").withDigitZero(0x0660)`
declares the rest. Native digits are the ten code points from a declared zero, the way
`DecimalFormatSymbols.getZeroDigit()` names a locale's digits, read beside ASCII
everywhere digits are — integer part, fraction, exponent — and never after a radix prefix.
`STYLE_TRAILING_SIGN` reads a sign after the digits (`5-`, `5 -`, `5 €-`, `2.5%-`), the way a
culture whose negative pattern is `n-` writes it; it is deliberately not in `STYLE_ALL`,
and `from(Locale)` sets it exactly when the locale's `DecimalFormat` writes a negative with
a suffix and no prefix. No locale in the JDK's own CLDR data does, so
`NumFormat.from(DecimalFormat)` — the same bridge, over a format you shaped — is how a
suffix-negative format is declared from pattern data. Every rule the core enforces is
enforced at construction and is a caller bug (`IllegalArgumentException`), never a
verdict: separators of 1 to 8 bytes that differ (the pair in force for a currency amount
too), signs, the percent symbol and both decimal separators with no white space (any of
the 25 Unicode `White_Space` code points; only a group separator may be or contain one), no
ASCII digit in any symbol, a currency symbol of at most 16 bytes with no white space at
either end (inside is fine: CLDR's `F CFA`), a zero that starts a decimal digit block.

### Grouping: Western by default

`STYLE_GROUPING` lets a group separator in; the format's `Grouping` says how the groups are
judged, declared apart from the styles (a style mask never carries a policy). The default,
for every constructor and for `NumFormat.INVARIANT`, is `Grouping.WESTERN`: groups of exactly
three digits counted leftward from the decimal separator, the leftmost one to three, so
`1,234,567` reads and `1,23,456` is `MALFORMED` (an invalid group spans its separator and its
digits, `,23`). `Grouping.INDIAN`, only when declared, is three digits beside the decimal
separator, two to their left, the leftmost one or two (`1,23,456`, `12,34,567`), and refuses `1,234,567`.
Ungrouped numerals are valid under both; `12,34` and `1234,567` are `MALFORMED` under both, and a first group can't start with a zero (`0,125` is `MALFORMED`, spanning
`0`);
there is no grouping after the decimal separator or in an exponent; and the rules count digits,
native digits included, and apply to currency separators and to the interchangeable space-like
separators. With `STYLE_GROUPING` off a separator is forbidden under both. A shape neither policy reads is `MALFORMED` under a declared format, `INVARIANT` or a bridge, never misread, while `NumFormat.DETECT` infers by the delimiter rule, so `1,2345` is 1.2345 there: [docs/grouping.md](https://github.com/SkunkWerkx/HyperCast/blob/master/docs/grouping.md#parked).

```java
Cast.i32("1,23,456", NumFormat.INVARIANT); // Fault: MALFORMED at (1, 3)
Cast.i32("1,23,456", NumFormat.INVARIANT.withGrouping(Grouping.INDIAN)); // 123456
new NumFormat(".", ",", NumFormat.STYLE_ALL, Grouping.INDIAN); // the policy beside the styles
```

`withGrouping` replaces the policy (never joins it with the old one) and `withStyles` and the
other `with*` methods keep it. `NumFormat.grouping()` answers it, `styles()` the `STYLE_*`
bits alone, and `flags()` the ABI word, the styles and `INDIAN`'s 512. Bits that are
no style are `IllegalArgumentException` at construction, as 256 and 768 are for
`Grouping.of`: bit 8 is retired permanently and never reused.

**One deviation from the record copy rule.** Copying a record through its canonical constructor
(`new NumFormat(f.decimalSeparator(), …, f.styles(), …)`, a deconstruct-and-rebuild, a
serialization library) is exact for every component but the policy: `NumFormat` keeps 1.0's ten
components, so that the ten-binder record pattern still compiles, and `styles()` answers the
`STYLE_*` bits alone. A copy of a Western format equals it; a copy of an Indian format
comes out Western. Copy with
`withGrouping(original.grouping())` or the `with*` methods, of which every component has one
(`withSeparators` sets the pair). The policy sits in bits 16 and 17 of the canonical constructor's
`styles` component (0 Western, 2 Indian; 1 and 3 are refused): an internal encoding that
callers must not set.

**What the bridge declares.** `NumFormat.from(Locale)` follows the platform's number pattern,
`NumberFormat.getInstance(locale)` (never the locale's name, never the input). The JDK has only
threes to give, so the bridge declares `WESTERN` on every JDK measured (21 and 25), Indian locales
included. `Cast.i64("12,34,567", Locale.forLanguageTag("hi-IN"))` is a `MALFORMED` fault, as is every
`Locale` door's reading of that shape, until you declare the policy:
`Cast.i64("12,34,567", NumFormat.from(locale).withGrouping(Grouping.INDIAN))`. `STYLE_GROUPING` is
cleared only when neither that pattern nor `getCurrencyInstance(locale)`'s groups (`en-US-POSIX`);
when just one does not, the flag stays for the other and ungrouped values stay valid, since a
format cannot prohibit grouping of currency amounts alone. The rule is deterministic: the same
patterns give the same format. `from(DecimalFormat)` applies it to the one pattern it is given,
and `from(DecimalFormat number, DecimalFormat currency)` to a pair.

The JDK's `DecimalFormat` keeps one grouping size, the last interval (no secondary size), so its
own pattern for an Indian locale reads `#,##0.###` and the JDK writes `12,345,678` for it: Western
is right for what the platform produces. `new DecimalFormat("#,##,##0.###")` has already dropped the
secondary size when `from(DecimalFormat)` reads it, so that bridge is Western too. Locale data comes
from the JDK's own CLDR tables, never ICU. On JDK 25
`tok`, `tok-001` and `tok-Latn-001` write groups of two and declare `WESTERN`, which refuses them: their ungrouped numerals read, their grouped ones don't.

The boolean door's lexicon stays English (`true`/`yes`/`on`/`1` and the rest of the twenty),
by decision: no platform carries culture data for yes/no words, so no bridge could fill one.
A caller maps its own words before the door.

`Cast.decimal` is the exact door, and a canonical one. No `double` is formed on the way:
`0.1` is one tenth and `50%` is exactly `0.5`. Exact trailing zeros in the fraction are
trimmed, so the scale is minimal — `1.10`, `1.1` and `1.1000` all have a scale of 1, `100`
stays `100` (integer zeros are never touched), and zero is scale 0, never negative. Nothing
but a zero is ever dropped: precision past 2^96−1 or 28 places is `OUT_OF_RANGE`, never
rounded.

## DetailedCast: what failed

A `Fault` carries a reason and a span. `DetailedCast` is every door of `Cast`, with the same
names, shapes and input forms, answering a `DetailedVerdict` (or, with `Constraints`, a
`DetailedCheckedVerdict`) whose failure also carries the core's `FaultDetail`: a small shared
vocabulary of what failed (`digit_expected`, `group_size`, `too_large`, `not_finite`,
`invalid_utf8` …), the same names and numbers in every binding, for people diagnosing rejected
data and for programs that want to branch on it.

```java
String why = switch (DetailedCast.i32("12,34", NumFormat.INVARIANT)) {
    case DetailedSuccess<Integer> s -> "got " + s.value();
    case DetailedFault<Integer> f -> f.reason() + " " + f.detail() + " at char " + f.offset();
}; // MALFORMED group_size at char 2
```

It is a separate class with its own result types because `Verdict` and `Rejection` are sealed:
a detail on `Fault`, or a third case, would break the exhaustive switches written against them.
`Verdict`, `Fault`, `Success`, `CheckedVerdict`, `Rejection`, `Checked` and `Violation` carry no
detail, and a detailed failure is the same reason and span: `DetailedFault.toFault()` is the `Fault` the
plain door returns. A violation has no detail (nothing about the text was wrong), only its
keyword, value and span. `FaultDetail` is a record with constants, as `Keyword` is: a published code keeps its
number and meaning, new codes may arrive in a minor release, and a code this binding does not
name is a `FaultDetail` that keeps it (`isKnown()` is false, `name()` is `detail 999`) beside a
reason and span that stay usable.

### Date options and month names

The date doors keep today's shape rules unless `DetailedCast` is handed `DateOptions` (this
deliberately differs from white-space tolerance, which every door took: strict RFC 3339 callers
may rely on a space separator or `+00` being refused). `DateOptions.COMPACT_DATE` reads
`20260107` at `date`, `SPACE_SEPARATOR` a space for `T` and `HOUR_OFFSET` `+00` or `-05` at
`timestamp`; `DateOptions.COMMON_FORMS` is all three, safe at every door (an option that means
nothing to a door is ignored there). The pivot, 100 to 9999, is the last year of the 100-year
window a two-digit year reads in, with no default. A `MonthNames` vocabulary, built by the
caller (the platform builds none: aliases and grammatical forms are your decisions), lets a
month field be a declared name in the position the order selects, Excel style only:

```java
MonthNames english = MonthNames.builder().add(1, "Jan") /* … */.add(12, "Dec").build();
DetailedCast.date("07-Jan-26", DateOrder.DAY_MONTH_YEAR, new DateOptions(0, 2029), english); // 2026-01-07
DetailedCast.date("20260107", DateOptions.COMMON_FORMS); // 2026-01-07
```

Names fold case, punctuation is part of a name (`janv.` with `.` as the separator reads
`7.janv..2026`, and `7.janv.2026` is `MALFORMED`), and a vocabulary the core would refuse (a
month with no name, a name with a digit, `/`, `-` or white space, an empty or over-64-byte name,
two names that fold to one spelling) is an `IllegalArgumentException` when built.

## DateTimeFormat: the date order and the day periods

`Cast.dateTime` reads `1/7/2026 3:04 PM` under a declared `DateOrder`, and the English
`AM`/`PM` are always read, ASCII case-insensitively, after the clock or ahead of it
(`PM 3:04`), with any run of white space where one optional space stood, so the
`3:04\u202fPM` that JDK 20 and later write for `Locale.US` reads. A culture's own designators
are declared beside the order in a
`DateTimeFormat` — `{order, amDesignator, pmDesignator}`, `NumFormat`'s parallel for the one
temporal door that reads culture-sensitive text — and matched exactly, in either position,
with the English pair still read beside them. `DateTimeFormat.from(Locale)` fills it from
the locale's short-date pattern and `DateFormatSymbols.getAmPmStrings()`, the door takes
the `Locale` itself, and the `DateOrder` overloads declare no designators.

```java
import io.github.skunkwerkx.hypercast.DateOrder;
import io.github.skunkwerkx.hypercast.DateTimeFormat;
import java.util.Locale;

Cast.dateTime("2026-01-07 오후 3:04", DateTimeFormat.from(Locale.KOREA)); // 15:04
Cast.dateTime("2026-01-07 오후 3:04", Locale.KOREA); // the same, as a Locale
Cast.dateTime(
        "7.1.2026 3:04 nachm.",
        new DateTimeFormat(DateOrder.DAY_MONTH_YEAR, "vorm.", "nachm.")); // 15:04, January 7th
Cast.dateTime("2026-01-07 오후 3:04", DateOrder.YEAR_MONTH_DAY); // MALFORMED at 오: nothing declared
```

A designator is 1 to 16 UTF-8 bytes with no ASCII digit and no white space at either
end (a space inside, as in Spanish's `a. m.`, is fine); anything else is an
`IllegalArgumentException` at construction. `from(Locale)` leaves the English pair
undeclared, since the core reads it anyway, and so leaves a designator the layout cannot
hold — the JDK's data has over-16-byte ones for dozens of locales (78 of 1,158 on Temurin
25.0.4.1) — rather than refusing the locale, so those locales read the English pair only. Derived formats are cached per locale as `NumFormat.from`'s are: in a run for 0.9.0
(separate from the Benchmarks table), `Cast.dateTime` on `1/7/2026 3:04 PM` measured 74.1 ± 17.2 ns through `Locale.US` against 75.5 ± 14.4 ns under
the `DateOrder` alone, and 97.5 ± 7.4 ns with Spanish designators declared and matched — the
40-byte layout crossing where NULL did, and a longer designator read.

## Gating on the core, and what a span counts in

Nothing loads until the first door is called. A consumer with a managed fallback gates on
`Cast.isAvailable()` first: probed once, cached, never throws — `false` when the platform
library will not open, the jar carries no core for this OS/arch, or an older core lacks an
export this binding was built against. That last guard is what keeps a 1.0 core from reading
a numeral by 1.0's rule: a 1.0 library has no detailed exports (and a `libhypercast_jni` built
from a core without them registers 33 natives of the 37), so its load fails and the policy bits it would ignore are never sent. The doors do
not fall back. The first one to need a core that failed to load throws
`ExceptionInInitializerError`, and every later one `NoClassDefFoundError`; either way the
original failure is in its cause chain.
`Cast.nativeVersion()` reports the loaded core's `major.minor.patch`, and succeeds exactly
when `isAvailable()` is `true` — the pair that proves the library that resolved is the one
this jar was built against.

A `null` argument is a `NullPointerException` naming the parameter on both paths, before any
native call. The two paths differ in two ways only: the load failure's message names the
path's own library (`libhypercast_jni` on JNI), and on JNI an input above 64 KiB is copied
into native memory, which can throw `OutOfMemoryError` when native memory is short (up to 64
KiB the array is pinned for the core call alone, so a parse never holds off the garbage
collector for long). Each JVM (and class loader) extracts its own temporary copy of the
library; on Windows a loaded DLL cannot be deleted, so `hypercast*.dll` copies accumulate in
`%TEMP%`. A read-only or `noexec` temporary directory makes `isAvailable()` `false`, with the
loader's own exception in the cause chain.

A `Fault`'s `offset`/`length` count in the input's own unit. Through a `byte[]` or
`MemorySegment` door they are the core's byte span into the UTF-8, verbatim. Through a
`String` door they are UTF-16 code units — the byte span rebased, so
`text.substring(f.offset(), f.offset() + f.length())` is the offending text even when the
input is not ASCII: `Cast.i32("1€", …)` faults at `(1, 1)` as a `String` and `(1, 3)` as
bytes. ASCII input is identical either way and is never touched. The spans are `int`s, so a `MemorySegment`
longer than `Integer.MAX_VALUE` bytes is `OUT_OF_RANGE` (`TOO_LONG`) at every door, spanning `(0,
Integer.MAX_VALUE)`, on every JDK, and none of it is read.

## Native access

Loading native code is a *restricted* operation: the JDK wants the application, not a library
on its classpath, to say that native code may run. Without that the first cast still works.
On JDK 21 to 23 nothing is printed without the grant. From 24, a JVM without it prints a
four-line warning naming the method that loads native code — `java.lang.System::load` on 24,
where the JNI path loads the library, and `java.lang.foreign.SymbolLookup::libraryLookup` on
25 and later — ending "Restricted methods will be blocked in a future release unless native
access is enabled". Grant it where the JVM is launched:

```sh
java --enable-native-access=ALL-UNNAMED -cp app.jar:hypercast.jar com.example.Main        # on the classpath
java --enable-native-access=io.github.skunkwerkx.hypercast -p mods -m com.example.app     # on the module path
```

The jar's manifest carries `Automatic-Module-Name: io.github.skunkwerkx.hypercast`, so the
module-path form has a stable name to grant rather than one derived from the jar's file
name. An executable jar can make the same grant in its own manifest
(`Enable-Native-Access: ALL-UNNAMED`). Under `--illegal-native-access=deny` with no grant
the library cannot be loaded at all: `isAvailable()` is `false` and the doors throw.

### Shaded and fat jars

Keep `Multi-Release: true` (maven-shade's `ManifestResourceTransformer`, Shadow's
`manifest`). Without it JDK 25 reads only the base and runs the JNI path: correct, slower,
and its warning names `System::load`.

### The `MemorySegment` doors on JDK 21

`java.lang.foreign` is a preview API in JDK 21, so `javac --release 21` refuses code that
names `MemorySegment` unless it is compiled with `--enable-preview` (and run with it). The
`String` and `byte[]` doors need nothing; code on 21 that never names a `MemorySegment`
compiles and runs as on any other JDK. On the JNI path a segment's bytes are copied to the
heap before they cross. The same holds for the `interop` package below, whose readers take
segments.

## Why not `Integer.parseInt` / `Instant.parse` / the formatter zoo?

1. **Verdicts, not exceptions** — `NumberFormatException`-driven control flow costs a
   throw+fill-in-stack-trace on every bad input; a `Fault` is two ints and a reason, and
   bad data is the *expected* case when the text is untrusted.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, radix prefixes, all five .NET `Guid` text forms, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (the whole suite green on JDK 21 and 25, full corpus replay
   through real JNI calls and real FFM downcalls, with byte-exact fault spans).
4. **Faster where it matters, and the input is never copied.** JMH, `-prof gc` for the
   allocation column, the profile `./gradlew :benchmarks:jmh` runs: 1 fork, 3 warmup + 5
   measurement iterations of one second (linux-x64 on an Intel Core i9-11900H, Temurin 25.0.4).
   The error bars are that short profile's; the verdicts are far outside them. Every row is
   the FFM path on JDK 25; the JNI path on 21 has no published figures
   (`-PjmhJdk=21` measures it).

   | Door | HyperCast | JDK | Verdict |
   | --- | ---: | ---: | --- |
   | `Cast.time` vs `LocalTime.parse` | 29.6 ± 1.7 ns | 371.6 ± 25.5 ns | **12.6x faster** |
   | `Cast.timestamp` vs `Instant.parse` | 50.4 ± 6.3 ns | 553.8 ± 53.3 ns | **11.0x faster** |
   | `Cast.dateTime` vs `LocalDateTime.parse` (ISO) | 43.4 ± 2.8 ns | 552.6 ± 59.2 ns | **12.7x faster** |
   | `Cast.dateTime` vs a `M/d/yyyy h:mm a` formatter | 40.5 ± 2.0 ns | 348.9 ± 7.6 ns | **8.6x faster** |
   | `Cast.duration` vs `Duration.parse` | 33.8 ± 4.4 ns | 334.7 ± 10.8 ns | **9.9x faster** |
   | `Cast.date` (declared order) vs a `M/d/yyyy` formatter | 23.7 ± 1.0 ns | 152.5 ± 7.8 ns | **6.4x faster** |
   | `Cast.f64` (eurozone) vs `NumberFormat` (de-DE) | 42.6 ± 4.9 ns | 146.7 ± 13.3 ns | **3.4x faster** |
   | `Cast.i32` (grouped) vs `NumberFormat` | 36.1 ± 2.8 ns | 74.9 ± 4.3 ns | **2.1x faster** |
   | `Cast.f64` vs `Double.parseDouble` | 38.1 ± 2.9 ns | 40.4 ± 2.4 ns | wash |
   | `Cast.uuid` vs `UUID.fromString` | 29.2 ± 1.4 ns | 23.0 ± 1.5 ns | 1.3x slower — see below |
   | `Cast.decimal` vs `new BigDecimal(String)` | 46.7 ± 2.6 ns | 20.3 ± 1.1 ns | 2.3x slower — see below |
   | `Cast.bool` vs `Boolean.parseBoolean` | 13.1 ± 2.0 ns | 0.50 ns | honest loss — see below |

   The `String` rows above include the UTF-8 encode. A caller already holding bytes skips
   it, and the raw crossing is what a caller casting out of its own buffers pays per value:

   | Door (UTF-8 in hand) | `byte[]` | `MemorySegment` slice | allocation |
   | --- | ---: | ---: | ---: |
   | `Cast.timestamp` | 41.8 ± 1.0 ns | 38.5 ± 2.7 ns | 40 B (the `Instant` + record) |
   | `Cast.i32` (grouped) | 32.7 ± 2.9 ns | 27.1 ± 0.7 ns | 32 B (the `Integer` + record) |
   | `Cast.uuid` | 26.4 ± 1.3 ns | — | 48 B (the `UUID` + record) |

   Separator detection costs what the core says it costs: `NumFormat.DETECT` on
   `1.234.567,89` measures 49.8 ± 2.7 ns against 42.6 ± 4.9 ns declared — the structural
   resolution pass, visible because the carrier around it is thin. The `Locale` overload
   costs its cached format lookup and the currency symbol the locale declares: `Cast.i32` on
   `1,234,567` with `Locale.US` measures 45.6 ± 5.5 ns against 50.2 ± 11.8 ns under
   `NumFormat.INVARIANT` in a separate run for 0.9.0 —
   inside the noise. Rejection
   is where the verdict pays off: `Cast.i32` returns the `Fault` for `1,234x567` in
   60.3 ± 3.6 ns and 128 B, while `Integer.parseInt("1234x567")` takes 984 ± 77 ns and 888 B
   to throw its `NumberFormatException` and have it caught, 16x slower. The
   `DateTimeFormatter.ISO_OFFSET_DATE_TIME` control measures 715.5 ± 43.6 ns for the text
   the timestamp door reads in 52.5.

   **The lone-surrogate fix, measured (2026-11-02, Apple M6, macOS 27.2, Temurin 25.0.5, the
   same profile).** The `String` doors used to encode with `getBytes(UTF_8)`, which turns a
   lone surrogate into `?`, a guess a door could read as data. They now send the WTF-8 the
   checked doors send, keeping `getBytes`'s intrinsic encode whenever its bytes hold no `?`
   (so nothing was replaced) and encoding by hand otherwise. A first cut that always encoded
   by hand cost up to 13 ns a call; the kept fast path recovers most of it, not all. One
   machine, interleaved runs; the linux-x64 table above predates the change and was not
   re-measured there:

   | Door (`String`) | before (`getBytes`) | WTF-8 always | now | now vs before |
   | --- | ---: | ---: | ---: | ---: |
   | `Cast.uuid` | 12.3 ± 0.1 ns | 25.7 ± 0.2 ns | 16.5 ± 0.0 ns | **+4.2 ns, a loss** |
   | `Cast.timestamp` | 21.2 ± 1.3 ns | 30.7 ± 0.2 ns | 26.2 ± 0.7 ns | **+5.0 ns, a loss** |
   | `Cast.dateTime` (ISO) | 22.0 ± 0.2 ns | 33.4 ± 6.7 ns | 22.3 ± 0.3 ns | level |
   | `Cast.dateTime` (`1/7/2026 3:04 PM`) | 21.0 ± 0.2 ns | 27.9 ± 0.8 ns | 21.5 ± 1.8 ns | level |
   | `Cast.time` | 13.1 ± 0.1 ns | 20.5 ± 0.8 ns | 13.3 ± 0.1 ns | level |
   | `Cast.duration` | 18.0 ± 0.9 ns | 24.0 ± 2.7 ns | 17.3 ± 1.1 ns | level |
   | `Cast.date` (declared order) | 10.2 ± 0.4 ns | 14.0 ± 0.2 ns | 10.9 ± 0.1 ns | +0.6 ns |
   | `Cast.i32` (grouped) | 18.4 ± 3.3 ns | 22.4 ± 4.4 ns | 19.2 ± 4.8 ns | level |
   | `Cast.f64` | 15.2 ± 0.1 ns | 19.8 ± 0.3 ns | 15.2 ± 0.1 ns | level |
   | `Cast.decimal` | 20.3 ± 0.1 ns | 23.8 ± 0.3 ns | 20.7 ± 1.1 ns | level |
   | `Cast.bool` | 6.1 ± 0.4 ns | 8.0 ± 0.1 ns | 6.4 ± 0.3 ns | +0.3 ns |

   What is left is the look for `?`, about a cycle a byte, so it shows on the longest texts
   (the 36-character UUID, the 30-character timestamp); an eight-bytes-a-step look measured no
   better and was not kept. The `byte[]` and `MemorySegment` doors encode nothing and are
   unchanged.

**What keeps the crossing thin.** No door opens an arena per call: one `ThreadLocal` holds
the out/fault/format segments for the life of the thread. Every downcall is linked
`Linker.Option.critical(true)`, so the caller's own `byte[]` crosses as a pinned heap
segment, never copied into a native staging buffer. Sound because every door is a short, non-blocking
parse over those bytes that never calls back into Java — the profile that option exists
for — and `reachability-metadata.json` registers the option, so the GraalVM Native Image
smoke test proves it under AOT too. Every text door also has a `MemorySegment` overload:
slice one buffer holding many values (a mapped file, a direct buffer, one line of a CSV) and
cast a value out of it with nothing copied. The UUID door reads its sixteen bytes as two
big-endian longs rather than one byte at a time.

**The honest trade-off:** three rows lose, each to a JDK method with no boundary to cross
and one shape to read. `Boolean.parseBoolean` is unbeatable by construction: JIT folds a
loop-invariant `parseBoolean` into nothing, which an FFM downcall structurally can't match
— the twenty-lexeme vocabulary is why anyone calls this door. `UUID.fromString` is pure
bit-twiddling over one text form; what this door adds for its six nanoseconds is the
N/B/P/X forms and `urn:uuid:` prefixes that method doesn't accept. `new BigDecimal(String)`
reads plain digits and throws on anything else; `Cast.decimal` reads the same grouping,
currency and parentheses every other numeric door does and returns a verdict. It's also a
native dependency: for plain invariant integers, `Integer.parseInt` is the reasonable
choice.

## AOT

The GraalVM Native Image smoke test (`./gradlew :aot-smoke-test:nativeRun`) builds and
runs the `isAvailable()`/`nativeVersion()` probe, twenty-six doors, every one but the plain text door, whose checked form runs with the
constraints (the text doors
through their `String` form, the four typed doors through a `double`; the date-time door
with designators declared too, and a format with signs and native digits declared), the `byte[]` and `MemorySegment` forms (heap slice and native segment)
once per ABI shape, and the exhaustive union switch as a true native binary. Native Image
needs two separate registrations
and the jar ships both in its `reachability-metadata.json` under
`META-INF/native-image/io.github.skunkwerkx/hypercast/`, so a consumer inherits them with no
configuration: the FFM downcall *signatures* (reachability is per-signature, not per-function
— the doors share six shapes, and the version probe's `() -> int` is the seventh),
and three `resources` globs, one per core library name (`native/*/libhypercast.so`,
`native/*/libhypercast.dylib`, `native/*/hypercast.dll`). Native Image is built on GraalVM
for JDK 25 and always takes the FFM path, so the JNI library is deliberately not embedded;
the smoke test fails if it is. GraalVM Community for JDK 21 is past end of life, so the JNI
path has no image row. A `native-image.properties` beside it has the
class holding the downcall handles initialized at image build time, which is what keeps the
doors compiled rather than interpreted in the image — 60-110 ns a door there (the table
below).

A native image still loads the core's shared library at startup, extracted from the jar as on
the JVM, rather than linking the static archive into the executable the way C# Native AOT and
Go do. Linking it in works on Linux, but only through GraalVM's internal builder API, with
separate linker handling for each OS, so it is deliberately not done; the forge's
[levers not pulled](https://github.com/SkunkWerkx/.github#levers-deliberately-not-pulled)
table has the full reasoning, the proven recipe, and what would change the answer.

`NumFormat.from(Locale)` and `DateTimeFormat.from(Locale)`, and so every `Locale` overload,
need the locale in the image. Native Image includes only the locale it was built in unless
told otherwise, and for any other locale `DecimalFormatSymbols` quietly returns the root
locale's `.` and `,`, so a German format built from `Locale.GERMANY` reads `1.234,5` as
`Malformed` in the image while the JVM reads it as 1234.5. Name every locale the program
declares formats from, e.g. `-H:IncludeLocales=de-DE,fr-FR` (or `-H:+IncludeAllLocales`). A
`NumFormat` or `DateTimeFormat` built from explicit symbols needs nothing.

The resources half has a failure mode worth knowing, because nothing catches it at build
time: Native Image doesn't embed classpath resources unless they are registered, so without
the globs `Cast`'s `getResourceAsStream("/native/{rid}/{lib}")` returns null and a consumer's
binary compiles clean, then dies on its first call with "classpath resource not found". The
smoke test declares no glob of its own, so it passes on the packaged metadata alone, which is
what proves a consumer is fine.

**Under GraalVM Native Image** JMH cannot run, so these are a hand loop (warm up, one
million calls per door, best of five rounds) built into a native image with the metadata the
jar ships, same machine, the same loop on the JVM beside it:

| Door | FFM downcall, JVM | FFM downcall, Native Image |
| --- | ---: | ---: |
| `bool` | 18 ns | 60 ns |
| `i32` (grouped) | 32 ns | 91 ns |
| `f64` | 41 ns | 106 ns |
| `timestamp` | 52 ns | 111 ns |

A native image runs the FFM doors at two to three times their JVM cost — the ordinary price
of an ahead-of-time compiler without a profile.

The FFM doors are compiled in the image, not interpreted, because the downcall handles are
constants there: one per ABI shape, created from the C signature alone in a class that needs
nothing from the library, taking the export's address as its first argument, with
`META-INF/native-image/.../native-image.properties` in the jar initializing that class at
image build time. A consumer's `native-image` build inherits it with no configuration.

## Verifying provenance

The published jar verifies from the release tag alone, and so do the sources and javadoc jars,
the `.pom` and their signatures: the release attestation, signed by this repository's
`release.yml` at the tag, names every file of the Central bundle among everything the release
ships, with the release inventory as its predicate.

```sh
curl -LO https://repo1.maven.org/maven2/io/github/skunkwerkx/hypercast/<version>/hypercast-<version>.jar
gh attestation verify hypercast-<version>.jar --repo SkunkWerkx/HyperCast \
  --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

Its build provenance is the second check. The bundle is assembled by `hyper-pack-maven.yml`,
a reusable workflow that lives in `SkunkWerkx/.github`, and that is the identity Fulcio
records as the signer, so `--repo` alone is not enough there: add `--signer-repo`, or use
`--owner` in place of both. The native libraries inside the jar carry their own, from the Forge's build workflows at the preparation commit on master, and the
pack job verifies each of them before it goes into the jar.

```sh
gh attestation verify hypercast-<version>.jar \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
# or: gh attestation verify hypercast-<version>.jar --owner SkunkWerkx
```

Get the signer wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which
reads like a bad signature but is only an identity mismatch. The root README's
[Provenance](../README.md#provenance) section has the whole picture: the inventory, which
artifacts are signed from where, and the gates every byte passes before publication.

## Interop: building on HyperCast's C ABI

For a library that carries HyperCast's verdicts across a C ABI of its own — HyperTabular
does — and so reads the core's out-values, numeric format, verdict codes and
fault spans out of its own buffers. The `io.github.skunkwerkx.hypercast.interop` package is
the code `Cast`'s FFM path itself uses, so a value read that way is the value the door would
have returned. It is FFM-typed on every JDK, so on 21 it needs preview enabled in the
caller's own code ([above](#the-memorysegment-doors-on-jdk-21)).

- `NativeValues` — every out-value reader, each taking a `MemorySegment` and a byte offset:
  `instant`, `date`, `civil`, `time`, `duration`, `decimal`, `uuid`; `writeFormat`, the
  92-byte size-tagged `NumFormat` layout every numeric door reads, and `writeDayPeriods`,
  the 40-byte designator layout the date-time door reads; `fault(code, offset, length)`;
  `version(packed)` for a `*_version()` word; and the `*_BYTES` constants, each layout's
  size and so the stride of an array of them (`FORMAT_BYTES` is 92, `DAY_PERIODS_BYTES` 40,
  `FAULT_BYTES` 8, `FAULT_DETAIL_BYTES` 16 for the detailed exports' size-tagged fault and
  `DATE_OPTIONS_BYTES` 12, and for the validation constraints `CONSTRAINTS_BYTES` 104, `CHECK_BYTES` 12
  and `SPAN_BYTES` 8); `writeDateOptions` and `detailedFault(code, segment, at)`, which reads
  a detailed fault and keeps a detail code it does not name. A format's flags word, with the
  grouping policy in bits 8 and 9, is `NumFormat.flags()`.
- `Constraints.toRaw()` and `packedEnum()` — a declaration in the core's 104-byte layout (one
  slot per bound) and its `enum` list as the core reads it beside that layout (numeric entries in this platform's byte
  order, text entries behind little-endian length prefixes), so a
  library judging values through `cast_checked` hands the core exactly what `Cast` does.
- `NativePlatform` — the platform table and the jar loader, parameterized by library base
  name: `current("hypertabular")` resolves this JVM's `Target` (RID and file name, or `null`
  when no build exists for the platform), `load(anchor, baseName, target)` copies
  `/native/{rid}/{file}` out of the anchor class's jar and opens it as a `SymbolLookup` for
  the life of the process, and `missing` names what was absent.
- `NumFormat` and `DateTimeFormat` validate every field by the core's own rules at
  construction, so a value that exists is one the core accepts: a library building either
  layout from one of them never sees the core's contract-violation code.
- `UnixPrecision`, `DateOrder`, `ExcelEpoch` and `CastFailure` each have a public `code()`
  and a `fromCode(int)` returning an `Optional` — empty for a code the enum does not name,
  so `CastFailure.fromCode(0)`, success, is empty.

## Building and testing

The jar carries two native libraries per platform, so build both before Gradle runs: `cargo
cdylib` in `rust/`, then `cargo build --release --locked --lib` in `java/jni/`. The JNI library
registers one native per C export, all 36, plus the count it reports to the loader. Every door
crosses through the four detailed exports, which alone take the grouping policy, date options
and a month vocabulary; the 1.0 natives stay declared, as every site holds the whole ABI. Gradle stages
them into the jar and refuses a stale one: the build fails when either library is older than
a source cargo read for it (its dep-info), or than a `Cargo.toml` cargo has not run since (an
edit that changes nothing cargo builds is current once cargo has looked), or when one is built
without the other, and the message gives the rebuild commands.

Gradle, Spotless, javadoc and publishing run on JDK 25. `./gradlew test` runs the suite
against the built jar on JDK 25 (the FFM path); `testFloor` runs the same suite on JDK 21
(the JNI path end to end, under `-Xcheck:jni`), `checkSuiteParity` fails unless both ran the
same tests with none skipped, and `checkFloorApi` fails if the base references any JDK class,
method or field that JDK 21 lacks: the base compiles `-source/-target 21` against JDK 25's
API rather than `--release 21`, because `--release 21` refuses `java.lang.foreign` as preview.
`./gradlew check` and `./gradlew build` run all of them, and they need a local JDK 21:
`gradle.properties` turns toolchain auto-download off, so pass it explicitly.

```sh
./gradlew check -Dorg.gradle.java.installations.paths=<path to a JDK 21> \
  -Dorg.gradle.java.installations.auto-detect=false
```

Without a JDK 21 Gradle can see, `testFloor` fails with "Cannot find a Java installation …
matching: {languageVersion=21 …}". `./gradlew :aot-smoke-test:runFloor` runs the smoke test
on JDK 21, and `./gradlew :benchmarks:jmh -PjmhJdk=21` the benchmarks.

The downgrade test, outside `check`, runs child JVMs with a real 1.0 core first on the class path
and shows that `Cast.isAvailable()` is `false` and a numeric request fails to load rather than
reading by 1.0's rule: against a build of tag `v1.0.0` (which lacks even the checked exports),
against a core with every 1.0-era export and none of the detailed ones (the root cause then names
`cast_detailed`, or on JNI the native-count mismatch), and a `libhypercast_jni` of a core without the
detailed exports:

```sh
./gradlew testDowngrade testDowngradeFloor -Phypercast.v1Core=<libhypercast of tag v1.0.0> \
  -Phypercast.preDetailedCore=<a libhypercast with the 1.0-era exports, no detailed ones> \
  -Phypercast.v1Jni=<a libhypercast_jni without the detailed natives> \
  -Dorg.gradle.java.installations.paths=<path to a JDK 21>
```

## Install

Published to [Maven Central](https://central.sonatype.com/artifact/io.github.skunkwerkx/hypercast)
— no extra repository configuration, since `mavenCentral()` is already in virtually every
Gradle/Maven build:

```kotlin
dependencies {
    implementation("io.github.skunkwerkx:hypercast:<version>")
}
```

The current version is the one on the Maven Central badge above. Requires JDK 21 or later.
The jar bundles a native build for all eight platforms (linux-x64, linux-arm64,
linux-musl-x64, linux-musl-arm64, osx-x64, osx-arm64, win-x64, win-arm64) and picks the
right one at runtime; see [Native access](#native-access) for the one flag the JVM wants.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
