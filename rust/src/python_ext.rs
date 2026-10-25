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
//! floor up with the GIL, and abi3t-py315 (or per version, for 3.14t) for free-threaded
//! CPython.
//!
//! Free-threading: PyO3 0.30 declares the module needs no GIL, so on 3.14t/3.15t the doors
//! run truly in parallel, and nothing here leans on the GIL for exclusion. The pyclasses are
//! `frozen`, so a shared instance is only ever read. The statics below are `OnceLock`s set
//! once by `_bind` during the package's import (which CPython's import lock runs on one
//! thread) and only read after; `set` never runs Python under the lock, so it cannot
//! deadlock against another thread's attach. A door borrows its argument's bytes in place
//! (`Text::bytes`), which is sound without the GIL because both `str` and `bytes` are
//! immutable and the borrow lives no longer than the owned reference the door holds.

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
    const TRAILING_SIGN: u32 = core::NumFormat::TRAILING_SIGN;
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
    #[pyo3(signature = (
        decimal_sep, group_sep, flags, currency = "", *,
        currency_decimal_sep = "", currency_group_sep = "", minus_sign = "", plus_sign = "",
        percent = "", digit_zero = ""
    ))]
    #[allow(clippy::too_many_arguments)] // The constructor's signature is the API.
    fn new(
        decimal_sep: &str,
        group_sep: &str,
        flags: u32,
        currency: &str,
        currency_decimal_sep: &str,
        currency_group_sep: &str,
        minus_sign: &str,
        plus_sign: &str,
        percent: &str,
        digit_zero: &str,
    ) -> PyResult<Self> {
        declare(Declared {
            decimal_sep,
            group_sep,
            flags,
            currency,
            currency_decimal_sep,
            currency_group_sep,
            minus_sign,
            plus_sign,
            percent,
            digit_zero,
        })
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

    /// The decimal separator a currency amount is read with — ``""`` when it is
    /// ``decimal_sep``.
    #[getter]
    fn currency_decimal_sep(&self) -> String {
        self.resolved.currency_decimal_sep.to_string()
    }

    /// The digit-group separator a currency amount is read with — ``""`` when it is
    /// ``group_sep``.
    #[getter]
    fn currency_group_sep(&self) -> String {
        self.resolved.currency_group_sep.to_string()
    }

    /// The further spelling of the minus sign read beside ASCII ``-`` — ``""`` when none is
    /// declared.
    #[getter]
    fn minus_sign(&self) -> String {
        self.resolved.minus_sign.to_string()
    }

    /// The further spelling of the plus sign read beside ASCII ``+`` — ``""`` when none is
    /// declared.
    #[getter]
    fn plus_sign(&self) -> String {
        self.resolved.plus_sign.to_string()
    }

    /// The further spelling of the percent symbol read beside ASCII ``%`` — ``""`` when none
    /// is declared.
    #[getter]
    fn percent(&self) -> String {
        self.resolved.percent.to_string()
    }

    /// The zero of the native digits read beside ASCII ``0``–``9`` — ``""`` when none are
    /// declared.
    #[getter]
    fn digit_zero(&self) -> String {
        self.resolved.digits.zero().map(String::from).unwrap_or_default()
    }

    /// The 92 bytes this format crosses a C ABI as — the core's own size-tagged layout, which
    /// it reads in place: the size (``92``), the flags and the native digits' zero code point
    /// (``0`` for none) as little-endian ``u32``\ s, then each symbol as its UTF-8 followed by
    /// its byte length, zero-padded — 8 bytes plus the length for the decimal, group,
    /// currency-decimal and currency-group separators, the minus and plus signs and the
    /// percent symbol, in that order, and 16 plus the length for the currency symbol — for
    /// a library that declares numeric columns across a C ABI of its own.
    #[getter]
    fn packed<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &core::RawNumFormat::from(self.resolved).to_le_bytes())
    }

    /// The constructor call that builds this format again: the four positional arguments
    /// always, then only the keyword arguments that declare something.
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let repr = |text: &str| -> PyResult<String> { PyString::new(py, text).repr()?.extract() };
        let format = &self.resolved;
        let mut call = format!(
            "NumFormat({}, {}, {}, {}",
            repr(format.decimal_sep.as_str())?,
            repr(format.group_sep.as_str())?,
            format.flags,
            repr(format.currency.as_str())?
        );
        for (name, symbol) in [
            ("currency_decimal_sep", format.currency_decimal_sep.as_str()),
            ("currency_group_sep", format.currency_group_sep.as_str()),
            ("minus_sign", format.minus_sign.as_str()),
            ("plus_sign", format.plus_sign.as_str()),
            ("percent", format.percent.as_str()),
            ("digit_zero", &self.digit_zero()),
        ] {
            if !symbol.is_empty() {
                call.push_str(&format!(", {name}={}", repr(symbol)?));
            }
        }
        call.push(')');
        Ok(call)
    }

    /// Bridges ``locale.localeconv()`` (or a dict shaped like it) to a declared format:
    /// ``decimal_point`` and ``thousands_sep`` as the separators, ``currency_symbol``,
    /// ``mon_decimal_point`` and ``mon_thousands_sep`` as the separators a currency amount
    /// is read with, ``negative_sign`` and ``positive_sign`` as further spellings of the
    /// signs (U+2212 for Estonian and Basque, for instance — ASCII ``-`` and ``+`` are read
    /// whatever is declared), and an ``n_sign_posn`` of ``2`` or ``4`` (the sign after the
    /// amount) as ``TRAILING_SIGN``. ``localeconv()`` carries no percent symbol and no
    /// native digits, so those stay undeclared: ASCII ``%`` and ``0``–``9``.
    ///
    /// A separator the locale leaves empty takes its invariant default (``.`` decimal, ``,``
    /// group) unless the other separator already holds that character, in which case it
    /// takes the other of the pair — so a comma-decimal locale with no thousands separator
    /// groups on ``.`` rather than colliding; an empty currency separator keeps the number
    /// separator it stands in for, by the same rule. Two separators the locale itself
    /// declares equal are still the ``ValueError`` the constructor raises. Every lenience is
    /// on, radix prefixes and percent included; narrow the flags, or construct a
    /// ``NumFormat`` outright, for anything stricter.
    #[staticmethod]
    #[pyo3(signature = (conv = None))]
    fn from_localeconv(py: Python<'_>, conv: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let conv = match conv {
            Some(conv) => conv,
            None => py.import("locale")?.call_method0("localeconv")?.cast_into::<PyDict>()?,
        };
        // Empty when the key is absent or its text is empty: the locale declares nothing.
        let field = |name: &str| -> PyResult<String> {
            match conv.get_item(name)? {
                Some(value) => value.extract::<String>(),
                None => Ok(String::new()),
            }
        };
        let (decimal_sep, group_sep) =
            separator_pair(&field("decimal_point")?, &field("thousands_sep")?, (".", ","));
        // The currency pair defaults to the number pair rather than the invariant one, and
        // stays undeclared when the locale declares nothing — the core then reads a currency
        // amount with the number separators, which is what an empty pair means.
        let (currency_decimal_sep, currency_group_sep) = {
            let (decimal, group) = (field("mon_decimal_point")?, field("mon_thousands_sep")?);
            if decimal.is_empty() && group.is_empty() {
                (String::new(), String::new())
            } else {
                separator_pair(&decimal, &group, (&decimal_sep, &group_sep))
            }
        };
        let currency = field("currency_symbol")?;
        // The ASCII signs are always read, so declaring them again would say nothing.
        let sign = |name: &str| -> PyResult<String> {
            let sign = field(name)?;
            Ok(if sign == "-" || sign == "+" { String::new() } else { sign })
        };
        let (minus_sign, plus_sign) = (sign("negative_sign")?, sign("positive_sign")?);
        let mut flags = core::NumFormat::ALL;
        if let Some(position) = conv.get_item("n_sign_posn")?
            && matches!(position.extract::<i64>(), Ok(2 | 4))
        {
            flags |= core::NumFormat::TRAILING_SIGN;
        }
        declare(Declared {
            decimal_sep: &decimal_sep,
            group_sep: &group_sep,
            flags,
            currency: &currency,
            currency_decimal_sep: &currency_decimal_sep,
            currency_group_sep: &currency_group_sep,
            minus_sign: &minus_sign,
            plus_sign: &plus_sign,
            percent: "",
            digit_zero: "",
        })
    }
}

