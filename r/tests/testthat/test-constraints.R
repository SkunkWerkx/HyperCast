# The validation constraints around the corpus replay: the README's examples, the
# declaration's argument types and refusals, the door it keeps, the checked verdict's helpers
# and the text door's values.

bits <- function(x) writeBin(as.double(x), raw())

test_that("the README's examples: latitude, euro cash, a sex code", {
  latitude <- hypercast_constraints("f64", minimum = "-90", maximum = "90",
                                    multiple_of = "0.000001", rounding = "midpoint")
  v <- cast_checked(c("45.1234567", "91", "north", ""), num_format(), latitude)
  expect_identical(v$value, c(45.123457, 91, NA, NA))
  expect_identical(v$adjusted, c(TRUE, NA, NA, NA))
  expect_identical(v$keyword, c(NA, "maximum", NA, NA))
  expect_identical(as.character(v$reason), c(NA, NA, "malformed", "empty"))
  expect_identical(cast_checked_from_double(45.1234567, latitude)$value, 45.123457)

  cash <- hypercast_constraints("decimal", multiple_of = "0.05")
  euro <- num_format(",", ".", currency = "\u20ac")
  v <- cast_checked(c("12,35 \u20ac", "12,37 \u20ac", "1.234,50"), euro, cash)
  expect_identical(v$value, c("12.35", "12.37", "1234.5"))
  expect_identical(v$keyword, c(NA, "multipleOf", NA))
  expect_identical(c(v$offset[[2]], v$length[[2]]), c(0L, 7L))

  sex <- hypercast_constraints("text", enum = c("Female", "Male", "Other"), trim = TRUE)
  v <- cast_text(c(" male ", "OTHER", "Mail", "   "), sex)
  expect_identical(v$value, c("Male", "Other", "Mail", NA))
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
  not_taken <- "Constraints refused (code 1): a constraint the door does not take."
  held <- "a bound, step or enum entry the door cannot hold"
  expect_identical(refused(hypercast_constraints("i32", trim = TRUE)), not_taken)
  expect_identical(refused(hypercast_constraints("f64", case_sensitive = TRUE)), not_taken)
  expect_identical(refused(hypercast_constraints("text", multiple_of = "1",
                                                 rounding = "floor")), not_taken)
  expect_identical(refused(hypercast_constraints("i32", multiple_of = "0")),
                   "Constraints refused (code 5): multipleOf is not positive.")
  expect_identical(refused(hypercast_constraints("i32", rounding = "floor")),
                   "Constraints refused (code 6): a rounding without multipleOf.")
  expect_identical(refused(hypercast_constraints("decimal", maximum = "1e-29")),
                   paste0("Constraints refused (code 8): ", held, "."))
  # An entry that is not UTF-8 reaches the core as its bytes, and the core refuses it.
  # Marked, or a Latin-1 session would read these bytes as "a" and "y with diaeresis".
  invalid <- rawToChar(as.raw(c(0x61, 0xff)))
  Encoding(invalid) <- "UTF-8"
  expect_identical(refused(hypercast_constraints("text", enum = invalid)),
                   "Constraints refused (code 9): an enum entry that is not UTF-8.")
  message <- getFromNamespace("refusal_message", "hypercast")
  expect_identical(message(11L), "Constraints refused (code 11): a malformed constraints layout.")
  # The retired codes name no rule.
  for (code in c(2L, 3L, 4L, 7L, 10L, 12L, 13L, 14L)) {
    want <- sprintf("Constraints refused (code %d): a rule this binding does not name.", code)
    expect_identical(message(code), want)
  }
})

