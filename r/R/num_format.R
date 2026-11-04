#' Numeric styles
#'
#' The lenience flags and the separator policy of a [num_format()], as bits: `grouping`
#' (`1`, the group separator between digits), `parentheses` (`2`, accounting negatives,
#' `(1,234)`), `exponent` (`4`, `1e3`; an integer door refuses a negative one),
#' `radix_prefixes` (`8`, `0x`, `&H` and `0b` as two's-complement bit patterns, integer
#' doors only), `percent` (`16`, a trailing `%` divides by 100, real and decimal doors),
#' `separator_detect` (`32`, read the `.`/`,` roles from each input's structure instead of
#' the declared separators), `currency` (`64`, the declared currency symbol at either edge),
#' `trailing_sign` (`128`, the sign after the digits, as a culture whose negative pattern is
#' `n-` writes it) and `all` (`95`, every lenience but `trailing_sign`; not
#' `separator_detect`, which is a policy). These are the bits 0 to 7 of the declared format's
#' flags; the grouping policy sits in bits 8 and 9 and is declared apart, by `grouping` in
#' [num_format()], never inside a style mask.
#'
#' @format A named integer vector.
#' @family formats
#' @export
#' @examples
#' num_styles
#' bitwOr(num_styles[["grouping"]], num_styles[["parentheses"]])
num_styles <- c(
  grouping = 1L, parentheses = 2L, exponent = 4L, radix_prefixes = 8L, percent = 16L,
  separator_detect = 32L, currency = 64L, trailing_sign = 128L, all = 95L
)

# The 25 code points with the Unicode White_Space property, the core's `is_white_space`. The
# C shim (src/hypercast.c) restates them as a byte scanner, and the corpus's formats.json
# holds both to the same list.
white_space_code_points <- c(
  0x0009:0x000d, 0x0020, 0x0085, 0x00a0, 0x1680, 0x2000:0x200a, 0x2028, 0x2029, 0x202f,
  0x205f, 0x3000
)
ascii_digit <- as.raw(0x30:0x39)

one_string <- function(value, what) {
  if (!is.character(value) || length(value) != 1L || is.na(value)) {
    stop(sprintf("%s must be a single string", what), call. = FALSE)
  }
  bytes <- if (Encoding(value) == "unknown" && session_native() == 1L) {
    charToRaw(value)
  } else {
    charToRaw(enc2utf8(value))
  }
  if (!validUTF8(rawToChar(bytes))) {
    stop(sprintf("%s must be valid UTF-8", what), call. = FALSE)
  }
  bytes
}

# A symbol's bytes, or a caller bug named in the core's own terms. `limit` is the symbol's
# capacity; `flush` refuses white space anywhere (a sign or percent symbol, read against the
# digits), `edges` at either end only (a day period or a currency symbol, which may hold
# white space inside). "" declares nothing unless `required`. The rules apply in the core's
# order: length, digit, white space.
symbol_bytes <- function(value, what, limit, required = FALSE, flush = FALSE, edges = FALSE) {
  bytes <- one_string(value, what)
  if (!length(bytes) && !required) {
    return(bytes)
  }
  if (length(bytes) < 1L || length(bytes) > limit || any(bytes %in% ascii_digit)) {
    stop(sprintf("%s must be 1 to %d UTF-8 bytes with no ASCII digit; got \"%s\"", what, limit,
                 value), call. = FALSE)
  }
  if (flush) {
    refuse_white_space(bytes, value, what)
  }
  if (edges) {
    points <- code_points_of(bytes)
    if (points[[1L]] %in% white_space_code_points ||
          points[[length(points)]] %in% white_space_code_points) {
      stop(sprintf("%s must not start or end with white space; got \"%s\"", what, value),
           call. = FALSE)
    }
  }
  bytes
}

code_points_of <- function(bytes) utf8ToInt(rawToChar(bytes))

# A separator or symbol with white space anywhere in it, refused as the core refuses it.
refuse_white_space <- function(bytes, value, what) {
  if (any(code_points_of(bytes) %in% white_space_code_points)) {
    stop(sprintf("%s must not contain white space; got \"%s\"", what, value), call. = FALSE)
  }
}

# A symbol as it crosses the ABI: its bytes, zero padding to its capacity, then its length.
pack_symbol <- function(bytes, limit) {
  c(bytes, raw(limit - length(bytes)), as.raw(length(bytes)))
}

u32_le <- function(value) writeBin(as.integer(value), raw(), size = 4L, endian = "little")

