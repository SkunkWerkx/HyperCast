"""Date options and month names: opt-in shapes for the date doors. A door given none reads exactly
what it always has; the corpus rows that declare options are replayed in test_corpus.py."""

from __future__ import annotations

import datetime as dt

import pytest

import hypercast
from hypercast import (
    CastFailure,
    DateOptions,
    DateOrder,
    Detail,
    Fault,
    MonthNames,
    Success,
)

DMY, MDY, YMD = DateOrder.DAY_MONTH_YEAR, DateOrder.MONTH_DAY_YEAR, DateOrder.YEAR_MONTH_DAY

ENGLISH = [
    (1, "Jan"),
    (2, "Feb"),
    (3, "Mar"),
    (4, "Apr"),
    (5, "May"),
    (6, "Jun"),
    (7, "Jul"),
    (8, "Aug"),
    (9, "Sep"),
    (9, "Sept"),
    (10, "Oct"),
    (11, "Nov"),
    (12, "Dec"),
]
FRENCH = [
    (1, "janv."),
    (2, "févr."),
    (3, "mars"),
    (4, "avr."),
    (5, "mai"),
    (6, "juin"),
    (7, "juil."),
    (8, "août"),
    (9, "sept."),
    (10, "oct."),
    (11, "nov."),
    (12, "déc."),
]


def _only(verdict) -> object:
    assert isinstance(verdict, Success), verdict
    return verdict.value


# --- DateOptions ------------------------------------------------------------------------------


def test_the_options_are_flags_a_pivot_and_a_profile():
    """The three flags, ``COMMON_FORMS`` as all of them with no pivot, and the pivot beside."""
    assert (DateOptions.COMPACT_DATE, DateOptions.SPACE_SEPARATOR, DateOptions.HOUR_OFFSET) == (
        1,
        2,
        4,
    )
    common = DateOptions.COMMON_FORMS
    assert (common.flags, common.pivot) == (7, 0)
    assert isinstance(common, DateOptions)
    none = DateOptions()
    assert (none.flags, none.pivot) == (0, 0)
    both = DateOptions(DateOptions.COMPACT_DATE, 2029)
    assert (both.flags, both.pivot) == (1, 2029)
    assert DateOptions(flags=2, pivot=100) == DateOptions(2, 100)
    assert common.with_pivot(2029) == DateOptions(7, 2029)
    assert common.with_pivot(2029).with_pivot(0) == common
    assert hash(DateOptions(1, 2029)) == hash(DateOptions(1, 2029))
    assert DateOptions(1) != DateOptions(2) and DateOptions(1) != 1
    assert repr(DateOptions(5)) == "DateOptions(flags=5)"
    assert repr(DateOptions(5, 2029)) == "DateOptions(flags=5, pivot=2029)"
    assert eval(repr(DateOptions(5, 2029)), {"DateOptions": DateOptions}) == DateOptions(5, 2029)  # noqa: S307


def test_a_declaration_the_core_refuses_is_a_value_error():
    """An option bit this version does not define, and a pivot outside 100 to 9999 (0 is none)."""
    for flags in (8, 1 << 31, 7 | 8):
        with pytest.raises(ValueError, match="flags set a bit this version does not define"):
            DateOptions(flags)
    for pivot in (1, 99, 10000, 65535, 65536, 2**32 - 1):
        with pytest.raises(ValueError, match=r"pivot must be 0 \(none\) or 100 to 9999"):
            DateOptions(0, pivot)
        with pytest.raises(ValueError, match="pivot must be"):
            DateOptions.COMMON_FORMS.with_pivot(pivot)
    for pivot in (0, 100, 9999):
        assert DateOptions(0, pivot).pivot == pivot
    # An int no 32 unsigned bits hold is a declaration the core could not read, a ValueError
    # that names the field, not the OverflowError of extraction.
    for wide in (-1, 2**32, 2**70, -(2**70)):
        with pytest.raises(
            ValueError, match="Date option flags must be an int in 32 unsigned bits"
        ):
            DateOptions(wide)
        with pytest.raises(ValueError, match="Date pivot must be an int in 32 unsigned bits"):
            DateOptions(0, wide)
        with pytest.raises(ValueError, match="Date pivot must be an int in 32 unsigned bits"):
            DateOptions.COMMON_FORMS.with_pivot(wide)
    with pytest.raises(TypeError):
        DateOptions("1")  # type: ignore[arg-type]


