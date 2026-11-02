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
//!
//! The checked doors are swept the same way: every truncation through `cast_checked` at each
//! numeric door and through `cast_text_checked`, under declarations that make violations
//! common, so a violation's span is held to the invariant as a fault's is. And every
//! truncation of every `corpus/constraints.json` input goes through the C exports
//! themselves, `cast_checked` under the vector's own declaration and `cast_text`, as a
//! binding calls them: codes 1 to 4 write the span a binding slices with.

use hypercast::{
    Constraints, CurrencySymbol, DateOrder, ExcelEpoch, Fault, NumFormat, NumericRules, RawCheck,
    RawConstraints, RawNumFormat, RawSpan, Rejection, Rounding, TextRules, UnixPrecision,
    decimal_for_declaration,
};
use serde_json::Value;
use std::path::PathBuf;

/// Every corpus vector's text: its `input`, or its `input_bytes` (text that is not UTF-8).
/// A vector with neither (a typed `from_f64` row, a refused declaration) has no text.
fn corpus_inputs() -> Vec<Vec<u8>> {
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
            if let Some(input) = vector.get("input") {
                inputs.push(input.as_str().expect("input").as_bytes().to_vec());
            } else if let Some(bytes) = vector.get("input_bytes") {
                let bytes = bytes.as_array().expect("input_bytes");
                inputs.push(bytes.iter().map(|b| b.as_u64().expect("a byte") as u8).collect());
            }
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

fn assert_checked_span<T>(input: &[u8], verdict: Result<T, Rejection>, door: &str) {
    match verdict {
        Ok(_) => {}
        Err(Rejection::Fault(fault)) => assert_span::<()>(input, Err(fault), door),
        Err(Rejection::Violation(violation)) => {
            let (offset, len) = (violation.offset as usize, violation.len as usize);
            assert!(
                offset + len <= input.len(),
                "{door}: violation span {offset}+{len} escapes {:?} ({} bytes)",
                String::from_utf8_lossy(input),
                input.len()
            );
        }
    }
}

/// Declarations under which most inputs a door reads break something.
struct Declarations {
    strict: NumericRules,
    rounded: NumericRules,
    integer: NumericRules,
    wide: NumericRules,
    text: [TextRules; 2],
}

fn declarations() -> Declarations {
    let dec = |text: &str| decimal_for_declaration(text).expect("declarable");
    Declarations {
        strict: NumericRules::new()
            .minimum(dec("1"))
            .exclusive_maximum(dec("10"))
            .multiple_of(dec("0.05"), Rounding::Strict),
        rounded: NumericRules::new()
            .exclusive_minimum(dec("-1"))
            .maximum(dec("100"))
            .multiple_of(dec("0.000001"), Rounding::Midpoint),
        integer: NumericRules::new().minimum(dec("-100")).multiple_of(dec("5"), Rounding::Ceiling),
        wide: NumericRules::new().multiple_of(dec("10"), Rounding::Ceiling),
        text: [
            TextRules::new().min_length(2).max_length(4).trim(true),
            TextRules::new().min_length(3),
        ],
    }
}

fn every_checked_door(input: &[u8], formats: &[NumFormat], declared: &Declarations) {
    let integers =
        Constraints::<i8>::numeric(declared.integer, Some(&[0, 5, 125])).expect("declared");
    let wide = Constraints::<u64>::numeric(declared.wide, None).expect("declared");
    for format in formats {
        for rules in [declared.strict, declared.rounded] {
            let f32s = Constraints::<f32>::numeric(rules, None).expect("declared");
            let f64s = Constraints::<f64>::numeric(rules, None).expect("declared");
            let decimals =
                Constraints::<hypercast::Decimal>::numeric(rules, None).expect("declared");
            assert_checked_span(
                input,
                hypercast::cast_checked(input, format, &f32s),
                "checked f32",
            );
            assert_checked_span(
                input,
                hypercast::cast_checked(input, format, &f64s),
                "checked f64",
            );
            assert_checked_span(
                input,
                hypercast::cast_checked(input, format, &decimals),
                "checked decimal",
            );
        }
        assert_checked_span(input, hypercast::cast_checked(input, format, &integers), "checked i8");
        assert_checked_span(input, hypercast::cast_checked(input, format, &wide), "checked u64");
    }
    for rules in declared.text {
        let texts = Constraints::text(rules, Some(&["Male", "Female"])).expect("declared");
        assert_checked_span(input, hypercast::cast_text_checked(input, &texts), "checked text");
    }
}

fn formats() -> [NumFormat; 7] {
    [
        NumFormat::INVARIANT,
        NumFormat::DETECT,
        NumFormat::new(',', '.', NumFormat::ALL),
        NumFormat::new('.', ',', 0),
        NumFormat::INVARIANT.with_currency(CurrencySymbol::new("$").unwrap()),
        NumFormat::new(',', '.', NumFormat::ALL).with_currency(CurrencySymbol::new("kr.").unwrap()),
        NumFormat::DETECT.with_currency(CurrencySymbol::new("€").unwrap()),
    ]
}

fn every_door(input: &[u8], formats: &[NumFormat]) {
    for format in formats {
        assert_span(input, hypercast::cast_i8(input, format), "cast_i8");
        assert_span(input, hypercast::cast_i64(input, format), "cast_i64");
        assert_span(input, hypercast::cast_u64(input, format), "cast_u64");
        assert_span(input, hypercast::cast_f32(input, format), "cast_f32");
        assert_span(input, hypercast::cast_f64(input, format), "cast_f64");
        assert_span(input, hypercast::cast_decimal(input, format), "cast_decimal");
    }
    assert_span(input, hypercast::cast_bool(input), "cast_bool");
    assert_span(input, hypercast::cast_char(input), "cast_char");
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
    let formats = formats();
    let declared = declarations();
    for bytes in corpus_inputs() {
        // Every prefix (the truncation family the fuzzer caught) and every suffix (leading
        // truncation exercises the sign/prefix arms), through every door, checked or not.
        for end in 0..=bytes.len() {
            every_door(&bytes[..end], &formats);
            every_checked_door(&bytes[..end], &formats, &declared);
        }
        for begin in 0..=bytes.len() {
            every_door(&bytes[begin..], &formats);
            every_checked_door(&bytes[begin..], &formats, &declared);
        }
    }
}

#[repr(C)]
struct RawFault {
    offset: u32,
    len: u32,
}

// The C exports, linked from the rlib as a C caller links them.
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
    fn cast_text(ptr: *const u8, len: usize, out: *mut RawSpan, fault: *mut RawFault) -> i32;
}

