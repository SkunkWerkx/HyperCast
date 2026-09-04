require "wasmtime"

module HyperCast
  # The WebAssembly backend: the same Rust core, compiled once to a `wasm32-wasip1` module
  # (lib/hypercast/native/wasm32-wasip1/hypercast.wasm) and run inside this process by the
  # `wasmtime` gem — no per-platform shared library needed, and nothing dlopen'd. This is the
  # inverse of ruby.wasm: not Ruby running inside a wasm sandbox, but a wasm module running
  # inside Ruby.
  #
  # Same integration shape as the Magnus extension: on require (after hypercast.rb has
  # defined the pure-Fiddle module) this redefines methods **in place** on HyperCast — no
  # delegation layer, no second surface. The Magnus extension replaces every public door;
  # this file replaces only the four private bodies underneath them (`plain`, `numeric`,
  # `declared`, `packed_version` — the whole native crossing), so every door, every verdict
  # type, `utf8`, `verdict`, `instant` and the Symbol tables stay the exact code the other
  # two backends run, byte for byte.
  #
  # Two things differ from the native backends, both forced by the sandbox:
  #
  # * A wasm guest only sees its own linear memory, so no Fiddle::Pointer can cross. The
  #   input text is copied into a grow-only guest buffer, and the three out-params the doors
  #   fill — the 16-byte out-value, the 8-byte fault span, the 32-byte NumFormat — are guest
  #   allocations made once at load, all from the module's own exported `malloc` (the
  #   wasi-libc allocator Rust's std already uses on this target) and read back with
  #   `Memory#read`. Using the guest's allocator rather than a host-picked offset is
  #   load-bearing: dlmalloc claims the tail of the initial memory on first use, and
  #   HyperUuid observed a buffer written at a guessed offset corrupted by the guest's next
  #   allocation.
  # * A `Wasmtime::Store` is single-threaded, so every call is serialized under one Mutex
  #   around one shared instance per process.
  module WasmRuntime
    MODULE_PATH = File.join(Runtime::NATIVE_DIR, "wasm32-wasip1", "hypercast.wasm")
    # Development loop: the in-repo cargo build, the same fallback runtime.rb takes for the
    # native library.
    REPO_BUILD_PATH = File.expand_path(
      File.join(__dir__, "../../../rust/target/wasm32-wasip1/release/hypercast.wasm")
    )

    DOORS = Runtime::DOORS.keys.freeze

    # One instantiated module: the exported functions, the exported memory, and the guest
    # buffers every door crosses through for the life of the process.
    class Instance
      attr_reader :memory, :out, :fault, :format

      def initialize
        path = File.exist?(MODULE_PATH) ? MODULE_PATH : REPO_BUILD_PATH
        unless File.exist?(path)
          raise LoadError,
                "hypercast: #{MODULE_PATH} not found (this gem was built without its WebAssembly module)"
        end

        engine = Wasmtime::Engine.new
        mod = Wasmtime::Module.from_file(engine, path)
        linker = Wasmtime::Linker.new(engine)
        # The module imports four WASI preview1 functions — wasi-libc's startup and panic
        # plumbing (environ_*, fd_write, proc_exit); the core itself needs no clock and no
        # entropy. A default WasiConfig — no stdio, no filesystem, no environment — is all
        # any of them need.
        Wasmtime::WASI::P1.add_to_linker_sync(linker)
        store = Wasmtime::Store.new(engine, wasi_p1_config: Wasmtime::WasiConfig.new)
        instance = linker.instantiate(store, mod)

        @memory = instance.export("memory").to_memory
        @fn = (DOORS + [Runtime::VERSION_PROBE] + %i[malloc free]).to_h do |name|
          export = instance.export(name.to_s) or
            raise LoadError, "hypercast: #{path} does not export #{name} (a module older than this binding)"
          [name, export.to_func]
        end
        # dlmalloc hands back 8-byte-aligned (in fact 16) blocks, which covers the
        # decimal out-value's 8-byte alignment.
        @out = malloc(16)
        @fault = malloc(8)
        @format = malloc(32)
        # Grow-only buffer for the input text, so a steady stream of ordinary scalars never
        # touches the guest allocator again.
        @in = 0
        @in_cap = 0
        # The format currently written at @format, memoized by identity: formats are reused
        # constants in practice, and NumFormat is a frozen Data, so one `equal?` skips the
        # 32-byte write on the overwhelming majority of numeric calls. The Instance holds
        # the format it memoized, so the key can never be a recycled object.
        @last_format = nil
      end

      def fn(name)
        @fn.fetch(name)
      end

      def malloc(size)
        ptr = @fn[:malloc].call(size)
        raise NoMemoryError, "hypercast: wasm malloc(#{size}) failed" if ptr.zero?

        ptr
      end

      # Copies the input into the guest and returns its address — 0 (the ABI's NULL) for
      # empty input, which the core never dereferences.
      def stage_input(bytes)
        size = bytes.bytesize
        return 0 if size.zero?

        if size > @in_cap
          @fn[:free].call(@in) unless @in.zero?
          @in = malloc(size)
          @in_cap = size
        end
        @memory.write(@in, bytes)
        @in
      end

      def stage_format(format)
        unless format.equal?(@last_format)
          @memory.write(@format, format.packed)
          @last_format = format
        end
        @format
      end
    end

    @mutex = Mutex.new
    @instance = nil

    # Every call holds the one lock: the Store is single-threaded, and the lazily created
    # instance is shared by every thread for the life of the process (never torn down,
    # same as the dlopen'd library natively).
    def self.with_instance
      @mutex.synchronize do
        @instance ||= Instance.new
        yield @instance
      end
    end
  end

  class << self
    private

    # The four crossing bodies, redefined over the guest. Each door body reads the
    # out-value back only on success and the fault span only on a failure code, so
    # `verdict` sees exactly what the Fiddle path hands it.
    def plain(symbol, text, out_size)
      bytes = utf8(text)
      WasmRuntime.with_instance do |w|
        rc = w.fn(symbol).call(w.stage_input(bytes), bytes.bytesize, w.out, w.fault)
        verdict(rc, rc.zero? ? nil : w.memory.read(w.fault, 8), bytes) { yield(w.memory.read(w.out, out_size)) }
      end
    end

    def numeric(symbol, text, format, out_size)
      bytes = utf8(text)
      WasmRuntime.with_instance do |w|
        rc = w.fn(symbol).call(w.stage_input(bytes), bytes.bytesize, w.stage_format(format), w.out, w.fault)
        verdict(rc, rc.zero? ? nil : w.memory.read(w.fault, 8), bytes) { yield(w.memory.read(w.out, out_size)) }
      end
    end

    def declared(symbol, text, code, out_size)
      bytes = utf8(text)
      WasmRuntime.with_instance do |w|
        rc = w.fn(symbol).call(w.stage_input(bytes), bytes.bytesize, code, w.out, w.fault)
        verdict(rc, rc.zero? ? nil : w.memory.read(w.fault, 8), bytes) { yield(w.memory.read(w.out, out_size)) }
      end
    end

    def packed_version
      WasmRuntime.with_instance { |w| w.fn(Runtime::VERSION_PROBE).call }
    end
  end
end
