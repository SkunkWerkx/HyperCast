//! Text in, the right value out: each value formats to a canonical text and casts back to
//! itself, and every string near a valid one — one character replaced, removed or inserted,
//! multi-byte UTF-8 included — casts exactly when a deliberately naive reference parser below
//! says it should, to the value that reference gives.
//!
//! The references share nothing with the crate's parsers (fixed offsets, `str::split`,
//! `from_str_radix`, lookup lists), so they cannot agree with a bug by construction. The
//! fuzz target (`fuzz/`) and `cargo no-panic` prove the doors never crash and that every
//! fault span stays inside the input; what neither can see is a cast that succeeds with the
//! wrong value, or fails on text it should accept. That is what this file checks.
//!
//! Covered here: the UUID door (all five .NET forms plus the `urn:uuid:`/`guid:`/`uuid:`
//! prefixes), the boolean door, the eight integer doors under a format with every lenience
//! off (and that turning lenience on never changes a plain value), the strict ISO date door,
//! the f32/f64 doors' round trip of Rust's own shortest-round-trip formatting, and the
//! decimal door's round trip of `Decimal`'s own canonical `Display`. The doors
//! whose grammars are the lenience itself — grouping, currency, parentheses, separator
//! detection, the separated and time-bearing temporal forms, durations — are pinned by the
//! shared corpus (`tests/conformance.rs`) instead: a "naive" reference for those would be a
//! second implementation of the same grammar, not an independent reading of it.

use hypercast::{
    Date, Decimal, NumFormat, Reason, cast_bool, cast_date, cast_decimal, cast_f32, cast_f64,
    cast_i8, cast_i16, cast_i32, cast_i64, cast_u8, cast_u16, cast_u32, cast_u64, cast_uuid,
};

/// Every lenience off: a sign and ASCII digits, nothing else.
const STRICT: NumFormat = NumFormat::new('.', ',', 0);

/// The characters an edit draws from: the grammar's own bytes, the separators and markers
/// the lenient doors use, whitespace (which the doors trim only at the edges), a NUL, and
/// multi-byte UTF-8 — `é` and `€` change the byte length without changing the character
/// count, and the Arabic-Indic `٣` is a digit to `char::is_numeric` but not to ASCII.
const NOISE: [char; 18] =
    ['-', '+', ' ', '\t', '.', ',', 'e', 'x', 'g', '{', '}', '(', ')', ':', '\0', 'é', '€', '٣'];

/// Every string one character away from `base`: each character removed, and each of
/// `alphabet` substituted at and inserted before every position (and appended).
fn one_edit_neighbours(base: &str, alphabet: &[char]) -> Vec<String> {
    let chars: Vec<char> = base.chars().collect();
    let mut out = Vec::new();
    for i in 0..=chars.len() {
        if i < chars.len() {
            let mut removed = chars.clone();
            removed.remove(i);
            out.push(removed.into_iter().collect());
        }
        for &c in alphabet {
            if i < chars.len() {
                let mut replaced = chars.clone();
                replaced[i] = c;
                out.push(replaced.into_iter().collect());
            }
            let mut inserted = chars.clone();
            inserted.insert(i, c);
            out.push(inserted.into_iter().collect());
        }
    }
    out
}

/// The doors trim ASCII whitespace (space, tab, LF, FF, CR) from both ends and nothing else.
fn trim_ascii(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_ascii_whitespace())
}

/// A tiny deterministic generator, so a failure names a reproducible input.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

// ---------------------------------------------------------------------------------------
// UUID
// ---------------------------------------------------------------------------------------

fn hex_bytes(s: &str, count: usize) -> Option<Vec<u8>> {
    if s.len() != count * 2 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..count).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()).collect()
}

/// `dddddddd-dddd-dddd-dddd-dddddddddddd`: split on the hyphens, check each group's width.
fn reference_d(s: &str) -> Option<[u8; 16]> {
    let groups: Vec<&str> = s.split('-').collect();
    let widths = [4, 2, 2, 2, 6];
    if groups.len() != 5 {
        return None;
    }
    let mut out = Vec::new();
    for (group, width) in groups.iter().zip(widths) {
        out.extend(hex_bytes(group, width)?);
    }
    out.try_into().ok()
}

