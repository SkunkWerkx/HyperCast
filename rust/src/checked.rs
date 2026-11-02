//! The checked doors: a door's verdict, then the judgement of the [`Constraints`] declared
//! for it, in the spec's order. The door parses first, exactly as it does alone, and its
//! faults keep their meanings and spans. Then, for text, the trim; for numbers, `multipleOf`
//! (snapping under the declared [`Rounding`](crate::Rounding)), the bounds on the snapped
//! value, and an integer `enum`; for text, the lengths and a folded `enum`. Last, delivery
//! in the door's own type.
//!
//! Numbers are judged exactly, on the [`Exact`] view of what the door read: an integer door's
//! value, a decimal door's value, or a float door's text, streamed however long (never the
//! float, which would be judging a neighbour of the value). An unadjusted success delivers
//! the plain door's own value; only a rounded one is converted, through
//! [`Rounded::write_canonical`] and `float::parse` for a float. A float door then judges the
//! float it delivers against each bound as the door reads that bound (the same `float::parse`,
//! ties to even): `0.99999999999` is inside `exclusiveMaximum: 1` but delivers `1.0` at `f32`,
//! which is the bound itself, and the core never delivers a float past a bound so read.

use core::cmp::Ordering;
use core::fmt;

use crate::abi::RawConstraints;
use crate::constraints::{Constraints, Entries, Keyword, Numeric, Text, check_without_duplicates};
use crate::door::Door;
use crate::exact::{Exact, Rounded, Rounding, Snap, compare, snap, write_decimal_canonical};
use crate::float::{Real, from_significand};
use crate::text::{cast_text, code_points, eq_folded, trim_white_space};
use crate::typed::shortest_digits;
use crate::verdict::{Decimal, Fault, NumFormat, Reason, span_of, trim};

/// A value that passed its door and every declared constraint. Built by the checked doors,
/// or by [`new`](Self::new) for a test double; more fields may join in a minor release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Checked<T> {
    /// The value, in the door's own type: at the text door with an `enum`, the declared entry
    /// the input matched.
    pub value: T,
    /// True only when `multipleOf`'s rounding changed the value. Trimming and case never do.
    pub adjusted: bool,
    /// At the text door, the index of the `enum` entry the input matched; `None` otherwise,
    /// an integer `enum` included (the value is the answer).
    pub enum_index: Option<u32>,
}

impl<T> Checked<T> {
    /// A checked value with every field given, as a checked door would deliver it: for a
    /// test double or a downstream adapter. A field that joins in a minor release takes its
    /// neutral value here.
    pub const fn new(value: T, adjusted: bool, enum_index: Option<u32>) -> Checked<T> {
        Checked { value, adjusted, enum_index }
    }
}

/// Why a checked door refused a value: it did not parse, or it parsed and broke a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// The door's own fault, unchanged.
    Fault(Fault),
    /// The value parsed and broke a declared constraint.
    Violation(Violation),
}

/// A declared constraint a value broke, and the byte span of that value in the caller's
/// input (after the door's trim, and for text after `trim`). A number already decoded has no
/// text, and its span is empty. Built by the checked doors, or by [`new`](Self::new) for a
/// test double; more fields may join in a minor release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Violation {
    /// The keyword broken.
    pub keyword: Keyword,
    /// Where the value starts, as a byte offset into the caller's input.
    pub offset: u32,
    /// How many bytes the value runs.
    pub len: u32,
}

impl Violation {
    /// A violation of `keyword` spanning `len` bytes at `offset`, as a checked door would
    /// report it: for a test double or a downstream adapter. A field that joins in a minor
    /// release takes its neutral value here.
    ///
    /// ```
    /// use hypercast::*;
    ///
    /// let rules = NumericRules::new().maximum(decimal_for_declaration("9")?);
    /// let digit = Constraints::<i32>::numeric(rules, None)?;
    /// let broken = Violation::new(Keyword::Maximum, 1, 2);
    /// assert_eq!(cast_checked(" 10", &NumFormat::INVARIANT, &digit), Err(broken.into()));
    /// assert_eq!(cast_checked("7", &NumFormat::INVARIANT, &digit), Ok(Checked::new(7, false, None)));
    /// # Ok::<(), Refusal>(())
    /// ```
    pub const fn new(keyword: Keyword, offset: u32, len: u32) -> Violation {
        Violation { keyword, offset, len }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "breaks {} at bytes {}..{}",
            self.keyword.name(),
            self.offset,
            u64::from(self.offset) + u64::from(self.len)
        )
    }
}

impl core::error::Error for Violation {}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rejection::Fault(fault) => fault.fmt(f),
            Rejection::Violation(violation) => violation.fmt(f),
        }
    }
}

impl core::error::Error for Rejection {}

impl From<Fault> for Rejection {
    fn from(fault: Fault) -> Rejection {
        Rejection::Fault(fault)
    }
}

impl From<Violation> for Rejection {
    fn from(violation: Violation) -> Rejection {
        Rejection::Violation(violation)
    }
}

/// What a checked door reports beside its value, as the C exports carry it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CheckOutcome {
    /// Rounding changed the value.
    pub adjusted: bool,
    /// The text `enum` entry matched.
    pub enum_index: Option<u32>,
    /// The judged value's byte span in the input: the door's trimmed token, or for text the
    /// span after `trim`. Empty for a number already decoded.
    pub span: (u32, u32),
}

mod seal {
    /// The proof that a call comes from inside the crate. Its field is private to this
    /// module, so nothing outside it can build one, and every [`Judged`](super::Judged)
    /// method takes one: a downstream `T: Numeric` bound reaches the methods' names but
    /// can never call them.
    #[derive(Clone, Copy)]
    pub struct Token(());

    /// The only token there is.
    pub(crate) const TOKEN: Token = Token(());
}

use seal::{TOKEN, Token};

