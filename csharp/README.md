# HyperCast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)

**`TryParse` hands back a `bool` and a shrug. These doors hand back a native discriminated
union — the value, or `Empty`/`Malformed`/`OutOfRange` plus the exact byte span that
offended — and an unhandled case is a compile error, not a review nit.**

Allocation-free scalar casts — booleans, the full integer family, reals, UUIDs, temporals —
as source-generated `[LibraryImport]` P/Invoke straight into the native `libhypercast` Rust
core. No runtime bridge, no reflection anywhere in the assembly. .NET 11 is the floor
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

Door names mirror the native ABI (`Int32`, `Double`, `Timestamp`, …) so the polyglot
surface reads identically across bindings. Culture never lives in the core —
`NumFormat.From(CultureInfo)` bridges .NET's culture machinery to the caller-declared
format the native side actually reads. .NET-flavored fidelity, stated honestly:
`DateTimeOffset`/`TimeOnly`/`TimeSpan` resolve to 100 ns ticks, so sub-tick nanoseconds
truncate (the core carries full nanosecond fidelity; .NET's clock types don't).

## Why not the BCL's own `TryParse` family?

1. **The error story is data, not archaeology** — a closed reason plus the offending span,
   against the BCL's bare `false`.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, radix prefixes, all five `Guid` formats *plus* `urn:uuid:` prefixes,
   protobuf JSON durations — much of it grammar the BCL has no knob for at any price.
3. **One engine across a polyglot system** — the same Rust core, bit-for-bit verdicts,
   proven by the shared conformance corpus every binding replays (all 24 of this binding's
   tests include the full nine-file corpus through real P/Invoke).
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

   Reproduce: `dotnet run -c Release --project HyperCast.Benchmarks`.

**The honest trade-off:** a native dependency (shipped per-RID inside the package) and a
~15–65 ns FFI crossing on every call. For plain invariant integers the BCL is already
excellent; these doors earn their keep on the culture-machinery parsers, the closed error
contract, and cross-language agreement.

## AOT

`IsAotCompatible` is asserted and the analyzers fail the build on violations; the
`HyperCast.AotSmokeTest` project publishes under `PublishAot` into a genuine native binary
that runs every door — proven, not configured.

## WebAssembly (Blazor)

One compiled assembly covers browser-wasm too — every native entry point is declared twice
(`"hypercast"` for dlopen platforms, `"*"` for the statically-linked wasm module), sharing
the same `EntryPoint`, with `OperatingSystem.IsBrowser()` picked at the call site and
constant-folded by the linker. CI builds the `wasm32-unknown-emscripten` staticlib on every
PR; the release pack stages it under `runtimes/browser-wasm/nativeassets/`.

## Install

Not on nuget.org yet — the release pipeline is staged (`.github/workflows/release.yml`,
Trusted Publishing pending) and the package ships with the first coordinated tag as
`HyperCast`, with per-RID natives under `runtimes/` and this README inside the package.
Until then: clone the repo, `cargo build --release` in `rust/`, and reference
`csharp/HyperCast/HyperCast.csproj` — the csproj stages the fresh native build into the
output automatically.

See [the repo root README](https://github.com/SkunkWerkx/HyperCast/blob/master/README.md)
for the full door table, the receipts, and the state of every other language binding.
