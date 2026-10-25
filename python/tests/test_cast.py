"""Binding-level behavior the corpus can't express: match/case consumption, str-door
transcoding, localeconv bridging, datetime fidelity mapping (microsecond truncation —
Python's honest ceiling), the unbounded-int u64 story, and the caller-bug guards."""

from __future__ import annotations

import datetime as dt
import importlib.metadata
import re
import uuid as uuidlib
from decimal import Decimal
from pathlib import Path

import pytest

import hypercast
from hypercast import CastFailure, Fault, NumFormat, Success, UnixPrecision


def test_match_case_consumes_the_union():
    """A verdict destructures with match/case through __match_args__."""
    match hypercast.cast_i32("42", NumFormat.INVARIANT):
        case Success(value):
            assert value == 42
        case Fault():
            pytest.fail("42 should parse")


def test_fault_span_points_at_the_offending_byte():
    """A fault's span names the offending byte, offset into the untrimmed input."""
    assert hypercast.cast_i32("  12x4", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 4, 1)


def test_str_doors_transcode_non_ascii_separators():
    """A str door reads a declared non-ASCII separator after transcoding to UTF-8."""
    french = NumFormat(",", " ", NumFormat.ALL)
    assert hypercast.cast_f64("1 234,5", french) == Success(1234.5)


def test_fault_spans_come_back_in_the_callers_own_units():
    """The core reports byte spans into the UTF-8 it saw; a str caller's unit is the code point, so
    the same text faults at the same place in either unit and slices back out.
    """
    assert hypercast.cast_i32("€x", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 0, 1)
    assert hypercast.cast_i32("€x".encode(), NumFormat.INVARIANT) == Fault(
        CastFailure.MALFORMED, 0, 3
    )
    assert hypercast.cast_i32("1€", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 1, 1)
    assert hypercast.cast_i32("1€".encode(), NumFormat.INVARIANT) == Fault(
        CastFailure.MALFORMED, 1, 3
    )
    text = "12€4"
    for passed in (text, text.encode()):
        match hypercast.cast_f64(passed, NumFormat.INVARIANT):
            case Fault(_, offset, length):
                assert passed[offset : offset + length] == (
                    "€" if isinstance(passed, str) else "€".encode()
                )
            case other:
                raise AssertionError(f"{passed!r} parsed: {other!r}")
    # A non-ASCII input that succeeds, and an ASCII fault, are untouched by the mapping.
    assert hypercast.cast_f64("1\u00a0234,5", NumFormat(",", "\u00a0", NumFormat.ALL)) == Success(
        1234.5
    )
    assert hypercast.cast_i32("  12x4", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 4, 1)


def test_localeconv_bridge():
    """NumFormat.from_localeconv takes the separators a locale declares."""
    fmt = NumFormat.from_localeconv({"decimal_point": ",", "thousands_sep": "."})
    assert fmt.currency == ""
    assert hypercast.cast_f64("1.234,5", fmt) == Success(1234.5)
    euro = NumFormat.from_localeconv(
        {"decimal_point": ",", "thousands_sep": ".", "currency_symbol": "€"}
    )
    assert euro.currency == "€"
    assert hypercast.cast_f64("1.234,5 €", euro) == Success(1234.5)
    # The C locale's shape: an empty thousands separator falls back to ",".
    c_locale = NumFormat.from_localeconv({"decimal_point": ".", "thousands_sep": ""})
    assert (c_locale.decimal_sep, c_locale.group_sep) == (".", ",")
    # Nothing past the separators and the symbol declared: the rest reads as the invariant.
    assert (c_locale.minus_sign, c_locale.plus_sign, c_locale.percent, c_locale.digit_zero) == (
        "",
        "",
        "",
        "",
    )
    assert (c_locale.currency_decimal_sep, c_locale.currency_group_sep) == ("", "")
    assert c_locale.flags == NumFormat.ALL


