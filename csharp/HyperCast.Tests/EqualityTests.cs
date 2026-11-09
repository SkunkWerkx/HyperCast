namespace HyperCast.Tests;

/// <summary>
/// The declarations compare by what they declare, as the core compares them, and hash alike when
/// equal: a <see langword="null"/> string reads back as empty and is the same declaration, and two
/// vocabularies with the same entries are one, so options holding them are equal.
/// </summary>
public sealed class EqualityTests
{
	[Fact]
	void A_format_equals_one_declaring_the_same_thing_however_it_was_built()
	{
		var withNulls = NumFormat.Invariant with
		{
			CurrencySymbol = null!,
			CurrencyDecimalSeparator = null!,
			CurrencyGroupSeparator = null!,
			MinusSign = null!,
			PlusSign = null!,
			PercentSymbol = null!,
		};
		withNulls.ShouldBe(NumFormat.Invariant);
		withNulls.GetHashCode().ShouldBe(NumFormat.Invariant.GetHashCode());
		(withNulls == NumFormat.Invariant).ShouldBeTrue();
		new HashSet<NumFormat> { withNulls, NumFormat.Invariant, new(".", ",", NumStyles.All) }.Count.ShouldBe(1);

		var empty = new NumFormat("", "", NumStyles.None);
		default(NumFormat).ShouldBe(empty);
		default(NumFormat).GetHashCode().ShouldBe(empty.GetHashCode());

		// Any field, or the policy, apart: another declaration.
		NumFormat[] others =
		[
			new(",", ".", NumStyles.All),
			new(".", ",", NumStyles.Grouping),
			NumFormat.Invariant with { Grouping = NumGrouping.Indian },
			NumFormat.Invariant with { CurrencySymbol = "$" },
			NumFormat.Invariant with { CurrencyDecimalSeparator = ",", CurrencyGroupSeparator = "." },
			NumFormat.Invariant with { MinusSign = "−" },
			NumFormat.Invariant with { PlusSign = "＋" },
			NumFormat.Invariant with { PercentSymbol = "٪" },
			NumFormat.Invariant with { DigitZero = new System.Text.Rune(0x0660) },
		];
		foreach (var other in others)
			other.ShouldNotBe(NumFormat.Invariant);
		new HashSet<NumFormat>(others).Count.ShouldBe(others.Length);
	}

	[Fact]
	void A_date_format_equals_one_declaring_the_same_thing()
	{
		var withNulls = new DateFormat(DateOrder.DayMonthYear, null!, null!);
		withNulls.ShouldBe(new DateFormat(DateOrder.DayMonthYear));
		withNulls.GetHashCode().ShouldBe(new DateFormat(DateOrder.DayMonthYear).GetHashCode());
		var german = new DateFormat(DateOrder.DayMonthYear, "vorm.", "nachm.");
		german.ShouldBe(new DateFormat(DateOrder.DayMonthYear) with { AMDesignator = "vorm.", PMDesignator = "nachm." });
		german.ShouldNotBe(new DateFormat(DateOrder.DayMonthYear, "vorm.", "NACHM."));
		german.ShouldNotBe(new DateFormat(DateOrder.MonthDayYear, "vorm.", "nachm."));
	}

	[Fact]
	void Vocabularies_and_the_options_holding_them_compare_by_their_entries()
	{
		(int, string)[] entries = [.. Enumerable.Range(1, 12).Select(month => (month, $"m{(char)('a' + month)}"))];
		var built = new MonthNames(entries);
		var read = MonthNames.FromPacked(built.Packed);
		read.ShouldBe(built);
		read.Equals((object)built).ShouldBeTrue();
		read.GetHashCode().ShouldBe(built.GetHashCode());
		var reordered = new MonthNames([.. entries.Reverse()]);
		reordered.ShouldNotBe(built);
		built.Equals(null).ShouldBeFalse();

		var options = new DateOptions(DateShapes.CompactDate, 2029) { Months = built };
		var same = new DateOptions(DateShapes.CompactDate, 2029) { Months = read };
		same.ShouldBe(options);
		same.GetHashCode().ShouldBe(options.GetHashCode());
		(options with { Months = reordered }).ShouldNotBe(options);
		(options with { Months = null }).ShouldNotBe(options);
	}
}
