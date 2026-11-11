//! The fast Ruby backend: the hypercast core linked straight into a Ruby native extension
//! via Magnus — the PyO3 play, run for Ruby. Fiddle's measured per-call floor is 1.6 µs of
//! interpreted marshalling; an extension method is an ordinary C-function call, so the
//! doors drop to the cost of the parse plus building the Ruby values.
//!
//! On require (after `lib/hypercast.rb` has defined the pure-Fiddle module), this
//! extension defines the doors on the HyperCast module's singleton class, **in front of**
//! the Fiddle ones `HyperCast::FiddleBackend` holds (an override, not a redefinition) —
//! no delegation layer, no second surface. Verdicts stay the package's own `Success` /
//! `Fault` Data classes with Symbol reasons; `NumFormat` stays the package's Data class,
//! with `INVARIANT` recognized by identity so the overwhelmingly common format costs zero
//! attribute reads per call. `HYPERCAST_PURE=1` (checked Ruby-side) keeps Fiddle.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use magnus::encoding::EncodingCapable;
use magnus::rb_sys::{AsRawId, AsRawValue, FromRawValue};
use magnus::scan_args::{get_kwargs, scan_args};
use magnus::value::{Opaque, ReprValue};
use magnus::{
    Error, ExceptionClass, IntoValue, KwArgs, RArray, RClass, RHash, RModule, RString, RStruct,
    Ruby, Symbol, Value, function, prelude::*,
};
use rb_sys::VALUE;

use crate as core;
use crate::checked::{
    CheckOutcome, judge_numeric_detailed, judge_numeric_from_f64_detailed, judge_text_detailed,
};
use crate::constraints::{Entries, declared_decimal};
use crate::detailed;

/// Constant-referenced objects (classes, INVARIANT, DETECT) are anchored by Ruby constants,
/// so they are never collected — but a compacting GC can still move them, rewriting every
/// reference it can see, which these statics are not. `build_cache` registers each one
/// with the GC, which pins it, so the cached VALUEs stay theirs. The Symbols are static
/// symbols — interned once, immortal — so their raw VALUEs are stable too: every `:seconds`
/// a caller writes is the same VALUE, and a pointer compare resolves it.
struct Cached {
    hypercast: Opaque<RModule>,
    success: Opaque<Value>,
    fault: Opaque<Value>,
    date_class: Opaque<Value>,
    datetime_class: Opaque<Value>,
    decimal_class: Opaque<Value>,
    constraints_class: Opaque<RClass>,
    checked_class: Opaque<Value>,
    violation_class: Opaque<Value>,
    keyword_class: Opaque<Value>,
    detail_class: Opaque<Value>,
    refused_class: Opaque<ExceptionClass>,
    date_options_class: Opaque<RClass>,
    month_names_class: Opaque<RClass>,
    /// Detail 0 through 32 as the package's `Detail` constants (0 is an unknown one, never read).
    details: Vec<Opaque<Value>>,
    invariant_raw: VALUE,
    detect_raw: VALUE,
    invariant: core::ValidNumFormat,
    detect: core::ValidNumFormat,
    empty: Opaque<Symbol>,
    malformed: Opaque<Symbol>,
    out_of_range: Opaque<Symbol>,
    precisions: [(VALUE, core::UnixPrecision); 4],
    epochs: [(VALUE, core::ExcelEpoch); 2],
    orders: [(VALUE, core::DateOrder); 3],
}

static CACHED: OnceLock<Cached> = OnceLock::new();

fn cached() -> &'static Cached {
    CACHED.get().expect("hypercast_native used before init")
}

