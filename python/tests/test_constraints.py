"""Pins the validation constraints' own contracts, the ones the corpus replay cannot express:
how a declaration is built and refused, what each checked type looks like, that a lone
surrogate is never replaced, that spans come back in code points for ``str``, and that a text
``enum`` match hands back the caller's own entry. Every rule itself is the core's, replayed
from corpus/constraints.json in test_corpus.py."""

from __future__ import annotations

import re
import struct
from decimal import Decimal

import pytest

import hypercast
from hypercast import (
    CastFailure,
    Checked,
    Constraints,
    Door,
    Fault,
    Keyword,
    NumFormat,
    Rounding,
    Success,
    Violation,
)

from test_corpus import refusal_message

INVARIANT = NumFormat.INVARIANT
EURO = NumFormat(",", ".", NumFormat.ALL, "€")


# --- a declaration ---------------------------------------------------------------------------


def test_a_declaration_names_its_door():
    """No door is a TypeError: there is no declaration that silently means "no rules". An
    empty declaration is one the caller built on purpose, for a door, and judges nothing."""
    with pytest.raises(TypeError):
        Constraints()  # type: ignore[call-overload]
    for not_a_door in (0, 1, 12, 17, 20, 99):
        with pytest.raises(ValueError, match="door must be a Door"):
            Constraints(not_a_door)  # type: ignore[call-overload]
        with pytest.raises(ValueError, match="door must be a Door"):
            hypercast.cast_constraints_check(not_a_door)  # type: ignore[arg-type]
    empty = Constraints(Door.F64)
    assert hypercast.cast_checked("1e300", INVARIANT, empty) == Checked(1e300)
    assert empty.packed_enum == b"" and empty.enum is None


def test_bounds_are_exact_never_a_float():
    """A bound or step is a decimal.Decimal, an int or a str; a float (or a bool) is the
    TypeError a wrong argument type always is, and a value no door can hold is refused."""
    for exact in (Decimal("0.1"), "0.1", "1e-1", Decimal("1E-1")):
        assert Constraints(Door.F64, maximum=exact).maximum == Decimal("0.1")
    assert Constraints(Door.I32, minimum=-(2**31)).minimum == -(2**31)
    for inexact in (0.1, 1.0, True, b"0.1", [1]):
        with pytest.raises(TypeError, match="must be a decimal.Decimal, an int or a str"):
            Constraints(Door.F64, maximum=inexact)  # type: ignore[arg-type]
        with pytest.raises(TypeError):
            hypercast.cast_constraints_check(Door.F64, multiple_of=inexact)  # type: ignore[arg-type]
    for no_value in ("", "0.1 ", "1_000", "NaN", Decimal("NaN"), Decimal("Infinity"), "1e-29"):
        with pytest.raises(ValueError, match=re.escape(refusal_message(8))):
            Constraints(Door.DECIMAL, maximum=no_value)
        assert hypercast.cast_constraints_check(Door.DECIMAL, maximum=no_value) == 8


@pytest.mark.parametrize(
    "door, keywords, code",
    [
        (Door.I32, {"trim": True}, 1),
        (Door.F64, {"case_sensitive": True}, 1),
        (Door.TEXT, {"multiple_of": 1, "rounding": Rounding.MIDPOINT}, 1),
        (Door.DECIMAL, {"multiple_of": "-0.05"}, 5),
        (Door.F64, {"rounding": Rounding.MIDPOINT}, 6),
        (Door.DECIMAL, {"maximum": "1e-29"}, 8),
        (Door.I32, {"multiple_of": "0.5", "rounding": Rounding.MIDPOINT}, 8),
        (Door.TEXT, {"enum": ["ok", "\ud800"]}, 9),
        (Door.F64, {"multiple_of": 1, "rounding": 7}, 11),
    ],
)
def test_every_refusal_raises_its_own_message(door, keywords, code):
    """Each code the core refuses with is a ValueError worded from that code, and the same
    declaration through cast_constraints_check answers the code without raising."""
    with pytest.raises(ValueError, match=f"^{re.escape(refusal_message(code))}$"):
        Constraints(door, **keywords)
    assert hypercast.cast_constraints_check(door, **keywords) == code


