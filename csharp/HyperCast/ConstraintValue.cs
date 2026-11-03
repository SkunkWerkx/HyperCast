using HyperCast.Interop;

namespace HyperCast;

/// <summary>
/// A bound, step or <c>enum</c> entry of <see cref="Constraints{T}"/>, as the caller wrote it:
/// an exact <see cref="decimal"/> (an integer literal widens to one), or text. Never a binary
/// float — there is no conversion from <see cref="double"/>, so <c>0.1</c> cannot be
/// declared inexactly by accident; write <c>0.1m</c> or <c>"0.1"</c>.
/// </summary>
/// <remarks>
/// <para>
/// Text at a numeric door is a decimal in invariant notation — an optional sign, digits with
/// an optional <c>.</c> and fraction, an optional exponent (<c>0.000001</c>, <c>-90</c>,
/// <c>1e-6</c>) — converted exactly by the native core's own conversion: a value that does not
/// fit 96 bits at a scale of 28 or less is refused rather than rounded (<c>1e-29</c> is never
/// zero, as <c>decimal.Parse</c> would make it). Text at the text door is the <c>enum</c> entry
/// itself.
/// </para>
/// <para>
/// In a conditional expression, cast the present arm to <c>ConstraintValue?</c>
/// (<c>flag ? (ConstraintValue?)"0.05" : null</c>), so the <see langword="null"/> arm reads as
/// absent rather than converting through <see cref="string"/> into a value holding a null
/// string, which the <see cref="Constraints{T}"/> constructor refuses.
/// </para>
/// </remarks>
public readonly struct ConstraintValue
{
	readonly decimal _number;
	readonly string? _text;
	// 0 default, 1 number, 2 text.
	readonly byte _kind;

	ConstraintValue(decimal number)
	{
		_number = number;
		_kind = 1;
	}

	ConstraintValue(string text)
	{
		_text = text;
		_kind = 2;
	}

	/// <summary>An exact decimal. Integer literals reach this through their implicit widening to <see cref="decimal"/>.</summary>
	/// <param name="number">The value.</param>
	public static implicit operator ConstraintValue(decimal number) => new(number);

	/// <summary>Text: a decimal in invariant notation at a numeric door, an entry at the text door.</summary>
	/// <param name="text">The text.</param>
	public static implicit operator ConstraintValue(string text) => new(text);

	/// <summary>The decimal's invariant text, or the text as written.</summary>
	public override string ToString() =>
		_kind switch
		{
			1 => _number.ToString(System.Globalization.CultureInfo.InvariantCulture),
			2 => _text ?? "",
			_ => "",
		};

	internal bool IsText => _kind == 2;

	internal string? Text => _text;

	/// <summary>
	/// The exact decimal this value declares, or <see langword="false"/> when it declares none:
	/// text the core's conversion refuses, which the caller refuses as the core's code 8.
	/// </summary>
	/// <exception cref="ArgumentNullException">The value was made from a null string.</exception>
	/// <exception cref="ArgumentException">This is <c>default(ConstraintValue)</c>, which declares nothing.</exception>
	internal bool TryDecimal(string paramName, out RawDecimal value)
	{
		switch (_kind)
		{
			case 1:
				value = Raw(_number);
				return true;
			case 2:
				ArgumentNullException.ThrowIfNull(_text, paramName);
				return Cast.DeclaredDecimal(_text, out value);
			default:
				throw new ArgumentException("default(ConstraintValue) declares nothing; write a decimal or its text.", paramName);
		}
	}

	// decimal's own triple, exactly; zero carries no sign.
	static RawDecimal Raw(decimal number)
	{
		Span<int> bits = stackalloc int[4];
		decimal.GetBits(number, bits);
		var lo = (ulong)(uint)bits[0] | (ulong)(uint)bits[1] << 32;
		var hi = (uint)bits[2];
		var negative = bits[3] < 0 && (lo | hi) != 0;
		return new RawDecimal(lo, hi, (byte)(bits[3] >> 16), negative ? (byte)1 : (byte)0);
	}
}
