# The doors' shared plumbing: the input check and the one call into the C shim.

# x as the shim takes it: a character vector (a factor read as its levels, a bare NA as a
# missing string), or a list of raw vectors (NULL elements missing; a bare raw vector is one
# input). Anything else is a caller bug, as every other binding's text check is.
as_text <- function(x) {
  if (is.factor(x) || (is.logical(x) && all(is.na(x)))) {
    return(as.character(x))
  }
  if (is.character(x)) {
    return(x)
  }
  if (is.raw(x)) {
    return(list(x))
  }
  if (is.list(x) && !is.object(x) &&
        all(vapply(x, function(e) is.null(e) || is.raw(e), logical(1)))) {
    return(x)
  }
  stop("x must be a character vector or a list of raw vectors", call. = FALSE)
}

as_number <- function(x) {
  if ((is.numeric(x) && !is.object(x)) || (is.logical(x) && all(is.na(x)))) {
    return(as.double(x))
  }
  stop("x must be a numeric vector", call. = FALSE)
}

text_door <- function(x, door, format = NULL, periods = NULL, code = 0L, options = NULL,
                      months = NULL) {
  cols <- .Call(C_cast_text, as_text(x), door, format, periods, code, session_native(),
                check_date_options(options), check_months(months))
  new_verdict(cols, door)
}

# How this session holds an unmarked string, for the shim: 0 UTF-8; 1 ASCII (the C locale),
# whose non-ASCII bytes R can neither translate nor count as characters, so they are read as
# they are and counted in bytes, as substr() counts them; 2 any other locale, translated.
# Windows names no codeset: its C locale is code page 0.
session_native <- function(info = l10n_info()) {
  if (isTRUE(info[["UTF-8"]])) {
    return(0L)
  }
  if (identical(info[["codepage"]], 0L)) {
    return(1L)
  }
  codeset <- toupper(if (is.null(info[["codeset"]])) "" else info[["codeset"]])
  if (codeset %in% c("US-ASCII", "ANSI_X3.4-1968", "ASCII")) 1L else 2L
}

double_door <- function(x, door, code = 0L) {
  new_verdict(.Call(C_cast_double, as_number(x), door, code), door)
}

declared <- function(value, choices, what) {
  if (!is.character(value) || length(value) != 1L || is.na(value) ||
        !value %in% choices) {
    stop(sprintf("%s must be one of %s", what, paste0("\"", choices, "\"", collapse = ", ")),
         call. = FALSE)
  }
  match(value, choices)
}

date_order_code <- function(order) {
  declared(order, c("year_month_day", "month_day_year", "day_month_year"), "order")
}

epoch_code <- function(epoch) declared(epoch, c("y1900", "y1904"), "epoch")

#' Booleans, characters and UUIDs
#'
#' `cast_bool()` reads `true`/`false` and the conventions untrusted sources send: `t`/`f`,
#' `yes`/`no`, `y`/`n`, `1`/`0`, `on`/`off`, `enabled`/`disabled`, `active`/`inactive`,
#' `checked`/`unchecked`, `in`/`out`, ASCII case-insensitive.
#'
#' `cast_char()` reads one character: an input of exactly one character is that character
#' verbatim, before any trimming (`" "` is a space, `"6"` the digit six); anything else is
#' one declared code point, `65`, `U+0041`, `0x41`, `&H41`, or an HTML entity `&#65;` or
#' `&#x41;` (the `;` required), ASCII case-insensitive on the prefix, with optional white
#' space around it. A surrogate or anything past `U+10FFFF` is `out_of_range`. `U+0000`, the
#' NUL character, is a valid value an R string cannot hold, so it is `ok` with `NA` as its
#' `value`: a missing input has `ok` `NA` and a fault has `ok` `FALSE`, so nothing else reads so.
#'
#' `cast_uuid()` reads every .NET `Guid` text form (`D`, `N`, `B`, `P` and `X`) and the
#' `urn:uuid:`, `GUID:` and `UUID:` prefixes.
#'
#' @param x A character vector (a factor is read as its levels), or a list of raw vectors
#'   holding UTF-8 bytes (a bare raw vector is one input). `NA` and `NULL` elements are
#'   missing.
#' @return A [hypercast_verdict]. Its `value` is logical for `cast_bool()`; a
#'   one-character string for `cast_char()`; the lowercase hyphenated form for
#'   `cast_uuid()`.
#' @family doors
#' @export
#' @examples
#' cast_bool(c("yes", "Off", "maybe", NA))
#' cast_char(c("A", "U+00E9", "&#x1F600;", "U+D800"))$value
#' cast_uuid("{6BA7B810-9DAD-11D1-80B4-00C04FD430C8}")$value
cast_bool <- function(x) text_door(x, "bool")

