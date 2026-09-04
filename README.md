# HyperCast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
[![crates.io](https://img.shields.io/crates/v/hypercast.svg)](https://crates.io/crates/hypercast)
[![NuGet](https://img.shields.io/nuget/v/HyperCast.svg)](https://www.nuget.org/packages/HyperCast)
[![Maven Central](https://img.shields.io/maven-central/v/io.github.skunkwerkx/hypercast.svg)](https://central.sonatype.com/artifact/io.github.skunkwerkx/hypercast)
[![PyPI](https://img.shields.io/pypi/v/hypercast.svg)](https://pypi.org/project/hypercast/)
[![Go Reference](https://pkg.go.dev/badge/github.com/SkunkWerkx/HyperCast/go.svg)](https://pkg.go.dev/github.com/SkunkWerkx/HyperCast/go)
[![Swift Package](https://img.shields.io/github/v/tag/SkunkWerkx/HyperCast?label=swift%20package&sort=semver)](https://github.com/SkunkWerkx/HyperCast/tags)
[![Gem](https://img.shields.io/gem/v/hypercast.svg)](https://rubygems.org/gems/hypercast)
[![Packagist](https://img.shields.io/packagist/v/skunkwerkx/hypercast.svg)](https://packagist.org/packages/skunkwerkx/hypercast)

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
| **integers** i8–i64, u8–u64 | declared digit grouping, accounting parens `(1,234)`, exponent `1e3`, radix prefixes `0x`/`&H`/`0b` as two's-complement bit patterns (`0xFF` is -1 for i8) — each lenience individually declarable; or `NumFormat.DETECT`, resolving the `.`/`,` roles **structurally** per input | the type's own range, `OutOfRange` beyond it |
| **reals** f32/f64 | declared separators (eurozone `1.234,5`, French NBSP grouping), parens, exponent, percent (`50%` ⇒ 0.5), a declared currency symbol at either edge (`$1,234.50`, `-$5`, `($5)`, `1.234,50 kr.`) — finite values only; or `NumFormat.DETECT`: a repeated separator is grouping (`1.234.567,89`), with both present the rightmost is decimal, a non-3-digit right run is decimal (`3,1415`), a zero-led `0,785` is decimal — and the genuinely ambiguous (`12.185`, `1,000`) is `Malformed` at the separator, **never guessed** | IEEE, overflow-to-∞ is `OutOfRange`, `NaN` text is `Malformed` |
| **decimal** | the real doors' grammar — declared separators, grouping, parens, exponent, percent, currency, or `NumFormat.DETECT` — but no float is ever formed: `0.1` is one tenth, `50%` is exactly `0.5`, and the result is canonical (`1.10` and `1.1` are the same value and come out the same — trailing fraction zeros trimmed, nothing else ever dropped) | exact `{magnitude: u96, scale: 0..=28, negative}` — .NET `decimal`, `BigDecimal`, `decimal.Decimal` per binding; a magnitude or precision that cannot be represented is `OutOfRange`, **never rounded** |
| **uuid** | all five .NET `Guid` formats (D/N/B/P/X) plus `urn:uuid:` / `GUID:` / `UUID:` prefixes | 16 bytes, RFC 9562 order |
| **timestamp** | RFC 3339 with **mandatory** zone, normalized to UTC; separate Unix-epoch door with *declared* precision (s/ms/µs/ns — no magnitude guessing) | protobuf `{seconds: i64, nanos: i32}` — bindings present platform fidelity |
| **date / time** | strict `yyyy-MM-dd` (real calendar, leap days); separated dates (`1/7/2026`, `1.7.2026`) under a **caller-declared field order** — Jan 7th or Jul 1st only because you said which, never guessed (a 4-digit *first* field is structurally a year, so ISO forms parse under any declared order), and undeclared slash dates stay `Malformed`; 24-hour `HH:mm[:ss[.f≤9]]` | `{y, m, d}` / nanos-since-midnight |
| **local datetime** | the AM/PM world: `<date> [<time>]` — the declared-order date grammar plus an optional 24-hour or `AM`/`PM` time (`1/7/2026 3:04 PM`, `2026-01-07T15:04:05`, `3 PM` hour-only, `12 AM` = midnight), **no zone read and none invented** — zone-less text names no instant, so zoned text is `Malformed` here and RFC 3339 stays the timestamp door | civil `{y, m, d} + nanos-of-day` — `LocalDateTime` / `DateTime(Unspecified)` / naive `datetime` per binding; fusing a zone is the caller's job |
| **excel serial** | spreadsheet date serials under a **caller-declared epoch** (1900 or 1904 — a workbook-level setting no cell carries), whole part days and fraction time-of-day; the 1900 system's serial `60` is the `1900-02-29` that never existed (Lotus 1-2-3's leap-year bug, kept by Excel for file compatibility) and is `Malformed`, exactly as the text `1900-02-29` already is — so every serial past it is shifted one day, the arithmetic hand-rolled conversions get wrong | protobuf `{seconds, nanos}` read as UTC — a cell carries no zone and none is invented |
| **duration** | ISO 8601 fixed components (`P1DT6H30M15.5S` — years/months rejected: not fixed durations), invariant colon form, protobuf JSON seconds (`3.5s`) — with ISO 8601's comma decimal mark accepted in all three shapes (`PT1,5S`, `0:00:01,5`: durations have no grouping, so a comma can only be a decimal mark) | protobuf `{seconds, nanos}`, ±10,000-year window |

Culture never lives in the core: numeric doors take a caller-declared format (separators, lenience flags, and a declared currency symbol — the one field a culture table has to fill in, so `NumFormat.From(CultureInfo)` copies the culture's own), separated dates take a caller-declared field order (`DateOrder` — the en-US/en-GB `1/7/2026` ambiguity is resolved by declaration, never sniffed), and each binding bridges its platform's culture machinery to both (`NumFormat.From(CultureInfo)` / `DateOrders.From(CultureInfo)` in C#, `DateOrder.from(Locale)` in Java, `DateOrder.from(locale:)` in Swift). Optionality is presentation: `Empty` is a verdict, and the optional doors map it to absent. And before the first cast, every binding can say whether the core loaded and which version it is (`Cast.IsAvailable`/`NativeVersion` in C#, and the same pair in each language's idiom), so a consumer with a fallback gates on a probe rather than catching a load failure.

## Receipts — proven today, on this repo's own tests

- **Allocation-free is asserted by a counting allocator, not a doc comment** — `rust/tests/allocation_free.rs` wraps `#[global_allocator]` around 1000 calls to every door, success *and* failure paths, and demands zero. A fault is a byte span into the caller's input; nothing is ever captured or formatted on the error path.
- **The corpus is the contract** — `corpus/*.json` (seeded from the [Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim) `Norse.Primitives` test suites this project descends from) replays through the Rust core's suite *and* every binding's. All eight replay the full thirteen-file set today — C# through real P/Invoke, Java through FFM downcalls, Ruby through *both* its Magnus and Fiddle backends.
- **Published, to all five registries, and consumable from all eight languages** — every binding is published and installable from its real registry, with the live version on each badge above: [crates.io](https://crates.io/crates/hypercast), [nuget.org](https://www.nuget.org/packages/HyperCast), [PyPI](https://pypi.org/project/hypercast/) (6 abi3 wheels), [RubyGems](https://rubygems.org/gems/hypercast) (7 gems — one universal Fiddle, six precompiled Magnus, each fat across Ruby 3.4 and 4.0 since a Magnus extension is tied to one Ruby minor) and [Maven Central](https://central.sonatype.com/artifact/io.github.skunkwerkx/hypercast); Go and Swift resolve from the tag itself (Go's prefixed `go/vX.Y.Z`), PHP from [Packagist](https://packagist.org/packages/skunkwerkx/hypercast). Trusted Publishing/OIDC wherever the registry offers it — no long-lived tokens for NuGet, RubyGems, or PyPI. Every one of the eight was then installed from its real registry into a clean project and run, because "the publish succeeded" and "a consumer can use it" are different claims: identical verdicts across all eight, and Java AOT plus C# AOT and Blazor wasm verified against the published artifacts rather than the working tree.

  Three of the four bugs this project has shipped were found exactly there, in the gap between those two claims, and none of them could fail a build in this repo. v0.0.1's first tag landed four of five registries: Maven died in *our* Gradle config, where `sourcesJar` read `stageNativeLibrary`'s output without declaring the dependency — invisible to CI, which never builds a sources jar. Then v0.0.1's published artifacts turned out to be broken in two ways for AOT and wasm consumers specifically (see the Java AOT and WebAssembly notes below), which v0.0.2 fixes. The recovery protocol — gate the registries that accepted a version, fix the one that didn't, retag — is written into `release.yml`'s header, because a version is only ever burned where it was actually accepted.
- **Fast paths pay for the lenience** — plain-shaped input takes allocation-free fast lanes; only text that actually uses the forgiveness pays for it. Measured with criterion against Rust's own best-in-class (linux-arm64): `cast_uuid` 15.8 ns against the `uuid` crate's 11.8 ns, `cast_i64` 15.5 vs 9.7 ns `str::parse`, `cast_timestamp` 30.3 vs 21.9 ns `time`. In-process against Rust's own parsers these doors trade raw speed for what they *return* (a verdict with a span) and what they *accept*; the speed story belongs to the bindings, where the competition is culture machinery. **Correction on the record:** an earlier version of this line claimed the UUID door beat the `uuid` crate (15.4 vs 17.4 ns). Our number didn't move; `uuid` 1.26 got faster. Receipts get re-run, and this one changed.
- **Fuzzed, and it found real bugs** — a `cargo-fuzz` target (`rust/fuzz/`) drives every door under every format profile and every declared order/precision/epoch, asserting two invariants every binding silently relies on: a door never panics on any byte sequence, and every fault span stays inside the caller's buffer (`offset + len <= input.len()`, which bindings slice with). It caught two real classes within a minute — truncation faults pointing one byte past the input, and `char_len` spans overrunning on text ending mid-UTF-8-character — both since fixed structurally and pinned by `rust/tests/fault_span_invariant.rs` (every corpus input truncated at every byte boundary, through every door) so they fail plain `cargo test`. The following 550M-execution session found nothing.
- **WASM, for the core and for four of the bindings** — the full Rust test suite (unit tests, the allocation proof, every corpus replay, the fault-span invariant sweeps) passes under `wasmtime` on `wasm32-wasip1`. No clock, no randomness, no dependencies: strictly easier freight than HyperUuid, whose wasm train this rides. And the same `wasm32-wasip1` module now ships inside the jar, the gems and the wheels and is committed under `go/native/`, where GraalWasm (Java) and wasmtime (Ruby, Python, Go) run it in-process as a second backend behind each binding's existing switch — every one of those four suites, corpus replay included, runs a second time through it on every CI leg. See [WebAssembly](#webassembly).
- **C# binding on .NET 11, union-native** — `Verdict<T>` is a real discriminated union: two case arms, no default, and a missing disposition is a **compile error** (CS8509 as error). the whole suite green including the full corpus replay; source-generated `LibraryImport` only, and the AOT smoke test publishes under `PublishAot` into a genuine native binary that runs every door — proven, not configured.
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
- **Java binding on JDK 22+, union-native the JVM way** — `Verdict<T>` is a `sealed interface` over two records, so a two-arm switch with no default is proven exhaustive by `javac`: an unhandled disposition is a compile failure, the same guarantee the C# binding gets from CS8509-as-error, in Java's own idiom. 31 tests green including the full corpus replay through real FFM downcalls with byte-exact fault spans; and **full nanosecond fidelity** — `Instant`/`LocalTime`/`Duration` keep all nine fractional digits, making the JVM the one platform with zero truncation of what the core parses.
- **Java AOT, proven** — the GraalVM Native Image smoke test builds and runs every door plus the exhaustive union switch as a true native binary. Native Image needs the FFM downcall signatures *and* a resources glob registered, and the binding ships both in its `reachability-metadata.json` so a consumer inherits them with zero configuration — the non-negotiable, delivered on both managed platforms. The resources half was missing from v0.0.1: a consumer's native binary built clean and died on first call with "classpath resource not found", while this repo's own smoke test stayed green because it declared the glob itself. That override is gone, so the test now proves the packaged metadata alone is sufficient. Found by building a real native image against the published jar, not against the repo.
- **Java vs. the JDK — full-length, and the input no longer copies.** 2 forks × (5 warmup + 10 measurement) iterations, 20 samples a row, `-prof gc`: `Cast.timestamp` **52.5 ± 1.7 ns vs 595.9 ± 16.8 ns `Instant.parse`** (11.4x), `Cast.time` **45.1 vs 393.4 ns** (8.7x), `Cast.duration` **60.9 vs 268.8 ns** (4.4x), `Cast.uuid` **38.3 vs 45.9 ns `UUID.fromString`** — a loss in 0.1.0, a win now. **What moved the numbers, twice:** 0.1.0 removed the `Arena.ofConfined()` every door opened per call; 0.2.0 removed the copy that was left — every downcall is linked `Linker.Option.critical(true)`, so the caller's `byte[]` crosses as a pinned heap segment instead of being copied into a per-thread native staging buffer, every door gained a `MemorySegment` overload for casting a slice of one buffer with nothing copied, and the UUID door reads two big-endian longs instead of sixteen bytes. One row still loses and is printed as such: `Boolean.parseBoolean` is unbeatable by construction (the JIT folds a loop-invariant call to nothing).
- **The full seven-binding roster, corpus-green** — Python (PyO3 native extension, `match`/`case` over the two verdict types), Swift (`dlopen` + `@convention(c)`, and the strongest union in the roster — a real enum where exhaustive switch is *compiler-mandatory*, no opt-in flag), Go (dual backend: cgo on darwin/linux, purego everywhere else including Windows and every `CGO_ENABLED=0` cross-compile — the `(value, *Fault)` idiom with `*Fault` as `error`), Ruby (Fiddle fallback + Magnus extension, pattern-matched `Data` classes with Symbol reasons), and PHP (ext-ffi, `Success|Fault` union types over a backed enum). Every one replays every corpus file with byte-exact fault spans — **every suite green on this machine today** — and every one presents its platform's honest fidelity: Ruby and the JVM keep every nanosecond (Ruby's durations are exact `Rational` seconds across the whole ±10,000-year window), Python and PHP truncate to microseconds and say so, Swift's `Duration` is attosecond-backed, and Go returns the protobuf pair because `time.Duration`'s ±292-year ceiling can't hold the window — stated, not wrapped.
- **Benchmarks across the whole spectrum** — each binding carries its ecosystem's own harness, HyperUuid-style: Criterion, BenchmarkDotNet, JMH, `testing.B`, pyperf, benchmark-ips, phpbench, and ordo-one's package-benchmark. The spine of the story, the RFC 3339 timestamp door vs. each platform's own parser (linux-arm64, first-wave numbers, crossing and transcode included in every HyperCast figure):

  | Binding | HyperCast | Platform parser | Verdict |
  | --- | ---: | ---: | --- |
  | C# | 76.9 ns (`string`), **51.2 ns** (UTF-8) | 282 ns `DateTimeOffset.TryParse` | **3.7x / 5.5x faster** |
  | Java | **52.5 ns** (`String`), 46.2 ns (`byte[]`) | 596 ns `Instant.parse` | **11.4x faster** (full-length run) |
  | Swift | **55 ns** (was 281 in 0.1.0) | 817 ns `Date.ISO8601FormatStyle` | **14.9x faster** — uuid too: 37 vs 603 ns, zero mallocs on every door |
  | Go | 112 ns, 0 allocs (was 174 ns, 2 allocs) | 67 ns `time.Parse(RFC3339Nano)` | honest loss — Go's stdlib RFC 3339 path is exceptional, and every Go door still loses per-call to cgo's crossing, now without a heap allocation on top |
  | Ruby (Magnus backend) | **771 ns** | 3.32 µs `Time.iso8601` | **4.3x faster** — see below |
  | Ruby (Fiddle fallback) | 3.2 µs | — | parity with `Time.iso8601`, atop the measured 1.6 µs Fiddle floor; kept as the zero-compile path |
  | Python (PyO3) | **201 ns** | 163 ns `fromisoformat` (C-accelerated) | **near-parity** — see below |
  | Python (retired ctypes) | 3.1 µs | — | the measured ~1 µs ctypes floor — the before-picture that justified going PyO3-only |
  | PHP | **487 ns** | 1.3 µs `DateTimeImmutable` | **2.7x faster** — no new mechanism needed, just the wrapper diet the 105 ns ext-ffi floor demanded |

  **Python's escape from the interpreted tier is its own receipt**: the losses were never "Python calling native code" — they were *ctypes* (interpreted marshalling, ~1 µs/call, measured). The PyO3 extension (`hypercast._native`) is the Rust core linked directly into a CPython extension — no dlopen, no C-ABI hop, the same `METH_FASTCALL` door the builtins walk — and after the mechanism swap proved out, the ctypes fallback was retired entirely, HyperUuid-style: the abi3 wheels maturin builds *are* the package, one per platform covering every CPython 3.10+, no compiler needed to install (the ctypes rows above stand as the measured before-picture). Result: every door 10–18x faster — timestamp 3.07 µs → **201 ns**, i32 → **146 ns** vs `int()`'s 88, uuid **730 ns against 1.18 µs** before 0.2.0 — it now builds `uuid.UUID` the way HyperUuid pinned, `UUID.__new__` plus `object.__setattr__`, skipping the `__init__` that had bounded both sides — ahead of `uuid.UUID()`'s 979 ns, and the forgiveness doors at ~180 ns for grammar the stdlib doesn't sell.

  The honest reading, after the redemption arc: **every language in the roster now beats its own platform's culture-machinery parser except Go** — whose stdlib is simply excellent and whose per-call story waits for the batch layer. The interpreted tier's losses were never the languages; they were the FFI *mechanisms*, measured then replaced: Python got a PyO3 extension (no mechanism left to pay), Ruby got a Magnus extension (Fiddle's 1.6 µs floor gone — the doors now beat `Time.iso8601` by 4.3x while returning exact `Rational` durations), and PHP needed no new mechanism at all — its ext-ffi floor was 105 ns all along, so a wrapper diet (flat doors, typed cdef structs, static scratch, `createFromTimestamp`) took timestamp from 2.9 µs to 487 ns. Ruby keeps Fiddle as its zero-compile fallback (`HYPERCAST_PURE=1` forces it; both backends replay the corpus green, and a cross-backend agreement spec pins them together), with precompiled platform gems as the vehicle for shipping Magnus without ever making a consumer compile. Benchmarking sagas worth knowing: the Ruby doors were 4.3 µs until per-call `Fiddle::Pointer.malloc` finalizers were hoisted to thread-local scratch, PHP read 20x slow until a loaded Xdebug was caught (`XDEBUG_MODE=off` for all recorded numbers), and Swift's first tape was pure measurement-floor quantization until `.kilo` scaling amortized it — receipts include their own forensics.

- **The messy-feed doors, cross-binding** — the declared-order date/date-time doors exist for text with no stdlib parser at all, so each row pairs against whatever that platform *does* offer for the same string (`1/7/2026 3:04 PM`): a pattern formatter, a culture-aware `TryParse`, or `strptime`. Same machine, same run, linux-arm64:

  | Binding | HyperCast | Platform's closest parser | Verdict |
  | --- | ---: | ---: | --- |
  | Python | **409 ns** | 5.19 µs `datetime.strptime` | **12.7x faster** |
  | Java | **64.9 ns** | 386.2 ns `DateTimeFormatter` (`M/d/yyyy h:mm a`) | **6.0x faster** |
  | C# | **61.2 ns** | 222.9 ns `DateTime.TryParse` (en-US) | **3.6x faster** |
  | Swift | **129 ns** | 28 µs `DateFormatter` (same pattern, hoisted — measured at 810 ns on 0.1.0's toolchain, 28 µs on Swift 6.3.3 today; see swift/README) | **faster by two orders on this toolchain** |
  | PHP | **620 ns** | 1.36 µs `DateTimeImmutable::createFromFormat` | **2.2x faster** |
  | Go | 122 ns, 0 allocs | 135 ns `time.Parse` w/ layout | parity — the by-value shims closed the gap that was a loss in 0.1.0 |
  | Ruby | 1.30 µs | 1.02 µs `DateTime.strptime` | honest loss — see below |

  **Ruby's loss is about the carrier, not the parse.** Its timestamp door is 4x *faster* than `Time.iso8601` on the same backend; the civil door is slower because building a stdlib `DateTime` with an exact `Rational` second costs more than the entire native call, where `Time` is one cheap `rb_time_nano_new`. (Same reason its `DateTime` shows a `+00:00` offset: a property of the type, not a zone the parse assigned.) Printed because it's real — house rules.

  **Separator detection costs what the core says it costs, once the carrier is thin enough to see it.** `NumFormat.DETECT` resolves `.`/`,` roles structurally per input at ~11 ns in the raw Rust core. In 0.1.0 that vanished inside every binding's per-call overhead (Java 105.1 vs 106.0 ns declared, Swift 399 vs 406); with 0.2.0's carriers it shows: Java 100.1 vs 86.5 ns, Swift 86 vs 74, Go 172 vs 164, Ruby (Magnus) 591 vs 570 — both of those Ruby numbers down from ~990, since any format other than `INVARIANT` used to pay three method dispatches per call; C# ~6 ns, PHP ~22 ns, Python ~18 ns.

## Provenance

Every published artifact across all eight bindings — the package itself where a registry
has one, and the native binaries underneath it either way — carries a GitHub build-provenance
attestation, checkable with `gh attestation verify`. Which flags that needs depends on where
the signing workflow physically lives, not on which registry the artifact ended up in:
artifacts signed directly inside this repo's own `release.yml` — the RubyGems gems, the PyPI
wheels, and the published NuGet package — verify with plain `--repo SkunkWerkx/HyperCast`.
Artifacts signed by a reusable workflow hosted in `SkunkWerkx/.github` — the crates.io crate,
the Maven jar, the pre-push NuGet package, every native library (which is the entire
story for Go, Swift, and PHP, none of which has a package-level attestation of its own), and
the `wasm32-wasip1` module that rides inside the jar, the gems, the wheels and `go/native/` —
need `--signer-repo SkunkWerkx/.github` added, or `--owner SkunkWerkx` in place of both
flags. Get it wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which
reads like a bad signature but is only an identity mismatch.

The gates run on the way in, not just on the way out: `stage-native-binaries.yml` verifies
each native library's attestation before committing it for the Go/Swift/PHP consumers, the
RubyGems job verifies every native artifact it packs before building a gem, and both the
crate and the gems are attested *before* their irreversible push — a signing failure stops
the release while it can still be retried. See each binding's own README for its exact
verify command and artifact: [Rust](rust/#verifying-provenance),
[C#](csharp/#native-binary-provenance), [Java](java/#verifying-provenance),
[Ruby](ruby/#verifying-provenance), [Python](python/#verifying-provenance),
[PHP](php/#verifying-provenance), [Swift](swift/#verifying-provenance),
[Go](go/#verifying-build-provenance).

## WebAssembly

WebAssembly meets a binding in one of two directions, and they share nothing mechanically:

- **The core runs as wasm inside the binding.** The process stays native. The Rust core
  arrives as a `wasm32-wasip1` module, `hypercast.wasm`, and a wasm engine the ecosystem
  already has runs it in-process: no `dlopen`, no per-platform binary, the same
  C-ABI exports. The engine is an optional dependency the consumer adds only if they want
  this path.
- **The binding is compiled to wasm.** The whole consumer app becomes a wasm module
  (Blazor, a `wasm32` Rust crate) and the Rust core has to be linked into that build by the
  ecosystem's own toolchain.

Where each of the eight stands, today:

| Binding | Core as wasm inside the binding | Binding compiled to wasm |
| --- | --- | --- |
| Rust | Not applicable: the crate *is* the core, and the wasip1 build of it is the module everyone else embeds. | **Yes.** The full suite passes under [wasmtime](https://wasmtime.dev/) on `wasm32-wasip1`; no clock, no entropy, nothing to stub. See [`rust/README.md`](rust/README.md#webassembly). |
| C# | Not built. | **Packaged and linked, not yet run in a browser.** `dotnet add package HyperCast` into a Blazor WebAssembly project links the staticlib and exports all 20 doors through the shipped `.targets`; see [`csharp/README.md`](csharp/README.md#webassembly-blazor) and the aspirations below for what is still owed. |
| Java | **Yes.** [GraalWasm](https://www.graalvm.org/webassembly/); `-Dhypercast.backend=wasm`, or automatic when the jar has no native build for the platform. | Blocked. No Java-to-wasm compiler supports the Foreign Function & Memory API this binding is built on: GraalVM's Web Image (`--tool:svm-wasm`) is labeled experimental and never lists it, and neither TeaVM nor CheerpJ has it. |
| Ruby | **Yes.** [wasmtime gem](https://github.com/bytecodealliance/wasmtime-rb); `HYPERCAST_WASM=1`, or automatic when no native library exists for the platform. | Not tried. `ruby.wasm` has no runtime library search, so the Fiddle backend cannot work there; it does link C extensions statically at build time, and building a `ruby.wasm` with the Magnus extension linked in has not been attempted here. |
| Python | **Yes.** [wasmtime-py](https://github.com/bytecodealliance/wasmtime-py) (`pip install hypercast[wasm]`); `HYPERCAST_WASM=1`, or automatic when the PyO3 extension fails to import. | Proven once, then removed. The core built as an Emscripten side module loaded through `ctypes.CDLL` in a real [Pyodide](https://pyodide.org/) session; that smoke test existed to justify the `ctypes` backend and went with it when PyO3 `abi3` wheels made the fallback unnecessary. |
| Go | **Yes.** [wasmtime-go](https://github.com/bytecodealliance/wasmtime-go); `-tags hypercast_wasm`, opt-in only, never selected automatically. cgo throughout, so no win-arm64. | Blocked. `cgo` has no wasm target, `purego`'s supported-platform list has no wasm entry (its whole model is runtime `dlopen`), and `go:wasmimport`/`go:wasmexport` let a Go module talk to its host, not link a second module. |
| Swift | Not built. No wasm engine ships as a Swift package with a stable API, so there is nothing to embed. | Blocked. swift.org ships official WASM SDKs since Swift 6.2, but its own docs say dynamic linking "is not formally specified for `wasip1` triples and tooling for it is not available yet," and no static path to a Rust `.a` is documented either. |
| PHP | Not built. There is no maintained wasm engine PHP can embed. | Blocked. The maintained wasm PHP (WordPress Playground's `@php-wasm`) loads extensions at build time or startup only, and there is no indication the `FFI` extension this binding needs is available there at all. |

The two directions are blocked, where they are blocked, for different reasons. Compiling a
binding to wasm needs the ecosystem's toolchain to link a Rust static library into its own
wasm build. .NET has a supported mechanism for exactly that (`NativeFileReference`, which this
package's `.targets` injects for you); Swift, PHP and Go do not. Java's gap is different in
kind: the loading mechanism is not the problem, the compilers that exist have no FFM. The
in-process backends sidestep all of that rather than climb it, because the engine is the
loader, and they are what a platform with no native build falls back to.

### The in-process backends

One artifact, `hypercast.wasm`, built from the same crate with wasi-libc's `malloc`/`free`
exported (two linker flags in `rust/.cargo/config.toml`, no source change), ships beside the
native libraries in the jar, the gems and the wheels, and is committed under `go/native/`
like the rest of Go's binaries. CI builds it on every leg and runs the Java, Ruby, Python and
Go suites a second time through it (Go on the non-Windows legs, since wasmtime-go has no
win-arm64 build). Every number below was measured through the shipped binding on one
linux-arm64 box (WSL2) in one session, native column beside it; each binding's README has
the mechanics, the exact loop, and more doors.

| Binding | Engine dependency | `i32`, one call | `timestamp`, one call | Native, same box |
| --- | --- | ---: | ---: | --- |
| Java | `org.graalvm.polyglot:wasm`, `compileOnly`, never in the POM | 237 ns (grouped) on GraalVM CE 25.3 (JIT); 20.1 µs on Temurin 25 (interpreter) | 354 ns (JIT); 8.5 µs (interpreter) | 66 ns / 60 ns |
| Ruby | `wasmtime` gem, a Gemfile group for the suite, never a dependency of the gem | 2.65 µs | 3.10 µs | 555 ns / 769 ns (Magnus); 2.78 / 3.12 µs (Fiddle) |
| Python | `wasmtime`, via the `[wasm]` extra | 6.4 µs | 7.7 µs | 139 ns / 423 ns |
| Go | wasmtime-go, compiled in only under the tag | 3.4 µs | 3.8 µs | 84 ns / 101 ns |

Three footnotes to those rows. On a stock JDK GraalWasm has no JIT and runs the module
interpreted, with a startup warning, so its cost scales with how much wasm the parse
executes; under GraalVM's JIT the same doors sit at 4-10x the FFM downcall, and the wasm path
also survives a GraalVM Native Image build on the jar's own reachability metadata (the Java
README has the receipts). Ruby's wasm backend lands at Fiddle parity door for door, which
makes it a free fallback there. And unlike HyperUuid there is no batch door anywhere in this
crate to amortize the crossing behind — a per-cell workload pays it per cell — so these
backends are portability answers, not speed options, until round three's chunk layer crosses
once per chunk.

Two facts every one of those four shares, both learned the hard way in HyperUuid. The host
must take its buffers from the guest's own allocator: a host-picked offset past the data
segments looked free and was not, because dlmalloc claims the tail of the initial memory on
first use, and the next allocation overwrote a buffer mid-way. And every call is serialized
under a lock, because neither a GraalWasm `Context` nor a wasmtime `Store` is safe for
concurrent use; the native backends stay lock-free.

## Aspirations — the queue that turns into receipts

Stated the way this project states things: each of these becomes a measured table or a CI matrix row, or it gets cut. Details in [docs/roadmap.md](docs/roadmap.md).

- **Per-binding benchmark passes for the rest of the roster** — Java's is done (the scratch-arena pass landed and the full-length JMH run replaced its directional table), and every new door now carries numbers. What's left is the same discipline applied to the remaining first-wave figures: no number enters this file from a rushed run, and re-runs get published even when they go the wrong way — see the `uuid`-crate correction above.
- **The wasm leg beyond the core** — the packaging half is done and proven from a real consumer: a Blazor app that only adds a `PackageReference` links the staticlib and exports all 20 doors, via the `build/net11.0/HyperCast.targets` the package now ships. What's left is running that app in an actual browser session and reporting numbers from it; a successful `wasm-ld` link is not the same claim as working in a browser, and this project doesn't get to call it proven until it has been. The other direction — the core as wasm *inside* a native process — is built for Java, Ruby, Python and Go (see [WebAssembly](#webassembly)); Pyodide left with the ctypes backend.
- **The payoff: tabular ingestion** — CSV/TSV/delimited and XLSX parsing *on top of* these doors, so the FFI boundary is crossed once per chunk instead of once per cell. A million-row, 20-column file is 20M scalar casts; per-cell that's real crossing overhead, per-chunk it rounds to zero while 15–35 ns doors run in a tight native loop. HyperUuid already measured this exact amortization at 19.6x on its batch API. Column buffers in, parallel verdict arrays out — the reason every fault is a span and never an allocation. Under way in three repositories of their own — [HyperTabular](https://github.com/SkunkWerkx/HyperTabular) (the contract), [HyperDelimited](https://github.com/SkunkWerkx/HyperDelimited), and [HyperWorkbook](https://github.com/SkunkWerkx/HyperWorkbook) — each with a Rust crate green against this core; bindings and the corpus are what remain.

**Non-negotiables, every round:** full AOT in .NET and Java; wasm ride-along for the core and bindings; the tabular layer is server-domain (AOT yes, wasm out of scope there, by design).

## Layout

```
corpus/     the shared conformance vectors — the cross-language contract
rust/       the core: one cdylib, 20 cast_* exports, zero runtime dependencies
csharp/     the .NET 11 binding: Verdict<T> union, LibraryImport, corpus replay, AOT smoke test
java/       the JDK 22+ binding: sealed-interface union, FFM + GraalWasm backends, corpus replay, Native Image smoke test
python/     the 3.10+ binding: match/case verdicts, PyO3 native extension (abi3 wheels) + wasmtime backend
swift/      the SwiftPM binding: enum verdicts (mandatory-exhaustive switch) over dlopen
go/         the Go binding: (value, *Fault) verdicts, cgo + purego + wasmtime-go backends
ruby/       the 3.2+ binding: pattern-matched Data verdicts, Fiddle + Magnus + wasmtime backends
php/        the 8.1+ binding: Success|Fault union types over ext-ffi
docs/       roadmap and parked designs — where this goes, and what's deliberately not built yet
```

## Why "Hyper"

The SkunkWerkx Hyper* series — [HyperUuid](https://github.com/SkunkWerkx/HyperUuid), HyperCast — owes its founding attitude to Casey Muratori and his recent YouTube talks on what "premature optimization" actually meant. Knuth's line gets quoted as a license to never care; Muratori's point is that most slow software was never *optimized badly* — it was **pessimized by default**: allocations nobody needed, layers nobody asked for, work done and thrown away on every call. These libraries are that argument, practiced: allocation-free cores, no runtime bridge, no reflection, fast paths for the common shape — and every performance claim a measured receipt, because the other half of taking performance seriously is refusing to assert it.

## License

[MIT](LICENSE)