# The grouping policies, by the core's values in bits 8 and 9 of the flags word.
grouping_policies <- c(legacy = 0L, western = 256L, indian = 512L)

grouping_bits <- function(grouping) {
  grouping_policies[[declared(grouping, names(grouping_policies), "grouping")]]
}

style_bits <- function(flags) {
  if (is.character(flags) && !anyNA(flags)) {
    unknown <- setdiff(flags, names(num_styles))
    if (length(unknown)) {
      stop(sprintf("flags names no style %s; see num_styles",
                   paste0("\"", unknown, "\"", collapse = ", ")), call. = FALSE)
    }
    return(Reduce(bitwOr, num_styles[flags], 0L))
  }
  if (!is.numeric(flags) || length(flags) != 1L || is.na(flags) || flags < 0 ||
        flags != floor(flags) || flags > 4294967295) {
    stop("flags must be one non-negative whole number or a character vector of style names",
         call. = FALSE)
  }
  # A style mask holds the styles only: the policy is `grouping`, and a bit past it is no
  # style at all. The unknown bits are judged first, as the core judges a flags word.
  if (flags >= 1024) {
    stop(sprintf("flags holds only the styles of num_styles (bits 0 to 7); got %.0f", flags),
         call. = FALSE)
  }
  if (flags >= 256) {
    stop(sprintf(paste("flags must not carry a grouping policy (bits 8 and 9); declare it with",
                       "grouping; got %.0f"), flags), call. = FALSE)
  }
  as.integer(flags)
}