#' @rdname cast_bool
#' @export
cast_char <- function(x) text_door(x, "char")

#' @rdname cast_bool
#' @export
cast_uuid <- function(x) text_door(x, "uuid")

#' Integers, reals and decimals
#'
#' The numeric doors read text under a declared [num_format()]: its separators, grouping,
#' accounting parentheses `(1,234)`, exponents `1e3`, radix prefixes `0x`/`&H`/`0b` (integer
#' doors, as two's-complement bit patterns: `0xFF` is `-1` for `cast_i8()`), a trailing
#' percent (real and decimal doors: `50%` is `0.5`) and a declared currency symbol at either
#' edge, each as the format declares. A value outside the target's range is
#' `out_of_range`; the doors never wrap, saturate or round.
#'
#' Each value comes back in the R type that holds every value of the target exactly:
#'
#' - `cast_i8()`, `cast_i16()`, `cast_u8()`, `cast_u16()`: integer.
#' - `cast_i32()`, `cast_u32()`: double. R's integer has no room for `-2^31`, its `NA`, nor
#'   for anything past `2^31 - 1`.
#' - `cast_i64()`, `cast_u64()`: character, the exact decimal integer. No R type holds 64
#'   bits exactly (`as.numeric()` for a double, `bit64::as.integer64()` for a signed 64-bit
#'   integer).
#' - `cast_f32()`, `cast_f64()`: double (an `f32` widened exactly). Finite values only:
#'   `NaN` and `Inf` text is `malformed`, overflow `out_of_range`.
#' - `cast_decimal()`: character, the exact decimal, canonical (`1.10` comes out `1.1`),
#'   never formed as a double. A magnitude past 96 bits or more than 28 places is
#'   `out_of_range`.
#'
#' @inheritParams cast_bool
#' @param format A [num_format()]: the declared separators and styles.
#' @return A [hypercast_verdict].
#' @family doors
#' @export
#' @examples
#' cast_i32(c("42", "(1,234)", "1e3", "0x2A", "2147483648"), num_format())
#' cast_u8("0xFF", num_format())$value
#' cast_i64("-9223372036854775808", num_format())$value
#' cast_f64(c("25.5%", "1e400", "NaN"), num_format())
#' cast_decimal(c("0.1", "1.10", "$1,234.50"), num_format(currency = "$"))$value
cast_i8 <- function(x, format) text_door(x, "i8", check_format(format))

#' @rdname cast_i8
#' @export
cast_i16 <- function(x, format) text_door(x, "i16", check_format(format))

#' @rdname cast_i8
#' @export
cast_i32 <- function(x, format) text_door(x, "i32", check_format(format))

#' @rdname cast_i8
#' @export
cast_i64 <- function(x, format) text_door(x, "i64", check_format(format))

#' @rdname cast_i8
#' @export
cast_u8 <- function(x, format) text_door(x, "u8", check_format(format))

#' @rdname cast_i8
#' @export
cast_u16 <- function(x, format) text_door(x, "u16", check_format(format))

#' @rdname cast_i8
#' @export
cast_u32 <- function(x, format) text_door(x, "u32", check_format(format))

