/*
 * The shim's .Call entry points, defined in hypercast.c and registered in init.c. The core's
 * own C ABI is include/hypercast.h, staged from the repository's include/ by
 * tools/vendor.sh.
 */
#ifndef HYPERCAST_R_SHIM_H
#define HYPERCAST_R_SHIM_H

#include <Rinternals.h>

SEXP C_cast_text(SEXP x, SEXP door, SEXP format, SEXP periods, SEXP code, SEXP native_mode,
                 SEXP options, SEXP vocab);
SEXP C_cast_double(SEXP x, SEXP door, SEXP code);
SEXP C_constraints_check(SEXP layout, SEXP native_mode);
SEXP C_cast_checked(SEXP x, SEXP layout, SEXP format, SEXP native_mode);
SEXP C_cast_checked_double(SEXP x, SEXP layout, SEXP native_mode);
SEXP C_declared_decimal(SEXP x, SEXP native_mode);
SEXP C_core_guard(void);
SEXP C_month_names_check(SEXP vocab);
SEXP C_native_version(void);

/* Whether the linked core enforces grouping policies; R_init_hypercast refuses one that doesn't. */
int hypercast_core_enforces_grouping(void);

#endif
