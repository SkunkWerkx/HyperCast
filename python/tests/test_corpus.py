"""Replays the shared conformance corpus (corpus/*.json at the repository root) — the same
files the Rust core's, C# binding's, and Java binding's suites replay — through this
binding's bytes doors. Fault spans are byte offsets into the UTF-8 input, which is exactly
what these doors receive, so span assertions hold verbatim. This is the byte-for-byte
polyglot contract: a vector that fails here is a break in the promise, not just a failing
test.
"""

from __future__ import annotations

import datetime as dt
import inspect
import json
import re
import struct
import uuid as uuidlib
from decimal import Decimal
from pathlib import Path

import pytest

import hypercast
from hypercast import (
    CastFailure,
    Checked,
    Constraints,
    Door,
    ExcelEpoch,
    Fault,
    NumFormat,
    Rounding,
    Success,
    UnixPrecision,
    Violation,
)


def _corpus_dir() -> Path:
    # tests/corpus is a symlink to the root corpus/, so the vectors travel with this directory
    # when it is copied without the rest of the checkout: the Pyodide pass (the forge's
    # pyodide-suite action) copies tests/ alone into the interpreter's filesystem. Where the
    # link is checked out as a plain file (Windows without core.symlinks) it is not a
    # directory, and the walk carries on up to the root's own corpus/.
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
    # A key the vector leaves out declares nothing — the constructor's own default.
    return NumFormat(
        fmt["decimal_sep"],
        fmt["group_sep"],
        fmt["flags"],
        fmt.get("currency", ""),
        currency_decimal_sep=fmt.get("currency_decimal_sep", ""),
        currency_group_sep=fmt.get("currency_group_sep", ""),
        minus_sign=fmt.get("minus_sign", ""),
        plus_sign=fmt.get("plus_sign", ""),
        percent=fmt.get("percent", ""),
        digit_zero=fmt.get("digit_zero", ""),
    )


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
    """Replays corpus/boolean.json."""
    for vector in _corpus("boolean.json"):
        _assert_verdict("boolean", vector, hypercast.cast_bool(_input(vector)), vector.get("value"))


def test_char_corpus():
    """Replays corpus/char.json; the vector pins the code point, the door returns the str."""
    for vector in _corpus("char.json"):
        verdict = hypercast.cast_char(_input(vector))
        if isinstance(verdict, Success):
            assert len(verdict.value) == 1, f"char: {vector['input']!r} -> {verdict.value!r}"
            verdict = Success(ord(verdict.value))
        _assert_verdict("char", vector, verdict, vector.get("value"))


_INT_DOORS = {
    "i8": hypercast.cast_i8,
    "i16": hypercast.cast_i16,
    "i32": hypercast.cast_i32,
    "i64": hypercast.cast_i64,
    "u8": hypercast.cast_u8,
    "u16": hypercast.cast_u16,
    "u32": hypercast.cast_u32,
    "u64": hypercast.cast_u64,
}


def test_integer_corpus():
    """Replays corpus/integer.json through every integer width."""
    for vector in _corpus("integer.json"):
        door = _INT_DOORS[vector["type"]]
        # Python int is unbounded — u64's full unsigned value comes back natively.
        _assert_verdict(
            "integer", vector, door(_input(vector), _format_of(vector)), vector.get("value")
        )


def _bits(value: float) -> bytes:
    """A float by its bits, so zero has one form: ``-0.0 == 0.0``, but their bits differ."""
    return struct.pack(">d", value)


def _real(kind: str, value: float) -> float:
    """The corpus's value as the door delivers it: an f32 widened, losslessly, to a float."""
    return struct.unpack("f", struct.pack("f", value))[0] if kind == "f32" else value


def test_real_corpus():
    """Replays corpus/real.json through both real doors, comparing values by their bits."""
    for vector in _corpus("real.json"):
        door = hypercast.cast_f32 if vector["type"] == "f32" else hypercast.cast_f64
        expected = vector.get("value")
        if expected is not None:
            expected = _real(vector["type"], expected)
        verdict = door(_input(vector), _format_of(vector))
        _assert_verdict("real", vector, verdict, expected)
        if isinstance(verdict, Success):
            assert _bits(verdict.value) == _bits(expected), f"real: {vector['input']!r} bits"


