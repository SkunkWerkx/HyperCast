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

corpus_files <- c("boolean.json", "char.json", "constraints.json", "date.json",
                  "date_order.json", "datetime.json", "decimal.json", "declarations.json",
                  "duration.json", "excel_serial.json", "formats.json", "integer.json", "real.json",
                  "time.json", "timestamp.json", "typed.json", "unix.json", "uuid.json")

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
# has no exact double (constraints.json's whole values too, read back per door).
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
  if (name == "constraints.json") {
    json <- gsub("\"value\": (-?[0-9]+)([,}])", "\"value\": \"\\1\"\\2", json)
  }
  rows <- jsonlite::fromJSON(json, simplifyVector = FALSE)
  if (!length(rows)) stop("corpus file has no vectors: ", path)
  rows
}

# The white-space code points the corpus pins, read from its true rows: the test's own
# definition, which the package's table and its C scanner are both held to.
white_space_points <- function() {
  rows <- Filter(function(row) row$kind == "white_space" && row$white_space,
                 read_vectors("formats.json"))
  vapply(rows, function(row) strtoi(sub("^U\\+", "", row$code_point), 16L), 0L)
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

# A row's format. A row that declares no `grouping` keeps legacy semantics (the core's and the
# C ABI's meaning), never the constructor's strict default. A formats.json row may carry a
# policy in its `flags` (bits 8 and 9), which this binding declares beside the styles; a
# policy no name spells (3) and a bit no style is reach the constructor as given, to be refused.
format_of <- function(spec) {
  if (is.null(spec)) {
    return(num_format(grouping = "legacy"))
  }
  opt <- function(name) if (is.null(spec[[name]])) "" else spec[[name]]
  flags <- spec$flags
  grouping <- if (is.null(spec$grouping)) "legacy" else spec$grouping
  policy <- flags %/% 256 %% 4
  if (is.null(spec$grouping) && flags >= 256 && flags < 1024 && policy != 3) {
    grouping <- c("legacy", "western", "indian")[[policy + 1]]
    flags <- flags %% 256
  }
  num_format(spec$decimal_sep, spec$group_sep, flags, currency = opt("currency"),
             currency_decimal_sep = opt("currency_decimal_sep"),
             currency_group_sep = opt("currency_group_sep"), minus_sign = opt("minus_sign"),
             plus_sign = opt("plus_sign"), percent = opt("percent"),
             digit_zero = opt("digit_zero"), grouping = grouping)
}

# A row's `options` as the date options and the month vocabulary the doors take.
months_of <- function(months) {
  month_names(stats::setNames(vapply(months, function(m) as.numeric(m[[1]]), 0),
                              vapply(months, function(m) m[[2]], "")))
}

flags_of <- function(spec) {
  if (!is.null(spec$flag_bits)) spec$flag_bits else vapply(spec$flags, identity, "")
}

options_of <- function(spec) {
  if (is.null(spec)) {
    return(list(options = NULL, months = NULL))
  }
  pivot <- if (is.null(spec$pivot)) 0L else spec$pivot
  list(options = date_options(flags_of(spec), pivot),
       months = if (!is.null(spec$months)) months_of(spec$months))
}

options_key <- function(row) as.character(jsonlite::toJSON(row$options, auto_unbox = TRUE))

orders <- c("year_month_day", "month_day_year", "day_month_year")
precisions <- c("seconds", "milliseconds", "microseconds", "nanoseconds")
epochs <- c("y1900", "y1904")

# Groups rows by a key (a door, a format, an order) so each group is one vectorised call.
groups <- function(rows, key) split(rows, vapply(rows, key, ""))

# Every failure row names its detail: the name, and the core's number it stands for.
expect_detail <- function(v, i, row, info) {
  testthat::expect_false(is.null(row$detail), info = paste(info, "names no detail"))
  testthat::expect_identical(as.character(v$detail[[i]]), row$detail, info = info)
  testthat::expect_identical(v$detail_code[[i]], unname(fault_details[[row$detail]]), info = info)
}

# Checks verdict row i against corpus row `row`: the reason and pinned span, or `check_ok`
# on a success. `units` is "bytes" or "chars"; `label` names the row in a failure.
expect_row <- function(v, i, row, units, check_ok, label) {
  info <- sprintf("%s [%s] %s", label, units, encodeString(row$input, quote = "\""))
  testthat::expect_false(is.na(v$ok[[i]]), info = info)
  if (row$expect == "ok") {
    testthat::expect_true(v$ok[[i]], info = paste(info, "should parse; got", v$reason[[i]]))
    testthat::expect_true(is.na(v$detail[[i]]) && is.na(v$detail_code[[i]]), info = info)
    if (isTRUE(v$ok[[i]])) check_ok(v, i, row, info)
    return(invisible())
  }
  testthat::expect_false(v$ok[[i]], info = paste(info, "unexpectedly parsed"))
  testthat::expect_identical(as.character(v$reason[[i]]), row$expect, info = info)
  expect_detail(v, i, row, info)
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

# A double compared by its IEEE 754 bits: identical() holds 0 and -0 equal, and zero has one
# form (the core never returns -0), so a negative zero must fail here.
expect_bits <- function(got, want, info) {
  testthat::expect_identical(writeBin(as.double(got), raw()), writeBin(as.double(want), raw()),
                             info = paste(info, "bits of", format(got, digits = 17), "vs",
                                          format(want, digits = 17)))
}

expect_instant <- function(v, i, row, info) {
  expect_bits(v$seconds[[i]], as.numeric(row$seconds), info)
  testthat::expect_identical(v$nanos[[i]], as.integer(row$nanos), info = info)
  testthat::expect_identical(as.numeric(v$value[[i]]), row$seconds + trunc(row$nanos / 1e6) / 1e3,
                             info = info)
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
  expect_bits(v$nanos_of_day[[i]], as.numeric(row$nanos_of_day), info)
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
# no R string holds, as out_of_range over the input less its white space edges.
test_that("char.json", {
  rows <- read_vectors("char.json")
  nul <- vapply(rows, function(row) identical(row$value, 0L), FALSE)
  replay_text(rows[!nul], cast_char, function(v, i, row, info) {
    expect_identical(utf8ToInt(v$value[[i]]), row$value, info = info)
  }, "char")
  expect_gt(sum(nul), 0)
  # U+0000 is a success an R string cannot hold: `ok`, with NA as its value.
  replay_text(rows[nul], cast_char, function(v, i, row, info) {
    expect_identical(v$value[[i]], NA_character_, info = info)
  }, "char U+0000")
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
      expect_bits(v$value[[i]], want, info)
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
  for (group in groups(read_vectors("timestamp.json"), options_key)) {
    o <- options_of(group[[1]]$options)
    replay_text(group, function(x) cast_timestamp(x, o$options), expect_instant, "timestamp")
  }
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
  for (group in groups(read_vectors("date.json"), options_key)) {
    o <- options_of(group[[1]]$options)
    # A row's vocabulary is one the ISO door never reads (the core ignores it there); the R door
    # refuses a vocabulary without an order, so it is not passed.
    replay_text(group, function(x) cast_date(x, options = o$options),
                function(v, i, row, info) expect_date(v$value[[i]], row, info), "date")
  }
})

test_that("date_order.json", {
  key <- function(row) paste(row$order, options_key(row))
  for (group in groups(read_vectors("date_order.json"), key)) {
    order <- orders[[group[[1]]$order]]
    o <- options_of(group[[1]]$options)
    replay_text(group, function(x) cast_date(x, order, o$options, o$months),
                function(v, i, row, info) expect_date(v$value[[i]], row, info), order)
  }
})

test_that("datetime.json", {
  rows <- read_vectors("datetime.json")
  key <- function(row) paste(row$order, row$am, row$pm, options_key(row))
  for (group in groups(rows, key)) {
    order <- orders[[group[[1]]$order]]
    am <- if (is.null(group[[1]]$am)) "" else group[[1]]$am
    pm <- if (is.null(group[[1]]$pm)) "" else group[[1]]$pm
    o <- options_of(group[[1]]$options)
    replay_text(group, function(x) {
      cast_datetime(x, order, am = am, pm = pm, options = o$options, months = o$months)
    }, expect_civil, paste("datetime", order))
  }
})

test_that("time.json", {
  replay_text(read_vectors("time.json"), cast_time, function(v, i, row, info) {
    expect_bits(v$nanos[[i]], as.numeric(row$nanos), info)
    expect_identical(as.numeric(v$value[[i]]), (row$nanos %/% 1e6) / 1e3, info = info)
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
      excel_time = function(v, i, row, info) expect_bits(v$nanos[[i]], as.numeric(row$nanos), info),
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

# ---- validation constraints ----

# The core's Refusal Display text, one per code, restated here so a change to the wording
# the binding raises is a test failure.
refusal_rules <- c(
  "a constraint the door does not take", "minimum together with exclusiveMinimum",
  "maximum together with exclusiveMaximum", "the lower bound is not below the upper bound",
  "multipleOf is not positive", "a rounding without multipleOf", "minLength is above maxLength",
  "a bound, step or enum entry the door cannot hold", "an enum entry that is not UTF-8",
  "a duplicate enum entry", "a malformed constraints layout", "an enum with no entries",
  "a text enum entry no input can match"
)

constraint_args <- c(minimum = "minimum", exclusiveMinimum = "exclusive_minimum",
                     maximum = "maximum", exclusiveMaximum = "exclusive_maximum",
                     multipleOf = "multiple_of", rounding = "rounding", minLength = "min_length",
                     maxLength = "max_length", trim = "trim", enum = "enum")

# A corpus row's declaration through the public constructor, its keywords as R arguments.
constraints_of <- function(door, spec) {
  unknown <- setdiff(names(spec), names(constraint_args))
  if (length(unknown)) stop("unknown keyword in the corpus: ", unknown)
  args <- list(door)
  for (key in names(spec)) {
    value <- spec[[key]]
    if (key == "enum") value <- vapply(value, identity, "")
    args[[constraint_args[[key]]]] <- value
  }
  do.call(hypercast_constraints, args)
}

# A from_f64 row's double: JSON has no NaN, infinities or negative zero, so those are text.
f64_of <- function(value) {
  if (!is.character(value)) {
    return(as.numeric(value))
  }
  switch(value, "NaN" = NaN, Infinity = Inf, "-Infinity" = -Inf, "-0" = -0,
         stop("unknown from_f64 spelling ", value))
}

f32_of <- function(x) readBin(writeBin(x, raw(), size = 4L), "double", size = 4L)

expect_checked_value <- function(door, got, want, info) {
  switch(door,
    i8 = , i16 = , u8 = , u16 = testthat::expect_identical(got, as.integer(want), info = info),
    i32 = , u32 = testthat::expect_identical(got, as.numeric(want), info = info),
    f32 = expect_bits(got, f32_of(as.numeric(want)), info),
    f64 = expect_bits(got, as.numeric(want), info),
    testthat::expect_identical(got, want, info = info)
  )
}

# Checks checked-verdict row i against corpus row `row`: the value, adjusted and enum index
# on a success; the reason or the keyword, and the pinned span (re-expressed in characters
# for "chars"), on a rejection.
expect_checked_row <- function(v, i, row, units, label) {
  input <- if (is.null(row$input)) "" else row$input
  info <- sprintf("%s [%s] %s %s", label, units, encodeString(input, quote = "\""),
                  jsonlite::toJSON(row$constraints, auto_unbox = TRUE))
  testthat::expect_false(is.na(v$ok[[i]]), info = info)
  if (row$expect == "ok") {
    testthat::expect_true(v$ok[[i]],
                          info = paste(info, "should pass; got", v$reason[[i]], v$keyword[[i]]))
    if (!isTRUE(v$ok[[i]])) return(invisible())
    expect_checked_value(row$door, v$value[[i]], row$value, info)
    testthat::expect_identical(v$adjusted[[i]], row$adjusted, info = info)
    want_index <- if (is.null(row$enum_index)) NA_integer_ else as.integer(row$enum_index) + 1L
    testthat::expect_identical(v$enum_index[[i]], want_index, info = info)
    testthat::expect_true(is.na(v$reason[[i]]) && is.na(v$keyword[[i]]), info = info)
    testthat::expect_true(is.na(v$detail[[i]]) && is.na(v$detail_code[[i]]), info = info)
    return(invisible())
  }
  testthat::expect_false(v$ok[[i]], info = paste(info, "unexpectedly passed"))
  testthat::expect_true(is.na(v$value[[i]]) && is.na(v$adjusted[[i]]) &&
                          is.na(v$enum_index[[i]]), info = info)
  if (row$expect == "violation") {
    testthat::expect_true(is.na(v$reason[[i]]), info = info)
    testthat::expect_identical(v$keyword[[i]], row$keyword, info = info)
    # A violation carries no detail (the core writes 0).
    testthat::expect_null(row$detail, info = info)
    testthat::expect_true(is.na(v$detail[[i]]) && is.na(v$detail_code[[i]]), info = info)
  } else {
    testthat::expect_identical(as.character(v$reason[[i]]), row$expect, info = info)
    testthat::expect_true(is.na(v$keyword[[i]]), info = info)
    expect_detail(v, i, row, info)
  }
  span <- c(row$fault[[1]], row$fault[[2]])
  if (units == "chars") {
    bytes <- utf8_bytes(row$input)
    span <- c(chars_in(bytes, span[[1]]),
              chars_in(bytes, span[[1]] + span[[2]]) - chars_in(bytes, span[[1]]))
  }
  testthat::expect_identical(c(v$offset[[i]], v$length[[i]]), as.integer(span), info = info)
}

expect_checked_rows <- function(v, rows, units, label) {
  testthat::expect_s3_class(v, "hypercast_checked_verdict")
  testthat::expect_identical(nrow(v), length(rows))
  for (i in seq_along(rows)) expect_checked_row(v, i, rows[[i]], units, label)
}

test_that("constraints.json", {
  rows <- read_vectors("constraints.json")
  kinds <- c(refused = 0L, from_f64 = 0L, input_bytes = 0L, input = 0L)
  key <- function(row) {
    kind <- if (row$expect == "refused") "refused" else if (!is.null(row$from_f64)) "from_f64"
    else if (!is.null(row$input_bytes)) "input_bytes" else "input"
    paste(kind, row$door, jsonlite::toJSON(row$constraints, auto_unbox = TRUE),
          jsonlite::toJSON(row$format, auto_unbox = TRUE))
  }
  for (group in groups(rows, key)) {
    first <- group[[1]]
    label <- paste("constraints", first$door)
    if (first$expect == "refused") {
      for (row in group) {
        kinds[["refused"]] <- kinds[["refused"]] + 1L
        message <- tryCatch({
          constraints_of(row$door, row$constraints)
          "no error"
        }, error = conditionMessage)
        want <- sprintf("Constraints refused (code %d): %s.", row$refusal,
                        refusal_rules[[row$refusal]])
        expect_identical(message, want, info = paste(label, jsonlite::toJSON(row$constraints)))
      }
      next
    }
    constraints <- constraints_of(first$door, first$constraints)
    expect_s3_class(constraints, "hypercast_constraints")
    if (!is.null(first$from_f64)) {
      kinds[["from_f64"]] <- kinds[["from_f64"]] + length(group)
      v <- cast_checked_from_double(vapply(group, function(row) f64_of(row$from_f64), 0),
                                    constraints)
      expect_checked_rows(v, group, "bytes", paste(label, "from_f64"))
      next
    }
    door <- if (first$door == "text") {
      function(x) cast_text(x, constraints)
    } else {
      format <- format_of(first$format)
      function(x) cast_checked(x, format, constraints)
    }
    if (!is.null(first$input_bytes)) {
      # Invalid UTF-8 reaches the core as given from a raw vector and from a string marked
      # "bytes", both counted in bytes as substr() counts them.
      kinds[["input_bytes"]] <- kinds[["input_bytes"]] + length(group)
      bytes <- lapply(group, function(row) as.raw(unlist(row$input_bytes)))
      expect_checked_rows(door(bytes), group, "bytes", paste(label, "input_bytes"))
      marked <- vapply(bytes, function(b) {
        s <- rawToChar(b)
        Encoding(s) <- "bytes"
        s
      }, "")
      expect_checked_rows(door(marked), group, "bytes", paste(label, "input_bytes marked"))
      next
    }
    kinds[["input"]] <- kinds[["input"]] + length(group)
    expect_checked_rows(door(lapply(group, function(row) utf8_bytes(row$input))), group,
                        "bytes", label)
    expect_checked_rows(door(vapply(group, function(row) row$input, "")), group, "chars",
                        label)
  }
  # Every row was replayed, of every kind.
  expect_identical(sum(kinds), length(rows))
  expect_true(all(kinds > 0L))
})

# Every bound, step and integer enum entry crosses through cast_declared_decimal, the one
# conversion; the internal accessor hands back the raw triple, scale included.
test_that("declarations.json", {
  declared <- getFromNamespace("declared_decimal", "hypercast")
  rows <- read_vectors("declarations.json")
  replayed <- 0L
  for (row in rows) {
    replayed <- replayed + 1L
    inputs <- if (is.null(row$input_bytes)) {
      row$input
    } else {
      s <- rawToChar(as.raw(unlist(row$input_bytes)))
      marked <- s
      Encoding(marked) <- "bytes"
      c(s, marked)
    }
    for (input in inputs) {
      info <- paste("declarations", encodeString(input, quote = "\""))
      d <- declared(input)
      if (row$expect == "refused") {
        expect_identical(d$code, as.integer(row$refusal), info = info)
        next
      }
      expect_identical(d$code, 0L, info = info)
      expect_identical(d$magnitude, row$magnitude, info = info)
      expect_identical(d$scale, as.integer(row$scale), info = info)
      expect_identical(d$negative, row$negative, info = info)
      expect_identical(d$value, row$value, info = info)
    }
  }
  # Every row was replayed: the count is the file's, never a literal.
  expect_identical(replayed, length(rows))
})

# ---- formats.json: white space, and the declarations the core refuses ----

# Each refusing rule's message, as num_format() and cast_datetime() word it. A length and a
# digit refusal share one sentence, and so do the two edge rules (a designator's and a currency
# symbol's); the rest are told apart from them and each other.
rule_messages <- c(
  length = "UTF-8 bytes with no ASCII digit", digit = "UTF-8 bytes with no ASCII digit",
  equal_separators = "must differ", white_space = "must not contain white space",
  designator_edge = "must not start or end with white space",
  currency_edge = "must not start or end with white space",
  grouping_policy = "must not carry a grouping policy",
  unknown_flag = "holds only the styles of num_styles",
  flags = "date options in date_flags", pivot = "a year from 100 to 9999",
  layout = "cut short or has trailing bytes", utf8 = "Month name must be valid UTF-8",
  month = "Month must be 1 to 12", month_missing = "have no name for month",
  name_length = "must be 1 to 64 bytes long", character = "must not hold an ASCII digit",
  duplicate = "fold to the same spelling"
)

test_that("formats.json: the package's white space table is the corpus's", {
  rows <- Filter(function(row) row$kind == "white_space", read_vectors("formats.json"))
  expect_gt(length(rows), 25L)
  table <- getFromNamespace("white_space_code_points", "hypercast")
  for (row in rows) {
    point <- strtoi(sub("^U\\+", "", row$code_point), 16L)
    expect_identical(point %in% table, row$white_space, info = row$code_point)
  }
  expect_setequal(table, white_space_points())
  expect_length(table, 25L)
})

# `declare` builds the declaration; it is called once, and its refusal read as a message.
expect_refused_as <- function(declare, row) {
  info <- as.character(jsonlite::toJSON(row, auto_unbox = TRUE))
  message <- tryCatch({
    declare()
    NULL
  }, error = conditionMessage)
  if (row$expect == "ok") {
    testthat::expect_null(message, info = info)
    return(invisible())
  }
  testthat::expect_false(is.null(message), info = paste(info, "was not refused"))
  testthat::expect_match(message, rule_messages[[row$rule]], fixed = TRUE, info = info)
  # The message is the winning rule's alone: no other rule's wording is in it.
  for (other in setdiff(unique(rule_messages), rule_messages[[row$rule]])) {
    testthat::expect_false(grepl(other, message, fixed = TRUE),
                           info = paste(info, "also says", other))
  }
}

test_that("formats.json: num_format() and cast_datetime() refuse what the core refuses", {
  rows <- read_vectors("formats.json")
  counts <- c(num_format = 0L, day_periods = 0L, white_space = 0L, date_options = 0L,
              month_names = 0L)
  for (row in rows) {
    counts[[row$kind]] <- counts[[row$kind]] + 1L
    if (row$kind == "num_format") {
      expect_refused_as(function() format_of(row$format), row)
    } else if (row$kind == "day_periods") {
      am <- if (is.null(row$am)) "" else row$am
      pm <- if (is.null(row$pm)) "" else row$pm
      expect_refused_as(function() {
        cast_datetime("1/7/2026 3 PM", "month_day_year", am = am, pm = pm)
      }, row)
    } else if (row$kind == "date_options") {
      pivot <- if (is.null(row$pivot)) 0L else row$pivot
      expect_refused_as(function() date_options(flags_of(row), pivot), row)
    } else if (row$kind == "month_names") {
      expect_refused_as(function() {
        if (is.null(row$append_bytes)) {
          months_of(row$months)
        } else {
          packed <- getFromNamespace("month_names_packed", "hypercast")
          packed(c(months_of(row$months)$packed, as.raw(unlist(row$append_bytes))))
        }
      }, row)
    } else {
      expect_identical(row$kind, "white_space")
    }
  }
  # Every row was replayed, of every kind.
  expect_identical(sum(counts), length(rows))
  expect_true(all(counts > 0L))
})
