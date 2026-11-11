using System.Text;

namespace HyperCast.Tests;

/// <summary>
/// The checked doors through the public surface: the README's three declarations, the
/// conversions a declaration crosses before the core judges it, the text door's UTF-16
/// handling, and the two unions' consumption paths. The corpus replay is
/// <see cref="CorpusTests"/>'s.
/// </summary>
public sealed class ConstraintsTests
{
	static readonly Constraints<double> _latitude =
		new(minimum: -90, maximum: 90, multipleOf: 0.000001m, rounding: Rounding.Midpoint);

	static readonly Constraints<decimal> _euroCash = new(multipleOf: "0.05");

	static readonly string[] _sexes = ["Female", "Male", "Other"];

	static readonly Constraints<string> _sex = new(trim: true, @enum: [.. _sexes.Select(static s => (ConstraintValue)s)]);

	static string Describe<T>(CheckedVerdict<T> verdict) where T : notnull =>
		verdict switch
		{
			Checked<T> ok => $"ok {ok.Value}{(ok.Adjusted ? " adjusted" : "")}{(ok.EnumIndex is { } i ? $" #{i}" : "")}",
			Rejection<T> rejection => rejection switch
			{
				Fault fault => $"fault {fault.Reason} @ {fault.Offset}+{fault.Length}",
				Violation<T> violation => $"violation {violation.Keyword} {violation.Value} @ {violation.Offset}+{violation.Length}",
			},
		};

	[Fact]
	void The_readme_declarations_judge_as_documented()
	{
		Describe(Cast.Double("45.1234567", NumFormat.Invariant, _latitude)).ShouldBe("ok 45.123457 adjusted");
		Describe(Cast.Double("91", NumFormat.Invariant, _latitude)).ShouldBe("violation maximum 91 @ 0+2");
		Describe(Cast.DoubleFromDouble(45.1234567, _latitude)).ShouldBe("ok 45.123457 adjusted");
		Describe(Cast.Decimal("€12.35", new NumFormat('.', ',', NumStyles.All, "€"), _euroCash)).ShouldBe("ok 12.35");
		Describe(Cast.Decimal("12.37", NumFormat.Invariant, _euroCash)).ShouldBe("violation multipleOf 12.37 @ 0+5");
		Describe(Cast.Text("  MALE ", _sex)).ShouldBe("ok Male #1");
		Describe(Cast.Text("Mail", _sex)).ShouldBe("violation enum Mail @ 0+4");
		Describe(Cast.Text("   ", _sex)).ShouldBe("fault Empty @ 0+0");
		Describe(Cast.Int32("x", NumFormat.Invariant, new Constraints<int>(minimum: 0))).ShouldBe("fault Malformed @ 0+1");
	}

	[Fact]
	void A_text_match_is_the_callers_own_string()
	{
		Cast.Text("other", _sex).TryGetValue(out Checked<string> ok).ShouldBeTrue();
		ReferenceEquals(ok.Value, _sexes[2]).ShouldBeTrue();
		ok.EnumIndex.ShouldBe(2);
		Cast.Text("OTHER"u8, _sex).TryGetValue(out ok).ShouldBeTrue();
		ReferenceEquals(ok.Value, _sexes[2]).ShouldBeTrue();
		// With nothing trimmed and no enum, the string door hands back the input itself.
		var input = "untouched";
		Cast.Text(input, new Constraints<string>(maxLength: 9)).TryGetValue(out ok).ShouldBeTrue();
		ReferenceEquals(ok.Value, input).ShouldBeTrue();
		Cast.Text(input).TryGetValue(out ok).ShouldBeTrue();
		ReferenceEquals(ok.Value, input).ShouldBeTrue();
		Describe(Cast.Text(" é ", new Constraints<string>(trim: true))).ShouldBe("ok é");
	}

