# HyperCast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**Allocation-free parsers for scalars from untrusted text — booleans, numerics, UUIDs, and temporals. Every parse returns a Verdict: the value, or a closed reason code with the offending span. Never throws, never allocates. Written once in Rust, called directly from every host language, with a shared conformance corpus so every binding agrees byte for byte.**

Every runtime already has `TryParse`. What it hands back is a `bool` and a shrug — no reason, no location, and none of the notations untrusted sources actually send. HyperCast's doors return a discriminated union instead: the value, or `Empty` / `Malformed` / `OutOfRange` plus the exact byte span that offended, so the error story is data, not archaeology. And because the engine is one Rust `cdylib` (`libhypercast`) called over a plain C ABI, the same logic — the same *bit-for-bit verdicts* — runs in every binding, proven by one corpus every implementation replays.

```csharp
// C# (.NET 11+) — the verdict is a native union; an unhandled case is a compile error
var message = Cast.Int32("(1,234)", NumFormat.From(culture)) switch
{
    Success<int> s => $"got {s.Value}",                     // -1234, accounting negative
    Fault f => $"{f.Reason} at byte {f.Offset}",            // no third case: the compiler checked
};
```

```rust
// Rust — the core itself
let verdict = hypercast::cast_timestamp(b"2026-01-02T15:04:05.123456789+05:00");
// Ok(Timestamp { seconds, nanos }) — protobuf's dual-integer form, normalized to UTC
```

## The doors

| Door | Accepts | Comes out as |
| --- | --- | --- |
| **boolean** | `true/false`, `t/f`, `yes/no`, `y/n`, `1/0`, `on/off`, `enabled/disabled`, `active/inactive`, `checked/unchecked`, `in/out` — ASCII case-insensitive | `bool` |
| **integers** i8–i64, u8–u64 | declared digit grouping, accounting parens `(1,234)`, exponent `1e3`, radix prefixes `0x`/`&H`/`0b` as two's-complement bit patterns (`0xFF` is -1 for i8) — each lenience individually declarable | the type's own range, `OutOfRange` beyond it |
| **reals** f32/f64 | declared separators (eurozone `1.234,5`, French NBSP grouping), parens, exponent, percent (`50%` ⇒ 0.5) — finite values only | IEEE, overflow-to-∞ is `OutOfRange`, `NaN` text is `Malformed` |
| **uuid** | all five .NET `Guid` formats (D/N/B/P/X) plus `urn:uuid:` / `GUID:` / `UUID:` prefixes | 16 bytes, RFC 9562 order |
| **timestamp** | RFC 3339 with **mandatory** zone, normalized to UTC; separate Unix-epoch door with *declared* precision (s/ms/µs/ns — no magnitude guessing) | protobuf `{seconds: i64, nanos: i32}` — bindings present platform fidelity |
| **date / time** | strict `yyyy-MM-dd` (real calendar, leap days); 24-hour `HH:mm[:ss[.f≤9]]` | `{y, m, d}` / nanos-since-midnight |
| **duration** | ISO 8601 fixed components (`P1DT6H30M15.5S` — years/months rejected: not fixed durations), invariant colon form, protobuf JSON seconds (`3.5s`) | protobuf `{seconds, nanos}`, ±10,000-year window |

Culture never lives in the core: numeric doors take a caller-declared format (separators + lenience flags), and each binding bridges its platform's culture machinery to it (`NumFormat.From(CultureInfo)` in C#). Optionality is presentation: `Empty` is a verdict, and the optional doors map it to absent.

## Receipts — proven today, on this repo's own tests

