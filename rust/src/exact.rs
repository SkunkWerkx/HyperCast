//! Exact decimal judgement: a value read by a door, compared with a bound and snapped to a
//! step, without binary floating point and without copying the value's digits.
//!
//! The value is an [`Exact`]: a view of the caller's own text, re-read through the door's
//! own grammar on demand (`real::walk`, under the routing decision the door made,
//! `real::route`), or of ASCII digits in a buffer. It is never limited by a buffer's
//! length: `normalize` stops at 256 bytes, and a plain float door does not.
//!
//! A step is `m · 10^-s` (a [`Decimal`]). The value is a multiple when `v · 10^s` is an
//! integer divisible by `m`, decided by Horner's method mod `m`, digit by digit. Rounding
//! follows the spec's rule over `d = 2r − m`, the first discarded digit and a sticky flag
//! for the rest, so a tie is told apart exactly however long the value.

use core::cmp::Ordering;

use crate::detail::Failure;
use crate::integer::split_sign;
use crate::real::{Route, Token, route, walk};
use crate::typed::ShortestDigits;
use crate::verdict::{Decimal, NumFormat};

/// The bound an effective position is clamped to. Every value beyond it on either side
/// judges identically: below `-C` it is a nonzero fraction smaller than any step or bound
/// can resolve (both have a scale of at most 28), and above `+C` no door has accepted it.
const C: i64 = 1_000_000;

/// The digits a [`Rounded`] holds: a door-accepted value's integer part (309 digits for an
/// `f64`), a step's fraction (28 digits) and a carry, with room to spare.
const ROUNDED_DIGITS: usize = 340;

/// How a value that is not a multiple of its step is treated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    /// Not a multiple is a violation of `multipleOf`.
    Strict = 0,
    /// The nearest multiple, ties to the multiple whose quotient is even.
    Midpoint = 1,
    /// The multiple towards negative infinity.
    Floor = 2,
    /// The multiple towards positive infinity.
    Ceiling = 3,
}

/// An exact decimal value, viewed rather than copied: its sign, the power of ten of its
/// first significant digit, and its significant digits, most significant first, read from
/// the source on demand. Zero has no significant digits and is never negative.
#[derive(Clone, Debug)]
pub(crate) struct Exact<'a> {
    source: Source<'a>,
    negative: bool,
    /// The power of ten of the first significant digit, clamped to ±[`C`].
    position: i64,
}

