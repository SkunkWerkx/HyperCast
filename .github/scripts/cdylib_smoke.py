"""Loads the shared library every FFI binding loads and calls all thirty-six C exports.

The Forge's check-cdylib job (hyper-check-core.yml) runs this against a freshly built no_std library on every platform
the forge ships: a library that links but will not load, or loads but misbehaves at the ABI,
fails here before any binding sees it. ctypes because Python is on every runner and opens
the library the way a host does (dlopen, LoadLibrary), with nothing compiled in between.

Every export is called at least twice: once with input it accepts, the value checked, and
once with input it rejects, the reason (and the fault span where one is set) checked. The
out-structs mirror rust/src/verdict.rs's #[repr(C)] layouts and NumFormat and DayPeriods
mirror rust/src/abi.rs's RawNumFormat and RawDayPeriods (and Constraints, Check and Span its
RawConstraints, RawCheck and RawSpan, and FaultDetail and DateOptions its RawFaultDetail and
RawDateOptions), as the Go and Swift bindings do; a layout drift reads back wrong values.

usage: python cdylib_smoke.py <path-to-library> <rust/Cargo.toml>
"""

import ctypes
import re
import sys
import uuid
from pathlib import Path

lib_path, manifest = sys.argv[1], sys.argv[2]
want = re.search(r'^version = "([^"]+)"', Path(manifest).read_text(encoding="utf-8"), re.MULTILINE)[1]

OK, EMPTY, MALFORMED, OUT_OF_RANGE, VIOLATION, CONTRACT_VIOLATION = 0, 1, 2, 3, 4, -1


class Fault(ctypes.Structure):
    _fields_ = [("offset", ctypes.c_uint32), ("len", ctypes.c_uint32)]


class Symbol8(ctypes.Structure):
    _fields_ = [("bytes", ctypes.c_uint8 * 8), ("len", ctypes.c_uint8)]


class Symbol16(ctypes.Structure):
    _fields_ = [("bytes", ctypes.c_uint8 * 16), ("len", ctypes.c_uint8)]


class NumFormat(ctypes.Structure):
    _fields_ = [("size", ctypes.c_uint32), ("flags", ctypes.c_uint32), ("digit_zero", ctypes.c_uint32),
                ("decimal_sep", Symbol8), ("group_sep", Symbol8),
                ("currency_decimal_sep", Symbol8), ("currency_group_sep", Symbol8),
                ("minus_sign", Symbol8), ("plus_sign", Symbol8), ("percent", Symbol8),
                ("currency", Symbol16)]


class DayPeriods(ctypes.Structure):
    _fields_ = [("size", ctypes.c_uint32), ("am", Symbol16), ("pm", Symbol16), ("padding", ctypes.c_uint8 * 2)]


class Decimal(ctypes.Structure):
    _fields_ = [("lo", ctypes.c_uint64), ("hi", ctypes.c_uint32),
                ("scale", ctypes.c_uint8), ("negative", ctypes.c_bool)]


class Constraints(ctypes.Structure):
    _fields_ = [("size", ctypes.c_uint32), ("flags", ctypes.c_uint32), ("rounding", ctypes.c_uint32),
                ("min_length", ctypes.c_uint32), ("max_length", ctypes.c_uint32), ("reserved", ctypes.c_uint32),
                ("lower", Decimal), ("upper", Decimal), ("multiple_of", Decimal)]


class Check(ctypes.Structure):
    _fields_ = [("keyword", ctypes.c_uint32), ("adjusted", ctypes.c_uint32), ("enum_index", ctypes.c_uint32)]


class Span(ctypes.Structure):
    _fields_ = [("offset", ctypes.c_uint32), ("len", ctypes.c_uint32)]


class FaultDetail(ctypes.Structure):
    _fields_ = [("size", ctypes.c_uint32), ("offset", ctypes.c_uint32), ("len", ctypes.c_uint32),
                ("detail", ctypes.c_uint32)]


class DateOptions(ctypes.Structure):
    _fields_ = [("size", ctypes.c_uint32), ("flags", ctypes.c_uint32), ("pivot", ctypes.c_uint32)]


class Timestamp(ctypes.Structure):
    _fields_ = [("seconds", ctypes.c_int64), ("nanos", ctypes.c_int32)]


class Duration(ctypes.Structure):
    _fields_ = [("seconds", ctypes.c_int64), ("nanos", ctypes.c_int32)]


class Date(ctypes.Structure):
    _fields_ = [("year", ctypes.c_uint16), ("month", ctypes.c_uint8), ("day", ctypes.c_uint8)]


class CivilDateTime(ctypes.Structure):
    _fields_ = [("date", Date), ("nanos_of_day", ctypes.c_uint64)]


GROUPING, PARENS, EXPONENT, RADIX_PREFIX, PERCENT, _, CURRENCY, TRAILING_SIGN = (1 << i for i in range(8))
ALL = GROUPING | PARENS | EXPONENT | RADIX_PREFIX | PERCENT | CURRENCY
GROUPING_WESTERN, GROUPING_INDIAN = 1 << 8, 2 << 8


