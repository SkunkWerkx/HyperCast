// Proves the binding under Native AOT for real: this program publishes with PublishAot and
// crosses every native entry point the binding declares — the twenty-six cast_* functions and
// hypercast_version — against the real native library, including the generic door and the
// union's compile-checked consumption. Exit code 0 only if every cast lands as expected.
//
// The body lives here rather than in Program.cs so the apps that run it can call it: Program.cs
// is the console entry point, and HyperCast.AppleSmokeTest and HyperCast.AndroidSmokeTest
// compile this file too. An Android app is compiled as a library, where top-level statements
// are an error (CS8805).

namespace HyperCast.AotSmokeTest;

internal static class SmokeTest
{
	internal static int Run()
	{
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
		Check("char", Cast.Char("U+00E9"), 'é');
		Check("i8", Cast.SByte("-128", NumFormat.Invariant), (sbyte)-128);
		Check("i16", Cast.Int16("0x7FFF", NumFormat.Invariant), (short)32767);
		Check("i32", Cast.Int32("(1,234)", NumFormat.Invariant), -1234);
		Check("i64", Cast.Int64("1e3", NumFormat.Invariant), 1000L);
		Check("u8", Cast.Byte("255", NumFormat.Invariant), (byte)255);
		Check("u16", Cast.UInt16("65535", NumFormat.Invariant), (ushort)65535);
		Check("u32", Cast.UInt32("4294967295", NumFormat.Invariant), 4294967295u);
		Check("u64", Cast.UInt64("18446744073709551615", NumFormat.Invariant), ulong.MaxValue);
		Check("f32", Cast.Single("2.5", NumFormat.Invariant), 2.5f);
		Check("f64", Cast.Double("25.5%", NumFormat.Invariant), 0.255);
		Check("decimal", Cast.Decimal("(1,234.50)", NumFormat.Invariant), -1234.50m);
		Check("currency", Cast.Int32("($1,234)", new NumFormat('.', ',', NumStyles.All, "$")), -1234);
		// The generic door's typeof dispatch has to fold under AOT's shared generics too.
		Check("generic", Cast.Numeric<short>("0x7FFF", NumFormat.Invariant), (short)32767);
		// ...and Scalar's wider table, through non-numeric instantiations.
		Check("scalar uuid", Cast.Scalar<Guid>("{01020304-0506-0708-090a-0b0c0d0e0f10}", NumFormat.Invariant),
			new Guid("01020304-0506-0708-090a-0b0c0d0e0f10"));
		Check("scalar char", Cast.Scalar<char>("&#x41;", NumFormat.Invariant), 'A');
		Check("scalar datetime", Cast.Scalar<DateTime>("2026-01-02T15:04:05+05:00", NumFormat.Invariant),
			new DateTime(2026, 1, 2, 10, 4, 5, DateTimeKind.Utc));
		Check("uuid", Cast.Uuid("urn:uuid:01020304-0506-0708-090a-0b0c0d0e0f10"),
			new Guid("01020304-0506-0708-090a-0b0c0d0e0f10"));
		Check("timestamp", Cast.Timestamp("2026-01-02T15:04:05+05:00"),
			new DateTimeOffset(2026, 1, 2, 10, 4, 5, TimeSpan.Zero));
		Check("unix", Cast.Unix("1700000000123", UnixPrecision.Milliseconds),
			DateTimeOffset.FromUnixTimeMilliseconds(1_700_000_000_123));
		Check("excel serial", Cast.ExcelSerial("45292.75", ExcelEpoch.Y1900),
			new DateTimeOffset(2024, 1, 1, 18, 0, 0, TimeSpan.Zero));
		Check("date", Cast.Date("2026-01-02"), new DateOnly(2026, 1, 2));
		Check("date (ordered)", Cast.Date("1/7/2026", DateOrder.MonthDayYear), new DateOnly(2026, 1, 7));
		Check("datetime", Cast.DateTime("1/7/2026 3:04 PM", DateOrder.MonthDayYear),
			new DateTime(2026, 1, 7, 15, 4, 0));
		Check("time", Cast.Time("15:04:05"), new TimeOnly(15, 4, 5));
		Check("duration", Cast.Duration("P1DT6H"), new TimeSpan(1, 6, 0, 0));
		// The typed doors take the double a workbook stores, not text.
		Check("decimal from double", Cast.DecimalFromDouble(0.1 + 0.2), 0.30000000000000004m);
		Check("excel serial from double", Cast.ExcelSerialFromDouble(45292.75, ExcelEpoch.Y1900),
			new DateTime(2024, 1, 1, 18, 0, 0));
		Check("excel time", Cast.ExcelTime(0.75), new TimeOnly(18, 0));
		Check("excel duration", Cast.ExcelDuration(1.5), new TimeSpan(1, 12, 0, 0));
		// The UTF-8 doors are the native contract itself — no transcode in front of the crossing.
		Check("utf8", Cast.Int32("(1,234)"u8, NumFormat.Invariant), -1234);

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
	}
}
