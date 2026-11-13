//! C-ABI exports — built as a `cdylib` (`cargo cdylib`), this same source produces a
//! native `libhypercast.so`/`.dylib`/`.dll` loaded through ordinary P/Invoke/FFM/ctypes-style
//! FFI. This is the one contract every host binding calls through: a caller shares this
//! library's address space directly, so every export takes plain pointers into the caller's
//! own stack- or heap-allocated buffers — no allocator exports, no protocol beyond "here are
//! `len` UTF-8 bytes, fill in the out-value or the fault span".
//!
//! Return codes are the verdict discriminant every binding folds into its union:
//! `0` success, `1` empty, `2` malformed, `3` out of range — and `-1` for a contract
//! violation (an undefined precision discriminant, an invalid or self-colliding numeric
//! format), which is a caller bug, not a data verdict. On failure the nullable `fault`
//! out-param (when non-null) receives the offending byte span, indexed into the caller's
//! input; on success and contract violation it is left untouched. A `len` of 0 never
//! dereferences `ptr`.
//!
//! The four typed doors (`cast_decimal_from_f64`, `cast_excel_serial_from_f64`,
//! `cast_excel_time`, `cast_excel_duration`) take a `double` instead of text and keep the
//! same contract; there is no text for a span to index, so a fault's span is always empty.
//!
//! The validation constraints add five: the text door (`cast_text`), the two checked doors
//! (`cast_checked`, `cast_checked_from_f64`), which add return code `4`, a violation of a
//! declared rule, `cast_constraints_check`, which judges a declaration once and answers
//! with a refusal code rather than a verdict, and `cast_declared_decimal`, which converts a
//! declaration's decimal strings so no binding restates that conversion.
//!
//! The four detailed exports (`cast_detailed`, `cast_detailed_from_f64`,
//! `cast_checked_detailed`, `cast_checked_detailed_from_f64`) reach the same doors by their
//! [`Door`] code and report each failure's [`Detail`](crate::Detail) in a
//! [`RawFaultDetail`]. They judge a format by 1.1's rules, read the date options, and validate
//! everything before writing anything.

use crate::abi::{
    self, RawCheck, RawConstraints, RawDateOptions, RawDayPeriods, RawFaultDetail, RawNumFormat,
    RawSpan,
};
use crate::checked::{
    CheckOutcome, exact_span, judge_numeric_detailed, judge_numeric_from_f64_detailed,
    judge_text_detailed,
};
use crate::constraints::{Entries, Refusal, check_constraints, declared_decimal};
use crate::date_options::{DateOptions, MonthNames, ValidDateOptions};
use crate::detail::Failure;
use crate::detailed::{self, Rejection};
use crate::door::Door;
use crate::temporal::DayPeriods;
use crate::verdict::{
    CivilDateTime, Date, Decimal, Duration, Fault, NumFormat, Timestamp, ValidNumFormat,
};
use crate::{
    DateOrder, ExcelEpoch, Reason, UnixPrecision, boolean, character, decimal, integer, real,
    temporal, typed, uuid,
};
use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::slice;

const CONTRACT_VIOLATION: i32 = -1;

/// The fault span as it crosses the ABI — the reason travels as the return code.
#[repr(C)]
pub struct RawFault {
    /// Where the offending bytes start, as an offset into the caller's input.
    pub offset: u32,
    /// How many bytes they run; 0 for a typed door, which has no text to index.
    pub len: u32,
}

// Pinned like abi.rs's shapes: every binding restates it as two u32s.
const _: () = assert!(size_of::<RawFault>() == 8 && align_of::<RawFault>() == 4);

/// # Safety
/// Caller guarantees `ptr` points to `len` live bytes when `len > 0`.
unsafe fn text<'caller>(ptr: *const u8, len: usize) -> &'caller [u8] {
    if len == 0 {
        // Some callers reasonably pass null for empty input, and `slice::from_raw_parts`
        // requires non-null even for a 0-length slice.
        &[]
    } else {
        // SAFETY: per the function contract.
        unsafe { slice::from_raw_parts(ptr, len) }
    }
}

/// Reads a size-tagged layout in place, or, when the caller built fewer bytes than the
/// core's own layout, exactly those bytes over `zero` in `scratch`, so a field the caller's
/// layout predates declares nothing. `None` for a size the core does not read.
///
/// # Safety
/// Caller guarantees `ptr` points to a live, 4-byte aligned `u32` size word followed by
/// that many bytes in all.
#[inline]
unsafe fn read_sized<T: Copy>(
    ptr: *const T,
    zero: T,
    min_size: u32,
    scratch: &mut MaybeUninit<T>,
) -> Option<&T> {
    let full = size_of::<T>() as u32;
    // SAFETY: a non-null layout starts with its size word, per the function contract.
    let size = unsafe { ptr.cast::<u32>().read() };
    if size == full {
        // SAFETY: the caller built the whole layout, and it outlives this call.
        return Some(unsafe { &*ptr });
    }
    if size < min_size || size > full {
        return None;
    }
    let read = scratch.write(zero);
    // SAFETY: the caller built `size` bytes, `size` is under `T`'s own, and `read` holds
    // all of `T`; the two never overlap.
    unsafe {
        core::ptr::copy_nonoverlapping(
            ptr.cast::<u8>(),
            (&raw mut *read).cast::<u8>(),
            size as usize,
        )
    };
    Some(read)
}

/// The `flags` bits a 1.0 export reads: bits 8 and up (the grouping policy, and whatever a
/// later version defines) are ignored, so these exports always parse with Western grouping.
/// That is 1.0's layout, and 1.1's default policy: these exports refuse the grouped shapes
/// Western does (`0,125`, `12,34`, `1234,567`, `1,23,456`), which 1.0 read.
const FLAGS_1_0: u32 = 0xFF;

