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
//!
//! This module also holds the framing every numeric door shares — the percent, the
//! parentheses, the currency symbol and the signs around the digits ([`frame`]) — and the
//! digit reader that knows a declared native set ([`read_digit`]).

use crate::lane;
use crate::verdict::{Fault, NumFormat, trim};

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

/// The digit at `text[at..]`, as its value and byte length: an ASCII digit, or one of the
/// declared native digits. A digit is a digit before it is anything else, so every engine
/// loop asks this first.
#[inline]
pub(crate) fn read_digit(text: &[u8], at: usize, format: &NumFormat) -> Option<(u8, usize)> {
    let &byte = text.get(at)?;
    let digit = byte.wrapping_sub(b'0');
    if digit <= 9 {
        return Some((digit, 1));
    }
    if byte < 0x80 { None } else { format.digits.read(text, at) }
}

/// What surrounds the digits of a numeric token, in the engine's order of decisions: a
/// trailing percent (the real and decimal doors), accounting parentheses, the declared
/// currency symbol at either edge, and the sign — ahead of the digits, before or after a
/// leading symbol, or (with [`NumFormat::TRAILING_SIGN`]) after them, after a trailing
/// symbol included. What is left is the digits with their separators and an exponent,
/// which each door reads its own way.
pub(crate) struct Frame<'t> {
    /// The digits, separators and exponent, nothing else.
    pub(crate) body: &'t [u8],
    /// Where `body` starts in the caller's input.
    pub(crate) base: usize,
    pub(crate) negative: bool,
    /// A percent symbol was taken off the end.
    pub(crate) percent: bool,
    /// The declared currency symbol was found, so the currency separators apply.
    pub(crate) currency: bool,
}

/// Reads the frame of the trimmed, non-empty `text` at offset `start` under `format`.
/// `percent_door` is whether a trailing percent is this door's to read.
pub(crate) fn frame<'t>(
    text: &'t [u8],
    start: usize,
    format: &NumFormat,
    percent_door: bool,
) -> Result<Frame<'t>, Fault> {
    // Percent strips first, exactly as Svartalfheim checked `trimmed[^1]` first — the parens
    // and sign live inside the percent body: `(2.5)%` is -0.025. A trailing sign may follow
    // the percent (`2.5%-`); it is read here and applied below like any other.
    let mut post_sign: Option<TrailingSign> = None;
    let (body, percent) = match strip_percent(text, format, percent_door) {
        Some(rest) => (rest.trim_ascii_end(), true),
        None => match strip_signed_percent(text, start, format, percent_door) {
            Some((rest, sign)) => {
                post_sign = Some(sign);
                (rest.trim_ascii_end(), true)
            }
            None => (text, false),
        },
    };
    if body.is_empty() {
        return Err(Fault::malformed(start, text.len()));
    }
    let (body, base, parens) = strip_parens(body, start, format)?;
    let symbol = strip_currency(body, base, format)?;
    let Currency { mut body, mut base, pre_sign, post_sign: after_symbol, found: currency } =
        symbol;
    if let Some((_, at, len)) = after_symbol {
        // A sign after the symbol and another after the percent: the inner one is the fault.
        if post_sign.is_some() {
            return Err(Fault::malformed(at, len));
        }
        post_sign = after_symbol;
    }

    let mut negative = parens;
    let mut signed = parens;
    let leading = read_sign(body, format);
    if let Some(pre) = pre_sign {
        // The sign sat ahead of a leading currency symbol (`-$5`); a second one after it,
        // or one inside accounting parens, is double negation nonsense.
        if parens || leading.is_some() {
            return Err(Fault::malformed(base, leading.map_or(1, |(_, len)| len)));
        }
        negative = pre;
        signed = true;
    } else if let Some((minus, len)) = leading {
        if parens {
            // A sign inside accounting parens is double negation nonsense.
            return Err(Fault::malformed(base, len));
        }
        negative = minus;
        signed = true;
        body = body.get(len..).unwrap_or_default();
        base += len;
    }
    if format.allows(NumFormat::TRAILING_SIGN)
        && let Some((minus, len)) = read_trailing_sign(body, format)
    {
        let at = body.len() - len;
        if signed {
            return Err(Fault::malformed(base + at, len));
        }
        negative = minus;
        signed = true;
        body = body.get(..at).unwrap_or_default().trim_ascii_end();
    }
    if let Some((minus, at, len)) = post_sign {
        if signed {
            return Err(Fault::malformed(at, len));
        }
        negative = minus;
    }
    Ok(Frame { body, base, negative, percent, currency })
}