#' Declared numeric formats
#'
#' The numeric doors read text under a format the caller declares: the separators, the
#' lenience flags, and everything a culture table would otherwise fill in. The core carries
#' no culture data; a format that declares nothing past its separators reads exactly as the
#' invariant culture, and each declared symbol is read beside its ASCII default, never
#' instead of it.
#'
#' `num_format()` with no arguments is the invariant format, `.` decimal and `,` group
#' separators with every lenience in `num_styles[["all"]]`, and strict Western grouping.
#'
#' Grouping is strict unless the caller says otherwise. `grouping = "western"`, the default,
#' reads a group separator only between groups of exactly three digits counted from the
#' decimal separator (or the end of the numeral), the leftmost one to three: `1,234,567` is
#' read, `12,34` is `malformed` (`group_size`, over `,34`) and `1234,567` is too (over
#' `1234`). `"indian"` reads the group beside the decimal separator as three digits and every
#' group to its left as two (`12,34,567`). `"legacy"` reads a group separator anywhere
#' between two digits, as every HyperCast release before 1.1 did (`12,34` is `1234`); it is
#' the escape hatch for data written that way. Ungrouped numerals (`1234567`) are read under
#' all three; once a numeral uses a group separator every group must conform; grouping is
#' never read after the decimal separator or in an exponent. The rules apply to the
#' number and the currency group separator alike, to native digits, and to the white space
#' group separators that stand for each other (`" "`, U+00A0, U+202F). A policy does not turn
#' grouping on: with the `grouping` style off a group separator is `malformed` under all three.
#' Separator detection stays independent of the policy: no policy resolves an ambiguous
#' `1,234`. `num_format_detect()` resolves
#' the `.`/`,` roles from each input's structure instead: a repeated separator is grouping
#' (`1.234.567,89`), with both present the rightmost is the decimal, a single one with other
#' than three digits after it is the decimal (`3,1415`), and so is one after a lone zero
#' (`0,785`); anything else (`12.185`, `1,000`) is `malformed` at the separator, never
#' guessed.
#'
#' `num_format_from_localeconv()` bridges a locale's conventions, `Sys.localeconv()` by
#' default: `decimal_point` and `thousands_sep` as the separators, `currency_symbol`,
#' `mon_decimal_point` and `mon_thousands_sep` as the currency symbol and the separators a
#' currency amount is read with, `negative_sign` and `positive_sign` as further spellings of
#' the signs, and an `n_sign_posn` of `2` or `4` as `trailing_sign`. A separator the locale
#' leaves empty takes its invariant default unless the other separator holds that
#' character, in which case it takes the other of the pair; an empty currency pair keeps the
#' number separators. The locale carries no percent symbol and no native digits. Every
#' lenience in `num_styles[["all"]]` is on, except as the grouping rule below says.
#'
#' The grouping rule reads the locale's own `grouping` and `mon_grouping` patterns, never its
#' name and never the input, and never rejects what the platform itself writes. Each is C's
#' localeconv string: a byte per group size from the right, the last size repeating, `CHAR_MAX`
#' ending the pattern (an empty string, or `CHAR_MAX` first, groups nothing). A format has one
#' policy and one `grouping` style for numbers and currency amounts alike, so only the patterns
#' that group count. All of them groups of three (`3`, `3;3`): strict Western. All of them three
#' then twos (`3;2`): strict Indian. Any other shape (groups of two, a pattern that ends) or
#' patterns that group and disagree (`3;2` beside `3`): `"legacy"`, a group separator anywhere
#' between two digits. The `grouping` style is cleared only when neither pattern groups; when
#' just one does not, the style stays for the other (a numeral without a group separator is
#' valid anyway), and a format cannot forbid currency grouping on its own. A vector that carries
#' neither field declares nothing, and the format keeps the default. (R holds `LC_NUMERIC` at
#' `"C"`, so a locale's number pattern is usually empty and its currency pattern decides.)
#'
#' A format that cannot be is an error when it is declared, never when it is read: a
#' separator that is not 1 to 8 UTF-8 bytes or holds an ASCII digit, two equal separators
#' (the number pair, or the pair a currency amount is read with), a decimal separator or a
#' sign or percent symbol with white space in it, a currency symbol that is not 1 to 16 such
#' bytes or has white space at either end, a digit zero that starts no Unicode digit block,
#' or `flags` that carry a bit that is no style (a grouping policy, which `grouping` declares,
#' or an undefined bit).
#' White space is the 25 code points with the Unicode `White_Space` property; a group
#' separator may be or contain it, and a currency symbol may hold it inside (`"F CFA"`). These
#' are the core's own contract rules, raised in R so that every format that exists is one the
#' core reads.
#'
#' @param decimal_sep,group_sep The decimal and digit-group separators: 1 to 8 UTF-8 bytes
#'   each, with no ASCII digit, and different from each other.
#' @param flags The styles: one whole number of [num_styles] bits, or a character vector of
#'   their names.
#' @param currency The currency symbol, read at either edge of the number while the
#'   `currency` style is on; `""` declares none.
#' @param currency_decimal_sep,currency_group_sep The separators a currency amount is read
#'   with once the symbol is found; `""` keeps the number separator.
#' @param minus_sign,plus_sign Further spellings of the signs, read beside ASCII `-` and
#'   `+`; `""` declares none.
#' @param percent A further spelling of the percent symbol, read beside `%`; `""` declares
#'   none.
#' @param digit_zero The zero of a block of native digits read beside ASCII `0`-`9`
#'   (`"\\u0660"` for Arabic-Indic); `""` declares none.
#' @param grouping The grouping policy: `"western"` (the default), `"indian"` or
#'   `"legacy"`, the escape hatch for 1.0 behaviour.
#' @param conv A named character vector shaped like [Sys.localeconv()]'s.
#' @return A `hypercast_num_format`: a list of the declared fields, `grouping` and `packed`,
#'   the 92 bytes the format crosses the C ABI as (the core's size-tagged layout: the size,
#'   the flags (the styles and the policy) and the digit zero as little-endian 32-bit words,
#'   then each symbol's UTF-8 bytes, zero-padded, and its length), for a library with a C ABI
#'   of its own.
#' @family formats
#' @export
#' @examples
#' num_format()
#' eurozone <- num_format(",", ".")
#' cast_f64("1.234,5", eurozone)$value
#' cast_f64(c("1.234.567,89", "3,1415", "12.185"), num_format_detect())
#' cast_decimal("1.234,50 kr.", num_format(",", ".", currency = "kr."))$value
#' num_format(flags = c("grouping", "parentheses"))
#' cast_i32(c("1,234,567", "12,34"), num_format())
#' cast_i32(c("12,34", "12,34,567"), num_format(grouping = "legacy"))$value
#' cast_i32("12,34,567", num_format(grouping = "indian"))$value
num_format <- function(decimal_sep = ".", group_sep = ",", flags = num_styles[["all"]],
                       currency = "", currency_decimal_sep = "", currency_group_sep = "",
                       minus_sign = "", plus_sign = "", percent = "", digit_zero = "",
                       grouping = "western") {
  decimal <- symbol_bytes(decimal_sep, "Decimal separator", 8L, required = TRUE)
  group <- symbol_bytes(group_sep, "Group separator", 8L, required = TRUE)
  if (identical(decimal, group)) {
    stop(sprintf("Decimal and group separators must differ; both are \"%s\"", decimal_sep),
         call. = FALSE)
  }
  currency_decimal <- symbol_bytes(currency_decimal_sep, "Currency decimal separator", 8L)
  currency_group <- symbol_bytes(currency_group_sep, "Currency group separator", 8L)
  in_force <- function(own, number) if (length(own)) own else number
  if (identical(in_force(currency_decimal, decimal), in_force(currency_group, group))) {
    stop(sprintf("Currency decimal and group separators must differ; both are \"%s\"",
                 rawToChar(in_force(currency_decimal, decimal))), call. = FALSE)
  }
  # A numeral admits white space only as a group separator, so a decimal separator holding
  # any could never match.
  refuse_white_space(decimal, decimal_sep, "Decimal separator")
  refuse_white_space(currency_decimal, currency_decimal_sep, "Currency decimal separator")
  minus <- symbol_bytes(minus_sign, "Minus sign", 8L, flush = TRUE)
  plus <- symbol_bytes(plus_sign, "Plus sign", 8L, flush = TRUE)
  percent_bytes <- symbol_bytes(percent, "Percent symbol", 8L, flush = TRUE)
  currency_bytes <- symbol_bytes(currency, "Currency symbol", 16L, edges = TRUE)
  zero <- digit_code(digit_zero)
  bits <- style_bits(flags)
  policy <- grouping_bits(grouping)
  packed <- c(
    u32_le(92L), u32_le(bitwOr(bits, policy)), u32_le(zero),
    pack_symbol(decimal, 8L), pack_symbol(group, 8L),
    pack_symbol(currency_decimal, 8L), pack_symbol(currency_group, 8L),
    pack_symbol(minus, 8L), pack_symbol(plus, 8L), pack_symbol(percent_bytes, 8L),
    pack_symbol(currency_bytes, 16L)
  )
  structure(
    list(decimal_sep = decimal_sep, group_sep = group_sep, flags = bits, currency = currency,
         currency_decimal_sep = currency_decimal_sep, currency_group_sep = currency_group_sep,
         minus_sign = minus_sign, plus_sign = plus_sign, percent = percent,
         digit_zero = digit_zero, grouping = grouping, packed = packed),
    class = "hypercast_num_format"
  )
}

