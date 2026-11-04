# hypercast 1.1.0

## Added

* First release of the R binding to the HyperCast core, built from source with the
  vendored core crate (no network access at install).
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
* Number formats: `num_format()`, `num_format_detect()`, `num_format_from_localeconv()`
  and `num_styles`.
* `hypercast_native_version()`: the version of the loaded Rust core.
* Strict grouping: `num_format()`, `num_format_detect()` and `num_format_from_localeconv()`
  take `grouping`, `"western"` by default (a group of exactly three digits, so `12,34` is
  `malformed`), `"indian"` or `"legacy"` (a group separator anywhere between two digits, as
  before). `flags` holds styles only. `num_format_from_localeconv()` chooses its grouping
  by the locale's own patterns, never by name: strict Western when every pattern that groups
  is three-digit, strict Indian when every one is three then two, `"legacy"` for any other
  shape or when the number and currency patterns disagree (a bridge never rejects what its
  platform writes), and the `grouping` style cleared only when neither pattern groups.
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
