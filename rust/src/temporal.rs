//! The temporal doors — every one lands in protobuf's dual-integer forms
//! ([`Timestamp`]/[`Duration`] `{seconds, nanos}`, [`Date`] `{y, m, d}`, time-of-day as
//! nanos-since-midnight) so bindings fold them into their platform types at whatever
//! fidelity that platform actually has.
//!
//! Semantics port Svartalfheim's temporal family (`DateTimeOffsetParser`, `DateOnlyParser`,
//! `TimeOnlyParser`, `TimeSpanParser`) with the deliberate divergences documented per door:
//! fractional seconds widen from .NET's 7-digit ticks to 9-digit nanos, the protobuf JSON
//! duration form (`3.5s`) joins the accepted shapes, and the Min/Max sentinel guards move
//! to the bindings — the protobuf window's boundary instants are legal values here.
//!
//! No tzdb lives in the core: IANA zone resolution and DST fusion are host concerns,
//! exactly as HyperUuid left the wall clock to the host.

use crate::integer::char_len_at;
use crate::verdict::{CivilDateTime, Date, Duration, Fault, Reason, Timestamp, trim};
use core::num::NonZero;
use core::ops::RangeInclusive;

/// `0001-01-01T00:00:00Z` — the floor of the protobuf timestamp window.
pub const MIN_TIMESTAMP_SECONDS: i64 = -62_135_596_800;
/// `9999-12-31T23:59:59Z` — the ceiling of the protobuf timestamp window
/// (nanos may still carry up to .999999999 at this second).
pub const MAX_TIMESTAMP_SECONDS: i64 = 253_402_300_799;
/// ±10,000 years — the protobuf duration window's magnitude bound on whole seconds.
pub const MAX_DURATION_SECONDS: i64 = 315_576_000_000;

const NANOS_PER_SECOND: i128 = 1_000_000_000;

/// The declared unit of a Unix-epoch value. There is no magnitude guessing — the caller
/// states the unit, so a bare number is never silently interpreted as seconds or
/// milliseconds. Svartalfheim's `UnixPrecision`, extended below milliseconds now that the
/// output type carries nanos.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnixPrecision {
    /// Seconds since `1970-01-01T00:00:00Z`.
    Seconds = 1,
    /// Milliseconds since the epoch.
    Millis = 2,
    /// Microseconds since the epoch.
    Micros = 3,
    /// Nanoseconds since the epoch.
    Nanos = 4,
}

/// The date system an Excel serial number is expressed in. Spreadsheets carry no marker for
/// this — it is a workbook-level setting — so the caller states it, the same way
/// [`UnixPrecision`] and [`DateOrder`] are declared rather than guessed.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExcelEpoch {
    /// The 1900 system (the Windows default): serial `1` is `1900-01-01`, and serial `60`
    /// is a February 29th that never existed. See [`cast_excel_serial`].
    Y1900 = 1,
    /// The 1904 system (legacy Macintosh workbooks, still selectable today): serial `0` is
    /// `1904-01-01`, with no phantom day anywhere in it.
    Y1904 = 2,
}

/// Serial `0` of the 1900 system as days from the Unix epoch: `1899-12-30`, which is *two*
/// days before `1900-01-01` rather than one, precisely because the phantom `1900-02-29`
/// occupies a slot later in the same year.
const EXCEL_1900_ANCHOR_DAYS: i64 = days_from_civil(1899, 12, 30);
/// Serial `0` of the 1904 system as days from the Unix epoch: `1904-01-01`.
const EXCEL_1904_ANCHOR_DAYS: i64 = days_from_civil(1904, 1, 1);
/// The slot the 1900 system gives the nonexistent `1900-02-29`.
const EXCEL_1900_PHANTOM_SERIAL: i64 = 60;
/// Highest whole serial still inside the [`Timestamp`] window (`9999-12-31`), per system.
const MAX_EXCEL_1900_SERIAL: i64 = days_from_civil(9999, 12, 31) - EXCEL_1900_ANCHOR_DAYS;
const MAX_EXCEL_1904_SERIAL: i64 = days_from_civil(9999, 12, 31) - EXCEL_1904_ANCHOR_DAYS;
/// Fraction digits beyond this cannot move the nanosecond result — a day is 86,400 × 10⁹
/// nanoseconds, so 14 digits already resolves below 1 ns. Extra digits are consumed and
/// validated, just no longer accumulated, which keeps the i128 scaling far from overflow.
const MAX_EXCEL_FRACTION_DIGITS: usize = 14;

fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
    }
}

/// Days since 1970-01-01 for a proleptic-Gregorian civil date — Howard Hinnant's
/// `days_from_civil`, pure integer math.
const fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    // `as` rather than `i64::from`: identical value, but `From` is not const-stable and this
    // fn is const so the Excel anchors below can be computed rather than hardcoded.
    let adjusted_year = year - (month <= 2) as i64;
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year.rem_euclid(400);
    let month_shift: i64 = if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * (month as i64 + month_shift) + 2) / 5 + day as i64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The inverse of [`days_from_civil`]: `(year, month, day)` for days since 1970-01-01.
const fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
    let month = if month_index < 10 { month_index + 3 } else { month_index - 9 } as u32;
    (year_of_era + era * 400 + (month <= 2) as i64, month, day)
}

const NANOS_PER_DAY: u64 = 86_400_000_000_000;

impl CivilDateTime {
    /// This wall clock read as UTC — the instant it names if, and only if, the caller says
    /// its zone is UTC. The crate never makes that assumption for a caller (see the type's
    /// own doc); this is the caller making it, by name, where a zone-less source has to
    /// become an instant, as [`cast_excel_serial`] does for a spreadsheet's serial text.
    pub const fn assume_utc(self) -> Timestamp {
        let days =
            days_from_civil(self.date.year as i64, self.date.month as u32, self.date.day as u32);
        Timestamp {
            seconds: days * 86_400 + (self.nanos_of_day / 1_000_000_000) as i64,
            nanos: (self.nanos_of_day % 1_000_000_000) as i32,
        }
    }
}

