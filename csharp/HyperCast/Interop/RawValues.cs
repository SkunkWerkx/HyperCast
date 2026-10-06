using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace HyperCast.Interop;

// The native core's C ABI value layouts, public for a library that carries HyperCast's
// verdicts across a C ABI of its own (HyperTabular does): it reads these out of its own
// buffers and turns them into .NET values through the same conversions every Cast door
// uses, instead of restating them. Each mirrors a #[repr(C)] type of the Rust crate; the
// layouts are pinned by tests, and Sequential layout reproduces the C padding because the
// widest field fixes the alignment.

/// <summary>
/// The native core's 32-byte numeric-format layout — the Rust crate's <c>RawNumFormat</c>:
/// four <c>u32</c>s, then the currency symbol's UTF-8 bytes inline. Built by
/// <see cref="NumFormat.ToRaw"/>, which validates on the way.
/// </summary>
[StructLayout(LayoutKind.Sequential)]
public struct RawNumFormat
{
	/// <summary>The decimal separator's code point.</summary>
	public uint DecimalSep;

	/// <summary>The digit-group separator's code point.</summary>
	public uint GroupSep;

	/// <summary>The <see cref="NumStyles"/> bits.</summary>
	public uint Flags;

	/// <summary>How many of <see cref="Currency"/>'s bytes are the symbol; 0 declares none.</summary>
	public uint CurrencyLen;

	/// <summary>The currency symbol's UTF-8 bytes, zero-padded.</summary>
	public RawCurrency Currency;
}

/// <summary>The inline currency buffer of <see cref="RawNumFormat"/> — <see cref="NumFormat.MaxCurrencyBytes"/> bytes.</summary>
[InlineArray(NumFormat.MaxCurrencyBytes)]
public struct RawCurrency
{
	byte _element0;
}

/// <summary>A fault's span as the core writes it: byte offset and length into the input.</summary>
/// <param name="offset">The byte offset.</param>
/// <param name="length">The byte length.</param>
[StructLayout(LayoutKind.Sequential)]
public readonly struct RawFault(uint offset, uint length)
{
	/// <summary>The byte offset of the offending span.</summary>
	public readonly uint Offset = offset;

	/// <summary>The byte length of the offending span.</summary>
	public readonly uint Length = length;
}

/// <summary>
/// A decimal as the core writes it: the 96-bit magnitude as <c>lo</c>/<c>hi</c>, the scale,
/// and the sign — the same triple <see cref="decimal"/> is made of. 16 bytes, 2 of them
/// tail padding.
/// </summary>
/// <param name="lo">The magnitude's low 64 bits.</param>
/// <param name="hi">The magnitude's high 32 bits.</param>
/// <param name="scale">The power of ten the magnitude is divided by, 0–28.</param>
/// <param name="negative">Nonzero for a negative value.</param>
[StructLayout(LayoutKind.Sequential)]
public readonly struct RawDecimal(ulong lo, uint hi, byte scale, byte negative)
{
	/// <summary>The magnitude's low 64 bits.</summary>
	public readonly ulong Lo = lo;

	/// <summary>The magnitude's high 32 bits.</summary>
	public readonly uint Hi = hi;

	/// <summary>The power of ten the magnitude is divided by, 0–28.</summary>
	public readonly byte Scale = scale;

	/// <summary>Nonzero for a negative value.</summary>
	public readonly byte Negative = negative;

	/// <summary>The value, exactly: the decimal door's presentation.</summary>
	public decimal ToDecimal() => new((int)Lo, (int)(Lo >> 32), (int)Hi, Negative != 0, Scale);
}

/// <summary>
/// An instant as the core writes it — protobuf's <c>Timestamp</c>: whole seconds from the
/// Unix epoch and the nanoseconds after them (never negative). 16 bytes.
/// </summary>
/// <param name="seconds">Seconds from 1970-01-01T00:00:00Z.</param>
/// <param name="nanos">Nanoseconds after them, 0–999,999,999.</param>
[StructLayout(LayoutKind.Sequential)]
public readonly struct RawTimestamp(long seconds, int nanos)
{
	const long UnixEpochTicks = 621_355_968_000_000_000L;

	/// <summary>Seconds from 1970-01-01T00:00:00Z.</summary>
	public readonly long Seconds = seconds;

	/// <summary>Nanoseconds after them, 0–999,999,999.</summary>
	public readonly int Nanos = nanos;

	/// <summary>The instant at UTC, sub-tick nanoseconds truncated: the timestamp doors' presentation.</summary>
	public DateTimeOffset ToDateTimeOffset() =>
		new(UnixEpochTicks + Seconds * TimeSpan.TicksPerSecond + Nanos / 100, TimeSpan.Zero);
}

/// <summary>A calendar date as the core writes it. 4 bytes.</summary>
/// <param name="year">The year, 1–9999.</param>
/// <param name="month">The month, 1–12.</param>
/// <param name="day">The day of the month.</param>
[StructLayout(LayoutKind.Sequential)]
public readonly struct RawDate(ushort year, byte month, byte day)
{
	/// <summary>The year, 1–9999.</summary>
	public readonly ushort Year = year;

	/// <summary>The month, 1–12.</summary>
	public readonly byte Month = month;

	/// <summary>The day of the month.</summary>
	public readonly byte Day = day;

	/// <summary>The date: the date doors' presentation.</summary>
	public DateOnly ToDateOnly() => new(Year, Month, Day);
}

/// <summary>
/// A zone-less wall clock as the core writes it: a date and the nanoseconds since its
/// midnight. 16 bytes, 4 of them padding after the date.
/// </summary>
/// <param name="year">The year, 1–9999.</param>
/// <param name="month">The month, 1–12.</param>
/// <param name="day">The day of the month.</param>
/// <param name="nanosOfDay">Nanoseconds since midnight.</param>
[StructLayout(LayoutKind.Sequential)]
public readonly struct RawCivil(ushort year, byte month, byte day, ulong nanosOfDay)
{
	/// <summary>The year, 1–9999.</summary>
	public readonly ushort Year = year;

	/// <summary>The month, 1–12.</summary>
	public readonly byte Month = month;

	/// <summary>The day of the month.</summary>
	public readonly byte Day = day;

	/// <summary>Nanoseconds since midnight.</summary>
	public readonly ulong NanosOfDay = nanosOfDay;

	/// <summary>
	/// The wall clock as a <see cref="DateTimeKind.Unspecified"/> <see cref="System.DateTime"/>,
	/// sub-tick nanoseconds truncated: the date-time doors' presentation.
	/// </summary>
	public System.DateTime ToDateTime() =>
		new System.DateTime(Year, Month, Day, 0, 0, 0, DateTimeKind.Unspecified).AddTicks((long)(NanosOfDay / 100));
}

/// <summary>
/// A span as the core writes it — protobuf's <c>Duration</c>: whole seconds and the
/// nanoseconds after them, the two same-signed. 16 bytes.
/// </summary>
/// <param name="seconds">Whole seconds.</param>
/// <param name="nanos">Nanoseconds, signed as <paramref name="seconds"/>.</param>
[StructLayout(LayoutKind.Sequential)]
public readonly struct RawDuration(long seconds, int nanos)
{
	/// <summary>Whole seconds.</summary>
	public readonly long Seconds = seconds;

	/// <summary>Nanoseconds, signed as <see cref="Seconds"/>.</summary>
	public readonly int Nanos = nanos;

	/// <summary>The span, sub-tick nanoseconds truncated toward zero: the duration doors' presentation.</summary>
	public TimeSpan ToTimeSpan() => new(Seconds * TimeSpan.TicksPerSecond + Nanos / 100);
}
