//! The TypeScript binding: the hypercast core linked into a Node-API addon (napi-rs), the
//! same Rust for every native `.node` and for the wasm32-wasip1 fallback.
//!
//! Every door returns a verdict object, `{ ok: true, value }` or `{ ok: false, fault }`, and
//! never throws: not for bad text, and not for arguments of the wrong JavaScript type either.
//! Each argument crosses as an `Unknown` and is read here, because napi's own typed
//! conversions throw a `TypeError` before the door runs. A text that is `null` or `undefined`
//! is absent, the `empty` fault; anything else the types rule out (a text that is neither a
//! string nor a `Uint8Array`, a format that is not a `NumFormat`, an undeclared enum value, a
//! typed door's argument that is not a number) is a contract violation, reported as a
//! `malformed` fault with an empty span at offset 0. The formats are declared through
//! `NumFormat.declare` and `DateFormat.declare`, which answer `{ ok: false, message }` rather
//! than throw. What can still throw is the engine itself (out of memory building a result)
//! and a getter of the caller's own on a declaration object.
//!
//! A fault's span is in the caller's own units: UTF-16 code units for a string, which is what
//! `String.prototype.slice` takes, and bytes for a `Uint8Array`, the core's own.
//!
//! Every door, plain or checked, reads a string as WTF-8, so a lone surrogate reaches the core
//! as itself, malformed text at its own code unit, never as U+FFFD.
//!
//! The validation constraints (`Constraints`, `castChecked`, `castCheckedFromNumber`,
//! `castText`, `castConstraintsCheck`, `castDeclaredDecimal`) follow the same rules. Their
//! declarations answer `{ ok: false, message }` as the formats' do, and a checked door adds a
//! third verdict case, `{ ok: false, violation }`.
//!
//! A fault says why (`reason`) and where (`offset`, `length`). The four detailed doors
//! (`castDetailed`, `castDetailedFromNumber`, `castCheckedDetailed`,
//! `castCheckedDetailedFromNumber`) answer the same verdicts with a `DetailedFault`, which
//! adds what failed: a named `detail` and its raw `detailCode`. The plain doors keep their
//! 1.0 faults exactly, so no existing code sees a changed object. Both families read through
//! the core's detailed C exports, so a `NumFormat`'s grouping policy and a `DateOptions` are
//! honoured at every door that takes them.
//!
//! A `NumFormat` declares its grouping policy apart from its styles: Western unless the caller
//! says `Grouping.Indian`, and a door given no format reads `NumFormat.invariant()`, which is
//! Western too.

use hypercast as hc;
use napi::bindgen_prelude::{
    BigInt, ClassInstance, Either, Either3, JavaScriptClassExt, JsObjectValue, Object, ToNapiValue,
    Uint8Array, Unknown,
};
use napi::{Env, JsString, JsValue, Result, ValueType};
use napi_derive::napi;

/// The closed set of reasons a cast can fail.
#[napi(string_enum)]
pub enum CastFailure {
    /// The text was absent, empty or white space.
    #[napi(value = "empty")]
    Empty,
    /// The text was present but not recognizable as the target type.
    #[napi(value = "malformed")]
    Malformed,
    /// Well-formed, but outside the target's range: `"256"` for a u8, `1e400` for an f64.
    #[napi(value = "outOfRange")]
    OutOfRange,
}

/// The failure case of a cast: the reason, and the offending span in the caller's own units
/// (UTF-16 code units for a string, bytes for a `Uint8Array`), so
/// `text.slice(offset, offset + length)` is the offending text either way.
#[napi(object, object_from_js = false)]
pub struct Fault {
    /// Why the cast failed.
    pub reason: CastFailure,
    /// Where the offending span starts.
    pub offset: u32,
    /// How long it runs; 0 for `empty`, for a typed door and for a contract violation.
    pub length: u32,
}

/// The success case of a verdict: `ok` is `true` and `value` is the cast value.
#[napi(
    object,
    object_to_js = false,
    object_from_js = false,
    js_name = "Success<T>"
)]
pub struct SuccessCase {
    #[napi(ts_type = "true")]
    pub ok: bool,
    #[napi(ts_type = "T")]
    pub value: bool,
}

/// The failure case of a verdict: `ok` is `false` and `fault` says why.
#[napi(object, object_to_js = false, object_from_js = false)]
pub struct Failure {
    #[napi(ts_type = "false")]
    pub ok: bool,
    pub fault: Fault,
}

/// The outcome of a cast: exactly one of `Success<T>` or `Failure`, told apart by `ok`.
#[napi(js_name = "Verdict<T>")]
pub type VerdictType = Either<SuccessCase, Failure>;

/// What failed, the core's published detail vocabulary by name. The numbers never change and
/// codes may be added in a minor release, so a `switch` needs a `default` arm: a code this
/// binding does not name arrives as `'unknown'`, with its number in `detailCode`. `'none'` is
/// code 0, for a fault with no detail to give: a contract violation.
#[napi(string_enum)]
pub enum Detail {
    /// No detail: code 0.
    #[napi(value = "none")]
    None,
    /// Empty, or only white space.
    #[napi(value = "blank")]
    Blank,
    /// A character the position does not allow, where it allows several things.
    #[napi(value = "unexpectedCharacter")]
    UnexpectedCharacter,
    /// A numeral or digit field is under way, or a position needs a digit, and something
    /// else, or the end, is found.
    #[napi(value = "digitExpected")]
    DigitExpected,
    /// A required mark (`-`, `:`, `T`, `;`, a UUID hyphen) is missing or different.
    #[napi(value = "delimiterExpected")]
    DelimiterExpected,
    /// The value is complete and more text follows.
    #[napi(value = "trailingText")]
    TrailingText,
    /// The numeral is absent: only a sign, an affix, or a bare radix prefix.
    #[napi(value = "noDigits")]
    NoDigits,
    /// A sign where none may stand.
    #[napi(value = "signMisplaced")]
    SignMisplaced,
    /// A `(` without its `)`, or `( )`.
    #[napi(value = "parenthesesUnbalanced")]
    ParenthesesUnbalanced,
    /// A once-only mark appears twice.
    #[napi(value = "markRepeated")]
    MarkRepeated,
    /// A group separator not between digits, or an empty group.
    #[napi(value = "groupMisplaced")]
    GroupMisplaced,
    /// A group with the wrong number of digits for the grouping policy, or a first group that
    /// starts with a zero.
    #[napi(value = "groupSize")]
    GroupSize,
    /// Grouping after the decimal separator or in an exponent.
    #[napi(value = "groupInFraction")]
    GroupInFraction,
    /// Separator detection cannot tell `.` from `,`.
    #[napi(value = "ambiguousSeparator")]
    AmbiguousSeparator,
    /// A fraction where the type is whole.
    #[napi(value = "fractionNotAllowed")]
    FractionNotAllowed,
    /// A delimited field with the wrong number of digits.
    #[napi(value = "fieldWidth")]
    FieldWidth,
    /// A fixed-shape token, recognised by its length, with the wrong length.
    #[napi(value = "wrongLength")]
    WrongLength,
    /// A required part is absent: a zone, seconds, a duration unit.
    #[napi(value = "componentMissing")]
    ComponentMissing,
    /// Not one of the door's accepted words or values.
    #[napi(value = "notRecognized")]
    NotRecognized,
    /// An unknown or misplaced duration unit.
    #[napi(value = "unknownUnit")]
    UnknownUnit,
    /// Bytes that do not decode as UTF-8 where the door fails.
    #[napi(value = "invalidUtf8")]
    InvalidUtf8,
    /// Past an implementation limit: a 256-byte numeral, an 18-digit run.
    #[napi(value = "tooLong")]
    TooLong,
    /// A non-finite value: a textual NaN or infinity, or a typed one that cannot be held.
    #[napi(value = "notFinite")]
    NotFinite,
    /// A non-integral number at a whole-number door.
    #[napi(value = "notWhole")]
    NotWhole,
    /// A negative where the type has no negatives.
    #[napi(value = "negativeNotAllowed")]
    NegativeNotAllowed,
    /// The magnitude is more than the target type holds.
    #[napi(value = "tooLarge")]
    TooLarge,
    /// Below a signed type's minimum.
    #[napi(value = "tooSmall")]
    TooSmall,
    /// More fractional digits than the type holds.
    #[napi(value = "tooPrecise")]
    TooPrecise,
    /// Year 0, month 13, a day past the month's end.
    #[napi(value = "dateFieldOutOfRange")]
    DateFieldOutOfRange,
    /// An hour, minute, second or offset out of range.
    #[napi(value = "timeFieldOutOfRange")]
    TimeFieldOutOfRange,
    /// A representable instant or duration past the supported window.
    #[napi(value = "outsideWindow")]
    OutsideWindow,
    /// A day that never existed: Excel's phantom 1900-02-29.
    #[napi(value = "nonexistentDate")]
    NonexistentDate,
    /// A code point above U+10FFFF, or a surrogate.
    #[napi(value = "notAScalar")]
    NotAScalar,
    /// A code this binding does not name yet; the number is in `detailCode`.
    #[napi(value = "unknown")]
    Unknown,
}

/// The failure case of a detailed door: `reason`, `offset` and `length` as a plain `Fault`
/// carries them, and what failed. `detailCode` is the core's own number, `detail` its name.
#[napi(object, object_from_js = false)]
pub struct DetailedFault {
    /// Why the cast failed.
    pub reason: CastFailure,
    /// Where the offending span starts.
    pub offset: u32,
    /// How long it runs; 0 for `empty`, for a typed door and for a contract violation.
    pub length: u32,
    /// What failed, by name. Open, like `Violation.keyword`: a code this binding does not name
    /// yet is `'unknown'`, and a later release may name more, so a `switch` needs a `default`.
    #[napi(ts_type = "Detail | (string & {})")]
    pub detail: Detail,
    /// What failed, by the core's number: 1 to 32 are published; 0 is no detail.
    pub detail_code: u32,
}

/// The failure case of a detailed verdict: `ok` is `false` and `fault` says why and what.
#[napi(object, object_to_js = false, object_from_js = false)]
pub struct DetailedFailure {
    #[napi(ts_type = "false")]
    pub ok: bool,
    pub fault: DetailedFault,
}

/// The outcome of a detailed cast: exactly one of `Success<T>` or `DetailedFailure`, told
/// apart by `ok`.
#[napi(js_name = "DetailedVerdict<T>")]
pub type DetailedVerdictType = Either<SuccessCase, DetailedFailure>;

/// A declaration the binding refused: `ok` is `false` and `message` names the rule broken.
#[napi(object, object_to_js = false, object_from_js = false)]
pub struct Refusal {
    #[napi(ts_type = "false")]
    pub ok: bool,
    pub message: String,
}

/// The outcome of declaring a format: the format, or the reason it cannot be one.
#[napi(js_name = "Declaration<T>")]
pub type DeclarationType = Either<SuccessCase, Refusal>;

/// What a door reads: a string (as UTF-8, a lone surrogate kept as malformed text, never
/// U+FFFD), the bytes of a `Uint8Array` (a `Buffer` is one) as they are, or `null` or
/// `undefined` for absent input.
#[napi(js_name = "Text")]
pub type TextType = Option<Either<String, Uint8Array>>;

/// An instant: seconds since 1970-01-01T00:00:00Z and the nanoseconds after them, always
/// `0..=999_999_999` (protobuf's `Timestamp`).
#[napi(object, object_from_js = false)]
pub struct Timestamp {
    pub seconds: i64,
    pub nanos: i32,
}

/// A calendar date with no time and no zone.
#[napi(object, object_from_js = false)]
pub struct CalendarDate {
    pub year: u32,
    pub month: u32,
    pub day: u32,
}

/// A wall-clock time of day with no date and no zone; `nanos` is `0..=999_999_999`.
#[napi(object, object_from_js = false)]
pub struct TimeOfDay {
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub nanos: u32,
}

/// A civil date and time with no zone, as zone-less text names one.
#[napi(object, object_from_js = false)]
pub struct CivilDateTime {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub nanos: u32,
}

/// A signed span: whole seconds, and nanoseconds with the same sign (protobuf's `Duration`).
#[napi(object, object_from_js = false)]
pub struct Duration {
    pub seconds: i64,
    pub nanos: i32,
}

/// The declared unit of a Unix-epoch value.
#[napi]
pub enum UnixPrecision {
    Seconds = 1,
    Milliseconds = 2,
    Microseconds = 3,
    Nanoseconds = 4,
}

/// The declared field order of a separated calendar date: `1/7/2026` is January 7th or
/// July 1st only because the caller said which.
#[napi]
pub enum DateOrder {
    YearMonthDay = 1,
    MonthDayYear = 2,
    DayMonthYear = 3,
}

/// The date system an Excel serial is expressed in, a workbook setting the cell never carries.
#[napi]
pub enum ExcelEpoch {
    /// The Windows default: serial 1 is 1900-01-01, and serial 60 is a February 29th that
    /// never existed.
    Y1900 = 1,
    /// The legacy Macintosh system: serial 0 is 1904-01-01.
    Y1904 = 2,
}

/// The leniences a `NumFormat` permits, combined with `|`.
#[napi]
pub enum NumStyles {
    None = 0,
    /// The group separator between digits.
    Grouping = 1,
    /// Accounting parentheses as negation: `(1,234)` is -1234.
    Parentheses = 2,
    /// Exponent notation: `1e3`, `2.5E-3`.
    Exponent = 4,
    /// The radix prefixes `0x`, `&H` and `0b`, read as the bit pattern. Integer doors only.
    RadixPrefixes = 8,
    /// A trailing `%` (or the declared percent symbol), dividing by 100. Real doors only.
    Percent = 16,
    /// The `.`/`,` roles resolved per input from structure; the declared separators are
    /// ignored, and an undecidable separator is `malformed`, never guessed.
    SeparatorDetect = 32,
    /// The declared currency symbol at either edge of the number.
    Currency = 64,
    /// The sign after the digits (`5-`), as a culture whose negative pattern is `n-` writes it.
    TrailingSign = 128,
    /// Every lenience but `SeparatorDetect` and `TrailingSign`.
    All = 95,
}

/// How strictly a `NumFormat` reads groups of digits, the core's grouping policy. It is declared
/// apart from the styles, and replaces a policy rather than adding to one. It only matters
/// where `NumStyles.Grouping` is on: with grouping off, a group separator stays forbidden
/// under every policy.
#[napi]
pub enum Grouping {
    /// Groups of exactly three digits counted from the decimal separator, the leftmost of one
    /// to three and not starting with a zero (`1,234,567`; `12,34` and `0,125` are `malformed`).
    /// The default.
    Western = 0,
    /// A group of three beside the decimal separator and twos beyond it, the leftmost of one
    /// or two (`12,34,567`; `1,234,567` is `malformed`).
    Indian = 512,
}

/// The declared form of a `NumFormat`: `NumFormat.declare` reads it.
#[napi(object, object_to_js = false, object_from_js = false)]
pub struct NumFormatSpec {
    /// The decimal separator: 1 to 8 UTF-8 bytes with no ASCII digit.
    pub decimal_sep: String,
    /// The group separator, likewise, and different from `decimalSep`.
    pub group_sep: String,
    /// The leniences, `NumStyles` members combined with `|`. A style mask holds no grouping
    /// policy: declare that as `grouping`.
    pub flags: u32,
    /// How groups of digits are read; `Grouping.Western` when absent.
    #[napi(ts_type = "Grouping")]
    pub grouping: Option<u32>,
    /// The currency symbol, 1 to 16 UTF-8 bytes with no ASCII digit and no white space at either end.
    pub currency: Option<String>,
    /// The decimal separator of a currency amount; absent keeps `decimalSep`.
    pub currency_decimal_sep: Option<String>,
    /// The group separator of a currency amount; absent keeps `groupSep`.
    pub currency_group_sep: Option<String>,
    /// A further spelling of the minus sign, read beside ASCII `-`.
    pub minus_sign: Option<String>,
    /// A further spelling of the plus sign, read beside ASCII `+`.
    pub plus_sign: Option<String>,
    /// A further spelling of the percent symbol, read beside ASCII `%`.
    pub percent: Option<String>,
    /// The zero of native digits read beside ASCII `0`-`9`: one non-ASCII character that
    /// starts a Unicode digit block (`'٠'`, `'０'`).
    pub digit_zero: Option<String>,
}

/// The declared form of a `DateFormat`: `DateFormat.declare` reads it.
#[napi(object, object_to_js = false, object_from_js = false)]
pub struct DateFormatSpec {
    /// The field order.
    pub order: DateOrder,
    /// The designator of the first half of the day, read beside `AM`.
    pub am: Option<String>,
    /// The designator of the second half of the day, read beside `PM`.
    pub pm: Option<String>,
}

/// The extra shapes a `DateOptions` opens, combined with `|`. Each is read at one door, and a
/// style a door does not read is ignored there.
#[napi]
pub enum DateStyles {
    None = 0,
    /// `castDate` also reads `20260107`, eight ASCII digits as `yyyyMMdd`.
    CompactDate = 1,
    /// `castTimestamp` also reads one space where the `T` goes.
    SpaceSeparator = 2,
    /// `castTimestamp` also reads an offset of `±hh` (`+00`, `-05`).
    HourOffset = 4,
    /// `CompactDate`, `SpaceSeparator` and `HourOffset` together. It never supplies a pivot
    /// or month names.
    CommonForms = 7,
}

/// The declared form of a `DateOptions`: `DateOptions.declare` reads it.
#[napi(object, object_to_js = false, object_from_js = false)]
pub struct DateOptionsSpec {
    /// The extra shapes, `DateStyles` members combined with `|`; none when absent.
    #[napi(ts_type = "DateStyles")]
    pub flags: Option<u32>,
    /// The last year of the 100-year window a two-digit year is read in: `0` or absent for
    /// none (a two-digit year is then a `fieldWidth` fault), otherwise 100 to 9999. A year
    /// `yy` reads as the one year from `pivot - 99` to `pivot` that ends in `yy`.
    pub pivot: Option<u32>,
    /// Month names read in the month field of `castDate` with an order and of `castDatetime`.
    #[napi(ts_type = "MonthNames")]
    pub months: Option<bool>,
}

// ---------------------------------------------------------------------------------------
// Reading arguments. Nothing here throws: every napi failure is a value the door reports.
// ---------------------------------------------------------------------------------------

