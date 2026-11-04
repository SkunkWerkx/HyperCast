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
 * The doors, vectorised. Every element of `x` is one call into the core's detailed exports
 * (cast_detailed, cast_detailed_from_f64 and the checked pair), and its verdict becomes one
 * row of the columns this file returns: the value (NA where the element faulted), ok, the
 * reason code (1 empty, 2 malformed, 3 out of range), the fault's offset and length in the
 * caller's units, and its detail code (the core's, 1 or more; R names it). An NA element is
 * no call at all: every column is NA in its row. Nothing here raises an R error for any element of
 * `x`; the only errors are caller bugs R validated before the call (and the core's
 * contract-violation code, which that validation makes unreachable), and an interrupt.
 *
 * The checked doors (cast_checked_detailed, cast_checked_detailed_from_f64, and cast_text with
 * constraints) add
 * three columns, adjusted, enum_index and keyword, and one more outcome: a violation (code 4),
 * a value that parsed and broke a declared rule, whose keyword and span fill their columns
 * while reason stays NA.
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
  TEXT_DOOR,
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
    [TEXT_DOOR] = {"text", STRING_KIND},
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
  HyperCastRawSpan span;
};

/* The declarations a door reads beside its input, decoded once per call. */
struct declared {
  HyperCastRawNumFormat format;
  HyperCastRawDayPeriods periods;
  HyperCastRawDateOptions options;
  const uint8_t *vocab;
  uintptr_t vocab_len;
  int has_format;
  int has_periods;
  int has_options;
  uint32_t code; /* the precision, epoch or date order */
};

/* The core's door codes (rust/src/door.rs, `Door::code`) for every door this file reads. */
static uint32_t abi_door(enum door door) {
  switch (door) {
  case BOOL_DOOR:
    return 1;
  case I8_DOOR:
    return 2;
  case I16_DOOR:
    return 3;
  case I32_DOOR:
    return 4;
  case I64_DOOR:
    return 5;
  case U8_DOOR:
    return 6;
  case U16_DOOR:
    return 7;
  case U32_DOOR:
    return 8;
  case U64_DOOR:
    return 9;
  case F32_DOOR:
    return 10;
  case F64_DOOR:
    return 11;
  case UUID_DOOR:
    return 12;
  case TIMESTAMP_DOOR:
    return 13;
  case UNIX_DOOR:
    return 14;
  case DATE_DOOR:
    return 15;
  case TIME_DOOR:
  case EXCEL_TIME_DOOR:
    return 16;
  case DURATION_DOOR:
  case EXCEL_DURATION_DOOR:
    return 17;
  case TEXT_DOOR:
    return 18;
  case DECIMAL_DOOR:
  case DECIMAL_FROM_DOUBLE_DOOR:
    return 19;
  case DATE_ORDERED_DOOR:
    return 20;
  case DATETIME_DOOR:
    return 21;
  case EXCEL_SERIAL_DOOR:
  case EXCEL_SERIAL_FROM_DOUBLE_DOOR:
    return 22;
  case CHAR_DOOR:
    return 23;
  default:
    return 0;
  }
}

/* The parameter a door declares (the Unix precision, a date order, an Excel epoch), else 0. */
static uint32_t abi_param(enum door door, const struct declared *d) {
  switch (door) {
  case UNIX_DOOR:
  case EXCEL_SERIAL_DOOR:
  case EXCEL_SERIAL_FROM_DOUBLE_DOOR:
  case DATE_ORDERED_DOOR:
  case DATETIME_DOOR:
    return d->code;
  default:
    return 0;
  }
}

/* A detailed export's fault, as the caller sets it up: the size word, then nothing. */
static HyperCastRawFaultDetail fault_slot(void) {
  HyperCastRawFaultDetail f = {HyperCastRawFaultDetail_SIZE, 0, 0, 0};
  return f;
}

static int32_t text_door(enum door door, const uint8_t *p, uintptr_t n, const struct declared *d,
                         union value *out, HyperCastRawFaultDetail *fault) {
  return cast_detailed(abi_door(door), abi_param(door, d), d->has_format ? &d->format : NULL,
                       d->has_periods ? &d->periods : NULL, d->has_options ? &d->options : NULL,
                       d->vocab, d->vocab_len, p, n, out, fault);
}

