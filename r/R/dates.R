# Opt-in date shapes: the options and the month vocabulary the date doors take.

#' Date options
#'
#' The date doors keep today's shape rules unless the caller opts in, and `date_options()`
#' is how: the option `flags` (the bits of `date_flags`), and a `pivot` for two-digit years.
#' It declares nothing by default. Strict RFC 3339 is the default, so a caller can rely on a
#' space separator or `+00` being refused.
#'
#' The flags, as bits: `compact_date` (`1`, `20260107`, eight ASCII digits, at
#' [cast_date()] without `order`), `space_separator` (`2`, one space in place of the `T` at
#' [cast_timestamp()]), `hour_offset` (`4`, `+00` and `-05`, offsets of hours alone, at
#' [cast_timestamp()]) and `common_forms` (`7`, all three together; it never supplies a pivot
#' or a month vocabulary). A flag a door has no use for is ignored there, so one
#' [date_options()] value works at every date door. A bit that is none of these is an error.
#'
#' The `pivot` is the last year of a 100-year window: a two-digit year reads as the one year
#' in `[pivot - 99, pivot]` that ends in those two digits, so `pivot = 2029` reads `26` as
#' 2026 and `30` as 1930. It is read at [cast_date()] with `order` and at [cast_datetime()]
#' (four-digit years are unaffected), runs from 100 to 9999, and `0` declares none: there is
#' no default window, and without one a two-digit year is `malformed`.
#'
#' @param flags The options: a character vector of `date_flags` names, or one whole number
#'   of its bits.
#' @param pivot `0` for none, else a year from 100 to 9999.
#' @param x A `hypercast_date_options`.
#' @param ... Ignored.
#' @format `date_flags` is a named integer vector.
#' @return A `hypercast_date_options`: a list of the option bits `flags`, the `pivot` and
#'   `packed`, the 12 bytes the options cross the C ABI as (the size, the option bits and the
#'   pivot as little-endian 32-bit words).
#' @family dates
#' @export
#' @examples
#' date_flags
#' cast_timestamp(c("2026-01-07 15:04:05Z", "2026-01-07T15:04:05+00"))$ok
#' cast_timestamp(c("2026-01-07 15:04:05Z", "2026-01-07T15:04:05+00"),
#'                date_options("common_forms"))$ok
#' cast_date("1/7/26", "month_day_year", date_options(pivot = 2029))$value
#' cast_date("20260107", options = date_options("compact_date"))$value
date_options <- function(flags = character(), pivot = 0L) {
  bits <- date_flag_bits(flags)
  year <- pivot_year(pivot)
  structure(
    list(flags = bits, pivot = year, packed = c(u32_le(12L), u32_le(bits), u32_le(year))),
    class = "hypercast_date_options"
  )
}

#' @rdname date_options
#' @export
date_flags <- c(compact_date = 1L, space_separator = 2L, hour_offset = 4L, common_forms = 7L)

date_flags_message <- function(detail) {
  sprintf("flags must hold only the date options in date_flags; %s", detail)
}

date_flag_bits <- function(flags) {
  if (is.character(flags) && !anyNA(flags)) {
    unknown <- setdiff(flags, names(date_flags))
    if (length(unknown)) {
      stop(date_flags_message(sprintf("%s names none",
                                      paste0("\"", unknown, "\"", collapse = ", "))),
           call. = FALSE)
    }
    return(Reduce(bitwOr, date_flags[flags], 0L))
  }
  if (!is.numeric(flags) || length(flags) != 1L || is.na(flags) || flags < 0 ||
        flags != floor(flags) || flags > 4294967295) {
    stop("flags must be one non-negative whole number or a character vector of option names",
         call. = FALSE)
  }
  if (flags > date_flags[["common_forms"]]) {
    stop(date_flags_message(sprintf("got %.0f", flags)), call. = FALSE)
  }
  as.integer(flags)
}

