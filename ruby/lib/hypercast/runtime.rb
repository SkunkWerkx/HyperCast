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
      cast_f32: NUMERIC, cast_f64: NUMERIC,
      cast_uuid: PLAIN,
      cast_timestamp: PLAIN, cast_unix: UNIX, cast_excel_serial: UNIX,
      cast_date: PLAIN, cast_date_ordered: UNIX, cast_datetime: UNIX,
      cast_time: PLAIN, cast_duration: PLAIN
    }.freeze

    @mutex = Mutex.new
    @functions = nil

    class << self
      # The door's Fiddle::Function, for the caller to invoke directly — a splat-through
      # `call(symbol, *args)` here built and re-splatted an Array on every cast.
      def function(symbol)
        functions.fetch(symbol)
      end

      private

      # Loaded lazily and exactly once; the native library and its function pointers live
      # for the process's lifetime, same as every other binding (never dlclose'd). The
      # unsynchronized read is the fast path — a per-call mutex acquisition measured as a
      # real slice of the door cost; the benign race re-checks under the lock.
      def functions
        @functions || @mutex.synchronize { @functions ||= load_functions }
      end

      def load_functions
        rid, lib_name = NativePlatform.rid_and_library_name
        path = File.join(NATIVE_DIR, rid, lib_name)
        unless File.exist?(path)
          # Development loop: fall back to the in-repo cargo build, exactly what the other
          # bindings' local staging does.
          repo_build = File.expand_path(File.join(__dir__, "../../../rust/target/release", lib_name))
          path = repo_build if File.exist?(repo_build)
        end
        unless File.exist?(path)
          raise LoadError,
                "hypercast: #{path} not found (unsupported platform, or this gem was built " \
                "without a native library for it)"
        end

        handle = Fiddle.dlopen(path)
        DOORS.to_h do |name, signature|
          [name, Fiddle::Function.new(handle[name.to_s], signature, Fiddle::TYPE_INT32_T)]
        end
      end
    end
  end
end