/// A locale's separator pair as the bridge reads it: a field the locale leaves empty takes
/// its default unless the other separator already holds that text, in which case it takes
/// the other default — a comma-decimal locale with no thousands separator groups on `.`.
/// Two the locale itself declares equal are left equal, for `declare` to refuse.
fn separator_pair(decimal: &str, group: &str, defaults: (&str, &str)) -> (String, String) {
    let (default_decimal, default_group) = defaults;
    let (decimal, group) = match (decimal.is_empty(), group.is_empty()) {
        (false, false) => (decimal, group),
        (false, true) => {
            (decimal, if decimal == default_group { default_decimal } else { default_group })
        }
        (true, false) => {
            (if group == default_decimal { default_group } else { default_decimal }, group)
        }
        (true, true) => (default_decimal, default_group),
    };
    (decimal.to_string(), group.to_string())
}

/// What a caller declares, as text: the constructor's arguments, and what the locale bridge
/// reads out of `localeconv()`.
struct Declared<'a> {
    decimal_sep: &'a str,
    group_sep: &'a str,
    flags: u32,
    currency: &'a str,
    currency_decimal_sep: &'a str,
    currency_group_sep: &'a str,
    minus_sign: &'a str,
    plus_sign: &'a str,
    percent: &'a str,
    digit_zero: &'a str,
}

