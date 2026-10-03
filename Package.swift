// swift-tools-version:6.2
import PackageDescription

// This file exists purely so `.package(url: "https://github.com/SkunkWerkx/HyperCast", ...)`
// resolves at all — SwiftPM requires Package.swift at the repository root, with no monorepo
// subdirectory support (same hard constraint Packagist has for composer.json). CI's own
// build/test still goes through swift/Package.swift (working-directory: swift); this one
// just points its targets' `path:` at the real sources instead of duplicating them, and has
// to stay in step with it — see that file for what each target is.
let package = Package(
    name: "HyperCast",
    // macOS 13 floor: the duration door presents Swift's own Duration type, which (with
    // its .seconds/.nanoseconds arithmetic) is macOS 13+ — kept in sync with
    // swift/Package.swift, whose comment carries the full story.
    platforms: [
        .macOS(.v13)
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
            dependencies: ["HyperCastCore"],
            path: "swift/Sources/HyperCast"
        ),
        .testTarget(
            name: "HyperCastTests",
            dependencies: ["HyperCast"],
            path: "swift/Tests/HyperCastTests"
        ),
    ]
)
