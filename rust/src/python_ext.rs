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
//! abi3-py311, so one wheel per platform covers every CPython from the package's 3.11
//! floor up.

use std::sync::OnceLock;

use pyo3::exceptions::{PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyString};

use crate as core;

/// Cached Python-side companions: the three CastFailure members (the package's own
/// IntEnum, handed over via `_bind`), and the objects every value is built with
/// ([`crate::python::Values`], shared with any extension that presents the core's values).
static EMPTY: OnceLock<Py<PyAny>> = OnceLock::new();
static MALFORMED: OnceLock<Py<PyAny>> = OnceLock::new();
static OUT_OF_RANGE: OnceLock<Py<PyAny>> = OnceLock::new();
static VALUES: OnceLock<crate::python::Values> = OnceLock::new();

fn values() -> PyResult<&'static crate::python::Values> {
    VALUES.get().ok_or_else(|| PyRuntimeError::new_err("hypercast._native used before _bind"))
}

fn cached<'py>(
    py: Python<'py>,
    cell: &'static OnceLock<Py<PyAny>>,
) -> PyResult<&'py Bound<'py, PyAny>> {
    cell.get()
        .map(|value| value.bind(py))
        .ok_or_else(|| PyRuntimeError::new_err("hypercast._native used before _bind"))
}

/// The success case of a verdict: a cast value.
// `generic` gives the class a `__class_getitem__`, so the `Success[int]` the type stubs
// (`_native.pyi`) describe is also a legal expression at runtime — an annotation that gets
// evaluated, on the Pythons that still evaluate them eagerly, must not raise.
#[pyclass(frozen, generic, module = "hypercast")]
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

/// The failure case: a closed reason plus the offending span, in the caller's own units —
/// byte offsets for `bytes` input, code-point offsets for `str` input — so slicing the
/// offending text back out of what you passed (`text[offset:offset + length]`) needs no
/// mapping.
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
    const CURRENCY: u32 = core::NumFormat::CURRENCY;
    #[classattr]
    const ALL: u32 = core::NumFormat::ALL;
    #[classattr]
    const SEPARATOR_DETECT: u32 = core::NumFormat::SEPARATOR_DETECT;

    // Python's own spelling — the constant-style name is the API.
    #[allow(non_snake_case)]
    #[classattr]
    fn INVARIANT() -> NumFormat {
        NumFormat { resolved: core::NumFormat::INVARIANT }
    }

    // Python's own spelling, as with INVARIANT above.
    #[allow(non_snake_case)]
    #[classattr]
    fn DETECT() -> NumFormat {
        NumFormat { resolved: core::NumFormat::DETECT }
    }

    #[new]
    #[pyo3(signature = (decimal_sep, group_sep, flags, currency = ""))]
    fn new(decimal_sep: &str, group_sep: &str, flags: u32, currency: &str) -> PyResult<Self> {
        declare(single_char(decimal_sep)?, single_char(group_sep)?, flags, currency)
    }

    /// The declared decimal separator.
    #[getter]
    fn decimal_sep(&self) -> String {
        self.resolved.decimal_sep.to_string()
    }

    /// The declared digit-group separator.
    #[getter]
    fn group_sep(&self) -> String {
        self.resolved.group_sep.to_string()
    }

    /// The bitwise OR of the lenience flags.
    #[getter]
    fn flags(&self) -> u32 {
        self.resolved.flags
    }

    /// The declared currency symbol — ``""`` when none is declared.
    #[getter]
    fn currency(&self) -> String {
        self.resolved.currency.as_str().to_string()
    }

    /// The 32 bytes this format crosses a C ABI as: decimal and group separators as code
    /// points, the flags and the symbol's length as little-endian ``u32``\ s, then the
    /// symbol's UTF-8, zero-padded to 16 — for a library that declares numeric columns
    /// across a C ABI of its own.
    #[getter]
    fn packed<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &core::RawNumFormat::from(self.resolved).to_le_bytes())
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let repr = |text: &str| -> PyResult<String> { PyString::new(py, text).repr()?.extract() };
        Ok(format!(
            "NumFormat({}, {}, {}, {})",
            repr(&self.resolved.decimal_sep.to_string())?,
            repr(&self.resolved.group_sep.to_string())?,
            self.resolved.flags,
            repr(self.resolved.currency.as_str())?
        ))
    }

    /// Bridges ``locale.localeconv()`` (or a dict shaped like it) to a declared format —
    /// ``decimal_point``, ``thousands_sep``, and ``currency_symbol``.
    ///
    /// A field the locale leaves empty takes its invariant default (``.`` decimal, ``,``
    /// group) unless the other separator already holds that character, in which case it
    /// takes the other of the pair — so a comma-decimal locale with no thousands separator
    /// groups on ``.`` rather than colliding. Two separators the locale itself declares
    /// equal are still the ``ValueError`` the constructor raises.
    #[staticmethod]
    #[pyo3(signature = (conv = None))]
    fn from_localeconv(py: Python<'_>, conv: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let conv = match conv {
            Some(conv) => conv,
            None => py.import("locale")?.call_method0("localeconv")?.cast_into::<PyDict>()?,
        };
        // None when the key is absent or its text is empty: the locale declares nothing.
        let field = |name: &str| -> PyResult<Option<char>> {
            match conv.get_item(name)? {
                Some(value) => Ok(value.extract::<String>()?.chars().next()),
                None => Ok(None),
            }
        };
        let (decimal, group) = match (field("decimal_point")?, field("thousands_sep")?) {
            (Some(decimal), Some(group)) => (decimal, group),
            (Some(decimal), None) => (decimal, if decimal == ',' { '.' } else { ',' }),
            (None, Some(group)) => (if group == '.' { ',' } else { '.' }, group),
            (None, None) => ('.', ','),
        };
        let currency = match conv.get_item("currency_symbol")? {
            Some(value) => value.extract::<String>()?,
            None => String::new(),
        };
        declare(decimal, group, core::NumFormat::ALL, &currency)
    }
}

