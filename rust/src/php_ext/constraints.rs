//! Validation constraints: `Constraints`, the OpenAPI Schema Object's keywords bound to one
//! door, and the checked verdict's `Checked` and `Violation`, registered as plain `final
//! readonly` classes like the other value classes (`classes.rs`); and the checked doors'
//! bodies, which `Cast` calls (`cast.rs`).
//!
//! A declaration is laid out exactly as the C ABI carries it, a `RawConstraints` with the packed
//! `enum` list beside it, and judged by the core's own functions: `check_constraints` when a
//! `Constraints` is built, and the `judge_*` functions the checked C exports call, on every
//! cast. Bounds, steps and integer entries are converted by `declared_decimal`, the conversion
//! the C export `cast_declared_decimal` runs. No rule is restated here: every refusal and every
//! violation is the core's.
//!
//! A `Constraints` holds its declaration as PHP values (the door and rounding cases, each bound
//! as the exact `Decimal` it declares, the lengths, the `enum` entries), and a checked door lays
//! them out again on every call, as a door packs its `NumFormat` on every call: an object PHP
//! built around the constructor (`unserialize`, say) is judged by the core like any other, and
//! one whose properties were never written (`ReflectionClass::newInstanceWithoutConstructor`)
//! is the engine's `Error` for the first property read, never a declaration of no rules.

use std::mem::MaybeUninit;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};

use ext_php_rs::args::{Arg, ArgInfoTables};
use ext_php_rs::builders::FunctionBuilder;
use ext_php_rs::convert::{FromZval, IntoZval};
use ext_php_rs::exception::{PhpException, PhpResult};
use ext_php_rs::ffi::{IS_LONG, IS_STRING, zend_object};
use ext_php_rs::flags::{DataType, MethodFlags};
use ext_php_rs::types::{ZendHashTable, Zval};
use ext_php_rs::zend::{ClassEntry, ExecuteData, ce, run_handler};
use ext_php_rs::zend_fastcall;

use super::args::{exact, frame_args};
use super::classes::{
    Declaration, Type, Value, assign, bool_zval, build, bytes_zval, finish, finish_registration,
    float_zval, int_zval, object_of, read, register as register_class, this, widen,
};
use super::enums::{CastFailure, Door, Keyword, Rounding};
use super::errors::{contract_violation, pending};
use super::guard;
use super::values::{checked_args, decimal_text, fault, from_limbs, success};
use crate::checked::{CheckOutcome, judge_numeric, judge_numeric_from_f64, judge_text};
use crate::constraints::{Entries, declared_decimal};
use crate::{NumFormat, RawConstraints, Refusal, Rejection};

const CONSTRAINTS: Declaration = Declaration {
    name: "HyperCast\\Constraints",
    properties: &[
        ("door", Type::Class("HyperCast\\Door")),
        ("minimum", Type::NullableClass("HyperCast\\Decimal")),
        ("exclusiveMinimum", Type::NullableClass("HyperCast\\Decimal")),
        ("maximum", Type::NullableClass("HyperCast\\Decimal")),
        ("exclusiveMaximum", Type::NullableClass("HyperCast\\Decimal")),
        ("multipleOf", Type::NullableClass("HyperCast\\Decimal")),
        ("rounding", Type::Class("HyperCast\\Rounding")),
        ("minLength", Type::NullableInt),
        ("maxLength", Type::NullableInt),
        ("trim", Type::Bool),
        ("enum", Type::NullableArray),
    ],
    constants: &[("PACKED_BYTES", RawConstraints::SIZE as i64)],
    promoted: false,
};

const CHECKED: Declaration = Declaration {
    name: "HyperCast\\Checked",
    properties: &[
        ("value", Type::Mixed),
        ("adjusted", Type::Bool),
        ("enumIndex", Type::NullableInt),
    ],
    constants: &[],
    promoted: true,
};

const VIOLATION: Declaration = Declaration {
    name: "HyperCast\\Violation",
    properties: &[
        ("keyword", Type::ClassOrInt("HyperCast\\Keyword")),
        ("offset", Type::Int),
        ("length", Type::Int),
    ],
    constants: &[],
    promoted: true,
};

