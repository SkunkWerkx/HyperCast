# hypercast 1.1.0

## Added

* First release of the R binding to the HyperCast core, built from source with the
  vendored core crate and simdutf8, the crate its text door checks UTF-8 with (no network
  access at install).
* Text doors, each vectorised over a character vector or a list of raw vectors:
  `cast_bool()`, `cast_char()`, `cast_uuid()`, `cast_i8()`, `cast_i16()`, `cast_i32()`,
  `cast_i64()`, `cast_u8()`, `cast_u16()`, `cast_u32()`, `cast_u64()`, `cast_f32()`,
  `cast_f64()`, `cast_decimal()`, `cast_timestamp()`, `cast_unix()`,
  `cast_excel_serial()`, `cast_date()`, `cast_datetime()`, `cast_time()` and
  `cast_duration()`.
* Typed doors over a double vector: `cast_decimal_from_double()`,
  `cast_excel_serial_from_double()`, `cast_excel_time()` and `cast_excel_duration()`.
* Every door returns a `hypercast_verdict`, a data frame with one row per element: the
  value, or the fault's reason and its span in the input's own units. No door signals an
  error for its data. `is_ok()`, `faults()` and `optional()` read and present a verdict.
* `ok` describes the core's acceptance, and `value = NA` on an `ok` row means R can't represent
  the value. `cast_char()` has a `code_point` column, which is U+0000's exact output, and
  `cast_text()` reads text with an embedded NUL or over `INT_MAX` bytes as `ok` with `NA`, with
  `raw = TRUE` for the exact bytes as a list of raw vectors.
* Number formats: `num_format()`, `num_format_detect()`, `num_format_from_localeconv()`
  and `num_styles`.
* `hypercast_native_version()`: the version of the loaded Rust core.
* No policy reads a first group that starts with a zero: `0,125` is `malformed`
  (`group_size`), never 125.
* Grouping policies: `num_format()` and `num_format_detect()` take `grouping`, `"western"` by
  default (groups of exactly three digits, so `1,23,456` and `12,34` are `malformed`) or
  `"indian"` (`1,23,456` reads, `1,234,567` is `malformed`). `num_format_with_grouping()` chooses
  either on a format already made. What moves from 1.0: every shape neither policy names is
  refused, `1,23,456` included unless Indian is declared. `flags` holds styles only, and a stale
  flags word holding bit 8, such as 351, is refused. `num_format_from_localeconv()` follows the
  locale's number pattern, never its name: three then twos is `"indian"`, anything else
  `"western"`, and the `grouping` style is cleared only when neither pattern groups. R holds
  `LC_NUMERIC` at `"C"`, so the default `conv` reads `"western"` in an Indian locale. Shapes no
  policy reads (`tok`, `cmn_TW`, `unm_US`) are refused under a declared format;
  separator detection infers them and can read `1,2345` as 1.2345.
* Fault detail: every verdict ends with `detail` (a factor over `fault_details`, with an
  `unknown` level) and `detail_code`; `faults()` lists them. The package refuses to load
  against a core that does not enforce grouping policies.
* Opt-in date shapes: `date_options()` (`date_flags`, a two-digit-year `pivot`) and
  `month_names()`, taken as `options` and `months` by `cast_date()`, `cast_datetime()` and
  (`options`) `cast_timestamp()`.
* Validation constraints: `hypercast_constraints()` declares a door's rules (bounds,
  `multiple_of` with a rounding, lengths, trimming and enumerations), judged once when
  declared; `cast_checked()`, `cast_checked_from_double()` and `cast_text()` with
  constraints judge each value by them and return a `hypercast_checked_verdict`, read with
  `faults()`, `violations()` and `optional()`. `cast_text()` alone is the text door.
