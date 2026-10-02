//go:build cgo && (darwin || linux) && (amd64 || arm64) && !hypercast_wasm && !hypercast_dynamic

// This backend links libhypercast into the binary. It is what a cgo build gets on Linux and
// macOS, on amd64 and arm64: the core is a static library under staticlib/{goos}_{goarch}/,
// named on the cgo link line below, and every door is an ordinary C call to a symbol the
// linker resolved.
//
// What that removes, compared with the backend it replaced as the default (backend_cgo.go,
// still there behind the hypercast_dynamic tag): nothing is embedded, nothing is written to
// a temp directory, nothing is dlopen'd. A binary built this way carries the core for its
// own platform instead of every platform's shared library, starts without touching the
// filesystem, and runs where there is no writable temp directory or no dynamic loader at
// all — a read-only container, a `scratch` image, a fully static build. There is also
// nothing left that can fail to load: Available is always true.
//
// One archive serves both C libraries on Linux. cgo has no build constraint that tells
// glibc from musl, so there cannot be one per libc; the archive is the core built for the
// musl target, which asks the C library for nothing glibc and musl do not both have. The
// suite runs against it on Debian and on Alpine.
//
// Three other builds exist, and none of them changes:
//
//   - CGO_ENABLED=0 — which includes every cross-compile, per Go's own default — is the
//     purego backend (backend_purego.go): the shared library, embedded and dlopen'd. An
//     archive cannot be linked without a C linker.
//   - Windows is purego unconditionally; see backend_purego.go for why.
//   - `-tags hypercast_dynamic` keeps cgo but loads the shared library the way this module
//     did through 0.3.0 (backend_cgo.go), for a build that has to pick the core up at run
//     time rather than link time.
//
// The shims are backend_cgo.go's, unchanged: each takes the door as a function pointer and
// owns the out-params on the C stack, so no Go pointer crosses but the input bytes and the
// success path allocates nothing. Only where the pointers come from differs — the linker
// here, dlsym there.
//
// The archives are committed, for the reason the shared libraries are (staticlib/README.md):
// a Go module is whatever is in the tree at the resolved version.
package hypercast

