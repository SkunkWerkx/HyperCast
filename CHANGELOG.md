# Changelog

All eight packages in this repository — the `hypercast` crate and the C#, Java, Go, Python, Ruby,
PHP and Swift bindings — share one coordinated version, so one changelog covers all of them. Each
entry marks which packages it actually affects.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

*One core, one more way in*, ported from HyperUuid 0.3.0: Java, Ruby, Python and Go can now
run the Rust core as a `wasm32-wasip1` module inside the process, through a wasm engine the
ecosystem already has, so a platform with no native build in the package still has a working
backend and nothing has to be `dlopen`'d at all. No door changed its verdict on any input:
the corpus replays byte-identical through every backend, native and wasm alike.

### Added

- **A wasm backend in Java, Ruby, Python and Go.** The core built as a `wasm32-wasip1`
  module, `hypercast.wasm`, ships beside the native libraries in the jar, the gems and the
  wheels, and is committed under `go/native/`; a wasm engine the ecosystem already has runs it
  in-process, behind each binding's existing backend switch, with the engine an optional
  dependency the consumer adds only if they want this path:
  - **Java** — [GraalWasm](https://www.graalvm.org/webassembly/), `-Dhypercast.backend=wasm`,
    or automatic when the jar has no native build for the platform. `org.graalvm.polyglot:wasm`
    is `compileOnly` and never in the POM; `Cast.backend()` reports which path won. The seam is
    one level below the verdict (`Backend`), so every reader, exception and message is one
    implementation for both paths.
  - **Ruby** — the [wasmtime](https://rubygems.org/gems/wasmtime) gem, `HYPERCAST_WASM=1`, or
    automatic when no native library exists for the platform. It redefines only the three
    private crossing bodies under the doors; `HyperCast::BACKEND` reports `:wasm`, and
    `spec/wasm_backend_spec.rb` pins the outputs against Fiddle.
  - **Python** — [wasmtime-py](https://github.com/bytecodealliance/wasmtime-py) via
    `pip install hypercast[wasm]`, `HYPERCAST_WASM=1`, or automatic when the PyO3 extension
    fails to import. `hypercast.BACKEND` reports `"wasm"` or `"native"`; the backend presents
    the same `Success`/`Fault`/`NumFormat` types, and `tests/test_wasm_backend.py` pins it
    against the extension.
  - **Go** — [wasmtime-go](https://github.com/bytecodealliance/wasmtime-go) behind
    `-tags hypercast_wasm`, opt-in only and never selected automatically; the tag compiles in
    exactly one backend. cgo throughout, so no win-arm64 build.

  Measured on one box (linux-arm64, WSL2), through each shipped binding, `i32` and `timestamp`
  one call each: 237 (grouped) / 354 ns from Java under GraalVM CE 25.3's JIT and 20.1 / 8.5 µs
  on a stock Temurin 25, where GraalWasm runs interpreted; 2.65 / 3.10 µs from Ruby — Fiddle
  parity, door for door; 6.4 / 7.7 µs from Python; 3.4 / 3.8 µs from Go; against 66 / 60 ns,
  555 / 769 ns (Magnus), 139 / 423 ns and 84 / 101 ns native. The Java wasm path also
  survives a GraalVM Native Image build on the jar's own reachability metadata
  (`./gradlew :aot-smoke-test:nativeRun -Pwasm`). Every call is serialized under a lock, because neither a GraalWasm
  `Context` nor a wasmtime `Store` is safe for concurrent use; the native backends stay
  lock-free. The module exports wasi-libc's `malloc`/`free` through two linker flags in
  `rust/.cargo/config.toml`, because a host-picked offset into the guest's initial memory
  collides with dlmalloc. CI builds the module on every leg and runs the four suites a second
  time through it. *(Maven Central, RubyGems, PyPI, `go get`)*
- **Backend-agreement specs in Ruby.** `spec/native_backend_spec.rb` compares Magnus against
  Fiddle across a subprocess boundary, the contract the README already described and the
  suite did not yet pin; `spec/wasm_backend_spec.rb` does the same for wasm. *(RubyGems)*
- **The wasm module is attested like every native library.** `hypercast.wasm` carries the
  same build-provenance attestation as the six native builds, signed by the reusable workflow
  in `SkunkWerkx/.github`, and `stage-native-binaries.yml` refuses to commit it under
  `go/native/` unless that attestation verifies. *(release machinery)*
- **A wasm dev loop in every binding.** The Java build stages `rust/target/wasm32-wasip1/`
  beside the native library, and the Ruby and Python backends fall back to the same in-repo
  build when nothing is staged — the same shape the Fiddle runtime already had. *(dev only)*

### Changed

- **The allocation proof counts per thread.** `rust/tests/allocation_free.rs` moved from a
  process-wide atomic to a `const`-initialised thread-local, so the test harness's own
  allocations on its main thread — a join-handle map insert and a timeout push right after
  `spawn` — can no longer race the claim on a slow-to-schedule runner. HyperUuid saw exactly
  that flake on linux-arm64. *(`hypercast` crate, tests only)*

### Upgrade note

Drop-in for every binding. Nothing is removed or renamed. The wasm backends are opt-in and
change nothing until asked for: no new runtime dependency in any package (Java's GraalWasm is
`compileOnly`, Ruby's wasmtime a Gemfile group for the suite, Python's an extra, Go's behind a
build tag — though wasmtime-go does now appear in `go.mod`, so it enters a consumer's module
graph without entering their binary). The Rust crate's source is unchanged; the only addition
under `rust/` is the `.cargo/config.toml` section that exports `malloc`/`free` on
`wasm32-wasip1`, which applies to builds run from that directory and to nothing a consumer
compiles.

## [0.2.0] — 2026-09-02

The theme is *stop paying for the carrier*. Every binding already made exactly one native
call per cast; what cost real time was what each language wrapped around it — a copied
input, heap scratch for a 16-byte out-value, an object built to be thrown away. Each of
those is measured before and after on one machine in one session, and the repository is
back in step with what HyperUuid's 0.1.1 → 0.2.1 taught about the release pipeline. No
door changed its verdict on any input: the corpus replays byte-identical through all
eight packages, as it did at 0.1.0.

### Added

- **`MemorySegment` overloads on every Java door.** Slice one buffer holding many values —
  a mapped file, a direct buffer, one line of a CSV — and cast a value out of it with
  nothing copied. This is the shape round three's chunk layer will hand down. *(Maven
  Central)*
- **`UnsafeRawBufferPointer` overloads on every Swift door** — the primitive the `String`
  and `[UInt8]` forms now wrap. *(`.package(url:)`)*
- **`Cast::uuidBytes` in PHP** — the sixteen RFC-ordered octets as a binary string for a
  `BINARY(16)` bind, skipping the hex encoding and hyphen assembly the string door does.
  *(Packagist)*
- **UTF-8 rows in the C# benchmark suite**, measuring the `ReadOnlySpan<byte>` doors
  without the UTF-16 transcode every `string` row includes: `Cast.Uuid` 36.9 ns against
  `Guid.TryParse`'s 51.2, which the string row had reported as a wash. *(docs only)*
- **The seventh Ruby gem**, `aarch64-mingw-ucrt`, with the Magnus extension for both
  ABIs — the forge now builds and tests it on win-arm64, so Windows-on-ARM leaves the
  Fiddle fallback like every other mainstream platform. Both Windows extensions build the
  `gnullvm` targets. CI's receipt only, so far: no Magnus-versus-Fiddle numbers have been
  taken on Windows-on-ARM hardware for this crate. *(RubyGems)*
- **Per-platform Native AOT receipts.** CI publishes `HyperCast.AotSmokeTest` under
  `PublishAot` on all six RIDs, fails on any trim diagnostic, runs the binary, and uploads
  each leg's log as `aot-report-{rid}`. *(CI only)*
- **A *Verifying provenance* section in every README** — which artifact, which signer,
  and why `--signer-repo SkunkWerkx/.github` is needed on some and not others. The C#
  README carries the full three-attestation story. *(docs only)*
- **The ext-php-rs benchmark spike behind a `php` cargo feature**, exactly as HyperUuid
  carries it: twenty functions at `Cast.php`'s raw layer, built and load-checked by CI on
  every darwin/linux leg, never loaded by the Composer package. It exists so "ext-ffi is
  already extension-class" stays checkable rather than asserted. *(`hypercast` crate,
  off by default)*

### Changed

- **Java: the input crosses without a copy.** Every downcall is linked
  `Linker.Option.critical(true)`, so the caller's `byte[]` is pinned and handed to the
  native side directly; the per-thread native staging buffer and its arena are gone.
  `reachability-metadata.json` registers the option and the GraalVM Native Image smoke
  test passes on it. The UUID door reads two big-endian longs instead of sixteen bytes.
  Full-length JMH: `timestamp` 62.9 → **52.5 ns**, `uuid` 53.8 → **38.3 ns** (now ahead of
  `UUID.fromString` at 45.9), `time` 66.4 → **45.1 ns**. *(Maven Central)*
- **Go: the verdict comes back by value.** The cgo shims declare the out-value, fault span
  and format on their own stack and return one struct, so no Go pointer crosses and
  nothing escapes — **0 B, 0 allocs on every door** (was 1–3), `Bool` 111 → **78 ns**,
  `I32` 172 → **87 ns**, `Timestamp` 174 → **112 ns**, `DateTime` 173 → **122 ns** (now at
  parity with `time.Parse`). The README's "a floor for this call shape" claim was a floor
  for the pointer-passing shape, not the ABI, and is corrected. `runtime.KeepAlive` pins
  the input explicitly rather than by FFI-library internals. purego is unchanged within
  noise. *(`go get`)*
- **Swift: zero mallocs per door.** `String` doors hand the string's own UTF-8 across via
  `withUTF8` instead of copying into an `Array`; out/fault/format scratch are stack tuples
  instead of three heap arrays; the library handle is a class reference instead of a
  21-field struct copied per call. 3–4 mallocs → **0** on every door: `timestamp` 281 →
  **55 ns**, `uuid` 222 → **37 ns**, `f64` 354 → **45 ns**, `i32` 349 → **33 ns**.
  *(`.package(url:)`)*
- **Python: `cast_uuid` builds `uuid.UUID` the way HyperUuid pinned** — `UUID.__new__` plus
  `object.__setattr__` of the `int` and `is_safe` slots, skipping an `__init__` whose
  validation the core had already done: **1.18 µs → 730 ns**, ahead of `uuid.UUID()`'s
  979 ns. *(PyPI)*
- **Ruby (Magnus): options and formats resolve by pointer compare.** The three reason
  Symbols and the option-symbol tables are cached; `DETECT` is identity-matched like
  `INVARIANT`; any other format is resolved once per thread and memoized by identity,
  anchored in a thread-variable so the key can never be a recycled address. The numeric
  doors under a declared eurozone format or `DETECT` drop from ~990 to **~580 ns**; the
  rest are within noise. *(RubyGems)*
- **Ruby (Fiddle): ~20% off the numeric doors** (`i32` 3.37 → 2.62 µs, `f64` 3.31 →
  2.76 µs) by not building per call what never changes — the integer doors' interpolated
  `:cast_*` Symbol, the splat-and-resplat dispatcher, and the 12-byte format copy into
  scratch (each format now owns one native pointer, memoized by identity). *(RubyGems)*
- **PHP: the eight integer doors are flat** — one literal FFI call each, no shared helper
  doing a dynamic symbol lookup and a string match per call — the rule the real doors
  already followed. Within noise on phpbench; recorded as structure, not speed.
  *(Packagist)*
- **C#: the UTF-16 doors try the stack buffer first** and rent from the pool only when the
  encoder says the text did not fit, instead of sizing by the 3-bytes-per-char worst case
  that sent any text past ~170 chars to the pool. *(NuGet)*
- **CI conforms to the forge's collapsed per-platform job**: the retired
  `php_native_spike` input is gone, `csharp_aot_project` is handed over, the Ruby/Python
  tool pins move to 4.0/3.14, and the C#/Java local dev-loop native staging yields whenever
  CI has placed the library explicitly (the forge builds the PyO3 extension into
  `rust/target/release/` before it tests C# and Java — `rust/README.md` records the trap).
  *(CI only)*
- **`release.yml` drops the `CARGO_REGISTRY_TOKEN` hand-off** — the crate publishes
  tokenless through Trusted Publishing, and is packaged and attested before the
  irreversible push. *(crates.io)*

### Upgrade note

Drop-in for every binding. Nothing is removed or renamed; every new door is an overload or
a sibling beside the existing surface, and the verdict types are untouched. Two things a
consumer may notice: the Java jar's downcalls are now `critical`, so a consumer's own
GraalVM Native Image build inherits that option through the bundled
`reachability-metadata.json` with no configuration (verified by the smoke test), and the
Go module's cgo backend no longer allocates on any door, which a `-benchmem` row in a
consumer's suite will show as 0 allocs where it showed 1–3.

### Notes

- **Not in this release, deliberately: a batch door.** `docs/roadmap.md` places it in round
  three as new exports beside the scalar ABI. A binding-level loop over N crossings would
  be a fake win, and every change above is the per-call shape that layer will inherit.
- **Two platform controls moved between toolchains and are reported as measured.** The
  Swift `DateFormatter` control read 810 ns on 0.1.0's tape and 28 µs on Swift 6.3.3; the
  JDK's `ISO_OFFSET_DATE_TIME` control was unstable in the full-length run and is not
  quoted. The doors' own before/after numbers are the claim in both cases.

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

[Unreleased]: https://github.com/SkunkWerkx/HyperCast/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/SkunkWerkx/HyperCast/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/SkunkWerkx/HyperCast/releases/tag/v0.1.0
