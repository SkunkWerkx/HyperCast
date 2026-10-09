# HyperCast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![NuGet](https://img.shields.io/nuget/v/HyperCast.svg)](https://www.nuget.org/packages/HyperCast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**`TryParse` hands back a `bool` and a shrug. These doors hand back a native discriminated
union — the value, or `Empty`/`Malformed`/`OutOfRange` plus the exact span that offended, in
the units of the input you passed — and an unhandled case is a compile error, not a review
nit.**

Allocation-free scalar casts — booleans, the full integer family, reals, an exact
`decimal`, UUIDs, temporals — as source-generated `[LibraryImport]` P/Invoke straight into
the native `libhypercast` Rust core. No runtime bridge, no reflection anywhere in the
assembly. .NET 11 is the floor deliberately: `Verdict<T>` is a real `[Union]`, and CS8509
(non-exhaustive switch) is elevated to an error, so a missing disposition fails the build —
the entire point of returning a union instead of throwing.

```csharp
using System.Globalization;
using HyperCast;

var culture = CultureInfo.GetCultureInfo("en-US");
var message = Cast.Int32("(1,234)", NumFormat.From(culture)) switch
{
    Success<int> s => $"got {s.Value}",                  // -1234, accounting negative
    Fault f => $"{f.Reason} at char {f.Offset}",         // no third case: the compiler checked
};
```