#' @rdname cast_i8
#' @export
cast_u64 <- function(x, format) text_door(x, "u64", check_format(format))

#' @rdname cast_i8
#' @export
cast_f32 <- function(x, format) text_door(x, "f32", check_format(format))

#' @rdname cast_i8
#' @export
cast_f64 <- function(x, format) text_door(x, "f64", check_format(format))

#' @rdname cast_i8
#' @export
cast_decimal <- function(x, format) text_door(x, "decimal", check_format(format))

#' Instants
#'
#' `cast_timestamp()` reads RFC 3339 with a mandatory zone (`Z` or an offset), normalised to
#' UTC. `cast_unix()` reads an integer count since the Unix epoch in a declared unit: there is
#' no guessing a unit from a magnitude. `cast_excel_serial()` reads a spreadsheet date serial
#' under a declared date system, a workbook-level setting no cell carries; in the 1900
#' system serial `60` is the `1900-02-29` that never existed (Lotus 1-2-3's leap-year bug,
#' kept by Excel) and is `out_of_range`, and every serial past it is shifted one day. A serial
#' names no zone, and is read as UTC. Instants run from `0001-01-01T00:00:00Z` to
#' `9999-12-31T23:59:59.999999999Z`; a fraction past nine digits truncates to the nanosecond.
#'
#' Strict RFC 3339 is the default, and a caller can rely on it: `2026-01-07 15:04:05Z`
#' (a space for the `T`) and `+00` (an offset of hours alone) are `malformed`. `options`, a
#' [date_options()], opts in to either (`"space_separator"`, `"hour_offset"`, or both with
#' `"common_forms"`), and nothing else in it applies to this door.
#'
#' `value` is a `POSIXct` in UTC: a double of seconds, cut to the millisecond (never rounded, so
#' it cannot carry into the next second). The exact instant is in two more columns,
#' protobuf's `Timestamp` pair: `seconds` (a whole double, exact) and `nanos` (an integer,
#' `0` to `999999999`, counting forward from `seconds` even before the epoch).
#'
#' @inheritParams cast_bool
#' @param precision The unit: `"seconds"`, `"milliseconds"`, `"microseconds"` or
#'   `"nanoseconds"`.
#' @param epoch The date system: `"y1900"` (serial `1` is 1900-01-01, the Windows default)
#'   or `"y1904"` (serial `0` is 1904-01-01).
#' @param options `NULL` (the default, today's shape rules), or a [date_options()] declaration
#'   of the opt-in shapes, which the door reads where it applies and ignores otherwise.
#' @return A [hypercast_verdict] with columns `value`, `seconds` and `nanos` before the
#'   verdict's own.
#' @family doors
#' @export
#' @examples
#' cast_timestamp(c("2026-01-02T15:04:05.123456789+05:00", "2026-01-02T15:04:05"))
#' cast_unix("1700000000123", "milliseconds")$value
#' cast_excel_serial(c("45292.75", "60"), "y1900")
cast_timestamp <- function(x, options = NULL) text_door(x, "timestamp", options = options)

#' @rdname cast_timestamp
#' @export
cast_unix <- function(x, precision) {
  code <- declared(precision, c("seconds", "milliseconds", "microseconds", "nanoseconds"),
                   "precision")
  text_door(x, "unix", code = code)
}

#' @rdname cast_timestamp
#' @export
cast_excel_serial <- function(x, epoch) text_door(x, "excel_serial", code = epoch_code(epoch))

