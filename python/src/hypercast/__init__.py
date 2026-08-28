"""Allocation-free scalar casts — booleans, numerics, UUIDs, temporals — calling directly
into the native ``libhypercast`` shared library via ctypes. Every door returns a verdict:
:class:`Success` or :class:`Fault` (a closed reason plus the offending byte span), never an
exception for bad data — the only exceptions here are caller bugs (a malformed
:class:`NumFormat`), never data.

Consume with ``match``/``case`` over the two case types — Python's answer to C#'s native
union and Java's sealed interface. Exhaustiveness is the type checker's job here, not the
interpreter's (pair ``Verdict`` with ``typing.assert_never`` under mypy/pyright for the
compile-time guarantee the static bindings get natively)::

    match hypercast.cast_i32("(1,234)", hypercast.NumFormat.INVARIANT):
        case hypercast.Success(value):
            print("got", value)                       # -1234, accounting negative
        case hypercast.Fault(reason, offset, length):
            print(reason.name, "at byte", offset)

Door names mirror the native ABI (``cast_i32``, ``cast_f64``, ``cast_timestamp``, …) so the
polyglot surface reads identically across bindings. Inputs are ``str`` (UTF-8-encoded) or
``bytes`` — the native contract, and the form whose fault offsets need no mapping. Python
``int`` is unbounded, so ``cast_u64`` returns the true unsigned value with no bit-pattern
games; ``datetime``'s resolution is microseconds, so the core's nanoseconds truncate by
three digits on the temporal doors (the JVM binding is the fidelity king; this is Python's
honest ceiling).
"""

from __future__ import annotations

import datetime as _dt
import locale as _locale
import uuid as _uuid
from ctypes import byref, c_double, c_float, c_int8, c_int16, c_int32, c_int64, c_ubyte, \
    c_uint8, c_uint16, c_uint32, c_uint64
from dataclasses import dataclass
from enum import IntEnum
from typing import ClassVar, Generic, TypeVar, Union

from . import _runtime

_T = TypeVar("_T")

__all__ = [
    "CastFailure", "Success", "Fault", "Verdict", "NumFormat", "UnixPrecision", "optional",
    "cast_bool", "cast_i8", "cast_i16", "cast_i32", "cast_i64",
    "cast_u8", "cast_u16", "cast_u32", "cast_u64", "cast_f32", "cast_f64",
    "cast_uuid", "cast_timestamp", "cast_unix", "cast_date", "cast_time", "cast_duration",
]


class CastFailure(IntEnum):
    """The closed set of reasons a cast can fail — the native core's verdict codes, verbatim."""

    EMPTY = 1
    """Required input was empty or whitespace. :func:`optional` surfaces this as ``None``."""
    MALFORMED = 2
    """Input was present but not recognizable as the target type."""
    OUT_OF_RANGE = 3
    """Well-formed but outside the target's range — ``"256"`` for a u8, ``1e400`` for an f64."""


@dataclass(frozen=True, slots=True)
class Success(Generic[_T]):
    """The success case of a verdict: a cast value."""

    value: _T


@dataclass(frozen=True, slots=True)
class Fault(Generic[_T]):
    """The failure case: a closed reason plus the offending span, as byte offsets into the
    UTF-8 form of the input — exactly what the ``bytes`` doors received, and identical to
    character offsets for ASCII input. Nothing is captured; slicing the offending text out
    of the input is the caller's choice.
    """

    reason: CastFailure
    offset: int
    length: int


Verdict = Union[Success[_T], Fault[_T]]
"""The outcome of a cast: exactly one of :class:`Success` or :class:`Fault`."""


@dataclass(frozen=True, slots=True)
class NumFormat:
    """Caller-declared numeric notation for the integer and real doors. The core carries no
    culture data — a call site parsing culture-sensitive text declares its format out loud
    (:data:`NumFormat.INVARIANT`, or :meth:`from_localeconv`); there is no default argument,
    the same stance every binding in this repo takes.
    """

    decimal_sep: str
    group_sep: str
    flags: int

    GROUPING = 1
    """Permit the group separator between digits (sizes not validated — between digits is the rule)."""
    PARENTHESES = 1 << 1
    """Permit accounting parentheses as negation: ``(1,234)`` is -1234."""
    EXPONENT = 1 << 2
    """Permit exponent notation. Integer doors reject a negative exponent."""
    RADIX_PREFIXES = 1 << 3
    """Permit ``0x``/``&H``/``0b`` two's-complement radix prefixes (``0xFF`` is -1 for an i8)."""
    PERCENT = 1 << 4
    """Permit a trailing ``%``, dividing by 100. Real doors only."""
    ALL = GROUPING | PARENTHESES | EXPONENT | RADIX_PREFIXES | PERCENT

    INVARIANT: ClassVar["NumFormat"]  # assigned after the class body

    def __post_init__(self) -> None:
        if len(self.decimal_sep) != 1 or len(self.group_sep) != 1:
            raise ValueError("Separators must be single characters")
        if self.decimal_sep == self.group_sep:
            raise ValueError(f"Decimal and group separators must differ; both are {self.decimal_sep!r}")

    @classmethod
    def from_localeconv(cls, conv: dict | None = None) -> "NumFormat":
        """Derives a format from :func:`locale.localeconv` (or a compatible mapping), with
        every lenience on. Python's locale is process-global state — prefer declaring
        explicitly when the text's origin is known.
        """
        conv = conv if conv is not None else _locale.localeconv()
        decimal = conv.get("decimal_point") or "."
        group = conv.get("thousands_sep") or ","
        return cls(decimal[0], group[0], cls.ALL)


