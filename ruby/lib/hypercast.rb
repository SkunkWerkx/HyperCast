# Allocation-lean scalar casts — booleans, numerics, UUIDs, temporals — calling directly
# into the native libhypercast shared library via Fiddle. Every door returns a verdict:
# Success or Fault (a closed reason plus the offending byte span), never an exception for
# bad data — the only exceptions here are caller bugs (a malformed NumFormat), never data.
#
# Consume with Ruby's own pattern matching over the two Data case types:
#
#   case HyperCast.i32("(1,234)", HyperCast::NumFormat::INVARIANT)
#   in HyperCast::Success(value:) then puts "got #{value}"          # -1234
#   in HyperCast::Fault(reason:, offset:) then puts "#{reason} at byte #{offset}"
#   end
#
# Door names mirror the native ABI (i32, f64, timestamp, ...) so the polyglot surface reads
# identically across bindings. Ruby-flavored fidelity: Integer is unbounded (u64 comes back
# as the true unsigned value), Time carries full nanoseconds across the whole 0001-9999
# window, time-of-day is an exact Integer of nanoseconds since midnight, and durations come
# back as exact Rational seconds — no truncation anywhere, and no wrapping: Ruby and the
# JVM are the fidelity kings of this roster.

require "date"
require_relative "hypercast/native_platform"
require_relative "hypercast/runtime"

