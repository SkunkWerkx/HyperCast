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

use std::cell::RefCell;
use std::sync::OnceLock;

use magnus::encoding::EncodingCapable;
use magnus::rb_sys::{AsRawValue, FromRawValue};
use magnus::scan_args::scan_args;
use magnus::value::{Opaque, ReprValue};
use magnus::{
    Error, IntoValue, RModule, RString, RStruct, Ruby, Symbol, Value, function, prelude::*,
};

use crate as core;

/// Constant-referenced objects (classes, INVARIANT, DETECT) are anchored by Ruby constants
/// and never collected, so caching them by raw VALUE is GC-safe. The Symbols are static
/// symbols — interned once, immortal — so their raw VALUEs are stable too: every `:seconds`
/// a caller writes is the same VALUE, and a pointer compare resolves it.
struct Cached {
    success: Opaque<Value>,
    fault: Opaque<Value>,
    date_class: Opaque<Value>,
    datetime_class: Opaque<Value>,
    decimal_class: Opaque<Value>,
    invariant_raw: u64,
    detect_raw: u64,
    empty: Opaque<Symbol>,
    malformed: Opaque<Symbol>,
    out_of_range: Opaque<Symbol>,
    precisions: [(u64, core::UnixPrecision); 4],
    epochs: [(u64, core::ExcelEpoch); 2],
    orders: [(u64, core::DateOrder); 3],
}

static CACHED: OnceLock<Cached> = OnceLock::new();

fn cached() -> &'static Cached {
    CACHED.get().expect("hypercast_native used before init")
}

fn build_cache(ruby: &Ruby, hypercast: RModule) -> Result<Cached, Error> {
    let num_format: Value = hypercast.const_get("NumFormat")?;
    let invariant: Value = num_format.funcall("const_get", ("INVARIANT",))?;
    let detect: Value = num_format.funcall("const_get", ("DETECT",))?;
    let sym = |name: &str| ruby.to_symbol(name).as_raw();
    Ok(Cached {
        success: Opaque::from(hypercast.const_get::<_, Value>("Success")?),
        fault: Opaque::from(hypercast.const_get::<_, Value>("Fault")?),
        date_class: Opaque::from(
            ruby.class_object().funcall::<_, _, Value>("const_get", ("Date",))?,
        ),
        datetime_class: Opaque::from(
            ruby.class_object().funcall::<_, _, Value>("const_get", ("DateTime",))?,
        ),
        decimal_class: Opaque::from(hypercast.const_get::<_, Value>("Decimal")?),
        invariant_raw: invariant.as_raw(),
        detect_raw: detect.as_raw(),
        empty: Opaque::from(ruby.to_symbol("empty")),
        malformed: Opaque::from(ruby.to_symbol("malformed")),
        out_of_range: Opaque::from(ruby.to_symbol("out_of_range")),
        precisions: [
            (sym("seconds"), core::UnixPrecision::Seconds),
            (sym("milliseconds"), core::UnixPrecision::Millis),
            (sym("microseconds"), core::UnixPrecision::Micros),
            (sym("nanoseconds"), core::UnixPrecision::Nanos),
        ],
        epochs: [(sym("y1900"), core::ExcelEpoch::Y1900), (sym("y1904"), core::ExcelEpoch::Y1904)],
        orders: [
            (sym("year_month_day"), core::DateOrder::YearMonthDay),
            (sym("month_day_year"), core::DateOrder::MonthDayYear),
            (sym("day_month_year"), core::DateOrder::DayMonthYear),
        ],
    })
}

/// Resolves a declared option Symbol against its static-symbol table by raw VALUE — a
/// pointer compare per entry, no `Symbol#name` materialization. A dynamic Symbol that
/// spells the same name (rare: `"seconds".to_sym` where no static one existed) misses
/// here and is resolved by name in the caller's slow path.
fn lookup<T: Copy, const N: usize>(table: &[(u64, T); N], symbol: Symbol) -> Option<T> {
    let raw = symbol.as_raw();
    table.iter().find(|(known, _)| *known == raw).map(|(_, value)| *value)
}

/// A declared option that names nothing — an unknown Symbol, or no Symbol at all (a String,
/// say) — is a caller bug, and the same one on every backend: hypercast.rb resolves these
/// through `UNIX_PRECISIONS.fetch` / `EXCEL_EPOCHS.fetch` / `DATE_ORDERS.fetch`, so this is
/// `Hash#fetch`'s KeyError with `Hash#fetch`'s own message.
fn unknown_option(ruby: &Ruby, option: Value) -> Error {
    Error::new(ruby.exception_key_error(), format!("key not found: {}", option.inspect()))
}

