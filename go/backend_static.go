//go:build cgo && !tinygo && (darwin || linux || windows) && (amd64 || arm64) && !(ios && amd64 && !maccatalyst)

// The native backend: libhypercast linked into the binary. The core is a static library
// under staticlib/{goos}_{goarch}/, named on the cgo link line below, and every door is an
// ordinary C call to a symbol the linker resolved. Nothing is embedded, nothing is written to
// a temp directory, nothing is loaded at run time: a binary carries the core for its own
// platform, starts without touching the filesystem, runs from a read-only or `scratch`
// image, and has nothing that can fail to load.
//
// That takes cgo, and so a C compiler wherever the module is built — gcc or clang on Linux,
// the Xcode command-line tools on macOS, a MinGW-w64 gcc (or llvm-mingw on arm64) on
// Windows. TinyGo compiling to WebAssembly links the same core from backend_tinygo.go — its
// cgo cannot parse this file's per-platform #cgo lines, hence `!tinygo` above. Anything else
// does not compile; unsupported.go says so by name. There used to be three more backends — a
// purego one for CGO_ENABLED=0 and Windows that extracted an embedded shared library to a
// temp file, a cgo one that loaded that library instead of linking it, and a wasmtime one —
// and none of them reached a platform this one does not: wasmtime-go itself needs cgo and
// ships engines only for these same platforms.
//
// One archive serves both C libraries on Linux. cgo has no build constraint that tells
// glibc from musl, so there cannot be one per libc; the archive is the core built for the
// musl target, which asks the C library for nothing glibc and musl do not both have (memcpy,
// memset, bcmp, abort). The suite runs against it on Debian and on Alpine.
//
// Windows links the same MSVC archive C#'s Native AOT publish does: MinGW's linker reads
// MSVC's COFF objects, and what the archive asks of the C runtime — memcpy, memset, memcmp,
// abort, __CxxFrameHandler3 — resolves against msvcrt.dll through MinGW's own import
// library, so the link line names nothing else. The core imports nothing from Windows
// itself: parsing needs no system call.
//
// iOS and Mac Catalyst link an archive of their own, since Go builds for both as GOOS=ios
// and every Mach-O object says which platform it was built for. GOOS=ios also satisfies the
// darwin constraint, so the macOS lines below say !ios. The three that share ios/arm64 are
// told apart by build tag: `maccatalyst`, which gomobile sets for that target;
// `iossimulator`, which nothing sets, so a simulator build passes it by hand; and neither,
// for a device. There is no archive for the simulator on amd64.
//
// Android links an archive of its own too, for arm64 and amd64: the core built for the
// Android targets, which asks Bionic for what it asks musl for. GOOS=android satisfies the
// linux constraint, so the Linux lines below say !android. Building for it takes the NDK's
// clang as CC (CC=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang
// GOOS=android GOARCH=arm64 CGO_ENABLED=1), as any cgo package on Android does.
//
// cgo cannot call a function pointer directly — it needs a statically-typed C call site —
// hence one shim per ABI shape (plain, numeric, unix, version), each taking its door as a
// pointer. The shims also own the out-params: any Go pointer handed to a cgo call escapes
// to the heap, so instead of passing `&out` and `&fault` across, the C side declares the
// out-value, the fault span and the format on its own stack and hands the whole verdict
// back BY VALUE as one struct. No Go pointer crosses but the input bytes — a pointer value
// into memory that already exists, not an address taken of a local — and the success path
// allocates nothing (allocs_test.go holds that).
//
// The archives are committed (staticlib/README.md): a Go module is whatever is in the tree at
// the resolved version, with no packing step to stage them in. The hypercast_local build tag
// links the archive .github/scripts/local-core.sh builds from the checkout instead, so the
// suite can run against the core as it stands without replacing a committed archive.
package hypercast