/// The one place a format is built, so the constructor and the locale bridge refuse the
/// same caller bugs: equal separators, and a currency symbol the core cannot carry.
fn declare(decimal: char, group: char, flags: u32, currency: &str) -> PyResult<NumFormat> {
    if decimal == group {
        return Err(PyValueError::new_err(format!(
            "Decimal and group separators must differ; both are \"{decimal}\""
        )));
    }
    Ok(NumFormat {
        resolved: core::NumFormat::new(decimal, group, flags)
            .with_currency(currency_symbol(currency)?),
    })
}

/// The declared currency symbol: `""` declares none; anything else must be a valid
/// `CurrencySymbol` (1 to 16 UTF-8 bytes, no ASCII digit or whitespace), or it is a caller
/// bug raised here, at construction, the way equal separators are.
fn currency_symbol(text: &str) -> PyResult<core::CurrencySymbol> {
    if text.is_empty() {
        return Ok(core::CurrencySymbol::NONE);
    }
    core::CurrencySymbol::new(text).ok_or_else(|| {
        PyValueError::new_err(format!(
            "Currency symbol must be 1 to 16 UTF-8 bytes with no ASCII digit or whitespace; got \"{text}\""
        ))
    })
}

fn single_char(text: &str) -> PyResult<char> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(only), None) => Ok(only),
        _ => Err(PyValueError::new_err("Separators must be single characters")),
    }
}

/// Door input: str or bytes, each a zero-copy view into the caller's own object.
enum Text<'py> {
    Str(Bound<'py, PyString>),
    Bytes(Bound<'py, PyBytes>),
}

// Hand-written rather than `#[derive(FromPyObject)]`: the derived two-variant extractor
// tries `Str` first, and a failed variant is not free — it builds a `TypeError` with a
// formatted message and a cause chain, which the next variant's success then throws away.
// That was about a microsecond on every `bytes` input, ten times the cast itself (`"42"` at
// 116 ns against `b"42"` at 1,179 ns), on the path the corpus replay and every "zero-copy
// bytes" caller take. Checking the type tag directly costs nothing on either path.
impl<'py> FromPyObject<'_, 'py> for Text<'py> {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(text) = obj.cast::<PyString>() {
            Ok(Text::Str(text.to_owned()))
        } else if let Ok(bytes) = obj.cast::<PyBytes>() {
            Ok(Text::Bytes(bytes.to_owned()))
        } else {
            // PyO3 prefixes the argument name, so a door raises
            // "argument 'text': must be str or bytes".
            Err(PyTypeError::new_err("must be str or bytes"))
        }
    }
}

