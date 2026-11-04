//! `HyperCast\Interop\NativeValues` and `HyperCast\Interop\NativePlatform`: 1.0's public
//! helpers for code that calls the C ABI itself (HyperTabular's PHP binding builds its values
//! with them), kept with 1.0's signatures and results. ext-ffi and ext-intl are reached only
//! through the objects a caller passes (`FFI\CData`), so the extension needs neither.

#![allow(non_snake_case)] // parameter names are PHP's, which reflection and named arguments read

use ext_php_rs::convert::IntoZval;
use ext_php_rs::error::Result as ZvalResult;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::ffi::zend_read_property;
use ext_php_rs::flags::DataType;
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;
use ext_php_rs::zend::ExecutorGlobals;

use super::args::{Bool, CDataArg, DayPeriodsArg, Int, NumFormatArg, OptFfiArg, OptInt, Str};
use super::classes::{bytes_zval, call, int_zval, object_of};
use super::dates;
use super::enums::CastFailure;
use super::errors::{logic, pending, runtime};
use super::guard;
use super::num_format::quietly;
use super::values::{fault as new_fault, periods_packed};

/// A member of an `FFI` object (a declared function or variable), or `None` when the object
/// does not declare it: the engine's `FFI\Exception` for an undeclared name is cleared.
fn ffi_member(core: &Zval, name: &str) -> Option<Zval> {
    let object = object_of(core)?;
    let mut rv = Zval::new();
    // SAFETY: a live object, read through its own class entry as `$object->name` reads it.
    let found = unsafe {
        zend_read_property(
            (*object).ce,
            object,
            name.as_ptr().cast(),
            name.len(),
            false,
            &raw mut rv,
        )
    };
    if ExecutorGlobals::has_exception() {
        drop(ExecutorGlobals::take_exception());
        return None;
    }
    // SAFETY: a live zval, the member's or `rv`.
    unsafe { found.as_ref() }.map(Zval::shallow_clone)
}

/// Why a strict grouping policy may not be written for `core`, an `FFI` handle on the C
/// library, or `None` when it may. A core that predates the policy ignores its bits and parses
/// every format as legacy, so two checks, neither a hard-coded version: the library must
/// declare the detailed exports (`cast_detailed`, which a 1.0 library does not have, and which
/// an `FFI` handle only reads when its declarations name it and the library has it), and its
/// `hypercast_version()` must have this extension's major version and a minor version no older.
fn strict_refused(core: &Zval) -> Option<String> {
    if ffi_member(core, "cast_detailed").is_none() {
        return Some(
            "the core's FFI handle does not declare cast_detailed, which a core that reads a \
             grouping policy exports"
                .to_owned(),
        );
    }
    // An undeclared function is the engine's `FFI\Exception`, cleared: the refusal is the guard's.
    let reported = quietly(call(Some(core), "hypercast_version", &mut []));
    let Some(version) = reported.as_ref().and_then(Zval::long) else {
        return Some(
            "the core's FFI handle does not declare hypercast_version, so its version is unknown"
                .to_owned(),
        );
    };
    let own = i64::from(crate::hypercast_version());
    (!versions_compatible(version, own)).then(|| {
        format!(
            "the core is {}, which does not match this extension, {} (the same major version and a \
             minor version no older)",
            version_text(version),
            version_text(own)
        )
    })
}

/// Whether a core's packed version (`hypercast_version()`) suits the extension's: the same
/// major version and a minor version no older; the patch is ignored.
pub(crate) fn versions_compatible(core: i64, extension: i64) -> bool {
    let (major, minor) = (|v: i64| v >> 16, |v: i64| (v >> 8) & 0xFF);
    major(core) == major(extension) && minor(core) >= minor(extension)
}

/// A `\DateTimeImmutable`, built by PHP.
pub struct DateTimeImmutable(Zval);

impl IntoZval for DateTimeImmutable {
    const TYPE: DataType = DataType::object("DateTimeImmutable");
    const NULLABLE: bool = false;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        *zv = self.0;
        Ok(())
    }
}

/// A `HyperCast\Fault`.
pub struct FaultValue(Zval);

impl IntoZval for FaultValue {
    const TYPE: DataType = DataType::object("HyperCast\\Fault");
    const NULLABLE: bool = false;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        *zv = self.0;
        Ok(())
    }
}

