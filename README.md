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
| **date / time** | strict `yyyy-MM-dd` (real calendar, leap days); separated dates (`1/7/2026`, `1.7.2026`) under a **caller-declared field order** — Jan 7th or Jul 1st only because you said which, never guessed (a 4-digit *first* field is structurally a year, so ISO forms parse under any declared order), and undeclared slash dates stay `Malformed`; 24-hour `HH:mm[:ss[.f≤9]]`. The wrong shape is `Malformed`; the right shape naming nothing real (month `13`, `02-30`, `24:00`, a `:60` second) is `OutOfRange` at that field | `{y, m, d}` / nanos-since-midnight |
| **local datetime** | the AM/PM world: `<date> [<time>]` — the declared-order date grammar plus an optional 24-hour or `AM`/`PM` time (`1/7/2026 3:04 PM`, `2026-01-07T15:04:05`, `3 PM` hour-only, `12 AM` = midnight), **no zone read and none invented** — zone-less text names no instant, so zoned text is `Malformed` here and RFC 3339 stays the timestamp door | civil `{y, m, d} + nanos-of-day` — `LocalDateTime` / `DateTime(Unspecified)` / naive `datetime` per binding; fusing a zone is the caller's job |
| **excel serial** | spreadsheet date serials under a **caller-declared epoch** (1900 or 1904 — a workbook-level setting no cell carries), whole part days and fraction time-of-day; the 1900 system's serial `60` is the `1900-02-29` that never existed (Lotus 1-2-3's leap-year bug, kept by Excel for file compatibility) and is `OutOfRange`, exactly as the text `1900-02-29` is — so every serial past it is shifted one day, the arithmetic hand-rolled conversions get wrong | protobuf `{seconds, nanos}` read as UTC — a cell carries no zone and none is invented |
| **typed (a number already held)** | the `f64` a workbook stores for a numeric cell, with no text to parse: as an exact decimal (the shortest decimal that names the double — the digits Excel and LibreOffice write, so `0.1` is one tenth and `0.1 + 0.2` is `0.30000000000000004`), as an Excel serial under a declared epoch, as a serial's time of day, and as a span of days. NaN is `Malformed`; what the decimal cannot hold is `OutOfRange`, **never rounded** | the decimal, civil date-time, time-of-day and duration shapes above; a fault's span is empty, there being no text to index |
| **duration** | ISO 8601 fixed components (`P1DT6H30M15.5S` — years/months rejected: not fixed durations), invariant colon form, protobuf JSON seconds (`3.5s`) — with ISO 8601's comma decimal mark accepted in all three shapes (`PT1,5S`, `0:00:01,5`: durations have no grouping, so a comma can only be a decimal mark) | protobuf `{seconds, nanos}`, ±10,000-year window |

