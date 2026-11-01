//! `NumFormat`: 1.0's declared number format, its validation, its constructors from a
//! `localeconv()` array and an ext-intl `\NumberFormatter`, and its packed layout, which the
//! doors hand the core as 1.0 handed it through FFI (`RawNumFormat`).

use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;
use std::panic::AssertUnwindSafe;

use ext_php_rs::args::{Arg, ArgInfoTables};
use ext_php_rs::builders::FunctionBuilder;
use ext_php_rs::exception::{PhpException, PhpResult};
use ext_php_rs::ffi::zend_object;
use ext_php_rs::flags::{DataType, MethodFlags};
use ext_php_rs::types::Zval;
use ext_php_rs::zend::{ClassEntry, ExecuteData, ce, run_handler};
use ext_php_rs::zend_fastcall;

use super::classes::{
    Declaration, Type, Value, assign, build, bytes_zval, call, finish, finish_registration,
    int_zval, object_of, read_bytes, read_int, register as register_class, this,
};
use super::errors::{contract_violation, invalid_argument, pending};
use super::guard;
use super::values::{checked_args, field};
use crate::{NumFormat as CoreFormat, RawNumFormat};

const GROUPING: i64 = 1;
const PARENTHESES: i64 = 1 << 1;
const EXPONENT: i64 = 1 << 2;
const RADIX_PREFIXES: i64 = 1 << 3;
const PERCENT: i64 = 1 << 4;
const SEPARATOR_DETECT: i64 = 1 << 5;
const CURRENCY: i64 = 1 << 6;
const TRAILING_SIGN: i64 = 1 << 7;
const ALL: i64 = GROUPING | PARENTHESES | EXPONENT | RADIX_PREFIXES | PERCENT | CURRENCY;
const SYMBOL_MAX_BYTES: usize = 8;
const CURRENCY_MAX_BYTES: usize = 16;
const PACKED_BYTES: usize = 92;

/// The properties, in 1.0's declaration (and constructor) order.
const PROPERTIES: [&str; 10] = [
    "decimalSep",
    "groupSep",
    "flags",
    "currency",
    "currencyDecimalSep",
    "currencyGroupSep",
    "minusSign",
    "plusSign",
    "percent",
    "digitZero",
];

const NUM_FORMAT: Declaration = Declaration {
    name: "HyperCast\\NumFormat",
    properties: &[
        ("decimalSep", Type::String),
        ("groupSep", Type::String),
        ("flags", Type::Int),
        ("currency", Type::String),
        ("currencyDecimalSep", Type::String),
        ("currencyGroupSep", Type::String),
        ("minusSign", Type::String),
        ("plusSign", Type::String),
        ("percent", Type::String),
        ("digitZero", Type::String),
    ],
    constants: &[
        ("GROUPING", GROUPING),
        ("PARENTHESES", PARENTHESES),
        ("EXPONENT", EXPONENT),
        ("RADIX_PREFIXES", RADIX_PREFIXES),
        ("PERCENT", PERCENT),
        ("SEPARATOR_DETECT", SEPARATOR_DETECT),
        ("CURRENCY", CURRENCY),
        ("TRAILING_SIGN", TRAILING_SIGN),
        ("ALL", ALL),
        ("SYMBOL_MAX_BYTES", SYMBOL_MAX_BYTES as i64),
        ("CURRENCY_MAX_BYTES", CURRENCY_MAX_BYTES as i64),
        ("PACKED_BYTES", PACKED_BYTES as i64),
    ],
};

// `\NumberFormatter`'s symbol and text attribute numbers (ext-intl's, from ICU's enums).
const DECIMAL_SEPARATOR_SYMBOL: i64 = 0;
const GROUPING_SEPARATOR_SYMBOL: i64 = 1;
const PERCENT_SYMBOL: i64 = 3;
const ZERO_DIGIT_SYMBOL: i64 = 4;
const MINUS_SIGN_SYMBOL: i64 = 6;
const PLUS_SIGN_SYMBOL: i64 = 7;
const CURRENCY_SYMBOL: i64 = 8;
const MONETARY_SEPARATOR_SYMBOL: i64 = 10;
const MONETARY_GROUPING_SEPARATOR_SYMBOL: i64 = 17;
const NEGATIVE_PREFIX: i64 = 2;
const NEGATIVE_SUFFIX: i64 = 3;

