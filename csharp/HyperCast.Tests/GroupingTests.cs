using System.Globalization;

namespace HyperCast.Tests;

/// <summary>
/// The grouping policy of a <see cref="NumFormat"/>: strict Western by default, Indian and legacy as
/// explicit options that replace it, kept by <c>with { Styles = … }</c>, and judged by the doors.
/// </summary>
public sealed class GroupingTests
{
	[Fact]
	void A_format_is_strict_western_unless_it_declares_otherwise()
	{
		new NumFormat(".", ",", NumStyles.All).Grouping.ShouldBe(NumGrouping.Western);
		new NumFormat('.', ',', NumStyles.All).Grouping.ShouldBe(NumGrouping.Western);
		new NumFormat('.', ',', NumStyles.All, "$").Grouping.ShouldBe(NumGrouping.Western);
		NumFormat.Invariant.Grouping.ShouldBe(NumGrouping.Western);
		NumFormat.Detect.Grouping.ShouldBe(NumGrouping.Western);
		new NumFormat(".", ",", NumStyles.All, NumGrouping.Indian).Grouping.ShouldBe(NumGrouping.Indian);
		new NumFormat('.', ',', NumStyles.All, NumGrouping.Legacy).Grouping.ShouldBe(NumGrouping.Legacy);
		// The policy is set beside the styles: a style mask carries none, and `with` keeps the policy it had.
		var indian = new NumFormat(".", ",", NumStyles.All, NumGrouping.Indian);
		(indian with { Styles = NumStyles.Grouping }).Grouping.ShouldBe(NumGrouping.Indian);
		(indian with { Grouping = NumGrouping.Legacy }).Grouping.ShouldBe(NumGrouping.Legacy);
		(indian with { Grouping = NumGrouping.Legacy }).Styles.ShouldBe(NumStyles.All);
		// The formats differ in their policy, and so do their hashes' equality.
		indian.ShouldNotBe(new NumFormat(".", ",", NumStyles.All));
		(NumFormat.Invariant with { Grouping = NumGrouping.Western }).ShouldBe(NumFormat.Invariant);
	}

	[Fact]
	void The_default_judges_every_group_and_legacy_restores_1_0()
	{
		(Cast.Int32("12,34"u8, NumFormat.Invariant) is Fault { Reason: CastFailure.Malformed, Offset: 2, Length: 3, Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		(Cast.Int32("1,234,567"u8, NumFormat.Invariant) is Success<int> { Value: 1_234_567 }).ShouldBeTrue();
		(Cast.Int32("1234567"u8, NumFormat.Invariant) is Success<int> { Value: 1_234_567 }).ShouldBeTrue();
		(Cast.Int32("1234,567"u8, NumFormat.Invariant) is Fault { Offset: 0, Length: 4, Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		(Cast.Int32("1,2,3"u8, NumFormat.Invariant with { Grouping = NumGrouping.Legacy }) is Success<int> { Value: 123 }).ShouldBeTrue();
		(Cast.Int32("12,34"u8, NumFormat.Invariant with { Grouping = NumGrouping.Legacy }) is Success<int> { Value: 1234 }).ShouldBeTrue();
		// Indian: three digits by the point, twos beyond, the leftmost one or two.
		var indian = NumFormat.Invariant with { Grouping = NumGrouping.Indian };
		(Cast.Int32("12,34,567"u8, indian) is Success<int> { Value: 1_234_567 }).ShouldBeTrue();
		(Cast.Decimal("1,23,456.5"u8, indian) is Success<decimal> { Value: 123_456.5m }).ShouldBeTrue();
		(Cast.Int32("1,234,567"u8, indian) is Fault { Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		// Both input shapes, and every numeric door shares the rule.
		(Cast.Double("12,34", NumFormat.Invariant) is Fault { Offset: 2, Length: 3, Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		(Cast.Decimal("12,34", NumFormat.Invariant) is Fault { Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		(Cast.Numeric<long>("12,34", NumFormat.Invariant) is Fault { Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		(Cast.Scalar<ushort>("12,34"u8, NumFormat.Invariant) is Fault { Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		// Strict policies do not enable grouping, and grouping in the fraction is refused under each.
		(Cast.Int32("1,234"u8, NumFormat.Invariant with { Styles = NumStyles.None }) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		(Cast.Double("1.234,5"u8, new NumFormat(',', '.', NumStyles.All)) is Success<double> { Value: 1234.5 }).ShouldBeTrue();
		(Cast.Double("1.5,000"u8, NumFormat.Invariant) is Fault { Detail: FaultDetail.GroupInFraction }).ShouldBeTrue();
		// The checked doors judge it as the plain ones do.
		var checkedDoor = Cast.Int32("12,34"u8, NumFormat.Invariant, new Constraints<int>(maximum: 100_000));
		(checkedDoor.TryGetValue(out Rejection rejection) && rejection.TryGetValue(out Fault fault)
			&& fault is { Offset: 2, Length: 3, Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		(Cast.Int32("12,34"u8, NumFormat.Invariant with { Grouping = NumGrouping.Legacy }, new Constraints<int>(maximum: 100_000))
			.TryGetValue(out Checked<int> ok) && ok.Value == 1234).ShouldBeTrue();
	}

	[Fact]
	void The_provider_doors_judge_by_the_cultures_own_pattern()
	{
		var english = CultureInfo.GetCultureInfo("en-US");
		var india = CultureInfo.GetCultureInfo("en-IN");
		(Cast.Int32("12,34", english) is Fault { Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		(Cast.Int32("12,34,567", india) is Success<int> { Value: 1_234_567 }).ShouldBeTrue();
		(Cast.Int32("1,234,567", india) is Fault { Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		(Cast.Int32("1,234,567"u8, english) is Success<int> { Value: 1_234_567 }).ShouldBeTrue();
	}

	[Fact]
	void A_policy_or_style_bit_the_core_would_refuse_is_a_caller_bug_here()
	{
		static void Refused(NumFormat format, string message)
		{
			var thrown = Should.Throw<ArgumentException>(() => format.ToRaw());
			thrown.ParamName.ShouldBe("format");
			thrown.Message.ShouldContain(message);
			Should.Throw<ArgumentException>(() => Cast.Int32("1"u8, format)).Message.ShouldContain(message);
			Should.Throw<ArgumentException>(() => Cast.Int32("1", format, new Constraints<int>())).Message.ShouldContain(message);
		}
		Refused(NumFormat.Invariant with { Grouping = (NumGrouping)3 }, "not a defined grouping policy");
		Refused(NumFormat.Invariant with { Grouping = (NumGrouping)(-1) }, "not a defined grouping policy");
		// A policy riding in the style mask is no policy: it is an undefined style bit, bits 8 and up.
		foreach (var bits in new uint[] { 1u << 8, 2u << 8, 3u << 8, 1u << 10, 1u << 31, uint.MaxValue })
			Refused(NumFormat.Invariant with { Styles = (NumStyles)bits }, "undefined flag bits");
		// The policy is judged before a flag bit, as the core judges one word.
		Refused(NumFormat.Invariant with { Grouping = (NumGrouping)3, Styles = (NumStyles)(1u << 12) }, "grouping policy");
		// Every defined style is still fine, alone and together.
		foreach (var style in Enum.GetValues<NumStyles>())
			Should.NotThrow(() => (NumFormat.Invariant with { Styles = style }).ToRaw(), style.ToString());
		Should.NotThrow(() => (NumFormat.Invariant with { Styles = NumStyles.All | NumStyles.TrailingSign | NumStyles.SeparatorDetect }).ToRaw());
	}
}
