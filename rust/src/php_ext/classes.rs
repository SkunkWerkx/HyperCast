//! The value classes' machinery: plain PHP classes registered in MINIT, their typed readonly
//! properties, their objects, and calls back into PHP.
//!
//! 1.0 declared `Success`, `Fault`, `Decimal`, `Duration`, `NumFormat` and `DayPeriods` as
//! `final readonly` classes whose constructors promote every parameter to a public readonly
//! property. They are registered here the way the engine registers its own internal classes
//! from a stub (`ClassBuilder` with no object override, then `zend_declare_typed_property`),
//! so their objects are standard objects: the engine serializes, compares, clones, dumps and
//! exports them, and refuses every write, unset and dynamic property, with its own messages.
//! A constructor called from PHP writes through the engine (`zend_update_property`), so a
//! second `__construct` call is refused as 1.0's promoted assignment was; a value the binding
//! builds itself is written straight into its fresh object's slots.

use std::ffi::c_char;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

use ext_php_rs::args::ArgInfoTables;
use ext_php_rs::builders::{ClassBuilder, FunctionBuilder};
use ext_php_rs::exception::{PhpException, PhpResult};
use ext_php_rs::ffi::{
    _ZEND_TYPE_NAME_BIT, _call_user_function_impl, IS_ARRAY, IS_FALSE, IS_LONG, IS_NULL,
    IS_OBJECT_EX, IS_STRING, IS_TRUE, IS_UNDEF, MAY_BE_ANY, ZEND_ACC_PROMOTED, ZEND_ACC_PUBLIC,
    object_properties_init, zend_class_entry, zend_function, zend_object, zend_objects_new,
    zend_read_property, zend_string, zend_type, zval,
};
use ext_php_rs::flags::{ClassFlags, ConstantFlags, MethodFlags};
use ext_php_rs::types::{ZendObject, ZendStr, Zval};
use ext_php_rs::zend::{ClassEntry, ExecuteData, ExecutorGlobals};

use super::errors::{error, pending};

// Engine functions bindgen doesn't carry (`zend_API.h`, PHP 8.3 to 8.5).
#[cfg_attr(windows, link(name = "php8"))]
unsafe extern "C" {
    fn zend_declare_typed_property(
        ce: *mut zend_class_entry,
        name: *mut zend_string,
        property: *mut zval,
        access_type: i32,
        doc_comment: *mut zend_string,
        type_: zend_type,
    ) -> *mut std::ffi::c_void;
    fn zend_update_property(
        scope: *mut zend_class_entry,
        object: *mut zend_object,
        name: *const c_char,
        name_length: usize,
        value: *mut zval,
    );
}

/// `ZEND_ACC_READONLY` (`zend_compile.h`), for a property.
const ACC_READONLY: u32 = 1 << 7;

/// A value class, by its slot in [`ENTRIES`].
#[derive(Clone, Copy)]
pub(crate) enum Value {
    Success = 0,
    Fault = 1,
    Decimal = 2,
    Duration = 3,
    NumFormat = 4,
    DayPeriods = 5,
    Constraints = 6,
    Checked = 7,
    Violation = 8,
}

/// The value classes' entries, written once each, in MINIT.
static ENTRIES: [AtomicPtr<ClassEntry>; 9] = [
    AtomicPtr::new(ptr::null_mut()),
    AtomicPtr::new(ptr::null_mut()),
    AtomicPtr::new(ptr::null_mut()),
    AtomicPtr::new(ptr::null_mut()),
    AtomicPtr::new(ptr::null_mut()),
    AtomicPtr::new(ptr::null_mut()),
    AtomicPtr::new(ptr::null_mut()),
    AtomicPtr::new(ptr::null_mut()),
    AtomicPtr::new(ptr::null_mut()),
];

