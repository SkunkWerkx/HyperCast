using System.Globalization;
using System.Numerics;
using System.Reflection;
using System.Text;
using System.Text.Json;

namespace HyperCast.Tests;

/// <summary>
/// Replays the shared conformance corpus (<c>corpus/*.json</c> at the repository root) —
/// the same files the Rust core's own test suite replays — through this binding's UTF-8
/// doors. Fault spans are byte offsets into the UTF-8 input, which is exactly what these
/// doors receive, so span assertions hold verbatim. This is the byte-for-byte polyglot
/// contract: a vector that fails here is a break in the promise, not just a failing test.
/// </summary>
public sealed class CorpusTests
{
	static readonly string _corpusDirectory = FindCorpusDirectory();

	static string FindCorpusDirectory()
	{
		for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
		{
			var corpus = Path.Combine(dir.FullName, "corpus");
			if (Directory.Exists(corpus))
				return corpus;
		}
		throw new DirectoryNotFoundException($"corpus directory not found above {AppContext.BaseDirectory}");
	}

	static JsonElement[] Corpus(string name)
	{
		using var document = JsonDocument.Parse(File.ReadAllText(Path.Combine(_corpusDirectory, name)));
		return [.. document.RootElement.EnumerateArray().Select(vector => vector.Clone())];
	}

	static byte[] InputBytes(JsonElement vector) =>
		Encoding.UTF8.GetBytes(vector.GetProperty("input").GetString()!);

	// Every key a vector's format may carry; one it does not carry declares nothing.
	static NumFormat FormatOf(JsonElement vector)
	{
		if (!vector.TryGetProperty("format", out var format))
			return NumFormat.Invariant;
		string Symbol(string key) => format.TryGetProperty(key, out var symbol) ? symbol.GetString()! : "";
		return new(
			format.GetProperty("decimal_sep").GetString()!,
			format.GetProperty("group_sep").GetString()!,
			(NumStyles)format.GetProperty("flags").GetUInt32(),
			Symbol("currency"))
		{
			CurrencyDecimalSeparator = Symbol("currency_decimal_sep"),
			CurrencyGroupSeparator = Symbol("currency_group_sep"),
			MinusSign = Symbol("minus_sign"),
			PlusSign = Symbol("plus_sign"),
			PercentSymbol = Symbol("percent"),
			DigitZero = format.TryGetProperty("digit_zero", out var zero) ? Rune.GetRuneAt(zero.GetString()!, 0) : null,
		};
	}

	[Fact]
	void Decimal_corpus()
	{
		foreach (var vector in Corpus("decimal.json"))
		{
			var verdict = Cast.Decimal(InputBytes(vector), FormatOf(vector));
			// The raw triple is the contract; decimal's own constructor is exact for it.
			AssertVerdict("decimal", vector, verdict, static v =>
			{
				var magnitude = UInt128.Parse(v.GetProperty("magnitude").GetString()!, CultureInfo.InvariantCulture);
				var lo = (ulong)magnitude;
				return new decimal((int)lo, (int)(lo >> 32), (int)(magnitude >> 64),
					v.GetProperty("negative").GetBoolean(), v.GetProperty("scale").GetByte());
			});
			// The minimal scale is part of the contract too — decimal equality ignores it, so
			// pin it through the canonical text, which the core renders with no trailing zeros.
			if (verdict.TryGetValue(out Success<decimal> success))
				success.Value.ToString(CultureInfo.InvariantCulture).ShouldBe(vector.GetProperty("value").GetString());
		}
	}

	static void AssertVerdict<T>(string domain, JsonElement vector, Verdict<T> verdict, Func<JsonElement, T> expected)
		where T : struct
	{
		var input = vector.GetProperty("input").GetString()!;
		var expect = vector.GetProperty("expect").GetString()!;
		// TryGetValue consumption, not case-type patterns: with an open generic T the union
		// matcher refuses to bind a pattern variable (CS8780) — and this is the hot-loop
		// consumption shape anyway. The compile-checked pattern form is exercised with
		// concrete case types in CastTests.
		if (verdict.TryGetValue(out Success<T> success))
		{
			expect.ShouldBe("ok", $"{domain}: '{input}' unexpectedly parsed");
			Bits(success.Value).ShouldBe(Bits(expected(vector)), $"{domain}: '{input}'");
			return;
		}
		verdict.TryGetValue(out Fault fault).ShouldBeTrue($"{domain}: '{input}' produced no case");
		var reason = expect switch
		{
			"empty" => CastFailure.Empty,
			"malformed" => CastFailure.Malformed,
			"out_of_range" => CastFailure.OutOfRange,
			_ => throw new InvalidOperationException($"{domain}: '{input}' expected '{expect}' but faulted"),
		};
		fault.Reason.ShouldBe(reason, $"{domain}: '{input}'");
		if (vector.TryGetProperty("fault", out var span))
		{
			fault.Offset.ShouldBe(span[0].GetInt32(), $"{domain}: '{input}' fault offset");
			fault.Length.ShouldBe(span[1].GetInt32(), $"{domain}: '{input}' fault length");
		}
	}

