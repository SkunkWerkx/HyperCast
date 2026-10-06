require_relative "spec_helper"

# The public interop surface a gem carrying HyperCast's verdicts across its own C ABI decodes
# with: each value unpacked from bytes laid out as the core lays them out, as the door of the
# same name presents it.
RSpec.describe HyperCast::Interop do
  it "decodes every door's value as the door presents it" do
    expect(described_class.decode(:timestamp, [1_767_366_245, 123_456_789, 0].pack("q<l<l<")))
      .to eq(HyperCast.timestamp("2026-01-02T15:04:05.123456789Z").value)
    expect(described_class.decode(:date, [2026, 1, 7].pack("S<CC"))).to eq(Date.new(2026, 1, 7))
    expect(described_class.decode(:datetime, [2026, 1, 7, 0, 54_245_123_456_789].pack("S<CCL<Q<")))
      .to eq(HyperCast.datetime("2026-01-07 15:04:05.123456789", :year_month_day).value)
    expect(described_class.decode(:duration, [-1, -500_000_000, 0].pack("q<l<l<"))).to eq(Rational(-3, 2))
    expect(described_class.decode(:decimal, [12_345, 0, 2, 1, 0].pack("Q<L<CCS<")))
      .to eq(HyperCast::Decimal.new(magnitude: 12_345, scale: 2, negative: true))
    expect(described_class.decode(:uuid, ["550e8400e29b41d4a716446655440000"].pack("H*")))
      .to eq("550e8400-e29b-41d4-a716-446655440000")
    expect(described_class.decode(:bool, "\x01".b)).to be(true)
    expect(described_class.decode(:u64, [2**64 - 1].pack("Q<"))).to eq(2**64 - 1)
  end

  it "sizes every door's value" do
    described_class::RECORDS.each do |door, (directive, _fields, _build)|
      expect(("\0" * described_class::VALUE_BYTES.fetch(door)).b.unpack(directive)).not_to include(nil)
    end
  end

  it "keeps a fault's span and refuses a code that names no reason" do
    expect(described_class.fault(2, 3, 4)).to eq(HyperCast::Fault.new(reason: :malformed, offset: 3, length: 4))
    expect { described_class.fault(7, 0, 0) }.to raise_error(KeyError)
  end

  it "maps a byte span to characters, and leaves ASCII and binary alone" do
    expect(described_class.characters("é1x", 3, 1)).to eq([2, 1])
    expect(described_class.characters("ab1x", 3, 1)).to eq([3, 1])
    expect(described_class.characters("é1x".b, 3, 1)).to eq([3, 1])
  end

  it "unpacks versions" do
    expect(described_class.version(0x00_06_02)).to eq("0.6.2")
  end

  it "names the library of the core asked for" do
    expect(HyperCast::NativePlatform.rid_and_library_name("x86_64-linux", library: "hypertabular"))
      .to eq(["linux-x64", "libhypertabular.so"])
  end
end
