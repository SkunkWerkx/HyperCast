# Roadmap

Where HyperCast is headed, and why the layers stack in this order. Each round builds
strictly on the one below it; nothing in a later round requires undoing an earlier one.

## Non-negotiables

Requirements that hold across every round, stated up front so no layer designs them away:

- **Full AOT support in .NET and Java.** C# publishes cleanly under `PublishAot`; the Java
  binding survives a real GraalVM Native Image build into a standalone native binary. Both
  patterns are already proven end-to-end in HyperUuid (its C# packaging and
  `java/aot-smoke-test`); HyperCast inherits them as a requirement, not an aspiration —
  every layer, the tabular one included.
- **The scalar core and its bindings must ride alongside HyperUuid's wasm train.** The
  same proven shapes: the Rust core under `wasm32-wasip1`, C# via Blazor's
  `NativeFileReference` static linking, and the core running as a `wasm32-wasip1` module
  *inside* the Java, Ruby, Python and Go processes through an engine each ecosystem already
  has (GraalWasm, wasmtime). HyperCast's core is strictly easier freight than HyperUuid's
  here — pure computation over caller bytes, zero dependencies, no WASI clock or randomness
  imports at all — and both legs are proven, not projected: the full test suite (51 unit +
  the counting-allocator proof + all 12 corpus replays + both fault-span invariant sweeps —
  66 tests) passes under `wasmtime` on `wasm32-wasip1`
  (`CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime --dir <repo>" cargo test --target
  wasm32-wasip1`; the preopen is only so the conformance test can read `corpus/`), and the
  four in-process backends run their bindings' whole suites, corpus replay included, a
  second time on every CI leg (see the root README's WebAssembly section).
- **The tabular layer is server domain.** CSV/TSV/XLSX ingestion must be AOT-clean like
  everything else, but wasm is explicitly out of scope at that layer — no design
  contortions to keep zip/XML streaming sandbox-friendly.

## Round one — the scalar core (done)

One Rust `cdylib` (`rust/`, `libhypercast`), HyperUuid's proven FFI mechanics: 20 `cast_*`
exports over UTF-8 bytes and caller-owned out-buffers, verdict codes (`0` ok, `1` empty,
`2` malformed, `3` out of range) with the offending byte span through a nullable fault
out-param. Semantics ported from Svartalfheim's `Norse.Primitives` parser family; temporals
land in protobuf's dual-integer forms (`{seconds, nanos}` timestamp/duration, `{y, m, d}`
date, nanos-since-midnight time) so every binding presents them at its platform's own
fidelity. Allocation-free on success *and* failure paths, asserted by a counting global
allocator, and `corpus/*.json` is the byte-for-byte conformance contract every binding
replays.

## Round two — language bindings (done)

All seven shipped and published — crates.io, nuget.org, PyPI, RubyGems, Maven Central and
Packagist, with Go and Swift resolving from the tag. The HyperUuid
playbook, re-run: C# (`P/Invoke`), Java (FFM), Go, Swift, Ruby, PHP, Python,
each folding the verdict code + fault span into its platform's discriminated-union idiom
(a `Result`-shaped type, never an exception) and each replaying the shared corpus in its
own test suite before it ships. C# first — shortest path to a headline benchmark against
`DateTime.TryParse`/`TimeSpan.Parse` culture machinery, and the deepest muscle memory from
SequentialGuid/HyperUuid.

Per-call FFI honesty, learned from HyperUuid's numbers: a single scalar cast pays a real
crossing tax (~10 ns for P/Invoke, 100 ns+ for ctypes/Fiddle-class mechanisms). Against
heavyweight culture-machinery parsers that tax vanishes into a large win; against a
platform's leanest single-purpose call it can eat the margin. That asymmetry is not a flaw
to fix in round two — it is the setup for round three.

## Round three — the payoff: tabular ingestion

**The endgame is CSV/TSV/delimited parsing and XLSX parsing built on top of the scalar
core, so the FFI boundary is crossed once per chunk instead of once per cell.**

The amortization math is the whole thesis. A million-row CSV with 20 columns is 20 million
scalar casts: driven cell-by-cell from the host, that is ~0.2 s of pure crossing overhead
on even the cheapest FFI mechanism, and catastrophically more from Python or Ruby. Driven
as "here's a text buffer and a column-type schema, fill these column buffers and parallel
verdict arrays," the tax rounds to zero while the 15–35 ns doors run in a tight native
loop. HyperUuid already measured this exact effect: its purego batch API won 19.6x by
collapsing ~5,000 crossings into one. Svartalfheim already sketched the consumer shape,
too — `Primitives.Ingestion`'s `TabularReader`/`SepTabularReader`/`ExcelTabularReader` are
the origin blueprint for what this layer's binding surface looks like.

