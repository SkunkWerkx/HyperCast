# Validation constraints: the declaration, its checks, and the checked doors.

constraint_doors <- c("i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64",
                      "decimal", "text")
integer_doors <- c("i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64")
roundings <- c("strict", "midpoint", "floor", "ceiling")

# The keywords by the core's codes, OpenAPI's spelling. The list is open: a code this
# version does not know reads as "keyword N".
keywords <- c("minimum", "exclusiveMinimum", "maximum", "exclusiveMaximum", "multipleOf",
              "minLength", "maxLength", "enum")

keyword_name <- function(code) {
  name <- keywords[code]
  unknown <- !is.na(code) & is.na(name)
  name[unknown] <- paste("keyword", code[unknown])
  name
}

# The core's Refusal Display text, by code: one message per code, raised as every other
# caller bug in this package is. Codes 2, 3, 4, 7, 10, 12 and 13 are retired: the core
# accepts what they refused, as OpenAPI does.
refusal_rules <- c(
  "1" = "a constraint the door does not take", "5" = "multipleOf is not positive",
  "6" = "a rounding without multipleOf",
  "8" = "a bound, step or enum entry the door cannot hold",
  "9" = "an enum entry that is not UTF-8", "11" = "a malformed constraints layout"
)

refusal_message <- function(code) {
  rule <- refusal_rules[as.character(code)]
  if (!is.na(rule)) {
    sprintf("Constraints refused (code %d): %s.", code, rule)
  } else {
    sprintf("Constraints refused (code %d): a rule this binding does not name.", code)
  }
}

# A bound or step: one string, or one R integer (exact by construction). A double is refused
# even when whole: it is binary, and the string says exactly what is meant.
exact_value <- function(value, what) {
  if (is.null(value)) {
    return(NULL)
  }
  if ((is.character(value) || (is.integer(value) && !is.object(value))) &&
        length(value) == 1L && !is.na(value)) {
    return(value)
  }
  stop(sprintf(paste("%s must be NULL, one string (\"0.05\") or one integer (5L), never a",
                     "double, which is binary: 0.1 has no exact double"), what), call. = FALSE)
}

length_value <- function(value, what) {
  if (is.null(value)) {
    return(NULL)
  }
  if (!is.numeric(value) || is.object(value) || length(value) != 1L || is.na(value) ||
        value < 0 || value > 4294967295 || value != floor(value)) {
    stop(sprintf("%s must be NULL or one whole number from 0 to 4294967295", what),
         call. = FALSE)
  }
  as.double(value)
}

enum_value <- function(value, door) {
  if (is.null(value)) {
    return(NULL)
  }
  text <- door == "text"
  ok <- !is.object(value) && !anyNA(value) && (is.character(value) || is.integer(value))
  if (!ok) {
    stop(if (text) {
      paste("enum must be NULL, a character vector or an integer vector (numbers, which never",
            "match) with no NA at the text door")
    } else {
      paste("enum must be NULL, a character vector or an integer vector with no NA, never a",
            "double")
    }, call. = FALSE)
  }
  if (text && is.integer(value)) {
    # A number is a valid entry that no string equals (JSON Schema's enum is exact equality
    # across types): laid out as an empty entry, which no input matches (empty input is the
    # door's Empty fault).
    return(rep("", length(value)))
  }
  value
}

# The declaration's fields, checked, in the order the C shim reads them by position.
constraints_layout <- function(constraints) {
  if (!inherits(constraints, "hypercast_constraints") || !is.list(constraints)) {
    stop("constraints must be a hypercast_constraints, as hypercast_constraints() returns",
         call. = FALSE)
  }
  door <- declared(constraints$door, constraint_doors, "door")
  door <- constraint_doors[[door]]
  flag <- function(value, what) {
    if (!is.logical(value) || length(value) != 1L || is.na(value)) {
      stop(sprintf("%s must be TRUE or FALSE", what), call. = FALSE)
    }
    value
  }
  list(
    door,
    exact_value(constraints$minimum, "minimum"),
    exact_value(constraints$exclusive_minimum, "exclusive_minimum"),
    exact_value(constraints$maximum, "maximum"),
    exact_value(constraints$exclusive_maximum, "exclusive_maximum"),
    exact_value(constraints$multiple_of, "multiple_of"),
    declared(constraints$rounding, roundings, "rounding") - 1L,
    length_value(constraints$min_length, "min_length"),
    length_value(constraints$max_length, "max_length"),
    flag(constraints$trim, "trim"),
    flag(constraints$case_sensitive, "case_sensitive"),
    enum_value(constraints$enum, door)
  )
}