- **Allocation-free is asserted by a counting allocator, not a doc comment** — `rust/tests/allocation_free.rs` wraps `#[global_allocator]` around 1000 calls to every door, success *and* failure paths, and demands zero. A fault is a byte span into the caller's input; nothing is ever captured or formatted on the error path.
- **The corpus is the contract** — `corpus/*.json` (~250 vectors, seeded from the [Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim) `Norse.Primitives` test suites this project descends from) replays through the Rust core's suite *and* every binding's. The C# binding replays all nine files through real P/Invoke today.
- **Fast paths pay for the lenience** — plain-shaped input takes allocation-free fast lanes; only text that actually uses the forgiveness pays for it. Measured with criterion against Rust's own best-in-class: D-format UUID parsing runs **faster than the `uuid` crate's parser** (15.4 vs 17.4 ns, linux-arm64); i64/f64 land within the cost of the trim + span + range contract of `str::parse`.
- **WASM, already, for the core** — the full Rust test suite (unit + allocation proof + all nine corpus replays) passes under `wasmtime` on `wasm32-wasip1`. No clock, no randomness, no dependencies: strictly easier freight than HyperUuid, whose wasm train this rides.
- **C# binding on .NET 11, union-native** — `Verdict<T>` is a real discriminated union: two case arms, no default, and a missing disposition is a **compile error** (CS8509 as error). 24 tests green including the full corpus replay; source-generated `LibraryImport` only, and the AOT smoke test publishes under `PublishAot` into a genuine native binary that runs every door — proven, not configured.
- **C# vs. the BCL, first wave** — BenchmarkDotNet, `[MemoryDiagnoser]`, lenience matched where the BCL has the knob (`AllowThousands`, invariant culture, UTC styles), FFI crossing and UTF-16→UTF-8 transcode *included* in every HyperCast number. Measured on linux-arm64, .NET 11 preview (in-process toolchain — BDN doesn't know the net11 moniker yet); zero managed allocation on every row, both sides:

  | Door | HyperCast | BCL | Verdict |
  | --- | ---: | ---: | --- |
  | `Cast.Timestamp` vs `DateTimeOffset.TryParse` | 71.0 ns | 285.6 ns | **4.0x faster** |
  | `Cast.Duration` vs `TimeSpan.TryParse` | 64.1 ns | 143.0 ns | **2.2x faster** |
  | `Cast.Double` vs `double.TryParse` | 50.4 ns | 69.8 ns | **1.4x faster** |
  | `Cast.Uuid` vs `Guid.TryParse` | 54.8 ns | 51.9 ns | wash — and this door also takes N/B/P/X forms and `urn:uuid:` prefixes |
  | `Cast.Int32` (grouped) vs `int.TryParse` + `AllowThousands` | 64.5 ns | 54.0 ns | 1.2x slower — the crossing tax, paid honestly |
  | `Cast.Boolean` vs `bool.TryParse` | 18.5 ns | unmeasurable* | honest loss — the BCL's five-byte compare wins; the twenty-lexeme vocabulary is why anyone calls this door |

  \* BDN flags the BCL boolean lane `ZeroMeasurement` — the JIT hoists/folds `bool.TryParse` of a loop-invariant string into nothing, which an FFI call structurally can't match. The loss is real either way and is printed as one.

  Read the table the way it's meant: the wins land exactly where the BCL runs culture machinery, the washes come while *also* carrying notations the BCL has no knob for at any price, and every number crosses a native boundary the BCL doesn't. Per-scalar this is parity-or-better; the round-three tabular layer crosses once per chunk and makes the same doors a landslide.
