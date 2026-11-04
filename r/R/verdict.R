reasons <- c("empty", "malformed", "out_of_range")

#' Fault details
#'
#' What exactly failed, beside a fault's broad reason: the core's shared vocabulary of
#' details, by name and by its numeric code. A verdict's `detail` column is a factor over
#' these names (and `"unknown"`), and its `detail_code` column holds the core's number.
#'
#' The vocabulary is a contract of its own. Published codes keep their numbers and meanings,
#' new codes may appear in a minor release, and a code this version does not name reads as
#' `"unknown"` with its number kept in `detail_code`: code that branches on a detail needs a
#' default arm (`switch()`). The reason and the span of such a fault are as usable as any
#' other's. A detail says what failed, never how to retry. The same failure at two doors
#' shares one code, though its reason may differ (`Inf` is `out_of_range` at a float door and
#' `malformed` at an integer door, and `not_finite` at both).
#'
#' A detail is `NA` where there is no fault, and for a violation of a declared rule (which
#' names its `keyword` instead).
#'
#' @format A named integer vector of the 32 codes the core publishes.
#' @family verdicts
#' @export
#' @examples
#' fault_details
#' v <- cast_i32(c("12,34", "12x", "1e99"), num_format())
#' v$detail
#' v$detail_code
fault_details <- c(
  blank = 1L, unexpected_character = 2L, digit_expected = 3L, delimiter_expected = 4L,
  trailing_text = 5L, no_digits = 6L, sign_misplaced = 7L, parentheses_unbalanced = 8L,
  mark_repeated = 9L, group_misplaced = 10L, group_size = 11L, group_in_fraction = 12L,
  ambiguous_separator = 13L, fraction_not_allowed = 14L, field_width = 15L, wrong_length = 16L,
  component_missing = 17L, not_recognized = 18L, unknown_unit = 19L, invalid_utf8 = 20L,
  too_long = 21L, not_finite = 22L, not_whole = 23L, negative_not_allowed = 24L, too_large = 25L,
  too_small = 26L, too_precise = 27L, date_field_out_of_range = 28L,
  time_field_out_of_range = 29L, outside_window = 30L, nonexistent_date = 31L,
  not_a_scalar = 32L
)

detail_levels <- c(names(fault_details), "unknown")

# The core's detail codes as the factor a verdict carries: a published code as its name, a
# code this version does not name as "unknown", and a missing one as NA. Never an error: a
# newer core's vocabulary is still a fault with a usable reason and span.
detail_factor <- function(code) {
  index <- match(code, fault_details)
  index[!is.na(code) & is.na(index)] <- length(detail_levels)
  structure(index, levels = detail_levels, class = "factor")
}

# The detail column and the raw code beside it, appended after every 1.0 column.
add_detail <- function(cols) {
  cols$detail_code <- cols$detail
  cols$detail <- detail_factor(cols$detail)
  cols
}

# The C shim's columns as a verdict: the reason codes as a factor, the value (and its exact
# parts) in R's classes for the door's kind, and the data frame's class and row names.
new_verdict <- function(cols, door) {
  n <- length(cols$ok)
  cols$reason <- structure(cols$reason, levels = reasons, class = "factor")
  cols <- add_detail(cols)
  switch(door,
    timestamp = , unix = , excel_serial = {
      cols$value <- .POSIXct(cols$value, tz = "UTC")
    },
    date = , date_ordered = {
      cols$value <- structure(cols$value, class = "Date")
    },
    datetime = , excel_serial_from_double = {
      cols$date <- structure(cols$date, class = "Date")
    },
    time = , excel_time = , duration = , excel_duration = {
      cols$value <- structure(cols$value, units = "secs", class = "difftime")
    }
  )
  door <- if (door == "date_ordered") "date" else door
  structure(cols,
            class = c("hypercast_verdict", "data.frame"),
            row.names = if (n > 0L) c(NA_integer_, -n) else integer(),
            door = paste0("cast_", door))
}

