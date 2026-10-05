//go:build tinygo.wasm

// The same backend for TinyGo compiling to WebAssembly — a browser (`-target=wasm`, whose
// wasm_exec.js is TinyGo's own) or a WASI runtime (`-target=wasip1`). Stock Go cannot do
// this: its wasm toolchain links Go code only. TinyGo compiles through LLVM, links with
// wasm-ld against wasi-libc, and has cgo, so the core is an ordinary archive on the link
// line again: staticlib/wasm/libhypercast.a, the core built for wasm32-wasip1 — the same
// bytes as Swift's WebAssembly archive. Even `-target=wasm` is a wasm32-wasi build
// underneath, so the object links unchanged, and it imports nothing at all: parsing needs
// no clock, no randomness and no I/O. Nothing is fetched or instantiated beside the app's
// own module.
//
// The selector is the tinygo.wasm tag, which every TinyGo WebAssembly target sets. Not
// `cgo` (TinyGo never sets it) and not `wasm` (wasip2 and wasm-unknown report GOARCH=arm).
//
// Three limits of TinyGo's cgo make this a file of its own rather than more lines in
// backend_static.go:
//
//   - A #cgo line cannot carry a build constraint (`not implemented: build constraints in
//     #cgo line`, tinygo-org/tinygo#4087), so backend_static.go cannot even be parsed.
//     This file's //go:build does the selecting and its one #cgo line is unconditional.
//   - ${SRCDIR} is not expanded, and a bare archive path is refused as an invalid flag. A
//     relative -L is resolved against the package directory instead, which is where the
//     module cache puts staticlib/ too.
//   - A C struct cannot cross by value (tinygo-org/tinygo#4489), so backend_static.go's
//     shims, which hand the whole verdict back as one struct, are no use here. Each door
//     instead calls the core's export directly with pointers to the Go result's out-value
//     and fault span and to the Go copy of the format — the layouts cast.go already
//     declares to match ffi.rs — the way the core's exports take them anyway.
package hypercast

/*
#cgo LDFLAGS: -Lstaticlib/wasm -lhypercast
#include <stddef.h>
#include <stdint.h>

// The core's C ABI — rust/src/ffi.rs, the twenty-six exports every binding calls.
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
int32_t cast_decimal_from_f64(double value, void *out, void *fault);
int32_t cast_excel_serial_from_f64(double value, uint32_t epoch, void *out, void *fault);
int32_t cast_excel_time(double value, void *out, void *fault);
int32_t cast_excel_duration(double value, void *out, void *fault);
*/
import "C"

import "unsafe"

// The backend-defined symbol types cast.go's doors are written against: here each door is a
// Go function that calls its export directly (TinyGo's cgo has no use for a C function
// pointer cast.go could hold); backend_static.go's are the exports' linked addresses.
type (
	plainSymbol   func(ptr *C.uint8_t, length C.size_t, out, fault unsafe.Pointer) C.int32_t
	numericSymbol func(ptr *C.uint8_t, length C.size_t, format, out, fault unsafe.Pointer) C.int32_t
)

var (
	symBool plainSymbol = func(p *C.uint8_t, n C.size_t, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_bool(p, n, out, fault)
	}
	symUuid plainSymbol = func(p *C.uint8_t, n C.size_t, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_uuid(p, n, out, fault)
	}
	symTimestamp plainSymbol = func(p *C.uint8_t, n C.size_t, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_timestamp(p, n, out, fault)
	}
	symDate plainSymbol = func(p *C.uint8_t, n C.size_t, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_date(p, n, out, fault)
	}
	symTime plainSymbol = func(p *C.uint8_t, n C.size_t, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_time(p, n, out, fault)
	}
	symDuration plainSymbol = func(p *C.uint8_t, n C.size_t, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_duration(p, n, out, fault)
	}

	symI8 numericSymbol = func(p *C.uint8_t, n C.size_t, format, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_i8(p, n, format, out, fault)
	}
	symI16 numericSymbol = func(p *C.uint8_t, n C.size_t, format, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_i16(p, n, format, out, fault)
	}
	symI32 numericSymbol = func(p *C.uint8_t, n C.size_t, format, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_i32(p, n, format, out, fault)
	}
	symI64 numericSymbol = func(p *C.uint8_t, n C.size_t, format, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_i64(p, n, format, out, fault)
	}
	symU8 numericSymbol = func(p *C.uint8_t, n C.size_t, format, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_u8(p, n, format, out, fault)
	}
	symU16 numericSymbol = func(p *C.uint8_t, n C.size_t, format, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_u16(p, n, format, out, fault)
	}
	symU32 numericSymbol = func(p *C.uint8_t, n C.size_t, format, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_u32(p, n, format, out, fault)
	}
	symU64 numericSymbol = func(p *C.uint8_t, n C.size_t, format, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_u64(p, n, format, out, fault)
	}
	symF32 numericSymbol = func(p *C.uint8_t, n C.size_t, format, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_f32(p, n, format, out, fault)
	}
	symF64 numericSymbol = func(p *C.uint8_t, n C.size_t, format, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_f64(p, n, format, out, fault)
	}
	symDecimal numericSymbol = func(p *C.uint8_t, n C.size_t, format, out, fault unsafe.Pointer) C.int32_t {
		return C.cast_decimal(p, n, format, out, fault)
	}
)

// packedVersion is the core's own version, major<<16 | minor<<8 | patch, read through the
// ABI rather than from this module, so NativeVersion reports the archive that was linked.
func packedVersion() uint32 { return uint32(C.hypercast_version()) }

func callPlain(sym plainSymbol, ptr unsafe.Pointer, length uintptr) (r result) {
	r.code = int32(sym((*C.uint8_t)(ptr), C.size_t(length), unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault)))
	return r
}

func callNumeric(sym numericSymbol, ptr unsafe.Pointer, length uintptr, format rawNumFormat) (r result) {
	r.code = int32(sym((*C.uint8_t)(ptr), C.size_t(length), unsafe.Pointer(&format),
		unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault)))
	return r
}

func callUnix(ptr unsafe.Pointer, length uintptr, precision uint32) (r result) {
	r.code = int32(C.cast_unix((*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(precision),
		unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault)))
	return r
}

func callDateOrdered(ptr unsafe.Pointer, length uintptr, order uint32) (r result) {
	r.code = int32(C.cast_date_ordered((*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(order),
		unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault)))
	return r
}

func callDateTime(ptr unsafe.Pointer, length uintptr, order uint32) (r result) {
	r.code = int32(C.cast_datetime((*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(order),
		unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault)))
	return r
}

func callExcelSerial(ptr unsafe.Pointer, length uintptr, epoch uint32) (r result) {
	r.code = int32(C.cast_excel_serial((*C.uint8_t)(ptr), C.size_t(length), C.uint32_t(epoch),
		unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault)))
	return r
}

func callDecimalFromF64(value float64) (r result) {
	r.code = int32(C.cast_decimal_from_f64(C.double(value), unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault)))
	return r
}

func callExcelSerialFromF64(value float64, epoch uint32) (r result) {
	r.code = int32(C.cast_excel_serial_from_f64(C.double(value), C.uint32_t(epoch),
		unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault)))
	return r
}

func callExcelTime(value float64) (r result) {
	r.code = int32(C.cast_excel_time(C.double(value), unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault)))
	return r
}

func callExcelDuration(value float64) (r result) {
	r.code = int32(C.cast_excel_duration(C.double(value), unsafe.Pointer(&r.out), unsafe.Pointer(&r.fault)))
	return r
}
