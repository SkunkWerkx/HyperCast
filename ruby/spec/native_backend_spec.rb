require "spec_helper"
require "open3"

# Cross-backend agreement: the Magnus extension and the pure-Fiddle fallback must be
# indistinguishable through the public surface. The whole main spec suite already runs under
# both backends (HYPERCAST_PURE=1 forces Fiddle); this file pins the *agreement* between them
# by comparing deterministic outputs across a subprocess boundary.
RSpec.describe "native backend" do
  before(:all) do
    skip "Magnus extension not loaded (BACKEND=#{HyperCast::BACKEND})" unless
      HyperCast::BACKEND == :native
  end

  def fiddle_eval(expression)
    lib = File.expand_path("../lib", __dir__)
    out, status = Open3.capture2(
      { "HYPERCAST_PURE" => "1" },
      RbConfig.ruby, "-I", lib, "-r", "hypercast", "-e", "print (#{expression})"
    )
    raise "fiddle subprocess failed: #{out}" unless status.success?

    out
  end

  it "reports the native backend" do
    expect(HyperCast::BACKEND).to eq(:native)
    expect(fiddle_eval("HyperCast::BACKEND")).to eq("fiddle")
  end

  # The extension builds Success and Fault without going through Data.new (the keyword Hash
  # it gathers was most of a lean door's cost), so what it hands back has to be the object
  # Data.new would have made in every way a caller can observe.
  it "hands back verdicts indistinguishable from ones Data.new built" do
    success = HyperCast.bool("true")
    built = HyperCast::Success.new(value: true)
    expect(success).to eq(built).and be_frozen
    expect(success).to eql(built)
    expect(success.hash).to eq(built.hash)
    expect(success.to_h).to eq(value: true)
    expect(success.with(value: false)).to eq(HyperCast::Success.new(value: false))
    expect(Marshal.load(Marshal.dump(success))).to eq(built)
    expect((success in HyperCast::Success(value: true))).to be(true)

    fault = HyperCast.i32("12x", HyperCast::NumFormat::INVARIANT)
    expect(fault).to eq(HyperCast::Fault.new(reason: :malformed, offset: 2, length: 1)).and be_frozen
    expect(fault.to_h).to eq(reason: :malformed, offset: 2, length: 1)
    expect((fault in HyperCast::Fault(reason: :malformed, offset: 2, length: 1))).to be(true)
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
    expect(fiddle_eval("HyperCast.timestamp(#{text.inspect}).value.nsec"))
      .to eq(HyperCast.timestamp(text).value.nsec.to_s)
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
    native = HyperCast.decimal("($1,234.50)", HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",",
                                                                       flags: HyperCast::ALL_STYLES, currency: "$"))
    expect(native).to eq(HyperCast::Success.new(value: HyperCast::Decimal.new(magnitude: 12_345, scale: 1,
                                                                              negative: true)))
    expect(fiddle_eval(expression)).to eq(native.inspect)
    # Past 2**64: the magnitude crosses as a Bignum on both backends.
    big = "79228162514264337593543950335"
    expect(fiddle_eval("HyperCast.decimal(#{big.inspect}, HyperCast::NumFormat::INVARIANT).value.magnitude"))
      .to eq(HyperCast.decimal(big, HyperCast::NumFormat::INVARIANT).value.magnitude.to_s)
  end

  it "agrees with the Fiddle backend on the char door" do
    ['"U+00E9"', '"😀"', '" "', '"&#65"', '"U+110000"', '"U+é".b'].each do |text|
      expect(fiddle_eval("HyperCast.char(#{text}).inspect"))
        .to eq(HyperCast.char(eval(text)).inspect) # rubocop:disable Security/Eval
    end
    expect(fiddle_eval('HyperCast.char("&#x1F600;").value.encoding'))
      .to eq(HyperCast.char("&#x1F600;").value.encoding.to_s)
  end

  it "agrees with the Fiddle backend on the core's version and availability" do
    expect(fiddle_eval("HyperCast.native_version")).to eq(HyperCast.native_version)
    expect(fiddle_eval("HyperCast.available?")).to eq(HyperCast.available?.to_s)
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
    expect(fiddle_eval('HyperCast.i32("1€x".encode(Encoding::UTF_16LE), HyperCast::NumFormat::INVARIANT).inspect'))
      .to eq(HyperCast.i32("1€x".encode(Encoding::UTF_16LE), HyperCast::NumFormat::INVARIANT).inspect)
  end

  it "agrees with the Fiddle backend on format overrides, their verdicts and their caller bugs" do
    rescued = 'begin; (%s).inspect; rescue StandardError => e; "#{e.class}: #{e.message}"; end'
    inv = "HyperCast::NumFormat::INVARIANT"
    [
      "HyperCast.i32('1.234', #{inv}, separator: ',', delimiter: '.')",
      "HyperCast.u64('1.234', #{inv}, separator: ',', delimiter: '.')",
      "HyperCast.f64('1.234,5', #{inv}, separator: ',', delimiter: '.')",
      "HyperCast.decimal('€1.234,50', #{inv}, separator: ',', delimiter: '.', unit: '€')",
      "HyperCast.i32('1,234', #{inv}, separator: nil)",
      "HyperCast.i32('1', #{inv}, separator: ',')",
      "HyperCast.i32('1', #{inv}, precision: 2)",
      "HyperCast.f32('1', #{inv}, unit: '12')",
      "HyperCast.i32('1', nil, unit: '$')",
      "HyperCast.i32('1')",
      "HyperCast::NumFormat.from_i18n({ separator: ',', delimiter: '' })"
    ].each do |source|
      native = begin
        eval(source).inspect # rubocop:disable Security/Eval -- the same source, run on both backends
      rescue StandardError => e
        "#{e.class}: #{e.message}"
      end
      expect(fiddle_eval(format(rescued, source))).to eq(native), source
    end
  end

  it "agrees with the Fiddle backend on an explicit nil date order" do
    expect(fiddle_eval('HyperCast.date("2026-01-07", nil).inspect')).to eq(HyperCast.date("2026-01-07", nil).inspect)
    expect(fiddle_eval('HyperCast.date("1/7/2026", nil).inspect')).to eq(HyperCast.date("1/7/2026", nil).inspect)
  end

  it "agrees with the Fiddle backend on caller bugs: the same exception, worded the same" do
    # The extension replaces the doors themselves, so a caller bug is raised by Rust here and
    # by Ruby there. Class and message are compared across the subprocess boundary.
    rescued = 'begin; %s; rescue StandardError => e; "#{e.class}: #{e.message}"; end'
    lone_surrogate = '[0x0031, 0xD800].pack("v*").force_encoding(Encoding::UTF_16LE)'
    [
      ['HyperCast.unix("1", :fortnights)', -> { HyperCast.unix("1", :fortnights) }],
      ['HyperCast.unix("1", "seconds")', -> { HyperCast.unix("1", "seconds") }],
      ['HyperCast.excel_serial("1", nil)', -> { HyperCast.excel_serial("1", nil) }],
      ['HyperCast.date("1/7/2026", "month_day_year")', -> { HyperCast.date("1/7/2026", "month_day_year") }],
      ['HyperCast.datetime("1/7/2026 3:04 PM", 2)', -> { HyperCast.datetime("1/7/2026 3:04 PM", 2) }],
      ["HyperCast.bool(#{lone_surrogate})",
       -> { HyperCast.bool([0x0031, 0xD800].pack("v*").force_encoding(Encoding::UTF_16LE)) }],
      ["HyperCast.i32(42, HyperCast::NumFormat::INVARIANT)", -> { HyperCast.i32(42, HyperCast::NumFormat::INVARIANT) }],
      ['HyperCast.decimal_from_float("0.5")', -> { HyperCast.decimal_from_float("0.5") }],
      ["HyperCast.excel_time(nil)", -> { HyperCast.excel_time(nil) }],
      ["HyperCast.excel_duration(true)", -> { HyperCast.excel_duration(true) }],
      ["HyperCast.excel_time(:noon)", -> { HyperCast.excel_time(:noon) }],
      ["HyperCast.excel_duration(Complex(1, 2))", -> { HyperCast.excel_duration(Complex(1, 2)) }],
      ["HyperCast.excel_serial_from_float(1.0, :y2000)", -> { HyperCast.excel_serial_from_float(1.0, :y2000) }],
      ['HyperCast.excel_serial_from_float("1", :y2000)', -> { HyperCast.excel_serial_from_float("1", :y2000) }]
    ].each do |source, call|
      native = begin
        call.call
      rescue StandardError => e
        "#{e.class}: #{e.message}"
      end
      expect(native).to match(/\A(KeyError|TypeError|RangeError|Encoding::InvalidByteSequenceError): /), source
      expect(fiddle_eval(format(rescued, source))).to eq(native), source
    end
  end

  # The extension caches Ruby objects in Rust statics, out of the garbage collector's sight. A
  # constant keeps them from being collected but not from being moved, so each is pinned when the
  # extension loads; unpinned, a compacting collection moved them and the next call used whatever
  # took their place (a segfault, or a Success or Fault that was some other object). Every movable
  # object is moved here first, in a subprocess so a regression fails this example, not the run.
  it "keeps working after a compacting collection has moved everything it can" do
    skip "this Ruby's GC does not compact" unless GC.respond_to?(:verify_compaction_references)

    lib = File.expand_path("../lib", __dir__)
    script = <<~RUBY
      GC.verify_compaction_references(expand_heap: true, toward: :empty)
      print HyperCast::BACKEND, " ",
            HyperCast.i32("(1,234)", HyperCast::NumFormat::INVARIANT).inspect, " ",
            HyperCast.i32("1€", HyperCast::NumFormat::INVARIANT).inspect
    RUBY
    out, status = Open3.capture2e(RbConfig.ruby, "-I", lib, "-r", "hypercast", "-e", script)
    want = "native #<data HyperCast::Success value=-1234> " \
           "#<data HyperCast::Fault reason=:malformed, offset=1, length=1>"
    expect([status.success?, out]).to eq([true, want])
  end
end
