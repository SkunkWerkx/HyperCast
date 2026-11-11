using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using HyperCast.Interop;

namespace HyperCast.Tests;

/// <summary>
/// The public interop surface a library carrying HyperCast's verdicts across its own C ABI
/// reads with: every conversion presents what the matching Cast door presents, and every
/// code decodes to exactly the member it names.
/// </summary>
public sealed class InteropTests
{
	static T Ok<T>(Verdict<T> verdict) where T : struct =>
		verdict.TryGetValue(out Success<T> success) ? success.Value : throw new InvalidOperationException($"{verdict}");

	[Fact]
	void Declared_options_encode_to_their_discriminants_and_decode_back()
	{
		foreach (var precision in new[] { UnixPrecision.Seconds, UnixPrecision.Milliseconds, UnixPrecision.Microseconds, UnixPrecision.Nanoseconds })
			Abi.UnixPrecisionFrom(Abi.Code(precision)).ShouldBe(precision);
		foreach (var order in new[] { DateOrder.YearMonthDay, DateOrder.MonthDayYear, DateOrder.DayMonthYear })
			Abi.DateOrderFrom(Abi.Code(order)).ShouldBe(order);
		foreach (var epoch in new[] { ExcelEpoch.Y1900, ExcelEpoch.Y1904 })
			Abi.ExcelEpochFrom(Abi.Code(epoch)).ShouldBe(epoch);
		foreach (var reason in new[] { CastFailure.Empty, CastFailure.Malformed, CastFailure.OutOfRange })
			Abi.ReasonFrom((uint)reason).ShouldBe(reason);

		foreach (var outside in new uint[] { 0, 5, uint.MaxValue })
		{
			Abi.UnixPrecisionFrom(outside).ShouldBeNull();
			Abi.DateOrderFrom(outside).ShouldBeNull();
			Abi.ExcelEpochFrom(outside).ShouldBeNull();
			Abi.ReasonFrom(outside).ShouldBeNull();
		}
		Abi.DateOrderFrom(4).ShouldBeNull();
		Abi.ExcelEpochFrom(3).ShouldBeNull();
		Abi.ReasonFrom(4).ShouldBeNull();
	}

	[Fact]
	void An_undefined_option_names_the_callers_parameter()
	{
		var precision = UnixPrecision.Unspecified;
		Should.Throw<ArgumentOutOfRangeException>(() => Abi.Code(precision)).ParamName.ShouldBe("precision");
		var order = (DateOrder)9;
		Should.Throw<ArgumentOutOfRangeException>(() => Abi.Code(order)).ParamName.ShouldBe("order");
		Should.Throw<ArgumentOutOfRangeException>(() => Abi.Code(ExcelEpoch.Unspecified, "plan")).ParamName.ShouldBe("plan");
	}

	[Fact]
	void A_fault_keeps_its_span_and_a_code_that_names_no_reason_is_refused()
	{
		Abi.ToFault(2, new RawFault(3, 4)).ShouldBe(new Fault(CastFailure.Malformed, 3, 4));
		Should.Throw<InvalidOperationException>(() => Abi.ToFault(0, default));
		Should.Throw<InvalidOperationException>(() => Abi.ToFault(7, default));
	}

	[Fact]
	void Raw_values_present_as_the_doors_present_them()
	{
		new RawTimestamp(1_767_366_245, 123_456_789).ToDateTimeOffset()
			.ShouldBe(Ok(Cast.Timestamp("2026-01-02T15:04:05.123456789Z")));
		new RawDate(2026, 1, 7).ToDateOnly().ShouldBe(new DateOnly(2026, 1, 7));
		new RawCivil(2026, 1, 7, 54_245_123_456_789).ToDateTime()
			.ShouldBe(Ok(Cast.DateTime("2026-01-07 15:04:05.123456789", DateOrder.YearMonthDay)));
		new RawDuration(-1, -500_000_000).ToTimeSpan().ShouldBe(TimeSpan.FromSeconds(-1.5));
		new RawDecimal(12_345, 0, 2, 1).ToDecimal().ShouldBe(-123.45m);
		Abi.ToTimeOnly(54_245_000_000_000).ShouldBe(new TimeOnly(15, 4, 5));
		Abi.ToGuid([0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00])
			.ShouldBe(Guid.Parse("550e8400-e29b-41d4-a716-446655440000"));
	}

