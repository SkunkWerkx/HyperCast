require "spec_helper"
require "open3"

# Cross-backend agreement for the WebAssembly backend, the same contract
# native_backend_spec.rb pins for the Magnus extension: through the public surface, the
# core running as a wasm32-wasip1 module inside wasmtime must be indistinguishable from the
# same core dlopen'd through Fiddle. The whole main suite already runs under this backend
# (HYPERCAST_WASM=1); this file pins the agreement itself by comparing deterministic outputs
# across a subprocess boundary.
RSpec.describe "wasm backend" do
  before(:all) do
    skip "wasm backend not loaded (BACKEND=#{HyperCast::BACKEND})" unless
      HyperCast::BACKEND == :wasm
  end

  def fiddle_eval(expression)
    lib = File.expand_path("../lib", __dir__)
    out, status = Open3.capture2(
      { "HYPERCAST_PURE" => "1", "HYPERCAST_WASM" => nil },
      RbConfig.ruby, "-I", lib, "-r", "hypercast", "-e", "print (#{expression})"
    )
    raise "fiddle subprocess failed: #{out}" unless status.success?

    out
  end

  it "reports the wasm backend" do
    expect(HyperCast::BACKEND).to eq(:wasm)
    expect(fiddle_eval("HyperCast::BACKEND")).to eq("fiddle")
  end

  it "agrees with the Fiddle backend on a declared format" do
    eurozone = HyperCast::NumFormat.new(decimal_sep: ",", group_sep: ".", flags: HyperCast::ALL_STYLES)
    wasm = HyperCast.f64("1.234,5", eurozone)
    expect(wasm).to eq(HyperCast::Success.new(value: 1234.5))
    expect(fiddle_eval(
      'HyperCast.f64("1.234,5", HyperCast::NumFormat.new(decimal_sep: ",", group_sep: ".", ' \
      "flags: HyperCast::ALL_STYLES)).inspect"
    )).to eq(wasm.inspect)
  end

  it "agrees with the Fiddle backend on a fault span" do
    wasm = HyperCast.i32("  12x4", HyperCast::NumFormat::INVARIANT)
    expect(wasm).to eq(HyperCast::Fault.new(reason: :malformed, offset: 4, length: 1))
    expect(fiddle_eval('HyperCast.i32("  12x4", HyperCast::NumFormat::INVARIANT).inspect')).to eq(wasm.inspect)
  end

  it "agrees with the Fiddle backend on the temporal doors" do
    text = "2026-01-02T15:04:05.123456789+05:00"
    wasm = HyperCast.timestamp(text)
    expect(wasm).to eq(HyperCast::Success.new(value: Time.at(1_767_348_245, 123_456_789, :nanosecond, in: "UTC")))
    expect(fiddle_eval("HyperCast.timestamp(#{text.inspect}).value.nsec")).to eq(wasm.value.nsec.to_s)
    expect(fiddle_eval('HyperCast.duration("-1.5s").value.to_s')).to eq(HyperCast.duration("-1.5s").value.to_s)
    expect(fiddle_eval('HyperCast.datetime("1/7/2026 3:04 PM", :month_day_year).value.iso8601(9)'))
      .to eq(HyperCast.datetime("1/7/2026 3:04 PM", :month_day_year).value.iso8601(9))
  end

  it "agrees with the Fiddle backend on the UUID door" do
    text = "urn:uuid:01020304-0506-0708-090A-0B0C0D0E0F10"
    expect(fiddle_eval("HyperCast.uuid(#{text.inspect}).value")).to eq(HyperCast.uuid(text).value)
  end

  it "agrees with the Fiddle backend on the decimal door and a declared currency" do
    expression = 'HyperCast.decimal("($1,234.50)", HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", ' \
                 'flags: HyperCast::ALL_STYLES, currency: "$")).inspect'
    wasm = HyperCast.decimal("($1,234.50)", HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",",
                                                                     flags: HyperCast::ALL_STYLES, currency: "$"))
    expect(wasm).to eq(HyperCast::Success.new(value: HyperCast::Decimal.new(magnitude: 12_345, scale: 1,
                                                                            negative: true)))
    expect(fiddle_eval(expression)).to eq(wasm.inspect)
    big = "79228162514264337593543950335"
    expect(fiddle_eval("HyperCast.decimal(#{big.inspect}, HyperCast::NumFormat::INVARIANT).value.magnitude"))
      .to eq(HyperCast.decimal(big, HyperCast::NumFormat::INVARIANT).value.magnitude.to_s)
  end

  it "agrees with the Fiddle backend on the core's version and availability" do
    expect(HyperCast.native_version).to eq(HyperCast::VERSION)
    expect(fiddle_eval("HyperCast.native_version")).to eq(HyperCast.native_version)
    expect(HyperCast.available?).to be(true)
    expect(fiddle_eval("HyperCast.available?")).to eq("true")
  end

  it "agrees with the Fiddle backend on character and byte fault spans" do
    expect(HyperCast.i32("1€", HyperCast::NumFormat::INVARIANT))
      .to eq(HyperCast::Fault.new(reason: :malformed, offset: 1, length: 1))
    expect(HyperCast.i32("1€".b, HyperCast::NumFormat::INVARIANT))
      .to eq(HyperCast::Fault.new(reason: :malformed, offset: 1, length: 3))
    expect(fiddle_eval('HyperCast.i32("1€", HyperCast::NumFormat::INVARIANT).inspect'))
      .to eq(HyperCast.i32("1€", HyperCast::NumFormat::INVARIANT).inspect)
    expect(fiddle_eval('HyperCast.i32("1€".b, HyperCast::NumFormat::INVARIANT).inspect'))
      .to eq(HyperCast.i32("1€".b, HyperCast::NumFormat::INVARIANT).inspect)
  end

  it "only ever grows the guest input buffer, and loses nothing across the regrowth" do
    padded = "#{' ' * 10_000}42#{' ' * 10_000}"
    invariant = HyperCast::NumFormat::INVARIANT
    expect(HyperCast.i32("7", invariant)).to eq(HyperCast::Success.new(value: 7))
    expect(HyperCast.i32(padded, invariant)).to eq(HyperCast::Success.new(value: 42))
    expect(HyperCast.i32("7", invariant)).to eq(HyperCast::Success.new(value: 7))
    # Trailing junk after the padding: the first offending byte is the space at 10002.
    expect(HyperCast.i32("#{padded}x", invariant))
      .to eq(HyperCast::Fault.new(reason: :malformed, offset: 10_002, length: 1))
  end

  it "memoizes the format by identity, not by value" do
    # Two distinct formats must each write their bytes: a stale one would silently parse
    # under the wrong separators — or a stale currency symbol, which lives in the same
    # 32-byte write.
    a = HyperCast::NumFormat.new(decimal_sep: ",", group_sep: ".", flags: HyperCast::ALL_STYLES)
    b = HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: HyperCast::ALL_STYLES)
    c = HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: HyperCast::ALL_STYLES, currency: "€")
    expect(HyperCast.f64("1.234,5", a)).to eq(HyperCast::Success.new(value: 1234.5))
    expect(HyperCast.f64("1,234.5", b)).to eq(HyperCast::Success.new(value: 1234.5))
    expect(HyperCast.f64("1.234,5", a)).to eq(HyperCast::Success.new(value: 1234.5))
    expect(HyperCast.f64("1,234.5 €", c)).to eq(HyperCast::Success.new(value: 1234.5))
    expect(HyperCast.f64("1,234.5 €", b)).to be_a(HyperCast::Fault)
  end

  it "raises the package's own errors for caller bugs, never a verdict" do
    expect { HyperCast.unix("1", :fortnights) }.to raise_error(KeyError)
    expect { HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ".", flags: HyperCast::ALL_STYLES) }
      .to raise_error(ArgumentError)
  end

  it "serializes concurrent callers on the one shared instance" do
    invariant = HyperCast::NumFormat::INVARIANT
    values = Array.new(8) do |n|
      Thread.new { Array.new(200) { |i| HyperCast.i32((n * 1000 + i).to_s, invariant).value } }
    end.flat_map(&:value)
    expect(values.sort).to eq(Array.new(8) { |n| Array.new(200) { |i| n * 1000 + i } }.flatten.sort)
  end
end