/// A numeric door's type, as the checked doors read and deliver it: the supertrait of
/// [`Numeric`]'s seal, so every numeric door implements it, here. Nominally public in a
/// private module, as a seal is, and every method takes a [`Token`] only this crate can
/// build, so a downstream bound on [`Numeric`] cannot call any of them (supertrait methods
/// are otherwise callable through it). Which methods a type answers follows its door: the
/// integers [`whole`](Self::whole), the decimal [`decimal`](Self::decimal), the floats
/// [`from_canonical`](Self::from_canonical).
pub trait Judged: Copy {
    /// True at an integer door.
    fn integer(_: Token) -> bool {
        false
    }
    /// True at the decimal door.
    fn exact(_: Token) -> bool {
        false
    }
    /// The plain text door.
    fn read(_: Token, input: &[u8], format: &NumFormat) -> Result<Self, Fault>;
    /// The plain typed door, for a number already decoded.
    fn typed(_: Token, value: f64) -> Result<Self, Reason>;
    /// The value as an integer, at an integer door.
    fn whole(self, _: Token) -> Option<i128> {
        None
    }
    /// An integer in this type, at an integer door, or `None` when it does not fit.
    fn from_whole(_: Token, _: i128) -> Option<Self> {
        None
    }
    /// The value as a decimal, at the decimal door.
    fn decimal(self, _: Token) -> Option<Decimal> {
        None
    }
    /// A decimal in this type, at the decimal door.
    fn from_decimal(_: Token, _: Decimal) -> Option<Self> {
        None
    }
    /// Canonical ASCII (`[-]digits[e[-]exponent]`) as the nearest finite value, at a float
    /// door, through the crate's own conversion; an infinity does not fit.
    fn from_canonical(_: Token, _: &[u8]) -> Option<Self> {
        None
    }
    /// The value, widened without loss to an `f64`, at a float door: what the bounds judge
    /// again once it is delivered.
    fn float(self, _: Token) -> Option<f64> {
        None
    }
    /// A declared bound as this float door reads it (the nearest float, ties to even, as the
    /// door's own conversion reads the bound's text), widened without loss to an `f64`; past
    /// the type's range it is infinite. `None` at every other door.
    fn bound(_: Token, _: &Decimal) -> Option<f64> {
        None
    }
}

macro_rules! judged_integers {
    ($($ty:ty => $read:path, $typed:path);+ $(;)?) => {$(
        impl Judged for $ty {
            fn integer(_: Token) -> bool {
                true
            }
            fn read(_: Token, input: &[u8], format: &NumFormat) -> Result<Self, Fault> {
                $read(input, format)
            }
            fn typed(_: Token, value: f64) -> Result<Self, Reason> {
                $typed(value)
            }
            fn whole(self, _: Token) -> Option<i128> {
                Some(self.into())
            }
            fn from_whole(_: Token, value: i128) -> Option<Self> {
                <$ty>::try_from(value).ok()
            }
        }
    )+};
}

judged_integers! {
    i8 => crate::cast_i8, crate::i8_from_f64;
    i16 => crate::cast_i16, crate::i16_from_f64;
    i32 => crate::cast_i32, crate::i32_from_f64;
    i64 => crate::cast_i64, crate::i64_from_f64;
    u8 => crate::cast_u8, crate::u8_from_f64;
    u16 => crate::cast_u16, crate::u16_from_f64;
    u32 => crate::cast_u32, crate::u32_from_f64;
    u64 => crate::cast_u64, crate::u64_from_f64;
}

impl Judged for f32 {
    fn read(_: Token, input: &[u8], format: &NumFormat) -> Result<Self, Fault> {
        crate::cast_f32(input, format)
    }
    fn typed(_: Token, value: f64) -> Result<Self, Reason> {
        crate::f32_from_f64(value)
    }
    fn from_canonical(_: Token, text: &[u8]) -> Option<Self> {
        crate::float::parse::<f32>(text).filter(|value| value.is_finite())
    }
    fn float(self, _: Token) -> Option<f64> {
        Some(self.into())
    }
    fn bound(_: Token, bound: &Decimal) -> Option<f64> {
        read_bound::<f32>(bound).map(f64::from)
    }
}

impl Judged for f64 {
    fn read(_: Token, input: &[u8], format: &NumFormat) -> Result<Self, Fault> {
        crate::cast_f64(input, format)
    }
    fn typed(_: Token, value: f64) -> Result<Self, Reason> {
        if value.is_nan() {
            Err(Reason::Malformed)
        } else if value.is_infinite() {
            Err(Reason::OutOfRange)
        } else {
            // The core delivers no negative zero; `-0.0` is judged as zero and is `0.0`.
            Ok(crate::float::unsigned(value))
        }
    }
    fn from_canonical(_: Token, text: &[u8]) -> Option<Self> {
        crate::float::parse::<f64>(text).filter(|value| value.is_finite())
    }
    fn float(self, _: Token) -> Option<f64> {
        Some(self)
    }
    fn bound(_: Token, bound: &Decimal) -> Option<f64> {
        read_bound::<f64>(bound)
    }
}

impl Judged for Decimal {
    fn exact(_: Token) -> bool {
        true
    }
    fn read(_: Token, input: &[u8], format: &NumFormat) -> Result<Self, Fault> {
        crate::cast_decimal(input, format)
    }
    fn typed(_: Token, value: f64) -> Result<Self, Reason> {
        crate::decimal_from_f64(value)
    }
    fn decimal(self, _: Token) -> Option<Decimal> {
        Some(self)
    }
    fn from_decimal(_: Token, value: Decimal) -> Option<Self> {
        Some(value)
    }
}

