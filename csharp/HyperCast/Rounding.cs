namespace HyperCast;

/// <summary>
/// How a value that is not a multiple of <c>multipleOf</c> is treated, judged on the exact
/// decimal the door read. Any mode but <see cref="Strict"/> needs a <c>multipleOf</c>; a
/// rounded success is marked <see cref="Checked{T}.Adjusted"/>. Values match the native
/// core's codes.
/// </summary>
public enum Rounding : uint
{
	/// <summary>A value that is not an exact multiple is a violation of <c>multipleOf</c>. The default.</summary>
	Strict = 0,

	/// <summary>Rounds to the nearest multiple, ties to the even multiple (banker's rounding, IEEE 754's default).</summary>
	Midpoint = 1,

	/// <summary>Rounds to the multiple towards negative infinity.</summary>
	Floor = 2,

	/// <summary>Rounds to the multiple towards positive infinity.</summary>
	Ceiling = 3
}
