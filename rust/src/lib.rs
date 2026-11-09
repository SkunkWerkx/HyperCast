//! Allocation-free parsers for scalars from untrusted text — booleans, characters, numerics, UUIDs,
//! and temporals. Every cast returns a verdict: the value, or a [`Fault`] carrying a closed
//! [`Reason`] and the offending byte span. Never panics on bad input, never allocates —
//! semantics ported from Svartalfheim's `Norse.Primitives` parser family, mechanics from
//! this project's own HyperUuid (one Rust `cdylib`, every host binding calls straight in).
//!
//! - [`cast_bool`] — the natural-language boolean lexicon
//! - [`cast_char`] — one Unicode scalar, verbatim or as a declared code point (`U+00E9`,
//!   `&#233;`)
//! - [`cast_i8`]…[`cast_u64`] — the integer family under a caller-declared [`NumFormat`]
//! - [`cast_f32`] / [`cast_f64`] — finite reals only, percent notation included, and never a
//!   negative zero
//! - [`cast_decimal`] — the same grammar to an exact [`Decimal`] (sign, 96-bit magnitude,
//!   base-10 scale), never rounded
//! - [`cast_uuid`] — every .NET `Guid` text format plus `urn:uuid:`-style prefixes,
//!   16 RFC 9562-ordered bytes out
//! - [`cast_timestamp`] / [`cast_unix`] — instants to protobuf's `{seconds, nanos}` pair
//! - [`cast_excel_serial`] — spreadsheet date serials under a declared [`ExcelEpoch`],
//!   phantom `1900-02-29` and all
//! - [`excel_serial`] — the same serials already held as the `f64` a workbook stores,
//!   under the same rules, to a zone-less [`CivilDateTime`]; [`excel_time`] and
//!   [`excel_duration`] read the same `f64` as a time of day and as a span of days
//! - [`decimal_from_f64`], [`i32_from_f64`] and their kin — the typed twins of the numeric
//!   doors, for a number a workbook already stores as a double. They read its shortest
//!   decimal representation ([`shortest_digits`]); integer doors require that number
//!   to be whole and in range, and can differ from the exact binary integer above 2^53
//! - [`cast_date`] / [`cast_time`] / [`cast_duration`] — the remaining temporal shapes,
//!   likewise protobuf-formed
//! - [`cast_date_ordered`] — separated calendar dates under a caller-declared [`DateOrder`]
//!   (`1/7/2026` is January 7th or July 1st only because the caller said which)
//! - [`cast_datetime`] — zone-less civil date-times under a declared [`DateOrder`]
//!   (`1/7/2026 3:04 PM`, `2026-01-07 15:04:05`) to a [`CivilDateTime`]; no zone is read
//!   and none is invented
//! - [`Door`] — the numbering of every text door above, [`cast_char`]'s included (plus
//!   `Text`, the bytes themselves), so a layer that binds something to a door can name one,
//!   and the codes the detailed C exports dispatch on
//! - [`Constraints`] — JSON Schema's validation keywords (`minimum`, `multipleOf` with a
//!   [`Rounding`], `maxLength`, `enum`, …) declared for a door, refused at declaration with a
//!   [`Refusal`] when they cannot mean anything there ([`check_constraints`] is that judge)
//! - [`cast_checked`] / [`checked_from_f64`] / [`cast_text_checked`] — a door, then the
//!   [`Constraints`] declared for it, judged exactly (decimal arithmetic, never a float, and
//!   a float door's delivered value held to the bounds too, each read as the door reads it):
//!   [`Checked`] (the value, whether rounding `adjusted` it, a text `enum`'s index) or a
//!   [`Rejection`], the door's own [`Fault`] or a [`Violation`] of one [`Keyword`]
//! - [`cast_text`] — text as a door: valid UTF-8, untrimmed, empty as [`Reason::Empty`]
//! - [`detailed`] — every door again, failing with a [`Failure`]: the same reason and span,
//!   and the [`Detail`] of what failed; its numeric doors take a [`ValidNumFormat`]
//!
//! Text comes in as anything byte-viewable — `&str`, `String`, `&[u8]`, `Vec<u8>` — read
//! as UTF-8 bytes; each door trims Unicode white space ([`is_white_space`]) and treats
//! trimmed-empty input as [`Reason::Empty`], which [`optional`] (and every binding's
//! optional door) surfaces as absent. [`Fault`] implements [`core::error::Error`], so a verdict composes with `?`
//! and ordinary error chains when propagate-on-failure is the caller's idiom.

