# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Maven Central](https://img.shields.io/maven-central/v/io.github.skunkwerkx/hypercast.svg)](https://central.sonatype.com/artifact/io.github.skunkwerkx/hypercast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**Java's own discriminated union — a `sealed interface` over two records — carrying the
verdict of every cast: the value, or a closed reason plus the exact byte span that
offended. A two-arm switch with no default is proven exhaustive by `javac`; an unhandled
disposition is a compile failure.**

Allocation-lean scalar casts — booleans, characters, the full integer family, reals, exact
decimals, UUIDs, temporals — via `java.lang.foreign` (FFM) downcalls straight into the native `libhypercast` Rust core.
JDK 25 is the floor: the first long-term-support release with the final FFM API (JEP 454
finalized it in JDK 22, and 22 through 24 are past end of life), comfortably past the
Verdict union's own requirement — sealed interface + record patterns + exhaustive switch,
stable since 21. The jar bundles a native build for every supported platform
(Linux glibc, Linux musl, macOS, Windows × x64/arm64) under `/native/{rid}/` and picks the
right one at runtime, so a consumer adds one dependency and nothing else.

```java
String message = switch (Cast.i32("(1,234)", NumFormat.INVARIANT)) {
    case Success<Integer> s -> "got " + s.value();          // -1234, accounting negative
    case Fault<Integer> f -> f.reason() + " at char " + f.offset();
};  // no third case: javac checked
```

Door names mirror the native ABI (`i32`, `f64`, `timestamp`, …) so the polyglot surface
reads identically across bindings; every door also takes raw UTF-8 `byte[]` for callers
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

## One character

`Cast.character` (`char` being a keyword) reads one character: the input verbatim when it
is exactly one character, `" "` and `"6"` included, or a declared code point (`65`,
`U+00E9`, `0x41`, `&H41`, `&#233;`, `&#xE9;`). It returns a `Verdict<Character>`, and a
`char` holds only the Basic Multilingual Plane, so a supplementary character the core
accepts (`😀`, `U+1F600`) is `OUT_OF_RANGE` here, spanning the input less its ASCII
whitespace edges, not split into half a surrogate pair. That is the one verdict where this
binding departs from the core's by design. A `String` of exactly one UTF-16 code unit comes
back as that `char` without crossing into the core, a lone surrogate included.

## NumFormat: declared, never guessed

Every integer, real and decimal door takes a `NumFormat`: the two separators, the `STYLE_*`
lenience flags, a currency symbol, and everything else a culture spells differently — the
separators a currency amount is written with, further spellings of the minus and plus signs
and the percent symbol, and a block of native digits. One rule governs all of those: the
ASCII default is *always* read, and a declared symbol is read beside it. `-5` parses under
every format, `−5` (U+2212) too once that minus is declared, and nothing that parsed under
a sparser format stops parsing under a fuller one. `NumFormat.INVARIANT` is `.`/`,` with
every default lenience on and nothing else declared; `NumFormat.from(Locale)` fills every
field from the locale's own `DecimalFormatSymbols`, so a US caller gets `$`, a German one
`€`, an Estonian one its U+2212 minus and an Egyptian one `٠`–`٩`, `٫`, `٬` and `٪`:

```java
NumFormat us = NumFormat.from(Locale.US);
Cast.f64("$1,234.50", us);                                     // 1234.5
Cast.i32("($5)", us);                                          // -5: parentheses wrap symbol and digits
Cast.decimal("1.234,50 €", NumFormat.from(Locale.GERMANY));    // 1234.5 — scale 1, exact
Cast.i32("−5", Locale.forLanguageTag("et-EE"));                 // -5: U+2212, and ASCII '-' still reads
Cast.f64("١٬٢٣٤٫٥", Locale.forLanguageTag("ar-EG"));            // 1234.5: Arabic-Indic digits and separators
Cast.f64("CHF 1\u202F234.50", Locale.forLanguageTag("fr-CH"));  // 1234.5: fr-CH currency takes a decimal point
```

Or hand the door the `Locale` itself: every numeric door, in every input form, has a
`Locale` overload that is exactly that door under `NumFormat.from(locale)`, the shape
`NumberFormat.getInstance(Locale)` already gives Java code:

```java
Cast.f64("1.234,5", Locale.GERMANY);                           // 1234.5
Cast.i32("1\u202F234", Locale.FRANCE);                          // 1234 — fr-FR groups with U+202F
Cast.f64(text, Locale.getDefault(Locale.Category.FORMAT));     // the default, said out loud
```