def test_decimal_corpus():
    """Replays corpus/decimal.json, pinning the canonical scale as well as the value."""
    for vector in _corpus("decimal.json"):
        verdict = hypercast.cast_decimal(_input(vector), _format_of(vector))
        expected = Decimal(vector["value"]) if "value" in vector else None
        _assert_verdict("decimal", vector, verdict, expected)
        if isinstance(verdict, Success):
            # Equality alone would let "1.10" pass for "1.1": pin the canonical scale the core
            # produced, digit for digit, through Decimal's own (sign, digits, exponent) storage.
            sign, digits, exponent = verdict.value.as_tuple()
            text = vector["input"]
            assert sign == int(vector["negative"]), f"decimal: {text!r} sign"
            assert digits == tuple(int(d) for d in str(int(vector["magnitude"]))), (
                f"decimal: {text!r} digits"
            )
            assert exponent == -vector["scale"], f"decimal: {text!r} scale"


def test_uuid_corpus():
    """Replays corpus/uuid.json."""
    for vector in _corpus("uuid.json"):
        expected = uuidlib.UUID(hex=vector["value"]) if "value" in vector else None
        _assert_verdict("uuid", vector, hypercast.cast_uuid(_input(vector)), expected)


_EPOCH = dt.datetime(1970, 1, 1, tzinfo=dt.timezone.utc)


