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

Ships as real platform-specific abi3 wheels (linux/macOS/Windows, x64/arm64) built by
``maturin`` — no compiler needed to install. The same core also rides inside every wheel
as a ``wasm32-wasip1`` module, which ``wasmtime-py`` can run in-process for an interpreter
no wheel matches (``pip install hypercast[wasm]``, ``HYPERCAST_WASM=1``); see ``_wasm``.
"""

from __future__ import annotations

import os as _os
from enum import IntEnum
from typing import Union

# --- backend selection -----------------------------------------------------------------
# `_native` is the PyO3 extension: the Rust core linked straight into CPython, the backend
# every published wheel ships. `_wasm` is the same core compiled to wasm32-wasip1 and run
# inside this process by wasmtime-py (see `_wasm.py` for how the crossing works and what it
# costs). HYPERCAST_WASM=1 forces the wasm backend; otherwise it is the fallback for an
# interpreter no wheel matches, taken only when `wasmtime` is importable — a plain
# `pip install hypercast[wasm]` on an unsupported platform is the whole opt-in.
if _os.environ.get("HYPERCAST_WASM"):
    from . import _wasm as _native

    BACKEND = "wasm"
else:
    try:
        from . import _native

        BACKEND = "native"
    except ImportError as _native_error:
        try:
            import wasmtime as _wasmtime  # noqa: F401
        except ImportError:
            raise ImportError(
                f"{_native_error}. No hypercast._native extension for this interpreter; the "
                "wasm backend can stand in if wasmtime is installed: pip install hypercast[wasm]"
            ) from _native_error
        from . import _wasm as _native

        BACKEND = "wasm"

#: Which backend this process loaded: ``"native"`` (the PyO3 extension) or ``"wasm"`` (the
#: same core as a wasm32-wasip1 module under wasmtime-py). Informational — every door in
#: this module behaves identically on both; the test suite runs against each.
BACKEND: str

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


# The backend's own types and doors ARE the package surface — no delegation defs, no
# per-call Python frame on top (their docstrings live on the PyO3 functions/classes
# themselves, and on `_wasm`'s twins). _bind hands over the CastFailure members so faults
# carry the exact enum members callers compare with `is`, plus uuid.UUID for cast_uuid's
# construction; on the wasm backend it also instantiates the module, so a missing engine or
# module fails here, at import, where the extension's own import failure would.
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

Verdict = Union[Success, Fault]
"""The outcome of a cast: exactly one of :class:`Success` or :class:`Fault`."""


def optional(verdict):
    """Presents a verdict optionally: a :data:`CastFailure.EMPTY` fault becomes ``None``
    (Python's absent), everything else flows through untouched.
    """
    if isinstance(verdict, Fault) and verdict.reason is CastFailure.EMPTY:
        return None
    return verdict
