// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "HyperCast",
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
