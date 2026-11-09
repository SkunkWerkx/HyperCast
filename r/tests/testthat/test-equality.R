# A declaration is identical() to one that declares the same thing, however it was built, as
# the Rust core's == holds.
test_that("a format is identical to one declaring the same thing", {
  invariant <- num_format()
  expect_identical(num_format(".", ",", num_styles[["all"]]), invariant)
  expect_identical(num_format(flags = as.numeric(num_styles[["all"]])), invariant)
  expect_identical(num_format_with_grouping(num_format_with_grouping(invariant, "indian"),
                                            "western"), invariant)
  expect_false(identical(num_format_with_grouping(invariant, "indian"), invariant))
  expect_false(identical(num_format(currency = "$"), invariant))
  expect_false(identical(num_format_detect(), invariant))
})

test_that("date options and a vocabulary are identical to ones declaring the same thing", {
  expect_identical(date_options(pivot = 2050), date_options(0L, 2050L))
  expect_identical(date_options("compact_date"), date_options(1))
  expect_false(identical(date_options(pivot = 2050), date_options(pivot = 2049)))
  months <- function(first) {
    month_names(stats::setNames(1:12, c(first, paste0("m", letters[2:12]))))
  }
  french <- months("janvier")
  expect_identical(month_names_packed(french$packed), french)
  expect_false(identical(months("january"), french))
})