/// A PHP string of any bytes.
pub struct Bytes(Vec<u8>);

impl IntoZval for Bytes {
    const TYPE: DataType = DataType::String;
    const NULLABLE: bool = false;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        *zv = bytes_zval(&self.0);
        Ok(())
    }
}

/// 1.0's `NativeValues::uuid`: `bin2hex`, then `substr` pieces joined by hyphens (whatever the
/// length, as `substr` cuts).
pub(crate) fn uuid_text(bytes: &[u8]) -> Vec<u8> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let hex: Vec<u8> =
        bytes.iter().flat_map(|b| [HEX[usize::from(b >> 4)], HEX[usize::from(b & 15)]]).collect();
    let piece = |start: usize, len: usize| -> &[u8] {
        let start = start.min(hex.len());
        &hex[start..(start + len).min(hex.len())]
    };
    let mut text = Vec::with_capacity(36);
    for (i, (start, len)) in [(0, 8), (8, 4), (12, 4), (16, 4), (20, 12)].into_iter().enumerate() {
        if i > 0 {
            text.push(b'-');
        }
        text.extend_from_slice(piece(start, len));
    }
    text
}

/// 1.0's `NativeValues::version`: `sprintf('%d.%d.%d', $packed >> 16, ($packed >> 8) & 0xFF,
/// $packed & 0xFF)`, PHP's arithmetic shift.
pub(crate) fn version_text(packed: i64) -> String {
    format!("{}.{}.{}", packed >> 16, (packed >> 8) & 0xFF, packed & 0xFF)
}

/// `FFI::memcpy($target, $packed, $size)`, as 1.0's writers copied a packed layout.
fn memcpy(target: &Zval, packed: &[u8], size: usize) -> PhpResult<()> {
    call(
        None,
        "FFI::memcpy",
        &mut [target.shallow_clone(), bytes_zval(packed), int_zval(size as i64)],
    )?;
    Ok(())
}

/// The core's C ABI values, presented, for a package that carries HyperCast's verdicts across
/// a C ABI of its own (HyperTabular does): it reads the core's fields out of its own buffers
/// and builds each value through the conversion every `Cast` door uses. A `Decimal` is built
/// by `Decimal::fromLimbs()`, a span by the `Duration` constructor.
#[php_class]
#[php(name = "HyperCast\\Interop\\NativeValues")]
#[php(flags = ext_php_rs::flags::ClassFlags::Final)]
pub struct NativeValues;

#[php_impl]
impl NativeValues {
    /// Not instantiable: every helper is static.
    #[php(vis = "private")]
    pub fn __construct() -> PhpResult<Self> {
        guard(|| Ok(Self))
    }

    /// A UTC instant from Unix seconds and nanoseconds (truncated to microseconds).
    pub fn instant(seconds: Int, nanos: Int) -> PhpResult<DateTimeImmutable> {
        guard(|| dates::instant(seconds.0, nanos.0).map(DateTimeImmutable))
    }

    /// Midnight UTC of a civil date.
    pub fn date(year: Int, month: Int, day: Int) -> PhpResult<DateTimeImmutable> {
        guard(|| dates::date(year.0, month.0, day.0).map(DateTimeImmutable))
    }

    /// A civil date and time of day, read as UTC.
    pub fn civil(year: Int, month: Int, day: Int, nanosOfDay: Int) -> PhpResult<DateTimeImmutable> {
        guard(|| dates::civil(year.0, month.0, day.0, nanosOfDay.0).map(DateTimeImmutable))
    }

    /// 16 bytes as the canonical lowercase hyphenated text.
    pub fn uuid(bytes: Str<'_>) -> PhpResult<Bytes> {
        guard(|| Ok(Bytes(uuid_text(bytes.0))))
    }

    /// A `Fault` from the C ABI's verdict code and span.
    pub fn fault(code: Int, offset: Int, length: Int) -> PhpResult<FaultValue> {
        guard(|| {
            let reason = match code.0 {
                1 => CastFailure::Empty,
                2 => CastFailure::Malformed,
                3 => CastFailure::OutOfRange,
                other => {
                    return Err(logic(format!(
                        "{other} is not a verdict reason code — a binding bug, please report it"
                    )));
                }
            };
            new_fault(reason, offset.0, length.0).map(FaultValue)
        })
    }

