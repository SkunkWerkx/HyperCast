//! Decimal text to the nearest `f32`/`f64` — the conversion the real doors hand their
//! normalized digits to, written here so that it can be proven unable to panic. `core`'s
//! own parser reads the same text to the same bits, but keeps slice-index checks the
//! optimizer cannot discharge, and those were the one panic path left in the C ABI.
//!
//! Three steps, each exact or declining:
//!
//! 1. Clinger's fast path. A significand that fits the mantissa and a power of ten that is
//!    itself exact make one float multiplication or division, correctly rounded by the
//!    hardware.
//! 2. Eisel-Lemire. The first 19 significant digits times a 128-bit power of five
//!    (`float_table.rs`) gives the mantissa with enough spare bits to round — or says it
//!    cannot tell, which happens only near a halfway point. When digits were dropped past
//!    the 19th, it is run for the kept digits and for one more in the last place; if both
//!    land on the same float, the dropped digits could not have mattered.
//! 3. Exact division. What neither settles is decided with integers: up to 768
//!    significant digits (a float's halfway points have at most 767; a nonzero remainder is
//!    folded into one sticky digit) as a ratio of two fixed-size big integers, divided for
//!    a 64-bit quotient and a remainder, then rounded to nearest, ties to even.
//!
//! The first two are the algorithms `core` runs. The third is not `core`'s, which shifts a
//! decimal digit string instead; this one is slower and has nothing in it to get wrong,
//! which is the right trade for a path ordinary data never reaches. `tests` holds all
//! three to `core`'s answer, bit for bit.
//!
//! Nothing here indexes a slice or divides by a variable. The big integers are fixed
//! arrays on the stack, so the doors still allocate nothing.

use crate::float_table::{POWER_OF_FIVE_128, SMALLEST_POWER_OF_FIVE};
use core::cmp::Ordering;

/// What the conversion needs to know about a float type.
pub(crate) trait Real: Copy + core::ops::Neg<Output = Self> {
    /// Explicit mantissa bits: 52 for `f64`, 23 for `f32`.
    const SIG_BITS: u32;
    /// The exponent bias: 1023 for `f64`, 127 for `f32`.
    const BIAS: i32;
    /// The biased exponent of infinity.
    const INFINITE_POWER: i32;
    /// Below this decimal exponent a 64-bit significand is zero in this type.
    const SMALLEST_POWER_OF_TEN: i64;
    /// Above this decimal exponent a nonzero significand is infinite in this type.
    const LARGEST_POWER_OF_TEN: i64;
    /// The decimal exponents between which a product can sit exactly halfway.
    const MIN_EXPONENT_ROUND_TO_EVEN: i64;
    /// See [`Real::MIN_EXPONENT_ROUND_TO_EVEN`].
    const MAX_EXPONENT_ROUND_TO_EVEN: i64;

    /// The float with these bits (the low 32 for `f32`).
    fn from_bits64(bits: u64) -> Self;
    /// `w * 10^q` when both `w` and `10^|q|` are exact in this type, so that the one
    /// multiplication or division is correctly rounded.
    fn exact(w: u64, q: i64) -> Option<Self>;
    /// Positive zero.
    fn zero() -> Self;
}

impl Real for f64 {
    const SIG_BITS: u32 = 52;
    const BIAS: i32 = 1023;
    const INFINITE_POWER: i32 = 0x7FF;
    const SMALLEST_POWER_OF_TEN: i64 = -342;
    const LARGEST_POWER_OF_TEN: i64 = 308;
    const MIN_EXPONENT_ROUND_TO_EVEN: i64 = -4;
    const MAX_EXPONENT_ROUND_TO_EVEN: i64 = 23;

    fn from_bits64(bits: u64) -> f64 {
        f64::from_bits(bits)
    }

    fn exact(w: u64, q: i64) -> Option<f64> {
        const POW10: [f64; 23] = [
            1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15,
            1e16, 1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
        ];
        if w > 1 << 53 {
            return None;
        }
        let power = *POW10.get(usize::try_from(q.unsigned_abs()).ok()?)?;
        Some(if q < 0 { w as f64 / power } else { w as f64 * power })
    }

    fn zero() -> f64 {
        0.0
    }
}

