"""Replays the shared conformance corpus (corpus/*.json at the repository root) — the same
files the Rust core's, C# binding's, and Java binding's suites replay — through this
binding's bytes doors. Fault spans are byte offsets into the UTF-8 input, which is exactly
what these doors receive, so span assertions hold verbatim. This is the byte-for-byte
polyglot contract: a vector that fails here is a break in the promise, not just a failing
test.
"""

from __future__ import annotations

import datetime as dt
import json
import uuid as uuidlib
from pathlib import Path

import hypercast
from hypercast import CastFailure, Fault, NumFormat, Success, UnixPrecision


def _corpus_dir() -> Path:
    for parent in Path(__file__).resolve().parents:
        candidate = parent / "corpus"
        if candidate.is_dir():
            return candidate
    raise FileNotFoundError("corpus directory not found")


def _corpus(name: str) -> list[dict]:
    return json.loads((_corpus_dir() / name).read_text(encoding="utf-8"))


_EXPECTED_REASON = {
    "empty": CastFailure.EMPTY,
    "malformed": CastFailure.MALFORMED,
    "out_of_range": CastFailure.OUT_OF_RANGE,
}


def _format_of(vector: dict) -> NumFormat:
    fmt = vector.get("format")
    if fmt is None:
        return NumFormat.INVARIANT
    return NumFormat(fmt["decimal_sep"], fmt["group_sep"], fmt["flags"])


def _assert_verdict(domain: str, vector: dict, verdict, expected) -> None:
    text = vector["input"]
    expect = vector["expect"]
    # The union consumption idiom in action — match over the two case types.
    match verdict:
        case Success(value):
            assert expect == "ok", f"{domain}: {text!r} unexpectedly parsed to {value!r}"
            assert value == expected, f"{domain}: {text!r} -> {value!r}, want {expected!r}"
        case Fault(reason, offset, length):
            assert expect in _EXPECTED_REASON, f"{domain}: {text!r} expected {expect} but faulted"
            assert reason is _EXPECTED_REASON[expect], f"{domain}: {text!r} -> {reason}"
            if "fault" in vector:
                assert [offset, length] == vector["fault"], f"{domain}: {text!r} fault span"
        case other:  # pragma: no cover - the union is closed
            raise AssertionError(f"{domain}: {text!r} produced no case: {other!r}")


def _input(vector: dict) -> bytes:
    return vector["input"].encode("utf-8")


def test_boolean_corpus():
    for vector in _corpus("boolean.json"):
        _assert_verdict("boolean", vector, hypercast.cast_bool(_input(vector)), vector.get("value"))


_INT_DOORS = {
    "i8": hypercast.cast_i8, "i16": hypercast.cast_i16,
    "i32": hypercast.cast_i32, "i64": hypercast.cast_i64,
    "u8": hypercast.cast_u8, "u16": hypercast.cast_u16,
    "u32": hypercast.cast_u32, "u64": hypercast.cast_u64,
}


def test_integer_corpus():
    for vector in _corpus("integer.json"):
        door = _INT_DOORS[vector["type"]]
        # Python int is unbounded — u64's full unsigned value comes back natively.
        _assert_verdict("integer", vector, door(_input(vector), _format_of(vector)), vector.get("value"))


def test_real_corpus():
    for vector in _corpus("real.json"):
        door = hypercast.cast_f32 if vector["type"] == "f32" else hypercast.cast_f64
        expected = vector.get("value")
        if expected is not None and vector["type"] == "f32":
            import struct

            expected = struct.unpack("f", struct.pack("f", expected))[0]
        _assert_verdict("real", vector, door(_input(vector), _format_of(vector)), expected)


def test_uuid_corpus():
    for vector in _corpus("uuid.json"):
        expected = uuidlib.UUID(hex=vector["value"]) if "value" in vector else None
        _assert_verdict("uuid", vector, hypercast.cast_uuid(_input(vector)), expected)


_EPOCH = dt.datetime(1970, 1, 1, tzinfo=dt.timezone.utc)


def _expected_instant(vector: dict) -> dt.datetime | None:
    if "seconds" not in vector:
        return None
    return _EPOCH + dt.timedelta(seconds=vector["seconds"], microseconds=vector["nanos"] // 1000)


def test_timestamp_corpus():
    for vector in _corpus("timestamp.json"):
        _assert_verdict("timestamp", vector, hypercast.cast_timestamp(_input(vector)),
                        _expected_instant(vector))


def test_unix_corpus():
    for vector in _corpus("unix.json"):
        precision = UnixPrecision(vector["precision"])
        _assert_verdict("unix", vector, hypercast.cast_unix(_input(vector), precision),
                        _expected_instant(vector))


def test_date_corpus():
    for vector in _corpus("date.json"):
        expected = dt.date(vector["year"], vector["month"], vector["day"]) if "year" in vector else None
        _assert_verdict("date", vector, hypercast.cast_date(_input(vector)), expected)


def test_time_corpus():
    for vector in _corpus("time.json"):
        expected = None
        if "nanos" in vector:
            second_of_day, nano = divmod(vector["nanos"], 1_000_000_000)
            hour, rest = divmod(second_of_day, 3600)
            minute, second = divmod(rest, 60)
            expected = dt.time(hour, minute, second, nano // 1000)
        _assert_verdict("time", vector, hypercast.cast_time(_input(vector)), expected)


def test_duration_corpus():
    for vector in _corpus("duration.json"):
        expected = None
        if "seconds" in vector:
            nanos = vector["nanos"]
            micros = nanos // 1000 if nanos >= 0 else -((-nanos) // 1000)
            expected = dt.timedelta(seconds=vector["seconds"], microseconds=micros)
        _assert_verdict("duration", vector, hypercast.cast_duration(_input(vector)), expected)
