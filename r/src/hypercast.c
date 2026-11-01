#include <R.h>
#include <Rinternals.h>
#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

/* The core's own header, staged from the checkout's include/ by tools/vendor.sh. */
#include "include/hypercast.h"
#include "shim.h"

/*
 * The doors, vectorised. Every element of `x` is one call into the core, and its verdict
 * becomes one row of the columns this file returns: the value (NA where the element
 * faulted), ok, the reason code (1 empty, 2 malformed, 3 out of range), and the fault's
 * offset and length in the caller's units. An NA element is no call at all: every column
 * is NA in its row. Nothing here raises an R error for any element of `x`; the only errors
 * are caller bugs R validated before the call (and the core's contract-violation code,
 * which that validation makes unreachable), and an interrupt.
 *
 * Every loop lets the user interrupt once per 2^20 elements. R_CheckUserInterrupt can
 * longjmp out: that is safe because the only allocations are PROTECTed R vectors, which R
 * unwinds, and translateCharUTF8's R_alloc scratch, released per element with vmaxset.
 * Nothing is malloc'd.
 */

static inline void interrupt_point(R_xlen_t i) {
  if ((i & 0xFFFFF) == 0xFFFFF)
    R_CheckUserInterrupt();
}

/* ---- the doors ---- */

enum door {
  BOOL_DOOR,
  CHAR_DOOR,
  I8_DOOR,
  I16_DOOR,
  I32_DOOR,
  I64_DOOR,
  U8_DOOR,
  U16_DOOR,
  U32_DOOR,
  U64_DOOR,
  F32_DOOR,
  F64_DOOR,
  DECIMAL_DOOR,
  UUID_DOOR,
  TIMESTAMP_DOOR,
  UNIX_DOOR,
  EXCEL_SERIAL_DOOR,
  DATE_DOOR,
  DATE_ORDERED_DOOR,
  DATETIME_DOOR,
  TIME_DOOR,
  DURATION_DOOR,
  DECIMAL_FROM_DOUBLE_DOOR,
  EXCEL_SERIAL_FROM_DOUBLE_DOOR,
  EXCEL_TIME_DOOR,
  EXCEL_DURATION_DOOR,
  DOOR_COUNT
};

/* What a door's value becomes in R; each kind has its own set of columns. */
enum kind {
  LOGICAL_KIND,  /* value: logical */
  INTEGER_KIND,  /* value: integer */
  DOUBLE_KIND,   /* value: double */
  STRING_KIND,   /* value: character */
  INSTANT_KIND,  /* value: double seconds (POSIXct in R); seconds, nanos */
  DATE_KIND,     /* value: double days (Date in R) */
  CIVIL_KIND,    /* value: character wall clock; date (days), nanos_of_day */
  TIME_KIND,     /* value: double seconds (difftime in R); nanos */
  DURATION_KIND, /* value: double seconds (difftime in R); seconds, nanos */
};

static const struct {
  const char *name;
  enum kind kind;
} DOORS[DOOR_COUNT] = {
    [BOOL_DOOR] = {"bool", LOGICAL_KIND},
    [CHAR_DOOR] = {"char", STRING_KIND},
    [I8_DOOR] = {"i8", INTEGER_KIND},
    [I16_DOOR] = {"i16", INTEGER_KIND},
    [I32_DOOR] = {"i32", DOUBLE_KIND},
    [I64_DOOR] = {"i64", STRING_KIND},
    [U8_DOOR] = {"u8", INTEGER_KIND},
    [U16_DOOR] = {"u16", INTEGER_KIND},
    [U32_DOOR] = {"u32", DOUBLE_KIND},
    [U64_DOOR] = {"u64", STRING_KIND},
    [F32_DOOR] = {"f32", DOUBLE_KIND},
    [F64_DOOR] = {"f64", DOUBLE_KIND},
    [DECIMAL_DOOR] = {"decimal", STRING_KIND},
    [UUID_DOOR] = {"uuid", STRING_KIND},
    [TIMESTAMP_DOOR] = {"timestamp", INSTANT_KIND},
    [UNIX_DOOR] = {"unix", INSTANT_KIND},
    [EXCEL_SERIAL_DOOR] = {"excel_serial", INSTANT_KIND},
    [DATE_DOOR] = {"date", DATE_KIND},
    [DATE_ORDERED_DOOR] = {"date_ordered", DATE_KIND},
    [DATETIME_DOOR] = {"datetime", CIVIL_KIND},
    [TIME_DOOR] = {"time", TIME_KIND},
    [DURATION_DOOR] = {"duration", DURATION_KIND},
    [DECIMAL_FROM_DOUBLE_DOOR] = {"decimal_from_double", STRING_KIND},
    [EXCEL_SERIAL_FROM_DOUBLE_DOOR] = {"excel_serial_from_double", CIVIL_KIND},
    [EXCEL_TIME_DOOR] = {"excel_time", TIME_KIND},
    [EXCEL_DURATION_DOOR] = {"excel_duration", DURATION_KIND},
};

