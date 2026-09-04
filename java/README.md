# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Maven Central](https://img.shields.io/maven-central/v/io.github.skunkwerkx/hypercast.svg)](https://central.sonatype.com/artifact/io.github.skunkwerkx/hypercast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**Java's own discriminated union — a `sealed interface` over two records — carrying the
verdict of every cast: the value, or a closed reason plus the exact byte span that
offended. A two-arm switch with no default is proven exhaustive by `javac`; an unhandled
disposition is a compile failure.**

Allocation-lean scalar casts — booleans, the full integer family, reals, exact decimals,
UUIDs, temporals — via `java.lang.foreign` (FFM) downcalls straight into the native `libhypercast` Rust core.
JDK 22 is the floor: FFM is stable, non-preview only from JDK 22 (JEP 454), and the
Verdict union's whole point — sealed interface + record patterns + exhaustive switch — is
stable since 21. The jar bundles a native build for every supported platform
(linux/macOS/Windows × x64/arm64) under `/native/{rid}/` and picks the right one at
runtime, so a consumer adds one dependency and nothing else.

```java
String message = switch (Cast.i32("(1,234)", NumFormat.INVARIANT)) {
    case Success<Integer> s -> "got " + s.value();          // -1234, accounting negative
    case Fault<Integer> f -> f.reason() + " at byte " + f.offset();
};  // no third case: javac checked
```

Door names mirror the native ABI (`i32`, `f64`, `timestamp`, …) so the polyglot surface
reads identically across bindings; every door also takes raw UTF-8 `byte[]` for callers
already holding bytes. `NumFormat.from(Locale)` bridges Java's own locale machinery —
separators and currency symbol — to the caller-declared format the native side reads.
JVM-flavored fidelity, stated proudly: `Instant`, `LocalTime`, and `Duration` keep all nine
fractional digits, so nothing the core parses is truncated on the way out — full nanosecond
precision, end to end — and `Cast.decimal` lands in a `BigDecimal` built straight from the
core's exact sign, magnitude and scale.

## NumFormat: declared, never guessed

Every integer, real and decimal door takes a `NumFormat`: the two separators, the `STYLE_*`
lenience flags, and a currency symbol. `NumFormat.INVARIANT` is `.`/`,` with every lenience
on and no symbol declared; `NumFormat.from(Locale)` reads all three from the locale's own
`DecimalFormatSymbols`, so a US caller gets `$` and a German one `€`:

```java
NumFormat us = NumFormat.from(Locale.US);
Cast.f64("$1,234.50", us);                                     // 1234.5
Cast.i32("($5)", us);                                          // -5: parentheses wrap symbol and digits
Cast.decimal("1.234,50 €", NumFormat.from(Locale.GERMANY));    // 1234.5 — scale 1, exact
```

The symbol is matched whole, once, at either edge of the numeric body — leading (`$5`,
`-$5`, `$ -5`) or trailing (`5 €`, `1.234,50 kr.`) with optional whitespace between it and
the digits — and only while `STYLE_CURRENCY` (part of `STYLE_ALL`) is set: declared without
it, the symbol is the `MALFORMED` span. The three-argument constructor declares no symbol. A
symbol longer than 16 UTF-8 bytes or carrying an ASCII digit or whitespace is a caller bug
(`IllegalArgumentException` at construction), never a verdict.

`Cast.decimal` is the exact door, and a canonical one. No `double` is formed on the way:
`0.1` is one tenth and `50%` is exactly `0.5`. Exact trailing zeros in the fraction are
trimmed, so the scale is minimal — `1.10`, `1.1` and `1.1000` all have a scale of 1, `100`
stays `100` (integer zeros are never touched), and zero is scale 0, never negative. Nothing
but a zero is ever dropped: precision past 2^96−1 or 28 places is `OUT_OF_RANGE`, never
rounded.

## Gating on the core, and what a span counts in

Nothing loads until the first door is called. A consumer with a managed fallback gates on
`Cast.isAvailable()` first: probed once, cached, never throws — `false` when the platform
library will not open, the jar carries no core for this OS/arch, GraalWasm is missing on the
wasm path, or an older core lacks an export this binding was built against. The doors do
not fall back; a core that failed to load is thrown from every door as the failure it was.
`Cast.nativeVersion()` reports the loaded core's `major.minor.patch`, and succeeds exactly
when `isAvailable()` is `true` — the pair that proves the library that resolved is the one
this jar was built against. `Cast.backend()` says which path won.

