# NativeLibs/

Populated per-RID with the platform's native `libhypercast` build (`NativeLibs/{rid}/{lib}`),
committed to git — SwiftPM resolves a `.package(url:)` dependency straight from the git tree
at the resolved tag, with no packing step of its own (and `binaryTarget`/XCFramework is
Apple-only, so it can't cover the Linux/Windows RIDs), so the native binaries have to live
here for real, not be staged in transiently by CI (the same real bug HyperUuid found and
fixed across its PHP/Swift/Go bindings — see `php/src/native/README.md` for the origin
story). `stage-native-binaries.yml` refreshes them automatically on every rust/-touching
merge. Regenerate locally with `cargo cdylib` in `rust/` and copy the result in if
you need to update one by hand; CI's own `build-native` job does the same per-leg during
in-repo testing.

Four RIDs: `osx-x64`, `osx-arm64`, `win-x64` and `win-arm64`. Linux is not here, glibc or
musl. There the binding links the core in from `swift/HyperCastCore.artifactbundle` — a static
library per triple, WebAssembly included — instead of loading a shared one, which is the
only way to reach Swift's static Linux SDK at all: a statically linked executable has no
loader to open a `.so` with. `stage-native-binaries.yml` fills both directories from the
same CI run.

At run time the library is opened in place, from wherever SwiftPM staged this directory
(`HyperCast_HyperCast.bundle` beside the build products, or `HyperCast_HyperCast.resources` on
Windows before Swift 6.4) — see the binding README's "Loading and deployment" for what that
means for a deployed executable.

## Verifying provenance

These are compiled binaries committed to git, which is the least inspectable thing in this
repository — you cannot read a diff of them. So they carry
[SLSA build provenance](https://github.com/actions/attest-build-provenance): every one is
signed as it is built, and `stage-native-binaries.yml` verifies that signature *before* it is
allowed to commit the file, so a binary reaching this directory has already had its origin
checked. The staging commit records each file's SHA-256 in its own message.

Verify any of them yourself, against GitHub's transparency log, without trusting this
repository or whoever handed you a copy:

```shell
gh attestation verify linux-arm64/libhypercast.so \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
```

`--signer-repo` is required, not decoration. `--repo` on its own asserts two things at once:
that the artifact came from that repo, and that the workflow which signed it lives there.
Only the first is true here — the signing step is in `hyper-build-native.yml`, which lives in
the shared `SkunkWerkx/.github` forge repo, so that is what Fulcio records as the build
signer. Omit the flag and verification fails with an unhelpful
`verifying with issuer "sigstore.dev"`, which looks like a bad signature but is really an
identity mismatch.

That reports the exact commit and workflow run the binary was built from. Verification is by
content digest, so it holds for these committed copies even though they were produced as CI
artifacts — the bytes are identical. The same set of binaries is committed under `go/native/`,
`php/src/native/` and `swift/Sources/HyperCast/NativeLibs/`; git stores each one as a single
shared blob, so the three copies cost no extra repository space, and one attestation covers
all three.

If you would rather not trust a binary at all, build the core from source instead — it is a
plain Rust crate with no build-time codegen:

```shell
cd rust && cargo cdylib
```
