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

use crate::integer::char_len;
use crate::verdict::{trim, CivilDateTime, Date, Duration, Fault, Timestamp};

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
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year.rem_euclid(400);
    let month_shift: i64 = if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * (month as i64 + month_shift) + 2) / 5 + day as i64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
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
/// core's full fidelity.
fn read_fraction(
    text: &[u8],
    at: usize,
    start: usize,
) -> Result<(u32, usize), Fault> {
    if text.get(at) != Some(&b'.') {
        return Ok((0, at));
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

/// Parses the strict date prefix `yyyy-MM-dd` at the head of `text`, faulting into the
/// caller's coordinates. Year 0000 is well-formed but unrepresentable ⇒ `OutOfRange`;
/// an impossible month or day ⇒ `Malformed` at its digits.
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
    if year == 0 {
        return Err(Fault::out_of_range(start, 10));
    }
    if !(1..=12).contains(&month) {
        return Err(Fault::malformed(start + 5, 2));
    }
    if day == 0 || day > days_in_month(year as i64, month) {
        return Err(Fault::malformed(start + 8, 2));
    }
    Ok((year, month, day))
}

/// Casts a strict ISO 8601 `yyyy-MM-dd` calendar date. Empty ⇒ `Empty`; anything
/// time-bearing or non-ISO ⇒ `Malformed`; year 0000 ⇒ `OutOfRange`.
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
    Ok(Date { year: year as u16, month: month as u8, day: day as u8 })
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
        return Err(Fault::malformed(start + at, digits.max(1)));
    }
    Ok((value as u32, after))
}

/// Parses a separated calendar date at the head of `text` under the declared order,
/// returning the [`Date`] and the index after it. Shared by [`cast_date_ordered`] (which
/// then demands end-of-input) and [`cast_datetime`] (which continues into the time part).
fn read_ordered_date(
    text: &[u8],
    start: usize,
    order: DateOrder,
) -> Result<(Date, usize), Fault> {
    let (first, first_end) = read_date_field(text, 0, start)?;
    let sep = match text.get(first_end) {
        Some(&sep @ (b'/' | b'-' | b'.')) => sep,
        _ => return Err(Fault::malformed(start + first_end, 1)),
    };
    let (second, second_end) = read_date_field(text, first_end + 1, start)?;
    if text.get(second_end) != Some(&sep) {
        return Err(Fault::malformed(start + second_end, 1));
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
    let (fields, year_at, month_at, day_at) = match effective {
        DateOrder::YearMonthDay => ([first, second, third], 0, 1, 2),
        DateOrder::MonthDayYear => ([first, second, third], 2, 0, 1),
        DateOrder::DayMonthYear => ([first, second, third], 2, 1, 0),
    };
    let field_fault = |at: usize| Fault::malformed(start + spans[at].0, spans[at].1);

    let (year, month, day) = (fields[year_at], fields[month_at], fields[day_at]);
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
    if year == 0 {
        return Err(Fault::out_of_range(start, third_end));
    }
    if !(1..=12).contains(&month) {
        return Err(field_fault(month_at));
    }
    if day == 0 || day > days_in_month(i64::from(year), month) {
        return Err(field_fault(day_at));
    }
    Ok((Date { year: year as u16, month: month as u8, day: day as u8 }, third_end))
}

/// Casts a separated calendar date — three digit fields joined by one consistent separator
/// (`/`, `-`, or `.`) — under the caller-declared [`DateOrder`]. The year field must be
/// four digits wherever the order puts it (two-digit years mean century guessing, which
/// this core never does — `Malformed`); month and day take one or two; a four-digit
/// *first* field is structurally a year, so year-first dates parse under any declared
/// order. Empty ⇒ `Empty`; year 0000 ⇒ `OutOfRange`; an impossible month or day ⇒
/// `Malformed` at its own digits.
pub fn cast_date_ordered(input: impl AsRef<[u8]>, order: DateOrder) -> Result<Date, Fault> {
    let input = input.as_ref();
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }
    let (date, end) = read_ordered_date(text, start, order)?;
    if end != text.len() {
        return Err(Fault::malformed(start + end, char_len(text[end])));
    }
    Ok(date)
}