def test_localeconv_bridge_carries_the_signs_the_currency_separators_and_the_sign_position():
    """Every field localeconv() has a column for crosses: negative_sign and positive_sign as further
    spellings beside ASCII (Estonian's U+2212), mon_decimal_point and mon_thousands_sep as the pair
    a currency amount is read with, and an n_sign_posn of 2 or 4 — the sign after the amount — as
    TRAILING_SIGN. localeconv() has no percent symbol and no digits, so those stay undeclared.
    """
    estonian = NumFormat.from_localeconv(
        {
            "decimal_point": ",",
            "thousands_sep": "\u00a0",
            "currency_symbol": "€",
            "negative_sign": "\u2212",
            "positive_sign": "",
            "n_sign_posn": 1,
        }
    )
    assert (estonian.minus_sign, estonian.plus_sign) == ("\u2212", "")
    assert hypercast.cast_f64("\u22121\u00a0234,5", estonian) == Success(-1234.5)
    assert hypercast.cast_f64("-1\u00a0234,5", estonian) == Success(-1234.5)
    assert not estonian.flags & NumFormat.TRAILING_SIGN
    # A locale whose currency separators differ from its number separators (de_CH's shape:
    # apostrophe groups, but a comma decimal for money) reads a currency amount with the pair
    # it declares for one, and a plain number with the other.
    swiss = NumFormat.from_localeconv(
        {
            "decimal_point": ".",
            "thousands_sep": "'",
            "currency_symbol": "CHF",
            "mon_decimal_point": ",",
            "mon_thousands_sep": "'",
            "negative_sign": "-",
            "positive_sign": "+",
            "n_sign_posn": 2,
        }
    )
    assert (swiss.currency_decimal_sep, swiss.currency_group_sep) == (",", "'")
    assert hypercast.cast_decimal("CHF 1'234,50", swiss) == Success(Decimal("1234.5"))
    assert hypercast.cast_decimal("1'234.50", swiss) == Success(Decimal("1234.5"))
    # ASCII signs are read whatever is declared, so declaring them again says nothing.
    assert (swiss.minus_sign, swiss.plus_sign) == ("", "")
    # n_sign_posn 2 (and 4) put the sign after the amount: TRAILING_SIGN joins the flags.
    assert swiss.flags == NumFormat.ALL | NumFormat.TRAILING_SIGN
    assert hypercast.cast_decimal("1'234,50 CHF-", swiss) == Success(Decimal("-1234.5"))
    assert NumFormat.from_localeconv({"n_sign_posn": 4}).flags & NumFormat.TRAILING_SIGN
    for not_trailing in (0, 1, 3, 127, "2", None):
        assert not NumFormat.from_localeconv({"n_sign_posn": not_trailing}).flags & (
            NumFormat.TRAILING_SIGN
        )
    # The process's own locale, whatever it is, is a format: the C locale in CI.
    assert isinstance(NumFormat.from_localeconv(), NumFormat)


def test_localeconv_bridge_never_puts_one_character_in_both_roles():
    """An empty field takes its invariant default unless the other separator already holds that
    character, in which case it takes the other of the pair, so a comma-decimal locale with no
    thousands separator does not fall back to "," for the group too — a format no door can read.
    The PHP binding's bridge holds the same rule.
    """
    comma = NumFormat.from_localeconv({"decimal_point": ",", "thousands_sep": ""})
    assert (comma.decimal_sep, comma.group_sep) == (",", ".")
    assert hypercast.cast_f64("1.234,5", comma) == Success(1234.5)
    dotted_groups = NumFormat.from_localeconv({"decimal_point": "", "thousands_sep": "."})
    assert (dotted_groups.decimal_sep, dotted_groups.group_sep) == (",", ".")
    nothing = NumFormat.from_localeconv({})
    assert (nothing.decimal_sep, nothing.group_sep) == (".", ",")
    # Two separators the locale itself declares equal are still the constructor's caller bug.
    with pytest.raises(ValueError, match="must differ"):
        NumFormat.from_localeconv({"decimal_point": ",", "thousands_sep": ","})
    # The currency pair by the same rule, defaulting to the number pair: a monetary decimal
    # that is the number group separator takes the number decimal as its group, never itself.
    money = NumFormat.from_localeconv(
        {"decimal_point": ".", "thousands_sep": ",", "mon_decimal_point": ","}
    )
    assert (money.currency_decimal_sep, money.currency_group_sep) == (",", ".")
    groups = NumFormat.from_localeconv(
        {"decimal_point": ".", "thousands_sep": ",", "mon_thousands_sep": "."}
    )
    assert (groups.currency_decimal_sep, groups.currency_group_sep) == (",", ".")
    with pytest.raises(ValueError, match="Currency decimal and group separators must differ"):
        NumFormat.from_localeconv({"mon_decimal_point": ".", "mon_thousands_sep": "."})


def test_currency_symbol_is_declared_never_guessed():
    """A currency symbol is honored only when declared and its flag is set."""
    dollars = NumFormat(".", ",", NumFormat.ALL, "$")
    assert dollars.currency == "$"
    assert NumFormat.ALL & NumFormat.CURRENCY
    assert hypercast.cast_i32("$1,234", dollars) == Success(1234)
    assert hypercast.cast_i32("-$5", dollars) == Success(-5)
    assert hypercast.cast_i32("$ -5", dollars) == Success(-5)
    assert hypercast.cast_i32("($5)", dollars) == Success(-5)
    assert hypercast.cast_f64("$2.50", dollars) == Success(2.5)
    krone = NumFormat(",", ".", NumFormat.ALL, currency="kr.")
    assert hypercast.cast_decimal("1.234,50 kr.", krone) == Success(Decimal("1234.5"))
    # Declared but with the lenience off: the symbol is simply the first offending byte.
    off = NumFormat(".", ",", NumFormat.ALL & ~NumFormat.CURRENCY, "$")
    assert hypercast.cast_i32("$5", off) == Fault(CastFailure.MALFORMED, 0, 1)
    # Nothing declared (INVARIANT declares no symbol): the flag matches nothing.
    assert NumFormat.INVARIANT.currency == ""
    assert hypercast.cast_i32("$5", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 0, 1)


def test_currency_symbol_rules_are_a_caller_bug():
    """1 to 16 UTF-8 bytes, no ASCII digit or whitespace — the core's CurrencySymbol rule, enforced
    at construction like equal separators.
    """
    for bad in ("$5", "US D", "x" * 17, "€" * 6):
        with pytest.raises(ValueError):
            NumFormat(".", ",", NumFormat.ALL, bad)
    assert NumFormat(".", ",", NumFormat.ALL, "€" * 5).currency == "€" * 5  # 15 bytes


