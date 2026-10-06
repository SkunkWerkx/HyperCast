//! The core's values as Python objects — public for a crate whose own CPython extension
//! carries HyperCast's verdicts (HyperTabular's does), so a value it hands out is the object
//! `hypercast.cast_*` returns for the same text, built the same way. The `python-values`
//! feature; the `python` feature (this crate's own extension) uses exactly these.
//!
//! [`Values`] holds the `datetime`, `decimal` and `uuid` objects the conversions call, imported
//! once — each extension module keeps its own, bound when the module is. Temporal values
//! carry Python's own fidelity: sub-microsecond digits truncate (toward zero for a duration).
//!
//! The success and fault objects are not here: a verdict is the `hypercast` package's own
//! `Success` or `Fault`, which another extension module constructs through the package.

use pyo3::exceptions::PyValueError;
use pyo3::ffi;
use pyo3::intern;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDate, PyDateTime, PyDelta, PyTzInfo};

use crate as core;

/// The Python objects the conversions build with, imported by [`Values::import`].
pub struct Values {
    datetime: Py<PyAny>,
    date: Py<PyAny>,
    time: Py<PyAny>,
    utc: Py<PyAny>,
    // 0001-01-01, the datetime a duration is measured from (see `duration`).
    duration_anchor: Py<PyAny>,
    uuid: Py<PyAny>,
    // `uuid.SafeUUID.unknown`, what `UUID(bytes=...)` sets `is_safe` to.
    is_safe_unknown: Py<PyAny>,
    decimal: Py<PyAny>,
}

impl Values {
    /// Imports `datetime`, `decimal` and `uuid` and keeps what the conversions call.
    pub fn import(py: Python<'_>) -> PyResult<Values> {
        let datetime = py.import("datetime")?;
        let uuid = py.import("uuid")?;
        Ok(Values {
            datetime: datetime.getattr("datetime")?.unbind(),
            date: datetime.getattr("date")?.unbind(),
            time: datetime.getattr("time")?.unbind(),
            utc: datetime.getattr("timezone")?.getattr("utc")?.unbind(),
            duration_anchor: datetime.getattr("datetime")?.call1((1, 1, 1))?.unbind(),
            uuid: uuid.getattr("UUID")?.unbind(),
            is_safe_unknown: uuid.getattr("SafeUUID")?.getattr("unknown")?.unbind(),
            decimal: py.import("decimal")?.getattr("Decimal")?.unbind(),
        })
    }