/// A property's declared type.
#[derive(Clone, Copy)]
pub(crate) enum Type {
    Mixed,
    Int,
    String,
    Bool,
    Class(&'static str),
    /// `?int`.
    NullableInt,
    /// `?array`.
    NullableArray,
    /// `?Class`.
    NullableClass(&'static str),
    /// `Class|int`.
    ClassOrInt(&'static str),
}

/// A value class's declaration: its name, its promoted properties in declaration order, and
/// its constants.
pub(crate) struct Declaration {
    pub(crate) name: &'static str,
    pub(crate) properties: &'static [(&'static str, Type)],
    pub(crate) constants: &'static [(&'static str, i64)],
    /// Whether the properties are the constructor's promoted parameters (1.0's classes, and
    /// every class whose properties hold what the constructor was given), or assigned by a
    /// constructor that takes them in another form (`Constraints`).
    pub(crate) promoted: bool,
}

/// The class entry of `value`; registered in MINIT, so present whenever PHP calls in.
pub(crate) fn entry(value: Value) -> *mut zend_class_entry {
    ENTRIES[value as usize].load(Ordering::Acquire).cast::<zend_class_entry>()
}

/// Records the class entry `register` returned in `value`'s slot and declares its properties.
pub(crate) fn finish_registration(
    value: Value,
    declaration: &Declaration,
    ce: &'static mut ClassEntry,
) {
    for (name, ty) in declaration.properties {
        declare(ce, name, *ty, declaration.promoted);
    }
    ENTRIES[value as usize].store(ce, Ordering::Release);
}

