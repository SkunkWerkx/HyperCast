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
  VERSION = "1.1.0"

  # The success case of a verdict: a cast value.
  Success = Data.define(:value)

  # The failure case: a closed reason Symbol (:empty, :malformed, :out_of_range) plus the
  # offending span, in the units String#[] slices by on the text you passed: character
  # offsets for text (in any encoding — the core reads UTF-8 and the span is mapped back),
  # byte offsets for a binary (ASCII-8BIT) String, whose characters are its bytes. Either
  # way `text[offset, length]` is the offending text, with no mapping on your side.
  # Nothing is captured — slicing it out of the input is the caller's choice.
  #
  # A fault the core reports also carries #detail, what exactly was wrong (a Detail), beside the
  # broad reason. It is not a member: equality, #hash, #to_h, #inspect, the members and the
  # positional and keyword constructors are what they were, so a Fault built by hand with three
  # arguments is #== to the same fault read from a door, and its #detail is nil. A pattern asks
  # for it by name: `in Fault(reason: :malformed, detail: Detail::GROUP_SIZE)`.
  Fault = Data.define(:reason, :offset, :length) do
    # The three members, and optionally the Detail (or nil) the core reported.
    def initialize(reason:, offset:, length:, detail: nil)
      @detail = detail
      super(reason: reason, offset: offset, length: length)
    end

    # What exactly was wrong, a Detail; nil for a Fault built without one.
    attr_reader :detail

    # The members, and #detail when a pattern names it.
    def deconstruct_keys(keys)
      return super unless keys&.include?(:detail)

      super(keys - [:detail]).merge(detail: @detail)
    end

    # A copy with the given members replaced, keeping #detail unless one is given.
    def with(**changes)
      return self if changes.empty?

      self.class.new(**to_h, detail: @detail, **changes)
    end

    # Marshal's form: the members and the detail, so a loaded fault keeps its detail.
    def marshal_dump
      to_h.merge(detail: @detail)
    end

    # Restores a fault from #marshal_dump's form through #initialize.
    def marshal_load(members)
      initialize(**members)
    end
  end

  # The native core's failure codes, mapped to the closed reason Symbols a Fault carries.
  REASONS = { 1 => :empty, 2 => :malformed, 3 => :out_of_range }.freeze

  # What exactly a Fault found wrong: the native core's numeric code and its published name. The
  # codes keep their numbers and meanings, and a later core may add more, so a case over details
  # needs an else branch: a code this gem does not name is kept, its #name :unknown. The reason
  # and span of a fault stay usable either way. The constants (Detail::BLANK ...
  # Detail::NOT_A_SCALAR, one per published code) compare equal to every detail of their code, so
  # `in Fault(detail: Detail::GROUP_SIZE)` matches.
  Detail = Data.define(:code) do
    # The published name as a Symbol (:group_size), or :unknown for a code this gem does not name.
    def name
      self.class::NAMES.fetch(code, :unknown)
    end

    # True for a code this gem does not name; #code still holds it.
    def unknown?
      !self.class::NAMES.key?(code)
    end

    # The name, as text ("group_size"; "unknown" for an unknown code).
    def to_s
      name.to_s
    end

    # The constant that names this detail (HyperCast::Detail::GROUP_SIZE), or the Data form for
    # a code this gem does not name.
    def inspect
      unknown? ? super : "HyperCast::Detail::#{name.to_s.upcase}"
    end

    # The detail of a core's code: the constant of a published one, a new Detail for any other.
    def self.of(code)
      self::BY_CODE.fetch(code) { new(code: code) }
    end
  end

  # The published name of each detail, by code.
  Detail::NAMES = {
    1 => :blank, 2 => :unexpected_character, 3 => :digit_expected, 4 => :delimiter_expected,
    5 => :trailing_text, 6 => :no_digits, 7 => :sign_misplaced, 8 => :parentheses_unbalanced,
    9 => :mark_repeated, 10 => :group_misplaced, 11 => :group_size, 12 => :group_in_fraction,
    13 => :ambiguous_separator, 14 => :fraction_not_allowed, 15 => :field_width, 16 => :wrong_length,
    17 => :component_missing, 18 => :not_recognized, 19 => :unknown_unit, 20 => :invalid_utf8,
    21 => :too_long, 22 => :not_finite, 23 => :not_whole, 24 => :negative_not_allowed,
    25 => :too_large, 26 => :too_small, 27 => :too_precise, 28 => :date_field_out_of_range,
    29 => :time_field_out_of_range, 30 => :outside_window, 31 => :nonexistent_date, 32 => :not_a_scalar
  }.freeze

  # Each published detail by code, one frozen constant apiece (Detail::BLANK is code 1, and so
  # on through Detail::NOT_A_SCALAR, code 32): a Fault's #detail is one of these.
  Detail::BY_CODE = Detail::NAMES.to_h do |code, name|
    [code, Detail.const_set(name.to_s.upcase, Detail.new(code: code))]
  end.freeze

  # The 25 code points of Unicode's White_Space property, the core's one notion of white
  # space: a character class the declaration checks match with (the Magnus backend's designator
  # check reads the core's own predicate). Held to corpus/formats.json by the suite.
  WHITE_SPACE = /[\t\n\v\f\r \u0085\u00A0\u1680\u2000-\u200A\u2028\u2029\u202F\u205F\u3000]/
  private_constant :WHITE_SPACE

  class << self
    private

    # A declared symbol's text as the UTF-8 the core reads, transcoded from its own encoding.
    # Text with no UTF-8 form (a lone UTF-16 surrogate, bytes its encoding does not allow, a
    # character with no Unicode counterpart) is data at a door, MALFORMED at that character
    # (#kept); in a declaration it is the caller's bug, refused with the same ArgumentError on
    # every backend, never declared as a stand-in for the character it could not spell.
    def declared_utf8(field, text)
      symbol = text.encode(Encoding::UTF_8)
      return symbol if symbol.valid_encoding?

      raise ArgumentError, "#{field} #{text.inspect} must be whole characters with a UTF-8 form"
    rescue Encoding::InvalidByteSequenceError, Encoding::UndefinedConversionError,
           Encoding::ConverterNotFoundError
      raise ArgumentError, "#{field} #{text.inspect} must be whole characters with a UTF-8 form"
    end
  end

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
  # an equal pair in force for currency amounts), a symbol with no UTF-8 form (a lone UTF-16
  # surrogate, bytes its encoding does not allow), a symbol carrying an ASCII digit, a sign,
  # percent symbol, or a decimal separator, carrying white space (a group separator may be
  # white space), a currency symbol longer than 16 bytes or starting or ending in white
  # space (inside it white space is fine: "F CFA"), or a digit zero that is ASCII or starts
  # no digit block.
  #
  # Grouping is a policy, declared apart from the flags: `grouping:` is :western (the default:
  # once a numeral uses the group separator, groups of exactly three digits counted from the
  # decimal separator, the leftmost one to three and not starting with a zero, so "1,234,567"
  # reads and "12,34" and "0,125" are :malformed Faults with the detail Detail::GROUP_SIZE),
  # or :indian (the group beside the decimal separator three digits, every group to its left two,
  # the leftmost one or two: "12,34,567"), which reads only when declared. An ungrouped
  # numeral reads under every policy, and a policy only judges groups: without the GROUPING flag
  # a group separator is still refused. `flags` is the style mask (GROUPING ... TRAILING_SIGN)
  # and never carries a policy: bits 8 and 9 or any other bit this version does not define is an
  # ArgumentError.
  NumFormat = Data.define(:decimal_sep, :group_sep, :flags, :currency,
                          :currency_decimal_sep, :currency_group_sep,
                          :minus_sign, :plus_sign, :percent, :digit_zero) do
    # Validates every declared field up front — the separators (1 to 8 UTF-8 bytes each,
    # distinct from each other, and so the pair in force for currency amounts), the signs,
    # the percent and currency symbols (no ASCII digit, since a digit is a digit before it
    # is anything else, and no white space, which would collide with the trimming around
    # them; the decimal separators carry none either, and a currency symbol only at its
    # edges) and the digit zero (one non-ASCII
    # character whose ten successors share its UTF-8 prefix, which every Unicode digit
    # block's zero does) — so a malformed format fails loudly as the caller bug it is. Every
    # field is stored transcoded to UTF-8, the encoding the core reads — a separator handed
    # over in another encoding (a no-break space read from a Latin-1 file, say) is the same
    # code point on every backend rather than whatever its bytes happened to spell in the
    # encoding it arrived in.
    def initialize(decimal_sep:, group_sep:, flags:, currency: "", currency_decimal_sep: "",
                   currency_group_sep: "", minus_sign: "", plus_sign: "", percent: "", digit_zero: "",
                   grouping: :western)
      check_flags(flags)
      raise ArgumentError, "grouping must be one of #{GROUPING_POLICIES.keys.inspect}; got #{grouping.inspect}" unless
        GROUPING_POLICIES.key?(grouping)

      @grouping = grouping

      limit = self.class::SYMBOL_MAX_BYTES
      decimal_sep = declared("decimal separator", decimal_sep, limit, required: true)
      group_sep = declared("group separator", group_sep, limit, required: true)
      raise ArgumentError, "decimal and group separators must differ; both are #{decimal_sep.inspect}" if
        decimal_sep == group_sep

      currency_decimal_sep = declared("currency decimal separator", currency_decimal_sep, limit)
      currency_group_sep = declared("currency group separator", currency_group_sep, limit)
      currency = declared("currency symbol", currency, self.class::CURRENCY_MAX_BYTES)
      minus_sign = declared("minus sign", minus_sign, limit)
      plus_sign = declared("plus sign", plus_sign, limit)
      percent = declared("percent symbol", percent, limit)
      money_decimal = currency_decimal_sep.empty? ? decimal_sep : currency_decimal_sep
      money_group = currency_group_sep.empty? ? group_sep : currency_group_sep
      raise ArgumentError, "currency decimal and group separators must differ; both are #{money_decimal.inspect}" if
        money_decimal == money_group

      # White space comes last, after every field's length and digit and the equal separators.
      { "decimal separator" => decimal_sep, "currency decimal separator" => currency_decimal_sep,
        "minus sign" => minus_sign, "plus sign" => plus_sign, "percent symbol" => percent }.each do |field, symbol|
        raise ArgumentError, "#{field} #{symbol.inspect} must not contain white space" if
          symbol.match?(WHITE_SPACE)
      end
      # A currency symbol may hold white space inside ("F CFA"), never at an edge.
      raise ArgumentError, "currency symbol #{currency.inspect} must not start or end with white space" if
        currency.match?(/\A#{WHITE_SPACE.source}|#{WHITE_SPACE.source}\z/)

      super(decimal_sep: decimal_sep, group_sep: group_sep, flags: flags,
            currency: currency, currency_decimal_sep: currency_decimal_sep, currency_group_sep: currency_group_sep,
            minus_sign: minus_sign, plus_sign: plus_sign, percent: percent,
            digit_zero: digit_zero_of(digit_zero))
    end

    # The grouping policy, :western or :indian. It is not a member, as Fault#detail is
    # not: the ten members, #to_h, #inspect and the positional and keyword constructors are
    # 1.0's, and the policy rides beside them, counting in #==, #eql? and #hash (an :indian and a
    # :western format must never be one memo key). A NumFormat restored from a 1.0 Marshal
    # payload carries no policy and is :western, like everything that records none.
    def grouping
      @grouping || :western
    end

    # The flags word the native layout carries: #flags with the grouping policy in bits 8 and 9.
    # A 1.0 Marshal payload is restored member by member with no call this class can hook, so a
    # flags word it stored is judged here, on its first use.
    def raw_flags
      check_flags(flags)
      flags | GROUPING_POLICIES.fetch(grouping)
    end

    # Equal when every member and the grouping policy are.
    def ==(other)
      super && grouping == other.grouping
    end

    # Eql when every member and the grouping policy are.
    def eql?(other)
      super && grouping.eql?(other.grouping)
    end

    # A hash over the members and the grouping policy.
    def hash
      [super, grouping].hash
    end

    # The members, and #grouping when a pattern names it.
    def deconstruct_keys(keys)
      return super unless keys&.include?(:grouping)

      super(keys - [:grouping]).merge(grouping: grouping)
    end

    # A copy with the given members (or grouping:) replaced, keeping the policy unless one is
    # given, and validated as NumFormat.new validates. With none given it is the format itself.
    def with(**changes)
      return self if changes.empty?

      self.class.new(**to_h, grouping: grouping, **changes)
    end

    # Marshal's form: the members and the policy, so a loaded format keeps its policy and is
    # judged again as a new one is, and a stored policy that isn't one is an ArgumentError here. A
    # 1.0 payload (the struct form) loads without a policy, which #grouping reads as :western.
    def marshal_dump
      to_h.merge(grouping: grouping)
    end

    # Restores a format from #marshal_dump's form through #initialize.
    def marshal_load(members)
      initialize(**members)
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
    #
    # The Hash says nothing about how a locale groups digits, so grouping: declares it — :western,
    # the default, reads groups of three ("1.234.567,89"), and :indian is lakh and crore
    # ("12,34,567").
    def self.from_i18n(number_format, currency_format: nil, flags: ALL_STYLES, grouping: :western)
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
          currency_decimal_sep: currency_separator, currency_group_sep: currency_delimiter, grouping: grouping)
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

    # The 92-byte little-endian form of the ABI's size-tagged NumFormat struct (what the
    # core's RawNumFormat::from_le_bytes reads, and Python's NumFormat.packed is): the size
    # word (92), the flags with the grouping policy in bits 8 and 9 (#raw_flags), the digit
    # zero's code point (0 for none), each a little-endian u32 on every host, then each symbol
    # as its UTF-8 bytes zero-padded to its field followed by its byte length — eight bytes
    # and a length for each of the two number separators, the two currency separators, the two
    # signs and the percent symbol, in that order; sixteen and a length for the currency
    # symbol. A length of 0 declares nothing.
    def packed
      head = [self.class::PACKED_BYTES, raw_flags, digit_zero.empty? ? 0 : digit_zero.ord].pack("L<L<L<")
      symbols = [decimal_sep, group_sep, currency_decimal_sep, currency_group_sep, minus_sign, plus_sign, percent]
      head + symbols.map { |symbol| [symbol, symbol.bytesize].pack("a8C") }.join +
        [currency, currency.bytesize].pack("a16C")
    end

    private

    # The style mask: an Integer of the defined flag bits (0 through 7) and nothing else. The
    # grouping policy has its own keyword, so bits 8 and 9 are refused as a policy in the wrong
    # place, and any bit above them as one this version does not define.
    def check_flags(flags)
      raise ArgumentError, "flags must be an Integer; got #{flags.inspect}" unless flags.is_a?(Integer)
      raise ArgumentError, "flags #{flags} must not carry a grouping policy (bits 8 and 9); declare it with " \
                           "grouping:" if flags.anybits?(GROUPING_POLICY)
      raise ArgumentError, "flags #{flags} sets a bit this version does not define (bit 10 and above)" unless
        flags.between?(0, 0xFF)
    end

    # The declared text of one field as the UTF-8 the core reads: a String of 1 to `limit`
    # UTF-8 bytes with no ASCII digit, or "" where the field may be left undeclared. Anything
    # else is the caller bug it names. White space is refused by the caller, once every
    # field has passed this and the separators are known to differ.
    def declared(field, text, limit, required: false)
      raise ArgumentError, "#{field} must be a String; got #{text.inspect}" unless text.is_a?(String)

      symbol = HyperCast.send(:declared_utf8, field, text)
      if symbol.empty?
        raise ArgumentError, "#{field} must not be empty" if required

        return symbol
      end
      raise ArgumentError, "#{field} #{text.inspect} exceeds #{limit} UTF-8 bytes" if symbol.bytesize > limit

      raise ArgumentError, "#{field} #{text.inspect} must not contain an ASCII digit" if symbol.match?(/[0-9]/)

      symbol
    end

    # The declared digit zero as UTF-8: "" for none, else exactly one character that is not
    # ASCII and whose code point modulo 64 is at most 54 — true of the zero of every decimal
    # digit block Unicode defines, and the shape the core recognises a digit by its last
    # byte from.
    def digit_zero_of(text)
      raise ArgumentError, "digit zero must be a String; got #{text.inspect}" unless text.is_a?(String)

      zero = HyperCast.send(:declared_utf8, "digit zero", text)
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

  # Permit the group separator between digits. The NumFormat's grouping policy judges the sizes
  # (Western by default, :indian when declared).
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
  # white space between symbol and digits; accounting parentheses wrap the symbol
  # along with the digits ("($5)"). With no symbol declared the flag matches nothing.
  # Integer, real and decimal doors.
  CURRENCY = 1 << 6
  # Permit the sign after the digits ("5-", "5 -", "5 €-", "5- €"), the way a locale whose
  # negative_format is "%n-" writes it: ASCII - and + or the format's declared signs, once,
  # with optional white space before it, and never beside a leading sign or inside
  # accounting parentheses. Not part of ALL_STYLES — NumFormat.from_i18n sets it from the
  # locale's own negative_format rather than assuming it.
  TRAILING_SIGN = 1 << 7
  # Every lenience on — every one there was before TRAILING_SIGN, which is declared from a
  # locale's pattern rather than assumed (SEPARATOR_DETECT is a separator policy, not a
  # lenience, and is deliberately not included either).
  ALL_STYLES = GROUPING | PARENTHESES | EXPONENT | RADIX_PREFIXES | PERCENT | CURRENCY

  # Grouping policy Western, no bits, the default: groups of exactly three digits from the
  # decimal separator, the leftmost one to three.
  GROUPING_WESTERN = 0
  # Grouping policy Indian, bit 9, only when declared: three digits beside the decimal separator,
  # two in each group to its left, the leftmost one or two.
  GROUPING_INDIAN = 2 << 8
  # The two policy bits, 8 and 9, of the native flags word. Bit 8 (256) is retired and never
  # reused, so 256 and 768 name no policy and are refused. A NumFormat declares its policy with
  # grouping:, never in flags.
  GROUPING_POLICY = 3 << 8

  # The grouping policies a NumFormat's grouping: names, with their bits in the native flags word.
  GROUPING_POLICIES = { western: GROUPING_WESTERN, indian: GROUPING_INDIAN }.freeze

  # The invariant profile — '.' decimal, ',' grouping, every lenience on, Western
  # grouping, no currency symbol declared.
  NumFormat::INVARIANT = NumFormat.new(decimal_sep: ".", group_sep: ",", flags: ALL_STYLES)

  # The detection profile — every lenience on, Western grouping, ./, roles resolved per
  # input by SEPARATOR_DETECT's structural rules.
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

  # The date shapes the doors read only when the caller opts in: a flag set (the doors' own
  # shapes stay exactly as they are without it) and a pivot for two-digit years. Passed as
  # `options:` to the doors that read them — COMPACT_DATE at #date, SPACE_SEPARATOR and
  # HOUR_OFFSET at #timestamp, the pivot at #date with an order and at #datetime — and a known
  # option a door does not read is ignored. #flags is an Integer of the defined bits and nothing
  # else (any other bit is an ArgumentError); #pivot is nil for none, or the last year of the
  # 100-year window a two-digit year is read in, 100 through 9999: with a pivot of 2029, "26"
  # is 2026 and "30" is 1930. A pivot of 0 is none, too.
  DateOptions = Data.define(:flags, :pivot) do
    # Validates the flags and the pivot up front, so options a door could not honour are the
    # caller bug they are, never a verdict.
    def initialize(flags: 0, pivot: nil)
      raise ArgumentError, "date option flags must be an Integer; got #{flags.inspect}" unless flags.is_a?(Integer)
      raise ArgumentError, "date option flags #{flags} set a bit this version does not define" unless
        flags.nobits?(~self.class::DEFINED)
      unless pivot.nil? || (pivot.is_a?(Integer) && (pivot.zero? || pivot.between?(100, 9999)))
        raise ArgumentError, "date pivot must be nil, 0, or 100 to 9999; got #{pivot.inspect}"
      end

      super(flags: flags, pivot: pivot&.zero? ? nil : pivot)
    end

    # The 12 bytes the native layout carries, in the host's byte order: the size (12), the
    # flags and the pivot (0 for none), as u32s.
    def packed
      [self.class::PACKED_BYTES, flags, pivot || 0].pack("L3")
    end
  end

  # `cast_date` also reads `yyyyMMdd`, eight ASCII digits.
  DateOptions::COMPACT_DATE = 1
  # `cast_timestamp` also reads a single space where the `T` goes.
  DateOptions::SPACE_SEPARATOR = 2
  # `cast_timestamp` also reads an offset of `±hh`, with no minutes (`+00`, `-05`).
  DateOptions::HOUR_OFFSET = 4
  # Every flag this version defines.
  DateOptions::DEFINED = DateOptions::COMPACT_DATE | DateOptions::SPACE_SEPARATOR | DateOptions::HOUR_OFFSET
  # The size of DateOptions#packed, the native layout, in bytes.
  DateOptions::PACKED_BYTES = 12
  # COMPACT_DATE, SPACE_SEPARATOR and HOUR_OFFSET together: the shapes real feeds mix with
  # RFC 3339. It supplies no pivot and no month names (`COMMON_FORMS.with(pivot: 2029)` adds one).
  DateOptions::COMMON_FORMS = DateOptions.new(flags: DateOptions::DEFINED)

  # Declared month names for the doors that read a month field (#date with an order, and
  # #datetime): "Jan", "janvier", "janv.". Built by the caller, never looked up for a locale —
  # aliases, abbreviations and grammatical forms are the caller's decisions — from a Hash of
  # name => month, or an Array of [month, name] pairs (the order the native layout packs them in).
  # Passed as `months:` it replaces the numeric month field, in the position the declared field
  # order selects and with the date's own `/`, `-` or `.` separator: "07-Jan-26" reads with a
  # pivot, "7-Jan-2026" without one, and a vocabulary never reorders fields.
  #
  # Names match under simple case folding, the longest name followed by the separator winning;
  # punctuation is part of a name and never stripped, so with "janv." declared and "." as the
  # separator "7.janv..2026" reads and "7.janv.2026" is :malformed unless "janv" is declared too.
  # The vocabulary rules, each an ArgumentError here: every entry a month 1 through 12 and a
  # String with a UTF-8 form of 1 to 64 bytes holding no ASCII digit, "/", "-" or white space ("."
  # is fine anywhere); every month named at least once; no two names equal under case folding,
  # even for one month.
  MonthNames = Data.define(:entries) do
    # Judges the entries against the rules, then has the core judge the vocabulary once more
    # (the one rule Ruby cannot restate is the core's simple case folding), through the date
    # door: on the Fiddle backend that is a native call, so building a vocabulary raises a
    # LoadError, as every door does, when the shared library is missing.
    def initialize(entries:)
      list = self.class.send(:judged, self.class.send(:pairs, entries))
      @packed = self.class.send(:pack, list)
      super(entries: list)
      self.class.send(:core_judges, self)
    end

    # The vocabulary in the packed layout the native core reads: each entry a month byte, a
    # little-endian u32 byte length and the name's UTF-8 bytes, with no padding.
    def packed
      @packed
    end

    # Marshal's form: the entries, so a loaded vocabulary is judged again as a new one is.
    def marshal_dump
      entries
    end

    # Restores a vocabulary from #marshal_dump's entries through #initialize.
    def marshal_load(list)
      initialize(entries: list)
    end

    # The vocabulary read from its packed layout (what #packed returns), judged by the same rules
    # as one built from entries; an entry cut short or bytes trailing the last one is an
    # ArgumentError, as is a name that is not UTF-8.
    def self.from_packed(bytes)
      raise ArgumentError, "packed month names must be a String; got #{bytes.inspect}" unless bytes.is_a?(String)

      bytes = bytes.b
      list = []
      at = 0
      while at < bytes.bytesize
        head = bytes.byteslice(at, self::ENTRY_HEAD)
        raise ArgumentError, "month names are cut short, or bytes trail the last entry" if
          head.bytesize < self::ENTRY_HEAD

        month, length = head.unpack("CL<")
        # Measured before slicing: where a C long is 32 bits (Windows), byteslice raises RangeError
        # for a declared length past 2^31 - 1.
        raise ArgumentError, "month names are cut short, or bytes trail the last entry" if
          length > bytes.bytesize - at - self::ENTRY_HEAD

        name = bytes.byteslice(at + self::ENTRY_HEAD, length)
        list << [month, name.force_encoding(Encoding::UTF_8)]
        at += self::ENTRY_HEAD + length
      end
      new(entries: list)
    end

    # Entries as [month, name] pairs: from a Hash of name => month, or any Enumerable of pairs.
    def self.pairs(entries)
      case entries
      when Hash then entries.map { |name, month| [month, name] }
      else
        raise ArgumentError, "month names must be a Hash or an Array of [month, name] pairs; " \
                             "got #{entries.inspect}" unless entries.respond_to?(:to_a)

        entries.to_a.map do |pair|
          raise ArgumentError, "a month name entry must be a [month, name] pair; got #{pair.inspect}" unless
            pair.respond_to?(:to_ary) && pair.to_ary.size == 2

          pair.to_ary
        end
      end
    end

    # The entries as frozen [month, UTF-8 name] pairs, once every rule holds, in the order the
    # core meets them: each entry's month, UTF-8 form, length and characters; then each month's
    # entry.
    def self.judged(pairs)
      list = pairs.map do |month, name|
        raise ArgumentError, "month #{month.inspect} must be an Integer from 1 to 12" unless
          month.is_a?(Integer) && month.between?(1, 12)
        raise ArgumentError, "month name must be a String; got #{name.inspect}" unless name.is_a?(String)

        text = name.encoding == Encoding::BINARY ? name.dup.force_encoding(Encoding::UTF_8) : name
        text = HyperCast.send(:declared_utf8, "month name", text)
        raise ArgumentError, "month name #{name.inspect} must be 1 to 64 UTF-8 bytes" unless
          text.bytesize.between?(1, 64)
        raise ArgumentError, "month name #{name.inspect} must not contain an ASCII digit, a slash, a hyphen " \
                             "or white space" if text.match?(%r{[0-9/-]|#{WHITE_SPACE.source}})

        [month, text.dup.freeze].freeze
      end
      missing = (1..12).reject { |month| list.any? { |entry, _| entry == month } }
      raise ArgumentError, "month names must name every month; none for #{missing.join(', ')}" unless missing.empty?

      list.freeze
    end

    # The vocabulary in its packed layout, a frozen binary String.
    def self.pack(list)
      list.map { |month, name| [month, name.bytesize].pack("CL<") + name.b }.join.b.freeze
    end

    # The core's last word on a vocabulary Ruby accepted, which only a folded duplicate can
    # still fail: the core's simple case folding is not String#downcase(:fold)'s full one.
    def self.core_judges(names)
      HyperCast.date("", :year_month_day, months: names)
    rescue RefusedDeclaration
      raise ArgumentError, "two month names fold to the same spelling"
    end
    private_class_method :pairs, :judged, :pack, :core_judges
  end

  # A declaration Ruby accepted and the core refused: date options or month names that passed
  # their own checks and still broke a rule only the core judges (a folded duplicate), or that
  # were built without going through their constructors (Data#allocate, Marshal). The
  # constructors turn it into the ArgumentError they document; a door raises it as it is.
  class RefusedDeclaration < ArgumentError; end

  # The bytes an entry takes before its name in MonthNames#packed: the month byte and the length.
  MonthNames::ENTRY_HEAD = 5

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

  # Every door that reads text, by name, mapped to the native core's door code (the numbering
  # of its detailed exports).
  DETAILED_DOORS = {
    bool: 1, i8: 2, i16: 3, i32: 4, i64: 5, u8: 6, u16: 7, u32: 8, u64: 9, f32: 10, f64: 11, uuid: 12,
    timestamp: 13, unix: 14, date: 15, time: 16, duration: 17, text: 18, decimal: 19, date_ordered: 20,
    datetime: 21, excel_serial: 22, char: 23
  }.freeze

  # What a value that is not a multiple of a declared multiple_of becomes, mapped to the
  # native core's rounding codes: :strict (a :multiple_of Violation, the default), :midpoint
  # (the nearest multiple, ties to even), :floor (toward negative infinity) and :ceiling
  # (toward positive infinity). An unknown rounding is a caller bug (KeyError).
  ROUNDINGS = { strict: 0, midpoint: 1, floor: 2, ceiling: 3 }.freeze

  # The success case of a checked door: the value (rounded to multiple_of when the declaration
  # rounds; at the :text door with an enum, the very String the declaration was given),
  # adjusted (true only when rounding changed the value) and, at the :text door with an enum,
  # the index of the entry matched — nil otherwise, a numeric enum included, where the value
  # is the answer.
  Checked = Data.define(:value, :adjusted, :enum_index)

  # A value that cast and broke a declared rule: the Keyword broken, the value exactly as the
  # door cast it (in the door's own type, as a Checked's value is, and never rounded: a
  # rounding is part of the judgement, not of the value; at :text the judged String, after
  # trim, never an enum entry) and the value's span, in the units a Fault's span is in
  # (characters for text, bytes for a binary String). A number the caller already held has no
  # text, and an empty span. A Fault means the input is not the door's type at all; a
  # Violation means it is, and breaks a rule.
  Violation = Data.define(:keyword, :value, :offset, :length)

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

  # Validation constraints — the OpenAPI 3.2 Schema Object's keywords — bound to one door and
  # judged once, by the native core, when built: every declaration OpenAPI allows is accepted,
  # an unsatisfiable one included (crossed bounds, an empty enum: every value then violates
  # it), and a keyword for another type is ignored, as JSON Schema ignores it. What the core
  # refuses is an ArgumentError here, its message the rule it breaks ("Constraints refused
  # (code 5): multipleOf is not positive."), and every checked cast after that judges only
  # data. The door is required: no declaration ever silently means "no rules", and
  # `Constraints.new(door: :f64)` is an explicit, empty one.
  #
  # The numeric doors take minimum, exclusive_minimum, maximum and exclusive_maximum (four
  # independent keywords, all four together if declared so), multiple_of with a rounding (a
  # ROUNDINGS Symbol), and enum, matched exactly against the value; :text min_length,
  # max_length (in code points), enum (matched under simple case folding unless
  # case_sensitive: true, which compares byte for byte), and HyperCast's own extensions trim
  # (of Unicode White_Space) and case_sensitive. Every keyword is accepted at every door; the
  # core alone refuses trim, case_sensitive or a rounding where they mean nothing.
  #
  # A bound, step or numeric enum entry is an Integer, a BigDecimal, a String in invariant
  # notation ("0.000001", "-90", "1e-6") or this gem's own Decimal, and crosses the core's own
  # conversion (HyperCast.declared_decimal): never a Float, since 0.1 has no exact binary form
  # and a Float could only declare a neighbour of the value meant. Bounds are kept as the exact
  # Decimal declared. A text enum entry is a String, kept as the very object given when it is
  # frozen and as a frozen copy when it is not, so a match delivers what the core judged. An
  # enum left nil declares none, while `enum: []` declares one no value matches. With
  # duplicate entries, the first match wins.
  #
  #   latitude = HyperCast::Constraints.new(door: :f64, minimum: "-90", maximum: "90")
  #   HyperCast.checked("91", HyperCast::NumFormat::INVARIANT, latitude)
  #   # => #<data HyperCast::Violation keyword=HyperCast::Keyword::MAXIMUM, value=91.0, offset=0, length=2>
  Constraints = Data.define(:door, :minimum, :exclusive_minimum, :maximum, :exclusive_maximum, :multiple_of,
                            :rounding, :min_length, :max_length, :trim, :case_sensitive, :enum) do
    # Validates every keyword's type, converts the bounds, steps and numeric entries through
    # the core, and has the core judge the declaration; a refusal is an ArgumentError worded
    # from its code. A wrong type is an ArgumentError, an unknown door or rounding a KeyError.
    def initialize(door:, minimum: nil, exclusive_minimum: nil, maximum: nil, exclusive_maximum: nil,
                   multiple_of: nil, rounding: :strict, min_length: nil, max_length: nil, trim: false,
                   case_sensitive: false, enum: nil)
      keywords = { door: door, minimum: minimum, exclusive_minimum: exclusive_minimum, maximum: maximum,
                   exclusive_maximum: exclusive_maximum, multiple_of: multiple_of, rounding: rounding,
                   min_length: min_length, max_length: max_length, trim: trim, case_sensitive: case_sensitive,
                   enum: enum }
      code = HyperCast.constraints_check(**keywords)
      raise ArgumentError, self.class.send(:refusal, code) unless code.zero?

      super(**self.class.send(:members_of, **keywords))
    end

    # The 104 bytes this declaration crosses a C ABI as, in the native byte order the core
    # reads in place: the size (104), the flags, the rounding code, min_length, max_length and
    # a reserved zero as u32s, then minimum, maximum, multiple_of, exclusive_minimum and
    # exclusive_maximum as 16-byte decimals (the low 64 and high 32 bits of the magnitude, the
    # scale, the sign, two zero bytes) — for a gem that judges values across a C ABI of its
    # own, with #packed_enum beside it.
    def packed
      self.class.send(:pack_raw, to_h)
    end

    # The enum list as the core reads it beside #packed, "" for none (read only when the
    # flags in #packed carry HAS_ENUM, 256, set whenever an enum is declared): at a numeric
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
                        multiple_of: nil, rounding: :strict, min_length: nil, max_length: nil, trim: false,
                        case_sensitive: false, enum: nil)
      CHECKED_DOORS.fetch(door)
      ROUNDINGS.fetch(rounding)
      raise ArgumentError, "trim must be true or false; got #{trim.inspect}" unless [true, false].include?(trim)
      raise ArgumentError, "case_sensitive must be true or false; got #{case_sensitive.inspect}" unless
        [true, false].include?(case_sensitive)

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
        max_length: length("max_length", max_length), trim: trim, case_sensitive: case_sensitive, enum: entries }
    end

    # A bound, step or numeric enum entry as the core converts it: an Integer, a BigDecimal,
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
    # for none. An empty list is declared: one no value matches.
    def self.entries_of(door, enum)
      return nil if enum.nil?
      raise ArgumentError, "enum must be an Array of entries; got #{enum.inspect}" unless
        enum.respond_to?(:to_a) && !enum.is_a?(String)

      entries = enum.to_a
      if door == :text
        entries = entries.each_with_index.map do |entry, index|
          # A number is a valid entry that no String equals (JSON Schema's enum is exact
          # equality across types): kept as given, and packed empty by pack_enum.
          next entry if number_entry?(entry)

          raise ArgumentError, "enum entries at :text must be Strings, or numbers (an Integer, a BigDecimal or a " \
                               "HyperCast::Decimal), which never match; entry #{index} is #{entry.inspect}" unless
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

    # A declaration's members as the 104-byte native layout #packed describes: each bound in
    # a slot of its own.
    def self.pack_raw(members)
      flags = self::FLAGS.sum { |member, flag| members[member].nil? ? 0 : flag }
      flags |= 128 if members[:trim]
      flags |= 512 if members[:case_sensitive]
      decimals = members.values_at(:minimum, :maximum, :multiple_of, :exclusive_minimum, :exclusive_maximum)
      [self::PACKED_BYTES, flags, ROUNDINGS.fetch(members[:rounding]), members[:min_length] || 0,
       members[:max_length] || 0, 0].pack("L6") + decimals.map { |decimal| pack_decimal(decimal) }.join
    end

    # One Decimal in the core's native 16-byte layout; zero for nil.
    def self.pack_decimal(decimal)
      return [0, 0, 0, 0].pack("QLCCx2") if decimal.nil?

      magnitude = decimal.magnitude
      [magnitude & 0xFFFF_FFFF_FFFF_FFFF, magnitude >> 64, decimal.scale, decimal.negative ? 1 : 0].pack("QLCCx2")
    end

    # Whether a text enum entry is a number: valid, and never matched.
    def self.number_entry?(entry)
      entry.is_a?(Integer) || entry.is_a?(Decimal) || (defined?(BigDecimal) && entry.is_a?(BigDecimal))
    end

    # A declaration's enum as #packed_enum describes it, a binary String.
    def self.pack_enum(members)
      entries = members[:enum] or return "".b
      return entries.map { |entry| pack_decimal(entry) }.join.b unless members[:door] == :text

      # Each entry as input text crosses (HyperCast.utf8): one with no UTF-8 form is then
      # refused as an entry that is not UTF-8 (9), never transcoded with a guess.
      entries.map do |entry|
        # A number packs as an empty entry, which no input matches (empty input is the door's
        # Empty fault), so every other entry keeps its index.
        next [0].pack("L<") unless entry.is_a?(String)

        text = HyperCast.send(:utf8, entry)
        [text.bytesize].pack("L<") + text.b
      end.join.b
    end

    private_class_method :refusal, :layout, :members_of, :exact, :hint, :length, :entries_of, :number_entry?, :pack_raw,
                         :pack_decimal, :pack_enum
  end

  # The flag each declared member sets in the packed layout, HAS_ENUM (256) by an enum even
  # with no entries; TRIM (128) is set by trim and CASE_SENSITIVE (512) by case_sensitive.
  Constraints::FLAGS = { minimum: 1, exclusive_minimum: 2, maximum: 4, exclusive_maximum: 8, multiple_of: 16,
                         min_length: 32, max_length: 64, enum: 256 }.freeze

  # The size of Constraints#packed, the native RawConstraints layout, in bytes.
  Constraints::PACKED_BYTES = 104

  # The words a refused declaration's message gives, one per code: the native core's own. The
  # codes between belonged to refusals of declarations OpenAPI allows; they are retired.
  Constraints::REFUSALS = {
    1 => "a constraint the door does not take",
    5 => "multipleOf is not positive",
    6 => "a rounding without multipleOf",
    8 => "a bound, step or enum entry the door cannot hold",
    9 => "an enum entry that is not UTF-8",
    11 => "a malformed constraints layout"
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
      crossing(1, 0, text, 1) { |out| Interop.decode(:bool, out) }
    end

    # Casts char text to a one-character UTF-8 String: an input of exactly one character is
    # that character verbatim (" " is a space), otherwise one declared code point — 65,
    # U+0041, 0x41, &H41, &#65; or &#x41; — surrounded by optional white space. A code
    # point past U+10FFFF, or a surrogate, is :out_of_range.
    def char(text)
      crossing(23, 0, text, 4) { |out| Interop.decode(:char, out) }
    end

    %i[i8 i16 i32 i64 u8 u16 u32 u64].each do |door|
      unpack = Interop::SCALARS.fetch(door)
      size = Interop::VALUE_BYTES.fetch(door)
      # Resolved once here, not inside the method.
      code = DETAILED_DOORS.fetch(door)
      # Integer doors: the target type's own range, declared grouping, accounting parens,
      # non-negative exponent, and 0x/&H/0b two's-complement radix prefixes. Ruby Integer
      # is unbounded, so u64 comes back as the true unsigned value. separator:, delimiter:
      # and unit: override those fields of format for this call (NumFormat#override).
      define_method(door) do |text, format, separator: nil, delimiter: nil, unit: nil|
        format = format.override(separator: separator, delimiter: delimiter, unit: unit) if
          separator || delimiter || unit
        crossing(code, 0, text, size, format) { |out| out.unpack1(unpack) }
      end
    end

    # Casts real text to an IEEE single (widened losslessly on the way out): finite values
    # only, declared separators and grouping, parens, exponent, and trailing percent.
    # separator:, delimiter: and unit: override those fields of format for this call
    # (NumFormat#override), on this door and every other numeric one.
    def f32(text, format, separator: nil, delimiter: nil, unit: nil)
      format = format.override(separator: separator, delimiter: delimiter, unit: unit) if
        separator || delimiter || unit
      crossing(10, 0, text, 4, format) { |out| Interop.decode(:f32, out) }
    end

    # Casts real text to an IEEE double. Notation rules as f32.
    def f64(text, format, separator: nil, delimiter: nil, unit: nil)
      format = format.override(separator: separator, delimiter: delimiter, unit: unit) if
        separator || delimiter || unit
      crossing(11, 0, text, 8, format) { |out| Interop.decode(:f64, out) }
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
      crossing(19, 0, text, 16, format) { |out| Interop.decode(:decimal, out) }
    end

    # Casts UUID text — all five .NET Guid formats (D/N/B/P/X) plus urn:uuid:/GUID:/UUID:
    # prefixes — to Ruby's UUID lingua franca: the lowercase hyphenated String (the same
    # shape SecureRandom.uuid returns).
    def uuid(text)
      crossing(12, 0, text, 16) { |out| Interop.decode(:uuid, out) }
    end

    # Casts an RFC 3339 instant — zone mandatory — to a UTC Time at full nanosecond
    # fidelity across the whole 0001-9999 window. options: (a DateOptions) opens the shapes
    # RFC 3339 itself does not have, each only when asked for: SPACE_SEPARATOR a space where the
    # T goes, HOUR_OFFSET an offset of ±hh ("+00"); other options are ignored here.
    def timestamp(text, options: nil)
      dated(options, nil)
      crossing(13, 0, text, 16, nil, nil, options) { |out| Interop.decode(:timestamp, out) }
    end

    # Casts an integer Unix-epoch value under a caller-declared unit Symbol
    # (:seconds/:milliseconds/:microseconds/:nanoseconds) to a UTC Time. An unknown unit
    # is a caller bug (KeyError), never a verdict.
    def unix(text, precision)
      crossing(14, UNIX_PRECISIONS.fetch(precision), text, 16) { |out| Interop.decode(:unix, out) }
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
      crossing(22, EXCEL_EPOCHS.fetch(epoch), text, 16) { |out| Interop.decode(:excel_serial, out) }
    end

    # Casts a calendar date to a Date. With no order declared: the strict ISO 8601
    # yyyy-MM-dd form only. With a declared order Symbol (:year_month_day,
    # :month_day_year, :day_month_year), also the separated forms — "1/7/2026" is
    # January 7th or July 1st only because the caller said which; an unknown order is a
    # caller bug (KeyError), never a verdict.
    #
    # options: (a DateOptions) opens shapes only when asked for: COMPACT_DATE, "20260107", at the
    # ISO form; the pivot, two-digit years ("1/7/26"), with an order. months: (a MonthNames) opens
    # declared month names in the month field ("7-Jan-2026") with an order. Options a form does
    # not read are ignored.
    def date(text, order = nil, options: nil, months: nil)
      dated(options, months)
      if order.nil?
        crossing(15, 0, text, 4, nil, nil, options) { |out| Interop.decode(:date, out) }
      else
        crossing(20, DATE_ORDERS.fetch(order), text, 4, nil, nil, options, months) do |out|
          Interop.decode(:date_ordered, out)
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
    #
    # A 12-hour clock's AM/PM is read, before or after the clock ("3:04 PM", "PM 3:04"),
    # ASCII case-insensitively; am: and pm: declare a culture's own designators beside
    # them — "vorm."/"nachm.", "오전"/"오후", "a. m."/"p. m." — matched exactly, after the
    # clock ("3:04 nachm.") or ahead of it ("오후 3:04"), with any run of white space or none between.
    # Declared, never looked up, like a NumFormat's symbols; nil (the default) declares
    # nothing, so a call without them is exactly the call it was. A designator is 1 to 16
    # UTF-8 bytes with no ASCII digit and no white space at either end (a space inside
    # is fine); anything else is a caller bug (ArgumentError). The designators are read,
    # validated and packed per call: a call that declares them costs about twice the plain
    # call on the Magnus backend (the keyword scan itself — nil keywords pay the same) and
    # about half again here on Fiddle.
    #
    # options: (a DateOptions) gives two-digit years a pivot, and months: (a MonthNames) opens
    # declared month names ("07-Jan-26 3:04 PM"), each only when asked for.
    def datetime(text, order, am: nil, pm: nil, options: nil, months: nil)
      dated(options, months)
      periods = packed_day_periods(am, pm) if am || pm
      crossing(21, DATE_ORDERS.fetch(order), text, 16, nil, periods, options, months) do |out|
        Interop.decode(:datetime, out)
      end
    end

    # Casts an ISO 24-hour time-of-day to an exact Integer of nanoseconds since midnight
    # (Ruby has no time-of-day type; the integer keeps every digit).
    def time(text)
      crossing(16, 0, text, 8) { |out| Interop.decode(:time, out) }
    end

    # Casts a duration (ISO 8601 fixed components, invariant colon form, or protobuf JSON
    # seconds) to exact Rational seconds — full fidelity across the core's ±10,000-year
    # window, no wrapping and no truncation.
    def duration(text)
      crossing(17, 0, text, 16) { |out| Interop.decode(:duration, out) }
    end

    # Reads a number a caller already holds — an Integer, Float or Rational, the way Ruby's
    # own Float conversion takes it; anything else, a String included, is a TypeError — as
    # the exact Decimal it names: the shortest decimal that rounds back to the Float, the
    # digits a spreadsheet writes for it. 0.1 is magnitude 1, scale 1, not the binary
    # fraction nearest it; 0.1 + 0.2 is 0.30000000000000004. NaN is :malformed; an infinity,
    # a magnitude past 2**96 - 1 or more than 28 places is :out_of_range. A typed door's
    # Fault has no span: offset and length are 0.
    def decimal_from_float(value)
      typed(19, 0, float(value), 16) { |out| Interop.decode(:decimal, out) }
    end

    # Reads an Excel serial number under a declared epoch Symbol (:y1900/:y1904) as the
    # zone-less DateTime it names — the twin of excel_serial for a number a workbook reader
    # already holds, on the same carrier datetime uses. The 1900 system's phantom serial 60
    # is :out_of_range; a negative, NaN or infinite serial is :malformed.
    def excel_serial_from_float(value, epoch)
      number = float(value)
      typed(22, EXCEL_EPOCHS.fetch(epoch), number, 16) { |out| Interop.decode(:datetime, out) }
    end

    # Reads the fraction of an Excel serial number as an exact Integer of nanoseconds since
    # midnight, as time does; 0.75 and 45292.75 are both 18:00.
    def excel_time(value)
      typed(16, 0, float(value), 8) { |out| Interop.decode(:time, out) }
    end

    # Reads a number of days as exact Rational seconds, as duration does: 1.5 is 129600.
    def excel_duration(value)
      typed(17, 0, float(value), 16) { |out| Interop.decode(:duration, out) }
    end

    # Casts text as text: the input itself, as a String, once it is valid UTF-8 — a binary
    # String's bytes must be, and a UTF-8 String carrying invalid bytes is :malformed at the
    # first. Empty input is :empty; nothing is trimmed. The value is the String passed when it
    # is UTF-8 or US-ASCII, else a UTF-8 copy.
    #
    # With Constraints for :text it is judged by them, a Checked, a Fault or a Violation:
    # trimmed of Unicode White_Space when they say so (trimmed to nothing is :empty), lengths
    # counted in code points, and an enum matched under simple case folding (byte for byte
    # with case_sensitive), a match delivering the very String declared and its index; a
    # Violation's value is the judged text. Constraints for another door are an ArgumentError
    # (they go to checked).
    def text(text, constraints = nil)
      if constraints.nil?
        return crossing(18, 0, text, 8) { |out, bytes| text_value(bytes, *out.unpack("LL")) }
      end

      bytes = utf8(text)
      _door, raw, entries = constrained(constraints, :text)
      rc = checked_call(CHECKED_DOORS.fetch(:text), nil, raw, entries, bytes)
      checked_verdict(rc, bytes) do |out, check|
        offset, length = out[0, 8].unpack("LL")
        index = check && check[8, 4].unpack1("L")
        index.nil? || index == 0xFFFF_FFFF ? text_value(bytes, offset, length) : constraints.enum.fetch(index)
      end
    end

    # Casts numeric text through the door its Constraints are bound to, under the declared
    # format, then judges the value by them: a Checked, or the door's own Fault, or a
    # Violation of the first rule it breaks, carrying the value as cast. The value is judged exactly, on the decimal the
    # door read, never on a float. Spans are in the units a Fault's are. separator:,
    # delimiter: and unit: override those fields of format for this call
    # (NumFormat#override). Constraints for :text are an ArgumentError (they go to text).
    def checked(text, format, constraints, separator: nil, delimiter: nil, unit: nil)
      format = format.override(separator: separator, delimiter: delimiter, unit: unit) if
        separator || delimiter || unit
      bytes = utf8(text)
      symbol, raw, entries = constrained(constraints, :checked)
      rc = checked_call(CHECKED_DOORS.fetch(symbol), packed_cache[format], raw, entries, bytes)
      checked_verdict(rc, bytes) { |out| Interop.decode(symbol, out[0, Interop::VALUE_BYTES.fetch(symbol)]) }
    end

    # Reads a number the caller already holds — an Integer, Float or Rational, taken as
    # decimal_from_float takes it — through the door its Constraints are bound to, then judges
    # it by them on the shortest decimal that names it: 0.1 + 0.2 is 0.30000000000000004,
    # which breaks multiple_of: "0.1" under :strict and rounds to 0.3 under :midpoint. NaN is
    # :malformed; an infinity is :out_of_range at a float or decimal door and :malformed at an
    # integer door. Every fault and violation has an empty span.
    def checked_from_float(value, constraints)
      number = float(value)
      symbol, raw, entries = constrained(constraints, :checked_from_float)
      out, fault = scratch
      rc = Runtime.function(:cast_checked_detailed_from_f64).call(CHECKED_DOORS.fetch(symbol), 0, raw,
                                                                  input_ptr(entries), entries.bytesize, number,
                                                                  out, fault, checked_scratch[1])
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

    # Converts a bound, step or numeric enum entry written in invariant notation — an
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

  # The byte #kept stands a character with no UTF-8 form in for.
  NO_UTF8 = "\x80".b.freeze
  private_constant :NO_UTF8

  class << self
    private

    # Presents the input as UTF-8 bytes: already-compatible text crosses as-is (Fiddle
    # passes a String's own bytes for void* — no copy of them); only foreign encodings pay
    # a transcode. The Magnus extension calls this same method for a foreign encoding, so
    # both backends hand the core the same bytes. Text that cannot be transcoded (a lone
    # UTF-16 surrogate, bytes its encoding does not allow, a character with no Unicode
    # counterpart) is data, not a caller bug: see #kept.
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
      encoding = text.encoding
      # A US-ASCII String with a byte past 0x7F is not ASCII: it transcodes, and that byte is
      # malformed (#kept), never read as the UTF-8 it might spell.
      return text if Interop::BYTE_COMPATIBLE.include?(encoding) &&
                     (encoding != Encoding::US_ASCII || text.ascii_only?)

      begin
        text.encode(Encoding::UTF_8)
      rescue Encoding::InvalidByteSequenceError, Encoding::UndefinedConversionError,
             Encoding::ConverterNotFoundError
        kept(text)
      end
    end

    # Text that has no UTF-8 form, as the core reads it: each character transcoded, and each
    # one that cannot be (a lone surrogate, an invalid sequence, a character with no Unicode
    # counterpart) as the lone continuation byte 0x80, which no UTF-8 decoder accepts and the
    # core always judges as one malformed byte. The text is then MALFORMED at that character,
    # never a replacement standing for it, as at every binding's doors; and since 0x80 is one
    # character of the UTF-8 form, as the character it stands for is one of the caller's
    # String, a span still indexes the caller's own String.
    #
    # An encoding Ruby has no converter to UTF-8 for (EUC-TW, Windows-1258) gives no character
    # past ASCII a UTF-8 form, so each such character is one 0x80 too.
    #
    # A dummy encoding (UTF-16 and UTF-32 with their byte-order mark, ISO-2022-JP, UTF-7) has
    # no characters Ruby can split: each_char yields single bytes, and a String in one counts
    # bytes. Such text is transcoded whole (#transcoded), each conversion error standing as
    # one 0x80, so a span counts the characters of its UTF-8 form, the String
    # text.encode(Encoding::UTF_8, invalid: :replace, undef: :replace) spells, one for one.
    def kept(text)
      return transcoded(text) if text.encoding.dummy?

      text.each_char.map do |char|
        char.valid_encoding? ? char.encode(Encoding::UTF_8).b : NO_UTF8
      rescue Encoding::InvalidByteSequenceError, Encoding::UndefinedConversionError,
             Encoding::ConverterNotFoundError
        NO_UTF8
      end.join.force_encoding(Encoding::UTF_8)
    end

    # Text in a dummy encoding as UTF-8, through one converter: each invalid sequence, each
    # character with no Unicode counterpart, and an input cut short as one 0x80. A dummy
    # encoding with no converter at all (UTF-7, ISO-2022-JP-2) is one 0x80 for each byte, the
    # unit its String counts.
    def transcoded(text)
      converter = begin
        Encoding::Converter.new(text.encoding, Encoding::UTF_8)
      rescue Encoding::ConverterNotFoundError
        return (NO_UTF8 * text.bytesize).force_encoding(Encoding::UTF_8)
      end
      source = text.b
      piece = String.new
      bytes = String.new(encoding: Encoding::BINARY)
      loop do
        status = converter.primitive_convert(source, piece)
        bytes << piece.b
        piece.clear
        break if status == :finished

        bytes << NO_UTF8
      end
      bytes.force_encoding(Encoding::UTF_8)
    end

    # Fiddle spells a null pointer as nil — the core's contract for empty input.
    def input_ptr(bytes)
      bytes.empty? ? nil : bytes
    end

    # One 96-byte scratch allocation per thread, reused by every call: out-value at 0 (16 bytes
    # covers every door, and malloc's alignment covers the decimal's 8), the fault at 16 (a
    # RawFaultDetail, 16 bytes: its size word written once, here, and never by the core; then the
    # span and the detail), the datetime door's packed day periods at 32 (40 bytes, 4-aligned,
    # written only by a call that declares them) and the date options at 72 (12 bytes, 4-aligned,
    # likewise). Two Fiddle::Pointer.malloc(..., RUBY_FREE) calls per cast — each registering a GC
    # finalizer — measured as the dominant per-call cost by an order of magnitude. The NumFormat
    # does not live here: each format owns its own packed pointer (see packed_cache), so a numeric
    # call copies nothing into scratch before crossing. Allocated through Runtime.buffer, which
    # loads the library first, so a missing one is reported as such before Fiddle is touched.
    def scratch
      Thread.current[:hypercast_scratch] ||= begin
        base = Runtime.buffer(96)
        base[16, 4] = [Interop::FAULT_BYTES].pack("L")
        [base, base + 16, base + 32, base + 72]
      end
    end

    # Presents a native return code as the verdict union: 0 yields a Success, a failure
    # code becomes a Fault over `bytes` (the UTF-8 the core read) carrying the core's detail, and
    # -1 (contract violation) is a binding bug that raises.
    def verdict(rc, fault, bytes)
      if rc.zero?
        Success.new(value: yield)
      elsif rc == -1
        raise "hypercast: libhypercast reported a contract violation — a binding bug, please report it"
      else
        Interop.raw_fault(rc, fault[0, Interop::FAULT_BYTES], bytes)
      end
    end

    # The shared body of every door that reads text: one detailed native call over the scratch
    # buffers. door and param are the core's door code and what the door declares (0 for
    # nothing); format is the numeric doors' NumFormat, periods the datetime door's packed day
    # periods, options and months the date doors' DateOptions and MonthNames (nil for none), each
    # crossing only where the door reads it. The block reads the value from the out-bytes and the
    # UTF-8 the core read.
    def crossing(door, param, text, out_size, format = nil, periods = nil, options = nil, months = nil)
      bytes = utf8(text)
      out, fault, slot, dates = scratch
      if periods
        slot[0, periods.bytesize] = periods
      else
        slot = nil
      end
      if options
        dates[0, DateOptions::PACKED_BYTES] = options.packed
      else
        dates = nil
      end
      vocabulary = months&.packed
      rc = Runtime.function(:cast_detailed).call(door, param, format && packed_cache[format], slot, dates,
                                                 vocabulary, vocabulary ? vocabulary.bytesize : 0,
                                                 input_ptr(bytes), bytes.bytesize, out, fault)
      raise RefusedDeclaration, "the core refused the date options or month names" if rc == -1 && (options || months)

      verdict(rc, fault, bytes) { yield(out[0, out_size], bytes) }
    end

    # Date options and month names as a date door takes them: each nil or the right type, a
    # TypeError otherwise (what the Magnus extension raises for them too).
    def dated(options, months)
      raise TypeError, "options must be a HyperCast::DateOptions; got #{options.inspect}" unless
        options.nil? || options.is_a?(DateOptions)
      raise TypeError, "months must be a HyperCast::MonthNames; got #{months.inspect}" unless
        months.nil? || months.is_a?(MonthNames)
    end

    # The 40-byte native form of the ABI's size-tagged DayPeriods struct, which the core
    # reads in place: the size word (40, in native byte order), then each designator as its
    # UTF-8 bytes zero-padded to 16 followed by its byte length — am, then pm — and two bytes
    # of padding. A nil designator is a length of 0: nothing declared beside AM or PM.
    def packed_day_periods(am, pm)
      am = designator("am", am)
      pm = designator("pm", pm)
      [40].pack("L") + [am, am.bytesize, pm, pm.bytesize].pack("a16Ca16C") + "\0\0"
    end

    # A declared day-period designator as the UTF-8 the core reads — 1 to 16 bytes, no
    # ASCII digit, no white space at either end (the grammar around it reads that) —
    # or "" for nil. Anything else is the caller bug it names, the same ArgumentError on
    # every backend.
    def designator(name, text)
      return "" if text.nil?
      raise ArgumentError, "#{name} designator must be a String; got #{text.inspect}" unless text.is_a?(String)

      designator = declared_utf8("#{name} designator", text)
      unless designator.bytesize.between?(1, 16) &&
             !designator.match?(/[0-9]|\A#{WHITE_SPACE.source}|#{WHITE_SPACE.source}\z/)
        raise ArgumentError, "#{name} designator #{text.inspect} must be 1 to 16 UTF-8 bytes with no ASCII digit " \
                             "and no white space at either end"
      end
      designator
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

    # The shared body of the typed doors: a double instead of the text, through the core's
    # detailed typed export (door and param as #crossing's; a typed door's span is empty).
    def typed(door, param, number, out_size)
      out, fault = scratch
      rc = Runtime.function(:cast_detailed_from_f64).call(door, param, number, out, fault)
      if rc.zero?
        Success.new(value: yield(out[0, out_size]))
      elsif rc == -1
        raise "hypercast: libhypercast reported a contract violation — a binding bug, please report it"
      else
        Interop.raw_fault(rc, fault[0, Interop::FAULT_BYTES])
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

    # Identity-keyed memo of each Constraints as the core reads it: its door, its 104-byte
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

    # One 120-byte scratch allocation per thread for the checked doors, beside #scratch: a
    # packed layout constraints_check judges at 0 (104 bytes, 8-aligned by malloc), and the
    # check a checked door writes at 104 (12 bytes).
    def checked_scratch
      Thread.current[:hypercast_checked_scratch] ||= begin
        base = Runtime.buffer(Constraints::PACKED_BYTES + 16)
        [base, base + Constraints::PACKED_BYTES]
      end
    end

    # The shared body of the checked doors that read text: one detailed native call. door is
    # the core's door code, format the packed NumFormat pointer (nil at :text, which never reads
    # one), raw the packed constraints pointer and entries the packed enum.
    def checked_call(door, format, raw, entries, bytes)
      out, fault = scratch
      Runtime.function(:cast_checked_detailed).call(door, 0, format, raw, input_ptr(entries), entries.bytesize,
                                                    input_ptr(bytes), bytes.bytesize, out, fault,
                                                    checked_scratch[1])
    end

    # Presents a checked door's return code as its verdict: 0 a Checked of the value the
    # block reads from the out-value and check, 1 to 3 the door's Fault with its detail, 4 a
    # Violation of the value the block reads from the out-value (handed no check: the value as
    # cast, never an enum entry) — each span over `bytes` (the UTF-8 the core read) as a
    # Fault's is, or as written when there is no text — and -1 a binding bug that raises.
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
      return Interop.raw_fault(rc, fault[0, Interop::FAULT_BYTES], bytes) unless rc == 4

      offset, length = fault[4, 8].unpack("LL")
      offset, length = Interop.characters(bytes, offset, length) if bytes
      Violation.new(keyword: Keyword.new(code: check[0, 4].unpack1("L")), value: yield(out, nil), offset: offset,
                    length: length)
    end

    # The loaded core's version word, major << 16 | minor << 8 | patch, straight from the
    # library's zero-argument hypercast_version export.
    def packed_version
      Runtime.function(:hypercast_version).call
    end

    # NumFormat#packed as the C ABI reads it in place: its three leading words in the host's
    # native byte order (on a little-endian host, the very bytes), the symbols as they are.
    def native_packed(format)
      packed = format.packed
      packed[0, 12].unpack("L<3").pack("L3") + packed.byteslice(12, packed.bytesize - 12)
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
        pointer[0, size] = native_packed(format)
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
