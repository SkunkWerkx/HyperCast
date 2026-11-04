//! `DetailedFault` and `DetailedCast`: 1.1's fault detail, on a result path of its own.
//!
//! 1.0's `Fault` is `final readonly` with three promoted properties, and PHP compares objects
//! property by property (`==`, `assertEquals`, `serialize`). A `detail` on it would change what
//! every existing fault equals, so `Fault` is left exactly as it was and `DetailedCast`, a twin
//! of `Cast` over the core's `detailed` doors, returns a `DetailedFault` (the same reason and
//! span, the raw code, and the named `FaultDetail`) beside the same `Success`.

#![allow(non_snake_case)] // parameter names are PHP's, which reflection and named arguments read

use std::panic::AssertUnwindSafe;

use ext_php_rs::args::{Arg, ArgInfoTables};
use ext_php_rs::builders::FunctionBuilder;
use ext_php_rs::convert::IntoZval;
use ext_php_rs::error::Result as ZvalResult;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::flags::{DataType, MethodFlags};
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;
use ext_php_rs::zend::{ClassEntry, ExecuteData, run_handler};
use ext_php_rs::zend_fastcall;

use super::args::{
    ConstraintsArg, DetailedVerdictArg, EpochArg, Float, FormatArg, OptDateOptionsArg,
    OptMonthNamesArg, OptOrderArg, OptPeriodsArg, OrderArg, PrecisionArg, Str,
};
use super::cast::{civil_value, decimal_value, declared_periods, duration_value, instant_value};
use super::classes::{
    Declaration, Type, Value, assign, bool_zval, build, bytes_zval, finish, finish_registration,
    float_zval, int_zval, object_of, read, register as register_class, this,
};
use super::date_options::with_valid;
use super::dates::date;
use super::enums::{CastFailure, FaultDetail};
use super::errors::{contract_violation, pending};
use super::guard;
use super::num_format::declared;
use super::values::{checked_args, success};
use crate::{Failure, ValidNumFormat};

const DETAILED_FAULT: Declaration = Declaration {
    name: "HyperCast\\DetailedFault",
    properties: &[
        ("reason", Type::Class("HyperCast\\CastFailure")),
        ("offset", Type::Int),
        ("length", Type::Int),
        ("code", Type::Int),
        ("detail", Type::Class("HyperCast\\FaultDetail")),
    ],
    constants: &[],
    promoted: false,
};

fn keep_detailed_fault(ce: &'static mut ClassEntry, tables: ArgInfoTables) {
    std::mem::forget(tables);
    finish_registration(Value::DetailedFault, &DETAILED_FAULT, ce);
}

/// Registers `DetailedFault`. MINIT.
pub(crate) fn register() -> ext_php_rs::error::Result<()> {
    register_class(
        &DETAILED_FAULT,
        vec![
            (
                FunctionBuilder::new("__construct", detailed_fault_construct)
                    .arg(Arg::new("reason", DataType::object("HyperCast\\CastFailure")))
                    .arg(Arg::new("offset", DataType::Long))
                    .arg(Arg::new("length", DataType::Long))
                    .arg(Arg::new("code", DataType::Long)),
                MethodFlags::Public,
            ),
            (
                FunctionBuilder::new("toFault", detailed_fault_to_fault).returns(
                    DataType::object("HyperCast\\Fault"),
                    false,
                    false,
                ),
                MethodFlags::Public,
            ),
        ],
        keep_detailed_fault,
    )
}

zend_fastcall! {
    extern "C" fn detailed_fault_construct(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| detailed_fault_new(ex)), retval)));
    }
}

zend_fastcall! {
    extern "C" fn detailed_fault_to_fault(ex: &mut ExecuteData, retval: &mut Zval) {
        run_handler(AssertUnwindSafe(|| finish(guard(|| to_fault_call(ex)), retval)));
    }
}

/// The named detail of a raw code: its case, or `Unknown` for a code not published (a newer
/// core's, or a constructed one), never an error.
fn detail_zval(code: i64) -> PhpResult<Zval> {
    FaultDetail::from_code(u32::try_from(code).unwrap_or(0))
        .into_zval(false)
        .map_err(|_| pending("FaultDetail case"))
}

