# Autoloaded, not required: the platform gems carry no library for Fiddle to open and do not
# declare fiddle, so loading it eagerly would make `require "hypercast"` fail under Bundler
# wherever fiddle is a bundled rather than a default gem (Ruby 4.0). It loads the first time
# the Fiddle backend runs, and every path there goes through `functions` first, so a platform
# gem forced onto Fiddle still gets missing_library_message rather than a bare LoadError.
autoload :Fiddle, "fiddle"

module HyperCast
  # Fiddle plumbing for the native libhypercast shared library — dlopen/dlsym plus raw
  # C-ABI calls, no runtime bridge. Fiddle ships with every Ruby install; it's a plain gem
  # dependency of the universal gem (see hypercast.gemspec) rather than a third-party one.
  # A Ruby gem's files are already plain files on disk once installed, so native/{rid}/{lib}
  # dlopen's directly — no extraction.
  module Runtime
    NATIVE_DIR = File.join(__dir__, "native")

    # Each door's C signature, by shape — resolved to Fiddle types only in load_functions,
    # so nothing here touches Fiddle until the Fiddle backend actually runs: plain is
    # (text, len, out, fault), numeric adds the packed NumFormat pointer before out, and
    # declared a caller-declared u32 (precision, epoch, field order) in the same place. The
    # typed doors take a double instead of the text and its length: typed is (value, out,
    # fault), typed_declared adds the u32 after the value.
    DOORS = {
      cast_bool: :plain,
      cast_i8: :numeric, cast_i16: :numeric, cast_i32: :numeric, cast_i64: :numeric,
      cast_u8: :numeric, cast_u16: :numeric, cast_u32: :numeric, cast_u64: :numeric,
      cast_f32: :numeric, cast_f64: :numeric, cast_decimal: :numeric,
      cast_uuid: :plain,
      cast_timestamp: :plain, cast_unix: :declared, cast_excel_serial: :declared,
      cast_date: :plain, cast_date_ordered: :declared, cast_datetime: :declared,
      cast_time: :plain, cast_duration: :plain,
      cast_decimal_from_f64: :typed, cast_excel_serial_from_f64: :typed_declared,
      cast_excel_time: :typed, cast_excel_duration: :typed
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

      # Every Fiddle allocation goes through here, and loads the library first: that is what
      # keeps a missing library reported as missing_library_message instead of as whatever
      # touching Fiddle raises first. The per-thread scratch buffer and each packed NumFormat
      # (hypercast.rb) pay for it once apiece.
      def buffer(size)
        functions
        Fiddle::Pointer.malloc(size, Fiddle::RUBY_FREE)
      end

      private

      # The shared library to dlopen: this install's native/{rid}/{lib}, or — the
      # development loop — the in-repo cargo build, exactly what the other bindings' local
      # staging does. Nil when neither exists.
      def library_path
        Interop.library_path("hypercast", NATIVE_DIR, File.expand_path("../../..", __dir__))
      end

      # Why Fiddle found nothing to load. A precompiled platform gem is the one install where
      # that is by design rather than a gap: it carries only its Magnus extensions, and the
      # Fiddle backend is reached there only by forcing it (HYPERCAST_PURE) or because none
      # of its extensions loaded — a gem RubyGems matched to a Ruby it was not built for,
      # such as the glibc Linux gem that `gem install` on RubyGems 3.x picks on Alpine. So
      # that case names its fix, the universal gem, which carries every platform's library,
      # instead of a missing path that reads like a packaging bug. Both arguments are
      # parameters only so the specs can ask for every wording.
      def missing_library_message(gem_platform = Gem.loaded_specs["hypercast"]&.platform,
                                  forced = ENV.key?("HYPERCAST_PURE"))
        rid, lib_name = NativePlatform.rid_and_library_name
        missing = File.join(NATIVE_DIR, rid, lib_name)
        if gem_platform && gem_platform.to_s != Gem::Platform::RUBY
          reason =
            if forced
              "HYPERCAST_PURE forces the Fiddle backend (unset it to use the extension)"
            else
              "none of its extensions loads on this Ruby (#{RUBY_VERSION}, #{RUBY_PLATFORM})"
            end
          "hypercast: this #{gem_platform} platform gem carries only Magnus extensions, no " \
            "Fiddle library, and #{reason}. The universal gem has the Fiddle backend for every " \
            "platform: `gem install hypercast --platform ruby`, or Bundler's force_ruby_platform " \
            "(#{missing} not found)"
        else
          "hypercast: #{missing} not found (unsupported platform, or this gem was built " \
            "without a native library for it)"
        end
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
        raise LoadError, missing_library_message if path.nil?

        handle = Fiddle.dlopen(path)
        plain = [Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T, Fiddle::TYPE_VOIDP, Fiddle::TYPE_VOIDP]
        signatures = {
          plain: plain,
          numeric: plain.dup.insert(2, Fiddle::TYPE_VOIDP),
          declared: plain.dup.insert(2, Fiddle::TYPE_UINT32_T),
          typed: [Fiddle::TYPE_DOUBLE, Fiddle::TYPE_VOIDP, Fiddle::TYPE_VOIDP],
          typed_declared: [Fiddle::TYPE_DOUBLE, Fiddle::TYPE_UINT32_T, Fiddle::TYPE_VOIDP, Fiddle::TYPE_VOIDP]
        }
        functions = DOORS.to_h do |name, shape|
          [name, Fiddle::Function.new(handle[name.to_s], signatures.fetch(shape), Fiddle::TYPE_INT32_T)]
        end
        functions[VERSION_PROBE] =
          Fiddle::Function.new(handle[VERSION_PROBE.to_s], [], Fiddle::TYPE_UINT32_T)
        functions
      end
    end
  end
end
