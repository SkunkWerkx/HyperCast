//! Every fault span must index the caller's own buffer: `offset + len <= input.len()`,
//! the invariant every binding silently relies on when it slices the offending text back
//! out. Found violated for real by the fuzz target (`fuzz/fuzz_targets/doors.rs`) on
//! truncated inputs — `cast_datetime("12.5", YearMonthDay)` faulted at offset 4, length 1,
//! one byte past a 4-byte input — and fixed by the clamped-fault convention: a fault about
//! input that *ended too soon* is a zero-length span at the truncation point.
//!
//! This is the deterministic, stable-toolchain arm of that defense: every corpus input,
//! truncated at every byte boundary, through every door under every format/order variant.
//! The fuzzer explores; this pins what it found so a regression fails `cargo test`, not
//! just a nightly fuzz session.

use hypercast::{CurrencySymbol, DateOrder, ExcelEpoch, Fault, NumFormat, UnixPrecision};
use serde_json::Value;
use std::path::PathBuf;

fn corpus_inputs() -> Vec<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus");
    let mut inputs = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("corpus directory") {
        let path = entry.expect("corpus entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("corpus file");
        let vectors: Vec<Value> = serde_json::from_str(&text).expect("corpus json");
        for vector in vectors {
            inputs.push(vector["input"].as_str().expect("input").to_string());
        }
    }
    inputs
}

fn assert_span<T>(input: &[u8], verdict: Result<T, Fault>, door: &str) {
    if let Err(fault) = verdict {
        let (offset, len) = (fault.offset as usize, fault.len as usize);
        assert!(
            offset <= input.len() && offset + len <= input.len(),
            "{door}: fault span {offset}+{len} escapes {:?} ({} bytes)",
            String::from_utf8_lossy(input),
            input.len()
        );
    }
}

fn every_door(input: &[u8]) {
    let formats = [
        NumFormat::INVARIANT,
        NumFormat::DETECT,
        NumFormat::new(',', '.', NumFormat::ALL),
        NumFormat::new('.', ',', 0),
        NumFormat::INVARIANT.with_currency(CurrencySymbol::new("$").unwrap()),
        NumFormat::new(',', '.', NumFormat::ALL).with_currency(CurrencySymbol::new("kr.").unwrap()),
        NumFormat::DETECT.with_currency(CurrencySymbol::new("€").unwrap()),
    ];
    for format in &formats {
        assert_span(input, hypercast::cast_i8(input, format), "cast_i8");
        assert_span(input, hypercast::cast_i64(input, format), "cast_i64");
        assert_span(input, hypercast::cast_u64(input, format), "cast_u64");
        assert_span(input, hypercast::cast_f32(input, format), "cast_f32");
        assert_span(input, hypercast::cast_f64(input, format), "cast_f64");
        assert_span(input, hypercast::cast_decimal(input, format), "cast_decimal");
    }
    assert_span(input, hypercast::cast_bool(input), "cast_bool");
    assert_span(input, hypercast::cast_uuid(input), "cast_uuid");
    assert_span(input, hypercast::cast_timestamp(input), "cast_timestamp");
    assert_span(input, hypercast::cast_unix(input, UnixPrecision::Millis), "cast_unix");
    assert_span(input, hypercast::cast_date(input), "cast_date");
    assert_span(input, hypercast::cast_time(input), "cast_time");
    assert_span(input, hypercast::cast_duration(input), "cast_duration");
    for order in [DateOrder::YearMonthDay, DateOrder::MonthDayYear, DateOrder::DayMonthYear] {
        assert_span(input, hypercast::cast_date_ordered(input, order), "cast_date_ordered");
        assert_span(input, hypercast::cast_datetime(input, order), "cast_datetime");
    }
    for epoch in [ExcelEpoch::Y1900, ExcelEpoch::Y1904] {
        assert_span(input, hypercast::cast_excel_serial(input, epoch), "cast_excel_serial");
    }
}

#[test]
fn every_truncation_of_every_corpus_input_faults_in_bounds() {
    for input in corpus_inputs() {
        let bytes = input.as_bytes();
        // Every prefix (the truncation family the fuzzer caught) and every suffix (leading
        // truncation exercises the sign/prefix arms), through every door.
        for end in 0..=bytes.len() {
            every_door(&bytes[..end]);
        }
        for begin in 0..=bytes.len() {
            every_door(&bytes[begin..]);
        }
    }
}

#[test]
fn the_fuzzer_regressions_stay_fixed() {
    // The minimized crash inputs, verbatim, at their original doors.
    assert_span(b"12.5", hypercast::cast_datetime(b"12.5", DateOrder::YearMonthDay), "cast_datetime");
    assert_span(b"12", hypercast::cast_date_ordered(b"12", DateOrder::YearMonthDay), "cast_date_ordered");
    assert_span(b"15", hypercast::cast_time(b"15"), "cast_time");
    assert_span(b"1/7/2026T", hypercast::cast_datetime(b"1/7/2026T", DateOrder::MonthDayYear), "cast_datetime");
    assert_span(b"1/7/2026 3:", hypercast::cast_datetime(b"1/7/2026 3:", DateOrder::MonthDayYear), "cast_datetime");
    assert_span(b"2026-01-02T15:04:05+0", hypercast::cast_timestamp(b"2026-01-02T15:04:05+0"), "cast_timestamp");
    assert_span(b"1:2", hypercast::cast_duration(b"1:2"), "cast_duration");
}
