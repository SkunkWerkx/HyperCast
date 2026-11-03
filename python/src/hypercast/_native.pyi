"""The typed surface of ``hypercast._native``, the PyO3 extension (``tests/test_typing.py``
holds the loaded module to this file). ``hypercast`` re-exports all of it directly, so this
is what a type checker sees behind every door: its own ``Success[...] | Fault``, which is
what lets a ``match`` over the two cases be checked for exhaustiveness with
``typing.assert_never``.
"""

import datetime
import decimal
import uuid
from collections.abc import Iterable
from typing import Any, ClassVar, Final, Generic, Literal, TypeVar, final, overload

from . import CastFailure, DateOrder, Door, ExcelEpoch, Rounding, UnixPrecision

_T_co = TypeVar("_T_co", covariant=True)
_T = TypeVar("_T")
#: What a numeric door's Constraints deliver.
_N = TypeVar("_N", int, float, decimal.Decimal)
#: A bound, step or integer enum entry: exact, never a float.
_Exact = decimal.Decimal | int | str

@final
class Success(Generic[_T_co]):
    __match_args__: Final = ("value",)
    __hash__: ClassVar[None]  # type: ignore[assignment]
    def __new__(cls, value: _T_co) -> Success[_T_co]: ...
    @property
    def value(self) -> _T_co: ...
    def __eq__(self, other: object) -> bool: ...

