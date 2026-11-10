//! Criterion benchmarks pitting each door against the closest established parser — the
//! stdlib's own `str::parse` for booleans/numerics, the `uuid` crate for UUID text, and the
//! `time` crate's RFC 3339 well-known format for instants. The numbers are informational;
//! the load-bearing claims (allocation-free, corpus conformance) live in the tests.

use criterion::{Criterion, criterion_group, criterion_main};
use hypercast::{
    Constraints, DateOrder, Decimal, Grouping, NumFormat, NumericRules, Rounding, Text, TextRules,
    decimal_for_declaration,
};
use std::hint::black_box;
use time::format_description::well_known::Rfc3339;

fn benchmarks(c: &mut Criterion) {
    let format = NumFormat::INVARIANT;

    let mut group = c.benchmark_group("bool");
    group.bench_function("hypercast", |b| {
        b.iter(|| hypercast::cast_bool(black_box(b"true")).unwrap())
    });
    group.bench_function("stdlib", |b| b.iter(|| black_box("true").parse::<bool>().unwrap()));
    group.finish();

    let mut group = c.benchmark_group("i64");
    group.bench_function("hypercast", |b| {
        b.iter(|| hypercast::cast_i64(black_box(b"1234567890123"), &format).unwrap())
    });
    group.bench_function("stdlib", |b| {
        b.iter(|| black_box("1234567890123").parse::<i64>().unwrap())
    });
    group.finish();

    let mut group = c.benchmark_group("f64");
    group.bench_function("hypercast", |b| {
        b.iter(|| hypercast::cast_f64(black_box(b"12345.6789"), &format).unwrap())
    });
    group.bench_function("stdlib", |b| b.iter(|| black_box("12345.6789").parse::<f64>().unwrap()));
    group.finish();

    // The lenient shapes, one notation added at a time, so a regression in any one of them
    // has a row of its own: grouping, a declared currency symbol, both, and both inside
    // accounting parentheses. Same value throughout, on the three doors that take them.
    let usd = NumFormat::INVARIANT.with_currency(hypercast::CurrencySymbol::new("$").unwrap());
    let shapes: [(&str, &[u8]); 5] = [
        ("plain", b"12345.67"),
        ("grouped", b"12,345.67"),
        ("currency", b"$12345.67"),
        ("grouped currency", b"$12,345.67"),
        ("grouped currency in parens", b"($12,345.67)"),
    ];
    let mut group = c.benchmark_group("f64-shapes");
    for (name, text) in shapes {
        group.bench_function(name, |b| {
            b.iter(|| hypercast::cast_f64(black_box(text), &usd).unwrap())
        });
    }
    group.finish();

    let mut group = c.benchmark_group("decimal-shapes");
    for (name, text) in shapes {
        group.bench_function(name, |b| {
            b.iter(|| hypercast::cast_decimal(black_box(text), &usd).unwrap())
        });
    }
    group.finish();

    let mut group = c.benchmark_group("i64-shapes");
    for (name, text) in [
        ("plain", &b"1234567"[..]),
        ("grouped", b"1,234,567"),
        ("grouped currency", b"$1,234,567"),
        ("grouped currency in parens", b"($1,234,567)"),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| hypercast::cast_i64(black_box(text), &usd).unwrap())
        });
    }
    group.finish();

    // Both policies on grouped numerals; the groups above are judged as Western, the default.
    let format = NumFormat::INVARIANT;
    let mut group = c.benchmark_group("grouping-policy");
    group.bench_function("i64 western", |b| {
        b.iter(|| hypercast::cast_i64(black_box(b"1,234,567"), &format).unwrap())
    });
    group.bench_function("f64 western", |b| {
        b.iter(|| hypercast::cast_f64(black_box(b"12,345.67"), &format).unwrap())
    });
    group.bench_function("decimal western", |b| {
        b.iter(|| hypercast::cast_decimal(black_box(b"12,345.67"), &format).unwrap())
    });
    let indian = NumFormat::INVARIANT.with_grouping(Grouping::Indian);
    group.bench_function("i64 indian", |b| {
        b.iter(|| hypercast::cast_i64(black_box(b"12,34,567"), &indian).unwrap())
    });
    group.finish();

    // What declaring a currency symbol costs text that does not carry one: the same grouped
    // integer under the invariant format and under one that declares `$`.
    let mut group = c.benchmark_group("i32-declared-currency");
    group.bench_function("invariant", |b| {
        b.iter(|| hypercast::cast_i32(black_box(b"1,234,567"), &format).unwrap())
    });
    group.bench_function("declared $", |b| {
        b.iter(|| hypercast::cast_i32(black_box(b"1,234,567"), &usd).unwrap())
    });
    group.finish();

    // The rejection path: a fault falls out of the fast lanes into the full engine, which
    // owns every fault span. Against the stdlib's own failing parse of the same mistake.
    let mut group = c.benchmark_group("i32-malformed");
    group.bench_function("hypercast", |b| {
        b.iter(|| hypercast::cast_i32(black_box(b"1,234x567"), &format).unwrap_err())
    });
    group
        .bench_function("stdlib", |b| b.iter(|| black_box("1234x567").parse::<i32>().unwrap_err()));
    group.finish();

    // The cells a column of bad data is made of, as HyperTabular's `faulty` scope feeds them: a
    // numeral with a stray character after it or ahead of it, and no text at all.
    let mut group = c.benchmark_group("refused");
    group.bench_function("i32 123x", |b| {
        b.iter(|| hypercast::cast_i32(black_box(b"123x"), &format).unwrap_err())
    });
    group.bench_function("i32 y123", |b| {
        b.iter(|| hypercast::cast_i32(black_box(b"y123"), &format).unwrap_err())
    });
    group.bench_function("i32 empty", |b| {
        b.iter(|| hypercast::cast_i32(black_box(b""), &format).unwrap_err())
    });
    group.bench_function("f64 123z", |b| {
        b.iter(|| hypercast::cast_f64(black_box(b"123z"), &format).unwrap_err())
    });
    group.finish();

    let mut group = c.benchmark_group("decimal");
    group.bench_function("hypercast", |b| {
        b.iter(|| hypercast::cast_decimal(black_box(b"12345.6789"), &format).unwrap())
    });
    group.bench_function("hypercast grouped currency", |b| {
        b.iter(|| hypercast::cast_decimal(black_box(b"$12,345.67"), &usd).unwrap())
    });
    group.finish();

    // The text door's UTF-8 validation, against core's `str::from_utf8` on the same bytes:
    // under 64 bytes (core alone), at 64 (one SIMD chunk) and at about 4 KiB, ASCII and
    // multi-byte; then the 4 KiB inputs with one bad byte first, second, midway and last. A
    // failure costs a whole SIMD pass wherever the bad byte is, then core's check up to it.
    let ascii = "a".repeat(4096);
    let multi = "aé€𝄞".repeat(410);
    let euros = format!("{}a", "€".repeat(21));
    let bad = |text: &str, at: usize, byte: u8| {
        let mut bytes = text.as_bytes().to_vec();
        bytes[at] = byte;
        bytes
    };
    let bad_ascii = [0, 1, 2048, 4095].map(|at| bad(&ascii, at, 0x80));
    let bad_multi = [0, 1, 2050, 4099].map(|at| bad(&multi, at, 0xff));
    let cases: [(&str, &[u8]); 14] = [
        ("63 B ascii", &ascii.as_bytes()[..63]),
        ("64 B ascii", &ascii.as_bytes()[..64]),
        ("63 B multi-byte", &euros.as_bytes()[..63]),
        ("64 B multi-byte", euros.as_bytes()),
        ("4 KiB ascii", ascii.as_bytes()),
        ("4,100 B multi-byte", multi.as_bytes()),
        ("4 KiB ascii bad at 0", &bad_ascii[0]),
        ("4 KiB ascii bad at 1", &bad_ascii[1]),
        ("4 KiB ascii bad at 2048", &bad_ascii[2]),
        ("4 KiB ascii bad at 4095", &bad_ascii[3]),
        ("4,100 B multi-byte bad at 0", &bad_multi[0]),
        ("4,100 B multi-byte bad at 1", &bad_multi[1]),
        ("4,100 B multi-byte bad at 2050", &bad_multi[2]),
        ("4,100 B multi-byte bad at 4099", &bad_multi[3]),
    ];
    let mut group = c.benchmark_group("text");
    for (name, bytes) in cases {
        group.bench_function(name, |b| b.iter(|| hypercast::cast_text(black_box(bytes)).is_ok()));
        group.bench_function(format!("{name} stdlib"), |b| {
            b.iter(|| core::str::from_utf8(black_box(bytes)).is_ok())
        });
    }
    group.finish();

    let mut group = c.benchmark_group("uuid");
    group.bench_function("hypercast", |b| {
        b.iter(|| hypercast::cast_uuid(black_box(b"01020304-0506-0708-090a-0b0c0d0e0f10")).unwrap())
    });
    group.bench_function("uuid-crate", |b| {
        b.iter(|| uuid::Uuid::try_parse(black_box("01020304-0506-0708-090a-0b0c0d0e0f10")).unwrap())
    });
    group.finish();

    let mut group = c.benchmark_group("timestamp");
    group.bench_function("hypercast", |b| {
        b.iter(|| hypercast::cast_timestamp(black_box(b"2026-01-02T15:04:05.123456789Z")).unwrap())
    });
    group.bench_function("time-crate", |b| {
        b.iter(|| {
            time::OffsetDateTime::parse(black_box("2026-01-02T15:04:05.123456789Z"), &Rfc3339)
                .unwrap()
        })
    });
    group.finish();

    // The messy civil shape: no Rust stdlib parser accepts it at all, so the pairing is
    // against the `time` crate driven by a hand-built format description — the closest
    // equivalent work, same as the Java/C# tables pair against pattern formatters.
    let mut group = c.benchmark_group("datetime");
    group.bench_function("hypercast-messy", |b| {
        b.iter(|| {
            hypercast::cast_datetime(black_box(b"1/7/2026 3:04 PM"), DateOrder::MonthDayYear)
                .unwrap()
        })
    });
    group.bench_function("hypercast-iso", |b| {
        b.iter(|| {
            hypercast::cast_datetime(black_box(b"2026-01-07T15:04:05"), DateOrder::YearMonthDay)
                .unwrap()
        })
    });
    group.bench_function("hypercast-date-ordered", |b| {
        b.iter(|| {
            hypercast::cast_date_ordered(black_box(b"1/7/2026"), DateOrder::MonthDayYear).unwrap()
        })
    });
    group.finish();

    // Separator detection against the same text under a declared format: the cost of the
    // structural resolution pass, isolated.
    let eurozone = NumFormat::new(',', '.', NumFormat::ALL);
    let mut group = c.benchmark_group("separator-detection");
    group.bench_function("hypercast-detect", |b| {
        b.iter(|| hypercast::cast_f64(black_box(b"1.234.567,89"), &NumFormat::DETECT).unwrap())
    });
    group.bench_function("hypercast-declared", |b| {
        b.iter(|| hypercast::cast_f64(black_box(b"1.234.567,89"), &eurozone).unwrap())
    });
    group.finish();

    let mut group = c.benchmark_group("duration");
    group.bench_function("hypercast-iso", |b| {
        b.iter(|| hypercast::cast_duration(black_box(b"P1DT6H30M15.5S")).unwrap())
    });
    group.bench_function("hypercast-colon", |b| {
        b.iter(|| hypercast::cast_duration(black_box(b"1.06:30:15.5")).unwrap())
    });
    group.finish();
    checked_benchmarks(c);
}