@pytest.mark.parametrize(
    "door, keywords",
    [
        (Door.I32, {"min_length": 1}),
        (Door.TEXT, {"minimum": 1}),
        (Door.F64, {"minimum": 1, "exclusive_minimum": 0}),
        (Door.F64, {"maximum": 1, "exclusive_maximum": 2}),
        (Door.I8, {"minimum": 5, "maximum": 5}),
        (Door.I8, {"minimum": 6, "maximum": 5}),
        (Door.TEXT, {"min_length": 3, "max_length": 2}),
        (Door.U8, {"minimum": "-1", "maximum": 300}),
        (Door.I32, {"minimum": "0.5"}),
        (Door.I32, {"multiple_of": "0.5"}),
        (Door.TEXT, {"enum": ["Straße", "STRASSE", "straße"]}),
        (Door.I32, {"enum": []}),
        (Door.TEXT, {"enum": ()}),
        (Door.F64, {"enum": []}),
        (Door.I32, {"enum": ["7.5", 300, 7]}),
        (Door.TEXT, {"enum": ["Male", "", " "]}),
        (Door.TEXT, {"trim": True, "enum": [" Male "]}),
        (Door.TEXT, {"case_sensitive": True, "enum": ["Male", "Male"]}),
    ],
)
def test_every_declaration_openapi_allows_is_accepted(door, keywords):
    """OpenAPI 3.2 allows each of these, so the core accepts it: a keyword for another type is
    ignored, every bound applies on its own, and an enum may hold anything, even nothing."""
    assert hypercast.cast_constraints_check(door, **keywords) == 0
    Constraints(door, **keywords)


def test_bounds_and_enums_are_judged_as_openapi_says():
    """Four independent bounds, an ignored keyword, an empty enum no value is in, and entries
    no value can equal, each judged on the value as the door cast it."""
    both = Constraints(Door.F64, minimum=1, exclusive_minimum=0)
    assert hypercast.cast_checked("0.5", INVARIANT, both) == Violation(Keyword.MINIMUM, 0.5, 0, 3)
    assert hypercast.cast_checked("0", INVARIANT, both) == Violation(Keyword.MINIMUM, 0.0, 0, 1)
    assert hypercast.cast_checked("1", INVARIANT, both) == Checked(1.0)
    upper = Constraints(Door.I32, maximum=10, exclusive_maximum=10)
    assert hypercast.cast_checked("10", INVARIANT, upper) == Violation(
        Keyword.EXCLUSIVE_MAXIMUM, 10, 0, 2
    )
    crossed = Constraints(Door.I8, minimum=6, maximum=5)
    assert hypercast.cast_checked("5", INVARIANT, crossed) == Violation(Keyword.MINIMUM, 5, 0, 1)
    assert hypercast.cast_checked("7", INVARIANT, Constraints(Door.I32, min_length=9)) == Checked(7)
    nothing = Constraints(Door.I32, enum=[])
    assert hypercast.cast_checked("0", INVARIANT, nothing) == Violation(Keyword.ENUM, 0, 0, 1)
    assert hypercast.cast_text("x", Constraints(Door.TEXT, enum=[])) == Violation(
        Keyword.ENUM, "x", 0, 1
    )
    halves = Constraints(Door.I32, enum=["7.5", 8])
    assert hypercast.cast_checked("8", INVARIANT, halves) == Checked(8)
    assert hypercast.cast_checked("7", INVARIANT, halves) == Violation(Keyword.ENUM, 7, 0, 1)
    prices = Constraints(Door.DECIMAL, enum=["1.5", "2.25"])
    assert hypercast.cast_checked("2.250", INVARIANT, prices) == Checked(Decimal("2.25"))
    assert hypercast.cast_checked("2.5", INVARIANT, prices) == Violation(
        Keyword.ENUM, Decimal("2.5"), 0, 3
    )


