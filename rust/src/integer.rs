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

use crate::detail::Failure;
use crate::lane;
use crate::site::Site;
use crate::text::{skip_white_space, skip_white_space_back};
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
/// White space of any kind and amount around each of those is no part of the numeral and
/// is ignored; inside the numeral only a declared group separator may be white space.
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
) -> Result<Frame<'t>, Failure> {
    // Percent strips first, exactly as Svartalfheim checked `trimmed[^1]` first — the parens
    // and sign live inside the percent body: `(2.5)%` is -0.025. A trailing sign may follow
    // the percent (`2.5%-`); it is read here and applied below like any other.
    let mut post_sign: Option<TrailingSign> = None;
    let (body, percent) = match strip_percent(text, format, percent_door) {
        Some(rest) => (trim_end(rest), true),
        None => match strip_signed_percent(text, start, format, percent_door) {
            Some((rest, sign)) => {
                post_sign = Some(sign);
                (trim_end(rest), true)
            }
            None => (text, false),
        },
    };
    if body.is_empty() {
        return Err(Failure::at(Site::FramePercentOnly, start, text.len()));
    }
    let (body, base, parens) = strip_parens(body, start, format)?;
    let symbol = strip_currency(body, base, format)?;
    let Currency { mut body, mut base, pre_sign, post_sign: after_symbol, found: currency } =
        symbol;
    if let Some((_, at, len)) = after_symbol {
        // A sign after the symbol and another after the percent: the inner one is the fault.
        if post_sign.is_some() {
            return Err(Failure::at(Site::FrameSignAfterSymbolAndPercent, at, len));
        }
        post_sign = after_symbol;
    }

    let mut negative = parens;
    let mut signed = parens;
    let leading = read_sign(body, format);
    if let Some((pre, pre_at, pre_len)) = pre_sign {
        // The sign sat ahead of a leading currency symbol (`-$5`); a second one after it,
        // or one inside accounting parens, is double negation nonsense. The fault is the
        // second sign, or the one inside the parens.
        if let Some((_, len)) = leading {
            return Err(Failure::at(Site::FrameSecondSignAfterSymbol, base, len));
        }
        if parens {
            return Err(Failure::at(Site::FrameSignBeforeSymbolInParens, pre_at, pre_len));
        }
        negative = pre;
        signed = true;
    } else if let Some((minus, len)) = leading {
        if parens {
            // A sign inside accounting parens is double negation nonsense.
            return Err(Failure::at(Site::FrameSignInParens, base, len));
        }
        negative = minus;
        signed = true;
        let rest = body.get(len..).unwrap_or_default();
        let gap = skip_white_space(rest);
        body = rest.get(gap..).unwrap_or_default();
        base += len + gap;
    }
    if format.allows(NumFormat::TRAILING_SIGN)
        && let Some((minus, len)) = read_trailing_sign(body, format)
    {
        let at = body.len() - len;
        if signed {
            return Err(Failure::at(Site::FrameTrailingSignTwice, base + at, len));
        }
        negative = minus;
        signed = true;
        body = trim_end(body.get(..at).unwrap_or_default());
    }
    if let Some((minus, at, len)) = post_sign {
        if signed {
            return Err(Failure::at(Site::FramePostSignTwice, at, len));
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

/// `text` less the white space that ends it.
#[inline]
fn trim_end(text: &[u8]) -> &[u8] {
    text.get(..text.len().saturating_sub(skip_white_space_back(text))).unwrap_or_default()
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
    let ahead = trim_end(text.get(..text.len() - len)?);
    let rest = strip_percent(ahead, format, true)?;
    Some((rest, (minus, start + text.len() - len, len)))
}

/// Strips accounting parentheses when the flag permits, returning the re-trimmed body, its
/// base offset in the caller's input, and whether the parens declared negation.
fn strip_parens<'t>(
    text: &'t [u8],
    start: usize,
    format: &NumFormat,
) -> Result<(&'t [u8], usize, bool), Failure> {
    if !format.allows(NumFormat::PARENS) || text.first() != Some(&b'(') {
        return Ok((text, start, false));
    }
    let [_, inner @ .., b')'] = text else {
        return Err(Failure::at(Site::FrameParensUnclosed, start, text.len()));
    };
    let (inner, inner_start) = trim(inner);
    if inner.is_empty() {
        return Err(Failure::at(Site::FrameParensEmpty, start, text.len()));
    }
    Ok((inner, start + 1 + inner_start, true))
}

/// What [`strip_currency`] leaves: the body less the symbol, and the signs it read on the
/// symbol's far side — ahead of a leading symbol (`-$5`), or after a trailing one
/// (`5 €-`), each with where it sits in the caller's input and its length.
struct Currency<'t> {
    body: &'t [u8],
    base: usize,
    pre_sign: Option<(bool, usize, usize)>,
    post_sign: Option<(bool, usize, usize)>,
    found: bool,
}

