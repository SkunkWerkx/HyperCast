//! What exactly failed: the [`Detail`] vocabulary, and the [`Failure`] that carries one beside
//! a [`Fault`]'s reason and span. The [`detailed`](crate::detailed) doors return a `Failure`;
//! the other doors return the same parse's `Fault` ([`Failure::fault`]).
//!
//! A detail is for diagnosing rejected data. Published codes keep their numbers and meanings,
//! new codes may join in a minor release (so a consumer tolerates one it doesn't know: the
//! reason and span still stand), and a code says what failed, never how to retry or where in
//! the parser it happened.

use crate::site::Site;
use crate::verdict::{Fault, Reason, span_of, utf8_ok};

/// What exactly a cast found wrong, beside the broad [`Reason`]. The numbers are the ABI's
/// and never change; `0` is reserved for "no detail" (a checked door's violation) and is
/// never a `Detail`. More members may join in a minor release.
///
/// A detail is independent of the reason: ∞ is `OutOfRange` at a float door and `Malformed`
/// at an integer door, and [`NotFinite`](Self::NotFinite) at both.
#[repr(u8)]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Detail {
    /// Empty, or only white space.
    Blank = 1,
    /// A character the position doesn't allow, where the position allows several things: a
    /// letter in a numeral, an undeclared symbol.
    UnexpectedCharacter = 2,
    /// A numeral or digit field is under way, or a fixed position requires a digit, and
    /// another character or the end is found (`0xG`, `1e`, `2026-0x-07`).
    DigitExpected = 3,
    /// A required mark (`-`, `:`, `T`, `}`, `0x`, `;`, a UUID hyphen) is missing or different.
    DelimiterExpected = 4,
    /// The value is complete, but more text follows.
    TrailingText = 5,
    /// The numeral is absent entirely: only a sign, a currency or percent affix, a bare radix
    /// prefix, or a protobuf duration's bare `s`.
    NoDigits = 6,
    /// A sign where none may stand: a second sign, inside parentheses, mid-token, leading
    /// plus trailing.
    SignMisplaced = 7,
    /// `(` without `)`, or `( )`.
    ParenthesesUnbalanced = 8,
    /// A once-only mark appears twice (a second decimal separator, a second `T`).
    MarkRepeated = 9,
    /// A group separator not between digits, or an empty group.
    GroupMisplaced = 10,
    /// A group with the wrong digit count for the declared grouping, or a first group that
    /// starts with a zero.
    GroupSize = 11,
    /// Grouping after the decimal separator or in an exponent.
    GroupInFraction = 12,
    /// Separator detection can't tell `.` from `,`.
    AmbiguousSeparator = 13,
    /// A fraction where the type is whole: an integer door, a Unix count, a negative exponent
    /// at an integer door, a fraction on a duration unit other than seconds.
    FractionNotAllowed = 14,
    /// A delimited field has the wrong number of digits.
    FieldWidth = 15,
    /// A fixed-shape token, recognised by its total length, has the wrong length.
    WrongLength = 16,
    /// A required part of the value is absent: a zone, seconds, a duration unit or any
    /// duration component, the UUID after a bare `GUID:`.
    ComponentMissing = 17,
    /// Not one of the door's accepted words or values.
    NotRecognized = 18,
    /// An unknown or misplaced duration unit.
    UnknownUnit = 19,
    /// The bytes where the cast failed don't decode as UTF-8.
    InvalidUtf8 = 20,
    /// Past an implementation limit: a 256-byte numeral, an 18-digit run, a span past
    /// `u32::MAX`.
    TooLong = 21,
    /// A non-finite value: a NaN or infinity spelled at a numeric door, or a NaN or ∞ given
    /// to a door that can't represent it.
    NotFinite = 22,
    /// A non-integral number at a whole-number door.
    NotWhole = 23,
    /// Negative where the type has no negatives (an unsigned integer, an Excel serial).
    NegativeNotAllowed = 24,
    /// The magnitude exceeds what the target type represents.
    TooLarge = 25,
    /// Below a signed integer type's minimum.
    TooSmall = 26,
    /// More fractional digits than the type holds.
    TooPrecise = 27,
    /// A date field that names nothing: year 0, month 13, a day past the month's end.
    DateFieldOutOfRange = 28,
    /// An hour, minute, second or offset field out of range.
    TimeFieldOutOfRange = 29,
    /// A well-formed instant or duration past the supported window (years 0001–9999,
    /// ±10,000 years).
    OutsideWindow = 30,
    /// A day that never existed: Excel's phantom 1900-02-29.
    NonexistentDate = 31,
    /// A code point above U+10FFFF, or a surrogate.
    NotAScalar = 32,
}