def test_a_violation_carries_the_value_as_cast_never_rounded():
    """The value a violation carries is the door's, as it cast it: a rounding past what the
    type holds is a multiple_of violation of the parsed value, and at Door.TEXT the value is
    the judged text, after trim, never an entry."""
    ceiling = Constraints(Door.I8, multiple_of=5, rounding=Rounding.CEILING)
    assert hypercast.cast_checked("127", INVARIANT, ceiling) == Violation(
        Keyword.MULTIPLE_OF, 127, 0, 3
    )
    capped = Constraints(Door.I32, multiple_of=10, rounding=Rounding.MIDPOINT, maximum=100)
    # 106 rounds to 110, past the maximum: the violation carries 106, as cast.
    assert hypercast.cast_checked("106", INVARIANT, capped) == Violation(Keyword.MAXIMUM, 106, 0, 3)
    assert hypercast.cast_checked("104", INVARIANT, capped) == Checked(100, True)
    sex = Constraints(Door.TEXT, trim=True, enum=["Female", "Male"])
    verdict = hypercast.cast_text(" Femal ", sex)
    assert verdict == Violation(Keyword.ENUM, "Femal", 1, 5)
    assert isinstance(verdict, Violation) and type(verdict.value) is str
    assert hypercast.cast_text(b" Femal ", sex) == Violation(Keyword.ENUM, "Femal", 1, 5)
    assert hypercast.cast_checked_from_float(-0.5, Constraints(Door.F32, minimum=0)) == Violation(
        Keyword.MINIMUM, -0.5, 0, 0
    )


def test_case_sensitive_matches_an_enum_byte_for_byte():
    """A text enum folds case by default; case_sensitive compares byte for byte, and is a text
    extension, refused at a numeric door as trim is."""
    folded = Constraints(Door.TEXT, enum=["Male"])
    exact = Constraints(Door.TEXT, case_sensitive=True, enum=["Male"])
    assert folded.case_sensitive is False and exact.case_sensitive is True
    assert hypercast.cast_text("male", folded) == Checked("Male", False, 0)
    assert hypercast.cast_text("male", exact) == Violation(Keyword.ENUM, "male", 0, 4)
    assert hypercast.cast_text("Male", exact) == Checked("Male", False, 0)
    assert repr(exact) == "Constraints(Door.TEXT, case_sensitive=True, enum=('Male',))"
    assert exact != folded and hash(exact) != hash(folded)
    assert struct.unpack("=I", exact.packed[4:8])[0] == 256 | 512
    with pytest.raises(ValueError, match=re.escape(refusal_message(1))):
        Constraints(Door.I32, case_sensitive=True)


def test_a_declaration_reads_back_what_it_declares():
    """The getters, the repr (the call that builds it again), equality and hashing, and the
    104-byte layout the core reads, which a library with a C ABI of its own passes on."""
    latitude = Constraints(
        Door.F64, minimum="-90", maximum=90, multiple_of="1e-6", rounding=Rounding.MIDPOINT
    )
    assert latitude.door is Door.F64
    assert (latitude.minimum, latitude.maximum) == (Decimal(-90), Decimal(90))
    assert latitude.multiple_of.as_tuple() == Decimal("0.000001").as_tuple()
    assert latitude.rounding is Rounding.MIDPOINT
    assert latitude.exclusive_minimum is latitude.exclusive_maximum is None
    assert (latitude.min_length, latitude.max_length, latitude.trim) == (None, None, False)
    assert repr(latitude) == (
        "Constraints(Door.F64, minimum=Decimal('-90'), maximum=Decimal('90'), "
        "multiple_of=Decimal('0.000001'), rounding=Rounding.MIDPOINT)"
    )
    same = Constraints(
        Door.F64,
        minimum=Decimal(-90),
        maximum="90.0",
        multiple_of=Decimal("0.0000010"),
        rounding=Rounding.MIDPOINT,
    )
    assert latitude == same and hash(latitude) == hash(same)
    assert latitude != Constraints(Door.F32, minimum=-90, maximum=90, multiple_of="1e-6")
    size, flags, rounding, *_ = struct.unpack("=6I", latitude.packed[:24])
    assert (len(latitude.packed), size, flags, rounding) == (104, 104, 1 | 4 | 16, 1)
    # Each bound in its own slot: minimum 24, maximum 40, multiple_of 56, exclusive_minimum 72,
    # exclusive_maximum 88, each a 16-byte decimal.
    four = Constraints(
        Door.I32, minimum=1, maximum=2, multiple_of=3, exclusive_minimum=4, exclusive_maximum=5
    )
    slots = [struct.unpack("=QIBB2x", four.packed[at : at + 16]) for at in range(24, 104, 16)]
    assert [lo for lo, *_ in slots] == [1, 2, 3, 4, 5]
    assert four.exclusive_minimum == 4 and four.exclusive_maximum == 5
    sex = Constraints(Door.TEXT, trim=True, min_length=1, max_length=6, enum=("Female", "Male"))
    assert sex.enum == ("Female", "Male")
    assert sex.packed_enum == b"\x06\x00\x00\x00Female\x04\x00\x00\x00Male"
    assert repr(sex) == (
        "Constraints(Door.TEXT, min_length=1, max_length=6, trim=True, enum=('Female', 'Male'))"
    )
    with pytest.raises(AttributeError):
        sex.trim = False  # type: ignore[misc]


