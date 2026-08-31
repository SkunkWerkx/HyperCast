//! The real doors (f32/f64). Svartalfheim's `RealParser` re-hosted on a caller-declared
//! [`NumFormat`]: declared grouping and decimal separator, accounting parentheses, exponent
//! form, and trailing-percent notation (`50%` ⇒ 0.5).
//!
//! Only finite reals come out. The scanner admits nothing but sign, digits, separators, and
//! exponent — so the `NaN`/`Infinity` literals Rust's own parser would accept are `Malformed`
//! here by construction — and a well-formed magnitude that overflows to ±∞ (`1e400`) is
//! `OutOfRange`. The parse itself is `core`'s dec2flt via `str::parse`, fed from a fixed
//! stack buffer holding the normalized ASCII (declared separators swapped to invariant,
//! grouping stripped): `core` cannot allocate, so neither can this door.

use crate::integer::{char_len, strip_parens, Sep};
use crate::verdict::{trim, Fault, NumFormat};

/// Upper bound on the normalized numeric text — Svartalfheim's decimal digit guard
/// generalized: no meaningful finite real needs this many characters, and a fixed bound is
/// what keeps the scratch space on the stack.
const MAX_NORMALIZED: usize = 256;

macro_rules! real_doors {
    ($($(#[$doc:meta])* $door:ident => $ty:ty),+ $(,)?) => {$(
        $(#[$doc])*
        pub fn $door(input: impl AsRef<[u8]>, format: &NumFormat) -> Result<$ty, Fault> {
            let input = input.as_ref();
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
            // hand the caller's own bytes straight to core's dec2flt, no scratch buffer.
            // Non-plain input (declared separators, grouping, parens, percent, or any
            // stray byte) falls through to the full engine; verdicts are identical.
            let value: $ty = if is_plain(text, format) {
                // SAFETY: is_plain admits only ASCII bytes.
                let plain = unsafe { str::from_utf8_unchecked(text) };
                match plain.parse() {
                    Ok(value) => value,
                    Err(_) => return Err(Fault::malformed(start, text.len())),
                }
            } else {
                let mut buf = [0u8; MAX_NORMALIZED];
                let (len, percent) = normalize(text, start, format, &mut buf)?;
                // SAFETY: normalize writes only ASCII bytes.
                let normalized = unsafe { str::from_utf8_unchecked(&buf[..len]) };
                let value: $ty = normalized
                    .parse()
                    .map_err(|_| Fault::malformed(start, text.len()))?;
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
    cast_f32 => f32,
    /// Casts real text to f64. Empty ⇒ `Empty`; unrecognized ⇒ `Malformed`; a magnitude
    /// beyond f64's finite range ⇒ `OutOfRange`.
    cast_f64 => f64,
}

/// True when the trimmed token is exactly the invariant shape
/// `[+|-]digits[.digits][e[+|-]digits]` (with `.digits`-only mantissas allowed) under a
/// format whose declared separators can't reinterpret any of those bytes — so the full
/// engine would produce the very same characters, and the parse can skip the copy. The
/// exponent arm is gated on the EXPONENT flag; everything else that a flag governs
/// (grouping, parens, percent) uses bytes this shape already excludes, and `NaN`/`inf`
/// literals are excluded by construction, exactly as in the full scanner.
fn is_plain(text: &[u8], format: &NumFormat) -> bool {
    if format.decimal_sep != '.'
        || matches!(format.group_sep, '0'..='9' | '.' | 'e' | 'E' | '+' | '-')
    {
        return false;
    }
    let mut i = usize::from(matches!(text[0], b'+' | b'-'));
    let mut any_digit = false;
    while i < text.len() && text[i].is_ascii_digit() {
        any_digit = true;
        i += 1;
    }
    if i < text.len() && text[i] == b'.' {
        i += 1;
        while i < text.len() && text[i].is_ascii_digit() {
            any_digit = true;
            i += 1;
        }
    }
    if !any_digit {
        return false;
    }
    if i < text.len() && matches!(text[i], b'e' | b'E') {
        if !format.allows(NumFormat::EXPONENT) {
            return false;
        }
        i += 1;
        if i < text.len() && matches!(text[i], b'+' | b'-') {
            i += 1;
        }
        let exponent_digits = i;
        while i < text.len() && text[i].is_ascii_digit() {
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
fn normalize(
    text: &[u8],
    start: usize,
    format: &NumFormat,
    buf: &mut [u8; MAX_NORMALIZED],
) -> Result<(usize, bool), Fault> {
    // Percent strips first, exactly as Svartalfheim checked `trimmed[^1]` first — the parens
    // and sign live inside the percent body: `(2.5)%` is -0.025.
    let (body, percent) = if format.allows(NumFormat::PERCENT) && *text.last().unwrap() == b'%' {
        (text[..text.len() - 1].trim_ascii_end(), true)
    } else {
        (text, false)
    };
    if body.is_empty() {
        return Err(Fault::malformed(start, text.len()));
    }
    let (body, base, parens) = strip_parens(body, start, format)?;

    let mut out = 0;
    let mut push = |byte: u8, out: &mut usize| -> bool {
        if *out >= MAX_NORMALIZED {
            return false;
        }
        buf[*out] = byte;
        *out += 1;
        true
    };
    let overflow = Fault::malformed(start, text.len());

    let mut i = 0;
    let mut negative = parens;
    if body[0] == b'+' || body[0] == b'-' {
        if parens {
            return Err(Fault::malformed(base, 1));
        }
        negative = body[0] == b'-';
        i = 1;
    }
    if negative && !push(b'-', &mut out) {
        return Err(overflow);
    }

    let decimal = Sep::new(format.decimal_sep);
    let group = Sep::new(format.group_sep);
    let mut any_digit = false;
    let mut seen_decimal = false;
    while i < body.len() {
        let byte = body[i];
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
            let between_digits = !seen_decimal
                && i > 0
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
            let mut exp_sign = 0u8;
            if i < body.len() && (body[i] == b'+' || body[i] == b'-') {
                exp_sign = body[i];
                i += 1;
            }
            if i >= body.len() || !body[i].is_ascii_digit() {
                return Err(Fault::malformed(base + e_pos, 1));
            }
            if !push(b'e', &mut out) || (exp_sign != 0 && !push(exp_sign, &mut out)) {
                return Err(overflow);
            }
            while i < body.len() && body[i].is_ascii_digit() {
                if !push(body[i], &mut out) {
                    return Err(overflow);
                }
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
    Ok((out, percent))
}