impl Timestamp {
    /// The UTC wall clock of this instant — the inverse of [`CivilDateTime::assume_utc`].
    /// `None` outside the timestamp window (`0001-01-01` through `9999-12-31`, nanos in
    /// `0..=999_999_999`), which no door produces but the public fields can spell.
    pub const fn utc_civil(self) -> Option<CivilDateTime> {
        if self.seconds < MIN_TIMESTAMP_SECONDS
            || self.seconds > MAX_TIMESTAMP_SECONDS
            || self.nanos < 0
            || self.nanos > 999_999_999
        {
            return None;
        }
        let (year, month, day) = civil_from_days(self.seconds.div_euclid(86_400));
        let second_of_day = self.seconds.rem_euclid(86_400) as u64;
        Some(CivilDateTime {
            date: Date { year: year as u16, month: month as u8, day: day as u8 },
            nanos_of_day: second_of_day * 1_000_000_000 + self.nanos as u64,
        })
    }
}

/// Reads exactly two ASCII digits at `at` — one bounds check, wrapping-sub digit test.
fn read2(text: &[u8], at: usize) -> Option<u32> {
    let pair: &[u8; 2] = text.get(at..at + 2)?.try_into().ok()?;
    let hi = pair[0].wrapping_sub(b'0');
    let lo = pair[1].wrapping_sub(b'0');
    if hi > 9 || lo > 9 {
        return None;
    }
    Some(u32::from(hi) * 10 + u32::from(lo))
}

/// Reads exactly four ASCII digits at `at`.
fn read4(text: &[u8], at: usize) -> Option<u32> {
    Some(read2(text, at)? * 100 + read2(text, at + 2)?)
}

/// Reads `.f{1..=9}` at `at` when present, returning the value widened to nanoseconds and
/// the index after the fraction. A tenth fractional digit is `Malformed` — nanos is the
/// core's full fidelity. The RFC 3339/ISO-time doors: dot only.
fn read_fraction(text: &[u8], at: usize, start: usize) -> Result<(u32, usize), Fault> {
    read_fraction_marked(text, at, start, b".")
}

/// [`read_fraction`] with the accepted decimal marks parameterized: the duration door
/// takes ISO 8601's comma alongside the dot (`PT1,5S`, `0:00:01,5`, `1,5s` — eurozone
/// feeds really send these), unambiguously — durations have no digit grouping, so a
/// comma there can only be a decimal mark. RFC 3339 timestamps stay dot-only per spec.
fn read_fraction_marked(
    text: &[u8],
    at: usize,
    start: usize,
    marks: &[u8],
) -> Result<(u32, usize), Fault> {
    match text.get(at) {
        Some(byte) if marks.contains(byte) => {}
        _ => return Ok((0, at)),
    }
    let mut i = at + 1;
    let mut nanos: u32 = 0;
    let mut digits = 0;
    while i < text.len() && text[i].is_ascii_digit() {
        if digits == 9 {
            return Err(Fault::malformed(start + i, 1));
        }
        nanos = nanos * 10 + (text[i] - b'0') as u32;
        digits += 1;
        i += 1;
    }
    if digits == 0 {
        return Err(Fault::malformed(start + at, 1));
    }
    while digits < 9 {
        nanos *= 10;
        digits += 1;
    }
    Ok((nanos, i))
}

/// A `Malformed` fault at `at`, clamped to the trimmed text: a span that would start or
/// run past the end — the "input ended too soon" faults — becomes a zero-length span at
/// the truncation point, so `offset + len <= input.len()` holds for every fault a binding
/// might slice with (the invariant the fuzz target pins; found by it, not assumed).
fn malformed_at(text: &[u8], start: usize, at: usize, len: usize) -> Fault {
    if at >= text.len() {
        Fault::malformed(start + text.len(), 0)
    } else {
        Fault::malformed(start + at, len.min(text.len() - at))
    }
}

/// Where the fields of a strict `yyyy-MM-dd` date sit: offset and length of the year, the
/// month and the day, relative to the date's first byte.
const ISO_DATE_SPANS: [(usize, usize); 3] = [(0, 4), (5, 2), (8, 2)];

/// Reads the strict date prefix `yyyy-MM-dd` at the head of `text` for its shape only —
/// four, two and two digits, hyphens between — faulting `Malformed` into the caller's
/// coordinates at the first piece that is wrong. The values are not checked here:
/// [`check_date`] does that once the caller has read the rest of its input, so a shape
/// fault anywhere in the text is reported ahead of an impossible value.
fn read_date(text: &[u8], start: usize) -> Result<(u32, u32, u32), Fault> {
    let year = read4(text, 0).ok_or_else(|| Fault::malformed(start, text.len().min(4)))?;
    if text.get(4) != Some(&b'-') {
        return Err(Fault::malformed(start + 4, 1));
    }
    let month = read2(text, 5).ok_or(Fault::malformed(start + 5, 2))?;
    if text.get(7) != Some(&b'-') {
        return Err(Fault::malformed(start + 7, 1));
    }
    let day = read2(text, 8).ok_or(Fault::malformed(start + 8, 2))?;
    Ok((year, month, day))
}

/// Checks that a well-formed date names a real day. `spans` are the year, month and day
/// fields' offsets and lengths within the trimmed text. Year 0000, a month outside 1–12 or
/// a day the month does not have ⇒ `OutOfRange` at that field's digits: the text is shaped
/// like a date, and names one that does not exist.
fn check_date(
    year: u32,
    month: u32,
    day: u32,
    spans: [(usize, usize); 3],
    start: usize,
) -> Result<Date, Fault> {
    let fault = |(at, len): (usize, usize)| Fault::out_of_range(start + at, len);
    if year == 0 {
        return Err(fault(spans[0]));
    }
    if !(1..=12).contains(&month) {
        return Err(fault(spans[1]));
    }
    if day == 0 || day > days_in_month(i64::from(year), month) {
        return Err(fault(spans[2]));
    }
    Ok(Date { year: year as u16, month: month as u8, day: day as u8 })
}

