namespace HyperCast;

/// <summary>The success case of <see cref="CheckedVerdict{T}"/>: a value that obeys every declared rule.</summary>
/// <typeparam name="T">The door's type: a number, or <see cref="string"/> for the text door.</typeparam>
/// <param name="Value">
/// The value, rounded to its <c>multipleOf</c> when a rounding said so. At the text door, the
/// text (trimmed when declared); with an <c>enum</c>, the caller's own declared entry, the
/// same string instance, whatever case the input arrived in.
/// </param>
/// <param name="Adjusted"><see langword="true"/> only when rounding changed the value. Trimming and case never set it.</param>
/// <param name="EnumIndex">
/// At the text door with an <c>enum</c>, the zero-based index of the entry matched;
/// <see langword="null"/> otherwise, a numeric <c>enum</c> included (the value is the answer).
/// </param>
public readonly record struct Checked<T>(T Value, bool Adjusted, int? EnumIndex) where T : notnull;
