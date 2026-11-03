//! Empirically verifies this crate's own "allocation-free" claim rather than just asserting
//! it in a doc comment: a counting `#[global_allocator]` wraps the system allocator for this
//! one test binary only (integration tests compile as separate binaries, so this never
//! affects the library itself or its other consumers) and asserts zero allocations across
//! 1000 calls to every door — success and failure paths both, since a fault that captured
//! text instead of a span would be exactly the allocation this design exists to avoid.
//!
//! The counter is per-thread, not process-wide, and this file is deliberately one `#[test]`
//! function. Both matter. `cargo test` runs each test body on its own spawned thread while
//! the harness's main thread keeps going: right after `spawn` returns it inserts the join
//! handle into a `HashMap` and pushes a timeout entry onto a `VecDeque`, and the first of
//! each allocates — concurrently with the body already running. HyperUuid's process-wide
//! counter saw exactly that on a slow-to-schedule CI runner (linux-arm64, 1-allocation-off
//! on the very first measured call, green on retry), and had earlier seen the same shape
//! from sibling test threads' spawn overhead when it was several `#[test]` fns. Counting in
//! a thread-local makes the harness invisible by construction; one test function keeps the
//! whole claim on one thread with no `--test-threads=1` flag for CI's invocation to drift
//! away from.
//!
//! The thread-local is `const`-initialised and holds a type with no `Drop`, so reaching it
//! from inside the allocator registers no destructor and allocates nothing itself.

use hypercast::{NumFormat, RawCheck, RawConstraints, RawNumFormat, RawSpan, UnixPrecision};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct CountingAllocator;

