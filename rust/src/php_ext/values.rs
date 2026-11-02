//! `Success`, `Fault`, `Decimal`, `Duration` and `DayPeriods`: 1.0's value classes, their
//! constructors and methods, with 1.0's arithmetic and messages.

use std::panic::AssertUnwindSafe;

use ext_php_rs::args::Arg;
use ext_php_rs::args::ArgInfoTables;
use ext_php_rs::builders::FunctionBuilder;
use ext_php_rs::convert::IntoZval;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::flags::{DataType, MethodFlags};
use ext_php_rs::types::Zval;
use ext_php_rs::zend::{ClassEntry, ExecuteData, run_handler};
use ext_php_rs::zend_fastcall;

use super::args::{coerce, frame_args};
use super::classes::{
    Declaration, Type, Value, assign, bool_zval, build, bytes_zval, call, finish,
    finish_registration, float_zval, int_zval, read, read_bool, read_bytes, read_int,
    register as register_class, this,
};
use super::enums::CastFailure;
use super::errors::{invalid_argument, pending, type_error};
use super::guard;

const SUCCESS: Declaration = Declaration {
    name: "HyperCast\\Success",
    properties: &[("value", Type::Mixed)],
    constants: &[],
    promoted: true,
};

const FAULT: Declaration = Declaration {
    name: "HyperCast\\Fault",
    properties: &[
        ("reason", Type::Class("HyperCast\\CastFailure")),
        ("offset", Type::Int),
        ("length", Type::Int),
    ],
    constants: &[],
    promoted: true,
};

const DECIMAL: Declaration = Declaration {
    name: "HyperCast\\Decimal",
    properties: &[("magnitude", Type::String), ("scale", Type::Int), ("negative", Type::Bool)],
    constants: &[],
    promoted: true,
};

const DURATION: Declaration = Declaration {
    name: "HyperCast\\Duration",
    properties: &[("seconds", Type::Int), ("nanos", Type::Int)],
    constants: &[],
    promoted: true,
};

/// `DayPeriods::MAX_BYTES`.
const PERIOD_MAX_BYTES: usize = 16;

/// `DayPeriods::PACKED_BYTES`.
pub(crate) const PERIODS_PACKED_BYTES: usize = 40;

const DAY_PERIODS: Declaration = Declaration {
    name: "HyperCast\\DayPeriods",
    properties: &[("am", Type::String), ("pm", Type::String)],
    constants: &[("MAX_BYTES", 16), ("PACKED_BYTES", 40)],
    promoted: true,
};

fn keep_success(ce: &'static mut ClassEntry, tables: ArgInfoTables) {
    std::mem::forget(tables);
    finish_registration(Value::Success, &SUCCESS, ce);
}

fn keep_fault(ce: &'static mut ClassEntry, tables: ArgInfoTables) {
    std::mem::forget(tables);
    finish_registration(Value::Fault, &FAULT, ce);
}

fn keep_decimal(ce: &'static mut ClassEntry, tables: ArgInfoTables) {
    std::mem::forget(tables);
    finish_registration(Value::Decimal, &DECIMAL, ce);
}

fn keep_duration(ce: &'static mut ClassEntry, tables: ArgInfoTables) {
    std::mem::forget(tables);
    finish_registration(Value::Duration, &DURATION, ce);
}

fn keep_day_periods(ce: &'static mut ClassEntry, tables: ArgInfoTables) {
    std::mem::forget(tables);
    finish_registration(Value::DayPeriods, &DAY_PERIODS, ce);
}

/// A constructor: public, its parameters as declared, no return type.
fn constructor(
    mut function: FunctionBuilder<'static>,
    params: &[(&str, DataType)],
) -> (FunctionBuilder<'static>, MethodFlags) {
    for (name, ty) in params {
        function = function.arg(Arg::new(*name, *ty));
    }
    (function, MethodFlags::Public)
}

