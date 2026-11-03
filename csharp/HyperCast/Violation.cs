namespace HyperCast;

/// <summary>
/// The other failure case of <see cref="Rejection"/>: the value parsed and broke a declared
/// rule. The keyword says which, and the span covers the value (after trimming, for text) in
/// the same units as a <see cref="Fault"/>'s: bytes from the UTF-8 doors, UTF-16 code units
/// from the <c>ReadOnlySpan&lt;char&gt;</c>/<see cref="string"/> doors, and an empty span
/// from a door that read a <see cref="double"/>.
/// </summary>
/// <param name="Keyword">The rule broken.</param>
/// <param name="Offset">Offset of the value, in the caller's own units.</param>
/// <param name="Length">Length of the value, in the caller's own units.</param>
public readonly record struct Violation(Keyword Keyword, int Offset, int Length);