/// The declared option as a Symbol. Taken as a plain `Value` rather than a `Symbol`
/// parameter on purpose: magnus would turn anything else into a TypeError before the door
/// ran, where the Fiddle and wasm backends raise `unknown_option`'s KeyError.
fn declared_symbol(ruby: &Ruby, option: Value) -> Result<Symbol, Error> {
    Symbol::from_value(option).ok_or_else(|| unknown_option(ruby, option))
}

/// Builds an instance of one of the package's `Data` classes with its members in definition
/// order.
///
/// `Data.new` is the expensive part of a lean door: it gathers its arguments into a keyword
/// Hash before it stores them, and measured ~280 ns for `Success` alone against ~450 for a
/// whole `bool` cast. A `Data` instance is a struct underneath, so the same object is made
/// here the way `Data#initialize` finishes making it — allocated, its members stored, frozen
/// — without the Hash. Should a Ruby ever stop representing `Data` as a struct, `from_value`
/// says so and the door falls back to `.new`.
fn data<const N: usize>(class: Value, members: [Value; N]) -> Result<Value, Error> {
    // SAFETY: `class` is a live class object (cached at load); rb_obj_alloc runs its
    // allocator and raises only when memory is exhausted.
    let instance = unsafe { Value::from_raw(rb_sys::rb_obj_alloc(class.as_raw())) };
    let Some(fields) = RStruct::from_value(instance) else {
        return match members.len() {
            1 => class.funcall("new", (members[0],)),
            _ => class.funcall("new", (members[0], members[1], members[2])),
        };
    };
    for (index, member) in members.into_iter().enumerate() {
        fields.aset(index, member)?;
    }
    instance.freeze();
    Ok(instance)
}

fn success(ruby: &Ruby, value: impl magnus::IntoValue) -> Result<Value, Error> {
    data(ruby.get_inner(cached().success), [value.into_value_with(ruby)])
}

/// Builds the Fault over `text` — the UTF-8 the core read — with its span in the units
/// `String#[]` slices by (see `character_span`).
fn fault(ruby: &Ruby, text: RString, failed: core::Fault) -> Result<Value, Error> {
    let cache = cached();
    let reason = ruby.get_inner(match failed.reason {
        core::Reason::Empty => cache.empty,
        core::Reason::Malformed => cache.malformed,
        core::Reason::OutOfRange => cache.out_of_range,
    });
    let (offset, length) = character_span(text, failed.offset, failed.len);
    data(
        ruby.get_inner(cached().fault),
        [reason.as_value(), offset.into_value_with(ruby), length.into_value_with(ruby)],
    )
}

/// The core's byte span in the units `String#[]` slices by — the same rule as
/// hypercast.rb's `characters`, through the routine Ruby itself uses to turn a byte
/// position into a character position: an identity for a binary String and for 7-bit
/// text (O(1) off the cached coderange), a character count otherwise. Failure path only.
fn character_span(text: RString, offset: u32, len: u32) -> (i64, i64) {
    let raw = text.as_raw();
    // SAFETY: `text` is a live String; positions come from the core and never exceed its
    // byte length (the fault-span invariant the core's own tests pin).
    let start = unsafe { rb_sys::rb_str_sublen(raw, offset as std::ffi::c_long) };
    let end = unsafe { rb_sys::rb_str_sublen(raw, (offset + len) as std::ffi::c_long) };
    (start as i64, (end - start) as i64)
}

/// Presents the input as the UTF-8 the core reads — hypercast.rb's `utf8` rule: UTF-8,
/// US-ASCII and binary cross as-is (one encoding-index read); any other encoding pays a
/// transcode. Character offsets survive the transcode, so a fault span mapped on the
/// UTF-8 form still indexes the caller's own String.
///
/// The transcode is `String#encode` itself, the very call hypercast.rb makes, rather than
/// `rb_str_conv_enc`: text that cannot be transcoded (a lone UTF-16 surrogate, say) must
/// raise the same `Encoding::` error on every backend, and `rb_str_conv_enc` does not raise
/// — it hands the original bytes back, which this door would then have parsed as UTF-8.
fn utf8(ruby: &Ruby, text: RString) -> Result<RString, Error> {
    let index = text.enc_get();
    if index == ruby.utf8_encindex()
        || index == ruby.usascii_encindex()
        || index == ruby.ascii8bit_encindex()
    {
        Ok(text)
    } else {
        text.funcall("encode", (ruby.utf8_encoding(),))
    }
}

