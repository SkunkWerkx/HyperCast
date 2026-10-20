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
		MemoryMarshal.Read<uint>(bytes[4..]).ShouldBe(223u);
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
		RefusedPeriods(new DateFormat(DateOrder.YearMonthDay, "a1", "pm"));
		RefusedPeriods(new DateFormat(DateOrder.YearMonthDay, "am", new string('오', 6)));
		RefusedPeriods(new DateFormat(DateOrder.YearMonthDay, "\uDC00", "pm"));
		new DateFormat(DateOrder.YearMonthDay, "a. m.", "p. m.").ToRaw().Pm.Length.ShouldBe((byte)5);
	}
}