/// The view of `value`, which its door read from the trimmed `text` at `start`: an integer
/// door's value (not its text: a radix prefix is the integer doors' own grammar, outside the
/// decimal view's), the decimal door's value (the text's exactly, since that door never
/// drops a digit), or a float door's text, streamed through the door's own routing. The
/// door read the text without a fault, so the view does too.
fn view_of<'a, T: Judged>(
    value: T,
    text: &'a [u8],
    start: usize,
    format: &'a NumFormat,
    buf: &'a mut [u8; 40],
) -> Result<Exact<'a>, Fault> {
    if let Some(whole) = value.whole(TOKEN) {
        return Ok(Exact::from_integer(whole, buf));
    }
    if let Some(decimal) = value.decimal(TOKEN) {
        return Ok(Exact::from_decimal(&decimal, buf));
    }
    Exact::from_text(text, start, format, true)
}

/// A rounded value in the door's type, or `None` when it does not fit.
fn deliver<T: Judged>(rounded: &Rounded) -> Option<T> {
    if T::integer(TOKEN) {
        return rounded.to_i128().and_then(|whole| T::from_whole(TOKEN, whole));
    }
    if T::exact(TOKEN) {
        return rounded.to_decimal().and_then(|decimal| T::from_decimal(TOKEN, decimal));
    }
    let mut out = [0u8; 350];
    let len = rounded.write_canonical(&mut out);
    T::from_canonical(TOKEN, out.get(..len)?)
}

/// The rounding a layout names; an unknown code (which a judged declaration never holds)
/// is strict.
fn rounding(code: u32) -> Rounding {
    match code {
        1 => Rounding::Midpoint,
        2 => Rounding::Floor,
        3 => Rounding::Ceiling,
        _ => Rounding::Strict,
    }
}

/// The span of `len` bytes at `start`, exactly, or `OutOfRange` (spanning what
/// [`span_of`] clamps to) when it ends past `u32::MAX`, where a success span cannot be
/// written.
// The C exports' text doors (ffi.rs), so unused in a build without `exports`.
#[cfg_attr(not(feature = "exports"), allow(dead_code))]
pub(crate) fn exact_span(start: usize, len: usize) -> Result<(u32, u32), Fault> {
    match u32::try_from(start.saturating_add(len)) {
        Ok(_) => Ok(span_of(start, len)),
        Err(_) => {
            let (offset, len) = span_of(start, len);
            Err(Fault { reason: Reason::OutOfRange, offset, len })
        }
    }
}

fn violation(keyword: Keyword, span: (u32, u32)) -> Rejection {
    Rejection::Violation(Violation::new(keyword, span.0, span.1))
}

/// A delivered float's order against `bound` read as `T`'s door reads it: IEEE comparison of
/// two floats. Neither is NaN (no door delivers one, and a bound read is finite or infinite),
/// so the order is total; an unreadable bound, which canonical text never is, is equal.
fn float_order<T: Judged>(value: f64, bound: &Decimal) -> Ordering {
    match T::bound(TOKEN, bound) {
        Some(bound) => value.partial_cmp(&bound).unwrap_or(Ordering::Equal),
        None => Ordering::Equal,
    }
}

/// `bound` as the nearest `T`, ties to even. A magnitude that fits 64 bits goes straight to
/// the conversion's significand path (`magnitude × 10^-scale`, correctly rounded or `None`
/// when it cannot decide); a wider one, or an undecided one, is read as its canonical text,
/// exactly as a door reads its input.
fn read_bound<T: Real>(bound: &Decimal) -> Option<T> {
    if bound.hi == 0
        && let Some(magnitude) = from_significand::<T>(bound.lo, -i64::from(bound.scale))
    {
        return Some(if bound.negative { -magnitude } else { magnitude });
    }
    read_bound_text(bound)
}

/// `bound` as the nearest `T`, read from its canonical text.
fn read_bound_text<T: Real>(bound: &Decimal) -> Option<T> {
    let mut text = [0u8; 48];
    let len = write_decimal_canonical(bound, &mut text);
    crate::float::parse::<T>(text.get(..len).unwrap_or_default())
}

/// The first declared bound `order` (a value's order against a bound) says is broken.
fn broken_bound(raw: &RawConstraints, order: impl Fn(&Decimal) -> Ordering) -> Option<Keyword> {
    let has = |flag: u32| raw.flags & flag != 0;
    if has(RawConstraints::HAS_MINIMUM) && order(&raw.lower) == Ordering::Less {
        return Some(Keyword::Minimum);
    }
    if has(RawConstraints::HAS_EXCLUSIVE_MINIMUM) && order(&raw.lower) != Ordering::Greater {
        return Some(Keyword::ExclusiveMinimum);
    }
    if has(RawConstraints::HAS_MAXIMUM) && order(&raw.upper) == Ordering::Greater {
        return Some(Keyword::Maximum);
    }
    if has(RawConstraints::HAS_EXCLUSIVE_MAXIMUM) && order(&raw.upper) != Ordering::Less {
        return Some(Keyword::ExclusiveMaximum);
    }
    None
}

