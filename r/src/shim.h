/*
 * The shim's .Call entry points, defined in hypercast.c and registered in init.c. The core's
 * own C ABI is include/hypercast.h, staged from the repository's include/ by
 * tools/vendor.sh.
 */
#ifndef HYPERCAST_R_SHIM_H
#define HYPERCAST_R_SHIM_H

#include <Rinternals.h>

SEXP C_cast_text(SEXP x, SEXP door, SEXP format, SEXP periods, SEXP code, SEXP native_mode);
SEXP C_cast_double(SEXP x, SEXP door, SEXP code);
SEXP C_native_version(void);

#endif
