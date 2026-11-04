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

use crate::DateOptions;
use crate::date_options::{Follow, MonthNames, ValidDateOptions, word_len};
use crate::detail::Failure;
use crate::integer::char_len_at;
use crate::site::Site;
use crate::text::{skip_white_space, white_space_len, white_space_len_back};
use crate::typed::whole;
use crate::verdict::{CivilDateTime, Date, Duration, Fault, Reason, Symbol, Timestamp, trim};
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

impl UnixPrecision {
    /// The precision whose ABI discriminant is `code` (`1` seconds … `4` nanoseconds), or
    /// `None` for any other value — the one decoding every binding and every crate that
    /// carries a declared precision across its own C ABI shares.
    pub const fn from_code(code: u32) -> Option<UnixPrecision> {
        Some(match code {
            1 => UnixPrecision::Seconds,
            2 => UnixPrecision::Millis,
            3 => UnixPrecision::Micros,
            4 => UnixPrecision::Nanos,
            _ => return None,
        })
    }
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

impl ExcelEpoch {
    /// The date system whose ABI discriminant is `code` (`1` the 1900 system, `2` the 1904
    /// system), or `None` for any other value.
    pub const fn from_code(code: u32) -> Option<ExcelEpoch> {
        Some(match code {
            1 => ExcelEpoch::Y1900,
            2 => ExcelEpoch::Y1904,
            _ => return None,
        })
    }
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

#[inline]
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
#[inline]
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
#[inline]
fn read4(text: &[u8], at: usize) -> Option<u32> {
    Some(read2(text, at)? * 100 + read2(text, at + 2)?)
}

/// Reads `.f+` at `at` when present, returning the value widened to nanoseconds and the
/// index after the fraction. ISO 8601, RFC 3339 and XSD put no cap on fraction digits, but
/// protobuf's `nanos` holds nine: digits past the ninth are consumed and truncated, never
/// rounded — rounding could carry into the second, and from `9999-12-31T23:59:59` out of
/// the window. Excel's strict writer emits seventeen (`15:04:05.00000000000312325`). The
/// RFC 3339/ISO-time doors: dot only.
#[inline]
fn read_fraction(text: &[u8], at: usize, start: usize) -> Result<(u32, usize), Failure> {
    read_fraction_marked(text, at, start, b".")
}

/// [`read_fraction`] with the accepted decimal marks parameterized: the duration door
/// takes ISO 8601's comma alongside the dot (`PT1,5S`, `0:00:01,5`, `1,5s` — eurozone
/// feeds really send these), unambiguously — durations have no digit grouping, so a
/// comma there can only be a decimal mark. RFC 3339 timestamps stay dot-only per spec.
#[inline(always)]
fn read_fraction_marked(
    text: &[u8],
    at: usize,
    start: usize,
    marks: &[u8],
) -> Result<(u32, usize), Failure> {
    match text.get(at) {
        Some(byte) if marks.contains(byte) => {}
        _ => return Ok((0, at)),
    }
    let mut i = at + 1;
    let mut nanos: u32 = 0;
    let mut digits = 0;
    // Eight digits at a time when the text has them — milli-, micro- and nanosecond
    // fractions are the common case — then one at a time for the rest. Eight digits are
    // below the nine kept, so the cap below is untouched.
    if let Some(chunk) = text.get(i..).and_then(|rest| rest.first_chunk::<8>()) {
        let chunk = u64::from_le_bytes(*chunk);
        if crate::float::is_8digits(chunk) {
            nanos = crate::float::parse_8digits(chunk) as u32;
            digits = 8;
            i += 8;
        }
    }
    while i < text.len() && text[i].is_ascii_digit() {
        if digits < 9 {
            nanos = nanos * 10 + (text[i] - b'0') as u32;
            digits += 1;
        }
        i += 1;
    }
    if digits == 0 {
        return Err(Failure::at(Site::FractionDigitExpected, start + at, 1));
    }
    // Widen to nine digits with one multiplication. (A loop of `*= 10` here was
    // vectorized into a SIMD power computation slower than the whole timestamp.)
    let scale = POWERS_OF_TEN.get(9 - digits).copied().unwrap_or(1) as u32;
    Ok((nanos * scale, i))
}

/// The failure `site` names at `at`, clamped to the trimmed text: a span that would start
/// or run past the end — the "input ended too soon" faults — becomes a zero-length span at
/// the truncation point, so `offset + len <= input.len()` holds for every fault a binding
/// might slice with (the invariant the fuzz target pins; found by it, not assumed).
fn malformed_at(site: Site, text: &[u8], start: usize, at: usize, len: usize) -> Failure {
    if at >= text.len() {
        Failure::at(site, start + text.len(), 0)
    } else {
        Failure::at(site, start + at, len.min(text.len() - at))
    }
}

/// Where the fields of a strict `yyyy-MM-dd` date sit: offset and length of the year, the
/// month and the day, relative to the date's first byte.
const ISO_DATE_SPANS: [(usize, usize); 3] = [(0, 4), (5, 2), (8, 2)];

/// The same for a compact `yyyyMMdd` date.
const COMPACT_DATE_SPANS: [(usize, usize); 3] = [(0, 4), (4, 2), (6, 2)];

/// Reads the strict date prefix `yyyy-MM-dd` at the head of `text` for its shape only —
/// four, two and two digits, hyphens between — faulting `Malformed` into the caller's
/// coordinates at the first piece that is wrong. The values are not checked here:
/// [`check_date`] does that once the caller has read the rest of its input, so a shape
/// fault anywhere in the text is reported ahead of an impossible value.
#[inline]
fn read_date(text: &[u8], start: usize) -> Result<(u32, u32, u32), Failure> {
    let year = read4(text, 0).ok_or_else(|| {
        let site = if text.len() < 4 { Site::DateYearShort } else { Site::DateYearDigits };
        Failure::at(site, start, text.len().min(4))
    })?;
    if text.get(4) != Some(&b'-') {
        return Err(Failure::at(Site::DateFirstHyphen, start + 4, 1));
    }
    let month = read2(text, 5).ok_or_else(|| Failure::at(Site::DateMonthDigits, start + 5, 2))?;
    if text.get(7) != Some(&b'-') {
        return Err(Failure::at(Site::DateSecondHyphen, start + 7, 1));
    }
    let day = read2(text, 8).ok_or_else(|| Failure::at(Site::DateDayDigits, start + 8, 2))?;
    Ok((year, month, day))
}

/// Checks that a well-formed date names a real day. `spans` are the year, month and day
/// fields' offsets and lengths within the trimmed text. Year 0000, a month outside 1–12 or
/// a day the month does not have ⇒ `OutOfRange` at that field's digits: the text is shaped
/// like a date, and names one that does not exist.
#[inline]
fn check_date(
    year: u32,
    month: u32,
    day: u32,
    spans: [(usize, usize); 3],
    start: usize,
) -> Result<Date, Failure> {
    let fault = |site: Site, (at, len): (usize, usize)| Failure::at(site, start + at, len);
    if year == 0 {
        return Err(fault(Site::DateYearRange, spans[0]));
    }
    if !(1..=12).contains(&month) {
        return Err(fault(Site::DateMonthRange, spans[1]));
    }
    if day == 0 || day > days_in_month(i64::from(year), month) {
        return Err(fault(Site::DateDayRange, spans[2]));
    }
    Ok(Date { year: year as u16, month: month as u8, day: day as u8 })
}

/// Casts a strict ISO 8601 `yyyy-MM-dd` calendar date. Empty ⇒ `Empty`; anything
/// time-bearing or non-ISO ⇒ `Malformed`; year 0000, month 00 or 13+, or a day the month
/// does not have (`2026-02-29`) ⇒ `OutOfRange` at that field. The compact `yyyyMMdd` is
/// read only by the [`detailed`](crate::detailed) door, under
/// [`COMPACT_DATE`](DateOptions::COMPACT_DATE).
pub fn cast_date(input: impl AsRef<[u8]>) -> Result<Date, Fault> {
    date(input.as_ref(), None).map_err(Failure::fault)
}

/// The ISO date door's one parse, for both entry points.
#[inline]
pub(crate) fn date(input: &[u8], options: Option<&ValidDateOptions<'_>>) -> Result<Date, Failure> {
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Failure::spanless(Site::DateBlank));
    }
    // Eight ASCII digits are the compact form; any other eight bytes are judged as ever.
    if options.is_some_and(|o| o.has(DateOptions::COMPACT_DATE))
        && let Some(year) = read4(text, 0).filter(|_| text.len() == 8)
        && let (Some(month), Some(day)) = (read2(text, 4), read2(text, 6))
    {
        return check_date(year, month, day, COMPACT_DATE_SPANS, start);
    }
    if text.len() != 10 {
        return Err(Failure::at(Site::DateWrongLength, start, text.len()));
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

impl DateOrder {
    /// The order whose ABI discriminant is `code` (`1` year-month-day, `2` month-day-year,
    /// `3` day-month-year), or `None` for any other value.
    pub const fn from_code(code: u32) -> Option<DateOrder> {
        Some(match code {
            1 => DateOrder::YearMonthDay,
            2 => DateOrder::MonthDayYear,
            3 => DateOrder::DayMonthYear,
            _ => return None,
        })
    }
}

/// Caller-declared day-period designators for [`cast_datetime_with`]: the words a culture
/// writes for the two halves of a 12-hour clock (`오전`/`오후`, `vorm.`/`nachm.`, `a. m.`/
/// `p. m.`), read beside the English `AM`/`PM`, which are always read. Declared, never
/// looked up, like every other piece of culture here; a binding's bridge fills them from
/// its platform's data. Up to 16 UTF-8 bytes each, with no ASCII digit and no white
/// space at either end (the grammar around a designator reads that); inside stays allowed.
///
/// ABI layout: the two designators in place, 34 bytes with no alignment, each its 16 bytes
/// then its length; [`RawDayPeriods`](crate::RawDayPeriods) puts a size word ahead of them.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DayPeriods {
    /// The designator of the first half of the day; [`Symbol::NONE`] declares none beside `AM`.
    pub am: Symbol<16>,
    /// The designator of the second half; [`Symbol::NONE`] declares none beside `PM`.
    pub pm: Symbol<16>,
}

impl DayPeriods {
    /// Nothing declared: the English `AM`/`PM` only, which is what [`cast_datetime`] reads.
    pub const ENGLISH: DayPeriods = DayPeriods { am: Symbol::NONE, pm: Symbol::NONE };

