using System.Globalization;
using System.Text;

namespace HyperCast;

/// <summary>
/// Caller-declared numeric notation for the integer, real and decimal doors. The native
/// core carries no culture data — this is the bridge from .NET's culture machinery to the
/// core's declared fields. A call site parsing culture-sensitive text declares its format
/// out loud (<see cref="Invariant"/>, <see cref="From(CultureInfo)"/>,
/// <see cref="From(IFormatProvider)"/>, or the constructor with the separators spelled
/// directly) — there is no defaulting overload, the same stance Svartalfheim took with
/// <c>IFormatProvider</c>.
/// </summary>
/// <remarks>
/// Separators are single UTF-16 code units, which covers every separator any real culture
/// declares (<c>.</c> <c>,</c> U+00A0, U+202F, …). The currency symbol is the one field
/// that genuinely needs a culture table to fill in, and it is declared rather than looked
/// up: <see cref="From(CultureInfo)"/> copies the culture's own, or state it yourself.
/// Currency symbols are not localized further — <c>$</c> is matched as <c>$</c>.
/// </remarks>
/// <param name="DecimalSeparator">The declared decimal separator.</param>
/// <param name="GroupSeparator">The declared digit-group separator. Must differ from <paramref name="DecimalSeparator"/>.</param>
/// <param name="Styles">The lenience flags.</param>
public readonly record struct NumFormat(char DecimalSeparator, char GroupSeparator, NumStyles Styles)
{
	/// <summary>The most UTF-8 bytes a <see cref="CurrencySymbol"/> may occupy — the core's inline buffer.</summary>
	public const int MaxCurrencyBytes = 16;

	/// <summary>The invariant profile — <c>.</c> decimal, <c>,</c> grouping, every lenience on, no currency symbol.</summary>
	public static readonly NumFormat Invariant = new('.', ',', NumStyles.All);

	/// <summary>
	/// The detection profile — every lenience on, <c>.</c>/<c>,</c> roles resolved per
	/// input by <see cref="NumStyles.SeparatorDetect"/>'s structural rules.
	/// </summary>
	public static readonly NumFormat Detect = new('.', ',', NumStyles.All | NumStyles.SeparatorDetect);

	/// <summary>Declares the separators, flags and currency symbol together.</summary>
	/// <param name="decimalSeparator">The declared decimal separator.</param>
	/// <param name="groupSeparator">The declared digit-group separator. Must differ from <paramref name="decimalSeparator"/>.</param>
	/// <param name="styles">The lenience flags.</param>
	/// <param name="currencySymbol">The declared currency symbol, honored under <see cref="NumStyles.Currency"/>; empty declares none.</param>
	public NumFormat(char decimalSeparator, char groupSeparator, NumStyles styles, string currencySymbol)
		: this(decimalSeparator, groupSeparator, styles) =>
		CurrencySymbol = currencySymbol;

	/// <summary>
	/// The declared currency symbol, honored only while <see cref="NumStyles.Currency"/> is
	/// set (it is part of <see cref="NumStyles.All"/>). Empty — the default — declares none.
	/// At most <see cref="MaxCurrencyBytes"/> bytes of UTF-8, containing no ASCII digit or
	/// whitespace; anything else is a caller bug the doors reject with an
	/// <see cref="ArgumentException"/>.
	/// </summary>
	public string CurrencySymbol { get; init; } = "";

	/// <summary>
	/// Derives a format from a culture's number formatting: the first code unit of its
	/// decimal and group separators, its currency symbol, and every lenience on. For
	/// cultures whose group separator is a multi-unit string, only the first unit is used —
	/// declare explicitly if that matters.
	/// </summary>
	/// <param name="culture">The culture to derive from. Never null.</param>
	/// <exception cref="ArgumentNullException"><paramref name="culture"/> is null.</exception>
	public static NumFormat From(CultureInfo culture)
	{
		ArgumentNullException.ThrowIfNull(culture);
		return From(culture.NumberFormat);
	}

	/// <summary>
	/// Derives a format from whatever number formatting <paramref name="provider"/> resolves
	/// to — the shape every BCL <c>TryParse</c> already takes, so a caller that is itself
	/// <see cref="IFormatProvider"/>-shaped needs no <see cref="CultureInfo"/> cast first.
	/// Resolution is <see cref="NumberFormatInfo.GetInstance"/>'s: a
	/// <see cref="NumberFormatInfo"/> directly, a <see cref="CultureInfo"/>'s number format,
	/// or the current culture's for any other provider.
	/// </summary>
	/// <param name="provider">The format provider to derive from. Never null.</param>
	/// <exception cref="ArgumentNullException"><paramref name="provider"/> is null.</exception>
	public static NumFormat From(IFormatProvider provider)
	{
		ArgumentNullException.ThrowIfNull(provider);
		return From(NumberFormatInfo.GetInstance(provider));
	}

	/// <summary>Derives a format from number-format data directly: separators, currency symbol, every lenience on.</summary>
	/// <param name="numberFormat">The number formatting to derive from. Never null.</param>
	/// <exception cref="ArgumentNullException"><paramref name="numberFormat"/> is null.</exception>
	public static NumFormat From(NumberFormatInfo numberFormat)
	{
		ArgumentNullException.ThrowIfNull(numberFormat);
		var decimalSeparator = numberFormat.NumberDecimalSeparator[0];
		var groupSeparator = numberFormat.NumberGroupSeparator is { Length: > 0 } group
			? group[0]
			: ',';
		return new(decimalSeparator, groupSeparator, NumStyles.All, numberFormat.CurrencySymbol);
	}

	/// <summary>Validates and converts to the native core's layout.</summary>
	/// <exception cref="ArgumentException">
	/// The separators are equal, a separator is a UTF-16 surrogate half (not a whole code
	/// point), or the currency symbol is too long or carries a digit or whitespace — either
	/// way a caller bug, not a data verdict.
	/// </exception>
	internal Cast.RawNumFormat ToRaw()
	{
		if (DecimalSeparator == GroupSeparator)
			throw new ArgumentException(
				$"Decimal and group separators must differ; both are '{DecimalSeparator}'.");
		if (char.IsSurrogate(DecimalSeparator) || char.IsSurrogate(GroupSeparator))
			throw new ArgumentException("Separators must be whole code points, not surrogate halves.");
		var raw = new Cast.RawNumFormat
		{
			DecimalSep = DecimalSeparator,
			GroupSep = GroupSeparator,
			Flags = (uint)Styles,
		};
		if (CurrencySymbol.Length == 0)
			return raw;
		foreach (var c in CurrencySymbol)
			if (char.IsAsciiDigit(c) || char.IsWhiteSpace(c))
				throw new ArgumentException($"Currency symbol '{CurrencySymbol}' must not contain a digit or whitespace.");
		if (!Encoding.UTF8.TryGetBytes(CurrencySymbol, raw.Currency, out var written))
			throw new ArgumentException($"Currency symbol '{CurrencySymbol}' exceeds {MaxCurrencyBytes} UTF-8 bytes.");
		raw.CurrencyLen = (uint)written;
		return raw;
	}
}
