//! The lenient fast lane, shared by the integer, real and decimal doors.
//!
//! Each of those doors already had one fast path, for a token in the plain invariant shape,
//! and one full engine for everything else. "Everything else" turned out to include the
//! shapes money actually arrives in — `12,345.67`, `$12,345.67`, `($12,345.67)` — and the
//! engine cost them double: 41 ns against 9 for an i64, 47 against 25 for a decimal. Not
//! because of the currency symbol; grouping alone cost as much as grouping, symbol and
//! parentheses together. The price was leaving the plain path at all.
//!
//! This is a second lane for exactly those shapes: accounting parentheses, a sign, the
//! declared currency symbol at either edge, grouped digits and a fraction, read in one
//! pass with no scratch buffer for the integer and decimal doors. It reads success shapes
//! only. Anything it does not recognise — whitespace inside the token, an exponent, a
//! percent, a multi-byte separator, a misplaced separator, more digits than the door's
//! accumulator holds — returns `None`, and the door runs the engine it always ran, which
//! rescans from the top and owns every fault span. So the lane can make a cast faster and
//! cannot change a verdict: `lib.rs`'s differential test holds every door to the engine's
//! answer over an enumerated grammar of token sequences, with the lane on and off.
//!
//! It mirrors the engine's order of decisions rather than restating its grammar, because
//! the order is where the corner cases live: parentheses come off before the currency
//! symbol; a sign is read before a leading symbol and again after one; a leading symbol is
//! tried before a trailing one, and only one of them is taken; a digit is a digit before it
//! is anything else, and the decimal separator is tried before the group separator.

use crate::integer::is_digit_at;
use crate::verdict::NumFormat;

/// Where the lane hands what it reads. One per door family: the integer door accumulates a
/// value and refuses a decimal point, the decimal door accumulates a magnitude and counts
/// fraction digits, the real doors accumulate a significand for `float::from_significand`.
pub(crate) trait Sink {
    /// One digit, as its value `0..=9`. `false` takes the token out of the lane.
    fn digit(&mut self, digit: u8) -> bool;
    /// The decimal separator. `false` takes the token out of the lane.
    fn point(&mut self) -> bool;
}

/// Reads the trimmed, non-empty `text` under `format` into `sink`. `Some(negative)` when
/// the whole token is one of the lane's shapes; `None` to fall through to the full engine,
/// in which case whatever the sink was handed is discarded.
///
/// `percent_door` is whether the calling door reads a trailing `%` as "divide by 100" —
/// the real and decimal doors do, the integer doors do not. Where it does, a token ending
/// in `%` is the engine's: it strips the percent before anything else, so a declared
/// currency symbol of `%` must not get to claim that byte first.
#[inline(always)]
pub(crate) fn scan<S: Sink>(
    text: &[u8],
    format: &NumFormat,
    percent_door: bool,
    sink: &mut S,
) -> Option<bool> {
    // A separator outside ASCII is a multi-byte match; the engine has the machinery for it.
    if !format.decimal_sep.is_ascii() || !format.group_sep.is_ascii() {
        return None;
    }
    let decimal = format.decimal_sep as u8;
    let group = format.group_sep as u8;
    let grouping = format.allows(NumFormat::GROUPING);
    if percent_door && text.last() == Some(&b'%') && format.allows(NumFormat::PERCENT) {
        return None;
    }

    let mut body = text;
    let mut negative = false;
    let parens = format.allows(NumFormat::PARENS) && text.first() == Some(&b'(');
    if parens {
        // Whitespace just inside the parentheses is legal and the engine trims it; here it
        // simply fails the scan below, like any other byte the lane does not read. `()`
        // leaves an empty body, which reads no digit and falls through.
        let [_, inner @ .., b')'] = text else {
            return None;
        };
        body = inner;
        negative = true;
    }

    let mut i = 0;
    let mut signed = false;
    if let Some(&sign @ (b'+' | b'-')) = body.first() {
        // A sign inside accounting parentheses is double negation; the engine faults it.
        if parens {
            return None;
        }
        signed = true;
        negative = sign == b'-';
        i = 1;
    }
    if format.allows(NumFormat::CURRENCY) && !format.currency.is_empty() {
        // One byte decides almost every token, so the comparison proper (a call into
        // memcmp, three nanoseconds a time) only runs where the symbol could be.
        let symbol = format.currency.as_bytes();
        let leads = body.get(i) == symbol.first()
            && (symbol.len() == 1 || body.get(i..).is_some_and(|rest| rest.starts_with(symbol)));
        if leads {
            i += symbol.len();
            // `$-5` is fine; `-$-5` and `($-5)` are not.
            if let Some(&sign @ (b'+' | b'-')) = body.get(i) {
                if signed || parens {
                    return None;
                }
                negative = sign == b'-';
                i += 1;
            }
        } else if body.last() == symbol.last() && (symbol.len() == 1 || body.ends_with(symbol)) {
            // The trailing symbol is cut off the body, so the loops below stop short of it.
            body = body.get(..body.len() - symbol.len()).unwrap_or_default();
        }
    }

    // Two loops rather than one with a flag: the integer part, where a group separator may
    // sit between two digits, then the fraction, where nothing but digits may follow. A
    // digit is a digit before it is anything else, and the decimal separator is tried
    // before the group separator — the engine's order, which is what decides a format
    // whose separators are digits or each other.
    let mut any_digit = false;
    let mut after_digit = false;
    while let Some(&byte) = body.get(i) {
        let digit = byte.wrapping_sub(b'0');
        if digit <= 9 {
            if !sink.digit(digit) {
                return None;
            }
            any_digit = true;
            after_digit = true;
        } else if byte == decimal {
            break;
        } else if grouping && byte == group && after_digit && is_digit_at(body, i + 1) {
            after_digit = false;
        } else {
            return None;
        }
        i += 1;
    }
    if i < body.len() {
        if !sink.point() {
            return None;
        }
        i += 1;
        while let Some(&byte) = body.get(i) {
            let digit = byte.wrapping_sub(b'0');
            if digit > 9 || !sink.digit(digit) {
                return None;
            }
            any_digit = true;
            i += 1;
        }
    }
    if any_digit { Some(negative) } else { None }
}