/* Everything a door can write. */
union value {
  uint8_t boolean;
  uint32_t scalar;
  int8_t i8;
  int16_t i16;
  int32_t i32;
  int64_t i64;
  uint8_t u8;
  uint16_t u16;
  uint32_t u32;
  uint64_t u64;
  float f32;
  double f64;
  HyperCastDecimal decimal;
  uint8_t uuid[16];
  HyperCastTimestamp timestamp;
  HyperCastDate date;
  HyperCastCivilDateTime civil;
  uint64_t nanos;
  HyperCastDuration duration;
};

/* The declarations a door reads beside its input, decoded once per call. */
struct declared {
  HyperCastRawNumFormat format;
  HyperCastRawDayPeriods periods;
  int has_periods;
  uint32_t code; /* the precision, epoch or date order */
};

static int32_t text_door(enum door door, const uint8_t *p, uintptr_t n, const struct declared *d,
                         union value *out, HyperCastRawFault *fault) {
  const HyperCastRawNumFormat *f = &d->format;
  switch (door) {
  case BOOL_DOOR:
    return cast_bool(p, n, &out->boolean, fault);
  case CHAR_DOOR:
    return cast_char(p, n, &out->scalar, fault);
  case I8_DOOR:
    return cast_i8(p, n, f, &out->i8, fault);
  case I16_DOOR:
    return cast_i16(p, n, f, &out->i16, fault);
  case I32_DOOR:
    return cast_i32(p, n, f, &out->i32, fault);
  case I64_DOOR:
    return cast_i64(p, n, f, &out->i64, fault);
  case U8_DOOR:
    return cast_u8(p, n, f, &out->u8, fault);
  case U16_DOOR:
    return cast_u16(p, n, f, &out->u16, fault);
  case U32_DOOR:
    return cast_u32(p, n, f, &out->u32, fault);
  case U64_DOOR:
    return cast_u64(p, n, f, &out->u64, fault);
  case F32_DOOR:
    return cast_f32(p, n, f, &out->f32, fault);
  case F64_DOOR:
    return cast_f64(p, n, f, &out->f64, fault);
  case DECIMAL_DOOR:
    return cast_decimal(p, n, f, &out->decimal, fault);
  case UUID_DOOR:
    return cast_uuid(p, n, out->uuid, fault);
  case TIMESTAMP_DOOR:
    return cast_timestamp(p, n, &out->timestamp, fault);
  case UNIX_DOOR:
    return cast_unix(p, n, d->code, &out->timestamp, fault);
  case EXCEL_SERIAL_DOOR:
    return cast_excel_serial(p, n, d->code, &out->timestamp, fault);
  case DATE_DOOR:
    return cast_date(p, n, &out->date, fault);
  case DATE_ORDERED_DOOR:
    return cast_date_ordered(p, n, d->code, &out->date, fault);
  case DATETIME_DOOR:
    return cast_datetime(p, n, d->code, d->has_periods ? &d->periods : NULL, &out->civil, fault);
  case TIME_DOOR:
    return cast_time(p, n, &out->nanos, fault);
  case DURATION_DOOR:
    return cast_duration(p, n, &out->duration, fault);
  default:
    return -1;
  }
}