/// Casts a strict ISO 8601 `yyyy-MM-dd` calendar date. Empty ⇒ `Empty`; anything
/// time-bearing or non-ISO ⇒ `Malformed`; year 0000, month 00 or 13+, or a day the month
/// does not have (`2026-02-29`) ⇒ `OutOfRange` at that field.
pub fn cast_date(input: impl AsRef<[u8]>) -> Result<Date, Fault> {
    let input = input.as_ref();
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }
    if text.len() != 10 {
        return Err(Fault::malformed(start, text.len()));
    }
    let (year, month, day) = read_date(text, start)?;
    check_date(year, month, day, ISO_DATE_SPANS, start)
}

/// The caller-declared field order of a separated calendar date. There is no guessing —
/// `1/7/2026` is January 7th or July 1st only because the caller said which (en-US short
/// dates are month-first, en-GB and most of the world day-first, ISO year-first), the same
/// declare-don't-sniff stance [`NumFormat`](crate::NumFormat) takes for numeric notation
/// and [`UnixPrecision`] takes for epoch magnitude. The strict [`cast_date`] door keeps
/// rejecting every separated form: an *undeclared* `1/7/2026` stays `Malformed` everywhere.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateOrder {
    /// Year, month, day — ISO's order with any accepted separator (`2026/1/7`, `2026.1.7`;
    /// strict `2026-01-07` is a subset).
    YearMonthDay = 1,
    /// Month, day, year — the en-US short-date order (`1/7/2026` is January 7th).
    MonthDayYear = 2,
    /// Day, month, year — the en-GB/most-of-the-world order (`1/7/2026` is July 1st).
    DayMonthYear = 3,
}

/// Reads a run of 1..=4 ASCII digits at `at` (a calendar date field), returning the value
/// and the index after the run. A zero-length or five-plus-digit run faults at the run
/// itself. Thin wrapper over the duration parser's [`read_digit_run`].
fn read_date_field(text: &[u8], at: usize, start: usize) -> Result<(u32, usize), Fault> {
    let (value, digits, after) = read_digit_run(text, at, start)?;
    if digits == 0 || digits > 4 {
        return Err(malformed_at(text, start, at, digits.max(1)));
    }
    Ok((value as u32, after))
}

/// A separated date whose shape [`read_ordered_date`] has accepted, its fields put in
/// year-month-day order with their spans, not yet checked by [`check_date`].
struct OrderedDate {
    fields: [u32; 3],
    spans: [(usize, usize); 3],
}

impl OrderedDate {
    fn check(&self, start: usize) -> Result<Date, Fault> {
        let [year, month, day] = self.fields;
        check_date(year, month, day, self.spans, start)
    }
}

/// Parses a separated calendar date at the head of `text` under the declared order, for
/// its shape, returning it and the index after it. Shared by [`cast_date_ordered`] (which
/// then demands end-of-input) and [`cast_datetime`] (which continues into the time part);
/// each range-checks it once the rest of the input has been read.
fn read_ordered_date(
    text: &[u8],
    start: usize,
    order: DateOrder,
) -> Result<(OrderedDate, usize), Fault> {
    let (first, first_end) = read_date_field(text, 0, start)?;
    let sep = match text.get(first_end) {
        Some(&sep @ (b'/' | b'-' | b'.')) => sep,
        _ => return Err(malformed_at(text, start, first_end, 1)),
    };
    let (second, second_end) = read_date_field(text, first_end + 1, start)?;
    if text.get(second_end) != Some(&sep) {
        return Err(malformed_at(text, start, second_end, 1));
    }
    let (third, third_end) = read_date_field(text, second_end + 1, start)?;

    // Field spans, for pointing a fault at the offending digits.
    let spans = [
        (0, first_end),
        (first_end + 1, second_end - first_end - 1),
        (second_end + 1, third_end - second_end - 1),
    ];
    // A four-digit FIRST field can only be a year (month and day never exceed two digits),
    // so a year-first date is structurally unambiguous under any declared order —
    // "2026/1/7" and ISO "2026-01-07" read year-month-day even when the declaration says
    // month- or day-first. This is width detection, not value sniffing: the genuinely
    // ambiguous forms ("1/7/2026" under a wrong declaration) still parse exactly as
    // declared, because no structure distinguishes them.
    let effective = if spans[0].1 == 4 { DateOrder::YearMonthDay } else { order };
    let fields = [first, second, third];
    let (year_at, month_at, day_at) = match effective {
        DateOrder::YearMonthDay => (0, 1, 2),
        DateOrder::MonthDayYear => (2, 0, 1),
        DateOrder::DayMonthYear => (2, 1, 0),
    };
    let field_fault = |at: usize| Fault::malformed(start + spans[at].0, spans[at].1);

    // The year field is four digits wherever the order puts it; month and day are one or
    // two (a three-digit field faults at its own digits). A two-digit year would mean
    // century guessing, which this core never does.
    if spans[month_at].1 > 2 {
        return Err(field_fault(month_at));
    }
    if spans[day_at].1 > 2 {
        return Err(field_fault(day_at));
    }
    if spans[year_at].1 != 4 {
        return Err(field_fault(year_at));
    }
    let date = OrderedDate {
        fields: [fields[year_at], fields[month_at], fields[day_at]],
        spans: [spans[year_at], spans[month_at], spans[day_at]],
    };
    Ok((date, third_end))
}

/// Casts a separated calendar date — three digit fields joined by one consistent separator
/// (`/`, `-`, or `.`) — under the caller-declared [`DateOrder`]. The year field must be
/// four digits wherever the order puts it (two-digit years mean century guessing, which
/// this core never does — `Malformed`); month and day take one or two; a four-digit
/// *first* field is structurally a year, so year-first dates parse under any declared
/// order. Empty ⇒ `Empty`; year 0000, an impossible month or a day the month does not
/// have ⇒ `OutOfRange` at that field's own digits.
pub fn cast_date_ordered(input: impl AsRef<[u8]>, order: DateOrder) -> Result<Date, Fault> {
    let input = input.as_ref();
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }
    let (date, end) = read_ordered_date(text, start, order)?;
    if end != text.len() {
        return Err(Fault::malformed(start + end, char_len_at(text, end)));
    }
    date.check(start)
}

/// A time of day whose shape has been read, not yet range-checked: each field's value and
/// where its digits sit in the trimmed text. Minutes or seconds the text leaves out read
/// as zero, which is always in range, so their spans are never reported.
struct Clock {
    hour: u32,
    minute: u32,
    second: u32,
    nanos: u32,
    spans: [(usize, usize); 3],
}

