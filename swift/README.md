# HyperCast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)

**The strongest union in this roster: `Verdict<T>` is a real Swift `enum`, and an
exhaustive `switch` over it is *compiler-mandatory* — not an opt-in analyzer flag, not a
review convention. The value, or a closed reason plus the exact byte span that offended.**

Allocation-lean scalar casts — booleans, the full integer family, reals, UUIDs, temporals —
calling directly into the native `libhypercast` Rust core via `dlopen`/`dlsym`
(`LoadLibraryW`/`GetProcAddress` on Windows) and `@convention(c)` function-pointer casts,
no shim layer. The package bundles a native build for every supported platform as SwiftPM
resources under `NativeLibs/{rid}/` — `binaryTarget`/XCFramework is Apple-only, so the
resource-bundle approach is what covers Linux and Windows too — and `NativePlatform`
resolves the RID at compile time.

```swift
switch try Cast.i32("(1,234)", format: .invariant) {
case .success(let value): print("got \(value)")          // -1234, accounting negative
case .fault(let fault): print("\(fault.reason) at byte \(fault.offset)")
}   // no default: the compiler mandates both arms, and only both arms
```

Door names mirror the native ABI (`i32`, `f64`, `timestamp`, …); every door also takes raw
UTF-8 `[UInt8]` for callers already holding bytes. Swift-flavored fidelity: `Duration` is
attosecond-backed, so the duration door keeps every nanosecond the core parses; the
`Duration` presentation is also why `Package.swift` carries a `.macOS(.v13)` floor (Linux
has no availability gates — this only sets the Darwin deployment target).

## Why not `Int32("...")` / `ISO8601FormatStyle`?

1. **Verdicts with location** — Swift's failable initializers return `nil` with no reason
   and no span; formatters throw.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, radix prefixes, all five .NET `Guid` text forms plus
   `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (24 tests green, full nine-file corpus replay with byte-exact
   fault spans).
4. **Faster on the culture-machinery doors** — numbers from ordo-one's package-benchmark
   (linux-arm64, p50, `swift package benchmark run` in `Benchmarks/`): timestamp **278 ns
   vs 837 ns `Date.ISO8601FormatStyle`** (3.0x), uuid **225 ns vs 629 ns
   `UUID(uuidString:)`** (2.8x), and the messy civil shape **326 ns vs 810 ns** for a
   `DateFormatter` with the equivalent `M/d/yyyy h:mm a` pattern (2.5x — and that
   formatter is hoisted out of the loop, so its notorious construction cost isn't in the
   number). The declared-order date door is 287 ns.

   Separator detection is free: `1.234.567,89` under `.detect` is 399 ns against 406 ns
   for the same text under a declared eurozone format.

**The honest trade-off:** a native dependency carried as a package resource, a dlopen at
first use, and an FFI crossing per call — for plain invariant integers, `Int32("...")` is
the reasonable choice. (Benchmark forensics worth knowing: the first Swift tape was pure
measurement-floor quantization until `.kilo` scaling amortized it — receipts include their
own archaeology.)

## Install

SwiftPM resolves a `.package(url:)` dependency straight from a git tag — that will be the
whole publish story here (the repo-root `Package.swift` exists exactly for this; SwiftPM
has no monorepo-subdirectory support). No tags exist yet, and the native libraries under
`Sources/HyperCast/NativeLibs/{rid}/` are committed by `stage-native-binaries.yml` (SwiftPM
has no packing step — see `NativeLibs/README.md`). Until the first tag: clone the repo,
`cargo build --release` in `rust/`, copy the library into `NativeLibs/{rid}/`, and
`swift test` in `swift/`.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.