A `Fault`'s `offset`/`length` count in the input's own unit. Through a `byte[]` or
`MemorySegment` door they are the core's byte span into the UTF-8, verbatim. Through a
`String` door they are UTF-16 code units — the byte span rebased, so
`text.substring(f.offset(), f.offset() + f.length())` is the offending text even when the
input is not ASCII: `Cast.i32("1€", …)` faults at `(1, 1)` as a `String` and `(1, 3)` as
bytes. ASCII input is identical either way and is never touched.

## Why not `Integer.parseInt` / `Instant.parse` / the formatter zoo?

1. **Verdicts, not exceptions** — `NumberFormatException`-driven control flow costs a
   throw+fill-in-stack-trace on every bad input; a `Fault` is two ints and a reason, and
   bad data is the *expected* case when the text is untrusted.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, radix prefixes, all five .NET `Guid` text forms, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (the whole suite green, full thirteen-file corpus replay through
   real FFM downcalls with byte-exact fault spans — and a second time through the GraalWasm
   backend, on every build).
4. **Faster where it matters, and the input no longer copies.** JMH, full-length — 2 forks,
   5 warmup + 10 measurement iterations, 20 samples per row, `-prof gc` for the allocation
   column (linux-arm64, JDK 25). Reproduce: `./gradlew :benchmarks:jmh` — which now runs
   HyperUuid's short profile (1 fork, 3 + 5 iterations of one second) and lands within a
   few percent of the table in under two minutes; the full-length numbers are the ones
   printed.

   | Door | HyperCast | JDK | Verdict |
   | --- | ---: | ---: | --- |
   | `Cast.timestamp` vs `Instant.parse` | 52.5 ± 1.7 ns | 595.9 ± 16.8 ns | **11.4x faster** |
   | `Cast.dateTime` vs `LocalDateTime.parse` (ISO) | 67.5 ± 1.9 ns | 535.0 ± 13.9 ns | **7.9x faster** |
   | `Cast.dateTime` vs a `M/d/yyyy h:mm a` formatter | 64.9 ± 1.3 ns | 386.2 ± 15.7 ns | **6.0x faster** |
   | `Cast.date` (declared order) vs a `M/d/yyyy` formatter | 39.5 ± 2.1 ns | 176.4 ± 9.0 ns | **4.5x faster** |
   | `Cast.time` vs `LocalTime.parse` | 45.1 ± 1.3 ns | 393.4 ± 18.0 ns | **8.7x faster** |
   | `Cast.duration` vs `Duration.parse` | 60.9 ± 5.1 ns | 268.8 ± 16.2 ns | **4.4x faster** |
   | `Cast.i32` (grouped) vs `NumberFormat` | 65.2 ± 2.9 ns | 94.7 ± 3.5 ns | **1.5x faster** |
   | `Cast.f64` vs `Double.parseDouble` | 50.5 ± 1.3 ns | 67.4 ± 6.7 ns | **1.3x faster** |
   | `Cast.f64` (eurozone) vs `NumberFormat` (de-DE) | 86.5 ± 3.3 ns | 157.5 ± 4.0 ns | **1.8x faster** |
   | `Cast.uuid` vs `UUID.fromString` | 38.3 ± 2.5 ns | 45.9 ± 1.5 ns | **1.2x faster** — was a 1.3x loss |
   | `Cast.bool` vs `Boolean.parseBoolean` | 17.7 ± 0.4 ns | 0.61 ns | honest loss — see below |

   The `String` rows above include the UTF-8 encode. A caller already holding bytes skips
   it, and the raw crossing is what round three's chunk layer will pay per cell:

   | Door (UTF-8 in hand) | `byte[]` | `MemorySegment` slice | allocation |
   | --- | ---: | ---: | ---: |
   | `Cast.timestamp` | 46.2 ± 1.7 ns | 49.1 ± 2.0 ns | 40 B (the `Instant` + record) |
   | `Cast.i32` (grouped) | 58.7 ± 2.2 ns | 62.7 ± 2.0 ns | 32 B (the `Integer` + record) |
   | `Cast.uuid` | 30.2 ± 1.2 ns | — | 48 B (the `UUID` + record) |

   Separator detection costs what the core says it costs: `NumFormat.DETECT` on
   `1.234.567,89` measures 100.1 ± 9.0 ns against 86.5 ± 3.3 ns declared — the structural
   resolution pass, now visible because the carrier around it got thin. The
   `DateTimeFormatter.ISO_OFFSET_DATE_TIME` control was unstable in this run (29 µs ± 61 µs
   across forks) and is not quoted; the 0.1.0 tape had it at 690 ns.