def test_each_flag_opens_its_one_shape_and_nothing_else_does():
    """Compact dates at ``cast_date``, a space separator and an hour offset at
    ``cast_timestamp``; each is refused without its flag."""
    assert isinstance(hypercast.cast_date("20260107"), Fault)
    compact = DateOptions(DateOptions.COMPACT_DATE)
    assert _only(hypercast.cast_date("20260107", options=compact)) == dt.date(2026, 1, 7)
    assert isinstance(hypercast.cast_date("20260107", options=DateOptions(2 | 4)), Fault)
    plain = "2026-01-07T03:04:05Z"
    spaced = "2026-01-07 03:04:05Z"
    hour = "2026-01-07T03:04:05+05"
    utc = dt.timezone.utc
    assert _only(hypercast.cast_timestamp(plain)) == dt.datetime(2026, 1, 7, 3, 4, 5, tzinfo=utc)
    assert isinstance(hypercast.cast_timestamp(spaced), Fault)
    assert isinstance(hypercast.cast_timestamp(hour), Fault)
    space = DateOptions(DateOptions.SPACE_SEPARATOR)
    offset = DateOptions(DateOptions.HOUR_OFFSET)
    assert _only(hypercast.cast_timestamp(spaced, options=space)) == _only(
        hypercast.cast_timestamp(plain)
    )
    assert isinstance(hypercast.cast_timestamp(hour, options=space), Fault)
    assert _only(hypercast.cast_timestamp(hour, options=offset)) == dt.datetime(
        2026, 1, 6, 22, 4, 5, tzinfo=utc
    )
    assert isinstance(hypercast.cast_timestamp(spaced, options=offset), Fault)


def test_common_forms_works_at_every_date_door_and_asks_nothing_of_the_rest():
    """The profile opens the three shapes together, and at a door that reads none of them it is
    ignored, never refused."""
    common = DateOptions.COMMON_FORMS
    assert _only(hypercast.cast_date("20260107", options=common)) == dt.date(2026, 1, 7)
    assert _only(hypercast.cast_timestamp("2026-01-07 03:04:05+05", options=common)) == dt.datetime(
        2026, 1, 6, 22, 4, 5, tzinfo=dt.timezone.utc
    )
    assert _only(hypercast.cast_date("7/1/2026", DMY, options=common)) == dt.date(2026, 1, 7)
    assert _only(hypercast.cast_datetime("7/1/2026 3:04 PM", DMY, options=common)) == dt.datetime(
        2026, 1, 7, 15, 4
    )
    # Compact is not an ordered shape, and the profile supplies no pivot.
    verdict = hypercast.cast_date("20260107", DMY, options=common)
    assert isinstance(verdict, Fault) and verdict.detail is Detail.FIELD_WIDTH
    verdict = hypercast.cast_date("7/1/26", DMY, options=common)
    assert isinstance(verdict, Fault) and verdict.detail is Detail.FIELD_WIDTH


def test_a_pivot_reads_a_two_digit_year_in_its_window():
    """The one year in ``[pivot - 99, pivot]`` ending in the two digits; four-digit years are
    unaffected; without a pivot a two-digit year is a width fault."""
    pivot = DateOptions(0, 2029)
    assert _only(hypercast.cast_date("7/1/26", DMY, options=pivot)) == dt.date(2026, 1, 7)
    assert _only(hypercast.cast_date("7/1/29", DMY, options=pivot)) == dt.date(2029, 1, 7)
    assert _only(hypercast.cast_date("7/1/30", DMY, options=pivot)) == dt.date(1930, 1, 7)
    assert _only(hypercast.cast_date("7/1/2026", DMY, options=pivot)) == dt.date(2026, 1, 7)
    assert _only(hypercast.cast_date("26/1/7", YMD, options=pivot)) == dt.date(2026, 1, 7)
    assert _only(hypercast.cast_datetime("7/1/26 3:04", DMY, options=pivot)) == dt.datetime(
        2026, 1, 7, 3, 4
    )
    # The window's edges: pivot 100 reads 00 as year 100 and 01 as year 1; 9999 reads 99 as 9999.
    low, high = DateOptions(0, 100), DateOptions(0, 9999)
    assert _only(hypercast.cast_date("1/1/00", MDY, options=low)) == dt.date(100, 1, 1)
    assert _only(hypercast.cast_date("1/1/01", MDY, options=low)) == dt.date(1, 1, 1)
    assert _only(hypercast.cast_date("1/1/99", MDY, options=high)) == dt.date(9999, 1, 1)
    assert _only(hypercast.cast_date("1/1/00", MDY, options=high)) == dt.date(9900, 1, 1)
    verdict = hypercast.cast_date("7/1/26", DMY)
    assert verdict == Fault(CastFailure.MALFORMED, 4, 2)
    assert verdict.detail is Detail.FIELD_WIDTH


# --- MonthNames -------------------------------------------------------------------------------