#' Verdicts
#'
#' Every door returns a `hypercast_verdict`: a data frame with one row per element of its
#' input, in order, holding that element's verdict. The verdict is HyperCast's
#' discriminated union, the value or a fault, laid out the way R holds a union over a vector:
#' one column per field, with `ok` as the discriminant and `NA` in the fields of the other
#' case.
#'
#' | Column | Success (`ok` is `TRUE`) | Fault (`ok` is `FALSE`) | Missing (`ok` is `NA`) |
#' | --- | --- | --- | --- |
#' | `value` | the value, in the door's R type | `NA` | `NA` |
#' | `ok` | `TRUE` | `FALSE` | `NA` |
#' | `reason` | `NA` | a factor: `empty`, `malformed` or `out_of_range` | `NA` |
#' | `offset` | `NA` | where the offending text starts, counted from 0 | `NA` |
#' | `length` | `NA` | how long it runs | `NA` |
#' | `detail` | `NA` | a factor: what exactly failed, one of [fault_details] or `unknown` | `NA` |
#' | `detail_code` | `NA` | the core's number for it | `NA` |
#'
#' A missing row is an `NA` element of the input, which no door reads: `NA` in, `NA` out.
#' [optional()] turns `empty` faults into missing rows too.
#'
#' `detail` and `detail_code` come last, after the five columns every earlier release had in
#' this order, so code that reads a verdict by name or by position keeps working; binding a
#' verdict made before this column existed to one made after is a binding of different
#' columns. The two together are what [fault_details] describes, and a `detail` the version
#' does not know reads `unknown` with its code in `detail_code`.
#'
#' `offset` and `length` count in the units `substr()` counts the input in, so `substr(x,
#' offset + 1, offset + length)` gives back the offending text. That is characters for a
#' character vector, and bytes for a list of raw vectors, for a string marked `"bytes"` and
#' for an unmarked string in a session whose locale is ASCII (the C locale), which R has no
#' encoding to translate from and whose bytes are read as UTF-8 as they are. The core
#' reports a span in UTF-8 bytes; for text counted in characters it is re-expressed. A
#' typed door, which reads a number rather than text, faults over an empty span (`0`, `0`).
#'
#' Where the value column cannot carry everything the core returned, the exact parts follow
#' it as columns of their own: `seconds` and `nanos` for an instant and a duration, `nanos`
#' for a time of day, `date` and `nanos_of_day` for a civil date-time. See the doors for each
#' value's type.
#'
#' A verdict is a data frame, so it subsets, binds and converts as one; [as.data.frame()]
#' drops the class. `is_ok()` is the `ok` column, and `faults()` the faulted rows, with
#' `row` naming each one's element.
#'
#' @param x A `hypercast_verdict`, as a door returns (`is_ok()` and `faults()` take a
#'   [hypercast_checked_verdict] too).
#' @param ... Passed on to the data frame's print method.
#' @return `is_ok()` returns a logical vector. `faults()` returns a data frame with columns
#'   `row`, `reason`, `offset`, `length`, `detail` and `detail_code`, one row per faulted
#'   element. `print()` returns `x` invisibly.
#' @name hypercast_verdict
#' @family verdicts
#' @examples
#' v <- cast_i32(c("42", "(1,234)", "12x", "", NA), num_format())
#' v
#' v$value
#' is_ok(v)
#' faults(v)
#' x <- c("42", "(1,234)", "12x")
#' with(faults(cast_i32(x, num_format())), substr(x[row], offset + 1, offset + length))
NULL

#' @rdname hypercast_verdict
#' @export
is_ok <- function(x) {
  check_verdict(x, checked = TRUE)
  x$ok
}

#' @rdname hypercast_verdict
#' @export
faults <- function(x) UseMethod("faults")

#' @export
faults.default <- function(x) {
  check_verdict(x, checked = TRUE)
}

#' @export
faults.hypercast_verdict <- function(x) {
  check_verdict(x)
  fault_rows(x, which(x$ok %in% FALSE))
}

