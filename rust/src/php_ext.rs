//! Benchmark spike: this crate linked straight into a Zend extension via `ext-php-rs`,
//! mirroring the Python (PyO3) / Ruby (Magnus) native-backend pattern, to measure it
//! against PHP's `ext-ffi` path (`../../php/src/Cast.php`). `Cast.php` already measured the
//! raw `ext-ffi` crossing at ~105 ns — "already extension-class" — unlike ctypes (~1 µs)
//! and Fiddle (~1.6 µs), which is *why* Python and Ruby got a native backend and PHP didn't.
//! This module exists to check that reasoning against real numbers rather than leave it
//! asserted — the same spike HyperUuid carries, at this crate's doors.
//!
//! Deliberately not wired into the `skunkwerkx/hypercast` Composer package: this is a
//! benchmark-only spike, not a second production backend. Functions mirror `Cast.php`'s
//! raw layer — text in; the verdict code, the out-value and the fault span out, as one
//! packed array — so the two paths are compared at the same layer, with no
//! `Success`/`Fault` object construction on either side. The array's first element is the
//! verdict code (0 success, 1 empty, 2 malformed, 3 out of range); on success the value's
//! fields follow, on failure the fault's byte offset and length do. A contract violation
//! (an invalid format, an undefined discriminant) is a caller bug and throws.

use ext_php_rs::binary::Binary;
use ext_php_rs::boxed::ZBox;
use ext_php_rs::convert::IntoZval;
use ext_php_rs::prelude::*;
use ext_php_rs::types::ZendHashTable;

use crate as core;
use crate::{DateOrder, ExcelEpoch, UnixPrecision};

type Reply = PhpResult<ZBox<ZendHashTable>>;

fn push<V: IntoZval>(table: &mut ZendHashTable, value: V) -> PhpResult<()> {
    table
        .push(value)
        .map_err(|e| PhpException::default(format!("hypercast: building the verdict array failed: {e}")))
}

/// The packed verdict: `[code, ...value fields]` or `[code, offset, length]`.
fn reply<T>(
    outcome: Result<T, core::Fault>,
    fields: impl FnOnce(&mut ZendHashTable, T) -> PhpResult<()>,
) -> Reply {
    let mut table = ZendHashTable::new();
    match outcome {
        Ok(value) => {
            push(&mut table, 0i64)?;
            fields(&mut table, value)?;
        }
        Err(fault) => {
            push(&mut table, fault.reason as i64)?;
            push(&mut table, i64::from(fault.offset))?;
            push(&mut table, i64::from(fault.len))?;
        }
    }
    Ok(table)
}

fn format(decimal_sep: u32, group_sep: u32, flags: u32) -> PhpResult<core::NumFormat> {
    let (Some(decimal_sep), Some(group_sep)) = (char::from_u32(decimal_sep), char::from_u32(group_sep))
    else {
        return Err(PhpException::default("separators must be Unicode scalar values".into()));
    };
    if decimal_sep == group_sep {
        return Err(PhpException::default("decimal and group separators must differ".into()));
    }
    Ok(core::NumFormat { decimal_sep, group_sep, flags })
}

fn timestamp(table: &mut ZendHashTable, ts: core::Timestamp) -> PhpResult<()> {
    push(table, ts.seconds)?;
    push(table, i64::from(ts.nanos))
}

fn date(table: &mut ZendHashTable, date: core::Date) -> PhpResult<()> {
    push(table, i64::from(date.year))?;
    push(table, i64::from(date.month))?;
    push(table, i64::from(date.day))
}

#[php_function]
#[php(name = "hypercast_native_cast_bool")]
pub fn hypercast_native_cast_bool(text: Binary<u8>) -> Reply {
    reply(core::cast_bool(text.as_slice()), push)
}

macro_rules! integer_doors {
    ($($php:ident, $name:literal => $core:ident),+ $(,)?) => {$(
        // Named explicitly: ext-php-rs derives a default by snake-casing the Rust
        // identifier, which splits the width off as its own word (`cast_i_8`).
        #[php_function]
        #[php(name = $name)]
        pub fn $php(text: Binary<u8>, decimal_sep: u32, group_sep: u32, flags: u32) -> Reply {
            let format = format(decimal_sep, group_sep, flags)?;
            reply(core::$core(text.as_slice(), &format), |table, value| push(table, i64::from(value)))
        }
    )+};
}

integer_doors! {
    hypercast_native_cast_i8, "hypercast_native_cast_i8" => cast_i8,
    hypercast_native_cast_i16, "hypercast_native_cast_i16" => cast_i16,
    hypercast_native_cast_i32, "hypercast_native_cast_i32" => cast_i32,
    hypercast_native_cast_i64, "hypercast_native_cast_i64" => cast_i64,
    hypercast_native_cast_u8, "hypercast_native_cast_u8" => cast_u8,
    hypercast_native_cast_u16, "hypercast_native_cast_u16" => cast_u16,
    hypercast_native_cast_u32, "hypercast_native_cast_u32" => cast_u32,
}

/// `u64` alone cannot ride PHP's signed `int`; it crosses as the two's-complement bit
/// pattern, exactly as `Cast.php`'s `u64` door presents it.
#[php_function]
#[php(name = "hypercast_native_cast_u64")]
pub fn hypercast_native_cast_u64(text: Binary<u8>, decimal_sep: u32, group_sep: u32, flags: u32) -> Reply {
    let format = format(decimal_sep, group_sep, flags)?;
    reply(core::cast_u64(text.as_slice(), &format), |table, value| push(table, value as i64))
}

