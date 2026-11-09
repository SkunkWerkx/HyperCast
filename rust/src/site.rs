//! Every place a cast fails, named: one [`Site`] per construction of a failure, each with
//! the one [`Reason`] and [`Detail`] it reports ([`SITES`]). `docs/fault-sites.md` is
//! generated from this table by `tests.rs`'s `docs_fault_sites_is_current`, and
//! `every_site_is_reached` replays the corpus to show that every site but the ones listed as
//! unreachable (each with its reason) fires, with the detail the table gives it.
//!
//! A site's detail is fixed. Where one condition in the code covers distinct failures, each
//! failure is its own site, so the table states every classification the doors make. The one
//! detail a door decides beyond its site is [`Detail::InvalidUtf8`], from the bytes at the
//! winning span ([`Failure::classified`](crate::Failure)).

use crate::detail::Detail;
use crate::verdict::Reason;

macro_rules! sites {
    ($(
        $(#[doc = $doc:literal])+
        $site:ident = $file:literal, $reason:ident, $detail:ident $(, $kind:ident $($note:literal)+)?;
    )+) => {
        /// A place a cast fails. Each variant's doc is the condition, and [`SITES`] its
        /// reason and detail.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub(crate) enum Site {
            $(
                $(#[doc = $doc])+
                #[doc = ""]
                #[doc = concat!("Raised in `", $file, "`.")]
                $site,
            )+
        }

        impl Site {
            /// The reason this site reports.
            #[inline(always)]
            pub(crate) const fn reason(self) -> Reason {
                match self {
                    $(Site::$site => Reason::$reason,)+
                }
            }

            /// The detail this site reports.
            #[inline(always)]
            pub(crate) const fn detail(self) -> Detail {
                match self {
                    $(Site::$site => Detail::$detail,)+
                }
            }
        }

        /// Every site with its reason and detail, in source order.
        #[cfg_attr(not(test), allow(dead_code))]
        pub(crate) const SITES: &[(Site, Reason, Detail)] = &[
            $((Site::$site, Reason::$reason, Detail::$detail),)+
        ];

        /// The sites no corpus row reaches, each marked `unreachable` (no input reaches it)
        /// or `impractical` (only an input no corpus row can carry does) or
        /// `impractical_on_64bit` (likewise where `usize` is 64 bits; structurally unreachable
        /// where it is 32), with its notes: the reason, and for an impractical site first the
        /// unit test that reaches it.
        #[cfg(test)]
        pub(crate) const MARKS: &[(Site, &str, &[&str])] = &[
            $($((Site::$site, stringify!($kind), &[$($note),+]),)?)+
        ];

        /// Each site's file and condition, for `docs/fault-sites.md`.
        #[cfg(test)]
        pub(crate) const DOCS: &[(Site, &str, &str)] = &[
            $((Site::$site, $file, concat!($($doc),+)),)+
        ];
    };
}

impl Site {
    /// Notes that this site produced a failure, for `every_site_is_reached`: only in the
    /// unit-test build, and only on a thread that is recording.
    #[inline(always)]
    pub(crate) fn fired(self) {
        #[cfg(test)]
        recorder::record(self);
    }
}

/// The sites marked `unreachable`, with the reason.
#[cfg(test)]
pub(crate) fn unreachable() -> impl Iterator<Item = (Site, &'static str)> {
    MARKS
        .iter()
        .filter(|(_, kind, _)| *kind == "unreachable")
        .map(|&(site, _, notes)| (site, notes[0]))
}

/// The sites marked `impractical` or `impractical_on_64bit`, with the unit test that reaches
/// each, the reason no corpus row does, and whether only a 64-bit `usize` can reach it.
#[cfg(test)]
pub(crate) fn impractical() -> impl Iterator<Item = (Site, &'static str, &'static str, bool)> {
    MARKS.iter().filter(|(_, kind, _)| kind.starts_with("impractical")).map(
        |&(site, kind, notes)| {
            (site, notes[0], notes.get(1).copied().unwrap_or(""), kind == "impractical_on_64bit")
        },
    )
}

/// The unit-test build's record of the sites that fired, kept per thread so that a replay
/// sees only its own calls and never those of a test running beside it.
#[cfg(test)]
pub(crate) mod recorder {
    use super::Site;
    use std::cell::RefCell;
    use std::vec::Vec;

    std::thread_local! {
        static FIRED: RefCell<Option<Vec<Site>>> = const { RefCell::new(None) };
    }

    pub(super) fn record(site: Site) {
        FIRED.with(|fired| {
            if let Some(fired) = fired.borrow_mut().as_mut() {
                fired.push(site);
            }
        });
    }

    /// Starts recording on this thread, forgetting anything recorded before.
    pub(crate) fn start() {
        FIRED.with(|fired| *fired.borrow_mut() = Some(Vec::new()));
    }

    /// What fired on this thread since the last `start` or `take`, in order; recording
    /// continues.
    pub(crate) fn take() -> Vec<Site> {
        FIRED.with(|fired| fired.borrow_mut().as_mut().map(core::mem::take).unwrap_or_default())
    }

    /// Stops recording on this thread.
    pub(crate) fn stop() {
        FIRED.with(|fired| *fired.borrow_mut() = None);
    }
}

sites! {
    // --- boolean.rs ---
    /// The trimmed input is empty.
    BoolBlank = "boolean.rs", Empty, Blank;
    /// The trimmed input is not one of the lexicon's words.
    BoolNotRecognized = "boolean.rs", Malformed, NotRecognized;

    // --- character.rs ---
    /// The input is empty.
    CharEmpty = "character.rs", Empty, Blank;
    /// The input is only white space, and not one white-space character (which is verbatim).
    CharBlank = "character.rs", Empty, Blank;
    /// `&` is followed by something other than `#` or `H`, or by nothing.
    CharAmpersandMark = "character.rs", Malformed, DelimiterExpected;
    /// No prefix, and the token doesn't start with a decimal digit: not a code-point spelling.
    CharNotASpelling = "character.rs", Malformed, UnexpectedCharacter;
    /// A prefix (`U+`, `0x`, `&H`, `&#`, `&#x`) is followed by a character that is not a digit.
    CharDigitExpected = "character.rs", Malformed, DigitExpected;
    /// A bare prefix (`U+`, `0x`, `&H`, `&#`, `&#x`) with nothing after it: no numeral at all,
    /// as a bare `0x` is at the integer doors; the span is the whole token.
    CharBarePrefix = "character.rs", Malformed, NoDigits;
    /// An entity's digits end the input without its `;`; the span is the whole token.
    CharEntityUnterminated = "character.rs", Malformed, DelimiterExpected;
    /// An entity's digits are followed by a character other than `;`.
    CharEntityStray = "character.rs", Malformed, UnexpectedCharacter;
    /// More text follows an entity's `;`.
    CharEntityTrailing = "character.rs", Malformed, TrailingText;
    /// A non-entity spelling's digits are followed by another character.
    CharAfterDigits = "character.rs", Malformed, UnexpectedCharacter;
    /// The spelling names a code point above U+10FFFF or a surrogate.
    CharNotAScalar = "character.rs", OutOfRange, NotAScalar;

    // --- text.rs ---
    /// The input is empty.
    TextEmpty = "text.rs", Empty, Blank;
    /// The input is not UTF-8; the span is the first invalid sequence.
    TextInvalidUtf8 = "text.rs", Malformed, InvalidUtf8;

    // --- verdict.rs ---
    /// Separator detection finds a single `.` or `,` with exactly three digits after it and an
    /// integer part other than `0`.
    DetectAmbiguous = "verdict.rs", Malformed, AmbiguousSeparator;

    // --- integer.rs: the frame every numeric door shares ---
    /// Only a percent symbol (and a trailing sign) at a real or decimal door.
    FramePercentOnly = "integer.rs", Malformed, NoDigits;
    /// `(` without a closing `)`.
    FrameParensUnclosed = "integer.rs", Malformed, ParenthesesUnbalanced;
    /// Accounting parentheses with nothing inside.
    FrameParensEmpty = "integer.rs", Malformed, ParenthesesUnbalanced;
    /// A leading currency symbol (and sign) with nothing after it.
    FrameCurrencyOnlyLeading = "integer.rs", Malformed, NoDigits;
    /// A trailing currency symbol with nothing before it (a symbol that starts with a sign).
    FrameCurrencyOnlyTrailing = "integer.rs", Malformed, NoDigits;
    /// A sign after a trailing currency symbol and another after the percent; the span is the
    /// inner one.
    FrameSignAfterSymbolAndPercent = "integer.rs", Malformed, SignMisplaced;
    /// A sign ahead of a leading currency symbol inside accounting parentheses (`(-$5)`); the
    /// span is that sign.
    FrameSignBeforeSymbolInParens = "integer.rs", Malformed, SignMisplaced;
    /// A sign ahead of a leading currency symbol and another after it (`-$-5`); the span is
    /// the second.
    FrameSecondSignAfterSymbol = "integer.rs", Malformed, SignMisplaced;
    /// A sign inside accounting parentheses (`(-5)`).
    FrameSignInParens = "integer.rs", Malformed, SignMisplaced;
    /// A trailing sign on a token that is already signed or in parentheses.
    FrameTrailingSignTwice = "integer.rs", Malformed, SignMisplaced;
    /// A sign after the percent on a token that is already signed or in parentheses.
    FramePostSignTwice = "integer.rs", Malformed, SignMisplaced;

    // --- integer.rs: the integer doors ---
    /// The trimmed input is empty.
    IntBlank = "integer.rs", Empty, Blank;
    /// The plain-digits fast path reads a negative value at an unsigned door.
    IntFastNegative = "integer.rs", OutOfRange, NegativeNotAllowed;
    /// The plain-digits fast path reads a value above the type's maximum.
    IntFastTooLarge = "integer.rs", OutOfRange, TooLarge;
    /// The plain-digits fast path reads a value below a signed type's minimum.
    IntFastTooSmall = "integer.rs", OutOfRange, TooSmall;
    /// A radix prefix (`0x`, `&H`, `0b`) with no digits after it.
    RadixNoDigits = "integer.rs", Malformed, NoDigits;
    /// A character after a radix prefix that is not a digit of that radix.
    RadixDigitExpected = "integer.rs", Malformed, DigitExpected;
    /// A radix bit pattern wider than the type.
    RadixTooLarge = "integer.rs", OutOfRange, TooLarge;
    /// The lenient lane reads a negative value at an unsigned door.
    IntLaneNegative = "integer.rs", OutOfRange, NegativeNotAllowed;
    /// The lenient lane reads a value above the type's maximum.
    IntLaneTooLarge = "integer.rs", OutOfRange, TooLarge;
    /// The lenient lane reads a value below a signed type's minimum.
    IntLaneTooSmall = "integer.rs", OutOfRange, TooSmall;
    /// A decimal separator at an integer door.
    IntDecimalPoint = "integer.rs", Malformed, FractionNotAllowed;
    /// A group separator not between two digits.
    IntGroupMisplaced = "integer.rs", Malformed, GroupMisplaced;
    /// A group its grouping policy refuses: the wrong number of digits, or a first group that
    /// starts with a zero. The span is the group, with the separator before it unless it
    /// is the leftmost.
    IntGroupSize = "integer.rs", Malformed, GroupSize;
    /// A negative exponent at an integer door; the span is its `-`.
    IntNegativeExponent = "integer.rs", Malformed, FractionNotAllowed;
    /// An `e` (and its sign) with no exponent digit after it; the span is the `e`.
    IntExponentDigitExpected = "integer.rs", Malformed, DigitExpected;
    /// A sign after the exponent's digits.
    IntAfterExponentSign = "integer.rs", Malformed, SignMisplaced;
    /// A group separator after the exponent's digits.
    IntGroupInExponent = "integer.rs", Malformed, GroupInFraction;
    /// Another character after the exponent's digits.
    IntAfterExponent = "integer.rs", Malformed, UnexpectedCharacter;
    /// A second leading sign, or a sign inside the numeral.
    IntSignMisplaced = "integer.rs", Malformed, SignMisplaced;
    /// The token, less its frame, is `nan`, `inf` or `infinity` (ASCII case-insensitive); the
    /// span is its first letter.
    IntNotFinite = "integer.rs", Malformed, NotFinite;
    /// Any other character the numeral doesn't allow: a letter, an undeclared symbol, `%`,
    /// `e` with exponents off, interior white space.
    IntUnexpected = "integer.rs", Malformed, UnexpectedCharacter;
    /// Only a sign: no digit at all.
    IntNoDigits = "integer.rs", Malformed, NoDigits;
    /// The magnitude overflows 128 bits, negative, at an unsigned door.
    IntOverflowNegative = "integer.rs", OutOfRange, NegativeNotAllowed;
    /// The magnitude overflows 128 bits, positive.
    IntOverflowTooLarge = "integer.rs", OutOfRange, TooLarge;
    /// The magnitude overflows 128 bits, negative, at a signed door.
    IntOverflowTooSmall = "integer.rs", OutOfRange, TooSmall;
    /// The exponent scales the value past 128 bits, negative, at an unsigned door.
    IntScaledNegative = "integer.rs", OutOfRange, NegativeNotAllowed;
    /// The exponent scales the value past 128 bits, positive.
    IntScaledTooLarge = "integer.rs", OutOfRange, TooLarge;
    /// The exponent scales the value past 128 bits, negative, at a signed door.
    IntScaledTooSmall = "integer.rs", OutOfRange, TooSmall;
    /// The engine reads a negative value at an unsigned door.
    IntRangeNegative = "integer.rs", OutOfRange, NegativeNotAllowed;
    /// The engine reads a value above the type's maximum.
    IntRangeTooLarge = "integer.rs", OutOfRange, TooLarge;
    /// The engine reads a value below a signed type's minimum.
    IntRangeTooSmall = "integer.rs", OutOfRange, TooSmall;

    // --- real.rs (the engine walk is shared with the decimal door) ---
    /// The trimmed input is empty.
    RealBlank = "real.rs", Empty, Blank;
    /// The conversion declines the text the engine wrote, which is always in its shape.
    RealUnparsed = "real.rs", Malformed, UnexpectedCharacter,
        unreachable "the engine only writes `[-]digits[.digits][e[+|-]digits]` with a digit, which the conversion always reads";
    /// A magnitude past the type's finite range, read by the engine.
    RealEngineTooLarge = "real.rs", OutOfRange, TooLarge;
    /// A magnitude past the type's finite range, read by the plain shape or the lenient lane.
    RealTooLarge = "real.rs", OutOfRange, TooLarge;
    /// A second decimal separator.
    RealMarkRepeated = "real.rs", Malformed, MarkRepeated;
    /// A group separator after the decimal separator.
    RealGroupInFraction = "real.rs", Malformed, GroupInFraction;
    /// A group separator not between two digits.
    RealGroupMisplaced = "real.rs", Malformed, GroupMisplaced;
    /// A group its grouping policy refuses: the wrong number of digits, or a first group that
    /// starts with a zero. The span is the group, with the separator before it unless it
    /// is the leftmost.
    RealGroupSize = "real.rs", Malformed, GroupSize;
    /// An `e` (and its sign) with no exponent digit after it; the span is the `e`.
    RealExponentDigitExpected = "real.rs", Malformed, DigitExpected;
    /// A sign after the exponent's digits.
    RealAfterExponentSign = "real.rs", Malformed, SignMisplaced;
    /// A group separator after the exponent's digits.
    RealGroupInExponent = "real.rs", Malformed, GroupInFraction;
    /// Another character after the exponent's digits.
    RealAfterExponent = "real.rs", Malformed, UnexpectedCharacter;
    /// A second leading sign, or a sign inside the numeral.
    RealSignMisplaced = "real.rs", Malformed, SignMisplaced;
    /// The token, less its frame, is `nan`, `inf` or `infinity` (ASCII case-insensitive); the
    /// span is its first letter.
    RealNotFinite = "real.rs", Malformed, NotFinite;
    /// Any other character the numeral doesn't allow: a letter, an undeclared symbol, `e`
    /// with exponents off, interior white space.
    RealUnexpected = "real.rs", Malformed, UnexpectedCharacter;
    /// Only a sign: no digit at all.
    RealNoDigits = "real.rs", Malformed, NoDigits;
    /// The numeral normalizes to more than 256 bytes.
    RealTooLong = "real.rs", Malformed, TooLong;

    // --- decimal.rs ---
    /// The trimmed input is empty.
    DecimalBlank = "decimal.rs", Empty, Blank;
    /// The whole part of the magnitude overflows 128 bits, or a fraction digit lands on a
    /// whole part already past 96 bits.
    DecimalWholeTooLarge = "decimal.rs", OutOfRange, TooLarge;
    /// A nonzero fraction digit no longer fits 96 bits.
    DecimalFractionTooPrecise = "decimal.rs", OutOfRange, TooPrecise;
    /// The invariant text has no digit.
    DecimalNoDigits = "decimal.rs", Malformed, NoDigits,
        unreachable "the plain shape and the engine's text always carry a digit";
    /// The invariant text's exponent has no digit.
    DecimalExponentDigitExpected = "decimal.rs", Malformed, DigitExpected,
        unreachable "the plain shape and the engine's text always carry an exponent digit";
    /// The invariant text has more after the numeral.
    DecimalTrailing = "decimal.rs", Malformed, TrailingText,
        unreachable "the plain shape and the engine's text are read to their end";
    /// The exponent shifts the magnitude past 96 bits.
    DecimalShiftTooLarge = "decimal.rs", OutOfRange, TooLarge;
    /// The magnitude, its exact zeros shed, is past 96 bits.
    DecimalTooLarge = "decimal.rs", OutOfRange, TooLarge;
    /// The scale, its exact zeros shed, is past 28.
    DecimalTooPrecise = "decimal.rs", OutOfRange, TooPrecise;

    // --- typed.rs ---
    /// A NaN or an infinity at a whole-number door.
    WholeNotFinite = "typed.rs", Malformed, NotFinite;
    /// A finite double with a fractional part at a whole-number door.
    WholeNotWhole = "typed.rs", Malformed, NotWhole;
    /// An integral double from 2^53 up has no shortest digits.
    WholeUnrepresentable = "typed.rs", Malformed, NotFinite,
        unreachable "every finite nonzero double has shortest digits";
    /// A negative whole number at an unsigned integer door.
    TypedIntNegative = "typed.rs", OutOfRange, NegativeNotAllowed;
    /// A whole number above the integer type's maximum.
    TypedIntTooLarge = "typed.rs", OutOfRange, TooLarge;
    /// A whole number below a signed integer type's minimum.
    TypedIntTooSmall = "typed.rs", OutOfRange, TooSmall;
    /// NaN at the `f32` door.
    F32TypedNan = "typed.rs", Malformed, NotFinite;
    /// An infinity at the `f32` door.
    F32TypedInfinite = "typed.rs", OutOfRange, NotFinite;
    /// A finite double past `f32::MAX`.
    F32TypedTooLarge = "typed.rs", OutOfRange, TooLarge;
    /// NaN or an infinity at the `bool` door.
    BoolTypedNotFinite = "typed.rs", Malformed, NotFinite;
    /// A finite double other than `0` or `1` at the `bool` door.
    BoolTypedNotRecognized = "typed.rs", Malformed, NotRecognized;
    /// NaN at the decimal door.
    DecimalTypedNan = "typed.rs", Malformed, NotFinite;
    /// An infinity at the decimal door.
    DecimalTypedInfinite = "typed.rs", OutOfRange, NotFinite;
    /// The shortest digits shifted by their exponent overflow 128 bits.
    DecimalTypedShiftTooLarge = "typed.rs", OutOfRange, TooLarge;
    /// The magnitude is past 96 bits.
    DecimalTypedTooLarge = "typed.rs", OutOfRange, TooLarge;
    /// The scale is past 28.
    DecimalTypedTooPrecise = "typed.rs", OutOfRange, TooPrecise;

    // --- checked.rs ---
    /// NaN at the checked `f64` door.
    CheckedF64Nan = "checked.rs", Malformed, NotFinite;
    /// An infinity at the checked `f64` door.
    CheckedF64Infinite = "checked.rs", OutOfRange, NotFinite;
    /// A success span that ends past `u32::MAX` bytes.
    CheckedSpanTooLong = "checked.rs", OutOfRange, TooLong,
        impractical_on_64bit "checked_span_too_long" "only an input past 4 GiB reaches it";
    /// `multipleOf` rounding yields a multiple too long to hold.
    CheckedMultipleTooLong = "checked.rs", OutOfRange, TooLarge,
        unreachable "a declared step is never zero, and a value a door accepts rounds to at most 337 digits (an `f64` near its maximum at a step of scale 28), within the 340 `Rounded` holds";
    /// `multipleOf` rounding yields a multiple past what the door's type holds.
    CheckedRoundedTooLarge = "checked.rs", OutOfRange, TooLarge;
    /// `multipleOf` rounding yields a multiple below a signed integer type's minimum.
    CheckedRoundedTooSmall = "checked.rs", OutOfRange, TooSmall;
    /// The text door's input is white space only, under `trim`.
    CheckedTextBlank = "checked.rs", Empty, Blank;

    // --- uuid.rs ---
    /// The trimmed input is empty.
    UuidBlank = "uuid.rs", Empty, Blank;
    /// A prefix (`urn:uuid:`, `GUID:`, `UUID:`) with nothing after it.
    UuidBarePrefix = "uuid.rs", Malformed, ComponentMissing;
    /// A character that is not a hex digit where one is required.
    UuidHexDigit = "uuid.rs", Malformed, DigitExpected;
    /// An unwrapped token of neither 32 nor 36 characters.
    UuidWrongLength = "uuid.rs", Malformed, WrongLength;
    /// A D-format hyphen is missing or different.
    UuidHyphen = "uuid.rs", Malformed, DelimiterExpected;
    /// An N-format token of other than 32 characters.
    UuidNWrongLength = "uuid.rs", Malformed, WrongLength,
        unreachable "the N format is chosen only for 32 characters";
    /// The digits fail to decode but no pair holds a non-hex byte.
    UuidNoBadPair = "uuid.rs", Malformed, DigitExpected,
        unreachable "the decoder refuses exactly the pairs holding a non-hex byte";
    /// A braced or parenthesized token of other than 38 characters.
    UuidWrappedWrongLength = "uuid.rs", Malformed, WrongLength;
    /// A braced or parenthesized token without its closing mark.
    UuidWrappedClose = "uuid.rs", Malformed, DelimiterExpected;
    /// An X-format mark (`{`, `,`, `}`) is different.
    UuidXDelimiter = "uuid.rs", Malformed, DelimiterExpected;
    /// The X format ends before a mark; the span is the whole token.
    UuidXEnded = "uuid.rs", Malformed, DelimiterExpected;
    /// An X-format component without its `0x`.
    UuidXPrefix = "uuid.rs", Malformed, DelimiterExpected;
    /// An X-format component with more digits than its width.
    UuidXWidth = "uuid.rs", Malformed, FieldWidth;
    /// An X-format component's `0x` with no digit after it.
    UuidXDigitExpected = "uuid.rs", Malformed, DigitExpected;
    /// More text after the X format's closing `}`.
    UuidXTrailing = "uuid.rs", Malformed, TrailingText;

    // --- temporal.rs: shared pieces ---
    /// A decimal mark with no digit after it; the span is the mark.
    FractionDigitExpected = "temporal.rs", Malformed, DigitExpected;
    /// A digit run past eighteen digits; the span is the nineteenth.
    DigitRunTooLong = "temporal.rs", Malformed, TooLong;
    /// The ISO date's year is too short to read.
    DateYearShort = "temporal.rs", Malformed, WrongLength,
        unreachable "the ISO date is read only from ten bytes";
    /// The ISO date's year is not four digits.
    DateYearDigits = "temporal.rs", Malformed, DigitExpected;
    /// The ISO date's first `-` is missing or different.
    DateFirstHyphen = "temporal.rs", Malformed, DelimiterExpected;
    /// The ISO date's month is not two digits.
    DateMonthDigits = "temporal.rs", Malformed, DigitExpected;
    /// The ISO date's second `-` is missing or different.
    DateSecondHyphen = "temporal.rs", Malformed, DelimiterExpected;
    /// The ISO date's day is not two digits.
    DateDayDigits = "temporal.rs", Malformed, DigitExpected;
    /// Year 0000.
    DateYearRange = "temporal.rs", OutOfRange, DateFieldOutOfRange;
    /// A month outside 1–12.
    DateMonthRange = "temporal.rs", OutOfRange, DateFieldOutOfRange;
    /// A day the month doesn't have.
    DateDayRange = "temporal.rs", OutOfRange, DateFieldOutOfRange;
    /// An hour outside the clock's range.
    ClockHourRange = "temporal.rs", OutOfRange, TimeFieldOutOfRange;
    /// A minute past 59.
    ClockMinuteRange = "temporal.rs", OutOfRange, TimeFieldOutOfRange;
    /// A second past 59 (a leap second included).
    ClockSecondRange = "temporal.rs", OutOfRange, TimeFieldOutOfRange;

    // --- temporal.rs: cast_date ---
    /// The trimmed input is empty.
    DateBlank = "temporal.rs", Empty, Blank;
    /// The trimmed input is not ten characters long.
    DateWrongLength = "temporal.rs", Malformed, WrongLength;

    // --- temporal.rs: separated dates (cast_date_ordered, cast_datetime) ---
    /// A date field with no digit.
    OrderedFieldDigitExpected = "temporal.rs", Malformed, DigitExpected;
    /// A date field of five or more digits.
    OrderedFieldWidth = "temporal.rs", Malformed, FieldWidth;
    /// The first separator is not `/`, `-` or `.`, or is missing.
    OrderedFirstSeparator = "temporal.rs", Malformed, DelimiterExpected;
    /// The second separator is missing or differs from the first.
    OrderedSecondSeparator = "temporal.rs", Malformed, DelimiterExpected;
    /// The month field is more than two digits.
    OrderedMonthWidth = "temporal.rs", Malformed, FieldWidth;
    /// The day field is more than two digits.
    OrderedDayWidth = "temporal.rs", Malformed, FieldWidth;
    /// The year field is not four digits (nor two, under a pivot).
    OrderedYearWidth = "temporal.rs", Malformed, FieldWidth;
    /// A month field that starts with a word, under a vocabulary, that is not exactly one of
    /// its names; the span is the word.
    OrderedMonthNotRecognized = "temporal.rs", Malformed, NotRecognized;
    /// A month field that is exactly a declared name, under a vocabulary, with a different
    /// mark or none after it; the span is that character, empty at the end.
    OrderedMonthDelimiter = "temporal.rs", Malformed, DelimiterExpected;
    /// The trimmed input is empty (`cast_date_ordered`).
    OrderedBlank = "temporal.rs", Empty, Blank;
    /// More text after a complete date (`cast_date_ordered`).
    OrderedTrailing = "temporal.rs", Malformed, TrailingText;

    // --- temporal.rs: cast_datetime ---
    /// The trimmed input is empty.
    DatetimeBlank = "temporal.rs", Empty, Blank;
    /// A character other than a space or `T` after the date.
    DatetimeSeparator = "temporal.rs", Malformed, DelimiterExpected;
    /// The hour has no digit.
    CivilHourDigitExpected = "temporal.rs", Malformed, DigitExpected;
    /// The hour has more than two digits.
    CivilHourWidth = "temporal.rs", Malformed, FieldWidth;
    /// The minutes are not two digits.
    CivilMinuteDigits = "temporal.rs", Malformed, DigitExpected;
    /// The seconds are not two digits.
    CivilSecondDigits = "temporal.rs", Malformed, DigitExpected;
    /// A bare hour with neither minutes nor a day-period marker; the span is the hour.
    CivilMinutesMissing = "temporal.rs", Malformed, ComponentMissing;
    /// More text after a complete time.
    DatetimeTrailing = "temporal.rs", Malformed, TrailingText;

    // --- temporal.rs: cast_time ---
    /// The trimmed input is empty.
    TimeBlank = "temporal.rs", Empty, Blank;
    /// More text after a complete time.
    TimeTrailing = "temporal.rs", Malformed, TrailingText;
    /// The hour is not two digits.
    TimeHourDigits = "temporal.rs", Malformed, DigitExpected;
    /// The `:` after the hour is missing or different.
    TimeColon = "temporal.rs", Malformed, DelimiterExpected;
    /// The minutes are not two digits.
    TimeMinuteDigits = "temporal.rs", Malformed, DigitExpected;
    /// The seconds are not two digits.
    TimeSecondDigits = "temporal.rs", Malformed, DigitExpected;

    // --- temporal.rs: cast_timestamp ---
    /// The trimmed input is empty.
    TimestampBlank = "temporal.rs", Empty, Blank;
    /// The trimmed input is shorter than a date and its `T`.
    TimestampTooShort = "temporal.rs", Malformed, WrongLength;
    /// The `T` after the date is missing or different.
    TimestampT = "temporal.rs", Malformed, DelimiterExpected;
    /// The time's first non-digit, ahead of its seconds, is where a digit must be; the span
    /// is the whole time.
    TimestampTimeDigit = "temporal.rs", Malformed, DigitExpected;
    /// The time's first wrong character, ahead of its seconds, is where a `:` must be; the
    /// span is the whole time.
    TimestampTimeColon = "temporal.rs", Malformed, DelimiterExpected;
    /// `HH:mm` ends the input or meets a zone: the seconds are missing; the span is the whole
    /// time.
    TimestampSecondsMissing = "temporal.rs", Malformed, ComponentMissing;
    /// No zone after the time.
    TimestampZoneMissing = "temporal.rs", Malformed, ComponentMissing;
    /// More text after `Z`.
    TimestampTrailingAfterZ = "temporal.rs", Malformed, TrailingText;
    /// The offset's hours are not two digits.
    TimestampOffsetHourDigits = "temporal.rs", Malformed, DigitExpected;
    /// The offset's `:` is missing or different.
    TimestampOffsetColon = "temporal.rs", Malformed, DelimiterExpected;
    /// The offset's minutes are not two digits.
    TimestampOffsetMinuteDigits = "temporal.rs", Malformed, DigitExpected;
    /// More text after the offset.
    TimestampTrailingAfterOffset = "temporal.rs", Malformed, TrailingText;
    /// A character after the time that starts neither a fraction nor a zone.
    TimestampZoneUnexpected = "temporal.rs", Malformed, UnexpectedCharacter;
    /// The offset's hours past 23.
    TimestampOffsetHourRange = "temporal.rs", OutOfRange, TimeFieldOutOfRange;
    /// The offset's minutes past 59.
    TimestampOffsetMinuteRange = "temporal.rs", OutOfRange, TimeFieldOutOfRange;
    /// A real instant outside 0001-01-01 through 9999-12-31 UTC.
    TimestampOutsideWindow = "temporal.rs", OutOfRange, OutsideWindow;

    // --- temporal.rs: cast_unix and unix_from_f64 ---
    /// The trimmed input is empty.
    UnixBlank = "temporal.rs", Empty, Blank;
    /// Only a sign.
    UnixNoDigits = "temporal.rs", Malformed, NoDigits;
    /// A `.` in the count.
    UnixFraction = "temporal.rs", Malformed, FractionNotAllowed;
    /// A second sign.
    UnixSignMisplaced = "temporal.rs", Malformed, SignMisplaced;
    /// Any other non-digit in the count.
    UnixUnexpected = "temporal.rs", Malformed, UnexpectedCharacter;
    /// The count overflows 128 bits.
    UnixOverflow = "temporal.rs", OutOfRange, TooLarge;
    /// The instant is outside the timestamp window.
    UnixOutsideWindow = "temporal.rs", OutOfRange, OutsideWindow;
    /// The typed count overflows 128 bits.
    UnixTypedOverflow = "temporal.rs", OutOfRange, TooLarge;
    /// The typed count's instant is outside the timestamp window.
    UnixTypedOutsideWindow = "temporal.rs", OutOfRange, OutsideWindow;

    // --- temporal.rs: the typed Excel doors ---
    /// NaN or an infinity as a serial.
    SerialNotFinite = "temporal.rs", Malformed, NotFinite;
    /// A negative serial.
    SerialNegative = "temporal.rs", Malformed, NegativeNotAllowed;
    /// A serial past either system's last day.
    SerialPastLastDay = "temporal.rs", OutOfRange, OutsideWindow;
    /// The 1900 system's phantom serial 60 (`1900-02-29`).
    SerialPhantom = "temporal.rs", OutOfRange, NonexistentDate;
    /// A serial below the system's first day, or past `9999-12-31`.
    SerialOutsideWindow = "temporal.rs", OutOfRange, OutsideWindow;
    /// NaN or an infinity as a duration in days.
    DurationTypedNotFinite = "temporal.rs", Malformed, NotFinite;
    /// A duration in days past ±10,000 years.
    DurationTypedOutsideWindow = "temporal.rs", OutOfRange, OutsideWindow;
    /// A duration in days whose snapped size is past ±10,000 years.
    DurationTypedSnappedOutsideWindow = "temporal.rs", OutOfRange, OutsideWindow,
        unreachable "a span inside the window snaps by at most a few microseconds, never past a whole second";

    // --- temporal.rs: cast_excel_serial ---
    /// The trimmed input is empty.
    SerialTextBlank = "temporal.rs", Empty, Blank;
    /// A `-` ahead of the serial.
    SerialTextNegative = "temporal.rs", Malformed, NegativeNotAllowed;
    /// A `+` ahead of the serial: a sign where none may stand (only a `-` is a negative).
    SerialTextPlus = "temporal.rs", Malformed, SignMisplaced;
    /// Another character where the serial's first digit must be.
    SerialTextDigitExpected = "temporal.rs", Malformed, DigitExpected;
    /// A `.` with no digit after it; the span is the `.`.
    SerialTextFractionDigit = "temporal.rs", Malformed, DigitExpected;
    /// White space after the serial, with more text after it.
    SerialTextTrailing = "temporal.rs", Malformed, TrailingText;
    /// Any other character after the serial's digits.
    SerialTextUnexpected = "temporal.rs", Malformed, UnexpectedCharacter;
    /// The whole days overflow 64 bits.
    SerialTextOverflow = "temporal.rs", OutOfRange, TooLarge;
    /// The 1900 system's phantom serial 60 (`1900-02-29`).
    SerialTextPhantom = "temporal.rs", OutOfRange, NonexistentDate;
    /// A serial below the system's first day, or past `9999-12-31`.
    SerialTextOutsideWindow = "temporal.rs", OutOfRange, OutsideWindow;
    /// The serial's instant is outside the timestamp window.
    SerialTextInstantOutsideWindow = "temporal.rs", OutOfRange, OutsideWindow,
        unreachable "every serial the day check accepts, with any fraction, is inside the window";

    // --- temporal.rs: cast_duration ---
    /// The trimmed input is empty.
    DurationBlank = "temporal.rs", Empty, Blank;
    /// Whole seconds past ±10,000 years.
    DurationOutsideWindow = "temporal.rs", OutOfRange, OutsideWindow;
    /// A second `T` in an ISO duration.
    IsoTRepeated = "temporal.rs", Malformed, MarkRepeated;
    /// A character where an ISO component's number must start.
    IsoDigitExpected = "temporal.rs", Malformed, DigitExpected;
    /// An ISO component's number with no unit; the span is the number.
    IsoUnitMissing = "temporal.rs", Malformed, ComponentMissing;
    /// An unknown or misplaced ISO unit (years, months before `T`, a time unit before `T`).
    IsoUnknownUnit = "temporal.rs", Malformed, UnknownUnit;
    /// A fraction on an ISO unit other than seconds; the span is the component.
    IsoFractionNotAllowed = "temporal.rs", Malformed, FractionNotAllowed;
    /// The ISO total overflows 128 bits of nanoseconds.
    IsoOverflow = "temporal.rs", OutOfRange, TooLarge,
        impractical "iso_overflow" "a component is at most 18 digits of weeks, so overflowing takes some 280,000 components, megabytes of input";
    /// An ISO duration with no component, or a `T` with no time component.
    IsoNoComponent = "temporal.rs", Malformed, ComponentMissing;
    /// The colon form's first field has no digit.
    ColonFirstDigitExpected = "temporal.rs", Malformed, DigitExpected;
    /// The hours after a day part have no digit.
    ColonHourDigitExpected = "temporal.rs", Malformed, DigitExpected;
    /// The hours after a day part have more than two digits.
    ColonHourWidth = "temporal.rs", Malformed, FieldWidth;
    /// The hours, with no day part, have more than two digits.
    ColonFirstWidth = "temporal.rs", Malformed, FieldWidth;
    /// The `:` after the hours is missing or different.
    ColonColon = "temporal.rs", Malformed, DelimiterExpected;
    /// The minutes have no digit.
    ColonMinuteDigitExpected = "temporal.rs", Malformed, DigitExpected;
    /// The minutes have more than two digits.
    ColonMinuteWidth = "temporal.rs", Malformed, FieldWidth;
    /// The seconds have no digit.
    ColonSecondDigitExpected = "temporal.rs", Malformed, DigitExpected;
    /// The seconds have more than two digits.
    ColonSecondWidth = "temporal.rs", Malformed, FieldWidth;
    /// More text after a complete colon duration.
    ColonTrailing = "temporal.rs", Malformed, TrailingText;
    /// Hours past 23, or minutes or seconds past 59.
    ColonFieldRange = "temporal.rs", OutOfRange, TimeFieldOutOfRange;
    /// A token that doesn't end in `s` (not a duration at all, or one missing its unit).
    ProtobufUnitMissing = "temporal.rs", Malformed, ComponentMissing;
    /// Only `s`, or a sign and `s`.
    ProtobufNoDigits = "temporal.rs", Malformed, NoDigits;
    /// Something other than a digit where the seconds must start.
    ProtobufDigitExpected = "temporal.rs", Malformed, DigitExpected;
    /// An `s` ends a complete duration and more text follows.
    ProtobufTrailing = "temporal.rs", Malformed, TrailingText;
    /// Any other character after the seconds.
    ProtobufUnexpected = "temporal.rs", Malformed, UnexpectedCharacter;
}