	[Fact]
	void The_text_door_counts_code_points_and_spans_chars()
	{
		var thumbs = new Constraints<string>(maxLength: 1);
		// Two code points, four chars, eight bytes.
		Describe(Cast.Text("👍🏽", thumbs)).ShouldBe("violation maxLength 👍🏽 @ 0+4");
		Describe(Cast.Text("👍🏽"u8, thumbs)).ShouldBe("violation maxLength 👍🏽 @ 0+8");
		Describe(Cast.Text(" 👍🏽x ".AsSpan(), new Constraints<string>(trim: true, maxLength: 2))).ShouldBe("violation maxLength 👍🏽x @ 1+5");
	}

	[Fact]
	void A_violation_carries_the_value_as_cast_never_rounded()
	{
		// Level 1 casts, level 2 validates: the value broke the rule, so the violation holds it,
		// in the door's type, before any rounding.
		static Violation<T> Broken<T>(CheckedVerdict<T> verdict) where T : notnull =>
			verdict.TryGetValue(out Rejection<T> rejection) && rejection.TryGetValue(out Violation<T> violation)
				? violation
				: throw new InvalidOperationException($"no violation: {verdict}");
		var f = NumFormat.Invariant;
		// A rounding past what the type holds breaks multipleOf, carrying the parsed value.
		Broken(Cast.SByte("127", f, new Constraints<sbyte>(multipleOf: 5, rounding: Rounding.Ceiling)))
			.ShouldBe(new Violation<sbyte>(Keyword.MultipleOf, 127, 0, 3));
		// A bound broken after rounding: the value is the one cast, not the rounded one.
		Broken(Cast.Int32("19", f, new Constraints<int>(multipleOf: 10, rounding: Rounding.Midpoint, maximum: 15)))
			.ShouldBe(new Violation<int>(Keyword.Maximum, 19, 0, 2));
		Broken(Cast.Decimal("$12.37", new NumFormat('.', ',', NumStyles.All, "$"), _euroCash)).Value.ShouldBe(12.37m);
		Broken(Cast.Decimal("$12.37".AsSpan(), new NumFormat('.', ',', NumStyles.All, "$"), _euroCash))
			.ShouldBe(new Violation<decimal>(Keyword.MultipleOf, 12.37m, 0, 6));
		Broken(Cast.DoubleFromDouble(91.5, _latitude)).ShouldBe(new Violation<double>(Keyword.Maximum, 91.5, 0, 0));
		// The text door's value is the judged text, after trim, the input's own, never an entry.
		var units = new Constraints<string>(trim: true, caseSensitive: true, @enum: ["mm", "Mm"]);
		Broken(Cast.Text(" MM ", units)).ShouldBe(new Violation<string>(Keyword.Enum, "MM", 1, 2));
		Broken(Cast.Text(" MM ".AsSpan(), units)).ShouldBe(new Violation<string>(Keyword.Enum, "MM", 1, 2));
		Broken(Cast.Text("\u00A0MM"u8, units)).ShouldBe(new Violation<string>(Keyword.Enum, "MM", 2, 2));
		// Untrimmed, the string door's violation holds the input itself.
		var input = "Mail";
		ReferenceEquals(Broken(Cast.Text(input, _sex)).Value, input).ShouldBeTrue();
		// caseSensitive: only the exact entry matches, and it is the one delivered.
		Describe(Cast.Text(" Mm ", units)).ShouldBe("ok Mm #1");
		Describe(Cast.Text("mm", units)).ShouldBe("ok mm #0");
		Describe(Cast.Text("MM", new Constraints<string>(@enum: ["mm", "Mm"]))).ShouldBe("ok mm #0");
		// An enum at the float and decimal doors compares exactly to the value.
		Describe(Cast.Double("2.50", f, new Constraints<double>(@enum: [1, "2.5"]))).ShouldBe("ok 2.5");
		Describe(Cast.Double("2", f, new Constraints<double>(@enum: [1, "2.5"]))).ShouldBe("violation enum 2 @ 0+1");
		Describe(Cast.Decimal("0.10", f, new Constraints<decimal>(@enum: [0.1m, 0.25m]))).ShouldBe("ok 0.1");
		// minimum and exclusiveMinimum apply independently, the first broken in keyword order reported.
		var both = new Constraints<int>(minimum: 5, exclusiveMinimum: 7);
		Describe(Cast.Int32("4", f, both)).ShouldBe("violation minimum 4 @ 0+1");
		Describe(Cast.Int32("6", f, both)).ShouldBe("violation exclusiveMinimum 6 @ 0+1");
		Describe(Cast.Int32("8", f, both)).ShouldBe("ok 8");
	}