/// `new DetailedFault($reason, $offset, $length, $code)` from PHP: the four assignments, then
/// the case the code names.
fn detailed_fault_new(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    let Some(args) = checked_args(ex, 4, 4) else {
        return Err(pending("argument check"));
    };
    let code = args[3].long().unwrap_or_default();
    for ((name, _), arg) in DETAILED_FAULT.properties.iter().zip(args) {
        assign(Value::DetailedFault, object, name, arg)?;
    }
    assign(Value::DetailedFault, object, "detail", detail_zval(code)?)?;
    Ok(Zval::new())
}

/// `new DetailedFault(...)`, built by the binding for a core failure.
pub(crate) fn detailed_fault(failure: Failure) -> PhpResult<Zval> {
    let reason = CastFailure::from(failure.reason)
        .into_zval(false)
        .map_err(|_| pending("CastFailure case"))?;
    let code = i64::from(failure.detail.code());
    build(
        Value::DetailedFault,
        vec![
            reason,
            int_zval(i64::from(failure.offset)),
            int_zval(i64::from(failure.len)),
            int_zval(code),
            detail_zval(code)?,
        ],
    )
}

/// `$fault->toFault()`: the plain `Fault` of the same reason and span.
fn to_fault_call(ex: &mut ExecuteData) -> PhpResult<Zval> {
    let object = this(ex)?;
    if checked_args(ex, 0, 0).is_none() {
        return Err(pending("argument check"));
    }
    let get = |name: &str| read(Value::DetailedFault, object, name);
    build(Value::Fault, vec![get("reason")?, get("offset")?, get("length")?])
}

/// A detailed door's verdict, a `Success` or a `DetailedFault`.
pub struct Verdict(Zval);

impl IntoZval for Verdict {
    const TYPE: DataType = DataType::object("HyperCast\\Success|HyperCast\\DetailedFault");
    const NULLABLE: bool = false;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        *zv = self.0;
        Ok(())
    }
}

/// A detailed checked door's verdict, a `Checked`, a `DetailedFault` or a `Violation`.
pub struct CheckedVerdict(Zval);

impl IntoZval for CheckedVerdict {
    const TYPE: DataType =
        DataType::object("HyperCast\\Checked|HyperCast\\DetailedFault|HyperCast\\Violation");
    const NULLABLE: bool = false;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        *zv = self.0;
        Ok(())
    }
}

/// `DetailedCast::optional`'s answer: the verdict, or `null` for an empty input.
pub struct MaybeVerdict(Option<Zval>);

impl IntoZval for MaybeVerdict {
    const TYPE: DataType = DataType::object(
        "HyperCast\\Success|HyperCast\\Checked|HyperCast\\DetailedFault|HyperCast\\Violation",
    );
    const NULLABLE: bool = true;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        match self.0 {
            Some(value) => *zv = value,
            None => zv.set_null(),
        }
        Ok(())
    }
}

/// The verdict for a core result: `Success` of `ok`'s value, or the detailed fault.
fn verdict<T>(
    result: Result<T, Failure>,
    ok: impl FnOnce(T) -> PhpResult<Zval>,
) -> PhpResult<Verdict> {
    Ok(Verdict(match result {
        Ok(value) => success(ok(value)?)?,
        Err(failure) => detailed_fault(failure)?,
    }))
}

/// The format a door declares, validated for the core's `detailed` doors. A format the core
/// refuses is the caller's contract violation, as at every `Cast` door.
fn valid_format(format: &Zval) -> PhpResult<ValidNumFormat> {
    declared(format)?.validate().map_err(|_| contract_violation())
}

/// An integer door's verdict, the value an `int`.
fn integer<T: Into<i64>>(
    text: &[u8],
    format: &Zval,
    door: fn(&[u8], &ValidNumFormat) -> Result<T, Failure>,
) -> PhpResult<Verdict> {
    let format = valid_format(format)?;
    verdict(door(text, &format), |n| Ok(int_zval(n.into())))
}

/// The doors again, each with its detail: a `Success` holding the value, the same one `Cast`
/// returns, or a `DetailedFault` naming the reason, the byte span, and what exactly failed.
/// Every door reads exactly as its `Cast` twin does: the two are one parse.
#[php_class]
#[php(name = "HyperCast\\DetailedCast")]
#[php(flags = ext_php_rs::flags::ClassFlags::Final)]
pub struct DetailedCast;