/// The one place a format is built, so the constructor and the locale bridge refuse the
/// same caller bugs — the C ABI's own contract rules, raised here at construction so a
/// format that exists is one the core reads: a separator that is not 1 to 8 UTF-8 bytes,
/// equal separators (the number pair, or the pair a currency amount is read with), a sign or
/// percent symbol with whitespace in it, a currency symbol the core cannot carry, a digit
/// zero that starts no digit block.
fn declare(declared: Declared<'_>) -> PyResult<NumFormat> {
    let decimal_sep = separator("Decimal separator", declared.decimal_sep)?;
    let group_sep = separator("Group separator", declared.group_sep)?;
    if decimal_sep == group_sep {
        return Err(PyValueError::new_err(format!(
            "Decimal and group separators must differ; both are \"{}\"",
            declared.decimal_sep
        )));
    }
    let currency_decimal_sep =
        optional("Currency decimal separator", declared.currency_decimal_sep, false)?;
    let currency_group_sep =
        optional("Currency group separator", declared.currency_group_sep, false)?;
    let in_force =
        |own: core::Symbol<8>, number: core::Symbol<8>| if own.is_empty() { number } else { own };
    let (decimal, group) =
        (in_force(currency_decimal_sep, decimal_sep), in_force(currency_group_sep, group_sep));
    if decimal == group {
        return Err(PyValueError::new_err(format!(
            "Currency decimal and group separators must differ; both are \"{decimal}\""
        )));
    }
    let resolved = core::NumFormat::with_separators(decimal_sep, group_sep, declared.flags)
        .with_currency(currency_symbol(declared.currency)?)
        .with_currency_separators(currency_decimal_sep, currency_group_sep)
        .with_signs(
            optional("Minus sign", declared.minus_sign, true)?,
            optional("Plus sign", declared.plus_sign, true)?,
        )
        .with_percent(optional("Percent symbol", declared.percent, true)?)
        .with_digits(native_digits(declared.digit_zero)?);
    debug_assert!(resolved.check(), "every contract rule is raised above");
    Ok(NumFormat { resolved })
}

/// A number separator: 1 to 8 UTF-8 bytes with no ASCII digit, the core's own rule, or a
/// caller bug raised at construction.
fn separator(what: &str, text: &str) -> PyResult<core::Symbol<8>> {
    core::Symbol::new(text).ok_or_else(|| {
        PyValueError::new_err(format!(
            "{what} must be 1 to 8 UTF-8 bytes with no ASCII digit; got \"{text}\""
        ))
    })
}

/// A symbol a caller need not declare: `""` declares none; anything else is a `separator`,
/// and with `flush` (a sign, the percent symbol — read against the digits) one with no
/// ASCII whitespace in it either.
fn optional(what: &str, text: &str, flush: bool) -> PyResult<core::Symbol<8>> {
    if text.is_empty() {
        return Ok(core::Symbol::NONE);
    }
    if flush && text.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(PyValueError::new_err(format!(
            "{what} must be 1 to 8 UTF-8 bytes with no ASCII digit or whitespace; got \"{text}\""
        )));
    }
    separator(what, text)
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

