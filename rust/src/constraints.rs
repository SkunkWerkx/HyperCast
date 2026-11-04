//! Validation constraints: the JSON Schema keywords a value is judged by once its door has
//! read it (`minimum`, `exclusiveMinimum`, `maximum`, `exclusiveMaximum`, `multipleOf` with a
//! rounding, `minLength`, `maxLength`, `enum`, and `trim`), declared **for a door** and refused
//! at declaration when they cannot mean anything there.
//!
//! [`check_constraints`] is the one judge of a declaration: the C ABI's
//! `cast_constraints_check` and every binding's `Constraints` constructor ask it, so no two
//! layers can disagree on what is refused. The checked doors re-run every rule but the
//! duplicate pass on each call ([`check_without_duplicates`]).

use core::cmp::Ordering;
use core::fmt;
use core::marker::PhantomData;

use crate::abi::RawConstraints;
use crate::door::Door;
pub use crate::exact::Rounding;
use crate::exact::compare_decimals;
use crate::text::{eq_folded, trim_white_space};
use crate::verdict::Decimal;

/// The largest magnitude a [`Decimal`] holds, 2⁹⁶ − 1.
const MAX_MAGNITUDE: u128 = (1 << 96) - 1;

/// The largest scale a declared [`Decimal`] may carry.
const MAX_SCALE: u8 = 28;

/// The bytes one packed integer `enum` entry takes: a [`Decimal`] in its ABI layout.
const PACKED_DECIMAL: usize = 16;

/// The numeric keywords' flags.
const NUMERIC_FLAGS: u32 = RawConstraints::HAS_MINIMUM
    | RawConstraints::HAS_EXCLUSIVE_MINIMUM
    | RawConstraints::HAS_MAXIMUM
    | RawConstraints::HAS_EXCLUSIVE_MAXIMUM
    | RawConstraints::HAS_MULTIPLE_OF;

/// The text keywords' flags.
const TEXT_FLAGS: u32 =
    RawConstraints::HAS_MIN_LENGTH | RawConstraints::HAS_MAX_LENGTH | RawConstraints::TRIM;

/// Every flag the layout defines.
const KNOWN_FLAGS: u32 = NUMERIC_FLAGS | TEXT_FLAGS | RawConstraints::HAS_ENUM;

/// A numeric bound: the value itself is allowed (`minimum`, `maximum`) or it is not
/// (`exclusiveMinimum`, `exclusiveMaximum`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    /// The bound is allowed.
    Inclusive(Decimal),
    /// The bound is not allowed.
    Exclusive(Decimal),
}

/// A rule a value broke, spelled the OpenAPI way, with the C ABI's code.
///
/// More keywords (`format`, `pattern`) may join in a minor release. Matches outside this
/// crate need a wildcard arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Keyword {
    /// `minimum`.
    Minimum = 1,
    /// `exclusiveMinimum`.
    ExclusiveMinimum,
    /// `maximum`.
    Maximum,
    /// `exclusiveMaximum`.
    ExclusiveMaximum,
    /// `multipleOf`.
    MultipleOf,
    /// `minLength`.
    MinLength,
    /// `maxLength`.
    MaxLength,
    /// `enum`.
    Enum,
}

impl Keyword {
    /// The C ABI's code: `1` minimum … `8` enum.
    pub const fn code(self) -> u32 {
        self as u32
    }

    /// The OpenAPI spelling: `minimum`, `exclusiveMinimum`, … `enum`.
    pub const fn name(self) -> &'static str {
        match self {
            Keyword::Minimum => "minimum",
            Keyword::ExclusiveMinimum => "exclusiveMinimum",
            Keyword::Maximum => "maximum",
            Keyword::ExclusiveMaximum => "exclusiveMaximum",
            Keyword::MultipleOf => "multipleOf",
            Keyword::MinLength => "minLength",
            Keyword::MaxLength => "maxLength",
            Keyword::Enum => "enum",
        }
    }
}

/// Why a declaration was refused: the first rule it breaks, in code order, except that a
/// layout that cannot be read is [`MalformedLayout`](Self::MalformedLayout) first of all: a
/// `size` other than [`RawConstraints::SIZE`], and at the C ABI a decimal's sign byte other
/// than `0` or `1`. A refusal is the caller's contract violation, never a verdict on data.
///
/// More refusals may join in a minor release. Matches outside this crate need a wildcard
/// arm.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Refusal {
    /// A keyword the door does not take (or any constraint on a door that takes none).
    KeywordNotForDoor = 1,
    /// `minimum` together with `exclusiveMinimum`.
    MinimumWithExclusive,
    /// `maximum` together with `exclusiveMaximum`.
    MaximumWithExclusive,
    /// A lower bound that is not strictly below the upper bound.
    LowerNotBelowUpper,
    /// A `multipleOf` that is zero or negative.
    MultipleOfNotPositive,
    /// A rounding other than strict without `multipleOf`.
    RoundingWithoutMultipleOf,
    /// `minLength` above `maxLength`.
    MinLengthAboveMax,
    /// A bound, step or `enum` entry the door cannot hold: a fraction or a value outside an
    /// integer door's range, a decimal past 96 bits or a scale of 28, or a text `enum` entry
    /// longer than `u32::MAX` bytes, which the packed form cannot carry and no input can match
    /// (a judged input that long is `OutOfRange`).
    NotHeldByDoor,
    /// A text `enum` entry that is not valid UTF-8.
    EnumNotUtf8,
    /// Two `enum` entries that are equal (for text, under simple case folding).
    DuplicateEnum,
    /// A layout the core does not read: a `size` other than [`RawConstraints::SIZE`], an
    /// unknown flag bit, rounding code or nonzero `_reserved`, a decimal whose sign byte is
    /// not `0` or `1` (a bound or step read across the C ABI, or a packed integer `enum`
    /// entry), a packed `enum` list cut short (a text entry's length word or bytes, or a
    /// 16-byte integer entry), or an `enum` list passed without
    /// [`HAS_ENUM`](RawConstraints::HAS_ENUM).
    MalformedLayout,
    /// An `enum` declared with no entries, which no value could ever match.
    EmptyEnum,
    /// A text `enum` entry no input can match: an empty entry, or under `trim` one with
    /// leading or trailing Unicode `White_Space` (one that is all `White_Space` included),
    /// since the input is judged trimmed.
    UnmatchableEnumEntry,
}

impl Refusal {
    /// The C ABI's code, `1` to `13`.
    pub const fn code(self) -> u32 {
        self as u32
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Refusal::KeywordNotForDoor => "a constraint the door does not take",
            Refusal::MinimumWithExclusive => "minimum together with exclusiveMinimum",
            Refusal::MaximumWithExclusive => "maximum together with exclusiveMaximum",
            Refusal::LowerNotBelowUpper => "the lower bound is not below the upper bound",
            Refusal::MultipleOfNotPositive => "multipleOf is not positive",
            Refusal::RoundingWithoutMultipleOf => "a rounding without multipleOf",
            Refusal::MinLengthAboveMax => "minLength is above maxLength",
            Refusal::NotHeldByDoor => "a bound, step or enum entry the door cannot hold",
            Refusal::EnumNotUtf8 => "an enum entry that is not UTF-8",
            Refusal::DuplicateEnum => "a duplicate enum entry",
            Refusal::MalformedLayout => "a malformed constraints layout",
            Refusal::EmptyEnum => "an enum with no entries",
            Refusal::UnmatchableEnumEntry => "a text enum entry no input can match",
        })
    }
}

impl core::error::Error for Refusal {}

/// The numeric keywords: the bounds, and `multipleOf` with its rounding. Built from
/// [`new`](Self::new) (or `default()`) and the builder methods, so a keyword that joins in a
/// minor release breaks no caller:
///
/// ```
/// use hypercast::{Constraints, NumericRules, Rounding, decimal_for_declaration};
///
/// let rules = NumericRules::new()
///     .exclusive_minimum(decimal_for_declaration("-90")?)
///     .maximum(decimal_for_declaration("90")?)
///     .multiple_of(decimal_for_declaration("0.000001")?, Rounding::Midpoint);
/// let latitude = Constraints::<f64>::numeric(rules, None)?;
/// # let _ = latitude;
/// # Ok::<(), hypercast::Refusal>(())
/// ```
///
/// Each keyword has a `maybe_` twin taking an `Option`, where `None` declares nothing, for a
/// declaration read from a schema whose keywords may or may not be there. Declaring both
/// `minimum` and `exclusiveMinimum` (or both maximums) is refused by
/// [`Constraints::numeric`] as [`Refusal::MinimumWithExclusive`]
/// ([`MaximumWithExclusive`](Refusal::MaximumWithExclusive)), as the C ABI refuses it: the
/// later builder call does not quietly replace the earlier.
///
/// ```
/// use hypercast::{Constraints, NumericRules, Refusal, decimal_for_declaration};
///
/// let zero = decimal_for_declaration("0")?;
/// let schema_minimum = None;
/// let rules = NumericRules::new().maybe_minimum(schema_minimum).maybe_exclusive_minimum(Some(zero));
/// assert!(Constraints::<f64>::numeric(rules, None).is_ok());
/// let both = NumericRules::new().minimum(zero).exclusive_minimum(zero);
/// assert_eq!(Constraints::<f64>::numeric(both, None).err(), Some(Refusal::MinimumWithExclusive));
/// # Ok::<(), hypercast::Refusal>(())
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct NumericRules {
    /// `minimum` or `exclusiveMinimum`.
    pub lower: Option<Bound>,
    /// `maximum` or `exclusiveMaximum`.
    pub upper: Option<Bound>,
    /// `multipleOf`, and how a value that is not a multiple is treated.
    pub multiple_of: Option<(Decimal, Rounding)>,
    /// The bounds declared both ways by the builders: [`LOWER_CONFLICT`](Self::LOWER_CONFLICT)
    /// and [`UPPER_CONFLICT`](Self::UPPER_CONFLICT) bits.
    conflict: u8,
}

