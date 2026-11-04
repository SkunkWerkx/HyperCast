//! `DateOptions` and `MonthNames`: 1.1's opt-in date shapes, registered as plain `final
//! readonly` classes like the other value classes (`classes.rs`), and the reading of them the
//! date doors do (`Declared`, which `Cast` and `DetailedCast` take their `ValidDateOptions`
//! from).
//!
//! A declaration is judged by the core's own functions, `DateOptions::validate` and
//! `MonthNames::from_packed`, when it is built and again on every door call (the core keeps no
//! state, and an object PHP built around the constructor, `unserialize` say, is judged like any
//! other); no rule is restated here, only worded.

use std::panic::AssertUnwindSafe;

use ext_php_rs::args::{Arg, ArgInfoTables};
use ext_php_rs::builders::FunctionBuilder;
use ext_php_rs::exception::{PhpException, PhpResult};
use ext_php_rs::ffi::zend_object;
use ext_php_rs::flags::{DataType, MethodFlags};
use ext_php_rs::types::{ArrayKey, ZendHashTable, Zval};
use ext_php_rs::zend::{ClassEntry, ExecuteData, ce, run_handler};
use ext_php_rs::zend_fastcall;

use super::classes::{
    Declaration, Type, Value, assign, build, bytes_zval, finish, finish_registration, int_zval,
    object_of, read, read_int, register as register_class, this,
};
use super::errors::{contract_violation, invalid_argument, pending};
use super::guard;
use super::values::checked_args;
use crate::ValidDateOptions;
use crate::{
    DateOptions as CoreOptions, DeclarationError, MonthNames as CoreNames, MonthNamesFault,
};

const COMPACT_DATE: i64 = 1;
const SPACE_SEPARATOR: i64 = 2;
const HOUR_OFFSET: i64 = 4;
const COMMON_FORMS: i64 = COMPACT_DATE | SPACE_SEPARATOR | HOUR_OFFSET;

/// A month name's longest length in bytes, which the core refuses to exceed.
const MAX_NAME_BYTES: i64 = 64;

const DATE_OPTIONS: Declaration = Declaration {
    name: "HyperCast\\DateOptions",
    properties: &[("flags", Type::Int), ("pivot", Type::Int)],
    constants: &[
        ("COMPACT_DATE", COMPACT_DATE),
        ("SPACE_SEPARATOR", SPACE_SEPARATOR),
        ("HOUR_OFFSET", HOUR_OFFSET),
        ("COMMON_FORMS", COMMON_FORMS),
    ],
    promoted: true,
};

const MONTH_NAMES: Declaration = Declaration {
    name: "HyperCast\\MonthNames",
    properties: &[("months", Type::Array)],
    constants: &[("MAX_NAME_BYTES", MAX_NAME_BYTES)],
    promoted: true,
};

fn keep_date_options(ce: &'static mut ClassEntry, tables: ArgInfoTables) {
    std::mem::forget(tables);
    finish_registration(Value::DateOptions, &DATE_OPTIONS, ce);
}

fn keep_month_names(ce: &'static mut ClassEntry, tables: ArgInfoTables) {
    std::mem::forget(tables);
    finish_registration(Value::MonthNames, &MONTH_NAMES, ce);
}

/// Registers the two classes. MINIT.
pub(crate) fn register() -> ext_php_rs::error::Result<()> {
    let int = DataType::Long;
    register_class(
        &DATE_OPTIONS,
        vec![(
            FunctionBuilder::new("__construct", date_options_construct)
                .not_required()
                .arg(Arg::new("flags", int).default("0"))
                .arg(Arg::new("pivot", int).default("0")),
            MethodFlags::Public,
        )],
        keep_date_options,
    )?;
    register_class(
        &MONTH_NAMES,
        vec![
            (
                FunctionBuilder::new("__construct", month_names_construct)
                    .arg(Arg::new("months", DataType::Array)),
                MethodFlags::Public,
            ),
            (
                FunctionBuilder::new("fromPacked", month_names_from_packed)
                    .arg(Arg::new("packed", DataType::String))
                    .returns(DataType::object("HyperCast\\MonthNames"), false, false),
                MethodFlags::Public | MethodFlags::Static,
            ),
            (
                FunctionBuilder::new("packed", month_names_packed).returns(
                    DataType::String,
                    false,
                    false,
                ),
                MethodFlags::Public,
            ),
        ],
        keep_month_names,
    )
}

