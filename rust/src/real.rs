//! The real doors (f32/f64). Svartalfheim's `RealParser` re-hosted on a caller-declared
//! [`NumFormat`]: declared grouping and decimal separator, accounting parentheses, exponent
//! form, and trailing-percent notation (`50%` ⇒ 0.5).
//!
//! Only finite reals come out. The scanner admits nothing but sign, digits, separators, and
//! exponent — so the `NaN`/`Infinity` literals Rust's own parser would accept are `Malformed`
//! here by construction — and a well-formed magnitude that overflows to ±∞ (`1e400`) is
//! `OutOfRange`. The conversion itself is `float.rs` — `core`'s own algorithms for the
//! ordinary case, an exact integer division for the rest, none of it able to panic — fed
//! from a fixed stack buffer holding the normalized ASCII (declared separators swapped to
//! invariant, grouping stripped). Nothing allocates.

use crate::float::{self, Real};
use crate::integer::{Sep, char_len_at, is_digit_at, split_sign, strip_currency, strip_parens};
use crate::lane;
use crate::verdict::{Fault, NumFormat, trim};

/// Upper bound on the normalized numeric text — Svartalfheim's decimal digit guard
/// generalized: no meaningful finite real needs this many characters, and a fixed bound is
/// what keeps the scratch space on the stack.
pub(crate) const MAX_NORMALIZED: usize = 256;

