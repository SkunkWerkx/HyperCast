"""Many threads at once: on a free-threaded CPython they really run in parallel, so these are
where a GIL assumption in the extension would show. A GIL build runs them too, serialized."""

from __future__ import annotations

import os
import sys
import sysconfig
import threading

import pytest

import hypercast
from hypercast import DateOrder, ExcelEpoch, NumFormat, UnixPrecision

pytestmark = pytest.mark.skipif(sys.platform == "emscripten", reason="Pyodide has no threads")

THREADS = 8
ROUNDS = 500

GROUPED = NumFormat(",", ".", NumFormat.ALL, "€")
ARABIC = NumFormat(
    "\u066b",
    "\u066c",
    NumFormat.ALL | NumFormat.TRAILING_SIGN,
    "\u062f.\u0625.",
    minus_sign="\u2212",
    percent="\u066a",
    digit_zero="\u0660",
)

#: One call per door shape: text and bytes, success and every fault reason, a non-ASCII str
#: whose fault span is remapped to code points, and each value type the package builds.
CALLS = [
    lambda: hypercast.cast_bool("yes"),
    lambda: hypercast.cast_bool(b"maybe"),
    lambda: hypercast.cast_char("U+00E9"),
    lambda: hypercast.cast_i32("(1,234)", NumFormat.INVARIANT),
    lambda: hypercast.cast_i8(b"300", NumFormat.INVARIANT),
    lambda: hypercast.cast_i64("", NumFormat.INVARIANT),
    lambda: hypercast.cast_u64("18446744073709551615", NumFormat.INVARIANT),
    lambda: hypercast.cast_f64("1.5e3", NumFormat.INVARIANT),
    lambda: hypercast.cast_decimal("€1.234,50", GROUPED),
    lambda: hypercast.cast_decimal("1.234,5x€", GROUPED),
    lambda: hypercast.cast_f64("\u0661\u066c\u0662\u0663\u0664\u066b\u0665\u066a\u2212", ARABIC),
    lambda: hypercast.cast_decimal_from_float(0.1),
    lambda: hypercast.cast_uuid("01020304-0506-4708-890a-0b0c0d0e0f10"),
    lambda: hypercast.cast_uuid(b"not-a-uuid"),
    lambda: hypercast.cast_timestamp("2026-10-19T12:34:56.789123456+02:00"),
    lambda: hypercast.cast_unix("1700000000123", UnixPrecision.MILLISECONDS),
    lambda: hypercast.cast_excel_serial("45000.5", ExcelEpoch.Y1900),
    lambda: hypercast.cast_excel_serial_from_float(45000.25, ExcelEpoch.Y1900),
    lambda: hypercast.cast_excel_time(0.75),
    lambda: hypercast.cast_excel_duration(-1.5),
    lambda: hypercast.cast_date("2026-10-19"),
    lambda: hypercast.cast_date("19/10/2026", DateOrder.DAY_MONTH_YEAR),
    lambda: hypercast.cast_datetime("10/19/2026 1:02 PM", DateOrder.MONTH_DAY_YEAR),
    lambda: hypercast.cast_datetime(
        "2026-10-19 오후 1:02", DateOrder.YEAR_MONTH_DAY, am="오전", pm="오후"
    ),
    lambda: hypercast.cast_time("23:59:59.999999"),
    lambda: hypercast.cast_duration("P1DT2H3M4.5S"),
    lambda: hypercast.cast_duration("-3.25s"),
]


def _run_together(work) -> list:
    """Runs ``work(i)`` on THREADS threads released at once, and returns what each returned."""
    results: list = [None] * THREADS
    errors: list = []
    barrier = threading.Barrier(THREADS)

    def run(i: int) -> None:
        barrier.wait()
        try:
            results[i] = work(i)
        except BaseException as error:  # noqa: BLE001 - re-raised on the main thread below
            errors.append(error)

    workers = [threading.Thread(target=run, args=(i,)) for i in range(THREADS)]
    for worker in workers:
        worker.start()
    for worker in workers:
        worker.join()
    if errors:
        raise errors[0]
    return results


@pytest.mark.skipif(
    not sysconfig.get_config_var("Py_GIL_DISABLED"), reason="needs a free-threaded CPython"
)
@pytest.mark.skipif("PYTHON_GIL" in os.environ, reason="PYTHON_GIL overrides the module's choice")
def test_import_leaves_the_gil_off():
    """The extension declares it needs no GIL, so importing it doesn't switch the GIL back on."""
    assert not sys._is_gil_enabled()


def test_every_door_from_many_threads_at_once_gives_the_single_threaded_verdicts():
    """Eight threads replaying every door shape at once get exactly the verdicts one thread
    gets: the cached CastFailure members, the shared value constructors and the formats are
    only ever read, so nothing one call does can reach another's."""
    expected = [call() for call in CALLS]

    def work(i: int) -> list:
        mismatches = []
        for round_ in range(ROUNDS):
            # Each thread walks the doors from a different starting point, so different
            # doors really do run against each other.
            for k in range(len(CALLS)):
                index = (k + i + round_) % len(CALLS)
                got = CALLS[index]()
                if got != expected[index]:
                    mismatches.append((index, got))
        return mismatches

    for mismatches in _run_together(work):
        assert mismatches == []


def test_threads_sharing_one_fresh_non_ascii_str_all_read_it_whole():
    """A door reads a ``str`` through its cached UTF-8, which a non-ASCII string builds on
    first use. Here every thread hands the same fresh strings to a door at once, so that first
    use is raced, and each must still see the whole text: the same value, and the same
    code-point fault span."""
    rounds = 2_000

    def fresh(n: int) -> tuple[str, str]:
        # Built at run time, so no cached UTF-8 exists until a door asks for it.
        return "".join(["€", str(n), ",5"]), "".join(["€", str(n), ",5é"])

    texts = [fresh(n) for n in range(rounds)]

    def work(i: int) -> list:
        mismatches = []
        for n, (good, bad) in enumerate(texts):
            ok, fault = hypercast.cast_decimal(good, GROUPED), hypercast.cast_decimal(bad, GROUPED)
            if not isinstance(ok, hypercast.Success) or str(ok.value) != f"{n}.5":
                mismatches.append((n, ok))
            # The fault span is in code points: "€" is one, though three bytes in UTF-8.
            if not isinstance(fault, hypercast.Fault) or fault.offset != len(bad) - 1:
                mismatches.append((n, fault))
        return mismatches

    for mismatches in _run_together(work):
        assert mismatches == []


def test_localeconv_bridges_from_many_threads_agree():
    """from_localeconv reads a caller's dict; many threads bridging the same one at once each
    get the same format."""
    conv = {
        "decimal_point": ",",
        "thousands_sep": "",
        "currency_symbol": "kr",
        "mon_decimal_point": ",",
        "mon_thousands_sep": "\u00a0",
        "negative_sign": "\u2212",
        "n_sign_posn": 2,
    }
    expected = repr(NumFormat.from_localeconv(conv))
    results = _run_together(
        lambda i: {repr(NumFormat.from_localeconv(conv)) for _ in range(ROUNDS)}
    )
    for seen in results:
        assert seen == {expected}