impl NumericRules {
    const LOWER_CONFLICT: u8 = 1;
    const UPPER_CONFLICT: u8 = 2;

    /// No keyword declared.
    pub const fn new() -> NumericRules {
        NumericRules { lower: None, upper: None, multiple_of: None, conflict: 0 }
    }

    /// `minimum`: the value may equal it. After `exclusive_minimum`, a refused declaration.
    pub const fn minimum(self, value: Decimal) -> NumericRules {
        self.with_lower(Bound::Inclusive(value))
    }

    /// `exclusiveMinimum`: the value must exceed it. After `minimum`, a refused declaration.
    pub const fn exclusive_minimum(self, value: Decimal) -> NumericRules {
        self.with_lower(Bound::Exclusive(value))
    }

    /// `maximum`: the value may equal it. After `exclusive_maximum`, a refused declaration.
    pub const fn maximum(self, value: Decimal) -> NumericRules {
        self.with_upper(Bound::Inclusive(value))
    }

    /// `exclusiveMaximum`: the value must be below it. After `maximum`, a refused
    /// declaration.
    pub const fn exclusive_maximum(self, value: Decimal) -> NumericRules {
        self.with_upper(Bound::Exclusive(value))
    }

    /// `multipleOf`, and how a value that is not a multiple is treated.
    pub const fn multiple_of(mut self, step: Decimal, rounding: Rounding) -> NumericRules {
        self.multiple_of = Some((step, rounding));
        self
    }

    /// [`minimum`](Self::minimum) when `value` is `Some`; `None` declares nothing.
    pub const fn maybe_minimum(self, value: Option<Decimal>) -> NumericRules {
        match value {
            Some(value) => self.minimum(value),
            None => self,
        }
    }

    /// [`exclusive_minimum`](Self::exclusive_minimum) when `value` is `Some`; `None` declares
    /// nothing.
    pub const fn maybe_exclusive_minimum(self, value: Option<Decimal>) -> NumericRules {
        match value {
            Some(value) => self.exclusive_minimum(value),
            None => self,
        }
    }

    /// [`maximum`](Self::maximum) when `value` is `Some`; `None` declares nothing.
    pub const fn maybe_maximum(self, value: Option<Decimal>) -> NumericRules {
        match value {
            Some(value) => self.maximum(value),
            None => self,
        }
    }

    /// [`exclusive_maximum`](Self::exclusive_maximum) when `value` is `Some`; `None` declares
    /// nothing.
    pub const fn maybe_exclusive_maximum(self, value: Option<Decimal>) -> NumericRules {
        match value {
            Some(value) => self.exclusive_maximum(value),
            None => self,
        }
    }

    /// [`multiple_of`](Self::multiple_of) when `step` is `Some`; `None` declares nothing.
    pub const fn maybe_multiple_of(self, step: Option<(Decimal, Rounding)>) -> NumericRules {
        match step {
            Some((step, rounding)) => self.multiple_of(step, rounding),
            None => self,
        }
    }

    /// Sets the lower bound, recording a conflict when the other kind was declared.
    const fn with_lower(mut self, bound: Bound) -> NumericRules {
        if differs(self.lower, bound) {
            self.conflict |= Self::LOWER_CONFLICT;
        }
        self.lower = Some(bound);
        self
    }

    /// Sets the upper bound, recording a conflict when the other kind was declared.
    const fn with_upper(mut self, bound: Bound) -> NumericRules {
        if differs(self.upper, bound) {
            self.conflict |= Self::UPPER_CONFLICT;
        }
        self.upper = Some(bound);
        self
    }
}

/// Whether `declared` is a bound of the other kind than `bound` (inclusive against
/// exclusive): the same keyword declared twice is one keyword, its later value winning.
const fn differs(declared: Option<Bound>, bound: Bound) -> bool {
    matches!(
        (declared, bound),
        (Some(Bound::Inclusive(_)), Bound::Exclusive(_))
            | (Some(Bound::Exclusive(_)), Bound::Inclusive(_))
    )
}

/// The text keywords: lengths in code points, and whether the text is trimmed of Unicode
/// `White_Space` first. Built from [`new`](Self::new) (or `default()`) and the builder
/// methods, as [`NumericRules`] is:
///
/// ```
/// use hypercast::{Constraints, TextRules, cast_text_checked};
///
/// let rules = TextRules::new().max_length(10).trim(true);
/// let sex = Constraints::text(rules, Some(&["Female", "Male", "Other"]))?;
/// assert_eq!(cast_text_checked(b" MALE ", &sex).map(|c| c.value), Ok("Male"));
/// # Ok::<(), hypercast::Refusal>(())
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct TextRules {
    /// `minLength`.
    pub min_length: Option<u32>,
    /// `maxLength`.
    pub max_length: Option<u32>,
    /// Trim leading and trailing `White_Space` before judging.
    pub trim: bool,
}

impl TextRules {
    /// No keyword declared, and no trim.
    pub const fn new() -> TextRules {
        TextRules { min_length: None, max_length: None, trim: false }
    }

    /// `minLength`, in code points.
    pub const fn min_length(mut self, length: u32) -> TextRules {
        self.min_length = Some(length);
        self
    }

    /// `maxLength`, in code points.
    pub const fn max_length(mut self, length: u32) -> TextRules {
        self.max_length = Some(length);
        self
    }

    /// [`min_length`](Self::min_length) when `length` is `Some`; `None` declares nothing.
    pub const fn maybe_min_length(self, length: Option<u32>) -> TextRules {
        match length {
            Some(length) => self.min_length(length),
            None => self,
        }
    }

    /// [`max_length`](Self::max_length) when `length` is `Some`; `None` declares nothing.
    pub const fn maybe_max_length(self, length: Option<u32>) -> TextRules {
        match length {
            Some(length) => self.max_length(length),
            None => self,
        }
    }

    /// Whether the text is trimmed of leading and trailing `White_Space` before it is judged.
    pub const fn trim(mut self, trim: bool) -> TextRules {
        self.trim = trim;
        self
    }
}

mod sealed {
    /// Seals [`Numeric`](super::Numeric), and carries what the checked doors read and
    /// deliver through each numeric door.
    pub trait Sealed: crate::checked::Judged {}
}

/// A type a numeric door delivers, naming that door. Sealed: the doors are the crate's.
pub trait Numeric: Copy + sealed::Sealed {
    /// The door that delivers this type.
    const DOOR: Door;
}

macro_rules! numeric {
    ($($ty:ty => $door:ident),+ $(,)?) => {$(
        impl sealed::Sealed for $ty {}
        impl Numeric for $ty {
            const DOOR: Door = Door::$door;
        }
    )+};
}

numeric! {
    i8 => I8, i16 => I16, i32 => I32, i64 => I64,
    u8 => U8, u16 => U16, u32 => U32, u64 => U64,
    f32 => F32, f64 => F64, Decimal => Decimal,
}

/// The text door, as a type a [`Constraints`] is bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Text;

/// A declaration bound to its door, `T` (a [`Numeric`] type, or [`Text`]). Built only
/// through [`numeric`](Constraints::numeric) or [`text`](Constraints::text), which refuse
/// what [`check_constraints`] refuses, so a value of this type is always a valid
/// declaration.
#[derive(Clone, Copy, Debug)]
pub struct Constraints<'e, T> {
    raw: RawConstraints,
    entries: Entries<'e>,
    door: PhantomData<fn() -> T>,
}