/// Reads the civil time part of [`cast_datetime`] at `at`: `h[:mm[:ss[.f{1..9}]]]`, hour
/// one or two digits, with an optional case-insensitive `AM`/`PM` marker (preceding space
/// optional). With a marker the hour is `1..=12` (`12 AM` is midnight, `12 PM` noon);
/// without one the time is 24-hour and minutes are mandatory (a bare trailing number is
/// not a time). Returns nanos-since-midnight and the index after the time.
fn read_civil_time(text: &[u8], at: usize, start: usize) -> Result<(u64, usize), Fault> {
    let (hour_value, hour_digits, mut i) = read_digit_run(text, at, start)?;
    if hour_digits == 0 || hour_digits > 2 {
        return Err(Fault::malformed(start + at, hour_digits.max(1)));
    }
    let hour_span = (at, hour_digits);
    let mut hour = hour_value as u32;
    let mut minute = 0;
    let mut second = 0;
    let mut nanos = 0;
    let mut has_minutes = false;
    if text.get(i) == Some(&b':') {
        minute = read2(text, i + 1).ok_or(Fault::malformed(start + i + 1, 2))?;
        if minute > 59 {
            return Err(Fault::malformed(start + i + 1, 2));
        }
        has_minutes = true;
        i += 3;
        if text.get(i) == Some(&b':') {
            second = read2(text, i + 1).ok_or(Fault::malformed(start + i + 1, 2))?;
            // Leap seconds rejected, same as every other temporal door.
            if second > 59 {
                return Err(Fault::malformed(start + i + 1, 2));
            }
            i += 3;
            let (fraction, after) = read_fraction(text, i, start)?;
            nanos = fraction;
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
        Some(pm) => {
            if !(1..=12).contains(&hour) {
                return Err(Fault::malformed(start + hour_span.0, hour_span.1));
            }
            if hour == 12 {
                hour = 0;
            }
            if pm {
                hour += 12;
            }
            i = meridiem_at + 2;
        }
        None => {
            if !has_minutes {
                // A bare trailing number is only a time when a meridiem names it one.
                return Err(Fault::malformed(start + hour_span.0, hour_span.1));
            }
            if hour > 23 {
                return Err(Fault::malformed(start + hour_span.0, hour_span.1));
            }
        }
    }
    let total = u64::from(hour) * 3_600 + u64::from(minute) * 60 + u64::from(second);
    Ok((total * 1_000_000_000 + u64::from(nanos), i))
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
    if date_end == text.len() {
        return Ok(CivilDateTime { date, nanos_of_day: 0 });
    }
    if !matches!(text[date_end], b' ' | b'T' | b't') {
        return Err(Fault::malformed(start + date_end, char_len(text[date_end])));
    }
    let (nanos_of_day, end) = read_civil_time(text, date_end + 1, start)?;
    if end != text.len() {
        return Err(Fault::malformed(start + end, char_len(text[end])));
    }
    Ok(CivilDateTime { date, nanos_of_day })
}

/// Casts an ISO 8601 24-hour time-of-day — `HH:mm`, `HH:mm:ss`, or `HH:mm:ss.f{1..9}` —
/// to nanoseconds since midnight. Midnight and `23:59:59.999999999` are both real clock
/// readings, so there is no range failure on this door: empty ⇒ `Empty`, everything else
/// wrong ⇒ `Malformed`.
pub fn cast_time(input: impl AsRef<[u8]>) -> Result<u64, Fault> {
    let input = input.as_ref();
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }
    let (nanos, end) = read_time(text, 0, start)?;
    if end != text.len() {
        return Err(Fault::malformed(start + end, char_len(text[end])));
    }
    Ok(nanos)
}

