//! Validation constraints: the OpenAPI 3.2 Schema Object keywords a value is judged by once
//! its door has read it (`minimum`, `exclusiveMinimum`, `maximum`, `exclusiveMaximum`,
//! `multipleOf`, `minLength`, `maxLength`, `enum`), with HyperCast's own extensions beside
//! them (a rounding for `multipleOf`, `trim`, and `caseSensitive` for a text `enum`, which
//! folds case otherwise), declared **for a door**. A declaration OpenAPI allows is never
//! refused, and a value that breaks one is a violation, never an error.
//!
//! [`check_constraints`] is the one judge of a declaration: the C ABI's
//! `cast_constraints_check` and every binding's `Constraints` constructor ask it, so no two
//! layers can disagree on what is refused. The checked doors re-run every rule on each call
//! ([`check`]).

use core::fmt;
use core::marker::PhantomData;

use crate::abi::RawConstraints;
use crate::door::Door;
pub use crate::exact::Rounding;
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
const TEXT_FLAGS: u32 = RawConstraints::HAS_MIN_LENGTH | RawConstraints::HAS_MAX_LENGTH;

/// The text door's own extensions, beyond OpenAPI: refused at any other door, since they
/// would mean nothing there.
const TEXT_EXTENSIONS: u32 = RawConstraints::TRIM | RawConstraints::CASE_SENSITIVE;

/// Every flag the layout defines.
const KNOWN_FLAGS: u32 = NUMERIC_FLAGS | TEXT_FLAGS | TEXT_EXTENSIONS | RawConstraints::HAS_ENUM;

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
    /// A constraint on a door that takes none, or one of HyperCast's own extensions where it
    /// means nothing: `trim` or `caseSensitive` off the text door, a rounding at the text door.
    /// An OpenAPI keyword for another type (`minLength` at a numeric door, `minimum` at the
    /// text door) is not refused: OpenAPI ignores it, and so does the door.
    KeywordNotForDoor = 1,
    /// A `multipleOf` that is zero or negative, which OpenAPI forbids.
    MultipleOfNotPositive = 5,
    /// A rounding other than strict without `multipleOf`.
    RoundingWithoutMultipleOf = 6,
    /// A bound, step or `enum` entry no door can hold: a decimal with a scale past 28, or a
    /// text `enum` entry longer than `u32::MAX` bytes, which the packed form cannot carry and
    /// no input can match (a judged input that long is `OutOfRange`). Also a rounding with a
    /// fractional step at an integer door, whose rounded value the door could not deliver.
    NotHeldByDoor = 8,
    /// A text `enum` entry that is not valid UTF-8.
    EnumNotUtf8 = 9,
    /// A layout the core does not read: a `size` other than [`RawConstraints::SIZE`], an
    /// unknown flag bit, rounding code or nonzero `_reserved`, a decimal whose sign byte is
    /// not `0` or `1` (a bound or step read across the C ABI, or a packed integer `enum`
    /// entry), a packed `enum` list cut short (a text entry's length word or bytes, or a
    /// 16-byte integer entry), or an `enum` list passed without
    /// [`HAS_ENUM`](RawConstraints::HAS_ENUM).
    MalformedLayout = 11,
}

