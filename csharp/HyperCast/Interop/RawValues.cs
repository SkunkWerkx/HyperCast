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
/// The native core's numeric-format layout — the Rust crate's <c>RawNumFormat</c>, 92
/// bytes with 4-byte alignment: a size word, the <see cref="NumStyles"/> bits and the grouping
/// policy (<see cref="NumGrouping.Indian"/> in bit 9 of the same word; Western sets no bit), the native
/// digits' zero, then every declared symbol in place as its UTF-8 bytes followed by its
/// length, with nothing between one field and the next. The core reads it where it stands,
/// so a cast pays for a check of it and no copy. Built by <see cref="NumFormat.ToRaw"/>,
/// which validates on the way.
/// </summary>
/// <remarks>
/// <list type="table">
/// <listheader><term>Offset</term><description>Field</description></listheader>
/// <item><term>0</term><description><see cref="Size"/></description></item>
/// <item><term>4</term><description><see cref="Flags"/></description></item>
/// <item><term>8</term><description><see cref="DigitZero"/></description></item>
/// <item><term>12</term><description><see cref="DecimalSep"/>, its length at 20</description></item>
/// <item><term>21</term><description><see cref="GroupSep"/>, length at 29</description></item>
/// <item><term>30</term><description><see cref="CurrencyDecimalSep"/>, length at 38</description></item>
/// <item><term>39</term><description><see cref="CurrencyGroupSep"/>, length at 47</description></item>
/// <item><term>48</term><description><see cref="MinusSign"/>, length at 56</description></item>
/// <item><term>57</term><description><see cref="PlusSign"/>, length at 65</description></item>
/// <item><term>66</term><description><see cref="Percent"/>, length at 74</description></item>
/// <item><term>75</term><description><see cref="Currency"/>, length at 91</description></item>
/// </list>
/// The size word is first so the layout can grow without another coordinated change: a
/// caller states how many bytes it built and the core reads exactly those, defaulting the
/// rest. The 0.8.0 layout was 32 bytes with the separators as code points at its head; it
/// has no size word and is not read.
/// </remarks>
[StructLayout(LayoutKind.Sequential)]
public struct RawNumFormat
{
	/// <summary>The layout's size as the caller built it: 92.</summary>
	public uint Size;

	/// <summary>The <see cref="NumStyles"/> bits in bits 0 to 7, and the grouping policy: 0 Western, 512 Indian. Bit 8 is retired and the core's detailed exports refuse it.</summary>
	public uint Flags;

	/// <summary>The native digits' zero code point; 0 declares none.</summary>
	public uint DigitZero;

	/// <summary>The decimal separator. Always declared.</summary>
	public RawSymbol DecimalSep;

	/// <summary>The digit-group separator. Always declared, and never the decimal one.</summary>
	public RawSymbol GroupSep;

	/// <summary>The decimal separator of a currency amount; length 0 keeps <see cref="DecimalSep"/>.</summary>
	public RawSymbol CurrencyDecimalSep;

	/// <summary>The group separator of a currency amount; length 0 keeps <see cref="GroupSep"/>.</summary>
	public RawSymbol CurrencyGroupSep;

	/// <summary>A further minus sign; length 0 declares none beside ASCII <c>-</c>.</summary>
	public RawSymbol MinusSign;

	/// <summary>A further plus sign; length 0 declares none beside ASCII <c>+</c>.</summary>
	public RawSymbol PlusSign;

	/// <summary>A further percent symbol; length 0 declares none beside ASCII <c>%</c>.</summary>
	public RawSymbol Percent;

	/// <summary>The currency symbol; length 0 declares none.</summary>
	public RawWideSymbol Currency;
}

/// <summary>
/// A declared symbol as it crosses the ABI — the Rust crate's <c>Symbol&lt;8&gt;</c>: up to
/// <see cref="NumFormat.MaxSymbolBytes"/> bytes of UTF-8, zero-padded, then its length. 9
/// bytes with no alignment. Bytes past <see cref="Length"/> are not the symbol.
/// </summary>
[StructLayout(LayoutKind.Sequential)]
public struct RawSymbol
{
	/// <summary>The symbol's UTF-8 bytes, zero-padded.</summary>
	public RawSymbolBytes Bytes;

	/// <summary>How many of <see cref="Bytes"/> are the symbol; 0 declares none.</summary>
	public byte Length;
}

/// <summary>The inline buffer of a <see cref="RawSymbol"/> — <see cref="NumFormat.MaxSymbolBytes"/> bytes.</summary>
[InlineArray(NumFormat.MaxSymbolBytes)]
public struct RawSymbolBytes
{
	byte _element0;
}