def symbol(field, text):
    """Writes `text` into an inline symbol field: its UTF-8 bytes, then the length."""
    raw = text.encode()
    field.bytes[:len(raw)] = list(raw)
    field.len = len(raw)


def num_format(decimal_sep, group_sep, flags, currency="", digit_zero="", **symbols):
    """A format in the 92-byte layout; `symbols` names any other field (`minus_sign=`)."""
    fmt = NumFormat(ctypes.sizeof(NumFormat), flags, ord(digit_zero) if digit_zero else 0)
    for name, text in {"decimal_sep": decimal_sep, "group_sep": group_sep, "currency": currency, **symbols}.items():
        symbol(getattr(fmt, name), text)
    return fmt


def day_periods(am, pm):
    periods = DayPeriods(ctypes.sizeof(DayPeriods))
    symbol(periods.am, am)
    symbol(periods.pm, pm)
    return periods


lib = ctypes.CDLL(lib_path)
txt, size, u32, i32, P = ctypes.c_char_p, ctypes.c_size_t, ctypes.c_uint32, ctypes.c_int32, ctypes.POINTER
FaultP, FormatP, PeriodsP = P(Fault), P(NumFormat), P(DayPeriods)

lib.hypercast_version.restype, lib.hypercast_version.argtypes = u32, []
PLAIN = {"cast_bool": ctypes.c_uint8, "cast_char": ctypes.c_uint32, "cast_uuid": ctypes.c_uint8 * 16, "cast_timestamp": Timestamp,
         "cast_date": Date, "cast_time": ctypes.c_uint64, "cast_duration": Duration}
NUMERIC = {"cast_i8": ctypes.c_int8, "cast_i16": ctypes.c_int16, "cast_i32": ctypes.c_int32,
           "cast_i64": ctypes.c_int64, "cast_u8": ctypes.c_uint8, "cast_u16": ctypes.c_uint16,
           "cast_u32": ctypes.c_uint32, "cast_u64": ctypes.c_uint64, "cast_f32": ctypes.c_float,
           "cast_f64": ctypes.c_double, "cast_decimal": Decimal}
SELECTOR = {"cast_unix": Timestamp, "cast_excel_serial": Timestamp, "cast_date_ordered": Date}
# The civil date-time door takes the declared day periods after the order (null: AM/PM only).
DATETIME = {"cast_datetime": CivilDateTime}
for name, out in PLAIN.items():
    fn = getattr(lib, name)
    fn.restype, fn.argtypes = i32, [txt, size, P(out), FaultP]
for name, out in NUMERIC.items():
    fn = getattr(lib, name)
    fn.restype, fn.argtypes = i32, [txt, size, FormatP, P(out), FaultP]
for name, out in SELECTOR.items():
    fn = getattr(lib, name)
    fn.restype, fn.argtypes = i32, [txt, size, u32, P(out), FaultP]
for name, out in DATETIME.items():
    fn = getattr(lib, name)
    fn.restype, fn.argtypes = i32, [txt, size, u32, PeriodsP, P(out), FaultP]
# The typed doors take a double instead of text, and give a fault no span.
TYPED = {"cast_decimal_from_f64": (Decimal, []), "cast_excel_serial_from_f64": (CivilDateTime, [u32]),
         "cast_excel_time": (ctypes.c_uint64, []), "cast_excel_duration": (Duration, [])}
for name, (out, extra) in TYPED.items():
    fn = getattr(lib, name)
    fn.restype, fn.argtypes = i32, [ctypes.c_double, *extra, P(out), FaultP]
# The validation constraints: the checked doors take a declaration, an `enum` list and a
# nullable RawCheck, and write the door's own type through a void pointer.
ConstraintsP, CheckP, SpanP, bytes_p = P(Constraints), P(Check), P(Span), ctypes.c_char_p
lib.cast_checked.restype, lib.cast_checked.argtypes = i32, [u32, u32, FormatP, ConstraintsP, bytes_p, size, txt, size,
                                                            ctypes.c_void_p, FaultP, CheckP]
lib.cast_checked_from_f64.restype, lib.cast_checked_from_f64.argtypes = i32, [u32, ctypes.c_double, ConstraintsP,
                                                                              bytes_p, size, ctypes.c_void_p,
                                                                              FaultP, CheckP]
lib.cast_constraints_check.restype, lib.cast_constraints_check.argtypes = i32, [u32, u32, ConstraintsP, bytes_p, size]
lib.cast_text.restype, lib.cast_text.argtypes = i32, [txt, size, SpanP, FaultP]
lib.cast_declared_decimal.restype, lib.cast_declared_decimal.argtypes = i32, [bytes_p, size, P(Decimal)]
# The detailed exports: a door code and its parameter, every out through a void pointer.
DetailP, DateOptionsP, void_p, f64 = P(FaultDetail), P(DateOptions), ctypes.c_void_p, ctypes.c_double
lib.cast_detailed.restype, lib.cast_detailed.argtypes = i32, [u32, u32, FormatP, PeriodsP, DateOptionsP, bytes_p,
                                                              size, txt, size, void_p, DetailP]
