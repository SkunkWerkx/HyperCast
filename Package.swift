// swift-tools-version:6.2
import PackageDescription

#if canImport(Darwin)
    import Foundation
#endif

// This file exists purely so `.package(url: "https://github.com/SkunkWerkx/HyperCast", ...)`
// resolves at all — SwiftPM requires Package.swift at the repository root, with no monorepo
// subdirectory support (same hard constraint Packagist has for composer.json). CI's own
// build/test still goes through swift/Package.swift (working-directory: swift); this one
// just points its targets' `path:` at the real sources instead of duplicating them, and has
// to stay in step with it — see that file for what each target is.

// iOS, the iOS simulator and Mac Catalyst take the core from an XCFramework instead: an app
// for those is built by Xcode, which has linked a static library out of an XCFramework since
// Xcode 12 and does not read a static-library artifact bundle. It carries the same no_std
// archives, one slice each (arm64 only), with the header and module map under
// Headers/HyperCastCore/, so `import HyperCastCore` finds the same module either way.
//
// Only a Mac can build for those platforms, so only a Mac's manifest declares the target; on
// Linux and Windows this file is what it was. And only when the XCFramework is there: it is
// committed by stage-native-binaries.yml, whole, so a checkout from before its first staging
// has none, and a binary target whose path is missing fails the whole package, macOS
// included.
#if canImport(Darwin)
    let appleCore = "swift/HyperCastCoreApple.xcframework"
    let linksAppleCore = FileManager.default.fileExists(
        atPath: "\(Context.packageDirectory)/\(appleCore)/Info.plist")
#else
    let appleCore = ""
    let linksAppleCore = false
#endif
let appleCoreTargets: [Target] =
    linksAppleCore ? [.binaryTarget(name: "HyperCastCoreApple", path: appleCore)] : []
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
        .binaryTarget(
            name: "HyperCastCore",
            path: "swift/HyperCastCore.artifactbundle"
        ),
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
    ] + appleCoreTargets
)