/// The text less its trailing percent symbol — ASCII `%` or the declared one — when the
/// door reads one and the format permits it.
#[inline]
fn strip_percent<'t>(text: &'t [u8], format: &NumFormat, percent_door: bool) -> Option<&'t [u8]> {
    if !percent_door || !format.allows(NumFormat::PERCENT) {
        return None;
    }
    if let Some(rest) = text.strip_suffix(b"%") {
        return Some(rest);
    }
    if format.percent.ends(text) {
        return text.get(..text.len() - format.percent.len());
    }
    None
}

/// A trailing sign as `frame` records one: negative, offset in the caller's input, length.
type TrailingSign = (bool, usize, usize);

/// `2.5%-`: the trailing sign after the percent, on a door that reads the percent and
/// under a format that reads a trailing sign. The body ahead of the percent, and the sign
/// as `frame` records a trailing one (negative, offset, length).
fn strip_signed_percent<'t>(
    text: &'t [u8],
    start: usize,
    format: &NumFormat,
    percent_door: bool,
) -> Option<(&'t [u8], TrailingSign)> {
    if !(percent_door
        && format.allows(NumFormat::PERCENT)
        && format.allows(NumFormat::TRAILING_SIGN))
    {
        return None;
    }
    let (minus, len) = read_trailing_sign(text, format)?;
    let ahead = text.get(..text.len() - len)?;
    let rest = strip_percent(ahead, format, true)?;
    Some((rest, (minus, start + text.len() - len, len)))
}

/// Strips accounting parentheses when the flag permits, returning the re-trimmed body, its
/// base offset in the caller's input, and whether the parens declared negation.
fn strip_parens<'t>(
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

/// What [`strip_currency`] leaves: the body less the symbol, and the signs it read on the
/// symbol's far side — ahead of a leading symbol (`-$5`), or after a trailing one
/// (`5 €-`, with where it sits in the caller's input and its length).
struct Currency<'t> {
    body: &'t [u8],
    base: usize,
    pre_sign: Option<bool>,
    post_sign: Option<(bool, usize, usize)>,
    found: bool,
}

/// Strips the declared currency symbol when the flag permits, from either edge of the
/// (already paren-stripped) body: leading — before or after a sign — or trailing, once,
/// with ASCII whitespace between symbol and digits trimmed away. The sign consumed ahead
/// of a leading symbol, and a trailing sign after a trailing symbol, are reported for
/// [`frame`] to apply exactly as it would a sign it read itself.
fn strip_currency<'t>(
    body: &'t [u8],
    base: usize,
    format: &NumFormat,
) -> Result<Currency<'t>, Fault> {
    let untouched = Currency { body, base, pre_sign: None, post_sign: None, found: false };
    if !format.allows(NumFormat::CURRENCY) || format.currency.is_empty() {
        return Ok(untouched);
    }
    let symbol = format.currency.as_bytes();
    let (sign, sign_len) = match read_sign(body, format) {
        Some((minus, len)) => (Some(minus), len),
        None => (None, 0),
    };
    if let Some(after_symbol) = body.get(sign_len..).and_then(|rest| rest.strip_prefix(symbol)) {
        let after = body.len() - after_symbol.len();
        let (rest, rest_start) = trim(after_symbol);
        if rest.is_empty() {
            return Err(Fault::malformed(base, body.len()));
        }
        let base = base + after + rest_start;
        return Ok(Currency { body: rest, base, pre_sign: sign, post_sign: None, found: true });
    }
    // A trailing symbol may be followed by a trailing sign (`5 €-`): look for the symbol
    // just ahead of one.
    let trailing = if format.allows(NumFormat::TRAILING_SIGN) {
        read_trailing_sign(body, format).filter(|&(_, len)| len < body.len())
    } else {
        None
    };
    let ahead = match trailing {
        Some((_, len)) => body.get(..body.len() - len).unwrap_or_default(),
        None => body,
    };
    let Some(before_symbol) = ahead.strip_suffix(symbol) else {
        return Ok(untouched);
    };
    let (rest, rest_start) = trim(before_symbol);
    if rest.is_empty() {
        return Err(Fault::malformed(base, body.len()));
    }
    let post_sign = trailing.map(|(minus, len)| (minus, base + body.len() - len, len));
    Ok(Currency { body: rest, base: base + rest_start, pre_sign: None, post_sign, found: true })
}