    /// Declares both designators.
    pub const fn new(am: Symbol<16>, pm: Symbol<16>) -> DayPeriods {
        DayPeriods { am, pm }
    }

    /// Whether designators read across the ABI are ones this API would declare: each
    /// empty or a [`Symbol`] with no white space at either end.
    #[inline]
    pub fn check(&self) -> bool {
        let edges = |symbol: &Symbol<16>| {
            symbol.check_optional(false)
                && white_space_len(symbol.as_bytes()) == 0
                && white_space_len_back(symbol.as_bytes()) == 0
        };
        edges(&self.am) && edges(&self.pm)
    }

    /// The designator at `text[at..]`, as `(pm, byte length)`: a declared one first, exact,
    /// then the English pair, ASCII case-insensitive. The declared ones go first because a
    /// culture's designator can begin with the English one (`PM.`).
    #[inline]
    fn read(&self, text: &[u8], at: usize) -> Option<(bool, usize)> {
        if self.pm.matches(text, at) {
            return Some((true, self.pm.len()));
        }
        if self.am.matches(text, at) {
            return Some((false, self.am.len()));
        }
        match (
            text.get(at).map(u8::to_ascii_lowercase),
            text.get(at + 1).map(u8::to_ascii_lowercase),
        ) {
            (Some(b'a'), Some(b'm')) => Some((false, 2)),
            (Some(b'p'), Some(b'm')) => Some((true, 2)),
            _ => None,
        }
    }
}

/// Reads a run of 1..=4 ASCII digits at `at` (a calendar date field), returning the value
/// and the index after the run. A zero-length or five-plus-digit run faults at the run
/// itself. Thin wrapper over the duration parser's [`read_digit_run`].
#[inline(always)]
fn read_date_field(text: &[u8], at: usize, start: usize) -> Result<(u32, usize), Failure> {
    let (value, digits, after) = read_digit_run(text, at, start)?;
    if digits == 0 {
        return Err(malformed_at(Site::OrderedFieldDigitExpected, text, start, at, 1));
    }
    if digits > 4 {
        return Err(malformed_at(Site::OrderedFieldWidth, text, start, at, digits));
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
    #[inline]
    fn check(&self, start: usize) -> Result<Date, Failure> {
        let [year, month, day] = self.fields;
        check_date(year, month, day, self.spans, start)
    }
}

/// The failure for a month field that starts with something other than a digit and names no
/// eligible month: no word at all is today's missing digit, a word that is exactly a declared
/// name (so what is wrong is the mark after it) is a missing or different separator, and any
/// other word is not a month in the vocabulary.
fn unmatched_month(months: &MonthNames<'_>, text: &[u8], start: usize, at: usize) -> Failure {
    let word = word_len(text, at);
    if word == 0 {
        return malformed_at(Site::OrderedFieldDigitExpected, text, start, at, 1);
    }
    let after = at + word;
    if text.get(at..after).is_some_and(|bytes| months.names(bytes)) {
        return malformed_at(
            Site::OrderedMonthDelimiter,
            text,
            start,
            after,
            char_len_at(text, after),
        );
    }
    Failure::at(Site::OrderedMonthNotRecognized, start + at, word)
}

/// Reads a declared month name at `at`, returning its month and the index after it.
fn read_month_name(
    months: &MonthNames<'_>,
    text: &[u8],
    start: usize,
    at: usize,
    follow: Follow,
) -> Result<(u32, usize), Failure> {
    match months.read(text, at, follow) {
        Some((month, len)) => Ok((month, at + len)),
        None => Err(unmatched_month(months, text, start, at)),
    }
}

/// The year a two-digit `yy` names under `pivot`: the one year in `[pivot−99, pivot]` that
/// ends in `yy`. A pivot validated as 100–9999 keeps the year within 1–9999.
fn pivot_year(yy: u32, pivot: u16) -> u32 {
    let pivot = u32::from(pivot);
    let yy = yy % 100;
    pivot - (pivot % 100 + 100 - yy) % 100
}

/// Parses a separated calendar date at the head of `text` under the declared order, for
/// its shape, returning it and the index after it. Shared by [`cast_date_ordered`] (which
/// then demands end-of-input) and [`cast_datetime`] (which continues into the time part);
/// each range-checks it once the rest of the input has been read.
///
/// Under `options`, the month field may be a declared name (at the position the order gives
/// the month, never elsewhere) and a two-digit year is read through the pivot.
#[inline(always)]
fn read_ordered_date(
    text: &[u8],
    start: usize,
    order: DateOrder,
    options: Option<&ValidDateOptions<'_>>,
) -> Result<(OrderedDate, usize), Failure> {
    let pivot = options.map_or(0, ValidDateOptions::pivot);
    let months = options.and_then(ValidDateOptions::months);

    // A name stands first only where the order puts the month first, and where a digit
    // doesn't: names hold none.
    let first_digit = text.first().is_some_and(u8::is_ascii_digit);
    let first_name = months.filter(|_| order == DateOrder::MonthDayYear && !first_digit);
    let (first, first_end) = match first_name {
        Some(months) => read_month_name(months, text, start, 0, Follow::Any)?,
        None => read_date_field(text, 0, start)?,
    };
    let sep = match text.get(first_end) {
        Some(&sep @ (b'/' | b'-' | b'.')) => sep,
        _ => return Err(malformed_at(Site::OrderedFirstSeparator, text, start, first_end, 1)),
    };

    // A four-digit FIRST field can only be a year (month and day never exceed two digits),
    // so a year-first date is structurally unambiguous under any declared order —
    // "2026/1/7" and ISO "2026-01-07" read year-month-day even when the declaration says
    // month- or day-first. This is width detection, not value sniffing: the genuinely
    // ambiguous forms ("1/7/2026" under a wrong declaration) still parse exactly as
    // declared, because no structure distinguishes them.
    let effective =
        if first_name.is_none() && first_end == 4 { DateOrder::YearMonthDay } else { order };
    let (year_at, month_at, day_at) = match effective {
        DateOrder::YearMonthDay => (0, 1, 2),
        DateOrder::MonthDayYear => (2, 0, 1),
        DateOrder::DayMonthYear => (2, 1, 0),
    };

    let second_at = first_end + 1;
    let second_name = months.filter(|_| {
        month_at == 1 && text.get(second_at).is_some_and(|byte| !byte.is_ascii_digit())
    });
    let (second, second_end) = match second_name {
        Some(months) => read_month_name(months, text, start, second_at, Follow::Separator(sep))?,
        None => read_date_field(text, second_at, start)?,
    };
    if text.get(second_end) != Some(&sep) {
        return Err(malformed_at(Site::OrderedSecondSeparator, text, start, second_end, 1));
    }
    let (third, third_end) = read_date_field(text, second_end + 1, start)?;
    let named = first_name.is_some() || second_name.is_some();

    // Field spans, for pointing a fault at the offending digits (or the name).
    let spans = [
        (0, first_end),
        (first_end + 1, second_end - first_end - 1),
        (second_end + 1, third_end - second_end - 1),
    ];
    let fields = [first, second, third];
    let field_fault = |site: Site, at: usize| Failure::at(site, start + spans[at].0, spans[at].1);

    // The year field is four digits wherever the order puts it, or two under a pivot; month
    // and day are one or two (a three-digit field faults at its own digits), a month name
    // any length. A two-digit year without a pivot would mean century guessing, which this
    // core never does.
    if !named && spans[month_at].1 > 2 {
        return Err(field_fault(Site::OrderedMonthWidth, month_at));
    }
    if spans[day_at].1 > 2 {
        return Err(field_fault(Site::OrderedDayWidth, day_at));
    }
    let mut year = fields[year_at];
    if spans[year_at].1 != 4 {
        if pivot == 0 || spans[year_at].1 != 2 {
            return Err(field_fault(Site::OrderedYearWidth, year_at));
        }
        year = pivot_year(year, pivot);
    }
    let date = OrderedDate {
        fields: [year, fields[month_at], fields[day_at]],
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
    date_ordered(input.as_ref(), order, None).map_err(Failure::fault)
}

/// The separated date door's one parse, for both entry points.
#[inline(always)]
pub(crate) fn date_ordered(
    input: &[u8],
    order: DateOrder,
    options: Option<&ValidDateOptions<'_>>,
) -> Result<Date, Failure> {
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Failure::spanless(Site::OrderedBlank));
    }
    let (date, end) = read_ordered_date(text, start, order, options)?;
    if end != text.len() {
        return Err(Failure::at(Site::OrderedTrailing, start + end, char_len_at(text, end)));
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
    #[inline]
    fn check(&self, hours: RangeInclusive<u32>, start: usize) -> Result<(), Failure> {
        let fault = |site: Site, (at, len): (usize, usize)| Failure::at(site, start + at, len);
        if !hours.contains(&self.hour) {
            return Err(fault(Site::ClockHourRange, self.spans[0]));
        }
        if self.minute > 59 {
            return Err(fault(Site::ClockMinuteRange, self.spans[1]));
        }
        if self.second > 59 {
            return Err(fault(Site::ClockSecondRange, self.spans[2]));
        }
        Ok(())
    }

    /// Nanoseconds since midnight, with the (checked) hour given on the 24-hour clock.
    #[inline]
    fn nanos_of_day(&self, hour: u32) -> u64 {
        let total = u64::from(hour) * 3_600 + u64::from(self.minute) * 60 + u64::from(self.second);
        total * 1_000_000_000 + u64::from(self.nanos)
    }
}

/// Reads the civil time part of [`cast_datetime`] at `at` for its shape:
/// `h[:mm[:ss[.f+]]]`, hour one or two digits, with an optional day-period marker — the
/// English `AM`/`PM`, case-insensitive, or a declared designator — after the clock
/// (preceding white space optional, any run of it) or, as Korean and Chinese write it, ahead
/// of it (following white space optional). Without a marker minutes are mandatory (a bare
/// trailing number is not a time). Returns the clock, the marker (`Some(true)` for PM) and the index after the
/// time; [`civil_nanos`] range-checks them.
#[inline(always)]
fn read_civil_time(
    text: &[u8],
    at: usize,
    start: usize,
    periods: &DayPeriods,
) -> Result<(Clock, Option<bool>, usize), Failure> {
    // A leading marker names the half of the day before the clock is read.
    let (leading, at) = match periods.read(text, at) {
        Some((pm, len)) => {
            let after = at + len;
            (Some(pm), after + skip_white_space(text.get(after..).unwrap_or(&[])))
        }
        None => (None, at),
    };
    let (hour, hour_digits, mut i) = read_digit_run(text, at, start)?;
    if hour_digits == 0 {
        return Err(malformed_at(Site::CivilHourDigitExpected, text, start, at, 1));
    }
    if hour_digits > 2 {
        return Err(malformed_at(Site::CivilHourWidth, text, start, at, hour_digits));
    }
    let mut clock =
        Clock { hour: hour as u32, minute: 0, second: 0, nanos: 0, spans: [(at, hour_digits); 3] };
    let mut has_minutes = false;
    if text.get(i) == Some(&b':') {
        clock.minute = read2(text, i + 1)
            .ok_or_else(|| malformed_at(Site::CivilMinuteDigits, text, start, i + 1, 2))?;
        clock.spans[1] = (i + 1, 2);
        has_minutes = true;
        i += 3;
        if text.get(i) == Some(&b':') {
            clock.second = read2(text, i + 1)
                .ok_or_else(|| malformed_at(Site::CivilSecondDigits, text, start, i + 1, 2))?;
            clock.spans[2] = (i + 1, 2);
            i += 3;
            let (fraction, after) = read_fraction(text, i, start)?;
            clock.nanos = fraction;
            i = after;
        }
    }
    if leading.is_some() {
        // One marker names the half of the day; a second one after the clock is left for
        // the caller to fault as trailing text.
        return Ok((clock, leading, i));
    }
    // Optional trailing meridiem: [white space] AM/PM or a declared designator.
    let meridiem_at = i + skip_white_space(text.get(i..).unwrap_or(&[]));
    let marker = periods.read(text, meridiem_at);
    match marker {
        Some((_, len)) => i = meridiem_at + len,
        // A bare trailing number is only a time when a meridiem names it one.
        None if !has_minutes => {
            return Err(Failure::at(Site::CivilMinutesMissing, start + at, hour_digits));
        }
        None => {}
    }
    Ok((clock, marker.map(|(pm, _)| pm), i))
}

/// Range-checks a civil time and converts it to nanoseconds since midnight. With a marker
/// the hour is `1..=12` (`12 AM` is midnight, `12 PM` noon); without one it is `0..=23`.
#[inline]
fn civil_nanos(clock: &Clock, pm: Option<bool>, start: usize) -> Result<u64, Failure> {
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
/// `h:mm[:ss[.f+]]` or 12-hour with an `AM`/`PM` marker (`3 PM` allowed, `12 AM` is
/// midnight); absent, the time is midnight. No zone is read and none is invented — a
/// zone-less text names no instant, so fusing a zone is the caller's job
/// ([`cast_timestamp`] remains the strict RFC 3339 instant door).
pub fn cast_datetime(input: impl AsRef<[u8]>, order: DateOrder) -> Result<CivilDateTime, Fault> {
    cast_datetime_with(input, order, &DayPeriods::ENGLISH)
}

/// [`cast_datetime`] with the declared [`DayPeriods`] read beside `AM`/`PM`, before or
/// after the clock (`1/7/2026 3:04 nachm.`, `2026. 1. 7. 오후 3:04`).
pub fn cast_datetime_with(
    input: impl AsRef<[u8]>,
    order: DateOrder,
    periods: &DayPeriods,
) -> Result<CivilDateTime, Fault> {
    datetime(input.as_ref(), order, periods, None).map_err(Failure::fault)
}

/// The civil date-time door's one parse, for both entry points.
#[inline(always)]
pub(crate) fn datetime(
    input: &[u8],
    order: DateOrder,
    periods: &DayPeriods,
    options: Option<&ValidDateOptions<'_>>,
) -> Result<CivilDateTime, Failure> {
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Failure::spanless(Site::DatetimeBlank));
    }
    let (date, date_end) = read_ordered_date(text, start, order, options)?;
    match text.get(date_end) {
        None => return Ok(CivilDateTime { date: date.check(start)?, nanos_of_day: 0 }),
        Some(b' ' | b'T' | b't') => {}
        Some(_) => {
            let len = char_len_at(text, date_end);
            return Err(Failure::at(Site::DatetimeSeparator, start + date_end, len));
        }
    }
    let (clock, pm, end) = read_civil_time(text, date_end + 1, start, periods)?;
    if end != text.len() {
        return Err(Failure::at(Site::DatetimeTrailing, start + end, char_len_at(text, end)));
    }
    let date = date.check(start)?;
    Ok(CivilDateTime { date, nanos_of_day: civil_nanos(&clock, pm, start)? })
}

/// Casts an ISO 8601 24-hour time-of-day — `HH:mm`, `HH:mm:ss`, or `HH:mm:ss.f+` —
/// to nanoseconds since midnight, `00:00` through `23:59:59.999999999`. Empty ⇒ `Empty`;
/// a well-formed hour past 23, minute past 59 or second past 59 (`24:00`, `15:04:60`) ⇒
/// `OutOfRange` at that field; anything else wrong ⇒ `Malformed`.
pub fn cast_time(input: impl AsRef<[u8]>) -> Result<u64, Fault> {
    time(input.as_ref()).map_err(Failure::fault)
}

/// The time-of-day door's one parse, for both entry points.
#[inline]
pub(crate) fn time(input: &[u8]) -> Result<u64, Failure> {
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Failure::spanless(Site::TimeBlank));
    }
    let (clock, end) = read_time(text, 0, start)?;
    if end != text.len() {
        return Err(Failure::at(Site::TimeTrailing, start + end, char_len_at(text, end)));
    }
    clock.check(0..=23, start)?;
    Ok(clock.nanos_of_day(clock.hour))
}

