//! The integer doors (i8–i64, u8–u64). Extends bare digit parsing with the notations
//! untrusted sources actually send — Svartalfheim's `IntegerParser`, re-hosted on a
//! caller-declared [`NumFormat`] instead of `IFormatProvider`: declared digit grouping,
//! accounting parentheses, exponent form, and the culture-insensitive `0x`/`&H`/`0b`
//! radix prefixes.
//!
//! Range is the target type's own — `"256"` for a u8 is `OutOfRange` for free. A decimal
//! point is never accepted on an integer, so `1e3` casts to 1000 but `1.5e3` (and any
//! negative exponent) is `Malformed`. Hex and binary are read as the two's-complement bit
//! pattern, so `0xFF` is -1 for an i8; the pattern must fit the target's width.

use crate::verdict::{trim, Fault, NumFormat};

/// The UTF-8 length of the character starting with `byte` — so a fault span covers the
/// whole offending character, not just its leading byte.
pub(crate) fn char_len(byte: u8) -> usize {
    match byte {
        b if b >= 0xF0 => 4,
        b if b >= 0xE0 => 3,
        b if b >= 0xC0 => 2,
        _ => 1,
    }
}

/// A declared separator pre-encoded to UTF-8, matchable at a byte position.
pub(crate) struct Sep {
    bytes: [u8; 4],
    pub(crate) len: usize,
}

impl Sep {
    pub(crate) fn new(sep: char) -> Sep {
        let mut bytes = [0u8; 4];
        let len = sep.encode_utf8(&mut bytes).len();
        Sep { bytes, len }
    }

    pub(crate) fn matches(&self, text: &[u8], at: usize) -> bool {
        text[at..].starts_with(&self.bytes[..self.len])
    }
}

/// Strips accounting parentheses when the flag permits, returning the re-trimmed body, its
/// base offset in the caller's input, and whether the parens declared negation. Shared with
/// the real doors.
pub(crate) fn strip_parens<'t>(
    text: &'t [u8],
    start: usize,
    format: &NumFormat,
) -> Result<(&'t [u8], usize, bool), Fault> {
    if !format.allows(NumFormat::PARENS) || text[0] != b'(' {
        return Ok((text, start, false));
    }
    if text.len() < 3 || *text.last().unwrap() != b')' {
        return Err(Fault::malformed(start, text.len()));
    }
    let (inner, inner_start) = trim(&text[1..text.len() - 1]);
    if inner.is_empty() {
        return Err(Fault::malformed(start, text.len()));
    }
    Ok((inner, start + 1 + inner_start, true))
}

fn digit_value(byte: u8, radix: u32) -> Option<u32> {
    (byte as char).to_digit(radix)
}

/// Detects a radix prefix at the head of the trimmed token: `0x`/`&H` (hex) or `0b`
/// (binary), case-insensitive.
fn radix_prefix(text: &[u8]) -> Option<u32> {
    if text.len() < 2 {
        return None;
    }
    let second = text[1] | 0x20;
    if (text[0] == b'0' && second == b'x') || (text[0] == b'&' && second == b'h') {
        return Some(16);
    }
    if text[0] == b'0' && second == b'b' {
        return Some(2);
    }
    None
}

/// Parses radix-prefixed digits as an unsigned bit pattern, then reinterprets as
/// two's complement for signed targets. A pattern wider than `bits` is `OutOfRange`.
fn parse_radix(
    text: &[u8],
    start: usize,
    radix: u32,
    min: i128,
    max: i128,
    bits: u32,
) -> Result<i128, Fault> {
    let digits = &text[2..];
    if digits.is_empty() {
        return Err(Fault::malformed(start, text.len()));
    }
    let mut pattern: u128 = 0;
    let mut over = false;
    for (index, &byte) in digits.iter().enumerate() {
        let Some(digit) = digit_value(byte, radix) else {
            return Err(Fault::malformed(start + 2 + index, char_len(byte)));
        };
        pattern = match pattern
            .checked_mul(radix as u128)
            .and_then(|shifted| shifted.checked_add(digit as u128))
        {
            Some(next) => next,
            None => {
                over = true;
                0
            }
        };
    }
    let mask: u128 = (1u128 << bits) - 1;
    if over || pattern > mask {
        return Err(Fault::out_of_range(start, text.len()));
    }
    let value = if min < 0 && pattern > max as u128 {
        pattern as i128 - (1i128 << bits)
    } else {
        pattern as i128
    };
    Ok(value)
}

