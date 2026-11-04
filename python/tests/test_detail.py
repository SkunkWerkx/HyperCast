"""Fault detail: what failed, beside the reason. The vocabulary is the core's (held to it here),
every door's fault carries its detail, and the 1.0 surface of ``Fault`` (its constructor, its
patterns, its equality and repr) behaves exactly as it did. The per-row details are replayed in
test_corpus.py."""

from __future__ import annotations

import ast
from pathlib import Path

import pytest

import hypercast
from hypercast import (
    CastFailure,
    Checked,
    Constraints,
    Detail,
    Door,
    Fault,
    NumFormat,
    Success,
    Violation,
    _native,
)


def test_the_vocabulary_is_the_cores_by_number_and_by_name():
    """Codes 1 to 32 are the members, never renumbered; each member's name is the core's own
    spelling upper-cased, and a code the core does not publish is no member."""
    assert [member.value for member in Detail if member is not Detail.UNKNOWN] == list(range(1, 33))
    for code in range(0, 64):
        published = _native._detail_name(code)
        if published is None:
            assert Detail.from_code(code) is Detail.UNKNOWN, code
        else:
            assert Detail(code).name.lower() == published, code
            assert Detail.from_code(code) is Detail(code), code
    assert Detail.UNKNOWN.value == -1


def test_every_member_is_documented():
    """Each member carries a docstring in the package source, the meaning the core gives it."""
    tree = ast.parse(Path(hypercast.__file__).read_text(encoding="utf-8"))
    (detail,) = [n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == "Detail"]
    documented = set()
    for assignment, after in zip(detail.body, detail.body[1:]):
        if (
            isinstance(assignment, ast.Assign)
            and isinstance(after, ast.Expr)
            and isinstance(after.value, ast.Constant)
            and isinstance(after.value.value, str)
        ):
            documented.add(assignment.targets[0].id)  # type: ignore[attr-defined]
    assert documented == {member.name for member in Detail}


# --- the 1.0 surface of Fault, as 1.0 code used it ---------------------------------------------


def test_fault_keeps_its_1_0_constructor_patterns_equality_and_repr():
    """What a program written against 1.0 does with a Fault still works, byte for byte: three
    positional arguments, positional destructuring, equality over reason, offset and length,
    the repr, and unhashability."""
    fault = Fault(CastFailure.MALFORMED, 4, 1)
    assert Fault.__match_args__ == ("reason", "offset", "length")
    assert repr(fault) == "Fault(reason=<CastFailure.MALFORMED: 2>, offset=4, length=1)"
    assert fault.detail is None and fault.detail_code is None
    match fault:
        case Fault(reason, offset, length):
            assert (reason, offset, length) == (CastFailure.MALFORMED, 4, 1)
        case other:  # pragma: no cover
            pytest.fail(f"{other!r} did not destructure")
    assert fault == Fault(CastFailure.MALFORMED, 4, 1)
    assert fault != Fault(CastFailure.MALFORMED, 4, 2)
    assert fault != Fault(CastFailure.EMPTY, 4, 1)
    assert fault != (CastFailure.MALFORMED, 4, 1)
    with pytest.raises(TypeError):
        hash(fault)
    with pytest.raises(AttributeError):
        fault.offset = 0
    # The detail is keyword-only: a fourth positional argument is still a TypeError.
    with pytest.raises(TypeError):
        Fault(CastFailure.MALFORMED, 4, 1, 12)  # type: ignore[call-arg]


def test_a_detail_never_changes_what_a_fault_equals():
    """Equality is the reason, offset and length alone: faults that differ only in detail are
    equal, with it or without, and a door's fault equals its 1.0-style twin."""
    base = Fault(CastFailure.MALFORMED, 2, 3)
    assert base == Fault(CastFailure.MALFORMED, 2, 3, detail_code=Detail.GROUP_SIZE)
    assert Fault(CastFailure.MALFORMED, 2, 3, detail_code=1) == Fault(
        CastFailure.MALFORMED, 2, 3, detail_code=999
    )
    from_a_door = hypercast.cast_i32("12,34", NumFormat.INVARIANT)
    assert isinstance(from_a_door, Fault) and from_a_door.detail is Detail.GROUP_SIZE
    assert from_a_door == base and base == from_a_door