    /// An instant as an aware UTC `datetime` — the timestamp, Unix and Excel-serial doors'
    /// value.
    pub fn instant<'py>(
        &self,
        py: Python<'py>,
        ts: core::Timestamp,
    ) -> PyResult<Bound<'py, PyAny>> {
        let days = ts.seconds.div_euclid(86_400);
        let second_of_day = ts.seconds.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        let (hour, rest) = (second_of_day / 3_600, second_of_day % 3_600);
        let (minute, second) = (rest / 60, rest % 60);
        let micros = (ts.nanos / 1_000) as u32;
        if packs(year) {
            let state =
                packed_datetime(year, month, day, hour as u8, minute as u8, second as u8, micros);
            return self.datetime.bind(py).call1((PyBytes::new(py, &state), self.utc.bind(py)));
        }
        let utc = PyTzInfo::utc(py)?;
        Ok(PyDateTime::new(
            py,
            year,
            month,
            day,
            hour as u8,
            minute as u8,
            second as u8,
            micros,
            Some(&utc),
        )?
        .into_any())
    }

    /// A calendar `date` — the date doors' value.
    pub fn date<'py>(&self, py: Python<'py>, date: core::Date) -> PyResult<Bound<'py, PyAny>> {
        let year = i32::from(date.year);
        if packs(year) {
            let state = packed_date(year, date.month, date.day);
            return self.date.bind(py).call1((PyBytes::new(py, &state),));
        }
        Ok(PyDate::new(py, year, date.month, date.day)?.into_any())
    }

    /// A zone-less wall clock as a naive `datetime` — the date-time doors' value. The text
    /// named no zone, so the value carries none; fusing a zone is the caller's job.
    pub fn civil<'py>(
        &self,
        py: Python<'py>,
        civil: core::CivilDateTime,
    ) -> PyResult<Bound<'py, PyAny>> {
        let (second_of_day, nano) =
            (civil.nanos_of_day / 1_000_000_000, civil.nanos_of_day % 1_000_000_000);
        let (hour, rest) = (second_of_day / 3_600, second_of_day % 3_600);
        let (minute, second) = (rest / 60, rest % 60);
        let year = i32::from(civil.date.year);
        if packs(year) {
            let state = packed_datetime(
                year,
                civil.date.month,
                civil.date.day,
                hour as u8,
                minute as u8,
                second as u8,
                (nano / 1_000) as u32,
            );
            return self.datetime.bind(py).call1((PyBytes::new(py, &state),));
        }
        Ok(PyDateTime::new(
            py,
            year,
            civil.date.month,
            civil.date.day,
            hour as u8,
            minute as u8,
            second as u8,
            (nano / 1_000) as u32,
            None,
        )?
        .into_any())
    }

    /// Nanoseconds since midnight as a `time` — the time doors' value.
    pub fn time<'py>(&self, py: Python<'py>, nanos: u64) -> PyResult<Bound<'py, PyAny>> {
        let (second_of_day, nano) = (nanos / 1_000_000_000, nanos % 1_000_000_000);
        let (hour, rest) = (second_of_day / 3_600, second_of_day % 3_600);
        let (minute, second) = (rest / 60, rest % 60);
        let state = packed_time(hour as u8, minute as u8, second as u8, (nano / 1_000) as u32);
        self.time.bind(py).call1((PyBytes::new(py, &state),))
    }

    /// A span as a `timedelta` — the duration doors' value.
    pub fn duration<'py>(
        &self,
        py: Python<'py>,
        span: core::Duration,
    ) -> PyResult<Bound<'py, PyAny>> {
        // `timedelta(days, seconds, microseconds)` is the slowest constructor in the module —
        // it normalizes through arbitrary-precision arithmetic whatever it is given — while
        // subtracting two datetimes hands back the same object from a fixed-width fast path.
        // So a span that fits is built as (0001-01-01 + |span|) - 0001-01-01, or the other
        // way round for a negative one: sub-microsecond digits truncate toward zero on the
        // magnitude, and the subtraction normalizes the sign exactly as the constructor
        // would. A span too long for a datetime to stand that far from year 1 (past roughly
        // 9,998 years) takes the constructor below.
        let (magnitude_seconds, magnitude_nanos) =
            (span.seconds.unsigned_abs(), span.nanos.unsigned_abs());
        let days = magnitude_seconds / 86_400;
        if days <= ANCHOR_REACH_DAYS {
            let second_of_day = magnitude_seconds % 86_400;
            let (year, month, day) = civil_from_days(days as i64 + ANCHOR_DAYS_FROM_EPOCH);
            let (hour, rest) = (second_of_day / 3_600, second_of_day % 3_600);
            let (minute, second) = (rest / 60, rest % 60);
            let state = packed_datetime(
                year,
                month,
                day,
                hour as u8,
                minute as u8,
                second as u8,
                magnitude_nanos / 1_000,
            );
            let anchor = self.duration_anchor.bind(py);
            let moved = self.datetime.bind(py).call1((PyBytes::new(py, &state),))?;
            let negative = span.seconds < 0 || span.nanos < 0;
            let (left, right) = if negative { (anchor, &moved) } else { (&moved, anchor) };
            return left.sub(right);
        }
        // Truncate sub-microsecond digits toward zero on both signs, matching every other
        // binding's truncation; PyDelta normalizes the mixed-sign pieces.
        let nanos = i64::from(span.nanos);
        let micros = if nanos >= 0 { nanos / 1_000 } else { -((-nanos) / 1_000) };
        let days = span.seconds.div_euclid(86_400);
        let seconds = span.seconds.rem_euclid(86_400);
        Ok(PyDelta::new(py, days as i32, seconds as i32, micros as i32, true)?.into_any())
    }

    /// An exact `decimal.Decimal` — the decimal doors' value, from the core's canonical text
    /// rendered on the stack. Measured against the `(sign, digits, exponent)` tuple
    /// constructor (which has to build a Python int per digit): the text path was faster on
    /// every shape tried, `1234.50` at 405 ns vs 556 ns and the 29-digit ceiling at 788 ns vs
    /// 1189 ns (CPython 3.14, linux-arm64, timeit best of five).
    pub fn decimal<'py>(
        &self,
        py: Python<'py>,
        value: core::Decimal,
    ) -> PyResult<Bound<'py, PyAny>> {
        use std::fmt::Write;
        let mut canonical = Canonical { buf: [0; 48], len: 0 };
        write!(canonical, "{value}")
            .map_err(|_| PyValueError::new_err("hypercast: decimal text overflowed its buffer"))?;
        let text = std::str::from_utf8(&canonical.buf[..canonical.len])
            .map_err(|_| PyValueError::new_err("hypercast: decimal text was not UTF-8"))?;
        self.decimal.bind(py).call1((text,))
    }

    /// A `uuid.UUID` from its 16 RFC 9562-ordered bytes — the uuid door's value.
    pub fn uuid<'py>(&self, py: Python<'py>, bytes: [u8; 16]) -> PyResult<Bound<'py, PyAny>> {
        // The core has already validated the text; `UUID.__init__` would only re-check a
        // value it cannot reject, so the instance is built the way `uuid.UUID` itself stores
        // it — the 128-bit `int` slot, big-endian from the RFC-ordered bytes, and `is_safe`
        // left at `SafeUUID.unknown`, exactly what `UUID(bytes=...)` would set.
        //
        // Through the C API's own entry points rather than Python callables:
        // PyType_GenericAlloc and PyObject_GenericSetAttr are what `object.__new__` and
        // `object.__setattr__` do underneath, without a call, an argument tuple or a fresh
        // `str` per attribute name. Both are in the stable ABI.
        let class = self.uuid.bind(py);
        let value = int_from_be_bytes(py, bytes)?;
        let is_safe = self.is_safe_unknown.bind(py);
        // SAFETY: `uuid` is the `uuid.UUID` type object (imported in `import`); both calls
        // check their arguments and report failure by return value with an exception set.
        unsafe {
            let instance = Bound::from_owned_ptr_or_err(
                py,
                ffi::PyType_GenericAlloc(class.as_ptr().cast(), 0),
            )?;
            if ffi::PyObject_GenericSetAttr(
                instance.as_ptr(),
                intern!(py, "int").as_ptr(),
                value.as_ptr(),
            ) != 0
                || ffi::PyObject_GenericSetAttr(
                    instance.as_ptr(),
                    intern!(py, "is_safe").as_ptr(),
                    is_safe.as_ptr(),
                ) != 0
            {
                return Err(PyErr::fetch(py));
            }
            Ok(instance)
        }
    }
}

