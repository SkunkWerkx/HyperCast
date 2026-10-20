using System.Globalization;
using System.Text;

namespace HyperCast.Tests;

/// <summary>
/// The <see cref="IFormatProvider"/> overloads: each must be exactly its
/// <see cref="NumFormat"/> door under <see cref="NumFormat.From(IFormatProvider)"/>, with
/// <see langword="null"/> read as the current culture the way a BCL <c>TryParse</c> reads it.
/// </summary>
public sealed class ProviderTests
{
	static readonly CultureInfo English = CultureInfo.GetCultureInfo("en-US");
	static readonly CultureInfo German = CultureInfo.GetCultureInfo("de-DE");
	static readonly CultureInfo French = CultureInfo.GetCultureInfo("fr-FR");

	// A format no shipped culture has, so a pass cannot be a coincidence of defaults:
	// every field the bridge fills, spelled oddly, and a trailing-sign negative pattern.
	static readonly NumberFormatInfo Custom = new()
	{
		NumberDecimalSeparator = "|",
		NumberGroupSeparator = "_",
		CurrencySymbol = "¤¤",
		CurrencyDecimalSeparator = ";",
		CurrencyGroupSeparator = ":",
		NegativeSign = "~",
		PositiveSign = "^",
		PercentSymbol = "pct",
		NumberNegativePattern = 3,
	};

	public static TheoryData<string> Providers => ["en-US", "de-DE", "fr-FR", "custom", "invariant"];

	static IFormatProvider Provider(string name) => name switch
	{
		"custom" => Custom,
		"invariant" => CultureInfo.InvariantCulture,
		_ => CultureInfo.GetCultureInfo(name),
	};

	// Text spelled in the provider's own symbols, so the fr-FR rows use whatever group
	// separator this ICU version gives it (U+202F today, U+00A0 before).
	static string Spell(IFormatProvider provider, string invariantText)
	{
		var info = NumberFormatInfo.GetInstance(provider);
		return invariantText
			.Replace(",", "\u0001", StringComparison.Ordinal)
			.Replace(".", info.NumberDecimalSeparator, StringComparison.Ordinal)
			.Replace("\u0001", info.NumberGroupSeparator, StringComparison.Ordinal);
	}