def test_a_length_is_an_int_never_a_bool():
    """``True`` is an int to Python, but a length of ``True`` is a caller's bug: a TypeError, as
    a bool bound is, at the constructor and at cast_constraints_check alike."""
    for keyword in ("min_length", "max_length"):
        for flag in (True, False):
            with pytest.raises(TypeError, match="a length must be an int, not bool"):
                Constraints(Door.TEXT, **{keyword: flag})
            with pytest.raises(TypeError, match="a length must be an int, not bool"):
                hypercast.cast_constraints_check(Door.TEXT, **{keyword: flag})
        assert getattr(Constraints(Door.TEXT, **{keyword: 1}), keyword) == 1
        for wrong in (1.0, "1"):
            with pytest.raises(TypeError):
                Constraints(Door.TEXT, **{keyword: wrong})
        with pytest.raises(OverflowError):
            Constraints(Door.TEXT, **{keyword: -1})


def test_a_number_in_a_text_enum_never_matches():
    """A number is a valid text entry that no string equals (JSON Schema's enum is exact
    equality across types): declared, packed empty, and every other entry keeps its index."""
    sex = Constraints(Door.TEXT, enum=[1, "One", Decimal("2.5"), "Two"])
    assert sex.packed_enum[:4] == b"\0\0\0\0"
    assert hypercast.cast_text("1", sex) == Violation(Keyword.ENUM, "1", 0, 1)
    assert hypercast.cast_text("2.5", sex) == Violation(Keyword.ENUM, "2.5", 0, 3)
    assert hypercast.cast_text("one", sex) == Checked("One", False, 1)
    assert hypercast.cast_text("TWO", sex) == Checked("Two", False, 3)
    assert hypercast.cast_text("x", Constraints(Door.TEXT, enum=[7])) == Violation(
        Keyword.ENUM, "x", 0, 1
    )


def test_an_enum_is_a_list_of_entries():
    """A text door's entries are str; a single str is not a list of its characters; a
    numeric door's entries are exact numbers, at any scale, and packed as decimals."""
    with pytest.raises(TypeError, match="not a single str or bytes"):
        Constraints(Door.TEXT, enum="abc")
    with pytest.raises(TypeError, match="entry 1 is float"):
        Constraints(Door.TEXT, enum=["a", 1.0])  # type: ignore[list-item]
    with pytest.raises(TypeError, match="entry 0 is bool"):
        Constraints(Door.TEXT, enum=[True])  # type: ignore[list-item]
    with pytest.raises(TypeError):
        Constraints(Door.I32, enum=[1.0])  # type: ignore[list-item]
    ints = Constraints(Door.I32, enum=[10, "20", Decimal("3E+1"), "40.0"])
    assert len(ints.packed_enum) == 4 * 16
    assert hypercast.cast_checked("40", INVARIANT, ints) == Checked(40)
    assert hypercast.cast_checked("41", INVARIANT, ints) == Violation(Keyword.ENUM, 41, 0, 2)
    with pytest.raises(ValueError, match=re.escape(refusal_message(8))):
        Constraints(Door.I32, enum=["1e-29"])
    reals = Constraints(Door.F64, enum=["0.5", 2])
    assert len(reals.packed_enum) == 2 * 16
    assert hypercast.cast_checked("0.50", INVARIANT, reals) == Checked(0.5)
    # None declares no enum; an empty list declares one no value is in.
    assert Constraints(Door.TEXT, enum=None) == Constraints(Door.TEXT)
    assert Constraints(Door.TEXT, enum=[]) != Constraints(Door.TEXT)
    # Without trim, an entry with edge white space is matchable, and declared as given.
    spaced = Constraints(Door.TEXT, enum=[" Male "])
    assert hypercast.cast_text(" MALE ", spaced) == Checked(" Male ", False, 0)
    assert struct.unpack("=I", spaced.packed[4:8])[0] == 256


