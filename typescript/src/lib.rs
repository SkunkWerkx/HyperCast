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
// and the fault's two words); tests/constraints.test.ts holds the header to the same numbers.
const _: () = {
    assert!(size_of::<hc::RawConstraints>() == 72 && hc::RawConstraints::SIZE == 72);
    assert!(align_of::<hc::RawConstraints>() == 8);
    assert!(size_of::<hc::RawCheck>() == 12 && align_of::<hc::RawCheck>() == 4);
    assert!(size_of::<hc::RawSpan>() == 8 && align_of::<hc::RawSpan>() == 4);
    assert!(size_of::<RawFault>() == 8 && align_of::<RawFault>() == 4);
    assert!(size_of::<hc::Decimal>() == 16 && align_of::<hc::Decimal>() == 8);
};

/// The core's checked exports, as include/hypercast.h declares them.
mod abi {
    use super::{RawFault, hc};

    unsafe extern "C" {
        pub(super) fn cast_checked(
            door: u32,
            param: u32,
            format: *const hc::RawNumFormat,
            constraints: *const hc::RawConstraints,
            enum_ptr: *const u8,
            enum_len: usize,
            ptr: *const u8,
            len: usize,
            out: *mut std::ffi::c_void,
            fault: *mut RawFault,
            check: *mut hc::RawCheck,
        ) -> i32;
        pub(super) fn cast_checked_from_f64(
            door: u32,
            value: f64,
            constraints: *const hc::RawConstraints,
            enum_ptr: *const u8,
            enum_len: usize,
            out: *mut std::ffi::c_void,
            fault: *mut RawFault,
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

/// The door a `Constraints` is bound to, numbered as the core numbers it: the integer family,
/// the two reals and the exact decimal (judged by `castChecked` and `castCheckedFromNumber`),
/// and text (judged by `castText`).
#[napi]
pub enum Door {
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
    Text = 18,
    Decimal = 19,
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
    /// The door the declaration is bound to.
    #[napi(ts_type = "Door")]
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
        .ok_or_else(|| Problem::Contract("door must be a Door".to_string()))?;
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
    let (Some(constraints), Some(format)) = (constraints_arg(&constraints), num_format(&format))
    else {
        return contract(env);
    };
    let door = constraints.door;
    if door == hc::Door::Text {
        return contract(env);
    }
    let input = CheckedInput::read(env, &text);
    match input {
        CheckedInput::Absent => return failure(env, ABSENT),
        CheckedInput::Invalid => return contract(env),
        CheckedInput::Str(..) | CheckedInput::Bytes(_) => {}
    }
    let bytes = input.bytes();
    let format = hc::RawNumFormat::from(format);
    let mut slot = [0u64; 2];
    let mut fault = RawFault::default();
    let mut check = NO_CHECK;
    // SAFETY: every pointer is live for the call, and `slot` holds any numeric door's type.
    let code = unsafe {
        abi::cast_checked(
            door.code(),
            0,
            &format,
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
    match code {
        0 => numeric_success(env, door, &slot, &check),
        code => rejected(env, code, input.span(fault.offset, fault.len), &check),
    }
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
    let (Some(constraints), Some(value)) = (constraints_arg(&constraints), number(&value)) else {
        return contract(env);
    };
    let door = constraints.door;
    if door == hc::Door::Text {
        return contract(env);
    }
    let mut slot = [0u64; 2];
    let mut fault = RawFault::default();
    let mut check = NO_CHECK;
    // SAFETY: every pointer is live for the call, and `slot` holds any numeric door's type.
    let code = unsafe {
        abi::cast_checked_from_f64(
            door.code(),
            value,
            &constraints.raw,
            constraints.packed.as_ptr(),
            constraints.packed.len(),
            slot.as_mut_ptr().cast(),
            &mut fault,
            &mut check,
        )
    };
    match code {
        0 => numeric_success(env, door, &slot, &check),
        code => rejected(env, code, (fault.offset, fault.len), &check),
    }
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
    let constraints = match constraints.get_type() {
        Ok(ValueType::Undefined | ValueType::Null) => None,
        _ => match constraints_arg(&constraints) {
            Some(constraints) if constraints.door == hc::Door::Text => Some(constraints),
            _ => return contract(env),
        },
    };
    let input = CheckedInput::read(env, &text);
    match input {
        CheckedInput::Absent => return failure(env, ABSENT),
        CheckedInput::Invalid => return contract(env),
        CheckedInput::Str(..) | CheckedInput::Bytes(_) => {}
    }
    let bytes = input.bytes();
    let mut span = hc::RawSpan { offset: 0, len: 0 };
    let mut fault = RawFault::default();
    let Some(constraints) = constraints else {
        // SAFETY: every pointer is live for the call.
        return match unsafe { abi::cast_text(bytes.as_ptr(), bytes.len(), &mut span, &mut fault) } {
            0 => success(env, input.text_at(env, span.offset, span.len)?),
            code => rejected(env, code, input.span(fault.offset, fault.len), &NO_CHECK),
        };
    };
    let mut check = NO_CHECK;
    // SAFETY: every pointer is live for the call; the text door writes a `RawSpan`.
    let code = unsafe {
        abi::cast_checked(
            hc::Door::Text.code(),
            0,
            std::ptr::null(),
            &constraints.raw,
            constraints.packed.as_ptr(),
            constraints.packed.len(),
            bytes.as_ptr(),
            bytes.len(),
            (&raw mut span).cast(),
            &mut fault,
            &mut check,
        )
    };
    if code != 0 {
        return rejected(env, code, input.span(fault.offset, fault.len), &check);
    }
    if check.enum_index == u32::MAX {
        return checked_success(env, input.text_at(env, span.offset, span.len)?, &check);
    }
    match constraints.entries.get(check.enum_index as usize) {
        Some(Entry::Text(entry)) => checked_success(env, entry.as_str(), &check),
        _ => Err(binding_bug("matched an enum entry never declared")),
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
