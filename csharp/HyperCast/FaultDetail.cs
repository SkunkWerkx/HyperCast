namespace HyperCast;

/// <summary>
/// What failed, in the native core's published vocabulary — a <see cref="Fault"/>'s
/// <see cref="Fault.Detail"/>, finer than its <see cref="CastFailure"/> reason. The numbers are
/// the core's codes, never renumbered; <see cref="Fault.DetailCode"/> holds the raw one.
/// </summary>
/// <remarks>
/// <para>
/// New codes may appear in a minor release, so a consumer tolerates <see cref="Unknown"/>: a
/// fault whose code this binding does not name decodes with that member and keeps its number,
/// and its reason and span stay as usable as any other's. A detail says what failed, never how
/// to retry.
/// </para>
/// <para>
/// <see cref="None"/> means no detail was reported: a fault a 1.0-style construction made, or
/// the violation of a checked door (which is not a <see cref="Fault"/> at all).
/// </para>
/// </remarks>
public enum FaultDetail : byte
{
	/// <summary>No detail reported.</summary>
	None = 0,

	/// <summary>Empty, or only white space.</summary>
	Blank = 1,

	/// <summary>A character the position does not allow, where it allows several things (a letter in a numeral, an undeclared symbol).</summary>
	UnexpectedCharacter = 2,

	/// <summary>A numeral or digit field is under way, or a fixed position requires a digit, and another character or the end is found.</summary>
	DigitExpected = 3,

	/// <summary>A required mark (<c>-</c>, <c>:</c>, <c>T</c>, <c>}</c>, <c>0x</c>, <c>;</c>, a UUID hyphen) is missing or different.</summary>
	DelimiterExpected = 4,

	/// <summary>The value is complete, but more text follows.</summary>
	TrailingText = 5,

	/// <summary>The numeral is absent entirely: only a sign, an affix, a bare radix prefix or a bare <c>s</c>.</summary>
	NoDigits = 6,

	/// <summary>A sign where none may stand.</summary>
	SignMisplaced = 7,

	/// <summary>A <c>(</c> without its <c>)</c>, or empty parentheses.</summary>
	ParenthesesUnbalanced = 8,

	/// <summary>A once-only mark appears twice.</summary>
	MarkRepeated = 9,

	/// <summary>A group separator not between digits, or an empty group.</summary>
	GroupMisplaced = 10,

	/// <summary>A group with the wrong digit count for the declared <see cref="NumGrouping"/>.</summary>
	GroupSize = 11,

	/// <summary>Grouping after the decimal separator or in an exponent.</summary>
	GroupInFraction = 12,

	/// <summary>Separator detection cannot tell <c>.</c> from <c>,</c>.</summary>
	AmbiguousSeparator = 13,

	/// <summary>A fraction where the type is whole.</summary>
	FractionNotAllowed = 14,

	/// <summary>A delimited field has the wrong number of digits.</summary>
	FieldWidth = 15,

	/// <summary>A fixed-shape token, recognised by its total length, has the wrong length.</summary>
	WrongLength = 16,

	/// <summary>A required part is absent (a zone, seconds, a duration unit or component).</summary>
	ComponentMissing = 17,

	/// <summary>Not one of the door's accepted words or values (a boolean word, a month name).</summary>
	NotRecognized = 18,

	/// <summary>An unknown or misplaced duration unit.</summary>
	UnknownUnit = 19,

	/// <summary>The bytes at the failure site do not decode as UTF-8.</summary>
	InvalidUtf8 = 20,

	/// <summary>Past an implementation limit (a 256-byte numeral, an 18-digit run).</summary>
	TooLong = 21,

	/// <summary>A non-finite value where it cannot be represented.</summary>
	NotFinite = 22,

	/// <summary>A non-integral double at a whole-number door.</summary>
	NotWhole = 23,

	/// <summary>A negative where the type has no negatives.</summary>
	NegativeNotAllowed = 24,

	/// <summary>The magnitude exceeds what the target type represents; also a scalar above U+FFFF at the <see cref="char"/> door.</summary>
	TooLarge = 25,

	/// <summary>Below a signed type's minimum.</summary>
	TooSmall = 26,

	/// <summary>More fractional digits than the type holds.</summary>
	TooPrecise = 27,

	/// <summary>A year 0, a month 13, a day past the month's end.</summary>
	DateFieldOutOfRange = 28,

	/// <summary>An hour, minute, second or offset out of range.</summary>
	TimeFieldOutOfRange = 29,

	/// <summary>A representable instant or duration past the supported window.</summary>
	OutsideWindow = 30,

	/// <summary>A day that never existed: Excel's phantom 1900-02-29.</summary>
	NonexistentDate = 31,

	/// <summary>A code point above U+10FFFF, or a surrogate.</summary>
	NotAScalar = 32,

	/// <summary>A code this binding does not name — read <see cref="Fault.DetailCode"/>.</summary>
	Unknown = 255,
}
