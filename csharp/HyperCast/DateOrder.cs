using System.Globalization;

namespace HyperCast;

/// <summary>
/// The caller-declared field order of a separated calendar date. There is no guessing —
/// <c>1/7/2026</c> is January 7th or July 1st only because the caller said which (en-US
/// short dates are month-first, en-GB and most of the world day-first, ISO year-first),
/// the same declare-don't-sniff stance <see cref="NumFormat"/> takes for numeric notation
/// and <see cref="UnixPrecision"/> takes for epoch magnitude. Values match the native
/// core's discriminants.
/// </summary>
public enum DateOrder : uint
{
	/// <summary>Sentinel CLR default — never a valid order; rejected by the Date door.</summary>
	Unspecified = 0,

	/// <summary>Year, month, day — ISO's order with any accepted separator.</summary>
	YearMonthDay = 1,

	/// <summary>Month, day, year — the en-US short-date order (<c>1/7/2026</c> is January 7th).</summary>
	MonthDayYear = 2,

	/// <summary>Day, month, year — the en-GB/most-of-the-world order (<c>1/7/2026</c> is July 1st).</summary>
	DayMonthYear = 3
}

/// <summary>
/// Bridges .NET's culture machinery to a declared <see cref="DateOrder"/> — the date-order
/// counterpart of <see cref="NumFormat.From(CultureInfo)"/>.
/// </summary>
public static class DateOrders
{
	/// <summary>
	/// Derives the field order from a culture's own short-date pattern: the first of
	/// <c>y</c>/<c>M</c>/<c>d</c> to appear decides (quoted literals skipped). Prefer
	/// declaring explicitly when the text's origin is known — a culture is process/user
	/// state; the text's dialect is a property of the text.
	/// </summary>
	/// <param name="culture">The culture whose short-date pattern declares the order.</param>
	/// <returns>The culture's field order.</returns>
	/// <exception cref="ArgumentException">The pattern names none of y/M/d — a malformed culture, not a data verdict.</exception>
	public static DateOrder From(CultureInfo culture)
	{
		var inLiteral = false;
		foreach (var ch in culture.DateTimeFormat.ShortDatePattern)
		{
			if (ch == '\'')
			{
				inLiteral = !inLiteral;
				continue;
			}
			if (inLiteral)
				continue;
			switch (ch)
			{
				case 'y': return DateOrder.YearMonthDay;
				case 'M': return DateOrder.MonthDayYear;
				case 'd': return DateOrder.DayMonthYear;
			}
		}
		throw new ArgumentException(
			$"Short date pattern '{culture.DateTimeFormat.ShortDatePattern}' names none of y/M/d.",
			nameof(culture));
	}
}
