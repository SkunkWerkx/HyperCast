namespace HyperCast;

/// <summary>
/// How strictly the numeric doors judge digit groups — the grouping policy of a
/// <see cref="NumFormat"/>, which travels beside the <see cref="NumStyles"/> and is never one
/// of them. It matters only where <see cref="NumStyles.Grouping"/> is set: with grouping off, a
/// group separator is forbidden under every policy.
/// </summary>
/// <remarks>
/// The default is <see cref="Western"/>, so <c>12,34</c> is <see cref="CastFailure.Malformed"/> where
/// 1.0 read 1234. <see cref="Legacy"/> is the escape hatch that restores 1.0's rule.
/// </remarks>
public enum NumGrouping
{
	/// <summary>
	/// Groups of exactly three digits counted leftward from the decimal separator (or the end of
	/// the numeral), the leftmost group one to three digits: <c>1,234,567</c>. An ungrouped
	/// numeral stays valid, and once a numeral groups, every group must conform. The default.
	/// </summary>
	Western = 0,

	/// <summary>
	/// The Indian pattern: three digits next to the decimal separator, two in every group to the
	/// left of it, the leftmost one or two: <c>12,34,567</c>. Declared only where the platform's
	/// own number pattern says so (<see cref="NumFormat.From(System.Globalization.NumberFormatInfo)"/>
	/// reads it from <see cref="System.Globalization.NumberFormatInfo.NumberGroupSizes"/>).
	/// </summary>
	Indian = 1,

	/// <summary>1.0's rule: a group separator anywhere between two digits, whatever the group sizes.</summary>
	Legacy = 2,
}