@final
class Fault:
    __match_args__: Final = ("reason", "offset", "length")
    __hash__: ClassVar[None]  # type: ignore[assignment]
    def __new__(cls, reason: CastFailure, offset: int, length: int) -> Fault: ...
    @property
    def reason(self) -> CastFailure: ...
    @property
    def offset(self) -> int: ...
    @property
    def length(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...

@final
class NumFormat:
    GROUPING: ClassVar[int]
    PARENTHESES: ClassVar[int]
    EXPONENT: ClassVar[int]
    RADIX_PREFIXES: ClassVar[int]
    PERCENT: ClassVar[int]
    CURRENCY: ClassVar[int]
    TRAILING_SIGN: ClassVar[int]
    ALL: ClassVar[int]
    SEPARATOR_DETECT: ClassVar[int]
    INVARIANT: ClassVar[NumFormat]
    DETECT: ClassVar[NumFormat]
    def __new__(
        cls,
        decimal_sep: str,
        group_sep: str,
        flags: int,
        currency: str = "",
        *,
        currency_decimal_sep: str = "",
        currency_group_sep: str = "",
        minus_sign: str = "",
        plus_sign: str = "",
        percent: str = "",
        digit_zero: str = "",
    ) -> NumFormat: ...
    @property
    def decimal_sep(self) -> str: ...
    @property
    def group_sep(self) -> str: ...
    @property
    def flags(self) -> int: ...
    @property
    def currency(self) -> str: ...
    @property
    def currency_decimal_sep(self) -> str: ...
    @property
    def currency_group_sep(self) -> str: ...
    @property
    def minus_sign(self) -> str: ...
    @property
    def plus_sign(self) -> str: ...
    @property
    def percent(self) -> str: ...
    @property
    def digit_zero(self) -> str: ...
    @property
    def packed(self) -> bytes:
        """The 92 bytes this format crosses a C ABI as, for a library with one of its own."""
    @staticmethod
    def from_localeconv(conv: dict[str, Any] | None = None) -> NumFormat: ...

@final
class Constraints(Generic[_T]):
    @overload
    def __new__(
        cls,
        door: Literal[Door.I8, Door.I16, Door.I32, Door.I64, Door.U8, Door.U16, Door.U32, Door.U64],
        *,
        minimum: _Exact | None = None,
        exclusive_minimum: _Exact | None = None,
        maximum: _Exact | None = None,
        exclusive_maximum: _Exact | None = None,
        multiple_of: _Exact | None = None,
        rounding: Rounding = ...,
        min_length: int | None = None,
        max_length: int | None = None,
        trim: bool = False,
        enum: Iterable[_Exact] | None = None,
    ) -> Constraints[int]: ...
    @overload
    def __new__(
        cls,
        door: Literal[Door.F32, Door.F64],
        *,
        minimum: _Exact | None = None,
        exclusive_minimum: _Exact | None = None,
        maximum: _Exact | None = None,
        exclusive_maximum: _Exact | None = None,
        multiple_of: _Exact | None = None,
        rounding: Rounding = ...,
        min_length: int | None = None,
        max_length: int | None = None,
        trim: bool = False,
        enum: Iterable[_Exact] | None = None,
    ) -> Constraints[float]: ...
    @overload
    def __new__(
        cls,
        door: Literal[Door.DECIMAL],
        *,
        minimum: _Exact | None = None,
        exclusive_minimum: _Exact | None = None,
        maximum: _Exact | None = None,
        exclusive_maximum: _Exact | None = None,
        multiple_of: _Exact | None = None,
        rounding: Rounding = ...,
        min_length: int | None = None,
        max_length: int | None = None,
        trim: bool = False,
        enum: Iterable[_Exact] | None = None,
    ) -> Constraints[decimal.Decimal]: ...
    @overload
    def __new__(
        cls,
        door: Literal[Door.TEXT],
        *,
        minimum: _Exact | None = None,
        exclusive_minimum: _Exact | None = None,
        maximum: _Exact | None = None,
        exclusive_maximum: _Exact | None = None,
        multiple_of: _Exact | None = None,
        rounding: Rounding = ...,
        min_length: int | None = None,
        max_length: int | None = None,
        trim: bool = False,
        enum: Iterable[str] | None = None,
    ) -> Constraints[str]: ...
    @overload
    def __new__(
        cls,
        door: Door,
        *,
        minimum: _Exact | None = None,
        exclusive_minimum: _Exact | None = None,
        maximum: _Exact | None = None,
        exclusive_maximum: _Exact | None = None,
        multiple_of: _Exact | None = None,
        rounding: Rounding = ...,
        min_length: int | None = None,
        max_length: int | None = None,
        trim: bool = False,
        enum: Iterable[_Exact] | None = None,
    ) -> Constraints[Any]: ...
    @property
    def door(self) -> Door: ...
    @property
    def minimum(self) -> decimal.Decimal | None: ...
    @property
    def exclusive_minimum(self) -> decimal.Decimal | None: ...
    @property
    def maximum(self) -> decimal.Decimal | None: ...
    @property
    def exclusive_maximum(self) -> decimal.Decimal | None: ...
    @property
    def multiple_of(self) -> decimal.Decimal | None: ...
    @property
    def rounding(self) -> Rounding: ...
    @property
    def min_length(self) -> int | None: ...
    @property
    def max_length(self) -> int | None: ...
    @property
    def trim(self) -> bool: ...
    @property
    def enum(self) -> tuple[Any, ...] | None: ...
    @property
    def packed(self) -> bytes:
        """The 72 bytes this declaration crosses a C ABI as, native byte order."""
    @property
    def packed_enum(self) -> bytes:
        """The enum list as the core reads it beside ``packed``."""
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Keyword:
    MINIMUM: ClassVar[Keyword]
    EXCLUSIVE_MINIMUM: ClassVar[Keyword]
    MAXIMUM: ClassVar[Keyword]
    EXCLUSIVE_MAXIMUM: ClassVar[Keyword]
    MULTIPLE_OF: ClassVar[Keyword]
    MIN_LENGTH: ClassVar[Keyword]
    MAX_LENGTH: ClassVar[Keyword]
    ENUM: ClassVar[Keyword]
    def __new__(cls, code: int) -> Keyword: ...
    @property
    def code(self) -> int: ...
    @property
    def name(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Checked(Generic[_T_co]):
    __match_args__: Final = ("value", "adjusted", "enum_index")
    __hash__: ClassVar[None]  # type: ignore[assignment]
    def __new__(
        cls, value: _T_co, adjusted: bool = False, enum_index: int | None = None
    ) -> Checked[_T_co]: ...
    @property
    def value(self) -> _T_co: ...
    @property
    def adjusted(self) -> bool: ...
    @property
    def enum_index(self) -> int | None: ...
    def __eq__(self, other: object) -> bool: ...

@final
class Violation:
    __match_args__: Final = ("keyword", "offset", "length")
    __hash__: ClassVar[None]  # type: ignore[assignment]
    def __new__(cls, keyword: Keyword, offset: int, length: int) -> Violation: ...
    @property
    def keyword(self) -> Keyword: ...
    @property
    def offset(self) -> int: ...
    @property
    def length(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...

def cast_bool(text: str | bytes) -> Success[bool] | Fault: ...
def cast_char(text: str | bytes) -> Success[str] | Fault: ...
def cast_i8(text: str | bytes, fmt: NumFormat) -> Success[int] | Fault: ...
def cast_i16(text: str | bytes, fmt: NumFormat) -> Success[int] | Fault: ...
def cast_i32(text: str | bytes, fmt: NumFormat) -> Success[int] | Fault: ...
def cast_i64(text: str | bytes, fmt: NumFormat) -> Success[int] | Fault: ...
def cast_u8(text: str | bytes, fmt: NumFormat) -> Success[int] | Fault: ...
def cast_u16(text: str | bytes, fmt: NumFormat) -> Success[int] | Fault: ...
def cast_u32(text: str | bytes, fmt: NumFormat) -> Success[int] | Fault: ...
def cast_u64(text: str | bytes, fmt: NumFormat) -> Success[int] | Fault: ...
def cast_f32(text: str | bytes, fmt: NumFormat) -> Success[float] | Fault: ...
def cast_f64(text: str | bytes, fmt: NumFormat) -> Success[float] | Fault: ...
def cast_decimal(text: str | bytes, fmt: NumFormat) -> Success[decimal.Decimal] | Fault: ...
def cast_uuid(text: str | bytes) -> Success[uuid.UUID] | Fault: ...
def cast_timestamp(text: str | bytes) -> Success[datetime.datetime] | Fault: ...
def cast_unix(
    text: str | bytes, precision: UnixPrecision
) -> Success[datetime.datetime] | Fault: ...
def cast_excel_serial(
    text: str | bytes, epoch: ExcelEpoch
) -> Success[datetime.datetime] | Fault: ...
def cast_date(
    text: str | bytes, order: DateOrder | None = None
) -> Success[datetime.date] | Fault: ...
def cast_datetime(
    text: str | bytes, order: DateOrder, *, am: str = "", pm: str = ""
) -> Success[datetime.datetime] | Fault: ...
def cast_time(text: str | bytes) -> Success[datetime.time] | Fault: ...
def cast_duration(text: str | bytes) -> Success[datetime.timedelta] | Fault: ...
def cast_decimal_from_float(value: float) -> Success[decimal.Decimal] | Fault: ...
def cast_excel_serial_from_float(
    value: float, epoch: ExcelEpoch
) -> Success[datetime.datetime] | Fault: ...
def cast_excel_time(value: float) -> Success[datetime.time] | Fault: ...
def cast_excel_duration(value: float) -> Success[datetime.timedelta] | Fault: ...
@overload
def cast_text(text: str | bytes, constraints: None = None) -> Success[str] | Fault: ...
@overload
def cast_text(
    text: str | bytes, constraints: Constraints[str]
) -> Checked[str] | Fault | Violation: ...
def cast_checked(
    text: str | bytes, fmt: NumFormat, constraints: Constraints[_N]
) -> Checked[_N] | Fault | Violation: ...
def cast_checked_from_float(
    value: float, constraints: Constraints[_N]
) -> Checked[_N] | Fault | Violation: ...
def cast_constraints_check(
    door: Door,
    *,
    minimum: _Exact | None = None,
    exclusive_minimum: _Exact | None = None,
    maximum: _Exact | None = None,
    exclusive_maximum: _Exact | None = None,
    multiple_of: _Exact | None = None,
    rounding: Rounding = ...,
    min_length: int | None = None,
    max_length: int | None = None,
    trim: bool = False,
    enum: Iterable[_Exact] | None = None,
) -> int: ...
def cast_declared_decimal(text: str | bytes) -> decimal.Decimal: ...
def native_version() -> str: ...
def _bind(cast_failure: type[CastFailure], door: type[Door], rounding: type[Rounding]) -> None: ...
