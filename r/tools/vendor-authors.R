# Writes inst/AUTHORS: one line per vendored crate (the dependency closure of the rcore
# wrapper, without rcore itself and the hypercast core, which is this project's own), from
# cargo metadata. Run from the package root by tools/vendor.sh, which leaves src/rust/vendor
# in place for it: the metadata is read offline against that directory, since the lock names
# the crates.io source for the core at a version that is not published while a release packs
# this package. The core has no dependencies today, so the file says so in one line; a crate
# the core comes to depend on gets its line here, and its authors a place in DESCRIPTION.
# The metadata goes through a file: on Windows `system2(stdout = TRUE)` splits captured lines
# at 8095 bytes, and the break can land inside a string.
metadata <- tempfile(fileext = ".json")
vendored <- c("--offline",
              "--config", shQuote('source.crates-io.replace-with="vendored-sources"'),
              "--config", shQuote('source.vendored-sources.directory="src/rust/vendor"'))
status <- system2("cargo", c("metadata", "--format-version", "1", "--locked", vendored,
                             "--manifest-path", "src/rust/Cargo.toml"), stdout = metadata)
if (status != 0L) stop("vendor-authors.R: cargo metadata exited with status ", status)
meta <- jsonlite::fromJSON(metadata, simplifyVector = FALSE)
pkgs <- Filter(function(p) !p$name %in% c("rcore", "hypercast"), meta$packages)
pkgs <- pkgs[order(vapply(pkgs, function(p) p$name, ""))]
field <- function(x, missing) {
  if (is.null(x) || !length(x)) missing else paste(unlist(x), collapse = ", ")
}
credit <- function(p) {
  if (is.null(p$authors) || !length(p$authors)) {
    stop("vendor-authors.R: ", p$name, " declares no authors; credit it as its license names them")
  }
  paste(unlist(p$authors), collapse = ", ")
}
lines <- vapply(pkgs, function(p) {
  sprintf("%s %s: %s (%s, %s)", p$name, p$version, credit(p),
          field(p$license, "no license field"), field(p$repository, "no repository field"))
}, "")
if (!length(lines)) {
  lines <- paste("No third-party Rust crates are vendored: src/rust/vendor.tar.xz carries only",
                 "the hypercast core, this project's own work (Skunk Werkx, MIT).")
}
dir.create("inst", showWarnings = FALSE)
writeLines(lines, "inst/AUTHORS", useBytes = TRUE)