/// Registers a value class: final, readonly, closed to dynamic properties, with `methods`,
/// then (in `registration`, which must call [`finish_registration`]) its properties.
pub(crate) fn register(
    declaration: &Declaration,
    methods: Vec<(FunctionBuilder<'static>, MethodFlags)>,
    registration: fn(&'static mut ClassEntry, ArgInfoTables),
) -> ext_php_rs::error::Result<()> {
    let mut builder = ClassBuilder::new(declaration.name)
        .flags(ClassFlags::Final | ClassFlags::ReadonlyClass | ClassFlags::NoDynamicProperties);
    for (function, flags) in methods {
        builder = builder.method(function, flags);
    }
    for (name, value) in declaration.constants {
        builder = builder.constant(*name, *value, &[], ConstantFlags::Public)?;
    }
    builder.registration(registration).register()
}

/// Declares the public readonly property `name` of type `ty` on `ce`, with no default, as the
/// compiler declares a promoted constructor parameter (or, not `promoted`, a property the
/// constructor assigns).
fn declare(ce: &mut ClassEntry, name: &str, ty: Type, promoted: bool) {
    let mask = match ty {
        Type::Mixed => MAY_BE_ANY,
        Type::Int => 1 << IS_LONG,
        Type::String => 1 << IS_STRING,
        Type::Bool => (1 << IS_FALSE) | (1 << IS_TRUE),
        Type::Class(_) => _ZEND_TYPE_NAME_BIT,
        Type::NullableInt => (1 << IS_LONG) | (1 << IS_NULL),
        Type::NullableArray => (1 << IS_ARRAY) | (1 << IS_NULL),
        Type::NullableClass(_) => _ZEND_TYPE_NAME_BIT | (1 << IS_NULL),
        Type::ClassOrInt(_) => _ZEND_TYPE_NAME_BIT | (1 << IS_LONG),
    };
    let type_ptr = match ty {
        // An internal class's type names a persistent interned string, as a stub's does.
        Type::Class(class) | Type::NullableClass(class) | Type::ClassOrInt(class) => {
            ZendStr::new_interned(class, true).into_raw().cast()
        }
        _ => ptr::null_mut(),
    };
    let name = ZendStr::new_interned(name, true).into_raw();
    let mut default = Zval::new();
    default.u1.type_info = IS_UNDEF;
    // SAFETY: MINIT, on the class `register` just created; the engine keeps the interned name
    // and type, and copies the (undefined) default.
    unsafe {
        zend_declare_typed_property(
            ce,
            name,
            &raw mut default,
            (ZEND_ACC_PUBLIC | ACC_READONLY | if promoted { ZEND_ACC_PROMOTED } else { 0 }) as i32,
            ptr::null_mut(),
            zend_type { ptr: type_ptr, type_mask: mask },
        );
    }
}

/// Adds the type bits `bits` to the parameter at `position` of the registered method `method`
/// (lowercase) of `ce`: the engine's own copy of its argument info, which the engine made
/// when it registered the method and reflection, the argument checks and the error messages
/// read. For the types ext-php-rs's builders can't declare: a class beside scalars
/// (`Keyword|int`, `int|string|Decimal|null`). MINIT, before any request. `false` when the
/// method or the parameter is missing, which the caller turns into a failed registration (so
/// MINIT fails and the extension never loads declaring a narrower type than it takes).
#[must_use]
pub(crate) fn widen(ce: &ClassEntry, method: &str, position: usize, bits: u32) -> bool {
    // SAFETY: a class's function table holds `IS_PTR` zvals of its methods.
    let found = ce.function_table.get(method).and_then(|zv| unsafe { zv.ptr::<zend_function>() });
    let Some(function) = found else {
        debug_assert!(false, "widen: {method} is not registered");
        return false;
    };
    // SAFETY: a method `zend_register_functions` just registered: an internal function whose
    // argument info, `num_args` entries of it, the engine copied into memory of its own when
    // it resolved the class names, and which nothing else reads or writes during MINIT.
    unsafe {
        let internal = &mut (*function).internal_function;
        if position >= internal.num_args as usize || internal.arg_info.is_null() {
            debug_assert!(false, "widen: {method} has no parameter {position}");
            return false;
        }
        (*internal.arg_info.add(position)).type_.type_mask |= bits;
    }
    true
}

/// A new object of `value`'s class with its properties uninitialized, as `new` makes one before
/// the constructor runs.
pub(crate) fn instantiate(value: Value) -> PhpResult<*mut zend_object> {
    let ce = entry(value);
    if ce.is_null() {
        return Err(super::errors::runtime("hypercast: class not registered"));
    }
    // SAFETY: a registered class with the standard object handlers.
    unsafe {
        let object = zend_objects_new(ce);
        object_properties_init(object, ce);
        Ok(object)
    }
}

/// A zval holding `object`, taking over the reference `instantiate` returned.
pub(crate) fn object_zval(object: *mut zend_object) -> Zval {
    let mut zv = Zval::new();
    zv.value.obj = object;
    zv.u1.type_info = IS_OBJECT_EX;
    zv
}

/// Builds a value of `value`'s class from `values`, one per property in declaration order,
/// written straight into the fresh object's slots.
pub(crate) fn build(value: Value, values: Vec<Zval>) -> PhpResult<Zval> {
    let object = instantiate(value)?;
    // SAFETY: a fresh object of the class: its property slots, `default_properties_count` of
    // them, are undefined, so each write takes over the zval's reference and leaves nothing to
    // release. The slot's extra bits are the property flags (`Z_PROP_FLAG_P`), cleared as an
    // initializing assignment clears them.
    unsafe {
        let count = (*(*object).ce).default_properties_count as usize;
        let table = (*object).properties_table.as_mut_ptr();
        for (i, value) in values.into_iter().enumerate().take(count) {
            let slot = table.add(i);
            ptr::write(slot, value);
            (*slot).u2.extra = 0;
        }
    }
    Ok(object_zval(object))
}

/// `$this` of the running method.
pub(crate) fn this(ex: &mut ExecuteData) -> PhpResult<*mut zend_object> {
    ex.This
        .object_mut()
        .map(ptr::from_mut)
        .map(|object: *mut ZendObject| object.cast::<zend_object>())
        .ok_or_else(|| error("Using $this when not in object context"))
}

/// Assigns `$this->name = value` from inside the class, as 1.0's promoted parameters did: the
/// engine checks the readonly rules and the type.
pub(crate) fn assign(
    value: Value,
    object: *mut zend_object,
    name: &str,
    mut new: Zval,
) -> PhpResult<()> {
    // SAFETY: a live object of the class, and a class entry registered in MINIT.
    unsafe {
        zend_update_property(entry(value), object, name.as_ptr().cast(), name.len(), &raw mut new);
    }
    if ExecutorGlobals::has_exception() {
        return Err(pending("property assignment"));
    }
    Ok(())
}

/// Reads `$object->name` from inside the class, as 1.0's methods did: an uninitialized
/// property is the engine's `Error`.
pub(crate) fn read(value: Value, object: *mut zend_object, name: &str) -> PhpResult<Zval> {
    let mut rv = Zval::new();
    // SAFETY: a live object and a registered class; the result points at the property or `rv`.
    let found = unsafe {
        zend_read_property(
            entry(value),
            object,
            name.as_ptr().cast(),
            name.len(),
            false,
            &raw mut rv,
        )
    };
    if ExecutorGlobals::has_exception() {
        return Err(pending("property read"));
    }
    // SAFETY: a live zval, the property's or `rv`; the copy holds its own reference.
    let copy = unsafe { found.as_ref() }.map(Zval::shallow_clone).unwrap_or_default();
    Ok(copy)
}

/// A property read as a string's bytes (the declared type guarantees one).
pub(crate) fn read_bytes(value: Value, object: *mut zend_object, name: &str) -> PhpResult<Vec<u8>> {
    let zv = read(value, object, name)?;
    Ok(zv.zend_str().map(|s| s.as_bytes().to_vec()).unwrap_or_default())
}

/// A property read as an int (the declared type guarantees one).
pub(crate) fn read_int(value: Value, object: *mut zend_object, name: &str) -> PhpResult<i64> {
    Ok(read(value, object, name)?.long().unwrap_or_default())
}

/// A property read as a bool (the declared type guarantees one).
pub(crate) fn read_bool(value: Value, object: *mut zend_object, name: &str) -> PhpResult<bool> {
    Ok(read(value, object, name)?.bool().unwrap_or_default())
}

/// The object a zval holds, if it holds one.
pub(crate) fn object_of(zv: &Zval) -> Option<*mut zend_object> {
    zv.object().map(|object| ptr::from_ref(object).cast_mut().cast::<zend_object>())
}

/// A string zval of `bytes`, which need not be UTF-8.
pub(crate) fn bytes_zval(bytes: &[u8]) -> Zval {
    let mut zv = Zval::new();
    zv.set_zend_string(ZendStr::new(bytes, false));
    zv
}

/// An int zval.
pub(crate) fn int_zval(n: i64) -> Zval {
    let mut zv = Zval::new();
    zv.set_long(n);
    zv
}

/// A bool zval.
pub(crate) fn bool_zval(b: bool) -> Zval {
    let mut zv = Zval::new();
    zv.set_bool(b);
    zv
}

/// A float zval.
pub(crate) fn float_zval(f: f64) -> Zval {
    let mut zv = Zval::new();
    zv.set_double(f);
    zv
}

/// Calls the PHP function or static method `name` (`"Class::method"`), or with `object` the
/// method `name` on it, with `args`, as a PHP caller would. Whatever it throws stays pending
/// and is returned as the error.
pub(crate) fn call(object: Option<&Zval>, name: &str, args: &mut [Zval]) -> PhpResult<Zval> {
    let mut function = bytes_zval(name.as_bytes());
    let mut target = object.map(Zval::shallow_clone);
    let mut retval = Zval::new();
    let count = u32::try_from(args.len()).map_err(|_| pending(name))?;
    // SAFETY: live zvals for the call; the engine copies what it keeps.
    let status = unsafe {
        _call_user_function_impl(
            target.as_mut().map_or(ptr::null_mut(), ptr::from_mut),
            &raw mut function,
            &raw mut retval,
            count,
            args.as_mut_ptr(),
            ptr::null_mut(),
        )
    };
    if status < 0 || ExecutorGlobals::has_exception() {
        return Err(pending(name));
    }
    Ok(retval)
}

/// Whether the class `class` (lowercase) has the method `method` (lowercase), as
/// `method_exists` answers for an internal class.
pub(crate) fn has_method(class: &str, method: &str) -> bool {
    ClassEntry::try_find(class).is_some_and(|ce| ce.function_table.get(method).is_some())
}

/// Sets the handler's return value, or throws.
pub(crate) fn finish(result: PhpResult<Zval>, retval: &mut Zval) {
    match result {
        Ok(value) => *retval = value,
        Err(e) => throw(e),
    }
}

/// Throws `e` unless an exception is already pending.
pub(crate) fn throw(e: PhpException) {
    e.throw();
}

/// Registers every value class. MINIT.
pub(crate) fn register_all() -> ext_php_rs::error::Result<()> {
    super::values::register()?;
    super::num_format::register()?;
    super::constraints::register()
}
