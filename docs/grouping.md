# Grouping

A `NumFormat` has one grouping policy, and it judges every group separator in the integer part
of a numeral. There are two policies, Western and Indian. Neither is ever inferred from a name or
from the input: a format is Western unless its caller, or a locale's number pattern read through
a bridge, says Indian.

| Numeral | Western | Indian |
| --- | --- | --- |
| `1,000`, `1,234` | reads | reads |
| `1,234,567` | reads | `Malformed` |
| `1,23,456`, `12,34,567`, `1,23,45,678` | `Malformed` | reads |
| `0125`, `1234567` (no separator) | reads | reads |
| `0,125`, `0,000`, `12,34`, `1,2`, `1234,567`, `1234 567`, `1,2,345` | `Malformed` | `Malformed` |

Under Western, the groups are exactly three digits counted from the decimal separator or the end
of the numeral, and the leftmost holds one to three. Under Indian, the group next to the decimal
separator holds three and every group to its left two, the leftmost one or two. Under both, the
leftmost group can't start with a zero. Every refusal here is `Malformed` with the detail
`group_size`, spanning the group that doesn't fit. A policy doesn't turn grouping on: with the
grouping style off, a group separator is `Malformed` under both.

`corpus/grouping.json` holds these shapes, and every binding replays it.

## The flags

The styles are bits 0 to 7 of the flags word. The policy is read from the field in bits 8 and 9.

| Value | Meaning |
| --- | --- |
| `0` | Western, the default: no bits, so a null format, a 1.0 layout, a 1.0 export and a 1.0 payload are all Western with no special handling |
| `512` (`2 << 8`) | Indian, declared |
| `256` (bit 8) | Retired permanently and never reused |
| `768` | Refused: it holds the retired bit |

At the default styles the two refused words are `351` and `863`. A future policy, such as
four-digit grouping, takes a new bit from bit 10 up.

A bad policy fails where it enters, at every constructor, `with_grouping` equivalent, raw-flags
setter, ABI validator and deserializer. Rust's `with_grouping` takes `Grouping`, an enum of the two
policies, so it has nothing to refuse, and `Grouping::from_code` is the way from a number to one.
These entry points can't refuse, and what stops the value instead is named here:

| Entry point | Why it can't refuse | What stops it |
| --- | --- | --- |
| The 1.0 C exports | They mask flags to 8 bits, so they can't see the policy field | They read every format Western; `cast_detailed` and its siblings refuse with -1 |
| Rust's `NumFormat::new` and `NumFormat::with_separators` | They're 1.0 API, returning a `NumFormat` from a plain `u32` | `check` and `validate` refuse; a plain door reads the stale bits Western, never Indian |
| Rust's `NumFormat` fields, and raw doors | The fields are public | `check` and `validate` refuse; a plain door reads the stale bits Western, never Indian |
| Go struct literal, `encoding/gob` and any decoder other than `encoding/json` (`encoding/xml` decodes `<Styles>351</Styles>`) | A literal runs no code, and a reflective decoder sets the field directly. A `GobDecoder` on the type would also refuse every stored 1.0 stream | `Raw()` returns an error, and the first door panics |
| Ruby `Marshal`, 1.0 struct form | It restores member by member, with no hook | The first use raises `ArgumentError` |
| R `readRDS()` | It has no hook | The first door raises an error |
| C# `RawNumFormat.Flags` | It's a public struct field, and no door takes it from a caller | The core's detailed exports refuse with -1 |
| C `HyperCastRawNumFormat` | The caller fills it in | The core's detailed exports refuse with -1 |

A stored format stops at decode wherever the decoder has a hook. PHP's `unserialize` and Go's
`encoding/json` both refuse bit 8 and any bit past the policy field, though 1.0 ignored those bits
in PHP. The one difference left is where the policy lives: PHP's `flags` holds styles alone, so a
stored `607` is refused and the policy is the `grouping` property, while Go's `Styles` holds the
policy too, so a stored `607` is an Indian format.

## Locale bridges

A bridge reads the locale's number pattern, never its name. An all-Indian pattern (three, then
twos) is Indian, and anything else is Western. The grouping style is cleared only when neither the
number nor the currency pattern groups. One format has one policy, so numbers and currency
amounts share it.

A bridge can only follow what its platform says, and platforms disagree about the same locale.
Whichever policy a bridge gives, declare the other on the format to read the other:

