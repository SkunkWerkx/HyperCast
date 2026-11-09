test_that("a verdict is a data frame of one row per element, in order", {
  v <- cast_i32(c("42", "(1,234)", "12x", "", NA), num_format())
  expect_s3_class(v, c("hypercast_verdict", "data.frame"), exact = TRUE)
  expect_named(v, c("value", "ok", "reason", "offset", "length", "detail", "detail_code"))
  expect_identical(attr(v, "door"), "cast_i32")
  expect_identical(nrow(v), 5L)
  expect_identical(v$value, c(42, -1234, NA, NA, NA))
  expect_identical(v$ok, c(TRUE, TRUE, FALSE, FALSE, NA))
  expect_s3_class(v$reason, "factor")
  expect_identical(levels(v$reason), c("empty", "malformed", "out_of_range"))
  expect_identical(as.character(v$reason), c(NA, NA, "malformed", "empty", NA))
  expect_identical(v$offset, c(NA, NA, 2L, 0L, NA))
  expect_identical(v$length, c(NA, NA, 1L, 0L, NA))
  expect_identical(is_ok(v), v$ok)
})

test_that("an empty input is an empty verdict with every column", {
  v <- cast_timestamp(character())
  expect_identical(nrow(v), 0L)
  expect_named(v, c("value", "seconds", "nanos", "ok", "reason", "offset", "length", "detail",
                    "detail_code"))
  expect_s3_class(v$value, "POSIXct")
  expect_identical(nrow(faults(v)), 0L)
  expect_identical(nrow(cast_excel_time(numeric())), 0L)
})

test_that("faults() lists the faulted rows, and their spans slice the input back out", {
  x <- c("42", "4\u00e92", "(1,234", "1e999", "")
  f <- faults(cast_i32(x, num_format()))
  expect_identical(f$row, 2:5)
  expect_identical(as.character(f$reason), c("malformed", "malformed", "out_of_range", "empty"))
  expect_identical(substr(x[f$row], f$offset + 1L, f$offset + f$length),
                   c("\u00e9", "(1,234", "1e999", ""))
})

test_that("optional() makes empty faults missing and leaves every other row", {
  v <- optional(cast_f64(c("1.5", "", " \t ", "x", NA), num_format()))
  expect_identical(v$ok, c(TRUE, NA, NA, FALSE, NA))
  expect_identical(as.character(v$reason), c(NA, NA, NA, "malformed", NA))
  expect_identical(v$offset, c(NA, NA, NA, 0L, NA))
  expect_identical(v$value, c(1.5, NA, NA, NA, NA))
  expect_s3_class(v, "hypercast_verdict")
})

test_that("a verdict prints its door and counts, and converts and subsets as a data frame", {
  v <- cast_bool(c("yes", "nope", NA))
  out <- capture.output(res <- print(v))
  expect_identical(res, v)
  expect_identical(out[[1]], "<hypercast_verdict: cast_bool> 1 ok, 1 faulted, 1 missing")
  plain <- as.data.frame(v)
  expect_identical(class(plain), "data.frame")
  expect_identical(plain$value, c(TRUE, NA, NA))
  expect_s3_class(v[2:3, ], "hypercast_verdict")
  expect_identical(nrow(rbind(v, v)), 6L)
  expect_error(is_ok(plain), "hypercast_verdict")
})

test_that("each door's value comes back in its R type", {
  f <- num_format()
  expect_type(cast_bool("t")$value, "logical")
  expect_identical(cast_char("U+00E9")$value, "\u00e9")
  expect_identical(Encoding(cast_char("U+00E9")$value), "UTF-8")
  expect_identical(cast_i8("0xFF", f)$value, -1L)
  expect_identical(cast_u16("65535", f)$value, 65535L)
  expect_identical(cast_i32("-2147483648", f)$value, -2147483648)
  expect_identical(cast_u32("4294967295", f)$value, 4294967295)
  expect_identical(cast_i64("-9223372036854775808", f)$value, "-9223372036854775808")
  expect_identical(cast_u64("18446744073709551615", f)$value, "18446744073709551615")
  expect_identical(cast_f32("0.1", f)$value, readBin(writeBin(0.1, raw(), size = 4L), "double",
                                                     size = 4L))
  expect_identical(cast_decimal(c("1.10", "-0.000", "(2.5)%"), f)$value, c("1.1", "0", "-0.025"))
  expect_identical(cast_uuid("urn:uuid:6BA7B810-9DAD-11D1-80B4-00C04FD430C8")$value,
                   "6ba7b810-9dad-11d1-80b4-00c04fd430c8")
  ts <- cast_timestamp("1969-12-31T23:59:59.5Z")
  expect_s3_class(ts$value, "POSIXct")
  expect_identical(attr(ts$value, "tzone"), "UTC")
  expect_identical(c(ts$seconds, as.numeric(ts$nanos)), c(-1, 5e8))
  expect_identical(as.numeric(ts$value), -0.5)
  expect_identical(cast_date("0001-01-01")$value, as.Date("0001-01-01"))
  expect_s3_class(cast_time("12:00")$value, "difftime")
  expect_identical(units(cast_duration("PT1M")$value), "secs")
  expect_identical(as.numeric(cast_duration("-PT1M30.5S")$value), -90.5)
})

