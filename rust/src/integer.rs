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

use crate::lane;
use crate::verdict::{trim, Fault, NumFormat};

/// The UTF-8 length of the character starting at `text[at]`, clamped to the text — so a
/// fault span covers the whole offending character without ever running past the input
/// when the text ends mid-character (arbitrary bytes are a legal input; the fuzz target
/// caught a lead byte at the last position producing a span one past the end).
///
/// An `at` past the end (no caller passes one) gives 0 rather than a bounds panic.
pub(crate) fn char_len_at(text: &[u8], at: usize) -> usize {
    text.get(at).map_or(0, |&byte| char_len(byte).min(text.len() - at))
}

/// The UTF-8 length of the character starting with `byte` (unclamped — span builders use
/// [`char_len_at`]).
fn char_len(byte: u8) -> usize {
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

    pub(crate) fn as_bytes(&self) -> &[u8] {
        // `len` is at most 4 by construction; `get` says so without a bounds check.
        self.bytes.get(..self.len).unwrap_or_default()
    }

    pub(crate) fn matches(&self, text: &[u8], at: usize) -> bool {
        text.get(at..).is_some_and(|rest| rest.starts_with(self.as_bytes()))
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
    if !format.allows(NumFormat::PARENS) || text.first() != Some(&b'(') {
        return Ok((text, start, false));
    }
    let [_, inner @ .., b')'] = text else {
        return Err(Fault::malformed(start, text.len()));
    };
    let (inner, inner_start) = trim(inner);
    if inner.is_empty() {
        return Err(Fault::malformed(start, text.len()));
    }
    Ok((inner, start + 1 + inner_start, true))
}

/// Strips the declared currency symbol when the flag permits, from either edge of the
/// (already paren-stripped) body: leading — before or after a sign — or trailing, once,
/// with ASCII whitespace between symbol and digits trimmed away. Returns the remaining
/// body, its base offset, and the sign byte consumed ahead of a leading symbol (`-$5`),
/// which the caller applies exactly as it would a sign it read itself. Shared with the
/// real and decimal doors.
pub(crate) fn strip_currency<'t>(
    body: &'t [u8],
    base: usize,
    format: &NumFormat,
) -> Result<(&'t [u8], usize, Option<u8>), Fault> {
    if !format.allows(NumFormat::CURRENCY) || format.currency.is_empty() {
        return Ok((body, base, None));
    }
    let symbol = format.currency.as_bytes();
    let (sign, unsigned) = split_sign(body);
    if let Some(after_symbol) = unsigned.strip_prefix(symbol) {
        let after = body.len() - after_symbol.len();
        let (rest, rest_start) = trim(after_symbol);
        if rest.is_empty() {
            return Err(Fault::malformed(base, body.len()));
        }
        return Ok((rest, base + after + rest_start, sign));
    }
    if let Some(before_symbol) = body.strip_suffix(symbol) {
        let (rest, rest_start) = trim(before_symbol);
        if rest.is_empty() {
            return Err(Fault::malformed(base, body.len()));
        }
        return Ok((rest, base + rest_start, None));
    }
    Ok((body, base, None))
}

/// Splits a leading `+`/`-` off `text`, returning the sign byte (if any) and the rest.
/// Shared with the real and decimal doors.
pub(crate) fn split_sign(text: &[u8]) -> (Option<u8>, &[u8]) {
    match text {
        [sign @ (b'+' | b'-'), rest @ ..] => (Some(*sign), rest),
        _ => (None, text),
    }
}

fn digit_value(byte: u8, radix: u32) -> Option<u32> {
    (byte as char).to_digit(radix)
}

/// Detects a radix prefix at the head of the trimmed token: `0x`/`&H` (hex) or `0b`
/// (binary), case-insensitive. Returns the radix and the digits after the prefix.
fn radix_prefix(text: &[u8]) -> Option<(u32, &[u8])> {
    let [first, second, digits @ ..] = text else {
        return None;
    };
    match (first, second | 0x20) {
        (b'0', b'x') | (b'&', b'h') => Some((16, digits)),
        (b'0', b'b') => Some((2, digits)),
        _ => None,
    }
}

