# Changelog

All eight packages in this repository — the `hypercast` crate and the C#, Java, Go, Python, Ruby,
PHP and Swift bindings — share one coordinated version, so one changelog covers all of them. Each
entry marks which packages it actually affects.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-08-31

First coordinated release — all eight packages published together from one tag, each verified by
installing it from its real registry and running every door, not by a green CI run alone. Full
notes: [v0.1.0 release](https://github.com/SkunkWerkx/HyperCast/releases/tag/v0.1.0).

### Added

- One allocation-free scalar parsing engine written in Rust and called directly from C#, Java, Go,
  Swift, Ruby, PHP and Python — published to crates.io, NuGet, Maven Central, PyPI, RubyGems and
  Packagist, with Swift and Go resolving from git tags.
- **Twenty doors over a plain C ABI.** Boolean (twenty lexemes, ASCII case-insensitive), the full
  integer family (i8–i64, u8–u64), reals (f32/f64), UUID (all five .NET `Guid` text forms plus
  `urn:uuid:`/`GUID:`/`UUID:` prefixes), RFC 3339 timestamp, Unix epoch at a declared precision,
  strict and declared-order dates, time, local date-time, Excel serial dates, and duration
  (ISO 8601, invariant colon form, protobuf JSON seconds).
- **A verdict, not a shrug.** Every door returns the value, or `Empty`/`Malformed`/`OutOfRange`
  plus the exact byte span that offended. Each binding presents that as its own platform's
  discriminated union — a native `[Union]` in C#, a `sealed interface` in Java, a compiler-
  mandatory-exhaustive `enum` in Swift, `match`/`case` in Python, pattern-matched `Data` in Ruby,
  `Success|Fault` union types in PHP, `(value, *Fault)` in Go — never an exception for bad data.
- **Culture is caller-declared, never sniffed.** Numeric doors take a `NumFormat` (separators plus
  individually declarable leniences: digit grouping, accounting parens, exponent, radix prefixes,
  percent); separated dates take a `DateOrder`. `NumFormat.DETECT` resolves `.`/`,` roles
  *structurally* per input and reports the genuinely ambiguous (`12.185`, `1,000`) as `Malformed`
  at the separator rather than guessing. Each binding bridges its own platform's culture machinery
  to both (`NumFormat.From(CultureInfo)`, `DateOrder.from(Locale)`, `DateOrder.from(locale:)`).
- **The corpus is the contract.** `corpus/*.json` — 380 vectors across twelve files — replays
  through the Rust core's suite *and* every binding's, so all eight agree byte for byte on both
  values and fault spans. Ruby replays it twice, once per backend.
- **The Rust core is `#![no_std]` and links no allocator**, under a default-on `std` feature; the
  crate publishes the `no-std` category and a bare-metal consumer supplies a `#[panic_handler]`
  and nothing else. Default-on rather than unconditional because the same crate builds the
  `cdylib` every other binding loads, and a linked artifact needs the panic handler only std
  supplies. *(`hypercast` crate)*
- **Allocation-free is asserted, not claimed.** `rust/tests/allocation_free.rs` wraps a counting
  `#[global_allocator]` around 1000 calls to every door on both the success and failure paths and
  demands zero. A fault is a span into the caller's buffer; nothing is captured or formatted on
  the error path. *(`hypercast` crate)*
- **Fuzzing, with the bugs it caught pinned as tests.** A `cargo-fuzz` target drives all 20 doors
  under six format profiles and every declared order/precision/epoch, asserting that no door
  panics on any byte sequence and that every fault span stays inside the caller's buffer. It found
  two real classes — truncation faults pointing one byte past the input, and `char_len` spans
  overrunning on text ending mid-UTF-8-character — both fixed structurally and pinned by
  `rust/tests/fault_span_invariant.rs` so they fail plain `cargo test`. *(`hypercast` crate)*
- **AOT on both managed platforms.** The C# package publishes cleanly under `PublishAot` into a
  genuine native binary that runs every door, and the Java binding ships FFM downcall signatures
  *and* a resources glob in its `reachability-metadata.json`, so a GraalVM Native Image consumer
  inherits both with zero configuration. Each is proven by a smoke test built against the
  published artifact, not against the working tree. *(NuGet, Maven)*
- **Blazor WebAssembly packaging.** The NuGet package ships `build/net11.0/HyperCast.targets`, so
  a browser-wasm consumer that adds only a `PackageReference` gets the staticlib linked and all 20
  doors exported — both halves required, and neither discoverable from a server-side build.
  *(NuGet)*
- Go's module is tagged separately as `go/v0.1.0`, a Go modules requirement for a subdirectory
  module, pushed alongside the bare tag.

### Notes

- **Per-language floors**, each chosen for a language feature the verdict type actually needs:
  .NET 11 (native `[Union]`), JDK 22 (FFM), Python 3.10 (`match`/`case`), Ruby 3.2
  (`Data.define`), PHP 8.1 (enums and union types), Swift tools 5.9 / macOS 13, Go 1.26.
- **Every binding beats its own platform's culture-machinery parser except Go**, and Go's loss is
  printed rather than omitted: its stdlib RFC 3339 path is genuinely excellent, and every Go door
  pays a crossing tax against it. The per-call story there waits for the round-three batch layer.
  Individual honest losses elsewhere are reported the same way — `Boolean.parseBoolean` and
  `UUID.fromString` on the JVM, `int.TryParse` with `AllowThousands` on .NET, Ruby's civil-date
  door.
- **WebAssembly** is proven for the Rust core — the full suite (51 unit tests, the allocation
  proof, all twelve corpus replays and both fault-span sweeps: 66 tests) passes under `wasmtime`
  on `wasm32-wasip1`. The C# browser-wasm story is proven as far as packaging and linking go; it
  has not yet been run in a real browser session, and is not claimed as such. Python's Pyodide
  path left deliberately with the retired ctypes backend — the abi3 wheels are real native
  extensions, which have no browser story.
- **Round three — tabular ingestion (CSV/TSV/XLSX), and the JSON layer behind it — is not in this
  release.** It is the reason the doors are shaped the way they are: zero allocation per cast and
  verdict-as-span exist so a batch layer can cross the FFI boundary once per chunk instead of once
  per cell. See [docs/roadmap.md](docs/roadmap.md) and [docs/json.md](docs/json.md).
- **0.0.1 and 0.0.2 were pipeline-proving pre-releases, not consumable ones.** Trusted Publishing
  has no local test path — the OIDC exchange only exists inside an Actions runner — so the only
  way to prove a publish leg is to run it against the real registry. Those two versions exist to
  have burned a cheap version doing that. Three of the four bugs the project has shipped were
  found in that window, in the gap between "the publish succeeded" and "a consumer can use it",
  and none of them could have failed a build in this repository.

[Unreleased]: https://github.com/SkunkWerkx/HyperCast/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/SkunkWerkx/HyperCast/releases/tag/v0.1.0
