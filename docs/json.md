# JSON ingestion — parked design

Banked from a design conversation (2026-08-31) so it doesn't get lost. **Status: parked.**
Nothing here is committed work: the round-three tabular layer ([roadmap](roadmap.md)) comes
first, and JSON rides the same contract once that exists. The reasoning is written down
alongside the conclusions so future-us can disagree with the specifics without re-deriving
the argument.

## Why a JSON layer belongs here at all

Every JSON library — serde_json, simd-json, sonic-rs, yyjson — **insists on parsing scalars
itself**: numbers through Eisel-Lemire, strings unescaped, done. That's wrong for this
project's world in two separate ways.

1. **Messy feeds carry scalars as strings.** `"price": "1.234,5"`, `"when": "1/7/2026 3:04
   PM"`, `"active": "yes"`. No JSON parser will ever type those for you — they are
   HyperCast doors, and they need the caller-declared semantics (`NumFormat`/`DETECT`,
   `DateOrder`, `UnixPrecision`) the doors already have.
2. **Even legitimate JSON numbers arrive as text spans.** A verdict with a reason and an
   offending span beats a silent `f64` — and `"price": 300` landing in a `u8` column should
   be `OutOfRange` with a span, which no JSON library gives you.

So the division of labor is: **structure parsing from a JSON library, scalar semantics from
the one engine that's byte-identical in eight languages.** Same split the roadmap already
parks for XLSX.

## What we need from the structure parser

Hand over **raw scalar byte spans without interpreting them**, pure Rust, so scalars are
parsed exactly once — by the doors.

| Tier | Candidates | Notes |
| --- | --- | --- |
| Boring (start here) | `serde_json` `&RawValue`; pull parsers (`struson`, `json-event-parser`) | Structure + syntax validation, no number conversion, no unescaping — values stay borrowed text. Scalar bytes get scanned twice (extent, then door), but no float is ever built twice. Dead simple to hold correct. |
| Fast lazy | `sonic-rs` (`LazyValue`) | The closest pure-Rust match to simdjson On-Demand: SIMD structural navigation, raw bytes per node without materializing. **Verify fresh:** aarch64/NEON path has historically trailed AVX2, and it leans on a lot of `unsafe`. |
| Max | simdjson **stage 1** (structural indexer) | The right algorithm, but `simd-json`'s public API runs stage 2 — which parses numbers and destructively unescapes strings, exactly what we don't want. Using stage 1 alone means vendoring. Later, if ever. |

Non-starter as-is: anything that only exposes a fully-materialized DOM.

Discipline, as everywhere in this repo: build on the boring tier behind a seam, then
benchmark SIMD **with receipts** before buying the complexity. If the structure scan turns
out to be 10% of chunk time, the boring parser wins on maintenance forever.

## The FFI question dissolves

The concern that motivated this: "one crossing per scalar would eat the win." It doesn't
apply, because of *where the layer lives*.

The JSON provider is **its own crate** (the core stays zero-runtime-deps — that's an
advertised receipt), depending on `hypercast` **as an rlib**. Inside that crate the doors
aren't FFI at all — they're ordinary inlined Rust calls. The per-scalar crossing cost isn't
amortized; **it doesn't exist**.

The only FFI in the whole path is the provider's own chunk API: host hands over a buffer of
NDJSON/JSON plus a schema, gets back column-shaped verdict arrays. One crossing per chunk,
structure parsed once, scalars parsed once.

## The real challenge: the row type lives in the binding's language

Stated plainly, this is the thing that looked like it broke the concept. It doesn't. The
move is: **don't get the binding's *type* across the boundary — ship a *schema descriptor*
across instead, once, at setup, and get columnar verdicts back.** The type stays in the
host language; what crosses is a description of it.

- **Across:** field paths, each mapped to a door plus its declarations (`price` → f64 door,
  `DETECT`; `when` → datetime door, `MonthDayYear`). Plain data.
- **Back:** flat parallel arrays per field — a value buffer, a reason array, span arrays.

**Reading a returned buffer is not a crossing.** Materializing host objects from flat arrays
is pure host-side memory access plus the object allocation you'd pay for host objects no
matter who parsed them.

