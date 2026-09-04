# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/hypercast.svg)](https://pypi.org/project/hypercast/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**`match`/`case` over a two-case verdict — the value, or a closed reason plus the exact
byte span that offended — with the Rust core linked straight into CPython as a native
extension. No dlopen, no ctypes marshalling, no runtime bridge.**

Allocation-lean scalar casts — booleans, the full integer family, reals, UUIDs, temporals.
The PyO3 extension (`hypercast._native`) is the backend every wheel ships — a door is an
ordinary `METH_FASTCALL` extension call into a direct Rust call, and the wheel maturin
builds is the whole package (the interim ctypes fallback is gone). A second backend runs the
same core as a `wasm32-wasip1` module inside CPython through `wasmtime-py`, opt-in via
`pip install hypercast[wasm]` and `HYPERCAST_WASM=1` — see
[WebAssembly (wasmtime)](#webassembly-wasmtime). Python 3.10 is the floor (`match`/`case`
is the consumption idiom); wheels are abi3-py310, one per platform covering every CPython
from there up, no compiler needed to install.

```python
import hypercast

match hypercast.cast_i32("(1,234)", hypercast.NumFormat.INVARIANT):
    case hypercast.Success(value):
        print("got", value)                       # -1234, accounting negative
    case hypercast.Fault(reason, offset, length):
        print(reason.name, "at byte", offset)
```

Door names mirror the native ABI (`cast_i32`, `cast_f64`, `cast_timestamp`, …); inputs are
`str` or `bytes`, both zero-copy views. Exhaustiveness is the type checker's job here —
pair the two cases with `typing.assert_never` under mypy/pyright for the compile-time
guarantee the static bindings get natively. Python-flavored fidelity, stated honestly:
`int` is unbounded, so `cast_u64` returns the true unsigned value with no bit-pattern
games; `datetime`'s resolution is microseconds, so the core's nanoseconds truncate by
three digits on the temporal doors — `datetime`'s own ceiling, not the parser's, and said
out loud rather than discovered later.

## Why not `int()` / `fromisoformat` / `dateutil`?

1. **Verdicts, not exceptions** — bad data is the expected case for untrusted text, and a
   `Fault` is a reason plus a span, not a `ValueError` to catch and regex.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, radix prefixes, all five .NET `Guid` text forms plus
   `urn:uuid:` prefixes, protobuf JSON durations — with each lenience individually
   declared, never guessed.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (the whole suite green on both backends, full twelve-file
   corpus replay).
4. **Native-extension speed** — the escape from the interpreted tier is this binding's own
   receipt: the old losses were never "Python calling native code," they were *ctypes*
   (~1 µs of interpreted marshalling per call, measured). With the mechanism replaced,
   every door runs 10-18x faster than it did over ctypes (pyperf, linux-arm64): timestamp
   3.07 µs → **201 ns** — near-parity with C-accelerated `fromisoformat` (163 ns) while
   returning verdicts instead of exceptions; i32 at **146 ns** vs `int()`'s 88; and the
   forgiveness doors at ~180 ns for grammar the stdlib doesn't sell at any price. The uuid
   door used to sit at parity with `uuid.UUID()` because both were bounded by
   `UUID.__init__`; it now builds the instance the way HyperUuid pinned — `UUID.__new__`
   plus `object.__setattr__` of the `int` and `is_safe` slots, skipping an `__init__` whose
   validation the core has already done — and measures **730 ns against 1.18 µs** before
   the change, same machine, same session, ahead of `uuid.UUID()`'s 979 ns.

   The messy-feed doors are where the gap is widest, because `strptime` is the only stdlib
   parser that accepts their input at all — and it is *slow*:

   | Door | HyperCast | stdlib | Verdict |
   | --- | ---: | ---: | --- |
   | `cast_datetime("1/7/2026 3:04 PM", MONTH_DAY_YEAR)` | 409 ns | 5.19 µs `datetime.strptime` | **12.7x faster** |
   | `cast_date("1/7/2026", MONTH_DAY_YEAR)` | 292 ns | 3.86 µs `datetime.strptime` | **13.2x faster** |

   Separator detection costs ~18 ns: `1.234.567,89` under `NumFormat.DETECT` is 216 ns
   against 198 ns for the same text under a declared eurozone format.
   Reproduce: `python bench_cast.py --fast` (pyperf, in the `bench` extra).

**The honest trade-off:** for plain invariant integers `int()` still wins — it's a
C-accelerated builtin with no boundary to cross. These doors earn their keep on the
culture-machinery parsers, the closed error contract, and cross-language agreement. And
dropping ctypes means dropping the Pyodide path Python briefly had — the wheels are real
native extensions, and a native extension has no browser story. The wasm backend below is
the other direction entirely: the core as wasm inside an ordinary CPython, not CPython
inside a browser.

## WebAssembly (wasmtime)