impl<'e, T: Numeric> Constraints<'e, T> {
    /// Declares the numeric keywords and an integer `enum` (`None` for none; an empty list is
    /// refused with [`Refusal::EmptyEnum`]) for `T`'s door.
    pub fn numeric(rules: NumericRules, entries: Option<&'e [i128]>) -> Result<Self, Refusal> {
        let mut raw = RawConstraints::ZERO;
        match rules.lower {
            Some(Bound::Inclusive(value)) => {
                raw.flags |= RawConstraints::HAS_MINIMUM;
                raw.lower = value;
            }
            Some(Bound::Exclusive(value)) => {
                raw.flags |= RawConstraints::HAS_EXCLUSIVE_MINIMUM;
                raw.lower = value;
            }
            None => {}
        }
        match rules.upper {
            Some(Bound::Inclusive(value)) => {
                raw.flags |= RawConstraints::HAS_MAXIMUM;
                raw.upper = value;
            }
            Some(Bound::Exclusive(value)) => {
                raw.flags |= RawConstraints::HAS_EXCLUSIVE_MAXIMUM;
                raw.upper = value;
            }
            None => {}
        }
        // A bound the builders saw declared both ways carries both flags, so the judgement
        // refuses it where the C ABI would, in code order.
        if rules.conflict & NumericRules::LOWER_CONFLICT != 0 {
            raw.flags |= RawConstraints::HAS_MINIMUM | RawConstraints::HAS_EXCLUSIVE_MINIMUM;
        }
        if rules.conflict & NumericRules::UPPER_CONFLICT != 0 {
            raw.flags |= RawConstraints::HAS_MAXIMUM | RawConstraints::HAS_EXCLUSIVE_MAXIMUM;
        }
        if let Some((step, rounding)) = rules.multiple_of {
            raw.flags |= RawConstraints::HAS_MULTIPLE_OF;
            raw.multiple_of = step;
            raw.rounding = rounding as u32;
        }
        let entries = match entries {
            Some(entries) => {
                raw.flags |= RawConstraints::HAS_ENUM;
                Entries::Integers(entries)
            }
            None => Entries::None,
        };
        Self::checked(T::DOOR, raw, entries)
    }

    /// Reads a declaration in its C ABI form for `T`'s door: `raw`, and the integer `enum`
    /// list packed as the C ABI passes it (16-byte [`Decimal`]s; empty for none). Refused
    /// exactly where [`check_constraints`] refuses, duplicates included. For a parser that
    /// carries declarations across its own C ABI and judges each value with the checked doors.
    ///
    /// ```
    /// use hypercast::{Constraints, NumericRules, Rounding, decimal_for_declaration};
    ///
    /// let degrees = |text| decimal_for_declaration(text).unwrap();
    /// let rules = NumericRules::new()
    ///     .minimum(degrees("-90"))
    ///     .maximum(degrees("90"))
    ///     .multiple_of(degrees("0.000001"), Rounding::Midpoint);
    /// let latitude = Constraints::<f64>::numeric(rules, None).unwrap();
    /// // Across a C ABI and back: the struct, and no `enum` list.
    /// let read = Constraints::<f64>::from_raw(latitude.raw(), &[]).unwrap();
    /// let checked = hypercast::cast_checked("51.47784461", &Default::default(), &read).unwrap();
    /// assert_eq!((checked.value, checked.adjusted), (51.477845, true));
    /// ```
    pub fn from_raw(raw: RawConstraints, entries: &'e [u8]) -> Result<Self, Refusal> {
        Self::checked(T::DOOR, raw, Entries::packed(T::DOOR, entries))
    }
}

impl<'e> Constraints<'e, Text> {
    /// Reads a declaration in its C ABI form for the text door: `raw`, and the text `enum`
    /// list packed as the C ABI passes it (each entry a little-endian `u32` byte length, then
    /// its bytes; empty for none). Refused exactly where [`check_constraints`] refuses,
    /// duplicates included.
    ///
    /// ```
    /// use hypercast::{Constraints, Text};
    ///
    /// let mut packed = Vec::new();
    /// for entry in ["Female", "Male"] {
    ///     packed.extend_from_slice(&(entry.len() as u32).to_le_bytes());
    ///     packed.extend_from_slice(entry.as_bytes());
    /// }
    /// let raw = hypercast::RawConstraints {
    ///     flags: hypercast::RawConstraints::HAS_ENUM,
    ///     ..hypercast::RawConstraints::ZERO
    /// };
    /// let sexes = Constraints::<Text>::from_raw(raw, &packed).unwrap();
    /// let checked = hypercast::cast_text_checked(b"MALE", &sexes).unwrap();
    /// assert_eq!((checked.value, checked.enum_index), ("Male", Some(1)));
    /// ```
    pub fn from_raw(raw: RawConstraints, entries: &'e [u8]) -> Result<Self, Refusal> {
        Self::checked(Door::Text, raw, Entries::packed(Door::Text, entries))
    }
    /// Declares the text keywords and a text `enum` (`None` for none; an empty list is
    /// refused with [`Refusal::EmptyEnum`]) for the text door.
    pub fn text(rules: TextRules, entries: Option<&'e [&'e str]>) -> Result<Self, Refusal> {
        let mut raw = RawConstraints::ZERO;
        if let Some(length) = rules.min_length {
            raw.flags |= RawConstraints::HAS_MIN_LENGTH;
            raw.min_length = length;
        }
        if let Some(length) = rules.max_length {
            raw.flags |= RawConstraints::HAS_MAX_LENGTH;
            raw.max_length = length;
        }
        if rules.trim {
            raw.flags |= RawConstraints::TRIM;
        }
        let entries = match entries {
            Some(entries) => {
                raw.flags |= RawConstraints::HAS_ENUM;
                Entries::Text(entries)
            }
            None => Entries::None,
        };
        Self::checked(Door::Text, raw, entries)
    }
}

impl<'e, T> Constraints<'e, T> {
    fn checked(door: Door, raw: RawConstraints, entries: Entries<'e>) -> Result<Self, Refusal> {
        check(door, &raw, &entries, true)?;
        Ok(Constraints { raw, entries, door: PhantomData })
    }

    /// The declaration in its ABI layout (the `enum` list travels beside it, and
    /// [`HAS_ENUM`](RawConstraints::HAS_ENUM) says whether one was declared).
    pub fn raw(&self) -> RawConstraints {
        self.raw
    }

    /// The `enum` entries.
    pub(crate) fn entries(&self) -> &Entries<'e> {
        &self.entries
    }

    /// Packs the `enum` list into `out` as the C ABI passes it beside [`raw`](Self::raw), and
    /// returns the bytes the packed list takes (`Some(0)` for no `enum`): integers as 16-byte
    /// [`Decimal`]s in their native layout, text as a little-endian `u32` byte length then the
    /// bytes, and a list read by [`from_raw`](Self::from_raw) as it was given.
    ///
    /// When `out` is shorter than that, nothing is written and the length is still returned,
    /// so a caller sizes its buffer and calls again. `None` when the packed list would take
    /// more than `usize::MAX` bytes, which only a borrowed text list on a 32-bit target can
    /// reach; nothing is written then either. Never allocates.
    ///
    /// ```
    /// use hypercast::{Constraints, Text, TextRules};
    ///
    /// let sexes = Constraints::text(TextRules::new(), Some(&["Female", "Male"])).unwrap();
    /// let len = sexes.pack_enum(&mut []).unwrap();
    /// let mut packed = vec![0; len];
    /// assert_eq!(sexes.pack_enum(&mut packed), Some(len));
    /// let read = Constraints::<Text>::from_raw(sexes.raw(), &packed).unwrap();
    /// assert_eq!(hypercast::cast_text_checked(b"male", &read).unwrap().value, "Male");
    /// ```
    pub fn pack_enum(&self, out: &mut [u8]) -> Option<usize> {
        let len = self.entries.packed_len()?;
        let Some(out) = out.get_mut(..len) else {
            return Some(len);
        };
        match self.entries {
            Entries::None => {}
            Entries::PackedIntegers(bytes) | Entries::PackedText(bytes) => {
                if let Some(out) = out.get_mut(..bytes.len()) {
                    out.copy_from_slice(bytes);
                }
            }
            Entries::Integers(entries) => {
                let (chunks, _) = out.as_chunks_mut::<PACKED_DECIMAL>();
                for (&entry, chunk) in entries.iter().zip(chunks) {
                    *chunk = packed_integer(entry);
                }
            }
            Entries::Text(entries) => {
                let mut rest = out;
                for entry in entries {
                    // The checker refused any entry longer than `u32::MAX` bytes, and `len`
                    // counted every byte written here, so neither split comes up short.
                    let Some((length, tail)) = rest.split_at_mut_checked(4) else { break };
                    let Some((bytes, tail)) = tail.split_at_mut_checked(entry.len()) else { break };
                    length.copy_from_slice(&(entry.len() as u32).to_le_bytes());
                    bytes.copy_from_slice(entry.as_bytes());
                    rest = tail;
                }
            }
        }
        Some(len)
    }
}

/// An integer `enum` entry as the C ABI packs it: a [`Decimal`] at scale 0 in its native
/// layout (`lo`, `hi`, `scale`, the sign byte, two zero bytes). The checker held every entry to
/// an integer door's range, so the magnitude fits the 96 bits `lo` and `hi` carry.
fn packed_integer(entry: i128) -> [u8; PACKED_DECIMAL] {
    let magnitude = entry.unsigned_abs();
    let [l0, l1, l2, l3, l4, l5, l6, l7] = (magnitude as u64).to_ne_bytes();
    let [h0, h1, h2, h3] = ((magnitude >> 64) as u32).to_ne_bytes();
    [l0, l1, l2, l3, l4, l5, l6, l7, h0, h1, h2, h3, 0, u8::from(entry < 0), 0, 0]
}

/// The bytes a packed text list takes for entries of these byte lengths: `4 + len` each, or
/// `None` past `usize::MAX`.
pub(crate) fn packed_text_len(mut lengths: impl Iterator<Item = usize>) -> Option<usize> {
    lengths.try_fold(0usize, |total, len| total.checked_add(len)?.checked_add(4))
}

