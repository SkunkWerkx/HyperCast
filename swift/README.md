# HyperCast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Swift Package](https://img.shields.io/github/v/tag/SkunkWerkx/HyperCast?label=swift%20package&sort=semver)](https://github.com/SkunkWerkx/HyperCast/tags)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**`Verdict<T>` is a real Swift `enum`, so an exhaustive `switch` over it is
*compiler-mandatory* — not an opt-in analyzer flag, not a review convention. The value, or a
closed reason plus the exact byte span that offended.**

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
   held by the shared corpus (25 tests green, full twelve-file corpus replay with byte-exact
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

Add the package URL as a dependency:

```
https://github.com/SkunkWerkx/HyperCast
```

In Xcode that's File ▸ Add Package Dependencies; in a `Package.swift` it's a `.package(url:)`
entry with whatever version requirement suits you. SwiftPM resolves the newest release that
satisfies it, so there is no version to copy from here and none to go stale.

SwiftPM has no separate registry to publish to — `.package(url:from:)` resolves straight from
a git tag, which *is* the complete publish story here rather than a placeholder for one. It
requires `Package.swift` at the repository root with no monorepo-subdirectory support, which
is why [the root's own `Package.swift`](../Package.swift) exists, with its targets pointed at
the real sources under `swift/` via `path:`. The native libraries under
`Sources/HyperCast/NativeLibs/{rid}/` are committed straight to git for the same reason as
the tag itself: SwiftPM has no packing step, so the tree at the resolved tag is what a
consumer's build bundles as resources.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.
