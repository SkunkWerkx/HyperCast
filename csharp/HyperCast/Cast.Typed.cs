namespace HyperCast;

public static partial class Cast
{
	/// <summary>
	/// Reads a number a caller already holds — the <see cref="double"/> a workbook stores for a
	/// numeric cell — as the exact <see cref="decimal"/> it names: the shortest decimal that
	/// rounds back to the double, the digits a spreadsheet writes for it. So <c>0.1</c> is one
	/// tenth, not the binary fraction nearest it, and <c>0.1 + 0.2</c> is
	/// <c>0.30000000000000004</c> — where <c>(decimal)double</c> rounds to 15 significant
	/// digits and drops the rest. The result is canonical, as <see cref="Decimal(ReadOnlySpan{byte}, NumFormat)"/>'s
	/// is. <see cref="double.NaN"/> is <see cref="CastFailure.Malformed"/>; an infinity, a
	/// magnitude past <see cref="decimal.MaxValue"/> or more than 28 places is
	/// <see cref="CastFailure.OutOfRange"/> — the digits are never cut to fit. A typed door's
	/// <see cref="Fault"/> has no span: its offset and length are 0.
	/// </summary>
	/// <param name="value">The number to read.</param>
	public static unsafe Verdict<decimal> DecimalFromDouble(double value)
	{
		RawDecimal raw = default;
		RawFault fault = default;
		var code = cast_decimal_from_f64(value, &raw, &fault);
		return code == 0
			? new decimal((int)raw.Lo, (int)(raw.Lo >> 32), (int)raw.Hi, raw.Negative != 0, raw.Scale)
			: Failed<decimal>(code, fault);
	}

	/// <summary>
	/// Reads an Excel date serial a caller already holds as a <see cref="double"/> under a
	/// declared <see cref="ExcelEpoch"/> — the twin of
	/// <see cref="ExcelSerial(ReadOnlySpan{byte}, ExcelEpoch)"/> for a workbook reader that has
	/// the cell's number and no text. The result is the zone-less wall clock the cell holds, a
	/// <see cref="System.DateTime"/> of <see cref="DateTimeKind.Unspecified"/> as
	/// <see cref="DateTime(ReadOnlySpan{byte}, DateOrder)"/> returns; the fraction is rounded to
	/// the nearest nanosecond, then sub-tick nanoseconds truncate. The 1900 system's phantom
	/// serial <c>60</c>, a serial below the system's first day and one past 9999-12-31 are
	/// <see cref="CastFailure.OutOfRange"/>; a negative, NaN or infinite serial is
	/// <see cref="CastFailure.Malformed"/>.
	/// </summary>
	/// <param name="value">The serial.</param>
	/// <param name="epoch">The declared date system. Never <see cref="ExcelEpoch.Unspecified"/>.</param>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="epoch"/> is undefined — a caller bug, not a data verdict.</exception>
	public static unsafe Verdict<System.DateTime> ExcelSerialFromDouble(double value, ExcelEpoch epoch)
	{
		GuardEpoch(epoch);
		RawCivil raw = default;
		RawFault fault = default;
		var code = cast_excel_serial_from_f64(value, (uint)epoch, &raw, &fault);
		return code == 0
			? new System.DateTime(raw.Year, raw.Month, raw.Day, 0, 0, 0, DateTimeKind.Unspecified)
				.AddTicks((long)(raw.NanosOfDay / 100))
			: Failed<System.DateTime>(code, fault);
	}

	/// <summary>
	/// Reads the fraction of an Excel serial a caller already holds as a <see cref="double"/>
	/// as a <see cref="TimeOnly"/>: <c>0.75</c> and <c>45292.75</c> are both 18:00. Rounded to
	/// the nearest nanosecond, a fraction that rounds to a whole day is midnight, and sub-tick
	/// nanoseconds truncate. A negative, NaN or infinite serial is
	/// <see cref="CastFailure.Malformed"/>; one past 9999-12-31 is
	/// <see cref="CastFailure.OutOfRange"/>.
	/// </summary>
	/// <param name="value">The serial.</param>
	public static unsafe Verdict<TimeOnly> ExcelTime(double value)
	{
		ulong nanos = 0;
		RawFault fault = default;
		var code = cast_excel_time(value, &nanos, &fault);
		return code == 0 ? new TimeOnly((long)(nanos / 100)) : Failed<TimeOnly>(code, fault);
	}

	/// <summary>
	/// Reads a number of days a caller already holds as a <see cref="double"/> — what an
	/// elapsed-time format (<c>[h]:mm:ss</c>) stores — as a <see cref="TimeSpan"/>: <c>1.5</c>
	/// is a day and twelve hours, and a negative span is negative. Rounded to the nearest
	/// nanosecond, then sub-tick nanoseconds truncate. NaN or an infinity is
	/// <see cref="CastFailure.Malformed"/>; beyond ±10,000 years is
	/// <see cref="CastFailure.OutOfRange"/>.
	/// </summary>
	/// <param name="value">The number of days.</param>
	public static unsafe Verdict<TimeSpan> ExcelDuration(double value)
	{
		RawDuration raw = default;
		RawFault fault = default;
		var code = cast_excel_duration(value, &raw, &fault);
		return code == 0
			? new TimeSpan(raw.Seconds * TimeSpan.TicksPerSecond + raw.Nanos / 100)
			: Failed<TimeSpan>(code, fault);
	}
}
