//! What `cargo no-panic` must also see: the public API no C export reaches.
//!
//! `cargo no-panic` checks only the functions marked `#[no_panic]` that are linked into its
//! cdylib, which is otherwise the C exports alone. [`Constraints::from_raw`] and
//! [`Constraints::pack_enum`] are generic and no export calls them, so without this module they
//! would never be instantiated in that build, and never checked. Each function here is one
//! instantiation, taking every input as an argument (pointers and lengths) so that no branch,
//! a short buffer's included, can be folded away.
//!
//! What the four detailed exports already instantiate under their own `#[no_panic]`, and what
//! this module adds for every path they don't reach:
//!
//! | Path | Reached by |
//! | --- | --- |
//! | every `detailed` text door at `&[u8]` (the 23 of `Door`, `cast_datetime_with` for 21) | `cast_detailed` |
//! | `detailed::decimal_from_f64`, `excel_serial`, `excel_time`, `excel_duration` | `cast_detailed_from_f64` |
//! | the checked pair's numeric and text judgement, classified at the span | `cast_checked_detailed`, `cast_checked_detailed_from_f64` |
//! | `RawNumFormat::resolve` (1.1's `check`), `ValidNumFormat::from_checked` | the detailed exports |
//! | `DateOptions::validate`, `MonthNames::from_packed`, and the month reader under it | `cast_detailed` |
//! | `Detail::code`, `Failure`'s fields | every detailed export |
//! | `Constraints::from_raw`, `pack_enum` at every door | [`hypercast_harness_from_raw_i8`] and the rest below |
//! | `Constraints::enum_entries` and its two iterators, over packed and built lists | [`hypercast_harness_enum_packed_numbers`], [`hypercast_harness_enum_packed_text`], [`hypercast_harness_enum_decimals`], [`hypercast_harness_enum_text`] |
//! | `MonthNames::pack` (packed and built), `from_entries` + `validate` | [`hypercast_harness_month_names_pack_packed`], [`hypercast_harness_month_names_pack_entries`], [`hypercast_harness_month_names_entries_validate`] |
//! | the detailed date doors under options built from raw parts | [`hypercast_harness_date_doors`] |
//! | the Rust-only detailed typed doors (the eight integers, `f32`, `bool`, Unix) | [`hypercast_harness_typed_doors`] |
//! | every detailed text door at `&str`, `detailed::cast_datetime`, `optional` | [`hypercast_harness_str_doors`] |
//! | `detailed::cast_checked` and `checked_from_f64` at every numeric type, `cast_text_checked`, `optional_checked` | [`hypercast_harness_detailed_checked_i8`] and the rest below |
//! | `Grouping::from_code`, `NumFormat::with_grouping`, `NumFormat::validate` | [`hypercast_harness_num_format`] |
//! | `Detail::from_code`, `Failure::new`, `Failure::fault` | [`hypercast_harness_detail`] |
//! | the text door's UTF-8 validation (simdutf8's check, core's on failure), named on its own | [`hypercast_harness_utf8`] |
//!
//! Compiled only under the `no-panic` feature, into the throwaway no-panic build: never into a
//! shipped library, so the export list and the symbol checks don't see it. The canary at the
//! bottom is compiled in only under `--cfg hypercast_canary`, where the proof must reject it,
//! by name (the Forge's `no_panic_canary` input on core-checks).

use core::slice;

use crate::abi::{RawConstraints, RawNumFormat};
use crate::constraints::{Constraints, NumericRules, Text, TextRules};
use crate::date_options::{DateOptions, MonthNames};
use crate::detail::{Detail, Failure};
use crate::detailed;
use crate::temporal::{DateOrder, DayPeriods, UnixPrecision};
use crate::verdict::{Decimal, DeclarationError, Grouping, NumFormat, Reason, ValidNumFormat};

/// What a refused declaration, or a list too long to size, answers.
const REFUSED: usize = usize::MAX;

/// # Safety
/// `ptr` points to `len` live elements when `len > 0` (it may be null when `len == 0`).
unsafe fn items<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if len == 0 || ptr.is_null() {
        &[]
    } else {
        // SAFETY: per the function contract.
        unsafe { slice::from_raw_parts(ptr, len) }
    }
}

