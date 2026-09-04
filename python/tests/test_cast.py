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
    match hypercast.cast_i32("42", NumFormat.INVARIANT):
        case Success(value):
            assert value == 42
        case Fault():
            pytest.fail("42 should parse")


def test_fault_span_points_at_the_offending_byte():
    assert hypercast.cast_i32("  12x4", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 4, 1)


def test_str_doors_transcode_non_ascii_separators():
    french = NumFormat(",", " ", NumFormat.ALL)
    assert hypercast.cast_f64("1 234,5", french) == Success(1234.5)


def test_fault_spans_come_back_in_the_callers_own_units():
    # The core reports byte spans into the UTF-8 it saw; a str caller's unit is the code
    # point, so the same text faults at the same place in either unit and slices back out.
    assert hypercast.cast_i32("€x", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 0, 1)
    assert hypercast.cast_i32("€x".encode(), NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 0, 3)
    assert hypercast.cast_i32("1€", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 1, 1)
    assert hypercast.cast_i32("1€".encode(), NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 1, 3)
    text = "12€4"
    for passed in (text, text.encode()):
        match hypercast.cast_f64(passed, NumFormat.INVARIANT):
            case Fault(_, offset, length):
                assert passed[offset:offset + length] == ("€" if isinstance(passed, str) else "€".encode())
            case other:
                raise AssertionError(f"{passed!r} parsed: {other!r}")
    # A non-ASCII input that succeeds, and an ASCII fault, are untouched by the mapping.
    assert hypercast.cast_f64("1\u00a0234,5", NumFormat(",", "\u00a0", NumFormat.ALL)) == Success(1234.5)
    assert hypercast.cast_i32("  12x4", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 4, 1)


def test_localeconv_bridge():
    fmt = NumFormat.from_localeconv({"decimal_point": ",", "thousands_sep": "."})
    assert fmt.currency == ""
    assert hypercast.cast_f64("1.234,5", fmt) == Success(1234.5)
    euro = NumFormat.from_localeconv({"decimal_point": ",", "thousands_sep": ".", "currency_symbol": "€"})
    assert euro.currency == "€"
    assert hypercast.cast_f64("1.234,5 €", euro) == Success(1234.5)


def test_currency_symbol_is_declared_never_guessed():
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
    # 1 to 16 UTF-8 bytes, no ASCII digit or whitespace — the core's CurrencySymbol rule,
    # enforced at construction like equal separators, identically on both backends.
    for bad in ("$5", "US D", "x" * 17, "€" * 6):
        with pytest.raises(ValueError):
            NumFormat(".", ",", NumFormat.ALL, bad)
    assert NumFormat(".", ",", NumFormat.ALL, "€" * 5).currency == "€" * 5  # 15 bytes


def test_decimal_is_exact_and_canonical():
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
        CastFailure.OUT_OF_RANGE, 0, len(str(top + 1)))


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
            found = re.search(r'^version\s*=\s*"([^"]+)"', manifest.read_text(encoding="utf-8"), re.MULTILINE)
            assert found, f"no version in {manifest}"
            return found.group(1)
    raise FileNotFoundError("rust/Cargo.toml not found")


def test_native_version_names_the_loaded_core():
    assert hypercast.native_version() == _expected_version()


def test_uuid_agrees_with_the_platforms_own_parser():
    text = "01020304-0506-0708-090a-0b0c0d0e0f10"
    assert hypercast.cast_uuid(text) == Success(uuidlib.UUID(text))
    assert hypercast.cast_uuid(f"urn:uuid:{text}") == Success(uuidlib.UUID(text))


def test_timestamp_is_aware_utc_with_microsecond_truncation():
    verdict = hypercast.cast_timestamp("2026-01-02T15:04:05.123456789Z")
    expected = dt.datetime(2026, 1, 2, 15, 4, 5, 123456, tzinfo=dt.timezone.utc)
    assert verdict == Success(expected)
    # Offset input normalizes to UTC.
    assert hypercast.cast_timestamp("2026-01-02T15:04:05+05:00") == Success(
        dt.datetime(2026, 1, 2, 10, 4, 5, tzinfo=dt.timezone.utc))
    # The full protobuf window survives timedelta arithmetic.
    assert hypercast.cast_timestamp("0001-01-01T00:00:00Z") == Success(
        dt.datetime(1, 1, 1, tzinfo=dt.timezone.utc))