/// The format a 1.0 export parses with: the caller's layout with `flags` cut to
/// [`FLAGS_1_0`], judged by [`RawNumFormat::resolve`]. The caller's memory is never written.
///
/// # Safety
/// Caller guarantees `format` is null or points to a live, 4-byte aligned [`RawNumFormat`]
/// of the size its first word says.
#[inline]
unsafe fn resolve_format(
    format: *const RawNumFormat,
    scratch: &mut MaybeUninit<RawNumFormat>,
) -> Option<&NumFormat> {
    if format.is_null() {
        return Some(&NumFormat::INVARIANT);
    }
    // SAFETY: a non-null layout starts with its size word, per the function contract.
    let size = unsafe { format.cast::<u32>().read() };
    // `flags` is the second word; a layout too short to hold it is refused below.
    let flags = if size >= 8 {
        // SAFETY: the caller built at least `size` bytes, so the second word.
        unsafe { format.cast::<u32>().add(1).read() }
    } else {
        0
    };
    if flags & !FLAGS_1_0 == 0 {
        // SAFETY: per the function contract.
        return unsafe { read_sized(format, RawNumFormat::ZERO, RawNumFormat::MIN_SIZE, scratch) }?
            .resolve();
    }
    // SAFETY: per the function contract.
    let mut masked =
        *unsafe { read_sized(format, RawNumFormat::ZERO, RawNumFormat::MIN_SIZE, scratch) }?;
    masked.format.flags &= FLAGS_1_0;
    scratch.write(masked).resolve()
}

/// # Safety
/// Caller guarantees `periods` is null or points to a live, 4-byte aligned
/// [`RawDayPeriods`] of the size its first word says.
#[inline]
unsafe fn resolve_periods(
    periods: *const RawDayPeriods,
    scratch: &mut MaybeUninit<RawDayPeriods>,
) -> Option<&DayPeriods> {
    if periods.is_null() {
        return Some(&DayPeriods::ENGLISH);
    }
    // SAFETY: per the function contract.
    unsafe { read_sized(periods, RawDayPeriods::ZERO, RawDayPeriods::MIN_SIZE, scratch) }?.resolve()
}

/// # Safety
/// Caller guarantees `out` points to a live `T` and `fault` is null or points to a live
/// [`RawFault`].
unsafe fn finish<T>(verdict: Result<T, Fault>, out: *mut T, fault: *mut RawFault) -> i32 {
    match verdict {
        Ok(value) => {
            // SAFETY: caller guarantees `out` points to a live T.
            unsafe { out.write(value) };
            0
        }
        Err(failed) => {
            if !fault.is_null() {
                // SAFETY: caller guarantees a non-null `fault` points to a live RawFault.
                unsafe { fault.write(RawFault { offset: failed.offset, len: failed.len }) };
            }
            failed.reason as i32
        }
    }
}

/// A typed door's verdict as a text door's: the same reason, over an empty span.
fn spanless<T>(verdict: Result<T, Reason>) -> Result<T, Fault> {
    verdict.map_err(|reason| Fault { reason, offset: 0, len: 0 })
}

/// This library's version, packed `major << 16 | minor << 8 | patch` from the crate's own
/// manifest — so a host can prove the library it loaded is the one its binding was built
/// against before making the first cast, and can name the mismatch when it isn't. Takes
/// nothing, touches nothing: the cheapest possible "did the native library resolve" probe.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn hypercast_version() -> u32 {
    abi::hypercast_version()
}

/// Casts boolean text at `ptr`/`len` into `out`: 1 for true, 0 for false.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_bool(ptr: *const u8, len: usize, out: *mut u8, fault: *mut RawFault) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(boolean::cast_bool(text(ptr, len)).map(u8::from), out, fault) }
}

/// Casts char text at `ptr`/`len` into `out` as a Unicode scalar value (never a surrogate,
/// never past U+10FFFF).
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_char(
    ptr: *const u8,
    len: usize,
    out: *mut u32,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(character::cast_char(text(ptr, len)).map(u32::from), out, fault) }
}

// Every numeric door is certified, the two real ones included: their conversion is this
// crate's own (float.rs), because `core`'s float parser keeps slice-index checks the
// optimizer cannot discharge.

/// The numeric doors' one body: resolves `format`, then hands the text and the resolved
/// format to `door`.
///
/// # Safety
/// The export's own contract, per the module doc.
#[inline(always)]
unsafe fn numeric<'caller, T>(
    door: impl FnOnce(&'caller [u8], &NumFormat) -> Result<T, Fault>,
    ptr: *const u8,
    len: usize,
    format: *const RawNumFormat,
    out: *mut T,
    fault: *mut RawFault,
) -> i32 {
    let mut scratch = MaybeUninit::uninit();
    // SAFETY: per the function contract.
    unsafe {
        let Some(resolved) = resolve_format(format, &mut scratch) else {
            return CONTRACT_VIOLATION;
        };
        finish(door(text(ptr, len), resolved), out, fault)
    }
}

/// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
/// outside `int8_t` is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_i8(
    ptr: *const u8,
    len: usize,
    format: *const RawNumFormat,
    out: *mut i8,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { numeric(integer::cast_i8, ptr, len, format, out, fault) }
}

/// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
/// outside `int16_t` is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_i16(
    ptr: *const u8,
    len: usize,
    format: *const RawNumFormat,
    out: *mut i16,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { numeric(integer::cast_i16, ptr, len, format, out, fault) }
}

/// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
/// outside `int32_t` is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_i32(
    ptr: *const u8,
    len: usize,
    format: *const RawNumFormat,
    out: *mut i32,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { numeric(integer::cast_i32, ptr, len, format, out, fault) }
}

/// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
/// outside `int64_t` is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_i64(
    ptr: *const u8,
    len: usize,
    format: *const RawNumFormat,
    out: *mut i64,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { numeric(integer::cast_i64, ptr, len, format, out, fault) }
}

/// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
/// outside `uint8_t` is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_u8(
    ptr: *const u8,
    len: usize,
    format: *const RawNumFormat,
    out: *mut u8,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { numeric(integer::cast_u8, ptr, len, format, out, fault) }
}

/// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
/// outside `uint16_t` is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_u16(
    ptr: *const u8,
    len: usize,
    format: *const RawNumFormat,
    out: *mut u16,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { numeric(integer::cast_u16, ptr, len, format, out, fault) }
}

/// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
/// outside `uint32_t` is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_u32(
    ptr: *const u8,
    len: usize,
    format: *const RawNumFormat,
    out: *mut u32,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { numeric(integer::cast_u32, ptr, len, format, out, fault) }
}

/// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
/// outside `uint64_t` is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_u64(
    ptr: *const u8,
    len: usize,
    format: *const RawNumFormat,
    out: *mut u64,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { numeric(integer::cast_u64, ptr, len, format, out, fault) }
}