/// An `enum` list: as Rust values, or packed as the C ABI passes it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Entries<'e> {
    /// No `enum`.
    None,
    /// Integer entries.
    Integers(&'e [i128]),
    /// Text entries.
    Text(&'e [&'e str]),
    /// Integer entries as the ABI packs them: [`Decimal`]s in their native layout, 16 bytes
    /// each, holding whole numbers at any scale (`7.0` is `7`).
    PackedIntegers(&'e [u8]),
    /// Text entries as the ABI packs them: each a little-endian `u32` byte length, then
    /// that many bytes, with no padding.
    PackedText(&'e [u8]),
}

impl<'e> Entries<'e> {
    /// The `enum` list the ABI passes for `door`: packed text at the text door, packed
    /// integers anywhere else (where a door takes no integer `enum`, any entry is refused).
    /// Read only under [`HAS_ENUM`](RawConstraints::HAS_ENUM); empty is no entries.
    /// The bytes the list takes packed as the C ABI passes it, or `None` past `usize::MAX`
    /// (only a borrowed text list on a 32-bit target gets there: a list of `i128`s lives in
    /// memory, and its packing takes no more bytes than it does).
    fn packed_len(&self) -> Option<usize> {
        match self {
            Entries::None => Some(0),
            Entries::Integers(entries) => entries.len().checked_mul(PACKED_DECIMAL),
            Entries::Text(entries) => packed_text_len(entries.iter().map(|entry| entry.len())),
            Entries::PackedIntegers(bytes) | Entries::PackedText(bytes) => Some(bytes.len()),
        }
    }

    pub(crate) fn packed(door: Door, bytes: &'e [u8]) -> Entries<'e> {
        match (bytes.is_empty(), door) {
            (true, _) => Entries::None,
            (false, Door::Text) => Entries::PackedText(bytes),
            (false, _) => Entries::PackedIntegers(bytes),
        }
    }

    /// The number of entries; a packed list cut short counts its whole entries only.
    pub(crate) fn len(&self) -> usize {
        match self {
            Entries::None => 0,
            Entries::Integers(entries) => entries.len(),
            Entries::Text(entries) => entries.len(),
            Entries::PackedIntegers(bytes) => bytes.len() / PACKED_DECIMAL,
            Entries::PackedText(_) => self.texts().count(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Entries::None => true,
            Entries::Integers(entries) => entries.is_empty(),
            Entries::Text(entries) => entries.is_empty(),
            Entries::PackedIntegers(bytes) | Entries::PackedText(bytes) => bytes.is_empty(),
        }
    }

    /// Entry `i` as an integer, or `None` past the end, for a text list, or for a packed
    /// entry with a fractional part or a scale past 28.
    pub(crate) fn integer(&self, i: usize) -> Option<i128> {
        match self {
            Entries::Integers(entries) => entries.get(i).copied(),
            Entries::PackedIntegers(_) => integral(&self.decimal(i)?),
            _ => None,
        }
    }

    /// Packed integer entry `i` as the [`Decimal`] it spells, and whether its sign byte is
    /// a valid `bool` (`0` or `1`).
    fn packed_decimal(&self, i: usize) -> Option<(Decimal, bool)> {
        let Entries::PackedIntegers(bytes) = self else {
            return None;
        };
        let start = i.checked_mul(PACKED_DECIMAL)?;
        let chunk = bytes.get(start..start.checked_add(PACKED_DECIMAL)?)?;
        let chunk: &[u8; PACKED_DECIMAL] = chunk.try_into().ok()?;
        let [l0, l1, l2, l3, l4, l5, l6, l7, h0, h1, h2, h3, scale, sign, _, _] = *chunk;
        let decimal = Decimal {
            lo: u64::from_ne_bytes([l0, l1, l2, l3, l4, l5, l6, l7]),
            hi: u32::from_ne_bytes([h0, h1, h2, h3]),
            scale,
            negative: sign != 0,
        };
        Some((decimal, sign <= 1))
    }

    fn decimal(&self, i: usize) -> Option<Decimal> {
        self.packed_decimal(i).map(|(decimal, _)| decimal)
    }

    /// Entry `i` as text bytes, or `None` past the end, for an integer list, or past a
    /// packed list cut short. Walks a packed list from its start: [`texts`](Self::texts)
    /// reads them in order. The checked doors walk [`texts`](Self::texts) instead.
    #[cfg(test)]
    pub(crate) fn text(&self, i: usize) -> Option<&'e [u8]> {
        match self {
            Entries::Text(entries) => entries.get(i).map(|entry| entry.as_bytes()),
            Entries::PackedText(_) => self.texts().nth(i),
            _ => None,
        }
    }

    /// The text entries in order, stopping where a packed list is cut short.
    pub(crate) fn texts(&self) -> Texts<'e> {
        match *self {
            Entries::Text(entries) => Texts::Slice(entries.iter()),
            Entries::PackedText(bytes) => Texts::Packed(bytes),
            _ => Texts::Packed(&[]),
        }
    }
}

/// The text entries of an [`Entries`], in order.
#[derive(Clone, Debug)]
pub(crate) enum Texts<'e> {
    Slice(core::slice::Iter<'e, &'e str>),
    Packed(&'e [u8]),
}

impl<'e> Iterator for Texts<'e> {
    type Item = &'e [u8];

    fn next(&mut self) -> Option<&'e [u8]> {
        match self {
            Texts::Slice(entries) => entries.next().map(|entry| entry.as_bytes()),
            Texts::Packed(rest) => match next_packed(rest) {
                Some((entry, tail)) => {
                    *rest = tail;
                    Some(entry)
                }
                None => {
                    *rest = &[];
                    None
                }
            },
        }
    }
}

/// The first packed text entry and what follows it, or `None` when none is whole.
fn next_packed(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    let (length, rest) = bytes.split_first_chunk::<4>()?;
    let length = usize::try_from(u32::from_le_bytes(*length)).ok()?;
    rest.split_at_checked(length)
}

/// Whether a packed text list is whole: every entry's length word and bytes present.
fn packed_text_is_whole(mut bytes: &[u8]) -> bool {
    while !bytes.is_empty() {
        match next_packed(bytes) {
            Some((_, rest)) => bytes = rest,
            None => return false,
        }
    }
    true
}

/// A decimal's value as an integer, or `None` for a fraction or a scale past 28.
fn integral(decimal: &Decimal) -> Option<i128> {
    if decimal.scale > MAX_SCALE {
        return None;
    }
    let power = 10u128.checked_pow(decimal.scale.into())?;
    let magnitude = decimal.magnitude();
    if magnitude.checked_rem(power)? != 0 {
        return None;
    }
    let whole = i128::try_from(magnitude.checked_div(power)?).ok()?;
    Some(if decimal.negative { -whole } else { whole })
}

/// The range of an integer door; `None` for every other door.
fn integer_range(door: Door) -> Option<(i128, i128)> {
    Some(match door {
        Door::I8 => (i8::MIN.into(), i8::MAX.into()),
        Door::I16 => (i16::MIN.into(), i16::MAX.into()),
        Door::I32 => (i32::MIN.into(), i32::MAX.into()),
        Door::I64 => (i64::MIN.into(), i64::MAX.into()),
        Door::U8 => (0, u8::MAX.into()),
        Door::U16 => (0, u16::MAX.into()),
        Door::U32 => (0, u32::MAX.into()),
        Door::U64 => (0, u64::MAX.into()),
        _ => return None,
    })
}

/// Whether `door` can hold a declared decimal: any decimal of scale 28 or less at a float or
/// decimal door (every one lies within `f32`'s range), and a whole one in range at an
/// integer door.
fn held(door: Door, decimal: &Decimal) -> bool {
    match integer_range(door) {
        Some((min, max)) => integral(decimal).is_some_and(|value| (min..=max).contains(&value)),
        None => decimal.scale <= MAX_SCALE,
    }
}

/// Judges a declaration against its door, duplicates included: `Ok` when every rule holds,
/// otherwise the first rule it breaks, in [`Refusal`]'s code order. `entries` is the `enum`
/// list as the C ABI packs it, read only under [`RawConstraints::HAS_ENUM`] (and then
/// empty is an `enum` with no entries, [`Refusal::EmptyEnum`]): at the text door, each entry
/// a little-endian `u32` byte length then that many bytes; at an integer door, [`Decimal`]s
/// in their native 16-byte layout, holding whole numbers at any scale (`7.0` is `7`). A
/// non-empty list without `HAS_ENUM` is [`Refusal::MalformedLayout`].
///
/// One exception to the order: a `size` other than [`RawConstraints::SIZE`] is refused as
/// [`Refusal::MalformedLayout`] before anything else, since no other field of a layout of
/// unknown size can be read (the C ABI judges a decimal's sign byte there too, before it
/// reads the layout; a Rust `bool` holds no other byte). Code order also settles the `enum` refusals among themselves:
/// an entry that is not UTF-8 (9) before a duplicate (10), before a list cut short (11),
/// before an empty list (12), before a text entry no input can match (13), so `["", ""]` is
/// a duplicate.
pub fn check_constraints(door: Door, raw: &RawConstraints, entries: &[u8]) -> Result<(), Refusal> {
    check(door, raw, &Entries::packed(door, entries), true)
}