#![deny(missing_docs)]
// The crate publishes the `no-std` category, and the core has always been core-only in
// fact — no `std::`, `String`, `Vec`, `Box`, or `format!` in any parsing module. This makes
// that compiler-enforced rather than a claim nobody checks.
//
// Gated on the default-on `std` feature rather than unconditional, because the extension
// modules (python, ruby, php) need std through their own dependencies and the test harness
// needs it to run at all. Every artifact this crate ships itself is built without it: the
// static libraries (`cargo staticlib`) and the shared library every binding dlopens
// (`cargo cdylib`), which bring the panic handler below in std's place. A no_std rlib
// consumer takes the crate with `default-features = false` and brings a handler of their
// own, the way such a consumer must anyway. Nothing in the parsing modules ever touches std
// either way.
#![cfg_attr(not(feature = "std"), no_std)]
// The PHP extension's handlers are `extern "vectorcall"` on Windows (ext-php-rs's
// `zend_fastcall!`, which every `#[php_impl]` method and every hand-registered value-class
// method expand to), and that ABI is unstable (rust-lang/rust#124485); the Forge's Windows
// extension build is the one step that sets RUSTC_BOOTSTRAP=1 for it. Every other build
// leaves it out.
#![cfg_attr(all(windows, feature = "php"), feature(abi_vectorcall))]

// The panic handler for the no_std artifacts this crate links itself: the static libraries
// behind the `staticlib` feature and the shared library behind `cdylib` (Cargo.toml has
// what each is and why neither carries std). Built with `panic = "abort"`, so a panic ends
// the program instead of unwinding into the host: on wasm32 it is the `unreachable` trap,
// which the host sees as a RuntimeError rather than as a corrupted return value; anywhere
// else it is the C library's `abort`, which every process the library is loaded or linked
// into already has. Never compiled for a bare-metal rlib consumer, who brings a handler of
// their own, nor with `std`, which has one.
#[cfg(all(feature = "staticlib", not(feature = "std")))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    #[cfg(target_arch = "wasm32")]
    core::arch::wasm32::unreachable();
    #[cfg(not(target_arch = "wasm32"))]
    {
        unsafe extern "C" {
            safe fn abort() -> !;
        }
        abort()
    }
}

// The return instruction that ends the `rust_eh_personality` stubs below, spelled the way
// each architecture's assembler takes it: `ret` is no instruction on s390x, PowerPC or 32-bit
// Arm. Nothing ever calls a stub, so the instruction only has to assemble. Every other
// architecture whose inline assembly is stable (x86, Arm64, RISC-V, LoongArch) takes `ret`;
// on one whose assembly is not (SPARC, MIPS) stable rustc refuses the stub, so the no_std
// libraries do not build there at all rather than build without it. Defined only where a
// stub is compiled.
#[cfg(all(
    any(feature = "cdylib", feature = "staticlib"),
    not(feature = "std"),
    not(target_os = "windows"),
    not(target_arch = "wasm32"),
    target_arch = "s390x",
))]
macro_rules! personality_return {
    () => {
        "br %r14"
    };
}
#[cfg(all(
    any(feature = "cdylib", feature = "staticlib"),
    not(feature = "std"),
    not(target_os = "windows"),
    not(target_arch = "wasm32"),
    any(target_arch = "powerpc", target_arch = "powerpc64"),
))]
macro_rules! personality_return {
    () => {
        "blr"
    };
}
#[cfg(all(
    any(feature = "cdylib", feature = "staticlib"),
    not(feature = "std"),
    not(target_os = "windows"),
    not(target_arch = "wasm32"),
    target_arch = "arm",
))]
macro_rules! personality_return {
    () => {
        "bx lr"
    };
}
#[cfg(all(
    any(feature = "cdylib", feature = "staticlib"),
    not(feature = "std"),
    not(target_os = "windows"),
    not(target_arch = "wasm32"),
    not(any(
        target_arch = "s390x",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "arm"
    )),
))]
macro_rules! personality_return {
    () => {
        "ret"
    };
}

