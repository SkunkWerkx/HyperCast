//! The C ABI, measured through the exports themselves as a binding calls them: each 1.0 export
//! beside `cast_detailed` (or its typed and checked twins) on the same input, once on a value
//! and once on a failure, and what a month vocabulary costs, judged on every call.
//!
//! The layouts are declared here rather than taken from the crate, and everything that needs
//! the detailed exports sits behind `cfg(not(hypercast_bench_base))`, so the same file builds
//! against a tree without them (`RUSTFLAGS="--cfg hypercast_bench_base"`) and its `old/` rows
//! are before-and-after receipts for the 1.0 exports.

// Linked for its exports alone where nothing else names the crate (the base build).
extern crate hypercast;

use criterion::{Criterion, criterion_group, criterion_main};
use std::ffi::c_void;
use std::hint::black_box;

#[repr(C)]
struct RawFault {
    offset: u32,
    len: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawCheck {
    keyword: u32,
    adjusted: u32,
    enum_index: u32,
}

/// `RawConstraints` with `maximum` set and nothing else: the size, the flags, four zero words,
/// then the 16-byte decimals: `minimum`, `maximum`, `multiple_of`, `exclusive_minimum` and
/// `exclusive_maximum`, all zero but `maximum`. The base tree's layout (`hypercast_bench_base`) is the 72-byte one it was built with: no
/// `exclusive_minimum` or `exclusive_maximum`, each of which since has a field of its own.
#[repr(C, align(8))]
struct RawConstraints {
    words: [u32; 6],
    minimum: [u64; 2],
    maximum: [u64; 2],
    multiple_of: [u64; 2],
    #[cfg(not(hypercast_bench_base))]
    exclusive_minimum: [u64; 2],
    #[cfg(not(hypercast_bench_base))]
    exclusive_maximum: [u64; 2],
}

const MAXIMUM_100: RawConstraints = RawConstraints {
    words: [size_of::<RawConstraints>() as u32, 4, 0, 0, 0, 0],
    minimum: [0, 0],
    maximum: [100, 0],
    multiple_of: [0, 0],
    #[cfg(not(hypercast_bench_base))]
    exclusive_minimum: [0, 0],
    #[cfg(not(hypercast_bench_base))]
    exclusive_maximum: [0, 0],
};

unsafe extern "C" {
    fn cast_bool(ptr: *const u8, len: usize, out: *mut c_void, fault: *mut RawFault) -> i32;
    fn cast_char(ptr: *const u8, len: usize, out: *mut c_void, fault: *mut RawFault) -> i32;
    fn cast_i32(
        p: *const u8,
        l: usize,
        f: *const c_void,
        out: *mut c_void,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_i64(
        p: *const u8,
        l: usize,
        f: *const c_void,
        out: *mut c_void,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_f64(
        p: *const u8,
        l: usize,
        f: *const c_void,
        out: *mut c_void,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_decimal(
        p: *const u8,
        l: usize,
        f: *const c_void,
        out: *mut c_void,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_uuid(ptr: *const u8, len: usize, out: *mut c_void, fault: *mut RawFault) -> i32;
    fn cast_timestamp(ptr: *const u8, len: usize, out: *mut c_void, fault: *mut RawFault) -> i32;
    fn cast_date(ptr: *const u8, len: usize, out: *mut c_void, fault: *mut RawFault) -> i32;
    fn cast_date_ordered(
        p: *const u8,
        l: usize,
        order: u32,
        out: *mut c_void,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_datetime(
        p: *const u8,
        l: usize,
        order: u32,
        periods: *const c_void,
        out: *mut c_void,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_time(ptr: *const u8, len: usize, out: *mut c_void, fault: *mut RawFault) -> i32;
    fn cast_duration(ptr: *const u8, len: usize, out: *mut c_void, fault: *mut RawFault) -> i32;
    fn cast_text(ptr: *const u8, len: usize, out: *mut c_void, fault: *mut RawFault) -> i32;
    fn cast_decimal_from_f64(value: f64, out: *mut c_void, fault: *mut RawFault) -> i32;
    fn cast_excel_serial_from_f64(
        value: f64,
        epoch: u32,
        out: *mut c_void,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_excel_time(value: f64, out: *mut c_void, fault: *mut RawFault) -> i32;
    fn cast_excel_duration(value: f64, out: *mut c_void, fault: *mut RawFault) -> i32;
    fn cast_checked(
        door: u32,
        param: u32,
        format: *const c_void,
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
}

/// A text door: its name, its `Door` code and parameter, a value and a failure.
type TextDoor = (&'static str, u32, u32, &'static [u8], &'static [u8]);

const TEXT_DOORS: [TextDoor; 14] = [
    ("bool", 1, 0, b"true", b"maybe"),
    ("char", 23, 0, b"U+00E9", b"U+D800"),
    ("i32", 4, 0, b"-1234567", b"12a"),
    ("i64", 5, 0, b"1,234,567,890", b"99999999999999999999"),
    ("f64", 11, 0, b"12345.6789", b"1.2.3"),
    ("decimal", 19, 0, b"-12345.6789", b"12.34.5"),
    ("uuid", 12, 0, b"67e55044-10b1-426f-9247-bb680e5fe0c8", b"not-a-guid"),
    ("timestamp", 13, 0, b"2026-01-07T15:04:05.123Z", b"2026-01-07T15:04:05"),
    ("date", 15, 0, b"2026-01-07", b"2026-13-07"),
    ("date_ordered", 20, 3, b"07/01/2026", b"07/13/2026"),
    ("datetime", 21, 2, b"1/7/2026 3:04 PM", b"1/7/2026 3:04 XM"),
    ("time", 16, 0, b"15:04:05.5", b"25:00"),
    ("duration", 17, 0, b"PT1H30M", b"P1Y"),
    ("text", 18, 0, b"hello", b"ab\xFFc"),
];

/// The 1.0 export of a text door, with nothing declared.
fn old_text(code: u32, param: u32, input: &[u8], out: *mut c_void, fault: *mut RawFault) -> i32 {
    let (p, l, null) = (input.as_ptr(), input.len(), core::ptr::null());
    // SAFETY: `out` holds 32 bytes, room for any door's value, and `fault` is live.
    unsafe {
        match code {
            1 => cast_bool(p, l, out, fault),
            23 => cast_char(p, l, out, fault),
            4 => cast_i32(p, l, null, out, fault),
            5 => cast_i64(p, l, null, out, fault),
            11 => cast_f64(p, l, null, out, fault),
            19 => cast_decimal(p, l, null, out, fault),
            12 => cast_uuid(p, l, out, fault),
            13 => cast_timestamp(p, l, out, fault),
            15 => cast_date(p, l, out, fault),
            20 => cast_date_ordered(p, l, param, out, fault),
            21 => cast_datetime(p, l, param, null, out, fault),
            16 => cast_time(p, l, out, fault),
            17 => cast_duration(p, l, out, fault),
            _ => cast_text(p, l, out, fault),
        }
    }
}

/// A typed door: its name, its code and parameter, a value and a failure.
type TypedDoor = (&'static str, u32, u32, f64, f64);

const TYPED_DOORS: [TypedDoor; 4] = [
    ("decimal_from_f64", 19, 0, 0.1, f64::NAN),
    ("excel_serial_from_f64", 22, 1, 45000.5, 60.0),
    ("excel_time", 16, 0, 0.75, -1.0),
    ("excel_duration", 17, 0, 1.5, f64::INFINITY),
];

fn old_typed(code: u32, param: u32, value: f64, out: *mut c_void, fault: *mut RawFault) -> i32 {
    // SAFETY: as in `old_text`.
    unsafe {
        match code {
            19 => cast_decimal_from_f64(value, out, fault),
            22 => cast_excel_serial_from_f64(value, param, out, fault),
            16 => cast_excel_time(value, out, fault),
            _ => cast_excel_duration(value, out, fault),
        }
    }
}

fn old_exports(c: &mut Criterion) {
    let mut out = [0u64; 4];
    let out = out.as_mut_ptr().cast::<c_void>();
    let mut fault = RawFault { offset: 0, len: 0 };
    let fault = &raw mut fault;
    for (name, code, param, value, failure) in TEXT_DOORS {
        assert_eq!(old_text(code, param, value, out, fault), 0, "{name}");
        assert!(old_text(code, param, failure, out, fault) > 0, "{name}");
        let mut group = c.benchmark_group(format!("old/{name}"));
        group.bench_function("value", |b| {
            b.iter(|| old_text(code, param, black_box(value), out, fault))
        });
        group.bench_function("failure", |b| {
            b.iter(|| old_text(code, param, black_box(failure), out, fault))
        });
        group.finish();
    }
    for (name, code, param, value, failure) in TYPED_DOORS {
        assert_eq!(old_typed(code, param, value, out, fault), 0, "{name}");
        assert!(old_typed(code, param, failure, out, fault) > 0, "{name}");
        let mut group = c.benchmark_group(format!("old/{name}"));
        group.bench_function("value", |b| {
            b.iter(|| old_typed(code, param, black_box(value), out, fault))
        });
        group.bench_function("failure", |b| {
            b.iter(|| old_typed(code, param, black_box(failure), out, fault))
        });
        group.finish();
    }
    let mut check = RawCheck { keyword: 0, adjusted: 0, enum_index: 0 };
    let check = &raw mut check;
    let checked = |input: &[u8]| {
        // SAFETY: every pointer is live for the call.
        unsafe {
            cast_checked(
                4,
                0,
                core::ptr::null(),
                &MAXIMUM_100,
                core::ptr::null(),
                0,
                input.as_ptr(),
                input.len(),
                out,
                fault,
                check,
            )
        }
    };
    assert_eq!([checked(b"42"), checked(b"420"), checked(b"4x")], [0, 4, 2]);
    let mut group = c.benchmark_group("old/checked_i32");
    for (case, input) in [("value", &b"42"[..]), ("violation", b"420"), ("failure", b"4x")] {
        group.bench_function(case, |b| b.iter(|| checked(black_box(input))));
    }
    group.finish();
    let checked_typed = |value: f64| {
        // SAFETY: every pointer is live for the call.
        unsafe {
            cast_checked_from_f64(4, value, &MAXIMUM_100, core::ptr::null(), 0, out, fault, check)
        }
    };
    let mut group = c.benchmark_group("old/checked_from_f64_i32");
    for (case, value) in [("value", 42.0), ("violation", 420.0), ("failure", 4.5)] {
        group.bench_function(case, |b| b.iter(|| checked_typed(black_box(value))));
    }
    group.finish();
}

#[cfg(not(hypercast_bench_base))]
mod detailed {
    use super::*;

    #[repr(C)]
    pub struct RawFaultDetail {
        pub size: u32,
        pub offset: u32,
        pub len: u32,
        pub detail: u32,
    }

    #[repr(C)]
    pub struct RawDateOptions {
        pub size: u32,
        pub flags: u32,
        pub pivot: u32,
    }

    unsafe extern "C" {
        fn cast_detailed(
            door: u32,
            param: u32,
            format: *const c_void,
            periods: *const c_void,
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
            format: *const c_void,
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
    }

    /// `cast_detailed` with nothing declared but `options` and the vocabulary.
    #[allow(clippy::too_many_arguments)]
    fn detailed_text(
        code: u32,
        param: u32,
        options: *const RawDateOptions,
        vocab: &[u8],
        input: &[u8],
        out: *mut c_void,
        fault: *mut RawFaultDetail,
    ) -> i32 {
        let null = core::ptr::null();
        // SAFETY: every pointer is live for the call, and `out` has room for any value.
        unsafe {
            cast_detailed(
                code,
                param,
                null,
                null,
                options,
                vocab.as_ptr(),
                vocab.len(),
                input.as_ptr(),
                input.len(),
                out,
                fault,
            )
        }
    }

    /// The English abbreviations, with `Sept` beside `Sep`: 13 entries.
    fn english() -> Vec<u8> {
        let mut packed = Vec::new();
        for (month, name) in [
            (1, "Jan"),
            (2, "Feb"),
            (3, "Mar"),
            (4, "Apr"),
            (5, "May"),
            (6, "Jun"),
            (7, "Jul"),
            (8, "Aug"),
            (9, "Sep"),
            (9, "Sept"),
            (10, "Oct"),
            (11, "Nov"),
            (12, "Dec"),
        ] {
            packed.push(month);
            packed.extend_from_slice(&(name.len() as u32).to_le_bytes());
            packed.extend_from_slice(name.as_bytes());
        }
        packed
    }

    pub fn detailed_exports(c: &mut Criterion) {
        let mut out = [0u8; 40];
        // One byte in: the detailed exports write unaligned, as a binding may hand them.
        let out = unsafe { out.as_mut_ptr().add(1) }.cast::<c_void>();
        let mut fault = RawFaultDetail { size: 16, offset: 0, len: 0, detail: 0 };
        let fault = &raw mut fault;
        let none = core::ptr::null();
        for (name, code, param, value, failure) in TEXT_DOORS {
            assert_eq!(detailed_text(code, param, none, &[], value, out, fault), 0, "{name}");
            assert!(detailed_text(code, param, none, &[], failure, out, fault) > 0, "{name}");
            let mut group = c.benchmark_group(format!("detailed/{name}"));
            group.bench_function("value", |b| {
                b.iter(|| detailed_text(code, param, none, &[], black_box(value), out, fault))
            });
            group.bench_function("failure", |b| {
                b.iter(|| detailed_text(code, param, none, &[], black_box(failure), out, fault))
            });
            group.finish();
        }
        for (name, code, param, value, failure) in TYPED_DOORS {
            let typed = |value: f64| {
                // SAFETY: every pointer is live for the call.
                unsafe { cast_detailed_from_f64(code, param, value, out, fault) }
            };
            assert_eq!(typed(value), 0, "{name}");
            assert!(typed(failure) > 0, "{name}");
            let mut group = c.benchmark_group(format!("detailed/{name}"));
            group.bench_function("value", |b| b.iter(|| typed(black_box(value))));
            group.bench_function("failure", |b| b.iter(|| typed(black_box(failure))));
            group.finish();
        }
        let mut check = RawCheck { keyword: 0, adjusted: 0, enum_index: 0 };
        let check = &raw mut check;
        let checked = |input: &[u8]| {
            // SAFETY: every pointer is live for the call.
            unsafe {
                cast_checked_detailed(
                    4,
                    0,
                    core::ptr::null(),
                    &MAXIMUM_100,
                    core::ptr::null(),
                    0,
                    input.as_ptr(),
                    input.len(),
                    out,
                    fault,
                    check,
                )
            }
        };
        assert_eq!([checked(b"42"), checked(b"420"), checked(b"4x")], [0, 4, 2]);
        let mut group = c.benchmark_group("detailed/checked_i32");
        for (case, input) in [("value", &b"42"[..]), ("violation", b"420"), ("failure", b"4x")] {
            group.bench_function(case, |b| b.iter(|| checked(black_box(input))));
        }
        group.finish();
        let checked_typed = |value: f64| {
            // SAFETY: every pointer is live for the call.
            unsafe {
                cast_checked_detailed_from_f64(
                    4,
                    0,
                    &MAXIMUM_100,
                    core::ptr::null(),
                    0,
                    value,
                    out,
                    fault,
                    check,
                )
            }
        };
        let mut group = c.benchmark_group("detailed/checked_from_f64_i32");
        for (case, value) in [("value", 42.0), ("violation", 420.0), ("failure", 4.5)] {
            group.bench_function(case, |b| b.iter(|| checked_typed(black_box(value))));
        }
        group.finish();

        // What a vocabulary costs: judged in full on every call, then read. The numeric date
        // with no vocabulary is the baseline; with one declared, the difference is the
        // judgement alone; a named month adds the reading.
        let vocab = english();
        let pivot = RawDateOptions { size: 12, flags: 0, pivot: 2029 };
        let mut group = c.benchmark_group("vocabulary");
        let cases: [(&str, *const RawDateOptions, &[u8], &[u8]); 6] = [
            ("numeric, none declared", none, &[], b"07/09/2026"),
            ("numeric, vocabulary declared", none, &vocab, b"07/09/2026"),
            ("named month", none, &vocab, b"7-Sep-2026"),
            ("named alias", none, &vocab, b"7-Sept-2026"),
            ("named month, pivot", &pivot, &vocab, b"7-Sep-26"),
            ("unknown name", none, &vocab, b"7-Sepx-2026"),
        ];
        for (case, options, vocab, input) in cases {
            let code = detailed_text(20, 3, options, vocab, input, out, fault);
            assert_eq!(code, if case == "unknown name" { 2 } else { 0 }, "{case}");
            group.bench_function(case, |b| {
                b.iter(|| detailed_text(20, 3, options, vocab, black_box(input), out, fault))
            });
        }
        group.bench_function("validate alone (Rust)", |b| {
            b.iter(|| {
                let names = hypercast::MonthNames::from_packed(black_box(&vocab)).unwrap();
                hypercast::DateOptions::NONE.with_pivot(2029).validate(Some(names)).is_ok()
            })
        });
        group.finish();

        // Western grouping, a numeral it reads and one it refuses.
        let western = hypercast::RawNumFormat::from(hypercast::NumFormat::INVARIANT);
        let mut group = c.benchmark_group("grouping");
        for (case, format, input) in [
            ("western 1,234,567", &western, &b"1,234,567"[..]),
            ("western 12,34 (group_size)", &western, b"12,34"),
        ] {
            group.bench_function(case, |b| {
                b.iter(|| unsafe {
                    let input = black_box(input);
                    cast_detailed(
                        4,
                        0,
                        (format as *const hypercast::RawNumFormat).cast(),
                        none.cast(),
                        none,
                        core::ptr::null(),
                        0,
                        input.as_ptr(),
                        input.len(),
                        out,
                        fault,
                    )
                })
            });
        }
        group.finish();
    }
}

#[cfg(not(hypercast_bench_base))]
fn all(c: &mut Criterion) {
    old_exports(c);
    detailed::detailed_exports(c);
}

#[cfg(hypercast_bench_base)]
fn all(c: &mut Criterion) {
    old_exports(c);
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_millis(500))
        .measurement_time(std::time::Duration::from_secs(2));
    targets = all
}
criterion_main!(benches);
