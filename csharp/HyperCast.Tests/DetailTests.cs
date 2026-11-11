using System.Globalization;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using HyperCast.Interop;

namespace HyperCast.Tests;

/// <summary>
/// The detail of a failure (the core's published vocabulary on a <see cref="Fault"/>), the 1.0
/// surface it was added beside, the decoding of a code nobody published, and the 1.1 layouts the
/// detailed exports read and write.
/// </summary>
public sealed class DetailTests
{
	// What a 1.0 caller wrote, verbatim: the positional constructor, the deconstruction, positional
	// and property patterns, `with`, equality, hashing and the rendered text. None of it names a detail.
	[Fact]
	void The_1_0_surface_of_a_fault_still_compiles_and_behaves_the_same()
	{
		var fault = new Fault(CastFailure.Malformed, 1, 2);
		var (reason, offset, length) = fault;
		(reason, offset, length).ShouldBe((CastFailure.Malformed, 1, 2));
		(fault is (CastFailure.Malformed, 1, 2)).ShouldBeTrue();
		(fault is { Reason: CastFailure.Malformed, Offset: 1, Length: 2 }).ShouldBeTrue();
		(fault with { Offset = 3 }).ShouldBe(new Fault(CastFailure.Malformed, 3, 2));
		fault.ToString().ShouldBe("Fault { Reason = Malformed, Offset = 1, Length = 2 }");
		(fault == new Fault(CastFailure.Malformed, 1, 2)).ShouldBeTrue();
		(fault != new Fault(CastFailure.Empty, 1, 2)).ShouldBeTrue();
		fault.GetHashCode().ShouldBe(new Fault(CastFailure.Malformed, 1, 2).GetHashCode());
		// A fault built the 1.0 way names no detail.
		(fault.DetailCode, fault.Detail).ShouldBe((0u, FaultDetail.None));

		// The union and its two cases, consumed as 1.0 consumed them.
		Verdict<int> verdict = fault;
		(verdict is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		verdict.Match(static s => "ok", static f => $"{f.Reason}").ShouldBe("Malformed");
		verdict.ToString().ShouldBe(new Verdict<int>(new Fault(CastFailure.Malformed, 1, 2)).ToString());
		var rejection = new Rejection<int>(fault);
		(rejection.TryGetValue(out Fault inside) && inside == fault).ShouldBeTrue();
		rejection.ToString().ShouldBe("Fault(Malformed @ 1+2)");

		// The 1.0 numeric format: both constructors, the deconstruction a positional record gives, `with`.
		var format = new NumFormat('.', ',', NumStyles.All);
		var (decimalSeparator, groupSeparator, styles) = format;
		(decimalSeparator, groupSeparator, styles).ShouldBe((".", ",", NumStyles.All));
		new NumFormat(".", ",", NumStyles.All, "$").CurrencySymbol.ShouldBe("$");
		new NumFormat(',', '.', NumStyles.All, "€").CurrencySymbol.ShouldBe("€");
		(format with { Styles = NumStyles.None }).Styles.ShouldBe(NumStyles.None);
		format.ShouldBe(new NumFormat(".", ",", NumStyles.All));
		format.GetHashCode().ShouldBe(new NumFormat(".", ",", NumStyles.All).GetHashCode());
	}

	[Fact]
	void Two_faults_that_differ_only_in_detail_are_equal_and_hash_alike()
	{
		var plain = new Fault(CastFailure.Malformed, 4, 1);
		var named = plain with { DetailCode = (uint)FaultDetail.UnexpectedCharacter };
		var other = plain with { DetailCode = (uint)FaultDetail.DigitExpected };
		named.Detail.ShouldBe(FaultDetail.UnexpectedCharacter);
		(plain == named).ShouldBeTrue();
		(named == other).ShouldBeTrue();
		named.Equals((object)other).ShouldBeTrue();
		named.GetHashCode().ShouldBe(other.GetHashCode());
		new HashSet<Fault> { plain, named, other }.Count.ShouldBe(1);
		named.ToString().ShouldBe(plain.ToString());
		// And so are the verdicts that hold them.
		new Verdict<int>(named).ShouldBe(new Verdict<int>(other));
		new CheckedVerdict<int>(new Rejection<int>(named)).ShouldBe(new CheckedVerdict<int>(new Rejection<int>(other)));
		// The real doors agree: one failure through the byte door, however it is named.
		Cast.Int32("12a"u8, NumFormat.Invariant).ShouldBe(new Verdict<int>(new Fault(CastFailure.Malformed, 2, 1)));
	}

	[Fact]
	void A_detail_survives_the_utf16_remap_and_the_checked_doors()
	{
		// "€x": the byte door spans the euro as three bytes, the char door as one unit; both name the detail.
		(Cast.Int32("€x"u8, NumFormat.Invariant) is Fault { Offset: 0, Length: 3, Detail: FaultDetail.UnexpectedCharacter, DetailCode: 2 }).ShouldBeTrue();
		(Cast.Int32("€x", NumFormat.Invariant) is Fault { Offset: 0, Length: 1, Detail: FaultDetail.UnexpectedCharacter, DetailCode: 2 }).ShouldBeTrue();
		(Cast.Date("2026-01-07T"u8) is Fault { Detail: FaultDetail.WrongLength }).ShouldBeTrue();
		var checkedDoor = Cast.Int32("€x", NumFormat.Invariant, new Constraints<int>(maximum: 10));
		(checkedDoor.TryGetValue(out Rejection<int> rejection) && rejection.TryGetValue(out Fault fault)
			&& fault is { Offset: 0, Length: 1, Detail: FaultDetail.UnexpectedCharacter }).ShouldBeTrue();
		// A violation is no fault, and has no detail to name: it carries the value as cast.
		var violation = Cast.Int32("11"u8, NumFormat.Invariant, new Constraints<int>(maximum: 10));
		(violation.TryGetValue(out Rejection<int> broke) && broke.TryGetValue(out Violation<int> value) && value.Value == 11).ShouldBeTrue();
		// The typed doors name theirs too, with an empty span.
		(Cast.DecimalFromDouble(double.NaN) is Fault { Reason: CastFailure.Malformed, Offset: 0, Length: 0, Detail: FaultDetail.NotFinite }).ShouldBeTrue();
		(Cast.DecimalFromDouble(double.PositiveInfinity) is Fault { Reason: CastFailure.OutOfRange, Detail: FaultDetail.NotFinite }).ShouldBeTrue();
		(Cast.ExcelTime(-1) is Fault { Detail: FaultDetail.NegativeNotAllowed }).ShouldBeTrue();
		(Cast.ExcelDuration(double.NaN) is Fault { Detail: FaultDetail.NotFinite }).ShouldBeTrue();
		(Cast.ExcelSerialFromDouble(60, ExcelEpoch.Y1900) is Fault { Detail: FaultDetail.NonexistentDate }).ShouldBeTrue();
		// A scalar the core accepts and a char cannot hold is out of range for the target type.
		(Cast.Char("U+1F600"u8) is Fault { Reason: CastFailure.OutOfRange, Detail: FaultDetail.TooLarge, DetailCode: 25 }).ShouldBeTrue();
		(Cast.Char("U+110000"u8) is Fault { Reason: CastFailure.OutOfRange, Detail: FaultDetail.NotAScalar }).ShouldBeTrue();
	}

	[Fact]
	void A_code_nobody_published_decodes_without_throwing_and_keeps_its_reason_and_span()
	{
		// A fault as a newer core would write it: a code past the published 32.
		var constructed = new RawFaultDetail { Size = RawFaultDetail.SizeInBytes, Offset = 5, Length = 3, Detail = 999 };
		var fault = Abi.ToFault(2, constructed);
		fault.Reason.ShouldBe(CastFailure.Malformed);
		(fault.Offset, fault.Length).ShouldBe((5, 3));
		fault.DetailCode.ShouldBe(999u);
		fault.Detail.ShouldBe(FaultDetail.Unknown);
		fault.ToString().ShouldBe("Fault { Reason = Malformed, Offset = 5, Length = 3 }");
		// Usable as any fault is: in a verdict, matched, equal to the same span.
		Verdict<int> verdict = fault;
		(verdict is Fault { Reason: CastFailure.Malformed, Offset: 5, Length: 3, Detail: FaultDetail.Unknown }).ShouldBeTrue();
		(fault == new Fault(CastFailure.Malformed, 5, 3)).ShouldBeTrue();
		new CheckedVerdict<int>(new Rejection<int>(fault)).TryGetValue(out Rejection<int> _).ShouldBeTrue();
		// Every code past the vocabulary, the edges included.
		foreach (var code in new uint[] { 33, 100, 255, 256, 999, 65_536, uint.MaxValue })
		{
			Abi.DetailFrom(code).ShouldBe(FaultDetail.Unknown, $"{code}");
			Abi.ToFault(3, new RawFaultDetail { Size = 16, Detail = code }).DetailCode.ShouldBe(code);
		}
		Abi.DetailFrom(0).ShouldBe(FaultDetail.None);
		// And a reason the core never writes is still the binding bug it was.
		Should.Throw<InvalidOperationException>(() => Abi.ToFault(0, constructed));
		Should.Throw<InvalidOperationException>(() => Abi.ToFault(9, constructed));
	}

	[Fact]
	void Every_published_code_has_its_name()
	{
		var names = Enum.GetValues<FaultDetail>().Where(static d => d is not (FaultDetail.None or FaultDetail.Unknown)).ToArray();
		names.Length.ShouldBe(32);
		for (var i = 0; i < names.Length; i++)
		{
			((uint)names[i]).ShouldBe((uint)i + 1);
			Abi.DetailFrom((uint)i + 1).ShouldBe(names[i]);
		}
		((byte)FaultDetail.Unknown).ShouldBe((byte)255);
	}

	// include/hypercast.h, or null where the tree holds none (the musl suite): the literals alone then.
	static readonly string? _header = FindHeader() is { } path ? File.ReadAllText(path) : null;

	static string? FindHeader()
	{
		for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
		{
			var header = Path.Combine(dir.FullName, "include", "hypercast.h");
			if (File.Exists(header))
				return header;
		}
		return null;
	}

	static uint Define(string name, uint restated)
	{
		if (_header is null)
			return restated;
		// A plain number, or the header's own shift spelling of a policy: `(2 << 8)`.
		var match = System.Text.RegularExpressions.Regex.Match(_header, $@"^#define {name} (?:\((\d+) << (\d+)\)|(\d+))$",
			System.Text.RegularExpressions.RegexOptions.Multiline);
		match.Success.ShouldBeTrue($"{name} is not in include/hypercast.h");
		var value = match.Groups[3].Success
			? uint.Parse(match.Groups[3].Value, CultureInfo.InvariantCulture)
			: uint.Parse(match.Groups[1].Value, CultureInfo.InvariantCulture) << int.Parse(match.Groups[2].Value, CultureInfo.InvariantCulture);
		value.ShouldBe(restated, name);
		return restated;
	}

	[Fact]
	void The_detailed_layouts_are_the_headers()
	{
		((uint)Unsafe.SizeOf<RawFaultDetail>()).ShouldBe(Define("HyperCastRawFaultDetail_SIZE", 16));
		RawFaultDetail.SizeInBytes.ShouldBe(16u);
		new[] { "Size", "Offset", "Length", "Detail" }.Select(static f => (int)Marshal.OffsetOf<RawFaultDetail>(f)).ShouldBe([0, 4, 8, 12]);
		RawFaultDetail.Empty.Size.ShouldBe(16u);
		((uint)Unsafe.SizeOf<RawDateOptions>()).ShouldBe(Define("HyperCastRawDateOptions_SIZE", 12));
		new[] { "Size", "Flags", "Pivot" }.Select(static f => (int)Marshal.OffsetOf<RawDateOptions>(f)).ShouldBe([0, 4, 8]);
		// The grouping policy: the styles in the low byte, Indian in bit 9, Western setting no bit.
		((uint)NumGrouping.Western).ShouldBe(Define("HyperCastNumFormat_GROUPING_WESTERN", 0));
		((uint)NumGrouping.Indian).ShouldBe(Define("HyperCastNumFormat_GROUPING_INDIAN", 2u << 8));
		NumFormat.Invariant.ToRaw().Flags.ShouldBe((uint)NumStyles.All);
		(NumFormat.Invariant with { Grouping = NumGrouping.Indian }).ToRaw().Flags
			.ShouldBe((uint)NumStyles.All | 2u << 8);
		Define("HyperCastNumFormat_GROUPING_POLICY", 3u << 8).ShouldBe(3u << 8);
	}

	[Fact]
	void Date_options_pack_into_the_cores_layout()
	{
		new DateOptions().TryToRaw(out _).ShouldBeFalse();
		default(DateOptions).TryToRaw(out _).ShouldBeFalse();
		DateOptions.CommonForms.TryToRaw(out var common).ShouldBeTrue();
		(common.Size, common.Flags, common.Pivot).ShouldBe((12u, 7u, 0u));
		new DateOptions(DateShapes.None, 2029).TryToRaw(out var pivot).ShouldBeTrue();
		(pivot.Flags, pivot.Pivot).ShouldBe((0u, 2029u));
		DateOptions.CommonForms.Shapes.ShouldBe(DateShapes.CompactDate | DateShapes.SpaceSeparator | DateShapes.HourOffset);
		DateOptions.CommonForms.Pivot.ShouldBe(0);
		DateOptions.CommonForms.Months.ShouldBeNull();
		// The edges of the window and the shapes the core would refuse.
		new DateOptions(DateShapes.None, 100).TryToRaw(out _).ShouldBeTrue();
		new DateOptions(DateShapes.None, 9999).TryToRaw(out _).ShouldBeTrue();
		foreach (var pivotYear in new[] { 1, 99, 10_000, -1, int.MinValue })
			Should.Throw<ArgumentOutOfRangeException>(() => new DateOptions(DateShapes.None, pivotYear).TryToRaw(out _));
		foreach (var bits in new uint[] { 8, 16, 0x8000_0000, uint.MaxValue })
			Should.Throw<ArgumentException>(() => new DateOptions((DateShapes)bits).TryToRaw(out _));
	}

	[Fact]
	void A_vocabulary_packs_into_the_cores_layout_and_reads_back()
	{
		var names = new MonthNames(
			(1, "Jan"), (2, "Feb"), (3, "Mar"), (4, "Apr"), (5, "May"), (6, "Jun"),
			(7, "Jul"), (8, "Aug"), (9, "Sep"), (9, "Sept"), (10, "Oct"), (11, "Nov"), (12, "Dec"));
		// A month byte, a little-endian length, the UTF-8 bytes, no padding.
		names.Packed[..8].ToArray().ShouldBe([1, 3, 0, 0, 0, (byte)'J', (byte)'a', (byte)'n']);
		names.Packed.Length.ShouldBe(12 * 8 + 9);
		MonthNames.FromPacked(names.Packed).Entries.ShouldBe(names.Entries);
		names.Entries[9].ShouldBe((9, "Sept"));
		new MonthNames(names.Entries).Packed.ToArray().ShouldBe(names.Packed.ToArray());
		// A name past one byte of length and one past the ASCII are measured in bytes.
		var wide = new MonthNames([(1, "janvier"), (2, "février"), (3, "mars"), (4, "avril"), (5, "mai"), (6, "juin"),
			(7, "juillet"), (8, "août"), (9, "septembre"), (10, "octobre"), (11, "novembre"), (12, "décembre")]);
		wide.Packed[12..21].ToArray().ShouldBe([2, 8, 0, 0, 0, (byte)'f', 0xC3, 0xA9, (byte)'v']);
		Should.Throw<ArgumentNullException>(() => new MonthNames((IEnumerable<(int, string)>)null!));
		Should.Throw<ArgumentException>(() => new MonthNames([(1, "a"), (2, null!)])).ParamName.ShouldBe("entries");
	}

	[Fact]
	void A_repeat_only_the_cores_folding_sees_is_still_refused()
	{
		var months = Enumerable.Range(2, 11).Select(static m => (m, "m" + (char)('a' + m))).ToList();
		// Final and medial sigma are different lower-case letters to the invariant culture and one to the core's simple folding.
		"aς".ToLowerInvariant().ShouldNotBe("aσ".ToLowerInvariant());
		var thrown = Should.Throw<ArgumentException>(() => new MonthNames([(1, "aσ"), (1, "aς"), .. months]));
		thrown.Message.ShouldContain("repeats an earlier name");
		thrown.ParamName.ShouldBe("entries");
	}

	[Fact]
	void A_vocabulary_alone_opens_month_names_and_nothing_else()
	{
		var english = new MonthNames(
			(1, "Jan"), (2, "Feb"), (3, "Mar"), (4, "Apr"), (5, "May"), (6, "Jun"),
			(7, "Jul"), (8, "Aug"), (9, "Sep"), (10, "Oct"), (11, "Nov"), (12, "Dec"));
		var months = new DateOptions { Months = english };
		(Cast.Date("7-Jan-2026"u8, DateOrder.DayMonthYear, months) is Success<DateOnly> { Value: { Year: 2026, Month: 1, Day: 7 } }).ShouldBeTrue();
		(Cast.Date("7-Jan-26"u8, DateOrder.DayMonthYear, months) is Fault { Detail: FaultDetail.FieldWidth }).ShouldBeTrue();
		(Cast.Date("20260107"u8, months) is Fault { Detail: FaultDetail.WrongLength }).ShouldBeTrue();
		// The pivot opens two-digit years: the window ends at 2029, so 26 is 2026 and 30 is 1930.
		var both = new DateOptions(DateShapes.None, 2029) { Months = english };
		(Cast.Date("07-Jan-26"u8, DateOrder.DayMonthYear, both) is Success<DateOnly> { Value.Year: 2026 }).ShouldBeTrue();
		(Cast.Date("07-Jan-30"u8, DateOrder.DayMonthYear, both) is Success<DateOnly> { Value.Year: 1930 }).ShouldBeTrue();
		(Cast.DateTime("07-Jan-26 15:04"u8, DateOrder.DayMonthYear, both) is Success<DateTime> { Value: { Year: 2026, Hour: 15 } }).ShouldBeTrue();
		(Cast.DateTime("07-Jan-26 3:04 PM"u8, new DateFormat(DateOrder.DayMonthYear), both) is Success<DateTime> { Value: { Year: 2026, Hour: 15 } }).ShouldBeTrue();
		// Declared names, with a dotted one: janv. needs its own '.' and the separator after it.
		var french = new DateOptions(DateShapes.None, 2029)
		{
			Months = new MonthNames([(1, "janv."), (2, "févr."), (3, "mars"), (4, "avr."), (5, "mai"), (6, "juin"),
				(7, "juil."), (8, "août"), (9, "sept."), (10, "oct."), (11, "nov."), (12, "déc.")]),
		};
		(Cast.Date("7.janv..26"u8, DateOrder.DayMonthYear, french) is Success<DateOnly> { Value: { Year: 2026, Month: 1, Day: 7 } }).ShouldBeTrue();
		(Cast.Date("7.janv.26"u8, DateOrder.DayMonthYear, french) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
	}

	[Fact]
	void The_common_forms_open_exactly_their_three_shapes()
	{
		var common = DateOptions.CommonForms;
		(Cast.Date("20260107"u8, common) is Success<DateOnly> { Value: { Year: 2026, Month: 1, Day: 7 } }).ShouldBeTrue();
		(Cast.Date("20260107"u8) is Fault { Detail: FaultDetail.WrongLength }).ShouldBeTrue();
		(Cast.Timestamp("2026-01-07 15:04:05Z"u8, common) is Success<DateTimeOffset>).ShouldBeTrue();
		(Cast.Timestamp("2026-01-07T15:04:05+00"u8, common) is Success<DateTimeOffset>).ShouldBeTrue();
		(Cast.Timestamp("2026-01-07 15:04:05Z"u8) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		(Cast.Timestamp("2026-01-07T15:04:05+00"u8) is Fault { Reason: CastFailure.Malformed }).ShouldBeTrue();
		// Each alone opens one, and the others stay shut.
		(Cast.Timestamp("2026-01-07 15:04:05Z"u8, new DateOptions(DateShapes.SpaceSeparator)) is Success<DateTimeOffset>).ShouldBeTrue();
		(Cast.Timestamp("2026-01-07T15:04:05+00"u8, new DateOptions(DateShapes.SpaceSeparator)) is Fault).ShouldBeTrue();
		(Cast.Timestamp("2026-01-07T15:04:05-05"u8, new DateOptions(DateShapes.HourOffset)) is Success<DateTimeOffset> { Value.Hour: 20 }).ShouldBeTrue();
		// Irrelevant options are ignored, never refused: the commons at the ordered date door.
		(Cast.Date("7/1/2026"u8, DateOrder.DayMonthYear, common) is Success<DateOnly> { Value.Month: 1 }).ShouldBeTrue();
		// The UTF-16 doors re-express a fault's span in code units and keep the detail.
		(Cast.Date("€20260107", common) is Fault { Offset: 0, Length: 9, Detail: FaultDetail.WrongLength }).ShouldBeTrue();
		(Cast.Date("20260107", common) is Success<DateOnly>).ShouldBeTrue();
		(Cast.Timestamp("2026-01-07 15:04:05Z", common) is Success<DateTimeOffset>).ShouldBeTrue();
		(Cast.DateTime("2026-01-07 15:04", DateOrder.YearMonthDay, common) is Success<DateTime>).ShouldBeTrue();
		(Cast.DateTime("2026-01-07 15:04", new DateFormat(DateOrder.YearMonthDay), common) is Success<DateTime>).ShouldBeTrue();
		(Cast.Date("7/1/26", DateOrder.DayMonthYear, new DateOptions(DateShapes.None, 2029)) is Success<DateOnly> { Value.Year: 2026 }).ShouldBeTrue();
		// An undefined shape or a pivot outside its range is a caller bug, at every door.
		Should.Throw<ArgumentException>(() => Cast.Date("2026-01-07"u8, new DateOptions((DateShapes)8)));
		Should.Throw<ArgumentOutOfRangeException>(() => Cast.Timestamp("2026-01-07T00:00:00Z", new DateOptions(DateShapes.None, 50)));
		Should.Throw<ArgumentOutOfRangeException>(() => Cast.DateTime("2026-01-07", DateOrder.YearMonthDay, new DateOptions(DateShapes.None, 20_000)));
		Should.Throw<ArgumentOutOfRangeException>(() => Cast.Date("2026-01-07"u8, DateOrder.Unspecified, default)).ParamName.ShouldBe("order");
	}
}
