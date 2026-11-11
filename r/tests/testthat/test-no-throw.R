# No door signals an R condition for anything in its data: every element's outcome is a row
# of the verdict. This drives every door over hostile data (every corpus input truncated at
# every byte, invalid UTF-8, strings in every encoding R marks, controls, every special
# double) and holds each call to raising nothing at all, not even a warning, and to returning
# one well-formed row per element. It also holds every fault span inside its input, in the
# input's own units, as rust/tests/fault_span_invariant.rs holds the core's.

testthat::skip_if_not_installed("jsonlite")

corpus_inputs <- function() {
  dir <- testthat::test_path("corpus")
  if (!dir.exists(dir)) dir <- file.path(testthat::test_path(), "..", "..", "..", "corpus")
  files <- list.files(dir, pattern = "\\.json$", full.names = TRUE)
  stopifnot(length(files) == 19L)
  inputs <- unlist(lapply(files, function(path) {
    json <- paste(readLines(path, encoding = "UTF-8", warn = FALSE), collapse = "\n")
    json <- gsub("\\u0000", "\\uE000", json, fixed = TRUE)
    # A refused declaration, a decoded number or raw bytes has no text input.
    lapply(jsonlite::fromJSON(json, simplifyVector = FALSE), function(row) row$input)
  }))
  unique(inputs)
}

# Every prefix of every input's UTF-8 bytes, NUL stand-ins restored: the truncations that
# split a multi-byte character included, so the raw replay sees invalid UTF-8 too.
truncations <- function(inputs) {
  standin <- charToRaw("\ue000")
  out <- list()
  for (text in inputs) {
    bytes <- charToRaw(enc2utf8(text))
    hit <- which(bytes == standin[[1]])
    for (h in rev(hit)) {
      if (h + 2L <= length(bytes) && identical(bytes[h:(h + 2L)], standin)) {
        bytes <- c(bytes[seq_len(h - 1L)], as.raw(0L), bytes[-seq_len(h + 2L)])
      }
    }
    for (n in 0:length(bytes)) out[[length(out) + 1L]] <- bytes[seq_len(n)]
  }
  out
}

hostile_strings <- function() {
  latin1 <- "caf\xe9 1,5 \xe9t\xe9"
  Encoding(latin1) <- "latin1"
  bytes <- "12\xff\xfe34"
  Encoding(bytes) <- "bytes"
  invalid <- rawToChar(as.raw(c(0x31, 0xc3, 0x28, 0x32, 0xe2, 0x82)))
  Encoding(invalid) <- "UTF-8"
  c(latin1, bytes, invalid, rawToChar(as.raw(c(0xf0, 0x9f, 0x98))), "\u00a0 42 \u00a0",
    strrep("9", 100000), strrep(" ", 1000), "\x01\x02\x7f", "U+110000", "&#xD800;",
    "\u202f1\u202f234,5", "\u2212 5", "1e999999999999", NA)
}

english_months <- month_names(c(Jan = 1, Feb = 2, Mar = 3, Apr = 4, May = 5, Jun = 6, Jul = 7,
                                Aug = 8, Sep = 9, Sept = 9, Oct = 10, Nov = 11, Dec = 12))

