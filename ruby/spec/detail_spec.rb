require "open3"
require "spec_helper"

# HyperCast 1.1 on the Ruby surface: strict grouping, fault detail and the opt-in date shapes.
# Run under both backends, like the rest of the suite.
RSpec.describe "HyperCast 1.1" do
  invariant = HyperCast::NumFormat::INVARIANT
  fault = ->(text, format = invariant) { HyperCast.i32(text, format) }

  describe "strict grouping" do
    it "defaults every constructor and the profiles to strict Western" do
      expect(invariant.grouping).to eq(:western)
      expect(HyperCast::NumFormat::DETECT.grouping).to eq(:western)
      expect(HyperCast::NumFormat.new(decimal_sep: ",", group_sep: ".", flags: HyperCast::ALL_STYLES).grouping)
        .to eq(:western)
      expect(HyperCast::NumFormat.from_i18n({ separator: ",", delimiter: "." }).grouping).to eq(:western)
    end

    it "reads groups of three, and refuses the rest with the group's span and detail" do
      expect(fault.call("1,234,567")).to eq(HyperCast::Success.new(value: 1_234_567))
      expect(fault.call("1234")).to eq(HyperCast::Success.new(value: 1234))
      expect(fault.call("12,34")).to eq(HyperCast::Fault.new(reason: :malformed, offset: 2, length: 3))
      expect(fault.call("12,34").detail).to eq(HyperCast::Detail::GROUP_SIZE)
      expect(fault.call("1234,567")).to eq(HyperCast::Fault.new(reason: :malformed, offset: 0, length: 4))
      expect(fault.call("1,234.5,6")).to be_a(HyperCast::Fault)
      expect(HyperCast.f64("1,234.5", invariant)).to eq(HyperCast::Success.new(value: 1234.5))
      expect(HyperCast.decimal("12,34", invariant).detail).to eq(HyperCast::Detail::GROUP_SIZE)
    end

    it "reads lakh and crore only when Indian is declared, and 1.0's reading when legacy is" do
      indian = invariant.with(grouping: :indian)
      legacy = invariant.with(grouping: :legacy)
      expect(fault.call("12,34,567", indian)).to eq(HyperCast::Success.new(value: 1_234_567))
      expect(fault.call("1,234,567", indian).detail).to eq(HyperCast::Detail::GROUP_SIZE)
      expect(fault.call("12,34", legacy)).to eq(HyperCast::Success.new(value: 1234))
      expect(fault.call("12,34,567", invariant).detail).to eq(HyperCast::Detail::GROUP_SIZE)
    end

    it "keeps the policy through #with and #override, and replaces it, never ORs it" do
      indian = invariant.with(grouping: :indian)
      expect(indian.with(flags: HyperCast::GROUPING).grouping).to eq(:indian)
      expect(indian.override(separator: ",", delimiter: ".").grouping).to eq(:indian)
      expect(indian.with(grouping: :legacy).raw_flags & HyperCast::GROUPING_POLICY).to eq(0)
      expect(indian.with(grouping: :western).raw_flags & HyperCast::GROUPING_POLICY).to eq(HyperCast::GROUPING_WESTERN)
    end

    it "judges a strict format with GROUPING off as forbidding the group separator" do
      ungrouped = invariant.with(flags: HyperCast::ALL_STYLES & ~HyperCast::GROUPING)
      expect(fault.call("1,234", ungrouped)).to be_a(HyperCast::Fault)
      expect(fault.call("1234", ungrouped)).to eq(HyperCast::Success.new(value: 1234))
    end

    it "counts native digits as digits and holds a space-like separator to the same sizes" do
      arabic = HyperCast::NumFormat.new(decimal_sep: ".", group_sep: "\u202F", flags: HyperCast::ALL_STYLES,
                                        digit_zero: "\u0660")
      expect(fault.call("\u0661\u202F\u0662\u0663\u0664", arabic)).to eq(HyperCast::Success.new(value: 1234))
      two = fault.call("\u0661\u0662\u202F\u0663\u0664", arabic)
      expect([two.reason, two.offset, two.length]).to eq([:malformed, 2, 3])
      expect(two.detail).to eq(HyperCast::Detail::GROUP_SIZE)
    end

    it "takes grouping: from_i18n, and refuses a policy that is not one" do
      expect(HyperCast::NumFormat.from_i18n({ delimiter: "," }, grouping: :indian).grouping).to eq(:indian)
      expect { HyperCast::NumFormat.from_i18n({}, grouping: :lakh) }
        .to raise_error(ArgumentError, /grouping must be one of \[:legacy, :western, :indian\]/)
      expect { HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: 0, grouping: 1 << 8) }
        .to raise_error(ArgumentError, /grouping must be one of/)
    end

    it "keeps the style mask apart from the policy: bits 8 and 9, and undefined bits, are refused" do
      refused = ->(flags) { HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: flags) }
      [HyperCast::GROUPING_WESTERN, HyperCast::GROUPING_INDIAN, HyperCast::GROUPING_POLICY,
       HyperCast::ALL_STYLES | HyperCast::GROUPING_POLICY].each do |flags|
        expect { refused.call(flags) }.to raise_error(ArgumentError, /must not carry a grouping policy/), flags.to_s
      end
      [1 << 10, (1 << 31) | 95, -1 & ~HyperCast::GROUPING_POLICY].each do |flags|
        expect { refused.call(flags) }.to raise_error(ArgumentError, /does not define/), flags.to_s
      end
      expect { refused.call("63") }.to raise_error(ArgumentError, /flags must be an Integer/)
      expect(refused.call(0xFF).flags).to eq(0xFF)
    end

    it "hands a strict request to a 1.1 core and nothing else" do
      expect(HyperCast::NumFormat::INVARIANT.packed[4, 4].unpack1("L<") & HyperCast::GROUPING_POLICY)
        .to eq(HyperCast::GROUPING_WESTERN)
    end
  end

  describe "fault detail" do
    it "names every published code, and keeps one it does not name" do
      expect(HyperCast::Detail::NAMES.keys).to eq((1..32).to_a)
      HyperCast::Detail::NAMES.each do |code, name|
        detail = HyperCast::Detail.of(code)
        expect([detail.code, detail.name, detail.unknown?]).to eq([code, name, false])
        expect(detail).to equal(HyperCast::Detail.const_get(name.to_s.upcase))
        expect(detail).to eq(HyperCast::Detail.new(code: code)).and be_frozen
      end
      unknown = HyperCast::Detail.of(999)
      expect([unknown.code, unknown.name, unknown.unknown?, unknown.to_s]).to eq([999, :unknown, true, "unknown"])
      expect(unknown).to eq(HyperCast::Detail.new(code: 999))
    end

    # A fault read out of a constructed RawFaultDetail carrying an unpublished code: the code
    # is kept, the reason and span stay usable, and nothing raises — the Fiddle path's own decode.
    it "decodes a constructed fault carrying an unpublished code without raising" do
      raw = [16, 3, 4, 999].pack("L4")
      decoded = HyperCast::Interop.raw_fault(2, raw, "12,345x")
      expect(decoded).to eq(HyperCast::Fault.new(reason: :malformed, offset: 3, length: 4))
      expect(decoded.detail).to be_unknown
      expect(decoded.detail.code).to eq(999)
      expect(decoded.detail).to eq(HyperCast::Detail.new(code: 999))
      expect(("12,345x"[decoded.offset, decoded.length])).to eq("345x")
      case decoded
      in HyperCast::Fault(reason: :malformed, detail: HyperCast::Detail(code: 999)) then nil
      end
      expect(HyperCast::Interop.raw_fault(2, [16, 0, 0, 0].pack("L4")).detail).to be_nil
    end

    it "exposes the detail by name in a pattern, and as a member nowhere" do
      case fault.call("12,34")
      in HyperCast::Fault(reason: :malformed, detail: HyperCast::Detail::GROUP_SIZE) then nil
      end
      case HyperCast.u8("-1", invariant)
      in HyperCast::Fault(detail: HyperCast::Detail::NEGATIVE_NOT_ALLOWED) then nil
      end
      expect(HyperCast.bool("maybe").detail).to eq(HyperCast::Detail::NOT_RECOGNIZED)
      expect(HyperCast.bool("").detail).to eq(HyperCast::Detail::BLANK)
      expect(HyperCast.text("\xFF".b).detail).to eq(HyperCast::Detail::INVALID_UTF8)
      expect(HyperCast.i32("12x", invariant).detail.name).to eq(:unexpected_character)
      expect(HyperCast::Fault.members).to eq(%i[reason offset length])
    end

    it "gives the typed doors and the checked doors a detail too, and a violation none" do
      expect(HyperCast.decimal_from_float(Float::NAN).detail).to eq(HyperCast::Detail::NOT_FINITE)
      expect(HyperCast.excel_serial_from_float(60, :y1900).detail).to eq(HyperCast::Detail::NONEXISTENT_DATE)
      expect(HyperCast.excel_time(-1).detail).to eq(HyperCast::Detail::NEGATIVE_NOT_ALLOWED)
      expect(HyperCast.excel_duration(Float::INFINITY).detail).to eq(HyperCast::Detail::NOT_FINITE)
      positive = HyperCast::Constraints.new(door: :i32, minimum: "0", maximum: "10")
      expect(HyperCast.checked("12,34", invariant, positive).detail).to eq(HyperCast::Detail::GROUP_SIZE)
      expect(HyperCast.checked_from_float(Float::NAN, positive).detail).to eq(HyperCast::Detail::NOT_FINITE)
      violation = HyperCast.checked("11", invariant, positive)
      expect(violation).to be_a(HyperCast::Violation)
      expect(violation).not_to respond_to(:detail)
      text = HyperCast::Constraints.new(door: :text, min_length: 3)
      expect(HyperCast.text("\xFF".b, text).detail).to eq(HyperCast::Detail::INVALID_UTF8)
    end

    # What a 1.0 caller wrote: the constructors, patterns and equality of Fault, unchanged.
    it "keeps the 1.0 Fault surface: constructors, patterns, equality, hash, Marshal" do
      by_keyword = HyperCast::Fault.new(reason: :malformed, offset: 2, length: 3)
      by_position = HyperCast::Fault.new(:malformed, 2, 3)
      expect(by_position).to eq(by_keyword).and eql(by_keyword)
      expect(by_position.hash).to eq(by_keyword.hash)
      expect(by_keyword.detail).to be_nil
      read = fault.call("12,34")
      expect(read).to eq(by_keyword).and eql(by_keyword)
      expect(read.hash).to eq(by_keyword.hash)
      expect(read.to_h).to eq(reason: :malformed, offset: 2, length: 3)
      expect(read.deconstruct_keys(nil)).to eq(reason: :malformed, offset: 2, length: 3)
      expect(read.deconstruct_keys(%i[reason offset])).to eq(reason: :malformed, offset: 2)
      expect(read.inspect).to eq(by_keyword.inspect)
      expect((read in HyperCast::Fault(reason: :malformed, offset: 2, length: 3))).to be(true)
      expect((read in { reason: :malformed })).to be(true)
      expect(read.with(offset: 0)).to eq(HyperCast::Fault.new(reason: :malformed, offset: 0, length: 3))
      expect(read.with(offset: 0).detail).to eq(HyperCast::Detail::GROUP_SIZE)
      expect(read.with(detail: HyperCast::Detail::BLANK).detail).to eq(HyperCast::Detail::BLANK)
      expect(read).to be_frozen
      expect(read.dup.detail).to eq(read.detail)
    end

    it "round-trips a fault through Marshal with its detail, and loads a 1.0 payload" do
      read = fault.call("12,34")
      loaded = Marshal.load(Marshal.dump(read))
      expect(loaded).to eq(read)
      expect(loaded.detail).to eq(HyperCast::Detail::GROUP_SIZE)
      # Marshal.dump of 1.0's Fault: the struct form, three members and no detail.
      payload = "\x04\bS:\x15HyperCast::Fault\b:\vreason:\x0Emalformed:\voffseti\a:\vlengthi\b".b
      old = Marshal.load(payload)
      expect(old).to eq(HyperCast::Fault.new(reason: :malformed, offset: 2, length: 3))
      expect(old.detail).to be_nil
    end

    # What a 1.0 caller wrote against NumFormat: ten members, positional and keyword
    # construction, array and hash patterns, Marshal. The policy is not a member (as
    # Fault#detail is not), so none of it moves.
    it "keeps 1.0's NumFormat: ten members, constructors, array and hash patterns, equality" do
      one_oh = HyperCast::NumFormat.new(decimal_sep: ".", group_sep: ",", flags: HyperCast::ALL_STYLES)
      expect(HyperCast::NumFormat.members).to eq(%i[decimal_sep group_sep flags currency currency_decimal_sep
                                                    currency_group_sep minus_sign plus_sign percent digit_zero])
      expect(one_oh).to eq(invariant).and eql(invariant)
      expect(one_oh.hash).to eq(invariant.hash)
      expect((one_oh in { decimal_sep: ".", group_sep: ",", flags: 95 })).to be(true)
      expect((one_oh in HyperCast::NumFormat[".", ",", 95, "", "", "", "", "", "", ""])).to be(true)
      expect((one_oh in HyperCast::NumFormat[_d, _g, _fl, _c, _cd, _cg, _mi, _pl, _pe, _dz])).to be(true)
      expect(one_oh.deconstruct).to eq([".", ",", 95, "", "", "", "", "", "", ""])
      expect(one_oh.to_h.keys).to eq(HyperCast::NumFormat.members)
      expect(one_oh.inspect).not_to include("grouping")
      expect(HyperCast::NumFormat.new(".", ",", 95)).to eq(one_oh)
      expect(invariant.flags).to eq(HyperCast::ALL_STYLES)
      expect(HyperCast::NumFormat.new(".", ",", 95, "$").currency).to eq("$")
    end

    it "keeps the policy in equality, eql?, hash and a pattern that names it" do
      western = invariant
      legacy = invariant.with(grouping: :legacy)
      indian = invariant.with(grouping: :indian)
      expect(western).not_to eq(legacy)
      expect(western).not_to eql(legacy)
      expect(legacy).not_to eq(indian)
      expect([western, legacy, indian].map(&:hash).uniq.size).to eq(3)
      expect(legacy).to eq(invariant.with(grouping: :legacy)).and eql(invariant.with(grouping: :legacy))
      expect(legacy.hash).to eq(invariant.with(grouping: :legacy).hash)
      expect({ western => 1, legacy => 2 }.size).to eq(2)
      expect(legacy.to_h).to eq(western.to_h)
      expect((legacy in { grouping: :legacy, flags: 95 })).to be(true)
      expect((western in { grouping: :legacy })).to be(false)
      expect(legacy.deconstruct_keys(%i[flags])).to eq(flags: 95)
      expect(legacy.with(flags: 0).grouping).to eq(:legacy)
      expect(legacy.with).to equal(legacy)
      expect(legacy.override(separator: ",", delimiter: ".").grouping).to eq(:legacy)
      expect(legacy.dup.grouping).to eq(:legacy)
      # Each policy reads differently through a door: the memo can never hand one's bytes to another.
      expect(HyperCast.i32("12,34", legacy)).to be_a(HyperCast::Success)
      expect(HyperCast.i32("12,34", western)).to be_a(HyperCast::Fault)
      expect(HyperCast.i32("12,34", legacy)).to be_a(HyperCast::Success)
    end

    it "round-trips a NumFormat through Marshal with its policy, and loads a 1.0 payload as legacy" do
      indian = invariant.with(grouping: :indian, currency: "₹")
      expect(Marshal.load(Marshal.dump(indian))).to eq(indian)
      expect(Marshal.load(Marshal.dump(indian)).grouping).to eq(:indian)
      expect(Marshal.load(Marshal.dump(invariant))).to eql(invariant)
      # Marshal.dump of 1.0's NumFormat (ten members, ",", ".", flags 95, the rest empty): the
      # struct form. It carries no policy, so it keeps 1.0's meaning, :legacy (it was legacy when
      # 1.0 created it; stored data does not change with the constants), where a NumFormat.new that
      # declares none is :western.
      payload = "\x04\bS:\x19HyperCast::NumFormat\x0F:\x10decimal_sepI\"\x06,\x06:\x06ET:\x0Egroup_sepI\"\x06.\x06;" \
                "\aT:\nflagsid:\rcurrencyI\"\x00\x06;\aT:\x19currency_decimal_sepI\"\x00\x06;\aT:\x17currency_" \
                "group_sepI\"\x00\x06;\aT:\x0Fminus_signI\"\x00\x06;\aT:\x0Eplus_signI\"\x00\x06;\aT:\fpercentI" \
                "\"\x00\x06;\aT:\x0Fdigit_zeroI\"\x00\x06;\aT".b
      old = Marshal.load(payload)
      current = HyperCast::NumFormat.new(decimal_sep: ",", group_sep: ".", flags: 95)
      expect(old.grouping).to eq(:legacy)
      expect(old).to eq(current.with(grouping: :legacy))
      expect(old).not_to eq(current)
      expect(old.to_h).to eq(current.to_h)
      expect(old.packed[4, 4].unpack1("L<")).to eq(95)
      expect(old.packed).to eq(current.with(grouping: :legacy).packed)
      expect(HyperCast.f64("1.234,5", old)).to eq(HyperCast::Success.new(value: 1234.5))
      expect(HyperCast.i32("12.34", old)).to eq(HyperCast::Success.new(value: 1234))
      expect(HyperCast.i32("12.34", current)).to be_a(HyperCast::Fault)
      # Dumped again, it is a 1.1 payload that keeps the policy it read as.
      expect(Marshal.load(Marshal.dump(old)).grouping).to eq(:legacy)
    end
  end

  describe "date options" do
    options = HyperCast::DateOptions
    names = HyperCast::MonthNames
    english = %w[Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec].each_with_index.to_h { |name, i| [name, i + 1] }

    it "validates its flags and pivot, and packs the 12-byte layout" do
      expect(options.new.flags).to eq(0)
      expect(options.new.pivot).to be_nil
      expect(options::COMMON_FORMS.flags).to eq(7)
      expect(options::COMMON_FORMS.pivot).to be_nil
      expect(options::COMMON_FORMS.with(pivot: 2029).packed.unpack("L3")).to eq([12, 7, 2029])
      expect(options.new(pivot: 0).pivot).to be_nil
      expect { options.new(flags: 8) }.to raise_error(ArgumentError, /date option flags 8 set a bit/)
      expect { options.new(flags: -1) }.to raise_error(ArgumentError, /set a bit/)
      expect { options.new(flags: "1") }.to raise_error(ArgumentError, /must be an Integer/)
      [1, 99, 10_000, 65_536, -5, 2029.5, "2029"].each do |pivot|
        expect { options.new(pivot: pivot) }.to raise_error(ArgumentError, /date pivot/), pivot.inspect
      end
      expect(options.new(pivot: 100).pivot).to eq(100)
      expect(options.new(pivot: 9999).pivot).to eq(9999)
    end

    it "opens each shape only when asked, at the doors that read it" do
      expect(HyperCast.date("20260107")).to be_a(HyperCast::Fault)
      expect(HyperCast.date("20260107", options: options.new(flags: options::COMPACT_DATE)).value)
        .to eq(Date.new(2026, 1, 7))
      expect(HyperCast.timestamp("2026-01-07 15:04:05Z")).to be_a(HyperCast::Fault)
      expect(HyperCast.timestamp("2026-01-07 15:04:05Z", options: options.new(flags: options::SPACE_SEPARATOR)))
        .to be_a(HyperCast::Success)
      expect(HyperCast.timestamp("2026-01-07T15:04:05+00")).to be_a(HyperCast::Fault)
      expect(HyperCast.timestamp("2026-01-07T15:04:05+00", options: options::COMMON_FORMS).value)
        .to eq(Time.utc(2026, 1, 7, 15, 4, 5))
      expect(HyperCast.date("1/7/26", :month_day_year)).to be_a(HyperCast::Fault)
      expect(HyperCast.date("1/7/26", :month_day_year).detail).to eq(HyperCast::Detail::FIELD_WIDTH)
      expect(HyperCast.date("1/7/26", :month_day_year, options: options.new(pivot: 2029)).value)
        .to eq(Date.new(2026, 1, 7))
      expect(HyperCast.datetime("1/7/26 3:04 PM", :month_day_year, options: options.new(pivot: 2029)).value.year)
        .to eq(2026)
      expect(HyperCast.date("20260107", options: options::COMMON_FORMS.with(pivot: 2029))).to be_a(HyperCast::Success)
    end

    it "reads month names where the declared order puts the month field, with and without options" do
      months = names.new(english)
      expect(HyperCast.date("7-Jan-2026", :day_month_year, months: months).value).to eq(Date.new(2026, 1, 7))
      expect(HyperCast.date("7-jan-2026", :day_month_year, months: months).value).to eq(Date.new(2026, 1, 7))
      expect(HyperCast.date("7-Jan-26", :day_month_year, months: months)).to be_a(HyperCast::Fault)
      expect(HyperCast.date("07-Jan-26", :day_month_year, options: options.new(pivot: 2029), months: months).value)
        .to eq(Date.new(2026, 1, 7))
      expect(HyperCast.datetime("7-Jan-2026 15:04", :day_month_year, months: months).value.hour).to eq(15)
      expect(HyperCast.date("7-Foo-2026", :day_month_year, months: months).detail)
        .to eq(HyperCast::Detail::NOT_RECOGNIZED)
      # A door that reads no vocabulary ignores one.
      expect(HyperCast.date("2026-01-07", months: months).value).to eq(Date.new(2026, 1, 7))
    end

    # The dotted name, the `.` separator and a pivot together: `janv.` is the name, `.` the separator.
    it "keeps punctuation a part of the name" do
      french = %w[janv. févr. mars avr. mai juin juil. août sept. oct. nov. déc.]
               .each_with_index.to_h { |name, i| [name, i + 1] }
      declared = options.new(pivot: 2029)
      months = names.new(french)
      expect(HyperCast.date("7.janv..26", :day_month_year, options: declared, months: months).value)
        .to eq(Date.new(2026, 1, 7))
      expect(HyperCast.date("7.janv.26", :day_month_year, options: declared, months: months))
        .to be_a(HyperCast::Fault)
    end

    it "types its arguments, and refuses a vocabulary any rule breaks" do
      expect { HyperCast.date("x", :year_month_day, options: 1) }.to raise_error(TypeError, /options must be a/)
      expect { HyperCast.date("x", :year_month_day, months: english) }.to raise_error(TypeError, /months must be a/)
      expect { HyperCast.timestamp("x", options: {}) }.to raise_error(TypeError, /options must be a/)
      expect { HyperCast.datetime("x", :year_month_day, options: 1) }.to raise_error(TypeError, /options must be a/)
      expect { names.new(english.merge("JAN" => 2)) }.to raise_error(ArgumentError, /fold to the same spelling/)
      expect { names.new(english.except("Dec")) }.to raise_error(ArgumentError, /every month; none for 12/)
      expect { names.new(english.merge("Ja1" => 1)) }.to raise_error(ArgumentError, /ASCII digit/)
      expect { names.new(english.merge("a b" => 1)) }.to raise_error(ArgumentError, /white space/)
      expect { names.new(english.merge("a" * 65 => 1)) }.to raise_error(ArgumentError, /1 to 64 UTF-8 bytes/)
      expect { names.new(english.merge("x" => 13)) }.to raise_error(ArgumentError, /Integer from 1 to 12/)
      expect { names.new(42) }.to raise_error(ArgumentError, /Hash or an Array/)
      expect { names.new([[1]]) }.to raise_error(ArgumentError, /\[month, name\] pair/)
      expect(names.new(english.merge("Sept" => 9)).entries.size).to eq(13)
    end

    # The core's refusal is the one thing core_judges relabels: a folded duplicate. Any other
    # ArgumentError, or a missing library's LoadError, goes up as it is.
    it "relabels only the core's refusal of a vocabulary, never another error" do
      allow(HyperCast).to receive(:date).and_raise(HyperCast::RefusedDeclaration, "refused")
      expect { names.new(english) }.to raise_error(ArgumentError, "two month names fold to the same spelling")
      allow(HyperCast).to receive(:date).and_raise(ArgumentError, "something else")
      expect { names.new(english) }.to raise_error(ArgumentError, "something else")
      allow(HyperCast).to receive(:date).and_raise(LoadError, "no library")
      expect { names.new(english) }.to raise_error(LoadError, "no library")
      expect(HyperCast::RefusedDeclaration.ancestors).to include(ArgumentError)
    end

    it "raises the core's refusal as RefusedDeclaration at a door, for a vocabulary built around Ruby's checks" do
      folded = names.allocate
      folded.instance_variable_set(:@packed, names.send(:pack, names.new(english).entries + [[2, "JAN"]]))
      expect { HyperCast.date("1/1/2026", :year_month_day, months: folded) }
        .to raise_error(HyperCast::RefusedDeclaration, "the core refused the date options or month names")
    end

    it "packs month, little-endian length and bytes, and reads its own packing back" do
      months = names.new([[1, "Jan"], [2, "Feb"], [3, "Mar"], [4, "Apr"], [5, "May"], [6, "Jun"], [7, "Jul"],
                          [8, "Aug"], [9, "Sep"], [10, "Oct"], [11, "Nov"], [12, "Dé"]])
      expect(months.packed.byteslice(0, 8)).to eq("\x01\x03\x00\x00\x00Jan".b)
      expect(months.packed.bytesize).to eq(11 * 8 + 5 + 3)
      expect(months.packed).to be_frozen
      expect(names.from_packed(months.packed)).to eq(months)
      expect(Marshal.load(Marshal.dump(months))).to eq(months)
      expect { names.from_packed(months.packed + "\x01".b) }.to raise_error(ArgumentError, /cut short/)
    end
  end

  # The 1.0 library (tag v1.0.0, built apart: HYPERCAST_CORE_1_0 names it) has none of the four
  # detailed exports. The Fiddle backend refuses it as it loads — and so a strict request is
  # never answered in legacy mode — and `available?` says so quietly.
  describe "the downgrade guard" do
    old_core = ENV.fetch("HYPERCAST_CORE_1_0", nil)

    it "refuses a 1.0 core rather than read a strict request as legacy" do
      skip "HYPERCAST_CORE_1_0 names no 1.0 libhypercast to load" unless old_core && File.exist?(old_core)
      script = <<~RUBY
        HyperCast::Runtime.define_singleton_method(:library_path) { #{old_core.inspect} }
        puts HyperCast.available?
        begin
          p HyperCast.i32("12,34", HyperCast::NumFormat::INVARIANT)
        rescue LoadError => error
          puts error.message
        end
      RUBY
      out, status = Open3.capture2e({ "HYPERCAST_PURE" => "1" }, RbConfig.ruby, "-I",
                                    File.expand_path("../lib", __dir__), "-r", "hypercast", "-e", script)
      expect(status).to be_success
      expect(out.lines.first.strip).to eq("false")
      expect(out).to match(/is core 1\.0\.0, which predates 1\.1/)
      expect(out).not_to include("Success")
    end

    it "loads the current core, which has the four exports" do
      expect(HyperCast::Runtime::DETAILED_EXPORTS.size).to eq(4)
      expect(HyperCast.available?).to be(true)
    end
  end
end
