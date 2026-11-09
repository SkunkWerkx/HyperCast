"""Allocation-free scalar casts — booleans, numerics, UUIDs, temporals — the native
``libhypercast`` Rust core linked straight into a CPython extension module
(``hypercast._native``, PyO3). No dlopen, no ctypes marshalling, no runtime bridge: a door
is an ordinary ``METH_FASTCALL`` extension call into a direct Rust call. Every door
returns a verdict: :class:`Success` or :class:`Fault` (a closed reason plus the offending
byte span), never an exception for bad data — the only exceptions here are caller bugs (a
:class:`NumFormat`, a :class:`Constraints` or a day-period designator that cannot be), never
data.

The checked doors (:func:`cast_checked`, :func:`cast_checked_from_float`, and
:func:`cast_text` with constraints) judge a value that parsed by a :class:`Constraints`
declared for its door, and answer :class:`Checked` or a :data:`Rejection` —
:class:`Fault` or :class:`Violation`.

Consume with ``match``/``case`` over the two case types — Python's answer to C#'s native
union and Java's sealed interface. Exhaustiveness is the type checker's job here, not the
interpreter's (pair the two cases with ``typing.assert_never`` under mypy/pyright for the
compile-time guarantee the static bindings get natively)::

    match hypercast.cast_i32("(1,234)", hypercast.NumFormat.INVARIANT):
        case hypercast.Success(value):
            print("got", value)                       # -1234, accounting negative
        case hypercast.Fault(reason, offset, length):
            print(reason.name, "at char", offset)

Door names mirror the native ABI (``cast_i32``, ``cast_f64``, ``cast_timestamp``, …) so the
polyglot surface reads identically across bindings. Inputs are ``str`` (read as UTF-8) or
``bytes``, and a :class:`Fault`'s span comes back in the caller's own units — byte offsets
for ``bytes``, code-point offsets for ``str`` — so slicing the offending text back out of
what you passed needs no mapping either way. Python ``int`` is unbounded, so ``cast_u64``
returns the true unsigned value with no bit-pattern games; ``cast_decimal`` returns an
exact, canonical ``decimal.Decimal``;
``datetime``'s resolution is microseconds, so the core's nanoseconds truncate by three
digits on the temporal doors (the JVM binding is the fidelity king; this is Python's honest
ceiling).

Ships as real platform-specific wheels (linux glibc and musl, macOS, Windows; x64 and arm64,
plus Pyodide's ``pyemscripten`` wasm32 for the browser) built by ``maturin`` — no compiler
needed to install where a wheel fits; elsewhere pip builds it from the sdist, which needs
Rust. The wheels are ``abi3`` for every CPython 3.11+, and ``abi3t`` for free-threaded
CPython 3.15+, where the extension runs without the GIL and every door is safe to call
from many threads at once. The package is typed:
``py.typed`` ships beside it, with a stub for the extension module, so a checker sees each
door's own ``Success[...] | Fault`` rather than ``Any``.
"""

from __future__ import annotations

from enum import IntEnum
from types import ModuleType
from typing import TypeVar, Union, overload

from . import _native

__all__ = [
    "CastFailure",
    "Success",
    "Fault",
    "Verdict",
    "NumFormat",
    "Detail",
    "DateOptions",
    "MonthNames",
    "UnixPrecision",
    "DateOrder",
    "ExcelEpoch",
    "Door",
    "Rounding",
    "Constraints",
    "Checked",
    "Violation",
    "Keyword",
    "Rejection",
    "CheckedVerdict",
    "optional",
    "cast_bool",
    "cast_char",
    "cast_i8",
    "cast_i16",
    "cast_i32",
    "cast_i64",
    "cast_u8",
    "cast_u16",
    "cast_u32",
    "cast_u64",
    "cast_f32",
    "cast_f64",
    "cast_decimal",
    "cast_uuid",
    "cast_timestamp",
    "cast_unix",
    "cast_excel_serial",
    "cast_date",
    "cast_datetime",
    "cast_time",
    "cast_duration",
    "cast_decimal_from_float",
    "cast_excel_serial_from_float",
    "cast_excel_time",
    "cast_excel_duration",
    "cast_text",
    "cast_checked",
    "cast_checked_from_float",
    "cast_constraints_check",
    "cast_declared_decimal",
    "native_version",
]