    /// `hypercast_version()`'s packed version as `major.minor.patch`.
    pub fn version(packed: Int) -> PhpResult<String> {
        guard(|| Ok(version_text(packed.0)))
    }

    /// Whether a core's packed `hypercast_version()` suits this extension's, or `$extensionVersion`
    /// when given: the same major version and a minor version no older, the patch ignored.
    pub fn compatible(coreVersion: Int, extensionVersion: OptInt) -> PhpResult<bool> {
        guard(|| {
            let own = extensionVersion.0.unwrap_or_else(|| i64::from(crate::hypercast_version()));
            Ok(versions_compatible(coreVersion.0, own))
        })
    }

    /// Writes a format's packed layout into an `hc_format` (ext-ffi). A core that predates the
    /// grouping policy ignores its bits and parses every format as legacy, so a caller passes
    /// the `FFI` handle it loaded the core with: a strict policy is then refused for a core that
    /// lacks the detailed exports or is older than this extension, never written to be parsed
    /// as legacy. Without a handle, the caller owns that check.
    pub fn write_format(
        format: NumFormatArg<'_>,
        target: CDataArg<'_>,
        core: OptFfiArg<'_>,
    ) -> PhpResult<()> {
        guard(|| {
            let object = object_of(format.0).ok_or_else(|| pending("format"))?;
            let packed = super::num_format::packed(object)?;
            let flags = u32::from_le_bytes([packed[4], packed[5], packed[6], packed[7]]);
            if let Some(core) = core.0
                && flags & crate::NumFormat::GROUPING_POLICY != 0
                && let Some(why) = strict_refused(core)
            {
                return Err(runtime(format!(
                    "hypercast: a strict grouping policy is refused: {why}; declare GROUPING_LEGACY for it"
                )));
            }
            memcpy(target.0, &packed, 92)
        })
    }

    /// Writes a set of day periods' packed layout into an `hc_periods` (ext-ffi).
    pub fn write_day_periods(periods: DayPeriodsArg<'_>, target: CDataArg<'_>) -> PhpResult<()> {
        guard(|| {
            let object = object_of(periods.0).ok_or_else(|| pending("day periods"))?;
            let packed = periods_packed(object)?;
            memcpy(target.0, &packed, 40)
        })
    }
}

/// 1.0's `NativePlatform::resolve`.
fn resolve_platform(
    base: &[u8],
    os_family: &[u8],
    machine: &[u8],
    int_size: i64,
    musl: bool,
) -> PhpResult<Vec<Vec<u8>>> {
    let base_text = String::from_utf8_lossy(base);
    if int_size != 8 {
        return Err(runtime(format!(
            "{base_text}: unsupported platform — a 64-bit PHP is required"
        )));
    }
    let named = |prefix: &[u8], suffix: &[u8]| [prefix, base, suffix].concat();
    if os_family == b"Windows" {
        return Ok(vec![b"win-x64".to_vec(), named(b"", b".dll")]);
    }
    let arch: &[u8] = match machine.to_ascii_lowercase().as_slice() {
        b"x86_64" | b"amd64" => b"x64",
        b"aarch64" | b"arm64" => b"arm64",
        _ => {
            return Err(runtime(format!(
                "{base_text}: unsupported platform — no native library for architecture '{}'",
                String::from_utf8_lossy(machine)
            )));
        }
    };
    match os_family {
        b"Darwin" => Ok(vec![[b"osx-", arch].concat(), named(b"lib", b".dylib")]),
        b"Linux" => {
            let prefix: &[u8] = if musl { b"linux-musl-" } else { b"linux-" };
            Ok(vec![[prefix, arch].concat(), named(b"lib", b".so")])
        }
        _ => Err(runtime(format!(
            "{base_text}: unsupported platform PHP_OS_FAMILY={}",
            String::from_utf8_lossy(os_family)
        ))),
    }
}

/// A list of strings, as PHP's `array`.
pub struct StringList(Vec<Vec<u8>>);

impl IntoZval for StringList {
    const TYPE: DataType = DataType::Array;
    const NULLABLE: bool = false;

    fn set_zval(self, zv: &mut Zval, _: bool) -> ZvalResult<()> {
        let mut array = ext_php_rs::types::ZendHashTable::new();
        for item in self.0 {
            array.push(bytes_zval(&item))?;
        }
        array.set_zval(zv, false)
    }
}