test_that("a civil date-time names no instant until the caller names a zone", {
  v <- cast_datetime(c("1/7/2026 3:04:05.25 PM", "2026-01-07T00:00"), "month_day_year")
  expect_identical(v$value, c("2026-01-07 15:04:05.25", "2026-01-07 00:00:00"))
  expect_identical(v$date, as.Date(c("2026-01-07", "2026-01-07")))
  expect_identical(v$nanos_of_day, c(54245.25e9, 0))
  chicago <- as.POSIXct(v$value[[1]], tz = "America/Chicago")
  expect_identical(format(chicago, "%Y-%m-%d %H:%M:%OS2 %Z"), "2026-01-07 15:04:05.25 CST")
  expect_identical(cast_datetime("2026-01-07 15:04Z", "year_month_day")$ok, FALSE)
})

test_that("value stops at the millisecond, and the exact columns keep the rest", {
  end <- cast_timestamp("9999-12-31T23:59:59.999999999Z")
  expect_identical(as.numeric(end$value), 253402300799.999)
  expect_identical(format(end$value, "%Y"), "9999")
  expect_identical(c(end$seconds, as.numeric(end$nanos)), c(253402300799, 999999999))
  before <- cast_timestamp("1969-12-31T23:59:59.9999Z")
  expect_identical(as.numeric(before$value), -1 + 0.999)
  day <- cast_time("23:59:59.999999999")
  expect_lt(as.numeric(day$value), 86400)
  expect_identical(day$nanos, 86399999999999)
  negative <- cast_duration("-PT0.0019S")
  expect_identical(as.numeric(negative$value), -0.001)
  expect_identical(negative$nanos, -1900000L)
})

test_that("U+0000 is a success with NA as its value, since an R string cannot hold it", {
  v <- cast_char(list(charToRaw(" U+0000 "), charToRaw("&#0;"), charToRaw("00"), as.raw(0L),
                      charToRaw("\u00a0U+0000\u3000")))
  expect_identical(v$ok, rep(TRUE, 5L))
  expect_identical(v$value, rep(NA_character_, 5L))
  expect_identical(v$reason, factor(rep(NA, 5L), levels = levels(v$reason)))
  expect_identical(nrow(faults(v)), 0L)
  # Told apart from a missing input (ok is NA) and from a fault (ok is FALSE).
  three <- cast_char(c("U+0000", NA, "ab"))
  expect_identical(three$ok, c(TRUE, NA, FALSE))
  expect_identical(cast_char("0")$value, "0")
})

test_that("U+0000 has an exact output", {
  v <- cast_char("U+0000")
  expect_identical(c(v$ok, is.na(v$value), v$code_point), c(TRUE, TRUE, 0L))
  expect_identical(cast_char("A")$code_point, 65L)
  # The top of the range, a fault and a missing input.
  rows <- cast_char(c("U+10FFFF", "ab", NA))
  expect_identical(rows$code_point, c(0x10FFFFL, NA, NA))
  expect_identical(rows$value[[1]], "\U10FFFF")
})

test_that("text R can't hold is ok with NA, and raw gives the bytes", {
  bytes <- as.raw(c(0x61, 0x00, 0x62))
  v <- cast_text(list(bytes))
  expect_true(v$ok)
  expect_true(is.na(v$value))
  expect_identical(nrow(faults(v)), 0L)
  expect_identical(cast_text(list(bytes), raw = TRUE)$value[[1]], bytes)
  expect_true(is.na(cast_text(NA_character_)$ok)) # missing input stays distinct
})

test_that("cast_text(raw = TRUE) holds every row's exact output, or NULL where there is none", {
  latin1 <- "\xe9"
  Encoding(latin1) <- "latin1"
  native <- cast_text(c("plain", latin1, NA), raw = TRUE)
  expect_s3_class(native, "hypercast_verdict")
  expect_type(native$value, "list")
  expect_identical(native$value[1:2], list(charToRaw("plain"), as.raw(c(0xc3, 0xa9))))
  expect_null(native$value[[3]])
  expect_identical(native$ok, c(TRUE, TRUE, NA))
  # A fault has no output; it is the same fault the character value reports.
  bad <- list(as.raw(c(0x61, 0xff)), NULL, as.raw(0L))
  v <- cast_text(bad, raw = TRUE)
  expect_identical(v$ok, c(FALSE, NA, TRUE))
  expect_null(v$value[[1]])
  expect_null(v$value[[2]])
  expect_identical(v$value[[3]], as.raw(0L))
  plain <- cast_text(bad)
  expect_identical(plain[names(plain) != "value"], v[names(v) != "value"])
  # A verdict, list column included: it subsets, faults and prints as any other does.
  expect_identical(faults(v), faults(plain))
  expect_identical(v[3, ]$value[[1]], as.raw(0L))
  expect_output(print(v), "1 ok, 1 faulted, 1 missing")
  expect_identical(optional(v)$value, v$value)
  expect_identical(cast_text(character(), raw = TRUE)$value, list())
})