	/// <summary>
	/// A value as the corpus compares it: a float or a decimal by its bits, since zero has one
	/// form and the core never delivers a negative one (<c>-0.0 == 0.0</c>, and
	/// <see cref="decimal"/> equality ignores the sign and the scale), anything else as itself.
	/// </summary>
	static object Bits<T>(T value) where T : notnull =>
		value switch
		{
			double d => BitConverter.DoubleToInt64Bits(d),
			float f => BitConverter.SingleToInt32Bits(f),
			decimal m => string.Join(",", decimal.GetBits(m)),
			_ => value,
		};

	[Fact]
	void Boolean_corpus()
	{
		foreach (var vector in Corpus("boolean.json"))
			AssertVerdict("boolean", vector, Cast.Boolean(InputBytes(vector)),
				static v => v.GetProperty("value").GetBoolean());
	}

	/// <summary>
	/// A char vector pins the scalar as its code-point integer. Replayed through both doors:
	/// a scalar above U+FFFF is one the core accepts and a <see cref="char"/> cannot hold, so
	/// this binding expects <see cref="CastFailure.OutOfRange"/> over the trimmed input there;
	/// the UTF-16 door's spans are the vector's byte spans re-expressed in chars.
	/// </summary>
	[Fact]
	void Char_corpus()
	{
		foreach (var vector in Corpus("char.json"))
		{
			var text = vector.GetProperty("input").GetString()!;
			var bytes = Encoding.UTF8.GetBytes(text);
			AssertChar(vector, Cast.Char(bytes), bytes, utf16: false);
			AssertChar(vector, Cast.Char(text), bytes, utf16: true);
		}
	}

	static void AssertChar(JsonElement vector, Verdict<char> verdict, byte[] bytes, bool utf16)
	{
		var label = $"char ({(utf16 ? "utf-16" : "utf-8")}): '{vector.GetProperty("input").GetString()}'";
		int Units(int byteCount) => utf16 ? Encoding.UTF8.GetCharCount(bytes, 0, byteCount) : byteCount;
		void Span(Fault fault, int offset, int length)
		{
			fault.Offset.ShouldBe(Units(offset), $"{label} fault offset");
			fault.Length.ShouldBe(Units(offset + length) - Units(offset), $"{label} fault length");
		}

		var expect = vector.GetProperty("expect").GetString()!;
		if (expect == "ok")
		{
			var scalar = vector.GetProperty("value").GetUInt32();
			if (scalar <= char.MaxValue)
			{
				verdict.TryGetValue(out Success<char> success).ShouldBeTrue($"{label} should parse");
				success.Value.ShouldBe((char)scalar, label);
				return;
			}
			verdict.TryGetValue(out Fault wide).ShouldBeTrue($"{label} is past one char");
			wide.Reason.ShouldBe(CastFailure.OutOfRange, label);
			var trimmed = bytes.AsSpan().TrimStart(" \t\n\f\r"u8);
			var start = bytes.Length - trimmed.Length;
			Span(wide, start, trimmed.TrimEnd(" \t\n\f\r"u8).Length);
			return;
		}
		verdict.TryGetValue(out Fault fault).ShouldBeTrue($"{label} unexpectedly parsed");
		fault.Reason.ShouldBe(expect switch
		{
			"empty" => CastFailure.Empty,
			"malformed" => CastFailure.Malformed,
			"out_of_range" => CastFailure.OutOfRange,
			_ => throw new InvalidOperationException($"{label}: unexpected label '{expect}'"),
		}, label);
		if (vector.TryGetProperty("fault", out var span))
			Span(fault, span[0].GetInt32(), span[1].GetInt32());
	}

