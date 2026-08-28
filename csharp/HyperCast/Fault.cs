namespace HyperCast;

/// <summary>
/// The failure case of <see cref="Verdict{T}"/>: a closed conversion reason plus the
/// offending span, pointing back into the caller's own input. Nothing is captured or
/// allocated — rendering a diagnostic (slicing the offending text out of the input) is the
/// caller's choice, paid only when actually rendering.
/// </summary>
/// <remarks>
/// <see cref="Offset"/> and <see cref="Length"/> are byte offsets into the UTF-8
/// representation of the input — exactly what the UTF-8 (<c>ReadOnlySpan&lt;byte&gt;</c>)
/// doors received, and identical to character offsets whenever the input is ASCII (which
/// scalar text almost always is). For non-ASCII input passed through a UTF-16 door, map
/// accordingly before slicing.
/// </remarks>
/// <param name="Reason">The closed-set conversion reason. Never <see cref="CastFailure.Unspecified"/>.</param>
/// <param name="Offset">Byte offset of the offending span in the input's UTF-8 form.</param>
/// <param name="Length">Byte length of the offending span. Zero for <see cref="CastFailure.Empty"/>.</param>
public readonly record struct Fault(CastFailure Reason, int Offset, int Length);
