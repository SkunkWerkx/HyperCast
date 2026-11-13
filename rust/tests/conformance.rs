//! Replays the shared conformance corpus (`corpus/*.json` at the repository root) through
//! the public Rust API. The corpus is the byte-for-byte cross-language contract: every
//! future binding replays these same files against the same native library, so a vector
//! that drifts here is a break in the polyglot promise, not just a failing Rust test.
//!
//! Vector schema: `{ "input", "expect": "ok"|"empty"|"malformed"|"out_of_range", ... }`
//! plus a per-domain value shape on "ok" vectors, an optional `"fault": [offset, len]`
//! span assertion on failures, and `"type"`/`"format"`/`"precision"` where a door takes one.
//! Every failure names its `"detail"`. Each row is replayed through the plain door and its
//! `detailed` twin, which must agree: the same value, or the plain fault with that detail.

use hypercast::{
    CurrencySymbol, DateOptions, DateOrder, DayPeriods, DeclarationError, Detail, ExcelEpoch,
    Failure, Fault, Grouping, MonthNames, MonthNamesFault, NativeDigits, NumFormat, Reason, Symbol,
    UnixPrecision, ValidDateOptions, ValidNumFormat, detailed,
};
use serde_json::Value;
use std::path::PathBuf;

// The declaration conversion as every binding calls it, linked from the rlib as a C caller
// links it.
unsafe extern "C" {
    fn cast_declared_decimal(ptr: *const u8, len: usize, out: *mut hypercast::Decimal) -> i32;
}

/// A declaration's decimal string converted twice, by `decimal_for_declaration` and by the C
/// export `cast_declared_decimal`, which must agree: the same `Decimal`, or the export's code
/// 8 for the refusal. The Rust answer is returned.
fn declared(bytes: &[u8]) -> Result<hypercast::Decimal, hypercast::Refusal> {
    let untouched = hypercast::Decimal { lo: 0xA5A5, hi: 0x5A5A, scale: 99, negative: true };
    let mut out = untouched;
    // SAFETY: `bytes` and `out` are live for the call.
    let code = unsafe { cast_declared_decimal(bytes.as_ptr(), bytes.len(), &mut out) };
    let rust = match std::str::from_utf8(bytes) {
        Ok(text) => hypercast::decimal_for_declaration(text),
        Err(_) => Err(hypercast::Refusal::NotHeldByDoor),
    };
    match rust {
        Ok(value) => assert_eq!((code, out), (0, value), "cast_declared_decimal {bytes:?}"),
        Err(refusal) => assert_eq!(
            (code, out),
            (refusal.code() as i32, untouched),
            "cast_declared_decimal {bytes:?}: refused, `out` untouched"
        ),
    }
    rust
}

fn corpus(name: &str) -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus").join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("parsing {name}: {error}"))
}

fn input(vector: &Value) -> &str {
    vector["input"].as_str().expect("input")
}

fn expect(vector: &Value) -> &str {
    vector["expect"].as_str().expect("expect")
}

/// Asserts a failing verdict matches the vector's `expect` label and, when the vector pins
/// one, the fault span.
fn assert_failure(name: &str, vector: &Value, fault: Fault) {
    let reason = match expect(vector) {
        "empty" => Reason::Empty,
        "malformed" => Reason::Malformed,
        "out_of_range" => Reason::OutOfRange,
        other => panic!("{name}: {:?} unexpected verdict label {other}", input(vector)),
    };
    assert_eq!(fault.reason, reason, "{name}: {:?}", input(vector));
    if let Some(span) = vector.get("fault") {
        let offset = span[0].as_u64().expect("fault offset") as u32;
        let len = span[1].as_u64().expect("fault len") as u32;
        assert_eq!(
            (fault.offset, fault.len),
            (offset, len),
            "{name}: {:?} fault span",
            input(vector)
        );
    }
}

/// The detail a corpus row names.
fn detail_named(name: &str) -> Detail {
    (1..=32)
        .filter_map(Detail::from_code)
        .find(|detail| detail.name() == name)
        .unwrap_or_else(|| panic!("unknown detail {name}"))
}

/// Asserts that a row's detailed verdict is its plain verdict with a detail, and that a
/// failure's detail is the one the row names (every failure row names one; no other row does).
fn assert_detail<T: PartialEq + core::fmt::Debug>(
    name: &str,
    vector: &Value,
    plain: &Result<T, Fault>,
    detailed: Result<T, Failure>,
) {
    let named = vector.get("detail").map(|detail| detail.as_str().expect("detail"));
    match detailed {
        Ok(value) => {
            assert_eq!(plain.as_ref().ok(), Some(&value), "{name}: {:?} detailed", input(vector));
            assert_eq!(named, None, "{name}: {:?} names a detail but parsed", input(vector));
        }
        Err(failure) => {
            assert_eq!(plain, &Err(failure.fault()), "{name}: {:?} detailed", input(vector));
            let named =
                named.unwrap_or_else(|| panic!("{name}: {:?} names no detail", input(vector)));
            assert_eq!(failure.detail, detail_named(named), "{name}: {:?} detail", input(vector));
        }
    }
}

/// The option bits a row's `options` (or a `date_options` row) declares: the named flags, or
/// the raw `flag_bits` a refusal row spells.
fn flag_bits_of(options: &Value) -> u32 {
    if let Some(bits) = options.get("flag_bits") {
        return u32::try_from(bits.as_u64().expect("flag_bits")).expect("flag_bits fit u32");
    }
    options.get("flags").map_or(0, |flags| {
        flags.as_array().expect("flags").iter().fold(0, |bits, flag| {
            bits | match flag.as_str().expect("a flag name") {
                "compact_date" => DateOptions::COMPACT_DATE,
                "space_separator" => DateOptions::SPACE_SEPARATOR,
                "hour_offset" => DateOptions::HOUR_OFFSET,
                "common_forms" => DateOptions::COMMON_FORMS.flags(),
                other => panic!("unknown flag {other}"),
            }
        })
    })
}

/// A row's pivot as the C ABI carries it, a `u32`, read without truncation.
fn pivot_of(options: &Value) -> u32 {
    options.get("pivot").map_or(0, |pivot| {
        u32::try_from(pivot.as_u64().expect("pivot")).expect("a pivot fits the ABI's u32")
    })
}

/// The date options a row declares, without the vocabulary, or `None` for a pivot past
/// `u16::MAX`, which Rust's `with_pivot` can't carry (only the C ABI can, which refuses it).
fn date_options_of(options: &Value) -> Option<DateOptions> {
    let pivot = u16::try_from(pivot_of(options)).ok()?;
    Some(DateOptions::from_flags(flag_bits_of(options)).with_pivot(pivot))
}

/// A row's vocabulary entries: `[month, "name"]` pairs.
fn month_entries(options: &Value) -> Option<Vec<(u8, &str)>> {
    options.get("months").map(|months| {
        months
            .as_array()
            .expect("months")
            .iter()
            .map(|pair| {
                let month = pair[0].as_u64().expect("month");
                (u8::try_from(month).expect("month byte"), pair[1].as_str().expect("name"))
            })
            .collect()
    })
}

/// A vocabulary in its packed layout: each entry's month byte, little-endian length and
/// bytes, then any `append_bytes` a refusal row adds raw.
fn pack_months(options: &Value) -> Option<Vec<u8>> {
    let entries = month_entries(options)?;
    let mut packed = Vec::new();
    for (month, name) in entries {
        packed.push(month);
        packed.extend_from_slice(&(name.len() as u32).to_le_bytes());
        packed.extend_from_slice(name.as_bytes());
    }
    if let Some(raw) = options.get("append_bytes") {
        packed.extend(
            raw.as_array().expect("append_bytes").iter().map(|b| b.as_u64().unwrap() as u8),
        );
    }
    Some(packed)
}

