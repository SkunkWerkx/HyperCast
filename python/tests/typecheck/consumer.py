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
    Checked,
    CheckedVerdict,
    Constraints,
    DateOptions,
    DateOrder,
    Detail,
    Door,
    ExcelEpoch,
    Fault,
    Keyword,
    MonthNames,
    NumFormat,
    Rounding,
    Success,
    UnixPrecision,
    Verdict,
    Violation,
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


def diagnose(verdict: Verdict[int]) -> str:
    """Reads a failure's detail: a keyword pattern beside the positional one, an `UNKNOWN` arm
    for a code this version names no member for, and the raw number beside it."""
    match verdict:
        case Success():
            return "ok"
        case Fault(detail=Detail.GROUP_SIZE):
            return "a group of the wrong size"
        case Fault(detail=Detail.UNKNOWN) as unnamed:
            assert_type(unnamed.detail_code, int | None)
            return f"detail {unnamed.detail_code}"
        case Fault() as other:
            assert_type(other.detail, Detail | None)
            return "something else"
        case _:
            assert_never(verdict)


def judge(verdict: CheckedVerdict[float]) -> str:
    """Presents a checked verdict by matching its three cases, exhaustively, and a keyword
    with the default arm keywords need."""
    match verdict:
        case Checked(value, adjusted, enum_index):
            assert_type(value, float)
            assert_type(adjusted, bool)
            assert_type(enum_index, int | None)
            return f"got {value}"
        case Violation(keyword, value, offset, length):
            assert_type(keyword, Keyword)
            assert_type(value, float)
            assert_type(offset, int)
            match keyword:
                case Keyword.MINIMUM | Keyword.MAXIMUM:
                    return f"{value} is out of bounds"
                case _:
                    return f"breaks {keyword.name} at {offset}+{length}"
        case Fault(reason, offset, length):
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
assert_type(
    NumFormat(
        ",",
        "\u00a0",
        NumFormat.ALL | NumFormat.TRAILING_SIGN,
        "€",
        currency_decimal_sep=".",
        currency_group_sep=",",
        minus_sign="\u2212",
        plus_sign="+",
        percent="٪",
        digit_zero="٠",
    ),
    NumFormat,
)
assert_type(NumFormat.from_localeconv(), NumFormat)
assert_type(NumFormat.ALL | NumFormat.SEPARATOR_DETECT, int)
assert_type(fmt.decimal_sep, str)
assert_type(fmt.flags, int)
assert_type(fmt.minus_sign, str)
assert_type(fmt.digit_zero, str)
assert_type(fmt.packed, bytes)

assert_type(hypercast.cast_bool("yes"), Success[bool] | Fault)
assert_type(hypercast.cast_char("U+00E9"), Success[str] | Fault)
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
assert_type(
    hypercast.cast_datetime("…", DateOrder.YEAR_MONTH_DAY, am="오전", pm="오후"),
    Success[datetime.datetime] | Fault,
)
assert_type(hypercast.cast_time("…"), Success[datetime.time] | Fault)
assert_type(hypercast.cast_duration("…"), Success[datetime.timedelta] | Fault)

assert_type(hypercast.optional(hypercast.cast_i32("", fmt)), Success[int] | Fault | None)
assert_type(describe(hypercast.cast_i32("42", fmt)), str)
assert_type(hypercast.native_version(), str)

latitude = Constraints(
    Door.F64, minimum="-90", maximum=90, multiple_of="0.000001", rounding=Rounding.MIDPOINT
)
assert_type(latitude, Constraints[float])
assert_type(Constraints(Door.I32, enum=[1, "2", decimal.Decimal(3)]), Constraints[int])
assert_type(
    Constraints(Door.DECIMAL, multiple_of=decimal.Decimal("0.05")), Constraints[decimal.Decimal]
)
sex = Constraints(Door.TEXT, trim=True, enum=["Female", "Male", "Other"])
assert_type(sex, Constraints[str])
assert_type(latitude.minimum, decimal.Decimal | None)
assert_type(latitude.rounding, Rounding)
assert_type(latitude.packed, bytes)
assert_type(hypercast.cast_checked("1", fmt, latitude), Checked[float] | Fault | Violation[float])
assert_type(
    hypercast.cast_checked_from_float(1.0, latitude), Checked[float] | Fault | Violation[float]
)
assert_type(hypercast.cast_text("x"), Success[str] | Fault)
assert_type(hypercast.cast_text("x", sex), Checked[str] | Fault | Violation[str])
assert_type(Constraints(Door.TEXT, case_sensitive=True, enum=["Male"]).case_sensitive, bool)
assert_type(hypercast.cast_constraints_check(Door.U8, minimum="-1"), int)
assert_type(hypercast.cast_declared_decimal("1e-6"), decimal.Decimal)
assert_type(
    hypercast.optional(hypercast.cast_text(" ", sex)), Checked[str] | Fault | Violation[str] | None
)
assert_type(judge(hypercast.cast_checked("42", fmt, latitude)), str)
assert_type(Keyword.ENUM.name, str)

assert_type(NumFormat.GROUPING_WESTERN, int)
assert_type(NumFormat(".", ",", NumFormat.ALL, grouping=NumFormat.GROUPING_INDIAN), NumFormat)
assert_type(fmt.grouping, int)
assert_type(diagnose(hypercast.cast_i32("12,34", fmt)), str)
assert_type(Detail.from_code(11), Detail)
options = DateOptions(DateOptions.COMPACT_DATE, pivot=2029)
assert_type(options, DateOptions)
assert_type(DateOptions.COMMON_FORMS.with_pivot(2029).pivot, int)
months = MonthNames([(1, "Jan"), (2, "Feb")])
assert_type(months.entries, tuple[tuple[int, str], ...])
assert_type(
    hypercast.cast_date("07-Jan-26", DateOrder.DAY_MONTH_YEAR, options=options, months=months),
    Success[datetime.date] | Fault,
)
assert_type(
    hypercast.cast_datetime("…", DateOrder.YEAR_MONTH_DAY, options=options, months=months),
    Success[datetime.datetime] | Fault,
)
assert_type(
    hypercast.cast_timestamp("…", options=DateOptions.COMMON_FORMS),
    Success[datetime.datetime] | Fault,
)
