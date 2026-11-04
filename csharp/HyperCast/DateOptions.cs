using System.Runtime.CompilerServices;
using HyperCast.Interop;

namespace HyperCast;

/// <summary>
/// The date and time shapes a door accepts only when asked — <see cref="DateOptions.Shapes"/>,
/// bit for bit the native core's option flags. Every one is opt-in: a door without options keeps
/// today's strict shape rules, so a caller reading strict RFC 3339 may rely on a space separator
/// or <c>+00</c> being refused.
/// </summary>
[Flags]
public enum DateShapes : uint
{
	/// <summary>No option: the door's shape rules as they are.</summary>
	None = 0,

	/// <summary><see cref="Cast.Date(ReadOnlySpan{byte}, DateOptions)"/> also reads <c>20260107</c>, eight ASCII digits as <c>yyyyMMdd</c>.</summary>
	CompactDate = 1,

	/// <summary><see cref="Cast.Timestamp(ReadOnlySpan{byte}, DateOptions)"/> also reads a single space in place of <c>T</c>.</summary>
	SpaceSeparator = 2,

	/// <summary><see cref="Cast.Timestamp(ReadOnlySpan{byte}, DateOptions)"/> also reads <c>±hh</c> offsets (<c>+00</c>, <c>-05</c>).</summary>
	HourOffset = 4,

	/// <summary>The three together: <see cref="CompactDate"/>, <see cref="SpaceSeparator"/> and <see cref="HourOffset"/>. Never a pivot or a vocabulary.</summary>
	CommonForms = CompactDate | SpaceSeparator | HourOffset,
}

/// <summary>
/// What a date door reads beyond its strict shapes, declared by the caller: the
/// <see cref="Shapes"/> to accept, a <see cref="Pivot"/> year for two-digit years and a
/// <see cref="Months"/> vocabulary for month names. Everything is opt-in, a known option a door
/// has no use for is ignored there, and <c>default</c> declares nothing.
/// </summary>
/// <remarks>
/// <para>
/// <b>The pivot</b> is the last year of a 100-year window: a two-digit year <c>yy</c> reads as the
/// one year in <c>[pivot − 99, pivot]</c> that ends in <c>yy</c>. It is 0 (absent: a two-digit year
/// is <see cref="FaultDetail.FieldWidth"/>) or 100 to 9999, and four-digit years are unaffected. It is read by the
/// ordered date and the civil date-time doors only.
/// </para>
/// <para>
/// <b>The vocabulary</b> is independent of the rest: <c>new DateOptions { Months = … }</c> opens month
/// names and nothing else. It too is read by the ordered date and the civil date-time doors only.
/// </para>
/// A value the core would refuse (an undefined shape bit, a pivot outside its range) is a caller bug
/// the door reports with an <see cref="ArgumentException"/>.
/// </remarks>
/// <param name="Shapes">The shapes to accept.</param>
/// <param name="Pivot">The pivot year, or 0 for none.</param>
public readonly record struct DateOptions(DateShapes Shapes = DateShapes.None, int Pivot = 0)
{
	/// <summary>The most common forms together (<see cref="DateShapes.CommonForms"/>), with no pivot and no vocabulary.</summary>
	public static DateOptions CommonForms => new(DateShapes.CommonForms);

	/// <summary>The month names read in the month field, or <see langword="null"/> for none.</summary>
	public MonthNames? Months { get; init; }

	const uint DefinedShapes = (uint)DateShapes.CommonForms;

	/// <summary>
	/// Validates and converts to the native core's layout, or reports <see langword="false"/> when
	/// nothing is declared (the door then passes no options at all) — what every date door does with
	/// its options argument, public for a library that declares date columns across a C ABI of its own.
	/// The vocabulary crosses separately, as <see cref="MonthNames.Packed"/>.
	/// </summary>
	/// <param name="raw">The 12-byte layout when something is declared.</param>
	/// <exception cref="ArgumentException"><see cref="Shapes"/> carries a bit no shape defines.</exception>
	/// <exception cref="ArgumentOutOfRangeException"><see cref="Pivot"/> is neither 0 nor 100 to 9999.</exception>
	public bool TryToRaw(out RawDateOptions raw)
	{
		var shapes = (uint)Shapes;
		if ((shapes & ~DefinedShapes) != 0)
			throw new ArgumentException($"Date shapes 0x{shapes:X} carry undefined flag bits.", "options");
		if (Pivot != 0 && Pivot is < 100 or > 9999)
			throw new ArgumentOutOfRangeException("options", Pivot, "Pivot must be 0 (none) or 100 to 9999.");
		raw = new RawDateOptions { Size = (uint)Unsafe.SizeOf<RawDateOptions>(), Flags = shapes, Pivot = (uint)Pivot };
		return shapes != 0 || Pivot != 0;
	}
}