/// Parses `HH:mm[:ss[.f+]]` at `at` for its shape, returning the clock (not yet
/// range-checked) and the index after the time.
#[inline]
fn read_time(text: &[u8], at: usize, start: usize) -> Result<(Clock, usize), Failure> {
    let hour = read2(text, at).ok_or_else(|| {
        Failure::at(Site::TimeHourDigits, start + at, (text.len() - at).clamp(1, 2))
    })?;
    if text.get(at + 2) != Some(&b':') {
        return Err(malformed_at(Site::TimeColon, text, start, at + 2, 1));
    }
    let minute = read2(text, at + 3)
        .ok_or_else(|| malformed_at(Site::TimeMinuteDigits, text, start, at + 3, 2))?;
    let mut clock =
        Clock { hour, minute, second: 0, nanos: 0, spans: [(at, 2), (at + 3, 2), (at + 3, 2)] };
    let mut i = at + 5;
    if text.get(i) == Some(&b':') {
        clock.second = read2(text, i + 1)
            .ok_or_else(|| malformed_at(Site::TimeSecondDigits, text, start, i + 1, 2))?;
        clock.spans[2] = (i + 1, 2);
        i += 3;
        let (fraction, after) = read_fraction(text, i, start)?;
        clock.nanos = fraction;
        i = after;
    }
    Ok((clock, i))
}

