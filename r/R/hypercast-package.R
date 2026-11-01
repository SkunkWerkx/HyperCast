#' @details
#' Every door takes a vector and returns a [hypercast_verdict]: a data frame with one row
#' per element, holding the element's value where it parsed and its fault (a reason and the
#' offending span) where it did not. No door signals an error for anything in its data; the
#' verdict is the result. Errors are reserved for caller bugs in the arguments that declare
#' how to read the data (a format, an order, a precision, an epoch, a day period), as in
#' every other HyperCast binding.
#' @keywords internal
"_PACKAGE"

#' @useDynLib hypercast, .registration = TRUE, .fixes = "C_"
NULL
