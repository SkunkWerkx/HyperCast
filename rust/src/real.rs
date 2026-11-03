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
use crate::integer::{Frame, char_len_at, frame, is_digit_at, read_digit, split_sign};
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
            //
            // The doors do not call `route`; they make the same decision in this order, held
            // to it by tests: `float_parse_accepts_exactly_the_plain_shape` (the parse accepts
            // exactly `is_plain`'s tokens), and exact.rs's `exact_agrees_with_the_door` and
            // `route_precedence` (the exact view, which does call `route`, reads every token
            // to the door's value).
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
                let normalized = buf.get(..len).unwrap_or_default();
                // A percent is the same digits two places further right of the point, read
                // in one correctly rounded conversion: dividing the parsed float by 100
                // would round twice (`1.1%` would be `0.011000000000000001`).
                let mut hundredths = [0u8; MAX_NORMALIZED + 3];
                let text_read = if percent {
                    let len = shift_point_left_two(normalized, &mut hundredths);
                    hundredths.get(..len).unwrap_or_default()
                } else {
                    normalized
                };
                // A tiny negative percent can underflow to zero (`-1e-400%`); `float::parse`
                // leaves every zero unsigned.
                let value: $ty =
                    float::parse(text_read).ok_or(Fault::malformed(start, text.len()))?;
                if !value.is_finite() {
                    return Err(Fault::out_of_range(start, text.len()));
                }
                return Ok(value);
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
    /// beyond f32's finite range ⇒ `OutOfRange`. Casts never produce a negative zero: `-0`,
    /// `(0)` and an underflow like `-1e-400` are `0.0`.
    cast_f32 / real_f32 / engine_only_f32 => f32,
    /// Casts real text to f64. Empty ⇒ `Empty`; unrecognized ⇒ `Malformed`; a magnitude
    /// beyond f64's finite range ⇒ `OutOfRange`. Casts never produce a negative zero: `-0`,
    /// `(0)` and an underflow like `-1e-400` are `0.0`.
    cast_f64 / real_f64 / engine_only_f64 => f64,
}

/// The lenient lane for a real: the lane reads the significant digits straight into a
/// significand, with every grouping separator, symbol and parenthesis left out, and
/// `float::from_significand` converts it. The sign is applied afterwards, which is exact —
/// negation flips one bit — except on a zero, which stays unsigned, as `float::parse` leaves
/// it. `None` falls through to the full engine.
#[inline]
fn lenient<T: Real>(text: &[u8], format: &NumFormat, lenient_lane: bool) -> Option<T> {
    if !lenient_lane {
        return None;
    }
    let mut sink = lane::Significand::default();
    let negative = lane::scan(text, format, true, &mut sink)?;
    let value: T = float::from_significand(sink.value, -i64::from(sink.fraction_digits))?;
    Some(if negative { float::unsigned(-value) } else { value })
}

/// Which reading a numeric token takes, as [`route`] decides it for the exact view
/// (`exact.rs`): the decision the real and decimal doors make in their own flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Route {
    /// The plain invariant shape, read as written ([`is_plain`]).
    Plain,
    /// Everything else: the grammar [`walk`] streams. The doors' lenient lane is a fast
    /// path inside this route, reading only shapes the engine reads, to the same value.
    Engine,
}

/// The routing decision the real and decimal doors make for the trimmed, non-empty `text`
/// at offset `start`, for the exact view (`Exact::from_text`). Separator detection resolves
/// first, into `resolved`: it must precede the plain shape, or `1.234` (undecidable under
/// detection) would be read as 1.234. Then the plain shape is tried before anything strips
/// a currency symbol, so `.5` under a declared symbol of `.` is one half. Returns the route
/// and the format to read it under.
///
/// The doors do not call this: each keeps its own single-pass probe ahead of everything,
/// which a shared call would scan twice. The decision is held in one place by tests instead
/// of by a call: `float_parse_accepts_exactly_the_plain_shape` (the real doors' probe accepts
/// exactly [`is_plain`]'s tokens), and exact.rs's `exact_agrees_with_the_door` (the view
/// reads every corpus token each door accepts to the door's value) and `route_precedence`
/// (detection, and the plain shape before the currency symbol).
pub(crate) fn route<'f>(
    text: &[u8],
    start: usize,
    format: &'f NumFormat,
    resolved: &'f mut NumFormat,
) -> Result<(Route, &'f NumFormat), Fault> {
    let format: &'f NumFormat = if format.allows(NumFormat::SEPARATOR_DETECT) {
        *resolved = format.resolve_detected(text, start)?;
        resolved
    } else {
        format
    };
    let route = if is_plain(text, format) { Route::Plain } else { Route::Engine };
    Ok((route, format))
}