/// The declared native digits: `""` declares none; anything else is the one character the
/// ten digits start from, which must start a Unicode digit block (not ASCII, and the ten
/// code points from it sharing one UTF-8 prefix — every block Unicode defines does).
fn native_digits(text: &str) -> PyResult<core::NativeDigits> {
    if text.is_empty() {
        return Ok(core::NativeDigits::NONE);
    }
    let mut chars = text.chars();
    if let (Some(zero), None) = (chars.next(), chars.next())
        && let Some(digits) = core::NativeDigits::new(zero)
    {
        return Ok(digits);
    }
    Err(PyValueError::new_err(format!(
        "Digit zero must be the one non-ASCII character a Unicode digit block starts at; got \"{text}\""
    )))
}

/// A day-period designator for ``cast_datetime``: `""` declares none; anything else is 1 to
/// 16 UTF-8 bytes with no ASCII digit and no ASCII whitespace at either end (inside is fine:
/// Spanish ``a. m.``), or a caller bug raised at the call, the way a format's rules are at
/// construction.
fn designator(what: &str, text: &str) -> PyResult<core::Symbol<16>> {
    if text.is_empty() {
        return Ok(core::Symbol::NONE);
    }
    let edges = text.bytes().next().is_some_and(|byte| byte.is_ascii_whitespace())
        || text.bytes().last().is_some_and(|byte| byte.is_ascii_whitespace());
    if !edges && let Some(symbol) = core::Symbol::new(text) {
        return Ok(symbol);
    }
    Err(PyValueError::new_err(format!(
        "Day period {what} must be 1 to 16 UTF-8 bytes with no ASCII digit and no whitespace at either end; got \"{text}\""
    )))
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
    // the text. No GIL is needed to hold either slice: a str and a bytes never change once
    // built (a free-threaded CPython fills a str's UTF-8 cache once, thread-safely,
    // and never frees it before the str), and `self` holds a strong reference throughout.
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

/// Casts one character — the text verbatim when it is exactly one scalar, otherwise a declared
/// code point (``65``, ``U+0041``, ``0x41``, ``&H41``, ``&#65;``, ``&#x41;``) — to a
/// one-character ``str``. Past ``U+10FFFF`` or a surrogate is ``OUT_OF_RANGE``.
#[pyfunction]
fn cast_char(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::cast_char(text.bytes()?), |py, value| {
        Ok(value.into_pyobject(py)?.into_any().unbind())
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
/// ``datetime``. The day periods ``am`` and ``pm`` declare a culture's designators
/// (``"vorm."``/``"nachm."``, ``"오전"``/``"오후"``), read exactly, after the clock or
/// ahead of it, beside the English ``AM``/``PM``, which are read whatever is declared;
/// each is 1 to 16 UTF-8 bytes with no ASCII digit and no whitespace at either end, or a
/// ``ValueError``, a caller bug like a malformed ``NumFormat``.
#[pyfunction]
#[pyo3(signature = (text, order, *, am = "", pm = ""))]
fn cast_datetime(
    py: Python<'_>,
    text: Text<'_>,
    order: u32,
    am: &str,
    pm: &str,
) -> PyResult<Py<PyAny>> {
    let Some(order) = core::DateOrder::from_code(order) else {
        return Err(PyValueError::new_err("order must be a DateOrder"));
    };
    // Nothing declared takes the core's own English door, where the designators are a
    // constant the parser folds away; the declared path pays for its two checks.
    if am.is_empty() && pm.is_empty() {
        return verdict(py, &text, core::cast_datetime(text.bytes()?, order), civil_value);
    }
    let periods = core::DayPeriods::new(designator("am", am)?, designator("pm", pm)?);
    verdict(py, &text, core::cast_datetime_with(text.bytes()?, order, &periods), civil_value)
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
//
// `gil_used = false` is PyO3 0.30's default; it is spelled out because it is the whole
// free-threading claim: a free-threaded interpreter reads it at import and keeps the GIL
// off, where a module that leaves it unsaid in some future PyO3 would import with the GIL
// switched back on and a RuntimeWarning. test_free_threading.py checks the GIL stays off.
#[pymodule(gil_used = false)]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Success>()?;
    m.add_class::<Fault>()?;
    m.add_class::<NumFormat>()?;
    m.add_function(wrap_pyfunction!(cast_bool, m)?)?;
    m.add_function(wrap_pyfunction!(cast_char, m)?)?;
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
