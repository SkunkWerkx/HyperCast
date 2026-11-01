## Submission

This is a new submission.

## Test environments

* macOS 27.2 on Apple M6 (aarch64-apple-darwin27.0.0), R 4.6.2 (Homebrew build), rustc and
  cargo 1.99.1.

## R CMD check results

`R CMD check --as-cran hypercast_<version>.tar.gz`, with `_R_CHECK_CRAN_INCOMING_REMOTE_=TRUE`.

macOS, R 4.6.2, with the PDF manual (TinyTeX), `checkbashisms`, `qpdf` and `pandoc` installed:

    Status: 1 NOTE

    * checking CRAN incoming feasibility ... NOTE
    Maintainer: 'Brian Buvinghausen <buvinghausen@gmail.com>'

    New submission

The NOTE is the new-submission notice. The testthat expectations pass, including the replay
of the fifteen bundled corpus files every binding of the project replays. `NEWS.md`'s heading
carries the version number, which R's NEWS parser requires.

## Compiled code

The package builds a Rust static library from the vendored crate in
`src/rust/vendor.tar.xz` (one crate, the `hypercast` core), offline, with `cargo build -j 2`,
`CARGO_HOME` under `src/`, and removes the Rust build directory after linking. `configure`
prints the `cargo` and `rustc` versions and stops with a pointer to https://rustup.rs when
either is missing or `rustc` is older than 1.89.

The wrapper crate in `src/rust/` depends on the `hypercast` crate from crates.io at exactly
this package's version, as any Rust consumer of it would, and on nothing else; the core has
no dependencies of its own, so `inst/AUTHORS` names no third-party crate. The vendored
`hypercast` is that release's source as `cargo package` writes it, from the tagged commit
the crates.io release is uploaded from.

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
  proves it at link time (`cargo no-panic` links every C export under `#[no_panic]` in a
  release build in CI), and the archive is built with `panic = "abort"` behind a panic
  handler nothing reaches: in the built `hypercast.so`, `nm -g` finds no `abort` symbol,
  and the check reports no calls to `abort`, `exit` or `printf` from compiled code.

## Windows

Rtools ships no Rust, so the Windows build uses rustup's `x86_64-pc-windows-gnu` target
(`aarch64-pc-windows-gnullvm` on ARM64), chosen by `tools/rustarch.R` in
`src/Makevars.win`. Not tested locally; to be confirmed on win-builder.