fn verdict<T: magnus::IntoValue>(
    ruby: &Ruby,
    text: RString,
    outcome: Result<T, core::Fault>,
) -> Result<Value, Error> {
    match outcome {
        Ok(value) => success(ruby, value),
        Err(failed) => fault(ruby, text, failed),
    }
}

thread_local! {
    /// The last non-constant format this thread resolved, keyed by the object's raw VALUE.
    /// Formats are reused constants in practice — a per-locale instance built once — so
    /// this turns three method dispatches per numeric call into one pointer compare, the
    /// same memo the Java, PHP and Python bindings keep. `NumFormat` is an immutable,
    /// frozen `Data`, which is what makes an identity memo sound at all — provided the key
    /// can never be a recycled address: the memoized object is anchored in a Ruby
    /// thread-variable (below) for exactly as long as this entry names it. Only plain data
    /// lives here, deliberately — a GC-registered handle in a thread-local would try to
    /// unregister itself from a VM that has already shut down when the thread exits.
    static LAST_FORMAT: RefCell<Option<(u64, core::NumFormat)>> = const { RefCell::new(None) };
}

/// Keeps `format` alive for as long as `LAST_FORMAT` keys on it: one Ruby thread-variable
/// per thread, replaced on every miss, so the previous format becomes collectable and the
/// current one cannot be. `thread_variable_set` is thread-local (not fiber-local, unlike
/// `Thread#[]=`), matching the Rust thread-local it guards.
fn anchor_format(ruby: &Ruby, format: Value) -> Result<(), Error> {
    let _: Value = ruby
        .thread_current()
        .funcall("thread_variable_set", (ruby.to_symbol("__hypercast_last_format"), format))?;
    Ok(())
}

/// Resolves the declared format: INVARIANT and DETECT are identity-matched constants and
/// cost a pointer compare; any other format costs four attribute reads the first time
/// this thread sees it and a pointer compare after that.
fn resolve_format(ruby: &Ruby, format: Value) -> Result<core::NumFormat, Error> {
    let raw = format.as_raw();
    let cache = cached();
    if raw == cache.invariant_raw {
        return Ok(core::NumFormat::INVARIANT);
    }
    if raw == cache.detect_raw {
        return Ok(core::NumFormat::DETECT);
    }
    let memo = LAST_FORMAT.with(|last| {
        last.borrow().as_ref().filter(|(key, _)| *key == raw).map(|(_, resolved)| *resolved)
    });
    if let Some(resolved) = memo {
        return Ok(resolved);
    }
    let decimal: RString = format.funcall("decimal_sep", ())?;
    let group: RString = format.funcall("group_sep", ())?;
    let flags: u32 = format.funcall("flags", ())?;
    // Copied out (once per format per thread — this is the memo's miss path): the symbol
    // is re-validated against the core's own rule below, so a Ruby-side check that drifted
    // would surface here as an ArgumentError rather than a silent contract violation.
    let currency: String = format.funcall("currency", ())?;
    // The Data class validated single characters at construction; read each as a str
    // straight from the RString's bytes — no Ruby call happens inside either borrow.
    let first_char = |text: RString| -> Option<char> {
        std::str::from_utf8(unsafe { text.as_slice() }).ok()?.chars().next()
    };
    let (Some(decimal), Some(group)) = (first_char(decimal), first_char(group)) else {
        return Err(Error::new(ruby.exception_arg_error(), "separators must be single characters"));
    };
    let symbol = if currency.is_empty() {
        core::CurrencySymbol::NONE
    } else {
        core::CurrencySymbol::new(&currency).ok_or_else(|| {
            Error::new(
                ruby.exception_arg_error(),
                format!(
                    "currency symbol {currency:?} must be at most {} UTF-8 bytes with no ASCII digit or whitespace",
                    core::CurrencySymbol::MAX_BYTES
                ),
            )
        })?
    };
    let resolved = core::NumFormat::new(decimal, group, flags).with_currency(symbol);
    anchor_format(ruby, format)?;
    LAST_FORMAT.with(|last| *last.borrow_mut() = Some((raw, resolved)));
    Ok(resolved)
}

/// Borrows the RString's bytes for the duration of the parse only — no Ruby calls happen
/// inside the borrow, so the slice cannot be invalidated mid-use.
fn with_bytes<T>(text: RString, parse: impl FnOnce(&[u8]) -> T) -> T {
    parse(unsafe { text.as_slice() })
}