/// A constraints vector's door code and its declaration as the C ABI takes it, or `None` for
/// a row with no text (a refused declaration, a `from_f64` row).
fn exported_declaration(vector: &Value) -> Option<(u32, RawConstraints, Vec<u8>, Vec<u8>)> {
    let input = match (vector.get("input"), vector.get("input_bytes")) {
        (Some(text), _) => text.as_str()?.as_bytes().to_vec(),
        (_, Some(bytes)) => bytes.as_array()?.iter().map(|b| b.as_u64().unwrap() as u8).collect(),
        _ => return None,
    };
    let door = match vector["door"].as_str()? {
        "i8" => 2,
        "i16" => 3,
        "i32" => 4,
        "i64" => 5,
        "u8" => 6,
        "u16" => 7,
        "u32" => 8,
        "u64" => 9,
        "f32" => 10,
        "f64" => 11,
        "text" => 18,
        "decimal" => 19,
        other => panic!("unknown door {other}"),
    };
    let c = &vector["constraints"];
    let dec = |field: &str| c.get(field).map(|v| decimal_for_declaration(v.as_str().unwrap()));
    let mut raw = RawConstraints::ZERO;
    for (field, flag, upper) in [
        ("minimum", RawConstraints::HAS_MINIMUM, false),
        ("exclusiveMinimum", RawConstraints::HAS_EXCLUSIVE_MINIMUM, false),
        ("maximum", RawConstraints::HAS_MAXIMUM, true),
        ("exclusiveMaximum", RawConstraints::HAS_EXCLUSIVE_MAXIMUM, true),
    ] {
        if let Some(value) = dec(field) {
            raw.flags |= flag;
            *(if upper { &mut raw.upper } else { &mut raw.lower }) = value.unwrap();
        }
    }
    if let Some(value) = dec("multipleOf") {
        raw.flags |= RawConstraints::HAS_MULTIPLE_OF;
        raw.multiple_of = value.unwrap();
    }
    raw.rounding = match c.get("rounding").and_then(Value::as_str) {
        Some("midpoint") => 1,
        Some("floor") => 2,
        Some("ceiling") => 3,
        _ => 0,
    };
    for (field, flag) in [
        ("minLength", RawConstraints::HAS_MIN_LENGTH),
        ("maxLength", RawConstraints::HAS_MAX_LENGTH),
    ] {
        if let Some(length) = c.get(field) {
            raw.flags |= flag;
            let length = length.as_u64().unwrap() as u32;
            *(if flag == RawConstraints::HAS_MIN_LENGTH {
                &mut raw.min_length
            } else {
                &mut raw.max_length
            }) = length;
        }
    }
    if c.get("trim").and_then(Value::as_bool) == Some(true) {
        raw.flags |= RawConstraints::TRIM;
    }
    let mut entries = Vec::new();
    if c.get("enum").is_some() {
        raw.flags |= RawConstraints::HAS_ENUM;
    }
    for entry in c.get("enum").and_then(Value::as_array).into_iter().flatten() {
        let entry = entry.as_str().unwrap();
        if door == 18 {
            entries.extend_from_slice(&(entry.len() as u32).to_le_bytes());
            entries.extend_from_slice(entry.as_bytes());
        } else {
            let d = decimal_for_declaration(entry).unwrap();
            entries.extend_from_slice(&d.lo.to_ne_bytes());
            entries.extend_from_slice(&d.hi.to_ne_bytes());
            entries.extend_from_slice(&[d.scale, d.negative.into(), 0, 0]);
        }
    }
    Some((door, raw, entries, input))
}

