using System.Globalization;
using System.Runtime.CompilerServices;
using System.Text;

namespace HyperCast;

/// <summary>
/// Caller-declared shape of a civil date-time for
/// <see cref="Cast.DateTime(ReadOnlySpan{byte}, DateFormat)"/>: the field
/// <see cref="Order"/> of the date, and the day-period designators a 12-hour clock is
/// written with — the words a culture writes for the two halves of the day (<c>오전</c>/<c>오후</c>,
/// <c>vorm.</c>/<c>nachm.</c>, <c>a. m.</c>/<c>p. m.</c>). The temporal counterpart of
/// <see cref="NumFormat"/>: declared, never looked up, with <see cref="From(CultureInfo)"/>
/// filling it from a culture's own data, and the English <c>AM</c>/<c>PM</c> always read
/// beside whatever is declared — after the clock (<c>3:04 nachm.</c>) or ahead of it
/// (<c>오후 3:04</c>). A bare <see cref="DateOrder"/> still declares the order alone, as it
/// always has.
/// </summary>
/// <param name="Order">The declared field order of the date. Never <see cref="DateOrder.Unspecified"/>.</param>
public readonly record struct DateFormat(DateOrder Order)
{
	/// <summary>The most UTF-8 bytes a designator may occupy — the core's inline buffer.</summary>
	public const int MaxDesignatorBytes = NumFormat.MaxCurrencyBytes;

	// What the civil date-time door calls the parameter this value arrives in.
	const string DoorParameter = "format";

	/// <summary>Declares the order and both designators together.</summary>
	/// <param name="order">The declared field order of the date. Never <see cref="DateOrder.Unspecified"/>.</param>
	/// <param name="amDesignator">The designator of the first half of the day; empty declares none beside <c>AM</c>.</param>
	/// <param name="pmDesignator">The designator of the second half; empty declares none beside <c>PM</c>.</param>
	public DateFormat(DateOrder order, string amDesignator, string pmDesignator)
		: this(order)
	{
		AMDesignator = amDesignator;
		PMDesignator = pmDesignator;
	}

	/// <summary>
	/// The designator of the first half of the day, matched exactly where the English
	/// <c>AM</c> is matched case-insensitively. Empty — the default — declares none beside
	/// <c>AM</c>. One to <see cref="MaxDesignatorBytes"/> bytes of UTF-8, with no ASCII
	/// digit and no ASCII whitespace at either end (the grammar around it reads that; an
	/// inner space, as in Spanish <c>a. m.</c>, is part of the word); anything else is a
	/// caller bug the door rejects with an <see cref="ArgumentException"/>.
	/// </summary>
	/// <remarks>
	/// Never <see langword="null"/>, whatever built the value: a <see langword="null"/> handed
	/// to the constructor or the initializer, and the unset field of a
	/// <c>default(DateFormat)</c>, both read back as empty — nothing declared — as
	/// <see cref="NumFormat"/>'s string fields do.
	/// </remarks>
	public string AMDesignator { get => field ?? ""; init; } = "";

	/// <summary>The designator of the second half of the day, as <see cref="AMDesignator"/>, beside <c>PM</c>.</summary>
	public string PMDesignator { get => field ?? ""; init; } = "";

	/// <summary>
	/// <see langword="true"/> when a designator is declared beside the English pair — what
	/// the door hands the core; with neither declared it passes nothing, and the cast is
	/// exactly the bare <see cref="DateOrder"/> one.
	/// </summary>
	public bool HasDesignators => AMDesignator.Length != 0 || PMDesignator.Length != 0;

	/// <summary>
	/// Derives the format from a culture: the order from its short-date pattern, as
	/// <see cref="DateOrders.From(CultureInfo)"/>, and the designators from
	/// <see cref="DateTimeFormatInfo.AMDesignator"/> and
	/// <see cref="DateTimeFormatInfo.PMDesignator"/>, as <see cref="From(DateTimeFormatInfo)"/>.
	/// </summary>
	/// <param name="culture">The culture to derive from. Never null.</param>
	/// <exception cref="ArgumentNullException"><paramref name="culture"/> is null.</exception>
	/// <exception cref="ArgumentException">The culture's short-date pattern names none of y/M/d — a malformed culture, not a data verdict.</exception>
	public static DateFormat From(CultureInfo culture)
	{
		ArgumentNullException.ThrowIfNull(culture);
		return From(culture.DateTimeFormat);
	}

	/// <summary>
	/// Derives the format from whatever date-time formatting <paramref name="provider"/>
	/// resolves to, as <see cref="DateTimeFormatInfo.GetInstance"/> resolves it: a
	/// <see cref="DateTimeFormatInfo"/> directly, a <see cref="CultureInfo"/>'s, or the
	/// current culture's for any other provider — the shape a BCL <c>DateTime.TryParse</c>
	/// takes.
	/// </summary>
	/// <param name="provider">The format provider to derive from. Never null.</param>
	/// <exception cref="ArgumentNullException"><paramref name="provider"/> is null.</exception>
	/// <exception cref="ArgumentException">The resolved short-date pattern names none of y/M/d — a malformed culture, not a data verdict.</exception>
	public static DateFormat From(IFormatProvider provider)
	{
		ArgumentNullException.ThrowIfNull(provider);
		return From(DateTimeFormatInfo.GetInstance(provider));
	}

	/// <summary>
	/// Derives the format from date-time formatting data directly: the order from
	/// <see cref="DateTimeFormatInfo.ShortDatePattern"/>, as
	/// <see cref="DateOrders.From(DateTimeFormatInfo)"/>, and the designators from
	/// <see cref="DateTimeFormatInfo.AMDesignator"/> and
	/// <see cref="DateTimeFormatInfo.PMDesignator"/>.
	/// </summary>
	/// <remarks>
	/// A designator that is the English one (<c>AM</c>, <c>am</c>) is left undeclared, since
	/// the core reads it regardless; so is one the layout cannot carry — past
	/// <see cref="MaxDesignatorBytes"/> bytes of UTF-8, which the Thai, Devanagari and a few
	/// other scripts' words are — and that culture reads the English pair only. Anything a
	/// culture's data holds beyond those two cases fits, and every culture .NET ships maps
	/// without a throw.
	/// </remarks>
	/// <param name="dateTimeFormat">The date-time formatting to derive from. Never null.</param>
	/// <exception cref="ArgumentNullException"><paramref name="dateTimeFormat"/> is null.</exception>
	/// <exception cref="ArgumentException">The short-date pattern names none of y/M/d — a malformed culture, not a data verdict.</exception>
	public static DateFormat From(DateTimeFormatInfo dateTimeFormat)
	{
		ArgumentNullException.ThrowIfNull(dateTimeFormat);
		return new(DateOrders.From(dateTimeFormat), Declared(dateTimeFormat.AMDesignator, "AM"), Declared(dateTimeFormat.PMDesignator, "PM"));
	}

	// A culture's designator where it is not the English one the core reads regardless, and
	// not wider than the layout carries.
	static string Declared(string designator, string english) =>
		designator.Equals(english, StringComparison.OrdinalIgnoreCase) || Encoding.UTF8.GetByteCount(designator) > MaxDesignatorBytes
			? ""
			: designator;

	/// <summary>
	/// Validates and converts the designators to the native core's 40-byte layout — what the
	/// civil date-time door does with its format argument when <see cref="HasDesignators"/>,
	/// public for a library that declares civil date-time columns across a C ABI of its own.
	/// The <see cref="Order"/> crosses separately, as <see cref="Interop.Abi.Code(DateOrder, string)"/>.
	/// </summary>
	/// <exception cref="ArgumentException">
	/// A contract violation — a caller bug, not a data verdict: a designator past
	/// <see cref="MaxDesignatorBytes"/> bytes, carrying an ASCII digit, not whole code points
	/// (a lone UTF-16 surrogate), or starting or ending in ASCII whitespace.
	/// </exception>
	public Interop.RawDayPeriods ToRaw()
	{
		var raw = new Interop.RawDayPeriods { Size = (uint)Unsafe.SizeOf<Interop.RawDayPeriods>() };
		raw.Am.Length = Encode(AMDesignator, raw.Am.Bytes, "AM designator");
		raw.Pm.Length = Encode(PMDesignator, raw.Pm.Bytes, "PM designator");
		return raw;
	}

	static byte Encode(string designator, Span<byte> bytes, string what)
	{
		if (designator.Length != 0 && (IsAsciiWhitespace(designator[0]) || IsAsciiWhitespace(designator[^1])))
			throw new ArgumentException($"{what} '{designator}' must not start or end with whitespace.", DoorParameter);
		return NumFormat.Encode(designator, bytes, what, flush: false);
	}

	static bool IsAsciiWhitespace(char c) => c is ' ' or '\t' or '\n' or '\f' or '\r';
}
