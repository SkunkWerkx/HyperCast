//! The natives against a stub `JNIEnv`, beside the core called directly through its C ABI:
//! the registration table, every door's words, the critical-region discipline and every
//! failed-array-access path. Arrays are plain pointers the tests own.

use super::*;
use core::cell::RefCell;
use core::mem::MaybeUninit;
use core::ptr::{addr_of_mut, null, null_mut};
use hypercast::NumFormat;
use jni_sys::{JNINativeInterface_, jarray, jboolean, jsize};

/// What the stub env records and how it behaves; one per test thread.
#[derive(Default)]
struct State {
    crit_gets: usize,
    crit_releases: usize,
    throws: usize,
    mallocs: usize,
    frees: usize,
    crit_null: bool,
    pending: bool,
    pending_after_region: bool,
    malloc_null: bool,
    array_len: jsize,
    order: Vec<&'static str>,
    found_class: String,
    release_mode: Option<jint>,
    long_writes: Vec<(jsize, jsize)>,
    buffer_capacity: jlong,
    buffer_address_null: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// The test stand-ins for `alloc` and `release`: counted, and switchable to fail.
pub(super) fn alloc(n: usize) -> *mut u8 {
    if with(|s| {
        if s.malloc_null {
            return true;
        }
        s.mallocs += 1;
        false
    }) {
        return null_mut();
    }
    unsafe { malloc(n) }.cast()
}
pub(super) fn release(p: *mut u8) {
    with(|s| s.frees += 1);
    unsafe { free(p.cast()) };
}

const CLASS_TOKEN: usize = 0x1000;

unsafe extern "system" fn exception_check(_env: *mut JNIEnv) -> bool {
    with(|s| {
        s.order.push("ExceptionCheck");
        s.pending
    })
}
unsafe extern "system" fn find_class(_env: *mut JNIEnv, name: *const c_char) -> jclass {
    with(|s| {
        s.found_class = unsafe { core::ffi::CStr::from_ptr(name) }
            .to_str()
            .unwrap()
            .to_owned();
    });
    CLASS_TOKEN as jclass
}
unsafe extern "system" fn throw_new(_env: *mut JNIEnv, class: jclass, _msg: *const c_char) -> jint {
    with(|s| {
        assert_eq!(class as usize, CLASS_TOKEN);
        s.throws += 1;
    });
    0
}
unsafe extern "system" fn get_array_length(_env: *mut JNIEnv, _a: jarray) -> jsize {
    with(|s| {
        s.order.push("GetArrayLength");
        s.array_len
    })
}
unsafe extern "system" fn get_critical(
    _env: *mut JNIEnv,
    array: jarray,
    _is_copy: *mut jboolean,
) -> *mut c_void {
    with(|s| {
        s.order.push("GetPrimitiveArrayCritical");
        if s.crit_null {
            return null_mut();
        }
        s.crit_gets += 1;
        array.cast()
    })
}
unsafe extern "system" fn release_critical(
    _env: *mut JNIEnv,
    _array: jarray,
    _p: *mut c_void,
    mode: jint,
) {
    with(|s| {
        s.release_mode = Some(mode);
        s.order.push("ReleasePrimitiveArrayCritical");
        s.crit_releases += 1;
    });
}
unsafe extern "system" fn get_byte_region(
    _env: *mut JNIEnv,
    array: jbyteArray,
    start: jsize,
    len: jsize,
    buf: *mut jbyte,
) {
    with(|s| {
        s.order.push("GetByteArrayRegion");
        if s.pending_after_region {
            s.pending = true;
        }
    });
    unsafe {
        core::ptr::copy_nonoverlapping(array.cast::<jbyte>().add(start as usize), buf, len as usize)
    };
}
unsafe extern "system" fn set_long_region(
    _env: *mut JNIEnv,
    array: jlongArray,
    start: jsize,
    len: jsize,
    buf: *const jlong,
) {
    with(|s| {
        s.order.push("SetLongArrayRegion");
        s.long_writes.push((start, len));
    });
    let dst = array.cast::<i64>();
    for i in 0..len as usize {
        unsafe { dst.add(start as usize + i).write(buf.add(i).read()) };
    }
}

unsafe extern "system" fn get_direct_capacity(_env: *mut JNIEnv, _buf: jobject) -> jlong {
    with(|s| {
        s.order.push("GetDirectBufferCapacity");
        s.buffer_capacity
    })
}
unsafe extern "system" fn get_direct_address(_env: *mut JNIEnv, buf: jobject) -> *mut c_void {
    with(|s| {
        s.order.push("GetDirectBufferAddress");
        if s.buffer_address_null {
            null_mut()
        } else {
            buf.cast()
        }
    })
}

/// A `JNIEnv` stub: only the entries a path calls are written, through their addresses,
/// and the interface is never read as a whole.
struct StubEnv {
    _iface: Box<MaybeUninit<JNINativeInterface_>>,
    table: Box<*const JNINativeInterface_>,
}

impl StubEnv {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut iface = Box::new(MaybeUninit::<JNINativeInterface_>::uninit());
        let p = iface.as_mut_ptr();
        // SAFETY: each field is written through its own address.
        unsafe {
            addr_of_mut!((*p).v1_2.ExceptionCheck).write(exception_check);
            addr_of_mut!((*p).v1_2.FindClass).write(find_class);
            addr_of_mut!((*p).v1_2.ThrowNew).write(throw_new);
            addr_of_mut!((*p).v1_2.GetArrayLength).write(get_array_length);
            addr_of_mut!((*p).v1_2.GetPrimitiveArrayCritical).write(get_critical);
            addr_of_mut!((*p).v1_2.ReleasePrimitiveArrayCritical).write(release_critical);
            addr_of_mut!((*p).v1_2.GetByteArrayRegion).write(get_byte_region);
            addr_of_mut!((*p).v1_2.SetLongArrayRegion).write(set_long_region);
            addr_of_mut!((*p).v1_4.GetDirectBufferCapacity).write(get_direct_capacity);
            addr_of_mut!((*p).v1_4.GetDirectBufferAddress).write(get_direct_address);
        }
        let table = Box::new(p.cast_const());
        StubEnv {
            _iface: iface,
            table,
        }
    }
    fn env(&mut self) -> *mut JNIEnv {
        &mut *self.table as *mut *const JNINativeInterface_
    }
}