test_that("cast_text(raw =) takes one TRUE or FALSE, and leaves the constraints to the caller", {
  for (bad in list(NA, NULL, "TRUE", 1L, c(TRUE, FALSE))) {
    expect_error(cast_text("x", raw = bad), "raw must be TRUE or FALSE")
    expect_error(cast_text("x", hypercast_constraints("text"), raw = bad),
                 "raw must be TRUE or FALSE")
  }
  # raw alone is the plain door, and an explicit `constraints` still has to be a declaration.
  expect_s3_class(cast_text("x", raw = TRUE), "hypercast_verdict")
  expect_identical(cast_text("x", raw = FALSE), cast_text("x"))
  expect_error(cast_text("x", NULL, raw = TRUE), "must be a hypercast_constraints")
  forward <- function(x, constraints, ...) cast_text(x, constraints, ...)
  expect_error(forward("x", raw = TRUE), "\"constraints\" is missing")
})

test_that("inputs are read in their own units: characters, or bytes for raw and \"bytes\"", {
  latin1 <- "\xe9\xe912x"
  Encoding(latin1) <- "latin1"
  bytes <- "\xc3\xa912x"
  Encoding(bytes) <- "bytes"
  v <- cast_i32(c(latin1, bytes, "\u00e912x"), num_format())
  expect_identical(v$offset, c(0L, 0L, 0L))
  expect_identical(v$length, c(1L, 2L, 1L))
  r <- cast_i32(list(charToRaw("12x"), NULL, charToRaw("\u00e9")), num_format())
  expect_identical(r$offset, c(2L, NA, 0L))
  expect_identical(r$length, c(1L, NA, 2L))
  expect_identical(r$ok, c(FALSE, NA, FALSE))
  expect_identical(cast_bool(charToRaw("yes"))$value, TRUE)
  expect_identical(cast_bool(factor(c("no", "yes", "no")))$value, c(FALSE, TRUE, FALSE))
  expect_identical(cast_bool(NA)$ok, NA)
})

test_that("an unmarked string in a C session is read as its bytes, and counted in bytes", {
  old <- Sys.getlocale("LC_CTYPE")
  on.exit(Sys.setlocale("LC_CTYPE", old), add = TRUE)
  skip_if(suppressWarnings(Sys.setlocale("LC_CTYPE", "C")) == "", "no C locale")
  skip_if(isTRUE(l10n_info()[["UTF-8"]]), "the C locale is UTF-8 here")
  # U+2212 MINUS SIGN as UTF-8 bytes in an unmarked string, as readLines() gives in the C
  # locale: R would translate each non-ASCII byte to a "<xx>" escape.
  minus <- rawToChar(as.raw(c(0xe2, 0x88, 0x92)))
  expect_identical(Encoding(minus), "unknown")
  f <- num_format(minus_sign = minus)
  x <- paste0(c("", "1"), minus, "5")
  v <- cast_i32(x, f)
  expect_identical(v$value, c(-5, NA))
  expect_identical(c(v$offset[[2]], v$length[[2]]), c(1L, 3L))
  expect_identical(substr(x[[2]], v$offset[[2]] + 1L, v$offset[[2]] + v$length[[2]]), minus)
})

test_that("the C locale reads as its bytes on every platform", {
  native <- getFromNamespace("session_native", "hypercast")
  utf8 <- list(MBCS = TRUE, `UTF-8` = TRUE, `Latin-1` = FALSE)
  expect_identical(native(c(utf8, codeset = "UTF-8")), 0L)
  expect_identical(native(c(utf8, codepage = 65001L, system.codepage = 65001L)), 0L)
  ascii <- list(MBCS = FALSE, `UTF-8` = FALSE, `Latin-1` = FALSE)
  expect_identical(native(c(ascii, codeset = "ANSI_X3.4-1968")), 1L)
  # Windows' l10n_info() names a code page, not a codeset, and its C locale is code page 0.
  expect_identical(native(c(ascii, codepage = 0L, system.codepage = 65001L)), 1L)
  latin1 <- list(MBCS = FALSE, `UTF-8` = FALSE, `Latin-1` = TRUE)
  expect_identical(native(c(latin1, codeset = "ISO-8859-1")), 2L)
  expect_identical(native(c(latin1, codepage = 1252L, system.codepage = 65001L)), 2L)
})

test_that("a typed door reads NaN as a number and NA as missing", {
  v <- cast_decimal_from_double(c(0.1, NaN, NA, 1L))
  expect_identical(v$value, c("0.1", NA, NA, "1"))
  expect_identical(v$ok, c(TRUE, FALSE, NA, TRUE))
  expect_identical(c(v$offset[[2]], v$length[[2]]), c(0L, 0L))
  expect_identical(cast_excel_serial_from_double(60, "y1900")$ok, FALSE)
  expect_identical(cast_excel_serial_from_double(61, "y1900")$value, "1900-03-01 00:00:00")
})