/// A row with `options` through a date door, twice: with the vocabulary read from its
/// packed layout (as the C ABI hands it over) and with it built by the Rust builder. The two
/// must agree; the packed answer is returned.
fn with_options<T: PartialEq + core::fmt::Debug>(
    name: &str,
    vector: &Value,
    door: impl Fn(Option<&ValidDateOptions<'_>>) -> Result<T, Failure>,
) -> Result<T, Failure> {
    let options = &vector["options"];
    let declared = date_options_of(options).expect("a door row's pivot fits");
    let packed = pack_months(options);
    let from_packed = packed.as_deref().map(|bytes| {
        MonthNames::from_packed(bytes)
            .unwrap_or_else(|error| panic!("{name}: {:?} vocabulary: {error}", input(vector)))
    });
    let valid = declared
        .validate(from_packed)
        .unwrap_or_else(|error| panic!("{name}: {:?} options: {error}", input(vector)));
    let verdict = door(Some(&valid));

    let entries = month_entries(options);
    let built = entries.as_deref().map(MonthNames::from_entries);
    let valid = declared.validate(built).expect("the builder's options validate");
    assert_eq!(door(Some(&valid)), verdict, "{name}: {:?} the builder's vocabulary", input(vector));
    verdict
}

/// Asserts a detailed verdict from a row with `options` against the row: its value, or its
/// reason, span and detail.
fn assert_optioned<T: PartialEq + core::fmt::Debug>(
    name: &str,
    vector: &Value,
    verdict: Result<T, Failure>,
    expected: impl FnOnce(&Value) -> T,
) {
    let label = format!("{name}: {:?} {}", input(vector), vector["options"]);
    match verdict {
        Ok(value) => {
            assert_eq!(expect(vector), "ok", "{label}: unexpectedly parsed");
            assert_eq!(value, expected(vector), "{label}");
            assert!(vector.get("detail").is_none(), "{label}: names a detail but parsed");
        }
        Err(failure) => {
            assert_failure(name, vector, failure.fault());
            let named = vector.get("detail").and_then(Value::as_str);
            let named = named.unwrap_or_else(|| panic!("{label}: names no detail"));
            assert_eq!(failure.detail, detail_named(named), "{label}: detail");
        }
    }
}

/// A row's format, validated for the detailed doors.
fn valid(format: &NumFormat) -> ValidNumFormat {
    format.validate().expect("every corpus format validates")
}

fn assert_verdict<T: PartialEq + core::fmt::Debug>(
    name: &str,
    vector: &Value,
    verdict: Result<T, Fault>,
    expected: impl FnOnce(&Value) -> T,
) {
    match verdict {
        Ok(value) => {
            assert_eq!(expect(vector), "ok", "{name}: {:?} unexpectedly parsed", input(vector));
            assert_eq!(value, expected(vector), "{name}: {:?}", input(vector));
        }
        Err(fault) => assert_failure(name, vector, fault),
    }
}

/// The grouping policy a format declares: `"grouping"` is `"western"` or `"indian"`, and absent
/// means Western unless the row's `flags` carry policy bits (kept, so a `formats.json` row can
/// declare a policy no name spells).
fn grouping_of(format: &Value) -> u32 {
    match format.get("grouping").and_then(Value::as_str) {
        None => format["flags"].as_u64().expect("flags") as u32 & NumFormat::GROUPING_POLICY,
        Some("western") => NumFormat::GROUPING_WESTERN,
        Some("indian") => NumFormat::GROUPING_INDIAN,
        Some(other) => panic!("unknown grouping {other}"),
    }
}

/// The format a vector declares. Beyond the separators and flags every vector has, each
/// field is optional and absent means not declared: `currency`, the two currency
/// separators, `minus_sign`, `plus_sign`, `percent`, `digit_zero` (one character), and
/// `grouping` ([`grouping_of`]).
fn format_of(vector: &Value) -> NumFormat {
    let Some(format) = vector.get("format") else {
        return NumFormat::INVARIANT;
    };
    let symbol = |field: &str| -> Symbol<8> {
        match format.get(field).and_then(Value::as_str) {
            Some(text) => Symbol::new(text).unwrap_or_else(|| panic!("declarable {field}")),
            None => Symbol::NONE,
        }
    };
    let currency = match format.get("currency").and_then(Value::as_str) {
        Some(symbol) => CurrencySymbol::new(symbol).expect("declarable currency symbol"),
        None => CurrencySymbol::NONE,
    };
    let digits = match format.get("digit_zero").and_then(Value::as_str) {
        Some(zero) => {
            let zero = zero.chars().next().expect("a zero digit");
            NativeDigits::new(zero).expect("declarable digit zero")
        }
        None => NativeDigits::NONE,
    };
    NumFormat::with_separators(
        symbol("decimal_sep"),
        symbol("group_sep"),
        format["flags"].as_u64().expect("flags") as u32,
    )
    .with_currency(currency)
    .with_currency_separators(symbol("currency_decimal_sep"), symbol("currency_group_sep"))
    .with_signs(symbol("minus_sign"), symbol("plus_sign"))
    .with_percent(symbol("percent"))
    .with_digits(digits)
    .with_grouping(Grouping::from_code(grouping_of(format)).expect("a policy"))
}

/// The day-period designators a datetime vector declares (`am` and `pm`, together or not
/// at all).
fn periods_of(vector: &Value) -> DayPeriods {
    let designator = |field: &str| -> Symbol<16> {
        match vector.get(field).and_then(Value::as_str) {
            Some(text) => Symbol::new(text).unwrap_or_else(|| panic!("declarable {field}")),
            None => Symbol::NONE,
        }
    };
    DayPeriods::new(designator("am"), designator("pm"))
}

/// A decimal vector pins the raw triple (`magnitude` as a decimal string, `scale`,
/// `negative`) and the canonical `value` text; both must agree with the door.
/// `grouping.json`: each shape under every way to say a policy. No policy recorded, however it
/// is said, is Western; Indian only reads when declared, and either declaration replaces the
/// other. The flags rows hold what `validate` and the C ABI refuse.
#[test]
fn grouping_corpus() {
    use hypercast::detailed;
    fn holds(label: &str, want: &Value, input: &str, format: &NumFormat) {
        let (i, f, d) = (
            hypercast::cast_i64(input, format),
            hypercast::cast_f64(input, format),
            hypercast::cast_decimal(input, format),
        );
        if let Some(value) = want.get("value") {
            let value = value.as_i64().expect("value");
            assert_eq!(i, Ok(value), "{label} {input:?}");
            assert_eq!(f, Ok(value as f64), "{label} {input:?}");
            assert_eq!(d.map(|d| (d.lo, d.scale)), Ok((value as u64, 0)), "{label} {input:?}");
        } else {
            let span = want["fault"].as_array().expect("a shape holds a value or a fault");
            let span = (span[0].as_u64().unwrap() as u32, span[1].as_u64().unwrap() as u32);
            let valid = format.validate().unwrap();
            for failure in [
                detailed::cast_i64(input, &valid).unwrap_err(),
                detailed::cast_f64(input, &valid).unwrap_err(),
                detailed::cast_decimal(input, &valid).unwrap_err(),
            ] {
                assert_eq!(
                    (failure.reason, failure.detail, failure.offset, failure.len),
                    (Reason::Malformed, hypercast::Detail::GroupSize, span.0, span.1),
                    "{label} {input:?}"
                );
            }
        }
    }
    let indian = NumFormat::INVARIANT.with_grouping(Grouping::Indian);
    let mut shapes = 0;
    for row in corpus("grouping.json") {
        match row["kind"].as_str().unwrap() {
            "shape" => {
                shapes += 1;
                let text = input(&row);
                for (label, format) in [
                    ("no policy", NumFormat::new('.', ',', NumFormat::ALL)),
                    ("invariant", NumFormat::INVARIANT),
                    ("default", NumFormat::default()),
                    ("declared western", NumFormat::INVARIANT.with_grouping(Grouping::Western)),
                    ("indian then western", indian.with_grouping(Grouping::Western)),
                ] {
                    holds(label, &row["western"], text, &format);
                }
                holds("declared indian", &row["indian"], text, &indian);
            }
            "detect" => holds("detect", &row, input(&row), &NumFormat::DETECT),
            kind @ ("refused_flags" | "accepted_flags") => {
                let format = NumFormat::new('.', ',', row["flags"].as_u64().unwrap() as u32);
                let raw = hypercast::RawNumFormat::from(format);
                let refused = kind == "refused_flags";
                assert_eq!(format.check(), !refused, "{row}");
                assert_eq!(raw.resolve().is_none(), refused, "{row}");
                assert_eq!(
                    format.validate().err(),
                    refused.then_some(DeclarationError::GroupingPolicy),
                    "{row}"
                );
            }
            other => panic!("unknown kind {other}"),
        }
    }
    assert!(shapes >= 18, "{shapes} shapes");
}

#[test]
fn decimal_corpus() {
    for vector in corpus("decimal.json") {
        let text = input(&vector).as_bytes();
        let format = format_of(&vector);
        let verdict = hypercast::cast_decimal(text, &format);
        assert_detail("decimal", &vector, &verdict, detailed::cast_decimal(text, &valid(&format)));
        assert_verdict("decimal", &vector, verdict, |v| {
            let magnitude: u128 =
                v["magnitude"].as_str().expect("magnitude").parse().expect("u128");
            hypercast::Decimal {
                lo: magnitude as u64,
                hi: (magnitude >> 64) as u32,
                scale: v["scale"].as_u64().expect("scale") as u8,
                negative: v["negative"].as_bool().expect("negative"),
            }
        });
        if let Ok(value) = verdict {
            assert_eq!(
                value.to_string(),
                vector["value"].as_str().expect("value"),
                "decimal: {:?}",
                input(&vector)
            );
        }
    }
}

#[test]
fn boolean_corpus() {
    for vector in corpus("boolean.json") {
        let verdict = hypercast::cast_bool(input(&vector).as_bytes());
        assert_detail("boolean", &vector, &verdict, detailed::cast_bool(input(&vector)));
        assert_verdict("boolean", &vector, verdict, |v| v["value"].as_bool().expect("value"));
    }
}

/// A char vector pins the scalar as its code-point integer, the shape every binding
/// compares whatever its own char type.
#[test]
fn char_corpus() {
    for vector in corpus("char.json") {
        let verdict = hypercast::cast_char(input(&vector).as_bytes()).map(u32::from);
        let twin = detailed::cast_char(input(&vector)).map(u32::from);
        assert_detail("char", &vector, &verdict, twin);
        assert_verdict("char", &vector, verdict, |v| v["value"].as_u64().expect("value") as u32);
    }
}

#[test]
fn integer_corpus() {
    for vector in corpus("integer.json") {
        let text = input(&vector).as_bytes();
        let format = format_of(&vector);
        // Every width funnels through the same engine; the corpus exercises each door at
        // its own range edges and folds the value through i128 for one comparison shape.
        let checked = valid(&format);
        macro_rules! both {
            ($door:ident) => {
                (
                    hypercast::$door(text, &format).map(i128::from),
                    detailed::$door(text, &checked).map(i128::from),
                )
            };
        }
        let (verdict, twin) = match vector["type"].as_str().expect("type") {
            "i8" => both!(cast_i8),
            "i16" => both!(cast_i16),
            "i32" => both!(cast_i32),
            "i64" => both!(cast_i64),
            "u8" => both!(cast_u8),
            "u16" => both!(cast_u16),
            "u32" => both!(cast_u32),
            "u64" => both!(cast_u64),
            other => panic!("integer: unknown type {other}"),
        };
        assert_detail("integer", &vector, &verdict, twin);
        assert_verdict("integer", &vector, verdict, |v| {
            let value = &v["value"];
            value
                .as_i64()
                .map(i128::from)
                .or_else(|| value.as_u64().map(i128::from))
                .expect("value")
        });
    }
}

#[test]
fn real_corpus() {
    for vector in corpus("real.json") {
        let text = input(&vector).as_bytes();
        let format = format_of(&vector);
        // Compared by bits, each at its own width: the corpus values are chosen to be
        // exactly representable outcomes, so equality is the contract, not approximation,
        // and a zero must be the unsigned one (the core delivers no negative zero).
        let checked = valid(&format);
        match vector["type"].as_str().expect("type") {
            "f32" => {
                let verdict = hypercast::cast_f32(text, &format).map(f32::to_bits);
                let twin = detailed::cast_f32(text, &checked).map(f32::to_bits);
                assert_detail("real", &vector, &verdict, twin);
                assert_verdict("real", &vector, verdict, |v| {
                    (v["value"].as_f64().expect("value") as f32).to_bits()
                })
            }
            "f64" => {
                let verdict = hypercast::cast_f64(text, &format).map(f64::to_bits);
                let twin = detailed::cast_f64(text, &checked).map(f64::to_bits);
                assert_detail("real", &vector, &verdict, twin);
                assert_verdict("real", &vector, verdict, |v| {
                    v["value"].as_f64().expect("value").to_bits()
                })
            }
            other => panic!("real: unknown type {other}"),
        }
    }
}

#[test]
fn uuid_corpus() {
    for vector in corpus("uuid.json") {
        let verdict = hypercast::cast_uuid(input(&vector).as_bytes());
        assert_detail("uuid", &vector, &verdict, detailed::cast_uuid(input(&vector)));
        assert_verdict("uuid", &vector, verdict, |v| {
            let hex = v["value"].as_str().expect("value");
            let mut bytes = [0u8; 16];
            for (i, slot) in bytes.iter_mut().enumerate() {
                *slot = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("hex value");
            }
            bytes
        });
    }
}

fn timestamp_of(vector: &Value) -> hypercast::Timestamp {
    hypercast::Timestamp {
        seconds: vector["seconds"].as_i64().expect("seconds"),
        nanos: vector["nanos"].as_i64().expect("nanos") as i32,
    }
}

#[test]
fn timestamp_corpus() {
    for vector in corpus("timestamp.json") {
        if vector.get("options").is_some() {
            let verdict = with_options("timestamp", &vector, |options| {
                detailed::cast_timestamp(input(&vector), options)
            });
            assert_optioned("timestamp", &vector, verdict, timestamp_of);
            continue;
        }
        let verdict = hypercast::cast_timestamp(input(&vector).as_bytes());
        assert_detail(
            "timestamp",
            &vector,
            &verdict,
            detailed::cast_timestamp(input(&vector), None),
        );
        assert_verdict("timestamp", &vector, verdict, timestamp_of);
    }
}

#[test]
fn unix_corpus() {
    for vector in corpus("unix.json") {
        let precision = match vector["precision"].as_u64().expect("precision") {
            1 => UnixPrecision::Seconds,
            2 => UnixPrecision::Millis,
            3 => UnixPrecision::Micros,
            4 => UnixPrecision::Nanos,
            other => panic!("unix: unknown precision {other}"),
        };
        let verdict = hypercast::cast_unix(input(&vector).as_bytes(), precision);
        assert_detail("unix", &vector, &verdict, detailed::cast_unix(input(&vector), precision));
        assert_verdict("unix", &vector, verdict, timestamp_of);
    }
}

#[test]
fn excel_serial_corpus() {
    for vector in corpus("excel_serial.json") {
        let epoch = match vector["epoch"].as_u64().expect("epoch") {
            1 => ExcelEpoch::Y1900,
            2 => ExcelEpoch::Y1904,
            other => panic!("excel_serial: unknown epoch {other}"),
        };
        let verdict = hypercast::cast_excel_serial(input(&vector).as_bytes(), epoch);
        let twin = detailed::cast_excel_serial(input(&vector), epoch);
        assert_detail("excel_serial", &vector, &verdict, twin);
        assert_verdict("excel_serial", &vector, verdict, timestamp_of);
    }
}

/// The typed door reads the same corpus the text door does. Every vector whose input is a
/// plain decimal is parsed to the `f64` a workbook would have stored and must land on the
/// same verdict: the same reason on failure, and on success the same instant to within a
/// microsecond — a double near serial 45,000 resolves about 0.6 µs, so the text door's
/// exact decimal digits are not all there to compare.
#[test]
fn excel_serial_typed_agrees_with_the_text_door() {
    let mut replayed = 0;
    for vector in corpus("excel_serial.json") {
        let text = input(&vector).trim();
        let plain = !text.is_empty()
            && text.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.')
            && text.bytes().filter(|&byte| byte == b'.').count() <= 1
            && !text.starts_with('.')
            && !text.ends_with('.');
        if !plain {
            continue;
        }
        let epoch = match vector["epoch"].as_u64().expect("epoch") {
            1 => ExcelEpoch::Y1900,
            2 => ExcelEpoch::Y1904,
            other => panic!("excel_serial: unknown epoch {other}"),
        };
        let serial: f64 = text.parse().expect("a plain decimal");
        let verdict = hypercast::excel_serial(serial, epoch);
        match expect(&vector) {
            "ok" => {
                let expected = timestamp_of(&vector);
                let actual = verdict
                    .unwrap_or_else(|reason| panic!("excel_serial({text}): {reason:?}"))
                    .assume_utc();
                let nanos = |t: hypercast::Timestamp| {
                    i128::from(t.seconds) * 1_000_000_000 + i128::from(t.nanos)
                };
                let drift = (nanos(actual) - nanos(expected)).abs();
                assert!(drift <= 1_000, "excel_serial({text}): {actual:?} vs {expected:?}");
                assert_eq!(actual.utc_civil(), verdict.ok(), "excel_serial({text}) round trip");
            }
            "out_of_range" => assert_eq!(verdict, Err(Reason::OutOfRange), "excel_serial({text})"),
            other => panic!("excel_serial({text}): a plain decimal expecting {other}"),
        }
        replayed += 1;
    }
    assert!(replayed >= 15, "only {replayed} excel_serial vectors were plain decimals");

    // What the text door rejects at the sign or cannot spell at all.
    assert_eq!(hypercast::excel_serial(-1.0, ExcelEpoch::Y1900), Err(Reason::Malformed));
    assert_eq!(hypercast::excel_serial(f64::NAN, ExcelEpoch::Y1904), Err(Reason::Malformed));
    assert_eq!(hypercast::excel_serial(f64::INFINITY, ExcelEpoch::Y1900), Err(Reason::Malformed));
    // A time-only cell has no date in the 1900 system, and is 1904-01-01 in the other.
    assert_eq!(hypercast::excel_serial(0.5, ExcelEpoch::Y1900), Err(Reason::OutOfRange));
    assert_eq!(
        hypercast::excel_serial(0.5, ExcelEpoch::Y1904),
        Ok(hypercast::CivilDateTime {
            date: hypercast::Date { year: 1904, month: 1, day: 1 },
            nanos_of_day: 43_200_000_000_000,
        })
    );
    // A fraction within half a nanosecond of a whole day rounds up to it and carries into
    // the date. Only a small serial can sit that close: one ulp under 45,000 is already
    // 629 ns short of midnight, and stays on its own day — snapped to the 100 ns its
    // midpoints admit, 600 ns short.
    assert_eq!(
        hypercast::excel_serial(2.0 - f64::EPSILON, ExcelEpoch::Y1900),
        Ok(hypercast::CivilDateTime {
            date: hypercast::Date { year: 1900, month: 1, day: 2 },
            nanos_of_day: 0,
        })
    );
    assert_eq!(
        hypercast::excel_serial(45_000.0 - 45_000.0 * f64::EPSILON / 2.0, ExcelEpoch::Y1900),
        Ok(hypercast::CivilDateTime {
            date: hypercast::Date { year: 2023, month: 3, day: 14 },
            nanos_of_day: 86_399_999_999_400,
        })
    );
}

fn date_of(vector: &Value) -> hypercast::Date {
    hypercast::Date {
        year: vector["year"].as_u64().expect("year") as u16,
        month: vector["month"].as_u64().expect("month") as u8,
        day: vector["day"].as_u64().expect("day") as u8,
    }
}

#[test]
fn date_corpus() {
    for vector in corpus("date.json") {
        if vector.get("options").is_some() {
            let verdict = with_options("date", &vector, |options| {
                detailed::cast_date(input(&vector), options)
            });
            assert_optioned("date", &vector, verdict, date_of);
            continue;
        }
        let verdict = hypercast::cast_date(input(&vector).as_bytes());
        assert_detail("date", &vector, &verdict, detailed::cast_date(input(&vector), None));
        assert_verdict("date", &vector, verdict, date_of);
    }
}

#[test]
fn datetime_corpus() {
    for vector in corpus("datetime.json") {
        let order = match vector["order"].as_u64().expect("order") {
            1 => DateOrder::YearMonthDay,
            2 => DateOrder::MonthDayYear,
            3 => DateOrder::DayMonthYear,
            other => panic!("datetime: unknown order {other}"),
        };
        let periods = periods_of(&vector);
        let civil = |v: &Value| hypercast::CivilDateTime {
            date: date_of(v),
            nanos_of_day: v["nanos_of_day"].as_u64().expect("nanos_of_day"),
        };
        if vector.get("options").is_some() {
            let verdict = with_options("datetime", &vector, |options| {
                detailed::cast_datetime_with(input(&vector), order, &periods, options)
            });
            assert_optioned("datetime", &vector, verdict, civil);
            continue;
        }
        let verdict = hypercast::cast_datetime_with(input(&vector).as_bytes(), order, &periods);
        let twin = detailed::cast_datetime_with(input(&vector), order, &periods, None);
        assert_detail("datetime", &vector, &verdict, twin);
        assert_verdict("datetime", &vector, verdict, civil);
    }
}

#[test]
fn date_order_corpus() {
    for vector in corpus("date_order.json") {
        let order = match vector["order"].as_u64().expect("order") {
            1 => DateOrder::YearMonthDay,
            2 => DateOrder::MonthDayYear,
            3 => DateOrder::DayMonthYear,
            other => panic!("date_order: unknown order {other}"),
        };
        if vector.get("options").is_some() {
            let verdict = with_options("date_order", &vector, |options| {
                detailed::cast_date_ordered(input(&vector), order, options)
            });
            assert_optioned("date_order", &vector, verdict, date_of);
            continue;
        }
        let verdict = hypercast::cast_date_ordered(input(&vector).as_bytes(), order);
        let twin = detailed::cast_date_ordered(input(&vector), order, None);
        assert_detail("date_order", &vector, &verdict, twin);
        assert_verdict("date_order", &vector, verdict, date_of);
    }
}

#[test]
fn time_corpus() {
    for vector in corpus("time.json") {
        let verdict = hypercast::cast_time(input(&vector).as_bytes());
        assert_detail("time", &vector, &verdict, detailed::cast_time(input(&vector)));
        assert_verdict("time", &vector, verdict, |v| v["nanos"].as_u64().expect("nanos"));
    }
}

#[test]
fn duration_corpus() {
    for vector in corpus("duration.json") {
        let verdict = hypercast::cast_duration(input(&vector).as_bytes());
        assert_detail("duration", &vector, &verdict, detailed::cast_duration(input(&vector)));
        assert_verdict("duration", &vector, verdict, |v| hypercast::Duration {
            seconds: v["seconds"].as_i64().expect("seconds"),
            nanos: v["nanos"].as_i64().expect("nanos") as i32,
        });
    }
}

/// A typed door's detailed twin: no span, and otherwise as [`assert_detail`] says.
fn typed_twin<T: PartialEq + core::fmt::Debug>(
    vector: &Value,
    plain: &Result<T, Fault>,
    detailed: Result<T, Failure>,
) {
    if let Err(failure) = &detailed {
        assert_eq!((failure.offset, failure.len), (0, 0), "typed {:?}: spanless", input(vector));
    }
    assert_detail("typed", vector, plain, detailed);
}

/// The typed doors, replayed by bits: `corpus/typed.json` names each double by its IEEE 754
/// pattern (`bits`, hex), which every language can rebuild exactly — NaN and the infinities
/// included, which JSON cannot spell — and carries `input` only for the reader. A typed
/// door's fault has no span.
#[test]
fn typed_corpus() {
    let mut doors = std::collections::BTreeSet::new();
    for vector in corpus("typed.json") {
        let bits = u64::from_str_radix(vector["bits"].as_str().expect("bits"), 16).expect("hex");
        let value = f64::from_bits(bits);
        let door = vector["door"].as_str().expect("door");
        let spanned = |reason| Fault { reason, offset: 0, len: 0 };
        match door {
            "decimal" => {
                let verdict = hypercast::decimal_from_f64(value).map_err(spanned);
                typed_twin(&vector, &verdict, detailed::decimal_from_f64(value));
                assert_verdict("typed decimal", &vector, verdict, |v| {
                    let magnitude: u128 =
                        v["magnitude"].as_str().expect("magnitude").parse().expect("u128");
                    hypercast::Decimal {
                        lo: magnitude as u64,
                        hi: (magnitude >> 64) as u32,
                        scale: v["scale"].as_u64().expect("scale") as u8,
                        negative: v["negative"].as_bool().expect("negative"),
                    }
                });
                if let Ok(decimal) = verdict {
                    assert_eq!(decimal.to_string(), vector["value"].as_str().expect("value"));
                }
            }
            "excel_serial" => {
                let epoch = match vector["epoch"].as_u64().expect("epoch") {
                    1 => ExcelEpoch::Y1900,
                    2 => ExcelEpoch::Y1904,
                    other => panic!("typed excel_serial: unknown epoch {other}"),
                };
                let verdict = hypercast::excel_serial(value, epoch).map_err(spanned);
                typed_twin(&vector, &verdict, detailed::excel_serial(value, epoch));
                assert_verdict("typed excel_serial", &vector, verdict, |v| {
                    hypercast::CivilDateTime {
                        date: hypercast::Date {
                            year: v["year"].as_u64().expect("year") as u16,
                            month: v["month"].as_u64().expect("month") as u8,
                            day: v["day"].as_u64().expect("day") as u8,
                        },
                        nanos_of_day: v["nanos_of_day"].as_u64().expect("nanos_of_day"),
                    }
                });
            }
            "excel_time" => {
                let verdict = hypercast::excel_time(value).map_err(spanned);
                typed_twin(&vector, &verdict, detailed::excel_time(value));
                assert_verdict("typed excel_time", &vector, verdict, |v| {
                    v["nanos"].as_u64().expect("nanos")
                });
            }
            "excel_duration" => {
                let verdict = hypercast::excel_duration(value).map_err(spanned);
                typed_twin(&vector, &verdict, detailed::excel_duration(value));
                assert_verdict("typed excel_duration", &vector, verdict, |v| hypercast::Duration {
                    seconds: v["seconds"].as_i64().expect("seconds"),
                    nanos: v["nanos"].as_i64().expect("nanos") as i32,
                });
            }
            other => panic!("typed: unknown door {other}"),
        }
        doors.insert(door.to_owned());
    }
    assert_eq!(doors.len(), 4, "every typed door has vectors");
}

/// The typed doors only Rust has (the integers, `f32`, `bool` and Unix), replayed by bits from
/// `rust/tests/typed_rust.json`, which no binding reads: the plain door and its detailed twin
/// agree, and each failure names its detail.
#[test]
fn rust_only_typed_doors() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/typed_rust.json");
    let rows: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let spanned = |reason| Fault { reason, offset: 0, len: 0 };
    for vector in rows {
        let bits = u64::from_str_radix(vector["bits"].as_str().expect("bits"), 16).expect("hex");
        let value = f64::from_bits(bits);
        macro_rules! integer {
            ($plain:ident, $ty:ty) => {{
                let verdict = hypercast::$plain(value).map_err(spanned);
                typed_twin(&vector, &verdict, detailed::$plain(value));
                assert_verdict("typed", &vector, verdict, |v| {
                    v["value"].as_i64().expect("value") as $ty
                });
            }};
        }
        match vector["door"].as_str().expect("door") {
            "i8" => integer!(i8_from_f64, i8),
            "i16" => integer!(i16_from_f64, i16),
            "i32" => integer!(i32_from_f64, i32),
            "i64" => integer!(i64_from_f64, i64),
            "u8" => integer!(u8_from_f64, u8),
            "u16" => integer!(u16_from_f64, u16),
            "u32" => integer!(u32_from_f64, u32),
            "u64" => integer!(u64_from_f64, u64),
            "f32" => {
                let verdict = hypercast::f32_from_f64(value).map(f32::to_bits).map_err(spanned);
                let twin = detailed::f32_from_f64(value).map(f32::to_bits);
                typed_twin(&vector, &verdict, twin);
                assert_verdict("typed", &vector, verdict, |v| {
                    (v["value"].as_f64().expect("value") as f32).to_bits()
                });
            }
            "bool" => {
                let verdict = hypercast::bool_from_f64(value).map_err(spanned);
                typed_twin(&vector, &verdict, detailed::bool_from_f64(value));
                assert_verdict("typed", &vector, verdict, |v| v["value"].as_bool().expect("value"));
            }
            "unix" => {
                let seconds = UnixPrecision::Seconds;
                let verdict = hypercast::unix_from_f64(value, seconds).map_err(spanned);
                typed_twin(&vector, &verdict, detailed::unix_from_f64(value, seconds));
                assert_verdict("typed", &vector, verdict, timestamp_of);
            }
            other => panic!("typed_rust: unknown door {other}"),
        }
    }
}

