# Benchmarks

The claims in the main README are measured, not just asserted: each binding with a mature
benchmarking ecosystem has its own harness (Criterion, BenchmarkDotNet, JMH, `testing.B`,
package-benchmark, benchmark-ips, phpbench, pyperf, bench, mitata), the losses print next to the
wins, and every figure here is a dated, machine-specific receipt from its binding's own session
(regenerate with the commands in each binding's README on your own hardware). Every HyperCast
figure includes the binding's crossing and any transcode.

Two machines. Every table is linux-x64 on an Intel Core i9-11900H unless it says otherwise; these
were measured on an Apple M6 (Mac18,5) under macOS, each section dated: Go's current figures on
2026-11-05 and R's on 2026-11-13 ([below](#go-and-r-on-an-apple-m6)), the checked doors from
2026-11-01 ([below](#the-checked-doors-on-an-apple-m6)), the Rust core's plain doors before and
after the Unicode white-space change on 2026-11-03
([below](#the-plain-doors-and-unicode-white-space-on-an-apple-m6)), the grouping policies on
2026-11-06 ([below](#judging-every-grouping-policy-on-an-apple-m6)), the text door's UTF-8 check on
2026-11-08 ([below](#the-text-doors-utf-8-check-on-an-apple-m6)) and refused cells on 2026-11-10
([below](#refused-cells-on-an-apple-m6)). The two machines are not comparable with each other. Two
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

**2026-11-02 (Apple M6, .NET 11 RC 3).** The `string` doors now send a lone surrogate as WTF-8
rather than U+FFFD. Measured A/B on that machine the rows are level within a nanosecond
(`Cast.Uuid` 13.2 against 13.0 ns, `Cast.Timestamp` 30.2 against 30.2); the widest gap is
`Cast.Decimal`, 27.9 against 26.9 ns. The table above predates the change.

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

**Correction, 2026-11-02 (Apple M6, Temurin 25.0.5).** The `String` doors now send a lone
surrogate as WTF-8 instead of `getBytes(UTF_8)`'s `?`, keeping `getBytes` whenever its bytes
hold no `?`. On that machine the longest texts pay for the look: `Cast.uuid` 16.5 ns against
12.3 ns before, `Cast.timestamp` 26.2 against 21.2; the other `String` rows are level. The
figures above predate the change; the Java README prints the A/B table.

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
| Go, on an Apple M6 | 48.8 ns, 0 allocs | 47.2 ns `time.Parse` w/ layout | level — with the declared-order date door, which beats `time.Parse` (15.8 vs 28.2 ns) |

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

Every door cost 270-680 ns on 1.0's FFI package (recorded on 2026-10-01), nearly all of it the `ext-ffi` call and the PHP
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

Measured on an Apple M6 (Mac18,5, 12 cores) under macOS, Go on 2026-11-05 and R on 2026-11-13,
a different machine and OS from every table above, so they are not comparable to the linux-x64
numbers.

**Go** 1.27.3, `go test -bench=. -benchmem` from `go/`, six runs of each binding interleaved in
one session, benchstat medians, on the core with the checked doors and the white-space trim. The cgo column is the binding
the translated core replaced, linked against a static archive built from the same `rust/` and run
with the same benchmark file in the same session; other work shared the machine, so the spread is
printed in [the Go README](../go/README.md#benchmarks), which has every row. A selection:

| Door | translated core | cgo binding | | stdlib |
| --- | ---: | ---: | --- | ---: |
| `Bool` | 15.3 ns | 23.3 ns | 1.5x faster | 1.4 ns `strconv.ParseBool` |
| `I32` | 40.4 ns | 42.9 ns | 1.1x faster | 3.6 ns `strconv.Atoi` |
| `I32` (grouped), 12 goroutines, aggregate | 20.4 ns | 6.8 ns | **3.0x slower** | — |
| `I32("1,234x567")`, rejected | 107 ns, 1 alloc | 64.2 ns, 1 alloc | **1.7x slower** | 14.6 ns, 2 allocs `strconv.Atoi("1234x567")` |
| `Timestamp` | 40.5 ns | 33.8 ns | **1.2x slower** | 20.8 ns `time.Parse(RFC3339Nano)` |
| `DateTime` (`1/7/2026 3:04 PM`) | 39.2 ns | 31.4 ns | **1.2x slower** | 46.6 ns `time.Parse` w/ layout |
| `DateOnlyOrdered` (`1/7/2026`) | 21.8 ns | 24.9 ns | 1.1x faster | 28.1 ns `time.Parse` w/ layout |

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
| reals | 38.7 µs | 41.7 µs | hypercast 1.1x faster |
| 32-bit integers | 32.6 µs | 30.6 µs | **hypercast 1.1x slower** |
| booleans | 25.6 µs | 10.5 µs | **hypercast 2.4x slower** |
| ISO dates | 25 µs | 2.4 ms | hypercast 96x faster |
| ISO instants | 33.8 µs | 4 ms | hypercast 118x faster |

Every door allocates more than its base R counterpart: the verdict's seven columns (and a
timestamp's two exact ones) are R vectors of the input's length. Measured 2026-11-13 on
hypercast 1.1.0; 1.0.0's five-column verdict read 27.84, 25.58, 18.25, 19.93 and 27.14 µs on
the same machine on 2026-11-01, so every door costs 5 to 11 µs more per 1000 strings
in 1.1, and the 32-bit integers went from a win to a loss.

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

### 2026-11-02, later: the `Decimal` loss re-measured (a correction)

Measured on the same Apple M6 under macOS 27.2 later on 2026-11-02, with rustc 1.99.1, by the
same command: two runs of the current code back to back, then the 2026-11-01 code as a control,
with the machine less loaded than for the subsection above (load average 1.4 to 3.3). Nothing
the checked doors run changed between the two sessions.

| Case | plain | checked | cost of the verdict | run 2 (plain / checked) | 2026-11-01 code, same session |
| --- | ---: | ---: | ---: | ---: | ---: |
| `f64`, latitude | 6.67 ns | 87.6 ns | +81 ns | 6.60 / 87.9 ns | 6.79 / 84.7 ns |
| `checked_from_f64`, latitude | 6.67 ns | 260 ns | **+253 ns** | 6.60 / 261 ns | 6.79 / 261 ns |
| `Decimal`, euro cash (a violation) | 3.87 ns | 26.4 ns | +23 ns | 3.78 / 22.4 ns | 3.92 / 23.3 ns |
| `Text`, trimmed `enum` | 4.00 ns | 60.1 ns | +56 ns | 3.92 / 61.1 ns | 3.92 / 59.6 ns |

Every row is still 0 allocations. The subsection above overstated the `Decimal` loss: the same
binary measures either about 22 to 24 ns or about 26 ns from one run to the next (run 1 and
run 2 here, and five further interleaved pairs of the `Decimal` case alone: current code 26.4,
25.4, 24.5, 26.4 and 24.1 ns, the 2026-11-01 code 22.8, 25.0, 24.3, 23.2 and 22.9 ns). Read the
loss as about 9% by the median of those pairs (25.4 against 23.2 ns), and up to 13% on a
slow run, not 18%. It is still a loss and it was not recovered: the instructions the
case runs are the same in both builds (its callees, `cast_decimal`, `snap` and `compare`,
disassemble identically, and the inlined judgement does no extra work), and keeping
`checked_numeric` out of line did not move it, so it stays printed. Against this control, `f64`
is about 3% slower (the second judgement of the delivered float), `checked_from_f64` level and
`Text` within 1%.

## The plain doors and Unicode white space, on an Apple M6

Every door now trims Unicode `White_Space` at its edges where it trimmed ASCII (see the
CHANGELOG), which puts a scan where a byte test was. Measured on the same Apple M6 (12 cores,
32 GiB) under macOS 27.2 on 2026-11-03, with rustc 1.99.1: `cargo bench --bench cast_benchmarks`
from `rust/`, once on a detached checkout of the base commit (`446b4f7`) and once on the
change, back to back. Medians are Criterion's point estimates. The machine was not idle: load
average 2.6 before the base run, 4.3 between the runs and 3.6 after, with other applications
open, so read a difference under about 3% as noise (the comparators the change cannot touch,
`f64/stdlib` and `i64/stdlib` and the rest, moved from -2.5% to +3.0%).

| Case | base | change | diff |
| --- | ---: | ---: | ---: |
| bool/hypercast | 1.11 ns | 0.91 ns | -18.4% |
| bool/stdlib | 0.37 ns | 0.37 ns | -0.2% |
| i64/hypercast | 4.22 ns | 4.00 ns | -5.2% |
| i64/stdlib | 3.51 ns | 3.52 ns | +0.0% |
| f64/hypercast | 6.71 ns | 6.79 ns | +1.2% |
| f64/stdlib | 4.50 ns | 4.64 ns | +3.0% |
| f64-shapes/plain | 5.71 ns | 5.71 ns | +0.0% |
| f64-shapes/grouped | 10.07 ns | 10.21 ns | +1.4% |
| f64-shapes/currency | 9.72 ns | 9.87 ns | +1.5% |
| f64-shapes/grouped currency | 10.20 ns | 10.38 ns | +1.7% |
| f64-shapes/grouped currency in parens | 10.40 ns | 10.94 ns | +5.1% |
| decimal-shapes/plain | 4.92 ns | 4.83 ns | -1.9% |
| decimal-shapes/grouped | 10.39 ns | 10.23 ns | -1.6% |
| decimal-shapes/currency | 9.35 ns | 9.29 ns | -0.7% |
| decimal-shapes/grouped currency | 9.93 ns | 9.92 ns | -0.1% |
| decimal-shapes/grouped currency in parens | 10.14 ns | 10.29 ns | +1.5% |
| i64-shapes/plain | 2.74 ns | 2.55 ns | -7.1% |
| i64-shapes/grouped | 6.40 ns | 6.01 ns | -6.0% |
| i64-shapes/grouped currency | 6.83 ns | 6.51 ns | -4.7% |
| i64-shapes/grouped currency in parens | 6.80 ns | 6.66 ns | -2.1% |
| i32-declared-currency/invariant | 6.24 ns | 6.26 ns | +0.3% |
| i32-declared-currency/declared $ | 6.74 ns | 6.57 ns | -2.4% |
| i32-malformed/hypercast | 19.99 ns | 20.89 ns | +4.5% |
| i32-malformed/stdlib | 2.13 ns | 2.15 ns | +1.0% |
| decimal/hypercast | 5.86 ns | 5.93 ns | +1.1% |
| decimal/hypercast grouped currency | 10.35 ns | 10.50 ns | +1.4% |
| uuid/hypercast | 4.98 ns | 4.78 ns | -4.0% |
| uuid/uuid-crate | 7.03 ns | 6.85 ns | -2.5% |
| timestamp/hypercast | 7.16 ns | 6.98 ns | -2.4% |
| timestamp/time-crate | 7.31 ns | 7.37 ns | +0.8% |
| datetime/hypercast-messy | 8.08 ns | 8.29 ns | +2.6% |
| datetime/hypercast-iso | 7.84 ns | 7.22 ns | -7.9% |
| datetime/hypercast-date-ordered | 3.49 ns | 3.23 ns | -7.4% |
| separator-detection/hypercast-detect | 16.68 ns | 16.78 ns | +0.6% |
| separator-detection/hypercast-declared | 9.09 ns | 9.02 ns | -0.7% |
| duration/hypercast-iso | 10.95 ns | 11.84 ns | +8.1% |
| duration/hypercast-colon | 7.14 ns | 7.33 ns | +2.6% |
| checked-f64/plain | 6.96 ns | 7.04 ns | +1.2% |
| checked-f64/checked | 88.24 ns | 88.40 ns | +0.2% |
| checked-f64/checked from_f64 | 264.88 ns | 270.44 ns | +2.1% |
| checked-decimal/plain | 3.91 ns | 3.88 ns | -0.7% |
| checked-decimal/checked | 26.64 ns | 26.57 ns | -0.3% |
| checked-text/plain | 3.97 ns | 3.96 ns | -0.2% |
| checked-text/checked | 62.03 ns | 62.26 ns | +0.4% |

On text with nothing to trim, the doors are level inside the noise: nothing is clearly faster,
and the slowest case, `duration/hypercast-iso`, is **+8.1%** (+0.9 ns, more than any
comparator moved), printed here as the loss it may be. The `hypercast` rows that came out
faster (`bool` -18.4%, `datetime/hypercast-iso` -7.9%, the date door -7.4%) are not claimed:
the change reorganised the trim, and a few percent either way is layout and noise.

**Padded input is slower.** The padded cases are a group of the same harness, `padded` in
`rust/benches/cast_benchmarks.rs`: each door's text bare and padded with an ASCII space before
and a newline after (` true\n`, `  1234567890123\n`, `  12345.6789\n`, and a D-format UUID in a
space and a newline). They were run on 2026-11-03 on the same Apple M6 (macOS 27.2, rustc
1.99.1), with that harness copied into the base checkout so both sides ran the same file,
three times each, alternating base and change: twice as `cargo bench --bench cast_benchmarks
-- 'padded/|duration/' --warm-up-time 1 --measurement-time 4` and once as the whole bench at
Criterion's defaults. Load average ran 3.6 to 2.4 over the filtered pairs and 2.0 to 1.6 over
the whole-bench pair, so the machine was not idle. Each figure is the median of the three
runs' Criterion medians (the three runs of a case spread by 0.4 ns at most):

| Case | base | change | diff |
| --- | ---: | ---: | ---: |
| `bool`, `true` | 1.32 ns | 1.39 ns | +5% |
| `bool`, ` true\n` | 1.32 ns | 3.29 ns | **+150%** |
| `i64`, `1234567890123` | 4.80 ns | 4.79 ns | 0% |
| `i64`, `  1234567890123\n` | 4.83 ns | 7.33 ns | +52% |
| `f64`, `12345.6789` | 7.85 ns | 6.47 ns | -18% |
| `f64`, `  12345.6789\n` | 7.78 ns | 8.57 ns | +10% |
| `uuid`, D format | 4.44 ns | 4.74 ns | +7% |
| `uuid`, padded the same way | 5.16 ns | 7.02 ns | +36% |

Read the padding cost inside each build, not the diff column. On the change, padding adds
1.9 ns to `bool`, 2.5 to `i64`, 2.1 to `f64` and 2.3 to `uuid`; on the base it added 0.0,
0.0, -0.1 and 0.7 ns. A padded cell is about 2 ns slower than on the base. The bare `f64` row
shows why the diff column cannot be read: the same base commit measured 6.26 and 6.71 ns
for the bare `f64` in the two earlier sessions and 7.85 ns here, because the bench file
gained cases and the code moved, so the bare rows' -18% to +7% are layout, not claimed, and
the padded diffs are percentages of that base (the earlier run, with a 6.63 ns base, had
`f64` at +31%). Padded with U+00A0 on both sides (the `padded-nbsp` group, the change only,
one run at 4 s, load 2.0) the change costs the same as with ASCII: `bool` 3.73 ns, `i64`
7.09 ns, `f64` 9.14 ns; the base reads none of that text, so there is nothing to put beside
it. This loss is the price of reading what Excel and the web paste, and it was not
recovered.

**The unpadded doors, a second time.** The whole-bench pair above is a second run of every
case in the table further up, with the base and the change again back to back. The doors
with nothing to trim showed no pattern of loss again, with the spread wider than
the first time, from -22% to +13%: `f64/hypercast` and `f64-shapes/plain` read -17% and -22%
on the base's 7.9 ns (the layout shift above), and the losses were `i32-malformed/hypercast` +9.4% (it was
+4.5% in the first run), `checked-decimal/checked` +12.6% (it was -0.3%), `uuid/hypercast`
+7.3% (it was -4.0%), `bool/hypercast` +6.3% and `checked-text/plain` +5.4%, printed here as
the losses they may be. `duration/hypercast-iso`, the first run's +8.1%, did not repeat:
-2.4% in the whole-bench run and -11% in the two filtered runs (10.5 ns against 11.9), so
that loss is within what this machine moves from run to run. One case is slower in both
sessions: `i32-malformed/hypercast`, +4.5% and +9.4% (a rejected input; "Refused cells"
below has its cause and its recovery), so it stays printed here as the loss it was.

History, since a first measurement of this change was worse: the same pair of runs taken
before the trim was reworked (its scanners were out-of-line loops run at every call) showed
every `hypercast` door +16% to +34% on unpadded input and `bool` +172% (1.10 to 2.99 ns),
while the comparators stood still. The rework returns at once when an input's first and last
bytes are visible ASCII and inlines the scanners, and the runs above are after it. Every other table in this file, each binding's included,
predates the white-space change and has not been re-run.

## Refused cells, on an Apple M6

A cell the door refuses falls out of the fast lanes into the engine, which owns every fault span,
so a column of bad data is read at the engine's price. That price had risen since 1.0 (the engine
counts groups as it reads and chooses a more exact fault), and three changes took it back: the
integer engine gathers a numeral's first nineteen digits in a `u64` and widens to its checked
`i128` only for a longer one, a declared symbol is tried against its first byte before any slices
are compared (a one-byte symbol, which nearly every one is, is that comparison alone), and a cell
with no text returns before the white-space scan is called. No verdict or span changed.

`1.0` is a build of tag `v1.0.0`, `before` is `f558c7e` and `after` is the change, each with the
same benchmark file's `refused` group (1.0's own file with that group copied in, since it lacks
the later groups, so those rows have no 1.0 figure). Measured on the Apple M6 above under macOS
27.2 on 2026-11-10, with rustc 1.99.1: each tree's `cargo bench --bench cast_benchmarks` binary
was saved, and the three were run one after another for five rounds, the order rotated each
round, with `--warm-up-time 1 --measurement-time 3`, at a load average of 1.3 to 1.6. Each
figure is the median of the five rounds' Criterion medians. A figure moves in steps of about
0.2 ns here (one cycle), and the rows the change cannot reach (`i32-malformed/stdlib`,
`f64/stdlib`, `i64/stdlib`, the padded rows) moved from -2.6% to +9.0% between `before` and
`after`, `i64/stdlib` the most, so read a difference under about 3% as noise.

The refused rows are the gain: a numeral with a stray character after it is 24% faster than
before and 8% faster than 1.0, one with the character first 20% and level with 1.0, an empty
cell 68% and 40%, and the `i32-malformed` row, a loss in every earlier table of this file, is
29% faster than before and 9% faster than 1.0. The real door's refusal recovered 11%, and is
still **6% slower than 1.0**.

**What it cost, and what it didn't recover.** Two numerals that read are slower than before, in
every run that measured them: a grouped `i32` under the invariant format, **+3.2%** (one step,
6.77 to 6.99 ns), and the decimal door's `$12,345.67`, **+4.2%** (9.44 to 9.83 ns, which puts
it level with 1.0). The other grouped rows moved by 3% or less either way. The losses against 1.0 that "Judging
every grouping policy" prints are all still here, since this change wasn't aimed at them: the
integer door's plain numeral +27%, its grouped ones +23 to 26%, the grouped `i32` +10 to 14%,
`i64/hypercast` +8%, `f64/hypercast` +7%, `f64-shapes/plain` +11%, and separator detection +12%.

| Case | 1.0 | before | after | after vs before | after vs 1.0 |
| --- | ---: | ---: | ---: | ---: | ---: |
| refused/i32 123x | 14.22 ns | 17.26 ns | 13.09 ns | -24.1% | -7.9% |
| refused/i32 y123 | 10.06 ns | 12.45 ns | 9.99 ns | -19.8% | -0.7% |
| refused/i32 empty | 0.52 ns | 0.98 ns | 0.31 ns | -68.1% | -40.0% |
| refused/f64 123z | 16.66 ns | 19.94 ns | 17.66 ns | -11.4% | +6.1% |
| i32-malformed/hypercast | 19.02 ns | 24.24 ns | 17.29 ns | -28.7% | -9.1% |
| i32-malformed/stdlib | 2.10 ns | 2.10 ns | 2.10 ns | +0.0% | -0.1% |
| i64/hypercast | 4.37 ns | 4.80 ns | 4.73 ns | -1.5% | +8.2% |
| i64/stdlib | 3.46 ns | 3.33 ns | 3.63 ns | +9.0% | +4.9% |
| f64/hypercast | 6.05 ns | 6.47 ns | 6.46 ns | -0.2% | +6.7% |
| f64/stdlib | 4.40 ns | 4.39 ns | 4.38 ns | -0.1% | -0.3% |
| i64-shapes/plain | 2.52 ns | 3.17 ns | 3.21 ns | +1.4% | +27.2% |
| i64-shapes/grouped | 6.23 ns | 7.43 ns | 7.65 ns | +2.9% | +22.8% |
| i64-shapes/grouped currency | 6.25 ns | 7.87 ns | 7.87 ns | -0.0% | +25.9% |
| i64-shapes/grouped currency in parens | 6.44 ns | 8.08 ns | 8.08 ns | -0.0% | +25.6% |
| i32-declared-currency/invariant | 6.16 ns | 6.77 ns | 6.99 ns | +3.2% | +13.5% |
| i32-declared-currency/declared $ | 6.64 ns | 7.21 ns | 7.32 ns | +1.5% | +10.2% |
| f64-shapes/plain | 5.49 ns | 6.10 ns | 6.08 ns | -0.4% | +10.7% |
| f64-shapes/grouped | 9.84 ns | 10.55 ns | 10.74 ns | +1.8% | +9.2% |
| f64-shapes/currency | 9.69 ns | 9.61 ns | 9.81 ns | +2.0% | +1.2% |
| f64-shapes/grouped currency | 10.11 ns | 10.71 ns | 10.71 ns | +0.0% | +5.9% |
| f64-shapes/grouped currency in parens | 10.61 ns | 11.04 ns | 11.10 ns | +0.6% | +4.6% |
| decimal-shapes/plain | 5.02 ns | 4.59 ns | 4.59 ns | +0.0% | -8.7% |
| decimal-shapes/grouped | 10.61 ns | 9.30 ns | 9.41 ns | +1.2% | -11.3% |
| decimal-shapes/currency | 9.18 ns | 7.88 ns | 7.98 ns | +1.3% | -13.0% |
| decimal-shapes/grouped currency | 9.83 ns | 8.98 ns | 9.00 ns | +0.2% | -8.5% |
| decimal-shapes/grouped currency in parens | 9.96 ns | 9.33 ns | 9.39 ns | +0.7% | -5.7% |
| decimal/hypercast | 5.82 ns | 5.90 ns | 5.90 ns | -0.0% | +1.4% |
| decimal/hypercast grouped currency | 9.83 ns | 9.44 ns | 9.83 ns | +4.2% | -0.0% |
| grouping-policy/i64 western |  | 6.88 ns | 6.99 ns | +1.6% |  |
| grouping-policy/f64 western |  | 9.14 ns | 9.21 ns | +0.8% |  |
| grouping-policy/decimal western |  | 8.95 ns | 9.05 ns | +1.2% |  |
| grouping-policy/i64 indian |  | 7.21 ns | 7.32 ns | +1.6% |  |
| separator-detection/hypercast-detect | 16.60 ns | 18.35 ns | 18.52 ns | +0.9% | +11.6% |
| separator-detection/hypercast-declared | 8.77 ns | 9.95 ns | 9.78 ns | -1.6% | +11.5% |
| padded/bool/plain |  | 1.40 ns | 1.40 ns | +0.0% |  |
| padded/bool/ascii-padded |  | 3.27 ns | 3.23 ns | -1.2% |  |
| padded/i64/plain |  | 4.70 ns | 4.80 ns | +2.2% |  |
| padded/i64/ascii-padded |  | 7.01 ns | 7.01 ns | -0.0% |  |
| padded/f64/plain |  | 6.46 ns | 6.46 ns | -0.1% |  |
| padded/f64/ascii-padded |  | 8.43 ns | 8.46 ns | +0.4% |  |
| padded/uuid/plain |  | 4.85 ns | 4.86 ns | +0.2% |  |
| padded/uuid/ascii-padded |  | 7.21 ns | 7.17 ns | -0.6% |  |
| padded-nbsp/bool |  | 3.81 ns | 3.71 ns | -2.6% |  |
| padded-nbsp/i64 |  | 7.53 ns | 7.49 ns | -0.5% |  |
| padded-nbsp/f64 |  | 9.24 ns | 9.20 ns | -0.5% |  |

The same rows in the earlier tables of this file predate this change and weren't re-run.

## The text door's UTF-8 check, on an Apple M6

The text door checks UTF-8 with simdutf8's NEON validator from 64 bytes up, and with
`core::str::from_utf8` below that and on every failure (`rust/README.md`, "UTF-8
validation"). Apple M6 (aarch64), macOS 27.2, rustc 1.99.1, 2026-11-08:
`cargo bench --bench cast_benchmarks -- '^text/'`, Criterion point estimates. "Before" is the
same tree with the fast path compiled off, so the door is core's check alone; the two builds
ran alternately, two rounds each, and each cell is the mean of the two. The `stdlib` rows
(core's `str::from_utf8` on the same bytes, unchanged by this) moved 0 to 10% between the
builds, which is this table's noise.

| Input | Before | After | Change |
| --- | ---: | ---: | ---: |
| 63 B ASCII | 5.60 ns | 5.64 ns | +1% (core's check either way) |
| 63 B multi-byte | 20.2 ns | 17.7 ns | noise (core's check either way) |
| 64 B ASCII | 2.38 ns | 1.13 ns | 2.1x faster |
| 64 B multi-byte | 19.5 ns | 4.08 ns | 4.8x faster |
| 4 KiB ASCII | 60.4 ns | 19.8 ns | 3.0x faster |
| 4,100 B multi-byte | 1,188 ns | 247 ns | 4.8x faster |
| 4 KiB ASCII, bad at byte 0 | 1.16 ns | 31.8 ns | **27x slower, a loss** |
| 4 KiB ASCII, bad at byte 1 | 1.97 ns | 32.6 ns | **17x slower, a loss** |
| 4 KiB ASCII, bad at byte 2,048 | 30.3 ns | 56.5 ns | **+86%, a loss** |
| 4 KiB ASCII, bad last byte | 66.0 ns | 87.5 ns | **+33%, a loss** |
| 4,100 B multi-byte, bad at byte 0 | 1.23 ns | 244 ns | **199x slower, a loss** |
| 4,100 B multi-byte, bad at byte 1 | 1.97 ns | 244 ns | **124x slower, a loss** |
| 4,100 B multi-byte, bad at byte 2,050 | 529 ns | 776 ns | **+47%, a loss** |
| 4,100 B multi-byte, bad last byte | 1,048 ns | 1,274 ns | **+22%, a loss** |

The losses are the price of taking the span from core: simdutf8's `basic` check stops at no
fault, so input that fails is read once whole by the SIMD check and then by core up to the
fault. Before, a fault at the first byte cost core nothing; now it costs the SIMD pass over
the whole input (32 ns for 4 KiB of ASCII, 244 ns for 4,100 bytes of multi-byte text). The
worst ratio is that early fault, 199x; in absolute time, the worst is a fault at the end,
which against core's check of the same length of *valid* text is +45% for ASCII (87.5 ns
against 60.5 ns) and +10% for multi-byte (1,274 ns against 1,160 ns). So a failing input can
cost more than core spends on a valid one of the same length; it is bounded by one SIMD pass
over the input plus core's check of it. Under 64 bytes the door adds only a length test,
inside this table's noise. x86_64 (AVX2 or SSE 4.2, detected at run time) has not been
measured: Rosetta's timings mean nothing, and the receipt waits for a real x86_64 machine.

## Judging every grouping policy, on an Apple M6

A numeral with group separators is judged group by group, under Western by default and under Indian
when declared. The lane counts the groups as it reads, in a second loop an ungrouped numeral never
enters. `1.0` here is a build of tag `v1.0.0`, whose default read straight through, and `now` is the
tree with the two policies. Measured on the Apple M6 above under macOS 27.2 on 2026-11-06, with rustc
1.99.1: `cargo bench --bench cast_benchmarks -- 'shapes|separator-detection|decimal/hypercast|grouping-policy'`
from `rust/` on each tree, each run twice, alternating. Medians are Criterion's point estimates from each
tree's second run, at a load average of 1.2 to 3.7. Read a difference under about 3% as noise.

The losses are real: the integer door's grouped numeral costs 18 to 22% more than 1.0, separator
detection 14% and a declared eurozone format 17%. The move isn't all grouping: this compares releases,
so it also holds everything else since 1.0, such as the white-space change above, and `i64-shapes/plain`,
which never enters the group loop, lost 25% too (2.54 to 3.17 ns). The real door's grouped numeral
moved 3 to 4%, and the decimal door's gained 6 to 13%, as its plain and currency rows did.

The plain `i64` loss arrived at merge `cb32632` (pull request 43, which holds `ee6997c`: Unicode white
space, strict grouping and fault detail), and it hasn't been won back. A walk of the first-parent merges
reads 2.62 to 2.91 ns at `446b4f7` and 3.08 to 3.16 ns at `cb32632`. That walk is three runs of a separate
loop with no LTO, under a load of 2.5, so compare it only within itself. The merge wasn't split further,
and `f64` got faster at the same step, so some of the move may be code layout. The two policies didn't
add to it: the same loop reads 3.12 to 3.20 ns before them and 3.14 to 3.17 ns after.

The `*-shapes` and `separator-detection` rows use `NumFormat::INVARIANT`, or `NumFormat::new` with no
policy, which is Western. `grouping-policy` has no 1.0 figure, because 1.0 had no policy. Those rows in
"The plain doors and Unicode white space" above, the grouped table in `rust/README.md`, and the
grouped figures in the binding tables and "Separator detection" predate this change and weren't re-run.

| Case | 1.0 | now | diff |
| --- | ---: | ---: | ---: |
| f64-shapes/plain | 5.53 ns | 6.18 ns | +11.6% |
| f64-shapes/grouped | 10.13 ns | 10.39 ns | +2.6% |
| f64-shapes/currency | 9.74 ns | 9.70 ns | -0.3% |
| f64-shapes/grouped currency | 10.27 ns | 10.72 ns | +4.4% |
| f64-shapes/grouped currency in parens | 10.77 ns | 11.13 ns | +3.3% |
| decimal-shapes/plain | 4.75 ns | 4.59 ns | -3.4% |
| decimal-shapes/grouped | 11.25 ns | 9.84 ns | -12.6% |
| decimal-shapes/currency | 9.29 ns | 7.93 ns | -14.6% |
| decimal-shapes/grouped currency | 9.61 ns | 8.94 ns | -7.0% |
| decimal-shapes/grouped currency in parens | 9.84 ns | 9.27 ns | -5.8% |
| i64-shapes/plain | 2.54 ns | 3.17 ns | +24.7% |
| i64-shapes/grouped | 6.27 ns | 7.43 ns | +18.4% |
| i64-shapes/grouped currency | 6.55 ns | 7.86 ns | +20.0% |
| i64-shapes/grouped currency in parens | 6.62 ns | 8.08 ns | +22.0% |
| grouping-policy/i64 western | | 6.80 ns | |
| grouping-policy/f64 western | | 9.28 ns | |
| grouping-policy/decimal western | | 9.28 ns | |
| grouping-policy/i64 indian | | 7.09 ns | |
| decimal/hypercast | 5.68 ns | 5.90 ns | +3.8% |
| decimal/hypercast grouped currency | 9.87 ns | 9.41 ns | -4.6% |
| separator-detection/hypercast-detect | 16.57 ns | 18.84 ns | +13.7% |
| separator-detection/hypercast-declared | 8.69 ns | 10.14 ns | +16.6% |

## TypeScript

No figures are published yet. `bun run bench` (and `bun run bench:bun`, and either under
`NAPI_RS_FORCE_WASI=error` for the WebAssembly fallback) runs `typescript/bench/bench.ts` with
mitata against `parseInt`, `parseFloat` and `Date.parse`; a figure goes here with its date and
machine once it is taken.