/// What a verdict costs: each checked door beside the plain door it wraps, on the same text.
/// The `Constraints` are built once, outside the timed loop, as a caller holding a column's
/// rule would. The checked door does strictly more work, so it is slower by construction;
/// the figures are the price of the verdict.
fn checked_benchmarks(c: &mut Criterion) {
    let format = NumFormat::INVARIANT;
    let dec = |text| decimal_for_declaration(text).unwrap();

    // Latitude: (-90, 90], rounded to the nearest 0.000001.
    let latitude = Constraints::<f64>::numeric(
        NumericRules::new()
            .exclusive_minimum(dec("-90"))
            .maximum(dec("90"))
            .multiple_of(dec("0.000001"), Rounding::Midpoint),
        None,
    )
    .unwrap();
    let mut group = c.benchmark_group("checked-f64");
    group.bench_function("plain", |b| {
        b.iter(|| hypercast::cast_f64(black_box(b"45.1234567"), &format).unwrap())
    });
    group.bench_function("checked", |b| {
        b.iter(|| hypercast::cast_checked::<f64>(black_box(b"45.1234567"), &format, &latitude))
    });
    // The spreadsheet route: a value already decoded to a float, rounded by the same rule.
    group.bench_function("checked from_f64", |b| {
        b.iter(|| hypercast::checked_from_f64::<f64>(black_box(45.1234567), &latitude))
    });
    group.finish();

    // A price, strict: 12.37 against a step of 0.05 is a violation by design, so the checked
    // door's verdict is the rejection; the plain door's is the value.
    let price = Constraints::<Decimal>::numeric(
        NumericRules::new().multiple_of(dec("0.05"), Rounding::Strict),
        None,
    )
    .unwrap();
    let mut group = c.benchmark_group("checked-decimal");
    group.bench_function("plain", |b| {
        b.iter(|| hypercast::cast_decimal(black_box(b"12.37"), &format).unwrap())
    });
    group.bench_function("checked", |b| {
        b.iter(|| hypercast::cast_checked::<Decimal>(black_box(b"12.37"), &format, &price))
    });
    group.finish();

    // Text: a three-entry enum, trimmed. The plain door is `cast_text`.
    let entries = ["draft", "published", "archived"];
    let status = Constraints::<Text>::text(TextRules::new().trim(true), Some(&entries)).unwrap();
    let mut group = c.benchmark_group("checked-text");
    group.bench_function("plain", |b| {
        b.iter(|| hypercast::cast_text(black_box(b"  published ")).unwrap())
    });
    group.bench_function("checked", |b| {
        b.iter(|| hypercast::cast_text_checked(black_box(b"  published "), &status))
    });
    group.finish();

    // The same text bare and padded, so what an edge costs is measured against the text
    // alone. The U+00A0 group has no base to run against (the base read none of it), so
    // `cargo bench -- padded/` runs the ASCII pairs alone, on a build that rejects it.
    let mut group = c.benchmark_group("padded");
    group.bench_function("bool/plain", |b| {
        b.iter(|| hypercast::cast_bool(black_box(b"true")).unwrap())
    });
    group.bench_function("bool/ascii-padded", |b| {
        b.iter(|| hypercast::cast_bool(black_box(b" true\n")).unwrap())
    });
    group.bench_function("i64/plain", |b| {
        b.iter(|| hypercast::cast_i64(black_box(b"1234567890123"), &format).unwrap())
    });
    group.bench_function("i64/ascii-padded", |b| {
        b.iter(|| hypercast::cast_i64(black_box(b"  1234567890123\n"), &format).unwrap())
    });
    group.bench_function("f64/plain", |b| {
        b.iter(|| hypercast::cast_f64(black_box(b"12345.6789"), &format).unwrap())
    });
    group.bench_function("f64/ascii-padded", |b| {
        b.iter(|| hypercast::cast_f64(black_box(b"  12345.6789\n"), &format).unwrap())
    });
    group.bench_function("uuid/plain", |b| {
        b.iter(|| hypercast::cast_uuid(black_box(b"550e8400-e29b-41d4-a716-446655440000")).unwrap())
    });
    group.bench_function("uuid/ascii-padded", |b| {
        b.iter(|| {
            hypercast::cast_uuid(black_box(b" 550e8400-e29b-41d4-a716-446655440000\n")).unwrap()
        })
    });
    group.finish();

    let mut group = c.benchmark_group("padded-nbsp");
    group.bench_function("bool", |b| {
        b.iter(|| hypercast::cast_bool(black_box("\u{a0}true\u{a0}".as_bytes())).unwrap())
    });
    group.bench_function("i64", |b| {
        b.iter(|| {
            hypercast::cast_i64(black_box("\u{a0}1234567890123\u{a0}".as_bytes()), &format).unwrap()
        })
    });
    group.bench_function("f64", |b| {
        b.iter(|| {
            hypercast::cast_f64(black_box("\u{a0}12345.6789\u{a0}".as_bytes()), &format).unwrap()
        })
    });
    group.finish();
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
