//! Replays the shared conformance corpus (`corpus/*.json` at the repository root) through
//! the public Rust API. The corpus is the byte-for-byte cross-language contract: every
//! future binding replays these same files against the same native library, so a vector
//! that drifts here is a break in the polyglot promise, not just a failing Rust test.
//!
//! Vector schema: `{ "input", "expect": "ok"|"empty"|"malformed"|"out_of_range", ... }`
//! plus a per-domain value shape on "ok" vectors, an optional `"fault": [offset, len]`
//! span assertion on failures, and `"type"`/`"format"`/`"precision"` where a door takes one.

use hypercast::{
    CurrencySymbol, DateOrder, DayPeriods, ExcelEpoch, Fault, NativeDigits, NumFormat, Reason,
    Symbol, UnixPrecision,
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

/// The format a vector declares. Beyond the separators and flags every vector has, each
/// field is optional and absent means not declared: `currency`, the two currency
/// separators, `minus_sign`, `plus_sign`, `percent`, and `digit_zero` (one character).
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
#[test]
fn decimal_corpus() {
    for vector in corpus("decimal.json") {
        let text = input(&vector).as_bytes();
        let format = format_of(&vector);
        let verdict = hypercast::cast_decimal(text, &format);
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
        assert_verdict("boolean", &vector, verdict, |v| v["value"].as_bool().expect("value"));
    }
}

/// A char vector pins the scalar as its code-point integer, the shape every binding
/// compares whatever its own char type.
#[test]
fn char_corpus() {
    for vector in corpus("char.json") {
        let verdict = hypercast::cast_char(input(&vector).as_bytes()).map(u32::from);
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
        let verdict: Result<i128, Fault> = match vector["type"].as_str().expect("type") {
            "i8" => hypercast::cast_i8(text, &format).map(i128::from),
            "i16" => hypercast::cast_i16(text, &format).map(i128::from),
            "i32" => hypercast::cast_i32(text, &format).map(i128::from),
            "i64" => hypercast::cast_i64(text, &format).map(i128::from),
            "u8" => hypercast::cast_u8(text, &format).map(i128::from),
            "u16" => hypercast::cast_u16(text, &format).map(i128::from),
            "u32" => hypercast::cast_u32(text, &format).map(i128::from),
            "u64" => hypercast::cast_u64(text, &format).map(i128::from),
            other => panic!("integer: unknown type {other}"),
        };
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
        match vector["type"].as_str().expect("type") {
            "f32" => assert_verdict(
                "real",
                &vector,
                hypercast::cast_f32(text, &format).map(f32::to_bits),
                |v| (v["value"].as_f64().expect("value") as f32).to_bits(),
            ),
            "f64" => assert_verdict(
                "real",
                &vector,
                hypercast::cast_f64(text, &format).map(f64::to_bits),
                |v| v["value"].as_f64().expect("value").to_bits(),
            ),
            other => panic!("real: unknown type {other}"),
        }
    }
}

#[test]
fn uuid_corpus() {
    for vector in corpus("uuid.json") {
        let verdict = hypercast::cast_uuid(input(&vector).as_bytes());
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
        let verdict = hypercast::cast_timestamp(input(&vector).as_bytes());
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

#[test]
fn date_corpus() {
    for vector in corpus("date.json") {
        let verdict = hypercast::cast_date(input(&vector).as_bytes());
        assert_verdict("date", &vector, verdict, |v| hypercast::Date {
            year: v["year"].as_u64().expect("year") as u16,
            month: v["month"].as_u64().expect("month") as u8,
            day: v["day"].as_u64().expect("day") as u8,
        });
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
        let verdict = hypercast::cast_datetime_with(input(&vector).as_bytes(), order, &periods);
        assert_verdict("datetime", &vector, verdict, |v| hypercast::CivilDateTime {
            date: hypercast::Date {
                year: v["year"].as_u64().expect("year") as u16,
                month: v["month"].as_u64().expect("month") as u8,
                day: v["day"].as_u64().expect("day") as u8,
            },
            nanos_of_day: v["nanos_of_day"].as_u64().expect("nanos_of_day"),
        });
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
        let verdict = hypercast::cast_date_ordered(input(&vector).as_bytes(), order);
        assert_verdict("date_order", &vector, verdict, |v| hypercast::Date {
            year: v["year"].as_u64().expect("year") as u16,
            month: v["month"].as_u64().expect("month") as u8,
            day: v["day"].as_u64().expect("day") as u8,
        });
    }
}

#[test]
fn time_corpus() {
    for vector in corpus("time.json") {
        let verdict = hypercast::cast_time(input(&vector).as_bytes());
        assert_verdict("time", &vector, verdict, |v| v["nanos"].as_u64().expect("nanos"));
    }
}

#[test]
fn duration_corpus() {
    for vector in corpus("duration.json") {
        let verdict = hypercast::cast_duration(input(&vector).as_bytes());
        assert_verdict("duration", &vector, verdict, |v| hypercast::Duration {
            seconds: v["seconds"].as_i64().expect("seconds"),
            nanos: v["nanos"].as_i64().expect("nanos") as i32,
        });
    }
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
                assert_verdict("typed excel_time", &vector, verdict, |v| {
                    v["nanos"].as_u64().expect("nanos")
                });
            }
            "excel_duration" => {
                let verdict = hypercast::excel_duration(value).map_err(spanned);
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
        let verdict = packed.and_then(|(raw, entries)| {
            hypercast::check_constraints(door_of, &raw, &entries).map(|()| raw)
        });
        if expect(&vector) == "refused" {
            let code = vector["refusal"].as_u64().expect("refusal") as u32;
            assert_eq!(verdict.map_err(|r| r.code()), Err(code), "{label}");
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
            "i8" => numeric_row::<i8>(&vector, c, raw),
            "i16" => numeric_row::<i16>(&vector, c, raw),
            "i32" => numeric_row::<i32>(&vector, c, raw),
            "i64" => numeric_row::<i64>(&vector, c, raw),
            "u8" => numeric_row::<u8>(&vector, c, raw),
            "u16" => numeric_row::<u16>(&vector, c, raw),
            "u32" => numeric_row::<u32>(&vector, c, raw),
            "u64" => numeric_row::<u64>(&vector, c, raw),
            "f32" => numeric_row::<f32>(&vector, c, raw),
            "f64" => numeric_row::<f64>(&vector, c, raw),
            "decimal" => numeric_row::<hypercast::Decimal>(&vector, c, raw),
            _ => text_row(&vector, c, raw),
        };
        assert_checked(&label, &vector, checked);
    }
    for door in ["i32", "i64", "u64", "f32", "f64", "decimal", "text", "refused"] {
        assert!(seen.contains(door), "constraints: no {door} vectors");
    }
}

/// What a checked door returned, with the value made comparable with the corpus.
type CheckedRow = Result<(CorpusValue, bool, Option<u32>), hypercast::Rejection>;

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
    for (field, flag) in
        [("minimum", Raw::HAS_MINIMUM), ("exclusiveMinimum", Raw::HAS_EXCLUSIVE_MINIMUM)]
    {
        if let Some(value) = decimal_field(c, field) {
            raw.flags |= flag;
            raw.lower = value?;
        }
    }
    for (field, flag) in
        [("maximum", Raw::HAS_MAXIMUM), ("exclusiveMaximum", Raw::HAS_EXCLUSIVE_MAXIMUM)]
    {
        if let Some(value) = decimal_field(c, field) {
            raw.flags |= flag;
            raw.upper = value?;
        }
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
) -> CheckedRow {
    use hypercast::NumericRules;
    let decimal = |field: &str| decimal_field(c, field).map(|d| d.expect("converted"));
    let rules = NumericRules::new()
        .maybe_minimum(decimal("minimum"))
        .maybe_exclusive_minimum(decimal("exclusiveMinimum"))
        .maybe_maximum(decimal("maximum"))
        .maybe_exclusive_maximum(decimal("exclusiveMaximum"))
        .maybe_multiple_of(decimal("multipleOf").map(|step| (step, rounding_of(c))));
    let entries: Option<Vec<i128>> = enum_texts(c)
        .map(|entries| entries.iter().map(|e| e.parse().expect("an integer enum entry")).collect());
    let constraints =
        hypercast::Constraints::<T>::numeric(rules, entries.as_deref()).expect("declared");
    assert_eq!(constraints.raw(), raw, "the builder's layout");
    let checked = if let Some(value) = vector.get("from_f64") {
        let value = match value.as_str() {
            Some("-0") => -0.0,
            Some("NaN") => f64::NAN,
            Some("Infinity") => f64::INFINITY,
            Some("-Infinity") => f64::NEG_INFINITY,
            Some(other) => panic!("unknown from_f64 {other}"),
            None => value.as_f64().expect("from_f64"),
        };
        hypercast::checked_from_f64(value, &constraints)
    } else {
        let format = format_of(vector);
        hypercast::cast_checked(input(vector), &format, &constraints)
    };
    checked.map(|c| (c.value.corpus_value(), c.adjusted, c.enum_index))
}

fn text_row(vector: &Value, c: &Value, raw: hypercast::RawConstraints) -> CheckedRow {
    let length = |field: &str| c.get(field).map(|n| n.as_u64().expect(field) as u32);
    let rules = hypercast::TextRules::new()
        .trim(c.get("trim").and_then(Value::as_bool) == Some(true))
        .maybe_min_length(length("minLength"))
        .maybe_max_length(length("maxLength"));
    let entries = enum_texts(c);
    let constraints = hypercast::Constraints::text(rules, entries.as_deref()).expect("declared");
    assert_eq!(constraints.raw(), raw, "the builder's layout");
    let bytes: Vec<u8> = match vector.get("input_bytes") {
        Some(bytes) => bytes
            .as_array()
            .expect("input_bytes")
            .iter()
            .map(|b| b.as_u64().expect("a byte") as u8)
            .collect(),
        None => input(vector).as_bytes().to_vec(),
    };
    hypercast::cast_text_checked(&bytes, &constraints)
        .map(|c| (CorpusValue::Text(c.value.to_owned()), c.adjusted, c.enum_index))
}

fn assert_checked(label: &str, vector: &Value, checked: CheckedRow) {
    use hypercast::{Keyword, Rejection};
    let span = |vector: &Value| {
        let span = vector["fault"].as_array().expect("every failure pins its span");
        (span[0].as_u64().expect("offset") as u32, span[1].as_u64().expect("len") as u32)
    };
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
