//! The fast Ruby backend: the hypercast core linked straight into a Ruby native extension
//! via Magnus — the PyO3 play, run for Ruby. Fiddle's measured per-call floor is 1.6 µs of
//! interpreted marshalling; an extension method is an ordinary C-function call, so the
//! doors drop to the cost of the parse plus building the Ruby values.
//!
//! On require (after `lib/hypercast.rb` has defined the pure-Fiddle module), this
//! extension redefines the door singleton methods **in place on the HyperCast module** —
//! no delegation layer, no second surface. Verdicts stay the package's own `Success` /
//! `Fault` Data classes with Symbol reasons; `NumFormat` stays the package's Data class,
//! with `INVARIANT` recognized by identity so the overwhelmingly common format costs zero
//! attribute reads per call. `HYPERCAST_PURE=1` (checked Ruby-side) keeps Fiddle.

use std::sync::OnceLock;

use magnus::rb_sys::{AsRawValue, FromRawValue};
use magnus::value::{Opaque, ReprValue};
use magnus::{function, prelude::*, Error, IntoValue, RModule, RString, Ruby, Symbol, Value};

use hypercast as core;

/// Constant-referenced objects (classes, INVARIANT) are anchored by Ruby constants and
/// never collected, so caching them by raw VALUE is GC-safe.
struct Cached {
    success: Opaque<Value>,
    fault: Opaque<Value>,
    date_class: Opaque<Value>,
    invariant_raw: u64,
}

static CACHED: OnceLock<Cached> = OnceLock::new();

fn cached() -> &'static Cached {
    CACHED.get().expect("hypercast_native used before init")
}

fn build_cache(ruby: &Ruby, hypercast: RModule) -> Result<Cached, Error> {
    let num_format: Value = hypercast.const_get("NumFormat")?;
    let invariant: Value = num_format.funcall("const_get", ("INVARIANT",))?;
    Ok(Cached {
        success: Opaque::from(hypercast.const_get::<_, Value>("Success")?),
        fault: Opaque::from(hypercast.const_get::<_, Value>("Fault")?),
        date_class: Opaque::from(
            ruby.class_object().funcall::<_, _, Value>("const_get", ("Date",))?,
        ),
        invariant_raw: invariant.as_raw(),
    })
}

fn success(ruby: &Ruby, value: impl magnus::IntoValue) -> Result<Value, Error> {
    ruby.get_inner(cached().success)
        .funcall("new", (value.into_value_with(ruby),))
}

fn fault(ruby: &Ruby, failed: core::Fault) -> Result<Value, Error> {
    let reason = match failed.reason {
        core::Reason::Empty => ruby.to_symbol("empty"),
        core::Reason::Malformed => ruby.to_symbol("malformed"),
        core::Reason::OutOfRange => ruby.to_symbol("out_of_range"),
    };
    ruby.get_inner(cached().fault)
        .funcall("new", (reason, failed.offset, failed.len))
}

fn verdict<T: magnus::IntoValue>(
    ruby: &Ruby,
    outcome: Result<T, core::Fault>,
) -> Result<Value, Error> {
    match outcome {
        Ok(value) => success(ruby, value),
        Err(failed) => fault(ruby, failed),
    }
}

/// Resolves the declared format: identity-matched INVARIANT costs nothing; any other
/// format pays three attribute reads.
fn resolve_format(ruby: &Ruby, format: Value) -> Result<core::NumFormat, Error> {
    let _ = ruby;
    if format.as_raw() == cached().invariant_raw {
        return Ok(core::NumFormat::INVARIANT);
    }
    let decimal: String = format.funcall("decimal_sep", ())?;
    let group: String = format.funcall("group_sep", ())?;
    let flags: u32 = format.funcall("flags", ())?;
    let (Some(decimal), Some(group)) = (decimal.chars().next(), group.chars().next()) else {
        return Err(Error::new(ruby.exception_arg_error(), "separators must be single characters"));
    };
    Ok(core::NumFormat { decimal_sep: decimal, group_sep: group, flags })
}

/// Borrows the RString's bytes for the duration of the parse only — no Ruby calls happen
/// inside the borrow, so the slice cannot be invalidated mid-use.
fn with_bytes<T>(text: RString, parse: impl FnOnce(&[u8]) -> T) -> T {
    parse(unsafe { text.as_slice() })
}

fn bool_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    verdict(ruby, with_bytes(text, |bytes| core::cast_bool(bytes)))
}

macro_rules! numeric_doors {
    ($($door:ident => $core:ident),+ $(,)?) => {$(
        fn $door(ruby: &Ruby, text: RString, format: Value) -> Result<Value, Error> {
            let resolved = resolve_format(ruby, format)?;
            verdict(ruby, with_bytes(text, |bytes| core::$core(bytes, &resolved)))
        }
    )+};
}

numeric_doors! {
    i8_door => cast_i8,
    i16_door => cast_i16,
    i32_door => cast_i32,
    i64_door => cast_i64,
    u8_door => cast_u8,
    u16_door => cast_u16,
    u32_door => cast_u32,
    u64_door => cast_u64,
    f32_door => cast_f32,
    f64_door => cast_f64,
}

