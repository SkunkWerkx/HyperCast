# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Gem](https://img.shields.io/gem/v/hypercast.svg)](https://rubygems.org/gems/hypercast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**Ruby's own pattern matching over two `Data` case types — the value, or a closed reason
Symbol plus the exact byte span that offended. Two backends, one public surface: a Magnus
native extension where a precompiled platform gem covers you, stdlib Fiddle everywhere
else — selected automatically, zero compiles either way.**

Allocation-lean scalar casts — booleans, the full integer family, reals, UUIDs, temporals —
calling directly into the native `libhypercast` Rust core. Ruby 3.2 is the floor
(`Data.define` for the verdict case types). The fast path links the core straight into a
Ruby extension (Magnus): on require it redefines the doors in place on the `HyperCast`
module — no delegation layer, no second surface, which is exactly what keeps the backends
provably in agreement. `HyperCast::BACKEND` reports which is live; `HYPERCAST_PURE=1`
forces Fiddle.

```ruby
case HyperCast.i32("(1,234)", HyperCast::NumFormat::INVARIANT)
in HyperCast::Success(value:) then puts "got #{value}"          # -1234
in HyperCast::Fault(reason:, offset:) then puts "#{reason} at byte #{offset}"
end
```

Door names mirror the native ABI (`i32`, `f64`, `timestamp`, …). Ruby-flavored fidelity,
stated proudly — nothing the core parses is lost on the way out: `Integer` is
unbounded (u64 comes back as the true unsigned value), `Time` carries full nanoseconds
across the whole 0001–9999 window, time-of-day is an exact Integer of nanoseconds since
midnight, and durations come back as exact `Rational` seconds across the core's whole
±10,000-year window — no truncation anywhere, no wrapping.

## Why not `Integer()` / `Time.iso8601` / `Float()`?

1. **Verdicts, not exceptions** — bad data is the expected case for untrusted text; a
   `Fault` is a Symbol and two integers, not an `ArgumentError` to rescue.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, radix prefixes, all five .NET `Guid` text forms plus
   `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (24 examples green on *both* backends, full twelve-file corpus
   replay; a cross-backend agreement spec compares Magnus and Fiddle outputs across a
   subprocess boundary).
4. **Faster than the stdlib on the Magnus backend, where the carrier is cheap** —
   benchmark-ips (`ruby benchmark/cast_benchmark.rb`, linux-arm64): timestamp **713 ns vs
   2.88 µs `Time.iso8601`** (4.0x) — while returning exact `Rational` durations on the
   duration door. The Fiddle fallback lands at ~3.5 µs: parity with `Time.iso8601`, sitting
   on Fiddle's measured 1.6 µs per-call marshalling floor.

   Separator detection is free here: `1.234.567,89` under `NumFormat::DETECT` runs
   1.073M i/s against 1.092M i/s for the same text under a declared eurozone format —
   inside the error bars.

**The honest trade-off, and Ruby's one real loss:** the civil date-time door is *slower*
than `strptime` — 1.30 µs against `DateTime.strptime`'s 1.02 µs, and the date door 1.04 µs
against `Date.strptime`'s 619 ns. The parse isn't the problem; the carrier is. Building a
stdlib `DateTime` with an exact `Rational` second costs more than the whole native call,
where the timestamp door's `Time` is built by a single cheap `rb_time_nano_new`. Printed
because it's real: if you want Ruby's fastest civil parse and don't need the verdict or the
declared order, `strptime` wins. Also note the carrier's other caveat — `DateTime`'s offset
defaults to `+00:00`, which is an artifact of the type, not a zone the parse assigned.

On the Fiddle fallback the doors are parity-at-best — Fiddle's
per-call floor is the mechanism's price, kept because it's the universal zero-compile
path. (Benchmark forensics worth knowing: the doors read 4.3 µs until per-call
`Fiddle::Pointer.malloc` finalizers were hoisted to thread-local scratch — receipts
include their own archaeology.)

## Install

```sh
gem install hypercast
```

Six gems are published per release: one universal `ruby`-platform gem (pure Fiddle, all six
platforms' natives bundled) plus five precompiled Magnus platform gems (`x86_64-linux`,
`aarch64-linux`, `x86_64-darwin`, `arm64-darwin`, `x64-mingw-ucrt`) that `gem install`
auto-selects when they match. Nobody ever compiles anything.

Selection has **two** axes here, unlike every other binding in this repo. A Magnus extension
is bound to one Ruby minor ABI — there is no `abi3` equivalent to collapse the version axis
the way [the Python binding's](../python/) wheels do — so each platform gem is a "fat" gem
carrying one compiled extension per supported Ruby, under `lib/hypercast/<minor>/`, and picks
one at `require` time:

| Ruby | the five platforms above | anywhere else (musl/Alpine, win-arm64, …) |
| --- | --- | --- |
| 4.0 (primary) | Magnus, `backend: :native` | Fiddle |
| 3.4 (floor, until its EOL 2028-03-31) | Magnus, `backend: :native` | Fiddle |
| 3.2 / 3.3 | Fiddle | Fiddle |

The platform gems declare `required_ruby_version >= 3.4, < 4.1` precisely so RubyGems
*declines* them outside that range and resolves the universal gem instead — a wrong-ABI
extension must never be installed in the first place. On Windows it would at least fail to
load cleanly (the extension imports `x64-ucrt-ruby<minor>.dll` by name), but Linux extensions
don't link libruby at all, so one can load successfully against the wrong ABI and misbehave
later. Every cell in that table replays the same corpus green.

Windows-on-x64 gained a Magnus gem once it turned out the long-standing "Magnus doesn't target
RubyInstaller's MinGW rubies" reasoning was backwards: MinGW is the *only* Windows flavour
`rb-sys` targets (`x64-mingw-ucrt` → `x86_64-pc-windows-gnu`, `supported: true` in its own
`data/toolchains.json`); the one it has no support for is `x86_64-pc-windows-msvc`. Windows is
also where the Fiddle fallback cost the most, so it was the most worthwhile gem in the set.
Windows-on-ARM stays on Fiddle for now — RubyInstaller ships `aarch64-mingw-ucrt` and `rb-sys`
lists it supported, but it hasn't been built or tested here yet.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.