impl Detail {
    /// Every detail, in code order.
    #[cfg(test)]
    pub(crate) const ALL: [Detail; 32] = [
        Detail::Blank,
        Detail::UnexpectedCharacter,
        Detail::DigitExpected,
        Detail::DelimiterExpected,
        Detail::TrailingText,
        Detail::NoDigits,
        Detail::SignMisplaced,
        Detail::ParenthesesUnbalanced,
        Detail::MarkRepeated,
        Detail::GroupMisplaced,
        Detail::GroupSize,
        Detail::GroupInFraction,
        Detail::AmbiguousSeparator,
        Detail::FractionNotAllowed,
        Detail::FieldWidth,
        Detail::WrongLength,
        Detail::ComponentMissing,
        Detail::NotRecognized,
        Detail::UnknownUnit,
        Detail::InvalidUtf8,
        Detail::TooLong,
        Detail::NotFinite,
        Detail::NotWhole,
        Detail::NegativeNotAllowed,
        Detail::TooLarge,
        Detail::TooSmall,
        Detail::TooPrecise,
        Detail::DateFieldOutOfRange,
        Detail::TimeFieldOutOfRange,
        Detail::OutsideWindow,
        Detail::NonexistentDate,
        Detail::NotAScalar,
    ];

    /// The detail's ABI code, `1` through `32` today.
    pub const fn code(self) -> u32 {
        self as u32
    }

    /// The detail whose ABI code is `code`, or `None` for any other value: `0` ("no detail")
    /// and codes this version doesn't know included.
    pub const fn from_code(code: u32) -> Option<Detail> {
        Some(match code {
            1 => Detail::Blank,
            2 => Detail::UnexpectedCharacter,
            3 => Detail::DigitExpected,
            4 => Detail::DelimiterExpected,
            5 => Detail::TrailingText,
            6 => Detail::NoDigits,
            7 => Detail::SignMisplaced,
            8 => Detail::ParenthesesUnbalanced,
            9 => Detail::MarkRepeated,
            10 => Detail::GroupMisplaced,
            11 => Detail::GroupSize,
            12 => Detail::GroupInFraction,
            13 => Detail::AmbiguousSeparator,
            14 => Detail::FractionNotAllowed,
            15 => Detail::FieldWidth,
            16 => Detail::WrongLength,
            17 => Detail::ComponentMissing,
            18 => Detail::NotRecognized,
            19 => Detail::UnknownUnit,
            20 => Detail::InvalidUtf8,
            21 => Detail::TooLong,
            22 => Detail::NotFinite,
            23 => Detail::NotWhole,
            24 => Detail::NegativeNotAllowed,
            25 => Detail::TooLarge,
            26 => Detail::TooSmall,
            27 => Detail::TooPrecise,
            28 => Detail::DateFieldOutOfRange,
            29 => Detail::TimeFieldOutOfRange,
            30 => Detail::OutsideWindow,
            31 => Detail::NonexistentDate,
            32 => Detail::NotAScalar,
            _ => return None,
        })
    }

    /// The detail's name, the snake-case spelling the corpus and every binding use
    /// (`unexpected_character`).
    pub const fn name(self) -> &'static str {
        match self {
            Detail::Blank => "blank",
            Detail::UnexpectedCharacter => "unexpected_character",
            Detail::DigitExpected => "digit_expected",
            Detail::DelimiterExpected => "delimiter_expected",
            Detail::TrailingText => "trailing_text",
            Detail::NoDigits => "no_digits",
            Detail::SignMisplaced => "sign_misplaced",
            Detail::ParenthesesUnbalanced => "parentheses_unbalanced",
            Detail::MarkRepeated => "mark_repeated",
            Detail::GroupMisplaced => "group_misplaced",
            Detail::GroupSize => "group_size",
            Detail::GroupInFraction => "group_in_fraction",
            Detail::AmbiguousSeparator => "ambiguous_separator",
            Detail::FractionNotAllowed => "fraction_not_allowed",
            Detail::FieldWidth => "field_width",
            Detail::WrongLength => "wrong_length",
            Detail::ComponentMissing => "component_missing",
            Detail::NotRecognized => "not_recognized",
            Detail::UnknownUnit => "unknown_unit",
            Detail::InvalidUtf8 => "invalid_utf8",
            Detail::TooLong => "too_long",
            Detail::NotFinite => "not_finite",
            Detail::NotWhole => "not_whole",
            Detail::NegativeNotAllowed => "negative_not_allowed",
            Detail::TooLarge => "too_large",
            Detail::TooSmall => "too_small",
            Detail::TooPrecise => "too_precise",
            Detail::DateFieldOutOfRange => "date_field_out_of_range",
            Detail::TimeFieldOutOfRange => "time_field_out_of_range",
            Detail::OutsideWindow => "outside_window",
            Detail::NonexistentDate => "nonexistent_date",
            Detail::NotAScalar => "not_a_scalar",
        }
    }
}

