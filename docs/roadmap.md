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
  same three proven shapes: the Rust core under `wasm32-wasip1`, C# via Blazor's
  `NativeFileReference` static linking, Python via an Emscripten side module in Pyodide.
  HyperCast's core is strictly easier freight than HyperUuid's here — pure computation
  over caller bytes, zero dependencies, no WASI clock or randomness imports at all — and
  the core leg is already proven, not projected: the full test suite (51 unit + the
  counting-allocator proof + all 12 corpus replays) passes under `wasmtime` on
  `wasm32-wasip1` today (`CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime --dir <repo>" cargo
  test --target wasm32-wasip1`; the preopen is only so the conformance test can read
  `corpus/`).
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

Design constraints round one already locked in on purpose:

- **The scalar doors are the inner loop.** Zero allocation per cast and verdict-as-span
  (`{code, offset, len}` into the caller's buffer) mean per-cell faults in a batch cost
  nothing and need no string materialization — a failed cell is a row/column index plus a
  span, reported in a parallel verdict array.
- **The batch entry point is additive.** Nothing about the scalar ABI changes; the tabular
  layer is new exports beside it, not a rework beneath it.

Known design work for this round, parked deliberately:

- **Excel serial dates.** XLSX cells don't carry RFC 3339 — dates are serial numbers
  (1900/1904 epochs, plus the deliberate 1900 leap-year bug kept for Lotus 1-2-3
  compatibility). An "Excel serial → Timestamp" door is a natural, tiny addition to
  `temporal.rs` when this round starts.
- **XLSX container handling.** XLSX is zip + XML (inflate, shared-strings table, cell-type
  attributes). Streaming decompression versus caller-buffer protocols is the real design
  conversation, because it's the first place "never allocates" needs a deliberate,
  documented boundary — the same way HyperUuid's batch scratch buffer is its one
  documented allocating path.
- **Delimited-text dialect surface.** Quoting, embedded newlines, separator declaration —
  the same caller-declares-everything philosophy as `NumFormat`: no sniffing, no guessing.

Pattern prior art, studied deliberately: **nietras/Sep** is the model for what an
allocation-free, span-first C# tabular surface looks like (Svartalfheim's
`SepTabularReader` already sits on it), and that lesson then carries to
**Sylvan.Data.Excel** for the XLSX reader shape. Public flowers — the README
acknowledgment both deserve as the inspiration — wait until the end goal is achieved and
the numbers are in hand; this note is the engineering lineage, not the celebration.
