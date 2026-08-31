using System.Globalization;
using BenchmarkDotNet.Attributes;

namespace HyperCast.Benchmarks;

/// <summary>
/// Each pair pits a HyperCast door (FFI crossing included) against the closest BCL parse
/// with equivalent settings declared — lenience matched where the BCL can match it
/// (<see cref="NumberStyles.AllowThousands"/> for grouped integers), the BCL's own culture
/// machinery where that's simply how the BCL door works
/// (<see cref="DateTimeOffset.TryParse(ReadOnlySpan{char}, IFormatProvider, DateTimeStyles, out DateTimeOffset)"/>).
/// <c>[MemoryDiagnoser]</c> keeps the allocation claim measured, not asserted.
/// </summary>
[MemoryDiagnoser]
public class CastBenchmarks
{
	static readonly NumFormat _invariant = NumFormat.Invariant;
	static readonly CultureInfo _invariantCulture = CultureInfo.InvariantCulture;

	// static readonly, not const — a const string invites the JIT to fold the BCL side of a
	// pair into a constant (observed: sub-nanosecond "parses"), which the FFI side
	// structurally can't match. Fields keep both sides honest.
	static readonly string
		BoolText = "true",
		IntText = "1,234,567",
		DoubleText = "12345.6789",
		GuidText = "01020304-0506-0708-090a-0b0c0d0e0f10",
		TimestampText = "2026-01-02T15:04:05.1234567Z",
		DurationText = "1.06:30:15.5";

	[Benchmark]
	public bool CastBoolean() => Cast.Boolean(BoolText).TryGetValue(out Success<bool> s) && s.Value;

	[Benchmark]
	public bool BclBoolTryParse() => bool.TryParse(BoolText, out var value) && value;

	[Benchmark]
	public int CastInt32() => Cast.Int32(IntText, _invariant).TryGetValue(out Success<int> s) ? s.Value : 0;

	[Benchmark]
	public int BclIntTryParse() =>
		int.TryParse(IntText, NumberStyles.Integer | NumberStyles.AllowThousands, _invariantCulture, out var value)
			? value
			: 0;

	[Benchmark]
	public double CastDouble() => Cast.Double(DoubleText, _invariant).TryGetValue(out Success<double> s) ? s.Value : 0;

	[Benchmark]
	public double BclDoubleTryParse() =>
		double.TryParse(DoubleText, NumberStyles.Float, _invariantCulture, out var value) ? value : 0;

	[Benchmark]
	public Guid CastUuid() => Cast.Uuid(GuidText).TryGetValue(out Success<Guid> s) ? s.Value : default;

	[Benchmark]
	public Guid BclGuidTryParse() => Guid.TryParse(GuidText, out var value) ? value : default;

	[Benchmark]
	public DateTimeOffset CastTimestamp() =>
		Cast.Timestamp(TimestampText).TryGetValue(out Success<DateTimeOffset> s) ? s.Value : default;

	[Benchmark]
	public DateTimeOffset BclDateTimeOffsetTryParse() =>
		DateTimeOffset.TryParse(TimestampText, _invariantCulture,
			DateTimeStyles.AssumeUniversal | DateTimeStyles.AdjustToUniversal, out var value)
			? value
			: default;

	[Benchmark]
	public TimeSpan CastDuration() =>
		Cast.Duration(DurationText).TryGetValue(out Success<TimeSpan> s) ? s.Value : default;

	[Benchmark]
	public TimeSpan BclTimeSpanTryParse() =>
		TimeSpan.TryParse(DurationText, _invariantCulture, out var value) ? value : default;
	// --- the declared-order doors, vs the BCL parse that accepts the same text. This is
	// the messy-feed shape: "1/7/2026 3:04 PM" needs a culture (or an exact format) in the
	// BCL, and gets one here — en-US, matching the declared MonthDayYear order.

	const string MessyDateTimeText = "1/7/2026 3:04 PM";
	const string MessyDateText = "1/7/2026";
	const string EuroNumberText = "1.234.567,89";

	static readonly CultureInfo _enUs = CultureInfo.GetCultureInfo("en-US");
	static readonly CultureInfo _deDe = CultureInfo.GetCultureInfo("de-DE");
	static readonly NumFormat _eurozone = new(',', '.', NumStyles.All);

	[Benchmark]
	public DateTime CastDateTimeMessy() =>
		Cast.DateTime(MessyDateTimeText, DateOrder.MonthDayYear).TryGetValue(out Success<DateTime> s)
			? s.Value
			: default;

	[Benchmark]
	public DateTime BclDateTimeTryParseMessy() =>
		System.DateTime.TryParse(MessyDateTimeText, _enUs, DateTimeStyles.None, out var value)
			? value
			: default;

	[Benchmark]
	public DateOnly CastDateOrdered() =>
		Cast.Date(MessyDateText, DateOrder.MonthDayYear).TryGetValue(out Success<DateOnly> s)
			? s.Value
			: default;

	[Benchmark]
	public DateOnly BclDateOnlyTryParse() =>
		DateOnly.TryParse(MessyDateText, _enUs, DateTimeStyles.None, out var value) ? value : default;

	// --- separator detection, vs the same text under a declared format and vs the BCL's
	// own culture machinery.

	[Benchmark]
	public double CastDoubleDetect() =>
		Cast.Double(EuroNumberText, NumFormat.Detect).TryGetValue(out Success<double> s) ? s.Value : default;

	[Benchmark]
	public double CastDoubleDeclared() =>
		Cast.Double(EuroNumberText, _eurozone).TryGetValue(out Success<double> s) ? s.Value : default;

	[Benchmark]
	public double BclDoubleTryParseGerman() =>
		double.TryParse(EuroNumberText, NumberStyles.Float | NumberStyles.AllowThousands, _deDe, out var value)
			? value
			: default;

}
