require "date"
require_relative "hypercast/native_platform"
require_relative "hypercast/runtime"

# Allocation-lean scalar casts — booleans, numerics, exact decimals, UUIDs, temporals —
# calling directly into the native libhypercast shared library via Fiddle. Every door
# returns a verdict: Success or Fault (a closed reason plus the offending byte span), never
# an exception for bad data — the only exceptions here are caller bugs (a malformed
# NumFormat), never data.
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
# window, time-of-day is an exact Integer of nanoseconds since midnight, durations come
# back as exact Rational seconds, and the decimal door returns an exact Decimal (sign,
# 96-bit magnitude, base-10 scale) that never rounds — no truncation anywhere, and no
# wrapping: Ruby and the JVM are the fidelity kings of this roster.
module HyperCast
  # This gem's own version — kept in lockstep with hypercast.gemspec by the
  # prepare-release workflow, so the two can never drift apart again.
  VERSION = "0.3.0"

  # The success case of a verdict: a cast value.
  Success = Data.define(:value)

  # The failure case: a closed reason Symbol (:empty, :malformed, :out_of_range) plus the
  # offending span, in the units String#[] slices by on the text you passed: character
  # offsets for text (in any encoding — the core reads UTF-8 and the span is mapped back),
  # byte offsets for a binary (ASCII-8BIT) String, whose characters are its bytes. Either
  # way `text[offset, length]` is the offending text, with no mapping on your side.
  # Nothing is captured — slicing it out of the input is the caller's choice.
  Fault = Data.define(:reason, :offset, :length)

  # The native core's failure codes, mapped to the closed reason Symbols a Fault carries.
  REASONS = { 1 => :empty, 2 => :malformed, 3 => :out_of_range }.freeze

  # Caller-declared numeric notation for the integer, real and decimal doors — declared out
  # loud (INVARIANT, or a literal), never defaulted, the same stance every binding takes.
  # The currency symbol is the one field a culture table would fill in: it is declared
  # here, never looked up, and honored only while the CURRENCY flag is set — declared with
  # the flag off, the symbol is a :malformed Fault at the symbol, and the flag with no
  # symbol ("" — the default) matches nothing. Equal separators, or a symbol longer than
  # 16 UTF-8 bytes or carrying an ASCII digit or ASCII whitespace, are a caller bug
  # (ArgumentError), never a verdict.
  NumFormat = Data.define(:decimal_sep, :group_sep, :flags, :currency) do
    # The widest currency symbol the native ABI carries inline, in UTF-8 bytes.
    CURRENCY_MAX_BYTES = 16

    # Validates the declared separators up front — single characters, and distinct from
    # each other — and the currency symbol (a String of at most 16 UTF-8 bytes with no
    # ASCII digit or ASCII whitespace, since those would collide with the digit scan and
    # the trimming around the symbol), so a malformed format fails loudly as the caller bug
    # it is. The symbol is stored transcoded to UTF-8, the encoding the core reads.
    def initialize(decimal_sep:, group_sep:, flags:, currency: "")
      raise ArgumentError, "separators must be single characters" unless
        decimal_sep.is_a?(String) && decimal_sep.length == 1 &&
        group_sep.is_a?(String) && group_sep.length == 1
      raise ArgumentError, "decimal and group separators must differ; both are #{decimal_sep.inspect}" if
        decimal_sep == group_sep
      raise ArgumentError, "currency symbol must be a String; got #{currency.inspect}" unless currency.is_a?(String)

      symbol = currency.encode(Encoding::UTF_8)
      raise ArgumentError, "currency symbol #{currency.inspect} exceeds #{CURRENCY_MAX_BYTES} UTF-8 bytes" if
        symbol.bytesize > CURRENCY_MAX_BYTES
      raise ArgumentError, "currency symbol #{currency.inspect} must not contain an ASCII digit or whitespace" if
        symbol.match?(/[0-9\t\n\f\r ]/)

      super(decimal_sep: decimal_sep, group_sep: group_sep, flags: flags, currency: symbol)
    end

    # The 32-byte little-endian form the native ABI's NumFormat struct expects: the two
    # separators as code points, the flags, the symbol's byte length, then the symbol's
    # UTF-8 bytes zero-padded to 16.
    def packed
      [decimal_sep.ord, group_sep.ord, flags, currency.bytesize].pack("L<L<L<L<") + [currency].pack("a16")
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
  # Permit a trailing %, dividing by 100. Real and decimal doors only.
  PERCENT = 1 << 4
  # Resolve the ./, roles per input from structure instead of the declared separators
  # (which are ignored while this flag is set). Detection, not sniffing: a repeated
  # separator is grouping ("1.234.567,89"); with both present the rightmost is the
  # decimal; a single separator with a non-3-digit right run is the decimal ("3,1415");
  # with exactly 3 digits right, only a 0 integer part proves decimal ("0,785").
  # Genuinely ambiguous input ("12.185", "1,000") is a :malformed Fault at the separator,
  # never guessed.
  SEPARATOR_DETECT = 1 << 5
  # Permit the format's declared currency symbol at either edge of the numeric body —
  # leading ("$5", "-$5", "$ -5") or trailing ("5 €", "1.234,50 kr."), once, with optional
  # ASCII whitespace between symbol and digits; accounting parentheses wrap the symbol
  # along with the digits ("($5)"). With no symbol declared the flag matches nothing.
  # Integer, real and decimal doors.
  CURRENCY = 1 << 6
  # Every lenience on (SEPARATOR_DETECT is a separator policy, not a lenience, and is
  # deliberately not included).
  ALL_STYLES = GROUPING | PARENTHESES | EXPONENT | RADIX_PREFIXES | PERCENT | CURRENCY

  # The invariant profile — '.' decimal, ',' grouping, every lenience on, no currency
  # symbol declared.
  NumFormat::INVARIANT = NumFormat.new(decimal_sep: ".", group_sep: ",", flags: ALL_STYLES)

  # The detection profile — every lenience on, ./, roles resolved per input by
  # SEPARATOR_DETECT's structural rules.
  NumFormat::DETECT = NumFormat.new(decimal_sep: ".", group_sep: ",",
                                    flags: ALL_STYLES | SEPARATOR_DETECT)

  # The declared unit of a Unix-epoch value — no magnitude guessing, ever.
  UNIX_PRECISIONS = { seconds: 1, milliseconds: 2, microseconds: 3, nanoseconds: 4 }.freeze

  # The date system an Excel serial number is expressed in. Spreadsheets carry no marker
  # for this — it is a workbook-level setting — so the caller states it, the same way
  # UNIX_PRECISIONS and DATE_ORDERS are declared rather than guessed.
  EXCEL_EPOCHS = { y1900: 1, y1904: 2 }.freeze

  # The declared field order of a separated calendar date — no guessing, ever: "1/7/2026"
  # is January 7th (:month_day_year, the en-US order) or July 1st (:day_month_year, the
  # en-GB order) only because the caller said which.
  DATE_ORDERS = { year_month_day: 1, month_day_year: 2, day_month_year: 3 }.freeze

  # An exact decimal, the decimal door's value: an unbounded Integer magnitude (the core's
  # 96-bit unsigned, so at most 2**96 - 1), an Integer base-10 scale (0..28 — the number of
  # places the magnitude is shifted right), and a negative flag. The value is
  # (negative ? -1 : 1) * magnitude * 10**-scale. The triple is canonical: exact trailing
  # zeros in the fraction are always trimmed, so the scale is minimal ("1.10", "1.1" and
  # "1.1000" are all magnitude 11, scale 1; "100" stays magnitude 100, scale 0), and zero
  # is scale 0 and never negative. Nothing but a zero is ever dropped — text carrying more
  # precision than the core holds is an :out_of_range Fault, never an approximation.
  # Convert with to_r (exact Rational), to_s (the canonical text), or to_d (BigDecimal).
  Decimal = Data.define(:magnitude, :scale, :negative) do
    # True for a negative value; zero is never negative.
    def negative?
      negative
    end

    # The value as an exact Rational.
    def to_r
      value = Rational(magnitude, 10**scale)
      negative ? -value : value
    end

    # The canonical text form — "-1234.5", "-0.025", "0": the minimal scale rendered
    # plainly, no exponent — the same string every binding renders and the conformance
    # corpus pins as `value`.
    def to_s
      digits = magnitude.to_s.rjust(scale + 1, "0")
      text = scale.zero? ? digits : "#{digits[0, digits.length - scale]}.#{digits[-scale, scale]}"
      negative ? "-#{text}" : text
    end

    # The value as a BigDecimal, built exactly from the canonical text. Requires the
    # bigdecimal gem (a bundled gem since Ruby 3.4, not a dependency of this one — add it
    # to your Gemfile under Bundler), loaded lazily on first use so a consumer who never
    # calls this pays nothing for it.
    def to_d
      require "bigdecimal"
      BigDecimal(to_s)
    end
  end

  class << self
    # Presents a verdict optionally: an :empty fault becomes nil (Ruby's absent),
    # everything else flows through untouched.
    def optional(verdict)
      return nil if verdict in Fault(reason: :empty)

      verdict
    end

    # Whether a backend actually loaded and exports the ABI this binding was built against
    # — what a consumer with a fallback of its own checks before committing to these doors.
    # Probed once (a native_version round trip: the cheapest call the core has), cached,
    # and never raises: a missing shared library, an older core without the version
    # export, or a wasm module the wasmtime gem cannot instantiate all answer false. The
    # doors themselves keep their own behavior — the first call on an unavailable backend
    # raises its precise LoadError, exactly as before — and so does require-time selection
    # under HYPERCAST_WASM=1 without wasmtime; this only answers the question quietly.
    def available?
      return @available unless @available.nil?

      @available = begin
        native_version.is_a?(String)
      rescue LoadError, StandardError
        false
      end
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
      # Resolved once here, not inside the method: interpolating a Symbol per call built a
      # String and interned it on every integer cast.
      symbol = :"cast_#{door}"
      # Integer doors: the target type's own range, declared grouping, accounting parens,
      # non-negative exponent, and 0x/&H/0b two's-complement radix prefixes. Ruby Integer
      # is unbounded, so u64 comes back as the true unsigned value.
      define_method(door) do |text, format|
        numeric(symbol, text, format, size) { |out| out.unpack1(unpack) }
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

    # Casts decimal text to an exact Decimal — the real doors' grammar (declared separators
    # and grouping, parens, exponent, trailing percent, declared currency), but no float is
    # ever formed: "0.1" is one tenth, "50%" is exactly 0.5 (magnitude 5, scale 1), and
    # "1.10" is canonical magnitude 11, scale 1 — exact trailing zeros are always trimmed.
    # A magnitude past 2**96 - 1, or more fractional precision than 28 places can hold, is
    # an :out_of_range Fault — the door never rounds.
    def decimal(text, format)
      numeric(:cast_decimal, text, format, 16) do |out|
        lo, hi, scale, negative = out.unpack("Q<L<CCx2")
        Decimal.new(magnitude: (hi << 64) | lo, scale: scale, negative: negative != 0)
      end
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
      declared(:cast_unix, text, UNIX_PRECISIONS.fetch(precision), 16) { |out| instant(out) }
    end

    # Casts an Excel date serial under a caller-declared epoch Symbol (:y1900/:y1904) to a
    # UTC Time. The whole part counts days from the system's own day zero and the fraction
    # is the time of day, so "45292.75" is 2024-01-01T18:00:00Z; a cell carries no zone and
    # none is invented.
    #
    # The 1900 system contains a day that never existed: serial 60 is 1900-02-29, kept
    # deliberately because Lotus 1-2-3 wrongly treated 1900 as a leap year and Excel copied
    # the bug for file compatibility. It is :malformed here — the same verdict .date gives
    # the text "1900-02-29" — so every serial above it is shifted one day against a naive
    # count. An unknown epoch is a caller bug (KeyError), never a verdict.
    def excel_serial(text, epoch)
      declared(:cast_excel_serial, text, EXCEL_EPOCHS.fetch(epoch), 16) { |out| instant(out) }
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
        declared(:cast_date_ordered, text, DATE_ORDERS.fetch(order), 4) do |out|
          year, month, day = out.unpack("S<CC")
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
      declared(:cast_datetime, text, DATE_ORDERS.fetch(order), 16) do |out|
        year, month, day, nanos = out.unpack("S<CCx4Q<")
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

    # The version of the native core actually loaded, as "major.minor.patch" — read from
    # the library itself, not from this gem, so a consumer can prove the core behind the
    # doors is the one this binding was built against (and name the mismatch when it is
    # not). The cheapest possible probe that the backend resolved at all: takes nothing,
    # cannot fail.
    def native_version
      word = packed_version
      "#{word >> 16}.#{(word >> 8) & 0xFF}.#{word & 0xFF}"
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

    # One 24-byte scratch allocation per thread, reused by every call: out-value at 0
    # (16 bytes covers every door, and malloc's alignment covers the decimal's 8), fault
    # span at 16. Two Fiddle::Pointer.malloc(..., RUBY_FREE) calls per cast — each
    # registering a GC finalizer — measured as the dominant per-call cost by an order of
    # magnitude. The NumFormat no longer lives here: each format owns its own packed
    # pointer (see packed_cache), so a numeric call copies nothing into scratch before
    # crossing.
    def scratch
      Thread.current[:hypercast_scratch] ||= begin
        base = Fiddle::Pointer.malloc(24, Fiddle::RUBY_FREE)
        [base, base + 16]
      end
    end

    # Presents a native return code as the verdict union: 0 yields a Success, a failure
    # code becomes a Fault over `bytes` (the UTF-8 the core read), and -1 (contract
    # violation) is a binding bug that raises.
    def verdict(rc, fault, bytes)
      if rc.zero?
        Success.new(value: yield)
      elsif rc == -1
        raise "hypercast: libhypercast reported a contract violation — a binding bug, please report it"
      else
        offset, length = characters(bytes, *fault[0, 8].unpack("L<L<"))
        Fault.new(reason: REASONS.fetch(rc), offset: offset, length: length)
      end
    end

    # The core's byte span in the units String#[] slices by: an identity for a binary
    # String (its characters are its bytes) and for ASCII text (`ascii_only?` reads the
    # cached coderange — no scan), a byte-to-character remap otherwise. Character counts
    # survive transcoding, so a span mapped on the UTF-8 form indexes the caller's own
    # String whatever encoding it arrived in. Failure path only: a Success never pays.
    def characters(bytes, offset, length)
      return [offset, length] if bytes.encoding == Encoding::ASCII_8BIT || bytes.ascii_only?

      [bytes.byteslice(0, offset).length, bytes.byteslice(offset, length).length]
    end

    # The shared body of every format-free door: one native call over the scratch buffers.
    def plain(symbol, text, out_size)
      bytes = utf8(text)
      out, fault = scratch
      rc = Runtime.function(symbol).call(input_ptr(bytes), bytes.bytesize, out, fault)
      verdict(rc, fault, bytes) { yield(out[0, out_size]) }
    end

    # The shared body of the integer/real/decimal doors: plain, plus the format's own
    # packed pointer, passed straight through — no per-call copy of the 32 bytes into
    # scratch.
    def numeric(symbol, text, format, out_size)
      bytes = utf8(text)
      out, fault = scratch
      rc = Runtime.function(symbol).call(input_ptr(bytes), bytes.bytesize, packed_cache[format], out, fault)
      verdict(rc, fault, bytes) { yield(out[0, out_size]) }
    end

    # The shared body of the four doors that take a caller-declared u32 — a precision, an
    # epoch, or a field order — already resolved from its Symbol by the door. These bodies
    # (plain, numeric, declared, and packed_version below) are the whole native crossing:
    # the wasm backend (wasm_runtime.rb) redefines exactly these four in place and nothing
    # above them.
    def declared(symbol, text, code, out_size)
      bytes = utf8(text)
      out, fault = scratch
      rc = Runtime.function(symbol).call(input_ptr(bytes), bytes.bytesize, code, out, fault)
      verdict(rc, fault, bytes) { yield(out[0, out_size]) }
    end

    # The loaded core's version word, major << 16 | minor << 8 | patch, straight from the
    # library's zero-argument hypercast_version export.
    def packed_version
      Runtime.function(:hypercast_version).call
    end

    # Identity-keyed memo (compare_by_identity — a pointer hash, not Data's structural
    # #hash over three Strings and an Integer) of a native 32-byte RawNumFormat per format
    # object, filled once from NumFormat#packed. Formats are reused constants in practice,
    # so the common call finds its pointer in one lookup and packs nothing. The Hash holds
    # the format, so a key can never be a recycled address; the race is benign (idempotent).
    def packed_cache
      @packed_cache ||= Hash.new do |cache, format|
        pointer = Fiddle::Pointer.malloc(32, Fiddle::RUBY_FREE)
        pointer[0, 32] = format.packed
        cache[format] = pointer
      end.compare_by_identity
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
#
# The third backend is WebAssembly (lib/hypercast/wasm_runtime.rb): the same core as a
# wasm32-wasip1 module, run in-process by the `wasmtime` gem, which is deliberately not a
# runtime dependency of this gem — a consumer who wants it installs it. HYPERCAST_WASM=1
# forces it (and fails loudly if wasmtime is missing); otherwise it is only ever chosen when
# there is no native library for this platform at all and wasmtime happens to be available,
# so no supported platform's behavior changes by its existence.
HyperCast::BACKEND =
  if ENV["HYPERCAST_WASM"]
    begin
      require "wasmtime"
    rescue LoadError
      raise LoadError,
            "hypercast: HYPERCAST_WASM=1 needs the wasmtime gem — `gem install wasmtime` (or add it to your Gemfile)"
    end
    require_relative "hypercast/wasm_runtime"
    :wasm
  elsif ENV["HYPERCAST_PURE"]
    :fiddle
  else
    # Two layouts, and both have to work. A released platform gem is a "fat" gem carrying one
    # extension per supported Ruby ABI under lib/hypercast/<minor>/ (see the Rakefile's
    # native:gem task for why an ABI-per-file is unavoidable — Magnus has no `abi3`
    # equivalent). CI's in-job staging and a local `cargo build --release --features ruby`
    # instead drop a single extension flat at lib/. Trying the versioned path first and the
    # flat one second means neither has to know the other exists.
    #
    # A miss on both is not an error: it means this Ruby/platform combination has no
    # precompiled extension, which is precisely what the Fiddle backend is for.
    begin
      require "hypercast/#{RUBY_VERSION[/\d+\.\d+/]}/hypercast_native"
      :native
    rescue LoadError
      begin
        require "hypercast_native"
        :native
      rescue LoadError
        if HyperCast::Runtime.fiddle_library_available?
          :fiddle
        else
          # No shared library for this platform either. wasmtime, if the consumer has it,
          # is the only backend left that can run here; without it, stay on Fiddle so the
          # first call raises its own precise "not found" LoadError rather than a vaguer one
          # from here.
          begin
            require "wasmtime"
            require_relative "hypercast/wasm_runtime"
            :wasm
          rescue LoadError
            :fiddle
          end
        end
      end
    end
  end
