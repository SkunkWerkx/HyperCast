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
	/// <param name="format">The declared numeric notation — state it out loud (<see cref="NumFormat.Invariant"/>, or <see cref="NumFormat.From"/>).</param>
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

	/// <inheritdoc cref="SByte(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation.</param>
	public static Verdict<sbyte> SByte(ReadOnlySpan<char> input, NumFormat format)
	{
		byte[]? rented = null;
		try
		{
			return SByte(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented), format);
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
			return Int16(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented), format);
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
			return Int32(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented), format);
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
			return Int64(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented), format);
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
			return Byte(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented), format);
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
			return UInt16(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented), format);
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
			return UInt32(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented), format);
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
			return UInt64(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented), format);
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
			return Single(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented), format);
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
			return Double(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented), format);
		}
		finally
		{
			Recycle(rented);
		}
	}
}