#' Validation constraints
#'
#' `hypercast_constraints()` declares the rules a value must keep once it has been cast,
#' bound to one door: OpenAPI 3.2's (JSON Schema 2020-12's) numeric and string keywords,
#' judged by the core on the exact value the door read. Every declaration OpenAPI allows is
#' accepted, as it means there: the four bounds are independent (`minimum` and
#' `exclusive_minimum` may both be declared, and a value is held to each), bounds no value
#' meets and an empty `enum` make every value a violation, a duplicate `enum` entry is
#' harmless (the first match wins), and a keyword for another type (`min_length` at a numeric
#' door, `minimum` at the text door) is ignored. The declaration is judged once, here: one
#' the core refuses (a door that takes no constraints, `trim` or `case_sensitive` at a
#' numeric door, a `rounding` at the text door, a step that is not positive, a rounding
#' without `multiple_of`, a bound, step or entry the door cannot hold, an entry that is not
#' UTF-8) is an error, `Constraints refused (code N): <the rule>.`, with the code the core
#' returned. Every rule is the core's; the package checks only the R types of the
#' arguments.
#'
#' The door is an argument the declaration keeps: [cast_checked()] and
#' [cast_checked_from_double()] take the numeric doors' declarations and [cast_text()] the
#' text door's, and handing a declaration to another door's cast is an error.
#'
#' Bounds, `multiple_of` and numeric `enum` entries are exact: a string in invariant notation
#' (`"0.05"`, `"-90"`, `"1e-6"`) or an R integer (`5L`). A double is refused even when it is
#' whole, since `0.1` has no exact double; write `"0.1"`. Whole numbers past R's integer
#' range (`i64`, `u64`) are strings. A numeric `enum` is a character or an integer vector,
#' each entry compared exactly to the value the door delivers; an entry the door's type
#' cannot hold (a fraction at an integer door, `"256"` at `"u8"`) never matches. The text
#' door's `enum` is a character vector, matched under Unicode simple case folding unless
#' `case_sensitive` is `TRUE`, and a match comes back as the entry exactly as declared; an
#' integer vector there is numbers, valid entries no text equals, so every value violates it.
#'
#' The keywords, by door:
#'
#' - The numeric doors (`"i8"` to `"u64"`, `"f32"`, `"f64"` and `"decimal"`): `minimum`,
#'   `exclusive_minimum`, `maximum`, `exclusive_maximum`, `multiple_of` (with `rounding`) and
#'   `enum`.
#' - `"text"`: `min_length` and `max_length` (in code points), `enum`, and HyperCast's own
#'   `trim` (Unicode `White_Space` at either end, before anything is judged) and
#'   `case_sensitive` (an `enum` matched byte for byte).
#'
#' `rounding` is how a value that is not a multiple of `multiple_of` is judged: `"strict"`
#' (the default) is a `multipleOf` violation; `"midpoint"` the nearest multiple, a tie to the
#' even one; `"floor"` down and `"ceiling"` up. Rounding happens on the exact decimal, before
#' the bounds judge the result, and marks the row `adjusted`; a rounding past what the door's
#' type holds is a `multipleOf` violation. A violation's `value` is always the value as cast,
#' never rounded.
#'
#' @param door The door: `"i8"`, `"i16"`, `"i32"`, `"i64"`, `"u8"`, `"u16"`, `"u32"`,
#'   `"u64"`, `"f32"`, `"f64"`, `"decimal"` or `"text"`.
#' @param minimum,exclusive_minimum,maximum,exclusive_maximum The bounds; `NULL` declares
#'   none.
#' @param multiple_of The step every value must be a multiple of; `NULL` declares none.
#' @param rounding `"strict"`, `"midpoint"`, `"floor"` or `"ceiling"`.
#' @param min_length,max_length The fewest and most code points the text may hold; `NULL`
#'   declares none.
#' @param trim Whether text is trimmed of Unicode `White_Space` before it is judged.
#' @param case_sensitive Whether a text `enum` matches byte for byte (`TRUE`) rather than
#'   under Unicode simple case folding (`FALSE`, the default), for a vocabulary whose case
#'   carries meaning (`"mm"` and `"Mm"`).
#' @param enum The values allowed; `NULL` declares none. An empty vector (`character(0)`)
#'   declares one no value matches: every value is then an `enum` violation.
#' @param x A `hypercast_constraints`.
#' @param ... Ignored.
#' @return A `hypercast_constraints`: a list of the door and the declared keywords.
#' @family constraints
#' @export
#' @examples
#' latitude <- hypercast_constraints("f64", minimum = "-90", maximum = "90",
#'                                   multiple_of = "0.000001", rounding = "midpoint")
#' latitude
#' unit <- hypercast_constraints("text", enum = c("mm", "Mm"), case_sensitive = TRUE)
#' cast_text(c("Mm", "MM"), unit)
#' try(hypercast_constraints("i32", multiple_of = "0"))
#' try(hypercast_constraints("f64", maximum = 90))
hypercast_constraints <- function(door, minimum = NULL, exclusive_minimum = NULL,
                                  maximum = NULL, exclusive_maximum = NULL,
                                  multiple_of = NULL, rounding = "strict", min_length = NULL,
                                  max_length = NULL, trim = FALSE, case_sensitive = FALSE,
                                  enum = NULL) {
  constraints <- structure(
    list(door = door, minimum = minimum, exclusive_minimum = exclusive_minimum,
         maximum = maximum, exclusive_maximum = exclusive_maximum, multiple_of = multiple_of,
         rounding = rounding, min_length = min_length, max_length = max_length, trim = trim,
         case_sensitive = case_sensitive, enum = enum),
    class = "hypercast_constraints"
  )
  layout <- constraints_layout(constraints)
  code <- .Call(C_constraints_check, layout, session_native())
  if (code != 0L) {
    stop(refusal_message(code), call. = FALSE)
  }
  constraints
}

