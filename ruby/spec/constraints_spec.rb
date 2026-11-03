# The validation constraints' binding-level contracts the corpus can't express: what a
# declaration accepts and refuses as a caller bug, the door it is bound to, the String a text
# match delivers, and the Keyword's open shape. The corpus replay (corpus_spec.rb) holds the
# judgements themselves to every other binding's.

require "bigdecimal"
require "spec_helper"

RSpec.describe HyperCast::Constraints do
  invariant = HyperCast::NumFormat::INVARIANT

  it "requires a door, so no declaration silently means no rules" do
    expect { described_class.new }.to raise_error(ArgumentError, /missing keyword: :door/)
    expect { described_class.new(minimum: "1") }.to raise_error(ArgumentError, /missing keyword: :door/)
    expect { described_class.new(door: :bool) }.to raise_error(KeyError, /:bool/)
    expect { described_class.new(door: "f64") }.to raise_error(KeyError)
    # Declared out loud, no rules is a declaration of its own.
    expect(HyperCast.checked("5", invariant, described_class.new(door: :f64)))
      .to eq(HyperCast::Checked.new(value: 5.0, adjusted: false, enum_index: nil))
    # One never built has no door, and is refused rather than read as no rules.
    expect { HyperCast.checked("5", invariant, described_class.allocate) }
      .to raise_error(ArgumentError, /was never built/)
  end

  it "takes a bound as an Integer, a BigDecimal, a String or a Decimal, never a Float or a boolean" do
    expected = described_class.new(door: :decimal, minimum: "0.05")
    [5, BigDecimal("0.05"), "5e-2", HyperCast::Decimal.new(magnitude: 5, scale: 2, negative: false)]
      .each_with_index do |bound, index|
        declared = described_class.new(door: :decimal, minimum: bound)
        expect(declared).to eq(expected) unless index.zero?
        expect(declared.minimum).to be_a(HyperCast::Decimal)
      end
    expect(described_class.new(door: :i64, maximum: 2**63 - 1).maximum.magnitude).to eq(2**63 - 1)
    expect { described_class.new(door: :f64, multiple_of: 0.1) }
      .to raise_error(ArgumentError, /multiple_of must be .* got 0\.1 \(a Float holds most decimals only approximately/)
    [true, false, Rational(1, 2), :one].each do |bound|
      expect { described_class.new(door: :f64, minimum: bound) }
        .to raise_error(ArgumentError, /minimum must be an Integer, a BigDecimal, a String or a HyperCast::Decimal/)
    end
    expect { described_class.new(door: :i32, enum: [1, 2.0]) }.to raise_error(ArgumentError, /enum entry must be/)
    # A bound the core cannot convert is the refusal the core names.
    expect { described_class.new(door: :f64, minimum: "one") }
      .to raise_error(ArgumentError, "Constraints refused (code 8): a bound, step or enum entry the door cannot hold.")
    expect { described_class.new(door: :f64, minimum: BigDecimal("NaN")) }.to raise_error(ArgumentError, /code 8/)
  end

  it "takes a length as an Integer the core can count in, never a boolean" do
    [true, false, -1, 2**32, 1.0, "3"].each do |length|
      expect { described_class.new(door: :text, max_length: length) }
        .to raise_error(ArgumentError, /max_length must be an Integer from 0 to 4294967295/)
    end
    expect(described_class.new(door: :text, min_length: 0, max_length: 2**32 - 1).max_length).to eq(2**32 - 1)
    expect { described_class.new(door: :text, trim: nil) }.to raise_error(ArgumentError, /trim must be true or false/)
  end

  it "declares the rounding by Symbol" do
    expect(described_class.new(door: :f64, multiple_of: "0.1", rounding: :midpoint).rounding).to eq(:midpoint)
    expect(described_class.new(door: :f64).rounding).to eq(:strict)
    [1, "midpoint", :nearest].each do |rounding|
      expect { described_class.new(door: :f64, multiple_of: "0.1", rounding: rounding) }.to raise_error(KeyError)
    end
  end

  it "takes an enum as a list, nil declaring none and an empty one refused" do
    expect(described_class.new(door: :text, enum: nil).enum).to be_nil
    expect(described_class.new(door: :text, enum: nil).packed_enum).to eq("".b)
    expect(described_class.new(door: :text).packed.unpack1("@4L")).to eq(0)
    expect(described_class.new(door: :text, enum: %w[a]).packed.unpack1("@4L")).to eq(256)
    empty = "Constraints refused (code 12): an enum with no entries."
    expect { described_class.new(door: :text, enum: []) }.to raise_error(ArgumentError, empty)
    expect { described_class.new(door: :i32, enum: []) }.to raise_error(ArgumentError, empty)
    expect(HyperCast.constraints_check(door: :text, trim: true, enum: [])).to eq(12)
    # An enum at a door that takes none is refused for that first.
    expect(HyperCast.constraints_check(door: :f64, enum: [])).to eq(1)
    expect { described_class.new(door: :text, enum: "Male") }.to raise_error(ArgumentError, /enum must be an Array/)
    expect { described_class.new(door: :text, enum: ["Male", :female]) }
      .to raise_error(ArgumentError, /enum entries at :text must be Strings; entry 1 is :female/)
    expect(described_class.new(door: :i32, enum: [7, "8", BigDecimal("9")]).enum.map(&:to_s)).to eq(%w[7 8 9])
    expect(described_class.new(door: :text, enum: %w[a b]).enum).to be_frozen
  end

  it "hands the core a text entry's bytes, so one that is not UTF-8 is the core's refusal" do
    expect { described_class.new(door: :text, enum: ["ok", "\xFF".b]) }
      .to raise_error(ArgumentError, "Constraints refused (code 9): an enum entry that is not UTF-8.")
    expect(HyperCast.constraints_check(door: :text, enum: ["\xC3".dup.force_encoding(Encoding::UTF_8)])).to eq(9)
    # Another encoding is transcoded first, as input text is.
    latin = "Mädchen".encode(Encoding::ISO_8859_1).freeze
    declared = described_class.new(door: :text, enum: [latin])
    expect(HyperCast.text("MÄDCHEN", declared).value).to equal(latin)
  end

  it "delivers the very String declared on a text enum match, and its index" do
    female = "Female".freeze
    sex = described_class.new(door: :text, trim: true, enum: ["Male", female, "Other"])
    verdict = HyperCast.text("  female ", sex)
    expect(verdict).to eq(HyperCast::Checked.new(value: "Female", adjusted: false, enum_index: 1))
    expect(verdict.value).to equal(female)
    expect(HyperCast.text("Femal", sex))
      .to eq(HyperCast::Violation.new(keyword: HyperCast::Keyword::ENUM, offset: 0, length: 5))
    expect(HyperCast.text(" \t ", sex)).to eq(HyperCast::Fault.new(reason: :empty, offset: 0, length: 0))
    expect(HyperCast.optional(HyperCast.text(" \t ", sex))).to be_nil
  end

  it "keeps a mutable text entry as a frozen copy, so mutating it later changes nothing" do
    female = +"Female"
    sex = described_class.new(door: :text, enum: ["Male", female])
    female.replace("Fem")
    expect(sex.enum).to eq(%w[Male Female]).and(all(be_frozen))
    verdict = HyperCast.text("FEMALE", sex)
    expect(verdict).to eq(HyperCast::Checked.new(value: "Female", adjusted: false, enum_index: 1))
    expect(verdict.value).to be_frozen
    expect(verdict.value).not_to equal(female)
    expect(HyperCast.text("Fem", sex))
      .to eq(HyperCast::Violation.new(keyword: HyperCast::Keyword::ENUM, offset: 0, length: 3))
  end

  it "judges a declaration Marshal restores as Constraints.new does" do
    latitude = described_class.new(door: :f64, minimum: "-90", maximum: "90")
    restored = Marshal.load(Marshal.dump(latitude))
    expect(restored).to eq(latitude).and(be_frozen)
    expect(HyperCast.checked("91", invariant, restored).keyword).to eq(HyperCast::Keyword::MAXIMUM)
    five = HyperCast::Decimal.new(magnitude: 5, scale: 0, negative: false)
    forged = latitude.to_h.merge(door: :i32, minimum: five, maximum: five)
    refusal = "Constraints refused (code 4): the lower bound is not below the upper bound."
    # Payloads forged by dumping a stand-in class and renaming it (same length, so every
    # Marshal offset holds). Marshal's form for a Constraints is its members, loaded through
    # #initialize.
    rename = ->(dump) { dump.sub("HyperCast::Constraintz", "HyperCast::Constraints") }
    user = Class.new { define_method(:marshal_dump) { forged } }
    stub_const("HyperCast::Constraintz", user)
    expect { Marshal.load(rename.call(Marshal.dump(user.new))) }.to raise_error(ArgumentError, refusal)
    # A payload in Data's Struct form sets the members without #initialize; it is judged at
    # its first use instead, on every checked door.
    struct = Data.define(*described_class.members)
    stub_const("HyperCast::Constraintz", struct)
    payload = rename.call(Marshal.dump(struct.new(**forged)))
    smuggled = Marshal.load(payload)
    expect(smuggled.minimum).to eq(five)
    expect { HyperCast.checked("5", invariant, smuggled) }.to raise_error(ArgumentError, refusal)
    expect { HyperCast.checked_from_float(5, smuggled) }.to raise_error(ArgumentError, refusal)
  end

  it "returns the text passed, or a UTF-8 String of what was judged" do
    text = +"héllo"
    expect(HyperCast.text(text).value).to equal(text)
    expect(HyperCast.text(text, described_class.new(door: :text)).value).to equal(text)
    binary = HyperCast.text("héllo".b).value
    expect([binary, binary.encoding]).to eq(["héllo", Encoding::UTF_8])
    trimmed = HyperCast.text(" héllo ", described_class.new(door: :text, trim: true)).value
    expect([trimmed, trimmed.encoding]).to eq(["héllo", Encoding::UTF_8])
    utf16 = HyperCast.text("héllo".encode(Encoding::UTF_16LE)).value
    expect([utf16, utf16.encoding]).to eq(["héllo", Encoding::UTF_8])
    expect(HyperCast.text("")).to eq(HyperCast::Fault.new(reason: :empty, offset: 0, length: 0))
    # Spans are in characters, as a fault's: "é" is one.
    expect(HyperCast.text("ab\xFFc".dup.force_encoding(Encoding::UTF_8)))
      .to eq(HyperCast::Fault.new(reason: :malformed, offset: 2, length: 1))
    expect(HyperCast.text("éé", described_class.new(door: :text, max_length: 1)))
      .to eq(HyperCast::Violation.new(keyword: HyperCast::Keyword::MAX_LENGTH, offset: 0, length: 2))
    expect(HyperCast.checked("€5", HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",",
                                                            flags: HyperCast::ALL_STYLES, currency: "€"),
                             described_class.new(door: :i32, maximum: 4)))
      .to eq(HyperCast::Violation.new(keyword: HyperCast::Keyword::MAXIMUM, offset: 0, length: 2))
  end

  it "is judged only at the door it is bound to" do
    text = described_class.new(door: :text, max_length: 3)
    number = described_class.new(door: :i32, maximum: 3)
    expect { HyperCast.checked("1", invariant, text) }
      .to raise_error(ArgumentError, "Constraints for :text are judged by HyperCast.text")
    expect { HyperCast.checked_from_float(1, text) }
      .to raise_error(ArgumentError, "Constraints for :text are judged by HyperCast.text")
    expect { HyperCast.text("1", number) }
      .to raise_error(ArgumentError, "Constraints for :i32 are judged by HyperCast.checked")
    expect { HyperCast.checked("1", invariant, :i32) }
      .to raise_error(TypeError, "constraints must be a HyperCast::Constraints; got :i32")
    expect { HyperCast.text("1", 3) }.to raise_error(TypeError, "constraints must be a HyperCast::Constraints; got 3")
  end

  it "declares each value the door delivers" do
    expect(HyperCast.checked("200", invariant, described_class.new(door: :u8, minimum: 1)))
      .to eq(HyperCast::Checked.new(value: 200, adjusted: false, enum_index: nil))
    expect(HyperCast.checked("2.5", invariant, described_class.new(door: :f32, maximum: 3)).value).to eq(2.5)
    expect(HyperCast.checked("1.10", invariant, described_class.new(door: :decimal, maximum: 3)).value)
      .to eq(HyperCast::Decimal.new(magnitude: 11, scale: 1, negative: false))
    expect(HyperCast.checked("18446744073709551615", invariant, described_class.new(door: :u64)).value)
      .to eq(2**64 - 1)
    # An integer enum delivers the value, with no index.
    expect(HyperCast.checked("7", invariant, described_class.new(door: :i32, enum: [7, 9])))
      .to eq(HyperCast::Checked.new(value: 7, adjusted: false, enum_index: nil))
    # The numeric doors' format overrides apply here too.
    expect(HyperCast.checked("1.234,5", invariant, described_class.new(door: :f64, maximum: 2000),
                             separator: ",", delimiter: ".").value).to eq(1234.5)
  end

  it "judges a number already held on its shortest decimal, zero having one form" do
    step = described_class.new(door: :f64, multiple_of: "0.1")
    expect(HyperCast.checked_from_float(0.1 + 0.2, step))
      .to eq(HyperCast::Violation.new(keyword: HyperCast::Keyword::MULTIPLE_OF, offset: 0, length: 0))
    rounded = HyperCast.checked_from_float(0.1 + 0.2, step.with(rounding: :midpoint))
    expect(rounded).to eq(HyperCast::Checked.new(value: 0.3, adjusted: true, enum_index: nil))
    zero = HyperCast.checked_from_float(-0.0, step)
    expect([zero.value].pack("G")).to eq([0.0].pack("G"))
    expect(zero.adjusted).to be(false)
    expect(HyperCast.checked_from_float(Float::NAN, step)).to eq(HyperCast::Fault.new(reason: :malformed, offset: 0,
                                                                                      length: 0))
    expect { HyperCast.checked_from_float("0.3", step) }.to raise_error(TypeError)
  end

  it "is an immutable value, revalidated by #with" do
    latitude = described_class.new(door: :f64, minimum: "-90", maximum: "90")
    expect(latitude).to be_frozen
    expect(latitude).to eq(described_class.new(door: :f64, minimum: -90, maximum: BigDecimal("90")))
    expect(latitude.with(maximum: "45").maximum.to_s).to eq("45")
    expect { latitude.with(exclusive_maximum: "45") }
      .to raise_error(ArgumentError, "Constraints refused (code 3): maximum together with exclusiveMaximum.")
    expect(latitude.packed.bytesize).to eq(72)
    expect(latitude.packed.unpack("L6")).to eq([72, 1 | 4, 0, 0, 0, 0])
  end

  it "refuses a text entry no input can match" do
    unmatchable = "Constraints refused (code 13): a text enum entry no input can match."
    expect { described_class.new(door: :text, enum: ["Male", ""]) }.to raise_error(ArgumentError, unmatchable)
    expect { described_class.new(door: :text, trim: true, enum: [" Male "]) }
      .to raise_error(ArgumentError, unmatchable)
    # Untrimmed, edge white space is part of the entry, and matches as declared.
    spaced = described_class.new(door: :text, enum: [" Male "])
    expect(HyperCast.text(" MALE ", spaced))
      .to eq(HyperCast::Checked.new(value: " Male ", adjusted: false, enum_index: 0))
  end

  it "words a refusal code it does not name as every binding does, still an ArgumentError" do
    expect(described_class.send(:refusal, 14))
      .to eq("Constraints refused (code 14): a rule this binding does not name.")
    expect(described_class.send(:refusal, 13))
      .to eq("Constraints refused (code 13): a text enum entry no input can match.")
  end

  it "answers a refusal's code from constraints_check, never raising for one" do
    expect(HyperCast.constraints_check(door: :i32, minimum: 5, maximum: 5)).to eq(4)
    expect(HyperCast.constraints_check(door: :i32, minimum: 5)).to eq(0)
    expect(HyperCast.constraints_check(door: :i32, minimum: "x")).to eq(8)
    expect { HyperCast.constraints_check(door: :i32, minimum: 0.5) }.to raise_error(ArgumentError)
    expect { HyperCast.constraints_check }.to raise_error(ArgumentError, /missing keyword: :door/)
  end

  it "converts declarations through the core" do
    expect(HyperCast.declared_decimal("0.000001")).to eq(HyperCast::Decimal.new(magnitude: 1, scale: 6,
                                                                                negative: false))
    expect(HyperCast.declared_decimal("-1.50")).to eq(HyperCast::Decimal.new(magnitude: 15, scale: 1, negative: true))
    expect { HyperCast.declared_decimal("1,5") }.to raise_error(ArgumentError, /code 8/)
    expect { HyperCast.declared_decimal(15) }.to raise_error(TypeError)
  end