**What changed, twice.** 0.1.0's first tuning removed the `Arena.ofConfined()` every door
used to open per call — one `ThreadLocal` holds the out/fault/format segments for the life
of the thread — worth ~100 ns a call. 0.2.0 removed the copy that was left: every downcall
is now linked `Linker.Option.critical(true)`, so the caller's own `byte[]` crosses as a
pinned heap segment instead of being copied into a per-thread native staging buffer, and
that buffer and its arena are gone. Sound because every door is a short, non-blocking
parse over those bytes that never calls back into Java — the profile that option exists
for — and `reachability-metadata.json` registers the option, so the GraalVM Native Image
smoke test proves it under AOT too. Every door also gained a `MemorySegment` overload: slice
one buffer holding many values (a mapped file, a direct buffer, one line of a CSV) and cast
a value out of it with nothing copied. The UUID door reads its sixteen bytes as two
big-endian longs instead of one byte at a time, which is what turned that row from a loss
into a win.

**The honest trade-off:** two rows still lose. `UUID.fromString` beats this door by ~12 ns
— it's pure bit-twiddling with no boundary to cross, and this door also accepts N/B/P/X
forms and `urn:uuid:` prefixes it doesn't. And `Boolean.parseBoolean` is unbeatable by
construction: JIT folds a loop-invariant `parseBoolean` into nothing, which an FFM downcall
structurally can't match — the twenty-lexeme vocabulary is why anyone calls this door. It's
also a native dependency: for plain invariant integers, `Integer.parseInt` is the
reasonable choice.

## AOT

