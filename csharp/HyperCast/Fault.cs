using System.Text;

namespace HyperCast;

/// <summary>
/// The failure case of <see cref="Verdict{T}"/>: a closed conversion reason plus the
/// offending span, pointing back into the caller's own input, and what failed
/// (<see cref="Detail"/>). Nothing is captured or allocated — rendering a diagnostic
/// (slicing the offending text out of the input) is the caller's choice, paid only when
/// actually rendering.
/// </summary>
/// <remarks>
/// <para>
/// <see cref="Offset"/> and <see cref="Length"/> index the input exactly as the caller
/// passed it: byte offsets from the UTF-8 (<c>ReadOnlySpan&lt;byte&gt;</c>) doors, UTF-16
/// code-unit offsets from the <c>ReadOnlySpan&lt;char&gt;</c>/<see cref="string"/> doors
/// — so slicing the offending text back out of the original input needs no mapping on
/// either side. The two agree whenever the input is ASCII, which scalar text almost always
/// is; the UTF-16 doors pay a prefix count only on a non-ASCII failure.
/// </para>
/// <para>
/// <b>The detail is beside the identity, not part of it.</b> <see cref="DetailCode"/> is a
/// non-positional property: the constructor, the deconstruction and the pattern shapes are
/// 1.0's, and two faults with the same reason and span are equal, and hash alike, whatever
/// their details say (as is the text <see cref="object.ToString"/> renders).
/// </para>
/// </remarks>
/// <param name="Reason">The closed-set conversion reason. Never <see cref="CastFailure.Unspecified"/>.</param>
/// <param name="Offset">Offset of the offending span, in the caller's own units.</param>
/// <param name="Length">Length of the offending span, in the caller's own units. Zero for <see cref="CastFailure.Empty"/>.</param>
public readonly record struct Fault(CastFailure Reason, int Offset, int Length)
{
	/// <summary>
	/// The core's raw detail code: 1 to 32 as published, 0 when none was reported. A code
	/// this binding does not name is kept here, and <see cref="Detail"/> reads
	/// <see cref="FaultDetail.Unknown"/> for it. Not part of equality.
	/// </summary>
	public uint DetailCode { get; init; }

	/// <summary>What failed, named: <see cref="FaultDetail.None"/>, one of the published members, or <see cref="FaultDetail.Unknown"/>.</summary>
	public FaultDetail Detail => Interop.Abi.DetailFrom(DetailCode);

	/// <summary>Whether <paramref name="other"/> has the same reason, offset and length; the detail is not compared.</summary>
	/// <param name="other">The fault to compare with.</param>
	public bool Equals(Fault other) => Reason == other.Reason && Offset == other.Offset && Length == other.Length;

	/// <summary>A hash of the reason, offset and length; the detail is not hashed.</summary>
	public override int GetHashCode() => HashCode.Combine(Reason, Offset, Length);

	// The members 1.0's generated printing named, so the text does not change.
	bool PrintMembers(StringBuilder builder)
	{
		builder.Append("Reason = ").Append(Reason.ToString())
			.Append(", Offset = ").Append(Offset.ToString())
			.Append(", Length = ").Append(Length.ToString());
		return true;
	}
}