/*
#cgo linux,!android,amd64,!hypercast_local LDFLAGS: ${SRCDIR}/staticlib/linux_amd64/libhypercast.a
#cgo linux,!android,arm64,!hypercast_local LDFLAGS: ${SRCDIR}/staticlib/linux_arm64/libhypercast.a
#cgo darwin,!ios,amd64,!hypercast_local LDFLAGS: ${SRCDIR}/staticlib/darwin_amd64/libhypercast.a
#cgo darwin,!ios,arm64,!hypercast_local LDFLAGS: ${SRCDIR}/staticlib/darwin_arm64/libhypercast.a
#cgo ios,arm64,!iossimulator,!maccatalyst LDFLAGS: ${SRCDIR}/staticlib/ios_arm64/libhypercast.a
#cgo ios,arm64,iossimulator,!maccatalyst LDFLAGS: ${SRCDIR}/staticlib/iossimulator_arm64/libhypercast.a
#cgo ios,arm64,maccatalyst LDFLAGS: ${SRCDIR}/staticlib/maccatalyst_arm64/libhypercast.a
#cgo ios,amd64,maccatalyst LDFLAGS: ${SRCDIR}/staticlib/maccatalyst_amd64/libhypercast.a
#cgo windows,amd64,!hypercast_local LDFLAGS: ${SRCDIR}/staticlib/windows_amd64/libhypercast.a
#cgo windows,arm64,!hypercast_local LDFLAGS: ${SRCDIR}/staticlib/windows_arm64/libhypercast.a
#cgo android,amd64,!hypercast_local LDFLAGS: ${SRCDIR}/staticlib/android_amd64/libhypercast.a
#cgo android,arm64,!hypercast_local LDFLAGS: ${SRCDIR}/staticlib/android_arm64/libhypercast.a
#cgo linux,!android,amd64,hypercast_local LDFLAGS: ${SRCDIR}/../rust/target/local-core/go/staticlib/linux_amd64/libhypercast.a
#cgo linux,!android,arm64,hypercast_local LDFLAGS: ${SRCDIR}/../rust/target/local-core/go/staticlib/linux_arm64/libhypercast.a
#cgo darwin,!ios,amd64,hypercast_local LDFLAGS: ${SRCDIR}/../rust/target/local-core/go/staticlib/darwin_amd64/libhypercast.a
#cgo darwin,!ios,arm64,hypercast_local LDFLAGS: ${SRCDIR}/../rust/target/local-core/go/staticlib/darwin_arm64/libhypercast.a
#cgo windows,amd64,hypercast_local LDFLAGS: ${SRCDIR}/../rust/target/local-core/go/staticlib/windows_amd64/libhypercast.a
#cgo windows,arm64,hypercast_local LDFLAGS: ${SRCDIR}/../rust/target/local-core/go/staticlib/windows_arm64/libhypercast.a
#cgo android,amd64,hypercast_local LDFLAGS: ${SRCDIR}/../rust/target/local-core/go/staticlib/android_amd64/libhypercast.a
#cgo android,arm64,hypercast_local LDFLAGS: ${SRCDIR}/../rust/target/local-core/go/staticlib/android_arm64/libhypercast.a
#include <stddef.h>
#include <stdint.h>

// The core's C ABI — rust/src/ffi.rs, the twenty-seven exports every binding calls.
// `out`, `format` and `fault` are untyped because the shims below fill and read them as
// the raw layouts ffi.rs declares.
uint32_t hypercast_version(void);
int32_t cast_bool(const uint8_t *ptr, size_t len, void *out, void *fault);
int32_t cast_char(const uint8_t *ptr, size_t len, void *out, void *fault);
int32_t cast_i8(const uint8_t *ptr, size_t len, const void *format, void *out, void *fault);
int32_t cast_i16(const uint8_t *ptr, size_t len, const void *format, void *out, void *fault);
int32_t cast_i32(const uint8_t *ptr, size_t len, const void *format, void *out, void *fault);
int32_t cast_i64(const uint8_t *ptr, size_t len, const void *format, void *out, void *fault);
int32_t cast_u8(const uint8_t *ptr, size_t len, const void *format, void *out, void *fault);
int32_t cast_u16(const uint8_t *ptr, size_t len, const void *format, void *out, void *fault);
int32_t cast_u32(const uint8_t *ptr, size_t len, const void *format, void *out, void *fault);
int32_t cast_u64(const uint8_t *ptr, size_t len, const void *format, void *out, void *fault);
int32_t cast_f32(const uint8_t *ptr, size_t len, const void *format, void *out, void *fault);
int32_t cast_f64(const uint8_t *ptr, size_t len, const void *format, void *out, void *fault);
int32_t cast_decimal(const uint8_t *ptr, size_t len, const void *format, void *out, void *fault);
int32_t cast_uuid(const uint8_t *ptr, size_t len, void *out, void *fault);
int32_t cast_timestamp(const uint8_t *ptr, size_t len, void *out, void *fault);
int32_t cast_unix(const uint8_t *ptr, size_t len, uint32_t precision, void *out, void *fault);
int32_t cast_excel_serial(const uint8_t *ptr, size_t len, uint32_t epoch, void *out, void *fault);
int32_t cast_date(const uint8_t *ptr, size_t len, void *out, void *fault);
int32_t cast_date_ordered(const uint8_t *ptr, size_t len, uint32_t order, void *out, void *fault);
int32_t cast_datetime(const uint8_t *ptr, size_t len, uint32_t order, void *out, void *fault);
int32_t cast_time(const uint8_t *ptr, size_t len, void *out, void *fault);
int32_t cast_duration(const uint8_t *ptr, size_t len, void *out, void *fault);
int32_t cast_decimal_from_f64(double value, void *out, void *fault);
int32_t cast_excel_serial_from_f64(double value, uint32_t epoch, void *out, void *fault);
int32_t cast_excel_time(double value, void *out, void *fault);
int32_t cast_excel_duration(double value, void *out, void *fault);

typedef int32_t (*fn_plain)(const uint8_t*, size_t, void*, void*);
typedef int32_t (*fn_numeric)(const uint8_t*, size_t, const void*, void*, void*);
typedef int32_t (*fn_unix)(const uint8_t*, size_t, uint32_t, void*, void*);
typedef uint32_t (*fn_version)(void);
typedef int32_t (*fn_typed)(double, void*, void*);

typedef struct { uint32_t offset; uint32_t len; } hc_fault;
// RawNumFormat, 32 bytes: the currency symbol is currency_len UTF-8 bytes held inline.
typedef struct { uint32_t decimal_sep; uint32_t group_sep; uint32_t flags; uint32_t currency_len; uint8_t currency[16]; } hc_format;

// The verdict, by value: 16 bytes of out-value (the widest door — a timestamp or a civil
// date-time — 8-aligned because the core writes an i64/u64 into it), the code, and the
// fault span. Every door reads its own prefix of `out`.
typedef struct { uint64_t out[2]; int32_t code; uint32_t offset; uint32_t len; } hc_result;

// At the sizes rust/src/abi.rs and ffi.rs pin for RawFault and RawNumFormat.
_Static_assert(sizeof(hc_fault) == 8, "RawFault");
_Static_assert(sizeof(hc_format) == 32, "RawNumFormat");

static hc_result call_plain(void *fn, const uint8_t *ptr, size_t len) {
	hc_result r = {{0, 0}, 0, 0, 0};
	hc_fault f = {0, 0};
	r.code = ((fn_plain)fn)(ptr, len, r.out, &f);
	r.offset = f.offset;
	r.len = f.len;
	return r;
}
// The format arrives as scalars — the 16 currency bytes as two little-endian u64 halves —
// and is assembled here on the C stack, so no Go pointer to a local crosses the call.
static hc_result call_numeric(void *fn, const uint8_t *ptr, size_t len, uint32_t decimal_sep, uint32_t group_sep, uint32_t flags,
                              uint32_t currency_len, uint64_t cur_lo, uint64_t cur_hi) {
	hc_result r = {{0, 0}, 0, 0, 0};
	hc_fault f = {0, 0};
	hc_format format = {decimal_sep, group_sep, flags, currency_len, {0}};
	for (int i = 0; i < 8; i++) {
		format.currency[i] = (uint8_t)(cur_lo >> (8 * i));
		format.currency[8 + i] = (uint8_t)(cur_hi >> (8 * i));
	}
	r.code = ((fn_numeric)fn)(ptr, len, &format, r.out, &f);
	r.offset = f.offset;
	r.len = f.len;
	return r;
}
// The typed doors take a double instead of text: one shim for the three that take nothing
// else, and the Excel serial door's own for its epoch.
static hc_result call_typed(void *fn, double value) {
	hc_result r = {{0, 0}, 0, 0, 0};
	hc_fault f = {0, 0};
	r.code = ((fn_typed)fn)(value, r.out, &f);
	r.offset = f.offset;
	r.len = f.len;
	return r;
}
static hc_result call_excel_serial_from_f64(double value, uint32_t epoch) {
	hc_result r = {{0, 0}, 0, 0, 0};
	hc_fault f = {0, 0};
	r.code = cast_excel_serial_from_f64(value, epoch, r.out, &f);
	r.offset = f.offset;
	r.len = f.len;
	return r;
}
static uint32_t call_version(void *fn) {
	return ((fn_version)fn)();
}
static hc_result call_unix(void *fn, const uint8_t *ptr, size_t len, uint32_t discriminant) {
	hc_result r = {{0, 0}, 0, 0, 0};
	hc_fault f = {0, 0};
	r.code = ((fn_unix)fn)(ptr, len, discriminant, r.out, &f);
	r.offset = f.offset;
	r.len = f.len;
	return r;
}
*/
import "C"