#[php_function]
#[php(name = "hypercast_native_cast_f32")]
pub fn hypercast_native_cast_f32(text: Binary<u8>, decimal_sep: u32, group_sep: u32, flags: u32) -> Reply {
    let format = format(decimal_sep, group_sep, flags)?;
    reply(core::cast_f32(text.as_slice(), &format), |table, value| push(table, f64::from(value)))
}

#[php_function]
#[php(name = "hypercast_native_cast_f64")]
pub fn hypercast_native_cast_f64(text: Binary<u8>, decimal_sep: u32, group_sep: u32, flags: u32) -> Reply {
    let format = format(decimal_sep, group_sep, flags)?;
    reply(core::cast_f64(text.as_slice(), &format), push)
}

/// The 16 RFC 9562-ordered bytes as a binary string — the same raw form `Cast::uuidBytes`
/// hands back over ext-ffi.
#[php_function]
#[php(name = "hypercast_native_cast_uuid")]
pub fn hypercast_native_cast_uuid(text: Binary<u8>) -> Reply {
    reply(core::cast_uuid(text.as_slice()), |table, bytes| push(table, Binary::from(bytes.to_vec())))
}

#[php_function]
#[php(name = "hypercast_native_cast_timestamp")]
pub fn hypercast_native_cast_timestamp(text: Binary<u8>) -> Reply {
    reply(core::cast_timestamp(text.as_slice()), timestamp)
}

#[php_function]
#[php(name = "hypercast_native_cast_unix")]
pub fn hypercast_native_cast_unix(text: Binary<u8>, precision: u32) -> Reply {
    let precision = match precision {
        1 => UnixPrecision::Seconds,
        2 => UnixPrecision::Millis,
        3 => UnixPrecision::Micros,
        4 => UnixPrecision::Nanos,
        other => return Err(PhpException::default(format!("undefined UnixPrecision {other}"))),
    };
    reply(core::cast_unix(text.as_slice(), precision), timestamp)
}

#[php_function]
#[php(name = "hypercast_native_cast_excel_serial")]
pub fn hypercast_native_cast_excel_serial(text: Binary<u8>, epoch: u32) -> Reply {
    let epoch = match epoch {
        1 => ExcelEpoch::Y1900,
        2 => ExcelEpoch::Y1904,
        other => return Err(PhpException::default(format!("undefined ExcelEpoch {other}"))),
    };
    reply(core::cast_excel_serial(text.as_slice(), epoch), timestamp)
}

fn order(order: u32) -> PhpResult<DateOrder> {
    match order {
        1 => Ok(DateOrder::YearMonthDay),
        2 => Ok(DateOrder::MonthDayYear),
        3 => Ok(DateOrder::DayMonthYear),
        other => Err(PhpException::default(format!("undefined DateOrder {other}"))),
    }
}

#[php_function]
#[php(name = "hypercast_native_cast_date")]
pub fn hypercast_native_cast_date(text: Binary<u8>) -> Reply {
    reply(core::cast_date(text.as_slice()), date)
}

#[php_function]
#[php(name = "hypercast_native_cast_date_ordered")]
pub fn hypercast_native_cast_date_ordered(text: Binary<u8>, order_code: u32) -> Reply {
    let order = order(order_code)?;
    reply(core::cast_date_ordered(text.as_slice(), order), date)
}

#[php_function]
#[php(name = "hypercast_native_cast_datetime")]
pub fn hypercast_native_cast_datetime(text: Binary<u8>, order_code: u32) -> Reply {
    let order = order(order_code)?;
    reply(core::cast_datetime(text.as_slice(), order), |table, civil| {
        date(table, civil.date)?;
        push(table, civil.nanos_of_day as i64)
    })
}

#[php_function]
#[php(name = "hypercast_native_cast_time")]
pub fn hypercast_native_cast_time(text: Binary<u8>) -> Reply {
    reply(core::cast_time(text.as_slice()), |table, nanos| push(table, nanos as i64))
}

#[php_function]
#[php(name = "hypercast_native_cast_duration")]
pub fn hypercast_native_cast_duration(text: Binary<u8>) -> Reply {
    reply(core::cast_duration(text.as_slice()), |table, span| {
        push(table, span.seconds)?;
        push(table, i64::from(span.nanos))
    })
}

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(hypercast_native_cast_bool))
        .function(wrap_function!(hypercast_native_cast_i8))
        .function(wrap_function!(hypercast_native_cast_i16))
        .function(wrap_function!(hypercast_native_cast_i32))
        .function(wrap_function!(hypercast_native_cast_i64))
        .function(wrap_function!(hypercast_native_cast_u8))
        .function(wrap_function!(hypercast_native_cast_u16))
        .function(wrap_function!(hypercast_native_cast_u32))
        .function(wrap_function!(hypercast_native_cast_u64))
        .function(wrap_function!(hypercast_native_cast_f32))
        .function(wrap_function!(hypercast_native_cast_f64))
        .function(wrap_function!(hypercast_native_cast_uuid))
        .function(wrap_function!(hypercast_native_cast_timestamp))
        .function(wrap_function!(hypercast_native_cast_unix))
        .function(wrap_function!(hypercast_native_cast_excel_serial))
        .function(wrap_function!(hypercast_native_cast_date))
        .function(wrap_function!(hypercast_native_cast_date_ordered))
        .function(wrap_function!(hypercast_native_cast_datetime))
        .function(wrap_function!(hypercast_native_cast_time))
        .function(wrap_function!(hypercast_native_cast_duration))
}
