# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/hypercast.svg)](https://pypi.org/project/hypercast/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**`match`/`case` over a two-case verdict — the value, or a closed reason plus the exact
byte span that offended — with the Rust core linked straight into CPython as a native
extension. No dlopen, no ctypes marshalling, no runtime bridge.**

Allocation-lean scalar casts — booleans, the full integer family, reals, exact decimals,
UUIDs, temporals.
The PyO3 extension (`hypercast._native`) is the backend every wheel ships — a door is an
ordinary `METH_FASTCALL` extension call into a direct Rust call, and the wheel maturin
builds is the whole package (the interim ctypes fallback is gone). A second backend runs the
same core as a `wasm32-wasip1` module inside CPython through `wasmtime-py`, opt-in via
`pip install hypercast[wasm]` and `HYPERCAST_WASM=1` — see
[WebAssembly (wasmtime)](#webassembly-wasmtime). Python 3.11 is the floor, the oldest version still in upstream support (`match`/`case` is
the consumption idiom).

## Install

```sh
pip install hypercast
```

Real platform-specific wheels, so it lands at native speed with nothing to compile — no
compiler needed and no dependencies at all; the PyO3 extension *is* the package. The wheels
are `abi3` (abi3-py311), so one per platform covers every CPython from the 3.11 floor up,
eight in all:

| | x64 | arm64 |
| --- | :---: | :---: |
| Linux, glibc 2.28 or newer (`manylinux_2_28`) | ✓ | ✓ |
| Linux, musl 1.2 or newer (`musllinux_1_2` — Alpine) | ✓ | ✓ |
| macOS | ✓ | ✓ |
| Windows | ✓ | ✓ |

**Only wheels are published — there is no sdist.** An interpreter none of the eight matches
(a glibc older than 2.28, PyPy, free-threaded CPython) gets `No matching distribution found`
from pip, not a source build.