/// The constructor's parameters after the door, in order: the five decimals, then the rest.
const BOUNDS: [(&str, u32); 5] = [
    ("minimum", RawConstraints::HAS_MINIMUM),
    ("exclusiveMinimum", RawConstraints::HAS_EXCLUSIVE_MINIMUM),
    ("maximum", RawConstraints::HAS_MAXIMUM),
    ("exclusiveMaximum", RawConstraints::HAS_EXCLUSIVE_MAXIMUM),
    ("multipleOf", RawConstraints::HAS_MULTIPLE_OF),
];

/// The keywords this binding names, for `Violation::keywordName()`.
const KEYWORDS: [crate::Keyword; 8] = [
    crate::Keyword::Minimum,
    crate::Keyword::ExclusiveMinimum,
    crate::Keyword::Maximum,
    crate::Keyword::ExclusiveMaximum,
    crate::Keyword::MultipleOf,
    crate::Keyword::MinLength,
    crate::Keyword::MaxLength,
    crate::Keyword::Enum,
];

/// Whether every `widen()` in a registration callback found its parameter; read by
/// `register()`, which fails the registration (and so MINIT) when one did not.
static WIDENED: AtomicBool = AtomicBool::new(true);

fn keep_constraints(ce: &'static mut ClassEntry, tables: ArgInfoTables) {
    std::mem::forget(tables);
    // `int|string|Decimal|null` for each bound: ext-php-rs declares the class and `null`.
    for position in 1..=BOUNDS.len() {
        if !widen(ce, "__construct", position, (1 << IS_LONG) | (1 << IS_STRING)) {
            WIDENED.store(false, Ordering::Release);
        }
    }
    finish_registration(Value::Constraints, &CONSTRAINTS, ce);
}

fn keep_checked(ce: &'static mut ClassEntry, tables: ArgInfoTables) {
    std::mem::forget(tables);
    finish_registration(Value::Checked, &CHECKED, ce);
}

fn keep_violation(ce: &'static mut ClassEntry, tables: ArgInfoTables) {
    std::mem::forget(tables);
    // `Keyword|int $keyword`: ext-php-rs declares the class.
    if !widen(ce, "__construct", 0, 1 << IS_LONG) {
        WIDENED.store(false, Ordering::Release);
    }
    finish_registration(Value::Violation, &VIOLATION, ce);
}

/// Registers the three classes. MINIT.
pub(crate) fn register() -> ext_php_rs::error::Result<()> {
    let int = DataType::Long;
    let string = DataType::String;
    let decimal = DataType::object("HyperCast\\Decimal");
    let mut constructor = FunctionBuilder::new("__construct", constraints_construct)
        .arg(Arg::new("door", DataType::object("HyperCast\\Door")))
        .not_required();
    for (name, _) in BOUNDS {
        constructor = constructor.arg(Arg::new(name, decimal).allow_null().default("null"));
    }
    constructor = constructor
        .arg(
            Arg::new("rounding", DataType::object("HyperCast\\Rounding"))
                .default("\\HyperCast\\Rounding::Strict"),
        )
        .arg(Arg::new("minLength", int).allow_null().default("null"))
        .arg(Arg::new("maxLength", int).allow_null().default("null"))
        .arg(Arg::new("trim", DataType::Bool).default("false"))
        .arg(Arg::new("enum", DataType::Array).allow_null().default("null"));
    register_class(
        &CONSTRAINTS,
        vec![
            (constructor, MethodFlags::Public),
            (
                FunctionBuilder::new("packed", constraints_packed).returns(string, false, false),
                MethodFlags::Public,
            ),
            (
                FunctionBuilder::new("packedEnum", constraints_packed_enum)
                    .returns(string, false, false),
                MethodFlags::Public,
            ),
        ],
        keep_constraints,
    )?;
    register_class(
        &CHECKED,
        vec![(
            FunctionBuilder::new("__construct", checked_construct)
                .arg(Arg::new("value", DataType::Mixed))
                .not_required()
                .arg(Arg::new("adjusted", DataType::Bool).default("false"))
                .arg(Arg::new("enumIndex", int).allow_null().default("null")),
            MethodFlags::Public,
        )],
        keep_checked,
    )?;
    register_class(
        &VIOLATION,
        vec![
            (
                FunctionBuilder::new("__construct", violation_construct)
                    .arg(Arg::new("keyword", DataType::object("HyperCast\\Keyword")))
                    .arg(Arg::new("offset", int))
                    .arg(Arg::new("length", int)),
                MethodFlags::Public,
            ),
            (
                FunctionBuilder::new("keywordName", violation_keyword_name)
                    .returns(string, false, false),
                MethodFlags::Public,
            ),
        ],
        keep_violation,
    )?;
    if WIDENED.load(Ordering::Acquire) {
        Ok(())
    } else {
        Err(ext_php_rs::error::Error::InvalidPointer)
    }
}

