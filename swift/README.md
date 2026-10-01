# HyperCast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Swift Package](https://img.shields.io/github/v/tag/SkunkWerkx/HyperCast?label=swift%20package&sort=semver)](https://github.com/SkunkWerkx/HyperCast/tags)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**`Verdict<T>` is a real Swift `enum`, so an exhaustive `switch` over it is
*compiler-mandatory* — not an opt-in analyzer flag, not a review convention. The value, or a
closed reason plus the exact byte span that offended.**

Allocation-lean scalar casts — booleans, the full integer family, reals, exact decimals,
UUIDs, temporals — calling directly into the native `hypercast` Rust core through
`@convention(c)` function pointers, no shim layer. On Linux (glibc and musl) and
WebAssembly the core is linked into your executable as a static library, so there is
nothing to deploy beside it; on macOS and Windows it is a bundled shared library, opened on
first use with `dlopen`/`dlsym` or `LoadLibraryW`/`GetProcAddress`. Which one, and for
which architecture, is decided at compile time.

```swift
switch try Cast.i32("(1,234)", format: .invariant) {
case .success(let value): print("got \(value)")          // -1234, accounting negative
case .fault(let fault): print("\(fault.reason) at byte \(fault.offset)")
}   // no default: the compiler mandates both arms, and only both arms
```

