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
//! - [`cast_f32`] / [`cast_f64`] — finite reals only, percent notation included
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
//!
//! Text comes in as anything byte-viewable — `&str`, `String`, `&[u8]`, `Vec<u8>` — read
//! as UTF-8 bytes; each door trims ASCII whitespace and treats trimmed-empty input as
//! [`Reason::Empty`], which [`optional`] (and every binding's optional door) surfaces as
//! absent. [`Fault`] implements [`core::error::Error`], so a verdict composes with `?`
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

// The one symbol a no_std shared library needs that a static library does not. `core` and
// `compiler_builtins` ship precompiled with unwind tables, and an entry that names
// `rust_eh_personality` survives into the library (on linux-x64, that of the 128-bit
// division intrinsic). In a static library the reference is left for the final link to
// resolve; a shared library is the final link, so without a definition every dlopen fails
// with "undefined symbol: rust_eh_personality". With `panic = "abort"` nothing ever
// unwinds, so nothing ever calls it — the definition only has to exist.
//
// Assembly rather than a `#[no_mangle]` function, for the visibility: a `#[no_mangle]`
// item is exported from a cdylib whatever its Rust visibility, which would put a 23rd
// symbol beside the 26 C ABI exports, one any other library in the process could bind to.
// Defined hidden instead, it satisfies the library's own reference and is seen by nothing
// outside it. Never compiled for the static libraries: two Hyper* archives that each
// defined it could not be linked into one program, the duplicate-symbol failure that kept
// std out of them in the first place. Windows needs no definition (its unwind tables name
// the C runtime's handler), and wasm32 has no unwind tables to name one.
#[cfg(all(feature = "cdylib", not(feature = "std"), target_vendor = "apple"))]
core::arch::global_asm!(
    ".globl _rust_eh_personality",
    ".private_extern _rust_eh_personality",
    "_rust_eh_personality:",
    "ret",
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
    "ret",
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
mod decimal;
#[cfg(feature = "exports")]
mod ffi;
mod float;
mod float_table;
mod integer;
mod lane;
mod real;
mod temporal;
mod typed;
mod uuid;
mod verdict;

#[cfg(feature = "php")]
mod php_ext;
#[cfg(feature = "python-values")]
pub mod python;
#[cfg(feature = "python")]
mod python_ext;
#[cfg(feature = "ruby")]
mod ruby_ext;

pub use abi::{RawDayPeriods, RawNumFormat, hypercast_version};
pub use boolean::cast_bool;
pub use character::cast_char;
pub use decimal::cast_decimal;
pub use integer::{cast_i8, cast_i16, cast_i32, cast_i64, cast_u8, cast_u16, cast_u32, cast_u64};
pub use real::{cast_f32, cast_f64};
pub use temporal::{
    DateOrder, DayPeriods, ExcelEpoch, MAX_DURATION_SECONDS, MAX_TIMESTAMP_SECONDS,
    MIN_TIMESTAMP_SECONDS, UnixPrecision, cast_date, cast_date_ordered, cast_datetime,
    cast_datetime_with, cast_duration, cast_excel_serial, cast_time, cast_timestamp, cast_unix,
    excel_duration, excel_serial, excel_time, unix_from_f64,
};
pub use typed::{
    ShortestDigits, bool_from_f64, decimal_from_f64, f32_from_f64, i8_from_f64, i16_from_f64,
    i32_from_f64, i64_from_f64, shortest_digits, u8_from_f64, u16_from_f64, u32_from_f64,
    u64_from_f64,
};
pub use uuid::cast_uuid;
pub use verdict::{
    CivilDateTime, CurrencySymbol, Date, Decimal, Duration, Fault, NativeDigits, NumFormat, Reason,
    Symbol, Timestamp,
};

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