fn camel(snake: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in snake.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.push(c.to_ascii_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn name_of(n: &Native) -> &str {
    core::str::from_utf8(&n.name[..n.name.len() - 1]).unwrap()
}
fn desc_of(n: &Native) -> &str {
    core::str::from_utf8(&n.descriptor[..n.descriptor.len() - 1]).unwrap()
}

const EXPORTS: [&str; 36] = [
    "cast_bool",
    "cast_char",
    "cast_i8",
    "cast_i16",
    "cast_i32",
    "cast_i64",
    "cast_u8",
    "cast_u16",
    "cast_u32",
    "cast_u64",
    "cast_f32",
    "cast_f64",
    "cast_decimal",
    "cast_uuid",
    "cast_timestamp",
    "cast_unix",
    "cast_excel_serial",
    "cast_date_ordered",
    "cast_datetime",
    "cast_date",
    "cast_time",
    "cast_duration",
    "cast_decimal_from_f64",
    "cast_excel_serial_from_f64",
    "cast_excel_time",
    "cast_excel_duration",
    "hypercast_version",
    "cast_checked",
    "cast_checked_from_f64",
    "cast_constraints_check",
    "cast_text",
    "cast_declared_decimal",
    "cast_detailed",
    "cast_detailed_from_f64",
    "cast_checked_detailed",
    "cast_checked_detailed_from_f64",
];

/// The input as Java hands it: the stub reads `array_len` for its length.
fn text(bytes: &[u8]) -> jbyteArray {
    with(|s| s.array_len = bytes.len() as jsize);
    bytes.as_ptr().cast_mut().cast()
}

/// The format as `NativeCore` packs it: the layout's own bytes, in the platform's order.
fn packed(format: NumFormat) -> [u8; 92] {
    // SAFETY: `RawNumFormat` is 92 bytes of `u32`s and `u8`s with no padding.
    unsafe { core::mem::transmute::<RawNumFormat, [u8; 92]>(RawNumFormat::from(format)) }
}

fn invariant() -> [u8; 92] {
    packed(NumFormat::INVARIANT)
}

/// The day-period layout as `NativeCore` packs it: the size word in the platform's order, then the
/// designators, then two bytes of padding.
fn periods(am: &str, pm: &str) -> [u8; 40] {
    let mut bytes = [0u8; 40];
    bytes[..4].copy_from_slice(&40u32.to_ne_bytes());
    bytes[4..4 + am.len()].copy_from_slice(am.as_bytes());
    bytes[20] = am.len() as u8;
    bytes[21..21 + pm.len()].copy_from_slice(pm.as_bytes());
    bytes[37] = pm.len() as u8;
    bytes
}

const UNTOUCHED: jlong = 0x5A5A;

fn out() -> [jlong; 4] {
    [UNTOUCHED; 4]
}

fn arr(out: &mut [jlong; 4]) -> jlongArray {
    out.as_mut_ptr().cast()
}

#[test]
fn the_table_has_an_entry_per_export_and_jni_natives_reports_it() {
    assert_eq!(NATIVES.len(), EXPORTS.len() + 1);
    assert_eq!(unsafe { native_jni_natives(null_mut(), null_mut()) }, 37);
}

#[test]
fn table_names_are_camel_of_every_export_plus_jni_natives() {
    let mut want: Vec<String> = EXPORTS.iter().map(|e| camel(e)).collect();
    want.push("jniNatives".into());
    want.sort();
    let mut got: Vec<String> = NATIVES.iter().map(|n| name_of(n).to_owned()).collect();
    got.sort();
    assert_eq!(got, want);
}

#[test]
fn table_descriptors_are_pinned() {
    let text = "([B[J)I";
    let numeric = "([B[B[J)I";
    let declared = "([BI[J)I";
    let typed = "(D[J)I";
    let want = [
        ("castBool", text),
        ("castChar", text),
        ("castI8", numeric),
        ("castI16", numeric),
        ("castI32", numeric),
        ("castI64", numeric),
        ("castU8", numeric),
        ("castU16", numeric),
        ("castU32", numeric),
        ("castU64", numeric),
        ("castF32", numeric),
        ("castF64", numeric),
        ("castDecimal", numeric),
        ("castUuid", text),
        ("castTimestamp", text),
        ("castUnix", declared),
        ("castExcelSerial", declared),
        ("castDateOrdered", declared),
        ("castDatetime", "([BI[B[J)I"),
        ("castDate", text),
        ("castTime", text),
        ("castDuration", text),
        ("castDecimalFromF64", typed),
        ("castExcelSerialFromF64", "(DI[J)I"),
        ("castExcelTime", typed),
        ("castExcelDuration", typed),
        ("hypercastVersion", "()I"),
        ("castChecked", "(I[B[BLjava/nio/ByteBuffer;[B[J)I"),
        ("castCheckedFromF64", "(ID[BLjava/nio/ByteBuffer;[J)I"),
        ("castConstraintsCheck", "(I[BLjava/nio/ByteBuffer;)I"),
        ("castText", text),
        ("castDeclaredDecimal", text),
        ("castDetailed", "(II[B[B[BLjava/nio/ByteBuffer;[B[J)I"),
        ("castDetailedFromF64", "(IID[J)I"),
        ("castCheckedDetailed", "(I[B[BLjava/nio/ByteBuffer;[B[J)I"),
        (
            "castCheckedDetailedFromF64",
            "(ID[BLjava/nio/ByteBuffer;[J)I",
        ),
        ("jniNatives", "()I"),
    ];
    assert_eq!(want.len(), NATIVES.len());
    for (name, desc) in want {
        let n = NATIVES.iter().find(|n| name_of(n) == name).expect(name);
        assert_eq!(desc_of(n), desc, "{name}");
    }
}

#[test]
fn names_unique_rejects_a_repeat() {
    let one = NATIVES[0].name;
    let a = Native {
        name: one,
        descriptor: NATIVES[0].descriptor,
        function: null::<c_void>(),
    };
    let b = Native {
        name: one,
        descriptor: NATIVES[1].descriptor,
        function: null::<c_void>(),
    };
    assert!(!names_unique(&[a, b]));
    assert!(names_unique(&NATIVES));
}

/// Calls a text native, returning its code and the four longs Java would read.
fn call_text(
    native: unsafe extern "system" fn(*mut JNIEnv, jclass, jbyteArray, jlongArray) -> jint,
    input: &[u8],
) -> (jint, [jlong; 4]) {
    let mut stub = StubEnv::new();
    let env = stub.env();
    let mut o = out();
    let rc = unsafe { native(env, null_mut(), text(input), arr(&mut o)) };
    (rc, o)
}

fn call_numeric(
    native: unsafe extern "system" fn(
        *mut JNIEnv,
        jclass,
        jbyteArray,
        jbyteArray,
        jlongArray,
    ) -> jint,
    input: &[u8],
    format: &[u8; 92],
) -> (jint, [jlong; 4]) {
    let mut stub = StubEnv::new();
    let env = stub.env();
    let mut o = out();
    let rc = unsafe {
        native(
            env,
            null_mut(),
            text(input),
            format.as_ptr().cast_mut().cast(),
            arr(&mut o),
        )
    };
    (rc, o)
}

#[test]
fn every_text_door_writes_its_words() {
    assert_eq!(
        call_text(native_cast_bool, b"yes"),
        (0, [1, 0, UNTOUCHED, UNTOUCHED])
    );
    assert_eq!(
        call_text(native_cast_char, b"U+00E9"),
        (0, [0xE9, 0, UNTOUCHED, UNTOUCHED])
    );
    let (rc, o) = call_text(native_cast_uuid, b"f1e2d3c4-b5a6-9788-8968-5a4b3c2d1e0f");
    assert_eq!(rc, 0);
    assert_eq!(o[0] as u64, 0xf1e2_d3c4_b5a6_9788);
    assert_eq!(o[1] as u64, 0x8968_5a4b_3c2d_1e0f);
    assert_eq!(
        call_text(native_cast_timestamp, b"1970-01-01T00:00:01.5-00:01"),
        (0, [61, 500_000_000, UNTOUCHED, UNTOUCHED])
    );
    assert_eq!(
        call_text(native_cast_date, b"2026-01-07"),
        (0, [(2026 << 16) | (1 << 8) | 7, 0, UNTOUCHED, UNTOUCHED])
    );
    assert_eq!(
        call_text(native_cast_time, b"15:04:05.000000001"),
        (
            0,
            [
                (15 * 3600 + 4 * 60 + 5) * 1_000_000_000 + 1,
                0,
                UNTOUCHED,
                UNTOUCHED
            ]
        )
    );
    assert_eq!(
        call_text(native_cast_duration, b"-1.5s"),
        (0, [-1, -500_000_000, UNTOUCHED, UNTOUCHED])
    );
}

#[test]
fn every_numeric_door_writes_its_words() {
    let f = invariant();
    assert_eq!(call_numeric(native_cast_i8, b"-128", &f).1[0], -128);
    assert_eq!(call_numeric(native_cast_i16, b"-32,768", &f).1[0], -32768);
    assert_eq!(call_numeric(native_cast_i32, b"(1,234)", &f).1[0], -1234);
    assert_eq!(
        call_numeric(native_cast_i64, b"-9223372036854775808", &f).1[0],
        i64::MIN
    );
    // Unsigned values are zero-extended, never sign-extended.
    assert_eq!(call_numeric(native_cast_u8, b"255", &f).1[0], 255);
    assert_eq!(call_numeric(native_cast_u16, b"65535", &f).1[0], 65535);
    assert_eq!(
        call_numeric(native_cast_u32, b"4294967295", &f).1[0],
        4_294_967_295
    );
    assert_eq!(
        call_numeric(native_cast_u64, b"18446744073709551615", &f).1[0],
        -1
    );
    assert_eq!(
        call_numeric(native_cast_f32, b"1.5", &f).1[0],
        jlong::from(1.5f32.to_bits())
    );
    assert_eq!(
        call_numeric(native_cast_f64, b"25.5%", &f).1[0],
        0.255f64.to_bits() as jlong
    );
    // -1234.50 is 123450 at scale 2, negative: canonical, so 12345 at scale 1.
    let (rc, o) = call_numeric(native_cast_decimal, b"-1,234.50", &f);
    assert_eq!(rc, 0);
    assert_eq!(o[0], 12345);
    assert_eq!(o[1], (1 << 32) | (1 << 40));
}

#[test]
fn a_declared_format_crosses_in_the_92_byte_layout() {
    let declared = packed(NumFormat::new(',', '.', NumFormat::ALL));
    assert_eq!(
        call_numeric(native_cast_i32, b"1.234", &declared),
        (0, [1234, 0, UNTOUCHED, UNTOUCHED])
    );
    assert_eq!(call_numeric(native_cast_i32, b"1.234", &invariant()).0, 2);
}

#[test]
fn every_declared_door_passes_its_code() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    let mut o = out();
    let rc = unsafe { native_cast_unix(env, null_mut(), text(b"1700000000"), 1, arr(&mut o)) };
    assert_eq!((rc, o[0], o[1]), (0, 1_700_000_000, 0));
    let mut o = out();
    let rc =
        unsafe { native_cast_excel_serial(env, null_mut(), text(b"45292.75"), 1, arr(&mut o)) };
    assert_eq!((rc, o[0], o[1]), (0, 1_704_132_000, 0));
    let mut o = out();
    let rc =
        unsafe { native_cast_date_ordered(env, null_mut(), text(b"1/7/2026"), 3, arr(&mut o)) };
    assert_eq!((rc, o[0]), (0, (2026 << 16) | (7 << 8) | 1));
    // An undefined code is the core's contract violation, and nothing is written.
    let mut o = out();
    let rc = unsafe { native_cast_unix(env, null_mut(), text(b"1"), 9, arr(&mut o)) };
    assert_eq!((rc, o), (-1, out()));
}

#[test]
fn the_datetime_door_reads_declared_designators_or_none() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    let input = "1/7/2026 3:04 p. m.".as_bytes();
    let declared = periods("a. m.", "p. m.");
    let mut o = out();
    let rc = unsafe {
        native_cast_datetime(
            env,
            null_mut(),
            text(input),
            2,
            declared.as_ptr().cast_mut().cast(),
            arr(&mut o),
        )
    };
    assert_eq!(rc, 0);
    assert_eq!(o[0], (2026 << 16) | (1 << 8) | 7);
    assert_eq!(o[1], (15 * 3600 + 4 * 60) * 1_000_000_000);
    // The same text with nothing declared is malformed at the designator.
    let mut o = out();
    let rc =
        unsafe { native_cast_datetime(env, null_mut(), text(input), 2, null_mut(), arr(&mut o)) };
    assert_eq!(rc, 2);
    assert_eq!(&o[..2], &[UNTOUCHED, UNTOUCHED]);
    // The core reads the layout it would have read from any other binding.
    let english = periods("", "");
    let mut o = out();
    let rc = unsafe {
        native_cast_datetime(
            env,
            null_mut(),
            text(b"2026-01-07 3:04 PM"),
            1,
            english.as_ptr().cast_mut().cast(),
            arr(&mut o),
        )
    };
    assert_eq!((rc, o[1]), (0, (15 * 3600 + 4 * 60) * 1_000_000_000));
}