/// The shared decimal engine: every width funnels through one i128 accumulator, so the
/// range check is the target's own `[min, max]` and nothing else.
fn parse_int(
    input: &[u8],
    format: &NumFormat,
    min: i128,
    max: i128,
    bits: u32,
) -> Result<i128, Fault> {
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }

    // Fast path: `[+|-]digits`, the overwhelmingly common shape. 19 digits can't overflow
    // a u64 accumulator, and a pure digit run can't be a radix prefix, grouping, or an
    // exponent — the first byte that breaks the shape falls through to the full engine,
    // which rescans from the top and owns every fault span. Verdicts are identical either
    // way; only plain input skips the lenience tax.
    let digits_at = usize::from(matches!(text[0], b'+' | b'-'));
    let digits = &text[digits_at..];
    if !digits.is_empty() && digits.len() <= 19 {
        let mut value: u64 = 0;
        let mut plain = true;
        for &byte in digits {
            let digit = byte.wrapping_sub(b'0');
            if digit > 9 {
                plain = false;
                break;
            }
            value = value * 10 + u64::from(digit);
        }
        if plain {
            let signed =
                if text[0] == b'-' { -(value as i128) } else { value as i128 };
            return if signed < min || signed > max {
                Err(Fault::out_of_range(start, text.len()))
            } else {
                Ok(signed)
            };
        }
    }

    if format.allows(NumFormat::RADIX_PREFIX)
        && let Some(radix) = radix_prefix(text)
    {
        return parse_radix(text, start, radix, min, max, bits);
    }

    let (body, base, parens) = strip_parens(text, start, format)?;

    let mut i = 0;
    let mut negative = parens;
    if body[0] == b'+' || body[0] == b'-' {
        if parens {
            // A sign inside accounting parens is double negation nonsense.
            return Err(Fault::malformed(base, 1));
        }
        negative = body[0] == b'-';
        i = 1;
    }

    let decimal = Sep::new(format.decimal_sep);
    let group = Sep::new(format.group_sep);
    let mut acc: i128 = 0;
    let mut over = false;
    let mut any_digit = false;
    let mut exp: u32 = 0;
    let mut exp_over = false;
    while i < body.len() {
        let byte = body[i];
        if byte.is_ascii_digit() {
            any_digit = true;
            acc = match acc
                .checked_mul(10)
                .and_then(|shifted| shifted.checked_add((byte - b'0') as i128))
            {
                Some(next) => next,
                None => {
                    over = true;
                    acc
                }
            };
            i += 1;
        } else if decimal.matches(body, i) {
            // A decimal point is never accepted on an integer.
            return Err(Fault::malformed(base + i, decimal.len));
        } else if format.allows(NumFormat::GROUPING) && group.matches(body, i) {
            let after = i + group.len;
            let between_digits = i > 0
                && body[i - 1].is_ascii_digit()
                && after < body.len()
                && body[after].is_ascii_digit();
            if !between_digits {
                return Err(Fault::malformed(base + i, group.len));
            }
            i = after;
        } else if (byte == b'e' || byte == b'E') && format.allows(NumFormat::EXPONENT) && any_digit
        {
            let e_pos = i;
            i += 1;
            if i < body.len() && body[i] == b'+' {
                i += 1;
            }
            if i < body.len() && body[i] == b'-' {
                // A negative exponent demands a fraction — never integral.
                return Err(Fault::malformed(base + i, 1));
            }
            if i >= body.len() || !body[i].is_ascii_digit() {
                return Err(Fault::malformed(base + e_pos, 1));
            }
            while i < body.len() && body[i].is_ascii_digit() {
                exp = match exp
                    .checked_mul(10)
                    .and_then(|shifted| shifted.checked_add((body[i] - b'0') as u32))
                {
                    Some(next) if next <= 100_000 => next,
                    _ => {
                        exp_over = true;
                        exp
                    }
                };
                i += 1;
            }
            if i != body.len() {
                return Err(Fault::malformed(base + i, char_len(body[i])));
            }
        } else {
            return Err(Fault::malformed(base + i, char_len(byte)));
        }
    }

    if !any_digit {
        return Err(Fault::malformed(start, text.len()));
    }
    if over || (exp_over && acc != 0) {
        return Err(Fault::out_of_range(start, text.len()));
    }
    let mut value = acc;
    if exp > 0 && acc != 0 {
        value = match 10i128.checked_pow(exp).and_then(|scale| acc.checked_mul(scale)) {
            Some(scaled) => scaled,
            None => return Err(Fault::out_of_range(start, text.len())),
        };
    }
    if negative {
        value = -value;
    }
    if value < min || value > max {
        return Err(Fault::out_of_range(start, text.len()));
    }
    Ok(value)
}

macro_rules! integer_doors {
    ($($(#[$doc:meta])* $door:ident => $ty:ty),+ $(,)?) => {$(
        $(#[$doc])*
        pub fn $door(input: impl AsRef<[u8]>, format: &NumFormat) -> Result<$ty, Fault> {
            let input = input.as_ref();
            parse_int(input, format, <$ty>::MIN as i128, <$ty>::MAX as i128, <$ty>::BITS)
                .map(|value| value as $ty)
        }
    )+};
}

integer_doors! {
    /// Casts integer text to i8. Empty ⇒ `Empty`; unrecognized ⇒ `Malformed`; outside
    /// `i8::MIN..=i8::MAX` ⇒ `OutOfRange`.
    cast_i8 => i8,
    /// Casts integer text to i16.
    cast_i16 => i16,
    /// Casts integer text to i32.
    cast_i32 => i32,
    /// Casts integer text to i64.
    cast_i64 => i64,
    /// Casts integer text to u8.
    cast_u8 => u8,
    /// Casts integer text to u16.
    cast_u16 => u16,
    /// Casts integer text to u32.
    cast_u32 => u32,
    /// Casts integer text to u64.
    cast_u64 => u64,
}
