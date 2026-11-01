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

use hypercast as hc;
use napi::bindgen_prelude::{
    BigInt, ClassInstance, Either, JavaScriptClassExt, Object, ToNapiValue, Uint8Array, Unknown,
};
use napi::{Env, Result, ValueType};
use napi_derive::napi;

/// The closed set of reasons a cast can fail.
#[napi(string_enum)]
pub enum CastFailure {
    /// The text was absent, empty or whitespace.
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

/// What a door reads: a string (as UTF-8), the bytes of a `Uint8Array` (a `Buffer` is one)
/// as they are, or `null` or `undefined` for absent input.
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

/// The declared form of a `NumFormat`: `NumFormat.declare` reads it.
#[napi(object, object_to_js = false, object_from_js = false)]
pub struct NumFormatSpec {
    /// The decimal separator: 1 to 8 UTF-8 bytes with no ASCII digit.
    pub decimal_sep: String,
    /// The group separator, likewise, and different from `decimalSep`.
    pub group_sep: String,
    /// The leniences, `NumStyles` members combined with `|`.
    pub flags: u32,
    /// The currency symbol, 1 to 16 UTF-8 bytes with no ASCII digit or whitespace.
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

// ---------------------------------------------------------------------------------------
// Reading arguments. Nothing here throws: every napi failure is a value the door reports.
// ---------------------------------------------------------------------------------------

/// A door's text, as the caller passed it.
enum Input {
    /// A string, held as the UTF-8 napi converted it to (a lone surrogate became U+FFFD,
    /// three bytes for its one code unit, so spans still map back exactly).
    Str(String),
    /// The bytes of a `Uint8Array`, read as they are.
    Bytes(Uint8Array),
    /// `null` or `undefined`.
    Absent,
    /// Anything else.
    Invalid,
}

impl Input {
    fn read(value: &Unknown<'_>) -> Input {
        match value.get_type() {
            Ok(ValueType::String) => match unsafe { value.cast::<String>() } {
                Ok(text) => Input::Str(text),
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
        let Input::Str(text) = self else {
            return fault;
        };
        let bytes = text.as_bytes();
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

/// UTF-16 code units in well-formed UTF-8: one per lead byte, two for a four-byte sequence.
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

/// A `format` argument: absent is the invariant format, as a null format is at the C ABI.
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
    let reason = match fault.reason {
        hc::Reason::Empty => CastFailure::Empty,
        hc::Reason::Malformed => CastFailure::Malformed,
        hc::Reason::OutOfRange => CastFailure::OutOfRange,
    };
    let mut verdict = Object::new(env)?;
    verdict.set("ok", false)?;
    verdict.set(
        "fault",
        Fault {
            reason,
            offset: fault.offset,
            length: fault.len,
        },
    )?;
    Ok(verdict)
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
    let input = Input::read(text);
    let outcome = match &input {
        Input::Str(text) => cast(text.as_bytes(), declared),
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

/// A declaration's text field: absent (`undefined`) declares nothing.
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
        Ok(ValueType::String) => unsafe { value.cast::<String>() }
            .map(Some)
            .map_err(|_| format!("{name} could not be read")),
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

/// A number separator: 1 to 8 UTF-8 bytes with no ASCII digit, the core's own rule.
fn separator(what: &str, text: &str) -> std::result::Result<hc::Symbol<8>, String> {
    hc::Symbol::new(text).ok_or_else(|| {
        format!("{what} must be 1 to 8 UTF-8 bytes with no ASCII digit; got \"{text}\"")
    })
}

/// A symbol a caller need not declare: absent or `""` declares none; with `flush` (a sign,
/// the percent symbol) it may hold no ASCII whitespace either.
fn optional_symbol(
    what: &str,
    text: Option<&str>,
    flush: bool,
) -> std::result::Result<hc::Symbol<8>, String> {
    let Some(text) = text.filter(|text| !text.is_empty()) else {
        return Ok(hc::Symbol::NONE);
    };
    if flush && text.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(format!(
            "{what} must be 1 to 8 UTF-8 bytes with no ASCII digit or whitespace; got \"{text}\""
        ));
    }
    separator(what, text)
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
    let currency = match currency.as_deref().filter(|text| !text.is_empty()) {
        None => hc::CurrencySymbol::NONE,
        Some(text) => hc::CurrencySymbol::new(text).ok_or_else(|| {
            format!(
                "Currency symbol must be 1 to 16 UTF-8 bytes with no ASCII digit or whitespace; got \"{text}\""
            )
        })?,
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
        .with_digits(digits);
    debug_assert!(resolved.check(), "every contract rule is refused above");
    Ok(resolved)
}

#[napi]
impl NumFormat {
    /// Declares a format, or says which rule the declaration breaks; never throws. The
    /// separators are each 1 to 8 UTF-8 bytes with no ASCII digit, and differ (so do the
    /// pair a currency amount is read with); a sign or percent symbol holds no ASCII
    /// whitespace; the currency symbol is at most 16 bytes with no ASCII digit or whitespace;
    /// the digit zero is the one non-ASCII character a Unicode digit block starts at.
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

    /// The invariant format: `.` decimal, `,` grouping, `NumStyles.All`. What a door reads
    /// with when its format is left out.
    #[napi(factory, catch_unwind)]
    pub fn invariant() -> Self {
        NumFormat {
            resolved: hc::NumFormat::INVARIANT,
        }
    }

    /// The detection format: `NumStyles.All | NumStyles.SeparatorDetect`, the `.`/`,` roles
    /// resolved per input from structure.
    #[napi(factory, catch_unwind)]
    pub fn detect() -> Self {
        NumFormat {
            resolved: hc::NumFormat::DETECT,
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

    /// The leniences, `NumStyles` members combined.
    #[napi(getter, catch_unwind)]
    pub fn flags(&self) -> u32 {
        self.resolved.flags
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
    /// library that declares numeric columns across a C ABI of its own.
    #[napi(getter, catch_unwind)]
    pub fn packed(&self) -> Uint8Array {
        Uint8Array::from(hc::RawNumFormat::from(self.resolved).to_le_bytes().to_vec())
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
/// bytes with no ASCII digit and no ASCII whitespace at either end (`a. m.` is fine).
fn designator(what: &str, text: Option<&str>) -> std::result::Result<hc::Symbol<16>, String> {
    let Some(text) = text.filter(|text| !text.is_empty()) else {
        return Ok(hc::Symbol::NONE);
    };
    let edges = text
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_whitespace())
        || text
            .bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_whitespace());
    if !edges && let Some(symbol) = hc::Symbol::new(text) {
        return Ok(symbol);
    }
    Err(format!(
        "Day period {what} must be 1 to 16 UTF-8 bytes with no ASCII digit and no whitespace at either end; got \"{text}\""
    ))
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
    /// whitespace at either end, read exactly, after the clock or ahead of it.
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
#[napi(
    catch_unwind,
    ts_args_type = "text: Text",
    ts_return_type = "Verdict<Timestamp>"
)]
pub fn cast_timestamp<'env>(env: &'env Env, text: Unknown<'env>) -> Result<Object<'env>> {
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
/// (`1/7/2026`, `1.7.2026`) under a declared one.
#[napi(
    catch_unwind,
    ts_args_type = "text: Text, order?: DateOrder | undefined | null",
    ts_return_type = "Verdict<CalendarDate>"
)]
pub fn cast_date<'env>(
    env: &'env Env,
    text: Unknown<'env>,
    order: Unknown<'env>,
) -> Result<Object<'env>> {
    let order = match order.get_type() {
        Ok(ValueType::Undefined | ValueType::Null) => Some(None),
        _ => code(&order).and_then(hc::DateOrder::from_code).map(Some),
    };
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
#[napi(
    catch_unwind,
    ts_args_type = "text: Text, format: DateOrder | DateFormat",
    ts_return_type = "Verdict<CivilDateTime>"
)]
pub fn cast_datetime<'env>(
    env: &'env Env,
    text: Unknown<'env>,
    format: Unknown<'env>,
) -> Result<Object<'env>> {
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