zend_fastcall! {
    extern "C" fn date_options_construct(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| date_options_new(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn month_names_construct(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| month_names_new(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn month_names_from_packed(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| from_packed_call(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn month_names_packed(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| packed_call(ex)), retval)));
    }
}

/// The core's options for the two integers, or the core's refusal of them. A flag word no
/// `u32` holds has an undefined bit; a pivot no `u16` holds saturates to `u16::MAX`, which the
/// core's pivot rule refuses like any pivot past 9999. The flags are judged first.
fn core_options(flags: i64, pivot: i64) -> Result<CoreOptions, DeclarationError> {
    let flags = u32::try_from(flags).map_err(|_| DeclarationError::DateFlags)?;
    let options =
        CoreOptions::from_flags(flags).with_pivot(u16::try_from(pivot).unwrap_or(u16::MAX));
    options.validate(None).map(|_| options)
}

/// `new DateOptions($flags = 0, $pivot = 0)`: the promoted assignments, then the core's rules
/// (an undefined flag bit first, then the pivot).
fn date_options_new(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    let Some(args) = checked_args(ex, 0, 2) else {
        return Err(pending("argument check"));
    };
    let int = |i: usize| args.get(i).and_then(Zval::long).unwrap_or(0);
    let (flags, pivot) = (int(0), int(1));
    assign(Value::DateOptions, object, "flags", int_zval(flags))?;
    assign(Value::DateOptions, object, "pivot", int_zval(pivot))?;
    match core_options(flags, pivot) {
        Ok(_) => Ok(Zval::new()),
        Err(DeclarationError::DatePivot) => Err(invalid_argument(format!(
            "Date pivot must be 0 (none) or 100 through 9999; got {pivot}"
        ))),
        Err(_) => Err(invalid_argument(format!(
            "Date option flags must name only COMPACT_DATE, SPACE_SEPARATOR and HOUR_OFFSET; got {flags}"
        ))),
    }
}

/// The message each rule of a refused vocabulary carries, worded in this binding's style.
fn names_message(fault: MonthNamesFault) -> &'static str {
    match fault {
        MonthNamesFault::Layout => "Month names are cut short or have bytes after the last entry",
        MonthNamesFault::Month => "A month name must belong to a month from 1 to 12",
        MonthNamesFault::Utf8 => "A month name must be valid UTF-8",
        MonthNamesFault::NameLength => "A month name must be one to 64 bytes",
        MonthNamesFault::Character => {
            "A month name must not contain an ASCII digit, '/', '-' or white space"
        }
        MonthNamesFault::MonthMissing => "Every month from 1 to 12 needs at least one name",
        MonthNamesFault::Duplicate => "Two month names fold to the same spelling",
    }
}

/// The refusal of a vocabulary the core judged, as an exception.
fn refused_names(error: DeclarationError) -> PhpException {
    match error {
        DeclarationError::MonthNames(fault) => invalid_argument(names_message(fault)),
        _ => contract_violation(),
    }
}

/// The packed layout's entries in order, as `(month byte, name)`; the bytes are known whole
/// (the core judged them), so a short read ends the walk.
fn entries(packed: &[u8]) -> Vec<(u8, &[u8])> {
    let mut out = Vec::new();
    let mut rest = packed;
    while let [month, a, b, c, d, tail @ ..] = rest {
        let len = u32::from_le_bytes([*a, *b, *c, *d]) as usize;
        let Some((name, after)) = tail.split_at_checked(len) else {
            break;
        };
        out.push((*month, name));
        rest = after;
    }
    out
}

/// A vocabulary judged by the core: its names as the property holds them, a list per month, the
/// months in the order they first appear.
fn judged_months(packed: &[u8]) -> PhpResult<Zval> {
    CoreNames::from_packed(packed).map_err(refused_names)?;
    let mut order: Vec<u8> = Vec::new();
    let mut names: Vec<Vec<&[u8]>> = Vec::new();
    for (month, name) in entries(packed) {
        match order.iter().position(|seen| *seen == month) {
            Some(at) => names[at].push(name),
            None => {
                order.push(month);
                names.push(vec![name]);
            }
        }
    }
    let mut months = ZendHashTable::new();
    for (month, names) in order.into_iter().zip(names) {
        let mut list = ZendHashTable::new();
        for name in names {
            list.push(bytes_zval(name)).map_err(|_| pending("month name"))?;
        }
        let mut zv = Zval::new();
        zv.set_hashtable(list);
        months.insert_at_index(i64::from(month), zv).map_err(|_| pending("month names"))?;
    }
    let mut zv = Zval::new();
    zv.set_hashtable(months);
    Ok(zv)
}

/// What a month's entry (a string or a list of strings) holds, as bytes; `None` for anything
/// else.
fn names_of(value: &Zval) -> Option<Vec<Vec<u8>>> {
    let value = value.dereference();
    if let Some(text) = value.zend_str().filter(|_| value.is_string()) {
        return Some(vec![text.as_bytes().to_vec()]);
    }
    let list = value.array()?;
    list.values()
        .map(|name| {
            let name = name.dereference();
            name.zend_str().filter(|_| name.is_string()).map(|s| s.as_bytes().to_vec())
        })
        .collect()
}

/// The packed layout of a constructor's `array`: each month's names, a month the byte cannot
/// hold packed as 0 (which the core refuses as no month).
fn pack_array(months: &ZendHashTable) -> PhpResult<Vec<u8>> {
    let mut packed = Vec::new();
    for (key, value) in months {
        let month = match key {
            ArrayKey::Long(month) => month,
            other => {
                return Err(PhpException::new(
                    format!(
                        "HyperCast\\MonthNames::__construct(): Argument #1 ($months) must be keyed by month number; got the key '{}'",
                        match other {
                            ArrayKey::String(key) => key,
                            ArrayKey::Str(key) => key.to_owned(),
                            _ => String::new(),
                        }
                    ),
                    0,
                    ce::type_error(),
                ));
            }
        };
        let Some(names) = names_of(value) else {
            return Err(PhpException::new(
                format!(
                    "HyperCast\\MonthNames::__construct(): Argument #1 ($months) must map each month to a string or a list of strings; month {month} holds {}",
                    super::value_name(value.dereference())
                ),
                0,
                ce::type_error(),
            ));
        };
        let byte = u8::try_from(month).unwrap_or(0);
        for name in names {
            packed.push(byte);
            packed.extend_from_slice(&u32::try_from(name.len()).unwrap_or(u32::MAX).to_le_bytes());
            packed.extend_from_slice(&name);
        }
    }
    Ok(packed)
}

/// `new MonthNames(array $months)`: `[month => name or list of names]`, judged by the core.
fn month_names_new(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    let Some(args) = checked_args(ex, 1, 1) else {
        return Err(pending("argument check"));
    };
    let months = args[0].array().ok_or_else(|| pending("months"))?;
    let months = judged_months(&pack_array(months)?)?;
    assign(Value::MonthNames, object, "months", months)?;
    Ok(Zval::new())
}

/// `MonthNames::fromPacked(string $packed)`: the layout `packed()` writes, judged by the core.
fn from_packed_call(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let Some(args) = checked_args(ex, 1, 1) else {
        return Err(pending("argument check"));
    };
    let packed = args[0].zend_str().map(|s| s.as_bytes().to_vec()).unwrap_or_default();
    build(Value::MonthNames, vec![judged_months(&packed)?])
}

/// A `MonthNames` object's names in the packed layout, month by month, as its property holds
/// them.
fn names_packed(object: *mut zend_object) -> PhpResult<Vec<u8>> {
    let months = read(Value::MonthNames, object, "months")?;
    let table = months.array().ok_or_else(contract_violation)?;
    let mut packed = Vec::new();
    for (key, value) in table {
        let ArrayKey::Long(month) = key else {
            return Err(contract_violation());
        };
        let names = names_of(value).ok_or_else(contract_violation)?;
        let byte = u8::try_from(month).unwrap_or(0);
        for name in names {
            packed.push(byte);
            packed.extend_from_slice(&u32::try_from(name.len()).unwrap_or(u32::MAX).to_le_bytes());
            packed.extend_from_slice(&name);
        }
    }
    Ok(packed)
}

fn packed_call(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    if checked_args(ex, 0, 0).is_none() {
        return Err(pending("argument check"));
    }
    Ok(bytes_zval(&names_packed(object)?))
}

/// The date options and vocabulary a door was handed, laid out for the core: the options as
/// the core takes them, and the vocabulary packed (the core borrows it for the call).
pub(crate) struct Declared {
    options: CoreOptions,
    packed: Option<Vec<u8>>,
}

impl Declared {
    /// What `options` and `months` declare, or `None` when a door was handed neither. A
    /// `DateOptions` or `MonthNames` PHP built around the constructor, `unserialize` say, is
    /// judged here like any other: the core's refusal is the caller's contract violation.
    pub(crate) fn read(
        options: Option<&Zval>,
        months: Option<&Zval>,
    ) -> PhpResult<Option<Declared>> {
        if options.is_none() && months.is_none() {
            return Ok(None);
        }
        let core = match options {
            None => CoreOptions::NONE,
            Some(zv) => {
                let object = object_of(zv).ok_or_else(|| pending("date options"))?;
                let flags = read_int(Value::DateOptions, object, "flags")?;
                let pivot = read_int(Value::DateOptions, object, "pivot")?;
                core_options(flags, pivot).map_err(|_| contract_violation())?
            }
        };
        let packed = match months {
            None => None,
            Some(zv) => {
                let object = object_of(zv).ok_or_else(|| pending("month names"))?;
                Some(names_packed(object)?)
            }
        };
        Ok(Some(Declared { options: core, packed }))
    }

    /// The core's validated declaration, borrowing the packed vocabulary for the call.
    pub(crate) fn valid(&self) -> PhpResult<ValidDateOptions<'_>> {
        let months = match &self.packed {
            None => None,
            Some(packed) => Some(CoreNames::from_packed(packed).map_err(|_| contract_violation())?),
        };
        self.options.validate(months).map_err(|_| contract_violation())
    }
}

/// Runs a date door under what `options` and `months` declare, the core's validated options
/// (`None` when the door was handed neither, which reads the door's plain shape).
pub(crate) fn with_valid<T>(
    options: Option<&Zval>,
    months: Option<&Zval>,
    run: impl FnOnce(Option<&ValidDateOptions<'_>>) -> PhpResult<T>,
) -> PhpResult<T> {
    match Declared::read(options, months)? {
        None => run(None),
        Some(declared) => {
            let valid = declared.valid()?;
            run(Some(&valid))
        }
    }
}
