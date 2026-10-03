"""Pins the extension's own contracts, the ones no corpus vector can express: the fast UUID
constructor produces objects indistinguishable from ``UUID(...)``-constructed ones (the
``__slots__`` invariant it leans on), the verdict types hold their shape, a caller's bug is
the documented exception, and every door answers ``help()``."""

from __future__ import annotations

import uuid

import pytest

import hypercast
from hypercast import CastFailure, DateOrder, ExcelEpoch, Fault, NumFormat, Success, UnixPrecision

DOORS = [name for name in hypercast.__all__ if name.startswith("cast_")]


def test_fast_constructed_uuids_are_indistinguishable():
    # The pin for the UUID.__new__ + object.__setattr__ fast path cast_uuid builds through:
    # every observable surface of the result must match a stdlib-constructed twin, so a
    # change to uuid.UUID's slots fails here instead of handing callers a half-built object.
    text = "01020304-0506-4708-890a-0b0c0d0e0f10"
    cast = hypercast.cast_uuid(text).value
    twin = uuid.UUID(text)
    assert type(cast) is uuid.UUID
    assert cast == twin
    assert hash(cast) == hash(twin)
    assert cast.int == twin.int
    assert cast.bytes == twin.bytes
    assert cast.version == twin.version == 4
    assert cast.variant == twin.variant
    assert cast.is_safe is uuid.SafeUUID.unknown
    assert str(cast) == str(twin) == text
    assert repr(cast) == repr(twin)


def test_the_verdict_types_hold_their_shape():
    success, fault = Success(42), Fault(CastFailure.MALFORMED, 4, 1)
    assert Success.__match_args__ == ("value",)
    assert Fault.__match_args__ == ("reason", "offset", "length")
    assert repr(success) == "Success(value=42)"
    assert repr(fault) == "Fault(reason=<CastFailure.MALFORMED: 2>, offset=4, length=1)"
    # Immutable, and — defining equality — deliberately unhashable.
    with pytest.raises(AttributeError):
        success.value = 43
    with pytest.raises(AttributeError):
        fault.offset = 0
    for verdict in (success, fault):
        with pytest.raises(TypeError):
            hash(verdict)
    assert success != fault and success != 42 and fault != (CastFailure.MALFORMED, 4, 1)


def test_success_is_generic_at_runtime_as_well_as_in_the_stub():
    # `Success[int]` is how the stub spells a door's success case; an annotation that gets
    # evaluated must not raise.
    assert Success[int].__origin__ is Success
    assert hypercast.Verdict[int].__args__ == (Success[int], Fault)


# --- a caller's bug is an exception, of the documented type ---------------------------------


@pytest.mark.parametrize("door", DOORS)
@pytest.mark.parametrize("text", [42, 4.2, None, bytearray(b"42"), memoryview(b"42")])
def test_text_that_is_neither_str_nor_bytes_is_a_type_error(door, text):
    cast = getattr(hypercast, door)
    declared = {
        "cast_unix": (UnixPrecision.SECONDS,),
        "cast_excel_serial": (ExcelEpoch.Y1900,),
        "cast_datetime": (DateOrder.YEAR_MONTH_DAY,),
        "cast_bool": (), "cast_uuid": (), "cast_timestamp": (), "cast_date": (),
        "cast_time": (), "cast_duration": (),
    }.get(door, (NumFormat.INVARIANT,))
    with pytest.raises(TypeError):
        cast(text, *declared)


_DECLARED = [
    (hypercast.cast_unix, 4),
    (hypercast.cast_excel_serial, 2),
    (hypercast.cast_date, 3),
    (hypercast.cast_datetime, 3),
]


@pytest.mark.parametrize("door, members", _DECLARED)
def test_a_declaration_is_checked_for_type_then_width_then_membership(door, members):
    for wrong_type in ("1", 1.0, b"1"):
        with pytest.raises(TypeError):
            door("1", wrong_type)
    # Not a 32-bit unsigned integer at all: the extension's argument extraction says so
    # before the door looks at it.
    for too_wide in (-1, 2**32, 2**70):
        with pytest.raises(OverflowError):
            door("1", too_wide)
    for not_a_member in (0, members + 1, 255):
        with pytest.raises(ValueError):
            door("1", not_a_member)


def test_num_format_arguments_are_checked_for_type_before_value():
    for args in ((1, ",", 0), (".", b",", 0), (".", ",", 0, 5), (".", ",", "0"), (".", ",", 1.5)):
        with pytest.raises(TypeError):
            NumFormat(*args)
    for flags in (-1, 2**32):
        with pytest.raises(OverflowError):
            NumFormat(".", ",", flags)
    # A type error wins over a value error in the same call, as argument extraction does.
    with pytest.raises(TypeError):
        NumFormat("too long", ",", "0")
    for args in (("", ",", 0), ("ab", ",", 0), (".", ".", 0)):
        with pytest.raises(ValueError):
            NumFormat(*args)
    with pytest.raises(TypeError):
        hypercast.cast_i32("42", (".", ",", 0))


def test_a_str_that_is_not_encodable_is_the_one_exception_a_door_raises_for_data():
    # A lone surrogate has no UTF-8 form, so there are no bytes to hand the core and no span
    # to report: the str itself is rejected before any door
    # runs. Every other piece of bad data is a Fault.
    for door in (hypercast.cast_bool, hypercast.cast_uuid, hypercast.cast_timestamp):
        with pytest.raises(UnicodeEncodeError):
            door("\ud800")
    with pytest.raises(UnicodeEncodeError):
        hypercast.cast_i32("4\udfff2", NumFormat.INVARIANT)


# --- help() ----------------------------------------------------------------------------------

@pytest.mark.parametrize("door", DOORS + ["native_version", "optional"])
def test_every_door_has_a_docstring(door):
    doc = getattr(hypercast, door).__doc__
    assert doc and doc.strip(), f"help(hypercast.{door}) is empty"


@pytest.mark.parametrize("member", ["from_localeconv", "decimal_sep", "group_sep", "flags", "currency"])
def test_num_format_members_have_docstrings(member):
    doc = getattr(NumFormat, member).__doc__
    assert doc and doc.strip(), f"help(NumFormat.{member}) is empty"


def test_the_verdict_types_have_docstrings():
    for cls in (Success, Fault, NumFormat):
        assert cls.__doc__ and cls.__doc__.strip()
