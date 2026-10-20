using System.Globalization;
using System.Numerics;
using System.Runtime.CompilerServices;

namespace HyperCast;

// The numeric doors, the two generic ones and the civil date-time door again, taking the
// platform's own format object instead of a NumFormat or a DateFormat: the IFormatProvider
// every BCL TryParse and ISpanParsable<T> takes, so a caller that is itself
// IFormatProvider-shaped (a parsing gateway, a serializer converter) passes what it was handed
// straight through. Each maps to NumFormat.From or DateFormat.From and calls the door it
// overloads; the mapping reads the culture's own symbols and allocates nothing.
//
// Each carries OverloadResolutionPriority(-1) so that `default` as the second argument still
// binds the NumFormat door, as it did before these existed (default(NumFormat) is the
// no-separators caller bug it always was). null, a CultureInfo or a NumberFormatInfo
// cannot convert to NumFormat, so for them these are the only candidates and the priority
// never comes into it.
public static partial class Cast
{
	// NumberFormatInfo.GetInstance is the BCL's own resolution, null included: a
	// NumberFormatInfo as is, a CultureInfo's number format, and the current culture's for
	// null or any other provider. DateTimeFormatInfo.GetInstance resolves the same way.
	static NumFormat Format(IFormatProvider? provider) => NumFormat.From(NumberFormatInfo.GetInstance(provider));
	static DateFormat DateFormatOf(IFormatProvider? provider) => DateFormat.From(DateTimeFormatInfo.GetInstance(provider));

	/// <summary>
	/// <see cref="SByte(ReadOnlySpan{byte}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<sbyte> SByte(ReadOnlySpan<byte> utf8, IFormatProvider? provider) => SByte(utf8, Format(provider));

	/// <summary>
	/// <see cref="SByte(ReadOnlySpan{char}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<sbyte> SByte(ReadOnlySpan<char> input, IFormatProvider? provider) => SByte(input, Format(provider));

	/// <summary>
	/// <see cref="Int16(ReadOnlySpan{byte}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<short> Int16(ReadOnlySpan<byte> utf8, IFormatProvider? provider) => Int16(utf8, Format(provider));

	/// <summary>
	/// <see cref="Int16(ReadOnlySpan{char}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<short> Int16(ReadOnlySpan<char> input, IFormatProvider? provider) => Int16(input, Format(provider));

	/// <summary>
	/// <see cref="Int32(ReadOnlySpan{byte}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<int> Int32(ReadOnlySpan<byte> utf8, IFormatProvider? provider) => Int32(utf8, Format(provider));

	/// <summary>
	/// <see cref="Int32(ReadOnlySpan{char}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<int> Int32(ReadOnlySpan<char> input, IFormatProvider? provider) => Int32(input, Format(provider));

	/// <summary>
	/// <see cref="Int64(ReadOnlySpan{byte}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<long> Int64(ReadOnlySpan<byte> utf8, IFormatProvider? provider) => Int64(utf8, Format(provider));

	/// <summary>
	/// <see cref="Int64(ReadOnlySpan{char}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<long> Int64(ReadOnlySpan<char> input, IFormatProvider? provider) => Int64(input, Format(provider));

	/// <summary>
	/// <see cref="Byte(ReadOnlySpan{byte}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<byte> Byte(ReadOnlySpan<byte> utf8, IFormatProvider? provider) => Byte(utf8, Format(provider));

	/// <summary>
	/// <see cref="Byte(ReadOnlySpan{char}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<byte> Byte(ReadOnlySpan<char> input, IFormatProvider? provider) => Byte(input, Format(provider));

	/// <summary>
	/// <see cref="UInt16(ReadOnlySpan{byte}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<ushort> UInt16(ReadOnlySpan<byte> utf8, IFormatProvider? provider) => UInt16(utf8, Format(provider));

	/// <summary>
	/// <see cref="UInt16(ReadOnlySpan{char}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<ushort> UInt16(ReadOnlySpan<char> input, IFormatProvider? provider) => UInt16(input, Format(provider));

	/// <summary>
	/// <see cref="UInt32(ReadOnlySpan{byte}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<uint> UInt32(ReadOnlySpan<byte> utf8, IFormatProvider? provider) => UInt32(utf8, Format(provider));

	/// <summary>
	/// <see cref="UInt32(ReadOnlySpan{char}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<uint> UInt32(ReadOnlySpan<char> input, IFormatProvider? provider) => UInt32(input, Format(provider));

	/// <summary>
	/// <see cref="UInt64(ReadOnlySpan{byte}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<ulong> UInt64(ReadOnlySpan<byte> utf8, IFormatProvider? provider) => UInt64(utf8, Format(provider));

	/// <summary>
	/// <see cref="UInt64(ReadOnlySpan{char}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<ulong> UInt64(ReadOnlySpan<char> input, IFormatProvider? provider) => UInt64(input, Format(provider));

	/// <summary>
	/// <see cref="Single(ReadOnlySpan{byte}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<float> Single(ReadOnlySpan<byte> utf8, IFormatProvider? provider) => Single(utf8, Format(provider));

	/// <summary>
	/// <see cref="Single(ReadOnlySpan{char}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<float> Single(ReadOnlySpan<char> input, IFormatProvider? provider) => Single(input, Format(provider));

	/// <summary>
	/// <see cref="Double(ReadOnlySpan{byte}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<double> Double(ReadOnlySpan<byte> utf8, IFormatProvider? provider) => Double(utf8, Format(provider));

	/// <summary>
	/// <see cref="Double(ReadOnlySpan{char}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<double> Double(ReadOnlySpan<char> input, IFormatProvider? provider) => Double(input, Format(provider));

