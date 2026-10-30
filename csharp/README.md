# HyperCast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![NuGet](https://img.shields.io/nuget/v/HyperCast.svg)](https://www.nuget.org/packages/HyperCast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**`TryParse` hands back a `bool` and a shrug. These doors hand back a native discriminated
union — the value, or `Empty`/`Malformed`/`OutOfRange` plus the exact span that offended, in
the units of the input you passed — and an unhandled case is a compile error, not a review
nit.**

Allocation-free scalar casts — booleans, characters, the full integer family, reals, an exact
`decimal`, UUIDs, temporals — as source-generated `[LibraryImport]` P/Invoke straight into
the native `libhypercast` Rust core. No runtime bridge, no reflection anywhere in the
assembly. It targets .NET 10 and 11. `Verdict<T>` is a real `[Union]`, and CS8509
(non-exhaustive switch) elevated to an error — as this repository does, and a consumer does
with `<WarningsAsErrors>CS8509</WarningsAsErrors>` — makes a missing disposition fail the
build, the entire point of returning a union instead of throwing. The `switch` below is
C# 15; on C# 14, `Match` and `Switch` do the same job ([below](#without-c-15)).

```sh
dotnet add package HyperCast
```

```csharp
using System.Globalization;
using HyperCast;

var culture = CultureInfo.GetCultureInfo("en-US");
var message = Cast.Int32("(1,234)", NumFormat.From(culture)) switch
{
    Success<int> s => $"got {s.Value}",                  // -1234, accounting negative
    Fault f => $"{f.Reason} at char {f.Offset}",         // no third case: the compiler checked
};
Console.WriteLine(message);

var timestamp = Cast.Timestamp("2026-01-02T15:04:05.123456789+05:00") switch
{
    Success<DateTimeOffset> s => $"timestamp {s.Value:O}", // normalized to UTC, to the 100 ns tick
    Fault f => $"{f.Reason} at char {f.Offset}",
};
Console.WriteLine(timestamp);
```

The `switch` needs the target type spelled out. Inside a method generic over it, where the
verdict is a `Verdict<T>` with `T` still open, the compiler cannot tell a case pattern on the
union from one on its underlying value and rejects binding a variable there (CS8780). Ask the
union for each case with `TryGetValue` instead:

```csharp
static string Describe<T>(string text) where T : struct
{
    var verdict = Cast.Scalar<T>(text, NumFormat.Invariant);
    if (verdict.TryGetValue(out Success<T> success)) return $"got {success.Value}";
    verdict.TryGetValue(out Fault fault);                    // the only other case
    return $"{fault.Reason} at char {fault.Offset}";
}

Console.WriteLine(Describe<int>("(1,234)"));                // got -1234
```

### Without C# 15

The union `switch` needs C# 15: a .NET 11 project's default, and
`<LangVersion>preview</LangVersion>` in a .NET 10 one, on either SDK. The .NET 10 SDK's
compiler calls the feature "Preview and *unsupported*" and stops at its default `LangVersion`
with CS8652; the .NET 11 SDK at `LangVersion` 14 stops with CS9327. Without C# 15, `Match` and
`Switch` take one delegate per case, both required, so a missing case is still a compile error:

```csharp
var verdict = Cast.Int32("(1,234)", NumFormat.From(CultureInfo.GetCultureInfo("en-US")));
var message = verdict.Match(s => $"got {s.Value}", f => $"{f.Reason} at char {f.Offset}");
Console.WriteLine(message);                                 // got -1234
verdict.Switch(s => Console.WriteLine(s.Value), f => Console.WriteLine(f.Reason));
```

`TryGetValue`, above, works there too. All three are on both frameworks, so a .NET 10 library
that calls them runs unchanged inside a .NET 11 app. Each throws `InvalidOperationException`
on `default(Verdict<T>)`, as the union `switch` throws `SwitchExpressionException`.

