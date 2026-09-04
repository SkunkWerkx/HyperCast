# HyperCast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![NuGet](https://img.shields.io/nuget/v/HyperCast.svg)](https://www.nuget.org/packages/HyperCast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**`TryParse` hands back a `bool` and a shrug. These doors hand back a native discriminated
union — the value, or `Empty`/`Malformed`/`OutOfRange` plus the exact byte span that
offended — and an unhandled case is a compile error, not a review nit.**

Allocation-free scalar casts — booleans, the full integer family, reals, an exact
`decimal`, UUIDs, temporals — as source-generated `[LibraryImport]` P/Invoke straight into
the native `libhypercast` Rust core. No runtime bridge, no reflection anywhere in the assembly. .NET 11 is the floor
deliberately: `Verdict<T>` is a real `[Union]`, and CS8509 (non-exhaustive switch) is
elevated to an error, so a missing disposition fails the build — the entire point of
returning a union instead of throwing.

```csharp
var message = Cast.Int32("(1,234)", NumFormat.From(culture)) switch
{
    Success<int> s => $"got {s.Value}",                  // -1234, accounting negative
    Fault f => $"{f.Reason} at byte {f.Offset}",         // no third case: the compiler checked
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
fraction zeros trimmed, never rounded — and `Fault` spans on the `string`/`ReadOnlySpan<char>` doors are
char offsets, so slicing the offending text back out needs no mapping.

Before the first cast, `Cast.IsAvailable` says whether the native library resolved and
`Cast.NativeVersion` names the core it loaded — the probe a consumer with a managed fallback
gates on, instead of catching `DllNotFoundException` around its first real call.

## Why not the BCL's own `TryParse` family?

1. **The error story is data, not archaeology** — a closed reason plus the offending span,
   against the BCL's bare `false`.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, radix prefixes, all five `Guid` formats *plus* `urn:uuid:` prefixes,
   protobuf JSON durations, a declared currency symbol at either edge — much of it grammar
   the BCL has no knob for at any price.
3. **One engine across a polyglot system** — the same Rust core, bit-for-bit verdicts,
   proven by the shared conformance corpus every binding replays (all 28 of this binding's
   tests include the full thirteen-file corpus through real P/Invoke). The corpus also
   ships as the `HyperCast.Corpus` content package, versioned in lockstep, for a downstream
   suite that routes its own parsers through these doors.
4. **Not slower — mostly faster.** BenchmarkDotNet, `[MemoryDiagnoser]`, lenience matched
   where the BCL has the knob, FFI crossing and UTF-16→UTF-8 transcode *included* in every
   HyperCast number; zero managed allocation on every row, both sides (linux-arm64,
   .NET 11 preview):

   | Door | HyperCast | BCL | Verdict |
   | --- | ---: | ---: | --- |
   | `Cast.Timestamp` vs `DateTimeOffset.TryParse` | 71.0 ns | 285.6 ns | **4.0x faster** |
   | `Cast.Duration` vs `TimeSpan.TryParse` | 64.1 ns | 143.0 ns | **2.2x faster** |
   | `Cast.Double` vs `double.TryParse` | 50.4 ns | 69.8 ns | **1.4x faster** |
   | `Cast.Uuid` vs `Guid.TryParse` | 54.8 ns | 51.9 ns | wash — while also taking N/B/P/X and `urn:uuid:` |
   | `Cast.Int32` (grouped) vs `int.TryParse` | 64.5 ns | 54.0 ns | 1.2x slower — the crossing tax, paid honestly |
   | `Cast.Boolean` vs `bool.TryParse` | 18.5 ns | JIT-folded | honest loss — the twenty-lexeme vocabulary is why anyone calls this door |
   | `Cast.DateTime` (`1/7/2026 3:04 PM`) vs `DateTime.TryParse` (en-US) | 61.2 ns | 222.9 ns | **3.6x faster** |
   | `Cast.Date` (declared order) vs `DateOnly.TryParse` (en-US) | 33.7 ns | 132.3 ns | **3.9x faster** |
   | `Cast.Double` (eurozone) vs `double.TryParse` (de-DE) | 98.7 ns | 65.9 ns | 1.5x slower — see below |

   Reproduce: `dotnet run -c Release --project HyperCast.Benchmarks`.

   Every row above is the `string` door, transcode included. The `ReadOnlySpan<byte>`
   doors are the primary surface — a caller holding UTF-8 already (a file, a wire buffer,
   one field of a delimited line) never pays that transcode — and 0.2.0 measures them on
   their own, same machine, same run, BCL rows re-measured alongside:

   | Door (UTF-8 in hand) | HyperCast | `string` door | BCL, same run |
   | --- | ---: | ---: | ---: |
   | `Cast.Timestamp` | **51.2 ns** | 76.9 ns | 282.0 ns `DateTimeOffset.TryParse` |
   | `Cast.Uuid` | **36.9 ns** | 47.6 ns | 51.2 ns `Guid.TryParse` — the wash becomes a win |
   | `Cast.Double` | **32.3 ns** | 45.9 ns | 66.8 ns `double.TryParse` |
   | `Cast.Int32` (grouped) | 52.9 ns | 64.1 ns | 49.3 ns `int.TryParse` — still a loss, by 3.5 ns now |

   The UTF-16 doors themselves changed in one small way: they try the stack buffer first
   and rent from the pool only when the encoder says the text did not fit, instead of
   sizing by the 3-bytes-per-char worst case — which had sent any text past ~170 chars to
   the pool even when it was plain ASCII that fit with room to spare.

   The two doors the first consumer asked for, same box, one run, string doors with the
   transcode included, invariant unless stated — printed as measured, because two of the
   three rows are losses:

   | Door | HyperCast | BCL | Verdict |
   | --- | ---: | ---: | --- |
   | `Cast.Decimal` (`12,345.6789`) vs `decimal.TryParse` | 95.9 ns | 94.8 ns (median 81.8) | wash — and exact, canonical, never rounded |
   | `Cast.Decimal` (`($1,234.50)`, en-US `$`) vs `decimal.TryParse` `NumberStyles.Currency` | 115.6 ns | 71.3 ns | 1.6x slower |
   | `Cast.Double` (same text, same format) vs `double.TryParse` `NumberStyles.Currency` | 117.3 ns | 69.1 ns | 1.7x slower |
   | `Cast.Double` (`12345.6789`) vs `double.TryParse`, same run | 56.1 ns | 56.1 ns | wash |

   The currency rows lose for the reason the eurozone row below does, plus one more: a
   declared symbol takes the door's normalize-then-parse path rather than its invariant fast
   lane, and the core itself pays ~29 ns for the symbol and the grouping (`cast_decimal`
   30.5 ns plain against 58.8 ns for `$12,345.67`, measured in the Rust suite). What the
   consumer buys with the loss is the reason it asked: without a declared symbol, every
   non-invariant culture fell out of the native path entirely. A fast lane for a symbol at
   one edge is the obvious next receipt to chase; it is not built.

   **Separator detection is nearly free**: `NumFormat.Detect` on `1.234.567,89` costs
   104.4 ns against 98.7 ns for the same text under a declared eurozone format — ~6 ns for
   resolving the `.`/`,` roles structurally instead of being told them.

**The honest trade-off:** the eurozone `double` row is a real loss — `double.TryParse`
under de-DE beats this door by ~33 ns, because non-invariant separators take the door's
normalize-then-parse path rather than its invariant fast lane. And it's a native dependency
(shipped per-RID inside the package) with a ~15–65 ns FFI crossing on every call. For plain
invariant integers the BCL is already excellent; these doors earn their keep on the
culture-machinery parsers, the closed error contract, and cross-language agreement.

## AOT

`IsAotCompatible` is asserted and the analyzers fail the build on violations; the
`HyperCast.AotSmokeTest` project publishes under `PublishAot` into a genuine native binary
that runs a door from every family — proven, not configured, and re-proven per platform on
every PR: CI publishes it on all six RIDs, fails on any trim diagnostic, runs the binary, and
uploads the log (see [Native binary provenance](#native-binary-provenance)).

## WebAssembly (Blazor)

One compiled assembly covers browser-wasm too — every native entry point is declared twice
(`"hypercast"` for dlopen platforms, `"*"` for the statically-linked wasm module), sharing
the same `EntryPoint`, with `OperatingSystem.IsBrowser()` picked at the call site and
constant-folded by the linker. CI builds the `wasm32-unknown-emscripten` staticlib on every
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
cd rust && cargo build --release
# -> target/release/libhypercast.so  (.dylib on macOS, hypercast.dll on Windows)
```

Drop the result into `csharp/HyperCast/runtimes/<rid>/native/` and the package's own MSBuild
globs will pick it up, or point `dlopen` at it however you prefer — the C ABI in
`rust/src/ffi.rs` is the entire contract: twenty-one exported `cast_*` functions that take
plain pointers into your own buffers, plus `hypercast_version`.

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
Native AOT on all six RIDs, fails the build on any `ILxxxx`/`AOTxxxx` trim diagnostic,
executes the resulting binary, and requires exit 0. Each leg's log uploads as an
`aot-report-{rid}` artifact.

## Install

```sh
dotnet add package HyperCast
```

Per-RID native libraries ship inside the package under `runtimes/`, so a consumer adds one
reference and nothing else — no build step, no manual native staging.

See [the repo root README](https://github.com/SkunkWerkx/HyperCast/blob/master/README.md)
for the full door table, the receipts, and the state of every other language binding.
