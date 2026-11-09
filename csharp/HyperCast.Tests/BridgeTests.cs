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

	// A culture with its designators set by hand: a real culture object for the door to
	// resolve, with spellings this test chose rather than this runtime's ICU or NLS.
	static CultureInfo WithDesignators(string name, string am, string pm)
	{
		var culture = (CultureInfo)Culture(name).Clone();
		culture.DateTimeFormat.AMDesignator = am;
		culture.DateTimeFormat.PMDesignator = pm;
		return culture;
	}

	// The bridge's rule for a designator, restated: declared unless it is the English one,
	// wider than the layout, carries an ASCII digit, or is edged with white space (the
	// corpus's set, not the binding's table).
	static string Expected(string designator, string english) =>
		designator.Length == 0
			|| designator.Equals(english, StringComparison.OrdinalIgnoreCase)
			|| Encoding.UTF8.GetByteCount(designator) > DateFormat.MaxDesignatorBytes
			|| designator.Any(char.IsAsciiDigit)
			|| CorpusTests.IsWhiteSpace(designator[0])
			|| CorpusTests.IsWhiteSpace(designator[^1])
			? ""
			: designator;

	// Whether the core can read the symbol, restated: one to `maxBytes` bytes of whole code
	// points, no ASCII digit, and no white space unless it is a group separator.
	static bool Readable(string symbol, int maxBytes, bool mayHoldWhiteSpace)
	{
		if (symbol.Length == 0 || symbol.Any(char.IsAsciiDigit) || (!mayHoldWhiteSpace && symbol.Any(static c => CorpusTests.IsWhiteSpace(c))))
			return false;
		for (var i = 0; i < symbol.Length; i++)
			if (char.IsLowSurrogate(symbol[i]) || (char.IsHighSurrogate(symbol[i]) && !(++i < symbol.Length && char.IsLowSurrogate(symbol[i]))))
				return false;
		return Encoding.UTF8.GetByteCount(symbol) <= maxBytes;
	}

	// A sign or percent symbol the bridge declares: not the ASCII one, and readable with no
	// white space in it.
	static string ExpectedSymbol(string symbol, string ascii, int maxBytes) =>
		symbol != ascii && Readable(symbol, maxBytes, mayHoldWhiteSpace: false) ? symbol : "";

	// The currency symbol the bridge declares: readable, with white space allowed inside (CLDR's
	// `F CFA`) but not at either edge.
	static string ExpectedCurrency(string symbol) =>
		Readable(symbol, NumFormat.MaxCurrencyBytes, mayHoldWhiteSpace: true)
			&& !CorpusTests.IsWhiteSpace(symbol[0]) && !CorpusTests.IsWhiteSpace(symbol[^1])
			? symbol
			: "";

	// The separators the bridge declares for a culture's number data, restated: the culture's
	// where readable (a decimal separator with no white space, a group separator with any),
	// else the ASCII default; a group equal to the decimal one takes the other of the ASCII
	// pair; the currency pair only where it differs from the number pair and does not collide.
	static (string Decimal, string Group, string CurrencyDecimal, string CurrencyGroup) ExpectedSeparators(NumberFormatInfo data)
	{
		var number = Readable(data.NumberDecimalSeparator, NumFormat.MaxSymbolBytes, false) ? data.NumberDecimalSeparator : ".";
		var group = Readable(data.NumberGroupSeparator, NumFormat.MaxSymbolBytes, true) ? data.NumberGroupSeparator : ",";
		if (group == number)
			group = number == "," ? "." : ",";
		var currencyNumber = Readable(data.CurrencyDecimalSeparator, NumFormat.MaxSymbolBytes, false) ? data.CurrencyDecimalSeparator : number;
		var currencyGroup = Readable(data.CurrencyGroupSeparator, NumFormat.MaxSymbolBytes, true) ? data.CurrencyGroupSeparator : group;
		if (currencyNumber == currencyGroup)
			return (number, group, "", "");
		return (number, group, currencyNumber == number ? "" : currencyNumber, currencyGroup == group ? "" : currencyGroup);
	}

	// A format as the bridge declares it for hand-built group sizes: everything else invariant.
	static NumFormat WithSizes(int[] number, int[] currency)
	{
		var data = (NumberFormatInfo)CultureInfo.InvariantCulture.NumberFormat.Clone();
		data.NumberGroupSizes = number;
		data.CurrencyGroupSizes = currency;
		return NumFormat.From(data);
	}

	// The rule, restated: the number pattern's all-Indian is Indian, anything else Western; the flag goes only
	// when neither pattern groups (a leading 0 is none).
	static (NumGrouping, bool) ExpectedGrouping(int[] number, int[] currency) =>
		(number is [3, 2] or [3, 2, 2] ? NumGrouping.Indian : NumGrouping.Western,
			new[] { number, currency }.Any(static p => p.Length != 0 && p[0] != 0));

	// Whether a currency amount's shape is one the culture's declared policy reads: a Western amount through
	// `ml`'s Indian numbers is refused, which The_bridge_refuses_the_pattern_its_policy_does_not_name pins.
	static bool CurrencyFollowsTheNumberPattern(NumberFormatInfo numbers)
	{
		static bool Indian(int[] sizes) => sizes is [3, 2] or [3, 2, 2];
		static bool None(int[] sizes) => sizes.Length == 0 || sizes[0] == 0;
		return None(numbers.CurrencyGroupSizes) || Indian(numbers.CurrencyGroupSizes) == Indian(numbers.NumberGroupSizes);
	}

	[Fact]
	void A_cultures_own_patterns_decide_the_grouping_policy_never_its_name()
	{
		// ICU spells en-IN and hi-IN with the Indian pattern, en-US and de-DE with the Western one.
		Culture("en-IN").NumberFormat.NumberGroupSizes.ShouldBe([3, 2]);
		Culture("en-IN").NumberFormat.CurrencyGroupSizes.ShouldBe([3, 2]);
		foreach (var name in new[] { "en-IN", "hi-IN" })
			NumFormat.From(Culture(name)).Grouping.ShouldBe(NumGrouping.Indian, name);
		foreach (var name in new[] { "en-US", "de-DE", "fr-FR", "" })
		{
			NumFormat.From(Culture(name)).Grouping.ShouldBe(NumGrouping.Western, name);
			NumFormat.From(Culture(name)).Styles.HasFlag(NumStyles.Grouping).ShouldBeTrue(name);
		}
		// The next three shapes depend on the runtime's ICU (Windows' spells tok [3], Linux's spells
		// en-BG's currency [3]), so each is proven on sizes set by hand, and on the culture where it has them.
		static NumberFormatInfo Sized(int[] number, int[] currency)
		{
			var sized = (NumberFormatInfo)CultureInfo.InvariantCulture.NumberFormat.Clone();
			sized.NumberGroupSizes = number;
			sized.CurrencyGroupSizes = currency;
			return sized;
		}
		// Toki Pona groups by twos, which no policy names: Western, and its grouped output is refused.
		NumFormat.From(Sized([2], [2])).Grouping.ShouldBe(NumGrouping.Western);
		foreach (var name in new[] { "tok", "tok-001" })
			if (Culture(name).NumberFormat is { NumberGroupSizes: [2], CurrencyGroupSizes: [2] })
				NumFormat.From(Culture(name)).Grouping.ShouldBe(NumGrouping.Western, name);
		// en-US-POSIX writes no grouping anywhere ([0]): the flag goes, since nothing is grouped.
		NumFormat.From(Sized([0], [0])).Styles.HasFlag(NumStyles.Grouping).ShouldBeFalse();
		if (Culture("en-US-POSIX").NumberFormat is { NumberGroupSizes: [0], CurrencyGroupSizes: [0] })
			NumFormat.From(Culture("en-US-POSIX")).Styles.HasFlag(NumStyles.Grouping).ShouldBeFalse();
		// en-BG groups numbers and not currency: the flag stays for the one that does, as Western.
		foreach (var oneSided in new[] { Sized([3], [0]), Culture("en-BG").NumberFormat })
		{
			var bulgaria = NumFormat.From(oneSided);
			bulgaria.Grouping.ShouldBe(NumGrouping.Western);
			bulgaria.Styles.HasFlag(NumStyles.Grouping).ShouldBeTrue();
		}
		// Indian numbers and Western currency (ICU's ml, mr and or, as of ICU 78) are Indian: the number pattern
		// decides. Whichever cultures this runtime's ICU has that disagree, each follows its number sizes.
		var disagreeing = CultureInfo.GetCultures(CultureTypes.AllCultures)
			.Where(static c => c.NumberFormat.NumberGroupSizes is [3, 2] != c.NumberFormat.CurrencyGroupSizes is [3, 2]).ToArray();
		foreach (var culture in disagreeing)
			NumFormat.From(culture).Grouping.ShouldBe(culture.NumberFormat.NumberGroupSizes is [3, 2] ? NumGrouping.Indian : NumGrouping.Western, culture.Name);

		// The policy is read from the sizes, not the name: an English locale given the Indian sizes is Indian,
		// an Indian one given the Western sizes is Western.
		var indian = (NumberFormatInfo)Culture("en-US").NumberFormat.Clone();
		indian.NumberGroupSizes = [3, 2];
		indian.CurrencyGroupSizes = [3, 2];
		NumFormat.From(indian).Grouping.ShouldBe(NumGrouping.Indian);
		var western = (NumberFormatInfo)Culture("hi-IN").NumberFormat.Clone();
		western.NumberGroupSizes = [3];
		western.CurrencyGroupSizes = [3];
		NumFormat.From(western).Grouping.ShouldBe(NumGrouping.Western);
	}

	[Fact]
	void A_number_and_a_currency_pattern_share_one_policy_and_one_flag()
	{
		// (number, currency) → policy, and whether the flag stays. Only patterns that group count.
		(int[] Number, int[] Currency, NumGrouping Policy, bool Groups)[] cases =
		[
			([3], [3], NumGrouping.Western, true),
			([3, 2], [3, 2], NumGrouping.Indian, true),
			([3], [3, 2], NumGrouping.Western, true),       // the number pattern decides
			([3, 2], [3], NumGrouping.Indian, true),
			([3, 2], [0], NumGrouping.Indian, true),        // Indian + none
			([0], [3, 2], NumGrouping.Western, true),
			([3], [0], NumGrouping.Western, true),          // Western + none
			([0], [3], NumGrouping.Western, true),
			([3], [2], NumGrouping.Western, true),
			([2], [3], NumGrouping.Western, true),
			([3, 2], [2], NumGrouping.Indian, true),
			([2], [2], NumGrouping.Western, true),          // Toki Pona
			([0], [2], NumGrouping.Western, true),
			([0], [0], NumGrouping.Western, false),         // none + none: the flag is cleared
			([], [], NumGrouping.Western, false),
			// .NET's meaning of the sizes: the last nonzero size repeats, a trailing 0 ends grouping.
			([3, 3], [3], NumGrouping.Western, true),
			([3, 2, 2], [3, 2], NumGrouping.Indian, true),
			([3, 0], [3], NumGrouping.Western, true),       // 1234,567: finite, no policy
			([3, 2, 0], [3, 2, 0], NumGrouping.Western, true),
			([2, 3], [3], NumGrouping.Western, true),
			([4], [4], NumGrouping.Western, true),
		];
		foreach (var (number, currency, policy, groups) in cases)
		{
			var label = $"[{string.Join(",", number)}] / [{string.Join(",", currency)}]";
			var format = WithSizes(number, currency);
			format.Grouping.ShouldBe(policy, label);
			format.Styles.HasFlag(NumStyles.Grouping).ShouldBe(groups, label);
		}
		// The caller-built number [3] with currency [3, 2]: Western, whose reads the number's and refuses the rest.
		var mixed = WithSizes([3], [3, 2]);
		(Cast.Int32("1,234,567", mixed) is Success<int> { Value: 1_234_567 }).ShouldBeTrue();
		(Cast.Int32("12,34,567", mixed) is Fault { Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		// Cleared grouping forbids a group separator, which no pattern of this culture writes.
		(Cast.Int32("1,234", WithSizes([0], [0])) is Fault).ShouldBeTrue();
		(Cast.Int32("1234", WithSizes([0], [0])) is Success<int> { Value: 1234 }).ShouldBeTrue();
		// One flag serves both: with just the number grouped, an ungrouped amount is still valid.
		(Cast.Int32("1234", WithSizes([3], [0])) is Success<int> { Value: 1234 }).ShouldBeTrue();
	}

	// What a platform writes it must read: the bridge's format against the culture's own "N" and "C" output.
	static readonly decimal[] _values = [0, 7, 12.5m, 123, 1234, 12345.67m, 123456, 1234567.89m, 12345678, -1234567.89m, -42];

	static void ReadsItsOwnOutput(CultureInfo culture)
	{
		var format = NumFormat.From(culture);
		foreach (var value in _values)
			foreach (var specifier in new[] { "N2", "C2", "N0" })
			{
				if (specifier == "C2" && !CurrencyFollowsTheNumberPattern(culture.NumberFormat))
					continue;
				var text = value.ToString(specifier, culture);
				var expected = decimal.Round(value, specifier == "N0" ? 0 : 2, MidpointRounding.AwayFromZero);
				var verdict = Cast.Decimal(text, format);
				(verdict.TryGetValue(out Success<decimal> ok) && ok.Value == expected)
					.ShouldBeTrue($"{culture.Name}: {specifier} of {value} is '{text}', read as {verdict}");
			}
	}

	[Fact]
	void The_bridge_reads_what_representative_cultures_write()
	{
		foreach (var name in new[] { "en-IN", "hi-IN", "en-US", "en-BG", "en-US-POSIX", "de-DE", "fr-FR", "ml-IN", "ta-IN", "mr-IN", "or-IN", "de-CH", "ar-SA", "" })
			ReadsItsOwnOutput(Culture(name));
		// tok's grouping is the platform's data: ICU writes twos (1 23 45 67), Windows' copy need not.
		// The bridge declares Western either way, which reads threes and refuses anything else, never
		// misreading.
		var tok = Culture("tok");
		NumFormat.From(tok).Grouping.ShouldBe(NumGrouping.Western);
		var written = 1234567.89m.ToString("N2", tok);
		var read = Cast.Decimal(written, NumFormat.From(tok));
		if (tok.NumberFormat.NumberGroupSizes is [3] or [3, 0])
			(read.TryGetValue(out Success<decimal> ok) && ok.Value == 1234567.89m).ShouldBeTrue($"tok wrote '{written}', read as {read}");
		else
			(read is Fault { Detail: FaultDetail.GroupSize }).ShouldBeTrue($"tok wrote '{written}', read as {read}");
	}

	[Fact]
	void The_bridge_refuses_the_pattern_its_policy_does_not_name()
	{
		var data = (NumberFormatInfo)CultureInfo.InvariantCulture.NumberFormat.Clone();
		data.NumberGroupSizes = [3, 2];
		data.CurrencyGroupSizes = [3];
		data.CurrencySymbol = "₹";
		var format = NumFormat.From(data);
		format.Grouping.ShouldBe(NumGrouping.Indian);
		(Cast.Decimal("₹1,234,567.50", format) is Fault { Offset: 2, Length: 4, Detail: FaultDetail.GroupSize }).ShouldBeTrue();
		(Cast.Decimal("₹1,234,567.50", format with { Grouping = NumGrouping.Western }) is Success<decimal> { Value: 1_234_567.5m }).ShouldBeTrue();
	}

	/// <summary>
	/// The rule held on every culture this runtime ships, as far as the data goes: the bridge's format reads the
	/// culture's own formatted numbers, and amounts whose shape is the policy's.
	/// </summary>
	[Fact]
	void The_bridge_reads_every_shipped_cultures_own_grouped_output()
	{
		var failures = new List<string>();
		// Toki Pona's twos are refused: see The_bridge_reads_what_representative_cultures_write.
		bool Twos(CultureInfo culture) => culture.NumberFormat.NumberGroupSizes is [2, ..];
		foreach (var culture in CultureInfo.GetCultures(CultureTypes.AllCultures).Where(culture => !Twos(culture)))
		{
			var format = NumFormat.From(culture);
			foreach (var value in new[] { 1234567.89m, 12345678.5m, 1234m })
				foreach (var specifier in new[] { "N2", "C2" })
				{
					// A culture spelling both separators alike (mn-Mong-MN on Windows: `1,234,567,89`)
					// writes text no reader can split.
					var numbers = culture.NumberFormat;
					if (specifier == "N2" ? numbers.NumberDecimalSeparator == numbers.NumberGroupSeparator : numbers.CurrencyDecimalSeparator == numbers.CurrencyGroupSeparator)
						continue;
					if (specifier == "C2" && !CurrencyFollowsTheNumberPattern(numbers))
						continue;
					var text = value.ToString(specifier, culture);
					if (Cast.Decimal(text, format) is Success<decimal> { Value: var read } && read == decimal.Round(value, 2, MidpointRounding.AwayFromZero))
						continue;
					failures.Add($"{culture.Name} {specifier} '{text}'");
				}
		}
		// Anything this misses must be a symbol, never the digit groups: the same text with the groups taken out fares no better.
		foreach (var culture in CultureInfo.GetCultures(CultureTypes.AllCultures).Where(culture => !Twos(culture)))
		{
			var format = NumFormat.From(culture);
			var text = 1234567.89m.ToString("N2", culture);
			var verdict = Cast.Decimal(text, format);
			if (verdict.TryGetValue(out Fault fault))
				fault.Detail.ShouldNotBe(FaultDetail.GroupSize, $"{culture.Name} '{text}'");
		}
		failures.ShouldBeEmpty(string.Join("; ", failures.Take(20)));
	}

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
		// when it sees the symbol, and so does the core now. ICU spells en-DE so on this
		// machine and not on every runtime, so the data is built by hand in en-DE's shape,
		// and the real culture is checked against the bridge's rule below.
		var enDe = NumFormat.From(new NumberFormatInfo
		{
			NumberDecimalSeparator = ",",
			NumberGroupSeparator = ".",
			CurrencyDecimalSeparator = ".",
			CurrencyGroupSeparator = ",",
			CurrencySymbol = "€",
		});
		(enDe.DecimalSeparator, enDe.GroupSeparator, enDe.CurrencySymbol).ShouldBe((",", ".", "€"));
		(enDe.CurrencyDecimalSeparator, enDe.CurrencyGroupSeparator).ShouldBe((".", ","));
		var real = Culture("en-DE").NumberFormat;
		NumFormat.From(real).CurrencyDecimalSeparator.ShouldBe(real.CurrencyDecimalSeparator == real.NumberDecimalSeparator ? "" : real.CurrencyDecimalSeparator);
		(Cast.Decimal("€1,234.50", enDe) is Success<decimal> { Value: 1234.50m }).ShouldBeTrue();
		(Cast.Decimal("1,234.50 €", enDe) is Success<decimal> { Value: 1234.50m }).ShouldBeTrue();
		(Cast.Decimal("1.234,50", enDe) is Success<decimal> { Value: 1234.50m }).ShouldBeTrue();
		(Cast.Decimal("€1.234,50", enDe) is Fault { Reason: CastFailure.Malformed, Offset: 6, Length: 1 }).ShouldBeTrue();
		(Cast.Decimal("1,234.50", enDe) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		decimal.TryParse("€1,234.50", NumberStyles.Currency, new NumberFormatInfo { CurrencyDecimalSeparator = ".", CurrencyGroupSeparator = ",", CurrencySymbol = "€" }, out var bcl).ShouldBeTrue();
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
		// fr-FR groups with U+202F (U+00A0 on older ICU) and de-CH with U+2019 (an ASCII
		// apostrophe on some runtimes' data); whatever this runtime spells, the separator
		// carries over as the string it is, no first-character truncation.
		var french = NumFormat.From(Culture("fr-FR"));
		french.GroupSeparator.ShouldBe(Culture("fr-FR").NumberFormat.NumberGroupSeparator);
		(Cast.Double($"1{french.GroupSeparator}234,5", french) is Success<double> { Value: 1234.5 }).ShouldBeTrue();
		var swiss = NumFormat.From(Culture("de-CH"));
		swiss.GroupSeparator.ShouldBe(Culture("de-CH").NumberFormat.NumberGroupSeparator);
		(Cast.Int32($"1{swiss.GroupSeparator}234{swiss.GroupSeparator}567", swiss) is Success<int> { Value: 1_234_567 }).ShouldBeTrue();
		var typographic = new NumFormat(".", "’", NumStyles.All, "CHF");
		(Cast.Int32("1’234’567", typographic) is Success<int> { Value: 1_234_567 }).ShouldBeTrue();
		(Cast.Decimal("CHF 1’234.50", typographic) is Success<decimal> { Value: 1234.50m }).ShouldBeTrue();
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
		// The spellings are set on the cultures by hand (this machine's ICU has them so;
		// some runtimes' data has the English pair for ko-KR), so what is proven is the
		// bridge and the reading it buys, not one ICU's data; the real cultures are checked
		// against the bridge's rule in the loop at the end.
		var korean = DateFormat.From(WithDesignators("ko-KR", "오전", "오후"));
		(korean.Order, korean.AMDesignator, korean.PMDesignator).ShouldBe((DateOrder.YearMonthDay, "오전", "오후"));
		(Cast.DateTime("2026-01-07 오후 3:04", korean) is Success<DateTime> { Value: var pm } && pm == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue();
		(Cast.DateTime("2026-01-07 오후3:04", korean) is Success<DateTime> { Value: var flush } && flush == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue();
		(Cast.DateTime("2026-01-07 오전 12:00", korean) is Success<DateTime> { Value: var midnight } && midnight == new DateTime(2026, 1, 7)).ShouldBeTrue();
		(Cast.DateTime("2026-01-07 3:04 PM", korean) is Success<DateTime> { Value: var english } && english == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue();
		(Cast.DateTime("2026-01-07 오후 3:04", DateOrder.YearMonthDay) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		// Spanish writes `a. m.` with a no-break space (narrow on some ICUs), which is part of the word.
		var spanish = DateFormat.From(WithDesignators("es-ES", "a. m.", "p. m."));
		spanish.PMDesignator.ShouldBe("p. m.");
		(Cast.DateTime($"7/1/2026 3:04 {spanish.PMDesignator}", spanish) is Success<DateTime> { Value: var madrid } && madrid == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue();
		// Arabic's are one letter each.
		var arabic = DateFormat.From(WithDesignators("ar-SA", "ص", "م"));
		(arabic.AMDesignator, arabic.PMDesignator).ShouldBe(("ص", "م"));
		(Cast.DateTime("2026-01-07 3:04 م", arabic with { Order = DateOrder.YearMonthDay }) is Success<DateTime> { Value: var riyadh } && riyadh == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue();
		// The real cultures, as this runtime spells them: the bridge declares exactly what its
		// rule says, and whatever it declares, the culture's own word reads after the clock.
		foreach (var name in new[] { "ko-KR", "es-ES", "ar-SA", "de-DE", "ja-JP", "zh-CN", "vi-VN" })
		{
			var data = Culture(name).DateTimeFormat;
			var format = DateFormat.From(Culture(name)) with { Order = DateOrder.YearMonthDay };
			(format.AMDesignator, format.PMDesignator).ShouldBe((Expected(data.AMDesignator, "AM"), Expected(data.PMDesignator, "PM")), name);
			var word = format.PMDesignator.Length != 0 ? format.PMDesignator : "PM";
			(Cast.DateTime($"2026-01-07 3:04 {word}", format) is Success<DateTime> { Value: var value } && value == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue(name);
		}
	}

	[Fact]
	void The_english_pair_and_a_designator_the_core_could_not_read_are_left_undeclared()
	{
		// The English pair, in any case, which the core reads anyway.
		foreach (var (am, pm) in new[] { ("AM", "PM"), ("am", "pm"), ("Am", "Pm"), ("", "") })
		{
			var format = DateFormat.From(WithDesignators("en-US", am, pm));
			format.HasDesignators.ShouldBeFalse($"{am}/{pm}");
			(Cast.DateTime("2026-01-07 3:04 pm", format with { Order = DateOrder.YearMonthDay }) is Success<DateTime> { Value: var value } && value == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue(am);
		}
		// A designator the layout cannot carry (Thai's and Nepali's words run past 16 bytes),
		// one carrying an ASCII digit (Anii writes `1ka`/`1la`), and one edged with white
		// space (any kind) are left undeclared rather than thrown on, each judged on its own; the
		// culture reads the English pair beside whatever is declared. Sixteen bytes exactly fits.
		var sixteen = new string('ä', 8);
		var seventeen = sixteen + "x";
		foreach (var (am, pm, expectedAm, expectedPm) in new[]
		{
			(seventeen, sixteen, "", sixteen),
			("ก่อนเที่ยง", "หลังเที่ยง", "", ""),
			("1ka", "1la", "", ""),
			("vorm. ", "nachm.", "", "nachm."),
			(" vorm.", "\tnachm.", "", ""),
			("vorm.\u00A0", "nachm.", "", "nachm."),
			("\u202Fvorm.", "\u3000nachm.", "", ""),
			("vorm.\u2009", "\u0085nachm.", "", ""),
			("a.\u202Fm.", "p.\u00A0m.", "a.\u202Fm.", "p.\u00A0m."),
			("a. m.", "p. m.", "a. m.", "p. m."),
		})
		{
			var format = DateFormat.From(WithDesignators("en-US", am, pm));
			(format.AMDesignator, format.PMDesignator).ShouldBe((expectedAm, expectedPm), am);
			Should.NotThrow(() => format.ToRaw(), am);
			(Cast.DateTime("2026-01-07 3:04 PM", format with { Order = DateOrder.YearMonthDay }) is Success<DateTime> { Value: var english } && english == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue(am);
			if (expectedPm.Length != 0)
				(Cast.DateTime($"2026-01-07 3:04 {expectedPm}", format with { Order = DateOrder.YearMonthDay }) is Success<DateTime> { Value: var own } && own == new DateTime(2026, 1, 7, 15, 4, 0)).ShouldBeTrue(am);
		}
		// The real cultures those cases come from, as this runtime spells them.
		foreach (var name in new[] { "th-TH", "ne-NP", "de-DE", "en-GB", "fr-FR", "" })
		{
			var data = Culture(name).DateTimeFormat;
			var format = DateFormat.From(Culture(name));
			(format.AMDesignator, format.PMDesignator).ShouldBe((Expected(data.AMDesignator, "AM"), Expected(data.PMDesignator, "PM")), name);
			Should.NotThrow(() => format.ToRaw(), name);
		}
	}

	[Fact]
	void A_runtimes_number_data_never_throws_whatever_it_holds()
	{
		// NLS gives mn-Mong-MN a comma for both separators; the group takes the other of the
		// ASCII pair, and the BCL's own reading (decimal only) is preserved.
		var comma = NumFormat.From(new NumberFormatInfo { NumberDecimalSeparator = ",", NumberGroupSeparator = "," });
		(comma.DecimalSeparator, comma.GroupSeparator).ShouldBe((",", "."));
		(Cast.Double("1,5", comma) is Success<double> { Value: 1.5 }).ShouldBeTrue();
		var point = NumFormat.From(new NumberFormatInfo { NumberDecimalSeparator = ".", NumberGroupSeparator = "." });
		(point.DecimalSeparator, point.GroupSeparator).ShouldBe((".", ","));
		// A separator the core could not read (a digit; the BCL refuses an empty one itself)
		// falls back to the ASCII default; a symbol it
		// could not read is left undeclared, and the ASCII spelling is read alone.
		var odd = NumFormat.From(new NumberFormatInfo
		{
			NumberDecimalSeparator = "5",
			NumberGroupSeparator = "1",
			CurrencySymbol = "12",
			NegativeSign = "- ",
			PositiveSign = "+ ",
			PercentSymbol = "pct 0",
			CurrencyDecimalSeparator = "abcdefghi",
			CurrencyGroupSeparator = ".",
		});
		(odd.DecimalSeparator, odd.GroupSeparator, odd.CurrencySymbol).ShouldBe((".", ",", ""));
		(odd.MinusSign, odd.PlusSign, odd.PercentSymbol).ShouldBe(("", "", ""));
		// The currency pair in force collided (decimal falls back to '.', group declared '.'), so neither is declared.
		(odd.CurrencyDecimalSeparator, odd.CurrencyGroupSeparator).ShouldBe(("", ""));
		Should.NotThrow(() => odd.ToRaw());
		(Cast.Int32("-1,234", odd) is Success<int> { Value: -1234 }).ShouldBeTrue();
		// A white-space group separator is one some cultures declare, and is read: any of the
		// space-like three stands for the others at the door.
		var spaced = NumFormat.From(new NumberFormatInfo { NumberGroupSeparator = " " });
		spaced.GroupSeparator.ShouldBe(" ");
		(Cast.Int32("1 234", spaced) is Success<int> { Value: 1234 }).ShouldBeTrue();
		var narrow = NumFormat.From(new NumberFormatInfo { NumberDecimalSeparator = ",", NumberGroupSeparator = "\u202F", CurrencyDecimalSeparator = ",", CurrencyGroupSeparator = "\u00A0", CurrencySymbol = "€" });
		(narrow.GroupSeparator, narrow.CurrencyGroupSeparator).ShouldBe(("\u202F", "\u00A0"));
		(Cast.Int32("1\u00A0234", narrow) is Success<int> { Value: 1234 }).ShouldBeTrue();
		(Cast.Decimal("1\u202F234,56\u00A0€", narrow) is Success<decimal> { Value: 1234.56m }).ShouldBeTrue();
		// White space in a decimal separator, a sign or the percent symbol, or at the edge of a
		// currency symbol, can never match the input (white space around a symbol is ignored), so
		// each is left undeclared: a decimal separator falls back to the ASCII default, and a
		// symbol to the ASCII spelling alone. Inside a currency symbol it is the symbol's own.
		var blank = NumFormat.From(new NumberFormatInfo
		{
			NumberDecimalSeparator = "\u00A0",
			NumberGroupSeparator = "\u202F",
			CurrencyDecimalSeparator = "\u202F ",
			CurrencyGroupSeparator = "\u00A0",
			CurrencySymbol = "Kč\u00A0",
			NegativeSign = "\u00A0-",
			PositiveSign = "+\u202F",
			PercentSymbol = "\u3000%",
		});
		(blank.DecimalSeparator, blank.GroupSeparator, blank.CurrencySymbol).ShouldBe((".", "\u202F", ""));
		(blank.MinusSign, blank.PlusSign, blank.PercentSymbol).ShouldBe(("", "", ""));
		(blank.CurrencyDecimalSeparator, blank.CurrencyGroupSeparator).ShouldBe(("", "\u00A0"));
		Should.NotThrow(() => blank.ToRaw());
		(Cast.Decimal("1\u00A0234.5", blank) is Success<decimal> { Value: 1234.5m }).ShouldBeTrue();
		// The CFA franc: CLDR's symbol holds a U+202F, the input writes any of the space-like
		// three there, and the symbol stays declared whole.
		foreach (var cfa in new[] { "F\u202FCFA", "F\u00A0CFA", "F CFA", "F\u2009CFA" })
		{
			var franc = NumFormat.From(new NumberFormatInfo { CurrencySymbol = cfa });
			franc.CurrencySymbol.ShouldBe(cfa);
			Should.NotThrow(() => franc.ToRaw(), cfa);
		}
		var cfaFormat = NumFormat.From(new NumberFormatInfo { CurrencySymbol = "F\u202FCFA", NumberDecimalSeparator = ",", CurrencyDecimalSeparator = ",", NumberGroupSeparator = "\u00A0", CurrencyGroupSeparator = "\u00A0" });
		foreach (var space in new[] { "\u202F", "\u00A0", " " })
			(Cast.Decimal($"1\u00A0234,5{space}F{space}CFA", cfaFormat) is Success<decimal> { Value: 1234.5m }).ShouldBeTrue();
		(Cast.Decimal("F\u00A0CFA 5", cfaFormat) is Success<decimal> { Value: 5m }).ShouldBeTrue();
		// A lone surrogate half is not whole code points; the symbol is left undeclared.
		NumFormat.From(new NumberFormatInfo { CurrencySymbol = "\ud83d" }).CurrencySymbol.ShouldBe("");
		NumFormat.From(new NumberFormatInfo { CurrencySymbol = "💰" }).CurrencySymbol.ShouldBe("💰");
	}

	/// <summary>
	/// Every culture this runtime ships maps to a format and a date format the core accepts:
	/// the bridge never throws on real data, and nothing it builds is a contract violation
	/// at the crossing (which a door would surface as an <see cref="InvalidOperationException"/>).
	/// It also asserts which symbols stay undeclared, by the rules restated above: an unreadable
	/// culture symbol (a digit, white space in a sign, percent or decimal separator or at the
	/// edge of a currency symbol, past its buffer) is left out, never worked around; white space
	/// inside a currency symbol (the CFA franc's <c>F CFA</c>) stays declared.
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
			var data = culture.NumberFormat;
			var (number, group, currencyNumber, currencyGroup) = ExpectedSeparators(data);
			(format.DecimalSeparator, format.GroupSeparator, format.CurrencyDecimalSeparator, format.CurrencyGroupSeparator)
				.ShouldBe((number, group, currencyNumber, currencyGroup), culture.Name);
			(format.MinusSign, format.PlusSign, format.PercentSymbol, format.CurrencySymbol).ShouldBe((
				ExpectedSymbol(data.NegativeSign, "-", NumFormat.MaxSymbolBytes), ExpectedSymbol(data.PositiveSign, "+", NumFormat.MaxSymbolBytes),
				ExpectedSymbol(data.PercentSymbol, "%", NumFormat.MaxSymbolBytes), ExpectedCurrency(data.CurrencySymbol)), culture.Name);
			// The grouping is what the culture's own patterns write, restated below.
			var (policy, groups) = ExpectedGrouping(data.NumberGroupSizes, data.CurrencyGroupSizes);
			(format.Grouping, format.Styles.HasFlag(NumStyles.Grouping)).ShouldBe((policy, groups), culture.Name);
			(Cast.Int32("1", format) is Success<int> { Value: 1 }).ShouldBeTrue(culture.Name);
			(Cast.Double($"1{format.DecimalSeparator}5", format) is Success<double> { Value: 1.5 }).ShouldBeTrue(culture.Name);
			var dateFormat = DateFormat.From(culture);
			(dateFormat.AMDesignator, dateFormat.PMDesignator)
				.ShouldBe((Expected(culture.DateTimeFormat.AMDesignator, "AM"), Expected(culture.DateTimeFormat.PMDesignator, "PM")), culture.Name);
			Should.NotThrow(() => dateFormat.ToRaw(), culture.Name);
			(Cast.DateTime("2026-01-07 15:04", dateFormat with { Order = DateOrder.YearMonthDay }) is Success<DateTime>).ShouldBeTrue(culture.Name);
			Cast.Date("2026-01-07", DateOrders.From(culture.DateTimeFormat)).ShouldBe(Cast.Date("2026-01-07", DateOrders.From(culture)), culture.Name);
		}
	}
}