Door names mirror the native ABI (`i32`, `f64`, `decimal`, `timestamp`, …); every door also
takes raw UTF-8 `[UInt8]` for callers already holding bytes. Swift-flavored fidelity:
`Duration` is attosecond-backed, so the duration door keeps every nanosecond the core
parses; the `Duration` presentation is also why `Package.swift` carries a `.macOS(.v13)`
floor (Linux has no availability gates — this only sets the Darwin deployment target).
`Cast.decimal` presents Foundation's `Decimal`, whose 38-digit mantissa holds every value
the core's 96-bit, 28-place decimal produces exactly — `0.1` is one tenth, `50%` is exactly
`0.5`, and excess precision is `outOfRange` rather than rounded. The core's result is
canonical — exact trailing fraction zeros are trimmed, so `1.10`, `1.1` and `1.1000` are all
magnitude 11 at scale 1 — which is exactly what `Decimal` represents, so nothing is lost
between the core and the presentation. `Cast.nativeVersion()` reports the loaded
library's own `major.minor.patch` (`hypercast_version`), so a caller can prove the binary
it resolved is the one this binding was built against before the first cast; `Cast.isAvailable`
is the non-throwing form of the same question — the probe a consumer with a fallback gates
on, so a door's `throws` (which only ever means "the library couldn't load", and is always a
`NativeLibraryError` — see [Loading and deployment](#loading-and-deployment)) never has to be
caught at a call site. A caller that is itself generic over its target uses
`Cast.numeric<T>(_:format:)`, resolved statically over the closed `NumericCastTarget` set —
exactly the eleven numeric targets (`Int8`…`Int64`, `UInt8`…`UInt64`, `Float`, `Double`,
`Decimal`), each routed to its own door; any other `T` is a compile error, not a runtime
one.

```swift
func column<T: NumericCastTarget>(_ cells: [String], as _: T.Type, format: NumFormat) throws -> [Verdict<T>] {
    try cells.map { try Cast.numeric($0, format: format) }
}
```

## Why not `Int32("...")` / `ISO8601FormatStyle`?

1. **Verdicts with location** — Swift's failable initializers return `nil` with no reason
   and no span; formatters throw.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, radix prefixes, all five .NET `Guid` text forms plus
   `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (every corpus file replayed, with byte-exact fault spans).
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
   the scratch is a tuple of fixed-width integers on the stack, and the library handle (one
   function pointer per native export) is a class reference rather than a struct copied
   out of a `Result` per call. The
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

## Declared formats and currency

The numeric doors take a `NumFormat` — declared separators plus `NumStyles` lenience flags
— with no default argument: `.invariant` (`.` decimal, `,` grouping, every lenience on) or
`NumFormat.from(locale:)`, which reads the locale's separators and its currency symbol. The
core carries no culture table, so the symbol is the one field a culture has to supply, and
it is declared, never looked up. With `.currency` in the styles (part of `.all`) the
declared symbol is accepted once at either edge of the number — leading, before or after a
sign (`$5`, `-$5`, `$ -5`), or trailing (`5 €`, `1.234,50 kr.`) — with optional whitespace
between symbol and digits, and accounting parentheses wrap the two together (`($5)`). A
symbol declared with the flag off is `malformed` at the symbol; the flag with no symbol
declared changes nothing. The symbol is at most 16 UTF-8 bytes with no digit or whitespace
in it — anything else is a precondition failure, a caller bug the same way equal separators
are.

```swift
let danish = NumFormat(decimalSeparator: ",", groupSeparator: ".", styles: .all, currencySymbol: "kr.")
switch try Cast.decimal("(1.234,50 kr.)", format: danish) {
case .success(let amount): print(amount)                   // -1234.5, exact — no double was ever formed
case .fault(let fault): print("\(fault.reason) at byte \(fault.offset)")
}
let enUs = NumFormat.from(locale: Locale(identifier: "en_US"))   // "$", from the locale's own data
try Cast.i32("-$5", format: enUs)                                  // .success(-5)
```

## Requirements

- **Swift 6.2 or later.** The manifests declare `swift-tools-version:6.2`: the first release
  whose package manager can link a static library as a binary target (SE-0482), which is how
  the core reaches Linux and WebAssembly. CI runs `swift test` on Swift 6.4 on every
  platform, and runs Linux (glibc and musl) and WebAssembly again on 6.2 in Swift's own
  containers. macOS and Windows are tested on 6.4 only.
- **Platforms.** Linux on glibc and on musl (Swift's static Linux SDK), macOS and Windows,
  each on x86_64 and arm64, and WebAssembly (`wasm32-unknown-wasip1`). macOS 13 is the declared deployment floor, for `Duration`.
- **Not supported: everything else.** iOS, tvOS, watchOS, visionOS, Android, and any other
  architecture on the supported systems have no native build here and stop at an `#error`
  — at compile time, rather than being handed a library that can't load.

## Loading and deployment

How the native core gets into your program depends on the target, and on most of them there
is nothing for you to do.

**Linux and WebAssembly: linked in.** The package declares the core as a SwiftPM binary
target — one static library per triple, in `HyperCastCore.artifactbundle` — and SwiftPM links
the one for your target into your executable. There is no shared library to find at run
time and nothing to deploy beside the binary: a multi-stage Dockerfile that copies only the
executable works, and so does a fully static build with
`swift build --swift-sdk x86_64-swift-linux-musl`. `Cast.isAvailable` is always `true`
here.

**macOS and Windows: loaded.** There the core is a shared library that travels as a SwiftPM
resource. `swift build` stages `NativeLibs/` into a directory beside the built products,
and the first call opens this platform's library straight out of it — nothing is extracted,
copied or left behind in a temp directory. The directory is `HyperCast_HyperCast.bundle` on macOS,
and on Windows with Swift 6.4 and later; on Windows with Swift 6.2 or 6.3 (or
`--build-system native`) it is `HyperCast_HyperCast.resources`. The loader accepts either.

**On those two platforms that directory has to ship with your executable.** A deployment
that copies only the binary has no native library to load:

```sh
cp -R .build/release/MyTool .build/release/HyperCast_HyperCast.bundle /path/to/deploy/
```

The loader looks beside the executable first, then in the main bundle's resources, which is
where an app bundle carries it. On the machine that built the package it also falls back to
the package's own checkout, so an executable copied out of `.build` keeps working *there* —
which is exactly why a missing directory tends to show up only after deployment. Test the
deployed layout, not the build tree.

When the library can't be found or loaded, nothing crashes. Every door throws
`NativeLibraryError` — a public type, and the only thing a door ever throws, naming the path
it looked for or the export it couldn't resolve — and `Cast.isAvailable` answers the same
question without a `do`/`catch`:

```swift
guard Cast.isAvailable else {
    return Int32(text)                          // your fallback
}

do {
    return try Cast.i32(text, format: .invariant)
} catch let error as NativeLibraryError {
    // .openFailed(path:reason:) or .symbolNotFound(name:) — the library, never the data
}
```

The load is attempted once per process and its outcome kept, so `isAvailable` costs nothing
after the first answer, and a failed load throws the same error from every later call.

## WebAssembly

The binding compiles to WebAssembly from Swift 6.2: install swift.org's WebAssembly SDK
and build with it.

```sh
swift sdk install <the Wasm SDK URL and checksum from swift.org/install>
swift build --swift-sdk swift-6.4.0-RELEASE_wasm     # `swift sdk list` names yours
```

The core is linked in as a static library — the same binary target Linux uses, with a
`wasm32-unknown-wasip1` archive — so there is no module to load and no engine to embed.
The core needs neither a clock nor randomness, so it asks nothing of the host. CI runs the whole suite under WasmKit on Swift 6.4, and a smoke executable on 6.2, whose
XCTest does not start under WASI.

The other direction — running the core as wasm *inside* a native Swift process, the way the
Java, Ruby, Python and Go bindings do — is not built: no wasm engine ships as a Swift
package with a stable API, and nothing here needs one, since every platform this binding
supports has the core natively. The root README's
[WebAssembly section](../README.md#webassembly) tracks both directions for every binding.

## Verifying provenance

Like PHP, there's no separate package registry to attest here — SwiftPM resolves a git tag
directly against this repo. The native binaries the package carries — the shared libraries
under `swift/Sources/HyperCast/NativeLibs/` and the static libraries under
`swift/HyperCastCore.artifactbundle/`, both staged by `stage-native-binaries.yml` — each carry
their own build-provenance attestation from `hyper-build-native.yml`, which physically lives
in `SkunkWerkx/.github` — so verifying needs `--signer-repo` alongside `--repo`, or `gh`
reports a bare `verifying with issuer "sigstore.dev"` that reads like a bad signature but is
only an identity mismatch:

```sh
gh attestation verify swift/Sources/HyperCast/NativeLibs/osx-arm64/libhypercast.dylib \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
gh attestation verify swift/HyperCastCore.artifactbundle/x86_64-unknown-linux-gnu/libhypercast.a \
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
entry with whatever version requirement suits you, plus the product on each target that
uses it:

```swift
dependencies: [
    .package(url: "https://github.com/SkunkWerkx/HyperCast", from: "…"),   // the tag on the badge above
],
targets: [
    .target(
        name: "MyTarget",
        dependencies: [.product(name: "HyperCast", package: "HyperCast")]
    ),
]
```

SwiftPM resolves the newest release that satisfies the requirement, so there is no version
to copy from here and none to go stale.

SwiftPM has no separate registry to publish to — `.package(url:from:)` resolves straight from
a git tag, which *is* the complete publish story here rather than a placeholder for one. It
requires `Package.swift` at the repository root with no monorepo-subdirectory support, which
is why [the root's own `Package.swift`](../Package.swift) exists, with its targets pointed at
the real sources under `swift/` via `path:`. The native binaries — the shared libraries
under `Sources/HyperCast/NativeLibs/{rid}/` and the static ones under
`HyperCastCore.artifactbundle/{triple}/` — are committed straight to git for the same
reason as the tag itself: SwiftPM has no packing step, so the tree at the resolved tag is
what a consumer's build links or bundles.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