lib.cast_detailed_from_f64.restype, lib.cast_detailed_from_f64.argtypes = i32, [u32, u32, f64, void_p, DetailP]
lib.cast_checked_detailed.restype, lib.cast_checked_detailed.argtypes = i32, [u32, u32, FormatP, ConstraintsP,
                                                                              bytes_p, size, txt, size, void_p,
                                                                              DetailP, CheckP]
lib.cast_checked_detailed_from_f64.restype, lib.cast_checked_detailed_from_f64.argtypes = i32, [
    u32, u32, ConstraintsP, bytes_p, size, f64, void_p, DetailP, CheckP]
OUT_TYPES = {**PLAIN, **NUMERIC, **SELECTOR, **DATETIME, **{name: out for name, (out, _) in TYPED.items()}}
called = set()


def check(cond, what):
    if not cond:
        sys.exit(f"cdylib smoke: {what}")


def call(name, text, *extra):
    """Calls one export the way every binding does; returns (status, out, fault)."""
    called.add(name)
    raw = text.encode()
    out, fault = OUT_TYPES[name](), Fault(0xFFFF, 0xFFFF)
    status = getattr(lib, name)(raw, len(raw), *extra, ctypes.byref(out), ctypes.byref(fault))
    return status, out, fault


def ok(name, text, *extra):
    status, out, _ = call(name, text, *extra)
    check(status == OK, f"{name}({text!r}) returned {status}, expected ok")
    return out


def typed(name, value, *extra):
    """Calls one typed export; returns (status, out, fault)."""
    called.add(name)
    out, fault = OUT_TYPES[name](), Fault(0xFFFF, 0xFFFF)
    status = getattr(lib, name)(value, *extra, ctypes.byref(out), ctypes.byref(fault))
    return status, out, fault


def fails(name, text, reason, *extra, span=None):
    status, _, fault = call(name, text, *extra)
    check(status == reason, f"{name}({text!r}) returned {status}, expected {reason}")
    if span is not None:
        check((fault.offset, fault.len) == span,
              f"{name}({text!r}) fault {(fault.offset, fault.len)}, expected {span}")


for shape, size in ((Fault, 8), (NumFormat, 92), (DayPeriods, 40), (Decimal, 16), (Timestamp, 16),
                    (Duration, 16), (Date, 4), (CivilDateTime, 16)):
    check(ctypes.sizeof(shape) == size,
          f"{shape.__name__} is {ctypes.sizeof(shape)} bytes; rust/src/abi.rs pins {size}")

v = lib.hypercast_version()
got = f"{v >> 16}.{(v >> 8) & 0xFF}.{v & 0xFF}"
check(got == want, f"hypercast_version reports {got}, rust/Cargo.toml says {want}")

check(ok("cast_bool", "Yes").value == 1 and ok("cast_bool", "off").value == 0, "cast_bool value")
fails("cast_bool", "maybe", MALFORMED, span=(0, 5))
fails("cast_bool", "  ", EMPTY)
check(ok("cast_char", "U+00E9").value == 0xE9 and ok("cast_char", " ").value == 0x20, "cast_char value")
fails("cast_char", "65x", MALFORMED, span=(2, 1))
fails("cast_char", "U+D800", OUT_OF_RANGE, span=(0, 6))

want_uuid = uuid.UUID("01020304-0506-0708-090a-0b0c0d0e0f10")
check(bytes(ok("cast_uuid", "{01020304-0506-0708-090A-0B0C0D0E0F10}")) == want_uuid.bytes, "cast_uuid value")
fails("cast_uuid", "not-a-guid", MALFORMED)

ts = ok("cast_timestamp", "2026-01-02T15:04:05.123Z")
check((ts.seconds, ts.nanos) == (1767366245, 123000000), "cast_timestamp value")
fails("cast_timestamp", "0000-01-01T00:00:00Z", OUT_OF_RANGE)

d = ok("cast_date", "2024-02-29")
check((d.year, d.month, d.day) == (2024, 2, 29), "cast_date value")
fails("cast_date", "2026-02-29", OUT_OF_RANGE, span=(8, 2))

check(ok("cast_time", "15:04:05.123").value == 54245123000000, "cast_time value")
fails("cast_time", "25:00", OUT_OF_RANGE, span=(0, 2))

du = ok("cast_duration", "PT1H30M")
check((du.seconds, du.nanos) == (5400, 0), "cast_duration value")
fails("cast_duration", "315576000001s", OUT_OF_RANGE)

# The numeric doors, each under the invariant format (a null pointer), then the integer and
# float doors under a declared one so RawNumFormat's layout is exercised end to end.
for name, top in [("cast_i8", 127), ("cast_i16", 32767), ("cast_i32", 2**31 - 1), ("cast_i64", 2**63 - 1),
                  ("cast_u8", 255), ("cast_u16", 65535), ("cast_u32", 2**32 - 1), ("cast_u64", 2**64 - 1)]:
    check(ok(name, f" {top} ", None).value == top, f"{name} max value")
    check(ok(name, "42", None).value == 42, f"{name} value")
    fails(name, str(top + 1), OUT_OF_RANGE, None)
    fails(name, "12x", MALFORMED, None)