impl Clock {
    /// Checks the fields against a clock whose hours run over `hours`: an hour outside it,
    /// a minute past 59 or a second past 59 ⇒ `OutOfRange` at that field's digits. A leap
    /// second (`:60`) is out of range too: protobuf timestamps have no representation for
    /// it, and a deterministic core doesn't smear.
    fn check(&self, hours: RangeInclusive<u32>, start: usize) -> Result<(), Fault> {
        let fault = |(at, len): (usize, usize)| Fault::out_of_range(start + at, len);
        if !hours.contains(&self.hour) {
            return Err(fault(self.spans[0]));
        }
        if self.minute > 59 {
            return Err(fault(self.spans[1]));
        }
        if self.second > 59 {
            return Err(fault(self.spans[2]));
        }
        Ok(())
    }

    /// Nanoseconds since midnight, with the (checked) hour given on the 24-hour clock.
    fn nanos_of_day(&self, hour: u32) -> u64 {
        let total = u64::from(hour) * 3_600 + u64::from(self.minute) * 60 + u64::from(self.second);
        total * 1_000_000_000 + u64::from(self.nanos)
    }
}

/// Reads the civil time part of [`cast_datetime`] at `at` for its shape:
/// `h[:mm[:ss[.f{1..9}]]]`, hour one or two digits, with an optional case-insensitive
/// `AM`/`PM` marker (preceding space optional). Without a marker minutes are mandatory (a
/// bare trailing number is not a time). Returns the clock, the marker (`Some(true)` for
/// PM) and the index after the time; [`civil_nanos`] range-checks them.
fn read_civil_time(
    text: &[u8],
    at: usize,
    start: usize,
) -> Result<(Clock, Option<bool>, usize), Fault> {
    let (hour, hour_digits, mut i) = read_digit_run(text, at, start)?;
    if hour_digits == 0 || hour_digits > 2 {
        return Err(malformed_at(text, start, at, hour_digits.max(1)));
    }
    let mut clock =
        Clock { hour: hour as u32, minute: 0, second: 0, nanos: 0, spans: [(at, hour_digits); 3] };
    let mut has_minutes = false;
    if text.get(i) == Some(&b':') {
        clock.minute = read2(text, i + 1).ok_or_else(|| malformed_at(text, start, i + 1, 2))?;
        clock.spans[1] = (i + 1, 2);
        has_minutes = true;
        i += 3;
        if text.get(i) == Some(&b':') {
            clock.second = read2(text, i + 1).ok_or_else(|| malformed_at(text, start, i + 1, 2))?;
            clock.spans[2] = (i + 1, 2);
            i += 3;
            let (fraction, after) = read_fraction(text, i, start)?;
            clock.nanos = fraction;
            i = after;
        }
    }
    // Optional meridiem: [space] AM/PM, ASCII case-insensitive.
    let mut meridiem_at = i;
    if text.get(meridiem_at) == Some(&b' ') {
        meridiem_at += 1;
    }
    let marker = match (
        text.get(meridiem_at).map(u8::to_ascii_lowercase),
        text.get(meridiem_at + 1).map(u8::to_ascii_lowercase),
    ) {
        (Some(b'a'), Some(b'm')) => Some(false),
        (Some(b'p'), Some(b'm')) => Some(true),
        _ => None,
    };
    match marker {
        Some(_) => i = meridiem_at + 2,
        // A bare trailing number is only a time when a meridiem names it one.
        None if !has_minutes => return Err(Fault::malformed(start + at, hour_digits)),
        None => {}
    }
    Ok((clock, marker, i))
}

/// Range-checks a civil time and converts it to nanoseconds since midnight. With a marker
/// the hour is `1..=12` (`12 AM` is midnight, `12 PM` noon); without one it is `0..=23`.
fn civil_nanos(clock: &Clock, pm: Option<bool>, start: usize) -> Result<u64, Fault> {
    let hour = match pm {
        Some(pm) => {
            clock.check(1..=12, start)?;
            clock.hour % 12 + if pm { 12 } else { 0 }
        }
        None => {
            clock.check(0..=23, start)?;
            clock.hour
        }
    };
    Ok(clock.nanos_of_day(hour))
}

/// Casts a civil (wall-clock) date and time with **no zone** — the shape untrusted feeds
/// actually send (`1/7/2026 3:04 PM`, `2026-01-07 15:04:05`, `1.7.2026`) — under the
/// caller-declared [`DateOrder`], to a [`CivilDateTime`]. The date part follows
/// [`cast_date_ordered`]'s grammar (year-first forms, ISO included, parse under any
/// declared order); the optional time part — separated by one space or `T` — is 24-hour
/// `h:mm[:ss[.f{1..9}]]` or 12-hour with an `AM`/`PM` marker (`3 PM` allowed, `12 AM` is
/// midnight); absent, the time is midnight. No zone is read and none is invented — a
/// zone-less text names no instant, so fusing a zone is the caller's job
/// ([`cast_timestamp`] remains the strict RFC 3339 instant door).
pub fn cast_datetime(input: impl AsRef<[u8]>, order: DateOrder) -> Result<CivilDateTime, Fault> {
    let input = input.as_ref();
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }
    let (date, date_end) = read_ordered_date(text, start, order)?;
    match text.get(date_end) {
        None => return Ok(CivilDateTime { date: date.check(start)?, nanos_of_day: 0 }),
        Some(b' ' | b'T' | b't') => {}
        Some(_) => return Err(Fault::malformed(start + date_end, char_len_at(text, date_end))),
    }
    let (clock, pm, end) = read_civil_time(text, date_end + 1, start)?;
    if end != text.len() {
        return Err(Fault::malformed(start + end, char_len_at(text, end)));
    }
    let date = date.check(start)?;
    Ok(CivilDateTime { date, nanos_of_day: civil_nanos(&clock, pm, start)? })
}