- **Java binding on JDK 22+, union-native the JVM way** — `Verdict<T>` is a `sealed interface` over two records, so a two-arm switch with no default is proven exhaustive by `javac`: an unhandled disposition is a compile failure, the same guarantee the C# binding gets from CS8509-as-error, in Java's own idiom. 21 tests green including the full corpus replay through real FFM downcalls with byte-exact fault spans; and **full nanosecond fidelity** — `Instant`/`LocalTime`/`Duration` keep all nine fractional digits, making the JVM the one platform with zero truncation of what the core parses.
- **Java AOT, proven** — the GraalVM Native Image smoke test builds and runs every door plus the exhaustive union switch as a true native binary. FFM downcalls need explicit Native Image registration; the binding ships its `reachability-metadata.json` in `META-INF` so every consumer inherits it — the non-negotiable, delivered on both managed platforms.
- **Java vs. the JDK, first look** — JMH, honest caveat up front: a deliberately shortened run (wide error bars), so these are directional, not final. The direction is unambiguous where it matters: `Cast.timestamp` **~172 ns vs ~667 ns `Instant.parse`** and **~740 ns `DateTimeFormatter.ISO_OFFSET_DATE_TIME`**, time-of-day ~146 vs ~417 ns `LocalTime.parse`, ISO duration ~163 vs ~267 ns `Duration.parse`. The lean doors (f64/uuid/i32) currently lose — not structurally, but to ~100 ns of per-call `Arena.ofConfined()` setup, the documented next tuning target (thread-local scratch) before a full-length run replaces this table. Losses printed here in the meantime, per house rules.
- **The full seven-binding roster, corpus-green** — Python (PyO3 native extension, `match`/`case` over the two verdict types), Swift (`dlopen` + `@convention(c)`, and the strongest union in the roster — a real enum where exhaustive switch is *compiler-mandatory*, no opt-in flag), Go (dual backend: cgo on darwin/linux, purego everywhere else including Windows and every `CGO_ENABLED=0` cross-compile — the `(value, *Fault)` idiom with `*Fault` as `error`), Ruby (Fiddle fallback + Magnus extension, pattern-matched `Data` classes with Symbol reasons), and PHP (ext-ffi, `Success|Fault` union types over a backed enum). Every one replays all nine corpus files with byte-exact fault spans — **96 binding tests across the five, green on this machine today** — and every one presents its platform's honest fidelity: Ruby and the JVM keep every nanosecond (Ruby's durations are exact `Rational` seconds across the whole ±10,000-year window), Python and PHP truncate to microseconds and say so, Swift's `Duration` is attosecond-backed, and Go returns the protobuf pair because `time.Duration`'s ±292-year ceiling can't hold the window — stated, not wrapped.
- **Benchmarks across the whole spectrum** — each binding carries its ecosystem's own harness, HyperUuid-style: Criterion, BenchmarkDotNet, JMH, `testing.B`, pyperf, benchmark-ips, phpbench, and ordo-one's package-benchmark. The spine of the story, the RFC 3339 timestamp door vs. each platform's own parser (linux-arm64, first-wave numbers, crossing and transcode included in every HyperCast figure):

  | Binding | HyperCast | Platform parser | Verdict |
  | --- | ---: | ---: | --- |
  | C# | 71 ns | 286 ns `DateTimeOffset.TryParse` | **4.0x faster** |
  | Java | ~172 ns | ~667 ns `Instant.parse` | **~3.9x faster** (shortened run) |
  | Swift | 293 ns | 837 ns `Date.ISO8601FormatStyle` | **2.9x faster** — uuid too: 227 vs 629 ns |
  | Go | 155 ns | 59 ns `time.Parse(RFC3339Nano)` | honest loss — Go's stdlib RFC 3339 path is exceptional, and every Go door loses per-call to cgo's crossing + stdlib quality |
  | Ruby (Magnus backend) | **748 ns** | 3.2 µs `Time.iso8601` | **4.3x faster** — see below |
  | Ruby (Fiddle fallback) | 3.5 µs | — | parity with `Time.iso8601`, atop the measured 1.6 µs Fiddle floor; kept as the zero-compile path |
  | Python (PyO3) | **201 ns** | 163 ns `fromisoformat` (C-accelerated) | **near-parity** — see below |
  | Python (retired ctypes) | 3.1 µs | — | the measured ~1 µs ctypes floor — the before-picture that justified going PyO3-only |
  | PHP | **487 ns** | 1.3 µs `DateTimeImmutable` | **2.7x faster** — no new mechanism needed, just the wrapper diet the 105 ns ext-ffi floor demanded |

  **Python's escape from the interpreted tier is its own receipt**: the losses were never "Python calling native code" — they were *ctypes* (interpreted marshalling, ~1 µs/call, measured). The PyO3 extension (`hypercast._native`) is the Rust core linked directly into a CPython extension — no dlopen, no C-ABI hop, the same `METH_FASTCALL` door the builtins walk — and after the mechanism swap proved out, the ctypes fallback was retired entirely, HyperUuid-style: the abi3 wheels maturin builds *are* the package, one per platform covering every CPython 3.10+, no compiler needed to install (the ctypes rows above stand as the measured before-picture). Result: every door 10–18x faster — timestamp 3.07 µs → **201 ns**, i32 → **146 ns** vs `int()`'s 88, uuid at parity with `uuid.UUID()` (both sides now bounded by `UUID.__init__` itself), and the forgiveness doors at ~180 ns for grammar the stdlib doesn't sell.

  The honest reading, after the redemption arc: **every language in the roster now beats its own platform's culture-machinery parser except Go** — whose stdlib is simply excellent and whose per-call story waits for the batch layer. The interpreted tier's losses were never the languages; they were the FFI *mechanisms*, measured then replaced: Python got a PyO3 extension (no mechanism left to pay), Ruby got a Magnus extension (Fiddle's 1.6 µs floor gone — the doors now beat `Time.iso8601` by 4.3x while returning exact `Rational` durations), and PHP needed no new mechanism at all — its ext-ffi floor was 105 ns all along, so a wrapper diet (flat doors, typed cdef structs, static scratch, `createFromTimestamp`) took timestamp from 2.9 µs to 487 ns. Ruby keeps Fiddle as its zero-compile fallback (`HYPERCAST_PURE=1` forces it; both backends replay the corpus green, and a cross-backend agreement spec pins them together), with precompiled platform gems as the vehicle for shipping Magnus without ever making a consumer compile. Benchmarking sagas worth knowing: the Ruby doors were 4.3 µs until per-call `Fiddle::Pointer.malloc` finalizers were hoisted to thread-local scratch, PHP read 20x slow until a loaded Xdebug was caught (`XDEBUG_MODE=off` for all recorded numbers), and Swift's first tape was pure measurement-floor quantization until `.kilo` scaling amortized it — receipts include their own forensics.

## Aspirations — the queue that turns into receipts

Stated the way this project states things: each of these becomes a measured table or a CI matrix row, or it gets cut. Details in [docs/roadmap.md](docs/roadmap.md).

- **First green run of the consolidated pipeline** — the 6-platform matrix is wired as a thin caller of [HyperForge](https://github.com/SkunkWerkx/.github)'s canonical pipeline (plus the browser-wasm staticlib build and five doc-coverage lint jobs, per PR), and it went green in its pre-consolidation form; the native-extensions-as-cargo-features consolidation re-proves on the next PR or dispatch. The Go purego fallback has landed (Windows and `CGO_ENABLED=0` build and pass — measured against cgo in [go/README.md](go/README.md)), and the release machinery — version-baking pack jobs, the prepare-release bump, automatic native-binary staging for the no-packing-step registries — is staged with every registry publish gated off until this repo's own credentials exist.
- **Java's full-length benchmark table** — the thread-local scratch-arena pass, then an unshortened JMH run to graduate the first-look numbers above into final receipts. Per-binding benchmark passes for the rest of the roster follow the same discipline: no number enters this file from a rushed run.
- **The wasm leg beyond the core** — dotnet browser-wasm end-to-end in a real browser session (the staticlib is already built per PR and staged into the NuGet pack; the single-assembly `OperatingSystem.IsBrowser()` pattern is in). Server-side bindings stay native; Pyodide left with the ctypes backend.
- **The payoff: tabular ingestion** — CSV/TSV/delimited and XLSX parsing *on top of* these doors, so the FFI boundary is crossed once per chunk instead of once per cell. A million-row, 20-column file is 20M scalar casts; per-cell that's real crossing overhead, per-chunk it rounds to zero while 15–35 ns doors run in a tight native loop. HyperUuid already measured this exact amortization at 19.6x on its batch API. Column buffers in, parallel verdict arrays out — the reason every fault is a span and never an allocation.

**Non-negotiables, every round:** full AOT in .NET and Java; wasm ride-along for the core and bindings; the tabular layer is server-domain (AOT yes, wasm out of scope there, by design).

## Layout

```
corpus/     the shared conformance vectors — the cross-language contract
rust/       the core: one cdylib, 17 cast_* exports, zero runtime dependencies
csharp/     the .NET 11 binding: Verdict<T> union, LibraryImport, corpus replay, AOT smoke test
java/       the JDK 22+ binding: sealed-interface union, FFM, corpus replay, Native Image smoke test
python/     the 3.10+ binding: match/case verdicts, PyO3 native extension (abi3 wheels)
swift/      the SwiftPM binding: enum verdicts (mandatory-exhaustive switch) over dlopen
go/         the Go binding: (value, *Fault) verdicts, cgo + purego dual backend
ruby/       the 3.2+ binding: pattern-matched Data verdicts, Fiddle + Magnus dual backend
php/        the 8.1+ binding: Success|Fault union types over ext-ffi
docs/       roadmap — where this goes and what's deliberately parked
```

## Why "Hyper"

The SkunkWerkx Hyper* series — [HyperUuid](https://github.com/SkunkWerkx/HyperUuid), HyperCast — owes its founding attitude to Casey Muratori and his recent YouTube talks on what "premature optimization" actually meant. Knuth's line gets quoted as a license to never care; Muratori's point is that most slow software was never *optimized badly* — it was **pessimized by default**: allocations nobody needed, layers nobody asked for, work done and thrown away on every call. These libraries are that argument, practiced: allocation-free cores, no runtime bridge, no reflection, fast paths for the common shape — and every performance claim a measured receipt, because the other half of taking performance seriously is refusing to assert it.

## License

[MIT](LICENSE)