pivot_year <- function(pivot) {
  if (!is.numeric(pivot) || length(pivot) != 1L || is.na(pivot) || pivot != floor(pivot)) {
    stop("pivot must be one whole number", call. = FALSE)
  }
  if (pivot != 0 && (pivot < 100 || pivot > 9999)) {
    stop(sprintf("pivot must be 0 (none) or a year from 100 to 9999; got %.0f", pivot),
         call. = FALSE)
  }
  as.integer(pivot)
}

#' @rdname date_options
#' @export
print.hypercast_date_options <- function(x, ...) {
  bits <- x$flags
  named <- if (bits == date_flags[["common_forms"]]) {
    "common_forms"
  } else {
    names(date_flags)[1:3][bitwAnd(bits, date_flags[1:3]) != 0L]
  }
  flags <- if (length(named) == 1L) {
    encodeString(named, quote = "\"")
  } else if (length(named)) {
    paste0("c(", paste(encodeString(named, quote = "\""), collapse = ", "), ")")
  }
  args <- c(flags, if (x$pivot != 0L) paste0("pivot = ", x$pivot, "L"))
  cat("date_options(", paste(args, collapse = ", "), ")\n", sep = "")
  invisible(x)
}

check_date_options <- function(options) {
  if (is.null(options)) {
    return(NULL)
  }
  if (!inherits(options, "hypercast_date_options") || !is.raw(options$packed) ||
        length(options$packed) != 12L) {
    stop("options must be NULL or a hypercast_date_options, as date_options() returns",
         call. = FALSE)
  }
  options$packed
}

#' Month names
#'
#' The month vocabulary a date door reads a month field with, declared by the caller (no
#' platform builds one: aliases, abbreviations and grammatical forms are the caller's
#' decisions). `entries` is a named whole-number vector: each name a spelling, each value its
#' month, 1 to 12, so `c(Jan = 1, Feb = 2, Sept = 9, Sep = 9)` and so on. Several spellings
#' may share a month (`Sep` and `Sept`), and every month needs at least one.
#'
#' A declared name replaces the numeric month field, in the position the declared `order`
#' selects, between the same repeated `/`, `-` or `.` separators: `07-Jan-26` reads (with a
#' pivot in [date_options()]) and so does `7-Jan-2026`; prose layouts, optional commas and
#' reordering are not read, and a vocabulary never overrides `order`. Matching folds case
#' (Unicode simple case folding, as the text `enum` does), and the longest entry followed by
#' the required separator wins. Punctuation is part of the name and never stripped: with
#' `janv.` declared and `.` the separator, `7.janv..2026` reads and `7.janv.2026` does not.
#'
#' An entry is 1 to 64 UTF-8 bytes holding no ASCII digit, `/`, `-` or white space (the 25
#' code points with the Unicode `White_Space` property); `.` is allowed anywhere. Two entries
#' that fold to the same spelling are refused, even for one month. A vocabulary that cannot
#' be is an error when it is declared, never when it is read.
#'
#' @param entries A named whole-number vector: spellings as names, months as values.
#' @param x A `hypercast_month_names`.
#' @param ... Ignored.
#' @return A `hypercast_month_names`: a list of the `entries` and `packed`, the bytes the
#'   vocabulary crosses the C ABI as (each entry a month byte, a little-endian 32-bit byte
#'   length, then its UTF-8 bytes).
#' @family dates
#' @export
#' @examples
#' months <- month_names(c("janv." = 1, "fevr." = 2, mars = 3, "avr." = 4, mai = 5, juin = 6,
#'                         "juil." = 7, aout = 8, "sept." = 9, "oct." = 10, "nov." = 11,
#'                         "dec." = 12))
#' cast_date(c("7.janv..2026", "7.janv.2026"), "day_month_year", months = months)
month_names <- function(entries) {
  if (length(entries) == 0L) {
    months <- integer()
    spellings <- character()
  } else {
    if (!is.numeric(entries) || is.object(entries) || anyNA(entries) ||
          is.null(names(entries)) || anyNA(names(entries))) {
      stop(paste("entries must be a named whole-number vector with no NA: the spelling as the",
                 "name, its month as the value"), call. = FALSE)
    }
    months <- entries
    spellings <- names(entries)
  }
  bytes <- lapply(spellings, one_string, what = "Month name")
  validate_month_entries(months, spellings, bytes)
  packed <- unlist(lapply(seq_along(months), function(i) {
    c(as.raw(months[[i]]), u32_le(length(bytes[[i]])), bytes[[i]])
  }), use.names = FALSE)
  declare_month_names(entries = months, spellings = spellings, packed = packed)
}

