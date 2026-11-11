//! The JNI library the Java binding loads below JDK 25. It sits beside the core in one
//! process: the core's C exports are linked in (so they remain exported), and `JNI_OnLoad`
//! registers one table of natives on `NativeCore`, one per C export. No `alloc`, no `std`,
//! and no panic path: every native returns the core's own verdict code, or `PENDING` when a
//! Java exception is already on its way.
//!
//! A verdict crosses back through the caller's `long[]`: on success the value as two
//! `long`s in `out[0..2]` ([`Words`] has each type's encoding), on a fault its span's offset
//! and length in `out[2..4]`, and on a contract violation (`-1`) nothing at all — the same
//! writes the C ABI makes to its `out` and `fault` pointers. The checked doors write the value
//! as cast to `out[0..2]` and the span to `out[2..4]` on a violation, as the C ABI writes
//! both `out` and `fault` there, and the C ABI's `check` to `out[4..7]` on every verdict
//! (see [`check_words`]); the four detailed natives write a fault's `detail` code to `out[7]` (on a violation, `0`), so
//! Java hands every door a `long[8]`; the 1.0 natives read and write `out[0..4]` alone.
//!
//! The natives are `pub unsafe extern "system" fn`: as `pub` fns taking raw pointers they
//! would otherwise fail clippy's `not_unsafe_ptr_arg_deref`. Their safety contract is
//! the JNI one (a live `JNIEnv`, and arrays of the declared type), hence the crate-wide allow.
#![cfg_attr(not(test), no_std)]
#![allow(clippy::missing_safety_doc)]

extern crate hypercast;

use core::ffi::{c_char, c_void};
use core::mem::MaybeUninit;
use core::ptr::{null, null_mut};
use hypercast::{
    CivilDateTime, Date, DateOrder, Decimal, Door, Duration, ExcelEpoch, RawCheck, RawConstraints,
    RawDateOptions, RawDayPeriods, RawFaultDetail, RawNumFormat, RawSpan, Timestamp, UnixPrecision,
};
use jni_sys::{
    JNI_ABORT, JNI_ERR, JNI_OK, JNI_VERSION_1_8, JNIEnv, JNINativeMethod, JavaVM, jbyte,
    jbyteArray, jclass, jdouble, jint, jlong, jlongArray, jobject, jsize,
};

/// A fault's span as the C ABI writes it: two `u32`s, the reason being the return code.
#[repr(C)]
pub struct RawFault {
    offset: u32,
    len: u32,
}

