//! The doors `doors.rs` does not reach: the detailed doors, the date doors under fuzzed
//! options and a fuzzed month vocabulary, the checked doors under a fuzzed declaration read
//! through `Constraints::from_raw`, and the typed doors over a fuzzed double. The invariants
//! are `doors.rs`'s: no door and no declaration ever panics, and every span a fault, a
//! failure or a violation carries indexes the caller's own buffer.
//!
//! The leading four bytes select the family, the format profile, the order/precision/epoch
//! and the length of the declaration; that many bytes are the declaration (a packed month
//! vocabulary, or the fields of a `RawConstraints` followed by its packed `enum` list), and
//! the rest is the input. A declaration the core refuses is skipped: refusing is its verdict.

#![no_main]

use hypercast::detailed;
use hypercast::{
    Constraints, DateOptions, DateOrder, Decimal, ExcelEpoch, Failure, MonthNames, NumFormat,
    Numeric, RawConstraints, Rejection, Text, UnixPrecision, ValidDateOptions, ValidNumFormat,
    cast_checked, cast_text_checked, checked_from_f64, excel_duration, excel_serial, excel_time,
    optional_checked, shortest_digits, unix_from_f64,
};
use libfuzzer_sys::fuzz_target;

fn inside(input: &[u8], offset: u32, len: u32, what: &str) {
    let (offset, len) = (offset as usize, len as usize);
    assert!(
        offset <= input.len() && offset + len <= input.len(),
        "{what} span {offset}+{len} escapes a {}-byte input: {input:?}",
        input.len()
    );
}

fn failure<T>(input: &[u8], verdict: Result<T, Failure>) {
    if let Err(failure) = verdict {
        inside(input, failure.offset, failure.len, "failure");
    }
}

fn rejection<T>(input: &[u8], verdict: Result<T, Rejection<impl Sized>>) {
    match verdict {
        Err(Rejection::Fault(fault)) => inside(input, fault.offset, fault.len, "fault"),
        Err(Rejection::Violation(broken)) => inside(input, broken.offset, broken.len, "violation"),
        _ => {}
    }
}

fn detailed_rejection<T>(input: &[u8], verdict: Result<T, detailed::Rejection<impl Sized>>) {
    match verdict {
        Err(detailed::Rejection::Failure(failure)) => {
            inside(input, failure.offset, failure.len, "failure");
        }
        Err(detailed::Rejection::Violation(broken)) => {
            inside(input, broken.offset, broken.len, "violation");
        }
        _ => {}
    }
}

/// English month abbreviations, the vocabulary Excel's `07-Jan-26` needs.
const ENGLISH: [(u8, &str); 12] = [
    (1, "Jan"),
    (2, "Feb"),
    (3, "Mar"),
    (4, "Apr"),
    (5, "May"),
    (6, "Jun"),
    (7, "Jul"),
    (8, "Aug"),
    (9, "Sep"),
    (10, "Oct"),
    (11, "Nov"),
    (12, "Dec"),
];

/// A cursor over the declaration bytes; a field past their end reads as zero.
struct Fields<'a>(&'a [u8]);

impl<'a> Fields<'a> {
    fn byte(&mut self) -> u8 {
        match self.0 {
            [first, rest @ ..] => {
                self.0 = rest;
                *first
            }
            [] => 0,
        }
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }

    fn u64(&mut self) -> u64 {
        u64::from(self.u32()) | u64::from(self.u32()) << 32
    }

    fn decimal(&mut self) -> Decimal {
        let sign = self.byte();
        // Mostly the scales a decimal holds, sometimes one it cannot.
        let scale = if sign & 0x80 == 0 { self.byte() % 29 } else { self.byte() };
        Decimal { lo: self.u64(), hi: self.u32(), scale, negative: sign & 1 == 1 }
    }

    /// The fields of a declaration, in the layout's order; what is left is its `enum` list.
    fn constraints(&mut self) -> RawConstraints {
        // Two bytes cover every defined flag and one undefined bit; mostly a rounding the
        // layout defines.
        let flags = u32::from(self.byte()) | u32::from(self.byte() & 0x07) << 8;
        let rounding = u32::from(self.byte() % 6);
        RawConstraints {
            size: RawConstraints::SIZE,
            flags,
            rounding,
            min_length: u32::from(self.byte()),
            max_length: u32::from(self.byte()),
            _reserved: 0,
            minimum: self.decimal(),
            maximum: self.decimal(),
            multiple_of: self.decimal(),
            exclusive_minimum: self.decimal(),
            exclusive_maximum: self.decimal(),
        }
    }
}

/// One numeric door under the declaration, by text and by the double the input's bytes spell.
fn checked<T: Numeric>(
    input: &[u8],
    format: &NumFormat,
    valid: Option<&ValidNumFormat>,
    raw: RawConstraints,
    entries: &[u8],
) {
    let Ok(constraints) = Constraints::<T>::from_raw(raw, entries) else {
        return;
    };
    if let Some(declared) = constraints.enum_entries() {
        assert_eq!(declared.len(), declared.count());
    }
    let mut packed = [0u8; 512];
    if let Some(len) = constraints.pack_enum(&mut packed) {
        assert!(len <= packed.len());
    }
    rejection(input, cast_checked(input, format, &constraints));
    rejection(input, optional_checked(cast_checked(input, format, &constraints)));
    if let Some(valid) = valid {
        detailed_rejection(input, detailed::cast_checked(input, valid, &constraints));
    }
    let value = double(input);
    let _ = checked_from_f64(value, &constraints);
    let _ = detailed::checked_from_f64(value, &constraints);
}