/// `{0xdddddddd,0xdddd,0xdddd,{0xdd,0xdd,0xdd,0xdd,0xdd,0xdd,0xdd,0xdd}}`, each component one
/// digit up to its full width.
fn reference_x(s: &str) -> Option<[u8; 16]> {
    let body = s.strip_prefix('{')?.strip_suffix("}}")?;
    let (head, tail) = body.split_once(",{")?;
    let parts: Vec<&str> = head.split(',').chain(tail.split(',')).collect();
    let widths = [4, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1];
    if parts.len() != widths.len() {
        return None;
    }
    let mut out = Vec::new();
    for (part, width) in parts.iter().zip(widths) {
        let digits = part.strip_prefix("0x").or_else(|| part.strip_prefix("0X"))?;
        if digits.is_empty()
            || digits.len() > width * 2
            || !digits.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return None;
        }
        let value = u64::from_str_radix(digits, 16).ok()?;
        out.extend_from_slice(&value.to_be_bytes()[8 - width..]);
    }
    out.try_into().ok()
}

/// Any of the five forms, after an optional case-insensitive prefix, with ASCII whitespace
/// trimmed around both.
fn reference_uuid(s: &str) -> Option<[u8; 16]> {
    let mut text = trim_ascii(s);
    for prefix in ["urn:uuid:", "guid:", "uuid:"] {
        if text.len() >= prefix.len()
            && text.is_char_boundary(prefix.len())
            && text[..prefix.len()].eq_ignore_ascii_case(prefix)
        {
            text = trim_ascii(&text[prefix.len()..]);
            break;
        }
    }
    let wrapped = |open: char, close: char| {
        text.strip_prefix(open).and_then(|t| t.strip_suffix(close)).and_then(reference_d)
    };
    reference_d(text)
        .or_else(|| hex_bytes(text, 16).and_then(|v| v.try_into().ok()))
        .or_else(|| wrapped('{', '}'))
        .or_else(|| wrapped('(', ')'))
        .or_else(|| reference_x(text))
}

fn uuid_forms(bytes: &[u8; 16]) -> Vec<String> {
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let d = format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    );
    let tail: Vec<String> = bytes[8..].iter().map(|b| format!("0x{b:02x}")).collect();
    let x =
        format!("{{0x{},0x{},0x{},{{{}}}}}", &hex[0..8], &hex[8..12], &hex[12..16], tail.join(","));
    vec![
        d.clone(),
        d.to_uppercase(),
        hex.clone(),
        format!("{{{d}}}"),
        format!("({d})"),
        x.clone(),
        x.to_uppercase(),
        format!("urn:uuid:{d}"),
        format!("GUID:{d}"),
        format!(" uuid: {hex}\t"),
    ]
}

fn uuid_samples() -> Vec<[u8; 16]> {
    let mut ids = vec![[0u8; 16], [0xFF; 16]];
    for pos in 0..16 {
        for value in [0x00, 0x09, 0x0a, 0x0f, 0x10, 0x7f, 0x80, 0x9a, 0xa9, 0xf0, 0xff] {
            let mut bytes = [0u8; 16];
            bytes[pos] = value;
            ids.push(bytes);
        }
    }
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    for _ in 0..500 {
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&rng.next().to_le_bytes());
        bytes[8..].copy_from_slice(&rng.next().to_le_bytes());
        ids.push(bytes);
    }
    ids
}

#[test]
fn every_uuid_in_every_form_casts_back_to_its_bytes() {
    for bytes in uuid_samples() {
        for text in uuid_forms(&bytes) {
            assert_eq!(cast_uuid(&text), Ok(bytes), "{text:?}");
            assert_eq!(reference_uuid(&text), Some(bytes), "the reference itself, on {text:?}");
        }
    }
}

#[test]
fn every_one_character_edit_of_a_uuid_casts_exactly_when_the_reference_says_so() {
    let mut alphabet = NOISE.to_vec();
    alphabet.extend(['0', '9', 'a', 'A', 'f', 'F', 'X', 'u', 'U']);
    let bases = [
        "00000000-0000-0000-0000-000000000000",
        "01234567-89ab-cdef-0123-456789ABCDEF",
        "0123456789abcdef0123456789abcdef",
        "{01234567-89ab-cdef-0123-456789abcdef}",
        "(01234567-89ab-cdef-0123-456789abcdef)",
        "{0x01234567,0x89ab,0xcdef,{0x01,0x23,0x45,0x67,0x89,0xab,0xcd,0xef}}",
        "{0x1,0x2,0x3,{0x4,0x5,0x6,0x7,0x8,0x9,0xa,0xb}}",
        "urn:uuid:01234567-89ab-cdef-0123-456789abcdef",
    ];
    let mut checked = 0usize;
    for base in bases {
        for text in one_edit_neighbours(base, &alphabet) {
            let got = cast_uuid(&text);
            assert_eq!(got.ok(), reference_uuid(&text), "cast_uuid({text:?})");
            if let Err(fault) = got {
                let reason =
                    if trim_ascii(&text).is_empty() { Reason::Empty } else { Reason::Malformed };
                assert_eq!(fault.reason, reason, "cast_uuid({text:?})");
            }
            checked += 1;
        }
    }
    assert!(checked > 15_000, "only {checked} edits checked");
}

