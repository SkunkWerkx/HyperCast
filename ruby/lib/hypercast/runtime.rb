require "fiddle"

module HyperCast
  # Fiddle plumbing for the native libhypercast shared library — dlopen/dlsym plus raw
  # C-ABI calls, no runtime bridge. Fiddle ships with every Ruby install; it's a plain gem
  # dependency here rather than a third-party one. A Ruby gem's files are already plain
  # files on disk once installed, so native/{rid}/{lib} dlopen's directly — no extraction.
  module Runtime
    NATIVE_DIR = File.join(__dir__, "native")

    PLAIN = [Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T, Fiddle::TYPE_VOIDP, Fiddle::TYPE_VOIDP].freeze
    NUMERIC = [Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T, Fiddle::TYPE_VOIDP, Fiddle::TYPE_VOIDP,
               Fiddle::TYPE_VOIDP].freeze
    UNIX = [Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T, Fiddle::TYPE_UINT32_T, Fiddle::TYPE_VOIDP,
            Fiddle::TYPE_VOIDP].freeze

    DOORS = {
      cast_bool: PLAIN,
      cast_i8: NUMERIC, cast_i16: NUMERIC, cast_i32: NUMERIC, cast_i64: NUMERIC,
      cast_u8: NUMERIC, cast_u16: NUMERIC, cast_u32: NUMERIC, cast_u64: NUMERIC,
      cast_f32: NUMERIC, cast_f64: NUMERIC, cast_decimal: NUMERIC,
      cast_uuid: PLAIN,
      cast_timestamp: PLAIN, cast_unix: UNIX, cast_excel_serial: UNIX,
      cast_date: PLAIN, cast_date_ordered: UNIX, cast_datetime: UNIX,
      cast_time: PLAIN, cast_duration: PLAIN
    }.freeze

    # The one export that is not a door: the zero-argument version probe, returning the
    # core's packed version word (major << 16 | minor << 8 | patch) rather than a verdict
    # code — so it gets its own Fiddle signature beside DOORS instead of a row in it.
    VERSION_PROBE = :hypercast_version

    @mutex = Mutex.new
    @functions = nil

    class << self
      # The door's (or VERSION_PROBE's) Fiddle::Function, for the caller to invoke directly
      # — a splat-through `call(symbol, *args)` here built and re-splatted an Array on
      # every cast.
      def function(symbol)
        functions.fetch(symbol)
      end

      # Whether this platform has a shared library for Fiddle to dlopen at all: a known RID
      # and the file actually present — in this install, or in the in-repo cargo build the
      # dev loop falls back to. Backend selection (hypercast.rb) asks this before falling
      # back to the WebAssembly backend, which needs neither.
      def fiddle_library_available?
        !library_path.nil?
      rescue NativePlatform::UnsupportedPlatformError
        false
      end

      private

      # The shared library to dlopen: this install's native/{rid}/{lib}, or — the
      # development loop — the in-repo cargo build, exactly what the other bindings' local
      # staging does. Nil when neither exists.
      def library_path
        rid, lib_name = NativePlatform.rid_and_library_name
        path = File.join(NATIVE_DIR, rid, lib_name)
        return path if File.exist?(path)

        repo_build = File.expand_path(File.join(__dir__, "../../../rust/target/release", lib_name))
        File.exist?(repo_build) ? repo_build : nil
      end

      # Loaded lazily and exactly once; the native library and its function pointers live
      # for the process's lifetime, same as every other binding (never dlclose'd). The
      # unsynchronized read is the fast path — a per-call mutex acquisition measured as a
      # real slice of the door cost; the benign race re-checks under the lock.
      def functions
        @functions || @mutex.synchronize { @functions ||= load_functions }
      end

      def load_functions
        path = library_path
        if path.nil?
          rid, lib_name = NativePlatform.rid_and_library_name
          raise LoadError,
                "hypercast: #{File.join(NATIVE_DIR, rid, lib_name)} not found (unsupported " \
                "platform, or this gem was built without a native library for it)"
        end

        handle = Fiddle.dlopen(path)
        functions = DOORS.to_h do |name, signature|
          [name, Fiddle::Function.new(handle[name.to_s], signature, Fiddle::TYPE_INT32_T)]
        end
        functions[VERSION_PROBE] =
          Fiddle::Function.new(handle[VERSION_PROBE.to_s], [], Fiddle::TYPE_UINT32_T)
        functions
      end
    end
  end
end