import (
	"encoding/binary"
	"unsafe"
)

// The backend-defined symbol types cast.go's doors are written against: here both are the
// address of a linked C function, called through the statically-typed shims above;
// backend_tinygo.go's are Go functions that call the core directly.
type (
	plainSymbol   = unsafe.Pointer
	numericSymbol = unsafe.Pointer
)

// Each door's address, resolved by the linker — nothing to look up at run time.
var (
	symBool      = plainSymbol(C.cast_bool)
	symChar      = plainSymbol(C.cast_char)
	symUuid      = plainSymbol(C.cast_uuid)
	symTimestamp = plainSymbol(C.cast_timestamp)
	symDate      = plainSymbol(C.cast_date)
	symTime      = plainSymbol(C.cast_time)
	symDuration  = plainSymbol(C.cast_duration)

	symI8      = numericSymbol(C.cast_i8)
	symI16     = numericSymbol(C.cast_i16)
	symI32     = numericSymbol(C.cast_i32)
	symI64     = numericSymbol(C.cast_i64)
	symU8      = numericSymbol(C.cast_u8)
	symU16     = numericSymbol(C.cast_u16)
	symU32     = numericSymbol(C.cast_u32)
	symU64     = numericSymbol(C.cast_u64)
	symF32     = numericSymbol(C.cast_f32)
	symF64     = numericSymbol(C.cast_f64)
	symDecimal = numericSymbol(C.cast_decimal)
)

