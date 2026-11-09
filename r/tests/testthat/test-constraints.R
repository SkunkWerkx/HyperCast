# The validation constraints around the corpus replay: the README's examples, the
# declaration's argument types and refusals, the door it keeps, the checked verdict's helpers
# and the text door's values.

bits <- function(x) writeBin(as.double(x), raw())

test_that("the README's examples: latitude, euro cash, a sex code", {
  latitude <- hypercast_constraints("f64", minimum = "-90", maximum = "90",
                                    multiple_of = "0.000001", rounding = "midpoint")
  v <- cast_checked(c("45.1234567", "91", "north", ""), num_format(), latitude)
  expect_identical(v$value, c(45.123457, NA, NA, NA))
  expect_identical(v$adjusted, c(TRUE, NA, NA, NA))
  expect_identical(v$keyword, c(NA, "maximum", NA, NA))
  expect_identical(as.character(v$reason), c(NA, NA, "malformed", "empty"))
  expect_identical(cast_checked_from_double(45.1234567, latitude)$value, 45.123457)

  cash <- hypercast_constraints("decimal", multiple_of = "0.05")
  euro <- num_format(",", ".", currency = "\u20ac")
  v <- cast_checked(c("12,35 \u20ac", "12,37 \u20ac", "1.234,50"), euro, cash)
  expect_identical(v$value, c("12.35", NA, "1234.5"))
  expect_identical(v$keyword, c(NA, "multipleOf", NA))
  expect_identical(c(v$offset[[2]], v$length[[2]]), c(0L, 7L))

  sex <- hypercast_constraints("text", enum = c("Female", "Male", "Other"), trim = TRUE)
  v <- cast_text(c(" male ", "OTHER", "Mail", "   "), sex)
  expect_identical(v$value, c("Male", "Other", NA, NA))
  expect_identical(v$enum_index, c(2L, 3L, NA, NA))
  expect_identical(v$keyword, c(NA, NA, "enum", NA))
  expect_identical(as.character(v$reason), c(NA, NA, NA, "empty"))
})

test_that("a refused declaration is an error worded from the core's code", {
  refused <- function(expr) {
    tryCatch({
      expr
      "no error"
    }, error = conditionMessage)
  }
  held <- "a bound, step or enum entry the door cannot hold"
  expect_identical(refused(hypercast_constraints("i32", min_length = 1)),
                   "Constraints refused (code 1): a constraint the door does not take.")
  expect_identical(refused(hypercast_constraints("i32", minimum = "5", maximum = "5")),
                   "Constraints refused (code 4): the lower bound is not below the upper bound.")
  expect_identical(refused(hypercast_constraints("u8", minimum = -1L)),
                   paste0("Constraints refused (code 8): ", held, "."))
  # An entry that is not UTF-8 reaches the core as its bytes, and the core refuses it.
  # Marked, or a Latin-1 session would read these bytes as "a" and "y with diaeresis".
  invalid <- rawToChar(as.raw(c(0x61, 0xff)))
  Encoding(invalid) <- "UTF-8"
  expect_identical(refused(hypercast_constraints("text", enum = invalid)),
                   "Constraints refused (code 9): an enum entry that is not UTF-8.")
  expect_identical(refused(hypercast_constraints("text", enum = c("Male", "MALE"))),
                   "Constraints refused (code 10): a duplicate enum entry.")
  message <- getFromNamespace("refusal_message", "hypercast")
  expect_identical(message(11L), "Constraints refused (code 11): a malformed constraints layout.")
  expect_identical(message(12L), "Constraints refused (code 12): an enum with no entries.")
  expect_identical(message(14L),
                   "Constraints refused (code 14): a rule this binding does not name.")
  # NULL declares no enum; an empty one is declared, and refused, at every door that takes
  # one (and one that takes none refuses the keyword first).
  empty <- "Constraints refused (code 12): an enum with no entries."
  expect_identical(refused(hypercast_constraints("text", enum = character(0))), empty)
  expect_identical(refused(hypercast_constraints("i32", enum = integer(0))), empty)
  expect_identical(refused(hypercast_constraints("i32", enum = character(0))), empty)
  expect_identical(refused(hypercast_constraints("f64", enum = character(0))),
                   "Constraints refused (code 1): a constraint the door does not take.")
  expect_s3_class(hypercast_constraints("text", enum = NULL), "hypercast_constraints")
  unmatchable <- "Constraints refused (code 13): a text enum entry no input can match."
  expect_identical(refused(hypercast_constraints("text", enum = c("Male", ""))), unmatchable)
  expect_identical(refused(hypercast_constraints("text", enum = " Male ", trim = TRUE)),
                   unmatchable)
  # Untrimmed, edge white space is part of the entry, and matches as declared.
  spaced <- cast_text(" MALE ", hypercast_constraints("text", enum = " Male "))
  expect_identical(spaced$value, " Male ")
  expect_identical(spaced$enum_index, 1L)
})