zend_fastcall! {
    extern "C" fn constraints_construct(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| constraints_new(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn constraints_packed(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| packed_call(ex, false)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn constraints_packed_enum(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| packed_call(ex, true)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn checked_construct(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| checked_new(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn violation_construct(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| violation_new(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn violation_keyword_name(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| keyword_name_call(ex)), retval)));
    }
}

/// A refused declaration, the caller's bug: an `\InvalidArgumentException` whose code is the
/// core's refusal code, one message per code, worded as the core's `Refusal` displays it.
pub(crate) fn refused(refusal: Refusal) -> PhpException {
    PhpException::new(
        format!("Constraints refused (code {}): {refusal}.", refusal.code()),
        refusal.code() as i32,
        super::errors::invalid_argument_exception(),
    )
}

/// `HyperCast\Constraints::__construct(): Argument #n ($name) ...`, the engine's wording for
/// an argument refused by value.
fn argument(position: usize, name: &str, rest: &str) -> String {
    format!("HyperCast\\Constraints::__construct(): Argument #{} (${name}) {rest}", position + 1)
}

/// A bound, step or integer `enum` entry as the core converts it: an `int` or a `string`
/// through its text, a `Decimal` through its canonical text, each by `declared_decimal`.
/// `None` for any other value, which the caller words.
fn declared(value: &Zval) -> PhpResult<Option<Result<crate::Decimal, Refusal>>> {
    // An array element PHP holds by reference (`foreach ($list as &$v)` leaves them so) is
    // the value it refers to.
    let value = value.dereference();
    if let Some(n) = value.long().filter(|_| value.is_long()) {
        return Ok(Some(declared_decimal(n.to_string().as_bytes())));
    }
    if let Some(text) = value.zend_str() {
        return Ok(Some(declared_decimal(text.as_bytes())));
    }
    let is_decimal = value
        .object()
        .and_then(|o| o.get_class_name().ok())
        .is_some_and(|name| name.eq_ignore_ascii_case("HyperCast\\Decimal"));
    if is_decimal {
        let object = object_of(value).ok_or_else(|| pending("bound"))?;
        return Ok(Some(declared_decimal(&decimal_text(object)?)));
    }
    Ok(None)
}

/// A `Decimal` of the core's value, as the doors build one.
fn decimal_value(d: crate::Decimal) -> PhpResult<Zval> {
    // The C ABI's `uint64_t lo` read into a PHP int, as ext-ffi reads it: the same 64 bits.
    from_limbs(d.lo as i64, i64::from(d.hi), i64::from(d.scale), d.negative)
}

/// What a declaration says, as PHP values: the constructor's arguments, or a `Constraints`
/// object's properties.
struct Said<'a> {
    door: Door,
    bounds: [Option<&'a Zval>; 5],
    rounding: Rounding,
    min_length: Option<&'a Zval>,
    max_length: Option<&'a Zval>,
    trim: bool,
    entries: Option<&'a ZendHashTable>,
}

/// A declaration laid out as the C ABI carries it, and the core's values of its decimals and
/// integer entries.
struct Layout {
    door: crate::Door,
    raw: RawConstraints,
    packed: Vec<u8>,
    bounds: [Option<crate::Decimal>; 5],
    integers: Vec<crate::Decimal>,
}

/// Why a declaration could not be laid out: the core refused one of its values (a bound, step
/// or entry it cannot hold), or a value is not of a type the parameter takes (only through the
/// constructor; a property's declared type holds it otherwise), worded by the caller.
enum Unlaid {
    Refused(Refusal),
    Length(usize, &'static str),
    Entry(String),
}

/// Lays a declaration out: each decimal through the core's conversion, the lengths as the
/// core's `u32`s, the `enum` list packed as the ABI reads it. No rule is judged here; the core
/// judges the layout.
fn lay_out(said: &Said<'_>) -> PhpResult<Result<Layout, Unlaid>> {
    let door = crate::Door::from(said.door);
    let mut raw = RawConstraints::ZERO;
    raw.rounding = said.rounding.code();
    let mut bounds = [None; 5];
    for (i, ((_, flag), value)) in BOUNDS.iter().zip(said.bounds).enumerate() {
        let Some(value) = value.filter(|value| !value.is_null()) else {
            continue;
        };
        let decimal = match declared(value)? {
            Some(Ok(decimal)) => decimal,
            Some(Err(refusal)) => return Ok(Err(Unlaid::Refused(refusal))),
            None => return Err(contract_violation()),
        };
        match *flag {
            RawConstraints::HAS_MINIMUM | RawConstraints::HAS_EXCLUSIVE_MINIMUM => {
                raw.lower = decimal;
            }
            RawConstraints::HAS_MULTIPLE_OF => raw.multiple_of = decimal,
            _ => raw.upper = decimal,
        }
        raw.flags |= flag;
        bounds[i] = Some(decimal);
    }
    for (value, flag, position, name) in [
        (said.min_length, RawConstraints::HAS_MIN_LENGTH, 7, "minLength"),
        (said.max_length, RawConstraints::HAS_MAX_LENGTH, 8, "maxLength"),
    ] {
        let Some(value) = value.filter(|value| !value.is_null()) else {
            continue;
        };
        let Some(length) = value.long().and_then(|n| u32::try_from(n).ok()) else {
            return Ok(Err(Unlaid::Length(position, name)));
        };
        if flag == RawConstraints::HAS_MIN_LENGTH {
            raw.min_length = length;
        } else {
            raw.max_length = length;
        }
        raw.flags |= flag;
    }
    if said.trim {
        raw.flags |= RawConstraints::TRIM;
    }
    if said.entries.is_some() {
        // Declared, even with no entries: the core refuses an empty `enum` (12), never reads
        // it as none.
        raw.flags |= RawConstraints::HAS_ENUM;
    }
    let mut packed = Vec::new();
    let mut integers = Vec::new();
    for (i, entry) in said.entries.into_iter().flat_map(ZendHashTable::values).enumerate() {
        // An element PHP holds by reference is the value it refers to.
        let entry = entry.dereference();
        if door == crate::Door::Text {
            // At the text door an entry is the text itself, packed as the ABI reads it: a
            // little-endian u32 byte length, then the bytes, whatever they are (the core
            // refuses an entry that is not UTF-8).
            let Some(text) = entry.zend_str().filter(|_| entry.is_string()) else {
                return Ok(Err(Unlaid::Entry(format!(
                    "must hold only strings at HyperCast\\Door::Text; entry {i} is {}",
                    super::value_name(entry)
                ))));
            };
            let bytes = text.as_bytes();
            let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
            packed.extend_from_slice(&len.to_le_bytes());
            packed.extend_from_slice(bytes);
        } else {
            // Elsewhere a Decimal in its native 16-byte layout: lo, hi, scale, sign, two bytes
            // of padding.
            let decimal = match declared(entry)? {
                Some(Ok(decimal)) => decimal,
                Some(Err(refusal)) => return Ok(Err(Unlaid::Refused(refusal))),
                None => {
                    return Ok(Err(Unlaid::Entry(format!(
                        "must hold only int, string or HyperCast\\Decimal entries; entry {i} is {}",
                        super::value_name(entry)
                    ))));
                }
            };
            packed.extend_from_slice(&decimal.lo.to_ne_bytes());
            packed.extend_from_slice(&decimal.hi.to_ne_bytes());
            packed.extend_from_slice(&[decimal.scale, u8::from(decimal.negative), 0, 0]);
            integers.push(decimal);
        }
    }
    Ok(Ok(Layout { door, raw, packed, bounds, integers }))
}

/// The 72-byte layout, native-endian as the core reads it in place.
fn layout_bytes(raw: &RawConstraints) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(RawConstraints::SIZE as usize);
    for word in [raw.size, raw.flags, raw.rounding, raw.min_length, raw.max_length, raw._reserved] {
        bytes.extend_from_slice(&word.to_ne_bytes());
    }
    for decimal in [raw.lower, raw.upper, raw.multiple_of] {
        bytes.extend_from_slice(&decimal.lo.to_ne_bytes());
        bytes.extend_from_slice(&decimal.hi.to_ne_bytes());
        bytes.extend_from_slice(&[decimal.scale, u8::from(decimal.negative), 0, 0]);
    }
    bytes
}

/// `new Constraints(...)` from PHP: every argument checked exactly (a bound is never coerced,
/// even in weak mode: a `float` is a `TypeError`, as is a `bool` length), the declaration laid
/// out and judged by the core, then the properties written.
fn constraints_new(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    let Some(args) = frame_args(ex, 1, 11) else {
        return Err(pending("argument check"));
    };
    for arg in args.iter_mut() {
        if !exact(arg) {
            return Err(pending("argument check"));
        }
    }
    let args: Vec<Zval> = args.iter().map(Zval::shallow_clone).collect();
    let door = Door::from_zval(&args[0]).ok_or_else(contract_violation)?;
    let rounding = match args.get(6) {
        Some(zv) => Rounding::from_zval(zv).ok_or_else(contract_violation)?,
        None => Rounding::Strict,
    };
    let said = Said {
        door,
        bounds: [args.get(1), args.get(2), args.get(3), args.get(4), args.get(5)],
        rounding,
        min_length: args.get(7),
        max_length: args.get(8),
        trim: args.get(9).and_then(Zval::bool).unwrap_or(false),
        entries: args.get(10).and_then(Zval::array),
    };
    let layout = match lay_out(&said)? {
        Ok(layout) => layout,
        Err(Unlaid::Refused(refusal)) => return Err(refused(refusal)),
        Err(Unlaid::Length(position, name)) => {
            return Err(PhpException::new(
                argument(position, name, "must be between 0 and 4294967295"),
                0,
                ce::value_error(),
            ));
        }
        Err(Unlaid::Entry(rest)) => {
            return Err(PhpException::new(argument(10, "enum", &rest), 0, ce::type_error()));
        }
    };
    crate::check_constraints(layout.door, &layout.raw, &layout.packed).map_err(refused)?;

    // The properties: the door, each decimal as the exact `Decimal` the core read, the
    // rounding, the lengths, `trim`, and the `enum` entries as a list (the caller's own
    // strings at the text door, `Decimal`s elsewhere), `null` for none.
    assign(Value::Constraints, object, "door", args[0].shallow_clone())?;
    for ((name, _), bound) in BOUNDS.iter().zip(layout.bounds) {
        let value = match bound {
            Some(decimal) => decimal_value(decimal)?,
            None => null(),
        };
        assign(Value::Constraints, object, name, value)?;
    }
    let rounding = rounding.into_zval(false).map_err(|_| pending("Rounding case"))?;
    assign(Value::Constraints, object, "rounding", rounding)?;
    for (index, name) in [(7, "minLength"), (8, "maxLength")] {
        let value = args.get(index).map(Zval::shallow_clone).unwrap_or_else(null);
        assign(Value::Constraints, object, name, value)?;
    }
    assign(Value::Constraints, object, "trim", bool_zval(said.trim))?;
    let mut list = ZendHashTable::new();
    if door == Door::Text {
        for entry in said.entries.into_iter().flat_map(ZendHashTable::values) {
            // The value, never the caller's reference: a readonly declaration shares nothing
            // the caller can still write.
            list.push(entry.dereference().shallow_clone()).map_err(|_| pending("enum entry"))?;
        }
    } else {
        for decimal in layout.integers {
            list.push(decimal_value(decimal)?).map_err(|_| pending("enum entry"))?;
        }
    }
    let entries = if said.entries.is_none() {
        null()
    } else {
        let mut zv = Zval::new();
        zv.set_hashtable(list);
        zv
    };
    assign(Value::Constraints, object, "enum", entries)?;
    Ok(Zval::new())
}

/// A `Constraints` whose properties hold a declaration the constructor would not have taken.
fn not_accepted(why: &str) -> PhpException {
    super::errors::invalid_argument(format!(
        "not a declaration HyperCast\\Constraints::__construct accepts: {why}"
    ))
}

fn null() -> Zval {
    let mut zv = Zval::new();
    zv.set_null();
    zv
}

/// A `Constraints` object's declaration, read from its properties and laid out again. An
/// unwritten property is the engine's `Error`; a refused declaration (only a `Constraints` PHP
/// built around the constructor can hold one) is the refusal the constructor would have
/// thrown.
struct Declared {
    door: Door,
    layout: Layout,
    entries: Option<Zval>,
}

fn declaration(object: *mut zend_object) -> PhpResult<Declared> {
    let read = |name: &str| read(Value::Constraints, object, name);
    let door = Door::from_zval(&read("door")?).ok_or_else(contract_violation)?;
    let bounds = [
        read("minimum")?,
        read("exclusiveMinimum")?,
        read("maximum")?,
        read("exclusiveMaximum")?,
        read("multipleOf")?,
    ];
    let rounding = Rounding::from_zval(&read("rounding")?).ok_or_else(contract_violation)?;
    let min_length = read("minLength")?;
    let max_length = read("maxLength")?;
    let trim = read("trim")?.bool().unwrap_or(false);
    let entries = read("enum")?;
    let said = Said {
        door,
        bounds: [
            Some(&bounds[0]),
            Some(&bounds[1]),
            Some(&bounds[2]),
            Some(&bounds[3]),
            Some(&bounds[4]),
        ],
        rounding,
        min_length: Some(&min_length),
        max_length: Some(&max_length),
        trim,
        entries: entries.array(),
    };
    let layout = match lay_out(&said)? {
        Ok(layout) => layout,
        Err(Unlaid::Refused(refusal)) => return Err(refused(refusal)),
        // Only an object PHP built around the constructor (`unserialize`) holds one: the
        // caller's bug, worded as the constructor words it.
        Err(Unlaid::Length(_, name)) => {
            return Err(not_accepted(&format!("{name} must be between 0 and 4294967295")));
        }
        Err(Unlaid::Entry(rest)) => return Err(not_accepted(&format!("enum {rest}"))),
    };
    let entries = (!entries.is_null()).then_some(entries);
    Ok(Declared { door, layout, entries })
}

/// The declaration a door was handed, which must be bound to that door (`numeric`: any numeric
/// door; otherwise the text door).
fn declaration_for(constraints: &Zval, numeric: bool) -> PhpResult<Declared> {
    let object = object_of(constraints).ok_or_else(|| pending("constraints"))?;
    let declared = declaration(object)?;
    let is_text = declared.door == Door::Text;
    if numeric == is_text {
        let (door, method) =
            (declared.door.case_name(), if is_text { "checkedText" } else { "checked" });
        return Err(super::errors::invalid_argument(format!(
            "Constraints for HyperCast\\Door::{door} are judged by HyperCast\\Cast::{method}()"
        )));
    }
    Ok(declared)
}

/// A judge that answered `None` refused a declaration: the refusal the constructor throws, or
/// (if the full check passes) a binding bug.
fn refused_by_judge(layout: &Layout) -> PhpException {
    match crate::check_constraints(layout.door, &layout.raw, &layout.packed) {
        Err(refusal) => refused(refusal),
        Ok(()) => contract_violation(),
    }
}

/// What a checked judge answers: the value and what was reported beside it, or a rejection.
type Judgement = Result<(Zval, CheckOutcome), Rejection>;

/// Runs a numeric judge, which writes the value to the pointer it is handed.
///
/// # Safety
/// `T` must be the type the judge writes on success.
unsafe fn judged<T>(
    judge: impl FnOnce(*mut u8) -> Option<Result<CheckOutcome, Rejection>>,
) -> Option<Result<(T, CheckOutcome), Rejection>> {
    let mut out = MaybeUninit::<T>::uninit();
    judge(out.as_mut_ptr().cast()).map(|judgement| {
        // SAFETY: on success the judge wrote a `T`, per the function contract.
        judgement.map(|outcome| (unsafe { out.assume_init() }, outcome))
    })
}

/// Judges at a numeric door through `judge`, which writes that door's own type, and presents
/// the value as the plain door does: an `int` (u64 as its bit pattern), a `float` (an f32
/// widened exactly) or a `Decimal`. `None` is a refused declaration.
fn numeric(
    door: Door,
    judge: impl FnOnce(*mut u8) -> Option<Result<CheckOutcome, Rejection>>,
) -> PhpResult<Option<Judgement>> {
    macro_rules! at {
        ($ty:ty, $present:expr) => {{
            // SAFETY: the door judged is `door`, which writes this type.
            match unsafe { judged::<$ty>(judge) } {
                None => Ok(None),
                Some(Err(rejection)) => Ok(Some(Err(rejection))),
                Some(Ok((value, outcome))) => Ok(Some(Ok(($present(value)?, outcome)))),
            }
        }};
    }
    let int = |n: i64| -> PhpResult<Zval> { Ok(int_zval(n)) };
    match door {
        Door::I8 => at!(i8, |n: i8| int(n.into())),
        Door::I16 => at!(i16, |n: i16| int(n.into())),
        Door::I32 => at!(i32, |n: i32| int(n.into())),
        Door::I64 => at!(i64, int),
        Door::U8 => at!(u8, |n: u8| int(n.into())),
        Door::U16 => at!(u16, |n: u16| int(n.into())),
        Door::U32 => at!(u32, |n: u32| int(n.into())),
        // A `uint64_t` read into a PHP int, as `Cast::u64` reads it.
        Door::U64 => at!(u64, |n: u64| int(n as i64)),
        Door::F32 => at!(f32, |f: f32| -> PhpResult<Zval> { Ok(float_zval(f64::from(f))) }),
        Door::F64 => at!(f64, |f: f64| -> PhpResult<Zval> { Ok(float_zval(f)) }),
        Door::Decimal => at!(crate::Decimal, decimal_value),
        Door::Text => Err(contract_violation()),
    }
}

/// A checked verdict: `Checked`, or the door's `Fault`, or a `Violation`, its span as the core
/// reported it (bytes, as every fault's).
fn verdict(judgement: Judgement) -> PhpResult<Zval> {
    match judgement {
        Ok((value, outcome)) => build(
            Value::Checked,
            vec![
                value,
                bool_zval(outcome.adjusted),
                outcome.enum_index.map_or_else(null, |index| int_zval(i64::from(index))),
            ],
        ),
        Err(Rejection::Fault(failed)) => {
            fault(CastFailure::from(failed.reason), i64::from(failed.offset), i64::from(failed.len))
        }
        Err(Rejection::Violation(broken)) => {
            let code = broken.keyword.code();
            let keyword = match Keyword::from_code(code) {
                Some(keyword) => keyword.into_zval(false).map_err(|_| pending("Keyword case"))?,
                None => int_zval(i64::from(code)),
            };
            build(
                Value::Violation,
                vec![keyword, int_zval(i64::from(broken.offset)), int_zval(i64::from(broken.len))],
            )
        }
        // `Rejection` is the core's closed pair; nothing else reaches here.
        #[allow(unreachable_patterns)]
        Err(_) => Err(contract_violation()),
    }
}

/// `Cast::checked`: numeric text through the door the declaration is bound to, judged by it.
pub(crate) fn checked(text: &[u8], format: &NumFormat, constraints: &Zval) -> PhpResult<Zval> {
    let Declared { door, layout, .. } = declaration_for(constraints, true)?;
    let entries = Entries::packed(layout.door, &layout.packed);
    let judgement = numeric(door, |out| {
        // SAFETY: `numeric` hands a pointer to the type `door` writes.
        unsafe { judge_numeric(layout.door, text, format, &layout.raw, &entries, out) }
    })?;
    verdict(judgement.ok_or_else(|| refused_by_judge(&layout))?)
}

/// `Cast::checkedFromFloat`: a number already decoded, judged on the shortest decimal that
/// names it.
pub(crate) fn checked_from_float(value: f64, constraints: &Zval) -> PhpResult<Zval> {
    let Declared { door, layout, .. } = declaration_for(constraints, true)?;
    let entries = Entries::packed(layout.door, &layout.packed);
    let judgement = numeric(door, |out| {
        // SAFETY: `numeric` hands a pointer to the type `door` writes.
        unsafe { judge_numeric_from_f64(layout.door, value, &layout.raw, &entries, out) }
    })?;
    verdict(judgement.ok_or_else(|| refused_by_judge(&layout))?)
}

/// `Cast::checkedText`: text judged by a text declaration. The value is the judged text (the
/// input, or its trimmed span), or with an `enum` the caller's own declared entry.
pub(crate) fn checked_text(text: &[u8], constraints: &Zval) -> PhpResult<Zval> {
    let Declared { layout, entries, .. } = declaration_for(constraints, false)?;
    let packed = Entries::packed(crate::Door::Text, &layout.packed);
    let judged = judge_text(text, &layout.raw, &packed).ok_or_else(|| refused_by_judge(&layout))?;
    let judgement = match judged {
        Err(rejection) => Err(rejection),
        Ok(outcome) => {
            let declared = outcome.enum_index.and_then(|index| {
                entries
                    .as_ref()
                    .and_then(Zval::array)
                    .and_then(|list| list.values().nth(index as usize))
                    .map(Zval::shallow_clone)
            });
            let value = match (outcome.enum_index, declared) {
                (Some(_), Some(entry)) => entry,
                (Some(_), None) => return Err(contract_violation()),
                (None, _) => {
                    let (offset, len) = (outcome.span.0 as usize, outcome.span.1 as usize);
                    let span = text
                        .get(offset..offset.saturating_add(len))
                        .ok_or_else(contract_violation)?;
                    bytes_zval(span)
                }
            };
            Ok((value, outcome))
        }
    };
    verdict(judgement)
}

/// `Cast::text`: the input itself once it is valid UTF-8, a `Success`; empty input is `Empty`,
/// invalid UTF-8 `Malformed` at its span. Nothing is trimmed.
pub(crate) fn text(text: &[u8]) -> PhpResult<Zval> {
    match crate::cast_text(text) {
        Ok(valid) => success(bytes_zval(valid.as_bytes())),
        Err(failed) => {
            fault(CastFailure::from(failed.reason), i64::from(failed.offset), i64::from(failed.len))
        }
    }
}

/// `Constraints::packed()` and `Constraints::packedEnum()`.
fn packed_call(ex: &mut ExecuteData, list: bool) -> PhpResult<Zval> {
    let object = this(ex)?;
    if checked_args(ex, 0, 0).is_none() {
        return Err(pending("argument check"));
    }
    let Declared { layout, .. } = declaration(object)?;
    Ok(bytes_zval(&if list { layout.packed } else { layout_bytes(&layout.raw) }))
}

/// `new Checked($value, $adjusted = false, $enumIndex = null)` from PHP.
fn checked_new(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    let Some(args) = checked_args(ex, 1, 3) else {
        return Err(pending("argument check"));
    };
    let mut args = args.into_iter();
    let value = args.next().unwrap_or_default();
    let adjusted = args.next().unwrap_or_else(|| bool_zval(false));
    let enum_index = args.next().unwrap_or_else(null);
    assign(Value::Checked, object, "value", value)?;
    assign(Value::Checked, object, "adjusted", adjusted)?;
    assign(Value::Checked, object, "enumIndex", enum_index)?;
    Ok(Zval::new())
}

/// `new Violation($keyword, $offset, $length)` from PHP.
fn violation_new(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    let Some(args) = checked_args(ex, 3, 3) else {
        return Err(pending("argument check"));
    };
    for ((name, _), mut arg) in VIOLATION.properties.iter().zip(args) {
        // A code this binding names is its `Keyword` case, as a door builds it; any other code
        // stays the bare `int`.
        if let Some(keyword) = arg
            .long()
            .filter(|_| *name == "keyword" && arg.is_long())
            .and_then(|code| u32::try_from(code).ok().and_then(Keyword::from_code))
        {
            arg = keyword.into_zval(false).map_err(|_| pending("Keyword case"))?;
        }
        assign(Value::Violation, object, name, arg)?;
    }
    Ok(Zval::new())
}

/// `Violation::keywordName()`: the OpenAPI spelling of the keyword (`exclusiveMinimum`), or
/// `keyword N` for a code this binding does not name.
fn keyword_name_call(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    if checked_args(ex, 0, 0).is_none() {
        return Err(pending("argument check"));
    }
    let keyword = read(Value::Violation, object, "keyword")?;
    let code = match keyword.long().filter(|_| keyword.is_long()) {
        Some(code) => code,
        None => Keyword::from_zval(&keyword)
            .map(|keyword| i64::from(keyword.code()))
            .ok_or_else(contract_violation)?,
    };
    let name = match KEYWORDS.iter().find(|keyword| i64::from(keyword.code()) == code) {
        Some(keyword) => keyword.name().to_owned(),
        None => format!("keyword {code}"),
    };
    Ok(bytes_zval(name.as_bytes()))
}
