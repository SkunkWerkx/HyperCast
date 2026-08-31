//! The Python backend: the hypercast core linked straight into a CPython extension module
//! via PyO3 — and the *only* Python backend: the wheel maturin builds from this feature is
//! the whole package (HyperUuid's PyO3-wheels-are-the-whole-story consolidation, ported
//! home; the interim ctypes fallback is gone). A door here is an ordinary extension call —
//! the same `METH_FASTCALL` path the builtins walk — into a direct Rust call. No dlopen,
//! no C-ABI hop, no per-call boxing.
//!
//! The Python package (`hypercast/__init__.py`) re-exports everything here directly:
//! `Success`/`Fault`/`NumFormat` are the package's own types, `__match_args__` included,
//! so `match`/`case` and equality behave exactly as the docstrings promise. Built
//! abi3-py310, so one wheel per platform covers every CPython from the package's 3.10
//! floor up.

use std::borrow::Cow;
use std::sync::OnceLock;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDate, PyDateTime, PyDelta, PyDict, PyTime, PyTuple, PyTzInfo};

use crate as core;

/// Cached Python-side companions: the three CastFailure members (the package's own
/// IntEnum, handed over via `_bind`) and `uuid.UUID`.
static EMPTY: OnceLock<Py<PyAny>> = OnceLock::new();
static MALFORMED: OnceLock<Py<PyAny>> = OnceLock::new();
static OUT_OF_RANGE: OnceLock<Py<PyAny>> = OnceLock::new();
static UUID_CLASS: OnceLock<Py<PyAny>> = OnceLock::new();

/// The success case of a verdict: a cast value.
#[pyclass(frozen, module = "hypercast")]
struct Success {
    #[pyo3(get)]
    value: Py<PyAny>,
}

#[pymethods]
impl Success {
    // Python's own spelling — the dunder is the API, not a Rust constant.
    #[allow(non_upper_case_globals)]
    #[classattr]
    const __match_args__: (&'static str,) = ("value",);

    #[new]
    fn new(value: Py<PyAny>) -> Self {
        Success { value }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<bool> {
        let Ok(other) = other.cast::<Success>() else {
            return Ok(false);
        };
        self.value.bind(py).eq(other.get().value.bind(py))
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!("Success(value={})", self.value.bind(py).repr()?))
    }
}

/// The failure case: a closed reason plus the offending byte span into the UTF-8 input.
#[pyclass(frozen, module = "hypercast")]
struct Fault {
    #[pyo3(get)]
    reason: Py<PyAny>,
    #[pyo3(get)]
    offset: u32,
    #[pyo3(get)]
    length: u32,
}

#[pymethods]
impl Fault {
    #[allow(non_upper_case_globals)]
    #[classattr]
    const __match_args__: (&'static str, &'static str, &'static str) =
        ("reason", "offset", "length");

    #[new]
    fn new(reason: Py<PyAny>, offset: u32, length: u32) -> Self {
        Fault { reason, offset, length }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<bool> {
        let Ok(other) = other.cast::<Fault>() else {
            return Ok(false);
        };
        let other = other.get();
        Ok(self.offset == other.offset
            && self.length == other.length
            && self.reason.bind(py).eq(other.reason.bind(py))?)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "Fault(reason={}, offset={}, length={})",
            self.reason.bind(py).repr()?,
            self.offset,
            self.length
        ))
    }
}

/// Caller-declared numeric notation, held pre-resolved as the core's `NumFormat` so the
/// hot path pays zero conversion.
#[pyclass(frozen, module = "hypercast")]
struct NumFormat {
    resolved: core::NumFormat,
}

#[pymethods]
impl NumFormat {
    #[classattr]
    const GROUPING: u32 = core::NumFormat::GROUPING;
    #[classattr]
    const PARENTHESES: u32 = core::NumFormat::PARENS;
    #[classattr]
    const EXPONENT: u32 = core::NumFormat::EXPONENT;
    #[classattr]
    const RADIX_PREFIXES: u32 = core::NumFormat::RADIX_PREFIX;
    #[classattr]
    const PERCENT: u32 = core::NumFormat::PERCENT;
    #[classattr]
    const ALL: u32 = core::NumFormat::ALL;