/// The sign at the head of `text`: ASCII `+`/`-` first, then the declared spellings.
/// `Some((negative, byte length))`.
#[inline]
pub(crate) fn read_sign(text: &[u8], format: &NumFormat) -> Option<(bool, usize)> {
    match text.first() {
        Some(b'-') => Some((true, 1)),
        Some(b'+') => Some((false, 1)),
        Some(_) if format.minus_sign.matches(text, 0) => Some((true, format.minus_sign.len())),
        Some(_) if format.plus_sign.matches(text, 0) => Some((false, format.plus_sign.len())),
        _ => None,
    }
}

/// The sign at the end of `text`, in the same spellings. `Some((negative, byte length))`.
#[inline]
fn read_trailing_sign(text: &[u8], format: &NumFormat) -> Option<(bool, usize)> {
    match text.last() {
        Some(b'-') => Some((true, 1)),
        Some(b'+') => Some((false, 1)),
        Some(_) if format.minus_sign.ends(text) => Some((true, format.minus_sign.len())),
        Some(_) if format.plus_sign.ends(text) => Some((false, format.plus_sign.len())),
        _ => None,
    }
}

/// Splits a leading ASCII `+`/`-` off `text`, returning the sign byte (if any) and the
/// rest — the plain shapes' own sign, which a declared spelling never takes part in.
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
            let signed = if sign == Some(b'-') { -(value as i128) } else { value as i128 };
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

    let Frame { body, base, negative, currency, .. } = frame(text, start, format, false)?;
    let (decimal, group) = format.separators(currency);

    let mut i = 0;
    let mut acc: i128 = 0;
    let mut over = false;
    let mut any_digit = false;
    // Whether the byte before `i` was a digit — what a group separator must sit after.
    let mut after_digit = false;
    let mut exp: u32 = 0;
    let mut exp_over = false;
    while let Some(&byte) = body.get(i) {
        if let Some((digit, len)) = read_digit(body, i, format) {
            any_digit = true;
            after_digit = true;
            acc = match acc.checked_mul(10).and_then(|shifted| shifted.checked_add(digit as i128)) {
                Some(next) => next,
                None => {
                    over = true;
                    acc
                }
            };
            i += len;
        } else if decimal.matches(body, i) {
            // A decimal point is never accepted on an integer.
            return Err(Fault::malformed(base + i, decimal.len()));
        } else if format.allows(NumFormat::GROUPING) && group.matches(body, i) {
            let after = i + group.len();
            if !after_digit || read_digit(body, after, format).is_none() {
                return Err(Fault::malformed(base + i, group.len()));
            }
            after_digit = false;
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
            if read_digit(body, i, format).is_none() {
                return Err(Fault::malformed(base + e_pos, 1));
            }
            while let Some((digit, len)) = read_digit(body, i, format) {
                exp =
                    match exp.checked_mul(10).and_then(|shifted| shifted.checked_add(digit as u32))
                    {
                        Some(next) if next <= 100_000 => next,
                        _ => {
                            exp_over = true;
                            exp
                        }
                    };
                i += len;
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
