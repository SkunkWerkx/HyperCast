# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)

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
stated proudly: this is one of the two fidelity kings of the roster — `Integer` is
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
   held by the shared corpus (19 examples green on *both* backends, full nine-file corpus
   replay; a cross-backend agreement spec compares Magnus and Fiddle outputs across a
   subprocess boundary).
4. **Faster than the stdlib on the Magnus backend** — benchmark-ips
   (`ruby benchmark/cast_benchmark.rb`, linux-arm64): timestamp **748 ns vs 3.2 µs
   `Time.iso8601`** (4.3x) — while returning exact `Rational` durations on the duration
   door. The Fiddle fallback lands at ~3.5 µs: parity with `Time.iso8601`, sitting on
   Fiddle's measured 1.6 µs per-call marshalling floor.

**The honest trade-off:** on the Fiddle fallback the doors are parity-at-best — Fiddle's
per-call floor is the mechanism's price, kept because it's the universal zero-compile
path. (Benchmark forensics worth knowing: the doors read 4.3 µs until per-call
`Fiddle::Pointer.malloc` finalizers were hoisted to thread-local scratch — receipts
include their own archaeology.)

## Install

Not on RubyGems yet — the release pipeline is staged (`.github/workflows/release.yml`,
Trusted Publishing pending): one universal `ruby`-platform gem (pure Fiddle, all six
platforms' natives bundled) plus four precompiled Magnus platform gems
(`x86_64-linux`, `aarch64-linux`, `x86_64-darwin`, `arm64-darwin`) that `gem install`
auto-selects when they match — nobody ever compiles anything. Until the first tag: clone
the repo, `cargo build --release` in `rust/` (plus `--features ruby`, staged to
`lib/hypercast_native.so`, for the Magnus backend), and `bundle exec rspec`.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.