/// Casts an RFC 3339 instant — `yyyy-MM-ddTHH:mm:ss[.f+](Z|±hh:mm)` — to a protobuf
/// [`Timestamp`], normalized to UTC. The zone is mandatory (a zone-less or space-separated
/// form is `Malformed`, Svartalfheim parity); `-00:00` is accepted as UTC; `T`/`Z` are
/// case-insensitive; seconds are mandatory. Every piece is checked for shape before any
/// value is: a well-formed field that names nothing — year 0000, month 13, `02-30`, hour
/// 24, a `:60` leap second, an offset of `+24:00` — ⇒ `OutOfRange` at that field, and an
/// instant whose fields are all real but which falls outside the window (an offset pushing
/// past an edge) ⇒ `OutOfRange` spanning the token.
pub fn cast_timestamp(input: impl AsRef<[u8]>) -> Result<Timestamp, Fault> {
    timestamp(input.as_ref(), None).map_err(Failure::fault)
}

/// The RFC 3339 door's one parse, for both entry points.
#[inline]
pub(crate) fn timestamp(
    input: &[u8],
    options: Option<&ValidDateOptions<'_>>,
) -> Result<Timestamp, Failure> {
    let has = |flag: u32| options.is_some_and(|o| o.has(flag));
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Failure::spanless(Site::TimestampBlank));
    }
    if text.len() < 11 {
        return Err(Failure::at(Site::TimestampTooShort, start, text.len()));
    }
    let (year, month, day) = read_date(&text[..10], start)?;
    let separator = text[10];
    if !(matches!(separator, b'T' | b't')
        || (separator == b' ' && has(DateOptions::SPACE_SEPARATOR)))
    {
        return Err(Failure::at(Site::TimestampT, start + 10, char_len_at(text, 10)));
    }

    // The time segment reuses read_time's grammar but with seconds mandatory.
    if text.get(13) != Some(&b':') || text.get(16) != Some(&b':') {
        return Err(Failure::at(time_shape(text), start + 11, text.len() - 11));
    }
    let (clock, after_time) = read_time(text, 11, start)?;

    // The zone, for shape: `None` is UTC, `Some` the sign, hours and minutes of an offset.
    let offset = match text.get(after_time) {
        None => {
            // Zone-less — an ambiguous instant, rejected whole.
            return Err(Failure::at(Site::TimestampZoneMissing, start, text.len()));
        }
        Some(&(b'Z' | b'z')) => {
            if after_time + 1 != text.len() {
                return Err(Failure::at(
                    Site::TimestampTrailingAfterZ,
                    start + after_time + 1,
                    char_len_at(text, after_time + 1),
                ));
            }
            None
        }
        Some(&sign @ (b'+' | b'-')) => {
            let hours = read2(text, after_time + 1).ok_or_else(|| {
                malformed_at(Site::TimestampOffsetHourDigits, text, start, after_time + 1, 2)
            })?;
            // `±hh` alone ends the text; anything after the hours is judged as `±hh:mm`.
            if has(DateOptions::HOUR_OFFSET) && after_time + 3 == text.len() {
                Some((sign, hours, 0))
            } else {
                if text.get(after_time + 3) != Some(&b':') {
                    return Err(malformed_at(
                        Site::TimestampOffsetColon,
                        text,
                        start,
                        after_time + 3,
                        1,
                    ));
                }
                let minutes = read2(text, after_time + 4).ok_or_else(|| {
                    malformed_at(Site::TimestampOffsetMinuteDigits, text, start, after_time + 4, 2)
                })?;
                if after_time + 6 != text.len() {
                    return Err(Failure::at(
                        Site::TimestampTrailingAfterOffset,
                        start + after_time + 6,
                        char_len_at(text, after_time + 6),
                    ));
                }
                Some((sign, hours, minutes))
            }
        }
        Some(_) => {
            let len = char_len_at(text, after_time);
            return Err(Failure::at(Site::TimestampZoneUnexpected, start + after_time, len));
        }
    };

    // Every piece is well-formed; now each value, in reading order.
    let date = check_date(year, month, day, ISO_DATE_SPANS, start)?;
    clock.check(0..=23, start)?;
    let offset_seconds = match offset {
        None => 0i64,
        Some((sign, hours, minutes)) => {
            if hours > 23 {
                return Err(Failure::at(Site::TimestampOffsetHourRange, start + after_time + 1, 2));
            }
            if minutes > 59 {
                let at = start + after_time + 4;
                return Err(Failure::at(Site::TimestampOffsetMinuteRange, at, 2));
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
        return Err(Failure::at(Site::TimestampOutsideWindow, start, text.len()));
    }
    Ok(Timestamp { seconds, nanos })
}

/// What is wrong with a timestamp's time when its `:` at 13 or 16 is not there: the first
/// of `HH:mm:` (at 11 through 16) that is not what it must be. A digit's place is
/// `digit_expected` and a `:`'s `delimiter_expected`, except that `HH:mm` followed by the end
/// or a zone is missing its seconds.
fn time_shape(text: &[u8]) -> Site {
    for (at, colon) in [(11, false), (12, false), (13, true), (14, false), (15, false)] {
        let found = text.get(at);
        let fits = if colon { found == Some(&b':') } else { found.is_some_and(u8::is_ascii_digit) };
        if !fits {
            return if colon { Site::TimestampTimeColon } else { Site::TimestampTimeDigit };
        }
    }
    match text.get(16) {
        None | Some(b'Z' | b'z' | b'+' | b'-') => Site::TimestampSecondsMissing,
        Some(_) => Site::TimestampTimeColon,
    }
}

/// Casts an integer Unix-epoch value under a caller-declared unit to a protobuf
/// [`Timestamp`]. Negatives (pre-1970) are allowed; a fractional or non-integer value ⇒
/// `Malformed`; outside the window ⇒ `OutOfRange`. Sub-second units land in `nanos`, which
/// stays non-negative even before the epoch (seconds floor toward -∞, protobuf convention).
pub fn cast_unix(input: impl AsRef<[u8]>, precision: UnixPrecision) -> Result<Timestamp, Fault> {
    unix(input.as_ref(), precision).map_err(Failure::fault)
}

/// The Unix door's one parse, for both entry points.
#[inline]
pub(crate) fn unix(input: &[u8], precision: UnixPrecision) -> Result<Timestamp, Failure> {
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Failure::spanless(Site::UnixBlank));
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
        return Err(Failure::at(Site::UnixNoDigits, start, text.len()));
    }
    let mut magnitude: i128 = 0;
    let mut over = false;
    while i < text.len() {
        let byte = text[i];
        if !byte.is_ascii_digit() {
            let site = match byte {
                b'.' => Site::UnixFraction,
                b'+' | b'-' => Site::UnixSignMisplaced,
                _ => Site::UnixUnexpected,
            };
            return Err(Failure::at(site, start + i, char_len_at(text, i)));
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
        return Err(Failure::at(Site::UnixOverflow, start, text.len()));
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
        return Err(Failure::at(Site::UnixOutsideWindow, start, text.len()));
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

/// Which of `[phantom, outside]` a whole serial [`excel_day_number`] refused is: the 1900
/// system's phantom serial, or a serial outside the system's days.
fn refused_day(days: i64, epoch: ExcelEpoch, [phantom, outside]: [Site; 2]) -> Site {
    if epoch == ExcelEpoch::Y1900 && days == EXCEL_1900_PHANTOM_SERIAL { phantom } else { outside }
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
/// is snapped, where the text door truncates digits it was given exactly: a double near
/// serial 45,000 resolves about 0.6 µs and one near `9999-12-31` about 40 µs, so the
/// nearest nanosecond is noise (Excel's own `9999-12-31 23:59:59` reads 5,424 ns late).
/// The time read is the one with the fewest fractional-second digits that the writer's
/// conversion would have stored as this same double — see `snap`. A fraction that
/// snaps to a whole day carries into the date.
///
/// The verdict is a bare [`Reason`]: there is no text for a [`Fault`]'s span to index.
pub fn excel_serial(serial: f64, epoch: ExcelEpoch) -> Result<CivilDateTime, Reason> {
    serial_typed(serial, epoch).map_err(|failure| failure.reason)
}

/// The typed Excel serial door's one parse, for both entry points.
pub(crate) fn serial_typed(serial: f64, epoch: ExcelEpoch) -> Result<CivilDateTime, Failure> {
    let (days, nanos_of_day) = split_serial(serial)?;
    let day_number = excel_day_number(days, epoch).ok_or_else(|| {
        let site = refused_day(days, epoch, [Site::SerialPhantom, Site::SerialOutsideWindow]);
        Failure::spanless(site)
    })?;
    let (year, month, day) = civil_from_days(day_number);
    Ok(CivilDateTime {
        date: Date { year: year as u16, month: month as u8, day: day as u8 },
        nanos_of_day,
    })
}

/// Reads the time of day of an Excel serial already held as the number a workbook stores:
/// its fraction, nanoseconds since midnight as [`cast_time`] returns them — the typed twin
/// of that door for a cell formatted as a time, whether or not the serial also carries a
/// date (`0.75` and `45292.75` are both `18:00`). Snapped as [`excel_serial`] snaps, and a
/// fraction that snaps to a whole day is midnight. A
/// negative, NaN or infinite value is `Malformed`; a serial past either system's last day
/// is `OutOfRange`. No date system is asked for: the fraction means the same in both.
pub fn excel_time(serial: f64) -> Result<u64, Reason> {
    time_typed(serial).map_err(|failure| failure.reason)
}

/// The typed time-of-day door's one parse, for both entry points.
pub(crate) fn time_typed(serial: f64) -> Result<u64, Failure> {
    split_serial(serial).map(|(_, nanos_of_day)| nanos_of_day)
}

/// Splits a non-negative serial into whole days and the nanoseconds of its fractional day,
/// snapped by [`snap`] and carried into the day if that lands on a whole one — the one
/// statement of how a serial's fraction is read.
fn split_serial(serial: f64) -> Result<(i64, u64), Failure> {
    if !serial.is_finite() {
        return Err(Failure::spanless(Site::SerialNotFinite));
    }
    if serial < 0.0 {
        return Err(Failure::spanless(Site::SerialNegative));
    }
    // Past either system's last day; also what keeps the serial below snap's 2^22.
    if serial >= (MAX_EXCEL_1900_SERIAL + 1) as f64 {
        return Err(Failure::spanless(Site::SerialPastLastDay));
    }
    let (days, nanos_of_day) = snap(serial);
    Ok((days as i64, nanos_of_day))
}

/// `NANOS_PER_DAY` is `2^16 × DAY_ODD_FACTOR`: dividing a nanosecond count below 2^71 by
/// it is a shift and then a `u64` division, with no 128-bit division routine brought along.
const DAY_ODD_FACTOR: u64 = NANOS_PER_DAY >> 16;

/// `nanos` rounded down to a multiple of `10^digits`, `digits` from 0 to 9: a match, so
/// every arm divides by a constant, which compiles to a multiply.
#[inline]
fn floor_to_power(nanos: u64, digits: u32) -> u64 {
    match digits {
        9 => nanos / 1_000_000_000 * 1_000_000_000,
        8 => nanos / 100_000_000 * 100_000_000,
        7 => nanos / 10_000_000 * 10_000_000,
        6 => nanos / 1_000_000 * 1_000_000,
        5 => nanos / 100_000 * 100_000,
        4 => nanos / 10_000 * 10_000,
        3 => nanos / 1_000 * 1_000,
        2 => nanos / 100 * 100,
        1 => nanos / 10 * 10,
        _ => nanos,
    }
}

const POWERS_OF_TEN: [u64; 10] =
    [1, 10, 100, 1_000, 10_000, 100_000, 1_000_000, 10_000_000, 100_000_000, 1_000_000_000];

/// For [`snap`], once whole seconds have not fit: the largest power of ten below a second
/// with a multiple in `first..=last`, as that unit and its largest multiple there — or
/// `None` if not even a whole nanosecond fits. A power that fits makes every smaller one
/// fit, so the count of powers that fit picks the unit: every power is tried and the answer
/// picked by index, with no branch per power, since which powers fit is as unpredictable
/// as the double.
#[inline(never)]
fn finer_unit(first: i64, last: u64) -> Option<(u64, u64)> {
    if (last as i64) < first {
        return None;
    }
    let mut floors = [last; 9];
    let mut power = 0;
    for exponent in 1..=8 {
        floors[exponent as usize] = floor_to_power(last, exponent);
        power += usize::from(floors[exponent as usize] as i64 >= first);
    }
    // At most eight powers fit; the bound is spelled out so no index check is left behind.
    let power = power.min(8);
    Some((floors[power], POWERS_OF_TEN[power]))
}

/// Reads `days`, finite, non-negative and below 2^22, as whole days and the nanoseconds of
/// the day after them, snapped. A workbook writer stores a time as the double nearest the
/// exact serial, so every real number between this double's midpoints with its neighbours
/// is a time the writer could have meant; all of them are stored as this same double. Of
/// those, this reads the one with the fewest fractional-second digits: whole seconds if one
/// lies between the midpoints, else whole tenths, and so on down to the nanosecond, the
/// candidate nearest the double winning when two qualify. It is [`shortest_digits`]'s rule
/// — the shortest value that round-trips — counted in time instead of decimal places, so
/// Excel's `9999-12-31 23:59:59` (stored 5,424 ns late, all a double holds there) is read
/// back on the second, while a real sub-millisecond time a double can resolve is kept. A
/// double too close to its neighbours for any whole nanosecond to lie between the
/// midpoints is read as the nearest nanosecond, halves up.
///
/// Exact, in integers: the double is `m × 2^q`, and in quarter-ulps its midpoints are
/// `4m ± 2` (`4m − 1` below a power of two, where the gap beneath is half as wide). Scaled
/// to nanoseconds by `NANOS_PER_DAY` (under 2^47), every bound is below 2^102, over a
/// denominator `2^(2 − q)`. A midpoint itself rounds to the even mantissa, so it belongs to
/// this double exactly when `m` is even.
///
/// [`shortest_digits`]: crate::shortest_digits
fn snap(days: f64) -> (u64, u64) {
    let bits = days.to_bits();
    let biased = ((bits >> 52) & 0x7FF) as u32;
    // Below 2^-72 days (and zero, and the subnormals) is under 2^-25 ns: zero. The bound
    // also keeps the shift below under 128.
    if biased < 950 {
        return (0, 0);
    }
    let mantissa = bits & ((1 << 52) - 1) | 1 << 52;
    let shift = 1_077 - biased;
    let per_day = u128::from(NANOS_PER_DAY);
    // 55 bits by 47: a single 64 × 64 → 128 multiply, and the midpoints are a day's
    // nanoseconds (one or two of them) to either side.
    let center = u128::from(mantissa << 2) * per_day;
    let low = center - if mantissa == 1 << 52 { per_day } else { per_day << 1 };
    let high = center + (per_day << 1);
    let inclusive = mantissa & 1 == 0;
    let mask = (1u128 << shift) - 1;

    // Counted from the midnight before the double: the whole nanoseconds between the
    // midpoints are `first..=last` (empty if first > last), `last` at least `within`, the
    // double's own floor, and `first` at most a day early.
    let floor = center >> shift;
    let day = ((floor >> 16) as u64) / DAY_ODD_FACTOR;
    let day_start = u128::from(day) * per_day;
    let within = (floor - day_start) as u64;
    let first =
        ((low >> shift) + u128::from(low & mask != 0 || !inclusive)).wrapping_sub(day_start) as i64;
    let last = ((high >> shift) - u128::from(high & mask == 0 && !inclusive))
        .wrapping_sub(day_start) as u64;

    // The midpoints are at most 80 µs apart (an ulp at 9999-12-31), so a unit of 100 µs or
    // more has at most one multiple between them: the largest one up to `last`, if that is
    // not below `first`. Whole seconds are tried first, inline — what a typed time almost
    // always snaps to — and anything finer out of line, so this path never pays for it.
    let seconds = floor_to_power(last, 9);
    let nanos = if seconds as i64 >= first {
        seconds
    } else {
        match finer_unit(first, last) {
            None => within + u64::from(center & mask >= 1 << (shift - 1)),
            Some((highest, unit)) if (highest as i64) - (unit as i64) < first => highest,
            // Several multiples fit: the one nearest the double is one of the two around
            // its floor, found with the only division by a variable here.
            Some((_, unit)) => {
                // `unit` is a power of ten; the zero case is spelled out so that no
                // division-by-zero check is left behind.
                let below = within - within.checked_rem(unit).unwrap_or(0);
                let above = below + unit;
                match (below as i64 >= first, above <= last) {
                    (true, false) => below,
                    (false, _) => above,
                    // Two fit only when the midpoints are a nanosecond or more apart, so the
                    // shift is under 50 and twice the center compares with their sum
                    // shifted unclipped.
                    (true, true) => {
                        let sum = (day_start << 1) + u128::from(below + above);
                        if sum.leading_zeros() >= shift && center << 1 > sum << shift {
                            above
                        } else {
                            below
                        }
                    }
                }
            }
        }
    };
    if nanos >= NANOS_PER_DAY { (day + 1, nanos - NANOS_PER_DAY) } else { (day, nanos) }
}

/// Reads a span of days already held as the number a workbook stores — what an elapsed-time
/// format (`[h]:mm:ss`) or an ODS `office:time-value` converted to days says — as a
/// protobuf [`Duration`]: the typed twin of [`cast_duration`]. `1.5` is a day and twelve
/// hours, and a negative span is negative, `seconds` and `nanos` same-signed. Its size is
/// snapped as [`excel_serial`] snaps a time, the sign put back after, so a span and its
/// negation read alike. NaN or an infinity is `Malformed`; past ±10,000 years of seconds
/// is `OutOfRange`.
pub fn excel_duration(days: f64) -> Result<Duration, Reason> {
    duration_typed(days).map_err(|failure| failure.reason)
}

/// The typed duration door's one parse, for both entry points.
pub(crate) fn duration_typed(days: f64) -> Result<Duration, Failure> {
    if !days.is_finite() {
        return Err(Failure::spanless(Site::DurationTypedNotFinite));
    }
    let seconds = days * 86_400.0;
    if seconds.abs() > MAX_DURATION_SECONDS as f64 {
        return Err(Failure::spanless(Site::DurationTypedOutsideWindow));
    }
    // ±10,000 years is about 3.65 million days, inside snap's 2^22.
    let (whole, nanos) = snap(days.abs());
    let size = i128::from(whole) * i128::from(NANOS_PER_DAY) + i128::from(nanos);
    if size / NANOS_PER_SECOND > i128::from(MAX_DURATION_SECONDS) {
        return Err(Failure::spanless(Site::DurationTypedSnappedOutsideWindow));
    }
    let total = if days < 0.0 { -size } else { size };
    Ok(Duration {
        seconds: (total / NANOS_PER_SECOND) as i64,
        nanos: (total % NANOS_PER_SECOND) as i32,
    })
}

/// Reads an integer Unix-epoch value already held as an `f64` under a caller-declared unit
/// — the typed twin of [`cast_unix`], under its rules: an integral value only (a fraction,
/// NaN or an infinity is `Malformed`), negatives allowed, `nanos` non-negative even before
/// the epoch, and outside the window `OutOfRange`.
pub fn unix_from_f64(value: f64, precision: UnixPrecision) -> Result<Timestamp, Reason> {
    unix_typed(value, precision).map_err(|failure| failure.reason)
}

/// The typed Unix door's one parse, for both entry points.
pub(crate) fn unix_typed(value: f64, precision: UnixPrecision) -> Result<Timestamp, Failure> {
    let epoch = whole(value)?.ok_or_else(|| Failure::spanless(Site::UnixTypedOverflow))?;
    let per_second: i128 = match precision {
        UnixPrecision::Seconds => 1,
        UnixPrecision::Millis => 1_000,
        UnixPrecision::Micros => 1_000_000,
        UnixPrecision::Nanos => 1_000_000_000,
    };
    let seconds = epoch.div_euclid(per_second);
    let nanos = epoch.rem_euclid(per_second) * (NANOS_PER_SECOND / per_second);
    if seconds < MIN_TIMESTAMP_SECONDS as i128 || seconds > MAX_TIMESTAMP_SECONDS as i128 {
        return Err(Failure::spanless(Site::UnixTypedOutsideWindow));
    }
    Ok(Timestamp { seconds: seconds as i64, nanos: nanos as i32 })
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
    excel_serial_text(input.as_ref(), epoch).map_err(Failure::fault)
}

/// The Excel serial text door's one parse, for both entry points.
#[inline]
pub(crate) fn excel_serial_text(input: &[u8], epoch: ExcelEpoch) -> Result<Timestamp, Failure> {
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Failure::spanless(Site::SerialTextBlank));
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
        let site = match text.first() {
            Some(b'-') => Site::SerialTextNegative,
            Some(b'+') => Site::SerialTextPlus,
            _ => Site::SerialTextDigitExpected,
        };
        return Err(Failure::at(site, start, char_len_at(text, 0)));
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
            return Err(Failure::at(Site::SerialTextFractionDigit, start + fraction_start - 1, 1));
        }
        nanos_of_day = (scaled * 86_400 * NANOS_PER_SECOND as u128 / scale) as i128;
    }
    if i != text.len() {
        // White space after a whole serial leaves more text after a complete value; any
        // other character sits where the numeral's grammar allows a digit or the point.
        let rest = text.get(i..).unwrap_or_default();
        let site = if crate::text::white_space_len(rest) != 0 {
            Site::SerialTextTrailing
        } else {
            Site::SerialTextUnexpected
        };
        return Err(Failure::at(site, start + i, char_len_at(text, i)));
    }

    let day_number = match excel_day_number(days, epoch) {
        Some(day_number) if !over => day_number,
        _ => {
            let site = if over {
                Site::SerialTextOverflow
            } else {
                refused_day(days, epoch, [Site::SerialTextPhantom, Site::SerialTextOutsideWindow])
            };
            return Err(Failure::at(site, start, text.len()));
        }
    };

    let seconds = day_number * 86_400 + (nanos_of_day / NANOS_PER_SECOND) as i64;
    let nanos = (nanos_of_day % NANOS_PER_SECOND) as i32;
    if !(MIN_TIMESTAMP_SECONDS..=MAX_TIMESTAMP_SECONDS).contains(&seconds) {
        return Err(Failure::at(Site::SerialTextInstantOutsideWindow, start, text.len()));
    }
    Ok(Timestamp { seconds, nanos })
}

