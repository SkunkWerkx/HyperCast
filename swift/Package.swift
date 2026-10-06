// swift-tools-version:6.2
import PackageDescription

// The development loop: HYPERCAST_LOCAL_CORE=1 links the bundle .github/scripts/local-core.sh
// builds from the checkout, under rust/target/ (which git ignores), in place of the committed
// one, so the suite can run against the core as it stands without replacing a committed
// archive. The repository root's Package.swift, which consumers resolve, has no such switch.
let coreBundle =
    Context.environment["HYPERCAST_LOCAL_CORE"] == nil
    ? "HyperCastCore.artifactbundle" : "../rust/target/local-core/swift/HyperCastCore.artifactbundle"

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
        // The native core as static libraries, one per triple (SE-0482, which is what sets
        // the tools version above): glibc and musl Linux and macOS on x86_64 and arm64,
        // Windows (MSVC) on x86_64 and arm64, and WASI. SwiftPM picks the variant for the
        // triple being built and links it into the consumer's executable, so nothing ships
        // beside it and nothing is opened at run time. A triple with no variant has no
        // `HyperCastCore` module, and the build stops there rather than at run time.
        .binaryTarget(
            name: "HyperCastCore",
            path: coreBundle
        ),
        .target(
            name: "HyperCast",
            dependencies: ["HyperCastCore"]
        ),
        .testTarget(
            name: "HyperCastTests",
            dependencies: ["HyperCast"]
        ),
    ]
)
