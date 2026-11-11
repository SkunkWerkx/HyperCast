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

use std::cell::OnceCell;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::mem::MaybeUninit;
use std::sync::OnceLock;

use pyo3::exceptions::{
    PyOverflowError, PyRuntimeError, PyTypeError, PyUnicodeEncodeError, PyValueError,
};
use pyo3::intern;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyFloat, PyInt, PyString, PyTuple};

use crate as core;
use crate::checked::{
    CheckOutcome, judge_numeric_detailed, judge_numeric_from_f64_detailed, judge_text_detailed,
};
use crate::constraints::{Entries, declared_decimal};

/// Cached Python-side companions: the three CastFailure members (the package's own
/// IntEnum, handed over via `_bind`), and the objects every value is built with
/// ([`crate::python::Values`], shared with any extension that presents the core's values).
static EMPTY: OnceLock<Py<PyAny>> = OnceLock::new();
static MALFORMED: OnceLock<Py<PyAny>> = OnceLock::new();
static OUT_OF_RANGE: OnceLock<Py<PyAny>> = OnceLock::new();
static VALUES: OnceLock<crate::python::Values> = OnceLock::new();
/// The package's own `Door` and `Rounding` IntEnums and `decimal.Decimal`, handed over by
/// `_bind` like `CastFailure`'s members: what a declaration's getters answer with, and the type
/// a bound may be.
static DOOR: OnceLock<Py<PyAny>> = OnceLock::new();
static ROUNDING: OnceLock<Py<PyAny>> = OnceLock::new();
static DECIMAL: OnceLock<Py<PyAny>> = OnceLock::new();
/// The package's own `Detail` IntEnum, which a fault's `detail` answers with.
static DETAIL: OnceLock<Py<PyAny>> = OnceLock::new();

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
///
/// A door's fault also carries a ``detail``: what failed, finer than the reason, a
/// :class:`Detail` (``detail_code`` is its raw number, kept when this binding names no member
/// for it). A ``Fault`` built by hand has none unless ``detail_code`` is given. The detail is
/// beside the fault, never part of it: equality, ``__match_args__`` and the three positional
/// arguments are the reason, offset and length alone.
#[pyclass(frozen, module = "hypercast")]
struct Fault {
    #[pyo3(get)]
    reason: Py<PyAny>,
    #[pyo3(get)]
    offset: u32,
    #[pyo3(get)]
    length: u32,
    /// The detail's raw code, or `None` for a fault nobody classified.
    #[pyo3(get)]
    detail_code: Option<u32>,
}

#[pymethods]
impl Fault {
    #[allow(non_upper_case_globals)]
    #[classattr]
    const __match_args__: (&'static str, &'static str, &'static str) =
        ("reason", "offset", "length");

    #[new]
    #[pyo3(signature = (reason, offset, length, *, detail_code = None))]
    fn new(
        reason: Py<PyAny>,
        offset: u32,
        length: u32,
        #[pyo3(from_py_with = optional_detail_code)] detail_code: Option<u32>,
    ) -> Self {
        Fault { reason, offset, length, detail_code }
    }

    /// What failed, as a :class:`Detail` (``Detail.UNKNOWN`` for a code this version names no
    /// member for, which ``detail_code`` keeps), or ``None`` when the fault carries no detail.
    #[getter]
    fn detail(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let Some(code) = self.detail_code else {
            return Ok(None);
        };
        Ok(Some(cached(py, &DETAIL)?.call_method1(intern!(py, "from_code"), (code,))?.unbind()))
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
        let mut text = format!(
            "Fault(reason={}, offset={}, length={}",
            self.reason.bind(py).repr()?,
            self.offset,
            self.length
        );
        if let Some(detail) = self.detail(py)? {
            text.push_str(&format!(", detail={}", detail.bind(py).repr()?));
        }
        text.push(')');
        Ok(text)
    }
}

/// Caller-declared numeric notation, held pre-validated as the core's `ValidNumFormat` so the
/// hot path pays zero conversion.
#[pyclass(frozen, module = "hypercast")]
struct NumFormat {
    valid: core::ValidNumFormat,
}

impl NumFormat {
    fn format(&self) -> &core::NumFormat {
        self.valid.format()
    }

    /// The lenience flags alone: `flags` without the grouping policy, which has a getter of
    /// its own.
    fn lenience(&self) -> u32 {
        self.format().flags & !core::NumFormat::GROUPING_POLICY
    }

    fn grouping_policy(&self) -> u32 {
        self.format().flags & core::NumFormat::GROUPING_POLICY
    }
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
    /// Western grouping, the default: once a numeral uses a group separator, its
    /// integer part is groups of exactly three digits counted leftward from the decimal
    /// separator, the leftmost one to three and not starting with a zero, so ``12,34`` and
    /// ``0,125`` are ``MALFORMED``.
    #[classattr]
    const GROUPING_WESTERN: u32 = core::NumFormat::GROUPING_WESTERN;
    /// Indian grouping, only when declared: the group beside the decimal separator holds three digits,
    /// every group left of it two, the leftmost one or two (``12,34,567``).
    #[classattr]
    const GROUPING_INDIAN: u32 = core::NumFormat::GROUPING_INDIAN;
    /// The mask of the policy's two bits, which names no policy itself. Bit 8 (256) is retired and
    /// never reused: ``ValueError`` from every constructor.
    #[classattr]
    const GROUPING_POLICY: u32 = core::NumFormat::GROUPING_POLICY;

    // Python's own spelling — the constant-style name is the API.
    #[allow(non_snake_case)]
    #[classattr]
    fn INVARIANT() -> PyResult<NumFormat> {
        named_format(core::NumFormat::INVARIANT)
    }

    // Python's own spelling, as with INVARIANT above.
    #[allow(non_snake_case)]
    #[classattr]
    fn DETECT() -> PyResult<NumFormat> {
        named_format(core::NumFormat::DETECT)
    }

    #[new]
    #[pyo3(signature = (
        decimal_sep, group_sep, flags, currency = "", *,
        currency_decimal_sep = "", currency_group_sep = "", minus_sign = "", plus_sign = "",
        percent = "", digit_zero = "", grouping = core::NumFormat::GROUPING_WESTERN
    ))]
    #[allow(clippy::too_many_arguments)] // The constructor's signature is the API.
    fn new(
        #[pyo3(from_py_with = symbol_decimal_sep)] decimal_sep: &str,
        #[pyo3(from_py_with = symbol_group_sep)] group_sep: &str,
        flags: u32,
        #[pyo3(from_py_with = symbol_currency)] currency: &str,
        #[pyo3(from_py_with = symbol_currency_decimal_sep)] currency_decimal_sep: &str,
        #[pyo3(from_py_with = symbol_currency_group_sep)] currency_group_sep: &str,
        #[pyo3(from_py_with = symbol_minus_sign)] minus_sign: &str,
        #[pyo3(from_py_with = symbol_plus_sign)] plus_sign: &str,
        #[pyo3(from_py_with = symbol_percent)] percent: &str,
        #[pyo3(from_py_with = symbol_digit_zero)] digit_zero: &str,
        #[pyo3(from_py_with = grouping_int)] grouping: u32,
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
            grouping,
        })
    }

    /// The declared decimal separator.
    #[getter]
    fn decimal_sep(&self) -> String {
        self.format().decimal_sep.to_string()
    }

    /// The declared digit-group separator.
    #[getter]
    fn group_sep(&self) -> String {
        self.format().group_sep.to_string()
    }

    /// The bitwise OR of the lenience flags, the grouping policy not among them.
    #[getter]
    fn flags(&self) -> u32 {
        self.lenience()
    }

    /// The declared grouping policy: ``GROUPING_WESTERN`` or ``GROUPING_INDIAN``.
    #[getter]
    fn grouping(&self) -> u32 {
        self.grouping_policy()
    }

    /// This format with the policy ``grouping`` in place of its own, everything else kept: how a
    /// bridged format's column declares ``GROUPING_WESTERN`` or ``GROUPING_INDIAN``. Any other
    /// value is the ``ValueError`` the constructor raises.
    fn with_grouping(&self, #[pyo3(from_py_with = grouping_int)] grouping: u32) -> PyResult<Self> {
        declared_flags(self.lenience(), grouping)?;
        let Some(policy) = core::Grouping::from_code(grouping) else {
            return Err(PyValueError::new_err(format!(
                "Grouping must be NumFormat.GROUPING_WESTERN or GROUPING_INDIAN; got {grouping}"
            )));
        };
        Ok(NumFormat { valid: refuse_unless_checked(&self.format().with_grouping(policy))? })
    }

    /// The declared currency symbol — ``""`` when none is declared.
    #[getter]
    fn currency(&self) -> String {
        self.format().currency.as_str().to_string()
    }

    /// The decimal separator a currency amount is read with — ``""`` when it is
    /// ``decimal_sep``.
    #[getter]
    fn currency_decimal_sep(&self) -> String {
        self.format().currency_decimal_sep.to_string()
    }

    /// The digit-group separator a currency amount is read with — ``""`` when it is
    /// ``group_sep``.
    #[getter]
    fn currency_group_sep(&self) -> String {
        self.format().currency_group_sep.to_string()
    }

    /// The further spelling of the minus sign read beside ASCII ``-`` — ``""`` when none is
    /// declared.
    #[getter]
    fn minus_sign(&self) -> String {
        self.format().minus_sign.to_string()
    }

    /// The further spelling of the plus sign read beside ASCII ``+`` — ``""`` when none is
    /// declared.
    #[getter]
    fn plus_sign(&self) -> String {
        self.format().plus_sign.to_string()
    }

    /// The further spelling of the percent symbol read beside ASCII ``%`` — ``""`` when none
    /// is declared.
    #[getter]
    fn percent(&self) -> String {
        self.format().percent.to_string()
    }

    /// The zero of the native digits read beside ASCII ``0``–``9`` — ``""`` when none are
    /// declared.
    #[getter]
    fn digit_zero(&self) -> String {
        self.format().digits.zero().map(String::from).unwrap_or_default()
    }

    /// The 92 bytes this format crosses a C ABI as — the core's own size-tagged layout, which
    /// it reads in place: the size (``92``), the flags (the grouping policy in bits 8 and 9
    /// among them) and the native digits' zero code point
    /// (``0`` for none) as little-endian ``u32``\ s, then each symbol as its UTF-8 followed by
    /// its byte length, zero-padded — 8 bytes plus the length for the decimal, group,
    /// currency-decimal and currency-group separators, the minus and plus signs and the
    /// percent symbol, in that order, and 16 plus the length for the currency symbol — for
    /// a library that declares numeric columns across a C ABI of its own.
    #[getter]
    fn packed<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &core::RawNumFormat::from(*self.format()).to_le_bytes())
    }

    /// Equal to a format that declares the same thing, however each was made: the core's own
    /// equality, every field and the grouping policy.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.cast::<NumFormat>().is_ok_and(|other| other.get().format() == self.format())
    }

    /// A hash of the layout the format crosses the ABI in, which holds each symbol's declared
    /// bytes alone, as the core's equality does: equal formats are one ``dict`` key.
    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        core::RawNumFormat::from(*self.format()).to_le_bytes().hash(&mut hasher);
        hasher.finish()
    }

    /// The constructor call that builds this format again: the four positional arguments
    /// always, then only the keyword arguments that declare something (``grouping`` when it
    /// is not Western, the default).
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let repr = |text: &str| -> PyResult<String> { PyString::new(py, text).repr()?.extract() };
        let format = self.format();
        let mut call = format!(
            "NumFormat({}, {}, {}, {}",
            repr(format.decimal_sep.as_str())?,
            repr(format.group_sep.as_str())?,
            self.lenience(),
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
        if self.grouping_policy() == core::NumFormat::GROUPING_INDIAN {
            call.push_str(", grouping=NumFormat.GROUPING_INDIAN");
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
    /// The policy follows the locale's number pattern, ``grouping``, never its name and never the
    /// input: one all Indian (``[3, 2, 0]``) is Indian, and anything else is Western, so a locale
    /// with another shape reads what is groups of three and refuses what isn't (``Malformed``),
    /// never misreading it. A pattern is read as C defines it: sizes from the decimal separator
    /// outward, a trailing ``0`` repeating the last size, ``CHAR_MAX`` (this platform's: 127, or
    /// 255 where ``char`` is unsigned) ending grouping, and one with no size (``[]``, ``[127]``,
    /// ``[0]``) not grouping at all. One format has one policy, so a locale with Indian numbers
    /// and Western currency (``ml``) refuses a Western ``1,234,567``: declare
    /// ``grouping=NumFormat.GROUPING_WESTERN`` on that column to read it. ``GROUPING`` is cleared
    /// only when neither ``grouping`` nor ``mon_grouping`` groups; when just one does not (a
    /// currency with no grouping), the flag stays for the other. A dict that carries neither key
    /// declares nothing: Western with ``GROUPING`` on, as the constructor.
    ///
    /// A separator the locale leaves empty takes its invariant default (``.`` decimal, ``,``
    /// group) unless the other separator already holds that character, in which case it
    /// takes the other of the pair — so a comma-decimal locale with no thousands separator
    /// groups on ``.`` rather than colliding; an empty currency separator keeps the number
    /// separator it stands in for, by the same rule. Two separators the locale itself
    /// declares equal are still the ``ValueError`` the constructor raises. Every other lenience
    /// is on, radix prefixes and percent included; narrow the flags, or construct a
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
                Some(value) => {
                    value.extract::<String>().map_err(|err| naming(value.py(), name, err))
                }
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
        let (grouping, grouped) = grouping_of(&conv);
        let mut flags = core::NumFormat::ALL;
        if !grouped {
            flags &= !core::NumFormat::GROUPING;
        }
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
            grouping,
        })
    }
}