/// Casts real text at `ptr`/`len` under the declared `format` into `out`; a magnitude
/// beyond `float`'s finite range is out of range.
/// Never writes a negative zero: `-0`, `(0)` and an underflow like `-1e-400` are `0.0`.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_f32(
    ptr: *const u8,
    len: usize,
    format: *const RawNumFormat,
    out: *mut f32,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { numeric(real::cast_f32, ptr, len, format, out, fault) }
}

/// Casts real text at `ptr`/`len` under the declared `format` into `out`; a magnitude
/// beyond `double`'s finite range is out of range.
/// Never writes a negative zero: `-0`, `(0)` and an underflow like `-1e-400` are `0.0`.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_f64(
    ptr: *const u8,
    len: usize,
    format: *const RawNumFormat,
    out: *mut f64,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { numeric(real::cast_f64, ptr, len, format, out, fault) }
}

/// Casts decimal text at `ptr`/`len` under the declared `format` into `out`, exact and
/// canonical; a magnitude past 96 bits, or more fractional precision than 28 places hold
/// once exact trailing zeros are trimmed, is out of range. Never rounds.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_decimal(
    ptr: *const u8,
    len: usize,
    format: *const RawNumFormat,
    out: *mut Decimal,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { numeric(decimal::cast_decimal, ptr, len, format, out, fault) }
}

/// Casts UUID text at `ptr`/`len` into the 16 bytes at `out`, RFC 9562 order.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_uuid(ptr: *const u8, len: usize, out: *mut u8, fault: *mut RawFault) -> i32 {
    // SAFETY: caller guarantees the pointer contracts (`out` is 16 live bytes).
    unsafe { finish(uuid::cast_uuid(text(ptr, len)), out.cast::<[u8; 16]>(), fault) }
}

/// Casts an RFC 3339 instant at `ptr`/`len` into `out`.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_timestamp(
    ptr: *const u8,
    len: usize,
    out: *mut Timestamp,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(temporal::cast_timestamp(text(ptr, len)), out, fault) }
}

/// Casts an integer Unix-epoch value at `ptr`/`len` under the declared `precision`
/// (1 seconds, 2 milliseconds, 3 microseconds, 4 nanoseconds — anything else is a contract
/// violation) into `out`.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_unix(
    ptr: *const u8,
    len: usize,
    precision: u32,
    out: *mut Timestamp,
    fault: *mut RawFault,
) -> i32 {
    let Some(precision) = UnixPrecision::from_code(precision) else {
        return CONTRACT_VIOLATION;
    };
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(temporal::cast_unix(text(ptr, len), precision), out, fault) }
}

/// Casts an Excel date serial at `ptr`/`len` under the declared `epoch` (1 the 1900 system,
/// 2 the 1904 system — anything else is a contract violation) into `out`.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_excel_serial(
    ptr: *const u8,
    len: usize,
    epoch: u32,
    out: *mut Timestamp,
    fault: *mut RawFault,
) -> i32 {
    let Some(epoch) = ExcelEpoch::from_code(epoch) else {
        return CONTRACT_VIOLATION;
    };
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(temporal::cast_excel_serial(text(ptr, len), epoch), out, fault) }
}

/// Casts a separated calendar date at `ptr`/`len` under the caller-declared field `order`
/// (1 year-month-day, 2 month-day-year, 3 day-month-year — anything else is a contract
/// violation) into `out`.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_date_ordered(
    ptr: *const u8,
    len: usize,
    order: u32,
    out: *mut Date,
    fault: *mut RawFault,
) -> i32 {
    let Some(order) = DateOrder::from_code(order) else {
        return CONTRACT_VIOLATION;
    };
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(temporal::cast_date_ordered(text(ptr, len), order), out, fault) }
}

/// Casts a zone-less civil date-time at `ptr`/`len` under the caller-declared field
/// `order` (1 year-month-day, 2 month-day-year, 3 day-month-year — anything else is a
/// contract violation) and the declared day-period designators at `periods` (null ⇒ the
/// English `AM`/`PM` only; an invalid designator is a contract violation) into `out`.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_datetime(
    ptr: *const u8,
    len: usize,
    order: u32,
    periods: *const RawDayPeriods,
    out: *mut CivilDateTime,
    fault: *mut RawFault,
) -> i32 {
    let Some(order) = DateOrder::from_code(order) else {
        return CONTRACT_VIOLATION;
    };
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe {
        let mut scratch = MaybeUninit::uninit();
        let Some(periods) = resolve_periods(periods, &mut scratch) else {
            return CONTRACT_VIOLATION;
        };
        finish(temporal::cast_datetime_with(text(ptr, len), order, periods), out, fault)
    }
}

/// Casts a strict `yyyy-MM-dd` date at `ptr`/`len` into `out`.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_date(
    ptr: *const u8,
    len: usize,
    out: *mut Date,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(temporal::cast_date(text(ptr, len)), out, fault) }
}

/// Casts an ISO 24-hour time-of-day at `ptr`/`len` into `out` as nanoseconds since
/// midnight.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_time(
    ptr: *const u8,
    len: usize,
    out: *mut u64,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(temporal::cast_time(text(ptr, len)), out, fault) }
}

/// Casts a duration at `ptr`/`len` (ISO 8601, invariant colon form, or protobuf JSON
/// seconds) into `out`.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_duration(
    ptr: *const u8,
    len: usize,
    out: *mut Duration,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(temporal::cast_duration(text(ptr, len)), out, fault) }
}

/// Reads the number `value` into `out` as the shortest decimal that names it, canonical as
/// `cast_decimal`'s. NaN is malformed; an infinity, a magnitude past 2^96 - 1 or more than
/// 28 places is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_decimal_from_f64(
    value: f64,
    out: *mut Decimal,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(spanless(typed::decimal_from_f64(value)), out, fault) }
}

/// Reads the Excel serial `value` under the declared `epoch` (1 the 1900 system, 2 the 1904
/// system — anything else is a contract violation) into `out` as the zone-less wall clock
/// it names, its fraction snapped to the fewest fractional-second digits that store as
/// this same double.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_excel_serial_from_f64(
    value: f64,
    epoch: u32,
    out: *mut CivilDateTime,
    fault: *mut RawFault,
) -> i32 {
    let Some(epoch) = ExcelEpoch::from_code(epoch) else {
        return CONTRACT_VIOLATION;
    };
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(spanless(temporal::excel_serial(value, epoch)), out, fault) }
}