#[test]
fn every_typed_door_writes_its_words() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    let mut o = out();
    let rc = unsafe { native_cast_decimal_from_f64(env, null_mut(), 0.1 + 0.2, arr(&mut o)) };
    assert_eq!((rc, o[0], o[1]), (0, 30_000_000_000_000_004, 17 << 32));
    let mut o = out();
    let rc =
        unsafe { native_cast_excel_serial_from_f64(env, null_mut(), 45292.75, 1, arr(&mut o)) };
    assert_eq!(
        (rc, o[0], o[1]),
        (0, (2024 << 16) | (1 << 8) | 1, 18 * 3600 * 1_000_000_000)
    );
    let mut o = out();
    let rc = unsafe { native_cast_excel_time(env, null_mut(), 0.75, arr(&mut o)) };
    assert_eq!((rc, o[0]), (0, 18 * 3600 * 1_000_000_000));
    let mut o = out();
    let rc = unsafe { native_cast_excel_duration(env, null_mut(), -1.5, arr(&mut o)) };
    assert_eq!((rc, o[0], o[1]), (0, -129_600, 0));
    // A typed fault has no span: offset and length 0, in out[2..4] only.
    let mut o = out();
    let rc = unsafe { native_cast_excel_time(env, null_mut(), f64::NAN, arr(&mut o)) };
    assert_eq!((rc, o), (2, [UNTOUCHED, UNTOUCHED, 0, 0]));
}

#[test]
fn a_fault_writes_its_span_and_leaves_the_value_alone() {
    let (rc, o) = call_numeric(native_cast_i32, b"12x4", &invariant());
    assert_eq!((rc, o), (2, [UNTOUCHED, UNTOUCHED, 2, 1]));
    let (rc, o) = call_text(native_cast_bool, b"");
    assert_eq!((rc, o), (1, [UNTOUCHED, UNTOUCHED, 0, 0]));
    with(|s| assert_eq!(s.long_writes, [(2, 2)]));
}

#[test]
fn a_format_the_core_refuses_is_a_contract_violation_and_writes_nothing() {
    let mut bad = invariant();
    bad[0] = 29; // a size under the smallest the core reads
    let (rc, o) = call_numeric(native_cast_i32, b"1", &bad);
    assert_eq!((rc, o), (-1, out()));
    with(|s| assert!(s.long_writes.is_empty()));
}

#[test]
fn the_version_is_the_cores() {
    assert_eq!(
        unsafe { native_hypercast_version(null_mut(), null_mut()) } as u32,
        hypercast::hypercast_version()
    );
}

#[test]
fn the_critical_region_closes_before_any_other_call() {
    let (rc, _) = call_numeric(native_cast_i32, b"42", &invariant());
    assert_eq!(rc, 0);
    with(|s| {
        assert_eq!(
            s.order,
            [
                "GetByteArrayRegion",
                "ExceptionCheck",
                "GetArrayLength",
                "GetPrimitiveArrayCritical",
                "ReleasePrimitiveArrayCritical",
                "SetLongArrayRegion"
            ]
        );
        assert_eq!(s.release_mode, Some(JNI_ABORT));
    });
}

#[test]
fn an_input_of_exactly_the_bound_pins_and_above_it_is_copied() {
    let mut input = vec![b' '; CRITICAL_MAX];
    input[0] = b'7';
    assert_eq!(call_numeric(native_cast_i32, &input, &invariant()).1[0], 7);
    with(|s| {
        assert_eq!(
            (s.crit_gets, s.crit_releases, s.mallocs, s.frees),
            (1, 1, 0, 0)
        )
    });
    input.push(b' ');
    assert_eq!(call_numeric(native_cast_i32, &input, &invariant()).1[0], 7);
    with(|s| {
        assert_eq!((s.crit_gets, s.mallocs, s.frees), (0, 1, 1));
        assert!(!s.order.contains(&"GetPrimitiveArrayCritical"));
    });
}

#[test]
fn a_copied_input_faults_with_the_same_span() {
    let mut input = vec![b' '; CRITICAL_MAX + 10];
    input[CRITICAL_MAX + 3] = b'x';
    let (rc, o) = call_text(native_cast_bool, &input);
    assert_eq!((rc, o[2], o[3]), (2, (CRITICAL_MAX + 3) as jlong, 1));
}

#[test]
fn a_zero_length_passes_null_and_pins_nothing() {
    let (rc, _) = call_text(native_cast_bool, b"");
    assert_eq!(rc, 1);
    with(|s| assert_eq!((s.crit_gets, s.mallocs), (0, 0)));
}

