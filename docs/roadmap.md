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
  same proven shapes: the Rust core under `wasm32-wasip1` and `wasm32-unknown-unknown`,
  C# via Blazor's `NativeFileReference` static linking, Python as a Pyodide wheel, Go
  through TinyGo and Swift through its WebAssembly SDK, each linking the core in. HyperCast's
  core is strictly easier freight than HyperUuid's here — pure computation over caller
  bytes, zero dependencies, no WASI clock or randomness imports at all — and every leg is
  proven, not projected: the full test suite (unit tests, the counting-allocator proof,
  every corpus replay, both fault-span invariant sweeps) passes under `wasmtime` on
  `wasm32-wasip1`
  (`CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime --dir <repo>" cargo test --target
  wasm32-wasip1`; the preopen is only so the conformance test can read `corpus/`), and the
  browser targets run in headless Chrome in CI, all but ruby.wasm on every pull request (see the root README's
  WebAssembly section). No binding carries an in-process wasm backend: each links the core
  or loads a native library on every platform it ships for, so a fallback would have
  nothing to catch, and the jar is FFM downcalls into the library it bundles.
- **The tabular layer rides the wasm train too.** It needs no contortions: HyperTabular's
  core is no_std, allocation-free and imports nothing, so zip, inflate and XML run over
  caller bytes in a sandbox exactly as natively, and it ships every leg HyperCast
  does — Blazor, TinyGo, Swift's WebAssembly SDK, Pyodide and ruby.wasm.

## Round one — the scalar core (done)

One Rust `cdylib` (`rust/`, `libhypercast`), HyperUuid's proven FFI mechanics: 21 `cast_*`
exports (25 since 0.7.0 added four typed doors, 26 since 0.8.0 added the char door) over UTF-8 bytes and caller-owned
out-buffers, verdict codes (`0` ok, `1` empty, `2` malformed, `3` out of range) with the
offending byte span through a nullable fault out-param. Semantics ported from Svartalfheim's
`Norse.Primitives` parser family; temporals land in protobuf's dual-integer forms
(`{seconds, nanos}` timestamp/duration, `{y, m, d}` date, nanos-since-midnight time) so
every binding presents them at its platform's own fidelity. Allocation-free on success *and* failure paths, asserted by a counting global
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

## Round three — the payoff: tabular ingestion (done)

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