    // Python's own spelling — the constant-style name is the API.
    #[allow(non_snake_case)]
    #[classattr]
    fn INVARIANT() -> NumFormat {
        NumFormat { resolved: core::NumFormat::INVARIANT }
    }

    #[new]
    fn new(decimal_sep: &str, group_sep: &str, flags: u32) -> PyResult<Self> {
        let (decimal, group) = (single_char(decimal_sep)?, single_char(group_sep)?);
        if decimal == group {
            return Err(PyValueError::new_err(format!(
                "Decimal and group separators must differ; both are {decimal_sep:?}"
            )));
        }
        Ok(NumFormat { resolved: core::NumFormat { decimal_sep: decimal, group_sep: group, flags } })
    }

    #[getter]
    fn decimal_sep(&self) -> String {
        self.resolved.decimal_sep.to_string()
    }

    #[getter]
    fn group_sep(&self) -> String {
        self.resolved.group_sep.to_string()
    }

    #[getter]
    fn flags(&self) -> u32 {
        self.resolved.flags
    }

    #[staticmethod]
    #[pyo3(signature = (conv = None))]
    fn from_localeconv(py: Python<'_>, conv: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let conv = match conv {
            Some(conv) => conv,
            None => py
                .import("locale")?
                .call_method0("localeconv")?
                .cast_into::<PyDict>()?,
        };
        let field = |name: &str, fallback: char| -> PyResult<char> {
            match conv.get_item(name)? {
                Some(value) => {
                    let text = value.extract::<String>()?;
                    Ok(text.chars().next().unwrap_or(fallback))
                }
                None => Ok(fallback),
            }
        };
        let decimal = field("decimal_point", '.')?;
        let group = field("thousands_sep", ',')?;
        Ok(NumFormat {
            resolved: core::NumFormat {
                decimal_sep: decimal,
                group_sep: group,
                flags: core::NumFormat::ALL,
            },
        })
    }
}

fn single_char(text: &str) -> PyResult<char> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(only), None) => Ok(only),
        _ => Err(PyValueError::new_err("Separators must be single characters")),
    }
}

/// Door input: str (borrowed as UTF-8 where CPython's cached encoding allows) or bytes,
/// zero-copy views into the caller's own object.
#[derive(FromPyObject)]
enum Text<'py> {
    #[pyo3(transparent)]
    Str(Bound<'py, pyo3::types::PyString>),
    #[pyo3(transparent)]
    Bytes(Bound<'py, PyBytes>),
}

impl Text<'_> {
    // to_str() needs non-limited-API access, unavailable under abi3; to_cow() is the
    // abi3-safe equivalent — still borrowed for the ASCII/UTF-8-cached common case, owned
    // only when the limited API forces a copy (same trade HyperUuid made).
    fn bytes(&self) -> PyResult<Cow<'_, [u8]>> {
        match self {
            Text::Str(text) => Ok(match text.to_cow()? {
                Cow::Borrowed(text) => Cow::Borrowed(text.as_bytes()),
                Cow::Owned(text) => Cow::Owned(text.into_bytes()),
            }),
            Text::Bytes(bytes) => Ok(Cow::Borrowed(bytes.as_bytes())),
        }
    }
}

fn fault(py: Python<'_>, failed: core::Fault) -> PyResult<Py<PyAny>> {
    let member = match failed.reason {
        core::Reason::Empty => &EMPTY,
        core::Reason::Malformed => &MALFORMED,
        core::Reason::OutOfRange => &OUT_OF_RANGE,
    };
    let reason = member
        .get()
        .ok_or_else(|| PyValueError::new_err("hypercast._native used before _bind"))?
        .clone_ref(py);
    Ok(Py::new(py, Fault { reason, offset: failed.offset, length: failed.len })?.into_any())
}

fn verdict<'py, T>(
    py: Python<'py>,
    outcome: Result<T, core::Fault>,
    into: impl FnOnce(Python<'py>, T) -> PyResult<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    match outcome {
        Ok(value) => {
            let value = into(py, value)?;
            Ok(Py::new(py, Success { value })?.into_any())
        }
        Err(failed) => fault(py, failed),
    }
}

