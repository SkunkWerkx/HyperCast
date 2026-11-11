//! Every door again, reporting a [`Failure`] — the plain door's reason and span, with the
//! [`Detail`](crate::Detail) of what failed — instead of a bare [`Fault`](crate::Fault).
//!
//! Each function has its door's name and reads exactly as that door does: the two are one
//! parse, so a detailed door's failure is always the plain door's fault with a detail added
//! ([`Failure::fault`]), and a value is always the same value. The numeric doors take a
//! [`ValidNumFormat`], a declaration [`NumFormat::validate`](crate::NumFormat::validate) has
//! accepted, so a format they could not honour never reaches them; a refusal there is the
//! caller's [`DeclarationError`](crate::DeclarationError), never a `Failure`, whose reason
//! always describes the input.
//!
//! ```
//! use hypercast::{Detail, Reason, ValidNumFormat, detailed};
//!
//! let failure = detailed::cast_i32("12a", &ValidNumFormat::INVARIANT).unwrap_err();
//! assert_eq!((failure.reason, failure.detail), (Reason::Malformed, Detail::UnexpectedCharacter));
//! assert_eq!((failure.offset, failure.len), (2, 1));
//! assert_eq!(Err(failure.fault()), hypercast::cast_i32("12a", &hypercast::NumFormat::INVARIANT));
//! ```

use core::fmt;

use crate::checked::{Checked, Violation};
use crate::constraints::{Constraints, Numeric, Text};
use crate::date_options::ValidDateOptions;
use crate::detail::Failure;
use crate::temporal::{DateOrder, DayPeriods, ExcelEpoch, UnixPrecision};
use crate::verdict::{CivilDateTime, Date, Decimal, Duration, Reason, Timestamp, ValidNumFormat};

/// Why a detailed checked door refused a value: it did not parse, or it parsed and broke a
/// rule. The plain checked door's [`Rejection`](crate::Rejection), with the failure detailed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection<T> {
    /// The door's own failure: the input did not cast.
    Failure(Failure),
    /// The value cast and broke a declared constraint; the violation carries it.
    Violation(Violation<T>),
}

impl<T> Rejection<T> {
    /// The same rejection, a violation's value as `f(value)`; a failure passes through.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Rejection<U> {
        match self {
            Rejection::Failure(failure) => Rejection::Failure(failure),
            Rejection::Violation(violation) => Rejection::Violation(violation.map(f)),
        }
    }
}

impl<T> fmt::Display for Rejection<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rejection::Failure(failure) => failure.fmt(f),
            Rejection::Violation(violation) => violation.fmt(f),
        }
    }
}

impl<T: fmt::Debug> core::error::Error for Rejection<T> {}

impl<T> From<Failure> for Rejection<T> {
    fn from(failure: Failure) -> Rejection<T> {
        Rejection::Failure(failure)
    }
}

impl<T> From<Violation<T>> for Rejection<T> {
    fn from(violation: Violation<T>) -> Rejection<T> {
        Rejection::Violation(violation)
    }
}

/// A rejection as a detailed door reports it for `input`: its failure classified at its span.
pub(crate) fn classified<T>(rejection: Rejection<T>, input: &[u8]) -> Rejection<T> {
    match rejection {
        Rejection::Failure(failure) => Rejection::Failure(failure.classified(input)),
        violation => violation,
    }
}

/// [`crate::optional`] for a detailed verdict: a [`Reason::Empty`] failure becomes `Ok(None)`.
pub fn optional<T>(verdict: Result<T, Failure>) -> Result<Option<T>, Failure> {
    match verdict {
        Ok(value) => Ok(Some(value)),
        Err(failure) if failure.reason == Reason::Empty => Ok(None),
        Err(failure) => Err(failure),
    }
}

/// [`crate::optional_checked`] for a detailed checked verdict: a [`Reason::Empty`] failure
/// becomes `Ok(None)`; every other failure and every violation flows through.
pub fn optional_checked<T>(
    verdict: Result<Checked<T>, Rejection<T>>,
) -> Result<Option<Checked<T>>, Rejection<T>> {
    match verdict {
        Ok(checked) => Ok(Some(checked)),
        Err(Rejection::Failure(failure)) if failure.reason == Reason::Empty => Ok(None),
        Err(rejection) => Err(rejection),
    }
}

/// [`crate::cast_bool`], detailed.
pub fn cast_bool(input: impl AsRef<[u8]>) -> Result<bool, Failure> {
    let input = input.as_ref();
    crate::boolean::boolean(input).map_err(|failure| failure.classified(input))
}