impl Text<'_> {
    // to_str() borrows the str's own cached UTF-8 with no copy. It is in the limited API
    // from 3.10, below this extension's floor (abi3-py311), so nothing here ever owns
    // the text.
    fn bytes(&self) -> PyResult<&[u8]> {
        match self {
            Text::Str(text) => Ok(text.to_str()?.as_bytes()),
            Text::Bytes(bytes) => Ok(bytes.as_bytes()),
        }
    }

    /// Presents a fault in the caller's own units: the core's byte span as-is for `bytes`
    /// input, remapped to code points for `str` input. An ASCII `str` needs no mapping and
    /// pays one length comparison (code points equal bytes); only a non-ASCII `str`, and
    /// only on the fault path, counts code points.
    fn present(&self, failed: core::Fault) -> PyResult<core::Fault> {
        let Text::Str(text) = self else {
            return Ok(failed);
        };
        let bytes = self.bytes()?;
        if bytes.len() == text.len()? {
            return Ok(failed);
        }
        let offset = (failed.offset as usize).min(bytes.len());
        let end = offset.saturating_add(failed.len as usize).min(bytes.len());
        Ok(core::Fault {
            reason: failed.reason,
            offset: code_points(&bytes[..offset]),
            len: code_points(&bytes[offset..end]),
        })
    }
}

/// Every code point starts with exactly one non-continuation byte.
fn code_points(bytes: &[u8]) -> u32 {
    bytes.iter().filter(|byte| **byte & 0xC0 != 0x80).count() as u32
}

fn fault(py: Python<'_>, failed: core::Fault) -> PyResult<Py<PyAny>> {
    let member = match failed.reason {
        core::Reason::Empty => &EMPTY,
        core::Reason::Malformed => &MALFORMED,
        core::Reason::OutOfRange => &OUT_OF_RANGE,
    };
    let reason = cached(py, member)?.clone().unbind();
    Ok(Py::new(py, Fault { reason, offset: failed.offset, length: failed.len })?.into_any())
}

fn verdict<'py, T>(
    py: Python<'py>,
    text: &Text<'py>,
    outcome: Result<T, core::Fault>,
    into: impl FnOnce(Python<'py>, T) -> PyResult<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    match outcome {
        Ok(value) => {
            let value = into(py, value)?;
            Ok(Py::new(py, Success { value })?.into_any())
        }
        Err(failed) => fault(py, text.present(failed)?),
    }
}

/// A typed door's verdict: the reason alone, presented as a fault with an empty span (there
/// is no text for one to index).
fn typed<'py, T>(
    py: Python<'py>,
    outcome: Result<T, core::Reason>,
    into: impl FnOnce(Python<'py>, T) -> PyResult<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    match outcome {
        Ok(value) => {
            let value = into(py, value)?;
            Ok(Py::new(py, Success { value })?.into_any())
        }
        Err(reason) => fault(py, core::Fault { reason, offset: 0, len: 0 }),
    }
}