/// Parses `HH:mm[:ss[.f{1..9}]]` at `at`, returning nanos-since-midnight and the index
/// after the time.
fn read_time(text: &[u8], at: usize, start: usize) -> Result<(u64, usize), Fault> {
    let hour = read2(text, at).ok_or_else(|| {
        Fault::malformed(start + at, (text.len() - at).clamp(1, 2))
    })?;
    if hour > 23 {
        return Err(Fault::malformed(start + at, 2));
    }
    if text.get(at + 2) != Some(&b':') {
        return Err(Fault::malformed(start + at + 2, 1));
    }
    let minute = read2(text, at + 3).ok_or(Fault::malformed(start + at + 3, 2))?;
    if minute > 59 {
        return Err(Fault::malformed(start + at + 3, 2));
    }
    let mut second = 0;
    let mut i = at + 5;
    let mut nanos = 0;
    if text.get(i) == Some(&b':') {
        second = read2(text, i + 1).ok_or(Fault::malformed(start + i + 1, 2))?;
        // A leap second (:60) is deliberately rejected — protobuf timestamps have no
        // representation for it, and a deterministic core doesn't smear.
        if second > 59 {
            return Err(Fault::malformed(start + i + 1, 2));
        }
        i += 3;
        let (fraction, after) = read_fraction(text, i, start)?;
        nanos = fraction;
        i = after;
    }
    let total = u64::from(hour) * 3_600 + u64::from(minute) * 60 + u64::from(second);
    Ok((total * 1_000_000_000 + u64::from(nanos), i))
}

/// Casts an RFC 3339 instant — `yyyy-MM-ddTHH:mm:ss[.f{1..9}](Z|±hh:mm)` — to a protobuf
/// [`Timestamp`], normalized to UTC. The zone is mandatory (a zone-less or space-separated
/// form is `Malformed`, Svartalfheim parity); `-00:00` is accepted as UTC; `T`/`Z` are
/// case-insensitive; seconds are mandatory; `:60` leap seconds are `Malformed`. A
/// well-formed instant outside the window (year 0000, or an offset pushing past an edge)
/// ⇒ `OutOfRange` spanning the token.
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
        return Err(Fault::malformed(start + 10, char_len(text[10])));
    }

    // The time segment reuses read_time's grammar but with seconds mandatory.
    if text.get(13) != Some(&b':') || text.get(16) != Some(&b':') {
        return Err(Fault::malformed(start + 11, text.len() - 11));
    }
    let (nanos_of_day, after_time) = read_time(text, 11, start)?;

    let offset_seconds = match text.get(after_time) {
        None => {
            // Zone-less — an ambiguous instant, rejected whole.
            return Err(Fault::malformed(start, text.len()));
        }
        Some(&(b'Z' | b'z')) => {
            if after_time + 1 != text.len() {
                return Err(Fault::malformed(start + after_time + 1, char_len(text[after_time + 1])));
            }
            0i64
        }
        Some(&sign @ (b'+' | b'-')) => {
            let hours = read2(text, after_time + 1)
                .ok_or(Fault::malformed(start + after_time + 1, 2))?;
            if text.get(after_time + 3) != Some(&b':') {
                return Err(Fault::malformed(start + after_time + 3, 1));
            }
            let minutes = read2(text, after_time + 4)
                .ok_or(Fault::malformed(start + after_time + 4, 2))?;
            if hours > 23 || minutes > 59 {
                return Err(Fault::malformed(start + after_time + 1, 5));
            }
            if after_time + 6 != text.len() {
                return Err(Fault::malformed(start + after_time + 6, char_len(text[after_time + 6])));
            }
            let magnitude = i64::from(hours) * 3_600 + i64::from(minutes) * 60;
            if sign == b'-' { -magnitude } else { magnitude }
        }
        Some(&other) => {
            return Err(Fault::malformed(start + after_time, char_len(other)));
        }
    };

    let day_seconds = (nanos_of_day / 1_000_000_000) as i64;
    let nanos = (nanos_of_day % 1_000_000_000) as i32;
    let seconds = days_from_civil(year as i64, month, day) * 86_400 + day_seconds - offset_seconds;
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
            return Err(Fault::malformed(start + i, char_len(byte)));
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
    Ok(Duration {
        seconds: seconds as i64,
        nanos: (total_nanos % NANOS_PER_SECOND) as i32,
    })
}

