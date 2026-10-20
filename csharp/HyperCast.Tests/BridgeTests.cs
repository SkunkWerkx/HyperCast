using System.Globalization;
using System.Text;

namespace HyperCast.Tests;

/// <summary>
/// The culture bridges against real cultures' data — <see cref="NumFormat.From(CultureInfo)"/>
/// filling every field the core reads, <see cref="DateFormat.From(CultureInfo)"/> the
/// designators — and the reading each one buys: what a culture's own <c>TryParse</c> reads,
/// this door now reads too, with the ASCII spelling always read beside it. The data is this
/// machine's ICU, so each test pins what it leans on before leaning on it.
/// </summary>
public sealed class BridgeTests
{
	static CultureInfo Culture(string name) => CultureInfo.GetCultureInfo(name);

	[Fact]
	void A_cultures_own_minus_sign_is_read_beside_the_ascii_one()
	{
		// Estonian and Basque declare U+2212; int.TryParse("−5", et-EE) is -5 and so is this.
		var estonian = NumFormat.From(Culture("et-EE"));
		estonian.MinusSign.ShouldBe("−");
		estonian.PlusSign.ShouldBe("");
		(Cast.Int32("−5", estonian) is Success<int> { Value: -5 }).ShouldBeTrue();
		(Cast.Int32("-5", estonian) is Success<int> { Value: -5 }).ShouldBeTrue();
		(Cast.Decimal("−1 234,5", estonian) is Success<decimal> { Value: -1234.5m }).ShouldBeTrue();
		(Cast.Int32("−5 €", estonian) is Success<int> { Value: -5 }).ShouldBeTrue();
		(Cast.Int32("−−5", estonian) is Fault { Reason: CastFailure.Malformed, Offset: 1, Length: 1 }).ShouldBeTrue();
		// Undeclared, the sign is a stray character — the invariant profile is unchanged.
		(Cast.Int32("−5", NumFormat.Invariant) is Fault { Reason: CastFailure.Malformed, Offset: 0, Length: 1 }).ShouldBeTrue();
		// The Arabic-script cultures mark their signs with a bidi control; the mark is part of the sign.
		var arabic = NumFormat.From(Culture("ar-SA"));
		arabic.MinusSign.ShouldBe("؜-");
		arabic.PlusSign.ShouldBe("؜+");
		(Cast.Int32("؜-5", arabic) is Success<int> { Value: -5 }).ShouldBeTrue();
		(Cast.Int32("؜+5", arabic) is Success<int> { Value: 5 }).ShouldBeTrue();
	}