test_that("num_format() declares a format, and refuses one that cannot be", {
  f <- num_format(",", ".", c("grouping", "currency"), currency = "kr.", minus_sign = "\u2212")
  expect_s3_class(f, "hypercast_num_format")
  expect_identical(f$flags, 65L)
  expect_length(f$packed, 92L)
  expect_identical(f$packed[1:4], as.raw(c(92, 0, 0, 0)))
  # print() writes the minus sign as encodeString() does in this session's locale.
  expect_identical(capture.output(print(f)), capture.output(cat(
    "num_format(\",\", \".\", 65, currency = \"kr.\", minus_sign = ",
    encodeString("\u2212", quote = "\""), ")\n", sep = ""
  )))
  expect_identical(cast_decimal("\u22121.234,5 kr.", f)$value, "-1234.5")
  expect_identical(num_format_detect()$flags, 127L)
  expect_error(num_format(".", "."), "must differ")
  expect_error(num_format("1", ","), "no ASCII digit")
  expect_error(num_format("123456789", ","), "1 to 8 UTF-8 bytes")
  expect_error(num_format(currency_decimal_sep = ","), "Currency decimal and group")
  expect_error(num_format(minus_sign = "- "), "must not contain white space")
  expect_error(num_format(minus_sign = "\u00a0"), "Minus sign must not contain white space")
  expect_error(num_format(percent = "\u202f%"), "Percent symbol must not contain white space")
  expect_error(num_format(currency = " kr"),
               "Currency symbol must not start or end with white space")
  expect_error(num_format(currency = "kr\u00a0"),
               "Currency symbol must not start or end with white space")
  expect_error(num_format(currency = "\u202fF CFA"),
               "Currency symbol must not start or end with white space")
  expect_error(num_format(currency = "F CFA\u3000"),
               "Currency symbol must not start or end with white space")
  # White space inside a currency symbol is a declaration the core reads: F CFA, whichever space.
  for (space in c(" ", "\u00a0", "\u202f", "\u2009", "\u3000")) {
    cfa <- paste0("F", space, "CFA")
    expect_identical(num_format(currency = cfa)$currency, cfa)
  }
  expect_error(num_format("\u00a0", ","), "Decimal separator must not contain white space")
  expect_error(num_format(currency_decimal_sep = "x\u2009", currency_group_sep = "."),
               "Currency decimal separator must not contain white space")
  expect_error(num_format("\u00a0", "\u00a0"), "must differ")
  expect_identical(num_format(",", "\u00a0")$group_sep, "\u00a0")
  expect_error(num_format(currency = "12"), "Currency symbol")
  expect_error(num_format(digit_zero = "0"), "Digit zero")
  expect_error(num_format(digit_zero = "\u0679"), "Digit zero")
  expect_identical(num_format(digit_zero = "\u0660")$digit_zero, "\u0660")
  expect_error(num_format(flags = "lenient"), "no style")
  expect_error(num_format(flags = -1), "flags")
  expect_error(num_format(NA, ","), "single string")
  # Text that is not UTF-8 (a lone surrogate's bytes, marked UTF-8) declares nothing.
  surrogate <- "\xed\xa0\x80"
  Encoding(surrogate) <- "UTF-8"
  expect_error(num_format(currency = surrogate), "^Currency symbol must be valid UTF-8$")
  expect_error(num_format(surrogate, ","), "^Decimal separator must be valid UTF-8$")
  expect_error(cast_datetime("1/7/2026", "day_month_year", am = surrogate),
               "^Day period am must be valid UTF-8$")
})

test_that("num_format_from_localeconv() bridges a locale's conventions", {
  de <- c(decimal_point = ",", thousands_sep = ".", currency_symbol = "\u20ac",
          mon_decimal_point = ",", mon_thousands_sep = ".", negative_sign = "-",
          positive_sign = "", n_sign_posn = "1")
  f <- num_format_from_localeconv(de)
  expect_identical(c(f$decimal_sep, f$group_sep, f$currency), c(",", ".", "\u20ac"))
  expect_identical(f$minus_sign, "")
  expect_identical(cast_f64("-1.234,5 \u20ac", f)$value, -1234.5)
  trailing <- num_format_from_localeconv(c(decimal_point = ",", thousands_sep = "",
                                           n_sign_posn = "2"))
  expect_identical(c(trailing$decimal_sep, trailing$group_sep), c(",", "."))
  expect_identical(bitwAnd(trailing$flags, num_styles[["trailing_sign"]]), 128L)
  expect_identical(num_format_from_localeconv(c(decimal_point = ""))$decimal_sep, ".")
  expect_s3_class(num_format_from_localeconv(), "hypercast_num_format")
  expect_error(num_format_from_localeconv(list()), "named character vector")
})

test_that("the declarations and the input's type are arguments, checked before any cast", {
  expect_error(cast_i32("1", "invariant"), "hypercast_num_format")
  expect_error(cast_i32("1"), "format")
  expect_error(cast_date("1/7/2026", "mdy"), "order must be one of")
  expect_error(cast_unix("1", "ms"), "precision must be one of")
  expect_error(cast_excel_serial("1", 1900), "epoch must be one of")
  expect_error(cast_datetime("1/7/2026 3 PM", "month_day_year", am = " am"),
               "Day period am must not start or end with white space")
  expect_error(cast_datetime("1/7/2026 3 PM", "month_day_year", am = "am\u00a0"),
               "Day period am must not start or end with white space")
  expect_error(cast_datetime("1/7/2026 3 PM", "month_day_year", pm = "\u202fpm"),
               "Day period pm must not start or end with white space")
  expect_error(cast_datetime("1/7/2026 3 PM", "month_day_year", pm = "p1"), "Day period pm")
  expect_error(cast_bool(42), "character vector or a list of raw vectors")
  expect_error(cast_bool(list("yes")), "character vector or a list of raw vectors")
  expect_error(cast_excel_time("0.5"), "numeric vector")
})

test_that("the native version is the package version", {
  expect_s3_class(hypercast_native_version(), "package_version")
  expect_equal(hypercast_native_version(), packageVersion("hypercast"))
})