def test_decimal_is_exact_and_canonical():
    """The decimal door never forms a float and trims exact trailing zeros."""
    assert hypercast.cast_decimal("0.1", NumFormat.INVARIANT) == Success(Decimal("0.1"))
    verdict = hypercast.cast_decimal("1.10", NumFormat.INVARIANT)
    assert isinstance(verdict.value, Decimal)
    assert verdict == Success(Decimal("1.1"))
    # Equality would also accept Decimal("1.10"); the tuple pins the canonical scale: exact
    # trailing zeros in the fraction are trimmed, so "1.10", "1.1" and "1.1000" are one value.
    assert verdict.value.as_tuple() == (0, (1, 1), -1)
    for text in ("1.1", "1.1000"):
        assert hypercast.cast_decimal(text, NumFormat.INVARIANT).value.as_tuple() == (0, (1, 1), -1)
    assert hypercast.cast_decimal("50%", NumFormat.INVARIANT).value.as_tuple() == (0, (5,), -1)
    assert hypercast.cast_decimal("100", NumFormat.INVARIANT).value.as_tuple() == (0, (1, 0, 0), 0)
    # Zero is scale 0 and never negative.
    assert hypercast.cast_decimal("-0.00", NumFormat.INVARIANT).value.as_tuple() == (0, (0,), 0)
    # 96 bits of magnitude, exactly: one past the ceiling is out of range, never rounded.
    top = 2**96 - 1
    assert hypercast.cast_decimal(str(top), NumFormat.INVARIANT) == Success(Decimal(top))
    assert hypercast.cast_decimal(str(top + 1), NumFormat.INVARIANT) == Fault(
        CastFailure.OUT_OF_RANGE, 0, len(str(top + 1))
    )


def _expected_version() -> str:
    # The one version source is rust/Cargo.toml: maturin bakes it into the package metadata,
    # and the crate bakes it into hypercast_version — so the expectation is derived, never a
    # literal that goes stale on the next bump. The metadata is absent when the suite runs
    # off the source tree with no install, so fall back to the manifest itself.
    try:
        return importlib.metadata.version("hypercast")
    except importlib.metadata.PackageNotFoundError:
        pass
    for parent in Path(__file__).resolve().parents:
        manifest = parent / "rust" / "Cargo.toml"
        if manifest.is_file():
            found = re.search(
                r'^version\s*=\s*"([^"]+)"', manifest.read_text(encoding="utf-8"), re.MULTILINE
            )
            assert found, f"no version in {manifest}"
            return found.group(1)
    raise FileNotFoundError("rust/Cargo.toml not found")


def test_native_version_names_the_loaded_core():
    """native_version reports the core this package was built from."""
    assert hypercast.native_version() == _expected_version()


def test_uuid_agrees_with_the_platforms_own_parser():
    """The UUID door's value equals uuid.UUID's own for the same text."""
    text = "01020304-0506-0708-090a-0b0c0d0e0f10"
    assert hypercast.cast_uuid(text) == Success(uuidlib.UUID(text))
    assert hypercast.cast_uuid(f"urn:uuid:{text}") == Success(uuidlib.UUID(text))


def test_timestamp_is_aware_utc_with_microsecond_truncation():
    """An instant comes back aware and in UTC, nanoseconds truncated to microseconds."""
    verdict = hypercast.cast_timestamp("2026-01-02T15:04:05.123456789Z")
    expected = dt.datetime(2026, 1, 2, 15, 4, 5, 123456, tzinfo=dt.timezone.utc)
    assert verdict == Success(expected)
    # Offset input normalizes to UTC.
    assert hypercast.cast_timestamp("2026-01-02T15:04:05+05:00") == Success(
        dt.datetime(2026, 1, 2, 10, 4, 5, tzinfo=dt.timezone.utc)
    )
    # The full protobuf window survives timedelta arithmetic.
    assert hypercast.cast_timestamp("0001-01-01T00:00:00Z") == Success(
        dt.datetime(1, 1, 1, tzinfo=dt.timezone.utc)
    )


def test_unix_maps_the_declared_precision():
    """The Unix door reads the value in the unit the caller declared."""
    assert hypercast.cast_unix("-1", UnixPrecision.SECONDS) == Success(
        dt.datetime(1969, 12, 31, 23, 59, 59, tzinfo=dt.timezone.utc)
    )
    assert hypercast.cast_unix("1700000000123", UnixPrecision.MILLISECONDS) == Success(
        dt.datetime.fromtimestamp(1700000000.0, dt.timezone.utc) + dt.timedelta(milliseconds=123)
    )


def test_u64_comes_back_as_the_true_unsigned_value():
    """u64 is the true unsigned value, int being unbounded."""
    assert hypercast.cast_u64("18446744073709551615", NumFormat.INVARIANT) == Success(2**64 - 1)