impl Real for f32 {
    const SIG_BITS: u32 = 23;
    const BIAS: i32 = 127;
    const INFINITE_POWER: i32 = 0xFF;
    const SMALLEST_POWER_OF_TEN: i64 = -65;
    const LARGEST_POWER_OF_TEN: i64 = 38;
    const MIN_EXPONENT_ROUND_TO_EVEN: i64 = -17;
    const MAX_EXPONENT_ROUND_TO_EVEN: i64 = 10;

    fn from_bits64(bits: u64) -> f32 {
        f32::from_bits(bits as u32)
    }

    fn exact(w: u64, q: i64) -> Option<f32> {
        const POW10: [f32; 11] = [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10];
        if w > 1 << 24 {
            return None;
        }
        let power = *POW10.get(usize::try_from(q.unsigned_abs()).ok()?)?;
        Some(if q < 0 { w as f32 / power } else { w as f32 * power })
    }

    fn zero() -> f32 {
        0.0
    }
}

/// A float as its mantissa field and biased exponent field, not yet packed.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Fields {
    mantissa: u64,
    power: i32,
}

impl Fields {
    const ZERO: Fields = Fields { mantissa: 0, power: 0 };

    fn infinite<T: Real>() -> Fields {
        Fields { mantissa: 0, power: T::INFINITE_POWER }
    }

    fn pack<T: Real>(self) -> T {
        T::from_bits64(self.mantissa | (self.power as u64) << T::SIG_BITS)
    }
}

/// Significant digits kept in the 64-bit significand: the most that always fit.
const KEPT_DIGITS: u32 = 19;
/// Where an explicit exponent stops accumulating. Far past anything finite, far short of
/// overflow, and more digits than any text has.
const EXPONENT_CEILING: i64 = 1_000_000_000_000_000;

fn digits_of(text: &[u8]) -> (&[u8], &[u8]) {
    let count = text.iter().take_while(|byte| byte.is_ascii_digit()).count();
    text.split_at_checked(count).unwrap_or((text, &[]))
}

/// True when all eight bytes of `chunk`, read little-endian, are ASCII digits.
fn is_8digits(chunk: u64) -> bool {
    let above = chunk.wrapping_add(0x4646_4646_4646_4646);
    let below = chunk.wrapping_sub(0x3030_3030_3030_3030);
    (above | below) & 0x8080_8080_8080_8080 == 0
}

/// The value of eight ASCII digits read little-endian: pairs, then fours, then all eight,
/// three multiplications instead of eight.
fn parse_8digits(chunk: u64) -> u64 {
    const MASK: u64 = 0x0000_00FF_0000_00FF;
    const MUL1: u64 = 0x000F_4240_0000_0064;
    const MUL2: u64 = 0x0000_2710_0000_0001;
    let chunk = chunk.wrapping_sub(0x3030_3030_3030_3030);
    let chunk = chunk.wrapping_mul(10).wrapping_add(chunk >> 8);
    let low = (chunk & MASK).wrapping_mul(MUL1);
    let high = ((chunk >> 16) & MASK).wrapping_mul(MUL2);
    u64::from((low.wrapping_add(high) >> 32) as u32)
}

/// Reads the run of digits at the front of `text` into `w` — wrapping, so the caller
/// decides from the count whether `w` is the whole run — and returns the run and what
/// follows it.
#[inline(always)]
fn accumulate<'t>(text: &'t [u8], w: &mut u64) -> (&'t [u8], &'t [u8]) {
    let mut rest = text;
    while let Some((chunk, after)) = rest.split_first_chunk::<8>() {
        let chunk = u64::from_le_bytes(*chunk);
        if !is_8digits(chunk) {
            break;
        }
        *w = w.wrapping_mul(100_000_000).wrapping_add(parse_8digits(chunk));
        rest = after;
    }
    while let Some((&byte, after)) = rest.split_first() {
        let digit = byte.wrapping_sub(b'0');
        if digit > 9 {
            break;
        }
        *w = w.wrapping_mul(10).wrapping_add(u64::from(digit));
        rest = after;
    }
    let count = text.len() - rest.len();
    (text.get(..count).unwrap_or_default(), rest)
}