/// # Safety
/// `ptr` points to `len` writable bytes when `len > 0` (it may be null when `len == 0`).
unsafe fn out<'a>(ptr: *mut u8, len: usize) -> &'a mut [u8] {
    if len == 0 || ptr.is_null() {
        &mut []
    } else {
        // SAFETY: per the function contract.
        unsafe { slice::from_raw_parts_mut(ptr, len) }
    }
}

/// # Safety
/// `raw` is null or points to a readable `RawConstraints`.
unsafe fn read(raw: *const RawConstraints) -> Option<RawConstraints> {
    // SAFETY: per the function contract; the layout is plain data.
    unsafe { raw.as_ref().copied() }
}

macro_rules! from_raw {
    ($($name:ident => $ty:ty),+ $(,)?) => {$(
        /// `from_raw` at this type's door: `0`, or the refusal's code (`u32::MAX` for a null
        /// `raw`).
        ///
        /// # Safety
        /// `raw` is null or readable; `entries` points to `len` live bytes when `len > 0`.
        #[unsafe(no_mangle)]
        #[no_panic::no_panic]
        pub unsafe extern "C" fn $name(raw: *const RawConstraints, entries: *const u8, len: usize) -> u32 {
            // SAFETY: per the function contract.
            let (Some(raw), entries) = (unsafe { read(raw) }, unsafe { items(entries, len) }) else {
                return u32::MAX;
            };
            match Constraints::<$ty>::from_raw(raw, entries) {
                Ok(_) => 0,
                Err(refusal) => refusal.code(),
            }
        }
    )+};
}

from_raw! {
    hypercast_harness_from_raw_i8 => i8,
    hypercast_harness_from_raw_i16 => i16,
    hypercast_harness_from_raw_i32 => i32,
    hypercast_harness_from_raw_i64 => i64,
    hypercast_harness_from_raw_u8 => u8,
    hypercast_harness_from_raw_u16 => u16,
    hypercast_harness_from_raw_u32 => u32,
    hypercast_harness_from_raw_u64 => u64,
    hypercast_harness_from_raw_f32 => f32,
    hypercast_harness_from_raw_f64 => f64,
    hypercast_harness_from_raw_decimal => Decimal,
    hypercast_harness_from_raw_text => Text,
}

fn packed<T>(constraints: Result<Constraints<'_, T>, crate::Refusal>, out: &mut [u8]) -> usize {
    match constraints {
        Ok(constraints) => constraints.pack_enum(out).unwrap_or(REFUSED),
        Err(_) => REFUSED,
    }
}

/// `pack_enum` over a packed integer list read by `from_raw` at the `i64` door: the length,
/// or `usize::MAX`.
///
/// # Safety
/// As the `from_raw` functions; `out` points to `out_len` writable bytes when `out_len > 0`.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_pack_packed_integers(
    raw: *const RawConstraints,
    entries: *const u8,
    len: usize,
    out_ptr: *mut u8,
    out_len: usize,
) -> usize {
    // SAFETY: per the function contract.
    let Some(raw) = (unsafe { read(raw) }) else { return REFUSED };
    // SAFETY: per the function contract.
    let (entries, out) = unsafe { (items(entries, len), out(out_ptr, out_len)) };
    packed(Constraints::<i64>::from_raw(raw, entries), out)
}

/// `pack_enum` over a packed text list read by `from_raw` at the text door.
///
/// # Safety
/// As [`hypercast_harness_pack_packed_integers`].
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_pack_packed_text(
    raw: *const RawConstraints,
    entries: *const u8,
    len: usize,
    out_ptr: *mut u8,
    out_len: usize,
) -> usize {
    // SAFETY: per the function contract.
    let Some(raw) = (unsafe { read(raw) }) else { return REFUSED };
    // SAFETY: per the function contract.
    let (entries, out) = unsafe { (items(entries, len), out(out_ptr, out_len)) };
    packed(Constraints::<Text>::from_raw(raw, entries), out)
}

