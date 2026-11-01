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