def test_a_vocabulary_is_pairs_and_packs_the_specs_layout():
    """Each entry is a month byte, a little-endian ``u32`` length, then the UTF-8; the packed
    bytes read back, and entries keep their order."""
    names = MonthNames(ENGLISH)
    expected = b"".join(
        bytes([month]) + len(name.encode()).to_bytes(4, "little") + name.encode()
        for month, name in ENGLISH
    )
    assert names.packed == expected
    assert names.entries == tuple(ENGLISH)
    assert MonthNames.from_packed(expected) == names
    assert MonthNames(iter(ENGLISH)) == names
    assert MonthNames(tuple(ENGLISH)) == names
    assert hash(MonthNames(ENGLISH)) == hash(names)
    assert eval(repr(names), {"MonthNames": MonthNames}) == names  # noqa: S307
    assert MonthNames([[m, n] for m, n in ENGLISH]) == names


def test_a_vocabulary_that_breaks_a_rule_is_a_value_error_naming_it():
    """The core's rules, one message each, through both constructors."""
    cases = [
        ([(m, n) for m, n in ENGLISH if m != 12], "has no name"),
        (ENGLISH + [(0, "x")], "outside 1 to 12"),
        (ENGLISH + [(13, "x")], "outside 1 to 12"),
        (ENGLISH + [(300, "x")], "outside 1 to 12"),
        (ENGLISH + [(-1, "x")], "outside 1 to 12"),
        (ENGLISH + [(2**70, "x")], "outside 1 to 12"),
        (ENGLISH + [(-(2**70), "x")], "outside 1 to 12"),
        ([(1, "")] + ENGLISH[1:], "empty or longer than 64 bytes"),
        (ENGLISH + [(1, "x" * 65)], "empty or longer than 64 bytes"),
        (ENGLISH + [(1, "Ja1")], "ASCII digit, a slash"),
        (ENGLISH + [(1, "J/n")], "ASCII digit, a slash"),
        (ENGLISH + [(1, "J-n")], "ASCII digit, a slash"),
        (ENGLISH + [(1, "J n")], "ASCII digit, a slash"),
        (ENGLISH + [(1, "J\u00a0n")], "ASCII digit, a slash"),
        (ENGLISH + [(2, "JAN")], "fold to the same spelling"),
        (ENGLISH + [(1, "jan")], "fold to the same spelling"),
        (ENGLISH + [(1, "J\ud800n")], "is not UTF-8"),
        ([], "has no name"),
    ]
    for entries, phrase in cases:
        with pytest.raises(ValueError, match=phrase):
            MonthNames(entries)
    for packed, phrase in (
        (MonthNames(ENGLISH).packed + b"\x01", "cut short"),
        (MonthNames(ENGLISH).packed + b"\x01\x05\x00\x00\x00ab", "cut short"),
        (MonthNames(ENGLISH).packed + b"\x01\x02\x00\x00\x00\xc3\x28", "is not UTF-8"),
        (b"", "has no name"),
    ):
        with pytest.raises(ValueError, match=phrase):
            MonthNames.from_packed(packed)
    # A period is fine anywhere, and so is a 64-byte name.
    MonthNames(ENGLISH + [(1, "J.a.n."), (2, "x" * 64)])


def test_the_wrong_kinds_of_argument_are_type_errors():
    """A vocabulary is an iterable of pairs; a str, bytes, a bad pair or a bad name is a caller's
    TypeError, as are options and months of the wrong type at a door."""
    for entries in ("Jan", b"Jan", 3, None):
        with pytest.raises(TypeError):
            MonthNames(entries)  # type: ignore[arg-type]
    for entries in ([(1,)], [1], [("1", "Jan")], [(1, b"Jan")], [(1.5, "Jan")]):
        with pytest.raises(TypeError, match="pairs"):
            MonthNames(entries)  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        MonthNames.from_packed("abc")  # type: ignore[arg-type]
    for wrong in (1, "compact", object(), MonthNames(ENGLISH)):
        with pytest.raises(TypeError):
            hypercast.cast_date("20260107", options=wrong)  # type: ignore[arg-type]
        with pytest.raises(TypeError):
            hypercast.cast_timestamp("2026-01-07T03:04:05Z", options=wrong)  # type: ignore[arg-type]
    for wrong in (1, ENGLISH, DateOptions(), "Jan"):
        with pytest.raises(TypeError):
            hypercast.cast_date("7-Jan-2026", DMY, months=wrong)  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        hypercast.cast_timestamp("2026-01-07T03:04:05Z", months=MonthNames(ENGLISH))  # type: ignore[call-arg]