/// A format's fields as the constructor takes them.
struct Fields {
    decimal_sep: Vec<u8>,
    group_sep: Vec<u8>,
    flags: i64,
    currency: Vec<u8>,
    currency_decimal_sep: Vec<u8>,
    currency_group_sep: Vec<u8>,
    minus_sign: Vec<u8>,
    plus_sign: Vec<u8>,
    percent: Vec<u8>,
    digit_zero: Vec<u8>,
}

impl Fields {
    fn plain(decimal_sep: &[u8], group_sep: &[u8], flags: i64) -> Fields {
        Fields {
            decimal_sep: decimal_sep.to_vec(),
            group_sep: group_sep.to_vec(),
            flags,
            currency: Vec::new(),
            currency_decimal_sep: Vec::new(),
            currency_group_sep: Vec::new(),
            minus_sign: Vec::new(),
            plus_sign: Vec::new(),
            percent: Vec::new(),
            digit_zero: Vec::new(),
        }
    }

    fn into_zvals(self) -> Vec<Zval> {
        vec![
            bytes_zval(&self.decimal_sep),
            bytes_zval(&self.group_sep),
            int_zval(self.flags),
            bytes_zval(&self.currency),
            bytes_zval(&self.currency_decimal_sep),
            bytes_zval(&self.currency_group_sep),
            bytes_zval(&self.minus_sign),
            bytes_zval(&self.plus_sign),
            bytes_zval(&self.percent),
            bytes_zval(&self.digit_zero),
        ]
    }
}

fn keep_num_format(ce: &'static mut ClassEntry, tables: ArgInfoTables) {
    std::mem::forget(tables);
    finish_registration(Value::NumFormat, &NUM_FORMAT, ce);
}

/// Registers `NumFormat`. MINIT.
pub(crate) fn register() -> ext_php_rs::error::Result<()> {
    let string = DataType::String;
    let this_class = DataType::object("HyperCast\\NumFormat");
    let mut constructor = FunctionBuilder::new("__construct", construct_handler)
        .arg(Arg::new("decimalSep", string))
        .arg(Arg::new("groupSep", string))
        .arg(Arg::new("flags", DataType::Long))
        .not_required();
    for name in &PROPERTIES[3..] {
        constructor = constructor.arg(Arg::new(*name, string).default("''"));
    }
    let statics = MethodFlags::Public | MethodFlags::Static;
    register_class(
        &NUM_FORMAT,
        vec![
            (constructor, MethodFlags::Public),
            (
                FunctionBuilder::new("invariant", invariant_handler)
                    .returns(this_class, false, false),
                statics,
            ),
            (
                FunctionBuilder::new("detect", detect_handler).returns(this_class, false, false),
                statics,
            ),
            (
                FunctionBuilder::new("fromLocaleconv", from_localeconv_handler)
                    .not_required()
                    .arg(Arg::new("conv", DataType::Array).allow_null().default("null"))
                    .returns(this_class, false, false),
                statics,
            ),
            (
                FunctionBuilder::new("fromNumberFormatter", from_number_formatter_handler)
                    .arg(Arg::new("formatter", DataType::object("NumberFormatter")))
                    .returns(this_class, false, false),
                statics,
            ),
            (
                FunctionBuilder::new("packed", packed_handler).returns(string, false, false),
                MethodFlags::Public,
            ),
        ],
        keep_num_format,
    )
}