test_that("bounds and steps are exact: strings or R integers, never doubles", {
  expect_error(hypercast_constraints("f64", maximum = 90), "never a double")
  expect_error(hypercast_constraints("f64", multiple_of = 0.1), "never a double")
  expect_error(hypercast_constraints("i32", minimum = NA_character_), "minimum must be")
  expect_error(hypercast_constraints("i32", minimum = NA_integer_), "minimum must be")
  expect_error(hypercast_constraints("i32", minimum = c("1", "2")), "minimum must be")
  expect_error(hypercast_constraints("i32", minimum = TRUE), "minimum must be")
  expect_error(hypercast_constraints("i32", minimum = factor("1")), "minimum must be")
  ints <- hypercast_constraints("i32", minimum = -5L, maximum = 5L)
  expect_identical(cast_checked(c("-5", "-6", "5"), num_format(), ints)$keyword,
                   c(NA, "minimum", NA))
  big <- hypercast_constraints("u64", maximum = "18446744073709551615", multiple_of = "5")
  expect_identical(cast_checked("18446744073709551615", num_format(), big)$value,
                   "18446744073709551615")
  expect_error(hypercast_constraints("i32", enum = c(1, 2)), "never a double")
  expect_error(hypercast_constraints("i32", enum = c(1L, NA)), "no NA")
  expect_error(hypercast_constraints("text", enum = 1:2), "character vector")
  tens <- hypercast_constraints("u8", enum = c(10L, 20L))
  expect_identical(cast_checked("0x14", num_format(), tens)$value, 20L)
})

test_that("lengths are whole numbers, never logicals or strings; trim is TRUE or FALSE", {
  expect_error(hypercast_constraints("text", min_length = TRUE), "min_length must be")
  expect_error(hypercast_constraints("text", max_length = "2"), "max_length must be")
  expect_error(hypercast_constraints("text", max_length = -1), "max_length must be")
  expect_error(hypercast_constraints("text", max_length = 2.5), "max_length must be")
  expect_error(hypercast_constraints("text", max_length = 2^32), "max_length must be")
  expect_error(hypercast_constraints("text", trim = NA), "trim must be")
  expect_error(hypercast_constraints("text", rounding = "half_up"), "rounding must be one of")
  expect_error(hypercast_constraints("bool"), "door must be one of")
  two <- hypercast_constraints("text", min_length = 2, max_length = 2L)
  expect_identical(cast_text(c("US", "USA"), two)$keyword, c(NA, "maxLength"))
})

test_that("the door is kept: a declaration for another door, or none, is an error", {
  f64 <- hypercast_constraints("f64", maximum = "1")
  text <- hypercast_constraints("text", max_length = 1)
  expect_error(cast_text("x", f64), "text door's constraints; these are for \"f64\"")
  expect_error(cast_checked("1", num_format(), text), "go to cast_text")
  expect_error(cast_checked_from_double(1, text), "go to cast_text")
  # An explicit NULL is not "no rules": only a missing argument is the plain door.
  expect_error(cast_text("x", NULL), "must be a hypercast_constraints")
  expect_error(cast_checked("1", num_format(), NULL), "must be a hypercast_constraints")
  expect_error(cast_checked("1", num_format(), list(door = "f64")), "hypercast_constraints")
  expect_error(cast_checked("1", NULL, f64), "hypercast_num_format")
  expect_s3_class(cast_text("x"), "hypercast_verdict")
  # A wrapper forwarding its own missing `constraints` is not the plain door either.
  forward <- function(x, constraints) cast_text(x, constraints)
  expect_error(forward("x"), "constraints")
  # A declaration edited into one the package would not accept is refused at the cast.
  edited <- f64
  edited$maximum <- 1
  expect_error(cast_checked("1", num_format(), edited), "never a double")
})

