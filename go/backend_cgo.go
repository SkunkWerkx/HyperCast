//go:build cgo && (darwin || linux)

// The cgo backend — HyperUuid's measured lesson applied from day one: purego's per-call
// trampoline allocations would eat scalar parsing alive (its batch API was 19.6x faster
// than per-call precisely because of that overhead), so scalar-per-call HyperCast starts
// on real C calls. A purego fallback for Windows and CGO_ENABLED=0 builds lands with the
// CI round, mirroring HyperUuid's backend pair; until then this module requires cgo on
// darwin/linux.
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

var (
	initOnce sync.Once
	initErr  error

	symbols map[string]unsafe.Pointer
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

		symbols = make(map[string]unsafe.Pointer, 17)
		for _, name := range []string{
			"cast_bool", "cast_i8", "cast_i16", "cast_i32", "cast_i64",
			"cast_u8", "cast_u16", "cast_u32", "cast_u64", "cast_f32", "cast_f64",
			"cast_uuid", "cast_timestamp", "cast_unix", "cast_date", "cast_time",
			"cast_duration",
		} {
			cName := C.CString(name)
			sym := C.dlsym(handle, cName)
			C.free(unsafe.Pointer(cName))
			if sym == nil {
				initErr = fmt.Errorf("hypercast: symbol %s not found in native library: %s", name, C.GoString(C.dlerror()))
				return
			}
			symbols[name] = sym
		}
	})
	return initErr
}

func callPlain(symbol string, ptr unsafe.Pointer, length uintptr, out, fault unsafe.Pointer) int32 {
	return int32(C.call_plain(symbols[symbol], (*C.uint8_t)(ptr), C.size_t(length), out, fault))
}

func callNumeric(symbol string, ptr unsafe.Pointer, length uintptr, format, out, fault unsafe.Pointer) int32 {
	return int32(C.call_numeric(symbols[symbol], (*C.uint8_t)(ptr), C.size_t(length), format, out, fault))
}

func callUnix(ptr unsafe.Pointer, length uintptr, precision uint32, out, fault unsafe.Pointer) int32 {
	return int32(C.call_unix(symbols["cast_unix"], (*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(precision), out, fault))
}