/// A door's text, as the caller passed it.
enum Input {
    /// A string, as WTF-8: its UTF-8, with a lone surrogate the three bytes no UTF-8 decoder
    /// accepts (three bytes for its one code unit, so spans still map back exactly).
    Str(Vec<u8>),
    /// The bytes of a `Uint8Array`, read as they are.
    Bytes(Uint8Array),
    /// `null` or `undefined`.
    Absent,
    /// Anything else.
    Invalid,
}

impl Input {
    /// napi's UTF-8 read is the fast path; a string it may not have read exactly (see
    /// [`exact_utf8`]) is read again, from its UTF-16, as [`string_wtf8`], so a door never
    /// judges a replacement for a lone surrogate.
    fn read(env: &Env, value: &Unknown<'_>) -> Input {
        match value.get_type() {
            Ok(ValueType::String) => match unsafe { value.cast::<String>() } {
                Ok(text) if exact_utf8(&text) => Input::Str(text.into_bytes()),
                Ok(_) => string_wtf8(env, value).map_or(Input::Invalid, Input::Str),
                Err(_) => Input::Invalid,
            },
            Ok(ValueType::Undefined | ValueType::Null) => Input::Absent,
            // `cast` checks the typed array's kind and fails for every other one.
            Ok(ValueType::Object) => match unsafe { value.cast::<Uint8Array>() } {
                Ok(bytes) => Input::Bytes(bytes),
                Err(_) => Input::Invalid,
            },
            _ => Input::Invalid,
        }
    }

    /// The fault in the caller's units: bytes as they are for a `Uint8Array`, UTF-16 code
    /// units for a string (an ASCII string needs no mapping, and only a fault pays for it).
    fn present(&self, fault: hc::Fault) -> hc::Fault {
        let Input::Str(bytes) = self else {
            return fault;
        };
        if bytes.is_ascii() {
            return fault;
        }
        let offset = (fault.offset as usize).min(bytes.len());
        let end = offset.saturating_add(fault.len as usize).min(bytes.len());
        hc::Fault {
            reason: fault.reason,
            offset: utf16_units(&bytes[..offset]),
            len: utf16_units(&bytes[offset..end]),
        }
    }
}

/// Whether napi's UTF-8 read of a string is the string exactly: whether it held no lone
/// surrogate. A native addon's runtime (Node, Bun) writes U+FFFD for one, so UTF-8 with no
/// U+FFFD in it is exact, and only a string with a U+FFFD (a real one, or a replacement) is
/// read again. The WebAssembly fallback's runtime encodes with Emscripten's `stringToUTF8`,
/// which fuses a lone surrogate with the code unit after it into another character, so there
/// only an ASCII string is known to be exact. `tests/constraints.test.ts` pins both: a lone
/// surrogate at a plain door is malformed at its own code unit.
#[cfg(not(target_family = "wasm"))]
fn exact_utf8(text: &str) -> bool {
    text.is_ascii() || !text.contains('\u{FFFD}')
}

/// See the native definition: under the WebAssembly fallback only ASCII is known to be exact.
#[cfg(target_family = "wasm")]
fn exact_utf8(text: &str) -> bool {
    text.is_ascii()
}

/// UTF-16 code units in well-formed UTF-8 or WTF-8: one per lead byte (a lone surrogate's
/// three bytes are its one unit), two for a four-byte sequence.
fn utf16_units(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .map(|&byte| match byte {
            0x80..=0xBF => 0,
            0xF0..=0xFF => 2,
            _ => 1,
        })
        .sum()
}

/// A JavaScript number that is a whole `u32`, the way an enum member crosses.
fn code(value: &Unknown<'_>) -> Option<u32> {
    let number = number(value)?;
    (number.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&number))
        .then_some(number as u32)
}

/// A JavaScript number, as a typed door reads it.
fn number(value: &Unknown<'_>) -> Option<f64> {
    match value.get_type() {
        Ok(ValueType::Number) => unsafe { value.cast::<f64>() }.ok(),
        _ => None,
    }
}

/// A `format` argument: absent is `NumFormat.invariant()`, the invariant separators under
/// Western grouping, as a null format at the C ABI is.
fn num_format(value: &Unknown<'_>) -> Option<hc::NumFormat> {
    match value.get_type() {
        Ok(ValueType::Undefined | ValueType::Null) => Some(hc::NumFormat::INVARIANT),
        Ok(ValueType::Object) => unsafe { value.cast::<ClassInstance<NumFormat>>() }
            .ok()
            .map(|format| format.resolved),
        _ => None,
    }
}

const CONTRACT: hc::Fault = hc::Fault {
    reason: hc::Reason::Malformed,
    offset: 0,
    len: 0,
};
const ABSENT: hc::Fault = hc::Fault {
    reason: hc::Reason::Empty,
    offset: 0,
    len: 0,
};

// ---------------------------------------------------------------------------------------
// Building verdicts.
// ---------------------------------------------------------------------------------------

fn success<'env, V: ToNapiValue>(env: &'env Env, value: V) -> Result<Object<'env>> {
    let mut verdict = Object::new(env)?;
    verdict.set("ok", true)?;
    verdict.set("value", value)?;
    Ok(verdict)
}

fn failure(env: &Env, fault: hc::Fault) -> Result<Object<'_>> {
    let mut verdict = Object::new(env)?;
    verdict.set("ok", false)?;
    verdict.set(
        "fault",
        Fault {
            reason: cast_failure(fault.reason),
            offset: fault.offset,
            length: fault.len,
        },
    )?;
    Ok(verdict)
}

fn cast_failure(reason: hc::Reason) -> CastFailure {
    match reason {
        hc::Reason::Empty => CastFailure::Empty,
        hc::Reason::Malformed => CastFailure::Malformed,
        hc::Reason::OutOfRange => CastFailure::OutOfRange,
    }
}

/// The name of a detail code, `Detail::None` for 0 (no detail) and `Detail::Unknown` for a
/// code the core publishes after this binding was written, or one it never did. Never fails:
/// a failure with an unknown detail still has its reason and span.
fn detail_of(code: u32) -> Detail {
    let Some(published) = hc::Detail::from_code(code) else {
        return if code == 0 {
            Detail::None
        } else {
            Detail::Unknown
        };
    };
    match published {
        hc::Detail::Blank => Detail::Blank,
        hc::Detail::UnexpectedCharacter => Detail::UnexpectedCharacter,
        hc::Detail::DigitExpected => Detail::DigitExpected,
        hc::Detail::DelimiterExpected => Detail::DelimiterExpected,
        hc::Detail::TrailingText => Detail::TrailingText,
        hc::Detail::NoDigits => Detail::NoDigits,
        hc::Detail::SignMisplaced => Detail::SignMisplaced,
        hc::Detail::ParenthesesUnbalanced => Detail::ParenthesesUnbalanced,
        hc::Detail::MarkRepeated => Detail::MarkRepeated,
        hc::Detail::GroupMisplaced => Detail::GroupMisplaced,
        hc::Detail::GroupSize => Detail::GroupSize,
        hc::Detail::GroupInFraction => Detail::GroupInFraction,
        hc::Detail::AmbiguousSeparator => Detail::AmbiguousSeparator,
        hc::Detail::FractionNotAllowed => Detail::FractionNotAllowed,
        hc::Detail::FieldWidth => Detail::FieldWidth,
        hc::Detail::WrongLength => Detail::WrongLength,
        hc::Detail::ComponentMissing => Detail::ComponentMissing,
        hc::Detail::NotRecognized => Detail::NotRecognized,
        hc::Detail::UnknownUnit => Detail::UnknownUnit,
        hc::Detail::InvalidUtf8 => Detail::InvalidUtf8,
        hc::Detail::TooLong => Detail::TooLong,
        hc::Detail::NotFinite => Detail::NotFinite,
        hc::Detail::NotWhole => Detail::NotWhole,
        hc::Detail::NegativeNotAllowed => Detail::NegativeNotAllowed,
        hc::Detail::TooLarge => Detail::TooLarge,
        hc::Detail::TooSmall => Detail::TooSmall,
        hc::Detail::TooPrecise => Detail::TooPrecise,
        hc::Detail::DateFieldOutOfRange => Detail::DateFieldOutOfRange,
        hc::Detail::TimeFieldOutOfRange => Detail::TimeFieldOutOfRange,
        hc::Detail::OutsideWindow => Detail::OutsideWindow,
        hc::Detail::NonexistentDate => Detail::NonexistentDate,
        hc::Detail::NotAScalar => Detail::NotAScalar,
        // `hc::Detail` is non-exhaustive: a code a later core adds is named when this binding
        // learns it, and `unknown` until then.
        _ => Detail::Unknown,
    }
}

/// The reason a detailed export's return code 1, 2 or 3 names.
fn reason_of(code: i32) -> Option<hc::Reason> {
    match code {
        1 => Some(hc::Reason::Empty),
        2 => Some(hc::Reason::Malformed),
        3 => Some(hc::Reason::OutOfRange),
        _ => None,
    }
}

/// A detailed fault: a plain one's reason and span, plus what failed by name and by number. A
/// code this binding does not name is `Detail::Unknown` with its number kept; the reason and
/// span are as usable as ever.
fn detailed_fault(reason: hc::Reason, (offset, length): (u32, u32), detail: u32) -> DetailedFault {
    DetailedFault {
        reason: cast_failure(reason),
        offset,
        length,
        detail: detail_of(detail),
        detail_code: detail,
    }
}

/// A detailed failure's verdict: the fault as a plain one is, plus what failed.
fn detailed_failure(
    env: &Env,
    reason: hc::Reason,
    span: (u32, u32),
    detail: u32,
) -> Result<Object<'_>> {
    let mut verdict = Object::new(env)?;
    verdict.set("ok", false)?;
    verdict.set("fault", detailed_fault(reason, span, detail))?;
    Ok(verdict)
}

/// A failure's verdict in the family the door belongs to: with its detail at a detailed door,
/// the plain fault at the others.
fn fault_verdict(
    env: &Env,
    detailed: bool,
    reason: hc::Reason,
    span: (u32, u32),
    detail: u32,
) -> Result<Object<'_>> {
    if detailed {
        detailed_failure(env, reason, span, detail)
    } else {
        failure(
            env,
            hc::Fault {
                reason,
                offset: span.0,
                len: span.1,
            },
        )
    }
}

/// A contract violation's verdict in the door's family. A detailed one has no detail to give:
/// code 0, `'none'`.
fn contract_verdict(env: &Env, detailed: bool) -> Result<Object<'_>> {
    clear_pending_exception(env);
    fault_verdict(env, detailed, hc::Reason::Malformed, (0, 0), 0)
}

/// Absent input's verdict: the `empty` fault, whose detail is `blank`.
fn absent_verdict(env: &Env, detailed: bool) -> Result<Object<'_>> {
    fault_verdict(
        env,
        detailed,
        hc::Reason::Empty,
        (0, 0),
        hc::Detail::Blank.code(),
    )
}

/// A contract violation's verdict. Reading an argument of the wrong kind runs no JavaScript
/// natively, but emnapi (the WASI build's Node-API) can: its typed-array and unwrap checks
/// reach a Proxy's traps, and a trap that throws leaves its exception pending. It is cleared
/// here, so the door answers the verdict rather than rethrowing it.
fn contract(env: &Env) -> Result<Object<'_>> {
    clear_pending_exception(env);
    failure(env, CONTRACT)
}

/// A text door: reads the text, runs the cast, and presents the outcome. `declared` is the
/// door's other arguments, already read: `None` when one broke the contract.
fn door<'env, D, T, V: ToNapiValue>(
    env: &'env Env,
    text: &Unknown<'_>,
    declared: Option<D>,
    cast: impl FnOnce(&[u8], D) -> std::result::Result<T, hc::Fault>,
    value: impl FnOnce(T) -> V,
) -> Result<Object<'env>> {
    let Some(declared) = declared else {
        return contract(env);
    };
    let input = Input::read(env, text);
    let outcome = match &input {
        Input::Str(text) => cast(text, declared),
        Input::Bytes(bytes) => cast(bytes, declared),
        Input::Absent => return failure(env, ABSENT),
        Input::Invalid => return contract(env),
    };
    match outcome {
        Ok(cast) => success(env, value(cast)),
        Err(fault) => failure(env, input.present(fault)),
    }
}

/// A typed door: a JavaScript number in, a fault with no span out (there is no text).
fn typed<'env, D, T, V: ToNapiValue>(
    env: &'env Env,
    number_arg: &Unknown<'_>,
    declared: Option<D>,
    cast: impl FnOnce(f64, D) -> std::result::Result<T, hc::Reason>,
    value: impl FnOnce(T) -> V,
) -> Result<Object<'env>> {
    let (Some(declared), Some(number)) = (declared, number(number_arg)) else {
        return contract(env);
    };
    match cast(number, declared) {
        Ok(cast) => success(env, value(cast)),
        Err(reason) => failure(
            env,
            hc::Fault {
                reason,
                offset: 0,
                len: 0,
            },
        ),
    }
}

// ---------------------------------------------------------------------------------------
// Values.
// ---------------------------------------------------------------------------------------

fn uuid_text(bytes: [u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0xf)]));
    }
    out
}

fn timestamp(value: hc::Timestamp) -> Timestamp {
    Timestamp {
        seconds: value.seconds,
        nanos: value.nanos,
    }
}

fn calendar(date: hc::Date) -> CalendarDate {
    CalendarDate {
        year: u32::from(date.year),
        month: u32::from(date.month),
        day: u32::from(date.day),
    }
}

fn time_of_day(nanos_of_day: u64) -> TimeOfDay {
    let seconds = nanos_of_day / 1_000_000_000;
    TimeOfDay {
        hour: (seconds / 3600) as u32,
        minute: (seconds / 60 % 60) as u32,
        second: (seconds % 60) as u32,
        nanos: (nanos_of_day % 1_000_000_000) as u32,
    }
}

fn civil(value: hc::CivilDateTime) -> CivilDateTime {
    let time = time_of_day(value.nanos_of_day);
    CivilDateTime {
        year: u32::from(value.date.year),
        month: u32::from(value.date.month),
        day: u32::from(value.date.day),
        hour: time.hour,
        minute: time.minute,
        second: time.second,
        nanos: time.nanos,
    }
}

fn duration(value: hc::Duration) -> Duration {
    Duration {
        seconds: value.seconds,
        nanos: value.nanos,
    }
}

// ---------------------------------------------------------------------------------------
// Declarations.
// ---------------------------------------------------------------------------------------

/// Caller-declared numeric notation, held resolved as the core's own format. Made by
/// `NumFormat.declare`, `NumFormat.invariant` or `NumFormat.detect`, never by `new`.
#[napi]
pub struct NumFormat {
    resolved: hc::NumFormat,
}

/// A declaration's field, read the one way a declaration can run JavaScript: a getter or a
/// Proxy trap of the caller's that throws. Its exception is cleared, so the declaration
/// answers a refusal rather than rethrowing it.
fn spec_field<'env>(
    env: &Env,
    spec: &Object<'env>,
    name: &str,
) -> std::result::Result<Option<Unknown<'env>>, String> {
    spec.get(name).map_err(|_| {
        clear_pending_exception(env);
        format!("{name} could not be read")
    })
}

fn clear_pending_exception(env: &Env) {
    let mut pending = false;
    // SAFETY: `env` is the live environment of the call in progress.
    unsafe {
        if napi::sys::napi_is_exception_pending(env.raw(), &mut pending)
            == napi::sys::Status::napi_ok
            && pending
        {
            let mut exception = std::ptr::null_mut();
            napi::sys::napi_get_and_clear_last_exception(env.raw(), &mut exception);
        }
    }
}

/// A declaration's text field: absent (`undefined`) declares nothing. A string napi's UTF-8
/// read may not have read exactly (see [`exact_utf8`]) is read again from its UTF-16, so a
/// lone surrogate is refused, never declared as the replacement character napi wrote (or, under
/// the WebAssembly fallback, the character it fused).
fn spec_text(
    env: &Env,
    spec: &Object<'_>,
    name: &str,
) -> std::result::Result<Option<String>, String> {
    let value = spec_field(env, spec, name)?;
    let Some(value) = value else {
        return Ok(None);
    };
    match value.get_type() {
        Ok(ValueType::Undefined) => Ok(None),
        Ok(ValueType::String) => match unsafe { value.cast::<String>() } {
            Ok(text) if exact_utf8(&text) => Ok(Some(text)),
            Ok(_) => {
                let bytes =
                    string_wtf8(env, &value).ok_or_else(|| format!("{name} could not be read"))?;
                String::from_utf8(bytes)
                    .map(Some)
                    .map_err(|_| format!("{name} must be whole code points, not surrogate halves"))
            }
            Err(_) => Err(format!("{name} could not be read")),
        },
        _ => Err(format!("{name} must be a string")),
    }
}

/// The declaration object itself, or the refusal for anything that is not one.
fn spec_object<'env>(
    spec: &Unknown<'env>,
    what: &str,
) -> std::result::Result<Object<'env>, String> {
    match spec.get_type() {
        Ok(ValueType::Object) => {
            unsafe { spec.cast::<Object>() }.map_err(|_| format!("{what} must be an object"))
        }
        _ => Err(format!("{what} must be an object")),
    }
}

fn declared<'env, T: ToNapiValue>(
    env: &'env Env,
    outcome: std::result::Result<T, String>,
) -> Result<Object<'env>> {
    let mut declaration = Object::new(env)?;
    match outcome {
        Ok(value) => {
            declaration.set("ok", true)?;
            declaration.set("value", value)?;
        }
        Err(message) => {
            declaration.set("ok", false)?;
            declaration.set("message", message)?;
        }
    }
    Ok(declaration)
}

/// The `flags` bits a style mask may hold: the eight `NumStyles` members' (bits 8 and 9 are the
/// grouping policy, which `grouping` declares).
const STYLE_BITS: u32 = 0xFF;

/// A number separator: 1 to 8 UTF-8 bytes with no ASCII digit, the core's own rule.
fn separator(what: &str, text: &str) -> std::result::Result<hc::Symbol<8>, String> {
    hc::Symbol::new(text).ok_or_else(|| {
        format!("{what} must be 1 to 8 UTF-8 bytes with no ASCII digit; got \"{text}\"")
    })
}

fn has_white_space(text: &str) -> bool {
    text.chars().any(hc::is_white_space)
}

/// The refusal of a symbol that holds white space, which no input can match: white space
/// around a symbol is ignored, and a numeral admits it only as a group separator.
fn white_space_refusal(what: &str, text: &str) -> String {
    format!("{what} must not contain white space; got \"{text}\"")
}

