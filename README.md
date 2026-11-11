# HyperCast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/hypercast.svg)](https://crates.io/crates/hypercast)
[![NuGet](https://img.shields.io/nuget/v/HyperCast.svg)](https://www.nuget.org/packages/HyperCast)
[![Maven Central](https://img.shields.io/maven-central/v/io.github.skunkwerkx/hypercast.svg)](https://central.sonatype.com/artifact/io.github.skunkwerkx/hypercast)
[![PyPI](https://img.shields.io/pypi/v/hypercast.svg)](https://pypi.org/project/hypercast/)
[![Go Reference](https://pkg.go.dev/badge/github.com/SkunkWerkx/HyperCast/go.svg)](https://pkg.go.dev/github.com/SkunkWerkx/HyperCast/go)
[![Swift Package](https://img.shields.io/github/v/tag/SkunkWerkx/HyperCast?label=swift%20package&sort=semver)](https://github.com/SkunkWerkx/HyperCast/tags)
[![Gem](https://img.shields.io/gem/v/hypercast.svg)](https://rubygems.org/gems/hypercast)
[![npm](https://img.shields.io/npm/v/@skunkwerkx/hypercast.svg)](https://www.npmjs.com/package/@skunkwerkx/hypercast)
[![CRAN](https://img.shields.io/cran/v/hypercast.svg)](https://cran.r-project.org/package=hypercast)
[![Packagist](https://img.shields.io/packagist/v/skunkwerkx/hypercast.svg)](https://packagist.org/packages/skunkwerkx/hypercast)
[![C/C++](https://img.shields.io/github/v/release/SkunkWerkx/HyperCast?label=c%2Fc%2B%2B&sort=semver)](https://github.com/SkunkWerkx/HyperCast/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**Allocation-free parsers for scalars from untrusted text — booleans, characters, numerics, UUIDs, and temporals. Every parse returns a Verdict: the value, or a closed reason code with the offending span. Never throws, never allocates. Written once in Rust, called in-process from C#, Java, Go, Swift, Ruby, PHP, Python, TypeScript, R, C and C++, with a shared conformance corpus so every binding agrees byte for byte.**

Every runtime already has `TryParse`. What it hands back is a `bool` and a shrug — no reason, no location, and none of the notations untrusted sources actually send. HyperCast's doors return a discriminated union instead: the value, or `Empty` / `Malformed` / `OutOfRange` plus the exact byte span that offended, so the error story is data, not archaeology. And because the engine is one Rust core — reached over a plain C ABI (`P/Invoke`, FFM, JNI, `Fiddle`, R's `.Call`), linked into the language VM as a native extension (PyO3, Magnus, ext-php-rs, napi-rs), or, for Go, translated from its WebAssembly build into Go source — the same logic, the same *bit-for-bit verdicts*, runs in every binding, proven by one corpus every implementation replays.

```csharp
// C# (.NET 10+; the switch is C# 15) — the verdict is a native union; an unhandled case is a compile error
using System.Globalization;
using HyperCast;

var culture = CultureInfo.GetCultureInfo("en-US");
var message = Cast.Int32("(1,234)", NumFormat.From(culture)) switch
{
    Success<int> s => $"got {s.Value}",                  // -1234, accounting negative
    Fault f => $"{f.Reason} at char {f.Offset}",         // no third case: the compiler checked
};
Console.WriteLine(message);
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
| **char** | exactly one character, verbatim and *before* trimming (`" "` is a space, `"6"` the digit six); otherwise one declared code point — decimal `65`, `U+0041`, `0x41`, `&H41`, or an HTML entity `&#65;` / `&#x41;` (the `;` required), ASCII case-insensitive on the prefix. A surrogate or anything past `U+10FFFF` is `OutOfRange` | a Unicode scalar — `char` in Rust; the binding's own character type elsewhere, where C# and Java's UTF-16 `char` turns a scalar past `U+FFFF` into `OutOfRange` |
| **integers** i8–i64, u8–u64 | declared digit grouping, accounting parens `(1,234)`, exponent `1e3`, radix prefixes `0x`/`&H`/`0b` as two's-complement bit patterns (`0xFF` is -1 for i8) — each lenience individually declarable; or `NumFormat.DETECT`, resolving the `.`/`,` roles **structurally** per input | the type's own range, `OutOfRange` beyond it |
| **reals** f32/f64 | declared separators (eurozone `1.234,5`, French NBSP grouping), parens, exponent, percent (`50%` ⇒ 0.5), a declared currency symbol at either edge (`$1,234.50`, `-$5`, `($5)`, `1.234,50 kr.`) — finite values only; or `NumFormat.DETECT`: a repeated separator is grouping (`1.234.567,89`), with both present the rightmost is decimal, a non-3-digit right run is decimal (`3,1415`), a zero-led `0,785` is decimal — and the genuinely ambiguous (`12.185`, `1,000`, and `Rs.1,234` under a declared `Rs.`, and `١,٢٣٤` in declared native digits) is `Malformed` at the separator, **never guessed** | IEEE, overflow-to-∞ is `OutOfRange`, `NaN` text is `Malformed` |
| **decimal** | the real doors' grammar — declared separators, grouping, parens, exponent, percent, currency, or `NumFormat.DETECT` — but no float is ever formed: `0.1` is one tenth, `50%` is exactly `0.5`, and the result is canonical (`1.10` and `1.1` are the same value and come out the same — trailing fraction zeros trimmed, nothing else ever dropped) | exact `{magnitude: u96, scale: 0..=28, negative}` — .NET `decimal`, `BigDecimal`, `decimal.Decimal` per binding; a magnitude or precision that cannot be represented is `OutOfRange`, **never rounded** |
| **uuid** | all five .NET `Guid` formats (D/N/B/P/X) plus `urn:uuid:` / `GUID:` / `UUID:` prefixes | 16 bytes, RFC 9562 order |
| **timestamp** | RFC 3339 with **mandatory** zone, normalized to UTC; separate Unix-epoch door with *declared* precision (s/ms/µs/ns — no magnitude guessing); here and in every time and duration door a fraction past nine digits truncates to the nanosecond, never rounds | protobuf `{seconds: i64, nanos: i32}` — bindings present platform fidelity |
| **date / time** | strict `yyyy-MM-dd` (real calendar, leap days); separated dates (`1/7/2026`, `1.7.2026`) under a **caller-declared field order** — Jan 7th or Jul 1st only because you said which, never guessed (a 4-digit *first* field is structurally a year, so ISO forms parse under any declared order), and undeclared slash dates stay `Malformed`; 24-hour `HH:mm[:ss[.f≤9]]`. The wrong shape is `Malformed`; the right shape naming nothing real (month `13`, `02-30`, `24:00`, a `:60` second) is `OutOfRange` at that field | `{y, m, d}` / nanos-since-midnight |
| **local datetime** | the AM/PM world: `<date> [<time>]` — the declared-order date grammar plus an optional 24-hour or `AM`/`PM` time (`1/7/2026 3:04 PM`, `2026-01-07T15:04:05`, `3 PM` hour-only, `12 AM` = midnight), **no zone read and none invented** — zone-less text names no instant, so zoned text is `Malformed` here and RFC 3339 stays the timestamp door | civil `{y, m, d} + nanos-of-day` — `LocalDateTime` / `DateTime(Unspecified)` / naive `datetime` per binding; fusing a zone is the caller's job |
| **excel serial** | spreadsheet date serials under a **caller-declared epoch** (1900 or 1904 — a workbook-level setting no cell carries), whole part days and fraction time-of-day; the 1900 system's serial `60` is the `1900-02-29` that never existed (Lotus 1-2-3's leap-year bug, kept by Excel for file compatibility) and is `OutOfRange`, exactly as the text `1900-02-29` is — so every serial past it is shifted one day, the arithmetic hand-rolled conversions get wrong | protobuf `{seconds, nanos}` read as UTC — a cell carries no zone and none is invented |
| **typed (a number already held)** | the `f64` a workbook stores for a numeric cell, with no text to parse: as an exact decimal (the shortest decimal that names the double — the digits Excel and LibreOffice write, so `0.1` is one tenth and `0.1 + 0.2` is `0.30000000000000004`), as an Excel serial under a declared epoch, as a serial's time of day, and as a span of days, each time snapped to the fewest fractional-second digits that store as the same double (Excel's `9999-12-31 23:59:59` reads on the second, not 5,424 ns of float noise later). NaN is `Malformed`; what the decimal cannot hold is `OutOfRange`, **never rounded** | the decimal, civil date-time, time-of-day and duration shapes above; a fault's span is empty, there being no text to index |
| **duration** | ISO 8601 fixed components (`P1DT6H30M15.5S` — years/months rejected: not fixed durations), invariant colon form, protobuf JSON seconds (`3.5s`) — with ISO 8601's comma decimal mark accepted in all three shapes (`PT1,5S`, `0:00:01,5`: durations have no grouping, so a comma can only be a decimal mark) | protobuf `{seconds, nanos}`, ±10,000-year window |

White space is Unicode's `White_Space` — the 25 code points, ASCII, U+0085, U+00A0, U+2000 to U+200A, U+202F and U+3000 among them, so what Excel, Word and the web paste reads. Every door ignores it at both ends (a one-character `char` input is taken verbatim first), and nothing but white space is `Empty`. The numeric doors also ignore it around the numeral: after a leading sign, before a trailing sign or a percent symbol, on either side of a currency symbol and inside accounting parentheses (`- 12`, `( 12 )`, `12,5 %`, `1 234,56 €`, the last two under a declared comma decimal separator, the last with a space group separator and `€` declared), but not inside the digits, where it is `Malformed` unless the format declares a space-like group separator. An ASCII space, U+00A0 and U+202F stand for each other there (declare any one as the group separator and the input may use any of the three, mixed); any other group separator stays exact. Between a time and a trailing `AM`/`PM` (or after a leading one) any run of white space reads, so `3:04\u202fPM`, as ICU 72+, JDK 20+ and Apple platforms format en-US, is `15:04`. A declaration the input could never match is refused: a sign or percent symbol, or a decimal separator (number or currency), with white space in it; a currency symbol or a day-period designator with white space at either edge (inside is fine: `F CFA`, `a. m.`). Rust also exports the predicate, `hypercast::is_white_space(char)`.

Culture never lives in the core: numeric doors take a caller-declared format — separators, lenience flags, and everything a culture table has to fill in: its currency symbol and the separators a currency amount uses, its minus and plus signs, whether the sign trails, its percent symbol, its digits — so `NumFormat.From(CultureInfo)` copies the culture's own, and a format that declares nothing reads exactly as the invariant culture (each declared symbol is read *beside* the ASCII default, never instead of it). Separated dates take a caller-declared field order (`DateOrder` — the en-US/en-GB `1/7/2026` ambiguity is resolved by declaration, never sniffed) and, for a 12-hour clock, the culture's own `AM`/`PM` words beside the English ones; each binding bridges its platform's culture machinery to both (`NumFormat.From(CultureInfo)` / `DateFormat.From(CultureInfo)` in C#, `NumFormat.from(Locale)` / `DateTimeFormat.from(Locale)` in Java, `NumFormat.from(locale:)` / `DateFormat.from(locale:)` in Swift). The numeric doors also take the platform's own format object directly, mapped to the core's format at the door: an `IFormatProvider` in C# (so `Cast.Scalar<T>(span, provider)` slots into a `TryParse`-shaped gateway), a `Locale` in Java and Swift, an intl `NumberFormatter` in PHP, and Rails' `separator:`/`delimiter:`/`unit:` keywords in Ruby. Optionality is presentation: `Empty` is a verdict, and the optional doors map it to absent. And before the first cast, every binding can say which version of the core it has, and where a core can be missing, whether it loaded (`Cast.IsAvailable`/`NativeVersion` in C#, and the same pair in each language's idiom), so a consumer with a fallback gates on a probe rather than catching a load failure. Python has only `native_version()`, since a successful import means the extension is there, and R only `hypercast_native_version()`, since the package does not load without its compiled code; PHP's probe is PHP's own `extension_loaded('hypercast')` beside `Cast::nativeVersion()`, since without the extension its classes do not exist. Every binding's spelling of every concept is in [docs/bindings.md](docs/bindings.md). Group separators are judged by the format's grouping policy, Western by default and Indian when declared, so `1,23,456` is `Malformed` until a caller says Indian ([docs/grouping.md](docs/grouping.md)).

## Validation: the checked doors

Untrusted input is cast first and validated second. A value that casts can still be wrong for
the field it fills, so the checked doors validate the cast value in the same call, in the
vocabulary of the OpenAPI 3.2 Schema Object (JSON Schema 2020-12): `minimum`,
`exclusiveMinimum`, `maximum`, `exclusiveMaximum`, `multipleOf` and `enum` for the integer,
real and decimal doors, `minLength`, `maxLength` and `enum` for text. The rules are declared in
code (no schema is parsed), checked once by the core when they are built, and bound to one
door. Every valid OpenAPI 3.2 declaration is accepted: the four bounds are independent keywords
(a longitude is `minimum: -180` with `exclusiveMaximum: 180`), a keyword for another type is
ignored, as JSON Schema ignores it, and an unsatisfiable schema declares and then rejects every
value. A declaration the spec forbids (a `multipleOf` not above zero) or the core cannot hold
(a decimal past 28 digits) is the caller's contract violation, and never means "no rules".

- **Two levels, never blurred.** A fault (`Empty`, `Malformed`, `OutOfRange`) says the input
  is not the type: `W73.98` is no `f64`. A violation says it is one and breaks the schema, and
  carries the value exactly as cast with the keyword and the span: `180` is the `f64` `180.0`,
  which breaks `exclusiveMaximum: 180`.
- **Exact judgement.** Bounds, steps and numeric `enum` entries are exact decimals, never
  floats, and a value is judged by its exact text, so `0.1` is one tenth against a
  `multipleOf` of `0.05`. A real door also judges the float it delivers against each bound
  read as the door reads it, so an exclusive bound is never met, by rounding or by the float:
  `179.99999999999999999` is the `f64` `180.0`, a violation of `exclusiveMaximum: 180`, while
  the decimal door holds it exactly and accepts it.
- **Rounding is declared.** `multipleOf` takes a rounding: `strict` (the default) rejects a
  value off the step, as euro cash at `0.05` should; `midpoint` (ties to even), `floor` and
  `ceiling` snap to it, as a latitude at `0.000001` may, and the verdict says the value was
  adjusted. The rules judge the snapped value, and a violation still carries the value as
  cast: `179.9999996` rounds to `180`, breaks `exclusiveMaximum: 180`, and is reported as
  `179.9999996`.
- **Text is forgiving where the world is messy.** `trim` strips Unicode white space before
  judging, lengths count code points, and an `enum` matches under simple case folding unless
  declared `caseSensitive`, returning the declared spelling and its index: `"male"` against
  `["Female", "Male"]` gives `"Male"`, index 1. A number among a text `enum`'s entries is
  valid and never matches, as JSON Schema's exact equality across types says.
- **Two arms, never three.** The checked verdict is the value (with the adjusted flag and, for
  a text enum, the index), or a rejection: the door's own fault, or a violation. Every binding
  spells it in its own idiom ([docs/bindings.md](docs/bindings.md#constraints)), and every
  binding replays the same constraint corpus.

## Receipts — proven on this repo's own tests

- **Allocation-free is asserted by a counting allocator, not a doc comment** — `rust/tests/allocation_free.rs` wraps `#[global_allocator]` around 1000 calls to every door, success *and* failure paths, and demands zero. A fault is a byte span into the caller's input; nothing is ever captured or formatted on the error path.
- **The corpus is the contract** — `corpus/*.json` (seeded from the [Svartalfheim](https://github.com/NorseArchitecture/Svartalfheim) `Norse.Primitives` test suites this project descends from) replays through the Rust core's suite *and* every binding's: all nineteen files, through C#'s real P/Invoke, Java's JNI on JDK 21 and FFM downcalls on 25, Go's translated core, Swift, Ruby's Magnus *and* Fiddle backends, PHP's extension, Python's, TypeScript's addon and its WebAssembly fallback, R's `.Call` shim, and the C++ header over the C ABI.
- **Fast paths pay for the lenience** — plain-shaped input takes allocation-free fast lanes; only text that actually uses the forgiveness pays for it. Measured with criterion against Rust's own best-in-class (linux-x64, an Intel Core i9-11900H): `cast_timestamp` 14.5 vs 17.1 ns `time`, `cast_uuid` 12.8 ns against the `uuid` crate's 11.1 ns, `cast_i64` 8.7 vs 7.1 ns `str::parse`. The timestamp door is the one ahead of its Rust counterpart; elsewhere, in-process against Rust's own parsers, these doors trade a few nanoseconds for what they *return* (a verdict with a span) and what they *accept*, and the speed story belongs to the bindings, where the competition is culture machinery ([the scoreboard](#the-scoreboard)).
- **Fuzzed, with what it finds pinned as tests** — a `cargo-fuzz` target (`rust/fuzz/`) drives every door under every format profile and every declared order/precision/epoch, asserting two invariants every binding silently relies on: a door never panics on any byte sequence, and every fault span stays inside the caller's buffer (`offset + len <= input.len()`, which bindings slice with). The two classes of bug it has caught — truncation faults pointing one byte past the input, and `char_len` spans overrunning on text ending mid-UTF-8-character — are fixed structurally and pinned by `rust/tests/fault_span_invariant.rs` (every corpus input truncated at every byte boundary, through every door) so they fail plain `cargo test`.
- **Published to every registry, and consumed from each** — crates.io, nuget.org, PyPI (17 wheels — `abi3` and free-threaded `abi3t` on each of eight platforms, two of them `musllinux`, and one for Pyodide in the browser — and the sdist), RubyGems (9 gems — one universal Fiddle, seven precompiled Magnus, each fat across Ruby 3.4 and 4.0 since a Magnus extension is tied to one Ruby minor, and `hypercast-wasm` for ruby.wasm in the browser), Maven Central and npm (ten packages: the root, eight platform addons and the WebAssembly fallback); R goes to CRAN as a source package, by hand, from the release's attested bytes; Go and Swift resolve from the tag itself (Go's prefixed `go/v<version>`), PHP through PIE from Packagist, and the C and C++ archives, PHP's extension zips and Swift's core zips are on the GitHub Release. Trusted Publishing/OIDC wherever the registry offers it — no long-lived tokens for crates.io, NuGet, RubyGems, PyPI or npm; Maven Central, which has no Trusted Publishing, takes a token. Each package 1.0.0 published was then installed from its real registry into a clean project and run, because "the publish succeeded" and "a consumer can use it" are different claims: identical verdicts across all of them, and Java AOT plus C# AOT and Blazor wasm verified against the published artifacts rather than the working tree. npm, CRAN, the C archives, the PIE and Swift zips and the JNI jar first publish in 1.1.0 and get the same check then. How each one is packaged: [docs/platforms.md](docs/platforms.md#publishing).

  Several of the bugs this project has shipped were found in that gap, where no build in this repo could have failed on them; the CHANGELOG records each. The recovery protocol — a dispatch at the same tag from the preserved, attested packages, uploading only what a registry has not already accepted, which every release runs once as a no-op — is in [`.github/RELEASING.md`](.github/RELEASING.md), because a version is only ever burned where it was actually accepted.
- **WASM, for the core and in the browser** — the full Rust test suite (unit tests, the allocation proof, every corpus replay, the fault-span invariant sweeps) passes under `wasmtime` on `wasm32-wasip1`. No clock, no randomness, and one dependency, simdutf8, which is `no_std` and takes the core's own SIMD path only when built with simd128: strictly easier freight than HyperUuid, whose wasm train this rides. Eight of these languages compile into a WebAssembly app with the core inside it: Rust (`wasm32-unknown-unknown` in the browser, `wasm32-wasip1`), C# under Blazor, Python under Pyodide, Ruby through ruby.wasm, Go under stock Go and TinyGo alike (its core is Go source), Swift through its WebAssembly SDK, TypeScript through its fallback package's browser entry, and C from the wasi-wasm32 archive (packed and layout-checked, not run: the same library Swift's WebAssembly build links and tests); CI runs each browser target in headless Chrome. PHP's is proven for a raw build of the crate and unshipped, Java's is blocked on its compilers, and R's is not built. See [WebAssembly](#webassembly).
- **C# binding on .NET 10 and 11, union-native** — `Verdict<T>` is a real discriminated union: two case arms, no default, and a missing disposition is a **compile error** (CS8509 as error); on C# 14, `Match` and `Switch` take both cases instead. The whole suite is green including the full corpus replay; source-generated `LibraryImport` only, and the AOT smoke test publishes under `PublishAot` into a genuine native binary that runs every door — proven, not configured.
- **Java binding on JDK 21+, union-native the JVM way** — `Verdict<T>` is a `sealed interface` over two records, so a two-arm switch with no default is proven exhaustive by `javac`: an unhandled disposition is a compile failure, the same guarantee the C# binding gets from CS8509-as-error, in Java's own idiom. One multi-release jar reaches the core through JNI on JDK 21–24 and FFM downcalls on 25+, and the suite is green on both, including the full corpus replay with byte-exact fault spans; and **full nanosecond fidelity** — `Instant`/`LocalTime`/`Duration` keep all nine fractional digits.
- **Java AOT, proven** — the GraalVM Native Image smoke test builds and runs every door plus the exhaustive union switch as a true native binary. Native Image needs the FFM downcall signatures *and* a resources glob registered, and the binding ships both in its `reachability-metadata.json` so a consumer inherits them with zero configuration — the non-negotiable, delivered on both managed platforms. The smoke test declares no glob of its own, so it proves the packaged metadata alone is sufficient.
- **The rest of the roster, corpus-green** — Python (PyO3 native extension, `match`/`case` over the two verdict types), Swift (the core linked in statically + `@convention(c)`, and the strongest union in the roster — a real enum where exhaustive switch is *compiler-mandatory*, no opt-in flag), Go (the core compiled as Go source, no cgo, every GOOS and GOARCH — the `(value, *Fault)` idiom with `*Fault` as `error`), Ruby (Fiddle fallback + Magnus extension, pattern-matched `Data` classes with Symbol reasons), PHP (an ext-php-rs extension installed by PIE, `Success|Fault` union types over a backed enum), TypeScript (a napi-rs addon for Node and Bun, `{ ok: true, value } | { ok: false, fault }` narrowed on `ok`, never throwing for any argument), R (every door vectorised, a data-frame verdict with one row per element and `NA` in as `NA` out), C (a result code and a fault out-param, nothing that aborts or longjmps) and C++ (a header-only, `noexcept` `Verdict<T>` that builds with exceptions off). Every one replays every corpus file with byte-exact fault spans, and every one presents its platform's honest fidelity: Ruby and the JVM keep every nanosecond (Ruby's durations are exact `Rational` seconds across the whole ±10,000-year window), TypeScript, C and C++ hand back the core's own nanosecond fields, R adds them as exact columns beside its `POSIXct` and `difftime`, Python and PHP truncate to microseconds and say so, Swift's `Duration` is attosecond-backed, and Go returns the protobuf pair because `time.Duration`'s ±292-year ceiling can't hold the window — stated, not wrapped.

## The scoreboard

Each binding carries its ecosystem's own harness, HyperUuid-style: Criterion, BenchmarkDotNet, JMH, `testing.B`, pyperf, benchmark-ips, phpbench, ordo-one's package-benchmark, mitata and bench. The spine of the story, the RFC 3339 timestamp door vs. each platform's own parser, each row from its binding's own benchmark session, crossing and transcode included in every HyperCast figure. The receipts behind every row, and the culture-machinery tables where the doors win widest, are in [docs/benchmarks.md](docs/benchmarks.md).

| Binding | HyperCast | Platform parser | Verdict |
| --- | ---: | ---: | --- |
| Swift | **37 ns** | 627 ns `Date.ISO8601FormatStyle` | **17x faster** — uuid too: 31 vs 479 ns, zero mallocs on every door |
| Java § | **50.4 ns** (`String`), 41.8 ns (`byte[]`) | 554 ns `Instant.parse` | **11.0x faster** (the FFM path, JDK 25) |
| Ruby (Magnus backend) | **404 ns** | 2.70 µs `Time.iso8601` | **6.7x faster** |
| C# † | 57.0 ns (`string`), **40.7 ns** (UTF-8) | 126 ns `DateTimeOffset.TryParse` | **2.2x / 3.1x faster** |
| PHP ‡ | 538 ns | 560 ns `DateTimeImmutable` | level |
| Ruby (Fiddle fallback) | 2.85 µs | — | a little behind `Time.iso8601`, atop Fiddle's per-call floor; kept as the zero-compile path |
| Python (PyO3) | 252 ns | 146 ns `fromisoformat` (C-accelerated) | 1.7x slower |
| Go, on an Apple M6 | 40.5 ns, 0 allocs | 20.8 ns `time.Parse(RFC3339Nano)` | 1.9x slower — Go's stdlib RFC 3339 path is exceptional |
| R, on an Apple M6 | **27.14 µs** per 1000 strings | 3.83 ms `as.POSIXct` | **141x faster** |
| TypeScript | not measured yet | | |

Every row but Go's and R's was measured on linux-x64 on an Intel Core i9-11900H; Go's and R's on an Apple M6 (Mac18,5) under macOS, Go on 2026-11-05 and R on 2026-11-01, so the two groups are not comparable with each other. † Measured on .NET 11 RC 1 with BenchmarkDotNet's in-process toolchain, before the harness moved to running each benchmark in its own process; the figures stand until it is run again. ‡ Predates the extension: it measured 1.0's FFI package, recorded on 2026-10-01. § The `String` figure predates the `String` doors' lone-surrogate fix, which [java/README.md](java/README.md#why-not-integerparseint--instantparse--the-formatter-zoo) measures on its own; the `byte[]` door is unchanged.

The honest reading: **the doors win wherever the platform's own parser is culture machinery, and draw or lose wherever it is a C builtin for the one shape being timed.** Swift, Java, Ruby and C# beat their platform's timestamp parser by 2.2-17x, and R's vectorised door beats `as.POSIXct` by two orders; PHP tied its, measured before the extension; Go and Python lose to theirs. What the interpreted tier pays for is the FFI *mechanism*, not the language, which is why Python, Ruby, PHP and TypeScript link the core into an extension instead of crossing a generic FFI ([why](docs/benchmarks.md#where-the-wins-come-from)), and why Go compiles it as Go source and crosses nothing at all.

## Platform support

`.github/workflows/ci.yml`'s `core` job builds the Rust core fresh on each of 5 real-hardware legs and the two musl RIDs (in real Alpine containers), and each language's `test-<binding>` job then runs that language's actual test suite, corpus replay included, on the same hosts against that leg's freshly-built native library (Go's against the committed translation of the same core; the musl suites on the language's own official `*-alpine` image) — not just that it compiles. A pull request runs the bindings it touched; a dispatch runs them all. Intel macOS has no leg of its own; see the osx-x64 note under the table.

| Language | linux-x64 | linux-arm64 | linux-musl-x64 | linux-musl-arm64 | osx-x64 | osx-arm64 | win-x64 | win-arm64 | Status |
| --- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | --- |
| [Rust](rust/) (core) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [crates.io](https://crates.io/crates/hypercast) |
| [C#](csharp/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | [NuGet](https://www.nuget.org/packages/HyperCast) |
| [Java](java/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [Maven Central](https://central.sonatype.com/artifact/io.github.skunkwerkx/hypercast) |
| [Go](go/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | `go get` (git tag) |
| [Swift](swift/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | `.package(url:)` (git tag) |
| [Ruby](ruby/) | ✅ | ✅ | ✅ | ✅ | Fiddle, built | ✅ | ✅ | ✅ | [RubyGems](https://rubygems.org/gems/hypercast) |
| [PHP](php/) | ✅ | ✅ | ✅ | ✅ | — | ✅ | ✅ | — | `pie install` ([Packagist](https://packagist.org/packages/skunkwerkx/hypercast); zips on the GitHub Release) |
| [Python](python/) | ✅ | ✅ | ✅ | ✅ | built | ✅ | ✅ | ✅ | [PyPI](https://pypi.org/project/hypercast/) |
| [TypeScript](typescript/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | [npm](https://www.npmjs.com/package/@skunkwerkx/hypercast) |
| [R](r/) | source | source | source | source | source | source | source | experimental | [CRAN](https://cran.r-project.org/package=hypercast) |
| [C, C++](c/) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | GitHub Release (one archive per RID) |

The cells that are not a plain ✅, and why each is deliberate:

- **osx-x64 (Intel macOS): built, and tested at the core, in TypeScript, in Java and in C only.** There is no Intel macOS CI leg. The library is cross-compiled on the Apple silicon runner, attested, and shipped in every package that carries one, and the Rust core's own suite runs on it there under Rosetta 2, which is why that row keeps its ✅. TypeScript's and Java's are the two binding suites that run on Intel macOS, TypeScript's on an x64 Node under Rosetta 2 against an addon cross-built there, Java's on x64 Temurin 21 and 25 under Rosetta 2 (the JNI path and the FFM path), and the C archive's smoke consumers run there too, which is why those rows have a ✅; every other one still runs on Apple silicon against the same source. Ruby there installs the universal gem and runs on Fiddle, since no precompiled gem is built for it. The leg took 37 minutes against 8 on Apple silicon, for hardware Apple stopped selling in 2023 on runners GitHub retires by Fall 2027.
- **PHP on osx-x64 and win-arm64: no zip.** Intel macOS: Homebrew bottles no PHP for it any more, and no leg compiles a PHP to build against. Windows on ARM: PHP has never shipped a native Windows ARM64 build, so it runs as an x64 process there regardless of host CPU and takes the win-x64 zip — already exercised for real by the win-x64 leg.
- **TypeScript on Bun.** The suite runs on Node on every leg and in both musl containers, and on Bun everywhere but win-arm64, where Bun is not yet verified, and osx-x64, whose cross-build leg runs Node alone.
- **C and C++: one archive per RID, plus Android, iOS, Mac Catalyst and WebAssembly.** CI packs every archive from the run's own libraries with the release's packer, then on each RID's runner builds the C and C++ smoke consumers against it through CMake, and the C one through pkg-config, shared and static, and replays the corpus through the C++ header (osx-x64 under Rosetta 2, musl in Alpine). The iOS, iOS simulator and Mac Catalyst archives are static only and built against on a Mac: linked for a device, run in the simulator, and run as Mac Catalyst on both architectures. The android-arm64, android-x64 and wasi-wasm32 archives are packed but not run: no runner executes them, and their libraries are the ones C# and Swift already test there.
- **R: source.** R ships a source package: CRAN compiles it for each platform it serves and builds its own Windows and macOS binaries, and a source install needs Rust 1.89 or newer. CI runs `R CMD check --as-cran` on linux-x64, linux-arm64, osx-arm64 and win-x64, on the current R and, on linux-x64, the previous minor too; the musl and osx-x64 cells are source builds no job here checks. On win-arm64 the build picks `aarch64-pc-windows-gnullvm`; R on Windows ARM64 is experimental, and that build is untested.

The musl libraries are built so that they depend on musl's libc and nothing else — the unwinder is linked statically — which is what lets them load on a bare `alpine` or `python:alpine` image with no `libgcc` installed. The glibc libraries need glibc 2.34 or newer. The TypeScript addon links Rust's std, so that floor is not its floor; its own is not stated here yet. The core's suite, corpus included, also runs on big-endian s390x under QEMU.

**Runtime floors** follow upstream support: a version that has reached end of life is not a floor. Today that is Go 1.26, Python 3.11, Ruby 3.3, PHP 8.3, Node 22.14 (`^22.14.0 || >=23.6.0`, the first releases with Node-API 10) and .NET 10, and the musl job runs the Ruby, Python and TypeScript suites on those oldest versions as well as the newest (for Node, on every major `engines` admits); Go's suites run on the version `go.mod` declares. PHP's extension is built per minor, so a release run builds and tests 8.3, 8.4 and 8.5 on every platform, and a pull request the newest on linux-x64 and 8.3 on win-x64. R follows CRAN's window instead: CI checks the current release and the previous minor, and the package declares R 4.2. C# covers Windows, Linux, macOS and Blazor WebAssembly on .NET 10; iOS, Mac Catalyst and Android need .NET 11 (the union `switch` itself is C# 15, .NET 11's default and `LangVersion` preview on .NET 10). JDK 21 sits above that line, though 17 is still supported upstream (Temurin to 2027-10-31): Temurin builds no JDK 17 for Windows arm64 or musl arm64, two of the jar's eight platforms. Swift's floor is 6.2 for a different reason: it is the first release whose package manager can link a static library, which is how the binding reaches musl and WebAssembly at all. CI runs it on 6.2 as well as 6.4. The crate's own floor is Rust 1.89 (`rust-version` in its manifest, the floor the Hyper* crates share), and CI checks the library on exactly that toolchain.

How each binding reaches the core (linked into the executable for Swift and C#'s Native AOT, compiled as Go source for Go, inside the extension for Python, Ruby, PHP, TypeScript and R), iOS, Mac Catalyst and Android, and how each one is published: [docs/platforms.md](docs/platforms.md).

## WebAssembly

Eight of these languages compile into a WebAssembly app with the core inside it: Rust on
`wasm32-unknown-unknown` and `wasm32-wasip1`, C# in Blazor WebAssembly (.NET 10+), Python under
Pyodide, Ruby through `rbwasm build`, Go under stock Go (`GOOS=js`, `GOOS=wasip1`) and TinyGo
alike, since its core is Go source, Swift with swift.org's WebAssembly SDK, TypeScript through
its fallback package's browser entry, and C from the wasi-wasm32 archive. TypeScript is also the
one binding that runs the core as wasm inside a native process: its fallback package, where no
native addon fits. PHP's is proven for a raw build of the crate and unshipped, Java's is blocked
on its compilers, and R's is not built. [docs/platforms.md](docs/platforms.md#webassembly) has
the table, binding by binding, and why each gap is where it is.

## Provenance

Every file a release ships verifies from the tag alone. One release attestation, signed by
this repository's own `release.yml` at the tag, names every registry package (the `.nupkg` as
packed and as nuget.org serves it, the gems, the wheels and the sdist, the crate, the Maven files, the ten
npm tarballs and the R source tarball), Swift's two zips, the fifteen C archives and the 36 PHP
extension zips on the GitHub Release, and its predicate is the release inventory: every file's path, size, SHA-256, the commit
it was built from and the repository that signed its build provenance. One command, any file:

```sh
gh attestation verify <file> --repo SkunkWerkx/HyperCast \
  --source-ref refs/tags/v<version> --predicate-type https://github.com/SkunkWerkx/.github/predicates/release/v1
```

Which identity signs each artifact's build provenance, and the gates on the way in: [docs/provenance.md](docs/provenance.md).

## Aspirations — the queue that turns into receipts

Stated the way this project states things: each of these becomes a measured table or a CI matrix row, or it gets cut. Details in [docs/roadmap.md](docs/roadmap.md).

- **Benchmark passes for what changed** — no number enters this file from a rushed run, and re-runs get published even when they go the wrong way, as the CHANGELOG's 0.7.0 `cast_uuid` entry records. What is owed: TypeScript's first figures (the harness is in `typescript/bench/`), PHP's extension on the machine its FFI figures came from, the C# tables out of process, Go on linux-x64, and every remaining linux-x64 table on the machine that now measures Go and R.
- **The wasm leg beyond the core** — the C# browser-wasm path is proven in a real browser: `HyperCast.WasmSmokeTest` imports the shipped `build/net10.0/HyperCast.targets` and calls every native entry point in headless Chromium, on .NET 10 and 11, in Release and in an unstripped Debug build. It catches what a successful `wasm-ld` link hides: a module the browser refuses to compile, as one mixing .NET 11's exception-handling encoding with the legacy one in Rust's prebuilt sysroot was until the archive was built without it. CI runs it beside the Rust crate's own browser tests, Python's whole suite under Pyodide, Go's suite under stock Go and TinyGo, TypeScript's corpus through its fallback's browser entry and Swift's smoke executable. What's left is running the same check from a packed `.nupkg` in a separate consumer project, reporting numbers from a browser session, and shipping the browser route proven for PHP (see [WebAssembly](#webassembly)).

**Non-negotiables, every round:** full AOT in .NET and Java; wasm ride-along for the core and bindings, the tabular layer ([HyperTabular](https://github.com/SkunkWerkx/HyperTabular)) included.

## Layout

```
corpus/     the shared conformance vectors — the cross-language contract
rust/       the core: 35 cast_* exports plus hypercast_version, one runtime dependency (simdutf8); a cdylib (and two for Android),
            fifteen static archives (four for iOS and Mac Catalyst, two for Android) and the Blazor archive;
            the Python, Ruby and PHP extensions are its features
csharp/     the .NET 10 and 11 binding: Verdict<T> union, LibraryImport, corpus replay, AOT smoke test, Blazor wasm
java/       the JDK 21+ binding: sealed-interface union, JNI on 21–24 and FFM on 25+ from one multi-release jar,
            corpus replay, Native Image smoke test
java/jni/   the JNI library, a no_std Rust cdylib linking the core in
python/     the 3.11+ binding: match/case verdicts, PyO3 native extension (abi3 and free-threaded wheels, the sdist,
            Pyodide in the browser)
swift/      the SwiftPM binding: enum verdicts (mandatory-exhaustive switch); the core linked in as a static library
            on every platform
go/         the Go binding: (value, *Fault) verdicts; the core as Go source, translated by wasm2go from its
            wasm32-wasip1 build; stock Go and TinyGo alike, browser and WASI included
ruby/       the 3.3+ binding: pattern-matched Data verdicts, Magnus extension + Fiddle fallback
php/        the 8.3+ binding's suite, benchmarks and stubs: the extension itself is rust/'s php feature (ext-php-rs),
            installed by PIE
typescript/ the Node 22.14+ and Bun binding: a napi-rs addon with the core linked in, one npm platform package per
            RID, and the WebAssembly fallback package loaded where none fits
r/          the R 4.2+ binding: .Call into a C shim over the core's static library, built from vendored crates
include/    hypercast.h, the C header, generated by cbindgen from rust/src/ffi.rs, and hypercast.hpp, the
            hand-written C++17 header over it
c/          the C archives' CMake and pkg-config files, and the C and C++ smoke consumers
docs/       the cross-binding parity table, benchmarks, platforms and provenance in depth, the roadmap and parked
            designs — where this goes, and what's deliberately not built yet
```

## Why "Hyper"

The SkunkWerkx Hyper* series — [HyperUuid](https://github.com/SkunkWerkx/HyperUuid), HyperCast, [HyperTabular](https://github.com/SkunkWerkx/HyperTabular) — owes its founding attitude to Casey Muratori and his recent YouTube talks on what "premature optimization" actually meant. Knuth's line gets quoted as a license to never care; Muratori's point is that most slow software was never *optimized badly* — it was **pessimized by default**: allocations nobody needed, layers nobody asked for, work done and thrown away on every call. These libraries are that argument, practiced: allocation-free cores, no runtime bridge, no reflection, fast paths for the common shape — and every performance claim a measured receipt, because the other half of taking performance seriously is refusing to assert it.

## Contributing

Pull requests and issues are welcome. `.github/workflows/ci.yml` builds the core on every host and runs each binding's suite when the pull request could have broken it (its own tree, the core or the workflows changed); `pr-board` is the one status to keep green before merging.

## License

[MIT](LICENSE). The simple case folding that `enum` and month-name matching use is generated from
the Unicode Character Database's `CaseFolding.txt`, © Unicode, Inc., under the
[Unicode License v3](LICENSE-UNICODE). Packages name both, and carry both texts where the
registry's format lets them. PHP's release zips hold one file by PIE's rule, and Swift's hold the
library, so for those two this paragraph is the notice.
