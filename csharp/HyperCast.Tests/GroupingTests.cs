using System.Globalization;
using System.Reflection;

namespace HyperCast.Tests;

/// <summary>
/// The grouping policy of a <see cref="NumFormat"/>: Western by default, Indian as an
/// explicit option that replaces it, both chosen on a format, kept by <c>with { Styles = … }</c>, and judged by the doors.
/// </summary>
public sealed class GroupingTests
{
	[Fact]
	void A_format_is_western_unless_it_declares_otherwise()
	{
		new NumFormat(".", ",", NumStyles.All).Grouping.ShouldBe(NumGrouping.Western);
		new NumFormat('.', ',', NumStyles.All).Grouping.ShouldBe(NumGrouping.Western);
		new NumFormat('.', ',', NumStyles.All, "$").Grouping.ShouldBe(NumGrouping.Western);
		NumFormat.Invariant.Grouping.ShouldBe(NumGrouping.Western);
		NumFormat.Detect.Grouping.ShouldBe(NumGrouping.Western);
		new NumFormat(".", ",", NumStyles.All, NumGrouping.Indian).Grouping.ShouldBe(NumGrouping.Indian);
		new NumFormat('.', ',', NumStyles.All, NumGrouping.Western).Grouping.ShouldBe(NumGrouping.Western);
		default(NumFormat).Grouping.ShouldBe(NumGrouping.Western);
		// The policy is set beside the styles: a style mask carries none, and `with` keeps the policy it had.
		var indian = new NumFormat(".", ",", NumStyles.All, NumGrouping.Indian);
		(indian with { Styles = NumStyles.Grouping }).Grouping.ShouldBe(NumGrouping.Indian);
		(indian with { Grouping = NumGrouping.Western }).Grouping.ShouldBe(NumGrouping.Western);
		(indian with { Grouping = NumGrouping.Western }).Styles.ShouldBe(NumStyles.All);
		(NumFormat.Invariant with { Grouping = NumGrouping.Indian }).ShouldBe(indian);
		(indian with { Grouping = NumGrouping.Western }).ShouldBe(NumFormat.Invariant);
		// The formats differ in their policy, and so do their hashes' equality.
		indian.ShouldNotBe(new NumFormat(".", ",", NumStyles.All));
		(NumFormat.Invariant with { Grouping = NumGrouping.Western }).ShouldBe(NumFormat.Invariant);
	}

