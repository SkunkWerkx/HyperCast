// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "HyperCast",
    // macOS 13 floor: the duration door presents Swift's own Duration type, which (with
    // its .seconds/.nanoseconds arithmetic) is macOS 13+. Linux builds carry no such
    // availability gate — this only sets the Darwin deployment target.
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .library(name: "HyperCast", targets: ["HyperCast"])
    ],
    targets: [
        // Bundles every platform's native build under NativeLibs/{rid}/{lib} (the same
        // reasoning as HyperUuid's Swift binding: SwiftPM's binaryTarget/XCFramework
        // mechanism is Apple-only and can't cover the Windows/Linux RIDs).
        // NativePlatform.swift picks the resource path at compile time;
        // DynamicLibrary.swift dlopen/dlsym's it at runtime.
        .target(
            name: "HyperCast",
            resources: [.copy("NativeLibs")]
        ),
        .testTarget(
            name: "HyperCastTests",
            dependencies: ["HyperCast"]
        ),
    ]
)