	[Fact]
	void Each_provider_reads_its_own_separators()
	{
		Cast.Double("1,234.5", English).ShouldBe(Cast.Double("1,234.5", NumFormat.Invariant));
		(Cast.Double("1,234.5", English) is Success<double> { Value: 1234.5 }).ShouldBeTrue();
		(Cast.Double("1.234,5", German) is Success<double> { Value: 1234.5 }).ShouldBeTrue();
		(Cast.Int32("(1.234)", German) is Success<int> { Value: -1234 }).ShouldBeTrue();
		(Cast.Double(Spell(French, "1,234.5"), French) is Success<double> { Value: 1234.5 }).ShouldBeTrue();
		(Cast.Decimal("1_234|50", Custom) is Success<decimal> { Value: 1234.5m }).ShouldBeTrue();
		(Cast.Decimal("1.234,50 €", German) is Success<decimal> { Value: 1234.5m }).ShouldBeTrue();
		(Cast.Decimal("¤¤1:234;50", Custom) is Success<decimal> { Value: 1234.5m }).ShouldBeTrue();
		// With the symbol present the custom currency separators are in force, not the number ones.
		(Cast.Decimal("¤¤1_234|50", Custom) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		// The other culture's spelling is not accepted: the provider is a declaration.
		(Cast.Double("1.234,5", English) is Fault).ShouldBeTrue();
	}

	[Fact]
	void Null_is_the_current_culture_like_the_BCL()
	{
		var saved = CultureInfo.CurrentCulture;
		try
		{
			CultureInfo.CurrentCulture = German;
			(Cast.Int32("1.234", null) is Success<int> { Value: 1234 }).ShouldBeTrue();
			(Cast.Double("1,5", null) is Success<double> { Value: 1.5 }).ShouldBeTrue();
			(Cast.Decimal("2,25"u8, null) is Success<decimal> { Value: 2.25m }).ShouldBeTrue();
			(Cast.Numeric<long>("-7.000", null) is Success<long> { Value: -7000 }).ShouldBeTrue();
			(Cast.Scalar<double>("3,5", null) is Success<double> { Value: 3.5 }).ShouldBeTrue();
			double.TryParse("1,5", null, out var bcl).ShouldBeTrue();
			bcl.ShouldBe(1.5);

			CultureInfo.CurrentCulture = English;
			(Cast.Double("1,5", null) is Success<double> { Value: 15 }).ShouldBeTrue();
			(Cast.Double("1.5", null) is Success<double> { Value: 1.5 }).ShouldBeTrue();
		}
		finally
		{
			CultureInfo.CurrentCulture = saved;
		}
	}

	// Every overload against its NumFormat door, success and fault, both input shapes.
	[Theory]
	[MemberData(nameof(Providers))]
	void Every_overload_is_its_NumFormat_door_under_NumFormat_From(string name)
	{
		var provider = Provider(name);
		var format = NumFormat.From(provider);
		foreach (var invariant in new[] { "42", "-1,234", "(5)", "1e3", "0x7F", "1,234.5", "50%", "nope", "", "99999999999999999999999999999999", "~7", "7~", "7 ~", "^7", "50pct", "¤¤1:234;5" })
		{
			var text = Spell(provider, invariant);
			var utf8 = Encoding.UTF8.GetBytes(text);
			Cast.SByte(text, provider).ShouldBe(Cast.SByte(text, format), text);
			Cast.SByte(utf8, provider).ShouldBe(Cast.SByte(utf8, format), text);
			Cast.Int16(text, provider).ShouldBe(Cast.Int16(text, format), text);
			Cast.Int16(utf8, provider).ShouldBe(Cast.Int16(utf8, format), text);
			Cast.Int32(text, provider).ShouldBe(Cast.Int32(text, format), text);
			Cast.Int32(utf8, provider).ShouldBe(Cast.Int32(utf8, format), text);
			Cast.Int64(text, provider).ShouldBe(Cast.Int64(text, format), text);
			Cast.Int64(utf8, provider).ShouldBe(Cast.Int64(utf8, format), text);
			Cast.Byte(text, provider).ShouldBe(Cast.Byte(text, format), text);
			Cast.Byte(utf8, provider).ShouldBe(Cast.Byte(utf8, format), text);
			Cast.UInt16(text, provider).ShouldBe(Cast.UInt16(text, format), text);
			Cast.UInt16(utf8, provider).ShouldBe(Cast.UInt16(utf8, format), text);
			Cast.UInt32(text, provider).ShouldBe(Cast.UInt32(text, format), text);
			Cast.UInt32(utf8, provider).ShouldBe(Cast.UInt32(utf8, format), text);
			Cast.UInt64(text, provider).ShouldBe(Cast.UInt64(text, format), text);
			Cast.UInt64(utf8, provider).ShouldBe(Cast.UInt64(utf8, format), text);
			Cast.Single(text, provider).ShouldBe(Cast.Single(text, format), text);
			Cast.Single(utf8, provider).ShouldBe(Cast.Single(utf8, format), text);
			Cast.Double(text, provider).ShouldBe(Cast.Double(text, format), text);
			Cast.Double(utf8, provider).ShouldBe(Cast.Double(utf8, format), text);
			Cast.Decimal(text, provider).ShouldBe(Cast.Decimal(text, format), text);
			Cast.Decimal(utf8, provider).ShouldBe(Cast.Decimal(utf8, format), text);
			Cast.Numeric<int>(text, provider).ShouldBe(Cast.Numeric<int>(text, format), text);
			Cast.Numeric<int>(utf8, provider).ShouldBe(Cast.Numeric<int>(utf8, format), text);
			Cast.Numeric<decimal>(text, provider).ShouldBe(Cast.Numeric<decimal>(text, format), text);
			Cast.Numeric<decimal>(utf8, provider).ShouldBe(Cast.Numeric<decimal>(utf8, format), text);
			Cast.Scalar<double>(text, provider).ShouldBe(Cast.Scalar<double>(text, format), text);
			Cast.Scalar<double>(utf8, provider).ShouldBe(Cast.Scalar<double>(utf8, format), text);
			Cast.Scalar<ulong>(text, provider).ShouldBe(Cast.Scalar<ulong>(text, format), text);
			Cast.Scalar<ulong>(utf8, provider).ShouldBe(Cast.Scalar<ulong>(utf8, format), text);
		}
	}

	[Theory]
	[MemberData(nameof(Providers))]
	void Scalar_ignores_the_provider_for_non_numeric_targets(string name)
	{
		var provider = Provider(name);
		Cast.Scalar<bool>("yes", provider).ShouldBe(Cast.Boolean("yes"));
		Cast.Scalar<Guid>("01020304-0506-0708-090a-0b0c0d0e0f10", provider)
			.ShouldBe(Cast.Uuid("01020304-0506-0708-090a-0b0c0d0e0f10"));
		Cast.Scalar<DateOnly>("2026-01-02", provider).ShouldBe(Cast.Date("2026-01-02"));
		Cast.Scalar<TimeSpan>("PT1,5S"u8, provider).ShouldBe(Cast.Duration("PT1,5S"u8));
		Cast.Scalar<char>("U+00E9", provider).ShouldBe(Cast.Char("U+00E9"));
		Cast.Scalar<DateTime>("2026-01-02T03:04:05Z", provider)
			.ShouldBe(Cast.Scalar<DateTime>("2026-01-02T03:04:05Z", NumFormat.Invariant));
	}

	[Fact]
	void The_custom_provider_reads_every_field_it_declares()
	{
		var format = NumFormat.From(Custom);
		format.Styles.ShouldBe(NumStyles.All | NumStyles.TrailingSign);
		(format.CurrencyDecimalSeparator, format.CurrencyGroupSeparator, format.MinusSign, format.PlusSign, format.PercentSymbol)
			.ShouldBe((";", ":", "~", "^", "pct"));
		(Cast.Int32("~7", Custom) is Success<int> { Value: -7 }).ShouldBeTrue();
		(Cast.Int32("7~", Custom) is Success<int> { Value: -7 }).ShouldBeTrue();
		(Cast.Int32("7 -", Custom) is Success<int> { Value: -7 }).ShouldBeTrue();
		(Cast.Int32("^7", Custom) is Success<int> { Value: 7 }).ShouldBeTrue();
		(Cast.Double("50pct", Custom) is Success<double> { Value: 0.5 }).ShouldBeTrue();
		(Cast.Decimal("¤¤1:234;5", Custom) is Success<decimal> { Value: 1234.5m }).ShouldBeTrue();
		(Cast.Decimal("1_234|5", Custom) is Success<decimal> { Value: 1234.5m }).ShouldBeTrue();
		(Cast.Decimal("¤¤1_234|5", Custom) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		// The ASCII spellings are read beside the declared ones, never instead of them.
		(Cast.Int32("-7", Custom) is Success<int> { Value: -7 }).ShouldBeTrue();
		(Cast.Double("50%", Custom) is Success<double> { Value: 0.5 }).ShouldBeTrue();
		// Under en-US none of that is declared, and the same text is a fault.
		(Cast.Int32("7~", English) is Fault).ShouldBeTrue();
		(Cast.Int32("7-", English) is Fault).ShouldBeTrue();
	}

	/// <summary>
	/// The civil date-time door's provider overload is its <see cref="DateFormat"/> door
	/// under <see cref="DateFormat.From(IFormatProvider)"/>, <see langword="null"/> read as
	/// the current culture.
	/// </summary>
	[Fact]
	void The_datetime_overload_is_its_DateFormat_door_under_DateFormat_From()
	{
		// ko-KR's designators are 오전/오후 on this machine's ICU and the English pair on
		// some runtimes' data, so the clone pins them: a culture object, not a spelling lookup.
		var korean = (CultureInfo)CultureInfo.GetCultureInfo("ko-KR").Clone();
		korean.DateTimeFormat.AMDesignator = "오전";
		korean.DateTimeFormat.PMDesignator = "오후";
		var custom = new DateTimeFormatInfo { ShortDatePattern = "dd/MM/yyyy", AMDesignator = "vorm.", PMDesignator = "nachm." };
		foreach (var (text, provider) in new (string, IFormatProvider)[]
		{
			("2026-01-07 오후 3:04", korean), ("2026-01-07 3:04 PM", korean), ("2026-01-07 3:04 nachm.", korean),
			("1/7/2026 3:04 nachm.", custom), ("1/7/2026 3:04 PM", custom), ("1/7/2026 nachm. 3:04", custom),
			("1/7/2026 3:04 PM", English), ("1/7/2026 3:04 PM", German), ("1/7/2026 3:04 pm", French), ("1/7/2026 3:04 p.m.", English), ("", German),
		})
		{
			var format = DateFormat.From(provider);
			var utf8 = Encoding.UTF8.GetBytes(text);
			Cast.DateTime(text, provider).ShouldBe(Cast.DateTime(text, format), text);
			Cast.DateTime(utf8, provider).ShouldBe(Cast.DateTime(utf8, format), text);
		}
		(Cast.DateTime("2026-01-07 오후 3:04", korean) is Success<DateTime> { Value: var seoul } && seoul == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue();
		(Cast.DateTime("1/7/2026 3:04 nachm.", custom) is Success<DateTime> { Value: var day } && day == new DateTime(2026, 7, 1, 15, 4, 0)).ShouldBeTrue();
		(Cast.DateTime("1/7/2026 3:04 nachm.", English) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();

		var saved = CultureInfo.CurrentCulture;
		try
		{
			CultureInfo.CurrentCulture = korean;
			(Cast.DateTime("2026-01-07 오후 3:04", null) is Success<DateTime> { Value: var current } && current == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue();
			CultureInfo.CurrentCulture = English;
			(Cast.DateTime("2026-01-07 오후 3:04", null) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
			(Cast.DateTime("1/7/2026", null) is Success<DateTime> { Value: var us } && us == new DateTime(2026, 1, 7)).ShouldBeTrue();
		}
		finally
		{
			CultureInfo.CurrentCulture = saved;
		}
	}

	[Fact]
	void Unsupported_targets_still_throw_before_reading_the_provider()
	{
		Should.Throw<NotSupportedException>(() => Cast.Scalar<Half>("1", German)).Message.ShouldContain("Half");
		Should.Throw<NotSupportedException>(() => Cast.Scalar<Int128>("1"u8, null)).Message.ShouldContain("Int128");
	}

	[Fact]
	void Overload_resolution_picks_the_provider_door_for_null_and_cultures()
	{
		// These compiling unambiguously is half the test: null and a CultureInfo can only
		// convert to IFormatProvider?, and a NumFormat argument still binds the NumFormat door.
		(Cast.Int32("12", null) is Success<int> { Value: 12 }).ShouldBeTrue();
		(Cast.Int32("12", CultureInfo.InvariantCulture) is Success<int> { Value: 12 }).ShouldBeTrue();
		(Cast.Int32("12", NumberFormatInfo.InvariantInfo) is Success<int> { Value: 12 }).ShouldBeTrue();
		(Cast.Int32("12", NumFormat.Invariant) is Success<int> { Value: 12 }).ShouldBeTrue();
		(Cast.Scalar<int>("12", null) is Success<int> { Value: 12 }).ShouldBeTrue();
		(Cast.Numeric<int>("12"u8, null) is Success<int> { Value: 12 }).ShouldBeTrue();
		(Cast.DateTime("2026-01-07", null) is Success<DateTime> { Value: var iso } && iso == new DateTime(2026, 1, 7)).ShouldBeTrue();
		(Cast.DateTime("2026-01-07", CultureInfo.InvariantCulture) is Success<DateTime>).ShouldBeTrue();
		(Cast.DateTime("2026-01-07", DateTimeFormatInfo.InvariantInfo) is Success<DateTime>).ShouldBeTrue();
		(Cast.DateTime("2026-01-07", DateOrder.YearMonthDay) is Success<DateTime>).ShouldBeTrue();
		(Cast.DateTime("2026-01-07", new DateFormat(DateOrder.YearMonthDay)) is Success<DateTime>).ShouldBeTrue();
	}
}
