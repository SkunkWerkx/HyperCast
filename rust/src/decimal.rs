//! The decimal door. Same grammar as the real doors — declared separators and grouping,
//! accounting parentheses, exponent, trailing percent, declared currency — but the value
//! comes out exact: a sign, a 96-bit magnitude and a base-10 scale ([`Decimal`]), the
//! shape .NET's `decimal` stores and the arbitrary-precision decimal types elsewhere build
//! from. No float is ever formed, so `0.1` is one tenth and `50%` is exactly `0.5`. The
//! result is canonical: exact trailing zeros in the fraction are trimmed, so `1.10`, `1.1`
//! and `1.1000` all come out as magnitude 11, scale 1, and zero is scale 0 and never
//! negative.
//!
//! Precision is a range, not a rounding opportunity: a magnitude past 2⁹⁶ − 1 or a scale
//! past 28 once the zeros are trimmed is `OutOfRange`. The door never drops a nonzero
//! digit — the one thing a caller who reached for a decimal instead of a double is
//! entitled to assume.

use crate::real::{is_plain, normalize, MAX_NORMALIZED};
use crate::verdict::{trim, Decimal, Fault, NumFormat};

/// The largest magnitude a [`Decimal`] carries: 2⁹⁶ − 1.
const MAX_MAGNITUDE: u128 = (1u128 << 96) - 1;
/// The largest scale a [`Decimal`] carries.
const MAX_SCALE: u32 = 28;
/// Exponents past this are clamped; with any nonzero digit the value is out of range long
/// before, and with none it is zero regardless.
const EXPONENT_CLAMP: i32 = 100_000;

/// Casts decimal text to an exact, canonical [`Decimal`] under the declared `format`.
/// Empty ⇒ `Empty`; unrecognized ⇒ `Malformed`; a magnitude past 96 bits, or more
/// fractional precision than 28 places can hold after trimming exact zeros, ⇒ `OutOfRange`.
pub fn cast_decimal(input: impl AsRef<[u8]>, format: &NumFormat) -> Result<Decimal, Fault> {
    let input = input.as_ref();
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }
    let resolved;
    let format = if format.allows(NumFormat::SEPARATOR_DETECT) {
        resolved = format.resolve_detected(text, start)?;
        &resolved
    } else {
        format
    };
    let whole_token = Fault::malformed(start, text.len());
    let out_of_range = Fault::out_of_range(start, text.len());
    if is_plain(text, format) {
        return from_invariant(text, false).map_err(|range| if range { out_of_range } else { whole_token });
    }
    let mut buf = [0u8; MAX_NORMALIZED];
    let (len, percent) = normalize(text, start, format, &mut buf)?;
    from_invariant(&buf[..len], percent).map_err(|range| if range { out_of_range } else { whole_token })
}

/// Reads the invariant shape `[+|-]digits[.digits][e[+|-]digits]` (with `.digits`-only
/// mantissas allowed) into a [`Decimal`]. `Err(false)` is malformed, `Err(true)` is out of
/// range.
fn from_invariant(text: &[u8], percent: bool) -> Result<Decimal, bool> {
    let mut i = 0;
    let negative = match text.first() {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };

    let mut magnitude: u128 = 0;
    let mut any_digit = false;
    let mut seen_point = false;
    let mut fraction_digits: u32 = 0;
    // A fraction zero that no longer fits the magnitude is simply not read: it would only
    // widen the scale, and the reduction below would shed it again as an exact zero. Once
    // one has been skipped every later digit is either another skipped zero or a nonzero
    // digit that cannot be represented, which is out of range outright.
    while i < text.len() {
        let byte = text[i];
        if byte.is_ascii_digit() {
            any_digit = true;
            let digit = u128::from(byte - b'0');
            match magnitude.checked_mul(10).and_then(|shifted| shifted.checked_add(digit)) {
                Some(next) if next <= MAX_MAGNITUDE || !seen_point => magnitude = next,
                _ if seen_point && digit == 0 => {
                    i += 1;
                    continue;
                }
                _ => return Err(true),
            }
            if seen_point {
                fraction_digits += 1;
            }
        } else if byte == b'.' && !seen_point {
            seen_point = true;
        } else {
            break;
        }
        i += 1;
    }
    if !any_digit {
        return Err(false);
    }
    let mut exponent: i32 = 0;
    if i < text.len() && matches!(text[i], b'e' | b'E') {
        i += 1;
        let exponent_negative = match text.get(i) {
            Some(b'-') => {
                i += 1;
                true
            }
            Some(b'+') => {
                i += 1;
                false
            }
            _ => false,
        };
        let digits_at = i;
        while i < text.len() && text[i].is_ascii_digit() {
            exponent = (exponent * 10 + i32::from(text[i] - b'0')).min(EXPONENT_CLAMP);
            i += 1;
        }
        if i == digits_at {
            return Err(false);
        }
        if exponent_negative {
            exponent = -exponent;
        }
    }
    if i != text.len() {
        return Err(false);
    }

    let mut scale: i32 = fraction_digits as i32 - exponent + if percent { 2 } else { 0 };
    if magnitude == 0 {
        return Ok(Decimal { lo: 0, hi: 0, scale: 0, negative: false });
    }
    // A negative scale means the exponent outran the fraction: shift the magnitude left.
    while scale < 0 {
        magnitude = magnitude.checked_mul(10).filter(|next| *next <= MAX_MAGNITUDE).ok_or(true)?;
        scale += 1;
    }
    // Canonical form: exact trailing zeros in the fraction are shed, always — `1.10` and
    // `1.1` are the same value and come out the same. Nothing else is ever dropped, so
    // this is also the only way an over-wide or over-deep literal may come to fit.
    while scale > 0 && magnitude.is_multiple_of(10) {
        magnitude /= 10;
        scale -= 1;
    }
    if magnitude > MAX_MAGNITUDE || scale > MAX_SCALE as i32 {
        return Err(true);
    }
    Ok(Decimal {
        lo: magnitude as u64,
        hi: (magnitude >> 64) as u32,
        scale: scale as u8,
        negative,
    })
}