/// A symbol a caller need not declare: absent or `""` declares none; with `flush` (a sign,
/// the percent symbol) it may hold no white space either, checked after the length and digit.
fn optional_symbol(
    what: &str,
    text: Option<&str>,
    flush: bool,
) -> std::result::Result<hc::Symbol<8>, String> {
    let Some(text) = text.filter(|text| !text.is_empty()) else {
        return Ok(hc::Symbol::NONE);
    };
    let symbol = separator(what, text)?;
    if flush && has_white_space(text) {
        return Err(white_space_refusal(what, text));
    }
    Ok(symbol)
}

/// The backstop behind the per-field validators, in every build: the core refuses the
/// resolved format, so a validator above lags a rule the core has.
fn refuse_unless_checked(resolved: &hc::NumFormat) -> std::result::Result<(), String> {
    if resolved.check() {
        Ok(())
    } else {
        Err("The NumFormat declaration is a format the core refuses".to_string())
    }
}

fn declare_num_format(env: &Env, spec: &Object<'_>) -> std::result::Result<hc::NumFormat, String> {
    let required = |name: &str| -> std::result::Result<String, String> {
        spec_text(env, spec, name)?.ok_or_else(|| format!("{name} is required"))
    };
    let decimal_text = required("decimalSep")?;
    let group_text = required("groupSep")?;
    let flags = spec_field(env, spec, "flags")?;
    let flags = flags
        .as_ref()
        .and_then(code)
        .ok_or_else(|| "flags must be a whole number from 0 to 4294967295".to_string())?;
    if flags & hc::NumFormat::GROUPING_POLICY != 0 {
        return Err(format!(
            "flags must not carry a grouping policy; declare it as grouping; got {flags}"
        ));
    }
    if flags & !STYLE_BITS != 0 {
        return Err(format!(
            "flags holds a bit no NumStyles member names; got {flags}"
        ));
    }
    let grouping = match spec_field(env, spec, "grouping")? {
        Some(value) if !matches!(value.get_type(), Ok(ValueType::Undefined)) => code(&value)
            .and_then(hc::Grouping::from_code)
            .ok_or_else(|| "grouping must be a Grouping".to_string())?,
        _ => hc::Grouping::Western,
    };
    let currency = spec_text(env, spec, "currency")?;
    let currency_decimal = spec_text(env, spec, "currencyDecimalSep")?;
    let currency_group = spec_text(env, spec, "currencyGroupSep")?;
    let minus = spec_text(env, spec, "minusSign")?;
    let plus = spec_text(env, spec, "plusSign")?;
    let percent = spec_text(env, spec, "percent")?;
    let digit_zero = spec_text(env, spec, "digitZero")?;

    let decimal_sep = separator("Decimal separator", &decimal_text)?;
    let group_sep = separator("Group separator", &group_text)?;
    if decimal_sep == group_sep {
        return Err(format!(
            "Decimal and group separators must differ; both are \"{decimal_text}\""
        ));
    }
    if has_white_space(&decimal_text) {
        return Err(white_space_refusal("Decimal separator", &decimal_text));
    }
    let currency_decimal_sep = optional_symbol(
        "Currency decimal separator",
        currency_decimal.as_deref(),
        false,
    )?;
    let currency_group_sep =
        optional_symbol("Currency group separator", currency_group.as_deref(), false)?;
    let in_force =
        |own: hc::Symbol<8>, number: hc::Symbol<8>| if own.is_empty() { number } else { own };
    let (decimal, group) = (
        in_force(currency_decimal_sep, decimal_sep),
        in_force(currency_group_sep, group_sep),
    );
    if decimal == group {
        return Err(format!(
            "Currency decimal and group separators must differ; both are \"{decimal}\""
        ));
    }
    if let Some(text) = currency_decimal.as_deref()
        && has_white_space(text)
    {
        return Err(white_space_refusal("Currency decimal separator", text));
    }
    let currency = match currency.as_deref().filter(|text| !text.is_empty()) {
        None => hc::CurrencySymbol::NONE,
        Some(text) => {
            let refusal = || {
                format!(
                    "Currency symbol must be 1 to 16 UTF-8 bytes with no ASCII digit; got \"{text}\""
                )
            };
            if !(1..=hc::CurrencySymbol::MAX_BYTES).contains(&text.len())
                || text.bytes().any(|byte| byte.is_ascii_digit())
            {
                return Err(refusal());
            }
            if text.starts_with(hc::is_white_space) || text.ends_with(hc::is_white_space) {
                return Err(format!(
                    "Currency symbol must not start or end with white space; got \"{text}\""
                ));
            }
            hc::CurrencySymbol::new(text).ok_or_else(refusal)?
        }
    };
    let digits = match digit_zero.as_deref().filter(|text| !text.is_empty()) {
        None => hc::NativeDigits::NONE,
        Some(text) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(zero), None) => hc::NativeDigits::new(zero),
                _ => None,
            }
            .ok_or_else(|| {
                format!(
                    "Digit zero must be the one non-ASCII character a Unicode digit block starts at; got \"{text}\""
                )
            })?
        }
    };
    let resolved = hc::NumFormat::with_separators(decimal_sep, group_sep, flags)
        .with_currency(currency)
        .with_currency_separators(currency_decimal_sep, currency_group_sep)
        .with_signs(
            optional_symbol("Minus sign", minus.as_deref(), true)?,
            optional_symbol("Plus sign", plus.as_deref(), true)?,
        )
        .with_percent(optional_symbol("Percent symbol", percent.as_deref(), true)?)
        .with_digits(digits)
        .with_grouping(grouping);
    refuse_unless_checked(&resolved)?;
    Ok(resolved)
}

#[napi]
impl NumFormat {
    /// Declares a format, or says which rule the declaration breaks; never throws. The
    /// separators are each 1 to 8 UTF-8 bytes with no ASCII digit, and differ (so do the
    /// pair a currency amount is read with), and a decimal separator holds no white space; a
    /// sign or percent symbol holds no white space; the currency symbol is at most 16 bytes
    /// with no ASCII digit and no white space at either end (`F CFA` is fine); the digit zero is the one non-ASCII character
    /// a Unicode digit block starts at.
    #[napi(
        catch_unwind,
        ts_args_type = "spec: NumFormatSpec",
        ts_return_type = "Declaration<NumFormat>"
    )]
    pub fn declare<'env>(env: &'env Env, spec: Unknown<'env>) -> Result<Object<'env>> {
        let outcome = spec_object(&spec, "The NumFormat declaration")
            .and_then(|spec| declare_num_format(env, &spec));
        match outcome {
            Ok(resolved) => declared(env, Ok(NumFormat { resolved }.into_instance(env)?)),
            Err(message) => declared::<bool>(env, Err(message)),
        }
    }

    /// The invariant format: `.` decimal, `,` grouping, `NumStyles.All`, Western
    /// grouping (`Grouping.Western`). What a door reads with when its format is left out.
    #[napi(factory, catch_unwind)]
    pub fn invariant() -> Self {
        NumFormat {
            resolved: hc::NumFormat::INVARIANT,
        }
    }

    /// The detection format: `NumStyles.All | NumStyles.SeparatorDetect`, the `.`/`,` roles
    /// resolved per input from structure, under Western grouping. A grouping policy
    /// never resolves an ambiguous separator.
    #[napi(factory, catch_unwind)]
    pub fn detect() -> Self {
        NumFormat {
            resolved: hc::NumFormat::DETECT,
        }
    }

    /// The same format under another policy, which replaces the old one. A value that is not a
    /// `Grouping` is refused, by the same rule `declare` holds; it never throws.
    #[napi(
        catch_unwind,
        ts_args_type = "grouping: Grouping",
        ts_return_type = "Declaration<NumFormat>"
    )]
    pub fn with_grouping<'env>(
        &self,
        env: &'env Env,
        grouping: Unknown<'env>,
    ) -> Result<Object<'env>> {
        match code(&grouping).and_then(hc::Grouping::from_code) {
            Some(policy) => {
                let resolved = self.resolved.with_grouping(policy);
                declared(env, Ok(NumFormat { resolved }.into_instance(env)?))
            }
            None => declared::<bool>(env, Err("grouping must be a Grouping".to_string())),
        }
    }

    /// The decimal separator.
    #[napi(getter, catch_unwind)]
    pub fn decimal_sep(&self) -> String {
        self.resolved.decimal_sep.to_string()
    }

    /// The group separator.
    #[napi(getter, catch_unwind)]
    pub fn group_sep(&self) -> String {
        self.resolved.group_sep.to_string()
    }

    /// The leniences, `NumStyles` members combined: never a grouping policy, which is `grouping`.
    #[napi(getter, catch_unwind)]
    pub fn flags(&self) -> u32 {
        self.resolved.flags & STYLE_BITS
    }

    /// How groups of digits are read.
    #[napi(getter, catch_unwind, ts_return_type = "Grouping")]
    pub fn grouping(&self) -> u32 {
        self.resolved.flags & hc::NumFormat::GROUPING_POLICY
    }

    /// The currency symbol, `""` when none is declared.
    #[napi(getter, catch_unwind)]
    pub fn currency(&self) -> String {
        self.resolved.currency.as_str().to_string()
    }

    /// The decimal separator of a currency amount, `""` when it is `decimalSep`.
    #[napi(getter, catch_unwind)]
    pub fn currency_decimal_sep(&self) -> String {
        self.resolved.currency_decimal_sep.to_string()
    }

    /// The group separator of a currency amount, `""` when it is `groupSep`.
    #[napi(getter, catch_unwind)]
    pub fn currency_group_sep(&self) -> String {
        self.resolved.currency_group_sep.to_string()
    }

    /// The further spelling of the minus sign, `""` when none is declared.
    #[napi(getter, catch_unwind)]
    pub fn minus_sign(&self) -> String {
        self.resolved.minus_sign.to_string()
    }

    /// The further spelling of the plus sign, `""` when none is declared.
    #[napi(getter, catch_unwind)]
    pub fn plus_sign(&self) -> String {
        self.resolved.plus_sign.to_string()
    }

    /// The further spelling of the percent symbol, `""` when none is declared.
    #[napi(getter, catch_unwind)]
    pub fn percent(&self) -> String {
        self.resolved.percent.to_string()
    }

    /// The zero of the native digits, `""` when none are declared.
    #[napi(getter, catch_unwind)]
    pub fn digit_zero(&self) -> String {
        self.resolved
            .digits
            .zero()
            .map(String::from)
            .unwrap_or_default()
    }

    /// The 92 bytes this format crosses a C ABI as (the core's size-tagged layout), for a
    /// library that declares numeric columns across a C ABI of its own. The `flags` word
    /// carries the grouping policy: `Grouping.Indian` in bit 9, nothing for Western. Only the
    /// core's detailed exports read it (the 1.0 exports ignore it and read Western).
    #[napi(getter, catch_unwind)]
    pub fn packed(&self) -> Uint8Array {
        Uint8Array::from(hc::RawNumFormat::from(self.resolved).to_le_bytes().to_vec())
    }

    /// `true` when `other` is a `NumFormat` declaring the same thing, however each was made: the
    /// core's own equality, every field and the grouping policy. `===` compares identity.
    #[napi(catch_unwind, ts_args_type = "other: unknown")]
    pub fn equals<'env>(&self, other: Unknown<'env>) -> bool {
        matches!(other.get_type(), Ok(ValueType::Object))
            && unsafe { other.cast::<ClassInstance<NumFormat>>() }
                .is_ok_and(|other| other.resolved == self.resolved)
    }
}

/// A declared field order and, beside the English `AM`/`PM`, a culture's own day-period
/// designators. Made by `DateFormat.declare`, never by `new`.
#[napi]
pub struct DateFormat {
    order: hc::DateOrder,
    periods: hc::DayPeriods,
}

/// A day-period designator: absent or `""` declares none; anything else is 1 to 16 UTF-8
/// bytes with no ASCII digit and no white space at either end (`a. m.` is fine).
fn designator(what: &str, text: Option<&str>) -> std::result::Result<hc::Symbol<16>, String> {
    let Some(text) = text.filter(|text| !text.is_empty()) else {
        return Ok(hc::Symbol::NONE);
    };
    let Some(symbol) = hc::Symbol::new(text) else {
        return Err(format!(
            "Day period {what} must be 1 to 16 UTF-8 bytes with no ASCII digit; got \"{text}\""
        ));
    };
    if text.starts_with(hc::is_white_space) || text.ends_with(hc::is_white_space) {
        return Err(format!(
            "Day period {what} must not have white space at either end; got \"{text}\""
        ));
    }
    Ok(symbol)
}

fn declare_date_format(env: &Env, spec: &Object<'_>) -> std::result::Result<DateFormat, String> {
    let order = spec_field(env, spec, "order")?;
    let order = order
        .as_ref()
        .and_then(code)
        .and_then(hc::DateOrder::from_code)
        .ok_or_else(|| "order must be a DateOrder".to_string())?;
    let am = designator("am", spec_text(env, spec, "am")?.as_deref())?;
    let pm = designator("pm", spec_text(env, spec, "pm")?.as_deref())?;
    Ok(DateFormat {
        order,
        periods: hc::DayPeriods::new(am, pm),
    })
}

#[napi]
impl DateFormat {
    /// Declares a field order and day periods, or says which rule the declaration breaks;
    /// never throws. Each designator is 1 to 16 UTF-8 bytes with no ASCII digit and no
    /// white space at either end, read exactly, after the clock or ahead of it.
    #[napi(
        catch_unwind,
        ts_args_type = "spec: DateFormatSpec",
        ts_return_type = "Declaration<DateFormat>"
    )]
    pub fn declare<'env>(env: &'env Env, spec: Unknown<'env>) -> Result<Object<'env>> {
        let outcome = spec_object(&spec, "The DateFormat declaration")
            .and_then(|spec| declare_date_format(env, &spec));
        match outcome {
            Ok(format) => declared(env, Ok(format.into_instance(env)?)),
            Err(message) => declared::<bool>(env, Err(message)),
        }
    }

    /// The field order.
    #[napi(getter, catch_unwind, ts_return_type = "DateOrder")]
    pub fn order(&self) -> u32 {
        self.order as u32
    }

    /// The first-half designator, `""` when none is declared.
    #[napi(getter, catch_unwind)]
    pub fn am(&self) -> String {
        self.periods.am.to_string()
    }

    /// The second-half designator, `""` when none is declared.
    #[napi(getter, catch_unwind)]
    pub fn pm(&self) -> String {
        self.periods.pm.to_string()
    }

    /// `true` when `other` is a `DateFormat` declaring the same order and designators. `===`
    /// compares identity.
    #[napi(catch_unwind, ts_args_type = "other: unknown")]
    pub fn equals<'env>(&self, other: Unknown<'env>) -> bool {
        matches!(other.get_type(), Ok(ValueType::Object))
            && unsafe { other.cast::<ClassInstance<DateFormat>>() }
                .is_ok_and(|other| other.order == self.order && other.periods == self.periods)
    }
}

/// The refusal of a month vocabulary, one message for each rule it can break.
fn month_names_message(fault: hc::MonthNamesFault) -> String {
    match fault {
        hc::MonthNamesFault::Layout => "Month names must be packed entries: a month byte, a little-endian u32 length, then the name's UTF-8, with no entry cut short and no bytes after the last".to_string(),
        hc::MonthNamesFault::Month => "Month names must each name a month, a whole number from 1 to 12".to_string(),
        hc::MonthNamesFault::Utf8 => "Month names must be whole code points, not surrogate halves or bytes that are not UTF-8".to_string(),
        hc::MonthNamesFault::NameLength => "Month names must each be 1 to 64 UTF-8 bytes".to_string(),
        hc::MonthNamesFault::Character => "Month names must hold no ASCII digit, slash, hyphen or white space".to_string(),
        hc::MonthNamesFault::MonthMissing => "Month names must name every month from 1 to 12 at least once".to_string(),
        hc::MonthNamesFault::Duplicate => "Month names must differ once case-folded, and two fold to the same spelling".to_string(),
        other => format!("Month names refused: {other}"),
    }
}

/// A month vocabulary's packed bytes, judged in full by the core.
fn judge_month_names(packed: Vec<u8>) -> std::result::Result<MonthNames, String> {
    match hc::MonthNames::from_packed(&packed) {
        Ok(_) => Ok(MonthNames { packed }),
        Err(hc::DeclarationError::MonthNames(fault)) => Err(month_names_message(fault)),
        Err(other) => Err(format!("Month names refused: {other}")),
    }
}

/// An array argument's elements, or `None` when it is no array.
fn elements<'env>(env: &Env, value: &Unknown<'env>) -> Option<Vec<Unknown<'env>>> {
    let mut is_array = false;
    // SAFETY: `env` and `value` are the live call's.
    let status = unsafe { napi::sys::napi_is_array(env.raw(), value.raw(), &mut is_array) };
    if status != napi::sys::Status::napi_ok || !is_array {
        return None;
    }
    unsafe { value.cast::<Vec<Unknown>>() }.ok().or_else(|| {
        clear_pending_exception(env);
        None
    })
}

/// The packed layout of the `[month, name]` entries a caller wrote. A month that is no whole
/// number from 0 to 255 is written as 0, which the core refuses as a month; a name is its
/// WTF-8, so a lone surrogate reaches the core to be refused as not UTF-8.
fn pack_month_entries(env: &Env, value: &Unknown<'_>) -> std::result::Result<Vec<u8>, String> {
    let entries = elements(env, value)
        .ok_or_else(|| "Month names must be an array of [month, name] entries".to_string())?;
    let mut packed = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        let pair = elements(env, entry)
            .filter(|pair| pair.len() == 2)
            .ok_or_else(|| format!("Month names entry {i} must be a [month, name] pair"))?;
        let month = code(&pair[0])
            .and_then(|month| u8::try_from(month).ok())
            .unwrap_or(0);
        if !matches!(pair[1].get_type(), Ok(ValueType::String)) {
            return Err(format!(
                "Month names entry {i} must be a [month, name] pair with a string name"
            ));
        }
        let name = string_wtf8(env, &pair[1])
            .ok_or_else(|| format!("Month names entry {i} could not be read"))?;
        packed.push(month);
        packed.extend_from_slice(&u32::try_from(name.len()).unwrap_or(u32::MAX).to_le_bytes());
        packed.extend_from_slice(&name);
    }
    Ok(packed)
}