/// What a digit-group pattern groups like, as C's `localeconv` defines one: sizes from the
/// decimal separator outward, a trailing `0` repeating the last size, `CHAR_MAX` ending
/// grouping after the last size, so no size at all is no grouping.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// No size: the locale does not group.
    Ungrouped,
    /// Threes, repeated.
    Western,
    /// A three, then twos, repeated.
    Indian,
    /// Any other shape: groups of two, a size that ends, a size no digit group can have.
    Other,
}

fn shape(pattern: &[i64]) -> Shape {
    // The platform's own, as CPython reports a `localeconv()` size list: 255 where `char` is
    // unsigned (aarch64 Linux), 127 elsewhere.
    const CHAR_MAX: i64 = std::ffi::c_char::MAX as i64;
    // A list that stops with no terminator is read as C reads a string that ends: as a `0`.
    let (sizes, repeats) = match pattern.split_last() {
        Some((&CHAR_MAX, sizes)) => (sizes, false),
        Some((0, sizes)) => (sizes, true),
        _ => (pattern, true),
    };
    match sizes {
        [] => Shape::Ungrouped,
        _ if !repeats => Shape::Other,
        threes if threes.iter().all(|size| *size == 3) => Shape::Western,
        [3, twos @ ..] if twos.iter().all(|size| *size == 2) => Shape::Indian,
        _ => Shape::Other,
    }
}

/// The grouping policy a locale's patterns declare, and whether the format groups at all: the
/// number pattern all Indian is Indian, anything else Western; `GROUPING` is off only when every
/// pattern the dict carries is ungrouped. A key that is absent (or not a list of ints) says
/// nothing, and a dict that says nothing declares the constructor's default.
fn grouping_of(conv: &Bound<'_, PyDict>) -> (u32, bool) {
    let pattern = |name: &str| -> Option<Shape> {
        let value = conv.get_item(name).ok()??;
        value.extract::<Vec<i64>>().ok().map(|pattern| shape(&pattern))
    };
    let (number, currency) = (pattern("grouping"), pattern("mon_grouping"));
    let policy = if number == Some(Shape::Indian) {
        core::NumFormat::GROUPING_INDIAN
    } else {
        core::NumFormat::GROUPING_WESTERN
    };
    let shapes = [number, currency];
    let grouped = shapes.iter().all(Option::is_none)
        || shapes.iter().flatten().any(|shape| *shape != Shape::Ungrouped);
    (policy, grouped)
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
    grouping: u32,
}

/// The one place a format is built, so the constructor and the locale bridge refuse the
/// same caller bugs — the C ABI's own contract rules, raised here at construction so a
/// format that exists is one the core reads: a separator that is not 1 to 8 UTF-8 bytes,
/// equal separators (the number pair, or the pair a currency amount is read with), a decimal
/// separator, sign or percent symbol with white space in it, a currency symbol the core
/// cannot carry, a digit zero that starts no digit block. The flags come first, as the core's
/// own check takes them (the grouping policy, then a bit it does not define); within a field the
/// rules apply in this order: length, digit, equal separators, white space.
fn declare(declared: Declared<'_>) -> PyResult<NumFormat> {
    let flags = declared_flags(declared.flags, declared.grouping)?;
    let decimal_sep = separator("Decimal separator", declared.decimal_sep)?;
    let group_sep = separator("Group separator", declared.group_sep)?;
    if decimal_sep == group_sep {
        return Err(PyValueError::new_err(format!(
            "Decimal and group separators must differ; both are \"{}\"",
            declared.decimal_sep
        )));
    }
    no_white_space("Decimal separator", declared.decimal_sep)?;
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
    no_white_space("Currency decimal separator", declared.currency_decimal_sep)?;
    let resolved = core::NumFormat::with_separators(decimal_sep, group_sep, flags)
        .with_currency(currency_symbol(declared.currency)?)
        .with_currency_separators(currency_decimal_sep, currency_group_sep)
        .with_signs(
            optional("Minus sign", declared.minus_sign, true)?,
            optional("Plus sign", declared.plus_sign, true)?,
        )
        .with_percent(optional("Percent symbol", declared.percent, true)?)
        .with_digits(native_digits(declared.digit_zero)?);
    Ok(NumFormat { valid: refuse_unless_checked(&resolved)? })
}

/// A policy given as `grouping`, with the leniences of `flags`: a lenience mask never carries
/// the policy, and the policy is one of the two the core names, so `256` and `768`, alone or
/// over the styles (`351`, `863`), are refused here. The core judges which bits it defines.
fn declared_flags(flags: u32, grouping: u32) -> PyResult<u32> {
    const POLICY: u32 = core::NumFormat::GROUPING_POLICY;
    if grouping != core::NumFormat::GROUPING_WESTERN && grouping != core::NumFormat::GROUPING_INDIAN
    {
        return Err(PyValueError::new_err(format!(
            "Grouping must be NumFormat.GROUPING_WESTERN or GROUPING_INDIAN; got {grouping}"
        )));
    }
    if flags & POLICY != 0 {
        return Err(PyValueError::new_err(format!(
            "Flags are the lenience mask and must not carry grouping bits (declare the policy \
             with grouping=); got {flags}"
        )));
    }
    let declared = flags | grouping;
    match (core::NumFormat { flags: declared, ..core::NumFormat::INVARIANT }).validate() {
        Err(core::DeclarationError::UnknownFlag) => Err(PyValueError::new_err(format!(
            "Flags set a bit this version does not define; got {flags}"
        ))),
        _ => Ok(declared),
    }
}

/// A named format, which declares no policy and so is Western: `INVARIANT` and `DETECT`.
fn named_format(format: core::NumFormat) -> PyResult<NumFormat> {
    let valid =
        format.validate().map_err(|_| binding_bug("refused a named format it had accepted"))?;
    Ok(NumFormat { valid })
}

/// The backstop behind every per-field rule above, in every build: a format the core's own
/// `check` refuses is a caller bug, so a validator that lags the core raises here rather than
/// handing the doors a format they cannot read.
fn refuse_unless_checked(resolved: &core::NumFormat) -> PyResult<core::ValidNumFormat> {
    resolved
        .validate()
        .map_err(|_| PyValueError::new_err("NumFormat declares a format the core refuses"))
}