The same Rust core, compiled to `wasm32-wasip1`, run *inside* CPython by
[`wasmtime-py`](https://github.com/bytecodealliance/wasmtime-py) — the inverse of the Pyodide
experiment this package once carried (CPython itself in the browser, loading the core as an
Emscripten side module). Nothing is reimplemented: `hypercast._wasm` calls the identical
twenty `cast_*` C-ABI exports the PyO3 extension does, across a guest/host memory boundary
instead of a direct call, and presents the same `Success`/`Fault`/`NumFormat` types with the
same `__match_args__`, equality, `repr` and error messages. The whole test suite runs against
it, corpus replay included, and `tests/test_wasm_backend.py` pins its outputs against the
extension across a subprocess boundary.

```sh
pip install hypercast[wasm]        # adds wasmtime; the .wasm module ships inside every wheel and the sdist
HYPERCAST_WASM=1 python app.py     # force it; hypercast.BACKEND reports "wasm" or "native"
```

Without the variable, `_native` is used whenever it imports, and `_wasm` is the fallback when it
does not and `wasmtime` is installed — an install whose extension cannot load keeps working
instead of failing at import. One honest limit on that story today: pip still resolves an
interpreter with no matching wheel to the sdist, and the sdist builds the PyO3 extension, so it
needs a Rust toolchain either way. A pure-Python wheel carrying only the wasm backend is what
would make `pip install hypercast[wasm]` land with nothing to compile anywhere; it is not built
yet.

Three things about the crossing decide the numbers below:

- **Buffers come from the guest.** A wasm module only sees its own linear memory, so this backend
  asks the module's exported `malloc` for every buffer it touches — the input text (a grow-only
  buffer), the 16-byte out-value, the fault span, the `NumFormat` — rather than picking an offset
  itself. That is load-bearing, not tidiness: the guest's own allocator (dlmalloc, which claims
  the tail of the initial memory on first use) corrupted a host-chosen buffer in HyperUuid.
- **Calls are serialized.** A wasmtime `Store` is not thread-safe, so one process-wide lock
  guards every call. Uncontended under the GIL; on a free-threaded build it is what keeps two
  threads out of one store.
- **The call path sidesteps wasmtime-py's per-call type lookup.** `Func.__call__` re-fetches the
  function's type from the engine and builds and frees a `FuncType` plus one `ValType` wrapper per
  parameter and result on *every* call. This backend builds the argument and result arrays once
  and hands them to the same `wasmtime_func_call` C entry point the library reaches after that
  bookkeeping. That touches `wasmtime._ffi`, which is not public API, so it is bound inside a
  `try` at load time and degrades to the public call — slow, never broken — if a wasmtime release
  moves it.

Measured end to end on CPython 3.14.7, linux-arm64 (WSL2), `timeit` best of five, same session
as the native column:

| Door | wasm backend | native (`_native`) |
| --- | ---: | ---: |
| `cast_bool` | 6.0 µs | 99 ns |
| `cast_i32` | 6.4 µs | 139 ns |
| `cast_f64` | 6.4 µs | 153 ns |
| `cast_uuid` | 6.9 µs | 681 ns |
| `cast_timestamp` | 7.7 µs | 423 ns |
| `cast_datetime` (`1/7/2026 3:04 PM`) | 7.2 µs | 407 ns |
| `cast_duration` (ISO) | 7.5 µs | 662 ns |
| `cast_i32`, a fault | 6.8 µs | 138 ns |

Read it the way the rest of this README reads: every door pays the crossing — roughly 6 µs of
lock, argument packing, guest memory copies and the call itself — and the parse underneath is
invisible next to it. There is no batch door here to amortize that behind, so this backend is
the answer to "no wheel for this interpreter", not a speed option; the object-building doors
(`uuid`, `timestamp`) close the gap a little only because their native carrier is already the
expensive part.

## Verifying provenance

Every wheel PyPI serves carries a GitHub build-provenance attestation, signed directly by
this repo's own `release.yml` (the `pypi-build-wheels` job attests each platform wheel
right where it's built, no reusable workflow in between), so plain `--repo` verifies it:

```sh
pip download hypercast==X.Y.Z --no-deps -d .
gh attestation verify hypercast-X.Y.Z-*.whl --repo SkunkWerkx/HyperCast
```

This is a separate thing from the [PEP 740](https://peps.python.org/pep-0740/) attestations
`gh-action-pypi-publish` already sends to PyPI itself, which PyPI-side tooling checks on its
own — this is the GitHub/Sigstore transparency-log route, checked with `gh attestation
verify`, the same route every other artifact in this project uses. See
[csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance) for why
some artifacts here need `--signer-repo` and this one doesn't.

## Install

```sh
pip install hypercast
```

abi3 wheels for all six platforms, so one wheel per platform covers every CPython 3.10+.
No compiler needed to install and no dependencies at all — the PyO3 extension *is* the
package.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.