// Each door's doc comment is its Python docstring — what `help(hypercast.cast_i32)` prints —
// so it is written in reStructuredText, and a test holds every door to having one.
macro_rules! numeric_doors {
    ($($(#[$doc:meta])* $door:ident => $core:ident),+ $(,)?) => {$(
        $(#[$doc])*
        #[pyfunction]
        fn $door(py: Python<'_>, text: Text<'_>, fmt: PyRef<'_, NumFormat>) -> PyResult<Py<PyAny>> {
            verdict(py, &text, core::$core(text.bytes()?, &fmt.resolved), |py, value| {
                Ok(value.into_pyobject(py)?.into_any().unbind())
            })
        }
    )+};
}

numeric_doors! {
    /// Casts integer text to a signed 8-bit value under the declared format.
    cast_i8 => cast_i8,
    /// Casts integer text to a signed 16-bit value under the declared format.
    cast_i16 => cast_i16,
    /// Casts integer text to a signed 32-bit value under the declared format.
    cast_i32 => cast_i32,
    /// Casts integer text to a signed 64-bit value under the declared format.
    cast_i64 => cast_i64,
    /// Casts integer text to an unsigned 8-bit value under the declared format.
    cast_u8 => cast_u8,
    /// Casts integer text to an unsigned 16-bit value under the declared format.
    cast_u16 => cast_u16,
    /// Casts integer text to an unsigned 32-bit value under the declared format.
    cast_u32 => cast_u32,
    /// Casts integer text to an unsigned 64-bit value — the true unsigned value, ``int``
    /// being unbounded — under the declared format.
    cast_u64 => cast_u64,
    /// Casts real text to an IEEE single (widened losslessly) under the declared format.
    cast_f32 => cast_f32,
    /// Casts real text to an IEEE double under the declared format.
    cast_f64 => cast_f64,
}

/// Casts decimal text under the declared format to an exact, canonical
/// ``decimal.Decimal`` — trailing fraction zeros trimmed, so ``"1.10"`` is ``Decimal('1.1')``;
/// never rounded.
#[pyfunction]
fn cast_decimal(py: Python<'_>, text: Text<'_>, fmt: PyRef<'_, NumFormat>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::cast_decimal(text.bytes()?, &fmt.resolved), decimal_value)
}

/// Reads a ``float`` as the exact ``decimal.Decimal`` it names: the shortest decimal that
/// rounds back to it, the digits a spreadsheet writes for it — ``0.1`` is
/// ``Decimal('0.1')``, not the binary fraction ``Decimal(0.1)`` spells out. NaN is
/// ``MALFORMED``; an infinity, past 96 bits or more than 28 places is ``OUT_OF_RANGE``.
#[pyfunction]
fn cast_decimal_from_float(py: Python<'_>, value: f64) -> PyResult<Py<PyAny>> {
    typed(py, core::decimal_from_f64(value), decimal_value)
}

fn decimal_value(py: Python<'_>, value: core::Decimal) -> PyResult<Py<PyAny>> {
    Ok(values()?.decimal(py, value)?.unbind())
}

/// This library's version as ``"major.minor.patch"``, decoded from the same packed
/// ``hypercast_version`` export every other binding probes.
#[pyfunction]
fn native_version() -> String {
    let packed = core::hypercast_version();
    format!("{}.{}.{}", packed >> 16, (packed >> 8) & 0xff, packed & 0xff)
}

/// Casts boolean text under the natural-language lexicon.
#[pyfunction]
fn cast_bool(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::cast_bool(text.bytes()?), |py, value| {
        Ok(value.into_pyobject(py)?.to_owned().into_any().unbind())
    })
}

/// Casts UUID text — every .NET ``Guid`` form plus ``urn:uuid:``-style prefixes — to a
/// ``uuid.UUID``.
#[pyfunction]
fn cast_uuid(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::cast_uuid(text.bytes()?), |py, bytes| {
        Ok(values()?.uuid(py, bytes)?.unbind())
    })
}

fn instant(py: Python<'_>, ts: core::Timestamp) -> PyResult<Py<PyAny>> {
    Ok(values()?.instant(py, ts)?.unbind())
}

/// Casts an RFC 3339 instant to an aware UTC ``datetime`` (microsecond truncation).
#[pyfunction]
fn cast_timestamp(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::cast_timestamp(text.bytes()?), instant)
}

/// Casts an integer Unix-epoch value under the declared ``UnixPrecision``.
#[pyfunction]
fn cast_unix(py: Python<'_>, text: Text<'_>, precision: u32) -> PyResult<Py<PyAny>> {
    let Some(precision) = core::UnixPrecision::from_code(precision) else {
        return Err(PyValueError::new_err("precision must be a UnixPrecision"));
    };
    verdict(py, &text, core::cast_unix(text.bytes()?, precision), instant)
}

/// Casts an Excel date serial under the declared ``ExcelEpoch``.
#[pyfunction]
fn cast_excel_serial(py: Python<'_>, text: Text<'_>, epoch: u32) -> PyResult<Py<PyAny>> {
    let Some(epoch) = core::ExcelEpoch::from_code(epoch) else {
        return Err(PyValueError::new_err("epoch must be an ExcelEpoch"));
    };
    verdict(py, &text, core::cast_excel_serial(text.bytes()?, epoch), instant)
}

/// Reads an Excel serial ``float`` under the declared ``ExcelEpoch`` as the naive
/// ``datetime`` it names — the twin of ``cast_excel_serial`` for a number a workbook reader
/// already holds. Zone-less, as the cell is.
#[pyfunction]
fn cast_excel_serial_from_float(py: Python<'_>, value: f64, epoch: u32) -> PyResult<Py<PyAny>> {
    let Some(epoch) = core::ExcelEpoch::from_code(epoch) else {
        return Err(PyValueError::new_err("epoch must be an ExcelEpoch"));
    };
    typed(py, core::excel_serial(value, epoch), civil_value)
}

/// Reads the fraction of an Excel serial ``float`` as a ``time`` of day (microsecond
/// truncation).
#[pyfunction]
fn cast_excel_time(py: Python<'_>, value: f64) -> PyResult<Py<PyAny>> {
    typed(py, core::excel_time(value), time_value)
}