static int32_t double_door(enum door door, double v, const struct declared *d, union value *out,
                           HyperCastRawFault *fault) {
  switch (door) {
  case DECIMAL_FROM_DOUBLE_DOOR:
    return cast_decimal_from_f64(v, &out->decimal, fault);
  case EXCEL_SERIAL_FROM_DOUBLE_DOOR:
    return cast_excel_serial_from_f64(v, d->code, &out->civil, fault);
  case EXCEL_TIME_DOOR:
    return cast_excel_time(v, &out->nanos, fault);
  case EXCEL_DURATION_DOOR:
    return cast_excel_duration(v, &out->duration, fault);
  default:
    return -1;
  }
}

/* ---- declarations: decoded from the little-endian bytes R packed ---- */

static uint32_t le32(const uint8_t *b) {
  return (uint32_t)b[0] | (uint32_t)b[1] << 8 | (uint32_t)b[2] << 16 | (uint32_t)b[3] << 24;
}

/*
 * The 92-byte RawNumFormat R packs (num_format()'s `packed`): three little-endian u32s, then
 * the symbols byte for byte, which is the struct's own layout from offset 12 on. Decoded
 * field by field rather than copied whole, so a big-endian host reads the words right.
 */
_Static_assert(sizeof(HyperCastRawNumFormat) == HyperCastRawNumFormat_SIZE, "RawNumFormat");
_Static_assert(offsetof(HyperCastRawNumFormat, format.decimal_sep) == 12, "decimal_sep");
_Static_assert(offsetof(HyperCastRawNumFormat, format.currency) == 75, "currency");
static void read_format(SEXP packed, HyperCastRawNumFormat *out) {
  const uint8_t *b = RAW(packed);
  out->size = le32(b);
  out->format.flags = le32(b + 4);
  out->format.digits = le32(b + 8);
  memcpy((uint8_t *)out + 12, b + 12, HyperCastRawNumFormat_SIZE - 12);
}

/* The 40-byte RawDayPeriods R packs: a little-endian size word, the two designators, padding. */
_Static_assert(sizeof(HyperCastRawDayPeriods) == HyperCastRawDayPeriods_SIZE, "RawDayPeriods");
_Static_assert(offsetof(HyperCastRawDayPeriods, periods.am) == 4, "am");
static void read_periods(SEXP packed, HyperCastRawDayPeriods *out) {
  const uint8_t *b = RAW(packed);
  out->size = le32(b);
  memcpy((uint8_t *)out + 4, b + 4, HyperCastRawDayPeriods_SIZE - 4);
}

static enum door door_arg(SEXP name) {
  if (TYPEOF(name) == STRSXP && XLENGTH(name) == 1) {
    const char *s = CHAR(STRING_ELT(name, 0));
    for (int i = 0; i < DOOR_COUNT; i++) {
      if (strcmp(s, DOORS[i].name) == 0)
        return (enum door)i;
    }
  }
  Rf_error("hypercast: unknown door");
}

/*
 * `format`: NULL or a 92-byte raw vector; `periods`: NULL or a 40-byte raw vector; `code`: a
 * single integer (0 where the door takes none). R built and checked all three.
 */
static void declared_arg(SEXP format, SEXP periods, SEXP code, struct declared *d) {
  memset(d, 0, sizeof *d);
  if (TYPEOF(format) == RAWSXP && XLENGTH(format) == HyperCastRawNumFormat_SIZE) {
    read_format(format, &d->format);
  } else if (format != R_NilValue) {
    Rf_error("hypercast: format must be a packed num_format");
  }
  if (TYPEOF(periods) == RAWSXP && XLENGTH(periods) == HyperCastRawDayPeriods_SIZE) {
    read_periods(periods, &d->periods);
    d->has_periods = 1;
  } else if (periods != R_NilValue) {
    Rf_error("hypercast: periods must be packed day periods");
  }
  if (TYPEOF(code) != INTSXP || XLENGTH(code) != 1 || INTEGER(code)[0] < 0)
    Rf_error("hypercast: code must be one non-negative integer");
  d->code = (uint32_t)INTEGER(code)[0];
}

/* ---- the columns ---- */

#define MAX_COLUMNS 8

struct columns {
  SEXP list;
  int count;
  int extras;
  enum kind kind;
  /* The value column and the extras, by kind. */
  int *lgl;
  int *integer;
  double *real;
  SEXP strings;
  double *extra_real[2];
  int *extra_int;
  int *ok;
  int *reason;
  int *offset;
  int *length;
};

