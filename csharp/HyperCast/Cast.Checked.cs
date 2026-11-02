using System.Buffers;
using System.Numerics;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using HyperCast.Interop;

namespace HyperCast;

// The checked doors: each numeric door, and the text door, judged by a door-bound
// Constraints<T> after it parses, through cast_checked, cast_checked_from_f64 and cast_text.
// The declaration itself is judged once, by cast_constraints_check, when it is built, its
// decimal text converted by cast_declared_decimal.
public static partial class Cast
{
	[LibraryImport("hypercast", EntryPoint = "cast_checked")]
	private static unsafe partial int cast_checked_native(uint door, uint param, RawNumFormat* format, RawConstraints* constraints, byte* enumPtr, nuint enumLen, byte* ptr, nuint len, void* value, RawFault* fault, RawCheck* check);
	[LibraryImport("*", EntryPoint = "cast_checked")]
	private static unsafe partial int cast_checked_browser(uint door, uint param, RawNumFormat* format, RawConstraints* constraints, byte* enumPtr, nuint enumLen, byte* ptr, nuint len, void* value, RawFault* fault, RawCheck* check);
	[LibraryImport("__Internal", EntryPoint = "cast_checked")]
	private static unsafe partial int cast_checked_internal(uint door, uint param, RawNumFormat* format, RawConstraints* constraints, byte* enumPtr, nuint enumLen, byte* ptr, nuint len, void* value, RawFault* fault, RawCheck* check);
	private static unsafe int cast_checked(uint door, uint param, RawNumFormat* format, RawConstraints* constraints, byte* enumPtr, nuint enumLen, byte* ptr, nuint len, void* value, RawFault* fault, RawCheck* check) =>
		OperatingSystem.IsBrowser() ? cast_checked_browser(door, param, format, constraints, enumPtr, enumLen, ptr, len, value, fault, check)
			: OperatingSystem.IsIOS() ? cast_checked_internal(door, param, format, constraints, enumPtr, enumLen, ptr, len, value, fault, check)
			: cast_checked_native(door, param, format, constraints, enumPtr, enumLen, ptr, len, value, fault, check);

	[LibraryImport("hypercast", EntryPoint = "cast_checked_from_f64")]
	private static unsafe partial int cast_checked_from_f64_native(uint door, double number, RawConstraints* constraints, byte* enumPtr, nuint enumLen, void* value, RawFault* fault, RawCheck* check);
	[LibraryImport("*", EntryPoint = "cast_checked_from_f64")]
	private static unsafe partial int cast_checked_from_f64_browser(uint door, double number, RawConstraints* constraints, byte* enumPtr, nuint enumLen, void* value, RawFault* fault, RawCheck* check);
	[LibraryImport("__Internal", EntryPoint = "cast_checked_from_f64")]
	private static unsafe partial int cast_checked_from_f64_internal(uint door, double number, RawConstraints* constraints, byte* enumPtr, nuint enumLen, void* value, RawFault* fault, RawCheck* check);
	private static unsafe int cast_checked_from_f64(uint door, double number, RawConstraints* constraints, byte* enumPtr, nuint enumLen, void* value, RawFault* fault, RawCheck* check) =>
		OperatingSystem.IsBrowser() ? cast_checked_from_f64_browser(door, number, constraints, enumPtr, enumLen, value, fault, check)
			: OperatingSystem.IsIOS() ? cast_checked_from_f64_internal(door, number, constraints, enumPtr, enumLen, value, fault, check)
			: cast_checked_from_f64_native(door, number, constraints, enumPtr, enumLen, value, fault, check);

	[LibraryImport("hypercast", EntryPoint = "cast_constraints_check")]
	private static unsafe partial int cast_constraints_check_native(uint door, uint param, RawConstraints* constraints, byte* enumPtr, nuint enumLen);
	[LibraryImport("*", EntryPoint = "cast_constraints_check")]
	private static unsafe partial int cast_constraints_check_browser(uint door, uint param, RawConstraints* constraints, byte* enumPtr, nuint enumLen);
	[LibraryImport("__Internal", EntryPoint = "cast_constraints_check")]
	private static unsafe partial int cast_constraints_check_internal(uint door, uint param, RawConstraints* constraints, byte* enumPtr, nuint enumLen);
	private static unsafe int cast_constraints_check(uint door, uint param, RawConstraints* constraints, byte* enumPtr, nuint enumLen) =>
		OperatingSystem.IsBrowser() ? cast_constraints_check_browser(door, param, constraints, enumPtr, enumLen)
			: OperatingSystem.IsIOS() ? cast_constraints_check_internal(door, param, constraints, enumPtr, enumLen)
			: cast_constraints_check_native(door, param, constraints, enumPtr, enumLen);