This round shipped as **[HyperTabular](https://github.com/SkunkWerkx/HyperTabular)** 0.7.0
(2026-10-07): delimited text and XLSX/ODS workbooks, one crate and one native library, on
this repo's 0.7.0, in all eight languages, with a 231-case conformance corpus that includes
files written by Excel, LibreOffice and Google Sheets. It began as three repositories — a
contract crate and two provider cdylibs, HyperDelimited and HyperWorkbook — and was folded
into one when the linker ruled on that shape: two provider archives each carried Rust's
standard library and this crate's `cast_*` symbols, so no binding that links the core
rather than loading it (Go, Swift, C# Native AOT) could take both. HyperTabular's
`docs/design.md` ("One repository, one library") has the record. Apple Numbers documents
have since joined XLSX and ODS on HyperTabular's master, for its next release, on the same
scalar doors.

Design constraints round one already locked in on purpose:

- **The scalar doors are the inner loop.** Zero allocation per cast and verdict-as-span
  (`{code, offset, len}` into the caller's buffer) mean per-cell faults in a batch cost
  nothing and need no string materialization — a failed cell is a row/column index plus a
  span, reported in a parallel verdict array.
- **The batch entry point is additive.** Nothing about the scalar ABI changes. The batch
  lives beside it, not beneath it: `hypertabular` links `hypercast` as an rlib and its own
  library, `libhypertabular`, exports the batch surface, so `libhypercast` itself never gains a batch
  export. The link is static, so HyperTabular takes the crate with `default-features =
  false`: the `cast_*` symbols are an `exports` feature (0.7.0), and leaving it off keeps
  them out of HyperTabular's library, which would otherwise collide with `libhypercast`'s
  own when both are linked into one program.

One piece of this round landed here, ahead of schedule and on purpose:

- **Excel serial dates — done.** XLSX cells don't carry RFC 3339: dates are serial numbers
  under a workbook-level epoch (1900 or 1904), plus the deliberate `1900-02-29` phantom at
  serial 60 that Excel keeps for Lotus 1-2-3 file compatibility. This was parked here as "a
  natural, tiny addition to `temporal.rs` when this round starts" — it turned out to be
  exactly that, so it was built early rather than left to block the tabular layer. The door
  ships in every binding today (`cast_excel_serial`, a caller-declared `ExcelEpoch`, the
  phantom serial `OutOfRange` exactly as the text `1900-02-29` is), with
  `corpus/excel_serial.json` holding it byte-identical across all eight languages. The door
  reads serial *text* — a CSV column of serials. A workbook reader starts from the `f64`
  the file stores; `excel_serial` is the same rules for that number, from the same code,
  and the corpus is replayed through both. That typed door, with a decimal, a time-of-day
  and a duration read from the same `f64`, is now exported to every binding as well, and
  `corpus/typed.json` holds the four byte-identical across all eight languages.

Two designs this file once parked are now built, and recorded in HyperTabular's docs:

- **XLSX container handling** — `docs/workbook.md`, "Container and streaming": a
  hand-rolled central-directory zip reader (zip64 honoured, encryption refused), the core's
  own inflate into a sliding window the caller provides, and one pull tokenizer for every
  XML part. It allocates nothing — the shared-string preload this file once expected to be
  the series' one allocating path never became one.
- **Delimited-text dialect surface** — `docs/delimited.md`, "What is fixed here, and why":
  one ASCII byte as separator, `"` as the only quote with RFC 4180 doubling,
  `\n`/`\r\n`/`\r` terminators, column count fixed by the first record. The same
  caller-declares-everything philosophy as `NumFormat`: no sniffing, no guessing.

Pattern prior art, studied and recorded in HyperTabular's `docs/prior-art.md` (direct
reads of each project's source, 2026-08-28): **nietras/Sep** is the model for what an
allocation-free, span-first C# tabular surface looks like (Svartalfheim's
`SepTabularReader` already sits on it), and that lesson then carries to
**Sylvan.Data.Excel** for the XLSX reader shape. Public flowers — the README
acknowledgment both deserve as the inspiration — were to wait until the end goal was
achieved and the numbers were in hand. Both now are, and HyperTabular's README does not
yet carry that acknowledgment; its `docs/design.md` records the lineage, which is the
engineering record, not the celebration.

## What the first consumer asked for (2026-09-04)

Svartalfheim's ingestion branch
([NorseArchitecture/Svartalfheim#62](https://github.com/NorseArchitecture/Svartalfheim/pull/62))
is the first real consumer to route an entire scalar-parser family through the C# binding,
keeping its own managed code as the fallback for platforms the package does not cover. Its
design record (Glitnir, `docs/Svartalfheim/specs/2026-09-03-hyperuuid-hypercast-ingestion-design.md`,
§9) lists five upstream asks; the PR itself, read against this repo, adds several more and
retracts two. Sorted into what already exists, what the core is missing, and what only the
binding is missing.

### Already covered — the consumer could not find it

- **A caller-declared separator pair and per-call lenience flags** (§9 items 4 and 5). Both
  exist: `NumFormat` is a positional record, so `new NumFormat(',', '\u00A0', NumStyles.All)`
  declares an arbitrary pair, and `new NumFormat('.', ',', NumStyles.None)` turns every
  lenience off, percent included. The seven corpus vectors the consumer excludes are excluded
  because *its* public API takes an `IFormatProvider` and never surfaces a `NumFormat`, not
  because this binding lacks a door. The spec's §9 should be corrected, and the C# README
  should show the direct-construction form alongside `From(CultureInfo)` so the next reader
  finds it.
- **`UuidGenerator.NewV5(Guid, ReadOnlySpan<byte>)`** in HyperUuid. The consumer's
  `DeterministicGuid` calls the `string` overload through `name.ToString()` and
  `Encoding.UTF8.GetString`, then guards with `Utf8.IsValid` to fix a "non-UTF-8 bytes hash
  differently on native and managed" bug. The byte-span door hashes raw bytes and would have
  made the two engines agree with no allocation and no guard.
- **`Cast.Unix`.** The consumer's `ParseUnix` stays entirely managed, and its own corpus
  harness notes the managed door reports out-of-range epochs as `Malformed`, diverging from
  `unix.json`.

### What was missing, and where it stands

Everything below except the build matrix landed together on one branch the same day, as
one coordinated ABI bump across the core and all seven bindings — the CHANGELOG entry is
the record of what shipped; this is the record of why.

1. **A declared currency symbol on `NumFormat` — built.** The consumer gated *both* its
   integer and real parsers to invariant-culture callers only, because its `NumberStyles`
   include `AllowCurrencySymbol` and the core had no field for one. Every non-invariant
   culture therefore took the managed path — the native engine was dead on exactly the
   culture-machinery inputs the README's benchmark table is built on. The format now
   carries the symbol inline (a `CURRENCY` flag gates it, part of `ALL`), matched once at
   either edge of the numeric body, and every binding's culture bridge fills it in.
2. **A decimal door — built.** `RealParser<decimal>` stayed managed, with its own 29-digit
   guard and a `double`-probe to tell overflow from garbage. `cast_decimal` produces a sign,
   a 96-bit magnitude and a scale by pure integer scanning — no float anywhere, never
   rounded — and the same triple maps onto `decimal`, `BigDecimal`, `decimal.Decimal`, and
   an honest carrier type where the platform has no exact decimal.
3. **An ABI-version export — built.** The consumer's capability probe called HyperUuid's
   `TryNewV4` and assumed HyperCast loaded too; if `hypercast` failed to resolve while
   `hyperuuid` did, a `DllNotFoundException` escaped from a parser that promises never to
   throw (Codex's review of the PR found the same hole independently). `hypercast_version`
   is the zero-argument probe, and every binding fronts it with an availability check plus
   the loaded core's version.
4. **musl builds — built; iOS and Mac Catalyst — built (0.7.0); Android — built (0.8.0).** The forge built the three desktop OS families
   on x64 and arm64 plus browser-wasm, and nothing for `ios-*`, `android-*` or
   `linux-musl-*`. That gap is the sole reason the PR carries several hundred lines of
   managed fallback grammar duplicating this core — RFC 3339, the three-shape duration
   grammar, separator detection. `linux-musl-x64` and `linux-musl-arm64` are now built in
   Alpine containers, attested, and shipped in every binding that has a dynamic-loading
   story on musl; Swift, which has none, links the core in statically instead. Alpine no
   longer takes the fallback. Since 0.7.0, C#, Swift and Go link the core into iOS and Mac
   Catalyst apps from static archives built in the same job, as HyperUuid's do. Since 0.8.0
   the same three reach Android, arm64 and x86_64: the C# package carries the shared library
   and Native AOT archives, so a MAUI app has the core on every platform MAUI targets, and Go
   and Swift link the same archives. The forge gained an opt-in `android` input to build
   them, which HyperUuid has since turned on too.
5. **A corpus content package — declined.** The consumer vendored the corpus files plus a
   snapshot SHA by hand and asked for a package. The ruling is that the corpus is this
   repository's receipt, not a product: a downstream suite takes `corpus/*.json` from the
   tag its core was built against, and the SHA it records is the pin. (One `HyperCast.Corpus`
   version reached nuget.org before that ruling was applied to the release train; it is
   unlisted and will not be followed.)

### Binding quality of life — built, system-wide

Each of these started as a C# finding and was then ported to every binding where the
language has an idiomatic home for it, with the exceptions named:

- **One numeric door generic over the target.** The consumer wrote an eight-arm
  `switch (typeof(T))` plus a helper that uses `TryGetValue` because CS8780 blocks union
  pattern matching on a generic type argument. C# `Cast.Numeric<T>`, Go `Numeric[V]`, Swift
  `Cast.numeric<T>`; not in Java (no generics over primitives) nor the dynamic bindings,
  where the per-width doors are the idiom.
- **A culture bridge in the shape the platform already uses.** C# `NumFormat.From(IFormatProvider)`
  beside `From(CultureInfo)`; PHP `fromLocaleconv`; Java, Swift and Python already had one.
  Go and Ruby have no locale data in their standard libraries.
- **An availability probe.** The binding half of item 3: a probe-once, never-throwing check
  in every binding that loads or links a library (Python's extension is the package itself,
  so a successful import is the probe), so a consumer with a fallback gates on it instead of
  catching a load failure around its first real cast.
- **Fault spans in the caller's own units.** The core reports UTF-8 byte offsets; the C#
  `string` doors, Java `String` doors, Python `str` and Ruby text inputs now remap to
  char/code-point offsets on a non-ASCII failure, so slicing the offending text back out of
  what was passed needs no mapping. Go, Swift and PHP strings are already UTF-8 bytes.
- **HyperUuid: `NewV5(Guid, ReadOnlySpan<char>)` — built.** The `string` door already
  transcoded into a stack buffer; the char-span overload is the same code and removes the
  consumer's `ToString()`. HyperUuid also gained the load probe this section asks for
  (`hyperuuid_version`, `UuidGenerator.IsAvailable`), so a consumer no longer infers that
  library's presence from a `TryNewV4` call.

Found along the way, not acted on, worth their own pass. **The currency path costs about
twice the plain path in the core** — `cast_decimal` 30.5 ns for `12345.6789` against 58.8 ns
for `$12,345.67` under a declared `$`, and the C# binding's `Cast.Double`/`Cast.Decimal` on
`($1,234.50)` land at ~116 ns against ~70 ns for the BCL's `NumberStyles.Currency`. A
declared symbol currently forces the full normalize-then-parse engine; a fast lane that
strips a symbol at one edge and re-enters `is_plain` would recover most of it. **Built
since, though not as proposed:** measuring each notation on its own showed the symbol was
never the cost. Grouping alone was as slow as grouping, symbol and parentheses together
(41.5 ns against 44.5 for an i64), because the price was leaving the plain path at all, so
stripping a symbol and re-entering it would have helped `$12345.67` and nothing a person
actually types. `rust/src/lane.rs` is a second fast lane for the whole family instead —
grouping, a declared symbol, accounting parentheses — and brings an i64 from 41–45 ns to
14, a decimal from 42–50 ns to 21–24, and a real from 40–42 ns to 33. The plain decimal
path went from 25.5 ns to 11.8 on the way. Through the C# binding, same machine and
session, `Cast.Decimal` on `($1,234.50)` went from 104 ns to 66 against the BCL's 60, and
`Cast.Double` from 83 ns to 74 against 52: the decimal door is level with
`NumberStyles.Currency` now, and the real door still trailed it, because a real was
copied without its separators for `core`'s float parser where a decimal is read in place —
a copy 0.7.0 removed: the lane now hands the crate's own conversion (`float.rs`) the
significand directly.
And, **Python's `bytes` input cost about
a microsecond more than the same text as `str`** on every native door, success and failure
alike — `"42"` at 116 ns against `b"42"` at 1,179 ns on the same box. The cause was the
derived PyO3 `Text` extractor trying the `str` variant first and materializing a downcast
error before falling through to `bytes`. **Built since:** a hand-written extractor checks
the type tag directly, and the two inputs now cost the same.

Not found: any core hot-path optimization evidenced by the PR. The consumer's own re-measured
ratios sit inside the crossing-tax band the C# README already describes, and its one outlier turned out to
be a missing routing branch on its side. The wins on offer are capability gaps, not speed.

## What the consumer asked for next (2026-10-08)

Norse is removing its last hand-written scalar parsers in favour of HyperCast and filed two
issues against the C# binding. Both landed in every binding where the language can express
the idea; neither stayed C#-only.

- **A char door ([#22](https://github.com/SkunkWerkx/HyperCast/issues/22)) — core, built.**
  Norse's `CharParser` was the one scalar grammar with no counterpart here: exactly one
  character taken verbatim (before trimming, so `" "` is a space), or one declared code
  point (`65`, `U+0041`, `0x41`, `&H41`, `&#65;`, `&#x41;`). The grammar lives once in the
  core as `cast_char`, the twenty-sixth export, and `corpus/char.json` is its contract.
  The issue planned for the other bindings to add the door later. It shipped in all eight at
  once, because the corpus claim ("all eight replay every file") is only worth anything if it
  stays true. Each binding returns its own character type, so the decision was what to do
  where that type is narrower than a scalar: C# and Java's UTF-16 `char` turns a scalar
  past `U+FFFF` into `OutOfRange` (the issue's choice, and Norse will converge on it) rather
  than returning half a surrogate pair.
- **One door generic over every target ([#21](https://github.com/SkunkWerkx/HyperCast/issues/21))
  — binding, built where `Numeric<T>` already lives.** `Cast.Scalar<T>` in C#, `Scalar[V]`
  in Go, `Cast.scalar<T>` in Swift; not in Java or the dynamic bindings, for the reasons the
  numeric door gave. The decision that needed making was `DateTime`, which in C# names two
  doors. `Scalar<DateTime>` reads RFC 3339 and comes out as `UtcDateTime`, the one reading
  that needs no declared order; a caller wanting the civil door calls `Cast.DateTime`
  directly. The same question excluded types elsewhere: Go's `rune` is its `int32`, and
  Swift's `DateComponents` is the result of three doors, so neither is dispatchable.

## Culture fidelity — the sweep (built, 0.9.0)

Every binding takes its platform's own culture object (`IFormatProvider`, `Locale`,
`NumberFormatter`, Rails' `number.format`), but until 0.9.0 what reached the core was mostly
the en-US shape: the bridges could only fill the fields `NumFormat` had, and the core
hard-coded the rest as ASCII and English. Found while landing the `IFormatProvider` overloads
for Norse Architecture (2026-10-08); fixed in one coordinated round, the core and all seven
bindings at once, under the project's own rule: **culture is declared, never sniffed.** Each
gap became something a format (or the datetime door's declaration) carries, the core reads
exactly what is declared and defaults to the invariant behavior, every binding's bridge fills
it from its platform's data, and `corpus/` pins each row.

One rule covers every new field: **the ASCII or English default is always read, and a
declared symbol is a further spelling read beside it.** So nothing that parsed in 0.8.0
reads differently, a bridge never has to re-declare `-` to keep it, and a culture whose
minus is U+2212 still reads the hyphen its users type anyway (as the BCL does).

| Gap | 0.8.0 | Built |
| --- | --- | --- |
| **Sign** | Only ASCII `+` and `-`. | `minus_sign` and `plus_sign` in `NumFormat`, up to 8 UTF-8 bytes each, read wherever the ASCII signs are: ahead of the digits, before or after a leading currency symbol. Two signs fault at the later one. |
| **Trailing sign** | `5-` was Malformed. | The `TRAILING_SIGN` lenience: `5-`, `5 -`, `5+`, `5 €-`, `5- €`, `2.5%-`, `2.5-%`; never beside a leading sign or inside parentheses. Not in `ALL`: each bridge sets it from the culture's negative pattern (`n-`, `n -`, `n-$`, a negative suffix, `n_sign_posn`). |
| **Currency separators** | A currency amount was read with the number separators. | `currency_decimal_sep` and `currency_group_sep`, read in place of the number separators once the declared symbol is found in the text, as the BCL switches when it sees the symbol. Each falls back to its number counterpart when not declared; detection (`SEPARATOR_DETECT`) governs the whole token and ignores them. |
| **Percent symbol** | Only an ASCII `%`. | `percent`, read as a trailing `%` is (`50٪`, `５０％`). Position is unchanged: a leading percent (`%50`, Turkish) is still Malformed, and is the one row left open below. |
| **Native digits** | Only ASCII `0`–`9`. | `digits`, the zero code point of a contiguous block (`٠` U+0660, `０` U+FF10, as ICU and Java describe a locale's digits), read beside ASCII everywhere a digit is: integer part, fraction, exponent. Radix-prefixed digits stay ASCII. |
| **AM/PM designators** | Only English `AM`/`PM`, after the clock. | `DayPeriods` beside the declared `DateOrder` (`cast_datetime_with`; the C ABI's `cast_datetime` takes a `RawDayPeriods` pointer after the order, null for English only). Read after the clock (`3:04 nachm.`) or ahead of it (`오후 3:04`, the CJK order) — and English is now read ahead of the clock too. Declared designators match exactly, up to 16 bytes with a space inside allowed (`a. m.`); English matches ASCII case-insensitively. |
| **Separators wider than one character** | One `char` per separator. | Separators are `Symbol<8>`s: 1 to 8 UTF-8 bytes (`, `, an LTR mark and a comma). Every binding's separator fields became strings. |
| **Boolean words** | Svartalfheim's English lexicon only. | **Decided: the boolean door stays English, documented.** No platform's culture data carries a boolean lexicon (`bool.Parse`, `Boolean.parseBoolean` and `bool()` are English or not parsers at all), so no bridge could fill one from its platform's data, which is the sweep's rule for every field. A feed that says `ja`/`oui` maps its words before the door. |
| **Swift per-user overrides** | The `locale:` overloads cached by locale identifier. | The cache is skipped for `.current` and `.autoupdatingCurrent`, which are derived per call so a user's own number-format overrides are honored; explicit identifiers stay cached. Measured in `swift/README.md`: 60 to 70 µs a call on Linux Foundation, where `.current` carries no overrides at all, so a hot loop passes an explicit locale or a held `NumFormat`. |

**The ABI shape, decided once.** `RawNumFormat` is a size-tagged layout, and the layout is
the core's own `#[repr(C)] NumFormat` behind a size word (92 bytes, 4-aligned; every symbol
its bytes then its length). The core reads the caller's struct *in place* — a cast pays for
a validation pass and no copy — which is why the first attempt, a copy into a differently
laid-out `NumFormat`, lost 11 ns per crossing to store-forwarding stalls and was abandoned.
The size word lets the layout grow again without a coordinated bump: a caller states how many
bytes it built, the core reads those and defaults the rest, and a size it does not know is a
contract violation. `RawDayPeriods` (40 bytes) has the same shape. Measured through the C ABI
on one machine, a grouped `cast_i32` under a declared `$` went from 21.9 ns on the 0.8.0 core
to 19.6 ns, and under `¤` from 22.2 to 20.9 ns; the sweep costs nothing at the crossing.

Still open, for the record: a percent symbol ahead of the number (`%50`); separator
detection, which counts ASCII digits only, so a token in native digits needs a declared
format; and Indian lakh grouping was never a gap (`12,34,567` is accepted). Python's and
Go's standard libraries have no per-object locale to take, and Java's Native Image includes
only the locales named at build time (`-H:IncludeLocales`), which is the consumer's choice.

## Open follow-ups (as of 2026-10-25)

- Benchmark tables were measured on the WSL2 i9-11900H box. When the dev machine changes,
  re-measure every harness there before relabelling a table; never mix machines, and say
  which tables predate a core change. A benchmarking workflow is an idea, not started.
- Java does not reach Android: the binding uses FFM, which ART lacks, so it would need a
  JNI layer. Python, Ruby and PHP don't reach Android either.
- A GraalVM Native Image static-link spike (the core linked into the image rather than
  loaded) was proven on linux-x64 and parked, because the only way in is GraalVM's internal
  API; the Forge README's ledger has the recipe and the trigger to revisit.