class CastFailure(IntEnum):
    """The closed set of reasons a cast can fail — the native core's verdict codes, verbatim."""

    EMPTY = 1
    """Required input was empty or white space. :func:`optional` surfaces this as ``None``."""
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
    ISO ``yyyy-MM-dd`` only. :func:`cast_datetime` takes one too, and beside it a culture's
    day-period designators as keyword-only ``am`` and ``pm``, read alongside the English
    ``AM``/``PM``.
    """

    YEAR_MONTH_DAY = 1
    MONTH_DAY_YEAR = 2
    DAY_MONTH_YEAR = 3


class Door(IntEnum):
    """The door a :class:`Constraints` is bound to, numbered as the native core numbers it:
    the integer family, the two reals and the exact decimal (judged by :func:`cast_checked` and
    :func:`cast_checked_from_float`), and text (judged by :func:`cast_text`).
    """

    I8 = 2
    I16 = 3
    I32 = 4
    I64 = 5
    U8 = 6
    U16 = 7
    U32 = 8
    U64 = 9
    F32 = 10
    F64 = 11
    TEXT = 18
    DECIMAL = 19


class Rounding(IntEnum):
    """What a value that is not a multiple of the declared ``multiple_of`` becomes — the native
    core's codes, verbatim. Every mode but :data:`STRICT` needs a ``multiple_of``, and rounding
    happens on the exact decimal, before the bounds judge the result.
    """

    STRICT = 0
    """Not a multiple is a ``multipleOf`` violation. The default."""
    MIDPOINT = 1
    """To the nearest multiple, a tie to the even one."""
    FLOOR = 2
    """Down, toward negative infinity."""
    CEILING = 3
    """Up, toward positive infinity."""


class Detail(IntEnum):
    """What exactly a cast found wrong, beside the broad :class:`CastFailure` — the native core's
    codes, verbatim, for diagnosing rejected data (``fault.detail``).

    Published codes keep their numbers and meanings. New members may join in a minor release, so
    a ``match`` over details needs a ``case _:`` arm, and a code this version names no member for
    is :data:`UNKNOWN`: the fault's ``detail_code`` keeps the number, and its reason and span
    still stand. A detail says what failed, never how to retry, and is independent of the
    reason: an infinity is ``OUT_OF_RANGE`` at a float door and ``MALFORMED`` at an integer door,
    and :data:`NOT_FINITE` at both.
    """

    BLANK = 1
    """Empty, or only white space."""
    UNEXPECTED_CHARACTER = 2
    """A character the position doesn't allow, where the position allows several things: a
    letter in a numeral, an undeclared symbol."""
    DIGIT_EXPECTED = 3
    """A numeral or digit field is under way, or a fixed position requires a digit, and another
    character or the end is found (``0xG``, ``1e``, ``2026-0x-07``)."""
    DELIMITER_EXPECTED = 4
    """A required mark (``-``, ``:``, ``T``, ``}``, ``0x``, ``;``, a UUID hyphen) is missing or
    different."""
    TRAILING_TEXT = 5
    """The value is complete, but more text follows."""
    NO_DIGITS = 6
    """The numeral is absent entirely: only a sign, a currency or percent affix, a bare radix
    prefix, or a protobuf duration's bare ``s``."""
    SIGN_MISPLACED = 7
    """A sign where none may stand: a second sign, inside parentheses, mid-token, leading plus
    trailing."""
    PARENTHESES_UNBALANCED = 8
    """``(`` without ``)``, or ``( )``."""
    MARK_REPEATED = 9
    """A once-only mark appears twice (a second decimal separator, a second ``T``)."""
    GROUP_MISPLACED = 10
    """A group separator not between digits, or an empty group."""
    GROUP_SIZE = 11
    """A group with the wrong digit count for the declared grouping, or a first group that starts with a zero."""
    GROUP_IN_FRACTION = 12
    """Grouping after the decimal separator or in an exponent."""
    AMBIGUOUS_SEPARATOR = 13
    """Separator detection can't tell ``.`` from ``,``."""
    FRACTION_NOT_ALLOWED = 14
    """A fraction where the type is whole: an integer door, a Unix count, a negative exponent at
    an integer door, a fraction on a duration unit other than seconds."""
    FIELD_WIDTH = 15
    """A delimited field has the wrong number of digits."""
    WRONG_LENGTH = 16
    """A fixed-shape token, recognised by its total length, has the wrong length."""
    COMPONENT_MISSING = 17
    """A required part of the value is absent: a zone, seconds, a duration unit or any duration
    component, the UUID after a bare ``GUID:``."""
    NOT_RECOGNIZED = 18
    """Not one of the door's accepted words or values."""
    UNKNOWN_UNIT = 19
    """An unknown or misplaced duration unit."""
    INVALID_UTF8 = 20
    """The bytes where the cast failed don't decode as UTF-8."""
    TOO_LONG = 21
    """Past an implementation limit: a 256-byte numeral, an 18-digit run, a span past
    ``u32::MAX``."""
    NOT_FINITE = 22
    """A non-finite value: a NaN or infinity spelled at a numeric door, or a NaN or ∞ given to a
    door that can't represent it."""
    NOT_WHOLE = 23
    """A non-integral number at a whole-number door."""
    NEGATIVE_NOT_ALLOWED = 24
    """Negative where the type has no negatives (an unsigned integer, an Excel serial)."""
    TOO_LARGE = 25
    """The magnitude exceeds what the target type represents."""
    TOO_SMALL = 26
    """Below a signed integer type's minimum."""
    TOO_PRECISE = 27
    """More fractional digits than the type holds."""
    DATE_FIELD_OUT_OF_RANGE = 28
    """A date field that names nothing: year 0, month 13, a day past the month's end."""
    TIME_FIELD_OUT_OF_RANGE = 29
    """An hour, minute, second or offset field out of range."""
    OUTSIDE_WINDOW = 30
    """A well-formed instant or duration past the supported window (years 0001–9999, ±10,000
    years)."""
    NONEXISTENT_DATE = 31
    """A day that never existed: Excel's phantom 1900-02-29."""
    NOT_A_SCALAR = 32
    """A code point above U+10FFFF, or a surrogate."""
    UNKNOWN = -1
    """A code with no member here: published by a later core than this package names. Not a code
    itself — ``Fault.detail_code`` holds the number."""

    @classmethod
    def from_code(cls, code: int) -> Detail:
        """The member named for a core ``code``; :data:`UNKNOWN` for one this version does not
        name (and for ``0``, which no detail ever is). Never raises."""
        try:
            return cls(code)
        except ValueError:
            return cls.UNKNOWN