german = num_format(",", ".", ALL)
check(ok("cast_i32", "1.234.567", ctypes.byref(german)).value == 1234567, "cast_i32 under a declared format")
fails("cast_i32", "1,5", MALFORMED, ctypes.byref(german), span=(1, 1))

check(ok("cast_f32", "1.5", None).value == 1.5, "cast_f32 value")
fails("cast_f32", "NaN", MALFORMED, None)
check(ok("cast_f64", "-3.25e2", None).value == -325.0, "cast_f64 value")
check(ok("cast_f64", "1.234,5", ctypes.byref(german)).value == 1234.5, "cast_f64 under a declared format")
fails("cast_f64", "Infinity", MALFORMED, None)

dec = ok("cast_decimal", "-3.5", None)
check((dec.lo, dec.hi, dec.scale, dec.negative) == (35, 0, 1, True), "cast_decimal value")
# The smallest scale that represents the value: 1234.50 reads back as 12345 at scale 1.
kroner = num_format(",", ".", ALL, "kr.")
dec = ok("cast_decimal", "1.234,50 kr.", ctypes.byref(kroner))
check((dec.lo, dec.hi, dec.scale, dec.negative) == (12345, 0, 1, False), "cast_decimal with a currency symbol")
fails("cast_decimal", "1.2.3", MALFORMED, None)

# A format the core cannot honor is a contract violation, not a verdict about the text.
same = num_format(".", ".", ALL)
fails("cast_decimal", "1", CONTRACT_VIOLATION, ctypes.byref(same))
unsized = num_format(".", ",", ALL)
unsized.size = 0
fails("cast_decimal", "1", CONTRACT_VIOLATION, ctypes.byref(unsized))
# So is a decimal separator that is white space (U+00A0), which no input could match, a
# sign or percent symbol containing white space, and a currency symbol starting with it.
fails("cast_decimal", "1", CONTRACT_VIOLATION, ctypes.byref(num_format("\u00a0", ",", ALL)))
fails("cast_decimal", "1", CONTRACT_VIOLATION, ctypes.byref(num_format(".", ",", ALL, minus_sign="\u202f")))
fails("cast_decimal", "1", CONTRACT_VIOLATION, ctypes.byref(num_format(".", ",", ALL, "\u00a0\u20ac")))

# The sweep's fields, each read beside its ASCII default: a declared minus sign and a
# trailing sign, the currency separators once the symbol is seen, a declared percent,
# native digits, and a separator of two characters.
estonian = num_format(",", "\u00a0", ALL | TRAILING_SIGN, minus_sign="\u2212")
check(ok("cast_i32", "\u22125", ctypes.byref(estonian)).value == -5, "cast_i32 with a declared minus sign")
check(ok("cast_i32", "5-", ctypes.byref(estonian)).value == -5, "cast_i32 with a trailing sign")
fails("cast_i32", "\u22125", MALFORMED, ctypes.byref(german), span=(0, 3))
en_de = num_format(",", ".", ALL, "\u20ac", currency_decimal_sep=".", currency_group_sep=",")
check(ok("cast_f64", "\u20ac1,234.50", ctypes.byref(en_de)).value == 1234.5, "cast_f64 with currency separators")
check(ok("cast_f64", "1.234,50", ctypes.byref(en_de)).value == 1234.5, "cast_f64 number separators without the symbol")
arabic = num_format("\u066b", "\u066c", ALL, digit_zero="\u0660", percent="\u066a")
check(ok("cast_f64", "\u0661\u066c\u0662\u0663\u0664\u066b\u0665", ctypes.byref(arabic)).value == 1234.5,
      "cast_f64 with native digits")
check(ok("cast_f64", "\u0665\u0660\u066a", ctypes.byref(arabic)).value == 0.5, "cast_f64 with a declared percent")
wide = num_format(".", ", ", ALL)
check(ok("cast_i32", "1, 234", ctypes.byref(wide)).value == 1234, "cast_i32 with a two-character separator")

u = ok("cast_unix", "1700000000123", 2)
check((u.seconds, u.nanos) == (1700000000, 123000000), "cast_unix value")
fails("cast_unix", "253402300800", OUT_OF_RANGE, 1)
fails("cast_unix", "1", CONTRACT_VIOLATION, 0)

e = ok("cast_excel_serial", "25569", 1)
check((e.seconds, e.nanos) == (0, 0), "cast_excel_serial 1900 epoch")
e = ok("cast_excel_serial", "24107", 2)
check((e.seconds, e.nanos) == (0, 0), "cast_excel_serial 1904 epoch")
fails("cast_excel_serial", "60", OUT_OF_RANGE, 1, span=(0, 2))
fails("cast_excel_serial", "1", CONTRACT_VIOLATION, 3)

d = ok("cast_date_ordered", "1/7/2026", 3)
check((d.year, d.month, d.day) == (2026, 7, 1), "cast_date_ordered value")
fails("cast_date_ordered", "13/7/2026", OUT_OF_RANGE, 2, span=(0, 2))
fails("cast_date_ordered", "1/7/2026", CONTRACT_VIOLATION, 4)

