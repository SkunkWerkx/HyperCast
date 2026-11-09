## Submission

This is a new submission.

## Test environments

Every result below is a row of the evidence ledger,
https://github.com/SkunkWerkx/HyperCast/blob/master/docs/CRAN_EVIDENCE.md, which names the
checked tarball's SHA-256. They are **candidate** rows: the release candidate's tarball built
from its working tree. The tarball the release's tag run preserves is the one submitted, and it
is checked again before submission.

* macOS 27.2 on Apple M6 (aarch64-apple-darwin27.0.0), R 4.6.2 (Homebrew build), rustc and
  cargo 1.99.1, TinyTeX, `checkbashisms`, `qpdf`, `pandoc`.
* Debian forky/sid on x86-64 (the `rocker/r-devel` image), R Under development (unstable)
  (2026-11-01 r90825), with the distribution's rustc and cargo 1.97.1.
* Ubuntu on aarch64 (the `rocker/r-ver:4.2.3` image), R 4.2.3, the package's declared floor, with
  rustc and cargo 1.89.0, the core crate's `rust-version`.
* Ubuntu on aarch64 (the `rocker/r-ver:4.6.2` image), R 4.6.2, rustc and cargo 1.99.1: the
  check, valgrind and the LTO build below.
* R-devel built with AddressSanitizer on x86-64 (the `wch1/r-debug` image's `RDsan`, under
  emulation), the C shims built with `-fsanitize=address,undefined`, rustc 1.99.1, and nightly
  rustc for the Rust archive under `-Zsanitizer=address`: the sanitizer runs below.
* The `kalibera/rchk` image, for rchk.

## R CMD check results

`R CMD check --as-cran hypercast_<version>.tar.gz`, with `_R_CHECK_CRAN_INCOMING_REMOTE_=TRUE`.

macOS, R 4.6.2, with the PDF manual, on the tarball `R CMD build` made (524,288 bytes):

    Status: 1 NOTE

    * checking CRAN incoming feasibility ... NOTE
    Maintainer: 'Brian Buvinghausen <buvinghausen@gmail.com>'

    New submission

