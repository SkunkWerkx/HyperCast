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

const EXPORTS: [&str; 27] = [
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
    assert_eq!(unsafe { native_jni_natives(null_mut(), null_mut()) }, 28);
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