def test_fast_built_temporals_equal_the_ordinary_constructors():
    """The extension builds dates, times and datetimes from their packed pickle state and durations
    by datetime subtraction; whatever it hands back has to be the object the keyword constructors
    would have made, type included, across the fields' ranges.
    """
    utc = dt.timezone.utc
    for text, expected in (
        ("0001-01-01T00:00:00Z", dt.datetime(1, 1, 1, tzinfo=utc)),
        ("1969-12-31T23:59:59.999999Z", dt.datetime(1969, 12, 31, 23, 59, 59, 999999, tzinfo=utc)),
        ("2024-02-29T12:34:56.000001Z", dt.datetime(2024, 2, 29, 12, 34, 56, 1, tzinfo=utc)),
        (
            "9999-12-31T23:59:59.999999999Z",
            dt.datetime(9999, 12, 31, 23, 59, 59, 999999, tzinfo=utc),
        ),
    ):
        got = hypercast.cast_timestamp(text)
        assert isinstance(got, Success) and type(got.value) is dt.datetime
        assert got.value == expected and got.value.tzinfo is utc
    got = hypercast.cast_date("9999-12-31")
    assert (
        isinstance(got, Success)
        and type(got.value) is dt.date
        and got.value == dt.date(9999, 12, 31)
    )
    got = hypercast.cast_time("23:59:59.999999")
    assert (
        isinstance(got, Success)
        and type(got.value) is dt.time
        and got.value == dt.time(23, 59, 59, 999999)
    )
    got = hypercast.cast_datetime("12/31/9999 11:59:59 PM", hypercast.DateOrder.MONTH_DAY_YEAR)
    assert (
        isinstance(got, Success)
        and got.value == dt.datetime(9999, 12, 31, 23, 59, 59)
        and got.value.tzinfo is None
    )


def test_durations_equal_the_timedelta_constructor_on_both_sides_of_the_fast_path():
    """3,652,058 days is the longest span the subtraction path can build; one more day takes the
    constructor. Both signs, and sub-microsecond digits truncating toward zero.
    """
    for text, expected in (
        ("PT0S", dt.timedelta(0)),
        ("PT1H30M15.5S", dt.timedelta(hours=1, minutes=30, seconds=15, microseconds=500000)),
        ("-PT1H30M15.5S", -dt.timedelta(hours=1, minutes=30, seconds=15, microseconds=500000)),
        ("PT0.000001S", dt.timedelta(microseconds=1)),
        ("-PT0.000001S", dt.timedelta(microseconds=-1)),
        ("PT0.0000009S", dt.timedelta(0)),
        ("-PT0.0000009S", dt.timedelta(0)),
        (
            "P3652058DT23H59M59.999999S",
            dt.timedelta(days=3652058, hours=23, minutes=59, seconds=59, microseconds=999999),
        ),
        ("-P3652058D", dt.timedelta(days=-3652058)),
        ("P3652059D", dt.timedelta(days=3652059)),
        ("-P3652059D", dt.timedelta(days=-3652059)),
    ):
        got = hypercast.cast_duration(text)
        assert isinstance(got, Success), text
        assert type(got.value) is dt.timedelta and got.value == expected, text


def test_date_time_and_duration_map_to_their_stdlib_types():
    """Dates, times and durations come back as the datetime module's own types."""
    assert hypercast.cast_date("2026-01-02") == Success(dt.date(2026, 1, 2))
    assert hypercast.cast_time("15:04:05.123456789") == Success(dt.time(15, 4, 5, 123456))
    assert hypercast.cast_duration("P1DT6H") == Success(dt.timedelta(hours=30))
    assert hypercast.cast_duration("-1.5s") == Success(dt.timedelta(seconds=-1.5))
    assert hypercast.cast_duration("01:30") == Success(dt.timedelta(minutes=90))


def test_optional_presents_empty_as_none():
    """optional folds an EMPTY fault to None and passes everything else through."""
    assert hypercast.optional(hypercast.cast_i32("   ", NumFormat.INVARIANT)) is None
    assert hypercast.optional(hypercast.cast_i32("42", NumFormat.INVARIANT)) == Success(42)
    assert hypercast.optional(hypercast.cast_i32("abc", NumFormat.INVARIANT)) == Fault(
        CastFailure.MALFORMED, 0, 1
    )


def test_equal_separators_are_a_caller_bug():
    """A format whose separators are equal raises rather than becoming a verdict."""
    with pytest.raises(ValueError):
        NumFormat(".", ".", NumFormat.ALL)


def test_separators_are_one_to_eight_utf8_bytes():
    """A separator is 1 to 8 UTF-8 bytes with no ASCII digit, the core's own rule: the
    comma-space and apostrophe-space groupings some feeds carry, read as declared.
    """
    comma_space = NumFormat(".", ", ", NumFormat.ALL)
    assert comma_space.group_sep == ", "
    assert hypercast.cast_i32("1, 234", comma_space) == Success(1234)
    assert hypercast.cast_i32("1,234", comma_space) == Fault(CastFailure.MALFORMED, 1, 1)
    assert hypercast.cast_f64("1' 234.5", NumFormat(".", "' ", NumFormat.ALL)) == Success(1234.5)
    assert hypercast.cast_f64(
        "1\u00a0\u00a0234,5", NumFormat(",", "\u00a0\u00a0", NumFormat.ALL)
    ) == (Success(1234.5))
    assert NumFormat(".", "\u00a0\u00a0\u00a0\u00a0", NumFormat.ALL).group_sep == "\u00a0" * 4
    for bad in ("", "1", ".0", "x" * 9, "\u00a0" * 5):
        with pytest.raises(ValueError, match="1 to 8 UTF-8 bytes"):
            NumFormat(bad, ",", NumFormat.ALL)
        with pytest.raises(ValueError, match="1 to 8 UTF-8 bytes"):
            NumFormat(".", bad, NumFormat.ALL)