/// The first 19 significant digits of a mantissa longer than that, the power of ten that
/// scales them, and whether any digit past them was nonzero.
fn first_nineteen(integer: &[u8], fraction: &[u8], explicit: i64) -> (u64, i64, bool) {
    fn without_leading_zeros(digits: &[u8]) -> &[u8] {
        let zeros = digits.iter().take_while(|&&byte| byte == b'0').count();
        digits.get(zeros..).unwrap_or_default()
    }
    fn read(digits: &[u8], w: u64) -> u64 {
        digits.iter().fold(w, |w, &byte| w * 10 + u64::from(byte - b'0'))
    }
    let integer = without_leading_zeros(integer);
    // Zeros that open the fraction are significant only once a digit precedes them.
    let significant = if integer.is_empty() { without_leading_zeros(fraction) } else { fraction };
    let skipped = fraction.len() - significant.len();

    let kept = KEPT_DIGITS as usize;
    let (head, dropped) = integer.split_at_checked(kept).unwrap_or((integer, &[]));
    let room = kept - head.len();
    let (tail, dropped_tail) = significant.split_at_checked(room).unwrap_or((significant, &[]));
    let w = read(tail, read(head, 0));
    let dropped_nonzero = dropped.iter().chain(dropped_tail).any(|&byte| byte != b'0');
    let q = explicit + dropped.len() as i64 - (skipped + tail.len()) as i64;
    (w, q, dropped_nonzero)
}

/// Reads `[+|-]digits[.digits][e[+|-]digits]` — at least one mantissa digit on either side
/// of the point, at least one exponent digit when there is an `e` — to the nearest `T`,
/// ties to even. A magnitude past the type's range is infinite and one below it is zero,
/// signed either way, exactly as `core`'s parser returns them. `None` is text outside that
/// shape, which the doors' own scanners never produce.
pub(crate) fn parse<T: Real>(text: &[u8]) -> Option<T> {
    let (negative, body) = match text.split_first() {
        Some((b'-', rest)) => (true, rest),
        Some((b'+', rest)) => (false, rest),
        _ => (false, text),
    };
    // One pass: the digits go into `w` as they are recognized. Nineteen or fewer cannot
    // overflow it, which is every number but the long ones `first_nineteen` re-reads.
    let mut w = 0u64;
    let (integer, rest) = accumulate(body, &mut w);
    let (fraction, rest) = match rest.split_first() {
        Some((b'.', after)) => accumulate(after, &mut w),
        _ => (&[][..], rest),
    };
    let digits = integer.len() + fraction.len();
    if digits == 0 {
        return None;
    }
    let explicit = match rest.split_first() {
        None => 0,
        Some((b'e' | b'E', exponent)) => {
            let (negative, digits) = match exponent.split_first() {
                Some((b'-', rest)) => (true, rest),
                Some((b'+', rest)) => (false, rest),
                _ => (false, exponent),
            };
            let (digits, rest) = digits_of(digits);
            if digits.is_empty() || !rest.is_empty() {
                return None;
            }
            let mut value = 0i64;
            for &digit in digits {
                if value < EXPONENT_CEILING {
                    value = value * 10 + i64::from(digit - b'0');
                }
            }
            if negative { -value } else { value }
        }
        Some(_) => return None,
    };
    let (w, q, dropped_nonzero) = if digits <= KEPT_DIGITS as usize {
        (w, explicit - fraction.len() as i64, false)
    } else {
        first_nineteen(integer, fraction, explicit)
    };

    let magnitude = if w == 0 {
        T::zero()
    } else if let Some(value) = T::exact(w, q).filter(|_| !dropped_nonzero) {
        value
    } else {
        let settled = match eisel_lemire::<T>(q, w) {
            Some(fields) if !dropped_nonzero => Some(fields),
            // The true value lies strictly between `w` and `w + 1` in the last kept place.
            Some(fields) => (eisel_lemire::<T>(q, w + 1) == Some(fields)).then_some(fields),
            None => None,
        };
        match settled {
            Some(fields) => fields.pack::<T>(),
            None => exact_division::<T>(integer, fraction, explicit)?.pack::<T>(),
        }
    };
    Some(if negative { -magnitude } else { magnitude })
}

