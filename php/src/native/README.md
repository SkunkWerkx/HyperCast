# native/

Populated per-RID with the platform's native `libhypercast` build (`native/{rid}/{lib}`),
committed to git — unlike every other registry this repo publishes to (NuGet, Maven Central,
PyPI, crates.io), Packagist has no packing/build step of its own: whatever's literally in the
git tree at a tagged commit *is* the published package, so the native binaries have to live
here for real, not be staged in transiently by CI (a bug HyperUuid hit for real, three
separate times, before banking the fix as `stage-native-binaries.yml`). Regenerate locally
with `cargo build --release` in `rust/` and copy the result in if you need to update one by
hand; CI's own `build-native` job does the same per-leg during in-repo testing, overwriting
whichever platform's file matches that leg — harmless, since it's the same build either way.

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
artifacts — the bytes are identical.