/// The integer doors' sink: up to nineteen digits, which cannot overflow a `u64`, and no
/// decimal point — a point on an integer is the engine's fault to report.
#[derive(Default)]
pub(crate) struct Integer {
    pub(crate) value: u64,
    digits: u32,
}

impl Sink for Integer {
    #[inline]
    fn digit(&mut self, digit: u8) -> bool {
        if self.digits == 19 {
            return false;
        }
        self.value = self.value * 10 + u64::from(digit);
        self.digits += 1;
        true
    }

    #[inline]
    fn point(&mut self) -> bool {
        false
    }
}

/// The decimal door's sink: up to twenty-eight digits in all, which is below both limits a
/// [`Decimal`](crate::Decimal) has — 10²⁸ is under 2⁹⁶, and a scale cannot exceed the digit
/// count — so nothing read here can be out of range and nothing needs a checked operation.
/// The first nineteen digits accumulate in a `u64`; only a longer literal pays for `u128`.
#[derive(Default)]
pub(crate) struct Exact {
    narrow: u64,
    wide: u128,
    digits: u32,
    pub(crate) fraction_digits: u32,
    seen_point: bool,
}

impl Exact {
    pub(crate) fn magnitude(&self) -> u128 {
        if self.digits <= 19 { u128::from(self.narrow) } else { self.wide }
    }
}

impl Sink for Exact {
    #[inline]
    fn digit(&mut self, digit: u8) -> bool {
        if self.digits < 19 {
            self.narrow = self.narrow * 10 + u64::from(digit);
        } else if self.digits < 28 {
            if self.digits == 19 {
                self.wide = u128::from(self.narrow);
            }
            self.wide = self.wide * 10 + u128::from(digit);
        } else {
            return false;
        }
        self.digits += 1;
        self.fraction_digits += u32::from(self.seen_point);
        true
    }

    #[inline]
    fn point(&mut self) -> bool {
        self.seen_point = true;
        true
    }
}

/// The real doors' sink: the significand and the count of fraction digits, for
/// [`float::from_significand`](crate::float::from_significand). Nineteen significant digits
/// at most, which always fit a `u64`; zeros ahead of the first nonzero digit are not
/// significant and are only counted. A longer literal is left to the engine.
///
/// This replaced a sink that copied the digits into a text buffer for `float::parse` to
/// read again: the byte-at-a-time copy read back as eight-byte words was a store the CPU
/// could not forward, and the second scan cost as much as the first.
#[derive(Default)]
pub(crate) struct Significand {
    pub(crate) value: u64,
    digits: u32,
    pub(crate) fraction_digits: u32,
    seen_point: bool,
}

impl Sink for Significand {
    #[inline]
    fn digit(&mut self, digit: u8) -> bool {
        if self.value != 0 || digit != 0 {
            if self.digits == 19 {
                return false;
            }
            self.value = self.value * 10 + u64::from(digit);
            self.digits += 1;
        }
        self.fraction_digits += u32::from(self.seen_point);
        true
    }

    #[inline]
    fn point(&mut self) -> bool {
        self.seen_point = true;
        true
    }
}
