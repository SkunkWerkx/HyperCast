// A C program built against an unpacked release archive, the way a consumer would: through
// CMake's find_package or pkg-config, linked to the shared or the static library. It calls
// all 32 functions hypercast.h declares, checks each verdict (the value, or the reason and
// its byte span), and prints the library's version, which run.sh holds to the package's.
// CMakeLists.txt builds it as C99, C11 and, where the compiler has it, C23, warnings as
// errors.

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

// A symbol field of a layout: its bytes, then its length.
static void declare(uint8_t *bytes, uint8_t *len, const char *symbol) {
  size_t n = strlen(symbol);
  memcpy(bytes, symbol, n);
  *len = (uint8_t)n;
}

int main(void) {
  HyperCastRawFault fault = {0, 0};

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

  uint32_t version = hypercast_version();
  printf("%u.%u.%u\n", version >> 16, (version >> 8) & 0xff, version & 0xff);
  return 0;
}