/// A text with no white space (any Unicode `White_Space` code point) in it, or a caller bug.
/// A decimal separator admits none: a numeral admits white space only as a group separator,
/// so such a declaration could never match.
fn no_white_space(what: &str, text: &str) -> PyResult<()> {
    if text.chars().any(core::is_white_space) {
        return Err(PyValueError::new_err(format!(
            "{what} must not contain white space; got \"{text}\""
        )));
    }
    Ok(())
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
/// and with `flush` (a sign, the percent symbol — read against the digits) one with no white
/// space in it either, checked after the separator's own rules.
fn optional(what: &str, text: &str, flush: bool) -> PyResult<core::Symbol<8>> {
    if text.is_empty() {
        return Ok(core::Symbol::NONE);
    }
    let symbol = separator(what, text)?;
    if flush {
        no_white_space(what, text)?;
    }
    Ok(symbol)
}

/// The declared currency symbol: `""` declares none; anything else must be a valid
/// `CurrencySymbol` (1 to 16 UTF-8 bytes, no ASCII digit, no white space at either edge:
/// inside is part of the symbol, CLDR's ``F CFA``), or it is a caller bug raised here, at
/// construction, the way equal separators are. A symbol breaking two rules names the
/// earlier: length, digit, white space.
fn currency_symbol(text: &str) -> PyResult<core::CurrencySymbol> {
    if text.is_empty() {
        return Ok(core::CurrencySymbol::NONE);
    }
    core::CurrencySymbol::new(text).ok_or_else(|| {
        let by_length_or_digit = text.len() > core::CurrencySymbol::MAX_BYTES
            || text.bytes().any(|byte| byte.is_ascii_digit());
        PyValueError::new_err(if by_length_or_digit {
            format!(
                "Currency symbol must be 1 to 16 UTF-8 bytes with no ASCII digit; got \"{text}\""
            )
        } else {
            format!("Currency symbol must not start or end with white space; got \"{text}\"")
        })
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
/// 16 UTF-8 bytes with no ASCII digit and no white space at either end (inside is fine:
/// Spanish ``a. m.``), or a caller bug raised at the call, the way a format's rules are at
/// construction. Length and digit are checked before the edges.
fn designator(what: &str, text: &str) -> PyResult<core::Symbol<16>> {
    if text.is_empty() {
        return Ok(core::Symbol::NONE);
    }
    let Some(symbol) = core::Symbol::new(text) else {
        return Err(PyValueError::new_err(format!(
            "Day period {what} must be 1 to 16 UTF-8 bytes with no ASCII digit; got \"{text}\""
        )));
    };
    if text.chars().next().is_some_and(core::is_white_space)
        || text.chars().next_back().is_some_and(core::is_white_space)
    {
        return Err(PyValueError::new_err(format!(
            "Day period {what} must have no white space at either end; got \"{text}\""
        )));
    }
    Ok(symbol)
}

/// Door input: str or bytes, each a zero-copy view into the caller's own object. A `str`
/// holding a lone surrogate has no UTF-8 form; its `surrogatepass` encoding, each surrogate
/// the three bytes no UTF-8 decoder accepts, is made once, when a door first reads it, and
/// kept beside it. Every door, plain or checked, then reads the surrogate as malformed text at
/// its own code point, never as a replacement character and never as an exception.
enum Text<'py> {
    Str(Bound<'py, PyString>, OnceCell<Bound<'py, PyBytes>>),
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
            Ok(Text::Str(text.to_owned(), OnceCell::new()))
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
    //
    // Only a `str` with a lone surrogate (whose `to_str()` fails) takes the second path, and
    // it pays one `encode` call, the first time a door reads it.
    fn bytes(&self) -> PyResult<&[u8]> {
        match self {
            Text::Str(text, kept) => match text.to_str() {
                Ok(utf8) => Ok(utf8.as_bytes()),
                Err(_) => {
                    if kept.get().is_none() {
                        let _ = kept.set(surrogates_kept(text)?);
                    }
                    let kept = kept.get().ok_or_else(|| binding_bug("lost a str's encoding"))?;
                    Ok(kept.as_bytes())
                }
            },
            Text::Bytes(bytes) => Ok(bytes.as_bytes()),
        }
    }

    /// Presents a failure in the caller's own units: the core's byte span as-is for `bytes`
    /// input, remapped to code points for `str` input. An ASCII `str` needs no mapping and
    /// pays one length comparison (code points equal bytes); only a non-ASCII `str`, and
    /// only on the fault path, counts code points.
    fn present(&self, failed: core::Failure) -> PyResult<core::Failure> {
        let Text::Str(text, _) = self else {
            return Ok(failed);
        };
        let bytes = self.bytes()?;
        if bytes.len() == text.len()? {
            return Ok(failed);
        }
        let (offset, len) = code_point_span(bytes, failed.offset, failed.len);
        Ok(core::Failure { offset, len, ..failed })
    }
}

/// A byte span of `bytes` as code points: where it starts, and how many it runs.
fn code_point_span(bytes: &[u8], offset: u32, len: u32) -> (u32, u32) {
    let offset = (offset as usize).min(bytes.len());
    let end = offset.saturating_add(len as usize).min(bytes.len());
    (code_points(&bytes[..offset]), code_points(&bytes[offset..end]))
}

/// Every code point starts with exactly one non-continuation byte.
fn code_points(bytes: &[u8]) -> u32 {
    bytes.iter().filter(|byte| **byte & 0xC0 != 0x80).count() as u32
}

fn fault(py: Python<'_>, failed: core::Failure) -> PyResult<Py<PyAny>> {
    let member = match failed.reason {
        core::Reason::Empty => &EMPTY,
        core::Reason::Malformed => &MALFORMED,
        core::Reason::OutOfRange => &OUT_OF_RANGE,
    };
    let reason = cached(py, member)?.clone().unbind();
    let detail_code = Some(failed.detail.code());
    Ok(Py::new(py, Fault { reason, offset: failed.offset, length: failed.len, detail_code })?
        .into_any())
}

fn verdict<'py, T>(
    py: Python<'py>,
    text: &Text<'_>,
    outcome: Result<T, core::Failure>,
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

/// A typed door's verdict: the reason and detail, presented as a fault with an empty span
/// (there is no text for one to index).
fn typed<'py, T>(
    py: Python<'py>,
    outcome: Result<T, core::Failure>,
    into: impl FnOnce(Python<'py>, T) -> PyResult<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    match outcome {
        Ok(value) => {
            let value = into(py, value)?;
            Ok(Py::new(py, Success { value })?.into_any())
        }
        Err(failed) => fault(py, core::Failure { offset: 0, len: 0, ..failed }),
    }
}

// Each door's doc comment is its Python docstring — what `help(hypercast.cast_i32)` prints —
// so it is written in reStructuredText, and a test holds every door to having one.
macro_rules! numeric_doors {
    ($($(#[$doc:meta])* $door:ident => $core:ident),+ $(,)?) => {$(
        $(#[$doc])*
        #[pyfunction]
        fn $door(py: Python<'_>, text: Text<'_>, fmt: PyRef<'_, NumFormat>) -> PyResult<Py<PyAny>> {
            verdict(py, &text, core::detailed::$core(text.bytes()?, &fmt.valid), |py, value| {
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
    verdict(py, &text, core::detailed::cast_decimal(text.bytes()?, &fmt.valid), decimal_value)
}

/// Reads a ``float`` as the exact ``decimal.Decimal`` it names: the shortest decimal that
/// rounds back to it, the digits a spreadsheet writes for it — ``0.1`` is
/// ``Decimal('0.1')``, not the binary fraction ``Decimal(0.1)`` spells out. NaN is
/// ``MALFORMED``; an infinity, past 96 bits or more than 28 places is ``OUT_OF_RANGE``.
#[pyfunction]
fn cast_decimal_from_float(py: Python<'_>, value: f64) -> PyResult<Py<PyAny>> {
    typed(py, core::detailed::decimal_from_f64(value), decimal_value)
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
    verdict(py, &text, core::detailed::cast_bool(text.bytes()?), |py, value| {
        Ok(value.into_pyobject(py)?.to_owned().into_any().unbind())
    })
}

/// Casts one character — the text verbatim when it is exactly one scalar, otherwise a declared
/// code point (``65``, ``U+0041``, ``0x41``, ``&H41``, ``&#65;``, ``&#x41;``) — to a
/// one-character ``str``. Past ``U+10FFFF`` or a surrogate is ``OUT_OF_RANGE``.
#[pyfunction]
fn cast_char(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::detailed::cast_char(text.bytes()?), |py, value| {
        Ok(value.into_pyobject(py)?.into_any().unbind())
    })
}

/// Casts UUID text — every .NET ``Guid`` form plus ``urn:uuid:``-style prefixes — to a
/// ``uuid.UUID``.
#[pyfunction]
fn cast_uuid(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::detailed::cast_uuid(text.bytes()?), |py, bytes| {
        Ok(values()?.uuid(py, bytes)?.unbind())
    })
}

fn instant(py: Python<'_>, ts: core::Timestamp) -> PyResult<Py<PyAny>> {
    Ok(values()?.instant(py, ts)?.unbind())
}

// --- Date options ---------------------------------------------------------------------------
//
// The shapes the date doors read beyond their plain ones are opt-in, declared with these two
// types and judged by the core's own validation, as `DateOptions::validate` runs it, on every
// call: nothing is kept between calls, and the core refuses a declaration here at construction
// the way it would at a call.

/// Which date shapes a door also reads, and the pivot two-digit years are read around. Built
/// from the flag constants (``COMPACT_DATE``, ``SPACE_SEPARATOR``, ``HOUR_OFFSET``), or
/// ``COMMON_FORMS`` for all three: every shape is opt-in, so a door given none reads exactly
/// what it always has. An option a door does not read is ignored by it. An option bit this
/// version does not define, or a pivot outside ``100``–``9999`` (``0`` is none), is a
/// ``ValueError`` here. Immutable.
///
/// ``COMPACT_DATE`` lets :func:`cast_date` read ``yyyyMMdd``; ``SPACE_SEPARATOR`` lets
/// :func:`cast_timestamp` read a space where the ``T`` goes and ``HOUR_OFFSET`` an offset of
/// ``±hh``. The pivot is the last year of the hundred-year window a two-digit year is read in
/// (``26`` is 2026 under a pivot of ``2029``), for the ordered :func:`cast_date` and
/// :func:`cast_datetime`; without one a two-digit year is a fault.
#[pyclass(frozen, module = "hypercast")]
struct DateOptions {
    options: core::DateOptions,
}

impl DateOptions {
    fn declare(flags: u32, pivot: u32) -> PyResult<Self> {
        // A pivot too wide for the core's own type is refused as the same bad pivot.
        let pivot16 = u16::try_from(pivot).unwrap_or(u16::MAX);
        let options = core::DateOptions::from_flags(flags).with_pivot(pivot16);
        match options.validate(None) {
            Ok(_) => Ok(DateOptions { options }),
            Err(core::DeclarationError::DateFlags) => Err(PyValueError::new_err(format!(
                "Date option flags set a bit this version does not define; got {flags}"
            ))),
            Err(core::DeclarationError::DatePivot) => Err(PyValueError::new_err(format!(
                "Date pivot must be 0 (none) or 100 to 9999; got {pivot}"
            ))),
            Err(_) => Err(binding_bug("refused date options it should not judge")),
        }
    }
}

#[pymethods]
impl DateOptions {
    /// ``cast_date`` also reads ``yyyyMMdd``, eight ASCII digits.
    #[classattr]
    const COMPACT_DATE: u32 = core::DateOptions::COMPACT_DATE;
    /// ``cast_timestamp`` also reads a single space where the ``T`` goes.
    #[classattr]
    const SPACE_SEPARATOR: u32 = core::DateOptions::SPACE_SEPARATOR;
    /// ``cast_timestamp`` also reads an offset of ``±hh``, with no minutes.
    #[classattr]
    const HOUR_OFFSET: u32 = core::DateOptions::HOUR_OFFSET;

    /// ``COMPACT_DATE``, ``SPACE_SEPARATOR`` and ``HOUR_OFFSET`` together, the shapes real
    /// feeds mix with RFC 3339; never a pivot, never month names.
    // Python's own spelling — the constant-style name is the API.
    #[allow(non_snake_case)]
    #[classattr]
    fn COMMON_FORMS() -> DateOptions {
        DateOptions { options: core::DateOptions::COMMON_FORMS }
    }

    #[new]
    #[pyo3(signature = (flags = 0, pivot = 0))]
    fn new(
        #[pyo3(from_py_with = date_flags_int)] flags: u32,
        #[pyo3(from_py_with = date_pivot_int)] pivot: u32,
    ) -> PyResult<Self> {
        DateOptions::declare(flags, pivot)
    }

    /// These options with ``pivot`` as the pivot, ``0`` for none.
    fn with_pivot(
        &self,
        #[pyo3(from_py_with = date_pivot_int)] pivot: u32,
    ) -> PyResult<DateOptions> {
        DateOptions::declare(self.options.flags(), pivot)
    }

    /// The bitwise OR of the shape flags.
    #[getter]
    fn flags(&self) -> u32 {
        self.options.flags()
    }

    /// The last year of the window a two-digit year is read in, ``0`` for none.
    #[getter]
    fn pivot(&self) -> u32 {
        u32::from(self.options.pivot())
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.cast::<DateOptions>().is_ok_and(|other| other.get().options == self.options)
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.options.flags(), self.options.pivot()).hash(&mut hasher);
        hasher.finish()
    }

    /// The constructor call that builds these options again.
    fn __repr__(&self) -> String {
        match self.options.pivot() {
            0 => format!("DateOptions(flags={})", self.options.flags()),
            pivot => format!("DateOptions(flags={}, pivot={pivot})", self.options.flags()),
        }
    }
}

/// Month names a date door reads in the month field, the way Excel writes them
/// (``07-Jan-26``): ``(month, name)`` pairs, a month 1–12 each, aliases as separate pairs
/// (``Sep`` and ``Sept``), every month with at least one. Built by the caller; none is looked
/// up from a locale. A name is 1 to 64 UTF-8 bytes holding no ASCII digit, ``/``, ``-`` or
/// white space (``.`` is fine anywhere), and no two names may fold to the same spelling; a
/// vocabulary that breaks a rule is a ``ValueError`` naming it. Names match under simple
/// case folding, the longest one followed by the date's separator winning, and punctuation is
/// part of the name. Passed to :func:`cast_date` and :func:`cast_datetime` as ``months``,
/// where it replaces the numeric month at the position the declared ``DateOrder`` gives it.
/// Immutable.
#[pyclass(frozen, module = "hypercast")]
struct MonthNames {
    packed: Vec<u8>,
}

impl MonthNames {
    /// The vocabulary the core judges `packed` to be, or the caller's bug it names.
    fn judged(packed: Vec<u8>) -> PyResult<Self> {
        match core::MonthNames::from_packed(&packed) {
            Ok(_) => Ok(MonthNames { packed }),
            Err(core::DeclarationError::MonthNames(fault)) => {
                Err(PyValueError::new_err(format!("Month names refused: {fault}.")))
            }
            Err(_) => Err(binding_bug("refused a vocabulary with a rule it does not judge")),
        }
    }

    /// The entries, read back out of the layout `judged` accepted.
    fn declared_entries(&self) -> Vec<(u8, &str)> {
        let mut entries = Vec::new();
        let mut rest = self.packed.as_slice();
        while let [month, a, b, c, d, tail @ ..] = rest {
            let len = u32::from_le_bytes([*a, *b, *c, *d]) as usize;
            let (Some(name), Some(after)) = (tail.get(..len), tail.get(len..)) else {
                break;
            };
            let Ok(name) = std::str::from_utf8(name) else {
                break;
            };
            entries.push((*month, name));
            rest = after;
        }
        entries
    }
}

#[pymethods]
impl MonthNames {
    #[new]
    fn new(entries: &Bound<'_, PyAny>) -> PyResult<Self> {
        if entries.is_instance_of::<PyString>() || entries.is_instance_of::<PyBytes>() {
            return Err(PyTypeError::new_err(
                "month names must be an iterable of (month, name) pairs, not a single str or bytes",
            ));
        }
        let mut packed = Vec::new();
        for (i, entry) in entries.try_iter()?.enumerate() {
            let entry = entry?;
            let Some((month, name)) = month_name_pair(&entry) else {
                return Err(PyTypeError::new_err(format!(
                    "month names must be (month, name) pairs; entry {i} is {}",
                    entry.get_type().name().map_or_else(|_| "?".into(), |name| name.to_string())
                )));
            };
            // A month no byte holds is refused by the core as the month it is not.
            packed.push(u8::try_from(month).unwrap_or(u8::MAX));
            str_bytes(&name, |bytes| {
                let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
                packed.extend_from_slice(&len.to_le_bytes());
                packed.extend_from_slice(bytes);
            })?;
        }
        MonthNames::judged(packed)
    }

    /// Reads a vocabulary from the layout the core takes across its C ABI: each entry a month
    /// byte (1–12), a little-endian ``u32`` byte length, then that many UTF-8 bytes, with no
    /// padding. The core's own rules judge it; one it breaks is a ``ValueError``.
    #[staticmethod]
    fn from_packed(data: &Bound<'_, PyBytes>) -> PyResult<Self> {
        MonthNames::judged(data.as_bytes().to_vec())
    }

    /// The vocabulary in the layout the core's C ABI takes (:meth:`from_packed` reads it back).
    #[getter]
    fn packed<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.packed)
    }

    /// The ``(month, name)`` pairs, in the order they were declared.
    #[getter]
    fn entries<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.declared_entries())
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.cast::<MonthNames>().is_ok_and(|other| other.get().packed == self.packed)
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.packed.hash(&mut hasher);
        hasher.finish()
    }

    /// The constructor call that builds this vocabulary again.
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!("MonthNames({})", PyTuple::new(py, self.declared_entries())?.repr()?))
    }
}