test_that("a text enum match is the caller's own declared string", {
  cafe <- "Caf\xe9"
  Encoding(cafe) <- "latin1"
  v <- cast_text(c("CAF\u00c9", "caf\u00e9 "), hypercast_constraints("text", enum = cafe))
  expect_identical(v$value[[1]], cafe)
  expect_identical(Encoding(v$value[[1]]), "latin1")
  expect_identical(v$enum_index, c(1L, NA))
  expect_identical(v$keyword, c(NA, "enum"))
  # The Turkish dotless i folds to nothing else, and "I" folds to "i".
  v <- cast_text(c("i", "\u0131", "\u0130"), hypercast_constraints("text", enum = c("I", "\u0131")))
  expect_identical(v$value, c("I", "\u0131", NA))
})

test_that("the text door's value is the caller's string, or the trimmed span of it", {
  latin1 <- "\xe9t\xe9"
  Encoding(latin1) <- "latin1"
  trim <- hypercast_constraints("text", trim = TRUE)
  plain <- cast_text(c(latin1, " a "))
  expect_identical(Encoding(plain$value[[1]]), "latin1")
  expect_identical(plain$value[[2]], " a ")
  trimmed <- cast_text(c("\u3000 \u00e9t\u00e9\u00a0", " a "), trim)
  expect_identical(trimmed$value, c("\u00e9t\u00e9", "a"))
  expect_identical(Encoding(trimmed$value[[1]]), "UTF-8")
  expect_identical(cast_text(list(charToRaw("\u00e9")))$value, "\u00e9")
  # Spans count characters for a string, bytes for raw.
  long <- hypercast_constraints("text", max_length = 1, trim = TRUE)
  v <- cast_text(c(" \u00e9\u00e9 ", NA), long)
  expect_identical(c(v$offset[[1]], v$length[[1]]), c(1L, 2L))
  expect_identical(v$ok, c(FALSE, NA))
  r <- cast_text(list(charToRaw(" \u00e9\u00e9 ")), long)
  expect_identical(c(r$offset, r$length), c(1L, 4L))
  # U+0000 comes through a raw vector and no R string can hold it: the core accepted the value,
  # so it is `ok` with NA, and raw = TRUE gives the trimmed span's bytes.
  spaced <- list(as.raw(c(0x20, 0x61, 0x00, 0x20)))
  nul <- cast_text(spaced, trim)
  expect_identical(c(nul$ok, is.na(nul$value)), c(TRUE, TRUE))
  expect_identical(nul$adjusted, FALSE)
  expect_true(is.na(nul$reason))
  expect_true(is.na(nul$keyword))
  expect_identical(c(nul$offset, nul$length), c(NA_integer_, NA_integer_))
  expect_identical(cast_text(spaced, trim, raw = TRUE)$value[[1]], as.raw(c(0x61, 0x00)))
  lone <- cast_text(list(as.raw(0L)))
  expect_identical(c(lone$ok, is.na(lone$value)), c(TRUE, TRUE))
  # The rules still judge the value R can't hold: the core rejects it, and that's a violation.
  short <- cast_text(list(as.raw(c(0x61, 0x00, 0x62))),
                     hypercast_constraints("text", max_length = 2))
  expect_identical(c(short$ok, short$keyword), c(FALSE, "maxLength"))
  # Invalid UTF-8 is malformed at its own bytes.
  bad <- cast_text(list(as.raw(c(0x61, 0xff))))
  expect_identical(c(as.character(bad$reason), bad$offset, bad$length), c("malformed", "1", "1"))
})

test_that("text that is not UTF-8 is the same fault at the plain doors as at the checked", {
  # An R string marked UTF-8 can hold bytes that are not (a lone surrogate's three, a stray
  # byte); every door reads them as they are, so the plain fault is the checked one, span and
  # all, and never a replacement standing for them.
  marked <- function(b) {
    s <- rawToChar(as.raw(b))
    Encoding(s) <- "UTF-8"
    s
  }
  texts <- c(marked(c(0xed, 0xa0, 0x80, 0x35)), marked(c(0x35, 0xed, 0xbf, 0xbf)),
             marked(c(0x31, 0x32, 0xff, 0x33)), marked(c(0xef, 0xbf, 0xbd, 0x35, 0xc0)))
  f <- num_format(currency = "\ufffd")
  plain <- cast_i32(texts, f)
  checked <- cast_checked(texts, f, hypercast_constraints("i32"))
  expect_identical(as.character(plain$reason), rep("malformed", 4L))
  expect_identical(as.character(plain$reason), as.character(checked$reason))
  expect_identical(plain$offset, checked$offset)
  expect_identical(plain$length, checked$length)
  # The same bytes from a raw vector, counted in bytes.
  raw_plain <- cast_i32(list(as.raw(c(0xed, 0xa0, 0x80, 0x35))), f)
  expect_identical(c(raw_plain$offset, raw_plain$length), c(0L, 3L))
  text <- cast_text(marked(c(0x61, 0xff, 0x62)))
  expect_identical(c(as.character(text$reason), text$offset, text$length), c("malformed", "1", "1"))
  expect_identical(text$offset,
                   cast_text(marked(c(0x61, 0xff, 0x62)), hypercast_constraints("text"))$offset)
})

