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

	static NumFormat FormatOf(JsonElement vector)
	{
		if (!vector.TryGetProperty("format", out var format))
			return NumFormat.Invariant;
		return new(
			format.GetProperty("decimal_sep").GetString()![0],
			format.GetProperty("group_sep").GetString()![0],
			(NumStyles)format.GetProperty("flags").GetUInt32());
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
			success.Value.ShouldBe(expected(vector), $"{domain}: '{input}'");
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

	[Fact]
	void Boolean_corpus()
	{
		foreach (var vector in Corpus("boolean.json"))
			AssertVerdict("boolean", vector, Cast.Boolean(InputBytes(vector)),
				static v => v.GetProperty("value").GetBoolean());
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
	void Date_corpus()
	{
		foreach (var vector in Corpus("date.json"))
			AssertVerdict("date", vector, Cast.Date(InputBytes(vector)), static v => new DateOnly(
				v.GetProperty("year").GetInt32(),
				v.GetProperty("month").GetInt32(),
				v.GetProperty("day").GetInt32()));
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
}