#' @rdname hypercast_constraints
#' @export
print.hypercast_constraints <- function(x, ...) {
  show <- function(value) {
    if (is.character(value)) {
      value <- encodeString(value, quote = "\"")
    } else if (is.integer(value)) {
      value <- paste0(value, "L")
    }
    if (length(value) == 0L) {
      return(paste0(typeof(value), "(0)"))
    }
    if (length(value) == 1L) value else paste0("c(", paste(value, collapse = ", "), ")")
  }
  call <- sprintf("hypercast_constraints(%s", show(x$door))
  for (name in c("minimum", "exclusive_minimum", "maximum", "exclusive_maximum",
                 "multiple_of", "rounding", "min_length", "max_length", "trim",
                 "case_sensitive", "enum")) {
    value <- x[[name]]
    default <- is.null(value) || (name == "rounding" && identical(value, "strict")) ||
      (name %in% c("trim", "case_sensitive") && isFALSE(value))
    if (!default) call <- paste0(call, ", ", name, " = ", show(value))
  }
  cat(call, ")\n", sep = "")
  invisible(x)
}

checked_layout <- function(constraints, text) {
  layout <- constraints_layout(constraints)
  if (text && layout[[1]] != "text") {
    stop(sprintf("cast_text() takes the text door's constraints; these are for \"%s\"",
                 layout[[1]]), call. = FALSE)
  }
  if (!text && layout[[1]] == "text") {
    stop("the text door's constraints go to cast_text()", call. = FALSE)
  }
  layout
}