#[test]
fn a_null_critical_with_a_pending_exception_throws_nothing_more() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    with(|s| {
        s.crit_null = true;
        s.pending = true;
    });
    let mut o = out();
    let rc = unsafe { native_cast_bool(env, null_mut(), text(b"yes"), arr(&mut o)) };
    assert_eq!((rc, o), (PENDING, out()));
    with(|s| assert_eq!((s.throws, s.crit_releases), (0, 0)));
}

#[test]
fn a_null_critical_without_an_exception_throws_out_of_memory() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    with(|s| s.crit_null = true);
    let mut o = out();
    let rc = unsafe { native_cast_bool(env, null_mut(), text(b"yes"), arr(&mut o)) };
    assert_eq!((rc, o), (PENDING, out()));
    with(|s| {
        assert_eq!(s.throws, 1);
        assert_eq!(s.found_class, "java/lang/OutOfMemoryError");
    });
}

#[test]
fn a_null_malloc_throws_out_of_memory_and_frees_nothing() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    with(|s| s.malloc_null = true);
    let input = vec![b'1'; CRITICAL_MAX + 1];
    let mut o = out();
    let rc = unsafe { native_cast_bool(env, null_mut(), text(&input), arr(&mut o)) };
    assert_eq!((rc, o), (PENDING, out()));
    with(|s| assert_eq!((s.throws, s.frees), (1, 0)));
}

#[test]
fn an_exception_after_the_input_copy_frees_and_returns_pending() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    with(|s| s.pending_after_region = true);
    let input = vec![b'1'; CRITICAL_MAX + 1];
    let mut o = out();
    let rc = unsafe { native_cast_bool(env, null_mut(), text(&input), arr(&mut o)) };
    assert_eq!((rc, o), (PENDING, out()));
    with(|s| assert_eq!((s.mallocs, s.frees, s.throws), (1, 1, 0)));
}

#[test]
fn an_exception_reading_the_format_pins_nothing_and_returns_pending() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    with(|s| s.pending_after_region = true);
    let format = invariant();
    let mut o = out();
    let rc = unsafe {
        native_cast_i32(
            env,
            null_mut(),
            text(b"1"),
            format.as_ptr().cast_mut().cast(),
            arr(&mut o),
        )
    };
    assert_eq!((rc, o), (PENDING, out()));
    with(|s| assert_eq!((s.crit_gets, s.mallocs), (0, 0)));
}

#[test]
fn an_exception_reading_the_designators_pins_nothing_and_returns_pending() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    with(|s| s.pending_after_region = true);
    let declared = periods("am", "pm");
    let mut o = out();
    let rc = unsafe {
        native_cast_datetime(
            env,
            null_mut(),
            text(b"2026-01-07"),
            1,
            declared.as_ptr().cast_mut().cast(),
            arr(&mut o),
        )
    };
    assert_eq!((rc, o), (PENDING, out()));
    with(|s| assert_eq!(s.crit_gets, 0));
}

#[test]
fn a_core_fault_on_a_pinned_input_still_releases() {
    let (rc, _) = call_text(native_cast_bool, b"maybe");
    assert_eq!(rc, 2);
    with(|s| assert_eq!((s.crit_gets, s.crit_releases), (1, 1)));
}

// --- the validation constraints ---

/// The 72-byte constraints layout as `NativeCore` packs it, in the platform's order: the
/// five words, the reserved word, then the three decimals, each `{lo, hi, scale, sign}` and
/// two bytes of padding.
fn wire(raw: &RawConstraints) -> [u8; 72] {
    let mut bytes = [0u8; 72];
    let words = [
        raw.size,
        raw.flags,
        raw.rounding,
        raw.min_length,
        raw.max_length,
        raw._reserved,
    ];
    for (i, word) in words.iter().enumerate() {
        bytes[i * 4..i * 4 + 4].copy_from_slice(&word.to_ne_bytes());
    }
    for (at, d) in [(24, raw.lower), (40, raw.upper), (56, raw.multiple_of)] {
        bytes[at..at + 16].copy_from_slice(&packed_decimal(d));
    }
    bytes
}

fn packed_decimal(d: Decimal) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&d.lo.to_ne_bytes());
    bytes[8..12].copy_from_slice(&d.hi.to_ne_bytes());
    bytes[12] = d.scale;
    bytes[13] = u8::from(d.negative);
    bytes
}

fn whole(value: u64, negative: bool) -> Decimal {
    Decimal {
        lo: value,
        hi: 0,
        scale: 0,
        negative,
    }
}

fn declared(flags: u32) -> RawConstraints {
    RawConstraints {
        size: RawConstraints::SIZE,
        flags,
        rounding: 0,
        min_length: 0,
        max_length: 0,
        _reserved: 0,
        lower: whole(0, false),
        upper: whole(0, false),
        multiple_of: whole(0, false),
    }
}

/// `minimum: 10, multipleOf: 5` under `rounding`.
fn ten_by_five(rounding: u32) -> [u8; 72] {
    let mut raw = declared(RawConstraints::HAS_MINIMUM | RawConstraints::HAS_MULTIPLE_OF);
    raw.lower = whole(10, false);
    raw.multiple_of = whole(5, false);
    raw.rounding = rounding;
    wire(&raw)
}

/// The text `enum` packed as `NativeCore` packs it: a little-endian length, then the bytes.
fn text_entries(entries: &[&str]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for entry in entries {
        bytes.extend_from_slice(&(entry.len() as u32).to_le_bytes());
        bytes.extend_from_slice(entry.as_bytes());
    }
    bytes
}

/// The `enum` list as the direct `ByteBuffer` Java hands over: its capacity set on the stub,
/// its address the token the stub hands back.
fn buffer(bytes: &[u8]) -> jobject {
    with(|s| s.buffer_capacity = bytes.len() as jlong);
    bytes.as_ptr().cast_mut().cast()
}

const NO_ENTRY: jlong = u32::MAX as jlong;

fn checked_out() -> [jlong; 8] {
    [UNTOUCHED; 8]
}

fn call_checked(
    door: u32,
    format: Option<&[u8; 92]>,
    constraints: &[u8; 72],
    entries: Option<&[u8]>,
    input: &[u8],
) -> (jint, [jlong; 8]) {
    let mut stub = StubEnv::new();
    let env = stub.env();
    let mut o = checked_out();
    let entries = entries.map_or(null_mut(), buffer);
    let rc = unsafe {
        native_cast_checked(
            env,
            null_mut(),
            door as jint,
            format.map_or(null_mut(), |f| f.as_ptr().cast_mut().cast()),
            constraints.as_ptr().cast_mut().cast(),
            entries,
            text(input),
            o.as_mut_ptr().cast(),
        )
    };
    (rc, o)
}

#[test]
fn a_checked_success_writes_its_value_and_its_check() {
    let i32_door = Door::I32.code();
    let f = invariant();
    let (rc, o) = call_checked(i32_door, Some(&f), &ten_by_five(1), None, b"12");
    assert_eq!(rc, 0);
    assert_eq!(o[..2], [10, 0]);
    assert_eq!(o[2..4], [UNTOUCHED, UNTOUCHED]);
    assert_eq!(o[4..7], [0, 1, NO_ENTRY]);
    assert_eq!(o[7], UNTOUCHED);
    let (rc, o) = call_checked(i32_door, Some(&f), &ten_by_five(0), None, b"15");
    assert_eq!(
        (rc, &o[..2], &o[4..7]),
        (0, &[15, 0][..], &[0, 0, NO_ENTRY][..])
    );
}

