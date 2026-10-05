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
use pyo3::ffi;
use pyo3::intern;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDate, PyDateTime, PyDelta, PyDict, PyString, PyTzInfo};

use crate as core;

/// Cached Python-side companions: the three CastFailure members (the package's own
/// IntEnum, handed over via `_bind`), `uuid.UUID`, and `decimal.Decimal`.
static EMPTY: OnceLock<Py<PyAny>> = OnceLock::new();
static MALFORMED: OnceLock<Py<PyAny>> = OnceLock::new();
static OUT_OF_RANGE: OnceLock<Py<PyAny>> = OnceLock::new();
static UUID_CLASS: OnceLock<Py<PyAny>> = OnceLock::new();
// The fastuuid-style constructor HyperUuid pinned: `UUID.__new__` plus `object.__setattr__`
// of the `int` and `is_safe` slots, skipping `UUID.__init__` — whose validation the core
// already performed on the text, and whose cost was the whole of this door's carrier.
static DATETIME_CLASS: OnceLock<Py<PyAny>> = OnceLock::new();
static DATE_CLASS: OnceLock<Py<PyAny>> = OnceLock::new();
static TIME_CLASS: OnceLock<Py<PyAny>> = OnceLock::new();
static UTC: OnceLock<Py<PyAny>> = OnceLock::new();
static DURATION_ANCHOR: OnceLock<Py<PyAny>> = OnceLock::new();
static IS_SAFE_UNKNOWN: OnceLock<Py<PyAny>> = OnceLock::new();
// `decimal.Decimal` — the exact host type the decimal door builds, from the core's
// canonical text through the C `_decimal` constructor.
static DECIMAL_CLASS: OnceLock<Py<PyAny>> = OnceLock::new();

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

/// The core's canonical decimal text (`Display`), rendered on the stack — at most a sign,
/// 29 digits and a point — so the host `Decimal` comes straight out of `_decimal`'s C string
/// constructor with nothing allocated on this side. Measured against the
/// `(sign, digits, exponent)` tuple constructor (which has to build a Python int per digit):
/// the text path was faster on every shape tried, `1234.50` at 405 ns vs 556 ns and the
/// 29-digit ceiling at 788 ns vs 1189 ns (CPython 3.14, linux-arm64, timeit best of five).
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
    use std::fmt::Write;
    let mut canonical = Canonical { buf: [0; 48], len: 0 };
    write!(canonical, "{value}")
        .map_err(|_| PyValueError::new_err("hypercast: decimal text overflowed its buffer"))?;
    let text = std::str::from_utf8(&canonical.buf[..canonical.len])
        .map_err(|_| PyValueError::new_err("hypercast: decimal text was not UTF-8"))?;
    Ok(cached(py, &DECIMAL_CLASS)?.call1((text,))?.unbind())
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
        // The core has already validated the text; `UUID.__init__` would only re-check a
        // value it cannot reject, so the instance is built the way `uuid.UUID` itself
        // stores it — the 128-bit `int` slot, big-endian from the RFC-ordered bytes, and
        // `is_safe` left at `SafeUUID.unknown`, exactly what `UUID(bytes=...)` would set.
        //
        // Through the C API's own entry points rather than Python callables:
        // PyType_GenericAlloc and PyObject_GenericSetAttr are what `object.__new__` and
        // `object.__setattr__` do underneath, without a call, an argument tuple or a fresh
        // `str` per attribute name. Both are in the stable ABI.
        let class = cached(py, &UUID_CLASS)?;
        let value = int_from_be_bytes(py, bytes)?;
        let is_safe = cached(py, &IS_SAFE_UNKNOWN)?;
        // SAFETY: UUID_CLASS is a type object (bound in _bind); both calls check their
        // arguments and report failure by return value with an exception set.
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
            Ok(instance.unbind())
        }
    })
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

/// 0001-01-01, the datetime a duration is measured from (see `cast_duration`): its day number
/// on the Unix epoch's count, and how many whole days past it a datetime can still stand
/// (9999-12-31 is day 3,652,058 from it).
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

