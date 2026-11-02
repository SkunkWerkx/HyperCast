require "date"
require_relative "hypercast/native_platform"
require_relative "hypercast/interop"
require_relative "hypercast/runtime"

# Allocation-lean scalar casts — booleans, numerics, exact decimals, UUIDs, temporals —
# from one Rust core, reached through whichever of two backends this install can load:
# the core linked straight into a Magnus native extension (shipped precompiled in the
# platform gems), or the native libhypercast shared library called through Fiddle (the
# universal gem bundles one per supported platform). HyperCast::BACKEND names the one that
# loaded; the selection logic is at the bottom of this file. Every door returns a verdict: Success or
# Fault (a closed reason plus the offending span), and every checked door — the doors that
# also judge a value by declared Constraints — a Checked, a Fault or a Violation; never an
# exception for bad data. The only exceptions here are caller bugs (a malformed NumFormat, a
# refused Constraints, an undeclared option, text that is not a String), never data, and the
# same exception on every backend.
#
# Consume with Ruby's own pattern matching over the two Data case types:
#
#   case HyperCast.i32("(1,234)", HyperCast::NumFormat::INVARIANT)
#   in HyperCast::Success(value:) then puts "got #{value}"          # -1234
#   in HyperCast::Fault(reason:, offset:) then puts "#{reason} at #{offset}"
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
  VERSION = "1.0.0"

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
  # Past the separators and the flags, every field is one a culture table would fill in,
  # and each is declared here, never looked up: the currency symbol; the separators a
  # currency amount is written with (currency_decimal_sep, currency_group_sep — read in
  # place of the number separators when the symbol is found in the text, each "" keeping
  # its number counterpart); further spellings of the minus and plus signs and of the
  # percent symbol; and the culture's own digits (digit_zero — the ten code points from
  # that zero up, "٠" for the Arabic-Indic digits). A declared symbol is read *beside* its
  # ASCII default, never instead of it — ASCII "-", "+", "%" and 0-9 are always read — so a
  # format that declares nothing ("" everywhere; the default) reads exactly as the
  # invariant culture, and nothing that parsed before stops parsing. The currency symbol is
  # honored only while the CURRENCY flag is set — declared with the flag off, the symbol is
  # a :malformed Fault at the symbol, and the flag with no symbol matches nothing — and the
  # percent symbol while PERCENT is. A separator may be wider than one character (", " —
  # comma, space — is a separator), up to 8 UTF-8 bytes. A malformed format is a caller bug
  # (ArgumentError), never a verdict: an empty or over-long separator, equal separators (or
  # an equal pair in force for currency amounts), a symbol carrying an ASCII digit, a sign
  # or percent symbol carrying ASCII whitespace, a currency symbol longer than 16 bytes or
  # carrying a digit or whitespace, or a digit zero that is ASCII or starts no digit block.
  NumFormat = Data.define(:decimal_sep, :group_sep, :flags, :currency,
                          :currency_decimal_sep, :currency_group_sep,
                          :minus_sign, :plus_sign, :percent, :digit_zero) do
    # Validates every declared field up front — the separators (1 to 8 UTF-8 bytes each,
    # distinct from each other, and so the pair in force for currency amounts), the signs,
    # the percent and currency symbols (no ASCII digit, since a digit is a digit before it
    # is anything else, and no ASCII whitespace, which would collide with the trimming
    # around them) and the digit zero (one non-ASCII character whose ten successors share
    # its UTF-8 prefix, which every Unicode digit block's zero does) — so a malformed
    # format fails loudly as the caller bug it is. Every field is stored transcoded to
    # UTF-8, the encoding the core reads — a separator handed over in another encoding (a
    # no-break space read from a Latin-1 file, say) is the same code point on every backend
    # rather than whatever its bytes happened to spell in the encoding it arrived in.
    def initialize(decimal_sep:, group_sep:, flags:, currency: "", currency_decimal_sep: "",
                   currency_group_sep: "", minus_sign: "", plus_sign: "", percent: "", digit_zero: "")
      limit = self.class::SYMBOL_MAX_BYTES
      decimal_sep = declared("decimal separator", decimal_sep, limit, required: true)
      group_sep = declared("group separator", group_sep, limit, required: true)
      raise ArgumentError, "decimal and group separators must differ; both are #{decimal_sep.inspect}" if
        decimal_sep == group_sep

      currency_decimal_sep = declared("currency decimal separator", currency_decimal_sep, limit)
      currency_group_sep = declared("currency group separator", currency_group_sep, limit)
      money_decimal = currency_decimal_sep.empty? ? decimal_sep : currency_decimal_sep
      money_group = currency_group_sep.empty? ? group_sep : currency_group_sep
      raise ArgumentError, "currency decimal and group separators must differ; both are #{money_decimal.inspect}" if
        money_decimal == money_group

      super(decimal_sep: decimal_sep, group_sep: group_sep, flags: flags,
            currency: declared("currency symbol", currency, self.class::CURRENCY_MAX_BYTES, flush: true),
            currency_decimal_sep: currency_decimal_sep, currency_group_sep: currency_group_sep,
            minus_sign: declared("minus sign", minus_sign, limit, flush: true),
            plus_sign: declared("plus sign", plus_sign, limit, flush: true),
            percent: declared("percent symbol", percent, limit, flush: true),
            digit_zero: digit_zero_of(digit_zero))
    end

    # This format with the fields Ruby's number vocabulary names replaced — the words Rails'
    # number helpers and I18n's number.format use: separator: (the decimal mark),
    # delimiter: (grouping) and unit: (the currency symbol). A nil keyword leaves its
    # field alone, so with none given this is the format itself, not a copy. The result is
    # validated like any NumFormat (declarable, distinct separators; a declarable symbol),
    # and the flags and every other field are untouched: a unit: declared on a format
    # without CURRENCY is a :malformed Fault at the symbol, exactly as declaring it in the
    # constructor would be. Every numeric door takes the same three keywords and applies
    # them through this method; the fields Rails has no word for (the signs, the percent
    # symbol, the digits, the currency separators) are declared in the constructor or
    # through Data#with.
    def override(separator: nil, delimiter: nil, unit: nil)
      return self unless separator || delimiter || unit

      with(**{ decimal_sep: separator, group_sep: delimiter, currency: unit }.compact)
    end

    # A format from a Hash shaped like I18n's number.format (or number.currency.format):
    # :separator, :delimiter, :unit and :negative_format, Symbol or String keys, every other
    # key (precision, format, significant, ...) ignored — so
    # `NumFormat.from_i18n(I18n.t("number.format"))` drops a Rails locale straight in, with
    # no I18n dependency here. A missing or nil key keeps the invariant's "." / "," / no
    # symbol, except where that would repeat the mark the Hash does declare: a separator of
    # "," with no delimiter groups with ".", and a delimiter of "." with no separator takes
    # "," for the decimal mark. An empty delimiter — the locale writes no grouping — turns GROUPING off rather
    # than declaring a separator, since a NumFormat always carries one; the placeholder it
    # gets is whichever of "," and "." the decimal mark is not. A negative_format that writes
    # the sign after the number ("%n-", "%n %u-") turns TRAILING_SIGN on; "-%n", the Rails
    # default, leaves it off. flags defaults to every lenience, the same as INVARIANT.
    #
    # currency_format: takes the locale's number.currency.format Hash beside number.format,
    # for a column that mixes plain numbers and currency amounts in a locale whose two
    # differ (the Swiss German franc, say — "1'234.50" beside "1.234,50"): its separator and
    # delimiter become the currency separators, read in place of the number ones when the
    # unit is found in the text, and its unit the currency symbol. A missing key there
    # declares nothing, keeping the number separator it stands in for, unless that would
    # repeat the currency mark it does declare, in which case the missing one is whichever
    # of "," and "." that mark is not; an explicitly empty
    # delimiter gets the same placeholder rule as above; its negative_format turns
    # TRAILING_SIGN on the same way. Passed alone, number.currency.format reads as the one
    # number format, as before.
    #
    # The rest of the format is what the Hash cannot offer: Rails carries no minus or plus
    # sign, no percent symbol and no digits, so those stay undeclared here — the core reads
    # ASCII "-", "+", "%" and 0-9 — and a locale that writes negatives with another sign
    # (U+2212 for Estonian and Basque, for instance) declares it on the result with
    # Data#with (minus_sign: "−"). Every lenience is on, radix prefixes and percent included;
    # narrow flags: for anything stricter.
    def self.from_i18n(number_format, currency_format: nil, flags: ALL_STYLES)
      entries = i18n_entries(number_format)
      separator = entries[:separator] || (entries[:delimiter] == "." ? "," : ".")
      delimiter = entries[:delimiter]
      if delimiter.nil? || delimiter == ""
        flags &= ~GROUPING if delimiter == ""
        delimiter = separator == "," ? "." : ","
      end
      flags |= TRAILING_SIGN if trailing_sign?(entries[:negative_format])
      currency = entries[:unit] || ""
      currency_separator = currency_delimiter = ""
      if currency_format
        money = i18n_entries(currency_format)
        currency = money[:unit] || currency
        currency_separator = money[:separator] || ""
        currency_delimiter = money[:delimiter] || ""
        currency_delimiter = (money[:separator] || separator) == "," ? "." : "," if money[:delimiter] == ""
        if currency_separator.empty? && currency_delimiter == separator
          currency_separator = currency_delimiter == "," ? "." : ","
        elsif currency_delimiter.empty? && currency_separator == delimiter
          currency_delimiter = currency_separator == "," ? "." : ","
        end
        flags |= TRAILING_SIGN if trailing_sign?(money[:negative_format])
      end
      new(decimal_sep: separator, group_sep: delimiter, flags: flags, currency: currency,
          currency_decimal_sep: currency_separator, currency_group_sep: currency_delimiter)
    end

    # The I18n Hash's entries under Symbol keys, whatever keys it arrived with.
    def self.i18n_entries(number_format)
      raise ArgumentError, "expected a Hash-like number format; got #{number_format.inspect}" unless
        number_format.respond_to?(:to_h)

      number_format.to_h.transform_keys { |key| key.respond_to?(:to_sym) ? key.to_sym : key }
    end
    private_class_method :i18n_entries

    # Whether an I18n negative_format ("-%n", "%n-", "-%u%n", "%n %u-") writes the sign
    # after the number: a "-" to the right of the "%n" placeholder.
    def self.trailing_sign?(negative_format)
      return false unless negative_format.is_a?(String)

      number = negative_format.index("%n")
      sign = negative_format.rindex("-")
      !number.nil? && !sign.nil? && sign > number
    end
    private_class_method :trailing_sign?

    # The 92-byte little-endian form of the native ABI's size-tagged NumFormat struct, the
    # layout the core reads in place: the size word (92), the flags, the digit zero's code
    # point (0 for none), then each symbol as its UTF-8 bytes zero-padded to its field
    # followed by its byte length — eight bytes and a length for each of the two number
    # separators, the two currency separators, the two signs and the percent symbol, in
    # that order; sixteen and a length for the currency symbol. A length of 0 declares
    # nothing.
    def packed
      head = [self.class::PACKED_BYTES, flags, digit_zero.empty? ? 0 : digit_zero.ord].pack("L<L<L<")
      symbols = [decimal_sep, group_sep, currency_decimal_sep, currency_group_sep, minus_sign, plus_sign, percent]
      head + symbols.map { |symbol| [symbol, symbol.bytesize].pack("a8C") }.join +
        [currency, currency.bytesize].pack("a16C")
    end

    private

    # The declared text of one field as the UTF-8 the core reads: a String of 1 to `limit`
    # UTF-8 bytes with no ASCII digit — nor, for a symbol that sits flush against the
    # digits (the signs, the percent and currency symbols), any ASCII whitespace — or ""
    # where the field may be left undeclared. Anything else is the caller bug it names.
    def declared(field, text, limit, flush: false, required: false)
      raise ArgumentError, "#{field} must be a String; got #{text.inspect}" unless text.is_a?(String)

      symbol = text.encode(Encoding::UTF_8)
      if symbol.empty?
        raise ArgumentError, "#{field} must not be empty" if required

        return symbol
      end
      raise ArgumentError, "#{field} #{text.inspect} exceeds #{limit} UTF-8 bytes" if symbol.bytesize > limit

      if flush
        raise ArgumentError, "#{field} #{text.inspect} must not contain an ASCII digit or whitespace" if
          symbol.match?(/[0-9\t\n\f\r ]/)
      elsif symbol.match?(/[0-9]/)
        raise ArgumentError, "#{field} #{text.inspect} must not contain an ASCII digit"
      end
      symbol
    end

    # The declared digit zero as UTF-8: "" for none, else exactly one character that is not
    # ASCII and whose code point modulo 64 is at most 54 — true of the zero of every decimal
    # digit block Unicode defines, and the shape the core recognises a digit by its last
    # byte from.
    def digit_zero_of(text)
      raise ArgumentError, "digit zero must be a String; got #{text.inspect}" unless text.is_a?(String)

      zero = text.encode(Encoding::UTF_8)
      return zero if zero.empty?

      raise ArgumentError, "digit zero #{text.inspect} must be one non-ASCII character that starts a digit block" unless
        zero.length == 1 && zero.ord >= 0x80 && zero.ord % 64 <= 54

      zero
    end
  end

  # The widest currency symbol the native ABI carries inline, in UTF-8 bytes. Assigned out
  # here rather than inside the Data.define block above: a constant written in that block
  # is scoped lexically, and landed on HyperCast instead of on NumFormat.
  NumFormat::CURRENCY_MAX_BYTES = 16

  # The widest separator, sign or percent symbol the native ABI carries inline, in UTF-8
  # bytes: enough for every separator a real culture declares (one or two characters).
  NumFormat::SYMBOL_MAX_BYTES = 8

  # The size of NumFormat#packed — the native ABI's size-tagged NumFormat layout — in bytes.
  NumFormat::PACKED_BYTES = 92

  # Permit the group separator between digits (sizes not validated — between digits is the rule).
  GROUPING = 1
  # Permit accounting parentheses as negation: (1,234) is -1234.
  PARENTHESES = 1 << 1
  # Permit exponent notation. Integer doors reject a negative exponent.
  EXPONENT = 1 << 2
  # Permit 0x/&H/0b two's-complement radix prefixes (0xFF is -1 for an i8).
  RADIX_PREFIXES = 1 << 3
  # Permit a trailing % (or the format's declared percent symbol), dividing by 100. Real
  # and decimal doors only.
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
  # Permit the sign after the digits ("5-", "5 -", "5 €-", "5- €"), the way a locale whose
  # negative_format is "%n-" writes it: ASCII - and + or the format's declared signs, once,
  # with optional ASCII whitespace before it, and never beside a leading sign or inside
  # accounting parentheses. Not part of ALL_STYLES — NumFormat.from_i18n sets it from the
  # locale's own negative_format rather than assuming it.
  TRAILING_SIGN = 1 << 7
  # Every lenience on — every one there was before TRAILING_SIGN, which is declared from a
  # locale's pattern rather than assumed (SEPARATOR_DETECT is a separator policy, not a
  # lenience, and is deliberately not included either).
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

  # The doors a Constraints can be bound to, mapped to the native core's door codes: the
  # integer, real and decimal doors, and :text.
  CHECKED_DOORS = {
    i8: 2, i16: 3, i32: 4, i64: 5, u8: 6, u16: 7, u32: 8, u64: 9, f32: 10, f64: 11, text: 18, decimal: 19
  }.freeze

  # What a value that is not a multiple of a declared multiple_of becomes, mapped to the
  # native core's rounding codes: :strict (a :multiple_of Violation, the default), :midpoint
  # (the nearest multiple, ties to even), :floor (toward negative infinity) and :ceiling
  # (toward positive infinity). An unknown rounding is a caller bug (KeyError).
  ROUNDINGS = { strict: 0, midpoint: 1, floor: 2, ceiling: 3 }.freeze

  # The success case of a checked door: the value (rounded to multiple_of when the declaration
  # rounds; at the :text door with an enum, the very String the declaration was given),
  # adjusted (true only when rounding changed the value) and, at the :text door with an enum,
  # the index of the entry matched — nil otherwise, an integer enum included, where the value
  # is the answer.
  Checked = Data.define(:value, :adjusted, :enum_index)

  # A value that parsed and broke a declared rule: the Keyword broken and the value's span, in
  # the units a Fault's span is in (characters for text, bytes for a binary String). A number
  # the caller already held has no text, and an empty span.
  Violation = Data.define(:keyword, :offset, :length)

  # The rule a Violation broke: the native core's code, named the OpenAPI way by #name
  # ("exclusiveMinimum"). Open to additions — format and pattern may join in a minor release —
  # so a case over keywords needs an else branch; a code this gem does not name yet is kept,
  # its name "keyword N". The constants (Keyword::MINIMUM ... Keyword::ENUM) compare equal to
  # every keyword of their code, so `in Violation(keyword: Keyword::MINIMUM)` matches.
  Keyword = Data.define(:code) do
    # The OpenAPI spelling: "minimum", "exclusiveMinimum", ... "enum"; "keyword N" for a code
    # this gem does not name.
    def name
      self.class::NAMES.fetch(code) { "keyword #{code}" }
    end

    alias_method :to_s, :name

    # The constant that names this keyword (HyperCast::Keyword::MINIMUM), or the Data form
    # for a code this gem does not name.
    def inspect
      constant = self.class::CONSTANTS[code]
      constant ? "HyperCast::Keyword::#{constant}" : super
    end
  end

  # The OpenAPI spelling of each keyword the core names, by code.
  Keyword::NAMES = {
    1 => "minimum", 2 => "exclusiveMinimum", 3 => "maximum", 4 => "exclusiveMaximum", 5 => "multipleOf",
    6 => "minLength", 7 => "maxLength", 8 => "enum"
  }.freeze

  # The constant each named keyword is published as, by code.
  Keyword::CONSTANTS = {
    1 => "MINIMUM", 2 => "EXCLUSIVE_MINIMUM", 3 => "MAXIMUM", 4 => "EXCLUSIVE_MAXIMUM", 5 => "MULTIPLE_OF",
    6 => "MIN_LENGTH", 7 => "MAX_LENGTH", 8 => "ENUM"
  }.freeze

  # `minimum`, broken by a value below it.
  Keyword::MINIMUM = Keyword.new(code: 1)
  # `exclusiveMinimum`, broken by a value at or below it.
  Keyword::EXCLUSIVE_MINIMUM = Keyword.new(code: 2)
  # `maximum`, broken by a value above it.
  Keyword::MAXIMUM = Keyword.new(code: 3)
  # `exclusiveMaximum`, broken by a value at or above it.
  Keyword::EXCLUSIVE_MAXIMUM = Keyword.new(code: 4)
  # `multipleOf`, broken by a value that is not a multiple of it under :strict rounding.
  Keyword::MULTIPLE_OF = Keyword.new(code: 5)
  # `minLength`, broken by text of fewer code points.
  Keyword::MIN_LENGTH = Keyword.new(code: 6)
  # `maxLength`, broken by text of more code points.
  Keyword::MAX_LENGTH = Keyword.new(code: 7)
  # `enum`, broken by a value that is none of the entries.
  Keyword::ENUM = Keyword.new(code: 8)

  # Validation constraints — the OpenAPI Schema Object's keywords — bound to one door and
  # judged once, by the native core, when built: a declaration that cannot mean anything at
  # its door is an ArgumentError here, its message the rule it breaks ("Constraints refused
  # (code 4): the lower bound is not below the upper bound."), and every checked cast after
  # that judges only data. The door is required: no declaration ever silently means "no
  # rules", and `Constraints.new(door: :f64)` is an explicit, empty one.
  #
  # The integer doors take minimum, exclusive_minimum, maximum, exclusive_maximum,
  # multiple_of with a rounding (a ROUNDINGS Symbol), and enum; :f32, :f64 and :decimal the
  # same but enum; :text min_length, max_length (in code points), trim (of Unicode
  # White_Space) and enum (matched under simple case folding). Every keyword is accepted at
  # every door, and the core alone refuses one its door does not take.
  #
  # A bound, step or integer enum entry is an Integer, a BigDecimal, a String in invariant
  # notation ("0.000001", "-90", "1e-6") or this gem's own Decimal, and crosses the core's own
  # conversion (HyperCast.declared_decimal): never a Float, since 0.1 has no exact binary form
  # and a Float could only declare a neighbour of the value meant. Bounds are kept as the exact
  # Decimal declared. A text enum entry is a String, kept as the very object given when it is
  # frozen and as a frozen copy when it is not, so a match delivers what the core judged. An
  # enum left nil declares none, while `enum: []` declares one no value could match and is
  # refused (code 12), as is a text entry no input can match (code 13: an empty one, or under
  # trim one with leading or trailing white space).
  #
  #   latitude = HyperCast::Constraints.new(door: :f64, minimum: "-90", maximum: "90")
  #   HyperCast.checked("91", HyperCast::NumFormat::INVARIANT, latitude)
  #   # => #<data HyperCast::Violation keyword=HyperCast::Keyword::MAXIMUM, offset=0, length=2>
  Constraints = Data.define(:door, :minimum, :exclusive_minimum, :maximum, :exclusive_maximum, :multiple_of,
                            :rounding, :min_length, :max_length, :trim, :enum) do
    # Validates every keyword's type, converts the bounds, steps and integer entries through
    # the core, and has the core judge the declaration; a refusal is an ArgumentError worded
    # from its code. A wrong type is an ArgumentError, an unknown door or rounding a KeyError.
    def initialize(door:, minimum: nil, exclusive_minimum: nil, maximum: nil, exclusive_maximum: nil,
                   multiple_of: nil, rounding: :strict, min_length: nil, max_length: nil, trim: false, enum: nil)
      keywords = { door: door, minimum: minimum, exclusive_minimum: exclusive_minimum, maximum: maximum,
                   exclusive_maximum: exclusive_maximum, multiple_of: multiple_of, rounding: rounding,
                   min_length: min_length, max_length: max_length, trim: trim, enum: enum }
      code = HyperCast.constraints_check(**keywords)
      raise ArgumentError, self.class.send(:refusal, code) unless code.zero?

      super(**self.class.send(:members_of, **keywords))
    end

    # The 72 bytes this declaration crosses a C ABI as, in the native byte order the core
    # reads in place: the size (72), the flags, the rounding code, min_length, max_length and
    # a reserved zero as u32s, then the lower bound, the upper bound and multiple_of as 16-byte
    # decimals (the low 64 and high 32 bits of the magnitude, the scale, the sign, two zero
    # bytes) — for a gem that judges values across a C ABI of its own, with #packed_enum
    # beside it.
    def packed
      self.class.send(:pack_raw, to_h)
    end

    # The enum list as the core reads it beside #packed, "" for none (read only when the
    # flags in #packed carry HAS_ENUM, 256, set whenever an enum is declared): at an integer
    # door 16-byte decimals in native byte order, at :text each entry a little-endian u32 byte
    # length followed by its UTF-8.
    def packed_enum
      self.class.send(:pack_enum, to_h)
    end

    # The message a refused declaration raises with, one per refusal code; a code this gem
    # does not name is worded as every binding words it.
    def self.refusal(code)
      "Constraints refused (code #{code}): #{self::REFUSALS.fetch(code, 'a rule this binding does not name')}."
    end

    # The declaration as both backends hand it to the core: the door code, #packed and
    # #packed_enum. Judged again here, once per object (each backend remembers the answer),
    # because a Constraints can exist without Constraints.new having judged it: one never
    # built (Data's allocate) has no door, and one restored by Marshal from a payload in the
    # Struct form sets its members directly. Either is the caller's bug, raised as
    # Constraints.new raises it, never a declaration of no rules.
    def declaration
      raise ArgumentError, "this HyperCast::Constraints was never built: declare it with Constraints.new(door: ...)" if
        door.nil?

      code = HyperCast.constraints_check(**to_h)
      raise ArgumentError, self.class.send(:refusal, code) unless code.zero?

      [CHECKED_DOORS.fetch(door), packed, packed_enum]
    end
    private :declaration

    # Marshal's form of a declaration: its members, so loading one builds it through
    # #initialize, judged as Constraints.new judges it.
    def marshal_dump
      to_h
    end

    # Restores a declaration from #marshal_dump's members through #initialize, so a payload
    # that declares something the core refuses raises its refusal.
    def marshal_load(members)
      initialize(**members)
    end

    # The declaration HyperCast.constraints_check judges: its door code, packed layout and
    # packed enum, or the code 8 the conversion of a bound, step or entry was refused with.
    def self.layout(**keywords)
      members = members_of(**keywords)
      return members if members.is_a?(Integer)

      [CHECKED_DOORS.fetch(members[:door]), pack_raw(members), pack_enum(members)]
    end

    # Every keyword validated and converted — a declaration's members — or the code 8 a
    # conversion was refused with.
    def self.members_of(door:, minimum: nil, exclusive_minimum: nil, maximum: nil, exclusive_maximum: nil,
                        multiple_of: nil, rounding: :strict, min_length: nil, max_length: nil, trim: false, enum: nil)
      CHECKED_DOORS.fetch(door)
      ROUNDINGS.fetch(rounding)
      raise ArgumentError, "trim must be true or false; got #{trim.inspect}" unless [true, false].include?(trim)

      bounds = { minimum: minimum, exclusive_minimum: exclusive_minimum, maximum: maximum,
                 exclusive_maximum: exclusive_maximum, multiple_of: multiple_of }
      bounds.each do |name, value|
        next if value.nil?

        bounds[name] = exact(name, value)
        return bounds[name] if bounds[name].is_a?(Integer)
      end
      entries = entries_of(door, enum)
      return entries if entries.is_a?(Integer)

      { door: door, **bounds, rounding: rounding, min_length: length("min_length", min_length),
        max_length: length("max_length", max_length), trim: trim, enum: entries }
    end

    # A bound, step or integer enum entry as the core converts it: an Integer, a BigDecimal,
    # a String or a Decimal, each through its text and HyperCast.declared_decimal — the exact
    # Decimal, or the code 8 it was refused with. Never a Float, nor true or false.
    def self.exact(name, value)
      text =
        case value
        when Integer, String, Decimal then value.to_s
        else
          raise ArgumentError, "#{name} must be an Integer, a BigDecimal, a String or a HyperCast::Decimal; got " \
                               "#{value.inspect}#{hint(value)}" unless defined?(BigDecimal) && value.is_a?(BigDecimal)

          value.to_s("F")
        end
      begin
        HyperCast.declared_decimal(text)
      rescue ArgumentError
        8
      end
    end

    # Why a Float is refused, beside the refusal.
    def self.hint(value)
      return "" unless value.is_a?(Float)

      " (a Float holds most decimals only approximately: write it as a String or a BigDecimal)"
    end

    # A declared length: nil, or an Integer in the 32 unsigned bits the core counts code
    # points in (never true or false, nor a Float).
    def self.length(name, value)
      return nil if value.nil?
      raise ArgumentError, "#{name} must be an Integer from 0 to #{0xFFFF_FFFF}; got #{value.inspect}" unless
        value.is_a?(Integer) && value.between?(0, 0xFFFF_FFFF)

      value
    end

    # The enum entries, frozen, as a declaration keeps them: at :text the caller's own
    # Strings, elsewhere each entry's exact Decimal (or the code 8 one was refused with); nil
    # for none. An empty list is declared, for the core to refuse (code 12).
    def self.entries_of(door, enum)
      return nil if enum.nil?
      raise ArgumentError, "enum must be an Array of entries; got #{enum.inspect}" unless
        enum.respond_to?(:to_a) && !enum.is_a?(String)

      entries = enum.to_a
      if door == :text
        entries = entries.each_with_index.map do |entry, index|
          raise ArgumentError, "enum entries at :text must be Strings; entry #{index} is #{entry.inspect}" unless
            entry.is_a?(String)

          # A Hash key's rule: a frozen String is kept as itself, a mutable one as a frozen
          # copy, so what a match delivers is always what the core judged.
          entry.frozen? ? entry : entry.dup.freeze
        end
      else
        entries = entries.map do |entry|
          decimal = exact("enum entry", entry)
          return decimal if decimal.is_a?(Integer)

          decimal
        end
      end
      entries.freeze
    end

    # A declaration's members as the 72-byte native layout #packed describes.
    def self.pack_raw(members)
      flags = self::FLAGS.sum { |member, flag| members[member].nil? ? 0 : flag }
      flags |= 128 if members[:trim]
      decimals = [members[:minimum] || members[:exclusive_minimum], members[:maximum] || members[:exclusive_maximum],
                  members[:multiple_of]]
      [72, flags, ROUNDINGS.fetch(members[:rounding]), members[:min_length] || 0, members[:max_length] || 0, 0]
        .pack("L6") + decimals.map { |decimal| pack_decimal(decimal) }.join
    end

    # One Decimal in the core's native 16-byte layout; zero for nil.
    def self.pack_decimal(decimal)
      return [0, 0, 0, 0].pack("QLCCx2") if decimal.nil?

      magnitude = decimal.magnitude
      [magnitude & 0xFFFF_FFFF_FFFF_FFFF, magnitude >> 64, decimal.scale, decimal.negative ? 1 : 0].pack("QLCCx2")
    end

    # A declaration's enum as #packed_enum describes it, a binary String.
    def self.pack_enum(members)
      entries = members[:enum] or return "".b
      return entries.map { |entry| pack_decimal(entry) }.join.b unless members[:door] == :text

      entries.map do |entry|
        text = Interop::BYTE_COMPATIBLE.include?(entry.encoding) ? entry : entry.encode(Encoding::UTF_8)
        [text.bytesize].pack("L<") + text.b
      end.join.b
    end

    private_class_method :refusal, :layout, :members_of, :exact, :hint, :length, :entries_of, :pack_raw, :pack_decimal,
                         :pack_enum
  end

  # The flag each declared member sets in the packed layout, HAS_ENUM (256) by an enum even
  # with no entries; TRIM (128) is set by trim.
  Constraints::FLAGS = { minimum: 1, exclusive_minimum: 2, maximum: 4, exclusive_maximum: 8, multiple_of: 16,
                         min_length: 32, max_length: 64, enum: 256 }.freeze

  # The words a refused declaration's message gives, one per code: the native core's own.
  Constraints::REFUSALS = {
    1 => "a constraint the door does not take",
    2 => "minimum together with exclusiveMinimum",
    3 => "maximum together with exclusiveMaximum",
    4 => "the lower bound is not below the upper bound",
    5 => "multipleOf is not positive",
    6 => "a rounding without multipleOf",
    7 => "minLength is above maxLength",
    8 => "a bound, step or enum entry the door cannot hold",
    9 => "an enum entry that is not UTF-8",
    10 => "a duplicate enum entry",
    11 => "a malformed constraints layout",
    12 => "an enum with no entries",
    13 => "a text enum entry no input can match"
  }.freeze

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
    # and never raises: a missing shared library, an unsupported platform, or an older core
    # without the version export all answer false. The doors themselves keep their own
    # behavior — the first call on an unavailable backend raises its precise LoadError —
    # this only answers the question quietly.
    def available?
      return @available unless @available.nil?

      @available = begin
        native_version.is_a?(String)
      rescue LoadError, StandardError
        false
      end
    end
  end

  # The doors, the Fiddle way. They live in a module HyperCast extends rather than on its
  # singleton class, so the Magnus extension's define_singleton_method puts its definition on
  # the singleton class in front of them: an override through the ancestor chain, not a
  # redefinition, and `ruby -w` has nothing to say about it (1.0.0 defined them on the
  # singleton class and printed "method redefined" twice per door on every require under -w).
  # Lookup is unchanged either way: the singleton class first, then this module, so with the
  # extension loaded no call ever reaches these. Their helpers stay private on HyperCast.
  module FiddleBackend
    # Casts boolean text: true/false plus the conventions untrusted sources actually send
    # (t/f, yes/no, y/n, 1/0, on/off, enabled/disabled, active/inactive,
    # checked/unchecked, in/out), ASCII case-insensitive.
    def bool(text)
      plain(:cast_bool, text, 1) { |out| Interop.decode(:bool, out) }
    end

    # Casts char text to a one-character UTF-8 String: an input of exactly one character is
    # that character verbatim (" " is a space), otherwise one declared code point — 65,
    # U+0041, 0x41, &H41, &#65; or &#x41; — surrounded by optional ASCII whitespace. A code
    # point past U+10FFFF, or a surrogate, is :out_of_range.
    def char(text)
      plain(:cast_char, text, 4) { |out| Interop.decode(:char, out) }
    end

    %i[i8 i16 i32 i64 u8 u16 u32 u64].each do |door|
      unpack = Interop::SCALARS.fetch(door)
      size = Interop::VALUE_BYTES.fetch(door)
      # Resolved once here, not inside the method: interpolating a Symbol per call built a
      # String and interned it on every integer cast.
      symbol = :"cast_#{door}"
      # Integer doors: the target type's own range, declared grouping, accounting parens,
      # non-negative exponent, and 0x/&H/0b two's-complement radix prefixes. Ruby Integer
      # is unbounded, so u64 comes back as the true unsigned value. separator:, delimiter:
      # and unit: override those fields of format for this call (NumFormat#override).
      define_method(door) do |text, format, separator: nil, delimiter: nil, unit: nil|
        format = format.override(separator: separator, delimiter: delimiter, unit: unit) if
          separator || delimiter || unit
        numeric(symbol, text, format, size) { |out| out.unpack1(unpack) }
      end
    end

    # Casts real text to an IEEE single (widened losslessly on the way out): finite values
    # only, declared separators and grouping, parens, exponent, and trailing percent.
    # separator:, delimiter: and unit: override those fields of format for this call
    # (NumFormat#override), on this door and every other numeric one.
    def f32(text, format, separator: nil, delimiter: nil, unit: nil)
      format = format.override(separator: separator, delimiter: delimiter, unit: unit) if
        separator || delimiter || unit
      numeric(:cast_f32, text, format, 4) { |out| Interop.decode(:f32, out) }
    end

    # Casts real text to an IEEE double. Notation rules as f32.
    def f64(text, format, separator: nil, delimiter: nil, unit: nil)
      format = format.override(separator: separator, delimiter: delimiter, unit: unit) if
        separator || delimiter || unit
      numeric(:cast_f64, text, format, 8) { |out| Interop.decode(:f64, out) }
    end

    # Casts decimal text to an exact Decimal — the real doors' grammar (declared separators
    # and grouping, parens, exponent, trailing percent, declared currency), but no float is
    # ever formed: "0.1" is one tenth, "50%" is exactly 0.5 (magnitude 5, scale 1), and
    # "1.10" is canonical magnitude 11, scale 1 — exact trailing zeros are always trimmed.
    # A magnitude past 2**96 - 1, or more fractional precision than 28 places can hold, is
    # an :out_of_range Fault — the door never rounds.
    def decimal(text, format, separator: nil, delimiter: nil, unit: nil)
      format = format.override(separator: separator, delimiter: delimiter, unit: unit) if
        separator || delimiter || unit
      numeric(:cast_decimal, text, format, 16) { |out| Interop.decode(:decimal, out) }
    end

    # Casts UUID text — all five .NET Guid formats (D/N/B/P/X) plus urn:uuid:/GUID:/UUID:
    # prefixes — to Ruby's UUID lingua franca: the lowercase hyphenated String (the same
    # shape SecureRandom.uuid returns).
    def uuid(text)
      plain(:cast_uuid, text, 16) { |out| Interop.decode(:uuid, out) }
    end

    # Casts an RFC 3339 instant — zone mandatory — to a UTC Time at full nanosecond
    # fidelity across the whole 0001-9999 window.
    def timestamp(text)
      plain(:cast_timestamp, text, 16) { |out| Interop.decode(:timestamp, out) }
    end

    # Casts an integer Unix-epoch value under a caller-declared unit Symbol
    # (:seconds/:milliseconds/:microseconds/:nanoseconds) to a UTC Time. An unknown unit
    # is a caller bug (KeyError), never a verdict.
    def unix(text, precision)
      declared(:cast_unix, text, UNIX_PRECISIONS.fetch(precision), 16) { |out| Interop.decode(:unix, out) }
    end

    # Casts an Excel date serial under a caller-declared epoch Symbol (:y1900/:y1904) to a
    # UTC Time. The whole part counts days from the system's own day zero and the fraction
    # is the time of day, so "45292.75" is 2024-01-01T18:00:00Z; a cell carries no zone and
    # none is invented.
    #
    # The 1900 system contains a day that never existed: serial 60 is 1900-02-29, kept
    # deliberately because Lotus 1-2-3 wrongly treated 1900 as a leap year and Excel copied
    # the bug for file compatibility. It is :out_of_range here — the same verdict .date gives
    # the text "1900-02-29" — so every serial above it is shifted one day against a naive
    # count. An unknown epoch is a caller bug (KeyError), never a verdict.
    def excel_serial(text, epoch)
      declared(:cast_excel_serial, text, EXCEL_EPOCHS.fetch(epoch), 16) { |out| Interop.decode(:excel_serial, out) }
    end

    # Casts a calendar date to a Date. With no order declared: the strict ISO 8601
    # yyyy-MM-dd form only. With a declared order Symbol (:year_month_day,
    # :month_day_year, :day_month_year), also the separated forms — "1/7/2026" is
    # January 7th or July 1st only because the caller said which; an unknown order is a
    # caller bug (KeyError), never a verdict.
    def date(text, order = nil)
      if order.nil?
        plain(:cast_date, text, 4) { |out| Interop.decode(:date, out) }
      else
        declared(:cast_date_ordered, text, DATE_ORDERS.fetch(order), 4) { |out| Interop.decode(:date_ordered, out) }
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
    #
    # A 12-hour clock's AM/PM is read, before or after the clock ("3:04 PM", "PM 3:04"),
    # ASCII case-insensitively; am: and pm: declare a culture's own designators beside
    # them — "vorm."/"nachm.", "오전"/"오후", "a. m."/"p. m." — matched exactly, after the
    # clock ("3:04 nachm.") or ahead of it ("오후 3:04"), with or without a space between.
    # Declared, never looked up, like a NumFormat's symbols; nil (the default) declares
    # nothing, so a call without them is exactly the call it was. A designator is 1 to 16
    # UTF-8 bytes with no ASCII digit and no ASCII whitespace at either end (a space inside
    # is fine); anything else is a caller bug (ArgumentError). The designators are read,
    # validated and packed per call: a call that declares them costs about twice the plain
    # call on the Magnus backend (the keyword scan itself — nil keywords pay the same) and
    # about half again here on Fiddle.
    def datetime(text, order, am: nil, pm: nil)
      periods = packed_day_periods(am, pm) if am || pm
      civil(:cast_datetime, text, DATE_ORDERS.fetch(order), periods, 16) { |out| Interop.decode(:datetime, out) }
    end

    # Casts an ISO 24-hour time-of-day to an exact Integer of nanoseconds since midnight
    # (Ruby has no time-of-day type; the integer keeps every digit).
    def time(text)
      plain(:cast_time, text, 8) { |out| Interop.decode(:time, out) }
    end

    # Casts a duration (ISO 8601 fixed components, invariant colon form, or protobuf JSON
    # seconds) to exact Rational seconds — full fidelity across the core's ±10,000-year
    # window, no wrapping and no truncation.
    def duration(text)
      plain(:cast_duration, text, 16) { |out| Interop.decode(:duration, out) }
    end

    # Reads a number a caller already holds — an Integer, Float or Rational, the way Ruby's
    # own Float conversion takes it; anything else, a String included, is a TypeError — as
    # the exact Decimal it names: the shortest decimal that rounds back to the Float, the
    # digits a spreadsheet writes for it. 0.1 is magnitude 1, scale 1, not the binary
    # fraction nearest it; 0.1 + 0.2 is 0.30000000000000004. NaN is :malformed; an infinity,
    # a magnitude past 2**96 - 1 or more than 28 places is :out_of_range. A typed door's
    # Fault has no span: offset and length are 0.
    def decimal_from_float(value)
      typed(:cast_decimal_from_f64, float(value), 16) { |out| Interop.decode(:decimal, out) }
    end

    # Reads an Excel serial number under a declared epoch Symbol (:y1900/:y1904) as the
    # zone-less DateTime it names — the twin of excel_serial for a number a workbook reader
    # already holds, on the same carrier datetime uses. The 1900 system's phantom serial 60
    # is :out_of_range; a negative, NaN or infinite serial is :malformed.
    def excel_serial_from_float(value, epoch)
      number = float(value)
      typed(:cast_excel_serial_from_f64, number, 16, EXCEL_EPOCHS.fetch(epoch)) { |out| Interop.decode(:datetime, out) }
    end

    # Reads the fraction of an Excel serial number as an exact Integer of nanoseconds since
    # midnight, as time does; 0.75 and 45292.75 are both 18:00.
    def excel_time(value)
      typed(:cast_excel_time, float(value), 8) { |out| Interop.decode(:time, out) }
    end

    # Reads a number of days as exact Rational seconds, as duration does: 1.5 is 129600.
    def excel_duration(value)
      typed(:cast_excel_duration, float(value), 16) { |out| Interop.decode(:duration, out) }
    end

    # Casts text as text: the input itself, as a String, once it is valid UTF-8 — a binary
    # String's bytes must be, and a UTF-8 String carrying invalid bytes is :malformed at the
    # first. Empty input is :empty; nothing is trimmed. The value is the String passed when it
    # is UTF-8 or US-ASCII, else a UTF-8 copy.
    #
    # With Constraints for :text it is judged by them, a Checked, a Fault or a Violation:
    # trimmed of Unicode White_Space when they say so (trimmed to nothing is :empty), lengths
    # counted in code points, and an enum matched under simple case folding, a match
    # delivering the very String declared and its index. Constraints for another door are an
    # ArgumentError (they go to checked).
    def text(text, constraints = nil)
      bytes = utf8(text)
      if constraints.nil?
        out, fault = scratch
        rc = Runtime.function(:cast_text).call(input_ptr(bytes), bytes.bytesize, out, fault)
        return verdict(rc, fault, bytes) { text_value(bytes, *out[0, 8].unpack("LL")) }
      end
      _door, raw, entries = constrained(constraints, :text)
      rc = checked_call(:cast_checked, CHECKED_DOORS.fetch(:text), 0, nil, raw, input_ptr(entries), entries.bytesize,
                        input_ptr(bytes), bytes.bytesize)
      checked_verdict(rc, bytes) do |out, check|
        offset, length = out[0, 8].unpack("LL")
        index = check[8, 4].unpack1("L")
        index == 0xFFFF_FFFF ? text_value(bytes, offset, length) : constraints.enum.fetch(index)
      end
    end

    # Casts numeric text through the door its Constraints are bound to, under the declared
    # format, then judges the value by them: a Checked, or the door's own Fault, or a
    # Violation of the first rule it breaks. The value is judged exactly, on the decimal the
    # door read, never on a float. Spans are in the units a Fault's are. separator:,
    # delimiter: and unit: override those fields of format for this call
    # (NumFormat#override). Constraints for :text are an ArgumentError (they go to text).
    def checked(text, format, constraints, separator: nil, delimiter: nil, unit: nil)
      format = format.override(separator: separator, delimiter: delimiter, unit: unit) if
        separator || delimiter || unit
      bytes = utf8(text)
      symbol, raw, entries = constrained(constraints, :checked)
      rc = checked_call(:cast_checked, CHECKED_DOORS.fetch(symbol), 0, packed_cache[format], raw,
                        input_ptr(entries), entries.bytesize, input_ptr(bytes), bytes.bytesize)
      checked_verdict(rc, bytes) { |out| Interop.decode(symbol, out[0, Interop::VALUE_BYTES.fetch(symbol)]) }
    end

    # Reads a number the caller already holds — an Integer, Float or Rational, taken as
    # decimal_from_float takes it — through the door its Constraints are bound to, then judges
    # it by them on the shortest decimal that names it: 0.1 + 0.2 is 0.30000000000000004,
    # which breaks multiple_of: "0.1" under :strict and rounds to 0.3 under :midpoint. NaN is
    # :malformed and an infinity :out_of_range; every fault and violation has an empty span.
    def checked_from_float(value, constraints)
      number = float(value)
      symbol, raw, entries = constrained(constraints, :checked_from_float)
      rc = checked_call(:cast_checked_from_f64, CHECKED_DOORS.fetch(symbol), number, raw,
                        input_ptr(entries), entries.bytesize)
      checked_verdict(rc, nil) { |out| Interop.decode(symbol, out[0, Interop::VALUE_BYTES.fetch(symbol)]) }
    end

    # Judges a declaration without building it, the way Constraints.new does when it is
    # built, taking the same keywords: 0 when every rule holds, otherwise the code of the first
    # rule it breaks (Constraints::REFUSALS words each). A wrong argument is still raised, as
    # Constraints.new raises it.
    def constraints_check(**keywords)
      layout = Constraints.send(:layout, **keywords)
      return layout if layout.is_a?(Integer)

      door, raw, entries = layout
      pointer, = checked_scratch
      pointer[0, raw.bytesize] = raw
      Runtime.function(:cast_constraints_check).call(door, 0, pointer, input_ptr(entries), entries.bytesize)
    end

    # Converts a bound, step or integer enum entry written in invariant notation — an
    # optional sign, digits with an optional "." and fraction, an optional exponent:
    # "0.000001", "-90", "1e-6" — to the exact Decimal the core declares it as, trailing zeros
    # dropped: the conversion every bound a Constraints is given crosses. A value past 96 bits
    # or 28 places, or text in any other notation, is the ArgumentError a refused declaration
    # raises (code 8).
    def declared_decimal(text)
      bytes = utf8(text)
      out, = scratch
      rc = Runtime.function(:cast_declared_decimal).call(input_ptr(bytes), bytes.bytesize, out)
      raise ArgumentError, Constraints.send(:refusal, rc) if rc == 8
      raise "hypercast: libhypercast reported a contract violation — a binding bug, please report it" unless rc.zero?

      Interop.decode(:decimal, out[0, 16])
    end

    # The version of the native core actually loaded, as "major.minor.patch" — read from
    # the library itself, not from this gem, so a consumer can prove the core behind the
    # doors is the one this binding was built against (and name the mismatch when it is
    # not). The cheapest possible probe that the backend resolved at all: takes nothing,
    # cannot fail.
    def native_version
      Interop.version(packed_version)
    end
  end

  extend FiddleBackend

  class << self
    private

    # Presents the input as UTF-8 bytes: already-compatible text crosses as-is (Fiddle
    # passes a String's own bytes for void* — no copy of them); only foreign encodings pay
    # a transcode, and text that cannot be transcoded raises String#encode's own
    # Encoding:: error, the same one the Magnus extension raises through the same call.
    #
    # Text that is not a String is a caller bug, and the same TypeError on every backend:
    # the extension takes its argument through Ruby's implicit String conversion (to_str,
    # or a TypeError), so this does too.
    def utf8(text)
      unless text.is_a?(String)
        converted = String.try_convert(text) or
          raise TypeError, "no implicit conversion of #{text.class} into String"
        text = converted
      end
      Interop::BYTE_COMPATIBLE.include?(text.encoding) ? text : text.encode(Encoding::UTF_8)
    end

    # Fiddle spells a null pointer as nil — the core's contract for empty input.
    def input_ptr(bytes)
      bytes.empty? ? nil : bytes
    end

    # One 64-byte scratch allocation per thread, reused by every call: out-value at 0
    # (16 bytes covers every door, and malloc's alignment covers the decimal's 8), fault
    # span at 16, and the datetime door's packed day periods at 24 (40 bytes, 4-aligned,
    # written only by a call that declares them). Two Fiddle::Pointer.malloc(..., RUBY_FREE)
    # calls per cast — each registering a GC finalizer — measured as the dominant per-call
    # cost by an order of magnitude. The NumFormat does not live here: each format owns
    # its own packed pointer (see packed_cache), so a numeric call copies nothing into
    # scratch before crossing. Allocated through Runtime.buffer, which loads the library
    # first, so a missing one is reported as such before Fiddle is touched.
    def scratch
      Thread.current[:hypercast_scratch] ||= begin
        base = Runtime.buffer(64)
        [base, base + 16, base + 24]
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
        Interop.fault(rc, *Interop.characters(bytes, *fault[0, 8].unpack("L<L<")))
      end
    end

    # The shared body of every format-free door: one native call over the scratch buffers.
    def plain(symbol, text, out_size)
      bytes = utf8(text)
      out, fault = scratch
      rc = Runtime.function(symbol).call(input_ptr(bytes), bytes.bytesize, out, fault)
      verdict(rc, fault, bytes) { yield(out[0, out_size]) }
    end

    # The shared body of the integer/real/decimal doors: plain, plus the format's own
    # packed pointer, passed straight through — no per-call copy of the 92 bytes into
    # scratch.
    def numeric(symbol, text, format, out_size)
      bytes = utf8(text)
      out, fault = scratch
      rc = Runtime.function(symbol).call(input_ptr(bytes), bytes.bytesize, packed_cache[format], out, fault)
      verdict(rc, fault, bytes) { yield(out[0, out_size]) }
    end

    # The civil date-time door's body: declared, plus the day periods after the order — the
    # 40 packed bytes copied into this thread's scratch and passed from there, or nil
    # (Fiddle's null) when none are declared, which the core reads as English AM/PM only.
    def civil(symbol, text, code, periods, out_size)
      bytes = utf8(text)
      out, fault, slot = scratch
      if periods
        slot[0, periods.bytesize] = periods
      else
        slot = nil
      end
      rc = Runtime.function(symbol).call(input_ptr(bytes), bytes.bytesize, code, slot, out, fault)
      verdict(rc, fault, bytes) { yield(out[0, out_size]) }
    end

    # The 40-byte little-endian form of the native ABI's size-tagged DayPeriods struct: the
    # size word (40), then each designator as its UTF-8 bytes zero-padded to 16 followed by
    # its byte length — am, then pm — and two bytes of padding. A nil designator is a
    # length of 0: nothing declared beside AM or PM.
    def packed_day_periods(am, pm)
      am = designator("am", am)
      pm = designator("pm", pm)
      [40].pack("L<") + [am, am.bytesize, pm, pm.bytesize].pack("a16Ca16C") + "\0\0"
    end

    # A declared day-period designator as the UTF-8 the core reads — 1 to 16 bytes, no
    # ASCII digit, no ASCII whitespace at either end (the grammar around it reads that) —
    # or "" for nil. Anything else is the caller bug it names, the same ArgumentError on
    # every backend.
    def designator(name, text)
      return "" if text.nil?
      raise ArgumentError, "#{name} designator must be a String; got #{text.inspect}" unless text.is_a?(String)

      designator = text.encode(Encoding::UTF_8)
      unless designator.bytesize.between?(1, 16) && !designator.match?(/[0-9]|\A[\t\n\f\r ]|[\t\n\f\r ]\z/)
        raise ArgumentError, "#{name} designator #{text.inspect} must be 1 to 16 UTF-8 bytes with no ASCII digit " \
                             "and no ASCII whitespace at either end"
      end
      designator
    end

    # The shared body of the four doors that take a caller-declared u32 — a precision, an
    # epoch, or a field order — already resolved from its Symbol by the door. These bodies
    # (plain, numeric, declared, and packed_version below) are the whole Fiddle crossing.
    def declared(symbol, text, code, out_size)
      bytes = utf8(text)
      out, fault = scratch
      rc = Runtime.function(symbol).call(input_ptr(bytes), bytes.bytesize, code, out, fault)
      verdict(rc, fault, bytes) { yield(out[0, out_size]) }
    end

    # A typed door's number, converted the way the extension's argument is (C's
    # rb_num2dbl), with the same exception and message: a Numeric converts (a Complex with an
    # imaginary part is Complex#to_f's RangeError), anything else is a TypeError — a String
    # among them, which Kernel#Float would have parsed.
    def float(value)
      case value
      when Float then value
      when Numeric then Float(value)
      when String then raise TypeError, "no implicit conversion to float from string"
      when nil, true, false then raise TypeError, "no implicit conversion to float from #{value.inspect}"
      else raise TypeError, "can't convert #{value.class} into Float"
      end
    end

    # The shared body of the typed doors: a double instead of the text, then the declared
    # u32 when the door takes one.
    def typed(symbol, number, out_size, *declared)
      out, fault = scratch
      rc = Runtime.function(symbol).call(number, *declared, out, fault)
      if rc.zero?
        Success.new(value: yield(out[0, out_size]))
      elsif rc == -1
        raise "hypercast: libhypercast reported a contract violation — a binding bug, please report it"
      else
        Interop.fault(rc, 0, 0)
      end
    end

    # The text door's value at the byte span the core judged valid: the String passed when
    # the span is all of it and it is UTF-8 or US-ASCII, else that span as a new UTF-8 String
    # (a trimmed span, or a binary String's bytes).
    def text_value(bytes, offset, length)
      return bytes if offset.zero? && length == bytes.bytesize && bytes.encoding != Encoding::ASCII_8BIT

      bytes.byteslice(offset, length).force_encoding(Encoding::UTF_8)
    end

    # A Constraints as a checked door hands it to the core — its door Symbol, its packed
    # layout in 8-aligned native memory and its packed enum — checked against the door it was
    # passed to: a non-Constraints is a TypeError, and one bound to another door an
    # ArgumentError naming the door that judges it.
    def constrained(constraints, door)
      raise TypeError, "constraints must be a HyperCast::Constraints; got #{constraints.inspect}" unless
        constraints.is_a?(Constraints)

      declared = constraints_cache[constraints]
      text = declared[0] == :text
      if text != (door == :text)
        raise ArgumentError,
              "Constraints for #{declared[0].inspect} are judged by HyperCast.#{text ? 'text' : 'checked'}"
      end

      declared
    end

    # How many declarations constraints_cache remembers before it starts forgetting the
    # oldest: packed_cache's bound, for the same reason.
    CONSTRAINTS_CACHE_LIMIT = 64

    # Identity-keyed memo of each Constraints as the core reads it: its door, its 72-byte
    # layout copied once into native memory (which a String's bytes are not guaranteed to be
    # aligned for) and its packed enum, frozen. Bounded and evicted as packed_cache is.
    def constraints_cache
      @constraints_cache ||= Hash.new do |cache, constraints|
        cache.shift while cache.size >= CONSTRAINTS_CACHE_LIMIT
        _code, raw, entries = constraints.send(:declaration)
        pointer = Runtime.buffer(raw.bytesize)
        pointer[0, raw.bytesize] = raw
        cache[constraints] = [constraints.door, pointer, entries.freeze].freeze
      end.compare_by_identity
    end

    # One 96-byte scratch allocation per thread for the checked doors, beside #scratch: a
    # packed layout constraints_check judges at 0 (72 bytes, 8-aligned by malloc), and the
    # check a checked door writes at 72 (12 bytes).
    def checked_scratch
      Thread.current[:hypercast_checked_scratch] ||= begin
        base = Runtime.buffer(96)
        [base, base + 72]
      end
    end

    # The shared body of the checked doors: the door's own arguments, then the scratch
    # out-value, fault and check.
    def checked_call(symbol, *arguments)
      out, fault = scratch
      Runtime.function(symbol).call(*arguments, out, fault, checked_scratch[1])
    end

    # Presents a checked door's return code as its verdict: 0 a Checked of the value the
    # block reads from the out-value and check, 1 to 3 the door's Fault, 4 a Violation — each
    # span over `bytes` (the UTF-8 the core read) as a Fault's is, or as written when there
    # is no text — and -1 a binding bug that raises.
    def checked_verdict(rc, bytes)
      out, fault = scratch
      check = checked_scratch[1]
      if rc.zero?
        adjusted, index = check[4, 8].unpack("LL")
        return Checked.new(value: yield(out, check), adjusted: adjusted != 0,
                           enum_index: index == 0xFFFF_FFFF ? nil : index)
      end
      raise "hypercast: libhypercast reported a contract violation — a binding bug, please report it" unless
        rc.between?(1, 4)

      offset, length = fault[0, 8].unpack("LL")
      offset, length = Interop.characters(bytes, offset, length) if bytes
      return Interop.fault(rc, offset, length) unless rc == 4

      Violation.new(keyword: Keyword.new(code: check[0, 4].unpack1("L")), offset: offset, length: length)
    end

    # The loaded core's version word, major << 16 | minor << 8 | patch, straight from the
    # library's zero-argument hypercast_version export.
    def packed_version
      Runtime.function(:hypercast_version).call
    end

    # How many formats packed_cache remembers before it starts forgetting the oldest. Far
    # more than a process that reuses its formats ever holds; what it bounds is the one
    # that builds a NumFormat per request.
    PACKED_CACHE_LIMIT = 64

    # Identity-keyed memo (compare_by_identity — a pointer hash, not Data's structural
    # #hash over nine Strings and an Integer) of a native 92-byte RawNumFormat per format
    # object, filled once from NumFormat#packed. Formats are reused constants in practice,
    # so the common call finds its pointer in one lookup and packs nothing. The Hash holds
    # the format, so a key can never be a recycled address; the race is benign (idempotent).
    #
    # Bounded, oldest entry out first (a Hash keeps insertion order): unbounded, it held
    # every format ever cast with and its 92-byte allocation for the life of the process,
    # which a consumer building a format per request experienced as a leak. A format
    # evicted and then used again is simply packed again, and an evicted pointer stays
    # alive for any call already holding it — the caller's own reference outlives the cache's.
    def packed_cache
      @packed_cache ||= Hash.new do |cache, format|
        cache.shift while cache.size >= PACKED_CACHE_LIMIT
        size = NumFormat::PACKED_BYTES
        pointer = Runtime.buffer(size)
        pointer[0, size] = format.packed
        cache[format] = pointer
      end.compare_by_identity
    end
  end