Culture never lives in the core: numeric doors take a caller-declared format (separators, lenience flags, and a declared currency symbol — the one field a culture table has to fill in, so `NumFormat.From(CultureInfo)` copies the culture's own), separated dates take a caller-declared field order (`DateOrder` — the en-US/en-GB `1/7/2026` ambiguity is resolved by declaration, never sniffed), and each binding bridges its platform's culture machinery to both (`NumFormat.From(CultureInfo)` / `DateOrders.From(CultureInfo)` in C#, `DateOrder.from(Locale)` in Java, `DateOrder.from(locale:)` in Swift). Optionality is presentation: `Empty` is a verdict, and the optional doors map it to absent. And before the first cast, every binding can say whether the core loaded and which version it is (`Cast.IsAvailable`/`NativeVersion` in C#, and the same pair in each language's idiom), so a consumer with a fallback gates on a probe rather than catching a load failure.

## Receipts — proven today, on this repo's own tests

- **Allocation-free is asserted by a counting allocator, not a doc comment** — `rust/tests/allocation_free.rs` wraps `#[global_allocator]` around 1000 calls to every door, success *and* failure paths, and demands zero. A fault is a byte span into the caller's input; nothing is ever captured or formatted on the error path.
- **The corpus is the contract** — `corpus/*.json` (seeded from the [Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim) `Norse.Primitives` test suites this project descends from) replays through the Rust core's suite *and* every binding's. All eight replay the full fourteen-file set today — C# through real P/Invoke, Java through FFM downcalls, Ruby through *both* its Magnus and Fiddle backends.
- **Published, to all five registries, and consumable from all eight languages** — every binding is published and installable from its real registry, with the live version on each badge above: [crates.io](https://crates.io/crates/hypercast), [nuget.org](https://www.nuget.org/packages/HyperCast), [PyPI](https://pypi.org/project/hypercast/) (9 abi3 wheels, two of them `musllinux` and one for Pyodide in the browser), [RubyGems](https://rubygems.org/gems/hypercast) (8 gems — one universal Fiddle, seven precompiled Magnus, each fat across Ruby 3.4 and 4.0 since a Magnus extension is tied to one Ruby minor) and [Maven Central](https://central.sonatype.com/artifact/io.github.skunkwerkx/hypercast); Go and Swift resolve from the tag itself (Go's prefixed `go/vX.Y.Z`), PHP from [Packagist](https://packagist.org/packages/skunkwerkx/hypercast). Trusted Publishing/OIDC wherever the registry offers it — no long-lived tokens for NuGet, RubyGems, or PyPI. Every one of the eight was then installed from its real registry into a clean project and run, because "the publish succeeded" and "a consumer can use it" are different claims: identical verdicts across all eight, and Java AOT plus C# AOT and Blazor wasm verified against the published artifacts rather than the working tree.

  Three of the four bugs this project has shipped were found exactly there, in the gap between those two claims, and none of them could fail a build in this repo. v0.0.1's first tag landed four of five registries: Maven died in *our* Gradle config, where `sourcesJar` read `stageNativeLibrary`'s output without declaring the dependency — invisible to CI, which never builds a sources jar. Then v0.0.1's published artifacts turned out to be broken in two ways for AOT and wasm consumers specifically (see the Java AOT and WebAssembly notes below), which v0.0.2 fixes. The recovery protocol — gate the registries that accepted a version, fix the one that didn't, retag — is written into `release.yml`'s header, because a version is only ever burned where it was actually accepted.
- **Fast paths pay for the lenience** — plain-shaped input takes allocation-free fast lanes; only text that actually uses the forgiveness pays for it. Measured with criterion against Rust's own best-in-class (linux-x64, an Intel Core i9-11900H): `cast_uuid` 16.8 ns against the `uuid` crate's 11.3 ns, `cast_i64` 10.1 vs 7.8 ns `str::parse`, `cast_timestamp` 23.1 vs 17.5 ns `time`. In-process against Rust's own parsers these doors trade raw speed for what they *return* (a verdict with a span) and what they *accept*; the speed story belongs to the bindings, where the competition is culture machinery. **Correction on the record:** an earlier version of this line claimed the UUID door beat the `uuid` crate (15.4 vs 17.4 ns). Our number didn't move; `uuid` 1.26 got faster. Receipts get re-run, and this one changed.
- **Fuzzed, and it found real bugs** — a `cargo-fuzz` target (`rust/fuzz/`) drives every door under every format profile and every declared order/precision/epoch, asserting two invariants every binding silently relies on: a door never panics on any byte sequence, and every fault span stays inside the caller's buffer (`offset + len <= input.len()`, which bindings slice with). It caught two real classes within a minute — truncation faults pointing one byte past the input, and `char_len` spans overrunning on text ending mid-UTF-8-character — both since fixed structurally and pinned by `rust/tests/fault_span_invariant.rs` (every corpus input truncated at every byte boundary, through every door) so they fail plain `cargo test`. The following 550M-execution session found nothing.
- **WASM, for the core and in the browser for six of the bindings** — the full Rust test suite (unit tests, the allocation proof, every corpus replay, the fault-span invariant sweeps) passes under `wasmtime` on `wasm32-wasip1`. No clock, no randomness, no dependencies: strictly easier freight than HyperUuid, whose wasm train this rides. In the browser, the crate itself (`wasm32-unknown-unknown`), C# under Blazor, Python under Pyodide, Go under TinyGo, Swift through its WebAssembly SDK and Ruby through ruby.wasm each link the core in, and CI runs every one of them in headless Chrome on every pull request — Python's whole pytest suite included. The JVM keeps the core as a `wasm32-wasip1` module inside the jar, where GraalWasm runs it in-process as a second backend and the Java suite runs a second time through it on every CI leg. See [WebAssembly](#webassembly).
- **C# binding on .NET 11, union-native** — `Verdict<T>` is a real discriminated union: two case arms, no default, and a missing disposition is a **compile error** (CS8509 as error). The whole suite is green including the full corpus replay; source-generated `LibraryImport` only, and the AOT smoke test publishes under `PublishAot` into a genuine native binary that runs every door — proven, not configured.
- **C# vs. the BCL** — BenchmarkDotNet, `[MemoryDiagnoser]`, lenience matched where the BCL has the knob (`AllowThousands`, invariant culture, UTC styles), FFI crossing and UTF-16→UTF-8 transcode *included* in every HyperCast number. Measured on linux-x64 (an Intel Core i9-11900H), .NET 11 RC 1 (in-process toolchain — BDN doesn't know the net11 moniker yet); zero managed allocation on every row, both sides:

  | Door | HyperCast | BCL | Verdict |
  | --- | ---: | ---: | --- |
  | `Cast.DateTime` (`1/7/2026 3:04 PM`) vs `DateTime.TryParse` (en-US) | 48.2 ns | 186.0 ns | **3.9x faster** |
  | `Cast.Timestamp` vs `DateTimeOffset.TryParse` | 52.5 ns | 128.3 ns | **2.4x faster** |
  | `Cast.Duration` vs `TimeSpan.TryParse` | 49.0 ns | 118.7 ns | **2.4x faster** |
  | `Cast.Int32` (grouped) vs `int.TryParse` + `AllowThousands` | 46.2 ns | 48.7 ns | wash |
  | `Cast.Double` vs `double.TryParse` | 51.9 ns | 49.5 ns | wash |
  | `Cast.Uuid` vs `Guid.TryParse` | 40.3 ns | 27.4 ns | 1.5x slower — the expected loss: this door also takes N/B/P/X forms and `urn:uuid:` prefixes |
  | `Cast.Boolean` vs `bool.TryParse` | 21.2 ns | unmeasurable* | honest loss — the BCL's five-byte compare wins; the twenty-lexeme vocabulary is why anyone calls this door |

  \* BDN flags the BCL boolean lane `ZeroMeasurement` — the JIT hoists/folds `bool.TryParse` of a loop-invariant string into nothing, which an FFI call structurally can't match. The loss is real either way and is printed as one.

  Read the table the way it's meant: the wins land exactly where the BCL runs culture machinery, the washes come while *also* carrying notations the BCL has no knob for at any price, and every number crosses a native boundary the BCL doesn't. The losses are where the BCL has a parser built for the one shape being timed — `Guid` here, a currency-styled `decimal` in [the C# README](csharp/) — and a few nanoseconds there is what the door's flexibility costs: a format declared per call, a fault with a byte span, the same verdict in seven languages. Hand a door UTF-8 instead of a `string` and the grouped integer and the double become wins too (28.0 and 35.9 ns). The round-three tabular layer crosses once per chunk and makes the same doors a landslide.
- **Java binding on JDK 25+, union-native the JVM way** — `Verdict<T>` is a `sealed interface` over two records, so a two-arm switch with no default is proven exhaustive by `javac`: an unhandled disposition is a compile failure, the same guarantee the C# binding gets from CS8509-as-error, in Java's own idiom. The suite is green including the full corpus replay through real FFM downcalls with byte-exact fault spans; and **full nanosecond fidelity** — `Instant`/`LocalTime`/`Duration` keep all nine fractional digits, making the JVM the one platform with zero truncation of what the core parses.
- **Java AOT, proven** — the GraalVM Native Image smoke test builds and runs every door plus the exhaustive union switch as a true native binary. Native Image needs the FFM downcall signatures *and* a resources glob registered, and the binding ships both in its `reachability-metadata.json` so a consumer inherits them with zero configuration — the non-negotiable, delivered on both managed platforms. The resources half was missing from v0.0.1: a consumer's native binary built clean and died on first call with "classpath resource not found", while this repo's own smoke test stayed green because it declared the glob itself. That override is gone, so the test now proves the packaged metadata alone is sufficient. Found by building a real native image against the published jar, not against the repo.
- **Java vs. the JDK, and the input no longer copies.** JMH with `-prof gc`, the profile `./gradlew :benchmarks:jmh` runs (linux-x64, Temurin 25): `Cast.timestamp` **50.7 ± 11.7 ns vs 614.8 ± 139.9 ns `Instant.parse`** (12.1x), `Cast.time` **33.8 vs 415.3 ns** (12.3x), `Cast.duration` **54.6 vs 348.6 ns** (6.4x), a grouped `Cast.i32` **32.5 vs 85.8 ns `NumberFormat`** (2.6x). **What moved the numbers, twice:** 0.1.0 removed the `Arena.ofConfined()` every door opened per call; 0.2.0 removed the copy that was left — every downcall is linked `Linker.Option.critical(true)`, so the caller's `byte[]` crosses as a pinned heap segment instead of being copied into a per-thread native staging buffer, every door gained a `MemorySegment` overload for casting a slice of one buffer with nothing copied, and the UUID door reads two big-endian longs instead of sixteen bytes. Three rows lose and are printed as such, each to a JDK method with one shape to read and no boundary to cross: `Boolean.parseBoolean` is unbeatable by construction (the JIT folds a loop-invariant call to nothing), `UUID.fromString` wins by ten nanoseconds (25.6 vs 35.2 ns) and `new BigDecimal(String)` by thirty (22.6 vs 52.1 ns).
- **The full seven-binding roster, corpus-green** — Python (PyO3 native extension, `match`/`case` over the two verdict types), Swift (the core linked in statically + `@convention(c)`, and the strongest union in the roster — a real enum where exhaustive switch is *compiler-mandatory*, no opt-in flag), Go (the core linked in through cgo on Linux, macOS and Windows, and under TinyGo in the browser — the `(value, *Fault)` idiom with `*Fault` as `error`), Ruby (Fiddle fallback + Magnus extension, pattern-matched `Data` classes with Symbol reasons), and PHP (ext-ffi, `Success|Fault` union types over a backed enum). Every one replays every corpus file with byte-exact fault spans — **every suite green on this machine today** — and every one presents its platform's honest fidelity: Ruby and the JVM keep every nanosecond (Ruby's durations are exact `Rational` seconds across the whole ±10,000-year window), Python and PHP truncate to microseconds and say so, Swift's `Duration` is attosecond-backed, and Go returns the protobuf pair because `time.Duration`'s ±292-year ceiling can't hold the window — stated, not wrapped.
- **Benchmarks across the whole spectrum** — each binding carries its ecosystem's own harness, HyperUuid-style: Criterion, BenchmarkDotNet, JMH, `testing.B`, pyperf, benchmark-ips, phpbench, and ordo-one's package-benchmark. The spine of the story, the RFC 3339 timestamp door vs. each platform's own parser (linux-x64, an Intel Core i9-11900H; crossing and transcode included in every HyperCast figure):

  | Binding | HyperCast | Platform parser | Verdict |
  | --- | ---: | ---: | --- |
  | Swift | **51 ns** | 642 ns `Date.ISO8601FormatStyle` | **12.6x faster** — uuid too: 34 vs 523 ns, zero mallocs on every door |
  | Java | **50.7 ns** (`String`), 42.2 ns (`byte[]`) | 615 ns `Instant.parse` | **12.1x faster** |
  | Ruby (Magnus backend) | **447 ns** | 2.97 µs `Time.iso8601` | **6.6x faster** — see below |
  | C# | 52.5 ns (`string`), **41.9 ns** (UTF-8) | 128 ns `DateTimeOffset.TryParse` | **2.4x / 3.1x faster** |
  | PHP | 538 ns | 560 ns `DateTimeImmutable` | level — no new mechanism needed, just the wrapper diet the ext-ffi floor demanded |
  | Ruby (Fiddle fallback) | 3.25 µs | — | a little behind `Time.iso8601`, atop Fiddle's per-call floor; kept as the zero-compile path |
  | Go | 77 ns, 0 allocs | 44 ns `time.Parse(RFC3339Nano)` | honest loss — Go's stdlib RFC 3339 path is exceptional, and every Go door pays cgo's ~50 ns crossing, though no longer a heap allocation on top |
  | Python (PyO3) | 252 ns | 146 ns `fromisoformat` (C-accelerated) | 1.7x slower — see below |
  | Python (retired ctypes) | 3.1 µs | — | the ~1 µs ctypes floor, measured before it was retired — the before-picture that justified going PyO3-only |

  **Python's escape from the interpreted tier is its own receipt**: the losses were never "Python calling native code" — they were *ctypes* (interpreted marshalling, ~1 µs/call, measured). The PyO3 extension (`hypercast._native`) is the Rust core linked directly into a CPython extension — no dlopen, no C-ABI hop, the same `METH_FASTCALL` door the builtins walk — and after the mechanism swap proved out, the ctypes fallback was retired entirely, HyperUuid-style: the abi3 wheels maturin builds *are* the package, one per platform covering every CPython 3.11+, no compiler needed to install (the ctypes row above stands as the measured before-picture). Result: every door roughly an order of magnitude faster than it was over ctypes — timestamp at **252 ns**, i32 at **103 ns** vs `int()`'s 56, the forgiveness doors at ~115-160 ns for grammar the stdlib doesn't sell, and uuid at **262 ns**, three times faster than `uuid.UUID()`'s 798 because the instance is allocated and its slots set through the C API with no `__init__` to run. What the swap did not do is catch `fromisoformat`, a C builtin that reads one shape with no boundary to cross: that row is a 1.7x loss.

  The honest reading, after the redemption arc: **the doors win wherever the platform's own parser is culture machinery, and draw or lose wherever it is a C builtin for the one shape being timed.** Swift, Java, Ruby and C# beat their platform's timestamp parser by 2.4-12.6x; PHP ties its; Go and Python lose to theirs. The interpreted tier's old losses were never the languages; they were the FFI *mechanisms*, measured then replaced: Python got a PyO3 extension (no mechanism left to pay), Ruby got a Magnus extension (Fiddle's microsecond-plus floor gone — the doors now beat `Time.iso8601` by 6.6x while returning exact `Rational` durations), and PHP needed no new mechanism at all — its ext-ffi floor was extension-class all along, so a wrapper diet (flat doors, typed cdef structs, static scratch, `createFromTimestamp`) took its timestamp door down six-fold. Ruby keeps Fiddle as its zero-compile fallback (`HYPERCAST_PURE=1` forces it; both backends replay the corpus green, and a cross-backend agreement spec pins them together), with precompiled platform gems as the vehicle for shipping Magnus without ever making a consumer compile. Benchmarking sagas worth knowing: the Ruby doors were 4.3 µs until per-call `Fiddle::Pointer.malloc` finalizers were hoisted to thread-local scratch, PHP read 20x slow until a loaded Xdebug was caught (`XDEBUG_MODE=off` for all recorded numbers), Swift's first tape was pure measurement-floor quantization until `.kilo` scaling amortized it, and the platform parsers a door is timed against are priced by the machine too, so every ratio here belongs partly to the box it was measured on. Receipts include their own forensics.

- **The messy-feed doors, cross-binding** — the declared-order date/date-time doors exist for text with no stdlib parser at all, so each row pairs against whatever that platform *does* offer for the same string (`1/7/2026 3:04 PM`): a pattern formatter, a culture-aware `TryParse`, or `strptime`. Same machine, same session, linux-x64:

  | Binding | HyperCast | Platform's closest parser | Verdict |
  | --- | ---: | ---: | --- |
  | Swift | **103 ns** | 34 µs `DateFormatter` (same pattern, hoisted, 103 mallocs — measured at 810 ns on 0.1.0's toolchain, tens of microseconds on Swift 6.3.3; see swift/README) | **faster by two orders on this toolchain** |
  | Python | **230 ns** | 4.65 µs `datetime.strptime` | **20x faster** |
  | Java | **56.7 ns** | 392.6 ns `DateTimeFormatter` (`M/d/yyyy h:mm a`) | **6.9x faster** |
  | C# | **48.2 ns** | 186.0 ns `DateTime.TryParse` (en-US) | **3.9x faster** |
  | Go | **73 ns**, 0 allocs | 95 ns `time.Parse` w/ layout | **1.3x faster** — the one Go door that beats the stdlib, a loss in 0.1.0 |
  | PHP | 673 ns | 439 ns `DateTimeImmutable::createFromFormat` | 1.5x slower |
  | Ruby | 904 ns | 901 ns `DateTime.strptime` | level — see below |

  **Ruby's draw is about the carrier, not the parse.** Its timestamp door is 6.6x *faster* than `Time.iso8601` on the same backend; the civil door only draws because building a stdlib `DateTime` with an exact `Rational` second costs more than the entire native call, where `Time` is one cheap `rb_time_nano_new`. (Same reason its `DateTime` shows a `+00:00` offset: a property of the type, not a zone the parse assigned.) Printed because it's real — house rules.

  **Separator detection costs what the core says it costs, once the carrier is thin enough to see it.** `NumFormat.DETECT` resolves `.`/`,` roles structurally per input at ~10 ns in the raw Rust core. In 0.1.0 that vanished inside every binding's per-call overhead (Java 105.1 vs 106.0 ns declared, Swift 399 vs 406); with the carriers since 0.2.0 it shows: Java 73.0 vs 58.5 ns, Swift 75 vs 52, Go 117 vs 104, Ruby (Magnus) 178 vs 170; C# ~9 ns, Python ~15 ns, PHP ~54 ns.

## Platform support

`.github/workflows/ci.yml`'s `build-native` matrix builds the Rust core fresh on each of 5 real-hardware legs, then runs every language's actual test suite, corpus replay included, against that leg's freshly-built native library. A second job does the same for the two musl RIDs inside real Alpine containers, on the language's own official `*-alpine` image. Intel macOS has no leg of its own; see the osx-x64 note under the table.

| Language | linux-x64 | linux-arm64 | linux-musl-x64 | linux-musl-arm64 | osx-x64 | osx-arm64 | win-x64 | win-arm64 | Status |
| --- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | --- |
| [Rust](rust/) (core) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [crates.io](https://crates.io/crates/hypercast) |
| [C#](csharp/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | [NuGet](https://www.nuget.org/packages/HyperCast) |
| [Java](java/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | [Maven Central](https://central.sonatype.com/artifact/io.github.skunkwerkx/hypercast) |
| [Go](go/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | `go get` (git tag) |
| [Swift](swift/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | `.package(url:)` (git tag) |
| [Ruby](ruby/) | ✅ | ✅ | ✅ | ✅ | Fiddle, built | ✅ | ✅ | ✅ | [RubyGems](https://rubygems.org/gems/hypercast) |
| [PHP](php/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | — | [Packagist](https://packagist.org/packages/skunkwerkx/hypercast) |
| [Python](python/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | [PyPI](https://pypi.org/project/hypercast/) |

The cells that are not a plain ✅, and why each is deliberate:

- **osx-x64 (Intel macOS): built, and tested at the core only.** There is no Intel macOS CI leg. The library is cross-compiled on the Apple silicon runner, attested, and shipped in every package exactly as before, and the Rust core's own suite runs on it there under Rosetta 2, which is why that row keeps its ✅. No binding's suite runs on Intel macOS; each still runs on Apple silicon against the same source. Ruby there installs the universal gem and runs on Fiddle, since no precompiled gem is built for it. The leg took 37 minutes against 8 on Apple silicon, for hardware Apple stopped selling in 2023 on runners GitHub retires by Fall 2027.
- **PHP on win-arm64.** PHP has never shipped a native Windows ARM64 build, so it runs as an x64 process there regardless of host CPU and loads the win-x64 library — already exercised for real by the win-x64 leg.

Three bindings link the core into the consumer's executable where they can, instead of loading a shared library at run time. Swift always does, on every platform in the table, so there is nothing to deploy beside the executable: its musl cells are Swift's static Linux SDK, which CI proves with a smoke executable built and run in Swift's own containers (that SDK ships no XCTest), and the same mechanism compiles the binding to WebAssembly; see [WebAssembly](#webassembly). A platform with no prebuilt core fails to compile. Go always does, through cgo on Linux, macOS and Windows, so a binary carries the core for its own platform and loads nothing; building it takes a C compiler, and `CGO_ENABLED=0` or any other target is a compile error — except WebAssembly under TinyGo, which links the same way; see [WebAssembly](#webassembly). C# does for a Native AOT publish, on every RID, so the result is one executable. Everything else — the JIT, the JVM, the interpreters — loads the shared library, as before.

The musl libraries are built so that they depend on musl's libc and nothing else — the unwinder is linked statically — which is what lets them load on a bare `alpine` or `python:alpine` image with no `libgcc` installed. The glibc libraries need glibc 2.34 or newer.

**Runtime floors** follow upstream support: a version that has reached end of life is not a floor. Today that is .NET 11, JDK 25, Go 1.26, Python 3.11, Ruby 3.3 and PHP 8.2, and the musl job runs the PHP, Ruby and Python suites on those oldest versions as well as the newest. Swift's floor is 6.2 for a different reason: it is the first release whose package manager can link a static library, which is how the binding reaches musl and WebAssembly at all. CI runs it on 6.2 as well as 6.4. The crate's own floor is Rust 1.88 (`rust-version` in its manifest, for `let` chains), and CI builds the library on exactly that toolchain.

Every leg also builds the core as a `wasm32-wasip1` module and runs the Java suite a second time through its in-process GraalWasm backend — see [WebAssembly](#webassembly).

## Provenance

Every published artifact across all eight bindings — the package itself where a registry
has one, and the native binaries underneath it either way — carries a GitHub build-provenance
attestation, checkable with `gh attestation verify`. Which flags that needs depends on where
the signing workflow physically lives, not on which registry the artifact ended up in:
artifacts signed directly inside this repo's own `release.yml` — the RubyGems gems and the
published NuGet package — verify with plain `--repo SkunkWerkx/HyperCast`.
Artifacts signed by a reusable workflow hosted in `SkunkWerkx/.github` — the crates.io crate,
the Maven jar, the PyPI wheels, the pre-push NuGet package, every native library (which is the entire
story for Go, Swift, and PHP, none of which has a package-level attestation of its own), and
the `wasm32-wasip1` module that rides inside the jar —
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
  (Blazor, a `wasm32` Rust crate, Python under Pyodide) and the Rust core has to be linked
  into that build by the ecosystem's own toolchain.

Where each of the eight stands, today:

| Binding | Core as wasm inside the binding | Binding compiled to wasm |
| --- | --- | --- |
| Rust | Not applicable: the crate *is* the core, and the wasip1 build of it is the module the JVM embeds. | **Yes.** The full suite passes under [wasmtime](https://wasmtime.dev/) on `wasm32-wasip1`; no clock, no entropy, nothing to stub. On `wasm32-unknown-unknown` the crate needs nothing from a consumer, and `rust/browser-test` runs it in headless Chrome on every PR. See [`rust/README.md`](rust/README.md#webassembly). |
| C# | Not built. | **Yes, on .NET 11+.** `dotnet add package HyperCast` into a Blazor WebAssembly project links the staticlib, exports all 21 doors and applies the exception-handling translation .NET 11 needs, all through the shipped `.targets`. Proven in headless Chromium by `csharp/HyperCast.WasmSmokeTest`, which calls all 22 native entry points; see [`csharp/README.md`](csharp/README.md#webassembly-blazor). |
| Java | **Yes.** [GraalWasm](https://www.graalvm.org/webassembly/); `-Dhypercast.backend=wasm`, or automatic when the jar has no native build for the platform. | Blocked. No Java-to-wasm compiler supports the Foreign Function & Memory API this binding is built on: GraalVM's Web Image (`--tool:svm-wasm`) is labeled experimental and never lists it, and neither TeaVM nor CheerpJ has it. Loading the core as a module of its own and calling it through Web Image's JavaScript interop would work, but that is a second binding with its own glue, not this one; revisit when Web Image is mature and can call or link native code. |
| Ruby | Not built. The `wasmtime` gem ships precompiled only for platforms the universal gem already carries a native library for; anywhere else it compiles from source with a Rust toolchain, so a wasm backend would reach nothing the Fiddle backend does not. | **Yes, through `rbwasm build`.** ruby.wasm links extensions into the interpreter when it is built, so a browser app lists `hypercast-wasm` instead of `hypercast` in the Gemfile it hands `rbwasm build` (ruby_wasm 2.10+, Ruby 3.4 or 4.0). That gem carries the same Magnus extension prebuilt for `wasm32-wasip1`, one archive per Ruby minor, so the consumer needs no Rust toolchain, and it shares an interpreter with HyperUuid's `hyperuuid-wasm`. CI links it into both minors' interpreters and runs a smoke test under Node and in headless Chrome; see [`ruby/README.md`](ruby/README.md#ruby-in-the-browser). |
| Python | Not built. Only platform wheels are published, each carrying the PyO3 extension, and no sdist, so there is no install a wasm backend could fill in for. | **Yes, on [Pyodide](https://pyodide.org/) 314.x.** `await micropip.install("hypercast")` in the browser: the PyO3 extension built for Pyodide's Emscripten target ships to PyPI as a ninth wheel (`cp311-abi3-pyemscripten_2026_0_wasm32`, ~150 KB), the same native backend with no JavaScript bridge. CI runs the binding's whole pytest suite inside Pyodide, under Node and in headless Chrome. Each Pyodide ABI year needs its own wheel; see [`python/README.md`](python/README.md#in-the-browser-pyodide). |
| Go | Not built. The binding links the core in on every platform it supports, so there is no platform for a wasm backend to fill in for. | **Yes, through [TinyGo](https://tinygo.org) 0.42+.** `tinygo build -target=wasm` links the core in from the module's own `staticlib/wasm` archive (the `wasm32-wasip1` build); the core imports nothing, so the page loads one module. CI runs a smoke program covering every ABI shape in headless Chrome on every PR. Stock Go cannot: its wasm toolchain links Go code only, so `GOOS=wasip1`/`js` is a compile error. See [`go/README.md`](go/README.md#in-the-browser-tinygo). |
| Swift | Not built. No wasm engine ships as a Swift package with a stable API, so there is nothing to embed. | **Yes, on Swift 6.2+.** `swift build --swift-sdk` with swift.org's WebAssembly SDK links the core in as a static library (a SwiftPM binary target with a `wasm32-unknown-wasip1` archive), so there is nothing to load. The result is a plain `wasm32-wasip1` command module, so it also runs in the browser through a WASI shim such as [`@bjorn3/browser_wasi_shim`](https://github.com/bjorn3/browser_wasi_shim). CI runs the binding's suite under WasmKit on Swift 6.4, a smoke executable on 6.2, and the same smoke executable in headless Chrome; see [`swift/README.md`](swift/README.md#webassembly). |
| PHP | Not built. There is no maintained wasm engine PHP can embed. | Proven, not shipped. The `ext-php-rs` extension spike loads as a side module (258 KB, 85 KB gzipped) into WordPress Playground's prebuilt `@php-wasm` runtime (PHP 8.5, JSPI) and runs in node and headless Chromium; shipping it means a module per PHP minor and a ~4 GB build image in CI, so it waits on demand. [php/README.md](php/README.md#webassembly) has the full recipe and the two upstream issues it found ([ext-php-rs#800](https://github.com/extphprs/ext-php-rs/issues/800), [wordpress-playground#4377](https://github.com/WordPress/wordpress-playground/issues/4377)). |

The two directions are blocked, where they are blocked, for different reasons. Compiling a
binding to wasm needs the ecosystem's toolchain to link a Rust static library into its own
wasm build. .NET has a supported mechanism for exactly that (`NativeFileReference`, which this
package's `.targets` injects for you), so does Swift from 6.2 (a SwiftPM binary
static-library target), so does TinyGo (cgo, linked by `wasm-ld`), and so does Pyodide,
which loads a CPython extension module built as an Emscripten side module just as CPython
loads a native one, and so does ruby.wasm, whose `rbwasm build` links each gem's extension
into the interpreter it makes. WordPress Playground's PHP does too — it loads a Zend
extension built as an Emscripten side module — which is proven for PHP but not shipped (see
its row); stock Go does not. Java's gap is different in kind: the loading mechanism is not the problem, the compilers that exist have no FFM. The
in-process backend sidesteps all of that rather than climb it, because the engine is the
loader, and it is what a JVM on a platform with no native build falls back to.

### The in-process backend

One artifact, `hypercast.wasm`, built from the same crate with wasi-libc's `malloc`/`free`
exported (two linker flags in `rust/.cargo/config.toml`, no source change), ships beside the
native libraries in the jar. CI builds it on every leg and runs the Java suite a second time
through it. The numbers below were measured through the shipped binding on one linux-x64 box
in one session, native column beside it; [java/README](java/#webassembly-graalwasm) has the
mechanics, the exact loop, and more doors.

| Binding | Engine dependency | `i32`, one call | `timestamp`, one call | Native, same box |
| --- | --- | ---: | ---: | --- |
| Java | `org.graalvm.polyglot:wasm`, `compileOnly`, never in the POM | 134 ns (grouped) on GraalVM CE 25.4 (JIT); 5.0 µs on Temurin 25 (interpreter) | 285 ns (JIT); 7.9 µs (interpreter) | 33 ns / 51 ns |

Two footnotes to that row. On a stock JDK GraalWasm has no JIT and runs the module
interpreted, with a startup warning, so its cost scales with how much wasm the parse
executes; under GraalVM's JIT the same doors sit at 3-9x the FFM downcall, and the wasm path
also survives a GraalVM Native Image build on the jar's own reachability metadata (the Java
README has the receipts); keep the GraalWasm artifacts at the same release as the GraalVM
JDK. And unlike HyperUuid there is no batch door anywhere in this crate to amortize the
crossing behind — a per-cell workload pays it per cell — so the backend is a portability
answer, not a speed option, until round three's chunk layer crosses once per chunk.

Two facts the backend is built on, both learned the hard way in HyperUuid. The host
must take its buffers from the guest's own allocator: a host-picked offset past the data
segments looked free and was not, because dlmalloc claims the tail of the initial memory on
first use, and the next allocation overwrote a buffer mid-way. And every call is serialized
under a lock, because a GraalWasm `Context` is not safe for concurrent use; the native
backends stay lock-free.

## Aspirations — the queue that turns into receipts

Stated the way this project states things: each of these becomes a measured table or a CI matrix row, or it gets cut. Details in [docs/roadmap.md](docs/roadmap.md).

- **Per-binding benchmark passes for the rest of the roster** — Java's is done (the scratch-arena pass landed and the full-length JMH run replaced its directional table), and every new door now carries numbers. What's left is the same discipline applied to the remaining first-wave figures: no number enters this file from a rushed run, and re-runs get published even when they go the wrong way — see the `uuid`-crate correction above.
- **The wasm leg beyond the core** — the C# browser-wasm path is proven in a real browser: `HyperCast.WasmSmokeTest` imports the shipped `build/net11.0/HyperCast.targets` and calls every native entry point in headless Chromium. Building it found a real defect that a successful `wasm-ld` link had hidden: .NET 11 links the new exception-handling encoding while the Rust standard library inside the staticlib uses the legacy one, and the browser refused the module; the `.targets` now fixes that. It runs in CI on every pull request now, beside the Rust crate's own browser tests, Python's whole suite under Pyodide, Go under TinyGo and Swift's smoke executable. What's left is running the same check from a packed `.nupkg` in a separate consumer project, reporting numbers from a browser session, and shipping the browser routes proven for PHP and waiting on Magnus for Ruby (see [WebAssembly](#webassembly)).

**Non-negotiables, every round:** full AOT in .NET and Java; wasm ride-along for the core and bindings; the tabular layer is server-domain (AOT yes, wasm out of scope there, by design).

## Layout

```
corpus/     the shared conformance vectors — the cross-language contract
rust/       the core: 21 cast_* exports plus hypercast_version, zero runtime dependencies; a cdylib and nine static archives
csharp/     the .NET 11 binding: Verdict<T> union, LibraryImport, corpus replay, AOT smoke test
java/       the JDK 25+ binding: sealed-interface union, FFM + GraalWasm backends, corpus replay, Native Image smoke test
python/     the 3.11+ binding: match/case verdicts, PyO3 native extension (abi3 wheels, Pyodide in the browser)
swift/      the SwiftPM binding: enum verdicts (mandatory-exhaustive switch); the core linked in as a static library on every platform
go/         the Go binding: (value, *Fault) verdicts; the core linked in through cgo, and under TinyGo in the browser
ruby/       the 3.3+ binding: pattern-matched Data verdicts, Magnus extension + Fiddle fallback
php/        the 8.2+ binding: Success|Fault union types over ext-ffi (an ext-php-rs extension spike, proven in the browser, unshipped)
docs/       roadmap and parked designs — where this goes, and what's deliberately not built yet
```

## Why "Hyper"

The SkunkWerkx Hyper* series — [HyperUuid](https://github.com/SkunkWerkx/HyperUuid), HyperCast — owes its founding attitude to Casey Muratori and his recent YouTube talks on what "premature optimization" actually meant. Knuth's line gets quoted as a license to never care; Muratori's point is that most slow software was never *optimized badly* — it was **pessimized by default**: allocations nobody needed, layers nobody asked for, work done and thrown away on every call. These libraries are that argument, practiced: allocation-free cores, no runtime bridge, no reflection, fast paths for the common shape — and every performance claim a measured receipt, because the other half of taking performance seriously is refusing to assert it.

## Contributing

Pull requests and issues are welcome. `.github/workflows/ci.yml` builds and tests every binding on every platform — a PR should stay green there before merging.

## License

[MIT](LICENSE)