static int32_t double_door(enum door door, double v, const struct declared *d, union value *out,
                           HyperCastRawFaultDetail *fault) {
  return cast_detailed_from_f64(abi_door(door), abi_param(door, d), v, out, fault);
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
 * The 12-byte RawDateOptions R packs (date_options()'s `packed`): the size, the option bits and
 * the pivot as little-endian u32s.
 */
_Static_assert(sizeof(HyperCastRawDateOptions) == HyperCastRawDateOptions_SIZE, "RawDateOptions");
static void read_options(SEXP packed, HyperCastRawDateOptions *out) {
  const uint8_t *b = RAW(packed);
  out->size = le32(b);
  out->flags = le32(b + 4);
  out->pivot = le32(b + 8);
}

/*
 * `format`: NULL or a 92-byte raw vector; `periods`: NULL or a 40-byte raw vector; `code`: a
 * single integer (0 where the door takes none); `options`: NULL or a 12-byte raw vector;
 * `vocab`: NULL or the month names' packed raw vector. R built and checked all of them.
 */
static void declared_arg(SEXP format, SEXP periods, SEXP code, SEXP options, SEXP vocab,
                         struct declared *d) {
  memset(d, 0, sizeof *d);
  if (TYPEOF(format) == RAWSXP && XLENGTH(format) == HyperCastRawNumFormat_SIZE) {
    read_format(format, &d->format);
    d->has_format = 1;
  } else if (format != R_NilValue) {
    Rf_error("hypercast: format must be a packed num_format");
  }
  if (TYPEOF(periods) == RAWSXP && XLENGTH(periods) == HyperCastRawDayPeriods_SIZE) {
    read_periods(periods, &d->periods);
    d->has_periods = 1;
  } else if (periods != R_NilValue) {
    Rf_error("hypercast: periods must be packed day periods");
  }
  if (TYPEOF(options) == RAWSXP && XLENGTH(options) == HyperCastRawDateOptions_SIZE) {
    read_options(options, &d->options);
    d->has_options = 1;
  } else if (options != R_NilValue) {
    Rf_error("hypercast: options must be packed date options");
  }
  if (TYPEOF(vocab) == RAWSXP) {
    d->vocab_len = (uintptr_t)XLENGTH(vocab);
    d->vocab = d->vocab_len ? RAW(vocab) : NULL;
  } else if (vocab != R_NilValue) {
    Rf_error("hypercast: months must be packed month names");
  }
  if (TYPEOF(code) != INTSXP || XLENGTH(code) != 1 || INTEGER(code)[0] < 0)
    Rf_error("hypercast: code must be one non-negative integer");
  d->code = (uint32_t)INTEGER(code)[0];
}

/* ---- the columns ---- */

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
  int *detail;
  /* The checked doors' columns; NULL in a plain verdict. */
  int *adjusted;
  int *enum_index;
  int *keyword;
};

static SEXP column(struct columns *c, const char *name, SEXPTYPE type, R_xlen_t n) {
  SEXP v = allocVector(type, n);
  SET_VECTOR_ELT(c->list, c->count, v);
  SET_STRING_ELT(getAttrib(c->list, R_NamesSymbol), c->count, mkChar(name));
  c->count++;
  return v;
}

/*
 * Allocates every column; the result is PROTECTed once, as c->list. A checked verdict adds
 * adjusted and enum_index after the value's columns and keyword after reason. The detail
 * column ends every verdict: after the 1.0 columns, so none of them moves.
 */
static void columns_alloc(struct columns *c, enum kind kind, R_xlen_t n, int checked) {
  memset(c, 0, sizeof *c);
  c->kind = kind;
  int total = checked ? 9 : 6;
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
  if (checked) {
    c->adjusted = LOGICAL(column(c, "adjusted", LGLSXP, n));
    c->enum_index = INTEGER(column(c, "enum_index", INTSXP, n));
  }
  c->ok = LOGICAL(column(c, "ok", LGLSXP, n));
  c->reason = INTEGER(column(c, "reason", INTSXP, n));
  if (checked)
    c->keyword = INTEGER(column(c, "keyword", INTSXP, n));
  c->offset = INTEGER(column(c, "offset", INTSXP, n));
  c->length = INTEGER(column(c, "length", INTSXP, n));
  c->detail = INTEGER(column(c, "detail", INTSXP, n));
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
  if (c->adjusted) {
    c->adjusted[i] = NA_LOGICAL;
    c->enum_index[i] = NA_INTEGER;
  }
}

/* Row i is an NA element: nothing was cast, every column is NA. */
static void row_missing(struct columns *c, R_xlen_t i) {
  value_na(c, i);
  c->ok[i] = NA_LOGICAL;
  c->reason[i] = NA_INTEGER;
  if (c->keyword)
    c->keyword[i] = NA_INTEGER;
  c->offset[i] = NA_INTEGER;
  c->length[i] = NA_INTEGER;
  c->detail[i] = NA_INTEGER;
}

static int span_int(uint32_t v) { return v > (uint32_t)INT_MAX ? INT_MAX : (int)v; }

/*
 * Row i faulted with `reason` over offset/length, already in the caller's units, for the
 * core's `detail` code. A code this package does not name stays a code: R reads it as
 * "unknown".
 */
static void row_fault(struct columns *c, R_xlen_t i, int reason, uint32_t offset, uint32_t length,
                      uint32_t detail) {
  value_na(c, i);
  c->ok[i] = FALSE;
  c->reason[i] = reason;
  if (c->keyword)
    c->keyword[i] = NA_INTEGER;
  c->offset[i] = span_int(offset);
  c->length[i] = span_int(length);
  c->detail[i] = span_int(detail);
}

/* Row i parsed and broke the declared rule `keyword` over offset/length, in caller units. */
static void row_violation(struct columns *c, R_xlen_t i, uint32_t keyword, uint32_t offset,
                          uint32_t length) {
  value_na(c, i);
  c->ok[i] = FALSE;
  c->reason[i] = NA_INTEGER;
  c->keyword[i] = span_int(keyword);
  c->offset[i] = span_int(offset);
  c->length[i] = span_int(length);
  c->detail[i] = NA_INTEGER; /* a violation carries no detail (the core writes 0) */
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
  if (c->keyword)
    c->keyword[i] = NA_INTEGER;
  c->offset[i] = NA_INTEGER;
  c->length[i] = NA_INTEGER;
  c->detail[i] = NA_INTEGER;
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

/*
 * The bytes of the white-space code point (Unicode White_Space, the core's is_white_space)
 * that b[0..n) starts with, or 0: the 25 code points, as UTF-8. R's num_format.R and the
 * corpus's formats.json hold the same list.
 */
static uint32_t white_space_len(const uint8_t *b, uint32_t n) {
  if (n >= 1 && ((b[0] >= 0x09 && b[0] <= 0x0D) || b[0] == 0x20))
    return 1;
  if (n >= 2 && b[0] == 0xC2 && (b[1] == 0x85 || b[1] == 0xA0)) /* U+0085, U+00A0 */
    return 2;
  if (n < 3)
    return 0;
  if (b[0] == 0xE1 && b[1] == 0x9A && b[2] == 0x80) /* U+1680 */
    return 3;
  /* U+2000..U+200A, U+2028, U+2029, U+202F */
  if (b[0] == 0xE2 && b[1] == 0x80 &&
      ((b[2] >= 0x80 && b[2] <= 0x8A) || b[2] == 0xA8 || b[2] == 0xA9 || b[2] == 0xAF))
    return 3;
  if (b[0] == 0xE2 && b[1] == 0x81 && b[2] == 0x9F) /* U+205F */
    return 3;
  if (b[0] == 0xE3 && b[1] == 0x80 && b[2] == 0x80) /* U+3000 */
    return 3;
  return 0;
}

/* The bytes of the white-space code point that ends b[0..n), or 0. */
static uint32_t white_space_len_back(const uint8_t *b, uint32_t n) {
  for (uint32_t k = 1; k <= 3 && k <= n; k++)
    if (white_space_len(b + n - k, k) == k)
      return k;
  return 0;
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

/* The same, for a detailed fault's span: its detail is the core's and stays as it is. */
static HyperCastRawFault span_of(const HyperCastRawFaultDetail *f) {
  HyperCastRawFault span = {f->offset, f->len};
  return span;
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

/* The core's detail codes this file assigns itself (rust/src/detail.rs). */
#define DETAIL_UNEXPECTED_CHARACTER 2
#define DETAIL_TOO_LONG 21

/*
 * The char door's value U+0000 is a scalar the core accepts and an R string cannot hold (R's
 * strings are NUL-terminated), so it is reported the way C# and Java report a scalar their
 * UTF-16 char cannot hold: out of range, over the input less its white space edges. The core
 * read no fault, so the detail is this binding's: NUL is a character the position (an R
 * string) does not allow.
 */
static void nul_scalar(struct columns *c, R_xlen_t i, const uint8_t *p, uintptr_t n, int chars) {
  uintptr_t start = 0, end = n;
  uint32_t w;
  while (start < end && (w = white_space_len(p + start, (uint32_t)(end - start))) != 0)
    start += w;
  while (end > start && (w = white_space_len_back(p + start, (uint32_t)(end - start))) != 0)
    end -= w;
  HyperCastRawFault f = {(uint32_t)start, (uint32_t)(end - start)};
  present(p, n, chars, &f);
  row_fault(c, i, 3, f.offset, f.len, DETAIL_UNEXPECTED_CHARACTER);
}

/*
 * Element i of x, a character vector or (is_list) a list of raw vectors: the bytes the core
 * reads, whether its spans count characters, and the string itself (R_NilValue for a raw
 * vector). 0 for a missing element, an NA string or a NULL.
 */
static int element(SEXP x, int is_list, R_xlen_t i, int native, const uint8_t **p, uintptr_t *len,
                   int *chars, SEXP *s) {
  if (is_list) {
    SEXP el = VECTOR_ELT(x, i);
    *s = R_NilValue;
    if (el == R_NilValue)
      return 0;
    if (TYPEOF(el) != RAWSXP)
      Rf_error("hypercast: x must be a list of raw vectors");
    *p = RAW(el);
    *len = (uintptr_t)XLENGTH(el);
    *chars = 0;
    return 1;
  }
  *s = STRING_ELT(x, i);
  if (*s == NA_STRING)
    return 0;
  *p = string_bytes(*s, native, len, chars);
  return 1;
}

/*
 * The text door's value, the span `span` of the input: the caller's own string when the span
 * is all of it, otherwise the span's bytes in the encoding they were read in ("bytes" for a
 * string marked so, native for an unmarked one in an ASCII session, UTF-8 otherwise and for a
 * raw vector). U+0000 is valid UTF-8 a raw vector can carry and an R string cannot hold, so a
 * value holding it is out of range over the value, as the char door's U+0000 is.
 */
static void text_row(struct columns *c, R_xlen_t i, SEXP s, const uint8_t *p, uintptr_t n,
                     int chars, const HyperCastRawSpan *span) {
  if (span->offset > n || span->len > n - span->offset)
    Rf_error("hypercast: the core wrote a span outside the input (please report it)");
  const uint8_t *start = p + span->offset;
  int too_long = span->len > (uint32_t)INT_MAX;
  if (too_long || memchr(start, 0, span->len)) {
    HyperCastRawFault f = {span->offset, span->len};
    present(p, n, chars, &f);
    row_fault(c, i, 3, f.offset, f.len, too_long ? DETAIL_TOO_LONG : DETAIL_UNEXPECTED_CHARACTER);
    return;
  }
  row_ok(c, i, TEXT_DOOR, NULL);
  if (s != R_NilValue && span->offset == 0 && span->len == n) {
    SET_STRING_ELT(c->strings, i, s);
    return;
  }
  cetype_t ce = CE_UTF8;
  if (s != R_NilValue && getCharCE(s) == CE_BYTES)
    ce = CE_BYTES;
  else if (s != R_NilValue && getCharCE(s) == CE_NATIVE && !chars)
    ce = CE_NATIVE;
  SET_STRING_ELT(c->strings, i, mkCharLenCE((const char *)start, (int)span->len, ce));
}

static void contract_violation(void) {
  Rf_error("hypercast: the core refused a declared format, order, precision, epoch or day "
           "period (a contract violation this package checks for first; please report it)");
}

/*
 * x: a character vector, or a list whose elements are raw vectors or NULL (NULL is
 * missing). native: string_bytes()'s session code.
 */
SEXP C_cast_text(SEXP x, SEXP door_name, SEXP format, SEXP periods, SEXP code, SEXP native_mode,
                 SEXP options, SEXP vocab) {
  enum door door = door_arg(door_name);
  if (door >= DECIMAL_FROM_DOUBLE_DOOR)
    Rf_error("hypercast: %s takes numbers", DOORS[door].name);
  struct declared d;
  declared_arg(format, periods, code, options, vocab, &d);
  int is_list = TYPEOF(x) == VECSXP;
  if (!is_list && TYPEOF(x) != STRSXP)
    Rf_error("hypercast: x must be a character vector or a list of raw vectors");
  int native = asInteger(native_mode);
  R_xlen_t n = XLENGTH(x);
  struct columns c;
  columns_alloc(&c, DOORS[door].kind, n, 0);
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point(i);
    const void *vmax = vmaxget();
    const uint8_t *p;
    uintptr_t len;
    int chars;
    SEXP s;
    if (!element(x, is_list, i, native, &p, &len, &chars, &s)) {
      row_missing(&c, i);
      continue;
    }
    union value v;
    HyperCastRawFaultDetail f = fault_slot();
    int32_t rc = text_door(door, p, len, &d, &v, &f);
    if (rc == 0 && door == CHAR_DOOR && v.scalar == 0) {
      nul_scalar(&c, i, p, len, chars);
    } else if (rc == 0 && door == TEXT_DOOR) {
      text_row(&c, i, s, p, len, chars, &v.span);
    } else if (rc == 0) {
      row_ok(&c, i, door, &v);
    } else if (rc >= 1 && rc <= 3) {
      HyperCastRawFault span = span_of(&f);
      present(p, len, chars, &span);
      row_fault(&c, i, rc, span.offset, span.len, f.detail);
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
  declared_arg(R_NilValue, R_NilValue, code, R_NilValue, R_NilValue, &d);
  if (TYPEOF(x) != REALSXP)
    Rf_error("hypercast: x must be a double vector");
  R_xlen_t n = XLENGTH(x);
  struct columns c;
  columns_alloc(&c, DOORS[door].kind, n, 0);
  const double *xs = REAL(x);
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point(i);
    if (R_IsNA(xs[i])) {
      row_missing(&c, i);
      continue;
    }
    union value v;
    HyperCastRawFaultDetail f = fault_slot();
    int32_t rc = double_door(door, xs[i], &d, &v, &f);
    if (rc == 0) {
      row_ok(&c, i, door, &v);
    } else if (rc >= 1 && rc <= 3) {
      row_fault(&c, i, rc, f.offset, f.len, f.detail);
    } else {
      contract_violation();
    }
  }
  UNPROTECT(1);
  return c.list;
}

/* ---- validation constraints ---- */

/*
 * The declaration a hypercast_constraints() value lays out as, built here from the fields R
 * checked: the bounds, step and integer enum entries through cast_declared_decimal (the one
 * conversion every binding calls), the rest copied into the core's own struct. It is built
 * again for every call, which costs a few conversions per vector, never per element, and
 * judged by cast_checked's own layout checks each time; cast_constraints_check judged it,
 * duplicates included, when it was declared.
 */
_Static_assert(sizeof(HyperCastRawConstraints) == HyperCastRawConstraints_SIZE, "RawConstraints");
_Static_assert(offsetof(HyperCastRawConstraints, rounding) == 8, "rounding");
_Static_assert(offsetof(HyperCastRawConstraints, lower) == 24, "lower");
_Static_assert(offsetof(HyperCastRawConstraints, upper) == 40, "upper");
_Static_assert(offsetof(HyperCastRawConstraints, multiple_of) == 56, "multiple_of");
_Static_assert(sizeof(HyperCastRawCheck) == HyperCastRawCheck_SIZE, "RawCheck");
_Static_assert(sizeof(HyperCastRawSpan) == HyperCastRawSpan_SIZE, "RawSpan");
_Static_assert(sizeof(HyperCastDecimal) == 16, "Decimal");

/* The core's door codes (rust/src/door.rs) for the doors a declaration binds to; 0 otherwise. */
static uint32_t core_door(enum door door) {
  switch (door) {
  case I8_DOOR:
    return 2;
  case I16_DOOR:
    return 3;
  case I32_DOOR:
    return 4;
  case I64_DOOR:
    return 5;
  case U8_DOOR:
    return 6;
  case U16_DOOR:
    return 7;
  case U32_DOOR:
    return 8;
  case U64_DOOR:
    return 9;
  case F32_DOOR:
    return 10;
  case F64_DOOR:
    return 11;
  case TEXT_DOOR:
    return 18;
  case DECIMAL_DOOR:
    return 19;
  default:
    return 0;
  }
}

/* constraints_layout()'s list, by position. */
enum layout {
  L_DOOR,
  L_MINIMUM,
  L_EXCLUSIVE_MINIMUM,
  L_MAXIMUM,
  L_EXCLUSIVE_MAXIMUM,
  L_MULTIPLE_OF,
  L_ROUNDING,
  L_MIN_LENGTH,
  L_MAX_LENGTH,
  L_TRIM,
  L_ENUM,
  L_COUNT
};

struct declaration {
  enum door door;
  HyperCastRawConstraints raw;
  const uint8_t *enum_ptr;
  uintptr_t enum_len;
  SEXP entries; /* the text door's entries as the caller declared them, or R_NilValue */
};

/*
 * Element k of a bound, step or integer enum list through cast_declared_decimal: a string's
 * bytes as the doors read a string, or an R integer as its decimal digits. Returns its code,
 * 0 or 8.
 */
static int32_t declared_value(SEXP v, R_xlen_t k, int native, HyperCastDecimal *out) {
  if (TYPEOF(v) == INTSXP && INTEGER(v)[k] != NA_INTEGER) {
    int x = INTEGER(v)[k];
    char buf[16];
    char *end = buf + sizeof buf;
    uint64_t magnitude = x < 0 ? (uint64_t)0 - (uint64_t)(int64_t)x : (uint64_t)x;
    char *start = u64_digits(magnitude, end);
    if (x < 0)
      *--start = '-';
    return cast_declared_decimal((const uint8_t *)start, (uintptr_t)(end - start), out);
  }
  if (TYPEOF(v) == STRSXP && STRING_ELT(v, k) != NA_STRING) {
    uintptr_t n;
    int chars;
    const uint8_t *p = string_bytes(STRING_ELT(v, k), native, &n, &chars);
    return cast_declared_decimal(p, n, out);
  }
  Rf_error("hypercast: a bound, step or enum entry must be a string or an integer");
}

static uint32_t length_value(SEXP v) {
  double x = TYPEOF(v) == INTSXP || TYPEOF(v) == REALSXP ? asReal(v) : -1;
  if (!(x >= 0 && x <= 4294967295.0) || (double)(uint32_t)x != x)
    Rf_error("hypercast: a length must be one whole number from 0 to 4294967295");
  return (uint32_t)x;
}

/* The text door's enum: each entry a little-endian u32 byte length, then its bytes. */
static void pack_text_enum(SEXP e, int native, struct declaration *d) {
  if (TYPEOF(e) != STRSXP)
    Rf_error("hypercast: the text door's enum must be a character vector");
  R_xlen_t n = XLENGTH(e);
  const uint8_t **ptrs = (const uint8_t **)R_alloc((size_t)n, sizeof *ptrs);
  uintptr_t *lens = (uintptr_t *)R_alloc((size_t)n, sizeof *lens);
  uintptr_t total = 0;
  for (R_xlen_t k = 0; k < n; k++) {
    SEXP s = STRING_ELT(e, k);
    int chars;
    if (s == NA_STRING)
      Rf_error("hypercast: an enum entry must not be NA");
    ptrs[k] = string_bytes(s, native, &lens[k], &chars);
    if (total > UINTPTR_MAX - 4 - lens[k])
      Rf_error("hypercast: the enum is too large to lay out");
    total += 4 + lens[k];
  }
  uint8_t *buf = (uint8_t *)R_alloc(total, 1);
  uint8_t *o = buf;
  for (R_xlen_t k = 0; k < n; k++) {
    uint32_t len = (uint32_t)lens[k];
    o[0] = (uint8_t)len;
    o[1] = (uint8_t)(len >> 8);
    o[2] = (uint8_t)(len >> 16);
    o[3] = (uint8_t)(len >> 24);
    memcpy(o + 4, ptrs[k], lens[k]);
    o += 4 + lens[k];
  }
  d->enum_ptr = buf;
  d->enum_len = total;
  d->entries = e;
}

/*
 * Lays out the declaration R checked into d. Returns 0, or 8 when cast_declared_decimal
 * refuses a bound, step or entry (cast_constraints_check's code for the same refusal, which
 * comes first, as in every binding).
 */
static int32_t declaration_arg(SEXP layout, int native, struct declaration *d) {
  if (TYPEOF(layout) != VECSXP || XLENGTH(layout) != L_COUNT)
    Rf_error("hypercast: constraints must be a hypercast_constraints");
  memset(d, 0, sizeof *d);
  d->entries = R_NilValue;
  d->door = door_arg(VECTOR_ELT(layout, L_DOOR));
  if (!core_door(d->door))
    Rf_error("hypercast: the %s door takes no constraints", DOORS[d->door].name);
  HyperCastRawConstraints *r = &d->raw;
  r->size = HyperCastRawConstraints_SIZE;
  static const struct {
    int at;
    uint32_t flag;
    size_t field;
  } VALUES[] = {
      {L_MINIMUM, HyperCastRawConstraints_HAS_MINIMUM, offsetof(HyperCastRawConstraints, lower)},
      {L_EXCLUSIVE_MINIMUM, HyperCastRawConstraints_HAS_EXCLUSIVE_MINIMUM,
       offsetof(HyperCastRawConstraints, lower)},
      {L_MAXIMUM, HyperCastRawConstraints_HAS_MAXIMUM, offsetof(HyperCastRawConstraints, upper)},
      {L_EXCLUSIVE_MAXIMUM, HyperCastRawConstraints_HAS_EXCLUSIVE_MAXIMUM,
       offsetof(HyperCastRawConstraints, upper)},
      {L_MULTIPLE_OF, HyperCastRawConstraints_HAS_MULTIPLE_OF,
       offsetof(HyperCastRawConstraints, multiple_of)},
  };
  for (size_t k = 0; k < sizeof VALUES / sizeof VALUES[0]; k++) {
    SEXP v = VECTOR_ELT(layout, VALUES[k].at);
    if (v == R_NilValue)
      continue;
    if (XLENGTH(v) != 1)
      Rf_error("hypercast: a bound or step must be one value");
    HyperCastDecimal value;
    int32_t rc = declared_value(v, 0, native, &value);
    if (rc != 0)
      return rc;
    memcpy((uint8_t *)r + VALUES[k].field, &value, sizeof value);
    r->flags |= VALUES[k].flag;
  }
  SEXP rounding = VECTOR_ELT(layout, L_ROUNDING);
  if (TYPEOF(rounding) != INTSXP || XLENGTH(rounding) != 1 || INTEGER(rounding)[0] < 0)
    Rf_error("hypercast: rounding must be one non-negative integer code");
  r->rounding = (uint32_t)INTEGER(rounding)[0];
  if (VECTOR_ELT(layout, L_MIN_LENGTH) != R_NilValue) {
    r->min_length = length_value(VECTOR_ELT(layout, L_MIN_LENGTH));
    r->flags |= HyperCastRawConstraints_HAS_MIN_LENGTH;
  }
  if (VECTOR_ELT(layout, L_MAX_LENGTH) != R_NilValue) {
    r->max_length = length_value(VECTOR_ELT(layout, L_MAX_LENGTH));
    r->flags |= HyperCastRawConstraints_HAS_MAX_LENGTH;
  }
  if (asLogical(VECTOR_ELT(layout, L_TRIM)) == TRUE)
    r->flags |= HyperCastRawConstraints_TRIM;
  SEXP e = VECTOR_ELT(layout, L_ENUM);
  if (e == R_NilValue)
    return 0;
  /* Declared, even with no entries: the core refuses an empty enum (12), never reads it as
   * none. */
  r->flags |= HyperCastRawConstraints_HAS_ENUM;
  if (XLENGTH(e) == 0)
    return 0;
  if (d->door == TEXT_DOOR) {
    pack_text_enum(e, native, d);
    return 0;
  }
  R_xlen_t n = XLENGTH(e);
  HyperCastDecimal *entries = (HyperCastDecimal *)R_alloc((size_t)n, sizeof *entries);
  for (R_xlen_t k = 0; k < n; k++) {
    int32_t rc = declared_value(e, k, native, &entries[k]);
    if (rc != 0)
      return rc;
  }
  d->enum_ptr = (const uint8_t *)entries;
  d->enum_len = (uintptr_t)n * sizeof *entries;
  return 0;
}

/* The declaration as the core judges it: 0, or the code of the first rule it breaks. */
SEXP C_constraints_check(SEXP layout, SEXP native_mode) {
  struct declaration d;
  int32_t rc = declaration_arg(layout, asInteger(native_mode), &d);
  if (rc == 0)
    rc = cast_constraints_check(core_door(d.door), 0, &d.raw, d.enum_ptr, d.enum_len);
  return ScalarInteger(rc);
}

/* A declaration hypercast_constraints() judged no longer lays out: it was edited since. */
static void declaration_at_cast(SEXP layout, int native, struct declaration *d) {
  if (declaration_arg(layout, native, d) != 0)
    Rf_error("hypercast: these constraints no longer lay out as hypercast_constraints() "
             "judged them (were they edited?); declare them again");
}

static void checked_contract_violation(void) {
  Rf_error("hypercast: the core refused these constraints or the format at the cast (a "
           "contract violation this package checks for first; please report it)");
}

/* Row i passed: the value, with the text door's own rules for it, adjusted and enum_index. */
static void checked_ok(struct columns *c, R_xlen_t i, const struct declaration *d, SEXP s,
                       const uint8_t *p, uintptr_t n, int chars, const union value *v,
                       const HyperCastRawCheck *check) {
  if (d->door == TEXT_DOOR && check->enum_index != UINT32_MAX) {
    /* The caller's own declared entry, not a copy decoded from the input. */
    if (d->entries == R_NilValue || check->enum_index >= (uint64_t)XLENGTH(d->entries))
      Rf_error("hypercast: the core matched an enum entry that was not declared (please "
               "report it)");
    row_ok(c, i, TEXT_DOOR, NULL);
    SET_STRING_ELT(c->strings, i, STRING_ELT(d->entries, check->enum_index));
  } else if (d->door == TEXT_DOOR) {
    text_row(c, i, s, p, n, chars, &v->span);
    if (c->ok[i] != TRUE)
      return;
  } else {
    row_ok(c, i, d->door, v);
  }
  c->adjusted[i] = check->adjusted != 0;
  c->enum_index[i] = check->enum_index == UINT32_MAX || check->enum_index >= (uint32_t)INT_MAX
                         ? NA_INTEGER
                         : (int)check->enum_index + 1;
}

/*
 * x: a character vector, or a list whose elements are raw vectors or NULL. layout:
 * constraints_layout()'s list. format: a packed num_format for a numeric door, NULL for text.
 */
SEXP C_cast_checked(SEXP x, SEXP layout, SEXP format, SEXP native_mode) {
  int native = asInteger(native_mode);
  struct declaration d;
  declaration_at_cast(layout, native, &d);
  HyperCastRawNumFormat fmt;
  if (d.door == TEXT_DOOR) {
    if (format != R_NilValue)
      Rf_error("hypercast: the text door takes no format");
  } else if (TYPEOF(format) == RAWSXP && XLENGTH(format) == HyperCastRawNumFormat_SIZE) {
    read_format(format, &fmt);
  } else {
    Rf_error("hypercast: format must be a packed num_format");
  }
  int is_list = TYPEOF(x) == VECSXP;
  if (!is_list && TYPEOF(x) != STRSXP)
    Rf_error("hypercast: x must be a character vector or a list of raw vectors");
  uint32_t door = core_door(d.door);
  R_xlen_t n = XLENGTH(x);
  struct columns c;
  columns_alloc(&c, DOORS[d.door].kind, n, 1);
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point(i);
    const void *vmax = vmaxget();
    const uint8_t *p;
    uintptr_t len;
    int chars;
    SEXP s;
    if (!element(x, is_list, i, native, &p, &len, &chars, &s)) {
      row_missing(&c, i);
      continue;
    }
    union value v;
    HyperCastRawFaultDetail f = fault_slot();
    HyperCastRawCheck check = {0, 0, UINT32_MAX};
    int32_t rc = cast_checked_detailed(door, 0, d.door == TEXT_DOOR ? NULL : &fmt, &d.raw,
                                       d.enum_ptr, d.enum_len, p, len, &v, &f, &check);
    if (rc == 0) {
      checked_ok(&c, i, &d, s, p, len, chars, &v, &check);
    } else if (rc >= 1 && rc <= 3) {
      HyperCastRawFault span = span_of(&f);
      present(p, len, chars, &span);
      row_fault(&c, i, rc, span.offset, span.len, f.detail);
    } else if (rc == 4) {
      HyperCastRawFault span = span_of(&f);
      present(p, len, chars, &span);
      row_violation(&c, i, check.keyword, span.offset, span.len);
    } else {
      checked_contract_violation();
    }
    vmaxset(vmax);
  }
  UNPROTECT(1);
  return c.list;
}

/* x: a double vector; NA_real_ is missing, while NaN is a number the core reads. */
SEXP C_cast_checked_double(SEXP x, SEXP layout, SEXP native_mode) {
  struct declaration d;
  declaration_at_cast(layout, asInteger(native_mode), &d);
  if (d.door == TEXT_DOOR)
    Rf_error("hypercast: the text door takes text");
  if (TYPEOF(x) != REALSXP)
    Rf_error("hypercast: x must be a double vector");
  uint32_t door = core_door(d.door);
  R_xlen_t n = XLENGTH(x);
  struct columns c;
  columns_alloc(&c, DOORS[d.door].kind, n, 1);
  const double *xs = REAL(x);
  for (R_xlen_t i = 0; i < n; i++) {
    interrupt_point(i);
    if (R_IsNA(xs[i])) {
      row_missing(&c, i);
      continue;
    }
    union value v;
    HyperCastRawFaultDetail f = fault_slot();
    HyperCastRawCheck check = {0, 0, UINT32_MAX};
    int32_t rc = cast_checked_detailed_from_f64(door, 0, &d.raw, d.enum_ptr, d.enum_len, xs[i], &v,
                                                &f, &check);
    if (rc == 0) {
      checked_ok(&c, i, &d, R_NilValue, NULL, 0, 0, &v, &check);
    } else if (rc >= 1 && rc <= 3) {
      row_fault(&c, i, rc, f.offset, f.len, f.detail);
    } else if (rc == 4) {
      row_violation(&c, i, check.keyword, f.offset, f.len);
    } else {
      checked_contract_violation();
    }
  }
  UNPROTECT(1);
  return c.list;
}

/*
 * One string through cast_declared_decimal, for the declarations.json replay: its code and,
 * on 0, the raw triple (the 96-bit magnitude as decimal digits, the scale, the sign) and the
 * canonical text.
 */
SEXP C_declared_decimal(SEXP x, SEXP native_mode) {
  if (TYPEOF(x) != STRSXP || XLENGTH(x) != 1 || STRING_ELT(x, 0) == NA_STRING)
    Rf_error("hypercast: x must be a single string");
  HyperCastDecimal d;
  memset(&d, 0, sizeof d);
  int32_t rc = declared_value(x, 0, asInteger(native_mode), &d);
  const char *names[] = {"code", "magnitude", "scale", "negative", "value"};
  SEXP out = PROTECT(allocVector(VECSXP, 5));
  SEXP nm = PROTECT(allocVector(STRSXP, 5));
  for (int k = 0; k < 5; k++)
    SET_STRING_ELT(nm, k, mkChar(names[k]));
  setAttrib(out, R_NamesSymbol, nm);
  SET_VECTOR_ELT(out, 0, ScalarInteger(rc));
  if (rc == 0) {
    HyperCastDecimal magnitude = d;
    magnitude.scale = 0;
    magnitude.negative = 0;
    SET_VECTOR_ELT(out, 1, ScalarString(decimal_string(&magnitude)));
    SET_VECTOR_ELT(out, 2, ScalarInteger(d.scale));
    SET_VECTOR_ELT(out, 3, ScalarLogical(d.negative != 0));
    SET_VECTOR_ELT(out, 4, ScalarString(decimal_string(&d)));
  } else {
    SET_VECTOR_ELT(out, 1, ScalarString(NA_STRING));
    SET_VECTOR_ELT(out, 2, ScalarInteger(NA_INTEGER));
    SET_VECTOR_ELT(out, 3, ScalarLogical(NA_LOGICAL));
    SET_VECTOR_ELT(out, 4, ScalarString(NA_STRING));
  }
  UNPROTECT(2);
  return out;
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

/*
 * Whether the core this shared object is linked with enforces a strict grouping policy: a
 * strict Western format must refuse "12,34" as a group of the wrong size (code 2, detail 11,
 * group_size). A core that ignores the policy bits, as a 1.0 core does, reads it legacy and
 * returns 0 (1234), so a strict request would silently be parsed legacy; the package refuses
 * to load against one (R_init_hypercast). It is a capability probe, not a version number
 * compare: a 1.1 core under development still names itself 1.0.0 until prepare-release
 * moves the version, and the policy is what the package depends on. A 1.0 core has no
 * cast_detailed to link against at all, so the probe is the guard against a core that
 * links and still ignores the policy.
 */
int hypercast_core_enforces_grouping(void) {
  HyperCastRawNumFormat f;
  memset(&f, 0, sizeof f);
  f.size = HyperCastRawNumFormat_SIZE;
  f.format.flags = HyperCastNumFormat_GROUPING | HyperCastNumFormat_GROUPING_WESTERN;
  f.format.decimal_sep.bytes[0] = '.';
  f.format.decimal_sep.len = 1;
  f.format.group_sep.bytes[0] = ',';
  f.format.group_sep.len = 1;
  const uint8_t text[] = {'1', '2', ',', '3', '4'};
  int32_t out = 0;
  HyperCastRawFaultDetail fault = fault_slot();
  int32_t rc = cast_detailed(4, 0, &f, NULL, NULL, NULL, 0, text, sizeof text, &out, &fault);
  return rc == 2 && fault.detail == 11 && fault.offset == 2 && fault.len == 3;
}

SEXP C_core_guard(void) { return ScalarLogical(hypercast_core_enforces_grouping()); }

/*
 * A month vocabulary as the core judges it: 0 when it is accepted, 1 when the core refuses it
 * (a -1 from cast_detailed at the ordered date door, before anything is read), as
 * month_names() judges every rule it can state itself and leaves the one only the core's
 * case folding decides, a folded duplicate, to this.
 */
SEXP C_month_names_check(SEXP vocab) {
  if (TYPEOF(vocab) != RAWSXP)
    Rf_error("hypercast: months must be packed month names");
  uintptr_t len = (uintptr_t)XLENGTH(vocab);
  union value v;
  HyperCastRawFaultDetail fault = fault_slot();
  int32_t rc =
      cast_detailed(20, 1, NULL, NULL, NULL, len ? RAW(vocab) : NULL, len, NULL, 0, &v, &fault);
  return ScalarInteger(rc == -1);
}

/* The packed core version (major << 16 | minor << 8 | patch); R unpacks it. */
SEXP C_native_version(void) { return ScalarInteger((int)hypercast_version()); }