static SEXP column(struct columns *c, const char *name, SEXPTYPE type, R_xlen_t n) {
  SEXP v = allocVector(type, n);
  SET_VECTOR_ELT(c->list, c->count, v);
  SET_STRING_ELT(getAttrib(c->list, R_NamesSymbol), c->count, mkChar(name));
  c->count++;
  return v;
}

/* Allocates every column; the result is PROTECTed once, as c->list. */
static void columns_alloc(struct columns *c, enum kind kind, R_xlen_t n) {
  memset(c, 0, sizeof *c);
  c->kind = kind;
  int total = 5;
  switch (kind) {
  case INSTANT_KIND:
  case CIVIL_KIND:
  case DURATION_KIND:
    total += 2;
    break;
  case TIME_KIND:
    total += 1;
    break;
  default:
    break;
  }
  c->list = PROTECT(allocVector(VECSXP, total));
  SEXP names = PROTECT(allocVector(STRSXP, total));
  setAttrib(c->list, R_NamesSymbol, names);
  UNPROTECT(1);
  switch (kind) {
  case LOGICAL_KIND:
    c->lgl = LOGICAL(column(c, "value", LGLSXP, n));
    break;
  case INTEGER_KIND:
    c->integer = INTEGER(column(c, "value", INTSXP, n));
    break;
  case STRING_KIND:
    c->strings = column(c, "value", STRSXP, n);
    break;
  case CIVIL_KIND:
    c->strings = column(c, "value", STRSXP, n);
    c->extra_real[0] = REAL(column(c, "date", REALSXP, n));
    c->extra_real[1] = REAL(column(c, "nanos_of_day", REALSXP, n));
    break;
  case INSTANT_KIND:
  case DURATION_KIND:
    c->real = REAL(column(c, "value", REALSXP, n));
    c->extra_real[0] = REAL(column(c, "seconds", REALSXP, n));
    c->extra_int = INTEGER(column(c, "nanos", INTSXP, n));
    break;
  case TIME_KIND:
    c->real = REAL(column(c, "value", REALSXP, n));
    c->extra_real[0] = REAL(column(c, "nanos", REALSXP, n));
    break;
  default: /* DOUBLE_KIND, DATE_KIND */
    c->real = REAL(column(c, "value", REALSXP, n));
    break;
  }
  c->ok = LOGICAL(column(c, "ok", LGLSXP, n));
  c->reason = INTEGER(column(c, "reason", INTSXP, n));
  c->offset = INTEGER(column(c, "offset", INTSXP, n));
  c->length = INTEGER(column(c, "length", INTSXP, n));
}

/* The value columns of row i as missing. */
static void value_na(struct columns *c, R_xlen_t i) {
  if (c->lgl)
    c->lgl[i] = NA_LOGICAL;
  if (c->integer)
    c->integer[i] = NA_INTEGER;
  if (c->real)
    c->real[i] = NA_REAL;
  if (c->strings)
    SET_STRING_ELT(c->strings, i, NA_STRING);
  for (int k = 0; k < 2; k++) {
    if (c->extra_real[k])
      c->extra_real[k][i] = NA_REAL;
  }
  if (c->extra_int)
    c->extra_int[i] = NA_INTEGER;
}

/* Row i is an NA element: nothing was cast, every column is NA. */
static void row_missing(struct columns *c, R_xlen_t i) {
  value_na(c, i);
  c->ok[i] = NA_LOGICAL;
  c->reason[i] = NA_INTEGER;
  c->offset[i] = NA_INTEGER;
  c->length[i] = NA_INTEGER;
}

static int span_int(uint32_t v) { return v > (uint32_t)INT_MAX ? INT_MAX : (int)v; }

/* Row i faulted with `reason` over offset/length, already in the caller's units. */
static void row_fault(struct columns *c, R_xlen_t i, int reason, uint32_t offset, uint32_t length) {
  value_na(c, i);
  c->ok[i] = FALSE;
  c->reason[i] = reason;
  c->offset[i] = span_int(offset);
  c->length[i] = span_int(length);
}

/* ---- rendering exact values as text ---- */

