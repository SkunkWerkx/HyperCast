namespace HyperCast;

/// <summary>
/// How the numeric doors judge digit groups — the grouping policy of a
/// <see cref="NumFormat"/>, which travels beside the <see cref="NumStyles"/> and is never one
/// of them. It matters only where <see cref="NumStyles.Grouping"/> is set: with grouping off, a
/// group separator is forbidden under every policy.
/// </summary>
/// <remarks>
/// The default is <see cref="Western"/>, so <c>1,23,456</c> and <c>12,34</c> are
/// <see cref="CastFailure.Malformed"/> where 1.0 read them; declare <see cref="Indian"/> for
/// <c>1,23,456</c>. The values are the core's, the policy field of its flags word. Any other value
/// is refused where it is declared: 256 is retired permanently and never reused, and a future
/// policy takes a new bit from bit 10 up. See docs/grouping.md for shapes no policy reads.
/// </remarks>
public enum NumGrouping
{
	/// <summary>
	/// Groups of exactly three digits counted leftward from the decimal separator (or the end of
	/// the numeral), the leftmost group one to three digits and not starting with a zero:
	/// <c>1,234,567</c> reads and <c>0,125</c> is <see cref="CastFailure.Malformed"/>. An ungrouped
	/// numeral stays valid, and once a numeral groups, every group must conform. The default.
	/// </summary>
	Western = 0,

	/// <summary>
	/// The Indian pattern: three digits next to the decimal separator, two in every group to the
	/// left of it, the leftmost one or two: <c>12,34,567</c>. Declared by the caller, or by a
	/// bridge whose number pattern says so (<see cref="NumFormat.From(System.Globalization.NumberFormatInfo)"/>
	/// reads it from <see cref="System.Globalization.NumberFormatInfo.NumberGroupSizes"/>).
	/// </summary>
	Indian = 512,
}
