//! HyperCast inside a real browser: a door from each family, a fault span, and the C ABI's
//! version export, compiled for wasm32-unknown-unknown. Run with
//! `wasm-pack test --headless --chrome` (or `--firefox`); on any other target this file
//! compiles to nothing.

#![cfg(target_arch = "wasm32")]

use hypercast::{
    Date, DateOrder, Decimal, Duration, Fault, NumFormat, Reason, Timestamp, cast_bool, cast_date,
    cast_date_ordered, cast_decimal, cast_duration, cast_f64, cast_i32, cast_timestamp, cast_u8,
    cast_uuid, hypercast_version,
};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn the_core_reports_the_crate_version() {
    let manifest = include_str!("../../Cargo.toml");
    let want = manifest
        .lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.split('"').next())
        .expect("a version line in rust/Cargo.toml");
    let v = hypercast_version();
    assert_eq!(format!("{}.{}.{}", v >> 16, (v >> 8) & 0xFF, v & 0xFF), want);
}

#[wasm_bindgen_test]
fn booleans_and_integers() {
    assert_eq!(cast_bool(" Enabled "), Ok(true));
    assert_eq!(cast_bool("off"), Ok(false));
    assert_eq!(cast_bool("maybe"), Err(Fault { reason: Reason::Malformed, offset: 0, len: 5 }));
    assert_eq!(cast_i32("(1,234)", &NumFormat::INVARIANT), Ok(-1234));
    assert_eq!(cast_i32("0xFF", &NumFormat::INVARIANT), Ok(255));
    assert_eq!(
        cast_u8("256", &NumFormat::INVARIANT).map_err(|f| f.reason),
        Err(Reason::OutOfRange)
    );
}

#[wasm_bindgen_test]
fn reals_and_decimals() {
    let eurozone = NumFormat::new(',', '.', NumFormat::ALL);
    assert_eq!(cast_f64("1.234,5", &eurozone), Ok(1234.5));
    assert_eq!(
        cast_f64("1e400", &NumFormat::INVARIANT).map_err(|f| f.reason),
        Err(Reason::OutOfRange)
    );
    let decimal = cast_decimal("12345.6789", &NumFormat::INVARIANT).unwrap();
    assert_eq!(decimal, Decimal { lo: 123_456_789, hi: 0, scale: 4, negative: false });
    assert_eq!(decimal.to_string(), "12345.6789");
}

#[wasm_bindgen_test]
fn uuids_in_every_form() {
    let want = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd,
        0xef,
    ];
    for text in [
        "01234567-89ab-cdef-0123-456789abcdef",
        "0123456789ABCDEF0123456789ABCDEF",
        "{01234567-89ab-cdef-0123-456789abcdef}",
        "urn:uuid:01234567-89ab-cdef-0123-456789abcdef",
        "{0x01234567,0x89ab,0xcdef,{0x01,0x23,0x45,0x67,0x89,0xab,0xcd,0xef}}",
    ] {
        assert_eq!(cast_uuid(text), Ok(want), "{text}");
    }
    // 0.5.0's HyperUuid stray-hyphen shape: a fault at the misplaced byte, not a panic.
    assert!(cast_uuid("00000000-0000-0000-0000--00000000000").is_err());
}

#[wasm_bindgen_test]
fn temporals() {
    assert_eq!(
        cast_timestamp("2026-01-02T15:04:05.123+05:00"),
        Ok(Timestamp { seconds: 1_767_348_245, nanos: 123_000_000 })
    );
    assert_eq!(cast_date("2024-02-29"), Ok(Date { year: 2024, month: 2, day: 29 }));
    assert_eq!(cast_date("2026-02-29").map_err(|f| (f.offset, f.len)), Err((8, 2)));
    assert_eq!(
        cast_date_ordered("1/7/2026", DateOrder::MonthDayYear),
        Ok(Date { year: 2026, month: 1, day: 7 })
    );
    assert_eq!(cast_duration("PT1.5S"), Ok(Duration { seconds: 1, nanos: 500_000_000 }));
}
