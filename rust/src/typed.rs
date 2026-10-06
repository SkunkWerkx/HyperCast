//! The typed doors: the scalars a number already held as an `f64` names. A workbook reader
//! has the double an XLSX `<v>` or an ODS `office:value` stores and no text worth parsing
//! again, so each door here is the twin of a text door, under the same rules wherever a
//! double can meet them.
//!
//! - The number a double names is the shortest decimal that names it and no other — the
//!   digits a spreadsheet writes for it, and what any shortest round-trip formatter prints
//!   — so `0.1` is one tenth and the sum `0.1 + 0.2` is `0.30000000000000004`. It depends
//!   on the value alone, never on which writer spelled it, and every door below reads that
//!   decimal, so each agrees with its text twin read on the shortest text.
//! - A decimal is that number exactly. Past 96 bits or 28 places is `OutOfRange`, as in the
//!   text door, which never drops a nonzero digit either.
//! - An integer is that number when it is whole, and in range, or nothing. No rounding —
//!   `2.5` is `Malformed` for an `i32`, as the text `2.5` is. Below 2^53 that is the
//!   double's own value; above it, where a double no longer holds every integer, it is the
//!   shortest digits padded with zeros (`2^63` is `9223372036854776000`, as Excel shows it).
//! - NaN is `Malformed` everywhere (no text door spells one); an infinity is `OutOfRange`.
//!
//! The verdict is a bare [`Reason`]: there is no text for a [`crate::Fault`]'s span to index.
//!
//! The shortest decimal is worked out exactly, in fixed-size integers on the stack (Steele
//! and White's algorithm as `core` carries it for its own fallback), because `core`'s
//! formatting machinery cannot be put under the no-panic proof.

use crate::verdict::{Decimal, Reason};
use core::cmp::Ordering;

/// The largest magnitude a [`Decimal`] carries: 2⁹⁶ − 1.
const MAX_MAGNITUDE: u128 = (1u128 << 96) - 1;
/// The largest scale a [`Decimal`] carries.
const MAX_SCALE: u32 = 28;

/// True for a finite double with no fractional part.
fn is_integral(value: f64) -> bool {
    if !value.is_finite() {
        return false;
    }
    // Every double from 2^52 up is an integer; below it the round trip through i64 is exact.
    value.abs() >= 4_503_599_627_370_496.0 || (value as i64) as f64 == value
}

/// The whole number a double names, as an `i128`: non-integral (NaN and ∞ among them) is
/// `Malformed`, past the `i128` range is `OutOfRange`. Below 2^53 in magnitude it is the
/// double's own value; from there up it is the shortest decimal's, zeros and all.
pub(crate) fn whole(value: f64) -> Result<i128, Reason> {
    if !is_integral(value) {
        return Err(Reason::Malformed);
    }
    if value.abs() < 9_007_199_254_740_992.0 {
        return Ok(i128::from(value as i64));
    }
    let shortest = shortest_digits(value).ok_or(Reason::Malformed)?;
    let digits = shortest.digits();
    let mut size = digits.iter().fold(0i128, |sum, &digit| sum * 10 + i128::from(digit - b'0'));
    // An integral double from 2^53 up has no more digits than its exponent.
    for _ in digits.len()..usize::try_from(shortest.exponent()).unwrap_or(0) {
        size = size.checked_mul(10).ok_or(Reason::OutOfRange)?;
    }
    Ok(if value < 0.0 { -size } else { size })
}

macro_rules! integer_door {
    ($door:ident, $ty:ty, $text:ident) => {
        #[doc = concat!(
            "Reads a number already held as an `f64` as an `", stringify!($ty), "` — the typed ",
            "twin of [`", stringify!($text), "`](crate::", stringify!($text), "). An integral ",
            "value in range is itself; a fractional, NaN or infinite one is `Malformed`, ",
            "never rounded; an integral one past the type's range is `OutOfRange`."
        )]
        pub fn $door(value: f64) -> Result<$ty, Reason> {
            <$ty>::try_from(whole(value)?).map_err(|_| Reason::OutOfRange)
        }
    };
}

