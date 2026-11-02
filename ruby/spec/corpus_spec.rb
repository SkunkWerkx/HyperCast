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

  # Every key past the separators and flags is optional in the corpus: absent means nothing
  # declared for that field, which is the Data class's own default.
  def format_of(vector)
    spec = vector["format"]
    return HyperCast::NumFormat::INVARIANT unless spec

    HyperCast::NumFormat.new(
      decimal_sep: spec["decimal_sep"], group_sep: spec["group_sep"], flags: spec["flags"],
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
    in HyperCast::Fault(reason:, offset:, length:)
      expect(EXPECTED_REASON).to have_key(expect_label),
                                 "#{domain}: #{input.inspect} expected #{expect_label} but faulted"
      expect(reason).to eq(EXPECTED_REASON[expect_label]), "#{domain}: #{input.inspect} -> #{reason}"
      if (span = vector["fault"])
        expected_span = [input.byteslice(0, span[0]).length, input.byteslice(span[0], span[1]).length]
        expect([offset, length]).to eq(expected_span), "#{domain}: #{input.inspect} fault span"
      end
    end
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
      assert_verdict("timestamp", vector, HyperCast.timestamp(vector["input"]), expected_instant(vector))
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
      assert_verdict("date", vector, HyperCast.date(vector["input"]), expected)
    end
  end

  it "replays date_order.json" do
    orders = HyperCast::DATE_ORDERS.invert
    corpus("date_order.json").each do |vector|
      order = orders.fetch(vector["order"])
      expected = vector.key?("year") ? Date.new(vector["year"], vector["month"], vector["day"]) : nil
      assert_verdict("date_order", vector, HyperCast.date(vector["input"], order), expected)
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
      verdict = if vector.key?("am")
                  HyperCast.datetime(vector["input"], order, am: vector["am"], pm: vector["pm"])
                else
                  HyperCast.datetime(vector["input"], order)
                end
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
      trim: c.fetch("trim", false), enum: c["enum"]
    }
  end

  # The doubles JSON cannot spell, as the corpus names them.
  SPELLED = { "-0" => -0.0, "NaN" => Float::NAN, "Infinity" => Float::INFINITY,
              "-Infinity" => -Float::INFINITY }.freeze

  # One checked verdict against its row: the value by bits (a float), by its canonical text
  # (a decimal), or as it is; adjusted and enum_index; or the reason or keyword and the span,
  # which `span` re-expresses in the input's own units. A text enum match is the very String
  # the declaration was given.
  def assert_checked(label, vector, verdict, entries, &span)
    expectation = vector["expect"]
    case verdict
    in HyperCast::Checked(value:, adjusted:, enum_index:)
      expect(expectation).to eq("ok"), "#{label}: unexpectedly passed as #{verdict.inspect}"
      want = vector["value"]
      case vector["door"]
      when "f32", "f64"
        expect(value).to be_a(Float), label
        expect(bits(value)).to eq(bits(real(vector["door"], want))), "#{label}: #{value.inspect} bits"
      when "decimal"
        expect(value).to be_a(HyperCast::Decimal), label
        expect(value.to_s).to eq(want), "#{label}: #{value.inspect}"
      when "text"
        expect(value).to eq(want), "#{label}: #{value.inspect}"
        expect(value.encoding).to eq(Encoding::UTF_8).or(eq(Encoding::US_ASCII)), label
        expect(value).to equal(entries[enum_index]), "#{label}: not the declared entry" if enum_index
      else
        expect(value).to be_a(Integer).and(eq(want)), "#{label}: #{value.inspect}"
      end
      expect(adjusted).to be(vector["adjusted"]), "#{label}: adjusted"
      expect(enum_index).to eq(vector["enum_index"]), "#{label}: enum_index"
    in HyperCast::Violation(keyword:, offset:, length:)
      expect(expectation).to eq("violation"), "#{label}: #{verdict.inspect}"
      expect(keyword.name).to eq(vector["keyword"]), "#{label}: keyword"
      expect([offset, length]).to eq(span.call(*vector["fault"])), "#{label}: violation span"
    in HyperCast::Fault(reason:, offset:, length:)
      expect(EXPECTED_REASON).to have_key(expectation), "#{label}: expected #{expectation} but #{verdict.inspect}"
      expect(reason).to eq(EXPECTED_REASON[expectation]), "#{label}: #{reason}"
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
    seen = corpus("constraints.json").map do |vector|
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
    doors = HyperCast::CHECKED_DOORS.keys.map(&:to_s)
    expect(seen.uniq.sort).to eq((doors + %w[refused from_f64 input_bytes]).sort)
  end

  # The conversion every bound, step and integer enum entry crosses: declared_decimal, and a
  # Constraints maximum at the decimal door, whose packed layout carries the raw triple (scale
  # included, which a value comparison would ignore) at bytes 40 to 56. A refusal is code 8.
  it "replays declarations.json" do
    counts = Hash.new(0)
    corpus("declarations.json").each do |vector|
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
    expect(counts).to eq("ok" => 33, "refused" => 30, "bytes" => 3)
  end

  # The plain doors' verdict is exactly what it was, the checked doors being additive:
  # Success(value) or Fault(reason, offset, length) over the three reasons, every plain door
  # keeps its parameters, and none answers with a checked case.
  it "keeps the plain verdict's shape" do
    expect(HyperCast::Success.members).to eq([:value])
    expect(HyperCast::Fault.members).to eq(%i[reason offset length])
    expect(HyperCast::REASONS).to eq(1 => :empty, 2 => :malformed, 3 => :out_of_range)
    numeric = [%i[req text], %i[req format], %i[key separator], %i[key delimiter], %i[key unit]]
    plain = {
      bool: [%i[req text]], char: [%i[req text]], uuid: [%i[req text]], timestamp: [%i[req text]],
      unix: [%i[req text], %i[req precision]], excel_serial: [%i[req text], %i[req epoch]],
      date: [%i[req text], %i[opt order]],
      datetime: [%i[req text], %i[req order], %i[key am], %i[key pm]],
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
end