end

# --- backend selection: the Magnus extension, when present, defines the doors on this
# module's singleton class, in front of the FiddleBackend definitions it extends (no
# delegation layer, no redefinition) — Fiddle's measured 1.6 µs per-call floor
# drops to an ordinary extension call. The pure-Fiddle definitions stay the universal
# zero-compile fallback — the last resort, for a Ruby or a platform no platform gem covers;
# precompiled platform gems are how the extension ships without ever making a consumer
# compile anything, and they carry no Fiddle library at all.
#
# HYPERCAST_PURE forces Fiddle. It is a testing and diagnostic switch — CI runs the whole
# suite through it, and it is how a suspected extension bug is ruled in or out — not a
# setting a deployment needs. It is read for presence, not value, so "0" and the empty
# string force it too. Inside a platform gem there is no library for it to load: BACKEND
# still reads :fiddle, HyperCast.available? answers false, and the first door call raises a
# LoadError naming the universal gem, which is where the Fiddle backend lives.
HyperCast::BACKEND =
  if ENV["HYPERCAST_PURE"]
    :fiddle
  else
    # Two layouts, and both have to work. A released platform gem is a "fat" gem carrying one
    # extension per supported Ruby ABI under lib/hypercast/<minor>/ (see the Rakefile's
    # native:gem task for why an ABI-per-file is unavoidable — Magnus has no `abi3`
    # equivalent), and `rake native:dev` — the local dev loop — stages its own build at that
    # same versioned path. CI's in-job staging additionally drops a single extension flat at
    # lib/, which is also where a hand copy of `cargo ruby-ext`'s output goes (the alias only
    # builds into rust/target/ruby/release/; nothing but that rake task copies it here).
    # Trying the versioned path first and the flat one second means neither has to know the
    # other exists.
    #
    # A miss on both is not an error: it means this Ruby/platform combination has no
    # precompiled extension, which is precisely what the Fiddle backend is for. A platform
    # with no shared library either still lands on Fiddle, so the first call raises its own
    # precise error — a "not found" LoadError naming the path, or
    # NativePlatform::UnsupportedPlatformError naming the platform. HyperCast.available? is
    # the quiet way to ask.
    begin
      require "hypercast/#{RUBY_VERSION[/\d+\.\d+/]}/hypercast_native"
      :native
    rescue LoadError
      begin
        require "hypercast_native"
        :native
      rescue LoadError
        :fiddle
      end
    end
  end