text_doors <- function() {
  f <- num_format()
  list(
    bool = cast_bool, char = cast_char, uuid = cast_uuid, timestamp = cast_timestamp,
    date = cast_date, time = cast_time, duration = cast_duration,
    i8 = function(x) cast_i8(x, f), i16 = function(x) cast_i16(x, f),
    i32 = function(x) cast_i32(x, f), i64 = function(x) cast_i64(x, f),
    u8 = function(x) cast_u8(x, f), u16 = function(x) cast_u16(x, f),
    u32 = function(x) cast_u32(x, f), u64 = function(x) cast_u64(x, f),
    f32 = function(x) cast_f32(x, f), f64 = function(x) cast_f64(x, f),
    decimal = function(x) cast_decimal(x, f),
    decimal_detect = function(x) cast_decimal(x, num_format_detect()),
    f64_eurozone = function(x) {
      cast_f64(x, num_format(",", "\u00a0", currency = "\u20ac", minus_sign = "\u2212",
                             percent = "\u066a", digit_zero = "\u0660"))
    },
    unix_s = function(x) cast_unix(x, "seconds"), unix_ns = function(x) cast_unix(x, "nanoseconds"),
    excel_1900 = function(x) cast_excel_serial(x, "y1900"),
    excel_1904 = function(x) cast_excel_serial(x, "y1904"),
    date_mdy = function(x) cast_date(x, "month_day_year"),
    datetime_dmy = function(x) cast_datetime(x, "day_month_year"),
    datetime_periods = function(x) {
      cast_datetime(x, "year_month_day", am = "\uc624\uc804", pm = "a. m.")
    },
    decimal_indian = function(x) cast_decimal(x, num_format(grouping = "indian")),
    decimal_indian = function(x) cast_decimal(x, num_format(grouping = "indian")),
    timestamp_common = function(x) cast_timestamp(x, date_options("common_forms")),
    date_compact = function(x) cast_date(x, options = date_options("common_forms")),
    date_months = function(x) {
      cast_date(x, "day_month_year", date_options("common_forms", 2029), english_months)
    },
    datetime_months = function(x) {
      cast_datetime(x, "month_day_year", options = date_options(pivot = 100),
                    months = english_months)
    },
    text = cast_text,
    text_raw = function(x) cast_text(x, raw = TRUE),
    checked_text_raw = function(x) {
      cast_text(x, hypercast_constraints("text", max_length = 4, trim = TRUE), raw = TRUE)
    },
    checked_text = function(x) {
      cast_text(x, hypercast_constraints("text", min_length = 1, max_length = 4, trim = TRUE,
                                         enum = c("Male", "\u00e9t\u00e9", "\u0131")))
    },
    checked_i64 = function(x) {
      cast_checked(x, f, hypercast_constraints("i64", minimum = "-100", multiple_of = "5",
                                               rounding = "ceiling", enum = c("0", "5")))
    },
    checked_f64 = function(x) {
      cast_checked(x, f, hypercast_constraints("f64", exclusive_maximum = "1e10",
                                               multiple_of = "0.01", rounding = "midpoint"))
    },
    checked_decimal = function(x) {
      cast_checked(x, num_format_detect(),
                   hypercast_constraints("decimal", minimum = "0", multiple_of = "0.05"))
    }
  )
}

typed_doors <- list(
  decimal = cast_decimal_from_double,
  checked_u16 = function(x) {
    cast_checked_from_double(x, hypercast_constraints("u16", maximum = 1000L, multiple_of = 7L,
                                                      rounding = "floor"))
  },
  checked_f32 = function(x) {
    cast_checked_from_double(x, hypercast_constraints("f32", minimum = "-1", multiple_of = "0.5",
                                                      rounding = "midpoint"))
  },
  excel_1900 = function(x) cast_excel_serial_from_double(x, "y1900"),
  excel_1904 = function(x) cast_excel_serial_from_double(x, "y1904"),
  time = cast_excel_time, duration = cast_excel_duration
)