#[test]
fn a_checked_door_writes_the_words_of_its_plain_door() {
    let f = invariant();
    let none = wire(&declared(0));
    let words = |door: Door, input: &[u8]| {
        let (rc, o) = call_checked(door.code(), Some(&f), &none, None, input);
        assert_eq!(rc, 0, "{door:?}");
        [o[0], o[1]]
    };
    assert_eq!(words(Door::I8, b"-128"), [-128, 0]);
    assert_eq!(words(Door::I16, b"-32768"), [-32768, 0]);
    assert_eq!(words(Door::I64, b"-9223372036854775808"), [i64::MIN, 0]);
    assert_eq!(words(Door::U8, b"255"), [255, 0]);
    assert_eq!(words(Door::U16, b"65535"), [65535, 0]);
    assert_eq!(words(Door::U32, b"4294967295"), [4_294_967_295, 0]);
    assert_eq!(words(Door::U64, b"18446744073709551615"), [-1, 0]);
    assert_eq!(words(Door::F32, b"1.5"), [jlong::from(1.5f32.to_bits()), 0]);
    assert_eq!(words(Door::F64, b"0.25"), [0.25f64.to_bits() as jlong, 0]);
    assert_eq!(
        words(Door::Decimal, b"-1234.5"),
        [12345, (1 << 32) | (1 << 40)]
    );
    // Text ignores the format, and its value is the span of the input.
    assert_eq!(words(Door::Text, b"Male"), [0, 4]);
}

#[test]
fn a_violation_writes_its_span_and_its_keyword() {
    let (rc, o) = call_checked(Door::I32.code(), None, &ten_by_five(0), None, b"12");
    assert_eq!(rc, 4);
    assert_eq!(o[..4], [UNTOUCHED, UNTOUCHED, 0, 2]);
    assert_eq!(o[4..7], [5, 0, NO_ENTRY]);
    let (rc, o) = call_checked(Door::I32.code(), None, &ten_by_five(0), None, b"5");
    assert_eq!((rc, &o[2..7]), (4, &[0, 1, 1, 0, NO_ENTRY][..]));
}

#[test]
fn a_checked_fault_writes_its_span_and_no_keyword() {
    let (rc, o) = call_checked(Door::I32.code(), None, &ten_by_five(0), None, b"1x");
    assert_eq!(rc, 2);
    assert_eq!(o[..7], [UNTOUCHED, UNTOUCHED, 1, 1, 0, 0, NO_ENTRY]);
}

#[test]
fn a_text_enum_crosses_as_a_direct_buffer_and_reports_its_index() {
    let mut raw = declared(RawConstraints::TRIM | RawConstraints::HAS_ENUM);
    raw.rounding = 0;
    let constraints = wire(&raw);
    let entries = text_entries(&["Female", "Male", "Other"]);
    let (rc, o) = call_checked(
        Door::Text.code(),
        None,
        &constraints,
        Some(&entries),
        b" MALE ",
    );
    assert_eq!(rc, 0);
    assert_eq!(o[..2], [1, 4]);
    assert_eq!(o[4..7], [0, 0, 1]);
    // Every layout and the list's address are read before the input is pinned.
    with(|s| {
        assert_eq!(
            s.order,
            [
                "GetByteArrayRegion",
                "ExceptionCheck",
                "GetDirectBufferCapacity",
                "GetDirectBufferAddress",
                "GetArrayLength",
                "GetPrimitiveArrayCritical",
                "ReleasePrimitiveArrayCritical",
                "SetLongArrayRegion",
                "SetLongArrayRegion",
            ]
        );
        assert_eq!((s.crit_gets, s.crit_releases), (1, 1));
    });
    let (rc, o) = call_checked(
        Door::Text.code(),
        None,
        &constraints,
        Some(&entries),
        b"Femal",
    );
    assert_eq!((rc, &o[2..7]), (4, &[0, 5, 8, 0, NO_ENTRY][..]));
}

#[test]
fn an_integer_enum_crosses_in_native_order() {
    let mut entries = Vec::new();
    entries.extend_from_slice(&packed_decimal(whole(10, false)));
    entries.extend_from_slice(&packed_decimal(whole(20, true)));
    let listed = wire(&declared(RawConstraints::HAS_ENUM));
    let (rc, o) = call_checked(Door::I32.code(), None, &listed, Some(&entries), b"-20");
    assert_eq!((rc, o[0], &o[4..7]), (0, -20, &[0, 0, NO_ENTRY][..]));
    let (rc, o) = call_checked(Door::I32.code(), None, &listed, Some(&entries), b"20");
    assert_eq!((rc, o[4]), (4, 8));
}

#[test]
fn an_enum_buffer_with_no_address_is_a_contract_violation_and_writes_nothing() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    with(|s| s.buffer_address_null = true);
    let entries = text_entries(&["a"]);
    let constraints = wire(&declared(RawConstraints::HAS_ENUM));
    let mut o = checked_out();
    let rc = unsafe {
        native_cast_checked(
            env,
            null_mut(),
            Door::Text.code() as jint,
            null_mut(),
            constraints.as_ptr().cast_mut().cast(),
            buffer(&entries),
            text(b"a"),
            o.as_mut_ptr().cast(),
        )
    };
    assert_eq!((rc, o), (-1, checked_out()));
    with(|s| assert_eq!((s.crit_gets, s.long_writes.len()), (0, 0)));
    let rc = unsafe {
        native_cast_constraints_check(
            env,
            null_mut(),
            Door::Text.code() as jint,
            constraints.as_ptr().cast_mut().cast(),
            buffer(&entries),
        )
    };
    assert_eq!(rc, -1);
}

#[test]
fn a_declaration_the_core_refuses_is_a_contract_violation_at_the_checked_door() {
    let mut raw = declared(RawConstraints::HAS_MIN_LENGTH);
    raw.min_length = 1;
    let (rc, o) = call_checked(Door::I32.code(), None, &wire(&raw), None, b"1");
    assert_eq!((rc, o), (-1, checked_out()));
}

#[test]
fn an_exception_reading_the_constraints_pins_nothing_and_returns_pending() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    with(|s| s.pending_after_region = true);
    let constraints = ten_by_five(0);
    let mut o = checked_out();
    let rc = unsafe {
        native_cast_checked(
            env,
            null_mut(),
            Door::I32.code() as jint,
            null_mut(),
            constraints.as_ptr().cast_mut().cast(),
            null_mut(),
            text(b"1"),
            o.as_mut_ptr().cast(),
        )
    };
    assert_eq!((rc, o), (PENDING, checked_out()));
    with(|s| assert_eq!((s.crit_gets, s.mallocs), (0, 0)));
}

#[test]
fn a_large_input_is_copied_while_the_list_stays_in_place() {
    let mut input = vec![b' '; CRITICAL_MAX + 1];
    input[..4].copy_from_slice(b"Male");
    let entries = text_entries(&["Female", "Male"]);
    let (rc, o) = call_checked(
        Door::Text.code(),
        None,
        &wire(&declared(RawConstraints::TRIM | RawConstraints::HAS_ENUM)),
        Some(&entries),
        &input,
    );
    assert_eq!((rc, &o[..2], o[6]), (0, &[0, 4][..], 1));
    with(|s| assert_eq!((s.crit_gets, s.mallocs, s.frees), (0, 1, 1)));
}

#[test]
fn the_checked_door_over_a_double_writes_its_value_and_its_check() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    let mut raw = declared(RawConstraints::HAS_MULTIPLE_OF);
    raw.multiple_of = Decimal {
        lo: 1,
        hi: 0,
        scale: 1,
        negative: false,
    };
    raw.rounding = 1;
    let constraints = wire(&raw);
    let mut o = checked_out();
    let rc = unsafe {
        native_cast_checked_from_f64(
            env,
            null_mut(),
            Door::F64.code() as jint,
            0.1 + 0.2,
            constraints.as_ptr().cast_mut().cast(),
            null_mut(),
            o.as_mut_ptr().cast(),
        )
    };
    assert_eq!(rc, 0);
    assert_eq!(o[0], 0.3f64.to_bits() as jlong);
    assert_eq!(o[4..7], [0, 1, NO_ENTRY]);
    raw.rounding = 0;
    let constraints = wire(&raw);
    let mut o = checked_out();
    let rc = unsafe {
        native_cast_checked_from_f64(
            env,
            null_mut(),
            Door::F64.code() as jint,
            0.1 + 0.2,
            constraints.as_ptr().cast_mut().cast(),
            null_mut(),
            o.as_mut_ptr().cast(),
        )
    };
    assert_eq!((rc, &o[2..7]), (4, &[0, 0, 5, 0, NO_ENTRY][..]));
}

