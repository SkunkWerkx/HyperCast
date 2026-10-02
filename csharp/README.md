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
numeric doors for a caller that is itself generic over the target. Culture never lives in
the core — `NumFormat.From(CultureInfo)` (or `From(IFormatProvider)`, the shape every BCL
`TryParse` already takes) bridges .NET's culture machinery to the caller-declared format
the native side actually reads: separators, lenience flags, and the culture's currency
symbol. Or spell the format directly — `new NumFormat(',', '\u00A0', NumStyles.All, "€")`
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
   | `Cast.DateTime` (`1/7/2026 3:04 PM`) vs `DateTime.TryParse` (en-US) | 48.2 ns | 186.0 ns | **3.9x faster** |
   | `Cast.Timestamp` vs `DateTimeOffset.TryParse` | 52.5 ns | 128.3 ns | **2.4x faster** |
   | `Cast.Duration` vs `TimeSpan.TryParse` | 49.0 ns | 118.7 ns | **2.4x faster** |
   | `Cast.Date` (declared order) vs `DateOnly.TryParse` (en-US) | 41.5 ns | 91.6 ns | **2.2x faster** |
   | `Cast.Int32` (grouped) vs `int.TryParse` | 46.2 ns | 48.7 ns | wash |
   | `Cast.Double` vs `double.TryParse` | 51.9 ns | 49.5 ns | wash |
   | `Cast.Double` (eurozone) vs `double.TryParse` (de-DE) | 66.8 ns | 64.4 ns | wash |
   | `Cast.Uuid` vs `Guid.TryParse` | 40.3 ns | 27.4 ns | 1.5x slower — while also taking N/B/P/X and `urn:uuid:` |
   | `Cast.Boolean` vs `bool.TryParse` | 21.2 ns | JIT-folded | honest loss — the twenty-lexeme vocabulary is why anyone calls this door |

   Reproduce: `dotnet run -c Release --project csharp/HyperCast.Benchmarks`, from the repo
   root.

   Every row above is the `string` door, transcode included. The `ReadOnlySpan<byte>`
   doors are the primary surface — a caller holding UTF-8 already (a file, a wire buffer,
   one field of a delimited line) never pays that transcode — so they are measured on
   their own, same machine, same run:

   | Door (UTF-8 in hand) | HyperCast | `string` door | BCL, same run |
   | --- | ---: | ---: | ---: |
   | `Cast.Timestamp` | **41.9 ns** | 52.5 ns | 128.3 ns `DateTimeOffset.TryParse` |
   | `Cast.Int32` (grouped) | **28.0 ns** | 46.2 ns | 48.7 ns `int.TryParse` — the wash becomes a win |
   | `Cast.Double` | **35.9 ns** | 51.9 ns | 49.5 ns `double.TryParse` — and so does this one |
   | `Cast.Uuid` | 29.8 ns | 40.3 ns | 27.4 ns `Guid.TryParse` — still a loss, by 2.4 ns |

   The UTF-16 doors try the stack buffer first and rent from the pool only when the
   encoder says the text did not fit, rather than sizing by the 3-bytes-per-char worst
   case — which would send any text past ~170 chars to the pool even when it is plain
   ASCII that fits with room to spare.

   The two doors the first consumer asked for, same run, string doors with the transcode
   included, invariant unless stated — printed as measured, because two of the three rows
   are losses:

   | Door | HyperCast | BCL | Verdict |
   | --- | ---: | ---: | --- |
   | `Cast.Decimal` (`12,345.6789`) vs `decimal.TryParse` | 55.3 ns | 60.2 ns | 1.1x faster — and exact, canonical, never rounded |
   | `Cast.Decimal` (`($1,234.50)`, en-US `$`) vs `decimal.TryParse` `NumberStyles.Currency` | 66.9 ns | 59.4 ns | 1.1x slower |
   | `Cast.Double` (same text, same format) vs `double.TryParse` `NumberStyles.Currency` | 74.8 ns | 58.9 ns | 1.3x slower |

   Grouping, a declared symbol and accounting parentheses are read in one pass by the
   core's lenient fast lane (`rust/src/lane.rs`): in the Rust suite a `cast_decimal` of
   `$12,345.67` costs 20.6 ns against 14.1 ns plain. The same lane is why the grouped
   `Cast.Int32` row and the eurozone `Cast.Double` row are washes. What the currency rows
   pay is the crossing and the transcode, against a BCL parser that has neither.

   **Separator detection is nearly free**: `NumFormat.Detect` on `1.234.567,89` costs
   75.7 ns against 66.8 ns for the same text under a declared eurozone format — ~9 ns for
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
twenty-one `cast_*` functions and `hypercast_version` — plus the generic `Cast.Numeric<T>`
door, a UTF-8 door, and the union's exhaustive two-arm `switch`, and returns a nonzero exit
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

One compiled assembly covers browser-wasm too — every native entry point is declared twice
(`"hypercast"` for dlopen platforms, `"*"` for the statically-linked wasm module), sharing
the same `EntryPoint`, with `OperatingSystem.IsBrowser()` picked at the call site and
constant-folded by the linker. CI builds the `wasm32-unknown-emscripten` staticlib on every
PR; the release pack stages it under `runtimes/browser-wasm/nativeassets/`, and
`build/HyperCast.targets` ships inside the package to wire it up for a consumer with
no configuration at all.