/// Casts an ISO 8601 24-hour time-of-day — `HH:mm`, `HH:mm:ss`, or `HH:mm:ss.f{1..9}` —
/// to nanoseconds since midnight, `00:00` through `23:59:59.999999999`. Empty ⇒ `Empty`;
/// a well-formed hour past 23, minute past 59 or second past 59 (`24:00`, `15:04:60`) ⇒
/// `OutOfRange` at that field; anything else wrong ⇒ `Malformed`.
pub fn cast_time(input: impl AsRef<[u8]>) -> Result<u64, Fault> {
    let input = input.as_ref();
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }
    let (clock, end) = read_time(text, 0, start)?;
    if end != text.len() {
        return Err(Fault::malformed(start + end, char_len_at(text, end)));
    }
    clock.check(0..=23, start)?;
    Ok(clock.nanos_of_day(clock.hour))
}

/// Parses `HH:mm[:ss[.f{1..9}]]` at `at` for its shape, returning the clock (not yet
/// range-checked) and the index after the time.
fn read_time(text: &[u8], at: usize, start: usize) -> Result<(Clock, usize), Fault> {
    let hour = read2(text, at)
        .ok_or_else(|| Fault::malformed(start + at, (text.len() - at).clamp(1, 2)))?;
    if text.get(at + 2) != Some(&b':') {
        return Err(malformed_at(text, start, at + 2, 1));
    }
    let minute = read2(text, at + 3).ok_or_else(|| malformed_at(text, start, at + 3, 2))?;
    let mut clock =
        Clock { hour, minute, second: 0, nanos: 0, spans: [(at, 2), (at + 3, 2), (at + 3, 2)] };
    let mut i = at + 5;
    if text.get(i) == Some(&b':') {
        clock.second = read2(text, i + 1).ok_or_else(|| malformed_at(text, start, i + 1, 2))?;
        clock.spans[2] = (i + 1, 2);
        i += 3;
        let (fraction, after) = read_fraction(text, i, start)?;
        clock.nanos = fraction;
        i = after;
    }
    Ok((clock, i))
}

/// Casts an RFC 3339 instant — `yyyy-MM-ddTHH:mm:ss[.f{1..9}](Z|±hh:mm)` — to a protobuf
/// [`Timestamp`], normalized to UTC. The zone is mandatory (a zone-less or space-separated
/// form is `Malformed`, Svartalfheim parity); `-00:00` is accepted as UTC; `T`/`Z` are
/// case-insensitive; seconds are mandatory. Every piece is checked for shape before any
/// value is: a well-formed field that names nothing — year 0000, month 13, `02-30`, hour
/// 24, a `:60` leap second, an offset of `+24:00` — ⇒ `OutOfRange` at that field, and an
/// instant whose fields are all real but which falls outside the window (an offset pushing
/// past an edge) ⇒ `OutOfRange` spanning the token.
pub fn cast_timestamp(input: impl AsRef<[u8]>) -> Result<Timestamp, Fault> {
    let input = input.as_ref();
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }
    if text.len() < 11 {
        return Err(Fault::malformed(start, text.len()));
    }
    let (year, month, day) = read_date(&text[..10], start)?;
    if !matches!(text[10], b'T' | b't') {
        return Err(Fault::malformed(start + 10, char_len_at(text, 10)));
    }

    // The time segment reuses read_time's grammar but with seconds mandatory.
    if text.get(13) != Some(&b':') || text.get(16) != Some(&b':') {
        return Err(Fault::malformed(start + 11, text.len() - 11));
    }
    let (clock, after_time) = read_time(text, 11, start)?;

    // The zone, for shape: `None` is UTC, `Some` the sign, hours and minutes of an offset.
    let offset = match text.get(after_time) {
        None => {
            // Zone-less — an ambiguous instant, rejected whole.
            return Err(Fault::malformed(start, text.len()));
        }
        Some(&(b'Z' | b'z')) => {
            if after_time + 1 != text.len() {
                return Err(Fault::malformed(
                    start + after_time + 1,
                    char_len_at(text, after_time + 1),
                ));
            }
            None
        }
        Some(&sign @ (b'+' | b'-')) => {
            let hours = read2(text, after_time + 1)
                .ok_or_else(|| malformed_at(text, start, after_time + 1, 2))?;
            if text.get(after_time + 3) != Some(&b':') {
                return Err(malformed_at(text, start, after_time + 3, 1));
            }
            let minutes = read2(text, after_time + 4)
                .ok_or_else(|| malformed_at(text, start, after_time + 4, 2))?;
            if after_time + 6 != text.len() {
                return Err(Fault::malformed(
                    start + after_time + 6,
                    char_len_at(text, after_time + 6),
                ));
            }
            Some((sign, hours, minutes))
        }
        Some(_) => {
            return Err(Fault::malformed(start + after_time, char_len_at(text, after_time)));
        }
    };

    // Every piece is well-formed; now each value, in reading order.
    let date = check_date(year, month, day, ISO_DATE_SPANS, start)?;
    clock.check(0..=23, start)?;
    let offset_seconds = match offset {
        None => 0i64,
        Some((sign, hours, minutes)) => {
            if hours > 23 {
                return Err(Fault::out_of_range(start + after_time + 1, 2));
            }
            if minutes > 59 {
                return Err(Fault::out_of_range(start + after_time + 4, 2));
            }
            let magnitude = i64::from(hours) * 3_600 + i64::from(minutes) * 60;
            if sign == b'-' { -magnitude } else { magnitude }
        }
    };

    let nanos_of_day = clock.nanos_of_day(clock.hour);
    let day_seconds = (nanos_of_day / 1_000_000_000) as i64;
    let nanos = (nanos_of_day % 1_000_000_000) as i32;
    let seconds = days_from_civil(i64::from(date.year), u32::from(date.month), u32::from(date.day))
        * 86_400
        + day_seconds
        - offset_seconds;
    if !(MIN_TIMESTAMP_SECONDS..=MAX_TIMESTAMP_SECONDS).contains(&seconds) {
        return Err(Fault::out_of_range(start, text.len()));
    }
    Ok(Timestamp { seconds, nanos })
}