/// Reads the time of day of the Excel serial `value` into `out` as nanoseconds since
/// midnight, snapped as `cast_excel_serial_from_f64` snaps. A negative, NaN or infinite
/// value is malformed; a serial past either date system's last day is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_excel_time(value: f64, out: *mut u64, fault: *mut RawFault) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(spanless(temporal::excel_time(value)), out, fault) }
}

/// Reads `value` days into `out` as a duration, snapped as `cast_excel_serial_from_f64`
/// snaps a time. NaN or an infinity is malformed; past ±10,000 years is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_excel_duration(value: f64, out: *mut Duration, fault: *mut RawFault) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(spanless(temporal::excel_duration(value)), out, fault) }
}

// The validation constraints. A declaration is a 104-byte `RawConstraints` beside an `enum`
// list of `enum_len` bytes at `enum_ptr` (never dereferenced when `enum_len` is 0), read only
// under `HAS_ENUM`, by the door: at a numeric door, `Decimal`s in their native-endian 16-byte
// layout, compared as exact values (`7.0` is `7`); at the text door, entries each a
// little-endian `u32` byte length followed by that many UTF-8 bytes, with no padding.

/// The declaration a caller passed, copied, or `None` when it is null, its `size` is not the
/// layout's (nothing past the size word of a layout of unknown size can be read), or one of
/// its five decimals has a sign byte other than `0` or `1`, which is no Rust `bool`: those
/// bytes are read as bytes before the layout is read as a `RawConstraints`.
///
/// # Safety
/// Caller guarantees `constraints` is null or points to a live, 8-byte aligned `u32` size
/// word, followed by the rest of a [`RawConstraints`] when that word is its size.
#[inline]
unsafe fn resolve_constraints(constraints: *const RawConstraints) -> Option<RawConstraints> {
    use core::mem::offset_of;
    if constraints.is_null() {
        return None;
    }
    // SAFETY: a non-null layout starts with its size word, per the function contract.
    let size = unsafe { constraints.cast::<u32>().read() };
    if size != RawConstraints::SIZE {
        return None;
    }
    let sign = offset_of!(Decimal, negative);
    for decimal in [
        offset_of!(RawConstraints, minimum),
        offset_of!(RawConstraints, maximum),
        offset_of!(RawConstraints, multiple_of),
        offset_of!(RawConstraints, exclusive_minimum),
        offset_of!(RawConstraints, exclusive_maximum),
    ] {
        // SAFETY: the caller built the whole layout, and the offset lies inside it.
        let byte = unsafe { constraints.cast::<u8>().add(decimal + sign).read() };
        if byte > 1 {
            return None;
        }
    }
    // SAFETY: the caller built the whole layout, and every `bool` in it is 0 or 1.
    Some(unsafe { constraints.read() })
}

/// The door a 1.0 export names: [`Door::from_code`] over the codes 1.0 knew, so 23
/// (`Door::Char`, from 1.1) is still no door there and every 1.0 export answers for it as 1.0
/// did.
#[inline(always)]
fn door_1_0(code: u32, param: u32) -> Option<Door> {
    Door::from_code(code, param).filter(|door| *door != Door::Char)
}

/// A checked door's verdict as its return code, with `fault` handed the span and the detail
/// (`0` for a violation) on codes 1 to 4, and the report written to `check` on 0 and 4 (the
/// value itself already written to `out` on both).
/// `None` is a contract violation and writes nothing.
///
/// # Safety
/// Caller guarantees `check` is null or points to a live [`RawCheck`].
#[inline(always)]
unsafe fn finish_checked(
    verdict: Option<Result<CheckOutcome, Rejection<()>>>,
    fault: impl FnOnce(u32, u32, u32),
    check: *mut RawCheck,
) -> i32 {
    let (code, offset, len, detail, report) = match verdict {
        None => return CONTRACT_VIOLATION,
        Some(Ok(outcome)) => {
            let report = RawCheck {
                keyword: 0,
                adjusted: u32::from(outcome.adjusted),
                enum_index: outcome.enum_index.unwrap_or(u32::MAX),
            };
            if !check.is_null() {
                // SAFETY: caller guarantees a non-null `check` points to a live RawCheck.
                unsafe { check.write(report) };
            }
            return 0;
        }
        Some(Err(Rejection::Failure(failed))) => {
            (failed.reason as i32, failed.offset, failed.len, failed.detail.code(), None)
        }
        Some(Err(Rejection::Violation(broken))) => {
            let report =
                RawCheck { keyword: broken.keyword.code(), adjusted: 0, enum_index: u32::MAX };
            (4, broken.offset, broken.len, 0, Some(report))
        }
    };
    fault(offset, len, detail);
    if let Some(report) = report
        && !check.is_null()
    {
        // SAFETY: caller guarantees a non-null `check` points to a live RawCheck.
        unsafe { check.write(report) };
    }
    code
}

/// Writes the checked text door's `out`: the judged text's span, on a success and on a
/// violation, which carries the text; nothing on a failure.
///
/// # Safety
/// Caller guarantees `out` is valid for an unaligned write of a [`RawSpan`].
#[inline(always)]
unsafe fn write_checked_span(judged: &Result<CheckOutcome, Rejection<()>>, out: *mut c_void) {
    let (offset, len) = match judged {
        Ok(outcome) => outcome.span,
        Err(Rejection::Violation(broken)) => (broken.offset, broken.len),
        Err(Rejection::Failure(_)) => return,
    };
    // SAFETY: per the function contract; unaligned, as every checked door writes `out`.
    unsafe { out.cast::<RawSpan>().write_unaligned(RawSpan { offset, len }) };
}

/// A 1.0 checked export's fault: the span, written when `fault` is not null.
///
/// # Safety
/// Caller guarantees `fault` is null or points to a live [`RawFault`].
#[inline(always)]
unsafe fn plain_fault(fault: *mut RawFault) -> impl FnOnce(u32, u32, u32) {
    move |offset, len, _| {
        if !fault.is_null() {
            // SAFETY: caller guarantees a non-null `fault` points to a live RawFault.
            unsafe { fault.write(RawFault { offset, len }) };
        }
    }
}