/// The binary exponent of `10^q`'s leading bit: `floor(q * log2(10)) + 63`, by an integer
/// approximation of `log2(10)` that is exact across every exponent a float can have.
fn power(q: i32) -> i32 {
    (q.wrapping_mul(152_170 + 65_536) >> 16) + 63
}

fn full_multiplication(a: u64, b: u64) -> (u64, u64) {
    let product = u128::from(a) * u128::from(b);
    (product as u64, (product >> 64) as u64)
}

/// `w * 5^q` to 128 bits, as `(low, high)`: one multiplication by the table's high word,
/// and a second by its low word only when the first leaves the rounding bits in doubt.
fn product_approximation(q: i64, w: u64, precision: u32) -> Option<(u64, u64)> {
    let mask = if precision < 64 { u64::MAX >> precision } else { u64::MAX };
    let index = usize::try_from(q - SMALLEST_POWER_OF_FIVE).ok()?;
    let &(high_five, low_five) = POWER_OF_FIVE_128.get(index)?;
    let (mut first_low, mut first_high) = full_multiplication(w, high_five);
    if first_high & mask == mask {
        let (_, second_high) = full_multiplication(w, low_five);
        first_low = first_low.wrapping_add(second_high);
        if second_high > first_low {
            first_high += 1;
        }
    }
    Some((first_low, first_high))
}

/// Eisel-Lemire: `w * 10^q` rounded to `T`, or `None` when 128 bits of the power of five
/// cannot decide the rounding. `w` is nonzero.
fn eisel_lemire<T: Real>(q: i64, w: u64) -> Option<Fields> {
    if q < T::SMALLEST_POWER_OF_TEN {
        return Some(Fields::ZERO);
    }
    if q > T::LARGEST_POWER_OF_TEN {
        return Some(Fields::infinite::<T>());
    }
    let leading = w.leading_zeros();
    let w = w << leading;
    let (low, high) = product_approximation(q, w, T::SIG_BITS + 3)?;
    if low == u64::MAX && !(-27..=55).contains(&q) {
        // The product could be one more than computed, and that one could cross a halfway
        // point. Inside the range, 5^q fits the table exactly and it cannot.
        return None;
    }
    let upper_bit = (high >> 63) as i32;
    let shift = (upper_bit + 64 - T::SIG_BITS as i32 - 3) as u32;
    let mut mantissa = high >> shift;
    let mut power2 = power(q as i32) + upper_bit - leading as i32 + T::BIAS;
    if power2 <= 0 {
        if -power2 + 1 >= 64 {
            return Some(Fields::ZERO);
        }
        // Subnormal: shift into place, then round.
        mantissa >>= (-power2 + 1) as u32;
        mantissa += mantissa & 1;
        mantissa >>= 1;
        let power = i32::from(mantissa >= 1 << T::SIG_BITS);
        return Some(Fields { mantissa, power });
    }
    // A product that is exactly halfway rounds to even: only when 5^q fits one word, the
    // bit below the mantissa is the lone one set, and nothing was shifted out below it.
    if low <= 1
        && q >= T::MIN_EXPONENT_ROUND_TO_EVEN
        && q <= T::MAX_EXPONENT_ROUND_TO_EVEN
        && mantissa & 3 == 1
        && mantissa << shift == high
    {
        mantissa &= !1;
    }
    mantissa += mantissa & 1;
    mantissa >>= 1;
    if mantissa >= 2 << T::SIG_BITS {
        mantissa = 1 << T::SIG_BITS;
        power2 += 1;
    }
    mantissa &= !(1 << T::SIG_BITS);
    if power2 >= T::INFINITE_POWER {
        return Some(Fields::infinite::<T>());
    }
    Some(Fields { mantissa, power: power2 })
}

/// 64-bit limbs in a [`Big`]: 4,096 bits. The largest value the division forms is under
/// 3,700 (see [`exact_division`]).
const LIMBS: usize = 64;
/// Significant digits the exact step reads before folding the rest into a sticky digit.
const EXACT_DIGITS: usize = 768;
const TEN_19: u64 = 10_000_000_000_000_000_000;

/// A non-negative integer in a fixed array, little-endian. Limbs at and above `len` are
/// zero. Every operation that could grow past the array returns `None` instead.
#[derive(Clone, Copy)]
struct Big {
    limbs: [u64; LIMBS],
    len: usize,
}