	[Fact]
	void Versions_unpack_and_a_library_that_does_not_load_probes_as_absent()
	{
		Abi.ToVersion(0x00_06_02).ShouldBe(new Version(0, 6, 2));
		Abi.ProbeVersion(() => 0x01_02_03).ShouldBe(new Version(1, 2, 3));
		Abi.ProbeVersion(() => throw new DllNotFoundException()).ShouldBeNull();
		Should.Throw<InvalidOperationException>(() => Abi.ProbeVersion(() => throw new InvalidOperationException()));
	}

	[Fact]
	void A_format_packs_into_the_cores_layout()
	{
		// Every field declared, each a different width, so a slip in any offset shows: the
		// 92 bytes are read back as the core reads them — the three words, then each
		// symbol's bytes at its offset and its length byte at the end of its slot.
		var raw = new NumFormat(",", ". ", NumStyles.All | NumStyles.TrailingSign, "€")
		{
			CurrencyDecimalSeparator = ".",
			CurrencyGroupSeparator = ",",
			MinusSign = "−",
			PlusSign = "＋",
			PercentSymbol = "٪",
			DigitZero = (Rune)'٠',
		}.ToRaw();
		Unsafe.SizeOf<RawNumFormat>().ShouldBe(92);
		var bytes = MemoryMarshal.AsBytes(MemoryMarshal.CreateReadOnlySpan(in raw, 1));
		bytes.Length.ShouldBe(92);
		MemoryMarshal.Read<uint>(bytes).ShouldBe(92u);
		MemoryMarshal.Read<uint>(bytes[4..]).ShouldBe(223u); // the styles; Western sets no bit
		MemoryMarshal.Read<uint>(bytes[8..]).ShouldBe(0x0660u);
		Symbol(bytes, 12, 9).ShouldBe(",");
		Symbol(bytes, 21, 9).ShouldBe(". ");
		Symbol(bytes, 30, 9).ShouldBe(".");
		Symbol(bytes, 39, 9).ShouldBe(",");
		Symbol(bytes, 48, 9).ShouldBe("−");
		Symbol(bytes, 57, 9).ShouldBe("＋");
		Symbol(bytes, 66, 9).ShouldBe("٪");
		Symbol(bytes, 75, 17).ShouldBe("€");
		// Nothing declared past the separators is all zero, length bytes included.
		var invariant = NumFormat.Invariant.ToRaw();
		var invariantBytes = MemoryMarshal.AsBytes(MemoryMarshal.CreateReadOnlySpan(in invariant, 1));
		invariantBytes[30..].ToArray().ShouldAllBe(static b => b == 0);
		invariantBytes[20].ShouldBe((byte)1);
		invariantBytes[29].ShouldBe((byte)1);
	}

	// The symbol at `offset` in a layout: its bytes, as many as the length byte at the end
	// of its `slot` says.
	static string Symbol(ReadOnlySpan<byte> layout, int offset, int slot) =>
		Encoding.UTF8.GetString(layout.Slice(offset, layout[offset + slot - 1]));

	[Fact]
	void Day_periods_pack_into_the_cores_layout()
	{
		var raw = new DateFormat(DateOrder.YearMonthDay, "오전", "p. m.").ToRaw();
		Unsafe.SizeOf<RawDayPeriods>().ShouldBe(40);
		var bytes = MemoryMarshal.AsBytes(MemoryMarshal.CreateReadOnlySpan(in raw, 1));
		bytes.Length.ShouldBe(40);
		MemoryMarshal.Read<uint>(bytes).ShouldBe(40u);
		Symbol(bytes, 4, 17).ShouldBe("오전");
		Symbol(bytes, 21, 17).ShouldBe("p. m.");
		bytes[38..].ToArray().ShouldAllBe(static b => b == 0);
		// Nothing declared is a layout of zero lengths, which the door never hands over.
		var english = new DateFormat(DateOrder.YearMonthDay);
		english.HasDesignators.ShouldBeFalse();
		english.ToRaw().Am.Length.ShouldBe((byte)0);
		new DateFormat(DateOrder.YearMonthDay, "", "nachm.").HasDesignators.ShouldBeTrue();
	}

