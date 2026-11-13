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

	internal static JsonElement[] Corpus(string name)
	{
		using var document = JsonDocument.Parse(File.ReadAllText(Path.Combine(_corpusDirectory, name)));
		return [.. document.RootElement.EnumerateArray().Select(vector => vector.Clone())];
	}

	// White space as the corpus defines it: the `white_space` rows of formats.json that are
	// true. Loaded once, so the tests that trim or judge a symbol on their own side hold an
	// oracle of their own, and the binding's restated table is held to the same rows below.
	static readonly HashSet<int> _whiteSpace =
	[
		.. Corpus("formats.json")
			.Where(static v => v.GetProperty("kind").GetString() == "white_space" && v.GetProperty("white_space").GetBoolean())
			.Select(static v => CodePoint(v)),
	];

	internal static bool IsWhiteSpace(int codePoint) => _whiteSpace.Contains(codePoint);

	static int CodePoint(JsonElement vector) =>
		int.Parse(vector.GetProperty("code_point").GetString()!.AsSpan(2), NumberStyles.HexNumber, CultureInfo.InvariantCulture);

	static byte[] InputBytes(JsonElement vector) =>
		Encoding.UTF8.GetBytes(vector.GetProperty("input").GetString()!);

	// Every key a vector's format may carry; one it does not carry declares nothing. `flags` is the
	// core's word: the styles in bits 0 to 7 and a policy in bits 8 and 9. A row's `grouping` names the
	// policy outright; without one, the word's own policy bits are the value the declaration judges,
	// so 256 and 768 are refused there and the bits past 9 are carried as they are.
	static NumFormat FormatOf(JsonElement vector)
	{
		if (!vector.TryGetProperty("format", out var format))
			return NumFormat.Invariant;
		string Symbol(string key) => format.TryGetProperty(key, out var symbol) ? symbol.GetString()! : "";
		var flags = format.GetProperty("flags").GetUInt32();
		var grouping = format.TryGetProperty("grouping", out var named)
			? named.GetString() switch
			{
				"western" => NumGrouping.Western,
				"indian" => NumGrouping.Indian,
				var other => throw new InvalidOperationException($"unknown grouping '{other}'"),
			}
			: (NumGrouping)(flags & (3u << 8));
		return new(
			format.GetProperty("decimal_sep").GetString()!,
			format.GetProperty("group_sep").GetString()!,
			(NumStyles)(flags & ~(3u << 8)),
			Symbol("currency"))
		{
			Grouping = grouping,
			CurrencyDecimalSeparator = Symbol("currency_decimal_sep"),
			CurrencyGroupSeparator = Symbol("currency_group_sep"),
			MinusSign = Symbol("minus_sign"),
			PlusSign = Symbol("plus_sign"),
			PercentSymbol = Symbol("percent"),
			DigitZero = format.TryGetProperty("digit_zero", out var zero) ? Rune.GetRuneAt(zero.GetString()!, 0) : null,
		};
	}

	// A row's `options`: shape names, a pivot and a vocabulary, as the core's own replayers read them.
	static DateShapes ShapesOf(JsonElement flags) =>
		flags.EnumerateArray().Aggregate(DateShapes.None, static (shapes, flag) => shapes | flag.GetString() switch
		{
			"compact_date" => DateShapes.CompactDate,
			"space_separator" => DateShapes.SpaceSeparator,
			"hour_offset" => DateShapes.HourOffset,
			"common_forms" => DateShapes.CommonForms,
			var other => throw new InvalidOperationException($"unknown date option '{other}'"),
		});

	static MonthNames MonthsOf(JsonElement months) =>
		new([.. months.EnumerateArray().Select(static e => (e[0].GetInt32(), e[1].GetString()!))]);

	static DateOptions? OptionsOf(JsonElement vector) =>
		vector.TryGetProperty("options", out var options)
			? new DateOptions(ShapesOf(options.GetProperty("flags")), options.TryGetProperty("pivot", out var pivot) ? pivot.GetInt32() : 0)
			{
				Months = options.TryGetProperty("months", out var months) ? MonthsOf(months) : null,
			}
			: null;

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

	// The published vocabulary, restated from the spec: a code is its place in the list, from 1.
	static readonly string[] _details =
	[
		"blank", "unexpected_character", "digit_expected", "delimiter_expected", "trailing_text", "no_digits",
		"sign_misplaced", "parentheses_unbalanced", "mark_repeated", "group_misplaced", "group_size", "group_in_fraction",
		"ambiguous_separator", "fraction_not_allowed", "field_width", "wrong_length", "component_missing", "not_recognized",
		"unknown_unit", "invalid_utf8", "too_long", "not_finite", "not_whole", "negative_not_allowed", "too_large", "too_small",
		"too_precise", "date_field_out_of_range", "time_field_out_of_range", "outside_window", "nonexistent_date", "not_a_scalar",
	];

	static string Pascal(string snake) => string.Concat(snake.Split('_').Select(static w => char.ToUpperInvariant(w[0]) + w[1..]));

	// A failing row names its detail: asserted as the named value and as the raw code, so the enum
	// and the number are each held to the corpus.
	internal static void AssertDetail(Fault fault, JsonElement vector, string label)
	{
		vector.TryGetProperty("detail", out var named).ShouldBeTrue($"{label}: a failing row names no detail");
		var name = named.GetString()!;
		var code = Array.IndexOf(_details, name) + 1;
		code.ShouldBeGreaterThan(0, $"{label}: '{name}' is not in the vocabulary");
		fault.DetailCode.ShouldBe((uint)code, $"{label}: detail code");
		fault.Detail.ShouldBe(Enum.Parse<FaultDetail>(Pascal(name)), $"{label}: detail");
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
		AssertDetail(fault, vector, $"{domain}: '{input}'");
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

	// Each rule of formats.json as the message this binding's refusal carries, so a row is
	// held to the rule that wins, not just to a refusal.
	static readonly Dictionary<string, string> _ruleMessages = new()
	{
		["length"] = "UTF-8 bytes.",
		["digit"] = "must not contain a digit",
		["equal_separators"] = "must differ",
		["white_space"] = "must not contain white space",
		["designator_edge"] = "must not start or end with white space",
		["currency_edge"] = "must not start or end with white space",
		["grouping_policy"] = "not a defined grouping policy",
		["unknown_flag"] = "undefined flag bits",
	};

	// The message each rule of a month vocabulary's refusal carries, held to the same one-rule standard.
	static readonly Dictionary<string, string> _monthRuleMessages = new()
	{
		["layout"] = "cut short",
		["month"] = "is not 1 to 12",
		["utf8"] = "well-formed UTF-8",
		["name_length"] = "bytes long",
		["character"] = "must not contain a digit",
		["month_missing"] = "has no name",
		["duplicate"] = "repeats an earlier name",
	};

	static void AssertMonthRefusal(string label, string rule, Action declare)
	{
		var message = Should.Throw<ArgumentException>(declare, label).Message;
		foreach (var (name, fragment) in _monthRuleMessages)
			if (name == rule)
				message.ShouldContain(fragment, Case.Sensitive, $"{label}: expected the {name} rule, got: {message}");
			else
				message.ShouldNotContain(fragment, Case.Sensitive, $"{label}: expected the {rule} rule, got: {message}");
	}

	// A vocabulary row's bytes as the spec lays them out: each entry a month byte, a little-endian
	// length, the UTF-8 bytes; then the row's own raw bytes, which no builder could say.
	static byte[] PackedRow(JsonElement vector)
	{
		var packed = new List<byte>();
		foreach (var entry in vector.GetProperty("months").EnumerateArray())
		{
			var name = Encoding.UTF8.GetBytes(entry[1].GetString()!);
			packed.Add(entry[0].GetByte());
			packed.AddRange(BitConverter.GetBytes((uint)name.Length));
			packed.AddRange(name);
		}
		if (vector.TryGetProperty("append_bytes", out var raw))
			packed.AddRange(raw.EnumerateArray().Select(static b => b.GetByte()));
		return [.. packed];
	}

	void MonthNamesRow(JsonElement vector)
	{
		var packed = PackedRow(vector);
		var label = $"month_names {vector.GetRawText()}";
		var entries = vector.GetProperty("months").EnumerateArray().Select(static e => (e[0].GetInt32(), e[1].GetString()!)).ToArray();
		var builder = !vector.TryGetProperty("append_bytes", out _);
		if (vector.GetProperty("expect").GetString() == "refused")
		{
			var rule = vector.GetProperty("rule").GetString()!;
			AssertMonthRefusal(label, rule, () => MonthNames.FromPacked(packed));
			if (builder)
				AssertMonthRefusal($"{label} (built)", rule, () => _ = new MonthNames(entries));
			return;
		}
		MonthNames.FromPacked(packed).Packed.ToArray().ShouldBe(packed, label);
		new MonthNames(entries).Packed.ToArray().ShouldBe(packed, $"{label} (built)");
		// A vocabulary the core accepts is one a door takes: the same bytes go over the crossing.
		Cast.Date("2026-01-07"u8, DateOrder.YearMonthDay, new DateOptions { Months = new MonthNames(entries) })
			.ShouldBe(new DateOnly(2026, 1, 7), label);
	}

	static void DateOptionsRow(JsonElement vector)
	{
		var shapes = vector.TryGetProperty("flag_bits", out var bits) ? (DateShapes)bits.GetUInt32() : ShapesOf(vector.GetProperty("flags"));
		var pivot = vector.TryGetProperty("pivot", out var year) ? year.GetInt32() : 0;
		var options = new DateOptions(shapes, pivot);
		var label = $"date_options {vector.GetRawText()}";
		if (vector.GetProperty("expect").GetString() != "refused")
		{
			Should.NotThrow(() => options.TryToRaw(out _), label);
			Should.NotThrow(() => Cast.Date("2026-01-07"u8, DateOrder.YearMonthDay, options), label);
			return;
		}
		// The shapes are refused first, as the core judges them, and as an ArgumentException of its own;
		// a pivot outside its range is the out-of-range kind.
		if (vector.GetProperty("rule").GetString() == "flags")
			Should.Throw<ArgumentException>(() => options.TryToRaw(out _), label).GetType().ShouldBe(typeof(ArgumentException), label);
		else
			Should.Throw<ArgumentOutOfRangeException>(() => options.TryToRaw(out _), label);
		Should.Throw<ArgumentException>(() => Cast.Date("2026-01-07"u8, DateOrder.YearMonthDay, options), label);
		Should.Throw<ArgumentException>(() => Cast.Timestamp("2026-01-07T00:00:00Z"u8, options), label);
	}

	static void AssertRefusal(string label, JsonElement vector, Action declare)
	{
		var rule = vector.GetProperty("rule").GetString()!;
		var message = Should.Throw<ArgumentException>(declare, label).Message;
		foreach (var (name, fragment) in _ruleMessages)
			if (name == rule)
				message.ShouldContain(fragment, Case.Sensitive, $"{label}: expected the {name} rule, got: {message}");
			else if (fragment != _ruleMessages[rule])
				message.ShouldNotContain(fragment, Case.Sensitive, $"{label}: expected the {rule} rule, got: {message}");
	}

	/// <summary>
	/// <c>corpus/formats.json</c>: the declaration rules. A <c>white_space</c> row holds this
	/// binding's restated table to the code point, at each place the binding applies it (a
	/// sign, a decimal separator and a designator's edges through <c>ToRaw</c>; a char door's
	/// span around a scalar a <see cref="char"/> cannot hold). A <c>num_format</c> or
	/// <c>day_periods</c> row is declared through the public types, a refusal being the
	/// <see cref="ArgumentException"/> a door would raise, with the message of the row's rule.
	/// </summary>
	[Fact]
	void Formats_corpus()
	{
		var seen = new HashSet<string>();
		foreach (var vector in Corpus("formats.json"))
		{
			var kind = vector.GetProperty("kind").GetString()!;
			seen.Add(kind);
			switch (kind)
			{
				case "white_space":
					WhiteSpaceRow(vector);
					break;
				case "num_format":
					var label = $"num_format {vector.GetProperty("format").GetRawText()}";
					if (vector.GetProperty("expect").GetString() == "refused")
					{
						AssertRefusal(label, vector, () => FormatOf(vector).ToRaw());
						break;
					}
					var format = FormatOf(vector);
					Should.NotThrow(() => format.ToRaw(), label);
					Should.NotThrow(() => Cast.Int32("1", format), label);
					break;
				case "day_periods":
					var am = vector.TryGetProperty("am", out var a) ? a.GetString()! : "";
					var pm = vector.TryGetProperty("pm", out var p) ? p.GetString()! : "";
					var dateFormat = new DateFormat(DateOrder.YearMonthDay, am, pm);
					var periods = $"day_periods '{am}' '{pm}'";
					if (vector.GetProperty("expect").GetString() == "refused")
					{
						AssertRefusal(periods, vector, () => dateFormat.ToRaw());
						break;
					}
					Should.NotThrow(() => dateFormat.ToRaw(), periods);
					Should.NotThrow(() => Cast.DateTime("2026-01-07 15:04", dateFormat), periods);
					break;
				case "date_options":
					DateOptionsRow(vector);
					break;
				case "month_names":
					MonthNamesRow(vector);
					break;
				default:
					throw new InvalidOperationException($"formats: unknown kind '{kind}'");
			}
		}
		seen.ShouldBe(["white_space", "num_format", "day_periods", "date_options", "month_names"], ignoreOrder: true);
	}

	static void WhiteSpaceRow(JsonElement vector)
	{
		var codePoint = CodePoint(vector);
		var expected = vector.GetProperty("white_space").GetBoolean();
		var text = char.ConvertFromUtf32(codePoint);
		var label = $"U+{codePoint:X4}";
		// The one rule `Encode` applies to a sign: through its single-character path, and
		// through the transcoding one with the code point between two letters.
		bool Refuses(NumFormat format) =>
			Should.Throw<ArgumentException>(() => format.ToRaw(), label).Message.Contains("must not contain white space", StringComparison.Ordinal);
		foreach (var sign in new[] { text, "A" + text + "B" })
		{
			var format = NumFormat.Invariant with { MinusSign = sign };
			if (expected)
				Refuses(format).ShouldBeTrue($"{label} in a sign");
			else
				Should.NotThrow(() => format.ToRaw(), $"{label} in a sign");
		}
		// A currency symbol never starts or ends with one, though it may hold one inside.
		foreach (var currency in new[] { text, text + "F", "F" + text })
		{
			var format = NumFormat.Invariant with { CurrencySymbol = currency };
			if (expected)
				Should.Throw<ArgumentException>(() => format.ToRaw(), label).Message.ShouldContain("must not start or end with white space");
			else
				Should.NotThrow(() => format.ToRaw(), $"{label} at a currency symbol's edge");
		}
		Should.NotThrow(() => (NumFormat.Invariant with { CurrencySymbol = "F" + text + "CFA" }).ToRaw(), $"{label} inside a currency symbol");
		// A decimal separator never holds one, though a group separator may.
		var separator = new NumFormat(text, ",", NumStyles.None);
		var group = new NumFormat(".", text, NumStyles.None);
		if (expected)
		{
			Refuses(separator).ShouldBeTrue($"{label} as a decimal separator");
			Should.NotThrow(() => group.ToRaw(), $"{label} as a group separator");
		}
		else
		{
			Should.NotThrow(() => separator.ToRaw(), $"{label} as a decimal separator");
			Should.NotThrow(() => group.ToRaw(), $"{label} as a group separator");
		}
		// A designator's edges, either end.
		foreach (var designator in new[] { text + "AM", "AM" + text })
		{
			var format = new DateFormat(DateOrder.YearMonthDay, designator, "PM");
			if (expected)
				Should.Throw<ArgumentException>(() => format.ToRaw(), label).Message.ShouldContain("must not start or end with white space");
			else
				Should.NotThrow(() => format.ToRaw(), $"{label} at a designator's edge");
		}
		// The char door's span: the core trims the code point (when it is white space) and the
		// binding excludes it from the span it computes for a spelled scalar a char cannot hold.
		if (!expected)
			return;
		var spelled = text + "U+1F600" + text;
		var width = Encoding.UTF8.GetByteCount(text);
		(Cast.Char(Encoding.UTF8.GetBytes(spelled)) is Fault { Reason: CastFailure.OutOfRange } wide && wide.Offset == width && wide.Length == 7)
			.ShouldBeTrue($"{label} around a supplementary scalar (utf-8)");
		(Cast.Char(spelled) is Fault { Reason: CastFailure.OutOfRange } units && units.Offset == text.Length && units.Length == 7)
			.ShouldBeTrue($"{label} around a supplementary scalar (utf-16)");
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
			wide.Detail.ShouldBe(FaultDetail.TooLarge, label);
			var text = vector.GetProperty("input").GetString()!;
			var (first, last) = (0, text.Length);
			while (first < last && IsWhiteSpace(text[first]))
				first++;
			while (last > first && IsWhiteSpace(text[last - 1]))
				last--;
			Span(wide, Encoding.UTF8.GetByteCount(text.AsSpan(0, first)), Encoding.UTF8.GetByteCount(text.AsSpan(first, last - first)));
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
		AssertDetail(fault, vector, label);
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
		{
			var input = InputBytes(vector);
			var verdict = OptionsOf(vector) is { } options ? Cast.Timestamp(input, options) : Cast.Timestamp(input);
			// A row that asks for nothing reads the same under options that declare nothing.
			if (OptionsOf(vector) is null)
				Cast.Timestamp(input, default(DateOptions)).ShouldBe(verdict, vector.GetProperty("input").GetString());
			AssertVerdict("timestamp", vector, verdict, ExpectedInstant);
		}
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
		{
			var input = InputBytes(vector);
			var verdict = OptionsOf(vector) is { } options ? Cast.Date(input, options) : Cast.Date(input);
			if (OptionsOf(vector) is null)
				Cast.Date(input, default(DateOptions)).ShouldBe(verdict, vector.GetProperty("input").GetString());
			AssertVerdict("date", vector, verdict, static v => new DateOnly(
				v.GetProperty("year").GetInt32(),
				v.GetProperty("month").GetInt32(),
				v.GetProperty("day").GetInt32()));
		}
	}

	[Fact]
	void Date_order_corpus()
	{
		foreach (var vector in Corpus("date_order.json"))
		{
			var input = InputBytes(vector);
			var order = (DateOrder)vector.GetProperty("order").GetUInt32();
			var verdict = OptionsOf(vector) is { } options ? Cast.Date(input, order, options) : Cast.Date(input, order);
			if (OptionsOf(vector) is null)
				Cast.Date(input, order, default).ShouldBe(verdict, vector.GetProperty("input").GetString());
			AssertVerdict("date_order", vector, verdict,
				static v => new DateOnly(
					v.GetProperty("year").GetInt32(),
					v.GetProperty("month").GetInt32(),
					v.GetProperty("day").GetInt32()));
		}
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
			var options = OptionsOf(vector);
			var verdict = (vector.TryGetProperty("am", out var am), options) switch
			{
				(true, { } declared) => Cast.DateTime(input, new DateFormat(order, am.GetString()!, vector.GetProperty("pm").GetString()!), declared),
				(true, null) => Cast.DateTime(input, new DateFormat(order, am.GetString()!, vector.GetProperty("pm").GetString()!)),
				(false, { } declared) => Cast.DateTime(input, order, declared),
				(false, null) => Cast.DateTime(input, order),
			};
			if (!vector.TryGetProperty("am", out _) && options is null)
			{
				Cast.DateTime(input, new DateFormat(order)).ShouldBe(verdict, vector.GetProperty("input").GetString());
				Cast.DateTime(input, order, default).ShouldBe(verdict, vector.GetProperty("input").GetString());
			}
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
	/// UTF-8 one alone). Values compare by bits: the value, <c>adjusted</c> and
	/// <c>enum_index</c> on success; the keyword, the value as cast and the span on a violation;
	/// the reason and the span on a fault.
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
			var raw = new Constraints<decimal>(maximum: input).ToRaw().Maximum;
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
	// Codes 2, 3, 4, 7, 10, 12 and 13 are retired, and never returned.
	static readonly Dictionary<int, string> _refusals = new()
	{
		[1] = "a constraint the door does not take",
		[5] = "multipleOf is not positive",
		[6] = "a rounding without multipleOf",
		[8] = "a bound, step or enum entry the door cannot hold",
		[9] = "an enum entry that is not UTF-8",
		[11] = "a malformed constraints layout",
	};

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
			caseSensitive: c.TryGetProperty("caseSensitive", out var caseSensitive) && caseSensitive.GetBoolean(),
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
		verdict.TryGetValue(out Rejection<T> rejection).ShouldBeTrue($"{label}: produced no case");
		var span = vector.GetProperty("fault");
		var (offset, length) = (span[0].GetInt32(), span[1].GetInt32());
		(int Offset, int Length) expectedSpan = (units(offset), units(offset + length) - units(offset));
		if (rejection.TryGetValue(out Violation<T> violation))
		{
			expect.ShouldBe("violation", $"{label}: {verdict}");
			violation.Keyword.Name.ShouldBe(vector.GetProperty("keyword").GetString(), $"{label}: keyword");
			Bits(violation.Value).ShouldBe(Bits(expected(vector.GetProperty("value"))), $"{label}: violation value");
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
		AssertDetail(fault, vector, label);
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
		// Fault grew a detail beside its identity, and nothing else: every member 1.0 recorded is still
		// there, and the one non-positional property (with its setter) is all that was added.
		_faultSurface.ShouldBeSubsetOf(Surface(typeof(Fault)));
		Surface(typeof(Fault)).Except(_faultSurface).ShouldBe(_faultSurfaceAdded, ignoreOrder: true);
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
	// What 1.1 added to Fault: DetailCode (init-only) and the named Detail read from it.
	static readonly string[] _faultSurfaceAdded =
	[
		"Method HyperCast.FaultDetail get_Detail()",
		"Method UInt32 get_DetailCode()",
		"Method Void set_DetailCode(UInt32)",
		"Property HyperCast.FaultDetail Detail",
		"Property UInt32 DetailCode",
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

	/// <summary>
	/// <c>corpus/grouping.json</c>: every shape under both policies, through every way a format is had, and the
	/// retired policy bit refused at every declaration.
	/// </summary>
	[Fact]
	void Grouping_corpus()
	{
		static void Holds(string label, JsonElement want, string text, NumFormat format)
		{
			var ctx = $"{label} '{text}'";
			if (want.TryGetProperty("fault", out var fault))
			{
				var (offset, length) = (fault[0].GetInt32(), fault[1].GetInt32());
				bool Is(Fault f) => f is { Reason: CastFailure.Malformed, Detail: FaultDetail.GroupSize } && f.Offset == offset && f.Length == length;
				(Cast.Int64(text, format).TryGetValue(out Fault integer) && Is(integer)).ShouldBeTrue(ctx);
				(Cast.Double(text, format).TryGetValue(out Fault real) && Is(real)).ShouldBeTrue(ctx);
				(Cast.Decimal(text, format).TryGetValue(out Fault exact) && Is(exact)).ShouldBeTrue(ctx);
			}
			else
			{
				var value = want.GetProperty("value").GetInt64();
				(Cast.Int64(text, format) is Success<long> s1 && s1.Value == value).ShouldBeTrue(ctx);
				(Cast.Double(text, format) is Success<double> s2 && s2.Value == value).ShouldBeTrue(ctx);
				(Cast.Decimal(text, format) is Success<decimal> s3 && s3.Value == value).ShouldBeTrue(ctx);
			}
		}
		static NumFormat Bridge(int[] number, int[] currency)
		{
			var data = (NumberFormatInfo)CultureInfo.InvariantCulture.NumberFormat.Clone();
			data.NumberGroupSizes = number;
			data.CurrencyGroupSizes = currency;
			return NumFormat.From(data);
		}
		var indian = NumFormat.Invariant with { Grouping = NumGrouping.Indian };
		var saved = CultureInfo.CurrentCulture;
		var shapes = 0;
		foreach (var row in Corpus("grouping.json"))
			switch (row.GetProperty("kind").GetString())
			{
				case "shape":
					shapes++;
					var text = row.GetProperty("input").GetString()!;
					var (west, east) = (row.GetProperty("western"), row.GetProperty("indian"));
					Holds("invariant", west, text, NumFormat.Invariant);
					Holds("constructor", west, text, new NumFormat(".", ",", NumStyles.All));
					Holds("declared", west, text, new NumFormat(".", ",", NumStyles.All, NumGrouping.Western));
					Holds("indian to western", west, text, indian with { Grouping = NumGrouping.Western });
					Holds("bridge threes", west, text, Bridge([3], [3]));
					Holds("bridge fours", west, text, Bridge([4], [4]));
					Holds("bridge indian currency", west, text, Bridge([3], [3, 2]));
					Holds("the default struct's policy", west, text, NumFormat.Invariant with { Grouping = default });
					Holds("declared indian", east, text, new NumFormat('.', ',', NumStyles.All, NumGrouping.Indian));
					Holds("western to indian", east, text, indian);
					Holds("bridge indian", east, text, Bridge([3, 2], [3, 2]));
					Holds("bridge ml", east, text, Bridge([3, 2], [3]));
					// The current culture's door is the bridge, by the same rule.
					try
					{
						CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("en-US");
						Verdict(Cast.Int64(text, (IFormatProvider?)null), west).ShouldBeTrue($"current en-US '{text}'");
						CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("en-IN");
						Verdict(Cast.Int64(text, (IFormatProvider?)null), east).ShouldBeTrue($"current en-IN '{text}'");
					}
					finally
					{
						CultureInfo.CurrentCulture = saved;
					}
					break;
				case "detect":
					Holds("detect", row, row.GetProperty("input").GetString()!, NumFormat.Detect);
					break;
				case "refused_flags":
					var flags = row.GetProperty("flags").GetUInt32();
					var policy = (NumGrouping)(flags & (3u << 8));
					Should.Throw<ArgumentException>(() => new NumFormat(".", ",", (NumStyles)flags), $"flags {flags}");
					Should.Throw<ArgumentException>(() => NumFormat.Invariant with { Styles = (NumStyles)flags }, $"flags {flags}");
					Should.Throw<ArgumentException>(() => NumFormat.Invariant with { Grouping = policy }, $"policy {policy}");
					Should.Throw<ArgumentException>(() => new NumFormat('.', ',', NumStyles.All, policy), $"policy {policy}");
					break;
				case "accepted_flags":
					var word = row.GetProperty("flags").GetUInt32();
					new NumFormat(".", ",", (NumStyles)(word & 0xFF), (NumGrouping)(word & (3u << 8))).ToRaw().Flags.ShouldBe(word);
					break;
				case var other:
					throw new InvalidOperationException($"unknown kind '{other}'");
			}
		shapes.ShouldBeGreaterThanOrEqualTo(18);
	}

	static bool Verdict(Verdict<long> verdict, JsonElement want) =>
		want.TryGetProperty("fault", out var fault)
			? verdict is Fault { Reason: CastFailure.Malformed, Detail: FaultDetail.GroupSize } f && f.Offset == fault[0].GetInt32() && f.Length == fault[1].GetInt32()
			: verdict is Success<long> s && s.Value == want.GetProperty("value").GetInt64();
}