# Class attribute assignment — frozen dataclasses only guard instances.
NumFormat.INVARIANT = NumFormat(".", ",", NumFormat.ALL)


class UnixPrecision(IntEnum):
    """The declared unit of a Unix-epoch value — no magnitude guessing, ever."""

    SECONDS = 1
    MILLISECONDS = 2
    MICROSECONDS = 3
    NANOSECONDS = 4


def optional(verdict: Verdict[_T]) -> Verdict[_T] | None:
    """Presents a verdict optionally: an :data:`CastFailure.EMPTY` fault becomes ``None``
    (Python's absent), everything else flows through untouched."""
    if isinstance(verdict, Fault) and verdict.reason is CastFailure.EMPTY:
        return None
    return verdict


def _utf8(text: str | bytes) -> bytes:
    return text.encode("utf-8") if isinstance(text, str) else text


def _failed(code: int, fault: _runtime.RawFault) -> Fault:
    if code == -1:
        raise RuntimeError("libhypercast reported a contract violation — a binding bug, please report it")
    return Fault(CastFailure(code), fault.offset, fault.length)


def _raw_format(fmt: NumFormat) -> _runtime.RawNumFormat:
    return _runtime.RawNumFormat(ord(fmt.decimal_sep), ord(fmt.group_sep), fmt.flags)


def _plain(symbol: str, text: str | bytes, out) -> tuple[int, _runtime.RawFault]:
    data = _utf8(text)
    fault = _runtime.RawFault()
    code = getattr(_runtime.get_lib(), symbol)(data or None, len(data), byref(out), byref(fault))
    return code, fault


def _numeric(symbol: str, text: str | bytes, fmt: NumFormat, out) -> tuple[int, _runtime.RawFault]:
    data = _utf8(text)
    fault = _runtime.RawFault()
    code = getattr(_runtime.get_lib(), symbol)(
        data or None, len(data), byref(_raw_format(fmt)), byref(out), byref(fault))
    return code, fault


def cast_bool(text: str | bytes) -> Verdict[bool]:
    """Casts boolean text: ``true``/``false`` plus the conventions untrusted sources actually
    send (``t/f``, ``yes/no``, ``y/n``, ``1/0``, ``on/off``, ``enabled/disabled``,
    ``active/inactive``, ``checked/unchecked``, ``in/out``), ASCII case-insensitive."""
    out = c_ubyte()
    code, fault = _plain("cast_bool", text, out)
    return Success(bool(out.value)) if code == 0 else _failed(code, fault)


def _int_door(symbol: str, ctype):
    def door(text: str | bytes, fmt: NumFormat) -> Verdict[int]:
        out = ctype()
        code, fault = _numeric(symbol, text, fmt, out)
        return Success(out.value) if code == 0 else _failed(code, fault)

    door.__name__ = symbol
    door.__qualname__ = symbol
    door.__doc__ = (
        f"Casts integer text under the declared format — the target type's own range, "
        f"declared grouping, accounting parentheses, non-negative exponent, and "
        f"0x/&H/0b two's-complement radix prefixes. See the {symbol.removeprefix('cast_')} "
        f"door's semantics in the core."
    )
    return door


cast_i8 = _int_door("cast_i8", c_int8)
cast_i16 = _int_door("cast_i16", c_int16)
cast_i32 = _int_door("cast_i32", c_int32)
cast_i64 = _int_door("cast_i64", c_int64)
cast_u8 = _int_door("cast_u8", c_uint8)
cast_u16 = _int_door("cast_u16", c_uint16)
cast_u32 = _int_door("cast_u32", c_uint32)
cast_u64 = _int_door("cast_u64", c_uint64)


