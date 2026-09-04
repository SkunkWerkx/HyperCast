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

use hypercast::{NumFormat, UnixPrecision};
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

    // The failure paths must stay allocation-free too — a fault is a span, never captured text.
    assert_allocation_free("cast_bool failure", || hypercast::cast_bool(b"maybe").unwrap_err());
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