#[test]
fn a_declaration_is_judged_by_the_core() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    let check = |door: Door, raw: &RawConstraints, entries: Option<&[u8]>| unsafe {
        native_cast_constraints_check(
            env,
            null_mut(),
            door.code() as jint,
            wire(raw).as_ptr().cast_mut().cast(),
            entries.map_or(null_mut(), buffer),
        )
    };
    assert_eq!(check(Door::I32, &declared(0), None), 0);
    let mut raw = declared(RawConstraints::HAS_MINIMUM);
    raw.lower = whole(1, true);
    assert_eq!(check(Door::U8, &raw, None), 8);
    assert_eq!(check(Door::I8, &raw, None), 0);
    let duplicates = text_entries(&["I", "i"]);
    let listed = declared(RawConstraints::HAS_ENUM);
    assert_eq!(check(Door::Text, &listed, Some(&duplicates)), 10);
    // A list is read only under HAS_ENUM: without it, malformed; with it and no entries, 12.
    assert_eq!(check(Door::Text, &declared(0), Some(&duplicates)), 11);
    assert_eq!(check(Door::Text, &listed, None), 12);
    assert_eq!(check(Door::I32, &listed, Some(&[][..])), 12);
    assert_eq!(check(Door::Text, &listed, Some(&text_entries(&[""]))), 13);
    let mut malformed = declared(0);
    malformed.size = 71;
    assert_eq!(check(Door::I32, &malformed, None), 11);
}

#[test]
fn the_text_door_writes_the_span_of_valid_utf8() {
    assert_eq!(
        call_text(native_cast_text, "héllo".as_bytes()),
        (0, [0, 6, UNTOUCHED, UNTOUCHED])
    );
    assert_eq!(
        call_text(native_cast_text, b"ab\xFFc"),
        (2, [UNTOUCHED, UNTOUCHED, 2, 1])
    );
    assert_eq!(
        call_text(native_cast_text, b""),
        (1, [UNTOUCHED, UNTOUCHED, 0, 0])
    );
}

#[test]
fn a_declared_decimal_is_the_cores_conversion() {
    let (rc, o) = call_text(native_cast_declared_decimal, b"-1.50");
    assert_eq!((rc, o[0], o[1]), (0, 15, (1 << 32) | (1 << 40)));
    let (rc, o) = call_text(native_cast_declared_decimal, b"1e-6");
    assert_eq!((rc, o[0], o[1]), (0, 1, 6 << 32));
    let (rc, o) = call_text(native_cast_declared_decimal, b"1e-29");
    assert_eq!((rc, &o[..2]), (8, &[UNTOUCHED, UNTOUCHED][..]));
    let (rc, _) = call_text(native_cast_declared_decimal, b"0.1x");
    assert_eq!(rc, 8);
}

// --- the detailed exports ---

/// `NumFormat::INVARIANT` under a strict grouping policy, in the 92-byte layout.
fn strict(policy: u32) -> [u8; 92] {
    packed(NumFormat::INVARIANT.with_grouping(policy))
}

/// The 12-byte `RawDateOptions` as `NativeCore` packs it, in the platform's order.
fn date_options(flags: u32, pivot: u32) -> [u8; 12] {
    let mut bytes = [0u8; 12];
    bytes[..4].copy_from_slice(&12u32.to_ne_bytes());
    bytes[4..8].copy_from_slice(&flags.to_ne_bytes());
    bytes[8..].copy_from_slice(&pivot.to_ne_bytes());
    bytes
}