/// An entry of a vocabulary: any two-item sequence of an `int` month and a `str` name.
fn month_name_pair<'py>(entry: &Bound<'py, PyAny>) -> Option<(i64, Bound<'py, PyString>)> {
    let pair = entry.extract::<Vec<Bound<'py, PyAny>>>().ok()?;
    let [month, name] = pair.as_slice() else {
        return None;
    };
    if month.is_instance_of::<PyBool>() || !month.is_instance_of::<PyInt>() {
        return None;
    }
    // An `int` too wide for any month is a month the core refuses, not a wrong type.
    Some((month.extract().unwrap_or(i64::MAX), name.cast::<PyString>().ok()?.clone()))
}

/// Runs a date door under the declared options and vocabulary, judged by the core on every
/// call, and not at all when neither is declared: the door then reads what it always has.
fn with_date_options<R>(
    options: Option<&DateOptions>,
    months: Option<&MonthNames>,
    door: impl FnOnce(Option<&core::ValidDateOptions<'_>>) -> R,
) -> PyResult<R> {
    if options.is_none() && months.is_none() {
        return Ok(door(None));
    }
    let months = months
        .map(|names| core::MonthNames::from_packed(&names.packed))
        .transpose()
        .map_err(|_| binding_bug("refused a vocabulary it had accepted"))?;
    let valid = options
        .map_or(core::DateOptions::NONE, |declared| declared.options)
        .validate(months)
        .map_err(|_| binding_bug("refused date options it had accepted"))?;
    Ok(door(Some(&valid)))
}

/// Casts an RFC 3339 instant to an aware UTC ``datetime`` (microsecond truncation). With
/// ``options`` carrying ``SPACE_SEPARATOR`` it also reads a space where the ``T`` goes, and
/// with ``HOUR_OFFSET`` an offset of ``±hh``; given none, exactly the strict shape.
#[pyfunction]
#[pyo3(signature = (text, *, options = None))]
fn cast_timestamp(
    py: Python<'_>,
    text: Text<'_>,
    options: Option<PyRef<'_, DateOptions>>,
) -> PyResult<Py<PyAny>> {
    let bytes = text.bytes()?;
    let outcome = with_date_options(options.as_deref(), None, |valid| {
        core::detailed::cast_timestamp(bytes, valid)
    })?;
    verdict(py, &text, outcome, instant)
}

/// Casts an integer Unix-epoch value under the declared ``UnixPrecision``.
#[pyfunction]
fn cast_unix(py: Python<'_>, text: Text<'_>, precision: u32) -> PyResult<Py<PyAny>> {
    let Some(precision) = core::UnixPrecision::from_code(precision) else {
        return Err(PyValueError::new_err("precision must be a UnixPrecision"));
    };
    verdict(py, &text, core::detailed::cast_unix(text.bytes()?, precision), instant)
}

/// Casts an Excel date serial under the declared ``ExcelEpoch``.
#[pyfunction]
fn cast_excel_serial(py: Python<'_>, text: Text<'_>, epoch: u32) -> PyResult<Py<PyAny>> {
    let Some(epoch) = core::ExcelEpoch::from_code(epoch) else {
        return Err(PyValueError::new_err("epoch must be an ExcelEpoch"));
    };
    verdict(py, &text, core::detailed::cast_excel_serial(text.bytes()?, epoch), instant)
}

/// Reads an Excel serial ``float`` under the declared ``ExcelEpoch`` as the naive
/// ``datetime`` it names — the twin of ``cast_excel_serial`` for a number a workbook reader
/// already holds. Zone-less, as the cell is.
#[pyfunction]
fn cast_excel_serial_from_float(py: Python<'_>, value: f64, epoch: u32) -> PyResult<Py<PyAny>> {
    let Some(epoch) = core::ExcelEpoch::from_code(epoch) else {
        return Err(PyValueError::new_err("epoch must be an ExcelEpoch"));
    };
    typed(py, core::detailed::excel_serial(value, epoch), civil_value)
}

/// Reads the fraction of an Excel serial ``float`` as a ``time`` of day (microsecond
/// truncation).
#[pyfunction]
fn cast_excel_time(py: Python<'_>, value: f64) -> PyResult<Py<PyAny>> {
    typed(py, core::detailed::excel_time(value), time_value)
}

/// Reads a ``float`` count of days as a ``timedelta`` (microsecond truncation toward zero).
#[pyfunction]
fn cast_excel_duration(py: Python<'_>, value: f64) -> PyResult<Py<PyAny>> {
    typed(py, core::detailed::excel_duration(value), duration_value)
}

fn date_value(py: Python<'_>, date: core::Date) -> PyResult<Py<PyAny>> {
    Ok(values()?.date(py, date)?.unbind())
}

/// Casts a calendar date: strict ISO ``yyyy-MM-dd`` with no order, the separated forms
/// under a declared ``DateOrder``. ``options`` opens the opt-in shapes: ``COMPACT_DATE`` lets
/// the ISO form read ``yyyyMMdd``, and under an order its pivot reads two-digit years and
/// ``months`` (a :class:`MonthNames`) the month names Excel writes; each is judged by the
/// core, and a door given neither reads exactly what it always has.
#[pyfunction]
#[pyo3(signature = (text, order = None, *, options = None, months = None))]
fn cast_date(
    py: Python<'_>,
    text: Text<'_>,
    order: Option<u32>,
    options: Option<PyRef<'_, DateOptions>>,
    months: Option<PyRef<'_, MonthNames>>,
) -> PyResult<Py<PyAny>> {
    let Some(order) = order else {
        let bytes = text.bytes()?;
        let outcome = with_date_options(options.as_deref(), None, |valid| {
            core::detailed::cast_date(bytes, valid)
        })?;
        return verdict(py, &text, outcome, date_value);
    };
    let Some(order) = core::DateOrder::from_code(order) else {
        return Err(PyValueError::new_err("order must be a DateOrder"));
    };
    let bytes = text.bytes()?;
    let outcome = with_date_options(options.as_deref(), months.as_deref(), |valid| {
        core::detailed::cast_date_ordered(bytes, order, valid)
    })?;
    verdict(py, &text, outcome, date_value)
}

/// Casts a zone-less civil date-time under a declared ``DateOrder`` to a naive
/// ``datetime``. The day periods ``am`` and ``pm`` declare a culture's designators
/// (``"vorm."``/``"nachm."``, ``"오전"``/``"오후"``), read exactly, after the clock or
/// ahead of it, beside the English ``AM``/``PM``, which are read whatever is declared;
/// each is 1 to 16 UTF-8 bytes with no ASCII digit and no white space at either end, or a
/// ``ValueError``, a caller bug like a malformed ``NumFormat``. ``options`` (its pivot) and
/// ``months`` read two-digit years and month names in the date, as :func:`cast_date` does.
#[pyfunction]
#[pyo3(signature = (text, order, *, am = "", pm = "", options = None, months = None))]
#[allow(clippy::too_many_arguments)] // The signature is the API.
fn cast_datetime(
    py: Python<'_>,
    text: Text<'_>,
    order: u32,
    #[pyo3(from_py_with = symbol_am)] am: &str,
    #[pyo3(from_py_with = symbol_pm)] pm: &str,
    options: Option<PyRef<'_, DateOptions>>,
    months: Option<PyRef<'_, MonthNames>>,
) -> PyResult<Py<PyAny>> {
    let Some(order) = core::DateOrder::from_code(order) else {
        return Err(PyValueError::new_err("order must be a DateOrder"));
    };
    // Nothing declared takes the core's own English door, where the designators are a
    // constant the parser folds away; the declared path pays for its two checks.
    if am.is_empty() && pm.is_empty() {
        let bytes = text.bytes()?;
        let outcome = with_date_options(options.as_deref(), months.as_deref(), |valid| {
            core::detailed::cast_datetime(bytes, order, valid)
        })?;
        return verdict(py, &text, outcome, civil_value);
    }
    let periods = core::DayPeriods::new(designator("am", am)?, designator("pm", pm)?);
    let bytes = text.bytes()?;
    let outcome = with_date_options(options.as_deref(), months.as_deref(), |valid| {
        core::detailed::cast_datetime_with(bytes, order, &periods, valid)
    })?;
    verdict(py, &text, outcome, civil_value)
}

fn civil_value(py: Python<'_>, civil: core::CivilDateTime) -> PyResult<Py<PyAny>> {
    Ok(values()?.civil(py, civil)?.unbind())
}

/// Casts an ISO 24-hour time-of-day to a ``time`` (microsecond truncation).
#[pyfunction]
fn cast_time(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::detailed::cast_time(text.bytes()?), time_value)
}

