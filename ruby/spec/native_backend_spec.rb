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

  it "loads under -w without a method-redefinition warning" do
    # 1.0.0 printed 52 lines under `ruby -w`, "method redefined; discarding old bool" and
    # "previous definition of bool was here" for each of the twenty-six doors: the Fiddle
    # definitions sat on HyperCast's singleton class, exactly where the extension then defined
    # its own. Any consumer running with warnings on saw it on every require.
    lib = File.expand_path("../lib", __dir__)
    out, status = Open3.capture2e(RbConfig.ruby, "-w", "-I", lib, "-r", "hypercast",
                                  "-e", "print HyperCast::BACKEND")
    expect([status.success?, out]).to eq([true, "native"])
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

  # Every checked door and its caller bugs, run as the same source on both backends: the
  # verdicts (non-ASCII spans included), a text match, the refusals' messages, the wrong-door
  # and wrong-type errors, and a Constraints that was never built.
  it "agrees with the Fiddle backend on the checked doors and their caller bugs" do
    rescued = 'begin; (%s).inspect; rescue StandardError => e; "#{e.class}: #{e.message}"; end'
    inv = "HyperCast::NumFormat::INVARIANT"
    c = "HyperCast::Constraints"
    [
      "HyperCast.checked('€5', HyperCast::NumFormat.new(decimal_sep: '.', group_sep: ',', " \
      "flags: HyperCast::ALL_STYLES, currency: '€'), #{c}.new(door: :i32, maximum: 4))",
      "HyperCast.checked('1,5', #{inv}, #{c}.new(door: :decimal, multiple_of: '0.5'), separator: ',', delimiter: '.')",
      "HyperCast.checked('12.37', #{inv}, #{c}.new(door: :f32, multiple_of: '0.05', rounding: :ceiling))",
      "HyperCast.checked('x9', #{inv}, #{c}.new(door: :u64))",
      "HyperCast.checked_from_float(0.1 + 0.2, #{c}.new(door: :f64, multiple_of: '0.1', rounding: :midpoint))",
      "HyperCast.checked_from_float(-0.0, #{c}.new(door: :decimal, maximum: 1))",
      "HyperCast.checked_from_float(Float::INFINITY, #{c}.new(door: :i8))",
      "HyperCast.text(' éé ', #{c}.new(door: :text, trim: true, max_length: 1))",
      "HyperCast.text(' FEMALE ', #{c}.new(door: :text, trim: true, enum: %w[Male Female]))",
      "HyperCast.text('ab\\xFFc'.b, #{c}.new(door: :text))",
      "HyperCast.text('héllo'.encode(Encoding::UTF_16LE))",
      # Text with no UTF-8 form: a lone surrogate, malformed at its character on both backends.
      "HyperCast.text([0x31, 0xD800, 0x32].pack('v*').force_encoding(Encoding::UTF_16LE))",
      "HyperCast.bool([0x31, 0xD800].pack('v*').force_encoding(Encoding::UTF_16LE))",
      "HyperCast.i32([0x31, 0xD800].pack('v*').force_encoding(Encoding::UTF_16LE), #{inv})",
      "HyperCast.checked([0x31, 0xD800].pack('v*').force_encoding(Encoding::UTF_16LE), #{inv}, #{c}.new(door: :i32))",
      "#{c}.new(door: :text, enum: [[0xD800].pack('v*').force_encoding(Encoding::UTF_16LE)])",
      # Text in a dummy encoding: transcoded whole, malformed at each conversion error.
      "HyperCast.i32(\"\\xFE\\xFF\\x00\\x35\\xD8\\x00\".b.force_encoding(Encoding::UTF_16), #{inv})",
      "HyperCast.text(\"\\e$B\\x24\\x22\\e(B5\\xFF\".b.force_encoding(Encoding::ISO_2022_JP))",
      "HyperCast.i32('+AGE-'.b.force_encoding(Encoding::UTF_7), #{inv})",
      "HyperCast.i32(\"4\\xA1\\xA1\".b.force_encoding(Encoding::EUC_TW), #{inv})",
      "HyperCast.text('')",
      "HyperCast.declared_decimal('-1.50')",
      "HyperCast.declared_decimal('1,5')",
      "HyperCast.constraints_check(door: :text, enum: %w[Male MALE])",
      "#{c}.new(door: :i32, minimum: 0, exclusive_minimum: 0)",
      "#{c}.new(door: :text, enum: ['\\xFF'.b])",
      "#{c}.new(door: :text, enum: [])",
      "#{c}.new(door: :text, trim: true, enum: [' Male '])",
      "#{c}.new(door: :text, enum: [' Male '])",
      "#{c}.new(door: :f64, minimum: 0.5)",
      "HyperCast.checked('1', #{inv}, #{c}.new(door: :text))",
      "HyperCast.text('1', #{c}.new(door: :i8))",
      "HyperCast.checked('1', #{inv}, :i8)",
      "HyperCast.checked_from_float('1', #{c}.new(door: :i8))",
      "HyperCast.checked('1', #{inv}, #{c}.allocate)"
    ].each do |source|
      native = begin
        eval(source).inspect # rubocop:disable Security/Eval -- the same source, run on both backends
      rescue StandardError => e
        "#{e.class}: #{e.message}"
      end
      expect(fiddle_eval(format(rescued, source))).to eq(native), source
    end
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

  # Every field a culture table fills in, and the day periods: the extension reads the Data
  # class's fields into the core's own format and the designators into its own day periods,
  # where Fiddle hands the core the packed layouts. Verdicts, spans and the caller bugs for a
  # malformed field or designator must come out the same.
  it "agrees with the Fiddle backend on every declared symbol, the day periods and their caller bugs" do
    rescued = 'begin; (%s).inspect; rescue StandardError => e; "#{e.class}: #{e.message}"; end'
    everything = 'HyperCast::NumFormat.new(decimal_sep: ",", group_sep: ". ", ' \
                 "flags: HyperCast::ALL_STYLES | HyperCast::TRAILING_SIGN, " \
                 'currency: "€", currency_decimal_sep: ".", currency_group_sep: ",", minus_sign: "−", ' \
                 'plus_sign: "＋", percent: "٪", digit_zero: "٠")'
    [
      "HyperCast.i32('−5', #{everything})",
      "HyperCast.i32('5 €−', #{everything})",
      "HyperCast.i32('＋5-', #{everything})",
      "HyperCast.f64('€1,234.50', #{everything})",
      "HyperCast.f64('1. 234,50', #{everything})",
      "HyperCast.f64('٥٠٪', #{everything})",
      "HyperCast.decimal('١٢,٥-', #{everything})",
      "HyperCast.i32('−-5', #{everything})",
      "HyperCast.i32('1, 234', HyperCast::NumFormat.new(decimal_sep: '.', group_sep: ', ', flags: HyperCast::ALL_STYLES))",
      "HyperCast::NumFormat.new(decimal_sep: '.', group_sep: ',', flags: 95, digit_zero: '0')",
      "HyperCast::NumFormat.new(decimal_sep: '.', group_sep: ',', flags: 95, minus_sign: '- ')",
      "HyperCast::NumFormat.new(decimal_sep: ',', group_sep: '.', flags: 95, currency_decimal_sep: '.')",
      "HyperCast::NumFormat.new(decimal_sep: '.', group_sep: ',,,,,,,,,', flags: 95)",
      "HyperCast::NumFormat.new(decimal_sep: '.', group_sep: ',', flags: 95, " \
      "currency: \"\\xD8\\x00\".dup.force_encoding(Encoding::UTF_16BE))",
      "HyperCast::NumFormat.new(decimal_sep: \"\\xED\\xA0\\x80\".dup.force_encoding(Encoding::UTF_8), " \
      "group_sep: ',', flags: 95)",
      "HyperCast::NumFormat.from_i18n({ separator: ',', delimiter: '.', negative_format: '%n-' }, " \
      "currency_format: { separator: '.', delimiter: \"'\", unit: 'CHF' })",
      "HyperCast.datetime('1/7/2026 3:04 nachm.', :day_month_year, am: 'vorm.', pm: 'nachm.')",
      "HyperCast.datetime('2026-01-07 오후 3:04', :year_month_day, am: '오전', pm: '오후')",
      "HyperCast.datetime('1/7/2026 3:04 p. m.', :month_day_year, am: 'a. m.', pm: 'p. m.')",
      "HyperCast.datetime('1/7/2026 3:04 PM', :month_day_year, am: 'vorm.', pm: 'nachm.')",
      "HyperCast.datetime('1/7/2026 3:04 nachm.', :day_month_year, am: nil, pm: nil)",
      "HyperCast.datetime('1/7/2026 3:04 nachm.', :day_month_year, pm: 'nachm.')",
      "HyperCast.datetime('1/7/2026 3:04 nachm.', :day_month_year, " \
      "am: 'vorm.'.encode(Encoding::UTF_16LE), pm: 'nachm.')",
      "HyperCast.datetime('1/7/2026', :day_month_year, am: ' vorm.', pm: 'nachm.')",
      "HyperCast.datetime('1/7/2026', :day_month_year, am: 'vorm.', pm: '')",
      "HyperCast.datetime('1/7/2026', :day_month_year, am: 'vorm.', pm: :nachm)",
      "HyperCast.datetime('1/7/2026', :day_month_year, am: 'a1', pm: 'p1')",
      "HyperCast.datetime('1/7/2026', :day_month_year, " \
      "am: \"\\xD8\\x00\".dup.force_encoding(Encoding::UTF_16BE), pm: 'nachm.')",
      "HyperCast.datetime('1/7/2026', :day_month_year, " \
      "am: 'vorm.', pm: \"\\xED\\xA0\\x80\".dup.force_encoding(Encoding::UTF_8))",
      "HyperCast.datetime('1/7/2026', :day_month_year, noon: 'x')",
      "HyperCast.datetime('1/7/2026', :little_endian, am: 'vorm.', pm: 'nachm.')",
      "HyperCast.datetime('1/7/2026', :day_month_year, :extra, am: 'vorm.')"
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
    [
      ['HyperCast.unix("1", :fortnights)', -> { HyperCast.unix("1", :fortnights) }],
      ['HyperCast.unix("1", "seconds")', -> { HyperCast.unix("1", "seconds") }],
      ['HyperCast.excel_serial("1", nil)', -> { HyperCast.excel_serial("1", nil) }],
      ['HyperCast.date("1/7/2026", "month_day_year")', -> { HyperCast.date("1/7/2026", "month_day_year") }],
      ['HyperCast.datetime("1/7/2026 3:04 PM", 2)', -> { HyperCast.datetime("1/7/2026 3:04 PM", 2) }],
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
      expect(native).to match(/\A(KeyError|TypeError|RangeError): /), source
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

  # The extension remembers the last format each thread resolved. Keyed by the object's address,
  # a compacting collection moved the remembered format and a new one built at its old address
  # was read as it: a Eurozone format cast "1,234" as 1234. The key is the object_id, which
  # moves with the object and is never reused. Fiddle.dlwrap is the object's address.
  it "never mistakes a format built after a compacting collection for the one before it" do
    skip "this Ruby's GC does not compact" unless GC.respond_to?(:verify_compaction_references)

    lib = File.expand_path("../lib", __dir__)
    script = <<~RUBY
      require "fiddle"
      flags = HyperCast::ALL_STYLES
      dollars = HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: flags)
      HyperCast.i32("1,234", dollars)
      was = Fiddle.dlwrap(dollars)
      GC.verify_compaction_references(expand_heap: true, toward: :empty)
      kept = []
      verdict = nil
      200_000.times do
        euros = HyperCast::NumFormat.new(decimal_sep: ",", group_sep: ".", flags: flags)
        kept << euros
        next unless Fiddle.dlwrap(euros) == was

        verdict = HyperCast.i32("1,234", euros)
        break
      end
      print Fiddle.dlwrap(dollars) != was, " ", verdict.inspect
    RUBY
    out, status = Open3.capture2e(RbConfig.ruby, "-I", lib, "-r", "hypercast", "-e", script)
    skip "no format was built at the moved one's old address (#{out})" if out.end_with?(" nil")
    want = "true #<data HyperCast::Fault reason=:malformed, offset=1, length=1>"
    expect([status.success?, out]).to eq([true, want])
  end

  # Faults the extension builds without Data.new carry the detail as Data.new-built ones do, and
  # nothing else about them differs.
  it "hands back faults carrying their detail, equal to ones Data.new built" do
    fault = HyperCast.i32("12,34", HyperCast::NumFormat::INVARIANT)
    built = HyperCast::Fault.new(reason: :malformed, offset: 2, length: 3, detail: HyperCast::Detail::GROUP_SIZE)
    expect(fault).to eq(built).and be_frozen
    expect(fault).to eql(built)
    expect(fault.hash).to eq(built.hash)
    expect(fault.detail).to equal(HyperCast::Detail::GROUP_SIZE)
    expect(fault.with(offset: 1).detail).to eq(HyperCast::Detail::GROUP_SIZE)
    expect(Marshal.load(Marshal.dump(fault)).detail).to eq(HyperCast::Detail::GROUP_SIZE)
    expect((fault in HyperCast::Fault(reason: :malformed, detail: HyperCast::Detail::GROUP_SIZE))).to be(true)
    expect(HyperCast.decimal_from_float(Float::NAN).detail).to eq(HyperCast::Detail::NOT_FINITE)
  end

  it "agrees with the Fiddle backend on grouping, details and the date options" do
    expression = <<~'RUBY'
      invariant = HyperCast::NumFormat::INVARIANT
      options = HyperCast::DateOptions
      french = %w[janv. févr. mars avr. mai juin juil. août sept. oct. nov. déc.].each_with_index.to_h { |n, i| [n, i + 1] }
      months = HyperCast::MonthNames.new(french)
      [
        HyperCast.i32("12,34", invariant), HyperCast.i32("12,34", invariant).detail,
        HyperCast.i32("12,34,567", invariant.with(grouping: :indian)),
        HyperCast.i32("12,34", invariant.with(grouping: :indian)),
        HyperCast.f64("1,234.5,6", invariant).detail, HyperCast.decimal("1234,567", invariant).detail,
        HyperCast.checked("12,34", invariant, HyperCast::Constraints.new(door: :i32, maximum: "5")).detail,
        HyperCast.date("20260107", options: options::COMMON_FORMS),
        HyperCast.timestamp("2026-01-07 15:04:05+00", options: options::COMMON_FORMS),
        HyperCast.date("7.janv..26", :day_month_year, options: options.new(pivot: 2029), months: months),
        HyperCast.date("7.janv.26", :day_month_year, options: options.new(pivot: 2029), months: months).detail,
        HyperCast.datetime("7.janv..2026 3:04 PM", :day_month_year, months: months),
        HyperCast.excel_time(-1).detail,
        HyperCast.i32("\u0661\u0662\u202F\u0663\u0664", HyperCast::NumFormat.new(decimal_sep: ".", group_sep: "\u202F", flags: 95, digit_zero: "\u0660"))
      ].map(&:inspect).join("|")
    RUBY
    expect(fiddle_eval(expression)).to eq(eval(expression)) # rubocop:disable Security/Eval
  end

  it "agrees with the Fiddle backend on the caller bugs of 1.1" do
    expression = <<~'RUBY'
      bugs = [
        -> { HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: HyperCast::GROUPING_POLICY) },
        -> { HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: 1 << 10) },
        -> { HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: 0, grouping: :nope) },
        -> { HyperCast::DateOptions.new(flags: 8) }, -> { HyperCast::DateOptions.new(pivot: 99) },
        -> { HyperCast::MonthNames.new({}) }, -> { HyperCast::MonthNames.from_packed("\x01".b) },
        -> { HyperCast.date("x", :year_month_day, options: 1) }, -> { HyperCast.timestamp("x", options: 1) },
        -> { HyperCast.datetime("x", :year_month_day, months: 1) },
        -> { HyperCast.date("x", :year_month_day, months: :names) },
        -> { HyperCast.date("x", :year_month_day, months: HyperCast::MonthNames.allocate.tap { |n| n.instance_variable_set(:@packed, "".b) }) }
      ]
      bugs.map { |bug| begin; bug.call; rescue StandardError => error; "#{error.class}: #{error.message}"; end }.join("|")
    RUBY
    native = eval(expression) # rubocop:disable Security/Eval
    expect(native).not_to include("NoMethodError")
    expect(fiddle_eval(expression)).to eq(native)
  end
end