/// Judges `value`, which its door accepted, as `view` reads it: `multipleOf`, the bounds on
/// the snapped value, an integer `enum`, then delivery, and at a float door the bounds again
/// on the float delivered (not `multipleOf`: a float is no exact multiple of `0.1`). `span`
/// is what a violation, or a rounding past the type, reports.
fn judge<T: Judged>(
    value: T,
    view: &Exact<'_>,
    span: (u32, u32),
    raw: &RawConstraints,
    entries: &Entries<'_>,
) -> Result<(T, bool), Rejection> {
    let has = |flag: u32| raw.flags & flag != 0;
    let out_of_range =
        Rejection::Fault(Fault { reason: Reason::OutOfRange, offset: span.0, len: span.1 });

    // `multipleOf`. The door accepted the value, so `snap` sees nothing longer than a door
    // reads (its own doc says why that matters).
    let mut rounded: Option<Rounded> = None;
    if has(RawConstraints::HAS_MULTIPLE_OF) {
        let mode = rounding(raw.rounding);
        match snap(view, &raw.multiple_of, mode) {
            Snap::Exact => {}
            Snap::NotMultiple if mode == Rounding::Strict => {
                return Err(violation(Keyword::MultipleOf, span));
            }
            // A multiple too long to hold is past every door's range.
            Snap::NotMultiple => return Err(out_of_range),
            Snap::Rounded(multiple) => rounded = Some(multiple),
        }
    }
    let judged = match &rounded {
        Some(multiple) => multiple.as_exact(),
        None => view.clone(),
    };

    // The bounds, on the snapped value.
    if let Some(keyword) = broken_bound(raw, |bound| compare(&judged, bound)) {
        return Err(violation(keyword, span));
    }

    // An integer `enum`, on the snapped value: the first entry equal to it.
    if has(RawConstraints::HAS_ENUM) {
        let whole = match &rounded {
            Some(multiple) => multiple.to_i128(),
            None => value.whole(TOKEN),
        };
        let matched = whole
            .is_some_and(|whole| (0..entries.len()).any(|i| entries.integer(i) == Some(whole)));
        if !matched {
            return Err(violation(Keyword::Enum, span));
        }
    }

    // Delivery: the door's own value, or the rounded one in the door's type.
    let (delivered, adjusted) = match &rounded {
        None => (value, false),
        Some(multiple) => (deliver::<T>(multiple).ok_or(out_of_range)?, true),
    };

    // A float door's delivered value, judged once more against each bound as the door reads
    // it: float meets float, so `0.1` stays inside `maximum: 0.1`, while `0.99999999999`
    // delivers `1.0` at `f32`, the bound `exclusiveMaximum: 1` itself, and `1e-400`
    // delivers `0.0`, the bound `exclusiveMinimum: 0`.
    if let Some(float) = delivered.float(TOKEN)
        && let Some(keyword) = broken_bound(raw, |bound| float_order::<T>(float, bound))
    {
        return Err(violation(keyword, span));
    }
    Ok((delivered, adjusted))
}

/// What the text door and its constraints made of an input: the judged text, where it starts
/// in the input, and the `enum` entry it matched with that entry's index.
struct JudgedText<'i, 'e> {
    text: &'i str,
    /// Where the judged text starts in the input, unclamped.
    start: usize,
    matched: Option<(u32, &'e str)>,
}

/// The text door, then the text constraints: the trim, the lengths, and a folded `enum`.
fn judge_text_value<'i, 'e>(
    input: &'i [u8],
    raw: &RawConstraints,
    entries: &Entries<'e>,
) -> Result<JudgedText<'i, 'e>, Rejection> {
    let has = |flag: u32| raw.flags & flag != 0;
    let text = cast_text(input)?;
    let (text, start) = if has(RawConstraints::TRIM) { trim_white_space(text) } else { (text, 0) };
    if text.is_empty() {
        return Err(Rejection::Fault(Fault::EMPTY));
    }
    let span = span_of(start, text.len());
    let length = code_points(text);
    if has(RawConstraints::HAS_MIN_LENGTH) && length < raw.min_length {
        return Err(violation(Keyword::MinLength, span));
    }
    if has(RawConstraints::HAS_MAX_LENGTH) && length > raw.max_length {
        return Err(violation(Keyword::MaxLength, span));
    }
    // The first entry equal under folding. The index is counted by hand, saturating: a
    // `RangeFrom` would carry an overflow panic the no-panic proof cannot rule out.
    let mut matched = None;
    let mut index: u32 = 0;
    for entry in entries.texts() {
        if eq_folded(text.as_bytes(), entry) {
            // A judged declaration's entries are UTF-8; one that is not cannot have matched.
            matched = core::str::from_utf8(entry).ok().map(|entry| (index, entry));
            break;
        }
        index = index.saturating_add(1);
    }
    if has(RawConstraints::HAS_ENUM) && matched.is_none() {
        return Err(violation(Keyword::Enum, span));
    }
    Ok(JudgedText { text, start, matched })
}

/// Reads `input` through `T`'s door and judges it.
fn checked_numeric<T: Judged>(
    input: &[u8],
    format: &NumFormat,
    raw: &RawConstraints,
    entries: &Entries<'_>,
) -> Result<(T, CheckOutcome), Rejection> {
    let value = T::read(TOKEN, input, format)?;
    let (text, start) = trim(input);
    let span = span_of(start, text.len());
    let mut buf = [0u8; 40];
    let view = view_of(value, text, start, format, &mut buf)?;
    let (value, adjusted) = judge(value, &view, span, raw, entries)?;
    Ok((value, CheckOutcome { adjusted, enum_index: None, span }))
}

/// Reads an already-decoded `value` through `T`'s typed door and judges it on its sign and
/// shortest round-trip digits. Every failure has an empty span: there is no text.
fn checked_from_f64_numeric<T: Judged>(
    value: f64,
    raw: &RawConstraints,
    entries: &Entries<'_>,
) -> Result<(T, CheckOutcome), Rejection> {
    let read = T::typed(TOKEN, value).map_err(|reason| Fault { reason, offset: 0, len: 0 })?;
    let digits = shortest_digits(value);
    let view = Exact::from_shortest(value.is_sign_negative(), digits.as_ref());
    let (read, adjusted) = judge(read, &view, (0, 0), raw, entries)?;
    Ok((read, CheckOutcome { adjusted, enum_index: None, span: (0, 0) }))
}

/// Casts `input` through `T`'s door under `format`, then judges it by `constraints`: the
/// door's own [`Fault`], a [`Violation`] of the first constraint it breaks, or the value
/// (rounded to `multipleOf` when the rounding says so, and then `adjusted`).
///
/// Numbers are judged exactly, on the decimal value the door read, never on a float: with
/// `maximum: 1`, the text `1.00000000000000001` is a violation at the `f64` door although it
/// reads to the same `f64` as `1`. A float door then judges the float it delivers against each
/// bound as the door reads that bound: with `exclusiveMaximum: 1`, the text `0.99999999999` is
/// a violation at the `f32` door, which would deliver `1.0`, while `0.1` stays inside
/// `maximum: 0.1`, being the same float as the bound.
pub fn cast_checked<T: Numeric>(
    input: impl AsRef<[u8]>,
    format: &NumFormat,
    constraints: &Constraints<'_, T>,
) -> Result<Checked<T>, Rejection> {
    let raw = constraints.raw();
    let (value, outcome) =
        checked_numeric::<T>(input.as_ref(), format, &raw, constraints.entries())?;
    Ok(Checked { value, adjusted: outcome.adjusted, enum_index: None })
}

/// Reads a number already decoded (a JSON parser's `f64`, a spreadsheet's numeric cell)
/// through `T`'s typed door, then judges it by `constraints` on its sign and the shortest
/// round-trip decimal of its magnitude: `0.1 + 0.2` is `0.30000000000000004`, and `-0.0` is
/// zero, delivered at a float door as `0.0` and not `adjusted` (nothing was rounded). NaN is
/// `Malformed` and an infinity `OutOfRange`; every failure has an empty span.
pub fn checked_from_f64<T: Numeric>(
    value: f64,
    constraints: &Constraints<'_, T>,
) -> Result<Checked<T>, Rejection> {
    let raw = constraints.raw();
    let (value, outcome) = checked_from_f64_numeric::<T>(value, &raw, constraints.entries())?;
    Ok(Checked { value, adjusted: outcome.adjusted, enum_index: None })
}

/// The text door ([`cast_text`](crate::cast_text)), then `constraints`: trimmed of Unicode
/// `White_Space` when `trim` is declared (trimmed to nothing is `Empty`), lengths counted in
/// code points, and an `enum` matched under simple case folding. A match delivers the
/// declared entry and its index, so `"MALE"` against `["Female", "Male", "Other"]` is
/// `"Male"` at index 1. Never `adjusted`.
pub fn cast_text_checked<'a>(
    input: &'a [u8],
    constraints: &Constraints<'a, Text>,
) -> Result<Checked<&'a str>, Rejection> {
    let raw = constraints.raw();
    let JudgedText { text, matched, .. } = judge_text_value(input, &raw, constraints.entries())?;
    Ok(match matched {
        Some((index, entry)) => Checked { value: entry, adjusted: false, enum_index: Some(index) },
        None => Checked { value: text, adjusted: false, enum_index: None },
    })
}

