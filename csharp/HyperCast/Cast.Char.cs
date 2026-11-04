using HyperCast.Interop;

namespace HyperCast;

public static partial class Cast
{
	/// <summary>
	/// Casts char text: an input that is exactly one character is that character, taken
	/// <b>before</b> trimming (<c>" "</c> is a space, <c>"6"</c> is the digit six); otherwise,
	/// with white space (Unicode's, <c>U+00A0</c> and <c>U+3000</c> among it) trimmed, exactly
	/// one code-point spelling — decimal <c>65</c>, <c>U+0041</c>, <c>0x41</c>, <c>&amp;H41</c>, or an HTML numeric entity <c>&amp;#65;</c> /
	/// <c>&amp;#x41;</c> (the closing <c>;</c> required), ASCII case-insensitive on the prefix.
	/// Culture-insensitive — no <see cref="NumFormat"/>.
	/// </summary>
	/// <remarks>
	/// A spelling past <c>U+10FFFF</c> or naming a surrogate is
	/// <see cref="CastFailure.OutOfRange"/> in the core. A real scalar above <c>U+FFFF</c>
	/// (<c>U+1F600</c>, or a verbatim emoji) is one the core accepts but one
	/// <see cref="char"/> cannot hold, so this binding reports it as
	/// <see cref="CastFailure.OutOfRange"/> too, spanning the white-space-trimmed input.
	/// </remarks>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	public static unsafe Verdict<char> Char(ReadOnlySpan<byte> utf8)
	{
		uint value = 0;
		var fault = RawFaultDetail.Empty;
		var code = Detailed(DoorCodes.Char, 0, null, null, utf8, &value, &fault);
		if (code != 0)
			return Failed<char>(code, fault);
		return value <= char.MaxValue ? (char)value : new Verdict<char>(Beyond(utf8));
	}

	/// <inheritdoc cref="Char(ReadOnlySpan{byte})"/>
	/// <remarks>
	/// One UTF-16 code unit is returned as it is without crossing into the native core, a
	/// lone surrogate included: the verbatim rule is "exactly one character", and a
	/// <see cref="char"/> is what this overload was handed. Anything longer is transcoded to
	/// UTF-8 like every other <see cref="string"/> door, a lone surrogate inside longer text
	/// as the invalid sequence it is (never U+FFFD), so it is malformed there.
	/// </remarks>
	/// <param name="input">The raw scalar text.</param>
	public static Verdict<char> Char(ReadOnlySpan<char> input)
	{
		if (input.Length == 1)
			return input[0];
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Char(utf8), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	// The fault for a scalar the core accepted but a char cannot hold: out of range, too large for
	// the target type, over the
	// token the core read, i.e. the input less the white space it trims (the Unicode set,
	// whole code points only). A verbatim scalar is never white space, so the trimmed span
	// is the whole input there too.
	static Fault Beyond(ReadOnlySpan<byte> utf8)
	{
		var start = 0;
		var end = utf8.Length;
		for (int width; start < end && (width = WhiteSpace.LeadingLength(utf8[start..end])) != 0;)
			start += width;
		for (int width; end > start && (width = WhiteSpace.TrailingLength(utf8[start..end])) != 0;)
			end -= width;
		return new Fault(CastFailure.OutOfRange, start, end - start) { DetailCode = (uint)FaultDetail.TooLarge };
	}
}