/// Declared month names for the date doors that read a month field (`castDate` with an order,
/// `castDatetime`): `janvier`, `Sep` and `Sept`, `janv.`. Caller-built: no platform
/// vocabulary is looked up. Each entry maps a name to a month from 1 to 12 and aliases are
/// separate entries. Names match under simple case folding, and the longest name followed by
/// the separator the date uses wins; punctuation is part of a name and never stripped, so
/// with `janv.` declared and `.` as the separator, `7.janv..2026` reads while `7.janv.2026`
/// does not unless `janv` is also declared. Every month needs a name; a name is 1 to 64 UTF-8
/// bytes with no ASCII digit, `/`, `-` or white space, and two that fold alike are refused.
/// Made by `MonthNames.declare` or `MonthNames.fromPacked`, never by `new`.
#[napi]
#[derive(Clone, PartialEq, Eq)]
pub struct MonthNames {
    packed: Vec<u8>,
}

#[napi]
impl MonthNames {
    /// Declares month names from `[month, name]` entries, or says which rule they break; never
    /// throws.
    #[napi(
        catch_unwind,
        ts_args_type = "entries: ReadonlyArray<readonly [month: number, name: string]>",
        ts_return_type = "Declaration<MonthNames>"
    )]
    pub fn declare<'env>(env: &'env Env, entries: Unknown<'env>) -> Result<Object<'env>> {
        match pack_month_entries(env, &entries).and_then(judge_month_names) {
            Ok(names) => declared(env, Ok(names.into_instance(env)?)),
            Err(message) => declared::<bool>(env, Err(message)),
        }
    }

    /// Declares month names from their packed layout, the one the C ABI takes: each entry a
    /// month byte (1 to 12), a little-endian `u32` byte length, then that many UTF-8 bytes. Says
    /// which rule the bytes break; never throws.
    #[napi(
        catch_unwind,
        ts_args_type = "bytes: Uint8Array",
        ts_return_type = "Declaration<MonthNames>"
    )]
    pub fn from_packed<'env>(env: &'env Env, bytes: Unknown<'env>) -> Result<Object<'env>> {
        let packed = match bytes.get_type() {
            Ok(ValueType::Object) => unsafe { bytes.cast::<Uint8Array>() }
                .map(|bytes| bytes.to_vec())
                .map_err(|_| "Month names bytes must be a Uint8Array".to_string()),
            _ => Err("Month names bytes must be a Uint8Array".to_string()),
        };
        match packed.and_then(judge_month_names) {
            Ok(names) => declared(env, Ok(names.into_instance(env)?)),
            Err(message) => declared::<bool>(env, Err(message)),
        }
    }

    /// The names in the packed layout the core reads: each entry a month byte, a little-endian
    /// `u32` byte length, then the UTF-8, in the order declared.
    #[napi(getter, catch_unwind)]
    pub fn packed(&self) -> Uint8Array {
        Uint8Array::from(self.packed.clone())
    }

    /// `true` when `other` is a `MonthNames` holding the same entries in the same order: the
    /// same `packed` bytes. `===` compares identity.
    #[napi(catch_unwind, ts_args_type = "other: unknown")]
    pub fn equals<'env>(&self, other: Unknown<'env>) -> bool {
        matches!(other.get_type(), Ok(ValueType::Object))
            && unsafe { other.cast::<ClassInstance<MonthNames>>() }
                .is_ok_and(|other| *other == *self)
    }
}

/// The extra date shapes the date doors can read, a pivot for two-digit years, and month
/// names: all opt in, so a door keeps its own shape rules unless it is given options. Made by
/// `DateOptions.declare` or `DateOptions.commonForms`, never by `new`.
#[napi]
pub struct DateOptions {
    flags: u32,
    pivot: u32,
    months: Option<MonthNames>,
}

/// A refused date-options declaration, worded for the rule that refused it.
fn date_options_message(error: hc::DeclarationError, flags: u32, pivot: u32) -> String {
    match error {
        hc::DeclarationError::DateFlags => {
            format!("Date options flags must be DateStyles members combined; got {flags}")
        }
        hc::DeclarationError::DatePivot => {
            format!("Date options pivot must be 0 (none) or a year from 100 to 9999; got {pivot}")
        }
        hc::DeclarationError::MonthNames(fault) => month_names_message(fault),
        other => format!("Date options refused: {other}"),
    }
}

fn declare_date_options(env: &Env, spec: &Object<'_>) -> std::result::Result<DateOptions, String> {
    let whole = |name: &str| -> std::result::Result<u32, String> {
        match spec_field(env, spec, name)? {
            Some(value) if !matches!(value.get_type(), Ok(ValueType::Undefined)) => code(&value)
                .ok_or_else(|| format!("{name} must be a whole number from 0 to 4294967295")),
            _ => Ok(0),
        }
    };
    let flags = whole("flags")?;
    let pivot = whole("pivot")?;
    let months = match spec_field(env, spec, "months")? {
        Some(value) if !matches!(value.get_type(), Ok(ValueType::Undefined)) => {
            let names = match value.get_type() {
                Ok(ValueType::Object) => unsafe { value.cast::<ClassInstance<MonthNames>>() }
                    .ok()
                    .map(|names| MonthNames {
                        packed: names.packed.clone(),
                    }),
                _ => None,
            };
            Some(names.ok_or_else(|| "months must be a MonthNames".to_string())?)
        }
        _ => None,
    };
    // A pivot past a `u16` is refused as itself: `u16::MAX` is no pivot either.
    let options =
        hc::DateOptions::from_flags(flags).with_pivot(u16::try_from(pivot).unwrap_or(u16::MAX));
    let names = months
        .as_ref()
        .map(|names| hc::MonthNames::from_packed(&names.packed))
        .transpose()
        .map_err(|error| date_options_message(error, flags, pivot))?;
    options
        .validate(names)
        .map_err(|error| date_options_message(error, flags, pivot))?;
    Ok(DateOptions {
        flags,
        pivot,
        months,
    })
}

#[napi]
impl DateOptions {
    /// Declares date options, or says which rule the declaration breaks; never throws. The
    /// styles are `DateStyles` members, the pivot is 0 (none) or from 100 to 9999, and the
    /// month names are a `MonthNames`.
    #[napi(
        catch_unwind,
        ts_args_type = "spec: DateOptionsSpec",
        ts_return_type = "Declaration<DateOptions>"
    )]
    pub fn declare<'env>(env: &'env Env, spec: Unknown<'env>) -> Result<Object<'env>> {
        let outcome = spec_object(&spec, "The DateOptions declaration")
            .and_then(|spec| declare_date_options(env, &spec));
        match outcome {
            Ok(options) => declared(env, Ok(options.into_instance(env)?)),
            Err(message) => declared::<bool>(env, Err(message)),
        }
    }

    /// `DateStyles.CommonForms`: compact dates, a space for the `T` and `±hh` offsets,
    /// with no pivot and no month names. It works at every date door: a style a door does not
    /// read is ignored there.
    #[napi(factory, catch_unwind)]
    pub fn common_forms() -> Self {
        DateOptions {
            flags: hc::DateOptions::COMMON_FORMS.flags(),
            pivot: 0,
            months: None,
        }
    }

    /// The extra shapes, `DateStyles` members combined.
    #[napi(getter, catch_unwind, ts_return_type = "DateStyles")]
    pub fn flags(&self) -> u32 {
        self.flags
    }

    /// The pivot, `0` for none.
    #[napi(getter, catch_unwind)]
    pub fn pivot(&self) -> u32 {
        self.pivot
    }

    /// The declared month names, or `null`.
    #[napi(getter, catch_unwind)]
    pub fn months(&self) -> Option<MonthNames> {
        self.months.clone()
    }

    /// The 12 bytes these options cross a C ABI as (the core's size-tagged layout: the size,
    /// the flags and the pivot, each a little-endian `u32`), with `months.packed` beside it.
    #[napi(getter, catch_unwind)]
    pub fn packed(&self) -> Uint8Array {
        let mut bytes = Vec::with_capacity(hc::RawDateOptions::SIZE as usize);
        for word in [hc::RawDateOptions::SIZE, self.flags, self.pivot] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        Uint8Array::from(bytes)
    }

    /// `true` when `other` is a `DateOptions` declaring the same shapes and pivot, and month names
    /// with the same entries in the same order, or none on both. `===` compares identity.
    #[napi(catch_unwind, ts_args_type = "other: unknown")]
    pub fn equals<'env>(&self, other: Unknown<'env>) -> bool {
        matches!(other.get_type(), Ok(ValueType::Object))
            && unsafe { other.cast::<ClassInstance<DateOptions>>() }.is_ok_and(|other| {
                other.flags == self.flags
                    && other.pivot == self.pivot
                    && other.months == self.months
            })
    }
}

// ---------------------------------------------------------------------------------------
// The doors.
// ---------------------------------------------------------------------------------------

/// The core's version as `major.minor.patch`, unpacked from `hypercast_version()`.
#[napi(catch_unwind)]
pub fn native_version() -> String {
    let packed = hc::hypercast_version();
    format!(
        "{}.{}.{}",
        packed >> 16,
        (packed >> 8) & 0xff,
        packed & 0xff
    )
}

/// Casts boolean text under the natural-language lexicon.
#[napi(
    catch_unwind,
    ts_args_type = "text: Text",
    ts_return_type = "Verdict<boolean>"
)]
pub fn cast_bool<'env>(env: &'env Env, text: Unknown<'env>) -> Result<Object<'env>> {
    door(
        env,
        &text,
        Some(()),
        |bytes, ()| hc::cast_bool(bytes),
        |value| value,
    )
}

/// Casts one character: the text verbatim when it is exactly one scalar, otherwise a
/// declared code point (`65`, `U+0041`, `0x41`, `&H41`, `&#65;`, `&#x41;`). Past U+10FFFF
/// or a surrogate is `outOfRange`.
#[napi(
    catch_unwind,
    ts_args_type = "text: Text",
    ts_return_type = "Verdict<string>"
)]
pub fn cast_char<'env>(env: &'env Env, text: Unknown<'env>) -> Result<Object<'env>> {
    door(
        env,
        &text,
        Some(()),
        |bytes, ()| hc::cast_char(bytes),
        String::from,
    )
}

macro_rules! numeric_doors {
    ($($doc:literal $door:ident => $core:ident as $ts:literal, $value:expr;)+) => {$(
        #[doc = $doc]
        #[napi(
            catch_unwind,
            ts_args_type = "text: Text, format?: NumFormat | undefined | null",
            ts_return_type = $ts
        )]
        pub fn $door<'env>(
            env: &'env Env,
            text: Unknown<'env>,
            format: Unknown<'env>,
        ) -> Result<Object<'env>> {
            door(env, &text, num_format(&format), |bytes, format| hc::$core(bytes, &format), $value)
        }
    )+};
}

// Each door's doc string is a literal, not a `///` comment: a comment passed through the
// macro reaches napi's type generator as a raw string token, and binding.d.ts would print it
// with its `r"` quotes.
numeric_doors! {
    "Casts integer text to a signed 8-bit number under the declared format."
    cast_i8 => cast_i8 as "Verdict<number>", i32::from;
    "Casts integer text to a signed 16-bit number under the declared format."
    cast_i16 => cast_i16 as "Verdict<number>", i32::from;
    "Casts integer text to a signed 32-bit number under the declared format."
    cast_i32 => cast_i32 as "Verdict<number>", |value: i32| value;
    "Casts integer text to a signed 64-bit `bigint` under the declared format."
    cast_i64 => cast_i64 as "Verdict<bigint>", BigInt::from;
    "Casts integer text to an unsigned 8-bit number under the declared format."
    cast_u8 => cast_u8 as "Verdict<number>", u32::from;
    "Casts integer text to an unsigned 16-bit number under the declared format."
    cast_u16 => cast_u16 as "Verdict<number>", u32::from;
    "Casts integer text to an unsigned 32-bit number under the declared format."
    cast_u32 => cast_u32 as "Verdict<number>", |value: u32| value;
    "Casts integer text to an unsigned 64-bit `bigint` under the declared format."
    cast_u64 => cast_u64 as "Verdict<bigint>", BigInt::from;
    "Casts real text to an IEEE single, widened exactly to a number, under the declared format."
    cast_f32 => cast_f32 as "Verdict<number>", f64::from;
    "Casts real text to an IEEE double under the declared format."
    cast_f64 => cast_f64 as "Verdict<number>", |value: f64| value;
    "Casts decimal text under the declared format to its exact canonical text: trailing fraction zeros trimmed (`\"1.10\"` is `\"1.1\"`), never rounded, at most 28 places and 96 bits of magnitude."
    cast_decimal => cast_decimal as "Verdict<string>", |value: hc::Decimal| value.to_string();
}

/// Casts UUID text (every .NET `Guid` form, and `urn:uuid:`-style prefixes) to its
/// lowercase hyphenated form.
#[napi(
    catch_unwind,
    ts_args_type = "text: Text",
    ts_return_type = "Verdict<string>"
)]
pub fn cast_uuid<'env>(env: &'env Env, text: Unknown<'env>) -> Result<Object<'env>> {
    door(
        env,
        &text,
        Some(()),
        |bytes, ()| hc::cast_uuid(bytes),
        uuid_text,
    )
}

/// Casts an RFC 3339 instant, its zone mandatory, to seconds and nanoseconds since the epoch.
/// `options` (`DateOptions.declare({ flags: DateStyles.SpaceSeparator | DateStyles.HourOffset
/// })`, or `DateOptions.commonForms()`) open a space for the `T` and `±hh` offsets; without
/// them the door reads the strict shape.
#[napi(
    catch_unwind,
    ts_args_type = "text: Text, options?: DateOptions | undefined | null",
    ts_return_type = "Verdict<Timestamp>"
)]
pub fn cast_timestamp<'env>(
    env: &'env Env,
    text: Unknown<'env>,
    options: Unknown<'env>,
) -> Result<Object<'env>> {
    if !is_nothing(&options) {
        let call = Call::of(hc::Door::Timestamp, 0).and_then(|mut call| {
            call.dates = dates_arg(&options)?;
            Some(call)
        });
        return run(env, &text, call, false);
    }
    door(
        env,
        &text,
        Some(()),
        |bytes, ()| hc::cast_timestamp(bytes),
        timestamp,
    )
}

/// Casts an integer Unix-epoch value under the declared precision.
#[napi(
    catch_unwind,
    ts_args_type = "text: Text, precision: UnixPrecision",
    ts_return_type = "Verdict<Timestamp>"
)]
pub fn cast_unix<'env>(
    env: &'env Env,
    text: Unknown<'env>,
    precision: Unknown<'env>,
) -> Result<Object<'env>> {
    let precision = code(&precision).and_then(hc::UnixPrecision::from_code);
    door(
        env,
        &text,
        precision,
        |bytes, precision| hc::cast_unix(bytes, precision),
        timestamp,
    )
}

/// Casts an Excel date serial under the declared epoch, read as UTC.
#[napi(
    catch_unwind,
    ts_args_type = "text: Text, epoch: ExcelEpoch",
    ts_return_type = "Verdict<Timestamp>"
)]
pub fn cast_excel_serial<'env>(
    env: &'env Env,
    text: Unknown<'env>,
    epoch: Unknown<'env>,
) -> Result<Object<'env>> {
    let epoch = code(&epoch).and_then(hc::ExcelEpoch::from_code);
    door(
        env,
        &text,
        epoch,
        |bytes, epoch| hc::cast_excel_serial(bytes, epoch),
        timestamp,
    )
}

/// Casts a calendar date: strict ISO `yyyy-MM-dd` with no order, the separated forms
/// (`1/7/2026`, `1.7.2026`) under a declared one. `options` open `20260107` (no order), and
/// under an order a pivot for two-digit years and declared month names (`07-Jan-26`).
#[napi(
    catch_unwind,
    ts_args_type = "text: Text, order?: DateOrder | undefined | null, options?: DateOptions | MonthNames | undefined | null",
    ts_return_type = "Verdict<CalendarDate>"
)]
pub fn cast_date<'env>(
    env: &'env Env,
    text: Unknown<'env>,
    order: Unknown<'env>,
    options: Unknown<'env>,
) -> Result<Object<'env>> {
    let order = match order.get_type() {
        Ok(ValueType::Undefined | ValueType::Null) => Some(None),
        _ => code(&order).and_then(hc::DateOrder::from_code).map(Some),
    };
    if !is_nothing(&options) {
        let call = order.and_then(|order| {
            let mut call = match order {
                None => Call::of(hc::Door::Date, 0)?,
                Some(order) => Call::of(hc::Door::DateOrdered(order), order as u32)?,
            };
            call.dates = dates_arg(&options)?;
            Some(call)
        });
        return run(env, &text, call, false);
    }
    door(
        env,
        &text,
        order,
        |bytes, order| match order {
            None => hc::cast_date(bytes),
            Some(order) => hc::cast_date_ordered(bytes, order),
        },
        calendar,
    )
}

/// Casts a zone-less civil date-time under a declared order, or a `DateFormat` whose day
/// periods are read beside the English `AM`/`PM`. No zone is read and none is invented.
/// `options` give a pivot for two-digit years and declared month names.
#[napi(
    catch_unwind,
    ts_args_type = "text: Text, format: DateOrder | DateFormat, options?: DateOptions | MonthNames | undefined | null",
    ts_return_type = "Verdict<CivilDateTime>"
)]
pub fn cast_datetime<'env>(
    env: &'env Env,
    text: Unknown<'env>,
    format: Unknown<'env>,
    options: Unknown<'env>,
) -> Result<Object<'env>> {
    if !is_nothing(&options) {
        let call = datetime_call(&format).and_then(|mut call| {
            call.dates = dates_arg(&options)?;
            Some(call)
        });
        return run(env, &text, call, false);
    }
    let declared = match format.get_type() {
        Ok(ValueType::Number) => code(&format)
            .and_then(hc::DateOrder::from_code)
            .map(|order| (order, hc::DayPeriods::ENGLISH)),
        Ok(ValueType::Object) => unsafe { format.cast::<ClassInstance<DateFormat>>() }
            .ok()
            .map(|format| (format.order, format.periods)),
        _ => None,
    };
    door(
        env,
        &text,
        declared,
        |bytes, (order, periods)| {
            // Nothing declared takes the core's English door, whose designators are constants.
            if periods == hc::DayPeriods::ENGLISH {
                hc::cast_datetime(bytes, order)
            } else {
                hc::cast_datetime_with(bytes, order, &periods)
            }
        },
        civil,
    )
}