/// Registers the five classes. MINIT.
pub(crate) fn register() -> ext_php_rs::error::Result<()> {
    let int = DataType::Long;
    let string = DataType::String;
    let boolean = DataType::Bool;
    register_class(
        &SUCCESS,
        vec![constructor(
            FunctionBuilder::new("__construct", success_construct),
            &[("value", DataType::Mixed)],
        )],
        keep_success,
    )?;
    register_class(
        &FAULT,
        vec![constructor(
            FunctionBuilder::new("__construct", fault_construct),
            &[
                ("reason", DataType::object("HyperCast\\CastFailure")),
                ("offset", int),
                ("length", int),
            ],
        )],
        keep_fault,
    )?;
    register_class(
        &DECIMAL,
        vec![
            constructor(
                FunctionBuilder::new("__construct", decimal_construct),
                &[("magnitude", string), ("scale", int), ("negative", boolean)],
            ),
            (
                FunctionBuilder::new("fromLimbs", decimal_from_limbs)
                    .arg(Arg::new("lo", int))
                    .arg(Arg::new("hi", int))
                    .arg(Arg::new("scale", int))
                    .arg(Arg::new("negative", boolean))
                    .returns(DataType::object("HyperCast\\Decimal"), false, false),
                MethodFlags::Public | MethodFlags::Static,
            ),
            (
                FunctionBuilder::new("__toString", decimal_to_string).returns(string, false, false),
                MethodFlags::Public,
            ),
            (
                FunctionBuilder::new("toFloat", decimal_to_float).returns(
                    DataType::Double,
                    false,
                    false,
                ),
                MethodFlags::Public,
            ),
        ],
        keep_decimal,
    )?;
    register_class(
        &DURATION,
        vec![
            constructor(
                FunctionBuilder::new("__construct", duration_construct),
                &[("seconds", int), ("nanos", int)],
            ),
            (
                FunctionBuilder::new("toSeconds", duration_to_seconds).returns(
                    DataType::Double,
                    false,
                    false,
                ),
                MethodFlags::Public,
            ),
        ],
        keep_duration,
    )?;
    register_class(
        &DAY_PERIODS,
        vec![
            constructor(
                FunctionBuilder::new("__construct", day_periods_construct),
                &[("am", string), ("pm", string)],
            ),
            (
                FunctionBuilder::new("packed", day_periods_packed).returns(string, false, false),
                MethodFlags::Public,
            ),
        ],
        keep_day_periods,
    )
}

/// Checks every argument of a hand-registered method against its declared type, in order, as
/// the engine checks a userland function's on entry; `None` once one is refused (or the count
/// was wrong), the error pending.
pub(crate) fn checked_args(ex: &mut ExecuteData, min: u32, max: u32) -> Option<Vec<Zval>> {
    let args = frame_args(ex, min, max)?;
    let mut checked = Vec::with_capacity(args.len());
    for arg in args.iter_mut() {
        if !coerce(arg) {
            return None;
        }
        checked.push(arg.shallow_clone());
    }
    Some(checked)
}

/// The promoted assignments of a constructor called from PHP, in declaration order.
fn construct(ex: &mut ExecuteData, value: Value, declaration: &Declaration) -> PhpResult<Zval> {
    let object = this(ex)?;
    let count = declaration.properties.len() as u32;
    let Some(args) = checked_args(ex, count, count) else {
        return Err(pending("argument check"));
    };
    for ((name, _), arg) in declaration.properties.iter().zip(args) {
        assign(value, object, name, arg)?;
    }
    Ok(Zval::new())
}

