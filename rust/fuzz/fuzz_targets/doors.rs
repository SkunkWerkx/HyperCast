//! One dispatcher over every door, asserting the two invariants every binding silently
//! relies on: a door NEVER panics on input (a panic in the core takes down the host
//! process in eight languages — bad data must always come back as a verdict), and every
//! fault span indexes the caller's own buffer (`offset + len <= input.len()`, which the
//! bindings use for slicing the offending text back out).
//!
//! The leading three bytes select the door, the format profile, and the declared
//! order/precision; the rest is the input. Formats cycle through the invariant profile,
//! eurozone declarations, NBSP grouping, structural detection, fully fuzzed flag bits,
//! the documented-defined degenerate equal-separators case, and declared currency symbols.

#![no_main]

use hypercast::{
    cast_bool, cast_date, cast_date_ordered, cast_datetime, cast_decimal, cast_duration,
    cast_excel_serial, cast_f32, cast_f64, cast_i16, cast_i32, cast_i64, cast_i8, cast_time,
    cast_timestamp, cast_u16, cast_u32, cast_u64, cast_u8, cast_unix, cast_uuid, CurrencySymbol,
    DateOrder, ExcelEpoch, Fault, NumFormat, UnixPrecision,
};
use libfuzzer_sys::fuzz_target;

fn check<T>(input: &[u8], verdict: Result<T, Fault>) {
    if let Err(fault) = verdict {
        let offset = fault.offset as usize;
        let len = fault.len as usize;
        assert!(
            offset <= input.len() && offset + len <= input.len(),
            "fault span {offset}+{len} escapes a {}-byte input: {input:?}",
            input.len()
        );
    }
}

fuzz_target!(|data: &[u8]| {
    let [door, format_sel, order_sel, input @ ..] = data else {
        return;
    };

    let formats = [
        NumFormat::INVARIANT,
        NumFormat::DETECT,
        NumFormat::new(',', '.', NumFormat::ALL),
        NumFormat::new(',', '\u{a0}', NumFormat::ALL),
        NumFormat::new('.', ',', u32::from(*format_sel)),
        // Equal separators: documented as defined (decimal wins) at the Rust level — the
        // FFI layer rejects them, but the core must still never panic.
        NumFormat::new('.', '.', NumFormat::ALL),
        // Declared currency symbols, at both edges and overlapping the separators.
        NumFormat::INVARIANT.with_currency(CurrencySymbol::new("$").unwrap()),
        NumFormat::new(',', '.', NumFormat::ALL).with_currency(CurrencySymbol::new("kr.").unwrap()),
        NumFormat::DETECT.with_currency(CurrencySymbol::new("\u{20ac}").unwrap()),
    ];
    let format = formats[usize::from(*format_sel) % formats.len()];
    let order = match order_sel % 3 {
        0 => DateOrder::YearMonthDay,
        1 => DateOrder::MonthDayYear,
        _ => DateOrder::DayMonthYear,
    };
    let epoch = if order_sel % 2 == 0 { ExcelEpoch::Y1900 } else { ExcelEpoch::Y1904 };
    let precision = match order_sel % 4 {
        0 => UnixPrecision::Seconds,
        1 => UnixPrecision::Millis,
        2 => UnixPrecision::Micros,
        _ => UnixPrecision::Nanos,
    };

    match door % 21 {
        0 => check(input, cast_bool(input)),
        1 => check(input, cast_i8(input, &format)),
        2 => check(input, cast_i16(input, &format)),
        3 => check(input, cast_i32(input, &format)),
        4 => check(input, cast_i64(input, &format)),
        5 => check(input, cast_u8(input, &format)),
        6 => check(input, cast_u16(input, &format)),
        7 => check(input, cast_u32(input, &format)),
        8 => check(input, cast_u64(input, &format)),
        9 => check(input, cast_f32(input, &format)),
        10 => check(input, cast_f64(input, &format)),
        11 => check(input, cast_uuid(input)),
        12 => check(input, cast_timestamp(input)),
        13 => check(input, cast_unix(input, precision)),
        14 => check(input, cast_date(input)),
        15 => check(input, cast_date_ordered(input, order)),
        16 => check(input, cast_datetime(input, order)),
        17 => check(input, cast_time(input)),
        18 => check(input, cast_excel_serial(input, epoch)),
        19 => check(input, cast_decimal(input, &format)),
        _ => check(input, cast_duration(input)),
    }
});