#' Dates, times and durations
#'
#' `cast_date()` reads a strict `yyyy-MM-dd` date (a real calendar date, leap days and all)
#' without `order`. With `order` it reads separated dates (`1/7/2026`, `1.7.2026`,
#' `1-7-2026`) in the declared field order: `"1/7/2026"` is January 7th under
#' `"month_day_year"` and July 1st under `"day_month_year"` only because the caller said
#' which. A four-digit first field is a year, so ISO dates read under any order.
#'
#' The shapes beyond those are all opt-in, declared with `options` ([date_options()]) and
#' `months` ([month_names()]); without them every door keeps today's rules. `"compact_date"`
#' reads `20260107` at `cast_date()` without `order`; a `pivot` reads two-digit years at the
#' ordered doors (`1/7/26`, the one year in the 100-year window ending at the pivot); and
#' `months` reads a declared month name in the month field, Excel style (`07-Jan-26`,
#' `7-Jan-2026`), in the position `order` selects. A `months` vocabulary without `order` at
#' `cast_date()` can only be a mistake (the ISO door reads no month names) and is an error.
#'
#' `cast_datetime()` reads the zone-less civil date-times of the AM/PM world: a date in the
#' declared order and an optional 24-hour or `AM`/`PM` time (`1/7/2026 3:04 PM`,
#' `2026-01-07T15:04:05`, `3 PM`, `12 AM` is midnight). No zone is read and none is invented:
#' zoned text is `malformed` here, and [cast_timestamp()] reads it. `am` and `pm` declare a
#' culture's day-period words (`"vorm."`, `"a. m."`), read exactly as spelled beside the
#' English `AM` and `PM`, which are read case-insensitively.
#'
#' `cast_time()` reads a 24-hour `HH:mm[:ss[.f]]` time of day. `cast_duration()` reads an
#' ISO 8601 duration of fixed components (`P1DT6H30M15.5S`; years and months are not fixed
#' and are refused), the invariant colon form (`1.06:30:00`) or protobuf's JSON seconds
#' (`3.5s`), with ISO 8601's comma decimal mark accepted in all three, within 10,000 years
#' either way. A fraction past nine digits truncates to the nanosecond, never rounds. The
#' wrong shape is `malformed`; the right shape naming nothing real (month `13`, `02-30`,
#' `24:00`, a `:60` second) is `out_of_range` at that field.
#'
#' Each value comes back as:
#'
#' - `cast_date()`: `Date`.
#' - `cast_datetime()`: character, the wall clock as `"2026-01-07 15:04:05.123"`
#'   (`as.POSIXct(value, tz = zone)` names its instant once you name the zone), with the
#'   exact columns `date` (`Date`) and `nanos_of_day` (double).
#' - `cast_time()`: `difftime` in seconds since midnight, with `nanos` (double).
#' - `cast_duration()`: `difftime` in seconds, with `seconds` (double) and `nanos`
#'   (integer, the sign of `seconds`).
#'
#' A `difftime` holds seconds as a double, cut to the millisecond toward zero (never rounded, so
#' a time of day stays inside its day); the exact columns hold every nanosecond.
#'
#' @inheritParams cast_bool
#' @param order The field order: `"year_month_day"`, `"month_day_year"` or
#'   `"day_month_year"`. `NULL`, for `cast_date()`, reads strict ISO only.
#' @param am,pm A culture's day-period words, read beside `AM` and `PM`: 1 to 16 UTF-8 bytes
#'   with no ASCII digit and no white space at either end; `""` declares none.
#' @param options `NULL` (the default, today's shape rules), or a [date_options()] declaration
#'   of the opt-in shapes, which the door reads where it applies and ignores otherwise.
#' @param months `NULL` (the default, numeric months only), or a [month_names()] vocabulary
#'   the month field may be spelled with.
#' @return A [hypercast_verdict].
#' @family doors
#' @export
#' @examples
#' cast_date(c("2024-02-29", "2026-02-29", "1/7/2026"))
#' cast_date("1/7/2026", order = "day_month_year")$value
#' v <- cast_datetime("1/7/2026 3:04 PM", "month_day_year")
#' v
#' as.POSIXct(v$value, tz = "America/Chicago")
#' cast_datetime("7.1.2026 3:04 nachm.", "day_month_year", am = "vorm.", pm = "nachm.")$value
#' cast_date(c("20260107", "1/7/26"), "month_day_year", date_options("common_forms", 2029))
#' months <- month_names(c(Jan = 1, Feb = 2, Mar = 3, Apr = 4, May = 5, Jun = 6, Jul = 7,
#'                         Aug = 8, Sep = 9, Oct = 10, Nov = 11, Dec = 12))
#' cast_date("07-Jan-26", "day_month_year", date_options(pivot = 2029), months)$value
#' cast_time(c("15:04:05.123456789", "24:00"))
#' cast_duration(c("P1DT6H", "-1.5s", "P1Y"))
cast_date <- function(x, order = NULL, options = NULL, months = NULL) {
  if (is.null(order)) {
    if (!is.null(months)) {
      stop("months needs an order: a month vocabulary is read at the ordered date door",
           call. = FALSE)
    }
    return(text_door(x, "date", options = options))
  }
  text_door(x, "date_ordered", code = date_order_code(order), options = options, months = months)
}