	[LibraryImport("hypercast", EntryPoint = "cast_text")]
	private static unsafe partial int cast_text_native(byte* ptr, nuint len, RawSpan* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_text")]
	private static unsafe partial int cast_text_browser(byte* ptr, nuint len, RawSpan* value, RawFault* fault);
	[LibraryImport("__Internal", EntryPoint = "cast_text")]
	private static unsafe partial int cast_text_internal(byte* ptr, nuint len, RawSpan* value, RawFault* fault);
	private static unsafe int cast_text(byte* ptr, nuint len, RawSpan* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_text_browser(ptr, len, value, fault)
			: OperatingSystem.IsIOS() ? cast_text_internal(ptr, len, value, fault)
			: cast_text_native(ptr, len, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_declared_decimal")]
	private static unsafe partial int cast_declared_decimal_native(byte* ptr, nuint len, RawDecimal* value);
	[LibraryImport("*", EntryPoint = "cast_declared_decimal")]
	private static unsafe partial int cast_declared_decimal_browser(byte* ptr, nuint len, RawDecimal* value);
	[LibraryImport("__Internal", EntryPoint = "cast_declared_decimal")]
	private static unsafe partial int cast_declared_decimal_internal(byte* ptr, nuint len, RawDecimal* value);
	private static unsafe int cast_declared_decimal(byte* ptr, nuint len, RawDecimal* value) =>
		OperatingSystem.IsBrowser() ? cast_declared_decimal_browser(ptr, len, value)
			: OperatingSystem.IsIOS() ? cast_declared_decimal_internal(ptr, len, value)
			: cast_declared_decimal_native(ptr, len, value);

	/// <summary>
	/// A declaration's decimal text converted by the core (<c>cast_declared_decimal</c>), the one
	/// conversion every binding shares: <see langword="false"/> for text it refuses (its code 8).
	/// The text is transcoded without replacement, so a lone surrogate reaches it as the
	/// invalid UTF-8 it is.
	/// </summary>
	internal static unsafe bool DeclaredDecimal(string text, out RawDecimal value)
	{
		byte[]? rented = null;
		try
		{
			var length = Wtf8Length(text);
			Span<byte> target = length <= Utf8StackBytes
				? stackalloc byte[Utf8StackBytes]
				: rented = ArrayPool<byte>.Shared.Rent(length);
			var utf8 = target[..Wtf8(text, target)];
			RawDecimal declared = default;
			int code;
			fixed (byte* ptr = utf8)
				code = cast_declared_decimal(ptr, (nuint)utf8.Length, &declared);
			value = declared;
			return code switch
			{
				0 => true,
				8 => false,
				_ => throw new InvalidOperationException(
					"libhypercast reported a contract violation — a binding bug, please report it."),
			};
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <summary>The core's verdict on a declaration: 0, or the code of the first rule it breaks.</summary>
	internal static unsafe int CheckConstraints(uint door, RawConstraints raw, ReadOnlySpan<byte> packedEnum)
	{
		fixed (byte* entries = packedEnum)
			return cast_constraints_check(door, 0, &raw, entries, (nuint)packedEnum.Length);
	}

	/// <summary>
	/// Casts integer text to <see cref="sbyte"/> under the declared <paramref name="format"/>, as
	/// <see cref="SByte(ReadOnlySpan{byte}, NumFormat)"/> does, then judges the value by
	/// <paramref name="constraints"/>: a <see cref="Checked{T}"/>, or a <see cref="Rejection"/>
	/// holding the plain door's own <see cref="Fault"/> or the <see cref="Violation"/> of a
	/// declared rule, spanning the value. Every door below has the same shape.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<sbyte> SByte(ReadOnlySpan<byte> utf8, NumFormat format, Constraints<sbyte> constraints) =>
		Judged(utf8, format, constraints);

	/// <inheritdoc cref="SByte(ReadOnlySpan{byte}, NumFormat, Constraints{sbyte})"/>
	/// <param name="input">The raw scalar text; spans count UTF-16 code units.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	public static CheckedVerdict<sbyte> SByte(ReadOnlySpan<char> input, NumFormat format, Constraints<sbyte> constraints) =>
		JudgedChars(input, format, constraints);

	/// <summary>
	/// Judges a number a source already decoded (a JSON parser's double, a spreadsheet's
	/// numeric cell) at the <see cref="sbyte"/> door by <paramref name="constraints"/>, on its
	/// sign and the shortest decimal that rounds back to it: the text
	/// <see cref="DecimalFromDouble(double)"/> says a double is. <c>-0.0</c> is zero. NaN is
	/// <see cref="CastFailure.Malformed"/> and an infinity <see cref="CastFailure.OutOfRange"/>;
	/// a fraction at an integer door is <see cref="CastFailure.Malformed"/> unless a rounding
	/// takes it to a multiple. With no text to index, every fault and violation has an empty span.
	/// </summary>
	/// <param name="value">The number.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<sbyte> SByteFromDouble(double value, Constraints<sbyte> constraints) =>
		FromDouble(value, constraints);

	/// <summary>The checked <see cref="Int16(ReadOnlySpan{byte}, NumFormat)"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat, Constraints{sbyte})"/>.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<short> Int16(ReadOnlySpan<byte> utf8, NumFormat format, Constraints<short> constraints) =>
		Judged(utf8, format, constraints);

	/// <inheritdoc cref="Int16(ReadOnlySpan{byte}, NumFormat, Constraints{short})"/>
	/// <param name="input">The raw scalar text; spans count UTF-16 code units.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	public static CheckedVerdict<short> Int16(ReadOnlySpan<char> input, NumFormat format, Constraints<short> constraints) =>
		JudgedChars(input, format, constraints);

	/// <summary>
	/// Judges a number a source already decoded (a JSON parser's double, a spreadsheet's
	/// numeric cell) at the <see cref="short"/> door by <paramref name="constraints"/>, on its
	/// sign and the shortest decimal that rounds back to it: the text
	/// <see cref="DecimalFromDouble(double)"/> says a double is. <c>-0.0</c> is zero. NaN is
	/// <see cref="CastFailure.Malformed"/> and an infinity <see cref="CastFailure.OutOfRange"/>;
	/// a fraction at an integer door is <see cref="CastFailure.Malformed"/> unless a rounding
	/// takes it to a multiple. With no text to index, every fault and violation has an empty span.
	/// </summary>
	/// <param name="value">The number.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<short> Int16FromDouble(double value, Constraints<short> constraints) =>
		FromDouble(value, constraints);

	/// <summary>The checked <see cref="Int32(ReadOnlySpan{byte}, NumFormat)"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat, Constraints{sbyte})"/>.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<int> Int32(ReadOnlySpan<byte> utf8, NumFormat format, Constraints<int> constraints) =>
		Judged(utf8, format, constraints);

	/// <inheritdoc cref="Int32(ReadOnlySpan{byte}, NumFormat, Constraints{int})"/>
	/// <param name="input">The raw scalar text; spans count UTF-16 code units.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	public static CheckedVerdict<int> Int32(ReadOnlySpan<char> input, NumFormat format, Constraints<int> constraints) =>
		JudgedChars(input, format, constraints);

	/// <summary>
	/// Judges a number a source already decoded (a JSON parser's double, a spreadsheet's
	/// numeric cell) at the <see cref="int"/> door by <paramref name="constraints"/>, on its
	/// sign and the shortest decimal that rounds back to it: the text
	/// <see cref="DecimalFromDouble(double)"/> says a double is. <c>-0.0</c> is zero. NaN is
	/// <see cref="CastFailure.Malformed"/> and an infinity <see cref="CastFailure.OutOfRange"/>;
	/// a fraction at an integer door is <see cref="CastFailure.Malformed"/> unless a rounding
	/// takes it to a multiple. With no text to index, every fault and violation has an empty span.
	/// </summary>
	/// <param name="value">The number.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<int> Int32FromDouble(double value, Constraints<int> constraints) =>
		FromDouble(value, constraints);

	/// <summary>The checked <see cref="Int64(ReadOnlySpan{byte}, NumFormat)"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat, Constraints{sbyte})"/>.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<long> Int64(ReadOnlySpan<byte> utf8, NumFormat format, Constraints<long> constraints) =>
		Judged(utf8, format, constraints);

	/// <inheritdoc cref="Int64(ReadOnlySpan{byte}, NumFormat, Constraints{long})"/>
	/// <param name="input">The raw scalar text; spans count UTF-16 code units.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	public static CheckedVerdict<long> Int64(ReadOnlySpan<char> input, NumFormat format, Constraints<long> constraints) =>
		JudgedChars(input, format, constraints);

	/// <summary>
	/// Judges a number a source already decoded (a JSON parser's double, a spreadsheet's
	/// numeric cell) at the <see cref="long"/> door by <paramref name="constraints"/>, on its
	/// sign and the shortest decimal that rounds back to it: the text
	/// <see cref="DecimalFromDouble(double)"/> says a double is. <c>-0.0</c> is zero. NaN is
	/// <see cref="CastFailure.Malformed"/> and an infinity <see cref="CastFailure.OutOfRange"/>;
	/// a fraction at an integer door is <see cref="CastFailure.Malformed"/> unless a rounding
	/// takes it to a multiple. With no text to index, every fault and violation has an empty span.
	/// </summary>
	/// <param name="value">The number.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<long> Int64FromDouble(double value, Constraints<long> constraints) =>
		FromDouble(value, constraints);

	/// <summary>The checked <see cref="Byte(ReadOnlySpan{byte}, NumFormat)"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat, Constraints{sbyte})"/>.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<byte> Byte(ReadOnlySpan<byte> utf8, NumFormat format, Constraints<byte> constraints) =>
		Judged(utf8, format, constraints);

	/// <inheritdoc cref="Byte(ReadOnlySpan{byte}, NumFormat, Constraints{byte})"/>
	/// <param name="input">The raw scalar text; spans count UTF-16 code units.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	public static CheckedVerdict<byte> Byte(ReadOnlySpan<char> input, NumFormat format, Constraints<byte> constraints) =>
		JudgedChars(input, format, constraints);

	/// <summary>
	/// Judges a number a source already decoded (a JSON parser's double, a spreadsheet's
	/// numeric cell) at the <see cref="byte"/> door by <paramref name="constraints"/>, on its
	/// sign and the shortest decimal that rounds back to it: the text
	/// <see cref="DecimalFromDouble(double)"/> says a double is. <c>-0.0</c> is zero. NaN is
	/// <see cref="CastFailure.Malformed"/> and an infinity <see cref="CastFailure.OutOfRange"/>;
	/// a fraction at an integer door is <see cref="CastFailure.Malformed"/> unless a rounding
	/// takes it to a multiple. With no text to index, every fault and violation has an empty span.
	/// </summary>
	/// <param name="value">The number.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<byte> ByteFromDouble(double value, Constraints<byte> constraints) =>
		FromDouble(value, constraints);

	/// <summary>The checked <see cref="UInt16(ReadOnlySpan{byte}, NumFormat)"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat, Constraints{sbyte})"/>.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<ushort> UInt16(ReadOnlySpan<byte> utf8, NumFormat format, Constraints<ushort> constraints) =>
		Judged(utf8, format, constraints);

	/// <inheritdoc cref="UInt16(ReadOnlySpan{byte}, NumFormat, Constraints{ushort})"/>
	/// <param name="input">The raw scalar text; spans count UTF-16 code units.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	public static CheckedVerdict<ushort> UInt16(ReadOnlySpan<char> input, NumFormat format, Constraints<ushort> constraints) =>
		JudgedChars(input, format, constraints);

	/// <summary>
	/// Judges a number a source already decoded (a JSON parser's double, a spreadsheet's
	/// numeric cell) at the <see cref="ushort"/> door by <paramref name="constraints"/>, on its
	/// sign and the shortest decimal that rounds back to it: the text
	/// <see cref="DecimalFromDouble(double)"/> says a double is. <c>-0.0</c> is zero. NaN is
	/// <see cref="CastFailure.Malformed"/> and an infinity <see cref="CastFailure.OutOfRange"/>;
	/// a fraction at an integer door is <see cref="CastFailure.Malformed"/> unless a rounding
	/// takes it to a multiple. With no text to index, every fault and violation has an empty span.
	/// </summary>
	/// <param name="value">The number.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<ushort> UInt16FromDouble(double value, Constraints<ushort> constraints) =>
		FromDouble(value, constraints);

	/// <summary>The checked <see cref="UInt32(ReadOnlySpan{byte}, NumFormat)"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat, Constraints{sbyte})"/>.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<uint> UInt32(ReadOnlySpan<byte> utf8, NumFormat format, Constraints<uint> constraints) =>
		Judged(utf8, format, constraints);

	/// <inheritdoc cref="UInt32(ReadOnlySpan{byte}, NumFormat, Constraints{uint})"/>
	/// <param name="input">The raw scalar text; spans count UTF-16 code units.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	public static CheckedVerdict<uint> UInt32(ReadOnlySpan<char> input, NumFormat format, Constraints<uint> constraints) =>
		JudgedChars(input, format, constraints);

	/// <summary>
	/// Judges a number a source already decoded (a JSON parser's double, a spreadsheet's
	/// numeric cell) at the <see cref="uint"/> door by <paramref name="constraints"/>, on its
	/// sign and the shortest decimal that rounds back to it: the text
	/// <see cref="DecimalFromDouble(double)"/> says a double is. <c>-0.0</c> is zero. NaN is
	/// <see cref="CastFailure.Malformed"/> and an infinity <see cref="CastFailure.OutOfRange"/>;
	/// a fraction at an integer door is <see cref="CastFailure.Malformed"/> unless a rounding
	/// takes it to a multiple. With no text to index, every fault and violation has an empty span.
	/// </summary>
	/// <param name="value">The number.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<uint> UInt32FromDouble(double value, Constraints<uint> constraints) =>
		FromDouble(value, constraints);

	/// <summary>The checked <see cref="UInt64(ReadOnlySpan{byte}, NumFormat)"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat, Constraints{sbyte})"/>.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<ulong> UInt64(ReadOnlySpan<byte> utf8, NumFormat format, Constraints<ulong> constraints) =>
		Judged(utf8, format, constraints);

	/// <inheritdoc cref="UInt64(ReadOnlySpan{byte}, NumFormat, Constraints{ulong})"/>
	/// <param name="input">The raw scalar text; spans count UTF-16 code units.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	public static CheckedVerdict<ulong> UInt64(ReadOnlySpan<char> input, NumFormat format, Constraints<ulong> constraints) =>
		JudgedChars(input, format, constraints);

	/// <summary>
	/// Judges a number a source already decoded (a JSON parser's double, a spreadsheet's
	/// numeric cell) at the <see cref="ulong"/> door by <paramref name="constraints"/>, on its
	/// sign and the shortest decimal that rounds back to it: the text
	/// <see cref="DecimalFromDouble(double)"/> says a double is. <c>-0.0</c> is zero. NaN is
	/// <see cref="CastFailure.Malformed"/> and an infinity <see cref="CastFailure.OutOfRange"/>;
	/// a fraction at an integer door is <see cref="CastFailure.Malformed"/> unless a rounding
	/// takes it to a multiple. With no text to index, every fault and violation has an empty span.
	/// </summary>
	/// <param name="value">The number.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<ulong> UInt64FromDouble(double value, Constraints<ulong> constraints) =>
		FromDouble(value, constraints);

	/// <summary>The checked <see cref="Single(ReadOnlySpan{byte}, NumFormat)"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat, Constraints{sbyte})"/>.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<float> Single(ReadOnlySpan<byte> utf8, NumFormat format, Constraints<float> constraints) =>
		Judged(utf8, format, constraints);

	/// <inheritdoc cref="Single(ReadOnlySpan{byte}, NumFormat, Constraints{float})"/>
	/// <param name="input">The raw scalar text; spans count UTF-16 code units.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	public static CheckedVerdict<float> Single(ReadOnlySpan<char> input, NumFormat format, Constraints<float> constraints) =>
		JudgedChars(input, format, constraints);

	/// <summary>
	/// Judges a number a source already decoded (a JSON parser's double, a spreadsheet's
	/// numeric cell) at the <see cref="float"/> door by <paramref name="constraints"/>, on its
	/// sign and the shortest decimal that rounds back to it: the text
	/// <see cref="DecimalFromDouble(double)"/> says a double is. <c>-0.0</c> is zero. NaN is
	/// <see cref="CastFailure.Malformed"/> and an infinity <see cref="CastFailure.OutOfRange"/>;
	/// a fraction at an integer door is <see cref="CastFailure.Malformed"/> unless a rounding
	/// takes it to a multiple. With no text to index, every fault and violation has an empty span.
	/// </summary>
	/// <param name="value">The number.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<float> SingleFromDouble(double value, Constraints<float> constraints) =>
		FromDouble(value, constraints);

	/// <summary>The checked <see cref="Double(ReadOnlySpan{byte}, NumFormat)"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat, Constraints{sbyte})"/>.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<double> Double(ReadOnlySpan<byte> utf8, NumFormat format, Constraints<double> constraints) =>
		Judged(utf8, format, constraints);

	/// <inheritdoc cref="Double(ReadOnlySpan{byte}, NumFormat, Constraints{double})"/>
	/// <param name="input">The raw scalar text; spans count UTF-16 code units.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	public static CheckedVerdict<double> Double(ReadOnlySpan<char> input, NumFormat format, Constraints<double> constraints) =>
		JudgedChars(input, format, constraints);

	/// <summary>
	/// Judges a number a source already decoded (a JSON parser's double, a spreadsheet's
	/// numeric cell) at the <see cref="double"/> door by <paramref name="constraints"/>, on its
	/// sign and the shortest decimal that rounds back to it: the text
	/// <see cref="DecimalFromDouble(double)"/> says a double is. <c>-0.0</c> is zero. NaN is
	/// <see cref="CastFailure.Malformed"/> and an infinity <see cref="CastFailure.OutOfRange"/>;
	/// a fraction at an integer door is <see cref="CastFailure.Malformed"/> unless a rounding
	/// takes it to a multiple. With no text to index, every fault and violation has an empty span.
	/// </summary>
	/// <param name="value">The number.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<double> DoubleFromDouble(double value, Constraints<double> constraints) =>
		FromDouble(value, constraints);

	/// <summary>The checked <see cref="Decimal(ReadOnlySpan{byte}, NumFormat)"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat, Constraints{sbyte})"/>.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<decimal> Decimal(ReadOnlySpan<byte> utf8, NumFormat format, Constraints<decimal> constraints) =>
		Judged(utf8, format, constraints);

	/// <inheritdoc cref="Decimal(ReadOnlySpan{byte}, NumFormat, Constraints{decimal})"/>
	/// <param name="input">The raw scalar text; spans count UTF-16 code units.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	public static CheckedVerdict<decimal> Decimal(ReadOnlySpan<char> input, NumFormat format, Constraints<decimal> constraints) =>
		JudgedChars(input, format, constraints);

	/// <summary>
	/// Judges a number a source already decoded (a JSON parser's double, a spreadsheet's
	/// numeric cell) at the <see cref="decimal"/> door by <paramref name="constraints"/>, on its
	/// sign and the shortest decimal that rounds back to it: the text
	/// <see cref="DecimalFromDouble(double)"/> says a double is. <c>-0.0</c> is zero. NaN is
	/// <see cref="CastFailure.Malformed"/> and an infinity <see cref="CastFailure.OutOfRange"/>;
	/// a fraction at an integer door is <see cref="CastFailure.Malformed"/> unless a rounding
	/// takes it to a multiple. With no text to index, every fault and violation has an empty span.
	/// </summary>
	/// <param name="value">The number.</param>
	/// <param name="constraints">The declaration, bound to this door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<decimal> DecimalFromDouble(double value, Constraints<decimal> constraints) =>
		FromDouble(value, constraints);

	/// <summary>
	/// The numeric doors behind one generic entry point, checked: <see cref="Numeric{T}(ReadOnlySpan{byte}, NumFormat)"/>
	/// with the declaration's door for <typeparamref name="T"/>, which the
	/// <see cref="Constraints{T}"/> already names.
	/// </summary>
	/// <typeparam name="T">The numeric target, one of the eleven <see cref="Numeric{T}(ReadOnlySpan{byte}, NumFormat)"/> takes.</typeparam>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to <typeparamref name="T"/>'s door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<T> Numeric<T>(ReadOnlySpan<byte> utf8, NumFormat format, Constraints<T> constraints)
		where T : unmanaged, INumber<T> =>
		Judged(utf8, format, constraints);

	/// <inheritdoc cref="Numeric{T}(ReadOnlySpan{byte}, NumFormat, Constraints{T})"/>
	/// <param name="input">The raw scalar text; spans count UTF-16 code units.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <param name="constraints">The declaration, bound to <typeparamref name="T"/>'s door.</param>
	public static CheckedVerdict<T> Numeric<T>(ReadOnlySpan<char> input, NumFormat format, Constraints<T> constraints)
		where T : unmanaged, INumber<T> =>
		JudgedChars(input, format, constraints);

	/// <summary>
	/// Casts text through the text door: valid UTF-8 that is not empty, untrimmed. Invalid
	/// UTF-8 is <see cref="CastFailure.Malformed"/>, spanning the first invalid sequence; empty
	/// input is <see cref="CastFailure.Empty"/>. The text door's value is a <see cref="string"/>,
	/// which <see cref="Verdict{T}"/> does not hold, so it answers in a
	/// <see cref="CheckedVerdict{T}"/> whose rejection, with nothing declared, is always a
	/// <see cref="Fault"/>.
	/// </summary>
	/// <param name="utf8">The raw text as UTF-8 bytes; the value is decoded from them.</param>
	public static CheckedVerdict<string> Text(ReadOnlySpan<byte> utf8)
	{
		var code = JudgeText(utf8, null, out var span, out var fault, out var check);
		return code == 0
			? new Checked<string>(Encoding.UTF8.GetString(utf8.Slice((int)span.Offset, (int)span.Length)), false, null)
			: Rejected<string>(code, fault, check);
	}

	/// <inheritdoc cref="Text(ReadOnlySpan{byte})"/>
	/// <param name="input">
	/// The raw text; spans count UTF-16 code units, and the value is this string itself. A lone
	/// surrogate is not text: it is <see cref="CastFailure.Malformed"/>, never replaced.
	/// </param>
	/// <exception cref="ArgumentNullException"><paramref name="input"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<string> Text(string input)
	{
		ArgumentNullException.ThrowIfNull(input);
		return TextChars(input, input, null);
	}

	/// <inheritdoc cref="Text(ReadOnlySpan{byte})"/>
	/// <param name="input">The raw text; spans count UTF-16 code units. A lone surrogate is <see cref="CastFailure.Malformed"/>, never replaced.</param>
	public static CheckedVerdict<string> Text(ReadOnlySpan<char> input) => TextChars(input, null, null);

	/// <summary>
	/// Casts text through the text door, then judges it by <paramref name="constraints"/>: trimmed
	/// of Unicode <c>White_Space</c> when declared (trimmed to nothing is
	/// <see cref="CastFailure.Empty"/>), its length counted in code points, and with an
	/// <c>enum</c>, matched under Unicode simple case folding. A match delivers the caller's own
	/// declared string and its index, so <c>"MALE"</c> against <c>["Female", "Male", "Other"]</c>
	/// is <c>"Male"</c> and 1. A violation spans the (trimmed) text.
	/// </summary>
	/// <param name="utf8">The raw text as UTF-8 bytes.</param>
	/// <param name="constraints">The declaration, bound to the text door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<string> Text(ReadOnlySpan<byte> utf8, Constraints<string> constraints)
	{
		ArgumentNullException.ThrowIfNull(constraints);
		var code = JudgeText(utf8, constraints, out var span, out var fault, out var check);
		if (code != 0)
			return Rejected<string>(code, fault, check);
		var value = check.EnumIndex != uint.MaxValue
			? constraints.Entry(check.EnumIndex)
			: Encoding.UTF8.GetString(utf8.Slice((int)span.Offset, (int)span.Length));
		return Found(value, check);
	}

	/// <inheritdoc cref="Text(ReadOnlySpan{byte}, Constraints{string})"/>
	/// <param name="input">
	/// The raw text; spans count UTF-16 code units, and the value is this string itself when
	/// nothing was trimmed. A lone surrogate is <see cref="CastFailure.Malformed"/>, never replaced.
	/// </param>
	/// <param name="constraints">The declaration, bound to the text door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="input"/> or <paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<string> Text(string input, Constraints<string> constraints)
	{
		ArgumentNullException.ThrowIfNull(input);
		ArgumentNullException.ThrowIfNull(constraints);
		return TextChars(input, input, constraints);
	}

	/// <inheritdoc cref="Text(ReadOnlySpan{byte}, Constraints{string})"/>
	/// <param name="input">The raw text; spans count UTF-16 code units. A lone surrogate is <see cref="CastFailure.Malformed"/>, never replaced.</param>
	/// <param name="constraints">The declaration, bound to the text door.</param>
	/// <exception cref="ArgumentNullException"><paramref name="constraints"/> is <see langword="null"/>.</exception>
	public static CheckedVerdict<string> Text(ReadOnlySpan<char> input, Constraints<string> constraints)
	{
		ArgumentNullException.ThrowIfNull(constraints);
		return TextChars(input, null, constraints);
	}

	static unsafe CheckedVerdict<T> Judged<T>(ReadOnlySpan<byte> utf8, NumFormat format, Constraints<T> constraints)
		where T : unmanaged
	{
		ArgumentNullException.ThrowIfNull(constraints);
		var raw = format.ToRaw();
		var declared = constraints.ToRaw();
		RawFault fault = default;
		RawCheck check = default;
		int code;
		var entries = constraints.PackedEnum;
		if (typeof(T) == typeof(decimal))
		{
			RawDecimal value = default;
			fixed (byte* ptr = utf8)
			fixed (byte* list = entries)
				code = cast_checked(constraints.Door, 0, &raw, &declared, list, (nuint)entries.Length,
					ptr, (nuint)utf8.Length, &value, &fault, &check);
			return code == 0 ? FoundDecimal<T>(value, check) : Rejected<T>(code, fault, check);
		}
		else
		{
			T value = default;
			fixed (byte* ptr = utf8)
			fixed (byte* list = entries)
				code = cast_checked(constraints.Door, 0, &raw, &declared, list, (nuint)entries.Length,
					ptr, (nuint)utf8.Length, &value, &fault, &check);
			return code == 0 ? Found(value, check) : Rejected<T>(code, fault, check);
		}
	}

	static CheckedVerdict<T> JudgedChars<T>(ReadOnlySpan<char> input, NumFormat format, Constraints<T> constraints)
		where T : unmanaged
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Judged(utf8, format, constraints), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	static unsafe CheckedVerdict<T> FromDouble<T>(double number, Constraints<T> constraints) where T : unmanaged
	{
		ArgumentNullException.ThrowIfNull(constraints);
		var declared = constraints.ToRaw();
		RawFault fault = default;
		RawCheck check = default;
		int code;
		var entries = constraints.PackedEnum;
		if (typeof(T) == typeof(decimal))
		{
			RawDecimal value = default;
			fixed (byte* list = entries)
				code = cast_checked_from_f64(constraints.Door, number, &declared, list, (nuint)entries.Length, &value, &fault, &check);
			return code == 0 ? FoundDecimal<T>(value, check) : Rejected<T>(code, fault, check);
		}
		else
		{
			T value = default;
			fixed (byte* list = entries)
				code = cast_checked_from_f64(constraints.Door, number, &declared, list, (nuint)entries.Length, &value, &fault, &check);
			return code == 0 ? Found(value, check) : Rejected<T>(code, fault, check);
		}
	}

	// A success at a numeric door: the value, and whether rounding changed it. An integer enum
	// has no index; the value is the answer.
	static CheckedVerdict<T> Found<T>(T value, in RawCheck check) where T : notnull =>
		new(new Checked<T>(value, check.Adjusted != 0, check.EnumIndex == uint.MaxValue ? null : (int)check.EnumIndex));

	// The decimal door writes the core's triple; T is decimal here, proven by typeof.
	static CheckedVerdict<T> FoundDecimal<T>(RawDecimal value, in RawCheck check) where T : notnull
	{
		var number = value.ToDecimal();
		return Found(Unsafe.As<decimal, T>(ref number), check);
	}

	static CheckedVerdict<T> Rejected<T>(int code, in RawFault fault, in RawCheck check) where T : notnull =>
		code switch
		{
			4 => new(new Rejection(Abi.ToViolation(check.Keyword, fault))),
			-1 => throw new InvalidOperationException(
				"libhypercast reported a contract violation — a binding bug, please report it."),
			_ => new(new Rejection(Abi.ToFault((uint)code, fault))),
		};

	static unsafe int JudgeText(ReadOnlySpan<byte> utf8, Constraints<string>? constraints,
		out RawSpan span, out RawFault fault, out RawCheck check)
	{
		RawSpan value = default;
		RawFault failure = default;
		RawCheck report = new(0, 0, uint.MaxValue);
		int code;
		if (constraints is null)
		{
			fixed (byte* ptr = utf8)
				code = cast_text(ptr, (nuint)utf8.Length, &value, &failure);
		}
		else
		{
			var declared = constraints.ToRaw();
			var entries = constraints.PackedEnum;
			fixed (byte* ptr = utf8)
			fixed (byte* list = entries)
				code = cast_checked(constraints.Door, 0, null, &declared, list, (nuint)entries.Length,
					ptr, (nuint)utf8.Length, &value, &failure, &report);
		}
		(span, fault, check) = (value, failure, report);
		return code;
	}

	// The text door over UTF-16: transcoded without replacement (a lone surrogate reaches the
	// core as the invalid sequence it is), the value sliced from the caller's own input, and
	// every span re-expressed in code units.
	static CheckedVerdict<string> TextChars(ReadOnlySpan<char> input, string? whole, Constraints<string>? constraints)
	{
		byte[]? rented = null;
		try
		{
			var length = Encoding.UTF8.GetByteCount(input);
			Span<byte> target = length <= Utf8StackBytes
				? stackalloc byte[Utf8StackBytes]
				: rented = ArrayPool<byte>.Shared.Rent(length);
			var utf8 = target[..Wtf8(input, target)];
			var code = JudgeText(utf8, constraints, out var span, out var fault, out var check);
			if (code != 0)
				return Remap(Rejected<string>(code, fault, check), utf8, input.Length);
			if (check.EnumIndex != uint.MaxValue)
				return Found(constraints!.Entry(check.EnumIndex), check);
			var offset = Units(utf8[..(int)span.Offset]);
			var count = Units(utf8.Slice((int)span.Offset, (int)span.Length));
			var value = whole is not null && offset == 0 && count == whole.Length
				? whole
				: new string(input.Slice(offset, count));
			return Found(value, check);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <summary>
	/// Re-expresses a rejection's span in UTF-16 code units for the UTF-16 doors, as
	/// <see cref="Remap{T}(Verdict{T}, ReadOnlySpan{byte}, int)"/> does a fault's.
	/// </summary>
	static CheckedVerdict<T> Remap<T>(CheckedVerdict<T> verdict, ReadOnlySpan<byte> utf8, int charCount) where T : notnull
	{
		if (utf8.Length == charCount || !verdict.TryGetValue(out Rejection rejection))
			return verdict;
		if (rejection.TryGetValue(out Fault fault))
		{
			if (fault.Offset == 0 && fault.Length == 0)
				return verdict;
			var (offset, length) = Units(utf8, fault.Offset, fault.Length);
			return new(new Rejection(fault with { Offset = offset, Length = length }));
		}
		rejection.TryGetValue(out Violation violation);
		if (violation.Offset == 0 && violation.Length == 0)
			return verdict;
		var (start, units) = Units(utf8, violation.Offset, violation.Length);
		return new(new Rejection(violation with { Offset = start, Length = units }));
	}

	static (int Offset, int Length) Units(ReadOnlySpan<byte> utf8, int offset, int length) =>
		(Units(utf8[..offset]), Units(utf8.Slice(offset, length)));

	// UTF-16 code units in well-formed UTF-8 or WTF-8: one per sequence, two for a four-byte one.
	static int Units(ReadOnlySpan<byte> utf8)
	{
		var units = 0;
		foreach (var b in utf8)
		{
			if ((b & 0xC0) != 0x80)
				units++;
			if (b >= 0xF0)
				units++;
		}
		return units;
	}

	/// <summary>The bytes <see cref="Wtf8(ReadOnlySpan{char}, Span{byte})"/> writes for <paramref name="chars"/>.</summary>
	// A lone surrogate takes three bytes either way: as U+FFFD here, as itself there.
	internal static int Wtf8Length(ReadOnlySpan<char> chars) => Encoding.UTF8.GetByteCount(chars);

	/// <summary>
	/// Encodes UTF-16 as UTF-8, a lone surrogate as the three-byte sequence of its own code
	/// point (WTF-8) rather than U+FFFD, so the core sees text that is not valid UTF-8 as
	/// exactly that. <paramref name="destination"/> holds at least
	/// <see cref="Wtf8Length(ReadOnlySpan{char})"/> bytes.
	/// </summary>
	internal static int Wtf8(ReadOnlySpan<char> chars, Span<byte> destination)
	{
		var written = 0;
		while (true)
		{
			var status = System.Text.Unicode.Utf8.FromUtf16(chars, destination[written..], out var read, out var wrote,
				replaceInvalidSequences: false);
			written += wrote;
			if (status != OperationStatus.InvalidData)
				return written;
			var lone = chars[read];
			destination[written++] = (byte)(0xE0 | lone >> 12);
			destination[written++] = (byte)(0x80 | (lone >> 6) & 0x3F);
			destination[written++] = (byte)(0x80 | lone & 0x3F);
			chars = chars[(read + 1)..];
		}
	}
}