/// <summary>
/// A declared symbol with the wider buffer — the Rust crate's <c>CurrencySymbol</c> and
/// <c>Symbol&lt;16&gt;</c>: up to <see cref="NumFormat.MaxCurrencyBytes"/> bytes of UTF-8,
/// zero-padded, then its length. 17 bytes with no alignment. The currency symbol of a
/// <see cref="RawNumFormat"/> and each designator of a <see cref="RawDayPeriods"/>.
/// </summary>
[StructLayout(LayoutKind.Sequential)]
public struct RawWideSymbol
{
	/// <summary>The symbol's UTF-8 bytes, zero-padded.</summary>
	public RawWideSymbolBytes Bytes;

	/// <summary>How many of <see cref="Bytes"/> are the symbol; 0 declares none.</summary>
	public byte Length;
}

/// <summary>The inline buffer of a <see cref="RawWideSymbol"/> — <see cref="NumFormat.MaxCurrencyBytes"/> bytes.</summary>
[InlineArray(NumFormat.MaxCurrencyBytes)]
public struct RawWideSymbolBytes
{
	byte _element0;
}

/// <summary>
/// The native core's day-period layout — the Rust crate's <c>RawDayPeriods</c>, 40 bytes
/// with 4-byte alignment: a size word, then the two designators in place, the AM one's
/// bytes and length (4 to 20) and the PM one's (21 to 37), and two bytes of padding. A
/// length of 0 declares nothing beside the English pair. Built by
/// <see cref="DateFormat.ToRaw"/>, which validates on the way; where the civil date-time
/// door passes a pointer to one, null means the English <c>AM</c>/<c>PM</c> only.
/// </summary>
[StructLayout(LayoutKind.Sequential)]
public struct RawDayPeriods
{
	/// <summary>The layout's size as the caller built it: 40.</summary>
	public uint Size;

	/// <summary>The designator of the first half of the day; length 0 declares none beside <c>AM</c>.</summary>
	public RawWideSymbol Am;

	/// <summary>The designator of the second half; length 0 declares none beside <c>PM</c>.</summary>
	public RawWideSymbol Pm;
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
/// A fault's span and detail as the detailed exports write them — the Rust crate's
/// <c>RawFaultDetail</c>, 16 bytes with 4-byte alignment: the size word the caller states,
/// then the span and the detail code. The core accepts a <see cref="Size"/> of 16 or more
/// that is a multiple of 4, writes bytes 4 to 16 only, and never writes the size word.
/// </summary>
/// <remarks>
/// <see cref="Detail"/> is the published vocabulary's code (1 to 32), or 0 on a checked
/// door's violation. A code this binding does not know decodes as
/// <see cref="FaultDetail.Unknown"/> with the number kept, and the reason and span stay usable.
/// </remarks>
[StructLayout(LayoutKind.Sequential)]
public struct RawFaultDetail
{
	/// <summary>The layout's size in bytes: 16.</summary>
	public const uint SizeInBytes = 16;

	/// <summary>The size the caller provides: <see cref="SizeInBytes"/> or more, a multiple of 4. The core never writes it.</summary>
	public uint Size;

	/// <summary>The byte offset of the offending span.</summary>
	public uint Offset;

	/// <summary>The byte length of the offending span.</summary>
	public uint Length;

	/// <summary>The detail code: 1 to 32 as published, 0 for none.</summary>
	public uint Detail;

	/// <summary>An empty layout with its <see cref="Size"/> stated, as every detailed call needs.</summary>
	public static RawFaultDetail Empty => new() { Size = SizeInBytes };
}

/// <summary>
/// The date options the detailed exports read — the Rust crate's <c>RawDateOptions</c>, 12
/// bytes with 4-byte alignment: a size word, the option flags and the pivot year. The core
/// accepts a size of 8 (the pivot then reads as absent) or 12, never more. Built by
/// <see cref="DateOptions"/>, which validates on the way.
/// </summary>
[StructLayout(LayoutKind.Sequential)]
public struct RawDateOptions
{
	/// <summary>The layout's size as the caller built it: 12.</summary>
	public uint Size;

	/// <summary>The <see cref="DateShapes"/> bits.</summary>
	public uint Flags;