#' @rdname cast_date
#' @export
cast_datetime <- function(x, order, am = "", pm = "", options = NULL, months = NULL) {
  code <- date_order_code(order)
  am_bytes <- symbol_bytes(am, "Day period am", 16L, edges = TRUE)
  pm_bytes <- symbol_bytes(pm, "Day period pm", 16L, edges = TRUE)
  periods <- if (length(am_bytes) || length(pm_bytes)) {
    c(u32_le(40L), pack_symbol(am_bytes, 16L), pack_symbol(pm_bytes, 16L), raw(2))
  }
  text_door(x, "datetime", periods = periods, code = code, options = options, months = months)
}

#' @rdname cast_date
#' @export
cast_time <- function(x) text_door(x, "time")

#' @rdname cast_date
#' @export
cast_duration <- function(x) text_door(x, "duration")

#' Numbers a workbook already holds
#'
#' The typed doors read a double, the number a spreadsheet stores for a numeric cell, rather
#' than text. `cast_decimal_from_double()` reads it as the shortest decimal that names the
#' double, the digits Excel and LibreOffice write (`0.1` is one tenth, and `0.1 + 0.2` is
#' `0.30000000000000004`), canonical and exact as [cast_decimal()]'s.
#' `cast_excel_serial_from_double()` reads it as a date serial under a declared date system,
#' to the zone-less wall clock it names (as [cast_datetime()]'s value); `cast_excel_time()`
#' as a serial's time of day; `cast_excel_duration()` as a span of days. Each time is
#' snapped to the fewest fractional-second digits that store as the same double, so Excel's
#' `9999-12-31 23:59:59` reads on the second. `NaN` is `malformed`; what the value cannot be
#' is `out_of_range`, never rounded. With no text to index, every fault's span is empty
#' (`offset` and `length` `0`).
#'
#' @param x A numeric vector. `NA` is missing; `NaN` is a number, and `malformed`.
#' @inheritParams cast_excel_serial
#' @return A [hypercast_verdict], shaped as [cast_decimal()]'s, [cast_datetime()]'s,
#'   [cast_time()]'s and [cast_duration()]'s.
#' @family doors
#' @export
#' @examples
#' cast_decimal_from_double(c(0.1, 0.1 + 0.2, NaN, Inf))
#' cast_excel_serial_from_double(45292.75, "y1900")$value
#' cast_excel_time(0.5)$value
#' cast_excel_duration(1.25)$value
cast_decimal_from_double <- function(x) double_door(x, "decimal_from_double")

#' @rdname cast_decimal_from_double
#' @export
cast_excel_serial_from_double <- function(x, epoch) {
  double_door(x, "excel_serial_from_double", code = epoch_code(epoch))
}

#' @rdname cast_decimal_from_double
#' @export
cast_excel_time <- function(x) double_door(x, "excel_time")

#' @rdname cast_decimal_from_double
#' @export
cast_excel_duration <- function(x) double_door(x, "excel_duration")