macro_rules! numeric_doors {
    ($($door:ident => $core:ident),+ $(,)?) => {$(
        #[pyfunction]
        fn $door(py: Python<'_>, text: Text<'_>, fmt: PyRef<'_, NumFormat>) -> PyResult<Py<PyAny>> {
            verdict(py, core::$core(text.bytes()?, &fmt.resolved), |py, value| {
                Ok(value.into_pyobject(py)?.into_any().unbind())
            })
        }
    )+};
}

numeric_doors! {
    cast_i8 => cast_i8,
    cast_i16 => cast_i16,
    cast_i32 => cast_i32,
    cast_i64 => cast_i64,
    cast_u8 => cast_u8,
    cast_u16 => cast_u16,
    cast_u32 => cast_u32,
    cast_u64 => cast_u64,
    cast_f32 => cast_f32,
    cast_f64 => cast_f64,
}

#[pyfunction]
fn cast_bool(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, core::cast_bool(text.bytes()?), |py, value| {
        Ok(value.into_pyobject(py)?.to_owned().into_any().unbind())
    })
}

#[pyfunction]
fn cast_uuid(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, core::cast_uuid(text.bytes()?), |py, bytes| {
        let class = UUID_CLASS
            .get()
            .ok_or_else(|| PyValueError::new_err("hypercast._native used before _bind"))?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("bytes", PyBytes::new(py, &bytes))?;
        Ok(class
            .bind(py)
            .call(PyTuple::empty(py), Some(&kwargs))?
            .unbind())
    })
}

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

fn instant<'py>(py: Python<'py>, ts: core::Timestamp) -> PyResult<Py<PyAny>> {
    let days = ts.seconds.div_euclid(86_400);
    let second_of_day = ts.seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let (hour, rest) = (second_of_day / 3_600, second_of_day % 3_600);
    let (minute, second) = (rest / 60, rest % 60);
    let utc = PyTzInfo::utc(py)?;
    Ok(PyDateTime::new(
        py,
        year,
        month,
        day,
        hour as u8,
        minute as u8,
        second as u8,
        (ts.nanos / 1_000) as u32,
        Some(&utc),
    )?
    .into_any()
    .unbind())
}

#[pyfunction]
fn cast_timestamp(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, core::cast_timestamp(text.bytes()?), instant)
}

#[pyfunction]
fn cast_unix(py: Python<'_>, text: Text<'_>, precision: u32) -> PyResult<Py<PyAny>> {
    let precision = match precision {
        1 => core::UnixPrecision::Seconds,
        2 => core::UnixPrecision::Millis,
        3 => core::UnixPrecision::Micros,
        4 => core::UnixPrecision::Nanos,
        _ => return Err(PyValueError::new_err("precision must be a UnixPrecision")),
    };
    verdict(py, core::cast_unix(text.bytes()?, precision), instant)
}

fn date_value(py: Python<'_>, date: core::Date) -> PyResult<Py<PyAny>> {
    Ok(PyDate::new(py, i32::from(date.year), date.month, date.day)?
        .into_any()
        .unbind())
}

#[pyfunction]
#[pyo3(signature = (text, order = None))]
fn cast_date(py: Python<'_>, text: Text<'_>, order: Option<u32>) -> PyResult<Py<PyAny>> {
    let Some(order) = order else {
        return verdict(py, core::cast_date(text.bytes()?), date_value);
    };
    let order = match order {
        1 => core::DateOrder::YearMonthDay,
        2 => core::DateOrder::MonthDayYear,
        3 => core::DateOrder::DayMonthYear,
        _ => return Err(PyValueError::new_err("order must be a DateOrder")),
    };
    verdict(py, core::cast_date_ordered(text.bytes()?, order), date_value)
}

#[pyfunction]
fn cast_datetime(py: Python<'_>, text: Text<'_>, order: u32) -> PyResult<Py<PyAny>> {
    let order = match order {
        1 => core::DateOrder::YearMonthDay,
        2 => core::DateOrder::MonthDayYear,
        3 => core::DateOrder::DayMonthYear,
        _ => return Err(PyValueError::new_err("order must be a DateOrder")),
    };
    verdict(py, core::cast_datetime(text.bytes()?, order), |py, civil| {
        // Naive datetime — the text named no zone, so the value carries none; fusing a
        // zone is the caller's job. Sub-microsecond nanoseconds truncate (Python's ceiling).
        let (second_of_day, nano) = (civil.nanos_of_day / 1_000_000_000, civil.nanos_of_day % 1_000_000_000);
        let (hour, rest) = (second_of_day / 3_600, second_of_day % 3_600);
        let (minute, second) = (rest / 60, rest % 60);
        Ok(PyDateTime::new(
            py,
            i32::from(civil.date.year),
            civil.date.month,
            civil.date.day,
            hour as u8,
            minute as u8,
            second as u8,
            (nano / 1_000) as u32,
            None,
        )?
        .into_any()
        .unbind())
    })
}