def test_a_declaration_goes_to_its_own_door():
    """Text constraints are judged by cast_text and numeric ones by cast_checked; handing
    either to the other is a caller's bug."""
    text, number = Constraints(Door.TEXT), Constraints(Door.I32)
    with pytest.raises(ValueError, match="Door.TEXT are judged by cast_text"):
        hypercast.cast_checked("1", INVARIANT, text)  # type: ignore[type-var]
    with pytest.raises(ValueError, match="Door.TEXT are judged by cast_text"):
        hypercast.cast_checked_from_float(1.0, text)  # type: ignore[type-var]
    with pytest.raises(ValueError, match="Door.I32 are judged by cast_checked"):
        hypercast.cast_text("1", number)  # type: ignore[arg-type]
    for wrong in (None, "x", (Door.I32,)):
        with pytest.raises(TypeError):
            hypercast.cast_checked("1", INVARIANT, wrong)  # type: ignore[arg-type]


# --- the checked verdict ---------------------------------------------------------------------


def test_the_checked_types_hold_their_shape():
    """Checked and Violation match, compare and refuse mutation as Success and Fault do."""
    checked = Checked(42)
    violation = Violation(Keyword.MAXIMUM, 100, 0, 3)
    assert Checked.__match_args__ == ("value", "adjusted", "enum_index")
    assert Violation.__match_args__ == ("keyword", "value", "offset", "length")
    assert repr(checked) == "Checked(value=42, adjusted=False, enum_index=None)"
    assert repr(Checked("Male", False, 1)) == "Checked(value='Male', adjusted=False, enum_index=1)"
    assert repr(violation) == "Violation(keyword=Keyword.MAXIMUM, value=100, offset=0, length=3)"
    assert repr(Violation(Keyword.ENUM, "x", 0, 1)) == (
        "Violation(keyword=Keyword.ENUM, value='x', offset=0, length=1)"
    )
    assert checked == Checked(42, False, None) and checked != Checked(42, True)
    assert violation == Violation(Keyword(3), 100, 0, 3)
    assert violation != Violation(Keyword.MINIMUM, 100, 0, 3)
    assert violation != Violation(Keyword.MAXIMUM, 101, 0, 3)
    assert checked != Success(42) and violation != Fault(CastFailure.MALFORMED, 0, 3)
    # Each declared field refuses assignment. An undeclared name proves nothing: PyPy's cpyext
    # lets one be set on a native class.
    for verdict in (checked, violation):
        for field in type(verdict).__match_args__:
            with pytest.raises(AttributeError):
                setattr(verdict, field, 1)
        with pytest.raises(TypeError):
            hash(verdict)
    assert Checked[int].__origin__ is Checked and Violation[int].__origin__ is Violation
    assert hypercast.Rejection[int].__args__ == (Fault, Violation[int])
    assert hypercast.CheckedVerdict[int].__args__ == (Checked[int], Fault, Violation[int])


def describe(keyword: Keyword) -> str:
    """A match over keywords, with the default arm every one needs."""
    match keyword:
        case Keyword.MINIMUM | Keyword.MAXIMUM:
            return "out of bounds"
        case Keyword.ENUM:
            return "not one of the allowed values"
        case _:  # keywords are open
            return f"breaks {keyword.name}"