impl Big {
    fn from_u64(value: u64) -> Big {
        let mut limbs = [0; LIMBS];
        if let Some(first) = limbs.first_mut() {
            *first = value;
        }
        Big { limbs, len: usize::from(value != 0) }
    }

    fn is_zero(&self) -> bool {
        self.len == 0
    }

    fn trim(&mut self) {
        while self.len > 0 && self.limbs.get(self.len - 1) == Some(&0) {
            self.len -= 1;
        }
    }

    /// `self = self * factor + addend`.
    fn mul_add(&mut self, factor: u64, addend: u64) -> Option<()> {
        let mut carry = addend;
        for limb in self.limbs.iter_mut().take(self.len) {
            let product = u128::from(*limb) * u128::from(factor) + u128::from(carry);
            *limb = product as u64;
            carry = (product >> 64) as u64;
        }
        if carry != 0 {
            *self.limbs.get_mut(self.len)? = carry;
            self.len += 1;
        }
        Some(())
    }

    /// `self *= 10^exponent`.
    fn mul_pow10(&mut self, mut exponent: u64) -> Option<()> {
        while exponent >= 19 {
            self.mul_add(TEN_19, 0)?;
            exponent -= 19;
        }
        let mut rest = 1u64;
        for _ in 0..exponent {
            rest *= 10;
        }
        self.mul_add(rest, 0)
    }

    fn bit_len(&self) -> u64 {
        match self.len.checked_sub(1).and_then(|top| self.limbs.get(top)) {
            Some(top) => self.len as u64 * 64 - u64::from(top.leading_zeros()),
            None => 0,
        }
    }

    /// `self <<= bits`.
    fn shl(&mut self, bits: u64) -> Option<()> {
        if self.len == 0 || bits == 0 {
            return Some(());
        }
        let words = usize::try_from(bits / 64).ok()?;
        let rest = (bits % 64) as u32;
        // Highest limb first, so each source is read before its place is written over.
        let mut index = self.len;
        while index > 0 {
            index -= 1;
            let value = *self.limbs.get(index)?;
            if rest == 0 {
                *self.limbs.get_mut(index + words)? = value;
            } else {
                *self.limbs.get_mut(index + words + 1)? |= value >> (64 - rest);
                *self.limbs.get_mut(index + words)? = value << rest;
            }
        }
        for limb in self.limbs.iter_mut().take(words) {
            *limb = 0;
        }
        self.len = (self.len + words + 1).min(LIMBS);
        self.trim();
        Some(())
    }

    /// `self >>= 1`.
    fn shr1(&mut self) {
        let mut carry = 0;
        for limb in self.limbs.iter_mut().take(self.len).rev() {
            let low = *limb & 1;
            *limb = *limb >> 1 | carry << 63;
            carry = low;
        }
        self.trim();
    }

    fn compare(&self, other: &Big) -> Ordering {
        self.len.cmp(&other.len).then_with(|| {
            let mine = self.limbs.iter().take(self.len).rev();
            let theirs = other.limbs.iter().take(self.len).rev();
            mine.cmp(theirs)
        })
    }

    /// `self -= other`, for `other <= self`.
    fn sub(&mut self, other: &Big) {
        let mut borrow = false;
        for (limb, &take) in self.limbs.iter_mut().zip(other.limbs.iter()).take(self.len) {
            let (difference, first) = limb.overflowing_sub(take);
            let (difference, second) = difference.overflowing_sub(u64::from(borrow));
            *limb = difference;
            borrow = first | second;
        }
        self.trim();
    }
}

