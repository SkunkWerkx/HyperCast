using System.Buffers.Binary;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using HyperCast.Interop;

namespace HyperCast;

/// <summary>
/// A declaration of validation constraints — the OpenAPI Schema Object's keywords — bound to
/// the door its type names: <c>Constraints&lt;double&gt;</c> for <see cref="Cast.Double(ReadOnlySpan{char}, NumFormat, Constraints{double})"/>,
/// <c>Constraints&lt;string&gt;</c> for <see cref="Cast.Text(string, Constraints{string})"/>.
/// Immutable, and judged once, by the native core, when it is built: a declaration that
/// cannot mean anything at its door is refused there as a caller bug, and every checked cast
/// after that only judges data.
/// </summary>
/// <remarks>
/// <para>
/// The integer doors (<see cref="sbyte"/> to <see cref="ulong"/>) take <c>minimum</c>,
/// <c>exclusiveMinimum</c>, <c>maximum</c>, <c>exclusiveMaximum</c>, <c>multipleOf</c> with a
/// <see cref="HyperCast.Rounding"/>, and <c>enum</c>; <see cref="float"/>,
/// <see cref="double"/> and <see cref="decimal"/> the same but <c>enum</c>; the text door
/// (<see cref="string"/>) <c>minLength</c>, <c>maxLength</c>, <c>trim</c> and <c>enum</c>.
/// Bounds and steps are <see cref="ConstraintValue"/>s — an exact <see cref="decimal"/> or its text,
/// never a binary float.
/// </para>
/// <para>
/// The value is judged in the core, in this order, the first failure winning: the door parses
/// (a <see cref="Fault"/>, exactly as the plain door's); text is trimmed of Unicode
/// <c>White_Space</c> when <c>trim</c> says so; <c>multipleOf</c>, on the exact decimal, rounding
/// when declared; the bounds, on the rounded value; the lengths, in code points; <c>enum</c>
/// (text under Unicode simple case folding, delivering the declared entry). A broken rule is a
/// <see cref="Violation"/>.
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

	/// <summary>Declares the constraints and has the native core judge them for <typeparamref name="T"/>'s door.</summary>
	/// <param name="minimum">The smallest value allowed.</param>
	/// <param name="exclusiveMinimum">A value every value must be above.</param>
	/// <param name="maximum">The largest value allowed.</param>
	/// <param name="exclusiveMaximum">A value every value must be below.</param>
	/// <param name="multipleOf">The positive step every value must be a multiple of.</param>
	/// <param name="rounding">What a value that is not a multiple becomes; anything but <see cref="Rounding.Strict"/> needs <paramref name="multipleOf"/>.</param>
	/// <param name="minLength">The fewest code points the text may hold.</param>
	/// <param name="maxLength">The most code points the text may hold.</param>
	/// <param name="trim">Trim leading and trailing Unicode <c>White_Space</c> from the text before judging it.</param>
	/// <param name="enum">
	/// The values allowed, distinct: whole numbers at an integer door, strings at the text door
	/// (matched under simple case folding; a match delivers the entry as declared).
	/// <see langword="null"/> declares no <c>enum</c>; an empty list declares one with no
	/// entries, which is refused.
	/// </param>
	/// <exception cref="ArgumentException">
	/// The core refused the declaration — a contract violation, a caller bug rather than a
	/// data verdict — and the message names the rule it broke: a keyword the door does not
	/// take; <c>minimum</c> with <c>exclusiveMinimum</c>, or <c>maximum</c> with
	/// <c>exclusiveMaximum</c>; a lower bound not below the upper; a <c>multipleOf</c> not
	/// positive; a rounding without <c>multipleOf</c>; <c>minLength</c> above <c>maxLength</c>; a
	/// bound, step or entry the door cannot hold (a fraction or out of range at an integer
	/// door, or text that is no decimal or does not fit one); an entry that is not UTF-8 (a lone
	/// surrogate); duplicate entries; an <paramref name="enum"/> with no entries; a text entry no
	/// input can match (an empty one, or under <paramref name="trim"/> one with leading or
	/// trailing white space). Also a numeric <paramref name="enum"/> entry at the text
	/// door, a <c>default(ConstraintValue)</c>, or a length below zero
	/// (<see cref="ArgumentOutOfRangeException"/>).
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
		IEnumerable<ConstraintValue>? @enum = null)
	{
		Door = DoorOf();
		var raw = new RawConstraints { Size = (uint)Unsafe.SizeOf<RawConstraints>(), Rounding = Code(rounding) };
		if (minimum is { } min)
			Declare(ref raw.Lower, ref raw.Flags, RawConstraints.HasMinimum, min, nameof(minimum));
		if (exclusiveMinimum is { } exclusiveMin)
			Declare(ref raw.Lower, ref raw.Flags, RawConstraints.HasExclusiveMinimum, exclusiveMin, nameof(exclusiveMinimum));
		if (maximum is { } max)
			Declare(ref raw.Upper, ref raw.Flags, RawConstraints.HasMaximum, max, nameof(maximum));
		if (exclusiveMaximum is { } exclusiveMax)
			Declare(ref raw.Upper, ref raw.Flags, RawConstraints.HasExclusiveMaximum, exclusiveMax, nameof(exclusiveMaximum));
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
		if (@enum is not null)
			raw.Flags |= RawConstraints.HasEnum;
		(_packed, _entries) = @enum is null ? ([], null) : Pack(Door, @enum);
		var code = Cast.CheckConstraints(Door, raw, _packed);
		if (code != 0)
			throw new ArgumentException(Refusal(code));
		_raw = raw;
	}

	/// <summary>The native door code this declaration is bound to.</summary>
	internal uint Door { get; }

	/// <summary>The declared <c>minimum</c>, or <see langword="null"/>.</summary>
	public decimal? Minimum => Bound(RawConstraints.HasMinimum, _raw.Lower);

	/// <summary>The declared <c>exclusiveMinimum</c>, or <see langword="null"/>.</summary>
	public decimal? ExclusiveMinimum => Bound(RawConstraints.HasExclusiveMinimum, _raw.Lower);

	/// <summary>The declared <c>maximum</c>, or <see langword="null"/>.</summary>
	public decimal? Maximum => Bound(RawConstraints.HasMaximum, _raw.Upper);

	/// <summary>The declared <c>exclusiveMaximum</c>, or <see langword="null"/>.</summary>
	public decimal? ExclusiveMaximum => Bound(RawConstraints.HasExclusiveMaximum, _raw.Upper);

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

	/// <summary>
	/// The declaration in the native core's 72-byte layout, public for a library that judges
	/// values across a C ABI of its own; the <c>enum</c> list travels beside it, as
	/// <see cref="PackedEnum"/>.
	/// </summary>
	public RawConstraints ToRaw() => _raw;

	/// <summary>
	/// The <c>enum</c> list as the native core reads it beside <see cref="ToRaw"/> (whose
	/// <see cref="RawConstraints.HasEnum"/> flag says whether one was declared), empty for
	/// none: at an integer door, 16-byte <see cref="RawDecimal"/>s in native byte order; at the
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
				if (!list[i].IsText)
					throw new ArgumentException($"Text enum entries are strings; entry {i} is the number {list[i]}.", "enum");
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

	// One message per refusal code, worded as the Rust core's Refusal displays it.
	static string Refusal(int code) =>
		$"Constraints refused (code {code}): " + code switch
		{
			1 => "a constraint the door does not take",
			2 => "minimum together with exclusiveMinimum",
			3 => "maximum together with exclusiveMaximum",
			4 => "the lower bound is not below the upper bound",
			5 => "multipleOf is not positive",
			6 => "a rounding without multipleOf",
			7 => "minLength is above maxLength",
			8 => "a bound, step or enum entry the door cannot hold",
			9 => "an enum entry that is not UTF-8",
			10 => "a duplicate enum entry",
			11 => "a malformed constraints layout",
			12 => "an enum with no entries",
			13 => "a text enum entry no input can match",
			_ => "a rule this binding does not name",
		} + ".";
}