/// Casts an integer Unix-epoch value under a caller-declared unit to a protobuf
/// [`Timestamp`]. Negatives (pre-1970) are allowed; a fractional or non-integer value ⇒
/// `Malformed`; outside the window ⇒ `OutOfRange`. Sub-second units land in `nanos`, which
/// stays non-negative even before the epoch (seconds floor toward -∞, protobuf convention).
pub fn cast_unix(input: impl AsRef<[u8]>, precision: UnixPrecision) -> Result<Timestamp, Fault> {
    let input = input.as_ref();
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }
    let mut i = 0;
    let negative = match text[0] {
        b'-' => {
            i = 1;
            true
        }
        b'+' => {
            i = 1;
            false
        }
        _ => false,
    };
    if i == text.len() {
        return Err(Fault::malformed(start, text.len()));
    }
    let mut magnitude: i128 = 0;
    let mut over = false;
    while i < text.len() {
        let byte = text[i];
        if !byte.is_ascii_digit() {
            return Err(Fault::malformed(start + i, char_len_at(text, i)));
        }
        magnitude = match magnitude
            .checked_mul(10)
            .and_then(|shifted| shifted.checked_add((byte - b'0') as i128))
        {
            Some(next) => next,
            None => {
                over = true;
                magnitude
            }
        };
        i += 1;
    }
    if over {
        return Err(Fault::out_of_range(start, text.len()));
    }
    let epoch = if negative { -magnitude } else { magnitude };
    let per_second: i128 = match precision {
        UnixPrecision::Seconds => 1,
        UnixPrecision::Millis => 1_000,
        UnixPrecision::Micros => 1_000_000,
        UnixPrecision::Nanos => 1_000_000_000,
    };
    let seconds = epoch.div_euclid(per_second);
    let nanos = epoch.rem_euclid(per_second) * (NANOS_PER_SECOND / per_second);
    if seconds < MIN_TIMESTAMP_SECONDS as i128 || seconds > MAX_TIMESTAMP_SECONDS as i128 {
        return Err(Fault::out_of_range(start, text.len()));
    }
    Ok(Timestamp { seconds: seconds as i64, nanos: nanos as i32 })
}

/// The day a whole serial names, as days from the Unix epoch — or `None` where the system
/// has no such day: below its first serial (`1` for 1900, `0` for 1904), past
/// `9999-12-31`, or the 1900 system's phantom serial `60`. The one statement of the rule,
/// read by the text door and the typed one alike.
const fn excel_day_number(days: i64, epoch: ExcelEpoch) -> Option<i64> {
    match epoch {
        ExcelEpoch::Y1900 => {
            if days < 1 || days > MAX_EXCEL_1900_SERIAL || days == EXCEL_1900_PHANTOM_SERIAL {
                return None;
            }
            // Below the phantom the 1900 system counts true days from 1899-12-31; above
            // it, every serial has absorbed the extra day.
            Some(EXCEL_1900_ANCHOR_DAYS + days + (days < EXCEL_1900_PHANTOM_SERIAL) as i64)
        }
        ExcelEpoch::Y1904 => {
            if days < 0 || days > MAX_EXCEL_1904_SERIAL {
                return None;
            }
            Some(EXCEL_1904_ANCHOR_DAYS + days)
        }
    }
}

/// Reads an Excel date serial already held as the number a workbook stores — the typed
/// twin of [`cast_excel_serial`], for a reader that has the cell's `f64` and no text to
/// parse. The same two systems under the same rules, from the same code: the phantom
/// serial `60` of the 1900 system, a serial below the system's first real day (`1` for
/// 1900, `0` for 1904) and one past `9999-12-31` are `OutOfRange`; a negative, NaN or
/// infinite value is `Malformed`.
///
/// The result is the zone-less wall clock the cell holds, a [`CivilDateTime`];
/// [`CivilDateTime::assume_utc`] makes it the instant the text door returns. The fraction
/// is rounded to the nearest nanosecond, where the text door truncates digits it was given
/// exactly: a double near serial 45,000 resolves about 0.6 µs, so its last digits are
/// noise either way, and rounding is what keeps `0.5` at noon when the stored double sits
/// one ulp under it. A fraction that rounds up to a whole day carries into the date.
///
/// The verdict is a bare [`Reason`]: there is no text for a [`Fault`]'s span to index.
pub fn excel_serial(serial: f64, epoch: ExcelEpoch) -> Result<CivilDateTime, Reason> {
    if !serial.is_finite() || serial < 0.0 {
        return Err(Reason::Malformed);
    }
    // Past either system's last day; also what keeps the casts below exact.
    if serial >= (MAX_EXCEL_1900_SERIAL + 1) as f64 {
        return Err(Reason::OutOfRange);
    }
    // `core` has no `floor` or `round`. The value is non-negative and far below 2^53, so
    // the cast truncates to its floor exactly, and the remainder decides the rounding.
    let mut days = serial as i64;
    let scaled = (serial - days as f64) * NANOS_PER_DAY as f64;
    let mut nanos_of_day = scaled as u64;
    if scaled - nanos_of_day as f64 >= 0.5 {
        nanos_of_day += 1;
    }
    if nanos_of_day >= NANOS_PER_DAY {
        days += 1;
        nanos_of_day -= NANOS_PER_DAY;
    }
    let day_number = excel_day_number(days, epoch).ok_or(Reason::OutOfRange)?;
    let (year, month, day) = civil_from_days(day_number);
    Ok(CivilDateTime {
        date: Date { year: year as u16, month: month as u8, day: day as u8 },
        nanos_of_day,
    })
}

