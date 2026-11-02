# Benchmarks

The claims in the main README are measured, not just asserted: each binding with a mature
benchmarking ecosystem has its own harness (Criterion, BenchmarkDotNet, JMH, `testing.B`,
package-benchmark, benchmark-ips, phpbench, pyperf, bench, mitata), the losses print next to the
wins, and every figure here is a dated, machine-specific receipt from its binding's own session
(regenerate with the commands in each binding's README on your own hardware). Every HyperCast
figure includes the binding's crossing and any transcode.

Two machines. Every table is linux-x64 on an Intel Core i9-11900H unless it says otherwise; Go's
current figures and R's were measured on an Apple M6 (Mac18,5) under macOS on 2026-11-01
([below](#go-and-r-on-an-apple-m6)), as were the checked doors
([below](#the-checked-doors-on-an-apple-m6)), and the two groups are not comparable with each other. Two
labels mark figures whose implementation has since been replaced, kept until they are run again:

- **C# †**: measured on .NET 11 RC 1 with BenchmarkDotNet's in-process toolchain, before the
  harness moved to BenchmarkDotNet's 0.16 previews and runs each benchmark in its own process.
- **PHP ‡**: predates the extension. These measured 1.0's FFI package; the benchmarks in
  `php/bench/` now run against the ext-php-rs extension, which has not been measured on this
  machine yet.

Go's linux-x64 figures measured the cgo binding the translated core replaced and are labelled
where they appear. Measurement rules baked into the numbers: PHP benchmarks run with
`XDEBUG_MODE=off` (a loaded Xdebug inflates everything ~14x uniformly), every case is warmed
before it is timed, and the platform parsers a door is timed against are priced by the machine
too, so every ratio here belongs partly to the box it was measured on.

The summary, the timestamp door against each platform's parser, is the
[scoreboard](../README.md#the-scoreboard) in the main README. The Rust core's own figures,
in-process against Rust's best-in-class parsers, are in [`rust/README.md`](../rust/README.md#benchmarks).

## Where the wins come from

There is no single trick; there are three crossing strategies, chosen per language by where the
boundary cost actually lives.

**Direct FFI, where the crossing floor is already nanoseconds.** C#'s `P/Invoke` and Java's FFM
cost a few nanoseconds per call. At those floors the engine's own speed dominates, so those
bindings call the C ABI directly, and any remaining slowness is *wrapper*, which gets dieted, not
excused. Swift links the core in statically and calls it the same way.

**A native extension, where the FFI mechanism itself is the cost.** What calling native code
costs from Python is the mechanism — *ctypes*' interpreted marshalling is ~1 µs a call — and
Ruby's `Fiddle` charges over a microsecond. No diet fixes that, so those bindings link the Rust
core *directly into the language VM* as an ordinary native extension (PyO3, Magnus): no dlopen,
no C-ABI hop, the same `METH_FASTCALL` door the builtins walk in Python. Ruby keeps Fiddle as
its zero-compile fallback (`HYPERCAST_PURE=1` forces it; both backends replay the corpus green,
and a cross-backend agreement spec pins them together), with precompiled platform gems as the
vehicle for shipping Magnus without ever making a consumer compile. PHP took the
native-extension door too, for reach rather than for the crossing: by default PHP allows its
built-in FFI under a web SAPI only from preloaded code, so the binding is an ext-php-rs
extension that PIE installs prebuilt, one per PHP minor. TypeScript took it from the start
(a napi-rs addon, the core linked into the `.node` file), and R calls the C ABI from a
hand-written shim reached through `.Call`, linked against the core's static library.

**No crossing at all.** Go's core is the crate's `wasm32-wasip1` build translated to Go source
by [wasm2go](https://github.com/ncruces/wasm2go) and compiled with the binding, so a call into it
is a Go call; what is left to price is copying the text into the core's memory and the
bounds-checked translated code ([its table](#go-and-r-on-an-apple-m6)).

## C# vs. the BCL †

BenchmarkDotNet, `[MemoryDiagnoser]`, lenience matched where the BCL has the knob
(`AllowThousands`, invariant culture, UTC styles), FFI crossing and UTF-16→UTF-8 transcode
*included* in every HyperCast number; zero managed allocation on every row, both sides:

| Door | HyperCast | BCL | Verdict |
| --- | ---: | ---: | --- |
| `Cast.DateTime` (`1/7/2026 3:04 PM`) vs `DateTime.TryParse` (en-US) | 37.9 ns | 171.5 ns | **4.5x faster** |
| `Cast.Timestamp` vs `DateTimeOffset.TryParse` | 57.0 ns | 125.6 ns | **2.2x faster** |
| `Cast.Duration` vs `TimeSpan.TryParse` | 31.6 ns | 116.5 ns | **3.7x faster** |
| `Cast.Date` (declared order) vs `DateOnly.TryParse` (en-US) | 27.3 ns | 91.9 ns | **3.4x faster** |
| `Cast.Int32` (grouped) vs `int.TryParse` + `AllowThousands` | 38.3 ns | 45.0 ns | 1.2x faster |
| `Cast.Double` vs `double.TryParse` | 43.3 ns | 46.4 ns | 1.1x faster |
| `Cast.Uuid` vs `Guid.TryParse` | 37.8 ns | 25.6 ns | 1.5x slower — the expected loss: this door also takes N/B/P/X forms and `urn:uuid:` prefixes |
| `Cast.Boolean` vs `bool.TryParse` | 19.7 ns | unmeasurable* | honest loss — the BCL's five-byte compare wins; the twenty-lexeme vocabulary is why anyone calls this door |

\* BDN flags the BCL boolean lane `ZeroMeasurement` — the JIT hoists/folds `bool.TryParse` of a
loop-invariant string into nothing, which an FFI call structurally can't match. The loss is real
either way and is printed as one.

Read the table the way it's meant: the wins land exactly where the BCL runs culture machinery,
the plain-number rows edge ahead while *also* carrying notations the BCL has no knob for at any
price, and every number crosses a native boundary the BCL doesn't. The losses are where the BCL
has a parser built for the one shape being timed — `Guid` here, a currency-styled `decimal` in
[the C# README](../csharp/README.md#why-not-the-bcls-own-tryparse-family) — and a few nanoseconds
there is what the door's flexibility costs: a format declared per call, a fault with a byte span,
the same verdict in every binding. Hand a door UTF-8 instead of a `string` and those two margins
nearly double (24.2 and 25.2 ns). The tabular layer,
[HyperTabular](https://github.com/SkunkWerkx/HyperTabular), crosses once per chunk and makes the
same doors a landslide.

## Java vs. the JDK, with the input never copied

JMH with `-prof gc`, the profile `./gradlew :benchmarks:jmh` runs (Temurin 25). Every figure is
the FFM path on JDK 25; the JNI path on JDK 21–24 has no published figures (`-PjmhJdk=21`
measures it). `Cast.timestamp` **50.4 ± 6.3 ns vs 553.8 ± 53.3 ns `Instant.parse`** (11.0x),
`Cast.time` **29.6 vs 371.6 ns** (12.6x), `Cast.duration` **33.8 vs 334.7 ns** (9.9x), a grouped
`Cast.i32` **36.1 vs 74.9 ns `NumberFormat`** (2.1x). **What the numbers rest on:** no door opens
an `Arena` per call, and every downcall is linked `Linker.Option.critical(true)`, so the caller's
`byte[]` crosses as a pinned heap segment instead of being copied into a native staging buffer;
every door has a `MemorySegment` overload for casting a slice of one buffer with nothing copied,
and the UUID door reads two big-endian longs instead of sixteen bytes. Three rows lose and are
printed as such, each to a JDK method with one shape to read and no boundary to cross:
`Boolean.parseBoolean` is unbeatable by construction (the JIT folds a loop-invariant call to
nothing), `UUID.fromString` wins by six nanoseconds (23.0 vs 29.2 ns) and `new BigDecimal(String)`
by twenty-six (20.3 vs 46.7 ns). The full table is in
[the Java README](../java/README.md#why-not-integerparseint--instantparse--the-formatter-zoo).

## The messy-feed doors, cross-binding

The declared-order date/date-time doors exist for text with no stdlib parser at all, so each row
pairs against whatever that platform *does* offer for the same string (`1/7/2026 3:04 PM`): a
pattern formatter, a culture-aware `TryParse`, or `strptime`. Each row from its binding's own
benchmark session:

| Binding | HyperCast | Platform's closest parser | Verdict |
| --- | ---: | ---: | --- |
| Swift | **82 ns** | 33 µs `DateFormatter` (same pattern, hoisted, 103 mallocs; see [swift/README](../swift/README.md#why-not-int32--iso8601formatstyle)) | **faster by two orders on this toolchain** |
| Python | **230 ns** | 4.65 µs `datetime.strptime` | **20x faster** |
| Java | **40.5 ns** | 348.9 ns `DateTimeFormatter` (`M/d/yyyy h:mm a`) | **8.6x faster** |
| C# † | **37.9 ns** | 171.5 ns `DateTime.TryParse` (en-US) | **4.5x faster** |
| PHP ‡ | 673 ns | 439 ns `DateTimeImmutable::createFromFormat` | 1.5x slower |
| Ruby | 877 ns | 851 ns `DateTime.strptime` | level — see below |
| Go, on an Apple M6 | 49.0 ns, 0 allocs | 46.9 ns `time.Parse` w/ layout | level — with the declared-order date door, which beats `time.Parse` (15.5 vs 28.0 ns) |

Go's linux-x64 row, which **predates the translated core** (the cgo binding, 0.9.0): 84 ns,
0 allocs, against 118 ns `time.Parse` w/ layout, and the declared-order date door 67 against
72 ns.

**Ruby's draw is about the carrier, not the parse.** Its timestamp door is 6.7x *faster* than
`Time.iso8601` on the same backend; the civil door only draws because building a stdlib
`DateTime` with an exact `Rational` second costs more than the entire native call, where `Time`
is one cheap `rb_time_nano_new`. (Same reason its `DateTime` shows a `+00:00` offset: a property
of the type, not a zone the parse assigned.) Printed because it's real — house rules.

## Python's native extension

What calling native code costs from Python is the mechanism, so the PyO3 extension
(`hypercast._native`) is the Rust core linked directly into a CPython extension, and the wheels
maturin builds *are* the package: one abi3 wheel per platform covering every CPython 3.11+,
beside PEP 803's `abi3t` wheel for free-threaded CPython 3.15+, where the extension runs with the
GIL off, no compiler needed to install. Timestamp at **252 ns**, i32 at **103 ns** vs `int()`'s
56, the forgiveness doors at ~115-160 ns for grammar the stdlib doesn't sell, and uuid at
**262 ns**, three times faster than `uuid.UUID()`'s 798 because the instance is allocated and its
slots set through the C API with no `__init__` to run. What the extension does not catch is
`fromisoformat`, a C builtin that reads one shape with no boundary to cross: that row is a 1.7x
loss. The full table is in [the Python README](../python/README.md#benchmarks).

## PHP ‡

Every door cost 270-680 ns on 1.0's FFI package, nearly all of it the `ext-ffi` call and the PHP
around it rather than the parse: `Cast::timestamp` 538 ns against `new DateTimeImmutable`'s
560 ns, level. The table, and why it is kept until the extension is measured, are in
[the PHP README](../php/README.md#benchmarks).

## Separator detection

`NumFormat.DETECT` resolves `.`/`,` roles structurally per input at ~10 ns in the raw Rust core,
and every binding's carrier is thin enough to show it, `1.234.567,89` under `DETECT` against the
same text under a declared eurozone format: Java 49.8 vs 42.6 ns, Swift 54 vs 31, Ruby (Magnus)
154 vs 145, Go 54.6 vs 41.2 on the Apple M6; C# ~7 ns †, Python ~15 ns, PHP ~54 ns ‡. Go's
cgo binding, before the translated core, measured 141 vs 125 ns on linux-x64.

## Go and R, on an Apple M6

Measured on an Apple M6 (Mac18,5, 12 cores) under macOS on 2026-11-01, a different machine and
OS from every table above, so they are not comparable to the linux-x64 numbers.

**Go** 1.27.3, `go test -bench=. -benchmem` from `go/`, six runs of each binding interleaved in
one session, benchstat medians. The cgo column is the binding the translated core replaced, built
from the same `rust/` and run with the same benchmark file in the same session; other work shared
the machine, so the spread is printed with the noisy rows in [the Go README](../go/README.md#benchmarks),
which has every row. A selection:

| Door | translated core | cgo binding | | stdlib |
| --- | ---: | ---: | --- | ---: |
| `Bool` | 10.7 ns | 24.1 ns | 2.3x faster | 1.4 ns `strconv.ParseBool` |
| `I32` | 33.6 ns | 43.5 ns | 1.3x faster | 3.6 ns `strconv.Atoi` |
| `I32` (grouped), 12 goroutines, aggregate | 19.7 ns | 6.9 ns | **2.9x slower** | — |
| `I32("1,234x567")`, rejected | 96.0 ns, 1 alloc | 64.3 ns, 1 alloc | **1.5x slower** | 14.6 ns, 2 allocs `strconv.Atoi("1234x567")` |
| `Timestamp` | 34.5 ns | 34.5 ns | ~ | 21.1 ns `time.Parse(RFC3339Nano)` |
| `DateTime` (`1/7/2026 3:04 PM`) | 49.0 ns | 34.2 ns | **1.4x slower** | 46.9 ns `time.Parse` w/ layout |
| `DateOnlyOrdered` (`1/7/2026`) | 15.5 ns | 26.0 ns | 1.7x faster | 28.0 ns `time.Parse` w/ layout |

Every success row is 0 B and 0 allocations in both bindings. Most Go doors still lose per call
to Go's stdlib, whose parsers are excellent; the binding earns its keep on the vocabulary, the
closed error contract and cross-language agreement, and HyperTabular is where a per-call cost
amortizes. The cgo binding's own linux-x64 table (0.9.0, `Timestamp` 90 ns against 58 ns
`time.Parse(RFC3339Nano)`) **predates the translated core** and is kept, labelled, in the Go
README.

**R** 4.6.2 (Homebrew build), against base R's coercions with bench 1.1.4 (`R_LIBS=<lib> Rscript
bench/bench.R` from `r/`, against an installed copy), 1000 strings per call, medians. Base R
returns `NA` where a door returns a fault and its span, so the two do different work; base R is
the call an R user reaches for today.

| Comparison (median, 1000 strings) | hypercast | base R | Result |
| --- | --- | --- | --- |
| reals | 27.84 µs | 40.26 µs | hypercast 1.4x faster |
| 32-bit integers | 25.58 µs | 30.95 µs | hypercast 1.2x faster |
| booleans | 18.25 µs | 10.58 µs | **hypercast 1.7x slower** |
| ISO dates | 19.93 µs | 2.29 ms | hypercast 115x faster |
| ISO instants | 27.14 µs | 3.83 ms | hypercast 141x faster |

Every door allocates more than its base R counterpart: the verdict's five columns (and a
timestamp's two exact ones) are R vectors of the input's length.

## The checked doors, on an Apple M6

Measured on an Apple M6 (12 cores, 32 GiB) under macOS 27.2 on 2026-11-01, with rustc 1.99.1: the
same machine as the Go and R figures above, so not comparable to the linux-x64 tables.
`cargo bench --bench cast_benchmarks -- checked` from `rust/`, Criterion point estimates, two runs
back to back with the machine lightly loaded. Each checked door is timed beside the plain door it
wraps, on the same text, with its `Constraints` built once outside the timed loop.

A checked door runs the plain door and then judges the value, so it is slower by construction.
These figures are the price of the verdict, not a win.

| Case | plain | checked | cost of the verdict | run 2 (plain / checked) |
| --- | ---: | ---: | ---: | ---: |
| `f64`, latitude: `45.1234567` in (−90, 90], midpoint to 0.000001 | 6.69 ns | 85.1 ns | +78 ns | 6.64 / 85.0 ns |
| `checked_from_f64`, the same rule and value, against `cast_f64` on the text | 6.69 ns | 259 ns | **+253 ns** | 6.64 / 262 ns |
| `Decimal`, euro cash: `12.37`, strict to 0.05 (a violation) | 3.83 ns | 22.9 ns | +19 ns | 4.04 / 24.4 ns |
| `Text`, `"  published "`, trimmed, against a three-entry `enum` | 3.90 ns | 59.8 ns | +56 ns | 3.94 / 61.1 ns |

Every row is 0 allocations. The two runs agree within 2% except `Decimal`, which moved about 6%
with wide intervals in run 2; read its cost as 19 to 20 ns. The loss is `checked_from_f64`:
rounding is judged on the float's shortest decimal digits, so it formats the float and reads it
back, about three times the checked text door. HyperTabular's spreadsheet path takes that route.

### 2026-11-02: after the delivered-float judgement, and slower

Measured on the same Apple M6 (12 cores, 32 GiB) under macOS 27.2 on 2026-11-02, with rustc
1.99.1, by the same command, two runs back to back. The checked float doors now also judge the
float they deliver against each bound as the door reads it, and the declaration gained
`HAS_ENUM`. The machine was not lightly loaded this time (load average 4.6 to 7.3), so a
control run of the 2026-11-01 code was taken in the same session; an earlier pair of runs was
discarded because its second run doubled even the plain doors.

| Case | plain | checked | cost of the verdict | run 2 (plain / checked) | 2026-11-01 code, same session |
| --- | ---: | ---: | ---: | ---: | ---: |
| `f64`, latitude | 7.24 ns | 88.7 ns | +81 ns | 8.51 / 93.0 ns | 7.20 / 85.5 ns |
| `checked_from_f64`, latitude | 7.24 ns | 273 ns | **+266 ns** | 8.51 / 269 ns | 7.20 / 264 ns |
| `Decimal`, euro cash (a violation) | 4.53 ns | 27.3 ns | +23 ns | 4.07 / 31.0 ns | 4.05 / 23.0 ns |
| `Text`, trimmed `enum` | 4.02 ns | 65.8 ns | +62 ns | 4.41 / 66.7 ns | 3.95 / 61.9 ns |

Every row is still 0 allocations. All four are slower than the 2026-11-01 receipt above, and
the losses stand. Against the same-session control, `f64` is about 4% slower and
`checked_from_f64` about 3%: the second judgement of the delivered float, whose bounds convert
straight from their 64-bit significands. `Decimal` is about 18% slower although its case
returns at `multipleOf` before any of the new code runs: `judge` is inlined either way, and
forcing it in or out of line did not move it, so it is code layout, not work, and it was not
recovered. `Text`, whose path did not change, moved about 6%, which is within this session's
noise.

## TypeScript

No figures are published yet. `npm run bench` (and `npm run bench:bun`, and either under
`NAPI_RS_FORCE_WASI=error` for the WebAssembly fallback) runs `typescript/bench/bench.ts` with
mitata against `parseInt`, `parseFloat` and `Date.parse`; a figure goes here with its date and
machine once it is taken.