/// Parse-sanity bound on any single digit run in a duration — Svartalfheim's `MaxDigits`.
/// Not the overflow guard; overflow is the checked total-nanos arithmetic.
const MAX_DURATION_DIGITS: usize = 18;

/// Casts a duration in any of three cleanly-partitioned shapes to a protobuf [`Duration`]:
/// a leading `[-]P` is an ISO 8601 duration restricted to fixed components (`nW`/`nD`, then
/// `T` with `nH`/`nM`/`n[.f+]S` — years and months are not fixed durations and are
/// `Malformed`); a token containing `:` is the invariant colon form `[-][d.]hh:mm[:ss[.f]]`;
/// `[-]digits[.f+]s` is the protobuf JSON form. Beyond ±10,000 years of whole seconds
/// ⇒ `OutOfRange`. `seconds` and `nanos` come out same-signed.
pub fn cast_duration(input: impl AsRef<[u8]>) -> Result<Duration, Fault> {
    duration(input.as_ref()).map_err(Failure::fault)
}

/// The duration door's one parse, for both entry points.
#[inline]
pub(crate) fn duration(input: &[u8]) -> Result<Duration, Failure> {
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Failure::spanless(Site::DurationBlank));
    }
    let signed_head = if text[0] == b'-' { 1 } else { 0 };
    let total_nanos = if text.len() > signed_head && (text[signed_head] | 0x20) == b'p' {
        parse_iso_duration(text, start)?
    } else if text.contains(&b':') {
        parse_colon_duration(text, start)?
    } else {
        parse_protobuf_duration(text, start)?
    };

    // Split with a native 64-bit division whenever the total fits one (±292 years); i128
    // division is a software routine, and was an eighth of the door's time.
    let (seconds, nanos) = match i64::try_from(total_nanos) {
        Ok(total) => {
            (i128::from(total / NANOS_PER_SECOND as i64), (total % NANOS_PER_SECOND as i64) as i32)
        }
        Err(_) => (total_nanos / NANOS_PER_SECOND, (total_nanos % NANOS_PER_SECOND) as i32),
    };
    if seconds.unsigned_abs() > MAX_DURATION_SECONDS as u128 {
        return Err(Failure::at(Site::DurationOutsideWindow, start, text.len()));
    }
    Ok(Duration { seconds: seconds as i64, nanos })
}