dt = ok("cast_datetime", "1/7/2026 3:04 PM", 2, None)
check((dt.date.year, dt.date.month, dt.date.day, dt.nanos_of_day) == (2026, 1, 7, 54240000000000),
      "cast_datetime value")
fails("cast_datetime", "1/7/2026 25:04", OUT_OF_RANGE, 2, None, span=(9, 2))
fails("cast_datetime", "1/7/2026 3:04 PM", CONTRACT_VIOLATION, 0, None)
# Declared day periods, read beside AM/PM; one with whitespace at an edge is a caller bug.
german_day = day_periods("vorm.", "nachm.")
dt = ok("cast_datetime", "1/7/2026 3:04 nachm.", 3, ctypes.byref(german_day))
check((dt.date.year, dt.date.month, dt.date.day, dt.nanos_of_day) == (2026, 7, 1, 54240000000000),
      "cast_datetime with declared day periods")
fails("cast_datetime", "1/7/2026 3:04 nachm.", MALFORMED, 3, None, span=(13, 1))
fails("cast_datetime", "1/7/2026 3:04 PM", CONTRACT_VIOLATION, 2, ctypes.byref(day_periods(" am", "pm")))
fails("cast_datetime", "1/7/2026 3:04 PM", CONTRACT_VIOLATION, 2, ctypes.byref(day_periods("am", "\u202fPM")))

status, dec, _ = typed("cast_decimal_from_f64", 0.1 + 0.2)
check(status == OK and (dec.lo, dec.hi, dec.scale, dec.negative) == (30000000000000004, 0, 17, False),
      "cast_decimal_from_f64 value")
status, _, fault = typed("cast_decimal_from_f64", float("nan"))
check(status == MALFORMED and (fault.offset, fault.len) == (0, 0), "cast_decimal_from_f64 NaN, spanless")
status, wall, _ = typed("cast_excel_serial_from_f64", 45292.75, 1)
check(status == OK and (wall.date.year, wall.date.month, wall.date.day, wall.nanos_of_day)
      == (2024, 1, 1, 64800000000000), "cast_excel_serial_from_f64 value")
check(typed("cast_excel_serial_from_f64", 60.0, 1)[0] == OUT_OF_RANGE, "cast_excel_serial_from_f64 phantom")
check(typed("cast_excel_serial_from_f64", 1.0, 3)[0] == CONTRACT_VIOLATION, "cast_excel_serial_from_f64 epoch")
status, nanos, _ = typed("cast_excel_time", 0.75)
check(status == OK and nanos.value == 64800000000000, "cast_excel_time value")
check(typed("cast_excel_time", -0.5)[0] == MALFORMED, "cast_excel_time negative")
status, span, _ = typed("cast_excel_duration", -0.25)
check(status == OK and (span.seconds, span.nanos) == (-21600, 0), "cast_excel_duration value")
check(typed("cast_excel_duration", 1e10)[0] == OUT_OF_RANGE, "cast_excel_duration window")



# The validation constraints. HAS_MINIMUM 1, HAS_MAXIMUM 4, HAS_MULTIPLE_OF 16,
# HAS_MAX_LENGTH 64, TRIM 128, HAS_ENUM 256 (the `enum` list is read only under it);
# rounding 1 is midpoint.
F64, I32, TEXT = 11, 4, 18


def decimal(lo, scale=0, negative=False):
    return Decimal(lo, 0, scale, negative)


def constraints(flags=0, rounding=0, lower=None, upper=None, multiple_of=None, max_length=0):
    declared = Constraints(ctypes.sizeof(Constraints), flags, rounding, 0, max_length, 0)
    for name, value in (("lower", lower), ("upper", upper), ("multiple_of", multiple_of)):
        if value is not None:
            setattr(declared, name, value)
    return declared


def packed_text(*entries):
    """Text `enum` entries as the ABI packs them: a little-endian u32 length, then the UTF-8."""
    return b"".join(len(e.encode()).to_bytes(4, "little") + e.encode() for e in entries)


def checked(door, declared, entries, text, out):
    called.add("cast_checked")
    raw = text.encode()
    fault, report = Fault(0xFFFF, 0xFFFF), Check(7, 7, 7)
    status = lib.cast_checked(door, 0, None, ctypes.byref(declared), entries, len(entries), raw, len(raw),
                              ctypes.byref(out), ctypes.byref(fault), ctypes.byref(report))
    return status, out, (fault.offset, fault.len), (report.keyword, report.adjusted, report.enum_index)


for shape, size_ in ((Constraints, 72), (Check, 12), (Span, 8)):
    check(ctypes.sizeof(shape) == size_,
          f"{shape.__name__} is {ctypes.sizeof(shape)} bytes; rust/src/abi.rs pins {size_}")
