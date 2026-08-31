//! Replays the shared conformance corpus (`corpus/*.json` at the repository root) through
//! the public Rust API. The corpus is the byte-for-byte cross-language contract: every
//! future binding replays these same files against the same native library, so a vector
//! that drifts here is a break in the polyglot promise, not just a failing Rust test.
//!
//! Vector schema: `{ "input", "expect": "ok"|"empty"|"malformed"|"out_of_range", ... }`
//! plus a per-domain value shape on "ok" vectors, an optional `"fault": [offset, len]`
//! span assertion on failures, and `"type"`/`"format"`/`"precision"` where a door takes one.

use hypercast::{DateOrder, Fault, NumFormat, Reason, UnixPrecision};
use serde_json::Value;
use std::path::PathBuf;

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

fn format_of(vector: &Value) -> NumFormat {
    let Some(format) = vector.get("format") else {
        return NumFormat::INVARIANT;
    };
    let sep = |field: &str| {
        format[field]
            .as_str()
            .and_then(|text| text.chars().next())
            .expect("single-char separator")
    };
    NumFormat {
        decimal_sep: sep("decimal_sep"),
        group_sep: sep("group_sep"),
        flags: format["flags"].as_u64().expect("flags") as u32,
    }
}

#[test]
fn boolean_corpus() {
    for vector in corpus("boolean.json") {
        let verdict = hypercast::cast_bool(input(&vector).as_bytes());
        assert_verdict("boolean", &vector, verdict, |v| v["value"].as_bool().expect("value"));
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
        // Compared as exact f64 bits: the corpus values are chosen to be exactly
        // representable outcomes, so equality is the contract, not approximation.
        let verdict: Result<f64, Fault> = match vector["type"].as_str().expect("type") {
            "f32" => hypercast::cast_f32(text, &format).map(f64::from),
            "f64" => hypercast::cast_f64(text, &format),
            other => panic!("real: unknown type {other}"),
        };
        match vector["type"].as_str().expect("type") {
            "f32" => assert_verdict("real", &vector, verdict, |v| {
                v["value"].as_f64().expect("value") as f32 as f64
            }),
            _ => assert_verdict("real", &vector, verdict, |v| v["value"].as_f64().expect("value")),
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