/// Strips the declared currency symbol when the flag permits, from either edge of the
/// (already paren-stripped) body: leading — before or after a sign — or trailing, once,
/// with white space between symbol, sign and digits trimmed away. The symbol is matched as
/// [`CurrencySymbol`](crate::CurrencySymbol) says, so its length in the input can differ
/// from the declaration's; every offset here is measured in the input. The sign consumed ahead
/// of a leading symbol, and a trailing sign after a trailing symbol, are reported for
/// [`frame`] to apply exactly as it would a sign it read itself.
fn strip_currency<'t>(
    body: &'t [u8],
    base: usize,
    format: &NumFormat,
) -> Result<Currency<'t>, Failure> {
    let untouched = Currency { body, base, pre_sign: None, post_sign: None, found: false };
    if !format.allows(NumFormat::CURRENCY) || format.currency.is_empty() {
        return Ok(untouched);
    }
    let symbol = &format.currency;
    let (sign, sign_len) = match read_sign(body, format) {
        Some((minus, len)) => {
            (Some((minus, base, len)), len + body.get(len..).map_or(0, skip_white_space))
        }
        None => (None, 0),
    };
    if let Some(after_symbol) = body.get(sign_len..).and_then(|rest| symbol.strip_leading(rest)) {
        let after = body.len() - after_symbol.len();
        let (rest, rest_start) = trim(after_symbol);
        if rest.is_empty() {
            return Err(Failure::at(Site::FrameCurrencyOnlyLeading, base, body.len()));
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
        Some((_, len)) => trim_end(body.get(..body.len() - len).unwrap_or_default()),
        None => body,
    };
    let Some(before_symbol) = symbol.strip_trailing(ahead) else {
        return Ok(untouched);
    };
    let (rest, rest_start) = trim(before_symbol);
    if rest.is_empty() {
        return Err(Failure::at(Site::FrameCurrencyOnlyTrailing, base, body.len()));
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
) -> Result<i128, Failure> {
    if digits.is_empty() {
        return Err(Failure::at(Site::RadixNoDigits, start, text.len()));
    }
    let mut pattern: u128 = 0;
    let mut over = false;
    for (index, &byte) in digits.iter().enumerate() {
        let Some(digit) = digit_value(byte, radix) else {
            return Err(Failure::at(
                Site::RadixDigitExpected,
                start + 2 + index,
                char_len_at(digits, index),
            ));
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
        return Err(Failure::at(Site::RadixTooLarge, start, text.len()));
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
) -> Result<i128, Failure> {
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Failure::spanless(Site::IntBlank));
    }
    let out_of_range = |negative: bool, sites: [Site; 3]| {
        range_failure(negative, min == 0, sites, start, text.len())
    };

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
                Err(out_of_range(signed < 0, FAST_RANGE))
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
                Err(out_of_range(value < 0, LANE_RANGE))
            } else {
                Ok(value)
            };
        }
    }

    let Frame { body, base, negative, currency, .. } = frame(text, start, format, false)?;
    let (decimal, _) = format.separators(currency);
    let mut groups = Groups::new(format, base, Site::IntGroupSize);

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
            groups.digit(i);
            acc = match acc.checked_mul(10).and_then(|shifted| shifted.checked_add(digit as i128)) {
                Some(next) => next,
                None => {
                    over = true;
                    acc
                }
            };
            i += len;
        } else if decimal.matches(body, i) {
            groups.end(i)?;
            // A decimal point is never accepted on an integer.
            return Err(Failure::at(Site::IntDecimalPoint, base + i, decimal.len()));
        } else if let width @ 1.. = grouping(format, body, i, currency) {
            let after = i + width;
            // The group this separator closes comes first in reading order.
            if after_digit {
                groups.separator(i)?;
            }
            if !after_digit || read_digit(body, after, format).is_none() {
                return Err(Failure::at(Site::IntGroupMisplaced, base + i, width));
            }
            after_digit = false;
            i = after;
        } else if (byte == b'e' || byte == b'E') && format.allows(NumFormat::EXPONENT) && any_digit
        {
            groups.end(i)?;
            let e_pos = i;
            i += 1;
            if body.get(i) == Some(&b'+') {
                i += 1;
            }
            if body.get(i) == Some(&b'-') {
                // A negative exponent demands a fraction — never integral.
                return Err(Failure::at(Site::IntNegativeExponent, base + i, 1));
            }
            if read_digit(body, i, format).is_none() {
                return Err(Failure::at(Site::IntExponentDigitExpected, base + e_pos, 1));
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
                if let width @ 1.. = grouping(format, body, i, currency)
                    && Groups::strict(format)
                {
                    return Err(Failure::at(Site::IntGroupInExponent, base + i, width));
                }
                let site = stray(
                    body,
                    i,
                    format,
                    [Site::IntAfterExponentSign, Site::IntAfterExponent, Site::IntAfterExponent],
                );
                return Err(Failure::at(site, base + i, char_len_at(body, i)));
            }
        } else {
            groups.end(i)?;
            let site = stray(
                body,
                i,
                format,
                [Site::IntSignMisplaced, Site::IntNotFinite, Site::IntUnexpected],
            );
            return Err(Failure::at(site, base + i, char_len_at(body, i)));
        }
    }

    groups.end(body.len())?;
    if !any_digit {
        return Err(Failure::at(Site::IntNoDigits, start, text.len()));
    }
    if over || (exp_over && acc != 0) {
        return Err(out_of_range(
            negative,
            [Site::IntOverflowNegative, Site::IntOverflowTooLarge, Site::IntOverflowTooSmall],
        ));
    }
    let mut value = acc;
    if exp > 0 && acc != 0 {
        value = match 10i128.checked_pow(exp).and_then(|scale| acc.checked_mul(scale)) {
            Some(scaled) => scaled,
            None => {
                return Err(out_of_range(
                    negative,
                    [Site::IntScaledNegative, Site::IntScaledTooLarge, Site::IntScaledTooSmall],
                ));
            }
        };
    }
    if negative {
        value = -value;
    }
    if value < min || value > max {
        return Err(out_of_range(
            value < 0,
            [Site::IntRangeNegative, Site::IntRangeTooLarge, Site::IntRangeTooSmall],
        ));
    }
    Ok(value)
}

