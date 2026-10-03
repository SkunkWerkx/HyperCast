# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![RubyGems](https://img.shields.io/gem/v/hypercast.svg)](https://rubygems.org/gems/hypercast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**Ruby's own pattern matching over two `Data` case types — the value, or a closed reason
Symbol plus the exact span that offended. Two backends, one public surface: a Magnus
native extension where a precompiled platform gem covers you, stdlib Fiddle as the last
resort everywhere else — selected automatically, zero compiles either way.**

Allocation-lean scalar casts — booleans, the full integer family, reals, exact decimals,
UUIDs, temporals — calling directly into the native `libhypercast` Rust core. Ruby 3.3 is the
floor. The fast path links the core straight into a Ruby extension (Magnus): on require it
redefines the doors in place on the `HyperCast` module — no delegation layer, no second
surface, which is exactly what keeps the backends provably in agreement.
The last-resort fallback, in the universal gem only, calls the native `libhypercast` shared
library via [`Fiddle`](https://docs.ruby-lang.org/en/master/Fiddle.html) — dlopen/dlsym plus
raw C-ABI calls, no runtime bridge. `HyperCast::BACKEND` reports which one is live;
`HYPERCAST_PURE=1` forces Fiddle for testing — see [Backends](#backends).

```ruby
case HyperCast.i32("(1,234)", HyperCast::NumFormat::INVARIANT)
in HyperCast::Success(value:) then puts "got #{value}"          # -1234
in HyperCast::Fault(reason:, offset:) then puts "#{reason} at #{offset}"
end
```

Door names mirror the native ABI (`i32`, `f64`, `timestamp`, …). Ruby-flavored fidelity,
stated proudly — nothing the core parses is lost on the way out: `Integer` is
unbounded (u64 comes back as the true unsigned value), `Time` carries full nanoseconds
across the whole 0001–9999 window, time-of-day is an exact Integer of nanoseconds since
midnight, durations come back as exact `Rational` seconds across the core's whole
±10,000-year window, and `decimal` returns an exact `HyperCast::Decimal` that never rounds
— no truncation anywhere, no wrapping.

## The doors

| Door | Value | Declares |
|---|---|---|
| `bool(text)` | `true`/`false` | — |
| `i8` `i16` `i32` `i64` `u8` `u16` `u32` `u64` `(text, format)` | `Integer` (unbounded — u64 is the true unsigned value) | a `NumFormat` |
| `f32` `f64` `(text, format)` | `Float` (f32 widened losslessly) | a `NumFormat` |
| `decimal(text, format)` | `HyperCast::Decimal` — exact sign, 96-bit magnitude, base-10 scale | a `NumFormat` |
| `uuid(text)` | lowercase hyphenated `String` (`SecureRandom.uuid`'s shape) | — |
| `timestamp(text)` | UTC `Time`, full nanoseconds | — |
| `unix(text, precision)` | UTC `Time` | `:seconds` / `:milliseconds` / `:microseconds` / `:nanoseconds` |
| `excel_serial(text, epoch)` | UTC `Time` | `:y1900` / `:y1904` |
| `date(text, order = nil)` | `Date` | strict ISO, or `:year_month_day` / `:month_day_year` / `:day_month_year` |
| `datetime(text, order)` | zone-less `DateTime`, exact `Rational` seconds | a field order, as `date` |
| `time(text)` | `Integer` nanoseconds since midnight | — |
| `duration(text)` | exact `Rational` seconds | — |

Every door returns a `Success` or a `Fault`; `HyperCast.optional(verdict)` folds `:empty`
to `nil`. Beside the doors, `native_version` returns the loaded core's
`"major.minor.patch"` and `available?` answers `true`/`false` without ever raising — see
[Is the native core there?](#is-the-native-core-there).

### Errors

Bad data is never an exception — it is a `Fault`. What raises is a caller bug, and it raises
the same exception on every backend:

- `ArgumentError` from `NumFormat.new` for a malformed format: separators that are not single
  characters or not distinct, a currency symbol that is too long or carries an ASCII digit or
  whitespace.
- `KeyError` — `Hash#fetch`'s own, `key not found: …` — for a precision, epoch or date order
  that names nothing: an unknown Symbol, or anything that is not a Symbol (`"seconds"`
  included). `date(text, nil)` is not one of these: an explicit `nil` order is the strict ISO
  door, exactly as if it were left off.
- `TypeError` for text that is not a `String` (anything with `to_str` is taken as one).
- `String#encode`'s own `Encoding::` error for text in another encoding that cannot be
  transcoded to UTF-8.

### Is the native core there?

`HyperCast.available?` answers whether a backend actually loaded and exports the ABI this
binding was built against — probed once, cached, never raising — for a consumer with a
fallback of its own:

```ruby
value = HyperCast.available? ? HyperCast.i32(text, format) : my_own_parse(text)
```

`HyperCast.native_version` reads the version out of the loaded core itself (its
`hypercast_version` export — the cheapest probe that the backend resolved at all), so a
mismatch against `HyperCast::VERSION` can be named before the first cast.
`HyperCast::BACKEND` says which backend was *selected*, which is not the same question: it
reads `:fiddle` on a platform with no library at all, because that is the backend whose
first call explains what is missing — a `LoadError` naming the path it looked for, or
`HyperCast::NativePlatform::UnsupportedPlatformError` naming the platform. Inside a platform
gem, which carries no Fiddle library, that `LoadError` names the universal gem instead (see
[Backends](#backends)).

### Fault spans index the String you passed

A `Fault`'s `offset`/`length` are in the units `String#[]` slices by on your own input:
character offsets for text — in any encoding; the core reads UTF-8, and the span is mapped
back, an identity for ASCII — and byte offsets for a binary (`ASCII-8BIT`) String, whose
characters are its bytes. Either way `text[offset, length]` is the offending text with no
arithmetic on your side:

```ruby
HyperCast.i32("1€", HyperCast::NumFormat::INVARIANT)     # => Fault(reason: :malformed, offset: 1, length: 1)
HyperCast.i32("1€".b, HyperCast::NumFormat::INVARIANT)   # => Fault(reason: :malformed, offset: 1, length: 3)
```

The mapping happens only on the failure path, so a `Success` never pays for it.

### `decimal`: exact, never rounded

`HyperCast::Decimal` is a `Data` of `magnitude` (an unbounded `Integer`, at most 2⁹⁶ − 1),
`scale` (0–28: places the magnitude is shifted right) and `negative` — the shape .NET's
`decimal` stores and `BigDecimal` builds from directly. No float is ever formed, so `"0.1"`
is one tenth and `"50%"` is exactly `0.5`. The triple is canonical: exact trailing zeros in
the fraction are always trimmed, so the scale is minimal — `"1.10"`, `"1.1"` and `"1.1000"`
are all magnitude 11, scale 1, while `"100"` stays magnitude 100, scale 0 — and zero is
scale 0 and never negative. Nothing but a zero is ever dropped: text carrying more precision
than 96 bits and 28 places can hold is an `:out_of_range` `Fault`, not an approximation.

```ruby
value = HyperCast.decimal("-1,234.50", HyperCast::NumFormat::INVARIANT).value
value.to_s   # => "-1234.5"  (the canonical text, every binding renders the same)
value.to_r   # => (-2469/2)
value.to_d   # => BigDecimal("-1234.5")
```

`to_d` requires the `bigdecimal` gem lazily on first call — a bundled gem since Ruby 3.4,
deliberately not a dependency of this one, so under Bundler add it to your own Gemfile.

### `NumFormat`: declared separators, and a declared currency

```ruby
HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: HyperCast::ALL_STYLES, currency: "$")
```

The flags are `GROUPING`, `PARENTHESES`, `EXPONENT`, `RADIX_PREFIXES`, `PERCENT` and
`CURRENCY` (`ALL_STYLES` is all six), plus the `SEPARATOR_DETECT` policy. The currency
symbol is the one field a culture table would fill in: it is declared, never looked up, and
honored only while `CURRENCY` is set — once, leading (`$5`, `-$5`, `$ -5`) or trailing
(`5 €`, `1.234,50 kr.`), with optional ASCII whitespace between symbol and digits, and
accounting parentheses wrapping symbol and digits together (`($5)`). Declared with the
flag off, the symbol is a `:malformed` `Fault` at the symbol; the flag with nothing declared
(`currency: ""`, the default — `INVARIANT` and `DETECT` declare none) matches nothing. A
symbol longer than 16 UTF-8 bytes, or carrying an ASCII digit or ASCII whitespace, is an
`ArgumentError` at construction, the same caller-bug treatment equal separators get.

```ruby
dollars = HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: HyperCast::ALL_STYLES, currency: "$")
HyperCast.i32("($1,234)", dollars)        # => Success(value: -1234)
HyperCast.decimal("$1,234.50", dollars)   # => Success(value: Decimal(magnitude: 12345, scale: 1, negative: false))
HyperCast.i32("$5", HyperCast::NumFormat::INVARIANT)   # => Fault(reason: :malformed, offset: 0, length: 1)
```

Across the ABI a `NumFormat` is a 32-byte struct — the two separators as code points, the
flags, and the symbol's length and UTF-8 bytes held inline — packed once per format object
and memoized by identity on every backend, so declaring a currency costs a cast nothing.

## Why not `Integer()` / `Time.iso8601` / `Float()`?

1. **Verdicts, not exceptions** — bad data is the expected case for untrusted text; a
   `Fault` is a Symbol and two integers, not an `ArgumentError` to rescue.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, radix prefixes, all five .NET `Guid` text forms plus
   `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (the whole suite green on both backends, full corpus replay;
   cross-backend agreement specs compare Magnus against Fiddle across a subprocess
   boundary).
4. **Faster than the stdlib on the Magnus backend, where the carrier is cheap** —
   benchmark-ips (`ruby benchmark/cast_benchmark.rb`, linux-x64 on an Intel Core
   i9-11900H, Ruby 4.0.7): timestamp **447 ns vs 2.97 µs `Time.iso8601`** (6.6x) — while
   returning exact `Rational` durations on the duration door. The Fiddle fallback lands at
   3.25 µs: a little behind `Time.iso8601`, sitting on Fiddle's per-call marshalling floor.

   Separator detection is nearly free here: `1.234.567,89` under `NumFormat::DETECT` runs
   at 178 ns against 170 ns for the same text under a declared eurozone format. Both cost
   nearly twice that in 0.1.0, for a reason that had nothing to do with parsing: every format other than `INVARIANT` paid three method dispatches and two
   `String` allocations per call to read its separators back out of the `Data`. `DETECT`
   is now identity-matched like `INVARIANT`, and any other format is resolved once per
   thread and memoized by identity — anchored in a thread-variable so the memo's key can
   never be a recycled address — which turned the per-call cost into one pointer compare.
   The three reason Symbols and the option Symbols (`:seconds`, `:month_day_year`, …) are
   cached the same way, so a fault or a declared option is a pointer compare too, never a
   `Symbol#name` materialization.

**The honest trade-off:** the civil doors do not beat `strptime`. The date-time door is
level with `DateTime.strptime` (904 ns against 901 ns) and the date door a little behind
`Date.strptime` (620 ns against 513 ns). The parse isn't the cost; the carrier is. Building
a stdlib `DateTime` with an exact `Rational` second costs more than the whole native call,
where the timestamp door's `Time` is built by a single cheap `rb_time_nano_new`. If you want
Ruby's fastest civil parse and don't need the verdict or the declared order, `strptime` is
as good. Also note the carrier's other caveat — `DateTime`'s offset defaults to `+00:00`,
which is an artifact of the type, not a zone the parse assigned.

On the Fiddle fallback the doors are parity-at-best — Fiddle's
per-call floor is the mechanism's price, kept because it's the universal zero-compile
path. Its doors build nothing per call that does not change between calls: each format
owns one native pointer, memoized by identity and passed straight through. (Benchmark forensics worth knowing: the doors read 4.3 µs until
per-call `Fiddle::Pointer.malloc` finalizers were hoisted to thread-local scratch —
receipts include their own archaeology.)

## Benchmarks

`ruby benchmark/cast_benchmark.rb` pairs each door with Ruby's closest stdlib parse. Its
first line names the backend, core version and Ruby it measured, so run it again under
`HYPERCAST_PURE=1` for the other backend. The stdlib comparisons are in
[the section above](#why-not-integer--timeiso8601--float); this is the two backends against
each other.

Measured on Ruby 4.0.7, linux-x64 (an Intel Core i9-11900H), `benchmark-ips`, same session,
both backends:

| Door | Magnus | Fiddle |
|---|---:|---:|
| `bool` | 112 ns | 2.35 µs |
| `i32` | 133 ns | 2.63 µs |
| `f64` | 166 ns | 2.70 µs |
| `uuid` | 223 ns | 3.47 µs |
| `timestamp` | 447 ns | 3.25 µs |
| `datetime` (`1/7/2026 3:04 PM`) | 904 ns | 3.98 µs |
| `duration` (ISO) | 632 ns | 2.91 µs |
| `i32`, a fault | 225 ns | 3.18 µs |

A lean door on the Magnus backend is little more than the native call: the extension builds
the `Success` or `Fault` it returns directly — allocated, its members stored, frozen —
rather than through `Data.new`, whose keyword handling alone cost more than the cast.

## Backends

| `HyperCast::BACKEND` | What runs | Chosen when |
|---|---|---|
| `:native` | the core linked into a Magnus extension | a precompiled platform gem is installed — every one carries an extension for each Ruby it installs on; see [Install](#install) |
| `:fiddle` | `libhypercast` for this platform, `dlopen`ed through Fiddle | no extension loads: the universal gem, on a Ruby or platform no platform gem covers; also the answer when nothing loads at all |

Selection happens once, at `require`, in that order.

`HYPERCAST_PURE` forces `:fiddle`, even where an extension would load. It is a testing and
diagnostic switch — CI runs the whole suite through it, and it is how to rule an extension
problem in or out — not a setting a deployment needs. It is read for presence, not value: set
to anything at all, `0` and the empty string included, it forces Fiddle. A forced backend that
turns out to have nothing to load does not fall through to another one: the first call raises,
and `HyperCast.available?` answers `false`. That is what happens inside a platform gem, which
carries no Fiddle library: the `LoadError` names the universal gem
(`gem install hypercast --platform ruby`, or Bundler's `force_ruby_platform`), where the
Fiddle backend lives.

**Threads.** Every backend is safe to call from any number of threads. The extension runs
under the GVL. Fiddle releases the GVL for the duration of each call, so that is the one
backend where Ruby threads run the core truly in parallel — each through its own scratch
buffers; the core itself keeps no state between calls. `spec/cast_spec.rb`'s
concurrent-callers example runs under both.

**Ractors.** Main Ractor only, on every backend: called from another Ractor the doors raise
`Ractor::UnsafeError`.

## Verifying provenance

Every gem RubyGems.org serves — the universal fallback and each of the seven precompiled
platform gems — carries its own GitHub build-provenance attestation, signed directly by
this repo's own `release.yml` (the `rubygems-publish` job attests `ruby/pkg/*.gem` right
before the push), so plain `--repo` verifies any of them:

```sh
gem fetch hypercast -v X.Y.Z --platform <platform>   # or omit --platform for the universal gem
gh attestation verify hypercast-X.Y.Z-<platform>.gem --repo SkunkWerkx/HyperCast
```

That's the release's second layer of checking, not the only one: before any gem gets built,
the same job verifies every native binary it packs — the Fiddle libraries, the Magnus
extensions (one per Ruby ABI per platform) — against *their own*
attestations — those are signed from `SkunkWerkx/.github` by `hyper-build-native.yml`, so
that check needs `--signer-repo SkunkWerkx/.github` added — and refuses to proceed on an
unverified one.
RubyGems.org has no unpublish and no duplicate-version overwrite, so this all happens while a
bad artifact is still reversible. The release run's job summary then re-fetches every gem
from the CDN and records attested-vs-served digests, turning "rubygems.org stores an upload
verbatim" into a per-release measurement rather than an assumption — see
[csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance) for
more on why `--signer-repo` is needed for some artifacts here and not others.

## Install

```sh
gem install hypercast
```

Eight gems are published per release: seven precompiled Magnus platform gems that
`gem install` and `bundle` auto-select when they match — `x86_64-linux-gnu`,
`aarch64-linux-gnu`, `x86_64-linux-musl`, `aarch64-linux-musl`, `arm64-darwin`,
`x64-mingw-ucrt` and `aarch64-mingw-ucrt` — and one universal `ruby`-platform gem. A platform
gem carries its Magnus extensions and nothing else native: no Fiddle library at all. The
universal gem is the last resort, Fiddle with every platform's native library bundled, and it
is what RubyGems resolves for a Ruby the platform gems do not cover (3.3, or a Ruby newer than
the release, such as 4.1 before a release ships for it) and on a platform no platform gem is
built for — Intel macOS among them, which runs on Fiddle. No extra configuration needed either
way.

Selection has **two** axes here, unlike every other binding in this repo. A Magnus extension
is bound to one Ruby minor ABI — there is no `abi3` equivalent to collapse the version axis
the way [the Python binding's](../python/) wheels do — so each platform gem is a "fat" gem
carrying one compiled extension per supported Ruby, under `lib/hypercast/<minor>/`, and picks
one at `require` time:

| Ruby | Linux (glibc and musl), Apple silicon macOS, Windows — x64 and arm64 | Gem installed |
| --- | --- | --- |
| a newer Ruby than the release covers (4.1+) | Fiddle | universal |
| 4.0 (primary) | Magnus, `BACKEND == :native` | platform |
| 3.4 (until its EOL 2028-03-31) | Magnus, `BACKEND == :native` | platform |
| 3.3 (the floor, until its EOL 2027-03-31) | Fiddle | universal |

What stands behind each cell: CI replays the whole suite, shared corpus included, through
each Magnus extension on every push — Ruby 3.4 and 4.0 on all seven platform-gem platforms,
the two musl ones inside each Ruby's own Alpine image — and the Fiddle suite on Ruby 4.0 on
every one of them, plus on Ruby 3.3 inside Alpine. Intel macOS has no CI leg: its library is
cross-built and tested at the core, and Ruby there runs the universal gem's Fiddle backend
over it. Anywhere else — a platform with no native build at all — the universal gem installs
but `HyperCast.available?` answers `false`, and the first call raises
`HyperCast::NativePlatform::UnsupportedPlatformError`.

The platform gems declare `required_ruby_version >= 3.4, < 4.1` precisely so RubyGems
*declines* them outside that range and resolves the universal gem instead — a wrong-ABI
extension must never be installed in the first place. On Windows it would at least fail to
load cleanly (the extension imports `<arch>-ucrt-ruby<minor>.dll` by name —
`x64-ucrt-ruby400.dll` on x64, `aarch64-ucrt-ruby400.dll` on ARM), but Linux extensions don't
link libruby at all, so one can load successfully against the wrong ABI and misbehave later.
When 3.4 goes EOL it simply leaves the matrix and its users fall back to Fiddle, which is
exactly what the fallback is for.

**musl.** Alpine has platform gems of its own, `x86_64-linux-musl` and
`aarch64-linux-musl`, whose extensions are built inside each Ruby's official `ruby:*-alpine`
image and need nothing beyond musl's libc. The glibc gems name their libc too
(`x86_64-linux-gnu`, `aarch64-linux-gnu`), which is what makes both `gem install` and
Bundler pick the right one on every supported Ruby: next to a plain `x86_64-linux` gem,
RubyGems before 4.0 resolves that one on Alpine instead, even under
`--platform x86_64-linux-musl`.

**Nothing in this gem is ever compiled, on any platform.** The platform gems depend on nothing
at all. The universal gem depends on `fiddle`, which can be: `fiddle` is a bundled gem on Ruby
4.0 and a default gem on 3.3 and 3.4, and `gem install` is satisfied by the copy Ruby ships.
Bundler resolves the newest `fiddle` on rubygems.org instead, and when that is newer than the
one your Ruby ships — true today on 3.3 and 3.4, not on 4.0 — it builds Fiddle's own C
extension, which takes a compiler and libffi's headers (`apk add build-base libffi-dev` on
Alpine). That only reaches you where the universal gem installs: Ruby 3.3, Intel macOS, a
Ruby newer than the release, or a platform with no platform gem. `bundle install
--prefer-local` makes Bundler use the copy Ruby ships where it can (it did on Ruby 3.4's
Bundler 2.6, not on 3.3's 2.5); otherwise install the compiler and headers. Pinning `fiddle`
to your Ruby's version in the `Gemfile.lock` does not stop the build.

Both Windows architectures get a Magnus gem, and the reasoning that once kept them on Fiddle
was backwards: MinGW is the *only* Windows flavour `rb-sys` targets (`x64-mingw-ucrt` and
`aarch64-mingw-ucrt`, both `supported: true` in its own `data/toolchains.json`); the one it
has no support for is MSVC. Windows is also where the Fiddle fallback cost the most, so these
are the most worthwhile gems in the set. Both extensions are built for the `gnullvm` Rust
targets rather than `gnu` — the same mingw-w64/UCRT ABI RubyInstaller's Ruby uses, linked
with LLVM and compiler-rt instead of GCC and a statically-linked libgcc, which is what keeps
the shipped extension small. The build script, and the two flags that are load-bearing on
the ARM leg (a static libunwind, and a clang-spelled `--target` for bindgen), live in the
forge's `ruby-magnus` action (`build-magnus.sh`), shared with every other Hyper* repo.

## Development

Everything below runs from a checkout, with `rust/` and `corpus/` beside `ruby/`; none of it
is needed to use the gem. The Fiddle backend finds the in-repo build on its own when nothing
is staged under `lib/hypercast/native/`. The Magnus backend does not — an
extension has to be built for the Ruby you are running and put where `require` looks — and
that is what `rake native:dev` is for.

```sh
cd rust
cargo cdylib                           # libhypercast, what the Fiddle backend loads

cd ../ruby
bundle install
bundle exec rake native:dev          # build the Magnus extension for this Ruby and stage it
bundle exec rspec                    # BACKEND == :native
HYPERCAST_PURE=1 bundle exec rspec   # BACKEND == :fiddle
bundle exec rake docs:check          # every public object carries a doc comment
ruby benchmark/cast_benchmark.rb     # prints the backend it measured
```

`rake native:dev` runs `cargo ruby-ext` (an alias in `rust/.cargo/config.toml` that builds the
`ruby` feature into `rust/target/ruby/`, never over the plain library in
`rust/target/release/`) and copies the result to
`lib/hypercast/<minor>/hypercast_native.<so|bundle>` — the path `lib/hypercast.rb` tries
first. The copy is also a rename, and it is the step `cargo ruby-ext` alone does not do: cargo
names its output `libhypercast.so`, and Ruby derives the `Init_` function it calls from the
file name it was asked to `require`. Three things worth knowing:

- **Run it again after changing `rust/`.** The staged file is a copy; nothing rebuilds it.
- **One staging per Ruby.** An extension is bound to one Ruby minor, so under a second Ruby
  (`RBENV_VERSION=3.4.11 bundle exec rake native:dev`, say) the task rebuilds for that ABI
  and stages beside the first, each Ruby loading its own.
- **Without it, `bundle exec rspec` still passes — on Fiddle.** The examples under "native
  backend" report themselves pending with `BACKEND=fiddle`; that line, or
  `ruby -Ilib -rhypercast -e 'p HyperCast::BACKEND'`, is how to tell which backend a run
  exercised. Deleting `lib/hypercast/<minor>/` goes back to Fiddle.

The task covers Linux and macOS. The Windows extensions need the `gnullvm` targets and linker
flags in the forge's `build-magnus.sh`, which is also what CI uses on every platform.

The musl build has no host to run on outside a container. With the core built for musl at
`rust/target/musl/linux-musl-x64/libhypercast.so`, this runs the Fiddle suite — corpus replay
included — against it on Alpine, with the checkout mounted read-only:

```sh
docker run --rm -v "$PWD/..":/src:ro ruby:4.0-alpine sh -euc '
  mkdir /work && cp -r /src/ruby /work/ruby && cp -r /src/corpus /work/corpus
  mkdir -p /work/ruby/lib/hypercast/native/linux-musl-x64
  cp /src/rust/target/musl/linux-musl-x64/libhypercast.so /work/ruby/lib/hypercast/native/linux-musl-x64/
  cd /work/ruby && rm -f Gemfile.lock
  bundle install --quiet --prefer-local
  HYPERCAST_PURE=1 bundle exec rspec'
```

The floor's test is the same container on `ruby:3.3-alpine`. Bundler would compile a newer
Fiddle there (see [Install](#install)), so this one goes around Bundler and runs against the
Fiddle that Ruby 3.3 ships:

```sh
docker run --rm -v "$PWD/..":/src:ro ruby:3.3-alpine sh -euc '
  mkdir /work && cp -r /src/ruby /work/ruby && cp -r /src/corpus /work/corpus
  mkdir -p /work/ruby/lib/hypercast/native/linux-musl-x64
  cp /src/rust/target/musl/linux-musl-x64/libhypercast.so /work/ruby/lib/hypercast/native/linux-musl-x64/
  cd /work/ruby && rm -f Gemfile Gemfile.lock
  gem install rspec -v "~> 3.13" --no-document --silent
  HYPERCAST_PURE=1 rspec'
```

The musl Magnus extension is built the way CI builds it by the forge's
`ruby-magnus-musl/build-magnus-musl.sh`, which runs on any machine with Docker: from the repo
root, `build-magnus-musl.sh hypercast linux-musl-x64 4.0 . <dir with the musl libhypercast.so> <out-dir>`
compiles it in `ruby:4.0-alpine`, then runs this suite through it on a bare copy of that image.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.

## License

[MIT](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)