fault_rows <- function(x, rows) {
  out <- data.frame(row = rows, reason = x$reason[rows], offset = x$offset[rows],
                    length = x$length[rows])
  # A verdict from before the detail column existed has none to list.
  if (all(c("detail", "detail_code") %in% names(x))) {
    out$detail <- x$detail[rows]
    out$detail_code <- x$detail_code[rows]
  }
  out
}

#' Empty as absent
#'
#' Presents a verdict optionally: every `empty` fault (input that was empty or only white
#' space, the 25 Unicode `White_Space` code points) becomes a
#' missing row, as an `NA` element is, and every other row, a violation included, is left as
#' it is. The same presentation every HyperCast binding ships (`optional`, `Optional`), for
#' data where an absent value is not a failure.
#'
#' @param x A `hypercast_verdict` or a `hypercast_checked_verdict`, as a door returns.
#' @return `x`, with `ok`, `reason`, `offset`, `length` and `detail` set to `NA` in each `empty`
#'   row.
#' @family verdicts
#' @export
#' @examples
#' v <- optional(cast_f64(c("1.5", "", "  ", "x"), num_format()))
#' v
#' v$value
optional <- function(x) UseMethod("optional")

#' @export
optional.default <- function(x) {
  check_verdict(x, checked = TRUE)
}

#' @export
optional.hypercast_verdict <- function(x) {
  check_verdict(x)
  empty_as_missing(x)
}

empty_as_missing <- function(x) {
  empty <- which(x$reason == "empty")
  x$ok[empty] <- NA
  x$reason[empty] <- NA
  x$offset[empty] <- NA_integer_
  x$length[empty] <- NA_integer_
  if ("detail" %in% names(x)) {
    x$detail[empty] <- NA
    x$detail_code[empty] <- NA_integer_
  }
  x
}

#' @rdname hypercast_verdict
#' @export
print.hypercast_verdict <- function(x, ...) {
  if (all(c("ok", "reason") %in% names(x))) {
    door <- attr(x, "door")
    cat(sprintf("<hypercast_verdict%s> %d ok, %d faulted, %d missing\n",
                if (is.null(door)) "" else paste0(": ", door),
                sum(x$ok %in% TRUE), sum(x$ok %in% FALSE), sum(is.na(x$ok))))
  }
  print(as.data.frame(x), ...)
  invisible(x)
}

check_verdict <- function(x, checked = FALSE) {
  if (checked && inherits(x, "hypercast_checked_verdict")) {
    return(check_checked_verdict(x))
  }
  if (!inherits(x, "hypercast_verdict") || !all(c("ok", "reason") %in% names(x))) {
    stop(if (checked) {
      "x must be a hypercast_verdict or a hypercast_checked_verdict, as a door returns"
    } else {
      "x must be a hypercast_verdict, as a door returns"
    }, call. = FALSE)
  }
}

# The C shim's checked columns as a verdict: the reason codes as a factor, the keyword codes
# as their names (open: an unknown code reads "keyword N"), and the data frame's class.
new_checked_verdict <- function(cols, door) {
  n <- length(cols$ok)
  cols$reason <- structure(cols$reason, levels = reasons, class = "factor")
  cols <- add_detail(cols)
  cols$keyword <- keyword_name(cols$keyword)
  structure(cols,
            class = c("hypercast_checked_verdict", "data.frame"),
            row.names = if (n > 0L) c(NA_integer_, -n) else integer(),
            door = door)
}

