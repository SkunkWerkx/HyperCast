// Stands in for the export a 1.0 core lacks, for the downgrade test (run.sh, SMOKE_CORE_10).
// hypercast.hpp may name cast_detailed only behind its version guard, so on a 1.0 core nothing
// reaches this function; if the header's guard ever let a request through, it would land here,
// and the test ends with status 99 rather than reading it by that core's rule.

#include <stdio.h>
#include <stdlib.h>

#include "hypercast.h"

int32_t cast_detailed(uint32_t door, uint32_t param, const struct HyperCastRawNumFormat *format,
                      const struct HyperCastRawDayPeriods *periods,
                      const struct HyperCastRawDateOptions *date_options, const uint8_t *vocab_ptr,
                      uintptr_t vocab_len, const uint8_t *ptr, uintptr_t len, void *out,
                      struct HyperCastRawFaultDetail *fault) {
  (void)door, (void)param, (void)format, (void)periods, (void)date_options, (void)vocab_ptr,
      (void)vocab_len, (void)ptr, (void)len, (void)out, (void)fault;
  fputs("downgrade: cast_detailed was called on a core that has none\n", stderr);
  exit(99);
}