def test_unix_maps_the_declared_precision():
    assert hypercast.cast_unix("-1", UnixPrecision.SECONDS) == Success(
        dt.datetime(1969, 12, 31, 23, 59, 59, tzinfo=dt.timezone.utc))
    assert hypercast.cast_unix("1700000000123", UnixPrecision.MILLISECONDS) == Success(
        dt.datetime.fromtimestamp(1700000000.0, dt.timezone.utc) + dt.timedelta(milliseconds=123))


def test_u64_comes_back_as_the_true_unsigned_value():
    assert hypercast.cast_u64("18446744073709551615", NumFormat.INVARIANT) == Success(2**64 - 1)


def test_date_time_and_duration_map_to_their_stdlib_types():
    assert hypercast.cast_date("2026-01-02") == Success(dt.date(2026, 1, 2))
    assert hypercast.cast_time("15:04:05.123456789") == Success(dt.time(15, 4, 5, 123456))
    assert hypercast.cast_duration("P1DT6H") == Success(dt.timedelta(hours=30))
    assert hypercast.cast_duration("-1.5s") == Success(dt.timedelta(seconds=-1.5))
    assert hypercast.cast_duration("01:30") == Success(dt.timedelta(minutes=90))


def test_optional_presents_empty_as_none():
    assert hypercast.optional(hypercast.cast_i32("   ", NumFormat.INVARIANT)) is None
    assert hypercast.optional(hypercast.cast_i32("42", NumFormat.INVARIANT)) == Success(42)
    assert hypercast.optional(hypercast.cast_i32("abc", NumFormat.INVARIANT)) == Fault(
        CastFailure.MALFORMED, 0, 1)


def test_equal_separators_are_a_caller_bug():
    with pytest.raises(ValueError):
        NumFormat(".", ".", NumFormat.ALL)


def test_bytes_and_str_doors_agree():
    assert hypercast.cast_bool("yes") == hypercast.cast_bool(b"yes") == Success(True)


def test_date_order_disambiguates_like_the_cultures_do():
    # The canonical ambiguity: 1/7/2026 is January 7th under en-US's month-first short
    # dates and July 1st under en-GB's day-first ones — resolved only by declaration.
    assert hypercast.cast_date("1/7/2026", hypercast.DateOrder.MONTH_DAY_YEAR) == \
        hypercast.Success(dt.date(2026, 1, 7))
    assert hypercast.cast_date("1/7/2026", hypercast.DateOrder.DAY_MONTH_YEAR) == \
        hypercast.Success(dt.date(2026, 7, 1))
    # Undeclared, the door stays strict ISO — the ambiguity is never guessed at.
    match hypercast.cast_date("1/7/2026"):
        case hypercast.Fault(reason, _, _):
            assert reason is hypercast.CastFailure.MALFORMED
        case other:
            raise AssertionError(f"undeclared slash date parsed: {other!r}")


def test_datetime_reads_the_messy_civil_shapes():
    # The AM/PM world, zone-less: the value is naive because the text named no zone —
    # fusing one is the caller's job, never the parser's guess.
    verdict = hypercast.cast_datetime("1/7/2026 3:04 PM", hypercast.DateOrder.MONTH_DAY_YEAR)
    assert verdict == hypercast.Success(dt.datetime(2026, 1, 7, 15, 4))
    assert verdict.value.tzinfo is None
    assert hypercast.cast_datetime("1/7/2026 3:04 PM", hypercast.DateOrder.DAY_MONTH_YEAR) == \
        hypercast.Success(dt.datetime(2026, 7, 1, 15, 4))
    # ISO forms ride through the same door (a four-digit first field is structurally a year).
    assert hypercast.cast_datetime("2026-01-07T15:04:05", hypercast.DateOrder.MONTH_DAY_YEAR) == \
        hypercast.Success(dt.datetime(2026, 1, 7, 15, 4, 5))
    # A zone suffix is not this door's business — cast_timestamp is the instant door.
    match hypercast.cast_datetime("1/7/2026 15:04:05Z", hypercast.DateOrder.MONTH_DAY_YEAR):
        case hypercast.Fault(reason, _, _):
            assert reason is hypercast.CastFailure.MALFORMED
        case other:
            raise AssertionError(f"zoned text parsed through the civil door: {other!r}")