fn instant<'py>(py: Python<'py>, ts: core::Timestamp) -> PyResult<Py<PyAny>> {
    let days = ts.seconds.div_euclid(86_400);
    let second_of_day = ts.seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let (hour, rest) = (second_of_day / 3_600, second_of_day % 3_600);
    let (minute, second) = (rest / 60, rest % 60);
    let micros = (ts.nanos / 1_000) as u32;
    if packs(year) {
        let state =
            packed_datetime(year, month, day, hour as u8, minute as u8, second as u8, micros);
        return Ok(cached(py, &DATETIME_CLASS)?
            .call1((PyBytes::new(py, &state), cached(py, &UTC)?))?
            .unbind());
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
    .into_any()
    .unbind())
}

/// Casts an RFC 3339 instant to an aware UTC ``datetime`` (microsecond truncation).
#[pyfunction]
fn cast_timestamp(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::cast_timestamp(text.bytes()?), instant)
}

/// Casts an integer Unix-epoch value under the declared ``UnixPrecision``.
#[pyfunction]
fn cast_unix(py: Python<'_>, text: Text<'_>, precision: u32) -> PyResult<Py<PyAny>> {
    let precision = match precision {
        1 => core::UnixPrecision::Seconds,
        2 => core::UnixPrecision::Millis,
        3 => core::UnixPrecision::Micros,
        4 => core::UnixPrecision::Nanos,
        _ => return Err(PyValueError::new_err("precision must be a UnixPrecision")),
    };
    verdict(py, &text, core::cast_unix(text.bytes()?, precision), instant)
}

/// Casts an Excel date serial under the declared ``ExcelEpoch``.
#[pyfunction]
fn cast_excel_serial(py: Python<'_>, text: Text<'_>, epoch: u32) -> PyResult<Py<PyAny>> {
    let epoch = match epoch {
        1 => core::ExcelEpoch::Y1900,
        2 => core::ExcelEpoch::Y1904,
        _ => return Err(PyValueError::new_err("epoch must be an ExcelEpoch")),
    };
    verdict(py, &text, core::cast_excel_serial(text.bytes()?, epoch), instant)
}

/// Reads an Excel serial ``float`` under the declared ``ExcelEpoch`` as the naive
/// ``datetime`` it names — the twin of ``cast_excel_serial`` for a number a workbook reader
/// already holds. Zone-less, as the cell is.
#[pyfunction]
fn cast_excel_serial_from_float(py: Python<'_>, value: f64, epoch: u32) -> PyResult<Py<PyAny>> {
    let epoch = match epoch {
        1 => core::ExcelEpoch::Y1900,
        2 => core::ExcelEpoch::Y1904,
        _ => return Err(PyValueError::new_err("epoch must be an ExcelEpoch")),
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
    let year = i32::from(date.year);
    if packs(year) {
        let state = packed_date(year, date.month, date.day);
        return Ok(cached(py, &DATE_CLASS)?.call1((PyBytes::new(py, &state),))?.unbind());
    }
    Ok(PyDate::new(py, year, date.month, date.day)?.into_any().unbind())
}

/// Casts a calendar date: strict ISO ``yyyy-MM-dd`` with no order, the separated forms
/// under a declared ``DateOrder``.
#[pyfunction]
#[pyo3(signature = (text, order = None))]
fn cast_date(py: Python<'_>, text: Text<'_>, order: Option<u32>) -> PyResult<Py<PyAny>> {
    let Some(order) = order else {
        return verdict(py, &text, core::cast_date(text.bytes()?), date_value);
    };
    let order = match order {
        1 => core::DateOrder::YearMonthDay,
        2 => core::DateOrder::MonthDayYear,
        3 => core::DateOrder::DayMonthYear,
        _ => return Err(PyValueError::new_err("order must be a DateOrder")),
    };
    verdict(py, &text, core::cast_date_ordered(text.bytes()?, order), date_value)
}

/// Casts a zone-less civil date-time under a declared ``DateOrder`` to a naive
/// ``datetime``.
#[pyfunction]
fn cast_datetime(py: Python<'_>, text: Text<'_>, order: u32) -> PyResult<Py<PyAny>> {
    let order = match order {
        1 => core::DateOrder::YearMonthDay,
        2 => core::DateOrder::MonthDayYear,
        3 => core::DateOrder::DayMonthYear,
        _ => return Err(PyValueError::new_err("order must be a DateOrder")),
    };
    verdict(py, &text, core::cast_datetime(text.bytes()?, order), civil_value)
}

fn civil_value(py: Python<'_>, civil: core::CivilDateTime) -> PyResult<Py<PyAny>> {
    // Naive datetime — the text named no zone, so the value carries none; fusing a
    // zone is the caller's job. Sub-microsecond nanoseconds truncate (Python's ceiling).
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
        return Ok(cached(py, &DATETIME_CLASS)?.call1((PyBytes::new(py, &state),))?.unbind());
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
    .into_any()
    .unbind())
}