// The one symbol a no_std shared library needs that a static library does not. `core` and
// `compiler_builtins` ship precompiled with unwind tables, and an entry that names
// `rust_eh_personality` survives into the library (on linux-x64, that of the 128-bit
// division intrinsic). In a static library the reference is left for the final link to
// resolve; a shared library is the final link, so without a definition every dlopen fails
// with "undefined symbol: rust_eh_personality". With `panic = "abort"` nothing ever
// unwinds, so nothing ever calls it — the definition only has to exist.
//
// Assembly rather than a `#[no_mangle]` function, for the visibility: a `#[no_mangle]`
// item is exported from a cdylib whatever its Rust visibility, which would put a 33rd
// symbol beside the 36 C ABI exports, one any other library in the process could bind to.
// Defined hidden instead, it satisfies the library's own reference and is seen by nothing
// outside it. Never compiled for the static libraries: two Hyper* archives that each
// defined it could not be linked into one program, the duplicate-symbol failure that kept
// std out of them in the first place. Windows needs no definition (its unwind tables name
// the C runtime's handler), and wasm32 has no unwind tables to name one.
//
// The body is the architecture's own return instruction (`personality_return!`, above).
#[cfg(all(feature = "cdylib", not(feature = "std"), target_vendor = "apple"))]
core::arch::global_asm!(
    ".globl _rust_eh_personality",
    ".private_extern _rust_eh_personality",
    "_rust_eh_personality:",
    personality_return!(),
);
#[cfg(all(
    feature = "cdylib",
    not(feature = "std"),
    not(target_vendor = "apple"),
    not(target_os = "windows"),
    not(target_arch = "wasm32"),
))]
core::arch::global_asm!(
    ".globl rust_eh_personality",
    ".hidden rust_eh_personality",
    ".type rust_eh_personality, %function",
    "rust_eh_personality:",
    personality_return!(),
);

// The static library's half: the same reference, left for the final link, fails it
// ("undefined reference to `rust_eh_personality'") whenever the program pulls in an
// intrinsic whose unwind entry names it (the 128-bit division helpers) and the linker keeps
// unwind tables of unused code (no --gc-sections / -dead_strip, so a plain `cc app.c
// libhypercast.a`). A weak hidden definition resolves it, and two Hyper* archives (or a Rust
// program with std's own, strong, definition) linked into one program coalesce to one
// instead of colliding.
#[cfg(all(
    feature = "staticlib",
    not(feature = "cdylib"),
    not(feature = "std"),
    target_vendor = "apple"
))]
core::arch::global_asm!(
    ".globl _rust_eh_personality",
    ".weak_definition _rust_eh_personality",
    ".private_extern _rust_eh_personality",
    "_rust_eh_personality:",
    personality_return!(),
);
#[cfg(all(
    feature = "staticlib",
    not(feature = "cdylib"),
    not(feature = "std"),
    not(target_vendor = "apple"),
    not(target_os = "windows"),
    not(target_arch = "wasm32"),
))]
core::arch::global_asm!(
    ".weak rust_eh_personality",
    ".hidden rust_eh_personality",
    ".type rust_eh_personality, %function",
    "rust_eh_personality:",
    personality_return!(),
);

// The other half of the same gap: the C runtime. Under std it comes in through std's own
// link directives; a no_std shared library has to name it, or nothing does. On Linux the
// library then links with no NEEDED entry at all, its `abort`, `memcpy` and `memset`
// unversioned and resolved only because the host process happens to have libc loaded; on
// Windows nothing names the runtime its `memcpy`, `__CxxFrameHandler3` and DLL entry point
// come from. The choice mirrors the one std
// makes by way of the libc crate — libc on Unix, and on Windows the DLL import library
// ordinarily or the static runtime when crt-static asks for it — so the library depends on
// exactly the C runtime the std build did and nothing more.
#[cfg(all(feature = "cdylib", not(feature = "std"), unix))]
#[link(name = "c")]
unsafe extern "C" {}
#[cfg(all(feature = "cdylib", not(feature = "std"), target_env = "msvc"))]
#[cfg_attr(target_feature = "crt-static", link(name = "libcmt"))]
#[cfg_attr(not(target_feature = "crt-static"), link(name = "msvcrt"))]
unsafe extern "C" {}