	[Fact]
	void A_currency_amount_is_read_with_the_cultures_currency_separators()
	{
		// en-DE: a decimal comma in numbers, a decimal point in currency — the BCL switches
		// when it sees the symbol, and so does the core now.
		var enDe = NumFormat.From(Culture("en-DE"));
		(enDe.DecimalSeparator, enDe.GroupSeparator, enDe.CurrencySymbol).ShouldBe((",", ".", "€"));
		(enDe.CurrencyDecimalSeparator, enDe.CurrencyGroupSeparator).ShouldBe((".", ","));
		(Cast.Decimal("€1,234.50", enDe) is Success<decimal> { Value: 1234.50m }).ShouldBeTrue();
		(Cast.Decimal("1,234.50 €", enDe) is Success<decimal> { Value: 1234.50m }).ShouldBeTrue();
		(Cast.Decimal("1.234,50", enDe) is Success<decimal> { Value: 1234.50m }).ShouldBeTrue();
		(Cast.Decimal("€1.234,50", enDe) is Fault { Reason: CastFailure.Malformed, Offset: 6, Length: 1 }).ShouldBeTrue();
		(Cast.Decimal("1,234.50", enDe) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		decimal.TryParse("€1,234.50", NumberStyles.Currency, Culture("en-DE"), out var bcl).ShouldBeTrue();
		bcl.ShouldBe(1234.50m);
		// A culture whose currency separators are its number separators declares none.
		var german = NumFormat.From(Culture("de-DE"));
		(german.CurrencyDecimalSeparator, german.CurrencyGroupSeparator).ShouldBe(("", ""));
		(Cast.Decimal("1.234,50 €", german) is Success<decimal> { Value: 1234.50m }).ShouldBeTrue();
		// Detection governs the whole token, currency separators included.
		(Cast.Decimal("€1,234.50", enDe with { Styles = NumStyles.All | NumStyles.SeparatorDetect }) is Success<decimal> { Value: 1234.50m }).ShouldBeTrue();
		(Cast.Decimal("€1.234,50", enDe with { Styles = NumStyles.All | NumStyles.SeparatorDetect }) is Success<decimal> { Value: 1234.50m }).ShouldBeTrue();
	}

	[Fact]
	void A_cultures_native_digits_are_read_beside_ascii()
	{
		var arabic = NumFormat.From(Culture("ar-SA"));
		arabic.DigitZero.ShouldBe(new Rune(0x0660));
		(arabic.DecimalSeparator, arabic.GroupSeparator).ShouldBe(("٫", "٬"));
		(Cast.Int32("١٢٣", arabic) is Success<int> { Value: 123 }).ShouldBeTrue();
		(Cast.Int32("-١٢٣", arabic) is Success<int> { Value: -123 }).ShouldBeTrue();
		(Cast.Int32("12٣", arabic) is Success<int> { Value: 123 }).ShouldBeTrue();
		(Cast.Double("١٬٢٣٤٫٥", arabic) is Success<double> { Value: 1234.5 }).ShouldBeTrue();
		(Cast.Decimal("١٢٫٥", arabic) is Success<decimal> { Value: 12.5m }).ShouldBeTrue();
		(Cast.Int32("١٢٣", NumFormat.Invariant) is Fault { Reason: CastFailure.Malformed, Offset: 0, Length: 1 }).ShouldBeTrue();
		// Persian digits start another block; Chakma's is outside the BMP, which a Rune holds and a char could not.
		NumFormat.From(Culture("fa-IR")).DigitZero.ShouldBe(new Rune(0x06F0));
		(Cast.Int32("۱۲۳", NumFormat.From(Culture("fa-IR"))) is Success<int> { Value: 123 }).ShouldBeTrue();
		var chakma = NumFormat.From(Culture("ccp-BD"));
		chakma.DigitZero.ShouldBe(new Rune(0x11136));
		(Cast.Int32("\U00011137\U00011138\U00011139", chakma) is Success<int> { Value: 123 }).ShouldBeTrue();
		// ASCII digits declare nothing.
		NumFormat.From(Culture("en-US")).DigitZero.ShouldBeNull();
		NumFormat.From(CultureInfo.InvariantCulture).DigitZero.ShouldBeNull();
	}

	[Fact]
	void A_cultures_percent_symbol_is_read_beside_the_ascii_one()
	{
		var arabic = NumFormat.From(Culture("ar-SA"));
		arabic.PercentSymbol.ShouldBe("٪؜");
		(Cast.Double("٥٠٪؜", arabic) is Success<double> { Value: 0.5 }).ShouldBeTrue();
		(Cast.Double("50%", arabic) is Success<double> { Value: 0.5 }).ShouldBeTrue();
		(Cast.Decimal("٥٠٪؜", arabic) is Success<decimal> { Value: 0.5m }).ShouldBeTrue();
		var persian = NumFormat.From(Culture("fa-IR"));
		persian.PercentSymbol.ShouldBe("٪");
		(Cast.Double("۵۰٪", persian) is Success<double> { Value: 0.5 }).ShouldBeTrue();
		(Cast.Double("50٪", NumFormat.Invariant) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		// The integer doors never read a percent, declared or not.
		(Cast.Int32("۵۰٪", persian) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
	}

	[Fact]
	void A_trailing_sign_comes_from_the_cultures_negative_pattern()
	{
		// No ICU culture writes a plain number as `n-`, so the number pattern is proven on a
		// NumberFormatInfo built by hand; fy-NL writes its currency amounts `€ 5,00-`.
		var trailing = NumFormat.From(new NumberFormatInfo { NumberNegativePattern = 3 });
		trailing.Styles.ShouldBe(NumStyles.All | NumStyles.TrailingSign);
		NumFormat.From(new NumberFormatInfo { NumberNegativePattern = 4 }).Styles.ShouldBe(NumStyles.All | NumStyles.TrailingSign);
		NumFormat.From(new NumberFormatInfo { NumberNegativePattern = 1 }).Styles.ShouldBe(NumStyles.All);
		(Cast.Int32("5-", trailing) is Success<int> { Value: -5 }).ShouldBeTrue();
		(Cast.Int32("5 -", trailing) is Success<int> { Value: -5 }).ShouldBeTrue();
		(Cast.Int32("5+", trailing) is Success<int> { Value: 5 }).ShouldBeTrue();
		(Cast.Decimal("2.5%-", trailing) is Success<decimal> { Value: -0.025m }).ShouldBeTrue();
		(Cast.Int32("-5-", trailing) is Fault { Reason: CastFailure.Malformed, Offset: 2, Length: 1 }).ShouldBeTrue();
		(Cast.Int32("(5-)", trailing) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		(Cast.Int32("5-", NumFormat.Invariant) is Fault { Reason: CastFailure.Malformed, Offset: 1, Length: 1 }).ShouldBeTrue();
		int.TryParse("5-", NumberStyles.AllowTrailingSign, CultureInfo.InvariantCulture, out var bcl).ShouldBeTrue();
		bcl.ShouldBe(-5);
		var frisian = NumFormat.From(Culture("fy-NL"));
		Culture("fy-NL").NumberFormat.CurrencyNegativePattern.ShouldBe(11);
		frisian.Styles.ShouldBe(NumStyles.All | NumStyles.TrailingSign);
		(Cast.Decimal("€ 5,00-", frisian) is Success<decimal> { Value: -5m }).ShouldBeTrue();
		(Cast.Decimal("5,00 €-", frisian) is Success<decimal> { Value: -5m }).ShouldBeTrue();
		(Cast.Decimal("5,00- €", frisian) is Success<decimal> { Value: -5m }).ShouldBeTrue();
		NumFormat.From(Culture("nl-NL")).Styles.ShouldBe(NumStyles.All);
		NumFormat.From(Culture("en-US")).Styles.ShouldBe(NumStyles.All);
	}

	[Fact]
	void Wide_separators_carry_over_whole()
	{
		// fr-FR groups with U+202F (U+00A0 on older ICU) and de-CH with U+2019; the
		// separator carries over as the string it is, no first-character truncation.
		var french = NumFormat.From(Culture("fr-FR"));
		french.GroupSeparator.ShouldBe(Culture("fr-FR").NumberFormat.NumberGroupSeparator);
		(Cast.Double($"1{french.GroupSeparator}234,5", french) is Success<double> { Value: 1234.5 }).ShouldBeTrue();
		var swiss = NumFormat.From(Culture("de-CH"));
		swiss.GroupSeparator.ShouldBe("’");
		(Cast.Int32("1’234’567", swiss) is Success<int> { Value: 1_234_567 }).ShouldBeTrue();
		(Cast.Decimal("CHF 1’234.50", swiss) is Success<decimal> { Value: 1234.50m }).ShouldBeTrue();
		// A two-character separator, which no culture declares and a format can.
		var spaced = new NumFormat(".", ", ", NumStyles.All);
		(Cast.Int32("1, 234, 567", spaced) is Success<int> { Value: 1_234_567 }).ShouldBeTrue();
		(Cast.Int32("1,234", spaced) is Fault { Reason: CastFailure.Malformed, Offset: 1, Length: 1 }).ShouldBeTrue();
	}

	[Fact]
	void The_invariant_culture_declares_nothing_past_its_currency_symbol()
	{
		var invariant = NumFormat.From(CultureInfo.InvariantCulture);
		invariant.ShouldBe(NumFormat.Invariant with { CurrencySymbol = "¤" });
		(invariant.CurrencyDecimalSeparator, invariant.CurrencyGroupSeparator, invariant.MinusSign, invariant.PlusSign, invariant.PercentSymbol)
			.ShouldBe(("", "", "", "", ""));
		invariant.DigitZero.ShouldBeNull();
		// en-US likewise: the ASCII spellings are what the core reads anyway.
		var english = NumFormat.From(Culture("en-US"));
		english.ShouldBe(NumFormat.Invariant with { CurrencySymbol = "$" });
	}

	[Fact]
	void Day_periods_come_from_the_cultures_designators()
	{
		var korean = DateFormat.From(Culture("ko-KR"));
		(korean.Order, korean.AMDesignator, korean.PMDesignator).ShouldBe((DateOrder.YearMonthDay, "오전", "오후"));
		(Cast.DateTime("2026-01-07 오후 3:04", korean) is Success<DateTime> { Value: var pm } && pm == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue();
		(Cast.DateTime("2026-01-07 오후3:04", korean) is Success<DateTime> { Value: var flush } && flush == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue();
		(Cast.DateTime("2026-01-07 오전 12:00", korean) is Success<DateTime> { Value: var midnight } && midnight == new DateTime(2026, 1, 7)).ShouldBeTrue();
		(Cast.DateTime("2026-01-07 3:04 PM", korean) is Success<DateTime> { Value: var english } && english == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue();
		(Cast.DateTime("2026-01-07 오후 3:04", DateOrder.YearMonthDay) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		// Spanish writes `a. m.` with a no-break space, which is part of the word.
		var spanish = DateFormat.From(Culture("es-ES"));
		spanish.PMDesignator.ShouldBe("p. m.");
		(Cast.DateTime($"7/1/2026 3:04 {spanish.PMDesignator}", spanish) is Success<DateTime> { Value: var madrid } && madrid == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue();
		// Arabic's are one letter each.
		var arabic = DateFormat.From(Culture("ar-SA"));
		(arabic.AMDesignator, arabic.PMDesignator).ShouldBe(("ص", "م"));
		(Cast.DateTime("2026-01-07 3:04 م", arabic with { Order = DateOrder.YearMonthDay }) is Success<DateTime> { Value: var riyadh } && riyadh == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue();
	}

	[Fact]
	void The_english_pair_and_a_designator_too_wide_for_the_layout_are_left_undeclared()
	{
		// de-DE's designators are the English pair on this ICU, which the core reads anyway.
		foreach (var name in new[] { "de-DE", "en-US", "en-GB", "fr-FR", "" })
		{
			var format = DateFormat.From(Culture(name));
			format.HasDesignators.ShouldBeFalse(name);
			(Cast.DateTime("2026-01-07 3:04 pm", format with { Order = DateOrder.YearMonthDay }) is Success<DateTime> { Value: var value } && value == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue(name);
		}
		// Thai's and Nepali's designators run past 16 bytes of UTF-8; the layout cannot carry
		// them, so those cultures read the English pair only rather than throw.
		foreach (var name in new[] { "th-TH", "ne-NP" })
		{
			var culture = Culture(name);
			Encoding.UTF8.GetByteCount(culture.DateTimeFormat.AMDesignator).ShouldBeGreaterThan(DateFormat.MaxDesignatorBytes, name);
			Encoding.UTF8.GetByteCount(culture.DateTimeFormat.PMDesignator).ShouldBeGreaterThan(DateFormat.MaxDesignatorBytes, name);
			DateFormat.From(culture).HasDesignators.ShouldBeFalse(name);
			Should.NotThrow(() => DateFormat.From(culture).ToRaw(), name);
		}
		// Each designator is judged on its own: Kölsch's AM is too wide and its PM is exactly sixteen bytes.
		var kölsch = Culture("ksh-DE");
		Encoding.UTF8.GetByteCount(kölsch.DateTimeFormat.PMDesignator).ShouldBe(DateFormat.MaxDesignatorBytes);
		(DateFormat.From(kölsch).AMDesignator, DateFormat.From(kölsch).PMDesignator).ShouldBe(("", kölsch.DateTimeFormat.PMDesignator));
		(Cast.DateTime($"2026-01-07 3:04 {kölsch.DateTimeFormat.PMDesignator}", DateFormat.From(kölsch) with { Order = DateOrder.YearMonthDay }) is Success<DateTime> { Value: var cologne }
			&& cologne == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue();
	}

	/// <summary>
	/// Every culture this runtime ships maps to a format and a date format the core accepts:
	/// the bridge never throws on real data, and nothing it builds is a contract violation
	/// at the crossing (which a door would surface as an <see cref="InvalidOperationException"/>).
	/// </summary>
	[Fact]
	void Every_shipped_culture_bridges_without_a_throw_and_crosses_without_a_violation()
	{
		var cultures = CultureInfo.GetCultures(CultureTypes.AllCultures);
		cultures.Length.ShouldBeGreaterThan(100);
		foreach (var culture in cultures)
		{
			var format = NumFormat.From(culture);
			Should.NotThrow(() => format.ToRaw(), culture.Name);
			(Cast.Int32("1", format) is Success<int> { Value: 1 }).ShouldBeTrue(culture.Name);
			(Cast.Double($"1{format.DecimalSeparator}5", format) is Success<double> { Value: 1.5 }).ShouldBeTrue(culture.Name);
			var dateFormat = DateFormat.From(culture);
			Should.NotThrow(() => dateFormat.ToRaw(), culture.Name);
			(Cast.DateTime("2026-01-07 15:04", dateFormat with { Order = DateOrder.YearMonthDay }) is Success<DateTime>).ShouldBeTrue(culture.Name);
			Cast.Date("2026-01-07", DateOrders.From(culture.DateTimeFormat)).ShouldBe(Cast.Date("2026-01-07", DateOrders.From(culture)), culture.Name);
		}
	}
}