fn assert_exported_span(input: &[u8], code: i32, fault: &RawFault, door: &str) {
    assert!(
        (0..=4).contains(&code),
        "{door}: code {code} for {:?}",
        String::from_utf8_lossy(input)
    );
    if code != 0 {
        let (offset, len) = (fault.offset as usize, fault.len as usize);
        assert!(
            offset + len <= input.len(),
            "{door}: code {code} span {offset}+{len} escapes {:?} ({} bytes)",
            String::from_utf8_lossy(input),
            input.len()
        );
    }
}

fn through_the_exports(
    input: &[u8],
    door: u32,
    raw: &RawConstraints,
    entries: &[u8],
    formats: &[RawNumFormat],
) {
    let mut out = [0u64; 2];
    let mut check = RawCheck { keyword: 0, adjusted: 0, enum_index: 0 };
    for format in core::iter::once(core::ptr::null()).chain(formats.iter().map(|f| f as *const _)) {
        let mut fault = RawFault { offset: 0, len: 0 };
        // SAFETY: every pointer is live for the call, and `out` holds 16 aligned bytes,
        // room for any door's value.
        let code = unsafe {
            cast_checked(
                door,
                0,
                format,
                raw,
                entries.as_ptr(),
                entries.len(),
                input.as_ptr(),
                input.len(),
                out.as_mut_ptr().cast(),
                &mut fault,
                &mut check,
            )
        };
        assert_exported_span(input, code, &fault, "cast_checked");
        if code == 0 && door == 18 {
            // SAFETY: the text door wrote a RawSpan.
            let span = unsafe { out.as_ptr().cast::<RawSpan>().read() };
            assert!(span.offset as usize + span.len as usize <= input.len(), "text span");
        }
    }
    let mut span = RawSpan { offset: 0, len: 0 };
    let mut fault = RawFault { offset: 0, len: 0 };
    // SAFETY: every pointer is live for the call.
    let code = unsafe { cast_text(input.as_ptr(), input.len(), &mut span, &mut fault) };
    assert_exported_span(input, code, &fault, "cast_text");
}

#[test]
fn every_truncation_through_the_c_exports_faults_in_bounds() {
    let formats = formats().map(|format| RawNumFormat { size: RawNumFormat::SIZE, format });
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus/constraints.json");
    let vectors: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(path).expect("corpus")).expect("json");
    let mut swept = 0;
    for vector in &vectors {
        let Some((door, raw, entries, bytes)) = exported_declaration(vector) else {
            continue;
        };
        swept += 1;
        for end in 0..=bytes.len() {
            through_the_exports(&bytes[..end], door, &raw, &entries, &formats);
        }
        for begin in 0..=bytes.len() {
            through_the_exports(&bytes[begin..], door, &raw, &entries, &formats);
        }
    }
    assert!(swept > 200, "swept only {swept} constraints vectors");
}

#[test]
fn the_fuzzer_regressions_stay_fixed() {
    // The minimized crash inputs, verbatim, at their original doors.
    assert_span(
        b"12.5",
        hypercast::cast_datetime(b"12.5", DateOrder::YearMonthDay),
        "cast_datetime",
    );
    assert_span(
        b"12",
        hypercast::cast_date_ordered(b"12", DateOrder::YearMonthDay),
        "cast_date_ordered",
    );
    assert_span(b"15", hypercast::cast_time(b"15"), "cast_time");
    assert_span(
        b"1/7/2026T",
        hypercast::cast_datetime(b"1/7/2026T", DateOrder::MonthDayYear),
        "cast_datetime",
    );
    assert_span(
        b"1/7/2026 3:",
        hypercast::cast_datetime(b"1/7/2026 3:", DateOrder::MonthDayYear),
        "cast_datetime",
    );
    assert_span(
        b"2026-01-02T15:04:05+0",
        hypercast::cast_timestamp(b"2026-01-02T15:04:05+0"),
        "cast_timestamp",
    );
    assert_span(b"1:2", hypercast::cast_duration(b"1:2"), "cast_duration");
}