| Binding | Bridge | Where its data comes from |
| --- | --- | --- |
| C# | `NumFormat.From` | .NET's globalization: `libicu` on Linux, the system's ICU on macOS |
| Java | `NumFormat.from(Locale)`, and `Cast`'s `Locale` doors | The JDK's own CLDR tables, never ICU |
| Swift | `NumFormat.from(locale:)`, `from(formatter:)` | Foundation: Apple's on Darwin, swift-foundation's on Linux |
| PHP | `fromNumberFormatter`; `fromLocaleconv` | The ICU that ext-intl links; the C library |
| Python | `from_localeconv` | The C library, or the dict the caller passes |
| R | `num_format_from_localeconv` | The C library, through `Sys.localeconv()` |
| Ruby | `from_i18n` | The caller's Rails locale Hash, which says nothing about grouping, so `grouping:` declares it |
| Rust, Go, TypeScript, C, C++ | None | The caller declares it |

Three limits a user meets:

- **Java.** `DecimalFormat` keeps one grouping size, so on JDK 21 and 25 `en-IN`, `hi-IN`, `ml`,
  `mr`, `or` and `ta` have the pattern `#,##0.###`, and the JDK writes threes for them. The
  bridge is Western, so `Cast.i64("12,34,567", Locale.forLanguageTag("hi-IN"))` is `Malformed`.
  Text in CLDR's Indian grouping is read with `NumFormat.from(locale).withGrouping(Grouping.INDIAN)`.
- **R.** R keeps `LC_NUMERIC` at `"C"`, so `Sys.localeconv()` is Western even in an Indian locale.
  Declare it with `num_format_with_grouping(format, "indian")`.
- **Swift on Linux.** swift-foundation (Swift 6.4.2) gives `en_IN` and `hi_IN` the pattern
  `#,##0.###` and writes `1,234,567.5`, so the bridge there is Western where Darwin's is Indian.
  `.withGrouping(.indian)` declares it.

## Parked

### Locales no policy matches

Measured on 2026-11-06. ICU's numbers come from PHP's `NumberFormatter` (ICU 66.1, 72.1 and 76.1 in
Docker images, 79.1 on this Mac) and from Node 26's `Intl.NumberFormat` (ICU 78.3, which exposes no
pattern). glibc's come from `locale.localeconv()` and `locale.format_string` under glibc 2.41 in
`debian:trixie-slim`, each locale compiled with `localedef` from the source Debian ships.

| Locale | Source | Pattern | Writes |
| --- | --- | --- | --- |
| `tok` | ICU 78.3 (CLDR 48) | Not exposed | `1 23 45 67 89,5` for 123456789.5, `1 23 45 67,5` for 1234567.5 |
| `tok` | ICU 76.1 (CLDR 46), 79.1 (CLDR 49) | `#,#0.###` | `1 23 45 67,5` for 1234567.5 |
| `tok` | JDK 25 | `#,#0.###` | `1 23 45 67,5` for 1234567.5 |
| `tok` | Foundation, macOS 27.2 | `#,#0.###` | `1 23 45 67,5` for 1234567.5 |
| `tok` | .NET 10, macOS 27.2 and Ubuntu 24.04 | `NumberGroupSizes` `[2]` | `1 23 45 67,5` for 1234567.5 |
| `cmn_TW`, `hak_TW`, `lzh_TW`, `nan_TW` | glibc 2.41 | `[4, 0]`, currency the same | `123,4567.50` for 1234567.5 |
| `unm_US` | glibc 2.41 | `[2, 2, 2, 3, 0]`, currency `[3, 0]`, separator U+202F | `9 99.00` for 999, `1 23 45 67.50` for 1234567.5 |

Locale data moves. `tok` on ICU 66.1 (CLDR 36.1) and 72.1 (CLDR 42) is `0.######` and doesn't group
at all. On JDK 21 it is `#,##0.###` and on swift-foundation (Swift 6.4.2) too, in threes, so both
read it Western. ICU's `zh-TW` is Western on every version measured (`#,##0.###`, `1,234,567.5`), so
the fours belong to glibc's locales alone.

**Why they're refused.** Under the delimiter rule, a last group that isn't three digits is a
decimal, so `1,2345` can't be told from 1.2345 without a declaration. Neither policy declares it.

