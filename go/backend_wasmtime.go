//go:build hypercast_wasm

// This backend runs the Rust core as a WebAssembly module inside the Go process, through
// github.com/bytecodealliance/wasmtime-go, instead of dlopen'ing a native build. The module
// is the same C-ABI surface ffi.rs exports — the twenty cast_* doors — compiled for
// wasm32-wasip1 and embedded from native/wasm32-wasip1/hypercast.wasm alongside the
// per-platform shared libraries. It is the inverse of the direction the root README's
// WebAssembly table calls Go's structural blocker: that row is about compiling *this Go
// module* to wasm, which neither cgo nor purego can do; this file is wasm running *inside*
// Go, which needs neither.
//
// Selected by build tag, never automatically: `go build -tags hypercast_wasm`. Reach for it
// on a platform this module ships no native build for, or when a deployment must not dlopen
// anything from a temp file at all (see native_extract.go). Two things to know before you
// do:
//
//   - wasmtime-go is cgo throughout — it links wasmtime's own precompiled static library
//     through the C API. That cuts against the purego story backend_purego.go exists for:
//     this backend needs a working C toolchain on every platform, Windows included, where
//     the default backend deliberately doesn't. The wasmtime-go module is also required in
//     go.mod unconditionally (Go has no tag-conditional requirements), so it lands in every
//     consumer's module graph; it only compiles into a binary built with the tag.
//   - Every door is one host-to-guest call, and the crossing is the cost: the README's
//     WebAssembly section has the measured numbers against the cgo backend. There is no
//     batch door to amortize it behind here — that is round three's chunk layer — so a
//     per-cell workload pays it per cell.
//
// Memory protocol: a wasm guest sees only its own linear memory, so nothing here passes a Go
// pointer across. The input text is copied into a grow-only guest buffer, and the three
// out-params the doors fill — the 16-byte out-value, the 8-byte fault span, the 12-byte
// NumFormat — are guest allocations made once at load. All of it comes from the module's
// exported malloc (wasi-libc's, which Rust's std allocator on this target already sits on),
// never a host-picked offset past the data segments: dlmalloc claims the tail of the initial
// memory on first use, and HyperUuid observed a buffer written there corrupted by the
// guest's next allocation. Results are copied out of the guest into the by-value result
// cast.go reads. One Store and one Instance serve the whole process, created lazily and
// serialized under a mutex — a wasmtime Store is not safe for concurrent use.
package hypercast

import (
	"encoding/binary"
	"fmt"
	"sync"
	"unsafe"

	"github.com/bytecodealliance/wasmtime-go/v48"
)

const wasmModulePath = "native/wasm32-wasip1/hypercast.wasm"

// The backend-defined symbol types cast.go's shared doors are written against: here every
// symbol is a guest export, invoked through wasmtime's host-to-guest entry; in
// backend_cgo.go both alias a raw dlsym pointer, in backend_purego.go a typed trampoline.
type (
	plainSymbol   = *wasmtime.Func
	numericSymbol = *wasmtime.Func
)

// wasmCore is the one embedded module instance the process shares. Every field after
// store is a guest export or a guest address; none is meaningful outside mu.
type wasmCore struct {
	mu    sync.Mutex
	store *wasmtime.Store
	mem   *wasmtime.Memory

	malloc, free *wasmtime.Func

	// Guest addresses allocated once for the life of the process: the out-value (16 bytes
	// covers the widest door, 8-aligned by malloc because the temporal doors write an
	// i64/u64 into it), the fault span, and the NumFormat the numeric doors read.
	out, fault, format int32

	// Grow-only guest buffer the input text is copied into: freed and re-malloc'd only when
	// an input is longer than any before it, so a steady stream of ordinary scalars costs no
	// allocator traffic at all.
	in    int32
	inCap int

	// The format currently written at `format`. Formats are reused values in practice
	// (Invariant, Detect, a per-locale literal), so a stream of same-format numeric casts
	// writes the 12 bytes once — the same memo the Java, Python and Ruby bindings keep.
	lastFormat  rawNumFormat
	formatValid bool
}

