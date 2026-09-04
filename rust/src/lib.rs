//! Allocation-free parsers for scalars from untrusted text — booleans, numerics, UUIDs,
//! and temporals. Every cast returns a verdict: the value, or a [`Fault`] carrying a closed
//! [`Reason`] and the offending byte span. Never panics on bad input, never allocates —
//! semantics ported from Svartalfheim's `Norse.Primitives` parser family, mechanics from
//! this project's own HyperUuid (one Rust `cdylib`, every host binding calls straight in).
//!
//! - [`cast_bool`] — the natural-language boolean lexicon
//! - [`cast_i8`]…[`cast_u64`] — the integer family under a caller-declared [`NumFormat`]
//! - [`cast_f32`] / [`cast_f64`] — finite reals only, percent notation included
//! - [`cast_decimal`] — the same grammar to an exact [`Decimal`] (sign, 96-bit magnitude,
//!   base-10 scale), never rounded
//! - [`cast_uuid`] — every .NET `Guid` text format plus `urn:uuid:`-style prefixes,
//!   16 RFC 9562-ordered bytes out
//! - [`cast_timestamp`] / [`cast_unix`] — instants to protobuf's `{seconds, nanos}` pair
//! - [`cast_excel_serial`] — spreadsheet date serials under a declared [`ExcelEpoch`],
//!   phantom `1900-02-29` and all
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
// Gated on the default-on `std` feature rather than unconditional, because this crate also
// ships a `cdylib`: a final linked artifact needs a `#[panic_handler]`, which only std
// supplies (proven, not assumed — dropping `no_std` in unconditionally fails the release
// build with "`#[panic_handler]` function required, but not found" plus "unwinding panics
// are not supported without std"). So the shared library every binding dlopens builds with
// std as it always has, and a bare-metal consumer takes the crate with
// `default-features = false` and brings its own panic handler, the way such a consumer
// must anyway. Nothing in the parsing modules ever touches std either way.
#![cfg_attr(not(feature = "std"), no_std)]

mod boolean;
mod decimal;
mod ffi;
mod integer;
mod real;
mod temporal;
mod uuid;
mod verdict;

#[cfg(feature = "python")]
mod python_ext;
#[cfg(feature = "ruby")]
mod ruby_ext;
#[cfg(feature = "php")]
mod php_ext;