/// `pack_enum` with no `enum`, the declaration read by `from_raw` at the `f64` door.
///
/// # Safety
/// `raw` is null or readable; `out` points to `out_len` writable bytes when `out_len > 0`.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_pack_none(
    raw: *const RawConstraints,
    out_ptr: *mut u8,
    out_len: usize,
) -> usize {
    // SAFETY: per the function contract.
    let Some(raw) = (unsafe { read(raw) }) else { return REFUSED };
    // SAFETY: per the function contract.
    let out = unsafe { out(out_ptr, out_len) };
    packed(Constraints::<f64>::from_raw(raw, &[]), out)
}

/// `pack_enum` over a list of [`Decimal`]s declared through `Constraints::numeric` at the
/// `i64` door, as a Rust caller declares one.
///
/// # Safety
/// `entries` points to `count` live `Decimal`s when `count > 0`; `out` as above.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_pack_decimals(
    entries: *const Decimal,
    count: usize,
    out_ptr: *mut u8,
    out_len: usize,
) -> usize {
    // SAFETY: per the function contract.
    let (entries, out) = unsafe { (items(entries, count), out(out_ptr, out_len)) };
    packed(Constraints::<i64>::numeric(NumericRules::new(), Some(entries)), out)
}

/// `pack_enum` over a list of `&str`s declared through `Constraints::text`, as a Rust caller
/// declares one.
///
/// # Safety
/// `entries` points to `count` live `&str`s when `count > 0`; `out` as above.
// A `&str` has no C layout, but nothing calls this from C: it exists to be linked and checked.
#[allow(improper_ctypes_definitions)]
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_pack_text(
    entries: *const &str,
    count: usize,
    out_ptr: *mut u8,
    out_len: usize,
) -> usize {
    // SAFETY: per the function contract.
    let (entries, out) = unsafe { (items(entries, count), out(out_ptr, out_len)) };
    packed(Constraints::text(TextRules::new(), Some(entries)), out)
}

/// Walks a numeric `enum` as `enum_entries` reads it: the entries' low words summed, wrapping,
/// or `u64::MAX` for no `enum` or a refused declaration.
fn walked_numbers<T: crate::Numeric>(
    constraints: Result<Constraints<'_, T>, crate::Refusal>,
) -> u64 {
    let Some(entries) = constraints.ok().and_then(|c| c.enum_entries()) else { return u64::MAX };
    entries.fold(0u64, |sum, entry| sum.wrapping_add(entry.lo))
}

/// Walks a text `enum` as `enum_entries` reads it: the entries' byte lengths summed, wrapping,
/// or `usize::MAX` for no `enum` or a refused declaration.
fn walked_texts(constraints: Result<Constraints<'_, Text>, crate::Refusal>) -> usize {
    let Some(entries) = constraints.ok().and_then(|c| c.enum_entries()) else { return REFUSED };
    let count = entries.len();
    entries.fold(count, |sum, entry| sum.wrapping_add(entry.len()))
}

/// `enum_entries` over a packed integer list read by `from_raw` at the `i64` door.
///
/// # Safety
/// As the `from_raw` functions.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_enum_packed_numbers(
    raw: *const RawConstraints,
    entries: *const u8,
    len: usize,
) -> u64 {
    // SAFETY: per the function contract.
    let Some(raw) = (unsafe { read(raw) }) else { return u64::MAX };
    // SAFETY: per the function contract.
    walked_numbers(Constraints::<i64>::from_raw(raw, unsafe { items(entries, len) }))
}

/// `enum_entries` over a packed text list read by `from_raw` at the text door.
///
/// # Safety
/// As the `from_raw` functions.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_enum_packed_text(
    raw: *const RawConstraints,
    entries: *const u8,
    len: usize,
) -> usize {
    // SAFETY: per the function contract.
    let Some(raw) = (unsafe { read(raw) }) else { return REFUSED };
    // SAFETY: per the function contract.
    walked_texts(Constraints::<Text>::from_raw(raw, unsafe { items(entries, len) }))
}