/// [`check_constraints`] without the duplicate pass, which the checked doors run on every
/// call; there the first matching entry wins.
pub(crate) fn check_without_duplicates(
    door: Door,
    raw: &RawConstraints,
    entries: &Entries,
) -> Result<(), Refusal> {
    check(door, raw, entries, false)
}

fn check(
    door: Door,
    raw: &RawConstraints,
    entries: &Entries,
    duplicates: bool,
) -> Result<(), Refusal> {
    if raw.size != RawConstraints::SIZE {
        return Err(Refusal::MalformedLayout);
    }
    let flags = raw.flags;
    let has = |flag: u32| flags & flag != 0;
    // The list is read only when declared; one passed without the flag is judged at 11.
    let listed = has(RawConstraints::HAS_ENUM);
    let passed = entries;
    let entries = if listed { entries } else { &Entries::None };
    let integer_door = integer_range(door).is_some();
    let number_door = matches!(door, Door::F32 | Door::F64 | Door::Decimal);

    // 1: a keyword the door does not take.
    let not_for_door = if integer_door {
        has(TEXT_FLAGS) || matches!(entries, Entries::Text(_) | Entries::PackedText(_))
    } else if number_door {
        has(TEXT_FLAGS) || listed
    } else if door == Door::Text {
        has(NUMERIC_FLAGS) || matches!(entries, Entries::Integers(_) | Entries::PackedIntegers(_))
    } else {
        has(KNOWN_FLAGS)
    };
    if not_for_door {
        return Err(Refusal::KeywordNotForDoor);
    }

    // 2 and 3: an inclusive and an exclusive bound on the same side.
    if has(RawConstraints::HAS_MINIMUM) && has(RawConstraints::HAS_EXCLUSIVE_MINIMUM) {
        return Err(Refusal::MinimumWithExclusive);
    }
    if has(RawConstraints::HAS_MAXIMUM) && has(RawConstraints::HAS_EXCLUSIVE_MAXIMUM) {
        return Err(Refusal::MaximumWithExclusive);
    }

    // 4: the lower bound strictly below the upper, whichever kinds they are.
    let lower = has(RawConstraints::HAS_MINIMUM | RawConstraints::HAS_EXCLUSIVE_MINIMUM);
    let upper = has(RawConstraints::HAS_MAXIMUM | RawConstraints::HAS_EXCLUSIVE_MAXIMUM);
    if lower && upper && compare_decimals(&raw.lower, &raw.upper) != Ordering::Less {
        return Err(Refusal::LowerNotBelowUpper);
    }

    // 5: a positive step.
    let step = has(RawConstraints::HAS_MULTIPLE_OF);
    if step && (raw.multiple_of.magnitude() == 0 || raw.multiple_of.negative) {
        return Err(Refusal::MultipleOfNotPositive);
    }

    // 6: a rounding only with a step.
    if raw.rounding != Rounding::Strict as u32 && !step {
        return Err(Refusal::RoundingWithoutMultipleOf);
    }

    // 7: lengths in order; equal is allowed.
    if has(RawConstraints::HAS_MIN_LENGTH)
        && has(RawConstraints::HAS_MAX_LENGTH)
        && raw.min_length > raw.max_length
    {
        return Err(Refusal::MinLengthAboveMax);
    }

    // 8: every bound, step and integer entry held by the door, and every text entry short
    // enough for the packed form's `u32` length.
    if (lower && !held(door, &raw.lower))
        || (upper && !held(door, &raw.upper))
        || (step && !held(door, &raw.multiple_of))
        || !integer_entries_held(door, entries)
        || entries.texts().any(|entry| u32::try_from(entry.len()).is_err())
    {
        return Err(Refusal::NotHeldByDoor);
    }

    // 9: every text entry UTF-8.
    if entries.texts().any(|entry| core::str::from_utf8(entry).is_err()) {
        return Err(Refusal::EnumNotUtf8);
    }

    // 10: no duplicates; only at declaration.
    if duplicates && has_duplicates(entries) {
        return Err(Refusal::DuplicateEnum);
    }

    // 11: nothing the layout does not define.
    let whole = match entries {
        Entries::PackedIntegers(bytes) => {
            bytes.len() % PACKED_DECIMAL == 0
                && (0..entries.len())
                    .all(|i| entries.packed_decimal(i).is_some_and(|(_, sign)| sign))
        }
        Entries::PackedText(bytes) => packed_text_is_whole(bytes),
        _ => true,
    };
    if flags & !KNOWN_FLAGS != 0
        || raw.rounding > Rounding::Ceiling as u32
        || raw._reserved != 0
        || !whole
        || (!listed && !passed.is_empty())
    {
        return Err(Refusal::MalformedLayout);
    }

    // 12: an `enum` with entries.
    if listed && entries.is_empty() {
        return Err(Refusal::EmptyEnum);
    }

    // 13: every text entry one some input can match.
    let trimmed = has(RawConstraints::TRIM);
    if entries.texts().any(|entry| !matchable(entry, trimmed)) {
        return Err(Refusal::UnmatchableEnumEntry);
    }
    Ok(())
}

/// Whether some input can match a text `enum` entry: it is not empty, and under `trim` it has
/// no leading or trailing `White_Space`, which the judged input never has. An entry that is
/// not UTF-8 (refused before this is asked) answers true.
fn matchable(entry: &[u8], trimmed: bool) -> bool {
    match core::str::from_utf8(entry) {
        Ok(text) => !text.is_empty() && (!trimmed || trim_white_space(text).0.len() == text.len()),
        Err(_) => true,
    }
}

/// Whether every integer entry is whole and within the door's range. Only integer doors
/// reach this with integer entries.
fn integer_entries_held(door: Door, entries: &Entries) -> bool {
    let Some((min, max)) = integer_range(door) else {
        return true;
    };
    match entries {
        Entries::Integers(values) => values.iter().all(|value| (min..=max).contains(value)),
        Entries::PackedIntegers(_) => (0..entries.len())
            .all(|i| entries.integer(i).is_some_and(|value| (min..=max).contains(&value))),
        _ => true,
    }
}

/// Whether two entries are equal: integers by value, text under simple case folding. O(n²)
/// with no scratch space; `enum` lists are short.
fn has_duplicates(entries: &Entries) -> bool {
    match entries {
        Entries::Integers(_) | Entries::PackedIntegers(_) => {
            let count = entries.len();
            (0..count).any(|i| {
                let a = entries.integer(i);
                (i.saturating_add(1)..count).any(|j| entries.integer(j) == a)
            })
        }
        Entries::Text(_) | Entries::PackedText(_) => {
            let mut rest = entries.texts();
            while let Some(a) = rest.next() {
                if rest.clone().any(|b| eq_folded(a, b)) {
                    return true;
                }
            }
            false
        }
        Entries::None => false,
    }
}

/// A decimal declared in invariant notation (an optional sign, digits with an optional `.`
/// and fraction, an optional exponent: `0.000001`, `-90`, `1e-6`), converted exactly.
/// Trailing zeros are dropped (`1.50` is `1.5`, scale 1), so only significant digits count
/// towards the limits. Refused with [`Refusal::NotHeldByDoor`] when the value does not fit
/// a [`Decimal`] (a magnitude past 96 bits or a scale past 28) and when the text is not in
/// that notation. The bindings convert a declared bound, step or integer `enum` entry
/// through the C export `cast_declared_decimal`, which runs this same conversion.
pub fn decimal_for_declaration(text: &str) -> Result<Decimal, Refusal> {
    declared_decimal(text.as_bytes())
}