NO_INDEX = 0xFFFFFFFF
latitude = constraints(1 | 4 | 16, 1, decimal(90, 0, True), decimal(90), decimal(1, 6))
status, value, _, report = checked(F64, latitude, b"", "45.1234567", ctypes.c_double())
check(status == OK and value.value == 45.123457 and report == (0, 1, NO_INDEX), "cast_checked rounds and reports")
status, value, fault, report = checked(F64, latitude, b"", "91", ctypes.c_double())
check(status == VIOLATION and fault == (0, 2) and report[0] == 3 and value.value == 0, "cast_checked violation")
status, _, fault, report = checked(F64, latitude, b"", "9x", ctypes.c_double())
check(status == MALFORMED and fault == (1, 1) and report == (7, 7, 7), "cast_checked fault leaves check alone")
# A float door judges the float it delivers too: 0.99999999999 is 1.0 at f32, past exclusiveMaximum 1.
below_one = constraints(8, upper=decimal(1))
status, _, fault, report = checked(10, below_one, b"", "0.99999999999", ctypes.c_float())
check(status == VIOLATION and fault == (0, 13) and report[0] == 4, "cast_checked delivered float")
status, value, _, _ = checked(10, below_one, b"", "0.9999999", ctypes.c_float())
check(status == OK and value.value < 1, "cast_checked delivered float below the bound")
sexes = packed_text("Female", "Male", "Other")
status, span, _, report = checked(TEXT, constraints(64 | 128 | 256, max_length=6), sexes, "\u00a0MALE ", Span())
check(status == OK and (span.offset, span.len) == (2, 4) and report == (0, 0, 1), "cast_checked text enum")
status, _, fault, report = checked(TEXT, constraints(64 | 128 | 256, max_length=6), sexes, " Mail ", Span())
check(status == VIOLATION and fault == (1, 4) and report[0] == 8, "cast_checked text enum violation")
for bad in (0, 71, 73):
    unsized_ = constraints()
    unsized_.size = bad
    check(checked(I32, unsized_, b"", "1", ctypes.c_int32())[0] == CONTRACT_VIOLATION, f"cast_checked size {bad}")
check(checked(12, constraints(), b"", "1", ctypes.c_int32())[0] == CONTRACT_VIOLATION, "cast_checked uuid door")

called.add("cast_checked_from_f64")
out, fault, report = ctypes.c_double(), Fault(0xFFFF, 0xFFFF), Check(7, 7, 7)
status = lib.cast_checked_from_f64(F64, 0.1 + 0.2, ctypes.byref(constraints(16, 1, multiple_of=decimal(1, 1))), None,
                                   0, ctypes.byref(out), ctypes.byref(fault), ctypes.byref(report))
check(status == OK and out.value == 0.3 and report.adjusted == 1, "cast_checked_from_f64 rounds 0.1 + 0.2")
status = lib.cast_checked_from_f64(F64, 91.0, ctypes.byref(latitude), None, 0, ctypes.byref(out),
                                   ctypes.byref(fault), ctypes.byref(report))
check(status == VIOLATION and (fault.offset, fault.len) == (0, 0) and report.keyword == 3,
      "cast_checked_from_f64 violation, spanless")

called.add("cast_constraints_check")
judge = lib.cast_constraints_check
check(judge(F64, 0, ctypes.byref(latitude), None, 0) == 0, "cast_constraints_check accepts")
check(judge(TEXT, 0, ctypes.byref(constraints(256)), sexes, len(sexes)) == 0, "cast_constraints_check text enum")
twice = packed_text("Male", "MALE")
check(judge(TEXT, 0, ctypes.byref(constraints(256)), twice, len(twice)) == 10, "cast_constraints_check duplicate")
check(judge(TEXT, 0, ctypes.byref(constraints()), sexes, len(sexes)) == 11, "cast_constraints_check enum without HAS_ENUM")
check(judge(TEXT, 0, ctypes.byref(constraints(256)), None, 0) == 12, "cast_constraints_check empty enum")
padded = packed_text(" Male")
check(judge(TEXT, 0, ctypes.byref(constraints(128 | 256)), padded, len(padded)) == 13,
      "cast_constraints_check enum entry no trimmed input matches")
check(judge(6, 0, ctypes.byref(constraints(1, lower=decimal(1, 0, True))), None, 0) == 8,
      "cast_constraints_check bound past the door")
check(judge(TEXT, 0, ctypes.byref(constraints(256)), b"\x01\x00\x00\x00\xff", 5) == 9,
      "cast_constraints_check not UTF-8")
check(judge(I32, 0, None, None, 0) == 11, "cast_constraints_check null declaration")

called.add("cast_text")
span, fault = Span(), Fault(0xFFFF, 0xFFFF)
raw = "\U0001F44D\U0001F3FD".encode()
check(lib.cast_text(raw, len(raw), ctypes.byref(span), ctypes.byref(fault)) == OK and (span.offset, span.len) == (0, 8),
      "cast_text value")
check(lib.cast_text(b"ab\xffc", 4, ctypes.byref(span), ctypes.byref(fault)) == MALFORMED
      and (fault.offset, fault.len) == (2, 1), "cast_text invalid UTF-8")
check(lib.cast_text(None, 0, ctypes.byref(span), None) == EMPTY, "cast_text empty")