**Not yet covered: free-threaded (no-GIL) CPython (`3.13t`/`3.14t`).** An `abi3` wheel is
ignored by a free-threaded interpreter (it's a genuinely separate ABI, not a compatibility
flag), so closing this gap means building and shipping additional version-specific
`cp313t`/`cp314t` wheels alongside the existing eight, not just a build-flag change. PyO3
itself has supported free-threading (opt-in, `gil_used = false`) since 0.23; the cleaner
long-term fix — [PEP 803](https://peps.python.org/pep-0803/)'s `abi3t` stable ABI, one build
covering both GIL and no-GIL — needs Python 3.15+, not yet released. Revisiting once that
lands or free-threaded adoption justifies the extra wheel legs. The stance in the code
today: the extension module does not declare `gil_used = false`, so a build from a checkout
for a free-threaded interpreter imports with the GIL switched back on (CPython says so in a
`RuntimeWarning`) — correct, just not parallel. And no door releases the GIL: each is tens
to hundreds of nanoseconds of pure Rust, less than the handoff would cost.

The package is typed: it ships `py.typed` and a stub for the extension module, so mypy and
pyright see each door's own `Success[...] | Fault` rather than `Any` — which is what makes
the exhaustiveness check below a real one.

## Usage

```python
import hypercast

match hypercast.cast_i32("(1,234)", hypercast.NumFormat.INVARIANT):
    case hypercast.Success(value):
        print("got", value)                       # -1234, accounting negative
    case hypercast.Fault(reason, offset, length):
        print(reason.name, "at byte", offset)
```

Door names mirror the native ABI (`cast_i32`, `cast_f64`, `cast_decimal`, `cast_timestamp`,
…); inputs are `str` or `bytes`, both zero-copy views, and a `Fault`'s span comes back in
the caller's own units — byte offsets for `bytes`, code-point offsets for `str` — so
`text[offset:offset + length]` slices the offending text back out of whatever you passed,
no mapping needed. Exhaustiveness is the type checker's job here —
pair the two cases with `typing.assert_never` under mypy/pyright for the compile-time
guarantee the static bindings get natively. Python-flavored fidelity, stated honestly:
`int` is unbounded, so `cast_u64` returns the true unsigned value with no bit-pattern
games; `cast_decimal` returns an exact, canonical `decimal.Decimal`; `datetime`'s
resolution is microseconds, so the core's nanoseconds truncate by three digits on the
temporal doors — `datetime`'s own ceiling, not the parser's, and said out loud rather than
discovered later.

The exhaustiveness check, spelled out — `Verdict[int]` is `Success[int] | Fault`, what
`cast_i32` returns, and the fall-through arm stops type-checking the moment either case
above it goes missing (`assert_never` is in `typing` on every supported Python):

```python
from typing import assert_never

def describe(verdict: hypercast.Verdict[int]) -> str:
    match verdict:
        case hypercast.Success(value):
            return f"got {value}"                 # value: int
        case hypercast.Fault(reason, offset, length):
            return f"{reason.name} at {offset}"
        case _:
            assert_never(verdict)
```

### The doors

Every door takes the text first — `str` or `bytes` — and returns `Success(value)` or
`Fault(reason, offset, length)`:

| Door | Reads | `Success.value` |
| --- | --- | --- |
| `cast_bool(text)` | the natural-language boolean lexicon | `bool` |
| `cast_i8` `cast_i16` `cast_i32` `cast_i64` `(text, fmt)` | a signed integer under a declared `NumFormat` | `int` |
| `cast_u8` `cast_u16` `cast_u32` `cast_u64` `(text, fmt)` | an unsigned integer under a declared `NumFormat` | `int` |
| `cast_f32` `cast_f64` `(text, fmt)` | a real under a declared `NumFormat` | `float` |
| `cast_decimal(text, fmt)` | an exact decimal under a declared `NumFormat` | `decimal.Decimal` |
| `cast_uuid(text)` | every .NET `Guid` form, plus `urn:uuid:`-style prefixes | `uuid.UUID` |
| `cast_timestamp(text)` | an RFC 3339 instant | `datetime`, aware, UTC |
| `cast_unix(text, precision)` | an integer Unix-epoch value in a declared `UnixPrecision` | `datetime`, aware, UTC |
| `cast_excel_serial(text, epoch)` | an Excel date serial in a declared `ExcelEpoch` | `datetime`, aware, UTC |
| `cast_date(text)` | a strict ISO `yyyy-MM-dd` date | `date` |
| `cast_date(text, order)` | a separated date in a declared `DateOrder` | `date` |
| `cast_datetime(text, order)` | a zone-less civil date-time in a declared `DateOrder` | `datetime`, naive |
| `cast_time(text)` | an ISO 24-hour time of day | `time` |
| `cast_duration(text)` | ISO 8601, the invariant colon form, or protobuf JSON seconds | `timedelta` |

Nothing about the text is guessed; what a door cannot know from the text, the caller
declares, as an `IntEnum`:

- **`UnixPrecision`** — `SECONDS`, `MILLISECONDS`, `MICROSECONDS`, `NANOSECONDS`: the unit of
  a Unix-epoch value, never inferred from its magnitude.
- **`ExcelEpoch`** — `Y1900` (the Windows default, phantom February 29th 1900 included) or
  `Y1904` (the legacy Macintosh system): a workbook-level setting no serial number carries.
- **`DateOrder`** — `YEAR_MONTH_DAY`, `MONTH_DAY_YEAR`, `DAY_MONTH_YEAR`: `"1/7/2026"` is
  January 7th or July 1st only because the caller said which. Without one, `cast_date`
  stays strict ISO.

```python
from hypercast import DateOrder, ExcelEpoch, UnixPrecision

hypercast.cast_unix("1700000000123", UnixPrecision.MILLISECONDS)
# Success(value=datetime.datetime(2023, 11, 14, 22, 13, 20, 123000, tzinfo=datetime.timezone.utc))
hypercast.cast_excel_serial("45000", ExcelEpoch.Y1900)
# Success(value=datetime.datetime(2023, 3, 15, 0, 0, tzinfo=datetime.timezone.utc))
hypercast.cast_date("1/7/2026", DateOrder.DAY_MONTH_YEAR)
# Success(value=datetime.date(2026, 7, 1))
hypercast.cast_datetime("1/7/2026 3:04 PM", DateOrder.MONTH_DAY_YEAR)
# Success(value=datetime.datetime(2026, 1, 7, 15, 4))
hypercast.cast_duration("PT1H30M")
# Success(value=datetime.timedelta(seconds=5400))
```

A `Fault`'s `reason` is a `CastFailure` — `EMPTY` (nothing there, or only whitespace),
`MALFORMED` (present, but not the target type) or `OUT_OF_RANGE` (well-formed, outside the
target's range) — compared with `is`. `hypercast.optional(verdict)` presents an `EMPTY`
fault as `None` and passes everything else through, for the column where blank means
absent:

```python
hypercast.optional(hypercast.cast_i32("   ", hypercast.NumFormat.INVARIANT))    # None
hypercast.optional(hypercast.cast_i32("abc", hypercast.NumFormat.INVARIANT))    # Fault(reason=<CastFailure.MALFORMED: 2>, offset=0, length=1)
```

Bad data is always a `Fault`. What raises is a caller's bug, and it is the same exception on
both backends: `TypeError` for an argument of the wrong type (text that is neither `str` nor
`bytes`, a format that is not a `NumFormat`), `ValueError` for a declaration that names no
member of its enum or a `NumFormat` that cannot be (below), `OverflowError` for an integer
too wide for the 32 unsigned bits a flag set or a declaration is. The one exception that is
about data: a `str` holding a lone surrogate has no UTF-8 form, so it is a
`UnicodeEncodeError` before any door runs.

See [the repo root README](../README.md) for the receipts and the state of every other
language binding.

## Declared formats: separators, lenience flags, currency

Every numeric door — the integer family, `cast_f32`/`cast_f64`, and `cast_decimal` — takes
a `NumFormat`: the decimal and group separators, a bitwise OR of the lenience flags
(`GROUPING`, `PARENTHESES`, `EXPONENT`, `RADIX_PREFIXES`, `PERCENT`, `CURRENCY`; `ALL` is
all six — `SEPARATOR_DETECT` is a separator *policy*, not a lenience, and is opted into
separately), and an optional currency symbol. `NumFormat.INVARIANT` is `.`/`,` with every
lenience on and no symbol; `NumFormat.DETECT` is the same with `SEPARATOR_DETECT` added, so
the `.`/`,` roles are resolved per input from its structure (`1.234.567,89` and
`1,234,567.89` both read) and an undecidable one is a `MALFORMED` fault, never a guess;
`NumFormat.from_localeconv()` bridges `locale.localeconv()` — `decimal_point`,
`thousands_sep`, and `currency_symbol`. A format is immutable and reads back through
`decimal_sep`, `group_sep`, `flags` and `currency`.

A currency symbol is declared, never guessed. With `CURRENCY` set and a symbol declared, the
symbol is accepted once, leading (before or after the sign: `$5`, `-$5`, `$ -5`) or trailing
(`5 €`, `1.234,50 kr.`), with optional whitespace between it and the digits, and accounting
parentheses wrap the symbol along with the digits (`($5)`). Declared but with the flag off,
the symbol is simply the first offending byte of a `MALFORMED` fault; with no symbol declared
the flag matches nothing. A symbol is 1 to 16 UTF-8 bytes with no ASCII digit or whitespace —
anything else is a `ValueError` at construction, a caller bug like equal separators, raised
identically on both backends.

```python
dollars = hypercast.NumFormat(".", ",", hypercast.NumFormat.ALL, "$")
hypercast.cast_i32("($1,234)", dollars)          # Success(value=-1234)
hypercast.cast_decimal("$1,234.50", dollars)     # Success(value=Decimal('1234.5'))

krone = hypercast.NumFormat(",", ".", hypercast.NumFormat.ALL, currency="kr.")
hypercast.cast_decimal("1.234,50 kr.", krone)    # Success(value=Decimal('1234.5'))
```

`cast_decimal` is the real doors' grammar with an exact result: a `decimal.Decimal` built
from the core's sign, 96-bit magnitude and base-10 scale, never a float in between. The
result is canonical: exact trailing zeros in the fraction are trimmed, so `"0.1"` is one
tenth, `"50%"` is `Decimal('0.5')`, `"1.10"`, `"1.1"` and `"1.1000"` are all `Decimal('1.1')`
(`as_tuple()` gives digits `(1, 1)`, exponent `-1`), and zero is `Decimal('0')`, never negative.
Text past 96 bits of magnitude or 28 nonzero places is `OUT_OF_RANGE` — nothing but a zero is
ever dropped, never rounded, the one thing a caller who reached for a decimal is entitled to
assume.

`hypercast.native_version()` names the core actually loaded, `"major.minor.patch"`, decoded
from the same packed `hypercast_version` export every other binding probes, and
`hypercast.BACKEND` says which backend loaded it — `"native"` or `"wasm"`.

## Why not `int()` / `fromisoformat` / `dateutil`?

1. **Verdicts, not exceptions** — bad data is the expected case for untrusted text, and a
   `Fault` is a reason plus a span, not a `ValueError` to catch and regex.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators and currency symbols, radix prefixes, all five .NET
   `Guid` text forms plus `urn:uuid:` prefixes, protobuf JSON durations — with each
   lenience individually declared, never guessed.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (the whole suite green on both backends, the full corpus
   replayed).
4. **Native-extension speed** — the escape from the interpreted tier is this binding's own
   receipt: the old losses were never "Python calling native code," they were *ctypes*
   (~1 µs of interpreted marshalling per call, measured). The numbers are under
   [Benchmarks](#benchmarks).

**The honest trade-off:** for plain invariant integers `int()` still wins — it's a
C-accelerated builtin with no boundary to cross. These doors earn their keep on the
culture-machinery parsers, the closed error contract, and cross-language agreement. And
dropping ctypes means dropping the Pyodide path Python briefly had — the wheels are real
native extensions, and a native extension has no browser story. The wasm backend below is
the other direction entirely: the core as wasm inside an ordinary CPython, not CPython
inside a browser.

## Benchmarks

With the mechanism replaced, every door runs 10-18x faster than it did over ctypes (pyperf,
linux-arm64): timestamp 3.07 µs → **201 ns** — near-parity with C-accelerated
`fromisoformat` (163 ns) while returning verdicts instead of exceptions; i32 at **146 ns**
vs `int()`'s 88; and the forgiveness doors at ~180 ns for grammar the stdlib doesn't sell at
any price. The uuid door used to sit at parity with `uuid.UUID()` because both were bounded
by `UUID.__init__`; it now builds the instance the way HyperUuid pinned — `UUID.__new__`
plus `object.__setattr__` of the `int` and `is_safe` slots, skipping an `__init__` whose
validation the core has already done — and measures **730 ns against 1.18 µs** before the
change, same machine, same session, ahead of `uuid.UUID()`'s 979 ns.

The messy-feed doors are where the gap is widest, because `strptime` is the only stdlib
parser that accepts their input at all — and it is *slow*:

| Door | HyperCast | stdlib | Verdict |
| --- | ---: | ---: | --- |
| `cast_datetime("1/7/2026 3:04 PM", MONTH_DAY_YEAR)` | 409 ns | 5.19 µs `datetime.strptime` | **12.7x faster** |
| `cast_date("1/7/2026", MONTH_DAY_YEAR)` | 292 ns | 3.86 µs `datetime.strptime` | **13.2x faster** |

Separator detection costs ~18 ns: `1.234.567,89` under `NumFormat.DETECT` is 216 ns
against 198 ns for the same text under a declared eurozone format.

Reproduce: `maturin develop --release` (from `python/`, inside a virtualenv — `pyproject.toml`
already points maturin at `../rust/Cargo.toml` and the `python` feature) to build the release
extension, then `pip install pyperf` and `python bench_cast.py --fast`.

## WebAssembly (wasmtime)

The same Rust core, compiled to `wasm32-wasip1`, run *inside* CPython by
[`wasmtime-py`](https://github.com/bytecodealliance/wasmtime-py) — the inverse of the Pyodide
experiment this package once carried (CPython itself in the browser, loading the core as an
Emscripten side module). Nothing is reimplemented: `hypercast._wasm` calls the identical
`cast_*` C-ABI exports the PyO3 extension does, across a guest/host memory boundary
instead of a direct call, and presents the same `Success`/`Fault`/`NumFormat` types with the
same `__match_args__`, equality, `repr` and exception types. The whole test suite
runs against it, corpus replay included, and `tests/test_wasm_backend.py` pins its outputs
against the extension across a subprocess boundary.

```sh
pip install hypercast[wasm]        # adds wasmtime; the .wasm module ships inside every wheel
HYPERCAST_WASM=1 python app.py     # force it; hypercast.BACKEND reports "wasm" or "native"
```

Without the variable, `_native` is used whenever it imports, and `_wasm` is the fallback when it
does not and `wasmtime` is installed — an install whose extension cannot load keeps working
instead of failing at import. One honest limit on that story today: it does not widen where
`pip install hypercast` works. Only wheels are published and every one of them carries the
PyO3 extension, so an interpreter no wheel matches has nothing to install, with or without
`[wasm]`. A pure-Python wheel carrying only the wasm backend is what would make
`pip install hypercast[wasm]` land anywhere `wasmtime` itself does; it is not built yet.

One platform note, measured on a bare `python:3.14-alpine`: the musl wheel needs nothing
beyond the base image — it carries its own copy of `libgcc_s` — and the whole suite passes
there. The wasm backend on Alpine additionally needs `apk add libgcc`, because `wasmtime`'s
own musl wheel links `libgcc_s` without bundling it.

Three things about the crossing decide the numbers below:

- **Buffers come from the guest.** A wasm module only sees its own linear memory, so this backend
  asks the module's exported `malloc` for every buffer it touches — the input text (a grow-only
  buffer), the 16-byte out-value, the fault span, the 32-byte `NumFormat` — rather than picking
  an offset itself. That is load-bearing, not tidiness: the guest's own allocator (dlmalloc, which claims
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
the answer to "the extension will not load here", not a speed option; the object-building doors
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

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
