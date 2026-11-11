# Replays the shared conformance corpus (corpus/*.json at the repository root) — the same
# files every other binding's suite replays — through this binding's doors. The corpus pins
# fault spans as byte offsets into the UTF-8 input; this binding presents them in the units
# String#[] slices by (character offsets on text), so each pinned span is mapped through
# `byteslice` before comparing — an identity on ASCII input, which is nearly all of it.

require "json"
require "spec_helper"

RSpec.describe "conformance corpus" do
  EXPECTED_REASON = { "empty" => :empty, "malformed" => :malformed, "out_of_range" => :out_of_range }.freeze

  def corpus(name)
    dir = __dir__
    until dir == "/"
      candidate = File.join(dir, "corpus", name)
      return JSON.parse(File.read(candidate)) if File.exist?(candidate)

      dir = File.dirname(dir)
    end
    raise "corpus directory not found"
  end

  # A row's flags and grouping policy as this binding's constructor takes them apart. A row
  # that names `grouping` declares that policy. One that doesn't declares the policy bits its raw
  # `flags` carry, so a valid policy there moves out of the mask into `grouping:`, and none is
  # Western; a bit no policy or flag names stays in `flags`, to be refused as it is.
  def grouping_of(spec)
    flags = spec["flags"]
    return [flags, spec["grouping"].to_sym] if spec.key?("grouping")

    policy = HyperCast::GROUPING_POLICIES.key(flags & HyperCast::GROUPING_POLICY)
    policy ? [flags & ~HyperCast::GROUPING_POLICY, policy] : [flags, :western]
  end

  # Every key past the separators and flags is optional in the corpus: absent means nothing
  # declared for that field, which is the Data class's own default. A row with no `grouping`
  # is Western; `flags` is the row's raw word, so a row can carry a policy no
  # name spells (a refusal).
  def format_of(vector)
    spec = vector["format"]
    return HyperCast::NumFormat::INVARIANT unless spec

    flags, grouping = grouping_of(spec)
    HyperCast::NumFormat.new(
      decimal_sep: spec["decimal_sep"], group_sep: spec["group_sep"], flags: flags, grouping: grouping,
      currency: spec.fetch("currency", ""),
      currency_decimal_sep: spec.fetch("currency_decimal_sep", ""),
      currency_group_sep: spec.fetch("currency_group_sep", ""),
      minus_sign: spec.fetch("minus_sign", ""), plus_sign: spec.fetch("plus_sign", ""),
      percent: spec.fetch("percent", ""), digit_zero: spec.fetch("digit_zero", "")
    )
  end

  def assert_verdict(domain, vector, verdict, expected)
    input = vector["input"]
    expect_label = vector["expect"]
    # The union consumption idiom in action — pattern matching over the two case types.
    case verdict
    in HyperCast::Success(value:)
      expect(expect_label).to eq("ok"), "#{domain}: #{input.inspect} unexpectedly parsed to #{value.inspect}"
      expect(value).to eq(expected), "#{domain}: #{input.inspect} -> #{value.inspect}, want #{expected.inspect}"
    in HyperCast::Fault(reason:, offset:, length:, detail:)
      expect(EXPECTED_REASON).to have_key(expect_label),
                                 "#{domain}: #{input.inspect} expected #{expect_label} but faulted"
      expect(reason).to eq(EXPECTED_REASON[expect_label]), "#{domain}: #{input.inspect} -> #{reason}"
      assert_detail(domain, vector, detail)
      if (span = vector["fault"])
        expected_span = [input.byteslice(0, span[0]).length, input.byteslice(span[0], span[1]).length]
        expect([offset, length]).to eq(expected_span), "#{domain}: #{input.inspect} fault span"
      end
    end
  end

  # A failure row names its detail: the verdict's #detail is that named Detail, and its raw
  # code is the published one.
  def assert_detail(domain, vector, detail)
    label = "#{domain}: #{vector['input'].inspect}"
    expect(vector).to have_key("detail"), "#{label} names no detail"
    expect(detail).to be_a(HyperCast::Detail), "#{label}: no detail"
    expect(detail.name.to_s).to eq(vector["detail"]), "#{label}: detail #{detail.inspect}"
    expect(detail.code).to eq(HyperCast::Detail::NAMES.key(vector["detail"].to_sym)), "#{label}: detail code"
  end

  FLAG_NAMES = {
    "compact_date" => HyperCast::DateOptions::COMPACT_DATE,
    "space_separator" => HyperCast::DateOptions::SPACE_SEPARATOR,
    "hour_offset" => HyperCast::DateOptions::HOUR_OFFSET,
    "common_forms" => HyperCast::DateOptions::COMMON_FORMS.flags
  }.freeze

  # The option bits a row declares: its named flags, or the raw `flag_bits` a refusal spells.
  def flag_bits(options)
    return options["flag_bits"] if options.key?("flag_bits")

    options.fetch("flags", []).sum(0) { |name| FLAG_NAMES.fetch(name) } # rubocop:disable Style/SymbolProc
  end

  # The DateOptions a row's `options` declare (the vocabulary aside).
  def date_options_of(options)
    HyperCast::DateOptions.new(flags: flag_bits(options), pivot: options["pivot"])
  end

  # A row's vocabulary entries, [month, name] pairs, and the packed layout they spell.
  def packed_months(options)
    packed = options["months"].map { |month, name| [month, name.bytesize].pack("CL<") + name.b }.join.b
    packed + options.fetch("append_bytes", []).pack("C*")
  end

  # The keywords a door row's `options` pass: options: always, months: when it declares names.
  # The vocabulary is built from its entries, and again from its packed layout, which must agree.
  # (The timestamp door takes no vocabulary, as the core's ignores one: its rows pass options only.)
  def optioned(vector)
    options = vector["options"] or return {}
    keywords = { options: date_options_of(options) }
    if options.key?("months")
      names = HyperCast::MonthNames.new(options["months"])
      expect(HyperCast::MonthNames.from_packed(packed_months(options))).to eq(names)
      keywords[:months] = names
    end
    keywords
  end

  def expected_instant(vector)
    return nil unless vector.key?("seconds")

    Time.at(vector["seconds"], vector["nanos"], :nanosecond, in: "UTC")
  end

  it "replays boolean.json" do
    corpus("boolean.json").each do |vector|
      assert_verdict("boolean", vector, HyperCast.bool(vector["input"]), vector["value"])
    end
  end

  # The corpus pins a char as its code-point Integer; the door returns the one-character
  # String, so the comparison goes through #ord.
  it "replays char.json" do
    corpus("char.json").each do |vector|
      verdict = HyperCast.char(vector["input"])
      verdict = HyperCast::Success.new(value: verdict.value.ord) if verdict in HyperCast::Success
      assert_verdict("char", vector, verdict, vector["value"])
    end
  end

  it "replays integer.json" do
    corpus("integer.json").each do |vector|
      door = vector.fetch("type").to_sym
      verdict = HyperCast.public_send(door, vector["input"], format_of(vector))
      assert_verdict("integer", vector, verdict, vector["value"])
    end
  end

  # The bits of a double, so zero has one form: -0.0 == 0.0, and only the bits tell them apart.
  def bits(value)
    [value].pack("G")
  end

  # The double a corpus value names at its door: an f32's rounded to single precision.
  def real(door, value)
    door == "f32" ? [value].pack("e").unpack1("e") : value.to_f
  end

  it "replays real.json, by bits" do
    corpus("real.json").each do |vector|
      door = vector.fetch("type") == "f32" ? :f32 : :f64
      verdict = HyperCast.public_send(door, vector["input"], format_of(vector))
      expected = vector["value"] && real(door.to_s, vector["value"])
      assert_verdict("real", vector, verdict, expected)
      next unless verdict in HyperCast::Success(value:)

      expect(bits(value)).to eq(bits(expected)), "real: #{vector['input'].inspect} -> #{value.inspect} bits"
    end
  end

  it "replays decimal.json" do
    corpus("decimal.json").each do |vector|
      expected = if vector.key?("magnitude")
                   HyperCast::Decimal.new(magnitude: Integer(vector["magnitude"]), scale: vector["scale"],
                                          negative: vector["negative"])
                 end
      verdict = HyperCast.decimal(vector["input"], format_of(vector))
      assert_verdict("decimal", vector, verdict, expected)
      # The canonical text is pinned too: Decimal#to_s must render exactly what every
      # other binding renders for the same triple.
      if verdict in HyperCast::Success(value:)
        expect(value.to_s).to eq(vector["value"]), "decimal: #{vector['input'].inspect} to_s"
      end
    end
  end

  it "replays uuid.json" do
    corpus("uuid.json").each do |vector|
      expected = vector["value"]&.then do |hex|
        [hex].pack("H*").unpack("H8H4H4H4H12").join("-")
      end
      assert_verdict("uuid", vector, HyperCast.uuid(vector["input"]), expected)
    end
  end

  it "replays timestamp.json" do
    corpus("timestamp.json").each do |vector|
      assert_verdict("timestamp", vector, HyperCast.timestamp(vector["input"], **optioned(vector).except(:months)),
                     expected_instant(vector))
    end
  end

  it "replays unix.json" do
    precisions = HyperCast::UNIX_PRECISIONS.invert
    corpus("unix.json").each do |vector|
      verdict = HyperCast.unix(vector["input"], precisions.fetch(vector["precision"]))
      assert_verdict("unix", vector, verdict, expected_instant(vector))
    end
  end

  it "replays excel_serial.json" do
    epochs = HyperCast::EXCEL_EPOCHS.invert
    corpus("excel_serial.json").each do |vector|
      verdict = HyperCast.excel_serial(vector["input"], epochs.fetch(vector["epoch"]))
      assert_verdict("excel_serial", vector, verdict, expected_instant(vector))
    end
  end

  it "replays date.json" do
    corpus("date.json").each do |vector|
      expected = vector.key?("year") ? Date.new(vector["year"], vector["month"], vector["day"]) : nil
      assert_verdict("date", vector, HyperCast.date(vector["input"], **optioned(vector)), expected)
    end
  end

  it "replays date_order.json" do
    orders = HyperCast::DATE_ORDERS.invert
    corpus("date_order.json").each do |vector|
      order = orders.fetch(vector["order"])
      expected = vector.key?("year") ? Date.new(vector["year"], vector["month"], vector["day"]) : nil
      assert_verdict("date_order", vector, HyperCast.date(vector["input"], order, **optioned(vector)), expected)
    end
  end

  # A vector may declare day-period designators ("am" and "pm", together or not at all);
  # without them the call is the plain two-argument one.
  it "replays datetime.json" do
    orders = HyperCast::DATE_ORDERS.invert
    corpus("datetime.json").each do |vector|
      order = orders.fetch(vector["order"])
      expected = nil
      if vector.key?("year")
        second_of_day, frac = vector["nanos_of_day"].divmod(1_000_000_000)
        hour, rest = second_of_day.divmod(3600)
        minute, second = rest.divmod(60)
        expected = DateTime.new(vector["year"], vector["month"], vector["day"],
                                hour, minute, second + Rational(frac, 1_000_000_000))
      end
      periods = vector.key?("am") ? { am: vector["am"], pm: vector["pm"] } : {}
      verdict = HyperCast.datetime(vector["input"], order, **periods, **optioned(vector))
      assert_verdict("datetime", vector, verdict, expected)
    end
  end

  it "replays time.json" do
    corpus("time.json").each do |vector|
      assert_verdict("time", vector, HyperCast.time(vector["input"]), vector["nanos"])
    end
  end

  it "replays duration.json" do
    corpus("duration.json").each do |vector|
      expected = if vector.key?("seconds")
                   Rational(vector["seconds"] * 1_000_000_000 + vector["nanos"], 1_000_000_000)
                 end
      assert_verdict("duration", vector, HyperCast.duration(vector["input"]), expected)
    end
  end

  # The typed doors, by bits: [hex].pack("H*").unpack1("G") rebuilds each double exactly —
  # NaN and the infinities included, which JSON cannot spell. A typed door's Fault has no span.
  it "replays typed.json" do
    epochs = HyperCast::EXCEL_EPOCHS.invert
    doors = corpus("typed.json").map do |vector|
      value = [vector["bits"]].pack("H*").unpack1("G")
      verdict, expected =
        case vector["door"]
        in "decimal"
          [HyperCast.decimal_from_float(value),
           vector.key?("magnitude") &&
             HyperCast::Decimal.new(magnitude: Integer(vector["magnitude"]), scale: vector["scale"],
                                    negative: vector["negative"])]
        in "excel_serial"
          expected = nil
          if vector.key?("year")
            second_of_day, frac = vector["nanos_of_day"].divmod(1_000_000_000)
            hour, rest = second_of_day.divmod(3600)
            minute, second = rest.divmod(60)
            expected = DateTime.new(vector["year"], vector["month"], vector["day"],
                                    hour, minute, second + Rational(frac, 1_000_000_000))
          end
          [HyperCast.excel_serial_from_float(value, epochs.fetch(vector["epoch"])), expected]
        in "excel_time"
          [HyperCast.excel_time(value), vector["nanos"]]
        in "excel_duration"
          [HyperCast.excel_duration(value),
           vector.key?("seconds") && Rational(vector["seconds"] * 1_000_000_000 + vector["nanos"], 1_000_000_000)]
        end
      assert_verdict("typed #{vector['door']}", vector, verdict, expected)
      case verdict
      in HyperCast::Fault(offset:, length:)
        expect([offset, length]).to eq([0, 0]), "typed #{vector['door']}: #{vector['input']} span"
      in HyperCast::Success(value: HyperCast::Decimal => decimal)
        expect(decimal.to_s).to eq(vector["value"]), "typed decimal: #{vector['input']} to_s"
      else
        nil
      end
      vector["door"]
    end
    expect(doors.uniq.sort).to eq(%w[decimal excel_duration excel_serial excel_time])
  end

  # The message a refused declaration raises, one per code: the core's Refusal, worded.
  REFUSALS = {
    1 => "a constraint the door does not take",
    5 => "multipleOf is not positive",
    6 => "a rounding without multipleOf",
    8 => "a bound, step or enum entry the door cannot hold",
    9 => "an enum entry that is not UTF-8",
    11 => "a malformed constraints layout"
  }.freeze

  def refusal(code)
    "Constraints refused (code #{code}): #{REFUSALS.fetch(code)}."
  end

  # Every keyword a vector names, whether or not its door takes it, as the public
  # constructor's keyword arguments: bounds, steps and entries as the corpus spells them.
  def declared(vector)
    c = vector["constraints"]
    {
      door: vector["door"].to_sym, minimum: c["minimum"], exclusive_minimum: c["exclusiveMinimum"],
      maximum: c["maximum"], exclusive_maximum: c["exclusiveMaximum"], multiple_of: c["multipleOf"],
      rounding: c.fetch("rounding", "strict").to_sym, min_length: c["minLength"], max_length: c["maxLength"],
      trim: c.fetch("trim", false), case_sensitive: c.fetch("caseSensitive", false), enum: c["enum"]
    }
  end

  # The doubles JSON cannot spell, as the corpus names them.
  SPELLED = { "-0" => -0.0, "NaN" => Float::NAN, "Infinity" => Float::INFINITY,
              "-Infinity" => -Float::INFINITY }.freeze

  # A checked or violating value against its row's "value": by bits (a float), by its
  # canonical text (a decimal), or as it is.
  def assert_value(label, vector, value)
    want = vector.fetch("value") { raise "#{label}: the row names no value" }
    case vector["door"]
    when "f32", "f64"
      expect(value).to be_a(Float), label
      want = SPELLED.fetch(want) if want.is_a?(String)
      expect(bits(value)).to eq(bits(real(vector["door"], want))), "#{label}: #{value.inspect} bits"
    when "decimal"
      expect(value).to be_a(HyperCast::Decimal), label
      expect(value.to_s).to eq(want), "#{label}: #{value.inspect}"
    when "text"
      expect(value).to eq(want), "#{label}: #{value.inspect}"
      expect(value.encoding).to eq(Encoding::UTF_8).or(eq(Encoding::US_ASCII)), label
    else
      expect(value).to be_a(Integer).and(eq(want)), "#{label}: #{value.inspect}"
    end
  end

  # One checked verdict against its row: the value (assert_value); adjusted and enum_index;
  # or the keyword, the value as cast and the span, or the reason and the span, which `span`
  # re-expresses in the input's own units. A text enum match is the very String the
  # declaration was given.
  def assert_checked(label, vector, verdict, entries, &span)
    expectation = vector["expect"]
    case verdict
    in HyperCast::Checked(value:, adjusted:, enum_index:)
      expect(expectation).to eq("ok"), "#{label}: unexpectedly passed as #{verdict.inspect}"
      assert_value(label, vector, value)
      expect(value).to equal(entries[enum_index]), "#{label}: not the declared entry" if enum_index
      expect(adjusted).to be(vector["adjusted"]), "#{label}: adjusted"
      expect(enum_index).to eq(vector["enum_index"]), "#{label}: enum_index"
    in HyperCast::Violation(keyword:, value:, offset:, length:)
      expect(expectation).to eq("violation"), "#{label}: #{verdict.inspect}"
      expect(vector).not_to have_key("detail"), "#{label}: a violation names no detail"
      expect(keyword.name).to eq(vector["keyword"]), "#{label}: keyword"
      assert_value("#{label} (violation)", vector, value)
      expect([offset, length]).to eq(span.call(*vector["fault"])), "#{label}: violation span"
    in HyperCast::Fault(reason:, offset:, length:, detail:)
      expect(EXPECTED_REASON).to have_key(expectation), "#{label}: expected #{expectation} but #{verdict.inspect}"
      expect(reason).to eq(EXPECTED_REASON[expectation]), "#{label}: #{reason}"
      assert_detail(label, vector, detail)
      expect([offset, length]).to eq(span.call(*vector["fault"])), "#{label}: fault span"
    end
  end

  # A corpus byte span in the units String#[] slices by on `input`: characters, as a fault's.
  def characters(input, offset, length)
    [input.byteslice(0, offset).length, input.byteslice(offset, length).length]
  end

  # Every row through the public constructor, so bounds, steps and entries cross this
  # binding's own conversion: a refused row raises ArgumentError with its code's message and
  # constraints_check answers that code. Numeric rows go through HyperCast.checked and text
  # rows through HyperCast.text, each as text (character spans) and as a binary String (byte
  # spans); input_bytes rows as a binary String and as the same bytes tagged UTF-8 (where
  # they are invalid, counted as a fault's are); from_f64 rows through checked_from_float.
  it "replays constraints.json" do
    vectors = corpus("constraints.json")
    seen = vectors.map do |vector|
      keywords = declared(vector)
      what = vector.fetch("input") { vector.fetch("from_f64") { vector["input_bytes"] } }
      label = "constraints #{vector['door']} #{vector['constraints']} #{what.inspect}"
      if vector["expect"] == "refused"
        expect { HyperCast::Constraints.new(**keywords) }
          .to raise_error(ArgumentError, refusal(vector["refusal"])), label
        expect(HyperCast.constraints_check(**keywords)).to eq(vector["refusal"]), label
        next "refused"
      end
      expect(HyperCast.constraints_check(**keywords)).to eq(0), label
      constraints = HyperCast::Constraints.new(**keywords)
      # The entries the declaration keeps (the corpus's mutable Strings, as frozen copies).
      entries = constraints.enum
      same = ->(offset, length) { [offset, length] }
      if vector.key?("from_f64")
        value = vector["from_f64"]
        value = value.is_a?(String) ? SPELLED.fetch(value) : value.to_f
        assert_checked(label, vector, HyperCast.checked_from_float(value, constraints), entries, &same)
        next "from_f64"
      end
      if vector.key?("input_bytes")
        binary = vector["input_bytes"].pack("C*")
        assert_checked(label, vector, HyperCast.text(binary, constraints), entries, &same)
        tagged = binary.dup.force_encoding(Encoding::UTF_8)
        assert_checked("#{label} (UTF-8)", vector, HyperCast.text(tagged, constraints), entries) do |o, l|
          characters(tagged, o, l)
        end
        next "input_bytes"
      end
      input = vector["input"]
      chars = ->(offset, length) { characters(input, offset, length) }
      if vector["door"] == "text"
        assert_checked(label, vector, HyperCast.text(input, constraints), entries, &chars)
        assert_checked("#{label} (binary)", vector, HyperCast.text(input.b, constraints), entries, &same)
      else
        format = format_of(vector)
        assert_checked(label, vector, HyperCast.checked(input, format, constraints), entries, &chars)
        assert_checked("#{label} (binary)", vector, HyperCast.checked(input.b, format, constraints), entries, &same)
      end
      vector["door"]
    end
    # Every row was replayed (each names what it went through): the count is the file's.
    expect(seen.compact.size).to eq(vectors.size)
    doors = HyperCast::CHECKED_DOORS.keys.map(&:to_s)
    expect(seen.uniq.sort).to eq((doors + %w[refused from_f64 input_bytes]).sort)
  end

  # The conversion every bound, step and integer enum entry crosses: declared_decimal, and a
  # Constraints maximum at the decimal door, whose packed layout carries the raw triple (scale
  # included, which a value comparison would ignore) at bytes 40 to 56. A refusal is code 8.
  it "replays declarations.json" do
    counts = Hash.new(0)
    vectors = corpus("declarations.json")
    vectors.each do |vector|
      if vector.key?("input_bytes")
        counts["bytes"] += 1
        expect([vector["expect"], vector["refusal"]]).to eq(["refused", 8])
        expect { HyperCast.declared_decimal(vector["input_bytes"].pack("C*")) }
          .to raise_error(ArgumentError, refusal(8))
        next
      end
      text = vector["input"]
      if vector["expect"] == "refused"
        counts["refused"] += 1
        expect(vector["refusal"]).to eq(8)
        expect { HyperCast.declared_decimal(text) }.to raise_error(ArgumentError, refusal(8)), text
        expect { HyperCast::Constraints.new(door: :decimal, maximum: text) }
          .to raise_error(ArgumentError, refusal(8)), text
        next
      end
      counts["ok"] += 1
      triple = [Integer(vector["magnitude"]), vector["scale"], vector["negative"]]
      lo, hi, scale, negative = HyperCast::Constraints.new(door: :decimal, maximum: text).packed[40, 16].unpack("QLCC")
      expect([(hi << 64) | lo, scale, negative == 1]).to eq(triple), text
      decimal = HyperCast.declared_decimal(text)
      expect([decimal.magnitude, decimal.scale, decimal.negative]).to eq(triple), text
      expect(decimal.to_s).to eq(vector["value"]), text
    end
    # Every row was replayed, and every kind is present: counts from the file, never literals.
    expect(counts.values.sum).to eq(vectors.size)
    expect(counts.keys.sort).to eq(%w[bytes ok refused])
  end

  # The message each rule of formats.json raises, as hypercast.rb words it and the Magnus
  # extension words the designator rule: asserted on every refused row, the ordering rows included.
  RULE_MESSAGES = {
    "length" => /exceeds \d+ UTF-8 bytes/,
    "digit" => /must not contain an ASCII digit/,
    "equal_separators" => /separators must differ/,
    "white_space" => /must not contain white space/,
    "currency_edge" => /currency symbol .* must not start or end with white space/,
    "designator_edge" => /designator .* must be 1 to 16 UTF-8 bytes .* no white space at either end/,
    "grouping_policy" => /must not carry a grouping policy/,
    "unknown_flag" => /sets a bit this version does not define/,
    "flags" => /date option flags \d+ set a bit this version does not define/,
    "pivot" => /date pivot must be nil, 0, or 100 to 9999/,
    "layout" => /month names are cut short, or bytes trail the last entry/,
    "month" => /month .* must be an Integer from 1 to 12/,
    "utf8" => /month name .* must be whole characters with a UTF-8 form/,
    "name_length" => /month name .* must be 1 to 64 UTF-8 bytes/,
    "character" => /month name .* must not contain an ASCII digit, a slash, a hyphen or white space/,
    "month_missing" => /month names must name every month/,
    "duplicate" => /two month names fold to the same spelling/
  }.freeze

  # The white_space rows hold hypercast.rb's restated class to the corpus (the Magnus backend
  # reads the core's predicate, which the Rust suite holds to the same rows); the declaration
  # rows go through the public constructor and a door, so both backends and the core's
  # own refusal agree: an accepted declaration casts, a refused one raises the rule's message.
  it "replays formats.json" do
    vectors = corpus("formats.json")
    kinds = vectors.map do |vector|
      kind = vector.fetch("kind")
      case kind
      when "white_space"
        scalar = [Integer(vector["code_point"].delete_prefix("U+"), 16)].pack("U")
        expect(HyperCast.const_get(:WHITE_SPACE).match?(scalar)).to be(vector["white_space"]), vector["code_point"]
      when "num_format"
        label = "num_format #{vector['format']}"
        if vector["expect"] == "refused"
          expect { format_of(vector) }.to raise_error(ArgumentError, RULE_MESSAGES.fetch(vector["rule"])), label
        else
          format = format_of(vector)
          expect(HyperCast.i32("1", format)).to eq(HyperCast::Success.new(value: 1)), label
          expect(format.grouping).to eq(grouping_of(vector["format"]).last), label
        end
      when "date_options"
        label = "date_options #{vector.slice('flags', 'flag_bits', 'pivot')}"
        build = -> { date_options_of(vector) }
        if vector["expect"] == "refused"
          expect(&build).to raise_error(ArgumentError, RULE_MESSAGES.fetch(vector["rule"])), label
        else
          options = build.call
          expect([options.flags, options.pivot]).to eq([flag_bits(vector), vector["pivot"]&.nonzero?]), label
          expect(options.packed.unpack("L3")).to eq([12, flag_bits(vector), vector.fetch("pivot", 0)]), label
        end
      when "month_names"
        label = "month_names #{vector['months']} #{vector['append_bytes']}"
        packed = packed_months(vector)
        from_packed = -> { HyperCast::MonthNames.from_packed(packed) }
        built = -> { HyperCast::MonthNames.new(vector["months"]) }
        if vector["expect"] == "refused"
          message = RULE_MESSAGES.fetch(vector["rule"])
          expect(&from_packed).to raise_error(ArgumentError, message), label
          expect(&built).to raise_error(ArgumentError, message), label unless vector.key?("append_bytes")
        else
          names = from_packed.call
          expect(names.packed).to eq(packed), label
          expect(built.call).to eq(names), label
        end
      when "day_periods"
        label = "day_periods #{vector['am'].inspect} #{vector['pm'].inspect}"
        door = -> { HyperCast.datetime("1/7/2026 3:04", :day_month_year, am: vector["am"], pm: vector["pm"]) }
        if vector["expect"] == "refused"
          expect(&door).to raise_error(ArgumentError, RULE_MESSAGES.fetch(vector["rule"])), label
        else
          expect(door.call).to be_a(HyperCast::Success), label
        end
      else
        raise "unknown formats.json kind #{kind.inspect}"
      end
      kind
    end
    expect(kinds.uniq.sort).to eq(%w[date_options day_periods month_names num_format white_space])
    expect(vectors.count { |vector| vector["rule"] }).to eq(vectors.count { |vector| vector["expect"] == "refused" })
  end

  # The plain doors' verdict is exactly what it was, the checked doors being additive:
  # Success(value) or Fault(reason, offset, length) over the three reasons, every plain door
  # keeps its parameters, and none answers with a checked case.
  it "keeps the plain verdict's shape" do
    expect(HyperCast::Success.members).to eq([:value])
    expect(HyperCast::Fault.members).to eq(%i[reason offset length])
    expect(HyperCast::Fault.instance_method(:initialize).parameters)
      .to eq([%i[keyreq reason], %i[keyreq offset], %i[keyreq length], %i[key detail]])
    expect(HyperCast::REASONS).to eq(1 => :empty, 2 => :malformed, 3 => :out_of_range)
    numeric = [%i[req text], %i[req format], %i[key separator], %i[key delimiter], %i[key unit]]
    plain = {
      bool: [%i[req text]], char: [%i[req text]], uuid: [%i[req text]],
      unix: [%i[req text], %i[req precision]], excel_serial: [%i[req text], %i[req epoch]],
      timestamp: [%i[req text], %i[key options]],
      date: [%i[req text], %i[opt order], %i[key options], %i[key months]],
      datetime: [%i[req text], %i[req order], %i[key am], %i[key pm], %i[key options], %i[key months]],
      time: [%i[req text]], duration: [%i[req text]],
      decimal_from_float: [%i[req value]], excel_serial_from_float: [%i[req value], %i[req epoch]],
      excel_time: [%i[req value]], excel_duration: [%i[req value]]
    }
    %i[i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 decimal].each { |door| plain[door] = numeric }
    plain.each do |door, parameters|
      expect(HyperCast::FiddleBackend.instance_method(door).parameters).to eq(parameters), door.to_s
    end
    expect(HyperCast.i32("1", HyperCast::NumFormat::INVARIANT)).to be_an_instance_of(HyperCast::Success)
    expect(HyperCast.i32("x", HyperCast::NumFormat::INVARIANT)).to be_an_instance_of(HyperCast::Fault)
    expect(HyperCast.text("x")).to eq(HyperCast::Success.new(value: "x"))
    expect(HyperCast.text("\xFF".b)).to eq(HyperCast::Fault.new(reason: :malformed, offset: 0, length: 1))
  end

  it "reads every grouping shape as each way to say a policy declares it, and refuses a bad policy" do
    holds = lambda do |want, text, format, label|
      [HyperCast.i64(text, format), HyperCast.f64(text, format), HyperCast.decimal(text, format)].each do |verdict|
        if want.key?("value")
          expect(verdict).to be_a(HyperCast::Success), "#{label} #{text}"
          expect(verdict.value.to_r).to eq(want["value"]), "#{label} #{text}"
        else
          expect(verdict).to be_a(HyperCast::Fault), "#{label} #{text}"
          expect([verdict.reason, verdict.offset, verdict.length])
            .to eq([:malformed, *want["fault"]]), "#{label} #{text}"
          expect(verdict.detail).to eq(HyperCast::Detail::GROUP_SIZE), "#{label} #{text}"
        end
      end
    end
    invariant = HyperCast::NumFormat::INVARIANT
    new_format = ->(flags, **rest) { HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: flags, **rest) }
    shapes = 0
    corpus("grouping.json").each do |row|
      case row["kind"]
      when "shape"
        shapes += 1
        text = row["input"]
        i18n = { delimiter: ",", separator: "." }
        { "invariant" => invariant, "constructor" => new_format.call(95),
          "declared" => new_format.call(95, grouping: :western),
          "indian to western" => invariant.with(grouping: :indian).with(grouping: :western),
          "from_i18n" => HyperCast::NumFormat.from_i18n(i18n) }.each do |label, format|
          holds.call(row["western"], text, format, label)
        end
        { "declared indian" => new_format.call(95, grouping: :indian),
          "western to indian" => invariant.with(grouping: :indian),
          "from_i18n indian" => HyperCast::NumFormat.from_i18n(i18n, grouping: :indian) }.each do |label, format|
          holds.call(row["indian"], text, format, label)
        end
      when "detect"
        holds.call(row, row["input"], HyperCast::NumFormat::DETECT, "detect")
      when "refused_flags"
        flags = row["flags"]
        expect { new_format.call(flags) }.to raise_error(ArgumentError, /grouping policy/)
        expect { invariant.with(flags: flags) }.to raise_error(ArgumentError, /grouping policy/)
        expect { new_format.call(95, grouping: flags) }.to raise_error(ArgumentError, /grouping must be/)
        expect { invariant.with(grouping: flags) }.to raise_error(ArgumentError, /grouping must be/)
        # Marshal's own form, as a payload holding the bits would restore it.
        members = invariant.to_h.merge(flags: flags)
        expect { HyperCast::NumFormat.allocate.marshal_load(members) }.to raise_error(ArgumentError, /grouping policy/)
      when "accepted_flags"
        flags = row["flags"]
        policy = HyperCast::GROUPING_POLICIES.key(flags & HyperCast::GROUPING_POLICY)
        made = new_format.call(flags & ~HyperCast::GROUPING_POLICY, grouping: policy)
        expect(made.raw_flags).to eq(flags)
        expect(Marshal.load(Marshal.dump(made))).to eq(made)
      else
        raise "unknown kind #{row["kind"]}"
      end
    end
    expect(shapes).to be >= 18
  end
end