test_that("each checked door's value comes back in the plain door's R type", {
  f <- num_format()
  expect_identical(cast_checked("7", f, hypercast_constraints("i8", maximum = 9L))$value, 7L)
  expect_identical(cast_checked("7", f, hypercast_constraints("u32", maximum = 9L))$value, 7)
  expect_identical(cast_checked("-9223372036854775808", f,
                                hypercast_constraints("i64", maximum = 9L))$value,
                   "-9223372036854775808")
  f32 <- cast_checked("0.1", f, hypercast_constraints("f32", maximum = 1L))$value
  expect_identical(f32, readBin(writeBin(0.1, raw(), size = 4L), "double", size = 4L))
  expect_identical(cast_checked("1.10", f, hypercast_constraints("decimal", maximum = 9L))$value,
                   "1.1")
  # Zero has one form: a decoded -0 and a value rounded to zero both come back +0.
  zero <- hypercast_constraints("f64", multiple_of = "0.000001", rounding = "midpoint")
  got <- cast_checked_from_double(c(-0, -4e-7), zero)$value
  expect_identical(lapply(got, bits), list(bits(0), bits(0)))
  v <- cast_checked_from_double(c(NaN, NA, Inf), zero)
  expect_identical(as.character(v$reason), c("malformed", NA, "out_of_range"))
  expect_identical(v$offset, c(0L, NA, 0L))
})

test_that("faults(), violations(), optional() and is_ok() keep the two kinds apart", {
  age <- hypercast_constraints("u8", minimum = 18L, maximum = 130L)
  v <- cast_checked(c("42", "17", "x", "", NA), num_format(), age)
  expect_identical(is_ok(v), c(TRUE, FALSE, FALSE, FALSE, NA))
  f <- faults(v)
  expect_identical(f$row, 3:4)
  expect_identical(as.character(f$reason), c("malformed", "empty"))
  w <- violations(v)
  expect_identical(w$row, 2L)
  expect_identical(w$keyword, "minimum")
  expect_identical(c(w$offset, w$length), c(0L, 2L))
  o <- optional(v)
  expect_s3_class(o, "hypercast_checked_verdict")
  expect_identical(o$ok, c(TRUE, FALSE, FALSE, NA, NA))
  expect_identical(o$keyword, v$keyword)
  expect_identical(o$offset, c(NA, 0L, 0L, NA, NA))
  out <- capture.output(print(v))
  expect_identical(out[[1]],
                   "<hypercast_checked_verdict: u8> 1 ok, 2 faulted, 1 violated, 1 missing")
  expect_error(violations(cast_u8("1", num_format())), "hypercast_checked_verdict")
  expect_error(faults(data.frame(ok = TRUE)), "hypercast_verdict or a hypercast_checked_verdict")
  expect_error(optional(1), "hypercast_verdict or a hypercast_checked_verdict")
  expect_identical(nrow(rbind(v, v)), 10L)
  expect_identical(nrow(cast_text(character(), hypercast_constraints("text"))), 0L)
})

test_that("a keyword this version does not know reads as its code", {
  name <- getFromNamespace("keyword_name", "hypercast")
  expect_identical(name(c(1L, 2L, 3L, 4L, 5L, 6L, 7L, 8L, 9L, 12L, NA)),
                   c("minimum", "exclusiveMinimum", "maximum", "exclusiveMaximum", "multipleOf",
                     "minLength", "maxLength", "enum", "keyword 9", "keyword 12", NA))
  # Branching on it needs a default arm.
  describe <- function(k) switch(k, minimum = "too small", maximum = "too large", "other rule")
  expect_identical(describe(name(12L)), "other rule")
})

