# Changelog

All eight packages in this repository — the `hypercast` crate and the C#, Java, Go, Python, Ruby,
PHP and Swift bindings — share one coordinated version, so one changelog covers all of them. Each
entry marks which packages it actually affects.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **A char door, in the core and every binding.** It reads exactly one character verbatim,
  checked before trimming, so `" "` is a space and `"6"` the digit six. Otherwise it reads
  one declared code point: decimal `65`, `U+0041`, `0x41`, `&H41`, or an HTML entity
  `&#65;` / `&#x41;` (the `;` is required), ASCII case-insensitive on the prefix. A
  surrogate or anything past `U+10FFFF` is `OutOfRange`. It is the twenty-sixth C ABI export,
  `cast_char`, writing a `u32` scalar, and `corpus/char.json` is replayed by every suite
  ([#22](https://github.com/SkunkWerkx/HyperCast/issues/22)). *(every package)*
  - Rust `cast_char` → `char`; C# `Cast.Char` → `char`; Java `Cast.character` → `Character`;
    Go `Char` → `rune`; Swift `Cast.char` → `Unicode.Scalar`; PHP `Cast::char`, Ruby
    `HyperCast.char` and Python `cast_char` → a one-character string.
  - C# and Java return a scalar past `U+FFFF` as `OutOfRange`, since their `char` is one
    UTF-16 unit. A string of exactly one UTF-16 unit, a lone surrogate included, comes back
    as it is without calling the core.
- **One door generic over every target.** It picks the door from the requested type, so a
  caller holding a generic `T` routes through HyperCast in one call
  ([#21](https://github.com/SkunkWerkx/HyperCast/issues/21)). *(csharp, go, swift)*
  - C#: `Cast.Scalar<T>` over `bool`, the numerics, `Guid`, `DateOnly`, `TimeOnly`,
    `DateTimeOffset`, `TimeSpan`, `char` and `DateTime`. `DateTime` reads RFC 3339 as
    `UtcDateTime`, not through the civil `Cast.DateTime` door. Any other `T` throws
    `NotSupportedException` before calling the core.
  - Go: `Scalar[V ScalarTarget, T Text]`. A type outside the union is a compile error.
  - Swift: `Cast.scalar<T: ScalarCastTarget>(_:format:)`.
- **C# — Android, for .NET MAUI.** The package carries the core for `android-arm64` and
  `android-x64`, so a MAUI app has it on every platform MAUI targets (Windows, macOS
  through Mac Catalyst, iOS and Android) with nothing but the package reference. On
  CoreCLR, .NET 11's Android runtime, the SDK puts
  `runtimes/android-{rid}/native/libhypercast.so` in the APK and the ordinary import opens
  it. The libraries are NDK-built for API level 21 and 16 KB-aligned, as Android 15 devices
  and Google Play require. A Native AOT publish links the core in from
  `staticlibs/android-{rid}/libhypercast.a` instead. CI builds an app both ways for both
  RIDs from a package packed in the same run, runs the x64 pair in a 16 KB-page emulator
  and checks the arm64 APKs' contents and alignment. *(csharp)*

### Fixed

- **Ruby — the Magnus extension survives a compacting garbage collection.** It kept
  `Success`, `Fault`, `Date`, `DateTime`, `Decimal`, `NumFormat::INVARIANT` and
  `NumFormat::DETECT` in a Rust static, which a compacting collection could move without
  updating, so a door called after one built its verdict from whatever object had taken
  their place — a segfault, reproduced with `GC.verify_compaction_references`. Each is
  pinned when the extension loads, and a new spec moves every movable object before
  calling a door. *(ruby)*

## [0.7.0] — 2026-10-06

Three themes, all driven by the libraries that build on HyperCast — HyperTabular and
HyperWorkbook — and one new platform family: C#, Swift and Go now link the core into iOS and
Mac Catalyst apps, as HyperUuid's 0.7.0 does. *Interop*: every binding now exposes the code
that turns the core's raw out-values, numeric format, reason codes and fault spans into its
own types, so a library that carries HyperCast's verdicts across a C ABI of its own reads
them exactly as the doors do; and the crate's C symbols become an `exports` feature such a
library can turn off. *Numbers already held*: four typed doors read the `f64` a workbook
stores rather than text — as an exact decimal, an Excel serial, a time of day and a span —
taking the C ABI to 25 `cast_*` exports plus `hypercast_version`, and Rust gains the whole
typed family. *Faster*: in Rust the temporal doors are about twice as fast, `cast_f64`
22-36% and `cast_uuid` about 20%, and the bindings that cross cheaply keep most of it. One
set of verdicts changes on purpose: a fraction of a second past nine digits now truncates to
the nanosecond instead of failing. The manifests read 0.6.2 between releases, but no 0.6.2
was ever published; everything since 0.6.1 is here.

### Added

- **Every binding — a public interop surface for libraries that carry HyperCast's verdicts
  across a C ABI of their own.** HyperTabular's bindings had to copy the private code that
  turns the core's raw out-values, numeric format, reason codes and fault spans into each
  language's types, and its platform table and library loader; now every binding exposes
  them, and the Cast doors use exactly the same code, so a value read out of another
  library's buffer is the value the door would have returned. *(every package)*
  - Rust: `UnixPrecision`, `DateOrder`, `ExcelEpoch` and `Reason` gain `const fn
    from_code`; `RawNumFormat` gains `From<NumFormat>`, `to_le_bytes` and `from_le_bytes`;
    and the `python-values` feature exposes `hypercast::python::Values`, the conversions to
    `datetime`, `decimal.Decimal` and `uuid.UUID` (the `python` feature builds on it).
  - C#: the `HyperCast.Interop` namespace — `RawTimestamp`, `RawDate`, `RawCivil`,
    `RawDuration`, `RawDecimal`, `RawFault` and `RawNumFormat` with their conversions, and
    `Abi` for checked codes both ways, faults, versions and the version probe;
    `NumFormat.ToRaw()` is public.
  - Java: the `io.github.skunkwerkx.hypercast.interop` package — `NativeValues` (every
    out-value reader, the format writer, faults, versions) and `NativePlatform`
    (parameterized by library name, with the jar loader); the enums' `code()` is public and
    each gains `fromCode`, an `Optional`, as `CastFailure.fromCode` now is.
  - Go: `RawTimestamp`, `RawDate`, `RawCivil` and `RawNumFormat` with their conversions,
    `NumFormat.Raw()` (the doors' check as an error), `Valid()` for each declared
    option, `*FromCode` for each option and the reason, `FaultFromCode`, `FormatVersion` and
    `CurrencyMaxBytes`; `Decimal` and `Duration` are documented and checked as the core's
    own layout. `NumFormat` now also refuses a separator that is no Unicode scalar value,
    as the core does, instead of panicking with a contract violation.
  - Swift: the `Interop` namespace — the out-value readers, `rawFormat`, `fault` and
    `version`.
  - PHP: `HyperCast\Interop\NativeValues` (the value builders, `writeFormat`, `fault`,
    `version`) and `HyperCast\Interop\NativePlatform` (parameterized by library name, with
    the library-path lookup), which replaces the `@internal` `HyperCast\NativePlatform`.
  - Ruby: `HyperCast::Interop` — `SCALARS` (each scalar door's unpack directive),
    `RECORDS` (each record door's directive, field count and builder), `VALUE_BYTES`,
    `decode`, `fault`, `characters`, `version` and `library_path`;
    `NativePlatform.rid_and_library_name` takes `library:`.
  - Python: `NumFormat.packed`, the 32 bytes a format crosses a C ABI as, and a `repr` that
    builds the format again.
- **Rust — `excel_serial`, the Excel-serial door for a number.** `cast_excel_serial` reads
  serial *text*; a workbook reader holds the `f64` the file stores and had to re-implement
  the rules to convert it. `excel_serial(serial, epoch)` is the same two date systems from
  the same code — the phantom serial `60`, a serial below the system's first day and one
  past `9999-12-31` are `OutOfRange`; a negative, NaN or infinite value is `Malformed` —
  returning the zone-less `CivilDateTime` the cell holds, its fraction snapped (below).
  `corpus/excel_serial.json` is replayed through both doors, so the two can no longer drift.
  *(crates.io)*
- **Rust — the typed doors: every numeric cast for a number already held as an `f64`.**
  A workbook stores a numeric cell as a double, and its reader had to convert that to an
  integer, a decimal, a time or a span with rules of its own. `i8_from_f64` … `u64_from_f64`,
  `f32_from_f64`, `bool_from_f64`, `decimal_from_f64`, `unix_from_f64`, `excel_time` and
  `excel_duration` are the twins of the text doors, each with a bare `Reason` verdict. A double
  is read as the one number it names, the shortest decimal that rounds back to it
  (`shortest_digits`), which is the digits Excel and LibreOffice write into the file: `2.5` is
  the decimal `2.5`, `0.1 + 0.2` is `0.30000000000000004`, an integer door takes a whole number
  and never rounds a fraction, and above 2⁵³ an integer is that decimal's digits. Every twin is
  held to its text door read on the double's shortest text, over 20,000 doubles.
  `excel_serial` and `excel_time` share one statement of how a serial's fraction is read.
  *(crates.io)*
- **Every binding — four doors that read a number instead of text.** The typed doors a
  workbook reader needs most cross the C ABI as four new exports (26 in all), each taking a
  `double` and the usual `out`/`fault`: `cast_decimal_from_f64`, `cast_excel_serial_from_f64`
  (with its epoch), `cast_excel_time` and `cast_excel_duration`. A typed door's fault span is
  always empty — there is no text for it to index. Named after Rust's, in each language's
  casing and float word: `Cast.DecimalFromDouble` / `ExcelSerialFromDouble` / `ExcelTime` /
  `ExcelDuration` (C#), `decimalFromDouble`… (Java, Swift), `ExactFromFloat64` /
  `ExcelSerialFromFloat64` / `ExcelTime` / `ExcelDuration` (Go), `cast_decimal_from_float`…
  (Python), `decimal_from_float`… (Ruby), `Cast::decimalFromFloat`… (PHP). Each presents the
  value as its binding's text twin does: the decimal as the decimal door's type, the serial
  as the binding's zone-less civil date-time (as `datetime`/`DateTime` returns it), the time
  of day and the duration as the time and duration doors do. `corpus/typed.json` names each
  double by its IEEE 754 bits — NaN and the infinities included, which JSON cannot spell —
  and every binding replays it, Ruby through both backends and Java through both FFM and
  GraalWasm. *(all packages)*
- **Every door that reads a serial held as a number snaps its time.** A double resolves
  about 0.6 µs at today's serials and 40 µs at `9999-12-31`, so the nearest nanosecond is
  the double's float noise: Excel's own `9999-12-31 23:59:59` would read 5,424 ns late, and
  the number `1234.56` as a time `13:26:23.999999995`. `excel_serial`, `excel_time` and
  `excel_duration` (and their exports) read the time with the fewest fractional-second
  digits — whole seconds, else tenths, down to the nanosecond — that the writer's
  conversion would have stored as the same double: the shortest round-trip,
  `decimal_from_f64`'s rule counted in time. Excel's and Google's times read back exactly,
  and a real sub-millisecond time a double can resolve is kept. Exact, integer-only,
  allocation- and panic-free; checked against an exact rational reference over 240,000
  serials. One writer loss it cannot undo: LibreOffice writes serials to 15 significant
  digits, so its `9999-12-31 23:59:59` is a different double, 370 µs late. *(all packages)*
- **Rust — `CivilDateTime::assume_utc` and `Timestamp::utc_civil`.** The two conversions
  between a wall clock and an instant, for the caller who states the zone is UTC. The
  crate still never assumes it for them. *(crates.io)*
- **Rust — `RawNumFormat` is public.** `NumFormat` as it crosses the C ABI (32 bytes: two
  code points, the flags, the currency symbol inline), with `resolve()` returning the
  `NumFormat` or `None` for a contract violation, so a crate exporting a C ABI of its own
  declares a numeric column in the same layout instead of a copy of it. *(crates.io)*
- **C# — iOS and Mac Catalyst.** A .NET iOS, MAUI or Mac Catalyst app (`net11.0-ios`,
  `net11.0-maccatalyst`) can reference the package and nothing else. Those platforms load
  no libraries, so the package now carries the core as a static library for `ios-arm64`,
  `iossimulator-arm64`, `maccatalyst-arm64` and `maccatalyst-x64`,
  `build/net11.0/HyperCast.targets` hands the one for the RID being built to the SDK as a
  static `NativeReference`, and `Cast` declares every entry point a third time against
  `__Internal`, the name a P/Invoke reaches the app's own executable by, picked by
  `OperatingSystem.IsIOS()`. One wiring covers CoreCLR (the .NET 11 default there), Mono's
  AOT compiler and interpreter, and Native AOT, because the native link is the SDK's in all
  of them. CI builds
  `HyperCast.AppleSmokeTest` on a Mac from that run's archives: run as a Mac Catalyst
  process, installed and launched in an iOS simulator, and linked for an iOS device.
  Android, tvOS and the iOS simulator on Intel Macs remain unsupported. *(NuGet)*
- **Swift — iOS and Mac Catalyst.** The package builds for iOS 16, the iOS simulator and
  Mac Catalyst 16 on arm64 (16 because the duration door returns Swift's `Duration`),
  linking the core from a second binary target, `swift/HyperCastCoreApple.xcframework`: an
  app for those platforms is built by Xcode, which links a static library out of an
  XCFramework and does not read the static-library artifact bundle the other platforms use.
  Both targets define the one `HyperCastCore` module, and the manifest declares the
  XCFramework only on a Mac, so Linux, Windows and WebAssembly builds see the package they
  saw before. CI runs the suite on an iOS simulator and as a Mac Catalyst process, and
  builds the package for an iOS device. *(SwiftPM)*
- **Go — iOS and Mac Catalyst.** A cgo build for an iOS device, the iOS simulator on Apple
  silicon, or Mac Catalyst on either architecture links the core from its own archive under
  `go/staticlib/`. Go builds all of them as `GOOS=ios`, so build tags choose: none for a
  device, `iossimulator` for the simulator (nothing sets it, so it is passed by hand), and
  `maccatalyst`, which `gomobile` sets for that target. CI holds every platform and tag
  combination to the archive it should select (`.github/scripts/check_go_archives.sh`),
  runs the suite, corpus included, in an iOS simulator through Go's own `go_ios_exec`
  wrapper, and links a device build. *(`go get`)*

### Changed

- **A fraction of a second past nine digits truncates to nanoseconds instead of failing.**
  ISO 8601, RFC 3339 (`time-secfrac = "." 1*DIGIT`) and XSD put no cap on fraction digits,
  and Excel's strict writer emits seventeen (`15:04:05.00000000000312325`), so a tenth
  digit was valid input refused as `Malformed`. Every door that reads a fraction —
  timestamp, time, local datetime, duration — now keeps the first nine digits, protobuf's
  `nanos`, and drops the rest. Truncated, never rounded: rounding could carry into the
  second, and `9999-12-31T23:59:59.9999999999Z` out of the window. *(every package)*
- **Every language is formatted, and CI holds it there — library, tests, benchmarks and
  smoke tests alike.** Rust (`cargo fmt`) and Go (now `gofmt`, beside revive) already were.
  New: ruff format and the docstring rules over all of `python/` (100 columns); PSR-12 via
  phpcs over php's src, tests and bench (the doc rules stay on the public API); RuboCop,
  layout cops only, over every Ruby file; `dotnet format whitespace` against
  `csharp/.editorconfig` (tabs); Spotless with palantir-java-format (4 spaces, 120 columns)
  over every Java source set; and `swift format` against `swift/.swift-format` (4 spaces,
  120 columns, lint rules off). Whitespace and line breaks only — no behavior changed.
  *(repository)*
- **Every door is proven unable to panic — `cast_f32` and `cast_f64` were the exception.**
  They handed their normalized text to `core`'s float parser, which keeps slice-index
  checks the optimizer cannot remove, so the two real doors were the only C ABI exports
  outside the no-panic proof. The conversion is now this crate's own (`float.rs`):
  Clinger's fast path and Eisel-Lemire, as `core` runs them, and an exact integer division
  on fixed stack arrays for what those cannot settle. Same bits as before for every input —
  held to `core`'s answer over about a million and a half generated strings, exact halfway points
  and 1,100-digit expansions among them, on every path — at the same speed (within 4%
  either way on five workloads, linux-x64) and 1.7 KB more library. All 26 exports are in
  the proof now, on every PR. *(every package)*
- **Rust — the C ABI symbols are an `exports` feature, on by default.** A `#[no_mangle]`
  item is exported from whatever library the crate ends up in, so a crate that linked
  `hypercast` as an rlib and built a shared or static library of its own carried all 26
  `cast_*`/`hypercast_version` symbols, and its static archive failed to link into one
  program with `libhypercast.a` (`multiple definition of cast_bool`). Nothing changes for
  a default build, or for any library this repository ships: `staticlib` and `cdylib`
  imply the feature. A consumer exporting its own C ABI takes `default-features = false`
  (naming `std` again if it wants it) and gets none of them; `hypercast_version()` stays a
  Rust function either way. A `default-features = false` build that relied on the symbols
  now has to name `exports`. *(crates.io)*
- **The temporal doors are about twice as fast.** Their small field readers — two digits,
  a date, a clock, a fraction, a digit run — were real calls even under fat LTO, each
  handing its result back through the stack, and the door read it straight back: a store
  the CPU could not forward, on every field. The five that decide the time are now always
  inlined, and the fraction reads eight digits at once. Padding a fraction to nine digits
  was a loop of `* 10` that LLVM had vectorized into a SIMD power computation; it is one
  multiplication by a table entry. A duration's total splits into seconds and nanos with
  a native 64-bit division whenever it fits one (under 292 years), and each ISO component
  scales with a native multiplication; `i128` division is a software routine, and was an
  eighth of the door. On linux-x64: `cast_timestamp` 26.6 → 14.5 ns (the `time` crate's
  RFC 3339 parser is 17.1), `cast_datetime` 29.3 → 13.9 ns for `1/7/2026 3:04 PM` and
  31.9 → 13.3 ns for ISO, `cast_date_ordered` 17.5 → 8.5 ns, `cast_duration` 35.5 → 23.6
  ns for ISO 8601 and 31.4 → 16.5 ns for the colon form. *(all packages)*
- **`cast_f64` and `cast_f32` are 22-36% faster.** A plain token was scanned twice, once
  to recognise the shape and once to convert it; the conversion already reads exactly
  that grammar and declines anything else, so under any format that allows the exponent
  it is now the shape check too. A new test holds the two grammars equal over every token
  of up to seven characters. A money-shaped token — grouped, with a currency symbol, in
  parentheses — had its digits copied one byte at a time into a buffer the conversion
  then read back eight bytes at a time; the lane now accumulates the significand itself
  and hands the conversion a number, and declines (to the full engine, as before) a
  literal of more than nineteen significant digits or a value Eisel-Lemire cannot round.
  Plain `12345.6789` 18.0 → 13.0 ns, `$12,345.67` 28.8 → 20.4 ns, separator detection on
  `1.234.567,89` 38.6 → 29.0 ns. *(all packages)*
- **`cast_uuid` is about 20% faster.** The D and N formats decode their 32 hex digits
  eight at a time with plain integer arithmetic (SWAR) instead of 32 table lookups, and
  build the 16 bytes in registers instead of 16 single-byte stores the caller read back
  as one: 15.9 → 12.8 ns, against 11.1 for the `uuid` crate. A bad digit is still
  reported at its exact byte — the pair-by-pair reader runs again to find it, only on
  input that has one — and a new test holds the two readers to the same verdict for every
  byte value at every position. B and P formats are D inside brackets and gain the same.
  *(all packages)*
- **The shared library is 10 KB larger** for the inlining above: 123,784 → 134,216 bytes
  on linux-x64. Each of the five readers is now copied into every door that uses it;
  inlining only the three most-called gave back half the datetime gain for 4.7 KB.
  *(all packages)*
- **What the bindings see.** Every harness ran twice in one session on the same linux-x64
  box, once against the previous core and once against this one, one harness at a time.
  How much of the core's gain survives depends on what the crossing costs. Swift links the
  core in and keeps most of it: duration 45 → 33 ns, timestamp 44 → 37, messy datetime 96 →
  82, eurozone f64 41 → 31, uuid 35 → 31. Java FFM: date-time and duration doors 23-28%
  faster (`Cast.duration` 46.9 → 33.8 ns, ISO `Cast.dateTime` 58.5 → 43.4), timestamp
  10-21%, the declared and detected f64 doors 14-15%. C#: `Cast.Duration` 49.1 → 31.6 ns,
  declared-order `Cast.Date` 38.0 → 27.3, messy `Cast.DateTime` 49.9 → 37.9, the UTF-8
  `Cast.Double` 31.2 → 25.2. Go pays ~45 ns of cgo a call and still gains 11-21% on
  duration, the civil doors and separator detection. Python (PyO3) gains 5-13% on
  timestamp, duration and the eurozone f64 doors. On Ruby (Magnus, 100-900 ns a call) and
  PHP (ext-ffi, 250-800 ns) the change is within the run's noise, except PHP's duration
  door, 414 → 344 ns. The Rust, C#, Java, Go, Ruby and Swift tables are re-measured from
  these runs. The Python and PHP tables are not, because both moved for reasons that are
  not the core and are not yet explained. Under PHP 8.5.10 every ext-ffi door ran 20-30%
  above its recorded figure under the old core and the new one alike, while PHP's own
  functions matched theirs. On a pyenv CPython 3.14.8 rather than the Fedora 3.14.7 the
  table was recorded on, some stdlib rows moved by up to a third (`int()` 56 → 70 ns,
  `uuid.UUID()` 798 ns → 1.06 µs) and others not at all (`fromisoformat`). A new table
  would credit the core with either. *(repository)*

- **Go — Android is a compile error instead of a Linux build.** `GOOS=android` satisfies
  Go's `linux` constraint, so an Android cgo build linked the Linux archive, which nothing
  had ever tested there. It now lands on the same `undefined:
  hypercast_needs_cgo_and_a_C_compiler_…` stop as every other platform without an archive
  of its own, as does `GOOS=ios` on amd64 outside Mac Catalyst, the simulator on an Intel
  Mac. *(`go get`)*

### Fixed

- **`prepare-release.yml` left `rust/browser-test/Cargo.lock` out of its commit.** It
  regenerated the lockfile's own entry for the new version and then staged every manifest
  but that one. *(repository)*
- **Rust — the README's consumer no-panic recipe could not fail for a `cdylib`.** no-panic
  reports a surviving panic path as an undefined symbol, which a Linux shared library is
  allowed to have, so a `cdylib` built from the recipe linked cleanly even against 0.6.0,
  whose `cast_uuid` fails the proof. The recipe now adds `-C link-arg=-Wl,--no-undefined`,
  as this repository's own `cargo no-panic` always has. Documentation only. *(crates.io)*

### Upgrade note

Source-compatible for almost every consumer, with five things to know; the first is the
only one most will meet.

Every package: a fraction of a second longer than nine digits (`15:04:05.0000000001`) is
now a success, truncated to the nanosecond, where it was `Malformed` (Changed, first
entry). Code that leaned on that refusal to reject over-precise input has to check the text
itself; code that only reads verdicts needs nothing.

Rust: a build with `default-features = false` no longer carries the C ABI symbols. A crate
that relied on them — one that links `hypercast` into a library of its own and expected
`cast_*` to be exported from it — names `exports` beside `std`
(`features = ["std", "exports"]`). Default builds, and every library this repository ships,
are unchanged.

PHP: `HyperCast\NativePlatform` is now `HyperCast\Interop\NativePlatform`, and its
`ridAndLibraryName`, `resolve` and `libraryPath` take the library's base name first
(`'hypercast'` for this one). The old class was `@internal`, but PHP cannot enforce that;
code that called it needs the new name and the argument.

Java: `CastFailure.fromCode` is public and returns `Optional<CastFailure>`, empty for any
code that is not a failure, where it threw `IllegalStateException`. It was package-private,
so only code compiled into the `io.github.skunkwerkx.hypercast` package itself can see the
change.

Go: a cgo build for Android (`GOOS=android`) no longer compiles. It used to link the Linux
archive, untested; a module that needs it there should say so in an issue rather than rely
on that accident.

## [0.6.1] — 2026-10-03

### Added

- **Ruby — in the browser, through ruby.wasm: the `hypercast-wasm` gem.** ruby.wasm links
  extensions into the interpreter when `rbwasm build` makes it, so a browser app lists
  `hypercast-wasm` instead of `hypercast` in that Gemfile and gets the same Magnus extension,
  prebuilt for `wasm32-wasip1` for Ruby 3.4 and 4.0, with no Rust toolchain on the consumer's
  machine. It links into the same interpreter as HyperUuid's `hyperuuid-wasm`. CI builds each
  minor's archive from the commit, attests it, and runs the gem packed around it under Node and
  in headless Chrome; the release packs the published gem from those archives. Unblocked by
  Magnus 0.8.3/0.9.2, which fixed the two bugs that kept it off WASI
  ([magnus#186](https://github.com/matsadler/magnus/issues/186),
  [#187](https://github.com/matsadler/magnus/issues/187)). *(RubyGems)*

### Changed

- **Ruby — Magnus 0.9.** The extension builds on Magnus 0.9.2 (from 0.8.2), with no change to
  the gem's API. *(RubyGems)*

### Fixed

- **Ruby — the Magnus extension compiles on 32-bit targets.** It cached Ruby objects' raw
  `VALUE`s as `u64` to resolve the declared-option Symbols and memoize the last format, which
  only type-checks where `VALUE` is 64 bits. They are `rb_sys::VALUE` now, so the extension
  builds for `wasm32-wasip1` (the `hypercast-wasm` gem) and any other 32-bit target; nothing
  changes on the platforms the platform gems cover. *(RubyGems)*
- **Rust — the no-panic proof from a consumer's crate fails at `cast_uuid` on the recipe the
  README gives.** 0.6.0 said `lto = true` in your release profile is enough, and it is for
  every other export, but on stable that build still flagged `cast_uuid`, and on Rust 1.88
  (the crate's floor) so did this repository's own `cargo no-panic`. The UUID parser
  indexed its input after checking the length, and whether that bounds check vanished was the
  optimizer's call. It now reads every byte with `get`, so there is no panic path for any
  compiler to keep: the proof passes from a consumer with `lto = true` alone and in this
  repository, on both stable and 1.88. Verdicts and speed are unchanged. *(crates.io)*
- **C# — a .NET 10 project is refused at restore instead of failing to compile.** The
  package targets .NET 11 only (the binding is built on C#'s native union types), but its
  targets file and native libraries sat in no target-framework folder, so NuGet restored it
  into a net10.0 project without a warning and the build then stopped on missing `HyperCast`
  types. The targets file now ships under `build/net11.0/` and `buildTransitive/net11.0/`,
  and a net10.0 project gets NuGet's own NU1202 ("not compatible with net10.0"). Nothing
  changes for .NET 11: Native AOT still links the core in, and Blazor WebAssembly, direct or
  through a class library, still gets the wiring. *(NuGet)*
- **Go — printing a nil `*Fault` no longer traps under TinyGo.** A door's success is a nil
  `*Fault`, and printing one called `Error` on nil. Stock Go's `fmt` recovers that and
  prints `<nil>`; TinyGo's wasm targets cannot recover, so the program trapped. `Error` now
  returns `<nil>` for a nil receiver itself. *(Go module)*

## [0.6.0] — 2026-10-02

Four themes, shared with HyperUuid's 0.6.0; there was no HyperCast 0.5.0, so this release
also carries the work that went into HyperUuid's 0.5.0. *One way in*: every binding reaches the
core one way per platform. Go links it statically through cgo on Linux, macOS and Windows
and nothing else — purego, the loading cgo backend, the wasmtime backend and the nine
embedded files under `go/native/` are gone — and Swift now links it on macOS and Windows as
well as Linux and WebAssembly, so it has no loader, nothing to deploy and no door that can
throw. Ruby gets Magnus platform gems for Alpine and keeps Fiddle only in the universal gem,
as the last resort; Python and Ruby drop the wasmtime backend that reached no platform their
native backends did not. *In the browser*: C#, Rust, Python (a Pyodide wheel on PyPI), Go
(through TinyGo) and Swift (through a WASI shim) all run HyperCast in a web page, and CI runs
each in headless Chrome on every PR; PHP is proven and documented but not shipped.
*Smaller*: the native libraries are built without Rust's standard library and stripped —
the linux-x64 library 0.4.0 shipped at 529,960 bytes is 104,496 in a local build — the Ruby
platform gems carry only their own platform's extensions, Go's module and the Composer package lose their embedded
libraries, and a Blazor app no longer carries a 3 MB archive it never reads. *Proven*: every
door but the two float ones is proved unable to panic on every PR, and the crate declares
its minimum Rust (1.88) and gains clippy, semver, round-trip and browser checks. One set of
verdicts changes on purpose: a date or time shaped right but naming a moment that does not
exist (month 13, February 30th, hour 24, a `:60` second) is now `OutOfRange` at that field
instead of `Malformed`.

### Added

- **Go — Windows links the core in, like Linux and macOS.** A cgo build on Windows now names
  `go/staticlib/windows_amd64` (or `windows_arm64`) on its link line: the same MSVC archive
  the C# package links under Native AOT. MinGW's linker reads MSVC's objects, and the core
  needs nothing from Windows beyond the C runtime, which resolves against msvcrt, so the link
  line names nothing else and there is one Windows archive per architecture for both
  bindings. *(`go get`)*
- **Go — in the browser, through TinyGo.** [TinyGo](https://tinygo.org) 0.42+ compiles the
  module to WebAssembly with the core linked in, the way Blazor links it for C#:
  `tinygo build -target=wasm` picks up `backend_tinygo.go`, which names
  `go/staticlib/wasm/libhypercast.a` — the `wasm32-wasip1` archive, the same bytes as Swift's —
  on its link line. The core imports nothing (no clock, no randomness, no I/O), so the page
  loads one module. `-target=wasip1` works the same way under a WASI runtime. CI builds
  `go/internal/tinygosmoke` this way on every PR, with that run's archive, and runs 17
  checks in headless Chrome — every ABI shape (plain, numeric with a format and a currency
  symbol, discriminated), a fault with its span, and the core's version against
  `rust/Cargo.toml` — failing on any `FAIL` or a missing `DONE`. Stock Go on
  `GOOS=js`/`wasip1` is still a compile error, whose name now points at TinyGo. *(`go get`)*
- **Python — in the browser, under Pyodide.** A ninth wheel,
  `cp311-abi3-pyemscripten_2026_0_wasm32` (~150 KB), is the same PyO3 extension built for
  Pyodide 314.x's Emscripten target, so `await micropip.install("hypercast")` finds it on
  PyPI the way pip finds the native wheels — the native backend, no JavaScript bridge. CI
  runs the package's whole pytest suite, corpus replay included, inside Pyodide on every
  run, under Node and in headless Chrome, before the wheel is attested and uploaded.
  *(PyPI)*
- **Swift — in the browser, through a WASI shim.** What swift.org's WebAssembly SDK builds
  from the binding is a plain `wasm32-wasip1` command module with the core linked in, so a
  page runs it with [`@bjorn3/browser_wasi_shim`](https://github.com/bjorn3/browser_wasi_shim)
  — no JavaScriptKit, no change to the package. CI builds the smoke executable for
  WebAssembly with that run's archive and runs it in headless Chrome on every PR, failing
  unless it exits 0. *(`.package(url:)`)*
- **C# and Rust in the browser, run in CI on every PR.** The Blazor WebAssembly smoke app,
  which links the `wasm32-unknown-emscripten` archive through the package's own targets file
  and calls all 22 native entry points, is published and loaded in headless Chrome against
  the archive that run built (`check.sh` takes it as `STATICLIB`); through 0.4.0 it was a
  local check. The crate gets the same for `wasm32-unknown-unknown`: `rust/browser-test`
  depends on it the way a browser consumer does — with nothing to switch on, since the core
  reads no clock and no entropy — and `wasm-pack test --headless --chrome` runs five tests
  over every door family and the version export in a real browser. *(repository only)*
- **Rust — `rust-version = "1.88"`.** The lowest toolchain the crate builds on, now declared
  and measured: the parsers use `let` chains (stable since 1.88) and `is_multiple_of`, above
  edition 2024's own 1.85 floor. An older rustc says so by name instead of failing somewhere
  in the compile, and Cargo's resolver picks dependency versions that build on it. CI checks
  the library and the `no_std` shared library on exactly that version. *(crates.io)*
- **Rust — the doors are checked against a reference, not just for panics.**
  `tests/round_trip.rs`: values format to text and cast back to themselves, and every
  one-character edit of a valid string (replaced, removed or inserted, multi-byte UTF-8
  included) casts exactly when a deliberately naive reference parser says it should, to the
  same value — about 34,000 strings across the UUID, boolean, integer (all eight widths) and
  strict ISO date doors — and sampled floats and decimals survive their own canonical
  formatting through `cast_f32`/`cast_f64`/`cast_decimal` to the same value. The doors whose
  grammar *is* the lenience stay pinned by the shared corpus. *(repository only)*
- **Rust — clippy, semver and MSRV checks, and a ctypes check of the shared library, on
  every PR.** `lint-rust` runs clippy with warnings as errors over each configuration that
  compiles different code (default with tests and benches, the bare-metal `no_std` rlib, the
  `no_std` shared library, the Python extension, the fuzz target and the browser tests),
  beside `cargo fmt --check` over all three crates. `check-semver` runs `cargo-semver-checks`
  against the latest crates.io release; `check-msrv` builds on the declared `rust-version`. `check-cdylib` builds the `no_std` shared
  library on all seven CI platforms, holds its exports to the 22 C ABI symbols (and the musl
  builds to musl's libc as their one dependency), then loads it through Python's `ctypes` and
  calls every export with an accepted and a rejected input
  (`.github/scripts/cdylib_smoke.py`). *(repository only)*

### Removed

- **Go — every backend but the linked one.** The purego backend (`CGO_ENABLED=0`, and all of
  Windows until now), the loading cgo backend (`-tags hypercast_dynamic`) and the wasmtime
  backend (`-tags hypercast_wasm`) are gone, and with them `go/native/`: the eight shared
  libraries and the wasm module every non-linked build embedded, the copy each process wrote
  to a temp directory, and the libc detection that picked one. None of them reached a
  platform the linked build does not — wasmtime-go needs cgo itself and ships engines only
  for the same platforms — and stock Go compiled to WebAssembly never worked: its toolchain
  has no cgo and no external linker. (TinyGo's does; see Added.) `go.mod` drops purego and
  wasmtime-go, and `go/` (which the Composer archive carries too, for the Go proxy) is about a
  quarter of its 0.4.0 size. *(`go get`, Packagist)*
- **Python and Ruby — the in-process wasm backend.** `hypercast._wasm` and the `[wasm]` extra,
  `lib/hypercast/wasm_runtime.rb` and the Gemfile's `wasmtime` group, the `HYPERCAST_WASM`
  variable that forced either, and the `wasm32-wasip1` module inside every wheel and every gem
  are gone. Neither reached a platform the native backends do not: only platform wheels are
  published, each carrying the PyO3 extension, and no sdist, so a Python with no matching
  wheel had nothing to fall back from; and the Ruby backend's only extra reach was a platform
  with no Fiddle library, where the `wasmtime` gem itself has to be built from source with a
  Rust toolchain. `hypercast.BACKEND` is now always `"native"`, and `HyperCast::BACKEND` is
  `:native` or `:fiddle`. Java's GraalWasm backend stays, since it is plain Java and reaches
  every JVM platform, and the jar keeps the module. *(PyPI, RubyGems)*

### Changed

- **A date or time that is well-formed but impossible is `OutOfRange`, at the field that is
  wrong.** `Malformed` now means only the wrong shape: a missing separator, a stray
  character, three digits where two belong. Text with the right shape that names something
  that does not exist is `OutOfRange`, with the fault span on that field's digits. That
  covers year 0000, month 00 or 13 and up, a day the month does not have (`2026-02-29`), hour
  24 and up (outside 1–12 with `AM`/`PM`), minute or second 60 and up (leap seconds
  included), and a timestamp offset past `23:59`. All of these were `Malformed` before,
  except year 0000, which was already `OutOfRange` but spanned the whole date and now spans
  just the year. Every door that reads them moves together: `cast_date`,
  `cast_date_ordered`, `cast_datetime`, `cast_time`, `cast_timestamp`, `cast_duration`'s
  colon form (`25:00:00` — .NET's `TimeSpan` throws `OverflowException` there) and
  `cast_excel_serial`'s 1900 phantom serial `60`, which keeps agreeing with the text
  `1900-02-29`. Shape is checked across the whole input before any value, so
  `2026-13-01T00:00:00x` is `Malformed` at the `x`, not out of range at the month. The
  corpus carries every case with its span. *(every package)*
- **Every shipped library is stripped.** The release profile now drops the symbol table and
  debug info from what it links, never the exports, and leaves the machine code
  byte-identical. Measured on local builds, the linux-x64 shared library goes 110,048 →
  104,496 bytes, the wasm32-wasip1 module 137,432 → 126,724 and the Linux Magnus extension
  551,736 → 457,400 (a platform gem carries two). Windows DLLs keep their symbols in a PDB,
  not the DLL. The static libraries keep their symbols, since a consumer's linker resolves
  the core through them, and `cargo bench` keeps its own for profilers. *(every package that
  carries a native library)*
- **Ruby — Magnus platform gems for Alpine, glibc gems named for their libc, and Fiddle only
  in the universal gem.** Alpine gets platform gems of its own, `x86_64-linux-musl` and
  `aarch64-linux-musl`, whose extensions are built and tested inside each Ruby's
  `ruby:*-alpine` image; through 0.4.0 Alpine ran on Fiddle. The glibc gems are now
  `x86_64-linux-gnu` and `aarch64-linux-gnu` (0.4.0's `x86_64-linux` and `aarch64-linux`):
  beside a plain `*-linux` gem, `gem install` on RubyGems before 4.0 resolves that one on
  Alpine, even under `--platform x86_64-linux-musl`, while naming the libc on both sides
  makes every supported RubyGems and Bundler pick right — Nokogiri's scheme, for the same
  reason. With `arm64-darwin`, `x64-mingw-ucrt` and `aarch64-mingw-ucrt` that is seven
  platform gems, each carrying its Ruby 3.4 and 4.0 extensions and no Fiddle library at all,
  and no `fiddle` dependency either (Fiddle is autoloaded, so a platform gem never loads it);
  through 0.4.0 every platform gem also carried every RID's library and the wasm module,
  though one only installs where its own platform matches. Fiddle is now the last resort, in
  the universal gem alone, which still bundles all eight libraries and depends on `fiddle`:
  Ruby 3.3, a Ruby newer than the release, Intel macOS (which has no platform gem and no CI
  leg), or a platform with no build. `release.yml` checks every gem's contents before the
  push. *(RubyGems)*
- **Swift — the core is linked in on macOS and Windows too, so there is nothing to deploy
  and nothing to throw.** The SwiftPM binary target that already linked a static core on
  Linux and WebAssembly now carries macOS (`arm64-apple-macosx`, `x86_64-apple-macosx`) and
  Windows (`x86_64-unknown-windows-msvc`, `aarch64-unknown-windows-msvc`, as
  `hypercast.lib`) archives as well — nine in all, the same archives the C# package links
  under Native AOT and Go links through cgo — and every platform depends on it
  unconditionally. The shared libraries under `Sources/HyperCast/NativeLibs` (1.4 MB at
  0.4.0, carried by every build, Linux and WebAssembly included) and the
  `dlopen`/`LoadLibraryW` loader are gone: an executable copied on its own works on every
  platform. No door throws any more — the only thing one ever threw was the load failure —
  but each keeps its `throws`, so existing `try` call sites compile unchanged. `isAvailable`
  is always `true`. `NativeLibraryError` is deprecated and has no cases; a platform with no
  prebuilt core (iOS, Android, …) now fails to compile, with no `HyperCastCore` module,
  instead of at run time. On Swift 6.3 with the opt-in `--build-system swiftbuild`, a
  dependent package fails with `missing required module 'HyperCastCore'`
  ([swift-build#1295](https://github.com/swiftlang/swift-build/pull/1295), fixed in 6.4);
  every release's default build system is unaffected. *(`.package(url:)`)*
- **Go — `NativeVersion` can no longer panic.** It still returns the core's own
  `"major.minor.patch"`, now read from the linked core, which is always there; `Available` is
  always `true`, `LoadError` always `nil`, and `ErrNativeUnavailable`, which nothing returns
  or panics with any more, is deprecated. *(`go get`)*
- **PHP — the `ext-php-rs` extension spike is on ext-php-rs 0.16, and proven in the browser.**
  The extension builds and loads on PHP 8.5 as before. Built as a side module (258 KB, 85 KB
  gzipped), the same extension runs in WordPress Playground's prebuilt PHP for the browser
  (`@php-wasm/web`, PHP 8.5, JSPI), checked in node and headless Chromium across the version
  probe, the boolean, integer, real, UUID, date and duration doors, a fault span and the
  contract-violation exception. It is not shipped — a module per PHP minor and a large build
  image in CI, waiting on a request — and `php/README.md` carries the whole recipe, including
  the two upstream issues HyperUuid found on the same route
  ([ext-php-rs#800](https://github.com/extphprs/ext-php-rs/issues/800),
  [wordpress-playground#4377](https://github.com/WordPress/wordpress-playground/issues/4377)).
  *(repository only)*
- **Rust — every door except `cast_f32`/`cast_f64` is proved unable to panic, and CI
  re-proves it on every PR.** The doors always promised never to panic on bad input; at the
  C ABI a panic is an abort of the host process, so that promise is what keeps untrusted
  text from crashing the caller. It held because the optimizer happened to remove the
  bounds checks, not by construction, and 18 of the 22 exports could still reach one. The
  indexing is now `get`, slice patterns and `strip_prefix`/`strip_suffix`, the Excel serial
  fraction divides by a `NonZero`, and `cargo no-panic` (dtolnay's `no-panic`, behind a
  feature of the same name) fails the build if any export can still reach a panic. The two
  float doors are exempt: their remaining panic paths are inside `core`'s float parser.
  Verdicts are unchanged (every test, the corpus and the lane-vs-engine differential test
  pass), and interleaved benchmark runs put every door within noise or faster — grouped and
  currency integers in the lenient lane 18–23% faster. *(crates.io, and every package that
  carries a native library)*

- **Rust — the shared library is built by naming its crate type, not listed in the
  manifest.** `[lib]` now declares only the rlib, and the library every binding loads is
  built with `cargo cdylib` (an alias for `cargo rustc --release --crate-type cdylib`), the
  way the static libraries already were. In this repository `cargo cdylib` replaces
  `cargo build --release` in every dev loop, and `cargo wasm-module` builds the
  wasm32-wasip1 module; a plain `cargo build` now produces the rlib and no shared library.
  The fix below is the reason. *(crates.io, and every dev loop)*
- **The native libraries no longer carry Rust's standard library, and are a quarter the
  size.** `cargo cdylib` now builds the shared library every binding loads `#![no_std]`,
  with the same abort-on-panic handler the static libraries already had. What std added was
  its runtime — the unwinder, the backtrace symbolizer and the allocator — which no C ABI
  export can reach: linux-x64 goes from 436,504 bytes to 110,048, and imports nothing but
  the C library's `abort`, `memcpy`, `memset` and `bcmp`, so the musl builds no longer
  depend on libgcc_s. The exports are the same 22 symbols over the same code; a panic was
  already an abort of the host at the C ABI and still is, without the message printed
  first. The float doors' parse stays `core`'s own — making it panic-free would not have
  shrunk anything, since a std library carries the runtime whether or not a panic can
  reach it. The wasm32-wasip1 module keeps std, whose allocator its hosts call into, and
  the Python, Ruby and PHP extensions keep std and unwinding, so a panic in one still
  surfaces as a host exception. *(every package that carries a native library)*
- **The native libraries are smaller.** Cargo only passes `-C lto` for a cdylib built as an
  invocation's one crate type, so `lto = true` never reached the plain library while the
  manifest listed `["cdylib", "rlib"]`. The linux-x64 library is 457,008 bytes against
  535,440, at the same speed (the timestamp, uuid and bool doors through the C ABI, alternating runs, within noise).
  *(every package that carries a native library)*
- **Python — the wheels are built, installed and attested in CI, and the release publishes
  them unchanged.** Through 0.4.0 they were built in `release.yml` at the tag, so the first
  time a wheel was ever installed was after other registries had published; 0.4.0's osx-x64
  wheel failed there. All nine (the Pyodide one is new; see Added) are now built on every CI run by the forge's
  `hyper-build-wheels.yml`, installed on their own platform and called into, and
  `release.yml` verifies their count, version and provenance before uploading them. Because
  the forge signs them, `gh attestation verify` on a wheel now takes
  `--signer-repo SkunkWerkx/.github` (or `--owner SkunkWerkx`); wheels up to 0.4.0 verify
  with `--repo` alone. *(PyPI)*
- **PHP — the Composer package no longer carries the other bindings.** A `.gitattributes`
  `export-ignore` list keeps the C#, Java, Python, Ruby, Rust and Swift trees, the corpus,
  the docs, the workflows and PHP's own tests out of the archive Packagist serves: 3.5 MB against
  5.0 as downloaded, 8.3 MB against 12.1 unpacked. `go/` stays in, because
  the Go module proxy builds its zip from the same kind of archive. *(Packagist)*

### Fixed

- **C# — a Blazor WebAssembly app no longer gets the 3 MB wasm archive in its output.**
  Restore resolves `runtimes/browser-wasm/nativeassets/` as a copy-local native asset, so
  through 0.4.0 `libhypercast.a` was copied into `bin/` and the publish root of every
  browser-wasm consumer, outside `wwwroot`, never served and never read. The package's
  targets take it back out of the copy-local list; the link, which names the archive by
  path, is unchanged. *(NuGet)*
- **Rust — `default-features = false` builds on every target, not only bare metal.** Cargo
  builds every crate type a dependency lists, and a no_std cdylib has no panic handler, so
  through 0.4.0 a `default-features = false` consumer failed with "`#[panic_handler]`
  function required, but not found" on any target that can produce a cdylib: the
  developer's own machine and `wasm32-unknown-unknown`. Bare-metal targets drop the crate
  type, which is why the `thumbv7em` check in CI never saw it. CI now builds a real
  `default-features = false` consumer on the host and for `wasm32-unknown-unknown`.
  *(crates.io)*

### Upgrade note

Go has the breaking change, and it is the reason this is a minor release. Four smaller ones
follow it: Swift drops the `NativeLibraryError` cases (second paragraph), Ruby renames its
glibc platform gems (third), Python and Ruby lose an opt-in backend (fourth), and impossible
dates and times change their verdict in every package (last). The Go module
now builds only under cgo, on Linux, macOS and Windows on amd64 and arm64, which takes a C
compiler where it is built: gcc or clang on Linux (`build-base` on Alpine), the Xcode
command-line tools on macOS, MinGW-w64 gcc on Windows (llvm-mingw on arm64). A build with
`CGO_ENABLED=0`, for stock Go's `GOOS=wasip1` or `js`, or for any other platform stops at
compile time on `undefined: hypercast_needs_cgo_and_a_C_compiler_…`, which names the fix;
drop `-tags hypercast_dynamic` and `-tags hypercast_wasm`, which no longer select anything.
Cross-compiling needs a C cross-compiler, e.g. `CC=x86_64-w64-mingw32-gcc GOOS=windows
CGO_ENABLED=1`. The API is unchanged: `Available` is always `true`, `LoadError` always `nil`,
`NativeVersion` never panics, and `ErrNativeUnavailable` is deprecated. Stock Go compiled to
WebAssembly cannot use this module; build with TinyGo 0.42+ instead, which links the core
there, browser included (go/README's "In the browser (TinyGo)").

Swift on macOS and Windows has nothing to deploy beside the executable any more: delete the
step that copied `HyperCast_HyperCast.bundle` (or `.resources`) from deployment scripts and
Dockerfiles. Code that matches `NativeLibraryError` cases (`.openFailed`, `.symbolNotFound`)
no longer compiles — those cases are gone with the loader; a plain
`catch let error as NativeLibraryError` still compiles, with a deprecation warning, and can be
deleted along with any `do`/`catch` that existed only for it — no door throws. A Swift build
for a platform with no prebuilt core now fails at compile time instead of throwing at run
time. Nothing changes on Linux and WebAssembly.

Ruby on Alpine moves from `BACKEND == :fiddle` to `:native` on Ruby 3.4 and 4.0 with no
action, through the new musl platform gems; Intel Macs stay on Fiddle. The glibc platform
gems are renamed `x86_64-linux-gnu` and `aarch64-linux-gnu`: RubyGems and Bundler resolve
them by themselves, but anything that names the old `x86_64-linux`/`aarch64-linux` platform
string explicitly — a `gem install --platform`, a pinned gem file name — needs the new one. A
platform gem has no Fiddle library any more, so `HYPERCAST_PURE` inside one makes the first
call raise a `LoadError` that names the universal gem (`gem install hypercast --platform
ruby`, or Bundler's `force_ruby_platform`) instead of running on Fiddle.

Python and Ruby users who set `HYPERCAST_WASM` should unset it: both packages now ignore it
and load their native backend as though it were not set. `pip install hypercast[wasm]` still
installs, with pip's warning that the package has no `wasm` extra; drop the extra.

Every package: code that tells `Malformed` from `OutOfRange` on a date, time, timestamp,
colon-form duration or Excel serial will see `OutOfRange` where it saw `Malformed` for a
well-formed but impossible value (Changed, first entry). Code that only checks for success,
or that reports the fault span, needs nothing: the span still points at the same digits,
and for year 0000 it now points at the year instead of the whole date.

## [0.4.0] — 2026-10-01

Six themes, shared with HyperUuid's release of the same day. *The libraries report the
right version*: 0.3.0's native libraries say 0.2.0, and the release order that caused it is
fixed. *musl*: `linux-musl-x64` and `linux-musl-arm64` are built, attested and shipped.
*Only supported runtimes*: every floor that had reached end of life is raised, and the
floors are now tested. *The core links in*: Swift on Linux, which is what adds musl and
WebAssembly to that binding, and Go's cgo build and C# Native AOT, which now carry no shared
library to extract or load. *Faster*: a second fast lane reads grouped, currency and
accounting input in one pass; Java's FFM doors are compiled rather than interpreted in a
GraalVM Native Image; and Python and Ruby doors cost a half to a quarter of what they did.
*Measured again*: every benchmark table is re-measured on x86-64, and several verdicts
changed with the machine. No door changed its verdict on any input.

### Added

- **Swift — musl Linux and WebAssembly.** The binding builds with Swift's static Linux SDK
  (`--swift-sdk x86_64-swift-linux-musl`, and arm64) and with its WebAssembly SDK
  (`wasm32-unknown-wasip1`). Neither target can open a shared library, so the core is
  linked in: `swift/HyperCastCore.artifactbundle` carries one no_std static library per
  triple, about 140 KB each, built by `cargo staticlib` and attested like every other native
  binary. CI runs the suite under WasmKit and a smoke executable (`swift/StaticSmokeTest`)
  through the musl SDK, on Swift 6.4 and on the 6.2 floor. Through 0.3.0 a musl build
  stopped at a compile error. *(`.package(url:)`)*
- **musl (Alpine): `linux-musl-x64` and `linux-musl-arm64`.** Built inside an Alpine
  container with the unwinder linked statically, so the library depends on musl's libc and
  nothing else and loads on a bare `alpine`, `python:alpine` or `golang:alpine` image. In
  the NuGet package, the jar, the gems, `go/native/` and `php/src/native/`, and as
  `musllinux_1_2` wheels. Each binding resolves it for a process that has a musl loader
  mapped; Ruby runs on its Fiddle backend there. Through 0.3.0 Alpine got the glibc library,
  which does not load under musl — the gap that made the first consumer carry a managed
  fallback (`docs/roadmap.md`). Swift has no dynamic loader on musl and reaches it
  by linking the core in instead (above). *(every package)*
- **Go — `LoadError()` and `ErrNativeUnavailable`.** `Available()` said that the core had
  not loaded; `LoadError()` says why, without a panic, and the doors now panic with that
  same error so a `recover` can `errors.Is` it. `Example*` tests for pkg.go.dev, and the
  zero-allocation claim held by `testing.AllocsPerRun` tests on the success path.
  *(`go get`)*
- **Swift — `NativeLibraryError`, a load failure a caller can match.** The error a door
  throws when the bundled library cannot be found, opened or resolved was an internal type;
  it is public now and `LocalizedError`. *(`.package(url:)`)*
- **Java — `Automatic-Module-Name: io.github.skunkwerkx.hypercast`**, and a README section
  on `--enable-native-access`. *(Maven Central)*
- **Python — the package is typed.** `py.typed` and a stub for the extension module: every
  door returns its own `Success[...] | Fault`, `Verdict[T]` is generic, and the documented
  `assert_never` exhaustiveness now type-checks. Every native door and `NumFormat` member
  carries a docstring, word for word with its wasm twin. *(PyPI)*
- **Ruby — `rake native:dev`.** Builds the Magnus extension for the running Ruby and stages
  it where `require` looks. *(dev only)*
- **The declared floors are tested, and so is every wheel.** CI's new musl job runs the PHP,
  Ruby and Python suites on the oldest version each package declares as well as the newest;
  the Go purego backend now runs on Linux and macOS, where only cgo did; and the release
  installs each wheel and calls into it before anything is published. *(dev only)*
- **The AOT and browser smoke tests cross every native entry point**, the twenty-one
  `cast_*` functions and `hypercast_version`, in C# and Java; each ran "a door from every
  family" before. *(dev only)*
- **A browser proof of the C# WebAssembly package.** `csharp/HyperCast.WasmSmokeTest` is a
  Blazor WebAssembly app that imports the shipped `build/HyperCast.targets` and runs
  a door from every family, a fault through the union `switch` and the native-version probe.
  `./check.sh` publishes it, loads it in headless Chromium and requires `PASS`. Not yet run
  from a packed `.nupkg` in a separate consumer project or in CI. *(dev only)*
- **`cargo ruby-ext` and `cargo php-ext`**, aliases in `rust/.cargo/config.toml`, and
  `python/.cargo/config.toml` for maturin: each extension builds into its own target
  directory, so none of them overwrites the plain cdylib the other bindings load. *(dev only)*

### Changed

- **Every benchmark table was re-measured on x86-64, and several verdicts changed.** The
  published figures came from an arm64 WSL2 machine. On linux-x64 (an Intel Core i9-11900H)
  the doors' own costs moved little; the platform parsers they are timed against moved a
  lot — PHP's date functions run about 2.5x faster here — so the ratios did. C#: the
  date and time doors are 2.2–3.9x ahead of the BCL, `double` and a grouped `int` are
  washes, and `Guid` is 1.5x behind where it was a wash. Java: 4.6–12.3x on the date and
  time doors; `UUID.fromString` now beats the door by ten nanoseconds where it trailed it,
  and a `BigDecimal` row is printed for the first time, a 2.3x loss. Swift: 12.6x on a
  timestamp, 15x on a UUID. Go: every door still loses to the stdlib except the messy
  date-time one, which is 1.3x ahead. Ruby's wasm backend is faster than Fiddle rather
  than level with it; Ruby and Python are otherwise stated in the two entries below, which
  they were measured after. PHP: level with `DateTimeImmutable` (was 2.7x ahead) and 1.5x behind
  `createFromFormat` (was 2.2x ahead). The losses are stated as what they are: a few
  nanoseconds to a few hundred, against a parser built for the one shape being timed, for
  a format declared per call, a fault with a span, and the same verdict in seven
  languages. Java's table is the JMH profile the build's own task runs, and every README
  names the machine and runtime behind its tables. *(docs)*
- **Java — GraalWasm 25.4.4.1.1.** The wasm backend's optional engine moves from 25.3.4.1
  to the release that matches GraalVM 25.4, in the build, the benchmarks, the AOT smoke
  test and the README's dependency snippet. The two have to match: on a 25.4 JDK the older
  artifacts ran the module interpreted on the JVM, with no error, and failed a Native Image
  build outright. *(docs, dev only)*
- **Python — the doors that return an object cost about half what they did.** Past the
  parse, a door's cost was the Python object it handed back, built by calling Python
  callables. A `uuid.UUID` is now allocated and its slots set through the C API; a
  `datetime`, `date` or `time` is built from the packed state the `datetime` module's own
  pickling uses; and a `timedelta` comes from subtracting two datetimes, since its
  constructor is the slowest in the module. All of it is inside the stable ABI, so the
  wheels are unchanged, and each has a fallback to the ordinary constructor for a value the
  short path cannot carry. On CPython 3.14: `cast_uuid` 557 ns to 262 (3.0x
  `uuid.UUID()`), `cast_timestamp` 369 to 252 (1.7x behind `fromisoformat`, was 2.5x),
  `cast_datetime` 339 to 230 (20x `strptime`), `cast_duration` 581 to 262. *(PyPI)*
- **Ruby — a lean door on the Magnus backend costs a quarter of what it did.** The
  extension returned its `Success` or `Fault` through `Data.new`, whose keyword handling
  alone cost ~280 ns, more than the cast. It now builds the instance directly — allocated,
  its members stored, frozen — and falls back to `.new` if a Ruby ever stops representing
  `Data` as a struct; a spec pins the result as indistinguishable (`==`, `eql?`, `hash`,
  `to_h`, `with`, pattern matching, `Marshal`). `bool` 462 ns to 112, `i32` 468 to 133,
  `uuid` 629 to 223, `timestamp` 794 to 447 — 6.6x `Time.iso8601`, was 3.4x — and the
  civil date-time door is level with `DateTime.strptime` where it trailed it. *(RubyGems)*
- **Only upstream-supported runtimes.** PHP's floor is 8.2 (8.1 ended 2025-12-31; the
  binding already used `readonly class`, which is 8.2 syntax, while declaring 8.1), Ruby's
  is 3.3 (3.2 ended 2026-03-31), Python's is 3.11 (3.10 ends 2026-10-31; the wheels are
  `abi3-py311`), and Java's is JDK 25 (22, 23 and 24 are end of life; the jar is compiled
  `--release 25`). A consumer on an older runtime keeps resolving 0.3.0.
  *(Packagist, RubyGems, PyPI, Maven Central)*
- **A release rebuilds the native libraries at the version it ships.** `prepare-release`
  dispatches CI on the version-bump commit, staging follows that run automatically, and
  `release.yml` refuses a tag whose CI run or committed libraries were built at any other
  version. See the first Fixed entry. *(release machinery)*
- **An architecture with no native build is no longer taken for x64.** Go and PHP report an
  unsupported platform, Swift refuses to compile for it, and Java resolves to no native
  build and falls back to wasm; Java also falls back when a bundled library will not load.
  PHP on Windows always loads the x64 library, since PHP there is an x64 process even on
  ARM hardware. *(Maven Central, `go get`, `.package(url:)`, Packagist)*
- **PHP — ext-mbstring is no longer needed.** `NumFormat` validated and decoded separators
  with `mb_*` functions the package never declared; it uses PCRE and a small decoder now, so
  `ext-ffi` is the only extension required. *(Packagist)*
- **`NumFormat::fromLocaleconv` / `from_localeconv` never puts one character in both
  roles.** A field the locale leaves empty takes its invariant default unless the other
  separator already holds that character, in which case it takes the other of the pair, so a
  comma-decimal locale with no thousands separator groups on `.`. PHP used to throw for it;
  Python's extension built a `,`/`,` format no door can read. Two separators a locale itself
  declares equal are still a caller bug. *(Packagist, PyPI)*
- **Ruby — caller bugs raise the same exception on every backend**, `NumFormat` stores its
  separators as UTF-8, the `uuid` door returns a US-ASCII String on every backend as
  `SecureRandom.uuid` does, and `CURRENCY_MAX_BYTES` lives on `NumFormat`, where it was
  meant to. *(RubyGems)*
- **Python (wasm) — argument errors match the extension**: a non-`str` separator or a
  non-integer flag set is `TypeError`, one too wide for 32 bits is `OverflowError`. *(PyPI)*
- **Swift and Go open the native library `RTLD_LOCAL`**, and Swift opens it in place rather
  than copying it to a fresh temp file per process. *(`.package(url:)`, `go get`)*
- **Swift — the floor is Swift 6.2, and Linux links the core in.** On Linux the package
  no longer loads a shared library: the core is a static library SwiftPM links into the
  consumer's executable (a binary target, SE-0482, which is what sets the floor). Nothing
  has to be deployed beside the executable, `Cast.isAvailable` is always `true` there, and
  `NativeLibraryError` is never thrown. macOS and Windows still load a bundled shared
  library, and find its resource directory under both names SwiftPM uses: `.bundle` (Swift
  6.4's default build system) and `.resources` (6.2 and 6.3 on Windows). A toolchain older
  than 6.2 keeps resolving 0.3.0. *(`.package(url:)`)*
- **Grouped, currency and accounting input casts in about half the time.** `12,345.67`,
  `$12,345.67` and `($12,345.67)` used to take the full normalize-then-parse engine, at
  roughly twice the cost of a plain number — and it was leaving the plain path that cost,
  not the symbol: grouping alone was as slow as all three together. A second fast lane
  (`rust/src/lane.rs`) now reads those shapes in one pass and hands anything it does not
  recognise to the engine untouched, so no verdict and no fault span changes; a
  differential test runs every numeric door with the lane on and off over an enumerated
  grammar and requires identical answers. In the core, on one machine: an i64 from 41–45 ns
  to 14, a decimal from 42–50 ns to 21–24, a real from 40–42 ns to 33. A plain decimal
  such as `12345.67` also went from 25.5 ns to 11.8.

  Through the C# binding, old core against new on the same machine and in the same session
  (x86_64, .NET 11, the `string` doors with the transcode included), with the BCL beside
  them: `Cast.Int32("1,234,567")` 65 ns to 43, against 43 for `int.TryParse`;
  `Cast.Decimal("12,345.6789")` 98 ns to 62, against 59; `Cast.Decimal("($1,234.50)")`
  104 ns to 66, against 60 for `NumberStyles.Currency`; `Cast.Double("($1,234.50)")` 83 ns
  to 74, against 52; `Cast.Double` on `1.234.567,89` 77 ns to 61, against 58 for de-DE.
  Four of the five rows where this binding trailed the BCL are now within about ten
  percent of it; the real door on currency input still trails. The READMEs' tables were
  re-measured on that machine in the same round (see Changed). *(every package)*
- **Go — a cgo build links the core in.** On Linux and macOS, amd64 and arm64, the core
  is a static library on the cgo link line (`go/staticlib/`), not a shared library embedded
  for every platform, written to a temp file and `dlopen`ed on first use. A program that
  does nothing else is 3.2 MB instead of 6.7 MB, starts without touching the filesystem, and
  runs with no writable temp directory — including fully static, in an empty read-only
  container. One archive serves glibc and musl. `Available()` is always `true` in this
  build. `CGO_ENABLED=0`, Windows and cross-compiles still load through purego, unchanged;
  `-tags hypercast_dynamic` keeps cgo and loads the shared library as before. *(`go get`)*
- **C# — a Native AOT publish links the core in.** The package carries a static library
  per RID (`staticlibs/`), and its targets file hands the right one to the AOT linker and
  binds the P/Invokes as direct calls, so the publish directory is one executable with no
  `libhypercast` beside it. `<HyperCastStaticLink>false</HyperCastStaticLink>` restores the old
  behaviour; a JIT process is unaffected. *(NuGet)*
- **Intel macOS (`osx-x64`) is built and core-tested, with no CI leg of its own.** The
  library is cross-compiled on the Apple silicon runner, attested and shipped in every
  package as before, and the Rust core's own suite runs on it under Rosetta 2. No binding's
  suite runs there any more, and there is no `x86_64-darwin` precompiled gem: Ruby on an
  Intel Mac installs the universal gem and runs on Fiddle, the slower of the two backends. The `osx-x64` wheel is cross-built on the same runner and still installed
  and called into, under an x64 Python, before it is published. The leg took 37 minutes
  against 8 on Apple silicon, on hardware Apple stopped selling in 2023.
  *(RubyGems; CI for everything else)*
- **CI builds on Ubuntu 26.04 and tests Swift on 6.4.** The Linux legs name `ubuntu-26.04`
  and `ubuntu-26.04-arm` rather than `ubuntu-latest`. The glibc floor of the shared
  libraries is unchanged at 2.34, and CI now fails a Linux leg whose library references
  anything newer.

### Fixed

- **Java — every FFM door in a GraalVM Native Image went through the method-handle
  interpreter.** A native image built from this jar ran a door in 7.8–9.1 µs where the JVM
  takes 18–52 ns, 150 to 450 times slower and ten to thirty times slower than the wasm
  backend in the same binary. The downcall handles were `static final` but bound to the
  library's addresses, so their class initialized at run time, and Native Image only
  compiles a call through a handle that is a constant when the image is built. There is
  now one handle per ABI shape, created without an address in a holder class
  (`Cast.Downcalls`) that a `native-image.properties` in the jar initializes at image build
  time; each door passes its export's address as the first argument. In a native image:
  `bool` 60 ns, a grouped `i32` 91, `f64` 106, `timestamp` 111. Nothing changes on the JVM,
  and a consumer's `native-image` build inherits the setting with no configuration.
  *(Maven Central)*
- **C# — a Blazor WebAssembly app could not use HyperUuid and HyperCast together.** Each
  package's wasm static library bundled its own copy of Rust's standard library, and the
  two collided at link time: `wasm-ld: duplicate symbol: rust_eh_personality`. The library
  is now built without std (`cargo wasm-staticlib`: `--no-default-features` plus a
  `staticlib` feature that supplies the panic handler std would have, with panics
  aborting), so there is nothing to collide. Proven by linking both packed
  packages into one Blazor app and running it in headless Chromium; CI fails the build if
  the library ever defines `rust_eh_personality` again. *(NuGet, `hypercast` crate)*
- **C# — a Blazor WebAssembly app that reached the package through a class library got no
  native link at all.** NuGet imports `build/` only into a project that references a package
  directly, so an app depending on a library that depends on HyperCast received the managed
  assembly and none of the wasm wiring. The package now also ships `buildTransitive/`,
  which flows to every project downstream, and an app using both HyperUuid and HyperCast is
  handed the exception-handling translation flag once instead of twice. The file now sits at `build/HyperCast.targets`, with no target-framework folder. Proven with
  a packed `.nupkg`, a class library and a Blazor app in headless Chromium. *(NuGet)*
- **0.3.0's native libraries report version 0.2.0.** `hypercast_version` is compiled in
  from the crate manifest, and 0.3.0's libraries were built one minute before the manifest
  was bumped: the NuGet package, the jar, the gems and the libraries committed for Go, Swift
  and PHP all carry them, so `NativeVersion` reads `0.2.0` from a 0.3.0 package (the PyPI
  wheels and the crate, built from the tag, are right). Behaviour is unaffected; the number
  is wrong. This release is built in the corrected order. *(every package but PyPI and the
  crate)*
- **Python — `bytes` input cost about a microsecond more than `str` on every door.** The
  derived PyO3 extractor tried `str` first and built, then discarded, a `TypeError` for
  every `bytes` call: `"42"` at 116 ns against `b"42"` at 1,179 ns. A hand-written extractor
  checks the type directly, and the two now cost the same. The item `docs/roadmap.md`
  recorded as measured, not built. *(PyPI)*
- **Ruby — `HyperCast.date(text, nil)` raised `TypeError` on the Magnus backend**, though
  an explicit nil is the documented default; the Fiddle backend's packed-format memo grew
  without bound and now holds 64 formats; and on Alpine, 0.3.0 loaded the glibc library and
  `available?` was false. *(RubyGems)*
- **Swift — a missing resource bundle crashed the process, `Cast.isAvailable` included.**
  SwiftPM's generated accessor calls `fatalError`; the loader finds the directory by name
  now, so the probe is `false` and the doors throw. *(`.package(url:)`)*
- **C# — a `NumFormat` with no currency symbol set could throw `NullReferenceException`**,
  `DateOrders.From(null)` now throws `ArgumentNullException`, and every `ArgumentException`
  from a bad format names the `format` argument. `global.json` pinned nothing: `"11.0.100-"`
  is not a valid version and the host ignored the file. *(NuGet)*
- **Java — selecting wasm without GraalWasm now says what to add.** *(Maven Central)*
- **Go — `corpus_test.go` could loop forever on Windows**, as could PHP's corpus test, when
  `corpus/` was not above the test directory. *(dev only)*
- **Docs that had drifted from the code.** "20 `cast_*` exports" where there are 21; the
  Python READMEs described an sdist that is not published; C#'s `IsAvailable` claimed an ABI
  check it does not make and called `string`-door fault offsets bytes; Swift's provenance
  command named a file that does not exist. PHP's README now says how to enable FFI under a
  web SAPI. *(docs only)*
- **C# — Blazor WebAssembly on .NET 11 failed in the browser.** A successful `wasm-ld` link
  had hidden it: .NET 11 links browser-wasm with the new exception-handling encoding while
  the precompiled Rust standard library inside the static library uses the legacy one, and
  the browser refused the module (`module uses a mix of legacy and new exception handling
  instructions`). `HyperCast.targets` now appends Binaryen's translate-to-exnref pass to the
  SDK's post-link `wasm-opt`.

## [0.3.0] — 2026-09-04

*One core, one more way in*, ported from HyperUuid 0.3.0: Java, Ruby, Python and Go can now
run the Rust core as a `wasm32-wasip1` module inside the process, through a wasm engine the
ecosystem already has, so a platform with no native build in the package still has a working
backend and nothing has to be `dlopen`'d at all. No door changed its verdict on any input:
the corpus replays byte-identical through every backend, native and wasm alike.

*What the first consumer asked for.* Svartalfheim's ingestion branch
([NorseArchitecture/Svartalfheim#62](https://github.com/NorseArchitecture/Svartalfheim/pull/62))
routed a whole scalar-parser family through the C# binding and surfaced, in one PR, every
gap this section closes: a declared currency symbol (without it every non-invariant culture
fell back to managed code), an exact decimal door, a way to prove the native library loaded
before the first cast, and a handful of binding ergonomics. One coordinated ABI bump across
the core and all seven bindings; `docs/roadmap.md` records the reasoning.

### Added

- **A declared currency symbol on `NumFormat`, and a `CURRENCY` flag (now part of `ALL`).**
  The core's format carries up to 16 UTF-8 bytes of symbol inline; with the flag set the
  symbol is accepted once, leading — before or after a sign (`$5`, `-$5`, `$ -5`) — or
  trailing (`5 €`, `1.234,50 kr.`), with optional whitespace between symbol and digits, and
  accounting parentheses wrap symbol and digits together (`($5)`). Declared with the flag
  off it is `Malformed` at the symbol; flag on with nothing declared changes nothing. A
  symbol carrying an ASCII digit or whitespace, or longer than 16 bytes, is a contract
  violation at the ABI and a caller-bug exception in every binding. Every culture bridge
  fills it in from the platform's own data: C# `NumFormat.From(CultureInfo)`, Java
  `NumFormat.from(Locale)`, Swift `NumFormat.from(locale:)`, Python and PHP
  `from_localeconv`/`fromLocaleconv`. `corpus/integer.json` and `corpus/real.json` gained
  currency vectors, and a `format` object may now carry `"currency"`. *(every package)*
- **`cast_decimal` — an exact decimal door.** The real doors' grammar (declared separators
  and grouping, parentheses, exponent, percent, currency, separator detection), but no
  float is ever formed: the value is a sign, a 96-bit magnitude and a base-10 scale, so
  `0.1` is one tenth and `50%` is exactly `0.5`. The result is canonical: exact trailing
  zeros in the fraction are trimmed, so `1.10` and `1.1` come out the same (magnitude 11,
  scale 1) and zero is scale 0, never negative. Precision is a range, not a rounding
  opportunity: a magnitude past 2⁹⁶−1, or more than 28 nonzero places, is `OutOfRange` —
  nothing but a zero is ever dropped. Presented as `decimal` (C#), `BigDecimal` (Java),
  `decimal.Decimal` (Python), `Foundation.Decimal` (Swift), and an exact carrier type
  where the platform has no decimal:
  Go `Decimal` (door `Exact`), Ruby `HyperCast::Decimal` (with `to_r`, canonical
  `to_s`, lazy `to_d`), PHP `HyperCast\Decimal` (magnitude as a numeric string). New
  `corpus/decimal.json` pins the raw triple and the canonical text for every binding.
  *(every package)*
- **`hypercast_version` — the load probe.** A zero-argument export returning the core's
  version packed `major << 16 | minor << 8 | patch`: the cheapest possible proof that the
  library a host loaded is the one its binding was built against, before any door is the
  thing that finds out. Every binding fronts it two ways — an availability check that
  never throws (`Cast.IsAvailable`, `Cast.isAvailable()`, `Available()`,
  `Cast.isAvailable`, `HyperCast.available?`, `Cast::isAvailable()`) and the loaded
  version as text (`Cast.NativeVersion`, `nativeVersion()`, `NativeVersion()`,
  `native_version`). A consumer keeping a managed fallback gates on the probe instead of
  catching a load failure around its first real cast — the exact hole Codex's review found
  in the consumer PR, where only the sibling library was probed. *(every package)*
- **One numeric door generic over the target.** For a caller that is itself generic over
  the target type — a parser family, a column mapper — so it need not write the type
  dispatch: C# `Cast.Numeric<T>` over exactly the eleven numeric targets, Go
  `Numeric[V Number]` with the eleven as a constraint, Swift `Cast.numeric<T>`. The
  dispatch folds per instantiation; Go's stays allocation-free. Not in Java (no generics
  over primitives) nor the dynamic bindings, where the per-width doors are the idiom.
  *(NuGet, Go, Swift)*
- **`NumFormat.From(IFormatProvider)` and `From(NumberFormatInfo)` in C#** — the shape
  every BCL `TryParse` already takes, so an `IFormatProvider`-shaped caller needs no
  `CultureInfo` cast; and **`NumFormat::fromLocaleconv()` in PHP**, the platform-data
  bridge the other bindings already had. *(NuGet, Packagist)*
- **Rust: `NumFormat::new`, `NumFormat::with_currency`, `CurrencySymbol`, `Decimal`** (with
  `magnitude()` and a canonical `Display`), and `hypercast_version()` re-exported. The
  fuzz target covers the decimal door and three currency profiles; the allocation proof and
  the fault-span sweep cover both. *(`hypercast` crate)*
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

- **`RawNumFormat` is 32 bytes.** Four `u32`s — `decimal_sep`, `group_sep`, `flags`,
  `currency_len` — then the symbol's 16 bytes inline; alignment stays 4. Every binding's
  crossing, native and wasm alike, was updated together; nothing else at the ABI moved.
  *(every package)*
- **`ALL` is 95, not 31** — the currency lenience joined it — in every binding's flag set
  (`NumStyles.All`, `STYLE_ALL`, `AllStyles`, `.all`, `ALL_STYLES`, `NumFormat.ALL`).
  `SEPARATOR_DETECT` stays excluded. A caller that spelled `31` keeps exactly the old
  behavior. *(every package)*
- **Fault spans come back in the caller's own units.** The core reports UTF-8 byte
  offsets; the C# `string`/`ReadOnlySpan<char>` doors, Java `String` doors, Python `str`
  input and Ruby text input now remap a fault's offset and length to char/code-point units
  when the input was not ASCII, so slicing the offending text back out of what was passed
  needs no mapping. Byte input, and ASCII text, are unchanged and pay nothing. Go, Swift
  and PHP strings are already UTF-8 bytes. *(NuGet, Maven Central, PyPI, RubyGems)*
- **Java: `NumFormat` is a four-component record** (`currencySymbol` last); the
  three-argument constructor remains and declares no symbol. **Rust: `NumFormat` gained a
  `currency` field** — a struct literal of the three old fields no longer compiles; use
  `NumFormat::new(decimal_sep, group_sep, flags)` or `{ ..NumFormat::INVARIANT }`. *(Maven
  Central, `hypercast` crate)*
- **Go, purego backend:** a core missing an export is a reported load failure the probe
  sees, instead of a panic escaping initialization — matching the cgo and wasm backends.
  *(`go get`)*

- **The allocation proof counts per thread.** `rust/tests/allocation_free.rs` moved from a
  process-wide atomic to a `const`-initialised thread-local, so the test harness's own
  allocations on its main thread — a join-handle map insert and a timeout push right after
  `spawn` — can no longer race the claim on a slow-to-schedule runner. HyperUuid saw exactly
  that flake on linux-arm64. *(`hypercast` crate, tests only)*

### Notes

- **`HyperCast.Corpus` 0.3.0 exists on nuget.org and should not.** This release's run pushed
  a content-only package of `corpus/*.json` before the decision that the corpus is this
  repository's receipt and not a product had been applied to the release train. The package
  is unlisted, the project and its release steps are removed, and no later version will
  follow. A downstream suite that wants the vectors takes them from this repository at the
  tag it was built against.

### Upgrade note

Source-compatible for every binding's consumers, with three things to know. The native
library and the binding move together: the format crossing is 32 bytes now, and a binding of
this version on an older `libhypercast` fails at load, which the new probe reports rather
than the first cast. `ALL` changed value: code that stored the number keeps the old lenience
set, code that named the constant gains currency, which with no symbol declared changes
nothing. Non-ASCII fault spans on the C#, Java, Python and Ruby text doors now index
characters rather than bytes; the byte doors are untouched. In Rust, `NumFormat` struct
literals need the new field or `NumFormat::new`.

The wasm backends are opt-in and change nothing until asked for: no new runtime dependency
in any package (Java's GraalWasm is `compileOnly`, Ruby's wasmtime a Gemfile group for the
suite, Python's an extra, Go's behind a build tag — though wasmtime-go does now appear in
`go.mod`, so it enters a consumer's module graph without entering their binary). The
`.cargo/config.toml` section that exports `malloc`/`free` on `wasm32-wasip1` applies to
builds run from `rust/` and to nothing a consumer compiles.

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

[Unreleased]: https://github.com/SkunkWerkx/HyperCast/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/SkunkWerkx/HyperCast/compare/v0.6.1...v0.7.0
[0.6.1]: https://github.com/SkunkWerkx/HyperCast/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/SkunkWerkx/HyperCast/compare/v0.4.0...v0.6.0
[0.4.0]: https://github.com/SkunkWerkx/HyperCast/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/SkunkWerkx/HyperCast/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/SkunkWerkx/HyperCast/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/SkunkWerkx/HyperCast/releases/tag/v0.1.0