/// Reads a bounded ASCII digit run, returning (value, digit count, next index).
#[inline(always)]
fn read_digit_run(text: &[u8], at: usize, start: usize) -> Result<(i128, usize, usize), Failure> {
    let mut i = at;
    // Eighteen digits fit a u64, so the run accumulates natively and widens once.
    let mut value: u64 = 0;
    while i < text.len() && text[i].is_ascii_digit() {
        if i - at == MAX_DURATION_DIGITS {
            return Err(Failure::at(Site::DigitRunTooLong, start + i, 1));
        }
        value = value * 10 + u64::from(text[i] - b'0');
        i += 1;
    }
    Ok((i128::from(value), i - at, i))
}

/// The ISO 8601 grammar, ported from Svartalfheim's `TryParseIso8601Duration`:
/// `[-] 'P' { n('W'|'D') } [ 'T' { n('H'|'M') | n[.f]('S') } ]` — at least one component,
/// fraction only on seconds, year/month and misplaced units rejected.
fn parse_iso_duration(text: &[u8], start: usize) -> Result<i128, Failure> {
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
                return Err(Failure::at(Site::IsoTRepeated, start + i, 1));
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
            return Err(Failure::at(Site::IsoDigitExpected, start + bad, char_len_at(text, bad)));
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
            return Err(Failure::at(Site::IsoUnitMissing, start + run_start, i - run_start));
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
            _ => {
                let len = char_len_at(text, unit_pos);
                return Err(Failure::at(Site::IsoUnknownUnit, start + unit_pos, len));
            }
        };
        if has_fraction && nanos_per_unit != NANOS_PER_SECOND {
            let len = unit_pos - run_start + 1;
            return Err(Failure::at(Site::IsoFractionNotAllowed, start + run_start, len));
        }

        // A component under ~292 years scales with one native 64-bit multiplication; only a
        // larger one pays for the 128-bit product.
        let scaled = i64::try_from(value)
            .ok()
            .zip(i64::try_from(nanos_per_unit).ok())
            .and_then(|(value, unit)| value.checked_mul(unit))
            .map(i128::from)
            .or_else(|| value.checked_mul(nanos_per_unit));
        total = scaled
            .and_then(|scaled| scaled.checked_add(fraction_nanos))
            .and_then(|component| total.checked_add(component))
            .ok_or_else(|| Failure::at(Site::IsoOverflow, start, text.len()))?;
        saw_component = true;
        if in_time {
            saw_time_component = true;
        }
    }

    if !saw_component || (in_time && !saw_time_component) {
        return Err(Failure::at(Site::IsoNoComponent, start, text.len()));
    }
    Ok(if negative { -total } else { total })
}