	[Fact]
	void Integer_corpus()
	{
		foreach (var vector in Corpus("integer.json"))
		{
			var input = InputBytes(vector);
			var format = FormatOf(vector);
			switch (vector.GetProperty("type").GetString())
			{
				case "i8":
					AssertVerdict("integer", vector, Cast.SByte(input, format), static v => v.GetProperty("value").GetSByte());
					break;
				case "i16":
					AssertVerdict("integer", vector, Cast.Int16(input, format), static v => v.GetProperty("value").GetInt16());
					break;
				case "i32":
					AssertVerdict("integer", vector, Cast.Int32(input, format), static v => v.GetProperty("value").GetInt32());
					break;
				case "i64":
					AssertVerdict("integer", vector, Cast.Int64(input, format), static v => v.GetProperty("value").GetInt64());
					break;
				case "u8":
					AssertVerdict("integer", vector, Cast.Byte(input, format), static v => v.GetProperty("value").GetByte());
					break;
				case "u16":
					AssertVerdict("integer", vector, Cast.UInt16(input, format), static v => v.GetProperty("value").GetUInt16());
					break;
				case "u32":
					AssertVerdict("integer", vector, Cast.UInt32(input, format), static v => v.GetProperty("value").GetUInt32());
					break;
				case "u64":
					AssertVerdict("integer", vector, Cast.UInt64(input, format), static v => v.GetProperty("value").GetUInt64());
					break;
				case var other:
					throw new InvalidOperationException($"integer: unknown type '{other}'");
			}
		}
	}

	[Fact]
	void Real_corpus()
	{
		foreach (var vector in Corpus("real.json"))
		{
			var input = InputBytes(vector);
			var format = FormatOf(vector);
			switch (vector.GetProperty("type").GetString())
			{
				case "f32":
					AssertVerdict("real", vector, Cast.Single(input, format),
						static v => (float)v.GetProperty("value").GetDouble());
					break;
				case "f64":
					AssertVerdict("real", vector, Cast.Double(input, format),
						static v => v.GetProperty("value").GetDouble());
					break;
				case var other:
					throw new InvalidOperationException($"real: unknown type '{other}'");
			}
		}
	}

	[Fact]
	void Uuid_corpus()
	{
		foreach (var vector in Corpus("uuid.json"))
			AssertVerdict("uuid", vector, Cast.Uuid(InputBytes(vector)), static v =>
			{
				var hex = v.GetProperty("value").GetString()!;
				var bytes = Convert.FromHexString(hex);
				return new Guid(bytes, bigEndian: true);
			});
	}

	static DateTimeOffset ExpectedInstant(JsonElement vector) =>
		DateTimeOffset.UnixEpoch
			.AddTicks(vector.GetProperty("seconds").GetInt64() * TimeSpan.TicksPerSecond
				+ vector.GetProperty("nanos").GetInt64() / 100);

	[Fact]
	void Timestamp_corpus()
	{
		foreach (var vector in Corpus("timestamp.json"))
			AssertVerdict("timestamp", vector, Cast.Timestamp(InputBytes(vector)), ExpectedInstant);
	}

	[Fact]
	void Unix_corpus()
	{
		foreach (var vector in Corpus("unix.json"))
		{
			var precision = (UnixPrecision)vector.GetProperty("precision").GetUInt32();
			AssertVerdict("unix", vector, Cast.Unix(InputBytes(vector), precision), ExpectedInstant);
		}
	}

	[Fact]
	void ExcelSerial_corpus()
	{
		foreach (var vector in Corpus("excel_serial.json"))
		{
			var epoch = (ExcelEpoch)vector.GetProperty("epoch").GetUInt32();
			AssertVerdict("excel_serial", vector, Cast.ExcelSerial(InputBytes(vector), epoch), ExpectedInstant);
		}
	}

	[Fact]
	void Date_corpus()
	{
		foreach (var vector in Corpus("date.json"))
			AssertVerdict("date", vector, Cast.Date(InputBytes(vector)), static v => new DateOnly(
				v.GetProperty("year").GetInt32(),
				v.GetProperty("month").GetInt32(),
				v.GetProperty("day").GetInt32()));
	}

	[Fact]
	void Date_order_corpus()
	{
		foreach (var vector in Corpus("date_order.json"))
			AssertVerdict("date_order", vector,
				Cast.Date(InputBytes(vector), (DateOrder)vector.GetProperty("order").GetUInt32()),
				static v => new DateOnly(
					v.GetProperty("year").GetInt32(),
					v.GetProperty("month").GetInt32(),
					v.GetProperty("day").GetInt32()));
	}

