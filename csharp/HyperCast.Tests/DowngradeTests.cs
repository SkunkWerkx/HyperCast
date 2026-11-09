using System.Reflection;
using System.Runtime.InteropServices;

namespace HyperCast.Tests;

/// <summary>
/// The downgrade guard: a grouping policy against a 1.0 core is rejected, never parsed with
/// 1.0's rules. A 1.0 core ignores the policy bits of a format and has no detailed export, and every
/// door of this binding calls one, so a door on it throws and <see cref="Cast.IsAvailable"/> says no.
/// </summary>
/// <remarks>
/// Runs only when <c>HYPERCAST_V10_LIBRARY</c> names a 1.0 core (a build of the v1.0.0 tag), and then alone:
/// it redirects the binding's library before any door has bound, which the rest of the suite would have
/// done first. <c>HYPERCAST_V10_LIBRARY=… dotnet test -f net10.0 --filter-class '*DowngradeTests'</c>.
/// </remarks>
public sealed class DowngradeTests
{
	[Fact]
	void A_1_0_core_is_rejected_and_never_parses_a_request_as_1_0_did()
	{
		var path = Environment.GetEnvironmentVariable("HYPERCAST_V10_LIBRARY");
		Assert.SkipUnless(!string.IsNullOrEmpty(path), "HYPERCAST_V10_LIBRARY names no 1.0 core.");
		NativeLibrary.SetDllImportResolver(typeof(Cast).Assembly,
			(name, _, _) => name == "hypercast" ? NativeLibrary.Load(path!) : IntPtr.Zero);

		// The library that answered is the 1.0 one: it loads and reports a version, but has none of the 1.1
		// doors. (Its stamp, 1.0.0, is the development core's too, so the exports are what tell them apart.)
		var library = NativeLibrary.Load(path!);
		foreach (var door in new[] { "cast_detailed", "cast_detailed_from_f64", "cast_checked_detailed", "cast_checked_detailed_from_f64" })
			NativeLibrary.TryGetExport(library, door, out _).ShouldBeFalse($"{door} is in the library; not a 1.0 core");
		NativeLibrary.TryGetExport(library, "cast_i32", out _).ShouldBeTrue();
		Cast.NativeVersion.ShouldNotBeNull("the redirect did not take; run this test alone");
		// ...and the guard turns it away.
		Cast.IsAvailable.ShouldBeFalse();

		// A request is refused: no door answers, least of all with 1.0's reading of "12,34" as 1234.
		Should.Throw<EntryPointNotFoundException>(() => Cast.Int32("12,34"u8, NumFormat.Invariant));
		Should.Throw<EntryPointNotFoundException>(() => Cast.Int32("12,34", NumFormat.Invariant));
		Should.Throw<EntryPointNotFoundException>(() => Cast.Double("12,34", new NumFormat('.', ',', NumStyles.All, NumGrouping.Indian)));
		Should.Throw<EntryPointNotFoundException>(() => Cast.Int32("12,34"u8, NumFormat.Invariant, new Constraints<int>(maximum: 100_000)));
		// So is every other door: nothing reaches a 1.0 export.
		Should.Throw<EntryPointNotFoundException>(() => Cast.Boolean("true"u8));
		Should.Throw<EntryPointNotFoundException>(() => Cast.Date("2026-01-07"u8));
		Should.Throw<EntryPointNotFoundException>(() => Cast.Date("20260107"u8, DateOptions.CommonForms));
		Should.Throw<EntryPointNotFoundException>(() => Cast.DecimalFromDouble(1.5));
	}
}