/// The digits of `integer` then `fraction` times `10^explicit`, rounded exactly.
///
/// The value is `N / M` for integers `N` and `M`: the digits, with `10^|q|` multiplied into
/// whichever side the exponent `q` of the last digit puts it. One of the two is shifted
/// left until `2^63 <= N / M < 2^64`, the quotient is taken a bit at a time, and whether
/// anything remained is the sticky bit. The sizes stay inside a [`Big`] because the
/// magnitude is bounded first: with at most 769 digits and a value between `10^-331` and
/// `10^310`, `q` is at least `-1100`, so `M` is under 3,655 bits and `N`, shifted, under
/// 3,700.
fn exact_division<T: Real>(integer: &[u8], fraction: &[u8], explicit: i64) -> Option<Fields> {
    let mut numerator = Big::from_u64(0);
    let mut q = explicit;
    let mut count = 0usize;
    let mut sticky = false;
    // Digits enter nineteen at a time: one multiplication of the big integer per chunk.
    let mut chunk = 0u64;
    let mut chunk_scale = 1u64;
    for (in_fraction, &byte) in
        integer.iter().map(|byte| (false, byte)).chain(fraction.iter().map(|byte| (true, byte)))
    {
        let digit = byte - b'0';
        if count == 0 && digit == 0 {
            q -= i64::from(in_fraction);
            continue;
        }
        if count < EXACT_DIGITS {
            chunk = chunk * 10 + u64::from(digit);
            chunk_scale *= 10;
            count += 1;
            q -= i64::from(in_fraction);
            if chunk_scale == TEN_19 {
                numerator.mul_add(chunk_scale, chunk)?;
                chunk = 0;
                chunk_scale = 1;
            }
        } else {
            q += i64::from(!in_fraction);
            sticky |= digit != 0;
        }
    }
    numerator.mul_add(chunk_scale, chunk)?;
    if sticky {
        // Past 768 digits only "more than nothing" matters: no halfway point is that long.
        numerator.mul_add(10, 1)?;
        q -= 1;
        count += 1;
    }
    if numerator.is_zero() {
        return Some(Fields::ZERO);
    }

    // The value is in [10^(magnitude - 1), 10^magnitude).
    let magnitude = q.saturating_add(count as i64);
    if magnitude > 310 {
        return Some(Fields::infinite::<T>());
    }
    if magnitude < -330 {
        return Some(Fields::ZERO);
    }
    let mut denominator = Big::from_u64(1);
    if q >= 0 {
        numerator.mul_pow10(q.unsigned_abs())?;
    } else {
        denominator.mul_pow10(q.unsigned_abs())?;
    }

    // Shift so the quotient has exactly 64 bits; `scale` is how far the numerator moved.
    let mut scale = 63 - (numerator.bit_len() as i64 - denominator.bit_len() as i64);
    if scale >= 0 {
        numerator.shl(scale.unsigned_abs())?;
    } else {
        denominator.shl(scale.unsigned_abs())?;
    }
    let mut step = denominator;
    step.shl(63)?;
    if numerator.compare(&step) == Ordering::Less {
        numerator.shl(1)?;
        scale += 1;
    }
    let mut quotient = 0u64;
    for bit in (0..64).rev() {
        if numerator.compare(&step) != Ordering::Less {
            numerator.sub(&step);
            quotient |= 1 << bit;
        }
        step.shr1();
    }
    Some(round::<T>(quotient, !numerator.is_zero(), -scale))
}