integer_door!(i8_from_f64, i8, cast_i8);
integer_door!(i16_from_f64, i16, cast_i16);
integer_door!(i32_from_f64, i32, cast_i32);
integer_door!(i64_from_f64, i64, cast_i64);
integer_door!(u8_from_f64, u8, cast_u8);
integer_door!(u16_from_f64, u16, cast_u16);
integer_door!(u32_from_f64, u32, cast_u32);
integer_door!(u64_from_f64, u64, cast_u64);

/// Narrows a number already held as an `f64` to an `f32` — the typed twin of
/// [`cast_f32`](crate::cast_f32): the `f32` nearest the double, ties to even. NaN is
/// `Malformed`; a value whose nearest `f32` is infinite (an infinity among them) is
/// `OutOfRange`.
pub fn f32_from_f64(value: f64) -> Result<f32, Reason> {
    if value.is_nan() {
        return Err(Reason::Malformed);
    }
    let narrowed = value as f32;
    if narrowed.is_infinite() { Err(Reason::OutOfRange) } else { Ok(narrowed) }
}

/// Reads a number already held as an `f64` as a boolean — the typed twin of
/// [`cast_bool`](crate::cast_bool), whose lexicon has `1` and `0` and no other number:
/// exactly `1` is true, exactly `0` (either sign) is false, and anything else is
/// `Malformed`.
pub fn bool_from_f64(value: f64) -> Result<bool, Reason> {
    if value == 1.0 {
        Ok(true)
    } else if value == 0.0 {
        Ok(false)
    } else {
        Err(Reason::Malformed)
    }
}

/// Reads a number already held as an `f64` as an exact [`Decimal`] — the typed twin of
/// [`cast_decimal`](crate::cast_decimal). The decimal is the shortest that names the double
/// and no other ([`shortest_digits`]), so the double a workbook stores for `2.5` is `2.5`
/// and the one it stores for `0.1` is one tenth, not the binary fraction nearest it; the
/// result is canonical as the text door's is. NaN is `Malformed`; an infinity, a magnitude
/// past 2⁹⁶ − 1 or more than 28 places is `OutOfRange` — the digits are never cut to fit.
pub fn decimal_from_f64(value: f64) -> Result<Decimal, Reason> {
    if value.is_nan() {
        return Err(Reason::Malformed);
    }
    if value.is_infinite() {
        return Err(Reason::OutOfRange);
    }
    let negative = value.is_sign_negative();
    let Some(shortest) = shortest_digits(value) else {
        // Zero, of either sign, and zero is never negative.
        return Ok(Decimal { lo: 0, hi: 0, scale: 0, negative: false });
    };
    let digits = shortest.digits();
    // Seventeen digits at most, so the integer they spell fits a u64 several times over.
    let mut magnitude =
        digits.iter().fold(0u128, |sum, &digit| sum * 10 + u128::from(digit - b'0'));
    // The value is `magnitude × 10^(exponent − len)`.
    let shift = i32::from(shortest.exponent()) - digits.len() as i32;
    let scale = if shift >= 0 {
        for _ in 0..shift {
            magnitude = magnitude.checked_mul(10).ok_or(Reason::OutOfRange)?;
        }
        0
    } else {
        shift.unsigned_abs()
    };
    if magnitude > MAX_MAGNITUDE || scale > MAX_SCALE {
        return Err(Reason::OutOfRange);
    }
    // The digits never end in a zero, so with a positive scale this is already canonical.
    Ok(Decimal { lo: magnitude as u64, hi: (magnitude >> 64) as u32, scale: scale as u8, negative })
}

/// The shortest run of decimal digits that names a double and no other, nearest to it
/// where several are as short: `0.d₁d₂…dₙ × 10^exponent`, with `d₁` never zero and `dₙ`
/// never zero. At most seventeen digits. The sign is not part of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShortestDigits {
    digits: [u8; 20],
    len: u8,
    exponent: i16,
}

impl ShortestDigits {
    /// The digits, as ASCII.
    pub fn digits(&self) -> &[u8] {
        self.digits.get(..usize::from(self.len)).unwrap_or_default()
    }