/// A string a PHP call returned, as bytes (`None` when it returned anything else).
fn string_of(zv: &Zval) -> Option<Vec<u8>> {
    zv.zend_str().map(|s| s.as_bytes().to_vec())
}

/// Whether this process runs on musl, as 1.0 read it from `/proc/self/maps`.
fn is_musl() -> bool {
    std::fs::read("/proc/self/maps").is_ok_and(|maps| {
        let has = |needle: &[u8]| maps.windows(needle.len()).any(|w| w == needle);
        has(b"ld-musl-") || has(b"libc.musl-")
    })
}

/// 1.0's `NativePlatform::ridAndLibraryName`, for the running PHP.
fn rid_and_library_name(base: &[u8]) -> PhpResult<Vec<Vec<u8>>> {
    let os_family = call(None, "constant", &mut [bytes_zval(b"PHP_OS_FAMILY")])?;
    let os_family = string_of(&os_family).unwrap_or_default();
    let machine = call(None, "php_uname", &mut [bytes_zval(b"m")])?;
    let machine = string_of(&machine).unwrap_or_default();
    let musl = os_family == b"Linux" && is_musl();
    resolve_platform(base, &os_family, &machine, size_of::<usize>() as i64, musl)
}

/// `is_file($path)`.
fn is_file(path: &[u8]) -> PhpResult<bool> {
    Ok(call(None, "is_file", &mut [bytes_zval(path)])?.is_true())
}

/// Maps the running process to the RID-style directory and file name of a native library
/// (`native/{rid}/`), and finds the library to load: for a package that ships a SkunkWerkx
/// core's builds that way and loads them with ext-ffi (HyperTabular loads `hypertabular`).
#[php_class]
#[php(name = "HyperCast\\Interop\\NativePlatform")]
#[php(flags = ext_php_rs::flags::ClassFlags::Final)]
pub struct NativePlatform;

#[php_impl]
impl NativePlatform {
    /// Not instantiable: every helper is static.
    #[php(vis = "private")]
    pub fn __construct() -> PhpResult<Self> {
        guard(|| Ok(Self))
    }

    /// `[rid, libraryName]` for the running PHP.
    pub fn rid_and_library_name(baseName: Str<'_>) -> PhpResult<StringList> {
        guard(|| rid_and_library_name(baseName.0).map(StringList))
    }

    /// `[rid, libraryName]` for a declared platform.
    pub fn resolve(
        baseName: Str<'_>,
        osFamily: Str<'_>,
        machine: Str<'_>,
        intSize: Int,
        musl: Bool,
    ) -> PhpResult<StringList> {
        guard(|| {
            resolve_platform(baseName.0, osFamily.0, machine.0, intSize.0, musl.0).map(StringList)
        })
    }

    /// Where a native library for the running platform is: the `$override` environment
    /// variable's path, else `<sourceDir>/native/<rid>/<library>`, else a repository build.
    pub fn library_path(
        baseName: Str<'_>,
        sourceDir: Str<'_>,
        r#override: Str<'_>,
    ) -> PhpResult<Bytes> {
        guard(|| {
            let names = rid_and_library_name(baseName.0)?;
            let (rid, library) = (&names[0], &names[1]);
            let mut path = [sourceDir.0, b"/native/", rid, b"/", library].concat();
            let named = call(None, "getenv", &mut [bytes_zval(r#override.0)])?;
            match string_of(&named) {
                Some(named) if !named.is_empty() => path = named,
                _ => {
                    if !is_file(&path)? {
                        let up =
                            call(None, "dirname", &mut [bytes_zval(sourceDir.0), int_zval(2)])?;
                        let repo_build = [
                            string_of(&up).unwrap_or_default().as_slice(),
                            b"/rust/target/release/",
                            library,
                        ]
                        .concat();
                        if is_file(&repo_build)? {
                            path = repo_build;
                        }
                    }
                }
            }
            if !is_file(&path)? {
                return Err(runtime(format!(
                    "{}: {} not found (unsupported platform, or this package was built without a native library for it)",
                    String::from_utf8_lossy(baseName.0),
                    String::from_utf8_lossy(&path)
                )));
            }
            Ok(Bytes(path))
        })
    }
}