/// The invariant colon form `[-][d.]hh:mm[:ss[.f+]]` — hours 0–23 (a larger total
/// needs the day part), minutes and seconds 0–59, each 1–2 digits, .NET's invariant
/// `TimeSpan` profile with the fraction widened to nanos. Shape first, as everywhere: a
/// well-formed field past its range (`25:00:00`, `01:60:00`) ⇒ `OutOfRange` at its digits,
/// where .NET throws `OverflowException`.
fn parse_colon_duration(text: &[u8], start: usize) -> Result<i128, Failure> {
    let mut i = 0;
    let negative = text[0] == b'-';
    if negative {
        i = 1;
    }

    let (first, first_digits, after_first) = read_digit_run(text, i, start)?;
    if first_digits == 0 {
        let bad = after_first.min(text.len() - 1);
        let len = char_len_at(text, bad);
        return Err(Failure::at(Site::ColonFirstDigitExpected, start + bad, len));
    }
    i = after_first;

    let mut days: i128 = 0;
    let hours: i128;
    let mut hour_span = (after_first - first_digits, first_digits);
    if text.get(i) == Some(&b'.') {
        days = first;
        i += 1;
        let (h, h_digits, after_hours) = read_digit_run(text, i, start)?;
        if h_digits == 0 {
            return Err(malformed_at(Site::ColonHourDigitExpected, text, start, i, 1));
        }
        if h_digits > 2 {
            return Err(malformed_at(Site::ColonHourWidth, text, start, i, after_hours - i));
        }
        hours = h;
        hour_span = (i, h_digits);
        i = after_hours;
    } else {
        if first_digits > 2 {
            let at = start + if negative { 1 } else { 0 };
            return Err(Failure::at(Site::ColonFirstWidth, at, first_digits));
        }
        hours = first;
    }

    if text.get(i) != Some(&b':') {
        let bad = i.min(text.len() - 1);
        return Err(Failure::at(Site::ColonColon, start + bad, char_len_at(text, bad)));
    }
    i += 1;
    let (minutes, m_digits, after_minutes) = read_digit_run(text, i, start)?;
    if m_digits == 0 {
        return Err(malformed_at(Site::ColonMinuteDigitExpected, text, start, i, 1));
    }
    if m_digits > 2 {
        return Err(malformed_at(Site::ColonMinuteWidth, text, start, i, after_minutes - i));
    }
    let minute_span = (i, m_digits);
    i = after_minutes;

    let mut seconds: i128 = 0;
    let mut second_span = minute_span;
    let mut fraction_nanos: i128 = 0;
    if text.get(i) == Some(&b':') {
        i += 1;
        let (s, s_digits, after_seconds) = read_digit_run(text, i, start)?;
        if s_digits == 0 {
            return Err(malformed_at(Site::ColonSecondDigitExpected, text, start, i, 1));
        }
        if s_digits > 2 {
            return Err(malformed_at(Site::ColonSecondWidth, text, start, i, after_seconds - i));
        }
        seconds = s;
        second_span = (i, s_digits);
        i = after_seconds;
        let (nanos, after_fraction) = read_fraction_marked(text, i, start, b".,")?;
        fraction_nanos = nanos as i128;
        i = after_fraction;
    }
    if i != text.len() {
        return Err(Failure::at(Site::ColonTrailing, start + i, char_len_at(text, i)));
    }
    for (value, (at, len), max) in
        [(hours, hour_span, 23), (minutes, minute_span, 59), (seconds, second_span, 59)]
    {
        if value > max {
            return Err(Failure::at(Site::ColonFieldRange, start + at, len));
        }
    }

    let total = ((days * 86_400 + hours * 3_600 + minutes * 60 + seconds) * NANOS_PER_SECOND)
        + fraction_nanos;
    Ok(if negative { -total } else { total })
}