// ---------------------------------------------------------------------------------------
// Boolean
// ---------------------------------------------------------------------------------------

const TRUE_WORDS: [&str; 10] =
    ["true", "t", "yes", "y", "1", "on", "enabled", "active", "checked", "in"];
const FALSE_WORDS: [&str; 10] =
    ["false", "f", "no", "n", "0", "off", "disabled", "inactive", "unchecked", "out"];

fn reference_bool(s: &str) -> Option<bool> {
    let word = trim_ascii(s).to_ascii_lowercase();
    if TRUE_WORDS.contains(&word.as_str()) {
        Some(true)
    } else if FALSE_WORDS.contains(&word.as_str()) {
        Some(false)
    } else {
        None
    }
}

#[test]
fn every_boolean_word_casts_in_any_case() {
    for (words, value) in [(TRUE_WORDS, true), (FALSE_WORDS, false)] {
        for word in words {
            let title: String = word
                .chars()
                .enumerate()
                .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                .collect();
            for text in [word.to_string(), word.to_uppercase(), title, format!(" \t{word}\r\n")] {
                assert_eq!(cast_bool(&text), Ok(value), "{text:?}");
            }
        }
    }
}

#[test]
fn every_one_character_edit_of_a_boolean_word_casts_exactly_when_the_reference_says_so() {
    let mut alphabet = NOISE.to_vec();
    alphabet.extend(['a', 'E', 'n', 'o', 's', 'T', 'u', 'y', '1', '0', 'İ', 'ſ']);
    let mut checked = 0usize;
    for base in TRUE_WORDS.iter().chain(&FALSE_WORDS).chain(&["ON", "Disabled"]) {
        for text in one_edit_neighbours(base, &alphabet) {
            assert_eq!(cast_bool(&text).ok(), reference_bool(&text), "cast_bool({text:?})");
            checked += 1;
        }
    }
    assert!(checked > 5000, "only {checked} edits checked");
}

// ---------------------------------------------------------------------------------------
// Integers
// ---------------------------------------------------------------------------------------

/// What the strict grammar reads: `Ok(value)` for a sign and ASCII digits whose value fits
/// `[min, max]`, `Err(OutOfRange)` for one that does not, `Err(Malformed)` for anything
/// else. A minus sign applies to zero on an unsigned door too: `-0` is 0.
fn reference_int(s: &str, min: i128, max: i128) -> Result<i128, Reason> {
    let text = trim_ascii(s);
    if text.is_empty() {
        return Err(Reason::Empty);
    }
    let (negative, digits) = match text.as_bytes()[0] {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Reason::Malformed);
    }
    let magnitude = digits.trim_start_matches('0');
    if magnitude.len() > 30 {
        return Err(Reason::OutOfRange);
    }
    let value = if magnitude.is_empty() { 0 } else { magnitude.parse::<i128>().unwrap() };
    let value = if negative { -value } else { value };
    if value < min || value > max { Err(Reason::OutOfRange) } else { Ok(value) }
}

/// Each door, widened to i128 so one reference serves all eight.
type Door = fn(&str, &NumFormat) -> Result<i128, Reason>;

macro_rules! doors {
    ($($door:ident => $ty:ty),+ $(,)?) => {
        [$(
            (
                stringify!($door),
                <$ty>::MIN as i128,
                <$ty>::MAX as i128,
                (|s, f| $door(s, f).map(|v| v as i128).map_err(|e| e.reason)) as Door,
            ),
        )+]
    };
}

