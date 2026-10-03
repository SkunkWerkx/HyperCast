"""Loads the shared library every FFI binding loads and calls all twenty-two C exports.

ci.yml's check-cdylib job runs this against a freshly built no_std library on every platform
the forge ships: a library that links but will not load, or loads but misbehaves at the ABI,
fails here before any binding sees it. ctypes because Python is on every runner and opens
the library the way a host does (dlopen, LoadLibrary), with nothing compiled in between.

Every export is called at least twice: once with input it accepts, the value checked, and
once with input it rejects, the reason (and the fault span where one is set) checked. The
out-structs mirror rust/src/verdict.rs's #[repr(C)] layouts and RawNumFormat mirrors
rust/src/ffi.rs's, as the Go and Swift bindings do; a layout drift reads back wrong values.

usage: python cdylib_smoke.py <path-to-library> <rust/Cargo.toml>
"""

import ctypes
import re
import sys
import uuid
from pathlib import Path

lib_path, manifest = sys.argv[1], sys.argv[2]
want = re.search(r'^version = "([^"]+)"', Path(manifest).read_text(encoding="utf-8"), re.MULTILINE)[1]

OK, EMPTY, MALFORMED, OUT_OF_RANGE, CONTRACT_VIOLATION = 0, 1, 2, 3, -1


class Fault(ctypes.Structure):
    _fields_ = [("offset", ctypes.c_uint32), ("len", ctypes.c_uint32)]


class NumFormat(ctypes.Structure):
    _fields_ = [("decimal_sep", ctypes.c_uint32), ("group_sep", ctypes.c_uint32),
                ("flags", ctypes.c_uint32), ("currency_len", ctypes.c_uint32),
                ("currency", ctypes.c_uint8 * 16)]


class Decimal(ctypes.Structure):
    _fields_ = [("lo", ctypes.c_uint64), ("hi", ctypes.c_uint32),
                ("scale", ctypes.c_uint8), ("negative", ctypes.c_bool)]


class Timestamp(ctypes.Structure):
    _fields_ = [("seconds", ctypes.c_int64), ("nanos", ctypes.c_int32)]


class Duration(ctypes.Structure):
    _fields_ = [("seconds", ctypes.c_int64), ("nanos", ctypes.c_int32)]


class Date(ctypes.Structure):
    _fields_ = [("year", ctypes.c_uint16), ("month", ctypes.c_uint8), ("day", ctypes.c_uint8)]


class CivilDateTime(ctypes.Structure):
    _fields_ = [("date", Date), ("nanos_of_day", ctypes.c_uint64)]


GROUPING, PARENS, EXPONENT, RADIX_PREFIX, PERCENT, _, CURRENCY = (1 << i for i in range(7))
ALL = GROUPING | PARENS | EXPONENT | RADIX_PREFIX | PERCENT | CURRENCY


def num_format(decimal_sep, group_sep, flags, currency=""):
    raw = currency.encode()
    fmt = NumFormat(ord(decimal_sep), ord(group_sep), flags, len(raw))
    fmt.currency[:len(raw)] = list(raw)
    return fmt


lib = ctypes.CDLL(lib_path)
txt, size, u32, i32, P = ctypes.c_char_p, ctypes.c_size_t, ctypes.c_uint32, ctypes.c_int32, ctypes.POINTER
FaultP, FormatP = P(Fault), P(NumFormat)

lib.hypercast_version.restype, lib.hypercast_version.argtypes = u32, []
PLAIN = {"cast_bool": ctypes.c_uint8, "cast_uuid": ctypes.c_uint8 * 16, "cast_timestamp": Timestamp,
         "cast_date": Date, "cast_time": ctypes.c_uint64, "cast_duration": Duration}
NUMERIC = {"cast_i8": ctypes.c_int8, "cast_i16": ctypes.c_int16, "cast_i32": ctypes.c_int32,
           "cast_i64": ctypes.c_int64, "cast_u8": ctypes.c_uint8, "cast_u16": ctypes.c_uint16,
           "cast_u32": ctypes.c_uint32, "cast_u64": ctypes.c_uint64, "cast_f32": ctypes.c_float,
           "cast_f64": ctypes.c_double, "cast_decimal": Decimal}
SELECTOR = {"cast_unix": Timestamp, "cast_excel_serial": Timestamp, "cast_date_ordered": Date,
            "cast_datetime": CivilDateTime}
for name, out in PLAIN.items():
    fn = getattr(lib, name)
    fn.restype, fn.argtypes = i32, [txt, size, P(out), FaultP]
for name, out in NUMERIC.items():
    fn = getattr(lib, name)
    fn.restype, fn.argtypes = i32, [txt, size, FormatP, P(out), FaultP]
for name, out in SELECTOR.items():
    fn = getattr(lib, name)
    fn.restype, fn.argtypes = i32, [txt, size, u32, P(out), FaultP]
OUT_TYPES = {**PLAIN, **NUMERIC, **SELECTOR}
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


def fails(name, text, reason, *extra, span=None):
    status, _, fault = call(name, text, *extra)
    check(status == reason, f"{name}({text!r}) returned {status}, expected {reason}")
    if span is not None:
        check((fault.offset, fault.len) == span,
              f"{name}({text!r}) fault {(fault.offset, fault.len)}, expected {span}")


v = lib.hypercast_version()
got = f"{v >> 16}.{(v >> 8) & 0xFF}.{v & 0xFF}"
check(got == want, f"hypercast_version reports {got}, rust/Cargo.toml says {want}")

check(ok("cast_bool", "Yes").value == 1 and ok("cast_bool", "off").value == 0, "cast_bool value")
fails("cast_bool", "maybe", MALFORMED, span=(0, 5))
fails("cast_bool", "  ", EMPTY)

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

dt = ok("cast_datetime", "1/7/2026 3:04 PM", 2)
check((dt.date.year, dt.date.month, dt.date.day, dt.nanos_of_day) == (2026, 1, 7, 54240000000000),
      "cast_datetime value")
fails("cast_datetime", "1/7/2026 25:04", OUT_OF_RANGE, 2, span=(9, 2))
fails("cast_datetime", "1/7/2026 3:04 PM", CONTRACT_VIOLATION, 0)

check(len(called) == 21, f"called {len(called)} cast exports, expected 21: {sorted(OUT_TYPES.keys() - called)}")
print(f"cdylib smoke: {lib_path} {got}, all 22 exports called")