**What they can do today.** Their ungrouped numerals read. Under a declared format, `Invariant` or
a bridge, their grouped ones are `Malformed`, never misread. Through a bridge these locales are
Western.

**Under `Detect`, they can misread.** `Detect` is inference from the input, not a declaration, and
the delimiter rule decides it: a right run that isn't three digits is a decimal. So a `[4, 0]`
locale's grouped output with one separator reads as a decimal. `1,2345` is 1.2345 and `123,4567` is
123.4567 at the real and decimal doors, where a `cmn_TW` writer meant 12345 and 1234567, and the
integer doors refuse both as `fraction_not_allowed`. `123,4567.50` and `1,2345,6789`, which the rule
can't read as decimals, are `Malformed`. Those locales' columns shouldn't use `Detect`.
`corpus/real.json`, `decimal.json` and `integer.json` pin these answers under both policies.

**What adding one takes.** A declared policy with its own flag bit from bit 10 up (256 is retired),
lanes, corpus vectors, the bridge mapping and every binding. It's declared only, never a default.

### Locales whose number and currency patterns differ

Which locales these are depends on the data in hand, so no list here is timeless. ICU, for
1234567.5 as a number and then as INR, on every version measured (66.1, 72.1, 76.1, 78.3 and 79.1):

| Locale | Number | INR | Number pattern | Currency pattern |
| --- | --- | --- | --- | --- |
| `ml`, `or` | `12,34,567.5` | `₹1,234,567.50` | `#,##,##0.###` | `¤#,##0.00` |
| `mr` | `१२,३४,५६७.५` | `₹१,२३४,५६७.५०` | `#,##,##0.###` | `¤#,##0.00` |
| `ta-SG`, `ta-MY` | `1,234,567.5` | `₹ 1,234,567.50` | `#,##0.###` | `¤ #,##0.00` |

`ta` and `ta-IN` move with the version:

| ICU (CLDR) | Number | INR | Currency pattern | Number and currency |
| --- | --- | --- | --- | --- |
| 66.1 (36.1) | `12,34,567.5` | `₹ 12,34,567.50` | `¤ #,##,##0.00` | Agree |
| 72.1 (42) | `12,34,567.5` | `₹12,34,567.50` | `¤#,##,##0.00` | Agree |
| 76.1 (46) | `12,34,567.5` | `₹12,34,567.50` | `¤#,##,##0.00` | Agree |
| 78.3 (48) | `12,34,567.5` | `₹12,34,567.50` | Not exposed | Agree |
| 79.1 (49) | `12,34,567.5` | `₹ 1,234,567.50` | `¤ #,##0.00` | Disagree |

The number pattern is `#,##,##0.###` on all of them. Versions older than 66.1 weren't measured.

The bindings that don't read plain ICU, measured the same day:

| Source | `ml`, `or`, `mr` | `ta` | `en-IN`, `hi-IN` |
| --- | --- | --- | --- |
| .NET 10 on Ubuntu 24.04 (`libicu` 74.2) | Number `[3, 2]`, currency `[3]` | `[3, 2]` for both | `[3, 2]` for both |
| .NET 10 on macOS 27.2 | `[3, 2]` for both | `[3, 2]` for both | `[3, 2]` for both |
| Foundation on macOS 27.2 | `#,##,##0.###`, `¤#,##,##0.00` | The same | The same |
| swift-foundation on Linux (Swift 6.4.2) | `#,##0.###`, `¤#,##0.00` | The same | The same |
| JDK 21 and 25 | `#,##0.###` for both | `#,##0.###` for both | `#,##0.###` for both |
| C library, macOS 27.2 | No such locale | No such locale | `[3, 2, 0]` for both |
| glibc 2.41 | See below | `[3, 2, 0]` for both | `[3, 2, 0]` for `en_IN` |

glibc 2.41: `ml_IN`, `or_IN` and `ta_IN` are `[3, 2, 0]` for both. `mr_IN` is `[3, 0]` for numbers and
`[3, 2, 0]` for currency, the reverse case, so its bridge is Western and its own `₹12,34,567.50` is
refused until the column declares Indian.

**The cost.** Where a locale's numbers are Indian and its currency Western, the format is Indian
because its numbers are, so a Western `₹1,234,567` is refused. Declaring Western on the column reads
it, with `with_grouping` or the binding's equivalent. Where the reverse holds, as for glibc's
`mr_IN`, declaring Indian reads the currency.