# The plain verdict's shape is public: positional readers and rbind() with earlier results
# depend on it, so the checked doors' verdict is a class of its own and this one never moves.
test_that("the plain verdict keeps exactly its columns, in order, for every kind of door", {
  f <- num_format()
  verdict <- c("ok", "reason", "offset", "length")
  shapes <- list(
    list(cast_bool("t"), "value"), list(cast_char("A"), c("value", "code_point")),
    list(cast_i8("1", f), "value"), list(cast_i32("1", f), "value"),
    list(cast_i64("1", f), "value"), list(cast_f64("1", f), "value"),
    list(cast_decimal("1", f), "value"), list(cast_uuid("x"), "value"),
    list(cast_text("x"), "value"),
    list(cast_timestamp("x"), c("value", "seconds", "nanos")),
    list(cast_unix("1", "seconds"), c("value", "seconds", "nanos")),
    list(cast_excel_serial("1", "y1900"), c("value", "seconds", "nanos")),
    list(cast_date("x"), "value"), list(cast_date("x", "day_month_year"), "value"),
    list(cast_datetime("x", "day_month_year"), c("value", "date", "nanos_of_day")),
    list(cast_time("x"), c("value", "nanos")),
    list(cast_duration("x"), c("value", "seconds", "nanos")),
    list(cast_decimal_from_double(1), "value"),
    list(cast_excel_serial_from_double(1, "y1900"), c("value", "date", "nanos_of_day")),
    list(cast_excel_time(0.5), c("value", "nanos")),
    list(cast_excel_duration(1), c("value", "seconds", "nanos"))
  )
  for (shape in shapes) {
    v <- shape[[1]]
    expect_s3_class(v, c("hypercast_verdict", "data.frame"), exact = TRUE)
    expect_identical(names(v), c(shape[[2]], verdict, "detail", "detail_code"),
                     info = attr(v, "door"))
    expect_s3_class(v$detail, "factor")
    expect_identical(levels(v$detail), c(names(fault_details), "unknown"))
    expect_type(v$detail_code, "integer")
    expect_identical(levels(v$reason), c("empty", "malformed", "out_of_range"))
    expect_type(v$ok, "logical")
    expect_type(v$offset, "integer")
    expect_type(v$length, "integer")
  }
  expect_identical(names(faults(cast_bool("x"))),
                   c("row", "reason", "offset", "length", "detail", "detail_code"))
})

test_that("the checked verdict is its own class, with its own columns, in order", {
  v <- cast_checked("5", num_format(), hypercast_constraints("i32", minimum = "1"))
  expect_s3_class(v, c("hypercast_checked_verdict", "data.frame"), exact = TRUE)
  expect_identical(names(v), c("value", "adjusted", "enum_index", "ok", "reason", "keyword",
                               "offset", "length", "detail", "detail_code"))
  expect_type(v$adjusted, "logical")
  expect_type(v$enum_index, "integer")
  expect_type(v$keyword, "character")
  expect_identical(levels(v$reason), c("empty", "malformed", "out_of_range"))
  expect_identical(names(faults(v)),
                   c("row", "reason", "offset", "length", "detail", "detail_code"))
  expect_identical(names(violations(v)), c("row", "keyword", "offset", "length"))
})

# ---- grouping ----

test_that("a format reads grouping Western unless the caller says otherwise", {
  f <- num_format()
  expect_identical(f$grouping, "western")
  v <- cast_i32(c("1,234,567", "12,34", "1234,567", "1234567", ",123", "123,", "1,,234"), f)
  expect_identical(v$ok, c(TRUE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE))
  expect_identical(v$value[c(1, 4)], c(1234567, 1234567))
  expect_identical(as.character(v$detail[2:3]), c("group_size", "group_size"))
  # An invalid group spans its separator: ",34"; an oversized leftmost one has none: "1234".
  expect_identical(c(v$offset[2], v$length[2]), c(2L, 3L))
  expect_identical(c(v$offset[3], v$length[3]), c(0L, 4L))
  # A policy does not turn grouping on: the style stays the switch.
  off <- num_format(flags = 0L)
  expect_identical(as.character(cast_i32("1,234", off)$reason), "malformed")
  expect_identical(as.character(cast_i32("1,234", off)$detail), "unexpected_character")
})

test_that("Indian is an explicit option, chosen in both directions", {
  refused <- cast_i32(c("1,23,456", "1,2,345", "12,34", "0,125", "1234,567"), num_format())
  expect_identical(as.character(refused$detail), rep("group_size", 5L))
  expect_identical(refused$offset, c(1L, 1L, 2L, 0L, 0L))
  indian <- num_format(grouping = "indian")
  v <- cast_i32(c("12,34,567", "12,345", "123,456", "1,23,456"), indian)
  expect_identical(v$ok, c(TRUE, TRUE, FALSE, TRUE))
  expect_identical(as.character(v$detail[3]), "group_size")
  expect_identical(num_format_detect(grouping = "indian")$grouping, "indian")
  expect_error(num_format(grouping = "european"), "grouping must be one of")
  expect_error(num_format(grouping = "legacy"), "grouping must be one of")
  expect_identical(capture.output(print(indian)),
                   "num_format(\".\", \",\", 95, grouping = \"indian\")")
  # The word is what a door reads, so a field set by hand can't quietly disagree with it.
  mislabelled <- indian
  mislabelled$grouping <- "western"
  expect_error(cast_i32("12,34,567", mislabelled), "its flags say \"indian\"")
  expect_error(num_format_with_grouping(mislabelled, "western"), "its flags say \"indian\"")
  western <- num_format()
  western$grouping <- "indian"
  expect_error(cast_i32("1,234", western), "its flags say \"western\"")
  # The way to change it, and a 1.0 format, which has no field, both pass.
  expect_identical(num_format_with_grouping(indian, "western")$grouping, "western")
  stored <- western
  stored$grouping <- NULL
  expect_equal(cast_i32("1,234", stored)$value, 1234)
  expect_identical(capture.output(print(num_format())), "num_format(\".\", \",\", 95)")
  # num_format_with_grouping() replaces the policy and nothing else.
  back <- num_format_with_grouping(indian, "western")
  expect_identical(back, num_format())
  expect_identical(num_format_with_grouping(num_format(), "indian"), indian)
  expect_error(num_format_with_grouping(indian, "legacy"), "grouping must be one of")
  expect_error(num_format_with_grouping(list(), "indian"), "must be a hypercast_num_format")
  euro <- num_format(",", ".", currency = "\u20ac")
  expect_identical(num_format_with_grouping(euro, "indian"),
                   num_format(",", ".", currency = "\u20ac", grouping = "indian"))
})