def _require_core_1_1(native: ModuleType) -> None:
    """Refuses a native core below 1.1, before anything is bound to it.

    A 1.0 core reads a format's grouping-policy bits as nothing and parses ``12,34`` as 1234
    whatever was declared, so a Western or Indian request against it would silently read otherwise.
    The package is built for the 1.1 core's surface (grouping policies, detail, date
    options), and a core that lacks it is a mismatched install, never a degraded mode.

    A capability check, not a numeric floor: the extension is the core, built from one crate,
    so its version cannot differ from the package's, and a crate still numbered 1.0.0 before the
    release bumps it (every dev and pre-release build) must load. What a 1.0 core lacks is the
    1.1 surface, which is what is asked for.
    """
    if hasattr(native, "DateOptions") and hasattr(native.NumFormat, "GROUPING_WESTERN"):
        return
    found = native.native_version() if hasattr(native, "native_version") else "of unknown version"
    raise ImportError(
        f"hypercast needs a native core of version 1.1 or later; the loaded hypercast._native "
        f"is {found}, which ignores the grouping policy a format declares. Reinstall hypercast "
        f"so the package and its extension come from one release."
    )


# The extension's own types and doors ARE the package surface — no delegation defs, no
# per-call Python frame on top (their docstrings live on the PyO3 functions/classes
# themselves). _bind hands over the CastFailure members so faults carry the exact enum
# members callers compare with `is`, plus uuid.UUID for cast_uuid's construction.
_require_core_1_1(_native)
_native._bind(CastFailure, Door, Rounding, Detail)

Success = _native.Success
Fault = _native.Fault
NumFormat = _native.NumFormat
DateOptions = _native.DateOptions
MonthNames = _native.MonthNames
Constraints = _native.Constraints
Checked = _native.Checked
Violation = _native.Violation
Keyword = _native.Keyword

cast_bool = _native.cast_bool
cast_char = _native.cast_char
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
cast_decimal_from_float = _native.cast_decimal_from_float
cast_excel_serial_from_float = _native.cast_excel_serial_from_float
cast_excel_time = _native.cast_excel_time
cast_excel_duration = _native.cast_excel_duration
cast_text = _native.cast_text
cast_checked = _native.cast_checked
cast_checked_from_float = _native.cast_checked_from_float
cast_constraints_check = _native.cast_constraints_check
cast_declared_decimal = _native.cast_declared_decimal
native_version = _native.native_version

_T = TypeVar("_T")

Verdict = Union[Success[_T], Fault]
"""The outcome of a cast: exactly one of :class:`Success` or :class:`Fault`. Generic in the
value a success carries — ``Verdict[int]`` is what :func:`cast_i32` returns."""


Rejection = Union[Fault, Violation]
"""Why a checked door gave no value: exactly one of :class:`Fault` (it did not parse — the very
type every plain door returns) or :class:`Violation` (it parsed and broke a declared rule)."""

CheckedVerdict = Union[Checked[_T], Fault, Violation]
"""The outcome of a checked door: :class:`Checked`, or a :data:`Rejection`. Generic in the
value a success carries — ``CheckedVerdict[float]`` is what :func:`cast_checked` returns for
``Constraints`` bound to ``Door.F64``."""


@overload
def optional(verdict: Success[_T] | Fault) -> Success[_T] | Fault | None: ...
@overload
def optional(
    verdict: Checked[_T] | Fault | Violation,
) -> Checked[_T] | Fault | Violation | None: ...
def optional(
    verdict: Success[_T] | Checked[_T] | Fault | Violation,
) -> Success[_T] | Checked[_T] | Fault | Violation | None:
    """Presents a verdict, plain or checked, optionally: a :data:`CastFailure.EMPTY` fault
    becomes ``None`` (Python's absent), everything else flows through untouched.
    """
    if isinstance(verdict, Fault) and verdict.reason is CastFailure.EMPTY:
        return None
    return verdict
