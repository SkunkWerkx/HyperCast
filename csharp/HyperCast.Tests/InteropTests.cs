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
		var raw = new NumFormat(',', '.', NumStyles.All, "€").ToRaw();
		raw.DecimalSep.ShouldBe((uint)',');
		raw.GroupSep.ShouldBe((uint)'.');
		raw.Flags.ShouldBe((uint)NumStyles.All);
		raw.CurrencyLen.ShouldBe(3u);
		((ReadOnlySpan<byte>)raw.Currency)[..3].ToArray().ShouldBe("€"u8.ToArray());
		Should.Throw<ArgumentException>(() => new NumFormat('.', '.', NumStyles.None).ToRaw());
	}
}