# The native digits' zero as its code point, 0 for none: one non-ASCII character a Unicode
# digit block starts at (its ten successors share its UTF-8 prefix: code point mod 64 <= 54).
digit_code <- function(digit_zero) {
  bytes <- one_string(digit_zero, "Digit zero")
  if (!length(bytes)) {
    return(0L)
  }
  code <- utf8ToInt(rawToChar(bytes))
  if (length(code) != 1L || code < 0x80 || code %% 64L > 54L) {
    stop(sprintf(paste("Digit zero must be the one non-ASCII character a Unicode digit block",
                       "starts at; got \"%s\""), digit_zero), call. = FALSE)
  }
  code
}

#' @rdname num_format
#' @export
num_format_detect <- function(grouping = "western") {
  num_format(flags = bitwOr(num_styles[["all"]], num_styles[["separator_detect"]]),
             grouping = grouping)
}

#' @rdname num_format
#' @export
num_format_from_localeconv <- function(conv = Sys.localeconv()) {
  if (!is.character(conv) || is.null(names(conv))) {
    stop("conv must be a named character vector, as Sys.localeconv() returns", call. = FALSE)
  }
  field <- function(name) {
    value <- conv[name]
    if (is.na(value)) "" else enc2utf8(unname(value))
  }
  number <- separator_pair(field("decimal_point"), field("thousands_sep"), c(".", ","))
  money <- c(field("mon_decimal_point"), field("mon_thousands_sep"))
  money <- if (all(money == "")) c("", "") else separator_pair(money[[1]], money[[2]], number)
  # The ASCII signs are always read, so declaring them again would say nothing.
  sign <- function(name) {
    value <- field(name)
    if (value %in% c("-", "+")) "" else value
  }
  flags <- num_styles[["all"]]
  if (field("n_sign_posn") %in% c("2", "4")) {
    flags <- bitwOr(flags, num_styles[["trailing_sign"]])
  }
  declared <- locale_grouping(conv)
  if (!declared$grouped) {
    flags <- bitwAnd(flags, bitwNot(num_styles[["grouping"]]))
  }
  num_format(number[[1]], number[[2]], flags,
             currency = field("currency_symbol"),
             currency_decimal_sep = money[[1]], currency_group_sep = money[[2]],
             minus_sign = sign("negative_sign"), plus_sign = sign("positive_sign"),
             grouping = declared$policy)
}

