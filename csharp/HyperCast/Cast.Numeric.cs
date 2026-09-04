using System.Numerics;
using System.Runtime.CompilerServices;

namespace HyperCast;

public static partial class Cast
{
	/// <summary>
	/// Casts integer text to <see cref="sbyte"/> under the declared <paramref name="format"/>:
	/// the type's own range, declared grouping, accounting parentheses, non-negative exponent
	/// (<c>1e3</c> is 1000; a decimal point is never accepted), and <c>0x</c>/<c>&amp;H</c>/<c>0b</c>
	/// two's-complement radix prefixes (<c>0xFF</c> is -1).
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation — state it out loud (<see cref="NumFormat.Invariant"/>, or <see cref="NumFormat.From(System.Globalization.CultureInfo)"/>).</param>
	public static unsafe Verdict<sbyte> SByte(ReadOnlySpan<byte> utf8, NumFormat format)
	{
		var raw = format.ToRaw();
		sbyte value = 0;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_i8(ptr, (nuint)utf8.Length, &raw, &value, &fault);
		return code == 0 ? value : Failed<sbyte>(code, fault);
	}

	/// <summary>Casts integer text to <see cref="short"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat)"/> for the shared notation rules.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static unsafe Verdict<short> Int16(ReadOnlySpan<byte> utf8, NumFormat format)
	{
		var raw = format.ToRaw();
		short value = 0;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_i16(ptr, (nuint)utf8.Length, &raw, &value, &fault);
		return code == 0 ? value : Failed<short>(code, fault);
	}

	/// <summary>Casts integer text to <see cref="int"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat)"/> for the shared notation rules.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static unsafe Verdict<int> Int32(ReadOnlySpan<byte> utf8, NumFormat format)
	{
		var raw = format.ToRaw();
		int value = 0;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_i32(ptr, (nuint)utf8.Length, &raw, &value, &fault);
		return code == 0 ? value : Failed<int>(code, fault);
	}

	/// <summary>Casts integer text to <see cref="long"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat)"/> for the shared notation rules.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static unsafe Verdict<long> Int64(ReadOnlySpan<byte> utf8, NumFormat format)
	{
		var raw = format.ToRaw();
		long value = 0;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_i64(ptr, (nuint)utf8.Length, &raw, &value, &fault);
		return code == 0 ? value : Failed<long>(code, fault);
	}

	/// <summary>Casts integer text to <see cref="byte"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat)"/> for the shared notation rules.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static unsafe Verdict<byte> Byte(ReadOnlySpan<byte> utf8, NumFormat format)
	{
		var raw = format.ToRaw();
		byte value = 0;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_u8(ptr, (nuint)utf8.Length, &raw, &value, &fault);
		return code == 0 ? value : Failed<byte>(code, fault);
	}

	/// <summary>Casts integer text to <see cref="ushort"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat)"/> for the shared notation rules.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static unsafe Verdict<ushort> UInt16(ReadOnlySpan<byte> utf8, NumFormat format)
	{
		var raw = format.ToRaw();
		ushort value = 0;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_u16(ptr, (nuint)utf8.Length, &raw, &value, &fault);
		return code == 0 ? value : Failed<ushort>(code, fault);
	}

	/// <summary>Casts integer text to <see cref="uint"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat)"/> for the shared notation rules.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static unsafe Verdict<uint> UInt32(ReadOnlySpan<byte> utf8, NumFormat format)
	{
		var raw = format.ToRaw();
		uint value = 0;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_u32(ptr, (nuint)utf8.Length, &raw, &value, &fault);
		return code == 0 ? value : Failed<uint>(code, fault);
	}

	/// <summary>Casts integer text to <see cref="ulong"/>. See <see cref="SByte(ReadOnlySpan{byte}, NumFormat)"/> for the shared notation rules.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static unsafe Verdict<ulong> UInt64(ReadOnlySpan<byte> utf8, NumFormat format)
	{
		var raw = format.ToRaw();
		ulong value = 0;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_u64(ptr, (nuint)utf8.Length, &raw, &value, &fault);
		return code == 0 ? value : Failed<ulong>(code, fault);
	}

	/// <summary>
	/// Casts real text to <see cref="float"/> under the declared <paramref name="format"/>:
	/// finite values only (<c>NaN</c>/<c>Infinity</c> literals are <see cref="CastFailure.Malformed"/>,
	/// overflow to infinity is <see cref="CastFailure.OutOfRange"/>), declared separators and
	/// grouping, accounting parentheses, exponent, and trailing percent (<c>50%</c> is 0.5).
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static unsafe Verdict<float> Single(ReadOnlySpan<byte> utf8, NumFormat format)
	{
		var raw = format.ToRaw();
		float value = 0;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_f32(ptr, (nuint)utf8.Length, &raw, &value, &fault);
		return code == 0 ? value : Failed<float>(code, fault);
	}

	/// <summary>Casts real text to <see cref="double"/>. See <see cref="Single(ReadOnlySpan{byte}, NumFormat)"/> for the shared notation rules.</summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static unsafe Verdict<double> Double(ReadOnlySpan<byte> utf8, NumFormat format)
	{
		var raw = format.ToRaw();
		double value = 0;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_f64(ptr, (nuint)utf8.Length, &raw, &value, &fault);
		return code == 0 ? value : Failed<double>(code, fault);
	}

