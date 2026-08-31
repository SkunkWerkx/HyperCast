# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)

**`match`/`case` over a two-case verdict — the value, or a closed reason plus the exact
byte span that offended — with the Rust core linked straight into CPython as a native
extension. No dlopen, no ctypes marshalling, no runtime bridge.**

Allocation-lean scalar casts — booleans, the full integer family, reals, UUIDs, temporals.
The PyO3 extension (`hypercast._native`) is the *only* backend — a door is an ordinary
`METH_FASTCALL` extension call into a direct Rust call, and the wheel maturin builds is
the whole package (the interim ctypes fallback is gone). Python 3.10 is the floor
(`match`/`case` is the consumption
idiom); wheels are abi3-py310, one per platform covering every CPython from there up, no
compiler needed to install.

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
   held by the shared corpus (26 tests green, full twelve-file corpus replay).
4. **Native-extension speed** — the escape from the interpreted tier is this binding's own
   receipt: the old losses were never "Python calling native code," they were *ctypes*
   (~1 µs of interpreted marshalling per call, measured). With the mechanism replaced,
   every door runs 10-18x faster than it did over ctypes (pyperf, linux-arm64): timestamp
   3.07 µs → **201 ns** — near-parity with C-accelerated `fromisoformat` (163 ns) while
   returning verdicts instead of exceptions; i32 at **146 ns** vs `int()`'s 88; uuid at
   parity with `uuid.UUID()` (both sides bounded by `UUID.__init__` itself); and the
   forgiveness doors at ~180 ns for grammar the stdlib doesn't sell at any price.

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
dropping ctypes means dropping the Pyodide/wasm path Python briefly had — the wheels are
real native extensions, and a native extension has no browser story.

## Install

Not on PyPI yet — the release pipeline is staged (`.github/workflows/release.yml`,
Trusted Publishing pending) and the wheels ship with the first coordinated tag. Until
then: clone the repo, `cargo build --release --features python` in `rust/`, copy
`libhypercast.so` to `python/src/hypercast/_native.abi3.so` (CI's own staging step, by
hand), and `pytest` — `tests/conftest.py` puts the in-repo package on the path with no
install step.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.