	/// <summary>
	/// <see cref="Decimal(ReadOnlySpan{byte}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<decimal> Decimal(ReadOnlySpan<byte> utf8, IFormatProvider? provider) => Decimal(utf8, Format(provider));

	/// <summary>
	/// <see cref="Decimal(ReadOnlySpan{char}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<decimal> Decimal(ReadOnlySpan<char> input, IFormatProvider? provider) => Decimal(input, Format(provider));

	/// <summary>
	/// <see cref="Numeric{T}(ReadOnlySpan{byte}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <typeparam name="T">The target type, as for <see cref="Numeric{T}(ReadOnlySpan{byte}, NumFormat)"/>.</typeparam>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	/// <exception cref="NotSupportedException">No door reads <typeparamref name="T"/>, as for <see cref="Numeric{T}(ReadOnlySpan{byte}, NumFormat)"/>.</exception>
	[OverloadResolutionPriority(-1)]
	public static Verdict<T> Numeric<T>(ReadOnlySpan<byte> utf8, IFormatProvider? provider) where T : struct, INumber<T> => Numeric<T>(utf8, Format(provider));

	/// <summary>
	/// <see cref="Numeric{T}(ReadOnlySpan{char}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <typeparam name="T">The target type, as for <see cref="Numeric{T}(ReadOnlySpan{char}, NumFormat)"/>.</typeparam>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture.</param>
	/// <exception cref="NotSupportedException">No door reads <typeparamref name="T"/>, as for <see cref="Numeric{T}(ReadOnlySpan{char}, NumFormat)"/>.</exception>
	[OverloadResolutionPriority(-1)]
	public static Verdict<T> Numeric<T>(ReadOnlySpan<char> input, IFormatProvider? provider) where T : struct, INumber<T> => Numeric<T>(input, Format(provider));

	/// <summary>
	/// <see cref="Scalar{T}(ReadOnlySpan{byte}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <typeparam name="T">The target type, as for <see cref="Scalar{T}(ReadOnlySpan{byte}, NumFormat)"/>.</typeparam>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture. Only the numeric targets read it; every other door ignores it.</param>
	/// <exception cref="NotSupportedException">No door reads <typeparamref name="T"/>, as for <see cref="Scalar{T}(ReadOnlySpan{byte}, NumFormat)"/>.</exception>
	[OverloadResolutionPriority(-1)]
	public static Verdict<T> Scalar<T>(ReadOnlySpan<byte> utf8, IFormatProvider? provider) where T : struct => Scalar<T>(utf8, Format(provider));

	/// <summary>
	/// <see cref="Scalar{T}(ReadOnlySpan{char}, NumFormat)"/> under the number formatting <paramref name="provider"/> resolves
	/// to, resolved as a BCL <c>TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="NumFormat.From(IFormatProvider)"/>'s — every symbol the culture
	/// declares, every lenience on; pass a <see cref="NumFormat"/> for anything stricter.
	/// </summary>
	/// <typeparam name="T">The target type, as for <see cref="Scalar{T}(ReadOnlySpan{char}, NumFormat)"/>.</typeparam>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or number format to read the symbols from; <see langword="null"/> for the current culture. Only the numeric targets read it; every other door ignores it.</param>
	/// <exception cref="NotSupportedException">No door reads <typeparamref name="T"/>, as for <see cref="Scalar{T}(ReadOnlySpan{char}, NumFormat)"/>.</exception>
	[OverloadResolutionPriority(-1)]
	public static Verdict<T> Scalar<T>(ReadOnlySpan<char> input, IFormatProvider? provider) where T : struct => Scalar<T>(input, Format(provider));

	/// <summary>
	/// <see cref="DateTime(ReadOnlySpan{byte}, DateFormat)"/> under the date-time formatting <paramref name="provider"/>
	/// resolves to, resolved as a BCL <c>DateTime.TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="DateFormat.From(IFormatProvider)"/>'s — the order of the culture's short-date pattern
	/// and its day-period designators; declare a <see cref="DateFormat"/> or a bare <see cref="DateOrder"/> for
	/// a dialect that is a property of the text rather than of the process.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="provider">The culture or date-time format to read the order and designators from; <see langword="null"/> for the current culture.</param>
	/// <exception cref="ArgumentException">The resolved short-date pattern names none of y/M/d — a malformed culture, not a data verdict.</exception>
	[OverloadResolutionPriority(-1)]
	public static Verdict<System.DateTime> DateTime(ReadOnlySpan<byte> utf8, IFormatProvider? provider) => DateTime(utf8, DateFormatOf(provider));

	/// <summary>
	/// <see cref="DateTime(ReadOnlySpan{char}, DateFormat)"/> under the date-time formatting <paramref name="provider"/>
	/// resolves to, resolved as a BCL <c>DateTime.TryParse</c> resolves one: <see langword="null"/> means the current culture.
	/// The format is <see cref="DateFormat.From(IFormatProvider)"/>'s — the order of the culture's short-date pattern
	/// and its day-period designators; declare a <see cref="DateFormat"/> or a bare <see cref="DateOrder"/> for
	/// a dialect that is a property of the text rather than of the process.
	/// </summary>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="provider">The culture or date-time format to read the order and designators from; <see langword="null"/> for the current culture.</param>
	/// <exception cref="ArgumentException">The resolved short-date pattern names none of y/M/d — a malformed culture, not a data verdict.</exception>
	[OverloadResolutionPriority(-1)]
	public static Verdict<System.DateTime> DateTime(ReadOnlySpan<char> input, IFormatProvider? provider) => DateTime(input, DateFormatOf(provider));
}
