using System.Globalization;

namespace HyperCast;

/// <summary>
/// Caller-declared numeric notation for the integer and real doors. The native core carries
/// no culture data — this is the bridge from .NET's culture machinery to the core's three
/// declared fields. A call site parsing culture-sensitive text declares its format out loud
/// (<see cref="Invariant"/>, or <see cref="From(CultureInfo)"/>) — there is no defaulting
/// overload, the same stance Svartalfheim took with <c>IFormatProvider</c>.
/// </summary>
/// <remarks>
/// Currency symbols — the one notation that truly needs culture tables — are deliberately
/// not supported. Separators are single UTF-16 code units, which covers every separator any
/// real culture declares (<c>.</c> <c>,</c> U+00A0, U+202F, …).
/// </remarks>
/// <param name="DecimalSeparator">The declared decimal separator.</param>
/// <param name="GroupSeparator">The declared digit-group separator. Must differ from <paramref name="DecimalSeparator"/>.</param>
/// <param name="Styles">The lenience flags.</param>
public readonly record struct NumFormat(char DecimalSeparator, char GroupSeparator, NumStyles Styles)
{
	/// <summary>The invariant profile — <c>.</c> decimal, <c>,</c> grouping, every lenience on.</summary>
	public static readonly NumFormat Invariant = new('.', ',', NumStyles.All);

	/// <summary>
	/// The detection profile — every lenience on, <c>.</c>/<c>,</c> roles resolved per
	/// input by <see cref="NumStyles.SeparatorDetect"/>'s structural rules.
	/// </summary>
	public static readonly NumFormat Detect = new('.', ',', NumStyles.All | NumStyles.SeparatorDetect);

	/// <summary>
	/// Derives a format from a culture's number formatting: the first code unit of its
	/// decimal and group separators, with every lenience on. For cultures whose group
	/// separator is a multi-unit string, only the first unit is used — declare explicitly if
	/// that matters.
	/// </summary>
	/// <param name="culture">The culture to derive from. Never null.</param>
	/// <exception cref="ArgumentNullException"><paramref name="culture"/> is null.</exception>
	public static NumFormat From(CultureInfo culture)
	{
		ArgumentNullException.ThrowIfNull(culture);
		var numberFormat = culture.NumberFormat;
		var decimalSeparator = numberFormat.NumberDecimalSeparator[0];
		var groupSeparator = numberFormat.NumberGroupSeparator is { Length: > 0 } group
			? group[0]
			: ',';
		return new(decimalSeparator, groupSeparator, NumStyles.All);
	}

	/// <summary>Validates and converts to the native core's layout.</summary>
	/// <exception cref="ArgumentException">
	/// The separators are equal, or a separator is a UTF-16 surrogate half (not a whole code
	/// point) — either way a caller bug, not a data verdict.
	/// </exception>
	internal Cast.RawNumFormat ToRaw()
	{
		if (DecimalSeparator == GroupSeparator)
			throw new ArgumentException(
				$"Decimal and group separators must differ; both are '{DecimalSeparator}'.");
		if (char.IsSurrogate(DecimalSeparator) || char.IsSurrogate(GroupSeparator))
			throw new ArgumentException("Separators must be whole code points, not surrogate halves.");
		return new(DecimalSeparator, GroupSeparator, (uint)Styles);
	}
}
