"""Pins the wasm backend's selection and its agreement with the native one. The whole main
suite already runs under both (``HYPERCAST_WASM=1 pytest`` forces wasm); this file pins the
*agreement* between them by comparing deterministic outputs across a subprocess boundary —
the same shape as the Ruby binding's backend-agreement specs. Skipped when the process under
test is not the wasm backend, so a plain ``pytest`` still exercises the native pin."""

from __future__ import annotations

import datetime as dt
import os
import subprocess
import sys
import threading
from decimal import Decimal
from pathlib import Path

import pytest

import hypercast
from hypercast import CastFailure, Fault, NumFormat, Success

pytestmark = pytest.mark.skipif(
    hypercast.BACKEND != "wasm", reason=f"wasm backend not loaded (BACKEND={hypercast.BACKEND})"
)


def native_eval(expression: str) -> str:
    src = str(Path(__file__).resolve().parent.parent / "src")
    env = {k: v for k, v in os.environ.items() if k != "HYPERCAST_WASM"}
    env["PYTHONPATH"] = src
    out = subprocess.run(
        [sys.executable, "-c", f"import hypercast; print({expression}, end='')"],
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )
    return out.stdout


def test_reports_the_wasm_backend():
    assert hypercast.BACKEND == "wasm"
    assert native_eval("hypercast.BACKEND") == "native"


def test_agrees_with_the_native_backend_on_a_declared_format():
    eurozone = NumFormat(",", ".", NumFormat.ALL)
    assert hypercast.cast_f64("1.234,5", eurozone) == Success(1234.5)
    assert native_eval(
        'repr(hypercast.cast_f64("1.234,5", hypercast.NumFormat(",", ".", hypercast.NumFormat.ALL)))'
    ) == repr(hypercast.cast_f64("1.234,5", eurozone))


def test_agrees_with_the_native_backend_on_a_fault_span():
    wasm = hypercast.cast_i32("  12x4", NumFormat.INVARIANT)
    assert wasm == Fault(CastFailure.MALFORMED, 4, 1)
    assert native_eval('repr(hypercast.cast_i32("  12x4", hypercast.NumFormat.INVARIANT))') == repr(wasm)


def test_agrees_with_the_native_backend_on_code_point_fault_spans():
    wasm_str = hypercast.cast_i32("1€", NumFormat.INVARIANT)
    wasm_bytes = hypercast.cast_i32("1€".encode(), NumFormat.INVARIANT)
    assert wasm_str == Fault(CastFailure.MALFORMED, 1, 1)
    assert wasm_bytes == Fault(CastFailure.MALFORMED, 1, 3)
    assert native_eval('repr(hypercast.cast_i32("1€", hypercast.NumFormat.INVARIANT))') == repr(wasm_str)
    assert native_eval('repr(hypercast.cast_i32("1€".encode(), hypercast.NumFormat.INVARIANT))') == repr(wasm_bytes)


def test_agrees_with_the_native_backend_on_the_temporal_doors():
    text = "2026-01-02T15:04:05.123456789+05:00"
    wasm = hypercast.cast_timestamp(text)
    assert wasm == Success(dt.datetime(2026, 1, 2, 10, 4, 5, 123456, tzinfo=dt.timezone.utc))
    assert native_eval(f'hypercast.cast_timestamp("{text}").value.isoformat()') == wasm.value.isoformat()
    assert native_eval('str(hypercast.cast_duration("-1.5s").value)') == str(hypercast.cast_duration("-1.5s").value)
    assert native_eval('str(hypercast.cast_time("15:04:05.123456789").value)') == str(
        hypercast.cast_time("15:04:05.123456789").value
    )


def test_agrees_with_the_native_backend_on_the_decimal_door():
    dollars = NumFormat(".", ",", NumFormat.ALL, "$")
    wasm = hypercast.cast_decimal("($1,234.50)", dollars)
    assert wasm == Success(Decimal("-1234.5"))
    assert wasm.value.as_tuple() == (1, (1, 2, 3, 4, 5), -1)
    assert native_eval(
        'repr(hypercast.cast_decimal("($1,234.50)", hypercast.NumFormat(".", ",", hypercast.NumFormat.ALL, "$")))'
    ) == repr(wasm)


def test_agrees_with_the_native_backend_on_the_version():
    assert native_eval("hypercast.native_version()") == hypercast.native_version()


def test_agrees_with_the_native_backend_on_uuid_construction():
    text = "urn:uuid:01020304-0506-0708-090a-0b0c0d0e0f10"
    wasm = hypercast.cast_uuid(text)
    assert native_eval(f'str(hypercast.cast_uuid("{text}").value)') == str(wasm.value)
    assert wasm.value.is_safe is __import__("uuid").SafeUUID.unknown


def test_input_buffer_only_ever_grows():
    # Grow-only guest input buffer: a long input after a short one, then the short one
    # again, must all come back intact — the regrowth cannot lose the staged bytes.
    padded = " " * 10_000 + "42" + " " * 10_000
    assert hypercast.cast_i32("7", NumFormat.INVARIANT) == Success(7)
    assert hypercast.cast_i32(padded, NumFormat.INVARIANT) == Success(42)
    assert hypercast.cast_i32("7", NumFormat.INVARIANT) == Success(7)
    # Trailing junk after the padding: the first offending byte is the space at 10002.
    assert hypercast.cast_i32(padded + "x", NumFormat.INVARIANT) == Fault(CastFailure.MALFORMED, 10_002, 1)


def test_format_memo_follows_the_object_not_the_value():
    # Two distinct but equal-valued formats must both write their bytes: the memo is by
    # identity, and a stale format would silently parse under the wrong separators.
    a = NumFormat(",", ".", NumFormat.ALL)
    b = NumFormat(".", ",", NumFormat.ALL)
    assert hypercast.cast_f64("1.234,5", a) == Success(1234.5)
    assert hypercast.cast_f64("1,234.5", b) == Success(1234.5)
    assert hypercast.cast_f64("1.234,5", a) == Success(1234.5)


def test_serializes_concurrent_callers_on_the_one_shared_instance():
    results: list[object] = []

    def work(n: int) -> None:
        for i in range(200):
            results.append(hypercast.cast_i32(str(n * 1000 + i), NumFormat.INVARIANT))

    threads = [threading.Thread(target=work, args=(n,)) for n in range(8)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    assert len(results) == 1600
    assert all(isinstance(r, Success) for r in results)
    assert sorted(r.value for r in results) == sorted(n * 1000 + i for n in range(8) for i in range(200))