	[Fact]
	void A_format_the_core_would_refuse_is_a_caller_bug_here()
	{
		// Each rule of the core's own check, caught before the crossing, against the
		// parameter the doors take the format in.
		static void Refused(NumFormat format) =>
			Should.Throw<ArgumentException>(() => format.ToRaw()).ParamName.ShouldBe("format");
		Refused(default);
		Refused(new NumFormat(".", ".", NumStyles.None));
		Refused(new NumFormat("", ",", NumStyles.None));
		Refused(new NumFormat(".", "", NumStyles.None));
		Refused(new NumFormat(".", ",,,,,,,,,", NumStyles.None));
		Refused(new NumFormat(".", "1", NumStyles.None));
		Refused(new NumFormat(".", "\uD800", NumStyles.None));
		Refused(NumFormat.Invariant with { CurrencyDecimalSeparator = "," });
		Refused(NumFormat.Invariant with { CurrencyGroupSeparator = "." });
		Refused(NumFormat.Invariant with { CurrencyDecimalSeparator = ";", CurrencyGroupSeparator = ";" });
		Refused(NumFormat.Invariant with { MinusSign = "- " });
		Refused(NumFormat.Invariant with { PlusSign = "+1" });
		Refused(NumFormat.Invariant with { PercentSymbol = "％％％" });
		Refused(NumFormat.Invariant with { CurrencySymbol = "US 1" });
		Refused(NumFormat.Invariant with { CurrencySymbol = new string('€', 6) });
		Refused(NumFormat.Invariant with { DigitZero = (Rune)'0' });
		Refused(NumFormat.Invariant with { DigitZero = new Rune(0x0677) });
		// White space of any kind: in a sign or the percent symbol, at either edge of the
		// currency symbol (the input's white space around them is ignored, so they could never
		// match), and in either decimal separator (a group separator may be one).
		foreach (var space in new[] { " ", "\u00A0", "\u202F", "\u3000", "\v", "\u0085" })
		{
			Refused(NumFormat.Invariant with { MinusSign = space });
			Refused(NumFormat.Invariant with { PlusSign = "+" + space });
			Refused(NumFormat.Invariant with { PercentSymbol = space + "%" });
			Refused(NumFormat.Invariant with { CurrencySymbol = space });
			Refused(NumFormat.Invariant with { CurrencySymbol = space + "F" });
			Refused(NumFormat.Invariant with { CurrencySymbol = "F" + space });
			Refused(NumFormat.Invariant with { CurrencySymbol = space + "F" + space + "CFA" });
			// Inside, white space is the symbol's own (CLDR's `F CFA`), whatever kind.
			(NumFormat.Invariant with { CurrencySymbol = "F" + space + "CFA" }).ToRaw();
			Refused(new NumFormat(space, ",", NumStyles.None));
			Refused(new NumFormat("x" + space, ",", NumStyles.None));
			Refused(NumFormat.Invariant with { CurrencyDecimalSeparator = space, CurrencyGroupSeparator = "." });
			Refused(NumFormat.Invariant with { CurrencyDecimalSeparator = "," + space, CurrencyGroupSeparator = "." });
			new NumFormat(",", space, NumStyles.All).ToRaw();
			(NumFormat.Invariant with { CurrencyDecimalSeparator = ",", CurrencyGroupSeparator = space }).ToRaw();
		}
		Should.Throw<ArgumentException>(() => (NumFormat.Invariant with { MinusSign = "\u00A0" }).ToRaw())
			.Message.ShouldBe("Minus sign '\u00A0' must not contain white space. (Parameter 'format')");
		Should.Throw<ArgumentException>(() => (NumFormat.Invariant with { CurrencySymbol = "\u00A0F" }).ToRaw())
			.Message.ShouldBe("Currency symbol '\u00A0F' must not start or end with white space. (Parameter 'format')");
		Should.Throw<ArgumentException>(() => (NumFormat.Invariant with { CurrencySymbol = "F\u202F" }).ToRaw())
			.Message.ShouldBe("Currency symbol 'F\u202F' must not start or end with white space. (Parameter 'format')");
		Should.Throw<ArgumentException>(() => (NumFormat.Invariant with { CurrencySymbol = new string('\u00A0', 9) }).ToRaw())
			.Message.ShouldContain("UTF-8 bytes");
		Should.Throw<ArgumentException>(() => (NumFormat.Invariant with { CurrencySymbol = "\u00A0$1" }).ToRaw())
			.Message.ShouldContain("digit");
		foreach (var (inside, bytes) in new[] { (" ", 5), ("\u00A0", 6), ("\u202F", 7) })
			(NumFormat.Invariant with { CurrencySymbol = "F" + inside + "CFA" }).ToRaw().Currency.Length.ShouldBe((byte)bytes);
		Should.Throw<ArgumentException>(() => new NumFormat("\u202F", ",", NumStyles.None).ToRaw())
			.Message.ShouldBe("Decimal separator '\u202F' must not contain white space. (Parameter 'format')");
		// The order of the core's check stands: length, then digit, then the separators' equality, then white space.
		Should.Throw<ArgumentException>(() => new NumFormat(new string('\u00A0', 5), ",", NumStyles.None).ToRaw())
			.Message.ShouldContain("UTF-8 bytes");
		Should.Throw<ArgumentException>(() => new NumFormat("\u00A01", ",", NumStyles.None).ToRaw())
			.Message.ShouldContain("digit");
		Should.Throw<ArgumentException>(() => new NumFormat("\u00A0", "\u00A0", NumStyles.None).ToRaw())
			.Message.ShouldContain("must differ");
		// A comma-space separator is two characters and fine; so is a separator with a
		// non-ASCII space, and a sign with a bidi mark.
		new NumFormat(".", ", ", NumStyles.All).ToRaw().GroupSep.Length.ShouldBe((byte)2);
		new NumFormat(",", "\u202F", NumStyles.All).ToRaw().GroupSep.Length.ShouldBe((byte)3);
		(NumFormat.Invariant with { MinusSign = "\u061C-" }).ToRaw().MinusSign.Length.ShouldBe((byte)3);
		(NumFormat.Invariant with { DigitZero = new Rune(0x11136) }).ToRaw().DigitZero.ShouldBe(0x11136u);

		static void RefusedPeriods(DateFormat format) =>
			Should.Throw<ArgumentException>(() => format.ToRaw()).ParamName.ShouldBe("format");
		RefusedPeriods(new DateFormat(DateOrder.YearMonthDay, " am", "pm"));
		RefusedPeriods(new DateFormat(DateOrder.YearMonthDay, "am", "pm "));
		RefusedPeriods(new DateFormat(DateOrder.YearMonthDay, "\u00A0am", "pm"));
		RefusedPeriods(new DateFormat(DateOrder.YearMonthDay, "am", "pm\u202F"));
		RefusedPeriods(new DateFormat(DateOrder.YearMonthDay, "\u3000", "pm"));
		Should.Throw<ArgumentException>(() => new DateFormat(DateOrder.YearMonthDay, "am\u00A0", "pm").ToRaw())
			.Message.ShouldBe("AM designator 'am\u00A0' must not start or end with white space. (Parameter 'format')");
		RefusedPeriods(new DateFormat(DateOrder.YearMonthDay, "a1", "pm"));
		RefusedPeriods(new DateFormat(DateOrder.YearMonthDay, "am", new string('오', 6)));
		RefusedPeriods(new DateFormat(DateOrder.YearMonthDay, "\uDC00", "pm"));
		new DateFormat(DateOrder.YearMonthDay, "a. m.", "p. m.").ToRaw().Pm.Length.ShouldBe((byte)5);
		new DateFormat(DateOrder.YearMonthDay, "a.\u202Fm.", "p.\u00A0m.").ToRaw().Am.Length.ShouldBe((byte)7);
	}

