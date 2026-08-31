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
runtime, the same trick the Go binding's `go:embed` uses.

```java
String message = switch (Cast.i32("(1,234)", NumFormat.INVARIANT)) {
    case Success<Integer> s -> "got " + s.value();          // -1234, accounting negative
    case Fault<Integer> f -> f.reason() + " at byte " + f.offset();
};  // no third case: javac checked
```

Door names mirror the native ABI (`i32`, `f64`, `timestamp`, …) so the polyglot surface
reads identically across bindings; every door also takes raw UTF-8 `byte[]` for callers
already holding bytes. `NumFormat.from(Locale)` bridges Java's own locale machinery to the
caller-declared format the native side reads. JVM-flavored fidelity, stated proudly: this
is one of the two fidelity kings of the roster — `Instant`, `LocalTime`, and `Duration`
keep all nine fractional digits, zero truncation of anything the core parses.

## Why not `Integer.parseInt` / `Instant.parse` / the formatter zoo?

1. **Verdicts, not exceptions** — `NumberFormatException`-driven control flow costs a
   throw+fill-in-stack-trace on every bad input; a `Fault` is two ints and a reason, and
   bad data is the *expected* case when the text is untrusted.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, radix prefixes, all five .NET `Guid` text forms, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (all 21 tests green, full nine-file corpus replay through real
   FFM downcalls with byte-exact fault spans).
4. **Faster where it matters** — JMH (`./gradlew :benchmarks:jmh`), with the honest caveat
   that these came from a deliberately shortened run (wide error bars, directional not
   final): `Cast.timestamp` **~172 ns vs ~667 ns `Instant.parse`** (and ~740 ns
   `DateTimeFormatter.ISO_OFFSET_DATE_TIME`), time-of-day ~146 vs ~417 ns
   `LocalTime.parse`, ISO duration ~163 vs ~267 ns `Duration.parse`.

**The honest trade-off:** the lean doors (f64/uuid/i32) currently lose — not structurally,
but to ~100 ns of per-call `Arena.ofConfined()` setup, the documented next tuning target
(thread-local scratch) before a full-length run replaces the numbers above. And it's a
native dependency: for plain invariant integers, `Integer.parseInt` is the reasonable
choice.

## AOT

The GraalVM Native Image smoke test (`./gradlew :aot-smoke-test:nativeRun`) builds and
runs every door plus the exhaustive union switch as a true native binary. FFM downcalls
need explicit Native Image registration; the jar ships its `reachability-metadata.json`
under `META-INF/native-image/io.github.skunkwerkx/hypercast/` so every consumer inherits
it.

## Install

Not on Maven Central yet — the release pipeline is staged
(`.github/workflows/release.yml`; the `io.github.skunkwerkx` namespace is already approved,
proven by HyperUuid's publishes) and the artifact ships with the first coordinated tag as
`io.github.skunkwerkx:hypercast`. Until then: clone the repo, `cargo build --release` in
`rust/`, and `./gradlew test` — the build stages the fresh native library automatically.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.