def test_the_detail_rides_beside_the_positional_pattern():
    """A keyword pattern reaches the detail where a positional one cannot, and the positional
    pattern over the same fault is unchanged."""
    verdict = hypercast.cast_i32("12,34", NumFormat.INVARIANT)
    match verdict:
        case Fault(CastFailure.MALFORMED, 2, 3, detail=Detail.GROUP_SIZE):
            pass
        case other:  # pragma: no cover
            pytest.fail(f"{other!r}")
    match verdict:
        case Fault(reason, offset, length):
            assert (reason, offset, length) == (CastFailure.MALFORMED, 2, 3)
        case other:  # pragma: no cover
            pytest.fail(f"{other!r}")


def test_a_repr_names_the_detail_only_when_there_is_one():
    """A hand-built fault's repr is the 1.0 text; a door's adds its detail, so a log says why."""
    assert hypercast.cast_i32("12,34", NumFormat.INVARIANT).__repr__() == (
        "Fault(reason=<CastFailure.MALFORMED: 2>, offset=2, length=3, "
        "detail=<Detail.GROUP_SIZE: 11>)"
    )
    assert repr(Fault(CastFailure.EMPTY, 0, 0)) == (
        "Fault(reason=<CastFailure.EMPTY: 1>, offset=0, length=0)"
    )


# --- an unpublished code decodes without throwing ---------------------------------------------


def test_an_unpublished_code_decodes_to_unknown_and_keeps_its_number():
    """A fault carrying a code this version names no member for (a later core's, here 999)
    decodes: no exception, ``Detail.UNKNOWN`` as the named value, the raw number beside it, and
    the reason and span as usable as ever."""
    fault = Fault(CastFailure.MALFORMED, 2, 1, detail_code=999)
    assert fault.detail is Detail.UNKNOWN
    assert fault.detail_code == 999
    assert (fault.reason, fault.offset, fault.length) == (CastFailure.MALFORMED, 2, 1)
    assert "12,34"[fault.offset : fault.offset + fault.length] == ","
    assert repr(fault) == (
        "Fault(reason=<CastFailure.MALFORMED: 2>, offset=2, length=1, detail=<Detail.UNKNOWN: -1>)"
    )
    match fault:
        case Fault(CastFailure.MALFORMED, offset, length, detail=Detail.UNKNOWN):
            assert (offset, length) == (2, 1)
        case other:  # pragma: no cover
            pytest.fail(f"{other!r}")
    for code in (0, 33, 999, 2**32 - 1):
        assert Fault(CastFailure.MALFORMED, 0, 0, detail_code=code).detail is Detail.UNKNOWN
        assert Detail.from_code(code) is Detail.UNKNOWN
    # A code no 32 unsigned bits hold is a declaration the core could not carry: a ValueError.
    for wide in (-1, 2**32, 2**70, -(2**70)):
        with pytest.raises(ValueError, match="detail_code must be an int in 32 unsigned bits"):
            Fault(CastFailure.MALFORMED, 0, 0, detail_code=wide)
    with pytest.raises(TypeError):
        Fault(CastFailure.MALFORMED, 0, 0, detail_code="11")  # type: ignore[arg-type]
    assert Fault(CastFailure.MALFORMED, 0, 0, detail_code=None).detail is None


# --- every door's fault carries its detail ---------------------------------------------------


def _detail(verdict) -> Detail | None:
    assert isinstance(verdict, Fault), verdict
    return verdict.detail


