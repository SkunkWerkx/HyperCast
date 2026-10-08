//! Replays the shared conformance corpus (`corpus/*.json` at the repository root) through
//! the public Rust API. The corpus is the byte-for-byte cross-language contract: every
//! future binding replays these same files against the same native library, so a vector
//! that drifts here is a break in the polyglot promise, not just a failing Rust test.
//!
//! Vector schema: `{ "input", "expect": "ok"|"empty"|"malformed"|"out_of_range", ... }`
//! plus a per-domain value shape on "ok" vectors, an optional `"fault": [offset, len]`
//! span assertion on failures, and `"type"`/`"format"`/`"precision"` where a door takes one.

use hypercast::{CurrencySymbol, DateOrder, ExcelEpoch, Fault, NumFormat, Reason, UnixPrecision};
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
        format[field].as_str().and_then(|text| text.chars().next()).expect("single-char separator")
    };
    let currency = match format.get("currency").and_then(Value::as_str) {
        Some(symbol) => CurrencySymbol::new(symbol).expect("declarable currency symbol"),
        None => CurrencySymbol::NONE,
    };
    NumFormat::new(
        sep("decimal_sep"),
        sep("group_sep"),
        format["flags"].as_u64().expect("flags") as u32,
    )
    .with_currency(currency)
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
        let verdict = hypercast::cast_datetime(input(&vector).as_bytes(), order);
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
