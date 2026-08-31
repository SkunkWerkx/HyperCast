# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)

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
   held by the shared corpus (all 28 tests green, full twelve-file corpus replay through real
   FFM downcalls with byte-exact fault spans).
4. **Faster where it matters.** JMH, full-length this time — 2 forks, 5 warmup + 10
   measurement iterations, 20 samples per row, error bars narrow enough to publish
   (linux-arm64, JDK 23). Reproduce: `./gradlew :benchmarks:jmh`.

   | Door | HyperCast | JDK | Verdict |
   | --- | ---: | ---: | --- |
   | `Cast.timestamp` vs `Instant.parse` | 62.9 ± 1.2 ns | 561.2 ± 11.9 ns | **8.9x faster** |
   | `Cast.timestamp` (offset) vs `DateTimeFormatter.ISO_OFFSET_DATE_TIME` | 77.3 ± 1.8 ns | 690.1 ± 11.0 ns | **8.9x faster** |
   | `Cast.dateTime` vs `LocalDateTime.parse` (ISO) | 91.4 ± 4.0 ns | 506.8 ± 20.3 ns | **5.5x faster** |
   | `Cast.dateTime` vs a `M/d/yyyy h:mm a` formatter | 81.8 ± 12.6 ns | 334.9 ± 2.4 ns | **4.1x faster** |
   | `Cast.date` (declared order) vs a `M/d/yyyy` formatter | 44.2 ± 1.8 ns | 167.8 ± 12.3 ns | **3.8x faster** |
   | `Cast.time` vs `LocalTime.parse` | 66.4 ± 0.8 ns | 360.4 ± 7.3 ns | **5.4x faster** |
   | `Cast.duration` vs `Duration.parse` | 73.8 ± 2.2 ns | 247.7 ± 5.8 ns | **3.4x faster** |
   | `Cast.i32` (grouped) vs `NumberFormat` | 67.4 ± 2.9 ns | 87.0 ± 2.0 ns | **1.3x faster** |
   | `Cast.f64` vs `Double.parseDouble` | 60.5 ± 6.4 ns | 63.9 ± 1.3 ns | parity, slightly ahead |
   | `Cast.f64` (eurozone) vs `NumberFormat` (de-DE) | 106.0 ± 3.3 ns | 142.6 ± 2.0 ns | **1.3x faster** |
   | `Cast.uuid` vs `UUID.fromString` | 53.8 ± 3.5 ns | 41.7 ± 1.4 ns | 1.3x slower |
   | `Cast.bool` vs `Boolean.parseBoolean` | 26.3 ± 1.0 ns | 0.43 ns | honest loss — see below |

   **Separator detection is free**: `NumFormat.DETECT` on `1.234.567,89` measures
   105.1 ± 3.0 ns against 106.0 ± 3.3 ns for the same text under a declared eurozone
   format — the structural resolution pass disappears inside the FFM crossing.

**What changed:** every one of those numbers is roughly 100 ns faster than the first
(shortened) run, because the doors no longer open an `Arena.ofConfined()` per call — one
`ThreadLocal` now holds the out/fault/format segments and a reusable input buffer for the
life of the thread. That was the documented next tuning target, and paying it flipped
`f64` and grouped `i32` from losses into wins.

**The honest trade-off:** two rows still lose. `UUID.fromString` beats this door by ~12 ns
— it's pure bit-twiddling with no boundary to cross, and this door also accepts N/B/P/X
forms and `urn:uuid:` prefixes it doesn't. And `Boolean.parseBoolean` is unbeatable by
construction: JIT folds a loop-invariant `parseBoolean` into nothing, which an FFM downcall
structurally can't match — the twenty-lexeme vocabulary is why anyone calls this door. It's
also a native dependency: for plain invariant integers, `Integer.parseInt` is the
reasonable choice.

## AOT

The GraalVM Native Image smoke test (`./gradlew :aot-smoke-test:nativeRun`) builds and
runs every door plus the exhaustive union switch as a true native binary. FFM downcalls
need explicit Native Image registration; the jar ships its `reachability-metadata.json`
under `META-INF/native-image/io.github.skunkwerkx/hypercast/` so every consumer inherits
it.

## Install

Not on Maven Central yet — the release pipeline is staged
(`.github/workflows/release.yml`; the `io.github.skunkwerkx` Central Portal namespace is
already approved) and the artifact ships with the first coordinated tag as
`io.github.skunkwerkx:hypercast`. Until then: clone the repo, `cargo build --release` in
`rust/`, and `./gradlew test` — the build stages the fresh native library automatically.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.