/// The declaration conversion (`corpus/declarations.json`): each `input` (or `input_bytes`,
/// bytes that are not UTF-8) converted by `decimal_for_declaration` and the C export
/// `cast_declared_decimal`, which must agree. An `ok` row pins the `Decimal` field by field
/// (`magnitude`, `scale`, `negative`) and its canonical `value` text; a `refused` row pins
/// the export's code, the `Refusal`'s. Every truncation of every input is converted both
/// ways as well, and must agree.
#[test]
fn declarations() {
    let mut refused = 0;
    for vector in corpus("declarations.json") {
        let bytes: Vec<u8> = match vector.get("input_bytes") {
            Some(bytes) => bytes
                .as_array()
                .expect("input_bytes")
                .iter()
                .map(|b| b.as_u64().expect("a byte") as u8)
                .collect(),
            None => input(&vector).as_bytes().to_vec(),
        };
        let label = format!("declarations {:?}", String::from_utf8_lossy(&bytes));
        // Every prefix and suffix too, cut mid-character included: `declared` asserts that
        // the two conversions agree there, whatever the answer.
        for cut in 0..bytes.len() {
            let _ = (declared(&bytes[..cut]), declared(&bytes[cut..]));
        }
        match (expect(&vector), declared(&bytes)) {
            ("ok", Ok(value)) => {
                let magnitude: u128 =
                    vector["magnitude"].as_str().expect("magnitude").parse().expect("u128");
                let expected = hypercast::Decimal {
                    lo: magnitude as u64,
                    hi: (magnitude >> 64) as u32,
                    scale: vector["scale"].as_u64().expect("scale") as u8,
                    negative: vector["negative"].as_bool().expect("negative"),
                };
                assert_eq!(value, expected, "{label}");
                assert_eq!(value.to_string(), vector["value"].as_str().expect("value"), "{label}");
            }
            ("refused", Err(refusal)) => {
                assert_eq!(u64::from(refusal.code()), vector["refusal"].as_u64().expect("refusal"));
                refused += 1;
            }
            (label_expect, verdict) => panic!("{label}: expected {label_expect}, got {verdict:?}"),
        }
    }
    assert!(refused > 0, "declarations: no refused vectors");
}