def test_a_month_name_replaces_the_numeric_month_where_the_order_puts_it():
    """Excel style: the declared name in the declared position, with the repeated separator;
    a pivot reads a two-digit year beside it; names fold case; a vocabulary never overrides
    the declared order."""
    names = MonthNames(ENGLISH)
    pivot = DateOptions(0, 2029)
    assert _only(hypercast.cast_date("7-Jan-2026", DMY, months=names)) == dt.date(2026, 1, 7)
    assert _only(hypercast.cast_date("07-Jan-26", DMY, options=pivot, months=names)) == dt.date(
        2026, 1, 7
    )
    assert _only(hypercast.cast_date("Jan-07-2026", MDY, months=names)) == dt.date(2026, 1, 7)
    assert _only(hypercast.cast_date("2026/Jan/07", YMD, months=names)) == dt.date(2026, 1, 7)
    assert _only(hypercast.cast_date("7-SEPT-2026", DMY, months=names)) == dt.date(2026, 9, 7)
    assert _only(hypercast.cast_date("7-sep-2026", DMY, months=names)) == dt.date(2026, 9, 7)
    assert _only(hypercast.cast_datetime("7-Jan-26 3:04 PM", DMY, options=pivot, months=names)) == (
        dt.datetime(2026, 1, 7, 15, 4)
    )
    # Without a pivot a two-digit year is still a width fault; without the vocabulary the name
    # is not recognised; the order is never guessed from a name.
    verdict = hypercast.cast_date("07-Jan-26", DMY, months=names)
    assert isinstance(verdict, Fault) and verdict.detail is Detail.FIELD_WIDTH
    verdict = hypercast.cast_date("7-Jan-2026", DMY)
    assert isinstance(verdict, Fault)
    verdict = hypercast.cast_date("7-Foo-2026", DMY, months=names)
    assert isinstance(verdict, Fault) and verdict.detail is Detail.NOT_RECOGNIZED
    assert isinstance(hypercast.cast_date("Jan-07-2026", DMY, months=names), Fault)
    # The numeric forms are untouched by a vocabulary, and one that reads no month field
    # (ISO, no order) is not looked at.
    assert _only(hypercast.cast_date("7/1/2026", DMY, months=names)) == dt.date(2026, 1, 7)
    assert _only(hypercast.cast_date("2026-01-07", months=names)) == dt.date(2026, 1, 7)


def test_dotted_names_a_separator_and_a_pivot_together():
    """With ``janv.`` declared, pivot 2029 and DMY, ``7.janv..26`` reads as 2026-01-07 and
    ``7.janv.26`` is malformed: punctuation is part of the name, never stripped."""
    names = MonthNames(FRENCH)
    pivot = DateOptions(0, 2029)
    assert _only(hypercast.cast_date("7.janv..26", DMY, options=pivot, months=names)) == dt.date(
        2026, 1, 7
    )
    assert _only(hypercast.cast_date("7.janv..2026", DMY, months=names)) == dt.date(2026, 1, 7)
    verdict = hypercast.cast_date("7.janv.26", DMY, options=pivot, months=names)
    assert isinstance(verdict, Fault) and verdict.reason is CastFailure.MALFORMED
    assert _only(hypercast.cast_date("7 mars 2026".replace(" ", "-"), DMY, months=names)) == (
        dt.date(2026, 3, 7)
    )
    assert _only(hypercast.cast_date("7-FÉVR.-2026", DMY, months=names)) == dt.date(2026, 2, 7)


def test_a_vocabulary_alone_opens_names_and_nothing_else():
    """No options at all: names read, and no pivot, flag or other shape comes with them."""
    names = MonthNames(ENGLISH)
    assert _only(hypercast.cast_date("7-Jan-2026", DMY, months=names)) == dt.date(2026, 1, 7)
    assert isinstance(hypercast.cast_date("7-Jan-26", DMY, months=names), Fault)
    assert isinstance(hypercast.cast_date("20260107", months=names), Fault)


def test_the_options_keywords_are_optional_and_default_to_none():
    """The 1.0 calls are the plain doors: no options, same verdicts, ``None`` accepted."""
    assert hypercast.cast_date("2026-01-07") == hypercast.cast_date(
        "2026-01-07", None, options=None, months=None
    )
    assert hypercast.cast_timestamp("2026-01-07T03:04:05Z") == hypercast.cast_timestamp(
        "2026-01-07T03:04:05Z", options=None
    )
    assert hypercast.cast_datetime("7/1/2026 3:04", DMY) == hypercast.cast_datetime(
        "7/1/2026 3:04", DMY, am="", pm="", options=None, months=None
    )
    # Day periods and options ride together.
    verdict = hypercast.cast_datetime(
        "7-Jan-26 3:04 nachm.",
        DMY,
        pm="nachm.",
        options=DateOptions(0, 2029),
        months=MonthNames(ENGLISH),
    )
    assert _only(verdict) == dt.datetime(2026, 1, 7, 15, 4)