end

RSpec.describe HyperCast::Keyword do
  it "names each keyword the OpenAPI way and keeps a code it does not name" do
    expect(HyperCast::Keyword::NAMES.values)
      .to eq(%w[minimum exclusiveMinimum maximum exclusiveMaximum multipleOf minLength maxLength enum])
    expect(HyperCast::Keyword::EXCLUSIVE_MINIMUM.name).to eq("exclusiveMinimum")
    expect(HyperCast::Keyword::ENUM.to_s).to eq("enum")
    expect(HyperCast::Keyword::MINIMUM.inspect).to eq("HyperCast::Keyword::MINIMUM")
    expect(described_class.new(code: 1)).to eq(HyperCast::Keyword::MINIMUM)
    future = described_class.new(code: 9)
    expect(future.name).to eq("keyword 9")
    expect(future.inspect).to eq("#<data HyperCast::Keyword code=9>")
  end

  it "matches in a case with an else branch" do
    describe = lambda do |verdict|
      case verdict
      in HyperCast::Checked(value:) then "ok #{value}"
      in HyperCast::Violation(keyword: HyperCast::Keyword::MAXIMUM) then "too far"
      in HyperCast::Violation(keyword:) then "breaks #{keyword}"
      in HyperCast::Fault(reason:) then reason.to_s
      end
    end
    latitude = HyperCast::Constraints.new(door: :f64, minimum: "-90", maximum: "90")
    expect(describe.call(HyperCast.checked("91", HyperCast::NumFormat::INVARIANT, latitude))).to eq("too far")
    expect(describe.call(HyperCast.checked("-91", HyperCast::NumFormat::INVARIANT, latitude)))
      .to eq("breaks minimum")
    expect(describe.call(HyperCast.checked("x", HyperCast::NumFormat::INVARIANT, latitude))).to eq("malformed")
    expect(describe.call(HyperCast::Violation.new(keyword: described_class.new(code: 9), offset: 0, length: 0)))
      .to eq("breaks keyword 9")
  end
end