test_that("every declaration OpenAPI allows is accepted, as it means there", {
  # A keyword for another type is ignored.
  ignored <- hypercast_constraints("i32", min_length = 1, max_length = 0)
  expect_identical(cast_checked("12", num_format(), ignored)$value, 12)
  expect_identical(cast_text("abc", hypercast_constraints("text", minimum = "5"))$ok, TRUE)
  # The four bounds are independent, and unsatisfiable bounds make every value a violation.
  both <- hypercast_constraints("i32", minimum = "0", exclusive_minimum = "5", maximum = "10",
                                exclusive_maximum = "8")
  v <- cast_checked(c("-1", "5", "6", "8", "11"), num_format(), both)
  expect_identical(v$keyword, c("minimum", "exclusiveMinimum", NA, "exclusiveMaximum",
                                "maximum"))
  expect_identical(v$value, c(-1, 5, 6, 8, 11))
  crossed <- hypercast_constraints("i32", minimum = "5", maximum = "5")
  expect_identical(cast_checked(c("5", "6"), num_format(), crossed)$keyword, c(NA, "maximum"))
  # NULL declares no enum; an empty one is declared, and no value matches it.
  for (door in c("i32", "f64", "decimal")) {
    v <- cast_checked("7", num_format(), hypercast_constraints(door, enum = character(0)))
    expect_identical(v$keyword, "enum", info = door)
  }
  expect_identical(cast_checked("7", num_format(),
                                hypercast_constraints("i32", enum = integer(0)))$keyword, "enum")
  expect_identical(cast_text("Male", hypercast_constraints("text", enum = character(0)))$keyword,
                   "enum")
  expect_s3_class(hypercast_constraints("text", enum = NULL), "hypercast_constraints")
  # A duplicate entry is harmless (the first match wins), and an empty entry is an entry.
  dup <- cast_text("MALE", hypercast_constraints("text", enum = c("Male", "MALE")))
  expect_identical(c(dup$value, dup$enum_index), c("Male", "1"))
  blank <- cast_text(c("", " Male "), hypercast_constraints("text", enum = c("Male", "")))
  expect_identical(as.character(blank$reason), c("empty", NA))
  # Untrimmed, edge white space is part of the entry, and matches as declared.
  spaced <- cast_text(" MALE ", hypercast_constraints("text", enum = " Male "))
  expect_identical(spaced$value, " Male ")
  expect_identical(spaced$enum_index, 1L)
  # A numeric enum at a float or decimal door compares exactly.
  halves <- hypercast_constraints("f64", enum = c("1", "2.5"))
  v <- cast_checked(c("2.50", "2"), num_format(), halves)
  expect_identical(v$value, c(2.5, 2))
  expect_identical(v$keyword, c(NA, "enum"))
})

test_that("a number in a text enum is a valid entry that never matches", {
  numbers <- hypercast_constraints("text", enum = c(1L, 2L))
  expect_identical(numbers$enum, c(1L, 2L))
  v <- cast_text(c("1", "2", "x"), numbers)
  expect_identical(v$keyword, rep("enum", 3))
  expect_identical(v$value, c("1", "2", "x"))
  # Control: the same entries as text match.
  expect_identical(cast_text("2", hypercast_constraints("text", enum = c("1", "2")))$enum_index,
                   2L)
  expect_error(hypercast_constraints("text", enum = 1), "never match")
})

test_that("case_sensitive matches a text enum byte for byte", {
  folded <- hypercast_constraints("text", enum = c("mm", "Mm"))
  exact <- hypercast_constraints("text", enum = c("mm", "Mm"), case_sensitive = TRUE)
  expect_identical(cast_text("Mm", folded)$enum_index, 1L)
  v <- cast_text(c("Mm", "MM"), exact)
  expect_identical(v$value, c("Mm", "MM"))
  expect_identical(v$enum_index, c(2L, NA))
  expect_identical(v$keyword, c(NA, "enum"))
  expect_error(hypercast_constraints("text", case_sensitive = NA), "case_sensitive must be")
  expect_error(hypercast_constraints("text", case_sensitive = "yes"), "case_sensitive must be")
})

test_that("a violation carries the value as cast, never rounded", {
  # A rounding past what the type holds is a multipleOf violation carrying the parsed value.
  v <- cast_checked("127", num_format(),
                    hypercast_constraints("i8", multiple_of = "5", rounding = "ceiling"))
  expect_identical(c(v$keyword, v$value), c("multipleOf", "127"))
  expect_identical(v$value, 127L)
  # Rounded to 20, which breaks the maximum: the violation carries 19, not 20.
  v <- cast_checked("19", num_format(),
                    hypercast_constraints("i32", multiple_of = "10", rounding = "midpoint",
                                          maximum = "15"))
  expect_identical(v$keyword, "maximum")
  expect_identical(v$value, 19)
  expect_identical(v$adjusted, NA)
  # From a double, and at the text door the judged (trimmed) text, not an entry.
  expect_identical(cast_checked_from_double(-10, hypercast_constraints("i32", minimum = "0"))$value,
                   -10)
  t <- cast_text(" Mail ", hypercast_constraints("text", enum = "Male", trim = TRUE))
  expect_identical(c(t$value, t$offset, t$length), c("Mail", "1", "4"))
  expect_identical(cast_text(list(charToRaw(" Mail ")),
                             hypercast_constraints("text", enum = "Male", trim = TRUE),
                             raw = TRUE)$value[[1]], charToRaw("Mail"))
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
  expect_error(hypercast_constraints("text", enum = c(1, 2)), "character vector")
  tens <- hypercast_constraints("u8", enum = c(10L, 20L))
  expect_identical(cast_checked("0x14", num_format(), tens)$value, 20L)
})