unsafe extern "C" {
    fn cast_i32(
        ptr: *const u8,
        len: usize,
        format: *const hypercast::RawNumFormat,
        out: *mut i32,
        fault: *mut [u32; 2],
    ) -> i32;
    fn cast_datetime(
        ptr: *const u8,
        len: usize,
        order: u32,
        periods: *const hypercast::RawDayPeriods,
        out: *mut hypercast::CivilDateTime,
        fault: *mut [u32; 2],
    ) -> i32;
    fn cast_f64(
        ptr: *const u8,
        len: usize,
        format: *const hypercast::RawNumFormat,
        out: *mut f64,
        fault: *mut [u32; 2],
    ) -> i32;
    fn cast_decimal(
        ptr: *const u8,
        len: usize,
        format: *const hypercast::RawNumFormat,
        out: *mut hypercast::Decimal,
        fault: *mut [u32; 2],
    ) -> i32;
    fn cast_detailed(
        door: u32,
        param: u32,
        format: *const hypercast::RawNumFormat,
        periods: *const hypercast::RawDayPeriods,
        date_options: *const hypercast::RawDateOptions,
        vocab_ptr: *const u8,
        vocab_len: usize,
        ptr: *const u8,
        len: usize,
        out: *mut core::ffi::c_void,
        fault: *mut hypercast::RawFaultDetail,
    ) -> i32;
    fn cast_checked(
        door: u32,
        param: u32,
        format: *const hypercast::RawNumFormat,
        constraints: *const hypercast::RawConstraints,
        enum_ptr: *const u8,
        enum_len: usize,
        ptr: *const u8,
        len: usize,
        out: *mut core::ffi::c_void,
        fault: *mut [u32; 2],
        check: *mut hypercast::RawCheck,
    ) -> i32;
}

