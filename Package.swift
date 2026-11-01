// swift-tools-version:6.2
import PackageDescription

import Foundation

// This file exists purely so `.package(url: "https://github.com/SkunkWerkx/HyperCast", ...)`
// resolves at all — SwiftPM requires Package.swift at the repository root, with no monorepo
// subdirectory support (same hard constraint Packagist has for composer.json). CI's own
// build/test still goes through swift/Package.swift (working-directory: swift); this one
// just points its targets' `path:` at the real sources instead of duplicating them, and has
// to stay in step with it — see that file for what each target is.

// The core comes from the GitHub Release: two zips per version, the static-library artifact
// bundle (Linux glibc and musl, macOS, Windows, WebAssembly and Android, one archive per
// triple) and, on a Mac, the XCFramework iOS, the iOS simulator and Mac Catalyst link (Xcode
// does not read a static-library artifact bundle). SwiftPM checks each zip against its
// checksum before using it. Release staging (the Forge's stage-release action) writes the
// three lines below for every release, from the zips the release then attaches; none resolves
// before the first release that carries them.
let coreRelease = "1.0.0"
let coreChecksum = ""
let appleCoreChecksum = ""

// In this repository, CI and .github/scripts/local-core.sh write the core into
// swift/HyperCastCore.artifactbundle/ (gitignored), and then this file links that instead, the
// way swift/Package.swift does: every CI check that builds through this manifest tests the core
// the run built. A consumer's checkout never has one.
func built(_ path: String) -> Bool {
    FileManager.default.fileExists(atPath: "\(Context.packageDirectory)/\(path)")
}
let assets = "https://github.com/SkunkWerkx/HyperCast/releases/download/v\(coreRelease)"
let localCore = "swift/HyperCastCore.artifactbundle"
let localAppleCore = "swift/HyperCastCoreApple.xcframework"
let isLocal = built("\(localCore)/info.json")
#if canImport(Darwin)
    let linksAppleCore = isLocal ? built("\(localAppleCore)/Info.plist") : true
#else
    let linksAppleCore = false
#endif
let coreTargets: [Target] =
    [
        isLocal
            ? .binaryTarget(name: "HyperCastCore", path: localCore)
            : .binaryTarget(
                name: "HyperCastCore", url: "\(assets)/HyperCastCore.artifactbundle.zip",
                checksum: coreChecksum)
    ]
    + (!linksAppleCore
        ? []
        : [
            isLocal
                ? .binaryTarget(name: "HyperCastCoreApple", path: localAppleCore)
                : .binaryTarget(
                    name: "HyperCastCoreApple", url: "\(assets)/HyperCastCoreApple.xcframework.zip",
                    checksum: appleCoreChecksum)
        ])
let coreDependencies: [Target.Dependency] =
    linksAppleCore
    ? [
        .target(name: "HyperCastCore", condition: .when(platforms: [.macOS, .linux, .windows, .wasi, .android])),
        .target(name: "HyperCastCoreApple", condition: .when(platforms: [.iOS, .macCatalyst])),
    ] : ["HyperCastCore"]

let package = Package(
    name: "HyperCast",
    // macOS 13 floor: the duration door presents Swift's own Duration type, which (with
    // its .seconds/.nanoseconds arithmetic) is macOS 13+ — kept in sync with
    // swift/Package.swift, whose comment carries the full story.
    platforms: [
        .macOS(.v13),
        .iOS(.v16),
        .macCatalyst(.v16),
    ],
    products: [
        .library(name: "HyperCast", targets: ["HyperCast"])
    ],
    targets: [
        .target(
            name: "HyperCast",
            dependencies: coreDependencies,
            path: "swift/Sources/HyperCast"
        ),
        .testTarget(
            name: "HyperCastTests",
            dependencies: ["HyperCast"],
            path: "swift/Tests/HyperCastTests"
        ),
    ] + coreTargets
)