test_that("a style mask never carries a policy", {
  expect_error(num_format(flags = 351), "must not carry a grouping policy")
  expect_error(num_format(flags = 768), "must not carry a grouping policy")
  expect_error(num_format(flags = 1119), "holds only the styles of num_styles")
  expect_error(num_format(flags = 2147483743), "holds only the styles of num_styles")
  # The policy sits beside the styles in the packed flags word, replaced and never OR-ed.
  flags <- function(f) sum(as.integer(f$packed[5:8]) * 256^(0:3))
  expect_identical(flags(num_format()), 95)
  expect_identical(flags(num_format(grouping = "indian")), 95 + 512)
})

test_that("native digits with a space-like group separator are counted in digits", {
  f <- num_format(",", "\u202f", digit_zero = "\u0660")
  v <- cast_i32(c("\u0661\u202f\u0662\u0663\u0664", "\u0661\u0662\u202f\u0663\u0664"), f)
  expect_identical(v$ok, c(TRUE, FALSE))
  expect_identical(v$value[[1]], 1234)
  expect_identical(as.character(v$detail[[2]]), "group_size")
  expect_identical(c(v$offset[[2]], v$length[[2]]), c(2L, 3L))
  raw <- cast_i32(list(charToRaw("\u0661\u0662\u202f\u0663\u0664")), f)
  expect_identical(c(raw$offset, raw$length), c(4L, 7L))
})

# localeconv's grouping as the bytes C writes: a size per group from the right, the last repeating,
# 127 (CHAR_MAX) ending the pattern.
sizes <- function(...) intToUtf8(c(...), multiple = FALSE)

# `digits` written with `sep` under a localeconv grouping vector (what the platform itself writes).
platform_grouped <- function(digits, sep, pattern) {
  size <- pattern
  out <- character()
  rest <- digits
  repeat {
    n <- size[[1L]]
    if (length(size) > 1L) size <- size[-1L]
    if (n >= 127L || nchar(rest) <= n) break
    out <- c(substring(rest, nchar(rest) - n + 1L), out)
    rest <- substring(rest, 1L, nchar(rest) - n)
  }
  paste(c(rest, out), collapse = sep)
}

bridge <- function(...) {
  num_format_from_localeconv(c(decimal_point = ".", thousands_sep = ",", ...))
}
policy_of <- function(...) bridge(...)$grouping
groups <- function(f) bitwAnd(f$flags, num_styles[["grouping"]]) != 0L

test_that("num_format_from_localeconv() declares the policy from the locale's own patterns", {
  # The actual patterns: en-US (3), en-US-POSIX (primary 3, secondary 0: repeat), en-IN (3;2),
  # tok (groups of two), en-BG (no currency grouping), ICU ml/ta (number 3;2, currency 3).
  expect_identical(policy_of(grouping = sizes(3), mon_grouping = sizes(3)), "western")
  expect_identical(policy_of(grouping = sizes(3, 3)), "western")
  expect_identical(policy_of(grouping = sizes(3, 2), mon_grouping = sizes(3, 2)), "indian")
  expect_identical(policy_of(grouping = sizes(3, 2, 2)), "indian")
  expect_identical(policy_of(grouping = sizes(2), mon_grouping = sizes(2)), "western")
  expect_identical(policy_of(grouping = sizes(2, 3)), "western")
  expect_identical(policy_of(grouping = sizes(3, 2, 3)), "western")
  # CHAR_MAX ends a pattern: a pattern that ends is another shape, and one that starts with it
  # groups nothing.
  expect_identical(policy_of(grouping = sizes(3, 127)), "western")
  expect_false(groups(bridge(grouping = sizes(127), mon_grouping = "")))
  expect_identical(policy_of(grouping = sizes(3), mon_grouping = sizes(127)), "western")
  expect_true(groups(bridge(grouping = sizes(3), mon_grouping = sizes(127))))
  # The number pattern decides, whatever the currency's says.
  expect_identical(policy_of(grouping = sizes(3), mon_grouping = ""), "western")
  expect_identical(policy_of(grouping = sizes(3, 2), mon_grouping = ""), "indian")
  expect_identical(policy_of(grouping = "", mon_grouping = sizes(3, 2)), "western")
  expect_identical(policy_of(grouping = sizes(3, 2), mon_grouping = sizes(127)), "indian")
  expect_identical(policy_of(grouping = sizes(3, 2), mon_grouping = sizes(3)), "indian")
  expect_identical(policy_of(grouping = sizes(3), mon_grouping = sizes(3, 2)), "western")
  expect_identical(policy_of(grouping = sizes(3), mon_grouping = sizes(2)), "western")
  expect_identical(policy_of(grouping = sizes(2), mon_grouping = sizes(3, 2)), "western")
  expect_true(groups(bridge(grouping = sizes(3, 2), mon_grouping = sizes(3))))
  # Neither groups: the style is cleared (a group separator is then malformed); one that does
  # keeps it.
  none <- bridge(grouping = "", mon_grouping = "")
  expect_false(groups(none))
  expect_identical(none$grouping, "western")
  expect_identical(cast_i32("1,234", none)$ok, FALSE)
  expect_false(groups(bridge(grouping = sizes(127), mon_grouping = sizes(127))))
  expect_true(groups(bridge(grouping = "", mon_grouping = sizes(3))))
  # A vector that states neither pattern declares nothing: the default.
  expect_true(groups(bridge()))
  expect_identical(policy_of(), "western")
  trailing <- num_format_from_localeconv(c(decimal_point = ",", n_sign_posn = "2"))
  expect_identical(bitwAnd(trailing$flags, num_styles[["trailing_sign"]]), 128L)
})