/// [`crate::cast_char`], detailed.
pub fn cast_char(input: impl AsRef<[u8]>) -> Result<char, Failure> {
    let input = input.as_ref();
    crate::character::character(input).map_err(|failure| failure.classified(input))
}

macro_rules! integer_doors {
    ($($door:ident => $ty:ty),+) => {$(
        #[doc = concat!("[`crate::", stringify!($door), "`], detailed.")]
        pub fn $door(input: impl AsRef<[u8]>, format: &ValidNumFormat) -> Result<$ty, Failure> {
            let input = input.as_ref();
            crate::integer::integer::<$ty>(input, format.format())
                .map_err(|failure| failure.classified(input))
        }
    )+};
}

integer_doors! {
    cast_i8 => i8, cast_i16 => i16, cast_i32 => i32, cast_i64 => i64,
    cast_u8 => u8, cast_u16 => u16, cast_u32 => u32, cast_u64 => u64
}

/// [`crate::cast_f32`], detailed.
pub fn cast_f32(input: impl AsRef<[u8]>, format: &ValidNumFormat) -> Result<f32, Failure> {
    let input = input.as_ref();
    crate::real::real_f32(input, format.format(), true).map_err(|failure| failure.classified(input))
}

/// [`crate::cast_f64`], detailed.
pub fn cast_f64(input: impl AsRef<[u8]>, format: &ValidNumFormat) -> Result<f64, Failure> {
    let input = input.as_ref();
    crate::real::real_f64(input, format.format(), true).map_err(|failure| failure.classified(input))
}

/// [`crate::cast_decimal`], detailed.
pub fn cast_decimal(input: impl AsRef<[u8]>, format: &ValidNumFormat) -> Result<Decimal, Failure> {
    let input = input.as_ref();
    crate::decimal::decimal(input, format.format(), true)
        .map_err(|failure| failure.classified(input))
}

/// [`crate::cast_uuid`], detailed.
pub fn cast_uuid(input: impl AsRef<[u8]>) -> Result<[u8; 16], Failure> {
    let input = input.as_ref();
    crate::uuid::uuid(input).map_err(|failure| failure.classified(input))
}

/// [`crate::cast_timestamp`], detailed, with the shapes `options` opens
/// ([`SPACE_SEPARATOR`](crate::DateOptions::SPACE_SEPARATOR) and
/// [`HOUR_OFFSET`](crate::DateOptions::HOUR_OFFSET)); `None` reads what the plain door reads.
pub fn cast_timestamp(
    input: impl AsRef<[u8]>,
    options: Option<&ValidDateOptions<'_>>,
) -> Result<Timestamp, Failure> {
    let input = input.as_ref();
    crate::temporal::timestamp(input, options).map_err(|failure| failure.classified(input))
}

/// [`crate::cast_unix`], detailed.
pub fn cast_unix(input: impl AsRef<[u8]>, precision: UnixPrecision) -> Result<Timestamp, Failure> {
    let input = input.as_ref();
    crate::temporal::unix(input, precision).map_err(|failure| failure.classified(input))
}

/// [`crate::cast_excel_serial`], detailed.
pub fn cast_excel_serial(input: impl AsRef<[u8]>, epoch: ExcelEpoch) -> Result<Timestamp, Failure> {
    let input = input.as_ref();
    crate::temporal::excel_serial_text(input, epoch).map_err(|failure| failure.classified(input))
}

/// [`crate::cast_date`], detailed, with the shape `options` opens
/// ([`COMPACT_DATE`](crate::DateOptions::COMPACT_DATE)); `None` reads what the plain door reads.
pub fn cast_date(
    input: impl AsRef<[u8]>,
    options: Option<&ValidDateOptions<'_>>,
) -> Result<Date, Failure> {
    let input = input.as_ref();
    crate::temporal::date(input, options).map_err(|failure| failure.classified(input))
}

/// [`crate::cast_date_ordered`], detailed, with the two-digit years of `options`' pivot and
/// the month names of its vocabulary; `None` reads what the plain door reads.
pub fn cast_date_ordered(
    input: impl AsRef<[u8]>,
    order: DateOrder,
    options: Option<&ValidDateOptions<'_>>,
) -> Result<Date, Failure> {
    let input = input.as_ref();
    crate::temporal::date_ordered(input, order, options)
        .map_err(|failure| failure.classified(input))
}