called.add("cast_declared_decimal")
declared = Decimal(0xA5A5, 0x5A5A, 99, True)
check(lib.cast_declared_decimal(b"1.50", 4, ctypes.byref(declared)) == OK
      and (declared.lo, declared.hi, declared.scale, declared.negative) == (15, 0, 1, False),
      "cast_declared_decimal value")
check(lib.cast_declared_decimal(b"-0", 2, ctypes.byref(declared)) == OK
      and (declared.lo, declared.hi, declared.scale, declared.negative) == (0, 0, 0, False),
      "cast_declared_decimal unsigned zero")
declared = Decimal(0xA5A5, 0x5A5A, 99, True)
check(lib.cast_declared_decimal(b"1e-29", 5, ctypes.byref(declared)) == 8
      and (declared.lo, declared.hi, declared.scale, declared.negative) == (0xA5A5, 0x5A5A, 99, True),
      "cast_declared_decimal refused, out untouched")
check(lib.cast_declared_decimal(b"1\xff", 2, ctypes.byref(declared)) == 8, "cast_declared_decimal not UTF-8")
check(lib.cast_declared_decimal(b"1", 1, None) == -1, "cast_declared_decimal null out")

# The four detailed exports reach the doors by Door's code (rust/src/door.rs) and write a
# RawFaultDetail: the span and `detail`, which names what failed. A declared format is judged
# by 1.1's rules, so its grouping policy (flag bits 8 and 9) holds there and nowhere else.
for shape, size_ in ((FaultDetail, 16), (DateOptions, 12)):
    check(ctypes.sizeof(shape) == size_,
          f"{shape.__name__} is {ctypes.sizeof(shape)} bytes; rust/src/abi.rs pins {size_}")
I32_DOOR, UUID_DOOR, DATE_DOOR, DATE_ORDERED, DECIMAL_DOOR, EXCEL_SERIAL, CHAR_DOOR = 4, 12, 15, 20, 19, 22, 23


def fault_detail():
    return FaultDetail(ctypes.sizeof(FaultDetail), 7, 7, 7)


def detailed(door, param, text, out, fmt=None, options=None, vocab=b""):
    called.add("cast_detailed")
    raw, fault = text.encode(), fault_detail()
    status = lib.cast_detailed(door, param, fmt, None, options, vocab or None, len(vocab), raw, len(raw),
                               ctypes.byref(out), ctypes.byref(fault))
    return status, out, (fault.offset, fault.len, fault.detail)


strict = num_format(".", ",", ALL | GROUPING_WESTERN)
status, value, fault = detailed(I32_DOOR, 0, "12,34", ctypes.c_int32(), ctypes.byref(strict))
check(status == MALFORMED and value.value == 0 and fault == (2, 3, 11), "cast_detailed strict grouping (group_size)")
check(ok("cast_i32", "12,34", ctypes.byref(strict)).value == 1234, "cast_i32 ignores the grouping policy")
status, value, _ = detailed(I32_DOOR, 0, "1,234", ctypes.c_int32(), ctypes.byref(strict))
check(status == OK and value.value == 1234, "cast_detailed strict grouping value")
indian = num_format(".", ",", ALL | GROUPING_INDIAN)
check(detailed(I32_DOOR, 0, "12,34,567", ctypes.c_int32(), ctypes.byref(indian))[1].value == 1234567,
      "cast_detailed Indian grouping")
status, value, fault = detailed(I32_DOOR, 0, "1", ctypes.c_int32(77), ctypes.byref(num_format(".", ",", ALL | (3 << 8))))
check(status == CONTRACT_VIOLATION and value.value == 77 and fault == (7, 7, 7), "cast_detailed policy 3 writes nothing")
check(detailed(24, 0, "1", ctypes.c_int32())[0] == CONTRACT_VIOLATION, "cast_detailed unknown door")
check(detailed(I32_DOOR, 1, "1", ctypes.c_int32())[0] == CONTRACT_VIOLATION, "cast_detailed stray param")
status, value, _ = detailed(UUID_DOOR, 0, "01020304-0506-0708-090a-0b0c0d0e0f10", (ctypes.c_uint8 * 16)())
check(status == OK and bytes(value) == want_uuid.bytes, "cast_detailed uuid")
status, value, fault = detailed(CHAR_DOOR, 0, "U+D800", ctypes.c_uint32())
check(status == OUT_OF_RANGE and fault == (0, 6, 32), "cast_detailed char not_a_scalar")
check(detailed(CHAR_DOOR, 0, "U+20AC", ctypes.c_uint32())[1].value == 0x20AC, "cast_detailed char")
status, _, fault = detailed(DATE_DOOR, 0, "20260107", Date())
check(status == MALFORMED and fault[2] == 16, "cast_detailed date wrong_length")
status, d, _ = detailed(DATE_DOOR, 0, "20260107", Date(), options=ctypes.byref(DateOptions(12, 1, 0)))
check(status == OK and (d.year, d.month, d.day) == (2026, 1, 7), "cast_detailed compact date")
check(detailed(DATE_DOOR, 0, "2026-01-07", Date(), options=ctypes.byref(DateOptions(12, 8, 0)))[0]
      == CONTRACT_VIOLATION, "cast_detailed undefined date option")
