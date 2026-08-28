//! C-ABI exports — this crate's `crate-type` is `cdylib`, so this same source produces a
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

use crate::verdict::{Date, Duration, Fault, NumFormat, Timestamp};
use crate::{boolean, integer, real, temporal, uuid, UnixPrecision};
use core::slice;

const CONTRACT_VIOLATION: i32 = -1;

/// The fault span as it crosses the ABI — the reason travels as the return code.
#[repr(C)]
pub struct RawFault {
    pub offset: u32,
    pub len: u32,
}

/// [`NumFormat`] as it crosses the ABI: separators as Unicode code points. A null pointer
/// means [`NumFormat::INVARIANT`]; a code point that is no `char`, or equal separators,
/// is a contract violation.
#[repr(C)]
pub struct RawNumFormat {
    pub decimal_sep: u32,
    pub group_sep: u32,
    pub flags: u32,
}

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

/// # Safety
/// Caller guarantees `format` is null or points to a live [`RawNumFormat`].
unsafe fn resolve_format(format: *const RawNumFormat) -> Option<NumFormat> {
    if format.is_null() {
        return Some(NumFormat::INVARIANT);
    }
    // SAFETY: per the function contract.
    let raw = unsafe { &*format };
    let decimal_sep = char::from_u32(raw.decimal_sep)?;
    let group_sep = char::from_u32(raw.group_sep)?;
    if decimal_sep == group_sep {
        return None;
    }
    Some(NumFormat { decimal_sep, group_sep, flags: raw.flags })
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

/// Casts boolean text at `ptr`/`len` into `out` (0 or 1). See [`boolean::cast_bool`].
#[unsafe(no_mangle)]
pub extern "C" fn cast_bool(ptr: *const u8, len: usize, out: *mut u8, fault: *mut RawFault) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(boolean::cast_bool(text(ptr, len)).map(u8::from), out, fault) }
}

macro_rules! numeric_exports {
    ($($export:ident => ($module:ident, $ty:ty)),+ $(,)?) => {$(
        /// Casts numeric text at `ptr`/`len` under the declared `format` (null ⇒ invariant)
        /// into `out`. See the same-named door in the core module.
        #[unsafe(no_mangle)]
        pub extern "C" fn $export(
            ptr: *const u8,
            len: usize,
            format: *const RawNumFormat,
            out: *mut $ty,
            fault: *mut RawFault,
        ) -> i32 {
            // SAFETY: caller guarantees the pointer contracts, per the module doc.
            unsafe {
                let Some(resolved) = resolve_format(format) else {
                    return CONTRACT_VIOLATION;
                };
                finish($module::$export(text(ptr, len), &resolved), out, fault)
            }
        }
    )+};
}

numeric_exports! {
    cast_i8 => (integer, i8),
    cast_i16 => (integer, i16),
    cast_i32 => (integer, i32),
    cast_i64 => (integer, i64),
    cast_u8 => (integer, u8),
    cast_u16 => (integer, u16),
    cast_u32 => (integer, u32),
    cast_u64 => (integer, u64),
    cast_f32 => (real, f32),
    cast_f64 => (real, f64),
}

/// Casts UUID text at `ptr`/`len` into the 16 bytes at `out`, RFC 9562 order.
/// See [`uuid::cast_uuid`].
#[unsafe(no_mangle)]
pub extern "C" fn cast_uuid(ptr: *const u8, len: usize, out: *mut u8, fault: *mut RawFault) -> i32 {
    // SAFETY: caller guarantees the pointer contracts (`out` is 16 live bytes).
    unsafe { finish(uuid::cast_uuid(text(ptr, len)), out.cast::<[u8; 16]>(), fault) }
}

/// Casts an RFC 3339 instant at `ptr`/`len` into `out`. See [`temporal::cast_timestamp`].
#[unsafe(no_mangle)]
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
/// violation) into `out`. See [`temporal::cast_unix`].
#[unsafe(no_mangle)]
pub extern "C" fn cast_unix(
    ptr: *const u8,
    len: usize,
    precision: u32,
    out: *mut Timestamp,
    fault: *mut RawFault,
) -> i32 {
    let precision = match precision {
        1 => UnixPrecision::Seconds,
        2 => UnixPrecision::Millis,
        3 => UnixPrecision::Micros,
        4 => UnixPrecision::Nanos,
        _ => return CONTRACT_VIOLATION,
    };
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(temporal::cast_unix(text(ptr, len), precision), out, fault) }
}

/// Casts a strict `yyyy-MM-dd` date at `ptr`/`len` into `out`. See [`temporal::cast_date`].
#[unsafe(no_mangle)]
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
/// midnight. See [`temporal::cast_time`].
#[unsafe(no_mangle)]
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
/// seconds) into `out`. See [`temporal::cast_duration`].
#[unsafe(no_mangle)]
pub extern "C" fn cast_duration(
    ptr: *const u8,
    len: usize,
    out: *mut Duration,
    fault: *mut RawFault,
) -> i32 {
    // SAFETY: caller guarantees the pointer contracts, per the module doc.
    unsafe { finish(temporal::cast_duration(text(ptr, len)), out, fault) }
}
