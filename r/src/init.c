#include <R.h>
#include <R_ext/Rdynload.h>
#include <Rinternals.h>
#include <stddef.h>

#include "shim.h"

static const R_CallMethodDef CALL_ENTRIES[] = {
    {"cast_text", (DL_FUNC)&C_cast_text, 6},
    {"cast_double", (DL_FUNC)&C_cast_double, 3},
    {"constraints_check", (DL_FUNC)&C_constraints_check, 2},
    {"cast_checked", (DL_FUNC)&C_cast_checked, 4},
    {"cast_checked_double", (DL_FUNC)&C_cast_checked_double, 3},
    {"declared_decimal", (DL_FUNC)&C_declared_decimal, 2},
    {"native_version", (DL_FUNC)&C_native_version, 0},
    {NULL, NULL, 0},
};

void R_init_hypercast(DllInfo *dll) {
  R_registerRoutines(dll, NULL, CALL_ENTRIES, NULL, NULL);
  R_useDynamicSymbols(dll, FALSE);
  R_forceSymbols(dll, TRUE);
}