zend_fastcall! {
    extern "C" fn construct_handler(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| construct(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn invariant_handler(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| shared_call(ex, 0)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn detect_handler(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| shared_call(ex, 1)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn from_localeconv_handler(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| from_localeconv(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn from_number_formatter_handler(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| from_number_formatter_call(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn packed_handler(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| packed_call(ex)), retval)));
    }
}

/// `new NumFormat(...)` from PHP: the promoted assignments in order (the omitted ones their
/// default, `''`), then 1.0's checks.
fn construct(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    let Some(args) = checked_args(ex, 3, 10) else {
        return Err(pending("argument check"));
    };
    let mut values: Vec<Zval> = args;
    while values.len() < PROPERTIES.len() {
        values.push(bytes_zval(b""));
    }
    let bytes = |zv: &Zval| zv.zend_str().map(|s| s.as_bytes().to_vec()).unwrap_or_default();
    let fields = Fields {
        decimal_sep: bytes(&values[0]),
        group_sep: bytes(&values[1]),
        flags: values[2].long().unwrap_or_default(),
        currency: bytes(&values[3]),
        currency_decimal_sep: bytes(&values[4]),
        currency_group_sep: bytes(&values[5]),
        minus_sign: bytes(&values[6]),
        plus_sign: bytes(&values[7]),
        percent: bytes(&values[8]),
        digit_zero: bytes(&values[9]),
    };
    for (name, value) in PROPERTIES.iter().zip(values) {
        assign(Value::NumFormat, object, name, value)?;
    }
    validate(&fields)?;
    Ok(Zval::new())
}

/// A `NumFormat` the binding builds, checked as 1.0's `new self(...)` checked it.
fn new_format(fields: Fields) -> PhpResult<Zval> {
    validate(&fields)?;
    build(Value::NumFormat, fields.into_zvals())
}

/// 1.0's constructor body.
fn validate(f: &Fields) -> PhpResult<()> {
    check_symbol(&f.decimal_sep, "Decimal separator", SYMBOL_MAX_BYTES, false)?;
    check_symbol(&f.group_sep, "Group separator", SYMBOL_MAX_BYTES, false)?;
    if f.decimal_sep == f.group_sep {
        return Err(invalid_argument(format!(
            "Decimal and group separators must differ; both are '{}'",
            text(&f.decimal_sep)
        )));
    }
    if !f.currency_decimal_sep.is_empty() {
        check_symbol(
            &f.currency_decimal_sep,
            "Currency decimal separator",
            SYMBOL_MAX_BYTES,
            false,
        )?;
    }
    if !f.currency_group_sep.is_empty() {
        check_symbol(&f.currency_group_sep, "Currency group separator", SYMBOL_MAX_BYTES, false)?;
    }
    let currency_decimal =
        if f.currency_decimal_sep.is_empty() { &f.decimal_sep } else { &f.currency_decimal_sep };
    let currency_group =
        if f.currency_group_sep.is_empty() { &f.group_sep } else { &f.currency_group_sep };
    if currency_decimal == currency_group {
        return Err(invalid_argument(format!(
            "Currency decimal and group separators must differ; both are '{}'",
            text(currency_decimal)
        )));
    }
    if !f.minus_sign.is_empty() {
        check_symbol(&f.minus_sign, "Minus sign", SYMBOL_MAX_BYTES, true)?;
    }
    if !f.plus_sign.is_empty() {
        check_symbol(&f.plus_sign, "Plus sign", SYMBOL_MAX_BYTES, true)?;
    }
    if !f.percent.is_empty() {
        check_symbol(&f.percent, "Percent symbol", SYMBOL_MAX_BYTES, true)?;
    }
    if !f.currency.is_empty() {
        check_symbol(&f.currency, "Currency symbol", CURRENCY_MAX_BYTES, true)?;
    }
    if !f.digit_zero.is_empty() {
        let one_char = std::str::from_utf8(&f.digit_zero).is_ok_and(|s| s.chars().count() == 1);
        if !one_char {
            return Err(invalid_argument("Digit zero must be exactly one character"));
        }
        let code = code_point(&f.digit_zero).unwrap_or_default();
        if code < 0x80 || code % 64 > 54 {
            return Err(invalid_argument(format!(
                "Digit zero must start a Unicode digit block ('٠', '०', '０'); got '{}'",
                text(&f.digit_zero)
            )));
        }
    }
    Ok(())
}

/// A symbol a check has found to be UTF-8, as text for a message.
fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// 1.0's `NumFormat::checkSymbol`.
fn check_symbol(symbol: &[u8], role: &str, max_bytes: usize, flush: bool) -> PhpResult<()> {
    if symbol.is_empty() || symbol.len() > max_bytes || std::str::from_utf8(symbol).is_err() {
        return Err(invalid_argument(format!("{role} must be one to {max_bytes} bytes of UTF-8")));
    }
    let refused = |b: &u8| {
        b.is_ascii_digit() || (flush && matches!(b, b' ' | b'\t' | b'\n' | b'\x0c' | b'\r'))
    };
    if symbol.iter().any(refused) {
        let symbol = text(symbol);
        return Err(invalid_argument(if flush {
            format!("{role} must not contain an ASCII digit or whitespace; got '{symbol}'")
        } else {
            format!("{role} must not contain an ASCII digit; got '{symbol}'")
        }));
    }
    Ok(())
}

/// 1.0's `NumFormat::codePoint`: the bytes decoded without validation, by length; `None` for
/// a length its `match` had no arm for.
fn code_point(c: &[u8]) -> Option<i64> {
    let b = |i: usize| i64::from(c[i]);
    Some(match c.len() {
        1 => b(0),
        2 => (b(0) & 0x1F) << 6 | (b(1) & 0x3F),
        3 => (b(0) & 0x0F) << 12 | (b(1) & 0x3F) << 6 | (b(2) & 0x3F),
        4 => (b(0) & 0x07) << 18 | (b(1) & 0x3F) << 12 | (b(2) & 0x3F) << 6 | (b(3) & 0x3F),
        _ => return None,
    })
}

/// 1.0's `NumFormat::packed`, its properties read in its order.
pub(crate) fn packed(object: *mut zend_object) -> PhpResult<Vec<u8>> {
    let read = |name: &str| read_bytes(Value::NumFormat, object, name);
    let digit_zero = read("digitZero")?;
    let zero = if digit_zero.is_empty() {
        0
    } else {
        match code_point(&digit_zero) {
            Some(code) => code,
            None => {
                return Err(PhpException::new(
                    format!("Unhandled match case {}", digit_zero.len()),
                    0,
                    ce::unhandled_match_error(),
                ));
            }
        }
    };
    let flags = read_int(Value::NumFormat, object, "flags")?;
    let mut out = Vec::with_capacity(PACKED_BYTES);
    out.extend_from_slice(&(PACKED_BYTES as u32).to_le_bytes());
    out.extend_from_slice(&(flags as u32).to_le_bytes());
    out.extend_from_slice(&(zero as u32).to_le_bytes());
    for name in [
        "decimalSep",
        "groupSep",
        "currencyDecimalSep",
        "currencyGroupSep",
        "minusSign",
        "plusSign",
        "percent",
    ] {
        field(&mut out, &read(name)?, SYMBOL_MAX_BYTES);
    }
    field(&mut out, &read("currency")?, CURRENCY_MAX_BYTES);
    Ok(out)
}

fn packed_call(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    if checked_args(ex, 0, 0).is_none() {
        return Err(pending("argument check"));
    }
    Ok(bytes_zval(&packed(object)?))
}

/// The core's format for a `NumFormat` object: its packed bytes, the first 92 as 1.0's
/// `FFI::memcpy` copied them, read as the C ABI reads them. A layout the core refuses is 1.0's
/// contract violation.
pub(crate) fn core_format(zv: &Zval) -> PhpResult<CoreFormat> {
    let object = object_of(zv).ok_or_else(|| pending("format"))?;
    let bytes = packed(object)?;
    let mut layout = [0u8; PACKED_BYTES];
    let n = bytes.len().min(PACKED_BYTES);
    layout[..n].copy_from_slice(&bytes[..n]);
    RawNumFormat::from_le_bytes(layout).resolve().copied().ok_or_else(contract_violation)
}

/// The format a door declares: a `NumFormat`, or 1.0's reading of a `\NumberFormatter`.
pub(crate) fn declared(format: &Zval) -> PhpResult<CoreFormat> {
    let is_num_format = format
        .object()
        .and_then(|o| o.get_class_name().ok())
        .is_some_and(|name| name.eq_ignore_ascii_case("HyperCast\\NumFormat"));
    if is_num_format {
        return core_format(format);
    }
    let built = from_number_formatter(format)?;
    core_format(&built)
}

thread_local! {
    /// `invariant()` and `detect()` for the current request, one instance each per request,
    /// as 1.0's function-static caches held them. Released at request shutdown
    /// ([`release_shared`]) and only there: `ManuallyDrop`, so the thread-local has no
    /// destructor to run after the engine's heap is gone.
    static SHARED: RefCell<[Option<ManuallyDrop<Zval>>; 2]> = const { RefCell::new([None, None]) };

    /// Whether a request is running (see `mod.rs`'s lifecycle callbacks): outside one nothing
    /// is cached, since nothing would release it before the engine frees its heap.
    static IN_REQUEST: Cell<bool> = const { Cell::new(false) };
}

const _: () = assert!(!std::mem::needs_drop::<[Option<ManuallyDrop<Zval>>; 2]>());

fn shared(slot: usize) -> PhpResult<Zval> {
    let make = || {
        let flags = if slot == 0 { ALL } else { ALL | SEPARATOR_DETECT };
        new_format(Fields::plain(b".", b",", flags))
    };
    if !IN_REQUEST.get() {
        return make();
    }
    SHARED.with_borrow_mut(|cache| {
        if cache[slot].is_none() {
            cache[slot] = Some(ManuallyDrop::new(make()?));
        }
        match &cache[slot] {
            Some(zv) => Ok(zv.shallow_clone()),
            None => Err(pending("shared format")),
        }
    })
}

fn shared_call(ex: &mut ExecuteData, slot: usize) -> PhpResult<Zval> {
    if checked_args(ex, 0, 0).is_none() {
        return Err(pending("argument check"));
    }
    shared(slot)
}

/// RSHUTDOWN: stops caching and releases the request's shared formats.
pub(crate) fn release_shared() {
    IN_REQUEST.set(false);
    let released = SHARED.with_borrow_mut(std::mem::take);
    for zv in released.into_iter().flatten() {
        drop(ManuallyDrop::into_inner(zv));
    }
}

/// Post-RSHUTDOWN: ends the request for the cache, forgetting (never releasing) anything left,
/// which the engine has freed with the request when the RSHUTDOWNs were skipped.
pub(crate) fn end_request() {
    IN_REQUEST.set(false);
    forget_stale();
}

/// Forgets, without releasing, what an earlier request left cached.
pub(crate) fn forget_stale() {
    SHARED.with_borrow_mut(|cache| cache.fill_with(|| None));
}

/// RINIT, last: starts caching.
pub(crate) fn begin_request() {
    IN_REQUEST.set(true);
}

/// 1.0's private `fromSymbols`: fills in what a locale leaves out and drops what matches the
/// invariant default, then constructs.
#[allow(clippy::too_many_arguments)]
fn from_symbols(
    mut decimal_sep: Vec<u8>,
    mut group_sep: Vec<u8>,
    currency: Vec<u8>,
    mut currency_decimal_sep: Vec<u8>,
    mut currency_group_sep: Vec<u8>,
    minus_sign: Vec<u8>,
    plus_sign: Vec<u8>,
    percent: Vec<u8>,
    digit_zero: Vec<u8>,
    trailing_sign: bool,
) -> PhpResult<Zval> {
    let swap = |sep: &[u8]| if sep == b"," { b".".to_vec() } else { b",".to_vec() };
    if decimal_sep.is_empty() {
        decimal_sep = if group_sep == b"." { b",".to_vec() } else { b".".to_vec() };
    }
    if group_sep.is_empty() {
        group_sep = if decimal_sep == b"," { b".".to_vec() } else { b",".to_vec() };
    }
    if currency_decimal_sep == decimal_sep {
        currency_decimal_sep.clear();
    }
    if currency_group_sep == group_sep {
        currency_group_sep.clear();
    }
    if currency_decimal_sep.is_empty() && currency_group_sep == decimal_sep {
        currency_decimal_sep = swap(&currency_group_sep);
    } else if currency_group_sep.is_empty() && currency_decimal_sep == group_sep {
        currency_group_sep = swap(&currency_decimal_sep);
    }
    let unless = |value: Vec<u8>, default: &[u8]| if value == default { Vec::new() } else { value };
    new_format(Fields {
        decimal_sep,
        group_sep,
        flags: ALL | if trailing_sign { TRAILING_SIGN } else { 0 },
        currency,
        currency_decimal_sep,
        currency_group_sep,
        minus_sign: unless(minus_sign, b"-"),
        plus_sign: unless(plus_sign, b"+"),
        percent: unless(percent, b"%"),
        digit_zero: unless(digit_zero, b"0"),
    })
}

/// `NumFormat::fromLocaleconv(?array $conv = null)`.
fn from_localeconv(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let Some(args) = checked_args(ex, 0, 1) else {
        return Err(pending("argument check"));
    };
    let conv = match args.into_iter().next() {
        Some(conv) if !conv.is_null() => conv,
        _ => call(None, "localeconv", &mut [])?,
    };
    let entry = |key: &str| -> Option<Zval> {
        conv.array().and_then(|array| array.get(key)).map(|zv| zv.dereference().shallow_clone())
    };
    let text = |key: &str| {
        entry(key).and_then(|zv| zv.zend_str().map(|s| s.as_bytes().to_vec())).unwrap_or_default()
    };
    let sign_position = entry("n_sign_posn").and_then(|zv| zv.long());
    from_symbols(
        text("decimal_point"),
        text("thousands_sep"),
        text("currency_symbol"),
        text("mon_decimal_point"),
        text("mon_thousands_sep"),
        text("negative_sign"),
        text("positive_sign"),
        Vec::new(),
        Vec::new(),
        matches!(sign_position, Some(2 | 4)),
    )
}

fn from_number_formatter_call(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let Some(args) = checked_args(ex, 1, 1) else {
        return Err(pending("argument check"));
    };
    from_number_formatter(&args[0])
}

/// `NumFormat::fromNumberFormatter`: the formatter's answers, asked in 1.0's order.
fn from_number_formatter(formatter: &Zval) -> PhpResult<Zval> {
    let ask = |method: &str, which: i64| call(Some(formatter), method, &mut [int_zval(which)]);
    let negative_prefix = ask("getTextAttribute", NEGATIVE_PREFIX)?;
    let negative_suffix = ask("getTextAttribute", NEGATIVE_SUFFIX)?;
    let suffix = negative_suffix.zend_str().map(|s| s.as_bytes().to_vec());
    let prefix_empty = negative_prefix.is_false()
        || negative_prefix.zend_str().is_some_and(|s| s.as_bytes().is_empty());
    let trailing_sign = suffix.is_some_and(|s| !s.is_empty()) && prefix_empty;
    let symbol = |which: i64| -> PhpResult<Vec<u8>> {
        let answer = ask("getSymbol", which)?;
        Ok(answer.zend_str().map(|s| s.as_bytes().to_vec()).unwrap_or_default())
    };
    let decimal_sep = symbol(DECIMAL_SEPARATOR_SYMBOL)?;
    let group_sep = symbol(GROUPING_SEPARATOR_SYMBOL)?;
    let currency = symbol(CURRENCY_SYMBOL)?;
    let currency_decimal_sep = symbol(MONETARY_SEPARATOR_SYMBOL)?;
    let currency_group_sep = symbol(MONETARY_GROUPING_SEPARATOR_SYMBOL)?;
    let minus_sign = symbol(MINUS_SIGN_SYMBOL)?;
    let plus_sign = symbol(PLUS_SIGN_SYMBOL)?;
    let percent = symbol(PERCENT_SYMBOL)?;
    let digit_zero = symbol(ZERO_DIGIT_SYMBOL)?;
    from_symbols(
        decimal_sep,
        group_sep,
        currency,
        currency_decimal_sep,
        currency_group_sep,
        minus_sign,
        plus_sign,
        percent,
        digit_zero,
        trailing_sign,
    )
}
