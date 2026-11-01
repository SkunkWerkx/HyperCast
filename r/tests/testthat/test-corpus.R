# Replays the shared conformance corpus (corpus/*.json at the repository root) through the
# public API, the same vectors the Rust core and every other binding replay. The staged copy
# under tests/testthat/corpus/ is what the tarball ships; a checkout without it walks up to
# the root's corpus/. A missing corpus file is an error, never a skip.
#
# Every text vector is replayed twice: as a list of raw vectors, whose spans are UTF-8 bytes
# and must equal the corpus's pinned spans exactly, and as a character vector, whose spans
# are characters and must equal the pinned spans re-expressed in characters. The corpus pins
# three inputs holding U+0000, which no R string can hold: those replay as raw bytes only.
# The whole file is cast in one call per door and declaration, the way R code calls a door.

# A dependency skip, not a corpus skip: jsonlite is in Suggests, and CRAN's noSuggests
# flavour runs the tests without it. With jsonlite present, a missing corpus still errors.
testthat::skip_if_not_installed("jsonlite")

corpus_files <- c("boolean.json", "char.json", "date.json", "date_order.json", "datetime.json",
                  "decimal.json", "duration.json", "excel_serial.json", "integer.json",
                  "real.json", "time.json", "timestamp.json", "typed.json", "unix.json",
                  "uuid.json")

corpus_dir <- function() {
  staged <- testthat::test_path("corpus")
  if (dir.exists(staged)) {
    return(staged)
  }
  here <- getwd()
  for (i in 1:6) {
    here <- file.path(here, "..")
    candidate <- file.path(here, "corpus")
    if (dir.exists(candidate) && file.exists(file.path(candidate, "boolean.json"))) {
      return(normalizePath(candidate))
    }
  }
  stop("corpus not found at ", staged, " or above ", getwd())
}

# U+0000 cannot reach an R string, so the JSON escape is swapped for U+E000 (a private-use
# code point the corpus never holds, checked here) before parsing, and swapped back to the
# byte 0x00 in the raw replay. Integer values are read as their exact text: a u64 past 2^53
# has no exact double.
nul_standin <- "\uE000"
read_vectors <- function(name) {
  path <- file.path(corpus_dir(), name)
  if (!file.exists(path) || file.size(path) == 0) {
    stop("corpus file missing or empty: ", path)
  }
  json <- paste(readLines(path, encoding = "UTF-8", warn = FALSE), collapse = "\n")
  stopifnot(!grepl(nul_standin, json, fixed = TRUE), !grepl("\\uE000", json, fixed = TRUE),
            !grepl("\\ue000", json, fixed = TRUE))
  json <- gsub("\\u0000", "\\uE000", json, fixed = TRUE)
  if (name == "integer.json") {
    json <- gsub("\"value\": (-?[0-9]+)", "\"value\": \"\\1\"", json)
  }
  rows <- jsonlite::fromJSON(json, simplifyVector = FALSE)
  if (!length(rows)) stop("corpus file has no vectors: ", path)
  rows
}

utf8_bytes <- function(text) {
  bytes <- charToRaw(enc2utf8(text))
  standin <- charToRaw(enc2utf8(nul_standin))
  out <- raw()
  i <- 1L
  while (i <= length(bytes)) {
    if (i + 2L <= length(bytes) && identical(bytes[i:(i + 2L)], standin)) {
      out <- c(out, as.raw(0L))
      i <- i + 3L
    } else {
      out <- c(out, bytes[[i]])
      i <- i + 1L
    }
  }
  out
}

# Characters in bytes[0, n): every one starts with exactly one non-continuation byte.
chars_in <- function(bytes, n) {
  sum(bitwAnd(as.integer(bytes[seq_len(n)]), 0xC0) != 0x80)
}

has_nul <- function(row) grepl(nul_standin, row$input, fixed = TRUE)

format_of <- function(spec) {
  if (is.null(spec)) {
    return(num_format())
  }
  opt <- function(name) if (is.null(spec[[name]])) "" else spec[[name]]
  num_format(spec$decimal_sep, spec$group_sep, spec$flags, currency = opt("currency"),
             currency_decimal_sep = opt("currency_decimal_sep"),
             currency_group_sep = opt("currency_group_sep"), minus_sign = opt("minus_sign"),
             plus_sign = opt("plus_sign"), percent = opt("percent"),
             digit_zero = opt("digit_zero"))
}

orders <- c("year_month_day", "month_day_year", "day_month_year")
precisions <- c("seconds", "milliseconds", "microseconds", "nanoseconds")
epochs <- c("y1900", "y1904")

# Groups rows by a key (a door, a format, an order) so each group is one vectorised call.
groups <- function(rows, key) split(rows, vapply(rows, key, ""))

