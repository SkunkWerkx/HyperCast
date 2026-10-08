# HyperCast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Swift Package](https://img.shields.io/github/v/tag/SkunkWerkx/HyperCast?label=swift%20package&sort=semver)](https://github.com/SkunkWerkx/HyperCast/tags)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**`Verdict<T>` is a real Swift `enum`, so an exhaustive `switch` over it is
*compiler-mandatory* — not an opt-in analyzer flag, not a review convention. The value, or a
closed reason plus the exact byte span that offended.**

Allocation-lean scalar casts — booleans, the full integer family, reals, exact decimals,
UUIDs, temporals — calling directly into the native `hypercast` Rust core through
`@convention(c)` function pointers, no shim layer. On every platform it supports — Linux
(glibc and musl), macOS, Windows and WebAssembly — the core is linked into your executable
as a static library, so there is nothing to load at run time and nothing to deploy beside
it.

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
floor, and `.iOS(.v16)` and `.macCatalyst(.v16)` beside it (Linux has no availability
gates — these only set the Darwin deployment targets).
`Cast.decimal` presents Foundation's `Decimal`, whose 38-digit mantissa holds every value
the core's 96-bit, 28-place decimal produces exactly — `0.1` is one tenth, `50%` is exactly
`0.5`, and excess precision is `outOfRange` rather than rounded. The core's result is
canonical — exact trailing fraction zeros are trimmed, so `1.10`, `1.1` and `1.1000` are all
magnitude 11 at scale 1 — which is exactly what `Decimal` represents, so nothing is lost
between the core and the presentation. `Cast.nativeVersion()` reports the linked core's
own `major.minor.patch` (`hypercast_version`), so a caller can prove the binary it resolved
is the one this binding was built against before the first cast — see
[Linking and deployment](#linking-and-deployment). A caller that is itself generic over its target uses
`Cast.numeric<T>(_:format:)`, resolved statically over the closed `NumericCastTarget` set —
exactly the eleven numeric targets (`Int8`…`Int64`, `UInt8`…`UInt64`, `Float`, `Double`,
`Decimal`), each routed to its own door; any other `T` is a compile error, not a runtime
one.

```swift
func column<T: NumericCastTarget>(_ cells: [String], as _: T.Type, format: NumFormat) throws -> [Verdict<T>] {
    try cells.map { try Cast.numeric($0, format: format) }
}
```

`Cast.scalar<T>(_:format:)` widens that to every door a type alone can choose, over the
closed `ScalarCastTarget` set: `Bool`, the eleven numeric targets, `UUID`, `Date` (the
RFC 3339 `timestamp` door, zone mandatory), `Duration` and `Unicode.Scalar` (the `char`
door). `format` is read by the numeric doors only. `DateComponents` is deliberately not a
target: the `date`, `time` and `dateTime` doors all return it, so the type cannot say which
was meant, and a caller names that door instead.

`Cast.char` reads one `Unicode.Scalar`: an input that is exactly one scalar is taken
verbatim before any trimming (`" "` is a space, `"6"` the digit six), and otherwise the
trimmed text must be one code-point spelling — `65`, `U+0041`, `0x41`, `&H41`, `&#65;`,
`&#x41;` — with a spelling past `U+10FFFF` or naming a surrogate `outOfRange`. Every scalar
fits `Unicode.Scalar`, so nothing narrows.

## Numbers a workbook already holds

A spreadsheet stores a numeric cell as a `Double`, and four doors read one directly, with no
text in between: `Cast.decimalFromDouble` (the shortest decimal that names the double, the
digits the file holds — `0.1` is one tenth), `Cast.excelSerialFromDouble(_:epoch:)` (the
`DateComponents` `dateTime` returns, under a declared `ExcelEpoch`), `Cast.excelTime` (a
serial's fraction, as `time` returns it) and `Cast.excelDuration` (days as a `Duration`). A
typed door's `Fault` has no span: its offset and length are 0.

## Why not `Int32("...")` / `ISO8601FormatStyle`?

1. **Verdicts with location** — Swift's failable initializers return `nil` with no reason
   and no span; formatters throw.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, radix prefixes, all five .NET `Guid` text forms plus
   `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (every corpus file replayed, with byte-exact fault spans).
4. **Faster on the culture-machinery doors, and allocation-free** — numbers from
   ordo-one's package-benchmark (linux-x64 on an Intel Core i9-11900H, p50, `swift package
   benchmark run` in `Benchmarks/`, Swift 6.3.3), Foundation's closest parser measured in
   the same run:

   | Door | HyperCast | mallocs/call | Foundation, same run |
   | --- | ---: | :---: | ---: |
   | `Cast.dateTime` (messy civil) | **82 ns** | 0 | 33 µs `DateFormatter` (`M/d/yyyy h:mm a`, hoisted), 103 mallocs |
   | `Cast.uuid` | **31 ns** | 0 | 479 ns `UUID(uuidString:)` — 15x |
   | `Cast.timestamp` | **37 ns** | 0 | 627 ns `Date.ISO8601FormatStyle` — 17x |
   | `Cast.decimal` | **39 ns** | 0 | 419 ns `Decimal(string:)` — 11x |
   | `Cast.f64` | **30 ns** | 0 | 68 ns `Double(String)` — 2.3x |
   | `Cast.i32` | 20 ns | 0 | 7 ns `Int(String)` — honest loss, see below |
   | `Cast.i32` (grouped) | 26 ns | 0 | — |
   | `Cast.date` (declared order) | 65 ns | 0 | — |
   | `Cast.duration` | 33 ns | 0 | — |
   | `Cast.bool` | 15 ns | 0 | — |

   **What makes the doors this cheap** is the carrier, not the parse: nothing is allocated
   around the call. The input crosses as a view of the string's own UTF-8 (`withUTF8`), the
   scratch is a tuple of fixed-width integers on the stack, and the call is a direct call to
   a linked-in symbol through a table of function pointers held as a class reference, not a
   struct copied per call.

   Two things to read straight: `Int(String)` at 7 ns is still faster than the invariant
   integer door, as it should be — a stdlib integer parse with no grouping, no parens and
   no radix prefixes is the floor, and the door only wins once the text needs any of those.
   And the `DateFormatter` control really does measure tens of microseconds and 103
   mallocs on this toolchain, where 0.1.0's tape on an older one recorded 810 ns; the
   door's own numbers are the claim here, the Foundation figure is reported as measured,
   not carried over.

   Separator detection shows its true cost, because the carrier is thin enough to see it:
   `1.234.567,89` under `.detect` is 54 ns against 31 ns for the same text under a declared
   eurozone format.

**The honest trade-off:** a native dependency — a prebuilt static library per platform,
linked into your executable — and a call across the C ABI per cast. For plain invariant
integers, `Int32("...")` is the reasonable choice.
(Benchmark forensics worth knowing: the first Swift tape was pure measurement-floor
quantization until `.kilo` scaling amortized it — receipts include their own archaeology.)

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

## Interop: building on HyperCast's C ABI

The `Interop` namespace is for a package that carries HyperCast's verdicts across a C ABI
of its own (HyperTabular does): it reads the value layouts the core writes
through exactly the conversions every `Cast` door applies, so a value read out of another
library's buffer is the value the door of the same name would have returned. Each reader
takes an `UnsafeRawBufferPointer` starting at one value, aligned for its widest field.

- Out-values: `Interop.scalar` → `Unicode.Scalar`, `decimal` → `Decimal`, `uuid` → `UUID`, `instant` → `Date`, `date`,
  `civil` and `time` → `DateComponents`, `duration` → `Duration`.
- `Interop.rawFormat(_:)` — a `NumFormat` as the core reads it, the 32-byte
  `Interop.RawNumFormat` tuple, the symbol inline up to `Interop.currencyMaxBytes` (16).
- `Interop.fault(code:offset:length:)` — the `Fault` a nonzero verdict code and its byte
  span name; any code other than 1–3 is a precondition failure, a binding bug.
- `Interop.version(_:)` — a `*_version()` export's packed `major << 16 | minor << 8 | patch`
  word as `"major.minor.patch"`.

```swift
let amount = buffer.withUnsafeBytes { Interop.decimal(UnsafeRawBufferPointer(rebasing: $0[offset..<offset + 16])) }
let format = Interop.rawFormat(.invariant)   // 32 bytes, ready for the other library's ABI
```

## Requirements

- **Swift 6.2 or later.** The manifests declare `swift-tools-version:6.2`: the first release
  whose package manager can link a static library as a binary target (SE-0482), which is how
  the core reaches every platform. CI runs `swift test` on Swift 6.4 on every platform, and
  runs Linux (glibc and musl) and WebAssembly again on 6.2 in Swift's own containers. macOS
  and Windows are tested on 6.4 only.
- **Platforms.** Linux on glibc and on musl (Swift's static Linux SDK), macOS and Windows,
  each on x86_64 and arm64, WebAssembly (`wasm32-unknown-wasip1`), in WASI hosts and in
  the browser, and iOS, the iOS simulator and Mac Catalyst on arm64. The declared
  deployment floors are macOS 13, iOS 16 and Mac Catalyst 16, for `Duration`.
- **Not supported: everything else.** tvOS, watchOS, visionOS, Android, the iOS simulator
  and Mac Catalyst on Intel Macs, and any other architecture on the supported systems have
  no prebuilt core here, so the build stops at compile time with no `HyperCastCore` module
  (Swift Build first warns that the artifact bundle has no matching variant) — never at
  run time.

## Linking and deployment

There is nothing for you to do. The package declares the core as a SwiftPM binary target —
one static library per triple, in `HyperCastCore.artifactbundle` — and SwiftPM links the one
for your target into your executable. There is no shared library to find at run time, no
resource bundle, and nothing to deploy beside the binary: a multi-stage Dockerfile that
copies only the executable works, so does a fully static build with
`swift build --swift-sdk x86_64-swift-linux-musl`, and so does copying a macOS or Windows
executable on its own. Each triple's archive is 130–180 KB, and the linker keeps only what
your executable reaches.

No door throws. Bad data was never thrown — it is a `Verdict`'s `Fault` — and the `throws`
every door still carries is left from when macOS and Windows loaded a shared library that
could fail to load, so existing `try` call sites keep compiling. `Cast.isAvailable` is
always `true`, and the `NativeLibraryError` type is deprecated and has no cases, for the
same reason: code that checks either still compiles.

iOS, the iOS simulator and Mac Catalyst get the same archives from a second binary target,
`HyperCastCoreApple.xcframework`: an app for those is built by Xcode, which links a static
library out of an XCFramework and does not read a static-library artifact bundle. The
manifest declares it only on a Mac, where those platforms can be built at all, and only
when the XCFramework is in the tree; both targets define the one `HyperCastCore` module the
binding imports. CI's `test-apple-mobile` job runs the suite on an iOS simulator and as a
Mac Catalyst process with `xcodebuild test`, and builds the package for an iOS device.

Android uses the same artifact bundle: it carries the core for `aarch64-unknown-linux-android`
and `x86_64-unknown-linux-android`, and a package built with the
[Swift SDK for Android](https://www.swift.org/documentation/articles/swift-sdk-for-android-getting-started.html)
(`swift build --swift-sdk aarch64-unknown-linux-android28`; Swift 6.3 or later, API 28 or
later) links it like any other triple. Page alignment is the final link's, which the SDK
does with the NDK's linker; NDK r28 and later align to the 16 KB pages Android 15 devices
may use by default. CI cross-builds this suite with the SDK for x86_64 and runs it in an emulator
whose image uses 16 KB pages, through `.github/scripts/android_build_suite.sh` and
`android_device_test.sh`, which run the same way against a local emulator; the aarch64
build is linked. The Swift runtime on Android is shared libraries, which an app packages
the way the SDK's documentation describes; the core adds nothing to them.

In a checkout of this repository, `HYPERCAST_LOCAL_CORE=1 swift test` run from `swift/` links
the bundle `.github/scripts/local-core.sh` builds from the checkout's core in place of the
committed one. The root `Package.swift`, the one a dependency resolves, has no such switch.

On Swift 6.3 with `--build-system swiftbuild` (opt-in there), a package that depends on
this one fails with `missing required module 'HyperCastCore'`: that release's Swift Build
drops a binary target's module map when it is reached through a product
([swift-build#1295](https://github.com/swiftlang/swift-build/pull/1295), fixed in 6.4).
The default build system of every release from 6.2 on is unaffected, and so is 6.4's
Swift Build (its default); 6.2's opt-in Swift Build predates static-library artifact
bundles altogether.

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

### In the browser

What the WebAssembly SDK builds is a plain `wasm32-wasip1` command module (it imports only
`wasi_snapshot_preview1`), so a browser runs it through a WASI shim — no JavaScriptKit and
no change to the package. With [`@bjorn3/browser_wasi_shim`](https://github.com/bjorn3/browser_wasi_shim):

```js
import { WASI, File, OpenFile, ConsoleStdout } from "@bjorn3/browser_wasi_shim";

const wasi = new WASI(["MyTool"], [], [
  new OpenFile(new File([])),                       // stdin
  ConsoleStdout.lineBuffered(console.log),          // stdout
  ConsoleStdout.lineBuffered(console.error),        // stderr
]);
const { instance } = await WebAssembly.instantiateStreaming(
  fetch("MyTool.wasm"), { wasi_snapshot_preview1: wasi.wasiImport });
const exitCode = wasi.start(instance);
```

CI builds the smoke executable for WebAssembly on Swift 6.4 and runs it this way in headless
Chrome on every PR, failing unless it exits 0. An executable that imports Foundation is
large (tens of MB, most of it ICU data); `-c release` and `wasm-opt` bring that down.

The other direction — running the core as wasm *inside* a native Swift process, the way the
Java binding does — is not built: no wasm engine ships as a Swift
package with a stable API, and nothing here needs one, since every platform this binding
supports has the core natively. The root README's
[WebAssembly section](../README.md#webassembly) tracks both directions for every binding.

## Verifying provenance

Like PHP, there's no separate package registry to attest here — SwiftPM resolves a git tag
directly against this repo. The native binaries the package carries — the static libraries
under `swift/HyperCastCore.artifactbundle/`, staged by `stage-native-binaries.yml` — each
carry their own build-provenance attestation from `hyper-build-native.yml`, which physically lives
in `SkunkWerkx/.github` — so verifying needs `--signer-repo` alongside `--repo`, or `gh`
reports a bare `verifying with issuer "sigstore.dev"` that reads like a bad signature but is
only an identity mismatch:

```sh
gh attestation verify swift/HyperCastCore.artifactbundle/arm64-apple-macosx/libhypercast.a \
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
the real sources under `swift/` via `path:`. The native binaries — the static libraries
under `HyperCastCore.artifactbundle/{triple}/` (`hypercast.lib` for the two Windows triples,
`libhypercast.a` for the rest) and the iOS and Mac Catalyst slices of
`HyperCastCoreApple.xcframework/` — are committed straight to git for the same reason as the
tag itself: SwiftPM has no packing step, so the tree at the resolved tag is what a
consumer's build links.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