impl core::fmt::Display for Detail {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// A [`Fault`] with its [`Detail`]: the failure case of a [`detailed`](crate::detailed) door.
/// The reason and span are exactly the plain door's fault ([`fault`](Self::fault)); the
/// detail says what in that span was wrong. More fields may join in a minor release.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Failure {
    /// The closed-set reason, as the plain door reports it.
    pub reason: Reason,
    /// What exactly failed.
    pub detail: Detail,
    /// Byte offset of the offending span in the caller's input.
    pub offset: u32,
    /// Byte length of the offending span.
    pub len: u32,
}

impl Failure {
    /// A failure with every field given, as a detailed door would return it: for a test
    /// double or a downstream adapter. A field that joins in a minor release takes its
    /// neutral value here.
    pub const fn new(reason: Reason, detail: Detail, offset: u32, len: u32) -> Failure {
        Failure { reason, detail, offset, len }
    }

    /// The plain door's fault: this failure without its detail.
    pub const fn fault(self) -> Fault {
        Fault { reason: self.reason, offset: self.offset, len: self.len }
    }

    /// The failure `site` names, spanning `len` bytes at `offset` in the caller's input
    /// (clamped as every span is).
    #[inline]
    pub(crate) fn at(site: Site, offset: usize, len: usize) -> Failure {
        site.fired();
        let (offset, len) = span_of(offset, len);
        Failure { reason: site.reason(), detail: site.detail(), offset, len }
    }

    /// The failure `site` names with an empty span at the start: empty input, or a number
    /// already decoded, which has no text.
    #[inline]
    pub(crate) fn spanless(site: Site) -> Failure {
        Failure::at(site, 0, 0)
    }

    /// This failure as a detailed door reports it for `input`: [`Detail::InvalidUtf8`] in
    /// place of the site's detail when the bytes at the span, widened to the whole UTF-8
    /// sequences it starts and ends in, don't decode. Only the winning failure is looked at,
    /// and only its bytes; the reason and span never change.
    #[inline]
    pub(crate) fn classified(self, input: &[u8]) -> Failure {
        if self.len == 0 || self.detail == Detail::InvalidUtf8 {
            return self;
        }
        let start = self.offset as usize;
        let end = start.saturating_add(self.len as usize).min(input.len());
        // An all-ASCII span starts and ends on a character's edge, so it widens to itself and
        // decodes: the common case, settled without the walk below.
        if input.get(start..end).is_some_and(<[u8]>::is_ascii) {
            return self;
        }
        // Back to the lead byte of a sequence the span starts inside.
        let mut from = start;
        let continuation = |at: usize| input.get(at).is_some_and(|&byte| byte & 0xC0 == 0x80);
        while from > 0 && start - from < 3 && continuation(from) {
            from -= 1;
        }
        // On to the end of a sequence the span stops inside.
        let mut to = end;
        let mut lead = end;
        while lead > from && end - lead < 4 {
            lead -= 1;
            if !continuation(lead) {
                break;
            }
        }
        let width = match input.get(lead) {
            Some(&byte) if byte >= 0xF0 => 4,
            Some(&byte) if byte >= 0xE0 => 3,
            Some(&byte) if byte >= 0xC0 => 2,
            _ => 1,
        };
        if lead + width > to {
            to = (lead + width).min(input.len());
        }
        match input.get(from..to) {
            Some(bytes) if !utf8_ok(bytes) => Failure { detail: Detail::InvalidUtf8, ..self },
            _ => self,
        }
    }
}

impl From<Failure> for Fault {
    fn from(failure: Failure) -> Fault {
        failure.fault()
    }
}

impl core::fmt::Display for Failure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} ({})", self.fault(), self.detail)
    }
}

/// A `Failure` composes with ordinary Rust error handling, as a [`Fault`] does.
impl core::error::Error for Failure {}