/// Writes `value` to `out`.
///
/// # Safety
/// `out` must be valid for an (unaligned) write of a `T`.
unsafe fn write<T>(out: *mut u8, value: T) {
    // SAFETY: per the function contract.
    unsafe { out.cast::<T>().write_unaligned(value) }
}

macro_rules! by_door {
    ($door:expr, $judge:ident ( $($arg:expr),* ), $out:expr) => {
        match $door {
            Door::I8 => by_door!(@one i8, $judge($($arg),*), $out),
            Door::I16 => by_door!(@one i16, $judge($($arg),*), $out),
            Door::I32 => by_door!(@one i32, $judge($($arg),*), $out),
            Door::I64 => by_door!(@one i64, $judge($($arg),*), $out),
            Door::U8 => by_door!(@one u8, $judge($($arg),*), $out),
            Door::U16 => by_door!(@one u16, $judge($($arg),*), $out),
            Door::U32 => by_door!(@one u32, $judge($($arg),*), $out),
            Door::U64 => by_door!(@one u64, $judge($($arg),*), $out),
            Door::F32 => by_door!(@one f32, $judge($($arg),*), $out),
            Door::F64 => by_door!(@one f64, $judge($($arg),*), $out),
            Door::Decimal => by_door!(@one Decimal, $judge($($arg),*), $out),
            _ => None,
        }
    };
    (@one $ty:ty, $judge:ident ( $($arg:expr),* ), $out:expr) => {
        Some($judge::<$ty>($($arg),*).map(|(value, outcome)| {
            // SAFETY: the caller's contract: `out` holds the type this door writes.
            unsafe { write($out, value) };
            outcome
        }))
    };
}

/// The C ABI's checked numeric door: re-judges the declaration (every rule but duplicates),
/// then reads `input` through `door` and judges it, writing the value to `out` on success.
/// `None` is a contract violation: a door that is not numeric, or a declaration
/// [`check_constraints`](crate::check_constraints) refuses.
///
/// # Safety
/// `out` must be valid for an unaligned write of the type `door` delivers (a [`Decimal`] at
/// the decimal door).
// The C exports' entry points (ffi.rs), so unused in a build without `exports`.
#[cfg_attr(not(feature = "exports"), allow(dead_code))]
pub(crate) unsafe fn judge_numeric(
    door: Door,
    input: &[u8],
    format: &NumFormat,
    raw: &RawConstraints,
    entries: &Entries<'_>,
    out: *mut u8,
) -> Option<Result<CheckOutcome, Rejection>> {
    if !door.is_numeric() || check_without_duplicates(door, raw, entries).is_err() {
        return None;
    }
    by_door!(door, checked_numeric(input, format, raw, entries), out)
}

/// [`judge_numeric`] for a number already decoded.
///
/// # Safety
/// As [`judge_numeric`].
// The C exports' entry points (ffi.rs), so unused in a build without `exports`.
#[cfg_attr(not(feature = "exports"), allow(dead_code))]
pub(crate) unsafe fn judge_numeric_from_f64(
    door: Door,
    value: f64,
    raw: &RawConstraints,
    entries: &Entries<'_>,
    out: *mut u8,
) -> Option<Result<CheckOutcome, Rejection>> {
    if !door.is_numeric() || check_without_duplicates(door, raw, entries).is_err() {
        return None;
    }
    by_door!(door, checked_from_f64_numeric(value, raw, entries), out)
}

