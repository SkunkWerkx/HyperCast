// A C program built against an unpacked release archive, the way a consumer would: through
// CMake's find_package or pkg-config, linked to the shared or the static library. It calls
// all 36 functions hypercast.h declares, checks each verdict (the value, or the reason and
// its byte span), and prints the library's version, which run.sh holds to the package's.
// CMakeLists.txt builds it as C99, C11 and, where the compiler has it, C23, warnings as
// errors.
//
// It is also the checked example for a direct C caller of the detailed exports and the grouping
// policy: ask hypercast_version() first, and refuse a core older than the headers the program
// was built against (see `core_is_current` below), because a core without the detailed exports
// ignores the policy bits in a format's flags, so a strict format would silently read as legacy.

#include <math.h>
#include <stdio.h>
#include <string.h>

#include "hypercast.h"

#define CHECK(condition)                                                                           \
  do {                                                                                             \
    if (!(condition)) {                                                                            \
      fprintf(stderr, "smoke: %s:%d: failed: %s\n", __FILE__, __LINE__, #condition);               \
      return 1;                                                                                    \
    }                                                                                              \
  } while (0)

// The text as the doors take it: a pointer to its UTF-8 and its length in bytes.
#define UTF8(literal) (const uint8_t *)(literal), (sizeof(literal) - 1)

// The version of the headers this program was built against, packed as hypercast_version() packs
// it (major << 16 | minor << 8 | patch): the archive's own, which CMake and run.sh pass in.
// A direct C caller checks the library's version against it before it declares a strict policy
// or calls cast_detailed and its three siblings: nothing in a format's flags can ask a core that
// predates them for the new behaviour, since it ignores the bits. 1.1.0, from release 1.1, is
// 0x010100.
#ifndef HYPERCAST_SMOKE_MIN_VERSION
#error "HYPERCAST_SMOKE_MIN_VERSION: the archive's version, packed (CMakeLists.txt, run.sh)"
#endif

// The rule, the same as hypercast.hpp's: the library's major version is the headers' and its minor
// at least theirs. The patch never matters: a patch release adds no export.
static int core_is_current(uint32_t version, uint32_t built_against) {
  return (version >> 16) == (built_against >> 16) &&
         ((version >> 8) & 0xff) >= ((built_against >> 8) & 0xff);
}

// A symbol field of a layout: its bytes, then its length.
static void declare(uint8_t *bytes, uint8_t *len, const char *symbol) {
  size_t n = strlen(symbol);
  memcpy(bytes, symbol, n);
  *len = (uint8_t)n;
}

int main(void) {
  HyperCastRawFault fault = {0, 0};

  // The guard: the same major version and at least the minor, whatever the patch.
  CHECK(!core_is_current(0x010000, 0x010100) && !core_is_current(0x0100ff, 0x010100));
  CHECK(core_is_current(0x010100, 0x010100) && core_is_current(0x010203, 0x010100));
  CHECK(core_is_current(0x010100, 0x010101) && core_is_current(0x010100, 0x0101ff));
  CHECK(!core_is_current(0x020000, 0x010100) && !core_is_current(0x020100, 0x010100));
  CHECK(!core_is_current(0x010100, 0x020100) && core_is_current(0x010000, 0x010000));
  const uint32_t version = hypercast_version();
  if (!core_is_current(version, HYPERCAST_SMOKE_MIN_VERSION)) {
    fprintf(stderr, "smoke: hypercast %u.%u.%u cannot drive the headers this was built against\n",
            version >> 16, (version >> 8) & 0xff, version & 0xff);
    return 1;
  }

  // The return code is the verdict: 0 the value, 1 empty, 2 malformed, 3 out of range, with
  // the span written to `fault`; -1 a contract violation.
  uint8_t flag = 0;
  CHECK(cast_bool(UTF8(" yes "), &flag, &fault) == 0 && flag == 1);
  CHECK(cast_bool(UTF8("\tyess\n"), &flag, &fault) == 2 && fault.offset == 1 && fault.len == 4);
  CHECK(cast_bool(NULL, 0, &flag, NULL) == 1);

  uint32_t scalar = 0;
  CHECK(cast_char(UTF8("\xe2\x82\xac"), &scalar, &fault) == 0 && scalar == 0x20AC);

  // A null format is the invariant one.
  int8_t i8 = 0;
  int16_t i16 = 0;
  int32_t i32 = 0;
  int64_t i64 = 0;
  uint8_t u8 = 0;
  uint16_t u16 = 0;
  uint32_t u32 = 0;
  uint64_t u64 = 0;
  CHECK(cast_i8(UTF8("0xFF"), NULL, &i8, &fault) == 0 && i8 == -1);
  CHECK(cast_i16(UTF8("-32768"), NULL, &i16, &fault) == 0 && i16 == -32768);
  CHECK(cast_i32(UTF8("(1,234)"), NULL, &i32, &fault) == 0 && i32 == -1234);
  CHECK(cast_i64(UTF8("9223372036854775807"), NULL, &i64, &fault) == 0 && i64 == INT64_MAX);
  CHECK(cast_u8(UTF8("256"), NULL, &u8, &fault) == 3 && fault.offset == 0 && fault.len == 3);
  CHECK(cast_u16(UTF8("65535"), NULL, &u16, &fault) == 0 && u16 == 65535);
  CHECK(cast_u32(UTF8("4294967295"), NULL, &u32, &fault) == 0 && u32 == UINT32_MAX);
  CHECK(cast_u64(UTF8("18446744073709551615"), NULL, &u64, &fault) == 0 && u64 == UINT64_MAX);

  // A declared format, read in place: `,` decimal and `.` group separators, the euro.
  HyperCastRawNumFormat euro;
  memset(&euro, 0, sizeof euro);
  euro.size = HyperCastRawNumFormat_SIZE;
  euro.format.flags = HyperCastNumFormat_ALL;
  declare(euro.format.decimal_sep.bytes, &euro.format.decimal_sep.len, ",");
  declare(euro.format.group_sep.bytes, &euro.format.group_sep.len, ".");
  declare(euro.format.currency.bytes, &euro.format.currency.len, "\xe2\x82\xac");

  float f32 = 0;
  double f64 = 0;
  CHECK(cast_f32(UTF8("1.5"), NULL, &f32, &fault) == 0 && f32 == 1.5f);
  CHECK(cast_f64(UTF8("1.234,5 \xe2\x82\xac"), &euro, &f64, &fault) == 0 && f64 == 1234.5);
  CHECK(cast_f64(UTF8("1e400"), NULL, &f64, &fault) == 3 && fault.offset == 0 && fault.len == 5);

  HyperCastDecimal decimal;
  CHECK(cast_decimal(UTF8("-1.234,50"), &euro, &decimal, &fault) == 0);
  CHECK(decimal.lo == 12345 && decimal.hi == 0 && decimal.scale == 1 && decimal.negative);

  // A format the core refuses (equal separators) is a contract violation, not a verdict.
  HyperCastRawNumFormat refused = euro;
  declare(refused.format.group_sep.bytes, &refused.format.group_sep.len, ",");
  CHECK(cast_i32(UTF8("1"), &refused, &i32, &fault) == -1);
  // So is a decimal separator that is white space (U+00A0), which no input could match.
  refused = euro;
  declare(refused.format.decimal_sep.bytes, &refused.format.decimal_sep.len, "\xc2\xa0");
  CHECK(cast_i32(UTF8("1"), &refused, &i32, &fault) == -1);
  // So is a currency symbol starting with white space. Inside one it is allowed (CLDR's
  // `F CFA`, with U+202F), and there ' ', U+00A0 and U+202F read each other.
  refused = euro;
  declare(refused.format.currency.bytes, &refused.format.currency.len, "\xc2\xa0\xe2\x82\xac");
  CHECK(cast_i32(UTF8("1"), &refused, &i32, &fault) == -1);
  // In octal, so the `CFA` after a space isn't read as hex.
  HyperCastRawNumFormat cfa = euro;
  declare(cfa.format.currency.bytes, &cfa.format.currency.len, "F\342\200\257CFA");
  CHECK(cast_i32(UTF8("1.000 F\302\240CFA"), &cfa, &i32, &fault) == 0 && i32 == 1000);

  static const uint8_t uuid_bytes[16] = {0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                                         0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10};
  uint8_t uuid[16];
  CHECK(cast_uuid(UTF8("01020304-0506-0708-090a-0b0c0d0e0f10"), uuid, &fault) == 0);
  CHECK(memcmp(uuid, uuid_bytes, sizeof uuid) == 0);

  HyperCastTimestamp instant;
  CHECK(cast_timestamp(UTF8("2022-02-22T19:22:22Z"), &instant, &fault) == 0);
  CHECK(instant.seconds == 1645557742 && instant.nanos == 0);
  CHECK(cast_unix(UTF8("1700000000123"), 2, &instant, &fault) == 0);
  CHECK(instant.seconds == 1700000000 && instant.nanos == 123000000);
  CHECK(cast_unix(UTF8("1700000000"), 9, &instant, &fault) == -1);
  CHECK(cast_excel_serial(UTF8("25569"), 1, &instant, &fault) == 0);
  CHECK(instant.seconds == 0 && instant.nanos == 0);

  HyperCastDate date;
  CHECK(cast_date(UTF8("2026-01-02"), &date, &fault) == 0);
  CHECK(date.year == 2026 && date.month == 1 && date.day == 2);
  CHECK(cast_date_ordered(UTF8("1/7/2026"), 3, &date, &fault) == 0);
  CHECK(date.year == 2026 && date.month == 7 && date.day == 1);
  CHECK(cast_date_ordered(UTF8("13/1/2026"), 2, &date, &fault) == 3);
  CHECK(fault.offset == 0 && fault.len == 2);

  // Declared day periods beside the English AM/PM, which are always read.
  HyperCastCivilDateTime civil;
  CHECK(cast_datetime(UTF8("1/7/2026 3:04 PM"), 2, NULL, &civil, &fault) == 0);
  CHECK(civil.date.month == 1 && civil.date.day == 7 && civil.nanos_of_day == 54240000000000ULL);
  HyperCastRawDayPeriods periods;
  memset(&periods, 0, sizeof periods);
  periods.size = HyperCastRawDayPeriods_SIZE;
  declare(periods.periods.am.bytes, &periods.periods.am.len, "vorm.");
  declare(periods.periods.pm.bytes, &periods.periods.pm.len, "nachm.");
  CHECK(cast_datetime(UTF8("7.1.2026 3:04 nachm."), 3, &periods, &civil, &fault) == 0);
  CHECK(civil.date.month == 1 && civil.date.day == 7 && civil.nanos_of_day == 54240000000000ULL);
  // A designator with white space at an edge (U+202F) is a contract violation.
  declare(periods.periods.pm.bytes, &periods.periods.pm.len, "\xe2\x80\xafPM");
  CHECK(cast_datetime(UTF8("7.1.2026 3:04 PM"), 3, &periods, &civil, &fault) == -1);

  uint64_t nanos = 0;
  CHECK(cast_time(UTF8("15:04:05"), &nanos, &fault) == 0 && nanos == 54245000000000ULL);

  HyperCastDuration span;
  CHECK(cast_duration(UTF8("PT1H30M"), &span, &fault) == 0 && span.seconds == 5400 &&
        span.nanos == 0);

  // The typed doors read a double; their fault span is always empty.
  CHECK(cast_decimal_from_f64(0.1, &decimal, &fault) == 0);
  CHECK(decimal.lo == 1 && decimal.scale == 1 && !decimal.negative);
  fault.offset = 7;
  fault.len = 7;
  CHECK(cast_decimal_from_f64(1e300, &decimal, &fault) == 3 && fault.offset == 0 && fault.len == 0);
  CHECK(cast_excel_serial_from_f64(45292.75, 1, &civil, &fault) == 0);
  CHECK(civil.date.year == 2024 && civil.date.month == 1 && civil.date.day == 1);
  CHECK(civil.nanos_of_day == 64800000000000ULL);
  CHECK(cast_excel_time(0.75, &nanos, &fault) == 0 && nanos == 64800000000000ULL);
  CHECK(cast_excel_duration(1.5, &span, &fault) == 0 && span.seconds == 129600 && span.nanos == 0);

  // The validation constraints. A declaration is a 72-byte layout, every field zero but the
  // size declaring nothing; a checked door adds code 4, a violation, and reports through the
  // nullable `check`.
  HyperCastRawConstraints latitude;
  memset(&latitude, 0, sizeof latitude);
  latitude.size = HyperCastRawConstraints_SIZE;
  latitude.flags = HyperCastRawConstraints_HAS_MINIMUM | HyperCastRawConstraints_HAS_MAXIMUM |
                   HyperCastRawConstraints_HAS_MULTIPLE_OF;
  latitude.rounding = 1; // midpoint
  latitude.lower.lo = 90;
  latitude.lower.negative = true;
  latitude.upper.lo = 90;
  latitude.multiple_of.lo = 1;
  latitude.multiple_of.scale = 6;
  CHECK(cast_constraints_check(11, 0, &latitude, NULL, 0) == 0);

  HyperCastRawCheck report = {7, 7, 7};
  CHECK(cast_checked(11, 0, NULL, &latitude, NULL, 0, UTF8("45.1234567"), &f64, &fault, &report) ==
        0);
  CHECK(f64 == 45.123457 && report.adjusted == 1 && report.enum_index == UINT32_MAX);
  CHECK(cast_checked(11, 0, NULL, &latitude, NULL, 0, UTF8("91"), &f64, &fault, &report) == 4);
  CHECK(report.keyword == 3 && fault.offset == 0 && fault.len == 2);
  CHECK(cast_checked_from_f64(11, -90.5, &latitude, NULL, 0, &f64, &fault, &report) == 4);
  CHECK(report.keyword == 1 && fault.offset == 0 && fault.len == 0);
  // A float door judges the float it delivers too: 0.99999999999 is 1.0 at f32, past
  // exclusiveMaximum 1.
  HyperCastRawConstraints below_one;
  memset(&below_one, 0, sizeof below_one);
  below_one.size = HyperCastRawConstraints_SIZE;
  below_one.flags = HyperCastRawConstraints_HAS_EXCLUSIVE_MAXIMUM;
  below_one.upper.lo = 1;
  CHECK(cast_checked(10, 0, NULL, &below_one, NULL, 0, UTF8("0.99999999999"), &f32, &fault,
                     &report) == 4);
  CHECK(report.keyword == 4 && fault.offset == 0 && fault.len == 13);

  // A text `enum`, read under HAS_ENUM: each entry a little-endian uint32_t length, then its
  // UTF-8. The span is the matched input, and `enum_index` names the entry.
  static const uint8_t sexes[] = {6,   0, 0, 0, 'F', 'e', 'm', 'a', 'l',
                                  'e', 4, 0, 0, 0,   'M', 'a', 'l', 'e'};
  HyperCastRawConstraints trimmed;
  memset(&trimmed, 0, sizeof trimmed);
  trimmed.size = HyperCastRawConstraints_SIZE;
  trimmed.flags = HyperCastRawConstraints_TRIM | HyperCastRawConstraints_HAS_ENUM;
  HyperCastRawSpan matched = {0, 0};
  CHECK(cast_checked(18, 0, NULL, &trimmed, sexes, sizeof sexes, UTF8(" MALE "), &matched, &fault,
                     &report) == 0);
  CHECK(matched.offset == 1 && matched.len == 4 && report.enum_index == 1);
  // A duplicate under case folding is refused, by code; a size the core does not know is a
  // malformed layout here and a contract violation at a checked door.
  static const uint8_t twice[] = {4, 0, 0, 0, 'M', 'a', 'l', 'e', 4, 0, 0, 0, 'M', 'A', 'L', 'E'};
  CHECK(cast_constraints_check(18, 0, &trimmed, twice, sizeof twice) == 10);
  // An `enum` with no entries, and under trim an entry with White_Space at an edge, which no
  // input can match; a list passed without HAS_ENUM is a malformed layout.
  CHECK(cast_constraints_check(18, 0, &trimmed, NULL, 0) == 12);
  static const uint8_t padded[] = {5, 0, 0, 0, ' ', 'M', 'a', 'l', 'e'};
  CHECK(cast_constraints_check(18, 0, &trimmed, padded, sizeof padded) == 13);
  trimmed.flags = HyperCastRawConstraints_TRIM;
  CHECK(cast_constraints_check(18, 0, &trimmed, sexes, sizeof sexes) == 11);
  trimmed.size = 0;
  CHECK(cast_constraints_check(18, 0, &trimmed, NULL, 0) == 11);
  CHECK(cast_checked(18, 0, NULL, &trimmed, NULL, 0, UTF8("a"), &matched, &fault, NULL) == -1);

  HyperCastRawSpan whole = {0, 0};
  CHECK(cast_text(UTF8("\xf0\x9f\x91\x8d"), &whole, &fault) == 0 && whole.offset == 0 &&
        whole.len == 4);
  CHECK(cast_text(UTF8("ab\xff"), &whole, &fault) == 2 && fault.offset == 2 && fault.len == 1);

  HyperCastDecimal declared = {0, 0, 0, false};
  CHECK(cast_declared_decimal(UTF8("1.50"), &declared) == 0 && declared.lo == 15 &&
        declared.hi == 0 && declared.scale == 1 && !declared.negative);
  CHECK(cast_declared_decimal(UTF8("1e-29"), &declared) == 8 && declared.lo == 15 &&
        declared.scale == 1);
  CHECK(cast_declared_decimal(UTF8("1"), NULL) == -1);

  // The detailed exports reach every door by its code and write a HyperCastRawFaultDetail: the
  // span, and `detail`, which names what failed. A null format is the invariant one with
  // legacy grouping; a declared one carries its grouping policy in flag bits 8 and 9.
  HyperCastRawFaultDetail detail = {HyperCastRawFaultDetail_SIZE, 0, 0, 0};
  HyperCastRawNumFormat strict;
  memset(&strict, 0, sizeof strict);
  strict.size = HyperCastRawNumFormat_SIZE;
  strict.format.flags = HyperCastNumFormat_ALL | HyperCastNumFormat_GROUPING_WESTERN;
  declare(strict.format.decimal_sep.bytes, &strict.format.decimal_sep.len, ".");
  declare(strict.format.group_sep.bytes, &strict.format.group_sep.len, ",");

  // The 1.0 exports ignore flag bits 8 and up and always read legacy grouping, so the strict
  // format they were handed still reads `12,34` as 1234; the detailed export judges it.
  i32 = 0;
  CHECK(cast_i32(UTF8("12,34"), &strict, &i32, &fault) == 0 && i32 == 1234);
  i32 = 0;
  CHECK(cast_detailed(4, 0, NULL, NULL, NULL, NULL, 0, UTF8("12,34"), &i32, &detail) == 0 &&
        i32 == 1234);
  i32 = 0;
  CHECK(cast_detailed(4, 0, &strict, NULL, NULL, NULL, 0, UTF8("12,34"), &i32, &detail) == 2);
  CHECK(i32 == 0 && detail.offset == 2 && detail.len == 3 && detail.detail == 11); /* group_size */
  CHECK(detail.size == HyperCastRawFaultDetail_SIZE);
  CHECK(cast_detailed(4, 0, &strict, NULL, NULL, NULL, 0, UTF8("1,234"), &i32, &detail) == 0 &&
        i32 == 1234);
  /* Indian grouping, the policy in bits 8 and 9 replaced, never combined. */
  HyperCastRawNumFormat indian = strict;
  indian.format.flags = (strict.format.flags & ~(uint32_t)HyperCastNumFormat_GROUPING_POLICY) |
                        HyperCastNumFormat_GROUPING_INDIAN;
  CHECK(cast_detailed(4, 0, &indian, NULL, NULL, NULL, 0, UTF8("12,34,567"), &i32, &detail) == 0 &&
        i32 == 1234567);
  /* Policy 3 and a flag bit nobody defined are contract violations, and write nothing. */
  HyperCastRawNumFormat bad = strict;
  bad.format.flags |= HyperCastNumFormat_GROUPING_POLICY;
  i32 = 77;
  detail.offset = detail.len = detail.detail = 7;
  CHECK(cast_detailed(4, 0, &bad, NULL, NULL, NULL, 0, UTF8("1"), &i32, &detail) == -1);
  CHECK(i32 == 77 && detail.offset == 7 && detail.len == 7 && detail.detail == 7);
  bad = strict;
  bad.format.flags |= 1u << 10;
  CHECK(cast_detailed(4, 0, &bad, NULL, NULL, NULL, 0, UTF8("1"), &i32, &detail) == -1);
  CHECK(i32 == 77 && detail.detail == 7);
  /* ...but the 1.0 export reads only bits 0 to 7, so it is no violation there. */
  CHECK(cast_i32(UTF8("1"), &bad, &i32, &fault) == 0 && i32 == 1);

  /* A door valid for one export is not valid for another: nothing is written. */
  i32 = 77;
  double real = 7;
  CHECK(cast_detailed_from_f64(18, 0, 1.0, &real, &detail) == -1 && real == 7 &&
        detail.detail == 7);
  CHECK(cast_detailed(24, 0, NULL, NULL, NULL, NULL, 0, UTF8("1"), &i32, &detail) == -1);
  CHECK(cast_detailed(4, 1, NULL, NULL, NULL, NULL, 0, UTF8("1"), &i32, &detail) == -1);
  CHECK(i32 == 77 && detail.detail == 7);
  /* The fault's size: under 16 or not a multiple of 4 is refused, more than 16 is accepted
     and its tail never written, and a null fault is allowed. */
  uint32_t big[6] = {24, 7, 7, 7, 0xaaaaaaaau, 0xbbbbbbbbu};
  HyperCastRawFaultDetail *as_fault = (HyperCastRawFaultDetail *)(void *)big;
  CHECK(cast_detailed(4, 0, &strict, NULL, NULL, NULL, 0, UTF8("12,34"), &i32, as_fault) == 2);
  CHECK(big[0] == 24 && big[1] == 2 && big[2] == 3 && big[3] == 11);
  CHECK(big[4] == 0xaaaaaaaau && big[5] == 0xbbbbbbbbu);
  detail.size = 12;
  CHECK(cast_detailed(4, 0, &strict, NULL, NULL, NULL, 0, UTF8("12,34"), &i32, &detail) == -1);
  detail.size = 18;
  CHECK(cast_detailed(4, 0, &strict, NULL, NULL, NULL, 0, UTF8("12,34"), &i32, &detail) == -1);
  detail.size = HyperCastRawFaultDetail_SIZE;
  CHECK(cast_detailed(4, 0, &strict, NULL, NULL, NULL, 0, UTF8("12,34"), &i32, NULL) == 2);
  /* `out` need not be aligned. */
  uint8_t unaligned[sizeof(int32_t) + 1] = {0};
  CHECK(cast_detailed(4, 0, NULL, NULL, NULL, NULL, 0, UTF8("(1,234)"), unaligned + 1, &detail) ==
        0);
  memcpy(&i32, unaligned + 1, sizeof i32);
  CHECK(i32 == -1234);
  /* A door that ignores a pointer never reads it: a poisoned one is fine. */
  CHECK(cast_detailed(12, 0, (const HyperCastRawNumFormat *)(uintptr_t)8,
                      (const HyperCastRawDayPeriods *)(uintptr_t)8,
                      (const HyperCastRawDateOptions *)(uintptr_t)8, (const uint8_t *)(uintptr_t)8,
                      0, UTF8("01020304-0506-0708-090a-0b0c0d0e0f10"), uuid, &detail) == 0);

  /* The char door, 23, and what its failures name. */
  CHECK(cast_detailed(23, 0, NULL, NULL, NULL, NULL, 0, UTF8("U+20AC"), &scalar, &detail) == 0 &&
        scalar == 0x20AC);
  CHECK(cast_detailed(23, 0, NULL, NULL, NULL, NULL, 0, UTF8("U+D800"), &scalar, &detail) == 3);
  CHECK(detail.detail == 32); /* not_a_scalar */
  CHECK(cast_detailed(1, 0, NULL, NULL, NULL, NULL, 0, NULL, 0, &flag, &detail) == 1 &&
        detail.detail == 1 && detail.len == 0); /* blank */
  /* A code this header's reader doesn't know is still a failure: its reason and span stand. */
  CHECK(cast_detailed(4, 0, NULL, NULL, NULL, NULL, 0, UTF8("12a"), &i32, &detail) == 2 &&
        detail.offset == 2 && detail.len == 1 && detail.detail == 2);

  /* The date options and a month vocabulary, which only the date doors read. */
  HyperCastRawDateOptions compact = {HyperCastRawDateOptions_SIZE, 1, 0};
  HyperCastRawDateOptions flags_only = {HyperCastRawDateOptions_MIN_SIZE, 1, 0};
  CHECK(cast_detailed(15, 0, NULL, NULL, NULL, NULL, 0, UTF8("20260107"), &date, &detail) == 2);
  CHECK(detail.detail == 16); /* wrong_length */
  CHECK(cast_detailed(15, 0, NULL, NULL, &compact, NULL, 0, UTF8("20260107"), &date, &detail) == 0);
  CHECK(date.year == 2026 && date.month == 1 && date.day == 7);
  CHECK(cast_detailed(15, 0, NULL, NULL, (const HyperCastRawDateOptions *)&flags_only, NULL, 0,
                      UTF8("20260107"), &date, &detail) == 0);
  HyperCastRawDateOptions bad_options = {HyperCastRawDateOptions_SIZE, 8, 0}; /* an undefined bit */
  CHECK(cast_detailed(15, 0, NULL, NULL, &bad_options, NULL, 0, UTF8("2026-01-07"), &date,
                      &detail) == -1);
  bad_options.flags = 0;
  bad_options.pivot = 99;
  CHECK(cast_detailed(15, 0, NULL, NULL, &bad_options, NULL, 0, UTF8("2026-01-07"), &date,
                      &detail) == -1);
  bad_options.size = HyperCastRawDateOptions_SIZE + 4; /* a layout this core doesn't know */
  bad_options.pivot = 0;
  CHECK(cast_detailed(15, 0, NULL, NULL, &bad_options, NULL, 0, UTF8("2026-01-07"), &date,
                      &detail) == -1);
  /* Twelve months, each entry a month byte, a little-endian length, then UTF-8. */
  static const char *const months[12] = {"Jan", "Feb", "Mar", "Apr", "May", "Jun",
                                         "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"};
  uint8_t vocabulary[12 * 8];
  size_t used = 0;
  for (unsigned month = 0; month < 12; ++month) {
    size_t length = strlen(months[month]);
    vocabulary[used++] = (uint8_t)(month + 1);
    vocabulary[used++] = (uint8_t)length;
    vocabulary[used++] = 0;
    vocabulary[used++] = 0;
    vocabulary[used++] = 0;
    memcpy(vocabulary + used, months[month], length);
    used += length;
  }
  HyperCastRawDateOptions pivot = {HyperCastRawDateOptions_SIZE, 0, 2029};
  CHECK(cast_detailed(20, 3, NULL, NULL, &pivot, vocabulary, used, UTF8("07-Jan-26"), &date,
                      &detail) == 0);
  CHECK(date.year == 2026 && date.month == 1 && date.day == 7);
  CHECK(cast_detailed(20, 3, NULL, NULL, NULL, vocabulary, used, UTF8("7-Jan-2026"), &date,
                      &detail) == 0); /* the vocabulary is independent of the options */
  CHECK(cast_detailed(20, 3, NULL, NULL, NULL, NULL, 0, UTF8("7-Jan-2026"), &date, &detail) == 2);
  CHECK(cast_detailed(20, 3, NULL, NULL, NULL, NULL, 5, UTF8("7-Jan-2026"), &date, &detail) == -1);
  CHECK(cast_detailed(20, 3, NULL, NULL, NULL, vocabulary, used - 1, UTF8("7-Jan-2026"), &date,
                      &detail) == -1);
  CHECK(cast_detailed(20, 3, NULL, NULL, NULL, vocabulary, 0, UTF8("7-Jan-2026"), &date, &detail) ==
        2); /* a length of 0 is none, whatever the pointer */
  CHECK(cast_detailed(21, 3, NULL, NULL, &pivot, vocabulary, used, UTF8("07-Jan-26 15:04"), &civil,
                      &detail) == 0);
  CHECK(civil.date.year == 2026 && civil.nanos_of_day == 54240000000000ULL);

  /* The typed doors of the detailed exports: a double in, an empty span on a failure. */
  CHECK(cast_detailed_from_f64(19, 0, 0.1, &decimal, &detail) == 0 && decimal.lo == 1 &&
        decimal.scale == 1);
  CHECK(cast_detailed_from_f64(19, 0, 1e300, &decimal, &detail) == 3 && detail.detail == 25 &&
        detail.offset == 0 && detail.len == 0); /* too_large */
  CHECK(cast_detailed_from_f64(19, 0, NAN, &decimal, &detail) == 2 && detail.detail == 22);
  CHECK(cast_detailed_from_f64(22, 1, 45292.75, &civil, &detail) == 0 && civil.date.year == 2024);
  CHECK(cast_detailed_from_f64(22, 3, 45292.75, &civil, &detail) == -1);
  CHECK(cast_detailed_from_f64(16, 0, 0.75, &nanos, &detail) == 0 && nanos == 64800000000000ULL);
  CHECK(cast_detailed_from_f64(17, 0, 1.5, &span, &detail) == 0 && span.seconds == 129600);
  CHECK(cast_detailed_from_f64(4, 0, 1.0, &i32, &detail) == -1); /* the integers are Rust-only */

  /* The checked pair: a violation writes the span and `detail` 0, and the `check` its keyword. */
  report.keyword = report.adjusted = report.enum_index = 7;
  CHECK(cast_checked_detailed(11, 0, NULL, &latitude, NULL, 0, UTF8("91"), &f64, &detail,
                              &report) == 4);
  CHECK(report.keyword == 3 && report.adjusted == 0 && report.enum_index == UINT32_MAX);
  CHECK(detail.offset == 0 && detail.len == 2 && detail.detail == 0);
  CHECK(cast_checked_detailed(11, 0, NULL, &latitude, NULL, 0, UTF8("abc"), &f64, &detail,
                              &report) == 2 &&
        detail.detail == 2);
  CHECK(cast_checked_detailed(11, 0, NULL, &latitude, NULL, 0, UTF8("45.1234567"), &f64, &detail,
                              &report) == 0 &&
        f64 == 45.123457 && report.adjusted == 1);
  CHECK(cast_checked_detailed(11, 1, NULL, &latitude, NULL, 0, UTF8("1"), &f64, &detail, &report) ==
        -1);
  HyperCastRawConstraints cap;
  memset(&cap, 0, sizeof cap);
  cap.size = HyperCastRawConstraints_SIZE;
  cap.flags = HyperCastRawConstraints_HAS_MAXIMUM;
  cap.upper.lo = 1000;
  CHECK(cast_checked_detailed(4, 0, &strict, &cap, NULL, 0, UTF8("12,34"), &i32, &detail,
                              &report) == 2 &&
        detail.detail == 11);
  CHECK(cast_checked_detailed(4, 0, &strict, &cap, NULL, 0, UTF8("1,234"), &i32, &detail,
                              &report) == 4 &&
        report.keyword == 3);
  /* The 1.0 export reads legacy whatever the flags say: 1234, past the maximum of 1000. */
  CHECK(cast_checked(4, 0, &strict, &cap, NULL, 0, UTF8("12,34"), &i32, &fault, &report) == 4 &&
        report.keyword == 3);
  CHECK(cast_checked_detailed_from_f64(11, 0, &latitude, NULL, 0, -90.5, &f64, &detail, &report) ==
            4 &&
        report.keyword == 1 && detail.len == 0 && detail.detail == 0);
  CHECK(cast_checked_detailed_from_f64(11, 0, &latitude, NULL, 0, NAN, &f64, &detail, &report) ==
            2 &&
        detail.detail == 22);
  CHECK(cast_checked_detailed_from_f64(11, 2, &latitude, NULL, 0, 1.0, &f64, &detail, &report) ==
        -1);
  CHECK(cast_checked_detailed(18, 0, NULL, &trimmed, NULL, 0, UTF8("a"), &matched, &detail, NULL) ==
        -1); /* the unbuilt declaration: size 0 */

  printf("%u.%u.%u\n", version >> 16, (version >> 8) & 0xff, version & 0xff);
  return 0;
}
