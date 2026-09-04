# Binding-level behavior the corpus can't express: pattern-matching consumption,
# Ruby-flavored fidelity (unbounded Integer, nanosecond Time, exact Rational durations),
# and the caller-bug guards.

require "spec_helper"
require "open3"

RSpec.describe HyperCast do
  invariant = HyperCast::NumFormat::INVARIANT

  it "consumes verdicts with pattern matching" do
    rendered = case described_class.i32("(1,234)", invariant)
               in HyperCast::Success(value:) then "ok #{value}"
               in HyperCast::Fault(reason:, offset:) then "#{reason} at #{offset}"
               end
    expect(rendered).to eq("ok -1234")
  end

  it "points the fault span at the offending byte" do
    expect(described_class.i32("  12x4", invariant))
      .to eq(HyperCast::Fault.new(reason: :malformed, offset: 4, length: 1))
  end

  it "reports the fault span in the units String#[] slices by" do
    # Text: character offsets, so text[offset, length] is the offending text as passed.
    expect(described_class.i32("€x", invariant)).to eq(HyperCast::Fault.new(reason: :malformed, offset: 0, length: 1))
    expect(described_class.i32("1€", invariant)).to eq(HyperCast::Fault.new(reason: :malformed, offset: 1, length: 1))
    expect("1€"[1, 1]).to eq("€")
    # The same bytes as a binary String: byte offsets, because its characters are its bytes.
    expect(described_class.i32("€x".b, invariant)).to eq(HyperCast::Fault.new(reason: :malformed, offset: 0, length: 3))
    expect(described_class.i32("1€".b, invariant)).to eq(HyperCast::Fault.new(reason: :malformed, offset: 1, length: 3))
    expect("1€".b[1, 3]).to eq("€".b)
    # A foreign encoding is transcoded for the core, and the span still indexes the String
    # the caller passed — character counts survive transcoding.
    utf16 = "1€x".encode(Encoding::UTF_16LE)
    expect(described_class.i32(utf16, invariant)).to eq(HyperCast::Fault.new(reason: :malformed, offset: 1, length: 1))
    # ASCII text is the identity either way.
    expect(described_class.i32("  12x4".b, invariant)).to eq(HyperCast::Fault.new(reason: :malformed, offset: 4, length: 1))
  end

  it "honors declared separators" do
    eurozone = HyperCast::NumFormat.new(decimal_sep: ",", group_sep: ".", flags: HyperCast::ALL_STYLES)
    expect(described_class.f64("1.234,5", eurozone)).to eq(HyperCast::Success.new(value: 1234.5))
  end

  it "honors a declared currency symbol at either edge, under the CURRENCY flag" do
    dollars = HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: HyperCast::ALL_STYLES,
                                       currency: "$")
    expect(described_class.i32("$1,234", dollars)).to eq(HyperCast::Success.new(value: 1234))
    expect(described_class.i32("-$5", dollars)).to eq(HyperCast::Success.new(value: -5))
    expect(described_class.i32("($5)", dollars)).to eq(HyperCast::Success.new(value: -5))
    expect(described_class.f64("$ -2.5", dollars)).to eq(HyperCast::Success.new(value: -2.5))
    expect(described_class.decimal("($1,234.50)", dollars))
      .to eq(HyperCast::Success.new(value: HyperCast::Decimal.new(magnitude: 12_345, scale: 1, negative: true)))
    # A trailing, multi-byte symbol under eurozone separators.
    kroner = HyperCast::NumFormat.new(decimal_sep: ",", group_sep: ".", flags: HyperCast::ALL_STYLES,
                                      currency: "kr.")
    expect(described_class.f64("1.234,50 kr.", kroner)).to eq(HyperCast::Success.new(value: 1234.5))
    # Declared but with the flag off: the symbol is malformed input, at the symbol.
    declared_off = HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",",
                                            flags: HyperCast::ALL_STYLES & ~HyperCast::CURRENCY, currency: "$")
    expect(described_class.i32("$5", declared_off)).to eq(HyperCast::Fault.new(reason: :malformed, offset: 0, length: 1))
    # The flag with nothing declared (INVARIANT) is a no-op: a symbol is just malformed input.
    expect(described_class.i32("$5", invariant)).to eq(HyperCast::Fault.new(reason: :malformed, offset: 0, length: 1))
  end

  it "casts decimals exactly and canonically, never rounding" do
    # Exact trailing zeros in the fraction are always trimmed: the scale is minimal.
    canonical = HyperCast::Success.new(value: HyperCast::Decimal.new(magnitude: 11, scale: 1, negative: false))
    expect(described_class.decimal("1.10", invariant)).to eq(canonical)
    expect(described_class.decimal("1.1", invariant)).to eq(canonical)
    expect(described_class.decimal("1.1000", invariant)).to eq(canonical)
    expect(described_class.decimal("1.0000", invariant))
      .to eq(HyperCast::Success.new(value: HyperCast::Decimal.new(magnitude: 1, scale: 0, negative: false)))
    # Only fraction zeros are shed: an integer's trailing zeros are significant.
    expect(described_class.decimal("100", invariant))
      .to eq(HyperCast::Success.new(value: HyperCast::Decimal.new(magnitude: 100, scale: 0, negative: false)))
    expect(described_class.decimal("0.1", invariant).value.to_r).to eq(Rational(1, 10))
    expect(described_class.decimal("50%", invariant))
      .to eq(HyperCast::Success.new(value: HyperCast::Decimal.new(magnitude: 5, scale: 1, negative: false)))
    expect(described_class.decimal("50%", invariant).value.to_s).to eq("0.5")
    # Zero is scale 0 and never negative, whatever the text said.
    expect(described_class.decimal("-0.00", invariant))
      .to eq(HyperCast::Success.new(value: HyperCast::Decimal.new(magnitude: 0, scale: 0, negative: false)))
    expect(described_class.decimal("-0.00", invariant).value.to_s).to eq("0")
    # The 96-bit ceiling is a range, not a rounding opportunity.
    expect(described_class.decimal("79228162514264337593543950335", invariant).value.magnitude).to eq(2**96 - 1)
    expect(described_class.decimal("79228162514264337593543950336", invariant)).to be_a(HyperCast::Fault)
    expect(described_class.decimal("79228162514264337593543950336", invariant).reason).to eq(:out_of_range)
  end

  it "converts a Decimal to Rational, canonical text and BigDecimal" do
    value = described_class.decimal("-1,234.50", invariant).value
    expect(value).to be_negative
    expect(value.to_r).to eq(Rational(-12_345, 10))
    expect(value.to_s).to eq("-1234.5")
    expect(value.to_d).to eq(BigDecimal("-1234.5"))
    expect(value.to_d).to be_a(BigDecimal)
  end

  it "reports the loaded core's version, pinned to this gem's own" do
    # VERSION and rust/Cargo.toml are swept together by prepare-release.yml, so the loaded
    # core must always report exactly this gem's version.
    expect(described_class.native_version).to match(/\A\d+\.\d+\.\d+\z/)
    expect(described_class.native_version).to eq(HyperCast::VERSION)
  end

  it "answers available? without raising, and consistently with native_version" do
    expect(described_class.available?).to be(true)
    expect(described_class.available?).to be(true) # cached
    expect(described_class.native_version).to eq(HyperCast::VERSION)
  end

  it "answers available? false without raising when no library resolves, while the doors still raise" do
    # A Fiddle subprocess with the library path stubbed away: the probe answers quietly,
    # the first door call keeps its precise LoadError.
    lib = File.expand_path("../lib", __dir__)
    script = "HyperCast::Runtime.singleton_class.define_method(:library_path) { nil }; " \
             "print HyperCast.available?; print ' '; " \
             "begin; HyperCast.i32('1', HyperCast::NumFormat::INVARIANT); rescue LoadError; print 'raised'; end"
    out, status = Open3.capture2({ "HYPERCAST_PURE" => "1", "HYPERCAST_WASM" => nil },
                                 RbConfig.ruby, "-I", lib, "-r", "hypercast", "-e", script)
    expect(status).to be_success
    expect(out).to eq("false raised")
  end

  it "returns u64 as the true unsigned value — Integer is unbounded" do
    expect(described_class.u64("18446744073709551615", invariant))
      .to eq(HyperCast::Success.new(value: 2**64 - 1))
  end

  it "agrees with SecureRandom.uuid's canonical shape" do
    text = "01020304-0506-0708-090A-0B0C0D0E0F10"
    expect(described_class.uuid("urn:uuid:#{text}"))
      .to eq(HyperCast::Success.new(value: text.downcase))
  end

  it "keeps full nanosecond fidelity on Time" do
    expect(described_class.timestamp("2026-01-02T15:04:05.123456789+05:00"))
      .to eq(HyperCast::Success.new(value: Time.at(1_767_348_245, 123_456_789, :nanosecond, in: "UTC")))
  end

  it "maps the declared unix precision" do
    expect(described_class.unix("-1", :seconds))
      .to eq(HyperCast::Success.new(value: Time.at(-1, in: "UTC")))
    expect { described_class.unix("1", :fortnights) }.to raise_error(KeyError)
  end

  it "disambiguates separated dates by declared order, never by guessing" do
    # 1/7/2026 is January 7th under en-US's month-first short dates and July 1st under
    # en-GB's day-first ones — resolved only by what the caller declared.
    expect(described_class.date("1/7/2026", :month_day_year))
      .to eq(HyperCast::Success.new(value: Date.new(2026, 1, 7)))
    expect(described_class.date("1/7/2026", :day_month_year))
      .to eq(HyperCast::Success.new(value: Date.new(2026, 7, 1)))
    # Undeclared, the door stays strict ISO — the ambiguity is never guessed at.
    expect(described_class.date("1/7/2026"))
      .to eq(HyperCast::Fault.new(reason: :malformed, offset: 0, length: 8))
    expect { described_class.date("1/7/2026", :little_endian) }.to raise_error(KeyError)
  end

  it "reads messy civil date-times without inventing a zone" do
    # The AM/PM world: a zone-less text names no instant, so the value comes back as a
    # stdlib DateTime (exact Rational seconds), never a zone-guessed Time.
    expect(described_class.datetime("1/7/2026 3:04 PM", :month_day_year))
      .to eq(HyperCast::Success.new(value: DateTime.new(2026, 1, 7, 15, 4, 0)))
    expect(described_class.datetime("1/7/2026 3:04 PM", :day_month_year))
      .to eq(HyperCast::Success.new(value: DateTime.new(2026, 7, 1, 15, 4, 0)))
    expect(described_class.datetime("2026-01-07T15:04:05.123456789", :month_day_year))
      .to eq(HyperCast::Success.new(value: DateTime.new(2026, 1, 7, 15, 4,
                                                        5 + Rational(123_456_789, 1_000_000_000))))
    # A zone suffix is not this door's business — timestamp is the instant door.
    expect(described_class.datetime("1/7/2026 15:04:05Z", :month_day_year))
      .to be_a(HyperCast::Fault)
  end

  it "returns durations as exact Rational seconds across the full window" do
    expect(described_class.duration("PT1.5S")).to eq(HyperCast::Success.new(value: Rational(3, 2)))
    expect(described_class.duration("-1.5s")).to eq(HyperCast::Success.new(value: Rational(-3, 2)))
    # The core's ±10,000-year window fits Rational exactly — no ceiling, unlike Go.
    expect(described_class.duration("315576000000s"))
      .to eq(HyperCast::Success.new(value: Rational(315_576_000_000)))
  end

  it "presents empty as nil through optional" do
    expect(described_class.optional(described_class.i32("   ", invariant))).to be_nil
    expect(described_class.optional(described_class.i32("42", invariant)))
      .to eq(HyperCast::Success.new(value: 42))
  end

  it "treats equal separators as a caller bug" do
    expect { HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ".", flags: HyperCast::ALL_STYLES) }
      .to raise_error(ArgumentError)
  end

  it "treats an invalid currency symbol as a caller bug" do
    build = ->(currency) { HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: HyperCast::ALL_STYLES, currency: currency) }
    expect { build.call("$1") }.to raise_error(ArgumentError)       # an ASCII digit
    expect { build.call("US $") }.to raise_error(ArgumentError)     # ASCII whitespace
    expect { build.call("€" * 6) }.to raise_error(ArgumentError)    # 18 UTF-8 bytes
    expect { build.call(:usd) }.to raise_error(ArgumentError)       # not a String
    expect(build.call("€" * 5).currency).to eq("€" * 5)             # 15 bytes: fine
    expect(build.call("").currency).to eq("")                       # none declared
  end
end