/// The 1.0 exports read only bits 0-7 of `flags` and always parse with Western grouping,
/// whatever policy, refused policy or undefined bit the caller's layout carries; the Rust
/// door given the same format honours a valid policy.
#[test]
fn old_exports_parse_with_western_grouping_whatever_the_flags_say() {
    let input = b"1,234,567";
    for flags in [
        NumFormat::ALL | NumFormat::GROUPING_WESTERN,
        NumFormat::ALL | NumFormat::GROUPING_INDIAN,
        NumFormat::ALL | NumFormat::GROUPING_POLICY,
        NumFormat::ALL | 1 << 10,
        NumFormat::ALL | 1 << 31,
        NumFormat::ALL | 0xFFFF_FF00,
    ] {
        let raw = hypercast::RawNumFormat {
            size: hypercast::RawNumFormat::SIZE,
            format: NumFormat::new('.', ',', flags),
        };
        let mut fault = [0u32; 2];
        let (mut int, mut real) = (0i32, 0f64);
        let mut decimal = hypercast::Decimal { lo: 0, hi: 0, scale: 0, negative: false };
        // SAFETY: every pointer is live for its call.
        let codes = unsafe {
            [
                cast_i32(input.as_ptr(), input.len(), &raw, &mut int, &mut fault),
                cast_f64(input.as_ptr(), input.len(), &raw, &mut real, &mut fault),
                cast_decimal(input.as_ptr(), input.len(), &raw, &mut decimal, &mut fault),
            ]
        };
        assert_eq!(codes, [0, 0, 0], "flags {flags:#x}");
        assert_eq!(
            (int, real, decimal.lo, decimal.scale),
            (1234567, 1234567.0, 1234567, 0),
            "flags {flags:#x}"
        );

        let constraints = hypercast::RawConstraints::ZERO;
        let mut check = hypercast::RawCheck { keyword: 9, adjusted: 9, enum_index: 9 };
        let mut out = 0i32;
        // SAFETY: every pointer is live for the call; `out` holds the i32 door's value.
        let code = unsafe {
            cast_checked(
                hypercast::Door::I32.code(),
                0,
                &raw,
                &constraints,
                core::ptr::null(),
                0,
                input.as_ptr(),
                input.len(),
                (&raw mut out).cast(),
                &mut fault,
                &mut check,
            )
        };
        assert_eq!((code, out), (0, 1234567), "cast_checked, flags {flags:#x}");
        // The layout the caller built is left as it was.
        assert_eq!(raw.format.flags, flags);
    }
    // The Rust door honours the declared policy the export ignores.
    let indian = NumFormat::INVARIANT.with_grouping(Grouping::Indian);
    assert_eq!(hypercast::cast_i32(input, &indian).map_err(|f| f.reason), Err(Reason::Malformed));
}

/// A symbol as the ABI lays it out: `capacity` bytes of UTF-8 and then one length byte. A
/// symbol too long for its field keeps the bytes that fit and its true length, which is what
/// a caller's over-long symbol looks like to the core.
fn symbol_field(text: Option<&str>, capacity: usize) -> Vec<u8> {
    let text = text.unwrap_or("").as_bytes();
    let mut field = vec![0u8; capacity + 1];
    let kept = text.len().min(capacity);
    field[..kept].copy_from_slice(&text[..kept]);
    field[capacity] = text.len() as u8;
    field
}

/// The `RawNumFormat` a `num_format` row declares, built from the layout's bytes so that a
/// declaration no constructor accepts still reaches `check` and the C ABI.
fn raw_format_of(row: &Value) -> hypercast::RawNumFormat {
    let format = &row["format"];
    let text = |field: &str| format.get(field).and_then(Value::as_str);
    let zero =
        text("digit_zero").map_or(0, |zero| zero.chars().next().expect("a zero digit") as u32);
    let mut bytes = Vec::with_capacity(92);
    bytes.extend_from_slice(&hypercast::RawNumFormat::SIZE.to_le_bytes());
    let flags = format["flags"].as_u64().expect("flags") as u32;
    let flags = flags & !NumFormat::GROUPING_POLICY | grouping_of(format);
    bytes.extend_from_slice(&flags.to_le_bytes());
    bytes.extend_from_slice(&zero.to_le_bytes());
    for field in [
        "decimal_sep",
        "group_sep",
        "currency_decimal_sep",
        "currency_group_sep",
        "minus_sign",
        "plus_sign",
        "percent",
    ] {
        bytes.extend_from_slice(&symbol_field(text(field), 8));
    }
    bytes.extend_from_slice(&symbol_field(text("currency"), 16));
    hypercast::RawNumFormat::from_le_bytes(bytes.try_into().expect("92 bytes"))
}

/// The `RawDayPeriods` a `day_periods` row declares, built the same way.
fn raw_periods_of(row: &Value) -> hypercast::RawDayPeriods {
    let mut bytes = Vec::with_capacity(40);
    bytes.extend_from_slice(&hypercast::RawDayPeriods::SIZE.to_ne_bytes());
    for field in ["am", "pm"] {
        bytes.extend_from_slice(&symbol_field(row.get(field).and_then(Value::as_str), 16));
    }
    bytes.extend_from_slice(&[0, 0]);
    let bytes: [u8; 40] = bytes.try_into().expect("40 bytes");
    // SAFETY: `RawDayPeriods` is `#[repr(C)]`, 40 bytes of a `u32` and `u8`s, and every bit
    // pattern is a value of it (a symbol's length is checked by `check`, never trusted).
    unsafe { core::mem::transmute::<[u8; 40], hypercast::RawDayPeriods>(bytes) }
}