declare_month_names <- function(entries, spellings, packed) {
  if (length(packed) && .Call(C_month_names_check, packed) != 0L) {
    # Every rule R can state was judged; what only the core's case folding decides is left.
    stop(month_message("duplicate"), call. = FALSE)
  }
  structure(list(entries = stats::setNames(as.integer(entries), spellings), packed = packed),
            class = "hypercast_month_names")
}

month_message <- function(rule, month = NA_integer_) {
  switch(rule,
    layout = paste("Month names layout is cut short or has trailing bytes: each entry is a month",
                   "byte, a 32-bit length, then that many bytes"),
    utf8 = "Month name must be valid UTF-8",
    month = sprintf("Month must be 1 to 12; got %s", month),
    month_missing = sprintf("Month names have no name for month %d", month),
    name_length = "Month name must be 1 to 64 bytes long",
    character = paste("Month name must not hold an ASCII digit, \"/\", \"-\" or white space"),
    duplicate = "Month names refused by the core (two entries may fold to the same spelling)"
  )
}

# The rules R states itself, in the core's order: each entry's month, length and characters,
# then that every month has a name. A folded duplicate is the core's to judge.
validate_month_entries <- function(months, spellings, bytes) {
  for (i in seq_along(months)) {
    month <- months[[i]]
    if (month != floor(month) || month < 1 || month > 12) {
      stop(month_message("month", format(month)), call. = FALSE)
    }
    if (length(bytes[[i]]) < 1L || length(bytes[[i]]) > 64L) {
      stop(month_message("name_length"), call. = FALSE)
    }
    points <- code_points_of(bytes[[i]])
    if (any(bytes[[i]] %in% c(ascii_digit, charToRaw("/-"))) ||
          any(points %in% white_space_code_points)) {
      stop(month_message("character"), call. = FALSE)
    }
  }
  missing <- setdiff(1:12, months)
  if (length(missing)) {
    stop(month_message("month_missing", missing[[1L]]), call. = FALSE)
  }
}

# Month names from the bytes of the packed layout, for a caller that holds them already (and
# the corpus's raw rows): decoded and judged by every rule month_names() judges.
month_names_packed <- function(packed) {
  months <- integer()
  spellings <- character()
  bytes <- list()
  at <- 1L
  while (at <= length(packed)) {
    if (at + 4L > length(packed)) {
      stop(month_message("layout"), call. = FALSE)
    }
    size <- sum(as.numeric(packed[(at + 1L):(at + 4L)]) * 256^(0:3))
    if (at + 4 + size > length(packed)) {
      stop(month_message("layout"), call. = FALSE)
    }
    name <- packed[seq_len(size) + at + 4L]
    if (!validUTF8(rawToChar(name))) {
      stop(month_message("utf8"), call. = FALSE)
    }
    months <- c(months, as.integer(packed[[at]]))
    spellings <- c(spellings, rawToChar(name))
    bytes <- c(bytes, list(name))
    at <- at + 5L + as.integer(size)
  }
  validate_month_entries(months, spellings, bytes)
  declare_month_names(months, spellings, packed)
}

#' @rdname month_names
#' @export
print.hypercast_month_names <- function(x, ...) {
  cat(sprintf("<hypercast_month_names> %d entries\n", length(x$entries)))
  print(x$entries, ...)
  invisible(x)
}

check_months <- function(months) {
  if (is.null(months)) {
    return(NULL)
  }
  if (!inherits(months, "hypercast_month_names") || !is.raw(months$packed)) {
    stop("months must be NULL or a hypercast_month_names, as month_names() returns",
         call. = FALSE)
  }
  months$packed
}