test_that("a declaration prints as the call that declares it", {
  sex <- hypercast_constraints("text", enum = c("Female", "Male"), trim = TRUE)
  expect_identical(capture.output(print(sex)),
                   "hypercast_constraints(\"text\", trim = TRUE, enum = c(\"Female\", \"Male\"))")
  expect_identical(capture.output(print(hypercast_constraints("i32", minimum = 1L))),
                   "hypercast_constraints(\"i32\", minimum = 1L)")
})

test_that("cast_text(raw = TRUE) is the same verdict, with the bytes, through constraints", {
  rules <- hypercast_constraints("text", trim = TRUE, max_length = 3,
                                 enum = c("\u00e9t\u00e9", "ab"))
  x <- list(charToRaw(" \u00c9T\u00c9 "), charToRaw("toolong"), charToRaw("c"), NULL,
            as.raw(c(0x61, 0xff)))
  plain <- cast_text(x, rules)
  v <- cast_text(x, rules, raw = TRUE)
  expect_s3_class(v, "hypercast_checked_verdict")
  expect_identical(plain[names(plain) != "value"], v[names(v) != "value"])
  # An enum match is the entry as declared, in UTF-8.
  expect_identical(v$value[[1]], charToRaw("\u00e9t\u00e9"))
  for (row in 2:5) expect_null(v$value[[row]])
})

test_that("text past INT_MAX bytes is ok with NA, and raw gives the bytes", {
  # Allocates over 6 GiB, so it runs only when asked for: HYPERCAST_TEST_LARGE=1.
  skip_if_not(nzchar(Sys.getenv("HYPERCAST_TEST_LARGE")), "HYPERCAST_TEST_LARGE is not set")
  over <- rep(as.raw(0x61), 2^31)
  v <- cast_text(list(over))
  expect_identical(c(v$ok, is.na(v$value)), c(TRUE, TRUE))
  expect_true(is.na(v$reason))
  exact <- cast_text(list(over), raw = TRUE)$value[[1]]
  expect_identical(length(exact), 2^31)
  expect_identical(exact[c(1, 2^31)], as.raw(c(0x61, 0x61)))
  rm(exact)
  rules <- hypercast_constraints("text", trim = TRUE)
  checked <- cast_text(list(c(as.raw(0x20), over)), rules)
  expect_identical(c(checked$ok, is.na(checked$value)), c(TRUE, TRUE))
  # INT_MAX itself is the longest text R can hold, and it holds it.
  rm(over)
  most <- rep(as.raw(0x61), .Machine$integer.max)
  at <- cast_text(list(most))
  expect_identical(at$ok, TRUE)
  expect_identical(nchar(at$value, type = "bytes"), .Machine$integer.max)
})

test_that("cast_text(raw = TRUE) gives a bytes-marked enum entry's bytes, as the core matched", {
  marked <- "\xc3\xa9"
  Encoding(marked) <- "bytes"
  rules <- hypercast_constraints("text", enum = marked)
  input <- list(as.raw(c(0xc3, 0xa9)))
  expect_identical(cast_text(input, rules)$ok, TRUE)
  v <- cast_text(input, rules, raw = TRUE)
  expect_identical(c(v$ok, v$enum_index), c(1L, 1L))
  expect_identical(v$value[[1]], as.raw(c(0xc3, 0xa9)))
})

test_that("cast_text(raw = TRUE) in a C session reads an unmarked enum entry as its bytes", {
  old <- Sys.getlocale("LC_CTYPE")
  on.exit(Sys.setlocale("LC_CTYPE", old), add = TRUE)
  skip_if(suppressWarnings(Sys.setlocale("LC_CTYPE", "C")) == "", "no C locale")
  skip_if(isTRUE(l10n_info()[["UTF-8"]]), "the C locale is UTF-8 here")
  expect_identical(getFromNamespace("session_native", "hypercast")(), 1L)
  # Translated, this entry would read as the text "<c3><a9>".
  rules <- hypercast_constraints("text", enum = rawToChar(as.raw(c(0xc3, 0xa9))))
  v <- cast_text(list(as.raw(c(0xc3, 0xa9))), rules, raw = TRUE)
  expect_identical(c(v$ok, v$enum_index), c(1L, 1L))
  expect_identical(v$value[[1]], as.raw(c(0xc3, 0xa9)))
})
