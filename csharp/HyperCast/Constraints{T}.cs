using System.Buffers.Binary;
using System.Collections.ObjectModel;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using HyperCast.Interop;

namespace HyperCast;

/// <summary>
/// A declaration of validation constraints — the OpenAPI Schema Object's keywords — bound to
/// the door its type names: <c>Constraints&lt;double&gt;</c> for <see cref="Cast.Double(ReadOnlySpan{char}, NumFormat, Constraints{double})"/>,
/// <c>Constraints&lt;string&gt;</c> for <see cref="Cast.Text(string, Constraints{string})"/>.
/// Immutable, and judged once, by the native core, when it is built. Every valid OpenAPI 3.2
/// (JSON Schema 2020-12) declaration is accepted, an unsatisfiable one included (crossed
/// bounds, an empty <c>enum</c>); only one the core cannot read is refused, as a caller bug,
/// and every checked cast after that only judges data.
/// </summary>
/// <remarks>
/// <para>
/// The numeric doors (<see cref="sbyte"/> to <see cref="ulong"/>, <see cref="float"/>,
/// <see cref="double"/> and <see cref="decimal"/>) read <c>minimum</c>,
/// <c>exclusiveMinimum</c>, <c>maximum</c>, <c>exclusiveMaximum</c> (each its own keyword, so
/// <c>minimum</c> and <c>exclusiveMinimum</c> may both apply), <c>multipleOf</c> with a
/// <see cref="HyperCast.Rounding"/>, and <c>enum</c>; the text door (<see cref="string"/>)
/// <c>minLength</c>, <c>maxLength</c> and <c>enum</c>, with the text-door extensions
/// <c>trim</c> and <c>caseSensitive</c>. A keyword for the other kind of door is ignored, as
/// JSON Schema ignores it. Bounds, steps and numeric entries are <see cref="ConstraintValue"/>s
/// — an exact <see cref="decimal"/> or its text, never a binary float.
/// </para>
/// <para>
/// A value is judged in two levels. First it is cast: input that is not the type is a
/// <see cref="Fault"/>, exactly as the plain door's. Then the value is validated, in the core,
/// in this order, the first failure winning: text is trimmed of Unicode <c>White_Space</c> when
/// <c>trim</c> says so; <c>multipleOf</c>, on the exact decimal, rounding when declared (a
/// rounding past what the type holds breaks <c>multipleOf</c>); <c>minimum</c>,
/// <c>exclusiveMinimum</c>, <c>maximum</c>, <c>exclusiveMaximum</c>, on the rounded value; the
/// lengths, in code points; <c>enum</c> (numbers compared exactly; text under Unicode simple
/// case folding unless <c>caseSensitive</c>, delivering the declared entry). A broken rule is a
/// <see cref="Violation{T}"/> carrying the value as cast.
/// </para>
/// </remarks>
/// <typeparam name="T">
/// The door: <see cref="sbyte"/>, <see cref="short"/>, <see cref="int"/>, <see cref="long"/>,
/// <see cref="byte"/>, <see cref="ushort"/>, <see cref="uint"/>, <see cref="ulong"/>,
/// <see cref="float"/>, <see cref="double"/>, <see cref="decimal"/> or <see cref="string"/>.
/// </typeparam>
public sealed class Constraints<T> where T : notnull
{
	readonly RawConstraints _raw;
	readonly byte[] _packed;
	readonly string[]? _entries;
	readonly ReadOnlyCollection<ConstraintValue>? _enum;