/// Casts the text at `ptr`/`len` through door `door` (with `param`, the precision, order or
/// date system a door declares; the door numbering is `Door`'s, 4 I32, 11 F64, 18 Text, 19
/// Decimal) under the declared `format` (null for the invariant one; ignored by Text), then
/// judges the value by `constraints` and the `enum` list at `enum_ptr`/`enum_len` (integer
/// entries native-endian `Decimal`s, 16 bytes each; text entries a little-endian `u32` length
/// followed by UTF-8 bytes). Returns 0 with the value written to `out`, the type the door
/// writes (a `RawSpan` of the input for Text, which with an `enum` is the matched input, or 3
/// when that span would end past `UINT32_MAX` bytes); 1, 2 or 3, the door's own fault, the
/// input not the type (`out` untouched); or 4, a value of the type that breaks a declared
/// rule, written to `out` exactly as cast (never rounded: `multipleOf`'s rounding is part of
/// the judgement; for Text, the span of the judged text), with `fault` spanning it. A
/// rounding past what the type holds is 4, `multipleOf`. At a float door the float written is
/// judged too, against each bound read as the door reads it (to the nearest float, ties to
/// even), so a value inside a bound whose float lands on or past it is a violation of that
/// bound.
/// Codes 1 to 4 write `fault`; 0 and 4 write `out`, and `check` when it is not null
/// (`keyword` on 4, `adjusted` and `enum_index` on 0). -1 is a contract violation: a null
/// `constraints`, a `size` other than `HyperCastRawConstraints_SIZE`, a door that is not
/// numeric or Text, a format the core refuses, or a declaration `cast_constraints_check`
/// refuses. With duplicate `enum` entries, the first matching entry wins. `out` need not be
/// aligned.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_checked(
    door: u32,
    param: u32,
    format: *const RawNumFormat,
    constraints: *const RawConstraints,
    enum_ptr: *const u8,
    enum_len: usize,
    ptr: *const u8,
    len: usize,
    out: *mut c_void,
    fault: *mut RawFault,
    check: *mut RawCheck,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe {
        let Some(raw) = resolve_constraints(constraints) else {
            return CONTRACT_VIOLATION;
        };
        let raw = &raw;
        let Some(door) = door_1_0(door, param) else {
            return CONTRACT_VIOLATION;
        };
        let entries = Entries::packed(door, text(enum_ptr, enum_len));
        let input = text(ptr, len);
        let verdict = if door == Door::Text {
            judge_text_detailed(input, raw, &entries)
                .inspect(|judged| write_checked_span(judged, out))
        } else {
            let mut scratch = MaybeUninit::uninit();
            let Some(resolved) = resolve_format(format, &mut scratch) else {
                return CONTRACT_VIOLATION;
            };
            judge_numeric_detailed(door, input, resolved, raw, &entries, out.cast::<u8>())
        };
        finish_checked(verdict, plain_fault(fault), check)
    }
}

/// Judges the number `value`, which a source already decoded (a JSON parser's double, a
/// spreadsheet's numeric cell), at the numeric door `door` by `constraints` and the integer
/// `enum` list at `enum_ptr`/`enum_len`, on its sign and shortest round-trip decimal: the
/// same judgement `cast_checked` makes of that text. `-0.0` is zero, written at a float door
/// as `0.0`. NaN is malformed; an infinity is out of range at a float or decimal door and
/// malformed at an integer door, as the typed doors judge it. With no text to index, every
/// fault and violation has an empty span. The codes and what each writes are
/// `cast_checked`'s, the delivered float's judgement included (at the `F32` door, the
/// double's digits are judged, then the narrowed float), and a door that is not numeric is
/// a contract violation.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_checked_from_f64(
    door: u32,
    value: f64,
    constraints: *const RawConstraints,
    enum_ptr: *const u8,
    enum_len: usize,
    out: *mut c_void,
    fault: *mut RawFault,
    check: *mut RawCheck,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe {
        let Some(raw) = resolve_constraints(constraints) else {
            return CONTRACT_VIOLATION;
        };
        // No numeric door declares a parameter.
        let Some(door) = door_1_0(door, 0) else {
            return CONTRACT_VIOLATION;
        };
        let entries = Entries::packed(door, text(enum_ptr, enum_len));
        let verdict =
            judge_numeric_from_f64_detailed(door, value, &raw, &entries, out.cast::<u8>());
        finish_checked(verdict, plain_fault(fault), check)
    }
}

/// Judges a declaration, `constraints` and the `enum` list at `enum_ptr`/`enum_len` (read
/// only under `HAS_ENUM`; numeric entries native-endian `Decimal`s, 16 bytes each; text
/// entries a little-endian `u32` length followed by UTF-8 bytes), against door `door` with
/// `param`, once, when a binding builds it. Returns 0 when every rule holds, otherwise the
/// first rule it breaks, in code order. Every valid OpenAPI 3.2 declaration is accepted,
/// unsatisfiable ones included (crossed bounds, duplicate or empty `enum`s), and a keyword
/// for another type is ignored, as JSON Schema ignores it. The codes: 1 a door that takes no
/// constraint (or a door code that names no door), `TRIM` or `CASE_SENSITIVE` at a numeric
/// door, or a rounding with `multipleOf` at the text door; 5 multipleOf not positive; 6 a
/// rounding without multipleOf; 8 a bound, step or numeric entry at a scale past 28, a text
/// entry past `u32::MAX` bytes, or a rounding at an integer door with a fractional
/// multipleOf; 9 an enum entry that is not UTF-8; 11 a malformed layout. Codes 2, 3, 4, 7,
/// 10, 12 and 13 are retired and never returned. A malformed layout is any of: a null
/// `constraints`; a `size` other than `HyperCastRawConstraints_SIZE` or a sign byte other
/// than 0 or 1 in any of its five decimals (these two judged before everything else, since
/// the layout cannot be read past them); an unknown flag bit, rounding code or nonzero
/// `_reserved`; a sign byte other than 0 or 1 in a numeric entry; a packed list cut short (a
/// text entry's length word or bytes, or a 16-byte numeric entry); or a non-empty list
/// without `HAS_ENUM`. Never -1.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_constraints_check(
    door: u32,
    param: u32,
    constraints: *const RawConstraints,
    enum_ptr: *const u8,
    enum_len: usize,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    let Some(raw) = (unsafe { resolve_constraints(constraints) }) else {
        return Refusal::MalformedLayout.code() as i32;
    };
    let Some(door) = door_1_0(door, param) else {
        return Refusal::KeywordNotForDoor.code() as i32;
    };
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    match check_constraints(door, &raw, unsafe { text(enum_ptr, enum_len) }) {
        Ok(()) => 0,
        Err(refusal) => refusal.code() as i32,
    }
}