/// The double the input's first eight bytes spell (zero-filled when there are fewer).
fn double(input: &[u8]) -> f64 {
    let mut bytes = [0u8; 8];
    for (to, from) in bytes.iter_mut().zip(input) {
        *to = *from;
    }
    f64::from_le_bytes(bytes)
}

fuzz_target!(|data: &[u8]| {
    let [family, format_sel, order_sel, declared_len, rest @ ..] = data else {
        return;
    };
    let (declaration, input) = rest.split_at(usize::from(*declared_len).min(rest.len()));

    let formats = [
        NumFormat::INVARIANT,
        NumFormat::DETECT,
        NumFormat::new(',', '.', NumFormat::ALL),
        NumFormat::new(',', '\u{a0}', NumFormat::ALL),
        NumFormat::new('.', ',', u32::from(*format_sel)),
        NumFormat::new('.', ',', NumFormat::ALL | NumFormat::GROUPING_INDIAN),
    ];
    let format = formats[usize::from(*format_sel) % formats.len()];
    let valid = format.validate().ok();
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

    match family % 5 {
        // The detailed doors that take no declaration.
        0 => {
            failure(input, detailed::cast_bool(input));
            failure(input, detailed::cast_char(input));
            failure(input, detailed::cast_uuid(input));
            failure(input, detailed::cast_unix(input, precision));
            failure(input, detailed::cast_excel_serial(input, epoch));
            failure(input, detailed::cast_time(input));
            failure(input, detailed::cast_duration(input));
            failure(input, detailed::cast_text(input));
            failure(input, detailed::optional(detailed::cast_text(input)));
            if let Some(valid) = &valid {
                failure(input, detailed::cast_i8(input, valid));
                failure(input, detailed::cast_i16(input, valid));
                failure(input, detailed::cast_i32(input, valid));
                failure(input, detailed::cast_i64(input, valid));
                failure(input, detailed::cast_u8(input, valid));
                failure(input, detailed::cast_u16(input, valid));
                failure(input, detailed::cast_u32(input, valid));
                failure(input, detailed::cast_u64(input, valid));
                failure(input, detailed::cast_f32(input, valid));
                failure(input, detailed::cast_f64(input, valid));
                failure(input, detailed::cast_decimal(input, valid));
            }
        }
        // The date doors under fuzzed options and a fuzzed month vocabulary.
        1 => {
            let mut fields = Fields(declaration);
            let flags = u32::from(fields.byte() & 0x0f);
            let pivot = match fields.byte() % 4 {
                0 => 0,
                1 => 2029,
                _ => u16::from(fields.byte()) << 8 | u16::from(fields.byte()),
            };
            // A vocabulary the fuzzer is unlikely to compose (every month named), the bytes as
            // a packed one, or none.
            let months = match fields.byte() % 3 {
                0 => None,
                1 => Some(MonthNames::from_entries(&ENGLISH)),
                _ => MonthNames::from_packed(fields.0).ok(),
            };
            let options: Option<ValidDateOptions<'_>> =
                DateOptions::from_flags(flags).with_pivot(pivot).validate(months).ok();
            let options = options.as_ref();
            failure(input, detailed::cast_date(input, options));
            failure(input, detailed::cast_date_ordered(input, order, options));
            failure(input, detailed::cast_datetime(input, order, options));
            failure(input, detailed::cast_timestamp(input, options));
        }
        // The numeric checked doors under a declaration read from its C ABI form.
        2 => {
            let mut fields = Fields(declaration);
            let raw = fields.constraints();
            let entries = fields.0;
            let valid = valid.as_ref();
            checked::<i8>(input, &format, valid, raw, entries);
            checked::<i16>(input, &format, valid, raw, entries);
            checked::<i32>(input, &format, valid, raw, entries);
            checked::<i64>(input, &format, valid, raw, entries);
            checked::<u8>(input, &format, valid, raw, entries);
            checked::<u16>(input, &format, valid, raw, entries);
            checked::<u32>(input, &format, valid, raw, entries);
            checked::<u64>(input, &format, valid, raw, entries);
            checked::<f32>(input, &format, valid, raw, entries);
            checked::<f64>(input, &format, valid, raw, entries);
            checked::<Decimal>(input, &format, valid, raw, entries);
        }
        // The text door under a declaration read from its C ABI form.
        3 => {
            let mut fields = Fields(declaration);
            let raw = fields.constraints();
            let Ok(constraints) = Constraints::<Text>::from_raw(raw, fields.0) else {
                return;
            };
            if let Some(declared) = constraints.enum_entries() {
                assert_eq!(declared.len(), declared.count());
            }
            let mut packed = [0u8; 512];
            if let Some(len) = constraints.pack_enum(&mut packed) {
                assert!(len <= packed.len());
            }
            rejection(input, cast_text_checked(input, &constraints));
            detailed_rejection(input, detailed::cast_text_checked(input, &constraints));
        }
        // The typed doors: a number already held, with no text and so no span to hold.
        _ => {
            let value = double(input);
            let _ = shortest_digits(value);
            let _ = excel_serial(value, epoch);
            let _ = excel_time(value);
            let _ = excel_duration(value);
            let _ = unix_from_f64(value, precision);
            let _ = detailed::excel_serial(value, epoch);
            let _ = detailed::excel_time(value);
            let _ = detailed::excel_duration(value);
            let _ = detailed::unix_from_f64(value, precision);
            let _ = detailed::decimal_from_f64(value);
            let _ = detailed::f32_from_f64(value);
            let _ = detailed::bool_from_f64(value);
            let _ = detailed::i8_from_f64(value);
            let _ = detailed::i64_from_f64(value);
            let _ = detailed::u8_from_f64(value);
            let _ = detailed::u64_from_f64(value);
        }
    }
});