	/// <summary>Declares the constraints and has the native core judge them for <typeparamref name="T"/>'s door.</summary>
	/// <param name="minimum">The smallest value allowed.</param>
	/// <param name="exclusiveMinimum">A value every value must be above.</param>
	/// <param name="maximum">The largest value allowed.</param>
	/// <param name="exclusiveMaximum">A value every value must be below.</param>
	/// <param name="multipleOf">The positive step every value must be a multiple of.</param>
	/// <param name="rounding">What a value that is not a multiple becomes; anything but <see cref="Rounding.Strict"/> needs <paramref name="multipleOf"/>.</param>
	/// <param name="minLength">The fewest code points the text may hold.</param>
	/// <param name="maxLength">The most code points the text may hold.</param>
	/// <param name="trim">Trim leading and trailing Unicode <c>White_Space</c> from the text before judging it. The text door only.</param>
	/// <param name="caseSensitive">
	/// Match a text <paramref name="enum"/> byte for byte instead of under simple case folding,
	/// for a vocabulary whose case carries meaning (<c>mm</c> and <c>Mm</c>). The text door only.
	/// </param>
	/// <param name="enum">
	/// The values allowed: numbers at a numeric door, compared exactly to the value (an entry
	/// the type cannot hold never matches); strings at the text door, matched under simple case
	/// folding unless <paramref name="caseSensitive"/> (a match delivers the entry as declared).
	/// With duplicates, the first match wins. <see langword="null"/> declares no <c>enum</c>; an
	/// empty list declares one no value matches.
	/// </param>
	/// <exception cref="ArgumentException">
	/// The core refused the declaration — a contract violation, a caller bug rather than a
	/// data verdict — and the message names the rule it broke: <paramref name="trim"/> or
	/// <paramref name="caseSensitive"/> at a numeric door, or a rounding with
	/// <c>multipleOf</c> at the text door; a <c>multipleOf</c> not positive; a rounding without
	/// <c>multipleOf</c>; a bound, step or entry the core cannot hold (text that is no decimal,
	/// or a decimal past 96 bits or a scale of 28), or a rounding at an integer door with a
	/// fractional <c>multipleOf</c>; an entry that is not UTF-8 (a lone surrogate). Also a
	/// numeric <paramref name="enum"/> entry at the text door, a <c>default(ConstraintValue)</c>,
	/// or a length below zero (<see cref="ArgumentOutOfRangeException"/>).
	/// </exception>
	/// <exception cref="ArgumentNullException">A bound, step or entry is a null string.</exception>
	/// <exception cref="NotSupportedException"><typeparamref name="T"/> has no checked door.</exception>
	public Constraints(
		ConstraintValue? minimum = null,
		ConstraintValue? exclusiveMinimum = null,
		ConstraintValue? maximum = null,
		ConstraintValue? exclusiveMaximum = null,
		ConstraintValue? multipleOf = null,
		Rounding rounding = Rounding.Strict,
		int? minLength = null,
		int? maxLength = null,
		bool trim = false,
		bool caseSensitive = false,
		IEnumerable<ConstraintValue>? @enum = null)
	{
		Door = DoorOf();
		var raw = new RawConstraints { Size = (uint)Unsafe.SizeOf<RawConstraints>(), Rounding = Code(rounding) };
		if (minimum is { } min)
			Declare(ref raw.Minimum, ref raw.Flags, RawConstraints.HasMinimum, min, nameof(minimum));
		if (exclusiveMinimum is { } exclusiveMin)
			Declare(ref raw.ExclusiveMinimum, ref raw.Flags, RawConstraints.HasExclusiveMinimum, exclusiveMin, nameof(exclusiveMinimum));
		if (maximum is { } max)
			Declare(ref raw.Maximum, ref raw.Flags, RawConstraints.HasMaximum, max, nameof(maximum));
		if (exclusiveMaximum is { } exclusiveMax)
			Declare(ref raw.ExclusiveMaximum, ref raw.Flags, RawConstraints.HasExclusiveMaximum, exclusiveMax, nameof(exclusiveMaximum));
		if (multipleOf is { } step)
			Declare(ref raw.MultipleOf, ref raw.Flags, RawConstraints.HasMultipleOf, step, nameof(multipleOf));
		if (minLength is { } fewest)
		{
			ArgumentOutOfRangeException.ThrowIfNegative(fewest, nameof(minLength));
			raw.Flags |= RawConstraints.HasMinLength;
			raw.MinLength = (uint)fewest;
		}
		if (maxLength is { } most)
		{
			ArgumentOutOfRangeException.ThrowIfNegative(most, nameof(maxLength));
			raw.Flags |= RawConstraints.HasMaxLength;
			raw.MaxLength = (uint)most;
		}
		if (trim)
			raw.Flags |= RawConstraints.Trim;
		if (caseSensitive)
			raw.Flags |= RawConstraints.CaseSensitive;
		// Read once, so an enumerable that yields differently each time packs what is kept.
		var declared = @enum?.ToArray();
		if (declared is not null)
			raw.Flags |= RawConstraints.HasEnum;
		(_packed, _entries) = declared is null ? ([], null) : Pack(Door, declared);
		_enum = declared is null ? null : Array.AsReadOnly(declared);
		var code = Cast.CheckConstraints(Door, raw, _packed);
		if (code != 0)
			throw new ArgumentException(Refusal(code));
		_raw = raw;
	}