/// Casts the text at `ptr`/`len` into `out` as the span of the whole input, once it is
/// valid UTF-8: the text door, untrimmed. Invalid UTF-8 is malformed, spanning the first
/// invalid sequence; empty input is empty; input past `UINT32_MAX` bytes, which a span
/// cannot carry, is out of range.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_text(
    ptr: *const u8,
    len: usize,
    out: *mut RawSpan,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(text_span(text(ptr, len)), out, fault) }
}

/// The text door's verdict as a span of the whole input, which must end within `u32`.
fn text_span(input: &[u8]) -> Result<RawSpan, Fault> {
    let valid = crate::text::cast_text(input)?;
    let (offset, len) = exact_span(0, valid.len())?;
    Ok(RawSpan { offset, len })
}

/// Converts a bound, `multipleOf` or integer `enum` entry written in invariant notation (an
/// optional sign, digits, an optional `.` and digits, an optional exponent; ASCII only, no
/// white space of any kind) at `ptr`/`len` into a `Decimal` at `out`, exactly: trailing zeros
/// dropped, zero unsigned. Returns 0 with `out` written, or 8 (a value no door can hold: a magnitude
/// past 96 bits or a scale past 28, or text not in invariant notation, invalid UTF-8
/// included), leaving `out` untouched; 8 is `cast_constraints_check`'s code for the same
/// refusal. -1 is a contract violation: a null `out`, or a null `ptr` with a non-zero `len`.
/// Every binding converts a declaration's strings through this one export, so none restates
/// the conversion.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_declared_decimal(ptr: *const u8, len: usize, out: *mut Decimal) -> i32 {
    if out.is_null() || (ptr.is_null() && len != 0) {
        return CONTRACT_VIOLATION;
    }
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    match declared_decimal(unsafe { text(ptr, len) }) {
        Ok(value) => {
            // SAFETY: `out` is not null, and the caller guarantees it points to a live Decimal.
            unsafe { out.write(value) };
            0
        }
        Err(refusal) => refusal.code() as i32,
    }
}

// The detailed exports. Each validates every argument it reads before it writes anything:
// the fault's capacity, the door and its parameter, then the declarations the door reads.

/// Whether a detailed export may write `fault`: it is null, or its size word says it holds at
/// least a [`RawFaultDetail`], in whole `u32` words.
///
/// # Safety
/// Caller guarantees `fault` is null or points to a live, 4-byte aligned `u32` size word.
#[inline(always)]
unsafe fn fault_fits(fault: *mut RawFaultDetail) -> bool {
    if fault.is_null() {
        return true;
    }
    // SAFETY: a non-null fault starts with its size word, per the function contract.
    let size = unsafe { fault.cast::<u32>().read() };
    size >= RawFaultDetail::SIZE && size % 4 == 0
}

/// Writes bytes 4 to 16 of a detailed fault, when it is not null; never `size`.
///
/// # Safety
/// Caller guarantees `fault` is null or points to a live [`RawFaultDetail`] that
/// [`fault_fits`] accepted.
#[inline(always)]
unsafe fn detailed_fault(fault: *mut RawFaultDetail) -> impl FnOnce(u32, u32, u32) {
    move |offset, len, detail| {
        if !fault.is_null() {
            // SAFETY: caller guarantees a non-null `fault` points to a live RawFaultDetail.
            unsafe {
                (&raw mut (*fault).offset).write(offset);
                (&raw mut (*fault).len).write(len);
                (&raw mut (*fault).detail).write(detail);
            }
        }
    }
}

/// The door a detailed export names: `code` with `param`, which must be `0` at a door that
/// declares nothing.
#[inline(always)]
fn detailed_door(code: u32, param: u32) -> Option<Door> {
    let door = Door::from_code(code, param)?;
    (door.declares() || param == 0).then_some(door)
}

/// A detailed door's verdict as its return code: the value written to `out` (unaligned), or
/// the span and detail handed to `fault`.
///
/// # Safety
/// Caller guarantees `out` is valid for an unaligned write of a `T`, and `fault` is what
/// [`detailed_fault`] takes.
#[inline(always)]
unsafe fn deliver<T>(
    verdict: Result<T, Failure>,
    out: *mut c_void,
    fault: *mut RawFaultDetail,
) -> i32 {
    match verdict {
        Ok(value) => {
            // SAFETY: caller guarantees `out` holds a T, at any alignment.
            unsafe { out.cast::<T>().write_unaligned(value) };
            0
        }
        Err(failed) => {
            // SAFETY: per the function contract.
            unsafe { detailed_fault(fault)(failed.offset, failed.len, failed.detail.code()) };
            failed.reason as i32
        }
    }
}

/// The format a detailed export parses with, judged by 1.1's rules ([`RawNumFormat::resolve`]
/// unmasked: the retired bit 8, so 351 and 863, and an undefined `flags` bit are refused). Null is the
/// invariant format.
///
/// # Safety
/// As [`resolve_format`].
#[inline(always)]
unsafe fn valid_format(
    format: *const RawNumFormat,
    scratch: &mut MaybeUninit<RawNumFormat>,
) -> Option<&ValidNumFormat> {
    if format.is_null() {
        return Some(&ValidNumFormat::INVARIANT);
    }
    // SAFETY: per the function contract.
    let raw = unsafe { read_sized(format, RawNumFormat::ZERO, RawNumFormat::MIN_SIZE, scratch) }?;
    raw.resolve().map(ValidNumFormat::from_checked)
}

