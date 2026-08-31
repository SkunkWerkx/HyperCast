require "date"
require_relative "hypercast/native_platform"
require_relative "hypercast/runtime"

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
module HyperCast
  # This gem's own version — kept in lockstep with hypercast.gemspec by the
  # prepare-release workflow, so the two can never drift apart again.
  VERSION = "0.1.0"

  # The success case of a verdict: a cast value.
  Success = Data.define(:value)

  # The failure case: a closed reason Symbol (:empty, :malformed, :out_of_range) plus the
  # offending span as byte offsets into the UTF-8 input. Nothing is captured — slicing the
  # offending text out of the input is the caller's choice.
  Fault = Data.define(:reason, :offset, :length)

  # The native core's failure codes, mapped to the closed reason Symbols a Fault carries.
  REASONS = { 1 => :empty, 2 => :malformed, 3 => :out_of_range }.freeze

  # Caller-declared numeric notation for the integer and real doors — declared out loud
  # (INVARIANT, or a literal), never defaulted, the same stance every binding takes.
  # Equal separators are a caller bug (ArgumentError), never a verdict.
  NumFormat = Data.define(:decimal_sep, :group_sep, :flags) do
    # Validates the declared separators up front — single characters, and distinct from
    # each other — so a malformed format fails loudly as the caller bug it is.
    def initialize(decimal_sep:, group_sep:, flags:)
      raise ArgumentError, "separators must be single characters" unless
        decimal_sep.is_a?(String) && decimal_sep.length == 1 &&
        group_sep.is_a?(String) && group_sep.length == 1
      raise ArgumentError, "decimal and group separators must differ; both are #{decimal_sep.inspect}" if
        decimal_sep == group_sep
      super
    end

    # The 12-byte little-endian form the native ABI's NumFormat struct expects.
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
  # Resolve the ./, roles per input from structure instead of the declared separators
  # (which are ignored while this flag is set). Detection, not sniffing: a repeated
  # separator is grouping ("1.234.567,89"); with both present the rightmost is the
  # decimal; a single separator with a non-3-digit right run is the decimal ("3,1415");
  # with exactly 3 digits right, only a 0 integer part proves decimal ("0,785").
  # Genuinely ambiguous input ("12.185", "1,000") is a :malformed Fault at the separator,
  # never guessed.
  SEPARATOR_DETECT = 1 << 5
  # Every lenience on (SEPARATOR_DETECT is a separator policy, not a lenience, and is
  # deliberately not included).
  ALL_STYLES = GROUPING | PARENTHESES | EXPONENT | RADIX_PREFIXES | PERCENT

  # The invariant profile — '.' decimal, ',' grouping, every lenience on.
  NumFormat::INVARIANT = NumFormat.new(decimal_sep: ".", group_sep: ",", flags: ALL_STYLES)

  # The detection profile — every lenience on, ./, roles resolved per input by
  # SEPARATOR_DETECT's structural rules.
  NumFormat::DETECT = NumFormat.new(decimal_sep: ".", group_sep: ",",
                                    flags: ALL_STYLES | SEPARATOR_DETECT)

  # The declared unit of a Unix-epoch value — no magnitude guessing, ever.
  UNIX_PRECISIONS = { seconds: 1, milliseconds: 2, microseconds: 3, nanoseconds: 4 }.freeze

  # The declared field order of a separated calendar date — no guessing, ever: "1/7/2026"
  # is January 7th (:month_day_year, the en-US order) or July 1st (:day_month_year, the
  # en-GB order) only because the caller said which.
  DATE_ORDERS = { year_month_day: 1, month_day_year: 2, day_month_year: 3 }.freeze

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

    # Casts real text to an IEEE single (widened losslessly on the way out): finite values
    # only, declared separators and grouping, parens, exponent, and trailing percent.
    def f32(text, format)
      numeric(:cast_f32, text, format, 4) { |out| out.unpack1("e") }
    end

    # Casts real text to an IEEE double. Notation rules as f32.
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
      out, fault, = scratch
      rc = Runtime.call(:cast_unix, input_ptr(bytes), bytes.bytesize, code, out, fault)
      verdict(rc, fault) { instant(out[0, 16]) }
    end

    # Casts a calendar date to a Date. With no order declared: the strict ISO 8601
    # yyyy-MM-dd form only. With a declared order Symbol (:year_month_day,
    # :month_day_year, :day_month_year), also the separated forms — "1/7/2026" is
    # January 7th or July 1st only because the caller said which; an unknown order is a
    # caller bug (KeyError), never a verdict.
    def date(text, order = nil)
      if order.nil?
        plain(:cast_date, text, 4) do |out|
          year, month, day = out.unpack("S<CC")
          Date.new(year, month, day)
        end
      else
        code = DATE_ORDERS.fetch(order)
        bytes = utf8(text)
        out, fault, = scratch
        rc = Runtime.call(:cast_date_ordered, input_ptr(bytes), bytes.bytesize, code, out, fault)
        verdict(rc, fault) do
          year, month, day = out[0, 4].unpack("S<CC")
          Date.new(year, month, day)
        end
      end
    end

    # Casts a zone-less civil date-time — the shape untrusted feeds actually send
    # ("1/7/2026 3:04 PM", "2026-01-07 15:04:05") — under a declared order Symbol to a
    # stdlib DateTime with exact Rational seconds. No zone is *read*: the text names no
    # instant, and the parse applies no offset. Ruby has no zone-less date-time type, so
    # the value rides a DateTime, whose offset defaults to +00:00 — that zero is a carrier
    # artifact, not data (the same caveat PHP's UTC-labeled DateTimeImmutable carries);
    # fusing a real zone is the caller's job, and timestamp stays the strict RFC 3339
    # instant door. An unknown order is a caller bug (KeyError).
    def datetime(text, order)
      code = DATE_ORDERS.fetch(order)
      bytes = utf8(text)
      out, fault, = scratch
      rc = Runtime.call(:cast_datetime, input_ptr(bytes), bytes.bytesize, code, out, fault)
      verdict(rc, fault) do
        year, month, day, nanos = out[0, 16].unpack("S<CCx4Q<")
        second_of_day, frac = nanos.divmod(1_000_000_000)
        hour, rest = second_of_day.divmod(3600)
        minute, second = rest.divmod(60)
        DateTime.new(year, month, day, hour, minute, second + Rational(frac, 1_000_000_000))
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

    # Encodings whose bytes already are the UTF-8 (or byte-identical) form the core reads.
    BYTE_COMPATIBLE = [Encoding::UTF_8, Encoding::US_ASCII, Encoding::ASCII_8BIT].freeze

    # Presents the input as UTF-8 bytes: already-compatible text crosses as-is (Fiddle
    # passes a String's bytes for void* directly — no Pointer wrapper, no dup); only
    # foreign encodings pay a transcode.
    def utf8(text)
      BYTE_COMPATIBLE.include?(text.encoding) ? text : text.encode(Encoding::UTF_8)
    end

    # Fiddle spells a null pointer as nil — the core's contract for empty input.
    def input_ptr(bytes)
      bytes.empty? ? nil : bytes
    end

    # One 36-byte scratch allocation per thread, reused by every call: out-value at 0
    # (16 bytes covers every door), fault span at 16, NumFormat at 24. Two
    # Fiddle::Pointer.malloc(..., RUBY_FREE) calls per cast — each registering a GC
    # finalizer — measured as the dominant per-call cost by an order of magnitude.
    def scratch
      Thread.current[:hypercast_scratch] ||= begin
        base = Fiddle::Pointer.malloc(36, Fiddle::RUBY_FREE)
        [base, base + 16, base + 24]
      end
    end

    # Presents a native return code as the verdict union: 0 yields a Success, a failure
    # code becomes a Fault, and -1 (contract violation) is a binding bug that raises.
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

    # The shared body of every format-free door: one native call over the scratch buffers.
    def plain(symbol, text, out_size)
      bytes = utf8(text)
      out, fault, = scratch
      rc = Runtime.call(symbol, input_ptr(bytes), bytes.bytesize, out, fault)
      verdict(rc, fault) { yield(out[0, out_size]) }
    end

    # The shared body of the integer/real doors: plain, plus the packed NumFormat.
    def numeric(symbol, text, format, out_size)
      bytes = utf8(text)
      out, fault, raw_format = scratch
      raw_format[0, 12] = packed_cache[format]
      rc = Runtime.call(symbol, input_ptr(bytes), bytes.bytesize, raw_format, out, fault)
      verdict(rc, fault) { yield(out[0, out_size]) }
    end

    # Identity-keyed memo of NumFormat#packed — formats are reused constants in practice,
    # and re-packing per call is a measurable allocation. The race is benign (idempotent).
    def packed_cache
      @packed_cache ||= Hash.new { |cache, format| cache[format] = format.packed }
    end

    # Builds a UTC Time from the core's protobuf-shaped {seconds, nanos} pair, exactly.
    def instant(bytes)
      seconds, nanos = bytes.unpack("q<l<")
      Time.at(seconds, nanos, :nanosecond, in: "UTC")
    end
  end
end

# --- backend selection: the Magnus extension, when present, replaces the doors above in
# place on this module (no delegation layer) — Fiddle's measured 1.6 µs per-call floor
# drops to an ordinary extension call. The pure-Fiddle definitions stay the universal
# zero-compile fallback; precompiled platform gems are how the extension ships without
# ever making a consumer compile anything. Set HYPERCAST_PURE=1 to force Fiddle.
HyperCast::BACKEND =
  if ENV["HYPERCAST_PURE"]
    :fiddle
  else
    begin
      require "hypercast_native"
      :native
    rescue LoadError
      :fiddle
    end
  end
