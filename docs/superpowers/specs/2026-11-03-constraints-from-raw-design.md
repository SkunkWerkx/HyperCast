# Constraints to and from the C ABI form (HyperCast 1.1.0)

Date: 2026-11-03 · Size: a small change · Scope: the Rust crate, HyperCast's `ci.yml`, and
one opt-in input in the Forge's `hyper-core-checks.yml`

## Intent

HyperTabular judges each cell of a constrained column with HyperCast's public checked doors
(`cast_checked::<T>`, `checked_from_f64::<T>`, `cast_text_checked`), so that every cast and
every judgement happens in HyperCast and HyperTabular only stores the declaration per column.

Declarations cross HyperTabular's own C ABI in HyperCast's ABI form: a `RawConstraints` and a
packed `enum` buffer (16-byte `Decimal`s in native layout for an integer `enum`; a
little-endian `u32` length then the bytes for each text entry). Two conversions are missing
from HyperCast's public Rust API:

1. **Into a `Constraints<T>`.** HyperTabular's core receives the ABI form from every binding
   and has no public way to build the `Constraints<'e, T>` the checked doors take. Only the
   private `Entries::packed` reads a packed list.
2. **Out of a `Constraints<T>`.** HyperTabular's Rust binding takes the caller's
   `Constraints<'e, T>` (built with `Constraints::numeric` or `Constraints::text`) and must
   hand the core the ABI form. `raw()` gives the struct, but a list declared as `&[i128]` or
   `&[&str]` cannot be packed from outside: `Entries` is private.

## Goals

- Both conversions, public, `no_std`, allocation-free and panic-free, in the crate's idiom.
- The one judge of a declaration stays `check_constraints`: a `Constraints` built from the ABI
  form is refused exactly where `cast_constraints_check` refuses, duplicates included, so a
  value of the type is still always a valid declaration.
- For every `Constraints` whose `pack_enum` answers `Some` (see Representability), the round
  trip (`Constraints` → ABI form → `Constraints`) is accepted and judges every input
  identically.

## Non-goals

- No change to the C ABI, the export count (31) or any other binding: every other binding
  already holds the ABI form, packs its own list, and calls the C exports.
- No public door-dispatched judging (`judge_numeric` / `judge_text` stay `pub(crate)`); see
  Approaches.
- No change to the per-call re-check the checked doors run (below, "Measured later").

## Approaches

| | What | Trade-off |
| --- | --- | --- |
| **A (chosen)** | A public constructor from the ABI form and a public packer; HyperTabular calls the existing typed checked doors, one call per door, mirroring its plain-door dispatch. | Typed and safe; one constructor and one packer of new API. A `Constraints` borrows its `enum` bytes, so HyperTabular rebuilds it per fill call (one batch), re-running the duplicate pass. |
| B | Make `judge_numeric` / `judge_text` public. | Fewer lines in HyperTabular, but `unsafe`, writes through a raw pointer sized by a runtime door, and freezes `CheckOutcome` and `Entries`, plumbing designed as private, as public API for the 1.x line. |

The owner chose A.

## Design

```rust
impl<'e, T: Numeric> Constraints<'e, T> {
    /// Reads a declaration in its C ABI form for `T`'s door: `raw` and the packed integer
    /// `enum` list (empty for none). Refused where `check_constraints` refuses.
    pub fn from_raw(raw: RawConstraints, entries: &'e [u8]) -> Result<Self, Refusal>;
}

impl<'e> Constraints<'e, Text> {
    /// The same for the text door, with the packed text `enum` list.
    pub fn from_raw(raw: RawConstraints, entries: &'e [u8]) -> Result<Self, Refusal>;
}

impl<'e, T> Constraints<'e, T> {
    /// Packs the `enum` list as the C ABI passes it into `out`, and returns the bytes the
    /// packed list takes (`Some(0)` for no `enum`). When `out` is shorter, nothing is
    /// written and the length is still returned, so a caller sizes its buffer and calls
    /// again. `None` when the packed list would take more than `usize::MAX` bytes, which
    /// only a borrowed text list on a 32-bit target can reach; nothing is written then.
    pub fn pack_enum(&self, out: &mut [u8]) -> Option<usize>;
}
```

