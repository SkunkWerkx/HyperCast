namespace HyperCast;

/// <summary>
/// The rule a <see cref="Violation{T}"/> broke, spelled the OpenAPI way (<see cref="Name"/> is
/// <c>exclusiveMinimum</c>, not <c>ExclusiveMinimum</c>), with the native core's code.
/// </summary>
/// <remarks>
/// Open to additions: <c>format</c> and <c>pattern</c> may join in a minor release, so a
/// keyword is a value with static members rather than an enum, and code that branches on
/// one needs a default arm — compare with <c>==</c>, or switch on <see cref="Name"/> with a
/// <c>_</c> case. A keyword this binding does not know yet keeps its <see cref="Code"/>.
/// </remarks>
public readonly record struct Keyword
{
	Keyword(uint code) => Code = code;

	/// <summary>The native core's code: 1 <c>minimum</c> … 8 <c>enum</c>.</summary>
	public uint Code { get; }

	/// <summary><c>minimum</c>: the value is below it.</summary>
	public static Keyword Minimum => new(1);

	/// <summary><c>exclusiveMinimum</c>: the value is at or below it.</summary>
	public static Keyword ExclusiveMinimum => new(2);

	/// <summary><c>maximum</c>: the value is above it.</summary>
	public static Keyword Maximum => new(3);

	/// <summary><c>exclusiveMaximum</c>: the value is at or above it.</summary>
	public static Keyword ExclusiveMaximum => new(4);

	/// <summary><c>multipleOf</c>: the value is not a multiple, under <see cref="Rounding.Strict"/>.</summary>
	public static Keyword MultipleOf => new(5);

	/// <summary><c>minLength</c>: the text has fewer code points.</summary>
	public static Keyword MinLength => new(6);

	/// <summary><c>maxLength</c>: the text has more code points.</summary>
	public static Keyword MaxLength => new(7);

	/// <summary><c>enum</c>: the value matches no entry.</summary>
	public static Keyword Enum => new(8);

	/// <summary>
	/// The OpenAPI spelling: <c>minimum</c>, <c>exclusiveMinimum</c>, <c>maximum</c>,
	/// <c>exclusiveMaximum</c>, <c>multipleOf</c>, <c>minLength</c>, <c>maxLength</c>,
	/// <c>enum</c>; <c>keyword N</c> for a code this binding does not name.
	/// </summary>
	public string Name => Code switch
	{
		1 => "minimum",
		2 => "exclusiveMinimum",
		3 => "maximum",
		4 => "exclusiveMaximum",
		5 => "multipleOf",
		6 => "minLength",
		7 => "maxLength",
		8 => "enum",
		_ => $"keyword {Code}",
	};

	/// <summary>The keyword whose native code is <paramref name="code"/>, or <see langword="null"/> for 0, which names none.</summary>
	/// <param name="code">The code a checked door wrote.</param>
	public static Keyword? FromCode(uint code) => code == 0 ? null : new Keyword(code);

	/// <summary>Renders <see cref="Name"/>.</summary>
	public override string ToString() => Name;
}
