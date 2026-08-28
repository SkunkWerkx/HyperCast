/// Maps this build's compile-time OS/arch to the RID-style directory (matching the other
/// bindings' `runtimes/{rid}/native/` / `native/{rid}/` convention) and filename the native
/// library was built for. A Swift build product is already single-arch/single-OS, so this
/// resolves at compile time via `#if os(...) && arch(...)`.
enum NativePlatform {
    #if os(Windows) && arch(arm64)
    static let rid = "win-arm64"
    static let libraryFileName = "hypercast.dll"
    #elseif os(Windows)
    static let rid = "win-x64"
    static let libraryFileName = "hypercast.dll"
    #elseif os(macOS) && arch(arm64)
    static let rid = "osx-arm64"
    static let libraryFileName = "libhypercast.dylib"
    #elseif os(macOS)
    static let rid = "osx-x64"
    static let libraryFileName = "libhypercast.dylib"
    #elseif os(Linux) && arch(arm64)
    static let rid = "linux-arm64"
    static let libraryFileName = "libhypercast.so"
    #elseif os(Linux)
    static let rid = "linux-x64"
    static let libraryFileName = "libhypercast.so"
    #else
    #error("hypercast: unsupported platform — no native build for this OS/arch combination")
    #endif
}