/// One piece of a numeric token as the engine reads it, in reading order: the sign, the
/// mantissa's digits and point, the exponent as written, and a trailing percent last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Token {
    /// The token is negative (a sign, or accounting parentheses). First when present.
    Negative,
    /// A mantissa digit, as its value `0..=9`, native digits included.
    Digit(u8),
    /// The decimal separator.
    Point,
    /// The exponent's sign, `b'+'` or `b'-'`, when one is written.
    ExponentSign(u8),
    /// An exponent digit, as its value `0..=9`, leading zeros included.
    ExponentDigit(u8),
    /// A trailing percent was read: the value is a hundredth of what the digits say.
    Percent,
}

/// Streams the trimmed token under `format` as [`Token`]s: the engine grammar (signs,
/// parentheses, currency symbol, separators, native digits, exponent and, where
/// `percent_door` says the door reads it, a trailing percent), with no buffer and no length
/// limit. The exponent is streamed as written, never collapsed into an integer here.
/// `emit` returning `false` stops the walk at once, with `Ok`. A fault is the engine's own,
/// and may come after some tokens were emitted.
pub(crate) fn walk(
    text: &[u8],
    start: usize,
    format: &NumFormat,
    percent_door: bool,
    emit: &mut impl FnMut(Token) -> bool,
) -> Result<(), Fault> {
    let Frame { body, base, negative, percent, currency } =
        frame(text, start, format, percent_door)?;
    let (decimal, group) = format.separators(currency);

    if negative && !emit(Token::Negative) {
        return Ok(());
    }
    let mut i = 0;
    let mut any_digit = false;
    let mut seen_decimal = false;
    // Whether the byte before `i` was a digit — what a group separator must sit after.
    let mut after_digit = false;
    while let Some(&byte) = body.get(i) {
        if let Some((digit, len)) = read_digit(body, i, format) {
            any_digit = true;
            after_digit = true;
            if !emit(Token::Digit(digit)) {
                return Ok(());
            }
            i += len;
        } else if decimal.matches(body, i) {
            if seen_decimal {
                return Err(Fault::malformed(base + i, decimal.len()));
            }
            seen_decimal = true;
            after_digit = false;
            if !emit(Token::Point) {
                return Ok(());
            }
            i += decimal.len();
        } else if format.allows(NumFormat::GROUPING) && group.matches(body, i) {
            let after = i + group.len();
            // Grouping lives in the integer part only, strictly between digits.
            let between_digits =
                !seen_decimal && after_digit && read_digit(body, after, format).is_some();
            if !between_digits {
                return Err(Fault::malformed(base + i, group.len()));
            }
            after_digit = false;
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
            if read_digit(body, i, format).is_none() {
                return Err(Fault::malformed(base + e_pos, 1));
            }
            if exp_sign != 0 && !emit(Token::ExponentSign(exp_sign)) {
                return Ok(());
            }
            while let Some((digit, len)) = read_digit(body, i, format) {
                if !emit(Token::ExponentDigit(digit)) {
                    return Ok(());
                }
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
    if percent {
        emit(Token::Percent);
    }
    Ok(())
}

/// True when `format`'s declared separators cannot reinterpret any byte of the plain
/// invariant shape — the precondition for reading such a token as written. A group
/// separator of one ASCII byte outside that shape's alphabet, or starting outside ASCII,
/// cannot; one of several ASCII bytes (`e3`) could, and sends every token to the engine.
pub(crate) fn reads_plain(format: &NumFormat) -> bool {
    format.decimal_sep.as_bytes() == b"."
        && match format.group_sep.as_bytes() {
            [byte] => !matches!(byte, b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-'),
            [lead, ..] => *lead >= 0x80,
            [] => true,
        }
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
/// a trailing percent was consumed: [`walk`]'s tokens, written out. A token longer than the
/// buffer is `Malformed` as a whole; the exact view reads such a token by walking it.
pub(crate) fn normalize(
    text: &[u8],
    start: usize,
    format: &NumFormat,
    buf: &mut [u8; MAX_NORMALIZED],
) -> Result<(usize, bool), Fault> {
    let mut out = 0;
    let mut overflow = false;
    let mut percent = false;
    let mut exponent = false;
    walk(text, start, format, true, &mut |token| {
        let (bytes, len): ([u8; 2], usize) = match token {
            Token::Negative => ([b'-', 0], 1),
            Token::Digit(digit) => ([b'0'.wrapping_add(digit), 0], 1),
            Token::Point => ([b'.', 0], 1),
            Token::ExponentSign(sign) => {
                exponent = true;
                ([b'e', sign], 2)
            }
            Token::ExponentDigit(digit) if !exponent => {
                exponent = true;
                ([b'e', b'0'.wrapping_add(digit)], 2)
            }
            Token::ExponentDigit(digit) => ([b'0'.wrapping_add(digit), 0], 1),
            Token::Percent => {
                percent = true;
                return true;
            }
        };
        for &byte in bytes.get(..len).unwrap_or_default() {
            let Some(slot) = buf.get_mut(out) else {
                overflow = true;
                return false;
            };
            *slot = byte;
            out += 1;
        }
        true
    })?;
    if overflow {
        return Err(Fault::malformed(start, text.len()));
    }
    Ok((out, percent))
}

/// Writes the normalized number `text` (`[-]digits[.digits][e…]`, digits on at least one side
/// of the point) into `out` divided by 100, exactly: its point moved two places left, zeros
/// padding a short integer part, the exponent copied unchanged. Returns the length. `out` is
/// three bytes longer than any normalized text, the most the move adds (`5` is `0.05`).
fn shift_point_left_two(text: &[u8], out: &mut [u8; MAX_NORMALIZED + 3]) -> usize {
    let mut len = 0;
    let mut push = |byte: u8| {
        if let Some(slot) = out.get_mut(len) {
            *slot = byte;
            len += 1;
        }
    };
    let (negative, rest) = match text.split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, text),
    };
    let split = rest.iter().position(|&byte| byte == b'e').unwrap_or(rest.len());
    let (mantissa, exponent) = rest.split_at_checked(split).unwrap_or((rest, &[]));
    let point = mantissa.iter().position(|&byte| byte == b'.').unwrap_or(mantissa.len());
    let (whole, fraction) = mantissa.split_at_checked(point).unwrap_or((mantissa, &[]));
    let fraction = fraction.get(1..).unwrap_or_default();
    if negative {
        push(b'-');
    }
    let kept = whole.len().saturating_sub(2);
    let (head, moved) = whole.split_at_checked(kept).unwrap_or((&[], whole));
    if head.is_empty() {
        push(b'0');
    }
    for &byte in head {
        push(byte);
    }
    push(b'.');
    for _ in moved.len()..2 {
        push(b'0');
    }
    for &byte in moved.iter().chain(fraction).chain(exponent) {
        push(byte);
    }
    len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_moves_the_point() {
        let shifted = |text: &str| {
            let mut out = [0u8; MAX_NORMALIZED + 3];
            let len = shift_point_left_two(text.as_bytes(), &mut out);
            String::from_utf8(out[..len].to_vec()).unwrap()
        };
        assert_eq!(shifted("1.1"), "0.011");
        assert_eq!(shifted("5"), "0.05");
        assert_eq!(shifted("-50"), "-0.50");
        assert_eq!(shifted("123.4"), "1.234");
        assert_eq!(shifted(".5"), "0.005");
        assert_eq!(shifted("5."), "0.05");
        assert_eq!(shifted("-1e-400"), "-0.01e-400");
        assert_eq!(shifted("12e3"), "0.12e3");
        // The longest normalized text still fits.
        let long = "9".repeat(MAX_NORMALIZED);
        assert_eq!(shifted(&long).len(), MAX_NORMALIZED + 1);
        assert_eq!(shifted("-1"), "-0.01");
        // Correctly rounded at both doors, where dividing would round twice.
        let inv = &NumFormat::INVARIANT;
        assert_eq!(cast_f64("1.1%", inv), Ok(0.011));
        assert_eq!(cast_f32("1.3%", inv), Ok(0.013));
        assert_eq!(cast_f64("-1e-400%", inv).map(f64::to_bits), Ok(0));
    }

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
