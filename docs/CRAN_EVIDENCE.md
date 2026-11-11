# CRAN evidence ledger: hypercast and hypertabular

One row per check run on an R source tarball. A check counts only for the tarball whose SHA-256
its row names. **candidate** rows are tarballs built from a working tree; **submission** rows are
only for the tarball the tag run preserved. Each package's `cran-comments.md` quotes these rows
and says which class they are. [`CRAN_CHECKLIST.md`](CRAN_CHECKLIST.md) is the gate the rows
close.

## What reruns when a package's contents change

| Change since the row's tarball | Rerun |
| --- | --- |
| Nothing inside the tarball, so the same SHA-256 | Nothing |
| R code, Rd, tests, fixtures, DESCRIPTION, NEWS | `--as-cran` on R-devel and release, the R 4.2.3 floor, locales, noSuggests, `gctorture`, check time, size, `r_revdep_check.sh` |
| `r/src`, the vendored crates or the Rust toolchain | All of the above, plus `rchk`, valgrind, UBSAN and ASAN, LTO, `nm -g`, the no-panic proof |

`prepare-release.yml` and the tag run build a different tarball, with the crates from crates.io,
so the submission tarball always takes the last row.

## Tarballs

| Tarball | Bytes | SHA-256 | Source | Class |
| --- | --: | --- | --- | --- |
| `hypercast_1.1.0.tar.gz` (round 1) | 524,288 | `e768cda83ef1f63ca01005f7f11eb0ac6ba67740e91880dd2a2335ed555b28d0` | working tree on `cran_1_1` over `f93610d`, `rust/` content-identical to tree `fd37f6b`'s, with `rust/Cargo.toml`, `r/DESCRIPTION` and the wrapper's pin set to 1.1.0 in a scratch copy (the tree still says 1.0.0); `r/src/hypercast.c` with the rchk fix below; vendored with `r/tools/vendor.sh` | candidate |
| `hypertabular_1.0.0.tar.gz` (round 1) | 1,417,147 | `768f6ed55fff0136b39d13dd1b83adc0590a608a56f9d22344b6f611d04c26c3` | working tree on `cran_1_1` over `9d2ddd6`; `r/src/hypertabular.c` with the rchk fix below; vendored with `HYPERCAST_PATH` naming the 1.1.0 copy above, so its `hypercast` crate is that tarball's | candidate |
| `hypercast_1.1.0.tar.gz` (round 0) | 524,193 | `56f05646d8eb39595a5d848085af55014d3da5cc8439c09a13f051272187fb96` | as round 1, before the rchk fix | candidate, superseded |
| `hypertabular_1.0.0.tar.gz` (round 0) | 1,417,032 | `e5ba57d97bd361618c649fa91ffd478b23f30ff0cb6442d1816d7228d2b8fb08` | as round 1, before the rchk fix | candidate, superseded |

Both 1.1 candidates are below CRAN's 5 MB guideline and 10 MB ceiling. hypertabular's unpacks to
8.2 MB, 6.9 MB of it the corpus (`workbook.json` is 6.3 MB) and its 41 workbooks, which
compress well; hypercast's unpacks to 1.3 MB. Installed, they are 884 KB and 740 KB on macOS.

## Environments

