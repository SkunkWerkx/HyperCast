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
4. **Faster on the culture-machinery doors, and now allocation-free** — numbers from
   ordo-one's package-benchmark (linux-arm64, p50, `swift package benchmark run` in
   `Benchmarks/`, Swift 6.3.3), before and after the 0.2.0 carrier rewrite, same machine,
   same session:

   | Door | 0.1.0 | 0.2.0 | mallocs/call | Foundation, same run |
   | --- | ---: | ---: | :---: | ---: |
   | `Cast.timestamp` | 281 ns | **55 ns** | 3 → **0** | 817 ns `Date.ISO8601FormatStyle` |
   | `Cast.uuid` | 222 ns | **37 ns** | 3 → **0** | 603 ns `UUID(uuidString:)` |
   | `Cast.dateTime` (messy civil) | 330 ns | **129 ns** | 3 → **0** | 28 µs `DateFormatter` (`M/d/yyyy h:mm a`, hoisted) |
   | `Cast.date` (declared order) | 289 ns | **101 ns** | 3 → **0** | — |
   | `Cast.duration` | 297 ns | **56 ns** | 3 → **0** | — |
   | `Cast.f64` | 354 ns | **45 ns** | 4 → **0** | 75 ns `Double(String)` |
   | `Cast.i32` | 349 ns | **33 ns** | 4 → **0** | 10 ns `Int(String)` — honest loss, see below |
   | `Cast.i32` (grouped) | 361 ns | **64 ns** | 4 → **0** | — |
   | `Cast.bool` | 244 ns | **28 ns** | 3 → **0** | — |

   **What moved the numbers** was the carrier, not the parse. Every `String` door copied the
   input into a fresh `[UInt8]` (`Array(text.utf8)`) and every door then allocated three
   more heap arrays for the out-value, the fault span and the format — four mallocs before
   the native call. The input now crosses as a view of the string's own UTF-8 (`withUTF8`),
   the scratch is a tuple of fixed-width integers on the stack, and the 21-function library
   handle is a class reference rather than a struct copied out of a `Result` per call. The
   Foundation columns are the same run's controls, unchanged between the two tapes, which
   is what makes the comparison a receipt.

   Two things to read straight: `Int(String)` at 10 ns is still faster than the invariant
   integer door, as it should be — a stdlib integer parse with no grouping, no parens and
   no radix prefixes is the floor, and the door only wins once the text needs any of those.
   And the `DateFormatter` control measured **28 µs with 103 mallocs** on this toolchain
   where 0.1.0's tape recorded 810 ns; the door's own numbers are the claim here, the
   Foundation figure is reported as measured today, not carried over.

   Separator detection now shows its true cost, because the carrier is thin enough to see
   it: `1.234.567,89` under `.detect` is 86 ns against 74 ns for the same text under a
   declared eurozone format — the ~11 ns the raw core spends resolving `.`/`,` roles,
   which 0.1.0's 399-vs-406 ns hid inside four heap allocations.

**The honest trade-off:** a native dependency carried as a package resource, a dlopen at
first use, and an FFI crossing per call — for plain invariant integers, `Int32("...")` is
the reasonable choice. (Benchmark forensics worth knowing: the first Swift tape was pure
measurement-floor quantization until `.kilo` scaling amortized it — receipts include their
own archaeology.)

Every door also takes an `UnsafeRawBufferPointer` — the primitive the `String` and
`[UInt8]` forms wrap — so a caller already holding a buffer (a mapped file, one field of a
delimited line) casts a slice of it without copying anything out first.

## Verifying provenance

Like PHP, there's no separate package registry to attest here — SwiftPM resolves a git tag
directly against this repo. The native libraries bundled under
`swift/Sources/HyperCast/NativeLibs/` (staged by `stage-native-binaries.yml`) each carry
their own build-provenance attestation from `hyper-build-native.yml`, which physically lives
in `SkunkWerkx/.github` — so verifying needs `--signer-repo` alongside `--repo`, or `gh`
reports a bare `verifying with issuer "sigstore.dev"` that reads like a bad signature but is
only an identity mismatch:

```sh
gh attestation verify swift/Sources/HyperCast/NativeLibs/osx-arm64/libhypercast.dylib \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
```

See [csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance)
for more on why `--signer-repo` is needed for some artifacts here and not others.

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
