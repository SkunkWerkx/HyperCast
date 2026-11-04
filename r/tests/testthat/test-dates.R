test_that("date options declare the opt-in shapes, and nothing by default", {
  expect_identical(date_options()$flags, 0L)
  expect_identical(date_options()$pivot, 0L)
  expect_identical(date_flags, c(compact_date = 1L, space_separator = 2L, hour_offset = 4L,
                                 common_forms = 7L))
  expect_identical(date_options("common_forms")$flags, 7L)
  expect_identical(date_options(c("compact_date", "hour_offset"), 2029)$flags, 5L)
  expect_identical(date_options(3)$flags, 3L)
  expect_identical(date_options()$packed, as.raw(c(12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)))
  expect_identical(date_options("common_forms", 2029)$packed,
                   as.raw(c(12, 0, 0, 0, 7, 0, 0, 0, 0xed, 0x07, 0, 0)))
  expect_error(date_options(8), "date options in date_flags")
  expect_error(date_options("lenient"), "date options in date_flags")
  expect_error(date_options(pivot = 99), "a year from 100 to 9999")
  expect_error(date_options(pivot = 10000), "a year from 100 to 9999")
  expect_error(date_options(pivot = 1.5), "whole number")
  expect_identical(capture.output(print(date_options("common_forms", 2029))),
                   "date_options(\"common_forms\", pivot = 2029L)")
  expect_error(cast_date("x", options = 7), "hypercast_date_options")
  expect_error(cast_date("x", "year_month_day", months = 7), "hypercast_month_names")
})

test_that("each opt-in shape is refused until asked for", {
  expect_identical(cast_date("20260107")$ok, FALSE)
  expect_identical(as.character(cast_date("20260107")$detail), "wrong_length")
  expect_identical(cast_date("20260107", options = date_options("compact_date"))$value,
                   as.Date("2026-01-07"))
  expect_identical(cast_date("20260107", options = date_options("common_forms"))$ok, TRUE)
  ts <- c("2026-01-07 15:04:05Z", "2026-01-07T15:04:05+00", "2026-01-07T15:04:05-05")
  expect_identical(cast_timestamp(ts)$ok, c(FALSE, FALSE, FALSE))
  expect_identical(cast_timestamp(ts, date_options("space_separator"))$ok, c(TRUE, FALSE, FALSE))
  expect_identical(cast_timestamp(ts, date_options("hour_offset"))$ok, c(FALSE, TRUE, TRUE))
  expect_identical(cast_timestamp(ts, date_options("common_forms"))$ok, c(TRUE, TRUE, TRUE))
  expect_identical(cast_date("1/7/26", "month_day_year")$ok, FALSE)
  expect_identical(cast_date("1/7/26", "month_day_year", date_options(pivot = 2029))$value,
                   as.Date("2026-01-07"))
  expect_identical(cast_datetime("1/7/26 3 PM", "month_day_year",
                                 options = date_options(pivot = 2029))$value,
                   "2026-01-07 15:00:00")
})

test_that("a month vocabulary reads names in the month field, and refuses what cannot be", {
  en <- c(Jan = 1, Feb = 2, Mar = 3, Apr = 4, May = 5, Jun = 6, Jul = 7, Aug = 8, Sep = 9,
          Sept = 9, Oct = 10, Nov = 11, Dec = 12)
  months <- month_names(en)
  expect_s3_class(months, "hypercast_month_names")
  expect_identical(length(months$entries), 13L)
  # Each entry: a month byte, a little-endian u32 length, the bytes.
  expect_identical(months$packed[1:8], as.raw(c(1, 3, 0, 0, 0, 0x4a, 0x61, 0x6e)))
  v <- cast_date(c("7-Jan-2026", "07-jan-26", "7-Sept-2026"), "day_month_year", months = months)
  expect_identical(v$ok, c(TRUE, FALSE, TRUE))
  expect_identical(as.character(v$detail[[2]]), "field_width")
  p <- date_options(pivot = 2029)
  expect_identical(cast_date("07-jan-26", "day_month_year", p, months)$value, as.Date("2026-01-07"))
  expect_identical(cast_datetime("7-Jan-2026 3:04 PM", "day_month_year", months = months)$value,
                   "2026-01-07 15:04:00")
  # Without order there is no ordered door to read it: a mistake, not an ignored option.
  expect_error(cast_date("2026-01-07", months = months), "months needs an order")
  expect_error(cast_date("2026-01-07", months = 7), "months needs an order")
  expect_error(month_names(c(Jan = 1)), "have no name for month")
  expect_error(month_names(c(en, Jan = 2)), "fold to the same spelling")
  expect_error(month_names(c(en, JAN = 2)), "fold to the same spelling")
  expect_error(month_names(c(en, Xyz = 13)), "Month must be 1 to 12")
  expect_error(month_names(c(en, "J/n" = 1)), "must not hold an ASCII digit")
  expect_error(month_names(c(en, "J n" = 1)), "must not hold an ASCII digit")
  expect_error(month_names(stats::setNames(c(en, 1), c(names(en), ""))), "1 to 64 bytes long")
  expect_error(month_names(1:12), "named whole-number vector")
})

test_that("a dotted month name, the dot separator and a pivot together", {
  names12 <- c("janv." = 1, "fevr." = 2, mars = 3, "avr." = 4, mai = 5, juin = 6, "juil." = 7,
               aout = 8, "sept." = 9, "oct." = 10, "nov." = 11, "dec." = 12)
  months <- month_names(names12)
  v <- cast_date(c("7.janv..26", "7.janv.26"), "day_month_year", date_options(pivot = 2029), months)
  expect_identical(v$ok, c(TRUE, FALSE))
  expect_identical(v$value[[1]], as.Date("2026-01-07"))
  expect_identical(as.character(v$reason[[2]]), "malformed")
})

test_that("a vocabulary from packed bytes is judged by the same rules", {
  packed <- getFromNamespace("month_names_packed", "hypercast")
  en <- month_names(c(Jan = 1, Feb = 2, Mar = 3, Apr = 4, May = 5, Jun = 6, Jul = 7, Aug = 8,
                      Sep = 9, Oct = 10, Nov = 11, Dec = 12))
  expect_identical(packed(en$packed)$packed, en$packed)
  expect_error(packed(c(en$packed, as.raw(c(3, 5, 0, 0, 0, 97, 98)))), "cut short")
  expect_error(packed(c(en$packed, as.raw(7))), "cut short")
  expect_error(packed(c(en$packed, as.raw(c(1, 2, 0, 0, 0, 195, 40)))), "valid UTF-8")
})
