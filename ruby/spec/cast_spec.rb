# Binding-level behavior the corpus can't express: pattern-matching consumption,
# Ruby-flavored fidelity (unbounded Integer, nanosecond Time, exact Rational durations),
# and the caller-bug guards.

require "spec_helper"

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

  it "honors declared separators" do
    eurozone = HyperCast::NumFormat.new(decimal_sep: ",", group_sep: ".", flags: HyperCast::ALL_STYLES)
    expect(described_class.f64("1.234,5", eurozone)).to eq(HyperCast::Success.new(value: 1234.5))
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
end
