// swift-tools-version:6.2
import PackageDescription

// A consumer, not part of the library: an executable that takes HyperCast the way a real
// project does — through the manifest at the repository root — and calls every kind of door.
// It exists for the targets `swift test` cannot reach. Swift's static Linux SDK (musl) ships
// no XCTest, so this is what proves the linked-in core there; built for glibc or WebAssembly
// it proves the same thing through a consumer's manifest rather than the package's own.
//
//     swift build --swift-sdk x86_64-swift-linux-musl && .build/debug/StaticSmokeTest
let package = Package(
    name: "StaticSmokeTest",
    platforms: [
        .macOS(.v13)
    ],
    dependencies: [
        .package(name: "HyperCast", path: "../..")
    ],
    targets: [
        .executableTarget(
            name: "StaticSmokeTest",
            dependencies: [.product(name: "HyperCast", package: "HyperCast")],
            path: "Sources"
        )
    ]
)