/// Casts an Excel date serial under a caller-declared [`ExcelEpoch`] to a protobuf
/// [`Timestamp`]. The whole part counts days from the system's own day zero; the fraction
/// is the time of day (`0.5` is noon), so `45292.75` is `2024-01-01T18:00:00Z`. The result
/// is a wall-clock instant read as UTC — a spreadsheet cell carries no zone, and this door
/// invents none.
///
/// **The 1900 system contains a day that never existed.** Serial `60` is `1900-02-29`, kept
/// deliberately since Lotus 1-2-3 wrongly treated 1900 as a leap year and Excel copied the
/// bug for file compatibility. It is rejected as `OutOfRange` here — the same verdict
/// [`cast_date`] already gives the text `1900-02-29`, so both doors agree that day does not
/// exist. Every serial above it is therefore shifted one day against a naive count, which is
/// the arithmetic hand-rolled conversions get wrong.
///
/// Non-numeric text, a bare or trailing `.`, or a sign ⇒ `Malformed`; a serial below the
/// system's first real day (`1` for 1900, `0` for 1904) or beyond `9999-12-31` ⇒
/// `OutOfRange`. A negative is never a date in either system, so it is `Malformed` at the
/// sign rather than silently reflected. Time-only cells — Excel writes those as a bare
/// fraction below `1` — are not instants; use [`cast_time`] for those.
pub fn cast_excel_serial(input: impl AsRef<[u8]>, epoch: ExcelEpoch) -> Result<Timestamp, Fault> {
    let input = input.as_ref();
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }

    let mut i = 0;
    let mut days: i64 = 0;
    let mut over = false;
    while let Some(&digit @ b'0'..=b'9') = text.get(i) {
        days = match days
            .checked_mul(10)
            .and_then(|shifted| shifted.checked_add((digit - b'0') as i64))
        {
            Some(next) => next,
            None => {
                over = true;
                days
            }
        };
        i += 1;
    }
    if i == 0 {
        return Err(Fault::malformed(start, char_len_at(text, 0)));
    }

    let mut nanos_of_day: i128 = 0;
    if text.get(i) == Some(&b'.') {
        i += 1;
        let fraction_start = i;
        // `scale` is NonZero by type, so the division below has no zero to check for.
        const TEN: NonZero<u128> = NonZero::new(10).unwrap();
        let mut scaled: u128 = 0;
        let mut scale = NonZero::<u128>::MIN;
        while let Some(&digit @ b'0'..=b'9') = text.get(i) {
            if i - fraction_start < MAX_EXCEL_FRACTION_DIGITS {
                scaled = scaled * 10 + u128::from(digit - b'0');
                scale = scale.saturating_mul(TEN);
            }
            i += 1;
        }
        if i == fraction_start {
            // Point at the `.` itself: a trailing separator has nothing after it to span.
            return Err(Fault::malformed(start + fraction_start - 1, 1));
        }
        nanos_of_day = (scaled * 86_400 * NANOS_PER_SECOND as u128 / scale) as i128;
    }
    if i != text.len() {
        return Err(Fault::malformed(start + i, char_len_at(text, i)));
    }

    let day_number = match excel_day_number(days, epoch) {
        Some(day_number) if !over => day_number,
        _ => return Err(Fault::out_of_range(start, text.len())),
    };

    let seconds = day_number * 86_400 + (nanos_of_day / NANOS_PER_SECOND) as i64;
    let nanos = (nanos_of_day % NANOS_PER_SECOND) as i32;
    if !(MIN_TIMESTAMP_SECONDS..=MAX_TIMESTAMP_SECONDS).contains(&seconds) {
        return Err(Fault::out_of_range(start, text.len()));
    }
    Ok(Timestamp { seconds, nanos })
}

/// Parse-sanity bound on any single digit run in a duration — Svartalfheim's `MaxDigits`.
/// Not the overflow guard; overflow is the checked total-nanos arithmetic.
const MAX_DURATION_DIGITS: usize = 18;

/// Casts a duration in any of three cleanly-partitioned shapes to a protobuf [`Duration`]:
/// a leading `[-]P` is an ISO 8601 duration restricted to fixed components (`nW`/`nD`, then
/// `T` with `nH`/`nM`/`n[.f{1..9}]S` — years and months are not fixed durations and are
/// `Malformed`); a token containing `:` is the invariant colon form `[-][d.]hh:mm[:ss[.f]]`;
/// `[-]digits[.f{1..9}]s` is the protobuf JSON form. Beyond ±10,000 years of whole seconds
/// ⇒ `OutOfRange`. `seconds` and `nanos` come out same-signed.
pub fn cast_duration(input: impl AsRef<[u8]>) -> Result<Duration, Fault> {
    let input = input.as_ref();
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }
    let signed_head = if text[0] == b'-' { 1 } else { 0 };
    let total_nanos = if text.len() > signed_head && (text[signed_head] | 0x20) == b'p' {
        parse_iso_duration(text, start)?
    } else if text.contains(&b':') {
        parse_colon_duration(text, start)?
    } else {
        parse_protobuf_duration(text, start)?
    };

    let seconds = total_nanos / NANOS_PER_SECOND;
    if seconds.unsigned_abs() > MAX_DURATION_SECONDS as u128 {
        return Err(Fault::out_of_range(start, text.len()));
    }
    Ok(Duration { seconds: seconds as i64, nanos: (total_nanos % NANOS_PER_SECOND) as i32 })
}

/// Reads a bounded ASCII digit run, returning (value, digit count, next index).
fn read_digit_run(text: &[u8], at: usize, start: usize) -> Result<(i128, usize, usize), Fault> {
    let mut i = at;
    let mut value: i128 = 0;
    while i < text.len() && text[i].is_ascii_digit() {
        if i - at == MAX_DURATION_DIGITS {
            return Err(Fault::malformed(start + i, 1));
        }
        value = value * 10 + (text[i] - b'0') as i128;
        i += 1;
    }
    Ok((value, i - at, i))
}