/* Writes v's decimal digits ending just before `end`; returns where they start. */
static char *u64_digits(uint64_t v, char *end) {
  do {
    *--end = (char)('0' + v % 10);
    v /= 10;
  } while (v);
  return end;
}

static SEXP i64_string(int64_t v) {
  char buf[24];
  char *end = buf + sizeof buf;
  uint64_t magnitude = v < 0 ? (uint64_t)0 - (uint64_t)v : (uint64_t)v;
  char *start = u64_digits(magnitude, end);
  if (v < 0)
    *--start = '-';
  return mkCharLen(start, (int)(end - start));
}

static SEXP u64_string(uint64_t v) {
  char buf[24];
  char *end = buf + sizeof buf;
  char *start = u64_digits(v, end);
  return mkCharLen(start, (int)(end - start));
}

/*
 * The exact decimal as the core's Display renders it: sign, the 96-bit magnitude's digits,
 * the point `scale` places from the right, "0." and zeros ahead when the scale reaches past
 * the digits. The corpus pins the text (decimal.json and typed.json, "value").
 */
static SEXP decimal_string(const HyperCastDecimal *d) {
  uint32_t limbs[3] = {d->hi, (uint32_t)(d->lo >> 32), (uint32_t)d->lo};
  char digits[32];
  int len = 0;
  do {
    uint64_t rem = 0;
    for (int k = 0; k < 3; k++) {
      uint64_t cur = rem << 32 | limbs[k];
      limbs[k] = (uint32_t)(cur / 10);
      rem = cur % 10;
    }
    digits[len++] = (char)('0' + rem);
  } while (limbs[0] | limbs[1] | limbs[2]);
  /* digits[] is least significant first. 255 places past 29 digits at most. */
  char buf[300];
  int o = 0;
  int scale = d->scale;
  if (d->negative)
    buf[o++] = '-';
  if (scale >= len) {
    buf[o++] = '0';
    buf[o++] = '.';
    for (int k = 0; k < scale - len; k++)
      buf[o++] = '0';
    for (int k = len - 1; k >= 0; k--)
      buf[o++] = digits[k];
  } else {
    for (int k = len - 1; k >= 0; k--) {
      buf[o++] = digits[k];
      if (k == scale && scale > 0)
        buf[o++] = '.';
    }
  }
  return mkCharLen(buf, o);
}

static SEXP uuid_string(const uint8_t b[16]) {
  static const char HEX[16] = "0123456789abcdef";
  char buf[36];
  int o = 0;
  for (int k = 0; k < 16; k++) {
    if (k == 4 || k == 6 || k == 8 || k == 10)
      buf[o++] = '-';
    buf[o++] = HEX[b[k] >> 4];
    buf[o++] = HEX[b[k] & 0x0f];
  }
  return mkCharLen(buf, 36);
}

/* A Unicode scalar as its UTF-8 bytes. The core never returns a surrogate or past U+10FFFF. */
static SEXP scalar_string(uint32_t c) {
  char buf[4];
  int n;
  if (c < 0x80) {
    buf[0] = (char)c;
    n = 1;
  } else if (c < 0x800) {
    buf[0] = (char)(0xC0 | c >> 6);
    buf[1] = (char)(0x80 | (c & 0x3F));
    n = 2;
  } else if (c < 0x10000) {
    buf[0] = (char)(0xE0 | c >> 12);
    buf[1] = (char)(0x80 | (c >> 6 & 0x3F));
    buf[2] = (char)(0x80 | (c & 0x3F));
    n = 3;
  } else {
    buf[0] = (char)(0xF0 | c >> 18);
    buf[1] = (char)(0x80 | (c >> 12 & 0x3F));
    buf[2] = (char)(0x80 | (c >> 6 & 0x3F));
    buf[3] = (char)(0x80 | (c & 0x3F));
    n = 4;
  }
  return mkCharLenCE(buf, n, CE_UTF8);
}

/* Days from 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's days_from_civil). */
static double days_from_civil(int y, int m, int d) {
  y -= m <= 2;
  const int era = (y >= 0 ? y : y - 399) / 400;
  const int yoe = y - era * 400;
  const int doy = (153 * (m + (m > 2 ? -3 : 9)) + 2) / 5 + d - 1;
  const int doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  return (double)era * 146097.0 + (double)doe - 719468.0;
}