test_that("the bridge's format reads what its platform writes in the pattern of its policy", {
  money <- c(currency_symbol = "\u20b9", mon_decimal_point = ".", mon_thousands_sep = ",")
  platforms <- list(
    en_US = list(number = 3L, currency = 3L), posix = list(number = 3L, currency = 3L),
    en_IN = list(number = c(3L, 2L), currency = c(3L, 2L)),
    en_BG = list(number = 3L, currency = 127L),
    ml = list(number = c(3L, 2L), currency = 3L),
    ta = list(number = c(3L, 2L), currency = 3L)
  )
  want <- c(en_US = "western", posix = "western", en_IN = "indian",
            en_BG = "western", ml = "indian", ta = "indian")
  for (name in names(platforms)) {
    p <- platforms[[name]]
    conv <- c(grouping = do.call(sizes, as.list(p$number)),
              mon_grouping = do.call(sizes, as.list(p$currency)), money)
    f <- bridge(conv)
    expect_identical(f$grouping, unname(want[[name]]), info = name)
    for (digits in c("7", "1234", "12345", "1234567", "123456789")) {
      number <- platform_grouped(digits, ",", p$number)
      expect_identical(cast_decimal(number, f)$value, digits, info = paste(name, number))
      amount <- paste0("\u20b9", platform_grouped(digits, ",", p$currency), ".50")
      # A Western amount through ml's Indian numbers is refused until the column declares Western.
      reader <- if (name %in% c("ml", "ta")) num_format_with_grouping(f, "western") else f
      expect_identical(cast_decimal(amount, reader)$value, paste0(digits, ".5"),
                       info = paste(name, amount))
    }
  }
  ml <- bridge(c(grouping = sizes(3, 2), mon_grouping = sizes(3), money))
  refused <- cast_decimal("\u20b91,234,567.50", ml)
  expect_identical(as.character(refused$detail), "group_size")
  expect_identical(c(refused$offset, refused$length), c(2L, 4L))
  # glibc's cmn_TW family groups in fours and its unm_US ends in twos: refused, not misread.
  fours <- bridge(grouping = sizes(4), mon_grouping = sizes(4))
  expect_identical(fours$grouping, "western")
  expect_identical(cast_f64("1,2345.00", fours)$offset, 1L)
  expect_identical(as.character(cast_f64("1,2345.00", fours)$detail), "group_size")
  expect_identical(cast_f64("2345.00", fours)$value, 2345)
  unm <- bridge(grouping = sizes(2, 2, 2, 3), mon_grouping = sizes(2, 2, 2, 3))
  expect_identical(unm$grouping, "western")
  expect_false(cast_f64("9,99.00", unm)$ok)
  # tok's groups of two: the bridge declares Western, which refuses them.
  tok <- bridge(grouping = sizes(2), mon_grouping = sizes(2))
  expect_identical(tok$grouping, "western")
  expect_identical(as.character(cast_i64("12,34,56,78", tok)$detail), "group_size")
  expect_false(cast_i64("12,34,56,78", num_format())$ok)
  # Platform values with no currency grouping, beside grouped numbers.
  bg <- bridge(grouping = sizes(3), mon_grouping = sizes(127), money)
  expect_identical(cast_decimal("\u20b91234567.50", bg)$value, "1234567.5")
  expect_identical(cast_decimal("1,234,567.5", bg)$value, "1234567.5")
})

test_that("a format made before the policy existed is Western, and prints so", {
  old <- num_format()
  old$grouping <- NULL
  expect_identical(capture.output(print(old)), "num_format(\".\", \",\", 95)")
  expect_false(cast_i32("1,23,456", old)$ok)
})

test_that("the linked core enforces grouping policies, and the package says so on load", {
  expect_true(.Call(getFromNamespace("C_core_guard", "hypercast")))
})

# ---- fault detail ----