var (
	initOnce sync.Once
	initErr  error
	core     *wasmCore

	symBool, symUuid, symTimestamp, symDate, symTime, symDuration plainSymbol

	symI8, symI16, symI32, symI64, symU8, symU16, symU32, symU64,
	symF32, symF64 numericSymbol

	// cast_unix, cast_excel_serial, cast_date_ordered and cast_datetime share the unix ABI
	// shape (ptr, len, u32, out, fault).
	symUnix, symDateOrdered, symDateTime, symExcelSerial *wasmtime.Func
)

// ensureLoaded instantiates the embedded wasm module exactly once. The name and signature
// match the native backends so cast.go needs no knowledge of which one it got.
func ensureLoaded() error {
	initOnce.Do(func() {
		core, initErr = newWasmCore()
	})
	return initErr
}

func newWasmCore() (*wasmCore, error) {
	wasm, err := nativeFS.ReadFile(wasmModulePath)
	if err != nil {
		return nil, fmt.Errorf("hypercast: %s not found in embedded native libs (this module was built without the wasm32-wasip1 module): %w", wasmModulePath, err)
	}

	engine := wasmtime.NewEngine()
	module, err := wasmtime.NewModule(engine, wasm)
	if err != nil {
		return nil, fmt.Errorf("hypercast: compiling wasm module: %w", err)
	}
	// The module imports four WASI preview1 functions — the environ/fd_write/proc_exit set
	// wasi-libc's startup and panic paths reference; the core itself needs no clock and no
	// entropy. An empty WasiConfig satisfies them: no files, no env, nothing inherited.
	linker := wasmtime.NewLinker(engine)
	if err := linker.DefineWasi(); err != nil {
		return nil, fmt.Errorf("hypercast: defining WASI imports: %w", err)
	}
	store := wasmtime.NewStore(engine)
	store.SetWasi(wasmtime.NewWasiConfig())
	instance, err := linker.Instantiate(store, module)
	if err != nil {
		return nil, fmt.Errorf("hypercast: instantiating wasm module: %w", err)
	}

	c := &wasmCore{store: store}
	memExport := instance.GetExport(store, "memory")
	if memExport == nil || memExport.Memory() == nil {
		return nil, fmt.Errorf("hypercast: wasm module exports no memory")
	}
	c.mem = memExport.Memory()

	exports := []struct {
		name string
		dst  **wasmtime.Func
	}{
		{"malloc", &c.malloc}, {"free", &c.free},
		{"cast_bool", &symBool}, {"cast_uuid", &symUuid},
		{"cast_i8", &symI8}, {"cast_i16", &symI16}, {"cast_i32", &symI32}, {"cast_i64", &symI64},
		{"cast_u8", &symU8}, {"cast_u16", &symU16}, {"cast_u32", &symU32}, {"cast_u64", &symU64},
		{"cast_f32", &symF32}, {"cast_f64", &symF64},
		{"cast_timestamp", &symTimestamp}, {"cast_unix", &symUnix},
		{"cast_excel_serial", &symExcelSerial},
		{"cast_date", &symDate}, {"cast_date_ordered", &symDateOrdered},
		{"cast_datetime", &symDateTime},
		{"cast_time", &symTime}, {"cast_duration", &symDuration},
	}
	for _, e := range exports {
		f := instance.GetFunc(store, e.name)
		if f == nil {
			return nil, fmt.Errorf("hypercast: export %s not found in wasm module", e.name)
		}
		*e.dst = f
	}

	if c.out, err = c.alloc(16); err != nil {
		return nil, err
	}
	if c.fault, err = c.alloc(8); err != nil {
		return nil, err
	}
	if c.format, err = c.alloc(12); err != nil {
		return nil, err
	}
	return c, nil
}

// alloc asks the guest allocator for n bytes and returns the guest address.
func (c *wasmCore) alloc(n int) (int32, error) {
	v, err := c.malloc.Call(c.store, int32(n))
	if err != nil {
		return 0, fmt.Errorf("hypercast: guest malloc(%d) trapped: %w", n, err)
	}
	p, _ := v.(int32)
	if p == 0 {
		return 0, fmt.Errorf("hypercast: guest malloc(%d) returned null", n)
	}
	return p, nil
}

// data is the guest's linear memory as a byte slice. Only valid until the next guest call
// that could grow memory (malloc can), so it is re-taken after every call, never cached.
func (c *wasmCore) data() []byte { return c.mem.UnsafeData(c.store) }