static void two(char *p, unsigned v) {
  p[0] = (char)('0' + v / 10);
  p[1] = (char)('0' + v % 10);
}

/*
 * A zone-less wall clock as "YYYY-MM-DD HH:MM:SS" with the fraction's nine digits, trailing
 * zeros trimmed: the text as.POSIXct() reads once the caller names the zone.
 */
static SEXP civil_string(const HyperCastCivilDateTime *c) {
  char buf[32];
  unsigned y = c->date.year;
  buf[0] = (char)('0' + y / 1000 % 10);
  buf[1] = (char)('0' + y / 100 % 10);
  two(buf + 2, y % 100);
  buf[4] = '-';
  two(buf + 5, c->date.month);
  buf[7] = '-';
  two(buf + 8, c->date.day);
  buf[10] = ' ';
  uint64_t secs = c->nanos_of_day / 1000000000u;
  uint32_t frac = (uint32_t)(c->nanos_of_day % 1000000000u);
  two(buf + 11, (unsigned)(secs / 3600));
  buf[13] = ':';
  two(buf + 14, (unsigned)(secs / 60 % 60));
  buf[16] = ':';
  two(buf + 17, (unsigned)(secs % 60));
  int o = 19;
  if (frac) {
    buf[o++] = '.';
    for (uint32_t scale = 100000000u; scale && frac; scale /= 10) {
      buf[o++] = (char)('0' + frac / scale);
      frac %= scale;
    }
  }
  return mkCharLen(buf, o);
}

/* Row i succeeded with `v`. */
static void row_ok(struct columns *c, R_xlen_t i, enum door door, const union value *v) {
  c->ok[i] = TRUE;
  c->reason[i] = NA_INTEGER;
  c->offset[i] = NA_INTEGER;
  c->length[i] = NA_INTEGER;
  switch (door) {
  case BOOL_DOOR:
    c->lgl[i] = v->boolean != 0;
    break;
  case CHAR_DOOR:
    SET_STRING_ELT(c->strings, i, scalar_string(v->scalar));
    break;
  case I8_DOOR:
    c->integer[i] = v->i8;
    break;
  case I16_DOOR:
    c->integer[i] = v->i16;
    break;
  case U8_DOOR:
    c->integer[i] = v->u8;
    break;
  case U16_DOOR:
    c->integer[i] = v->u16;
    break;
  case I32_DOOR:
    c->real[i] = (double)v->i32;
    break;
  case U32_DOOR:
    c->real[i] = (double)v->u32;
    break;
  case F32_DOOR:
    c->real[i] = (double)v->f32;
    break;
  case F64_DOOR:
    c->real[i] = v->f64;
    break;
  case I64_DOOR:
    SET_STRING_ELT(c->strings, i, i64_string(v->i64));
    break;
  case U64_DOOR:
    SET_STRING_ELT(c->strings, i, u64_string(v->u64));
    break;
  case DECIMAL_DOOR:
  case DECIMAL_FROM_DOUBLE_DOOR:
    SET_STRING_ELT(c->strings, i, decimal_string(&v->decimal));
    break;
  case UUID_DOOR:
    SET_STRING_ELT(c->strings, i, uuid_string(v->uuid));
    break;
  case TIMESTAMP_DOOR:
  case UNIX_DOOR:
  case EXCEL_SERIAL_DOOR:
    c->real[i] = (double)v->timestamp.seconds + (double)v->timestamp.nanos / 1e9;
    c->extra_real[0][i] = (double)v->timestamp.seconds;
    c->extra_int[i] = v->timestamp.nanos;
    break;
  case DATE_DOOR:
  case DATE_ORDERED_DOOR:
    c->real[i] = days_from_civil(v->date.year, v->date.month, v->date.day);
    break;
  case DATETIME_DOOR:
  case EXCEL_SERIAL_FROM_DOUBLE_DOOR:
    SET_STRING_ELT(c->strings, i, civil_string(&v->civil));
    c->extra_real[0][i] =
        days_from_civil(v->civil.date.year, v->civil.date.month, v->civil.date.day);
    c->extra_real[1][i] = (double)v->civil.nanos_of_day;
    break;
  case TIME_DOOR:
  case EXCEL_TIME_DOOR:
    c->real[i] = (double)v->nanos / 1e9;
    c->extra_real[0][i] = (double)v->nanos;
    break;
  case DURATION_DOOR:
  case EXCEL_DURATION_DOOR:
    c->real[i] = (double)v->duration.seconds + (double)v->duration.nanos / 1e9;
    c->extra_real[0][i] = (double)v->duration.seconds;
    c->extra_int[i] = v->duration.nanos;
    break;
  default:
    break;
  }
}

