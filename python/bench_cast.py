"""pyperf benchmarks: hypercast doors vs the stdlib's closest parse.

Run with: python bench_cast.py --fast -q
(--fast trims pyperf's default process/value counts so this finishes in minutes while
staying statistically valid.)

Honesty notes baked into the pairings: int() has no grouping knob at any price (the
ungrouped Cast row is the like-for-like); datetime tops out at microseconds, so both sides
parse a microsecond-precision instant; the stdlib has no ISO-8601 duration parser at all,
so that door runs unopposed, printed for the record rather than compared.
"""

from __future__ import annotations

import datetime
import functools
import sys
import uuid
from pathlib import Path

import pyperf

sys.path.insert(0, str(Path(__file__).resolve().parent / "src"))

import hypercast  # noqa: E402
from hypercast import NumFormat  # noqa: E402

INVARIANT = NumFormat.INVARIANT
BOOL = "true"
INT = "1234567"
INT_GROUPED = "1,234,567"
FLOAT = "12345.6789"
UUID_TEXT = "01020304-0506-0708-090a-0b0c0d0e0f10"
TIMESTAMP = "2026-01-02T15:04:05.123456+05:00"
ISO_SPAN = "PT1H30M15.5S"


MESSY_DATETIME = "1/7/2026 3:04 PM"
MESSY_DATE = "1/7/2026"
EURO_NUMBER = "1.234.567,89"
MDY = hypercast.DateOrder.MONTH_DAY_YEAR
DETECT = hypercast.NumFormat.DETECT
EUROZONE = hypercast.NumFormat(",", ".", hypercast.NumFormat.ALL)
DOLLARS = hypercast.NumFormat(".", ",", hypercast.NumFormat.ALL, "$")
INT_MALFORMED = "1,234x567"
# A culture that spells everything differently: Arabic-Indic digits and separators, a
# declared percent, a U+2212 minus read after the digits.
ARABIC = hypercast.NumFormat(
    "\u066b",
    "\u066c",
    hypercast.NumFormat.ALL | hypercast.NumFormat.TRAILING_SIGN,
    minus_sign="\u2212",
    percent="\u066a",
    digit_zero="\u0660",
)
ARABIC_NUMBER = "\u0661\u066c\u0662\u0663\u0664\u066b\u0665\u066a\u2212"
GERMAN_DATETIME = "1/7/2026 3:04 nachm."


def int_malformed(text: str) -> None:
    """int()'s rejection, caught: the stdlib's failure path costs an exception."""
    try:
        int(text)
    except ValueError:
        pass


def bench_all(runner: pyperf.Runner) -> None:
    """Registers every door with pyperf over its representative input."""
    runner.bench_func("cast_bool", hypercast.cast_bool, BOOL)

    runner.bench_func("cast_i32", hypercast.cast_i32, INT, INVARIANT)
    runner.bench_func("cast_i32 grouped", hypercast.cast_i32, INT_GROUPED, INVARIANT)
    runner.bench_func("int()", int, INT)
    # A declared currency symbol the text does not carry, and the rejection path.
    runner.bench_func("cast_i32 grouped, declared $", hypercast.cast_i32, INT_GROUPED, DOLLARS)
    runner.bench_func("cast_i32 malformed", hypercast.cast_i32, INT_MALFORMED, INVARIANT)
    runner.bench_func("int() malformed", int_malformed, "1234x567")

    runner.bench_func("cast_f64", hypercast.cast_f64, FLOAT, INVARIANT)
    runner.bench_func("float()", float, FLOAT)

    runner.bench_func("cast_uuid", hypercast.cast_uuid, UUID_TEXT)
    runner.bench_func("uuid.UUID()", uuid.UUID, UUID_TEXT)

    runner.bench_func("cast_timestamp", hypercast.cast_timestamp, TIMESTAMP)
    runner.bench_func("datetime.fromisoformat", datetime.datetime.fromisoformat, TIMESTAMP)

    # No stdlib ISO-8601 duration parser exists to pair against.
    runner.bench_func("cast_duration", hypercast.cast_duration, ISO_SPAN)

    # The messy civil shape: strptime is the stdlib parser that accepts it at all
    # (fromisoformat rejects it outright), so that's the honest pairing.
    runner.bench_func("cast_datetime", hypercast.cast_datetime, MESSY_DATETIME, MDY)
    runner.bench_func(
        "datetime.strptime", datetime.datetime.strptime, MESSY_DATETIME, "%m/%d/%Y %I:%M %p"
    )

    # Declared day periods: the keyword path through a partial, since bench_func passes
    # positional arguments only (a partial's own call is a few nanoseconds).
    runner.bench_func(
        "cast_datetime declared periods",
        functools.partial(hypercast.cast_datetime, GERMAN_DATETIME, MDY, am="vorm.", pm="nachm."),
    )

    runner.bench_func("cast_date ordered", hypercast.cast_date, MESSY_DATE, MDY)
    runner.bench_func("date.strptime", datetime.datetime.strptime, MESSY_DATE, "%m/%d/%Y")

    # Separator detection against the same text under a declared format.
    runner.bench_func("cast_f64 DETECT", hypercast.cast_f64, EURO_NUMBER, DETECT)
    runner.bench_func("cast_f64 declared", hypercast.cast_f64, EURO_NUMBER, EUROZONE)
    # Native digits, a declared percent and a trailing declared sign, all in one token.
    runner.bench_func("cast_f64 native digits", hypercast.cast_f64, ARABIC_NUMBER, ARABIC)


if __name__ == "__main__":
    bench_all(pyperf.Runner())