# Checks verdict row i against corpus row `row`: the reason and pinned span, or `check_ok`
# on a success. `units` is "bytes" or "chars"; `label` names the row in a failure.
expect_row <- function(v, i, row, units, check_ok, label) {
  info <- sprintf("%s [%s] %s", label, units, encodeString(row$input, quote = "\""))
  testthat::expect_false(is.na(v$ok[[i]]), info = info)
  if (row$expect == "ok") {
    testthat::expect_true(v$ok[[i]], info = paste(info, "should parse; got", v$reason[[i]]))
    if (isTRUE(v$ok[[i]])) check_ok(v, i, row, info)
    return(invisible())
  }
  testthat::expect_false(v$ok[[i]], info = paste(info, "unexpectedly parsed"))
  testthat::expect_identical(as.character(v$reason[[i]]), row$expect, info = info)
  if (!is.null(row$fault)) {
    span <- c(row$fault[[1]], row$fault[[2]])
    if (units == "chars") {
      bytes <- utf8_bytes(row$input)
      span <- c(chars_in(bytes, span[[1]]),
                chars_in(bytes, span[[1]] + span[[2]]) - chars_in(bytes, span[[1]]))
    }
    testthat::expect_identical(c(v$offset[[i]], v$length[[i]]), as.integer(span), info = info)
  }
}

# Replays text rows through `door` both ways, under one declaration.
replay_text <- function(rows, door, check_ok, label) {
  bytes <- lapply(rows, function(row) utf8_bytes(row$input))
  v <- door(bytes)
  testthat::expect_s3_class(v, "hypercast_verdict")
  testthat::expect_identical(nrow(v), length(rows))
  for (i in seq_along(rows)) expect_row(v, i, rows[[i]], "bytes", check_ok, label)
  text_rows <- Filter(Negate(has_nul), rows)
  v <- door(vapply(text_rows, function(row) row$input, ""))
  for (i in seq_along(text_rows)) expect_row(v, i, text_rows[[i]], "chars", check_ok, label)
}

expect_value <- function(v, i, want, info) {
  testthat::expect_identical(v$value[[i]], want, info = info)
}

expect_instant <- function(v, i, row, info) {
  testthat::expect_identical(v$seconds[[i]], as.numeric(row$seconds), info = info)
  testthat::expect_identical(v$nanos[[i]], as.integer(row$nanos), info = info)
  testthat::expect_equal(as.numeric(v$value[[i]]), row$seconds + row$nanos / 1e9,
                         tolerance = 1e-12, info = info)
}

expect_date <- function(got, row, info) {
  want <- as.Date(sprintf("%04d-%02d-%02d", row$year, row$month, row$day))
  testthat::expect_identical(got, want, info = info)
}

# The wall clock text a civil date-time comes out as, built from the vector's fields.
civil_text <- function(row) {
  secs <- row$nanos_of_day %/% 1e9
  frac <- row$nanos_of_day %% 1e9
  text <- sprintf("%04d-%02d-%02d %02d:%02d:%02d", row$year, row$month, row$day,
                  secs %/% 3600, secs %/% 60 %% 60, secs %% 60)
  if (frac > 0) text <- paste0(text, ".", sub("0+$", "", sprintf("%09.0f", frac)))
  text
}

expect_civil <- function(v, i, row, info) {
  expect_date(v$date[[i]], row, info)
  testthat::expect_identical(v$nanos_of_day[[i]], as.numeric(row$nanos_of_day), info = info)
  testthat::expect_identical(v$value[[i]], civil_text(row), info = info)
}

test_that("every corpus file is present in the corpus directory the tests read", {
  for (f in corpus_files) {
    expect_true(file.exists(file.path(corpus_dir(), f)), info = f)
  }
  expect_setequal(list.files(corpus_dir(), pattern = "\\.json$"), corpus_files)
})

test_that("boolean.json", {
  replay_text(read_vectors("boolean.json"), cast_bool, function(v, i, row, info) {
    expect_value(v, i, row$value, info)
  }, "bool")
})

# The vector pins the code point; the door returns the one-character string, and U+0000, which
# no R string holds, as out_of_range over the input less its ASCII whitespace edges.
test_that("char.json", {
  rows <- read_vectors("char.json")
  nul <- vapply(rows, function(row) identical(row$value, 0L), FALSE)
  replay_text(rows[!nul], cast_char, function(v, i, row, info) {
    expect_identical(utf8ToInt(v$value[[i]]), row$value, info = info)
  }, "char")
  expect_gt(sum(nul), 0)
  for (row in rows[nul]) {
    bytes <- utf8_bytes(row$input)
    space <- as.raw(c(0x20, 0x09, 0x0a, 0x0c, 0x0d))
    kept <- which(!bytes %in% space)
    start <- min(kept) - 1L
    end <- max(kept)
    row$expect <- "out_of_range"
    row$fault <- list(start, end - start)
    replay_text(list(row), cast_char, NULL, "char U+0000")
  }
})

test_that("integer.json", {
  rows <- read_vectors("integer.json")
  for (group in groups(rows, function(row) paste(row$type, jsonlite::toJSON(row$format)))) {
    type <- group[[1]]$type
    door <- get(paste0("cast_", type))
    format <- format_of(group[[1]]$format)
    replay_text(group, function(x) door(x, format), function(v, i, row, info) {
      got <- v$value[[i]]
      if (type %in% c("i64", "u64")) {
        expect_identical(got, row$value, info = info)
      } else {
        expect_identical(as.numeric(got), as.numeric(row$value), info = info)
        expect_type(got, if (type %in% c("i32", "u32")) "double" else "integer")
      }
    }, type)
  }
})

