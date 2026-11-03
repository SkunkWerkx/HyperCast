# hypercast (R)

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![CRAN](https://img.shields.io/cran/v/hypercast.svg)](https://cran.r-project.org/package=hypercast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

Scalar casts from untrusted text for R: booleans, characters, the integer family, reals,
exact decimals, UUIDs, dates, times, date-times, durations, Unix and spreadsheet serial
instants. A hand-written C shim calls the Rust core through `.Call`; the core is the same one
every other HyperCast binding uses, built here from source as a static library and linked
into the package. Every cast (a door) is vectorised, and returns a verdict per element: the
value, or a closed reason with the span of the input that caused it. No door signals an
error for anything in its data. [Validation constraints](#validation-constraints) judge a
parsed value by declared rules (bounds, steps, lengths, enumerations) in the core too.

## Install

From CRAN:

```r
install.packages("hypercast")
```

CRAN ships the source package and compiles it per platform, so its binaries for Windows and
macOS need no Rust; a source install (Linux, or `type = "source"`) does. R 4.2 or newer,
on every platform.

From a source tarball, which needs Rust (`cargo` and `rustc` 1.89 or newer, from
[rustup](https://rustup.rs)):

```sh
R CMD INSTALL hypercast_<version>.tar.gz
```

The tarball is what `R CMD build r` makes from a checkout after `tools/vendor.sh` (see
[Local loop](#local-loop)). It carries the `hypercast` core crate in
`src/rust/vendor.tar.xz` and builds offline. `configure` prints the `cargo` and `rustc`
versions it found, and stops with a pointer to rustup when either is missing or `rustc` is
older than the core crate's `rust-version` (1.89), which it reads from the vendored crate's
manifest; the message names the version required and the one found.

## Usage

```r
library(hypercast)

x <- c("42", "(1,234)", "12x", "", NA)
v <- cast_i32(x, num_format())
v
#> <hypercast_verdict: cast_i32> 2 ok, 2 faulted, 1 missing
#>   value    ok    reason offset length
#> 1    42  TRUE      <NA>     NA     NA
#> 2 -1234  TRUE      <NA>     NA     NA
#> 3    NA FALSE malformed      2      1
#> 4    NA FALSE     empty      0      0
#> 5    NA    NA      <NA>     NA     NA

faults(v)
#>   row    reason offset length
#> 1   3 malformed      2      1
#> 2   4     empty      0      0
with(faults(v), substr(x[row], offset + 1, offset + length))
#> [1] "x" ""
optional(v)$ok
#> [1]  TRUE  TRUE FALSE    NA    NA

eurozone <- num_format(",", ".", currency = "€")
cast_decimal("1.234,50 €", eurozone)$value
#> [1] "1234.5"
cast_date(c("2026-02-30", "01/11/2026"), "day_month_year")
#> <hypercast_verdict: cast_date> 1 ok, 1 faulted, 0 missing
#>        value    ok       reason offset length
#> 1       <NA> FALSE out_of_range      8      2
#> 2 2026-11-01  TRUE         <NA>     NA     NA
cast_timestamp("2026-11-01T12:34:56.789Z")$value
#> [1] "2026-11-01 12:34:56 UTC"
cast_u64("18446744073709551615", num_format())$value
#> [1] "18446744073709551615"
```

## The verdict

HyperCast's verdict is a two-case union: the value, or a fault. R holds a union over a
vector as a data frame, one column per field, so every door returns a `hypercast_verdict`
(a `data.frame` subclass) with one row per input element, in order:

| Column | Success (`ok` is `TRUE`) | Fault (`ok` is `FALSE`) | Missing (`ok` is `NA`) |
| --- | --- | --- | --- |
| `value` | the value, in the door's R type | `NA` | `NA` |
| `ok` | `TRUE` | `FALSE` | `NA` |
| `reason` | `NA` | factor: `empty`, `malformed`, `out_of_range` | `NA` |
| `offset` | `NA` | where the offending text starts, from 0 | `NA` |
| `length` | `NA` | how long it runs | `NA` |

`ok` is the discriminant: `v$value[v$ok %in% TRUE]` is the parsed values, and `is_ok(v)`
is the column itself. `NA` in is `NA` out: a missing element is a missing row,
never a fault. `faults(v)` is the faulted rows with `row` naming each one's element.
`optional(v)` turns `empty` faults (empty or ASCII-whitespace input) into missing rows, the
same presentation every binding ships as `optional`. A verdict subsets, binds and converts as
the data frame it is; `as.data.frame()` drops the class.

A door never signals an R condition for its data: no error, no warning, not for invalid
UTF-8, a 100,000-digit number or a `NaN`. R's argument errors are reserved for caller bugs in
the arguments that declare how to read the data (a malformed `num_format()`, an unknown
order, precision or epoch, an `x` of the wrong type): what the C ABI calls a contract
violation, and what Python, for one, raises `ValueError` and `TypeError` for.

### Spans and units

`offset` and `length` count in the units `substr()` counts the input in, so
`substr(x, offset + 1, offset + length)` gives back the offending text:

- characters (code points) for a character vector, whatever its encoding mark; a `latin1`
  string is read as UTF-8 after R translates it;
- bytes for a list of raw vectors (`NULL` elements are missing), which reaches the core
  exactly as given, invalid UTF-8 included;
- bytes for a string marked `"bytes"`, and for an unmarked string in a session whose locale
  is ASCII (the C locale), whose bytes are read as UTF-8 as they are, since R has no encoding
  to translate them from.

The core reports a span in UTF-8 bytes and the shim re-expresses it. The typed doors read
a double rather than text, and fault over an empty span (`0`, `0`).

### Values

Each value comes back in the R type that holds every value of the target exactly; where one
column cannot carry everything the core returned, the exact parts follow as columns of
their own.

| Door | `value` | Exact columns |
| --- | --- | --- |
| `cast_bool()` | logical | |
| `cast_char()` | one-character string | |
| `cast_i8()`, `cast_i16()`, `cast_u8()`, `cast_u16()` | integer | |
| `cast_i32()`, `cast_u32()` | double (R's integer has no `-2^31`) | |
| `cast_i64()`, `cast_u64()` | character, the exact decimal integer | |
| `cast_f32()`, `cast_f64()` | double | |
| `cast_decimal()`, `cast_decimal_from_double()` | character, the exact canonical decimal | |
| `cast_uuid()` | character, lowercase hyphenated | |
| `cast_timestamp()`, `cast_unix()`, `cast_excel_serial()` | `POSIXct` in UTC | `seconds` (double), `nanos` (integer) |
| `cast_date()` | `Date` | |
| `cast_datetime()`, `cast_excel_serial_from_double()` | character wall clock, `"2026-01-07 15:04:05.123"` | `date` (`Date`), `nanos_of_day` (double) |
| `cast_time()`, `cast_excel_time()` | `difftime`, seconds since midnight | `nanos` (double) |
| `cast_duration()`, `cast_excel_duration()` | `difftime` in seconds | `seconds` (double), `nanos` (integer) |

A civil date-time names no instant until the caller names a zone:
`as.POSIXct(v$value, tz = "America/Chicago")`. `cast_char()` reads U+0000 as `out_of_range`
over the input less its ASCII whitespace edges, since an R string cannot hold it; C# and Java
report a scalar their UTF-16 `char` cannot hold the same way.

### Declarations

Nothing is guessed. The number doors take a `num_format()`: decimal and group separators,
style flags (`num_styles`: grouping, parentheses, exponent, radix prefixes, percent,
separator detection, currency, trailing sign), currency, sign and percent spellings and a
native digit zero. `num_format()` with no arguments is the invariant format with every style
but detection; `num_format_detect()` adds detection; `num_format_from_localeconv()` bridges
`Sys.localeconv()` (or any named vector of its fields). `cast_date()` and `cast_datetime()`
take a field `order` (`"year_month_day"`, `"month_day_year"`, `"day_month_year"`), with
`NULL` for ISO 8601 only; `cast_datetime()` takes `am` and `pm` day periods; `cast_unix()`
takes its `precision` and the Excel doors their `epoch` (`"y1900"`, `"y1904"`).

`hypercast_native_version()` is the version of the loaded core, which equals the package
version.

## Validation constraints

A value that parsed can still break a rule the data declares: a latitude past 90, a cash
amount that is not a multiple of five cents, a code outside its list. `hypercast_constraints()`
declares those rules for one door, with JSON Schema's and OpenAPI's keywords, and the checked
doors judge each value by them in the core, on the exact value the door read:

```r
latitude <- hypercast_constraints("f64", minimum = "-90", maximum = "90",
                                  multiple_of = "0.000001", rounding = "midpoint")
cast_checked(c("45.1234567", "91", "north"), num_format(), latitude)
#> <hypercast_checked_verdict: f64> 1 ok, 1 faulted, 1 violated, 0 missing
#>      value adjusted enum_index    ok    reason keyword offset length
#> 1 45.12346     TRUE         NA  TRUE      <NA>    <NA>     NA     NA
#> 2       NA       NA         NA FALSE      <NA> maximum      0      2
#> 3       NA       NA         NA FALSE malformed    <NA>      0      1

cash <- hypercast_constraints("decimal", multiple_of = "0.05")
euro <- num_format(",", ".", currency = "€")
v <- cast_checked(c("12,35 €", "12,37 €", ""), euro, cash)
violations(v)
#>   row    keyword offset length
#> 1   2 multipleOf      0      7
faults(v)
#>   row reason offset length
#> 1   3  empty      0      0

sex <- hypercast_constraints("text", enum = c("Female", "Male", "Other"), trim = TRUE)
cast_text(c(" male ", "OTHER", "Mail"), sex)
#> <hypercast_checked_verdict: text> 2 ok, 0 faulted, 1 violated, 0 missing
#>   value adjusted enum_index    ok reason keyword offset length
#> 1  Male    FALSE          2  TRUE   <NA>    <NA>     NA     NA
#> 2 Other    FALSE          3  TRUE   <NA>    <NA>     NA     NA
#> 3  <NA>       NA         NA FALSE   <NA>    enum      0      4
```

The keywords are `minimum`, `exclusive_minimum`, `maximum`, `exclusive_maximum` and
`multiple_of` (with `rounding`: `"strict"`, the default, `"midpoint"` to the nearest
multiple with a tie to the even one, `"floor"` or `"ceiling"`) for the numeric doors, `enum`
for the integer doors, and `min_length`, `max_length` (code points), `trim` (Unicode
`White_Space`) and `enum` (matched under Unicode simple case folding) for the text door.
Bounds and `multiple_of` are exact: a string (`"0.000001"`) or an R integer (`18L`), never
a double, since `0.1` has no exact double; `maximum = 90` is an error, `maximum = "90"` or
`90L` is not. Whole numbers past R's integer range are strings. A float door also judges the
float it delivers against each bound read as the door reads it: `"1e-400"` at `"f64"` reads as
`0`, so it breaks `exclusive_minimum = "0"`.

A declaration is judged once, when it is declared, and the door is part of it. One that
cannot be is an error worded from the code the core returns, such as
`Constraints refused (code 4): the lower bound is not below the upper bound.` for
`hypercast_constraints("i32", minimum = "5", maximum = "5")`. `enum = NULL` declares none,
while an empty vector (`character(0)`) declares one no value could match and is refused (code
12), as is a text entry no input can match (code 13: `""`, or with `trim` one with leading or
trailing white space). A declaration handed to another
door's cast is an error, and so is `cast_text(x, NULL)`: only `cast_text(x)`, with no
constraints at all, is the plain text door, which validates UTF-8 and returns the text as a
`hypercast_verdict`.

- `cast_checked(x, format, constraints)` reads text through a numeric door under a
  `num_format()`, so `"$12.37"`, `"(12.35)"`, `"1.234,55"` and `"5%"` are judged on the
  number the door reads (`5%` is `0.05`).
- `cast_checked_from_double(x, constraints)` judges a number already decoded (a spreadsheet
  cell, a JSON number) on its shortest round-trip decimal, so `0.1 + 0.2` is
  `0.30000000000000004` and a `multipleOf` violation under `"strict"`. `-0` is zero, and no
  value comes back as `-0`.
- `cast_text(x, constraints)` judges text. A text `enum` match is the entry exactly as you
  declared it, and the input's own string comes back when nothing was trimmed.

Each returns a `hypercast_checked_verdict`, a data frame of its own class (the plain doors'
`hypercast_verdict` keeps exactly its columns):

| Column | Passed (`ok` `TRUE`) | Fault (`ok` `FALSE`) | Violation (`ok` `FALSE`) |
| --- | --- | --- | --- |
| `value` | the value, in the plain door's R type | `NA` | `NA` |
| `adjusted` | `TRUE` when rounding changed the value | `NA` | `NA` |
| `enum_index` | the matched text `enum` entry, counted from 1; else `NA` | `NA` | `NA` |
| `ok` | `TRUE` | `FALSE` | `FALSE` |
| `reason` | `NA` | `empty`, `malformed`, `out_of_range` | `NA` |
| `keyword` | `NA` | `NA` | the rule broken |
| `offset`, `length` | `NA` | the offending text | the whole value |

A missing element is a missing row, `NA` throughout. A violation is never a reason: it is a
value that parsed and broke a rule, named in `keyword` with OpenAPI's spelling (`"minimum"`,
`"exclusiveMinimum"`, `"maximum"`, `"exclusiveMaximum"`, `"multipleOf"`, `"minLength"`,
`"maxLength"`, `"enum"`). The list is open, since `format` and `pattern` may join it in a minor
release, and a keyword this version does not know reads as `"keyword N"`, so a `switch()` on
`keyword` needs a default. Spans count in the same units as a plain verdict's (a typed door's
are empty). `faults()` lists the faulted rows, `violations()` the violated ones (`row`,
`keyword`, `offset`, `length`), and `optional()` turns `empty` faults into missing rows and
leaves violations as they are. An integer `enum` gives no index: the value is the answer.

## Benchmarks

Apple M6 (Mac18,5), macOS, R 4.6.2 (Homebrew build), measured 2026-11-01, against base R's
coercions with bench 1.1.4 (`R_LIBS=<lib> Rscript bench/bench.R` against an installed
copy), 1000 strings per call. Base R returns `NA` where a door returns a fault and its span,
so the two do different work; base R is the call an R user reaches for today. These were
measured on a different machine and OS from the other bindings' figures and are not
comparable to them.

```text
2026-11-01, R version 4.6.2 (2026-10-29), Apple M6 (Mac18,5)
hypercast 1.0.0, bench 1.1.4

 expression                                                median  itr/sec mem_alloc
 hypercast::cast_f64(x, num_format())                      27.84µs 35807    79.93KB
 as.numeric(x)                                             40.26µs 24729     7.86KB
 hypercast::cast_i32(x, num_format())                      25.58µs 38946    34.77KB
 as.integer(x)                                             30.95µs 32169     3.95KB
 hypercast::cast_bool(x)                                   18.25µs 54214    22.19KB
 as.logical(x)                                             10.58µs 93887     3.95KB
 hypercast::cast_date(x)                                   19.93µs 50212    29.16KB
 as.Date(x)                                                 2.29ms   434   138.23KB
 hypercast::cast_timestamp(x)                              27.14µs 36906    37.93KB
 as.POSIXct(x, tz = "UTC", format = "%Y-%m-%dT%H:%M:%OSZ")  3.83ms   258   126.36KB
```

| Comparison (median, 1000 strings) | hypercast | base R | Result |
| --- | --- | --- | --- |
| reals | 27.84 µs | 40.26 µs | hypercast 1.4x faster |
| 32-bit integers | 25.58 µs | 30.95 µs | hypercast 1.2x faster |
| booleans | 18.25 µs | 10.58 µs | **hypercast 1.7x slower** |
| ISO dates | 19.93 µs | 2.29 ms | hypercast 115x faster |
| ISO instants | 27.14 µs | 3.83 ms | hypercast 141x faster |

Every door allocates more than its base R counterpart: the verdict's five columns (and a
timestamp's two exact ones) are R vectors of the input's length.

## Local loop

From `r/`, with Rust, GNU tar (`brew install gnu-tar` on macOS) and the R packages testthat,
jsonlite and devtools:

```sh
tools/vendor.sh            # derive the lock, vendor the core, stage the header and corpus, write inst/AUTHORS
tools/vendor.sh --verify   # the vendor tarball, the lock, the header and the corpus are present and current
Rscript -e 'devtools::test()'
cd .. && R CMD build r
_R_CHECK_CRAN_INCOMING_=false R CMD check --as-cran hypercast_<version>.tar.gz
```

`tools/vendor.sh --stage` does everything but `inst/AUTHORS`, the one step that needs
jsonlite. The wrapper crate in `src/rust/` depends on the `hypercast` crate by exact version,
as any crates.io consumer does, so the graph CRAN builds is the published one; `vendor.sh`
resolves that version from the checkout's `rust/` (a command-line `[patch]`, never a committed
file, and it refuses a pin that differs from `rust/Cargo.toml`), adds the core as `cargo
package` writes it, and rewrites the lock offline so it names the crates.io source. The
wrapper's `src/rust/Cargo.lock`, the vendor tarball, the C header under `src/include/` (the
checkout's `include/hypercast.h`) and the corpus copy under `tests/testthat/corpus/` are
generated and gitignored; the tarball `R CMD build` makes carries them all. Rerun
`tools/vendor.sh` after any change in `rust/`, `include/` or `corpus/`: the Rust build is
offline from the vendor tarball only, and stops without it rather than fetch the published
core. A missing corpus file is a test failure, never a skip; the replay skips only when
jsonlite (a Suggests) is absent, as on CRAN's no-Suggests check.

The suite replays every corpus file twice, as raw vectors (byte spans exactly as the corpus
states them) and as strings (spans in characters), with the constraints corpus's invalid
UTF-8 as raw vectors and `"bytes"` strings and its declarations through the one conversion
the core exports, compares every double by its bits, holds every door, checked doors
included, to signalling nothing over every truncation of every corpus input and a set of
hostile strings, and covers the API around both verdicts.

Lint against an installed copy: lintr's object-usage linter loads the package it lints, and
without one every exported name is a false positive.

```sh
R CMD INSTALL --library=/tmp/rlib .
R_LIBS=/tmp/rlib Rscript -e 'print(c(lintr::lint_package(), lintr::lint_dir("bench"), lintr::lint_dir("tools")))'
cargo fmt --check --manifest-path src/rust/Cargo.toml
clang-format --dry-run -Werror src/*.c src/*.h
```

`lint_package()` reads `.lintr` and covers `R/` and `tests/`; the two `lint_dir()` calls
cover the directories outside the package. An empty list is a pass. The C shim is formatted
by `clang-format` with `.clang-format`; the staged header under `src/include/` is cbindgen's
and is not.

## Windows

Rtools ships no Rust, so a Windows source build needs rustup's `x86_64-pc-windows-gnu`
target (`rustup target add x86_64-pc-windows-gnu`), which `src/Makevars.win` picks through
`tools/rustarch.R`. On ARM64 it picks `aarch64-pc-windows-gnullvm`; R on Windows ARM64 is
experimental and that build is untested. CRAN's own Windows binaries are built by CRAN, so
installing them needs no Rust.

## License

MIT; see [LICENSE.md](LICENSE.md).