def _expected_instant(vector: dict) -> dt.datetime | None:
    if "seconds" not in vector:
        return None
    return _EPOCH + dt.timedelta(seconds=vector["seconds"], microseconds=vector["nanos"] // 1000)


def test_timestamp_corpus():
    """Replays corpus/timestamp.json."""
    for vector in _corpus("timestamp.json"):
        _assert_verdict(
            "timestamp", vector, hypercast.cast_timestamp(_input(vector)), _expected_instant(vector)
        )


def test_unix_corpus():
    """Replays corpus/unix.json."""
    for vector in _corpus("unix.json"):
        precision = UnixPrecision(vector["precision"])
        _assert_verdict(
            "unix",
            vector,
            hypercast.cast_unix(_input(vector), precision),
            _expected_instant(vector),
        )


def test_excel_serial_corpus():
    """Replays corpus/excel_serial.json."""
    for vector in _corpus("excel_serial.json"):
        epoch = ExcelEpoch(vector["epoch"])
        _assert_verdict(
            "excel_serial",
            vector,
            hypercast.cast_excel_serial(_input(vector), epoch),
            _expected_instant(vector),
        )


def test_date_corpus():
    """Replays corpus/date.json."""
    for vector in _corpus("date.json"):
        expected = (
            dt.date(vector["year"], vector["month"], vector["day"]) if "year" in vector else None
        )
        _assert_verdict("date", vector, hypercast.cast_date(_input(vector)), expected)


def test_date_order_corpus():
    """Replays corpus/date_order.json."""
    for vector in _corpus("date_order.json"):
        order = hypercast.DateOrder(vector["order"])
        expected = (
            dt.date(vector["year"], vector["month"], vector["day"]) if "year" in vector else None
        )
        _assert_verdict("date_order", vector, hypercast.cast_date(_input(vector), order), expected)


def test_datetime_corpus():
    """Replays corpus/datetime.json."""
    for vector in _corpus("datetime.json"):
        order = hypercast.DateOrder(vector["order"])
        expected = None
        if "year" in vector:
            second_of_day, nano = divmod(vector["nanos_of_day"], 1_000_000_000)
            hour, rest = divmod(second_of_day, 3600)
            minute, second = divmod(rest, 60)
            expected = dt.datetime(
                vector["year"], vector["month"], vector["day"], hour, minute, second, nano // 1000
            )
        # Declared day periods ride beside the order; a vector without them reads English only.
        verdict = hypercast.cast_datetime(
            _input(vector), order, am=vector.get("am", ""), pm=vector.get("pm", "")
        )
        _assert_verdict("datetime", vector, verdict, expected)


def test_time_corpus():
    """Replays corpus/time.json."""
    for vector in _corpus("time.json"):
        expected = None
        if "nanos" in vector:
            second_of_day, nano = divmod(vector["nanos"], 1_000_000_000)
            hour, rest = divmod(second_of_day, 3600)
            minute, second = divmod(rest, 60)
            expected = dt.time(hour, minute, second, nano // 1000)
        _assert_verdict("time", vector, hypercast.cast_time(_input(vector)), expected)


def test_duration_corpus():
    """Replays corpus/duration.json."""
    for vector in _corpus("duration.json"):
        expected = None
        if "seconds" in vector:
            nanos = vector["nanos"]
            micros = nanos // 1000 if nanos >= 0 else -((-nanos) // 1000)
            expected = dt.timedelta(seconds=vector["seconds"], microseconds=micros)
        _assert_verdict("duration", vector, hypercast.cast_duration(_input(vector)), expected)


def test_typed_corpus():
    """Each double is named by its bits, which rebuild it exactly — NaN and the infinities included,
    which JSON cannot spell. A typed door's fault has no span.
    """
    doors = set()
    for vector in _corpus("typed.json"):
        value = struct.unpack(">d", bytes.fromhex(vector["bits"]))[0]
        door = vector["door"]
        doors.add(door)
        expected = None
        if door == "decimal":
            verdict = hypercast.cast_decimal_from_float(value)
            if "value" in vector:
                expected = Decimal(vector["value"])
        elif door == "excel_serial":
            verdict = hypercast.cast_excel_serial_from_float(value, ExcelEpoch(vector["epoch"]))
            if "year" in vector:
                second_of_day, nano = divmod(vector["nanos_of_day"], 1_000_000_000)
                hour, rest = divmod(second_of_day, 3600)
                minute, second = divmod(rest, 60)
                expected = dt.datetime(
                    vector["year"],
                    vector["month"],
                    vector["day"],
                    hour,
                    minute,
                    second,
                    nano // 1000,
                )
        elif door == "excel_time":
            verdict = hypercast.cast_excel_time(value)
            if "nanos" in vector:
                second_of_day, nano = divmod(vector["nanos"], 1_000_000_000)
                hour, rest = divmod(second_of_day, 3600)
                minute, second = divmod(rest, 60)
                expected = dt.time(hour, minute, second, nano // 1000)
        elif door == "excel_duration":
            verdict = hypercast.cast_excel_duration(value)
            if "seconds" in vector:
                nanos = vector["nanos"]
                micros = nanos // 1000 if nanos >= 0 else -((-nanos) // 1000)
                expected = dt.timedelta(seconds=vector["seconds"], microseconds=micros)
        else:
            raise AssertionError(f"typed: unknown door {door}")
        _assert_verdict(f"typed {door}", vector, verdict, expected)
        if isinstance(verdict, Fault):
            assert (verdict.offset, verdict.length) == (0, 0), f"typed {door}: span"
        elif door == "decimal":
            sign, digits, exponent = verdict.value.as_tuple()
            assert (sign, exponent) == (int(vector["negative"]), -vector["scale"]), vector["input"]
            assert digits == tuple(int(d) for d in vector["magnitude"]), vector["input"]
    assert doors == {"decimal", "excel_serial", "excel_time", "excel_duration"}


# --- validation constraints --------------------------------------------------------------

_DOORS = {
    "i8": Door.I8,
    "i16": Door.I16,
    "i32": Door.I32,
    "i64": Door.I64,
    "u8": Door.U8,
    "u16": Door.U16,
    "u32": Door.U32,
    "u64": Door.U64,
    "f32": Door.F32,
    "f64": Door.F64,
    "decimal": Door.DECIMAL,
    "text": Door.TEXT,
}

_ROUNDINGS = {
    "strict": Rounding.STRICT,
    "midpoint": Rounding.MIDPOINT,
    "floor": Rounding.FLOOR,
    "ceiling": Rounding.CEILING,
}

#: The core's Refusal as Rust displays it: the wording every binding's message carries.
_REFUSALS = [
    "",
    "a constraint the door does not take",
    "minimum together with exclusiveMinimum",
    "maximum together with exclusiveMaximum",
    "the lower bound is not below the upper bound",
    "multipleOf is not positive",
    "a rounding without multipleOf",
    "minLength is above maxLength",
    "a bound, step or enum entry the door cannot hold",
    "an enum entry that is not UTF-8",
    "a duplicate enum entry",
    "a malformed constraints layout",
    "an enum with no entries",
    "a text enum entry no input can match",
]


def refusal_message(code: int) -> str:
    """The message a refused declaration raises with, one per code."""
    return f"Constraints refused (code {code}): {_REFUSALS[code]}."


def _keywords(c: dict) -> dict:
    """Every keyword a vector names, whether or not its door takes it, as the public
    constructor's keyword arguments: bounds, steps and entries as the corpus spells them."""
    return {
        "minimum": c.get("minimum"),
        "exclusive_minimum": c.get("exclusiveMinimum"),
        "maximum": c.get("maximum"),
        "exclusive_maximum": c.get("exclusiveMaximum"),
        "multiple_of": c.get("multipleOf"),
        "rounding": _ROUNDINGS[c.get("rounding", "strict")],
        "min_length": c.get("minLength"),
        "max_length": c.get("maxLength"),
        "trim": c.get("trim", False),
        "enum": c.get("enum"),
    }


#: The doubles JSON cannot spell, as the corpus names them.
_SPELLED = {"-0": -0.0, "NaN": float("nan"), "Infinity": float("inf"), "-Infinity": float("-inf")}


def _from_f64(value) -> float:
    return _SPELLED[value] if isinstance(value, str) else float(value)


def _label(vector: dict) -> str:
    what = vector.get("input", vector.get("from_f64", vector.get("input_bytes")))
    return f"constraints {vector['door']} {vector['constraints']} {what!r}"


def _code_points(text: str, byte_offset: int) -> int:
    return len(text.encode("utf-8")[:byte_offset].decode("utf-8", errors="ignore"))


def _assert_checked(label: str, vector: dict, verdict, span) -> None:
    """One checked verdict against its row: the value by bits (a float), by its exact
    (sign, digits, exponent) (a decimal), or as it is; ``adjusted`` and ``enum_index``; or the
    reason or keyword and the span, which ``span`` re-expresses in the input's own units."""
    expect = vector["expect"]
    match verdict:
        case Checked(value, adjusted, enum_index):
            assert expect == "ok", f"{label}: unexpectedly passed as {verdict!r}"
            want = vector["value"]
            door = vector["door"]
            if door in ("f32", "f64"):
                assert type(value) is float, label
                assert _bits(value) == _bits(_real(door, want)), f"{label}: {value!r} bits"
            elif door == "decimal":
                assert value.as_tuple() == Decimal(want).as_tuple(), f"{label}: {value!r}"
            else:
                assert type(value) is type(want) and value == want, f"{label}: {value!r}"
            assert adjusted is vector["adjusted"], f"{label}: adjusted"
            assert enum_index == vector.get("enum_index"), f"{label}: enum_index"
        case Violation(keyword, offset, length):
            assert expect == "violation", f"{label}: {verdict!r}"
            assert keyword.name == vector["keyword"], f"{label}: keyword"
            assert (offset, length) == span(*vector["fault"]), f"{label}: violation span"
        case Fault(reason, offset, length):
            assert expect in _EXPECTED_REASON, f"{label}: expected {expect} but {verdict!r}"
            assert reason is _EXPECTED_REASON[expect], f"{label}: {reason}"
            assert (offset, length) == span(*vector["fault"]), f"{label}: fault span"
        case other:  # pragma: no cover - the union is closed
            raise AssertionError(f"{label}: produced no case: {other!r}")


def test_constraints_corpus():
    """Replays corpus/constraints.json, every row, through the public Constraints constructor
    (so bounds, steps and entries cross this binding's own conversion): a refused row raises
    ValueError with its code's message, and cast_constraints_check answers that code. Text
    rows go through cast_text and numeric rows through cast_checked, each as bytes (byte
    spans) and as str (code-point spans); from_f64 rows through cast_checked_from_float.
    """
    seen = set()
    vectors = _corpus("constraints.json")
    replayed = 0
    for vector in vectors:
        label = _label(vector)
        door = _DOORS[vector["door"]]
        keywords = _keywords(vector["constraints"])
        if vector["expect"] == "refused":
            seen.add("refused")
            code = vector["refusal"]
            with pytest.raises(ValueError, match=re.escape(refusal_message(code))):
                Constraints(door, **keywords)
            assert hypercast.cast_constraints_check(door, **keywords) == code, label
            replayed += 1
            continue
        seen.add(vector["door"])
        assert hypercast.cast_constraints_check(door, **keywords) == 0, label
        constraints = Constraints(door, **keywords)

        def same(offset, length):
            return (offset, length)

        if "from_f64" in vector:
            seen.add("from_f64")
            verdict = hypercast.cast_checked_from_float(_from_f64(vector["from_f64"]), constraints)
            _assert_checked(label, vector, verdict, same)
            replayed += 1
            continue
        if "input_bytes" in vector:
            seen.add("input_bytes")
            verdict = hypercast.cast_text(bytes(vector["input_bytes"]), constraints)
            _assert_checked(label, vector, verdict, same)
            replayed += 1
            continue
        text = vector["input"]

        def chars(offset, length, text=text):
            start = _code_points(text, offset)
            return (start, _code_points(text, offset + length) - start)

        if door is Door.TEXT:
            _assert_checked(label, vector, hypercast.cast_text(text.encode(), constraints), same)
            _assert_checked(f"{label} (str)", vector, hypercast.cast_text(text, constraints), chars)
        else:
            fmt = _format_of(vector)
            verdict = hypercast.cast_checked(text.encode(), fmt, constraints)
            _assert_checked(label, vector, verdict, same)
            verdict = hypercast.cast_checked(text, fmt, constraints)
            _assert_checked(f"{label} (str)", vector, verdict, chars)
        replayed += 1
    assert seen == set(_DOORS) | {"refused", "from_f64", "input_bytes"}, seen
    # Every row was replayed: the count is the file's, never a literal.
    assert replayed == len(vectors), (replayed, len(vectors))


def test_declarations_corpus():
    """Replays corpus/declarations.json through the conversion every bound, step and integer
    enum entry crosses: cast_declared_decimal, and a Constraints maximum at the decimal door,
    whose packed layout carries the raw triple (scale included, which Decimal equality would
    ignore) at bytes 40 to 56. A refused row raises ValueError with code 8."""
    counts = {"ok": 0, "refused": 0, "bytes": 0}
    vectors = _corpus("declarations.json")
    for vector in vectors:
        if "input_bytes" in vector:
            counts["bytes"] += 1
            assert vector["expect"] == "refused" and vector["refusal"] == 8
            with pytest.raises(ValueError, match=re.escape(refusal_message(8))):
                hypercast.cast_declared_decimal(bytes(vector["input_bytes"]))
            continue
        text = vector["input"]
        if vector["expect"] == "refused":
            counts["refused"] += 1
            assert vector["refusal"] == 8
            with pytest.raises(ValueError, match=re.escape(refusal_message(8))):
                hypercast.cast_declared_decimal(text)
            with pytest.raises(ValueError, match=re.escape(refusal_message(8))):
                Constraints(Door.DECIMAL, maximum=text)
            continue
        counts["ok"] += 1
        magnitude, scale, negative = int(vector["magnitude"]), vector["scale"], vector["negative"]
        raw = Constraints(Door.DECIMAL, maximum=text).packed[40:56]
        lo, hi, raw_scale, raw_negative = struct.unpack("=QIBB2x", raw)
        assert (lo | hi << 64, raw_scale, bool(raw_negative)) == (magnitude, scale, negative), text
        sign, digits, exponent = hypercast.cast_declared_decimal(text).as_tuple()
        assert int("".join(map(str, digits))) == magnitude, text
        assert (-exponent, bool(sign)) == (scale, negative), text
        assert format(hypercast.cast_declared_decimal(text), "f") == vector["value"], text
    # Every row was replayed, and every kind is present: counts from the file, never literals.
    assert sum(counts.values()) == len(vectors), (counts, len(vectors))
    assert all(counts.values()), counts


#: Every plain door and its parameters, as they were before the checked doors joined: the
#: constraints are additive, so none of these may change.
_PLAIN_DOORS = {
    "cast_bool": ["text"],
    "cast_char": ["text"],
    **{f"cast_{kind}": ["text", "fmt"] for kind in ["i8", "i16", "i32", "i64", "u8", "u16"]},
    **{f"cast_{kind}": ["text", "fmt"] for kind in ["u32", "u64", "f32", "f64", "decimal"]},
    "cast_uuid": ["text"],
    "cast_timestamp": ["text"],
    "cast_unix": ["text", "precision"],
    "cast_excel_serial": ["text", "epoch"],
    "cast_date": ["text", "order"],
    "cast_datetime": ["text", "order", "am", "pm"],
    "cast_time": ["text"],
    "cast_duration": ["text"],
    "cast_decimal_from_float": ["value"],
    "cast_excel_serial_from_float": ["value", "epoch"],
    "cast_excel_time": ["value"],
    "cast_excel_duration": ["value"],
}


def test_the_plain_verdict_keeps_its_shape():
    """The plain doors' verdict is exactly what it was: Success(value) or Fault(reason, offset,
    length) over the three CastFailure codes, every plain door keeps its parameters, and none
    answers with a checked case."""
    assert Success.__match_args__ == ("value",)
    assert Fault.__match_args__ == ("reason", "offset", "length")
    assert [(m.name, m.value) for m in CastFailure] == [
        ("EMPTY", 1),
        ("MALFORMED", 2),
        ("OUT_OF_RANGE", 3),
    ]
    assert hypercast.Verdict[int].__args__ == (Success[int], Fault)
    for name, parameters in _PLAIN_DOORS.items():
        door = getattr(hypercast, name)
        assert list(inspect.signature(door).parameters) == parameters, name
    assert type(hypercast.cast_i32("1", NumFormat.INVARIANT)) is Success
    assert type(hypercast.cast_i32("x", NumFormat.INVARIANT)) is Fault
    assert type(hypercast.cast_text("x")) is Success
    assert type(hypercast.cast_text(b"\xff")) is Fault
