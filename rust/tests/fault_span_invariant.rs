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
//! binding calls them: codes 1 to 4 write the span a binding slices with. Their detailed
//! twins, `cast_checked_detailed` and `cast_detailed` at the same door, go beside them.
//!
//! Every door's `detailed` twin is swept beside it and must agree on every truncation: the
//! same value, or the plain door's fault with a detail.

use hypercast::{
    Constraints, CurrencySymbol, DateOptions, DateOrder, Detail, ExcelEpoch, Failure, Fault,
    Grouping, MonthNames, NumFormat, NumericRules, RawCheck, RawConstraints, RawNumFormat, RawSpan,
    Rejection, Rounding, TextRules, UnixPrecision, decimal_for_declaration, detailed,
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

/// A door and its detailed twin on one input: the same value or the plain fault with a detail,
/// and that fault in bounds.
fn assert_twin<T: PartialEq + core::fmt::Debug>(
    input: &[u8],
    plain: Result<T, Fault>,
    twin: Result<T, Failure>,
    door: &str,
) {
    assert_eq!(
        plain,
        twin.map_err(Failure::fault),
        "{door}: the detailed twin disagrees on {:?}",
        String::from_utf8_lossy(input)
    );
    assert_span(input, plain, door);
}

/// A checked door and its detailed twin, likewise.
fn assert_checked_twin<T: PartialEq + core::fmt::Debug>(
    input: &[u8],
    plain: Result<hypercast::Checked<T>, Rejection<T>>,
    twin: Result<hypercast::Checked<T>, detailed::Rejection<T>>,
    door: &str,
) {
    assert_eq!(
        plain,
        twin.map_err(Rejection::from),
        "{door}: the detailed twin disagrees on {:?}",
        String::from_utf8_lossy(input)
    );
    assert_checked_span(input, plain, door);
}

fn assert_checked_span<T, V>(input: &[u8], verdict: Result<T, Rejection<V>>, door: &str) {
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
    let entries = [0, 5, 125].map(hypercast::Decimal::from);
    let integers = Constraints::<i8>::numeric(declared.integer, Some(&entries)).expect("declared");
    let wide = Constraints::<u64>::numeric(declared.wide, None).expect("declared");
    for format in formats {
        let valid = format.validate().expect("valid");
        macro_rules! twin {
            ($constraints:expr, $door:literal) => {
                assert_checked_twin(
                    input,
                    hypercast::cast_checked(input, format, $constraints),
                    detailed::cast_checked(input, &valid, $constraints),
                    $door,
                )
            };
        }
        for rules in [declared.strict, declared.rounded] {
            let f32s = Constraints::<f32>::numeric(rules, None).expect("declared");
            let f64s = Constraints::<f64>::numeric(rules, None).expect("declared");
            let decimals =
                Constraints::<hypercast::Decimal>::numeric(rules, None).expect("declared");
            twin!(&f32s, "checked f32");
            twin!(&f64s, "checked f64");
            twin!(&decimals, "checked decimal");
        }
        twin!(&integers, "checked i8");
        twin!(&wide, "checked u64");
    }
    for rules in declared.text {
        let texts = Constraints::text(rules, Some(&["Male", "Female"])).expect("declared");
        assert_checked_twin(
            input,
            hypercast::cast_text_checked(input, &texts),
            detailed::cast_text_checked(input, &texts),
            "checked text",
        );
    }
}

fn formats() -> [NumFormat; 9] {
    [
        NumFormat::INVARIANT,
        NumFormat::DETECT,
        NumFormat::new(',', '.', NumFormat::ALL),
        NumFormat::new('.', ',', 0),
        NumFormat::INVARIANT.with_currency(CurrencySymbol::new("$").unwrap()),
        NumFormat::new(',', '.', NumFormat::ALL).with_currency(CurrencySymbol::new("kr.").unwrap()),
        NumFormat::DETECT.with_currency(CurrencySymbol::new("€").unwrap()),
        NumFormat::new(',', '\u{202F}', NumFormat::ALL | NumFormat::TRAILING_SIGN)
            .with_currency(CurrencySymbol::new("€").unwrap()),
        NumFormat::new('.', '\u{202F}', NumFormat::ALL)
            .with_digits(hypercast::NativeDigits::new('\u{660}').unwrap())
            .with_grouping(Grouping::Indian),
    ]
}

fn every_door(input: &[u8], formats: &[NumFormat]) {
    macro_rules! twin {
        ($door:ident ( $($arg:expr),* )) => {
            assert_twin(
                input,
                hypercast::$door(input $(, $arg)*),
                detailed::$door(input $(, $arg)*),
                stringify!($door),
            )
        };
    }
    for format in formats {
        let valid = &format.validate().expect("valid");
        macro_rules! numeric {
            ($($door:ident),+) => {$(
                assert_twin(
                    input,
                    hypercast::$door(input, format),
                    detailed::$door(input, valid),
                    stringify!($door),
                );
            )+};
        }
        numeric!(cast_i8, cast_i64, cast_u64, cast_decimal);
        assert_twin(
            input,
            hypercast::cast_f32(input, format).map(f32::to_bits),
            detailed::cast_f32(input, valid).map(f32::to_bits),
            "cast_f32",
        );
        assert_twin(
            input,
            hypercast::cast_f64(input, format).map(f64::to_bits),
            detailed::cast_f64(input, valid).map(f64::to_bits),
            "cast_f64",
        );
    }
    twin!(cast_bool());
    twin!(cast_char());
    twin!(cast_uuid());
    twin!(cast_unix(UnixPrecision::Millis));
    assert_twin(
        input,
        hypercast::cast_timestamp(input),
        detailed::cast_timestamp(input, None),
        "cast_timestamp",
    );
    assert_twin(input, hypercast::cast_date(input), detailed::cast_date(input, None), "cast_date");
    twin!(cast_time());
    twin!(cast_duration());
    twin!(cast_text());
    for order in [DateOrder::YearMonthDay, DateOrder::MonthDayYear, DateOrder::DayMonthYear] {
        assert_twin(
            input,
            hypercast::cast_date_ordered(input, order),
            detailed::cast_date_ordered(input, order, None),
            "cast_date_ordered",
        );
        assert_twin(
            input,
            hypercast::cast_datetime(input, order),
            detailed::cast_datetime(input, order, None),
            "cast_datetime",
        );
    }
    for epoch in [ExcelEpoch::Y1900, ExcelEpoch::Y1904] {
        twin!(cast_excel_serial(epoch));
    }
}

/// The detailed date doors under every option at once and a vocabulary with dotted names: a
/// fault's span is held to the invariant as the plain doors' are.
fn every_dated_door(input: &[u8], options: &[hypercast::ValidDateOptions<'_>]) {
    for options in options {
        let options = Some(options);
        assert_span(input, detailed::cast_date(input, options).map_err(Failure::fault), "date");
        assert_span(
            input,
            detailed::cast_timestamp(input, options).map_err(Failure::fault),
            "timestamp",
        );
        for order in [DateOrder::YearMonthDay, DateOrder::MonthDayYear, DateOrder::DayMonthYear] {
            assert_span(
                input,
                detailed::cast_date_ordered(input, order, options).map_err(Failure::fault),
                "date_ordered",
            );
            assert_span(
                input,
                detailed::cast_datetime(input, order, options).map_err(Failure::fault),
                "datetime",
            );
        }
    }
}

#[test]
fn every_truncation_of_every_corpus_input_faults_in_bounds() {
    let formats = formats();
    let declared = declarations();
    let english = [(1, "Jan"), (2, "Feb"), (3, "Mar"), (4, "Apr"), (5, "May"), (6, "Jun")];
    let rest = [(7, "Jul"), (8, "Aug"), (9, "Sep"), (9, "Sept"), (10, "Oct"), (11, "Nov")];
    let names: Vec<(u8, &str)> =
        english.into_iter().chain(rest).chain([(12, "Dec"), (3, "mar."), (1, "janv.")]).collect();
    let options = [
        DateOptions::COMMON_FORMS
            .with_pivot(2029)
            .validate(Some(MonthNames::from_entries(&names)))
            .expect("valid"),
        DateOptions::NONE.with_pivot(100).validate(None).expect("valid"),
    ];
    for bytes in corpus_inputs() {
        for end in 0..=bytes.len() {
            every_dated_door(&bytes[..end], &options);
        }
        for begin in 0..=bytes.len() {
            every_dated_door(&bytes[begin..], &options);
        }
    }
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
    fn cast_detailed(
        door: u32,
        param: u32,
        format: *const RawNumFormat,
        periods: *const hypercast::RawDayPeriods,
        date_options: *const hypercast::RawDateOptions,
        vocab_ptr: *const u8,
        vocab_len: usize,
        ptr: *const u8,
        len: usize,
        out: *mut u8,
        fault: *mut hypercast::RawFaultDetail,
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
        out: *mut u8,
        fault: *mut hypercast::RawFaultDetail,
        check: *mut RawCheck,
    ) -> i32;
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
    for (field, flag, slot) in [
        ("minimum", RawConstraints::HAS_MINIMUM, 0),
        ("exclusiveMinimum", RawConstraints::HAS_EXCLUSIVE_MINIMUM, 1),
        ("maximum", RawConstraints::HAS_MAXIMUM, 2),
        ("exclusiveMaximum", RawConstraints::HAS_EXCLUSIVE_MAXIMUM, 3),
    ] {
        if let Some(value) = dec(field) {
            raw.flags |= flag;
            *(match slot {
                0 => &mut raw.minimum,
                1 => &mut raw.exclusive_minimum,
                2 => &mut raw.maximum,
                _ => &mut raw.exclusive_maximum,
            }) = value.unwrap();
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
    if c.get("caseSensitive").and_then(Value::as_bool) == Some(true) {
        raw.flags |= RawConstraints::CASE_SENSITIVE;
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
        // The detailed pair reports the same code and span, and the door alone the door's.
        let mut detail = hypercast::RawFaultDetail { size: 16, offset: 0, len: 0, detail: 0 };
        // SAFETY: as above.
        let twin = unsafe {
            cast_checked_detailed(
                door,
                0,
                format,
                raw,
                entries.as_ptr(),
                entries.len(),
                input.as_ptr(),
                input.len(),
                out.as_mut_ptr().cast(),
                &mut detail,
                &mut check,
            )
        };
        let spanned = |detail: &hypercast::RawFaultDetail| (detail.offset, detail.len);
        // SAFETY: `format` is null or a live layout.
        let western = unsafe { format.as_ref() }
            .is_none_or(|f| f.format.flags & NumFormat::GROUPING_POLICY == 0);
        if western {
            // A declared Indian is the detailed pair's alone: the 1.0 export parses Western.
            let label = String::from_utf8_lossy(input);
            assert_eq!(twin, code, "cast_checked_detailed on {label:?}");
            if code != 0 {
                assert_eq!(spanned(&detail), (fault.offset, fault.len), "span on {label:?}");
            }
        } else {
            let fault = RawFault { offset: detail.offset, len: detail.len };
            assert_exported_span(input, twin, &fault, "cast_checked_detailed");
        }
        // SAFETY: as above; `format` is null or a live layout, and nothing else is read.
        let alone = unsafe {
            cast_detailed(
                door,
                0,
                format,
                core::ptr::null(),
                core::ptr::null(),
                core::ptr::null(),
                0,
                input.as_ptr(),
                input.len(),
                out.as_mut_ptr().cast(),
                &mut detail,
            )
        };
        let fault = RawFault { offset: detail.offset, len: detail.len };
        assert_exported_span(input, alone, &fault, "cast_detailed");
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

/// A cut UTF-8 sequence at an edge is not white space: it stays in the token, and the
/// fault covers the stray byte.
#[test]
fn a_cut_sequence_at_an_edge_is_never_trimmed() {
    let spaced = NumFormat::new(',', '\u{202F}', NumFormat::ALL);
    for format in [NumFormat::INVARIANT, spaced] {
        for (input, span) in
            [(&b"12\xC2"[..], (2, 1)), (b"\xA012", (0, 1)), (b"12\xE2\x80", (2, 2))]
        {
            let malformed = |fault: Fault| {
                assert_eq!(fault.reason, hypercast::Reason::Malformed, "{input:?}");
                assert_eq!((fault.offset, fault.len), span, "{input:?}");
            };
            malformed(hypercast::cast_i32(input, &format).unwrap_err());
            malformed(hypercast::cast_f64(input, &format).unwrap_err());
            malformed(hypercast::cast_decimal(input, &format).unwrap_err());
            // The bytes at the span don't decode, and the detailed doors say so.
            let valid = format.validate().unwrap();
            let utf8 = |failure: Failure| {
                malformed(failure.fault());
                assert_eq!(failure.detail, Detail::InvalidUtf8, "{input:?}");
            };
            utf8(detailed::cast_i32(input, &valid).unwrap_err());
            utf8(detailed::cast_f64(input, &valid).unwrap_err());
            utf8(detailed::cast_decimal(input, &valid).unwrap_err());
        }
    }
}