/// Casts an ISO 24-hour time of day.
#[napi(
    catch_unwind,
    ts_args_type = "text: Text",
    ts_return_type = "Verdict<TimeOfDay>"
)]
pub fn cast_time<'env>(env: &'env Env, text: Unknown<'env>) -> Result<Object<'env>> {
    door(
        env,
        &text,
        Some(()),
        |bytes, ()| hc::cast_time(bytes),
        time_of_day,
    )
}

/// Casts a duration (ISO 8601, the invariant colon form, or protobuf JSON seconds).
#[napi(
    catch_unwind,
    ts_args_type = "text: Text",
    ts_return_type = "Verdict<Duration>"
)]
pub fn cast_duration<'env>(env: &'env Env, text: Unknown<'env>) -> Result<Object<'env>> {
    door(
        env,
        &text,
        Some(()),
        |bytes, ()| hc::cast_duration(bytes),
        duration,
    )
}

/// Reads a number as the exact decimal it names: the shortest decimal that rounds back to
/// it, the digits a spreadsheet writes (`0.1` is `"0.1"`). NaN is `malformed`; an infinity,
/// past 96 bits or more than 28 places is `outOfRange`.
#[napi(
    catch_unwind,
    ts_args_type = "value: number",
    ts_return_type = "Verdict<string>"
)]
pub fn cast_decimal_from_number<'env>(
    env: &'env Env,
    value: Unknown<'env>,
) -> Result<Object<'env>> {
    typed(
        env,
        &value,
        Some(()),
        |number, ()| hc::decimal_from_f64(number),
        |value| value.to_string(),
    )
}

/// Reads an Excel serial number under the declared epoch as the zone-less date-time it names.
#[napi(
    catch_unwind,
    ts_args_type = "value: number, epoch: ExcelEpoch",
    ts_return_type = "Verdict<CivilDateTime>"
)]
pub fn cast_excel_serial_from_number<'env>(
    env: &'env Env,
    value: Unknown<'env>,
    epoch: Unknown<'env>,
) -> Result<Object<'env>> {
    let epoch = code(&epoch).and_then(hc::ExcelEpoch::from_code);
    typed(env, &value, epoch, hc::excel_serial, civil)
}

/// Reads the fraction of an Excel serial number as a time of day.
#[napi(
    catch_unwind,
    ts_args_type = "value: number",
    ts_return_type = "Verdict<TimeOfDay>"
)]
pub fn cast_excel_time<'env>(env: &'env Env, value: Unknown<'env>) -> Result<Object<'env>> {
    typed(
        env,
        &value,
        Some(()),
        |number, ()| hc::excel_time(number),
        time_of_day,
    )
}

/// Reads a number of days as a duration.
#[napi(
    catch_unwind,
    ts_args_type = "value: number",
    ts_return_type = "Verdict<Duration>"
)]
pub fn cast_excel_duration<'env>(env: &'env Env, value: Unknown<'env>) -> Result<Object<'env>> {
    typed(
        env,
        &value,
        Some(()),
        |number, ()| hc::excel_duration(number),
        duration,
    )
}

// ---------------------------------------------------------------------------------------
// Validation constraints.
//
// A declaration is laid out exactly as the C ABI carries it, a `RawConstraints` with the
// packed `enum` list beside it, and judged by the core's own exports: `cast_constraints_check`
// once, when a `Constraints` is declared, and `cast_checked`, `cast_checked_from_f64` and
// `cast_text` on every cast. Bounds, steps and integer entries are converted by
// `cast_declared_decimal`. No rule is restated here: every refusal and every violation is the
// core's.
// ---------------------------------------------------------------------------------------

/// The fault span as it crosses the C ABI (ffi.rs's `RawFault`, restated as two `u32`s).
#[repr(C)]
#[derive(Default)]
struct RawFault {
    offset: u32,
    len: u32,
}

// The layouts this file passes to the core, pinned at the sizes the header carries
// (`HyperCastRawConstraints_SIZE` 72, `HyperCastRawCheck_SIZE` 12, `HyperCastRawSpan_SIZE` 8,
// `HyperCastRawFaultDetail_SIZE` 16, `HyperCastRawDateOptions_SIZE` 12 and the plain fault's
// two words); tests/constraints.test.ts holds the header to the same numbers. A value a door
// writes is read from a 16-byte slot, so none may outgrow it.
const _: () = {
    assert!(size_of::<hc::RawConstraints>() == 72 && hc::RawConstraints::SIZE == 72);
    assert!(align_of::<hc::RawConstraints>() == 8);
    assert!(size_of::<hc::RawCheck>() == 12 && align_of::<hc::RawCheck>() == 4);
    assert!(size_of::<hc::RawSpan>() == 8 && align_of::<hc::RawSpan>() == 4);
    assert!(size_of::<RawFault>() == 8 && align_of::<RawFault>() == 4);
    assert!(size_of::<hc::RawFaultDetail>() == 16 && hc::RawFaultDetail::SIZE == 16);
    assert!(align_of::<hc::RawFaultDetail>() == 4);
    assert!(size_of::<hc::RawDateOptions>() == 12 && hc::RawDateOptions::SIZE == 12);
    assert!(align_of::<hc::RawDateOptions>() == 4);
    assert!(size_of::<hc::Decimal>() == 16 && align_of::<hc::Decimal>() == 8);
    assert!(size_of::<hc::Timestamp>() == 16 && size_of::<hc::Duration>() == 16);
    assert!(size_of::<hc::CivilDateTime>() <= 16 && align_of::<hc::CivilDateTime>() <= 8);
    assert!(size_of::<hc::Date>() <= 16 && align_of::<hc::Date>() <= 8);
};

/// The core's exports this file calls, as include/hypercast.h declares them. The detailed ones
/// read a `NumFormat`'s grouping policy and report a failure's detail; the 1.0 checked exports
/// ignore the policy and always read Western, so no door here calls them.
mod abi {
    use super::{RawFault, hc};

    unsafe extern "C" {
        pub(super) fn cast_detailed(
            door: u32,
            param: u32,
            format: *const hc::RawNumFormat,
            periods: *const hc::RawDayPeriods,
            date_options: *const hc::RawDateOptions,
            vocab_ptr: *const u8,
            vocab_len: usize,
            ptr: *const u8,
            len: usize,
            out: *mut std::ffi::c_void,
            fault: *mut hc::RawFaultDetail,
        ) -> i32;
        pub(super) fn cast_detailed_from_f64(
            door: u32,
            param: u32,
            value: f64,
            out: *mut std::ffi::c_void,
            fault: *mut hc::RawFaultDetail,
        ) -> i32;
        pub(super) fn cast_checked_detailed(
            door: u32,
            param: u32,
            format: *const hc::RawNumFormat,
            constraints: *const hc::RawConstraints,
            enum_ptr: *const u8,
            enum_len: usize,
            ptr: *const u8,
            len: usize,
            out: *mut std::ffi::c_void,
            fault: *mut hc::RawFaultDetail,
            check: *mut hc::RawCheck,
        ) -> i32;
        pub(super) fn cast_checked_detailed_from_f64(
            door: u32,
            param: u32,
            constraints: *const hc::RawConstraints,
            enum_ptr: *const u8,
            enum_len: usize,
            value: f64,
            out: *mut std::ffi::c_void,
            fault: *mut hc::RawFaultDetail,
            check: *mut hc::RawCheck,
        ) -> i32;
        pub(super) fn cast_constraints_check(
            door: u32,
            param: u32,
            constraints: *const hc::RawConstraints,
            enum_ptr: *const u8,
            enum_len: usize,
        ) -> i32;
        pub(super) fn cast_text(
            ptr: *const u8,
            len: usize,
            out: *mut hc::RawSpan,
            fault: *mut RawFault,
        ) -> i32;
        pub(super) fn cast_declared_decimal(
            ptr: *const u8,
            len: usize,
            out: *mut hc::Decimal,
        ) -> i32;
    }
}

/// A door, numbered as the core numbers it. A `Constraints` is bound to the integer family,
/// the two reals and the exact decimal (judged by `castChecked` and `castCheckedFromNumber`),
/// or to text (judged by `castText`); `castDetailed` and `castDetailedFromNumber` take every
/// door the core's detailed exports name.
#[napi]
pub enum Door {
    /// A natural-language boolean.
    Bool = 1,
    I8 = 2,
    I16 = 3,
    I32 = 4,
    I64 = 5,
    U8 = 6,
    U16 = 7,
    U32 = 8,
    U64 = 9,
    F32 = 10,
    F64 = 11,
    /// A UUID in any text form the door reads.
    Uuid = 12,
    /// An RFC 3339 instant.
    Timestamp = 13,
    /// A Unix-epoch count; the declaration is a `UnixPrecision`.
    Unix = 14,
    /// A calendar date, strict ISO `yyyy-MM-dd` unless `DateStyles.CompactDate` is declared.
    Date = 15,
    /// An ISO 24-hour time of day.
    Time = 16,
    /// A duration.
    Duration = 17,
    Text = 18,
    Decimal = 19,
    /// A separated calendar date; the declaration is a `DateOrder`.
    DateOrdered = 20,
    /// A zone-less civil date-time; the declaration is a `DateOrder` or a `DateFormat`.
    DateTime = 21,
    /// An Excel date serial; the declaration is an `ExcelEpoch`.
    ExcelSerial = 22,
    /// One character, verbatim or as a declared code point.
    Char = 23,
}

/// What a value that is not a multiple of the declared `multipleOf` becomes, the core's codes
/// verbatim. Every mode but `Strict` needs a `multipleOf`, and rounding happens on the exact
/// decimal, before the bounds judge the result.
#[napi]
pub enum Rounding {
    /// Not a multiple is a `multipleOf` violation. The default.
    Strict = 0,
    /// To the nearest multiple, a tie to the even one.
    Midpoint = 1,
    /// To the multiple toward negative infinity.
    Floor = 2,
    /// To the multiple toward positive infinity.
    Ceiling = 3,
}

/// The declared form of a `Constraints`: `Constraints.declare` reads it. The integer doors
/// take the bounds, `multipleOf` with a `rounding`, and `enum`; `Door.F32`, `Door.F64` and
/// `Door.Decimal` the same but `enum`; `Door.Text` `minLength`, `maxLength` (in code points),
/// `trim` and `enum`. A bound, step or integer entry is a decimal string in invariant
/// notation (`"0.000001"`, `"-90"`, `"1e-6"`) or a `bigint`, never a `number`: a number is a
/// binary float, which holds most decimals only approximately.
#[napi(object, object_to_js = false, object_from_js = false)]
pub struct ConstraintsSpec {
    /// The door the declaration is bound to: an integer, real or decimal door, or `Door.Text`.
    #[napi(
        ts_type = "Door.I8 | Door.I16 | Door.I32 | Door.I64 | Door.U8 | Door.U16 | Door.U32 | Door.U64 | Door.F32 | Door.F64 | Door.Decimal | Door.Text"
    )]
    pub door: u32,
    /// The least value allowed.
    #[napi(ts_type = "string | bigint")]
    pub minimum: Option<String>,
    /// The bound every value must lie above.
    #[napi(ts_type = "string | bigint")]
    pub exclusive_minimum: Option<String>,
    /// The greatest value allowed.
    #[napi(ts_type = "string | bigint")]
    pub maximum: Option<String>,
    /// The bound every value must lie below.
    #[napi(ts_type = "string | bigint")]
    pub exclusive_maximum: Option<String>,
    /// The step every value must be a multiple of.
    #[napi(ts_type = "string | bigint")]
    pub multiple_of: Option<String>,
    /// What a value that is not a multiple becomes; `Rounding.Strict` when absent.
    #[napi(ts_type = "Rounding")]
    pub rounding: Option<u32>,
    /// The fewest code points the text may hold.
    pub min_length: Option<u32>,
    /// The most code points the text may hold.
    pub max_length: Option<u32>,
    /// Trim Unicode `White_Space` from both ends before judging.
    pub trim: Option<bool>,
    /// The values allowed: strings at `Door.Text` (matched under simple case folding), decimal
    /// strings or `bigint`s at an integer door. Left out declares none; an empty array declares
    /// one no value could match, which the core refuses (code 12).
    #[napi(js_name = "enum", ts_type = "ReadonlyArray<string | bigint>")]
    pub entries: Option<Vec<String>>,
}

/// A rule a value broke, in the OpenAPI spelling. Open to additions: `format` and `pattern`
/// may join in a minor release, so a `switch` over keywords needs a `default` arm, and a code
/// this binding does not name yet arrives as `"keyword N"`.
#[napi(object, object_from_js = false)]
pub struct Violation {
    /// The keyword broken.
    #[napi(
        ts_type = "'minimum' | 'exclusiveMinimum' | 'maximum' | 'exclusiveMaximum' | 'multipleOf' | 'minLength' | 'maxLength' | 'enum' | (string & {})"
    )]
    pub keyword: String,
    /// Where the value starts, in the caller's own units, as a fault's span is.
    pub offset: u32,
    /// How long it runs; 0 for a number already decoded, which has no text.
    pub length: u32,
}

/// The success case of a checked door: `ok` is `true`; `value` is the value (rounded to
/// `multipleOf` when the declaration rounds; at `Door.Text` with an `enum`, the caller's own
/// declared entry); `adjusted` is true only when rounding changed it; `enumIndex` is the index
/// of the entry a text matched, `null` otherwise (an integer `enum` included: the value is the
/// answer).
#[napi(
    object,
    object_to_js = false,
    object_from_js = false,
    js_name = "Checked<T>"
)]
pub struct CheckedCase {
    #[napi(ts_type = "true")]
    pub ok: bool,
    #[napi(ts_type = "T")]
    pub value: bool,
    pub adjusted: bool,
    #[napi(ts_type = "number | null")]
    pub enum_index: u32,
}

/// The failure case of a checked door for a value that parsed and broke a declared rule:
/// `ok` is `false` and `violation` says which rule, and where.
#[napi(object, object_to_js = false, object_from_js = false)]
pub struct Violated {
    #[napi(ts_type = "false")]
    pub ok: bool,
    pub violation: Violation,
}

/// Why a checked door refused a value: it did not parse (`Failure`, today's fault), or it
/// parsed and broke a rule (`Violated`).
#[napi(js_name = "Rejection")]
pub type RejectionType = Either<Failure, Violated>;

/// The outcome of a checked door: exactly one of `Checked<T>`, `Failure` or `Violated`, told
/// apart by `ok` and then by `'fault' in verdict`.
#[napi(js_name = "CheckedVerdict<T>")]
pub type CheckedVerdictType = Either3<CheckedCase, Failure, Violated>;

/// The outcome of a detailed checked door: exactly one of `Checked<T>`, `DetailedFailure` or
/// `Violated`, told apart by `ok` and then by `'fault' in verdict`. A violation names a rule
/// the value broke, not a failure to read it, so it carries no detail.
#[napi(js_name = "DetailedCheckedVerdict<T>")]
pub type DetailedCheckedVerdictType = Either3<CheckedCase, DetailedFailure, Violated>;

/// The refusals, in code order, for their wording.
const REFUSALS: [hc::Refusal; 13] = [
    hc::Refusal::KeywordNotForDoor,
    hc::Refusal::MinimumWithExclusive,
    hc::Refusal::MaximumWithExclusive,
    hc::Refusal::LowerNotBelowUpper,
    hc::Refusal::MultipleOfNotPositive,
    hc::Refusal::RoundingWithoutMultipleOf,
    hc::Refusal::MinLengthAboveMax,
    hc::Refusal::NotHeldByDoor,
    hc::Refusal::EnumNotUtf8,
    hc::Refusal::DuplicateEnum,
    hc::Refusal::MalformedLayout,
    hc::Refusal::EmptyEnum,
    hc::Refusal::UnmatchableEnumEntry,
];

/// The keywords this binding names.
const KEYWORDS: [hc::Keyword; 8] = [
    hc::Keyword::Minimum,
    hc::Keyword::ExclusiveMinimum,
    hc::Keyword::Maximum,
    hc::Keyword::ExclusiveMaximum,
    hc::Keyword::MultipleOf,
    hc::Keyword::MinLength,
    hc::Keyword::MaxLength,
    hc::Keyword::Enum,
];

fn keyword_name(code: u32) -> String {
    match KEYWORDS.iter().find(|keyword| keyword.code() == code) {
        Some(keyword) => keyword.name().to_string(),
        None => format!("keyword {code}"),
    }
}

/// A refused declaration, worded from the code the core returned: one message per code.
fn refused_message(code: i32) -> String {
    match REFUSALS
        .iter()
        .find(|refusal| i64::from(refusal.code()) == i64::from(code))
    {
        Some(refusal) => format!("Constraints refused (code {code}): {refusal}."),
        None => format!("Constraints refused (code {code}): a rule this binding does not name."),
    }
}

/// What the core cannot be asked to judge, which is a broken contract and never data.
fn binding_bug(what: &str) -> napi::Error {
    napi::Error::from_reason(format!("hypercast {what}: a binding bug, please report it"))
}

/// A string as WTF-8: its UTF-8, with each lone surrogate the three bytes no UTF-8 decoder
/// accepts. The core then reads a lone surrogate as malformed text at its own code unit, never
/// as the U+FFFD napi's UTF-8 conversion would put in its place.
fn wtf8(units: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(units.len());
    for decoded in char::decode_utf16(units.iter().copied()) {
        match decoded {
            Ok(c) => out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
            Err(lone) => {
                let unit = u32::from(lone.unpaired_surrogate());
                out.extend_from_slice(&[
                    0xE0 | (unit >> 12) as u8,
                    0x80 | ((unit >> 6) & 0x3F) as u8,
                    0x80 | (unit & 0x3F) as u8,
                ]);
            }
        }
    }
    out
}

/// A string argument's WTF-8, or `None` when it is no string. Its UTF-16 is read as it is:
/// napi's own UTF-8 read would have replaced a lone surrogate already.
fn string_wtf8(env: &Env, value: &Unknown<'_>) -> Option<Vec<u8>> {
    let ok = napi::sys::Status::napi_ok;
    let mut len = 0usize;
    // SAFETY: `env` and `value` are the live call's; a null buffer asks for the length.
    let status = unsafe {
        napi::sys::napi_get_value_string_utf16(
            env.raw(),
            value.raw(),
            std::ptr::null_mut(),
            0,
            &mut len,
        )
    };
    if status != ok {
        return None;
    }
    // One more unit for the terminator napi writes, dropped below.
    let mut units = vec![0u16; len + 1];
    let mut written = 0usize;
    // SAFETY: `units` holds `len + 1` units.
    let status = unsafe {
        napi::sys::napi_get_value_string_utf16(
            env.raw(),
            value.raw(),
            units.as_mut_ptr(),
            units.len(),
            &mut written,
        )
    };
    (status == ok).then(|| wtf8(&units[..written.min(len)]))
}