def test_declared_signs_are_read_beside_ascii():
    """A declared minus or plus is a further spelling wherever ASCII - or + is read: ahead of the
    digits, either side of a leading currency symbol. ASCII is always read; two signs are a fault.
    """
    estonian = NumFormat(",", "\u00a0", NumFormat.ALL, "€", minus_sign="\u2212", plus_sign="\uff0b")
    assert (estonian.minus_sign, estonian.plus_sign) == ("\u2212", "\uff0b")
    assert hypercast.cast_i32("\u22125", estonian) == Success(-5)
    assert hypercast.cast_i32("-5", estonian) == Success(-5)
    assert hypercast.cast_i32("\uff0b5", estonian) == Success(5)
    assert hypercast.cast_f64("\u22122,5", estonian) == Success(-2.5)
    assert hypercast.cast_decimal("€\u22125", estonian) == Success(Decimal("-5"))
    assert hypercast.cast_decimal("\u2212€5", estonian) == Success(Decimal("-5"))
    assert hypercast.cast_i32("\u2212-5", estonian) == Fault(CastFailure.MALFORMED, 1, 1)
    # Undeclared, U+2212 is just the first offending character.
    assert hypercast.cast_i32("\u22125", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 0, 1)


def test_trailing_sign_is_opted_into_not_assumed():
    """TRAILING_SIGN reads the sign after the digits (a culture whose negative pattern is n-),
    with optional whitespace before it and around a trailing symbol; it is not in ALL, and never
    beside a leading sign or inside parentheses.
    """
    assert not NumFormat.ALL & NumFormat.TRAILING_SIGN
    assert NumFormat.TRAILING_SIGN == 128
    trailing = NumFormat(
        ".", ",", NumFormat.ALL | NumFormat.TRAILING_SIGN, "€", minus_sign="\u2212"
    )
    assert hypercast.cast_i32("5-", trailing) == Success(-5)
    assert hypercast.cast_i32("5 -", trailing) == Success(-5)
    assert hypercast.cast_i32("5+", trailing) == Success(5)
    assert hypercast.cast_i32("5\u2212", trailing) == Success(-5)
    assert hypercast.cast_i32("5 €-", trailing) == Success(-5)
    assert hypercast.cast_i32("5- €", trailing) == Success(-5)
    assert hypercast.cast_decimal("2.5%-", trailing) == Success(Decimal("-0.025"))
    assert hypercast.cast_decimal("2.5-%", trailing) == Success(Decimal("-0.025"))
    assert hypercast.cast_i32("-5-", trailing) == Fault(CastFailure.MALFORMED, 2, 1)
    assert hypercast.cast_i32("(5-)", trailing) == Fault(CastFailure.MALFORMED, 2, 1)
    # Off by default: the same text is a fault at the sign.
    assert hypercast.cast_i32("5-", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 1, 1)


def test_currency_separators_apply_when_the_symbol_is_found():
    """A currency amount is read with the currency separators — each falling back to its number
    counterpart — when CURRENCY is set, the symbol is declared, and the symbol is in the text; a
    plain number keeps the number separators. Under SEPARATOR_DETECT detection governs instead.
    """
    fmt = NumFormat(",", ".", NumFormat.ALL, "€", currency_decimal_sep=".", currency_group_sep=",")
    assert (fmt.currency_decimal_sep, fmt.currency_group_sep) == (".", ",")
    assert hypercast.cast_f64("1,234.50 €", fmt) == Success(1234.5)
    assert hypercast.cast_decimal("€1,234.50", fmt) == Success(Decimal("1234.5"))
    assert hypercast.cast_f64("1.234,50", fmt) == Success(1234.5)
    assert hypercast.cast_f64("1,234.50", fmt) == Fault(CastFailure.MALFORMED, 5, 1)
    # One declared: the other stays the number separator.
    half = NumFormat(",", ".", NumFormat.ALL, "€", currency_decimal_sep="\u00b7")
    assert (half.currency_decimal_sep, half.currency_group_sep) == ("\u00b7", "")
    assert hypercast.cast_f64("1.234\u00b75 €", half) == Success(1234.5)
    detect = NumFormat(
        ",",
        ".",
        NumFormat.ALL | NumFormat.SEPARATOR_DETECT,
        "€",
        currency_decimal_sep=".",
        currency_group_sep=",",
    )
    assert hypercast.cast_f64("1.234,50 €", detect) == Success(1234.5)
    assert hypercast.cast_f64("1,234.50 €", detect) == Success(1234.5)