/// A month vocabulary as `NativeCore` packs it: each entry a month byte, a little-endian
/// length, then the bytes.
fn vocabulary(entries: &[(u8, &str)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (month, name) in entries {
        bytes.push(*month);
        bytes.extend_from_slice(&(name.len() as u32).to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
    }
    bytes
}

const ENGLISH_MONTHS: [(u8, &str); 12] = [
    (1, "Jan"),
    (2, "Feb"),
    (3, "Mar"),
    (4, "Apr"),
    (5, "May"),
    (6, "Jun"),
    (7, "Jul"),
    (8, "Aug"),
    (9, "Sep"),
    (10, "Oct"),
    (11, "Nov"),
    (12, "Dec"),
];

#[derive(Default)]
struct Declared<'a> {
    format: Option<&'a [u8; 92]>,
    periods: Option<&'a [u8; 40]>,
    options: Option<&'a [u8; 12]>,
    vocab: Option<&'a [u8]>,
}

fn call_detailed(door: Door, declared: &Declared, input: &[u8]) -> (jint, [jlong; 8]) {
    let mut stub = StubEnv::new();
    let env = stub.env();
    let mut o = checked_out();
    let param = match door {
        Door::Unix(p) => p as jint,
        Door::DateOrdered(order) | Door::DateTime(order) => order as jint,
        Door::ExcelSerial(epoch) => epoch as jint,
        _ => 0,
    };
    let vocab = declared.vocab.map_or(null_mut(), buffer);
    let rc = unsafe {
        native_cast_detailed(
            env,
            null_mut(),
            door.code() as jint,
            param,
            declared
                .format
                .map_or(null_mut(), |f| f.as_ptr().cast_mut().cast()),
            declared
                .periods
                .map_or(null_mut(), |f| f.as_ptr().cast_mut().cast()),
            declared
                .options
                .map_or(null_mut(), |f| f.as_ptr().cast_mut().cast()),
            vocab,
            text(input),
            o.as_mut_ptr().cast(),
        )
    };
    (rc, o)
}

#[test]
fn every_detailed_text_door_writes_the_words_of_its_plain_native() {
    use hypercast::{DateOrder, ExcelEpoch, UnixPrecision};
    let none = Declared::default();
    let words = |door: Door, input: &[u8]| {
        let (rc, o) = call_detailed(door, &none, input);
        assert_eq!(rc, 0, "{door:?}");
        assert_eq!(
            o[2..],
            [UNTOUCHED; 6],
            "{door:?}: only the value is written"
        );
        [o[0], o[1]]
    };
    assert_eq!(words(Door::Bool, b"yes"), [1, 0]);
    assert_eq!(words(Door::Char, b"U+00E9"), [0xE9, 0]);
    assert_eq!(words(Door::I8, b"-128"), [-128, 0]);
    assert_eq!(words(Door::I16, b"-32,768"), [-32768, 0]);
    assert_eq!(words(Door::I32, b"(1,234)"), [-1234, 0]);
    assert_eq!(words(Door::I64, b"-9223372036854775808"), [i64::MIN, 0]);
    assert_eq!(words(Door::U8, b"255"), [255, 0]);
    assert_eq!(words(Door::U16, b"65535"), [65535, 0]);
    assert_eq!(words(Door::U32, b"4294967295"), [4_294_967_295, 0]);
    assert_eq!(words(Door::U64, b"18446744073709551615"), [-1, 0]);
    assert_eq!(words(Door::F32, b"1.5"), [jlong::from(1.5f32.to_bits()), 0]);
    assert_eq!(words(Door::F64, b"25.5%"), [0.255f64.to_bits() as jlong, 0]);
    assert_eq!(
        words(Door::Decimal, b"-1,234.50"),
        [12345, (1 << 32) | (1 << 40)]
    );
    let uuid = words(Door::Uuid, b"f1e2d3c4-b5a6-9788-8968-5a4b3c2d1e0f");
    assert_eq!(
        uuid.map(|w| w as u64),
        [0xf1e2_d3c4_b5a6_9788, 0x8968_5a4b_3c2d_1e0f]
    );
    assert_eq!(
        words(Door::Timestamp, b"1970-01-01T00:00:01.5-00:01"),
        [61, 500_000_000]
    );
    assert_eq!(
        words(Door::Unix(UnixPrecision::Seconds), b"1700000000"),
        [1_700_000_000, 0]
    );
    assert_eq!(
        words(Door::ExcelSerial(ExcelEpoch::Y1900), b"45292.75"),
        [1_704_132_000, 0]
    );
    assert_eq!(
        words(Door::Date, b"2026-01-07"),
        [(2026 << 16) | (1 << 8) | 7, 0]
    );
    assert_eq!(
        words(Door::DateOrdered(DateOrder::DayMonthYear), b"1/7/2026"),
        [(2026 << 16) | (7 << 8) | 1, 0]
    );
    assert_eq!(
        words(
            Door::DateTime(DateOrder::YearMonthDay),
            b"2026-01-07 3:04 PM"
        ),
        [
            (2026 << 16) | (1 << 8) | 7,
            (15 * 3600 + 4 * 60) * 1_000_000_000
        ]
    );
    assert_eq!(
        words(Door::Time, b"15:04:05.000000001"),
        [(15 * 3600 + 4 * 60 + 5) * 1_000_000_000 + 1, 0]
    );
    assert_eq!(words(Door::Duration, b"-1.5s"), [-1, -500_000_000]);
    // Text's value is the span of the input.
    assert_eq!(words(Door::Text, b"Male"), [0, 4]);
}

#[test]
fn a_detailed_fault_writes_its_span_and_its_detail_to_the_last_word() {
    let f = invariant();
    let declared = Declared {
        format: Some(&f),
        ..Declared::default()
    };
    let (rc, o) = call_detailed(Door::I32, &declared, b"12x4");
    assert_eq!(rc, 2);
    // unexpected_character is detail 2.
    assert_eq!(
        o,
        [
            UNTOUCHED, UNTOUCHED, 2, 1, UNTOUCHED, UNTOUCHED, UNTOUCHED, 2
        ]
    );
    with(|s| assert_eq!(s.long_writes, [(2, 2), (7, 1)]));
    let (rc, o) = call_detailed(Door::Bool, &Declared::default(), b"");
    assert_eq!((rc, o[2], o[3], o[7]), (1, 0, 0, 1));
}

#[test]
fn the_declared_grouping_policy_decides_the_verdict_and_the_detail() {
    for (policy, text, rc, span, detail) in [
        (
            NumFormat::GROUPING_LEGACY,
            &b"12,34"[..],
            0,
            [UNTOUCHED, UNTOUCHED],
            UNTOUCHED,
        ),
        // group_size is detail 11, spanning the group with its preceding separator.
        (NumFormat::GROUPING_WESTERN, &b"12,34"[..], 2, [2, 3], 11),
        (
            NumFormat::GROUPING_INDIAN,
            &b"1,23,456"[..],
            0,
            [UNTOUCHED, UNTOUCHED],
            UNTOUCHED,
        ),
        (NumFormat::GROUPING_INDIAN, &b"1,234,567"[..], 2, [1, 4], 11),
    ] {
        let f = strict(policy);
        let declared = Declared {
            format: Some(&f),
            ..Declared::default()
        };
        let (got, o) = call_detailed(Door::I64, &declared, text);
        assert_eq!(
            (got, [o[2], o[3]], o[7]),
            (rc, span, detail),
            "{policy:#x} {text:?}"
        );
    }
}

#[test]
fn a_format_the_detailed_door_refuses_is_a_contract_violation_and_writes_nothing() {
    // Policy 3 and an undefined flag bit: 1.1's check, which only the detailed exports make.
    for flags in [3u32 << 8 | NumFormat::ALL, 1 << 10 | NumFormat::ALL] {
        let mut bad = invariant();
        bad[4..8].copy_from_slice(&flags.to_ne_bytes());
        let declared = Declared {
            format: Some(&bad),
            ..Declared::default()
        };
        let (rc, o) = call_detailed(Door::I32, &declared, b"1");
        assert_eq!((rc, o), (-1, checked_out()), "{flags:#x}");
        with(|s| assert!(s.long_writes.is_empty()));
    }
    // The 1.0 native still ignores them, and parses legacy.
    let mut stray = invariant();
    stray[4..8].copy_from_slice(&(3u32 << 8 | NumFormat::ALL).to_ne_bytes());
    assert_eq!(call_numeric(native_cast_i32, b"12,34", &stray).0, 0);
}

#[test]
fn a_door_and_a_parameter_outside_the_export_are_refused_and_write_nothing() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    for (door, param) in [(0, 0), (24, 0), (4, 1), (14, 0), (14, 5), (20, 4)] {
        let mut o = checked_out();
        let rc = unsafe {
            native_cast_detailed(
                env,
                null_mut(),
                door,
                param,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
                text(b"1"),
                o.as_mut_ptr().cast(),
            )
        };
        assert_eq!((rc, o), (-1, checked_out()), "door {door} param {param}");
    }
    // 18 (text) is a text door, not a typed one.
    let mut o = checked_out();
    let rc = unsafe {
        native_cast_detailed_from_f64(env, null_mut(), 18, 0, 1.0, o.as_mut_ptr().cast())
    };
    assert_eq!((rc, o), (-1, checked_out()));
}

#[test]
fn date_options_and_a_vocabulary_cross_beside_the_text() {
    use hypercast::DateOrder;
    let door = Door::DateOrdered(DateOrder::DayMonthYear);
    let options = date_options(0, 2029);
    let vocab = vocabulary(&ENGLISH_MONTHS);
    let declared = Declared {
        options: Some(&options),
        vocab: Some(&vocab),
        ..Declared::default()
    };
    let (rc, o) = call_detailed(door, &declared, b"07-Jan-26");
    assert_eq!((rc, o[0]), (0, (2026 << 16) | (1 << 8) | 7));
    // The vocabulary alone opens names and nothing else: no pivot, so the two-digit year is
    // field_width (detail 15) at its digits.
    let declared = Declared {
        vocab: Some(&vocab),
        ..Declared::default()
    };
    let (rc, o) = call_detailed(door, &declared, b"07-Jan-26");
    assert_eq!((rc, o[2], o[3], o[7]), (2, 7, 2, 15));
    // The options alone, without the vocabulary, leave a name unrecognized.
    let declared = Declared {
        options: Some(&options),
        ..Declared::default()
    };
    let (rc, o) = call_detailed(door, &declared, b"07-Jan-26");
    assert_eq!((rc, o[7]), (2, 3));
    // COMPACT_DATE at the date door.
    let compact = date_options(1, 0);
    let declared = Declared {
        options: Some(&compact),
        ..Declared::default()
    };
    let (rc, o) = call_detailed(Door::Date, &declared, b"20260107");
    assert_eq!((rc, o[0]), (0, (2026 << 16) | (1 << 8) | 7));
    // A layout the core refuses: an undefined option bit, a size past its own, a bad pivot.
    for bad in [date_options(8, 0), date_options(0, 99), {
        let mut longer = date_options(0, 0);
        longer[..4].copy_from_slice(&16u32.to_ne_bytes());
        longer
    }] {
        let declared = Declared {
            options: Some(&bad),
            ..Declared::default()
        };
        let (rc, o) = call_detailed(Door::Date, &declared, b"2026-01-07");
        assert_eq!((rc, o), (-1, checked_out()));
    }
}

#[test]
fn a_vocabulary_the_core_refuses_or_a_buffer_with_no_address_is_a_contract_violation() {
    use hypercast::DateOrder;
    let door = Door::DateOrdered(DateOrder::DayMonthYear);
    // Eleven months only.
    let vocab = vocabulary(&ENGLISH_MONTHS[..11]);
    let declared = Declared {
        vocab: Some(&vocab),
        ..Declared::default()
    };
    let (rc, o) = call_detailed(door, &declared, b"07-Jan-2026");
    assert_eq!((rc, o), (-1, checked_out()));
    // A buffer with no address (not direct) is refused before any core call.
    let vocab = vocabulary(&ENGLISH_MONTHS);
    let mut stub = StubEnv::new();
    let env = stub.env();
    let mut o = checked_out();
    let list = buffer(&vocab);
    with(|s| s.buffer_address_null = true);
    let rc = unsafe {
        native_cast_detailed(
            env,
            null_mut(),
            door.code() as jint,
            3,
            null_mut(),
            null_mut(),
            null_mut(),
            list,
            text(b"07-Jan-2026"),
            o.as_mut_ptr().cast(),
        )
    };
    assert_eq!((rc, o), (-1, checked_out()));
    with(|s| assert_eq!(s.crit_gets, 0));
}

