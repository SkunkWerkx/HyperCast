// The C ABI of the hypercast core: the twenty-six functions rust/src/ffi.rs exports, which
// is everything the static libraries in this bundle define. The Swift binding imports this
// as the module HyperCastCore where the core is linked in (Linux and WebAssembly); every
// other binding declares the same signatures in its own language.
//
// Every door reads `len` bytes of UTF-8 at `ptr` (never dereferenced when `len` is 0) and
// returns 0 with the value written to `out`, or a positive reason code with the offending
// byte span written to `fault` (two uint32_t: offset, length). -1 is a contract violation
// by the caller. `out`, `format` and `fault` are untyped here because this binding fills
// and reads them as raw bytes; their layouts are the #[repr(C)] types in rust/src/ffi.rs
// and rust/src/verdict.rs. A null `format` means the invariant number format. The four typed
// doors at the end read a double instead of text, so their fault span is always empty.
#ifndef HYPERCAST_H
#define HYPERCAST_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// The crate version, packed major << 16 | minor << 8 | patch.
uint32_t hypercast_version(void);

int32_t cast_bool(const uint8_t *ptr, uintptr_t len, void *out, void *fault);

int32_t cast_i8(const uint8_t *ptr, uintptr_t len, const void *format, void *out, void *fault);
int32_t cast_i16(const uint8_t *ptr, uintptr_t len, const void *format, void *out, void *fault);
int32_t cast_i32(const uint8_t *ptr, uintptr_t len, const void *format, void *out, void *fault);
int32_t cast_i64(const uint8_t *ptr, uintptr_t len, const void *format, void *out, void *fault);
int32_t cast_u8(const uint8_t *ptr, uintptr_t len, const void *format, void *out, void *fault);
int32_t cast_u16(const uint8_t *ptr, uintptr_t len, const void *format, void *out, void *fault);
int32_t cast_u32(const uint8_t *ptr, uintptr_t len, const void *format, void *out, void *fault);
int32_t cast_u64(const uint8_t *ptr, uintptr_t len, const void *format, void *out, void *fault);
int32_t cast_f32(const uint8_t *ptr, uintptr_t len, const void *format, void *out, void *fault);
int32_t cast_f64(const uint8_t *ptr, uintptr_t len, const void *format, void *out, void *fault);
int32_t cast_decimal(const uint8_t *ptr, uintptr_t len, const void *format, void *out, void *fault);

int32_t cast_uuid(const uint8_t *ptr, uintptr_t len, void *out, void *fault);

int32_t cast_timestamp(const uint8_t *ptr, uintptr_t len, void *out, void *fault);
int32_t cast_unix(const uint8_t *ptr, uintptr_t len, uint32_t precision, void *out, void *fault);
int32_t cast_excel_serial(const uint8_t *ptr, uintptr_t len, uint32_t epoch, void *out, void *fault);
int32_t cast_date(const uint8_t *ptr, uintptr_t len, void *out, void *fault);
int32_t cast_date_ordered(const uint8_t *ptr, uintptr_t len, uint32_t order, void *out, void *fault);
int32_t cast_datetime(const uint8_t *ptr, uintptr_t len, uint32_t order, void *out, void *fault);
int32_t cast_time(const uint8_t *ptr, uintptr_t len, void *out, void *fault);
int32_t cast_duration(const uint8_t *ptr, uintptr_t len, void *out, void *fault);

int32_t cast_decimal_from_f64(double value, void *out, void *fault);
int32_t cast_excel_serial_from_f64(double value, uint32_t epoch, void *out, void *fault);
int32_t cast_excel_time(double value, void *out, void *fault);
int32_t cast_excel_duration(double value, void *out, void *fault);

#ifdef __cplusplus
}
#endif

#endif