/// [`crate::cast_datetime`], detailed, with the pivot and the month names of `options`.
pub fn cast_datetime(
    input: impl AsRef<[u8]>,
    order: DateOrder,
    options: Option<&ValidDateOptions<'_>>,
) -> Result<CivilDateTime, Failure> {
    cast_datetime_with(input, order, &DayPeriods::ENGLISH, options)
}

/// [`crate::cast_datetime_with`], detailed, with the pivot and the month names of `options`.
pub fn cast_datetime_with(
    input: impl AsRef<[u8]>,
    order: DateOrder,
    periods: &DayPeriods,
    options: Option<&ValidDateOptions<'_>>,
) -> Result<CivilDateTime, Failure> {
    let input = input.as_ref();
    crate::temporal::datetime(input, order, periods, options)
        .map_err(|failure| failure.classified(input))
}

/// [`crate::cast_time`], detailed.
pub fn cast_time(input: impl AsRef<[u8]>) -> Result<u64, Failure> {
    let input = input.as_ref();
    crate::temporal::time(input).map_err(|failure| failure.classified(input))
}

/// [`crate::cast_duration`], detailed.
pub fn cast_duration(input: impl AsRef<[u8]>) -> Result<Duration, Failure> {
    let input = input.as_ref();
    crate::temporal::duration(input).map_err(|failure| failure.classified(input))
}

/// [`crate::cast_text`], detailed: invalid UTF-8 is
/// [`Detail::InvalidUtf8`](crate::Detail::InvalidUtf8), and text past `u32::MAX` bytes
/// [`Detail::TooLong`](crate::Detail::TooLong).
pub fn cast_text(input: &[u8]) -> Result<&str, Failure> {
    crate::text::door(input)
}

/// [`crate::excel_serial`], detailed. A typed door's failure has an empty span.
pub fn excel_serial(serial: f64, epoch: ExcelEpoch) -> Result<CivilDateTime, Failure> {
    crate::temporal::serial_typed(serial, epoch)
}

/// [`crate::excel_time`], detailed.
pub fn excel_time(serial: f64) -> Result<u64, Failure> {
    crate::temporal::time_typed(serial)
}

/// [`crate::excel_duration`], detailed.
pub fn excel_duration(days: f64) -> Result<Duration, Failure> {
    crate::temporal::duration_typed(days)
}

/// [`crate::unix_from_f64`], detailed.
pub fn unix_from_f64(value: f64, precision: UnixPrecision) -> Result<Timestamp, Failure> {
    crate::temporal::unix_typed(value, precision)
}

/// [`crate::decimal_from_f64`], detailed.
pub fn decimal_from_f64(value: f64) -> Result<Decimal, Failure> {
    crate::typed::decimal_typed(value)
}

/// [`crate::f32_from_f64`], detailed.
pub fn f32_from_f64(value: f64) -> Result<f32, Failure> {
    crate::typed::f32_typed(value)
}

/// [`crate::bool_from_f64`], detailed.
pub fn bool_from_f64(value: f64) -> Result<bool, Failure> {
    crate::typed::bool_typed(value)
}

macro_rules! typed_integer_doors {
    ($($door:ident => $ty:ty),+) => {$(
        #[doc = concat!("[`crate::", stringify!($door), "`], detailed.")]
        pub fn $door(value: f64) -> Result<$ty, Failure> {
            crate::typed::integer::<$ty>(value, <$ty>::MIN == 0)
        }
    )+};
}

typed_integer_doors! {
    i8_from_f64 => i8, i16_from_f64 => i16, i32_from_f64 => i32, i64_from_f64 => i64,
    u8_from_f64 => u8, u16_from_f64 => u16, u32_from_f64 => u32, u64_from_f64 => u64
}

/// [`crate::cast_checked`], detailed.
pub fn cast_checked<T: Numeric>(
    input: impl AsRef<[u8]>,
    format: &ValidNumFormat,
    constraints: &Constraints<'_, T>,
) -> Result<Checked<T>, Rejection<T>> {
    let input = input.as_ref();
    crate::checked::checked(input, format.format(), constraints)
        .map_err(|rejection| classified(rejection, input))
}

/// [`crate::checked_from_f64`], detailed.
pub fn checked_from_f64<T: Numeric>(
    value: f64,
    constraints: &Constraints<'_, T>,
) -> Result<Checked<T>, Rejection<T>> {
    crate::checked::checked_typed(value, constraints)
}

/// [`crate::cast_text_checked`], detailed.
pub fn cast_text_checked<'a>(
    input: &'a [u8],
    constraints: &Constraints<'a, Text>,
) -> Result<Checked<&'a str>, Rejection<&'a str>> {
    crate::checked::text_checked(input, constraints)
}