fn time_value(py: Python<'_>, nanos: u64) -> PyResult<Py<PyAny>> {
    Ok(values()?.time(py, nanos)?.unbind())
}

/// Casts a duration (ISO 8601, invariant colon form, or protobuf JSON seconds) to a
/// ``timedelta`` (microsecond truncation toward zero).
#[pyfunction]
fn cast_duration(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    verdict(py, &text, core::detailed::cast_duration(text.bytes()?), duration_value)
}

fn duration_value(py: Python<'_>, span: core::Duration) -> PyResult<Py<PyAny>> {
    Ok(values()?.duration(py, span)?.unbind())
}

// --- Validation constraints ----------------------------------------------------------------
//
// A declaration is laid out exactly as the C ABI carries it, a `RawConstraints` with the packed
// `enum` list beside it, and judged by the core's own functions: `check_constraints` once, when
// a `Constraints` is built (what `cast_constraints_check` asks), and the `judge_*` functions
// the checked C exports call, on every cast. Bounds, steps and integer entries are converted
// by `declared_decimal`, the conversion `cast_declared_decimal` runs. No rule is restated
// here: every refusal and every violation is the core's.

/// Checked-door input: the caller's text, read as every door reads it (a lone surrogate kept
/// as malformed bytes; see [`Text`]).
struct Input<'py> {
    text: Text<'py>,
}

impl<'py> Input<'py> {
    fn new(text: Text<'py>) -> PyResult<Self> {
        Ok(Input { text })
    }

    fn bytes(&self) -> PyResult<&[u8]> {
        self.text.bytes()
    }

    /// A byte span in the caller's own units, as a fault's is: as-is for `bytes`, in code
    /// points for `str` (an ASCII `str` pays one length comparison).
    fn span(&self, offset: u32, len: u32) -> PyResult<(u32, u32)> {
        let Text::Str(text, _) = &self.text else {
            return Ok((offset, len));
        };
        let bytes = self.bytes()?;
        if bytes.len() == text.len()? {
            return Ok((offset, len));
        }
        Ok(code_point_span(bytes, offset, len))
    }

    /// The judged text at a byte span the core reported as valid UTF-8: the caller's own `str`
    /// when the span is all of it, a new `str` otherwise (a trimmed span, or `bytes` input).
    fn text_at(&self, py: Python<'_>, (offset, len): (u32, u32)) -> PyResult<Py<PyAny>> {
        let bytes = self.bytes()?;
        if let Text::Str(text, _) = &self.text
            && offset == 0
            && len as usize == bytes.len()
        {
            return Ok(text.clone().into_any().unbind());
        }
        let start = offset as usize;
        let text = bytes
            .get(start..start.saturating_add(len as usize))
            .and_then(|span| std::str::from_utf8(span).ok())
            .ok_or_else(|| binding_bug("reported a text span that is not UTF-8"))?;
        Ok(PyString::new(py, text).into_any().unbind())
    }
}

/// A declared symbol's text, read as `&str` is: a `str` holding a lone surrogate has no UTF-8
/// form, and the `UnicodeEncodeError` Python raises for it keeps its encoding, object, position
/// and PyO3's "while processing" note, its reason naming the field, as every binding words it.
fn named<'py>(name: &'static str, obj: &'py Bound<'py, PyAny>) -> PyResult<&'py str> {
    obj.cast::<PyString>()?.to_str().map_err(|err| naming(obj.py(), name, err))
}

/// Names `name` in a `UnicodeEncodeError`'s reason; any other error passes through untouched.
fn naming(py: Python<'_>, name: &str, err: PyErr) -> PyErr {
    if err.is_instance_of::<PyUnicodeEncodeError>(py) {
        let reason = format!("{name} must be whole code points, not surrogate halves");
        if let Err(failed) = err.value(py).setattr("reason", reason) {
            return failed;
        }
    }
    err
}

/// One extractor per symbol parameter: `from_py_with` takes a path, not a closure.
macro_rules! symbols {
    ($($fn:ident => $name:literal),+ $(,)?) => {$(
        fn $fn<'py>(obj: &'py Bound<'py, PyAny>) -> PyResult<&'py str> {
            named($name, obj)
        }
    )+};
}

symbols! {
    symbol_decimal_sep => "decimal_sep",
    symbol_group_sep => "group_sep",
    symbol_currency => "currency",
    symbol_currency_decimal_sep => "currency_decimal_sep",
    symbol_currency_group_sep => "currency_group_sep",
    symbol_minus_sign => "minus_sign",
    symbol_plus_sign => "plus_sign",
    symbol_percent => "percent",
    symbol_digit_zero => "digit_zero",
    symbol_am => "am",
    symbol_pm => "pm",
}

/// A declaration's integer in the 32 unsigned bits the core counts it in. A wrong type is the
/// `TypeError` extraction raises; an `int` outside those bits is a declaration the core could
/// not read, so a `ValueError` naming the field, as every other refused declaration is.
fn declared_u32(what: &str, obj: &Bound<'_, PyAny>) -> PyResult<u32> {
    match obj.extract::<u32>() {
        Err(err) if err.is_instance_of::<PyOverflowError>(obj.py()) => Err(PyValueError::new_err(
            format!("{what} must be an int in 32 unsigned bits; got {}", obj.str()?),
        )),
        other => other,
    }
}

/// One extractor per such field: `from_py_with` takes a path, not a closure.
macro_rules! declared_ints {
    ($($fn:ident => $name:literal),+) => {$(
        fn $fn<'py>(obj: &'py Bound<'py, PyAny>) -> PyResult<u32> {
            declared_u32($name, obj)
        }
    )+};
}

declared_ints! {
    date_flags_int => "Date option flags",
    date_pivot_int => "Date pivot",
    grouping_int => "Grouping"
}

fn optional_detail_code<'py>(obj: &'py Bound<'py, PyAny>) -> PyResult<Option<u32>> {
    if obj.is_none() { Ok(None) } else { declared_u32("detail_code", obj).map(Some) }
}

/// A `str` encoded with its lone surrogates kept (`surrogatepass`).
fn surrogates_kept<'py>(text: &Bound<'py, PyString>) -> PyResult<Bound<'py, PyBytes>> {
    let py = text.py();
    let encoded = text.call_method1(
        intern!(py, "encode"),
        (intern!(py, "utf-8"), intern!(py, "surrogatepass")),
    )?;
    Ok(encoded.cast_into::<PyBytes>()?)
}

/// Runs `with` over a `str`'s bytes: its UTF-8, or with its lone surrogates kept.
fn str_bytes<R>(text: &Bound<'_, PyString>, with: impl FnOnce(&[u8]) -> R) -> PyResult<R> {
    Ok(match text.to_str() {
        Ok(utf8) => with(utf8.as_bytes()),
        Err(_) => with(surrogates_kept(text)?.as_bytes()),
    })
}

fn binding_bug(what: &str) -> PyErr {
    PyRuntimeError::new_err(format!("libhypercast {what}: a binding bug, please report it"))
}

/// A refused declaration, the caller's bug: one message per code, worded as the core's
/// `Refusal` displays it.
fn refused(refusal: core::Refusal) -> PyErr {
    PyValueError::new_err(format!("Constraints refused (code {}): {refusal}.", refusal.code()))
}

/// A bound, step or integer `enum` entry as the core converts it: a `decimal.Decimal`, an
/// `int` or a `str`, each through its text and `declared_decimal`. Never a `float` (nor a
/// `bool`): `0.1` has no exact binary form, so a float could only declare a neighbour of the
/// value meant. `Err` inside is the core's refusal; outside, a wrong type.
fn declared(
    what: &str,
    value: &Bound<'_, PyAny>,
) -> PyResult<Result<core::Decimal, core::Refusal>> {
    let py = value.py();
    if let Ok(text) = value.cast::<PyString>() {
        return str_bytes(text, declared_decimal);
    }
    let exact = !value.is_instance_of::<PyBool>()
        && (value.is_instance_of::<PyInt>() || value.is_instance(cached(py, &DECIMAL)?)?);
    if !exact {
        let hint = if value.is_instance_of::<PyFloat>() {
            "; a float holds most decimals only approximately, so write it as a str or a decimal.Decimal"
        } else {
            ""
        };
        return Err(PyTypeError::new_err(format!(
            "{what} must be a decimal.Decimal, an int or a str, not {}{hint}",
            value.get_type().name()?
        )));
    }
    Ok(declared_decimal(value.str()?.to_str()?.as_bytes()))
}

/// A declared length: an `int` in the 32 unsigned bits the core counts code points in, never a
/// `bool` (`True` is an `int` to Python, but a length of `True` is a caller's bug), as a bound
/// is never one.
struct Length(u32);

impl<'py> FromPyObject<'_, 'py> for Length {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        if obj.is_instance_of::<PyBool>() {
            return Err(PyTypeError::new_err("a length must be an int, not bool"));
        }
        Ok(Length(obj.extract()?))
    }
}

/// What a caller declares, as the constructor and `cast_constraints_check` take it.
struct Keywords<'py> {
    door: u32,
    minimum: Option<Bound<'py, PyAny>>,
    exclusive_minimum: Option<Bound<'py, PyAny>>,
    maximum: Option<Bound<'py, PyAny>>,
    exclusive_maximum: Option<Bound<'py, PyAny>>,
    multiple_of: Option<Bound<'py, PyAny>>,
    rounding: u32,
    min_length: Option<u32>,
    max_length: Option<u32>,
    trim: bool,
    entries: Option<Bound<'py, PyAny>>,
}

/// A declaration as the C ABI carries it, and the caller's own `enum` entries.
struct Declaration {
    door: core::Door,
    raw: core::RawConstraints,
    packed: Vec<u8>,
    entries: Option<Py<PyTuple>>,
}

/// The door a declaration is for: a checked door's code, or a `ValueError`, as a
/// `UnixPrecision` that names no member is.
fn checked_door(code: u32) -> PyResult<core::Door> {
    match core::Door::from_code(code, 0) {
        Some(door) if door.is_numeric() || door == core::Door::Text => Ok(door),
        _ => Err(PyValueError::new_err("door must be a Door")),
    }
}

