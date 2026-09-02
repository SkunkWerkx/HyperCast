//go:build cgo && (darwin || linux)

// The cgo backend — HyperUuid's measured lesson applied from day one: purego's per-call
// trampoline allocations would eat scalar parsing alive (on its purego backend, HyperUuid's
// batch API was 19.6x faster than per-call precisely because of that overhead; on cgo the
// two were a wash), so scalar-per-call HyperCast runs on real C calls wherever cgo is
// available. backend_purego.go is the fallback half of the pair — Windows, CGO_ENABLED=0,
// and cross-compiles land there automatically.
//
// cgo can't call an opaquely-typed void* function pointer directly — it needs a real,
// statically-typed C call site — hence the three per-signature shims below, one per ABI
// shape (plain, numeric, unix).
//
// The shims own the out-params. Every door used to declare `var out T; var fault rawFault`
// and pass `unsafe.Pointer(&out)` across — and any Go pointer handed to a cgo call escapes
// to the heap, which is where the one-to-three allocations per successful cast came from
// (`go build -gcflags=-m` says so, door by door: "moved to heap: out"). Now the C side
// declares the out-value, the fault span and the format on its own stack and hands the
// whole verdict back BY VALUE as one struct, so no Go pointer crosses at all except the
// input bytes — and those are a pointer *value* into memory that already exists, not an
// address taken of a local. Success-path allocations on this backend: zero.
package hypercast

/*
#include <dlfcn.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

typedef int32_t (*fn_plain)(const uint8_t*, size_t, void*, void*);
typedef int32_t (*fn_numeric)(const uint8_t*, size_t, const void*, void*, void*);
typedef int32_t (*fn_unix)(const uint8_t*, size_t, uint32_t, void*, void*);

typedef struct { uint32_t offset; uint32_t len; } hc_fault;
typedef struct { uint32_t decimal_sep; uint32_t group_sep; uint32_t flags; } hc_format;

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
static hc_result call_numeric(void *fn, const uint8_t *ptr, size_t len, uint32_t decimal_sep, uint32_t group_sep, uint32_t flags) {
	hc_result r = {{0, 0}, 0, 0, 0};
	hc_fault f = {0, 0};
	hc_format format = {decimal_sep, group_sep, flags};
	r.code = ((fn_numeric)fn)(ptr, len, &format, r.out, &f);
	r.offset = f.offset;
	r.len = f.len;
	return r;
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
	symDateTime, symTime, symDuration, symExcelSerial unsafe.Pointer
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
		symDateTime = sym("cast_datetime")
		symTime, symDuration = sym("cast_time"), sym("cast_duration")
		symExcelSerial = sym("cast_excel_serial")
	})
	return initErr
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
	return fromC(C.call_numeric(sym, (*C.uint8_t)(ptr), C.size_t(length),
		C.uint32_t(format.DecimalSep), C.uint32_t(format.GroupSep), C.uint32_t(format.Flags)))
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