mod abi;
mod boolean;
mod character;
mod checked;
mod constraints;
mod date_options;
mod decimal;
mod detail;
pub mod detailed;
mod door;
mod exact;
#[cfg(feature = "exports")]
mod ffi;
mod float;
mod float_table;
mod fold_table;
mod integer;
mod lane;
// Linked only into the no-panic proof's throwaway build, never a C ABI: kept out of the header.
/// cbindgen:ignore
#[cfg(feature = "no-panic")]
mod no_panic_harness;
mod real;
mod site;
mod temporal;
mod text;
mod typed;
mod utf8;
mod uuid;
mod verdict;

/// The PHP extension (`cargo php-ext`): the whole `HyperCast\\` API as a Zend extension.
/// Not public, and nothing in it is `extern "C"` toward C callers, so cbindgen has nothing
/// to emit from it.
///
/// cbindgen:ignore
#[cfg(feature = "php")]
mod php_ext;
#[cfg(feature = "python-values")]
pub mod python;
#[cfg(feature = "python")]
mod python_ext;
#[cfg(feature = "ruby")]
mod ruby_ext;

pub use abi::{
    RawCheck, RawConstraints, RawDateOptions, RawDayPeriods, RawFaultDetail, RawNumFormat, RawSpan,
};
pub use boolean::cast_bool;
pub use character::cast_char;
pub use checked::{
    Checked, Rejection, Violation, cast_checked, cast_text_checked, checked_from_f64,
    optional_checked,
};
pub use constraints::{
    Bound, Constraints, Keyword, Numeric, NumericRules, Refusal, Rounding, Text, TextRules,
    check_constraints, decimal_for_declaration,
};
pub use date_options::{DateOptions, MonthNames, MonthNamesFault, ValidDateOptions};
pub use decimal::cast_decimal;
pub use detail::{Detail, Failure};
pub use door::Door;
pub use integer::{cast_i8, cast_i16, cast_i32, cast_i64, cast_u8, cast_u16, cast_u32, cast_u64};
pub use real::{cast_f32, cast_f64};
pub use temporal::{
    DateOrder, DayPeriods, ExcelEpoch, MAX_DURATION_SECONDS, MAX_TIMESTAMP_SECONDS,
    MIN_TIMESTAMP_SECONDS, UnixPrecision, cast_date, cast_date_ordered, cast_datetime,
    cast_datetime_with, cast_duration, cast_excel_serial, cast_time, cast_timestamp, cast_unix,
    excel_duration, excel_serial, excel_time, unix_from_f64,
};
pub use text::{cast_text, is_white_space};
pub use typed::{
    ShortestDigits, bool_from_f64, decimal_from_f64, f32_from_f64, i8_from_f64, i16_from_f64,
    i32_from_f64, i64_from_f64, shortest_digits, u8_from_f64, u16_from_f64, u32_from_f64,
    u64_from_f64,
};
pub use uuid::cast_uuid;
pub use verdict::{
    CivilDateTime, CurrencySymbol, Date, Decimal, DeclarationError, Duration, Fault, Grouping,
    NativeDigits, NumFormat, Reason, Symbol, Timestamp, ValidNumFormat,
};

// `hypercast_version` is the C symbol itself when `exports` is on, as in 1.0.0, so the Rust
// function and the ABI name stay one item (cargo-semver-checks holds them together); with
// `exports` off it is the same function with no symbol, for a library that links this crate.
#[cfg(not(feature = "exports"))]
pub use abi::hypercast_version;
#[cfg(feature = "exports")]
pub use ffi::hypercast_version;

/// Presents a verdict optionally: an [`Reason::Empty`] fault becomes `Ok(None)` — Rust's
/// absent — and everything else flows through untouched. The same presentation helper
/// every binding ships (`optional`/`Optional`), for callers whose absent-is-fine doors
/// shouldn't treat missing input as a failure.
pub fn optional<T>(verdict: Result<T, Fault>) -> Result<Option<T>, Fault> {
    match verdict {
        Ok(value) => Ok(Some(value)),
        Err(Fault { reason: Reason::Empty, .. }) => Ok(None),
        Err(fault) => Err(fault),
    }
}

// The unit tests are in their own file, `tests.rs`: CodeQL's path filters are per file, and a
// test that formats a value into a panic message is not a finding worth carrying against the
// module that ships. See `.github/codeql/codeql-config.yml`.
#[cfg(test)]
mod tests;