/// The one place a declaration is laid out and judged, so `Constraints` and
/// `cast_constraints_check` refuse exactly the same declarations: the core's refusal inside,
/// a wrong argument outside.
fn declaration(
    py: Python<'_>,
    keywords: Keywords<'_>,
) -> PyResult<Result<Declaration, core::Refusal>> {
    macro_rules! decimal {
        ($what:literal, $value:expr) => {
            match declared($what, $value)? {
                Ok(decimal) => decimal,
                Err(refusal) => return Ok(Err(refusal)),
            }
        };
    }
    let door = checked_door(keywords.door)?;
    let mut raw = core::RawConstraints::ZERO;
    // Any code goes to the core as it is, which refuses one it does not know.
    raw.rounding = keywords.rounding;
    if let Some(value) = &keywords.minimum {
        raw.lower = decimal!("minimum", value);
        raw.flags |= core::RawConstraints::HAS_MINIMUM;
    }
    if let Some(value) = &keywords.exclusive_minimum {
        raw.lower = decimal!("exclusive_minimum", value);
        raw.flags |= core::RawConstraints::HAS_EXCLUSIVE_MINIMUM;
    }
    if let Some(value) = &keywords.maximum {
        raw.upper = decimal!("maximum", value);
        raw.flags |= core::RawConstraints::HAS_MAXIMUM;
    }
    if let Some(value) = &keywords.exclusive_maximum {
        raw.upper = decimal!("exclusive_maximum", value);
        raw.flags |= core::RawConstraints::HAS_EXCLUSIVE_MAXIMUM;
    }
    if let Some(value) = &keywords.multiple_of {
        raw.multiple_of = decimal!("multiple_of", value);
        raw.flags |= core::RawConstraints::HAS_MULTIPLE_OF;
    }
    if let Some(length) = keywords.min_length {
        raw.min_length = length;
        raw.flags |= core::RawConstraints::HAS_MIN_LENGTH;
    }
    if let Some(length) = keywords.max_length {
        raw.max_length = length;
        raw.flags |= core::RawConstraints::HAS_MAX_LENGTH;
    }
    if keywords.trim {
        raw.flags |= core::RawConstraints::TRIM;
    }
    let mut packed = Vec::new();
    let mut entries = None;
    if let Some(list) = &keywords.entries {
        if list.is_instance_of::<PyString>() || list.is_instance_of::<PyBytes>() {
            return Err(PyTypeError::new_err(
                "enum must be an iterable of entries, not a single str or bytes",
            ));
        }
        let list = PyTuple::new(py, list.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
        for (i, entry) in list.iter().enumerate() {
            if door == core::Door::Text {
                // At the text door an entry is the text itself, packed as the ABI reads it:
                // a little-endian u32 byte length, then the bytes, surrogates kept.
                let Ok(text) = entry.cast::<PyString>() else {
                    return Err(PyTypeError::new_err(format!(
                        "enum entries at Door.TEXT must be str; entry {i} is {}",
                        entry.get_type().name()?
                    )));
                };
                str_bytes(text, |bytes| {
                    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
                    packed.extend_from_slice(&len.to_le_bytes());
                    packed.extend_from_slice(bytes);
                })?;
            } else {
                // Elsewhere a Decimal in its native 16-byte layout: lo, hi, scale, sign, two
                // bytes of padding.
                let decimal = decimal!("enum entry", &entry);
                packed.extend_from_slice(&decimal.lo.to_ne_bytes());
                packed.extend_from_slice(&decimal.hi.to_ne_bytes());
                packed.extend_from_slice(&[decimal.scale, u8::from(decimal.negative), 0, 0]);
            }
        }
        // Declared, even with no entries: the core refuses an empty `enum` (12), never reads
        // it as none.
        raw.flags |= core::RawConstraints::HAS_ENUM;
        entries = Some(list.unbind());
    }
    Ok(core::check_constraints(door, &raw, &packed).map(|()| Declaration {
        door,
        raw,
        packed,
        entries,
    }))
}

/// Validation constraints, the OpenAPI Schema Object's keywords, bound to one door and judged
/// once, by the native core, when built: a declaration that cannot mean anything at its door
/// is a ``ValueError`` here, its message the rule it breaks, and every checked cast after that
/// judges only data. Immutable.
///
/// The integer doors take ``minimum``, ``exclusive_minimum``, ``maximum``,
/// ``exclusive_maximum``, ``multiple_of`` with a ``rounding``, and ``enum``; ``Door.F32``,
/// ``Door.F64`` and ``Door.DECIMAL`` the same but ``enum``; ``Door.TEXT`` ``min_length``,
/// ``max_length`` (in code points), ``trim`` and ``enum`` (matched under simple case folding).
/// Bounds and steps are a ``decimal.Decimal``, an ``int`` or a ``str`` in invariant notation,
/// never a ``float``.
#[pyclass(frozen, generic, module = "hypercast")]
struct Constraints {
    door: core::Door,
    raw: core::RawConstraints,
    packed: Vec<u8>,
    entries: Option<Py<PyTuple>>,
}

impl Constraints {
    fn bound(
        &self,
        py: Python<'_>,
        flag: u32,
        value: core::Decimal,
    ) -> PyResult<Option<Py<PyAny>>> {
        if self.raw.flags & flag == 0 {
            return Ok(None);
        }
        decimal_value(py, value).map(Some)
    }

    /// The 72-byte layout, native-endian as the core reads it in place.
    fn layout(&self) -> Vec<u8> {
        let raw = &self.raw;
        let mut bytes = Vec::with_capacity(core::RawConstraints::SIZE as usize);
        for word in
            [raw.size, raw.flags, raw.rounding, raw.min_length, raw.max_length, raw._reserved]
        {
            bytes.extend_from_slice(&word.to_ne_bytes());
        }
        for decimal in [raw.lower, raw.upper, raw.multiple_of] {
            bytes.extend_from_slice(&decimal.lo.to_ne_bytes());
            bytes.extend_from_slice(&decimal.hi.to_ne_bytes());
            bytes.extend_from_slice(&[decimal.scale, u8::from(decimal.negative), 0, 0]);
        }
        bytes
    }
}

#[pymethods]
impl Constraints {
    #[new]
    #[pyo3(signature = (
        door, *, minimum = None, exclusive_minimum = None, maximum = None,
        exclusive_maximum = None, multiple_of = None, rounding = 0, min_length = None,
        max_length = None, trim = false, r#enum = None
    ))]
    #[allow(clippy::too_many_arguments)] // The constructor's signature is the API.
    fn new<'py>(
        py: Python<'py>,
        door: u32,
        minimum: Option<Bound<'py, PyAny>>,
        exclusive_minimum: Option<Bound<'py, PyAny>>,
        maximum: Option<Bound<'py, PyAny>>,
        exclusive_maximum: Option<Bound<'py, PyAny>>,
        multiple_of: Option<Bound<'py, PyAny>>,
        rounding: u32,
        min_length: Option<Length>,
        max_length: Option<Length>,
        trim: bool,
        r#enum: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let keywords = Keywords {
            door,
            minimum,
            exclusive_minimum,
            maximum,
            exclusive_maximum,
            multiple_of,
            rounding,
            min_length: min_length.map(|Length(length)| length),
            max_length: max_length.map(|Length(length)| length),
            trim,
            entries: r#enum,
        };
        let Declaration { door, raw, packed, entries } =
            declaration(py, keywords)?.map_err(refused)?;
        Ok(Constraints { door, raw, packed, entries })
    }

    /// The ``Door`` this declaration is bound to.
    #[getter]
    fn door(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(cached(py, &DOOR)?.call1((self.door.code(),))?.unbind())
    }

    /// The declared ``minimum``, or ``None``.
    #[getter]
    fn minimum(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.bound(py, core::RawConstraints::HAS_MINIMUM, self.raw.lower)
    }

    /// The declared ``exclusive_minimum``, or ``None``.
    #[getter]
    fn exclusive_minimum(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.bound(py, core::RawConstraints::HAS_EXCLUSIVE_MINIMUM, self.raw.lower)
    }

    /// The declared ``maximum``, or ``None``.
    #[getter]
    fn maximum(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.bound(py, core::RawConstraints::HAS_MAXIMUM, self.raw.upper)
    }

    /// The declared ``exclusive_maximum``, or ``None``.
    #[getter]
    fn exclusive_maximum(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.bound(py, core::RawConstraints::HAS_EXCLUSIVE_MAXIMUM, self.raw.upper)
    }

    /// The declared ``multiple_of``, or ``None``.
    #[getter]
    fn multiple_of(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.bound(py, core::RawConstraints::HAS_MULTIPLE_OF, self.raw.multiple_of)
    }

    /// What a value that is not a multiple of ``multiple_of`` becomes: a ``Rounding``.
    #[getter]
    fn rounding(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(cached(py, &ROUNDING)?.call1((self.raw.rounding,))?.unbind())
    }

    /// The declared ``min_length``, in code points, or ``None``.
    #[getter]
    fn min_length(&self) -> Option<u32> {
        (self.raw.flags & core::RawConstraints::HAS_MIN_LENGTH != 0).then_some(self.raw.min_length)
    }

    /// The declared ``max_length``, in code points, or ``None``.
    #[getter]
    fn max_length(&self) -> Option<u32> {
        (self.raw.flags & core::RawConstraints::HAS_MAX_LENGTH != 0).then_some(self.raw.max_length)
    }

    /// Whether text is trimmed of Unicode ``White_Space`` before it is judged.
    #[getter]
    fn trim(&self) -> bool {
        self.raw.flags & core::RawConstraints::TRIM != 0
    }

    /// The declared ``enum`` entries as the caller gave them, in a tuple, or ``None``.
    #[getter]
    fn r#enum(&self, py: Python<'_>) -> Option<Py<PyTuple>> {
        self.entries.as_ref().map(|entries| entries.clone_ref(py))
    }

    /// The 72 bytes this declaration crosses a C ABI as, in the native byte order the core
    /// reads in place: the size (``72``), the flags, the rounding code, ``min_length``,
    /// ``max_length`` and a reserved zero as ``u32``\ s, then the lower bound, the upper bound
    /// and ``multiple_of`` as 16-byte decimals (the low 64 and high 32 bits of the magnitude,
    /// the scale, the sign, two zero bytes) — for a library that judges values across a C ABI
    /// of its own, with :attr:`packed_enum` beside it.
    #[getter]
    fn packed<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.layout())
    }

    /// The ``enum`` list as the core reads it beside :attr:`packed`, empty for none (and read
    /// only when the flags in :attr:`packed` carry ``HAS_ENUM``, 256, set when one is declared):
    /// at an integer door 16-byte decimals in native byte order, at ``Door.TEXT`` each entry a
    /// little-endian ``u32`` byte length followed by its UTF-8.
    #[getter]
    fn packed_enum<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.packed)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        let Ok(other) = other.cast::<Constraints>() else {
            return false;
        };
        let other = other.get();
        self.door == other.door && self.raw == other.raw && self.packed == other.packed
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.door.code(), self.layout(), &self.packed).hash(&mut hasher);
        hasher.finish()
    }

    /// The constructor call that builds this declaration again: the door, then only the
    /// keywords it declares.
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let member = |member: Py<PyAny>| spelled(member.bind(py));
        let repr = |value: Option<Py<PyAny>>| -> PyResult<Option<String>> {
            value.map(|value| Ok(value.bind(py).repr()?.to_string())).transpose()
        };
        let declared = [
            ("minimum", repr(self.minimum(py)?)?),
            ("exclusive_minimum", repr(self.exclusive_minimum(py)?)?),
            ("maximum", repr(self.maximum(py)?)?),
            ("exclusive_maximum", repr(self.exclusive_maximum(py)?)?),
            ("multiple_of", repr(self.multiple_of(py)?)?),
            ("rounding", (self.raw.rounding != 0).then(|| member(self.rounding(py)?)).transpose()?),
            ("min_length", self.min_length().map(|length| length.to_string())),
            ("max_length", self.max_length().map(|length| length.to_string())),
            ("trim", self.trim().then(|| "True".to_string())),
            ("enum", repr(self.r#enum(py).map(Py::into_any))?),
        ];
        let mut call = format!("Constraints({}", member(self.door(py)?)?);
        for (name, value) in declared {
            if let Some(value) = value {
                call.push_str(&format!(", {name}={value}"));
            }
        }
        call.push(')');
        Ok(call)
    }
}