#[php_impl]
impl DetailedCast {
    /// Not instantiable: every door is static.
    #[php(vis = "private")]
    pub fn __construct() -> PhpResult<Self> {
        guard(|| Ok(Self))
    }

    /// The verdict, plain or checked, or `null` for an `Empty` fault: every other verdict, a
    /// `Violation` included, comes back as it was.
    pub fn optional(verdict: DetailedVerdictArg<'_>) -> PhpResult<MaybeVerdict> {
        guard(|| {
            let is_fault = verdict
                .0
                .object()
                .and_then(|o| o.get_class_name().ok())
                .is_some_and(|name| name == "HyperCast\\DetailedFault");
            if is_fault {
                let object = object_of(verdict.0).ok_or_else(contract_violation)?;
                let reason = read(Value::DetailedFault, object, "reason")?;
                let empty =
                    CastFailure::Empty.into_zval(false).map_err(|_| pending("CastFailure case"))?;
                if object_of(&reason).is_some() && object_of(&reason) == object_of(&empty) {
                    return Ok(MaybeVerdict(None));
                }
            }
            Ok(MaybeVerdict(Some(verdict.0.shallow_clone())))
        })
    }

    /// `true`/`false` and their common spellings.
    pub fn bool(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| verdict(crate::detailed::cast_bool(text.0), |b| Ok(bool_zval(b))))
    }

    /// One Unicode scalar value, as its UTF-8 string.
    pub fn char(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| {
            verdict(crate::detailed::cast_char(text.0), |c| {
                let mut buffer = [0u8; 4];
                Ok(bytes_zval(c.encode_utf8(&mut buffer).as_bytes()))
            })
        })
    }

    pub fn i8(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, format.0, |t, f| crate::detailed::cast_i8(t, f)))
    }

    pub fn i16(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, format.0, |t, f| crate::detailed::cast_i16(t, f)))
    }

    pub fn i32(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, format.0, |t, f| crate::detailed::cast_i32(t, f)))
    }

    pub fn i64(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, format.0, |t, f| crate::detailed::cast_i64(t, f)))
    }

    pub fn u8(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, format.0, |t, f| crate::detailed::cast_u8(t, f)))
    }

    pub fn u16(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, format.0, |t, f| crate::detailed::cast_u16(t, f)))
    }

    pub fn u32(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, format.0, |t, f| crate::detailed::cast_u32(t, f)))
    }

    /// A `uint64_t` read into a PHP int, as `Cast::u64` reads it: above `PHP_INT_MAX` it wraps
    /// negative.
    pub fn u64(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| {
            let format = valid_format(format.0)?;
            verdict(crate::detailed::cast_u64(text.0, &format), |n| Ok(int_zval(n as i64)))
        })
    }

    pub fn f32(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| {
            let format = valid_format(format.0)?;
            verdict(crate::detailed::cast_f32(text.0, &format), |f| Ok(float_zval(f64::from(f))))
        })
    }

    pub fn f64(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| {
            let format = valid_format(format.0)?;
            verdict(crate::detailed::cast_f64(text.0, &format), |f| Ok(float_zval(f)))
        })
    }

    pub fn decimal(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| {
            let format = valid_format(format.0)?;
            verdict(crate::detailed::cast_decimal(text.0, &format), decimal_value)
        })
    }

    /// The canonical lowercase hyphenated form.
    pub fn uuid(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| {
            verdict(crate::detailed::cast_uuid(text.0), |bytes| {
                Ok(bytes_zval(super::interop::uuid_text(&bytes).as_slice()))
            })
        })
    }

    /// The 16 bytes, RFC 9562 order.
    pub fn uuid_bytes(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| verdict(crate::detailed::cast_uuid(text.0), |bytes| Ok(bytes_zval(&bytes))))
    }

    /// With `DateOptions`, the shapes they open: `SPACE_SEPARATOR` and `HOUR_OFFSET`.
    pub fn timestamp(text: Str<'_>, options: OptDateOptionsArg<'_>) -> PhpResult<Verdict> {
        guard(|| {
            with_valid(options.0, None, |valid| {
                verdict(crate::detailed::cast_timestamp(text.0, valid), instant_value)
            })
        })
    }

    pub fn unix(text: Str<'_>, precision: PrecisionArg) -> PhpResult<Verdict> {
        guard(|| verdict(crate::detailed::cast_unix(text.0, precision.0.into()), instant_value))
    }

    pub fn excel_serial(text: Str<'_>, epoch: EpochArg) -> PhpResult<Verdict> {
        guard(|| verdict(crate::detailed::cast_excel_serial(text.0, epoch.0.into()), instant_value))
    }

    /// With `DateOptions` and `MonthNames`, the shapes they open: `COMPACT_DATE` without an
    /// order; with one, the pivot and the month names.
    pub fn date(
        text: Str<'_>,
        order: OptOrderArg,
        options: OptDateOptionsArg<'_>,
        months: OptMonthNamesArg<'_>,
    ) -> PhpResult<Verdict> {
        guard(|| {
            with_valid(options.0, months.0, |valid| {
                let result = match order.0 {
                    None => crate::detailed::cast_date(text.0, valid),
                    Some(order) => crate::detailed::cast_date_ordered(text.0, order.into(), valid),
                };
                verdict(result, |d| date(i64::from(d.year), i64::from(d.month), i64::from(d.day)))
            })
        })
    }

    /// With `DateOptions` and `MonthNames`, the pivot and the month names.
    pub fn datetime(
        text: Str<'_>,
        order: OrderArg,
        periods: OptPeriodsArg<'_>,
        options: OptDateOptionsArg<'_>,
        months: OptMonthNamesArg<'_>,
    ) -> PhpResult<Verdict> {
        guard(|| {
            let periods = match periods.0 {
                Some(periods) => declared_periods(periods)?,
                None => crate::DayPeriods::ENGLISH,
            };
            with_valid(options.0, months.0, |valid| {
                verdict(
                    crate::detailed::cast_datetime_with(text.0, order.0.into(), &periods, valid),
                    civil_value,
                )
            })
        })
    }

    /// Nanoseconds since midnight.
    pub fn time(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| verdict(crate::detailed::cast_time(text.0), |n| Ok(int_zval(n as i64))))
    }

    pub fn duration(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| verdict(crate::detailed::cast_duration(text.0), duration_value))
    }

    pub fn decimal_from_float(value: Float) -> PhpResult<Verdict> {
        guard(|| verdict(crate::detailed::decimal_from_f64(value.0), decimal_value))
    }

    pub fn excel_serial_from_float(value: Float, epoch: EpochArg) -> PhpResult<Verdict> {
        guard(|| verdict(crate::detailed::excel_serial(value.0, epoch.0.into()), civil_value))
    }

    /// Nanoseconds since midnight.
    pub fn excel_time(value: Float) -> PhpResult<Verdict> {
        guard(|| verdict(crate::detailed::excel_time(value.0), |n| Ok(int_zval(n as i64))))
    }

    pub fn excel_duration(value: Float) -> PhpResult<Verdict> {
        guard(|| verdict(crate::detailed::excel_duration(value.0), duration_value))
    }

    /// Text as text: the input itself once it is valid UTF-8. Empty input is `Empty`, invalid
    /// UTF-8 `Malformed` at its bytes; nothing is trimmed.
    pub fn text(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| super::constraints::text_detailed(text.0).map(Verdict))
    }

    /// Numeric text through the door its `Constraints` are bound to, then judged by them:
    /// `Checked`, or the door's own `DetailedFault`, or a `Violation` (which carries no detail).
    pub fn checked(
        text: Str<'_>,
        format: FormatArg<'_>,
        constraints: ConstraintsArg<'_>,
    ) -> PhpResult<CheckedVerdict> {
        guard(|| {
            let format = declared(format.0)?;
            super::constraints::checked_detailed(text.0, &format, constraints.0).map(CheckedVerdict)
        })
    }

    /// A number already decoded through the door its `Constraints` are bound to. Every span is
    /// empty.
    pub fn checked_from_float(
        value: Float,
        constraints: ConstraintsArg<'_>,
    ) -> PhpResult<CheckedVerdict> {
        guard(|| {
            super::constraints::checked_from_float_detailed(value.0, constraints.0)
                .map(CheckedVerdict)
        })
    }

    /// Text judged by `Constraints` for `Door::Text`.
    pub fn checked_text(
        text: Str<'_>,
        constraints: ConstraintsArg<'_>,
    ) -> PhpResult<CheckedVerdict> {
        guard(|| {
            super::constraints::checked_text_detailed(text.0, constraints.0).map(CheckedVerdict)
        })
    }
}
