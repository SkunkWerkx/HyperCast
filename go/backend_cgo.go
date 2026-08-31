//go:build cgo && (darwin || linux)

// The cgo backend — HyperUuid's measured lesson applied from day one: purego's per-call
// trampoline allocations would eat scalar parsing alive (its batch API was 19.6x faster
// than per-call precisely because of that overhead), so scalar-per-call HyperCast runs
// on real C calls wherever cgo is available. backend_purego.go is the fallback half of
// the pair — Windows, CGO_ENABLED=0, and cross-compiles land there automatically.
//
// cgo can't call an opaquely-typed void* function pointer directly — it needs a real,
// statically-typed C call site — hence the three per-signature shims below, one per ABI
// shape (plain, numeric, unix).
package hypercast

/*
#include <dlfcn.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

typedef int32_t (*fn_plain)(const uint8_t*, size_t, void*, void*);
typedef int32_t (*fn_numeric)(const uint8_t*, size_t, const void*, void*, void*);
typedef int32_t (*fn_unix)(const uint8_t*, size_t, uint32_t, void*, void*);

static int32_t call_plain(void *fn, const uint8_t *ptr, size_t len, void *out, void *fault) {
	return ((fn_plain)fn)(ptr, len, out, fault);
}
static int32_t call_numeric(void *fn, const uint8_t *ptr, size_t len, const void *format, void *out, void *fault) {
	return ((fn_numeric)fn)(ptr, len, format, out, fault);
}
static int32_t call_unix(void *fn, const uint8_t *ptr, size_t len, uint32_t precision, void *out, void *fault) {
	return ((fn_unix)fn)(ptr, len, precision, out, fault);
}
*/
import "C"

import (
	"fmt"
	"sync"
	"unsafe"
)

// The backend-defined symbol types cast.go's shared doors are written against: here both
// alias a raw dlsym pointer and the calls go through the statically-typed C shims above;
// in backend_purego.go each symbol is a purego-registered typed trampoline instead.
type (
	plainSymbol   = unsafe.Pointer
	numericSymbol = unsafe.Pointer
)

var (
	initOnce sync.Once
	initErr  error

	symBool, symI8, symI16, symI32, symI64, symU8, symU16, symU32, symU64,
	symF32, symF64, symUuid, symTimestamp, symUnix, symDate, symDateOrdered,
	symTime, symDuration unsafe.Pointer
)

// ensureLoaded extracts this platform's embedded native library to a temp file and
// dlopen's it via cgo, exactly once.
func ensureLoaded() error {
	initOnce.Do(func() {
		path, err := extractNativeLib()
		if err != nil {
			initErr = err
			return
		}

		cPath := C.CString(path)
		defer C.free(unsafe.Pointer(cPath))
		handle := C.dlopen(cPath, C.RTLD_NOW|C.RTLD_GLOBAL)
		if handle == nil {
			initErr = fmt.Errorf("hypercast: dlopen failed: %s", C.GoString(C.dlerror()))
			return
		}

		sym := func(name string) unsafe.Pointer {
			if initErr != nil {
				return nil
			}
			cName := C.CString(name)
			defer C.free(unsafe.Pointer(cName))
			p := C.dlsym(handle, cName)
			if p == nil {
				initErr = fmt.Errorf("hypercast: symbol %s not found in native library: %s", name, C.GoString(C.dlerror()))
			}
			return p
		}
		symBool, symUuid = sym("cast_bool"), sym("cast_uuid")
		symI8, symI16, symI32, symI64 = sym("cast_i8"), sym("cast_i16"), sym("cast_i32"), sym("cast_i64")
		symU8, symU16, symU32, symU64 = sym("cast_u8"), sym("cast_u16"), sym("cast_u32"), sym("cast_u64")
		symF32, symF64 = sym("cast_f32"), sym("cast_f64")
		symTimestamp, symUnix = sym("cast_timestamp"), sym("cast_unix")
		symDate, symDateOrdered = sym("cast_date"), sym("cast_date_ordered")
		symTime, symDuration = sym("cast_time"), sym("cast_duration")
	})
	return initErr
}

func callPlain(sym plainSymbol, ptr unsafe.Pointer, length uintptr, out, fault unsafe.Pointer) int32 {
	return int32(C.call_plain(sym, (*C.uint8_t)(ptr), C.size_t(length), out, fault))
}

func callNumeric(sym numericSymbol, ptr unsafe.Pointer, length uintptr, format, out, fault unsafe.Pointer) int32 {
	return int32(C.call_numeric(sym, (*C.uint8_t)(ptr), C.size_t(length), format, out, fault))
}

func callUnix(ptr unsafe.Pointer, length uintptr, precision uint32, out, fault unsafe.Pointer) int32 {
	return int32(C.call_unix(symUnix, (*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(precision), out, fault))
}

// cast_date_ordered shares the unix ABI shape (ptr, len, u32, out, fault) — same C shim.
func callDateOrdered(ptr unsafe.Pointer, length uintptr, order uint32, out, fault unsafe.Pointer) int32 {
	return int32(C.call_unix(symDateOrdered, (*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(order), out, fault))
}
