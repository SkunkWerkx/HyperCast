test_that("a verdict is a data frame of one row per element, in order", {
  v <- cast_i32(c("42", "(1,234)", "12x", "", NA), num_format())
  expect_s3_class(v, c("hypercast_verdict", "data.frame"), exact = TRUE)
  expect_named(v, c("value", "ok", "reason", "offset", "length"))
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
  expect_named(v, c("value", "seconds", "nanos", "ok", "reason", "offset", "length"))
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

test_that("U+0000 is out of range, the way a UTF-16 binding reports a scalar it cannot hold", {
  v <- cast_char(c(" U+0000 ", "&#0;", "00"))
  expect_identical(as.character(v$reason), rep("out_of_range", 3))
  expect_identical(v$offset, c(1L, 0L, 0L))
  expect_identical(v$length, c(6L, 4L, 2L))
  expect_identical(cast_char("0")$value, "0")
  raw_nul <- cast_char(list(as.raw(0L)))
  expect_identical(c(raw_nul$offset, raw_nul$length), c(0L, 1L))
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
  # print() writes the minus sign as the session's locale can (a "<U+2212>" escape in C).
  expect_identical(capture.output(print(f)), capture.output(cat(
    "num_format(\",\", \".\", 65, currency = \"kr.\", minus_sign = \"\u2212\")\n"
  )))
  expect_identical(cast_decimal("\u22121.234,5 kr.", f)$value, "-1234.5")
  expect_identical(num_format_detect()$flags, 127L)
  expect_error(num_format(".", "."), "must differ")
  expect_error(num_format("1", ","), "no ASCII digit")
  expect_error(num_format("123456789", ","), "1 to 8 UTF-8 bytes")
  expect_error(num_format(currency_decimal_sep = ","), "Currency decimal and group")
  expect_error(num_format(minus_sign = "- "), "whitespace")
  expect_error(num_format(currency = "12"), "Currency symbol")
  expect_error(num_format(digit_zero = "0"), "Digit zero")
  expect_error(num_format(digit_zero = "\u0679"), "Digit zero")
  expect_identical(num_format(digit_zero = "\u0660")$digit_zero, "\u0660")
  expect_error(num_format(flags = "lenient"), "no style")
  expect_error(num_format(flags = -1), "flags")
  expect_error(num_format(NA, ","), "single string")
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
  expect_error(cast_datetime("1/7/2026 3 PM", "month_day_year", am = " am"), "Day period am")
  expect_error(cast_datetime("1/7/2026 3 PM", "month_day_year", pm = "p1"), "Day period pm")
  expect_error(cast_bool(42), "character vector or a list of raw vectors")
  expect_error(cast_bool(list("yes")), "character vector or a list of raw vectors")
  expect_error(cast_excel_time("0.5"), "numeric vector")
})

test_that("the native version is the package version", {
  expect_s3_class(hypercast_native_version(), "package_version")
  expect_equal(hypercast_native_version(), packageVersion("hypercast"))
})
