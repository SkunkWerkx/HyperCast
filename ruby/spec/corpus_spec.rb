# Replays the shared conformance corpus (corpus/*.json at the repository root) — the same
# files every other binding's suite replays — through this binding's doors. Fault spans are
# byte offsets into the UTF-8 input, identical to Ruby String byte offsets here.

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

  def format_of(vector)
    spec = vector["format"]
    return HyperCast::NumFormat::INVARIANT unless spec

    HyperCast::NumFormat.new(
      decimal_sep: spec["decimal_sep"], group_sep: spec["group_sep"], flags: spec["flags"]
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
        expect([offset, length]).to eq(span), "#{domain}: #{input.inspect} fault span"
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

  it "replays integer.json" do
    corpus("integer.json").each do |vector|
      door = vector.fetch("type").to_sym
      verdict = HyperCast.public_send(door, vector["input"], format_of(vector))
      assert_verdict("integer", vector, verdict, vector["value"])
    end
  end

  it "replays real.json" do
    corpus("real.json").each do |vector|
      door = vector.fetch("type") == "f32" ? :f32 : :f64
      expected = vector["value"]
      expected = [expected].pack("e").unpack1("e") if expected && door == :f32
      verdict = HyperCast.public_send(door, vector["input"], format_of(vector))
      assert_verdict("real", vector, verdict, expected)
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

  it "replays date.json" do
    corpus("date.json").each do |vector|
      expected = vector.key?("year") ? Date.new(vector["year"], vector["month"], vector["day"]) : nil
      assert_verdict("date", vector, HyperCast.date(vector["input"]), expected)
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
end