fn uuid_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    match with_bytes(text, |bytes| core::cast_uuid(bytes)) {
        Ok(bytes) => {
            let mut hyphenated = String::with_capacity(36);
            for (index, byte) in bytes.iter().enumerate() {
                if matches!(index, 4 | 6 | 8 | 10) {
                    hyphenated.push('-');
                }
                hyphenated.push(char::from_digit((byte >> 4) as u32, 16).unwrap());
                hyphenated.push(char::from_digit((byte & 0xF) as u32, 16).unwrap());
            }
            success(ruby, hyphenated)
        }
        Err(failed) => fault(ruby, failed),
    }
}

/// Builds a UTC Time at full nanosecond fidelity: `rb_time_nano_new` (local zone) then an
/// in-place `#utc` — two cheap calls instead of Fiddle-era `Time.at(..., in: "UTC")`.
fn utc_time(ruby: &Ruby, ts: core::Timestamp) -> Result<Value, Error> {
    let raw = unsafe { rb_sys::rb_time_nano_new(ts.seconds, ts.nanos as std::ffi::c_long) };
    let time = unsafe { Value::from_raw(raw) };
    let _: Value = time.funcall("utc", ())?;
    success(ruby, time)
}

fn timestamp_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    match with_bytes(text, |bytes| core::cast_timestamp(bytes)) {
        Ok(ts) => utc_time(ruby, ts),
        Err(failed) => fault(ruby, failed),
    }
}

fn unix_door(ruby: &Ruby, text: RString, precision: Symbol) -> Result<Value, Error> {
    let name = precision.name()?;
    let precision = match &*name {
        "seconds" => core::UnixPrecision::Seconds,
        "milliseconds" => core::UnixPrecision::Millis,
        "microseconds" => core::UnixPrecision::Micros,
        "nanoseconds" => core::UnixPrecision::Nanos,
        other => {
            return Err(Error::new(
                ruby.exception_key_error(),
                format!("unknown UnixPrecision {other:?}"),
            ))
        }
    };
    match with_bytes(text, |bytes| core::cast_unix(bytes, precision)) {
        Ok(ts) => utc_time(ruby, ts),
        Err(failed) => fault(ruby, failed),
    }
}

fn date_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    match with_bytes(text, |bytes| core::cast_date(bytes)) {
        Ok(date) => {
            let class = ruby.get_inner(cached().date_class);
            success(ruby, class.funcall::<_, _, Value>("new", (date.year, date.month, date.day))?)
        }
        Err(failed) => fault(ruby, failed),
    }
}

fn time_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    verdict(ruby, with_bytes(text, |bytes| core::cast_time(bytes)))
}

fn duration_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    match with_bytes(text, |bytes| core::cast_duration(bytes)) {
        Ok(span) => {
            // Exact Rational seconds, no float anywhere: nanos.quo(1e9) + whole seconds.
            // (Rational + Integer stays Rational; the parts each fit i64 where the total
            // in nanoseconds would not.)
            let fraction: Value = i64::from(span.nanos)
                .into_value_with(ruby)
                .funcall("quo", (1_000_000_000i64,))?;
            let rational: Value = fraction.funcall("+", (span.seconds,))?;
            success(ruby, rational)
        }
        Err(failed) => fault(ruby, failed),
    }
}

#[magnus::init(name = "hypercast_native")]
fn init(ruby: &Ruby) -> Result<(), Error> {
    // rb_define_module returns the existing module — lib/hypercast.rb has already defined
    // the pure-Fiddle surface; these redefinitions replace the doors in place.
    let hypercast = ruby.define_module("HyperCast")?;
    let _ = CACHED.set(build_cache(ruby, hypercast)?);
    hypercast.define_singleton_method("bool", function!(bool_door, 1))?;
    hypercast.define_singleton_method("i8", function!(i8_door, 2))?;
    hypercast.define_singleton_method("i16", function!(i16_door, 2))?;
    hypercast.define_singleton_method("i32", function!(i32_door, 2))?;
    hypercast.define_singleton_method("i64", function!(i64_door, 2))?;
    hypercast.define_singleton_method("u8", function!(u8_door, 2))?;
    hypercast.define_singleton_method("u16", function!(u16_door, 2))?;
    hypercast.define_singleton_method("u32", function!(u32_door, 2))?;
    hypercast.define_singleton_method("u64", function!(u64_door, 2))?;
    hypercast.define_singleton_method("f32", function!(f32_door, 2))?;
    hypercast.define_singleton_method("f64", function!(f64_door, 2))?;
    hypercast.define_singleton_method("uuid", function!(uuid_door, 1))?;
    hypercast.define_singleton_method("timestamp", function!(timestamp_door, 1))?;
    hypercast.define_singleton_method("unix", function!(unix_door, 2))?;
    hypercast.define_singleton_method("date", function!(date_door, 1))?;
    hypercast.define_singleton_method("time", function!(time_door, 1))?;
    hypercast.define_singleton_method("duration", function!(duration_door, 1))?;
    Ok(())
}