def test_keywords_are_open():
    """A keyword is its code and OpenAPI name; a code this binding does not name yet is kept,
    so a match needs its default arm."""
    names = [
        "minimum",
        "exclusiveMinimum",
        "maximum",
        "exclusiveMaximum",
        "multipleOf",
        "minLength",
        "maxLength",
        "enum",
    ]
    constants = [
        Keyword.MINIMUM,
        Keyword.EXCLUSIVE_MINIMUM,
        Keyword.MAXIMUM,
        Keyword.EXCLUSIVE_MAXIMUM,
        Keyword.MULTIPLE_OF,
        Keyword.MIN_LENGTH,
        Keyword.MAX_LENGTH,
        Keyword.ENUM,
    ]
    for code, (name, constant) in enumerate(zip(names, constants, strict=True), start=1):
        assert (constant.code, constant.name, str(constant)) == (code, name, name)
        assert Keyword(code) == constant and hash(Keyword(code)) == hash(constant)
    unknown = Keyword(9)
    assert (unknown.name, repr(unknown), unknown in constants) == ("keyword 9", "Keyword(9)", False)
    assert describe(Keyword.MAXIMUM) == "out of bounds"
    assert describe(Keyword.ENUM) == "not one of the allowed values"
    assert describe(unknown) == "breaks keyword 9"


# --- what the doors deliver ------------------------------------------------------------------


def test_the_readme_examples():
    """The README's three declarations, as it prints them."""
    latitude = Constraints(
        Door.F64, minimum="-90", maximum="90", multiple_of="0.000001", rounding=Rounding.MIDPOINT
    )
    euro_cash = Constraints(Door.DECIMAL, multiple_of=Decimal("0.05"))
    sex = Constraints(Door.TEXT, trim=True, enum=["Female", "Male", "Other"])
    cases = [
        (hypercast.cast_checked("45.1234567", INVARIANT, latitude), Checked(45.123457, True)),
        (
            hypercast.cast_checked("-90.5", INVARIANT, latitude),
            Violation(Keyword.MINIMUM, -90.5, 0, 5),
        ),
        (hypercast.cast_checked_from_float(45.1234567, latitude), Checked(45.123457, True)),
        (hypercast.cast_checked("€1.234,55", EURO, euro_cash), Checked(Decimal("1234.55"))),
        (
            hypercast.cast_checked("€12,37", EURO, euro_cash),
            Violation(Keyword.MULTIPLE_OF, Decimal("12.37"), 0, 6),
        ),
        (hypercast.cast_checked("12,3x", EURO, euro_cash), Fault(CastFailure.MALFORMED, 4, 1)),
        (hypercast.cast_text(" MALE ", sex), Checked("Male", False, 1)),
        (hypercast.cast_text("Femal", sex), Violation(Keyword.ENUM, "Femal", 0, 5)),
        (hypercast.cast_text("   ", sex), Fault(CastFailure.EMPTY, 0, 0)),
        (hypercast.cast_text(b" MALE "), Success(" MALE ")),
    ]
    for verdict, expected in cases:
        assert verdict == expected, verdict
    assert hypercast.optional(hypercast.cast_text("   ", sex)) is None


def test_a_text_enum_match_is_the_callers_own_entry():
    """A match delivers the declared entry itself, by index, not a copy decoded from the core;
    text with no enum is the caller's own str when nothing was trimmed."""
    entries = ["Female", "".join(["Ma", "le"]), "Other"]
    sex = Constraints(Door.TEXT, trim=True, enum=entries)
    verdict = hypercast.cast_text(" mALe ", sex)
    assert isinstance(verdict, Checked) and verdict.value is entries[1]
    assert verdict.enum_index == 1
    text = "".join(["un", "trimmed"])
    assert hypercast.cast_text(text).value is text  # type: ignore[union-attr]
    assert hypercast.cast_text(text, Constraints(Door.TEXT, max_length=9)).value is text  # type: ignore[union-attr]
    assert hypercast.cast_text(b"\xc3\xa9t\xc3\xa9").value == "été"  # type: ignore[union-attr]