/// The C ABI's checked text door: re-judges the declaration (every rule but duplicates),
/// then judges `input`. The outcome's span is the judged text (after `trim`); with an `enum`,
/// that is the matched input, and the index names the entry. A success whose span ends past
/// `u32::MAX` bytes, which a span cannot carry, is `OutOfRange` instead. `None` is a refused
/// declaration.
// The C exports' entry points (ffi.rs), so unused in a build without `exports`.
#[cfg_attr(not(feature = "exports"), allow(dead_code))]
pub(crate) fn judge_text(
    input: &[u8],
    raw: &RawConstraints,
    entries: &Entries<'_>,
) -> Option<Result<CheckOutcome, Rejection>> {
    if check_without_duplicates(Door::Text, raw, entries).is_err() {
        return None;
    }
    Some(judge_text_value(input, raw, entries).and_then(
        |JudgedText { text, start, matched, .. }| {
            let span = exact_span(start, text.len())?;
            Ok(CheckOutcome { adjusted: false, enum_index: matched.map(|(index, _)| index), span })
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::decimal_for_declaration;
    use serde_json::Value;
    use std::borrow::ToOwned;
    use std::string::{String, ToString};
    use std::vec::Vec;

    fn corpus() -> Vec<Value> {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus/constraints.json");
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn door_of(name: &str) -> Door {
        match name {
            "i8" => Door::I8,
            "i16" => Door::I16,
            "i32" => Door::I32,
            "i64" => Door::I64,
            "u8" => Door::U8,
            "u16" => Door::U16,
            "u32" => Door::U32,
            "u64" => Door::U64,
            "f32" => Door::F32,
            "f64" => Door::F64,
            "decimal" => Door::Decimal,
            _ => Door::Text,
        }
    }

    /// A vector's declaration as the C ABI passes it (`tests/conformance.rs` builds it the
    /// same way, and holds the typed builder to it).
    fn packed(door: Door, c: &Value) -> (RawConstraints, Vec<u8>) {
        let dec = |field: &str| c.get(field).map(|v| decimal_for_declaration(v.as_str().unwrap()));
        let mut raw = RawConstraints::ZERO;
        for (field, flag, upper) in [
            ("minimum", RawConstraints::HAS_MINIMUM, false),
            ("exclusiveMinimum", RawConstraints::HAS_EXCLUSIVE_MINIMUM, false),
            ("maximum", RawConstraints::HAS_MAXIMUM, true),
            ("exclusiveMaximum", RawConstraints::HAS_EXCLUSIVE_MAXIMUM, true),
        ] {
            if let Some(value) = dec(field) {
                raw.flags |= flag;
                *(if upper { &mut raw.upper } else { &mut raw.lower }) = value.unwrap();
            }
        }
        if let Some(value) = dec("multipleOf") {
            raw.flags |= RawConstraints::HAS_MULTIPLE_OF;
            raw.multiple_of = value.unwrap();
        }
        raw.rounding = match c.get("rounding").and_then(Value::as_str) {
            Some("midpoint") => 1,
            Some("floor") => 2,
            Some("ceiling") => 3,
            _ => 0,
        };
        if let Some(length) = c.get("minLength") {
            raw.flags |= RawConstraints::HAS_MIN_LENGTH;
            raw.min_length = length.as_u64().unwrap() as u32;
        }
        if let Some(length) = c.get("maxLength") {
            raw.flags |= RawConstraints::HAS_MAX_LENGTH;
            raw.max_length = length.as_u64().unwrap() as u32;
        }
        if c.get("trim").and_then(Value::as_bool) == Some(true) {
            raw.flags |= RawConstraints::TRIM;
        }
        let mut entries = Vec::new();
        if c.get("enum").is_some() {
            raw.flags |= RawConstraints::HAS_ENUM;
        }
        for entry in c.get("enum").and_then(Value::as_array).into_iter().flatten() {
            let entry = entry.as_str().unwrap();
            if door == Door::Text {
                entries.extend_from_slice(&(entry.len() as u32).to_le_bytes());
                entries.extend_from_slice(entry.as_bytes());
            } else {
                let d = decimal_for_declaration(entry).unwrap();
                entries.extend_from_slice(&d.lo.to_ne_bytes());
                entries.extend_from_slice(&d.hi.to_ne_bytes());
                entries.extend_from_slice(&[d.scale, d.negative.into(), 0, 0]);
            }
        }
        (raw, entries)
    }

    /// What a door wrote to `out`, as the corpus spells it.
    fn written(door: Door, out: &[u8; 16]) -> String {
        let bytes = |n: usize| -> [u8; 8] {
            let mut b = [0u8; 8];
            b[..n].copy_from_slice(&out[..n]);
            b
        };
        match door {
            Door::I8 => (out[0] as i8).to_string(),
            Door::I16 => i16::from_ne_bytes([out[0], out[1]]).to_string(),
            Door::I32 => i32::from_ne_bytes(out[..4].try_into().unwrap()).to_string(),
            Door::I64 => i64::from_ne_bytes(bytes(8)).to_string(),
            Door::U8 => out[0].to_string(),
            Door::U16 => u16::from_ne_bytes([out[0], out[1]]).to_string(),
            Door::U32 => u32::from_ne_bytes(out[..4].try_into().unwrap()).to_string(),
            Door::U64 => u64::from_ne_bytes(bytes(8)).to_string(),
            Door::F32 => {
                let value = f32::from_ne_bytes(out[..4].try_into().unwrap());
                std::format!("{:x}", value.to_bits())
            }
            Door::F64 => std::format!("{:x}", f64::from_ne_bytes(bytes(8)).to_bits()),
            _ => unsafe { out.as_ptr().cast::<Decimal>().read_unaligned() }.to_string(),
        }
    }

    fn expected(door: Door, value: &Value) -> String {
        match door {
            Door::F32 => std::format!("{:x}", (value.as_f64().unwrap() as f32).to_bits()),
            Door::F64 => std::format!("{:x}", value.as_f64().unwrap().to_bits()),
            Door::Decimal => value.as_str().unwrap().to_owned(),
            _ => value.to_string(),
        }
    }

    /// The C exports' path, `judge_numeric`, `judge_numeric_from_f64` and `judge_text` over
    /// packed entries, replays the corpus to the same answers as the public API.
    #[test]
    fn the_export_path_replays_the_corpus() {
        let mut replayed = 0;
        for vector in corpus() {
            let expect = vector["expect"].as_str().unwrap();
            if expect == "refused" {
                continue;
            }
            let door = door_of(vector["door"].as_str().unwrap());
            let (raw, entries) = packed(door, &vector["constraints"]);
            let entries = Entries::packed(door, &entries);
            let mut out = [0u8; 16];
            let input: Vec<u8> = match (vector.get("input"), vector.get("input_bytes")) {
                (Some(text), _) => text.as_str().unwrap().as_bytes().to_vec(),
                (_, Some(bytes)) => {
                    bytes.as_array().unwrap().iter().map(|b| b.as_u64().unwrap() as u8).collect()
                }
                _ => Vec::new(),
            };
            let verdict = if door == Door::Text {
                judge_text(&input, &raw, &entries)
            } else if let Some(value) = vector.get("from_f64") {
                let value = match value.as_str() {
                    Some("-0") => -0.0,
                    Some("NaN") => f64::NAN,
                    Some("Infinity") => f64::INFINITY,
                    Some("-Infinity") => f64::NEG_INFINITY,
                    _ => value.as_f64().unwrap(),
                };
                unsafe { judge_numeric_from_f64(door, value, &raw, &entries, out.as_mut_ptr()) }
            } else {
                let format = NumFormat::INVARIANT;
                // The format-bearing rows are replayed by the public API; here, invariant only.
                if vector.get("format").is_some() {
                    continue;
                }
                unsafe { judge_numeric(door, &input, &format, &raw, &entries, out.as_mut_ptr()) }
            };
            let verdict = verdict.expect("a judged declaration");
            let label = std::format!("{vector}");
            match verdict {
                Ok(outcome) => {
                    assert_eq!(expect, "ok", "{label}");
                    assert_eq!(outcome.adjusted, vector["adjusted"].as_bool().unwrap(), "{label}");
                    let index = vector.get("enum_index").map(|i| i.as_u64().unwrap() as u32);
                    assert_eq!(outcome.enum_index, index, "{label}");
                    if door == Door::Text {
                        // The span is the judged input; the value, the entry it matched.
                        let (offset, len) = (outcome.span.0 as usize, outcome.span.1 as usize);
                        let text = core::str::from_utf8(&input[offset..offset + len]).unwrap();
                        let value = vector["value"].as_str().unwrap();
                        match index {
                            Some(_) => assert!(
                                crate::text::eq_folded(text.as_bytes(), value.as_bytes()),
                                "{label}"
                            ),
                            None => assert_eq!(text, value, "{label}"),
                        }
                    } else {
                        assert_eq!(
                            written(door, &out),
                            expected(door, &vector["value"]),
                            "{label}"
                        );
                    }
                }
                Err(rejection) => {
                    let (code, offset, len) = match rejection {
                        Rejection::Fault(fault) => (fault.reason as u32, fault.offset, fault.len),
                        Rejection::Violation(v) => (4, v.offset, v.len),
                    };
                    let want = match expect {
                        "empty" => 1,
                        "malformed" => 2,
                        "out_of_range" => 3,
                        _ => 4,
                    };
                    assert_eq!(code, want, "{label}");
                    let span = vector["fault"].as_array().unwrap();
                    assert_eq!(
                        (u64::from(offset), u64::from(len)),
                        (span[0].as_u64().unwrap(), span[1].as_u64().unwrap()),
                        "{label}"
                    );
                    if let Rejection::Violation(v) = rejection {
                        assert_eq!(
                            v.keyword.name(),
                            vector["keyword"].as_str().unwrap(),
                            "{label}"
                        );
                    }
                }
            }
            replayed += 1;
        }
        assert!(replayed > 150, "{replayed}");
    }

    #[test]
    fn the_export_path_refuses_what_it_cannot_judge() {
        let mut out = [0u8; 16];
        let none = Entries::None;
        let raw = RawConstraints::ZERO;
        // A door that is not numeric, even with nothing declared.
        assert!(
            unsafe {
                judge_numeric(
                    Door::Bool,
                    b"1",
                    &NumFormat::INVARIANT,
                    &raw,
                    &none,
                    out.as_mut_ptr(),
                )
            }
            .is_none()
        );
        assert!(
            unsafe {
                judge_numeric(
                    Door::Text,
                    b"1",
                    &NumFormat::INVARIANT,
                    &raw,
                    &none,
                    out.as_mut_ptr(),
                )
            }
            .is_none()
        );
        assert!(
            unsafe { judge_numeric_from_f64(Door::Date, 1.0, &raw, &none, out.as_mut_ptr()) }
                .is_none()
        );
        // A declaration the door refuses: minLength on a number, a minimum on text, a bad size.
        let length = RawConstraints { flags: RawConstraints::HAS_MIN_LENGTH, ..raw };
        assert!(
            unsafe {
                judge_numeric(
                    Door::I32,
                    b"1",
                    &NumFormat::INVARIANT,
                    &length,
                    &none,
                    out.as_mut_ptr(),
                )
            }
            .is_none()
        );
        let minimum = RawConstraints { flags: RawConstraints::HAS_MINIMUM, ..raw };
        assert!(judge_text(b"a", &minimum, &none).is_none());
        let short = RawConstraints { size: 71, ..raw };
        assert!(judge_text(b"a", &short, &none).is_none());
        assert!(
            unsafe { judge_numeric_from_f64(Door::F64, 1.0, &short, &none, out.as_mut_ptr()) }
                .is_none()
        );
        // Duplicates are judged only at declaration: here the first matching entry wins.
        let mut twice = Vec::new();
        for entry in ["Male", "MALE"] {
            twice.extend_from_slice(&(entry.len() as u32).to_le_bytes());
            twice.extend_from_slice(entry.as_bytes());
        }
        let listed = RawConstraints { flags: RawConstraints::HAS_ENUM, ..raw };
        let outcome = judge_text(b"male", &listed, &Entries::packed(Door::Text, &twice));
        assert_eq!(
            outcome,
            Some(Ok(CheckOutcome { adjusted: false, enum_index: Some(0), span: (0, 4) }))
        );
        // A trimmed text's span is the text without its White_Space.
        let trim = RawConstraints { flags: RawConstraints::TRIM, ..raw };
        let outcome = judge_text("\u{00A0}Male\u{3000}".as_bytes(), &trim, &none);
        assert_eq!(
            outcome,
            Some(Ok(CheckOutcome { adjusted: false, enum_index: None, span: (2, 4) }))
        );
    }

    /// A bound is read as the door reads it: the nearest float, ties to even, which is what
    /// `core`'s own parser returns for the bound's text (zero aside: no door reads `-0.0`).
    #[test]
    fn bounds_read_as_the_float_doors_read_them() {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut fast = 0;
        for _ in 0..20_000 {
            let magnitude = u128::from(next()) << (next() % 33) & ((1u128 << 96) - 1);
            let decimal = Decimal {
                lo: magnitude as u64,
                hi: (magnitude >> 64) as u32,
                scale: (next() % 29) as u8,
                negative: next() % 2 == 0,
            };
            let shown = decimal.to_string();
            let as_f64 = <f64 as Judged>::bound(TOKEN, &decimal).unwrap();
            let as_f32 = <f32 as Judged>::bound(TOKEN, &decimal).unwrap();
            assert_eq!(as_f64, shown.parse::<f64>().unwrap(), "{shown}");
            assert_eq!(as_f32, f64::from(shown.parse::<f32>().unwrap()), "{shown}");
            // The 64-bit path and the text path agree wherever both decide.
            assert_eq!(read_bound_text::<f64>(&decimal), Some(as_f64), "{shown}");
            assert_eq!(read_bound_text::<f32>(&decimal).map(f64::from), Some(as_f32), "{shown}");
            if decimal.hi == 0 {
                fast += 1;
            }
        }
        assert!(fast > 1_000, "{fast} magnitudes took the 64-bit path");
        // Float meets float: the delivered `0.1` is the bound `0.1`, read at either door.
        assert_eq!(
            float_order::<f64>(0.1, &decimal_for_declaration("0.1").unwrap()),
            Ordering::Equal
        );
        assert_eq!(
            float_order::<f32>(f64::from(0.1f32), &decimal_for_declaration("0.1").unwrap()),
            Ordering::Equal
        );
        assert_eq!(
            float_order::<f64>(1.0, &decimal_for_declaration("1.00000000000000001").unwrap()),
            Ordering::Equal
        );
    }

    // A span past `u32::MAX` needs a 64-bit `usize`: on wasm32 there is no such input.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn spans_past_u32_are_clamped_or_out_of_range() {
        let max = u32::MAX as usize;
        assert_eq!(exact_span(0, max), Ok((0, u32::MAX)));
        assert_eq!(exact_span(1, max - 1), Ok((1, u32::MAX - 1)));
        let out = |offset, len| Err(Fault { reason: Reason::OutOfRange, offset, len });
        assert_eq!(exact_span(0, max + 1), out(0, u32::MAX));
        assert_eq!(exact_span(1, max), out(1, u32::MAX - 1));
        assert_eq!(exact_span(max, 1), out(u32::MAX, 0));
        assert_eq!(exact_span(max + 5, 7), out(u32::MAX, 0));
        // Clamped, a span still ends within the input it indexes (here, 6e9 bytes).
        let (offset, len) = span_of(3_000_000_000, 3_000_000_000);
        assert!(u64::from(offset) + u64::from(len) <= 6_000_000_000);
        assert_eq!((offset, len), (3_000_000_000, u32::MAX - 3_000_000_000));
        // A door's own fault clamps the same way: its end never passes `u32::MAX`.
        for fault in [
            Fault::malformed(3_000_000_000, 3_000_000_000),
            Fault::out_of_range(3_000_000_000, 3_000_000_000),
        ] {
            assert_eq!((fault.offset, fault.len), (3_000_000_000, u32::MAX - 3_000_000_000));
        }
        let fault = Fault::malformed(max + 9, 4);
        assert_eq!((fault.offset, fault.len), (u32::MAX, 0));
        // Display widens: a span built by hand to end past `u32::MAX` still prints.
        let wide = Fault { reason: Reason::Malformed, offset: u32::MAX, len: 2 };
        assert_eq!(wide.to_string(), "malformed input at bytes 4294967295..4294967297");
    }

    #[test]
    fn rejections_display() {
        let violation = Violation { keyword: Keyword::ExclusiveMinimum, offset: 2, len: 3 };
        assert_eq!(violation.to_string(), "breaks exclusiveMinimum at bytes 2..5");
        assert_eq!(Rejection::from(violation).to_string(), violation.to_string());
        let fault = Fault { reason: Reason::Malformed, offset: 0, len: 1 };
        assert_eq!(Rejection::from(fault).to_string(), fault.to_string());
    }
}
