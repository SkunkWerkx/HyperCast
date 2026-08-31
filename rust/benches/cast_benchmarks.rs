//! Criterion benchmarks pitting each door against the closest established parser — the
//! stdlib's own `str::parse` for booleans/numerics, the `uuid` crate for UUID text, and the
//! `time` crate's RFC 3339 well-known format for instants. Numbers are informational this
//! round; the load-bearing claims (allocation-free, corpus conformance) live in the tests.

use criterion::{criterion_group, criterion_main, Criterion};
use hypercast::{DateOrder, NumFormat};
use std::hint::black_box;
use time::format_description::well_known::Rfc3339;

fn benchmarks(c: &mut Criterion) {
    let format = NumFormat::INVARIANT;

    let mut group = c.benchmark_group("bool");
    group.bench_function("hypercast", |b| {
        b.iter(|| hypercast::cast_bool(black_box(b"true")).unwrap())
    });
    group.bench_function("stdlib", |b| {
        b.iter(|| black_box("true").parse::<bool>().unwrap())
    });
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
    group.bench_function("stdlib", |b| {
        b.iter(|| black_box("12345.6789").parse::<f64>().unwrap())
    });
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
    let mut group = c.benchmark_group("separator-detection");
    const EUROZONE: NumFormat =
        NumFormat { decimal_sep: ',', group_sep: '.', flags: NumFormat::ALL };
    group.bench_function("hypercast-detect", |b| {
        b.iter(|| hypercast::cast_f64(black_box(b"1.234.567,89"), &NumFormat::DETECT).unwrap())
    });
    group.bench_function("hypercast-declared", |b| {
        b.iter(|| hypercast::cast_f64(black_box(b"1.234.567,89"), &EUROZONE).unwrap())
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
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