/// `enum_entries` over a list of [`Decimal`]s declared through `Constraints::numeric` at the
/// `i64` door.
///
/// # Safety
/// `entries` points to `count` live `Decimal`s when `count > 0`.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_enum_decimals(
    entries: *const Decimal,
    count: usize,
) -> u64 {
    // SAFETY: per the function contract.
    let entries = unsafe { items(entries, count) };
    walked_numbers(Constraints::<i64>::numeric(NumericRules::new(), Some(entries)))
}

/// `enum_entries` over a list of `&str`s declared through `Constraints::text`.
///
/// # Safety
/// `entries` points to `count` live `&str`s when `count > 0`.
// A `&str` has no C layout, but nothing calls this from C: it exists to be linked and checked.
#[allow(improper_ctypes_definitions)]
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_enum_text(entries: *const &str, count: usize) -> usize {
    // SAFETY: per the function contract.
    let entries = unsafe { items(entries, count) };
    walked_texts(Constraints::text(TextRules::new(), Some(entries)))
}

/// `MonthNames::from_packed` and the validation of options around it: `0` for a vocabulary
/// `validate` accepts, `1` for one `from_packed` refuses, `2` for options `validate` refuses.
///
/// # Safety
/// `names` points to `len` live bytes when `len > 0`.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_month_names_validate(
    flags: u32,
    pivot: u32,
    names: *const u8,
    len: usize,
) -> u32 {
    // SAFETY: per the function contract.
    let names = unsafe { items(names, len) };
    let Ok(names) = MonthNames::from_packed(names) else { return 1 };
    let Ok(pivot) = u16::try_from(pivot) else { return 2 };
    match DateOptions::from_flags(flags).with_pivot(pivot).validate(Some(names)) {
        Ok(_) => 0,
        Err(_) => 2,
    }
}

/// `MonthNames::pack` over a vocabulary read by `from_packed`: the length, or `usize::MAX`.
///
/// # Safety
/// `names` points to `len` live bytes when `len > 0`; `out` to `out_len` writable bytes when
/// `out_len > 0`.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_month_names_pack_packed(
    names: *const u8,
    len: usize,
    out_ptr: *mut u8,
    out_len: usize,
) -> usize {
    // SAFETY: per the function contract.
    let (names, out) = unsafe { (items(names, len), out(out_ptr, out_len)) };
    match MonthNames::from_packed(names) {
        Ok(names) => names.pack(out).unwrap_or(REFUSED),
        Err(_) => REFUSED,
    }
}

/// `MonthNames::pack` over entries built as a Rust caller builds them: the length, or
/// `usize::MAX`.
///
/// # Safety
/// `entries` points to `count` live `(u8, &str)`s when `count > 0`; `out` as above.
// A `&str` has no C layout, but nothing calls this from C: it exists to be linked and checked.
#[allow(improper_ctypes_definitions)]
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_month_names_pack_entries(
    entries: *const (u8, &str),
    count: usize,
    out_ptr: *mut u8,
    out_len: usize,
) -> usize {
    // SAFETY: per the function contract.
    let (entries, out) = unsafe { (items(entries, count), out(out_ptr, out_len)) };
    MonthNames::from_entries(entries).pack(out).unwrap_or(REFUSED)
}

/// The detailed date doors under options built from raw parts: `door` 0 `cast_date`, 1
/// `cast_timestamp`, 2 `cast_date_ordered`, 3 `cast_datetime_with`. The answer is `0` for a
/// value, the failure's detail code for a failure, and `u32::MAX` for a declaration the
/// options, the vocabulary, the order or the door refuse.
///
/// # Safety
/// `names` and `text` point to `names_len` and `text_len` live bytes when those are above 0.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_date_doors(
    door: u32,
    order: u32,
    flags: u32,
    pivot: u32,
    names: *const u8,
    names_len: usize,
    text: *const u8,
    text_len: usize,
) -> u32 {
    // SAFETY: per the function contract.
    let (names, text) = unsafe { (items(names, names_len), items(text, text_len)) };
    let months = if names.is_empty() {
        None
    } else {
        match MonthNames::from_packed(names) {
            Ok(months) => Some(months),
            Err(_) => return u32::MAX,
        }
    };
    let Ok(pivot) = u16::try_from(pivot) else { return u32::MAX };
    let Ok(options) = DateOptions::from_flags(flags).with_pivot(pivot).validate(months) else {
        return u32::MAX;
    };
    let options = Some(&options);
    let Some(order) = DateOrder::from_code(order) else { return u32::MAX };
    let detail = |verdict: Result<(), crate::Failure>| verdict.err().map_or(0, |f| f.detail.code());
    match door {
        0 => detail(detailed::cast_date(text, options).map(drop)),
        1 => detail(detailed::cast_timestamp(text, options).map(drop)),
        2 => detail(detailed::cast_date_ordered(text, order, options).map(drop)),
        3 => detail(
            detailed::cast_datetime_with(text, order, &DayPeriods::ENGLISH, options).map(drop),
        ),
        _ => u32::MAX,
    }
}