/// A checked door's text: a string as WTF-8 (with the string itself, which a success that is
/// all of it hands back), the bytes of a `Uint8Array` as they are, or `null`/`undefined`.
enum CheckedInput<'env> {
    Str(Vec<u8>, JsString<'env>),
    Bytes(Uint8Array),
    Absent,
    Invalid,
}

impl<'env> CheckedInput<'env> {
    fn read(env: &Env, value: &Unknown<'env>) -> CheckedInput<'env> {
        match value.get_type() {
            Ok(ValueType::String) => {
                match (string_wtf8(env, value), unsafe { value.cast::<JsString>() }) {
                    (Some(bytes), Ok(string)) => CheckedInput::Str(bytes, string),
                    _ => CheckedInput::Invalid,
                }
            }
            Ok(ValueType::Undefined | ValueType::Null) => CheckedInput::Absent,
            Ok(ValueType::Object) => match unsafe { value.cast::<Uint8Array>() } {
                Ok(bytes) => CheckedInput::Bytes(bytes),
                Err(_) => CheckedInput::Invalid,
            },
            _ => CheckedInput::Invalid,
        }
    }

    fn bytes(&self) -> &[u8] {
        match self {
            CheckedInput::Str(bytes, _) => bytes,
            CheckedInput::Bytes(bytes) => bytes,
            CheckedInput::Absent | CheckedInput::Invalid => &[],
        }
    }

    /// A byte span in the caller's own units, as a fault's is: bytes for a `Uint8Array`,
    /// UTF-16 code units for a string (a lone surrogate's three WTF-8 bytes are its one unit).
    fn span(&self, offset: u32, len: u32) -> (u32, u32) {
        let CheckedInput::Str(bytes, _) = self else {
            return (offset, len);
        };
        if bytes.is_ascii() {
            return (offset, len);
        }
        let start = (offset as usize).min(bytes.len());
        let end = start.saturating_add(len as usize).min(bytes.len());
        (
            utf16_units(&bytes[..start]),
            utf16_units(&bytes[start..end]),
        )
    }

    /// The text at a byte span the core reported valid: the caller's own string when the span
    /// is all of it, a new string otherwise (a trimmed span, or `Uint8Array` input).
    fn text_at(&self, env: &'env Env, offset: u32, len: u32) -> Result<Unknown<'env>> {
        if let CheckedInput::Str(bytes, string) = self
            && offset == 0
            && len as usize == bytes.len()
        {
            return Ok(string.to_unknown());
        }
        let start = offset as usize;
        let text = self
            .bytes()
            .get(start..start.saturating_add(len as usize))
            .and_then(|span| std::str::from_utf8(span).ok())
            .ok_or_else(|| binding_bug("reported a text span that is not UTF-8"))?;
        Ok(env.create_string(text)?.to_unknown())
    }
}

/// A declared `enum` entry, as the caller gave it.
enum Entry {
    Text(String),
    Integer(String),
    Big(i128),
}

/// A declaration the binding could not lay out (the binding's own contract, worded), or one
/// the core refused (its code).
enum Problem {
    Contract(String),
    Refused(i32),
}

/// A declaration as the C ABI carries it, and the caller's own `enum` entries.
struct Declared {
    door: hc::Door,
    raw: hc::RawConstraints,
    packed: Vec<u8>,
    entries: Vec<Entry>,
}

/// A bound, step or integer entry's text, through the core's conversion. A `bigint` crosses
/// as its decimal digits, which napi's `ToString` writes (no caller code runs for a primitive).
fn declared_text(
    env: &Env,
    what: &str,
    value: &Unknown<'_>,
) -> std::result::Result<Vec<u8>, Problem> {
    let unreadable = || Problem::Contract(format!("{what} could not be read"));
    match value.get_type() {
        Ok(ValueType::String) => string_wtf8(env, value).ok_or_else(unreadable),
        Ok(ValueType::BigInt) => {
            let mut text = std::ptr::null_mut();
            // SAFETY: `env` and `value` are the live call's.
            let status =
                unsafe { napi::sys::napi_coerce_to_string(env.raw(), value.raw(), &mut text) };
            if status != napi::sys::Status::napi_ok {
                clear_pending_exception(env);
                return Err(unreadable());
            }
            // SAFETY: `text` is the string napi just made.
            unsafe { Unknown::from_raw_unchecked(env.raw(), text).cast::<String>() }
                .map(String::into_bytes)
                .map_err(|_| unreadable())
        }
        Ok(ValueType::Number) => Err(Problem::Contract(format!(
            "{what} must be a string or a bigint, not a number: a number is a binary float, which holds most decimals only approximately, so write it as a string (\"0.1\") or a bigint"
        ))),
        _ => Err(Problem::Contract(format!(
            "{what} must be a string or a bigint"
        ))),
    }
}

/// The core's conversion of a declared decimal's text: the decimal, or the core's refusal.
fn convert(bytes: &[u8]) -> std::result::Result<hc::Decimal, i32> {
    let mut out = hc::Decimal {
        lo: 0,
        hi: 0,
        scale: 0,
        negative: false,
    };
    // SAFETY: `bytes` is live for the call and `out` is a live, aligned Decimal.
    match unsafe { abi::cast_declared_decimal(bytes.as_ptr(), bytes.len(), &mut out) } {
        0 => Ok(out),
        code => Err(code),
    }
}

fn declared_decimal(
    env: &Env,
    what: &str,
    value: &Unknown<'_>,
) -> std::result::Result<hc::Decimal, Problem> {
    convert(&declared_text(env, what, value)?).map_err(Problem::Refused)
}

/// A whole `u32` field (a length, a rounding code): absent is `None`; anything else is the
/// binding's contract (a `boolean` included, which is no length).
fn declared_code(
    env: &Env,
    spec: &Object<'_>,
    name: &str,
    wants: &str,
) -> std::result::Result<Option<u32>, Problem> {
    let value = spec_field(env, spec, name).map_err(Problem::Contract)?;
    match value {
        None => Ok(None),
        Some(value) if matches!(value.get_type(), Ok(ValueType::Undefined)) => Ok(None),
        Some(value) => code(&value)
            .map(Some)
            .ok_or_else(|| Problem::Contract(format!("{name} must be {wants}"))),
    }
}

/// Lays a declaration out and asks the core to judge it: the one path `Constraints.declare`
/// and `castConstraintsCheck` share, so they refuse exactly the same declarations.
fn declaration(env: &Env, spec: &Unknown<'_>) -> std::result::Result<Declared, Problem> {
    let spec = spec_object(spec, "The Constraints declaration").map_err(Problem::Contract)?;
    let door = declared_code(env, &spec, "door", "a Door")?
        .and_then(|code| hc::Door::from_code(code, 0))
        .filter(|door| door.is_numeric() || *door == hc::Door::Text)
        .ok_or_else(|| Problem::Contract(
                "door must be a Door that carries constraints: an integer, real or decimal door, or Door.Text"
                    .to_string(),
            ))?;
    let mut raw = hc::RawConstraints::ZERO;
    let bound = |name: &str| -> std::result::Result<Option<hc::Decimal>, Problem> {
        match spec_field(env, &spec, name).map_err(Problem::Contract)? {
            Some(value) if !matches!(value.get_type(), Ok(ValueType::Undefined)) => {
                declared_decimal(env, name, &value).map(Some)
            }
            _ => Ok(None),
        }
    };
    for (name, flag, upper) in [
        ("minimum", hc::RawConstraints::HAS_MINIMUM, false),
        (
            "exclusiveMinimum",
            hc::RawConstraints::HAS_EXCLUSIVE_MINIMUM,
            false,
        ),
        ("maximum", hc::RawConstraints::HAS_MAXIMUM, true),
        (
            "exclusiveMaximum",
            hc::RawConstraints::HAS_EXCLUSIVE_MAXIMUM,
            true,
        ),
    ] {
        if let Some(value) = bound(name)? {
            raw.flags |= flag;
            *(if upper {
                &mut raw.upper
            } else {
                &mut raw.lower
            }) = value;
        }
    }
    if let Some(value) = bound("multipleOf")? {
        raw.flags |= hc::RawConstraints::HAS_MULTIPLE_OF;
        raw.multiple_of = value;
    }
    // Any whole code goes to the core as it is, which refuses one it does not know.
    raw.rounding = declared_code(env, &spec, "rounding", "a Rounding")?.unwrap_or(0);
    let length = "a whole number from 0 to 4294967295";
    if let Some(value) = declared_code(env, &spec, "minLength", length)? {
        raw.flags |= hc::RawConstraints::HAS_MIN_LENGTH;
        raw.min_length = value;
    }
    if let Some(value) = declared_code(env, &spec, "maxLength", length)? {
        raw.flags |= hc::RawConstraints::HAS_MAX_LENGTH;
        raw.max_length = value;
    }
    match spec_field(env, &spec, "trim").map_err(Problem::Contract)? {
        None => {}
        Some(value) => match value.get_type() {
            Ok(ValueType::Undefined) => {}
            Ok(ValueType::Boolean) => {
                if unsafe { value.cast::<bool>() }
                    .map_err(|_| Problem::Contract("trim could not be read".to_string()))?
                {
                    raw.flags |= hc::RawConstraints::TRIM;
                }
            }
            _ => return Err(Problem::Contract("trim must be a boolean".to_string())),
        },
    }
    let mut packed = Vec::new();
    let mut entries = Vec::new();
    if let Some(list) = spec_field(env, &spec, "enum").map_err(Problem::Contract)?
        && !matches!(list.get_type(), Ok(ValueType::Undefined))
    {
        let is_array = {
            let mut is_array = false;
            // SAFETY: `env` and `list` are the live call's.
            let status = unsafe { napi::sys::napi_is_array(env.raw(), list.raw(), &mut is_array) };
            status == napi::sys::Status::napi_ok && is_array
        };
        if !is_array {
            return Err(Problem::Contract(
                "enum must be an array of entries".to_string(),
            ));
        }
        let list = unsafe { list.cast::<Vec<Unknown>>() }.map_err(|_| {
            clear_pending_exception(env);
            Problem::Contract("enum could not be read".to_string())
        })?;
        // Declared, even with no entries: the core refuses an empty `enum` (12), never reads
        // it as none.
        raw.flags |= hc::RawConstraints::HAS_ENUM;
        for (i, entry) in list.iter().enumerate() {
            if door == hc::Door::Text {
                // At the text door an entry is the text itself, packed as the ABI reads it: a
                // little-endian u32 byte length, then the bytes, a lone surrogate's kept.
                if !matches!(entry.get_type(), Ok(ValueType::String)) {
                    return Err(Problem::Contract(format!(
                        "enum entries at Door.Text must be strings; entry {i} is not"
                    )));
                }
                let bytes = string_wtf8(env, entry).ok_or_else(|| {
                    Problem::Contract(format!("enum entry {i} could not be read"))
                })?;
                let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
                packed.extend_from_slice(&len.to_le_bytes());
                packed.extend_from_slice(&bytes);
                entries.push(Entry::Text(String::from_utf8_lossy(&bytes).into_owned()));
            } else {
                // Elsewhere a Decimal in its native 16-byte layout: lo, hi, scale, sign, two
                // bytes of padding.
                let what = format!("enum entry {i}");
                let text = declared_text(env, &what, entry)?;
                let decimal = convert(&text).map_err(Problem::Refused)?;
                packed.extend_from_slice(&decimal.lo.to_ne_bytes());
                packed.extend_from_slice(&decimal.hi.to_ne_bytes());
                packed.extend_from_slice(&[decimal.scale, u8::from(decimal.negative), 0, 0]);
                entries.push(match entry.get_type() {
                    Ok(ValueType::BigInt) => Entry::Big(
                        unsafe { entry.cast::<BigInt>() }
                            .map(|big| big.get_i128().0)
                            .map_err(|_| Problem::Contract(format!("{what} could not be read")))?,
                    ),
                    _ => Entry::Integer(String::from_utf8_lossy(&text).into_owned()),
                });
            }
        }
    }
    // SAFETY: `raw` and `packed` are live for the call.
    match unsafe {
        abi::cast_constraints_check(door.code(), 0, &raw, packed.as_ptr(), packed.len())
    } {
        0 => Ok(Declared {
            door,
            raw,
            packed,
            entries,
        }),
        code => Err(Problem::Refused(code)),
    }
}

/// Validation constraints, the OpenAPI Schema Object's keywords, bound to one door and judged
/// once, by the core, when declared: a declaration that cannot mean anything at its door is a
/// refusal naming the rule it breaks, and every checked cast after that judges only data.
/// Immutable. Made by `Constraints.declare`, never by `new`.
#[napi]
pub struct Constraints {
    door: hc::Door,
    raw: hc::RawConstraints,
    packed: Vec<u8>,
    entries: Vec<Entry>,
}

impl Constraints {
    fn bound(&self, flag: u32, value: hc::Decimal) -> Option<String> {
        (self.raw.flags & flag != 0).then(|| value.to_string())
    }
}

#[napi]
impl Constraints {
    /// Declares constraints for a door, or says which rule the declaration breaks; never
    /// throws. A refusal from the core reads `Constraints refused (code N): <the rule>.`, `N`
    /// the code `castConstraintsCheck` answers; anything else is a declaration this binding
    /// cannot read (a `number` bound, a `door` that is no `Door`), worded as such.
    #[napi(
        catch_unwind,
        ts_args_type = "spec: ConstraintsSpec",
        ts_return_type = "Declaration<Constraints>"
    )]
    pub fn declare<'env>(env: &'env Env, spec: Unknown<'env>) -> Result<Object<'env>> {
        match declaration(env, &spec) {
            Ok(Declared {
                door,
                raw,
                packed,
                entries,
            }) => declared(
                env,
                Ok(Constraints {
                    door,
                    raw,
                    packed,
                    entries,
                }
                .into_instance(env)?),
            ),
            Err(Problem::Contract(message)) => declared::<bool>(env, Err(message)),
            Err(Problem::Refused(code)) => declared::<bool>(env, Err(refused_message(code))),
        }
    }

    /// The door this declaration is bound to.
    #[napi(getter, catch_unwind, ts_return_type = "Door")]
    pub fn door(&self) -> u32 {
        self.door.code()
    }

    /// The declared `minimum`, as canonical decimal text, or `null`.
    #[napi(getter, catch_unwind)]
    pub fn minimum(&self) -> Option<String> {
        self.bound(hc::RawConstraints::HAS_MINIMUM, self.raw.lower)
    }

    /// The declared `exclusiveMinimum`, as canonical decimal text, or `null`.
    #[napi(getter, catch_unwind)]
    pub fn exclusive_minimum(&self) -> Option<String> {
        self.bound(hc::RawConstraints::HAS_EXCLUSIVE_MINIMUM, self.raw.lower)
    }

    /// The declared `maximum`, as canonical decimal text, or `null`.
    #[napi(getter, catch_unwind)]
    pub fn maximum(&self) -> Option<String> {
        self.bound(hc::RawConstraints::HAS_MAXIMUM, self.raw.upper)
    }

    /// The declared `exclusiveMaximum`, as canonical decimal text, or `null`.
    #[napi(getter, catch_unwind)]
    pub fn exclusive_maximum(&self) -> Option<String> {
        self.bound(hc::RawConstraints::HAS_EXCLUSIVE_MAXIMUM, self.raw.upper)
    }

    /// The declared `multipleOf`, as canonical decimal text, or `null`.
    #[napi(getter, catch_unwind)]
    pub fn multiple_of(&self) -> Option<String> {
        self.bound(hc::RawConstraints::HAS_MULTIPLE_OF, self.raw.multiple_of)
    }

    /// What a value that is not a multiple of `multipleOf` becomes.
    #[napi(getter, catch_unwind, ts_return_type = "Rounding")]
    pub fn rounding(&self) -> u32 {
        self.raw.rounding
    }

    /// The declared `minLength`, in code points, or `null`.
    #[napi(getter, catch_unwind)]
    pub fn min_length(&self) -> Option<u32> {
        (self.raw.flags & hc::RawConstraints::HAS_MIN_LENGTH != 0).then_some(self.raw.min_length)
    }

    /// The declared `maxLength`, in code points, or `null`.
    #[napi(getter, catch_unwind)]
    pub fn max_length(&self) -> Option<u32> {
        (self.raw.flags & hc::RawConstraints::HAS_MAX_LENGTH != 0).then_some(self.raw.max_length)
    }

    /// Whether text is trimmed of Unicode `White_Space` before it is judged.
    #[napi(getter, catch_unwind)]
    pub fn trim(&self) -> bool {
        self.raw.flags & hc::RawConstraints::TRIM != 0
    }

    /// The declared `enum` entries as the caller gave them (strings, or `bigint`s at an
    /// integer door), or `null`.
    #[napi(getter, catch_unwind, js_name = "enum")]
    pub fn entries(&self) -> Option<Vec<Either<String, BigInt>>> {
        if self.raw.flags & hc::RawConstraints::HAS_ENUM == 0 {
            return None;
        }
        Some(
            self.entries
                .iter()
                .map(|entry| match entry {
                    Entry::Text(text) | Entry::Integer(text) => Either::A(text.clone()),
                    Entry::Big(value) => Either::B(BigInt::from(*value)),
                })
                .collect(),
        )
    }

    /// The 72 bytes this declaration crosses a C ABI as, in the native byte order the core
    /// reads in place: the size (`72`), the flags, the rounding code, `minLength`, `maxLength`
    /// and a reserved zero as `u32`s, then the lower bound, the upper bound and `multipleOf` as
    /// 16-byte decimals (the low 64 and high 32 bits of the magnitude, the scale, the sign, two
    /// zero bytes): for a library that judges values across a C ABI of its own, with
    /// `packedEnum` beside it.
    #[napi(getter, catch_unwind)]
    pub fn packed(&self) -> Uint8Array {
        let raw = &self.raw;
        let mut bytes = Vec::with_capacity(hc::RawConstraints::SIZE as usize);
        for word in [
            raw.size,
            raw.flags,
            raw.rounding,
            raw.min_length,
            raw.max_length,
            raw._reserved,
        ] {
            bytes.extend_from_slice(&word.to_ne_bytes());
        }
        for decimal in [raw.lower, raw.upper, raw.multiple_of] {
            bytes.extend_from_slice(&decimal.lo.to_ne_bytes());
            bytes.extend_from_slice(&decimal.hi.to_ne_bytes());
            bytes.extend_from_slice(&[decimal.scale, u8::from(decimal.negative), 0, 0]);
        }
        Uint8Array::from(bytes)
    }