thread_local! {
    static ALLOC_COUNT: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_COUNT.with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[repr(C)]
struct RawFault {
    offset: u32,
    len: u32,
}

// The validation constraints' five C exports, linked from the rlib as a C caller links them.
unsafe extern "C" {
    fn cast_checked(
        door: u32,
        param: u32,
        format: *const RawNumFormat,
        constraints: *const RawConstraints,
        enum_ptr: *const u8,
        enum_len: usize,
        ptr: *const u8,
        len: usize,
        out: *mut u8,
        fault: *mut RawFault,
        check: *mut RawCheck,
    ) -> i32;
    fn cast_checked_from_f64(
        door: u32,
        value: f64,
        constraints: *const RawConstraints,
        enum_ptr: *const u8,
        enum_len: usize,
        out: *mut u8,
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
    fn cast_declared_decimal(ptr: *const u8, len: usize, out: *mut hypercast::Decimal) -> i32;
}

/// One export call with its out-params on the stack: the return code.
fn exported(call: impl Fn(*mut u8, *mut RawFault, *mut RawCheck) -> i32) -> i32 {
    let mut out = [0u64; 2];
    let mut fault = RawFault { offset: 0, len: 0 };
    let mut check = RawCheck { keyword: 0, adjusted: 0, enum_index: 0 };
    call(out.as_mut_ptr().cast(), &mut fault, &mut check)
}

/// Text `enum` entries packed as the ABI takes them.
fn packed_text(entries: &[&str]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for entry in entries {
        bytes.extend_from_slice(&(entry.len() as u32).to_le_bytes());
        bytes.extend_from_slice(entry.as_bytes());
    }
    bytes
}

/// Integer `enum` entries packed as the ABI takes them: native-endian `Decimal`s.
fn packed_integers(entries: &[i64]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for &entry in entries {
        bytes.extend_from_slice(&entry.unsigned_abs().to_ne_bytes());
        bytes.extend_from_slice(&0u32.to_ne_bytes());
        bytes.extend_from_slice(&[0, (entry < 0).into(), 0, 0]);
    }
    bytes
}

fn allocs_during<T>(f: impl Fn() -> T) -> usize {
    let before = ALLOC_COUNT.with(Cell::get);
    std::hint::black_box(f());
    ALLOC_COUNT.with(Cell::get) - before
}

#[track_caller]
fn assert_allocation_free<T>(door: &str, f: impl Fn() -> T) {
    for _ in 0..1000 {
        let allocs = allocs_during(&f);
        assert_eq!(allocs, 0, "{door} allocated {allocs} time(s) in one call");
    }
}

#[test]
fn allocation_free() {
    let format = NumFormat::INVARIANT;

    assert_allocation_free("cast_bool", || hypercast::cast_bool(b"enabled").unwrap());
    assert_allocation_free("cast_char", || hypercast::cast_char(b"&#x1F600;").unwrap());
    assert_allocation_free("cast_i64", || hypercast::cast_i64(b"(1,234,567)", &format).unwrap());
    assert_allocation_free("cast_u64", || {
        hypercast::cast_u64(b"18446744073709551615", &format).unwrap()
    });
    assert_allocation_free("cast_i8 hex", || hypercast::cast_i8(b"0xFF", &format).unwrap());
    assert_allocation_free("cast_f64", || hypercast::cast_f64(b"1,234.5e-3", &format).unwrap());
    assert_allocation_free("cast_f32 percent", || hypercast::cast_f32(b"25.5%", &format).unwrap());
    assert_allocation_free("cast_uuid", || {
        hypercast::cast_uuid(b"urn:uuid:01020304-0506-0708-090a-0b0c0d0e0f10").unwrap()
    });
    assert_allocation_free("cast_timestamp", || {
        hypercast::cast_timestamp(b"2026-01-02T15:04:05.123456789+05:00").unwrap()
    });
    assert_allocation_free("cast_unix", || {
        hypercast::cast_unix(b"1700000000123", UnixPrecision::Millis).unwrap()
    });
    assert_allocation_free("decimal_from_f64", || hypercast::decimal_from_f64(0.1 + 0.2).unwrap());
    assert_allocation_free("u64_from_f64", || hypercast::u64_from_f64(1e19).unwrap());
    assert_allocation_free("excel_duration", || hypercast::excel_duration(1.5).unwrap());
    assert_allocation_free("cast_excel_serial", || {
        hypercast::cast_excel_serial(b"45292.75", hypercast::ExcelEpoch::Y1900).unwrap()
    });
    assert_allocation_free("cast_date", || hypercast::cast_date(b"2026-01-02").unwrap());
    assert_allocation_free("cast_date_ordered", || {
        hypercast::cast_date_ordered(b"1/7/2026", hypercast::DateOrder::MonthDayYear).unwrap()
    });
    assert_allocation_free("cast_datetime", || {
        hypercast::cast_datetime(b"1/7/2026 3:04:05.123 PM", hypercast::DateOrder::MonthDayYear)
            .unwrap()
    });
    let dollars = NumFormat::INVARIANT.with_currency(hypercast::CurrencySymbol::new("$").unwrap());
    assert_allocation_free("cast_i64 currency", || {
        hypercast::cast_i64(b"($1,234)", &dollars).unwrap()
    });
    assert_allocation_free("cast_f64 currency", || {
        hypercast::cast_f64(b"-$1,234.5", &dollars).unwrap()
    });
    assert_allocation_free("cast_decimal", || {
        hypercast::cast_decimal(b"1,234.50", &format).unwrap()
    });
    assert_allocation_free("cast_decimal currency percent", || {
        hypercast::cast_decimal(b"($1,234.50)%", &dollars).unwrap()
    });
    assert_allocation_free("cast_decimal failure", || {
        hypercast::cast_decimal(b"79228162514264337593543950336", &format).unwrap_err()
    });
    assert_allocation_free("hypercast_version", || hypercast::hypercast_version());
    assert_allocation_free("cast_f64 (separator detection)", || {
        hypercast::cast_f64(b"1.234.567,89", &hypercast::NumFormat::DETECT).unwrap()
    });
    assert_allocation_free("cast_duration (comma decimal mark)", || {
        hypercast::cast_duration(b"0:00:01,5").unwrap()
    });
    assert_allocation_free("cast_time", || hypercast::cast_time(b"15:04:05.123456789").unwrap());
    assert_allocation_free("cast_duration iso", || {
        hypercast::cast_duration(b"P1DT6H30M15.5S").unwrap()
    });
    assert_allocation_free("cast_duration colon", || {
        hypercast::cast_duration(b"-1.06:30:15.5").unwrap()
    });
    assert_allocation_free("cast_duration protobuf", || {
        hypercast::cast_duration(b"3.000000001s").unwrap()
    });

    // The checked doors: judgement, rounding and delivery, on the stack.
    let dec = |text: &str| hypercast::decimal_for_declaration(text).unwrap();
    let latitude = hypercast::Constraints::<f64>::numeric(
        hypercast::NumericRules::new()
            .minimum(dec("-90"))
            .maximum(dec("90"))
            .multiple_of(dec("0.000001"), hypercast::Rounding::Midpoint),
        None,
    )
    .unwrap();
    assert_allocation_free("cast_checked rounded", || {
        hypercast::cast_checked(b"45.1234567", &format, &latitude).unwrap()
    });
    let long = format!("0.1{}1", "0".repeat(300));
    assert_allocation_free("cast_checked long", || {
        hypercast::cast_checked(&long, &format, &latitude).unwrap()
    });
    assert_allocation_free("checked_from_f64", || {
        hypercast::checked_from_f64(45.1234567, &latitude).unwrap()
    });
    let cash = hypercast::Constraints::<hypercast::Decimal>::numeric(
        hypercast::NumericRules::new().multiple_of(dec("0.05"), hypercast::Rounding::Strict),
        None,
    )
    .unwrap();
    assert_allocation_free("cast_checked violation", || {
        hypercast::cast_checked(b"($12.37)", &dollars, &cash).unwrap_err()
    });
    let codes =
        hypercast::Constraints::<i32>::numeric(Default::default(), Some(&[10, 20])).unwrap();
    assert_allocation_free("cast_checked enum", || {
        hypercast::cast_checked(b"20", &format, &codes).unwrap()
    });
    let sexes = hypercast::Constraints::text(
        hypercast::TextRules::new().min_length(1).max_length(10).trim(true),
        Some(&["Female", "Male", "Other"]),
    )
    .unwrap();
    assert_allocation_free("cast_text_checked", || {
        hypercast::cast_text_checked("\u{00A0}MALE ".as_bytes(), &sexes).unwrap()
    });
    assert_allocation_free("cast_text_checked violation", || {
        hypercast::cast_text_checked(b"Mail", &sexes).unwrap_err()
    });

    // The same judgements through the C exports, over a sample of
    // corpus/constraints.json: each verdict, a violation, a refusal and a contract violation.
    let latitude_raw = latitude.raw();
    let cash_raw = cash.raw();
    let sexes_raw = sexes.raw();
    let sexes_enum = packed_text(&["Female", "Male", "Other"]);
    let codes_enum = packed_integers(&[10, 20]);
    let codes_raw = codes.raw();
    let checked = |door: u32, raw: &RawConstraints, entries: &[u8], input: &[u8]| {
        exported(|out, fault, check| unsafe {
            cast_checked(
                door,
                0,
                core::ptr::null(),
                raw,
                entries.as_ptr(),
                entries.len(),
                input.as_ptr(),
                input.len(),
                out,
                fault,
                check,
            )
        })
    };
    // A label, the door, the declaration, its `enum` list, the input and the expected code.
    type Sample<'a> = (&'a str, u32, &'a RawConstraints, &'a [u8], &'a [u8], i32);
    let samples: [Sample; 8] = [
        ("cast_checked rounded", 11, &latitude_raw, &[], b"45.1234567", 0),
        ("cast_checked long", 11, &latitude_raw, &[], long.as_bytes(), 0),
        ("cast_checked violation", 11, &latitude_raw, &[], b"91", 4),
        ("cast_checked strict step", 19, &cash_raw, &[], b"12.37", 4),
        ("cast_checked enum", 4, &codes_raw, &codes_enum, b"20", 0),
        ("cast_checked text enum", 18, &sexes_raw, &sexes_enum, "\u{00A0}MALE ".as_bytes(), 0),
        ("cast_checked text violation", 18, &sexes_raw, &sexes_enum, b"Mail", 4),
        ("cast_checked malformed", 11, &latitude_raw, &[], b"9x", 2),
    ];
    for (door_name, door, raw, entries, input, code) in samples {
        assert_eq!(checked(door, raw, entries, input), code, "{door_name}");
        assert_allocation_free(door_name, || checked(door, raw, entries, input));
    }
    let from_f64 = |value: f64| {
        exported(|out, fault, check| unsafe {
            cast_checked_from_f64(11, value, &latitude_raw, core::ptr::null(), 0, out, fault, check)
        })
    };
    for (door_name, value, code) in [
        ("cast_checked_from_f64", 45.1234567, 0),
        ("cast_checked_from_f64 violation", 91.0, 4),
        ("cast_checked_from_f64 NaN", f64::NAN, 2),
    ] {
        assert_eq!(from_f64(value), code, "{door_name}");
        assert_allocation_free(door_name, || from_f64(value));
    }
    let judge = |door: u32, raw: &RawConstraints, entries: &[u8]| unsafe {
        cast_constraints_check(door, 0, raw, entries.as_ptr(), entries.len())
    };
    let twice = packed_text(&["Male", "MALE", "Female"]);
    let malformed = RawConstraints { size: 71, ..RawConstraints::ZERO };
    for (door_name, door, raw, entries, code) in [
        ("cast_constraints_check", 18, &sexes_raw, &sexes_enum[..], 0),
        ("cast_constraints_check duplicate", 18, &sexes_raw, &twice[..], 10),
        ("cast_constraints_check integers", 4, &codes_raw, &codes_enum[..], 0),
        ("cast_constraints_check empty enum", 4, &codes_raw, &[][..], 12),
        ("cast_constraints_check malformed", 4, &malformed, &[][..], 11),
    ] {
        assert_eq!(judge(door, raw, entries), code, "{door_name}");
        assert_allocation_free(door_name, || judge(door, raw, entries));
    }
    assert_eq!(checked(11, &malformed, &[], b"1"), -1);
    assert_allocation_free("cast_checked contract violation", || {
        checked(11, &malformed, &[], b"1")
    });
    let text = |input: &[u8]| {
        let mut span = RawSpan { offset: 0, len: 0 };
        let mut fault = RawFault { offset: 0, len: 0 };
        unsafe { cast_text(input.as_ptr(), input.len(), &mut span, &mut fault) }
    };
    assert_eq!(text("\u{1F44D}\u{1F3FD}".as_bytes()), 0);
    assert_allocation_free("cast_text", || text("\u{1F44D}\u{1F3FD}".as_bytes()));
    assert_eq!(text(b"ab\xFFc"), 2);
    assert_allocation_free("cast_text failure", || text(b"ab\xFFc"));
    let declared = |input: &[u8]| {
        let mut out = hypercast::Decimal { lo: 0, hi: 0, scale: 0, negative: false };
        unsafe { cast_declared_decimal(input.as_ptr(), input.len(), &mut out) }
    };
    for (door_name, input, code) in [
        ("cast_declared_decimal", &b"0.000001"[..], 0),
        ("cast_declared_decimal exponent", b"-1.50e3", 0),
        ("cast_declared_decimal refused", b"1e-29", 8),
        ("cast_declared_decimal not UTF-8", b"1\xFF", 8),
    ] {
        assert_eq!(declared(input), code, "{door_name}");
        assert_allocation_free(door_name, || declared(input));
    }
    assert_eq!(unsafe { cast_declared_decimal(b"1".as_ptr(), 1, core::ptr::null_mut()) }, -1);
    assert_allocation_free("cast_declared_decimal contract violation", || unsafe {
        cast_declared_decimal(b"1".as_ptr(), 1, core::ptr::null_mut())
    });

    // The failure paths must stay allocation-free too — a fault is a span, never captured text.
    assert_allocation_free("cast_bool failure", || hypercast::cast_bool(b"maybe").unwrap_err());
    assert_allocation_free("cast_char failure", || hypercast::cast_char(b"U+D800").unwrap_err());
    assert_allocation_free("cast_i32 failure", || {
        hypercast::cast_i32(b"99999999999999999999", &format).unwrap_err()
    });
    assert_allocation_free("cast_f64 failure", || {
        hypercast::cast_f64(b"1.2.3", &format).unwrap_err()
    });
    assert_allocation_free("cast_uuid failure", || {
        hypercast::cast_uuid(b"not-a-guid").unwrap_err()
    });
    assert_allocation_free("cast_timestamp failure", || {
        hypercast::cast_timestamp(b"2026-01-02T15:04:05").unwrap_err()
    });
    assert_allocation_free("cast_duration failure", || {
        hypercast::cast_duration(b"P1Y").unwrap_err()
    });
}
