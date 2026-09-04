require "spec_helper"
require "open3"

# Cross-backend agreement: the Magnus extension and the pure-Fiddle fallback must be
# indistinguishable through the public surface. The whole main spec suite already runs under
# every backend (HYPERCAST_PURE=1 forces Fiddle, HYPERCAST_WASM=1 the wasmtime module); this
# file pins the *agreement* between Magnus and Fiddle by comparing deterministic outputs
# across a subprocess boundary, and wasm_backend_spec.rb does the same for wasm against
# Fiddle.
RSpec.describe "native backend" do
  before(:all) do
    skip "Magnus extension not loaded (BACKEND=#{HyperCast::BACKEND})" unless
      HyperCast::BACKEND == :native
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

  it "reports the native backend" do
    expect(HyperCast::BACKEND).to eq(:native)
    expect(fiddle_eval("HyperCast::BACKEND")).to eq("fiddle")
  end

  it "agrees with the Fiddle backend on a declared format" do
    eurozone = HyperCast::NumFormat.new(decimal_sep: ",", group_sep: ".", flags: HyperCast::ALL_STYLES)
    native = HyperCast.f64("1.234,5", eurozone)
    expect(fiddle_eval(
      'HyperCast.f64("1.234,5", HyperCast::NumFormat.new(decimal_sep: ",", group_sep: ".", ' \
      "flags: HyperCast::ALL_STYLES)).inspect"
    )).to eq(native.inspect)
  end

  it "agrees with the Fiddle backend on a fault span" do
    native = HyperCast.i32("  12x4", HyperCast::NumFormat::INVARIANT)
    expect(native).to eq(HyperCast::Fault.new(reason: :malformed, offset: 4, length: 1))
    expect(fiddle_eval('HyperCast.i32("  12x4", HyperCast::NumFormat::INVARIANT).inspect')).to eq(native.inspect)
  end

  it "agrees with the Fiddle backend on the temporal doors" do
    text = "2026-01-02T15:04:05.123456789+05:00"
    expect(fiddle_eval("HyperCast.timestamp(#{text.inspect}).value.nsec")).to eq(HyperCast.timestamp(text).value.nsec.to_s)
    expect(fiddle_eval('HyperCast.duration("-1.5s").value.to_s')).to eq(HyperCast.duration("-1.5s").value.to_s)
    expect(fiddle_eval('HyperCast.datetime("1/7/2026 3:04 PM", :month_day_year).value.iso8601(9)'))
      .to eq(HyperCast.datetime("1/7/2026 3:04 PM", :month_day_year).value.iso8601(9))
  end

  it "agrees with the Fiddle backend on the UUID door" do
    text = "urn:uuid:01020304-0506-0708-090A-0B0C0D0E0F10"
    expect(fiddle_eval("HyperCast.uuid(#{text.inspect}).value")).to eq(HyperCast.uuid(text).value)
  end
end