/* ---- spans in the caller's units ---- */

/* Code points in b[0..n): every one starts with exactly one non-continuation byte. */
static uint32_t code_points(const uint8_t *b, uint32_t n) {
  uint32_t count = 0;
  for (uint32_t k = 0; k < n; k++)
    count += (b[k] & 0xC0) != 0x80;
  return count;
}

static int ascii_space(uint8_t b) {
  return b == ' ' || b == '\t' || b == '\n' || b == '\f' || b == '\r';
}

/*
 * The core's byte span over `p`, re-expressed in the units the caller's input slices by:
 * bytes for a raw vector or a "bytes"-encoded string, characters (code points of the UTF-8
 * the core read) for any other string. offset + length never exceeds the input.
 */
static void present(const uint8_t *p, uintptr_t n, int chars, HyperCastRawFault *f) {
  uint64_t offset = f->offset < n ? f->offset : n;
  uint64_t end = offset + f->len < n ? offset + f->len : n;
  if (!chars)
    return;
  uint32_t start = code_points(p, (uint32_t)offset);
  f->len = code_points(p + offset, (uint32_t)(end - offset));
  f->offset = start;
}

/* ---- the input ---- */

/*
 * The bytes the core reads for string element s, and whether the span counts characters. A
 * string marked UTF-8 or ASCII, or native in a UTF-8 session, is read as it is; one marked
 * latin1, or native in any other session, is translated to UTF-8 (each latin1 byte is one
 * character either way); one marked "bytes" is read as it is, and counted in bytes, as substr()
 * counts it.
 */
/*
 * native: how this session holds an unmarked string, from session_native() in R. 0, UTF-8: its
 * bytes are UTF-8, counted in characters. 1, an ASCII session (the C locale): R has no
 * encoding to translate its non-ASCII bytes from (it would write each as a "<xx>" escape) and
 * counts them as bytes, so they are read as they are, in bytes. 2, any other single- or
 * multi-byte locale: translated to UTF-8, counted in characters.
 */
static const uint8_t *string_bytes(SEXP s, int native, uintptr_t *n, int *chars) {
  const char *c = CHAR(s);
  cetype_t ce = getCharCE(s);
  *chars = ce != CE_BYTES && !(ce == CE_NATIVE && native == 1);
  if (ce == CE_LATIN1 || (ce == CE_NATIVE && native == 2)) {
    /* An ASCII string is the same bytes in every encoding R knows: no translation. */
    const char *k = c;
    while (*k && (unsigned char)*k < 0x80)
      k++;
    if (*k)
      c = translateCharUTF8(s);
  }
  *n = (uintptr_t)strlen(c);
  return (const uint8_t *)c;
}

/*
 * The char door's value U+0000 is a scalar the core accepts and an R string cannot hold (R's
 * strings are NUL-terminated), so it is reported the way C# and Java report a scalar their
 * UTF-16 char cannot hold: out of range, over the input less its ASCII whitespace edges.
 */
static void nul_scalar(struct columns *c, R_xlen_t i, const uint8_t *p, uintptr_t n, int chars) {
  uintptr_t start = 0, end = n;
  while (start < end && ascii_space(p[start]))
    start++;
  while (end > start && ascii_space(p[end - 1]))
    end--;
  HyperCastRawFault f = {(uint32_t)start, (uint32_t)(end - start)};
  present(p, n, chars, &f);
  row_fault(c, i, 3, f.offset, f.len);
}

static void contract_violation(void) {
  Rf_error("hypercast: the core refused a declared format, order, precision, epoch or day "
           "period (a contract violation this package checks for first; please report it)");
}

/*
 * x: a character vector, or a list whose elements are raw vectors or NULL (NULL is
 * missing). native: string_bytes()'s session code.
 */