That targets file is load-bearing, and both halves of it are: a `NativeFileReference` hands
the staticlib to the linker (restore never populates `@(NativeLibrary)` from a plain
`PackageReference`'s `nativeassets/` folder the way it does `runtimes/{rid}/native/`), and an
`EmccExportedFunction` per door makes the linked-in symbols resolvable through
`LibraryImport("*")` at runtime — the WASM SDK exports only its own baseline set and never
scans P/Invoke declarations to find the rest. v0.0.1 shipped without that file, and a real
Blazor consumer's publish died at `wasm-ld` with `undefined symbol: cast_i32`.

**Target frameworks.** One floor here, not two: the package targets net11.0 and nothing
older, so NuGet never imports that targets file into a project that predates the .NET 11
WebAssembly toolchain, and it carries no target-framework gate. (HyperUuid's package targets
net10.0 for its native platforms, so its copy of the file has one.)

`HyperCast.WasmSmokeTest` proves the whole chain in a real browser: a Blazor WebAssembly app
that imports that targets file, calls every native entry point — the twenty-one `cast_*`
functions and `hypercast_version` — through the public `Cast` surface, and renders `PASS` or
`FAIL` into the page. Every one, because that is the only way the check means what it says:
a door missing from the `EmccExportedFunction` list links fine and fails only when called.
It is a local check, not wired into the solution or CI, because it needs the `wasm-tools`
workload and a browser. To run it:

```shell
cd rust && cargo wasm-staticlib
mkdir -p ../csharp/HyperCast/runtimes/browser-wasm/nativeassets/net10.0
cp target/wasm32-unknown-emscripten/release/libhypercast.a ../csharp/HyperCast/runtimes/browser-wasm/nativeassets/net10.0/
cd ../csharp/HyperCast.WasmSmokeTest && dotnet publish -c Release -o /tmp/hypercast-wasm
cd /tmp/hypercast-wasm/wwwroot && python3 -m http.server 5099   # then open http://localhost:5099/
```

`check.sh` in that directory does all of it, including the headless-Chromium assertion.

**WebAssembly is .NET 11 and later only.** .NET 11 links browser-wasm with the new (exnref)
exception-handling encoding, while the precompiled Rust standard library inside the static
library uses the legacy one, and the browser refuses a module that mixes them (`module uses a
mix of legacy and new exception handling instructions`). The same `HyperCast.targets`
therefore appends Binaryen's translate-to-exnref pass to the SDK's post-link `wasm-opt`, with
no action needed from a consumer.

## Platform support

Native binaries ship inside the package for eight RIDs, plus a WebAssembly static library:

| Platform | RIDs | Native asset |
| --- | --- | --- |
| Linux (glibc) | `linux-x64`, `linux-arm64` | `libhypercast.so` |
| Linux (musl — Alpine) | `linux-musl-x64`, `linux-musl-arm64` | `libhypercast.so` |
| macOS | `osx-x64`, `osx-arm64` | `libhypercast.dylib` |
| Windows | `win-x64`, `win-arm64` | `hypercast.dll` |
| Blazor WebAssembly (.NET 11+) | `browser-wasm` | `libhypercast.a` (static — see above) |

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

**Known gap: iOS, Mac Catalyst, and Android are not supported.** A .NET MAUI app can reference
this package for its Windows and macOS heads, which the RIDs above cover, but not for its mobile
heads — the package neither ships those native assets nor declares those target frameworks. Stated
here as an explicit gap rather than left for a consumer to discover at link time.

Android is the smaller half: it needs an NDK cross-build added to the release matrix, but
resolution is then ordinary `dlopen` of a `.so` out of `runtimes/{rid}/native/`, exactly like the
Linux RIDs already do.

Apple mobile is a packaging change, not a matrix row.
[Native AOT for iOS-like platforms](https://learn.microsoft.com/dotnet/core/deploying/native-aot/ios-like-platforms/)
(.NET 9+) does cover `ios-arm64`, `iossimulator-arm64`/`-x64` and `maccatalyst-arm64`/`-x64` — but a
native dependency on those targets is linked statically into the app, via
[`NativeReference` with `Kind=Static`](https://learn.microsoft.com/dotnet/maui/migration/ios-binding-projects)
or Native AOT's
[`NativeLibrary`/`DirectPInvoke`](https://learn.microsoft.com/dotnet/core/deploying/native-aot/interop),
rather than resolved at runtime from `runtimes/{rid}/native/`. That is structurally the same problem
the WebAssembly support above already solves: build the Rust core as a `.a` rather than a shared
library, and let this package's own auto-imported `build/HyperCast.targets` inject the
reference so a consumer still writes nothing but a `PackageReference`. The packaging mechanism is
therefore already proven in this repo; what is *not* yet established is how the managed
`LibraryImport` declaration should resolve against a statically-linked core on iOS, which is the
first thing to settle whenever this is picked up.

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
`rust/src/ffi.rs` is the entire contract: the twenty-one `cast_*` functions and
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

Attestations are produced on pushes, releases, and same-repo pull requests. Only pull
requests *from forks* go unattested, because a fork's token can't sign. The post-publish half
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
