namespace HyperCast;

/// <summary>
/// The lenience flags of <see cref="NumFormat"/> — bit-for-bit the native core's flags.
/// </summary>
[Flags]
public enum NumStyles : uint
{
	/// <summary>No lenience: sign, digits, and (for reals) the declared decimal separator only.</summary>
	None = 0,

	/// <summary>
	/// Permit the declared group separator between digits. Group sizes are not validated —
	/// a separator must simply sit between two digits (HyperCast's own deterministic rule
	/// where .NET's <c>AllowThousands</c> deferred to per-culture quirks).
	/// </summary>
	Grouping = 1,

	/// <summary>Permit accounting parentheses as negation: <c>(1,234)</c> is -1234.</summary>
	Parentheses = 2,

	/// <summary>
	/// Permit exponent notation: <c>1e3</c>, <c>2.5E-3</c>. Integer doors reject a negative
	/// exponent — a decimal point is never accepted on an integer, in any disguise.
	/// </summary>
	Exponent = 4,

	/// <summary>
	/// Permit the culture-insensitive radix prefixes <c>0x</c>/<c>&amp;H</c> (hex) and
	/// <c>0b</c> (binary), read as the two's-complement bit pattern (<c>0xFF</c> is -1 for an
	/// <see cref="sbyte"/>). Integer doors only.
	/// </summary>
	RadixPrefixes = 8,

	/// <summary>Permit a trailing <c>%</c>, dividing by 100 (<c>50%</c> is 0.5). Real doors only.</summary>
	Percent = 16,

	/// <summary>Every lenience on.</summary>
	All = Grouping | Parentheses | Exponent | RadixPrefixes | Percent
}