/// Reads a bounded ASCII digit run, returning (value, digit count, next index).
fn read_digit_run(
    text: &[u8],
    at: usize,
    start: usize,
) -> Result<(i128, usize, usize), Fault> {
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
            return Err(Fault::malformed(start + bad, char_len(text[bad])));
        }
        let mut fraction_nanos: i128 = 0;
        let mut has_fraction = false;
        if text.get(i) == Some(&b'.') {
            has_fraction = true;
            let (nanos, after_fraction) = read_fraction(text, i, start)?;
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
            _ => return Err(Fault::malformed(start + unit_pos, char_len(unit))),
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
/// `TimeSpan` profile with the fraction widened to nanos.
fn parse_colon_duration(text: &[u8], start: usize) -> Result<i128, Fault> {
    let mut i = 0;
    let negative = text[0] == b'-';
    if negative {
        i = 1;
    }

    let (first, first_digits, after_first) = read_digit_run(text, i, start)?;
    if first_digits == 0 {
        let bad = after_first.min(text.len() - 1);
        return Err(Fault::malformed(start + bad, char_len(text[bad])));
    }
    i = after_first;

    let mut days: i128 = 0;
    let hours: i128;
    if text.get(i) == Some(&b'.') {
        days = first;
        i += 1;
        let (h, h_digits, after_hours) = read_digit_run(text, i, start)?;
        if h_digits == 0 || h_digits > 2 {
            return Err(Fault::malformed(start + i, (after_hours - i).max(1)));
        }
        hours = h;
        i = after_hours;
    } else {
        if first_digits > 2 {
            return Err(Fault::malformed(start + if negative { 1 } else { 0 }, first_digits));
        }
        hours = first;
    }
    if hours > 23 {
        return Err(Fault::malformed(start, text.len()));
    }

    if text.get(i) != Some(&b':') {
        let bad = i.min(text.len() - 1);
        return Err(Fault::malformed(start + bad, char_len(text[bad])));
    }
    i += 1;
    let (minutes, m_digits, after_minutes) = read_digit_run(text, i, start)?;
    if m_digits == 0 || m_digits > 2 || minutes > 59 {
        return Err(Fault::malformed(start + i, (after_minutes - i).max(1)));
    }
    i = after_minutes;

    let mut seconds: i128 = 0;
    let mut fraction_nanos: i128 = 0;
    if text.get(i) == Some(&b':') {
        i += 1;
        let (s, s_digits, after_seconds) = read_digit_run(text, i, start)?;
        if s_digits == 0 || s_digits > 2 || s > 59 {
            return Err(Fault::malformed(start + i, (after_seconds - i).max(1)));
        }
        seconds = s;
        i = after_seconds;
        let (nanos, after_fraction) = read_fraction(text, i, start)?;
        fraction_nanos = nanos as i128;
        i = after_fraction;
    }
    if i != text.len() {
        return Err(Fault::malformed(start + i, char_len(text[i])));
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
    let (nanos, after_fraction) = read_fraction(body, after, start)?;
    if after_fraction != body.len() {
        return Err(Fault::malformed(start + after_fraction, char_len(body[after_fraction])));
    }
    let total = seconds * NANOS_PER_SECOND + nanos as i128;
    Ok(if negative { -total } else { total })
}