A `null` locale is a `NullPointerException`, never a silent default: pass
`Locale.getDefault(Locale.Category.FORMAT)` to mean the JVM's. Deriving a format reads the
locale's `DecimalFormat` and `DecimalFormatSymbols`, which build new objects each time, so
`NumFormat.from` keeps each locale's format once derived (a bounded cache of 64) and the
overloads cost what the `NumFormat` doors do: a grouped `Cast.i32` measured 45.6 ± 5.5 ns
through `Locale.US` against 50.2 ± 11.8 ns through `NumFormat.INVARIANT` — the lookup is
inside the noise — where deriving the format on every call, every field of it, measured
1199.0 ± 169.4 ns and 3.0 KB (JMH, linux-x64, Temurin 25).

The currency symbol is matched whole, once, at either edge of the numeric body — leading
(`$5`, `-$5`, `$ -5`) or trailing (`5 €`, `1.234,50 kr.`) with optional whitespace between
it and the digits — and only while `STYLE_CURRENCY` (part of `STYLE_ALL`) is set: declared
without it, the symbol is the `MALFORMED` span. With the symbol in the text, the digits are
read with the currency separators where the format declares them — Swiss French writes a
decimal comma in numbers and a decimal point in currency, Austrian German groups numbers
with a no-break space and currency with a dot, and `from(Locale)` fills them from
`getMonetaryDecimalSeparator()` and `getMonetaryGroupingSeparator()` where they differ from
the number separators; under `STYLE_SEPARATOR_DETECT` detection governs the whole token and
they are ignored. The three-argument constructors declare no symbol.

Declare a format outright with the `String` constructors and the `with*` methods: a
separator is one or two characters — 1 to 8 UTF-8 bytes, so `new NumFormat(".", ", ",
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
too), signs and the percent symbol with no ASCII whitespace, no ASCII digit in any symbol,
a currency symbol of at most 16 bytes, a zero that starts a decimal digit block.

The boolean door's lexicon stays English (`true`/`yes`/`on`/`1` and the rest of the twenty),
by decision: no platform carries culture data for yes/no words, so no bridge could fill one.
A caller maps its own words before the door.

`Cast.decimal` is the exact door, and a canonical one. No `double` is formed on the way:
`0.1` is one tenth and `50%` is exactly `0.5`. Exact trailing zeros in the fraction are
trimmed, so the scale is minimal — `1.10`, `1.1` and `1.1000` all have a scale of 1, `100`
stays `100` (integer zeros are never touched), and zero is scale 0, never negative. Nothing
but a zero is ever dropped: precision past 2^96−1 or 28 places is `OUT_OF_RANGE`, never
rounded.

## DateTimeFormat: the date order and the day periods

`Cast.dateTime` reads `1/7/2026 3:04 PM` under a declared `DateOrder`, and the English
`AM`/`PM` it always read are read ASCII case-insensitively, after the clock or ahead of it
(`PM 3:04`). A culture's own designators are declared beside the order in a
`DateTimeFormat` — `{order, amDesignator, pmDesignator}`, `NumFormat`'s parallel for the one
temporal door that reads culture-sensitive text — and matched exactly, in either position,
with the English pair still read beside them. `DateTimeFormat.from(Locale)` fills it from
the locale's short-date pattern and `DateFormatSymbols.getAmPmStrings()`, the door takes
the `Locale` itself, and the `DateOrder` overloads are unchanged: they declare nothing.

```java
Cast.dateTime("2026-01-07 오후 3:04", DateTimeFormat.from(Locale.KOREA));          // 15:04
Cast.dateTime("2026-01-07 오후 3:04", Locale.KOREA);                                // the same, as a Locale
Cast.dateTime("7.1.2026 3:04 nachm.",
        new DateTimeFormat(DateOrder.DAY_MONTH_YEAR, "vorm.", "nachm."));           // 15:04, January 7th