/// `MonthNames::from_entries`, the Rust caller's builder, judged by `DateOptions::validate`:
/// `0` for a vocabulary it accepts, `1` for one it refuses.
///
/// # Safety
/// `entries` points to `count` live `(u8, &str)`s when `count > 0`.
// A `&str` has no C layout, but nothing calls this from C: it exists to be linked and checked.
#[allow(improper_ctypes_definitions)]
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_month_names_entries_validate(
    entries: *const (u8, &str),
    count: usize,
    flags: u32,
    pivot: u16,
) -> u32 {
    // SAFETY: per the function contract.
    let entries = unsafe { items(entries, count) };
    let names = MonthNames::from_entries(entries);
    match DateOptions::from_flags(flags).with_pivot(pivot).validate(Some(names)) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

/// The failure's detail code, or `0` for a value.
fn detail_of<T>(verdict: Result<T, Failure>) -> u32 {
    verdict.err().map_or(0, |failure| failure.detail.code())
}

/// The detailed typed doors no C export has: `door` 0 to 7 the integers (`i8` … `u64`), 8
/// `f32`, 9 `bool`, 10 to 13 Unix at each precision. The detail code, `0` for a value,
/// `u32::MAX` for another door.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub extern "C" fn hypercast_harness_typed_doors(door: u32, value: f64) -> u32 {
    match door {
        0 => detail_of(detailed::i8_from_f64(value)),
        1 => detail_of(detailed::i16_from_f64(value)),
        2 => detail_of(detailed::i32_from_f64(value)),
        3 => detail_of(detailed::i64_from_f64(value)),
        4 => detail_of(detailed::u8_from_f64(value)),
        5 => detail_of(detailed::u16_from_f64(value)),
        6 => detail_of(detailed::u32_from_f64(value)),
        7 => detail_of(detailed::u64_from_f64(value)),
        8 => detail_of(detailed::f32_from_f64(value)),
        9 => detail_of(detailed::bool_from_f64(value)),
        10 => detail_of(detailed::unix_from_f64(value, UnixPrecision::Seconds)),
        11 => detail_of(detailed::unix_from_f64(value, UnixPrecision::Millis)),
        12 => detail_of(detailed::unix_from_f64(value, UnixPrecision::Micros)),
        13 => detail_of(detailed::unix_from_f64(value, UnixPrecision::Nanos)),
        _ => u32::MAX,
    }
}

