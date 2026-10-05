"""What a consumer writes, type-checked by ``tests/test_typing.py`` (never imported or run).

Two things are held here. ``assert_type`` fails the check when the checker sees ``Any`` —
which is what every door was before the package shipped ``py.typed`` and a stub for the
extension module. And ``assert_never`` in the fall-through arm is the exhaustiveness the
README promises: it type-checks only because the two ``case`` arms above it leave nothing of
the verdict over.
"""

import datetime
import decimal
import uuid
from typing import assert_never, assert_type

import hypercast
from hypercast import (
    CastFailure,
    DateOrder,
    ExcelEpoch,
    Fault,
    NumFormat,
    Success,
    UnixPrecision,
    Verdict,
)


def describe(verdict: Verdict[int]) -> str:
    """Presents a verdict by matching its two cases, exhaustively."""
    match verdict:
        case Success(value):
            assert_type(value, int)
            return f"got {value}"
        case Fault(reason, offset, length):
            assert_type(reason, CastFailure)
            assert_type(offset, int)
            assert_type(length, int)
            return f"{reason.name} at {offset}+{length}"
        case _:
            assert_never(verdict)


def by_isinstance(verdict: Success[float] | Fault) -> float | None:
    """Narrows a verdict with isinstance instead of match."""
    if isinstance(verdict, Success):
        return verdict.value
    assert_type(verdict, Fault)
    return None


fmt = NumFormat.INVARIANT
assert_type(fmt, NumFormat)
assert_type(NumFormat(",", ".", NumFormat.ALL, currency="kr."), NumFormat)
assert_type(NumFormat.from_localeconv(), NumFormat)
assert_type(NumFormat.ALL | NumFormat.SEPARATOR_DETECT, int)
assert_type(fmt.decimal_sep, str)
assert_type(fmt.flags, int)

assert_type(hypercast.cast_bool("yes"), Success[bool] | Fault)
assert_type(hypercast.cast_i8(b"1", fmt), Success[int] | Fault)
assert_type(hypercast.cast_i32("1", fmt), Success[int] | Fault)
assert_type(hypercast.cast_u64("1", fmt), Success[int] | Fault)
assert_type(hypercast.cast_f32("1", fmt), Success[float] | Fault)
assert_type(hypercast.cast_f64("1", fmt), Success[float] | Fault)
assert_type(hypercast.cast_decimal("1", fmt), Success[decimal.Decimal] | Fault)
assert_type(hypercast.cast_uuid("…"), Success[uuid.UUID] | Fault)
assert_type(hypercast.cast_timestamp("…"), Success[datetime.datetime] | Fault)
assert_type(hypercast.cast_unix("1", UnixPrecision.SECONDS), Success[datetime.datetime] | Fault)
assert_type(hypercast.cast_excel_serial("1", ExcelEpoch.Y1900), Success[datetime.datetime] | Fault)
assert_type(hypercast.cast_decimal_from_float(0.1), Success[decimal.Decimal] | Fault)
assert_type(
    hypercast.cast_excel_serial_from_float(1.0, ExcelEpoch.Y1900),
    Success[datetime.datetime] | Fault,
)
assert_type(hypercast.cast_excel_time(0.5), Success[datetime.time] | Fault)
assert_type(hypercast.cast_excel_duration(1.5), Success[datetime.timedelta] | Fault)
assert_type(hypercast.cast_date("…"), Success[datetime.date] | Fault)
assert_type(hypercast.cast_date("…", DateOrder.DAY_MONTH_YEAR), Success[datetime.date] | Fault)
assert_type(
    hypercast.cast_datetime("…", DateOrder.MONTH_DAY_YEAR), Success[datetime.datetime] | Fault
)
assert_type(hypercast.cast_time("…"), Success[datetime.time] | Fault)
assert_type(hypercast.cast_duration("…"), Success[datetime.timedelta] | Fault)

assert_type(hypercast.optional(hypercast.cast_i32("", fmt)), Success[int] | Fault | None)
assert_type(describe(hypercast.cast_i32("42", fmt)), str)
assert_type(hypercast.native_version(), str)
assert_type(hypercast.BACKEND, str)