/// Parses radix-prefixed digits as an unsigned bit pattern, then reinterprets as
/// two's complement for signed targets. A pattern wider than `bits` is `OutOfRange`.
fn parse_radix(
    text: &[u8],
    start: usize,
    radix: u32,
    digits: &[u8],
    min: i128,
    max: i128,
    bits: u32,
) -> Result<i128, Fault> {
    if digits.is_empty() {
        return Err(Fault::malformed(start, text.len()));
    }
    let mut pattern: u128 = 0;
    let mut over = false;
    for (index, &byte) in digits.iter().enumerate() {
        let Some(digit) = digit_value(byte, radix) else {
            return Err(Fault::malformed(start + 2 + index, char_len_at(digits, index)));
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
/// range check is the target's own `[min, max]` and nothing else. `lenient_lane` is `true`
/// for every door; the differential test passes `false` to get the engine's own answer.
#[inline(always)]
fn parse_int(
    input: &[u8],
    format: &NumFormat,
    min: i128,
    max: i128,
    bits: u32,
    lenient_lane: bool,
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
    let (sign, digits) = split_sign(text);
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
                if sign == Some(b'-') { -(value as i128) } else { value as i128 };
            return if signed < min || signed > max {
                Err(Fault::out_of_range(start, text.len()))
            } else {
                Ok(signed)
            };
        }
    }

    // Separator detection resolves the '.'/',' roles from structure before the engine
    // runs; a token with no separators (the fast path above included) resolves to the
    // invariant roles and nothing changes.
    let resolved;
    let format = if format.allows(NumFormat::SEPARATOR_DETECT) {
        resolved = format.resolve_detected(text, start)?;
        &resolved
    } else {
        format
    };

    if format.allows(NumFormat::RADIX_PREFIX)
        && let Some((radix, digits)) = radix_prefix(text)
    {
        return parse_radix(text, start, radix, digits, min, max, bits);
    }

    // The lenient lane (lane.rs): grouped digits, a declared currency symbol and accounting
    // parentheses, read in one pass into a u64. After the radix check on purpose — `&H12`
    // is hex even when `&H` is also the declared currency symbol. Anything the lane does
    // not read falls through to the engine below, which owns every fault.
    if lenient_lane {
        let mut sink = lane::Integer::default();
        if let Some(negative) = lane::scan(text, format, false, &mut sink) {
            let value = if negative { -(sink.value as i128) } else { sink.value as i128 };
            return if value < min || value > max {
                Err(Fault::out_of_range(start, text.len()))
            } else {
                Ok(value)
            };
        }
    }

    let (body, base, parens) = strip_parens(text, start, format)?;
    let (body, base, pre_sign) = strip_currency(body, base, format)?;

    let mut i = 0;
    let mut negative = parens;
    let (sign, _) = split_sign(body);
    if let Some(pre_sign) = pre_sign {
        // The sign sat ahead of a leading currency symbol (`-$5`); a second one after it,
        // or one inside accounting parens, is double negation nonsense.
        if parens || sign.is_some() {
            return Err(Fault::malformed(base, 1));
        }
        negative = pre_sign == b'-';
    } else if let Some(sign) = sign {
        if parens {
            // A sign inside accounting parens is double negation nonsense.
            return Err(Fault::malformed(base, 1));
        }
        negative = sign == b'-';
        i = 1;
    }

    let decimal = Sep::new(format.decimal_sep);
    let group = Sep::new(format.group_sep);
    let mut acc: i128 = 0;
    let mut over = false;
    let mut any_digit = false;
    let mut exp: u32 = 0;
    let mut exp_over = false;
    while let Some(&byte) = body.get(i) {
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
            let between_digits = is_digit_at(body, i.wrapping_sub(1)) && is_digit_at(body, after);
            if !between_digits {
                return Err(Fault::malformed(base + i, group.len));
            }
            i = after;
        } else if (byte == b'e' || byte == b'E') && format.allows(NumFormat::EXPONENT) && any_digit
        {
            let e_pos = i;
            i += 1;
            if body.get(i) == Some(&b'+') {
                i += 1;
            }
            if body.get(i) == Some(&b'-') {
                // A negative exponent demands a fraction — never integral.
                return Err(Fault::malformed(base + i, 1));
            }
            if !is_digit_at(body, i) {
                return Err(Fault::malformed(base + e_pos, 1));
            }
            while let Some(&digit @ b'0'..=b'9') = body.get(i) {
                exp = match exp
                    .checked_mul(10)
                    .and_then(|shifted| shifted.checked_add((digit - b'0') as u32))
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
                return Err(Fault::malformed(base + i, char_len_at(body, i)));
            }
        } else {
            return Err(Fault::malformed(base + i, char_len_at(body, i)));
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

/// Whether `text` has an ASCII digit at `at`; false past either end (an `at` built by
/// `wrapping_sub` from 0 is past the far end).
pub(crate) fn is_digit_at(text: &[u8], at: usize) -> bool {
    text.get(at).is_some_and(u8::is_ascii_digit)
}

macro_rules! integer_doors {
    ($($(#[$doc:meta])* $door:ident => $ty:ty),+ $(,)?) => {$(
        $(#[$doc])*
        pub fn $door(input: impl AsRef<[u8]>, format: &NumFormat) -> Result<$ty, Fault> {
            let input = input.as_ref();
            parse_int(input, format, <$ty>::MIN as i128, <$ty>::MAX as i128, <$ty>::BITS, true)
                .map(|value| value as $ty)
        }
    )+};
}

/// The engine's answer with the lenient lane switched off — what `lib.rs`'s differential
/// test compares every integer door against.
#[cfg(test)]
pub(crate) fn engine_only<T: TryFrom<i128>>(
    input: &[u8],
    format: &NumFormat,
    min: i128,
    max: i128,
    bits: u32,
) -> Result<T, Fault> {
    parse_int(input, format, min, max, bits, false)
        .map(|value| T::try_from(value).ok().expect("the engine range-checked this"))
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