#' Checked doors
#'
#' The checked doors read a value as the plain doors do, then judge it by a
#' [hypercast_constraints()] declaration. `cast_checked()` reads text through a numeric door
#' under a declared [num_format()], so `"$12.37"`, `"(12.35)"`, `"1.234,55"` and `"5%"` are
#' judged on the number the door reads (`5%` is `0.05`). `cast_checked_from_double()` judges
#' a number the source already decoded (a spreadsheet cell, a JSON number) on its shortest
#' round-trip decimal, the text [cast_decimal_from_double()] reads it as, so `0.1 + 0.2` is
#' `0.30000000000000004`; `-0` is zero. `cast_text()` is the text door: alone it validates
#' UTF-8 and returns the text as a plain [hypercast_verdict]; with constraints it trims,
#' measures and matches it.
#'
#' Each returns a [hypercast_checked_verdict], whose `value` is in the plain door's R type
#' (text is character, or a list of raw vectors with `raw = TRUE`). A text `enum` match is the
#' entry exactly as declared, and its `enum_index` is the entry's position in `enum`, counted from
#' 1 as R counts. A violation carries its value too, in the same column: the value exactly as the
#' door cast it, never rounded, and at the text door the judged text (trimmed under `trim`), not
#' an entry.
#'
#' `cast_text(x)` with no `constraints` is the plain text door; an explicit `NULL`, or
#' anything that is not a `hypercast_constraints`, is an error rather than a cast without
#' rules. A declaration for another door is an error too.
#'
#' The text door's value is the input's own string when nothing was trimmed. Text no R string
#' can hold, with U+0000 inside it (a raw vector can carry that) or over `INT_MAX` bytes, is `ok`
#' with `NA` as its `value`, as [cast_char()]'s `U+0000` is. `ok` describes the core's acceptance
#' and `value = NA` means R can't represent it, so checking `ok` alone doesn't guarantee a usable
#' character value. `raw = TRUE` gives the exact output: `value` is a list holding each value's
#' UTF-8 bytes as a raw vector, trimmed as the character value is, with `NULL` where a row has no
#' value. A missing input still has `ok` `NA` and a fault `ok` `FALSE`.
#'
#' @param x A character vector (a factor is read as its levels) or a list of raw vectors
#'   holding UTF-8 bytes, as the text doors take; for `cast_checked_from_double()`, a
#'   numeric vector, where `NA` is missing and `NaN` is a number, and `malformed`.
#' @inheritParams cast_i8
#' @param constraints A [hypercast_constraints()] declaration for the door.
#' @param raw `FALSE` (the default) for a character `value`, `TRUE` for a list of raw vectors.
#' @return A [hypercast_checked_verdict]; `cast_text(x)` alone a [hypercast_verdict].
#' @family constraints
#' @export
#' @examples
#' latitude <- hypercast_constraints("f64", minimum = "-90", maximum = "90",
#'                                   multiple_of = "0.000001", rounding = "midpoint")
#' cast_checked(c("45.1234567", "91", "north"), num_format(), latitude)
#' cast_checked_from_double(45.1234567, latitude)$value
#'
#' cash <- hypercast_constraints("decimal", multiple_of = "0.05")
#' cast_checked(c("12.35", "12.37"), num_format(), cash)
#'
#' sex <- hypercast_constraints("text", enum = c("Female", "Male", "Other"), trim = TRUE)
#' cast_text(c(" male ", "OTHER", "x"), sex)
#' cast_text(c("plain", ""))
#'
#' nul <- list(as.raw(c(0x61, 0x00, 0x62)))
#' cast_text(nul)
#' cast_text(nul, raw = TRUE)$value[[1]]
cast_checked <- function(x, format, constraints) {
  layout <- checked_layout(constraints, text = FALSE)
  cols <- .Call(C_cast_checked, as_text(x), layout, check_format(format), session_native(),
                FALSE)
  new_checked_verdict(cols, layout[[1]])
}

#' @rdname cast_checked
#' @export
cast_checked_from_double <- function(x, constraints) {
  layout <- checked_layout(constraints, text = FALSE)
  cols <- .Call(C_cast_checked_double, as_number(x), layout, session_native())
  new_checked_verdict(cols, layout[[1]])
}

#' @rdname cast_checked
#' @export
cast_text <- function(x, constraints, raw = FALSE) {
  if (!isTRUE(raw) && !isFALSE(raw)) {
    stop("raw must be TRUE or FALSE", call. = FALSE)
  }
  # The call's own arguments, not missing(): a wrapper forwarding its own missing `constraints`
  # still passes it, and must fail rather than drop the rules.
  if (!"constraints" %in% names(match.call())) {
    return(text_door(x, "text", raw = raw))
  }
  layout <- checked_layout(constraints, text = TRUE)
  cols <- .Call(C_cast_checked, as_text(x), layout, NULL, session_native(), raw)
  new_checked_verdict(cols, "text")
}

# One string through cast_declared_decimal, the conversion every bound, step and integer
# entry takes: its code and the raw triple. For the declarations.json replay.
declared_decimal <- function(x) .Call(C_declared_decimal, x, session_native())
