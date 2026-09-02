# hypercast

[![CI](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml/badge.svg)](https://github.com/SkunkWerkx/HyperCast/actions/workflows/ci.yml)
[![Packagist](https://img.shields.io/packagist/v/skunkwerkx/hypercast.svg)](https://packagist.org/packages/skunkwerkx/hypercast)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/SkunkWerkx/HyperCast/blob/master/LICENSE)

**A real `Success|Fault` union type on every door — the value, or a closed backed-enum
reason plus the exact byte span that offended — over PHP's own built-in ext-ffi. Zero
Composer runtime dependencies, no extension to compile, no runtime bridge.**

Allocation-lean scalar casts — booleans, the full integer family, reals, UUIDs, temporals —
calling directly into the native `libhypercast` Rust core. PHP 8.1 is the floor (readonly
classes, enums); both verdict classes are `final` and every door's return type declares
the union, which is as closed as PHP's type system can state it.

```php
use HyperCast\{Cast, NumFormat, Success, Fault};

$verdict = Cast::i32('(1,234)', NumFormat::invariant());
echo match (true) {
    $verdict instanceof Success => "got {$verdict->value}",          // -1234
    $verdict instanceof Fault => "{$verdict->reason->name} at byte {$verdict->offset}",
};
```

Door names mirror the native ABI (`i32`, `f64`, `timestamp`, …); PHP strings are raw
bytes, so inputs cross verbatim and fault offsets need no mapping. PHP-flavored fidelity,
stated honestly: `int` is 64-bit signed, so u64 carries the two's-complement bit pattern
(render with `sprintf('%u', ...)`); `DateTimeImmutable` tops out at microseconds, so the
core's nanoseconds truncate by three digits; durations come back as the protobuf pair
(`Duration`) because `DateInterval` can't carry them.

## Why not `filter_var` / `DateTimeImmutable::createFromFormat`?

1. **Verdicts with location** — `filter_var` hands back `false` (indistinguishable from a
   parsed `false`, famously); a `Fault` is a reason and a span.
2. **The vocabulary untrusted sources actually send** — twenty boolean lexemes, accounting
   parentheses, declared separators, radix prefixes, all five .NET `Guid` text forms plus
   `urn:uuid:` prefixes, protobuf JSON durations.
3. **One engine across a polyglot system** — bit-for-bit verdicts with every other binding,
   held by the shared corpus (24 tests green, full twelve-file corpus replay with byte-exact
   fault spans).
4. **Faster than the platform's own parser** — phpbench
   (`XDEBUG_MODE=off vendor/bin/phpbench run --report=aggregate`, linux-arm64): timestamp
   **487 ns vs 1.3 µs `new DateTimeImmutable`** (2.7x). No new mechanism was needed for
   that — PHP's raw ext-ffi call floor is ~105 ns, already extension-class, so the win was
   a wrapper diet: flat doors (one FFI call, no closure indirection), typed cdef structs
   read as fields, static scratch `CData` with pre-taken addresses (PHP's request model
   makes static scratch safe), and `createFromTimestamp`/`setMicrosecond` on PHP 8.4+
   instead of a date-string parse. The messy civil shape lands the same way: `Cast::datetime`
   on `1/7/2026 3:04 PM` is **620 ns vs 1.36 µs** for `DateTimeImmutable::createFromFormat`
   with the equivalent pattern (2.2x), and the declared-order date door is 539 ns.
   Separator detection costs ~22 ns (385 ns vs 363 ns declared).

   When bytes are the destination — a `BINARY(16)` column bind, a wire format —
   `Cast::uuidBytes` returns the sixteen RFC-ordered octets as a binary string and skips
   the hex encoding and hyphen assembly `Cast::uuid` does to render the canonical form.
   The eight integer doors are now written out flat like the real doors — one literal FFI
   call each, no shared helper doing a dynamic symbol lookup and a string match to find
   the width's sign-extension shift.

**Why no native extension, when Python and Ruby got one:** the ~105 ns ext-ffi floor
above is already extension-class, so there is no mechanism tax left for a Zend extension
to remove. That reasoning is kept checkable rather than asserted: the core crate carries
an `ext-php-rs` build behind its `php` cargo feature (`rust/src/php_ext.rs`), the same
benchmark-only spike HyperUuid carries, exposing every door at the raw layer this package's
own FFI calls sit at. CI builds and load-checks it on every darwin/linux leg so it cannot
bit-rot; nothing in this Composer package loads it, and no phpunit runs against it.

**The honest trade-off:** a native library shipped inside the package and an FFI call per
door — for plain invariant integers, `(int)` casts and `ctype_digit` are the reasonable
choice. (Benchmark forensics worth knowing: PHP read 20x slow until a loaded Xdebug was
caught inflating everything uniformly ~14x — `XDEBUG_MODE=off` for every recorded number.)

## Verifying provenance

Packagist has nothing of its own to attest — there's no packed artifact, just a git tag it
resolves against this repo. What's actually worth checking is the native binaries
`stage-native-binaries.yml` committed into `php/src/native/`, each individually signed
by `hyper-build-native.yml` when it was built — that workflow physically lives in
`SkunkWerkx/.github`, so verifying needs `--signer-repo` alongside `--repo`, or `gh` reports
a bare `verifying with issuer "sigstore.dev"` that reads like a bad signature but is only an
identity mismatch:

```sh
composer require skunkwerkx/hypercast:X.Y.Z
gh attestation verify vendor/skunkwerkx/hypercast/src/native/linux-x64/libhypercast.so \
  --repo SkunkWerkx/HyperCast --signer-repo SkunkWerkx/.github
```

The staging commit's own message records the exact `ci.yml` run ID and source SHA the
binary came from (`chore: stage native binaries from ci.yml run <id>`), so you can
cross-check the attested commit against that message directly. See
[csharp/README.md's provenance section](../csharp/README.md#native-binary-provenance) for
more on why `--signer-repo` is needed for some artifacts here and not others.

## Install

```sh
composer require skunkwerkx/hypercast
```

Packagist has no packing step — the git tree at the tag *is* the package. That is why the six
per-RID native libraries under `src/native/` are committed to git (kept fresh automatically
by `stage-native-binaries.yml`; see `src/native/README.md`), and why the repository root
carries the `composer.json` Packagist requires, since Packagist has no monorepo-subdirectory
support.

See [the repo root README](../README.md) for the full door table, the receipts, and the
state of every other language binding.