def test_declared_percent_is_read_as_a_trailing_percent():
    """A declared percent symbol is read exactly as a trailing ASCII % is — real and decimal doors
    only, the integer doors never read one — and ASCII % is still read beside it.
    """
    arabic = NumFormat(".", ",", NumFormat.ALL, percent="\u066a")
    assert arabic.percent == "\u066a"
    assert hypercast.cast_decimal("50\u066a", arabic) == Success(Decimal("0.5"))
    assert hypercast.cast_f64("50\u066a", arabic) == Success(0.5)
    assert hypercast.cast_f64("50%", arabic) == Success(0.5)
    assert hypercast.cast_i32("50\u066a", arabic) == Fault(CastFailure.MALFORMED, 2, 1)
    assert hypercast.cast_f64("50\u066a", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 2, 1)


def test_native_digits_are_read_beside_ascii():
    """The ten code points from a declared zero are digits everywhere ASCII digits are — integer
    part, fraction, exponent, mixed with ASCII — and radix-prefixed digits stay ASCII.
    """
    arabic = NumFormat("\u066b", "\u066c", NumFormat.ALL, digit_zero="\u0660")
    assert arabic.digit_zero == "\u0660"
    assert hypercast.cast_i32("\u0661\u0662\u0663", arabic) == Success(123)
    assert hypercast.cast_i32("12\u0663", arabic) == Success(123)
    assert hypercast.cast_f64("\u0661\u066c\u0662\u0663\u0664\u066b\u0665", arabic) == Success(
        1234.5
    )
    assert hypercast.cast_f64("\u0661\u0662\u066b\u0665e\u0661", arabic) == Success(125.0)
    assert hypercast.cast_decimal("\u0661\u0662\u066b\u0665", arabic) == Success(Decimal("12.5"))
    assert hypercast.cast_i32("0x\u0661", arabic) == Fault(CastFailure.MALFORMED, 2, 1)
    fullwidth = NumFormat(".", ",", NumFormat.ALL, digit_zero="\uff10")
    assert hypercast.cast_i32("\uff11\uff12\uff13", fullwidth) == Success(123)
    assert hypercast.cast_i32("\u0661\u0662\u0663", NumFormat.INVARIANT) == Fault(
        CastFailure.MALFORMED, 0, 1
    )


def test_symbol_rules_are_a_caller_bug():
    """The C ABI's contract rules, raised at construction so a format that exists is one the core
    reads: a sign or percent with whitespace or a digit, a symbol too long for its field, equal
    currency separators in force, a digit zero that is ASCII or starts no digit block.
    """
    for name in ("minus_sign", "plus_sign", "percent"):
        for bad in ("- ", " -", "a b", "1", "x" * 9, "\u2212" * 3):
            with pytest.raises(ValueError):
                NumFormat(".", ",", NumFormat.ALL, **{name: bad})
    for name in ("currency_decimal_sep", "currency_group_sep"):
        for bad in ("x" * 9, "1", "\u00a0" * 5):
            with pytest.raises(ValueError):
                NumFormat(".", ",", NumFormat.ALL, **{name: bad})
    # The pair in force for a currency amount must differ, counting what each falls back to.
    with pytest.raises(ValueError, match="Currency decimal and group separators must differ"):
        NumFormat(".", ",", NumFormat.ALL, "€", currency_decimal_sep=",")
    with pytest.raises(ValueError, match="Currency decimal and group separators must differ"):
        NumFormat(".", ",", NumFormat.ALL, "€", currency_group_sep=".")
    with pytest.raises(ValueError, match="Currency decimal and group separators must differ"):
        NumFormat(".", ",", NumFormat.ALL, "€", currency_decimal_sep="'", currency_group_sep="'")
    # The same pair both ways round is fine.
    NumFormat(".", ",", NumFormat.ALL, "€", currency_decimal_sep=",", currency_group_sep=".")
    for bad in ("0", "٠٠", "a", "\u00bf", "\u00ff"):
        with pytest.raises(ValueError, match="Digit zero"):
            NumFormat(".", ",", NumFormat.ALL, digit_zero=bad)


def test_a_formats_repr_builds_it_again():
    """The repr is the constructor call: the four positional arguments always, then only the
    keywords that declare something.
    """
    assert repr(NumFormat.INVARIANT) == f"NumFormat('.', ',', {NumFormat.ALL}, '')"
    fmt = NumFormat(
        ",",
        "\u00a0",
        NumFormat.ALL | NumFormat.TRAILING_SIGN,
        "€",
        currency_decimal_sep=".",
        minus_sign="\u2212",
        percent="\u066a",
        digit_zero="\u0660",
    )
    # str's own repr spells each symbol, so a no-break space is escaped as it would be anywhere.
    assert repr(fmt) == (
        f"NumFormat(',', '\\xa0', {NumFormat.ALL | NumFormat.TRAILING_SIGN}, '€', "
        "currency_decimal_sep='.', minus_sign='\u2212', percent='\u066a', digit_zero='\u0660')"
    )
    rebuilt = eval(repr(fmt), {"NumFormat": NumFormat})  # noqa: S307 - the repr is the test
    assert repr(rebuilt) == repr(fmt) and rebuilt.packed == fmt.packed


def test_bytes_and_str_doors_agree():
    """str and bytes input reach the same verdict."""
    assert hypercast.cast_bool("yes") == hypercast.cast_bool(b"yes") == Success(True)