def test_every_family_of_door_reports_a_detail():
    """Text doors, the typed doors, the text door, and the checked doors."""
    fmt = NumFormat.INVARIANT
    assert _detail(hypercast.cast_i32("", fmt)) is Detail.BLANK
    assert hypercast.cast_i32("  ", fmt).reason is CastFailure.EMPTY
    assert _detail(hypercast.cast_i32("12a", fmt)) is Detail.UNEXPECTED_CHARACTER
    assert _detail(hypercast.cast_i8("128", fmt)) is Detail.TOO_LARGE
    assert _detail(hypercast.cast_u8("-1", fmt)) is Detail.NEGATIVE_NOT_ALLOWED
    assert _detail(hypercast.cast_f64("1e400", fmt)) is Detail.TOO_LARGE
    assert _detail(hypercast.cast_f64("NaN", fmt)) is Detail.NOT_FINITE
    assert _detail(hypercast.cast_bool("maybe")) is Detail.NOT_RECOGNIZED
    assert _detail(hypercast.cast_char("U+")) is Detail.NO_DIGITS
    assert _detail(hypercast.cast_uuid("nope")) is Detail.WRONG_LENGTH
    assert _detail(hypercast.cast_date("2026-13-01")) is Detail.DATE_FIELD_OUT_OF_RANGE
    assert _detail(hypercast.cast_time("25:00")) is Detail.TIME_FIELD_OUT_OF_RANGE
    assert _detail(hypercast.cast_duration("P")) is Detail.COMPONENT_MISSING
    assert _detail(hypercast.cast_text(b"\xff")) is Detail.INVALID_UTF8
    assert _detail(hypercast.cast_text("")) is Detail.BLANK
    # A typed door's fault has an empty span and its own details.
    nan = hypercast.cast_decimal_from_float(float("nan"))
    assert _detail(nan) is Detail.NOT_FINITE
    assert (nan.offset, nan.length) == (0, 0)
    assert _detail(hypercast.cast_excel_time(-1.0)) is Detail.NEGATIVE_NOT_ALLOWED
    assert _detail(hypercast.cast_excel_duration(float("inf"))) is Detail.NOT_FINITE


def test_a_fault_from_a_str_door_keeps_its_detail_in_code_points():
    """The span is the caller's own units; the detail rides along unchanged."""
    verdict = hypercast.cast_i32("€x", NumFormat.INVARIANT)
    assert verdict == Fault(CastFailure.MALFORMED, 0, 1)
    assert _detail(verdict) is Detail.UNEXPECTED_CHARACTER


def test_the_checked_doors_report_a_detail_on_a_fault_and_none_on_a_violation():
    """A checked fault is a door's fault with its detail; a violation is a different type and
    carries no detail (the core writes 0 there)."""
    fmt = NumFormat.INVARIANT
    constraints = Constraints(Door.I32, maximum=10)
    fault = hypercast.cast_checked("1x", fmt, constraints)
    assert _detail(fault) is Detail.UNEXPECTED_CHARACTER
    assert _detail(hypercast.cast_checked("12,34", fmt, constraints)) is Detail.GROUP_SIZE
    assert _detail(hypercast.cast_checked_from_float(float("nan"), constraints)) is (
        Detail.NOT_FINITE
    )
    violation = hypercast.cast_checked("11", fmt, constraints)
    assert isinstance(violation, Violation)
    assert not hasattr(violation, "detail") and not hasattr(violation, "detail_code")
    assert hypercast.cast_checked("9", fmt, constraints) == Checked(9)
    text = Constraints(Door.TEXT, min_length=3)
    assert _detail(hypercast.cast_text(b"\xff\xff\xff", text)) is Detail.INVALID_UTF8
    assert _detail(hypercast.cast_text("", text)) is Detail.BLANK


def test_optional_still_turns_an_empty_fault_into_none():
    """``optional`` reads the reason, which a detail does not change."""
    assert hypercast.optional(hypercast.cast_i32("  ", NumFormat.INVARIANT)) is None
    assert hypercast.optional(hypercast.cast_i32("1", NumFormat.INVARIANT)) == Success(1)
    kept = hypercast.optional(hypercast.cast_i32("x", NumFormat.INVARIANT))
    assert isinstance(kept, Fault) and kept.detail is Detail.UNEXPECTED_CHARACTER
