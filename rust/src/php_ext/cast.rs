//! `HyperCast\Cast`: the doors, each returning a `Success` or a `Fault` as 1.0's did, with 1.0's
//! values (its `int` for every integer width, its `Decimal`, `Duration` and
//! `\DateTimeImmutable`s) and its exceptions for a format the core refuses.

#![allow(non_snake_case)] // parameter names are PHP's, which reflection and named arguments read

use ext_php_rs::convert::IntoZval;
use ext_php_rs::error::Result as ZvalResult;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::flags::DataType;
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;

use super::args::{
    ConstraintsArg, EpochArg, Float, FormatArg, OptOrderArg, OptPeriodsArg, OrderArg, PrecisionArg,
    Str, VerdictArg,
};
use super::classes::{Value, bool_zval, bytes_zval, float_zval, int_zval, object_of, read};
use super::dates::{civil, date, instant};
use super::enums::CastFailure;
use super::errors::contract_violation;
use super::guard;
use super::num_format::declared;
use super::values::{PERIODS_PACKED_BYTES, duration, fault, from_limbs, periods_packed, success};
use crate::{CivilDateTime, Fault, NumFormat, RawDayPeriods, Timestamp};

/// A door's verdict, a `Success` or a `Fault`.
pub struct Verdict(Zval);

impl IntoZval for Verdict {
    const TYPE: DataType = DataType::object("HyperCast\\Success|HyperCast\\Fault");
    const NULLABLE: bool = false;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        *zv = self.0;
        Ok(())
    }
}

/// A checked door's verdict, a `Checked`, a `Fault` or a `Violation`.
pub struct CheckedVerdict(Zval);

impl IntoZval for CheckedVerdict {
    const TYPE: DataType =
        DataType::object("HyperCast\\Checked|HyperCast\\Fault|HyperCast\\Violation");
    const NULLABLE: bool = false;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        *zv = self.0;
        Ok(())
    }
}

/// `Cast::optional`'s answer: the verdict, or `null` for an empty input.
pub struct MaybeVerdict(Option<Zval>);

impl IntoZval for MaybeVerdict {
    const TYPE: DataType = DataType::object("HyperCast\\Success|HyperCast\\Fault");
    const NULLABLE: bool = true;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        match self.0 {
            Some(value) => *zv = value,
            None => zv.set_null(),
        }
        Ok(())
    }
}

/// The verdict for a core result: `Success` of `ok`'s value, or the fault, its span as the C
/// ABI reported it.
fn verdict<T>(
    result: Result<T, Fault>,
    ok: impl FnOnce(T) -> PhpResult<Zval>,
) -> PhpResult<Verdict> {
    let value = match result {
        Ok(value) => success(ok(value)?)?,
        Err(f) => fault(CastFailure::from(f.reason), i64::from(f.offset), i64::from(f.len))?,
    };
    Ok(Verdict(value))
}

/// A spanless door's verdict: the C ABI reports its fault at offset 0, length 0.
fn spanless<T>(
    result: Result<T, crate::Reason>,
    ok: impl FnOnce(T) -> PhpResult<Zval>,
) -> PhpResult<Verdict> {
    verdict(result.map_err(|reason| Fault { reason, offset: 0, len: 0 }), ok)
}

fn decimal_value(d: crate::Decimal) -> PhpResult<Zval> {
    // The C ABI's `uint64_t lo` read into a PHP int, as ext-ffi reads it: the same 64 bits.
    from_limbs(d.lo as i64, i64::from(d.hi), i64::from(d.scale), d.negative)
}

fn instant_value(t: Timestamp) -> PhpResult<Zval> {
    instant(t.seconds, i64::from(t.nanos))
}

fn civil_value(c: CivilDateTime) -> PhpResult<Zval> {
    civil(
        i64::from(c.date.year),
        i64::from(c.date.month),
        i64::from(c.date.day),
        c.nanos_of_day as i64,
    )
}

fn duration_value(d: crate::Duration) -> PhpResult<Zval> {
    duration(d.seconds, i64::from(d.nanos))
}

/// The day periods a `DayPeriods` object declares: its packed bytes, the first 40 as 1.0's
/// `FFI::memcpy` copied them, read as the C ABI reads them.
fn declared_periods(periods: &Zval) -> PhpResult<crate::DayPeriods> {
    let object = object_of(periods).ok_or_else(contract_violation)?;
    let bytes = periods_packed(object)?;
    let mut raw = RawDayPeriods::ZERO;
    let n = bytes.len().min(PERIODS_PACKED_BYTES);
    // SAFETY: `RawDayPeriods` is `repr(C)`, 40 bytes of integers and byte arrays, every bit
    // pattern of which is a value; at most 40 bytes are written into it.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), (&raw mut raw).cast::<u8>(), n);
    }
    raw.resolve().copied().ok_or_else(contract_violation)
}