def cast_f32(text: str | bytes, fmt: NumFormat) -> Verdict[float]:
    """Casts real text to an IEEE single (widened losslessly to Python's float): finite
    values only, declared separators, parentheses, exponent, and trailing percent."""
    out = c_float()
    code, fault = _numeric("cast_f32", text, fmt, out)
    return Success(out.value) if code == 0 else _failed(code, fault)


def cast_f64(text: str | bytes, fmt: NumFormat) -> Verdict[float]:
    """Casts real text to an IEEE double. Notation rules as :func:`cast_f32`."""
    out = c_double()
    code, fault = _numeric("cast_f64", text, fmt, out)
    return Success(out.value) if code == 0 else _failed(code, fault)


def cast_uuid(text: str | bytes) -> Verdict[_uuid.UUID]:
    """Casts UUID text — all five .NET ``Guid`` formats (D/N/B/P/X) plus
    ``urn:uuid:``/``GUID:``/``UUID:`` prefixes — to a :class:`uuid.UUID`."""
    out = (c_ubyte * 16)()
    code, fault = _plain("cast_uuid", text, out)
    return Success(_uuid.UUID(bytes=bytes(out))) if code == 0 else _failed(code, fault)


_EPOCH = _dt.datetime(1970, 1, 1, tzinfo=_dt.timezone.utc)


def _instant(raw: _runtime.RawTimestamp) -> _dt.datetime:
    # timedelta arithmetic covers the full 0001..9999 window; datetime.fromtimestamp
    # delegates to the OS on some platforms and doesn't. Nanos truncate to microseconds —
    # datetime's honest ceiling.
    return _EPOCH + _dt.timedelta(seconds=raw.seconds, microseconds=raw.nanos // 1000)


def cast_timestamp(text: str | bytes) -> Verdict[_dt.datetime]:
    """Casts an RFC 3339 instant — zone **mandatory** — to an aware UTC
    :class:`datetime.datetime`. Sub-microsecond nanoseconds truncate."""
    out = _runtime.RawTimestamp()
    code, fault = _plain("cast_timestamp", text, out)
    return Success(_instant(out)) if code == 0 else _failed(code, fault)


def cast_unix(text: str | bytes, precision: UnixPrecision) -> Verdict[_dt.datetime]:
    """Casts an integer Unix-epoch value under a caller-declared unit to an aware UTC
    :class:`datetime.datetime`."""
    if not isinstance(precision, UnixPrecision):
        raise TypeError("precision must be a UnixPrecision")
    data = _utf8(text)
    out = _runtime.RawTimestamp()
    fault = _runtime.RawFault()
    code = _runtime.get_lib().cast_unix(
        data or None, len(data), int(precision), byref(out), byref(fault))
    return Success(_instant(out)) if code == 0 else _failed(code, fault)


def cast_date(text: str | bytes) -> Verdict[_dt.date]:
    """Casts a strict ISO 8601 ``yyyy-MM-dd`` calendar date to a :class:`datetime.date`."""
    out = _runtime.RawDate()
    code, fault = _plain("cast_date", text, out)
    return Success(_dt.date(out.year, out.month, out.day)) if code == 0 else _failed(code, fault)


def cast_time(text: str | bytes) -> Verdict[_dt.time]:
    """Casts an ISO 24-hour time-of-day to a :class:`datetime.time`. Sub-microsecond
    nanoseconds truncate."""
    out = c_uint64()
    code, fault = _plain("cast_time", text, out)
    if code != 0:
        return _failed(code, fault)
    nanos = out.value
    second_of_day, nano_of_second = divmod(nanos, 1_000_000_000)
    hour, rest = divmod(second_of_day, 3600)
    minute, second = divmod(rest, 60)
    return Success(_dt.time(int(hour), int(minute), int(second), int(nano_of_second // 1000)))


def cast_duration(text: str | bytes) -> Verdict[_dt.timedelta]:
    """Casts a duration (ISO 8601 fixed components, invariant colon form, or protobuf JSON
    seconds) to a :class:`datetime.timedelta`. Sub-microsecond nanoseconds truncate."""
    out = _runtime.RawDuration()
    code, fault = _plain("cast_duration", text, out)
    if code != 0:
        return _failed(code, fault)
    # Same-signed seconds/nanos normalize correctly through timedelta's own arithmetic.
    # Truncate toward zero (// floors toward -inf) so sub-microsecond digits drop the same
    # way on both signs, matching the other bindings' truncation.
    nanos = out.nanos
    micros = nanos // 1000 if nanos >= 0 else -((-nanos) // 1000)
    return Success(_dt.timedelta(seconds=out.seconds, microseconds=micros))