pub use boolean::cast_bool;
pub use decimal::cast_decimal;
pub use ffi::hypercast_version;
pub use integer::{cast_i8, cast_i16, cast_i32, cast_i64, cast_u8, cast_u16, cast_u32, cast_u64};
pub use real::{cast_f32, cast_f64};
pub use temporal::{
    cast_date, cast_date_ordered, cast_datetime, cast_duration, cast_excel_serial, cast_time,
    cast_timestamp, cast_unix, DateOrder, ExcelEpoch, UnixPrecision, MAX_DURATION_SECONDS,
    MAX_TIMESTAMP_SECONDS, MIN_TIMESTAMP_SECONDS,
};
pub use uuid::cast_uuid;
pub use verdict::{
    CivilDateTime, CurrencySymbol, Date, Decimal, Duration, Fault, NumFormat, Reason, Timestamp,
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

#[cfg(test)]
mod tests {
    use super::*;

    const INVARIANT: NumFormat = NumFormat::INVARIANT;

    fn reason<T: core::fmt::Debug>(verdict: Result<T, Fault>) -> Reason {
        verdict.unwrap_err().reason
    }

    // --- boolean ---

    #[test]
    fn bool_recognizes_the_full_lexicon_case_insensitively() {
        for text in ["true", "TRUE", "t", "yes", "Y", "1", "on", "enabled", "Active", "checked", "in"] {
            assert_eq!(cast_bool(text.as_bytes()), Ok(true), "{text}");
        }
        for text in ["false", "F", "no", "N", "0", "off", "Disabled", "inactive", "unchecked", "Out"] {
            assert_eq!(cast_bool(text.as_bytes()), Ok(false), "{text}");
        }
    }

    #[test]
    fn bool_trims_ascii_whitespace() {
        assert_eq!(cast_bool(b"\ttrue\n"), Ok(true));
        assert_eq!(cast_bool(b"  Y  "), Ok(true));
    }

    #[test]
    fn bool_rejects_unrecognized_and_empty() {
        assert_eq!(reason(cast_bool(b"maybe")), Reason::Malformed);
        assert_eq!(reason(cast_bool(b"truee")), Reason::Malformed);
        assert_eq!(reason(cast_bool(b"2")), Reason::Malformed);
        assert_eq!(reason(cast_bool(b"")), Reason::Empty);
        assert_eq!(reason(cast_bool(b" \t\r\n ")), Reason::Empty);
    }

    #[test]
    fn bool_fault_spans_the_trimmed_token() {
        let fault = cast_bool(b"  maybe  ").unwrap_err();
        assert_eq!((fault.offset, fault.len), (2, 5));
    }

    // --- integers ---

    #[test]
    fn int_parses_the_svartalfheim_notation_set() {
        assert_eq!(cast_i32(b"42", &INVARIANT), Ok(42));
        assert_eq!(cast_i32(b"  7  ", &INVARIANT), Ok(7));
        assert_eq!(cast_i32(b"+13", &INVARIANT), Ok(13));
        assert_eq!(cast_i32(b"-13", &INVARIANT), Ok(-13));
        assert_eq!(cast_i32(b"1,234", &INVARIANT), Ok(1234));
        assert_eq!(cast_i32(b"(1,234)", &INVARIANT), Ok(-1234));
        assert_eq!(cast_i32(b"1e3", &INVARIANT), Ok(1000));
        assert_eq!(cast_i32(b"0x2A", &INVARIANT), Ok(42));
        assert_eq!(cast_i32(b"&H2A", &INVARIANT), Ok(42));
        assert_eq!(cast_i32(b"0b1010", &INVARIANT), Ok(10));
    }

    #[test]
    fn int_honors_declared_eurozone_grouping() {
        let eurozone = NumFormat::new(',', '.', NumFormat::ALL);
        assert_eq!(cast_i32(b"1.234", &eurozone), Ok(1234));
        assert_eq!(reason(cast_i32(b"1,5", &eurozone)), Reason::Malformed);
    }

    #[test]
    fn int_reads_hex_as_the_twos_complement_bit_pattern() {
        assert_eq!(cast_i8(b"0xFF", &INVARIANT), Ok(-1));
        assert_eq!(cast_i8(b"0x7F", &INVARIANT), Ok(127));
        assert_eq!(cast_u8(b"0xFF", &INVARIANT), Ok(255));
        assert_eq!(reason(cast_i8(b"0x1FF", &INVARIANT)), Reason::OutOfRange);
        assert_eq!(cast_i64(b"0xFFFFFFFFFFFFFFFF", &INVARIANT), Ok(-1));
    }

    #[test]
    fn int_rejects_decimal_points_in_any_disguise() {
        assert_eq!(reason(cast_i32(b"12.5", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"1.5e0", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"1e-2", &INVARIANT)), Reason::Malformed);
    }

    #[test]
    fn int_rejects_signed_radix_and_garbage() {
        assert_eq!(reason(cast_i32(b"-0x1F", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"abc", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"0x", &INVARIANT)), Reason::Malformed);
    }

    #[test]
    fn int_range_is_the_types_own() {
        assert_eq!(cast_u8(b"0", &INVARIANT), Ok(0));
        assert_eq!(cast_u8(b"255", &INVARIANT), Ok(255));
        assert_eq!(reason(cast_u8(b"256", &INVARIANT)), Reason::OutOfRange);
        assert_eq!(reason(cast_u8(b"-1", &INVARIANT)), Reason::OutOfRange);
        assert_eq!(cast_i8(b"127", &INVARIANT), Ok(i8::MAX));
        assert_eq!(cast_i8(b"-128", &INVARIANT), Ok(i8::MIN));
        assert_eq!(cast_i16(b"32767", &INVARIANT), Ok(i16::MAX));
        assert_eq!(cast_u16(b"65535", &INVARIANT), Ok(u16::MAX));
        assert_eq!(cast_u32(b"4294967295", &INVARIANT), Ok(u32::MAX));
        assert_eq!(cast_i64(b"9223372036854775807", &INVARIANT), Ok(i64::MAX));
        assert_eq!(cast_i64(b"-9223372036854775808", &INVARIANT), Ok(i64::MIN));
        assert_eq!(cast_u64(b"18446744073709551615", &INVARIANT), Ok(u64::MAX));
        assert_eq!(reason(cast_i64(b"99999999999999999999999", &INVARIANT)), Reason::OutOfRange);
        assert_eq!(reason(cast_i32(b"5e40", &INVARIANT)), Reason::OutOfRange);
    }

    #[test]
    fn int_group_separator_must_sit_between_digits() {
        assert_eq!(reason(cast_i32(b",1", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"1,", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"1,,2", &INVARIANT)), Reason::Malformed);
        // Group sizes are not validated — the separator just has to sit between digits.
        assert_eq!(cast_i32(b"12,34", &INVARIANT), Ok(1234));
    }

    #[test]
    fn int_fault_points_at_the_offending_byte() {
        let fault = cast_i32(b"  12x4", &INVARIANT).unwrap_err();
        assert_eq!((fault.reason, fault.offset, fault.len), (Reason::Malformed, 4, 1));
        let fault = cast_i32("12\u{00A0}4".as_bytes(), &INVARIANT).unwrap_err();
        assert_eq!((fault.reason, fault.offset, fault.len), (Reason::Malformed, 2, 2));
    }

    #[test]
    fn int_flags_gate_each_lenience() {
        let strict = NumFormat { flags: 0, ..NumFormat::INVARIANT };
        assert_eq!(cast_i32(b"1234", &strict), Ok(1234));
        assert_eq!(reason(cast_i32(b"1,234", &strict)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"(12)", &strict)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"1e3", &strict)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"0x2A", &strict)), Reason::Malformed);
    }

    // --- reals ---

    #[test]
    fn real_parses_the_svartalfheim_notation_set() {
        assert_eq!(cast_f64(b"1.5", &INVARIANT), Ok(1.5));
        assert_eq!(cast_f64(b"  2.25  ", &INVARIANT), Ok(2.25));
        assert_eq!(cast_f64(b"-3.5", &INVARIANT), Ok(-3.5));
        assert_eq!(cast_f64(b"1,234.5", &INVARIANT), Ok(1234.5));
        assert_eq!(cast_f64(b"(2.5)", &INVARIANT), Ok(-2.5));
        assert_eq!(cast_f64(b"2.5e3", &INVARIANT), Ok(2500.0));
        assert_eq!(cast_f64(b"2.5e-3", &INVARIANT), Ok(0.0025));
        assert_eq!(cast_f64(b"50%", &INVARIANT), Ok(0.5));
        assert_eq!(cast_f64(b"25.5%", &INVARIANT), Ok(0.255));
        assert_eq!(cast_f32(b"1.5", &INVARIANT), Ok(1.5f32));
        assert_eq!(cast_f64(b".5", &INVARIANT), Ok(0.5));
    }

    #[test]
    fn real_honors_declared_eurozone_separators() {
        let eurozone = NumFormat::new(',', '.', NumFormat::ALL);
        assert_eq!(cast_f64(b"1.234,5", &eurozone), Ok(1234.5));
        let french = NumFormat::new(',', '\u{00A0}', NumFormat::ALL);
        assert_eq!(cast_f64("1\u{00A0}234,5".as_bytes(), &french), Ok(1234.5));
    }

    #[test]
    fn separator_detection_resolves_structure_and_refuses_ambiguity() {
        const DETECT: NumFormat = NumFormat::DETECT;
        // Both separators present: the rightmost is the decimal.
        assert_eq!(cast_f64(b"1.234,5", &DETECT), Ok(1234.5));
        assert_eq!(cast_f64(b"1,234.5", &DETECT), Ok(1234.5));
        assert_eq!(cast_f64(b"1.234.567,89", &DETECT), Ok(1_234_567.89));
        // A repeated separator can only be grouping.
        assert_eq!(cast_f64(b"1,234,567", &DETECT), Ok(1_234_567.0));
        assert_eq!(cast_i64(b"1.234.567", &DETECT), Ok(1_234_567));
        // One separator, non-3-digit right run: decimal.
        assert_eq!(cast_f64(b"1,23", &DETECT), Ok(1.23));
        assert_eq!(cast_f64(b"3,1415", &DETECT), Ok(3.1415));
        assert_eq!(cast_f64(b"1,5e3", &DETECT), Ok(1500.0));
        // One separator, 3 digits right, zero-led integer part: decimal ("0785" would be
        // no number at all).
        assert_eq!(cast_f64(b"0,785", &DETECT), Ok(0.785));
        assert_eq!(cast_f64(b"0,785%", &DETECT), Ok(0.785 / 100.0));
        assert_eq!(cast_f64(b"(0.785)", &DETECT), Ok(-0.785));
        // Genuinely ambiguous: one separator, 3 digits right, non-zero integer part —
        // Malformed at the undecidable separator, never guessed.
        for text in ["12.185", "1,000", "12,185", "1,500e3"] {
            assert_eq!(reason(cast_f64(text.as_bytes(), &DETECT)), Reason::Malformed, "{text}");
            assert_eq!(reason(cast_i32(text.as_bytes(), &DETECT)), Reason::Malformed, "{text}");
        }
        // The fault span points at the separator itself.
        assert_eq!(cast_f64(b"12.185", &DETECT).unwrap_err().offset, 2);
        // No separators at all: nothing to detect, nothing changes.
        assert_eq!(cast_i32(b"1234", &DETECT), Ok(1234));
        assert_eq!(cast_f64(b"-2.5", &DETECT), Ok(-2.5));
        // Declared formats are untouched: 12.185 parses under an explicit declaration.
        assert_eq!(cast_f64(b"12.185", &INVARIANT), Ok(12.185));
    }

    #[test]
    fn real_admits_only_finite_values() {
        // The literals Rust's own parser would accept are Malformed here by construction...
        assert_eq!(reason(cast_f64(b"NaN", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_f64(b"Infinity", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_f64(b"-inf", &INVARIANT)), Reason::Malformed);
        // ...and a well-formed magnitude that overflows the type is OutOfRange.
        assert_eq!(reason(cast_f64(b"1e400", &INVARIANT)), Reason::OutOfRange);
        assert_eq!(reason(cast_f32(b"1e39", &INVARIANT)), Reason::OutOfRange);
        assert_eq!(cast_f64(b"1e39", &INVARIANT), Ok(1e39));
    }

    #[test]
    fn real_rejects_malformed_shapes() {
        assert_eq!(reason(cast_f64(b"abc", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_f64(b"1.2.3", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_f64(b"%", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_f64(b"1,234.5,6", &INVARIANT)), Reason::Malformed);
        assert_eq!(reason(cast_f64(b"", &INVARIANT)), Reason::Empty);
    }

    // --- currency ---

    fn dollars() -> NumFormat {
        NumFormat::INVARIANT.with_currency(CurrencySymbol::new("$").unwrap())
    }

    #[test]
    fn currency_symbol_is_matched_at_either_edge_once() {
        let usd = dollars();
        assert_eq!(cast_i32(b"$1,234", &usd), Ok(1234));
        assert_eq!(cast_i32(b"1,234$", &usd), Ok(1234));
        assert_eq!(cast_i32(b"$ 5", &usd), Ok(5));
        assert_eq!(cast_i32(b"5 $", &usd), Ok(5));
        assert_eq!(cast_i32(b"-$5", &usd), Ok(-5));
        assert_eq!(cast_i32(b"$-5", &usd), Ok(-5));
        assert_eq!(cast_i32(b"($5)", &usd), Ok(-5));
        assert_eq!(cast_f64(b"$1,234.50", &usd), Ok(1234.5));
        assert_eq!(cast_f64(b"$50%", &usd), Ok(0.5));
        assert_eq!(reason(cast_i32(b"$1$", &usd)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"-$-5", &usd)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"$", &usd)), Reason::Malformed);
        assert_eq!(reason(cast_i32(b"$(5)", &usd)), Reason::Malformed);
    }

    #[test]
    fn currency_symbol_may_contain_separator_characters() {
        let danish = NumFormat::new(',', '.', NumFormat::ALL)
            .with_currency(CurrencySymbol::new("kr.").unwrap());
        assert_eq!(cast_i64(b"1.234.567 kr.", &danish), Ok(1_234_567));
        assert_eq!(cast_f64(b"1.234,50 kr.", &danish), Ok(1234.5));
        let swiss = NumFormat::new('.', '\'', NumFormat::ALL)
            .with_currency(CurrencySymbol::new("CHF").unwrap());
        assert_eq!(cast_f32(b"CHF 1'234.50", &swiss), Ok(1234.5));
    }

    #[test]
    fn currency_flag_gates_the_symbol_and_no_symbol_matches_nothing() {
        let declared_but_off = NumFormat { flags: NumFormat::ALL & !NumFormat::CURRENCY, ..dollars() };
        let fault = cast_i32(b"$5", &declared_but_off).unwrap_err();
        assert_eq!((fault.reason, fault.offset, fault.len), (Reason::Malformed, 0, 1));
        assert_eq!(cast_i32(b"5", &declared_but_off), Ok(5));
        // Flag on, nothing declared: the invariant profile is unchanged by the flag.
        assert_eq!(reason(cast_i32(b"$5", &INVARIANT)), Reason::Malformed);
        assert_eq!(cast_i32(b"5", &INVARIANT), Ok(5));
    }

    #[test]
    fn currency_symbol_rejects_what_would_collide_with_the_scan() {
        assert!(CurrencySymbol::new("").is_none());
        assert!(CurrencySymbol::new("US 1").is_none());
        assert!(CurrencySymbol::new("$1").is_none());
        assert!(CurrencySymbol::new("seventeen-bytes!!").is_none());
        assert_eq!(CurrencySymbol::new("руб.").unwrap().as_str(), "руб.");
        assert!(CurrencySymbol::NONE.is_empty());
    }

    #[test]
    fn separator_detection_carries_the_currency_through() {
        let detect = NumFormat::DETECT.with_currency(CurrencySymbol::new("€").unwrap());
        assert_eq!(cast_f64("€ 1.234.567,89".as_bytes(), &detect), Ok(1_234_567.89));
        assert_eq!(cast_i32("1.234.567 €".as_bytes(), &detect), Ok(1_234_567));
    }

    // --- decimal ---

    fn dec(text: &[u8]) -> Result<Decimal, Fault> {
        cast_decimal(text, &INVARIANT)
    }

    fn decimal(magnitude: u128, scale: u8, negative: bool) -> Decimal {
        Decimal { lo: magnitude as u64, hi: (magnitude >> 64) as u32, scale, negative }
    }

    #[test]
    fn decimal_is_exact_and_canonical() {
        assert_eq!(dec(b"0.1"), Ok(decimal(1, 1, false)));
        assert_eq!(dec(b"1.10"), Ok(decimal(11, 1, false)));
        assert_eq!(dec(b"1.1000"), Ok(decimal(11, 1, false)));
        assert_eq!(dec(b"100"), Ok(decimal(100, 0, false)));
        assert_eq!(dec(b"1,234.50"), Ok(decimal(12345, 1, false)));
        assert_eq!(dec(b"(2.5)"), Ok(decimal(25, 1, true)));
        assert_eq!(dec(b".5"), Ok(decimal(5, 1, false)));
        assert_eq!(dec(b"2.5e3"), Ok(decimal(2500, 0, false)));
        assert_eq!(dec(b"2.5e-3"), Ok(decimal(25, 4, false)));
        assert_eq!(dec(b"50%"), Ok(decimal(5, 1, false)));
        assert_eq!(dec(b"100%"), Ok(decimal(1, 0, false)));
        assert_eq!(dec(b"(2.5)%"), Ok(decimal(25, 3, true)));
        assert_eq!(cast_decimal(b"$1,234.50", &dollars()), Ok(decimal(12345, 1, false)));
    }

    #[test]
    fn decimal_zero_is_never_negative() {
        assert_eq!(dec(b"-0"), Ok(decimal(0, 0, false)));
        assert_eq!(dec(b"-0.00"), Ok(decimal(0, 0, false)));
        assert_eq!(dec(b"0e999999"), Ok(decimal(0, 0, false)));
    }

    #[test]
    fn decimal_range_is_96_bits_and_28_places_with_no_rounding() {
        let max = (1u128 << 96) - 1;
        assert_eq!(dec(b"79228162514264337593543950335"), Ok(decimal(max, 0, false)));
        assert_eq!(dec(b"-79228162514264337593543950335"), Ok(decimal(max, 0, true)));
        assert_eq!(reason(dec(b"79228162514264337593543950336")), Reason::OutOfRange);
        assert_eq!(dec(b"0.0000000000000000000000000001"), Ok(decimal(1, 28, false)));
        assert_eq!(reason(dec(b"0.00000000000000000000000000001")), Reason::OutOfRange);
        assert_eq!(reason(dec(b"1e-29")), Reason::OutOfRange);
        assert_eq!(reason(dec(b"1e29")), Reason::OutOfRange);
        // Exact trailing zeros are always shed — which is also how an over-deep literal
        // comes to fit; a nonzero digit never is.
        assert_eq!(dec(b"1.0000000000000000000000000000000"), Ok(decimal(1, 0, false)));
        assert_eq!(dec(b"7922816251426433759354395033.50"), Ok(decimal(max, 1, false)));
        assert_eq!(reason(dec(b"7922816251426433759354395033.51")), Reason::OutOfRange);
    }

    #[test]
    fn decimal_rejects_the_same_shapes_the_real_doors_do() {
        assert_eq!(reason(dec(b"NaN")), Reason::Malformed);
        assert_eq!(reason(dec(b"Infinity")), Reason::Malformed);
        assert_eq!(reason(dec(b"1.2.3")), Reason::Malformed);
        assert_eq!(reason(dec(b"1e")), Reason::Malformed);
        assert_eq!(reason(dec(b"")), Reason::Empty);
        let fault = dec(b"1.2.3").unwrap_err();
        assert_eq!((fault.offset, fault.len), (3, 1));
    }

    #[test]
    fn decimal_renders_its_canonical_text() {
        assert_eq!(dec(b"1.10").unwrap().to_string(), "1.1");
        assert_eq!(dec(b"(2.5)%").unwrap().to_string(), "-0.025");
        assert_eq!(dec(b"0.00").unwrap().to_string(), "0");
        assert_eq!(dec(b"2.5e3").unwrap().to_string(), "2500");
        assert_eq!(dec(b"1e28").unwrap().to_string(), "10000000000000000000000000000");
    }

    #[test]
    fn version_is_packed_from_the_manifest() {
        let expected: u32 = env!("CARGO_PKG_VERSION")
            .split('.')
            .map(|field| field.parse::<u32>().unwrap())
            .fold(0, |acc, field| (acc << 8) | field);
        assert_eq!(hypercast_version(), expected);
    }

    // --- uuid ---

    const KNOWN: [u8; 16] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f, 0x10,
    ];

    #[test]
    fn uuid_accepts_every_dotnet_format_and_prefix() {
        for text in [
            "01020304-0506-0708-090a-0b0c0d0e0f10",
            "0102030405060708090a0b0c0d0e0f10",
            "{01020304-0506-0708-090a-0b0c0d0e0f10}",
            "(01020304-0506-0708-090a-0b0c0d0e0f10)",
            "  01020304-0506-0708-090a-0b0c0d0e0f10  ",
            "urn:uuid:01020304-0506-0708-090a-0b0c0d0e0f10",
            "GUID:01020304-0506-0708-090a-0b0c0d0e0f10",
            "uuid:01020304-0506-0708-090a-0b0c0d0e0f10",
            "01020304-0506-0708-090A-0B0C0D0E0F10",
            "{0x01020304,0x0506,0x0708,{0x09,0x0a,0x0b,0x0c,0x0d,0x0e,0x0f,0x10}}",
        ] {
            assert_eq!(cast_uuid(text.as_bytes()), Ok(KNOWN), "{text}");
        }
    }

    #[test]
    fn uuid_rejects_malformed_shapes() {
        for text in [
            "not-a-guid",
            "GUID:not-a-guid",
            "01020304-0506-0708-090a-0b0c0d0e0f10-extra",
            "01020304-0506-0708-090a-0b0c0d0e0f1",
            "{01020304-0506-0708-090a-0b0c0d0e0f10)",
            "GUID:",
        ] {
            assert_eq!(reason(cast_uuid(text.as_bytes())), Reason::Malformed, "{text}");
        }
        assert_eq!(reason(cast_uuid(b"   ")), Reason::Empty);
    }

    // --- timestamp ---

    #[test]
    fn timestamp_parses_rfc3339_and_normalizes_to_utc() {
        // RFC 9562's own test-vector instant: 2022-02-22T19:22:22Z = 1645557742.
        assert_eq!(
            cast_timestamp(b"2022-02-22T19:22:22Z"),
            Ok(Timestamp { seconds: 1_645_557_742, nanos: 0 })
        );
        assert_eq!(
            cast_timestamp(b"2026-01-02T15:04:05.123Z"),
            Ok(Timestamp { seconds: 1_767_366_245, nanos: 123_000_000 })
        );
        // 15:04:05+05:00 is 10:04:05Z.
        assert_eq!(
            cast_timestamp(b"2026-01-02T15:04:05+05:00").unwrap().seconds,
            cast_timestamp(b"2026-01-02T10:04:05Z").unwrap().seconds
        );
        assert_eq!(cast_timestamp(b"1970-01-01T00:00:00Z"), Ok(Timestamp { seconds: 0, nanos: 0 }));
        assert_eq!(
            cast_timestamp(b"2026-01-02t15:04:05z"),
            cast_timestamp(b"2026-01-02T15:04:05Z")
        );
        // -00:00 is RFC 3339's "offset unknown", still a UTC instant.
        assert_eq!(
            cast_timestamp(b"2026-01-02T15:04:05-00:00"),
            cast_timestamp(b"2026-01-02T15:04:05Z")
        );
    }

    #[test]
    fn timestamp_window_boundaries_are_legal_values() {
        assert_eq!(
            cast_timestamp(b"0001-01-01T00:00:00Z"),
            Ok(Timestamp { seconds: MIN_TIMESTAMP_SECONDS, nanos: 0 })
        );
        assert_eq!(
            cast_timestamp(b"9999-12-31T23:59:59.999999999Z"),
            Ok(Timestamp { seconds: MAX_TIMESTAMP_SECONDS, nanos: 999_999_999 })
        );
    }

    #[test]
    fn timestamp_faults_outside_the_window_as_out_of_range() {
        assert_eq!(reason(cast_timestamp(b"0000-01-01T00:00:00Z")), Reason::OutOfRange);
        // A legal wall time whose offset pushes it past an edge.
        assert_eq!(reason(cast_timestamp(b"0001-01-01T00:00:00+01:00")), Reason::OutOfRange);
        assert_eq!(reason(cast_timestamp(b"9999-12-31T23:59:59-01:00")), Reason::OutOfRange);
    }

    #[test]
    fn timestamp_demands_a_zone_and_the_t_separator() {
        assert_eq!(reason(cast_timestamp(b"2026-01-02T15:04:05")), Reason::Malformed);
        assert_eq!(reason(cast_timestamp(b"2026-01-02 15:04:05Z")), Reason::Malformed);
        assert_eq!(reason(cast_timestamp(b"1/2/2026 3:04 PM")), Reason::Malformed);
        assert_eq!(reason(cast_timestamp(b"1700000000")), Reason::Malformed);
        assert_eq!(reason(cast_timestamp(b"2026-01-02T15:04Z")), Reason::Malformed);
        assert_eq!(reason(cast_timestamp(b"2026-01-02T15:04:05+0500")), Reason::Malformed);
    }

    #[test]
    fn timestamp_rejects_impossible_calendar_and_clock_readings() {
        assert_eq!(reason(cast_timestamp(b"2026-02-29T00:00:00Z")), Reason::Malformed);
        assert!(cast_timestamp(b"2024-02-29T00:00:00Z").is_ok());
        assert_eq!(reason(cast_timestamp(b"2026-13-01T00:00:00Z")), Reason::Malformed);
        assert_eq!(reason(cast_timestamp(b"2026-01-02T24:00:00Z")), Reason::Malformed);
        // Leap seconds have no protobuf representation; a deterministic core doesn't smear.
        assert_eq!(reason(cast_timestamp(b"2016-12-31T23:59:60Z")), Reason::Malformed);
    }

    #[test]
    fn timestamp_fraction_carries_nanosecond_fidelity() {
        assert_eq!(cast_timestamp(b"1970-01-01T00:00:00.000000001Z").unwrap().nanos, 1);
        assert_eq!(reason(cast_timestamp(b"1970-01-01T00:00:00.0000000001Z")), Reason::Malformed);
        // Sub-second nanos count forward even before the epoch (protobuf convention).
        assert_eq!(
            cast_timestamp(b"1969-12-31T23:59:59.5Z"),
            Ok(Timestamp { seconds: -1, nanos: 500_000_000 })
        );
    }

    // --- unix ---

    #[test]
    fn unix_reads_the_declared_precision_and_never_guesses() {
        assert_eq!(
            cast_unix(b"1700000000", UnixPrecision::Seconds),
            Ok(Timestamp { seconds: 1_700_000_000, nanos: 0 })
        );
        assert_eq!(
            cast_unix(b"1700000000000", UnixPrecision::Millis),
            Ok(Timestamp { seconds: 1_700_000_000, nanos: 0 })
        );
        assert_eq!(
            cast_unix(b"1700000000123456", UnixPrecision::Micros),
            Ok(Timestamp { seconds: 1_700_000_000, nanos: 123_456_000 })
        );
        assert_eq!(
            cast_unix(b"1700000000123456789", UnixPrecision::Nanos),
            Ok(Timestamp { seconds: 1_700_000_000, nanos: 123_456_789 })
        );
    }

    #[test]
    fn unix_handles_pre_epoch_values_with_forward_counting_nanos() {
        assert_eq!(
            cast_unix(b"-1", UnixPrecision::Seconds),
            Ok(Timestamp { seconds: -1, nanos: 0 })
        );
        assert_eq!(
            cast_unix(b"-1", UnixPrecision::Millis),
            Ok(Timestamp { seconds: -1, nanos: 999_000_000 })
        );
    }

    #[test]
    fn unix_rejects_non_integers_and_the_out_of_window() {
        assert_eq!(reason(cast_unix(b"1700000000.5", UnixPrecision::Seconds)), Reason::Malformed);
        assert_eq!(reason(cast_unix(b"not-a-number", UnixPrecision::Seconds)), Reason::Malformed);
        assert_eq!(reason(cast_unix(b"253402300800", UnixPrecision::Seconds)), Reason::OutOfRange);
        assert_eq!(reason(cast_unix(b"-62135596801", UnixPrecision::Seconds)), Reason::OutOfRange);
        assert_eq!(cast_unix(b"253402300799", UnixPrecision::Seconds).unwrap().seconds, MAX_TIMESTAMP_SECONDS);
    }

    // --- excel serial ---

    /// The anchors are computed from `days_from_civil`, so these assertions are what pins
    /// them to the values Excel actually uses rather than to whatever the arithmetic
    /// happened to produce. 25569 is the well-known 1900-system serial for the Unix epoch.
    #[test]
    fn excel_serial_agrees_with_the_well_known_anchor_serials() {
        let epoch_1900 = cast_excel_serial(b"25569", ExcelEpoch::Y1900).unwrap();
        assert_eq!(epoch_1900, Timestamp { seconds: 0, nanos: 0 });
        let epoch_1904 = cast_excel_serial(b"24107", ExcelEpoch::Y1904).unwrap();
        assert_eq!(epoch_1904, Timestamp { seconds: 0, nanos: 0 });

        // First real day of each system.
        assert_eq!(cast_excel_serial(b"1", ExcelEpoch::Y1900).unwrap().seconds, -2_208_988_800);
        assert_eq!(cast_excel_serial(b"0", ExcelEpoch::Y1904).unwrap().seconds, -2_082_844_800);
    }

    /// The whole point of the door: serial 60 is Excel's phantom 1900-02-29, so 59 and 61
    /// are consecutive real days one 86,400-second step apart despite the gap in serials.
    #[test]
    fn excel_serial_rejects_the_phantom_leap_day_and_shifts_everything_after_it() {
        assert_eq!(reason(cast_excel_serial(b"60", ExcelEpoch::Y1900)), Reason::Malformed);

        let feb28 = cast_excel_serial(b"59", ExcelEpoch::Y1900).unwrap().seconds;
        let mar01 = cast_excel_serial(b"61", ExcelEpoch::Y1900).unwrap().seconds;
        assert_eq!(mar01 - feb28, 86_400, "59 and 61 are adjacent real days");

        // The same date reached through the text door, which already calls 1900-02-29
        // malformed — the two doors agree that day does not exist.
        assert_eq!(cast_date(b"1900-02-28"), Ok(Date { year: 1900, month: 2, day: 28 }));
        assert_eq!(reason(cast_date(b"1900-02-29")), Reason::Malformed);

        // 1904 has no phantom: its serial 60 is an ordinary day.
        assert!(cast_excel_serial(b"60", ExcelEpoch::Y1904).is_ok());
    }

    #[test]
    fn excel_serial_reads_the_fraction_as_time_of_day() {
        // 45292 is 2024-01-01 in the 1900 system; .75 is 18:00.
        let midnight = cast_excel_serial(b"45292", ExcelEpoch::Y1900).unwrap();
        let evening = cast_excel_serial(b"45292.75", ExcelEpoch::Y1900).unwrap();
        assert_eq!(evening.seconds - midnight.seconds, 64_800);
        assert_eq!(evening.nanos, 0);

        assert_eq!(cast_excel_serial(b"1.5", ExcelEpoch::Y1900).unwrap().nanos, 0);
        // A fraction that lands off a whole second still resolves into nanos:
        // 0.0000001 of a day is 0.00864 s.
        let odd = cast_excel_serial(b"1.0000001", ExcelEpoch::Y1900).unwrap();
        assert_eq!(odd.nanos, 8_640_000);
    }

    #[test]
    fn excel_serial_rejects_malformed_text_and_the_out_of_window() {
        assert_eq!(reason(cast_excel_serial(b"", ExcelEpoch::Y1900)), Reason::Empty);
        assert_eq!(reason(cast_excel_serial(b"not-a-number", ExcelEpoch::Y1900)), Reason::Malformed);
        assert_eq!(reason(cast_excel_serial(b"45292.", ExcelEpoch::Y1900)), Reason::Malformed);
        assert_eq!(reason(cast_excel_serial(b".5", ExcelEpoch::Y1900)), Reason::Malformed);
        // A date serial is never signed — no silent reflection into pre-1900.
        assert_eq!(reason(cast_excel_serial(b"-1", ExcelEpoch::Y1900)), Reason::Malformed);
        assert_eq!(reason(cast_excel_serial(b"+1", ExcelEpoch::Y1900)), Reason::Malformed);
        // Below each system's own first real day.
        assert_eq!(reason(cast_excel_serial(b"0", ExcelEpoch::Y1900)), Reason::OutOfRange);
        // Past 9999-12-31.
        assert_eq!(reason(cast_excel_serial(b"2958466", ExcelEpoch::Y1900)), Reason::OutOfRange);
        assert_eq!(reason(cast_excel_serial(b"2957004", ExcelEpoch::Y1904)), Reason::OutOfRange);
        assert!(cast_excel_serial(b"2958465", ExcelEpoch::Y1900).is_ok());
        assert!(cast_excel_serial(b"2957003", ExcelEpoch::Y1904).is_ok());
    }

    // --- date ---

    #[test]
    fn date_accepts_exactly_the_iso_profile() {
        assert_eq!(cast_date(b"2026-01-02"), Ok(Date { year: 2026, month: 1, day: 2 }));
        assert_eq!(cast_date(b"  2026-01-02  "), Ok(Date { year: 2026, month: 1, day: 2 }));
        assert_eq!(cast_date(b"2024-02-29"), Ok(Date { year: 2024, month: 2, day: 29 }));
        assert_eq!(cast_date(b"0001-01-01"), Ok(Date { year: 1, month: 1, day: 1 }));
        assert_eq!(cast_date(b"9999-12-31"), Ok(Date { year: 9999, month: 12, day: 31 }));
    }

    #[test]
    fn date_rejects_everything_else() {
        for text in ["1/2/2026", "2026-01-02T00:00:00", "2026/01/02", "garbage", "2026-02-29", "2026-00-01", "2026-01-00"] {
            assert_eq!(reason(cast_date(text.as_bytes())), Reason::Malformed, "{text}");
        }
        assert_eq!(reason(cast_date(b"0000-01-01")), Reason::OutOfRange);
        assert_eq!(reason(cast_date(b"")), Reason::Empty);
    }

    #[test]
    fn date_ordered_disambiguates_by_declaration_never_by_guessing() {
        // The canonical ambiguity: 1/7/2026 is January 7th in en-US (month-first) and
        // July 1st in en-GB (day-first) — resolved only by what the caller declared.
        assert_eq!(
            cast_date_ordered(b"1/7/2026", DateOrder::MonthDayYear),
            Ok(Date { year: 2026, month: 1, day: 7 })
        );
        assert_eq!(
            cast_date_ordered(b"1/7/2026", DateOrder::DayMonthYear),
            Ok(Date { year: 2026, month: 7, day: 1 })
        );
        assert_eq!(
            cast_date_ordered(b"2026/1/7", DateOrder::YearMonthDay),
            Ok(Date { year: 2026, month: 1, day: 7 })
        );
        // Zero-padding, dot and dash separators, and the strict ISO form as YMD's subset.
        assert_eq!(
            cast_date_ordered(b"01/07/2026", DateOrder::MonthDayYear),
            Ok(Date { year: 2026, month: 1, day: 7 })
        );
        assert_eq!(
            cast_date_ordered(b"1.7.2026", DateOrder::DayMonthYear),
            Ok(Date { year: 2026, month: 7, day: 1 })
        );
        assert_eq!(
            cast_date_ordered(b"2026-01-07", DateOrder::YearMonthDay),
            Ok(Date { year: 2026, month: 1, day: 7 })
        );
        // A four-digit FIRST field can only be a year — year-first forms parse under any
        // declared order (width detection, not value sniffing).
        assert_eq!(
            cast_date_ordered(b"2026/1/7", DateOrder::MonthDayYear),
            Ok(Date { year: 2026, month: 1, day: 7 })
        );
        // 13 can only be a day — valid day-first, malformed month-first, span on the field.
        assert_eq!(
            cast_date_ordered(b"13/1/2026", DateOrder::DayMonthYear),
            Ok(Date { year: 2026, month: 1, day: 13 })
        );
        assert_eq!(reason(cast_date_ordered(b"13/1/2026", DateOrder::MonthDayYear)), Reason::Malformed);
        // Real calendar, same as the strict door.
        assert_eq!(
            cast_date_ordered(b"29/2/2024", DateOrder::DayMonthYear),
            Ok(Date { year: 2024, month: 2, day: 29 })
        );
        assert_eq!(reason(cast_date_ordered(b"29/2/2026", DateOrder::DayMonthYear)), Reason::Malformed);
    }

    #[test]
    fn date_ordered_rejects_ambiguity_reintroducers() {
        // Two-digit years mean century guessing — never.
        assert_eq!(reason(cast_date_ordered(b"1/7/26", DateOrder::MonthDayYear)), Reason::Malformed);
        // A three-digit field fits no order — faulted at its own digits.
        assert_eq!(reason(cast_date_ordered(b"123/4/2026", DateOrder::MonthDayYear)), Reason::Malformed);
        // Mixed separators, trailing junk, missing fields.
        for text in ["1-7/2026", "1/7/2026 extra", "1/7", "1//2026", "garbage"] {
            assert_eq!(
                reason(cast_date_ordered(text.as_bytes(), DateOrder::MonthDayYear)),
                Reason::Malformed,
                "{text}"
            );
        }
        assert_eq!(reason(cast_date_ordered(b"1/7/0000", DateOrder::MonthDayYear)), Reason::OutOfRange);
        assert_eq!(reason(cast_date_ordered(b"   ", DateOrder::DayMonthYear)), Reason::Empty);
    }

    #[test]
    fn datetime_reads_civil_wall_clock_under_the_declared_order() {
        const NANOS_PER_MINUTE: u64 = 60 * 1_000_000_000;
        // The messy shapes untrusted feeds actually send, AM/PM included.
        assert_eq!(
            cast_datetime(b"1/7/2026 3:04 PM", DateOrder::MonthDayYear),
            Ok(CivilDateTime {
                date: Date { year: 2026, month: 1, day: 7 },
                nanos_of_day: (15 * 60 + 4) * NANOS_PER_MINUTE,
            })
        );
        assert_eq!(
            cast_datetime(b"1/7/2026 3:04 pm", DateOrder::DayMonthYear),
            Ok(CivilDateTime {
                date: Date { year: 2026, month: 7, day: 1 },
                nanos_of_day: (15 * 60 + 4) * NANOS_PER_MINUTE,
            })
        );
        // Hour-only with a meridiem; 12 AM is midnight, 12 PM is noon.
        assert_eq!(
            cast_datetime(b"1/7/2026 3PM", DateOrder::MonthDayYear).unwrap().nanos_of_day,
            15 * 60 * NANOS_PER_MINUTE
        );
        assert_eq!(
            cast_datetime(b"1/7/2026 12:00 AM", DateOrder::MonthDayYear).unwrap().nanos_of_day,
            0
        );
        assert_eq!(
            cast_datetime(b"1/7/2026 12:30 PM", DateOrder::MonthDayYear).unwrap().nanos_of_day,
            (12 * 60 + 30) * NANOS_PER_MINUTE
        );
        // 24-hour, single-digit hour, seconds and fraction; ISO date part and T separator
        // parse under any declared order (four-digit first field is structurally a year).
        assert_eq!(
            cast_datetime(b"2026-01-07T15:04:05.123456789", DateOrder::MonthDayYear),
            Ok(CivilDateTime {
                date: Date { year: 2026, month: 1, day: 7 },
                nanos_of_day: ((15 * 60 + 4) * 60 + 5) * 1_000_000_000 + 123_456_789,
            })
        );
        assert_eq!(
            cast_datetime(b"1/7/2026 9:05", DateOrder::MonthDayYear).unwrap().nanos_of_day,
            (9 * 60 + 5) * NANOS_PER_MINUTE
        );
        // Date-only means midnight — one door covers the mixed column.
        assert_eq!(
            cast_datetime(b"1/7/2026", DateOrder::MonthDayYear),
            Ok(CivilDateTime { date: Date { year: 2026, month: 1, day: 7 }, nanos_of_day: 0 })
        );
    }

    #[test]
    fn datetime_rejects_what_names_no_civil_time() {
        // A meridiem hour past 12, a 24-hour hour past 23, a bare trailing number, and a
        // zone suffix (this door reads no zone and invents none — RFC 3339 instants go
        // through cast_timestamp).
        for text in [
            "1/7/2026 13:04 PM",
            "1/7/2026 25:04",
            "1/7/2026 3",
            "1/7/2026 3:04 XM",
            "1/7/2026 3:04 PM +05:00",
            "1/7/2026 15:04:05Z",
            "1/7/2026  3:04 PM",
        ] {
            assert_eq!(
                reason(cast_datetime(text.as_bytes(), DateOrder::MonthDayYear)),
                Reason::Malformed,
                "{text}"
            );
        }
        assert_eq!(reason(cast_datetime(b"1/7/0000 3:04 PM", DateOrder::MonthDayYear)), Reason::OutOfRange);
        assert_eq!(reason(cast_datetime(b"", DateOrder::MonthDayYear)), Reason::Empty);
    }

    // --- time ---

    #[test]
    fn time_reads_the_24_hour_profile_to_nanos_since_midnight() {
        assert_eq!(cast_time(b"15:04:05"), Ok(54_245_000_000_000));
        assert_eq!(cast_time(b"15:04:05.123"), Ok(54_245_123_000_000));
        assert_eq!(cast_time(b"15:04"), Ok(54_240_000_000_000));
        assert_eq!(cast_time(b"00:00:00"), Ok(0));
        assert_eq!(cast_time(b"23:59:59.999999999"), Ok(86_399_999_999_999));
    }

    #[test]
    fn time_rejects_non_iso_readings() {
        for text in ["3:04:05 PM", "25:00", "noon", "15:60", "15:04:60", "15:04:05.0000000001"] {
            assert_eq!(reason(cast_time(text.as_bytes())), Reason::Malformed, "{text}");
        }
        assert_eq!(reason(cast_time(b"")), Reason::Empty);
    }

    // --- duration ---

    #[test]
    fn duration_parses_all_three_shapes_to_the_same_span() {
        let ninety_minutes = Duration { seconds: 5_400, nanos: 0 };
        assert_eq!(cast_duration(b"01:30:00"), Ok(ninety_minutes));
        assert_eq!(cast_duration(b"PT1H30M"), Ok(ninety_minutes));
        assert_eq!(cast_duration(b"5400s"), Ok(ninety_minutes));
        assert_eq!(cast_duration(b"1.06:00:00"), Ok(Duration { seconds: 108_000, nanos: 0 }));
        assert_eq!(cast_duration(b"P1DT6H"), Ok(Duration { seconds: 108_000, nanos: 0 }));
        assert_eq!(cast_duration(b"P2W"), Ok(Duration { seconds: 1_209_600, nanos: 0 }));
        assert_eq!(cast_duration(b"01:30"), Ok(ninety_minutes));
    }

    #[test]
    fn duration_carries_same_signed_fractions() {
        assert_eq!(cast_duration(b"PT1.5S"), Ok(Duration { seconds: 1, nanos: 500_000_000 }));
        assert_eq!(cast_duration(b"-PT1H"), Ok(Duration { seconds: -3_600, nanos: 0 }));
        assert_eq!(cast_duration(b"-1.5s"), Ok(Duration { seconds: -1, nanos: -500_000_000 }));
        assert_eq!(cast_duration(b"3.000000001s"), Ok(Duration { seconds: 3, nanos: 1 }));
        assert_eq!(cast_duration(b"-00:00:00.5"), Ok(Duration { seconds: 0, nanos: -500_000_000 }));
        assert_eq!(cast_duration(b"00:00:00"), Ok(Duration { seconds: 0, nanos: 0 }));
    }

    #[test]
    fn duration_accepts_iso_8601_comma_decimal_marks() {
        // ISO 8601 permits the comma as the decimal mark, and eurozone feeds send it in
        // all three shapes. Unambiguous — durations have no digit grouping, so a comma
        // here can only be a decimal mark.
        let second_and_a_half = Duration { seconds: 1, nanos: 500_000_000 };
        assert_eq!(cast_duration(b"PT1,5S"), Ok(second_and_a_half));
        assert_eq!(cast_duration(b"0:00:01,5"), Ok(second_and_a_half));
        assert_eq!(cast_duration(b"1,5s"), Ok(second_and_a_half));
        assert_eq!(cast_duration(b"-1,5s"), Ok(Duration { seconds: -1, nanos: -500_000_000 }));
        // A comma with nothing after it is still no fraction.
        assert_eq!(reason(cast_duration(b"PT1,S")), Reason::Malformed);
    }

    #[test]
    fn duration_rejects_calendar_units_and_shapeless_input() {
        for text in ["P1Y", "P2M", "P", "PT", "P3DT", "PT1H30", "90m", "garbage", "PT1.5H"] {
            assert_eq!(reason(cast_duration(text.as_bytes())), Reason::Malformed, "{text}");
        }
        assert_eq!(reason(cast_duration(b"")), Reason::Empty);
    }

    #[test]
    fn duration_bounds_the_protobuf_window() {
        assert_eq!(
            cast_duration(b"315576000000s"),
            Ok(Duration { seconds: MAX_DURATION_SECONDS, nanos: 0 })
        );
        assert_eq!(reason(cast_duration(b"315576000001s")), Reason::OutOfRange);
        assert_eq!(reason(cast_duration(b"-315576000001s")), Reason::OutOfRange);
        assert_eq!(reason(cast_duration(b"PT999999999999999999S")), Reason::OutOfRange);
        assert_eq!(reason(cast_duration(b"P9999999999999999W")), Reason::OutOfRange);
        // A digit run past the parse-sanity bound is shape noise, not a big number.
        assert_eq!(reason(cast_duration(b"PT9999999999999999999S")), Reason::Malformed);
    }

    #[test]
    fn duration_colon_hours_cap_at_23_without_a_day_part() {
        assert_eq!(reason(cast_duration(b"25:00:00")), Reason::Malformed);
        assert_eq!(cast_duration(b"1.01:00:00"), Ok(Duration { seconds: 90_000, nanos: 0 }));
    }

    // --- first-class Rust citizenship: the same end-user surface the bindings present ---

    #[test]
    fn doors_accept_anything_byte_viewable() {
        assert_eq!(cast_bool("yes"), Ok(true));
        assert_eq!(cast_bool(String::from("off")), Ok(false));
        assert_eq!(cast_i32("(1,234)", &INVARIANT), Ok(-1234));
        let owned: Vec<u8> = String::from("6ba7b810-9dad-11d1-80b4-00c04fd430c8").into_bytes();
        assert!(cast_uuid(owned).is_ok());
        assert_eq!(cast_time("23:59:59"), Ok(86_399_000_000_000));
    }

    #[test]
    fn faults_display_the_reason_and_span() {
        assert_eq!(cast_bool("   ").unwrap_err().to_string(), "empty input");
        assert_eq!(cast_bool("maybe").unwrap_err().to_string(), "malformed input at bytes 0..5");
        assert_eq!(cast_u8("256", &INVARIANT).unwrap_err().to_string(), "out of range input at bytes 0..3");
    }

    #[test]
    fn faults_compose_with_ordinary_error_propagation() {
        fn parse_pair(a: &str, b: &str) -> Result<(bool, i32), Box<dyn core::error::Error>> {
            Ok((cast_bool(a)?, cast_i32(b, &INVARIANT)?))
        }
        assert_eq!(parse_pair("on", "42").unwrap(), (true, 42));
        // The integer door's span points precisely at the offending byte, not the token.
        let err = parse_pair("on", "4x2").unwrap_err();
        assert_eq!(err.to_string(), "malformed input at bytes 1..2");
        assert_eq!(err.downcast_ref::<Fault>().unwrap().reason, Reason::Malformed);
    }

    #[test]
    fn optional_presents_empty_as_absent_and_nothing_else() {
        assert_eq!(optional(cast_bool("  ")), Ok(None));
        assert_eq!(optional(cast_bool("true")), Ok(Some(true)));
        assert_eq!(optional(cast_bool("maybe")).unwrap_err().reason, Reason::Malformed);
    }
}
