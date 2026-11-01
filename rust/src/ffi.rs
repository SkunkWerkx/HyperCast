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

use crate::abi::{self, RawDayPeriods, RawNumFormat};
use crate::temporal::DayPeriods;
use crate::verdict::{CivilDateTime, Date, Decimal, Duration, Fault, NumFormat, Timestamp};
use crate::{
    DateOrder, ExcelEpoch, Reason, UnixPrecision, boolean, character, decimal, integer, real,
    temporal, typed, uuid,
};
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
    // SAFETY: per the function contract.
    unsafe { read_sized(format, RawNumFormat::ZERO, RawNumFormat::MIN_SIZE, scratch) }?.resolve()
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