### The schema *is* where the declarations live

This is why the constraint turns out to be the design rather than a workaround for it.
HyperCast's whole stance is caller-declared semantics per call site. In a deserialization
world, "per call site" becomes **per field** — and a schema descriptor is exactly *per-field
declarations, as data*. A schemaless design (hand the host a tape and let it walk) can't
express "this field is day-first, that one is separator-detect"; you'd be back to guessing,
which this project doesn't do.

Prior art for the shape: Arrow made the same bet — schema as data plus flat buffers — for the
same polyglot reason: it's how you share typed structure across languages without sharing
types.

### Three tiers of host-side materialization, one wire contract

1. **Hand-written glue.** A dozen lines per row type reading `values[i]`/`reasons[i]` into a
   constructor. Fine forever for app code, and it's all the dynamic half of the roster needs
   — Python/Ruby/PHP build objects dynamically anyway, and introspecting a dataclass's
   annotations to *derive* the schema at startup is idiomatic there, not a hack.
2. **Reflection at setup, not per row.** Java and C# walk a record's shape once at startup,
   emitting both the schema descriptor and a materializer delegate. Reflection cost is
   O(types), not O(rows). Closed to C#-under-AOT, which is what motivates tier 3.
3. **Source generators.** The idiomatic endgame for the static half: a C# incremental
   generator over something like
   `[HyperRow] partial record Order([Format(Detect)] Verdict<double> Price, [Order(Mdy)] Verdict<DateTime> When)`
   emitting the schema blob plus a zero-reflection materializer; a Java annotation processor
   doing the same. Exactly the move `System.Text.Json` made with `JsonSerializerContext` for
   the same AOT reasons — and already this project's house style, since the C# binding is
   source-generated `[LibraryImport]` top to bottom.

Row objects are the *minority* use anyway: a million-row ingest wants columns (arrays,
spans, DataFrame-shaped things), which is what the wire contract returns natively. Row-object
mapping is optional last-mile sugar, not the load-bearing path.

## Design points to settle when this gets built

Both are contract-shaped, so they need deciding before the wire format sets.

- **Escaped strings and fault spans.** A JSON string's raw span includes quotes and escapes;
  doors need unescaped bytes. Fast path — no backslash in the span (the overwhelming
  majority) — the door reads the slice between the quotes, zero-copy. With escapes, unescape
  into scratch, and then **the fault span indexes the unescaped value text, not the original
  document**. That has to be stated in the contract honestly, not papered over.
- **Raw JSON numbers vs. strings-holding-scalars.** A bare JSON number can't legally carry
  grouping or parens, so it goes through the doors under `INVARIANT` and gets real range
  verdicts. The messy stuff arrives as strings and gets the full declared-format /
  `DETECT` / `DateOrder` treatment. Worth keeping these two paths visibly distinct.

## Scope line

Path-per-field schemas cover **NDJSON-of-records** (the ingestion case) cleanly. Genuinely
dynamic or deeply nested subtrees come back as raw spans for the host to route, or don't
cross at all. Draw that line explicitly rather than chasing full document-object mapping —
that's serde's job, not this project's.

## Bonus: a Rust-native on-ramp

Nothing stops the crate from also shipping a serde `Deserialize` impl for `Verdict<T>`, so
plain Rust users write `#[derive(Deserialize)] struct Row { price: Verdict<f64> }` and get
verdict semantics inside ordinary serde_json. It loses the parse-once property for bare
numbers (serde already parsed them), but as an ergonomic on-ramp it's nearly free.

## Sequencing

1. Round-three tabular layer lands first — the columnar verdict contract is the engine, and
   it serves CSV/XLSX/JSON alike.
2. JSON provider on the boring structure tier, tier-1 hand-written glue, proving the schema
   descriptor end to end.
3. Benchmark a SIMD structure parser against it, with receipts, before adopting one.
4. Source generators as a later additive round (HyperUuid's C# generator experience is
   already banked).

## Freshness caveat

The library survey above reflects early-2026 knowledge. Version-level claims — sonic-rs's
current aarch64 quality, `facet`'s maturity as a serde alternative — deserve a fresh look at
the point we actually pick a horse.