def test_spans_count_code_points_for_str_and_bytes_for_bytes():
    """A violation's span is in the caller's own units, exactly as a fault's: the euro sign is
    one code point and three bytes, and a supplementary character one code point and four."""
    euro_cash = Constraints(Door.DECIMAL, multiple_of="0.05")
    cents = Decimal("12.37")
    assert hypercast.cast_checked("€12,37", EURO, euro_cash) == Violation(
        Keyword.MULTIPLE_OF, cents, 0, 6
    )
    assert hypercast.cast_checked("€12,37".encode(), EURO, euro_cash) == Violation(
        Keyword.MULTIPLE_OF, cents, 0, 8
    )
    short = Constraints(Door.TEXT, trim=True, max_length=1)
    # A thumbs-up and its skin-tone modifier, between a no-break space and a space.
    thumb = "\U0001f44d\U0001f3fd"
    assert hypercast.cast_text(" 👍🏽 ", short) == Violation(Keyword.MAX_LENGTH, thumb, 1, 2)
    assert hypercast.cast_text(" 👍🏽 ".encode(), short) == Violation(
        Keyword.MAX_LENGTH, thumb, 2, 8
    )
    assert hypercast.cast_checked("€12,3x", EURO, euro_cash) == Fault(CastFailure.MALFORMED, 5, 1)


def test_a_lone_surrogate_is_never_replaced():
    """A str can hold a lone surrogate, which has no UTF-8 form. It crosses with its surrogate
    kept, never as U+FFFD: as an enum entry the core refuses it as not UTF-8 (9), and as input
    it is MALFORMED at its own code point, at every new door."""
    # A high surrogate then a low one is still two code points in a str, never a pair.
    for entry in ("\ud800", "a\udfffb", chr(0xD83D) + chr(0xDE00)):
        with pytest.raises(ValueError, match=re.escape(refusal_message(9))):
            Constraints(Door.TEXT, enum=["x", entry])
        assert hypercast.cast_constraints_check(Door.TEXT, enum=[entry]) == 9
    # U+FFFD itself is an ordinary entry, and a surrogate is not it.
    replacement = Constraints(Door.TEXT, enum=["�"])
    assert hypercast.cast_text("�", replacement) == Checked("�", False, 0)
    assert hypercast.cast_text("\ud800", replacement) == Fault(CastFailure.MALFORMED, 0, 1)
    malformed = Fault(CastFailure.MALFORMED, 2, 1)
    assert hypercast.cast_text("é€\udc00x") == malformed
    assert hypercast.cast_text("é€\udc00x", Constraints(Door.TEXT, trim=True)) == malformed
    assert hypercast.cast_checked("€1\ud800", EURO, Constraints(Door.DECIMAL)) == malformed
    with pytest.raises(ValueError, match=re.escape(refusal_message(8))):
        Constraints(Door.DECIMAL, maximum="1\ud800")
    with pytest.raises(ValueError, match=re.escape(refusal_message(8))):
        hypercast.cast_declared_decimal("\udc001")


def test_a_lone_surrogate_is_malformed_at_every_plain_door_too():
    """The plain doors read a str exactly as the checked doors do: a lone surrogate is MALFORMED
    at its own code point, the very fault the checked door gives, never a UnicodeEncodeError."""
    i32 = Constraints(Door.I32)
    for text in ("\ud8005", "5\udfff", "12\ud800", "€1\ud8003", "é€\udc00x"):
        plain = hypercast.cast_i32(text, EURO)
        assert isinstance(plain, Fault) and plain.reason is CastFailure.MALFORMED, (text, plain)
        assert plain == hypercast.cast_checked(text, EURO, i32), text
    assert hypercast.cast_i32("\ud8005", EURO) == Fault(CastFailure.MALFORMED, 0, 1)
    assert hypercast.cast_decimal("€1\ud8003", EURO) == Fault(CastFailure.MALFORMED, 2, 1)
    assert hypercast.cast_f64("12\ud800", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 2, 1)
    every_plain_door = [
        hypercast.cast_bool("\ud800"),
        hypercast.cast_char("\ud800"),
        hypercast.cast_uuid("\ud800"),
        hypercast.cast_timestamp("\ud800"),
        hypercast.cast_time("\ud800"),
        hypercast.cast_duration("\ud800"),
        hypercast.cast_date("\ud800"),
    ]
    for verdict in every_plain_door:
        assert isinstance(verdict, Fault) and verdict.reason is CastFailure.MALFORMED, verdict


def test_optional_lets_a_violation_through():
    """optional folds only an EMPTY fault to None; a violation and any other fault pass."""
    sex = Constraints(Door.TEXT, trim=True, enum=["Female", "Male"])
    assert hypercast.optional(hypercast.cast_text("   ", sex)) is None
    assert hypercast.optional(hypercast.cast_text("x", sex)) == Violation(Keyword.ENUM, "x", 0, 1)
    assert hypercast.optional(hypercast.cast_text("\ud800", sex)) == Fault(
        CastFailure.MALFORMED, 0, 1
    )
    assert hypercast.optional(hypercast.cast_text("male", sex)) == Checked("Male", False, 1)


