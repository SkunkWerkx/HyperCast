#include <R.h>
#include <R_ext/Rdynload.h>
#include <Rinternals.h>
#include <stddef.h>

#include "shim.h"

static const R_CallMethodDef CALL_ENTRIES[] = {
    {"cast_text", (DL_FUNC)&C_cast_text, 9},
    {"cast_double", (DL_FUNC)&C_cast_double, 3},
    {"constraints_check", (DL_FUNC)&C_constraints_check, 2},
    {"constraints_packed", (DL_FUNC)&C_constraints_packed, 2},
    {"cast_checked", (DL_FUNC)&C_cast_checked, 5},
    {"cast_checked_double", (DL_FUNC)&C_cast_checked_double, 3},
    {"declared_decimal", (DL_FUNC)&C_declared_decimal, 2},
    {"core_guard", (DL_FUNC)&C_core_guard, 0},
    {"month_names_check", (DL_FUNC)&C_month_names_check, 1},
    {"native_version", (DL_FUNC)&C_native_version, 0},
    {NULL, NULL, 0},
};

void R_init_hypercast(DllInfo *dll) {
  if (!hypercast_core_enforces_grouping())
    Rf_error("hypercast: the linked core does not enforce grouping policies (a HyperCast core "
             "below 1.1); the package is built against HyperCast 1.1 or later, and a request "
             "is never read by 1.0's rule");
  R_registerRoutines(dll, NULL, CALL_ENTRIES, NULL, NULL);
  R_useDynamicSymbols(dll, FALSE);
  R_forceSymbols(dll, TRUE);
}
