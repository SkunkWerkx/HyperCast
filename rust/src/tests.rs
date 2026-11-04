//! The crate's unit tests, in a file of their own rather than inline in `lib.rs`.
//!
//! Nothing here is built into an artifact — the module is `#[cfg(test)]` — and CodeQL filters
//! paths per file, so a test inline in `lib.rs` is analyzed as part of the file that ships, and
//! a value a failure message formats is reported as `rust/cleartext-logging` against it.
//! `.github/codeql/codeql-config.yml` ignores this file by name instead.

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

// --- char (the rest of its grammar is corpus/char.json) ---

fn span<T: core::fmt::Debug>(verdict: Result<T, Fault>) -> (Reason, u32, u32) {
    let fault = verdict.unwrap_err();
    (fault.reason, fault.offset, fault.len)
}

#[test]
fn char_is_never_verbatim_for_ill_formed_utf8() {
    // Bytes no JSON corpus can carry: a lone continuation, a lead byte cut short, an
    // encoded surrogate and an overlong encoding are none of them one scalar.
    assert_eq!(span(cast_char([0x80])), (Reason::Malformed, 0, 1));
    assert_eq!(span(cast_char([0xC3])), (Reason::Malformed, 0, 1));
    assert_eq!(span(cast_char([0xED, 0xA0, 0x80])), (Reason::Malformed, 0, 3));
    assert_eq!(span(cast_char([0xC0, 0x80])), (Reason::Malformed, 0, 2));
    assert_eq!(span(cast_char(b" \xFF ")), (Reason::Malformed, 1, 1));
    // A lead byte at the end of a spelling spans what is there, never past the input.
    assert_eq!(span(cast_char(b"U+4\xE2")), (Reason::Malformed, 3, 1));
}