/// Rounds `quotient * 2^exponent2` — plus something under one unit of `quotient` when
/// `above` — to `T`, nearest, ties to even. `quotient` has its top bit set.
fn round<T: Real>(quotient: u64, above: bool, exponent2: i64) -> Fields {
    // The value is in [2^exponent, 2^(exponent + 1)).
    let mut exponent = exponent2 + 63;
    let bias = i64::from(T::BIAS);
    if exponent > bias {
        return Fields::infinite::<T>();
    }
    let subnormal = exponent < 1 - bias;
    // How many low bits of the quotient fall below the mantissa.
    let mut shift = 63 - i64::from(T::SIG_BITS);
    if subnormal {
        shift += 1 - bias - exponent;
    }
    if shift > 64 {
        return Fields::ZERO;
    }
    let (mut mantissa, below, half) = if shift == 64 {
        (0, quotient, 1 << 63)
    } else {
        let shift = shift as u32;
        (quotient >> shift, quotient & ((1 << shift) - 1), 1 << (shift - 1))
    };
    if below > half || (below == half && (above || mantissa & 1 == 1)) {
        mantissa += 1;
    }
    if subnormal {
        // Rounding up to 2^SIG_BITS is the smallest normal, and these are its bits.
        return Fields { mantissa, power: 0 };
    }
    if mantissa == 2 << T::SIG_BITS {
        mantissa = 1 << T::SIG_BITS;
        exponent += 1;
        if exponent > bias {
            return Fields::infinite::<T>();
        }
    }
    Fields { mantissa: mantissa & !(1 << T::SIG_BITS), power: (exponent + bias) as i32 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::String;

    /// xorshift64*: deterministic, no dependency.
    struct Random(u64);

    impl Random {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }

        fn below(&mut self, bound: u64) -> u64 {
            self.next() % bound
        }
    }

    /// The exact step alone, bypassing the two fast ones, for the same text.
    fn exact_only<T: Real>(text: &str) -> T {
        let (negative, body) = match text.as_bytes().split_first() {
            Some((b'-', rest)) => (true, rest),
            Some((b'+', rest)) => (false, rest),
            _ => (false, text.as_bytes()),
        };
        let (integer, rest) = digits_of(body);
        let (fraction, rest) = match rest.split_first() {
            Some((b'.', after)) => digits_of(after),
            _ => (&[][..], rest),
        };
        let explicit = match rest.split_first() {
            Some((_, exponent)) => core::str::from_utf8(exponent)
                .unwrap()
                .parse::<i128>()
                .unwrap()
                .clamp(-i128::from(EXPONENT_CEILING), i128::from(EXPONENT_CEILING))
                as i64,
            None => 0,
        };
        let value = exact_division::<T>(integer, fraction, explicit).unwrap().pack::<T>();
        if negative { -value } else { value }
    }

    /// Every path against `core`, as bits: the door's own route and the exact step alone.
    #[track_caller]
    fn agree(text: &str) {
        let expected = text.parse::<f64>().unwrap().to_bits();
        assert_eq!(parse::<f64>(text.as_bytes()).map(f64::to_bits), Some(expected), "f64 {text}");
        assert_eq!(exact_only::<f64>(text).to_bits(), expected, "f64 exact {text}");
        let expected = text.parse::<f32>().unwrap().to_bits();
        assert_eq!(parse::<f32>(text.as_bytes()).map(f32::to_bits), Some(expected), "f32 {text}");
        assert_eq!(exact_only::<f32>(text).to_bits(), expected, "f32 exact {text}");
    }

    #[test]
    fn the_shapes_and_the_edges() {
        for text in [
            "0",
            "-0",
            "+0",
            "0.0",
            "0e0",
            "1",
            "-1",
            "+1",
            "1.",
            ".5",
            "-.5",
            "1e0",
            "1E5",
            "1e+5",
            "1e-5",
            "2.5",
            "0.1",
            "0.2",
            "0.3",
            "123456789",
            "1.7976931348623157e308",
            "1.7976931348623158e308",
            "1.7976931348623159e308",
            "1.8e308",
            "1e309",
            "1e400",
            "4.9e-324",
            "5e-324",
            "2.4703282292062327e-324",
            "2.4703282292062328e-324",
            "2.47032822920623272e-324",
            "2.2250738585072011e-308",
            "2.2250738585072012e-308",
            "2.2250738585072014e-308",
            "9007199254740993",
            "9007199254740992",
            "9007199254740991",
            "3.4028234e38",
            "3.4028235e38",
            "3.4028236e38",
            "3.4028235677973366e38",
            "1.4e-45",
            "7e-46",
            "7.006492321624085e-46",
            "7.006492321624086e-46",
            "1.17549435e-38",
            "16777217",
            "16777216",
            "16777215",
            "1e22",
            "1e23",
            "8.5e22",
            "1e-22",
            "1e-23",
            "123456789012345678901234567890",
            "0.000000000000000000000000000001",
            "1e99999999999999999999",
            "1e-99999999999999999999",
            "0e99999999999999999999",
            "100000000000000000000000000000000000000000000000000",
            "0.30000000000000004",
            "17976931348623157e292",
            "00000000000000000000001.5",
            "0.00000000000000000000000000000000000000000000000000000000000000000000000000001e80",
        ] {
            agree(text);
        }
        for text in
            ["", "-", "+", ".", "e5", "1e", "1e+", "1x", "1.2.3", "1e5x", " 1", "NaN", "inf"]
        {
            assert!(parse::<f64>(text.as_bytes()).is_none(), "{text:?}");
            assert!(parse::<f32>(text.as_bytes()).is_none(), "{text:?}");
        }
    }

    #[test]
    fn every_float_round_trips_through_its_own_text() {
        let mut random = Random(0x9E37_79B9_7F4A_7C15);
        for _ in 0..200_000 {
            let double = f64::from_bits(random.next());
            if double.is_finite() {
                agree(&format!("{double}"));
                agree(&format!("{double:e}"));
            }
            let single = f32::from_bits(random.next() as u32);
            if single.is_finite() {
                agree(&format!("{single}"));
                agree(&format!("{single:e}"));
            }
        }
    }

    #[test]
    fn random_digit_strings_agree_with_core() {
        let mut random = Random(0x0123_4567_89AB_CDEF);
        for _ in 0..300_000 {
            let mut text = String::new();
            if random.below(4) == 0 {
                text.push('-');
            }
            let integer_digits = random.below(25);
            for _ in 0..integer_digits {
                text.push(char::from(b'0' + random.below(10) as u8));
            }
            let fraction_digits = random.below(25) + u64::from(integer_digits == 0);
            if fraction_digits > 0 || random.below(8) == 0 {
                text.push('.');
            }
            for _ in 0..fraction_digits {
                text.push(char::from(b'0' + random.below(10) as u8));
            }
            if random.below(3) > 0 {
                let exponent = random.below(700) as i64 - 350;
                text.push_str(&format!("e{exponent}"));
            }
            agree(&text);
        }
    }

    /// Exact halfway points and their nearest neighbours — where rounding is decided by a
    /// tie or by a digit hundreds of places down. An odd integer times a power of two is
    /// exactly halfway between two doubles once it needs 54 bits; a power of two in the
    /// denominator is a power of five in the numerator and a decimal exponent.
    #[test]
    fn halfway_points_round_to_even_and_their_neighbours_do_not() {
        let mut random = Random(0xDEAD_BEEF_CAFE_F00D);
        for _ in 0..40_000 {
            let odd = u128::from((1u64 << 53) | random.below(1 << 53) | 1);
            let up = random.below(74) as u32;
            let integer = odd << up;
            agree(&format!("{integer}"));
            agree(&format!("{}", integer + 1));
            agree(&format!("{}", integer - 1));
            agree(&format!("{integer}.000000000000000000000000000000000000000001"));
            let down = random.below(32) as u32;
            let scaled = odd * 5u128.pow(down);
            agree(&format!("{scaled}e-{down}"));
            agree(&format!("{scaled}{}1e-{}", "0".repeat(27), down + 28));
            agree(&format!("{}e-{down}", scaled - 1));
            // The same construction for f32: 25 bits, exactly between two singles.
            let odd = u128::from((1u32 << 24) | (random.below(1 << 24) as u32) | 1);
            agree(&format!("{}", odd << random.below(100)));
            let down = random.below(40) as u32;
            agree(&format!("{}e-{down}", odd * 5u128.pow(down)));
        }
    }

    /// The exact decimal expansion of a float is hundreds of digits long and names that
    /// float; the expansion of the point halfway to its neighbour is one digit longer and
    /// is a tie. For `f32` both are exact in an `f64`, so `core` can print them.
    #[test]
    fn full_expansions_and_subnormals_agree_with_core() {
        let mut random = Random(0x5EED_5EED_5EED_5EED);
        for _ in 0..3_000 {
            let double = f64::from_bits(random.next() & !(1 << 63));
            if double.is_finite() {
                agree(&format!("{double:.1100e}"));
            }
            let subnormal = f64::from_bits(random.below(1 << 52));
            agree(&format!("{subnormal:.1100e}"));
            agree(&format!("{subnormal:e}"));
            let single = f32::from_bits(random.next() as u32 & !(1 << 31));
            let next = f32::from_bits(single.to_bits() + 1);
            if single.is_finite() && next.is_finite() {
                let halfway = (f64::from(single) + f64::from(next)) / 2.0;
                let text = format!("{halfway:.400e}");
                agree(&text);
                let (mantissa, exponent) = text.split_once('e').unwrap();
                agree(&format!("{mantissa}1e{exponent}"));
            }
        }
    }
}