def test_char_reads_one_scalar_verbatim_or_a_declared_code_point():
    """A single character is itself (a lone space or digit included); anything longer must spell
    a code point, and a str fault is spanned in code points.
    """
    assert hypercast.cast_char(" ") == Success(" ")
    assert hypercast.cast_char("6") == Success("6")
    assert hypercast.cast_char("😀") == hypercast.cast_char(b"\xf0\x9f\x98\x80") == Success("😀")
    for spelling in ("233", "U+00E9", "u+e9", "0xE9", "&HE9", "&#233;", "&#xE9;", "  U+00E9  "):
        assert hypercast.cast_char(spelling) == Success("é"), spelling
    assert hypercast.cast_char("   ") == Fault(CastFailure.EMPTY, 0, 0)
    assert hypercast.cast_char("&#65") == Fault(CastFailure.MALFORMED, 0, 4)
    assert hypercast.cast_char("U+D800") == Fault(CastFailure.OUT_OF_RANGE, 0, 6)
    # é is two UTF-8 bytes but one code point: the str span is in code points, bytes in bytes.
    assert hypercast.cast_char("éx") == Fault(CastFailure.MALFORMED, 0, 1)
    assert hypercast.cast_char("U+é") == Fault(CastFailure.MALFORMED, 2, 1)
    assert hypercast.cast_char("U+é".encode()) == Fault(CastFailure.MALFORMED, 2, 2)


def test_date_order_disambiguates_like_the_cultures_do():
    """The canonical ambiguity: 1/7/2026 is January 7th under en-US's month-first short dates and
    July 1st under en-GB's day-first ones — resolved only by declaration.
    """
    assert hypercast.cast_date("1/7/2026", hypercast.DateOrder.MONTH_DAY_YEAR) == hypercast.Success(
        dt.date(2026, 1, 7)
    )
    assert hypercast.cast_date("1/7/2026", hypercast.DateOrder.DAY_MONTH_YEAR) == hypercast.Success(
        dt.date(2026, 7, 1)
    )
    # Undeclared, the door stays strict ISO — the ambiguity is never guessed at.
    match hypercast.cast_date("1/7/2026"):
        case hypercast.Fault(reason, _, _):
            assert reason is hypercast.CastFailure.MALFORMED
        case other:
            raise AssertionError(f"undeclared slash date parsed: {other!r}")


def test_datetime_reads_the_messy_civil_shapes():
    """The AM/PM world, zone-less: the value is naive because the text named no zone — fusing one is
    the caller's job, never the parser's guess.
    """
    verdict = hypercast.cast_datetime("1/7/2026 3:04 PM", hypercast.DateOrder.MONTH_DAY_YEAR)
    assert verdict == hypercast.Success(dt.datetime(2026, 1, 7, 15, 4))
    assert verdict.value.tzinfo is None
    assert hypercast.cast_datetime(
        "1/7/2026 3:04 PM", hypercast.DateOrder.DAY_MONTH_YEAR
    ) == hypercast.Success(dt.datetime(2026, 7, 1, 15, 4))
    # ISO forms ride through the same door (a four-digit first field is structurally a year).
    assert hypercast.cast_datetime(
        "2026-01-07T15:04:05", hypercast.DateOrder.MONTH_DAY_YEAR
    ) == hypercast.Success(dt.datetime(2026, 1, 7, 15, 4, 5))
    # A zone suffix is not this door's business — cast_timestamp is the instant door.
    match hypercast.cast_datetime("1/7/2026 15:04:05Z", hypercast.DateOrder.MONTH_DAY_YEAR):
        case hypercast.Fault(reason, _, _):
            assert reason is hypercast.CastFailure.MALFORMED
        case other:
            raise AssertionError(f"zoned text parsed through the civil door: {other!r}")


def test_datetime_reads_declared_day_periods_beside_am_pm():
    """A culture's designators, declared as am= and pm=, are read exactly, after the clock or ahead
    of it, beside the English AM/PM — which are read whatever is declared, ASCII case-insensitively,
    and now ahead of the clock too. Undeclared, a designator is the first offending character.
    """
    mdy, ymd = hypercast.DateOrder.MONTH_DAY_YEAR, hypercast.DateOrder.YEAR_MONTH_DAY
    three_pm = hypercast.Success(dt.datetime(2026, 1, 7, 15, 4))
    german = {"am": "vorm.", "pm": "nachm."}
    assert hypercast.cast_datetime("1/7/2026 3:04 nachm.", mdy, **german) == three_pm
    assert hypercast.cast_datetime("1/7/2026 3:04nachm.", mdy, **german) == three_pm
    assert hypercast.cast_datetime("1/7/2026 3:04 vorm.", mdy, **german) == hypercast.Success(
        dt.datetime(2026, 1, 7, 3, 4)
    )
    assert hypercast.cast_datetime("1/7/2026 3:04 PM", mdy, **german) == three_pm
    assert hypercast.cast_datetime("1/7/2026 pm 3:04", mdy, **german) == three_pm
    korean = {"am": "오전", "pm": "오후"}
    assert hypercast.cast_datetime("2026-01-07 오후 3:04", ymd, **korean) == three_pm
    assert hypercast.cast_datetime("2026-01-07 오후3:04", ymd, **korean) == three_pm
    assert hypercast.cast_datetime("2026-01-07 오전 12:00", ymd, **korean) == hypercast.Success(
        dt.datetime(2026, 1, 7)
    )
    # Internal whitespace is part of a designator (Spanish); the edges may not be.
    assert hypercast.cast_datetime("1/7/2026 3:04 p. m.", mdy, am="a. m.", pm="p. m.") == three_pm
    # Declared designators match exact bytes; English matches either case. The fault sits
    # where the clock ends — at the trailing text — as for anything else after it.
    assert hypercast.cast_datetime("1/7/2026 3:04 NACHM.", mdy, **german) == hypercast.Fault(
        CastFailure.MALFORMED, 13, 1
    )
    assert hypercast.cast_datetime("1/7/2026 3:04 nachm.", mdy) == hypercast.Fault(
        CastFailure.MALFORMED, 13, 1
    )
    # One at a time is allowed, and bytes input reads the same.
    assert hypercast.cast_datetime("2026-01-07 오후 3:04", ymd, pm="오후") == three_pm
    assert hypercast.cast_datetime("2026-01-07 오후 3:04".encode(), ymd, **korean) == three_pm
    for undeclared in ("2026-01-07 오후 3:04", "2026-01-07 오후 3:04".encode()):
        assert hypercast.cast_datetime(undeclared, ymd) == hypercast.Fault(
            CastFailure.MALFORMED, 11, 1
        )


