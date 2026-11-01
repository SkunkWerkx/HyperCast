//! The exceptions the binding throws, with 1.0's classes and messages, and the way back from a
//! PHP call that threw.

use ext_php_rs::exception::PhpException;
use ext_php_rs::zend::{ClassEntry, ExecutorGlobals, ce};

// SPL's class entries, read from the PHP binary's exported data symbols. Not
// `ClassEntry::try_find`: it returns `None` while `ExecutorGlobals::class_table()` is null,
// which it is at MINIT. A data symbol imported from a DLL needs the import library on Windows,
// as ext-php-rs's windows_build.rs adds to its own bindgen blocks; that name is `php8` for the
// NTS release build this crate ships (debug and ZTS builds use other names).
#[cfg_attr(windows, link(name = "php8"))]
unsafe extern "C" {
    static spl_ce_RuntimeException: *mut ext_php_rs::ffi::zend_class_entry;
    static spl_ce_InvalidArgumentException: *mut ext_php_rs::ffi::zend_class_entry;
    static spl_ce_LogicException: *mut ext_php_rs::ffi::zend_class_entry;
}

/// `\RuntimeException`'s class entry.
pub(crate) fn runtime_exception() -> &'static ClassEntry {
    // SAFETY: SPL writes the pointer once, in its own MINIT, before shared extensions start, so
    // this unsynchronised read never races a write.
    unsafe { spl_ce_RuntimeException.as_ref() }.expect("SPL's RuntimeException is registered")
}

/// `\InvalidArgumentException`'s class entry.
fn invalid_argument_exception() -> &'static ClassEntry {
    // SAFETY: as for `runtime_exception`.
    unsafe { spl_ce_InvalidArgumentException.as_ref() }
        .expect("SPL's InvalidArgumentException is registered")
}

/// `\LogicException`'s class entry.
fn logic_exception() -> &'static ClassEntry {
    // SAFETY: as for `runtime_exception`.
    unsafe { spl_ce_LogicException.as_ref() }.expect("SPL's LogicException is registered")
}

/// A `\RuntimeException`.
pub(crate) fn runtime(message: impl Into<String>) -> PhpException {
    PhpException::new(message.into(), 0, runtime_exception())
}

/// An `\InvalidArgumentException`.
pub(crate) fn invalid_argument(message: impl Into<String>) -> PhpException {
    PhpException::new(message.into(), 0, invalid_argument_exception())
}

/// A `\LogicException`.
pub(crate) fn logic(message: impl Into<String>) -> PhpException {
    PhpException::new(message.into(), 0, logic_exception())
}

/// An `\Error`.
pub(crate) fn error(message: impl Into<String>) -> PhpException {
    PhpException::new(message.into(), 0, ce::error())
}

/// A `\TypeError`.
pub(crate) fn type_error(message: impl Into<String>) -> PhpException {
    PhpException::new(message.into(), 0, ce::type_error())
}

/// What 1.0's `Cast::fail` threw for the core's `-1`: a declared format or set of day periods
/// the core refuses, which only an object built around the constructor (`unserialize`, say)
/// can hold.
pub(crate) fn contract_violation() -> PhpException {
    runtime(
        "hypercast: libhypercast reported a contract violation — a binding bug, please report it",
    )
}

/// The exception a PHP call this binding made has left pending: returned as the error, it
/// throws nothing more (`PhpException::throw` leaves a pending exception alone), so the
/// original propagates untouched. Without one pending, a `\RuntimeException` saying what failed.
pub(crate) fn pending(what: &str) -> PhpException {
    if ExecutorGlobals::has_exception() {
        // Never thrown: the pending exception stands.
        PhpException::new(String::new(), 0, ce::exception())
    } else {
        runtime(format!("hypercast: {what} failed"))
    }
}