	[Fact]
	void A_lone_surrogate_is_never_replaced()
	{
		// As an enum entry it is not UTF-8, which the core refuses (9) rather than matching U+FFFD.
		Should.Throw<ArgumentException>(() => new Constraints<string>(@enum: ["ok", "\uD800"]))
			.Message.ShouldStartWith(CorpusTests.RefusalMessage(9));
		Should.Throw<ArgumentException>(() => new Constraints<string>(@enum: ["\uDC00x"]))
			.Message.ShouldStartWith(CorpusTests.RefusalMessage(9));
		// As input it is malformed, spanning the one char, through every UTF-16 text door.
		Describe(Cast.Text("a\uD800b")).ShouldBe("fault Malformed @ 1+1");
		Describe(Cast.Text("ab\uDBFF".AsSpan())).ShouldBe("fault Malformed @ 2+1");
		Describe(Cast.Text("é\uDC00", new Constraints<string>(maxLength: 9))).ShouldBe("fault Malformed @ 1+1");
		// A pair is one code point.
		Describe(Cast.Text("👍", new Constraints<string>(maxLength: 1))).ShouldBe("ok 👍");
	}

	[Fact]
	void A_declaration_is_judged_once_when_built()
	{
		static void Refused(int code, Func<object> declare) =>
			Should.Throw<ArgumentException>(declare).Message.ShouldStartWith(CorpusTests.RefusalMessage(code));
		// What the core cannot read, and only that, is refused.
		Refused(1, () => new Constraints<int>(trim: true));
		Refused(1, () => new Constraints<double>(caseSensitive: true));
		Refused(1, () => new Constraints<string>(multipleOf: 2, rounding: Rounding.Floor));
		Refused(5, () => new Constraints<float>(multipleOf: 0));
		Refused(6, () => new Constraints<decimal>(rounding: Rounding.Ceiling));
		Refused(8, () => new Constraints<decimal>(multipleOf: "1e-29"));
		Refused(8, () => new Constraints<int>(multipleOf: 0.5m, rounding: Rounding.Midpoint));
		Refused(11, () => new Constraints<int>(multipleOf: 2, rounding: (Rounding)7));
		// Every other valid OpenAPI 3.2 declaration is accepted, an unsatisfiable one included: a
		// keyword for the other kind of door (ignored), all four bounds, crossed or equal bounds,
		// minLength above maxLength, bounds and entries the type cannot hold, duplicate, empty
		// and blank entries, and an empty enum, which no value matches.
		new Constraints<string>(minimum: 0, exclusiveMaximum: 5).Minimum.ShouldBe(0m);
		new Constraints<int>(minLength: 3).MinLength.ShouldBe(3);
		new Constraints<long>(minimum: 0, exclusiveMinimum: 0, maximum: 0, exclusiveMaximum: 0).ExclusiveMaximum.ShouldBe(0m);
		new Constraints<double>(minimum: "5", maximum: 4m).Maximum.ShouldBe(4m);
		new Constraints<string>(minLength: 3, maxLength: 2).MaxLength.ShouldBe(2);
		new Constraints<byte>(minimum: -1, maximum: 1.5m).Minimum.ShouldBe(-1m);
		new Constraints<ulong>(maximum: "18446744073709551616").Maximum.ShouldBe(18446744073709551616m);
		new Constraints<int>(multipleOf: 0.5m).MultipleOf.ShouldBe(0.5m);
		new Constraints<int>(@enum: [1.5m, 1, 1, 1e12m]).PackedEnum.Length.ShouldBe(64);
		new Constraints<string>(@enum: ["Male", "MALE", "", " Male "], trim: true).ToRaw().Flags
			.ShouldBe(Interop.RawConstraints.HasEnum | Interop.RawConstraints.Trim);
		new Constraints<double>(@enum: [1, "2.5"]).PackedEnum.Length.ShouldBe(32);
		Describe(Cast.Int32("1", NumFormat.Invariant, new Constraints<int>(@enum: []))).ShouldBe("violation enum 1 @ 0+1");
		Describe(Cast.Text("a", new Constraints<string>(@enum: []))).ShouldBe("violation enum a @ 0+1");
		new Constraints<int>().ToRaw().Flags.ShouldBe(0u);
		// The type extremes are held exactly.
		new Constraints<long>(minimum: long.MinValue).Minimum.ShouldBe(long.MinValue);
		new Constraints<ulong>(maximum: ulong.MaxValue, @enum: [ulong.MaxValue, 0]).Maximum.ShouldBe(ulong.MaxValue);
	}