test_that("every failure names what failed, by name and by the core's code", {
  v <- cast_i32(c("42", "12x", "", "99999999999", NA), num_format())
  expect_identical(as.character(v$detail), c(NA, "unexpected_character", "blank", "too_large", NA))
  expect_identical(v$detail_code, c(NA, 2L, 1L, 25L, NA))
  expect_identical(unname(fault_details[["not_a_scalar"]]), 32L)
  expect_length(fault_details, 32L)
  expect_identical(unname(fault_details), 1:32)
  f <- faults(v)
  expect_identical(as.character(f$detail), c("unexpected_character", "blank", "too_large"))
  expect_identical(f$detail_code, c(2L, 1L, 25L))
  expect_identical(as.character(cast_f64("NaN", num_format())$detail), "not_finite")
  # The same failure at two doors shares one code though the reason differs.
  inf <- c(cast_i32("inf", num_format())$detail, cast_decimal_from_double(Inf)$detail,
           cast_f64("inf", num_format())$detail)
  expect_identical(as.character(inf), rep("not_finite", 3))
  expect_identical(as.character(cast_decimal_from_double(NaN)$detail), "not_finite")
  # An empty input is optional's missing row, with no detail.
  o <- optional(cast_f64(c("1.5", "", "x"), num_format()))
  expect_identical(as.character(o$detail), c(NA, NA, "unexpected_character"))
  expect_identical(o$detail_code, c(NA, NA, 2L))
})

test_that("a checked verdict carries a detail on a fault and none on a violation", {
  age <- hypercast_constraints("u8", minimum = 18L, maximum = 130L)
  v <- cast_checked(c("42", "17", "x", ""), num_format(), age)
  expect_identical(as.character(v$detail), c(NA, NA, "unexpected_character", "blank"))
  expect_identical(v$detail_code, c(NA, NA, 2L, 1L))
  expect_identical(v$keyword[[2]], "minimum")
  expect_identical(as.character(faults(v)$detail), c("unexpected_character", "blank"))
  d <- cast_checked_from_double(c(NaN, Inf), hypercast_constraints("f64"))
  expect_identical(as.character(d$detail), c("not_finite", "not_finite"))
  t <- cast_text(list(as.raw(c(0x61, 0xc3, 0x28))), hypercast_constraints("text"))
  expect_identical(as.character(t$detail), "invalid_utf8")
})

test_that("a detail this version does not name is 'unknown', with its code and a usable fault", {
  new_verdict <- getFromNamespace("new_verdict", "hypercast")
  cols <- list(value = c(NA, NA, 1), ok = c(FALSE, FALSE, TRUE), reason = c(2L, 3L, NA),
               offset = c(1L, 0L, NA), length = c(2L, 3L, NA), detail = c(999L, 11L, NA))
  v <- new_verdict(cols, "f64")
  expect_identical(as.character(v$detail), c("unknown", "group_size", NA))
  expect_identical(v$detail_code, c(999L, 11L, NA))
  expect_identical(as.character(v$reason), c("malformed", "out_of_range", NA))
  expect_identical(c(v$offset[[1]], v$length[[1]]), c(1L, 2L))
  f <- faults(v)
  expect_identical(as.character(f$detail), c("unknown", "group_size"))
  expect_identical(f$detail_code, c(999L, 11L))
  expect_identical(as.character(optional(v)$detail), c("unknown", "group_size", NA))
  # Branching on it needs a default arm, as a keyword does.
  describe <- function(d) switch(as.character(d), group_size = "grouping", "other")
  expect_identical(describe(v$detail[[1]]), "other")
})

test_that("1.0 code reads a verdict exactly as it always did", {
  # Constructors as 1.0 called them, positionally.
  f <- num_format(",", ".", c("grouping", "currency"), "kr.", "", "", "\u2212")
  expect_identical(cast_decimal("\u22121.234,5 kr.", f)$value, "-1234.5")
  v <- cast_i32(c("42", "(1,234)", "12x", "", NA), num_format())
  # By name and by position: the five 1.0 columns keep their places.
  expect_identical(names(v)[1:5], c("value", "ok", "reason", "offset", "length"))
  expect_identical(v[[1]], v$value)
  expect_identical(v[[5]], v$length)
  expect_identical(v$value, c(42, -1234, NA, NA, NA))
  expect_identical(as.character(v$reason), c(NA, NA, "malformed", "empty", NA))
  expect_identical(v$offset, c(NA, NA, 2L, 0L, NA))
  one <- as.data.frame(v)[c("value", "ok", "reason", "offset", "length")]
  expect_identical(nrow(one), 5L)
  expect_identical(nrow(rbind(v, v)), 10L)
  expect_identical(v[2:3, ]$ok, c(TRUE, FALSE))
  expect_identical(identical(v, cast_i32(c("42", "(1,234)", "12x", "", NA), num_format())), TRUE)
  expect_identical(cast_datetime("7.1.2026 3:04 nachm.", "day_month_year", "vorm.", "nachm.")$ok,
                   TRUE)
  expect_identical(cast_date("1/7/2026", "month_day_year")$value, as.Date("2026-01-07"))
  expect_identical(cast_timestamp("2026-01-07T15:04:05Z")$ok, TRUE)
  expect_identical(is_ok(v), v$ok)
  expect_identical(faults(v)[c("row", "reason", "offset", "length")]$row, 3:4)
})