/// The date options a detailed date door reads: the layout at `options` (null for none) and,
/// where the door reads month names (`months`), the packed vocabulary at `vocab_ptr`/
/// `vocab_len` (a length of 0 for none, whatever the pointer). `Some(None)` is nothing
/// declared; `None` a contract violation: a size other than 8 or 12, a pivot past 9999 (and,
/// by [`DateOptions::validate`], one under 100 but 0, or an undefined flag bit), a null
/// pointer with a nonzero length, or a vocabulary its rules refuse.
///
/// # Safety
/// Caller guarantees `options` is null or points to a live, 4-byte aligned layout of the size
/// its first word says, and, when `months` is set, `vocab_ptr` points to `vocab_len` live
/// bytes when it is not null and `vocab_len` is not 0.
#[inline(always)]
unsafe fn date_options<'caller>(
    options: *const RawDateOptions,
    vocab_ptr: *const u8,
    vocab_len: usize,
    months: bool,
) -> Option<Option<ValidDateOptions<'caller>>> {
    let declared = if options.is_null() {
        None
    } else {
        let words = options.cast::<u32>();
        // SAFETY: a non-null layout starts with its size word, per the function contract.
        let size = unsafe { words.read() };
        if size != RawDateOptions::MIN_SIZE && size != RawDateOptions::SIZE {
            return None;
        }
        // SAFETY: the caller built `size` bytes: the flags word, and the pivot at size 12.
        let (flags, pivot) = unsafe {
            (
                words.add(1).read(),
                if size == RawDateOptions::SIZE { words.add(2).read() } else { 0 },
            )
        };
        // Refused as itself, never narrowed: `validate` takes a `u16`.
        if pivot > 9999 {
            return None;
        }
        Some(DateOptions::from_flags(flags).with_pivot(pivot as u16))
    };
    let names = if months && vocab_len != 0 {
        if vocab_ptr.is_null() {
            return None;
        }
        // SAFETY: per the function contract.
        let bytes = unsafe { slice::from_raw_parts(vocab_ptr, vocab_len) };
        Some(MonthNames::from_packed(bytes).ok()?)
    } else {
        None
    };
    if declared.is_none() && names.is_none() {
        return Some(None);
    }
    declared.unwrap_or(DateOptions::NONE).validate(names).ok().map(Some)
}

/// The text door's detailed verdict as a span of the whole input.
#[inline(always)]
fn text_span_detailed(input: &[u8]) -> Result<RawSpan, Failure> {
    let valid = detailed::cast_text(input)?;
    let (offset, len) = exact_span(0, valid.len())?;
    Ok(RawSpan { offset, len })
}

/// Casts the text at `ptr`/`len` through door `door`, reporting a failure's detail. `door` is
/// `Door`'s code (1 bool, 2 to 9 the integers `int8_t` … `uint64_t`, 10 float, 11 double, 12
/// uuid, 13 timestamp, 14 unix, 15 date, 16 time, 17 duration, 18 text, 19 decimal, 20
/// date_ordered, 21 datetime, 22 excel_serial, 23 char) and `param` what the door declares,
/// numbered as its own export numbers it (14's precision, 20's and 21's order, 22's epoch),
/// and `0` at every other door. `out` receives exactly what that door's own export writes
/// (`cast_text`'s `RawSpan` at 18), and need not be aligned.
///
/// Each declaration is read only by the doors it is for, and a pointer a door ignores is
/// never read: `format` by the numeric doors 2 to 11 and 19 (null for the invariant format,
/// Western grouping), judged by 1.1's rules, so bit 8 and any other `flags` bit 1.1 doesn't
/// define are refused; `periods` by 21 (null for English); `date_options` by 13, 15,
/// 20 and 21 (null for none); and the month vocabulary at `vocab_ptr`/`vocab_len` by 20 and 21
/// only, packed as each entry's month byte (1 to 12), a little-endian `uint32_t` byte length,
/// then that many UTF-8 bytes, a length of 0 meaning none whatever the pointer. The
/// vocabulary is independent of `date_options`: with null options it opens month names and
/// nothing else. It is judged in full on every call.
///
/// Returns 0 with the value written to `out`; 1, 2 or 3, the door's own fault, with `offset`,
/// `len` and `detail` written to a non-null `fault`. -1 is a contract violation and writes
/// nothing: a `fault` whose `size` is under 16 or not a multiple of 4, an unknown door, a
/// `param` the door doesn't take, or a declaration the door reads and the core refuses
/// (a `date_options` size other than 8 or 12, an undefined option bit, a pivot other than 0
/// or 100 to 9999, a null `vocab_ptr` with a nonzero `vocab_len`, a vocabulary its rules
/// refuse).
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_detailed(
    door: u32,
    param: u32,
    format: *const RawNumFormat,
    periods: *const RawDayPeriods,
    date_options: *const RawDateOptions,
    vocab_ptr: *const u8,
    vocab_len: usize,
    ptr: *const u8,
    len: usize,
    out: *mut c_void,
    fault: *mut RawFaultDetail,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc and this export's.
    unsafe {
        if !fault_fits(fault) {
            return CONTRACT_VIOLATION;
        }
        let Some(door) = detailed_door(door, param) else {
            return CONTRACT_VIOLATION;
        };
        let input = text(ptr, len);
        let mut scratch = MaybeUninit::uninit();
        macro_rules! numeric {
            ($door:ident) => {{
                let Some(format) = valid_format(format, &mut scratch) else {
                    return CONTRACT_VIOLATION;
                };
                deliver(detailed::$door(input, format), out, fault)
            }};
        }
        macro_rules! dated {
            ($months:expr) => {
                match self::date_options(date_options, vocab_ptr, vocab_len, $months) {
                    Some(options) => options,
                    None => return CONTRACT_VIOLATION,
                }
            };
        }
        match door {
            Door::Bool => deliver(detailed::cast_bool(input).map(u8::from), out, fault),
            Door::Char => deliver(detailed::cast_char(input).map(u32::from), out, fault),
            Door::I8 => numeric!(cast_i8),
            Door::I16 => numeric!(cast_i16),
            Door::I32 => numeric!(cast_i32),
            Door::I64 => numeric!(cast_i64),
            Door::U8 => numeric!(cast_u8),
            Door::U16 => numeric!(cast_u16),
            Door::U32 => numeric!(cast_u32),
            Door::U64 => numeric!(cast_u64),
            Door::F32 => numeric!(cast_f32),
            Door::F64 => numeric!(cast_f64),
            Door::Decimal => numeric!(cast_decimal),
            Door::Uuid => deliver(detailed::cast_uuid(input), out, fault),
            Door::Timestamp => {
                let options = dated!(false);
                deliver(detailed::cast_timestamp(input, options.as_ref()), out, fault)
            }
            Door::Unix(precision) => deliver(detailed::cast_unix(input, precision), out, fault),
            Door::Date => {
                let options = dated!(false);
                deliver(detailed::cast_date(input, options.as_ref()), out, fault)
            }
            Door::Time => deliver(detailed::cast_time(input), out, fault),
            Door::Duration => deliver(detailed::cast_duration(input), out, fault),
            Door::Text => deliver(text_span_detailed(input), out, fault),
            Door::DateOrdered(order) => {
                let options = dated!(true);
                deliver(detailed::cast_date_ordered(input, order, options.as_ref()), out, fault)
            }
            Door::DateTime(order) => {
                let mut periods_scratch = MaybeUninit::uninit();
                let Some(periods) = resolve_periods(periods, &mut periods_scratch) else {
                    return CONTRACT_VIOLATION;
                };
                let options = dated!(true);
                let verdict = detailed::cast_datetime_with(input, order, periods, options.as_ref());
                deliver(verdict, out, fault)
            }
            Door::ExcelSerial(epoch) => {
                deliver(detailed::cast_excel_serial(input, epoch), out, fault)
            }
        }
    }
}