    /// The power of ten the digits are read under, after an implied leading point:
    /// `0.25` is digits `25` and exponent `0`, `2.5` is `25` and `1`, `2500` is `25` and `4`.
    pub fn exponent(&self) -> i16 {
        self.exponent
    }
}

/// The shortest decimal digits of a finite, nonzero double's magnitude, or `None` for zero,
/// NaN and the infinities — what a caller rendering a double as text builds on, under the
/// same rule [`decimal_from_f64`] reads it by.
pub fn shortest_digits(value: f64) -> Option<ShortestDigits> {
    if !value.is_finite() || value == 0.0 {
        return None;
    }
    let mut digits = [b'0'; 20];
    let (mut count, exponent) = shortest(value.abs(), &mut digits);
    // A carry through all nines leaves zeros behind a single `1`.
    while count > 1 && digits.get(count - 1) == Some(&b'0') {
        count -= 1;
    }
    Some(ShortestDigits { digits, len: count as u8, exponent })
}

/// An unsigned integer of 1280 bits in 32-bit digits, least significant first — room for
/// the largest power of two and of ten a double's shortest decimal is worked out against.
#[derive(Clone, Copy)]
struct Big {
    size: usize,
    base: [u32; Big::DIGITS],
}

impl Big {
    const DIGITS: usize = 40;

    fn from_u64(value: u64) -> Big {
        let mut base = [0u32; Big::DIGITS];
        if let Some(pair) = base.first_chunk_mut::<2>() {
            *pair = [value as u32, (value >> 32) as u32];
        }
        let size = match value {
            0 => 0,
            v if v >> 32 == 0 => 1,
            _ => 2,
        };
        Big { size, base }
    }

    fn mul_small(&mut self, factor: u32) {
        let mut carry = 0u64;
        for digit in self.base.iter_mut().take(self.size) {
            let wide = u64::from(*digit) * u64::from(factor) + carry;
            *digit = wide as u32;
            carry = wide >> 32;
        }
        if carry > 0
            && let Some(top) = self.base.get_mut(self.size)
        {
            *top = carry as u32;
            self.size += 1;
        }
    }

    fn mul_pow2(&mut self, mut bits: u32) {
        while bits >= 31 {
            self.mul_small(1 << 31);
            bits -= 31;
        }
        self.mul_small(1 << bits);
    }

    fn mul_pow10(&mut self, mut power: u32) {
        while power >= 9 {
            self.mul_small(1_000_000_000);
            power -= 9;
        }
        self.mul_small(10u32.pow(power));
    }

    fn add(&mut self, other: &Big) {
        let size = self.size.max(other.size);
        let mut carry = false;
        for (digit, &more) in self.base.iter_mut().zip(other.base.iter()).take(size) {
            let (sum, first) = digit.overflowing_add(more);
            let (sum, second) = sum.overflowing_add(u32::from(carry));
            *digit = sum;
            carry = first || second;
        }
        self.size = size;
        if carry && let Some(top) = self.base.get_mut(size) {
            *top = 1;
            self.size += 1;
        }
    }

    /// `self -= other`, for `other` no greater than `self`.
    fn sub(&mut self, other: &Big) {
        let mut borrow = false;
        for (digit, &less) in self.base.iter_mut().zip(other.base.iter()).take(self.size) {
            let (rest, first) = digit.overflowing_sub(less);
            let (rest, second) = rest.overflowing_sub(u32::from(borrow));
            *digit = rest;
            borrow = first || second;
        }
        while self.size > 0 && self.base.get(self.size - 1) == Some(&0) {
            self.size -= 1;
        }
    }

    fn cmp(&self, other: &Big) -> Ordering {
        let size = self.size.max(other.size);
        for (mine, theirs) in self.base.iter().zip(other.base.iter()).take(size).rev() {
            match mine.cmp(theirs) {
                Ordering::Equal => {}
                unequal => return unequal,
            }
        }
        Ordering::Equal
    }
}

