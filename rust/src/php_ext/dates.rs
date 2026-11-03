//! The `\DateTimeImmutable` values the doors return, built by PHP exactly as 1.0's
//! `Interop\NativeValues` built them, with 1.0's integer arithmetic (PHP's: an `int` that
//! overflows becomes a `float`, which 1.0's strict-typed file then refused).

use ext_php_rs::exception::PhpResult;
use ext_php_rs::ffi::{zend_class_entry, zval};
use ext_php_rs::types::Zval;
use ext_php_rs::zend::{ClassEntry, ExecutorGlobals};

use super::classes::{bytes_zval, call, has_method, int_zval};
use super::errors::{pending, type_error};

// `object_init_ex` (`zend_API.h`): `new`'s allocation, the class's own `create_object` included.
#[cfg_attr(all(windows, not(php_zts)), link(name = "php8"))]
#[cfg_attr(all(windows, php_zts), link(name = "php8ts"))]
unsafe extern "C" {
    fn object_init_ex(arg: *mut zval, class_type: *mut zend_class_entry) -> i32;
}

/// A number as PHP holds the result of `int` arithmetic.
#[derive(Clone, Copy)]
enum Num {
    Int(i64),
    Float(f64),
}

impl Num {
    fn as_f64(self) -> f64 {
        match self {
            Num::Int(n) => n as f64,
            Num::Float(f) => f,
        }
    }

    fn op(self, other: Num, int: fn(i64, i64) -> Option<i64>, float: fn(f64, f64) -> f64) -> Num {
        match (self, other) {
            (Num::Int(a), Num::Int(b)) => match int(a, b) {
                Some(n) => Num::Int(n),
                None => Num::Float(float(a as f64, b as f64)),
            },
            (a, b) => Num::Float(float(a.as_f64(), b.as_f64())),
        }
    }

    fn add(self, other: Num) -> Num {
        self.op(other, i64::checked_add, |a, b| a + b)
    }

    fn sub(self, other: Num) -> Num {
        self.op(other, i64::checked_sub, |a, b| a - b)
    }

    fn mul(self, other: Num) -> Num {
        self.op(other, i64::checked_mul, |a, b| a * b)
    }

    fn ge_zero(self) -> bool {
        self.as_f64() >= 0.0
    }
}

/// `intdiv($n, $d)` from a strict-typed file: a `float` is refused.
fn intdiv(n: Num, d: i64) -> PhpResult<Num> {
    match n {
        Num::Int(n) => Ok(Num::Int(n / d)),
        Num::Float(_) => {
            Err(type_error("intdiv(): Argument #1 ($num1) must be of type int, float given"))
        }
    }
}

/// 1.0's `NativeValues::epochSeconds` (Howard Hinnant's `days_from_civil`, times a day).
fn epoch_seconds(year: i64, month: i64, day: i64) -> PhpResult<i64> {
    let int = Num::Int;
    let shifted = if month <= 2 { int(year).sub(int(1)) } else { int(year) };
    let era = intdiv(if shifted.ge_zero() { shifted } else { shifted.sub(int(399)) }, 400)?;
    let year_of_era = shifted.sub(era.mul(int(400)));
    let day_of_year =
        intdiv(int(153).mul(int(month).add(int(if month > 2 { -3 } else { 9 }))).add(int(2)), 5)?
            .add(int(day))
            .sub(int(1));
    let day_of_era = year_of_era
        .mul(int(365))
        .add(intdiv(year_of_era, 4)?)
        .sub(intdiv(year_of_era, 100)?)
        .add(day_of_year);
    match era.mul(int(146_097)).add(day_of_era).sub(int(719_468)).mul(int(86_400)) {
        Num::Int(seconds) => Ok(seconds),
        Num::Float(_) => Err(type_error(
            "HyperCast\\Interop\\NativeValues::epochSeconds(): Return value must be of type int, float returned",
        )),
    }
}

/// 1.0's private `NativeValues::at`: PHP 8.4's `createFromTimestamp` and `setMicrosecond`
/// where they exist, `new \DateTimeImmutable("@seconds")` and `modify()` before.
fn at(seconds: i64, micros: i64) -> PhpResult<Zval> {
    let fast = has_method("datetimeimmutable", "createfromtimestamp")
        && has_method("datetimeimmutable", "setmicrosecond");
    if fast {
        let instant =
            call(None, "DateTimeImmutable::createFromTimestamp", &mut [int_zval(seconds)])?;
        if micros == 0 {
            return Ok(instant);
        }
        return call(Some(&instant), "setMicrosecond", &mut [int_zval(micros)]);
    }
    let instant = new_immutable(&format!("@{seconds}"))?;
    if micros == 0 {
        return Ok(instant);
    }
    call(Some(&instant), "modify", &mut [bytes_zval(format!("+{micros} microseconds").as_bytes())])
}

/// `new \DateTimeImmutable($text)`.
fn new_immutable(text: &str) -> PhpResult<Zval> {
    let class =
        ClassEntry::try_find("DateTimeImmutable").ok_or_else(|| pending("DateTimeImmutable"))?;
    let mut object = Zval::new();
    // SAFETY: ext/date's registered class; the zval receives the new object's reference.
    let status = unsafe {
        object_init_ex(
            &raw mut object,
            std::ptr::from_ref(class).cast_mut().cast::<zend_class_entry>(),
        )
    };
    if status != 0 || ExecutorGlobals::has_exception() {
        return Err(pending("new DateTimeImmutable"));
    }
    call(Some(&object), "__construct", &mut [bytes_zval(text.as_bytes())])?;
    Ok(object)
}

/// 1.0's `NativeValues::instant`: whole microseconds, truncated.
pub(crate) fn instant(seconds: i64, nanos: i64) -> PhpResult<Zval> {
    at(seconds, nanos / 1000)
}

/// 1.0's `NativeValues::date`: midnight UTC.
pub(crate) fn date(year: i64, month: i64, day: i64) -> PhpResult<Zval> {
    at(epoch_seconds(year, month, day)?, 0)
}

/// 1.0's `NativeValues::civil`: the wall-clock time read as UTC.
pub(crate) fn civil(year: i64, month: i64, day: i64, nanos_of_day: i64) -> PhpResult<Zval> {
    let Some(seconds) = epoch_seconds(year, month, day)?.checked_add(nanos_of_day / 1_000_000_000)
    else {
        return Err(type_error(
            "HyperCast\\Interop\\NativeValues::at(): Argument #1 ($seconds) must be of type int, float given",
        ));
    };
    at(seconds, nanos_of_day % 1_000_000_000 / 1000)
}
