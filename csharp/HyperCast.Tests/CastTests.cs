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
	void Equal_separators_are_a_caller_bug_not_a_verdict() =>
		Should.Throw<ArgumentException>(() =>
			Cast.Int32("42", new NumFormat('.', '.', NumStyles.All)));

	[Fact]
	void Undefined_precision_is_a_caller_bug_not_a_verdict() =>
		Should.Throw<ArgumentOutOfRangeException>(() =>
			Cast.Unix("1700000000", UnixPrecision.Unspecified));
}