Door names mirror the native ABI (`Int32`, `Double`, `Decimal`, `Timestamp`, …) so the
polyglot surface reads identically across bindings; `Cast.Numeric<T>` fronts all eleven
numeric doors for a caller that is itself generic over the target, and `Cast.Scalar<T>`
fronts every text door (see [One door for a generic `T`](#one-door-for-a-generic-t)). Culture never lives in
the core — `NumFormat.From(CultureInfo)` (or `From(IFormatProvider)`, the shape every BCL
`TryParse` already takes) bridges .NET's culture machinery to the caller-declared format
the native side actually reads: separators, lenience flags, the currency symbol and the
separators a currency amount is written with, the culture's own minus, plus and percent
signs, and its native digits — each read beside the ASCII spelling the core reads
regardless. `DateFormat.From(CultureInfo)` does the same for a date's field order and the
culture's AM/PM designators. Every numeric door, `Numeric<T>`, `Scalar<T>` and the civil
`DateTime` door also take that `IFormatProvider?` directly, as `TryParse` does (see
[A culture, the way the BCL takes one](#a-culture-the-way-the-bcl-takes-one)). Or spell
the format directly — `new NumFormat(',', '\u00A0', NumStyles.All, "€")` declares an
arbitrary pair, `NumFormat.Invariant with { MinusSign = "−" }` a further sign, and
`NumStyles.None` turns every lenience off. .NET-flavored
fidelity, stated honestly: `DateTimeOffset`/`TimeOnly`/`TimeSpan` resolve to 100 ns
ticks, so sub-tick nanoseconds truncate (the core carries full nanosecond fidelity; .NET's
clock types don't). `Cast.Decimal` is exact and canonical — sign, 96-bit magnitude, trailing
fraction zeros trimmed, never rounded. A `Fault`'s span is in the caller's own units: byte
offsets from the `ReadOnlySpan<byte>` doors, char offsets from the
`string`/`ReadOnlySpan<char>` ones, so slicing the offending text back out of what you
passed needs no mapping on either side.

Before the first cast, `Cast.IsAvailable` says whether the native library resolved and
`Cast.NativeVersion` names the core it loaded, as a `System.Version` (`null` when it did not
load) — the probe a consumer with a managed fallback
gates on, instead of catching `DllNotFoundException` around its first real call. It is
probed once and never throws; every door lets a load failure propagate. A library that
loaded but predates the probe reads as unavailable too: a stale binary beside a newer
binding is exactly the mismatch it exists to name.

## One character

`Cast.Char` reads one `char`: an input that is exactly one character is that character,
taken before any trimming (`" "` is a space, `"6"` is the digit six); otherwise, with ASCII
whitespace trimmed, exactly one code-point spelling — `65`, `U+0041`, `0x41`, `&H41`, or an
HTML numeric entity `&#65;` / `&#x41;` (the `;` required), case-insensitive on the prefix. A
surrogate or anything past `U+10FFFF` spelled as a code point is `OutOfRange`. So is a real
scalar above `U+FFFF` (`U+1F600`, or a verbatim emoji): the core accepts it, but one
`System.Char` can't hold it, so this binding faults `OutOfRange` over the trimmed input. The
`string`/`ReadOnlySpan<char>` overload returns a single UTF-16 code unit as it is without
crossing into the core, a lone surrogate included, because "exactly one character" is
what it was handed; inside longer text a lone surrogate transcodes to U+FFFD like any other
door's input.

## One door for a generic `T`

`Cast.Scalar<T>(text, format)` dispatches on `typeof(T)` to the door that reads `T`, for a
caller generic over the target (a `System.Text.Json` converter, a parsing gateway) that would
otherwise keep its own table mirroring `Cast`. The verdict is exactly the concrete door's,
spans included:

| `T` | Door |
| --- | --- |
| `bool` | `Boolean` |
| `sbyte` … `ulong`, `float`, `double`, `decimal` | the numeric doors, under `format` |
| `char` | `Char` |
| `Guid` | `Uuid` |
| `DateOnly` | `Date`, strict ISO `yyyy-MM-dd` (no `DateOrder`) |
| `TimeOnly` | `Time` |
| `DateTimeOffset` | `Timestamp` (RFC 3339, zone mandatory) |
| `DateTime` | `Timestamp`, projected to `.UtcDateTime` (`DateTimeKind.Utc`) |
| `TimeSpan` | `Duration` |

`DateTime` here is **not** `Cast.DateTime`'s civil door. That one needs a declared
`DateOrder` and returns `DateTimeKind.Unspecified`, and a generic caller has no order to
declare. The reading that needs no declaration is the RFC 3339 instant, so `Scalar<DateTime>`
is that instant in UTC, and zone-less text is `Malformed`. A caller that wants the civil
reading special-cases `DateTime` and calls `Cast.DateTime(text, order)` itself.

`format` is read only by the numeric doors. Any other `T` (`Int128`, `Half`, `nint`, an
enum, your own struct) throws `NotSupportedException` naming the type, before any native
call. That is a caller bug, not a data verdict, so a gateway can try `Scalar<T>` first and
fall back. `Cast.Optional(Cast.Scalar<T>(...))` composes as with any door, and the `typeof`
tests fold per instantiation under the JIT and Native AOT alike.

## A culture, the way the BCL takes one

Every door that takes a `NumFormat` — the eight integers, `Single`, `Double`, `Decimal`,
`Numeric<T>` and `Scalar<T>`, for `ReadOnlySpan<char>` and UTF-8 input alike — and the
civil `DateTime` door, which takes a `DateFormat`, has an overload taking the
`IFormatProvider?` a BCL `TryParse` or `ISpanParsable<T>` takes, so code that is already
provider-shaped passes what it was handed straight through:

```csharp
// A parsing gateway in ISpanParsable's shape, routing through HyperCast in one call.
// Generic over T, so TryGetValue rather than a switch (CS8780; see the top of this page).
static T ParseRequired<T>(ReadOnlySpan<char> text, IFormatProvider? provider) where T : struct
{
    var verdict = Cast.Scalar<T>(text, provider);
    if (verdict.TryGetValue(out Success<T> success))
        return success.Value;
    verdict.TryGetValue(out Fault fault);
    throw new FormatException($"{fault.Reason} at {fault.Offset}+{fault.Length}");
}

Cast.Double("1.234,5", CultureInfo.GetCultureInfo("de-DE"));   // 1234.5
Cast.Int32("1.234", null);                                       // under the current culture
Cast.Int32("−5", CultureInfo.GetCultureInfo("et-EE"));          // -5: Estonian's U+2212, and ASCII - too
Cast.Decimal("€1,234.50", CultureInfo.GetCultureInfo("en-DE")); // 1234.5: the currency separators, as the BCL
                                                                // (where the runtime's ICU spells en-DE so; not every one does)
Cast.Double("۵۰٪", CultureInfo.GetCultureInfo("fa-IR"));        // 0.5: Persian digits and ٪
```

The provider maps through `NumFormat.From(IFormatProvider)`: every field the core reads,
from the culture's own data — its decimal and group separators, whole, however many
characters each is; its currency symbol, and the separators it writes a currency amount
with where they differ (`en-DE` and a dozen others write a decimal comma in numbers and a
decimal point in currency, in the ICU data many runtimes carry, and the BCL switches when it
sees the symbol, as the core does); its `NegativeSign`, `PositiveSign` and
`PercentSymbol` where they are not the ASCII ones (U+2212 in `et-EE` and `eu-ES`, the bidi-marked signs of the Arabic-script cultures,
`٪`); the zero of its `NativeDigits` where it is not `0` (`ar-SA`, `fa-IR`, `bn-BD`); and
`NumStyles.TrailingSign` when its negative pattern puts the sign after the digits
(`NumberNegativePattern` 3 or 4, or a `CurrencyNegativePattern` of the same shape —
`fy-NL` writes `€ 5,00-`) — with every lenience on. Whatever a runtime's data holds, the
bridge never throws: a symbol the core could not read (an ASCII digit in it, whitespace
against the digits, past its buffer) is left undeclared and the ASCII spelling read alone;
a group separator equal to the decimal one (NLS gives `mn-Mong-MN` a comma for both) takes
the other of `.` and `,`; a currency pair that would collide declares nothing. `null` means
the current culture, resolved exactly as `TryParse` resolves it
(`NumberFormatInfo.GetInstance(null)`).
`Scalar<T>` reads the provider only for the numeric targets.

The provider is resolved the BCL's way, but the text is read HyperCast's way, and that
differs from `TryParse` under the same culture in two places:

- **Leniences.** Every one is on, so a trailing `%` and a radix prefix (`0x`, `&H`, `0b`) are
  read where `int.TryParse` would reject them. Narrow them with
  `NumFormat.From(provider) with { Styles = … }`, or declare a `NumFormat` outright.
- **ASCII beside the culture.** A declared symbol is a further spelling, never a
  replacement: `-5` is -5 under `et-EE` exactly as `−5` is, `50%` is 0.5 under `fa-IR` as
  `۵۰٪` is, and ASCII digits are read beside a culture's own. Nothing that parsed before a
  symbol was declared stops parsing once it is, where `TryParse` reads the culture's
  spelling only.

Boolean words are the one culture-shaped thing no format carries, by decision:
`Cast.Boolean`'s lexicon is English (`ja` and `oui` are `Malformed`), because no platform
has culture data for one and so no bridge could fill it. A caller maps its own words before
the door.

Pass a `NumFormat` for anything stricter, or for a format that must not follow the machine.
The mapping allocates nothing; mapped on every call it costs about 12 ns, and a `NumFormat`
held in a field skips it (measured in
[What an `IFormatProvider` costs](#why-not-the-bcls-own-tryparse-family)).

`default` as the second argument means `default(NumFormat)`, a format with no
separators, which is a caller bug: the provider overloads carry `[OverloadResolutionPriority(-1)]`,
so they are chosen only for an argument a `NumFormat` cannot be — `null`, a `CultureInfo`, a
`NumberFormatInfo`.

Every field is declarable without a culture, and a declared format is the one to hold
when the text's dialect is a property of the text rather than of the process:

```csharp
using System.Text;

var swiss = new NumFormat(".", "’", NumStyles.All, "CHF");                 // a separator is 1 to 8 bytes of UTF-8
var spaced = new NumFormat(".", ", ", NumStyles.All);                       // Cast.Int32("1, 234, 567", spaced) is 1234567
var declared = NumFormat.Invariant with
{
    MinusSign = "−",                      // read wherever ASCII - is, which is still read
    PlusSign = "＋",
    PercentSymbol = "％",                  // read as a trailing % is, under NumStyles.Percent
    DigitZero = new Rune(0x0660),         // ٠: the ten Arabic-Indic digits, beside 0-9
    CurrencySymbol = "€",
    CurrencyDecimalSeparator = ",",       // in force when € is in the text; else the number separators
    CurrencyGroupSeparator = ".",
    Styles = NumStyles.All | NumStyles.TrailingSign,   // 5-, 5 -, 5 €-; not in All, a culture declares it
};
Cast.Int32("−١٢٣", declared);              // -123
Cast.Decimal("€1.234,50", declared);       // 1234.5
Cast.Decimal("1,234.50", declared);        // 1234.5: no symbol, so the number separators
Cast.Double("٥٠％", declared);             // 0.5
Cast.Int32("5 −", declared);               // -5
```

The rules a format must keep are the core's own, checked before the crossing and reported
as an `ArgumentException` against the `format` argument, never as a verdict: a separator is
one to eight bytes of UTF-8 with no ASCII digit and the two differ (so does the currency
pair in force); a sign, the percent and the currency symbol carry no ASCII digit or
whitespace; `DigitZero` is never ASCII and is the zero of a digit block; and every symbol is whole code points.

## A culture's date order and day periods

A separated date is ambiguous on its face — `1/7/2026` is January 7th or July 1st — and
the core never guesses: `Cast.Date` and `Cast.DateTime` take a declared `DateOrder`, and
`DateOrders.From(culture)` reads it off the culture's short-date pattern. A 12-hour clock has
the same problem in its marker: `3:04 nachm.`, `오후 3:04` and `3:04 p. m.` are each a
culture's own word for the half of the day, and the core reads the English `AM`/`PM` alone
unless told otherwise. `DateFormat` is that declaration for `Cast.DateTime` — the order and
the two designators together, the temporal counterpart of `NumFormat`:

```csharp
var korean = DateFormat.From(CultureInfo.GetCultureInfo("ko-KR"));   // YearMonthDay, 오전 / 오후 (where the runtime's ICU spells ko-KR so)
Cast.DateTime("2026-01-07 오후 3:04", korean);                       // 2026-01-07 15:04
Cast.DateTime("2026-01-07 3:04 PM", korean);                         // the English pair is always read
Cast.DateTime("1.7.2026 3:04 nachm.", new DateFormat(DateOrder.DayMonthYear, "vorm.", "nachm."));
Cast.DateTime("1/7/2026 3:04 PM", DateOrder.MonthDayYear);           // the order alone
Cast.DateTime("2026-01-07 오후 3:04", CultureInfo.GetCultureInfo("ko-KR"));   // the provider overload
Cast.Date("2026. 1. 7.".Replace(" ", "").TrimEnd('.'), korean.Order);        // Date has no time part: the order alone
```

A declared designator is matched exactly, after the clock (with or without a space before
it) or ahead of it; the English pair is matched ASCII case-insensitively beside it, and it
too is read ahead of the clock (`PM 3:04`). `DateFormat.From` leaves the English pair
undeclared, since the core reads it anyway, and leaves out a designator the core could not
read — past the 16-byte layout (Thai's and Devanagari's words are), carrying an ASCII digit
(Anii's `1ka`), or edged with ASCII whitespace — each judged on its own, so such a culture
reads the English pair beside whatever is declared rather than throwing: every culture any
runtime ships maps without one, whatever its ICU or NLS spells. A
`DateTimeFormatInfo` built by hand maps the same way, so the `IFormatProvider?` overload
serves a format that is no culture's.

## Numbers a workbook already holds

A spreadsheet stores a numeric cell as a `double`, and four doors read one directly, with no
text in between: `Cast.DecimalFromDouble` (the shortest decimal that names the double, the
digits the file holds — `0.1 + 0.2` is `0.30000000000000004`, where `(decimal)double` rounds
to 15 significant digits), `Cast.ExcelSerialFromDouble` (a `DateTime` of
`DateTimeKind.Unspecified` under a declared `ExcelEpoch`), `Cast.ExcelTime` (a serial's
fraction as a `TimeOnly`) and `Cast.ExcelDuration` (days as a `TimeSpan`). A typed door's
`Fault` has no span: its offset and length are 0.

## Why not the BCL's own `TryParse` family?

1. **The error story is data, not archaeology** — a closed reason plus the offending span,
   against the BCL's bare `false`.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, radix prefixes, all five `Guid` formats *plus* `urn:uuid:` prefixes,
   protobuf JSON durations, a declared currency symbol at either edge, a culture's own
   signs, digits and day periods read beside the ASCII ones — much of it grammar the BCL
   has no knob for at any price.
3. **One engine across a polyglot system** — the same Rust core, bit-for-bit verdicts,
   proven by the shared conformance corpus every binding replays: here, all fifteen files
   through real P/Invoke, fault spans asserted byte for byte.
4. **Not slower — mostly faster.** BenchmarkDotNet, `[MemoryDiagnoser]`, lenience matched
   where the BCL has the knob, FFI crossing and UTF-16→UTF-8 transcode *included* in every
   HyperCast number; zero managed allocation on every row, both sides (linux-x64 on an
   Intel Core i9-11900H, .NET 11 RC 1):

   | Door | HyperCast | BCL | Verdict |
   | --- | ---: | ---: | --- |
   | `Cast.DateTime` (`1/7/2026 3:04 PM`) vs `DateTime.TryParse` (en-US) | 37.9 ns | 171.5 ns | **4.5x faster** |
   | `Cast.Timestamp` vs `DateTimeOffset.TryParse` | 57.0 ns | 125.6 ns | **2.2x faster** |
   | `Cast.Duration` vs `TimeSpan.TryParse` | 31.6 ns | 116.5 ns | **3.7x faster** |
   | `Cast.Date` (declared order) vs `DateOnly.TryParse` (en-US) | 27.3 ns | 91.9 ns | **3.4x faster** |
   | `Cast.Int32` (grouped) vs `int.TryParse` | 38.3 ns | 45.0 ns | 1.2x faster |
   | `Cast.Double` vs `double.TryParse` | 43.3 ns | 46.4 ns | 1.1x faster |
   | `Cast.Double` (eurozone) vs `double.TryParse` (de-DE) | 47.4 ns | 62.0 ns | **1.3x faster** |
   | `Cast.Uuid` vs `Guid.TryParse` | 37.8 ns | 25.6 ns | 1.5x slower — while also taking N/B/P/X and `urn:uuid:` |
   | `Cast.Boolean` vs `bool.TryParse` | 19.7 ns | JIT-folded | honest loss — the twenty-lexeme vocabulary is why anyone calls this door |

   Reproduce: `dotnet run -c Release --project csharp/HyperCast.Benchmarks`, from the repo
   root.

   Every row above is the `string` door, transcode included. The `ReadOnlySpan<byte>`
   doors are the primary surface — a caller holding UTF-8 already (a file, a wire buffer,
   one field of a delimited line) never pays that transcode — so they are measured on
   their own, same machine, same run:

   | Door (UTF-8 in hand) | HyperCast | `string` door | BCL, same run |
   | --- | ---: | ---: | ---: |
   | `Cast.Timestamp` | **40.7 ns** | 57.0 ns | 125.6 ns `DateTimeOffset.TryParse` |
   | `Cast.Int32` (grouped) | **24.2 ns** | 38.3 ns | 45.0 ns `int.TryParse` — the win nearly doubles |
   | `Cast.Double` | **25.2 ns** | 43.3 ns | 46.4 ns `double.TryParse` — and so does this one |
   | `Cast.Uuid` | 27.2 ns | 37.8 ns | 25.6 ns `Guid.TryParse` — still a loss, by 1.6 ns |

   The UTF-16 doors try the stack buffer first and rent from the pool only when the
   encoder says the text did not fit, rather than sizing by the 3-bytes-per-char worst
   case — which would send any text past ~170 chars to the pool even when it is plain
   ASCII that fits with room to spare.

   The two doors the first consumer asked for, same run, string doors with the transcode
   included, invariant unless stated — printed as measured, because two of the three rows
   are losses:

   | Door | HyperCast | BCL | Verdict |
   | --- | ---: | ---: | --- |
   | `Cast.Decimal` (`12,345.6789`) vs `decimal.TryParse` | 57.2 ns | 59.8 ns | 1.05x faster — and exact, canonical, never rounded |
   | `Cast.Decimal` (`($1,234.50)`, en-US `$`) vs `decimal.TryParse` `NumberStyles.Currency` | 70.0 ns | 55.8 ns | 1.25x slower |
   | `Cast.Double` (same text, same format) vs `double.TryParse` `NumberStyles.Currency` | 67.0 ns | 55.6 ns | 1.2x slower |

   Grouping, a declared symbol and accounting parentheses are read in one pass by the
   core's lenient fast lane (`rust/src/lane.rs`): in the Rust suite a `cast_decimal` of
   `$12,345.67` costs 18.1 ns against 13.6 ns plain. The same lane is why the grouped
   `Cast.Int32` row and the eurozone `Cast.Double` row are wins. What the currency rows
   pay is the crossing and the transcode, against a BCL parser that has neither.

   **Separator detection is nearly free**: `NumFormat.Detect` on `1.234.567,89` costs
   54.6 ns against 47.4 ns for the same text under a declared eurozone format — ~7 ns for
   resolving the `.`/`,` roles structurally instead of being told them.

   **What an `IFormatProvider` costs over a declared `NumFormat`.** The same door, text and
   culture, `Cast.Scalar<int>` on `1,234,567`, measured in a separate run on the same machine:

   | Format argument | Mean |
   | --- | ---: |
   | `NumFormat.Invariant` | 54.4 ns |
   | `NumFormat.From(CultureInfo.InvariantCulture)`, held in a field | 71.8 ns |
   | `(IFormatProvider)CultureInfo.InvariantCulture`, mapped per call | 83.6 ns |

   Mapping the provider on every call (`NumberFormatInfo.GetInstance`, then
   `NumFormat.From` reading the culture's eleven fields) is about 12 ns and allocates nothing
   — the one BCL getter that would, `NativeDigits`, clones its array on every read, so the
   bridge reads the field it clones from. Hold the `NumFormat` in a field to keep it. The
   gap under that is the currency symbol a culture carries and `NumFormat.Invariant` does not
   (the invariant culture's is `¤`): a format whose every symbol is one ASCII character is
   written to the layout without the encoder, and `¤` is not one, so that row pays the
   encoder and the core's wider read. A caller that never reads currency can drop the
   symbol: `NumFormat.From(provider) with { CurrencySymbol = "" }`.

   **Rejecting input costs more than accepting it.** `Cast.Int32` on the malformed
   `1,234x567` takes 74.8 ns against 32.1 ns for `int.TryParse` returning `false`, both
   allocation-free. The core's fast lanes read well-formed shapes only, so a fault falls
   through to the full engine, which rescans the text to report the fault's reason and span
   (37 ns of it in the core, against 3 ns for Rust's own `str::parse` failing). Next to the
   39.9 ns the same door took on well-formed text in that run, it is not a hot path anywhere sensible,
   but it is measured, not assumed.

**The honest trade-off:** the currency rows and `Guid` are losses, expected ones, and
every row carries the same cause: a native dependency (shipped per-RID inside the package)
with an FFI crossing on every call — about 15 ns here, plus the transcode when the text is
a `string` — against parsers that run in-process. Those two are also the rows where the
BCL has a parser built for exactly the one shape being timed, so the comparison is that
parser at its best against a door doing more: a format declared per call instead of
borrowed from a culture, separators it can detect, a fault with a byte span instead of
`false`, and the same verdict in seven languages. A handful of nanoseconds is what that
costs. For plain invariant numbers the BCL is already excellent; these doors earn their
keep on the culture-machinery parsers, the closed error contract, and cross-language
agreement.

## AOT

Publishes cleanly under `PublishAot` — `LibraryImport` is source-generated with no runtime
reflection anywhere in this assembly, and the project opts into (and fails the build on) the
trim/Native-AOT analyzers via `IsAotCompatible`.

That claim is reproducible rather than asserted. `HyperCast.AotSmokeTest/` is a real
AOT-published console app that crosses every native entry point the binding declares — the
twenty-six `cast_*` functions and `hypercast_version` — plus the generic `Cast.Numeric<T>`
and `Cast.Scalar<T>` doors, a UTF-8 door, and the union's exhaustive two-arm `switch`, and returns a nonzero exit
code on any mismatch:

```shell
dotnet publish csharp/HyperCast.AotSmokeTest/HyperCast.AotSmokeTest.csproj \
  -c Release -f net11.0 -r linux-x64 -p:PublishAot=true
./csharp/HyperCast.AotSmokeTest/bin/Release/net11.0/linux-x64/publish/HyperCast.AotSmokeTest
```

Last verified on `linux-x64` and, inside an Alpine container, `linux-musl-x64`: **zero
`ILxxxx`/`AOTxxxx` trim or AOT diagnostics**, a 1.7 MB native binary with the core inside it
and no shared library beside it, and `AOT smoke test passed.` with exit code 0. `TreatWarningsAsErrors` is on for the library
project, so an analyzer warning is a build failure, not a line in a log nobody reads. CI
re-proves it per platform on every PR (see
[Native binary provenance](#native-binary-provenance)).

**The core is linked into the executable.** A Native AOT publish does not load
`libhypercast.so` (or the `.dylib`, or the `.dll`): the package carries the core as a static
library for each RID under `staticlibs/`, and its targets file hands the one for your RID to
the AOT linker and binds every P/Invoke as a direct call. The publish directory holds one
executable and no native library beside it, `Cast.IsAvailable` is always `true`, and
this package and HyperUuid's can both be linked into the same executable. Nothing to configure;
`<HyperCastStaticLink>false</HyperCastStaticLink>` in the project puts it back to loading the shared
library, and so does publishing for a RID the package has no archive for. A JIT process is
unaffected: it cannot link an archive, and loads the shared library.

## WebAssembly (Blazor)

One compiled assembly covers browser-wasm too — every native entry point is declared three
times (`"hypercast"` for dlopen platforms, `"*"` for the statically-linked wasm module, and
`"__Internal"` for iOS and Mac Catalyst; see [Platform support](#platform-support)), sharing
the same `EntryPoint`, with `OperatingSystem.IsBrowser()` and `OperatingSystem.IsIOS()`
picked at the call site and constant-folded by the linker. CI builds the `wasm32-unknown-emscripten` staticlib on every
PR; the release pack stages it under `runtimes/browser-wasm/nativeassets/`, and
`build/net10.0/HyperCast.targets` ships inside the package to wire it up for a consumer with
no configuration beyond the `wasm-tools` workload.

That targets file is load-bearing, and its `NativeFileReference` is the whole wiring: it hands
the staticlib to the linker (restore never populates `@(NativeLibrary)` from a plain
`PackageReference`'s `nativeassets/` folder the way it does `runtimes/{rid}/native/`), and the
SDK scans every assembly's P/Invokes and generates a pinvoke table that calls each door by
symbol, so the link resolves them against the archive and `LibraryImport("*")` reaches each one
through that table. Without the file a Blazor publish dies at `wasm-ld` with
`undefined symbol: cast_i32`, which is exactly the failure the smoke test below reproduces with
the archive unlinked. The file lists no `EmccExportedFunction`: `"*"` resolution does not need
the Emscripten export, and the smoke test passes without it on .NET SDK 11.0.100-rc.2.

**Target frameworks.** The package builds `net10.0` and `net11.0` from one source, and package validation holds their public APIs identical. Its targets file sits under `build/net10.0/` (and `buildTransitive/net10.0/`); NuGet hands that one file to .NET 11 projects too. A project older than .NET 10 is refused at restore with NU1202. Blazor WebAssembly works on both frameworks (see [Platform support](#platform-support)). HyperUuid's package is laid out the same way.

`HyperCast.WasmSmokeTest` proves the whole chain in a real browser: a Blazor WebAssembly app
(`net10.0` and `net11.0`) that imports that targets file, calls every native entry point — the twenty-six `cast_*`
functions and `hypercast_version` — through the public `Cast` surface, and renders `PASS` or
`FAIL` into the page. Every one: the link proves each symbol is in the archive, and only a call proves it works
through the pinvoke table.
`./check.sh` in that directory stages the wasm static library, publishes the app four ways
(each framework, in Release and in an unstripped Debug build, where the SDK runs no `wasm-opt`
after the link), loads each in headless Chromium and requires `PASS`; name legs
(`net11-release net11-debug net10-release net10-debug`) to run fewer. CI runs the same script
on every PR, in headless Chrome with the `wasm-tools` and `wasm-tools-net10` workloads, against the static library that run just built (`STATICLIB`
names it, so the script skips building its own); it is not part of the solution, so a plain
`dotnet build` never needs the workload or a browser. By hand, the same steps are:

```shell
cd rust && cargo +"$(python3 -c 'import json; print(json.load(open("../.github/hyper.json"))["wasm"]["toolchain"])')" wasm-staticlib
mkdir -p ../csharp/HyperCast/runtimes/browser-wasm/nativeassets/net11.0
cp target/wasm32-unknown-emscripten/release/libhypercast.a ../csharp/HyperCast/runtimes/browser-wasm/nativeassets/net11.0/
cd ../csharp/HyperCast.WasmSmokeTest && dotnet publish -c Release -f net11.0 -o /tmp/hypercast-wasm
cd /tmp/hypercast-wasm/wwwroot && python3 -m http.server 5099   # then open http://localhost:5099/
```

The archive reaches the linker by the path the targets file names, and nowhere else. Restore
also resolves `runtimes/browser-wasm/nativeassets/` as a copy-local native asset, which on its
own would copy `libhypercast.a` into `bin/` and the publish root of every Blazor
WebAssembly consumer, never served and never read; the same targets file takes it back out of
the copy-local list, so a consumer's output carries only the linked `dotnet.native.wasm`.

That link happens whether or not the app ends up calling HyperCast. The targets file is
evaluated before the trimmer runs and cannot see what it removes, so a client bundle whose
trimmer drops `HyperCast.Managed` entirely still links the archive and all 27 entry points
into `dotnet.native.wasm`. One Blazor consumer measured the cost of that, for this
package and HyperUuid together, at 66 KB raw and 2 KB compressed.

**Needs the `wasm-tools` workload.** Only the workload's toolchain relinks the runtime.
Without it the SDK skips the link with nothing louder than its own warning, and the publish
succeeds with a `dotnet.native.wasm` that has no `cast_*` symbols in it; `NETSDK1147` does
not fire for this. So the targets file stops the build with error `HYPERCAST002` instead
(`dotnet workload install wasm-tools`; a `net10.0` app built with the .NET 11 SDK takes
`wasm-tools-net10`). An app that gates on `Cast.IsAvailable` and keeps a
managed fallback can set `<HyperCastRequireWasmWorkload>false</HyperCastRequireWasmWorkload>`.

**WebAssembly on .NET 10 and later, as linked.** The static library is built with `-Zbuild-std`
on a pinned nightly, for plain wasm (MVP plus `sign-ext` and `mutable-globals`), instead of on
Rust's prebuilt sysroot. So it holds no exception-handling instructions, and a browser never
sees .NET 11's encoding mixed with the sysroot's legacy one (`module uses a mix of legacy and
new exception handling instructions`), whether or not the SDK runs `wasm-opt` after the link.
And it uses no feature but those two, Binaryen's defaults, which every `wasm-opt` run in a .NET
10 publish accepts with no flag: Emscripten's does not know `bulk-memory-opt` or
`call-indirect-overlong` (dotnet/runtime#132858), and the SDK's own, after the link, rejects a
saturating float-to-int conversion. The archive is checked for both before it is packed.

**`Unknown option '--enable-bulk-memory-opt'` on a native relink** is .NET 10's WebAssembly
toolchain meeting a wasm object that declares `bulk-memory-opt` or `call-indirect-overlong`,
as everything rustc 1.87 or later builds on its prebuilt sysroot does
([dotnet/runtime#132858](https://github.com/dotnet/runtime/issues/132858), open as of
2026-10-28, reproduced on .NET 10.0.14). This package's archive declares neither; another
native library linked into the same app can still bring them in. .NET 11's toolchain relinks
either.

## Platform support

Native binaries ship inside the package for ten RIDs, plus static libraries for
WebAssembly, iOS and Mac Catalyst. Every row below is .NET 11 and later; with Windows, macOS,
iOS, Mac Catalyst and Android, that is every platform .NET MAUI targets. On .NET 10 the package
supports the Linux, macOS and Windows rows, framework-dependent or Native AOT, and Blazor
WebAssembly:

| Platform | RIDs | Native asset |
| --- | --- | --- |
| Linux (glibc) | `linux-x64`, `linux-arm64` | `libhypercast.so` |
| Linux (musl — Alpine) | `linux-musl-x64`, `linux-musl-arm64` | `libhypercast.so` |
| macOS | `osx-x64`, `osx-arm64` | `libhypercast.dylib` |
| Windows | `win-x64`, `win-arm64` | `hypercast.dll` |
| Blazor WebAssembly (.NET 10+) | `browser-wasm` | `libhypercast.a` (static — see above) |
| iOS | `ios-arm64`, `iossimulator-arm64` | `libhypercast.a` (static — see below) |
| Mac Catalyst | `maccatalyst-arm64`, `maccatalyst-x64` | `libhypercast.a` (static — see below) |
| Android (API 24+, .NET 11's floor) | `android-arm64`, `android-x64` | `libhypercast.so`; `libhypercast.a` for Native AOT (see below) |

**.NET 10 on iOS, Mac Catalyst or Android: `HYPERCAST003`.** These need .NET 11 or later. A
`net10.0-android`, `net10.0-ios` or `net10.0-maccatalyst` project stops at build with
`HYPERCAST003`, which names the platform and the framework to target. With the platform's
workload installed, it fires ahead of the Xcode and JDK checks. Without the workload, the SDK's
own missing-workload error (NETSDK1147) can come first; that case is not probed. A project
that reaches the package through a class library gets the same error.

**musl is its own build, not the glibc one relabeled.** A glibc `libhypercast.so` does not
load under musl's dynamic loader, and NuGet's RID graph falls back from `linux-musl-x64` to
`linux-x64` when nothing more specific is in the package, so without a musl build Alpine
would select the glibc library, fail to load it, and throw on the first cast. The
musl libraries are built inside Alpine itself and depend on nothing but musl libc. Proven
the way a consumer meets it, in an `mcr.microsoft.com/dotnet/sdk` Alpine container: this
binding's whole test suite, corpus replay included, and the Native AOT smoke test
(`-r linux-musl-x64`) against the musl library, then a throwaway console app consuming the
packed `.nupkg` through a plain `PackageReference` with the glibc *and* musl libraries both
inside it, which maps `runtimes/linux-musl-x64/native/libhypercast.so` and nothing else. The
same app against a glibc-only package is the control: `Cast.IsAvailable` is `false` there,
and nothing throws until something ignores it.

On any platform outside that table the package still restores and compiles — the managed
assembly is platform-neutral — and `Cast.IsAvailable` is how an app finds out at run time
that no native library came with it.

**iOS and Mac Catalyst: the core is linked into the app.** A .NET iOS, MAUI or Mac Catalyst
app (`net11.0-ios`, `net11.0-maccatalyst`) references the package like any other and writes
nothing else. Those platforms have no `runtimes/{rid}/native/` to load a library from: the
.NET SDK for them links native code into the app's own executable, and a P/Invoke reaches it
under the library name `__Internal`. So the package carries the core as a static library for
each of the four RIDs above, `build/net10.0/HyperCast.targets` hands the one for the RID being
built to the SDK as a
[`NativeReference` with `Kind=Static`](https://learn.microsoft.com/dotnet/maui/migration/ios-binding-projects),
and `Cast` declares every entry point a third time against `__Internal`, picked by
`OperatingSystem.IsIOS()` (which is true on Mac Catalyst too). It is the SDK's own native
link that takes the archive, so the same wiring serves an app on CoreCLR (the .NET 11
default for these platforms), one compiled by Mono's AOT compiler or run by its interpreter,
and one published with
[Native AOT](https://learn.microsoft.com/dotnet/core/deploying/native-aot/ios-like-platforms/);
the Native AOT wiring under [AOT](#aot) stands aside for these RIDs. A universal Mac Catalyst
app is built once per RID and merged, and each half links its own archive.

`HyperCast.AppleSmokeTest` is the Native AOT smoke test (`SmokeTest.cs`) as an app, crossing
every native entry point, and CI's `test-apple-mobile` job builds it three ways on a Mac from
that run's archives: as a Mac Catalyst app, run as a process; for the iOS simulator, installed
and launched; and for an iOS device with signing off, where the check is that the app's
executable defines the core's symbols, since no runner has a device to run it on.

The smoke test is a console-style program whose code touches no Apple API, which no real app
is, and a .NET 11 Release build keeps only what the app is seen to use. Left alone, the
trimmer drops the platform assembly the runtime calls into at startup, so on .NET 11's
default registrar, `trimmable-static`, the app aborts before `Main`
(`xamarin_bridge_call_runtime_initialize: failed to create delegate`), and the SDK leaves
UIKit, which the runtime's own library needs, out of the native link, so the link fails
first. `UIKitUse.cs` uses one UIKit symbol in a module initializer, as every real app uses
UIKit, and both go away: the test runs on the SDK's defaults.

**Android: the shared library, out of the APK.** A .NET for Android or MAUI app
(`net11.0-android`, API 24 and later, .NET 11's floor) references the package and writes
nothing else. On CoreCLR, .NET 11's Android runtime (Mono is not supported there; a `net10.0-android` build,
which would run on Mono, stops with `HYPERCAST003`), the
SDK takes `runtimes/android-arm64/native/libhypercast.so` and its x64 twin out of the package
and stores each in the APK under `lib/arm64-v8a/` and `lib/x86_64/`, and the ordinary
`"hypercast"` import opens it, exactly as on Linux. The libraries are cross-built with the
NDK for API level 21, below any app that can reference them, and their segments are aligned
to 16 KB: Android 15 devices may use 16 KB pages, a library aligned for 4 KB does not load on
one, and Google Play requires the alignment of every new app. A
Native AOT publish (`PublishAot`, `-r android-arm64`) takes the
[AOT](#aot) wiring instead, linking `staticlibs/android-{rid}/libhypercast.a` into the app's
own native library, and the shared one is left out of the APK. `android-arm64` covers
effectively every Android device in use and `android-x64` the emulator; these are the two
RIDs .NET for Android builds by default. The 32-bit `android-arm` and `android-x86` are not
in the package, so an app that adds them gets `Cast.IsAvailable == false` on those ABIs.

`HyperCast.AndroidSmokeTest` is the Native AOT smoke test's `SmokeTest.Run()` again, started from
an Activity, and unlike the other smoke tests it takes HyperCast as a package, from a local
folder CI packs it into, because the package's layout is what Android needs proven. CI's
`test-android` job builds it four ways from that run's libraries: CoreCLR and Native AOT,
for each RID. The x64 pair runs in an x86_64 emulator whose image uses 16 KB pages, so a
library aligned for 4 KB would fail to load there. Nothing hosted runs arm64 Android, so the
arm64 pair is inspected instead: each CoreCLR APK must carry `libhypercast.so` for its ABI,
each Native AOT APK must not carry it at all, and all four must pass `zipalign -P 16`.

**Known gap: tvOS and the iOS simulator on Intel Macs are not supported.**
`iossimulator-x64` and tvOS have no archive, so an app for them fails at its native link on
the undefined `cast_*` symbols, at build time and not at run time.

## Native binary provenance

The `.nupkg` carries compiled native code, which is a real thing to ask questions about
before adopting it inside a trust boundary. What is and isn't currently guaranteed:

**Where the binaries come from.** Nothing under `csharp/HyperCast/runtimes/` is committed —
it's `.gitignore`d. The native libraries are built from `rust/` by CI and staged into the
package at pack time, so what ships is produced by the same workflow run that built and
tested the source. (The Go, PHP, and Swift bindings are different: those *do* carry committed
binaries, staged by `stage-native-binaries.yml`, whose commit message records the exact
source SHA and CI run ID they came from — and which verifies each binary's attestation before
committing it.)

**Building it yourself.** The core is a normal Rust crate with no build-time codegen, so you
never have to take the shipped binary at all:

```shell
cd rust && cargo cdylib
# -> target/release/libhypercast.so  (.dylib on macOS, hypercast.dll on Windows)
```

Drop the result into `csharp/HyperCast/runtimes/<rid>/native/` and the package's own MSBuild
globs will pick it up, or point `dlopen` at it however you prefer — the C ABI in
`rust/src/ffi.rs` is the entire contract: the twenty-six `cast_*` functions and
`hypercast_version`, taking plain pointers into your own buffers. For local development
nothing needs dropping anywhere: when no library has been staged under `runtimes/` for your
machine's RID, the project copies `rust/target/release/` straight to the output, so
`dotnet test` after a `cargo cdylib` just runs.

**Reproducibility, stated honestly.** A rebuild is byte-identical only on the same toolchain
*and* the same build image. Measured on 2026-10-25 for linux-x64 at `0ee2caa`, whose Rust
sources are those of the library CI staged into `php/src/native/`: `cargo cdylib` in the
`rust:latest` Docker image (rustc 1.99.1) gives 160,488 bytes against CI's 160,512, with the
same exports and an identically sized `.text`. The difference is the C toolchain's version
string in `.comment`, GCC on CI's Ubuntu runner against the image's Debian. So "rebuild it
and compare hashes" verifies a binary only for someone who reproduces CI's runner image as
well as its toolchain. The mechanism that does work is a cryptographic attestation binding
each artifact to the workflow run and commit that produced it.

**Signed provenance.** Two kinds of attestation cover the package.

The *release attestation* is one statement, signed by this repository's `release.yml` at the release tag, whose subjects are every file the release ships — the `.nupkg` as packed and as nuget.org serves it, every other registry package, and every native library committed for Go, Swift and PHP — and whose predicate is the release inventory (`inventory.json`: every file's size, SHA-256 and the commit it was built from). One command verifies any of them from the tag alone:

```shell
gh attestation verify HyperCast.<version>.nupkg --repo SkunkWerkx/HyperCast \
  --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

*Build provenance* ([SLSA](https://github.com/actions/attest-build-provenance)) is the second layer, emitted where the bytes are made, which for this package is six places, because it is not the same bytes at every stage of its life:

| Attested artifact | Where | Signer |
| --- | --- | --- |
| Each native library, as built | `hyper-build-native.yml` | `SkunkWerkx/.github` |
| Each Android shared library, as built | `hyper-build-android.yml` | `SkunkWerkx/.github` |
| Each static library under `staticlibs/`, as built | `hyper-build-static.yml` | `SkunkWerkx/.github` |
| The browser-wasm archive, as built | `hyper-build-wasm.yml` | `SkunkWerkx/.github` |
| The `.nupkg` as packed, pre-push | `hyper-pack-nuget.yml` | `SkunkWerkx/.github` |
| The `.nupkg` as published | `release.yml`, after the push | `SkunkWerkx/HyperCast` |

The reason for the last two rows: **nuget.org adds its repository signature as a `.signature.p7s` entry inside the `.nupkg` zip during validation**, which changes the file's SHA-256. So the package you download is not the package that was built, and one attestation cannot cover both. The pipeline takes both, and the release attestation is made for both: once in the publication gate over the packed bytes, once after the push over the served bytes, with the same inventory as predicate. The signer column is the flag that changes: an artifact signed by a reusable workflow in `SkunkWerkx/.github` needs `--signer-repo SkunkWerkx/.github` beside `--repo` (or `--owner SkunkWerkx` in place of both); one signed by this repository's `release.yml` verifies with `--repo` alone. Get it wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which reads like a bad signature but is only an identity mismatch.

```shell
# the published bytes, signed by release.yml
gh attestation verify HyperCast.<version>.nupkg --repo SkunkWerkx/HyperCast

# the as-built package, recovered by removing nuget.org's one added entry; signed by the Forge
zip -d HyperCast.<version>.nupkg .signature.p7s
gh attestation verify HyperCast.<version>.nupkg \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
```

The release run downloads the package nuget.org serves, runs `dotnet nuget verify` on it, and requires every entry but `.signature.p7s` to match the packed package before attesting it. That claim is checked on every release rather than asserted here, so if nuget.org ever changes how it finalizes packages, the run says so instead of the README quietly going stale.

Attestations are produced by every release-mode build — the release's own run and the weekly one. Pull requests build in `pr` mode, which ships nothing and so signs nothing. The release itself is two acts at one tag: the tag push packs, attests and preserves everything without uploading, and a dispatch of `release.yml` publishes from those preserved bytes, waiting up to 20 minutes for nuget.org to serve the package before attesting the served bytes; the Go tag and GitHub Release wait on it. A slower validation is recovered by dispatching again with `publication_run_id`, which pushes nothing twice (see [RELEASING.md](https://github.com/SkunkWerkx/HyperCast/blob/master/.github/RELEASING.md)). The root README's [Provenance](https://github.com/SkunkWerkx/HyperCast/blob/master/README.md#provenance) section has the whole picture across the eight bindings.

**Not currently done: NuGet author signing.** The package carries nuget.org's repository
signature but no author signature of our own, which would need an X.509 code-signing
certificate registered to the account. It's complementary rather than a substitute, and the
difference is who does the checking: an author signature is verified automatically by every
consumer's SDK at restore time, whereas an attestation is only checked by someone who
deliberately runs `gh attestation verify`. Attestation ties an artifact to a commit and a
build; an author signature ties it to an identity. If you want the automatic restore-time
check, this is the gap.

**Per-platform AOT receipts.** The same CI run publishes `HyperCast.AotSmokeTest` under
Native AOT on five desktop RIDs (every one but `osx-x64`, which is cross-built and has no CI leg) and, in Alpine containers, on the two musl ones, fails the build on any `ILxxxx`/`AOTxxxx` trim diagnostic,
executes the resulting binary, and requires exit 0. Each leg's log uploads as an
`aot-report-{rid}` artifact.

## Interop: building on HyperCast's C ABI

For a library that carries HyperCast's verdicts across a C ABI of its own — HyperTabular
does — and so reads the core's out-values, numeric format, verdict codes and
fault spans out of its own buffers. The `HyperCast.Interop` namespace is the code the `Cast`
doors themselves use, so a value read that way is the value the door would have returned.

- `RawTimestamp`, `RawDate`, `RawCivil`, `RawDuration`, `RawDecimal` — the core's
  `#[repr(C)]` out-value layouts, each with its door's conversion: `ToDateTimeOffset()`,
  `ToDateOnly()`, `ToDateTime()` (`DateTimeKind.Unspecified`), `ToTimeSpan()`,
  `ToDecimal()`; sub-tick nanoseconds truncate exactly as the doors truncate them.
- `RawFault` — a fault's byte span, `Offset` and `Length`.
- `RawNumFormat` — the core's 92-byte numeric format: a size word, the `NumStyles` bits
  and the native digits' zero, then every declared symbol in place, each its UTF-8 bytes
  and then its length (`RawSymbol`, nine bytes, for the separators, the signs and the
  percent symbol; `RawWideSymbol`, seventeen, for the currency symbol). The core reads it
  where it stands. `NumFormat.ToRaw()` builds it, validating as every numeric door does
  (`ArgumentException` on a caller bug).
- `RawDayPeriods` — the core's 40-byte day-period layout: a size word, then the AM and PM
  designators as `RawWideSymbol`s. `DateFormat.ToRaw()` builds it; the civil date-time door
  passes null in its place when nothing is declared.
- `Abi` — `Code(UnixPrecision)`, `Code(DateOrder)` and `Code(ExcelEpoch)`, checked
  (`ArgumentOutOfRangeException` for an undefined value), and the way back as
  `UnixPrecisionFrom`, `DateOrderFrom`, `ExcelEpochFrom` and `ReasonFrom`, each `null` for a
  code it does not name; `ToFault(code, RawFault)`; `ToTimeOnly(nanosOfDay)` and
  `ToGuid(rfc9562)` for the time and UUID out-values; `ToVersion(packed)` for a
  `*_version()` word, as a `System.Version`; and `ProbeVersion(Func<uint>)`, the probe
  `Cast.IsAvailable` and `Cast.NativeVersion` rest on — the same `System.Version`, or `null`
  when the library did not load — for any library's version export.

## Install

Published to [nuget.org](https://www.nuget.org/packages/HyperCast) — no extra package source
needed:

```shell
dotnet add package HyperCast
```

Per-RID native libraries ship inside the package under `runtimes/`, so a consumer adds one
reference and nothing else — no build step, no manual native staging. See
[Platform support](#platform-support) for the list.

Targets net10.0 and net11.0. On .NET 10: Windows, Linux and macOS, framework-dependent or
Native AOT, and Blazor WebAssembly. On .NET 11 and later: those, plus iOS, Mac Catalyst and
Android — see [Platform support](#platform-support).

See [the repo root README](https://github.com/SkunkWerkx/HyperCast/blob/master/README.md)
for the full door table, the receipts, and the state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