#[pyfunction]
fn cast_time(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, core::cast_time(text.bytes()?), |py, nanos| {
        let (second_of_day, nano) = (nanos / 1_000_000_000, nanos % 1_000_000_000);
        let (hour, rest) = (second_of_day / 3_600, second_of_day % 3_600);
        let (minute, second) = (rest / 60, rest % 60);
        Ok(PyTime::new(
            py,
            hour as u8,
            minute as u8,
            second as u8,
            (nano / 1_000) as u32,
            None,
        )?
        .into_any()
        .unbind())
    })
}

#[pyfunction]
fn cast_duration(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, core::cast_duration(text.bytes()?), |py, span| {
        // Truncate sub-microsecond digits toward zero on both signs, matching every other
        // binding's truncation; PyDelta normalizes the mixed-sign pieces.
        let nanos = i64::from(span.nanos);
        let micros = if nanos >= 0 { nanos / 1_000 } else { -((-nanos) / 1_000) };
        let days = span.seconds.div_euclid(86_400);
        let seconds = span.seconds.rem_euclid(86_400);
        Ok(PyDelta::new(py, days as i32, seconds as i32, micros as i32, true)?
            .into_any()
            .unbind())
    })
}

/// Hands the package's own `CastFailure` IntEnum (and `uuid.UUID`) to this backend so
/// faults carry the exact members callers compare with `is`.
#[pyfunction]
fn _bind(py: Python<'_>, cast_failure: Bound<'_, PyAny>) -> PyResult<()> {
    let _ = EMPTY.set(cast_failure.getattr("EMPTY")?.unbind());
    let _ = MALFORMED.set(cast_failure.getattr("MALFORMED")?.unbind());
    let _ = OUT_OF_RANGE.set(cast_failure.getattr("OUT_OF_RANGE")?.unbind());
    let _ = UUID_CLASS.set(py.import("uuid")?.getattr("UUID")?.unbind());
    Ok(())
}

// Name must match module-name's last segment in pyproject.toml ("hypercast._native") —
// PyO3 generates a PyInit_<name> symbol from this function's own name, and maturin/Python's
// import machinery look for PyInit__native specifically (confirmed in HyperUuid via a real
// build warning, not assumed).
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Success>()?;
    m.add_class::<Fault>()?;
    m.add_class::<NumFormat>()?;
    m.add_function(wrap_pyfunction!(cast_bool, m)?)?;
    m.add_function(wrap_pyfunction!(cast_i8, m)?)?;
    m.add_function(wrap_pyfunction!(cast_i16, m)?)?;
    m.add_function(wrap_pyfunction!(cast_i32, m)?)?;
    m.add_function(wrap_pyfunction!(cast_i64, m)?)?;
    m.add_function(wrap_pyfunction!(cast_u8, m)?)?;
    m.add_function(wrap_pyfunction!(cast_u16, m)?)?;
    m.add_function(wrap_pyfunction!(cast_u32, m)?)?;
    m.add_function(wrap_pyfunction!(cast_u64, m)?)?;
    m.add_function(wrap_pyfunction!(cast_f32, m)?)?;
    m.add_function(wrap_pyfunction!(cast_f64, m)?)?;
    m.add_function(wrap_pyfunction!(cast_uuid, m)?)?;
    m.add_function(wrap_pyfunction!(cast_timestamp, m)?)?;
    m.add_function(wrap_pyfunction!(cast_unix, m)?)?;
    m.add_function(wrap_pyfunction!(cast_date, m)?)?;
    m.add_function(wrap_pyfunction!(cast_datetime, m)?)?;
    m.add_function(wrap_pyfunction!(cast_time, m)?)?;
    m.add_function(wrap_pyfunction!(cast_duration, m)?)?;
    m.add_function(wrap_pyfunction!(_bind, m)?)?;
    Ok(())
}