	/// <summary>
	/// A datetime vector declares its order, and its day-period designators (<c>am</c> and
	/// <c>pm</c>, together) when it has any. Those go through the <see cref="DateFormat"/>
	/// door; the rest through the bare <see cref="DateOrder"/> one, which a format declaring
	/// nothing must agree with.
	/// </summary>
	[Fact]
	void Datetime_corpus()
	{
		foreach (var vector in Corpus("datetime.json"))
		{
			var input = InputBytes(vector);
			var order = (DateOrder)vector.GetProperty("order").GetUInt32();
			var verdict = vector.TryGetProperty("am", out var am)
				? Cast.DateTime(input, new DateFormat(order, am.GetString()!, vector.GetProperty("pm").GetString()!))
				: Cast.DateTime(input, order);
			if (!vector.TryGetProperty("am", out _))
				Cast.DateTime(input, new DateFormat(order)).ShouldBe(verdict, vector.GetProperty("input").GetString());
			AssertVerdict("datetime", vector, verdict, static v =>
			{
				var nanos = v.GetProperty("nanos_of_day").GetUInt64();
				return new DateTime(
						v.GetProperty("year").GetInt32(),
						v.GetProperty("month").GetInt32(),
						v.GetProperty("day").GetInt32(),
						0, 0, 0, DateTimeKind.Unspecified)
					.AddTicks((long)(nanos / 100));
			});
		}
	}

	[Fact]
	void Time_corpus()
	{
		foreach (var vector in Corpus("time.json"))
			AssertVerdict("time", vector, Cast.Time(InputBytes(vector)),
				static v => new TimeOnly((long)(v.GetProperty("nanos").GetUInt64() / 100)));
	}

	[Fact]
	void Duration_corpus()
	{
		foreach (var vector in Corpus("duration.json"))
			AssertVerdict("duration", vector, Cast.Duration(InputBytes(vector)),
				static v => new TimeSpan(
					v.GetProperty("seconds").GetInt64() * TimeSpan.TicksPerSecond
					+ v.GetProperty("nanos").GetInt64() / 100));
	}

	/// <summary>
	/// The typed doors, by bits: each double is rebuilt exactly from its IEEE 754 pattern —
	/// NaN and the infinities included, which JSON cannot spell. A typed door's fault has no
	/// span.
	/// </summary>
	[Fact]
	void Typed_corpus()
	{
		var doors = new HashSet<string>();
		foreach (var vector in Corpus("typed.json"))
		{
			var value = BitConverter.Int64BitsToDouble(
				(long)ulong.Parse(vector.GetProperty("bits").GetString()!, NumberStyles.HexNumber, CultureInfo.InvariantCulture));
			var door = vector.GetProperty("door").GetString()!;
			doors.Add(door);
			switch (door)
			{
				case "decimal":
					var verdict = Cast.DecimalFromDouble(value);
					AssertTyped(door, vector, verdict, static v =>
					{
						var magnitude = UInt128.Parse(v.GetProperty("magnitude").GetString()!, CultureInfo.InvariantCulture);
						var lo = (ulong)magnitude;
						return new decimal((int)lo, (int)(lo >> 32), (int)(magnitude >> 64),
							v.GetProperty("negative").GetBoolean(), v.GetProperty("scale").GetByte());
					});
					if (verdict.TryGetValue(out Success<decimal> success))
						success.Value.ToString(CultureInfo.InvariantCulture).ShouldBe(vector.GetProperty("value").GetString());
					break;
				case "excel_serial":
					AssertTyped(door, vector,
						Cast.ExcelSerialFromDouble(value, (ExcelEpoch)vector.GetProperty("epoch").GetUInt32()),
						static v => new DateTime(
								v.GetProperty("year").GetInt32(),
								v.GetProperty("month").GetInt32(),
								v.GetProperty("day").GetInt32(),
								0, 0, 0, DateTimeKind.Unspecified)
							.AddTicks((long)(v.GetProperty("nanos_of_day").GetUInt64() / 100)));
					break;
				case "excel_time":
					AssertTyped(door, vector, Cast.ExcelTime(value),
						static v => new TimeOnly((long)(v.GetProperty("nanos").GetUInt64() / 100)));
					break;
				case "excel_duration":
					AssertTyped(door, vector, Cast.ExcelDuration(value),
						static v => new TimeSpan(
							v.GetProperty("seconds").GetInt64() * TimeSpan.TicksPerSecond
							+ v.GetProperty("nanos").GetInt64() / 100));
					break;
				default:
					throw new InvalidOperationException($"typed: unknown door {door}");
			}
		}
		doors.ShouldBe(["decimal", "excel_serial", "excel_time", "excel_duration"], ignoreOrder: true);
	}

	static void AssertTyped<T>(string door, JsonElement vector, Verdict<T> verdict, Func<JsonElement, T> expected)
		where T : struct
	{
		AssertVerdict($"typed {door}", vector, verdict, expected);
		if (verdict.TryGetValue(out Fault fault))
			(fault.Offset, fault.Length).ShouldBe((0, 0), $"typed {door}: '{vector.GetProperty("input").GetString()}' span");
	}