# The shape of one localeconv grouping string (C's semantics): each byte is a group size, the
# right-most group first; the last size repeats (a terminating 0 in C is the end of an R string,
# and means "repeat the last size", never "grouping off"); CHAR_MAX (127, or 255 where char is
# unsigned) ends the pattern with no further grouping. "none" groups nothing (empty, or CHAR_MAX
# first), "western" is groups of three, "indian" is three then twos, repeating (CLDR
# #,##,##0.###), and "other" is anything else (groups of two, a pattern that ends, 3;2;3).
# NULL: the vector does not carry the field, so the locale states nothing.
group_shape <- function(conv, name) {
  value <- conv[name]
  if (is.na(value)) {
    return(NULL)
  }
  sizes <- if (nzchar(value)) as.integer(charToRaw(unname(value))) else integer()
  end <- which(sizes >= 127L)[1L]
  ended <- !is.na(end)
  if (ended) sizes <- sizes[seq_len(end - 1L)]
  if (!length(sizes)) {
    "none"
  } else if (ended) {
    "other"
  } else if (all(sizes == 3L)) {
    "western"
  } else if (length(sizes) >= 2L && sizes[[1L]] == 3L && all(sizes[-1L] == 2L)) {
    "indian"
  } else {
    "other"
  }
}

# What a locale's own patterns declare, never read from its name or from input: the policy, and
# whether the format groups at all. One format has one policy and one GROUPING flag for number
# and currency alike, so only the patterns that group count: all Western is strict Western, all
# Indian strict Indian, and any other shape, or enabled patterns that disagree, is legacy (a
# bridge never rejects its platform's own output). GROUPING is cleared only when neither pattern
# groups; if one of them does not, the flag stays for the other and a format cannot forbid
# currency grouping alone. A vector that carries neither field says nothing: the default.
locale_grouping <- function(conv) {
  shapes <- c(group_shape(conv, "grouping"), group_shape(conv, "mon_grouping"))
  if (!length(shapes)) {
    return(list(policy = "western", grouped = TRUE))
  }
  enabled <- unique(shapes[shapes != "none"])
  policy <- if (!length(enabled) || identical(enabled, "western")) {
    "western"
  } else if (identical(enabled, "indian")) {
    "indian"
  } else {
    "legacy"
  }
  list(policy = policy, grouped = length(enabled) > 0L)
}

# A locale's separator pair: a field the locale leaves empty takes its default unless the
# other separator already holds that text, in which case it takes the other default. Two the
# locale itself declares equal are left equal, for num_format() to refuse.
separator_pair <- function(decimal, group, defaults) {
  if (decimal == "" && group == "") {
    return(defaults)
  }
  if (group == "") {
    group <- if (decimal == defaults[[2]]) defaults[[1]] else defaults[[2]]
  }
  if (decimal == "") {
    decimal <- if (group == defaults[[1]]) defaults[[2]] else defaults[[1]]
  }
  c(decimal, group)
}

#' @rdname num_format
#' @param x A `hypercast_num_format`.
#' @param ... Ignored.
#' @export
print.hypercast_num_format <- function(x, ...) {
  quote <- function(s) encodeString(s, quote = "\"")
  call <- sprintf("num_format(%s, %s, %d", quote(x$decimal_sep), quote(x$group_sep), x$flags)
  for (name in c("currency", "currency_decimal_sep", "currency_group_sep", "minus_sign",
                 "plus_sign", "percent", "digit_zero")) {
    if (nzchar(x[[name]])) call <- paste0(call, ", ", name, " = ", quote(x[[name]]))
  }
  # A format made before the policy existed has no `grouping` field, and read legacy.
  grouping <- if (is.null(x$grouping)) "legacy" else x$grouping
  if (!identical(grouping, "western")) {
    call <- paste0(call, ", grouping = ", quote(grouping))
  }
  cat(call, ")\n", sep = "")
  invisible(x)
}

check_format <- function(format) {
  if (!inherits(format, "hypercast_num_format") || !is.raw(format$packed) ||
        length(format$packed) != 92L) {
    stop("format must be a hypercast_num_format, as num_format() returns", call. = FALSE)
  }
  format$packed
}