# Calls door(x), failing on any condition it signals, and checks the verdict's shape.
expect_quiet_verdict <- function(door, x, label) {
  signalled <- NULL
  v <- withCallingHandlers(
    tryCatch(door(x), error = function(e) {
      signalled <<- e
      NULL
    }),
    warning = function(w) {
      signalled <<- w
      invokeRestart("muffleWarning")
    },
    message = function(m) {
      signalled <<- m
      invokeRestart("muffleMessage")
    }
  )
  testthat::expect_null(signalled, label = paste(label, "signalled"))
  if (is.null(v)) {
    return(NULL)
  }
  checked <- inherits(v, "hypercast_checked_verdict")
  if (!checked) testthat::expect_s3_class(v, "hypercast_verdict")
  testthat::expect_identical(nrow(v), length(x), label = label)
  ok <- v$ok
  # The union's discriminant agrees with its fields in every row: a rejection is a fault
  # (a reason) or a violation (a keyword), never both and never neither.
  testthat::expect_true(all(is.na(v$reason[ok %in% TRUE])), label = label)
  rejected <- if (checked) xor(!is.na(v$reason), !is.na(v$keyword)) else !is.na(v$reason)
  testthat::expect_true(all(rejected[ok %in% FALSE]), label = label)
  if (checked) {
    testthat::expect_true(all(is.na(v$keyword[!ok %in% FALSE])), label = label)
    testthat::expect_true(all(!is.na(v$adjusted[ok %in% TRUE])), label = label)
  }
  # raw = TRUE holds its value in a list, where a row with none is NULL rather than NA. A fault
  # and a missing row have none; a violation carries the value as cast.
  violated <- if (checked) !is.na(v$keyword) else rep(FALSE, length(ok))
  absent <- v$value[!ok %in% TRUE & !violated]
  testthat::expect_true(all(if (is.list(absent)) lengths(absent) == 0L else is.na(absent)),
                        label = label)
  carried <- v$value[violated]
  if (is.list(carried)) {
    testthat::expect_false(any(vapply(carried, is.null, FALSE)), label = label)
  } else if (!is.character(carried)) {
    # A character value can be NA where R can't hold the text (U+0000).
    testthat::expect_false(anyNA(carried), label = label)
  }
  testthat::expect_true(all(is.na(v$offset[!ok %in% FALSE])), label = label)
  # A fault names its detail (a published one, here); a violation, a success and a missing
  # row have none.
  faulted <- if (checked) !is.na(v$reason) else ok %in% FALSE
  testthat::expect_true(all(!is.na(v$detail[faulted]) & v$detail[faulted] != "unknown"),
                        label = label)
  testthat::expect_true(all(is.na(v$detail[!faulted]) & is.na(v$detail_code[!faulted])),
                        label = label)
  v
}

test_that("no text door signals anything for any truncation of any corpus input", {
  bytes <- truncations(corpus_inputs())
  expect_gt(length(bytes), 5000)
  for (name in names(text_doors())) {
    v <- expect_quiet_verdict(text_doors()[[name]], bytes, name)
    faulted <- which(v$ok %in% FALSE)
    lengths <- lengths(bytes)[faulted]
    expect_true(all(v$offset[faulted] >= 0L & v$length[faulted] >= 0L), label = name)
    expect_true(all(v$offset[faulted] + v$length[faulted] <= lengths), label = name)
  }
})

test_that("no text door signals anything for corpus text or hostile strings", {
  text <- c(corpus_inputs()[!grepl("\ue000", corpus_inputs(), fixed = TRUE)],
            hostile_strings())
  for (name in names(text_doors())) {
    v <- expect_quiet_verdict(text_doors()[[name]], text, name)
    valid <- which(v$ok %in% FALSE & validUTF8(text) & Encoding(text) != "bytes")
    expect_true(all(v$offset[valid] + v$length[valid] <= nchar(text[valid], "chars")),
                label = name)
  }
})

test_that("no typed door signals anything for any double", {
  set.seed(20261101)
  random <- readBin(as.raw(sample(0:255, 8 * 5000, replace = TRUE)), "double", n = 5000)
  specials <- c(NaN, Inf, -Inf, NA, 0, -0, .Machine$double.xmin, .Machine$double.xmax,
                -.Machine$double.xmax, 5e-324, 2^53, 2^96, 1e300, -1e300, 60, 2958465.99999,
                -2958465, 3652059, 1e-10)
  for (name in names(typed_doors)) {
    v <- expect_quiet_verdict(typed_doors[[name]], c(random, specials), name)
    faulted <- which(v$ok %in% FALSE)
    expect_true(all(v$offset[faulted] == 0L & v$length[faulted] == 0L), label = name)
  }
})