	// include/hypercast.h, or null where the tree holds none: the musl suite stages only
	// rust/Cargo.toml, corpus/ and csharp/, so its run asserts the literals below alone, and
	// every leg that has the header proves each literal is the header's.
	static readonly string? _header = FindHeader() is { } path ? File.ReadAllText(path) : null;

	static string? FindHeader()
	{
		for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
		{
			var header = Path.Combine(dir.FullName, "include", "hypercast.h");
			if (File.Exists(header))
				return header;
		}
		return null;
	}

	// The value this binding restates for a `#define NAME value` of the generated header,
	// held to that define wherever the header is there to read.
	static uint Define(string name, uint restated)
	{
		if (_header is null)
			return restated;
		var match = System.Text.RegularExpressions.Regex.Match(_header, $@"^#define {name} (\d+)$",
			System.Text.RegularExpressions.RegexOptions.Multiline);
		match.Success.ShouldBeTrue($"{name} is not in include/hypercast.h");
		uint.Parse(match.Groups[1].Value, System.Globalization.CultureInfo.InvariantCulture).ShouldBe(restated, name);
		return restated;
	}

	[Fact]
	void The_constraint_layouts_are_the_headers()
	{
		// Each restated layout, against the size cbindgen carries from the core's offset_of! asserts.
		((uint)Unsafe.SizeOf<RawConstraints>()).ShouldBe(Define("HyperCastRawConstraints_SIZE", 104));
		((uint)Unsafe.SizeOf<RawCheck>()).ShouldBe(Define("HyperCastRawCheck_SIZE", 12));
		((uint)Unsafe.SizeOf<RawSpan>()).ShouldBe(Define("HyperCastRawSpan_SIZE", 8));
		((uint)Unsafe.SizeOf<RawNumFormat>()).ShouldBe(Define("HyperCastRawNumFormat_SIZE", 92));
		((uint)Unsafe.SizeOf<RawDayPeriods>()).ShouldBe(Define("HyperCastRawDayPeriods_SIZE", 40));
		string[] fields = ["Size", "Flags", "Rounding", "MinLength", "MaxLength", "Reserved", "Lower", "Upper", "MultipleOf",
			"ExclusiveLower", "ExclusiveUpper"];
		fields.Select(static f => (int)Marshal.OffsetOf<RawConstraints>(f)).ShouldBe([0, 4, 8, 12, 16, 20, 24, 40, 56, 72, 88]);
		new[] { "Keyword", "Adjusted", "EnumIndex" }.Select(static f => (int)Marshal.OffsetOf<RawCheck>(f)).ShouldBe([0, 4, 8]);
		new[] { "Offset", "Length" }.Select(static f => (int)Marshal.OffsetOf<RawSpan>(f)).ShouldBe([0, 4]);
		(RawConstraints.HasMinimum, RawConstraints.HasExclusiveMinimum, RawConstraints.HasMaximum, RawConstraints.HasExclusiveMaximum)
			.ShouldBe((Define("HyperCastRawConstraints_HAS_MINIMUM", 1), Define("HyperCastRawConstraints_HAS_EXCLUSIVE_MINIMUM", 2),
				Define("HyperCastRawConstraints_HAS_MAXIMUM", 4), Define("HyperCastRawConstraints_HAS_EXCLUSIVE_MAXIMUM", 8)));
		(RawConstraints.HasMultipleOf, RawConstraints.HasMinLength, RawConstraints.HasMaxLength, RawConstraints.Trim)
			.ShouldBe((Define("HyperCastRawConstraints_HAS_MULTIPLE_OF", 16), Define("HyperCastRawConstraints_HAS_MIN_LENGTH", 32),
				Define("HyperCastRawConstraints_HAS_MAX_LENGTH", 64), Define("HyperCastRawConstraints_TRIM", 128)));
		(RawConstraints.HasEnum, RawConstraints.CaseSensitive)
			.ShouldBe((Define("HyperCastRawConstraints_HAS_ENUM", 256), Define("HyperCastRawConstraints_CASE_SENSITIVE", 512)));
	}

