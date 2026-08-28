// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "HyperCastBenchmarksPackage",
    platforms: [.macOS(.v13)],
    dependencies: [
        .package(path: "../"),
        .package(url: "https://github.com/ordo-one/benchmark", from: "1.4.0"),
    ],
    targets: [
        .executableTarget(
            name: "HyperCastBenchmarks",
            dependencies: [
                .product(name: "HyperCast", package: "swift"),
                .product(name: "Benchmark", package: "benchmark"),
            ],
            path: "Benchmarks/HyperCastBenchmarks",
            plugins: [
                .plugin(name: "BenchmarkPlugin", package: "benchmark")
            ]
        )
    ]
)