	[Fact]
	void The_default_judges_every_group_and_indian_is_declared()
	{
		(Cast.Int32("12,34"u8, NumFormat.Invariant) is Fault { Reason: CastFailure.Malformed, Offset: 2, Length: 3, Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		(Cast.Int32("1,234,567"u8, NumFormat.Invariant) is Success<int> { Value: 1_234_567 }).ShouldBeTrue();
		(Cast.Int32("1234567"u8, NumFormat.Invariant) is Success<int> { Value: 1_234_567 }).ShouldBeTrue();
		(Cast.Int32("1234,567"u8, NumFormat.Invariant) is Fault { Offset: 0, Length: 4, Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		(Cast.Int32("1,2,345"u8, NumFormat.Invariant) is Fault { Offset: 1, Length: 2, Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		(Cast.Int32("1,23,456"u8, NumFormat.Invariant) is Fault { Offset: 1, Length: 3, Detail: FaultDetail.GroupSize }).ShouldBeTrue();
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
		// A policy does not enable grouping, and grouping in the fraction is refused under each.
		(Cast.Int32("1,234"u8, NumFormat.Invariant with { Styles = NumStyles.None }) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		(Cast.Double("1.234,5"u8, new NumFormat(',', '.', NumStyles.All)) is Success<double> { Value: 1234.5 }).ShouldBeTrue();
		(Cast.Double("1.5,000"u8, NumFormat.Invariant) is Fault { Detail: FaultDetail.GroupInFraction }).ShouldBeTrue();
		// The checked doors judge it as the plain ones do.
		var checkedDoor = Cast.Int32("12,34"u8, NumFormat.Invariant, new Constraints<int>(maximum: 100_000));
		(checkedDoor.TryGetValue(out Rejection<int> rejection) && rejection.TryGetValue(out Fault fault)
			&& fault is { Offset: 2, Length: 3, Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		(Cast.Int32("1,23,456"u8, NumFormat.Invariant with { Grouping = NumGrouping.Indian }, new Constraints<int>(maximum: 200_000))
			.TryGetValue(out Checked<int> ok) && ok.Value == 123456).ShouldBeTrue();
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

	// A field's value put where neither declaration could see it, as a reinterpreted struct would.
	static NumFormat Forced(string property, object value)
	{
		object boxed = NumFormat.Invariant;
		typeof(NumFormat).GetField($"<{property}>k__BackingField", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(boxed, value);
		return (NumFormat)boxed;
	}

	[Fact]
	void A_policy_or_style_bit_no_declaration_names_is_refused_where_it_is_declared()
	{
		foreach (var bad in new[] { 256, 768, 1, 2, 3, 255, 513, 1024, -1 })
		{
			var grouping = (NumGrouping)bad;
			var message = "not a defined grouping policy";
			Should.Throw<ArgumentException>(() => NumFormat.Invariant with { Grouping = grouping }).Message.ShouldContain(message);
			Should.Throw<ArgumentException>(() => new NumFormat(".", ",", NumStyles.All, grouping)).Message.ShouldContain(message);
			Should.Throw<ArgumentException>(() => new NumFormat('.', ',', NumStyles.All, grouping)).Message.ShouldContain(message);
			Should.Throw<ArgumentException>(() => new NumFormat(".", ",", NumStyles.All) { Grouping = grouping }).Message.ShouldContain(message);
			// What got past both is the ABI validator's.
			var forced = Forced(nameof(NumFormat.Grouping), grouping);
			var thrown = Should.Throw<ArgumentException>(() => forced.ToRaw());
			thrown.ParamName.ShouldBe("format");
			thrown.Message.ShouldContain(message);
			Should.Throw<ArgumentException>(() => Cast.Int32("1"u8, forced)).Message.ShouldContain(message);
			Should.Throw<ArgumentException>(() => Cast.Int32("1", forced, new Constraints<int>())).Message.ShouldContain(message);
		}
		// A policy riding in the style mask is no policy: it is an undefined style bit, bits 8 and up.
		foreach (var bits in new uint[] { 351, 863, 1u << 8, 2u << 8, 3u << 8, 1u << 10, 1u << 31, uint.MaxValue })
		{
			var styles = (NumStyles)bits;
			var message = "undefined flag bits";
			Should.Throw<ArgumentException>(() => NumFormat.Invariant with { Styles = styles }).Message.ShouldContain(message);
			Should.Throw<ArgumentException>(() => new NumFormat(".", ",", styles)).Message.ShouldContain(message);
			Should.Throw<ArgumentException>(() => new NumFormat('.', ',', styles)).Message.ShouldContain(message);
			Should.Throw<ArgumentException>(() => new NumFormat(".", ",", styles, "$")).Message.ShouldContain(message);
			Should.Throw<ArgumentException>(() => new NumFormat(".", ",", styles, NumGrouping.Indian)).Message.ShouldContain(message);
			var forced = Forced(nameof(NumFormat.Styles), styles);
			Should.Throw<ArgumentException>(() => forced.ToRaw()).ParamName.ShouldBe("format");
		}
		// Every defined style is still fine, alone and together, and both policies.
		foreach (var style in Enum.GetValues<NumStyles>())
			Should.NotThrow(() => (NumFormat.Invariant with { Styles = style }).ToRaw(), style.ToString());
		Should.NotThrow(() => (NumFormat.Invariant with { Styles = NumStyles.All | NumStyles.TrailingSign | NumStyles.SeparatorDetect }).ToRaw());
		foreach (var grouping in Enum.GetValues<NumGrouping>())
			Should.NotThrow(() => (NumFormat.Invariant with { Grouping = grouping }).ToRaw());
		Enum.GetValues<NumGrouping>().ShouldBe([NumGrouping.Western, NumGrouping.Indian]);
	}
}