/// The failure for a value outside the door's range, spanning the whole token: which of
/// `[below_zero, above, below]` by its sign and whether the door is unsigned. Out of line, so
/// that no path that succeeds sets anything up for it.
#[cold]
#[inline(never)]
fn range_failure(
    negative: bool,
    unsigned: bool,
    [below_zero, above, below]: [Site; 3],
    start: usize,
    len: usize,
) -> Failure {
    let site = match negative {
        true if unsigned => below_zero,
        true => below,
        false => above,
    };
    Failure::at(site, start, len)
}

/// The fast path's range sites: below zero at an unsigned door, above the maximum, below
/// the minimum.
const FAST_RANGE: [Site; 3] = [Site::IntFastNegative, Site::IntFastTooLarge, Site::IntFastTooSmall];
/// The lenient lane's range sites, in the same order.
const LANE_RANGE: [Site; 3] = [Site::IntLaneNegative, Site::IntLaneTooLarge, Site::IntLaneTooSmall];

/// Which of `[sign, not_finite, other]` a character the numeral doesn't allow at `body[at..]`
/// is: a sign (a second one, or one inside the numeral), the first letter of a body that is
/// exactly `nan`, `inf` or `infinity` (ASCII case-insensitive), or anything else.
pub(crate) fn stray(
    body: &[u8],
    at: usize,
    format: &NumFormat,
    [sign, not_finite, other]: [Site; 3],
) -> Site {
    if read_sign(body.get(at..).unwrap_or_default(), format).is_some() {
        sign
    } else if at == 0
        && (body.eq_ignore_ascii_case(b"nan")
            || body.eq_ignore_ascii_case(b"inf")
            || body.eq_ignore_ascii_case(b"infinity"))
    {
        not_finite
    } else {
        other
    }
}

/// The byte length of a group separator at `text[at..]` where the format permits grouping
/// ([`NumFormat::group_at`]), `0` otherwise.
#[inline]
pub(crate) fn grouping(format: &NumFormat, text: &[u8], at: usize, currency: bool) -> usize {
    if format.allows(NumFormat::GROUPING) { format.group_at(text, at, currency) } else { 0 }
}