#[derive(Clone, Debug)]
enum Source<'a> {
    Zero,
    /// ASCII digits, the first of them significant.
    Digits(&'a [u8]),
    /// A plain token's mantissa: ASCII digits and at most one `.`, leading zeros included.
    Plain(&'a [u8]),
    /// A token the engine reads, walked again under the resolved format.
    Engine {
        text: &'a [u8],
        start: usize,
        format: NumFormat,
        percent_door: bool,
    },
}

/// What one pass over a token learns: the sign, the point, the first significant digit,
/// the exponent (saturated) and the percent.
struct Shape {
    negative: bool,
    digits: usize,
    point: Option<usize>,
    first: Option<usize>,
    exponent: i64,
    exponent_negative: bool,
    percent: bool,
    /// What the exponent's magnitude saturates at: the input's length plus [`C`].
    limit: i64,
}

impl Shape {
    fn new(length: usize) -> Shape {
        Shape {
            negative: false,
            digits: 0,
            point: None,
            first: None,
            exponent: 0,
            exponent_negative: false,
            percent: false,
            limit: i64::try_from(length).unwrap_or(i64::MAX).saturating_add(C),
        }
    }

    fn token(&mut self, token: Token) {
        match token {
            Token::Negative => self.negative = true,
            Token::Digit(digit) => {
                if self.first.is_none() && digit != 0 {
                    self.first = Some(self.digits);
                }
                self.digits = self.digits.saturating_add(1);
            }
            Token::Point => self.point = Some(self.digits),
            Token::ExponentSign(sign) => self.exponent_negative = sign == b'-',
            Token::ExponentDigit(digit) => {
                self.exponent = self
                    .exponent
                    .saturating_mul(10)
                    .saturating_add(i64::from(digit))
                    .min(self.limit);
            }
            Token::Percent => self.percent = true,
        }
    }

    /// The view this shape describes, over `source`. The effective position is the
    /// mantissa's (at most the input's length in magnitude) plus the exponent, summed in
    /// `i64` and only then clamped: a long mantissa can cancel a long exponent.
    fn view(self, source: Source<'_>) -> Exact<'_> {
        let Some(first) = self.first else {
            return Exact::ZERO;
        };
        let to_i64 = |count: usize| i64::try_from(count).unwrap_or(i64::MAX);
        let integer_digits = to_i64(self.point.unwrap_or(self.digits));
        let mantissa = integer_digits.saturating_sub(1).saturating_sub(to_i64(first));
        let exponent = if self.exponent_negative { -self.exponent } else { self.exponent };
        let percent = if self.percent { 2 } else { 0 };
        let position = mantissa.saturating_add(exponent).saturating_sub(percent).clamp(-C, C);
        Exact { source, negative: self.negative, position }
    }
}

impl<'a> Exact<'a> {
    const ZERO: Exact<'static> = Exact { source: Source::Zero, negative: false, position: 0 };

    /// The value of the trimmed, non-empty `text` (at offset `start` in the caller's input)
    /// as the real or decimal door reads it (`percent_door`), or an integer door does
    /// without it: the door's own routing, then the plain shape read in place, or the
    /// engine's grammar walked under the resolved format. The fault is the door's.
    pub(crate) fn from_text(
        text: &'a [u8],
        start: usize,
        format: &'a NumFormat,
        percent_door: bool,
    ) -> Result<Exact<'a>, Failure> {
        let mut resolved = NumFormat::EMPTY;
        let (way, used) = route(text, start, format, &mut resolved)?;
        let mut shape = Shape::new(text.len());
        if way == Route::Plain {
            let (sign, rest) = split_sign(text);
            shape.negative = sign == Some(b'-');
            let split = rest.iter().position(|&byte| byte | 0x20 == b'e').unwrap_or(rest.len());
            let (mantissa, exponent) = rest.split_at_checked(split).unwrap_or((rest, &[]));
            for &byte in mantissa {
                shape.token(if byte == b'.' {
                    Token::Point
                } else {
                    Token::Digit(byte.wrapping_sub(b'0'))
                });
            }
            for &byte in exponent.get(1..).unwrap_or_default() {
                shape.token(match byte {
                    b'+' | b'-' => Token::ExponentSign(byte),
                    _ => Token::ExponentDigit(byte.wrapping_sub(b'0')),
                });
            }
            return Ok(shape.view(Source::Plain(mantissa)));
        }
        let format = *used;
        walk(text, start, &format, percent_door, &mut |token| {
            shape.token(token);
            true
        })?;
        Ok(shape.view(Source::Engine { text, start, format, percent_door }))
    }

    /// The value of an integer, its digits written into `buf`.
    pub(crate) fn from_integer(value: i128, buf: &'a mut [u8; 40]) -> Exact<'a> {
        let at = write_digits(value.unsigned_abs(), buf);
        let buf: &'a [u8; 40] = buf;
        Exact::of_digits(buf.get(at..).unwrap_or_default(), value < 0, 0)
    }

    /// The value of a [`Decimal`], its magnitude's digits written into `buf`. A directly
    /// built negative zero is zero.
    pub(crate) fn from_decimal(decimal: &Decimal, buf: &'a mut [u8; 40]) -> Exact<'a> {
        let at = write_digits(decimal.magnitude(), buf);
        let buf: &'a [u8; 40] = buf;
        Exact::of_digits(buf.get(at..).unwrap_or_default(), decimal.negative, decimal.scale.into())
    }

    /// The value of an already-decoded number: its sign, and the shortest round-trip digits
    /// of its magnitude, `None` being zero. Zero is never negative, so `-0.0` is `0.0`.
    pub(crate) fn from_shortest(negative: bool, digits: Option<&'a ShortestDigits>) -> Exact<'a> {
        let Some(shortest) = digits else {
            return Exact::ZERO;
        };
        let position = i64::from(shortest.exponent()) - 1;
        match shortest.digits() {
            [] => Exact::ZERO,
            digits => Exact { source: Source::Digits(digits), negative, position },
        }
    }

    /// The view of ASCII `digits` (no leading zeros) shifted right by `scale` places.
    fn of_digits(digits: &'a [u8], negative: bool, scale: i64) -> Exact<'a> {
        if digits.is_empty() {
            return Exact::ZERO;
        }
        let position = i64::try_from(digits.len()).unwrap_or(i64::MAX) - 1 - scale;
        Exact { source: Source::Digits(digits), negative, position }
    }

    /// True for a negative value; zero never is.
    pub(crate) fn negative(&self) -> bool {
        self.negative
    }

    /// The power of ten of the first significant digit (zero for zero).
    #[cfg(test)]
    pub(crate) fn position(&self) -> i64 {
        self.position
    }

    /// Hands the significant digits to `f` as values `0..=9`, most significant first,
    /// until `f` returns `false` or they run out. Zero has none.
    pub(crate) fn digits(&self, f: &mut impl FnMut(u8) -> bool) {
        match &self.source {
            Source::Zero => {}
            Source::Digits(digits) => {
                for &byte in *digits {
                    if !f(byte.wrapping_sub(b'0')) {
                        return;
                    }
                }
            }
            Source::Plain(mantissa) => {
                let mut started = false;
                for &byte in *mantissa {
                    let digit = byte.wrapping_sub(b'0');
                    if byte == b'.' || (!started && digit == 0) {
                        continue;
                    }
                    started = true;
                    if !f(digit) {
                        return;
                    }
                }
            }
            Source::Engine { text, start, format, percent_door } => {
                let mut started = false;
                // The first pass read this token without a fault, so this one cannot.
                let _ = walk(text, *start, format, *percent_door, &mut |token| match token {
                    Token::Digit(digit) if started || digit != 0 => {
                        started = true;
                        f(digit)
                    }
                    Token::ExponentSign(_) | Token::ExponentDigit(_) => false,
                    _ => true,
                });
            }
        }
    }
}

/// Writes `value`'s decimal digits at the end of `buf`, returning where they start (40 for
/// zero, which has none). A `u128` has at most 39.
fn write_digits(mut value: u128, buf: &mut [u8; 40]) -> usize {
    let mut at = buf.len();
    while value != 0 && at > 0 {
        at -= 1;
        if let Some(slot) = buf.get_mut(at) {
            *slot = b'0' + (value % 10) as u8;
        }
        value /= 10;
    }
    at
}

/// Orders `value` against `bound`: sign, then the position of the first significant
/// digit, then the digits one by one, neither side converted.
pub(crate) fn compare(value: &Exact, bound: &Decimal) -> Ordering {
    let mut buf = [0u8; 40];
    let bound = Exact::from_decimal(bound, &mut buf);
    let Source::Digits(bound_digits) = bound.source else {
        // A zero bound: the value's sign decides.
        return match value.source {
            Source::Zero => Ordering::Equal,
            _ if value.negative => Ordering::Less,
            _ => Ordering::Greater,
        };
    };
    let value_sign = match value.source {
        Source::Zero => 0,
        _ if value.negative => -1,
        _ => 1,
    };
    let bound_sign = if bound.negative { -1 } else { 1 };
    if value_sign != bound_sign {
        return value_sign.cmp(&bound_sign);
    }
    let magnitude = match value.position.cmp(&bound.position) {
        Ordering::Equal => {
            let mut order = Ordering::Equal;
            let mut index = 0usize;
            value.digits(&mut |digit| {
                let other = bound_digits.get(index).map_or(0, |byte| byte.wrapping_sub(b'0'));
                index = index.saturating_add(1);
                order = digit.cmp(&other);
                order == Ordering::Equal
            });
            let rest = bound_digits.get(index..).unwrap_or_default();
            if order == Ordering::Equal && rest.iter().any(|&byte| byte != b'0') {
                Ordering::Less
            } else {
                order
            }
        }
        unequal => unequal,
    };
    if bound.negative { magnitude.reverse() } else { magnitude }
}

/// Orders two decimals exactly.
pub(crate) fn compare_decimals(a: &Decimal, b: &Decimal) -> Ordering {
    let mut buf = [0u8; 40];
    compare(&Exact::from_decimal(a, &mut buf), b)
}

/// Writes `decimal` as canonical ASCII (`[-]digits[e-scale]`), the text `float::parse` reads,
/// returning its length: how a float door reads a declared bound. A magnitude has at most 29
/// digits, so 48 bytes always hold it.
pub(crate) fn write_decimal_canonical(decimal: &Decimal, out: &mut [u8; 48]) -> usize {
    let mut len = 0;
    let mut push = |byte: u8| {
        if let Some(slot) = out.get_mut(len) {
            *slot = byte;
            len += 1;
        }
    };
    let mut digits = [0u8; 40];
    let at = write_digits(decimal.magnitude(), &mut digits);
    let digits = digits.get(at..).unwrap_or_default();
    if digits.is_empty() {
        push(b'0');
        return len;
    }
    if decimal.negative {
        push(b'-');
    }
    for &byte in digits {
        push(byte);
    }
    if decimal.scale != 0 {
        push(b'e');
        push(b'-');
        let mut scale = [0u8; 40];
        let at = write_digits(u128::from(decimal.scale), &mut scale);
        for &byte in scale.get(at..).unwrap_or_default() {
            push(byte);
        }
    }
    len
}

/// What [`snap`] makes of a value.
// The rounded digits live on the stack: the core has no allocator to box them with.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub(crate) enum Snap {
    /// The value is a multiple of the step already.
    Exact,
    /// It is not, and the rounding is strict.
    NotMultiple,
    /// The multiple the rounding chose.
    Rounded(Rounded),
}

/// A rounded value: ASCII digits with no leading or trailing zeros, the power of ten of
/// the last, and the sign. Zero has no digits and is never negative.
#[derive(Clone, Debug)]
pub(crate) struct Rounded {
    buf: [u8; ROUNDED_DIGITS],
    len: u16,
    exponent: i32,
    negative: bool,
}

/// Snaps `value` to the nearest multiple of `step` the rounding allows. The step's sign is
/// ignored; a zero step (which a declaration never holds) is `NotMultiple`, as is the
/// unreachable case of a rounded value too long for a [`Rounded`], which no door-accepted
/// value comes near.
pub(crate) fn snap(value: &Exact, step: &Decimal, mode: Rounding) -> Snap {
    let m = step.magnitude();
    if m == 0 {
        return Snap::NotMultiple;
    }
    let twice = m << 1;
    let scale = i64::from(step.scale);
    // T = |v| · 10^s truncated: its digit count, and where it sits in the buffer, right
    // aligned in a field wide enough for T, for m (at most 29 digits) and for a carry.
    let t_digits = match value.source {
        Source::Zero => 0,
        _ => value.position.saturating_add(scale).saturating_add(1).max(0),
    };
    let fits = t_digits < ROUNDED_DIGITS as i64;
    let t_len = if fits { t_digits as usize } else { 0 };
    let width = t_len.max(29) + 1;
    let mut buf = [0u8; ROUNDED_DIGITS];

    // Horner's method mod 2m over T, which gives the remainder mod m and the quotient's
    // parity together; the first digit past scale s; and whether any later one is nonzero.
    let mut r2: u128 = 0;
    let mut first = 0u8;
    let mut sticky = false;
    let mut index: i64 = 0;
    let last_kept = -scale;
    value.digits(&mut |digit| {
        let weight = value.position.saturating_sub(index);
        if weight >= last_kept {
            r2 = (r2 * 10 + u128::from(digit)) % twice;
            if fits && let Some(slot) = buf.get_mut(width - t_len + index as usize) {
                *slot = digit;
            }
        } else if weight == last_kept - 1 {
            first = digit;
        } else if digit != 0 {
            sticky = true;
            return false;
        }
        index = index.saturating_add(1);
        true
    });
    // The zeros between the last digit and scale s, nine at a time: r2 < 2^97, so times
    // 10^9 stays below 2^127.
    let mut zeros = t_digits.saturating_sub(index).max(0);
    while zeros > 0 {
        let run = zeros.min(9) as u32;
        r2 = r2 * 10u128.pow(run) % twice;
        zeros -= i64::from(run);
    }
    let odd = r2 >= m;
    let r = if odd { r2 - m } else { r2 };
    let fraction_zero = first == 0 && !sticky;
    if r == 0 && fraction_zero {
        return Snap::Exact;
    }
    let up = match mode {
        Rounding::Strict => return Snap::NotMultiple,
        Rounding::Floor => value.negative,
        Rounding::Ceiling => !value.negative,
        Rounding::Midpoint => {
            // The exact distance r + f against m / 2, as d = 2r − m (m < 2^96, so it fits
            // an i128), the first discarded digit and the sticky flag. A tie goes to the
            // multiple whose quotient is even: up exactly when the lower one's is odd.
            let d = (r as i128) * 2 - m as i128;
            match d {
                1.. => true,
                ..=-2 => false,
                0 if fraction_zero => odd,
                0 => true,
                _ if first == 5 && !sticky => odd,
                _ => first > 5 || (first == 5 && sticky),
            }
        }
    };
    if !fits {
        return Snap::NotMultiple;
    }
    // T − r, plus m when rounding up in magnitude, in the field's digits.
    let field = buf.get_mut(..width).unwrap_or_default();
    let mut take = r;
    let mut borrow = 0u8;
    for slot in field.iter_mut().rev() {
        if take == 0 && borrow == 0 {
            break;
        }
        let subtrahend = (take % 10) as u8 + borrow;
        take /= 10;
        if *slot >= subtrahend {
            *slot -= subtrahend;
            borrow = 0;
        } else {
            *slot = *slot + 10 - subtrahend;
            borrow = 1;
        }
    }
    if up {
        let mut add = m;
        let mut carry = 0u8;
        for slot in field.iter_mut().rev() {
            if add == 0 && carry == 0 {
                break;
            }
            let sum = *slot + (add % 10) as u8 + carry;
            add /= 10;
            *slot = sum % 10;
            carry = sum / 10;
        }
    }
    Snap::Rounded(Rounded::from_field(field, -scale, value.negative))
}

impl Rounded {
    /// The canonical form of the digit values in `field`, the last at power `exponent`.
    fn from_field(field: &[u8], mut exponent: i64, negative: bool) -> Rounded {
        let mut end = field.len();
        while end > 0 && field.get(end - 1) == Some(&0) {
            end -= 1;
            exponent += 1;
        }
        let start = field.iter().take(end).take_while(|&&digit| digit == 0).count();
        let digits = field.get(start..end).unwrap_or_default();
        let mut rounded =
            Rounded { buf: [0; ROUNDED_DIGITS], len: 0, exponent: 0, negative: false };
        if digits.is_empty() {
            return rounded;
        }
        for (slot, &digit) in rounded.buf.iter_mut().zip(digits) {
            *slot = b'0' + digit;
        }
        rounded.len = digits.len() as u16;
        rounded.exponent = exponent as i32;
        rounded.negative = negative;
        rounded
    }

    fn digits(&self) -> &[u8] {
        self.buf.get(..usize::from(self.len)).unwrap_or_default()
    }

    /// The rounded value as a view.
    pub(crate) fn as_exact(&self) -> Exact<'_> {
        let digits = self.digits();
        let position = i64::from(self.exponent) + i64::from(self.len) - 1;
        match digits {
            [] => Exact::ZERO,
            _ => Exact { source: Source::Digits(digits), negative: self.negative, position },
        }
    }

    /// The magnitude times `10^shift`, or `None` past `limit`.
    fn magnitude(&self, shift: u32, limit: u128) -> Option<u128> {
        let mut magnitude: u128 = 0;
        for &byte in self.digits() {
            magnitude =
                magnitude.checked_mul(10)?.checked_add(u128::from(byte.wrapping_sub(b'0')))?;
        }
        magnitude = magnitude.checked_mul(10u128.checked_pow(shift)?)?;
        (magnitude <= limit).then_some(magnitude)
    }

    /// The value as an integer, or `None` when it has a fraction or does not fit an `i128`.
    pub(crate) fn to_i128(&self) -> Option<i128> {
        let shift = u32::try_from(self.exponent).ok()?;
        let magnitude = self.magnitude(shift, 1 << 127)?;
        if self.negative {
            0i128.checked_sub_unsigned(magnitude)
        } else {
            i128::try_from(magnitude).ok()
        }
    }

    /// The value as a canonical [`Decimal`], or `None` past 96 bits or a scale of 28.
    pub(crate) fn to_decimal(&self) -> Option<Decimal> {
        let (shift, scale) = match u32::try_from(self.exponent) {
            Ok(shift) => (shift, 0),
            Err(_) => (0, u8::try_from(self.exponent.unsigned_abs()).ok().filter(|&s| s <= 28)?),
        };
        let magnitude = self.magnitude(shift, (1 << 96) - 1)?;
        Some(Decimal {
            lo: magnitude as u64,
            hi: (magnitude >> 64) as u32,
            scale: if magnitude == 0 { 0 } else { scale },
            negative: self.negative,
        })
    }

    /// Writes the value as `[-]digits[e[-]exponent]`, the plain shape `float::parse`
    /// reads, returning the length written. Zero is `0`.
    pub(crate) fn write_canonical(&self, out: &mut [u8; 350]) -> usize {
        let mut len = 0;
        let mut push = |byte: u8| {
            if let Some(slot) = out.get_mut(len) {
                *slot = byte;
                len += 1;
            }
        };
        if self.len == 0 {
            push(b'0');
            return len;
        }
        if self.negative {
            push(b'-');
        }
        for &byte in self.digits() {
            push(byte);
        }
        if self.exponent != 0 {
            push(b'e');
            if self.exponent < 0 {
                push(b'-');
            }
            let mut digits = [0u8; 40];
            let at = write_digits(u128::from(self.exponent.unsigned_abs()), &mut digits);
            for &byte in digits.get(at..).unwrap_or_default() {
                push(byte);
            }
        }
        len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::real::{MAX_NORMALIZED, Route, normalize, route};
    use crate::typed::shortest_digits;
    use crate::verdict::{CurrencySymbol, NativeDigits, Symbol, trim};
    use bigdecimal::BigDecimal;
    use bigdecimal::num_bigint::BigInt;
    use core::cmp::Ordering::{Equal, Greater, Less};
    use serde_json::Value;
    use std::format;
    use std::str::FromStr;
    use std::string::{String, ToString};
    use std::vec::Vec;

    const INVARIANT: &NumFormat = &NumFormat::INVARIANT;

    fn dec(text: &str) -> Decimal {
        crate::cast_decimal(text, INVARIANT).expect(text)
    }

    fn view(text: &str) -> Exact<'_> {
        Exact::from_text(text.as_bytes(), 0, INVARIANT, true).expect(text)
    }

    /// The canonical text of a rounded value, as `float::parse` would be handed it.
    fn canonical(rounded: &Rounded) -> String {
        let mut out = [0u8; 350];
        let len = rounded.write_canonical(&mut out);
        String::from_utf8(out[..len].to_vec()).unwrap()
    }

    /// Snaps `value` to `step` under `mode` and requires a `Rounded` equal to `expected`.
    fn rounds(value: &Exact, step: &str, mode: Rounding, expected: &str) -> Rounded {
        match snap(value, &dec(step), mode) {
            Snap::Rounded(rounded) => {
                let message = format!(
                    "{} to {step} under {mode:?}: got {}, expected {expected}",
                    render(value),
                    canonical(&rounded)
                );
                let got = BigDecimal::from_str(&canonical(&rounded)).unwrap();
                assert_eq!(got, BigDecimal::from_str(expected).unwrap(), "{message}");
                if let Ok(expected) = crate::cast_decimal(expected, INVARIANT) {
                    assert_eq!(compare(&rounded.as_exact(), &expected), Equal, "{message}");
                }
                if expected.trim_start_matches(['-', '0', '.']).is_empty() {
                    assert!(!rounded.negative, "a zero result is never negative");
                }
                rounded
            }
            other => panic!("{} to {step} under {mode:?}: {other:?}", render(value)),
        }
    }

    fn rounds_text(value: &str, step: &str, mode: Rounding, expected: &str) -> Rounded {
        rounds(&view(value), step, mode, expected)
    }

    /// The view's value written out, for messages: `-0.DIGITSeN`.
    fn render(value: &Exact) -> String {
        let mut digits = String::new();
        value.digits(&mut |digit| {
            digits.push(char::from(b'0' + digit));
            true
        });
        if digits.is_empty() {
            return "0".to_string();
        }
        let sign = if value.negative() { "-" } else { "" };
        format!("{sign}0.{digits}e{}", value.position() + 1)
    }

    /// Copies a short view's digits into a `Rounded`, so a view can be converted the way
    /// a rounded value is delivered: `write_canonical`, then `float::parse`.
    fn as_rounded(value: &Exact) -> Rounded {
        let mut rounded =
            Rounded { buf: [0; 340], len: 0, exponent: 0, negative: value.negative() };
        let mut len = 0usize;
        value.digits(&mut |digit| {
            rounded.buf[len] = b'0' + digit;
            len += 1;
            true
        });
        rounded.len = len as u16;
        rounded.exponent = (value.position() + 1 - len as i64) as i32;
        rounded
    }

    #[test]
    fn midpoint_with_an_odd_step() {
        for (value, expected) in
            [("1.4", "0"), ("1.5", "0"), ("1.6", "3"), ("-1.4", "0"), ("-1.5", "0"), ("-1.6", "-3")]
        {
            rounds_text(value, "3", Rounding::Midpoint, expected);
        }
    }

    #[test]
    fn midpoint_ties_go_to_even_quotients() {
        for (value, expected) in
            [("0.025", "0"), ("0.075", "0.10"), ("0.125", "0.10"), ("-0.075", "-0.10")]
        {
            rounds_text(value, "0.05", Rounding::Midpoint, expected);
        }
    }

    #[test]
    fn floor_and_ceiling_follow_the_number_line() {
        rounds_text("12.37", "0.05", Rounding::Floor, "12.35");
        rounds_text("12.37", "0.05", Rounding::Ceiling, "12.40");
        rounds_text("-12.37", "0.05", Rounding::Floor, "-12.40");
        rounds_text("-12.37", "0.05", Rounding::Ceiling, "-12.35");
    }

    #[test]
    fn strict_reports_not_multiple() {
        let step = dec("0.05");
        assert!(matches!(snap(&view("12.37"), &step, Rounding::Strict), Snap::NotMultiple));
        for value in ["12.35", "12.3500000", "-12.35", "0", "-0.00"] {
            for mode in [Rounding::Strict, Rounding::Midpoint, Rounding::Floor, Rounding::Ceiling] {
                assert!(matches!(snap(&view(value), &step, mode), Snap::Exact), "{value} {mode:?}");
            }
        }
    }

    #[test]
    fn latitude() {
        let step = "0.000001";
        rounds_text("45.1234567", step, Rounding::Midpoint, "45.123457");
        rounds_text("45.1234565", step, Rounding::Midpoint, "45.123456");
        rounds_text("89.9999996", step, Rounding::Midpoint, "90.000000");
        assert!(matches!(snap(&view("45.123456"), &dec(step), Rounding::Midpoint), Snap::Exact));
    }

    #[test]
    fn discarded_digits_far_past_the_step() {
        let value = format!("0.1{}1", "0".repeat(38));
        assert_eq!(value.len() - 2, 40);
        assert!(matches!(snap(&view(&value), &dec("0.1"), Rounding::Strict), Snap::NotMultiple));
        rounds_text(&value, "0.1", Rounding::Floor, "0.1");
        rounds_text(&value, "0.1", Rounding::Ceiling, "0.2");
        rounds_text(&value, "0.1", Rounding::Midpoint, "0.1");
    }

    #[test]
    fn negative_rounds_to_positive_zero() {
        let rounded = rounds_text("-0.0000004", "0.000001", Rounding::Midpoint, "0");
        assert!(!rounded.negative);
        assert_eq!(canonical(&rounded), "0");
        assert_eq!(rounded.to_i128(), Some(0));
        assert_eq!(rounded.to_decimal(), Some(Decimal { lo: 0, hi: 0, scale: 0, negative: false }));
        // Towards zero from below under ceiling, too.
        let rounded = rounds_text("-0.0000004", "0.000001", Rounding::Ceiling, "0");
        assert!(!rounded.negative);
    }

    #[test]
    fn compare_is_exact() {
        assert_eq!(compare(&view("90.0000000000000000001"), &dec("90")), Greater);
        assert_eq!(compare(&view("-90"), &dec("-90.000")), Equal);
        assert_eq!(compare(&view("1e3"), &dec("1000")), Equal);
        assert_eq!(compare(&view("-0"), &dec("0")), Equal);
        assert_eq!(compare(&view("-1"), &dec("0")), Less);
        assert_eq!(compare(&view("0.5"), &dec("-1")), Greater);
        assert_eq!(compare(&view("-2"), &dec("-1")), Less);
        assert_eq!(compare(&view("0.0999"), &dec("0.1")), Less);
        // A directly built negative zero is zero.
        assert_eq!(compare(&view("0"), &Decimal { lo: 0, hi: 0, scale: 3, negative: true }), Equal);
        assert_eq!(compare_decimals(&dec("1.10"), &dec("1.1")), Equal);
        assert_eq!(compare_decimals(&dec("-5"), &dec("3")), Less);
        assert_eq!(compare_decimals(&dec("79228162514264337593543950335"), &dec("1e28")), Greater);
    }

    #[test]
    fn decimals_write_as_canonical_text() {
        let text = |decimal: Decimal| {
            let mut out = [0u8; 48];
            let len = write_decimal_canonical(&decimal, &mut out);
            String::from_utf8(out[..len].to_vec()).unwrap()
        };
        let declared = |t: &str| crate::decimal_for_declaration(t).unwrap();
        assert_eq!(text(declared("0.1")), "1e-1");
        assert_eq!(text(declared("-90")), "-90");
        assert_eq!(text(declared("0")), "0");
        assert_eq!(text(Decimal { lo: 0, hi: 0, scale: 3, negative: true }), "0");
        assert_eq!(
            text(declared("79228162514264337593543950335")),
            "79228162514264337593543950335"
        );
        assert_eq!(text(declared("-1e-28")), "-1e-28");
        assert_eq!(text(declared("123.456")), "123456e-3");
    }

    #[test]
    fn long_input_past_normalize() {
        let value = format!("0.1{}1", "0".repeat(300));
        assert!(value.len() > MAX_NORMALIZED);
        assert!(matches!(snap(&view(&value), &dec("0.1"), Rounding::Strict), Snap::NotMultiple));
        rounds_text(&value, "0.1", Rounding::Floor, "0.1");
        // The same through the engine (grouping takes it off the plain route).
        let grouped = format!("0{}.1{}1", "", "0".repeat(300));
        let value = format!("(${grouped})");
        let format = NumFormat::INVARIANT.with_currency(CurrencySymbol::new("$").unwrap());
        let exact = Exact::from_text(value.as_bytes(), 0, &format, true).unwrap();
        rounds(&exact, "0.1", Rounding::Ceiling, "-0.1");
    }

    #[test]
    fn exponent_saturates() {
        let value = "1e-99999999999999999999999999";
        let step = dec("0.01");
        assert!(matches!(snap(&view(value), &step, Rounding::Strict), Snap::NotMultiple));
        rounds_text(value, "0.01", Rounding::Floor, "0");
        rounds_text(value, "0.01", Rounding::Ceiling, "0.01");
        assert_eq!(compare(&view(value), &dec("0")), Greater);
        assert_eq!(compare(&view(value), &step), Less);
        // And upwards: past every bound.
        assert_eq!(
            compare(&view("1e99999999999999999999999999"), &dec("79228162514264337593543950335")),
            Greater
        );
        assert_eq!(
            compare(&view("-1e99999999999999999999999999"), &dec("-79228162514264337593543950335")),
            Less
        );
    }

    #[test]
    fn mantissa_cancels_exponent() {
        let zeros = "0".repeat(1_000_000);
        let one = dec("1");
        for value in [format!("0.{zeros}1e1000001"), format!("1{zeros}e-1000000")] {
            assert_eq!(crate::cast_f64(&value, INVARIANT), Ok(1.0));
            assert_eq!(compare(&view(&value), &one), Equal);
            assert!(matches!(snap(&view(&value), &one, Rounding::Strict), Snap::Exact));
        }
        let value = format!("0.{zeros}1e999999");
        assert_eq!(crate::cast_f64(&value, INVARIANT), Ok(0.01));
        assert_eq!(compare(&view(&value), &dec("0.01")), Equal);
    }

    fn corpus(name: &str) -> Vec<Value> {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus").join(name);
        let text = std::fs::read_to_string(&path).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    /// The format a corpus vector declares (`tests/conformance.rs` reads it the same way).
    fn format_of(vector: &Value) -> NumFormat {
        let Some(format) = vector.get("format") else {
            return NumFormat::INVARIANT;
        };
        let symbol = |field: &str| -> Symbol<8> {
            format
                .get(field)
                .and_then(Value::as_str)
                .map_or(Symbol::NONE, |t| Symbol::new(t).unwrap())
        };
        let currency = format
            .get("currency")
            .and_then(Value::as_str)
            .map_or(CurrencySymbol::NONE, |t| CurrencySymbol::new(t).unwrap());
        let digits = format
            .get("digit_zero")
            .and_then(Value::as_str)
            .map_or(NativeDigits::NONE, |t| NativeDigits::new(t.chars().next().unwrap()).unwrap());
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

    fn has_radix_prefix(text: &[u8]) -> bool {
        let digits = text.strip_prefix(b"-").or(text.strip_prefix(b"+")).unwrap_or(text);
        matches!(digits, [b'0', b'x' | b'X' | b'b' | b'B', ..] | [b'&', b'h' | b'H', ..])
    }

    #[test]
    fn exact_agrees_with_the_door() {
        let mut checked = 0;
        for vector in corpus("real.json") {
            let format = format_of(&vector);
            let (text, start) = trim(vector["input"].as_str().unwrap().as_bytes());
            let ty = vector["type"].as_str().unwrap();
            let door = match ty {
                "f32" => crate::cast_f32(text, &format).map(|v| f64::from(v).to_bits()),
                _ => crate::cast_f64(text, &format).map(f64::to_bits),
            };
            let Ok(door) = door else { continue };
            let exact = Exact::from_text(text, start, &format, true)
                .expect("the view reads what the door read");
            let mut out = [0u8; 350];
            let len = as_rounded(&exact).write_canonical(&mut out);
            let viewed = match ty {
                "f32" => crate::float::parse::<f32>(&out[..len]).map(|v| f64::from(v).to_bits()),
                _ => crate::float::parse::<f64>(&out[..len]).map(f64::to_bits),
            };
            assert_eq!(
                viewed,
                Some(door),
                "{:?} as {ty}: view {}",
                vector["input"],
                render(&exact)
            );
            checked += 1;
        }
        for vector in corpus("decimal.json") {
            let format = format_of(&vector);
            let (text, start) = trim(vector["input"].as_str().unwrap().as_bytes());
            let Ok(door) = crate::cast_decimal(text, &format) else { continue };
            let exact = Exact::from_text(text, start, &format, true)
                .expect("the view reads what the door read");
            assert_eq!(
                compare(&exact, &door),
                Equal,
                "{:?}: view {}",
                vector["input"],
                render(&exact)
            );
            checked += 1;
        }
        for vector in corpus("integer.json") {
            let format = format_of(&vector);
            let (text, start) = trim(vector["input"].as_str().unwrap().as_bytes());
            let door: Result<i128, _> = match vector["type"].as_str().unwrap() {
                "i8" => crate::cast_i8(text, &format).map(i128::from),
                "i16" => crate::cast_i16(text, &format).map(i128::from),
                "i32" => crate::cast_i32(text, &format).map(i128::from),
                "i64" => crate::cast_i64(text, &format).map(i128::from),
                "u8" => crate::cast_u8(text, &format).map(i128::from),
                "u16" => crate::cast_u16(text, &format).map(i128::from),
                "u32" => crate::cast_u32(text, &format).map(i128::from),
                _ => crate::cast_u64(text, &format).map(i128::from),
            };
            let Ok(door) = door else { continue };
            let as_decimal = Decimal {
                lo: door.unsigned_abs() as u64,
                hi: (door.unsigned_abs() >> 64) as u32,
                scale: 0,
                negative: door < 0,
            };
            let mut buf = [0u8; 40];
            assert_eq!(compare(&Exact::from_integer(door, &mut buf), &as_decimal), Equal, "{door}");
            // A radix prefix is the integer doors' own grammar, outside the decimal view's.
            if !has_radix_prefix(text) {
                let exact = Exact::from_text(text, start, &format, false)
                    .expect("the view reads what the door read");
                assert_eq!(
                    compare(&exact, &as_decimal),
                    Equal,
                    "{:?}: view {}",
                    vector["input"],
                    render(&exact)
                );
            }
            checked += 1;
        }
        assert!(checked > 100, "{checked}");
    }

    #[test]
    fn route_precedence() {
        let dot = NumFormat::INVARIANT.with_currency(CurrencySymbol::new(".").unwrap());
        let half = dec("0.5");
        assert_eq!(crate::cast_decimal(".5", &dot), Ok(half));
        assert_eq!(compare(&Exact::from_text(b".5", 0, &dot, true).unwrap(), &half), Equal);
        let mut resolved = NumFormat::INVARIANT;
        assert!(matches!(route(b".5", 0, &dot, &mut resolved), Ok((Route::Plain, _))));
        assert!(matches!(route(b"$5", 0, &dot, &mut resolved), Ok((Route::Engine, _))));
        for (input, expected) in [("1.234,56", "1234.56"), ("12,5", "12.5")] {
            let expected = dec(expected);
            assert_eq!(crate::cast_decimal(input, &NumFormat::DETECT), Ok(expected));
            let exact = Exact::from_text(input.as_bytes(), 0, &NumFormat::DETECT, true).unwrap();
            assert_eq!(compare(&exact, &expected), Equal, "{input}");
        }
        // Detection resolves before the plain shape is tried: "1.234" is undecidable.
        assert!(Exact::from_text(b"1.234", 0, &NumFormat::DETECT, true).is_err());
    }

    #[test]
    fn from_shortest_carries_the_sign() {
        let digits = shortest_digits(12.37);
        let value = Exact::from_shortest(true, digits.as_ref());
        rounds(&value, "0.05", Rounding::Floor, "-12.40");
        rounds(&value, "0.05", Rounding::Ceiling, "-12.35");
        for negative in [true, false] {
            let zero = Exact::from_shortest(negative, None);
            assert!(!zero.negative());
            assert_eq!(compare(&zero, &dec("0")), Equal);
        }
        // `0.1 + 0.2` is not a multiple of a tenth, and rounds back to one.
        let digits = shortest_digits(0.1 + 0.2);
        let value = Exact::from_shortest(false, digits.as_ref());
        assert!(matches!(snap(&value, &dec("0.1"), Rounding::Strict), Snap::NotMultiple));
        let rounded = rounds(&value, "0.1", Rounding::Midpoint, "0.3");
        let mut out = [0u8; 350];
        let len = rounded.write_canonical(&mut out);
        assert_eq!(crate::float::parse::<f64>(&out[..len]), Some(0.3));
    }

    #[test]
    fn rounded_conversions() {
        let rounded = rounds_text(
            "-170141183460469231731687303715884105727.6",
            "1",
            Rounding::Floor,
            "-170141183460469231731687303715884105728",
        );
        assert_eq!(rounded.to_i128(), Some(i128::MIN));
        assert_eq!(rounded.to_decimal(), None);
        let rounded = rounds_text(
            "170141183460469231731687303715884105727.6",
            "1",
            Rounding::Ceiling,
            "170141183460469231731687303715884105728",
        );
        assert_eq!(rounded.to_i128(), None);
        let rounded = rounds_text("12.37", "0.05", Rounding::Ceiling, "12.4");
        assert_eq!(rounded.to_i128(), None);
        assert_eq!(rounded.to_decimal(), Some(dec("12.4")));
        assert_eq!(canonical(&rounded), "124e-1");
        let rounded = rounds_text("127", "5", Rounding::Ceiling, "130");
        assert_eq!(rounded.to_i128(), Some(130));
        assert_eq!(rounded.to_decimal(), Some(dec("130")));
        let mut buf = [0u8; 40];
        assert_eq!(
            compare(
                &Exact::from_integer(i128::MIN, &mut buf),
                &Decimal { lo: 0, hi: 0, scale: 0, negative: false }
            ),
            Less
        );
        let d = dec("-0.000123");
        let mut buf = [0u8; 40];
        assert_eq!(compare(&Exact::from_decimal(&d, &mut buf), &d), Equal);
    }

    /// `snap` and `compare` against `bigdecimal`, over values of up to forty digits at
    /// scales up to thirty and steps below 2^96 at scales up to twenty-eight.
    #[test]
    fn agrees_with_bigdecimal() {
        let mut state: u64 = 0x2026_1101_c0ff_ee00;
        let mut next = move || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            state >> 11
        };
        let modes = [Rounding::Strict, Rounding::Midpoint, Rounding::Floor, Rounding::Ceiling];
        let (mut ties, mut exact, mut rounded_count) = (0, 0, 0);
        for case in 0..20_000 {
            // The step: a magnitude below 2^96, of any width, at a scale up to 28.
            let wide = (u128::from(next()) << 64 | u128::from(next())) & ((1 << 96) - 1);
            let magnitude = (wide >> (next() % 96)).max(1);
            let step = Decimal {
                lo: magnitude as u64,
                hi: (magnitude >> 64) as u32,
                scale: (next() % 29) as u8,
                negative: false,
            };
            let step_big = BigDecimal::new(BigInt::from(magnitude), i64::from(step.scale));
            let negative = next() % 2 == 0;
            // The value: random digits, or a multiple of the step, or a midpoint between two.
            let text = match next() % 4 {
                0 | 1 => {
                    let len = 1 + (next() % 40) as usize;
                    let digits: String =
                        (0..len).map(|_| char::from(b'0' + (next() % 10) as u8)).collect();
                    let scale = (next() % 31) as usize;
                    if next() % 4 == 0 {
                        format!("{digits}e-{scale}")
                    } else if scale >= len {
                        format!("0.{}{digits}", "0".repeat(scale - len))
                    } else if scale == 0 {
                        digits
                    } else {
                        format!("{}.{}", &digits[..len - scale], &digits[len - scale..])
                    }
                }
                kind => {
                    let k = BigDecimal::from(next() % 1000);
                    let half = if kind == 3 {
                        BigDecimal::from_str("0.5").unwrap()
                    } else {
                        BigDecimal::from(0)
                    };
                    ((k + half) * &step_big).to_string()
                }
            };
            if text.trim_start_matches(['0', '.']).len() > 60 {
                continue;
            }
            let text = if negative { format!("-{text}") } else { text };
            let value_big = BigDecimal::from_str(&text).unwrap();
            let value = view(&text);

            // Common scale S: V and M are integers, M > 0.
            let (_, value_scale) = value_big.as_bigint_and_exponent();
            let scale = value_scale.max(i64::from(step.scale)).max(0);
            let at = |d: &BigDecimal| d.with_scale(scale).as_bigint_and_exponent().0;
            let (big_v, big_m) = (at(&value_big), at(&step_big));
            let floor = {
                let q = &big_v / &big_m;
                if &q * &big_m > big_v { q - 1 } else { q }
            };
            let remainder = &big_v - &floor * &big_m;
            let zero = BigInt::from(0);
            let mode = modes[(next() % 4) as usize];
            let quotient = if remainder == zero {
                None
            } else {
                Some(match mode {
                    Rounding::Strict => floor.clone(),
                    Rounding::Floor => floor.clone(),
                    Rounding::Ceiling => &floor + 1,
                    Rounding::Midpoint => {
                        let twice = &remainder * 2;
                        if twice < big_m {
                            floor.clone()
                        } else if twice > big_m {
                            &floor + 1
                        } else {
                            ties += 1;
                            if &floor % 2 == zero { floor.clone() } else { &floor + 1 }
                        }
                    }
                })
            };
            let got = snap(&value, &step, mode);
            match (quotient, got) {
                (None, Snap::Exact) => exact += 1,
                (Some(_), Snap::NotMultiple) if mode == Rounding::Strict => {}
                (Some(q), Snap::Rounded(rounded)) if mode != Rounding::Strict => {
                    rounded_count += 1;
                    let expected = BigDecimal::new(q * &big_m, scale);
                    let got = BigDecimal::from_str(&canonical(&rounded)).unwrap();
                    assert_eq!(got, expected, "case {case}: {text} to {step} under {mode:?}");
                    assert_eq!(
                        rounded.negative,
                        expected.sign() == bigdecimal::num_bigint::Sign::Minus,
                        "case {case}: sign"
                    );
                    if let Ok(expected) = crate::cast_decimal(expected.to_string(), INVARIANT) {
                        assert_eq!(compare(&rounded.as_exact(), &expected), Equal, "case {case}");
                    }
                }
                (expected, got) => panic!(
                    "case {case}: {text} to {step} under {mode:?}: expected {expected:?}, got {got:?}"
                ),
            }
            // Compare against the step and against a signed random bound.
            let ordering = value_big.cmp(&step_big);
            assert_eq!(compare(&value, &step), ordering, "case {case}: {text} against {step}");
            let bound = Decimal { negative: next() % 2 == 0, ..step };
            let bound_big = if bound.negative { -step_big.clone() } else { step_big.clone() };
            assert_eq!(
                compare(&value, &bound),
                value_big.cmp(&bound_big),
                "case {case}: {text} against {bound}"
            );
        }
        assert!(
            ties > 100 && exact > 1000 && rounded_count > 5000,
            "{ties} {exact} {rounded_count}"
        );
    }

    /// `normalize`'s output bytes for inputs whose exponent and grouping shapes matter,
    /// fixed here as literals so that a change to the grammar it streams cannot pass by
    /// changing both sides of the comparison at once.
    #[test]
    fn normalize_output_is_pinned() {
        let comma = NumFormat::new(',', '.', NumFormat::ALL);
        let arabic =
            NumFormat::INVARIANT.with_digits(NativeDigits::new('\u{0660}').expect("digits"));
        let nines = format!("1e-{}", "9".repeat(40));
        let cases: &[(&str, &NumFormat, &str, bool)] = &[
            ("1e+003", &NumFormat::INVARIANT, "1e+003", false),
            ("1.234,5", &comma, "1234.5", false),
            ("(1,234.50)", &NumFormat::INVARIANT, "-1234.50", false),
            (
                "1E-0000000000000000000000000000000000000000007",
                &NumFormat::INVARIANT,
                "1e-0000000000000000000000000000000000000000007",
                false,
            ),
            (&nines, &NumFormat::INVARIANT, &nines, false),
            ("1e9", &NumFormat::INVARIANT, "1e9", false),
            ("12.5%", &NumFormat::INVARIANT, "12.5", true),
            ("(2.5)%", &NumFormat::INVARIANT, "-2.5", true),
            ("-.5", &NumFormat::INVARIANT, "-.5", false),
            ("\u{0661}\u{0662}.\u{0663}e\u{0664}", &arabic, "12.3e4", false),
        ];
        for &(input, format, expected, percent) in cases {
            let (text, start) = trim(input.as_bytes());
            let mut buf = [0u8; MAX_NORMALIZED];
            let (len, got_percent) = normalize(text, start, format, &mut buf).expect(input);
            assert_eq!(core::str::from_utf8(&buf[..len]).unwrap(), expected, "{input}");
            assert_eq!(got_percent, percent, "{input}");
        }
        // Past the buffer, the whole token is malformed, as it always was.
        let long = "1".repeat(MAX_NORMALIZED + 1);
        let (text, start) = trim(long.as_bytes());
        let mut buf = [0u8; MAX_NORMALIZED];
        let fault = normalize(text, start, &NumFormat::INVARIANT, &mut buf).unwrap_err();
        assert_eq!((fault.offset, fault.len), (0, MAX_NORMALIZED as u32 + 1));
        // A token that fills it exactly is still read.
        let full = "1".repeat(MAX_NORMALIZED);
        let (len, _) = normalize(full.as_bytes(), 0, &NumFormat::INVARIANT, &mut buf).unwrap();
        assert_eq!(len, MAX_NORMALIZED);
        // A malformed byte ahead of the overflow is reported at itself.
        let fault = normalize(b"1x1", 0, &NumFormat::INVARIANT, &mut buf).unwrap_err();
        assert_eq!((fault.offset, fault.len), (1, 1));
    }
}