// packedVersion is the core's own version, major<<16 | minor<<8 | patch, read through the
// ABI rather than from this module, so NativeVersion reports the archive that was linked.
func packedVersion() uint32 {
	return uint32(C.call_version(unsafe.Pointer(C.hypercast_version)))
}

func fromC(r C.hc_result) result {
	return result{
		out:   [2]uint64{uint64(r.out[0]), uint64(r.out[1])},
		code:  int32(r.code),
		fault: rawFault{Offset: uint32(r.offset), Length: uint32(r.len)},
	}
}

func callPlain(sym plainSymbol, ptr unsafe.Pointer, length uintptr) result {
	return fromC(C.call_plain(sym, (*C.uint8_t)(ptr), C.size_t(length)))
}

func callNumeric(sym numericSymbol, ptr unsafe.Pointer, length uintptr, format RawNumFormat) result {
	// The currency bytes travel as two little-endian u64 halves so nothing here takes the
	// address of a local for the C side — the shim reassembles them byte by byte.
	return fromC(C.call_numeric(sym, (*C.uint8_t)(ptr), C.size_t(length),
		C.uint32_t(format.DecimalSep), C.uint32_t(format.GroupSep), C.uint32_t(format.Flags),
		C.uint32_t(format.CurrencyLen),
		C.uint64_t(binary.LittleEndian.Uint64(format.Currency[:8])),
		C.uint64_t(binary.LittleEndian.Uint64(format.Currency[8:]))))
}

func callUnix(ptr unsafe.Pointer, length uintptr, precision uint32) result {
	return fromC(C.call_unix(unsafe.Pointer(C.cast_unix), (*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(precision)))
}

// cast_date_ordered, cast_datetime and cast_excel_serial share the unix ABI shape (ptr,
// len, u32, out, fault) — same C shim.
func callDateOrdered(ptr unsafe.Pointer, length uintptr, order uint32) result {
	return fromC(C.call_unix(unsafe.Pointer(C.cast_date_ordered), (*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(order)))
}

func callDateTime(ptr unsafe.Pointer, length uintptr, order uint32) result {
	return fromC(C.call_unix(unsafe.Pointer(C.cast_datetime), (*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(order)))
}

func callExcelSerial(ptr unsafe.Pointer, length uintptr, epoch uint32) result {
	return fromC(C.call_unix(unsafe.Pointer(C.cast_excel_serial), (*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(epoch)))
}

func callDecimalFromF64(value float64) result {
	return fromC(C.call_typed(unsafe.Pointer(C.cast_decimal_from_f64), C.double(value)))
}

func callExcelSerialFromF64(value float64, epoch uint32) result {
	return fromC(C.call_excel_serial_from_f64(C.double(value), C.uint32_t(epoch)))
}

func callExcelTime(value float64) result {
	return fromC(C.call_typed(unsafe.Pointer(C.cast_excel_time), C.double(value)))
}

func callExcelDuration(value float64) result {
	return fromC(C.call_typed(unsafe.Pointer(C.cast_excel_duration), C.double(value)))
}