	[Fact]
	void A_declaration_packs_into_the_cores_layout()
	{
		var raw = new Constraints<decimal>(exclusiveMinimum: -1.5m, maximum: "79228162514264337593543950335",
			multipleOf: 0.25m, rounding: Rounding.Floor).ToRaw();
		var bytes = MemoryMarshal.AsBytes(MemoryMarshal.CreateReadOnlySpan(in raw, 1));
		bytes.Length.ShouldBe(104);
		MemoryMarshal.Read<uint>(bytes).ShouldBe(104u);
		MemoryMarshal.Read<uint>(bytes[4..]).ShouldBe(RawConstraints.HasExclusiveMinimum | RawConstraints.HasMaximum | RawConstraints.HasMultipleOf);
		MemoryMarshal.Read<uint>(bytes[8..]).ShouldBe(2u);
		bytes[12..24].ToArray().ShouldAllBe(static b => b == 0);
		// Each decimal: the 64 low bits, the 32 high bits, the scale, the sign, two of padding.
		// `minimum`'s slot stays zero: the exclusive bound has a slot of its own.
		bytes[24..40].ToArray().ShouldAllBe(static b => b == 0);
		MemoryMarshal.Read<ulong>(bytes[40..]).ShouldBe(ulong.MaxValue);
		MemoryMarshal.Read<uint>(bytes[48..]).ShouldBe(uint.MaxValue);
		(bytes[52], bytes[53]).ShouldBe(((byte)0, (byte)0));
		MemoryMarshal.Read<ulong>(bytes[56..]).ShouldBe(25ul);
		(bytes[68], bytes[69]).ShouldBe(((byte)2, (byte)0));
		MemoryMarshal.Read<ulong>(bytes[72..]).ShouldBe(15ul);
		MemoryMarshal.Read<uint>(bytes[80..]).ShouldBe(0u);
		(bytes[84], bytes[85]).ShouldBe(((byte)1, (byte)1));
		bytes[88..104].ToArray().ShouldAllBe(static b => b == 0);

		// All four bounds at once, each in its own slot.
		var four = new Constraints<int>(minimum: 1, exclusiveMinimum: 2, maximum: 3, exclusiveMaximum: 4).ToRaw();
		(four.Lower.ToDecimal(), four.ExclusiveLower.ToDecimal(), four.Upper.ToDecimal(), four.ExclusiveUpper.ToDecimal())
			.ShouldBe((1m, 2m, 3m, 4m));
		four.Flags.ShouldBe(RawConstraints.HasMinimum | RawConstraints.HasExclusiveMinimum | RawConstraints.HasMaximum
			| RawConstraints.HasExclusiveMaximum);

		var integers = new Constraints<long>(@enum: [-3, 9]).PackedEnum;
		integers.Length.ShouldBe(32);
		MemoryMarshal.Read<RawDecimal>(integers).ToDecimal().ShouldBe(-3m);
		MemoryMarshal.Read<RawDecimal>(integers[16..]).ToDecimal().ShouldBe(9m);

		var text = new Constraints<string>(minLength: 1, maxLength: 4, trim: true, caseSensitive: true, @enum: ["é", "ab"]);
		text.ToRaw().Flags.ShouldBe(RawConstraints.HasMinLength | RawConstraints.HasMaxLength | RawConstraints.Trim
			| RawConstraints.CaseSensitive | RawConstraints.HasEnum);
		text.CaseSensitive.ShouldBeTrue();
		new Constraints<string>(@enum: ["ab"]).CaseSensitive.ShouldBeFalse();
		(text.ToRaw().MinLength, text.ToRaw().MaxLength).ShouldBe((1u, 4u));
		text.PackedEnum.ToArray().ShouldBe([2, 0, 0, 0, 0xC3, 0xA9, 2, 0, 0, 0, (byte)'a', (byte)'b']);
		new Constraints<string>().PackedEnum.Length.ShouldBe(0);
	}

	[Fact]
	void A_violation_decodes_from_its_keyword_code()
	{
		Abi.ToViolation(4, 250L, new RawFault(2, 3)).ShouldBe(new Violation<long>(Keyword.ExclusiveMaximum, 250, 2, 3));
		Abi.ToViolation(8, "Mail", RawFaultDetail.Empty with { Offset = 1, Length = 4 })
			.ShouldBe(new Violation<string>(Keyword.Enum, "Mail", 1, 4));
		Should.Throw<InvalidOperationException>(() => Abi.ToViolation(0, 0, default(RawFault)));
	}
}