	[Fact]
	void Text_declares_a_decimal_exactly_or_not_at_all()
	{
		// The raw triple the core wrote — magnitude, scale and sign — not the decimal, whose
		// equality ignores the scale (1.50m == 1.5m).
		static (UInt128 Magnitude, byte Scale, bool Negative) Step(ConstraintValue step)
		{
			var raw = new Constraints<decimal>(multipleOf: step).ToRaw().MultipleOf;
			return ((UInt128)raw.Hi << 64 | raw.Lo, raw.Scale, raw.Negative != 0);
		}
		Step("0.000001").ShouldBe(((UInt128)1, (byte)6, false));
		Step("1e-6").ShouldBe(((UInt128)1, (byte)6, false));
		Step("+1.50").ShouldBe(((UInt128)15, (byte)1, false));
		Step("1.").ShouldBe(((UInt128)1, (byte)0, false));
		Step(".5").ShouldBe(((UInt128)5, (byte)1, false));
		Step("79228162514264337593543950335").ShouldBe(((UInt128.One << 96) - 1, (byte)0, false));
		Step("0.0000000000000000000000000001").ShouldBe(((UInt128)1, (byte)28, false));
		Step("1000000000000000000000000000000e-2").ShouldBe((UInt128.Parse("10000000000000000000000000000"), (byte)0, false));
		// A decimal is taken as it is, its own scale kept; only the sign of zero is dropped.
		Step(1.50m).ShouldBe(((UInt128)150, (byte)2, false));
		var zero = new Constraints<int>(minimum: "-0").ToRaw().Lower;
		(zero.Lo, zero.Hi, zero.Scale, zero.Negative).ShouldBe((0ul, 0u, (byte)0, (byte)0));
		new Constraints<double>(minimum: -0.0m).ToRaw().Lower.Negative.ShouldBe((byte)0);
		var negative = new Constraints<double>(minimum: "-2.50").ToRaw().Lower;
		(negative.Lo, negative.Scale, negative.Negative).ShouldBe((25ul, (byte)1, (byte)1));
		// decimal.Parse would round each of these to something it holds; the core refuses. A lone
		// surrogate reaches it as invalid UTF-8, never as U+FFFD.
		foreach (var refused in new[] { "1e-29", "0.00000000000000000000000000001", "79228162514264337593543950336",
			"3.5e38", "1e", "1e+", "", ".", "-", "1.2.3", "1,5", " 1", "1 ", "0x10", "١", "NaN", "Infinity", "1\uD800" })
			Should.Throw<ArgumentException>(() => Step(refused), refused).Message.ShouldStartWith(CorpusTests.RefusalMessage(8));
	}

