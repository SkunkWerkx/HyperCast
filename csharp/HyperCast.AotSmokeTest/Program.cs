// Proves the binding under Native AOT for real: this program publishes with PublishAot and
// exercises a door from every family — including the union's compile-checked consumption —
// against the real native library. Exit code 0 only if every cast lands as expected.

using HyperCast;

var failures = 0;

void Check<T>(string name, Verdict<T> verdict, T expected) where T : struct
{
	// TryGetValue consumption (generic context — see CorpusTests for why not case patterns).
	if (verdict.TryGetValue(out Success<T> success) && success.Value.Equals(expected))
	{
		Console.WriteLine($"ok   {name} = {success.Value}");
		return;
	}
	Console.WriteLine($"FAIL {name}: {verdict} (expected {expected})");
	failures++;
}

Check("bool", Cast.Boolean("enabled"), true);
Check("i32", Cast.Int32("(1,234)", NumFormat.Invariant), -1234);
Check("f64", Cast.Double("25.5%", NumFormat.Invariant), 0.255);
Check("decimal", Cast.Decimal("(1,234.50)", NumFormat.Invariant), -1234.50m);
Check("currency", Cast.Int32("($1,234)", new NumFormat('.', ',', NumStyles.All, "$")), -1234);
Check("generic", Cast.Numeric<short>("0x7FFF", NumFormat.Invariant), (short)32767);
Check("uuid", Cast.Uuid("urn:uuid:01020304-0506-0708-090a-0b0c0d0e0f10"),
	new Guid("01020304-0506-0708-090a-0b0c0d0e0f10"));
Check("timestamp", Cast.Timestamp("2026-01-02T15:04:05+05:00"),
	new DateTimeOffset(2026, 1, 2, 10, 4, 5, TimeSpan.Zero));
Check("date", Cast.Date("2026-01-02"), new DateOnly(2026, 1, 2));
Check("time", Cast.Time("15:04:05"), new TimeOnly(15, 4, 5));
Check("duration", Cast.Duration("P1DT6H"), new TimeSpan(1, 6, 0, 0));

// The exhaustive two-arm switch (concrete case types) must survive AOT too.
var disposition = Cast.Int32("not-a-number", NumFormat.Invariant) switch
{
	Success<int> s => $"unexpected ok {s.Value}",
	Fault f => $"fault {f.Reason} @ {f.Offset}+{f.Length}",
};
Console.WriteLine($"union switch: {disposition}");

// The probe must answer before any door is the thing that finds out, and it must name the
// core it actually loaded — under AOT too, where a stale runtimes/ asset is the classic slip.
Console.WriteLine($"native: available={Cast.IsAvailable} version={Cast.NativeVersion}");
if (!Cast.IsAvailable || Cast.NativeVersion is null)
	failures++;
if (!disposition.StartsWith("fault Malformed", StringComparison.Ordinal))
	failures++;

Console.WriteLine(failures == 0 ? "AOT smoke test passed." : $"AOT smoke test FAILED ({failures}).");
return failures == 0 ? 0 : 1;