/// The keywords this binding names, with the constant each is published as.
const KEYWORDS: [(core::Keyword, &str); 8] = [
    (core::Keyword::Minimum, "MINIMUM"),
    (core::Keyword::ExclusiveMinimum, "EXCLUSIVE_MINIMUM"),
    (core::Keyword::Maximum, "MAXIMUM"),
    (core::Keyword::ExclusiveMaximum, "EXCLUSIVE_MAXIMUM"),
    (core::Keyword::MultipleOf, "MULTIPLE_OF"),
    (core::Keyword::MinLength, "MIN_LENGTH"),
    (core::Keyword::MaxLength, "MAX_LENGTH"),
    (core::Keyword::Enum, "ENUM"),
];

/// The rule a :class:`Violation` broke: the native core's code, and its OpenAPI spelling as
/// ``name`` (``"exclusiveMinimum"``). Open to additions: ``format`` and ``pattern`` may join in
/// a minor release, so a ``match`` over keywords needs a ``case _:`` arm, and a code this
/// binding does not name yet is kept, its ``name`` ``"keyword N"``. The constants compare
/// equal to every keyword of their code, so ``case Keyword.MINIMUM:`` matches.
#[pyclass(frozen, module = "hypercast")]
struct Keyword {
    code: u32,
}

#[pymethods]
impl Keyword {
    // Python's own spelling — the constant-style names are the API.
    #[allow(non_snake_case)]
    #[classattr]
    fn MINIMUM() -> Keyword {
        Keyword { code: core::Keyword::Minimum.code() }
    }

    #[allow(non_snake_case)]
    #[classattr]
    fn EXCLUSIVE_MINIMUM() -> Keyword {
        Keyword { code: core::Keyword::ExclusiveMinimum.code() }
    }

    #[allow(non_snake_case)]
    #[classattr]
    fn MAXIMUM() -> Keyword {
        Keyword { code: core::Keyword::Maximum.code() }
    }

    #[allow(non_snake_case)]
    #[classattr]
    fn EXCLUSIVE_MAXIMUM() -> Keyword {
        Keyword { code: core::Keyword::ExclusiveMaximum.code() }
    }

    #[allow(non_snake_case)]
    #[classattr]
    fn MULTIPLE_OF() -> Keyword {
        Keyword { code: core::Keyword::MultipleOf.code() }
    }

    #[allow(non_snake_case)]
    #[classattr]
    fn MIN_LENGTH() -> Keyword {
        Keyword { code: core::Keyword::MinLength.code() }
    }

    #[allow(non_snake_case)]
    #[classattr]
    fn MAX_LENGTH() -> Keyword {
        Keyword { code: core::Keyword::MaxLength.code() }
    }

    #[allow(non_snake_case)]
    #[classattr]
    fn ENUM() -> Keyword {
        Keyword { code: core::Keyword::Enum.code() }
    }

    #[new]
    fn new(code: u32) -> Self {
        Keyword { code }
    }

    /// The native core's code: ``1`` minimum to ``8`` enum.
    #[getter]
    fn code(&self) -> u32 {
        self.code
    }

    /// The OpenAPI spelling: ``"minimum"``, ``"exclusiveMinimum"``, … ``"enum"``; ``"keyword
    /// N"`` for a code this binding does not name.
    #[getter]
    fn name(&self) -> String {
        match KEYWORDS.iter().find(|(keyword, _)| keyword.code() == self.code) {
            Some((keyword, _)) => keyword.name().to_string(),
            None => format!("keyword {}", self.code),
        }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.cast::<Keyword>().is_ok_and(|other| other.get().code == self.code)
    }

    fn __hash__(&self) -> u64 {
        u64::from(self.code)
    }

    fn __str__(&self) -> String {
        self.name()
    }

    fn __repr__(&self) -> String {
        match KEYWORDS.iter().find(|(keyword, _)| keyword.code() == self.code) {
            Some((_, constant)) => format!("Keyword.{constant}"),
            None => format!("Keyword({})", self.code),
        }
    }
}

/// The success case of a checked door: the value (rounded to ``multiple_of`` when the
/// declaration rounds; at ``Door.TEXT`` with an ``enum``, the caller's own declared entry),
/// ``adjusted`` (true only when rounding changed it) and, at ``Door.TEXT`` with an ``enum``,
/// the index of the entry it matched — ``None`` otherwise, an integer ``enum`` included, where
/// the value is the answer.
#[pyclass(frozen, generic, module = "hypercast")]
struct Checked {
    #[pyo3(get)]
    value: Py<PyAny>,
    #[pyo3(get)]
    adjusted: bool,
    #[pyo3(get)]
    enum_index: Option<u32>,
}

#[pymethods]
impl Checked {
    #[allow(non_upper_case_globals)]
    #[classattr]
    const __match_args__: (&'static str, &'static str, &'static str) =
        ("value", "adjusted", "enum_index");

    #[new]
    #[pyo3(signature = (value, adjusted = false, enum_index = None))]
    fn new(value: Py<PyAny>, adjusted: bool, enum_index: Option<u32>) -> Self {
        Checked { value, adjusted, enum_index }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<bool> {
        let Ok(other) = other.cast::<Checked>() else {
            return Ok(false);
        };
        let other = other.get();
        Ok(self.adjusted == other.adjusted
            && self.enum_index == other.enum_index
            && self.value.bind(py).eq(other.value.bind(py))?)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "Checked(value={}, adjusted={}, enum_index={})",
            self.value.bind(py).repr()?,
            if self.adjusted { "True" } else { "False" },
            self.enum_index.map_or_else(|| "None".to_string(), |index| index.to_string())
        ))
    }
}

/// A value that parsed and broke a declared rule: the :class:`Keyword` and the value's span,
/// in the caller's own units as a :class:`Fault`'s is (byte offsets for ``bytes`` input,
/// code-point offsets for ``str``). A number already decoded has no text, and an empty span.
#[pyclass(frozen, module = "hypercast")]
struct Violation {
    #[pyo3(get)]
    keyword: Py<Keyword>,
    #[pyo3(get)]
    offset: u32,
    #[pyo3(get)]
    length: u32,
}