	/// <summary>
	/// The numeric checked doors allocate nothing per call, as the README says: warmed, then
	/// measured on this thread across every path — UTF-8 and UTF-16 text, a format with a
	/// currency, an integer enum, a violation, a from-double call and the decimal door.
	/// </summary>
	[Fact]
	void The_numeric_checked_doors_allocate_nothing_per_call()
	{
		var integers = new Constraints<int>(minimum: 0, @enum: [1, 2, 3]);
		var euro = new NumFormat(',', '.', NumStyles.All, "€");
		void Round()
		{
			_ = Cast.Double("45.1234567", NumFormat.Invariant, _latitude);
			_ = Cast.Double("91"u8, NumFormat.Invariant, _latitude);
			_ = Cast.Int32("7", NumFormat.Invariant, integers);
			_ = Cast.Int32("2"u8, NumFormat.Invariant, integers);
			_ = Cast.Numeric("x", NumFormat.Invariant, integers);
			_ = Cast.DoubleFromDouble(45.1234567, _latitude);
			_ = Cast.Int32FromDouble(-1, integers);
			_ = Cast.Decimal("€12,37", euro, _euroCash);
			_ = Cast.DecimalFromDouble(12.35, _euroCash);
		}
		for (var i = 0; i < 100; i++)
			Round();
		var before = GC.GetAllocatedBytesForCurrentThread();
		for (var i = 0; i < 1000; i++)
			Round();
		(GC.GetAllocatedBytesForCurrentThread() - before).ShouldBe(0);
	}

	// JSON Schema's `enum` is exact equality across types: a number is a valid text entry that no
	// string equals, so `"1"` never matches it, and the entries after it keep their indexes.
	[Fact]
	void A_number_in_a_text_enum_never_matches()
	{
		var codes = new Constraints<string>(@enum: [1, "One", 2.5m, "Two"]);
		Describe(Cast.Text("1", codes)).ShouldBe("violation enum 1 @ 0+1");
		Describe(Cast.Text("2.5", codes)).ShouldBe("violation enum 2.5 @ 0+3");
		Describe(Cast.Text("one", codes)).ShouldBe("ok One #1");
		Describe(Cast.Text("TWO", codes)).ShouldBe("ok Two #3");
		Describe(Cast.Text("x", new Constraints<string>(@enum: [7]))).ShouldBe("violation enum x @ 0+1");
	}

	[Fact]
	void A_declaration_refuses_what_its_type_cannot_say()
	{
		Should.Throw<ArgumentNullException>(() => new Constraints<int>(minimum: (string)null!)).ParamName.ShouldBe("minimum");
		Should.Throw<ArgumentException>(() => new Constraints<int>(maximum: default(ConstraintValue))).ParamName.ShouldBe("maximum");
		Should.Throw<ArgumentNullException>(() => new Constraints<string>(@enum: [(string)null!])).ParamName.ShouldBe("enum");
		Should.Throw<ArgumentOutOfRangeException>(() => new Constraints<string>(minLength: -1)).ParamName.ShouldBe("minLength");
		Should.Throw<NotSupportedException>(() => new Constraints<Guid>());
	}

