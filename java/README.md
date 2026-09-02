# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Maven Central](https://img.shields.io/maven-central/v/io.github.skunkwerkx/hypercast.svg)](https://central.sonatype.com/artifact/io.github.skunkwerkx/hypercast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**Java's own discriminated union — a `sealed interface` over two records — carrying the
verdict of every cast: the value, or a closed reason plus the exact byte span that
offended. A two-arm switch with no default is proven exhaustive by `javac`; an unhandled
disposition is a compile failure.**

Allocation-lean scalar casts — booleans, the full integer family, reals, UUIDs, temporals —
via `java.lang.foreign` (FFM) downcalls straight into the native `libhypercast` Rust core.
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
already holding bytes. `NumFormat.from(Locale)` bridges Java's own locale machinery to the
caller-declared format the native side reads. JVM-flavored fidelity, stated proudly:
`Instant`, `LocalTime`, and `Duration` keep all nine fractional digits, so nothing the core
parses is truncated on the way out — full nanosecond precision, end to end.

## Why not `Integer.parseInt` / `Instant.parse` / the formatter zoo?

1. **Verdicts, not exceptions** — `NumberFormatException`-driven control flow costs a
   throw+fill-in-stack-trace on every bad input; a `Fault` is two ints and a reason, and
   bad data is the *expected* case when the text is untrusted.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, radix prefixes, all five .NET `Guid` text forms, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (all 31 tests green, full twelve-file corpus replay through real
   FFM downcalls with byte-exact fault spans).
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
runs every door plus the exhaustive union switch as a true native binary. Native Image needs two separate registrations
and the jar ships both in its `reachability-metadata.json` under
`META-INF/native-image/io.github.skunkwerkx/hypercast/`, so a consumer inherits them with no
configuration: the FFM downcall *signatures* (reachability is per-signature, not per-function
— four methods here share `(ADDRESS)void`), and a `resources` glob covering `native/*/*`.

The resources half was missing from v0.0.1, and the failure mode is worth knowing because
nothing catches it at build time: Native Image doesn't embed classpath resources unless they
are registered, so `Cast`'s `getResourceAsStream("/native/{rid}/{lib}")` returned null and a
consumer's binary compiled clean, then died on its first call with "classpath resource not
found". The in-repo smoke test was green throughout, because it declared the glob in its own
build file — so it proved only that *this repo* could be configured to work. That override is
gone now; the test passes on the packaged metadata alone, which is the only thing that
actually proves a consumer is fine.

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