/// An integer door's verdict, the value an `int`.
fn integer<T: Into<i64>>(
    text: &[u8],
    format: &NumFormat,
    door: fn(&[u8], &NumFormat) -> Result<T, Fault>,
) -> PhpResult<Verdict> {
    verdict(door(text, format), |n| Ok(int_zval(n.into())))
}

/// The doors: each casts a string and returns a `Success` holding the value or a `Fault`
/// naming the reason and the byte span that offended.
///
/// PHP-flavoured fidelity: `int` is 64-bit signed, so u64 carries the two's-complement bit
/// pattern (render with `sprintf('%u', ...)`); `DateTimeImmutable` tops out at microseconds,
/// so the core's nanoseconds truncate by three digits on the instant doors; time of day is an
/// exact `int` of nanoseconds since midnight; durations come back as the protobuf pair
/// (`Duration`) and decimals as the core's exact triple (`Decimal`).
#[php_class]
#[php(name = "HyperCast\\Cast")]
#[php(flags = ext_php_rs::flags::ClassFlags::Final)]
pub struct Cast;

#[php_impl]
impl Cast {
    /// Not instantiable: every door is static.
    #[php(vis = "private")]
    pub fn __construct() -> PhpResult<Self> {
        guard(|| Ok(Self))
    }

    /// The verdict, or `null` for an `Empty` fault.
    pub fn optional(verdict: VerdictArg<'_>) -> PhpResult<MaybeVerdict> {
        guard(|| {
            let is_fault = verdict
                .0
                .object()
                .and_then(|o| o.get_class_name().ok())
                .is_some_and(|name| name == "HyperCast\\Fault");
            if is_fault {
                let object = object_of(verdict.0).ok_or_else(contract_violation)?;
                // `$verdict->reason === CastFailure::Empty`: an enum case is one object.
                let reason = read(Value::Fault, object, "reason")?;
                let empty = CastFailure::Empty
                    .into_zval(false)
                    .map_err(|_| super::errors::pending("CastFailure case"))?;
                if object_of(&reason).is_some() && object_of(&reason) == object_of(&empty) {
                    return Ok(MaybeVerdict(None));
                }
            }
            Ok(MaybeVerdict(Some(verdict.0.shallow_clone())))
        })
    }