#[test]
fn an_exception_reading_the_date_options_pins_nothing_and_returns_pending() {
    let options = date_options(0, 0);
    let mut stub = StubEnv::new();
    let env = stub.env();
    with(|s| s.pending_after_region = true);
    let mut o = checked_out();
    let rc = unsafe {
        native_cast_detailed(
            env,
            null_mut(),
            Door::Date.code() as jint,
            0,
            null_mut(),
            null_mut(),
            options.as_ptr().cast_mut().cast(),
            null_mut(),
            text(b"2026-01-07"),
            o.as_mut_ptr().cast(),
        )
    };
    assert_eq!((rc, o), (PENDING, checked_out()));
    with(|s| assert_eq!((s.crit_gets, s.crit_releases), (0, 0)));
}

#[test]
fn a_large_detailed_input_is_copied_and_faults_with_the_same_span() {
    let mut big = vec![b' '; CRITICAL_MAX + 1];
    big.extend_from_slice(b"12x4");
    let f = invariant();
    let declared = Declared {
        format: Some(&f),
        ..Declared::default()
    };
    let (rc, o) = call_detailed(Door::I32, &declared, &big);
    assert_eq!((rc, o[2], o[3], o[7]), (2, CRITICAL_MAX as jlong + 3, 1, 2));
    with(|s| assert_eq!((s.mallocs, s.frees, s.crit_gets), (1, 1, 0)));
}

#[test]
fn every_detailed_typed_door_writes_its_words_and_a_fault_has_no_span() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    let from = |door: i32, param: i32, value: f64| {
        let mut o = checked_out();
        let rc = unsafe {
            native_cast_detailed_from_f64(
                env,
                null_mut(),
                door,
                param,
                value,
                o.as_mut_ptr().cast(),
            )
        };
        (rc, o)
    };
    let (rc, o) = from(19, 0, 0.1 + 0.2);
    assert_eq!((rc, o[0], o[1]), (0, 30_000_000_000_000_004, 17 << 32));
    let (rc, o) = from(22, 1, 45292.75);
    assert_eq!(
        (rc, o[0], o[1]),
        (0, (2024 << 16) | (1 << 8) | 1, 18 * 3600 * 1_000_000_000)
    );
    let (rc, o) = from(16, 0, 0.75);
    assert_eq!((rc, o[0]), (0, 18 * 3600 * 1_000_000_000));
    let (rc, o) = from(17, 0, -1.5);
    assert_eq!((rc, o[0], o[1]), (0, -129_600, 0));
    // not_finite is detail 22, with an empty span in out[2..4].
    let (rc, o) = from(16, 0, f64::NAN);
    assert_eq!((rc, o[2], o[3], o[7]), (2, 0, 0, 22));
    assert_eq!(o[..2], [UNTOUCHED, UNTOUCHED]);
}

fn call_checked_detailed(
    door: u32,
    format: Option<&[u8; 92]>,
    constraints: &[u8; 72],
    input: &[u8],
) -> (jint, [jlong; 8]) {
    let mut stub = StubEnv::new();
    let env = stub.env();
    let mut o = checked_out();
    let rc = unsafe {
        native_cast_checked_detailed(
            env,
            null_mut(),
            door as jint,
            format.map_or(null_mut(), |f| f.as_ptr().cast_mut().cast()),
            constraints.as_ptr().cast_mut().cast(),
            null_mut(),
            text(input),
            o.as_mut_ptr().cast(),
        )
    };
    (rc, o)
}

#[test]
fn a_checked_detailed_door_writes_the_value_the_span_the_check_and_the_detail() {
    let i32_door = Door::I32.code();
    let f = strict(NumFormat::GROUPING_WESTERN);
    // Strict Western: a one-digit group after a separator is group_size (11), spanning ",2".
    let (rc, o) = call_checked_detailed(i32_door, Some(&f), &ten_by_five(1), b"1,2");
    assert_eq!((rc, o[2], o[3], o[7]), (2, 1, 2, 11));
    // A value: its words and the check, and no fault.
    let (rc, o) = call_checked_detailed(i32_door, Some(&f), &ten_by_five(1), b"12");
    assert_eq!(
        (rc, &o[..2], &o[4..7]),
        (0, &[10, 0][..], &[0, 1, NO_ENTRY][..])
    );
    assert_eq!((o[2], o[3], o[7]), (UNTOUCHED, UNTOUCHED, UNTOUCHED));
    // A parse failure carries its detail (unexpected_character, 2).
    let (rc, o) = call_checked_detailed(i32_door, Some(&f), &ten_by_five(0), b"1x");
    assert_eq!(
        (rc, o[2], o[3], &o[4..7], o[7]),
        (2, 1, 1, &[0, 0, NO_ENTRY][..], 2)
    );
    // A violation spans the value, carries the keyword, and its detail is 0.
    let (rc, o) = call_checked_detailed(i32_door, Some(&f), &ten_by_five(0), b"12");
    assert_eq!(
        (rc, o[2], o[3], &o[4..7], o[7]),
        (4, 0, 2, &[5, 0, NO_ENTRY][..], 0)
    );
    with(|s| assert_eq!(s.long_writes, [(2, 2), (7, 1), (4, 3)]));
}

#[test]
fn a_checked_detailed_door_judges_the_format_by_1_1_and_refuses_before_writing() {
    let mut bad = invariant();
    bad[4..8].copy_from_slice(&(3u32 << 8 | NumFormat::ALL).to_ne_bytes());
    let (rc, o) = call_checked_detailed(Door::I32.code(), Some(&bad), &ten_by_five(0), b"12");
    assert_eq!((rc, o), (-1, checked_out()));
    with(|s| assert!(s.long_writes.is_empty()));
    // The 1.0 checked native still parses it legacy.
    let (rc, _) = call_checked(
        Door::I32.code(),
        Some(&bad),
        &ten_by_five(0),
        None,
        b"12,34",
    );
    assert_eq!(rc, 4);
}

#[test]
fn the_checked_detailed_door_over_a_double_writes_the_detail_of_a_fault() {
    let mut stub = StubEnv::new();
    let env = stub.env();
    let none = wire(&declared(0));
    let mut o = checked_out();
    let rc = unsafe {
        native_cast_checked_detailed_from_f64(
            env,
            null_mut(),
            Door::I32.code() as jint,
            f64::NAN,
            none.as_ptr().cast_mut().cast(),
            null_mut(),
            o.as_mut_ptr().cast(),
        )
    };
    // not_finite is detail 22.
    assert_eq!((rc, o[2], o[3], o[7]), (2, 0, 0, 22));
    let mut o = checked_out();
    let rc = unsafe {
        native_cast_checked_detailed_from_f64(
            env,
            null_mut(),
            Door::I32.code() as jint,
            7.0,
            none.as_ptr().cast_mut().cast(),
            null_mut(),
            o.as_mut_ptr().cast(),
        )
    };
    assert_eq!((rc, o[0], &o[4..7]), (0, 7, &[0, 0, NO_ENTRY][..]));
    // A violation from a double: its span is empty, its detail 0.
    let mut raw = declared(RawConstraints::HAS_MINIMUM);
    raw.lower = whole(10, false);
    let minimum = wire(&raw);
    let mut o = checked_out();
    let rc = unsafe {
        native_cast_checked_detailed_from_f64(
            env,
            null_mut(),
            Door::I32.code() as jint,
            7.0,
            minimum.as_ptr().cast_mut().cast(),
            null_mut(),
            o.as_mut_ptr().cast(),
        )
    };
    assert_eq!((rc, o[2], o[3], o[7], o[4]), (4, 0, 0, 0, 1));
}