/// The shortest run of decimal digits that names the positive finite double `value` and no
/// other, nearest to it where several are as short. Returns how many digits were written
/// and the exponent `k` in `0.d₁d₂… × 10ᵏ`.
fn shortest(value: f64, digits: &mut [u8; 20]) -> (usize, i16) {
    let bits = value.to_bits();
    let fraction = bits & ((1 << 52) - 1);
    let biased = ((bits >> 52) & 0x7FF) as i16;
    // The double is `mant × 2^exp`, and its neighbours are `minus` below and `plus` above in
    // the same unit. A power of two has a nearer neighbour below than above.
    let (mant, plus, exp) = if biased == 0 {
        (fraction << 1, 1u64, -1075i16)
    } else if fraction == 0 {
        (1u64 << 54, 2, biased - 1077)
    } else {
        ((fraction | 1 << 52) << 1, 1, biased - 1076)
    };
    // (`core` reads a subnormal's mantissa already doubled, and so always even.)
    let even = biased == 0 || (fraction & 1) == 0;
    // With an even mantissa the bounds themselves round to this double.
    let within =
        |order: Ordering| if even { order != Ordering::Greater } else { order == Ordering::Less };

    let width = 64 - i64::from((mant + plus - 1).leading_zeros());
    let mut k = (((width + i64::from(exp)) * 1_292_913_986) >> 32) as i16;

    let mut mant = Big::from_u64(mant);
    let mut minus = Big::from_u64(1);
    let mut plus = Big::from_u64(plus);
    let mut scale = Big::from_u64(1);
    if exp < 0 {
        scale.mul_pow2(u32::from(exp.unsigned_abs()));
    } else {
        mant.mul_pow2(u32::from(exp.unsigned_abs()));
        minus.mul_pow2(u32::from(exp.unsigned_abs()));
        plus.mul_pow2(u32::from(exp.unsigned_abs()));
    }
    if k >= 0 {
        scale.mul_pow10(u32::from(k.unsigned_abs()));
    } else {
        mant.mul_pow10(u32::from(k.unsigned_abs()));
        minus.mul_pow10(u32::from(k.unsigned_abs()));
        plus.mul_pow10(u32::from(k.unsigned_abs()));
    }

    // The estimate of `k` is at most one short.
    let mut high = mant;
    high.add(&plus);
    if within(scale.cmp(&high)) {
        k += 1;
    } else {
        mant.mul_small(10);
        minus.mul_small(10);
        plus.mul_small(10);
    }

    let mut scale2 = scale;
    scale2.mul_small(2);
    let mut scale4 = scale;
    scale4.mul_small(4);
    let mut scale8 = scale;
    scale8.mul_small(8);

    let mut count = 0usize;
    let (mut down, mut up) = (false, false);
    while count < 18 {
        // One digit: `mant / scale`, which is below ten.
        let mut digit = 0u8;
        for (step, weight) in [(&scale8, 8u8), (&scale4, 4), (&scale2, 2), (&scale, 1)] {
            if mant.cmp(step) != Ordering::Less {
                mant.sub(step);
                digit += weight;
            }
        }
        if let Some(slot) = digits.get_mut(count) {
            *slot = b'0' + digit;
        }
        count += 1;
        // Stop as soon as what is left can no longer be told from a neighbour.
        down = within(mant.cmp(&minus));
        let mut high = mant;
        high.add(&plus);
        up = within(scale.cmp(&high));
        if down || up {
            break;
        }
        mant.mul_small(10);
        minus.mul_small(10);
        plus.mul_small(10);
    }

    // Stopped between two candidates: the upper one, unless the lower is nearer.
    if up
        && (!down || {
            mant.mul_small(2);
            mant.cmp(&scale) != Ordering::Less
        })
    {
        let written = digits.get_mut(..count).unwrap_or_default();
        match written.iter().rposition(|&d| d != b'9') {
            Some(at) => {
                for (index, digit) in written.iter_mut().enumerate().skip(at) {
                    *digit = if index == at { *digit + 1 } else { b'0' };
                }
            }
            None => {
                // All nines: one more digit, and one more power of ten.
                for (index, digit) in written.iter_mut().enumerate() {
                    *digit = if index == 0 { b'1' } else { b'0' };
                }
                if let Some(slot) = digits.get_mut(count) {
                    *slot = b'0';
                }
                count += 1;
                k += 1;
            }
        }
    }
    (count, k)
}