	/// <summary>The native door code this declaration is bound to.</summary>
	internal uint Door { get; }

	/// <summary>The declared <c>minimum</c>, or <see langword="null"/>.</summary>
	public decimal? Minimum => Bound(RawConstraints.HasMinimum, _raw.Minimum);

	/// <summary>The declared <c>exclusiveMinimum</c>, or <see langword="null"/>.</summary>
	public decimal? ExclusiveMinimum => Bound(RawConstraints.HasExclusiveMinimum, _raw.ExclusiveMinimum);

	/// <summary>The declared <c>maximum</c>, or <see langword="null"/>.</summary>
	public decimal? Maximum => Bound(RawConstraints.HasMaximum, _raw.Maximum);

	/// <summary>The declared <c>exclusiveMaximum</c>, or <see langword="null"/>.</summary>
	public decimal? ExclusiveMaximum => Bound(RawConstraints.HasExclusiveMaximum, _raw.ExclusiveMaximum);

	/// <summary>The declared <c>multipleOf</c>, or <see langword="null"/>.</summary>
	public decimal? MultipleOf => Bound(RawConstraints.HasMultipleOf, _raw.MultipleOf);

	/// <summary>What a value that is not a multiple of <see cref="MultipleOf"/> becomes.</summary>
	public Rounding Rounding => (Rounding)_raw.Rounding;

	/// <summary>The declared <c>minLength</c>, or <see langword="null"/>.</summary>
	public int? MinLength => (_raw.Flags & RawConstraints.HasMinLength) != 0 ? (int)_raw.MinLength : null;

	/// <summary>The declared <c>maxLength</c>, or <see langword="null"/>.</summary>
	public int? MaxLength => (_raw.Flags & RawConstraints.HasMaxLength) != 0 ? (int)_raw.MaxLength : null;

	/// <summary>Whether text is trimmed of Unicode <c>White_Space</c> before it is judged.</summary>
	public bool Trim => (_raw.Flags & RawConstraints.Trim) != 0;

	/// <summary>Whether a text <c>enum</c> matches byte for byte rather than under simple case folding.</summary>
	public bool CaseSensitive => (_raw.Flags & RawConstraints.CaseSensitive) != 0;

	/// <summary>
	/// The declared <c>enum</c>, each entry as it was passed, in order, duplicates kept; empty
	/// for an empty <c>enum</c>, which every value violates, and <see langword="null"/> when none
	/// is declared. A read-only copy: changing the collection passed in changes nothing here.
	/// </summary>
	public IReadOnlyList<ConstraintValue>? Enum => _enum;

	/// <summary>
	/// The declaration in the native core's 104-byte layout, public for a library that judges
	/// values across a C ABI of its own; the <c>enum</c> list travels beside it, as
	/// <see cref="PackedEnum"/>.
	/// </summary>
	public RawConstraints ToRaw() => _raw;

	/// <summary>
	/// The <c>enum</c> list as the native core reads it beside <see cref="ToRaw"/> (whose
	/// <see cref="RawConstraints.HasEnum"/> flag says whether one was declared), empty for
	/// none: at a numeric door, 16-byte <see cref="RawDecimal"/>s in native byte order; at the
	/// text door, each entry a little-endian 32-bit byte length followed by its UTF-8 bytes.
	/// </summary>
	public ReadOnlySpan<byte> PackedEnum => _packed;

