namespace HyperCast;

/// <summary>
/// The failure case of <see cref="Verdict{T}"/>: a closed conversion reason plus the
/// offending span, pointing back into the caller's own input. Nothing is captured or
/// allocated — rendering a diagnostic (slicing the offending text out of the input) is the
/// caller's choice, paid only when actually rendering.
/// </summary>
/// <remarks>
/// <see cref="Offset"/> and <see cref="Length"/> index the input exactly as the caller
/// passed it: byte offsets from the UTF-8 (<c>ReadOnlySpan&lt;byte&gt;</c>) doors, UTF-16
/// code-unit offsets from the <c>ReadOnlySpan&lt;char&gt;</c>/<see cref="string"/> doors
/// — so slicing the offending text back out of the original input needs no mapping on
/// either side. The two agree whenever the input is ASCII, which scalar text almost always
/// is; the UTF-16 doors pay a prefix count only on a non-ASCII failure.
/// </remarks>
/// <param name="Reason">The closed-set conversion reason. Never <see cref="CastFailure.Unspecified"/>.</param>
/// <param name="Offset">Offset of the offending span, in the caller's own units.</param>
/// <param name="Length">Length of the offending span, in the caller's own units. Zero for <see cref="CastFailure.Empty"/>.</param>
public readonly record struct Fault(CastFailure Reason, int Offset, int Length);
