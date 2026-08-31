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
``bytes`` — the native contract, and the form whose fault offsets need no mapping. Python
``int`` is unbounded, so ``cast_u64`` returns the true unsigned value with no bit-pattern
games; ``datetime``'s resolution is microseconds, so the core's nanoseconds truncate by
three digits on the temporal doors (the JVM binding is the fidelity king; this is Python's
honest ceiling).

Ships as real platform-specific abi3 wheels (linux/macOS/Windows, x64/arm64) built by
``maturin`` — no compiler needed to install.
"""

from __future__ import annotations

from enum import IntEnum
from typing import Union

from . import _native

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


class UnixPrecision(IntEnum):
    """The declared unit of a Unix-epoch value — no magnitude guessing, ever."""

    SECONDS = 1
    MILLISECONDS = 2
    MICROSECONDS = 3
    NANOSECONDS = 4


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
cast_uuid = _native.cast_uuid
cast_timestamp = _native.cast_timestamp
cast_unix = _native.cast_unix
cast_date = _native.cast_date
cast_time = _native.cast_time
cast_duration = _native.cast_duration

Verdict = Union[Success, Fault]
"""The outcome of a cast: exactly one of :class:`Success` or :class:`Fault`."""


def optional(verdict):
    """Presents a verdict optionally: a :data:`CastFailure.EMPTY` fault becomes ``None``
    (Python's absent), everything else flows through untouched.
    """
    if isinstance(verdict, Fault) and verdict.reason is CastFailure.EMPTY:
        return None
    return verdict