#[pymethods]
impl Violation {
    #[allow(non_upper_case_globals)]
    #[classattr]
    const __match_args__: (&'static str, &'static str, &'static str) =
        ("keyword", "offset", "length");

    #[new]
    fn new(keyword: Py<Keyword>, offset: u32, length: u32) -> Self {
        Violation { keyword, offset, length }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        let Ok(other) = other.cast::<Violation>() else {
            return false;
        };
        let other = other.get();
        self.offset == other.offset
            && self.length == other.length
            && self.keyword.get().code == other.keyword.get().code
    }

    fn __repr__(&self) -> String {
        format!(
            "Violation(keyword={}, offset={}, length={})",
            self.keyword.get().__repr__(),
            self.offset,
            self.length
        )
    }
}

/// What a checked judge answers: the value and what was reported beside it, or a rejection.
type Judgement<T> = Result<(T, CheckOutcome), core::detailed::Rejection>;

/// Runs a numeric judge, which writes the value to the pointer it is handed.
///
/// # Safety
/// `T` must be the type the judge writes on success.
unsafe fn judged<T>(
    judge: impl FnOnce(*mut u8) -> Option<Result<CheckOutcome, core::detailed::Rejection>>,
) -> PyResult<Judgement<T>> {
    let mut out = MaybeUninit::<T>::uninit();
    match judge(out.as_mut_ptr().cast()) {
        // SAFETY: on success the judge wrote a `T`, per the function contract.
        Some(Ok(outcome)) => Ok(Ok((unsafe { out.assume_init() }, outcome))),
        Some(Err(rejection)) => Ok(Err(rejection)),
        None => Err(binding_bug("refused a declaration it had accepted")),
    }
}

/// Judges at a numeric door through `judge`, which writes that door's own type, and presents
/// the value as Python's: an ``int``, a ``float`` (an f32 widened losslessly) or an exact
/// ``decimal.Decimal``.
fn numeric(
    py: Python<'_>,
    door: core::Door,
    judge: impl FnOnce(*mut u8) -> Option<Result<CheckOutcome, core::detailed::Rejection>>,
) -> PyResult<Judgement<Py<PyAny>>> {
    macro_rules! at {
        ($ty:ty) => {{
            // SAFETY: the door judged is `door`, which writes this type.
            match unsafe { judged::<$ty>(judge) }? {
                Ok((value, outcome)) => {
                    Ok(Ok((value.into_pyobject(py)?.into_any().unbind(), outcome)))
                }
                Err(rejection) => Ok(Err(rejection)),
            }
        }};
    }
    match door {
        core::Door::I8 => at!(i8),
        core::Door::I16 => at!(i16),
        core::Door::I32 => at!(i32),
        core::Door::I64 => at!(i64),
        core::Door::U8 => at!(u8),
        core::Door::U16 => at!(u16),
        core::Door::U32 => at!(u32),
        core::Door::U64 => at!(u64),
        core::Door::F32 => at!(f32),
        core::Door::F64 => at!(f64),
        // SAFETY: the decimal door writes a `Decimal`.
        core::Door::Decimal => match unsafe { judged::<core::Decimal>(judge) }? {
            Ok((value, outcome)) => Ok(Ok((decimal_value(py, value)?, outcome))),
            Err(rejection) => Ok(Err(rejection)),
        },
        _ => Err(binding_bug("was asked to judge a door that is not numeric")),
    }
}

/// A checked verdict: :class:`Checked`, or the rejection, its span in the caller's units.
fn checked(
    py: Python<'_>,
    input: Option<&Input<'_>>,
    judgement: Judgement<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    let span = |offset, len| match input {
        Some(input) => input.span(offset, len),
        None => Ok((offset, len)),
    };
    match judgement {
        Ok((value, outcome)) => Ok(Py::new(
            py,
            Checked { value, adjusted: outcome.adjusted, enum_index: outcome.enum_index },
        )?
        .into_any()),
        Err(core::detailed::Rejection::Failure(failed)) => {
            let (offset, len) = span(failed.offset, failed.len)?;
            fault(py, core::Failure { offset, len, ..failed })
        }
        Err(core::detailed::Rejection::Violation(broken)) => {
            let (offset, length) = span(broken.offset, broken.len)?;
            let keyword = Py::new(py, Keyword { code: broken.keyword.code() })?;
            Ok(Py::new(py, Violation { keyword, offset, length })?.into_any())
        }
    }
}

/// An IntEnum member as a call spells it, `Door.F64`, rather than its `<Door.F64: 11>`.
fn spelled(member: &Bound<'_, PyAny>) -> PyResult<String> {
    let name = member.getattr(intern!(member.py(), "name"))?;
    Ok(format!("{}.{name}", member.get_type().name()?))
}

/// A declaration handed to the wrong door: a caller's bug.
fn wrong_door(py: Python<'_>, constraints: &Constraints, door: &str) -> PyResult<PyErr> {
    let bound = spelled(constraints.door(py)?.bind(py))?;
    Ok(PyValueError::new_err(format!("Constraints for {bound} are judged by {door}")))
}

/// Casts numeric text through the door its ``Constraints`` are bound to, under the declared
/// format, then judges the value by them: :class:`Checked`, or the door's own :class:`Fault`
/// or a :class:`Violation` of the first rule it breaks. The value is judged exactly, on the
/// decimal the door read, never on a float. Spans are in the caller's units, as a fault's are.
/// ``Constraints`` for ``Door.TEXT`` go to :func:`cast_text`; handing them here is a
/// ``ValueError``.
#[pyfunction]
fn cast_checked(
    py: Python<'_>,
    text: Text<'_>,
    fmt: PyRef<'_, NumFormat>,
    constraints: PyRef<'_, Constraints>,
) -> PyResult<Py<PyAny>> {
    let door = constraints.door;
    if door == core::Door::Text {
        return Err(wrong_door(py, &constraints, "cast_text")?);
    }
    let input = Input::new(text)?;
    let bytes = input.bytes()?;
    let entries = Entries::packed(door, &constraints.packed);
    let judgement = numeric(py, door, |out| {
        // SAFETY: `numeric` hands a pointer to the type `door` writes.
        unsafe {
            judge_numeric_detailed(door, bytes, fmt.format(), &constraints.raw, &entries, out)
        }
        .map(|verdict| verdict.map_err(|rejection| core::detailed::classified(rejection, bytes)))
    })?;
    checked(py, Some(&input), judgement)
}

/// Reads a number a source already decoded (a JSON parser's ``float``, a spreadsheet's
/// numeric cell) through the door its ``Constraints`` are bound to, then judges it by them on
/// the shortest decimal that names it: ``0.1 + 0.2`` is ``0.30000000000000004``, which breaks
/// ``multiple_of="0.1"`` unrounded and rounds to ``0.3`` under ``Rounding.MIDPOINT``. NaN is
/// ``MALFORMED``; an infinity is ``OUT_OF_RANGE`` at a float or decimal door and ``MALFORMED``
/// at an integer door. Every fault and violation has an empty span.
#[pyfunction]
fn cast_checked_from_float(
    py: Python<'_>,
    value: f64,
    constraints: PyRef<'_, Constraints>,
) -> PyResult<Py<PyAny>> {
    let door = constraints.door;
    if door == core::Door::Text {
        return Err(wrong_door(py, &constraints, "cast_text")?);
    }
    let entries = Entries::packed(door, &constraints.packed);
    let judgement = numeric(py, door, |out| {
        // SAFETY: `numeric` hands a pointer to the type `door` writes.
        unsafe { judge_numeric_from_f64_detailed(door, value, &constraints.raw, &entries, out) }
    })?;
    checked(py, None, judgement)
}

/// Casts text as text: the input itself, as a ``str``, once it is valid text — ``bytes`` must
/// be UTF-8, and a ``str`` holding a lone surrogate is ``MALFORMED`` at it. Empty input is
/// ``EMPTY``; nothing is trimmed. With ``Constraints`` for ``Door.TEXT`` it is judged by them:
/// trimmed of Unicode ``White_Space`` when they say so (trimmed to nothing is ``EMPTY``),
/// lengths counted in code points, and an ``enum`` matched under simple case folding, a match
/// delivering the caller's own declared entry and its index — a :class:`Checked`, a
/// :class:`Fault` or a :class:`Violation`.
#[pyfunction]
#[pyo3(signature = (text, constraints = None))]
fn cast_text(
    py: Python<'_>,
    text: Text<'_>,
    constraints: Option<PyRef<'_, Constraints>>,
) -> PyResult<Py<PyAny>> {
    let input = Input::new(text)?;
    let bytes = input.bytes()?;
    let Some(constraints) = constraints else {
        return match core::detailed::cast_text(bytes) {
            Ok(valid) => {
                // The door refuses text past `u32::MAX` bytes, so the length narrows exactly.
                let value = input.text_at(py, (0, valid.len() as u32))?;
                Ok(Py::new(py, Success { value })?.into_any())
            }
            Err(failed) => {
                let (offset, len) = input.span(failed.offset, failed.len)?;
                fault(py, core::Failure { offset, len, ..failed })
            }
        };
    };
    if constraints.door != core::Door::Text {
        return Err(wrong_door(py, &constraints, "cast_checked")?);
    }
    let entries = Entries::packed(core::Door::Text, &constraints.packed);
    let judgement = match judge_text_detailed(bytes, &constraints.raw, &entries) {
        None => return Err(binding_bug("refused a declaration it had accepted")),
        Some(Err(rejection)) => Err(rejection),
        Some(Ok(outcome)) => {
            let value = match (outcome.enum_index, &constraints.entries) {
                (None, _) => input.text_at(py, outcome.span)?,
                (Some(index), Some(entries)) => entries.bind(py).get_item(index as usize)?.unbind(),
                (Some(_), None) => return Err(binding_bug("matched an enum never declared")),
            };
            Ok((value, outcome))
        }
    };
    checked(py, Some(&input), judgement)
}

/// Judges a declaration without building it, the way ``Constraints`` does when it is built:
/// ``0`` when every rule holds, otherwise the code of the first rule it breaks — ``1`` a
/// keyword the door does not take, ``2`` minimum with exclusive_minimum, ``3`` maximum with
/// exclusive_maximum, ``4`` a lower bound not below the upper, ``5`` multiple_of not positive,
/// ``6`` a rounding without multiple_of, ``7`` min_length above max_length, ``8`` a bound, step
/// or entry the door cannot hold, ``9`` an entry that is not UTF-8 (a lone surrogate), ``10``
/// a duplicate entry, ``11`` a rounding code the core does not know, ``12`` an ``enum`` with no
/// entries (``enum=[]``; ``None`` declares none), ``13`` a text entry no input can match (an
/// empty one, or under ``trim`` one with leading or trailing white space). A wrong argument
/// type is still a ``TypeError``, and a door that is not a ``Door`` a ``ValueError``.
#[pyfunction]
#[pyo3(signature = (
    door, *, minimum = None, exclusive_minimum = None, maximum = None,
    exclusive_maximum = None, multiple_of = None, rounding = 0, min_length = None,
    max_length = None, trim = false, r#enum = None
))]
#[allow(clippy::too_many_arguments)] // The signature is Constraints's, the API.
fn cast_constraints_check<'py>(
    py: Python<'py>,
    door: u32,
    minimum: Option<Bound<'py, PyAny>>,
    exclusive_minimum: Option<Bound<'py, PyAny>>,
    maximum: Option<Bound<'py, PyAny>>,
    exclusive_maximum: Option<Bound<'py, PyAny>>,
    multiple_of: Option<Bound<'py, PyAny>>,
    rounding: u32,
    min_length: Option<Length>,
    max_length: Option<Length>,
    trim: bool,
    r#enum: Option<Bound<'py, PyAny>>,
) -> PyResult<u32> {
    let keywords = Keywords {
        door,
        minimum,
        exclusive_minimum,
        maximum,
        exclusive_maximum,
        multiple_of,
        rounding,
        min_length: min_length.map(|Length(length)| length),
        max_length: max_length.map(|Length(length)| length),
        trim,
        entries: r#enum,
    };
    Ok(declaration(py, keywords)?.map_or_else(|refusal| refusal.code(), |_| 0))
}

/// Converts a bound, step or integer ``enum`` entry written in invariant notation (an
/// optional sign, digits with an optional ``.`` and fraction, an optional exponent:
/// ``"0.000001"``, ``"-90"``, ``"1e-6"``) to the exact ``decimal.Decimal`` the core declares
/// it as, trailing zeros dropped: the conversion every bound a ``Constraints`` is given
/// crosses. A value past 96 bits or 28 places, or text in any other notation, is the
/// ``ValueError`` a refused declaration raises (code 8).
#[pyfunction]
fn cast_declared_decimal(py: Python<'_>, text: Text<'_>) -> PyResult<Py<PyAny>> {
    let input = Input::new(text)?;
    decimal_value(py, declared_decimal(input.bytes()?).map_err(refused)?)
}

/// Hands the package's own `CastFailure` IntEnum (plus `uuid.UUID` and `decimal.Decimal`)
/// to this backend so faults carry the exact members callers compare with `is`, its
/// `Door` and `Rounding` IntEnums, which a declaration's getters answer with, and its
/// `Detail`, which a fault's `detail` answers with.
#[pyfunction]
fn _bind(
    py: Python<'_>,
    cast_failure: Bound<'_, PyAny>,
    door: Bound<'_, PyAny>,
    rounding: Bound<'_, PyAny>,
    detail: Bound<'_, PyAny>,
) -> PyResult<()> {
    let _ = EMPTY.set(cast_failure.getattr("EMPTY")?.unbind());
    let _ = MALFORMED.set(cast_failure.getattr("MALFORMED")?.unbind());
    let _ = OUT_OF_RANGE.set(cast_failure.getattr("OUT_OF_RANGE")?.unbind());
    let _ = VALUES.set(crate::python::Values::import(py)?);
    let _ = DOOR.set(door.unbind());
    let _ = ROUNDING.set(rounding.unbind());
    let _ = DETAIL.set(detail.unbind());
    let _ = DECIMAL.set(py.import("decimal")?.getattr("Decimal")?.unbind());
    Ok(())
}

/// The core's own name for a detail code (``"blank"``, ``"group_size"``), or ``None`` for a
/// code it does not publish: what the package's ``Detail`` members are held to.
#[pyfunction]
fn _detail_name(code: u32) -> Option<&'static str> {
    core::Detail::from_code(code).map(core::Detail::name)
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
    m.add_class::<DateOptions>()?;
    m.add_class::<MonthNames>()?;
    m.add_class::<Constraints>()?;
    m.add_class::<Checked>()?;
    m.add_class::<Violation>()?;
    m.add_class::<Keyword>()?;
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
    m.add_function(wrap_pyfunction!(cast_text, m)?)?;
    m.add_function(wrap_pyfunction!(cast_checked, m)?)?;
    m.add_function(wrap_pyfunction!(cast_checked_from_float, m)?)?;
    m.add_function(wrap_pyfunction!(cast_constraints_check, m)?)?;
    m.add_function(wrap_pyfunction!(cast_declared_decimal, m)?)?;
    m.add_function(wrap_pyfunction!(native_version, m)?)?;
    m.add_function(wrap_pyfunction!(_bind, m)?)?;
    m.add_function(wrap_pyfunction!(_detail_name, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The backstop is a real check, not a debug assertion: a format whose decimal separator
    /// holds white space, built from the core's public fields so none of `declare`'s per-field
    /// validators ran, is refused, and `cargo test --release` holds it to the same answer.
    #[test]
    fn the_backstop_refuses_what_the_core_refuses_in_every_build() {
        let lagging = core::NumFormat {
            decimal_sep: core::Symbol::new("\u{a0}").expect("a symbol"),
            ..core::NumFormat::INVARIANT
        };
        assert!(!lagging.check());
        assert!(refuse_unless_checked(&lagging).is_err());
        assert!(refuse_unless_checked(&core::NumFormat::INVARIANT).is_ok());
        // Nor does a declaration the 1.1 flag rules refuse get through it.
        let policy =
            core::NumFormat::new('.', ',', core::NumFormat::ALL | core::NumFormat::GROUPING_POLICY);
        assert!(refuse_unless_checked(&policy).is_err());
    }
}