/// The protobuf JSON form `[-]digits[.f+]s`, case-insensitive suffix.
fn parse_protobuf_duration(text: &[u8], start: usize) -> Result<i128, Failure> {
    let last = text.len() - 1;
    if (text[last] | 0x20) != b's' {
        return Err(Failure::at(Site::ProtobufUnitMissing, start, text.len()));
    }
    let body = &text[..last];
    let mut i = 0;
    let negative = !body.is_empty() && body[0] == b'-';
    if negative {
        i = 1;
    }
    let (seconds, digits, after) = read_digit_run(body, i, start)?;
    if digits == 0 {
        // Nothing but the sign ahead of the `s` is a bare unit; anything else is no number.
        let site =
            if i == body.len() { Site::ProtobufNoDigits } else { Site::ProtobufDigitExpected };
        return Err(Failure::at(site, start, text.len()));
    }
    let (nanos, after_fraction) = read_fraction_marked(body, after, start, b".,")?;
    if after_fraction != body.len() {
        // An `s` here ends a complete duration with more after it.
        let site = match body.get(after_fraction) {
            Some(b's' | b'S') => Site::ProtobufTrailing,
            _ => Site::ProtobufUnexpected,
        };
        let len = char_len_at(body, after_fraction);
        return Err(Failure::at(site, start + after_fraction, len));
    }
    let total = seconds * NANOS_PER_SECOND + nanos as i128;
    Ok(if negative { -total } else { total })
}