#' Checked verdicts
#'
#' Every checked door ([cast_checked()], [cast_checked_from_double()], and [cast_text()]
#' with constraints) returns a `hypercast_checked_verdict`: a data frame with one row per
#' element of its input, in order. A checked verdict has two outcomes besides missing: the
#' value passed, or it was rejected, and a rejection is either a fault (it did not parse:
#' the same reasons a [hypercast_verdict] has) or a violation (it parsed and broke a declared
#' rule, named by its keyword). A violation is never a fifth reason: `reason` stays `NA` and
#' `keyword` names the rule.
#'
#' | Column | Passed (`ok` `TRUE`) | Fault (`ok` `FALSE`) | Violation (`ok` `FALSE`) | Missing |
#' | --- | --- | --- | --- | --- |
#' | `value` | the value, in the door's R type | `NA` | `NA` | `NA` |
#' | `adjusted` | `TRUE` when rounding changed the value | `NA` | `NA` | `NA` |
#' | `enum_index` | the matched text `enum` entry, from 1; else `NA` | `NA` | `NA` | `NA` |
#' | `ok` | `TRUE` | `FALSE` | `FALSE` | `NA` |
#' | `reason` | `NA` | `empty`, `malformed` or `out_of_range` | `NA` | `NA` |
#' | `keyword` | `NA` | `NA` | the rule broken | `NA` |
#' | `offset`, `length` | `NA` | the offending text | the value | `NA` |
#' | `detail`, `detail_code` | `NA` | what failed, as a [hypercast_verdict]'s | `NA` | `NA` |
#'
#' `keyword` is character, OpenAPI's spelling: `"minimum"`, `"exclusiveMinimum"`,
#' `"maximum"`, `"exclusiveMaximum"`, `"multipleOf"`, `"minLength"`, `"maxLength"` or
#' `"enum"`. The list is open, since `format` and `pattern` may join it in a minor release,
#' and a code this version does not know reads as `"keyword N"`: code that branches on it
#' (`switch()`) needs a default. Spans count in the same units as a [hypercast_verdict]'s;
#' a violation spans the whole value (after trimming, for text), and a typed door's spans are
#' empty. An integer `enum` gives no index: the value is the answer.
#'
#' `is_ok()` is the `ok` column, `faults()` the faulted rows (`row`, `reason`, `offset`,
#' `length`, `detail`, `detail_code`), `violations()` the violated rows (`row`, `keyword`,
#' `offset`, `length`), and
#' [optional()] turns `empty` faults into missing rows. A checked verdict is a data frame of
#' its own class, so binding it with a plain verdict is a binding of different columns.
#'
#' @param x A `hypercast_checked_verdict`, as a checked door returns.
#' @param ... Passed on to the data frame's print method.
#' @return `violations()` returns a data frame with columns `row`, `keyword`, `offset` and
#'   `length`, one row per violated element. `print()` returns `x` invisibly.
#' @name hypercast_checked_verdict
#' @family verdicts
#' @family constraints
#' @examples
#' age <- hypercast_constraints("u8", minimum = 18L, maximum = 130L)
#' v <- cast_checked(c("42", "17", "x", "", NA), num_format(), age)
#' v
#' faults(v)
#' violations(v)
#' optional(v)$ok
NULL

#' @rdname hypercast_checked_verdict
#' @export
violations <- function(x) {
  check_checked_verdict(x)
  rows <- which(!is.na(x$keyword))
  data.frame(row = rows, keyword = x$keyword[rows], offset = x$offset[rows],
             length = x$length[rows])
}

#' @export
faults.hypercast_checked_verdict <- function(x) {
  check_checked_verdict(x)
  fault_rows(x, which(!is.na(x$reason)))
}

#' @export
optional.hypercast_checked_verdict <- function(x) {
  check_checked_verdict(x)
  empty_as_missing(x)
}

#' @rdname hypercast_checked_verdict
#' @export
print.hypercast_checked_verdict <- function(x, ...) {
  if (all(c("ok", "reason", "keyword") %in% names(x))) {
    door <- attr(x, "door")
    cat(sprintf("<hypercast_checked_verdict%s> %d ok, %d faulted, %d violated, %d missing\n",
                if (is.null(door)) "" else paste0(": ", door), sum(x$ok %in% TRUE),
                sum(!is.na(x$reason)), sum(!is.na(x$keyword)), sum(is.na(x$ok))))
  }
  print(as.data.frame(x), ...)
  invisible(x)
}

check_checked_verdict <- function(x) {
  if (!inherits(x, "hypercast_checked_verdict") ||
        !all(c("ok", "reason", "keyword") %in% names(x))) {
    stop("x must be a hypercast_checked_verdict, as a checked door returns", call. = FALSE)
  }
}