fn build_cache(ruby: &Ruby, hypercast: RModule) -> Result<Cached, Error> {
    let num_format: Value = hypercast.const_get("NumFormat")?;
    let invariant: Value = num_format.funcall("const_get", ("INVARIANT",))?;
    let detect: Value = num_format.funcall("const_get", ("DETECT",))?;
    let success: Value = hypercast.const_get("Success")?;
    let fault: Value = hypercast.const_get("Fault")?;
    let date_class: Value = ruby.class_object().funcall("const_get", ("Date",))?;
    let datetime_class: Value = ruby.class_object().funcall("const_get", ("DateTime",))?;
    let decimal_class: Value = hypercast.const_get("Decimal")?;
    let constraints_class: RClass = hypercast.const_get("Constraints")?;
    let checked_class: Value = hypercast.const_get("Checked")?;
    let violation_class: Value = hypercast.const_get("Violation")?;
    let keyword_class: Value = hypercast.const_get("Keyword")?;
    let detail_class: Value = hypercast.const_get("Detail")?;
    let refused_class: ExceptionClass = hypercast.const_get("RefusedDeclaration")?;
    let date_options_class: RClass = hypercast.const_get("DateOptions")?;
    let month_names_class: RClass = hypercast.const_get("MonthNames")?;
    let details = (0..=32u32)
        .map(|code| detail_class.funcall::<_, _, Value>("of", (code,)))
        .collect::<Result<Vec<_>, Error>>()?;
    for pinned in details.iter().copied().chain([
        hypercast.as_value(),
        invariant,
        detect,
        success,
        fault,
        date_class,
        datetime_class,
        decimal_class,
        constraints_class.as_value(),
        checked_class,
        violation_class,
        keyword_class,
        detail_class,
        refused_class.as_value(),
        date_options_class.as_value(),
        month_names_class.as_value(),
    ]) {
        ruby.gc_register_mark_object(pinned);
    }
    let sym = |name: &str| ruby.to_symbol(name).as_raw();
    Ok(Cached {
        hypercast: Opaque::from(hypercast),
        success: Opaque::from(success),
        fault: Opaque::from(fault),
        date_class: Opaque::from(date_class),
        datetime_class: Opaque::from(datetime_class),
        decimal_class: Opaque::from(decimal_class),
        constraints_class: Opaque::from(constraints_class),
        checked_class: Opaque::from(checked_class),
        violation_class: Opaque::from(violation_class),
        keyword_class: Opaque::from(keyword_class),
        detail_class: Opaque::from(detail_class),
        refused_class: Opaque::from(refused_class),
        date_options_class: Opaque::from(date_options_class),
        month_names_class: Opaque::from(month_names_class),
        details: details.into_iter().map(Opaque::from).collect(),
        invariant_raw: invariant.as_raw(),
        detect_raw: detect.as_raw(),
        // The two profiles hypercast.rb declares: `.`/`,` with Western grouping.
        invariant: core::NumFormat::INVARIANT.validate().map_err(|refusal| {
            Error::new(ruby.exception_runtime_error(), format!("INVARIANT: {refusal}"))
        })?,
        detect: core::NumFormat::DETECT.validate().map_err(|refusal| {
            Error::new(ruby.exception_runtime_error(), format!("DETECT: {refusal}"))
        })?,
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
fn lookup<T: Copy, const N: usize>(table: &[(VALUE, T); N], symbol: Symbol) -> Option<T> {
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
/// ran, where the Fiddle backend raises `unknown_option`'s KeyError.
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

/// The package's `Detail` for a core detail: its published constant.
fn detail_value(ruby: &Ruby, detail: core::Detail) -> Result<Value, Error> {
    let cache = cached();
    match cache.details.get(detail.code() as usize) {
        Some(known) => Ok(ruby.get_inner(*known)),
        None => ruby.get_inner(cache.detail_class).funcall("of", (detail.code(),)),
    }
}

/// The package's `Fault` with its `detail`, which is not a member (hypercast.rb has why), set
/// as the instance variable `Fault#detail` reads, before the Data object is frozen.
fn fault_value(
    ruby: &Ruby,
    reason: core::Reason,
    (offset, length): (i64, i64),
    detail: core::Detail,
) -> Result<Value, Error> {
    let cache = cached();
    let reason = ruby.get_inner(match reason {
        core::Reason::Empty => cache.empty,
        core::Reason::Malformed => cache.malformed,
        core::Reason::OutOfRange => cache.out_of_range,
    });
    let detail = detail_value(ruby, detail)?;
    let class = ruby.get_inner(cache.fault);
    // SAFETY: `class` is a live class object (cached at load); rb_obj_alloc runs its
    // allocator and raises only when memory is exhausted.
    let instance = unsafe { Value::from_raw(rb_sys::rb_obj_alloc(class.as_raw())) };
    let Some(fields) = RStruct::from_value(instance) else {
        let keywords = ruby.hash_new();
        keywords.aset(ruby.to_symbol("reason"), reason)?;
        keywords.aset(ruby.to_symbol("offset"), offset)?;
        keywords.aset(ruby.to_symbol("length"), length)?;
        keywords.aset(ruby.to_symbol("detail"), detail)?;
        return class.funcall("new", (KwArgs(keywords),));
    };
    fields.aset(0, reason)?;
    fields.aset(1, offset)?;
    fields.aset(2, length)?;
    // SAFETY: `instance` is the live, still unfrozen object just allocated; rb_ivar_set raises
    // only on a frozen one.
    unsafe {
        rb_sys::rb_ivar_set(instance.as_raw(), ruby.intern("@detail").as_raw(), detail.as_raw())
    };
    instance.freeze();
    Ok(instance)
}

/// Builds the Fault over `text` — the UTF-8 the core read — with its span in the units
/// `String#[]` slices by (see `character_span`) and the core's detail.
fn fault(ruby: &Ruby, text: RString, failed: core::Failure) -> Result<Value, Error> {
    let span = character_span(text, failed.offset, failed.len);
    fault_value(ruby, failed.reason, span, failed.detail)
}

/// A typed door's Fault: the reason and detail alone, over an empty span — there is no text
/// for one to index.
fn typed_fault(ruby: &Ruby, failed: core::Failure) -> Result<Value, Error> {
    fault_value(ruby, failed.reason, (0, 0), failed.detail)
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
/// binary and ASCII-only US-ASCII cross as-is (an encoding-index read, and for US-ASCII the
/// String's cached coderange); any other text pays a transcode, a US-ASCII String holding a
/// byte past 0x7F included, so that byte is malformed rather than read as UTF-8. Character offsets survive the transcode, so a fault span mapped on the
/// UTF-8 form still indexes the caller's own String; a dummy encoding's String counts bytes,
/// and its span counts the characters of the UTF-8 form (hypercast.rb's `kept`).
///
/// The transcode is hypercast.rb's own `utf8`, called here, rather than `rb_str_conv_enc`,
/// so both backends hand the core the same bytes: text that cannot be transcoded (a lone
/// UTF-16 surrogate, say) crosses with each character that has no UTF-8 form as one byte
/// no UTF-8 decoder accepts, which the core reads as malformed at that character, never as
/// a guess; `rb_str_conv_enc` hands the original bytes back instead, which this door would
/// then have parsed as UTF-8.
fn utf8(ruby: &Ruby, text: RString) -> Result<RString, Error> {
    let index = text.enc_get();
    if index == ruby.utf8_encindex()
        || index == ruby.ascii8bit_encindex()
        || (index == ruby.usascii_encindex()
            && text.enc_coderange_scan() == magnus::encoding::Coderange::SevenBit)
    {
        Ok(text)
    } else {
        ruby.get_inner(cached().hypercast).funcall("utf8", (text,))
    }
}

fn verdict<T: magnus::IntoValue>(
    ruby: &Ruby,
    text: RString,
    outcome: Result<T, core::Failure>,
) -> Result<Value, Error> {
    match outcome {
        Ok(value) => success(ruby, value),
        Err(failed) => fault(ruby, text, failed),
    }
}

thread_local! {
    /// The last non-constant format this thread resolved, keyed by the object's `object_id`.
    /// Formats are reused constants in practice — a per-locale instance built once — so
    /// this turns ten attribute reads per numeric call into one id lookup, the same memo the
    /// Java, PHP and Python bindings keep. `NumFormat` is an immutable, frozen `Data`, which
    /// is what makes an identity memo sound at all. The key is not the raw VALUE: a
    /// compacting GC moves the object and hands its old address to another, which a VALUE
    /// key would then mistake for it (a `NumFormat` built after `GC.compact` was read as the
    /// one before). An `object_id` belongs to one object for the life of the process and is
    /// never reused, so a stale entry can only miss, and nothing has to keep the keyed object
    /// alive. Only plain data lives here, deliberately — a GC-registered handle in a
    /// thread-local would try to unregister itself from a VM that has already shut down
    /// when the thread exits.
    static LAST_FORMAT: RefCell<Option<(u64, core::ValidNumFormat)>> = const { RefCell::new(None) };
}

/// The `object_id` an identity memo keys on: stable across compaction, never reused.
fn object_id(object: Value) -> Result<u64, Error> {
    // SAFETY: `object` is a live object; rb_obj_id returns its Integer id.
    let id = unsafe { Value::from_raw(rb_sys::rb_obj_id(object.as_raw())) };
    u64::try_convert(id)
}

/// Resolves the declared format, validated for the detailed doors: INVARIANT and DETECT are
/// identity-matched constants (pinned, so their addresses never move) and cost a pointer
/// compare; any other format costs eleven attribute reads the first time this thread sees it
/// and an id lookup after that.
fn resolve_format(ruby: &Ruby, format: Value) -> Result<core::ValidNumFormat, Error> {
    let raw = format.as_raw();
    let cache = cached();
    if raw == cache.invariant_raw {
        return Ok(cache.invariant);
    }
    if raw == cache.detect_raw {
        return Ok(cache.detect);
    }
    let id = object_id(format)?;
    let memo = LAST_FORMAT.with(|last| {
        last.borrow().as_ref().filter(|(key, _)| *key == id).map(|(_, resolved)| *resolved)
    });
    if let Some(resolved) = memo {
        return Ok(resolved);
    }
    // Copied out (once per format per thread — this is the memo's miss path), and the
    // whole format validated against the core's own rules below, so a Ruby-side check
    // that drifted would surface here as an ArgumentError rather than a silent contract
    // violation. Each field is the UTF-8 the Data class stored; "" declares nothing.
    let malformed = |field: &str, text: &str| {
        Error::new(
            ruby.exception_arg_error(),
            format!("{field} {text:?} is not a declarable symbol"),
        )
    };
    let symbol = |field: &str| -> Result<core::Symbol<8>, Error> {
        let text: String = format.funcall(field, ())?;
        if text.is_empty() {
            return Ok(core::Symbol::NONE);
        }
        core::Symbol::new(&text).ok_or_else(|| malformed(field, &text))
    };
    // The style mask with the grouping policy in bits 8 and 9, as the native layout carries it.
    let flags: u32 = format.funcall("raw_flags", ())?;
    let (decimal, group) = (symbol("decimal_sep")?, symbol("group_sep")?);
    if decimal.is_empty() || group.is_empty() {
        return Err(Error::new(ruby.exception_arg_error(), "separators must not be empty"));
    }
    let currency: String = format.funcall("currency", ())?;
    let currency = if currency.is_empty() {
        core::CurrencySymbol::NONE
    } else {
        core::CurrencySymbol::new(&currency).ok_or_else(|| malformed("currency", &currency))?
    };
    let zero: String = format.funcall("digit_zero", ())?;
    let digits = match zero.chars().next() {
        None => core::NativeDigits::NONE,
        Some(zero_char) => {
            core::NativeDigits::new(zero_char).ok_or_else(|| malformed("digit_zero", &zero))?
        }
    };
    let resolved = core::NumFormat::with_separators(decimal, group, flags)
        .with_currency(currency)
        .with_currency_separators(symbol("currency_decimal_sep")?, symbol("currency_group_sep")?)
        .with_signs(symbol("minus_sign")?, symbol("plus_sign")?)
        .with_percent(symbol("percent")?)
        .with_digits(digits)
        .validate()
        .map_err(|refusal| {
            Error::new(ruby.exception_arg_error(), format!("malformed NumFormat: {refusal}"))
        })?;
    LAST_FORMAT.with(|last| *last.borrow_mut() = Some((id, resolved)));
    Ok(resolved)
}

/// Borrows the RString's bytes for the duration of the parse only — no Ruby calls happen
/// inside the borrow, so the slice cannot be invalidated mid-use.
fn with_bytes<T>(text: RString, parse: impl FnOnce(&[u8]) -> T) -> T {
    parse(unsafe { text.as_slice() })
}

fn bool_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    let text = utf8(ruby, text)?;
    verdict(ruby, text, with_bytes(text, |bytes| detailed::cast_bool(bytes)))
}

/// The scalar as a one-character UTF-8 String — what hypercast.rb's `Integer#chr(UTF_8)`
/// yields on the Fiddle backend.
fn char_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    let text = utf8(ruby, text)?;
    match with_bytes(text, |bytes| detailed::cast_char(bytes)) {
        Ok(scalar) => success(ruby, ruby.str_new(scalar.encode_utf8(&mut [0; 4]))),
        Err(failed) => fault(ruby, text, failed),
    }
}

/// A numeric door's `(text, format, separator: nil, delimiter: nil, unit: nil)`. With no
/// keywords — the plain call, `(text, format)` — it is the two conversions every numeric
/// door always made and nothing else: no `scan_args`, no Hash. With keywords, the format is
/// put through the package's own `NumFormat#override`, so the overridden format is built,
/// validated and refused (an unknown keyword included) by the same Ruby code the Fiddle
/// backend runs, with the same messages.
fn numeric_args(ruby: &Ruby, args: &[Value]) -> Result<(RString, core::ValidNumFormat), Error> {
    // SAFETY: called on the Ruby thread, inside the method call whose arguments these are.
    let keywords = unsafe { rb_sys::rb_keyword_given_p() } != 0;
    if let ([text, format], false) = (args, keywords) {
        let text = utf8(ruby, RString::try_convert(*text)?)?;
        return Ok((text, resolve_format(ruby, *format)?));
    }
    let scanned = scan_args::<(RString, Value), (), (), (), RHash, ()>(args)?;
    let (text, format) = scanned.required;
    let text = utf8(ruby, text)?;
    let format: Value = format.funcall("override", (KwArgs(scanned.keywords),))?;
    Ok((text, resolve_format(ruby, format)?))
}

macro_rules! numeric_doors {
    ($($door:ident => $core:ident),+ $(,)?) => {$(
        fn $door(ruby: &Ruby, args: &[Value]) -> Result<Value, Error> {
            let (text, resolved) = numeric_args(ruby, args)?;
            verdict(ruby, text, with_bytes(text, |bytes| detailed::$core(bytes, &resolved)))
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
fn decimal_door(ruby: &Ruby, args: &[Value]) -> Result<Value, Error> {
    let (text, resolved) = numeric_args(ruby, args)?;
    match with_bytes(text, |bytes| detailed::cast_decimal(bytes, &resolved)) {
        Ok(decimal) => decimal_value(ruby, decimal),
        Err(failed) => fault(ruby, text, failed),
    }
}

fn decimal_value(ruby: &Ruby, decimal: core::Decimal) -> Result<Value, Error> {
    success(ruby, decimal_object(ruby, decimal)?)
}

/// The package's own `Decimal` Data for a core `Decimal`.
fn decimal_object(ruby: &Ruby, decimal: core::Decimal) -> Result<Value, Error> {
    let class = ruby.get_inner(cached().decimal_class);
    let magnitude = ruby.integer_from_u128(decimal.magnitude());
    class.funcall("new", (magnitude, u32::from(decimal.scale), decimal.negative))
}

/// Reads a number as the exact Decimal it names — the shortest decimal that rounds back to
/// the Float. The value is taken as Ruby's own Float conversion takes it (`rb_num2dbl`): an
/// Integer, Float or Rational, and a TypeError for anything else, a String included.
fn decimal_from_float_door(ruby: &Ruby, value: f64) -> Result<Value, Error> {
    match detailed::decimal_from_f64(value) {
        Ok(decimal) => decimal_value(ruby, decimal),
        Err(failed) => typed_fault(ruby, failed),
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
    match with_bytes(text, |bytes| detailed::cast_uuid(bytes)) {
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

/// The date options and month names a date door was given, as the plain values the core
/// reads: `options:` and `months:` are the package's own `DateOptions` and `MonthNames`, which
/// hypercast.rb has already judged, and the core judges them again for each call.
struct Dates {
    flags: u32,
    pivot: u16,
    /// The vocabulary in its packed layout, a frozen String held by the `MonthNames`.
    packed: Option<RString>,
}

/// Reads `options:` and `months:` (nil is none): a TypeError, worded as hypercast.rb words it,
/// for anything but a `DateOptions` and a `MonthNames`. `months` is only taken up when the door
/// reads a vocabulary, and is type-checked either way.
fn dates(
    ruby: &Ruby,
    options: Option<Value>,
    months: Option<Value>,
    reads_months: bool,
) -> Result<Option<Dates>, Error> {
    let cache = cached();
    let given = |value: Option<Value>| value.filter(|declared| !declared.is_nil());
    let (options, months) = (given(options), given(months));
    if let Some(options) = options
        && !options.is_kind_of(ruby.get_inner(cache.date_options_class))
    {
        return Err(Error::new(
            ruby.exception_type_error(),
            format!("options must be a HyperCast::DateOptions; got {}", options.inspect()),
        ));
    }
    if let Some(months) = months
        && !months.is_kind_of(ruby.get_inner(cache.month_names_class))
    {
        return Err(Error::new(
            ruby.exception_type_error(),
            format!("months must be a HyperCast::MonthNames; got {}", months.inspect()),
        ));
    }
    let months = months.filter(|_| reads_months);
    if options.is_none() && months.is_none() {
        return Ok(None);
    }
    let (flags, pivot) = match options {
        Some(options) => {
            let flags: u32 = options.funcall("flags", ())?;
            let pivot: Option<u16> = options.funcall("pivot", ())?;
            (flags, pivot.unwrap_or(0))
        }
        None => (0, 0),
    };
    // A vocabulary with no bytes is none, as the C ABI reads a length of 0 (a constructed
    // MonthNames never has one; a hand-built object might, and both backends read it alike).
    let packed = match months {
        Some(months) => Some(months.funcall::<_, _, RString>("packed", ())?)
            .filter(|packed| with_bytes(*packed, |bytes| !bytes.is_empty())),
        None => None,
    };
    Ok(Some(Dates { flags, pivot, packed }))
}

/// The core's refusal of date options or month names hypercast.rb accepted: the caller's
/// contract violation, `HyperCast::RefusedDeclaration` (an ArgumentError), worded as the Fiddle
/// backend words it.
fn refused(ruby: &Ruby) -> Error {
    Error::new(
        ruby.get_inner(cached().refused_class),
        "the core refused the date options or month names",
    )
}

/// Runs a date door over `text` with the declared options, the core judging them first (and for
/// each call, as the C ABI does: nothing is kept between calls). The outer Result is the
/// declaration's, the inner one the data's.
fn dated<T>(
    ruby: &Ruby,
    text: RString,
    declared: Option<&Dates>,
    door: impl FnOnce(&[u8], Option<&core::ValidDateOptions<'_>>) -> Result<T, core::Failure>,
) -> Result<Result<T, core::Failure>, Error> {
    let Some(declared) = declared else {
        return Ok(with_bytes(text, |bytes| door(bytes, None)));
    };
    let options = core::DateOptions::from_flags(declared.flags).with_pivot(declared.pivot);
    match declared.packed {
        None => {
            let valid = options.validate(None).map_err(|_| refused(ruby))?;
            Ok(with_bytes(text, |bytes| door(bytes, Some(&valid))))
        }
        Some(packed) => with_bytes(packed, |names| {
            let names = core::MonthNames::from_packed(names).map_err(|_| refused(ruby))?;
            let valid = options.validate(Some(names)).map_err(|_| refused(ruby))?;
            Ok(with_bytes(text, |bytes| door(bytes, Some(&valid))))
        }),
    }
}

/// Whether the current call passed keywords. Called on the Ruby thread, inside the method call
/// whose arguments these are.
fn keywords_given() -> bool {
    // SAFETY: as stated: on the Ruby thread, inside the method call.
    unsafe { rb_sys::rb_keyword_given_p() != 0 }
}

/// `timestamp(text, options: nil)`.
fn timestamp_door(ruby: &Ruby, args: &[Value]) -> Result<Value, Error> {
    let (text, options) = if keywords_given() {
        let scanned = scan_args::<(RString,), (), (), (), RHash, ()>(args)?;
        let declared =
            get_kwargs::<_, (), (Option<Value>,), ()>(scanned.keywords, &[], &["options"])?;
        (scanned.required.0, declared.optional.0)
    } else {
        (scan_args::<(RString,), (), (), (), (), ()>(args)?.required.0, None)
    };
    let declared = dates(ruby, options, None, false)?;
    let text = utf8(ruby, text)?;
    match dated(ruby, text, declared.as_ref(), |bytes, options| {
        detailed::cast_timestamp(bytes, options)
    })? {
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
    match with_bytes(text, |bytes| detailed::cast_unix(bytes, precision)) {
        Ok(ts) => utc_time(ruby, ts),
        Err(failed) => fault(ruby, text, failed),
    }
}

fn resolve_epoch(ruby: &Ruby, epoch: Value) -> Result<core::ExcelEpoch, Error> {
    let symbol = declared_symbol(ruby, epoch)?;
    if let Some(known) = lookup(&cached().epochs, symbol) {
        return Ok(known);
    }
    match &*symbol.name()? {
        "y1900" => Ok(core::ExcelEpoch::Y1900),
        "y1904" => Ok(core::ExcelEpoch::Y1904),
        _ => Err(unknown_option(ruby, epoch)),
    }
}

fn excel_serial_door(ruby: &Ruby, text: RString, epoch: Value) -> Result<Value, Error> {
    let epoch = resolve_epoch(ruby, epoch)?;
    let text = utf8(ruby, text)?;
    match with_bytes(text, |bytes| detailed::cast_excel_serial(bytes, epoch)) {
        Ok(ts) => utc_time(ruby, ts),
        Err(failed) => fault(ruby, text, failed),
    }
}

// Variadic (arity -1) because the order argument is optional — magnus's fixed-arity
// function! would demand both; scan_args gives Ruby's own required-then-optional shape.
// The order is scanned as a plain Value: `date(text, nil)` is the documented
// `order = nil` default spelled out, and means exactly what leaving it off means — scanned
// as a Symbol it was a TypeError here and the strict ISO door on the other two backends.
// `options:` and `months:` are keywords, scanned only when the call gave some.
fn date_door(ruby: &Ruby, args: &[Value]) -> Result<Value, Error> {
    let (text, order, options, months) = if keywords_given() {
        let scanned = scan_args::<(RString,), (Option<Value>,), (), (), RHash, ()>(args)?;
        let declared = get_kwargs::<_, (), (Option<Value>, Option<Value>), ()>(
            scanned.keywords,
            &[],
            &["options", "months"],
        )?;
        (scanned.required.0, scanned.optional.0, declared.optional.0, declared.optional.1)
    } else {
        let scanned = scan_args::<(RString,), (Option<Value>,), (), (), (), ()>(args)?;
        (scanned.required.0, scanned.optional.0, None, None)
    };
    let order = match order.filter(|declared| !declared.is_nil()) {
        None => None,
        Some(declared) => Some(resolve_order(ruby, declared)?),
    };
    let declared = dates(ruby, options, months, order.is_some())?;
    let text = utf8(ruby, text)?;
    let verdict = dated(ruby, text, declared.as_ref(), |bytes, options| match order {
        None => detailed::cast_date(bytes, options),
        Some(order) => detailed::cast_date_ordered(bytes, order, options),
    })?;
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

/// A declared day-period designator — hypercast.rb's `designator` rule, raising its
/// ArgumentError worded the same: a String with a UTF-8 form, of 1 to 16 UTF-8 bytes with no
/// ASCII digit and no white space at either end; nil declares nothing beside the English
/// word.
fn designator(ruby: &Ruby, name: &str, text: Option<Value>) -> Result<core::Symbol<16>, Error> {
    let Some(text) = text.filter(|given| !given.is_nil()) else {
        return Ok(core::Symbol::NONE);
    };
    let Some(string) = RString::from_value(text) else {
        return Err(Error::new(
            ruby.exception_arg_error(),
            format!("{name} designator must be a String; got {}", text.inspect()),
        ));
    };
    // hypercast.rb's own transcoding, so text with no UTF-8 form is its ArgumentError here too.
    let string: RString = ruby
        .get_inner(cached().hypercast)
        .funcall("declared_utf8", (format!("{name} designator"), string))?;
    let declared = with_bytes(string, |bytes| {
        let designator = std::str::from_utf8(bytes).ok()?;
        if designator.starts_with(core::is_white_space)
            || designator.ends_with(core::is_white_space)
        {
            return None;
        }
        core::Symbol::<16>::new(designator)
    });
    declared.ok_or_else(|| {
        Error::new(
            ruby.exception_arg_error(),
            format!(
                "{name} designator {} must be 1 to 16 UTF-8 bytes with no ASCII digit and no white space at either end",
                text.inspect()
            ),
        )
    })
}

/// The civil door's `(text, order, am: nil, pm: nil, options: nil, months: nil)`. With no
/// keywords — the plain call, `(text, order)` — it is the two conversions the door always made
/// and nothing else: no `scan_args`, no Hash. With keywords, the designators are read and
/// validated here, by the rule hypercast.rb applies on the Fiddle backend, and read beside the
/// English `AM`/`PM` for this call only — they are per-call text, not a memoized format object —
/// and the date options and month names are taken up as the other date doors take them.
fn datetime_door(ruby: &Ruby, args: &[Value]) -> Result<Value, Error> {
    let keywords = keywords_given();
    let (text, order, periods, options, months) = if let ([text, order], false) = (args, keywords) {
        (RString::try_convert(*text)?, *order, core::DayPeriods::ENGLISH, None, None)
    } else {
        let scanned = scan_args::<(RString, Value), (), (), (), RHash, ()>(args)?;
        let (text, order) = scanned.required;
        let declared = get_kwargs::<
            _,
            (),
            (Option<Value>, Option<Value>, Option<Value>, Option<Value>),
            (),
        >(scanned.keywords, &[], &["am", "pm", "options", "months"])?;
        let (am, pm, options, months) = declared.optional;
        let periods =
            core::DayPeriods::new(designator(ruby, "am", am)?, designator(ruby, "pm", pm)?);
        (text, order, periods, options, months)
    };
    let order = resolve_order(ruby, order)?;
    let declared = dates(ruby, options, months, true)?;
    let text = utf8(ruby, text)?;
    match dated(ruby, text, declared.as_ref(), |bytes, options| {
        detailed::cast_datetime_with(bytes, order, &periods, options)
    })? {
        Ok(civil) => civil_value(ruby, civil),
        Err(failed) => fault(ruby, text, failed),
    }
}

/// Zone-less civil value on stdlib DateTime with exact Rational seconds — the input named
/// no zone, so none is invented; fusing one is the caller's job.
fn civil_value(ruby: &Ruby, civil: core::CivilDateTime) -> Result<Value, Error> {
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

/// Reads an Excel serial number under a declared epoch Symbol as the zone-less DateTime it
/// names — the typed twin of `excel_serial`.
fn excel_serial_from_float_door(ruby: &Ruby, value: f64, epoch: Value) -> Result<Value, Error> {
    let epoch = resolve_epoch(ruby, epoch)?;
    match detailed::excel_serial(value, epoch) {
        Ok(civil) => civil_value(ruby, civil),
        Err(failed) => typed_fault(ruby, failed),
    }
}

/// Reads the fraction of an Excel serial number as nanoseconds since midnight.
fn excel_time_door(ruby: &Ruby, value: f64) -> Result<Value, Error> {
    match detailed::excel_time(value) {
        Ok(nanos) => success(ruby, nanos),
        Err(failed) => typed_fault(ruby, failed),
    }
}

/// Reads a number of days as exact Rational seconds.
fn excel_duration_door(ruby: &Ruby, value: f64) -> Result<Value, Error> {
    match detailed::excel_duration(value) {
        Ok(span) => duration_value(ruby, span),
        Err(failed) => typed_fault(ruby, failed),
    }
}

fn time_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    let text = utf8(ruby, text)?;
    verdict(ruby, text, with_bytes(text, |bytes| detailed::cast_time(bytes)))
}

fn duration_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    let text = utf8(ruby, text)?;
    match with_bytes(text, |bytes| detailed::cast_duration(bytes)) {
        Ok(span) => duration_value(ruby, span),
        Err(failed) => fault(ruby, text, failed),
    }
}

/// Exact Rational seconds, no float anywhere: nanos.quo(1e9) + whole seconds. (Rational +
/// Integer stays Rational; the parts each fit i64 where the total in nanoseconds would not.)
fn duration_value(ruby: &Ruby, span: core::Duration) -> Result<Value, Error> {
    let fraction: Value =
        i64::from(span.nanos).into_value_with(ruby).funcall("quo", (1_000_000_000i64,))?;
    let rational: Value = fraction.funcall("+", (span.seconds,))?;
    success(ruby, rational)
}

// --- Validation constraints ----------------------------------------------------------------
//
// A declaration is the package's own `Constraints` Data: hypercast.rb validates its keywords,
// converts its bounds, steps and integer entries through `declared_decimal` below (the
// conversion `cast_declared_decimal` runs), and lays it out exactly as the C ABI carries it, a
// `RawConstraints` with the packed `enum` list beside it — the same code on both backends. This
// side judges that layout with the core's own functions: `check_constraints` for
// `constraints_check` (which `Constraints.new` calls), and the `judge_*` functions the checked
// C exports call, on every cast. No rule is restated here: every refusal and every violation
// is the core's.

/// A declaration as the core reads it: its door, its layout and its packed `enum` list.
struct Declaration {
    door: core::Door,
    raw: core::RawConstraints,
    packed: Vec<u8>,
}

thread_local! {
    /// The last declaration this thread resolved, keyed by the `Constraints` object's
    /// `object_id` — a `Constraints` is a frozen `Data`, reused in practice, so the common call
    /// costs one id lookup instead of packing its layout again. Keyed as `LAST_FORMAT` is, and
    /// for its reason: an `object_id` survives compaction and is never reused.
    static LAST_CONSTRAINTS: RefCell<Option<(u64, Rc<Declaration>)>> = const { RefCell::new(None) };
}

/// The binding bug a refused judgement of an accepted declaration is, worded as the Fiddle
/// backend words the C ABI's -1.
fn binding_bug(ruby: &Ruby) -> Error {
    Error::new(
        ruby.exception_runtime_error(),
        "hypercast: libhypercast reported a contract violation — a binding bug, please report it",
    )
}

/// A `RawConstraints` from the 72 native-endian bytes `Constraints#packed` lays out, or `None`
/// for any other length.
fn raw_constraints(bytes: &[u8]) -> Option<core::RawConstraints> {
    let bytes: &[u8; core::RawConstraints::SIZE as usize] = bytes.try_into().ok()?;
    let word =
        |at: usize| u32::from_ne_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let decimal = |at: usize| {
        let mut lo = [0; 8];
        lo.copy_from_slice(&bytes[at..at + 8]);
        core::Decimal {
            lo: u64::from_ne_bytes(lo),
            hi: word(at + 8),
            scale: bytes[at + 12],
            negative: bytes[at + 13] != 0,
        }
    };
    Some(core::RawConstraints {
        size: word(0),
        flags: word(4),
        rounding: word(8),
        min_length: word(12),
        max_length: word(16),
        _reserved: word(20),
        lower: decimal(24),
        upper: decimal(40),
        multiple_of: decimal(56),
    })
}

/// A declaration from `[door code, packed, packed enum]`, as `Constraints#declaration` and
/// `Constraints.layout` hand it over.
fn declaration_of(ruby: &Ruby, declared: RArray) -> Result<Declaration, Error> {
    let code: u32 = declared.entry(0)?;
    let raw: RString = declared.entry(1)?;
    let packed: RString = declared.entry(2)?;
    let door = core::Door::from_code(code, 0).ok_or_else(|| binding_bug(ruby))?;
    let raw = with_bytes(raw, raw_constraints).ok_or_else(|| binding_bug(ruby))?;
    Ok(Declaration { door, raw, packed: with_bytes(packed, <[u8]>::to_vec) })
}

/// The declaration a checked door judges by, checked against that door — hypercast.rb's
/// `constrained`, with its messages: a non-`Constraints` is a TypeError, and one bound to the
/// other kind of door an ArgumentError naming the door that judges it.
fn declaration(ruby: &Ruby, constraints: Value, text: bool) -> Result<Rc<Declaration>, Error> {
    if !constraints.is_kind_of(ruby.get_inner(cached().constraints_class)) {
        return Err(Error::new(
            ruby.exception_type_error(),
            format!("constraints must be a HyperCast::Constraints; got {}", constraints.inspect()),
        ));
    }
    let id = object_id(constraints)?;
    let memo = LAST_CONSTRAINTS.with(|last| {
        last.borrow().as_ref().filter(|(key, _)| *key == id).map(|(_, declared)| declared.clone())
    });
    let declared = match memo {
        Some(declared) => declared,
        None => {
            let declared = Rc::new(declaration_of(ruby, constraints.funcall("declaration", ())?)?);
            LAST_CONSTRAINTS.with(|last| *last.borrow_mut() = Some((id, declared.clone())));
            declared
        }
    };
    if (declared.door == core::Door::Text) != text {
        let door: Value = constraints.funcall("door", ())?;
        let judge = if text { "checked" } else { "text" };
        return Err(Error::new(
            ruby.exception_arg_error(),
            format!("Constraints for {} are judged by HyperCast.{judge}", door.inspect()),
        ));
    }
    Ok(declared)
}

/// What a judge answers: `None` for a declaration the core refused.
type Judgement = Option<Result<CheckOutcome, detailed::Rejection>>;

/// The success case: the package's own `Checked` Data.
fn checked_value(ruby: &Ruby, value: Value, outcome: CheckOutcome) -> Result<Value, Error> {
    data(
        ruby.get_inner(cached().checked_class),
        [value, outcome.adjusted.into_value_with(ruby), outcome.enum_index.into_value_with(ruby)],
    )
}

/// A rejection: the door's `Fault` with its detail, or a `Violation` of the keyword broken
/// (which has none), each spanned over `text` (the UTF-8 the core read) as a fault is — or,
/// with no text, as the core wrote it.
fn rejected(
    ruby: &Ruby,
    text: Option<RString>,
    rejection: detailed::Rejection,
) -> Result<Value, Error> {
    match rejection {
        detailed::Rejection::Failure(failed) => match text {
            Some(text) => fault(ruby, text, failed),
            None => typed_fault(ruby, failed),
        },
        detailed::Rejection::Violation(broken) => {
            let (offset, length) = match text {
                Some(text) => character_span(text, broken.offset, broken.len),
                None => (i64::from(broken.offset), i64::from(broken.len)),
            };
            let cache = cached();
            let keyword = data(
                ruby.get_inner(cache.keyword_class),
                [broken.keyword.code().into_value_with(ruby)],
            )?;
            data(
                ruby.get_inner(cache.violation_class),
                [keyword, offset.into_value_with(ruby), length.into_value_with(ruby)],
            )
        }
    }
}

/// Judges at a numeric door through `judge`, which writes that door's own type into the
/// buffer it is handed, and presents the value as hypercast.rb's `Interop.decode` does: an
/// Integer, a Float (an f32 widened losslessly) or the package's `Decimal`.
fn numeric_verdict(
    ruby: &Ruby,
    door: core::Door,
    text: Option<RString>,
    judge: impl FnOnce(*mut u8) -> Judgement,
) -> Result<Value, Error> {
    // 16 bytes, 8-aligned: room for every numeric door's value, the decimal's included.
    let mut out = [0u64; 2];
    let outcome = match judge(out.as_mut_ptr().cast()) {
        None => return Err(binding_bug(ruby)),
        Some(Err(rejection)) => return rejected(ruby, text, rejection),
        Some(Ok(outcome)) => outcome,
    };
    let at = out.as_ptr().cast::<u8>();
    macro_rules! read {
        ($ty:ty) => {
            // SAFETY: the judge wrote this door's type at `at` on success; `out` holds 16
            // bytes, enough for any of them, and the read is unaligned-safe.
            unsafe { std::ptr::read_unaligned(at.cast::<$ty>()) }.into_value_with(ruby)
        };
    }
    let value = match door {
        core::Door::I8 => read!(i8),
        core::Door::I16 => read!(i16),
        core::Door::I32 => read!(i32),
        core::Door::I64 => read!(i64),
        core::Door::U8 => read!(u8),
        core::Door::U16 => read!(u16),
        core::Door::U32 => read!(u32),
        core::Door::U64 => read!(u64),
        // SAFETY: as `read!`; the f32 is widened exactly.
        core::Door::F32 => {
            f64::from(unsafe { std::ptr::read_unaligned(at.cast::<f32>()) }).into_value_with(ruby)
        }
        core::Door::F64 => read!(f64),
        // SAFETY: as `read!`; the decimal door writes a `Decimal`.
        core::Door::Decimal => {
            decimal_object(ruby, unsafe { std::ptr::read_unaligned(at.cast::<core::Decimal>()) })?
        }
        _ => return Err(binding_bug(ruby)),
    };
    checked_value(ruby, value, outcome)
}

/// The text door's value at the byte span the core judged valid — hypercast.rb's
/// `text_value`: the String passed when the span is all of it and it is not binary, else
/// that span as a new UTF-8 String.
fn text_value(ruby: &Ruby, text: RString, (offset, len): (u32, u32)) -> Result<Value, Error> {
    let whole = with_bytes(text, |bytes| offset == 0 && len as usize == bytes.len());
    if whole && text.enc_get() != ruby.ascii8bit_encindex() {
        return Ok(text.as_value());
    }
    let start = offset as usize;
    // Copied out before Ruby allocates: a compacting GC may move an embedded String's bytes.
    let span = with_bytes(text, |bytes| bytes.get(start..start + len as usize).map(<[u8]>::to_vec));
    let span = span.ok_or_else(|| binding_bug(ruby))?;
    Ok(ruby.enc_str_new(span, ruby.utf8_encoding()).as_value())
}

/// `text(text, constraints = nil)`: the plain text door, or with `Constraints` for `:text`
/// the checked one, a text `enum` match delivering the very String declared.
fn text_door(ruby: &Ruby, args: &[Value]) -> Result<Value, Error> {
    let args = scan_args::<(RString,), (Option<Value>,), (), (), (), ()>(args)?;
    let (text,) = args.required;
    let (constraints,) = args.optional;
    let text = utf8(ruby, text)?;
    let Some(constraints) = constraints.filter(|given| !given.is_nil()) else {
        return match with_bytes(text, |bytes| {
            // The door refuses text past `u32::MAX` bytes, so the length narrows exactly.
            detailed::cast_text(bytes).map(|valid| valid.len() as u32)
        }) {
            Ok(len) => success(ruby, text_value(ruby, text, (0, len))?),
            Err(failed) => fault(ruby, text, failed),
        };
    };
    let declared = declaration(ruby, constraints, true)?;
    let entries = Entries::packed(core::Door::Text, &declared.packed);
    let outcome =
        match with_bytes(text, |bytes| judge_text_detailed(bytes, &declared.raw, &entries)) {
            None => return Err(binding_bug(ruby)),
            Some(Err(rejection)) => return rejected(ruby, Some(text), rejection),
            Some(Ok(outcome)) => outcome,
        };
    let value = match outcome.enum_index {
        None => text_value(ruby, text, outcome.span)?,
        Some(index) => {
            let entries: RArray = constraints.funcall("enum", ())?;
            entries.entry(index as isize)?
        }
    };
    checked_value(ruby, value, outcome)
}

/// `checked(text, format, constraints, separator: nil, delimiter: nil, unit: nil)`: numeric
/// text through the door the `Constraints` are bound to, then judged by them.
fn checked_door(ruby: &Ruby, args: &[Value]) -> Result<Value, Error> {
    // SAFETY: called on the Ruby thread, inside the method call whose arguments these are.
    let keywords = unsafe { rb_sys::rb_keyword_given_p() } != 0;
    let (text, format, constraints) = if let ([text, format, constraints], false) = (args, keywords)
    {
        (RString::try_convert(*text)?, *format, *constraints)
    } else {
        let scanned = scan_args::<(RString, Value, Value), (), (), (), RHash, ()>(args)?;
        let (text, format, constraints) = scanned.required;
        let format: Value = format.funcall("override", (KwArgs(scanned.keywords),))?;
        (text, format, constraints)
    };
    let text = utf8(ruby, text)?;
    let declared = declaration(ruby, constraints, false)?;
    let format = resolve_format(ruby, format)?;
    let door = declared.door;
    let entries = Entries::packed(door, &declared.packed);
    numeric_verdict(ruby, door, Some(text), |out| {
        with_bytes(text, |bytes| {
            // SAFETY: `numeric_verdict` hands 16 bytes, room for the type `door` writes.
            unsafe {
                judge_numeric_detailed(door, bytes, format.format(), &declared.raw, &entries, out)
            }
            .map(|verdict| verdict.map_err(|rejection| detailed::classified(rejection, bytes)))
        })
    })
}

/// `checked_from_float(value, constraints)`: a number already decoded, judged on the
/// shortest decimal that names it. Every fault and violation has an empty span.
fn checked_from_float_door(ruby: &Ruby, value: f64, constraints: Value) -> Result<Value, Error> {
    let declared = declaration(ruby, constraints, false)?;
    let door = declared.door;
    let entries = Entries::packed(door, &declared.packed);
    numeric_verdict(ruby, door, None, |out| {
        // SAFETY: `numeric_verdict` hands 16 bytes, room for the type `door` writes.
        unsafe { judge_numeric_from_f64_detailed(door, value, &declared.raw, &entries, out) }
    })
}

/// `constraints_check(**keywords)`: the declaration `Constraints.new` would build, judged
/// without building it — laid out by hypercast.rb's `Constraints.layout` (a refused
/// conversion is its code 8), then by the core's `check_constraints`.
fn constraints_check_door(ruby: &Ruby, args: &[Value]) -> Result<u32, Error> {
    let scanned = scan_args::<(), (), (), (), RHash, ()>(args)?;
    let class = ruby.get_inner(cached().constraints_class);
    let layout: Value = class.funcall("layout", (KwArgs(scanned.keywords),))?;
    let Some(declared) = RArray::from_value(layout) else {
        return u32::try_convert(layout);
    };
    let declared = declaration_of(ruby, declared)?;
    let judged = core::check_constraints(declared.door, &declared.raw, &declared.packed);
    Ok(judged.map_or_else(|refusal| refusal.code(), |()| 0))
}

/// `declared_decimal(text)`: the conversion every bound, step and integer entry crosses, the
/// package's `Decimal`, or the ArgumentError a refused declaration raises (code 8), worded by
/// the core's own `Refusal`.
fn declared_decimal_door(ruby: &Ruby, text: RString) -> Result<Value, Error> {
    let text = utf8(ruby, text)?;
    match with_bytes(text, declared_decimal) {
        Ok(decimal) => decimal_object(ruby, decimal),
        Err(refusal) => Err(Error::new(
            ruby.exception_arg_error(),
            format!("Constraints refused (code {}): {refusal}.", refusal.code()),
        )),
    }
}

#[magnus::init(name = "hypercast_native")]
fn init(ruby: &Ruby) -> Result<(), Error> {
    // rb_define_module returns the existing module — lib/hypercast.rb has already defined
    // the pure-Fiddle surface; these definitions go in front of its FiddleBackend doors.
    let hypercast = ruby.define_module("HyperCast")?;
    let _ = CACHED.set(build_cache(ruby, hypercast)?);
    hypercast.define_singleton_method("bool", function!(bool_door, 1))?;
    hypercast.define_singleton_method("char", function!(char_door, 1))?;
    hypercast.define_singleton_method("i8", function!(i8_door, -1))?;
    hypercast.define_singleton_method("i16", function!(i16_door, -1))?;
    hypercast.define_singleton_method("i32", function!(i32_door, -1))?;
    hypercast.define_singleton_method("i64", function!(i64_door, -1))?;
    hypercast.define_singleton_method("u8", function!(u8_door, -1))?;
    hypercast.define_singleton_method("u16", function!(u16_door, -1))?;
    hypercast.define_singleton_method("u32", function!(u32_door, -1))?;
    hypercast.define_singleton_method("u64", function!(u64_door, -1))?;
    hypercast.define_singleton_method("f32", function!(f32_door, -1))?;
    hypercast.define_singleton_method("f64", function!(f64_door, -1))?;
    hypercast.define_singleton_method("decimal", function!(decimal_door, -1))?;
    hypercast.define_singleton_method("uuid", function!(uuid_door, 1))?;
    hypercast.define_singleton_method("timestamp", function!(timestamp_door, -1))?;
    hypercast.define_singleton_method("unix", function!(unix_door, 2))?;
    hypercast.define_singleton_method("excel_serial", function!(excel_serial_door, 2))?;
    hypercast.define_singleton_method("date", function!(date_door, -1))?;
    hypercast.define_singleton_method("datetime", function!(datetime_door, -1))?;
    hypercast.define_singleton_method("time", function!(time_door, 1))?;
    hypercast.define_singleton_method("duration", function!(duration_door, 1))?;
    hypercast
        .define_singleton_method("decimal_from_float", function!(decimal_from_float_door, 1))?;
    hypercast.define_singleton_method(
        "excel_serial_from_float",
        function!(excel_serial_from_float_door, 2),
    )?;
    hypercast.define_singleton_method("excel_time", function!(excel_time_door, 1))?;
    hypercast.define_singleton_method("excel_duration", function!(excel_duration_door, 1))?;
    hypercast.define_singleton_method("text", function!(text_door, -1))?;
    hypercast.define_singleton_method("checked", function!(checked_door, -1))?;
    hypercast
        .define_singleton_method("checked_from_float", function!(checked_from_float_door, 2))?;
    hypercast
        .define_singleton_method("constraints_check", function!(constraints_check_door, -1))?;
    hypercast.define_singleton_method("declared_decimal", function!(declared_decimal_door, 1))?;
    hypercast.define_singleton_method("native_version", function!(native_version, 0))?;
    Ok(())
}