	[Fact]
	void Every_door_has_its_checked_overloads()
	{
		var f = NumFormat.Invariant;
		static string Ok<T>(CheckedVerdict<T> verdict) where T : notnull => Describe(verdict);
		Ok(Cast.SByte("-5", f, new Constraints<sbyte>(maximum: 0))).ShouldBe("ok -5");
		Ok(Cast.Int16("10", f, new Constraints<short>(multipleOf: 5))).ShouldBe("ok 10");
		Ok(Cast.Int32("19", f, new Constraints<int>(multipleOf: 10, rounding: Rounding.Midpoint))).ShouldBe("ok 20 adjusted");
		Ok(Cast.Int64("7", f, new Constraints<long>(@enum: [7, 9]))).ShouldBe("ok 7");
		Ok(Cast.Byte("9", f, new Constraints<byte>(@enum: [7]))).ShouldBe("violation enum 9 @ 0+1");
		Ok(Cast.UInt16("12", f, new Constraints<ushort>(multipleOf: 5, rounding: Rounding.Floor))).ShouldBe("ok 10 adjusted");
		Ok(Cast.UInt32("12", f, new Constraints<uint>(multipleOf: 5, rounding: Rounding.Ceiling))).ShouldBe("ok 15 adjusted");
		Ok(Cast.UInt64("1", f, new Constraints<ulong>(exclusiveMinimum: 1))).ShouldBe("violation exclusiveMinimum 1 @ 0+1");
		Ok(Cast.Single("0.25", f, new Constraints<float>(exclusiveMaximum: 0.25m))).ShouldBe("violation exclusiveMaximum 0.25 @ 0+4");
		Ok(Cast.Double("١٢٫٥", f with { DigitZero = new Rune('٠'), DecimalSeparator = "٫" }, new Constraints<double>(minimum: 13)))
			.ShouldBe("violation minimum 12.5 @ 0+4");
		Ok(Cast.Numeric("2.5", f, new Constraints<decimal>(multipleOf: 1, rounding: Rounding.Midpoint))).ShouldBe("ok 2 adjusted");
		Ok(Cast.Numeric("3"u8, f, new Constraints<short>(minimum: 4))).ShouldBe("violation minimum 3 @ 0+1");
		Ok(Cast.SByteFromDouble(127, new Constraints<sbyte>(multipleOf: 5, rounding: Rounding.Ceiling))).ShouldBe("violation multipleOf 127 @ 0+0");
		Ok(Cast.Int16FromDouble(-0.0, new Constraints<short>(minimum: 0))).ShouldBe("ok 0");
		Ok(Cast.Int32FromDouble(12.5, new Constraints<int>(maximum: 20))).ShouldBe("fault Malformed @ 0+0");
		Ok(Cast.Int64FromDouble(1e15, new Constraints<long>(maximum: 1))).ShouldBe("violation maximum 1000000000000000 @ 0+0");
		Ok(Cast.ByteFromDouble(255, new Constraints<byte>())).ShouldBe("ok 255");
		Ok(Cast.UInt16FromDouble(-1, new Constraints<ushort>())).ShouldBe("fault OutOfRange @ 0+0");
		Ok(Cast.UInt32FromDouble(double.NaN, new Constraints<uint>())).ShouldBe("fault Malformed @ 0+0");
		Ok(Cast.UInt64FromDouble(4, new Constraints<ulong>(@enum: [4]))).ShouldBe("ok 4");
		Ok(Cast.SingleFromDouble(0.1, new Constraints<float>(multipleOf: 0.1m))).ShouldBe("ok 0.1");
		Ok(Cast.DoubleFromDouble(0.1 + 0.2, new Constraints<double>(multipleOf: 0.1m))).ShouldBe("violation multipleOf 0.30000000000000004 @ 0+0");
		Ok(Cast.DecimalFromDouble(0.1 + 0.2, new Constraints<decimal>(multipleOf: 0.1m, rounding: Rounding.Midpoint))).ShouldBe("ok 0.3 adjusted");
	}

	[Fact]
	void Zero_has_one_form_on_every_checked_path()
	{
		static long Bits(CheckedVerdict<double> verdict) =>
			verdict.TryGetValue(out Checked<double> ok) ? BitConverter.DoubleToInt64Bits(ok.Value) : -1;
		var any = new Constraints<double>();
		Bits(Cast.DoubleFromDouble(-0.0, any)).ShouldBe(0);
		Bits(Cast.Double("-0", NumFormat.Invariant, any)).ShouldBe(0);
		Bits(Cast.DoubleFromDouble(-0.0000004, new Constraints<double>(multipleOf: 0.000001m, rounding: Rounding.Midpoint))).ShouldBe(0);
		Cast.DecimalFromDouble(-0.0000004, new Constraints<decimal>(multipleOf: 0.000001m, rounding: Rounding.Midpoint))
			.TryGetValue(out Checked<decimal> zero).ShouldBeTrue();
		decimal.IsNegative(zero.Value).ShouldBeFalse();
		zero.Adjusted.ShouldBeTrue();
	}