    /// `true`/`false` and their common spellings.
    pub fn bool(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| verdict(crate::cast_bool(text.0), |b| Ok(bool_zval(b))))
    }

    /// One Unicode scalar value, as its UTF-8 string.
    pub fn char(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| {
            verdict(crate::cast_char(text.0), |c| {
                let mut buffer = [0u8; 4];
                Ok(bytes_zval(c.encode_utf8(&mut buffer).as_bytes()))
            })
        })
    }

    pub fn i8(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, &declared(format.0)?, |t, f| crate::cast_i8(t, f)))
    }

    pub fn i16(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, &declared(format.0)?, |t, f| crate::cast_i16(t, f)))
    }

    pub fn i32(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, &declared(format.0)?, |t, f| crate::cast_i32(t, f)))
    }

    pub fn i64(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, &declared(format.0)?, |t, f| crate::cast_i64(t, f)))
    }

    pub fn u8(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, &declared(format.0)?, |t, f| crate::cast_u8(t, f)))
    }

    pub fn u16(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, &declared(format.0)?, |t, f| crate::cast_u16(t, f)))
    }

    pub fn u32(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| integer(text.0, &declared(format.0)?, |t, f| crate::cast_u32(t, f)))
    }

    /// A `uint64_t` read into a PHP int, as 1.0 read it: above `PHP_INT_MAX` it wraps negative.
    pub fn u64(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| {
            let format = declared(format.0)?;
            verdict(crate::cast_u64(text.0, &format), |n| Ok(int_zval(n as i64)))
        })
    }

    pub fn f32(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| {
            let format = declared(format.0)?;
            verdict(crate::cast_f32(text.0, &format), |f| Ok(float_zval(f64::from(f))))
        })
    }

    pub fn f64(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| {
            let format = declared(format.0)?;
            verdict(crate::cast_f64(text.0, &format), |f| Ok(float_zval(f)))
        })
    }

    pub fn decimal(text: Str<'_>, format: FormatArg<'_>) -> PhpResult<Verdict> {
        guard(|| {
            let format = declared(format.0)?;
            verdict(crate::cast_decimal(text.0, &format), decimal_value)
        })
    }

    /// The canonical lowercase hyphenated form.
    pub fn uuid(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| {
            verdict(crate::cast_uuid(text.0), |bytes| {
                Ok(bytes_zval(super::interop::uuid_text(&bytes).as_slice()))
            })
        })
    }

    /// The 16 bytes, RFC 9562 order.
    pub fn uuid_bytes(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| verdict(crate::cast_uuid(text.0), |bytes| Ok(bytes_zval(&bytes))))
    }

    pub fn timestamp(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| verdict(crate::cast_timestamp(text.0), instant_value))
    }

    pub fn unix(text: Str<'_>, precision: PrecisionArg) -> PhpResult<Verdict> {
        guard(|| verdict(crate::cast_unix(text.0, precision.0.into()), instant_value))
    }

    pub fn excel_serial(text: Str<'_>, epoch: EpochArg) -> PhpResult<Verdict> {
        guard(|| verdict(crate::cast_excel_serial(text.0, epoch.0.into()), instant_value))
    }

    pub fn date(text: Str<'_>, order: OptOrderArg) -> PhpResult<Verdict> {
        guard(|| {
            let result = match order.0 {
                None => crate::cast_date(text.0),
                Some(order) => crate::cast_date_ordered(text.0, order.into()),
            };
            verdict(result, |d| date(i64::from(d.year), i64::from(d.month), i64::from(d.day)))
        })
    }

    pub fn datetime(
        text: Str<'_>,
        order: OrderArg,
        periods: OptPeriodsArg<'_>,
    ) -> PhpResult<Verdict> {
        guard(|| {
            let periods = match periods.0 {
                Some(periods) => declared_periods(periods)?,
                None => crate::DayPeriods::ENGLISH,
            };
            verdict(crate::cast_datetime_with(text.0, order.0.into(), &periods), civil_value)
        })
    }

    /// Nanoseconds since midnight.
    pub fn time(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| verdict(crate::cast_time(text.0), |n| Ok(int_zval(n as i64))))
    }

    pub fn duration(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| verdict(crate::cast_duration(text.0), duration_value))
    }

    pub fn decimal_from_float(value: Float) -> PhpResult<Verdict> {
        guard(|| spanless(crate::decimal_from_f64(value.0), decimal_value))
    }

    pub fn excel_serial_from_float(value: Float, epoch: EpochArg) -> PhpResult<Verdict> {
        guard(|| spanless(crate::excel_serial(value.0, epoch.0.into()), civil_value))
    }

    /// Nanoseconds since midnight.
    pub fn excel_time(value: Float) -> PhpResult<Verdict> {
        guard(|| spanless(crate::excel_time(value.0), |n| Ok(int_zval(n as i64))))
    }

    pub fn excel_duration(value: Float) -> PhpResult<Verdict> {
        guard(|| spanless(crate::excel_duration(value.0), duration_value))
    }

    /// Text as text: the input itself once it is valid UTF-8. Empty input is `Empty`, invalid
    /// UTF-8 `Malformed` at its bytes; nothing is trimmed.
    pub fn text(text: Str<'_>) -> PhpResult<Verdict> {
        guard(|| super::constraints::text(text.0).map(Verdict))
    }

    /// Numeric text through the door its `Constraints` are bound to, under the declared format,
    /// then judged by them: `Checked`, or the door's own `Fault`, or a `Violation` of the first
    /// rule it breaks, judged exactly on the decimal the door read. Constraints for
    /// `Door::Text` go to `checkedText()`.
    pub fn checked(
        text: Str<'_>,
        format: FormatArg<'_>,
        constraints: ConstraintsArg<'_>,
    ) -> PhpResult<CheckedVerdict> {
        guard(|| {
            let format = declared(format.0)?;
            super::constraints::checked(text.0, &format, constraints.0).map(CheckedVerdict)
        })
    }

    /// A number already decoded (a JSON float, a spreadsheet's numeric cell) through the door
    /// its `Constraints` are bound to, judged on the shortest decimal that names it. NaN is
    /// `Malformed` and an infinity `OutOfRange`; every span is empty.
    pub fn checked_from_float(
        value: Float,
        constraints: ConstraintsArg<'_>,
    ) -> PhpResult<CheckedVerdict> {
        guard(|| super::constraints::checked_from_float(value.0, constraints.0).map(CheckedVerdict))
    }

    /// Text judged by `Constraints` for `Door::Text`: trimmed of Unicode `White_Space` when
    /// they say so (trimmed to nothing is `Empty`), lengths counted in code points, an `enum`
    /// matched under simple case folding, a match delivering the caller's own declared entry.
    pub fn checked_text(
        text: Str<'_>,
        constraints: ConstraintsArg<'_>,
    ) -> PhpResult<CheckedVerdict> {
        guard(|| super::constraints::checked_text(text.0, constraints.0).map(CheckedVerdict))
    }

    /// The core's version, `major.minor.patch`.
    pub fn native_version() -> PhpResult<String> {
        guard(|| Ok(super::interop::version_text(i64::from(crate::hypercast_version()))))
    }
}