/// A strict grouping policy's judge ([`NumFormat::GROUPING_WESTERN`],
/// [`NumFormat::GROUPING_INDIAN`]), fed a numeral's integer part as the engine reads it. Each
/// group is judged as soon as it closes, at the next group separator or at whatever ends the
/// integer part, so the leftmost invalid group in reading order is the one reported. Under
/// legacy it judges nothing.
pub(crate) struct Groups {
    /// `(leftmost, inner)` from [`NumFormat::strict_groups`]; `None` once nothing is left to
    /// judge (legacy, or the integer part has ended).
    sizes: Option<(u32, u32)>,
    /// Digits in the group being read.
    digits: u32,
    /// Where that group's span starts in the body: its first digit for the leftmost group,
    /// its separator for every other.
    start: usize,
    /// Whether a group separator has been read.
    separated: bool,
    /// The body's offset in the caller's input, and the site a wrong size reports.
    base: usize,
    site: Site,
}

impl Groups {
    #[inline]
    pub(crate) fn new(format: &NumFormat, base: usize, site: Site) -> Groups {
        Groups { sizes: format.strict_groups(), digits: 0, start: 0, separated: false, base, site }
    }

    /// One digit at `at` in the body.
    #[inline(always)]
    pub(crate) fn digit(&mut self, at: usize) {
        // Nothing is counted where nothing will be judged: legacy keeps the digit loop bare.
        if self.sizes.is_none() {
            return;
        }
        if self.digits == 0 && !self.separated {
            self.start = at;
        }
        self.digits += 1;
    }

    /// A group separator at `at`, right after a digit: the group it closes is judged, the
    /// leftmost against `1..=leftmost`, any other against `inner`.
    #[inline(always)]
    pub(crate) fn separator(&mut self, at: usize) -> Result<(), Failure> {
        let Some((leftmost, inner)) = self.sizes else {
            return Ok(());
        };
        let fits = if self.separated { self.digits == inner } else { self.digits <= leftmost };
        if !fits {
            return Err(self.wrong(at));
        }
        self.separated = true;
        self.digits = 0;
        self.start = at;
        Ok(())
    }

    /// The integer part ends at `at` (the decimal separator, an exponent, any other
    /// character, or the body's end): a grouped numeral's last group holds three digits.
    /// Nothing is judged after the first call.
    #[inline(always)]
    pub(crate) fn end(&mut self, at: usize) -> Result<(), Failure> {
        if self.sizes.take().is_some() && self.separated && self.digits != 3 {
            return Err(self.wrong(at));
        }
        Ok(())
    }

    /// Whether `format` declares a strict policy, under which a group separator after an
    /// exponent's digits is grouping in the exponent rather than a stray character.
    #[inline]
    pub(crate) fn strict(format: &NumFormat) -> bool {
        format.strict_groups().is_some()
    }

    #[cold]
    fn wrong(&self, end: usize) -> Failure {
        Failure::at(self.site, self.base + self.start, end.saturating_sub(self.start))
    }
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
            integer::<$ty>(input.as_ref(), format).map_err(Failure::fault)
        }
    )+};
}

/// An integer door's one parse, for both entry points: the engine over `T`'s range.
#[inline(always)]
pub(crate) fn integer<T: Integer>(input: &[u8], format: &NumFormat) -> Result<T, Failure> {
    parse_int(input, format, T::MIN, T::MAX, T::BITS, true).map(T::narrow)
}

/// An integer door's type, as the engine reads it.
pub(crate) trait Integer: Copy {
    const MIN: i128;
    const MAX: i128;
    const BITS: u32;
    /// A value the engine has range-checked against `MIN..=MAX`.
    fn narrow(value: i128) -> Self;
}

macro_rules! integers {
    ($($ty:ty),+) => {$(
        impl Integer for $ty {
            const MIN: i128 = <$ty>::MIN as i128;
            const MAX: i128 = <$ty>::MAX as i128;
            const BITS: u32 = <$ty>::BITS;
            fn narrow(value: i128) -> Self {
                value as $ty
            }
        }
    )+};
}

integers!(i8, i16, i32, i64, u8, u16, u32, u64);

/// The engine's answer with the lenient lane switched off — what `lib.rs`'s differential
/// test compares every integer door against.
#[cfg(test)]
pub(crate) fn engine_only<T: TryFrom<i128>>(
    input: &[u8],
    format: &NumFormat,
    min: i128,
    max: i128,
    bits: u32,
) -> Result<T, Failure> {
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