SEXP C_cast_text(SEXP x, SEXP door_name, SEXP format, SEXP periods, SEXP code, SEXP native_mode) {
  enum door door = door_arg(door_name);
  if (door >= DECIMAL_FROM_DOUBLE_DOOR)
    Rf_error("hypercast: %s takes numbers", DOORS[door].name);
  struct declared d;
  declared_arg(format, periods, code, &d);
  int is_list = TYPEOF(x) == VECSXP;
  if (!is_list && TYPEOF(x) != STRSXP)
    Rf_error("hypercast: x must be a character vector or a list of raw vectors");
  int native = asInteger(native_mode);
  R_xlen_t n = XLENGTH(x);
  struct columns c;
  columns_alloc(&c, DOORS[door].kind, n);
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point(i);
    const void *vmax = vmaxget();
    const uint8_t *p;
    uintptr_t len;
    int chars;
    if (is_list) {
      SEXP el = VECTOR_ELT(x, i);
      if (el == R_NilValue) {
        row_missing(&c, i);
        continue;
      }
      if (TYPEOF(el) != RAWSXP)
        Rf_error("hypercast: x must be a list of raw vectors");
      p = RAW(el);
      len = (uintptr_t)XLENGTH(el);
      chars = 0;
    } else {
      SEXP s = STRING_ELT(x, i);
      if (s == NA_STRING) {
        row_missing(&c, i);
        continue;
      }
      p = string_bytes(s, native, &len, &chars);
    }
    union value v;
    HyperCastRawFault f = {0, 0};
    int32_t rc = text_door(door, p, len, &d, &v, &f);
    if (rc == 0 && door == CHAR_DOOR && v.scalar == 0) {
      nul_scalar(&c, i, p, len, chars);
    } else if (rc == 0) {
      row_ok(&c, i, door, &v);
    } else if (rc >= 1 && rc <= 3) {
      present(p, len, chars, &f);
      row_fault(&c, i, rc, f.offset, f.len);
    } else {
      contract_violation();
    }
    vmaxset(vmax);
  }
  UNPROTECT(1);
  return c.list;
}

/* x: a double vector; NA_real_ is missing, while NaN is a number the core reads. */
SEXP C_cast_double(SEXP x, SEXP door_name, SEXP code) {
  enum door door = door_arg(door_name);
  if (door < DECIMAL_FROM_DOUBLE_DOOR)
    Rf_error("hypercast: %s takes text", DOORS[door].name);
  struct declared d;
  declared_arg(R_NilValue, R_NilValue, code, &d);
  if (TYPEOF(x) != REALSXP)
    Rf_error("hypercast: x must be a double vector");
  R_xlen_t n = XLENGTH(x);
  struct columns c;
  columns_alloc(&c, DOORS[door].kind, n);
  const double *xs = REAL(x);
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point(i);
    if (R_IsNA(xs[i])) {
      row_missing(&c, i);
      continue;
    }
    union value v;
    HyperCastRawFault f = {0, 0};
    int32_t rc = double_door(door, xs[i], &d, &v, &f);
    if (rc == 0) {
      row_ok(&c, i, door, &v);
    } else if (rc >= 1 && rc <= 3) {
      row_fault(&c, i, rc, f.offset, f.len);
    } else {
      contract_violation();
    }
  }
  UNPROTECT(1);
  return c.list;
}

/*
 * The one symbol the core's archive needs and does not define. compiler_builtins ships
 * precompiled with unwind tables, and the members R's linker pulls from the archive for the
 * core's 128-bit arithmetic name `rust_eh_personality` (the core's rust/src/lib.rs has the
 * same gap for its shared library, and the same answer). The archive is built with
 * `panic = "abort"`, so nothing ever unwinds and nothing ever calls this: it only has to
 * exist. Hidden, so the package's shared object neither exports nor shares it.
 */
#if defined(__GNUC__) && !defined(_WIN32)
__attribute__((visibility("hidden")))
#endif
void rust_eh_personality(void);
void rust_eh_personality(void) {}

/* The packed core version (major << 16 | minor << 8 | patch); R unpacks it. */
SEXP C_native_version(void) { return ScalarInteger((int)hypercast_version()); }