/// The core's canonical decimal text (`Display`), rendered on the stack — at most a sign,
/// 29 digits and a point — so the host `Decimal` comes straight out of `_decimal`'s C
/// string constructor with nothing allocated on this side.
struct Canonical {
    buf: [u8; 48],
    len: usize,
}

impl std::fmt::Write for Canonical {
    fn write_str(&mut self, piece: &str) -> std::fmt::Result {
        let end = self.len + piece.len();
        let slot = self.buf.get_mut(self.len..end).ok_or(std::fmt::Error)?;
        slot.copy_from_slice(piece.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// A Python `int` holding 16 big-endian bytes. The stable ABI has no byte-array constructor
/// for `int` before 3.14, so it goes through `PyLong_FromString` in base 16 — one allocation,
/// and measured faster than joining two 64-bit halves with a shift and an or (three).
fn int_from_be_bytes<'py>(py: Python<'py>, bytes: [u8; 16]) -> PyResult<Bound<'py, PyAny>> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    // 32 digits and the terminating NUL PyLong_FromString reads up to.
    let mut text = [0u8; 33];
    for (i, byte) in bytes.iter().enumerate() {
        text[2 * i] = HEX[(byte >> 4) as usize];
        text[2 * i + 1] = HEX[(byte & 15) as usize];
    }
    // SAFETY: `text` is NUL-terminated ASCII; the call returns a new reference, or null with
    // an exception set.
    unsafe {
        Bound::from_owned_ptr_or_err(
            py,
            ffi::PyLong_FromString(text.as_ptr().cast(), std::ptr::null_mut(), 16),
        )
    }
}

// The datetime module's own pickle constructors: `date(state)`, `time(state)` and
// `datetime(state[, tzinfo])`, where `state` is the packed bytes `__reduce__` emits — year
// (two bytes, big-endian), month, day, then hour, minute, second and microsecond (three
// bytes, big-endian). It is the pickle format, so it cannot change under a pickle written by
// an older Python, and it builds the same object as the keyword constructors in a third of
// the time: one bytes object instead of up to seven ints, and no per-field range checks.
// That last part is why each caller falls back to the ordinary constructor for a year
// outside 1..=9999 — the packed form would encode it, and the constructor is what refuses it.
const fn packed_date(year: i32, month: u8, day: u8) -> [u8; 4] {
    [(year >> 8) as u8, year as u8, month, day]
}

const fn packed_time(hour: u8, minute: u8, second: u8, micros: u32) -> [u8; 6] {
    [hour, minute, second, (micros >> 16) as u8, (micros >> 8) as u8, micros as u8]
}

fn packed_datetime(
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    micros: u32,
) -> [u8; 10] {
    let (d, t) = (packed_date(year, month, day), packed_time(hour, minute, second, micros));
    [d[0], d[1], d[2], d[3], t[0], t[1], t[2], t[3], t[4], t[5]]
}

const fn packs(year: i32) -> bool {
    1 <= year && year <= 9_999
}

/// 0001-01-01, the datetime a duration is measured from (see `Values::duration`): its day
/// number on the Unix epoch's count, and how many whole days past it a datetime can still
/// stand (9999-12-31 is day 3,652,058 from it).
const ANCHOR_DAYS_FROM_EPOCH: i64 = -719_162;
const ANCHOR_REACH_DAYS: u64 = 3_652_058;

/// Hinnant's civil_from_days — the inverse of the core's days_from_civil, for presenting
/// `{seconds, nanos}` as a datetime without a strftime round trip.
fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_shifted = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_shifted + 2) / 5 + 1) as u8;
    let month = (if month_shifted < 10 { month_shifted + 3 } else { month_shifted - 9 }) as u8;
    ((year + i64::from(month <= 2)) as i32, month, day)
}