| Name | What |
| --- | --- |
| mac | macOS 27.2, Apple M6 (aarch64-apple-darwin27.0.0), R 4.6.2 (Homebrew), rustc and cargo 1.99.1, TinyTeX, `checkbashisms`, `qpdf`, `pandoc` |
| linux | `rocker/r-ver:4.6.2` on aarch64 (Docker, 4 GiB), R 4.6.2, rustc and cargo 1.99.1, valgrind, gcc |
| floor | `rocker/r-ver:4.2.3` on aarch64 (Docker, 4 GiB), R 4.2.3, rustc and cargo 1.89.0 (and 1.84.1 for `configure`'s refusal) |
| devel | `rocker/r-devel` on x86-64 under emulation (Docker, 4 GiB), R Under development (unstable) (2026-11-01 r90825), Debian's rustc and cargo |
| rchk | `kalibera/rchk` on x86-64 under emulation (Docker, 4 GiB), rchk `059dc9a`, LLVM 14, its R-devel r89293, rustc 1.99.1 |
| rust | `rust:latest` on aarch64 (Docker, 4 GiB), rustc 1.99.1 |
| san | `wch1/r-debug` on x86-64 under emulation (Docker, 4 GiB), its `RDsan` (R-devel built with `-fsanitize=address`), shims compiled with `-fsanitize=address,undefined -fno-sanitize-recover=undefined` via `~/.R/Makevars`, rustc 1.99.1 and nightly 1.102.0 (2026-11-08) |

## Rows

All rows 2026-11-08 and 2026-11-09. Commands run from the package tarball's directory; `hc` and
`ht` are the two round-1 tarballs above unless a row says otherwise.

| # | Package | Tarball | Env | Command | Result | Class |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | hypercast | round 0 | rchk | the image's `container.sh` on the tarball | 7 functions reported: `columns_alloc` left one PROTECT for its callers (`C_cast_text`, `C_cast_double`, `C_cast_checked`, `C_cast_checked_double`: "negative depth"), and `column()` allocated a name while its new vector was unprotected | candidate, superseded |
| 2 | hypertabular | round 0 | rchk | the same | the same pattern (`columns_alloc`, `column`, `batch`) and `C_faults`, which PROTECTed inside a loop pass | candidate, superseded |
| 3 | both | round 0 | mac, linux, floor | the commands of those rows among 6 to 20 run in these environments, valgrind and LTO included | the same results as their round 1 rows. Superseded because the rchk fix changed `r/src`, not by a failure; every row below was run again on round 1 | candidate, superseded |
| 4 | hypercast | hc | rchk | the same | no function reported; 9 routines registered | candidate |
| 5 | hypertabular | ht | rchk | the same | no function reported; 30 routines registered. Negative control, the same tarball with `C_batch_check`'s `PROTECT` of `v` dropped (SHA-256 `9280b2f4…`): `unprotected variable v while calling allocating function` four times | candidate |
| 6 | hypercast | hc | mac | `_R_CHECK_CRAN_INCOMING_REMOTE_=TRUE R CMD check --as-cran`, with the PDF manual | `Status: 1 NOTE` (incoming feasibility: Maintainer, New submission); tests `[ FAIL 0 \| WARN 0 \| SKIP 1 \| PASS 40181 ]`; 49 s | candidate |
| 7 | hypertabular | ht | mac | `_R_CHECK_CRAN_INCOMING_=false R CMD check --as-cran`, with the PDF manual, hc installed | `Status: OK`; tests `[ FAIL 0 \| WARN 0 \| SKIP 2 \| PASS 1056 ]`; 53 s | candidate |
| 8 | hypertabular | ht | mac | its tests with `NOT_CRAN=true` (the two `skip_on_cran()` tests: both load orders in fresh processes, interrupts) | 1060 passed, 0 failed, 0 skipped | candidate |
| 9 | both | hc, ht | mac | `_R_CHECK_DEPENDS_ONLY_=true R CMD check --as-cran --no-manual` (noSuggests) | `Status: OK` each; 40 s and 51 s. testthat's own imports bring jsonlite, as on CRAN, so the corpus replay runs | candidate |
| 10 | both | hc, ht | mac | `LC_ALL=C LANG=C R CMD check --as-cran --no-manual` (session charset ASCII) | `Status: OK` each, tests as rows 6 and 7; 40 s and 52 s | candidate |
| 11 | both | hc, ht | mac | `LC_ALL=en_US.ISO8859-15 R CMD check --as-cran --no-manual` (session charset ISO8859-15) | `Status: OK` each, tests as rows 6 and 7; 40 s and 52 s | candidate |
| 12 | hypertabular | ht | mac | `test-lifecycle.R` with `gctorture(TRUE)` from its first line | 42 passed, 0 failed (2 `skip_on_cran()`); 937 s | candidate |
| 13 | both | hc, ht | mac | `gctorture2(20)`: hypertabular `test-values.R`; hypercast `test-api.R` and `test-no-throw.R` | 183, 579 and 923 passed, 0 failed. hypercast's `test-no-throw.R` at `gctorture(TRUE)` ran past a 20-minute limit, failing nothing (round 0) | candidate |
| 14 | hypertabular | ht | mac | the corpus replay with `tests/testthat/corpus/workbook/dates.ods` removed | 1 failure and 8 errors, no skip (whole: 689 passed) | candidate |
| 15 | hypercast, hypertabular | hc, ht | floor | `R CMD check --as-cran --no-manual`, rustc 1.89.0 | 3 and 2 NOTEs, all the image's: no network to verify the time, no `pandoc` (hypercast), and the image's hardening flags (`-Wdate-time -Werror=format-security -Wformat`, from `R CMD config CFLAGS`); tests as rows 6 and 7; 36 s and 50 s | candidate |
| 16 | both | hc, ht | floor | `R CMD INSTALL` with rustc 1.84.1 (D9) | `configure` stops before cargo runs: "hypercast needs rustc 1.89 or newer (the hypercast crate's rust-version); found rustc 1.84.1." and "hypertabular needs rustc 1.89 or newer; found 1.84.1." | candidate |
| 17 | both | hc, ht | linux | `R CMD check --as-cran --no-manual` | 1 NOTE each, the image's hardening flags as in row 15; tests as rows 6 and 7; 45 s and 52 s | candidate |
| 18 | both | hc, ht | devel | `RD CMD check --as-cran --no-manual`, rustc and cargo 1.97.1 (Debian's) | 1 NOTE each, the image's hardening flags as in row 15; tests as rows 6 and 7; 103 s and 116 s under x86-64 emulation | candidate |
| 19 | both | hc, ht | linux | `VALGRIND_OPTS="--leak-check=full --track-origins=yes" R CMD check --use-valgrind --no-manual` (tests and examples under memcheck) | `Status: OK` each; tests, and examples, of each: `ERROR SUMMARY: 0 errors from 0 contexts`, `definitely lost: 0 bytes in 0 blocks`; 1,682 s and 2,168 s | candidate |
| 20 | both | hc, ht | linux | `R CMD check --no-manual --install-args=--use-LTO`, `LTO_OPT = -flto=auto -Werror=odr -Werror=lto-type-mismatch` (GCC 15.2) | `Status: OK` each; tests as rows 6 and 7 | candidate |
| 21 | both | hc, ht | linux, mac | `nm -D --defined-only` (linux, row 20's build) and `nm -gU` (mac, row 6 and 7's) on the installed `hypercast.so` and `hypertabular.so` | hypercast: 47 global symbols, `R_init_hypercast`, 9 `C_*` routines, 35 `cast_*` and 2 `hypercast_*` (the core's C ABI); hypertabular: 54, `R_init_hypertabular`, 30 `C_*`, 23 `hypertabular_*` (exactly the export list), and none of hypercast's. Neither imports `abort`, `exit` or a printing function; no symbol names `panic` or `unwind`. Each holds one local simdutf8 function, `validate_utf8_basic_neon`, the `basic` validator only | candidate |
| 22 | hypertabular core | working tree `rust/` (the vendored crate's sources) over the 1.1.0 copy | rust | `rust/check-core.sh` | "all of it holds": no std or alloc named, `no_std` builds for `thumbv7em-none-eabi`, x86-64 and aarch64 Linux, dependencies exactly hypercast and simdutf8, 23 exports and no allocator import in the cdylib, a static library needing nothing from Rust, `cargo no-panic` over every export, the canary refused by name alone, the core's tests | candidate |
| 23 | hypercast core | the 1.1.0 copy (the vendored crate's sources) | rust | `cargo no-panic`, then the same with `--cfg hypercast_canary` | the proof passes over every export and the harness (75: 36 exports, 39 harness functions), simdutf8 linked in; with the canary it fails, naming `hypercast_canary` alone | candidate |
| 24 | hypertabular | ht, against hc | mac | `.github/scripts/r_revdep_check.sh hypercast_1.1.0.tar.gz hypertabular_1.0.0.tar.gz` (HyperTabular's repo; hc installed into a temporary library, then `R CMD check --as-cran --no-manual`) | `Status: OK`, the resolved hypercast 1.1.0 from the temporary library, exit 0; 74 s | candidate |
| 25 | hypertabular | ht, against a doctored hc (SHA-256 `d4f0f03d…`: `num_format.R` packs a 96-byte layout, which no hypertabular reads) | mac | row 24's | exit 1, `Status: 2 ERRORs` (examples, tests), each failure "hypercast 1.1.0 packs a layout of 96 bytes, and hypertabular 1.0.0 reads a number format of 30 to 92 bytes and date options of 8 to 12; install versions of the two that agree". A first doctoring to 88 bytes passed: 88 is 1.0's layout, which hypertabular still reads on purpose | candidate |
| 26 | hypertabular repo | working tree | mac | `python3 -m unittest` in `.github/scripts/tests`, with `.github/hyper.json`'s `r_corpus` given a stale entry (`workbook/basic-stale.ods`) in a scratch copy | `test_r_corpus_is_every_corpus_file_the_replay_reads` fails; unmodified, all 167 tests pass | candidate |
| 27 | both | hc, ht | linux | row 19's valgrind leg on a control: a C routine reading one `int` past a 16-byte `malloc` | `Invalid read of size 4` reported, so the leg sees the shared objects' heap reads | candidate |
| 28 | hypertabular, hypercast cores | the crate sources rows 22 and 23 name | rust | `cargo test --target wasm32-wasip1` under `wasmtime run` with the repo and target directories mounted | hypertabular: 331 passed, 0 failed, 4 ignored (the `should_panic` tests libtest skips there), 17 binaries, 213 s; hypercast: 277 passed, 0 failed, 11 binaries, 251 s | candidate |
| 29 | both | hc, ht | all | the elapsed time of rows 6, 7 and 15 to 18 | 36 s to 116 s, the longest R-devel's under x86-64 emulation; CRAN's norm is ten minutes | candidate |
| 30 | both | hc, ht | san | `RDsan CMD INSTALL` of each, then the tests (`testthat::test_dir`, installed package) and every Rd example under `RDsan` | no AddressSanitizer or `runtime error` report; tests hc 40181 passed, 1 skipped, ht 1056 passed, 2 skipped, 0 failed; examples run. The shims carry the instrumentation (488 and 1,097 `__asan_report` call sites); 569 s | candidate |
| 31 | both | hc, ht | san | row 30 with `RUSTFLAGS="-Zsanitizer=address -Cunsafe-allow-abi-mismatch=sanitizer"` on nightly, so the Rust archive is instrumented too | no report; tests as row 30; 7,103 and 11,733 `__asan_report` call sites. Controls, in each run: a C routine reading past a 16-byte `malloc` gives `AddressSanitizer: heap-buffer-overflow`, and an `int` overflow `runtime error: signed integer overflow`; 536 s | candidate |