fn bool_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    let text = utf8(ruby, text)?;
    verdict(ruby, text, with_bytes(text, |bytes| core::cast_bool(bytes)))
}

macro_rules! numeric_doors {
    ($($door:ident => $core:ident),+ $(,)?) => {$(
        fn $door(ruby: &Ruby, text: RString, format: Value) -> Result<Value, Error> {
            let text = utf8(ruby, text)?;
            let resolved = resolve_format(ruby, format)?;
            verdict(ruby, text, with_bytes(text, |bytes| core::$core(bytes, &resolved)))
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

/// The decimal door's value is the package's own `Decimal` Data (magnitude, scale,
/// negative): the 96-bit magnitude becomes one Ruby Integer — a Fixnum while it fits,
/// a Bignum past 2⁶⁴ — the scale a small Integer, and the sign a Boolean.
fn decimal_door(ruby: &Ruby, text: RString, format: Value) -> Result<Value, Error> {
    let text = utf8(ruby, text)?;
    let resolved = resolve_format(ruby, format)?;
    match with_bytes(text, |bytes| core::cast_decimal(bytes, &resolved)) {
        Ok(decimal) => {
            let class = ruby.get_inner(cached().decimal_class);
            let magnitude = ruby.integer_from_u128(decimal.magnitude());
            success(
                ruby,
                class.funcall::<_, _, Value>(
                    "new",
                    (magnitude, u32::from(decimal.scale), decimal.negative),
                )?,
            )
        }
        Err(failed) => fault(ruby, text, failed),
    }
}

/// The loaded core's version as "major.minor.patch", unpacked from the same word the
/// C-ABI `hypercast_version` export returns — here the core is linked in, so this is the
/// version of the crate this very extension was compiled from.
fn native_version() -> String {
    let word = core::hypercast_version();
    format!("{}.{}.{}", word >> 16, (word >> 8) & 0xFF, word & 0xFF)
}

fn uuid_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    let text = utf8(ruby, text)?;
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
            // US-ASCII, not the UTF-8 a Rust `String` would become: the encoding
            // `SecureRandom.uuid` returns, and the one hypercast.rb's `unpack("H…")` yields
            // on the other two backends — same characters and same `==` either way, and
            // now the same `#encoding` whichever backend is live.
            success(ruby, ruby.enc_str_new(&hyphenated, ruby.usascii_encoding()))
        }
        Err(failed) => fault(ruby, text, failed),
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
    let text = utf8(ruby, text)?;
    match with_bytes(text, |bytes| core::cast_timestamp(bytes)) {
        Ok(ts) => utc_time(ruby, ts),
        Err(failed) => fault(ruby, text, failed),
    }
}

fn unix_door(ruby: &Ruby, text: RString, precision: Value) -> Result<Value, Error> {
    let symbol = declared_symbol(ruby, precision)?;
    let precision = match lookup(&cached().precisions, symbol) {
        Some(known) => known,
        None => match &*symbol.name()? {
            "seconds" => core::UnixPrecision::Seconds,
            "milliseconds" => core::UnixPrecision::Millis,
            "microseconds" => core::UnixPrecision::Micros,
            "nanoseconds" => core::UnixPrecision::Nanos,
            _ => return Err(unknown_option(ruby, precision)),
        },
    };
    let text = utf8(ruby, text)?;
    match with_bytes(text, |bytes| core::cast_unix(bytes, precision)) {
        Ok(ts) => utc_time(ruby, ts),
        Err(failed) => fault(ruby, text, failed),
    }
}

fn excel_serial_door(ruby: &Ruby, text: RString, epoch: Value) -> Result<Value, Error> {
    let symbol = declared_symbol(ruby, epoch)?;
    let epoch = match lookup(&cached().epochs, symbol) {
        Some(known) => known,
        None => match &*symbol.name()? {
            "y1900" => core::ExcelEpoch::Y1900,
            "y1904" => core::ExcelEpoch::Y1904,
            _ => return Err(unknown_option(ruby, epoch)),
        },
    };
    let text = utf8(ruby, text)?;
    match with_bytes(text, |bytes| core::cast_excel_serial(bytes, epoch)) {
        Ok(ts) => utc_time(ruby, ts),
        Err(failed) => fault(ruby, text, failed),
    }
}

// Variadic (arity -1) because the order argument is optional — magnus's fixed-arity
// function! would demand both; scan_args gives Ruby's own required-then-optional shape.
// The order is scanned as a plain Value: `date(text, nil)` is the documented
// `order = nil` default spelled out, and means exactly what leaving it off means — scanned
// as a Symbol it was a TypeError here and the strict ISO door on the other two backends.
fn date_door(ruby: &Ruby, args: &[Value]) -> Result<Value, Error> {
    let args = scan_args::<(RString,), (Option<Value>,), (), (), (), ()>(args)?;
    let (text,) = args.required;
    let (order,) = args.optional;
    let order = match order.filter(|declared| !declared.is_nil()) {
        None => None,
        Some(declared) => Some(resolve_order(ruby, declared)?),
    };
    let text = utf8(ruby, text)?;
    let verdict = match order {
        None => with_bytes(text, |bytes| core::cast_date(bytes)),
        Some(order) => with_bytes(text, |bytes| core::cast_date_ordered(bytes, order)),
    };
    match verdict {
        Ok(date) => {
            let class = ruby.get_inner(cached().date_class);
            success(ruby, class.funcall::<_, _, Value>("new", (date.year, date.month, date.day))?)
        }
        Err(failed) => fault(ruby, text, failed),
    }
}

fn resolve_order(ruby: &Ruby, order: Value) -> Result<core::DateOrder, Error> {
    let symbol = declared_symbol(ruby, order)?;
    if let Some(known) = lookup(&cached().orders, symbol) {
        return Ok(known);
    }
    let name = symbol.name()?;
    match &*name {
        "year_month_day" => Ok(core::DateOrder::YearMonthDay),
        "month_day_year" => Ok(core::DateOrder::MonthDayYear),
        "day_month_year" => Ok(core::DateOrder::DayMonthYear),
        _ => Err(unknown_option(ruby, order)),
    }
}

fn datetime_door(ruby: &Ruby, text: RString, order: Value) -> Result<Value, Error> {
    let order = resolve_order(ruby, order)?;
    let text = utf8(ruby, text)?;
    match with_bytes(text, |bytes| core::cast_datetime(bytes, order)) {
        Ok(civil) => {
            // Zone-less civil value on stdlib DateTime with exact Rational seconds — the
            // text named no zone, so none is invented; fusing one is the caller's job.
            let (second_of_day, frac) =
                (civil.nanos_of_day / 1_000_000_000, civil.nanos_of_day % 1_000_000_000);
            let (hour, rest) = (second_of_day / 3_600, second_of_day % 3_600);
            let (minute, second) = (rest / 60, rest % 60);
            let fraction: Value =
                (frac as i64).into_value_with(ruby).funcall("quo", (1_000_000_000i64,))?;
            let seconds: Value = fraction.funcall("+", (second as i64,))?;
            let class = ruby.get_inner(cached().datetime_class);
            success(
                ruby,
                class.funcall::<_, _, Value>(
                    "new",
                    (civil.date.year, civil.date.month, civil.date.day, hour, minute, seconds),
                )?,
            )
        }
        Err(failed) => fault(ruby, text, failed),
    }
}

fn time_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    let text = utf8(ruby, text)?;
    verdict(ruby, text, with_bytes(text, |bytes| core::cast_time(bytes)))
}

fn duration_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    let text = utf8(ruby, text)?;
    match with_bytes(text, |bytes| core::cast_duration(bytes)) {
        Ok(span) => {
            // Exact Rational seconds, no float anywhere: nanos.quo(1e9) + whole seconds.
            // (Rational + Integer stays Rational; the parts each fit i64 where the total
            // in nanoseconds would not.)
            let fraction: Value =
                i64::from(span.nanos).into_value_with(ruby).funcall("quo", (1_000_000_000i64,))?;
            let rational: Value = fraction.funcall("+", (span.seconds,))?;
            success(ruby, rational)
        }
        Err(failed) => fault(ruby, text, failed),
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
    hypercast.define_singleton_method("decimal", function!(decimal_door, 2))?;
    hypercast.define_singleton_method("uuid", function!(uuid_door, 1))?;
    hypercast.define_singleton_method("timestamp", function!(timestamp_door, 1))?;
    hypercast.define_singleton_method("unix", function!(unix_door, 2))?;
    hypercast.define_singleton_method("excel_serial", function!(excel_serial_door, 2))?;
    hypercast.define_singleton_method("date", function!(date_door, -1))?;
    hypercast.define_singleton_method("datetime", function!(datetime_door, 2))?;
    hypercast.define_singleton_method("time", function!(time_door, 1))?;
    hypercast.define_singleton_method("duration", function!(duration_door, 1))?;
    hypercast.define_singleton_method("native_version", function!(native_version, 0))?;
    Ok(())
}