/// [`decimal_for_declaration`] over bytes, for the C export: anything but the ASCII of
/// invariant notation, invalid UTF-8 included, is [`Refusal::NotHeldByDoor`].
pub(crate) fn declared_decimal(bytes: &[u8]) -> Result<Decimal, Refusal> {
    const REFUSED: Refusal = Refusal::NotHeldByDoor;
    let (negative, mut rest) = split_sign(bytes);
    // The significant digits read so far, without their trailing zeros, which `pending`
    // counts until a nonzero digit follows them.
    let mut digits: u128 = 0;
    let mut pending: u32 = 0;
    let mut any_digit = false;
    let mut fraction: i64 = 0;
    let mut in_fraction = false;
    loop {
        match rest.split_first() {
            Some((&byte, tail)) if byte.is_ascii_digit() => {
                any_digit = true;
                if in_fraction {
                    fraction = fraction.saturating_add(1);
                }
                let digit = byte - b'0';
                if digit == 0 {
                    if digits != 0 {
                        pending = pending.saturating_add(1);
                    }
                } else {
                    digits = 10u128
                        .checked_pow(pending.saturating_add(1))
                        .and_then(|power| digits.checked_mul(power))
                        .and_then(|value| value.checked_add(digit.into()))
                        .filter(|&value| value <= MAX_MAGNITUDE)
                        .ok_or(REFUSED)?;
                    pending = 0;
                }
                rest = tail;
            }
            Some((b'.', tail)) if !in_fraction => {
                in_fraction = true;
                rest = tail;
            }
            _ => break,
        }
    }
    if !any_digit {
        return Err(REFUSED);
    }
    let mut exponent: i64 = 0;
    if let Some((b'e' | b'E', tail)) = rest.split_first() {
        let (exponent_negative, mut tail) = split_sign(tail);
        let mut any = false;
        while let Some((&byte, after)) = tail.split_first() {
            if !byte.is_ascii_digit() {
                break;
            }
            any = true;
            // Saturates far past any exponent that could fit: the value is refused either way.
            exponent =
                exponent.saturating_mul(10).saturating_add((byte - b'0').into()).min(1 << 40);
            tail = after;
        }
        if !any {
            return Err(REFUSED);
        }
        if exponent_negative {
            exponent = -exponent;
        }
        rest = tail;
    }
    if !rest.is_empty() {
        return Err(REFUSED);
    }
    if digits == 0 {
        return Ok(Decimal { lo: 0, hi: 0, scale: 0, negative: false });
    }
    // The value is `digits · 10^power`.
    let power = exponent.saturating_sub(fraction).saturating_add(pending.into());
    let (magnitude, scale) = if power >= 0 {
        let magnitude = u32::try_from(power)
            .ok()
            .and_then(|power| 10u128.checked_pow(power))
            .and_then(|scale| digits.checked_mul(scale))
            .filter(|&value| value <= MAX_MAGNITUDE)
            .ok_or(REFUSED)?;
        (magnitude, 0)
    } else {
        let scale = u8::try_from(power.unsigned_abs())
            .ok()
            .filter(|&scale| scale <= MAX_SCALE)
            .ok_or(REFUSED)?;
        (digits, scale)
    };
    Ok(Decimal { lo: magnitude as u64, hi: (magnitude >> 64) as u32, scale, negative })
}

