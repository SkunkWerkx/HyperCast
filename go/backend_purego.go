//go:build !(cgo && (darwin || linux))

// This backend calls libhypercast through github.com/ebitengine/purego — dlopen/dlsym plus
// per-arch call trampolines, no cgo and no C compiler required. It's the only backend on
// Windows (see backend_cgo.go's doc comment for why) and the fallback everywhere else: any
// darwin/linux build with CGO_ENABLED=0 — including, per Go's own default, any
// cross-compile to a non-native GOOS/GOARCH — lands here automatically, with no action
// needed from a consumer of this module.
//
// The trampoline costs more per call than the cgo backend's real C call sites — which is
// exactly why cgo stays the primary backend where it's available (HyperUuid measured
// purego's overhead the hard way) — but a slower door beats an unbuildable one.
package hypercast

import (
	"fmt"
	"sync"
	"unsafe"

	"github.com/ebitengine/purego"
)

// The backend-defined symbol types cast.go's shared doors are written against: here each
// "symbol" is a purego-registered typed trampoline; in backend_cgo.go both alias
// unsafe.Pointer and the calls go through real C call sites.
type (
	plainSymbol   = func(ptr unsafe.Pointer, length uintptr, out, fault unsafe.Pointer) int32
	numericSymbol = func(ptr unsafe.Pointer, length uintptr, format, out, fault unsafe.Pointer) int32
	unixSymbol    = func(ptr unsafe.Pointer, length uintptr, precision uint32, out, fault unsafe.Pointer) int32
)

var (
	initOnce sync.Once
	initErr  error

	symBool, symUuid, symTimestamp, symDate, symTime, symDuration plainSymbol

	symI8, symI16, symI32, symI64, symU8, symU16, symU32, symU64,
	symF32, symF64 numericSymbol

	// cast_date_ordered, cast_datetime and cast_excel_serial share the unix ABI shape
	// (ptr, len, u32, out, fault).
	symUnix, symDateOrdered, symDateTime, symExcelSerial unixSymbol
)

// ensureLoaded extracts this platform's embedded native library to a temp file and
// dlopen's it via purego, exactly once.
func ensureLoaded() error {
	initOnce.Do(func() {
		path, err := extractNativeLib()
		if err != nil {
			initErr = err
			return
		}

		handle, err := openLibrary(path)
		if err != nil {
			initErr = fmt.Errorf("hypercast: loading native library: %w", err)
			return
		}

		purego.RegisterLibFunc(&symBool, handle, "cast_bool")
		purego.RegisterLibFunc(&symUuid, handle, "cast_uuid")
		purego.RegisterLibFunc(&symTimestamp, handle, "cast_timestamp")
		purego.RegisterLibFunc(&symDate, handle, "cast_date")
		purego.RegisterLibFunc(&symTime, handle, "cast_time")
		purego.RegisterLibFunc(&symDuration, handle, "cast_duration")
		purego.RegisterLibFunc(&symI8, handle, "cast_i8")
		purego.RegisterLibFunc(&symI16, handle, "cast_i16")
		purego.RegisterLibFunc(&symI32, handle, "cast_i32")
		purego.RegisterLibFunc(&symI64, handle, "cast_i64")
		purego.RegisterLibFunc(&symU8, handle, "cast_u8")
		purego.RegisterLibFunc(&symU16, handle, "cast_u16")
		purego.RegisterLibFunc(&symU32, handle, "cast_u32")
		purego.RegisterLibFunc(&symU64, handle, "cast_u64")
		purego.RegisterLibFunc(&symF32, handle, "cast_f32")
		purego.RegisterLibFunc(&symF64, handle, "cast_f64")
		purego.RegisterLibFunc(&symUnix, handle, "cast_unix")
		purego.RegisterLibFunc(&symDateOrdered, handle, "cast_date_ordered")
		purego.RegisterLibFunc(&symDateTime, handle, "cast_datetime")
		purego.RegisterLibFunc(&symExcelSerial, handle, "cast_excel_serial")
	})
	return initErr
}

// The same by-value result shape the cgo backend returns, filled through pointers here:
// purego's trampoline boxes its arguments per call regardless, so the heap escape the
// cgo shims exist to avoid is not the cost that dominates this backend.
func callPlain(sym plainSymbol, ptr unsafe.Pointer, length uintptr) result {
	var r result
	r.code = sym(ptr, length, unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault))
	return r
}

func callNumeric(sym numericSymbol, ptr unsafe.Pointer, length uintptr, format rawNumFormat) result {
	var r result
	r.code = sym(ptr, length, unsafe.Pointer(&format), unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault))
	return r
}

func callUnix(ptr unsafe.Pointer, length uintptr, precision uint32) result {
	var r result
	r.code = symUnix(ptr, length, precision, unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault))
	return r
}

func callDateOrdered(ptr unsafe.Pointer, length uintptr, order uint32) result {
	var r result
	r.code = symDateOrdered(ptr, length, order, unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault))
	return r
}

func callExcelSerial(ptr unsafe.Pointer, length uintptr, epoch uint32) result {
	var r result
	r.code = symExcelSerial(ptr, length, epoch, unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault))
	return r
}

func callDateTime(ptr unsafe.Pointer, length uintptr, order uint32) result {
	var r result
	r.code = symDateTime(ptr, length, order, unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault))
	return r
}