Cast.dateTime("2026-01-07 오후 3:04", DateOrder.YEAR_MONTH_DAY);                   // MALFORMED at 오: nothing declared
```

A designator is 1 to 16 UTF-8 bytes with no ASCII digit and no ASCII whitespace at either
end (a space inside, as in Spanish's `a. m.`, is fine); anything else is an
`IllegalArgumentException` at construction. `from(Locale)` leaves the English pair
undeclared, since the core reads it anyway, and so leaves a designator the layout cannot
hold — the JDK's data has over-16-byte ones for a hundred-odd locales — rather than
refusing the locale, so those read exactly as they did before designators could be
declared. Derived formats are cached per locale as `NumFormat.from`'s are: `Cast.dateTime`
on `1/7/2026 3:04 PM` measured 74.1 ± 17.2 ns through `Locale.US` against 75.5 ± 14.4 ns under
the `DateOrder` alone, and 97.5 ± 7.4 ns with Spanish designators declared and matched — the
40-byte layout crossing where NULL did, and a longer designator read.

## Gating on the core, and what a span counts in

Nothing loads until the first door is called. A consumer with a managed fallback gates on
`Cast.isAvailable()` first: probed once, cached, never throws — `false` when the platform
library will not open, the jar carries no core for this OS/arch, or an older core lacks an
export this binding was built against. The doors do
not fall back. The first one to need a core that failed to load throws
`ExceptionInInitializerError`, and every later one `NoClassDefFoundError`; either way the
original failure is in its cause chain.
`Cast.nativeVersion()` reports the loaded core's `major.minor.patch`, and succeeds exactly
when `isAvailable()` is `true` — the pair that proves the library that resolved is the one
this jar was built against.

A `Fault`'s `offset`/`length` count in the input's own unit. Through a `byte[]` or
`MemorySegment` door they are the core's byte span into the UTF-8, verbatim. Through a
`String` door they are UTF-16 code units — the byte span rebased, so
`text.substring(f.offset(), f.offset() + f.length())` is the offending text even when the
input is not ASCII: `Cast.i32("1€", …)` faults at `(1, 1)` as a `String` and `(1, 3)` as
bytes. ASCII input is identical either way and is never touched.

## Native access

FFM downcalls are a *restricted* operation: the JDK wants the application, not a library on
its classpath, to say that native code may run. Without that the first cast still works,
and prints a four-line warning naming `java.lang.foreign.SymbolLookup::libraryLookup` and
ending "Restricted methods will be blocked in a future release unless native access is
enabled". Grant it where the JVM is launched:

```sh
java --enable-native-access=ALL-UNNAMED -cp app.jar:hypercast.jar com.example.Main        # on the classpath
java --enable-native-access=io.github.skunkwerkx.hypercast -p mods -m com.example.app     # on the module path
```

The jar's manifest carries `Automatic-Module-Name: io.github.skunkwerkx.hypercast`, so the
module-path form has a stable name to grant rather than one derived from the jar's file
name. An executable jar can make the same grant in its own manifest
(`Enable-Native-Access: ALL-UNNAMED`). Under `--illegal-native-access=deny` with no grant
the library cannot be loaded at all: `isAvailable()` is `false` and the doors throw.

## Why not `Integer.parseInt` / `Instant.parse` / the formatter zoo?

1. **Verdicts, not exceptions** — `NumberFormatException`-driven control flow costs a
   throw+fill-in-stack-trace on every bad input; a `Fault` is two ints and a reason, and
   bad data is the *expected* case when the text is untrusted.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, radix prefixes, all five .NET `Guid` text forms, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (the whole suite green, full corpus replay through
   real FFM downcalls with byte-exact fault spans).
4. **Faster where it matters, and the input no longer copies.** JMH, `-prof gc` for the
   allocation column, the profile `./gradlew :benchmarks:jmh` runs: 1 fork, 3 warmup + 5
   measurement iterations of one second (linux-x64 on an Intel Core i9-11900H, Temurin 25.0.4).
   The error bars are that short profile's; the verdicts are far outside them.

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
   `NumFormat.INVARIANT` in the same run — inside the noise. Rejection
   is where the verdict pays off: `Cast.i32` returns the `Fault` for `1,234x567` in
   60.3 ± 3.6 ns and 128 B, while `Integer.parseInt("1234x567")` takes 984 ± 77 ns and 888 B
   to throw its `NumberFormatException` and have it caught, 16x slower. The
   `DateTimeFormatter.ISO_OFFSET_DATE_TIME` control measures 715.5 ± 43.6 ns for the text
   the timestamp door reads in 52.5.

**What changed, twice.** 0.1.0's first tuning removed the `Arena.ofConfined()` every door
used to open per call — one `ThreadLocal` holds the out/fault/format segments for the life
of the thread — worth ~100 ns a call. 0.2.0 removed the copy that was left: every downcall
is now linked `Linker.Option.critical(true)`, so the caller's own `byte[]` crosses as a
pinned heap segment instead of being copied into a per-thread native staging buffer, and
that buffer and its arena are gone. Sound because every door is a short, non-blocking
parse over those bytes that never calls back into Java — the profile that option exists
for — and `reachability-metadata.json` registers the option, so the GraalVM Native Image
smoke test proves it under AOT too. Every door also gained a `MemorySegment` overload: slice
one buffer holding many values (a mapped file, a direct buffer, one line of a CSV) and cast
a value out of it with nothing copied. The UUID door reads its sixteen bytes as two
big-endian longs instead of one byte at a time, which took about a third off that row.

**The honest trade-off:** three rows lose, each to a JDK method with no boundary to cross
and one shape to read. `Boolean.parseBoolean` is unbeatable by construction: JIT folds a
loop-invariant `parseBoolean` into nothing, which an FFM downcall structurally can't match
— the twenty-lexeme vocabulary is why anyone calls this door. `UUID.fromString` is pure
bit-twiddling over one text form; what this door adds for its ten nanoseconds is the
N/B/P/X forms and `urn:uuid:` prefixes that method doesn't accept. `new BigDecimal(String)`
reads plain digits and throws on anything else; `Cast.decimal` reads the same grouping,
currency and parentheses every other numeric door does and returns a verdict. It's also a
native dependency: for plain invariant integers, `Integer.parseInt` is the reasonable
choice.

## AOT

The GraalVM Native Image smoke test (`./gradlew :aot-smoke-test:nativeRun`) builds and
runs the `isAvailable()`/`nativeVersion()` probe, all twenty-six doors through their
`String` form (the date-time door with designators declared too, and a format with the
0.9.0 fields filled), the `byte[]` and `MemorySegment` forms (heap slice and native segment)
once per ABI shape, and the exhaustive union switch as a true native binary. Native Image
needs two separate registrations
and the jar ships both in its `reachability-metadata.json` under
`META-INF/native-image/io.github.skunkwerkx/hypercast/`, so a consumer inherits them with no
configuration: the FFM downcall *signatures* (reachability is per-signature, not per-function
— the doors share six shapes, and the version probe's `() -> int` is the seventh),
and a `resources` glob covering `native/*/*`. A `native-image.properties` beside it has the
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

The resources half was missing from v0.0.1, and the failure mode is worth knowing because
nothing catches it at build time: Native Image doesn't embed classpath resources unless they
are registered, so `Cast`'s `getResourceAsStream("/native/{rid}/{lib}")` returned null and a
consumer's binary compiled clean, then died on its first call with "classpath resource not
found". The in-repo smoke test was green throughout, because it declared the glob in its own
build file — so it proved only that *this repo* could be configured to work. That override is
gone now; the test passes on the packaged metadata alone, which is the only thing that
actually proves a consumer is fine.

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
the code `Cast` itself uses, so a value read that way is the value the door would have
returned.

- `NativeValues` — every out-value reader, each taking a `MemorySegment` and a byte offset:
  `instant`, `date`, `civil`, `time`, `duration`, `decimal`, `uuid`; `writeFormat`, the
  92-byte size-tagged `NumFormat` layout every numeric door reads, and `writeDayPeriods`,
  the 40-byte designator layout the date-time door reads; `fault(code, offset, length)`;
  `version(packed)` for a `*_version()` word; and the `*_BYTES` constants, each layout's
  size and so the stride of an array of them (`FORMAT_BYTES` is 92, `DAY_PERIODS_BYTES` 40,
  `FAULT_BYTES` 8).
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

## Install

Published to [Maven Central](https://central.sonatype.com/artifact/io.github.skunkwerkx/hypercast)
— no extra repository configuration, since `mavenCentral()` is already in virtually every
Gradle/Maven build:

```kotlin
dependencies {
    implementation("io.github.skunkwerkx:hypercast:<version>")
}
```

The current version is the one on the Maven Central badge above. Requires JDK 25 or later.
The jar bundles a native build for all eight platforms (linux-x64, linux-arm64,
linux-musl-x64, linux-musl-arm64, osx-x64, osx-arm64, win-x64, win-arm64) and picks the
right one at runtime; see [Native access](#native-access) for the one flag the JVM wants.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
