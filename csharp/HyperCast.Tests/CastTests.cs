using System.Globalization;

namespace HyperCast.Tests;

/// <summary>
/// Binding-level behavior the corpus can't express: the union's compile-checked consumption,
/// culture-to-<see cref="NumFormat"/> bridging, .NET-type fidelity mapping, the UTF-16 doors'
/// transcode path, and the caller-bug guards.
/// </summary>
public sealed class CastTests
{
	[Fact]
	void Verdict_switch_expression_is_exhaustive_with_two_arms()
	{
		// This compiling at all is the feature: CS8509 is an error in this repo, so these
		// two arms with no default are proven exhaustive by the compiler.
		var rendered = Cast.Int32("42", NumFormat.Invariant) switch
		{
			Success<int> success => $"ok {success.Value}",
			Fault fault => $"fault {fault.Reason}",
		};
		rendered.ShouldBe("ok 42");
	}

	[Fact]
	void Verdict_pattern_matches_case_types_directly()
	{
		(Cast.Boolean("yes") is Success<bool> { Value: true }).ShouldBeTrue();
		(Cast.Boolean("maybe") is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
	}

	[Fact]
	void Defaulted_verdict_matches_neither_case()
	{
		var defaulted = default(Verdict<int>);
		defaulted.HasValue.ShouldBeFalse();
		(defaulted is Success<int>).ShouldBeFalse();
		(defaulted is Fault).ShouldBeFalse();
	}

	[Fact]
	void Defaulted_fault_is_rejected_as_a_case_value() =>
		Should.Throw<ArgumentOutOfRangeException>(() => new Verdict<int>(default(Fault)));

	[Fact]
	void NumFormat_bridges_from_a_real_culture()
	{
		var german = NumFormat.From(CultureInfo.GetCultureInfo("de-DE"));
		german.DecimalSeparator.ShouldBe(',');
		german.GroupSeparator.ShouldBe('.');
		(Cast.Double("1.234,5", german) is Success<double> { Value: 1234.5 }).ShouldBeTrue();
	}

	[Fact]
	void Utf16_doors_transcode_non_ascii_separators()
	{
		var french = new NumFormat(',', ' ', NumStyles.All);
		(Cast.Double("1 234,5", french) is Success<double> { Value: 1234.5 }).ShouldBeTrue();
	}

	[Fact]
	void Utf16_doors_survive_inputs_past_the_stack_buffer()
	{
		var oversized = new string('x', 4096);
		(Cast.Boolean(oversized) is Fault { Reason: CastFailure.Malformed, Offset: 0, Length: 4096 })
			.ShouldBeTrue();
	}

	[Fact]
	void Fault_span_points_at_the_offending_byte()
	{
		(Cast.Int32("  12x4", NumFormat.Invariant) is Fault { Reason: CastFailure.Malformed, Offset: 4, Length: 1 })
			.ShouldBeTrue();
	}

	[Fact]
	void Uuid_agrees_with_the_platforms_own_parser()
	{
		const string text = "01020304-0506-0708-090a-0b0c0d0e0f10";
		Cast.Uuid(text).ShouldBe(new Verdict<Guid>(new Success<Guid>(Guid.Parse(text))));
		Cast.Uuid($"urn:uuid:{text}").ShouldBe(new Verdict<Guid>(new Success<Guid>(Guid.Parse(text))));
	}

	[Fact]
	void Timestamp_maps_to_a_utc_datetimeoffset_at_tick_fidelity()
	{
		var expected = new DateTimeOffset(2026, 1, 2, 15, 4, 5, TimeSpan.Zero).AddTicks(1234567);
		(Cast.Timestamp("2026-01-02T15:04:05.1234567Z") is Success<DateTimeOffset> success).ShouldBeTrue();
		Cast.Timestamp("2026-01-02T15:04:05.1234567Z").ShouldBe(new(new Success<DateTimeOffset>(expected)));
		// Offset input normalizes to UTC — 15:04:05+05:00 is 10:04:05Z.
		Cast.Timestamp("2026-01-02T15:04:05+05:00")
			.ShouldBe(new(new Success<DateTimeOffset>(new(2026, 1, 2, 10, 4, 5, TimeSpan.Zero))));
		// Sub-tick nanoseconds (the core's extra fidelity) truncate to the tick.
		Cast.Timestamp("2026-01-02T15:04:05.123456789Z")
			.ShouldBe(new(new Success<DateTimeOffset>(
				new DateTimeOffset(2026, 1, 2, 15, 4, 5, TimeSpan.Zero).AddTicks(1234567))));
	}

	[Fact]
	void Unix_maps_the_declared_precision()
	{
		Cast.Unix("-1", UnixPrecision.Seconds)
			.ShouldBe(new(new Success<DateTimeOffset>(DateTimeOffset.UnixEpoch.AddSeconds(-1))));
		Cast.Unix("1700000000123", UnixPrecision.Milliseconds)
			.ShouldBe(new(new Success<DateTimeOffset>(DateTimeOffset.FromUnixTimeMilliseconds(1_700_000_000_123))));
	}

	[Fact]
	void Date_order_disambiguates_like_the_cultures_do()
	{
		// The canonical ambiguity: 1/7/2026 is January 7th under en-US's month-first short
		// dates and July 1st under en-GB's day-first ones — resolved only by declaration,
		// with the declaration derived from the real cultures' own short-date patterns.
		var enUs = DateOrders.From(CultureInfo.GetCultureInfo("en-US"));
		var enGb = DateOrders.From(CultureInfo.GetCultureInfo("en-GB"));
		enUs.ShouldBe(DateOrder.MonthDayYear);
		enGb.ShouldBe(DateOrder.DayMonthYear);
		(Cast.Date("1/7/2026", enUs) is Success<DateOnly> { Value: var us } && us == new DateOnly(2026, 1, 7))
			.ShouldBeTrue();
		(Cast.Date("1/7/2026", enGb) is Success<DateOnly> { Value: var gb } && gb == new DateOnly(2026, 7, 1))
			.ShouldBeTrue();
		// ISO-order cultures land on YearMonthDay; the ISO form is that order's subset.
		DateOrders.From(CultureInfo.GetCultureInfo("ja-JP")).ShouldBe(DateOrder.YearMonthDay);
		// Undeclared, the door stays strict ISO — the ambiguity is never guessed at.
		(Cast.Date("1/7/2026") is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		Should.Throw<ArgumentOutOfRangeException>(() => Cast.Date("1/7/2026", DateOrder.Unspecified));
	}

	[Fact]
	void Datetime_reads_the_messy_civil_shapes()
	{
		// The AM/PM world, zone-less: Kind is honestly Unspecified because the text named
		// no zone — fusing one is the caller's job, never the parser's guess.
		var enUs = DateOrders.From(CultureInfo.GetCultureInfo("en-US"));
		(Cast.DateTime("1/7/2026 3:04 PM", enUs) is Success<DateTime>
			{ Value: { Kind: DateTimeKind.Unspecified } value } && value == new DateTime(2026, 1, 7, 15, 4, 0))
			.ShouldBeTrue();
		(Cast.DateTime("1/7/2026 3:04 PM", DateOrder.DayMonthYear) is Success<DateTime> { Value: var gb }
			&& gb == new DateTime(2026, 7, 1, 15, 4, 0)).ShouldBeTrue();
		// A zone suffix is not this door's business — Timestamp is the instant door.
		(Cast.DateTime("1/7/2026 15:04:05Z", enUs) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
	}

	[Fact]
	void Date_time_and_duration_map_to_their_dotnet_types()
	{
		Cast.Date("2026-01-02").ShouldBe(new(new Success<DateOnly>(new(2026, 1, 2))));
		Cast.Time("15:04:05.1234567").ShouldBe(new(new Success<TimeOnly>(
			new TimeOnly(15, 4, 5).Add(TimeSpan.FromTicks(1234567)))));
		Cast.Duration("P1DT6H").ShouldBe(new(new Success<TimeSpan>(new(1, 6, 0, 0))));
		Cast.Duration("-1.5s").ShouldBe(new(new Success<TimeSpan>(TimeSpan.FromSeconds(-1.5))));
		Cast.Duration("01:30").ShouldBe(new(new Success<TimeSpan>(new(1, 30, 0))));
	}

	[Fact]
	void Optional_presents_empty_as_absent_and_everything_else_verbatim()
	{
		Cast.Optional(Cast.Int32("   ", NumFormat.Invariant)).ShouldBeNull();
		Cast.Optional(Cast.Int32("42", NumFormat.Invariant))
			.ShouldBe(new Verdict<int>(new Success<int>(42)));
		Cast.Optional(Cast.Int32("abc", NumFormat.Invariant))!.Value
			.TryGetValue(out Fault fault).ShouldBeTrue();
		fault.Reason.ShouldBe(CastFailure.Malformed);
	}

	[Fact]
	void NumFormat_bridges_from_any_format_provider()
	{
		// The shape every BCL TryParse takes — no CultureInfo cast at the call site.
		IFormatProvider provider = CultureInfo.GetCultureInfo("de-DE");
		NumFormat.From(provider).ShouldBe(NumFormat.From(CultureInfo.GetCultureInfo("de-DE")));
		NumFormat.From(NumberFormatInfo.InvariantInfo).CurrencySymbol.ShouldBe("¤");
		NumFormat.From(CultureInfo.InvariantCulture).ShouldBe(NumFormat.From(NumberFormatInfo.InvariantInfo));
	}

	[Fact]
	void Currency_symbol_comes_from_the_culture_and_is_matched_at_either_edge()
	{
		var usd = NumFormat.From(CultureInfo.GetCultureInfo("en-US"));
		usd.CurrencySymbol.ShouldBe("$");
		(Cast.Int32("$1,234", usd) is Success<int> { Value: 1234 }).ShouldBeTrue();
		(Cast.Int32("-$5", usd) is Success<int> { Value: -5 }).ShouldBeTrue();
		(Cast.Int32("($5)", usd) is Success<int> { Value: -5 }).ShouldBeTrue();
		(Cast.Double("$1,234.50", usd) is Success<double> { Value: 1234.5 }).ShouldBeTrue();
		(Cast.Decimal("$1,234.50", usd) is Success<decimal> { Value: 1234.50m }).ShouldBeTrue();
		var euro = NumFormat.From(CultureInfo.GetCultureInfo("de-DE"));
		euro.CurrencySymbol.ShouldBe("€");
		(Cast.Double("1.234,50 €", euro) is Success<double> { Value: 1234.5 }).ShouldBeTrue();
		// Declared but not permitted: the symbol is a stray character like any other.
		var declaredOff = usd with { Styles = NumStyles.All & ~NumStyles.Currency };
		(Cast.Int32("$5", declaredOff) is Fault { Reason: CastFailure.Malformed, Offset: 0, Length: 1 }).ShouldBeTrue();
		// Permitted but nothing declared: the invariant profile is unchanged by the flag.
		(Cast.Int32("$5", NumFormat.Invariant) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
	}

	[Fact]
	void Currency_symbol_that_would_collide_with_the_scan_is_a_caller_bug()
	{
		Should.Throw<ArgumentException>(() => Cast.Int32("5", NumFormat.Invariant with { CurrencySymbol = "US 1" }));
		Should.Throw<ArgumentException>(() => Cast.Int32("5", NumFormat.Invariant with { CurrencySymbol = new string('€', 6) }));
		// Multi-byte, multi-character symbols that fit are fine — and may contain a separator.
		var danish = new NumFormat(',', '.', NumStyles.All, "kr.");
		(Cast.Int64("1.234.567 kr.", danish) is Success<long> { Value: 1_234_567 }).ShouldBeTrue();
	}

	[Fact]
	void Decimal_is_exact_and_canonical()
	{
		Cast.Decimal("0.1", NumFormat.Invariant).ShouldBe(new(new Success<decimal>(0.1m)));
		// Canonical: trailing fraction zeros are trimmed, so the scale is minimal.
		Cast.Decimal("1.10", NumFormat.Invariant).TryGetValue(out Success<decimal> scaled).ShouldBeTrue();
		scaled.Value.ShouldBe(1.1m);
		scaled.Value.Scale.ShouldBe((byte)1);
		Cast.Decimal("100", NumFormat.Invariant).TryGetValue(out Success<decimal> hundred).ShouldBeTrue();
		hundred.Value.Scale.ShouldBe((byte)0);
		Cast.Decimal("(2.5)%", NumFormat.Invariant).ShouldBe(new(new Success<decimal>(-0.025m)));
		Cast.Decimal("79228162514264337593543950335", NumFormat.Invariant).ShouldBe(new(new Success<decimal>(decimal.MaxValue)));
		(Cast.Decimal("79228162514264337593543950336", NumFormat.Invariant) is Fault { Reason: CastFailure.OutOfRange }).ShouldBeTrue();
		(Cast.Decimal("0.00000000000000000000000000001", NumFormat.Invariant) is Fault { Reason: CastFailure.OutOfRange }).ShouldBeTrue();
		// Zero is never negative — no -0 decimal escapes the door.
		Cast.Decimal("-0.00", NumFormat.Invariant).TryGetValue(out Success<decimal> zero).ShouldBeTrue();
		decimal.IsNegative(zero.Value).ShouldBeFalse();
		zero.Value.Scale.ShouldBe((byte)0);
		(Cast.Decimal("1.234,50", NumFormat.Detect) is Success<decimal> { Value: 1234.50m }).ShouldBeTrue();
	}

	[Fact]
	void Numeric_dispatches_every_target_through_one_generic_door()
	{
		(Cast.Numeric<sbyte>("-128", NumFormat.Invariant) is Success<sbyte> { Value: -128 }).ShouldBeTrue();
		(Cast.Numeric<short>("(1,234)", NumFormat.Invariant) is Success<short> { Value: -1234 }).ShouldBeTrue();
		(Cast.Numeric<int>("0x2A", NumFormat.Invariant) is Success<int> { Value: 42 }).ShouldBeTrue();
		(Cast.Numeric<long>("1e3", NumFormat.Invariant) is Success<long> { Value: 1000 }).ShouldBeTrue();
		(Cast.Numeric<byte>("255", NumFormat.Invariant) is Success<byte> { Value: 255 }).ShouldBeTrue();
		(Cast.Numeric<ushort>("65535", NumFormat.Invariant) is Success<ushort> { Value: 65535 }).ShouldBeTrue();
		(Cast.Numeric<uint>("4294967295", NumFormat.Invariant) is Success<uint> { Value: 4294967295 }).ShouldBeTrue();
		(Cast.Numeric<ulong>("18446744073709551615", NumFormat.Invariant) is Success<ulong> { Value: ulong.MaxValue }).ShouldBeTrue();
		(Cast.Numeric<float>("2.5", NumFormat.Invariant) is Success<float> { Value: 2.5f }).ShouldBeTrue();
		(Cast.Numeric<double>("25.5%", NumFormat.Invariant) is Success<double> { Value: 0.255 }).ShouldBeTrue();
		(Cast.Numeric<decimal>("1.10", NumFormat.Invariant) is Success<decimal> { Value: 1.10m }).ShouldBeTrue();
		// The fault travels through untouched.
		(Cast.Numeric<byte>("256", NumFormat.Invariant) is Fault { Reason: CastFailure.OutOfRange }).ShouldBeTrue();
		Should.Throw<NotSupportedException>(() => Cast.Numeric<Int128>("1", NumFormat.Invariant));
	}

	[Fact]
	void Utf16_doors_report_char_offsets_not_byte_offsets()
	{
		// "€" is three UTF-8 bytes but one char: the byte door says 3+1, the char door 1+1.
		var euroThenX = "€x"u8;
		(Cast.Int32(euroThenX, NumFormat.Invariant) is Fault { Offset: 0, Length: 3 }).ShouldBeTrue();
		(Cast.Int32("€x", NumFormat.Invariant) is Fault { Offset: 0, Length: 1 }).ShouldBeTrue();
		(Cast.Int32("1€", NumFormat.Invariant) is Fault { Offset: 1, Length: 1 }).ShouldBeTrue();
		(Cast.Int32("1€"u8, NumFormat.Invariant) is Fault { Offset: 1, Length: 3 }).ShouldBeTrue();
		(Cast.Int32("€€1x", NumFormat.Invariant) is Fault { Offset: 0, Length: 1 }).ShouldBeTrue();
		(Cast.Double("1 234,5x", new NumFormat(',', ' ', NumStyles.All)) is Fault { Offset: 7, Length: 1 }).ShouldBeTrue();
	}

	[Fact]
	void Native_library_is_probed_once_and_reports_its_version()
	{
		Cast.IsAvailable.ShouldBeTrue();
		Cast.NativeVersion.ShouldNotBeNull();
		// The binding and the core share one version — the probe exists to catch the mismatch.
		var binding = typeof(Cast).Assembly.GetName().Version!;
		Cast.NativeVersion.ShouldBe(new Version(binding.Major, binding.Minor, binding.Build));
	}

	[Fact]
	void Equal_separators_are_a_caller_bug_not_a_verdict() =>
		Should.Throw<ArgumentException>(() =>
			Cast.Int32("42", new NumFormat('.', '.', NumStyles.All)));

	[Fact]
	void Undefined_precision_is_a_caller_bug_not_a_verdict() =>
		Should.Throw<ArgumentOutOfRangeException>(() =>
			Cast.Unix("1700000000", UnixPrecision.Unspecified));
}