test_that("real.json", {
  rows <- read_vectors("real.json")
  for (group in groups(rows, function(row) paste(row$type, jsonlite::toJSON(row$format)))) {
    type <- group[[1]]$type
    door <- if (type == "f32") cast_f32 else cast_f64
    format <- format_of(group[[1]]$format)
    replay_text(group, function(x) door(x, format), function(v, i, row, info) {
      want <- as.numeric(row$value)
      if (type == "f32") want <- readBin(writeBin(want, raw(), size = 4L), "double", size = 4L)
      expect_identical(v$value[[i]], want, info = info)
    }, type)
  }
})

test_that("decimal.json", {
  rows <- read_vectors("decimal.json")
  for (group in groups(rows, function(row) as.character(jsonlite::toJSON(row$format)))) {
    format <- format_of(group[[1]]$format)
    replay_text(group, function(x) cast_decimal(x, format), function(v, i, row, info) {
      expect_value(v, i, row$value, info)
    }, "decimal")
  }
})

test_that("uuid.json", {
  replay_text(read_vectors("uuid.json"), cast_uuid, function(v, i, row, info) {
    expect_identical(gsub("-", "", v$value[[i]]), tolower(row$value), info = info)
    expect_match(v$value[[i]], "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
  }, "uuid")
})

test_that("timestamp.json", {
  replay_text(read_vectors("timestamp.json"), cast_timestamp, expect_instant, "timestamp")
})

test_that("unix.json", {
  for (group in groups(read_vectors("unix.json"), function(row) as.character(row$precision))) {
    precision <- precisions[[group[[1]]$precision]]
    replay_text(group, function(x) cast_unix(x, precision), expect_instant, precision)
  }
})

test_that("excel_serial.json", {
  for (group in groups(read_vectors("excel_serial.json"), function(row) as.character(row$epoch))) {
    epoch <- epochs[[group[[1]]$epoch]]
    replay_text(group, function(x) cast_excel_serial(x, epoch), expect_instant, epoch)
  }
})

test_that("date.json", {
  replay_text(read_vectors("date.json"), cast_date, function(v, i, row, info) {
    expect_date(v$value[[i]], row, info)
  }, "date")
})

test_that("date_order.json", {
  for (group in groups(read_vectors("date_order.json"), function(row) as.character(row$order))) {
    order <- orders[[group[[1]]$order]]
    replay_text(group, function(x) cast_date(x, order), function(v, i, row, info) {
      expect_date(v$value[[i]], row, info)
    }, order)
  }
})

test_that("datetime.json", {
  rows <- read_vectors("datetime.json")
  key <- function(row) paste(row$order, row$am, row$pm)
  for (group in groups(rows, key)) {
    order <- orders[[group[[1]]$order]]
    am <- if (is.null(group[[1]]$am)) "" else group[[1]]$am
    pm <- if (is.null(group[[1]]$pm)) "" else group[[1]]$pm
    replay_text(group, function(x) cast_datetime(x, order, am = am, pm = pm), expect_civil,
                paste("datetime", order))
  }
})

test_that("time.json", {
  replay_text(read_vectors("time.json"), cast_time, function(v, i, row, info) {
    expect_identical(v$nanos[[i]], as.numeric(row$nanos), info = info)
    expect_equal(as.numeric(v$value[[i]]), row$nanos / 1e9, tolerance = 1e-15, info = info)
  }, "time")
})

test_that("duration.json", {
  replay_text(read_vectors("duration.json"), cast_duration, expect_instant, "duration")
})

# The typed doors read the double the vector pins as its IEEE 754 bits; with no text, every
# fault's span is empty.
test_that("typed.json", {
  rows <- read_vectors("typed.json")
  bits <- function(hex) {
    bytes <- as.raw(strtoi(substring(hex, seq(1, 15, 2), seq(2, 16, 2)), 16L))
    readBin(bytes, "double", size = 8L, endian = "big")
  }
  for (group in groups(rows, function(row) paste(row$door, row$epoch))) {
    first <- group[[1]]
    door <- switch(first$door,
      decimal = cast_decimal_from_double,
      excel_serial = function(x) cast_excel_serial_from_double(x, epochs[[first$epoch]]),
      excel_time = cast_excel_time,
      excel_duration = cast_excel_duration,
      stop("unknown typed door ", first$door)
    )
    check <- switch(first$door,
      decimal = function(v, i, row, info) expect_value(v, i, row$value, info),
      excel_serial = expect_civil,
      excel_time = function(v, i, row, info) {
        expect_identical(v$nanos[[i]], as.numeric(row$nanos), info = info)
      },
      excel_duration = expect_instant
    )
    v <- door(vapply(group, function(row) bits(row$bits), 0))
    expect_identical(nrow(v), length(group))
    for (i in seq_along(group)) {
      row <- group[[i]]
      row$fault <- if (row$expect != "ok") list(0L, 0L)
      expect_row(v, i, row, "bytes", check, paste("typed", first$door))
    }
  }
})