	/// <summary>The declared text entry at <paramref name="index"/>, the caller's own string.</summary>
	internal string Entry(uint index) =>
		_entries is { } entries && index < (uint)entries.Length
			? entries[index]
			: throw new InvalidOperationException($"libhypercast matched enum entry {index}, which was never declared — a binding bug, please report it.");

	decimal? Bound(uint flag, RawDecimal value) => (_raw.Flags & flag) != 0 ? value.ToDecimal() : null;

	static void Declare(ref RawDecimal field, ref uint flags, uint flag, ConstraintValue value, string paramName)
	{
		if (!value.TryDecimal(paramName, out field))
			throw new ArgumentException(Refusal(8), paramName);
		flags |= flag;
	}

	// The rounding's code; a value outside the enum goes over as it is, for the core to
	// refuse as a malformed layout.
	static uint Code(Rounding rounding) =>
		rounding switch
		{
			Rounding.Strict => 0,
			Rounding.Midpoint => 1,
			Rounding.Floor => 2,
			Rounding.Ceiling => 3,
			_ => (uint)rounding,
		};

	static uint DoorOf()
	{
		if (typeof(T) == typeof(sbyte)) return 2;
		if (typeof(T) == typeof(short)) return 3;
		if (typeof(T) == typeof(int)) return 4;
		if (typeof(T) == typeof(long)) return 5;
		if (typeof(T) == typeof(byte)) return 6;
		if (typeof(T) == typeof(ushort)) return 7;
		if (typeof(T) == typeof(uint)) return 8;
		if (typeof(T) == typeof(ulong)) return 9;
		if (typeof(T) == typeof(float)) return 10;
		if (typeof(T) == typeof(double)) return 11;
		if (typeof(T) == typeof(string)) return 18;
		if (typeof(T) == typeof(decimal)) return 19;
		throw new NotSupportedException($"No checked door casts to {typeof(T)}.");
	}

	const uint TextDoor = 18;

	// The enum list packed for the door, and at the text door the declared strings, kept so a
	// match hands back the caller's own instance.
	static (byte[] Packed, string[]? Entries) Pack(uint door, IEnumerable<ConstraintValue> values)
	{
		var list = values.ToArray();
		if (door == TextDoor)
		{
			var entries = new string[list.Length];
			var size = 0;
			for (var i = 0; i < list.Length; i++)
			{
				// A number is a valid entry that no string equals (JSON Schema's `enum` is
				// exact equality across types): packed as an empty entry, which no input
				// matches (empty input is the door's Empty fault), so every other entry
				// keeps its index.
				if (!list[i].IsText)
				{
					entries[i] = string.Empty;
					size = checked(size + 4);
					continue;
				}
				var entry = entries[i] = list[i].Text ?? throw new ArgumentNullException("enum", $"Enum entry {i} is null.");
				size = checked(size + 4 + Cast.Wtf8Length(entry));
			}
			var packed = new byte[size];
			var at = 0;
			foreach (var entry in entries)
			{
				var length = Cast.Wtf8(entry, packed.AsSpan(at + 4));
				BinaryPrimitives.WriteUInt32LittleEndian(packed.AsSpan(at), (uint)length);
				at += 4 + length;
			}
			return (packed, entries);
		}
		var numbers = new byte[checked(list.Length * Unsafe.SizeOf<RawDecimal>())];
		for (var i = 0; i < list.Length; i++)
		{
			if (!list[i].TryDecimal("enum", out var entry))
				throw new ArgumentException(Refusal(8), "enum");
			MemoryMarshal.Write(numbers.AsSpan(i * Unsafe.SizeOf<RawDecimal>()), in entry);
		}
		return (numbers, null);
	}

	// One message per refusal code, worded as the Rust core's Refusal displays it. Codes 2, 3,
	// 4, 7, 10, 12 and 13 are retired: the core never returns them.
	static string Refusal(int code) =>
		$"Constraints refused (code {code}): " + code switch
		{
			1 => "a constraint the door does not take",
			5 => "multipleOf is not positive",
			6 => "a rounding without multipleOf",
			8 => "a bound, step or enum entry the door cannot hold",
			9 => "an enum entry that is not UTF-8",
			11 => "a malformed constraints layout",
			_ => "a rule this binding does not name",
		} + ".";
}