    /// The `enum` list as the core reads it beside `packed`, empty for none: at an integer door
    /// 16-byte decimals in native byte order, at `Door.Text` each entry a little-endian `u32`
    /// byte length followed by its UTF-8.
    #[napi(getter, catch_unwind)]
    pub fn packed_enum(&self) -> Uint8Array {
        Uint8Array::from(self.packed.clone())
    }
}

/// The `Constraints` a checked door was handed, or `None` for anything else.
fn constraints_arg<'a>(value: &'a Unknown<'_>) -> Option<ClassInstance<'a, Constraints>> {
    match value.get_type() {
        Ok(ValueType::Object) => unsafe { value.cast::<ClassInstance<Constraints>>() }.ok(),
        _ => None,
    }
}

/// What a numeric door wrote, as JavaScript holds it.
enum Delivered {
    Int(i32),
    Uint(u32),
    Big(BigInt),
    Real(f64),
    Text(String),
}

/// Reads the value a numeric door wrote to `slot`.
///
/// # Safety
/// The core reported success at `door`, so `slot` holds that door's type.
unsafe fn delivered(door: hc::Door, slot: &[u64; 2]) -> Option<Delivered> {
    let at = slot.as_ptr();
    // SAFETY: per the function contract; `slot` is 16 bytes, 8-aligned.
    Some(unsafe {
        match door {
            hc::Door::I8 => Delivered::Int(i32::from(at.cast::<i8>().read())),
            hc::Door::I16 => Delivered::Int(i32::from(at.cast::<i16>().read())),
            hc::Door::I32 => Delivered::Int(at.cast::<i32>().read()),
            hc::Door::I64 => Delivered::Big(BigInt::from(at.cast::<i64>().read())),
            hc::Door::U8 => Delivered::Uint(u32::from(at.cast::<u8>().read())),
            hc::Door::U16 => Delivered::Uint(u32::from(at.cast::<u16>().read())),
            hc::Door::U32 => Delivered::Uint(at.cast::<u32>().read()),
            hc::Door::U64 => Delivered::Big(BigInt::from(at.cast::<u64>().read())),
            hc::Door::F32 => Delivered::Real(f64::from(at.cast::<f32>().read())),
            hc::Door::F64 => Delivered::Real(at.cast::<f64>().read()),
            hc::Door::Decimal => Delivered::Text(at.cast::<hc::Decimal>().read().to_string()),
            _ => return None,
        }
    })
}

fn checked_success<'env, V: ToNapiValue>(
    env: &'env Env,
    value: V,
    check: &hc::RawCheck,
) -> Result<Object<'env>> {
    let mut verdict = Object::new(env)?;
    verdict.set("ok", true)?;
    verdict.set("value", value)?;
    verdict.set("adjusted", check.adjusted != 0)?;
    verdict.set(
        "enumIndex",
        (check.enum_index != u32::MAX).then_some(check.enum_index),
    )?;
    Ok(verdict)
}

fn violated(env: &Env, keyword: u32, (offset, length): (u32, u32)) -> Result<Object<'_>> {
    let mut verdict = Object::new(env)?;
    verdict.set("ok", false)?;
    verdict.set(
        "violation",
        Violation {
            keyword: keyword_name(keyword),
            offset,
            length,
        },
    )?;
    Ok(verdict)
}

/// A rejection from a checked export's return code, its span in the caller's units.
fn rejected<'env>(
    env: &'env Env,
    code: i32,
    span: (u32, u32),
    check: &hc::RawCheck,
) -> Result<Object<'env>> {
    let reason = match code {
        1 => hc::Reason::Empty,
        2 => hc::Reason::Malformed,
        3 => hc::Reason::OutOfRange,
        4 => return violated(env, check.keyword, span),
        _ => return Err(binding_bug("refused a declaration it had accepted")),
    };
    failure(
        env,
        hc::Fault {
            reason,
            offset: span.0,
            len: span.1,
        },
    )
}

/// A numeric checked door's success: the value the door wrote, as JavaScript holds it.
fn numeric_success<'env>(
    env: &'env Env,
    door: hc::Door,
    slot: &[u64; 2],
    check: &hc::RawCheck,
) -> Result<Object<'env>> {
    // SAFETY: the core reported success at `door`.
    match unsafe { delivered(door, slot) } {
        Some(Delivered::Int(value)) => checked_success(env, value, check),
        Some(Delivered::Uint(value)) => checked_success(env, value, check),
        Some(Delivered::Big(value)) => checked_success(env, value, check),
        Some(Delivered::Real(value)) => checked_success(env, value, check),
        Some(Delivered::Text(value)) => checked_success(env, value, check),
        None => Err(binding_bug("judged a door that is not numeric")),
    }
}

const NO_CHECK: hc::RawCheck = hc::RawCheck {
    keyword: 0,
    adjusted: 0,
    enum_index: u32::MAX,
};

fn empty_fault() -> hc::RawFaultDetail {
    hc::RawFaultDetail {
        size: hc::RawFaultDetail::SIZE,
        offset: 0,
        len: 0,
        detail: 0,
    }
}

/// Which doors a checked entry point takes: `castChecked` the numeric ones, `castText` text,
/// `castCheckedDetailed` either.
#[derive(Clone, Copy, PartialEq)]
enum CheckedDoors {
    Numeric,
    Text,
    Either,
}

impl CheckedDoors {
    fn takes(self, door: hc::Door) -> bool {
        match self {
            CheckedDoors::Numeric => door != hc::Door::Text,
            CheckedDoors::Text => door == hc::Door::Text,
            CheckedDoors::Either => true,
        }
    }
}

/// A checked text door's verdict: the text read by the core's detailed checked export (which
/// reads the format's grouping policy, as the 1.0 export would not), judged by `constraints`.
/// `detailed` chooses the family a failure is presented in. `format` is the numeric doors'
/// and is never read at `Door.Text`.
fn checked_text_verdict<'env>(
    env: &'env Env,
    input: &CheckedInput<'env>,
    constraints: &Constraints,
    format: Option<&hc::RawNumFormat>,
    detailed: bool,
) -> Result<Object<'env>> {
    let door = constraints.door;
    let bytes = input.bytes();
    let mut slot = [0u64; 2];
    let mut fault = empty_fault();
    let mut check = NO_CHECK;
    // SAFETY: every pointer is live for the call, and `slot` holds any door's value, a text
    // door's `RawSpan` included.
    let code = unsafe {
        abi::cast_checked_detailed(
            door.code(),
            0,
            format.map_or(std::ptr::null(), std::ptr::from_ref),
            &constraints.raw,
            constraints.packed.as_ptr(),
            constraints.packed.len(),
            bytes.as_ptr(),
            bytes.len(),
            slot.as_mut_ptr().cast(),
            &mut fault,
            &mut check,
        )
    };
    let span = || input.span(fault.offset, fault.len);
    match code {
        0 if door == hc::Door::Text => {
            // SAFETY: the core reported success at the text door, which writes a `RawSpan`.
            let found = unsafe { slot.as_ptr().cast::<hc::RawSpan>().read() };
            if check.enum_index == u32::MAX {
                return checked_success(env, input.text_at(env, found.offset, found.len)?, &check);
            }
            match constraints.entries.get(check.enum_index as usize) {
                Some(Entry::Text(entry)) => checked_success(env, entry.as_str(), &check),
                _ => Err(binding_bug("matched an enum entry never declared")),
            }
        }
        0 => numeric_success(env, door, &slot, &check),
        4 => violated(env, check.keyword, span()),
        code => match reason_of(code) {
            Some(reason) => fault_verdict(env, detailed, reason, span(), fault.detail),
            None => Err(binding_bug("refused a declaration it had accepted")),
        },
    }
}

/// A checked entry point over text: reads the arguments, then judges through
/// [`checked_text_verdict`]. Anything the contract rules out is a `malformed` verdict with an
/// empty span at 0, in the family the entry point answers in.
fn checked_text<'env>(
    env: &'env Env,
    text: &Unknown<'env>,
    constraints: &Unknown<'env>,
    format: &Unknown<'env>,
    doors: CheckedDoors,
    detailed: bool,
) -> Result<Object<'env>> {
    let Some(constraints) = constraints_arg(constraints).filter(|c| doors.takes(c.door)) else {
        return contract_verdict(env, detailed);
    };
    let format = if constraints.door == hc::Door::Text {
        None
    } else {
        match num_format(format) {
            Some(format) => Some(hc::RawNumFormat::from(format)),
            None => return contract_verdict(env, detailed),
        }
    };
    let input = CheckedInput::read(env, text);
    match input {
        CheckedInput::Absent => return absent_verdict(env, detailed),
        CheckedInput::Invalid => return contract_verdict(env, detailed),
        CheckedInput::Str(..) | CheckedInput::Bytes(_) => {}
    }
    checked_text_verdict(env, &input, &constraints, format.as_ref(), detailed)
}

/// A checked entry point over a number a source already decoded.
fn checked_number<'env>(
    env: &'env Env,
    value: &Unknown<'env>,
    constraints: &Unknown<'env>,
    detailed: bool,
) -> Result<Object<'env>> {
    let (Some(constraints), Some(value)) = (constraints_arg(constraints), number(value)) else {
        return contract_verdict(env, detailed);
    };
    let door = constraints.door;
    if door == hc::Door::Text {
        return contract_verdict(env, detailed);
    }
    let mut slot = [0u64; 2];
    let mut fault = empty_fault();
    let mut check = NO_CHECK;
    // SAFETY: every pointer is live for the call, and `slot` holds any numeric door's type.
    let code = unsafe {
        abi::cast_checked_detailed_from_f64(
            door.code(),
            0,
            &constraints.raw,
            constraints.packed.as_ptr(),
            constraints.packed.len(),
            value,
            slot.as_mut_ptr().cast(),
            &mut fault,
            &mut check,
        )
    };
    let span = (fault.offset, fault.len);
    match code {
        0 => numeric_success(env, door, &slot, &check),
        4 => violated(env, check.keyword, span),
        code => match reason_of(code) {
            Some(reason) => fault_verdict(env, detailed, reason, span, fault.detail),
            None => Err(binding_bug("refused a declaration it had accepted")),
        },
    }
}

/// Casts numeric text through the door its `Constraints` are bound to, under the declared
/// format (absent is the invariant one), then judges the value by them: `Checked<T>`, the
/// door's own `Failure`, or `Violated` naming the first rule it breaks. The value is judged
/// exactly, on the decimal the door read, never on a float. The value's type follows the
/// door: `number` for `I8` to `U32`, `F32` and `F64`; `bigint` for `I64` and `U64`; canonical
/// decimal text for `Decimal`. Spans are in the caller's units, as a fault's are.
/// `Constraints` for `Door.Text`, or anything that is no `Constraints` (`undefined` and `null`
/// included: a failed declaration's missing `.value`), is a contract violation: `malformed`
/// with an empty span at 0.
#[napi(
    catch_unwind,
    ts_args_type = "text: Text, constraints: Constraints, format?: NumFormat | undefined | null",
    ts_return_type = "CheckedVerdict<number | bigint | string>"
)]
pub fn cast_checked<'env>(
    env: &'env Env,
    text: Unknown<'env>,
    constraints: Unknown<'env>,
    format: Unknown<'env>,
) -> Result<Object<'env>> {
    checked_text(
        env,
        &text,
        &constraints,
        &format,
        CheckedDoors::Numeric,
        false,
    )
}

/// Reads a number a source already decoded (a JSON parser's, a spreadsheet's numeric cell)
/// through the door its `Constraints` are bound to, then judges it by them on the shortest
/// decimal that names it: `0.1 + 0.2` is `0.30000000000000004`, which breaks
/// `multipleOf: "0.1"` unrounded and rounds to `0.3` under `Rounding.Midpoint`. NaN is
/// `malformed`; an infinity is `outOfRange` at a float or decimal door and `malformed` at an
/// integer door. Every fault and violation has an empty span.
/// `-0` is zero, delivered at a float door as `0`.
#[napi(
    catch_unwind,
    ts_args_type = "value: number, constraints: Constraints",
    ts_return_type = "CheckedVerdict<number | bigint | string>"
)]
pub fn cast_checked_from_number<'env>(
    env: &'env Env,
    value: Unknown<'env>,
    constraints: Unknown<'env>,
) -> Result<Object<'env>> {
    checked_number(env, &value, &constraints, false)
}

/// Casts text as text: the input itself, once it is valid text (a `Uint8Array` must be UTF-8,
/// and a string holding a lone surrogate is `malformed` at it), as a `Verdict<string>`. Empty
/// input is `empty`; nothing is trimmed. With `Constraints` for `Door.Text` it is judged by
/// them, a `CheckedVerdict<string>`: trimmed of Unicode `White_Space` when they say so
/// (trimmed to nothing is `empty`), lengths counted in code points, and an `enum` matched under
/// simple case folding, a match delivering the caller's own declared entry and its index.
/// `Constraints` for another door, or anything that is no `Constraints`, is a contract
/// violation: `malformed` with an empty span at 0. Plain or constrained is chosen by the number
/// of arguments, not by the second one's value, so an explicit `undefined` or `null`
/// declaration (a failed `Constraints.declare`'s missing `.value`) is that contract violation
/// too, never "no rules".
#[napi(
    catch_unwind,
    ts_args_type = "text: Text, constraints?: Constraints",
    ts_return_type = "Verdict<string> | CheckedVerdict<string>"
)]
pub fn cast_text<'env>(
    env: &'env Env,
    text: Unknown<'env>,
    constraints: Unknown<'env>,
) -> Result<Object<'env>> {
    if !matches!(
        constraints.get_type(),
        Ok(ValueType::Undefined | ValueType::Null)
    ) {
        return checked_text(
            env,
            &text,
            &constraints,
            &constraints,
            CheckedDoors::Text,
            false,
        );
    }
    let input = CheckedInput::read(env, &text);
    match input {
        CheckedInput::Absent => return failure(env, ABSENT),
        CheckedInput::Invalid => return contract(env),
        CheckedInput::Str(..) | CheckedInput::Bytes(_) => {}
    }
    let bytes = input.bytes();
    let mut span = hc::RawSpan { offset: 0, len: 0 };
    let mut fault = RawFault::default();
    // SAFETY: every pointer is live for the call.
    match unsafe { abi::cast_text(bytes.as_ptr(), bytes.len(), &mut span, &mut fault) } {
        0 => success(env, input.text_at(env, span.offset, span.len)?),
        code => rejected(env, code, input.span(fault.offset, fault.len), &NO_CHECK),
    }
}

/// Replaces the exported `castText` with one that reads its argument count, which a `#[napi]`
/// function cannot see (napi hands it `undefined` for a missing argument): one argument is the
/// plain door; two or more are the checked door, where the second must be `Constraints`, so an
/// explicit `undefined` or `null` (a failed `Constraints.declare`'s missing `.value`) is the
/// contract violation and never quietly "no rules". It runs once the exports are registered,
/// on every entry alike: Node's addon, and the WebAssembly fallback under Node or in a
/// browser. The installed function's own calls catch unwinds in napi's trampoline; this body
/// catches its own.
#[napi(module_exports)]
pub fn cast_text_by_arity(mut exports: Object<'_>, env: Env) -> Result<()> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let by_arity =
            env.create_function_from_closure::<(), napi::sys::napi_value, _>("castText", |ctx| {
                let env: &Env = ctx.env;
                let raw = env.raw();
                let verdict = if ctx.length() < 2 {
                    let text = if ctx.length() == 0 {
                        // SAFETY: `()` is `undefined` in the live call's env.
                        unsafe {
                            Unknown::from_raw_unchecked(raw, ToNapiValue::to_napi_value(raw, ())?)
                        }
                    } else {
                        ctx.get::<Unknown<'_>>(0)?
                    };
                    // SAFETY: as above.
                    let none = unsafe {
                        Unknown::from_raw_unchecked(raw, ToNapiValue::to_napi_value(raw, ())?)
                    };
                    cast_text(env, text, none)?
                } else {
                    let constraints = ctx.get::<Unknown<'_>>(1)?;
                    if matches!(
                        constraints.get_type(),
                        Ok(ValueType::Undefined | ValueType::Null)
                    ) {
                        contract(env)?
                    } else {
                        cast_text(env, ctx.get::<Unknown<'_>>(0)?, constraints)?
                    }
                };
                Ok(verdict.raw())
            })?;
        exports.set_named_property("castText", by_arity)
    }))
    .unwrap_or_else(|_| Err(binding_bug("panicked installing castText")))
}

// ---------------------------------------------------------------------------------------
// The detailed doors, and the date doors that take options.
//
// Every text door is one call of the core's `cast_detailed`, told which door by its code and
// what the door declares by `param` (a precision, an order, an epoch) and the layouts it
// reads: the format, the day periods, the date options and the month names. The plain date
// doors with options and the four detailed doors share this one path, and differ only in
// whether a failure is presented with its detail.
// ---------------------------------------------------------------------------------------

/// The date options and month names a date door was given.
#[derive(Default)]
struct Dates {
    options: Option<hc::RawDateOptions>,
    months: Vec<u8>,
}

/// An `options` argument: absent, a `DateOptions`, or a `MonthNames` alone (which opens month
/// names and nothing else). `None` for anything else, a contract violation.
fn dates_arg(value: &Unknown<'_>) -> Option<Dates> {
    match value.get_type() {
        Ok(ValueType::Undefined | ValueType::Null) => Some(Dates::default()),
        Ok(ValueType::Object) => {
            if let Ok(options) = unsafe { value.cast::<ClassInstance<DateOptions>>() } {
                return Some(Dates {
                    options: Some(hc::RawDateOptions {
                        size: hc::RawDateOptions::SIZE,
                        flags: options.flags,
                        pivot: options.pivot,
                    }),
                    months: options
                        .months
                        .as_ref()
                        .map(|names| names.packed.clone())
                        .unwrap_or_default(),
                });
            }
            unsafe { value.cast::<ClassInstance<MonthNames>>() }
                .ok()
                .map(|names| Dates {
                    options: None,
                    months: names.packed.clone(),
                })
        }
        _ => None,
    }
}