module HyperCast
  # The success case of a verdict: a cast value.
  Success = Data.define(:value)

  # The failure case: a closed reason Symbol (:empty, :malformed, :out_of_range) plus the
  # offending span as byte offsets into the UTF-8 input. Nothing is captured — slicing the
  # offending text out of the input is the caller's choice.
  Fault = Data.define(:reason, :offset, :length)

  REASONS = { 1 => :empty, 2 => :malformed, 3 => :out_of_range }.freeze

  # Caller-declared numeric notation for the integer and real doors — declared out loud
  # (INVARIANT, or a literal), never defaulted, the same stance every binding takes.
  # Equal separators are a caller bug (ArgumentError), never a verdict.
  NumFormat = Data.define(:decimal_sep, :group_sep, :flags) do
    def initialize(decimal_sep:, group_sep:, flags:)
      raise ArgumentError, "separators must be single characters" unless
        decimal_sep.is_a?(String) && decimal_sep.length == 1 &&
        group_sep.is_a?(String) && group_sep.length == 1
      raise ArgumentError, "decimal and group separators must differ; both are #{decimal_sep.inspect}" if
        decimal_sep == group_sep
      super
    end

    def packed
      [decimal_sep.ord, group_sep.ord, flags].pack("L<L<L<")
    end
  end

  # Permit the group separator between digits (sizes not validated — between digits is the rule).
  GROUPING = 1
  # Permit accounting parentheses as negation: (1,234) is -1234.
  PARENTHESES = 1 << 1
  # Permit exponent notation. Integer doors reject a negative exponent.
  EXPONENT = 1 << 2
  # Permit 0x/&H/0b two's-complement radix prefixes (0xFF is -1 for an i8).
  RADIX_PREFIXES = 1 << 3
  # Permit a trailing %, dividing by 100. Real doors only.
  PERCENT = 1 << 4
  # Every lenience on.
  ALL_STYLES = GROUPING | PARENTHESES | EXPONENT | RADIX_PREFIXES | PERCENT

  NumFormat::INVARIANT = NumFormat.new(decimal_sep: ".", group_sep: ",", flags: ALL_STYLES)

  # The declared unit of a Unix-epoch value — no magnitude guessing, ever.
  UNIX_PRECISIONS = { seconds: 1, milliseconds: 2, microseconds: 3, nanoseconds: 4 }.freeze

  class << self
    # Presents a verdict optionally: an :empty fault becomes nil (Ruby's absent),
    # everything else flows through untouched.
    def optional(verdict)
      return nil if verdict in Fault(reason: :empty)

      verdict
    end

    # Casts boolean text: true/false plus the conventions untrusted sources actually send
    # (t/f, yes/no, y/n, 1/0, on/off, enabled/disabled, active/inactive,
    # checked/unchecked, in/out), ASCII case-insensitive.
    def bool(text)
      plain(:cast_bool, text, 1) { |out| out.unpack1("C") != 0 }
    end

    { i8: "c", i16: "s<", i32: "l<", i64: "q<", u8: "C", u16: "S<", u32: "L<", u64: "Q<" }
      .each do |door, unpack|
      sizes = { "c" => 1, "C" => 1, "s<" => 2, "S<" => 2, "l<" => 4, "L<" => 4, "q<" => 8, "Q<" => 8 }
      size = sizes.fetch(unpack)
      # Integer doors: the target type's own range, declared grouping, accounting parens,
      # non-negative exponent, and 0x/&H/0b two's-complement radix prefixes. Ruby Integer
      # is unbounded, so u64 comes back as the true unsigned value.
      define_method(door) do |text, format|
        numeric(:"cast_#{door}", text, format, size) { |out| out.unpack1(unpack) }
      end
    end

    # Real doors: finite values only, declared separators, parens, exponent, percent.
    def f32(text, format)
      numeric(:cast_f32, text, format, 4) { |out| out.unpack1("e") }
    end

    def f64(text, format)
      numeric(:cast_f64, text, format, 8) { |out| out.unpack1("E") }
    end

    # Casts UUID text — all five .NET Guid formats (D/N/B/P/X) plus urn:uuid:/GUID:/UUID:
    # prefixes — to Ruby's UUID lingua franca: the lowercase hyphenated String (the same
    # shape SecureRandom.uuid returns).
    def uuid(text)
      plain(:cast_uuid, text, 16) do |out|
        out.unpack("H8H4H4H4H12").join("-")
      end
    end

    # Casts an RFC 3339 instant — zone mandatory — to a UTC Time at full nanosecond
    # fidelity across the whole 0001-9999 window.
    def timestamp(text)
      plain(:cast_timestamp, text, 16) { |out| instant(out) }
    end

    # Casts an integer Unix-epoch value under a caller-declared unit Symbol
    # (:seconds/:milliseconds/:microseconds/:nanoseconds) to a UTC Time. An unknown unit
    # is a caller bug (KeyError), never a verdict.
    def unix(text, precision)
      code = UNIX_PRECISIONS.fetch(precision)
      bytes = utf8(text)
      out = Fiddle::Pointer.malloc(16, Fiddle::RUBY_FREE)
      fault = Fiddle::Pointer.malloc(8, Fiddle::RUBY_FREE)
      rc = Runtime.call(:cast_unix, input_ptr(bytes), bytes.bytesize, code, out, fault)
      verdict(rc, fault) { instant(out[0, 16]) }
    end

    # Casts a strict ISO 8601 yyyy-MM-dd calendar date to a Date.
    def date(text)
      plain(:cast_date, text, 4) do |out|
        year, month, day = out.unpack("S<CC")
        Date.new(year, month, day)
      end
    end

    # Casts an ISO 24-hour time-of-day to an exact Integer of nanoseconds since midnight
    # (Ruby has no time-of-day type; the integer keeps every digit).
    def time(text)
      plain(:cast_time, text, 8) { |out| out.unpack1("Q<") }
    end

    # Casts a duration (ISO 8601 fixed components, invariant colon form, or protobuf JSON
    # seconds) to exact Rational seconds — full fidelity across the core's ±10,000-year
    # window, no wrapping and no truncation.
    def duration(text)
      plain(:cast_duration, text, 16) do |out|
        seconds, nanos = out.unpack("q<l<")
        Rational(seconds * 1_000_000_000 + nanos, 1_000_000_000)
      end
    end

    private

    def utf8(text)
      text.encode(Encoding::UTF_8).b
    end

    def input_ptr(bytes)
      bytes.empty? ? nil : Fiddle::Pointer.to_ptr(bytes)
    end

    def verdict(rc, fault)
      if rc.zero?
        Success.new(value: yield)
      elsif rc == -1
        raise "hypercast: libhypercast reported a contract violation — a binding bug, please report it"
      else
        offset, length = fault[0, 8].unpack("L<L<")
        Fault.new(reason: REASONS.fetch(rc), offset: offset, length: length)
      end
    end

    def plain(symbol, text, out_size)
      bytes = utf8(text)
      out = Fiddle::Pointer.malloc(out_size, Fiddle::RUBY_FREE)
      fault = Fiddle::Pointer.malloc(8, Fiddle::RUBY_FREE)
      rc = Runtime.call(symbol, input_ptr(bytes), bytes.bytesize, out, fault)
      verdict(rc, fault) { yield(out[0, out_size]) }
    end

    def numeric(symbol, text, format, out_size)
      bytes = utf8(text)
      raw_format = format.packed
      out = Fiddle::Pointer.malloc(out_size, Fiddle::RUBY_FREE)
      fault = Fiddle::Pointer.malloc(8, Fiddle::RUBY_FREE)
      rc = Runtime.call(symbol, input_ptr(bytes), bytes.bytesize,
                        Fiddle::Pointer.to_ptr(raw_format), out, fault)
      verdict(rc, fault) { yield(out[0, out_size]) }
    end

    def instant(bytes)
      seconds, nanos = bytes.unpack("q<l<")
      Time.at(seconds, nanos, :nanosecond, in: "UTC")
    end
  end
end