/// The declaration refusals and the white-space table (`corpus/formats.json`), three kinds
/// of row. `white_space` pins `is_white_space` for one code point. `num_format` and
/// `day_periods` declare a format or designators (the field names `format_of` and
/// `periods_of` read) and expect `ok` or `refused` with the `rule` that refuses it: the first
/// that applies of `grouping_policy` and `unknown_flag` (1.1's rules on `flags`), then
/// `length`, `digit`, `equal_separators` and `white_space` (`currency_edge` for a currency
/// symbol, `designator_edge` for designators). Each declaration is judged by
/// `NumFormat::check` and `NumFormat::validate` (which names the rule's
/// `DeclarationError`) / `DayPeriods::check`, and by the C ABI, where a refused declaration is
/// a contract violation (`-1`) even for empty input, and an accepted one is not; the 1.0
/// export reads only bits 0-7 of `flags`, so the two 1.1 rules are no violation there.
#[test]
fn formats() {
    const RULES: [&str; 17] = [
        "length",
        "digit",
        "equal_separators",
        "white_space",
        "currency_edge",
        "designator_edge",
        "grouping_policy",
        "unknown_flag",
        "flags",
        "pivot",
        "layout",
        "month",
        "utf8",
        "name_length",
        "character",
        "month_missing",
        "duplicate",
    ];
    // Every row is judged, and every disagreement reported together.
    let mut failures: Vec<String> = Vec::new();
    macro_rules! agree {
        ($got:expr, $want:expr, $($label:tt)+) => {
            let (got, want) = ($got, $want);
            if got != want {
                failures.push(format!("{}: got {got:?}, want {want:?}", format!($($label)+)));
            }
        };
    }
    let (mut white_space, mut num_format, mut day_periods, mut refused, mut accepted) =
        (0, 0, 0, 0, 0);
    let (mut date_options, mut month_names) = (0, 0);
    for row in corpus("formats.json") {
        let refused_row = match row.get("expect").and_then(Value::as_str) {
            Some("ok") => false,
            Some("refused") => {
                let rule = row["rule"].as_str().expect("a refused row carries its rule");
                assert!(RULES.contains(&rule), "unknown rule {rule}");
                true
            }
            None => false,
            Some(other) => panic!("unknown expect {other}"),
        };
        match row["kind"].as_str().expect("kind") {
            "white_space" => {
                let code = row["code_point"].as_str().expect("code_point");
                let c = char::from_u32(
                    u32::from_str_radix(code.strip_prefix("U+").expect("U+ prefix"), 16)
                        .expect("hex"),
                )
                .expect("a scalar value");
                let expected = row["white_space"].as_bool().expect("white_space");
                agree!(hypercast::is_white_space(c), expected, "is_white_space({code})");
                white_space += 1;
            }
            "num_format" => {
                let label = format!("num_format {}", row["format"]);
                let raw = raw_format_of(&row);
                agree!(raw.format.check(), !refused_row, "NumFormat::check, {label}");
                agree!(raw.resolve().is_some(), !refused_row, "resolve, {label}");
                let declared = match row.get("rule").and_then(Value::as_str) {
                    None => None,
                    Some("grouping_policy") => Some(DeclarationError::GroupingPolicy),
                    Some("unknown_flag") => Some(DeclarationError::UnknownFlag),
                    Some(_) => Some(DeclarationError::NumFormat),
                };
                agree!(raw.format.validate().err(), declared, "NumFormat::validate, {label}");
                // The 1.0 exports read only bits 0-7 of `flags`, so a policy or flag only 1.1
                // judges is no contract violation there.
                let flags_only_1_1_judges = matches!(
                    declared,
                    Some(DeclarationError::GroupingPolicy | DeclarationError::UnknownFlag)
                );
                let (mut out, mut fault) = (0i32, [0u32; 2]);
                // SAFETY: `raw`, `out` and `fault` are live for the call; empty input.
                let code = unsafe { cast_i32(b"".as_ptr(), 0, &raw, &mut out, &mut fault) };
                agree!(
                    code == -1,
                    refused_row && !flags_only_1_1_judges,
                    "cast_i32 code {code}, {label}"
                );
                if !refused_row {
                    let built = format_of(&row);
                    agree!(built.check(), true, "format_of builds a checked format, {label}");
                    agree!(built, raw.format, "format_of agrees with the layout, {label}");
                }
                num_format += 1;
            }
            "day_periods" => {
                let label = format!("day_periods {row}");
                let raw = raw_periods_of(&row);
                agree!(raw.periods.check(), !refused_row, "DayPeriods::check, {label}");
                agree!(raw.resolve().is_some(), !refused_row, "resolve, {label}");
                // SAFETY: `CivilDateTime` is plain integers, zero a valid value.
                let mut out: hypercast::CivilDateTime = unsafe { core::mem::zeroed() };
                let mut fault = [0u32; 2];
                // SAFETY: `raw`, `out` and `fault` are live for the call; empty input.
                let code = unsafe { cast_datetime(b"".as_ptr(), 0, 1, &raw, &mut out, &mut fault) };
                agree!(code == -1, refused_row, "cast_datetime code {code}, {label}");
                if !refused_row {
                    agree!(periods_of(&row), raw.periods, "periods_of agrees, {label}");
                }
                day_periods += 1;
            }
            "date_options" => {
                let label = format!("date_options {row}");
                // The C ABI judges the layout as the row spells it, the pivot a whole `u32`.
                let raw = hypercast::RawDateOptions {
                    size: hypercast::RawDateOptions::SIZE,
                    flags: flag_bits_of(&row),
                    pivot: pivot_of(&row),
                };
                let mut out = 0u32;
                let mut fault =
                    hypercast::RawFaultDetail { size: 16, offset: 0, len: 0, detail: 0 };
                // SAFETY: every pointer is live for the call; empty input at `cast_date`.
                let code = unsafe {
                    cast_detailed(
                        hypercast::Door::Date.code(),
                        0,
                        core::ptr::null(),
                        core::ptr::null(),
                        &raw,
                        core::ptr::null(),
                        0,
                        b"".as_ptr(),
                        0,
                        (&raw mut out).cast(),
                        &mut fault,
                    )
                };
                agree!(code == -1, refused_row, "cast_detailed code {code}, {label}");
                // A pivot past `u16::MAX` can't reach Rust's `with_pivot`: the ABI refused it.
                let verdict = match date_options_of(&row) {
                    Some(declared) => declared.validate(None),
                    None => Err(DeclarationError::DatePivot),
                };
                agree!(verdict.is_ok(), !refused_row, "validate, {label}");
                match verdict {
                    Ok(valid) => {
                        let declared = date_options_of(&row).expect("validated");
                        agree!(valid.flags(), flag_bits_of(&row), "flags, {label}");
                        agree!(valid.pivot(), declared.pivot(), "pivot, {label}");
                        agree!(valid.months().is_none(), true, "no vocabulary, {label}");
                    }
                    Err(error) => {
                        let rule = row["rule"].as_str().expect("rule");
                        let want = match rule {
                            "flags" => DeclarationError::DateFlags,
                            "pivot" => DeclarationError::DatePivot,
                            other => panic!("rule {other} for date_options"),
                        };
                        agree!(error, want, "refusal, {label}");
                    }
                }
                date_options += 1;
            }
            "month_names" => {
                let label = format!("month_names {row}");
                let packed = pack_months(&row).expect("months");
                let from_packed = MonthNames::from_packed(&packed);
                agree!(from_packed.is_ok(), !refused_row, "from_packed, {label}");
                let validated =
                    from_packed.and_then(|names| DateOptions::NONE.validate(Some(names)));
                agree!(validated.is_ok(), !refused_row, "validate, {label}");
                let refusal = validated.as_ref().err().copied();
                if refused_row {
                    let want = match row["rule"].as_str().expect("rule") {
                        "layout" => MonthNamesFault::Layout,
                        "month" => MonthNamesFault::Month,
                        "utf8" => MonthNamesFault::Utf8,
                        "name_length" => MonthNamesFault::NameLength,
                        "character" => MonthNamesFault::Character,
                        "month_missing" => MonthNamesFault::MonthMissing,
                        "duplicate" => MonthNamesFault::Duplicate,
                        other => panic!("rule {other} for month_names"),
                    };
                    agree!(refusal, Some(DeclarationError::MonthNames(want)), "{label}");
                }
                // The builder judges what the packed layout holds, when it can hold it.
                if row.get("append_bytes").is_none() {
                    let entries = month_entries(&row).expect("months");
                    let built =
                        DateOptions::NONE.validate(Some(MonthNames::from_entries(&entries)));
                    agree!(built.is_ok(), !refused_row, "the builder, {label}");
                    if refused_row {
                        agree!(built.err(), refusal, "the builder's refusal, {label}");
                    } else {
                        // An accepted vocabulary packs back to the bytes it was read from.
                        let names = MonthNames::from_packed(&packed).expect("accepted");
                        let mut out = vec![0; packed.len()];
                        agree!(names.pack(&mut out), Some(packed.len()), "pack, {label}");
                        agree!(out == packed, true, "pack round trip, {label}");
                        agree!(
                            MonthNames::from_entries(&entries).pack(&mut []),
                            Some(packed.len()),
                            "the builder packs the same length, {label}"
                        );
                    }
                }
                month_names += 1;
            }
            kind => panic!("unknown kind {kind}"),
        }
        if row["kind"] != "white_space" {
            if refused_row { refused += 1 } else { accepted += 1 }
        }
    }
    assert!(failures.is_empty(), "{} rows disagree:\n{}", failures.len(), failures.join("\n"));
    assert!(white_space > 0 && num_format > 0 && day_periods > 0, "every kind appears");
    assert!(date_options > 0 && month_names > 0, "the date kinds appear");
    assert!(refused > 0 && accepted > 0, "both outcomes appear");
}

/// The checked doors (`corpus/constraints.json`). Each vector names its door, its
/// `constraints` (bounds, steps and integer `enum` entries as decimal strings, converted by
/// `decimal_for_declaration` and by the C export `cast_declared_decimal`, which must agree,
/// and whose refusal is a refused row's answer), and one of `input`,
/// `input_bytes` (text that is not UTF-8) or `from_f64`. Every row's declaration is first
/// judged by `check_constraints` in its packed ABI form, which must refuse exactly the
/// `refused` rows with their code; every other row is then built through the typed
/// `Constraints` builder and replayed through `cast_checked`, `checked_from_f64` or
/// `cast_text_checked`, asserting the value, `adjusted`, `enum_index`, and on every failure
/// the reason or keyword and the span.
#[test]
fn constraints() {
    use hypercast::{Door, RawConstraints};

    let mut seen = std::collections::BTreeSet::new();
    for vector in corpus("constraints.json") {
        let door = vector["door"].as_str().expect("door");
        let c = &vector["constraints"];
        let label = format!("constraints {door} {c} {:?}", vector.get("input"));
        let door_of = match door {
            "i8" => Door::I8,
            "i16" => Door::I16,
            "i32" => Door::I32,
            "i64" => Door::I64,
            "u8" => Door::U8,
            "u16" => Door::U16,
            "u32" => Door::U32,
            "u64" => Door::U64,
            "f32" => Door::F32,
            "f64" => Door::F64,
            "decimal" => Door::Decimal,
            "text" => Door::Text,
            other => panic!("{label}: unknown door {other}"),
        };
        let packed = declaration(door_of, c);
        let entries = packed.as_ref().map(|(_, entries)| entries.clone()).unwrap_or_default();
        let verdict = packed.and_then(|(raw, entries)| {
            hypercast::check_constraints(door_of, &raw, &entries).map(|()| raw)
        });
        if expect(&vector) == "refused" {
            let code = vector["refusal"].as_u64().expect("refusal") as u32;
            assert_eq!(verdict.map_err(|r| r.code()), Err(code), "{label}");
            // `from_raw` refuses exactly what `check_constraints` refuses, duplicates included.
            // A declaration whose decimals don't convert never reaches the ABI form.
            if let Ok((raw, _)) = declaration(door_of, c) {
                assert_eq!(
                    from_raw(door_of, raw, &entries).map_err(|r| r.code()),
                    Err(code),
                    "{label}: from_raw"
                );
            }
            for field in ["input", "input_bytes", "from_f64"] {
                assert!(vector.get(field).is_none(), "{label}: a refused row has no {field}");
            }
            seen.insert("refused".to_owned());
            continue;
        }
        let raw: RawConstraints =
            verdict.unwrap_or_else(|refusal| panic!("{label}: refused ({refusal})"));
        seen.insert(door.to_owned());
        let checked = match door {
            "i8" => numeric_row::<i8>(&vector, c, raw, &entries),
            "i16" => numeric_row::<i16>(&vector, c, raw, &entries),
            "i32" => numeric_row::<i32>(&vector, c, raw, &entries),
            "i64" => numeric_row::<i64>(&vector, c, raw, &entries),
            "u8" => numeric_row::<u8>(&vector, c, raw, &entries),
            "u16" => numeric_row::<u16>(&vector, c, raw, &entries),
            "u32" => numeric_row::<u32>(&vector, c, raw, &entries),
            "u64" => numeric_row::<u64>(&vector, c, raw, &entries),
            "f32" => numeric_row::<f32>(&vector, c, raw, &entries),
            "f64" => numeric_row::<f64>(&vector, c, raw, &entries),
            "decimal" => numeric_row::<hypercast::Decimal>(&vector, c, raw, &entries),
            _ => text_row(&vector, c, raw, &entries),
        };
        assert_checked(&label, &vector, checked);
    }
    for door in ["i32", "i64", "u64", "f32", "f64", "decimal", "text", "refused"] {
        assert!(seen.contains(door), "constraints: no {door} vectors");
    }
}