#[test]
fn char_saturates_rather_than_wrapping_back_into_range() {
    // 2^32 + 65 would wrap to 'A' in a u32 accumulator.
    assert_eq!(reason(cast_char(b"4294967361")), Reason::OutOfRange);
    assert_eq!(reason(cast_char(b"0x100000041")), Reason::OutOfRange);
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

// `3,1415` is the README's own example of a non-3-digit right run; clippy reads the
// expected value as a sloppy PI, which it is not.
#[test]
#[allow(clippy::approx_constant)]
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
    assert_eq!(cast_f64(b"0,785%", &DETECT), Ok(0.00785));
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
    let danish =
        NumFormat::new(',', '.', NumFormat::ALL).with_currency(CurrencySymbol::new("kr.").unwrap());
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
fn a_currency_symbol_reads_any_of_the_three_spaces_inside_it() {
    let cfa = |space: &str| CurrencySymbol::new(&format!("F{space}CFA")).unwrap();
    for declared in [" ", "\u{A0}", "\u{202F}"] {
        let symbol = cfa(declared);
        for written in [" ", "\u{A0}", "\u{202F}"] {
            let text = format!("F{written}CFA 5");
            assert_eq!(symbol.strip_leading(text.as_bytes()), Some(b" 5".as_slice()));
            let text = format!("5 F{written}CFA");
            assert_eq!(symbol.strip_trailing(text.as_bytes()), Some(b"5 ".as_slice()));
        }
        // Other white space, a run, none, or a cut-off space never match.
        for text in ["F\u{2009}CFA", "F\u{3000}CFA", "F  CFA", "FCFA", "F", "F\u{C2}CFA", ""] {
            assert_eq!(symbol.strip_leading(text.as_bytes()), None, "{text:?}");
            assert_eq!(symbol.strip_trailing(text.as_bytes()), None, "{text:?}");
        }
        assert_eq!(symbol.strip_leading(&[b'F', 0xE2, 0x80]), None);
        assert_eq!(symbol.strip_trailing(&[0x80, 0xAF, b'C', b'F', b'A']), None);
    }
    // Any other white space inside matches only itself.
    let thin = cfa("\u{2009}");
    assert_eq!(thin.strip_leading("F\u{2009}CFA".as_bytes()), Some(b"".as_slice()));
    assert_eq!(thin.strip_leading(b"F CFA"), None);
    assert_eq!(thin.strip_trailing("F\u{202F}CFA".as_bytes()), None);
    assert_eq!(CurrencySymbol::NONE.strip_leading(b"5"), None);
    assert_eq!(CurrencySymbol::NONE.strip_trailing(b""), None);
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
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
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
    assert_eq!(cast_timestamp(b"2026-01-02t15:04:05z"), cast_timestamp(b"2026-01-02T15:04:05Z"));
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
    // Well-formed fields that name nothing are out of range, at the field.
    assert_eq!(cast_timestamp(b"2026-02-29T00:00:00Z"), Err(Fault::out_of_range(8, 2)));
    assert!(cast_timestamp(b"2024-02-29T00:00:00Z").is_ok());
    assert_eq!(cast_timestamp(b"2026-13-01T00:00:00Z"), Err(Fault::out_of_range(5, 2)));
    assert_eq!(cast_timestamp(b"0000-01-01T00:00:00Z"), Err(Fault::out_of_range(0, 4)));
    assert_eq!(cast_timestamp(b"2026-01-02T24:00:00Z"), Err(Fault::out_of_range(11, 2)));
    assert_eq!(cast_timestamp(b"2026-01-02T15:60:00Z"), Err(Fault::out_of_range(14, 2)));
    // Leap seconds have no protobuf representation; a deterministic core doesn't smear.
    assert_eq!(cast_timestamp(b"2016-12-31T23:59:60Z"), Err(Fault::out_of_range(17, 2)));
    assert_eq!(cast_timestamp(b"2026-01-02T15:04:05+24:00"), Err(Fault::out_of_range(20, 2)));
    assert_eq!(cast_timestamp(b"2026-01-02T15:04:05+05:60"), Err(Fault::out_of_range(23, 2)));
    // Shape is checked across the whole text first: the stray `x` is reported, not the
    // month that would also be wrong.
    assert_eq!(cast_timestamp(b"2026-13-01T00:00:00x"), Err(Fault::malformed(19, 1)));
}

#[test]
fn timestamp_fraction_carries_nanosecond_fidelity() {
    assert_eq!(cast_timestamp(b"1970-01-01T00:00:00.000000001Z").unwrap().nanos, 1);
    // Past the ninth digit the fraction truncates, never rounds: the last instant of the
    // window stays in it rather than carrying out.
    assert_eq!(cast_timestamp(b"1970-01-01T00:00:00.0000000001Z").unwrap().nanos, 0);
    assert_eq!(
        cast_timestamp(b"9999-12-31T23:59:59.99999999999999999Z"),
        Ok(Timestamp { seconds: MAX_TIMESTAMP_SECONDS, nanos: 999_999_999 })
    );
    // Sub-second nanos count forward even before the epoch (protobuf convention).
    assert_eq!(
        cast_timestamp(b"1969-12-31T23:59:59.5Z"),
        Ok(Timestamp { seconds: -1, nanos: 500_000_000 })
    );
}

// --- culture fidelity (the 0.9.0 sweep): everything a format declares beyond its
// separators, each read beside the ASCII default, never instead of it ---

fn symbol(text: &str) -> Symbol<8> {
    Symbol::new(text).unwrap()
}

#[test]
fn a_declared_sign_is_read_beside_the_ascii_one() {
    let estonian = INVARIANT.with_signs(symbol("\u{2212}"), symbol("\u{FF0B}"));
    assert_eq!(cast_i32("\u{2212}5".as_bytes(), &estonian), Ok(-5));
    assert_eq!(cast_i32("\u{FF0B}5".as_bytes(), &estonian), Ok(5));
    assert_eq!(cast_i32(b"-5", &estonian), Ok(-5));
    assert_eq!(cast_f64("\u{2212}2.5".as_bytes(), &estonian), Ok(-2.5));
    assert_eq!(cast_decimal("\u{2212}2.5".as_bytes(), &estonian).map(|d| d.negative), Ok(true));
    // Ahead of or after a leading currency symbol, like the ASCII sign.
    let dollars = estonian.with_currency(CurrencySymbol::new("$").unwrap());
    assert_eq!(cast_i32("\u{2212}$5".as_bytes(), &dollars), Ok(-5));
    assert_eq!(cast_i32("$\u{2212}5".as_bytes(), &dollars), Ok(-5));
    // Undeclared, it is the malformed character it always was; declared twice over, the
    // second sign is the fault, in its own width.
    assert_eq!(span(cast_i32("\u{2212}5".as_bytes(), &INVARIANT)), (Reason::Malformed, 0, 3));
    assert_eq!(span(cast_i32("-\u{2212}5".as_bytes(), &estonian)), (Reason::Malformed, 1, 3));
    assert_eq!(span(cast_i32("\u{2212}-5".as_bytes(), &estonian)), (Reason::Malformed, 3, 1));
    assert_eq!(span(cast_i32("(\u{2212}5)".as_bytes(), &estonian)), (Reason::Malformed, 1, 3));
    // The plain fast paths never take part: the ASCII shape reads as written.
    assert_eq!(cast_i64(b"-9223372036854775808", &estonian), Ok(i64::MIN));
}

#[test]
fn a_trailing_sign_needs_its_flag_and_stands_alone() {
    let trailing = NumFormat { flags: NumFormat::ALL | NumFormat::TRAILING_SIGN, ..INVARIANT };
    assert_eq!(cast_i32(b"5-", &trailing), Ok(-5));
    assert_eq!(cast_i32(b"5 -", &trailing), Ok(-5));
    assert_eq!(cast_i32(b"5+", &trailing), Ok(5));
    assert_eq!(cast_f64(b"2.5-", &trailing), Ok(-2.5));
    assert_eq!(cast_f64(b"2.5%-", &trailing).map(f64::to_bits), Ok((-0.025f64).to_bits()));
    assert_eq!(cast_f64(b"2.5-%", &trailing).map(f64::to_bits), Ok((-0.025f64).to_bits()));
    assert_eq!(cast_f64(b"2.5 %-", &trailing).map(f64::to_bits), Ok((-0.025f64).to_bits()));
    assert_eq!(span(cast_f64(b"-2.5%-", &trailing)), (Reason::Malformed, 5, 1));
    // Of two trailing signs, the later one is the fault, as with two leading ones.
    assert_eq!(span(cast_f64(b"2.5-%-", &trailing)), (Reason::Malformed, 5, 1));
    assert_eq!(cast_decimal(b"1,234.5 -", &trailing).map(|d| d.to_string()), Ok("-1234.5".into()));
    // Not in ALL: the invariant format reads it as it did.
    assert_eq!(span(cast_i32(b"5-", &INVARIANT)), (Reason::Malformed, 1, 1));
    // One sign in all, and none inside accounting parentheses.
    assert_eq!(span(cast_i32(b"-5-", &trailing)), (Reason::Malformed, 2, 1));
    assert_eq!(span(cast_i32(b"(5-)", &trailing)), (Reason::Malformed, 2, 1));
    assert_eq!(span(cast_i32(b"5--", &trailing)), (Reason::Malformed, 1, 1));
    // Around a trailing currency symbol, on either side of it.
    let euros = trailing.with_currency(CurrencySymbol::new("\u{20AC}").unwrap());
    assert_eq!(cast_i32("5 \u{20AC}-".as_bytes(), &euros), Ok(-5));
    assert_eq!(cast_i32("5- \u{20AC}".as_bytes(), &euros), Ok(-5));
    assert_eq!(cast_i32("\u{20AC}5-".as_bytes(), &euros), Ok(-5));
    assert_eq!(span(cast_i32("-5 \u{20AC}-".as_bytes(), &euros)), (Reason::Malformed, 6, 1));
    assert_eq!(span(cast_i32("(5 \u{20AC}-)".as_bytes(), &euros)), (Reason::Malformed, 6, 1));
    // A bare sign is no number.
    assert_eq!(span(cast_i32(b"-", &trailing)), (Reason::Malformed, 0, 1));
    assert_eq!(span(cast_i32("\u{20AC}-".as_bytes(), &euros)), (Reason::Malformed, 0, 4));
}

#[test]
fn a_currency_amount_is_read_with_the_currency_separators_once_the_symbol_is_seen() {
    // en-DE: a decimal comma in numbers, a decimal point in currency.
    let en_de = NumFormat::new(',', '.', NumFormat::ALL)
        .with_currency(CurrencySymbol::new("\u{20AC}").unwrap())
        .with_currency_separators(symbol("."), symbol(","));
    assert_eq!(cast_f64("\u{20AC}1,234.50".as_bytes(), &en_de), Ok(1234.5));
    assert_eq!(cast_f64("1,234.50 \u{20AC}".as_bytes(), &en_de), Ok(1234.5));
    assert_eq!(cast_f64("(\u{20AC}1,234.50)".as_bytes(), &en_de), Ok(-1234.5));
    assert_eq!(cast_f64(b"1.234,50", &en_de), Ok(1234.5));
    assert_eq!(cast_i32("\u{20AC}1,234".as_bytes(), &en_de), Ok(1234));
    assert_eq!(cast_i32(b"1.234", &en_de), Ok(1234));
    assert_eq!(
        cast_decimal("\u{20AC}1,234.50".as_bytes(), &en_de).map(|d| d.to_string()),
        Ok("1234.5".into())
    );
    // Without the symbol the number separators stand, so the currency shape is malformed
    // at its own point, and with it a number-shaped amount is.
    assert_eq!(span(cast_f64(b"1,234.50", &en_de)), (Reason::Malformed, 5, 1));
    assert_eq!(span(cast_i32("\u{20AC}1.234".as_bytes(), &en_de)), (Reason::Malformed, 4, 1));
    // One declared, the other keeps its number counterpart.
    let half = NumFormat::new(',', '.', NumFormat::ALL)
        .with_currency(CurrencySymbol::new("\u{20AC}").unwrap())
        .with_currency_separators(symbol("."), Symbol::NONE);
    assert_eq!(cast_f64("\u{20AC}1.5".as_bytes(), &half), Ok(1.5));
    // Both `.` then: decimal semantics win, as they do for equal number separators.
    assert_eq!(span(cast_f64("\u{20AC}1.234.5".as_bytes(), &half)), (Reason::Malformed, 8, 1));
    // Detection governs the whole token, currency separators included.
    let detect = NumFormat { flags: en_de.flags | NumFormat::SEPARATOR_DETECT, ..en_de };
    assert_eq!(cast_f64("\u{20AC}1.234,50".as_bytes(), &detect), Ok(1234.5));
    assert_eq!(cast_f64("\u{20AC}1,234.50".as_bytes(), &detect), Ok(1234.5));
}

#[test]
fn a_declared_percent_is_read_beside_the_ascii_one() {
    let arabic = INVARIANT.with_percent(symbol("\u{66A}"));
    assert_eq!(cast_f64("50\u{66A}".as_bytes(), &arabic), Ok(0.5));
    assert_eq!(cast_f64(b"50%", &arabic), Ok(0.5));
    assert_eq!(cast_f64("(2.5)\u{66A}".as_bytes(), &arabic), Ok(-0.025));
    assert_eq!(cast_f64("50 \u{66A}".as_bytes(), &arabic), Ok(0.5));
    assert_eq!(
        cast_decimal("50\u{66A}".as_bytes(), &arabic).map(|d| d.to_string()),
        Ok("0.5".into())
    );
    assert_eq!(span(cast_f64("50\u{66A}".as_bytes(), &INVARIANT)), (Reason::Malformed, 2, 2));
    // Real doors only, and only with the flag, exactly as `%`.
    assert_eq!(span(cast_i32("50\u{66A}".as_bytes(), &arabic)), (Reason::Malformed, 2, 2));
    let strict = NumFormat { flags: 0, ..arabic };
    assert_eq!(span(cast_f64("50\u{66A}".as_bytes(), &strict)), (Reason::Malformed, 2, 2));
}

#[test]
fn native_digits_are_read_beside_ascii_ones() {
    let arabic_indic = INVARIANT.with_digits(NativeDigits::new('\u{660}').unwrap());
    assert_eq!(cast_i32("\u{661}\u{662}\u{663}".as_bytes(), &arabic_indic), Ok(123));
    assert_eq!(cast_i32("-\u{661}\u{662}\u{663}".as_bytes(), &arabic_indic), Ok(-123));
    assert_eq!(cast_i32("\u{661},\u{662}\u{663}\u{664}".as_bytes(), &arabic_indic), Ok(1234));
    assert_eq!(cast_i32("1\u{662}3".as_bytes(), &arabic_indic), Ok(123));
    assert_eq!(cast_i32("\u{661}e\u{663}".as_bytes(), &arabic_indic), Ok(1000));
    assert_eq!(cast_f64("\u{661}\u{662}.\u{665}".as_bytes(), &arabic_indic), Ok(12.5));
    assert_eq!(cast_f64("\u{661}.\u{665}e\u{661}".as_bytes(), &arabic_indic), Ok(15.0));
    assert_eq!(
        cast_decimal("\u{661}\u{662}.\u{665}".as_bytes(), &arabic_indic).map(|d| d.to_string()),
        Ok("12.5".into())
    );
    assert_eq!(cast_u8("\u{662}\u{665}\u{665}".as_bytes(), &arabic_indic), Ok(255));
    assert_eq!(
        reason(cast_u8("\u{662}\u{665}\u{666}".as_bytes(), &arabic_indic)),
        Reason::OutOfRange
    );
    // The block's ten and no more: the character after nine is no digit, nor is the one
    // before zero, and an undeclared block is malformed in its own width.
    assert_eq!(span(cast_i32("\u{66A}".as_bytes(), &arabic_indic)), (Reason::Malformed, 0, 2));
    assert_eq!(span(cast_i32("\u{65F}".as_bytes(), &arabic_indic)), (Reason::Malformed, 0, 2));
    assert_eq!(
        span(cast_i32("\u{661}\u{662}\u{663}".as_bytes(), &INVARIANT)),
        (Reason::Malformed, 0, 2)
    );
    // Three-byte digits, and the fullwidth block.
    let fullwidth = INVARIANT.with_digits(NativeDigits::new('\u{FF10}').unwrap());
    assert_eq!(cast_i32("\u{FF11}\u{FF12}\u{FF13}".as_bytes(), &fullwidth), Ok(123));
    assert_eq!(span(cast_i32("\u{FF11}x".as_bytes(), &fullwidth)), (Reason::Malformed, 3, 1));
    // A group separator must still sit between digits, native ones included.
    assert_eq!(span(cast_i32("\u{661},".as_bytes(), &arabic_indic)), (Reason::Malformed, 2, 1));
    assert_eq!(span(cast_i32(",\u{661}".as_bytes(), &arabic_indic)), (Reason::Malformed, 0, 1));
    // A zero that is ASCII, or whose block would cross a UTF-8 boundary, declares nothing.
    assert!(NativeDigits::new('0').is_none());
    assert!(NativeDigits::new('\u{67F}').is_none());
    assert_eq!(NativeDigits::new('\u{660}').unwrap().zero(), Some('\u{660}'));
    assert_eq!(NativeDigits::NONE.zero(), None);
}

#[test]
fn a_separator_may_be_wider_than_one_character() {
    let wide = NumFormat::with_separators(symbol("."), symbol(", "), NumFormat::ALL);
    assert_eq!(cast_i32(b"1, 234", &wide), Ok(1234));
    assert_eq!(cast_f64(b"1, 234.5", &wide), Ok(1234.5));
    assert_eq!(span(cast_i32(b"1,234", &wide)), (Reason::Malformed, 1, 1));
    assert_eq!(span(cast_i32(b"1, x", &wide)), (Reason::Malformed, 1, 2));
    let wide_decimal = NumFormat::with_separators(symbol("'."), symbol(","), NumFormat::ALL);
    assert_eq!(cast_f64(b"1'.5", &wide_decimal), Ok(1.5));
    assert_eq!(span(cast_i32(b"1'.5", &wide_decimal)), (Reason::Malformed, 1, 2));
    assert_eq!(
        cast_decimal(b"1,234'.5", &wide_decimal).map(|d| d.to_string()),
        Ok("1234.5".into())
    );
    // NONE keeps the invariant separator; a two-character ASCII group separator sends
    // the plain shape through the engine, to the same answer.
    assert_eq!(NumFormat::with_separators(Symbol::NONE, Symbol::NONE, NumFormat::ALL), INVARIANT);
    let odd = NumFormat::with_separators(symbol("."), symbol("e."), NumFormat::ALL);
    assert_eq!(cast_f64(b"1e3", &odd), Ok(1000.0));
    assert_eq!(cast_i32(b"1e3", &odd), Ok(1000));
    assert!(Symbol::<8>::new("").is_none());
    assert!(Symbol::<8>::new("123456789").is_none());
    assert!(Symbol::<8>::new("x1").is_none());
    assert_eq!(symbol("\u{202F}").as_str(), "\u{202F}");
}

#[test]
fn declared_day_periods_are_read_beside_am_and_pm_on_either_side_of_the_clock() {
    let german = DayPeriods::new(Symbol::new("vorm.").unwrap(), Symbol::new("nachm.").unwrap());
    let korean = DayPeriods::new(
        Symbol::new("\u{C624}\u{C804}").unwrap(),
        Symbol::new("\u{C624}\u{D6C4}").unwrap(),
    );
    let dmy = DateOrder::DayMonthYear;
    let ymd = DateOrder::YearMonthDay;
    let at = |hour: u64, minute: u64| (hour * 3600 + minute * 60) * 1_000_000_000;
    let nanos = |verdict: Result<CivilDateTime, Fault>| verdict.map(|value| value.nanos_of_day);
    assert_eq!(nanos(cast_datetime_with(b"1/7/2026 3:04 nachm.", dmy, &german)), Ok(at(15, 4)));
    assert_eq!(nanos(cast_datetime_with(b"1/7/2026 3:04 vorm.", dmy, &german)), Ok(at(3, 4)));
    assert_eq!(nanos(cast_datetime_with(b"1/7/2026 3 nachm.", dmy, &german)), Ok(at(15, 0)));
    assert_eq!(nanos(cast_datetime_with(b"1/7/2026 12:00 vorm.", dmy, &german)), Ok(at(0, 0)));
    assert_eq!(nanos(cast_datetime_with(b"1/7/2026 3:04 PM", dmy, &german)), Ok(at(15, 4)));
    assert_eq!(
        nanos(cast_datetime_with("2026-01-07 \u{C624}\u{D6C4} 3:04".as_bytes(), ymd, &korean)),
        Ok(at(15, 4))
    );
    assert_eq!(
        nanos(cast_datetime_with("2026-01-07 \u{C624}\u{D6C4}3:04".as_bytes(), ymd, &korean)),
        Ok(at(15, 4))
    );
    assert_eq!(
        nanos(cast_datetime_with("2026-01-07 \u{C624}\u{C804} 12:00".as_bytes(), ymd, &korean)),
        Ok(at(0, 0))
    );
    assert_eq!(
        nanos(cast_datetime_with("2026-01-07T\u{C624}\u{D6C4} 3".as_bytes(), ymd, &korean)),
        Ok(at(15, 0))
    );
    // A designator with a space inside it (es: `a. m.`), and one that begins with the
    // English pair, which is tried after it.
    let spanish = DayPeriods::new(Symbol::new("a. m.").unwrap(), Symbol::new("p. m.").unwrap());
    assert_eq!(nanos(cast_datetime_with(b"1/7/2026 3:04 p. m.", dmy, &spanish)), Ok(at(15, 4)));
    let dotted = DayPeriods::new(Symbol::new("AM.").unwrap(), Symbol::new("PM.").unwrap());
    assert_eq!(nanos(cast_datetime_with(b"1/7/2026 3:04 PM.", dmy, &dotted)), Ok(at(15, 4)));
    assert_eq!(nanos(cast_datetime_with(b"1/7/2026 3:04 PM", dmy, &dotted)), Ok(at(15, 4)));
    // English ahead of the clock reads too, with nothing declared; a second marker is
    // trailing text, and the clock under a marker is a 12-hour one.
    assert_eq!(nanos(cast_datetime(b"1/7/2026 PM 3:04", dmy)), Ok(at(15, 4)));
    assert_eq!(span(cast_datetime(b"1/7/2026 PM 3:04 PM", dmy)), (Reason::Malformed, 16, 1));
    assert_eq!(span(cast_datetime(b"1/7/2026 PM 13:04", dmy)), (Reason::OutOfRange, 12, 2));
    // Undeclared, a designator is trailing text at the space before it.
    assert_eq!(span(cast_datetime(b"1/7/2026 3:04 nachm.", dmy)), (Reason::Malformed, 13, 1));
    assert_eq!(
        cast_datetime(b"1/7/2026 3:04 PM", dmy),
        cast_datetime_with(b"1/7/2026 3:04 PM", dmy, &DayPeriods::ENGLISH)
    );
    // The ABI's own rules for a designator: a digit, or whitespace at an edge.
    let raw =
        |am: &str| RawDayPeriods::from(DayPeriods::new(Symbol::new(am).unwrap(), Symbol::NONE));
    assert_eq!(
        raw("vorm.").resolve(),
        Some(&DayPeriods::new(Symbol::new("vorm.").unwrap(), Symbol::NONE))
    );
    assert_eq!(raw("a. m.").resolve().map(|p| p.am.as_str().to_string()), Some("a. m.".into()));
    assert_eq!(raw(" am").resolve(), None);
    assert_eq!(raw("am ").resolve(), None);
    for edge in ["\u{202F}PM", "PM\u{A0}", "\u{0B}am", "am\u{3000}", "\u{85}", "\u{2028}p"] {
        assert_eq!(raw(edge).resolve(), None, "{edge:?}");
    }
    assert!(raw("a.\u{202F}m.").resolve().is_some());
    assert!(raw("am\u{200B}").resolve().is_some());
    assert!(Symbol::<16>::new("a1").is_none());
    assert_eq!(RawDayPeriods { size: 3, ..RawDayPeriods::ZERO }.resolve(), None);
    assert_eq!(RawDayPeriods { size: 41, ..RawDayPeriods::ZERO }.resolve(), None);
    assert_eq!(
        RawDayPeriods { size: 4, ..RawDayPeriods::ZERO }.resolve(),
        Some(&DayPeriods::ENGLISH)
    );
    assert_eq!(RawDayPeriods::from(german).resolve(), Some(&german));
}

// --- ABI codes ---

/// The 92 bytes of a layout, built by hand: the size word, the flags, and whatever
/// `fill` writes at the offsets the bindings use.
fn layout(flags: u32, fill: impl FnOnce(&mut [u8; 92])) -> RawNumFormat {
    let mut bytes = [0u8; 92];
    bytes[..4].copy_from_slice(&92u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&flags.to_le_bytes());
    // The invariant separators, which a caller always declares.
    bytes[12] = b'.';
    bytes[20] = 1;
    bytes[21] = b',';
    bytes[29] = 1;
    fill(&mut bytes);
    RawNumFormat::from_le_bytes(bytes)
}

#[test]
fn a_format_crosses_as_its_bytes_and_comes_back() {
    let format =
        NumFormat::new(',', '.', NumFormat::ALL).with_currency(CurrencySymbol::new("€").unwrap());
    let raw = RawNumFormat::from(format);
    assert_eq!(raw.resolve(), Some(&format));
    let bytes = raw.to_le_bytes();
    // The layout's own order: the size word, the flags, the digit zero, then each
    // symbol's bytes followed by its length.
    assert_eq!(&bytes[..4], &92u32.to_le_bytes());
    assert_eq!(&bytes[4..8], &NumFormat::ALL.to_le_bytes());
    assert_eq!(&bytes[8..12], &0u32.to_le_bytes());
    assert_eq!((bytes[12], bytes[20]), (b',', 1));
    assert_eq!((bytes[21], bytes[29]), (b'.', 1));
    assert!(bytes[30..75].iter().all(|&byte| byte == 0));
    assert_eq!(&bytes[75..78], "€".as_bytes());
    assert!(bytes[78..91].iter().all(|&byte| byte == 0));
    assert_eq!(bytes[91], 3);
    assert_eq!(RawNumFormat::from_le_bytes(bytes), raw);
    assert_eq!(RawNumFormat::from(NumFormat::INVARIANT).resolve(), Some(&NumFormat::INVARIANT));
    assert_eq!(layout(NumFormat::ALL, |_| {}).resolve(), Some(&NumFormat::INVARIANT));
}

#[test]
fn a_crossing_currency_symbol_is_held_to_the_same_rules_as_a_declared_one() {
    let raw = |symbol: &[u8], len: u8, tail: u8| {
        layout(NumFormat::ALL, |bytes| {
            bytes[75..91].fill(tail);
            bytes[75..75 + symbol.len()].copy_from_slice(symbol);
            bytes[91] = len;
        })
    };
    let declared =
        |symbol: &str| NumFormat::INVARIANT.with_currency(CurrencySymbol::new(symbol).unwrap());
    // Bytes past the length are no part of the symbol, whatever a binding left there.
    for tail in [0, b'7', b' ', 0xFF] {
        assert_eq!(raw(b"$", 1, tail).resolve(), Some(&declared("$")));
        assert_eq!(raw("¤".as_bytes(), 2, tail).resolve(), Some(&declared("¤")));
        assert_eq!(raw("руб.".as_bytes(), 7, tail).resolve(), Some(&declared("руб.")));
    }
    let sixteen = "ABCDEFGHIJKLMNOP";
    assert_eq!(raw(sixteen.as_bytes(), 16, 0).resolve(), Some(&declared(sixteen)));
    // The rules `CurrencySymbol::new` enforces, each on its own: a digit, every ASCII
    // whitespace byte at an edge (inside, it is allowed), broken UTF-8 (a lone lead byte,
    // and a symbol cut mid-character), and a length past the inline capacity.
    assert_eq!(raw(b"US1", 3, 0).resolve(), None);
    for space in *b" \t\n\x0C\r" {
        assert_eq!(
            raw(&[b'k', space, b'r'], 3, 0).resolve(),
            Some(&declared(&format!("k{}r", space as char)))
        );
        assert_eq!(raw(&[space, b'k'], 2, 0).resolve(), None);
        assert_eq!(raw(&[b'k', space], 2, 0).resolve(), None);
    }
    assert_eq!(raw(&[0xC2], 1, 0).resolve(), None);
    assert_eq!(raw("€".as_bytes(), 2, 0).resolve(), None);
    assert_eq!(raw(b"$", 17, 0).resolve(), None);
    // A vertical tab is white space to the core, so it may not end a symbol.
    assert_eq!(raw(&[b'k', 0x0B], 2, 0).resolve(), None);
    // Nor may any other white-space code point start or end one, while any may sit inside
    // (`F CFA`); U+200B is not one, so it stays legal anywhere.
    for space in ["\u{A0}k", "k\u{85}", "\u{202F}k", "k\u{202F}", "k\u{3000}", "\u{A0}"] {
        assert_eq!(raw(space.as_bytes(), space.len() as u8, 0).resolve(), None);
        assert!(CurrencySymbol::new(space).is_none());
    }
    for inside in ["k\u{A0}r", "F\u{202F}CFA", "F CFA", "k\u{85}r", "k\u{3000}r", "a\u{2009} b"] {
        assert_eq!(
            raw(inside.as_bytes(), inside.len() as u8, 0).resolve(),
            Some(&declared(inside))
        );
        assert_eq!(CurrencySymbol::new(inside).unwrap().as_str(), inside);
    }
    assert!(CurrencySymbol::new("k\u{0B}").is_none());
    assert_eq!(raw("k\u{200B}".as_bytes(), 4, 0).resolve(), Some(&declared("k\u{200B}")));
}

#[test]
fn a_format_declaration_that_the_input_could_never_match_is_refused() {
    let symbol = |text| Symbol::<8>::new(text).unwrap();
    let base = NumFormat::new('.', ',', NumFormat::ALL);
    assert!(base.check());
    let spaces = ["\u{A0}", "\u{202F}", " ", "\t", "\u{0B}", "\u{2003}", "-\u{A0}", "\u{A0}%"];
    for space in spaces {
        // A sign or the percent symbol sits flush against the digits.
        assert!(!NumFormat { minus_sign: symbol(space), ..base }.check(), "{space:?}");
        assert!(!NumFormat { plus_sign: symbol(space), ..base }.check(), "{space:?}");
        assert!(!NumFormat { percent: symbol(space), ..base }.check(), "{space:?}");
    }
    for space in ["\u{A0}", "\u{202F}", " ", ",\u{A0}", "\u{A0},"] {
        // No numeral admits white space as a decimal separator.
        assert!(!NumFormat { decimal_sep: symbol(space), ..base }.check(), "{space:?}");
        assert!(
            !NumFormat { currency_decimal_sep: symbol(space), ..base }.check(),
            "currency {space:?}"
        );
        // A group separator may be or contain it.
        assert!(NumFormat { group_sep: symbol(space), ..base }.check(), "{space:?}");
        assert!(
            NumFormat { currency_group_sep: symbol(space), ..base }.check(),
            "currency {space:?}"
        );
    }
    // A currency symbol may hold white space inside (`F CFA`, as CLDR writes it), never at
    // an edge, where the doors ignore it.
    let currency = |text| NumFormat { currency: CurrencySymbol::new(text).unwrap(), ..base };
    for inside in ["F\u{202F}CFA", "F\u{A0}CFA", "F CFA"] {
        assert!(currency(inside).check(), "{inside:?}");
    }
    let raw_currency = |text: &str| {
        let mut bytes = RawNumFormat::from(base).to_le_bytes();
        bytes[75..75 + text.len()].copy_from_slice(text.as_bytes());
        bytes[91] = text.len() as u8;
        RawNumFormat::from_le_bytes(bytes)
    };
    for edge in ["\u{A0}\u{20AC}", "\u{20AC}\u{A0}", "\u{202F}F", "F ", " F CFA", "F CFA\u{3000}"] {
        assert!(CurrencySymbol::new(edge).is_none(), "{edge:?}");
        let raw = raw_currency(edge);
        assert!(!raw.format.check(), "{edge:?}");
        assert!(raw.resolve().is_none(), "{edge:?}");
    }
    // The earlier rules stand: the digit is refused before white space is looked for.
    assert!(!NumFormat::new('.', '1', NumFormat::ALL).check());
    assert!(!NumFormat { decimal_sep: symbol(" "), group_sep: symbol(" "), ..base }.check());
}

#[test]
fn a_crossing_format_declares_each_field_or_keeps_the_default_for_it() {
    let symbol = |text| Symbol::<8>::new(text).unwrap();
    // Every field declared crosses and comes back as declared, by bytes and by value.
    let declared = NumFormat::new(',', '\u{202F}', NumFormat::ALL | NumFormat::TRAILING_SIGN)
        .with_currency_separators(symbol("."), symbol("'"))
        .with_signs(symbol("\u{2212}"), symbol("\u{FF0B}"))
        .with_percent(symbol("\u{66A}"))
        .with_digits(NativeDigits::new('\u{660}').unwrap())
        .with_currency(CurrencySymbol::new("kr.").unwrap());
    let raw = RawNumFormat::from(declared);
    assert_eq!(raw.resolve(), Some(&declared));
    assert_eq!(RawNumFormat::from_le_bytes(raw.to_le_bytes()), raw);
    assert_eq!(RawNumFormat::from_le_bytes(raw.to_le_bytes()).resolve(), Some(&declared));
    let bytes = raw.to_le_bytes();
    assert_eq!(&bytes[8..12], &0x660u32.to_le_bytes());
    assert_eq!((&bytes[21..24], bytes[29]), ("\u{202F}".as_bytes(), 3));
    assert_eq!((bytes[30], bytes[38]), (b'.', 1));
    assert_eq!((bytes[39], bytes[47]), (b'\'', 1));
    assert_eq!((&bytes[48..51], bytes[56]), ("\u{2212}".as_bytes(), 3));
    assert_eq!((&bytes[57..60], bytes[65]), ("\u{FF0B}".as_bytes(), 3));
    assert_eq!((&bytes[66..68], bytes[74]), ("\u{66A}".as_bytes(), 2));
    assert_eq!((&bytes[75..78], bytes[91]), (b"kr.".as_slice(), 3));
    // Each field's own rule: a missing or over-long number separator, equal separators
    // (number or currency), a digit in any symbol, whitespace in a sign or the percent
    // (but not in a separator), an ASCII zero, a block that would cross a UTF-8
    // boundary, and a size the core does not read.
    let all = NumFormat::ALL;
    assert_eq!(layout(all, |b| b[20] = 0).resolve(), None);
    assert_eq!(layout(all, |b| b[29] = 9).resolve(), None);
    assert_eq!(layout(all, |b| b[21] = b'.').resolve(), None);
    assert_eq!(
        layout(all, |b| {
            b[30] = b',';
            b[38] = 1;
        })
        .resolve(),
        None
    );
    assert!(
        layout(all, |b| {
            b[30] = b',';
            b[38] = 1;
            b[39] = b'.';
            b[47] = 1;
        })
        .resolve()
        .is_some()
    );
    assert_eq!(
        layout(all, |b| {
            b[21] = b'1';
        })
        .resolve(),
        None
    );
    assert_eq!(
        layout(all, |b| {
            b[48] = b'1';
            b[56] = 1;
        })
        .resolve(),
        None
    );
    assert_eq!(
        layout(all, |b| {
            b[48] = b' ';
            b[56] = 1;
        })
        .resolve(),
        None
    );
    assert_eq!(
        layout(all, |b| {
            b[66] = b' ';
            b[74] = 1;
        })
        .resolve(),
        None
    );
    assert!(
        layout(all, |b| {
            b[21] = b' ';
        })
        .resolve()
        .is_some()
    );
    assert_eq!(
        layout(all, |b| b[8..12].copy_from_slice(&u32::from(b'0').to_le_bytes())).resolve(),
        None
    );
    assert_eq!(layout(all, |b| b[8..12].copy_from_slice(&0x67Fu32.to_le_bytes())).resolve(), None);
    assert_eq!(
        layout(all, |b| b[48..51].copy_from_slice(&[0xE2, 0x88, 0x92])).resolve(),
        Some(&NumFormat::INVARIANT)
    );
    for size in [0u32, 29, 93] {
        assert_eq!(layout(all, |b| b[..4].copy_from_slice(&size.to_le_bytes())).resolve(), None);
    }
    assert_eq!(
        layout(all, |b| b[..4].copy_from_slice(&30u32.to_le_bytes())).resolve(),
        Some(&NumFormat::INVARIANT)
    );
}

/// The C ABI reads a caller's layout in place, and a shorter one — a caller built
/// against an older layout — over zeros.
#[cfg(feature = "exports")]
#[test]
fn the_abi_reads_a_format_of_the_size_its_caller_declares() {
    #[repr(C)]
    struct RawFault {
        offset: u32,
        len: u32,
    }
    unsafe extern "C" {
        fn cast_i32(
            ptr: *const u8,
            len: usize,
            format: *const RawNumFormat,
            out: *mut i32,
            fault: *mut RawFault,
        ) -> i32;
    }
    let door = |text: &[u8], format: *const RawNumFormat| {
        let mut out = 0i32;
        let mut fault = RawFault { offset: 0, len: 0 };
        // SAFETY: the pointers are live for the call; `format` is a whole layout or null.
        let code = unsafe { cast_i32(text.as_ptr(), text.len(), format, &mut out, &mut fault) };
        (code, out, fault.offset, fault.len)
    };
    assert_eq!(door(b"1,234", core::ptr::null()), (0, 1234, 0, 0));
    let trailing = layout(NumFormat::ALL | NumFormat::TRAILING_SIGN, |_| {});
    assert_eq!(door(b"1,234-", &trailing), (0, -1234, 0, 0));
    assert_eq!(door(b"1,234-", &layout(NumFormat::ALL, |_| {})), (2, 0, 5, 1));
    // Thirty bytes: the separators and nothing after them, read as if the rest were
    // zero — so a trailing sign flag works and a currency symbol is not there.
    let shorter = layout(NumFormat::ALL | NumFormat::TRAILING_SIGN, |b| {
        b[..4].copy_from_slice(&30u32.to_le_bytes());
        b[75] = b'$';
        b[91] = 1;
    });
    assert_eq!(door(b"$5", &shorter), (2, 0, 0, 1));
    assert_eq!(door(b"5-", &shorter), (0, -5, 0, 0));
    // A size the core does not know is the caller's bug.
    for size in [0u32, 29, 93] {
        let bad = layout(NumFormat::ALL, |b| b[..4].copy_from_slice(&size.to_le_bytes()));
        assert_eq!(door(b"5", &bad), (-1, 0, 0, 0));
    }
    let eurozone = layout(NumFormat::ALL, |b| {
        b[12] = b',';
        b[21] = b'.';
    });
    assert_eq!(door(b"1.234", &eurozone), (0, 1234, 0, 0));
}

#[test]
fn every_code_decodes_to_the_member_it_names_and_nothing_else_decodes() {
    for precision in
        [UnixPrecision::Seconds, UnixPrecision::Millis, UnixPrecision::Micros, UnixPrecision::Nanos]
    {
        assert_eq!(UnixPrecision::from_code(precision as u32), Some(precision));
    }
    for epoch in [ExcelEpoch::Y1900, ExcelEpoch::Y1904] {
        assert_eq!(ExcelEpoch::from_code(epoch as u32), Some(epoch));
    }
    for order in [DateOrder::YearMonthDay, DateOrder::MonthDayYear, DateOrder::DayMonthYear] {
        assert_eq!(DateOrder::from_code(order as u32), Some(order));
    }
    for reason in [Reason::Empty, Reason::Malformed, Reason::OutOfRange] {
        assert_eq!(Reason::from_code(reason as u32), Some(reason));
    }
    for outside in [0, 5, u32::MAX] {
        assert_eq!(UnixPrecision::from_code(outside), None);
        assert_eq!(ExcelEpoch::from_code(outside), None);
        assert_eq!(DateOrder::from_code(outside), None);
        assert_eq!(Reason::from_code(outside), None);
    }
    assert_eq!(ExcelEpoch::from_code(3), None);
    assert_eq!(DateOrder::from_code(4), None);
    assert_eq!(Reason::from_code(4), None);
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
    assert_eq!(cast_unix(b"-1", UnixPrecision::Seconds), Ok(Timestamp { seconds: -1, nanos: 0 }));
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
    assert_eq!(
        cast_unix(b"253402300799", UnixPrecision::Seconds).unwrap().seconds,
        MAX_TIMESTAMP_SECONDS
    );
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
    assert_eq!(reason(cast_excel_serial(b"60", ExcelEpoch::Y1900)), Reason::OutOfRange);

    let feb28 = cast_excel_serial(b"59", ExcelEpoch::Y1900).unwrap().seconds;
    let mar01 = cast_excel_serial(b"61", ExcelEpoch::Y1900).unwrap().seconds;
    assert_eq!(mar01 - feb28, 86_400, "59 and 61 are adjacent real days");

    // The same date reached through the text door, which calls 1900-02-29 out of range
    // too — the two doors agree that day does not exist.
    assert_eq!(cast_date(b"1900-02-28"), Ok(Date { year: 1900, month: 2, day: 28 }));
    assert_eq!(reason(cast_date(b"1900-02-29")), Reason::OutOfRange);

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

// --- typed doors ---

#[test]
fn excel_time_reads_the_fraction_of_any_serial() {
    assert_eq!(excel_time(0.75), Ok(64_800_000_000_000));
    assert_eq!(excel_time(45_292.75), Ok(64_800_000_000_000));
    assert_eq!(excel_time(0.0), Ok(0));
    // Rounds as `excel_serial` does, and a fraction that rounds to a whole day is midnight.
    assert_eq!(excel_time(1.0 - f64::EPSILON / 2.0), Ok(0));
    assert_eq!(excel_time(-0.5), Err(Reason::Malformed));
    assert_eq!(excel_time(f64::NAN), Err(Reason::Malformed));
    assert_eq!(excel_time(3_000_000.0), Err(Reason::OutOfRange));
}

#[test]
fn excel_duration_reads_days_sign_and_all() {
    assert_eq!(excel_duration(1.5), Ok(Duration { seconds: 129_600, nanos: 0 }));
    assert_eq!(excel_duration(-0.25), Ok(Duration { seconds: -21_600, nanos: 0 }));
    // A tenth of a second, and its negation, same-signed.
    let tenth = 0.1 / 86_400.0;
    assert_eq!(excel_duration(tenth), Ok(Duration { seconds: 0, nanos: 100_000_000 }));
    assert_eq!(excel_duration(-tenth), Ok(Duration { seconds: 0, nanos: -100_000_000 }));
    let window = MAX_DURATION_SECONDS as f64 / 86_400.0;
    assert_eq!(excel_duration(window).map(|d| d.seconds), Ok(MAX_DURATION_SECONDS));
    assert_eq!(excel_duration(window * 1.000_001), Err(Reason::OutOfRange));
    assert_eq!(excel_duration(f64::INFINITY), Err(Reason::Malformed));
}

#[test]
fn bool_and_f32_from_a_double() {
    assert_eq!(bool_from_f64(1.0), Ok(true));
    assert_eq!(bool_from_f64(0.0), Ok(false));
    assert_eq!(bool_from_f64(-0.0), Ok(false));
    assert_eq!(bool_from_f64(2.0), Err(Reason::Malformed));
    assert_eq!(bool_from_f64(0.5), Err(Reason::Malformed));
    assert_eq!(bool_from_f64(f64::NAN), Err(Reason::Malformed));

    assert_eq!(f32_from_f64(0.1), Ok(0.1f32));
    assert_eq!(f32_from_f64(f64::from(f32::MAX)), Ok(f32::MAX));
    assert_eq!(f32_from_f64(1e39), Err(Reason::OutOfRange));
    assert_eq!(f32_from_f64(f64::NEG_INFINITY), Err(Reason::OutOfRange));
    assert_eq!(f32_from_f64(f64::NAN), Err(Reason::Malformed));
    assert_eq!(f32_from_f64(1e-50), Ok(0.0));
    // No negative zero comes out, whether the double is one or narrows to one; by bits,
    // since `-0.0 == 0.0`.
    let bits = |value| f32_from_f64(value).map(f32::to_bits);
    assert_eq!(bits(-0.0), Ok(0));
    assert_eq!(bits(-1e-300), Ok(0));
    assert_eq!(bits(-1e-50), Ok(0));
    assert_eq!(bits(-1.5), Ok((-1.5f32).to_bits()));
}

#[test]
fn a_double_is_the_decimal_it_is_written_as() {
    let decimal = |value| decimal_from_f64(value).map(|d| d.to_string());
    assert_eq!(decimal(2.5).as_deref(), Ok("2.5"));
    assert_eq!(decimal(0.1).as_deref(), Ok("0.1"));
    assert_eq!(decimal(0.1 + 0.2).as_deref(), Ok("0.30000000000000004"));
    assert_eq!(decimal(-1234.5).as_deref(), Ok("-1234.5"));
    assert_eq!(decimal(1e20).as_deref(), Ok("100000000000000000000"));
    assert_eq!(decimal(1e-28).as_deref(), Ok("0.0000000000000000000000000001"));
    assert_eq!(decimal(1e-29), Err(Reason::OutOfRange));
    assert_eq!(decimal(1e29), Err(Reason::OutOfRange));
    assert_eq!(i64_from_f64(9_223_372_036_854_775_808.0), Err(Reason::OutOfRange));
    assert_eq!(u64_from_f64(9_223_372_036_854_775_808.0), Ok(9_223_372_036_854_776_000));
    assert_eq!(i32_from_f64(2.5), Err(Reason::Malformed));
    assert_eq!(u8_from_f64(-0.0), Ok(0));
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
    for text in [
        "1/2/2026",
        "2026-01-02T00:00:00",
        "2026/01/02",
        "garbage",
        "2026-1x-01",
        "2026-001-01",
        "0000-1x-01",
    ] {
        assert_eq!(reason(cast_date(text.as_bytes())), Reason::Malformed, "{text}");
    }
    // Shaped like a date but naming none: out of range, at the field that is wrong.
    for (text, at) in [
        ("0000-01-01", (0, 4)),
        ("2026-00-01", (5, 2)),
        ("2026-13-01", (5, 2)),
        ("2026-01-00", (8, 2)),
        ("2026-01-32", (8, 2)),
        ("2026-02-29", (8, 2)),
        ("1900-02-29", (8, 2)),
    ] {
        assert_eq!(cast_date(text.as_bytes()), Err(Fault::out_of_range(at.0, at.1)), "{text}");
    }
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
    // 13 can only be a day — valid day-first, out of range month-first, span on the field.
    assert_eq!(
        cast_date_ordered(b"13/1/2026", DateOrder::DayMonthYear),
        Ok(Date { year: 2026, month: 1, day: 13 })
    );
    assert_eq!(
        cast_date_ordered(b"13/1/2026", DateOrder::MonthDayYear),
        Err(Fault::out_of_range(0, 2))
    );
    // Real calendar, same as the strict door.
    assert_eq!(
        cast_date_ordered(b"29/2/2024", DateOrder::DayMonthYear),
        Ok(Date { year: 2024, month: 2, day: 29 })
    );
    assert_eq!(
        cast_date_ordered(b"29/2/2026", DateOrder::DayMonthYear),
        Err(Fault::out_of_range(0, 2))
    );
    // Year 0000 points at the year field wherever the order puts it.
    assert_eq!(
        cast_date_ordered(b"1/7/0000", DateOrder::MonthDayYear),
        Err(Fault::out_of_range(4, 4))
    );
}

#[test]
fn date_ordered_rejects_ambiguity_reintroducers() {
    // Two-digit years mean century guessing — never.
    assert_eq!(reason(cast_date_ordered(b"1/7/26", DateOrder::MonthDayYear)), Reason::Malformed);
    // A three-digit field fits no order — faulted at its own digits.
    assert_eq!(
        reason(cast_date_ordered(b"123/4/2026", DateOrder::MonthDayYear)),
        Reason::Malformed
    );
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
    // A bare trailing number and a zone suffix (this door reads no zone and invents
    // none — RFC 3339 instants go through cast_timestamp).
    for text in [
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
    // Well-formed fields that name nothing: a meridiem hour outside 1–12, a 24-hour hour
    // past 23, a minute or second past 59, year 0000 — out of range, at the field.
    for (text, at) in [
        ("1/7/2026 13:04 PM", (9, 2)),
        ("1/7/2026 0 AM", (9, 1)),
        ("1/7/2026 25:04", (9, 2)),
        ("1/7/2026 3:60 PM", (11, 2)),
        ("1/7/2026 3:04:60", (14, 2)),
        ("1/7/0000 3:04 PM", (4, 4)),
    ] {
        assert_eq!(
            cast_datetime(text.as_bytes(), DateOrder::MonthDayYear),
            Err(Fault::out_of_range(at.0, at.1)),
            "{text}"
        );
    }
    // Shape is checked across the whole text first: the zone suffix is reported, not
    // the hour that would also be wrong.
    assert_eq!(
        reason(cast_datetime(b"1/7/2026 25:04Z", DateOrder::MonthDayYear)),
        Reason::Malformed
    );
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
    // Excel's strict writer: seventeen digits, truncated to nine, no carry into the day.
    assert_eq!(cast_time(b"15:04:05.00000000000312325"), Ok(54_245_000_000_000));
    assert_eq!(cast_time(b"23:59:59.9999999999"), Ok(86_399_999_999_999));
}

#[test]
fn time_rejects_non_iso_readings() {
    for text in ["3:04:05 PM", "noon", "1:04", "25:00x"] {
        assert_eq!(reason(cast_time(text.as_bytes())), Reason::Malformed, "{text}");
    }
    for (text, at) in
        [("24:00", (0, 2)), ("25:00", (0, 2)), ("15:60", (3, 2)), ("15:04:60", (6, 2))]
    {
        assert_eq!(cast_time(text.as_bytes()), Err(Fault::out_of_range(at.0, at.1)), "{text}");
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
    assert_eq!(cast_duration(b"25:00:00"), Err(Fault::out_of_range(0, 2)));
    assert_eq!(cast_duration(b"-1.24:00:00"), Err(Fault::out_of_range(3, 2)));
    assert_eq!(cast_duration(b"01:60:00"), Err(Fault::out_of_range(3, 2)));
    assert_eq!(cast_duration(b"01:30:60"), Err(Fault::out_of_range(6, 2)));
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
    assert_eq!(
        cast_u8("256", &INVARIANT).unwrap_err().to_string(),
        "out of range input at bytes 0..3"
    );
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

#[test]
fn optional_checked_presents_empty_as_absent_and_nothing_else() {
    let bound = |text| decimal_for_declaration(text).unwrap();
    let rules = NumericRules::new().minimum(bound("-90")).maximum(bound("90"));
    let latitude = Constraints::<f64>::numeric(rules, None).unwrap();
    let check = |text: &str| cast_checked(text, &NumFormat::INVARIANT, &latitude);
    // A blank cell is absent: Empty, and only Empty.
    assert_eq!(optional_checked(check("")), Ok(None));
    assert_eq!(optional_checked(check("   ")), Ok(None));
    // A good value is present, with what the checked door said about it.
    assert_eq!(optional_checked(check("45.5")), Ok(Some(Checked::new(45.5, false, None))));
    // Bad data stays the door's fault.
    match optional_checked(check("north")) {
        Err(Rejection::Fault(fault)) => assert_eq!(fault.reason, Reason::Malformed),
        other => panic!("expected a Malformed fault, got {other:?}"),
    }
    // A broken rule stays a violation: never mistaken for a missing value.
    match optional_checked(check("91")) {
        Err(Rejection::Violation(violation)) => assert_eq!(violation.keyword, Keyword::Maximum),
        other => panic!("expected a maximum violation, got {other:?}"),
    }
    // The text door's empty is absent too; a match keeps its enum index.
    let sexes = Constraints::text(TextRules::new().trim(true), Some(&["Female", "Male"])).unwrap();
    assert_eq!(optional_checked(cast_text_checked(b"  ", &sexes)), Ok(None));
    assert_eq!(
        optional_checked(cast_text_checked(b"male", &sexes)),
        Ok(Some(Checked::new("Male", false, Some(1))))
    );
}

// ---- The lenient lane changes no verdict ----
//
// lane.rs is a second fast path in front of the numeric engines. Its whole contract is
// that it may make a cast faster and may not change one: every token it reads must get
// the value the engine gives it, and every token it declines goes to the engine
// untouched. That is held here by running each numeric door twice — as shipped, and
// with the lane switched off — over every sequence of up to four tokens drawn from the
// grammar's own vocabulary (and up to six from the handful the lane actually reads),
// under formats chosen to put a symbol where a sign, a separator, a percent, a radix
// prefix or a parenthesis would be. Faults are compared whole, spans included; reals are
// compared by bit pattern, so `-0.0` is not `0.0`.

fn lane_formats() -> Vec<NumFormat> {
    let symbol = |text| CurrencySymbol::new(text).unwrap();
    let wide = |text| Symbol::<8>::new(text).unwrap();
    let all = NumFormat::ALL;
    let usd = NumFormat::INVARIANT.with_currency(symbol("$"));
    let eurozone = NumFormat::new(',', '.', all);
    let mut formats = vec![
        usd,
        NumFormat::INVARIANT,
        eurozone.with_currency(symbol("\u{20ac}")),
        // A symbol carrying the group separator, and ones that collide with a sign, the
        // percent, a radix prefix and each parenthesis.
        eurozone.with_currency(symbol("kr.")),
        NumFormat::INVARIANT.with_currency(symbol("%")),
        NumFormat::INVARIANT.with_currency(symbol("&H")),
        NumFormat::INVARIANT.with_currency(symbol("-")),
        NumFormat::INVARIANT.with_currency(symbol("+")),
        NumFormat::INVARIANT.with_currency(symbol("(")),
        NumFormat::INVARIANT.with_currency(symbol(")")),
        NumFormat::INVARIANT.with_currency(symbol(".")),
        NumFormat::INVARIANT.with_currency(symbol(",")),
        NumFormat::INVARIANT.with_currency(symbol("e")),
        // Separators that are whitespace, non-ASCII, equal, a digit and a sign.
        NumFormat::new('.', ' ', all).with_currency(symbol("$")),
        NumFormat::new('.', '\u{a0}', all).with_currency(symbol("$")),
        NumFormat::new('.', '.', all).with_currency(symbol("$")),
        NumFormat::new('.', '1', all).with_currency(symbol("$")),
        NumFormat::new('-', ',', all).with_currency(symbol("$")),
        NumFormat::new('.', '-', all).with_currency(symbol("$")),
        NumFormat::DETECT.with_currency(symbol("$")),
        // The sweep's fields: declared signs, a trailing sign, currency separators, a
        // declared percent, native digits, and a separator wider than one byte.
        usd.with_signs(wide("\u{2212}"), wide("\u{FF0B}")),
        NumFormat { flags: all | NumFormat::TRAILING_SIGN, ..usd },
        NumFormat { flags: all | NumFormat::TRAILING_SIGN, ..usd }
            .with_signs(wide("\u{2212}"), Symbol::NONE),
        eurozone.with_currency(symbol("$")).with_currency_separators(wide("."), wide(",")),
        usd.with_currency_separators(wide(","), wide(".")),
        usd.with_percent(wide("\u{66A}")),
        usd.with_percent(wide("e")),
        usd.with_digits(NativeDigits::new('\u{660}').unwrap()),
        NumFormat::with_separators(wide("."), wide(", "), all).with_currency(symbol("$")),
        NumFormat::with_separators(wide("'."), wide(","), all).with_currency(symbol("$")),
        NumFormat::with_separators(wide("."), wide("e."), all).with_currency(symbol("$")),
    ];
    // Each lenience switched off on its own, and all of them at once.
    for flag in [
        NumFormat::GROUPING,
        NumFormat::PARENS,
        NumFormat::EXPONENT,
        NumFormat::RADIX_PREFIX,
        NumFormat::PERCENT,
        NumFormat::CURRENCY,
        all,
    ] {
        formats.push(NumFormat::new('.', ',', all & !flag).with_currency(symbol("$")));
    }
    formats
}

/// The doors' one parse with the lenient lane and without it: the same value, or the same
/// failure, detail and span included.
fn lane_agrees_with_the_engine(input: &[u8], format: &NumFormat) {
    let shown = || format!("{:?} under {format:?}", String::from_utf8_lossy(input));
    macro_rules! integers {
        ($($door:ident => $ty:ty),+) => {$(
            assert_eq!(
                integer::integer::<$ty>(input, format),
                integer::engine_only::<$ty>(
                    input, format, <$ty>::MIN as i128, <$ty>::MAX as i128, <$ty>::BITS),
                "{} on {}", stringify!($door), shown()
            );
        )+};
    }
    integers!(cast_i8 => i8, cast_i64 => i64, cast_u8 => u8, cast_u64 => u64);
    assert_eq!(
        real::real_f32(input, format, true).map(f32::to_bits),
        real::engine_only_f32(input, format).map(f32::to_bits),
        "cast_f32 on {}",
        shown()
    );
    assert_eq!(
        real::real_f64(input, format, true).map(f64::to_bits),
        real::engine_only_f64(input, format).map(f64::to_bits),
        "cast_f64 on {}",
        shown()
    );
    assert_eq!(
        decimal::decimal(input, format, true),
        decimal::engine_only(input, format),
        "cast_decimal on {}",
        shown()
    );
}

/// Every sequence of `1..=longest` tokens, each handed to `visit` as one input.
fn each_sequence(tokens: &[&str], longest: usize, mut visit: impl FnMut(&[u8])) {
    let mut text = Vec::new();
    for length in 1..=longest {
        let mut odometer = vec![0usize; length];
        loop {
            text.clear();
            for &index in &odometer {
                text.extend_from_slice(tokens[index].as_bytes());
            }
            visit(&text);
            let mut position = 0;
            while position < length {
                odometer[position] += 1;
                if odometer[position] < tokens.len() {
                    break;
                }
                odometer[position] = 0;
                position += 1;
            }
            if position == length {
                break;
            }
        }
    }
}

#[test]
fn the_lenient_lane_changes_no_verdict_over_the_grammars_vocabulary() {
    let formats = lane_formats();
    let vocabulary = [
        "-", "+", "(", ")", "$", "\u{20ac}", "kr.", "%", "&H", " ", "\u{a0}", ",", ".", "0", "5",
        "12", "123", "e3", "E-2", "x", "\u{2212}", "\u{66A}", "\u{663}", "'",
    ];
    each_sequence(&vocabulary, 4, |input| {
        for format in &formats {
            lane_agrees_with_the_engine(input, format);
        }
    });
}

#[test]
fn the_lenient_lane_changes_no_verdict_over_longer_money_shapes() {
    let formats = lane_formats();
    each_sequence(&["(", ")", "$", "-", ",", ".", "1", "234"], 6, |input| {
        for format in &formats {
            lane_agrees_with_the_engine(input, format);
        }
    });
}

#[test]
fn the_lenient_lane_changes_no_verdict_at_its_own_limits() {
    // Where each sink stops and hands the token back: nineteen digits for an integer,
    // twenty-eight for a decimal, nineteen significant digits for a real (leading
    // zeros not counted) — and an f32 that overflows to infinity well inside that. One
    // digit either side of each, bare and dressed.
    let formats = lane_formats();
    for digits in [18usize, 19, 20, 27, 28, 29, 30, 38, 39, 40, 47, 48, 49, 50, 60] {
        for digit in ["9", "1", "0"] {
            let run = digit.repeat(digits);
            let grouped: String = run
                .as_bytes()
                .rchunks(3)
                .rev()
                .map(|chunk| str::from_utf8(chunk).unwrap())
                .collect::<Vec<_>>()
                .join(",");
            for body in [
                run.clone(),
                grouped.clone(),
                format!("{run}.5"),
                format!("{grouped}.50"),
                format!("0.{run}"),
                format!("0.000{run}"),
            ] {
                for dressed in [
                    body.clone(),
                    format!("${body}"),
                    format!("-${body}"),
                    format!("(${body})"),
                    format!("{body}$"),
                ] {
                    for format in &formats {
                        lane_agrees_with_the_engine(dressed.as_bytes(), format);
                    }
                }
            }
        }
    }
}

#[test]
fn the_lenient_lane_rounds_reals_exactly_as_the_engine_does() {
    // The real lane hands the engine's conversion a significand rather than text, so
    // its rounding is held to the engine's at the digit counts where rounding is
    // decided — fifteen to nineteen significant digits, which is where a double stops
    // being exact — and at values that sit on or beside a halfway point.
    let formats = lane_formats();
    let mut bodies = vec![
        "9,007,199,254,740,993".to_string(),
        "9,007,199,254,740,993.0".to_string(),
        "9,007,199,254,740,995".to_string(),
        "18,446,744,073,709,551,615".to_string(),
        "0.1".to_string(),
        "2.2250738585072011e-308".to_string(),
        "1,234,567,890,123,456,789".to_string(),
        "0.000000000000000000000000000001".to_string(),
    ];
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..4_000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let digits = 15 + (state % 5) as usize;
        let text = (state >> 8).to_string();
        let run = &text[..digits.min(text.len())];
        let point = (state >> 3) as usize % run.len();
        let (whole, fraction) = run.split_at(point.max(1));
        let grouped: String = whole
            .as_bytes()
            .rchunks(3)
            .rev()
            .map(|chunk| str::from_utf8(chunk).unwrap())
            .collect::<Vec<_>>()
            .join(",");
        bodies.push(if fraction.is_empty() { grouped } else { format!("{grouped}.{fraction}") });
    }
    for body in &bodies {
        for dressed in [body.clone(), format!("(${body})"), format!("-{body}$")] {
            for format in &formats {
                lane_agrees_with_the_engine(dressed.as_bytes(), format);
            }
        }
    }
}

#[test]
fn symbol_lengths_fit_in_the_abi_without_truncating_utf8() {
    let largest = format!("é{}", "x".repeat(253));
    assert_eq!(largest.len(), 255);
    assert_eq!(Symbol::<257>::new(&largest).unwrap().as_str(), largest);
    assert!(Symbol::<257>::new(&"x".repeat(256)).is_none());
    // Before the fix the length wrapped to 1, splitting the initial UTF-8 character.
    assert!(Symbol::<257>::new(&format!("é{}", "x".repeat(255))).is_none());
    assert_eq!(Symbol::<257>::MAX_BYTES, 255);
    assert!(Symbol::<0>::new("x").is_none());
    assert_eq!(Symbol::<8>::new("éééé").unwrap().as_str(), "éééé");
    assert_eq!(Symbol::<16>::new("é".repeat(8).as_str()).unwrap().as_str(), "é".repeat(8));
}

#[test]
fn decimal_display_handles_every_public_scale_without_losing_value() {
    for scale in 0..=u8::MAX {
        for magnitude in [0, 1, (1u128 << 96) - 1] {
            for negative in [false, true] {
                let value = decimal(magnitude, scale, negative);
                let digits = magnitude.to_string();
                let fractional = usize::from(scale);
                let expected = if fractional == 0 {
                    digits
                } else if fractional >= digits.len() {
                    format!("0.{}{}", "0".repeat(fractional - digits.len()), digits)
                } else {
                    let point = digits.len() - fractional;
                    format!("{}.{}", &digits[..point], &digits[point..])
                };
                let expected = if negative { format!("-{expected}") } else { expected };
                assert_eq!(value.to_string(), expected, "{value:?}");
            }
        }
    }
}

/// The validation constraints' C exports, called as a C caller calls them.
#[cfg(feature = "exports")]
mod constraint_exports {
    use super::*;
    use crate::ffi::{
        RawFault, cast_checked, cast_checked_from_f64, cast_constraints_check,
        cast_declared_decimal, cast_text,
    };
    use core::ffi::c_void;
    use core::mem::offset_of;
    use core::ptr::{null, null_mut};

    const UNTOUCHED: RawCheck = RawCheck { keyword: 7, adjusted: 7, enum_index: 7 };
    /// An `enum` declared, and nothing else.
    const LISTED: RawConstraints =
        RawConstraints { flags: RawConstraints::HAS_ENUM, ..RawConstraints::ZERO };
    const UNTOUCHED_FAULT: (u32, u32) = (0xFFFF, 0xFFFF);

    fn dec(text: &str) -> Decimal {
        decimal_for_declaration(text).unwrap()
    }

    fn latitude() -> RawConstraints {
        let mut raw = RawConstraints::ZERO;
        raw.flags = RawConstraints::HAS_MINIMUM
            | RawConstraints::HAS_MAXIMUM
            | RawConstraints::HAS_MULTIPLE_OF;
        raw.lower = dec("-90");
        raw.upper = dec("90");
        raw.multiple_of = dec("0.000001");
        raw.rounding = 1;
        raw
    }

    /// Text `enum` entries packed as the ABI takes them.
    fn packed_text(entries: &[&str]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for entry in entries {
            bytes.extend_from_slice(&(entry.len() as u32).to_le_bytes());
            bytes.extend_from_slice(entry.as_bytes());
        }
        bytes
    }

    /// Integer `enum` entries packed as the ABI takes them: native-endian `Decimal`s.
    fn packed_integers(entries: &[&str]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for entry in entries {
            let d = dec(entry);
            bytes.extend_from_slice(&d.lo.to_ne_bytes());
            bytes.extend_from_slice(&d.hi.to_ne_bytes());
            bytes.extend_from_slice(&[d.scale, d.negative.into(), 0, 0]);
        }
        bytes
    }

    /// `cast_checked` over `input`, with what it wrote to each out-param.
    fn checked<T: Copy>(
        door: u32,
        raw: *const RawConstraints,
        entries: &[u8],
        input: &[u8],
        out: T,
    ) -> (i32, T, (u32, u32), RawCheck) {
        let mut out = out;
        let mut fault = RawFault { offset: UNTOUCHED_FAULT.0, len: UNTOUCHED_FAULT.1 };
        let mut check = UNTOUCHED;
        let code = cast_checked(
            door,
            0,
            null(),
            raw,
            entries.as_ptr(),
            entries.len(),
            input.as_ptr(),
            input.len(),
            (&raw mut out).cast::<c_void>(),
            &mut fault,
            &mut check,
        );
        (code, out, (fault.offset, fault.len), check)
    }

    fn from_f64<T: Copy>(
        door: u32,
        raw: *const RawConstraints,
        value: f64,
        out: T,
    ) -> (i32, T, (u32, u32), RawCheck) {
        let mut out = out;
        let mut fault = RawFault { offset: UNTOUCHED_FAULT.0, len: UNTOUCHED_FAULT.1 };
        let mut check = UNTOUCHED;
        let code = cast_checked_from_f64(
            door,
            value,
            raw,
            null(),
            0,
            (&raw mut out).cast::<c_void>(),
            &mut fault,
            &mut check,
        );
        (code, out, (fault.offset, fault.len), check)
    }

    #[test]
    fn checked_reports_through_raw_check() {
        let raw = latitude();
        let (code, value, fault, check) = checked(11, &raw, &[], b"45.1234567", 0f64);
        assert_eq!((code, value, fault), (0, 45.123457, UNTOUCHED_FAULT));
        assert_eq!(check, RawCheck { keyword: 0, adjusted: 1, enum_index: u32::MAX });
        // Unrounded: not adjusted. A null `check` is fine.
        let mut value = 0f64;
        let input = b"-12.5";
        let code = cast_checked(
            11,
            0,
            null(),
            &raw,
            null(),
            0,
            input.as_ptr(),
            input.len(),
            (&raw mut value).cast::<c_void>(),
            null_mut(),
            null_mut(),
        );
        assert_eq!((code, value), (0, -12.5));
        let (code, value, _, check) = checked(11, &raw, &[], b"10", 0f64);
        assert_eq!((code, value, check.adjusted), (0, 10.0, 0));
        // The same judgement of a decoded number, which writes the same report.
        let (code, value, fault, check) = from_f64(11, &raw, 45.1234567, 0f64);
        assert_eq!((code, value, fault), (0, 45.123457, UNTOUCHED_FAULT));
        assert_eq!(check, RawCheck { keyword: 0, adjusted: 1, enum_index: u32::MAX });
        // An integer `enum` has no index: the value is the answer.
        let codes = packed_integers(&["10", "20"]);
        let (code, value, _, check) = checked(4, &LISTED, &codes, b"20", 0i32);
        assert_eq!((code, value, check), (0, 20, RawCheck { keyword: 0, ..check }));
        assert_eq!(check.enum_index, u32::MAX);
        // The decimal door writes a Decimal.
        let (code, value, _, _) = checked(19, &raw, &[], b"-0.0000004", dec("1"));
        assert_eq!((code, value), (0, dec("0")));
        assert!(!value.negative);
    }

    #[test]
    fn violation_is_code_4() {
        let raw = latitude();
        let (code, value, fault, check) = checked(11, &raw, &[], b" 91 ", 0f64);
        assert_eq!((code, value, fault), (4, 0.0, (1, 2)));
        assert_eq!(check, RawCheck { keyword: 3, adjusted: 0, enum_index: u32::MAX });
        let (code, _, fault, check) = checked(11, &raw, &[], b"-90.5", 0f64);
        assert_eq!((code, fault, check.keyword), (4, (0, 5), 1));
        let (code, _, fault, check) = from_f64(11, &raw, 91.0, 0f64);
        assert_eq!((code, fault, check.keyword), (4, (0, 0), 3));
        let codes = packed_integers(&["10", "20"]);
        let (code, _, fault, check) = checked(4, &LISTED, &codes, b"15", 0i32);
        assert_eq!((code, fault, check.keyword), (4, (0, 2), 8));
        // A parse failure is the door's own code: `fault` is written, `check` and `out` not.
        let (code, value, fault, check) = checked(11, &raw, &[], b"9x", 0f64);
        assert_eq!((code, value, fault, check), (2, 0.0, (1, 1), UNTOUCHED));
        let (code, value, fault, check) = checked(11, &raw, &[], b"", 0f64);
        assert_eq!((code, value, fault, check), (1, 0.0, (0, 0), UNTOUCHED));
        let (code, _, fault, check) = checked(2, &RawConstraints::ZERO, &[], b"300", 0i8);
        assert_eq!((code, fault, check), (3, (0, 3), UNTOUCHED));
        let (code, _, fault, check) = from_f64(11, &raw, f64::NAN, 0f64);
        assert_eq!((code, fault, check), (2, (0, 0), UNTOUCHED));
        let (code, _, _, check) = from_f64(11, &raw, f64::INFINITY, 0f64);
        assert_eq!((code, check), (3, UNTOUCHED));
    }

    #[test]
    fn the_text_door_writes_a_span() {
        let mut raw = RawConstraints::ZERO;
        raw.flags = RawConstraints::TRIM | RawConstraints::HAS_MAX_LENGTH;
        raw.max_length = 6;
        let listed = RawConstraints { flags: raw.flags | RawConstraints::HAS_ENUM, ..raw };
        let sexes = packed_text(&["Female", "Male", "Other"]);
        let blank = RawSpan { offset: 99, len: 99 };
        // The span is the matched input, after the trim; the index names the entry.
        let input = "\u{00A0}MALE ".as_bytes();
        let (code, span, _, check) = checked(18, &listed, &sexes, input, blank);
        assert_eq!((code, span), (0, RawSpan { offset: 2, len: 4 }));
        assert_eq!(check, RawCheck { keyword: 0, adjusted: 0, enum_index: 1 });
        // The span is written unaligned: `out` need not be 4-aligned.
        let mut buffer = [0xAAu8; 9];
        let code = cast_checked(
            18,
            0,
            null(),
            &listed,
            sexes.as_ptr(),
            sexes.len(),
            input.as_ptr(),
            input.len(),
            buffer[1..].as_mut_ptr().cast::<c_void>(),
            null_mut(),
            null_mut(),
        );
        let mut written = [0u8; 8];
        written.copy_from_slice(&buffer[1..]);
        let unaligned = (
            u32::from_ne_bytes(written[..4].try_into().unwrap()),
            u32::from_ne_bytes(written[4..].try_into().unwrap()),
        );
        assert_eq!((code, buffer[0], unaligned), (0, 0xAA, (2, 4)));
        let (code, span, fault, check) = checked(18, &listed, &sexes, b" Mail ", blank);
        assert_eq!((code, span, fault, check.keyword), (4, blank, (1, 4), 8));
        let (code, span, fault, check) = checked(18, &raw, &[], b"Females", blank);
        assert_eq!((code, span, fault, check.keyword), (4, blank, (0, 7), 7));
        // No `enum`: the span, and no index.
        let (code, span, _, check) = checked(18, &raw, &[], b" abc ", blank);
        assert_eq!((code, span, check.enum_index), (0, RawSpan { offset: 1, len: 3 }, u32::MAX));
        // Trimmed to nothing is empty; invalid UTF-8 malformed, spanning the bad sequence.
        let (code, _, fault, check) = checked(18, &raw, &[], b"   ", blank);
        assert_eq!((code, fault, check), (1, (0, 0), UNTOUCHED));
        let (code, _, fault, _) = checked(18, &raw, &[], b"ab\xFFc", blank);
        assert_eq!((code, fault), (2, (2, 1)));
        // The text door ignores `format`, even one it would refuse.
        let refused = RawNumFormat { size: 0, ..RawNumFormat::ZERO };
        let mut span = blank;
        let input = b"abc";
        let code = cast_checked(
            18,
            0,
            &refused,
            &RawConstraints::ZERO,
            null(),
            0,
            input.as_ptr(),
            input.len(),
            (&raw mut span).cast::<c_void>(),
            null_mut(),
            null_mut(),
        );
        assert_eq!((code, span), (0, RawSpan { offset: 0, len: 3 }));
    }

    #[test]
    fn cast_text_validates_utf8() {
        let door = |input: &[u8]| {
            let mut span = RawSpan { offset: 99, len: 99 };
            let mut fault = RawFault { offset: UNTOUCHED_FAULT.0, len: UNTOUCHED_FAULT.1 };
            let code = cast_text(input.as_ptr(), input.len(), &mut span, &mut fault);
            (code, span, (fault.offset, fault.len))
        };
        assert_eq!(door(b" a "), (0, RawSpan { offset: 0, len: 3 }, UNTOUCHED_FAULT));
        assert_eq!(door("\u{1F44D}\u{1F3FD}".as_bytes()).1, RawSpan { offset: 0, len: 8 });
        assert_eq!(door(b""), (1, RawSpan { offset: 99, len: 99 }, (0, 0)));
        assert_eq!(door(b"ab\xFFc").0, 2);
        assert_eq!(door(b"ab\xFFc").2, (2, 1));
        // Cut short: the span runs to the end of the input.
        assert_eq!(door(b"ab\xE2\x82").2, (2, 2));
        let mut span = RawSpan { offset: 99, len: 99 };
        assert_eq!(cast_text(null(), 0, &mut span, null_mut()), 1);
    }

    #[test]
    fn constraints_check_codes() {
        let check = |door: u32, raw: &RawConstraints, entries: &[u8]| {
            cast_constraints_check(door, 0, raw, entries.as_ptr(), entries.len())
        };
        let with = |edit: &dyn Fn(&mut RawConstraints)| {
            let mut raw = RawConstraints::ZERO;
            edit(&mut raw);
            raw
        };
        let (i32_door, u8_door, text_door) = (4, 6, 18);
        assert_eq!(check(11, &latitude(), &[]), 0);
        assert_eq!(check(text_door, &LISTED, &packed_text(&["a", "b"])), 0);
        assert_eq!(check(i32_door, &LISTED, &packed_integers(&["1", "2"])), 0);
        let trimmed = RawConstraints {
            flags: RawConstraints::HAS_ENUM | RawConstraints::TRIM,
            ..RawConstraints::ZERO
        };
        let cases: [(u32, RawConstraints, Vec<u8>, i32); 18] = [
            (
                i32_door,
                with(&|r| {
                    r.flags = RawConstraints::HAS_MIN_LENGTH;
                    r.min_length = 1;
                }),
                Vec::new(),
                1,
            ),
            (
                i32_door,
                with(&|r| {
                    r.flags = RawConstraints::HAS_MINIMUM | RawConstraints::HAS_EXCLUSIVE_MINIMUM
                }),
                Vec::new(),
                2,
            ),
            (
                i32_door,
                with(&|r| {
                    r.flags = RawConstraints::HAS_MAXIMUM | RawConstraints::HAS_EXCLUSIVE_MAXIMUM
                }),
                Vec::new(),
                3,
            ),
            (
                i32_door,
                with(&|r| {
                    r.flags = RawConstraints::HAS_MINIMUM | RawConstraints::HAS_MAXIMUM;
                    r.lower = dec("5");
                    r.upper = dec("5");
                }),
                Vec::new(),
                4,
            ),
            (i32_door, with(&|r| r.flags = RawConstraints::HAS_MULTIPLE_OF), Vec::new(), 5),
            (i32_door, with(&|r| r.rounding = 1), Vec::new(), 6),
            (
                text_door,
                with(&|r| {
                    r.flags = RawConstraints::HAS_MIN_LENGTH | RawConstraints::HAS_MAX_LENGTH;
                    r.min_length = 5;
                    r.max_length = 4;
                }),
                Vec::new(),
                7,
            ),
            (
                u8_door,
                with(&|r| {
                    r.flags = RawConstraints::HAS_MINIMUM;
                    r.lower = dec("-1");
                }),
                Vec::new(),
                8,
            ),
            // ABI-only: a binding's strings are always UTF-8.
            (text_door, LISTED, vec![1, 0, 0, 0, 0xFF], 9),
            (text_door, LISTED, packed_text(&["Male", "MALE"]), 10),
            (i32_door, with(&|r| r.size = 71), Vec::new(), 11),
            // A list passed without `HAS_ENUM` is never read.
            (i32_door, RawConstraints::ZERO, packed_integers(&["1"]), 11),
            (text_door, RawConstraints::ZERO, packed_text(&["a"]), 11),
            (i32_door, LISTED, Vec::new(), 12),
            (text_door, LISTED, Vec::new(), 12),
            (text_door, LISTED, packed_text(&["Male", ""]), 13),
            (text_door, trimmed, packed_text(&[" Male "]), 13),
            (text_door, trimmed, packed_text(&["   "]), 13),
        ];
        // Without `trim`, an entry with White_Space at its edges is matched as given.
        assert_eq!(check(text_door, &LISTED, &packed_text(&[" Male "])), 0);
        for (door, raw, entries, code) in &cases {
            assert_eq!(check(*door, raw, entries), *code, "{raw:?} {entries:?}");
        }
        // Every malformed layout, the size judged before anything else.
        for raw in [
            with(&|r| r.size = 0),
            with(&|r| r.size = 73),
            with(&|r| r.flags = 512),
            with(&|r| r._reserved = 1),
            with(&|r| {
                r.flags = RawConstraints::HAS_MULTIPLE_OF;
                r.multiple_of = dec("1");
                r.rounding = 4;
            }),
            with(&|r| {
                r.size = 71;
                r.flags = RawConstraints::HAS_MIN_LENGTH;
            }),
        ] {
            assert_eq!(check(i32_door, &raw, &[]), 11);
            // The checked doors refuse the same layouts as contract violations.
            assert_eq!(checked(i32_door, &raw, &[], b"1", 0i32).0, -1);
            assert_eq!(from_f64(i32_door, &raw, 1.0, 0i32).0, -1);
        }
        assert_eq!(cast_constraints_check(i32_door, 0, null(), null(), 0), 11);
        // A sign byte that is no `bool`, in each decimal, whether or not it is declared, read
        // as a byte (the layout as caller memory, never as a Rust value), is malformed; the
        // checked doors refuse it as a contract violation.
        for (field, flags, byte) in [
            (offset_of!(RawConstraints, lower), 0, 2u8),
            (offset_of!(RawConstraints, upper), RawConstraints::HAS_MAXIMUM, 255),
            (offset_of!(RawConstraints, multiple_of), 0, 2),
            (offset_of!(RawConstraints, lower), RawConstraints::HAS_MINIMUM, 1),
        ] {
            let mut memory = [0u64; 9];
            let bytes = memory.as_mut_ptr().cast::<u8>();
            let raw = RawConstraints { flags, ..RawConstraints::ZERO };
            unsafe {
                bytes.copy_from_nonoverlapping((&raw const raw).cast::<u8>(), 72);
                bytes.add(field + offset_of!(Decimal, negative)).write(byte);
            }
            let layout = memory.as_ptr().cast::<RawConstraints>();
            let want = if byte <= 1 { (0, 0) } else { (11, -1) };
            let code = cast_constraints_check(i32_door, 0, layout, null(), 0);
            let input = if flags == RawConstraints::HAS_MINIMUM { &b"0"[..] } else { b"-1" };
            assert_eq!(
                (code, checked(i32_door, layout, &[], input, 0i32).0),
                want,
                "{field} {byte}"
            );
            assert_eq!(from_f64(i32_door, layout, 0.0, 0i32).0, want.1, "{field} {byte}");
        }
        // A door code that names no door: never -1. 23 (`Char`, from 1.1) is still none to
        // this 1.0 export.
        for (door, param) in [(0, 0), (23, 0), (24, 0), (14, 9), (20, 0)] {
            assert_eq!(cast_constraints_check(door, param, &RawConstraints::ZERO, null(), 0), 1);
            // The size is judged first, before the door.
            let short = RawConstraints { size: 71, ..RawConstraints::ZERO };
            assert_eq!(cast_constraints_check(door, param, &short, null(), 0), 11);
        }
        // Duplicate detection belongs to this export alone: the checked door takes the
        // first match.
        let twice = packed_text(&["Male", "MALE"]);
        let (code, _, _, report) =
            checked(text_door, &LISTED, &twice, b"male", RawSpan { offset: 0, len: 0 });
        assert_eq!((code, report.enum_index), (0, 0));
    }

    #[test]
    fn null_and_zero_size_are_contract_violations() {
        let mut wrong_sizes = Vec::new();
        for size in [0u32, 1, 4, 64, 71, 73, 80, 92, u32::MAX] {
            wrong_sizes.push(RawConstraints { size, ..RawConstraints::ZERO });
        }
        for raw in &wrong_sizes {
            let (code, value, fault, check) = checked(4, raw, &[], b"1", 0i32);
            assert_eq!((code, value, fault, check), (-1, 0, UNTOUCHED_FAULT, UNTOUCHED), "{raw:?}");
            let (code, value, fault, check) = from_f64(4, raw, 1.0, 0i32);
            assert_eq!((code, value, fault, check), (-1, 0, UNTOUCHED_FAULT, UNTOUCHED), "{raw:?}");
            let (code, ..) = checked(18, raw, &[], b"a", RawSpan { offset: 0, len: 0 });
            assert_eq!(code, -1, "{raw:?}");
        }
        assert_eq!(checked(4, null(), &[], b"1", 0i32).0, -1);
        assert_eq!(from_f64(4, null(), 1.0, 0i32).0, -1);
        // A code that names no door, a door that is neither numeric nor Text (and Text for a
        // decoded number), a refused format, and a refused declaration.
        let zero = RawConstraints::ZERO;
        for door in [0, 1, 12, 13, 15, 23] {
            assert_eq!(checked(door, &zero, &[], b"1", 0u64).0, -1, "door {door}");
            assert_eq!(from_f64(door, &zero, 1.0, 0u64).0, -1, "door {door}");
        }
        assert_eq!(from_f64(18, &zero, 1.0, 0u64).0, -1);
        let refused_format = RawNumFormat { size: 0, ..RawNumFormat::ZERO };
        let mut out = 0i32;
        let input = b"1";
        let code = cast_checked(
            4,
            0,
            &refused_format,
            &zero,
            null(),
            0,
            input.as_ptr(),
            input.len(),
            (&raw mut out).cast::<c_void>(),
            null_mut(),
            null_mut(),
        );
        assert_eq!(code, -1);
        let refused = RawConstraints { flags: RawConstraints::HAS_MULTIPLE_OF, ..zero };
        assert_eq!(checked(4, &refused, &[], b"1", 0i32).0, -1);
        assert_eq!(from_f64(4, &refused, 1.0, 0i32).0, -1);
    }

    /// The export runs `decimal_for_declaration`: success writes the same `Decimal`; a
    /// refusal is code 8 and leaves `out` alone; bytes that are not UTF-8 are 8, never -1.
    #[test]
    fn declared_decimal_converts_as_the_rust_api_does() {
        let untouched = Decimal { lo: 0xA5A5, hi: 0x5A5A, scale: 99, negative: true };
        let convert = |bytes: &[u8]| {
            let mut out = untouched;
            let code = cast_declared_decimal(bytes.as_ptr(), bytes.len(), &mut out);
            (code, out)
        };
        let ok = |text: &str, lo: u64, scale: u8, negative: bool| {
            assert_eq!(convert(text.as_bytes()), (0, Decimal { lo, hi: 0, scale, negative }));
            assert_eq!(convert(text.as_bytes()).1, dec(text), "{text}");
        };
        ok("1.50", 15, 1, false);
        ok("-0", 0, 0, false);
        ok("7.0", 7, 0, false);
        ok("+1e3", 1000, 0, false);
        ok(".5", 5, 1, false);
        ok("-1e-28", 1, 28, true);
        assert_eq!(
            convert(b"79228162514264337593543950335"),
            (0, Decimal { lo: u64::MAX, hi: u32::MAX, scale: 0, negative: false })
        );
        for refused in [
            &b"1e-29"[..],
            b"79228162514264337593543950336",
            b"3.5e38",
            b" 1",
            b"--1",
            b"1e",
            b"",
            "\u{0661}".as_bytes(),
            b"\xFF",
            b"1\xFF",
            b"1.\xC3",
        ] {
            assert_eq!(convert(refused), (8, untouched), "{refused:?}");
        }
    }

    #[test]
    fn declared_decimal_contract_violations() {
        let mut out = Decimal { lo: 1, hi: 0, scale: 0, negative: false };
        assert_eq!(cast_declared_decimal(b"1".as_ptr(), 1, null_mut()), -1);
        assert_eq!(cast_declared_decimal(null(), 1, &mut out), -1);
        assert_eq!(out, Decimal { lo: 1, hi: 0, scale: 0, negative: false });
        // A null `ptr` with a zero `len` is empty text, which no notation spells: 8.
        assert_eq!(cast_declared_decimal(null(), 0, &mut out), 8);
        assert_eq!(cast_declared_decimal(null(), 0, null_mut()), -1);
    }
}

// --- packing an `enum` list ---

#[test]
fn packed_text_len_checks_its_sum() {
    use crate::constraints::packed_text_len;
    assert_eq!(packed_text_len([].into_iter()), Some(0));
    assert_eq!(packed_text_len([0, 5].into_iter()), Some(13));
    assert_eq!(packed_text_len([usize::MAX - 4].into_iter()), Some(usize::MAX));
    assert_eq!(packed_text_len([usize::MAX - 3].into_iter()), None);
    assert_eq!(packed_text_len([usize::MAX / 2, usize::MAX / 2].into_iter()), None);
}

// --- the white-space scanner ---

#[test]
fn white_space_scanner_matches_the_table() {
    use crate::text::{WHITE_SPACE, white_space_len, white_space_len_back};
    for c in WHITE_SPACE {
        let mut buf = [0u8; 4];
        let bytes = c.encode_utf8(&mut buf).as_bytes();
        assert_eq!(white_space_len(bytes), c.len_utf8(), "{:04X}", c as u32);
        assert_eq!(white_space_len_back(bytes), c.len_utf8(), "{:04X}", c as u32);
        assert!(is_white_space(c), "{:04X}", c as u32);
    }
    for c in ['\u{200B}', '\u{FEFF}', '\u{180E}', '\u{0000}', '\u{00AD}', 'a'] {
        let mut buf = [0u8; 4];
        let bytes = c.encode_utf8(&mut buf).as_bytes();
        assert_eq!(white_space_len(bytes), 0, "{:04X}", c as u32);
        assert_eq!(white_space_len_back(bytes), 0, "{:04X}", c as u32);
        assert!(!is_white_space(c), "{:04X}", c as u32);
    }
    for broken in [&b"\xC2"[..], b"\xE2\x80", b"\xC0\xA0"] {
        assert_eq!(white_space_len(broken), 0);
        assert_eq!(white_space_len_back(broken), 0);
    }
}

#[test]
fn trim_strips_unicode_edges() {
    use crate::verdict::trim;
    assert_eq!(trim("\u{A0}\u{3000} 12\u{202F}\n".as_bytes()), (&b"12"[..], 6));
    assert_eq!(trim(b"\x0B12"), (&b"12"[..], 1));
    assert_eq!(trim(b"12\xC2"), (&b"12\xC2"[..], 0));
}

#[test]
fn white_space_runs_and_interior_scan() {
    use crate::text::{contains_white_space, skip_white_space, skip_white_space_back};
    let text = "\u{A0}\u{3000} 1 2\u{202F}\n".as_bytes();
    assert_eq!(skip_white_space(text), 6);
    assert_eq!(skip_white_space_back(text), 4);
    assert_eq!(skip_white_space(b""), 0);
    assert_eq!(skip_white_space(b" \t\x0B"), 3);
    assert_eq!(skip_white_space_back(b"\xE2\x80"), 0);
    assert!(contains_white_space(b"1\xC2\xA02"));
    assert!(!contains_white_space(b"12\xC2\xE2\x80\xC0\xA0"));
    assert!(!contains_white_space(b""));
}

// --- fault detail ---

#[test]
fn every_detail_round_trips() {
    for (index, detail) in Detail::ALL.iter().enumerate() {
        assert_eq!(detail.code(), index as u32 + 1, "{detail:?} is numbered in order");
        assert_eq!(Detail::from_code(detail.code()), Some(*detail));
    }
    assert_eq!(Detail::from_code(0), None, "0 is no detail");
    assert_eq!(Detail::from_code(33), None);
    assert_eq!(Detail::from_code(u32::MAX), None);
    let mut names: Vec<&str> = Detail::ALL.iter().map(|detail| detail.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), Detail::ALL.len(), "every name is its own");
    assert_eq!(Detail::NotAScalar.to_string(), "not_a_scalar");
}

#[test]
fn a_failure_projects_to_the_plain_fault() {
    let failure = Failure::new(Reason::Malformed, Detail::DigitExpected, 2, 1);
    assert_eq!(failure.fault(), Fault { reason: Reason::Malformed, offset: 2, len: 1 });
    assert_eq!(Fault::from(failure), failure.fault());
    assert_eq!(failure.to_string(), "malformed input at bytes 2..3 (digit_expected)");
    let rejection = detailed::Rejection::from(failure);
    assert_eq!(Rejection::from(rejection), Rejection::Fault(failure.fault()));
}

/// The invalid-UTF-8 detail is decided from the bytes at the winning span alone, widened to
/// the whole sequences it starts or stops inside; the reason and span never move.
#[test]
fn invalid_utf8_is_decided_at_the_winning_span() {
    let valid = ValidNumFormat::INVARIANT;
    let detail = |input: &[u8]| detailed::cast_i32(input, &valid).unwrap_err();
    // A stray byte where the numeral fails.
    let stray = detail(b"12\xFF");
    assert_eq!((stray.detail, stray.offset, stray.len), (Detail::InvalidUtf8, 2, 1));
    // A whole letter that is valid UTF-8 is a letter, however much invalid text follows it.
    let letter = detail(b"12\xC3\xA9\xFF");
    assert_eq!((letter.detail, letter.offset, letter.len), (Detail::UnexpectedCharacter, 2, 2));
    let ascii = detail(b"12x\xFF");
    assert_eq!((ascii.detail, ascii.offset, ascii.len), (Detail::UnexpectedCharacter, 2, 1));
    // A lead byte whose sequence is cut short by a digit.
    let cut = detail(b"12\xC31");
    assert_eq!((cut.detail, cut.offset, cut.len), (Detail::InvalidUtf8, 2, 2));
    // An overlong encoding and a surrogate.
    assert_eq!(detail(b"1\xC0\x80").detail, Detail::InvalidUtf8);
    assert_eq!(detail(b"1\xED\xA0\x80").detail, Detail::InvalidUtf8);
    // A token-wide span is judged whole.
    let word = detailed::cast_bool(b"y\xFFs").unwrap_err();
    assert_eq!((word.detail, word.offset, word.len), (Detail::InvalidUtf8, 0, 3));
    assert_eq!(detailed::cast_bool(b"yess").unwrap_err().detail, Detail::NotRecognized);
    // Every plain door's fault is the same either way.
    for input in [&b"12\xFF"[..], b"12\xC3\xA9\xFF", b"12\xC31", b"\xFF"] {
        assert_eq!(
            cast_i32(input, &INVARIANT),
            detailed::cast_i32(input, &valid).map_err(Failure::fault)
        );
    }
    // A span that starts inside a valid character widens back to its lead byte.
    let inside = Failure::new(Reason::Malformed, Detail::UnexpectedCharacter, 3, 1);
    assert_eq!(inside.classified("12é".as_bytes()).detail, Detail::UnexpectedCharacter);
    let orphan = Failure::new(Reason::Malformed, Detail::UnexpectedCharacter, 1, 1);
    assert_eq!(orphan.classified(b"1\x80").detail, Detail::InvalidUtf8);
    // Empty spans and empty input carry no bytes to judge.
    assert_eq!(detailed::cast_i32("", &valid).unwrap_err().detail, Detail::Blank);
}

#[test]
fn a_format_validates_as_check_judges_it() {
    assert_eq!(NumFormat::INVARIANT.validate(), Ok(ValidNumFormat::INVARIANT));
    assert_eq!(ValidNumFormat::default().format(), &NumFormat::INVARIANT);
    let equal = NumFormat::new('.', '.', NumFormat::ALL);
    assert!(!equal.check());
    assert_eq!(equal.validate(), Err(DeclarationError::NumFormat));
    assert!(!DeclarationError::NumFormat.to_string().is_empty());
}

// --- strict grouping ---

#[test]
fn the_grouping_constants_are_bits_eight_and_nine() {
    assert_eq!(NumFormat::GROUPING_LEGACY, 0);
    assert_eq!(NumFormat::GROUPING_WESTERN, 1 << 8);
    assert_eq!(NumFormat::GROUPING_INDIAN, 2 << 8);
    assert_eq!(NumFormat::GROUPING_POLICY, 3 << 8);
    // `ALL` is a leniency mask and never carries a policy.
    assert_eq!(NumFormat::ALL & NumFormat::GROUPING_POLICY, 0);
}

#[test]
fn with_grouping_replaces_the_policy_never_ors_it() {
    let western = INVARIANT.with_grouping(NumFormat::GROUPING_WESTERN);
    assert_eq!(western.flags, NumFormat::ALL | NumFormat::GROUPING_WESTERN);
    let indian = western.with_grouping(NumFormat::GROUPING_INDIAN);
    assert_eq!(indian.flags, NumFormat::ALL | NumFormat::GROUPING_INDIAN, "replaced, not 11");
    assert!(indian.check());
    assert_eq!(indian.with_grouping(NumFormat::GROUPING_LEGACY), INVARIANT);
    // Only the policy field is written; any other bit in the argument is ignored.
    let stray = INVARIANT.with_grouping(NumFormat::GROUPING_WESTERN | NumFormat::TRAILING_SIGN);
    assert_eq!(stray.flags, NumFormat::ALL | NumFormat::GROUPING_WESTERN);
    // Everything else is kept.
    let euro = NumFormat::new(',', '.', NumFormat::PARENS)
        .with_currency(CurrencySymbol::new("€").unwrap())
        .with_grouping(NumFormat::GROUPING_INDIAN);
    assert_eq!(euro.flags, NumFormat::PARENS | NumFormat::GROUPING_INDIAN);
    assert_eq!(euro.decimal_sep.as_bytes(), b",");
    assert_eq!(euro.currency.as_bytes(), "€".as_bytes());
}

#[test]
fn strict_is_invariant_with_western_grouping_and_invariant_stays_legacy() {
    assert_eq!(
        NumFormat::STRICT,
        NumFormat { flags: NumFormat::ALL | NumFormat::GROUPING_WESTERN, ..INVARIANT }
    );
    assert!(NumFormat::STRICT.check());
    assert_eq!(INVARIANT.flags & NumFormat::GROUPING_POLICY, NumFormat::GROUPING_LEGACY);
    assert_eq!(NumFormat::DETECT.flags & NumFormat::GROUPING_POLICY, NumFormat::GROUPING_LEGACY);
    assert_eq!(NumFormat::default(), INVARIANT);
    assert_eq!(cast_i32(b"12,34", &INVARIANT), Ok(1234));
    assert_eq!(reason(cast_i32(b"12,34", &NumFormat::STRICT)), Reason::Malformed);
}

#[test]
fn a_refused_policy_and_undefined_flags_fail_validation() {
    let refused = NumFormat { flags: NumFormat::ALL | NumFormat::GROUPING_POLICY, ..INVARIANT };
    assert!(!refused.check());
    assert_eq!(refused.validate(), Err(DeclarationError::GroupingPolicy));
    for bit in 10..32 {
        let unknown = NumFormat { flags: NumFormat::ALL | 1 << bit, ..INVARIANT };
        assert!(!unknown.check(), "bit {bit}");
        assert_eq!(unknown.validate(), Err(DeclarationError::UnknownFlag), "bit {bit}");
    }
    // Every defined bit, with each valid policy, is accepted.
    for policy in
        [NumFormat::GROUPING_LEGACY, NumFormat::GROUPING_WESTERN, NumFormat::GROUPING_INDIAN]
    {
        let every = NumFormat::ALL | NumFormat::SEPARATOR_DETECT | NumFormat::TRAILING_SIGN;
        assert!(NumFormat { flags: every | policy, ..INVARIANT }.check());
        assert!(NumFormat { flags: policy, ..INVARIANT }.validate().is_ok());
    }
    for error in [DeclarationError::GroupingPolicy, DeclarationError::UnknownFlag] {
        assert!(!error.to_string().is_empty());
    }
}

/// A strict policy enables nothing: with `GROUPING` cleared from `STRICT`, a group separator
/// stays forbidden (plan Review Focus 5).
#[test]
fn a_strict_format_with_grouping_off_refuses_group_separators() {
    let format = NumFormat { flags: NumFormat::STRICT.flags & !NumFormat::GROUPING, ..INVARIANT };
    let format = format.validate().unwrap();
    let refused = |failure: Failure| (failure.reason, failure.detail, failure.offset, failure.len);
    let expected = Err((Reason::Malformed, Detail::UnexpectedCharacter, 1, 1));
    assert_eq!(detailed::cast_i32("1,234", &format).map(|_| ()).map_err(refused), expected);
    assert_eq!(detailed::cast_f64("1,234", &format).map(|_| ()).map_err(refused), expected);
    assert_eq!(detailed::cast_decimal("1,234", &format).map(|_| ()).map_err(refused), expected);
    assert_eq!(detailed::cast_i32("1234", &format), Ok(1234));
}

/// Native digits with a space-like group separator under strict Western: groups are counted
/// in digits, never bytes (plan Review Focus 1).
#[test]
fn native_digits_with_a_space_like_separator_are_counted_as_digits() {
    let format = NumFormat { group_sep: Symbol::new("\u{202f}").unwrap(), ..NumFormat::STRICT }
        .with_digits(NativeDigits::new('\u{660}').unwrap())
        .validate()
        .unwrap();
    let good = "\u{661}\u{202f}\u{662}\u{663}\u{664}";
    assert_eq!(detailed::cast_i32(good, &format), Ok(1234));
    assert_eq!(detailed::cast_f64(good, &format), Ok(1234.0));
    assert_eq!(detailed::cast_decimal(good, &format).map(|d| d.lo), Ok(1234));
    // The other space-like separators stand for U+202F.
    assert_eq!(detailed::cast_i32("\u{661}\u{a0}\u{662}\u{663}\u{664}", &format), Ok(1234));
    let bad = "\u{661}\u{662}\u{202f}\u{663}\u{664}";
    for failure in [
        detailed::cast_i32(bad, &format).unwrap_err(),
        detailed::cast_f64(bad, &format).unwrap_err(),
        detailed::cast_decimal(bad, &format).unwrap_err(),
    ] {
        // ` ٣٤`: the separator's three bytes and two two-byte digits.
        assert_eq!(
            (failure.reason, failure.detail, failure.offset, failure.len),
            (Reason::Malformed, Detail::GroupSize, 4, 7)
        );
    }
}

/// The lenient lane validates strict groups itself, so a grouped token under the bindings'
/// default policy stays on the fast path; whatever it can't validate falls through to the
/// engine, which is authoritative. Both give the same verdict, detail and span.
#[test]
fn lane_and_engine_agree_on_grouping() {
    let symbol = |text| CurrencySymbol::new(text).unwrap();
    let wide = |text| Symbol::<8>::new(text).unwrap();
    let policies =
        [NumFormat::GROUPING_LEGACY, NumFormat::GROUPING_WESTERN, NumFormat::GROUPING_INDIAN];
    let inputs = [
        "1,234,567",
        "12,34,567",
        "1,23,45,678",
        "1234,567",
        "12,34",
        "123,456",
        "1,2,345",
        "1,2345",
        "1,23,456",
        "12,345,678",
        ",123",
        "123,",
        "1,,234",
        "1234,",
        "1,234,",
        "1,234.5",
        "12,34.5",
        "1.234,5",
        "1,234.56,7",
        "1e1,000",
        "1,234e3",
        "12,34x",
        "$1,234",
        "$12,34",
        "1,234$",
        "(1,234)",
        "(12,34)",
        "-12,34,567.5",
        "+1,234",
        "1,234%",
        "1234",
        "1 234 567",
        "12 34 567",
        "1\u{a0}234\u{202f}567",
        "1\u{a0}23",
        "\u{661},\u{662}\u{663}\u{664}",
        "\u{661}\u{662},\u{663}\u{664}",
        "\u{661}\u{202f}\u{662}\u{663}\u{664}",
        "\u{661}\u{662}\u{202f}\u{663}\u{664}",
        "1.234.567",
        "1.23.456",
        "1.234,56",
        "12.34,5",
    ];
    for policy in policies {
        let formats = [
            INVARIANT.with_currency(symbol("$")),
            NumFormat::new('.', ' ', NumFormat::ALL),
            NumFormat::new(',', '.', NumFormat::ALL).with_currency(symbol("$")),
            INVARIANT.with_digits(NativeDigits::new('\u{660}').unwrap()),
            NumFormat { group_sep: wide("\u{202f}"), ..INVARIANT }
                .with_digits(NativeDigits::new('\u{660}').unwrap()),
            NumFormat::DETECT,
            INVARIANT.with_currency(symbol("$")).with_currency_separators(wide(","), wide(".")),
        ];
        for format in formats {
            let format = format.with_grouping(policy);
            for input in inputs {
                lane_agrees_with_the_engine(input.as_bytes(), &format);
            }
        }
    }

    // The lane itself accepts strict input only when it validated the groups.
    let lane = |input: &str, policy: u32| {
        let format = INVARIANT.with_currency(symbol("$")).with_grouping(policy);
        lane::scan(input.as_bytes(), &format, true, &mut lane::Exact::default()).is_some()
    };
    let western_valid =
        ["1,234,567", "1,234", "1234567", "$1,234", "(1,234)", "1,234.5", "-1,234,567.25"];
    let western_refused = ["12,34,567", "1234,567", "12,34", "1,23,456", "1,2345", "12,34.5"];
    let indian_valid = ["12,34,567", "1,23,45,678", "1,234", "12,345", "$12,34,567.5", "1234567"];
    let indian_refused = ["1,234,567", "123,456", "12,34", "1,2,345", "12,345,678", "1,23,4567"];
    for input in western_valid {
        assert!(lane(input, NumFormat::GROUPING_WESTERN), "western lane reads {input}");
        assert!(lane(input, NumFormat::GROUPING_LEGACY), "legacy lane reads {input}");
    }
    for input in western_refused {
        assert!(!lane(input, NumFormat::GROUPING_WESTERN), "western lane refuses {input}");
        assert!(lane(input, NumFormat::GROUPING_LEGACY), "legacy lane reads {input}");
    }
    for input in indian_valid {
        assert!(lane(input, NumFormat::GROUPING_INDIAN), "indian lane reads {input}");
    }
    for input in indian_refused {
        assert!(!lane(input, NumFormat::GROUPING_INDIAN), "indian lane refuses {input}");
    }
}

// --- date options (item 5) ---

const ENGLISH: [(u8, &str); 13] = [
    (1, "Jan"),
    (2, "Feb"),
    (3, "Mar"),
    (4, "Apr"),
    (5, "May"),
    (6, "Jun"),
    (7, "Jul"),
    (8, "Aug"),
    (9, "Sep"),
    (9, "Sept"),
    (10, "Oct"),
    (11, "Nov"),
    (12, "Dec"),
];

/// The English vocabulary and one more entry.
fn english_with(extra: (u8, &str)) -> Vec<(u8, &str)> {
    ENGLISH.into_iter().chain([extra]).collect()
}

fn valid_options<'a>(options: DateOptions, names: &'a [(u8, &'a str)]) -> ValidDateOptions<'a> {
    options.validate(Some(MonthNames::from_entries(names))).unwrap()
}

/// A vocabulary packed as the ABI lays it out, written independently of `MonthNames::pack`.
fn packed(entries: &[(u8, &str)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for &(month, name) in entries {
        bytes.push(month);
        bytes.extend_from_slice(&(name.len() as u32).to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
    }
    bytes
}

#[test]
fn the_option_flags_and_the_profile() {
    assert_eq!(
        (DateOptions::COMPACT_DATE, DateOptions::SPACE_SEPARATOR, DateOptions::HOUR_OFFSET),
        (1, 2, 4)
    );
    assert_eq!(DateOptions::COMMON_FORMS.flags(), 7);
    assert_eq!(DateOptions::COMMON_FORMS.pivot(), 0);
    assert_eq!((DateOptions::NONE.flags(), DateOptions::NONE.pivot()), (0, 0));
    assert_eq!(DateOptions::default(), DateOptions::NONE);
    let pivoted = DateOptions::COMMON_FORMS.with_pivot(2029);
    assert_eq!((pivoted.flags(), pivoted.pivot()), (7, 2029));
    assert_eq!(DateOptions::from_flags(5).flags(), 5);
    let valid = pivoted.validate(None).unwrap();
    assert_eq!((valid.flags(), valid.pivot()), (7, 2029));
    assert!(valid.months().is_none());
    assert!(ValidDateOptions::NONE.months().is_none());
    assert_eq!(DateOptions::NONE.validate(None).unwrap().flags(), 0);
}

#[test]
fn the_options_validate_flags_then_pivot_then_vocabulary() {
    // An unknown flag bit is refused, whatever else is set; every defined subset is accepted.
    for bits in 0..8 {
        assert!(DateOptions::from_flags(bits).validate(None).is_ok(), "{bits}");
    }
    for bit in 3..32 {
        let refused = DateOptions::from_flags(1 << bit).validate(None);
        assert_eq!(refused.unwrap_err(), DeclarationError::DateFlags, "bit {bit}");
        assert_eq!(
            DateOptions::from_flags(1 << bit).with_pivot(99).validate(None).err(),
            Some(DeclarationError::DateFlags),
            "flags are judged before the pivot, bit {bit}"
        );
    }
    // The pivot: absent, or 100 through 9999.
    for (pivot, accepted) in [
        (0, true),
        (1, false),
        (99, false),
        (100, true),
        (2029, true),
        (9999, true),
        (10_000, false),
    ] {
        let verdict = DateOptions::NONE.with_pivot(pivot).validate(None);
        assert_eq!(verdict.is_ok(), accepted, "pivot {pivot}");
        if !accepted {
            assert_eq!(verdict.unwrap_err(), DeclarationError::DatePivot);
        }
    }
    assert_eq!(
        DateOptions::NONE.with_pivot(u16::MAX).validate(None).unwrap_err(),
        DeclarationError::DatePivot
    );
    // A bad pivot is reported before a bad vocabulary.
    let none = MonthNames::from_entries(&[]);
    assert_eq!(
        DateOptions::NONE.with_pivot(99).validate(Some(none)).unwrap_err(),
        DeclarationError::DatePivot
    );
    assert_eq!(
        DateOptions::NONE.validate(Some(none)).unwrap_err(),
        DeclarationError::MonthNames(MonthNamesFault::MonthMissing)
    );
    for error in [
        DeclarationError::DateFlags,
        DeclarationError::DatePivot,
        DeclarationError::MonthNames(MonthNamesFault::Duplicate),
    ] {
        assert!(!error.to_string().is_empty());
    }
    for fault in [
        MonthNamesFault::Layout,
        MonthNamesFault::Month,
        MonthNamesFault::Utf8,
        MonthNamesFault::NameLength,
        MonthNamesFault::Character,
        MonthNamesFault::MonthMissing,
        MonthNamesFault::Duplicate,
    ] {
        assert!(!fault.to_string().is_empty());
    }
}

#[test]
fn each_option_opens_its_one_shape_at_its_door() {
    let compact = DateOptions::from_flags(DateOptions::COMPACT_DATE).validate(None).unwrap();
    let space = DateOptions::from_flags(DateOptions::SPACE_SEPARATOR).validate(None).unwrap();
    let hour = DateOptions::from_flags(DateOptions::HOUR_OFFSET).validate(None).unwrap();
    let all = DateOptions::COMMON_FORMS.validate(None).unwrap();
    let date = |o: &ValidDateOptions| detailed::cast_date("20260107", Some(o));
    let stamp = |text: &str, o: &ValidDateOptions| detailed::cast_timestamp(text, Some(o));
    let january = Date { year: 2026, month: 1, day: 7 };
    assert_eq!(date(&compact), Ok(january));
    assert_eq!(date(&all), Ok(january));
    assert_eq!(date(&space).unwrap_err().detail, Detail::WrongLength);
    assert_eq!(date(&hour).unwrap_err().detail, Detail::WrongLength);

    let base = stamp("2026-01-07T15:04:05Z", &ValidDateOptions::NONE).unwrap();
    assert_eq!(stamp("2026-01-07 15:04:05Z", &space), Ok(base));
    assert_eq!(stamp("2026-01-07 15:04:05Z", &all), Ok(base));
    for refused in [&compact, &hour, &ValidDateOptions::NONE] {
        let failure = stamp("2026-01-07 15:04:05Z", refused).unwrap_err();
        assert_eq!(
            (failure.detail, failure.offset, failure.len),
            (Detail::DelimiterExpected, 10, 1)
        );
    }
    assert_eq!(stamp("2026-01-07T15:04:05+00", &hour), Ok(base));
    assert_eq!(stamp("2026-01-07T15:04:05+00", &all), Ok(base));
    for refused in [&compact, &space, &ValidDateOptions::NONE] {
        let failure = stamp("2026-01-07T15:04:05+00", refused).unwrap_err();
        assert_eq!(
            (failure.detail, failure.offset, failure.len),
            (Detail::DelimiterExpected, 22, 0)
        );
    }
    // The old doors never see any of it.
    assert!(cast_date("20260107").is_err());
    assert!(cast_timestamp("2026-01-07 15:04:05Z").is_err());
    assert!(cast_timestamp("2026-01-07T15:04:05+00").is_err());
}

#[test]
fn options_that_ask_for_nothing_change_nothing() {
    // Every corpus input of the four date doors, through the detailed door with no options,
    // with an options value that sets nothing and with the irrelevant ones: one answer.
    use serde_json::Value;
    let names = MonthNames::from_entries(&ENGLISH);
    let inert = [
        DateOptions::NONE.validate(None).unwrap(),
        DateOptions::NONE.validate(Some(names)).unwrap(),
    ];
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus");
    for file in ["date.json", "timestamp.json", "date_order.json", "datetime.json"] {
        let rows: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(root.join(file)).unwrap()).unwrap();
        for row in rows.iter().filter(|row| row.get("options").is_none()) {
            let input = row["input"].as_str().unwrap();
            let order = row.get("order").map(|o| DateOrder::from_code(o.as_u64().unwrap() as u32));
            for options in inert.iter().map(Some).chain([None]) {
                let (plain, twin) = match (file, order.flatten()) {
                    ("date.json", _) => (
                        detailed::cast_date(input, None).map(|_| ()),
                        detailed::cast_date(input, options).map(|_| ()),
                    ),
                    ("timestamp.json", _) => (
                        detailed::cast_timestamp(input, None).map(|_| ()),
                        detailed::cast_timestamp(input, options).map(|_| ()),
                    ),
                    ("date_order.json", Some(order)) => (
                        detailed::cast_date_ordered(input, order, None).map(|_| ()),
                        detailed::cast_date_ordered(input, order, options).map(|_| ()),
                    ),
                    (_, Some(order)) => (
                        detailed::cast_datetime(input, order, None).map(|_| ()),
                        detailed::cast_datetime(input, order, options).map(|_| ()),
                    ),
                    other => panic!("{other:?}"),
                };
                // A vocabulary alone opens names only, and no row without options has one.
                assert_eq!(twin, plain, "{file} {input:?}");
            }
        }
    }
}

#[test]
fn every_pivot_maps_a_two_digit_year_into_its_window() {
    for pivot in 100..=9999u16 {
        let valid = DateOptions::NONE.with_pivot(pivot).validate(None).unwrap();
        for yy in 0..100u16 {
            let text = format!("1/1/{yy:02}");
            let date =
                detailed::cast_date_ordered(&text, DateOrder::MonthDayYear, Some(&valid)).unwrap();
            let window = pivot - 99..=pivot;
            let year = window.clone().find(|year| year % 100 == yy).unwrap();
            assert_eq!(date.year, year, "pivot {pivot}, yy {yy}");
            assert!(window.contains(&date.year));
        }
    }
}

#[test]
fn the_pivot_leaves_other_year_widths_alone() {
    let pivot = DateOptions::NONE.with_pivot(2029).validate(None).unwrap();
    let year = |text: &str| {
        detailed::cast_date_ordered(text, DateOrder::MonthDayYear, Some(&pivot)).map(|d| d.year)
    };
    assert_eq!(year("1/7/2026"), Ok(2026));
    assert_eq!(year("1/7/0099"), Ok(99));
    for (text, width) in [("1/7/6", 1), ("1/7/126", 3)] {
        let failure = year(text).unwrap_err();
        assert_eq!((failure.detail, failure.offset, failure.len), (Detail::FieldWidth, 4, width));
    }
    // Without a pivot, the same two-digit year is a width fault, exactly as the old door has it.
    let failure = detailed::cast_date_ordered("1/7/26", DateOrder::MonthDayYear, None).unwrap_err();
    assert_eq!(cast_date_ordered("1/7/26", DateOrder::MonthDayYear), Err(failure.fault()));
    assert_eq!((failure.detail, failure.offset, failure.len), (Detail::FieldWidth, 4, 2));
}

#[test]
fn month_names_read_where_the_declared_order_puts_the_month() {
    let options = valid_options(DateOptions::NONE, &ENGLISH);
    let read = |text: &str, order| detailed::cast_date_ordered(text, order, Some(&options));
    let date = |month, day| Ok(Date { year: 2026, month, day });
    assert_eq!(read("7-Jan-2026", DateOrder::DayMonthYear), date(1, 7));
    assert_eq!(read("Jan-7-2026", DateOrder::MonthDayYear), date(1, 7));
    assert_eq!(read("2026-Jan-7", DateOrder::YearMonthDay), date(1, 7));
    // A vocabulary never overrides the declared order.
    for (text, order) in [
        ("Jan-7-2026", DateOrder::DayMonthYear),
        ("Jan-7-2026", DateOrder::YearMonthDay),
        ("7-Jan-2026", DateOrder::MonthDayYear),
    ] {
        assert_eq!(read(text, order).unwrap_err().detail, Detail::DigitExpected, "{text}");
    }
    // Names fold, simply: the Kelvin sign is a `k`, and costs three bytes where `k` costs one.
    let german = [
        (1, "Jan"),
        (2, "Feb"),
        (3, "Mär"),
        (4, "Apr"),
        (5, "Mai"),
        (6, "Jun"),
        (7, "Jul"),
        (8, "Aug"),
        (9, "Sep"),
        (10, "Okt"),
        (11, "Nov"),
        (12, "Dez"),
    ];
    let options = valid_options(DateOptions::NONE, &german);
    let read =
        |text: &str| detailed::cast_date_ordered(text, DateOrder::DayMonthYear, Some(&options));
    assert_eq!(read("7-O\u{212A}T-2026"), Ok(Date { year: 2026, month: 10, day: 7 }));
    assert_eq!(read("7-MÄR-2026"), Ok(Date { year: 2026, month: 3, day: 7 }));
    // `ß` is not `ss`: folding is simple, never full.
    let sharp = [(1, "ß")].into_iter().chain(german.into_iter().skip(1));
    let sharp: Vec<(u8, &str)> = sharp.collect();
    let options = valid_options(DateOptions::NONE, &sharp);
    let read =
        |text: &str| detailed::cast_date_ordered(text, DateOrder::DayMonthYear, Some(&options));
    assert!(read("7-ß-2026").is_ok());
    assert_eq!(read("7-SS-2026").unwrap_err().detail, Detail::NotRecognized);
}

#[test]
fn month_names_match_the_longest_eligible_entry() {
    let dotted = [
        (1, "janv."),
        (2, "févr."),
        (3, "mar"),
        (3, "mar."),
        (4, "avr."),
        (5, "mai"),
        (6, "juin"),
        (7, "juil."),
        (8, "août"),
        (9, "sept."),
        (10, "oct."),
        (11, "nov."),
        (12, "déc."),
    ];
    let options = valid_options(DateOptions::NONE, &dotted);
    let read =
        |text: &str| detailed::cast_date_ordered(text, DateOrder::DayMonthYear, Some(&options));
    let (march, april) =
        (Date { year: 2026, month: 3, day: 7 }, Date { year: 2026, month: 4, day: 7 });
    // Both `mar` and `mar.` are followed by the `.` separator: the longer wins.
    assert_eq!(read("7.mar..2026"), Ok(march));
    // Only `mar` is followed by the separator here: the longer is not eligible, so it loses.
    assert_eq!(read("7.mar.2026"), Ok(march));
    // Punctuation is part of the name and is never stripped.
    assert_eq!(read("7.avr..2026"), Ok(april));
    let failure = read("7.avr.2026").unwrap_err();
    assert_eq!((failure.reason, failure.detail), (Reason::Malformed, Detail::NotRecognized));
    assert_eq!((failure.offset, failure.len), (2, 3));
    // The separator after the name is the one declared by the first.
    let failure = read("7-mar.2026").unwrap_err();
    assert_eq!((failure.detail, failure.offset, failure.len), (Detail::DelimiterExpected, 5, 1));
}

#[test]
fn an_unmatched_month_is_the_word_or_the_missing_mark() {
    let options = valid_options(DateOptions::NONE, &ENGLISH);
    let read =
        |text: &str| detailed::cast_date_ordered(text, DateOrder::DayMonthYear, Some(&options));
    let spans = |text: &str| {
        let failure = read(text).unwrap_err();
        (failure.reason, failure.detail, failure.offset, failure.len)
    };
    let (m, nr, de, dx) = (
        Reason::Malformed,
        Detail::NotRecognized,
        Detail::DelimiterExpected,
        Detail::DigitExpected,
    );
    assert_eq!(spans("7-Foo-2026"), (m, nr, 2, 3));
    assert_eq!(spans("7-Janu-2026"), (m, nr, 2, 4));
    assert_eq!(spans("7-Ja-2026"), (m, nr, 2, 2));
    assert_eq!(spans("7-é-2026"), (m, nr, 2, 2));
    // A declared name with no separator after it is a missing mark, not an unknown word.
    assert_eq!(spans("7-Jan"), (m, de, 5, 0));
    assert_eq!(spans("7-Jan 3"), (m, de, 5, 1));
    assert_eq!(spans("7-Jan\u{00A0}3"), (m, de, 5, 2));
    assert_eq!(spans("7-Jan/2026"), (m, de, 5, 1));
    // No word at all is today's digit fault.
    assert_eq!(spans("7--2026"), (m, dx, 2, 1));
    assert_eq!(spans("7-"), (m, dx, 2, 0));
}

#[test]
fn month_names_pack_as_pack_enum_does() {
    let names = MonthNames::from_entries(&ENGLISH);
    let expected = packed(&ENGLISH);
    // The length first, with nothing written when the buffer is short.
    assert_eq!(names.pack(&mut []), Some(expected.len()));
    let mut short = vec![0xA5; expected.len() - 1];
    assert_eq!(names.pack(&mut short), Some(expected.len()));
    assert!(short.iter().all(|&byte| byte == 0xA5), "a short buffer is untouched");
    // Exactly enough, and more than enough: the tail is untouched.
    let mut out = vec![0xA5; expected.len() + 7];
    assert_eq!(names.pack(&mut out), Some(expected.len()));
    assert_eq!(out[..expected.len()], expected[..]);
    assert!(out[expected.len()..].iter().all(|&byte| byte == 0xA5));
    // What was read packs back to itself.
    let read = MonthNames::from_packed(&expected).unwrap();
    let mut again = vec![0; expected.len()];
    assert_eq!(read.pack(&mut again), Some(expected.len()));
    assert_eq!(again, expected);
    assert_eq!(read.pack(&mut []), Some(expected.len()));
    let mut short = vec![0xA5; expected.len() - 1];
    assert_eq!(read.pack(&mut short), Some(expected.len()));
    assert!(short.iter().all(|&byte| byte == 0xA5));
    // The two sources judge and read alike.
    let from_bytes = DateOptions::NONE.validate(Some(read)).unwrap();
    let built = DateOptions::NONE.validate(Some(names)).unwrap();
    for text in ["7-Sept-2026", "7-sep-2026", "7-Foo-2026"] {
        assert_eq!(
            detailed::cast_date_ordered(text, DateOrder::DayMonthYear, Some(&from_bytes)),
            detailed::cast_date_ordered(text, DateOrder::DayMonthYear, Some(&built)),
            "{text}"
        );
    }
    // An empty vocabulary packs to nothing, and is itself refused.
    assert_eq!(MonthNames::from_entries(&[]).pack(&mut []), Some(0));
    assert_eq!(
        MonthNames::from_packed(&[]).unwrap_err(),
        DeclarationError::MonthNames(MonthNamesFault::MonthMissing)
    );
}

#[test]
fn a_vocabulary_is_judged_in_full_by_its_own_rule() {
    let refused = |entries: &[(u8, &str)], extra: &[u8]| {
        let mut bytes = packed(entries);
        bytes.extend_from_slice(extra);
        MonthNames::from_packed(&bytes).err()
    };
    let fault = |fault| Some(DeclarationError::MonthNames(fault));
    // Layout comes first: a month byte of 0 that is cut short is a cut entry, not a bad month.
    assert_eq!(refused(&ENGLISH, &[0]), fault(MonthNamesFault::Layout));
    assert_eq!(refused(&ENGLISH, &[0, 3, 0, 0]), fault(MonthNamesFault::Layout));
    assert_eq!(
        refused(&ENGLISH, &[1, 0xFF, 0xFF, 0xFF, 0xFF, b'a']),
        fault(MonthNamesFault::Layout)
    );
    // A name is 1 to 64 bytes, not characters.
    assert_eq!(refused(&english_with((1, "")), &[]), fault(MonthNamesFault::NameLength));
    assert_eq!(
        refused(&english_with((1, &"é".repeat(33))), &[]),
        fault(MonthNamesFault::NameLength)
    );
    assert_eq!(refused(&english_with((1, &"é".repeat(32))), &[]), None);
    assert_eq!(
        refused(&english_with((1, &"a".repeat(65))), &[]),
        fault(MonthNamesFault::NameLength)
    );
    assert_eq!(refused(&english_with((1, &"a".repeat(64))), &[]), None);
    // Only ASCII digits, `/`, `-` and white space are forbidden; `.` and look-alikes are not.
    for (name, ok) in [
        ("a1", false),
        ("a/", false),
        ("a-", false),
        ("a b", false),
        ("a\u{3000}", false),
        ("a.", true),
        (".", true),
        ("a\u{0663}", true),
        ("a\u{200B}", true),
        ("a\u{FF0D}", true),
    ] {
        let want = if ok { None } else { fault(MonthNamesFault::Character) };
        assert_eq!(refused(&english_with((1, name)), &[]), want, "{name:?}");
    }
    // A fold-equal pair is a duplicate even for one month, and a month needs an entry.
    assert_eq!(refused(&english_with((1, "JAN")), &[]), fault(MonthNamesFault::Duplicate));
    assert_eq!(refused(&english_with((2, "jan")), &[]), fault(MonthNamesFault::Duplicate));
    assert_eq!(refused(&english_with((13, "x")), &[]), fault(MonthNamesFault::Month));
    assert_eq!(refused(&english_with((0, "x")), &[]), fault(MonthNamesFault::Month));
    let no_december: Vec<_> = ENGLISH.into_iter().filter(|&(month, _)| month != 12).collect();
    assert_eq!(refused(&no_december, &[]), fault(MonthNamesFault::MonthMissing));
    // Invalid UTF-8 in a name, which only the packed layout can carry.
    assert_eq!(refused(&ENGLISH, &[1, 1, 0, 0, 0, 0xFF]), fault(MonthNamesFault::Utf8));
    // The builder is judged by `validate` the same way.
    let built = |entries: &[(u8, &str)]| {
        DateOptions::NONE.validate(Some(MonthNames::from_entries(entries))).err()
    };
    assert_eq!(built(&english_with((1, "Ja1"))), fault(MonthNamesFault::Character));
    assert_eq!(built(&english_with((1, "jan"))), fault(MonthNamesFault::Duplicate));
    assert_eq!(built(&ENGLISH), None);
}

/// The duplicate rule across the 64 entries whose keys the judge keeps on the stack: a
/// duplicate is found wherever its pair sits, kept or not, and the first broken rule in entry
/// order still wins.
#[test]
fn a_vocabulary_past_64_entries_is_judged_as_any_other() {
    // 130 distinct names without digits ("ma", "mb", …, "mfz"), every month covered.
    let name = |index: usize| {
        let mut name = String::from("m");
        let mut rest = index;
        loop {
            name.push(char::from(b'a' + (rest % 26) as u8));
            rest /= 26;
            if rest == 0 {
                break name;
            }
        }
    };
    let names: Vec<String> = (0..130).map(name).collect();
    let entries = |names: &[String]| -> Vec<(u8, String)> {
        names
            .iter()
            .enumerate()
            .map(|(index, name)| ((index % 12 + 1) as u8, name.clone()))
            .collect()
    };
    let judged = |entries: &[(u8, String)]| {
        let borrowed: Vec<(u8, &str)> = entries.iter().map(|(m, n)| (*m, n.as_str())).collect();
        let built = DateOptions::NONE.validate(Some(MonthNames::from_entries(&borrowed))).err();
        let read = MonthNames::from_packed(&packed(&borrowed)).err();
        assert_eq!(built, read, "{entries:?}");
        built
    };
    let fault = |fault| Some(DeclarationError::MonthNames(fault));
    let with = |changes: &[(usize, &str)]| {
        let mut names = names.clone();
        for &(at, name) in changes {
            names[at] = name.to_owned();
        }
        entries(&names)
    };

    assert_eq!(judged(&entries(&names)), None, "130 distinct names are accepted");
    let first = names[0].clone();
    // A duplicate of entry 0 at the first unkept index, and at the one after it.
    assert_eq!(judged(&with(&[(64, &first)])), fault(MonthNamesFault::Duplicate));
    assert_eq!(judged(&with(&[(65, &first)])), fault(MonthNamesFault::Duplicate));
    // A duplicate between two entries neither of which is kept.
    let seventy = names[70].clone();
    assert_eq!(judged(&with(&[(80, &seventy)])), fault(MonthNamesFault::Duplicate));
    // A case variant past 64, of a kept entry and of an unkept one.
    let upper = names[3].to_uppercase();
    assert_eq!(judged(&with(&[(100, &upper)])), fault(MonthNamesFault::Duplicate));
    let upper = names[90].to_uppercase();
    assert_eq!(judged(&with(&[(120, &upper)])), fault(MonthNamesFault::Duplicate));
    // The first broken rule in entry order wins either side of the boundary.
    assert_eq!(
        judged(&with(&[(80, &seventy), (90, "bad name")])),
        fault(MonthNamesFault::Duplicate)
    );
    assert_eq!(
        judged(&with(&[(75, "bad name"), (80, &seventy)])),
        fault(MonthNamesFault::Character)
    );
}

mod sites {
    use super::*;
    use crate::site::{DOCS, SITES, Site, impractical, recorder, unreachable};
    use serde_json::Value;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn rows(path: PathBuf) -> Vec<Value> {
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        serde_json::from_str(&text).unwrap()
    }

    fn corpus(name: &str) -> Vec<Value> {
        rows(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus").join(name))
    }

    fn text(row: &Value) -> Vec<u8> {
        match (row.get("input"), row.get("input_bytes")) {
            (_, Some(bytes)) => {
                bytes.as_array().unwrap().iter().map(|b| b.as_u64().unwrap() as u8).collect()
            }
            (Some(input), _) => input.as_str().unwrap().as_bytes().to_vec(),
            _ => Vec::new(),
        }
    }

    fn bits(row: &Value) -> f64 {
        f64::from_bits(u64::from_str_radix(row["bits"].as_str().unwrap(), 16).unwrap())
    }

    fn format_of(row: &Value) -> NumFormat {
        let Some(format) = row.get("format") else {
            return NumFormat::INVARIANT;
        };
        let symbol = |field: &str| match format.get(field).and_then(Value::as_str) {
            Some(text) => Symbol::new(text).unwrap(),
            None => Symbol::NONE,
        };
        let currency = format
            .get("currency")
            .and_then(Value::as_str)
            .map_or(CurrencySymbol::NONE, |symbol| CurrencySymbol::new(symbol).unwrap());
        let digits =
            format.get("digit_zero").and_then(Value::as_str).map_or(NativeDigits::NONE, |zero| {
                NativeDigits::new(zero.chars().next().unwrap()).unwrap()
            });
        NumFormat::with_separators(
            symbol("decimal_sep"),
            symbol("group_sep"),
            format["flags"].as_u64().unwrap() as u32,
        )
        .with_currency(currency)
        .with_currency_separators(symbol("currency_decimal_sep"), symbol("currency_group_sep"))
        .with_signs(symbol("minus_sign"), symbol("plus_sign"))
        .with_percent(symbol("percent"))
        .with_digits(digits)
        .with_grouping(match format.get("grouping").and_then(Value::as_str) {
            Some("western") => NumFormat::GROUPING_WESTERN,
            Some("indian") => NumFormat::GROUPING_INDIAN,
            Some("legacy") => NumFormat::GROUPING_LEGACY,
            _ => format["flags"].as_u64().unwrap() as u32 & NumFormat::GROUPING_POLICY,
        })
    }

    fn order(row: &Value) -> DateOrder {
        DateOrder::from_code(row["order"].as_u64().unwrap() as u32).unwrap()
    }

    fn epoch(row: &Value) -> ExcelEpoch {
        ExcelEpoch::from_code(row["epoch"].as_u64().unwrap() as u32).unwrap()
    }

    fn periods(row: &Value) -> DayPeriods {
        let designator = |field: &str| match row.get(field).and_then(Value::as_str) {
            Some(text) => Symbol::new(text).unwrap(),
            None => Symbol::NONE,
        };
        DayPeriods::new(designator("am"), designator("pm"))
    }

    /// A row's date options (none when it declares none), validated as the C ABI does with the
    /// vocabulary in its packed layout, for the detailed date doors.
    fn dated<T>(row: &Value, door: impl FnOnce(Option<&ValidDateOptions<'_>>) -> T) -> T {
        let Some(options) = row.get("options") else {
            return door(None);
        };
        let flag = |name: &str| match name {
            "compact_date" => DateOptions::COMPACT_DATE,
            "space_separator" => DateOptions::SPACE_SEPARATOR,
            "hour_offset" => DateOptions::HOUR_OFFSET,
            "common_forms" => DateOptions::COMMON_FORMS.flags(),
            other => panic!("flag {other}"),
        };
        let flags = options["flags"]
            .as_array()
            .unwrap()
            .iter()
            .fold(0, |all, f| all | flag(f.as_str().unwrap()));
        let pivot = options.get("pivot").map_or(0, |pivot| pivot.as_u64().unwrap() as u16);
        let mut packed = Vec::new();
        for pair in options.get("months").and_then(Value::as_array).into_iter().flatten() {
            let name = pair[1].as_str().unwrap();
            packed.push(pair[0].as_u64().unwrap() as u8);
            packed.extend_from_slice(&(name.len() as u32).to_le_bytes());
            packed.extend_from_slice(name.as_bytes());
        }
        let months = options.get("months").map(|_| MonthNames::from_packed(&packed).unwrap());
        let valid = DateOptions::from_flags(flags).with_pivot(pivot).validate(months).unwrap();
        door(Some(&valid))
    }

    /// A checked row's declaration, through `from_raw` as the C ABI hands it over.
    fn checked<T: Numeric>(
        row: &Value,
        run: impl FnOnce(&Constraints<'_, T>) -> Option<Failure>,
    ) -> Option<Failure> {
        let c = &row["constraints"];
        let dec = |field: &str| {
            c.get(field).map(|v| decimal_for_declaration(v.as_str().unwrap()).unwrap())
        };
        let mut raw = RawConstraints::ZERO;
        for (field, flag, upper) in [
            ("minimum", RawConstraints::HAS_MINIMUM, false),
            ("exclusiveMinimum", RawConstraints::HAS_EXCLUSIVE_MINIMUM, false),
            ("maximum", RawConstraints::HAS_MAXIMUM, true),
            ("exclusiveMaximum", RawConstraints::HAS_EXCLUSIVE_MAXIMUM, true),
        ] {
            if let Some(value) = dec(field) {
                raw.flags |= flag;
                *(if upper { &mut raw.upper } else { &mut raw.lower }) = value;
            }
        }
        if let Some(value) = dec("multipleOf") {
            raw.flags |= RawConstraints::HAS_MULTIPLE_OF;
            raw.multiple_of = value;
        }
        raw.rounding = match c.get("rounding").and_then(Value::as_str) {
            Some("midpoint") => 1,
            Some("floor") => 2,
            Some("ceiling") => 3,
            _ => 0,
        };
        let mut entries = Vec::new();
        if let Some(list) = c.get("enum").and_then(Value::as_array) {
            raw.flags |= RawConstraints::HAS_ENUM;
            for entry in list {
                let d = decimal_for_declaration(entry.as_str().unwrap()).unwrap();
                entries.extend_from_slice(&d.lo.to_ne_bytes());
                entries.extend_from_slice(&d.hi.to_ne_bytes());
                entries.extend_from_slice(&[d.scale, d.negative.into(), 0, 0]);
            }
        }
        let constraints = Constraints::<T>::from_raw(raw, &entries).unwrap();
        run(&constraints)
    }

    fn numeric_row<T: Numeric>(row: &Value) -> Option<Failure> {
        let failure = |rejection: detailed::Rejection| match rejection {
            detailed::Rejection::Failure(failure) => Some(failure),
            detailed::Rejection::Violation(_) => None,
        };
        checked::<T>(row, |constraints| {
            let verdict = match row.get("from_f64") {
                Some(value) => {
                    let value = match value.as_str() {
                        Some("-0") => -0.0,
                        Some("NaN") => f64::NAN,
                        Some("Infinity") => f64::INFINITY,
                        Some("-Infinity") => f64::NEG_INFINITY,
                        Some(other) => panic!("from_f64 {other}"),
                        None => value.as_f64().unwrap(),
                    };
                    detailed::checked_from_f64(value, constraints)
                }
                None => {
                    let format = format_of(row).validate().unwrap();
                    detailed::cast_checked(text(row), &format, constraints)
                }
            };
            verdict.err().and_then(failure)
        })
    }

    fn text_row(row: &Value) -> Option<Failure> {
        let c = &row["constraints"];
        let length = |field: &str| c.get(field).map(|n| n.as_u64().unwrap() as u32);
        let rules = TextRules::new()
            .trim(c.get("trim").and_then(Value::as_bool) == Some(true))
            .maybe_min_length(length("minLength"))
            .maybe_max_length(length("maxLength"));
        let entries: Option<Vec<&str>> = c
            .get("enum")
            .map(|list| list.as_array().unwrap().iter().map(|e| e.as_str().unwrap()).collect());
        let constraints = Constraints::text(rules, entries.as_deref()).unwrap();
        match detailed::cast_text_checked(&text(row), &constraints) {
            Err(detailed::Rejection::Failure(failure)) => Some(failure),
            _ => None,
        }
    }

    /// One row through its detailed door: the failure, or `None` for a value or a violation.
    fn replay(file: &str, row: &Value) -> Option<Failure> {
        let input = text(row);
        let input = &input[..];
        let drop_ok = |verdict: Result<(), Failure>| verdict.err();
        macro_rules! door {
            ($call:expr) => {
                drop_ok($call.map(drop))
            };
        }
        match file {
            "boolean.json" => door!(detailed::cast_bool(input)),
            "char.json" => door!(detailed::cast_char(input)),
            "date.json" => door!(dated(row, |o| detailed::cast_date(input, o))),
            "date_order.json" => {
                door!(dated(row, |o| detailed::cast_date_ordered(input, order(row), o)))
            }
            "datetime.json" => door!(dated(row, |o| {
                detailed::cast_datetime_with(input, order(row), &periods(row), o)
            })),
            "duration.json" => door!(detailed::cast_duration(input)),
            "excel_serial.json" => door!(detailed::cast_excel_serial(input, epoch(row))),
            "time.json" => door!(detailed::cast_time(input)),
            "timestamp.json" => door!(dated(row, |o| detailed::cast_timestamp(input, o))),
            "uuid.json" => door!(detailed::cast_uuid(input)),
            "unix.json" => {
                let precision =
                    UnixPrecision::from_code(row["precision"].as_u64().unwrap() as u32).unwrap();
                door!(detailed::cast_unix(input, precision))
            }
            "decimal.json" => {
                door!(detailed::cast_decimal(input, &format_of(row).validate().unwrap()))
            }
            "integer.json" | "real.json" => {
                let f = &format_of(row).validate().unwrap();
                match row["type"].as_str().unwrap() {
                    "i8" => door!(detailed::cast_i8(input, f)),
                    "i16" => door!(detailed::cast_i16(input, f)),
                    "i32" => door!(detailed::cast_i32(input, f)),
                    "i64" => door!(detailed::cast_i64(input, f)),
                    "u8" => door!(detailed::cast_u8(input, f)),
                    "u16" => door!(detailed::cast_u16(input, f)),
                    "u32" => door!(detailed::cast_u32(input, f)),
                    "u64" => door!(detailed::cast_u64(input, f)),
                    "f32" => door!(detailed::cast_f32(input, f)),
                    "f64" => door!(detailed::cast_f64(input, f)),
                    other => panic!("type {other}"),
                }
            }
            "typed.json" | "typed_rust.json" => {
                let value = bits(row);
                let seconds = UnixPrecision::Seconds;
                match row["door"].as_str().unwrap() {
                    "decimal" => door!(detailed::decimal_from_f64(value)),
                    "excel_serial" => door!(detailed::excel_serial(value, epoch(row))),
                    "excel_time" => door!(detailed::excel_time(value)),
                    "excel_duration" => door!(detailed::excel_duration(value)),
                    "i8" => door!(detailed::i8_from_f64(value)),
                    "i16" => door!(detailed::i16_from_f64(value)),
                    "i32" => door!(detailed::i32_from_f64(value)),
                    "i64" => door!(detailed::i64_from_f64(value)),
                    "u8" => door!(detailed::u8_from_f64(value)),
                    "u16" => door!(detailed::u16_from_f64(value)),
                    "u32" => door!(detailed::u32_from_f64(value)),
                    "u64" => door!(detailed::u64_from_f64(value)),
                    "f32" => door!(detailed::f32_from_f64(value)),
                    "bool" => door!(detailed::bool_from_f64(value)),
                    "unix" => door!(detailed::unix_from_f64(value, seconds)),
                    other => panic!("typed door {other}"),
                }
            }
            "constraints.json" => match row["door"].as_str().unwrap() {
                "i8" => numeric_row::<i8>(row),
                "i16" => numeric_row::<i16>(row),
                "i32" => numeric_row::<i32>(row),
                "i64" => numeric_row::<i64>(row),
                "u8" => numeric_row::<u8>(row),
                "u16" => numeric_row::<u16>(row),
                "u32" => numeric_row::<u32>(row),
                "u64" => numeric_row::<u64>(row),
                "f32" => numeric_row::<f32>(row),
                "f64" => numeric_row::<f64>(row),
                "decimal" => numeric_row::<Decimal>(row),
                "text" => text_row(row),
                other => panic!("constraints door {other}"),
            },
            other => panic!("no replay for {other}"),
        }
    }

    /// The corpus files a door reads, and the Rust-only typed rows.
    const FILES: [&str; 16] = [
        "boolean.json",
        "char.json",
        "constraints.json",
        "date.json",
        "date_order.json",
        "datetime.json",
        "decimal.json",
        "duration.json",
        "excel_serial.json",
        "integer.json",
        "real.json",
        "time.json",
        "timestamp.json",
        "typed.json",
        "unix.json",
        "uuid.json",
    ];

    /// Every corpus row (and every Rust-only typed row) through its detailed door, recording
    /// on this thread only which sites fire: each failing row fires exactly one, the site of
    /// the failure returned, whose reason is the failure's and whose detail is too (or the
    /// detail is `invalid_utf8`, decided from the bytes); a row that parses fires none. Then
    /// every site has fired but the ones the table lists as unreachable, and none of those.
    #[test]
    fn every_site_is_reached() {
        let mut every = Vec::new();
        for file in FILES {
            let skip = matches!(file, "constraints.json");
            for row in corpus(file) {
                if skip && row["expect"] == "refused" {
                    continue;
                }
                every.push((file, row));
            }
        }
        let typed = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/typed_rust.json");
        every.extend(rows(typed).into_iter().map(|row| ("typed_rust.json", row)));

        let mut reached = BTreeSet::new();
        recorder::start();
        for (file, row) in &every {
            let failure = replay(file, row);
            let fired = recorder::take();
            let label = format!("{file} {}", row.get("input").unwrap_or(&Value::Null));
            match failure {
                None if row["expect"] == "violation" || row["expect"] == "ok" => {
                    assert!(fired.is_empty(), "{label}: parsed, yet fired {fired:?}");
                }
                None => panic!("{label}: expected a failure"),
                Some(failure) => {
                    let [site] = fired[..] else {
                        panic!("{label}: fired {fired:?} for one failure");
                    };
                    assert_eq!(site.reason(), failure.reason, "{label}: {site:?}'s reason");
                    assert!(
                        failure.detail == site.detail() || failure.detail == Detail::InvalidUtf8,
                        "{label}: {site:?} gave {:?}",
                        failure.detail
                    );
                    reached.insert(site);
                }
            }
        }

        // An impractical site is reached by its named unit test, recorded the same way. One
        // impractical only on 64 bits is, where `usize` is 32 bits, structurally unreachable:
        // its test's input can't be built, and must neither fail nor fire.
        let mut unreached = Vec::new();
        let mut structural = BTreeSet::new();
        for (site, test, _, only_64bit) in impractical() {
            assert!(
                !reached.contains(&site),
                "{site:?} is marked impractical, yet a row reaches it"
            );
            let Some(&(_, reach)) = reach::ALL.iter().find(|(name, _)| *name == test) else {
                panic!("{site:?} names unit test `{test}`, which reach::ALL doesn't hold");
            };
            recorder::take();
            let failure = reach();
            let fired = recorder::take();
            if only_64bit && usize::BITS < 64 {
                assert!(
                    failure.is_none() && fired.is_empty(),
                    "{site:?}: `{test}` reached it with a {}-bit usize ({fired:?})",
                    usize::BITS
                );
                structural.insert(site);
                continue;
            }
            match failure {
                Some(failure) if fired[..] == [site] => {
                    assert_eq!(
                        (failure.reason, failure.detail),
                        (site.reason(), site.detail()),
                        "{test}: {site:?}"
                    );
                    reached.insert(site);
                }
                _ => unreached.push(site),
            }
        }
        recorder::stop();
        assert!(
            unreached.is_empty(),
            "impractical sites their unit tests don't reach: {unreached:?}"
        );

        let unreachable: BTreeSet<Site> = unreachable().map(|(site, _)| site).collect();
        let missing: Vec<Site> = SITES
            .iter()
            .map(|&(site, ..)| site)
            .filter(|site| {
                !reached.contains(site) && !unreachable.contains(site) && !structural.contains(site)
            })
            .collect();
        assert!(missing.is_empty(), "no corpus row reaches {missing:?}");
        let listed: Vec<&Site> = unreachable.intersection(&reached).collect();
        assert!(listed.is_empty(), "listed unreachable, yet reached: {listed:?}");
    }

    /// The bodies of the unit tests that reach the impractical sites, each returning the
    /// failure its input gets (`every_site_is_reached` calls them under the recorder).
    mod reach {
        use super::*;

        /// A unit test's body: the failure its input gets.
        type Reach = fn() -> Option<Failure>;

        pub(super) const ALL: &[(&str, Reach)] =
            &[("iso_overflow", iso_overflow), ("checked_span_too_long", checked_span_too_long)];

        /// An ISO duration of 300,000 maximal week components: 5.7 MB, past 128 bits of
        /// nanoseconds.
        pub(super) fn iso_overflow() -> Option<Failure> {
            let mut text = String::from("P");
            for _ in 0..300_000 {
                text.push_str("999999999999999999W");
            }
            detailed::cast_duration(&text).err()
        }

        /// A success span ending past `u32::MAX`, decided where the text door decides it,
        /// without 4 GiB of input.
        pub(super) fn checked_span_too_long() -> Option<Failure> {
            let past = usize::try_from(u64::from(u32::MAX) + 1).ok()?;
            crate::checked::exact_span(0, past).err()
        }
    }

    #[test]
    fn iso_overflow() {
        let failure = reach::iso_overflow().unwrap();
        assert_eq!((failure.reason, failure.detail), (Reason::OutOfRange, Detail::TooLarge));
        assert_eq!((failure.offset, failure.len), (0, 5_700_001));
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn checked_span_too_long() {
        let failure = reach::checked_span_too_long().unwrap();
        assert_eq!((failure.reason, failure.detail), (Reason::OutOfRange, Detail::TooLong));
        assert_eq!((failure.offset, failure.len), (0, u32::MAX));
    }

    /// With a 32-bit `usize` no span can end past `u32::MAX`: the site can't fire.
    #[cfg(not(target_pointer_width = "64"))]
    #[test]
    fn checked_span_too_long() {
        assert_eq!(reach::checked_span_too_long(), None);
        assert!(crate::checked::exact_span(usize::MAX, usize::MAX).is_ok());
    }

    /// The table names every site once, in declaration order, with the reason and detail the
    /// site reports; every unreachable entry gives its reason.
    #[test]
    fn the_site_table_is_whole() {
        for (index, &(site, reason, detail)) in SITES.iter().enumerate() {
            assert_eq!(site as usize, index, "{site:?} is listed once, in order");
            assert_eq!((site.reason(), site.detail()), (reason, detail), "{site:?}");
        }
        assert_eq!(DOCS.len(), SITES.len());
        for (site, why) in unreachable() {
            assert!(why.len() > 20, "{site:?} says why it is unreachable");
        }
        for (site, test, why, _) in impractical() {
            assert!(why.len() > 20 && !test.is_empty(), "{site:?} names its test and says why");
        }
        for &(site, kind, _) in crate::site::MARKS {
            assert!(
                matches!(kind, "unreachable" | "impractical" | "impractical_on_64bit"),
                "{site:?}: unknown mark {kind}"
            );
        }
        // Empty input is `blank` and nothing else is.
        for &(site, reason, detail) in SITES {
            assert_eq!(reason == Reason::Empty, detail == Detail::Blank, "{site:?}");
        }
    }

    /// `docs/fault-sites.md`, generated from the table: `HYPERCAST_BLESS=1` rewrites it, and
    /// the test fails when it differs.
    #[test]
    fn docs_fault_sites_is_current() {
        let mut doc = String::from(
            "# Fault sites\n\n\
             Generated from `rust/src/site.rs` by `docs_fault_sites_is_current` in \
             `rust/src/tests.rs`; never edit it by hand (`HYPERCAST_BLESS=1 cargo test --lib \
             docs_fault_sites` in `rust/` rewrites it).\n\n\
             Every place a door fails, with the reason and detail it reports. A detailed door \
             reports `invalid_utf8` instead wherever the bytes at the failure's span don't \
             decode as UTF-8. The corpus reaches every site but two kinds, as \
             `every_site_is_reached` checks: an **unreachable** site is a defensive check that \
             no input reaches, for the reason given; an **impractical** site is reached only by \
             inputs no corpus row can carry (megabytes, or past 4 GiB), and the named unit test \
             in `rust/src/tests.rs` reaches it instead, under the same check.\n\n\
             | Site | File | Reason | Detail | Condition |\n\
             |---|---|---|---|---|\n",
        );
        let reason = |reason: Reason| match reason {
            Reason::Empty => "empty",
            Reason::Malformed => "malformed",
            Reason::OutOfRange => "out_of_range",
        };
        for (&(site, file, condition), &(_, r, detail)) in DOCS.iter().zip(SITES) {
            let mut condition: String =
                condition.split_whitespace().collect::<Vec<_>>().join(" ").replace('|', "\\|");
            if let Some((_, why)) = unreachable().find(|(s, _)| *s == site) {
                condition.push_str(&format!(" **Unreachable:** {}.", why.replace('|', "\\|")));
            }
            if let Some((_, test, why, only_64bit)) = impractical().find(|(s, ..)| *s == site) {
                condition.push_str(&format!(
                    " **Impractical:** {}; reached by the unit test `{test}`.",
                    why.replace('|', "\\|")
                ));
                if only_64bit {
                    condition.push_str(
                        " Where `usize` is 32 bits (wasm32) no length can pass `u32::MAX`, so the \
                         site is structurally unreachable there, which the coverage test asserts.",
                    );
                }
            }
            doc.push_str(&format!(
                "| `{site:?}` | `{file}` | `{}` | `{}` | {condition} |\n",
                reason(r),
                detail.name()
            ));
        }
        doc.push_str(&format!(
            "\n{} sites: {} unreachable, {} impractical.\n",
            SITES.len(),
            unreachable().count(),
            impractical().count()
        ));
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/fault-sites.md");
        let bless = std::env::var_os("HYPERCAST_BLESS").is_some();
        if !bless && !path.parent().is_some_and(|docs| docs.is_dir()) {
            eprintln!("skipped: no docs/ beside rust/, so this is not a whole checkout");
            return;
        }
        if bless {
            std::fs::write(&path, &doc).unwrap();
        }
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(current == doc, "docs/fault-sites.md is stale: run with HYPERCAST_BLESS=1");
    }
}