Door names mirror the native ABI (`Int32`, `Double`, `Decimal`, `Timestamp`, …) so the
polyglot surface reads identically across bindings; `Cast.Numeric<T>` fronts all eleven
numeric doors for a caller that is itself generic over the target, and `Cast.Scalar<T>`
fronts every text door (see [One door for a generic `T`](#one-door-for-a-generic-t)). Culture never lives in
the core — `NumFormat.From(CultureInfo)` (or `From(IFormatProvider)`, the shape every BCL
`TryParse` already takes) bridges .NET's culture machinery to the caller-declared format
the native side actually reads: separators, lenience flags, and the culture's currency
symbol. Every numeric door, `Numeric<T>` and `Scalar<T>` also take that `IFormatProvider?`
directly, as `TryParse` does (see [A culture, the way the BCL takes one](#a-culture-the-way-the-bcl-takes-one)).
Or spell the format directly — `new NumFormat(',', '\u00A0', NumStyles.All, "€")`
declares an arbitrary pair, and `NumStyles.None` turns every lenience off. .NET-flavored
fidelity, stated honestly: `DateTimeOffset`/`TimeOnly`/`TimeSpan` resolve to 100 ns
ticks, so sub-tick nanoseconds truncate (the core carries full nanosecond fidelity; .NET's
clock types don't). `Cast.Decimal` is exact and canonical — sign, 96-bit magnitude, trailing
fraction zeros trimmed, never rounded. A `Fault`'s span is in the caller's own units: byte
offsets from the `ReadOnlySpan<byte>` doors, char offsets from the
`string`/`ReadOnlySpan<char>` ones, so slicing the offending text back out of what you
passed needs no mapping on either side.

Before the first cast, `Cast.IsAvailable` says whether the native library resolved and
`Cast.NativeVersion` names the core it loaded — the probe a consumer with a managed fallback
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
`Numeric<T>` and `Scalar<T>`, for `ReadOnlySpan<char>` and UTF-8 input alike — has an
overload taking the `IFormatProvider?` a BCL `TryParse` or `ISpanParsable<T>` takes, so code
that is already provider-shaped passes what it was handed straight through:

```csharp
// A parsing gateway in ISpanParsable's shape, routing through HyperCast in one call.
static T ParseRequired<T>(ReadOnlySpan<char> text, IFormatProvider? provider) where T : struct =>
    Cast.Scalar<T>(text, provider) switch
    {
        Success<T> s => s.Value,
        Fault f => throw new FormatException($"{f.Reason} at {f.Offset}+{f.Length}"),
    };

Cast.Double("1.234,5", CultureInfo.GetCultureInfo("de-DE"));   // 1234.5
Cast.Int32("1.234", null);                                       // under the current culture
```

The provider maps through `NumFormat.From(IFormatProvider)`: the culture's decimal and group
separators and its currency symbol, with every lenience on. `null` means the current
culture, exactly as it does for `TryParse` (`NumberFormatInfo.GetInstance(null)`), so the
overloads are a drop-in where a BCL call stood; pass a `NumFormat` for anything stricter, or
for a format that must not follow the machine. `Scalar<T>` reads the provider only for the
numeric targets. The mapping allocates nothing and costs nothing measurable beside the door
itself (77 ns either way for a grouped `Int32` on the machine that measured it).

`default` as the second argument still means `default(NumFormat)`, the equal-separators
caller bug it always was: the provider overloads carry `[OverloadResolutionPriority(-1)]`,
so they are chosen only for an argument a `NumFormat` cannot be — `null`, a `CultureInfo`, a
`NumberFormatInfo`.

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
   protobuf JSON durations, a declared currency symbol at either edge — much of it grammar
   the BCL has no knob for at any price.
3. **One engine across a polyglot system** — the same Rust core, bit-for-bit verdicts,
   proven by the shared conformance corpus every binding replays: here, all thirteen files
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
   | `Cast.Decimal` (`($1,234.50)`, en-US `$`) vs `decimal.TryParse` `NumberStyles.Currency` | 64.1 ns | 58.4 ns | 1.1x slower |
   | `Cast.Double` (same text, same format) vs `double.TryParse` `NumberStyles.Currency` | 60.9 ns | 57.4 ns | 1.06x slower |

   Grouping, a declared symbol and accounting parentheses are read in one pass by the
   core's lenient fast lane (`rust/src/lane.rs`): in the Rust suite a `cast_decimal` of
   `$12,345.67` costs 18.1 ns against 13.6 ns plain. The same lane is why the grouped
   `Cast.Int32` row and the eurozone `Cast.Double` row are wins. What the currency rows
   pay is the crossing and the transcode, against a BCL parser that has neither.

   **Separator detection is nearly free**: `NumFormat.Detect` on `1.234.567,89` costs
   54.6 ns against 47.4 ns for the same text under a declared eurozone format — ~7 ns for
   resolving the `.`/`,` roles structurally instead of being told them.

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
  -c Release -r linux-x64 -p:PublishAot=true
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
unaffected: it cannot link an archive, and loads the shared library as before.

## WebAssembly (Blazor)

One compiled assembly covers browser-wasm too — every native entry point is declared three
times (`"hypercast"` for dlopen platforms, `"*"` for the statically-linked wasm module, and
`"__Internal"` for iOS and Mac Catalyst; see [Platform support](#platform-support)), sharing
the same `EntryPoint`, with `OperatingSystem.IsBrowser()` and `OperatingSystem.IsIOS()`
picked at the call site and constant-folded by the linker. CI builds the `wasm32-unknown-emscripten` staticlib on every
PR; the release pack stages it under `runtimes/browser-wasm/nativeassets/`, and
`build/net11.0/HyperCast.targets` ships inside the package to wire it up for a consumer with
no configuration at all.

That targets file is load-bearing, and both halves of it are: a `NativeFileReference` hands
the staticlib to the linker (restore never populates `@(NativeLibrary)` from a plain
`PackageReference`'s `nativeassets/` folder the way it does `runtimes/{rid}/native/`), and an
`EmccExportedFunction` per door makes the linked-in symbols resolvable through
`LibraryImport("*")` at runtime — the WASM SDK exports only its own baseline set and never
scans P/Invoke declarations to find the rest. v0.0.1 shipped without that file, and a real
Blazor consumer's publish died at `wasm-ld` with `undefined symbol: cast_i32`.

**Target frameworks.** One floor here, not two: the package targets net11.0 and nothing
older, and the targets file sits under `build/net11.0/` (and `buildTransitive/net11.0/`) like
the assembly under `lib/net11.0/`. With every framework-specific asset in a net11.0 folder,
NuGet refuses the package to an older project at restore, with NU1202 ("not compatible with
net10.0"), so the file needs no target-framework gate of its own. (HyperUuid's package targets
net10.0 for its native platforms, so its copy sits in `build/` and has one.)

`HyperCast.WasmSmokeTest` proves the whole chain in a real browser: a Blazor WebAssembly app
that imports that targets file, calls every native entry point — the twenty-six `cast_*`
functions and `hypercast_version` — through the public `Cast` surface, and renders `PASS` or
`FAIL` into the page. Every one, because that is the only way the check means what it says:
a door missing from the `EmccExportedFunction` list links fine and fails only when called.
`./check.sh` in that directory stages the wasm static library, publishes the app, loads it in
headless Chromium and requires `PASS`. CI runs the same script on every PR, in headless Chrome
with the `wasm-tools` workload, against the static library that run just built (`STATICLIB`
names it, so the script skips building its own); it is not part of the solution, so a plain
`dotnet build` never needs the workload or a browser. By hand, the same steps are:

```shell
cd rust && cargo wasm-staticlib
mkdir -p ../csharp/HyperCast/runtimes/browser-wasm/nativeassets/net10.0
cp target/wasm32-unknown-emscripten/release/libhypercast.a ../csharp/HyperCast/runtimes/browser-wasm/nativeassets/net10.0/
cd ../csharp/HyperCast.WasmSmokeTest && dotnet publish -c Release -o /tmp/hypercast-wasm
cd /tmp/hypercast-wasm/wwwroot && python3 -m http.server 5099   # then open http://localhost:5099/
```

The archive reaches the linker by the path the targets file names, and nowhere else. Restore
also resolves `runtimes/browser-wasm/nativeassets/` as a copy-local native asset, which on its
own would copy the 3 MB `libhypercast.a` into `bin/` and the publish root of every Blazor
WebAssembly consumer, never served and never read; the same targets file takes it back out of
the copy-local list, so a consumer's output carries only the linked `dotnet.native.wasm`.

**WebAssembly is .NET 11 and later only.** .NET 11 links browser-wasm with the new (exnref)
exception-handling encoding, while Rust's precompiled `core` inside the static library uses
the legacy one, and the browser refuses a module that mixes them (`module uses a
mix of legacy and new exception handling instructions`). The same `HyperCast.targets`
therefore appends Binaryen's translate-to-exnref pass to the SDK's post-link `wasm-opt`, with
no action needed from a consumer.

## Platform support

Native binaries ship inside the package for ten RIDs, plus static libraries for
WebAssembly, iOS and Mac Catalyst. With Windows, macOS, iOS, Mac Catalyst and Android, that
is every platform .NET MAUI targets:

| Platform | RIDs | Native asset |
| --- | --- | --- |
| Linux (glibc) | `linux-x64`, `linux-arm64` | `libhypercast.so` |
| Linux (musl — Alpine) | `linux-musl-x64`, `linux-musl-arm64` | `libhypercast.so` |
| macOS | `osx-x64`, `osx-arm64` | `libhypercast.dylib` |
| Windows | `win-x64`, `win-arm64` | `hypercast.dll` |
| Blazor WebAssembly (.NET 11+) | `browser-wasm` | `libhypercast.a` (static — see above) |
| iOS | `ios-arm64`, `iossimulator-arm64` | `libhypercast.a` (static — see below) |
| Mac Catalyst | `maccatalyst-arm64`, `maccatalyst-x64` | `libhypercast.a` (static — see below) |
| Android (API 21+) | `android-arm64`, `android-x64` | `libhypercast.so`; `libhypercast.a` for Native AOT (see below) |

**musl is its own build, not the glibc one relabeled.** A glibc `libhypercast.so` does not
load under musl's dynamic loader, and NuGet's RID graph falls back from `linux-musl-x64` to
`linux-x64` when nothing more specific is in the package — which is what 0.3.0 and earlier
did on Alpine: the glibc library was selected, failed to load, and the first cast threw. The
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
each of the four RIDs above, `build/net11.0/HyperCast.targets` hands the one for the RID being
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

Two .NET 11 RC1 SDK behaviors shape that project, and an app on RC1 may meet them too; neither
is HyperCast's. A Release build links only the frameworks the trimmer sees the app use, while
the SDK's own runtime library still needs UIKit, so an app that touches no UIKit type fails
its native link; the smoke test, a console-style program, asks for UIKit by hand. And a
Release Mac Catalyst build on RC1's new default registrar, `trimmable-static`, aborted in CI
before `Main` (`xamarin_bridge_call_runtime_initialize: failed to create delegate`); the
smoke test sets `Registrar` to `managed-static` there, the default before RC1.

**Android: the shared library, out of the APK.** A .NET for Android or MAUI app
(`net11.0-android`, API 24 and later, .NET 11's floor) references the package and writes
nothing else. On CoreCLR, .NET 11's Android runtime (Mono is no longer supported there), the
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

Two .NET 11 RC1 workload behaviors shape that job, and an app on RC1 may meet them too. The
android workload's build tasks require JDK 21 (`XA0030` on newer). And the workload RC1
resolves was built against a runtime newer than RC1 on nuget.org, so a Native AOT publish
fails to restore `Microsoft.NETCore.App.Runtime.NativeAOT.android-*` by exact version until
the .NET 11 daily feed (`https://pkgs.dev.azure.com/dnceng/public/_packaging/dotnet11/nuget/v3/index.json`)
is added as a source; the job adds it.

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

**Reproducibility, stated honestly.** A Rust build is deterministic *locally* but not
bit-reproducible *across machines* — differing toolchain versions and embedded build paths
change the hash — so "rebuild it and compare hashes" is not a verification path a consumer
can rely on. The mechanism that does work is a cryptographic attestation binding each
artifact to the workflow run and commit that produced it.

**Signed provenance.** CI emits [SLSA build provenance](https://github.com/actions/attest-build-provenance)
at three points, because the package is not the same bytes at every stage of its life:

| Attested artifact | Where | How to verify |
| --- | --- | --- |
| Each native library, as built | `hyper-build-native.yml` | `gh attestation verify libhypercast.so --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github` |
| The `.nupkg` as packed, pre-push | `hyper-pack-nuget.yml` | strip the repo signature first (below) |
| The `.nupkg` as published | `release.yml`, after the push | verify the downloaded file directly |

The reason for the last two rows: **nuget.org adds its repository signature as a
`.signature.p7s` entry inside the `.nupkg` zip during validation**, which changes the file's
SHA-256. So the package you download is not the package that was built, and one attestation
cannot cover both. Rather than pick, the pipeline takes both — and because the mutation is
exactly one added zip entry, the pre-push attestation stays recoverable:

```shell
# verify the published bytes directly — nothing to undo.
# Signed by release.yml, which lives in this repo, so no --signer-repo is needed.
gh attestation verify HyperCast.X.Y.Z.nupkg --repo SkunkWerkx/HyperCast

# or recover the as-built artifact and verify that instead.
# Signed by hyper-pack-nuget.yml over in the forge repo, so this half needs --signer-repo.
zip -d HyperCast.X.Y.Z.nupkg .signature.p7s
gh attestation verify HyperCast.X.Y.Z.nupkg \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
```

**Why `--signer-repo` appears on some of these and not others.** `--repo X` asserts two
separate things: that the artifact came from repo X, and that the workflow which signed it
also lives in X. Everything CI builds here comes from this repo, so the first half always
holds — but the signing step's location varies. Anything signed inside a reusable workflow
(`hyper-build-native.yml`, `hyper-pack-nuget.yml`, `hyper-publish-crate.yml`,
`hyper-publish-maven.yml`) is signed by a file that physically lives in `SkunkWerkx/.github`,
and that is what Fulcio records as the build signer; anything signed directly by this repo's
own `release.yml` is signed by this repo. Get it wrong and `gh` reports
`verifying with issuer "sigstore.dev"` with no further detail, which reads like a bad
signature but is only an identity mismatch. `--owner SkunkWerkx` works for every row above if
you would rather not track which is which.

The release run's job summary prints all three digests — as packed, as published, and as
published-with-the-signature-removed — and asserts that the third equals the first. That
claim is checked on every release rather than asserted here, so if nuget.org ever changes how
it finalizes packages, the run says so instead of this README quietly going stale.

Attestations are produced by every release-mode build — the release's own run and the
weekly one. Pull requests build in `pr` mode, which ships nothing and so signs nothing. The post-publish half
is non-blocking: the push is irreversible, so a slow nuget.org validation is never allowed to
turn a successful publish into a failed release.

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
- `RawNumFormat` — the core's 32-byte numeric format: four `uint`s, then the currency
  symbol's UTF-8 inline in a `RawCurrency`. `NumFormat.ToRaw()` builds it, validating as
  every numeric door does (`ArgumentException` on a caller bug).
- `Abi` — `Code(UnixPrecision)`, `Code(DateOrder)` and `Code(ExcelEpoch)`, checked
  (`ArgumentOutOfRangeException` for an undefined value), and the way back as
  `UnixPrecisionFrom`, `DateOrderFrom`, `ExcelEpochFrom` and `ReasonFrom`, each `null` for a
  code it does not name; `ToFault(code, RawFault)`; `ToTimeOnly(nanosOfDay)` and
  `ToGuid(rfc9562)` for the time and UUID out-values; `ToVersion(packed)` for a
  `*_version()` word; and `ProbeVersion(Func<uint>)`, the probe `Cast.IsAvailable` and
  `Cast.NativeVersion` rest on — `null` when the library did not load — for any library's
  version export.

## Install

Published to [nuget.org](https://www.nuget.org/packages/HyperCast) — no extra package source
needed:

```shell
dotnet add package HyperCast
```

Per-RID native libraries ship inside the package under `runtimes/`, so a consumer adds one
reference and nothing else — no build step, no manual native staging. See
[Platform support](#platform-support) for the list.

Targets net11.0, on every platform including Blazor WebAssembly — see
[WebAssembly (Blazor)](#webassembly-blazor).

See [the repo root README](https://github.com/SkunkWerkx/HyperCast/blob/master/README.md)
for the full door table, the receipts, and the state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