def test_zero_has_one_form():
    """No zero a checked door delivers carries a sign, whatever the input spelled."""
    nonnegative = Constraints(Door.F64, minimum=0)
    for verdict in (
        hypercast.cast_checked_from_float(-0.0, nonnegative),
        hypercast.cast_checked("-0.0", INVARIANT, nonnegative),
        hypercast.cast_checked(
            "-0.0000004",
            INVARIANT,
            Constraints(Door.F64, multiple_of="0.000001", rounding=Rounding.MIDPOINT),
        ),
    ):
        assert isinstance(verdict, Checked)
        assert struct.pack(">d", verdict.value) == bytes(8), verdict


def test_the_declared_decimal_conversion():
    """cast_declared_decimal is the one conversion every bound crosses: exact, trailing zeros
    dropped, its refusal the code-8 ValueError."""
    assert hypercast.cast_declared_decimal("1e-6").as_tuple() == Decimal("0.000001").as_tuple()
    assert hypercast.cast_declared_decimal(b"-1.50").as_tuple() == Decimal("-1.5").as_tuple()
    assert hypercast.cast_declared_decimal("-0").as_tuple() == Decimal("0").as_tuple()
    for refused in ("", " 1", "1e", "0x10", b"\xff"):
        with pytest.raises(ValueError, match=re.escape(refusal_message(8))):
            hypercast.cast_declared_decimal(refused)
    with pytest.raises(TypeError):
        hypercast.cast_declared_decimal(1)  # type: ignore[arg-type]


@pytest.mark.parametrize("text", [42, 4.2, None, bytearray(b"42"), memoryview(b"42")])
def test_a_checked_door_takes_str_or_bytes(text):
    """The checked text doors raise TypeError for anything but str or bytes, as every plain
    text door does, and the typed one for anything but a number."""
    with pytest.raises(TypeError):
        hypercast.cast_checked(text, INVARIANT, Constraints(Door.I32))
    with pytest.raises(TypeError):
        hypercast.cast_text(text, Constraints(Door.TEXT))
    with pytest.raises(TypeError):
        hypercast.cast_declared_decimal(text)
    if not isinstance(text, (int, float)):
        with pytest.raises(TypeError):
            hypercast.cast_checked_from_float(text, Constraints(Door.I32))
    assert hypercast.cast_checked_from_float(7, Constraints(Door.I32)) == Checked(7)


def test_the_constraint_types_have_docstrings():
    """Constraints, Checked, Violation and Keyword carry docstrings, and so do their members."""
    for cls in (Constraints, Checked, Violation, Keyword):
        assert cls.__doc__ and cls.__doc__.strip(), cls
    for member in ("door", "minimum", "multiple_of", "rounding", "enum", "packed", "packed_enum"):
        doc = getattr(Constraints, member).__doc__
        assert doc and doc.strip(), member
    for member in ("code", "name"):
        assert getattr(Keyword, member).__doc__, member


def test_door_23_is_no_checked_door():
    """23 is the core's character door, which no declaration can be bound to: a ValueError at
    construction and at the refusal-code probe, whatever else is declared, never a quiet 'no
    rules'. The doors a declaration can name are the numeric ones and text."""
    for door in (23, 24, 0, 1, 12, 20, 21, 22):
        with pytest.raises(ValueError, match="door must be a Door"):
            Constraints(door)  # type: ignore[arg-type]
        with pytest.raises(ValueError, match="door must be a Door"):
            hypercast.cast_constraints_check(door)  # type: ignore[arg-type]
    with pytest.raises(ValueError, match="door must be a Door"):
        Constraints(door=23, min_length=1)  # type: ignore[arg-type]
    with pytest.raises(ValueError, match="door must be a Door"):
        hypercast.cast_constraints_check(door=23, trim=True)  # type: ignore[arg-type]
    with pytest.raises(ValueError):
        Door(23)
    assert 23 not in {member.value for member in Door}
