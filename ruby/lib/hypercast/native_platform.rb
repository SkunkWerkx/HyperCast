module HyperCast
  # Maps the running RUBY_PLATFORM to the RID-style directory (matching the other bindings'
  # runtimes/{rid}/native/ / native/{rid}/ convention) and native library filename to load.
  module NativePlatform
    class UnsupportedPlatformError < StandardError; end

    def self.rid_and_library_name
      is_arm = RUBY_PLATFORM.match?(/arm64|aarch64/)

      case RUBY_PLATFORM
      when /mingw|mswin|windows/
        is_arm ? ["win-arm64", "hypercast.dll"] : ["win-x64", "hypercast.dll"]
      when /darwin/
        is_arm ? ["osx-arm64", "libhypercast.dylib"] : ["osx-x64", "libhypercast.dylib"]
      when /linux/
        is_arm ? ["linux-arm64", "libhypercast.so"] : ["linux-x64", "libhypercast.so"]
      else
        raise UnsupportedPlatformError, "hypercast: unsupported platform RUBY_PLATFORM=#{RUBY_PLATFORM}"
      end
    end
  end
end
