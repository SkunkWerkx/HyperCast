//! The parts of the 1.0 surface that ext-php-rs's builders can't declare, written into the
//! registered class entries once every module's startup has run:
//!
//! - `CastFailure`, `DateOrder`, `ExcelEpoch`, `UnixPrecision`, `Door`, `Rounding` and
//!   `Keyword` are `final`, as every PHP enum is: ext-php-rs registers an enum through
//!   `zend_register_internal_enum` without the flag the compiler gives a declared one;
//! - `Cast`, `Interop\NativeValues` and `Interop\NativePlatform` accept `serialize` and
//!   `unserialize`, as 1.0's userland classes did: ext-php-rs marks a class backed by a Rust
//!   object `NotSerializable`, and these hold nothing to lose.
//!
//! `#[php_module]` runs a module's own startup hook before it registers any class, and no hook
//! follows registration, so a persistent load chains `zend_post_startup_cb`, which
//! `zend_post_startup` calls once, after every module's MINIT and before opcache's preload and
//! the first request (it clears the pointer before calling it). A `dl()` load comes after that
//! point and gets no post-startup call; its RINIT, which `dl()` runs straight after MINIT,
//! finishes instead ([`finish_late`]).
//!
//! `tests/SurfaceTest.php` holds the result to 1.0's surface.

use std::panic::catch_unwind;
use std::ptr;
use std::sync::Mutex;

use ext_php_rs::class::RegisteredClass;
use ext_php_rs::ffi::{ZEND_ACC_FINAL, zend_class_entry};
use ext_php_rs::flags::ClassFlags;

use super::cast::Cast;
use super::enums::{CastFailure, DateOrder, Door, ExcelEpoch, Keyword, Rounding, UnixPrecision};
use super::interop::{NativePlatform, NativeValues};

// The engine's hook for work after every MINIT and before any request (`zend.h`), which
// bindgen doesn't carry. Read and written only during startup, on the one startup thread.
#[cfg_attr(windows, link(name = "php8"))]
unsafe extern "C" {
    static mut zend_post_startup_cb: Option<unsafe extern "C" fn() -> i32>;
}

/// The post-startup callback that was installed before ours, called after ours.
static PREVIOUS_POST_STARTUP: Mutex<Option<unsafe extern "C" fn() -> i32>> = Mutex::new(None);

/// `MODULE_PERSISTENT` (`zend_modules.h`): loaded at startup, as opposed to by `dl()`.
const MODULE_PERSISTENT: i32 = 1;

/// Arranges for [`finish`] to run before any request. Called from MINIT.
pub(crate) fn schedule(module_type: i32) {
    if module_type != MODULE_PERSISTENT {
        return;
    }
    // SAFETY: MINIT runs on the startup thread before any request, the only time the engine
    // or another extension touches this pointer.
    let previous = unsafe { ptr::replace(&raw mut zend_post_startup_cb, Some(post_startup)) };
    if let Ok(mut slot) = PREVIOUS_POST_STARTUP.lock() {
        *slot = previous;
    }
}

/// Finishes the registration, then runs whatever post-startup callback was installed before.
unsafe extern "C" fn post_startup() -> i32 {
    // A panic must not unwind into the engine; failing to finish leaves a surface short of
    // 1.0's, which the tests catch.
    let _ = catch_unwind(finish);
    let previous = PREVIOUS_POST_STARTUP.lock().ok().and_then(|mut slot| slot.take());
    match previous {
        // SAFETY: the callback the engine held before ours, called as the engine would have.
        Some(previous) => unsafe { previous() },
        None => 0,
    }
}

/// RINIT's fallback for a `dl()` load, which gets no post-startup call. Each step checks the
/// entry's state first, so after a startup load this only reads.
pub(crate) fn finish_late() {
    let _ = catch_unwind(|| {
        if Cast::get_metadata().ce().flags().contains(ClassFlags::NotSerializable) {
            finish();
        }
    });
}

/// Writes the flags. Each write checks the entry's current state, so a second call changes
/// nothing.
fn finish() {
    for ce in [
        own_entry::<CastFailure>(),
        own_entry::<DateOrder>(),
        own_entry::<ExcelEpoch>(),
        own_entry::<UnixPrecision>(),
        own_entry::<Door>(),
        own_entry::<Rounding>(),
        own_entry::<Keyword>(),
    ]
    .into_iter()
    .flatten()
    {
        // SAFETY: a live, registered class entry, written before any request on the startup
        // thread (or in a `dl()` load's own RINIT; NTS only).
        unsafe { (*ce).ce_flags |= ZEND_ACC_FINAL };
    }
    for ce in [own_entry::<Cast>(), own_entry::<NativeValues>(), own_entry::<NativePlatform>()]
        .into_iter()
        .flatten()
    {
        // SAFETY: as above.
        unsafe { (*ce).ce_flags &= !ClassFlags::NotSerializable.bits() };
    }
}

/// The class entry ext-php-rs registered for `T`, found in the compiler's class table
/// (`CG(class_table)`, complete once every MINIT has run; the executor's isn't set up yet at
/// post-startup), or `None` if a namesake took it.
fn own_entry<T: RegisteredClass>() -> Option<*mut zend_class_entry> {
    // SAFETY: the compiler globals live for the process; the class table is complete once
    // every MINIT has run, and nothing else writes it during startup or RINIT.
    let table = unsafe { ext_php_rs::ffi::ext_php_rs_compiler_globals().as_ref() }
        .and_then(|globals| unsafe { globals.class_table.as_ref() })?;
    // SAFETY: the class table's values are `IS_PTR` zvals holding class entries.
    let found = table
        .get(T::CLASS_NAME.to_ascii_lowercase().as_str())
        .and_then(|zv| unsafe { zv.ptr::<zend_class_entry>() })?;
    ptr::eq(found.cast_const(), T::get_metadata().ce()).then_some(found)
}