// The core's C exports, reached by symbol the way every binding reaches them, plus the two
// C runtime calls the large-input path needs (there is no `libc` dependency).
unsafe extern "C" {
    fn cast_bool(ptr: *const u8, len: usize, out: *mut u8, fault: *mut RawFault) -> i32;
    fn cast_char(ptr: *const u8, len: usize, out: *mut u32, fault: *mut RawFault) -> i32;
    fn cast_i8(
        ptr: *const u8,
        len: usize,
        format: *const RawNumFormat,
        out: *mut i8,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_i16(
        ptr: *const u8,
        len: usize,
        format: *const RawNumFormat,
        out: *mut i16,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_i32(
        ptr: *const u8,
        len: usize,
        format: *const RawNumFormat,
        out: *mut i32,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_i64(
        ptr: *const u8,
        len: usize,
        format: *const RawNumFormat,
        out: *mut i64,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_u8(
        ptr: *const u8,
        len: usize,
        format: *const RawNumFormat,
        out: *mut u8,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_u16(
        ptr: *const u8,
        len: usize,
        format: *const RawNumFormat,
        out: *mut u16,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_u32(
        ptr: *const u8,
        len: usize,
        format: *const RawNumFormat,
        out: *mut u32,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_u64(
        ptr: *const u8,
        len: usize,
        format: *const RawNumFormat,
        out: *mut u64,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_f32(
        ptr: *const u8,
        len: usize,
        format: *const RawNumFormat,
        out: *mut f32,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_f64(
        ptr: *const u8,
        len: usize,
        format: *const RawNumFormat,
        out: *mut f64,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_decimal(
        ptr: *const u8,
        len: usize,
        format: *const RawNumFormat,
        out: *mut Decimal,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_uuid(ptr: *const u8, len: usize, out: *mut [u8; 16], fault: *mut RawFault) -> i32;
    fn cast_timestamp(ptr: *const u8, len: usize, out: *mut Timestamp, fault: *mut RawFault)
    -> i32;
    fn cast_unix(
        ptr: *const u8,
        len: usize,
        precision: u32,
        out: *mut Timestamp,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_excel_serial(
        ptr: *const u8,
        len: usize,
        epoch: u32,
        out: *mut Timestamp,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_date_ordered(
        ptr: *const u8,
        len: usize,
        order: u32,
        out: *mut Date,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_datetime(
        ptr: *const u8,
        len: usize,
        order: u32,
        periods: *const RawDayPeriods,
        out: *mut CivilDateTime,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_date(ptr: *const u8, len: usize, out: *mut Date, fault: *mut RawFault) -> i32;
    fn cast_time(ptr: *const u8, len: usize, out: *mut u64, fault: *mut RawFault) -> i32;
    fn cast_duration(ptr: *const u8, len: usize, out: *mut Duration, fault: *mut RawFault) -> i32;
    fn cast_decimal_from_f64(value: f64, out: *mut Decimal, fault: *mut RawFault) -> i32;
    fn cast_excel_serial_from_f64(
        value: f64,
        epoch: u32,
        out: *mut CivilDateTime,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_excel_time(value: f64, out: *mut u64, fault: *mut RawFault) -> i32;
    fn cast_excel_duration(value: f64, out: *mut Duration, fault: *mut RawFault) -> i32;
    fn hypercast_version() -> u32;
    fn cast_checked(
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
    ) -> i32;
    fn cast_checked_from_f64(
        door: u32,
        value: f64,
        constraints: *const RawConstraints,
        enum_ptr: *const u8,
        enum_len: usize,
        out: *mut c_void,
        fault: *mut RawFault,
        check: *mut RawCheck,
    ) -> i32;
    fn cast_constraints_check(
        door: u32,
        param: u32,
        constraints: *const RawConstraints,
        enum_ptr: *const u8,
        enum_len: usize,
    ) -> i32;
    fn cast_text(ptr: *const u8, len: usize, out: *mut RawSpan, fault: *mut RawFault) -> i32;
    fn cast_declared_decimal(ptr: *const u8, len: usize, out: *mut Decimal) -> i32;
    fn cast_detailed(
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
    ) -> i32;
    fn cast_detailed_from_f64(
        door: u32,
        param: u32,
        value: f64,
        out: *mut c_void,
        fault: *mut RawFaultDetail,
    ) -> i32;
    fn cast_checked_detailed(
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
    ) -> i32;
    fn cast_checked_detailed_from_f64(
        door: u32,
        param: u32,
        constraints: *const RawConstraints,
        enum_ptr: *const u8,
        enum_len: usize,
        value: f64,
        out: *mut c_void,
        fault: *mut RawFaultDetail,
        check: *mut RawCheck,
    ) -> i32;
    fn malloc(size: usize) -> *mut c_void;
    fn free(p: *mut c_void);
}

/// The largest input a critical array access may cover (bytes). Above it, the input is
/// copied into `malloc`'d memory instead, so a parse over untrusted megabytes never holds
/// off the garbage collector.
pub const CRITICAL_MAX: usize = 64 * 1024;

/// What a native returns when a Java exception is pending (a failed array access). Java
/// never reads it: the pending exception is thrown as the native returns. Distinct from
/// every core code, `-1` (a contract violation) included.
pub const PENDING: jint = jint::MIN;

const CLASS: &[u8] = b"io/github/skunkwerkx/hypercast/NativeCore\0";

/// One registered native: NUL-terminated name and descriptor, and the function.
pub struct Native {
    name: &'static [u8],
    descriptor: &'static [u8],
    function: *const c_void,
}

macro_rules! native {
    ($name:literal, $descriptor:literal, $function:path) => {
        Native {
            name: concat!($name, "\0").as_bytes(),
            descriptor: concat!($descriptor, "\0").as_bytes(),
            function: $function as *const c_void,
        }
    };
}

const NATIVES: [Native; 37] = [
    native!("castBool", "([B[J)I", native_cast_bool),
    native!("castChar", "([B[J)I", native_cast_char),
    native!("castI8", "([B[B[J)I", native_cast_i8),
    native!("castI16", "([B[B[J)I", native_cast_i16),
    native!("castI32", "([B[B[J)I", native_cast_i32),
    native!("castI64", "([B[B[J)I", native_cast_i64),
    native!("castU8", "([B[B[J)I", native_cast_u8),
    native!("castU16", "([B[B[J)I", native_cast_u16),
    native!("castU32", "([B[B[J)I", native_cast_u32),
    native!("castU64", "([B[B[J)I", native_cast_u64),
    native!("castF32", "([B[B[J)I", native_cast_f32),
    native!("castF64", "([B[B[J)I", native_cast_f64),
    native!("castDecimal", "([B[B[J)I", native_cast_decimal),
    native!("castUuid", "([B[J)I", native_cast_uuid),
    native!("castTimestamp", "([B[J)I", native_cast_timestamp),
    native!("castUnix", "([BI[J)I", native_cast_unix),
    native!("castExcelSerial", "([BI[J)I", native_cast_excel_serial),
    native!("castDateOrdered", "([BI[J)I", native_cast_date_ordered),
    native!("castDatetime", "([BI[B[J)I", native_cast_datetime),
    native!("castDate", "([B[J)I", native_cast_date),
    native!("castTime", "([B[J)I", native_cast_time),
    native!("castDuration", "([B[J)I", native_cast_duration),
    native!("castDecimalFromF64", "(D[J)I", native_cast_decimal_from_f64),
    native!(
        "castExcelSerialFromF64",
        "(DI[J)I",
        native_cast_excel_serial_from_f64
    ),
    native!("castExcelTime", "(D[J)I", native_cast_excel_time),
    native!("castExcelDuration", "(D[J)I", native_cast_excel_duration),
    native!("hypercastVersion", "()I", native_hypercast_version),
    native!(
        "castChecked",
        "(I[B[BLjava/nio/ByteBuffer;[B[J)I",
        native_cast_checked
    ),
    native!(
        "castCheckedFromF64",
        "(ID[BLjava/nio/ByteBuffer;[J)I",
        native_cast_checked_from_f64
    ),
    native!(
        "castConstraintsCheck",
        "(I[BLjava/nio/ByteBuffer;)I",
        native_cast_constraints_check
    ),
    native!("castText", "([B[J)I", native_cast_text),
    native!(
        "castDeclaredDecimal",
        "([B[J)I",
        native_cast_declared_decimal
    ),
    native!(
        "castDetailed",
        "(II[B[B[BLjava/nio/ByteBuffer;[B[J)I",
        native_cast_detailed
    ),
    native!(
        "castDetailedFromF64",
        "(IID[J)I",
        native_cast_detailed_from_f64
    ),
    native!(
        "castCheckedDetailed",
        "(I[B[BLjava/nio/ByteBuffer;[B[J)I",
        native_cast_checked_detailed
    ),
    native!(
        "castCheckedDetailedFromF64",
        "(ID[BLjava/nio/ByteBuffer;[J)I",
        native_cast_checked_detailed_from_f64
    ),
    native!("jniNatives", "()I", native_jni_natives),
];

/// True when no two entries share a name. A `const fn` so a repeat fails the build.
const fn names_unique(natives: &[Native]) -> bool {
    let mut i = 0;
    while i < natives.len() {
        let mut j = i + 1;
        while j < natives.len() {
            if bytes_eq(natives[i].name, natives[j].name) {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

const fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut k = 0;
    while k < a.len() {
        if a[k] != b[k] {
            return false;
        }
        k += 1;
    }
    true
}

const _: () = assert!(names_unique(&NATIVES));

/// The one allocation the library makes, for an input above [`CRITICAL_MAX`]. Production
/// calls `malloc` directly; tests swap it. The core itself never allocates.
#[cfg(not(test))]
unsafe fn alloc(n: usize) -> *mut u8 {
    // SAFETY: plain C `malloc`; the caller checks for null.
    unsafe { malloc(n) }.cast()
}
#[cfg(test)]
unsafe fn alloc(n: usize) -> *mut u8 {
    tests::alloc(n)
}

#[cfg(not(test))]
unsafe fn release(p: *mut u8) {
    // SAFETY: `p` came from `alloc` and is freed once.
    unsafe { free(p.cast()) }
}
#[cfg(test)]
unsafe fn release(p: *mut u8) {
    tests::release(p)
}

/// A null array access (a critical get or `malloc`) is never a core result. With an exception
/// already pending, return at once; otherwise throw `OutOfMemoryError`. Either way Java
/// throws, and never reads the [`PENDING`] returned.
unsafe fn fail_null(env: *mut JNIEnv) -> jint {
    // SAFETY: the caller passes a live env.
    unsafe {
        if ((**env).v1_2.ExceptionCheck)(env) {
            return PENDING;
        }
        let class = ((**env).v1_2.FindClass)(env, c"java/lang/OutOfMemoryError".as_ptr());
        if !class.is_null() {
            ((**env).v1_2.ThrowNew)(env, class, c"native memory unavailable".as_ptr());
        }
    }
    PENDING
}

/// A value as the two `long`s Java rebuilds it from (`NativeCore`'s readers in the base tree
/// are the other half). Integers are sign- or zero-extended as their type is, floats travel
/// as their bits, and the composite values field by field, so nothing depends on the
/// target's byte order.
pub trait Words: Copy {
    /// `out[0]` and `out[1]`.
    fn words(self) -> [jlong; 2];
}

macro_rules! extended {
    ($($ty:ty),+) => {$(
        impl Words for $ty {
            #[inline(always)]
            fn words(self) -> [jlong; 2] {
                [self as jlong, 0]
            }
        }
    )+};
}

extended!(u8, u16, u32, u64, i8, i16, i32, i64);

impl Words for f32 {
    #[inline(always)]
    fn words(self) -> [jlong; 2] {
        [self.to_bits() as jlong, 0]
    }
}

impl Words for f64 {
    #[inline(always)]
    fn words(self) -> [jlong; 2] {
        [self.to_bits() as jlong, 0]
    }
}

/// RFC 9562 order, which is exactly `java.util.UUID`'s two big-endian halves.
impl Words for [u8; 16] {
    #[inline(always)]
    fn words(self) -> [jlong; 2] {
        let whole = u128::from_be_bytes(self);
        [(whole >> 64) as jlong, whole as jlong]
    }
}

/// The 64 low bits, then the 32 high bits with the scale at bit 32 and the sign at bit 40.
impl Words for Decimal {
    #[inline(always)]
    fn words(self) -> [jlong; 2] {
        let high =
            u64::from(self.hi) | (u64::from(self.scale) << 32) | (u64::from(self.negative) << 40);
        [self.lo as jlong, high as jlong]
    }
}

impl Words for Timestamp {
    #[inline(always)]
    fn words(self) -> [jlong; 2] {
        [self.seconds, jlong::from(self.nanos)]
    }
}

impl Words for Duration {
    #[inline(always)]
    fn words(self) -> [jlong; 2] {
        [self.seconds, jlong::from(self.nanos)]
    }
}

/// `year << 16 | month << 8 | day`.
impl Words for Date {
    #[inline(always)]
    fn words(self) -> [jlong; 2] {
        let packed =
            (u32::from(self.year) << 16) | (u32::from(self.month) << 8) | u32::from(self.day);
        [jlong::from(packed), 0]
    }
}

/// The date packed as [`Date`]'s, then the nanoseconds of the day.
impl Words for CivilDateTime {
    #[inline(always)]
    fn words(self) -> [jlong; 2] {
        [self.date.words()[0], self.nanos_of_day as jlong]
    }
}

/// The text door's span of the input: its byte offset, then its byte length.
impl Words for RawSpan {
    #[inline(always)]
    fn words(self) -> [jlong; 2] {
        [jlong::from(self.offset), jlong::from(self.len)]
    }
}

/// Runs `door` over the bytes of `input`, a `byte[]`. Up to [`CRITICAL_MAX`] bytes the array
/// is pinned for the one core call and released before any other JNI call; above it the
/// bytes are copied into `malloc`'d memory, freed on every path. A zero length passes null
/// and pins nothing.
#[inline(always)]
unsafe fn with_text(
    env: *mut JNIEnv,
    input: jbyteArray,
    door: impl FnOnce(*const u8, usize) -> i32,
) -> jint {
    // SAFETY: the caller passes a live env and a `byte[]`.
    let len = unsafe { ((**env).v1_2.GetArrayLength)(env, input) };
    if len <= 0 {
        return door(null(), 0);
    }
    let n = len as usize;
    if n <= CRITICAL_MAX {
        // SAFETY: only the core call runs inside the critical region, which closes before
        // any other JNI call. JNI_ABORT: the core only reads.
        unsafe {
            let p = ((**env).v1_2.GetPrimitiveArrayCritical)(env, input, null_mut());
            if p.is_null() {
                return fail_null(env);
            }
            let rc = door(p.cast_const().cast(), n);
            ((**env).v1_2.ReleasePrimitiveArrayCritical)(env, input, p, JNI_ABORT);
            rc
        }
    } else {
        // SAFETY: `p` is `n` bytes, filled by `GetByteArrayRegion`, and freed on every path.
        unsafe {
            let p = alloc(n);
            if p.is_null() {
                return fail_null(env);
            }
            ((**env).v1_2.GetByteArrayRegion)(env, input, 0, len, p.cast::<jbyte>());
            if ((**env).v1_2.ExceptionCheck)(env) {
                release(p);
                return PENDING;
            }
            let rc = door(p, n);
            release(p);
            rc
        }
    }
}

/// Writes a verdict where Java reads it: the value's words to `out[0..2]` on success, the
/// span to `out[2..4]` on a fault, and nothing on a contract violation or [`PENDING`].
#[inline(always)]
unsafe fn finish<T: Words>(
    env: *mut JNIEnv,
    out: jlongArray,
    rc: i32,
    value: &MaybeUninit<T>,
    fault: &RawFault,
) -> jint {
    if rc == 0 {
        // SAFETY: the core wrote the value: it returns 0 only after it has.
        let words = unsafe { value.assume_init_read() }.words();
        // SAFETY: the caller passes a live env and a `long[]` of at least four elements.
        unsafe { ((**env).v1_2.SetLongArrayRegion)(env, out, 0, 2, words.as_ptr()) };
    } else if rc > 0 {
        let span = [jlong::from(fault.offset), jlong::from(fault.len)];
        // SAFETY: as above.
        unsafe { ((**env).v1_2.SetLongArrayRegion)(env, out, 2, 2, span.as_ptr()) };
    }
    rc
}

/// A text door: `call` is the core export, handed the input, the value slot and the span.
#[inline(always)]
unsafe fn text_door<T: Words>(
    env: *mut JNIEnv,
    input: jbyteArray,
    out: jlongArray,
    call: impl FnOnce(*const u8, usize, *mut T, *mut RawFault) -> i32,
) -> jint {
    let mut value = MaybeUninit::<T>::uninit();
    let mut fault = RawFault { offset: 0, len: 0 };
    // SAFETY: the caller passes a live env and a `byte[]`.
    let rc = unsafe {
        with_text(env, input, |p, n| {
            call(p, n, value.as_mut_ptr(), &mut fault)
        })
    };
    // SAFETY: the caller passes a live env and a `long[4]`.
    unsafe { finish(env, out, rc, &value, &fault) }
}

/// The layouts a door reads besides its text, [`RawNumFormat`], [`RawDayPeriods`] and
/// [`RawDateOptions`]: `repr(C)` structs of `u32` and `u8` fields (and padding), so any bytes
/// at all are a value of any, which the core's `resolve` then checks.
trait Layout: Copy {
    /// Nothing declared; the bytes Java sends overwrite all of it.
    const ZERO: Self;
}

impl Layout for RawNumFormat {
    const ZERO: Self = RawNumFormat::ZERO;
}

impl Layout for RawDayPeriods {
    const ZERO: Self = RawDayPeriods::ZERO;
}

impl Layout for RawDateOptions {
    const ZERO: Self = RawDateOptions {
        size: 0,
        flags: 0,
        pivot: 0,
    };
}

/// A layout as `NativeCore` packs it, in the platform's own byte order, copied onto the
/// stack whole: `size_of::<T>()` bytes. `None` with an exception pending (an array shorter
/// than the layout), and then no core call is made.
#[inline(always)]
unsafe fn read_layout<T: Layout>(env: *mut JNIEnv, array: jbyteArray) -> Option<T> {
    let mut layout = T::ZERO;
    // SAFETY: the caller passes a live env and a `byte[]`; `layout` is `size_of::<T>()`
    // bytes, and any bytes are a `T` (the trait's contract).
    unsafe {
        ((**env).v1_2.GetByteArrayRegion)(
            env,
            array,
            0,
            size_of::<T>() as jsize,
            (&raw mut layout).cast::<jbyte>(),
        );
        if ((**env).v1_2.ExceptionCheck)(env) {
            return None;
        }
    }
    Some(layout)
}

/// A numeric door: the format is read before the input is pinned, since no JNI call may
/// run inside the critical region.
#[inline(always)]
unsafe fn numeric_door<T: Words>(
    env: *mut JNIEnv,
    input: jbyteArray,
    format: jbyteArray,
    out: jlongArray,
    call: impl FnOnce(*const u8, usize, *const RawNumFormat, *mut T, *mut RawFault) -> i32,
) -> jint {
    // SAFETY: the caller passes a live env and `byte[]`s.
    let Some(layout) = (unsafe { read_layout::<RawNumFormat>(env, format) }) else {
        return PENDING;
    };
    // SAFETY: as above; `layout` outlives the call.
    unsafe {
        text_door(env, input, out, |p, n, value, fault| {
            call(p, n, &layout, value, fault)
        })
    }
}

/// A typed door: a `double` in, no input to pin.
#[inline(always)]
unsafe fn typed_door<T: Words>(
    env: *mut JNIEnv,
    out: jlongArray,
    call: impl FnOnce(*mut T, *mut RawFault) -> i32,
) -> jint {
    let mut value = MaybeUninit::<T>::uninit();
    let mut fault = RawFault { offset: 0, len: 0 };
    let rc = call(value.as_mut_ptr(), &mut fault);
    // SAFETY: the caller passes a live env and a `long[4]`.
    unsafe { finish(env, out, rc, &value, &fault) }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_bool(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: the JNI contract; the core writes only through the pointers it is handed.
    unsafe { text_door::<u8>(env, input, out, |p, n, v, f| cast_bool(p, n, v, f)) }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_char(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe { text_door::<u32>(env, input, out, |p, n, v, f| cast_char(p, n, v, f)) }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_i8(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    format: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        numeric_door::<i8>(env, input, format, out, |p, n, r, v, f| {
            cast_i8(p, n, r, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_i16(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    format: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        numeric_door::<i16>(env, input, format, out, |p, n, r, v, f| {
            cast_i16(p, n, r, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_i32(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    format: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        numeric_door::<i32>(env, input, format, out, |p, n, r, v, f| {
            cast_i32(p, n, r, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_i64(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    format: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        numeric_door::<i64>(env, input, format, out, |p, n, r, v, f| {
            cast_i64(p, n, r, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_u8(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    format: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        numeric_door::<u8>(env, input, format, out, |p, n, r, v, f| {
            cast_u8(p, n, r, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_u16(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    format: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        numeric_door::<u16>(env, input, format, out, |p, n, r, v, f| {
            cast_u16(p, n, r, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_u32(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    format: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        numeric_door::<u32>(env, input, format, out, |p, n, r, v, f| {
            cast_u32(p, n, r, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_u64(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    format: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        numeric_door::<u64>(env, input, format, out, |p, n, r, v, f| {
            cast_u64(p, n, r, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_f32(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    format: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        numeric_door::<f32>(env, input, format, out, |p, n, r, v, f| {
            cast_f32(p, n, r, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_f64(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    format: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        numeric_door::<f64>(env, input, format, out, |p, n, r, v, f| {
            cast_f64(p, n, r, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_decimal(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    format: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        numeric_door::<Decimal>(env, input, format, out, |p, n, r, v, f| {
            cast_decimal(p, n, r, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_uuid(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe { text_door::<[u8; 16]>(env, input, out, |p, n, v, f| cast_uuid(p, n, v, f)) }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_timestamp(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe { text_door::<Timestamp>(env, input, out, |p, n, v, f| cast_timestamp(p, n, v, f)) }
}

/// `precision` is the declared unit's code; any other value is the core's contract violation.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_unix(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    precision: jint,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        text_door::<Timestamp>(env, input, out, |p, n, v, f| {
            cast_unix(p, n, precision as u32, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_excel_serial(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    epoch: jint,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        text_door::<Timestamp>(env, input, out, |p, n, v, f| {
            cast_excel_serial(p, n, epoch as u32, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_date_ordered(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    order: jint,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        text_door::<Date>(env, input, out, |p, n, v, f| {
            cast_date_ordered(p, n, order as u32, v, f)
        })
    }
}

/// `periods` is the 40-byte designator layout, or null when nothing is declared (the core's
/// null). It is read before the input is pinned.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_datetime(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    order: jint,
    periods: jbyteArray,
    out: jlongArray,
) -> jint {
    let layout = if periods.is_null() {
        None
    } else {
        // SAFETY: the caller passes a live env and a `byte[]`.
        match unsafe { read_layout::<RawDayPeriods>(env, periods) } {
            Some(layout) => Some(layout),
            None => return PENDING,
        }
    };
    let declared: *const RawDayPeriods = match &layout {
        Some(layout) => layout,
        None => null(),
    };
    // SAFETY: as `native_cast_bool`; `layout` outlives the call.
    unsafe {
        text_door::<CivilDateTime>(env, input, out, |p, n, v, f| {
            cast_datetime(p, n, order as u32, declared, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_date(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe { text_door::<Date>(env, input, out, |p, n, v, f| cast_date(p, n, v, f)) }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_time(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe { text_door::<u64>(env, input, out, |p, n, v, f| cast_time(p, n, v, f)) }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_duration(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe { text_door::<Duration>(env, input, out, |p, n, v, f| cast_duration(p, n, v, f)) }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_decimal_from_f64(
    env: *mut JNIEnv,
    _class: jclass,
    value: jdouble,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe { typed_door::<Decimal>(env, out, |v, f| cast_decimal_from_f64(value, v, f)) }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_excel_serial_from_f64(
    env: *mut JNIEnv,
    _class: jclass,
    value: jdouble,
    epoch: jint,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe {
        typed_door::<CivilDateTime>(env, out, |v, f| {
            cast_excel_serial_from_f64(value, epoch as u32, v, f)
        })
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_excel_time(
    env: *mut JNIEnv,
    _class: jclass,
    value: jdouble,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe { typed_door::<u64>(env, out, |v, f| cast_excel_time(value, v, f)) }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_excel_duration(
    env: *mut JNIEnv,
    _class: jclass,
    value: jdouble,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe { typed_door::<Duration>(env, out, |v, f| cast_excel_duration(value, v, f)) }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_hypercast_version(_env: *mut JNIEnv, _class: jclass) -> jint {
    // SAFETY: takes no arguments.
    unsafe { hypercast_version() as jint }
}

/// [`RawConstraints`] as `NativeCore` packs it, in the platform's byte order: the same
/// fields at the same offsets, but each `Decimal`'s sign a plain byte, so that any bytes at
/// all are a value of it (a `bool` is not) and it can be read as a [`Layout`]. Rebuilt as
/// the core's own type by [`WireConstraints::raw`].
#[repr(C)]
#[derive(Clone, Copy)]
struct WireConstraints {
    size: u32,
    flags: u32,
    rounding: u32,
    min_length: u32,
    max_length: u32,
    reserved: u32,
    lower: WireDecimal,
    upper: WireDecimal,
    multiple_of: WireDecimal,
    exclusive_lower: WireDecimal,
    exclusive_upper: WireDecimal,
}

/// [`Decimal`]'s 16 bytes, its sign a byte.
#[repr(C)]
#[derive(Clone, Copy)]
struct WireDecimal {
    lo: u64,
    hi: u32,
    scale: u8,
    negative: u8,
    padding: [u8; 2],
}

const _: () = {
    use core::mem::offset_of;
    assert!(size_of::<WireConstraints>() == RawConstraints::SIZE as usize);
    assert!(size_of::<WireConstraints>() == size_of::<RawConstraints>());
    assert!(offset_of!(WireConstraints, lower) == offset_of!(RawConstraints, lower));
    assert!(offset_of!(WireConstraints, upper) == offset_of!(RawConstraints, upper));
    assert!(offset_of!(WireConstraints, multiple_of) == offset_of!(RawConstraints, multiple_of));
    assert!(
        offset_of!(WireConstraints, exclusive_lower) == offset_of!(RawConstraints, exclusive_lower)
    );
    assert!(
        offset_of!(WireConstraints, exclusive_upper) == offset_of!(RawConstraints, exclusive_upper)
    );
    assert!(size_of::<WireDecimal>() == size_of::<Decimal>());
    assert!(offset_of!(WireDecimal, scale) == offset_of!(Decimal, scale));
    assert!(offset_of!(WireDecimal, negative) == offset_of!(Decimal, negative));
};

const WIRE_DECIMAL_ZERO: WireDecimal = WireDecimal {
    lo: 0,
    hi: 0,
    scale: 0,
    negative: 0,
    padding: [0; 2],
};

impl Layout for WireConstraints {
    const ZERO: Self = WireConstraints {
        size: 0,
        flags: 0,
        rounding: 0,
        min_length: 0,
        max_length: 0,
        reserved: 0,
        lower: WIRE_DECIMAL_ZERO,
        upper: WIRE_DECIMAL_ZERO,
        multiple_of: WIRE_DECIMAL_ZERO,
        exclusive_lower: WIRE_DECIMAL_ZERO,
        exclusive_upper: WIRE_DECIMAL_ZERO,
    };
}

impl WireDecimal {
    #[inline(always)]
    fn decimal(self) -> Decimal {
        Decimal {
            lo: self.lo,
            hi: self.hi,
            scale: self.scale,
            negative: self.negative != 0,
        }
    }
}

impl WireConstraints {
    /// The core's own layout, field for field. `NativeCore` writes a sign as 0 or 1.
    #[inline(always)]
    fn raw(self) -> RawConstraints {
        RawConstraints {
            size: self.size,
            flags: self.flags,
            rounding: self.rounding,
            min_length: self.min_length,
            max_length: self.max_length,
            _reserved: self.reserved,
            lower: self.lower.decimal(),
            upper: self.upper.decimal(),
            multiple_of: self.multiple_of.decimal(),
            exclusive_lower: self.exclusive_lower.decimal(),
            exclusive_upper: self.exclusive_upper.decimal(),
        }
    }
}

/// The `enum` list, which `NativeCore` keeps in a direct `ByteBuffer` built once with its
/// `Constraints`: its address and length, read before any input is pinned, so the list is
/// neither pinned nor copied on a call. A null buffer, or an empty one, is a list with no
/// entries (the layout's `HAS_ENUM` says whether an `enum` is declared at all); a buffer with
/// no address (not direct) is `None`, which the natives answer as a contract violation.
#[inline(always)]
unsafe fn read_entries(env: *mut JNIEnv, entries: jobject) -> Option<(*const u8, usize)> {
    if entries.is_null() {
        return Some((null(), 0));
    }
    // SAFETY: the caller passes a live env and a `ByteBuffer`.
    let capacity = unsafe { ((**env).v1_4.GetDirectBufferCapacity)(env, entries) };
    let len = usize::try_from(capacity).ok()?;
    if len == 0 {
        return Some((null(), 0));
    }
    // SAFETY: as above.
    let p = unsafe { ((**env).v1_4.GetDirectBufferAddress)(env, entries) };
    if p.is_null() {
        return None;
    }
    Some((p.cast_const().cast(), len))
}

/// What a checked door reports beside its value, as `out[4..7]`: the keyword broken (0 for
/// none), whether rounding adjusted the value (1 or 0), and the matched text entry's index
/// (`u32::MAX`, zero-extended, for none), the C ABI's `RawCheck` field for field.
#[inline(always)]
pub fn check_words(check: &RawCheck) -> [jlong; 3] {
    [
        jlong::from(check.keyword),
        jlong::from(check.adjusted),
        jlong::from(check.enum_index),
    ]
}

const I8: u32 = Door::I8.code();
const I16: u32 = Door::I16.code();
const I32: u32 = Door::I32.code();
const I64: u32 = Door::I64.code();
const U8: u32 = Door::U8.code();
const U16: u32 = Door::U16.code();
const U32: u32 = Door::U32.code();
const U64: u32 = Door::U64.code();
const F32: u32 = Door::F32.code();
const F64: u32 = Door::F64.code();
const TEXT: u32 = Door::Text.code();
const DECIMAL: u32 = Door::Decimal.code();

/// The value a checked door wrote into `slot`, as the words its plain door's native hands
/// back: one door, one type, so `NativeCore`'s readers serve both. Read only after a 0 or a 4
/// (a violation carries the value as cast), which the core returns only for a door it knows.
#[inline(always)]
unsafe fn checked_words(door: u32, slot: &[u64; 2]) -> [jlong; 2] {
    let p = slot.as_ptr();
    // SAFETY: `slot` is 16 bytes, 8-aligned and zeroed, so it holds any door's value; the core
    // wrote the door's own type there, so a `Decimal`'s sign is a valid `bool`.
    unsafe {
        match door {
            I8 => p.cast::<i8>().read().words(),
            I16 => p.cast::<i16>().read().words(),
            I32 => p.cast::<i32>().read().words(),
            I64 => p.cast::<i64>().read().words(),
            U8 => p.cast::<u8>().read().words(),
            U16 => p.cast::<u16>().read().words(),
            U32 => p.cast::<u32>().read().words(),
            U64 => p.cast::<u64>().read().words(),
            F32 => p.cast::<f32>().read().words(),
            F64 => p.cast::<f64>().read().words(),
            TEXT => p.cast::<RawSpan>().read().words(),
            DECIMAL => p.cast::<Decimal>().read().words(),
            _ => [0, 0],
        }
    }
}

/// A checked verdict where Java reads it: [`finish`]'s writes, the value as cast to
/// `out[0..2]` on a violation too (the core writes it on 4 as on 0), then the check to
/// `out[4..7]` on every code the core returned but `-1`.
#[inline(always)]
unsafe fn finish_checked(
    env: *mut JNIEnv,
    out: jlongArray,
    rc: i32,
    door: u32,
    slot: &[u64; 2],
    fault: &RawFault,
    check: &RawCheck,
) -> jint {
    if rc == 0 || rc == 4 {
        // SAFETY: the core wrote the value: it returns 0 or 4 only after it has.
        let words = unsafe { checked_words(door, slot) };
        // SAFETY: the caller passes a live env and a `long[]` of at least seven elements.
        unsafe { ((**env).v1_2.SetLongArrayRegion)(env, out, 0, 2, words.as_ptr()) };
    }
    if rc > 0 {
        let span = [jlong::from(fault.offset), jlong::from(fault.len)];
        // SAFETY: as above.
        unsafe { ((**env).v1_2.SetLongArrayRegion)(env, out, 2, 2, span.as_ptr()) };
    }
    if rc >= 0 {
        let words = check_words(check);
        // SAFETY: as above.
        unsafe { ((**env).v1_2.SetLongArrayRegion)(env, out, 4, 3, words.as_ptr()) };
    }
    rc
}

/// The check a checked door starts from, which the core overwrites on 0 and 4.
const NO_CHECK: RawCheck = RawCheck {
    keyword: 0,
    adjusted: 0,
    enum_index: u32::MAX,
};

/// A checked door: `door` is the core's door code (an integer or float door, 18 Text, 19
/// Decimal), `format` the 92-byte layout or null (the invariant one; Text ignores it),
/// `constraints` the 104-byte layout and `entries` the `enum` list or null. Every layout is
/// read before the input is pinned.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "system" fn native_cast_checked(
    env: *mut JNIEnv,
    _class: jclass,
    door: jint,
    format: jbyteArray,
    constraints: jbyteArray,
    entries: jobject,
    input: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: the caller passes a live env and `byte[]`s.
    let Some(declared) = (unsafe { read_layout::<WireConstraints>(env, constraints) }) else {
        return PENDING;
    };
    let declared = declared.raw();
    let layout = if format.is_null() {
        None
    } else {
        // SAFETY: as above.
        match unsafe { read_layout::<RawNumFormat>(env, format) } {
            Some(layout) => Some(layout),
            None => return PENDING,
        }
    };
    let notation: *const RawNumFormat = match &layout {
        Some(layout) => layout,
        None => null(),
    };
    // SAFETY: as above.
    let Some((enum_ptr, enum_len)) = (unsafe { read_entries(env, entries) }) else {
        return -1;
    };
    let door = door as u32;
    let mut slot = [0u64; 2];
    let mut fault = RawFault { offset: 0, len: 0 };
    let mut check = NO_CHECK;
    // SAFETY: as `native_cast_bool`; the layouts and the list outlive the call.
    let rc = unsafe {
        with_text(env, input, |p, n| {
            cast_checked(
                door,
                0,
                notation,
                &declared,
                enum_ptr,
                enum_len,
                p,
                n,
                slot.as_mut_ptr().cast(),
                &mut fault,
                &mut check,
            )
        })
    };
    // SAFETY: the caller passes a live env and a `long[7]`.
    unsafe { finish_checked(env, out, rc, door, &slot, &fault, &check) }
}

/// The checked door over a `double` a source already decoded: `door` is a numeric door.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_checked_from_f64(
    env: *mut JNIEnv,
    _class: jclass,
    door: jint,
    value: jdouble,
    constraints: jbyteArray,
    entries: jobject,
    out: jlongArray,
) -> jint {
    // SAFETY: the caller passes a live env and a `byte[]`.
    let Some(declared) = (unsafe { read_layout::<WireConstraints>(env, constraints) }) else {
        return PENDING;
    };
    let declared = declared.raw();
    // SAFETY: as above.
    let Some((enum_ptr, enum_len)) = (unsafe { read_entries(env, entries) }) else {
        return -1;
    };
    let door = door as u32;
    let mut slot = [0u64; 2];
    let mut fault = RawFault { offset: 0, len: 0 };
    let mut check = NO_CHECK;
    // SAFETY: the core writes only through the pointers it is handed.
    let rc = unsafe {
        cast_checked_from_f64(
            door,
            value,
            &declared,
            enum_ptr,
            enum_len,
            slot.as_mut_ptr().cast(),
            &mut fault,
            &mut check,
        )
    };
    // SAFETY: the caller passes a live env and a `long[7]`.
    unsafe { finish_checked(env, out, rc, door, &slot, &fault, &check) }
}

/// A layout that may be absent: `Some(None)` for a null array (the core's null pointer, "none
/// declared"), `Some(Some(layout))` for the bytes, and `None` with an exception pending (an
/// array shorter than the layout), where the native returns [`PENDING`] before any core call.
#[inline(always)]
unsafe fn optional_layout<T: Layout>(env: *mut JNIEnv, array: jbyteArray) -> Option<Option<T>> {
    if array.is_null() {
        return Some(None);
    }
    // SAFETY: the caller passes a live env and a `byte[]`.
    unsafe { read_layout::<T>(env, array) }.map(Some)
}

/// The pointer to a layout read by [`optional_layout`], or null.
#[inline(always)]
fn layout_ptr<T>(layout: &Option<T>) -> *const T {
    match layout {
        Some(layout) => layout,
        None => null(),
    }
}

const BOOL: u32 = Door::Bool.code();
const UNIX: u32 = Door::Unix(UnixPrecision::Seconds).code();
const UUID: u32 = Door::Uuid.code();
const TIMESTAMP: u32 = Door::Timestamp.code();
const DATE: u32 = Door::Date.code();
const TIME: u32 = Door::Time.code();
const DURATION: u32 = Door::Duration.code();
const DATE_ORDERED: u32 = Door::DateOrdered(DateOrder::YearMonthDay).code();
const DATETIME: u32 = Door::DateTime(DateOrder::YearMonthDay).code();
const EXCEL_SERIAL: u32 = Door::ExcelSerial(ExcelEpoch::Y1900).code();
const CHAR: u32 = Door::Char.code();

/// Whether `T` fits the 16-byte, 8-aligned slot a door writes its value into (`[u64; 2]`).
const fn fits_slot<T>() -> bool {
    size_of::<T>() <= size_of::<[u64; 2]>() && align_of::<T>() <= align_of::<[u64; 2]>()
}

// Every type `detailed_words`, `typed_words` and `checked_words` read back out of the slot: each
// must fit it, or the read would run past what the core wrote.
const _: () = {
    assert!(fits_slot::<u8>() && fits_slot::<i8>());
    assert!(fits_slot::<u16>() && fits_slot::<i16>());
    assert!(fits_slot::<u32>() && fits_slot::<i32>());
    assert!(fits_slot::<u64>() && fits_slot::<i64>());
    assert!(fits_slot::<f32>() && fits_slot::<f64>());
    assert!(fits_slot::<[u8; 16]>());
    assert!(fits_slot::<Timestamp>() && fits_slot::<Duration>());
    assert!(fits_slot::<Date>() && fits_slot::<CivilDateTime>());
    assert!(fits_slot::<RawSpan>() && fits_slot::<Decimal>());
};

/// The value any detailed door wrote into `slot`, as the words its 1.0 native hands back.
/// Read only after a 0, which the core returns only for a door it knows.
#[inline(always)]
unsafe fn detailed_words(door: u32, slot: &[u64; 2]) -> [jlong; 2] {
    let p = slot.as_ptr();
    // SAFETY: `slot` is 16 bytes, 8-aligned and zeroed, so it holds any door's value; the core
    // wrote the door's own type there, so a `bool` or a `Decimal`'s sign is valid.
    unsafe {
        match door {
            BOOL => p.cast::<u8>().read().words(),
            I8 => p.cast::<i8>().read().words(),
            I16 => p.cast::<i16>().read().words(),
            I32 => p.cast::<i32>().read().words(),
            I64 => p.cast::<i64>().read().words(),
            U8 => p.cast::<u8>().read().words(),
            U16 => p.cast::<u16>().read().words(),
            U32 | CHAR => p.cast::<u32>().read().words(),
            U64 => p.cast::<u64>().read().words(),
            F32 => p.cast::<f32>().read().words(),
            F64 => p.cast::<f64>().read().words(),
            UUID => p.cast::<[u8; 16]>().read().words(),
            TIMESTAMP | UNIX | EXCEL_SERIAL => p.cast::<Timestamp>().read().words(),
            DATE | DATE_ORDERED => p.cast::<Date>().read().words(),
            TIME => p.cast::<u64>().read().words(),
            DURATION => p.cast::<Duration>().read().words(),
            DATETIME => p.cast::<CivilDateTime>().read().words(),
            TEXT => p.cast::<RawSpan>().read().words(),
            DECIMAL => p.cast::<Decimal>().read().words(),
            _ => [0, 0],
        }
    }
}

/// The value a typed detailed door wrote into `slot`: the typed doors' own types, which for
/// 22 is the civil date-time the text door's `Timestamp` is not.
#[inline(always)]
unsafe fn typed_words(door: u32, slot: &[u64; 2]) -> [jlong; 2] {
    let p = slot.as_ptr();
    // SAFETY: as `detailed_words`.
    unsafe {
        match door {
            DECIMAL => p.cast::<Decimal>().read().words(),
            EXCEL_SERIAL => p.cast::<CivilDateTime>().read().words(),
            TIME => p.cast::<u64>().read().words(),
            DURATION => p.cast::<Duration>().read().words(),
            _ => [0, 0],
        }
    }
}

/// A fault the way the detailed exports take it: `size` is the caller's to set, the core
/// writes the rest.
const NO_FAULT: RawFaultDetail = RawFaultDetail {
    size: RawFaultDetail::SIZE,
    offset: 0,
    len: 0,
    detail: 0,
};

/// A detailed verdict where Java reads it: the value's words (`words_of`: the text doors' or the
/// typed doors') to `out[0..2]` on a 0, and on a fault the span to `out[2..4]` and the `detail` code to `out[7]`. Nothing on `-1`.
#[inline(always)]
unsafe fn finish_detailed(
    env: *mut JNIEnv,
    out: jlongArray,
    rc: i32,
    words_of: unsafe fn(u32, &[u64; 2]) -> [jlong; 2],
    door: u32,
    slot: &[u64; 2],
    fault: &RawFaultDetail,
) -> jint {
    if rc == 0 {
        // SAFETY: the core wrote the value: it returns 0 only after it has.
        let words = unsafe { words_of(door, slot) };
        // SAFETY: the caller passes a live env and a `long[8]`.
        unsafe { ((**env).v1_2.SetLongArrayRegion)(env, out, 0, 2, words.as_ptr()) };
    } else if rc > 0 {
        let span = [jlong::from(fault.offset), jlong::from(fault.len)];
        let detail = [jlong::from(fault.detail)];
        // SAFETY: as above.
        unsafe {
            ((**env).v1_2.SetLongArrayRegion)(env, out, 2, 2, span.as_ptr());
            ((**env).v1_2.SetLongArrayRegion)(env, out, 7, 1, detail.as_ptr());
        }
    }
    rc
}

/// `cast_detailed`: `door` is `Door`'s code and `param` what it declares. `format` is the
/// 92-byte layout, `periods` the 40-byte one and `options` the 12-byte `RawDateOptions`, each
/// null for none (the core's null); `vocab` is the packed month vocabulary in a direct
/// buffer, or null. Every layout is read before the input is pinned.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "system" fn native_cast_detailed(
    env: *mut JNIEnv,
    _class: jclass,
    door: jint,
    param: jint,
    format: jbyteArray,
    periods: jbyteArray,
    options: jbyteArray,
    vocab: jobject,
    input: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: the caller passes a live env and `byte[]`s.
    let Some(format) = (unsafe { optional_layout::<RawNumFormat>(env, format) }) else {
        return PENDING;
    };
    // SAFETY: as above.
    let Some(periods) = (unsafe { optional_layout::<RawDayPeriods>(env, periods) }) else {
        return PENDING;
    };
    // SAFETY: as above.
    let Some(options) = (unsafe { optional_layout::<RawDateOptions>(env, options) }) else {
        return PENDING;
    };
    // SAFETY: as above.
    let Some((vocab_ptr, vocab_len)) = (unsafe { read_entries(env, vocab) }) else {
        return -1;
    };
    let door = door as u32;
    let mut slot = [0u64; 2];
    let mut fault = NO_FAULT;
    // SAFETY: as `native_cast_bool`; the layouts and the list outlive the call.
    let rc = unsafe {
        with_text(env, input, |p, n| {
            cast_detailed(
                door,
                param as u32,
                layout_ptr(&format),
                layout_ptr(&periods),
                layout_ptr(&options),
                vocab_ptr,
                vocab_len,
                p,
                n,
                slot.as_mut_ptr().cast(),
                &mut fault,
            )
        })
    };
    // SAFETY: the caller passes a live env and a `long[8]`.
    unsafe { finish_detailed(env, out, rc, detailed_words, door, &slot, &fault) }
}

/// `cast_detailed_from_f64`: the typed doors, a `double` in and no input to pin.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_detailed_from_f64(
    env: *mut JNIEnv,
    _class: jclass,
    door: jint,
    param: jint,
    value: jdouble,
    out: jlongArray,
) -> jint {
    let door = door as u32;
    let mut slot = [0u64; 2];
    let mut fault = NO_FAULT;
    // SAFETY: the core writes only through the pointers it is handed.
    let rc = unsafe {
        cast_detailed_from_f64(
            door,
            param as u32,
            value,
            slot.as_mut_ptr().cast(),
            &mut fault,
        )
    };
    // SAFETY: the caller passes a live env and a `long[8]`.
    unsafe { finish_detailed(env, out, rc, typed_words, door, &slot, &fault) }
}

/// A checked detailed verdict where Java reads it: [`finish_detailed`]'s writes, the value as
/// cast to `out[0..2]` on a violation too (the core writes it on 4 as on 0), then the check to
/// `out[4..7]` on every code but `-1` (the detail is `out[7]`, written with the fault).
#[inline(always)]
unsafe fn finish_checked_detailed(
    env: *mut JNIEnv,
    out: jlongArray,
    rc: i32,
    door: u32,
    slot: &[u64; 2],
    fault: &RawFaultDetail,
    check: &RawCheck,
) -> jint {
    // SAFETY: the caller passes a live env and a `long[8]`; a checked door's value is its
    // plain door's, which `checked_words` reads.
    unsafe { finish_detailed(env, out, rc, checked_words, door, slot, fault) };
    if rc == 4 {
        // SAFETY: the core wrote the value: it returns 4 only after it has.
        let words = unsafe { checked_words(door, slot) };
        // SAFETY: as above.
        unsafe { ((**env).v1_2.SetLongArrayRegion)(env, out, 0, 2, words.as_ptr()) };
    }
    if rc >= 0 {
        let words = check_words(check);
        // SAFETY: as above.
        unsafe { ((**env).v1_2.SetLongArrayRegion)(env, out, 4, 3, words.as_ptr()) };
    }
    rc
}

/// `cast_checked_detailed`: [`native_cast_checked`]'s arguments and layouts, the fault carrying
/// its `detail` to `out[7]`. No checked door declares a parameter, so `param` is 0.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "system" fn native_cast_checked_detailed(
    env: *mut JNIEnv,
    _class: jclass,
    door: jint,
    format: jbyteArray,
    constraints: jbyteArray,
    entries: jobject,
    input: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: the caller passes a live env and `byte[]`s.
    let Some(declared) = (unsafe { read_layout::<WireConstraints>(env, constraints) }) else {
        return PENDING;
    };
    let declared = declared.raw();
    // SAFETY: as above.
    let Some(format) = (unsafe { optional_layout::<RawNumFormat>(env, format) }) else {
        return PENDING;
    };
    // SAFETY: as above.
    let Some((enum_ptr, enum_len)) = (unsafe { read_entries(env, entries) }) else {
        return -1;
    };
    let door = door as u32;
    let mut slot = [0u64; 2];
    let mut fault = NO_FAULT;
    let mut check = NO_CHECK;
    // SAFETY: as `native_cast_bool`; the layouts and the list outlive the call.
    let rc = unsafe {
        with_text(env, input, |p, n| {
            cast_checked_detailed(
                door,
                0,
                layout_ptr(&format),
                &declared,
                enum_ptr,
                enum_len,
                p,
                n,
                slot.as_mut_ptr().cast(),
                &mut fault,
                &mut check,
            )
        })
    };
    // SAFETY: the caller passes a live env and a `long[8]`.
    unsafe { finish_checked_detailed(env, out, rc, door, &slot, &fault, &check) }
}

/// `cast_checked_detailed_from_f64`: the checked door over a `double` a source already
/// decoded, the fault carrying its `detail` to `out[7]`.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_checked_detailed_from_f64(
    env: *mut JNIEnv,
    _class: jclass,
    door: jint,
    value: jdouble,
    constraints: jbyteArray,
    entries: jobject,
    out: jlongArray,
) -> jint {
    // SAFETY: the caller passes a live env and a `byte[]`.
    let Some(declared) = (unsafe { read_layout::<WireConstraints>(env, constraints) }) else {
        return PENDING;
    };
    let declared = declared.raw();
    // SAFETY: as above.
    let Some((enum_ptr, enum_len)) = (unsafe { read_entries(env, entries) }) else {
        return -1;
    };
    let door = door as u32;
    let mut slot = [0u64; 2];
    let mut fault = NO_FAULT;
    let mut check = NO_CHECK;
    // SAFETY: the core writes only through the pointers it is handed.
    let rc = unsafe {
        cast_checked_detailed_from_f64(
            door,
            0,
            &declared,
            enum_ptr,
            enum_len,
            value,
            slot.as_mut_ptr().cast(),
            &mut fault,
            &mut check,
        )
    };
    // SAFETY: the caller passes a live env and a `long[8]`.
    unsafe { finish_checked_detailed(env, out, rc, door, &slot, &fault, &check) }
}

/// Judges a declaration once, as `Constraints` is built: the core's 0 or refusal code, or
/// `-1` for an `enum` buffer that is not direct (a binding bug).
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_constraints_check(
    env: *mut JNIEnv,
    _class: jclass,
    door: jint,
    constraints: jbyteArray,
    entries: jobject,
) -> jint {
    // SAFETY: the caller passes a live env and a `byte[]`.
    let Some(declared) = (unsafe { read_layout::<WireConstraints>(env, constraints) }) else {
        return PENDING;
    };
    let declared = declared.raw();
    // SAFETY: as above.
    let Some((enum_ptr, enum_len)) = (unsafe { read_entries(env, entries) }) else {
        return -1;
    };
    // SAFETY: the core only reads the layout and the list.
    unsafe { cast_constraints_check(door as u32, 0, &declared, enum_ptr, enum_len) }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_text(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe { text_door::<RawSpan>(env, input, out, |p, n, v, f| cast_text(p, n, v, f)) }
}

/// A declared bound, step or integer `enum` entry in invariant notation, converted by the
/// core: 0 with the `Decimal`'s words, or 8 for text no door can hold.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_cast_declared_decimal(
    env: *mut JNIEnv,
    _class: jclass,
    input: jbyteArray,
    out: jlongArray,
) -> jint {
    // SAFETY: as `native_cast_bool`.
    unsafe { text_door::<Decimal>(env, input, out, |p, n, v, _| cast_declared_decimal(p, n, v)) }
}

/// The length of the registration table, never a literal.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn native_jni_natives(_env: *mut JNIEnv, _class: jclass) -> jint {
    NATIVES.len() as jint
}

/// Registers every native on `NativeCore`, eagerly, so a table that does not match the Java
/// declarations fails `System.load` (with the pending exception) instead of a later call.
#[unsafe(no_mangle)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub unsafe extern "system" fn JNI_OnLoad(vm: *mut JavaVM, _reserved: *mut c_void) -> jint {
    let mut penv: *mut c_void = null_mut();
    // SAFETY: the JVM passes a live `JavaVM`.
    let got = unsafe { ((**vm).v1_2.GetEnv)(vm, &mut penv, JNI_VERSION_1_8) };
    // The null check is on `env`, the pointer dereferenced below, not on `penv`: CodeQL's
    // rust/access-invalid-pointer query takes `is_null()` as a barrier for its receiver only.
    let env = penv.cast::<*const jni_sys::JNINativeInterface_>();
    if got != JNI_OK || env.is_null() {
        return JNI_ERR;
    }
    // SAFETY: `env` came from `GetEnv` above and is not null; the class name is NUL-terminated.
    let class = unsafe { ((**env).v1_2.FindClass)(env, CLASS.as_ptr().cast::<c_char>()) };
    if class.is_null() {
        return JNI_ERR;
    }
    let empty = JNINativeMethod {
        name: null_mut(),
        signature: null_mut(),
        fnPtr: null_mut(),
    };
    let mut methods = [empty; NATIVES.len()];
    let mut i = 0;
    while i < NATIVES.len() {
        methods[i] = JNINativeMethod {
            name: NATIVES[i].name.as_ptr().cast_mut().cast::<c_char>(),
            signature: NATIVES[i].descriptor.as_ptr().cast_mut().cast::<c_char>(),
            fnPtr: NATIVES[i].function.cast_mut(),
        };
        i += 1;
    }
    // SAFETY: `methods` holds `NATIVES.len()` entries of NUL-terminated strings.
    let rc = unsafe {
        ((**env).v1_2.RegisterNatives)(env, class, methods.as_ptr(), NATIVES.len() as jint)
    };
    if rc != JNI_OK {
        return JNI_ERR;
    }
    JNI_VERSION_1_8
}

#[cfg(test)]
mod tests;