/// Reads a ``float`` count of days as a ``timedelta`` (microsecond truncation toward zero).
#[pyfunction]
fn cast_excel_duration(py: Python<'_>, value: f64) -> PyResult<Py<PyAny>> {
    typed(py, core::excel_duration(value), duration_value)
}

fn date_value(py: Python<'_>, date: core::Date) -> PyResult<Py<PyAny>> {
    Ok(values()?.date(py, date)?.unbind())
}

/// Casts a calendar date: strict ISO ``yyyy-MM-dd`` with no order, the separated forms
/// under a declared ``DateOrder``.
#[pyfunction]
#[pyo3(signature = (text, order = None))]
fn cast_date(py: Python<'_>, text: Text<'_>, order: Option<u32>) -> PyResult<Py<PyAny>> {
    let Some(order) = order else {
        return verdict(py, &text, core::cast_date(text.bytes()?), date_value);
    };
    let Some(order) = core::DateOrder::from_code(order) else {
        return Err(PyValueError::new_err("order must be a DateOrder"));
    };
    verdict(py, &text, core::cast_date_ordered(text.bytes()?, order), date_value)
}

/// Casts a zone-less civil date-time under a declared ``DateOrder`` to a naive
/// ``datetime``.
#[pyfunction]
fn cast_datetime(py: Python<'_>, text: Text<'_>, order: u32) -> PyResult<Py<PyAny>> {
    let Some(order) = core::DateOrder::from_code(order) else {
        return Err(PyValueError::new_err("order must be a DateOrder"));
    };
    verdict(py, &text, core::cast_datetime(text.bytes()?, order), civil_value)
}

fn civil_value(py: Python<'_>, civil: core::CivilDateTime) -> PyResult<Py<PyAny>> {
    Ok(values()?.civil(py, civil)?.unbind())
}

/// Casts an ISO 24-hour time-of-day to a ``time`` (microsecond truncation).
#[pyfunction]
fn cast_time(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::cast_time(text.bytes()?), time_value)
}

fn time_value(py: Python<'_>, nanos: u64) -> PyResult<Py<PyAny>> {
    Ok(values()?.time(py, nanos)?.unbind())
}

/// Casts a duration (ISO 8601, invariant colon form, or protobuf JSON seconds) to a
/// ``timedelta`` (microsecond truncation toward zero).
#[pyfunction]
fn cast_duration(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::cast_duration(text.bytes()?), duration_value)
}

fn duration_value(py: Python<'_>, span: core::Duration) -> PyResult<Py<PyAny>> {
    Ok(values()?.duration(py, span)?.unbind())
}

/// Hands the package's own `CastFailure` IntEnum (plus `uuid.UUID` and `decimal.Decimal`)
/// to this backend so faults carry the exact members callers compare with `is`.
#[pyfunction]
fn _bind(py: Python<'_>, cast_failure: Bound<'_, PyAny>) -> PyResult<()> {
    let _ = EMPTY.set(cast_failure.getattr("EMPTY")?.unbind());
    let _ = MALFORMED.set(cast_failure.getattr("MALFORMED")?.unbind());
    let _ = OUT_OF_RANGE.set(cast_failure.getattr("OUT_OF_RANGE")?.unbind());
    let _ = VALUES.set(crate::python::Values::import(py)?);
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
    m.add_function(wrap_pyfunction!(cast_decimal, m)?)?;
    m.add_function(wrap_pyfunction!(cast_uuid, m)?)?;
    m.add_function(wrap_pyfunction!(cast_timestamp, m)?)?;
    m.add_function(wrap_pyfunction!(cast_unix, m)?)?;
    m.add_function(wrap_pyfunction!(cast_excel_serial, m)?)?;
    m.add_function(wrap_pyfunction!(cast_date, m)?)?;
    m.add_function(wrap_pyfunction!(cast_datetime, m)?)?;
    m.add_function(wrap_pyfunction!(cast_time, m)?)?;
    m.add_function(wrap_pyfunction!(cast_duration, m)?)?;
    m.add_function(wrap_pyfunction!(cast_decimal_from_float, m)?)?;
    m.add_function(wrap_pyfunction!(cast_excel_serial_from_float, m)?)?;
    m.add_function(wrap_pyfunction!(cast_excel_time, m)?)?;
    m.add_function(wrap_pyfunction!(cast_excel_duration, m)?)?;
    m.add_function(wrap_pyfunction!(native_version, m)?)?;
    m.add_function(wrap_pyfunction!(_bind, m)?)?;
    Ok(())
}