impl Refusal {
    /// The C ABI's code: `1`, `5`, `6`, `8`, `9` or `11`. The codes between belonged to
    /// refusals of declarations OpenAPI allows (equal or crossed bounds, an inclusive and an
    /// exclusive bound together, `minLength` above `maxLength`, an empty or duplicated
    /// `enum`, an entry no input can match); they are retired and never reused.
    pub const fn code(self) -> u32 {
        self as u32
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Refusal::KeywordNotForDoor => "a constraint the door does not take",
            Refusal::MultipleOfNotPositive => "multipleOf is not positive",
            Refusal::RoundingWithoutMultipleOf => "a rounding without multipleOf",
            Refusal::NotHeldByDoor => "a bound, step or enum entry the door cannot hold",
            Refusal::EnumNotUtf8 => "an enum entry that is not UTF-8",
            Refusal::MalformedLayout => "a malformed constraints layout",
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
/// declaration read from a schema whose keywords may or may not be there. The four bounds are
/// independent, as in OpenAPI: `minimum` and `exclusiveMinimum` may both be declared, and a
/// value is held to each. Bounds no value meets (`minimum` above `maximum`) declare too, and
/// every value is then a violation.
///
/// ```
/// use hypercast::{Constraints, NumericRules, decimal_for_declaration};
///
/// let zero = decimal_for_declaration("0")?;
/// let schema_minimum = None;
/// let rules = NumericRules::new().maybe_minimum(schema_minimum).maybe_exclusive_minimum(Some(zero));
/// assert!(Constraints::<f64>::numeric(rules, None).is_ok());
/// let both = NumericRules::new().minimum(zero).exclusive_minimum(zero);
/// assert!(Constraints::<f64>::numeric(both, None).is_ok());
/// # Ok::<(), hypercast::Refusal>(())
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct NumericRules {
    /// `minimum`: the value may equal it.
    pub minimum: Option<Decimal>,
    /// `exclusiveMinimum`: the value must exceed it.
    pub exclusive_minimum: Option<Decimal>,
    /// `maximum`: the value may equal it.
    pub maximum: Option<Decimal>,
    /// `exclusiveMaximum`: the value must be below it.
    pub exclusive_maximum: Option<Decimal>,
    /// `multipleOf`, and how a value that is not a multiple is treated.
    pub multiple_of: Option<(Decimal, Rounding)>,
}

impl NumericRules {
    /// No keyword declared.
    pub const fn new() -> NumericRules {
        NumericRules {
            minimum: None,
            exclusive_minimum: None,
            maximum: None,
            exclusive_maximum: None,
            multiple_of: None,
        }
    }

    /// `minimum`: the value may equal it.
    pub const fn minimum(mut self, value: Decimal) -> NumericRules {
        self.minimum = Some(value);
        self
    }

    /// `exclusiveMinimum`: the value must exceed it.
    pub const fn exclusive_minimum(mut self, value: Decimal) -> NumericRules {
        self.exclusive_minimum = Some(value);
        self
    }

    /// `maximum`: the value may equal it.
    pub const fn maximum(mut self, value: Decimal) -> NumericRules {
        self.maximum = Some(value);
        self
    }

    /// `exclusiveMaximum`: the value must be below it.
    pub const fn exclusive_maximum(mut self, value: Decimal) -> NumericRules {
        self.exclusive_maximum = Some(value);
        self
    }

    /// `multipleOf`, and how a value that is not a multiple is treated.
    pub const fn multiple_of(mut self, step: Decimal, rounding: Rounding) -> NumericRules {
        self.multiple_of = Some((step, rounding));
        self
    }

    /// [`minimum`](Self::minimum) when `value` is `Some`; `None` declares nothing.
    pub const fn maybe_minimum(mut self, value: Option<Decimal>) -> NumericRules {
        if value.is_some() {
            self.minimum = value;
        }
        self
    }

    /// [`exclusive_minimum`](Self::exclusive_minimum) when `value` is `Some`; `None` declares
    /// nothing.
    pub const fn maybe_exclusive_minimum(mut self, value: Option<Decimal>) -> NumericRules {
        if value.is_some() {
            self.exclusive_minimum = value;
        }
        self
    }

    /// [`maximum`](Self::maximum) when `value` is `Some`; `None` declares nothing.
    pub const fn maybe_maximum(mut self, value: Option<Decimal>) -> NumericRules {
        if value.is_some() {
            self.maximum = value;
        }
        self
    }

    /// [`exclusive_maximum`](Self::exclusive_maximum) when `value` is `Some`; `None` declares
    /// nothing.
    pub const fn maybe_exclusive_maximum(mut self, value: Option<Decimal>) -> NumericRules {
        if value.is_some() {
            self.exclusive_maximum = value;
        }
        self
    }

    /// [`multiple_of`](Self::multiple_of) when `step` is `Some`; `None` declares nothing.
    pub const fn maybe_multiple_of(self, step: Option<(Decimal, Rounding)>) -> NumericRules {
        match step {
            Some((step, rounding)) => self.multiple_of(step, rounding),
            None => self,
        }
    }
}

/// The text keywords: lengths in code points, whether the text is trimmed of Unicode
/// `White_Space` first, and whether an `enum` matches case-sensitively (it folds case unless
/// told not to). Built from [`new`](Self::new) (or `default()`) and the builder
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
    /// Match the `enum` exactly, byte for byte, rather than under simple case folding.
    pub case_sensitive: bool,
}

impl TextRules {
    /// No keyword declared, and no trim.
    pub const fn new() -> TextRules {
        TextRules { min_length: None, max_length: None, trim: false, case_sensitive: false }
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

    /// Whether the `enum` matches exactly, byte for byte, rather than under simple case
    /// folding: for a vocabulary whose case carries meaning (`mm` and `Mm`, `Co` and `CO`).
    pub const fn case_sensitive(mut self, case_sensitive: bool) -> TextRules {
        self.case_sensitive = case_sensitive;
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
    /// Declares the numeric keywords and an `enum` (`None` for none; an empty list declares
    /// one no value matches) for `T`'s door. An entry matches a value equal to it exactly, as
    /// written (`7`, `7.0` and `7.00` are one value); at an integer door a fractional entry
    /// matches nothing.
    pub fn numeric(rules: NumericRules, entries: Option<&'e [Decimal]>) -> Result<Self, Refusal> {
        let mut raw = RawConstraints::ZERO;
        if let Some(value) = rules.minimum {
            raw.flags |= RawConstraints::HAS_MINIMUM;
            raw.lower = value;
        }
        if let Some(value) = rules.exclusive_minimum {
            raw.flags |= RawConstraints::HAS_EXCLUSIVE_MINIMUM;
            raw.exclusive_lower = value;
        }
        if let Some(value) = rules.maximum {
            raw.flags |= RawConstraints::HAS_MAXIMUM;
            raw.upper = value;
        }
        if let Some(value) = rules.exclusive_maximum {
            raw.flags |= RawConstraints::HAS_EXCLUSIVE_MAXIMUM;
            raw.exclusive_upper = value;
        }
        if let Some((step, rounding)) = rules.multiple_of {
            raw.flags |= RawConstraints::HAS_MULTIPLE_OF;
            raw.multiple_of = step;
            raw.rounding = rounding as u32;
        }
        let entries = match entries {
            Some(entries) => {
                raw.flags |= RawConstraints::HAS_ENUM;
                Entries::Decimals(entries)
            }
            None => Entries::None,
        };
        Self::checked(T::DOOR, raw, entries)
    }

    /// Reads a declaration in its C ABI form for `T`'s door: `raw`, and the `enum` list
    /// packed as the C ABI passes it (16-byte [`Decimal`]s; empty for none). Refused exactly
    /// where [`check_constraints`] refuses. For a parser that
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
    /// its bytes; empty for none). Refused exactly where [`check_constraints`] refuses.
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
    /// Declares the text keywords and a text `enum` (`None` for none; an empty list declares
    /// one no text matches) for the text door. Entries may repeat, and the first that matches
    /// wins.
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
        if rules.case_sensitive {
            raw.flags |= RawConstraints::CASE_SENSITIVE;
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
        check(door, &raw, &entries)?;
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
            Entries::PackedDecimals(bytes) | Entries::PackedText(bytes) => {
                if let Some(out) = out.get_mut(..bytes.len()) {
                    out.copy_from_slice(bytes);
                }
            }
            Entries::Decimals(entries) => {
                let (chunks, _) = out.as_chunks_mut::<PACKED_DECIMAL>();
                for (entry, chunk) in entries.iter().zip(chunks) {
                    *chunk = packed_decimal(entry);
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

/// A numeric `enum` entry as the C ABI packs it: a [`Decimal`] in its native layout (`lo`,
/// `hi`, `scale`, the sign byte, two zero bytes).
fn packed_decimal(entry: &Decimal) -> [u8; PACKED_DECIMAL] {
    let [l0, l1, l2, l3, l4, l5, l6, l7] = entry.lo.to_ne_bytes();
    let [h0, h1, h2, h3] = entry.hi.to_ne_bytes();
    [l0, l1, l2, l3, l4, l5, l6, l7, h0, h1, h2, h3, entry.scale, u8::from(entry.negative), 0, 0]
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
    /// Numeric entries.
    Decimals(&'e [Decimal]),
    /// Text entries.
    Text(&'e [&'e str]),
    /// Numeric entries as the ABI packs them: [`Decimal`]s in their native layout, 16 bytes
    /// each.
    PackedDecimals(&'e [u8]),
    /// Text entries as the ABI packs them: each a little-endian `u32` byte length, then
    /// that many bytes, with no padding.
    PackedText(&'e [u8]),
}

impl<'e> Entries<'e> {
    /// The bytes the list takes packed as the C ABI passes it, or `None` past `usize::MAX`
    /// (only a borrowed text list on a 32-bit target gets there: a list of `Decimal`s lives
    /// in memory, and its packing takes no more bytes than it does).
    fn packed_len(&self) -> Option<usize> {
        match self {
            Entries::None => Some(0),
            Entries::Decimals(entries) => entries.len().checked_mul(PACKED_DECIMAL),
            Entries::Text(entries) => packed_text_len(entries.iter().map(|entry| entry.len())),
            Entries::PackedDecimals(bytes) | Entries::PackedText(bytes) => Some(bytes.len()),
        }
    }

    /// The `enum` list the ABI passes for `door`: packed text at the text door, packed
    /// decimals anywhere else. Read only under [`HAS_ENUM`](RawConstraints::HAS_ENUM); empty
    /// is no entries.
    pub(crate) fn packed(door: Door, bytes: &'e [u8]) -> Entries<'e> {
        match (bytes.is_empty(), door) {
            (true, _) => Entries::None,
            (false, Door::Text) => Entries::PackedText(bytes),
            (false, _) => Entries::PackedDecimals(bytes),
        }
    }

    /// The number of entries; a packed list cut short counts its whole entries only.
    pub(crate) fn len(&self) -> usize {
        match self {
            Entries::None => 0,
            Entries::Decimals(entries) => entries.len(),
            Entries::Text(entries) => entries.len(),
            Entries::PackedDecimals(bytes) => bytes.len() / PACKED_DECIMAL,
            Entries::PackedText(_) => self.texts().count(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Entries::None => true,
            Entries::Decimals(entries) => entries.is_empty(),
            Entries::Text(entries) => entries.is_empty(),
            Entries::PackedDecimals(bytes) | Entries::PackedText(bytes) => bytes.is_empty(),
        }
    }

    /// Numeric entry `i`, or `None` past the end or for a text list.
    pub(crate) fn number(&self, i: usize) -> Option<Decimal> {
        match self {
            Entries::Decimals(entries) => entries.get(i).copied(),
            Entries::PackedDecimals(_) => self.packed_decimal(i).map(|(decimal, _)| decimal),
            _ => None,
        }
    }

    /// Packed numeric entry `i` as the [`Decimal`] it spells, and whether its sign byte is
    /// a valid `bool` (`0` or `1`).
    fn packed_decimal(&self, i: usize) -> Option<(Decimal, bool)> {
        let Entries::PackedDecimals(bytes) = self else {
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

/// Judges a declaration against its door: `Ok` when every rule holds, otherwise the first
/// rule it breaks, in [`Refusal`]'s code order. `entries` is the `enum` list as the C ABI
/// packs it, read only under [`RawConstraints::HAS_ENUM`] (and then empty is an `enum` no
/// value matches): at the text door, each entry a little-endian `u32` byte length then that
/// many bytes; at a numeric door, [`Decimal`]s in their native 16-byte layout. A non-empty
/// list without `HAS_ENUM` is [`Refusal::MalformedLayout`].
///
/// A declaration OpenAPI allows is never refused, even one no value can meet: equal or
/// crossed bounds, `minimum` with `exclusiveMinimum`, `minLength` above `maxLength`, an empty
/// `enum`, repeated entries (the first that matches wins), an entry no input can match. A
/// keyword for another type (`minLength` at a numeric door) is ignored, as OpenAPI ignores it.
///
/// One exception to the order: a `size` other than [`RawConstraints::SIZE`] is refused as
/// [`Refusal::MalformedLayout`] before anything else, since no other field of a layout of
/// unknown size can be read (the C ABI judges a decimal's sign byte there too, before it
/// reads the layout; a Rust `bool` holds no other byte).
pub fn check_constraints(door: Door, raw: &RawConstraints, entries: &[u8]) -> Result<(), Refusal> {
    check(door, raw, &Entries::packed(door, entries))
}

/// The declaration rules, in [`Refusal`]'s code order. OpenAPI decides what is a valid
/// declaration: a schema it allows is never refused here, even one no value can meet
/// (`minimum` above `maximum`, an empty `enum`), whose every value is then a violation. What
/// is refused is what OpenAPI forbids, what no door can hold, a misuse of HyperCast's own
/// extensions (`trim`, `caseSensitive`, a rounding), and a malformed layout.
pub(crate) fn check(door: Door, raw: &RawConstraints, entries: &Entries) -> Result<(), Refusal> {
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
    let numeric_door = integer_door || matches!(door, Door::F32 | Door::F64 | Door::Decimal);
    let step = has(RawConstraints::HAS_MULTIPLE_OF);
    let rounds = raw.rounding != Rounding::Strict as u32;

    // 1: a door that takes no constraint, or an extension where it means nothing. A keyword
    // for another type is ignored, as OpenAPI ignores it.
    let not_for_door = if numeric_door {
        has(TEXT_EXTENSIONS)
    } else if door == Door::Text {
        rounds && step
    } else {
        has(KNOWN_FLAGS)
    };
    if not_for_door {
        return Err(Refusal::KeywordNotForDoor);
    }

    // 5: a positive step, as OpenAPI requires.
    if step && (raw.multiple_of.magnitude() == 0 || raw.multiple_of.negative) {
        return Err(Refusal::MultipleOfNotPositive);
    }

    // 6: a rounding only with a step.
    if rounds && !step {
        return Err(Refusal::RoundingWithoutMultipleOf);
    }

    // 8: every decimal at a scale a `Decimal` holds, every text entry short enough for the
    // packed form's `u32` length, and a rounding an integer door can deliver.
    let declared = |flag: u32, value: &Decimal| !has(flag) || value.scale <= MAX_SCALE;
    if !declared(RawConstraints::HAS_MINIMUM, &raw.lower)
        || !declared(RawConstraints::HAS_EXCLUSIVE_MINIMUM, &raw.exclusive_lower)
        || !declared(RawConstraints::HAS_MAXIMUM, &raw.upper)
        || !declared(RawConstraints::HAS_EXCLUSIVE_MAXIMUM, &raw.exclusive_upper)
        || !declared(RawConstraints::HAS_MULTIPLE_OF, &raw.multiple_of)
        || (0..entries.len())
            .any(|i| entries.number(i).is_some_and(|entry| entry.scale > MAX_SCALE))
        || entries.texts().any(|entry| u32::try_from(entry.len()).is_err())
        || (integer_door && step && rounds && integral(&raw.multiple_of).is_none())
    {
        return Err(Refusal::NotHeldByDoor);
    }

    // 9: every text entry UTF-8.
    if entries.texts().any(|entry| core::str::from_utf8(entry).is_err()) {
        return Err(Refusal::EnumNotUtf8);
    }

    // 11: nothing the layout does not define.
    let whole = match entries {
        Entries::PackedDecimals(bytes) => {
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
    Ok(())
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
    const EXACT: u32 = RawConstraints::CASE_SENSITIVE;
    const LENGTHS: u32 = RawConstraints::HAS_MIN_LENGTH | RawConstraints::HAS_MAX_LENGTH;
    /// A flag bit the layout does not define.
    const UNKNOWN: u32 = 1024;

    #[test]
    fn layouts_are_pinned() {
        assert_eq!(size_of::<RawConstraints>(), 104);
        assert_eq!(align_of::<RawConstraints>(), 8);
        assert_eq!(offset_of!(RawConstraints, lower), 24);
        assert_eq!(offset_of!(RawConstraints, upper), 40);
        assert_eq!(offset_of!(RawConstraints, multiple_of), 56);
        assert_eq!(offset_of!(RawConstraints, exclusive_lower), 72);
        assert_eq!(offset_of!(RawConstraints, exclusive_upper), 88);
        assert_eq!(RawConstraints::SIZE as usize, size_of::<RawConstraints>());
        assert_eq!(size_of::<RawCheck>(), 12);
        assert_eq!(align_of::<RawCheck>(), 4);
        assert_eq!(size_of::<RawSpan>(), 8);
        assert_eq!(align_of::<RawSpan>(), 4);
    }

    #[test]
    fn each_refusal_has_its_code() {
        use Refusal::*;
        let rounding = RawConstraints { rounding: 1, ..RawConstraints::ZERO };
        let short = RawConstraints { size: 72, ..RawConstraints::ZERO };
        let reserved = RawConstraints { _reserved: 1, ..RawConstraints::ZERO };
        let none: &[u8] = &[];
        let not_utf8 = pack_text(&[&[0xFF]]);
        let deep = Decimal { lo: 1, hi: 0, scale: 29, negative: false };
        // Each case breaks one rule, or several where the first in code order must win.
        let cases: &[(Door, RawConstraints, &[u8], Refusal)] = &[
            // 1: a door that takes no constraint, or an extension where it means nothing.
            (Door::Bool, raw(MIN), none, KeywordNotForDoor),
            (Door::Uuid, raw(ENUM), &pack_text(&[b"x"]), KeywordNotForDoor),
            (Door::I32, raw(TRIM), none, KeywordNotForDoor),
            (Door::F64, raw(EXACT), none, KeywordNotForDoor),
            (Door::Text, RawConstraints { rounding: 2, ..stepped("1") }, none, KeywordNotForDoor),
            // 1 before 5: an extension and a zero step.
            (Door::I32, RawConstraints { flags: TRIM, ..stepped("0") }, none, KeywordNotForDoor),
            (Door::I32, stepped("0"), none, MultipleOfNotPositive),
            (Door::F64, stepped("-0.05"), none, MultipleOfNotPositive),
            (Door::I32, rounding, none, RoundingWithoutMultipleOf),
            // 6 before 11: an unknown flag bit too.
            (
                Door::I32,
                RawConstraints { flags: UNKNOWN, ..rounding },
                none,
                RoundingWithoutMultipleOf,
            ),
            // 8: a decimal past scale 28, or a rounding to a fraction at an integer door.
            (Door::F64, RawConstraints { multiple_of: deep, ..stepped("1") }, none, NotHeldByDoor),
            (
                Door::I32,
                RawConstraints { flags: XMAX, exclusive_upper: deep, ..raw(0) },
                none,
                NotHeldByDoor,
            ),
            (Door::I32, RawConstraints { rounding: 1, ..stepped("0.5") }, none, NotHeldByDoor),
            (Door::Text, raw(ENUM), &not_utf8, EnumNotUtf8),
            (Door::Text, raw(ENUM | EXACT), &pack_text(&[b"a", &[0xFF]]), EnumNotUtf8),
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
        // Every code: the ones between are retired, never reused.
        let codes = [
            (KeywordNotForDoor, 1),
            (MultipleOfNotPositive, 5),
            (RoundingWithoutMultipleOf, 6),
            (NotHeldByDoor, 8),
            (EnumNotUtf8, 9),
            (MalformedLayout, 11),
        ];
        for (refusal, code) in codes {
            assert_eq!(refusal.code(), code);
            assert!(!refusal.to_string().is_empty());
        }
    }

    /// Every declaration OpenAPI 3.2 allows declares, even one no value can meet.
    #[test]
    fn openapi_declarations_are_never_refused() {
        let none: &[u8] = &[];
        let mut lengths = raw(LENGTHS);
        (lengths.min_length, lengths.max_length) = (3, 2);
        let both_lower = RawConstraints {
            flags: MIN | XMIN | MAX | XMAX,
            lower: dec("5"),
            exclusive_lower: dec("7"),
            upper: dec("5"),
            exclusive_upper: dec("5"),
            ..RawConstraints::ZERO
        };
        let cases: &[(Door, RawConstraints, &[u8])] = &[
            // Equal and crossed bounds, and all four together.
            (Door::I32, bounded(MIN | MAX, "5", "5"), none),
            (Door::I32, bounded(MIN | MAX, "6", "5"), none),
            (Door::F64, both_lower, none),
            // `minLength` above `maxLength`.
            (Door::Text, lengths, none),
            // An empty `enum`, repeated entries, entries no input can match.
            (Door::I32, raw(ENUM), none),
            (Door::Text, raw(ENUM), none),
            (Door::Text, raw(ENUM), &pack_text(&[b"Male", b"MALE", b"Male"])),
            (Door::Text, raw(ENUM | TRIM), &pack_text(&[b"", b" Male ", b"   "])),
            (Door::I32, raw(ENUM), &pack_integers(&["7", "7.0"])),
            // A fractional or out-of-range bound, step or entry at an integer door.
            (Door::U8, bounded(MIN | MAX, "-1", "256"), none),
            (Door::I64, bounded(MIN, "-9223372036854775809", "0"), none),
            (Door::I32, bounded(MIN, "0.5", "0"), none),
            (Door::I32, stepped("0.5"), none),
            (Door::U8, raw(ENUM), &pack_integers(&["256", "0.5", "-1"])),
            // An `enum` at a float or decimal door.
            (Door::F64, raw(ENUM), &pack_integers(&["0.5", "1"])),
            (Door::Decimal, raw(ENUM), &pack_integers(&["0.01"])),
            // A keyword for another type, which the door ignores.
            (Door::I32, raw(LENGTHS), none),
            (Door::Text, bounded(MIN | MAX, "1", "2"), none),
            (Door::Text, stepped("1"), none),
            // The extensions where they mean something.
            (Door::Text, raw(TRIM | EXACT | ENUM), &pack_text(&[b"mm", b"Mm"])),
            (Door::I32, RawConstraints { rounding: 3, ..stepped("5") }, none),
            // Nothing declared, at any door.
            (Door::Bool, RawConstraints::ZERO, none),
            (Door::Uuid, RawConstraints::ZERO, none),
        ];
        for (door, raw, entries) in cases {
            assert_eq!(check_constraints(*door, raw, entries), Ok(()), "{door:?} {raw:?}");
        }
        // Through the Rust builders too.
        assert!(Constraints::<i32>::numeric(NumericRules::new(), Some(&[])).is_ok());
        assert!(Constraints::<Text>::text(TextRules::new(), Some(&[])).is_ok());
        assert!(Constraints::<Text>::text(TextRules::new().trim(true), Some(&[" Male "])).is_ok());
        assert!(Constraints::<Text>::text(TextRules::new(), Some(&["I", "i", "ı"])).is_ok());
        assert!(
            Constraints::<f64>::numeric(NumericRules::new(), Some(&[dec("0.5"), dec("1")])).is_ok()
        );
        let text = TextRules::new().min_length(3).max_length(2);
        assert!(Constraints::<Text>::text(text, None).is_ok());
    }

    #[test]
    fn equal_lengths_allowed() {
        let rules = TextRules::new().min_length(2).max_length(2).trim(true).case_sensitive(true);
        let constraints = Constraints::<Text>::text(rules, None).unwrap();
        let raw = constraints.raw();
        assert_eq!(raw.flags, LENGTHS | TRIM | EXACT);
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
    }

    #[test]
    fn numeric_constraints_carry_their_rules() {
        let rules = NumericRules::new()
            .exclusive_minimum(dec("-90"))
            .maximum(dec("90"))
            .multiple_of(dec("0.000001"), Rounding::Midpoint);
        assert_eq!((rules.exclusive_minimum, rules.maximum), (Some(dec("-90")), Some(dec("90"))));
        let raw = Constraints::<f64>::numeric(rules, None).unwrap().raw();
        assert_eq!(raw.flags, XMIN | MAX | RawConstraints::HAS_MULTIPLE_OF);
        assert_eq!(raw.rounding, 1);
        assert_eq!(
            (raw.exclusive_lower, raw.upper, raw.multiple_of),
            (dec("-90"), dec("90"), dec("0.000001"))
        );
        let entries = [Decimal::from(10), Decimal::from(20)];
        let constraints =
            Constraints::<i32>::numeric(NumericRules::default(), Some(&entries)).unwrap();
        let entries = constraints.entries();
        assert_eq!(
            (entries.len(), entries.number(1), entries.number(2)),
            (2, Some(Decimal::from(20)), None)
        );
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

    /// The four bounds are independent keywords: each lands in its own field, the same
    /// keyword twice keeps the later value, and the `maybe_` twins declare nothing on `None`.
    #[test]
    fn the_four_bounds_are_independent() {
        let numeric = |rules| Constraints::<i32>::numeric(rules, None).map(|c| c.raw());
        let (zero, one, five, nine) = (dec("0"), dec("1"), dec("5"), dec("9"));
        let raw = numeric(
            NumericRules::new()
                .minimum(zero)
                .exclusive_minimum(one)
                .maximum(nine)
                .exclusive_maximum(five),
        )
        .unwrap();
        assert_eq!(raw.flags, MIN | XMIN | MAX | XMAX);
        assert_eq!(
            (raw.lower, raw.exclusive_lower, raw.upper, raw.exclusive_upper),
            (zero, one, nine, five)
        );
        // The same keyword twice: the later value.
        let raw = numeric(NumericRules::new().minimum(nine).minimum(zero).maximum(nine)).unwrap();
        assert_eq!((raw.flags, raw.lower), (MIN | MAX, zero));
        // `None` declares nothing, and leaves what was declared.
        let rules = NumericRules::new()
            .minimum(zero)
            .maybe_minimum(None)
            .maybe_exclusive_minimum(None)
            .maybe_maximum(None)
            .maybe_exclusive_maximum(Some(nine))
            .maybe_multiple_of(None);
        assert_eq!(numeric(rules).map(|raw| (raw.flags, raw.lower)), Ok((MIN | XMAX, zero)));
        assert_eq!(NumericRules::new().maybe_minimum(None), NumericRules::new());
        let text = TextRules::new().maybe_min_length(None).maybe_max_length(Some(3));
        assert_eq!((text.min_length, text.max_length), (None, Some(3)));
    }

    #[test]
    fn decimals_from_integers() {
        assert_eq!(Decimal::from(-5i32), dec("-5"));
        assert_eq!(Decimal::from(0u8), dec("0"));
        assert_eq!(Decimal::from(i64::MIN), dec("-9223372036854775808"));
        assert_eq!(Decimal::from(u64::MAX), dec("18446744073709551615"));
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