fn integer_doors() -> [(&'static str, i128, i128, Door); 8] {
    doors! {
        cast_i8 => i8, cast_i16 => i16, cast_i32 => i32, cast_i64 => i64,
        cast_u8 => u8, cast_u16 => u16, cast_u32 => u32, cast_u64 => u64,
    }
}

#[test]
fn every_integer_formats_and_casts_back_to_itself_under_every_format() {
    let mut rng = XorShift(0xD1B5_4A32_D192_ED03);
    for (name, min, max, door) in integer_doors() {
        let mut values = vec![min, max, 0, 1, -1, min + 1, max - 1, 9, 10, 99, 100];
        for _ in 0..2000 {
            values.push(rng.next() as i128 % (max - min + 1).min(i128::from(u64::MAX)));
        }
        for value in values.into_iter().filter(|v| (min..=max).contains(v)) {
            let text = value.to_string();
            for format in [STRICT, NumFormat::INVARIANT, NumFormat::DETECT] {
                assert_eq!(door(&text, &format), Ok(value), "{name}({text:?}, {format:?})");
            }
            assert_eq!(door(&format!("+{text}"), &STRICT).is_ok(), value >= 0, "{name}(+{text})");
        }
        for out in [min - 1, max + 1] {
            assert_eq!(door(&out.to_string(), &STRICT), Err(Reason::OutOfRange), "{name}({out})");
        }
    }
}

#[test]
fn every_one_character_edit_of_an_integer_casts_exactly_when_the_reference_says_so() {
    let mut alphabet = NOISE.to_vec();
    alphabet.extend(['0', '1', '5', '9']);
    let bases = [
        "0",
        "7",
        "-128",
        "255",
        "+32767",
        "65536",
        "-2147483648",
        "4294967295",
        "9223372036854775807",
        "-9223372036854775808",
        "18446744073709551615",
        "00000000000000000000042",
    ];
    let mut checked = 0usize;
    for base in bases {
        for text in one_edit_neighbours(base, &alphabet) {
            for (name, min, max, door) in integer_doors() {
                let want = reference_int(&text, min, max);
                assert_eq!(door(&text, &STRICT), want, "{name}({text:?}) with every lenience off");
                // Lenience only ever adds readings: a plain value means the same under
                // every format.
                if let Ok(value) = want {
                    for format in [NumFormat::INVARIANT, NumFormat::DETECT] {
                        assert_eq!(door(&text, &format), Ok(value), "{name}({text:?}, {format:?})");
                    }
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 30_000, "only {checked} edits checked");
}

// ---------------------------------------------------------------------------------------
// Dates
// ---------------------------------------------------------------------------------------

fn days_in_month(year: u32, month: u32) -> u32 {
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month as usize - 1]
}

/// `yyyy-mm-dd` at fixed offsets, a real proleptic Gregorian day, year 0001 or later. The
/// wrong shape is malformed; the right shape naming a day that does not exist — year 0000,
/// month 00 or 13+, a day the month does not have — is out of range.
fn reference_date(s: &str) -> Result<Date, Reason> {
    let text = trim_ascii(s);
    if text.is_empty() {
        return Err(Reason::Empty);
    }
    let b = text.as_bytes();
    let digits_at =
        |range: std::ops::Range<usize>| range.into_iter().all(|i| b[i].is_ascii_digit());
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || !digits_at(0..4)
        || !digits_at(5..7)
        || !digits_at(8..10)
    {
        return Err(Reason::Malformed);
    }
    let (year, month, day): (u32, u32, u32) =
        (text[0..4].parse().unwrap(), text[5..7].parse().unwrap(), text[8..10].parse().unwrap());
    if year == 0 || !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return Err(Reason::OutOfRange);
    }
    Ok(Date { year: year as u16, month: month as u8, day: day as u8 })
}

#[test]
fn every_day_of_sampled_years_formats_and_casts_back_to_itself() {
    for year in [1, 4, 99, 100, 400, 1582, 1900, 1970, 2000, 2024, 2100, 9996, 9999] {
        for month in 1..=12 {
            for day in 1..=days_in_month(year, month) {
                let text = format!("{year:04}-{month:02}-{day:02}");
                let want = Date { year: year as u16, month: month as u8, day: day as u8 };
                assert_eq!(cast_date(&text), Ok(want), "{text}");
            }
            let past_the_end =
                format!("{year:04}-{month:02}-{:02}", days_in_month(year, month) + 1);
            assert_eq!(
                cast_date(&past_the_end).map_err(|f| f.reason),
                Err(Reason::OutOfRange),
                "{past_the_end}"
            );
        }
    }
}

#[test]
fn every_one_character_edit_of_a_date_casts_exactly_when_the_reference_says_so() {
    let mut alphabet = NOISE.to_vec();
    alphabet.extend(['0', '1', '2', '3', '9', '/', 'T', 'Z']);
    let bases = [
        "2024-02-29",
        "2023-02-28",
        "1900-02-28",
        "2000-02-29",
        "0001-01-01",
        "9999-12-31",
        "2026-11-30",
    ];
    let mut checked = 0usize;
    for base in bases {
        for text in one_edit_neighbours(base, &alphabet) {
            assert_eq!(
                cast_date(&text).map_err(|f| f.reason),
                reference_date(&text),
                "cast_date({text:?})"
            );
            checked += 1;
        }
    }
    assert!(checked > 3000, "only {checked} edits checked");
}

// ---------------------------------------------------------------------------------------
// Decimals
// ---------------------------------------------------------------------------------------

/// `Decimal`'s `Display` is the canonical text — no trailing fraction zeros, no negative
/// zero — so every canonical value, up to the full 96 bits and 28 places, must cast back to
/// exactly itself, and its negation to its negation.
#[test]
fn every_canonical_decimal_formats_and_casts_back_to_itself() {
    let mut rng = XorShift(0x6A09_E667_F3BC_C908);
    let mut values = vec![(0u128, 0u8), (1, 0), (1, 28), ((1 << 96) - 1, 0), ((1 << 96) - 1, 28)];
    while values.len() < 5000 {
        let bits = 1 + rng.next() % 96;
        let magnitude = (u128::from(rng.next()) << 64 | u128::from(rng.next())) >> (128 - bits);
        let scale = (rng.next() % 29) as u8;
        values.push((magnitude, scale));
    }
    let mut checked = 0usize;
    for (magnitude, scale) in values {
        if scale > 0 && magnitude.is_multiple_of(10) {
            continue; // not canonical: the door drops the trailing zero
        }
        for negative in [false, true] {
            if negative && magnitude == 0 {
                continue;
            }
            let value =
                Decimal { lo: magnitude as u64, hi: (magnitude >> 64) as u32, scale, negative };
            let text = value.to_string();
            for format in [STRICT, NumFormat::INVARIANT] {
                assert_eq!(cast_decimal(&text, &format), Ok(value), "{text} under {format:?}");
            }
            checked += 1;
        }
    }
    assert!(checked > 8000, "only {checked} decimals checked");
}

// ---------------------------------------------------------------------------------------
// Reals
// ---------------------------------------------------------------------------------------

/// Rust's `{:e}` is the shortest text that reads back to the same bits, so every finite
/// float must survive it exactly — sign of zero, subnormals and both extremes included —
/// and so must the plain `{}` form wherever that stays short.
#[test]
fn every_finite_float_formats_and_casts_back_to_the_same_bits() {
    let mut rng = XorShift(0x2545_F491_4F6C_DD1D);
    let mut doubles = vec![
        0.0,
        -0.0,
        1.0,
        -1.5,
        0.1,
        f64::MIN_POSITIVE,
        5e-324,
        f64::MAX,
        f64::MIN,
        f64::EPSILON,
    ];
    let mut singles =
        vec![0.0f32, -0.0, 1.0, 0.1, f32::MIN_POSITIVE, 1e-45, f32::MAX, f32::MIN, f32::EPSILON];
    while doubles.len() < 5000 {
        let candidate = f64::from_bits(rng.next());
        if candidate.is_finite() {
            doubles.push(candidate);
        }
    }
    while singles.len() < 5000 {
        let candidate = f32::from_bits(rng.next() as u32);
        if candidate.is_finite() {
            singles.push(candidate);
        }
    }
    for value in doubles {
        let exp = format!("{value:e}");
        assert_eq!(
            cast_f64(&exp, &NumFormat::INVARIANT).map(f64::to_bits),
            Ok(value.to_bits()),
            "{exp}"
        );
        let plain = format!("{value}");
        if plain.len() <= 40 {
            for format in [STRICT, NumFormat::INVARIANT] {
                assert_eq!(
                    cast_f64(&plain, &format).map(f64::to_bits),
                    Ok(value.to_bits()),
                    "{plain}"
                );
            }
        }
    }
    for value in singles {
        let exp = format!("{value:e}");
        assert_eq!(
            cast_f32(&exp, &NumFormat::INVARIANT).map(f32::to_bits),
            Ok(value.to_bits()),
            "{exp}"
        );
        let plain = format!("{value}");
        if plain.len() <= 40 {
            for format in [STRICT, NumFormat::INVARIANT] {
                assert_eq!(
                    cast_f32(&plain, &format).map(f32::to_bits),
                    Ok(value.to_bits()),
                    "{plain}"
                );
            }
        }
    }
}