/// What a checked door returned, with the value made comparable with the corpus, and the
/// detail its detailed twin gave a failure.
type CheckedRow =
    (Result<(CorpusValue, bool, Option<u32>), hypercast::Rejection<CorpusValue>>, Option<Detail>);

/// The detail of a detailed checked door's failure, once its verdict is shown to be the plain
/// door's with that detail.
fn checked_twin<T>(
    label: &str,
    plain: &Result<(CorpusValue, bool, Option<u32>), hypercast::Rejection<CorpusValue>>,
    twin: Result<hypercast::Checked<T>, detailed::Rejection<T>>,
    value: impl Fn(T) -> CorpusValue,
) -> Option<Detail> {
    let detail = match &twin {
        Err(detailed::Rejection::Failure(failure)) => Some(failure.detail),
        _ => None,
    };
    let projected = twin
        .map(|c| (value(c.value), c.adjusted, c.enum_index))
        .map_err(|rejection| hypercast::Rejection::from(rejection.map(&value)));
    assert_eq!(&projected, plain, "{label}: the detailed checked door agrees");
    detail
}

/// A delivered value: integers widened, floats by their bits (so the sign of a zero counts),
/// decimals and text by their text.
#[derive(Debug, PartialEq)]
enum CorpusValue {
    Integer(i128),
    F32(u32),
    F64(u64),
    Text(String),
}

trait CheckedDoor: hypercast::Numeric + Copy {
    fn corpus_value(self) -> CorpusValue;
    fn expected(value: &Value) -> CorpusValue;
}

macro_rules! integer_rows {
    ($($ty:ty),+) => {$(
        impl CheckedDoor for $ty {
            fn corpus_value(self) -> CorpusValue {
                CorpusValue::Integer(self.into())
            }
            fn expected(value: &Value) -> CorpusValue {
                CorpusValue::Integer(
                    value
                        .as_i64()
                        .map(i128::from)
                        .or_else(|| value.as_u64().map(i128::from))
                        .expect("an integer value"),
                )
            }
        }
    )+};
}

integer_rows!(i8, i16, i32, i64, u8, u16, u32, u64);

impl CheckedDoor for f32 {
    fn corpus_value(self) -> CorpusValue {
        CorpusValue::F32(self.to_bits())
    }
    fn expected(value: &Value) -> CorpusValue {
        CorpusValue::F32((value.as_f64().expect("a number value") as f32).to_bits())
    }
}

impl CheckedDoor for f64 {
    fn corpus_value(self) -> CorpusValue {
        CorpusValue::F64(self.to_bits())
    }
    fn expected(value: &Value) -> CorpusValue {
        CorpusValue::F64(value.as_f64().expect("a number value").to_bits())
    }
}

impl CheckedDoor for hypercast::Decimal {
    fn corpus_value(self) -> CorpusValue {
        CorpusValue::Text(self.to_string())
    }
    fn expected(value: &Value) -> CorpusValue {
        CorpusValue::Text(value.as_str().expect("a decimal string").to_owned())
    }
}

fn decimal_field(c: &Value, field: &str) -> Option<Result<hypercast::Decimal, hypercast::Refusal>> {
    c.get(field).map(|text| declared(text.as_str().expect("decimal").as_bytes()))
}

fn rounding_of(c: &Value) -> hypercast::Rounding {
    match c.get("rounding").map(|r| r.as_str().expect("rounding")) {
        None | Some("strict") => hypercast::Rounding::Strict,
        Some("midpoint") => hypercast::Rounding::Midpoint,
        Some("floor") => hypercast::Rounding::Floor,
        Some("ceiling") => hypercast::Rounding::Ceiling,
        Some(other) => panic!("unknown rounding {other}"),
    }
}

/// The `enum` entries, or `None` when the vector declares no `enum` (`[]` is an `enum` with
/// no entries, which is refused).
fn enum_texts(c: &Value) -> Option<Vec<&str>> {
    c.get("enum").map(|entries| {
        entries.as_array().expect("enum").iter().map(|e| e.as_str().expect("entry")).collect()
    })
}

/// A vector's declaration in its ABI form, every keyword it names whether or not the door
/// takes it: the layout, and the `enum` list packed for the door (text entries length-prefixed,
/// integer entries as 16-byte `Decimal`s). A decimal string the conversion refuses is the
/// answer.
fn declaration(
    door: hypercast::Door,
    c: &Value,
) -> Result<(hypercast::RawConstraints, Vec<u8>), hypercast::Refusal> {
    use hypercast::RawConstraints as Raw;
    let mut raw = Raw::ZERO;
    // Each of the four bounds is its own keyword, in its own slot.
    if let Some(value) = decimal_field(c, "minimum") {
        raw.flags |= Raw::HAS_MINIMUM;
        raw.minimum = value?;
    }
    if let Some(value) = decimal_field(c, "exclusiveMinimum") {
        raw.flags |= Raw::HAS_EXCLUSIVE_MINIMUM;
        raw.exclusive_minimum = value?;
    }
    if let Some(value) = decimal_field(c, "maximum") {
        raw.flags |= Raw::HAS_MAXIMUM;
        raw.maximum = value?;
    }
    if let Some(value) = decimal_field(c, "exclusiveMaximum") {
        raw.flags |= Raw::HAS_EXCLUSIVE_MAXIMUM;
        raw.exclusive_maximum = value?;
    }
    if let Some(value) = decimal_field(c, "multipleOf") {
        raw.flags |= Raw::HAS_MULTIPLE_OF;
        raw.multiple_of = value?;
    }
    raw.rounding = rounding_of(c) as u32;
    if let Some(length) = c.get("minLength") {
        raw.flags |= Raw::HAS_MIN_LENGTH;
        raw.min_length = length.as_u64().expect("minLength") as u32;
    }
    if let Some(length) = c.get("maxLength") {
        raw.flags |= Raw::HAS_MAX_LENGTH;
        raw.max_length = length.as_u64().expect("maxLength") as u32;
    }
    if c.get("trim").and_then(Value::as_bool) == Some(true) {
        raw.flags |= Raw::TRIM;
    }
    if c.get("caseSensitive").and_then(Value::as_bool) == Some(true) {
        raw.flags |= Raw::CASE_SENSITIVE;
    }
    let mut entries = Vec::new();
    let listed = enum_texts(c);
    if listed.is_some() {
        raw.flags |= Raw::HAS_ENUM;
    }
    for entry in listed.into_iter().flatten() {
        if door == hypercast::Door::Text {
            entries.extend_from_slice(&(entry.len() as u32).to_le_bytes());
            entries.extend_from_slice(entry.as_bytes());
        } else {
            let decimal = declared(entry.as_bytes())?;
            entries.extend_from_slice(&decimal.lo.to_ne_bytes());
            entries.extend_from_slice(&decimal.hi.to_ne_bytes());
            entries.extend_from_slice(&[decimal.scale, decimal.negative.into(), 0, 0]);
        }
    }
    Ok((raw, entries))
}