zend_fastcall! {
    extern "C" fn success_construct(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| construct(ex, Value::Success, &SUCCESS)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn fault_construct(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| construct(ex, Value::Fault, &FAULT)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn decimal_construct(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| construct(ex, Value::Decimal, &DECIMAL)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn duration_construct(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| construct(ex, Value::Duration, &DURATION)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn day_periods_construct(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| day_periods_new(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn decimal_from_limbs(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| from_limbs_call(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn decimal_to_string(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| decimal_text_call(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn decimal_to_float(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| decimal_float_call(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn duration_to_seconds(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| to_seconds_call(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn day_periods_packed(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| periods_packed_call(ex)), retval)));
    }
}

/// `new Success($value)`, built by the binding.
pub(crate) fn success(value: Zval) -> PhpResult<Zval> {
    build(Value::Success, vec![value])
}

/// `new Fault($reason, $offset, $length)`, built by the binding.
pub(crate) fn fault(reason: CastFailure, offset: i64, length: i64) -> PhpResult<Zval> {
    let reason = reason.into_zval(false).map_err(|_| pending("CastFailure case"))?;
    build(Value::Fault, vec![reason, int_zval(offset), int_zval(length)])
}

/// `new Duration($seconds, $nanos)`, built by the binding.
pub(crate) fn duration(seconds: i64, nanos: i64) -> PhpResult<Zval> {
    build(Value::Duration, vec![int_zval(seconds), int_zval(nanos)])
}

/// 1.0's `Decimal::fromLimbs`, its arithmetic PHP's: 64-bit ints, `<<` and `>>` as the engine
/// shifts, `intdiv` and `%` truncating.
pub(crate) fn from_limbs(lo: i64, hi: i64, scale: i64, negative: bool) -> PhpResult<Zval> {
    let digits = if hi == 0 && lo >= 0 {
        lo.to_string()
    } else {
        let mut limbs = [hi, (lo >> 32) & 0xFFFF_FFFF, lo & 0xFFFF_FFFF];
        let mut chunks: Vec<i64> = Vec::new();
        while limbs != [0, 0, 0] {
            let mut remainder: i64 = 0;
            for limb in &mut limbs {
                let accumulator = remainder.wrapping_shl(32) | *limb;
                *limb = accumulator / 1_000_000_000;
                remainder = accumulator % 1_000_000_000;
            }
            chunks.push(remainder);
        }
        let mut digits = chunks.pop().map(|c| c.to_string()).unwrap_or_default();
        while let Some(chunk) = chunks.pop() {
            digits.push_str(&format!("{chunk:09}"));
        }
        digits
    };
    build(Value::Decimal, vec![bytes_zval(digits.as_bytes()), int_zval(scale), bool_zval(negative)])
}

fn from_limbs_call(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let Some(args) = checked_args(ex, 4, 4) else {
        return Err(pending("argument check"));
    };
    let int = |i: usize| args[i].long().unwrap_or_default();
    from_limbs(int(0), int(1), int(2), args[3].bool().unwrap_or_default())
}

/// 1.0's `Decimal::__toString`.
pub(crate) fn decimal_text(object: *mut ext_php_rs::ffi::zend_object) -> PhpResult<Vec<u8>> {
    let mut digits = read_bytes(Value::Decimal, object, "magnitude")?;
    let scale = read_int(Value::Decimal, object, "scale")?;
    let sign = |text: Vec<u8>, negative: bool| {
        if negative {
            let mut signed = vec![b'-'];
            signed.extend(text);
            signed
        } else {
            text
        }
    };
    if scale == 0 {
        let negative = read_bool(Value::Decimal, object, "negative")?;
        return Ok(sign(digits, negative));
    }
    let len = digits.len() as i64;
    if len <= scale {
        // `str_pad($digits, $this->scale + 1, '0', STR_PAD_LEFT)`: an int overflowing to a
        // float there is refused in strict mode, as 1.0's file declared.
        let Some(width) = scale.checked_add(1) else {
            return Err(type_error(
                "str_pad(): Argument #2 ($length) must be of type int, float given",
            ));
        };
        if width > 1 << 24 {
            // As much padding as PHP would allocate (or fail to): PHP's own `str_pad`.
            let padded = call(
                None,
                "str_pad",
                &mut [bytes_zval(&digits), int_zval(width), bytes_zval(b"0"), int_zval(0)],
            )?;
            digits = padded.zend_str().map(|s| s.as_bytes().to_vec()).unwrap_or_default();
        } else {
            let mut padded = vec![b'0'; (width - len) as usize];
            padded.extend(&digits);
            digits = padded;
        }
    }
    let len = digits.len() as i64;
    // `$split = \strlen($digits) - $this->scale`, then `substr` with it: an overflow to a float
    // is refused by `substr` in strict mode.
    let Some(split) = len.checked_sub(scale) else {
        return Err(type_error(
            "substr(): Argument #3 ($length) must be of type ?int, float given",
        ));
    };
    let head = php_substr(&digits, 0, Some(split));
    let tail = php_substr(&digits, split, None);
    let mut text = head.to_vec();
    text.push(b'.');
    text.extend_from_slice(tail);
    let negative = read_bool(Value::Decimal, object, "negative")?;
    Ok(sign(text, negative))
}

/// PHP 8's `substr($s, $offset, $length)`.
fn php_substr(s: &[u8], offset: i64, length: Option<i64>) -> &[u8] {
    let len = s.len() as i64;
    let mut start = offset;
    if start > len {
        return &[];
    }
    if start < 0 {
        start = (len + start).max(0);
    }
    let end = match length {
        None => len,
        Some(l) if l < 0 => {
            let end = len + l;
            if end < start {
                return &[];
            }
            end
        }
        Some(l) => start.saturating_add(l).min(len),
    };
    &s[start as usize..end.max(start) as usize]
}

fn decimal_text_call(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    if checked_args(ex, 0, 0).is_none() {
        return Err(pending("argument check"));
    }
    Ok(bytes_zval(&decimal_text(object)?))
}

fn decimal_float_call(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    if checked_args(ex, 0, 0).is_none() {
        return Err(pending("argument check"));
    }
    let text = decimal_text(object)?;
    // `(float) $text`: the engine's own string-to-float conversion.
    call(None, "floatval", &mut [bytes_zval(&text)])
}

/// 1.0's `Duration::toSeconds`: `$this->seconds + $this->nanos / 1_000_000_000`, PHP's
/// arithmetic, returned as `float`.
fn to_seconds_call(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    if checked_args(ex, 0, 0).is_none() {
        return Err(pending("argument check"));
    }
    let seconds = read_int(Value::Duration, object, "seconds")?;
    let nanos = read_int(Value::Duration, object, "nanos")?;
    let result = if nanos % 1_000_000_000 == 0 {
        // An exact division is an int; int + int overflows to a float.
        let quotient = nanos / 1_000_000_000;
        match seconds.checked_add(quotient) {
            Some(sum) => sum as f64,
            None => seconds as f64 + quotient as f64,
        }
    } else {
        seconds as f64 + nanos as f64 / 1_000_000_000.0
    };
    Ok(float_zval(result))
}

/// `new DayPeriods($am, $pm)` called from PHP: the promoted assignments, then 1.0's checks.
fn day_periods_new(ex: &mut ExecuteData) -> PhpResult<Zval> {
    construct(ex, Value::DayPeriods, &DAY_PERIODS)?;
    let object = this(ex)?;
    let am = read_bytes(Value::DayPeriods, object, "am")?;
    let pm = read_bytes(Value::DayPeriods, object, "pm")?;
    check_designator(&am, "AM designator")?;
    check_designator(&pm, "PM designator")?;
    Ok(Zval::new())
}

/// 1.0's `DayPeriods::checkDesignator`.
fn check_designator(designator: &[u8], role: &str) -> PhpResult<()> {
    let Ok(text) = std::str::from_utf8(designator) else {
        return Err(invalid_argument(format!(
            "{role} must be one to {PERIOD_MAX_BYTES} bytes of UTF-8"
        )));
    };
    if text.is_empty() || text.len() > PERIOD_MAX_BYTES {
        return Err(invalid_argument(format!(
            "{role} must be one to {PERIOD_MAX_BYTES} bytes of UTF-8"
        )));
    }
    if text.bytes().any(|b| b.is_ascii_digit()) {
        return Err(invalid_argument(format!(
            "{role} must not contain an ASCII digit; got '{text}'"
        )));
    }
    let space = |b: u8| matches!(b, b' ' | b'\t' | b'\n' | b'\x0c' | b'\r');
    let bytes = text.as_bytes();
    if bytes.first().copied().is_some_and(space) || bytes.last().copied().is_some_and(space) {
        return Err(invalid_argument(format!(
            "{role} must not start or end with ASCII whitespace; got '{text}'"
        )));
    }
    Ok(())
}

/// `str_pad($s, $width, "\0") . \chr(\strlen($s))`, as 1.0's packed layouts write a symbol.
pub(crate) fn field(out: &mut Vec<u8>, symbol: &[u8], width: usize) {
    out.extend_from_slice(symbol);
    out.resize(out.len() + width.saturating_sub(symbol.len()), 0);
    out.push((symbol.len() % 256) as u8);
}

/// 1.0's `DayPeriods::packed`.
pub(crate) fn periods_packed(object: *mut ext_php_rs::ffi::zend_object) -> PhpResult<Vec<u8>> {
    let am = read_bytes(Value::DayPeriods, object, "am")?;
    let pm = read_bytes(Value::DayPeriods, object, "pm")?;
    let mut packed = (PERIODS_PACKED_BYTES as u32).to_le_bytes().to_vec();
    field(&mut packed, &am, PERIOD_MAX_BYTES);
    field(&mut packed, &pm, PERIOD_MAX_BYTES);
    packed.extend_from_slice(&[0, 0]);
    Ok(packed)
}

fn periods_packed_call(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    if checked_args(ex, 0, 0).is_none() {
        return Err(pending("argument check"));
    }
    Ok(bytes_zval(&periods_packed(object)?))
}

/// The read of a value's property, for the doors (`Success::$value` and the rest are public).
#[allow(dead_code)]
pub(crate) fn property(value: Value, zv: &Zval, name: &str) -> PhpResult<Zval> {
    match super::classes::object_of(zv) {
        Some(object) => read(value, object, name),
        None => Err(pending("property read")),
    }
}