months = b"".join(bytes([n + 1]) + len(m).to_bytes(4, "little") + m.encode()
                  for n, m in enumerate("Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec".split()))
status, d, _ = detailed(DATE_ORDERED, 3, "07-Jan-26", Date(), options=ctypes.byref(DateOptions(12, 0, 2029)),
                        vocab=months)
check(status == OK and (d.year, d.month, d.day) == (2026, 1, 7), "cast_detailed month vocabulary and pivot")
check(detailed(DATE_ORDERED, 3, "7-Jan-2026", Date())[0] == MALFORMED, "cast_detailed without a vocabulary")
check(detailed(DATE_ORDERED, 3, "7-Jan-2026", Date(), vocab=months[:-1])[0] == CONTRACT_VIOLATION,
      "cast_detailed vocabulary cut short")
fault = FaultDetail(12, 7, 7, 7)
check(lib.cast_detailed(I32_DOOR, 0, None, None, None, None, 0, b"1", 1, ctypes.byref(ctypes.c_int32()),
                        ctypes.byref(fault)) == CONTRACT_VIOLATION, "cast_detailed fault too small")


def detailed_typed(door, param, value, out):
    called.add("cast_detailed_from_f64")
    fault = fault_detail()
    status = lib.cast_detailed_from_f64(door, param, value, ctypes.byref(out), ctypes.byref(fault))
    return status, out, (fault.offset, fault.len, fault.detail)


status, dec, _ = detailed_typed(DECIMAL_DOOR, 0, 0.1, Decimal())
check(status == OK and (dec.lo, dec.scale) == (1, 1), "cast_detailed_from_f64 decimal")
status, _, fault = detailed_typed(DECIMAL_DOOR, 0, 1e300, Decimal())
check(status == OUT_OF_RANGE and fault == (0, 0, 25), "cast_detailed_from_f64 too_large, spanless")
status, wall, _ = detailed_typed(EXCEL_SERIAL, 1, 45292.75, CivilDateTime())
check(status == OK and wall.date.year == 2024 and wall.nanos_of_day == 64800000000000,
      "cast_detailed_from_f64 excel serial")
check(detailed_typed(EXCEL_SERIAL, 3, 45292.75, CivilDateTime())[0] == CONTRACT_VIOLATION,
      "cast_detailed_from_f64 epoch")
check(detailed_typed(I32_DOOR, 0, 1.0, ctypes.c_int32())[0] == CONTRACT_VIOLATION, "cast_detailed_from_f64 i32 door")

called.add("cast_checked_detailed")
fault, report, out = fault_detail(), Check(7, 7, 7), ctypes.c_double()
status = lib.cast_checked_detailed(F64, 0, None, ctypes.byref(latitude), None, 0, b"91", 2, ctypes.byref(out),
                                   ctypes.byref(fault), ctypes.byref(report))
check(status == VIOLATION and (fault.offset, fault.len, fault.detail) == (0, 2, 0)
      and (report.keyword, report.adjusted, report.enum_index) == (3, 0, NO_INDEX), "cast_checked_detailed violation")
cap = constraints(4, upper=decimal(1000))
status = lib.cast_checked_detailed(I32, 0, ctypes.byref(strict), ctypes.byref(cap), None, 0, b"12,34", 5,
                                   ctypes.byref(ctypes.c_int32()), ctypes.byref(fault), ctypes.byref(report))
check(status == MALFORMED and fault.detail == 11, "cast_checked_detailed strict grouping")
check(lib.cast_checked_detailed(F64, 1, None, ctypes.byref(latitude), None, 0, b"1", 1, ctypes.byref(out),
                                ctypes.byref(fault), None) == CONTRACT_VIOLATION, "cast_checked_detailed stray param")

called.add("cast_checked_detailed_from_f64")
status = lib.cast_checked_detailed_from_f64(F64, 0, ctypes.byref(latitude), None, 0, -90.5, ctypes.byref(out),
                                            ctypes.byref(fault), ctypes.byref(report))
check(status == VIOLATION and report.keyword == 1 and (fault.len, fault.detail) == (0, 0),
      "cast_checked_detailed_from_f64 violation, spanless")
status = lib.cast_checked_detailed_from_f64(F64, 0, ctypes.byref(latitude), None, 0, float("nan"), ctypes.byref(out),
                                            ctypes.byref(fault), ctypes.byref(report))
check(status == MALFORMED and fault.detail == 22, "cast_checked_detailed_from_f64 NaN")
check(lib.cast_checked_detailed_from_f64(F64, 2, ctypes.byref(latitude), None, 0, 1.0, ctypes.byref(out),
                                         ctypes.byref(fault), None) == CONTRACT_VIOLATION,
      "cast_checked_detailed_from_f64 stray param")

check(len(called) == 35, f"called {len(called)} cast exports, expected 35")
print(f"cdylib smoke: {lib_path} {got}, all 36 exports called")