	/// <summary>The pivot year, the last year of the 100-year window; 0 for none, otherwise 100 to 9999.</summary>
	public uint Pivot;
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

/// <summary>
/// The native core's validation-constraints layout — the Rust crate's <c>RawConstraints</c>,
/// 72 bytes with 8-byte alignment: a size word, the flags saying which fields are read, the
/// rounding code, the two lengths, a reserved word, then the three decimals. Pointer-free,
/// so it is the same on 32-bit and 64-bit targets; an <c>enum</c> list travels beside it.
/// Built by <see cref="Constraints{T}.ToRaw"/>, from a declaration the core has already
/// judged.
/// </summary>
/// <remarks>
/// <list type="table">
/// <listheader><term>Offset</term><description>Field</description></listheader>
/// <item><term>0</term><description><see cref="Size"/></description></item>
/// <item><term>4</term><description><see cref="Flags"/></description></item>
/// <item><term>8</term><description><see cref="Rounding"/></description></item>
/// <item><term>12</term><description><see cref="MinLength"/></description></item>
/// <item><term>16</term><description><see cref="MaxLength"/></description></item>
/// <item><term>20</term><description><see cref="Reserved"/></description></item>
/// <item><term>24</term><description><see cref="Lower"/></description></item>
/// <item><term>40</term><description><see cref="Upper"/></description></item>
/// <item><term>56</term><description><see cref="MultipleOf"/></description></item>
/// </list>
/// A <see cref="Size"/> other than 72, an unknown flag bit, an unknown rounding code, a
/// nonzero <see cref="Reserved"/>, a decimal sign byte other than 0 or 1, or a non-empty
/// <c>enum</c> list without <see cref="HasEnum"/> is a malformed layout.
/// </remarks>
[StructLayout(LayoutKind.Sequential)]
public struct RawConstraints
{
	/// <summary><see cref="Lower"/> is the inclusive <c>minimum</c>.</summary>
	public const uint HasMinimum = 1;

	/// <summary><see cref="Lower"/> is the <c>exclusiveMinimum</c>.</summary>
	public const uint HasExclusiveMinimum = 2;

	/// <summary><see cref="Upper"/> is the inclusive <c>maximum</c>.</summary>
	public const uint HasMaximum = 4;

	/// <summary><see cref="Upper"/> is the <c>exclusiveMaximum</c>.</summary>
	public const uint HasExclusiveMaximum = 8;

	/// <summary><see cref="MultipleOf"/> and <see cref="Rounding"/> are read.</summary>
	public const uint HasMultipleOf = 16;

	/// <summary><see cref="MinLength"/> is read.</summary>
	public const uint HasMinLength = 32;

	/// <summary><see cref="MaxLength"/> is read.</summary>
	public const uint HasMaxLength = 64;

	/// <summary>Text is trimmed of Unicode <c>White_Space</c> before it is judged.</summary>
	public const uint Trim = 128;

	/// <summary>An <c>enum</c> is declared: the list beside the layout is read, and an empty one is refused.</summary>
	public const uint HasEnum = 256;

	/// <summary>The layout's size as the caller built it: 72.</summary>
	public uint Size;

	/// <summary>Which constraints are declared: the <c>Has*</c> bits, <see cref="HasEnum"/> among them, and <see cref="Trim"/>.</summary>
	public uint Flags;

	/// <summary>The <see cref="HyperCast.Rounding"/> code: 0 strict, 1 midpoint, 2 floor, 3 ceiling.</summary>
	public uint Rounding;

	/// <summary>The fewest code points the text may hold.</summary>
	public uint MinLength;

	/// <summary>The most code points the text may hold.</summary>
	public uint MaxLength;

	/// <summary>Always 0.</summary>
	public uint Reserved;

	/// <summary>The minimum, or the exclusive minimum.</summary>
	public RawDecimal Lower;

	/// <summary>The maximum, or the exclusive maximum.</summary>
	public RawDecimal Upper;

	/// <summary>The step every value must be a multiple of.</summary>
	public RawDecimal MultipleOf;
}

/// <summary>
/// What a checked door reports beside its value, as the core writes it — the Rust crate's
/// <c>RawCheck</c>, 12 bytes with 4-byte alignment.
/// </summary>
/// <param name="keyword">On a violation, the <see cref="HyperCast.Keyword"/> code broken; 0 otherwise.</param>
/// <param name="adjusted">On success, 1 when rounding changed the value, else 0.</param>
/// <param name="enumIndex">On a success at the text door that matched an <c>enum</c> entry, its index; <see cref="uint.MaxValue"/> otherwise.</param>
[StructLayout(LayoutKind.Sequential)]
public readonly struct RawCheck(uint keyword, uint adjusted, uint enumIndex)
{
	/// <summary>On a violation, the <see cref="HyperCast.Keyword"/> code broken; 0 otherwise.</summary>
	public readonly uint Keyword = keyword;

	/// <summary>On success, 1 when rounding changed the value, else 0.</summary>
	public readonly uint Adjusted = adjusted;

	/// <summary>On a success at the text door that matched an <c>enum</c> entry, its index; <see cref="uint.MaxValue"/> otherwise.</summary>
	public readonly uint EnumIndex = enumIndex;
}

/// <summary>
/// A span of the caller's own input as the core writes it — the Rust crate's
/// <c>RawSpan</c>, 8 bytes with 4-byte alignment: what the text door writes in place of a
/// copy.
/// </summary>
/// <param name="offset">The byte offset where the span starts.</param>
/// <param name="length">The span's length in bytes.</param>
[StructLayout(LayoutKind.Sequential)]
public readonly struct RawSpan(uint offset, uint length)
{
	/// <summary>The byte offset where the span starts.</summary>
	public readonly uint Offset = offset;

	/// <summary>The span's length in bytes.</summary>
	public readonly uint Length = length;
}
