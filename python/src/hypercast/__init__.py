"""Allocation-free scalar casts — booleans, numerics, UUIDs, temporals — the native
``libhypercast`` Rust core linked straight into a CPython extension module
(``hypercast._native``, PyO3). No dlopen, no ctypes marshalling, no runtime bridge: a door
is an ordinary ``METH_FASTCALL`` extension call into a direct Rust call. Every door
returns a verdict: :class:`Success` or :class:`Fault` (a closed reason plus the offending
byte span), never an exception for bad data — the only exceptions here are caller bugs (a
malformed :class:`NumFormat`), never data.

Consume with ``match``/``case`` over the two case types — Python's answer to C#'s native
union and Java's sealed interface. Exhaustiveness is the type checker's job here, not the
interpreter's (pair the two cases with ``typing.assert_never`` under mypy/pyright for the
compile-time guarantee the static bindings get natively)::

    match hypercast.cast_i32("(1,234)", hypercast.NumFormat.INVARIANT):
        case hypercast.Success(value):
            print("got", value)                       # -1234, accounting negative
        case hypercast.Fault(reason, offset, length):
            print(reason.name, "at byte", offset)

Door names mirror the native ABI (``cast_i32``, ``cast_f64``, ``cast_timestamp``, …) so the
polyglot surface reads identically across bindings. Inputs are ``str`` (read as UTF-8) or
``bytes``, and a :class:`Fault`'s span comes back in the caller's own units — byte offsets
for ``bytes``, code-point offsets for ``str`` — so slicing the offending text back out of
what you passed needs no mapping either way. Python ``int`` is unbounded, so ``cast_u64`` returns the true unsigned value with no bit-pattern
games; ``cast_decimal`` returns an exact, canonical ``decimal.Decimal``;
``datetime``'s resolution is microseconds, so the core's nanoseconds truncate by three
digits on the temporal doors (the JVM binding is the fidelity king; this is Python's honest
ceiling).

Ships as real platform-specific abi3 wheels (linux glibc and musl, macOS, Windows; x64 and
arm64, plus Pyodide's ``pyemscripten`` wasm32 for the browser) built by ``maturin`` — no
compiler needed to install. The package is typed:
``py.typed`` ships beside it, with a stub for the extension module, so a checker sees each
door's own ``Success[...] | Fault`` rather than ``Any``.
"""

from __future__ import annotations

from enum import IntEnum
from typing import TypeVar, Union

from . import _native

#: Which backend this process loaded: always ``"native"``, the PyO3 extension that links the
#: Rust core straight into CPython — the only backend, and the one every published wheel ships.
BACKEND: str = "native"

__all__ = [
    "BACKEND",
    "CastFailure", "Success", "Fault", "Verdict", "NumFormat", "UnixPrecision", "DateOrder",
    "ExcelEpoch",
    "optional",
    "cast_bool", "cast_i8", "cast_i16", "cast_i32", "cast_i64",
    "cast_u8", "cast_u16", "cast_u32", "cast_u64", "cast_f32", "cast_f64", "cast_decimal",
    "cast_uuid", "cast_timestamp", "cast_unix", "cast_excel_serial", "cast_date",
    "cast_datetime", "cast_time",
    "cast_duration",
    "native_version",
]


class CastFailure(IntEnum):
    """The closed set of reasons a cast can fail — the native core's verdict codes, verbatim."""

    EMPTY = 1
    """Required input was empty or whitespace. :func:`optional` surfaces this as ``None``."""
    MALFORMED = 2
    """Input was present but not recognizable as the target type."""
    OUT_OF_RANGE = 3
    """Well-formed but outside the target's range — ``"256"`` for a u8, ``1e400`` for an f64."""


class UnixPrecision(IntEnum):
    """The declared unit of a Unix-epoch value — no magnitude guessing, ever."""

    SECONDS = 1
    MILLISECONDS = 2
    MICROSECONDS = 3
    NANOSECONDS = 4


class ExcelEpoch(IntEnum):
    """The date system an Excel serial number is expressed in. Spreadsheets carry no marker
    for this — it is a workbook-level setting — so the caller states it, the same way
    :class:`UnixPrecision` and :class:`DateOrder` are declared rather than guessed.
    """

    Y1900 = 1
    """The Windows default: serial ``1`` is 1900-01-01, and serial ``60`` is a February 29th
    that never existed."""
    Y1904 = 2
    """The legacy Macintosh system, still selectable today: serial ``0`` is 1904-01-01, with
    no phantom day anywhere in it."""


class DateOrder(IntEnum):
    """The declared field order of a separated calendar date — no guessing, ever:
    ``"1/7/2026"`` is January 7th (:data:`MONTH_DAY_YEAR`, the en-US order) or July 1st
    (:data:`DAY_MONTH_YEAR`, the en-GB order) only because the caller said which. Passed
    as :func:`cast_date`'s optional second argument; without it, the door stays strict
    ISO ``yyyy-MM-dd`` only.
    """

    YEAR_MONTH_DAY = 1
    MONTH_DAY_YEAR = 2
    DAY_MONTH_YEAR = 3


# The extension's own types and doors ARE the package surface — no delegation defs, no
# per-call Python frame on top (their docstrings live on the PyO3 functions/classes
# themselves). _bind hands over the CastFailure members so faults carry the exact enum
# members callers compare with `is`, plus uuid.UUID for cast_uuid's construction.
_native._bind(CastFailure)

Success = _native.Success
Fault = _native.Fault
NumFormat = _native.NumFormat

cast_bool = _native.cast_bool
cast_i8 = _native.cast_i8
cast_i16 = _native.cast_i16
cast_i32 = _native.cast_i32
cast_i64 = _native.cast_i64
cast_u8 = _native.cast_u8
cast_u16 = _native.cast_u16
cast_u32 = _native.cast_u32
cast_u64 = _native.cast_u64
cast_f32 = _native.cast_f32
cast_f64 = _native.cast_f64
cast_decimal = _native.cast_decimal
cast_uuid = _native.cast_uuid
cast_timestamp = _native.cast_timestamp
cast_unix = _native.cast_unix
cast_excel_serial = _native.cast_excel_serial
cast_date = _native.cast_date
cast_datetime = _native.cast_datetime
cast_time = _native.cast_time
cast_duration = _native.cast_duration
native_version = _native.native_version

_T = TypeVar("_T")

Verdict = Union[Success[_T], Fault]
"""The outcome of a cast: exactly one of :class:`Success` or :class:`Fault`. Generic in the
value a success carries — ``Verdict[int]`` is what :func:`cast_i32` returns."""


def optional(verdict: Verdict[_T]) -> Verdict[_T] | None:
    """Presents a verdict optionally: a :data:`CastFailure.EMPTY` fault becomes ``None``
    (Python's absent), everything else flows through untouched.
    """
    if isinstance(verdict, Fault) and verdict.reason is CastFailure.EMPTY:
        return None
    return verdict