The GraalVM Native Image smoke test (`./gradlew :aot-smoke-test:nativeRun`) builds and
runs every door plus the exhaustive union switch as a true native binary; `-Pwasm` does the
same through the GraalWasm backend (see [WebAssembly](#webassembly-graalwasm)). Native Image needs two separate registrations
and the jar ships both in its `reachability-metadata.json` under
`META-INF/native-image/io.github.skunkwerkx/hypercast/`, so a consumer inherits them with no
configuration: the FFM downcall *signatures* (reachability is per-signature, not per-function
— the twenty-one doors share three shapes, and the version probe's `() -> int` is the fourth),
and a `resources` glob covering `native/*/*`.

The resources half was missing from v0.0.1, and the failure mode is worth knowing because
nothing catches it at build time: Native Image doesn't embed classpath resources unless they
are registered, so `Cast`'s `getResourceAsStream("/native/{rid}/{lib}")` returned null and a
consumer's binary compiled clean, then died on its first call with "classpath resource not
found". The in-repo smoke test was green throughout, because it declared the glob in its own
build file — so it proved only that *this repo* could be configured to work. That override is
gone now; the test passes on the packaged metadata alone, which is the only thing that
actually proves a consumer is fine.

## WebAssembly (GraalWasm)

The jar carries the Rust core a second time, as `native/wasm32-wasip1/hypercast.wasm` — the
exact same twenty-one `cast_*` C exports (and the `hypercast_version` probe), compiled for
WASI preview 1 instead of an OS.
[GraalWasm](https://www.graalvm.org/webassembly/) runs that module inside the JVM, so `Cast`
has a second interop path that needs no platform-specific binary and no FFM downcall: the
polyglot API calls the exports, the input is copied into a guest buffer, and the guest's own
exported `malloc` supplies the 16-byte out-value, 8-byte fault-span and 32-byte `NumFormat`
buffers the core fills.
The seam is one level below the verdict (`Backend`): the wasm class performs the crossing and
fills the same per-thread scratch segments the native call would, and everything above it —
every door, every reader, every exception and message — is one implementation for both paths.
The full test suite runs twice on every build (`./gradlew test testWasm`), corpus replay
included, once through each.

This is not the Java binding compiled *to* WebAssembly (the root README's WebAssembly table
still says why that path is blocked). It is the opposite direction: the Rust core running
*as* WebAssembly inside an ordinary JVM.

**Enabling it.** GraalWasm is deliberately not a dependency of this jar — its POM lists
nothing, so the default FFM path pulls in nothing extra. Add the two artifacts yourself
(`wasm` is a POM-type dependency that fans out into the Truffle runtime):

```kotlin
dependencies {
    implementation("io.github.skunkwerkx:hypercast:<version>")
    implementation("org.graalvm.polyglot:polyglot:25.3.4.1")
    runtimeOnly("org.graalvm.polyglot:wasm:25.3.4.1")
}
```

Then either set `-Dhypercast.backend=wasm` to force it, or do nothing: with the property
unset, `Cast` takes the FFM path when the jar has a native build for the running OS/arch and
falls back to the wasm module when it does not. `-Dhypercast.backend=native` forces FFM and
fails loudly on a platform without a bundled library. `Cast.backend()` reports `"native"` or
`"wasm"` for whichever won. Selecting wasm without GraalWasm on the classpath fails at class
init with a message naming the two artifacts; the `org.graalvm.polyglot` classes are never
loaded otherwise.

**What it costs**, measured with the JMH suite on this repo's linux-arm64 box (WSL2), same
session, three ways: the FFM downcall (`./gradlew :benchmarks:jmh`; GraalVM CE 25.3 and
Temurin 25 agree within noise on that row), then the wasm path (`-Pwasm`) on a GraalVM JDK,
where Truffle JIT-compiles the guest, and on a stock Temurin 25, where it cannot:

| Door | FFM downcall | GraalWasm, GraalVM CE 25.3 (JIT) | GraalWasm, Temurin 25 (interpreter) |
| --- | ---: | ---: | ---: |
| `bool` | 18 ns, 40 B | 174 ns, 488 B | 1.9 µs, 2.6 KB |
| `uuid` | 37 ns, 104 B | 384 ns, 928 B | 6.5 µs, 4.0 KB |
| `f64` | 50 ns, 72 B | 230 ns, 544 B | 7.3 µs, 4.4 KB |
| `timestamp` | 60 ns, 88 B | 354 ns, 936 B | 8.5 µs, 6.1 KB |
| `dateTime` (`1/7/2026 3:04 PM`) | 65 ns, 120 B | 243 ns, 552 B | 10.7 µs, 8.0 KB |
| `i32` (grouped) | 66 ns, 64 B | 237 ns, 480 B | 20.1 µs, 12.0 KB |
| `duration` (ISO) | 61 ns, 72 B | 297 ns, 632 B | 17.3 µs, 11.8 KB |

Two things those rows say plainly. Under GraalVM's JIT the wasm path costs 4-10x the
downcall — the polyglot crossing, the input copy and the lock, with the parse itself
invisible behind them — and the JIT column needed a longer warmup than the FFM suite runs
(`-Pwasm` raises it), because the first seconds measure Truffle compiling the guest rather
than the door. On a stock OpenJDK, GraalWasm has no JIT: the engine prints a fallback-runtime
warning at startup (`-Dpolyglot.engine.WarnInterpreterOnly=false` silences it) and runs the
module interpreted, so the cost scales with how much wasm the parse executes — a two-lexeme
boolean is 100x the downcall, a grouped integer 300x — and the kilobytes per call are the
interpreter's, not this binding's. Nothing in this jar can change which of those a consumer
gets. Unlike HyperUuid there is no batch door to amortize the crossing behind; that is round
three's chunk layer.

**Threading.** A polyglot context does not allow concurrent access from multiple threads, so
every call on the wasm path is serialized on one lock; one context and one module instance
serve the whole process. The FFM path has no lock. A hot, multi-threaded caster should
expect that difference, not just the per-call one.

**Native Image.** The bundled `reachability-metadata.json` registers `WasmBackend`'s
constructor for reflection and the `native/*/*` resource glob already covers the module, so a
consumer's `native-image` build of the wasm path needs no extra configuration on this jar's
account — proven the same way the FFM path is: `./gradlew :aot-smoke-test:nativeRun -Pwasm`
puts GraalWasm on the smoke test's classpath and runs the binary with
`-Dhypercast.backend=wasm`, and every door plus the union switch passes with the binary
reporting `backend: wasm`. The same test without the property builds the FFM-only binary
(16.5 MiB against 50.4 MiB with the Truffle runtime linked in) and reports `backend:
native`.

## Verifying provenance

The published jar carries a GitHub build-provenance attestation, but not one signed by this
repo directly — `release.yml`'s `maven-publish` job hands off to a reusable workflow
(`hyper-publish-maven.yml`) that physically lives in `SkunkWerkx/.github`, and that's the
identity Fulcio records as the signer. `--repo` alone isn't enough; add `--signer-repo`,
or use `--owner` in place of both:

```sh
curl -LO https://repo1.maven.org/maven2/io/github/skunkwerkx/hypercast/X.Y.Z/hypercast-X.Y.Z.jar
gh attestation verify hypercast-X.Y.Z.jar \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
# or: gh attestation verify hypercast-X.Y.Z.jar --owner SkunkWerkx
```

Get the signer-repo wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`,
which reads like a bad signature but is only an identity mismatch — see
[csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance) for the
full breakdown of which artifacts in this project are signed from which repo and why.

## Install

Published to [Maven Central](https://central.sonatype.com/artifact/io.github.skunkwerkx/hypercast)
— no extra repository configuration, since `mavenCentral()` is already in virtually every
Gradle/Maven build:

```kotlin
dependencies {
    implementation("io.github.skunkwerkx:hypercast:<version>")
}
```

The current version is the one on the Maven Central badge above. The jar bundles a native
build for all six platforms and picks the right one at runtime.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.