/// Replays a numeric vector through the typed builder and `cast_checked` or
/// `checked_from_f64`. The builder's layout must be the one `declaration` packed.
fn numeric_row<T: CheckedDoor>(
    vector: &Value,
    c: &Value,
    raw: hypercast::RawConstraints,
    packed: &[u8],
) -> CheckedRow {
    use hypercast::NumericRules;
    let decimal = |field: &str| decimal_field(c, field).map(|d| d.expect("converted"));
    let rules = NumericRules::new()
        .maybe_minimum(decimal("minimum"))
        .maybe_exclusive_minimum(decimal("exclusiveMinimum"))
        .maybe_maximum(decimal("maximum"))
        .maybe_exclusive_maximum(decimal("exclusiveMaximum"))
        .maybe_multiple_of(decimal("multipleOf").map(|step| (step, rounding_of(c))));
    let entries: Option<Vec<hypercast::Decimal>> = enum_texts(c).map(|entries| {
        entries.iter().map(|e| declared(e.as_bytes()).expect("a numeric enum entry")).collect()
    });
    let constraints =
        hypercast::Constraints::<T>::numeric(rules, entries.as_deref()).expect("declared");
    // The ABI carries a text keyword a numeric declaration ignores; the builder has none.
    let ignored = hypercast::RawConstraints {
        flags: raw.flags
            & !(hypercast::RawConstraints::HAS_MIN_LENGTH
                | hypercast::RawConstraints::HAS_MAX_LENGTH),
        min_length: 0,
        max_length: 0,
        ..raw
    };
    assert_eq!(constraints.raw(), ignored, "the builder's layout");
    let from_raw = hypercast::Constraints::<T>::from_raw(raw, packed).expect("from_raw declared");
    let from_f64 = vector.get("from_f64").map(|value| match value.as_str() {
        Some("-0") => -0.0,
        Some("NaN") => f64::NAN,
        Some("Infinity") => f64::INFINITY,
        Some("-Infinity") => f64::NEG_INFINITY,
        Some(other) => panic!("unknown from_f64 {other}"),
        None => value.as_f64().expect("from_f64"),
    });
    let format = format_of(vector);
    let judge = |constraints: &hypercast::Constraints<'_, T>| {
        let checked = match from_f64 {
            Some(value) => hypercast::checked_from_f64(value, constraints),
            None => hypercast::cast_checked(input(vector), &format, constraints),
        };
        checked
            .map(|c| (c.value.corpus_value(), c.adjusted, c.enum_index))
            .map_err(|rejection| rejection.map(T::corpus_value))
    };
    let checked = judge(&constraints);
    let twin = match from_f64 {
        Some(value) => detailed::checked_from_f64(value, &constraints),
        None => detailed::cast_checked(input(vector), &valid(&format), &constraints),
    };
    let detail = checked_twin("constraints", &checked, twin, T::corpus_value);
    assert_eq!(judge(&from_raw), checked, "from_raw judges as the builder does");
    let repacked = repack(&constraints, T::DOOR);
    let round_trip =
        hypercast::Constraints::<T>::from_raw(raw, &repacked).expect("round trip declared");
    assert_eq!(judge(&round_trip), checked, "the round trip judges as the builder does");
    (checked, detail)
}

/// `pack_enum` sized, then filled: the bytes the C ABI passes, which `check_constraints`
/// accepts at `door`.
fn repack<T>(constraints: &hypercast::Constraints<'_, T>, door: hypercast::Door) -> Vec<u8> {
    let len = constraints.pack_enum(&mut []).expect("packable");
    let mut packed = vec![0; len];
    assert_eq!(constraints.pack_enum(&mut packed), Some(len), "the length again, now written");
    assert_eq!(
        hypercast::check_constraints(door, &constraints.raw(), &packed),
        Ok(()),
        "packed bytes accepted"
    );
    packed
}

fn text_row(
    vector: &Value,
    c: &Value,
    raw: hypercast::RawConstraints,
    packed: &[u8],
) -> CheckedRow {
    let length = |field: &str| c.get(field).map(|n| n.as_u64().expect(field) as u32);
    let rules = hypercast::TextRules::new()
        .trim(c.get("trim").and_then(Value::as_bool) == Some(true))
        .case_sensitive(c.get("caseSensitive").and_then(Value::as_bool) == Some(true))
        .maybe_min_length(length("minLength"))
        .maybe_max_length(length("maxLength"));
    let entries = enum_texts(c);
    let constraints = hypercast::Constraints::text(rules, entries.as_deref()).expect("declared");
    // The ABI carries a numeric keyword a text declaration ignores; the builder has none.
    use hypercast::RawConstraints as Raw;
    let numeric = Raw::HAS_MINIMUM
        | Raw::HAS_EXCLUSIVE_MINIMUM
        | Raw::HAS_MAXIMUM
        | Raw::HAS_EXCLUSIVE_MAXIMUM
        | Raw::HAS_MULTIPLE_OF;
    let ignored = Raw {
        flags: raw.flags & !numeric,
        minimum: Raw::ZERO.minimum,
        exclusive_minimum: Raw::ZERO.minimum,
        maximum: Raw::ZERO.minimum,
        exclusive_maximum: Raw::ZERO.minimum,
        multiple_of: Raw::ZERO.minimum,
        ..raw
    };
    assert_eq!(constraints.raw(), ignored, "the builder's layout");
    let bytes: Vec<u8> = match vector.get("input_bytes") {
        Some(bytes) => bytes
            .as_array()
            .expect("input_bytes")
            .iter()
            .map(|b| b.as_u64().expect("a byte") as u8)
            .collect(),
        None => input(vector).as_bytes().to_vec(),
    };
    let from_raw = hypercast::Constraints::<hypercast::Text>::from_raw(raw, packed)
        .expect("from_raw declared");
    let judge = |constraints: &hypercast::Constraints<'_, hypercast::Text>| {
        hypercast::cast_text_checked(&bytes, constraints)
            .map(|c| (CorpusValue::Text(c.value.to_owned()), c.adjusted, c.enum_index))
            .map_err(|rejection| rejection.map(|value| CorpusValue::Text(value.to_owned())))
    };
    let checked = judge(&constraints);
    let twin = detailed::cast_text_checked(&bytes, &constraints);
    let detail = checked_twin("constraints text", &checked, twin, |value: &str| {
        CorpusValue::Text(value.to_owned())
    });
    assert_eq!(judge(&from_raw), checked, "from_raw judges as the builder does");
    let repacked = repack(&constraints, hypercast::Door::Text);
    let round_trip = hypercast::Constraints::<hypercast::Text>::from_raw(raw, &repacked)
        .expect("round trip declared");
    assert_eq!(judge(&round_trip), checked, "the round trip judges as the builder does");
    (checked, detail)
}

/// `Constraints::from_raw` on `door`'s type, as a downstream core calls it.
fn from_raw(
    door: hypercast::Door,
    raw: hypercast::RawConstraints,
    entries: &[u8],
) -> Result<(), hypercast::Refusal> {
    use hypercast::{Constraints as C, Decimal, Door, Text};
    match door {
        Door::I8 => C::<i8>::from_raw(raw, entries).map(drop),
        Door::I16 => C::<i16>::from_raw(raw, entries).map(drop),
        Door::I32 => C::<i32>::from_raw(raw, entries).map(drop),
        Door::I64 => C::<i64>::from_raw(raw, entries).map(drop),
        Door::U8 => C::<u8>::from_raw(raw, entries).map(drop),
        Door::U16 => C::<u16>::from_raw(raw, entries).map(drop),
        Door::U32 => C::<u32>::from_raw(raw, entries).map(drop),
        Door::U64 => C::<u64>::from_raw(raw, entries).map(drop),
        Door::F32 => C::<f32>::from_raw(raw, entries).map(drop),
        Door::F64 => C::<f64>::from_raw(raw, entries).map(drop),
        Door::Decimal => C::<Decimal>::from_raw(raw, entries).map(drop),
        Door::Text => C::<Text>::from_raw(raw, entries).map(drop),
        other => panic!("no constraints door {other:?}"),
    }
}

fn assert_checked(label: &str, vector: &Value, (checked, detail): CheckedRow) {
    use hypercast::{Keyword, Rejection};
    let span = |vector: &Value| {
        let span = vector["fault"].as_array().expect("every failure pins its span");
        (span[0].as_u64().expect("offset") as u32, span[1].as_u64().expect("len") as u32)
    };
    let named = vector.get("detail").map(|detail| detail_named(detail.as_str().expect("detail")));
    assert_eq!(detail, named, "{label}: detail");
    match (expect(vector), checked) {
        ("ok", Ok((value, adjusted, enum_index))) => {
            let expected = match &value {
                CorpusValue::Integer(_) => i64::expected(&vector["value"]),
                CorpusValue::F32(_) => f32::expected(&vector["value"]),
                CorpusValue::F64(_) => f64::expected(&vector["value"]),
                CorpusValue::Text(_) => {
                    CorpusValue::Text(vector["value"].as_str().expect("value").to_owned())
                }
            };
            assert_eq!(value, expected, "{label}: value");
            assert_eq!(
                adjusted,
                vector["adjusted"].as_bool().expect("adjusted"),
                "{label}: adjusted"
            );
            let index = vector.get("enum_index").map(|i| i.as_u64().expect("enum_index") as u32);
            assert_eq!(enum_index, index, "{label}: enum_index");
        }
        ("violation", Err(Rejection::Violation(violation))) => {
            let keyword = match vector["keyword"].as_str().expect("keyword") {
                "minimum" => Keyword::Minimum,
                "exclusiveMinimum" => Keyword::ExclusiveMinimum,
                "maximum" => Keyword::Maximum,
                "exclusiveMaximum" => Keyword::ExclusiveMaximum,
                "multipleOf" => Keyword::MultipleOf,
                "minLength" => Keyword::MinLength,
                "maxLength" => Keyword::MaxLength,
                "enum" => Keyword::Enum,
                other => panic!("{label}: unknown keyword {other}"),
            };
            assert_eq!(violation.keyword, keyword, "{label}: keyword");
            assert_eq!((violation.offset, violation.len), span(vector), "{label}: span");
            // The second level carries the value as cast, never the raw text nor a rounding.
            let expected = match &violation.value {
                CorpusValue::Integer(_) => i64::expected(&vector["value"]),
                CorpusValue::F32(_) => f32::expected(&vector["value"]),
                CorpusValue::F64(_) => f64::expected(&vector["value"]),
                CorpusValue::Text(_) => {
                    CorpusValue::Text(vector["value"].as_str().expect("value").to_owned())
                }
            };
            assert_eq!(violation.value, expected, "{label}: violation value");
        }
        (label_expect @ ("empty" | "malformed" | "out_of_range"), Err(Rejection::Fault(fault))) => {
            let reason = match label_expect {
                "empty" => Reason::Empty,
                "malformed" => Reason::Malformed,
                _ => Reason::OutOfRange,
            };
            assert_eq!(fault.reason, reason, "{label}: reason");
            assert_eq!((fault.offset, fault.len), span(vector), "{label}: span");
        }
        (expected, got) => panic!("{label}: expected {expected}, got {got:?}"),
    }
}
