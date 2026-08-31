# NativeLibs/

Populated per-RID with the platform's native `libhypercast` build (`NativeLibs/{rid}/{lib}`),
committed to git — SwiftPM resolves a `.package(url:)` dependency straight from the git tree
at the resolved tag, with no packing step of its own (and `binaryTarget`/XCFramework is
Apple-only, so it can't cover the Linux/Windows RIDs), so the native binaries have to live
here for real, not be staged in transiently by CI (the same real bug HyperUuid found and
fixed across its PHP/Swift/Go bindings — see `php/src/native/README.md` for the origin
story). `stage-native-binaries.yml` refreshes them automatically on every rust/-touching
merge. Regenerate locally with `cargo build --release` in `rust/` and copy the result in if
you need to update one by hand; CI's own `test-swift` job does the same per-leg during
in-repo testing.