/// Every detailed text door at a `&str`, the instantiation a Rust caller makes and no export
/// does (they all pass `&[u8]`), plus `detailed::cast_datetime` (English periods) and
/// `detailed::optional`. `door` is a [`Door`](crate::Door) code (`param` 1 for every door that
/// declares one); the detail code, `0` for a value, `u32::MAX` for text that isn't UTF-8 or a
/// door that isn't one.
///
/// # Safety
/// `text` points to `len` live bytes when `len > 0`.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_str_doors(
    door: u32,
    text: *const u8,
    len: usize,
) -> u32 {
    // SAFETY: per the function contract.
    let Ok(text) = core::str::from_utf8(unsafe { items(text, len) }) else { return u32::MAX };
    let format = &ValidNumFormat::INVARIANT;
    let order = DateOrder::YearMonthDay;
    match door {
        1 => detail_of(detailed::optional(detailed::cast_bool(text))),
        2 => detail_of(detailed::cast_i8(text, format)),
        3 => detail_of(detailed::cast_i16(text, format)),
        4 => detail_of(detailed::cast_i32(text, format)),
        5 => detail_of(detailed::cast_i64(text, format)),
        6 => detail_of(detailed::cast_u8(text, format)),
        7 => detail_of(detailed::cast_u16(text, format)),
        8 => detail_of(detailed::cast_u32(text, format)),
        9 => detail_of(detailed::cast_u64(text, format)),
        10 => detail_of(detailed::cast_f32(text, format)),
        11 => detail_of(detailed::cast_f64(text, format)),
        12 => detail_of(detailed::cast_uuid(text)),
        13 => detail_of(detailed::cast_timestamp(text, None)),
        14 => detail_of(detailed::cast_unix(text, UnixPrecision::Seconds)),
        15 => detail_of(detailed::cast_date(text, None)),
        16 => detail_of(detailed::cast_time(text)),
        17 => detail_of(detailed::cast_duration(text)),
        19 => detail_of(detailed::cast_decimal(text, format)),
        20 => detail_of(detailed::cast_date_ordered(text, order, None)),
        21 => detail_of(detailed::cast_datetime(text, order, None)),
        22 => detail_of(detailed::cast_excel_serial(text, crate::ExcelEpoch::Y1900)),
        23 => detail_of(detailed::cast_char(text)),
        _ => u32::MAX,
    }
}

/// The rejection's code: the failure's detail, `u32::MAX - 1` for a violation, `0` for a value.
fn rejected<T, V>(verdict: Result<T, detailed::Rejection<V>>) -> u32 {
    match verdict {
        Ok(_) => 0,
        Err(detailed::Rejection::Failure(failure)) => failure.detail.code(),
        Err(detailed::Rejection::Violation(_)) => u32::MAX - 1,
    }
}

/// # Safety
/// `format` is null or points to a readable `RawNumFormat`.
unsafe fn valid_format(format: *const RawNumFormat) -> Option<ValidNumFormat> {
    // SAFETY: per the function contract; the layout is plain data.
    match unsafe { format.as_ref() } {
        None => Some(ValidNumFormat::INVARIANT),
        Some(raw) => raw.format.validate().ok(),
    }
}

macro_rules! detailed_checked {
    ($($name:ident => $ty:ty),+ $(,)?) => {$(
        /// `detailed::cast_checked` on `text` (`typed` 0) or `detailed::checked_from_f64` on
        /// `value` (`typed` 1) at this type's door, with `optional_checked` around it, under a
        /// declaration read by `from_raw` and a format `NumFormat::validate` accepts: the
        /// rejection's code ([`rejected`]), `u32::MAX` for a refused declaration or format.
        ///
        /// # Safety
        /// `raw` and `format` are null or readable; `entries` and `text` point to `len` and
        /// `text_len` live bytes when those are above 0.
        #[unsafe(no_mangle)]
        #[no_panic::no_panic]
        pub unsafe extern "C" fn $name(
            raw: *const RawConstraints,
            entries: *const u8,
            len: usize,
            format: *const RawNumFormat,
            text: *const u8,
            text_len: usize,
            value: f64,
            typed: u32,
        ) -> u32 {
            // SAFETY: per the function contract.
            let (Some(raw), entries, text) =
                (unsafe { read(raw) }, unsafe { items(entries, len) }, unsafe { items(text, text_len) })
            else {
                return u32::MAX;
            };
            // SAFETY: per the function contract.
            let Some(format) = (unsafe { valid_format(format) }) else { return u32::MAX };
            let Ok(constraints) = Constraints::<$ty>::from_raw(raw, entries) else { return u32::MAX };
            let verdict = match typed {
                0 => detailed::cast_checked(text, &format, &constraints),
                _ => detailed::checked_from_f64(value, &constraints),
            };
            rejected(detailed::optional_checked(verdict))
        }
    )+};
}

