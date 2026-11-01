reasons <- c("empty", "malformed", "out_of_range")

# The C shim's columns as a verdict: the reason codes as a factor, the value (and its exact
# parts) in R's classes for the door's kind, and the data frame's class and row names.
new_verdict <- function(cols, door) {
  n <- length(cols$ok)
  cols$reason <- structure(cols$reason, levels = reasons, class = "factor")
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
#'
#' A missing row is an `NA` element of the input, which no door reads: `NA` in, `NA` out.
#' [optional()] turns `empty` faults into missing rows too.
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
#' @param x A `hypercast_verdict`, as a door returns.
#' @param ... Passed on to the data frame's print method.
#' @return `is_ok()` returns a logical vector. `faults()` returns a data frame with columns
#'   `row`, `reason`, `offset` and `length`, one row per faulted element. `print()` returns
#'   `x` invisibly.
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
  check_verdict(x)
  x$ok
}

#' @rdname hypercast_verdict
#' @export
faults <- function(x) {
  check_verdict(x)
  rows <- which(x$ok %in% FALSE)
  data.frame(row = rows, reason = x$reason[rows], offset = x$offset[rows],
             length = x$length[rows])
}

#' Empty as absent
#'
#' Presents a verdict optionally: every `empty` fault (input that was empty or only ASCII
#' whitespace) becomes a missing row, as an `NA` element is, and every other row is left as
#' it is. The same presentation every HyperCast binding ships (`optional`, `Optional`), for
#' data where an absent value is not a failure.
#'
#' @param x A `hypercast_verdict`, as a door returns.
#' @return `x`, with `ok`, `reason`, `offset` and `length` set to `NA` in each `empty` row.
#' @family verdicts
#' @export
#' @examples
#' v <- optional(cast_f64(c("1.5", "", "  ", "x"), num_format()))
#' v
#' v$value
optional <- function(x) {
  check_verdict(x)
  empty <- which(x$reason == "empty")
  x$ok[empty] <- NA
  x$reason[empty] <- NA
  x$offset[empty] <- NA_integer_
  x$length[empty] <- NA_integer_
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

check_verdict <- function(x) {
  if (!inherits(x, "hypercast_verdict") || !all(c("ok", "reason") %in% names(x))) {
    stop("x must be a hypercast_verdict, as a door returns", call. = FALSE)
  }
}