	/// <summary>
	/// Casts decimal text to an exact <see cref="decimal"/> under the declared
	/// <paramref name="format"/> — the same notation the real doors read (declared
	/// separators and grouping, accounting parentheses, exponent, trailing percent, declared
	/// currency), but no floating point is ever formed: <c>0.1</c> is one tenth and
	/// <c>50%</c> is exactly <c>0.5</c>. The result is canonical — exact trailing zeros in
	/// the fraction are trimmed, so <c>1.10</c> and <c>1.1</c> both come back with a
	/// <see cref="decimal.Scale"/> of 1, and zero is scale 0, never negative. Precision is a
	/// range, not a rounding opportunity: a magnitude past <see cref="decimal.MaxValue"/> or
	/// more nonzero fractional places than 28 is <see cref="CastFailure.OutOfRange"/>.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static unsafe Verdict<decimal> Decimal(ReadOnlySpan<byte> utf8, NumFormat format)
	{
		var raw = format.ToRaw();
		RawDecimal value = default;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_decimal(ptr, (nuint)utf8.Length, &raw, &value, &fault);
		return code == 0
			? new decimal((int)value.Lo, (int)(value.Lo >> 32), (int)value.Hi, value.Negative != 0, value.Scale)
			: Failed<decimal>(code, fault);
	}

	/// <inheritdoc cref="Decimal(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static Verdict<decimal> Decimal(ReadOnlySpan<char> input, NumFormat format)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Decimal(utf8, format), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <summary>
	/// The numeric doors behind one generic entry point, for a caller that is itself generic
	/// over the target type — a parser family, a column mapper — so it need not write the
	/// <c>typeof</c> dispatch itself. Exactly the eleven numeric targets this binding has a
	/// door for: <see cref="sbyte"/>, <see cref="short"/>, <see cref="int"/>,
	/// <see cref="long"/>, <see cref="byte"/>, <see cref="ushort"/>, <see cref="uint"/>,
	/// <see cref="ulong"/>, <see cref="float"/>, <see cref="double"/> and
	/// <see cref="decimal"/>. The <c>typeof</c> test folds per instantiation, so a given
	/// <typeparamref name="T"/> costs one direct call.
	/// </summary>
	/// <typeparam name="T">The numeric target — one of the eleven above.</typeparam>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation.</param>
	/// <exception cref="NotSupportedException"><typeparamref name="T"/> has no native door — a caller bug, not a data verdict.</exception>
	public static Verdict<T> Numeric<T>(ReadOnlySpan<byte> utf8, NumFormat format) where T : struct, INumber<T>
	{
		if (typeof(T) == typeof(sbyte)) return Fold<sbyte, T>(SByte(utf8, format));
		if (typeof(T) == typeof(short)) return Fold<short, T>(Int16(utf8, format));
		if (typeof(T) == typeof(int)) return Fold<int, T>(Int32(utf8, format));
		if (typeof(T) == typeof(long)) return Fold<long, T>(Int64(utf8, format));
		if (typeof(T) == typeof(byte)) return Fold<byte, T>(Byte(utf8, format));
		if (typeof(T) == typeof(ushort)) return Fold<ushort, T>(UInt16(utf8, format));
		if (typeof(T) == typeof(uint)) return Fold<uint, T>(UInt32(utf8, format));
		if (typeof(T) == typeof(ulong)) return Fold<ulong, T>(UInt64(utf8, format));
		if (typeof(T) == typeof(float)) return Fold<float, T>(Single(utf8, format));
		if (typeof(T) == typeof(double)) return Fold<double, T>(Double(utf8, format));
		if (typeof(T) == typeof(decimal)) return Fold<decimal, T>(Decimal(utf8, format));
		throw new NotSupportedException($"No native door casts to {typeof(T)}.");
	}

	/// <inheritdoc cref="Numeric{T}(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static Verdict<T> Numeric<T>(ReadOnlySpan<char> input, NumFormat format) where T : struct, INumber<T>
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Numeric<T>(utf8, format), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	// Re-types a concrete door's verdict as the caller's T once typeof(T) has proven the two
	// identical — an identity reinterpret, never a conversion, so no value can change.
	static Verdict<T> Fold<TNative, T>(Verdict<TNative> verdict) where TNative : struct where T : struct
	{
		if (verdict.TryGetValue(out Success<TNative> success))
		{
			var value = success.Value;
			return new Verdict<T>(new Success<T>(Unsafe.As<TNative, T>(ref value)));
		}
		verdict.TryGetValue(out Fault fault);
		return new Verdict<T>(fault);
	}

	/// <inheritdoc cref="SByte(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static Verdict<sbyte> SByte(ReadOnlySpan<char> input, NumFormat format)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(SByte(utf8, format), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <inheritdoc cref="Int16(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static Verdict<short> Int16(ReadOnlySpan<char> input, NumFormat format)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Int16(utf8, format), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <inheritdoc cref="Int32(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static Verdict<int> Int32(ReadOnlySpan<char> input, NumFormat format)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Int32(utf8, format), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <inheritdoc cref="Int64(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static Verdict<long> Int64(ReadOnlySpan<char> input, NumFormat format)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Int64(utf8, format), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <inheritdoc cref="Byte(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static Verdict<byte> Byte(ReadOnlySpan<char> input, NumFormat format)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Byte(utf8, format), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <inheritdoc cref="UInt16(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static Verdict<ushort> UInt16(ReadOnlySpan<char> input, NumFormat format)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(UInt16(utf8, format), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <inheritdoc cref="UInt32(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static Verdict<uint> UInt32(ReadOnlySpan<char> input, NumFormat format)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(UInt32(utf8, format), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <inheritdoc cref="UInt64(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static Verdict<ulong> UInt64(ReadOnlySpan<char> input, NumFormat format)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(UInt64(utf8, format), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <inheritdoc cref="Single(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static Verdict<float> Single(ReadOnlySpan<char> input, NumFormat format)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Single(utf8, format), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <inheritdoc cref="Double(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static Verdict<double> Double(ReadOnlySpan<char> input, NumFormat format)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Double(utf8, format), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}
}