- `from_raw` is `Self::checked(door, raw, Entries::packed(door, entries))`: the existing
  private constructor every builder already ends in, so there is no second judge.
- `pack_enum` writes `Integers` as `Decimal`s (scale 0) in the ABI's native layout and
  `Text` as length-prefixed UTF-8; a `Constraints` already built from a packed list copies
  its bytes as they are.
- Both are additive and sit in `constraints.rs`. Text and the numeric types are disjoint
  (`Numeric` is sealed), so the two `from_raw`s do not overlap.

### Representability

What one entry of the packed form cannot carry is refused when the declaration is made, by
the shared checker. A total the packer cannot size is answered `None`, its arithmetic
checked. The round trip is promised only where `pack_enum` answers `Some`:

- **An entry's length.** A packed text entry's length is a `u32`. The shared checker
  (`check`, and so `check_constraints`, `Constraints::text` and `from_raw`) refuses a text
  `enum` entry longer than `u32::MAX` bytes with `Refusal::NotHeldByDoor`, whose doc gains
  the case. Only a Rust-built list can reach it (a packed entry's prefix cannot say more),
  so the C ABI's refusals don't move. It is also no loss: a judged input past `u32::MAX`
  bytes is already `OutOfRange` (a span cannot carry it), so no input could ever have matched
  such an entry.
- **The total.** `pack_enum` sums `4 + len` per text entry and `16` per integer entry with
  checked arithmetic, and answers `None` on overflow rather than wrapping. Only a borrowed
  text list on a 32-bit target can overflow: a list declared with `&[i128]` lives in memory,
  so `16 × len` bytes of packing are at most what it holds. The bound isn't moved into the
  checker, because the C ABI's packed lists already exist in memory and never overflow, so
  the packer is the only place that can.
- **The round trip holds for every `Constraints` `pack_enum` answers `Some` for:**
  `from_raw(c.raw(), packed)` is accepted and judges every input identically.

## Proof

- `corpus/constraints.json`: every declaration vector is also built through `from_raw` from
  its ABI form. Each refusal vector is refused with the same `Refusal`, and each accepted one
  judges every input identically to the builder-made `Constraints`.
- A round-trip test: for each accepted vector, builder → `raw()` + `pack_enum` → `from_raw`
  delivers the same verdict for every input, and `pack_enum`'s bytes pass
  `check_constraints`.
- `pack_enum` with a short buffer writes nothing and returns the full length, at every length
  from 0 up, for each of the four list forms (`Integers`, `Text`, `PackedIntegers`,
  `PackedText`) and for no `enum`.
- Representability: a text entry of `u32::MAX + 1` bytes is refused `NotHeldByDoor` by
  `Constraints::text` and `check_constraints`, and one of exactly `u32::MAX` bytes is not.
  Both are built without allocating 4 GiB by borrowing one zero-page mapping, or, where the
  target can't, the test is `#[ignore]`d with the reason. The total's overflow is tested on
  wasm32 under wasmtime (the suite already runs there) with a borrowed list repeating one
  large entry until the sum passes `usize::MAX`, expecting `None` and nothing written.
- `allocation_free.rs` counts zero for both.
- Negative control for the refusals: dropping the duplicate pass from `from_raw` must fail
  the refusal replay.

### No panic

`cargo no-panic` only checks what is marked `#[no_panic]` and linked into the cdylib, which
today is the C exports alone. The new methods are generic and no export calls them, so they
would never be instantiated in that build or checked. So:

- **A harness module**, `rust/src/no_panic_harness.rs`, compiled only under the `no-panic`
  feature (never in a shipped build, so the export list and the Forge's symbol check are
  untouched). It holds non-generic `#[unsafe(no_mangle)]` functions, each `#[no_panic]`,
  that take their inputs as arguments (raw pointers and lengths, so the optimizer can't fold
  any branch away):
  - `from_raw` for every `Numeric` type and for `Text`;
  - `pack_enum` for each of the four list forms and for no `enum`, with the output buffer's
    length an argument, so the short-buffer, exact and overflow branches are all linked;
  - `Constraints::numeric` and `Constraints::text` building `Integers` and `Text` lists
    from argument slices, so the packer's two Rust-list arms are reached through the same
    path a Rust caller takes.
- **A canary**, as HyperTabular has: under `--cfg hypercast_canary` the harness gains one
  more `#[no_panic]` function holding an `assert!`. `cargo no-panic` with that cfg must fail,
  naming only the canary. That is the negative control: it shows the harness is linked and
  checked, not passing vacuously.
- **Where it runs:** in CI, on every core-checks run (below), and locally in a Linux
  container before the push (the Mac build passes vacuously, `CLAUDE.md`).

### The canary in CI (a Forge change)

The owner's decision: the canary is a CI step, since a local negative control proves only
today's setup, while CI catches a later change that silently stops the harness being checked.
It is opt-in per crate, so HyperCast turns it on without HyperUuid having to adopt a canary.

- **A new input to `hyper-core-checks.yml`:** `no_panic_canary`, a string, default `""`
  (off). When set, it names the canary function, and the same name is the `--cfg` that
  compiles it in (HyperCast: `hypercast_canary`). HyperCast's `ci.yml` sets it in its
  `core-checks` `with:` block. HyperUuid passes nothing and runs as today.
- **The step**, after the existing no-panic step and on the same runners
  (`no_panic_runners`), when `inputs.no_panic && inputs.no_panic_canary != ''`:
  1. The normal no-panic build has already passed: the existing step, unchanged. If it
     fails, the job stops there.
  2. Run `no_panic_command` again with `RUSTFLAGS="--cfg <canary>"`, capturing its output.
     **It must fail.** Success fails CI ("a build with a panicking canary passed the
     proof").
  3. **The output must name the canary**: `detected panic in function \`<canary>\``.
     Without that line, the build failed for another reason, and CI fails, printing the tail
     of the log.
  4. **The output must name nothing else**: any other `detected panic in function` line
     fails CI, naming those functions.
  This is HyperTabular's `check-core.sh` canary check moved into the Forge, with the
  function name as the input.
- **`RUSTFLAGS` replaces the config's rustflags.** On the Linux GNU runner that
  `no_panic_runners` names for HyperCast, `rust/.cargo/config.toml` sets none, so nothing
  is lost. The input's description says that the canary build runs with only that cfg, so a
  crate whose no-panic target needs config rustflags knows to fold them into its command.
- **Contract tests:** `test_hyper_core_checks.py` in the Forge gains the input, its
  default-off shape, and the step's three verdicts. The step's grep logic sits in a small
  Forge script (`no_panic_canary.py`, taking the log and the name) so that each verdict
  (passed, failed for another reason, extra function named) has a unit test with a recorded
  log as fixture.
- **HyperTabular** keeps its own canary in `check-core.sh` for now. Moving it onto the input
  is a later cleanup, not part of this change.
- **Ordering:** the Forge change merges first (it's a no-op for every crate until one sets
  the input), then HyperCast sets `no_panic_canary: hypercast_canary` in the same pull
  request as the harness.

## Docs

- `CHANGELOG.md`: the 1.1.0 constraints entry gains one line for the two methods
  *(Rust)*.
- `rust/README.md` gains a constraints section (the review found it barely mentions the
  checked doors), its example verified by hand.

## Measured later

The checked doors re-run every rule but the duplicate pass on each call, even for a
`Constraints` that was validated when it was built. HyperTabular's million-row harness with a
constrained column will show whether that costs anything. If it does, HyperCast can skip the
re-check for a value of the type (which is always valid) without any API change.

## Left to the builder

Test layout, doc-comment wording, the harness functions' names, and where `pack_enum`'s
writer lives inside the crate.