R-devel on Debian and R 4.6.2 on Ubuntu, `--no-manual`: one NOTE each, the images' own hardening
flags (`-Wdate-time -Werror=format-security -Wformat`, from `R CMD config CFLAGS`, not from this
package's `Makevars`, which sets no compiler flags):

    * checking compilation flags used ... NOTE
    Compilation used the following non-portable flag(s):
      '-Wdate-time' '-Werror=format-security' '-Wformat'

R 4.2.3 on Ubuntu, `--no-manual`: that NOTE, and two from the image (it has no `pandoc` to check
`NEWS.md` with, and no network to verify the time).

The testthat expectations pass on each (`[ FAIL 0 | WARN 0 | SKIP 1 | PASS 40181 ]`; the skip is
a test that needs a 2 GiB string and runs only with `HYPERCAST_TEST_LARGE=1`), including the
replay of the nineteen bundled corpus files every binding of the project replays. They also pass
in the C and ISO-8859-15 locales and with `_R_CHECK_DEPENDS_ONLY_=true` (noSuggests).
`NEWS.md`'s heading carries the version number, which R's NEWS parser requires. If the incoming
check lists possibly misspelled words in `DESCRIPTION` (such as "UUIDs", "booleans" or
"vectorised"), they are spelled as intended.

hypertabular, which imports this package, passes `R CMD check --as-cran` against this tarball
(`Status: OK`), checked with `r_revdep_check.sh` from HyperTabular's repository, the step that
precedes every submission of this package.

## Memory, sanitizer and native-code checks

* valgrind (`R CMD check --use-valgrind`, `--leak-check=full --track-origins=yes`, R 4.6.2 on
  aarch64 Linux): the tests and examples report `ERROR SUMMARY: 0 errors from 0 contexts` and
  `definitely lost: 0 bytes in 0 blocks`.
* AddressSanitizer and UndefinedBehaviorSanitizer (R-devel built with ASan, the C shim with
  `-fsanitize=address,undefined`): the tests (`[ FAIL 0 | SKIP 1 | PASS 40181 ]`) and examples
  run with no report, both with the Rust archive built normally and with it built under
  `-Zsanitizer=address` too. A heap overread and a signed overflow put in on purpose are
  reported.
* rchk: no function reported. Its first run on this release found the C shim's column
  allocator returning with one `PROTECT` left for its callers to balance, and a column name
  allocated while the new column was not yet in its protected list; both are fixed, and a
  `PROTECT` dropped on purpose is reported, so the run reads the package.
* LTO (`--use-LTO`, GCC 15.2, `-flto=auto -Werror=odr -Werror=lto-type-mismatch`): `Status: OK`.
* `gctorture`: the API and no-throw tests pass under `gctorture2(20)`.

## Check time

R CMD check takes 49 s on the macOS machine above, and 103 s on R-devel under x86-64
emulation, against CRAN's ten-minute norm.

## Compiled code

The package builds a Rust static library from the vendored crates in `src/rust/vendor.tar.xz`
(two crates: the `hypercast` core and `simdutf8`, which it checks UTF-8 with), offline, with
`cargo build -j 2`, `CARGO_HOME` under `src/`, and removes the Rust build directory after
linking. `configure` prints the `cargo` and `rustc` versions before anything is compiled, and
stops with a pointer to https://rustup.rs when either is missing or when `rustc` is older than
the core crate's `rust-version`, which it reads from the vendored crate's manifest (1.89 today).
The message names both versions:

    hypercast needs rustc 1.89 or newer (the hypercast crate's rust-version);
    found rustc 1.84.1.

## r-oldrel macOS

The installation logs of CRAN's r-oldrel-macos-arm64 and r-oldrel-macos-x86_64 flavors report
rustc 1.84.1 (for example
https://www.r-project.org/nosvn/R.check/r-oldrel-macos-arm64/gifski-00install.html, read
2026-11-02), older than the 1.89 the core crate requires. On those two flavors `configure`
stops with the message above, before cargo runs, as CRAN's "Using Rust" policy asks. Every
other flavor's log reports 1.92.0 or newer (r-release macOS 1.93.0, Windows 1.92.0, Debian
1.97.1, Fedora 1.99.0). The 1.89 floor is the crate's `rust-version`, which CI checks the
library on exactly, shared with its sibling crates; it is not lowered for the R package.

The wrapper crate in `src/rust/` depends on the `hypercast` crate from crates.io at exactly this
package's version, as any Rust consumer of it would, and on nothing else. The core has one
dependency, `simdutf8` 0.1.5 (Hans Kratz, MIT OR Apache-2.0), a `no_std` SIMD UTF-8 validator,
vendored with it; `inst/AUTHORS` names it, and its author is a `cph` in `Authors@R` with the
simdjson authors and the simd-json.rs developers, whose algorithm and first Rust port it builds
on, as its README credits them. The core does carry one third party's data as well: its case
folding is generated from the Unicode Character Database's `CaseFolding.txt`, included
unchanged, so Unicode, Inc. is a `cph` in `Authors@R` and is credited in `inst/AUTHORS` under
the Unicode License v3. The vendored `hypercast` is that release's source as `cargo package`
writes it, from the tagged commit the crates.io release is uploaded from.

The core is `#![no_std]`, allocation-free and panic-free, and leaves what those would need
to the binding and to R:

* No std. The crate is built with `default-features = false`: no I/O, threads, formatting
  or heap from Rust's standard library, and the archive R links carries no copy of it. It
  takes nothing from the operating system.
* No allocation. No function in the core allocates; its suite asserts zero allocations
  with a counting allocator (`tests/allocation_free.rs`). Every buffer is R's: the C shim
  writes into vectors R allocated and sized.
* No panics. No input makes the core panic; every outcome is a return code over the C ABI,
  which the shim turns into one row of the verdict data frame each cast returns. No cast
  signals an error for its data; R errors are reserved for malformed arguments. The crate
  proves it at link time: `cargo no-panic` links all 75 C exports under `#[no_panic]` in a
  release build, `simdutf8` included, and fails when one export that can panic is added on
  purpose (rustc 1.99.1, on this release's crate sources). The proof covers the Rust core,
  not the C shim, which calls only those exports and R's API. The archive is built with
  `panic = "abort"` behind a panic handler nothing reaches: the built `hypercast.so` exports
  `R_init_hypercast`, its 9 registered routines and the core's C ABI and nothing else,
  imports no `abort`, `exit` or printing function, and no symbol in it names a panic or an
  unwind.

## Windows

Rtools ships no Rust, so the Windows build uses rustup's `x86_64-pc-windows-gnu` target
(`aarch64-pc-windows-gnullvm` on ARM64), chosen by `tools/rustarch.R` in
`src/Makevars.win`. Not checked for this candidate; to be confirmed on win-builder.