	/// <summary>
	/// The checked doors (<c>corpus/constraints.json</c>), every row: each declaration is built
	/// through the public <see cref="Constraints{T}"/> constructor from the corpus's own text
	/// (so bounds and steps cross this binding's decimal conversion, the mirror of the core's),
	/// and a <c>refused</c> row must throw with its code. Every other row is replayed through
	/// the door's UTF-8 overload and, by the generic <c>Numeric&lt;T&gt;</c>, its UTF-16 one
	/// (spans re-expressed in chars), or through its <c>FromDouble</c> overload; text rows
	/// through all three text overloads (the byte rows, which no string can carry, through the
	/// UTF-8 one alone). Values compare by bits, the value, <c>adjusted</c> and
	/// <c>enum_index</c> on success, and the reason or keyword and the span on every failure.
	/// </summary>
	[Fact]
	void Constraints_corpus()
	{
		var seen = new HashSet<string>();
		foreach (var vector in Corpus("constraints.json"))
		{
			var door = vector.GetProperty("door").GetString()!;
			seen.Add(vector.GetProperty("expect").GetString() == "refused" ? "refused" : door);
			switch (door)
			{
				case "i8": NumericRow<sbyte>(vector, Cast.SByte, Cast.SByteFromDouble, static v => v.GetSByte()); break;
				case "i16": NumericRow<short>(vector, Cast.Int16, Cast.Int16FromDouble, static v => v.GetInt16()); break;
				case "i32": NumericRow<int>(vector, Cast.Int32, Cast.Int32FromDouble, static v => v.GetInt32()); break;
				case "i64": NumericRow<long>(vector, Cast.Int64, Cast.Int64FromDouble, static v => v.GetInt64()); break;
				case "u8": NumericRow<byte>(vector, Cast.Byte, Cast.ByteFromDouble, static v => v.GetByte()); break;
				case "u16": NumericRow<ushort>(vector, Cast.UInt16, Cast.UInt16FromDouble, static v => v.GetUInt16()); break;
				case "u32": NumericRow<uint>(vector, Cast.UInt32, Cast.UInt32FromDouble, static v => v.GetUInt32()); break;
				case "u64": NumericRow<ulong>(vector, Cast.UInt64, Cast.UInt64FromDouble, static v => v.GetUInt64()); break;
				case "f32": NumericRow<float>(vector, Cast.Single, Cast.SingleFromDouble, static v => (float)v.GetDouble()); break;
				case "f64": NumericRow<double>(vector, Cast.Double, Cast.DoubleFromDouble, static v => v.GetDouble()); break;
				case "decimal":
					NumericRow<decimal>(vector, Cast.Decimal, Cast.DecimalFromDouble,
						static v => decimal.Parse(v.GetString()!, NumberStyles.Float, CultureInfo.InvariantCulture));
					break;
				case "text": TextRow(vector); break;
				default: throw new InvalidOperationException($"constraints: unknown door {door}");
			}
		}
		seen.ShouldBe(["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "decimal", "text", "refused"], ignoreOrder: true);
	}

	/// <summary>
	/// The declaration conversion (<c>corpus/declarations.json</c>): each input crosses
	/// <see cref="ConstraintValue"/>'s string path — as the <c>maximum</c> of a
	/// <see cref="Constraints{T}"/> at the decimal door, which holds any decimal — and the raw
	/// triple the core wrote is asserted exactly, scale included, since <see cref="decimal"/>
	/// equality ignores the scale. A <c>refused</c> row throws with code 8. The
	/// <c>input_bytes</c> rows are invalid UTF-8, which no .NET string can carry (a lone
	/// surrogate, the nearest a string comes, is pinned in <see cref="ConstraintsTests"/>), so
	/// they are counted and skipped here.
	/// </summary>
	[Fact]
	void Declarations_corpus()
	{
		var (accepted, refused, bytes) = (0, 0, 0);
		foreach (var vector in Corpus("declarations.json"))
		{
			if (vector.TryGetProperty("input_bytes", out _))
			{
				bytes++;
				continue;
			}
			var input = vector.GetProperty("input").GetString()!;
			if (vector.GetProperty("expect").GetString() == "refused")
			{
				vector.GetProperty("refusal").GetInt32().ShouldBe(8);
				Should.Throw<ArgumentException>(() => new Constraints<decimal>(maximum: input), input)
					.Message.ShouldStartWith(RefusalMessage(8), Case.Sensitive, input);
				refused++;
				continue;
			}
			var raw = new Constraints<decimal>(maximum: input).ToRaw().Upper;
			var magnitude = UInt128.Parse(vector.GetProperty("magnitude").GetString()!, CultureInfo.InvariantCulture);
			(raw.Lo, raw.Hi, raw.Scale, raw.Negative).ShouldBe(
				((ulong)magnitude, (uint)(magnitude >> 64), vector.GetProperty("scale").GetByte(),
					vector.GetProperty("negative").GetBoolean() ? (byte)1 : (byte)0), input);
			raw.ToDecimal().ToString(CultureInfo.InvariantCulture).ShouldBe(vector.GetProperty("value").GetString(), input);
			accepted++;
		}
		(accepted, refused, bytes).ShouldBe((33, 30, 3));
	}

	// The core's Refusal, as Rust displays it: the wording every binding's message carries.
	static readonly string[] _refusals =
	[
		"",
		"a constraint the door does not take",
		"minimum together with exclusiveMinimum",
		"maximum together with exclusiveMaximum",
		"the lower bound is not below the upper bound",
		"multipleOf is not positive",
		"a rounding without multipleOf",
		"minLength is above maxLength",
		"a bound, step or enum entry the door cannot hold",
		"an enum entry that is not UTF-8",
		"a duplicate enum entry",
		"a malformed constraints layout",
		"an enum with no entries",
		"a text enum entry no input can match",
	];

	internal static string RefusalMessage(int code) => $"Constraints refused (code {code}): {_refusals[code]}.";

	// Every keyword a vector names, whether or not its door takes it, through the public
	// constructor: bounds, steps and entries as the corpus spells them.
	static Constraints<T> Declared<T>(JsonElement c) where T : notnull
	{
		ConstraintValue? Text(string key) => c.TryGetProperty(key, out var value) ? (ConstraintValue?)value.GetString()! : null;
		int? Length(string key) => c.TryGetProperty(key, out var value) ? value.GetInt32() : null;
		return new Constraints<T>(
			minimum: Text("minimum"),
			exclusiveMinimum: Text("exclusiveMinimum"),
			maximum: Text("maximum"),
			exclusiveMaximum: Text("exclusiveMaximum"),
			multipleOf: Text("multipleOf"),
			rounding: (c.TryGetProperty("rounding", out var rounding) ? rounding.GetString() : "strict") switch
			{
				"strict" => Rounding.Strict,
				"midpoint" => Rounding.Midpoint,
				"floor" => Rounding.Floor,
				"ceiling" => Rounding.Ceiling,
				var other => throw new InvalidOperationException($"unknown rounding {other}"),
			},
			minLength: Length("minLength"),
			maxLength: Length("maxLength"),
			trim: c.TryGetProperty("trim", out var trim) && trim.GetBoolean(),
			@enum: c.TryGetProperty("enum", out var entries)
				? [.. entries.EnumerateArray().Select(static e => (ConstraintValue)e.GetString()!)]
				: null);
	}

	static string Label(JsonElement vector) =>
		$"constraints {vector.GetProperty("door").GetString()} {vector.GetProperty("constraints").GetRawText()} "
		+ (vector.TryGetProperty("input", out var input) ? $"'{input.GetString()}'"
			: vector.TryGetProperty("from_f64", out var number) ? $"from_f64 {number.GetRawText()}"
			: vector.TryGetProperty("input_bytes", out var bytes) ? $"bytes {bytes.GetRawText()}" : "");

	// A refused row throws, with its code's message; any other row is declared.
	static Constraints<T>? Declare<T>(JsonElement vector) where T : notnull
	{
		var c = vector.GetProperty("constraints");
		if (vector.GetProperty("expect").GetString() != "refused")
			return Declared<T>(c);
		var code = vector.GetProperty("refusal").GetInt32();
		Should.Throw<ArgumentException>(() => Declared<T>(c), Label(vector))
			.Message.ShouldStartWith(RefusalMessage(code), Case.Sensitive, Label(vector));
		return null;
	}

	static double FromF64(JsonElement value) =>
		value.ValueKind == JsonValueKind.String
			? value.GetString() switch
			{
				"-0" => -0.0,
				"NaN" => double.NaN,
				"Infinity" => double.PositiveInfinity,
				"-Infinity" => double.NegativeInfinity,
				var other => throw new InvalidOperationException($"unknown from_f64 {other}"),
			}
			: value.GetDouble();

	static void NumericRow<T>(JsonElement vector,
		Func<ReadOnlySpan<byte>, NumFormat, Constraints<T>, CheckedVerdict<T>> door,
		Func<double, Constraints<T>, CheckedVerdict<T>> fromDouble,
		Func<JsonElement, T> expected) where T : unmanaged, INumber<T>
	{
		if (Declare<T>(vector) is not { } constraints)
			return;
		var label = Label(vector);
		if (vector.TryGetProperty("from_f64", out var number))
		{
			AssertChecked(label, vector, fromDouble(FromF64(number), constraints), static units => units, expected);
			return;
		}
		var text = vector.GetProperty("input").GetString()!;
		var bytes = Encoding.UTF8.GetBytes(text);
		var format = FormatOf(vector);
		AssertChecked(label, vector, door(bytes, format, constraints), static units => units, expected);
		AssertChecked($"{label} (utf-16)", vector, Cast.Numeric(text.AsSpan(), format, constraints),
			units => Encoding.UTF8.GetCharCount(bytes, 0, units), expected);
	}

	static void TextRow(JsonElement vector)
	{
		if (Declare<string>(vector) is not { } constraints)
			return;
		var label = Label(vector);
		static string Value(JsonElement v) => v.GetString()!;
		if (vector.TryGetProperty("input_bytes", out var raw))
		{
			byte[] invalid = [.. raw.EnumerateArray().Select(static b => b.GetByte())];
			AssertChecked(label, vector, Cast.Text(invalid, constraints), static units => units, Value);
			return;
		}
		var text = vector.GetProperty("input").GetString()!;
		var bytes = Encoding.UTF8.GetBytes(text);
		int Chars(int units) => Encoding.UTF8.GetCharCount(bytes, 0, units);
		AssertChecked(label, vector, Cast.Text(bytes, constraints), static units => units, Value);
		AssertChecked($"{label} (string)", vector, Cast.Text(text, constraints), Chars, Value);
		AssertChecked($"{label} (utf-16)", vector, Cast.Text(text.AsSpan(), constraints), Chars, Value);
	}

	// `units` re-expresses a corpus byte offset in the door's own units.
	static void AssertChecked<T>(string label, JsonElement vector, CheckedVerdict<T> verdict,
		Func<int, int> units, Func<JsonElement, T> expected) where T : notnull
	{
		var expect = vector.GetProperty("expect").GetString()!;
		if (verdict.TryGetValue(out Checked<T> ok))
		{
			expect.ShouldBe("ok", $"{label}: unexpectedly passed as {verdict}");
			Bits(ok.Value).ShouldBe(Bits(expected(vector.GetProperty("value"))), $"{label}: value");
			ok.Adjusted.ShouldBe(vector.GetProperty("adjusted").GetBoolean(), $"{label}: adjusted");
			ok.EnumIndex.ShouldBe(vector.TryGetProperty("enum_index", out var index) ? index.GetInt32() : null, $"{label}: enum_index");
			return;
		}
		verdict.TryGetValue(out Rejection rejection).ShouldBeTrue($"{label}: produced no case");
		var span = vector.GetProperty("fault");
		var (offset, length) = (span[0].GetInt32(), span[1].GetInt32());
		(int Offset, int Length) expectedSpan = (units(offset), units(offset + length) - units(offset));
		if (rejection.TryGetValue(out Violation violation))
		{
			expect.ShouldBe("violation", $"{label}: {verdict}");
			violation.Keyword.Name.ShouldBe(vector.GetProperty("keyword").GetString(), $"{label}: keyword");
			(violation.Offset, violation.Length).ShouldBe(expectedSpan, $"{label}: violation span");
			return;
		}
		rejection.TryGetValue(out Fault fault).ShouldBeTrue($"{label}: a rejection of neither case");
		fault.Reason.ShouldBe(expect switch
		{
			"empty" => CastFailure.Empty,
			"malformed" => CastFailure.Malformed,
			"out_of_range" => CastFailure.OutOfRange,
			_ => throw new InvalidOperationException($"{label}: expected '{expect}' but faulted {fault}"),
		}, label);
		(fault.Offset, fault.Length).ShouldBe(expectedSpan, $"{label}: fault span");
	}

	/// <summary>
	/// The plain doors' verdict keeps exactly its shape: validation constraints are additive,
	/// so <see cref="Verdict{T}"/>, <see cref="Success{T}"/>, <see cref="Fault"/> and
	/// <see cref="CastFailure"/> are pinned member for member, and no plain door answers in
	/// anything but a <see cref="Verdict{T}"/>.
	/// </summary>
	[Fact]
	void The_plain_verdict_keeps_its_shape()
	{
		static string[] Surface(Type type) =>
			[.. type.GetMembers(BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly)
				.Select(static m => $"{m.MemberType} {m}").Order(StringComparer.Ordinal)];
		Surface(typeof(Verdict<>)).ShouldBe(_verdictSurface);
		Surface(typeof(Success<>)).ShouldBe(_successSurface);
		Surface(typeof(Fault)).ShouldBe(_faultSurface);
		typeof(Fault).GetConstructors().Single().GetParameters().Select(static p => $"{p.ParameterType.Name} {p.Name}")
			.ShouldBe(["CastFailure Reason", "Int32 Offset", "Int32 Length"]);
		Enum.GetUnderlyingType(typeof(CastFailure)).ShouldBe(typeof(byte));
		Enum.GetValues<CastFailure>().Select(static r => $"{r}={(byte)r}")
			.ShouldBe(["Unspecified=0", "Empty=1", "Malformed=2", "OutOfRange=3"]);
		foreach (var method in typeof(Cast).GetMethods(BindingFlags.Public | BindingFlags.Static))
		{
			var plain = !method.GetParameters().Any(static p => p.ParameterType.IsGenericType
				&& p.ParameterType.GetGenericTypeDefinition() == typeof(Constraints<>));
			var returns = method.ReturnType.IsGenericType ? method.ReturnType.GetGenericTypeDefinition() : method.ReturnType;
			if (returns == typeof(Nullable<>))
				returns = method.ReturnType.GetGenericArguments()[0].GetGenericTypeDefinition();
			if (method.IsSpecialName)
				continue;
			if (method.Name == "Text")
				returns.ShouldBe(typeof(CheckedVerdict<>), method.ToString());
			else if (method.Name == nameof(Cast.Optional))
				returns.ShouldBe(method.GetParameters()[0].ParameterType.GetGenericTypeDefinition(), method.ToString());
			else
				returns.ShouldBe(plain ? typeof(Verdict<>) : typeof(CheckedVerdict<>), method.ToString());
		}
	}

	// Each type's public surface as reflection prints it, recorded at 1.0.0.
	static readonly string[] _verdictSurface =
	[
		"Constructor Void .ctor(HyperCast.Fault)",
		"Constructor Void .ctor(HyperCast.Success`1[T])",
		"Method Boolean Equals(HyperCast.Verdict`1[T])",
		"Method Boolean Equals(System.Object)",
		"Method Boolean TryGetValue(HyperCast.Fault ByRef)",
		"Method Boolean TryGetValue(HyperCast.Success`1[T] ByRef)",
		"Method Boolean get_HasValue()",
		"Method Boolean op_Equality(HyperCast.Verdict`1[T], HyperCast.Verdict`1[T])",
		"Method Boolean op_Inequality(HyperCast.Verdict`1[T], HyperCast.Verdict`1[T])",
		"Method HyperCast.Verdict`1[T] op_Implicit(T)",
		"Method Int32 GetHashCode()",
		"Method System.Object get_Value()",
		"Method System.String ToString()",
		"Method TResult Match[TResult](System.Func`2[HyperCast.Success`1[T],TResult], System.Func`2[HyperCast.Fault,TResult])",
		"Method Void Switch(System.Action`1[HyperCast.Success`1[T]], System.Action`1[HyperCast.Fault])",
		"Property Boolean HasValue",
		"Property System.Object Value",
	];
	static readonly string[] _successSurface =
	[
		"Constructor Void .ctor(T)",
		"Method Boolean Equals(HyperCast.Success`1[T])",
		"Method Boolean Equals(System.Object)",
		"Method Boolean op_Equality(HyperCast.Success`1[T], HyperCast.Success`1[T])",
		"Method Boolean op_Inequality(HyperCast.Success`1[T], HyperCast.Success`1[T])",
		"Method Int32 GetHashCode()",
		"Method System.String ToString()",
		"Method T get_Value()",
		"Method Void Deconstruct(T ByRef)",
		"Method Void set_Value(T)",
		"Property T Value",
	];
	static readonly string[] _faultSurface =
	[
		"Constructor Void .ctor(HyperCast.CastFailure, Int32, Int32)",
		"Method Boolean Equals(HyperCast.Fault)",
		"Method Boolean Equals(System.Object)",
		"Method Boolean op_Equality(HyperCast.Fault, HyperCast.Fault)",
		"Method Boolean op_Inequality(HyperCast.Fault, HyperCast.Fault)",
		"Method HyperCast.CastFailure get_Reason()",
		"Method Int32 GetHashCode()",
		"Method Int32 get_Length()",
		"Method Int32 get_Offset()",
		"Method System.String ToString()",
		"Method Void Deconstruct(HyperCast.CastFailure ByRef, Int32 ByRef, Int32 ByRef)",
		"Method Void set_Length(Int32)",
		"Method Void set_Offset(Int32)",
		"Method Void set_Reason(HyperCast.CastFailure)",
		"Property HyperCast.CastFailure Reason",
		"Property Int32 Length",
		"Property Int32 Offset",
	];
}