	[Fact]
	void The_plain_text_door_validates_and_spans()
	{
		Describe(Cast.Text("")).ShouldBe("fault Empty @ 0+0");
		Describe(Cast.Text(" a ")).ShouldBe("ok  a ");
		Describe(Cast.Text([0x61, 0x62, 0xFF, 0x63])).ShouldBe("fault Malformed @ 2+1");
		Describe(Cast.Text("ça"u8)).ShouldBe("ok ça");
		Should.Throw<ArgumentNullException>(() => Cast.Text((string)null!));
		Should.Throw<ArgumentNullException>(() => Cast.Text("x", null!));
		Should.Throw<ArgumentNullException>(() => Cast.Int32("1", NumFormat.Invariant, null!));
	}

	[Fact]
	void Both_unions_have_the_pre_CSharp_15_paths()
	{
		var passed = Cast.Int32("15", NumFormat.Invariant, new Constraints<int>(multipleOf: 5));
		var broke = Cast.Int32("16", NumFormat.Invariant, new Constraints<int>(multipleOf: 5));
		passed.Match(ok => ok.Value, _ => -1).ShouldBe(15);
		broke.Match(_ => "", r => r.Match(f => $"{f.Reason}", v => $"{v.Keyword.Name} {v.Value}")).ShouldBe("multipleOf 16");
		int successes = 0, faults = 0, violations = 0;
		broke.Switch(_ => successes++, r => r.Switch(_ => faults++, _ => violations++));
		(successes, faults, violations).ShouldBe((0, 0, 1));
		broke.ToString().ShouldBe("Violation(multipleOf, 16 @ 0+2)");
		passed.ToString().ShouldBe("Checked(15)");
		Cast.Text("male", _sex).ToString().ShouldBe("Checked(Male, enum 1)");
		Should.Throw<InvalidOperationException>(() => default(CheckedVerdict<int>).Match(_ => 0, _ => 0));
		Should.Throw<InvalidOperationException>(() => default(Rejection<int>).Switch(_ => { }, _ => { }));
		Should.Throw<ArgumentOutOfRangeException>(() => new Rejection<int>(default(Fault)));
		Should.Throw<ArgumentOutOfRangeException>(() => new Rejection<int>(default(Violation<int>)));
		Should.Throw<ArgumentOutOfRangeException>(() => new CheckedVerdict<int>(default(Rejection<int>)));
	}

	[Fact]
	void Keywords_are_open_and_spelled_the_OpenAPI_way()
	{
		Keyword[] all = [Keyword.Minimum, Keyword.ExclusiveMinimum, Keyword.Maximum, Keyword.ExclusiveMaximum,
			Keyword.MultipleOf, Keyword.MinLength, Keyword.MaxLength, Keyword.Enum];
		all.Select(static k => k.Code).ShouldBe([1u, 2, 3, 4, 5, 6, 7, 8]);
		all.Select(static k => k.Name).ShouldBe(["minimum", "exclusiveMinimum", "maximum", "exclusiveMaximum",
			"multipleOf", "minLength", "maxLength", "enum"]);
		Keyword.FromCode(5).ShouldBe(Keyword.MultipleOf);
		Keyword.FromCode(0).ShouldBeNull();
		// A keyword this binding does not name yet keeps its code, and a branch over keywords
		// needs its default arm for it.
		var later = Keyword.FromCode(9)!.Value;
		later.Code.ShouldBe(9u);
		later.Name.ShouldBe("keyword 9");
		var arm = later.Name switch
		{
			"minimum" or "maximum" => "bound",
			_ => "default",
		};
		arm.ShouldBe("default");
	}
}