// stageInput copies the caller's bytes into the guest and returns their guest address —
// 0 (the ABI's NULL) for empty input, which the core never dereferences.
func (c *wasmCore) stageInput(ptr unsafe.Pointer, length uintptr) int32 {
	if length == 0 {
		return 0
	}
	n := int(length)
	if n > c.inCap {
		if c.in != 0 {
			if _, err := c.free.Call(c.store, c.in); err != nil {
				panic(fmt.Sprintf("hypercast: guest free trapped: %v", err))
			}
			c.in, c.inCap = 0, 0
		}
		p, err := c.alloc(n)
		if err != nil {
			panic(err)
		}
		c.in, c.inCap = p, n
	}
	copy(c.data()[c.in:int(c.in)+n], unsafe.Slice((*byte)(ptr), n))
	return c.in
}

// writeFormat stages the declared NumFormat in the guest, skipping the write when it is the
// one already there.
func (c *wasmCore) writeFormat(format rawNumFormat) {
	if c.formatValid && format == c.lastFormat {
		return
	}
	data := c.data()
	binary.LittleEndian.PutUint32(data[c.format:], format.DecimalSep)
	binary.LittleEndian.PutUint32(data[c.format+4:], format.GroupSep)
	binary.LittleEndian.PutUint32(data[c.format+8:], format.Flags)
	c.lastFormat, c.formatValid = format, true
}

// call invokes a guest door and returns its verdict code. A trap here is a bug in the core
// or this file (an out-of-bounds address), not a recoverable condition, so it panics the
// same way the native backends' contract-violation path does.
func (c *wasmCore) call(f *wasmtime.Func, args ...interface{}) int32 {
	v, err := f.Call(c.store, args...)
	if err != nil {
		panic(fmt.Sprintf("hypercast: door trapped inside the wasm core: %v", err))
	}
	rc, _ := v.(int32)
	return rc
}

// finish reads the verdict back out of the guest into the by-value result cast.go's doors
// consume: the out-value on success, the fault span on a failure code, nothing on a
// contract violation. wasm memory is little-endian by specification and every platform this
// module supports is too, so the sixteen bytes copy straight into the host-native layout
// read[V] reinterprets.
func (c *wasmCore) finish(rc int32) result {
	r := result{code: rc}
	data := c.data()
	switch {
	case rc == 0:
		copy(unsafe.Slice((*byte)(unsafe.Pointer(&r.out)), 16), data[c.out:c.out+16])
	case rc > 0:
		r.fault.Offset = binary.LittleEndian.Uint32(data[c.fault:])
		r.fault.Length = binary.LittleEndian.Uint32(data[c.fault+4:])
	}
	return r
}

func callPlain(sym plainSymbol, ptr unsafe.Pointer, length uintptr) result {
	c := core
	c.mu.Lock()
	defer c.mu.Unlock()
	in := c.stageInput(ptr, length)
	return c.finish(c.call(sym, in, int32(length), c.out, c.fault))
}

func callNumeric(sym numericSymbol, ptr unsafe.Pointer, length uintptr, format rawNumFormat) result {
	c := core
	c.mu.Lock()
	defer c.mu.Unlock()
	in := c.stageInput(ptr, length)
	c.writeFormat(format)
	return c.finish(c.call(sym, in, int32(length), c.format, c.out, c.fault))
}

// declared is the shared body of the four doors that take a caller-declared u32 —
// precision, epoch, or field order.
func (c *wasmCore) declared(f *wasmtime.Func, ptr unsafe.Pointer, length uintptr, discriminant uint32) result {
	c.mu.Lock()
	defer c.mu.Unlock()
	in := c.stageInput(ptr, length)
	return c.finish(c.call(f, in, int32(length), int32(discriminant), c.out, c.fault))
}

func callUnix(ptr unsafe.Pointer, length uintptr, precision uint32) result {
	return core.declared(symUnix, ptr, length, precision)
}

func callDateOrdered(ptr unsafe.Pointer, length uintptr, order uint32) result {
	return core.declared(symDateOrdered, ptr, length, order)
}

func callDateTime(ptr unsafe.Pointer, length uintptr, order uint32) result {
	return core.declared(symDateTime, ptr, length, order)
}

func callExcelSerial(ptr unsafe.Pointer, length uintptr, epoch uint32) result {
	return core.declared(symExcelSerial, ptr, length, epoch)
}