detailed_checked! {
    hypercast_harness_detailed_checked_i8 => i8,
    hypercast_harness_detailed_checked_i16 => i16,
    hypercast_harness_detailed_checked_i32 => i32,
    hypercast_harness_detailed_checked_i64 => i64,
    hypercast_harness_detailed_checked_u8 => u8,
    hypercast_harness_detailed_checked_u16 => u16,
    hypercast_harness_detailed_checked_u32 => u32,
    hypercast_harness_detailed_checked_u64 => u64,
    hypercast_harness_detailed_checked_f32 => f32,
    hypercast_harness_detailed_checked_f64 => f64,
    hypercast_harness_detailed_checked_decimal => Decimal,
}

/// `detailed::cast_text_checked` under a declaration read by `from_raw` at the text door: the
/// rejection's code ([`rejected`]), `u32::MAX` for a refused declaration.
///
/// # Safety
/// `raw` is null or readable; `entries` and `text` point to `len` and `text_len` live bytes
/// when those are above 0.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_detailed_checked_text(
    raw: *const RawConstraints,
    entries: *const u8,
    len: usize,
    text: *const u8,
    text_len: usize,
) -> u32 {
    // SAFETY: per the function contract.
    let (Some(raw), entries, text) =
        (unsafe { read(raw) }, unsafe { items(entries, len) }, unsafe { items(text, text_len) })
    else {
        return u32::MAX;
    };
    let Ok(constraints) = Constraints::<Text>::from_raw(raw, entries) else { return u32::MAX };
    rejected(detailed::cast_text_checked(text, &constraints))
}

/// The text door's UTF-8 validation (simdutf8 where the target has it, core otherwise), at
/// every length and both answers: the text's length, or the error's position and length folded
/// into one word.
///
/// # Safety
/// `text` points to `len` live bytes when `len > 0`.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_utf8(text: *const u8, len: usize) -> usize {
    // SAFETY: per the function contract.
    match crate::utf8::from_utf8(unsafe { items(text, len) }) {
        Ok(text) => text.len(),
        Err(e) => e.valid_up_to() ^ !e.error_len().map_or(0, usize::from),
    }
}

/// `Grouping::from_code`, `NumFormat::with_grouping` then `NumFormat::validate` on the format at
/// `format`: `0` for a valid format, else which rule refused it (`1` policy, `2` an undefined
/// flag, `3` the 1.0 rules), `u32::MAX` for a null pointer.
///
/// # Safety
/// `format` is null or points to a readable `RawNumFormat`.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub unsafe extern "C" fn hypercast_harness_num_format(
    format: *const RawNumFormat,
    policy: u32,
) -> u32 {
    // SAFETY: per the function contract; the layout is plain data.
    let Some(raw) = (unsafe { format.as_ref() }) else { return u32::MAX };
    let Some(policy) = Grouping::from_code(policy) else { return 1 };
    let format: NumFormat = raw.format.with_grouping(policy);
    match format.validate() {
        Ok(_) => 0,
        Err(DeclarationError::GroupingPolicy) => 1,
        Err(DeclarationError::UnknownFlag) => 2,
        Err(_) => 3,
    }
}

/// `Detail::from_code`, then a `Failure` of it projected by `Failure::fault`: the fault's
/// offset, `u32::MAX` for a code no detail has.
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub extern "C" fn hypercast_harness_detail(code: u32, reason: u32, offset: u32, len: u32) -> u32 {
    let Some(detail) = Detail::from_code(code) else { return u32::MAX };
    let reason = match reason {
        1 => Reason::Empty,
        2 => Reason::Malformed,
        _ => Reason::OutOfRange,
    };
    let fault = Failure::new(reason, detail, offset, len).fault();
    fault.offset.wrapping_add(fault.len).wrapping_add(fault.reason as u32)
}

// What the proof must refuse. Compiled only when core-checks asks, to watch it fail: a proof
// that has never rejected anything has not been shown to check anything.
#[cfg(hypercast_canary)]
#[unsafe(no_mangle)]
#[no_panic::no_panic]
pub extern "C" fn hypercast_canary(value: u32) -> u32 {
    assert!(value != 12_345, "the canary");
    value
}