def test_day_period_rules_are_a_caller_bug():
    """A designator is 1 to 16 UTF-8 bytes with no ASCII digit and no ASCII whitespace at either
    end — the C ABI's contract rule, raised at the call the way a format's rules are at
    construction.
    """
    mdy = hypercast.DateOrder.MONTH_DAY_YEAR
    for bad in (" am", "pm ", "a1", "x" * 17, "오" * 6):
        with pytest.raises(ValueError, match="1 to 16 UTF-8 bytes"):
            hypercast.cast_datetime("1/7/2026 3:04 PM", mdy, am=bad)
        with pytest.raises(ValueError, match="1 to 16 UTF-8 bytes"):
            hypercast.cast_datetime("1/7/2026 3:04 PM", mdy, pm=bad)
    for wrong_type in (b"am", 1, None):
        with pytest.raises(TypeError):
            hypercast.cast_datetime("1/7/2026 3:04 PM", mdy, am=wrong_type)
    # Positional designators are not a thing: the keywords are the declaration.
    with pytest.raises(TypeError):
        hypercast.cast_datetime("1/7/2026 3:04 PM", mdy, "vorm.", "nachm.")


def _symbol(text: str, width: int) -> bytes:
    """A symbol as it sits in the layout: its UTF-8 zero-padded to ``width``, then its length."""
    utf8 = text.encode()
    return utf8 + bytes(width - len(utf8)) + bytes([len(utf8)])


def test_a_format_packs_as_the_core_reads_it():
    """A format's packed bytes are the core's 92-byte size-tagged layout, field by field at the
    offsets every binding pins: size 0, flags 4, digit zero 8, then the 8+1-byte separators,
    signs and percent from 12, and the 16+1-byte currency symbol from 75.
    """
    packed = NumFormat(",", ".", NumFormat.ALL, "€").packed
    assert len(packed) == 92
    assert packed[0:4] == (92).to_bytes(4, "little")
    assert packed[4:8] == NumFormat.ALL.to_bytes(4, "little")
    assert packed[8:12] == bytes(4)
    assert packed[12:21] == _symbol(",", 8)
    assert packed[21:30] == _symbol(".", 8)
    assert packed[30:75] == bytes(45)  # nothing else declared: five empty symbols
    assert packed[75:92] == _symbol("€", 16)
    full = NumFormat(
        "\u066b",
        "\u066c",
        NumFormat.ALL | NumFormat.TRAILING_SIGN,
        "د.إ.",
        currency_decimal_sep=".",
        currency_group_sep=", ",
        minus_sign="\u2212",
        plus_sign="\uff0b",
        percent="\u066a",
        digit_zero="\u0660",
    ).packed
    assert len(full) == 92
    assert full[0:4] == (92).to_bytes(4, "little")
    assert full[4:8] == (NumFormat.ALL | NumFormat.TRAILING_SIGN).to_bytes(4, "little")
    assert full[8:12] == (0x0660).to_bytes(4, "little")
    assert full[12:21] == _symbol("\u066b", 8)
    assert full[21:30] == _symbol("\u066c", 8)
    assert full[30:39] == _symbol(".", 8)
    assert full[39:48] == _symbol(", ", 8)
    assert full[48:57] == _symbol("\u2212", 8)
    assert full[57:66] == _symbol("\uff0b", 8)
    assert full[66:75] == _symbol("\u066a", 8)
    assert full[75:92] == _symbol("د.إ.", 16)
    # The README's own example, byte for byte.
    assert NumFormat(".", ",", NumFormat.ALL, "$").packed == (
        b"\x5c\x00\x00\x00_\x00\x00\x00\x00\x00\x00\x00"
        + b".\x00\x00\x00\x00\x00\x00\x00\x01"
        + b",\x00\x00\x00\x00\x00\x00\x00\x01"
        + bytes(45)
        + b"$\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01"
    )
