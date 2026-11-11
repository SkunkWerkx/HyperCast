// Stands in for the exports a 1.0 core lacks, for the downgrade test (run.sh, SMOKE_CORE_10).
// hypercast.hpp may name each only behind its version guard, so on a 1.0 core nothing reaches
// these functions; if the header's guard ever let a request through, it would land here, and the
// test ends with status 99 rather than reading it by that core's rule or failing to link.

#include <stdio.h>
#include <stdlib.h>

#include "hypercast.h"

static void reached(const char *name) {
  fprintf(stderr, "downgrade: %s was called on a core that has none\n", name);
  exit(99);
}

int32_t cast_detailed(uint32_t door, uint32_t param, const struct HyperCastRawNumFormat *format,
                      const struct HyperCastRawDayPeriods *periods,
                      const struct HyperCastRawDateOptions *date_options, const uint8_t *vocab_ptr,
                      uintptr_t vocab_len, const uint8_t *ptr, uintptr_t len, void *out,
                      struct HyperCastRawFaultDetail *fault) {
  (void)door, (void)param, (void)format, (void)periods, (void)date_options, (void)vocab_ptr,
      (void)vocab_len, (void)ptr, (void)len, (void)out, (void)fault;
  reached("cast_detailed");
  return -1;
}

int32_t cast_text(const uint8_t *ptr, uintptr_t len, struct HyperCastRawSpan *out,
                  struct HyperCastRawFault *fault) {
  (void)ptr, (void)len, (void)out, (void)fault;
  reached("cast_text");
  return -1;
}

int32_t cast_checked(uint32_t door, uint32_t param, const struct HyperCastRawNumFormat *format,
                     const struct HyperCastRawConstraints *constraints, const uint8_t *enum_ptr,
                     uintptr_t enum_len, const uint8_t *ptr, uintptr_t len, void *out,
                     struct HyperCastRawFault *fault, struct HyperCastRawCheck *check) {
  (void)door, (void)param, (void)format, (void)constraints, (void)enum_ptr, (void)enum_len,
      (void)ptr, (void)len, (void)out, (void)fault, (void)check;
  reached("cast_checked");
  return -1;
}

int32_t cast_checked_from_f64(uint32_t door, double value,
                              const struct HyperCastRawConstraints *constraints,
                              const uint8_t *enum_ptr, uintptr_t enum_len, void *out,
                              struct HyperCastRawFault *fault, struct HyperCastRawCheck *check) {
  (void)door, (void)value, (void)constraints, (void)enum_ptr, (void)enum_len, (void)out,
      (void)fault, (void)check;
  reached("cast_checked_from_f64");
  return -1;
}

int32_t cast_constraints_check(uint32_t door, uint32_t param,
                               const struct HyperCastRawConstraints *constraints,
                               const uint8_t *enum_ptr, uintptr_t enum_len) {
  (void)door, (void)param, (void)constraints, (void)enum_ptr, (void)enum_len;
  reached("cast_constraints_check");
  return -1;
}

int32_t cast_declared_decimal(const uint8_t *ptr, uintptr_t len, struct HyperCastDecimal *out) {
  (void)ptr, (void)len, (void)out;
  reached("cast_declared_decimal");
  return -1;
}
