namespace HyperCast;

/// <summary>
/// The other failure case of <see cref="Rejection{T}"/>: the input cast to the door's type and
/// the value broke a declared rule. The keyword says which; the value is the one the door
/// cast, exactly as cast (never rounded: <c>multipleOf</c>'s rounding is part of the
/// judgement, not of the value; at the text door, the text after <c>trim</c>, the input's own
/// text rather than an <c>enum</c> entry); and the span covers it in the same units as a
/// <see cref="Fault"/>'s: bytes from the UTF-8 doors, UTF-16 code units from the
/// <c>ReadOnlySpan&lt;char&gt;</c>/<see cref="string"/> doors, and an empty span from a door
/// that read a <see cref="double"/>.
/// </summary>
/// <typeparam name="T">The door's type: a number, or <see cref="string"/> for the text door.</typeparam>
/// <param name="Keyword">The rule broken.</param>
/// <param name="Value">The value as the door cast it, in the door's own type, before any rounding.</param>
/// <param name="Offset">Offset of the value, in the caller's own units.</param>
/// <param name="Length">Length of the value, in the caller's own units.</param>
public readonly record struct Violation<T>(Keyword Keyword, T Value, int Offset, int Length) where T : notnull;
