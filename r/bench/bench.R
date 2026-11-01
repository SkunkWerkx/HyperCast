# hypercast's doors against base R's coercions, over 1000 strings per call. Base R returns NA
# where a door returns a fault and its span, so the two do different work; the comparison is
# the call an R user reaches for today.
# Run from the package root against an installed copy:
#   R CMD INSTALL --library=<lib> . && R_LIBS=<lib> Rscript bench/bench.R
# Results are machine-specific: the header names the date, R and the machine.
library(hypercast)
if (!requireNamespace("bench", quietly = TRUE)) {
  stop("bench/bench.R needs the bench package: install.packages(\"bench\")")
}

machine <- function() {
  if (Sys.info()[["sysname"]] == "Darwin") {
    cpu <- system("sysctl -n machdep.cpu.brand_string", intern = TRUE)
    model <- system("sysctl -n hw.model", intern = TRUE)
    return(sprintf("%s (%s)", cpu, model))
  }
  if (file.exists("/proc/cpuinfo")) {
    model <- grep("^model name", readLines("/proc/cpuinfo"), value = TRUE)
    if (length(model) > 0) return(sub("^model name\\s*:\\s*", "", model[[1]]))
  }
  Sys.info()[["machine"]]
}

cat(sprintf("%s, %s, %s\n", format(Sys.Date()), R.version.string, machine()))
cat(sprintf("hypercast %s, bench %s\n\n", packageVersion("hypercast"),
            packageVersion("bench")))

set.seed(1)
n <- 1000
invariant <- num_format()
reals <- format(runif(n, -1e6, 1e6), digits = 15)
ints <- as.character(sample.int(.Machine$integer.max, n))
bools <- sample(c("true", "false", "TRUE", "False"), n, replace = TRUE)
days <- format(as.Date("1970-01-01") + sample.int(40000, n))
instants <- format(as.POSIXct(runif(n, 0, 2e9), origin = "1970-01-01", tz = "UTC"),
                   "%Y-%m-%dT%H:%M:%SZ", tz = "UTC")

results <- bench::mark(
  `hypercast::cast_f64(x, num_format())` = cast_f64(reals, invariant),
  `as.numeric(x)` = as.numeric(reals),
  `hypercast::cast_i32(x, num_format())` = cast_i32(ints, invariant),
  `as.integer(x)` = as.integer(ints),
  `hypercast::cast_bool(x)` = cast_bool(bools),
  `as.logical(x)` = as.logical(bools),
  `hypercast::cast_date(x)` = cast_date(days),
  `as.Date(x)` = as.Date(days),
  `hypercast::cast_timestamp(x)` = cast_timestamp(instants),
  `as.POSIXct(x, tz = "UTC", format = "%Y-%m-%dT%H:%M:%OSZ")` =
    as.POSIXct(instants, tz = "UTC", format = "%Y-%m-%dT%H:%M:%OSZ"),
  check = FALSE
)

table <- data.frame(
  expression = as.character(results$expression),
  median = format(results$median),
  `itr/sec` = round(results$`itr/sec`),
  mem_alloc = format(results$mem_alloc),
  check.names = FALSE
)
options(width = 200)
print(table, row.names = FALSE, right = FALSE)