This round lives in three repositories of its own, not in this one, and each has a Rust
crate with a green test suite against this repo's master today:

- **[HyperTabular](https://github.com/SkunkWerkx/HyperTabular)** — the contract every
  provider speaks: the format-neutral `Cell`, the caller-declared `Plan` of doors, the cast
  engine, the column-major `Batch`, and the `#[repr(C)]` shapes the bindings share. An
  rlib; it consumes `hypercast` as a git dependency on this repo's master. Its
  `docs/design.md` and `docs/prior-art.md` are the design record for all three.
- **[HyperDelimited](https://github.com/SkunkWerkx/HyperDelimited)** — CSV/TSV/any
  single-byte ASCII separator, with the SIMD structural scanner. A cdylib.
- **[HyperWorkbook](https://github.com/SkunkWerkx/HyperWorkbook)** — XLSX and ODS: the zip
  container, streaming inflate, a sheet-XML tokenizer, styles and shared strings. A cdylib.

Not there yet: the seven bindings, and the conformance corpus (HyperTabular's `corpus/`
directory is empty of xlsx and ods fixtures; HyperWorkbook's tests run on synthetic
fixtures built by openpyxl and by hand).

Design constraints round one already locked in on purpose:

- **The scalar doors are the inner loop.** Zero allocation per cast and verdict-as-span
  (`{code, offset, len}` into the caller's buffer) mean per-cell faults in a batch cost
  nothing and need no string materialization — a failed cell is a row/column index plus a
  span, reported in a parallel verdict array.
- **The batch entry point is additive.** Nothing about the scalar ABI changes. The batch
  lives beside it, not beneath it: `hypertabular` links `hypercast` as an rlib and each
  provider's cdylib exports the batch surface, so `libhypercast` itself never gains a batch
  export — and because the link is static, each provider's library also carries the 20
  `cast_*` exports.

One piece of this round already landed, ahead of schedule and on purpose:

- **Excel serial dates — done.** XLSX cells don't carry RFC 3339: dates are serial numbers
  under a workbook-level epoch (1900 or 1904), plus the deliberate `1900-02-29` phantom at
  serial 60 that Excel keeps for Lotus 1-2-3 file compatibility. This was parked here as "a
  natural, tiny addition to `temporal.rs` when this round starts" — it turned out to be
  exactly that, so it was built early rather than left to block the tabular layer. The door
  ships in every binding today (`cast_excel_serial`, a caller-declared `ExcelEpoch`, the
  phantom serial `Malformed` exactly as the text `1900-02-29` already is), with
  `corpus/excel_serial.json` holding it byte-identical across all eight languages. The door
  reads serial *text* — a CSV column of serials. HyperWorkbook's reader starts from the
  `f64` the file stores and converts it in `hypertabular::serial`, which carries the same
  rules (epoch, phantom 60, fraction as time of day) independently; nothing yet pins the
  two to agree.

Two designs this file once parked are now recorded and built:

- **XLSX container handling** — HyperWorkbook's `docs/design.md`, "Container and
  streaming": a hand-rolled central-directory zip reader, streaming inflate through
  `flate2` on the `zlib-rs` backend (the one external crate in the three repositories),
  and the shared-string preload as the documented allocating boundary — the same way
  HyperUuid's batch scratch buffer is its one documented allocating path.
- **Delimited-text dialect surface** — HyperDelimited's `docs/design.md`: one ASCII byte
  as separator, `"` as the only quote with RFC 4180 doubling, `\n`/`\r\n`/`\r` terminators,
  column count fixed by the first record. The same caller-declares-everything philosophy
  as `NumFormat`: no sniffing, no guessing.

Pattern prior art, studied and recorded in HyperTabular's `docs/prior-art.md` (direct
reads of each project's source, 2026-08-28): **nietras/Sep** is the model for what an
allocation-free, span-first C# tabular surface looks like (Svartalfheim's
`SepTabularReader` already sits on it), and that lesson then carries to
**Sylvan.Data.Excel** for the XLSX reader shape. Public flowers — the README
acknowledgment both deserve as the inspiration — wait until the end goal is achieved and
the numbers are in hand; this note is the engineering lineage, not the celebration.