fn split_sign(bytes: &[u8]) -> (bool, &[u8]) {
    match bytes.split_first() {
        Some((b'-', rest)) => (true, rest),
        Some((b'+', rest)) => (false, rest),
        _ => (false, bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    use crate::abi::{RawCheck, RawSpan};

    fn dec(text: &str) -> Decimal {
        decimal_for_declaration(text).unwrap()
    }

    /// A layout with `flags` and nothing else.
    fn raw(flags: u32) -> RawConstraints {
        RawConstraints { flags, ..RawConstraints::ZERO }
    }

    fn bounded(flags: u32, lower: &str, upper: &str) -> RawConstraints {
        RawConstraints { flags, lower: dec(lower), upper: dec(upper), ..RawConstraints::ZERO }
    }

    fn stepped(step: &str) -> RawConstraints {
        RawConstraints {
            flags: RawConstraints::HAS_MULTIPLE_OF,
            multiple_of: dec(step),
            ..RawConstraints::ZERO
        }
    }

    fn pack_text(entries: &[&[u8]]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for entry in entries {
            bytes.extend_from_slice(&(entry.len() as u32).to_le_bytes());
            bytes.extend_from_slice(entry);
        }
        bytes
    }

    fn pack_integers(entries: &[&str]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for entry in entries {
            let decimal = dec(entry);
            bytes.extend_from_slice(&decimal.lo.to_ne_bytes());
            bytes.extend_from_slice(&decimal.hi.to_ne_bytes());
            bytes.extend_from_slice(&[decimal.scale, decimal.negative.into(), 0, 0]);
        }
        bytes
    }

    const MIN: u32 = RawConstraints::HAS_MINIMUM;
    const XMIN: u32 = RawConstraints::HAS_EXCLUSIVE_MINIMUM;
    const MAX: u32 = RawConstraints::HAS_MAXIMUM;
    const XMAX: u32 = RawConstraints::HAS_EXCLUSIVE_MAXIMUM;
    const ENUM: u32 = RawConstraints::HAS_ENUM;
    const TRIM: u32 = RawConstraints::TRIM;
    /// A flag bit the layout does not define.
    const UNKNOWN: u32 = 512;

    #[test]
    fn layouts_are_pinned() {
        assert_eq!(size_of::<RawConstraints>(), 72);
        assert_eq!(align_of::<RawConstraints>(), 8);
        assert_eq!(offset_of!(RawConstraints, lower), 24);
        assert_eq!(offset_of!(RawConstraints, upper), 40);
        assert_eq!(offset_of!(RawConstraints, multiple_of), 56);
        assert_eq!(RawConstraints::SIZE as usize, size_of::<RawConstraints>());
        assert_eq!(size_of::<RawCheck>(), 12);
        assert_eq!(align_of::<RawCheck>(), 4);
        assert_eq!(size_of::<RawSpan>(), 8);
        assert_eq!(align_of::<RawSpan>(), 4);
    }

    #[test]
    fn each_refusal_has_its_code() {
        use Refusal::*;
        let mut min_length = raw(RawConstraints::HAS_MIN_LENGTH | RawConstraints::HAS_MAX_LENGTH);
        min_length.min_length = 3;
        min_length.max_length = 2;
        let rounding = RawConstraints { rounding: 1, ..RawConstraints::ZERO };
        let short = RawConstraints { size: 71, ..RawConstraints::ZERO };
        let reserved = RawConstraints { _reserved: 1, ..RawConstraints::ZERO };
        let none: &[u8] = &[];
        let not_utf8 = pack_text(&[&[0xFF]]);
        let duplicate = pack_text(&[b"Male", b"MALE"]);
        // Each case breaks one rule, or several where the first in code order must win.
        let cases: &[(Door, RawConstraints, &[u8], Refusal)] = &[
            (Door::I32, raw(RawConstraints::HAS_MIN_LENGTH), none, KeywordNotForDoor),
            // 1 before 2: a text keyword and both minimums.
            (Door::I32, raw(MIN | XMIN | RawConstraints::TRIM), none, KeywordNotForDoor),
            // An `enum` at a float door, even an empty one (1 before 12).
            (Door::F64, raw(ENUM), &pack_integers(&["1"]), KeywordNotForDoor),
            (Door::F64, raw(ENUM), none, KeywordNotForDoor),
            (Door::I32, raw(MIN | XMIN), none, MinimumWithExclusive),
            // 2 before 3, and before 4 (lower 0 is not below upper 0).
            (Door::I32, raw(MIN | XMIN | MAX | XMAX), none, MinimumWithExclusive),
            (Door::I32, raw(MAX | XMAX), none, MaximumWithExclusive),
            // 3 before 4.
            (Door::I32, raw(MIN | MAX | XMAX), none, MaximumWithExclusive),
            (Door::I32, bounded(MIN | MAX, "5", "5"), none, LowerNotBelowUpper),
            (Door::I32, bounded(MIN | XMAX, "5", "5"), none, LowerNotBelowUpper),
            // 4 before 8: a fractional bound on an integer door.
            (Door::I32, bounded(MIN | MAX, "5.5", "5"), none, LowerNotBelowUpper),
            (Door::I32, stepped("0"), none, MultipleOfNotPositive),
            // 5 before 8: -0.05 is no integer either.
            (Door::I32, stepped("-0.05"), none, MultipleOfNotPositive),
            (Door::I32, rounding, none, RoundingWithoutMultipleOf),
            // 6 before 11: an unknown flag bit too.
            (
                Door::I32,
                RawConstraints { flags: UNKNOWN, ..rounding },
                none,
                RoundingWithoutMultipleOf,
            ),
            (Door::Text, min_length, none, MinLengthAboveMax),
            (Door::U8, bounded(MIN, "-1", "0"), none, NotHeldByDoor),
            (Door::U8, bounded(MAX, "0", "256"), none, NotHeldByDoor),
            (Door::I8, bounded(MIN, "-129", "0"), none, NotHeldByDoor),
            (Door::I32, bounded(MIN, "0.5", "0"), none, NotHeldByDoor),
            (Door::I32, stepped("0.5"), none, NotHeldByDoor),
            (Door::U8, raw(ENUM), &pack_integers(&["256"]), NotHeldByDoor),
            (Door::I32, raw(ENUM), &pack_integers(&["0.5"]), NotHeldByDoor),
            (Door::Text, raw(ENUM), &not_utf8, EnumNotUtf8),
            // 9 before 10: a duplicate pair after the bad entry.
            (Door::Text, raw(ENUM), &pack_text(&[&[0xFF], b"a", b"A"]), EnumNotUtf8),
            // 9 before 13: a bad entry beside an empty one.
            (Door::Text, raw(ENUM), &pack_text(&[b"", &[0xFF]]), EnumNotUtf8),
            (Door::Text, raw(ENUM), &duplicate, DuplicateEnum),
            (Door::I32, raw(ENUM), &pack_integers(&["7", "7.0"]), DuplicateEnum),
            // 10 before 13: two empty entries are a duplicate.
            (Door::Text, raw(ENUM), &pack_text(&[b"", b""]), DuplicateEnum),
            (Door::Text, raw(ENUM | TRIM), &pack_text(&[b" a", b" A"]), DuplicateEnum),
            // 10 before 11: an unknown flag bit, or a cut-short entry after the pair.
            (Door::Text, raw(ENUM | UNKNOWN), &duplicate, DuplicateEnum),
            (Door::Text, raw(ENUM), &[&duplicate[..], &[9, 0]].concat(), DuplicateEnum),
            (Door::I32, short, none, MalformedLayout),
            (Door::I32, raw(UNKNOWN), none, MalformedLayout),
            (Door::I32, reserved, none, MalformedLayout),
            (Door::I32, RawConstraints { rounding: 4, ..stepped("1") }, none, MalformedLayout),
            // Packed lists cut short: a length word, an entry's bytes, a decimal.
            (Door::Text, raw(ENUM), &[1, 0, 0], MalformedLayout),
            (Door::Text, raw(ENUM), &[5, 0, 0, 0, b'a'], MalformedLayout),
            (Door::Text, raw(ENUM), &[1, 0, 0, 0, b'a', 9], MalformedLayout),
            (Door::I32, raw(ENUM), &pack_integers(&["1"])[..15], MalformedLayout),
            // A list without `HAS_ENUM`, at any door, is never read: it is malformed.
            (Door::I32, raw(0), &pack_integers(&["1"]), MalformedLayout),
            (Door::Text, raw(0), &pack_text(&[b"a"]), MalformedLayout),
            (Door::F64, raw(0), &pack_integers(&["1"]), MalformedLayout),
            (Door::Bool, raw(0), &pack_text(&[b"yes"]), MalformedLayout),
            // 11 before 12 and 13: a list cut short with no whole entry, or after an empty one.
            (Door::Text, raw(ENUM | UNKNOWN), none, MalformedLayout),
            (Door::Text, raw(ENUM), &[&pack_text(&[b""])[..], &[9]].concat(), MalformedLayout),
            // 12: `HAS_ENUM` with no entries, at an integer door and at the text door.
            (Door::I32, raw(ENUM), none, EmptyEnum),
            (Door::Text, raw(ENUM), none, EmptyEnum),
            (Door::Text, raw(ENUM | TRIM), none, EmptyEnum),
            // 13: an empty entry, trimmed or not, and under `trim` an entry with White_Space
            // at either edge, or nothing else.
            (Door::Text, raw(ENUM), &pack_text(&[b"Male", b""]), UnmatchableEnumEntry),
            (Door::Text, raw(ENUM | TRIM), &pack_text(&[b""]), UnmatchableEnumEntry),
            (Door::Text, raw(ENUM | TRIM), &pack_text(&[b" Male "]), UnmatchableEnumEntry),
            (
                Door::Text,
                raw(ENUM | TRIM),
                &pack_text(&[b"Female", b"Male "]),
                UnmatchableEnumEntry,
            ),
            (
                Door::Text,
                raw(ENUM | TRIM),
                &pack_text(&["\u{3000}Male".as_bytes()]),
                UnmatchableEnumEntry,
            ),
            (Door::Text, raw(ENUM | TRIM), &pack_text(&[b"   "]), UnmatchableEnumEntry),
            (
                Door::Text,
                raw(ENUM | TRIM),
                &pack_text(&["\u{00A0}".as_bytes()]),
                UnmatchableEnumEntry,
            ),
            // The size before everything: nothing else in it can be read.
            (Door::Bool, RawConstraints { size: 0, ..raw(MIN) }, none, MalformedLayout),
        ];
        for (door, raw, entries, refusal) in cases {
            assert_eq!(
                check_constraints(*door, raw, entries),
                Err(*refusal),
                "{door:?} {raw:?} {entries:?}"
            );
        }
        // A sign byte that is not a bool.
        let mut bad_sign = pack_integers(&["1"]);
        bad_sign[13] = 2;
        assert_eq!(check_constraints(Door::I32, &raw(ENUM), &bad_sign), Err(MalformedLayout));
        // Entries no input can match only under `trim`, and White_Space inside, are judged
        // fine; so is an entry of White_Space alone without `trim`.
        for (flags, entries) in [
            (ENUM, pack_text(&[b" Male ", b"Male"])),
            (ENUM, pack_text(&[b"   "])),
            (ENUM | TRIM, pack_text(&[b"Not stated", "\u{00E9}".as_bytes()])),
            (ENUM | TRIM, pack_text(&[b"x"])),
        ] {
            assert_eq!(check_constraints(Door::Text, &raw(flags), &entries), Ok(()), "{entries:?}");
        }
        // Every code, in order.
        let codes = [
            KeywordNotForDoor,
            MinimumWithExclusive,
            MaximumWithExclusive,
            LowerNotBelowUpper,
            MultipleOfNotPositive,
            RoundingWithoutMultipleOf,
            MinLengthAboveMax,
            NotHeldByDoor,
            EnumNotUtf8,
            DuplicateEnum,
            MalformedLayout,
            EmptyEnum,
            UnmatchableEnumEntry,
        ];
        for (code, refusal) in (1..).zip(codes) {
            assert_eq!(refusal.code(), code);
            assert!(!refusal.to_string().is_empty());
        }
        assert_eq!(EmptyEnum.to_string(), "an enum with no entries");
        assert_eq!(UnmatchableEnumEntry.to_string(), "a text enum entry no input can match");
    }

    #[test]
    fn duplicates_only_at_declaration() {
        let entries = [7i128, 7];
        let entries = Entries::Integers(&entries);
        assert_eq!(check_without_duplicates(Door::I32, &raw(ENUM), &entries), Ok(()));
        assert_eq!(check(Door::I32, &raw(ENUM), &entries, true), Err(Refusal::DuplicateEnum));
    }

    #[test]
    fn refusal_applies_to_the_door() {
        let minimum = bounded(MIN, "-1", "0");
        assert_eq!(check_constraints(Door::I8, &minimum, &[]), Ok(()));
        assert_eq!(check_constraints(Door::U8, &minimum, &[]), Err(Refusal::NotHeldByDoor));
        let length = raw(RawConstraints::HAS_MIN_LENGTH);
        assert_eq!(check_constraints(Door::Text, &length, &[]), Ok(()));
        assert_eq!(check_constraints(Door::F64, &length, &[]), Err(Refusal::KeywordNotForDoor));
        let entries = pack_integers(&["1"]);
        assert_eq!(
            check_constraints(Door::F64, &raw(ENUM), &entries),
            Err(Refusal::KeywordNotForDoor)
        );
        assert_eq!(
            Constraints::<f64>::numeric(NumericRules::default(), Some(&[1])).err(),
            Some(Refusal::KeywordNotForDoor)
        );
        assert_eq!(
            Constraints::<f64>::numeric(NumericRules::default(), Some(&[])).err(),
            Some(Refusal::KeywordNotForDoor)
        );
        // No `enum` and an empty one are told apart.
        assert!(Constraints::<i32>::numeric(NumericRules::default(), None).is_ok());
        assert_eq!(
            Constraints::<i32>::numeric(NumericRules::default(), Some(&[])).err(),
            Some(Refusal::EmptyEnum)
        );
        assert!(Constraints::<Text>::text(TextRules::default(), None).is_ok());
        assert_eq!(
            Constraints::<Text>::text(TextRules::default(), Some(&[])).err(),
            Some(Refusal::EmptyEnum)
        );
        assert_eq!(
            Constraints::<Text>::text(TextRules::new().trim(true), Some(&[" Male "])).err(),
            Some(Refusal::UnmatchableEnumEntry)
        );
        assert!(Constraints::<Text>::text(TextRules::new(), Some(&[" Male "])).is_ok());
        let declared = Constraints::<i32>::numeric(NumericRules::default(), Some(&[1])).unwrap();
        assert_eq!(declared.raw().flags, ENUM);
        assert_eq!(check_constraints(Door::Text, &raw(MIN), &[]), Err(Refusal::KeywordNotForDoor));
        for constraint in [
            raw(MIN),
            raw(ENUM),
            raw(RawConstraints::HAS_MULTIPLE_OF),
            raw(RawConstraints::HAS_MAX_LENGTH),
            raw(RawConstraints::TRIM),
        ] {
            assert_eq!(
                check_constraints(Door::Bool, &constraint, &[]),
                Err(Refusal::KeywordNotForDoor)
            );
        }
        assert_eq!(
            check_constraints(Door::Bool, &raw(ENUM), &entries),
            Err(Refusal::KeywordNotForDoor)
        );
        assert_eq!(
            check_constraints(Door::Bool, &raw(ENUM), &pack_text(&[b"yes"])),
            Err(Refusal::KeywordNotForDoor)
        );
        // Nothing declared is no refusal, at any door.
        assert_eq!(check_constraints(Door::Bool, &RawConstraints::ZERO, &[]), Ok(()));
        assert_eq!(check_constraints(Door::Uuid, &RawConstraints::ZERO, &[]), Ok(()));
    }

    #[test]
    fn bounds_at_type_extremes() {
        let minimum = bounded(MIN, "-9223372036854775808", "0");
        assert_eq!(check_constraints(Door::I64, &minimum, &[]), Ok(()));
        let below = bounded(MIN, "-9223372036854775809", "0");
        assert_eq!(check_constraints(Door::I64, &below, &[]), Err(Refusal::NotHeldByDoor));
        let maximum = bounded(MAX, "0", "18446744073709551615");
        assert_eq!(check_constraints(Door::U64, &maximum, &[]), Ok(()));
        let above = bounded(MAX, "0", "18446744073709551616");
        assert_eq!(check_constraints(Door::U64, &above, &[]), Err(Refusal::NotHeldByDoor));
        // `5.0` at scale 1 is a whole number.
        let whole = RawConstraints {
            flags: MIN,
            lower: Decimal { lo: 50, hi: 0, scale: 1, negative: false },
            ..RawConstraints::ZERO
        };
        assert_eq!(check_constraints(Door::U8, &whole, &[]), Ok(()));
        let entries = pack_integers(&["-9223372036854775808", "9223372036854775807"]);
        assert_eq!(check_constraints(Door::I64, &raw(ENUM), &entries), Ok(()));
        assert_eq!(
            Constraints::<u64>::numeric(NumericRules::default(), Some(&[u64::MAX.into(), -1]))
                .err(),
            Some(Refusal::NotHeldByDoor)
        );
    }

    #[test]
    fn turkic_pairs_are_not_duplicates() {
        let rules = TextRules::default();
        assert!(Constraints::<Text>::text(rules, Some(&["I", "ı"])).is_ok());
        assert!(Constraints::<Text>::text(rules, Some(&["i", "İ"])).is_ok());
        assert_eq!(
            Constraints::<Text>::text(rules, Some(&["I", "i"])).err(),
            Some(Refusal::DuplicateEnum)
        );
        assert_eq!(
            check_constraints(
                Door::Text,
                &raw(ENUM),
                &pack_text(&["I".as_bytes(), "ı".as_bytes()])
            ),
            Ok(())
        );
    }

    #[test]
    fn equal_lengths_allowed() {
        let rules = TextRules { min_length: Some(2), max_length: Some(2), trim: true };
        let constraints = Constraints::<Text>::text(rules, None).unwrap();
        let raw = constraints.raw();
        assert_eq!(
            raw.flags,
            RawConstraints::HAS_MIN_LENGTH | RawConstraints::HAS_MAX_LENGTH | RawConstraints::TRIM
        );
        assert_eq!((raw.min_length, raw.max_length), (2, 2));
        assert_eq!(check_constraints(Door::Text, &raw, &[]), Ok(()));
    }

    #[test]
    fn float_doors_hold_every_declarable_bound() {
        let largest = "79228162514264337593543950335";
        let rules = NumericRules::new().minimum(dec(&format!("-{largest}"))).maximum(dec(largest));
        for door in [Door::F32, Door::F64, Door::Decimal] {
            let raw = Constraints::<f32>::numeric(rules, None).unwrap().raw();
            assert_eq!(check_constraints(door, &raw, &[]), Ok(()));
        }
        // The smallest step there is, too.
        let tiny = stepped("0.0000000000000000000000000001");
        assert_eq!(check_constraints(Door::F32, &tiny, &[]), Ok(()));
        // A directly built decimal past scale 28 is not declarable.
        let deep = RawConstraints {
            multiple_of: Decimal { lo: 1, hi: 0, scale: 29, negative: false },
            ..tiny
        };
        assert_eq!(check_constraints(Door::F64, &deep, &[]), Err(Refusal::NotHeldByDoor));
    }

    #[test]
    fn numeric_constraints_carry_their_rules() {
        let rules = NumericRules::new()
            .exclusive_minimum(dec("-90"))
            .maximum(dec("90"))
            .multiple_of(dec("0.000001"), Rounding::Midpoint);
        assert_eq!(rules.lower, Some(Bound::Exclusive(dec("-90"))));
        assert_eq!(rules.upper, Some(Bound::Inclusive(dec("90"))));
        let raw = Constraints::<f64>::numeric(rules, None).unwrap().raw();
        assert_eq!(raw.flags, XMIN | MAX | RawConstraints::HAS_MULTIPLE_OF);
        assert_eq!(raw.rounding, 1);
        assert_eq!(
            (raw.lower, raw.upper, raw.multiple_of),
            (dec("-90"), dec("90"), dec("0.000001"))
        );
        let constraints =
            Constraints::<i32>::numeric(NumericRules::default(), Some(&[10, 20])).unwrap();
        let entries = constraints.entries();
        assert_eq!((entries.len(), entries.integer(1), entries.integer(2)), (2, Some(20), None));
        let texts = ["Female", "Male", "Other"];
        let constraints = Constraints::<Text>::text(TextRules::default(), Some(&texts)).unwrap();
        assert_eq!(constraints.entries().text(1), Some("Male".as_bytes()));
        let packed = pack_text(&[b"Female", b"Male", b"Other"]);
        let packed = Entries::packed(Door::Text, &packed);
        assert_eq!(
            (packed.len(), packed.text(2), packed.text(3)),
            (3, Some("Other".as_bytes()), None)
        );
        assert_eq!(Keyword::Minimum.code(), 1);
        assert_eq!(Keyword::Enum.code(), 8);
    }

    /// A bound declared both ways through the builders is refused as the C ABI refuses both
    /// flags (2, 3), in code order; the same keyword twice is one keyword, the later value
    /// winning; and the `maybe_` twins declare nothing on `None`.
    #[test]
    fn builders_refuse_a_bound_declared_both_ways() {
        let numeric = |rules| Constraints::<i32>::numeric(rules, None).map(|c| c.raw());
        let (zero, one, nine) = (dec("0"), dec("1"), dec("9"));
        for rules in [
            NumericRules::new().minimum(zero).exclusive_minimum(one),
            NumericRules::new().exclusive_minimum(one).minimum(zero),
            NumericRules::new().minimum(zero).maybe_exclusive_minimum(Some(one)),
        ] {
            assert_eq!(numeric(rules), Err(Refusal::MinimumWithExclusive));
            assert_eq!(
                numeric(rules.maximum(nine).exclusive_maximum(nine)),
                Err(Refusal::MinimumWithExclusive)
            );
        }
        assert_eq!(
            numeric(NumericRules::new().maximum(nine).exclusive_maximum(nine)),
            Err(Refusal::MaximumWithExclusive)
        );
        assert_eq!(
            numeric(NumericRules::new().exclusive_maximum(nine).maximum(one)),
            Err(Refusal::MaximumWithExclusive)
        );
        // Code order, as the C ABI: an `enum` at a float door (1) before the conflict (2).
        let both = NumericRules::new().minimum(zero).exclusive_minimum(one);
        assert_eq!(
            Constraints::<f64>::numeric(both, Some(&[1])).err(),
            Some(Refusal::KeywordNotForDoor)
        );
        // The same keyword twice: the later value.
        let raw = numeric(NumericRules::new().minimum(nine).minimum(zero).maximum(nine)).unwrap();
        assert_eq!((raw.flags, raw.lower), (MIN | MAX, zero));
        // `None` declares nothing, and leaves what was declared.
        let rules = NumericRules::new()
            .minimum(zero)
            .maybe_exclusive_minimum(None)
            .maybe_maximum(None)
            .maybe_exclusive_maximum(Some(nine))
            .maybe_multiple_of(None);
        assert_eq!(numeric(rules).map(|raw| raw.flags), Ok(MIN | XMAX));
        assert_eq!(NumericRules::new().maybe_minimum(None), NumericRules::new());
        let text = TextRules::new().maybe_min_length(None).maybe_max_length(Some(3));
        assert_eq!((text.min_length, text.max_length), (None, Some(3)));
    }

    #[test]
    fn declaration_conversion() {
        for refused in [
            "3.5e38",
            "1e-29",
            "0.00000000000000000000000000001",
            "79228162514264337593543950336",
            "",
            "-",
            ".",
            "1e",
            "1x",
            " 1",
            "1..2",
        ] {
            assert_eq!(decimal_for_declaration(refused), Err(Refusal::NotHeldByDoor), "{refused}");
        }
        let decimal =
            |lo: u64, hi: u32, scale: u8, negative: bool| Decimal { lo, hi, scale, negative };
        assert_eq!(dec("0.000001"), decimal(1, 0, 6, false));
        assert_eq!(dec("-90"), decimal(90, 0, 0, true));
        assert_eq!(dec("1.50"), decimal(15, 0, 1, false));
        assert_eq!(dec("5.0"), decimal(5, 0, 0, false));
        assert_eq!(dec("-0"), decimal(0, 0, 0, false));
        assert_eq!(dec("+1e3"), decimal(1000, 0, 0, false));
        assert_eq!(dec(".5"), decimal(5, 0, 1, false));
        assert_eq!(dec("1E-28"), decimal(1, 0, 28, false));
        assert_eq!(dec("0.10000000000000000000000000000000000"), decimal(1, 0, 1, false));
        assert_eq!(dec("79228162514264337593543950335"), decimal(u64::MAX, u32::MAX, 0, false));
        assert_eq!(dec("7.9228162514264337593543950335e28"), decimal(u64::MAX, u32::MAX, 0, false));
        assert_eq!(dec("-9223372036854775808").to_string(), "-9223372036854775808");
    }
}