macro_rules! real_doors {
    ($($(#[$doc:meta])* $door:ident / $inner:ident / $engine_only:ident => $ty:ty),+ $(,)?) => {$(
        $(#[$doc])*
        pub fn $door(input: impl AsRef<[u8]>, format: &NumFormat) -> Result<$ty, Fault> {
            $inner(input.as_ref(), format, true)
        }

        /// The engine's answer with the lenient lane switched off — what `lib.rs`'s
        /// differential test compares the door against.
        #[cfg(test)]
        pub(crate) fn $engine_only(input: &[u8], format: &NumFormat) -> Result<$ty, Fault> {
            $inner(input, format, false)
        }

        #[inline(always)]
        fn $inner(input: &[u8], format: &NumFormat, lenient_lane: bool) -> Result<$ty, Fault> {
            let (text, start) = trim(input);
            if text.is_empty() {
                return Err(Fault::EMPTY);
            }
            // Separator detection resolves the '.'/',' roles from structure before either
            // path runs — it must precede is_plain, or "1.234" (ambiguous under
            // detection) would slip through the invariant fast lane as 1.234.
            let resolved;
            let format = if format.allows(NumFormat::SEPARATOR_DETECT) {
                resolved = format.resolve_detected(text, start)?;
                &resolved
            } else {
                format
            };
            // Fast path: a token already in the invariant shape needs no normalization —
            // hand the caller's own bytes straight to the conversion, no scratch buffer.
            // Non-plain input (declared separators, grouping, parens, percent, or any
            // stray byte) falls through to the full engine; verdicts are identical.
            //
            // `float::parse` reads exactly `is_plain`'s grammar and declines anything
            // else, so when the format allows the exponent the parse is the shape check
            // too, and the token is scanned once instead of twice. Without the exponent
            // flag the parse would accept an `e` the format refuses, so `is_plain` decides.
            let plain: Option<$ty> = if reads_plain(format) && format.allows(NumFormat::EXPONENT) {
                float::parse(text)
            } else if is_plain(text, format) {
                float::parse(text)
            } else {
                None
            };
            let value: $ty = if let Some(value) = plain {
                value
            } else if let Some(value) = lenient::<$ty>(text, format, lenient_lane) {
                // The lenient lane (lane.rs): grouped digits, a declared currency symbol
                // and accounting parentheses. The finite check below is the engine's own.
                value
            } else {
                let mut buf = [0u8; MAX_NORMALIZED];
                let (len, percent) = normalize(text, start, format, &mut buf)?;
                let value: $ty = float::parse(buf.get(..len).unwrap_or_default())
                    .ok_or(Fault::malformed(start, text.len()))?;
                if !value.is_finite() {
                    return Err(Fault::out_of_range(start, text.len()));
                }
                return Ok(if percent { value / 100.0 } else { value });
            };
            if !value.is_finite() {
                return Err(Fault::out_of_range(start, text.len()));
            }
            Ok(value)
        }
    )+};
}

real_doors! {
    /// Casts real text to f32. Empty ⇒ `Empty`; unrecognized ⇒ `Malformed`; a magnitude
    /// beyond f32's finite range ⇒ `OutOfRange`.
    cast_f32 / real_f32 / engine_only_f32 => f32,
    /// Casts real text to f64. Empty ⇒ `Empty`; unrecognized ⇒ `Malformed`; a magnitude
    /// beyond f64's finite range ⇒ `OutOfRange`.
    cast_f64 / real_f64 / engine_only_f64 => f64,
}

/// The lenient lane for a real: the lane reads the significant digits straight into a
/// significand, with every grouping separator, symbol and parenthesis left out, and
/// `float::from_significand` converts it. The sign is applied afterwards, which is exact —
/// negation flips one bit. `None` falls through to the full engine.
#[inline]
fn lenient<T: Real>(text: &[u8], format: &NumFormat, lenient_lane: bool) -> Option<T> {
    if !lenient_lane {
        return None;
    }
    let mut sink = lane::Significand::default();
    let negative = lane::scan(text, format, true, &mut sink)?;
    let value: T = float::from_significand(sink.value, -i64::from(sink.fraction_digits))?;
    Some(if negative { -value } else { value })
}

/// True when `format`'s declared separators cannot reinterpret any byte of the plain
/// invariant shape — the precondition for reading such a token as written.
pub(crate) fn reads_plain(format: &NumFormat) -> bool {
    format.decimal_sep == '.'
        && !matches!(format.group_sep, '0'..='9' | '.' | 'e' | 'E' | '+' | '-')
}

/// True when the trimmed token is exactly the invariant shape
/// `[+|-]digits[.digits][e[+|-]digits]` (with `.digits`-only mantissas allowed) under a
/// format whose declared separators can't reinterpret any of those bytes — so the full
/// engine would produce the very same characters, and the parse can skip the copy. The
/// exponent arm is gated on the EXPONENT flag; everything else that a flag governs
/// (grouping, parens, percent) uses bytes this shape already excludes, and `NaN`/`inf`
/// literals are excluded by construction, exactly as in the full scanner.
pub(crate) fn is_plain(text: &[u8], format: &NumFormat) -> bool {
    if !reads_plain(format) {
        return false;
    }
    let mut i = usize::from(split_sign(text).0.is_some());
    let mut any_digit = false;
    while is_digit_at(text, i) {
        any_digit = true;
        i += 1;
    }
    if text.get(i) == Some(&b'.') {
        i += 1;
        while is_digit_at(text, i) {
            any_digit = true;
            i += 1;
        }
    }
    if !any_digit {
        return false;
    }
    if let Some(b'e' | b'E') = text.get(i) {
        if !format.allows(NumFormat::EXPONENT) {
            return false;
        }
        i += 1;
        if let Some(b'+' | b'-') = text.get(i) {
            i += 1;
        }
        let exponent_digits = i;
        while is_digit_at(text, i) {
            i += 1;
        }
        if i == exponent_digits {
            return false;
        }
    }
    i == text.len()
}

/// Scans the trimmed token under the declared format and writes the invariant ASCII form
/// (`[-]digits[.digits][e[+|-]digits]`) into `buf`, returning the written length and whether
/// a trailing percent was consumed.
pub(crate) fn normalize(
    text: &[u8],
    start: usize,
    format: &NumFormat,
    buf: &mut [u8; MAX_NORMALIZED],
) -> Result<(usize, bool), Fault> {
    // Percent strips first, exactly as Svartalfheim checked `trimmed[^1]` first — the parens
    // and sign live inside the percent body: `(2.5)%` is -0.025.
    let (body, percent) = match text.strip_suffix(b"%") {
        Some(rest) if format.allows(NumFormat::PERCENT) => (rest.trim_ascii_end(), true),
        _ => (text, false),
    };
    if body.is_empty() {
        return Err(Fault::malformed(start, text.len()));
    }
    let (body, base, parens) = strip_parens(body, start, format)?;
    let (body, base, pre_sign) = strip_currency(body, base, format)?;

    let mut out = 0;
    let mut push = |byte: u8, out: &mut usize| -> bool {
        let Some(slot) = buf.get_mut(*out) else {
            return false;
        };
        *slot = byte;
        *out += 1;
        true
    };
    let overflow = Fault::malformed(start, text.len());

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
            return Err(Fault::malformed(base, 1));
        }
        negative = sign == b'-';
        i = 1;
    }
    if negative && !push(b'-', &mut out) {
        return Err(overflow);
    }

    let decimal = Sep::new(format.decimal_sep);
    let group = Sep::new(format.group_sep);
    let mut any_digit = false;
    let mut seen_decimal = false;
    while let Some(&byte) = body.get(i) {
        if byte.is_ascii_digit() {
            any_digit = true;
            if !push(byte, &mut out) {
                return Err(overflow);
            }
            i += 1;
        } else if decimal.matches(body, i) {
            if seen_decimal {
                return Err(Fault::malformed(base + i, decimal.len));
            }
            seen_decimal = true;
            if !push(b'.', &mut out) {
                return Err(overflow);
            }
            i += decimal.len;
        } else if format.allows(NumFormat::GROUPING) && group.matches(body, i) {
            let after = i + group.len;
            // Grouping lives in the integer part only, strictly between digits.
            let between_digits =
                !seen_decimal && is_digit_at(body, i.wrapping_sub(1)) && is_digit_at(body, after);
            if !between_digits {
                return Err(Fault::malformed(base + i, group.len));
            }
            i = after;
        } else if (byte == b'e' || byte == b'E') && format.allows(NumFormat::EXPONENT) && any_digit
        {
            let e_pos = i;
            i += 1;
            let mut exp_sign = 0u8;
            if let Some(&sign @ (b'+' | b'-')) = body.get(i) {
                exp_sign = sign;
                i += 1;
            }
            if !is_digit_at(body, i) {
                return Err(Fault::malformed(base + e_pos, 1));
            }
            if !push(b'e', &mut out) || (exp_sign != 0 && !push(exp_sign, &mut out)) {
                return Err(overflow);
            }
            while let Some(&digit @ b'0'..=b'9') = body.get(i) {
                if !push(digit, &mut out) {
                    return Err(overflow);
                }
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
    Ok((out, percent))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real doors let `float::parse` stand in for `is_plain` whenever the format allows
    /// the exponent. That holds only while the two read the same grammar, so every token of
    /// up to seven characters over the bytes that grammar is made of is held to it.
    #[test]
    fn float_parse_accepts_exactly_the_plain_shape() {
        const ALPHABET: &[u8] = b"09.eE+-x";
        let mut token = [0u8; 7];
        for len in 0..=token.len() {
            let total = ALPHABET.len().pow(len as u32);
            for mut index in 0..total {
                for slot in &mut token[..len] {
                    *slot = ALPHABET[index % ALPHABET.len()];
                    index /= ALPHABET.len();
                }
                let text = &token[..len];
                assert_eq!(
                    is_plain(text, &NumFormat::INVARIANT),
                    float::parse::<f64>(text).is_some(),
                    "{:?}",
                    core::str::from_utf8(text)
                );
            }
        }
    }
}