/// Casts an ISO 24-hour time-of-day to a ``time`` (microsecond truncation).
#[pyfunction]
fn cast_time(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::cast_time(text.bytes()?), time_value)
}

fn time_value(py: Python<'_>, nanos: u64) -> PyResult<Py<PyAny>> {
    let (second_of_day, nano) = (nanos / 1_000_000_000, nanos % 1_000_000_000);
    let (hour, rest) = (second_of_day / 3_600, second_of_day % 3_600);
    let (minute, second) = (rest / 60, rest % 60);
    let state = packed_time(hour as u8, minute as u8, second as u8, (nano / 1_000) as u32);
    Ok(cached(py, &TIME_CLASS)?.call1((PyBytes::new(py, &state),))?.unbind())
}

/// Casts a duration (ISO 8601, invariant colon form, or protobuf JSON seconds) to a
/// ``timedelta`` (microsecond truncation toward zero).
#[pyfunction]
fn cast_duration(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::cast_duration(text.bytes()?), duration_value)
}

fn duration_value(py: Python<'_>, span: core::Duration) -> PyResult<Py<PyAny>> {
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
        let anchor = cached(py, &DURATION_ANCHOR)?;
        let moved = cached(py, &DATETIME_CLASS)?.call1((PyBytes::new(py, &state),))?;
        let negative = span.seconds < 0 || span.nanos < 0;
        let (left, right) = if negative { (anchor, &moved) } else { (&moved, anchor) };
        return Ok(left.sub(right)?.unbind());
    }
    // Truncate sub-microsecond digits toward zero on both signs, matching every other
    // binding's truncation; PyDelta normalizes the mixed-sign pieces.
    let nanos = i64::from(span.nanos);
    let micros = if nanos >= 0 { nanos / 1_000 } else { -((-nanos) / 1_000) };
    let days = span.seconds.div_euclid(86_400);
    let seconds = span.seconds.rem_euclid(86_400);
    Ok(PyDelta::new(py, days as i32, seconds as i32, micros as i32, true)?.into_any().unbind())
}

/// Hands the package's own `CastFailure` IntEnum (plus `uuid.UUID` and `decimal.Decimal`)
/// to this backend so faults carry the exact members callers compare with `is`.
#[pyfunction]
fn _bind(py: Python<'_>, cast_failure: Bound<'_, PyAny>) -> PyResult<()> {
    let _ = EMPTY.set(cast_failure.getattr("EMPTY")?.unbind());
    let _ = MALFORMED.set(cast_failure.getattr("MALFORMED")?.unbind());
    let _ = OUT_OF_RANGE.set(cast_failure.getattr("OUT_OF_RANGE")?.unbind());
    let uuid_module = py.import("uuid")?;
    let _ = UUID_CLASS.set(uuid_module.getattr("UUID")?.unbind());
    let datetime_module = py.import("datetime")?;
    let _ = DATETIME_CLASS.set(datetime_module.getattr("datetime")?.unbind());
    let _ = DATE_CLASS.set(datetime_module.getattr("date")?.unbind());
    let _ = TIME_CLASS.set(datetime_module.getattr("time")?.unbind());
    let _ = UTC.set(datetime_module.getattr("timezone")?.getattr("utc")?.unbind());
    let _ = DURATION_ANCHOR.set(datetime_module.getattr("datetime")?.call1((1, 1, 1))?.unbind());
    let _ = IS_SAFE_UNKNOWN.set(uuid_module.getattr("SafeUUID")?.getattr("unknown")?.unbind());
    let _ = DECIMAL_CLASS.set(py.import("decimal")?.getattr("Decimal")?.unbind());
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