/// Reads the double `value` through typed door `door`, reporting a failure's detail: the
/// doors with a typed export, 19 (`cast_decimal_from_f64`, a `Decimal`), 22 with `param` the
/// epoch (`cast_excel_serial_from_f64`, a `CivilDateTime`), 16 (`cast_excel_time`, a
/// `uint64_t`) and 17 (`cast_excel_duration`, a `Duration`), each as that export reads it.
/// `param` is `0` but at 22. `out` need not be aligned. A failure's span is empty, with no
/// text to index. Returns as `cast_detailed` does; -1 for any other door, a `param` the door
/// doesn't take, or a `fault` too small.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_detailed_from_f64(
    door: u32,
    param: u32,
    value: f64,
    out: *mut c_void,
    fault: *mut RawFaultDetail,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc and this export's.
    unsafe {
        if !fault_fits(fault) {
            return CONTRACT_VIOLATION;
        }
        match detailed_door(door, param) {
            Some(Door::Decimal) => deliver(detailed::decimal_from_f64(value), out, fault),
            Some(Door::ExcelSerial(epoch)) => {
                deliver(detailed::excel_serial(value, epoch), out, fault)
            }
            Some(Door::Time) => deliver(detailed::excel_time(value), out, fault),
            Some(Door::Duration) => deliver(detailed::excel_duration(value), out, fault),
            _ => CONTRACT_VIOLATION,
        }
    }
}

/// `cast_checked`, reporting a failure's detail: the same doors (the numeric ones and 18,
/// text), declaration, `enum` list and `out`, with `param` `0` (no checked door declares one)
/// and `format` judged by 1.1's rules (null for the invariant format; never read at 18).
/// Returns 0 with the value written to `out` and `check` written (`keyword` 0, `adjusted`,
/// `enum_index`); 1, 2 or 3, the door's own fault, with `offset`, `len` and `detail` written
/// to `fault`; or 4, a violation of a declared rule, with `fault` spanning the value and
/// `detail` 0, and `check` holding the `keyword`, `adjusted` 0 and `enum_index`
/// `UINT32_MAX`. `fault` and `check` may be null. -1 writes nothing: what `cast_checked`
/// refuses, a `fault` whose `size` is under 16 or not a multiple of 4, or a nonzero `param`.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_checked_detailed(
    door: u32,
    param: u32,
    format: *const RawNumFormat,
    constraints: *const RawConstraints,
    enum_ptr: *const u8,
    enum_len: usize,
    ptr: *const u8,
    len: usize,
    out: *mut c_void,
    fault: *mut RawFaultDetail,
    check: *mut RawCheck,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc and this export's.
    unsafe {
        if !fault_fits(fault) {
            return CONTRACT_VIOLATION;
        }
        let Some(raw) = resolve_constraints(constraints) else {
            return CONTRACT_VIOLATION;
        };
        let raw = &raw;
        let Some(door) = detailed_door(door, param) else {
            return CONTRACT_VIOLATION;
        };
        let entries = Entries::packed(door, text(enum_ptr, enum_len));
        let input = text(ptr, len);
        let verdict = if door == Door::Text {
            judge_text_detailed(input, raw, &entries)
                .inspect(|judged| write_checked_span(judged, out))
        } else if door.is_numeric() {
            let mut scratch = MaybeUninit::uninit();
            let Some(format) = valid_format(format, &mut scratch) else {
                return CONTRACT_VIOLATION;
            };
            judge_numeric_detailed(door, input, format.format(), raw, &entries, out.cast::<u8>())
                .map(|verdict| verdict.map_err(|rejection| detailed::classified(rejection, input)))
        } else {
            None
        };
        finish_checked(verdict, detailed_fault(fault), check)
    }
}

/// `cast_checked_from_f64`, reporting a failure's detail: the numeric doors, `param` `0`, and
/// the codes and writes of `cast_checked_detailed`, every span empty. -1 writes nothing: what
/// `cast_checked_from_f64` refuses, a `fault` too small, or a nonzero `param`.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub extern "C" fn cast_checked_detailed_from_f64(
    door: u32,
    param: u32,
    constraints: *const RawConstraints,
    enum_ptr: *const u8,
    enum_len: usize,
    value: f64,
    out: *mut c_void,
    fault: *mut RawFaultDetail,
    check: *mut RawCheck,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc and this export's.
    unsafe {
        if !fault_fits(fault) {
            return CONTRACT_VIOLATION;
        }
        let Some(raw) = resolve_constraints(constraints) else {
            return CONTRACT_VIOLATION;
        };
        let Some(door) = detailed_door(door, param) else {
            return CONTRACT_VIOLATION;
        };
        let entries = Entries::packed(door, text(enum_ptr, enum_len));
        let verdict =
            judge_numeric_from_f64_detailed(door, value, &raw, &entries, out.cast::<u8>());
        finish_checked(verdict, detailed_fault(fault), check)
    }
}