/// The ISO 8601 grammar, ported from Svartalfheim's `TryParseIso8601Duration`:
/// `[-] 'P' { n('W'|'D') } [ 'T' { n('H'|'M') | n[.f]('S') } ]` — at least one component,
/// fraction only on seconds, year/month and misplaced units rejected.
fn parse_iso_duration(text: &[u8], start: usize) -> Result<i128, Fault> {
    let mut i = 0;
    let negative = text[0] == b'-';
    if negative {
        i = 1;
    }
    i += 1; // the sniffed 'P'

    let mut total: i128 = 0;
    let mut in_time = false;
    let mut saw_component = false;
    let mut saw_time_component = false;
    while i < text.len() {
        if matches!(text[i], b'T' | b't') {
            if in_time {
                return Err(Fault::malformed(start + i, 1));
            }
            in_time = true;
            i += 1;
            continue;
        }

        let run_start = i;
        let (value, digits, after) = read_digit_run(text, i, start)?;
        i = after;
        if digits == 0 {
            let bad = i.min(text.len() - 1);
            return Err(Fault::malformed(start + bad, char_len_at(text, bad)));
        }
        let mut fraction_nanos: i128 = 0;
        let mut has_fraction = false;
        if matches!(text.get(i), Some(&b'.') | Some(&b',')) {
            has_fraction = true;
            let (nanos, after_fraction) = read_fraction_marked(text, i, start, b".,")?;
            fraction_nanos = nanos as i128;
            i = after_fraction;
        }
        if i >= text.len() {
            // A number with no unit.
            return Err(Fault::malformed(start + run_start, i - run_start));
        }
        let unit = text[i];
        let unit_pos = i;
        i += 1;

        let nanos_per_unit: i128 = match unit {
            b'W' | b'w' if !in_time => 604_800 * NANOS_PER_SECOND,
            b'D' | b'd' if !in_time => 86_400 * NANOS_PER_SECOND,
            b'H' | b'h' if in_time => 3_600 * NANOS_PER_SECOND,
            b'M' | b'm' if in_time => 60 * NANOS_PER_SECOND,
            b'S' | b's' if in_time => NANOS_PER_SECOND,
            // Y, M-before-T (months), or a misplaced unit.
            _ => return Err(Fault::malformed(start + unit_pos, char_len_at(text, unit_pos))),
        };
        if has_fraction && nanos_per_unit != NANOS_PER_SECOND {
            return Err(Fault::malformed(start + run_start, unit_pos - run_start + 1));
        }

        total = value
            .checked_mul(nanos_per_unit)
            .and_then(|scaled| scaled.checked_add(fraction_nanos))
            .and_then(|component| total.checked_add(component))
            .ok_or_else(|| Fault::out_of_range(start, text.len()))?;
        saw_component = true;
        if in_time {
            saw_time_component = true;
        }
    }

    if !saw_component || (in_time && !saw_time_component) {
        return Err(Fault::malformed(start, text.len()));
    }
    Ok(if negative { -total } else { total })
}

/// The invariant colon form `[-][d.]hh:mm[:ss[.f{1..9}]]` — hours 0–23 (a larger total
/// needs the day part), minutes and seconds 0–59, each 1–2 digits, .NET's invariant
/// `TimeSpan` profile with the fraction widened to nanos. Shape first, as everywhere: a
/// well-formed field past its range (`25:00:00`, `01:60:00`) ⇒ `OutOfRange` at its digits,
/// where .NET throws `OverflowException`.
fn parse_colon_duration(text: &[u8], start: usize) -> Result<i128, Fault> {
    let mut i = 0;
    let negative = text[0] == b'-';
    if negative {
        i = 1;
    }

    let (first, first_digits, after_first) = read_digit_run(text, i, start)?;
    if first_digits == 0 {
        let bad = after_first.min(text.len() - 1);
        return Err(Fault::malformed(start + bad, char_len_at(text, bad)));
    }
    i = after_first;

    let mut days: i128 = 0;
    let hours: i128;
    let mut hour_span = (after_first - first_digits, first_digits);
    if text.get(i) == Some(&b'.') {
        days = first;
        i += 1;
        let (h, h_digits, after_hours) = read_digit_run(text, i, start)?;
        if h_digits == 0 || h_digits > 2 {
            return Err(malformed_at(text, start, i, (after_hours - i).max(1)));
        }
        hours = h;
        hour_span = (i, h_digits);
        i = after_hours;
    } else {
        if first_digits > 2 {
            return Err(Fault::malformed(start + if negative { 1 } else { 0 }, first_digits));
        }
        hours = first;
    }

    if text.get(i) != Some(&b':') {
        let bad = i.min(text.len() - 1);
        return Err(Fault::malformed(start + bad, char_len_at(text, bad)));
    }
    i += 1;
    let (minutes, m_digits, after_minutes) = read_digit_run(text, i, start)?;
    if m_digits == 0 || m_digits > 2 {
        return Err(malformed_at(text, start, i, (after_minutes - i).max(1)));
    }
    let minute_span = (i, m_digits);
    i = after_minutes;

    let mut seconds: i128 = 0;
    let mut second_span = minute_span;
    let mut fraction_nanos: i128 = 0;
    if text.get(i) == Some(&b':') {
        i += 1;
        let (s, s_digits, after_seconds) = read_digit_run(text, i, start)?;
        if s_digits == 0 || s_digits > 2 {
            return Err(malformed_at(text, start, i, (after_seconds - i).max(1)));
        }
        seconds = s;
        second_span = (i, s_digits);
        i = after_seconds;
        let (nanos, after_fraction) = read_fraction_marked(text, i, start, b".,")?;
        fraction_nanos = nanos as i128;
        i = after_fraction;
    }
    if i != text.len() {
        return Err(Fault::malformed(start + i, char_len_at(text, i)));
    }
    for (value, (at, len), max) in
        [(hours, hour_span, 23), (minutes, minute_span, 59), (seconds, second_span, 59)]
    {
        if value > max {
            return Err(Fault::out_of_range(start + at, len));
        }
    }

    let total = ((days * 86_400 + hours * 3_600 + minutes * 60 + seconds) * NANOS_PER_SECOND)
        + fraction_nanos;
    Ok(if negative { -total } else { total })
}

/// The protobuf JSON form `[-]digits[.f{1..9}]s`, case-insensitive suffix.
fn parse_protobuf_duration(text: &[u8], start: usize) -> Result<i128, Fault> {
    let last = text.len() - 1;
    if (text[last] | 0x20) != b's' {
        return Err(Fault::malformed(start, text.len()));
    }
    let body = &text[..last];
    let mut i = 0;
    let negative = !body.is_empty() && body[0] == b'-';
    if negative {
        i = 1;
    }
    let (seconds, digits, after) = read_digit_run(body, i, start)?;
    if digits == 0 {
        return Err(Fault::malformed(start, text.len()));
    }
    let (nanos, after_fraction) = read_fraction_marked(body, after, start, b".,")?;
    if after_fraction != body.len() {
        return Err(Fault::malformed(start + after_fraction, char_len_at(body, after_fraction)));
    }
    let total = seconds * NANOS_PER_SECOND + nanos as i128;
    Ok(if negative { -total } else { total })
}