test_that("lengths are whole numbers, never logicals or strings; the flags TRUE or FALSE", {
  expect_error(hypercast_constraints("text", min_length = TRUE), "min_length must be")
  expect_error(hypercast_constraints("text", max_length = "2"), "max_length must be")
  expect_error(hypercast_constraints("text", max_length = -1), "max_length must be")
  expect_error(hypercast_constraints("text", max_length = 2.5), "max_length must be")
  expect_error(hypercast_constraints("text", max_length = 2^32), "max_length must be")
  expect_error(hypercast_constraints("text", trim = NA), "trim must be")
  expect_error(hypercast_constraints("text", case_sensitive = 1L), "case_sensitive must be")
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
  expect_identical(v$value, c("I", "\u0131", "\u0130"))
  expect_identical(v$enum_index, c(1L, 2L, NA))
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
  expect_true(is.na(short$value))
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
  expect_identical(w$value, 17L)
  expect_identical(names(w), c("row", "keyword", "value", "offset", "length"))
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

test_that("a declaration carries the bytes it crosses the C ABI as", {
  word <- function(bytes, at) {
    readBin(bytes[at + 1:4], "integer", size = 4L, endian = .Platform$endian)
  }
  pay <- hypercast_constraints("i32", maximum = 10L, multiple_of = "5", rounding = "floor")
  expect_identical(length(pay$packed), 104L)
  # The size word, then the flags (HAS_MAXIMUM 4, HAS_MULTIPLE_OF 16) and the rounding's code.
  expect_identical(c(word(pay$packed, 0), word(pay$packed, 4), word(pay$packed, 8)),
                   c(104L, 20L, 2L))
  # maximum at 40 and multiple_of at 56, each a decimal whose low word is the value, scale 0.
  expect_identical(c(word(pay$packed, 40), word(pay$packed, 56)), c(10L, 5L))
  expect_identical(pay$packed_enum, raw())

  sex <- hypercast_constraints("text", enum = c("Female", "Male"))
  expect_identical(word(sex$packed, 4), 256L)
  # Each text entry its little-endian u32 byte length, then its UTF-8 bytes.
  expect_identical(sex$packed_enum,
                   c(as.raw(c(6, 0, 0, 0)), charToRaw("Female"), as.raw(c(4, 0, 0, 0)),
                     charToRaw("Male")))
  codes <- hypercast_constraints("u8", enum = c("1", "20"))
  expect_identical(length(codes$packed_enum), 32L)
  expect_identical(c(word(codes$packed_enum, 0), word(codes$packed_enum, 16)), c(1L, 20L))
  # An empty enum is declared, with nothing to pack.
  none <- hypercast_constraints("i32", enum = character(0))
  expect_identical(c(word(none$packed, 4), length(none$packed_enum)), c(256L, 0L))
  # The bytes are the core's: the same layout the checked doors pass, so equal declarations
  # carry equal bytes.
  expect_identical(hypercast_constraints("i32", maximum = "10")$packed,
                   hypercast_constraints("i32", maximum = 10L)$packed)
})

test_that("a declaration prints as the call that declares it", {
  sex <- hypercast_constraints("text", enum = c("Female", "Male"), trim = TRUE)
  expect_identical(capture.output(print(sex)),
                   "hypercast_constraints(\"text\", trim = TRUE, enum = c(\"Female\", \"Male\"))")
  expect_identical(capture.output(print(hypercast_constraints("i32", minimum = 1L))),
                   "hypercast_constraints(\"i32\", minimum = 1L)")
  unit <- hypercast_constraints("text", case_sensitive = TRUE, enum = character(0))
  expect_identical(capture.output(print(unit)),
                   "hypercast_constraints(\"text\", case_sensitive = TRUE, enum = character(0))")
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
  # An enum match is the entry as declared, in UTF-8; a violation is the judged text's bytes.
  expect_identical(v$value[[1]], charToRaw("\u00e9t\u00e9"))
  expect_identical(v$value[2:3], list(charToRaw("toolong"), charToRaw("c")))
  for (row in 4:5) expect_null(v$value[[row]])
  w <- violations(v)
  expect_identical(w$value, list(charToRaw("toolong"), charToRaw("c")))
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