/// One call of `cast_detailed`: the door and what it reads.
struct Call {
    door: hc::Door,
    param: u32,
    format: Option<hc::RawNumFormat>,
    periods: Option<hc::RawDayPeriods>,
    dates: Dates,
}

impl Call {
    fn of(door: hc::Door, param: u32) -> Option<Call> {
        Some(Call {
            door: hc::Door::from_code(door.code(), param)?,
            param,
            format: None,
            periods: None,
            dates: Dates::default(),
        })
    }
}

fn is_nothing(value: &Unknown<'_>) -> bool {
    matches!(value.get_type(), Ok(ValueType::Undefined | ValueType::Null))
}

/// The date-time door's call: a `DateOrder`, or a `DateFormat` whose day periods ride with it.
fn datetime_call(declared: &Unknown<'_>) -> Option<Call> {
    let datetime = |param: u32| Call::of(hc::Door::from_code(21, param)?, param);
    match declared.get_type() {
        Ok(ValueType::Number) => datetime(code(declared)?),
        Ok(ValueType::Object) => {
            let format = unsafe { declared.cast::<ClassInstance<DateFormat>>() }.ok()?;
            let mut call = datetime(format.order as u32)?;
            call.periods = Some(hc::RawDayPeriods::from(format.periods));
            Some(call)
        }
        _ => None,
    }
}

/// The call `castDetailed(door, text, declared, options)` makes, or `None` for a contract
/// violation: a number that names no door, a declaration the door does not read (or none
/// where it needs one), or options that are neither `DateOptions` nor `MonthNames`. The
/// numeric doors read a `NumFormat`, absent is `NumFormat.invariant()`.
fn detailed_call(
    door: &Unknown<'_>,
    declared: &Unknown<'_>,
    options: &Unknown<'_>,
) -> Option<Call> {
    let code = code(door)?;
    let mut call = match code {
        2..=11 | 19 => {
            let mut call = Call::of(hc::Door::from_code(code, 0)?, 0)?;
            call.format = Some(hc::RawNumFormat::from(num_format(declared)?));
            call
        }
        14 | 20 | 22 => {
            let param = self::code(declared)?;
            Call::of(hc::Door::from_code(code, param)?, param)?
        }
        21 => datetime_call(declared)?,
        1 | 12 | 13 | 15 | 16 | 17 | 18 | 23 => {
            if !is_nothing(declared) {
                return None;
            }
            Call::of(hc::Door::from_code(code, 0)?, 0)?
        }
        _ => return None,
    };
    if matches!(code, 13 | 15 | 20 | 21) {
        call.dates = dates_arg(options)?;
    }
    Some(call)
}

/// What the core made of a call.
enum Issued {
    Value([u64; 2]),
    Fault(hc::Reason, hc::RawFaultDetail),
    /// A call the core refused (-1): the binding validated less than the core does.
    Refused,
}

fn issue(call: &Call, bytes: &[u8]) -> Issued {
    let mut slot = [0u64; 2];
    let mut fault = empty_fault();
    let months = &call.dates.months;
    // SAFETY: every pointer is live for the call (a pointer the door ignores is never read,
    // and a null one is no declaration), `slot` is 16 bytes, which holds any door's value.
    let code = unsafe {
        abi::cast_detailed(
            call.door.code(),
            call.param,
            call.format
                .as_ref()
                .map_or(std::ptr::null(), std::ptr::from_ref),
            call.periods
                .as_ref()
                .map_or(std::ptr::null(), std::ptr::from_ref),
            call.dates
                .options
                .as_ref()
                .map_or(std::ptr::null(), std::ptr::from_ref),
            months.as_ptr(),
            months.len(),
            bytes.as_ptr(),
            bytes.len(),
            slot.as_mut_ptr().cast(),
            &mut fault,
        )
    };
    match (code, reason_of(code)) {
        (0, _) => Issued::Value(slot),
        (_, Some(reason)) => Issued::Fault(reason, fault),
        _ => Issued::Refused,
    }
}

/// A text door's success: the value the door wrote, as JavaScript holds it.
fn value_verdict<'env>(
    env: &'env Env,
    door: hc::Door,
    slot: &[u64; 2],
    input: &CheckedInput<'env>,
) -> Result<Object<'env>> {
    let at = slot.as_ptr();
    // SAFETY: the core reported success at `door`, so `slot` holds that door's type, written
    // at its start; `slot` is 16 bytes, 8-aligned, and no type is larger or stricter.
    unsafe {
        match door {
            hc::Door::Bool => success(env, at.cast::<u8>().read() != 0),
            hc::Door::Char => match char::from_u32(at.cast::<u32>().read()) {
                Some(c) => success(env, String::from(c)),
                None => Err(binding_bug("read a character that is no scalar value")),
            },
            hc::Door::Uuid => success(env, uuid_text(at.cast::<[u8; 16]>().read())),
            hc::Door::Timestamp | hc::Door::Unix(_) | hc::Door::ExcelSerial(_) => {
                success(env, timestamp(at.cast::<hc::Timestamp>().read()))
            }
            hc::Door::Date | hc::Door::DateOrdered(_) => {
                success(env, calendar(at.cast::<hc::Date>().read()))
            }
            hc::Door::Time => success(env, time_of_day(at.cast::<u64>().read())),
            hc::Door::Duration => success(env, duration(at.cast::<hc::Duration>().read())),
            hc::Door::DateTime(_) => success(env, civil(at.cast::<hc::CivilDateTime>().read())),
            hc::Door::Text => {
                let found = at.cast::<hc::RawSpan>().read();
                success(env, input.text_at(env, found.offset, found.len)?)
            }
            _ => match delivered(door, slot) {
                Some(Delivered::Int(value)) => success(env, value),
                Some(Delivered::Uint(value)) => success(env, value),
                Some(Delivered::Big(value)) => success(env, value),
                Some(Delivered::Real(value)) => success(env, value),
                Some(Delivered::Text(value)) => success(env, value),
                None => Err(binding_bug("read a door it does not know")),
            },
        }
    }
}

/// Runs a text door: `call` is `None` for a contract violation. `detailed` chooses the family
/// a failure is presented in.
fn run<'env>(
    env: &'env Env,
    text: &Unknown<'env>,
    call: Option<Call>,
    detailed: bool,
) -> Result<Object<'env>> {
    let Some(call) = call else {
        return contract_verdict(env, detailed);
    };
    let input = CheckedInput::read(env, text);
    match input {
        CheckedInput::Absent => return absent_verdict(env, detailed),
        CheckedInput::Invalid => return contract_verdict(env, detailed),
        CheckedInput::Str(..) | CheckedInput::Bytes(_) => {}
    }
    match issue(&call, input.bytes()) {
        Issued::Value(slot) => value_verdict(env, call.door, &slot, &input),
        Issued::Fault(reason, fault) => fault_verdict(
            env,
            detailed,
            reason,
            input.span(fault.offset, fault.len),
            fault.detail,
        ),
        Issued::Refused => Err(binding_bug("refused a declaration it had accepted")),
    }
}

/// The call `castDetailedFromNumber(door, value, declared)` makes: the four doors that read a
/// number, as the plain typed doors do, or `None` for a contract violation.
fn number_call(door: &Unknown<'_>, declared: &Unknown<'_>) -> Option<(hc::Door, u32)> {
    let code = code(door)?;
    let param = match code {
        22 => self::code(declared)?,
        16 | 17 | 19 if is_nothing(declared) => 0,
        _ => return None,
    };
    hc::Door::from_code(code, param).map(|door| (door, param))
}

/// A typed door's success: the value the door wrote from a number, as JavaScript holds it.
fn number_verdict<'env>(env: &'env Env, door: hc::Door, slot: &[u64; 2]) -> Result<Object<'env>> {
    let at = slot.as_ptr();
    // SAFETY: as `value_verdict`: the core reported success at `door`.
    unsafe {
        match door {
            hc::Door::Decimal => success(env, at.cast::<hc::Decimal>().read().to_string()),
            hc::Door::ExcelSerial(_) => success(env, civil(at.cast::<hc::CivilDateTime>().read())),
            hc::Door::Time => success(env, time_of_day(at.cast::<u64>().read())),
            hc::Door::Duration => success(env, duration(at.cast::<hc::Duration>().read())),
            _ => Err(binding_bug("read a door that reads no number")),
        }
    }
}

/// Casts text through any door the core's detailed export names, and says what failed: a
/// failure is a `DetailedFault`, the plain door's `reason`, `offset` and `length` with a named
/// `detail` and its raw `detailCode`. The value is what the door's own function answers
/// (`Door.Unix` a `Timestamp`, `Door.Char` a string, `Door.Text` the text itself).
///
/// `declared` is what the door declares: a `NumFormat` at the integer, real and decimal doors
/// (absent is `NumFormat.invariant()`), a `UnixPrecision` at `Door.Unix`, a `DateOrder` at
/// `Door.DateOrdered`, a `DateOrder` or `DateFormat` at `Door.DateTime`, an `ExcelEpoch` at
/// `Door.ExcelSerial`, and nothing anywhere else. `options` are read by `Door.Timestamp`,
/// `Door.Date`, `Door.DateOrdered` and `Door.DateTime` only (a `DateOptions`, or a
/// `MonthNames` alone for names and nothing else) and ignored by every other door.
/// A door that is no `Door` member, a declaration the door does not take, or options that are
/// neither `DateOptions` nor `MonthNames` is a contract violation: `malformed` with an empty
/// span at 0 and no detail (`'none'`, code 0). Absent text is `empty`, detail `blank`.
#[napi(
    catch_unwind,
    ts_args_type = "door: Door, text: Text, declared?: NumFormat | DateFormat | UnixPrecision | DateOrder | ExcelEpoch | undefined | null, options?: DateOptions | MonthNames | undefined | null",
    ts_return_type = "DetailedVerdict<boolean | number | bigint | string | Timestamp | CalendarDate | TimeOfDay | CivilDateTime | Duration>"
)]
pub fn cast_detailed<'env>(
    env: &'env Env,
    door: Unknown<'env>,
    text: Unknown<'env>,
    declared: Unknown<'env>,
    options: Unknown<'env>,
) -> Result<Object<'env>> {
    let call = detailed_call(&door, &declared, &options);
    run(env, &text, call, true)
}

/// Reads a number a source already decoded through one of the four doors that read one,
/// `Door.Decimal`, `Door.ExcelSerial` (`declared` an `ExcelEpoch`, answering a
/// `CivilDateTime`), `Door.Time` and `Door.Duration`, and says what failed. A failure has no
/// text to index, so its span is empty.
#[napi(
    catch_unwind,
    ts_args_type = "door: Door, value: number, declared?: ExcelEpoch | undefined | null",
    ts_return_type = "DetailedVerdict<string | CivilDateTime | TimeOfDay | Duration>"
)]
pub fn cast_detailed_from_number<'env>(
    env: &'env Env,
    door: Unknown<'env>,
    value: Unknown<'env>,
    declared: Unknown<'env>,
) -> Result<Object<'env>> {
    let (Some((door, param)), Some(value)) = (number_call(&door, &declared), number(&value)) else {
        return contract_verdict(env, true);
    };
    let mut slot = [0u64; 2];
    let mut fault = empty_fault();
    // SAFETY: every pointer is live for the call, and `slot` holds any of the four doors' types.
    let code = unsafe {
        abi::cast_detailed_from_f64(
            door.code(),
            param,
            value,
            slot.as_mut_ptr().cast(),
            &mut fault,
        )
    };
    match (code, reason_of(code)) {
        (0, _) => number_verdict(env, door, &slot),
        (_, Some(reason)) => detailed_failure(env, reason, (fault.offset, fault.len), fault.detail),
        _ => Err(binding_bug("refused a declaration it had accepted")),
    }
}

/// Casts text through the door its `Constraints` are bound to, `Door.Text` included, and
/// judges the value by them, saying what failed: a `DetailedCheckedVerdict`. A failure to read
/// is a `DetailedFault`; a value that broke a rule is `Violated`, as at `castChecked`, with no
/// detail (a violation names the rule, not a failure to read). `format` is read by the numeric
/// doors (absent is `NumFormat.invariant()`) and ignored at `Door.Text`.
#[napi(
    catch_unwind,
    ts_args_type = "text: Text, constraints: Constraints, format?: NumFormat | undefined | null",
    ts_return_type = "DetailedCheckedVerdict<number | bigint | string>"
)]
pub fn cast_checked_detailed<'env>(
    env: &'env Env,
    text: Unknown<'env>,
    constraints: Unknown<'env>,
    format: Unknown<'env>,
) -> Result<Object<'env>> {
    checked_text(
        env,
        &text,
        &constraints,
        &format,
        CheckedDoors::Either,
        true,
    )
}

/// Reads a number a source already decoded through the door its `Constraints` are bound to,
/// as `castCheckedFromNumber` does, and says what failed. Every span is empty.
#[napi(
    catch_unwind,
    ts_args_type = "value: number, constraints: Constraints",
    ts_return_type = "DetailedCheckedVerdict<number | bigint | string>"
)]
pub fn cast_checked_detailed_from_number<'env>(
    env: &'env Env,
    value: Unknown<'env>,
    constraints: Unknown<'env>,
) -> Result<Object<'env>> {
    checked_number(env, &value, &constraints, true)
}

/// Judges a declaration without building it, the way `Constraints.declare` does: `0` when every
/// rule holds, otherwise the code of the first rule it breaks (`1` a keyword the door does not
/// take, `2` minimum with exclusiveMinimum, `3` maximum with exclusiveMaximum, `4` a lower
/// bound not below the upper, `5` multipleOf not positive, `6` a rounding without multipleOf,
/// `7` minLength above maxLength, `8` a bound, step or entry the door cannot hold, `9` an entry
/// that is not UTF-8 (a lone surrogate), `10` a duplicate entry, `11` a rounding code the core
/// does not know, `12` an `enum` with no entries (`enum: []`; leaving `enum` out declares
/// none), `13` a text entry no input can match: an empty one, or under `trim` one with leading
/// or trailing white space). `-1` is a declaration this binding cannot read, whose wording
/// `Constraints.declare` gives.
#[napi(catch_unwind, ts_args_type = "spec: ConstraintsSpec")]
pub fn cast_constraints_check(env: &Env, spec: Unknown<'_>) -> i32 {
    match declaration(env, &spec) {
        Ok(_) => 0,
        Err(Problem::Refused(code)) => code,
        Err(Problem::Contract(_)) => -1,
    }
}

/// Converts a bound, step or integer `enum` entry written in invariant notation (an optional
/// sign, digits with an optional `.` and fraction, an optional exponent: `"0.000001"`, `"-90"`,
/// `"1e-6"`) to the exact decimal the core declares it as, as canonical text with its trailing
/// zeros dropped (`"1.50"` is `"1.5"`): the conversion every bound a `Constraints` is given
/// crosses. A value past 96 bits or 28 places, or text in any other notation, is refused as a
/// declaration is (code 8).
#[napi(
    catch_unwind,
    ts_args_type = "text: Text",
    ts_return_type = "Declaration<string>"
)]
pub fn cast_declared_decimal<'env>(env: &'env Env, text: Unknown<'env>) -> Result<Object<'env>> {
    let input = CheckedInput::read(env, &text);
    let outcome = match input {
        CheckedInput::Str(..) | CheckedInput::Bytes(_) => convert(input.bytes())
            .map(|decimal| decimal.to_string())
            .map_err(refused_message),
        CheckedInput::Absent | CheckedInput::Invalid => {
            clear_pending_exception(env);
            Err("text must be a string or a Uint8Array".to_string())
        }
    };
    declared(env, outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A format the per-field validators never saw: built from the core's public fields, with
    /// a white-space decimal separator, so only the backstop can refuse it. It must refuse in a
    /// release build too (`cargo test --release`), which a `debug_assert!` would not.
    #[test]
    fn the_backstop_refuses_a_format_the_core_refuses() {
        let unseen = hc::NumFormat {
            decimal_sep: hc::Symbol::new("\u{a0}").unwrap(),
            ..hc::NumFormat::INVARIANT
        };
        assert!(!unseen.check());
        let refusal = refuse_unless_checked(&unseen).unwrap_err();
        assert!(refusal.contains("a format the core refuses"), "{refusal}");
        assert_eq!(refuse_unless_checked(&hc::NumFormat::INVARIANT), Ok(()));
    }

    /// Every code the core publishes has a name here, and every other number is `unknown`
    /// (0 is `none`). A core that publishes a thirty-third code fails this until it is named.
    #[test]
    fn every_published_detail_is_named_and_the_rest_are_unknown() {
        for code in 1..=256 {
            let named = !matches!(detail_of(code), Detail::Unknown | Detail::None);
            assert_eq!(named, hc::Detail::from_code(code).is_some(), "code {code}");
        }
        assert!(matches!(detail_of(0), Detail::None));
        assert_eq!(
            (1..=256)
                .filter(|&c| hc::Detail::from_code(c).is_some())
                .count(),
            32
        );
    }

    /// A fault carrying a code no core published decodes: it never panics, the number is kept
    /// and the reason and span are untouched.
    #[test]
    fn a_fault_with_an_unpublished_detail_decodes_with_its_reason_and_span() {
        for code in [33, 999, u32::MAX] {
            let raw = hc::RawFaultDetail {
                size: hc::RawFaultDetail::SIZE,
                offset: 3,
                len: 2,
                detail: code,
            };
            let reason = reason_of(2).expect("code 2 is malformed");
            let fault = detailed_fault(reason, (raw.offset, raw.len), raw.detail);
            assert!(matches!(fault.detail, Detail::Unknown), "{code}");
            assert_eq!(fault.detail_code, code);
            assert!(matches!(fault.reason, CastFailure::Malformed));
            assert_eq!((fault.offset, fault.length), (3, 2));
        }
        let known = detailed_fault(hc::Reason::OutOfRange, (0, 4), 25);
        assert!(matches!(known.detail, Detail::TooLarge));
        assert_eq!(known.detail_code, 25);
    }

    #[test]
    fn a_return_code_names_its_reason() {
        assert_eq!(reason_of(1), Some(hc::Reason::Empty));
        assert_eq!(reason_of(2), Some(hc::Reason::Malformed));
        assert_eq!(reason_of(3), Some(hc::Reason::OutOfRange));
        for code in [-1, 0, 4, 5] {
            assert_eq!(reason_of(code), None);
        }
    }
}