/*
#cgo linux,amd64 LDFLAGS: ${SRCDIR}/staticlib/linux_amd64/libhypercast.a
#cgo linux,arm64 LDFLAGS: ${SRCDIR}/staticlib/linux_arm64/libhypercast.a
#cgo darwin,amd64 LDFLAGS: ${SRCDIR}/staticlib/darwin_amd64/libhypercast.a
#cgo darwin,arm64 LDFLAGS: ${SRCDIR}/staticlib/darwin_arm64/libhypercast.a
#include <stddef.h>
#include <stdint.h>

// The core's C ABI — rust/src/ffi.rs, the same twenty-two exports every backend calls.
// `out`, `format` and `fault` are untyped because the shims below fill and read them as
// the raw layouts ffi.rs declares.
uint32_t hypercast_version(void);
int32_t cast_bool(const uint8_t *ptr, size_t len, void *out, void *fault);
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

typedef int32_t (*fn_plain)(const uint8_t*, size_t, void*, void*);
typedef int32_t (*fn_numeric)(const uint8_t*, size_t, const void*, void*, void*);
typedef int32_t (*fn_unix)(const uint8_t*, size_t, uint32_t, void*, void*);
typedef uint32_t (*fn_version)(void);

typedef struct { uint32_t offset; uint32_t len; } hc_fault;
// RawNumFormat, 32 bytes: the currency symbol is currency_len UTF-8 bytes held inline.
typedef struct { uint32_t decimal_sep; uint32_t group_sep; uint32_t flags; uint32_t currency_len; uint8_t currency[16]; } hc_format;

// The verdict, by value: 16 bytes of out-value (the widest door — a timestamp or a civil
// date-time — 8-aligned because the core writes an i64/u64 into it), the code, and the
// fault span. Every door reads its own prefix of `out`.
typedef struct { uint64_t out[2]; int32_t code; uint32_t offset; uint32_t len; } hc_result;

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

// The backend-defined symbol types cast.go's shared doors are written against: here both
// alias the address of a linked C function and the calls go through the statically-typed C
// shims above;
// in backend_purego.go each symbol is a purego-registered typed trampoline instead.
type (
	plainSymbol   = unsafe.Pointer
	numericSymbol = unsafe.Pointer
)

var symBool, symI8, symI16, symI32, symI64, symU8, symU16, symU32, symU64,
	symF32, symF64, symDecimal, symUuid, symTimestamp, symUnix, symDate, symDateOrdered,
	symDateTime, symTime, symDuration, symExcelSerial, symVersion unsafe.Pointer

// loadBackend has nothing to load — the linker did that — so it takes each door's address
// from the linked symbol and makes the one call through the ABI that every backend ends
// with. It cannot fail. ensureLoaded (load.go) runs it exactly once.
func loadBackend() error {
	symBool, symUuid = unsafe.Pointer(C.cast_bool), unsafe.Pointer(C.cast_uuid)
	symI8, symI16 = unsafe.Pointer(C.cast_i8), unsafe.Pointer(C.cast_i16)
	symI32, symI64 = unsafe.Pointer(C.cast_i32), unsafe.Pointer(C.cast_i64)
	symU8, symU16 = unsafe.Pointer(C.cast_u8), unsafe.Pointer(C.cast_u16)
	symU32, symU64 = unsafe.Pointer(C.cast_u32), unsafe.Pointer(C.cast_u64)
	symF32, symF64 = unsafe.Pointer(C.cast_f32), unsafe.Pointer(C.cast_f64)
	symDecimal = unsafe.Pointer(C.cast_decimal)
	symVersion = unsafe.Pointer(C.hypercast_version)
	symTimestamp, symUnix = unsafe.Pointer(C.cast_timestamp), unsafe.Pointer(C.cast_unix)
	symDate, symDateOrdered = unsafe.Pointer(C.cast_date), unsafe.Pointer(C.cast_date_ordered)
	symDateTime = unsafe.Pointer(C.cast_datetime)
	symTime, symDuration = unsafe.Pointer(C.cast_time), unsafe.Pointer(C.cast_duration)
	symExcelSerial = unsafe.Pointer(C.cast_excel_serial)
	nativeVersion = callVersion()
	return nil
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

func callNumeric(sym numericSymbol, ptr unsafe.Pointer, length uintptr, format rawNumFormat) result {
	// The currency bytes travel as two little-endian u64 halves so nothing here takes the
	// address of a local for the C side — the shim reassembles them byte by byte.
	return fromC(C.call_numeric(sym, (*C.uint8_t)(ptr), C.size_t(length),
		C.uint32_t(format.DecimalSep), C.uint32_t(format.GroupSep), C.uint32_t(format.Flags),
		C.uint32_t(format.CurrencyLen),
		C.uint64_t(binary.LittleEndian.Uint64(format.Currency[:8])),
		C.uint64_t(binary.LittleEndian.Uint64(format.Currency[8:]))))
}

func callVersion() uint32 {
	return uint32(C.call_version(symVersion))
}

func callUnix(ptr unsafe.Pointer, length uintptr, precision uint32) result {
	return fromC(C.call_unix(symUnix, (*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(precision)))
}

// cast_date_ordered and cast_datetime share the unix ABI shape (ptr, len, u32, out,
// fault) — same C shim.
func callDateOrdered(ptr unsafe.Pointer, length uintptr, order uint32) result {
	return fromC(C.call_unix(symDateOrdered, (*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(order)))
}

func callDateTime(ptr unsafe.Pointer, length uintptr, order uint32) result {
	return fromC(C.call_unix(symDateTime, (*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(order)))
}

func callExcelSerial(ptr unsafe.Pointer, length uintptr, epoch uint32) result {
	return fromC(C.call_unix(symExcelSerial, (*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(epoch)))
}
