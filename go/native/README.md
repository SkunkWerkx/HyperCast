# native/

Populated per-RID with the platform's native `libhypercast` build (`native/{rid}/{lib}`),
committed to git — unlike NuGet/Maven Central/PyPI/crates.io, a `go get`/`go build` consumer
has no packing step of its own: `//go:embed native` (in `embed.go`) embeds whatever's
literally in the git tree at the resolved module version, so the native binaries have to live
here for real, not be staged in transiently by CI (the same real bug HyperUuid found and
fixed for its PHP/Swift/Go bindings — see `php/src/native/README.md` for the origin story).
`stage-native-binaries.yml` refreshes them automatically on every rust/-touching merge.
Regenerate locally with `cargo build --release` in `rust/` and copy the result in if you need
to update one by hand; CI's own `build-native` job does the same per-leg during in-repo testing,
overwriting whichever platform's file matches that leg — harmless, since it's the same build
either way.
