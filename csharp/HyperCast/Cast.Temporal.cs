namespace HyperCast;

public static partial class Cast
{
	/// <summary>
	/// Casts UUID text to a <see cref="Guid"/>: every format .NET's own
	/// <see cref="Guid.TryParse(ReadOnlySpan{char}, out Guid)"/> accepts (D, N, B, P, X),
	/// after stripping a case-insensitive <c>urn:uuid:</c>/<c>GUID:</c>/<c>UUID:</c> prefix.
	/// Culture-insensitive.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	public static unsafe Verdict<Guid> Uuid(ReadOnlySpan<byte> utf8)
	{
		Span<byte> bytes = stackalloc byte[16];
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
		fixed (byte* value = bytes)
			code = cast_uuid(ptr, (nuint)utf8.Length, value, &fault);
		return code == 0 ? new Guid(bytes, bigEndian: true) : Failed<Guid>(code, fault);
	}

	/// <inheritdoc cref="Uuid(ReadOnlySpan{byte})"/>
	/// <param name="input">The raw scalar text.</param>
	public static Verdict<Guid> Uuid(ReadOnlySpan<char> input)
	{
		byte[]? rented = null;
		try
		{
			return Uuid(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented));
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <summary>
	/// Casts an RFC 3339 instant — <c>yyyy-MM-ddTHH:mm:ss[.f{1..9}](Z|±hh:mm)</c>, zone
	/// <b>mandatory</b> — to a UTC <see cref="DateTimeOffset"/>. A zone-less or
	/// space-separated form is <see cref="CastFailure.Malformed"/>; an instant outside
	/// 0001-01-01 to 9999-12-31 UTC is <see cref="CastFailure.OutOfRange"/>. The tenth
	/// fractional digit onward has no .NET representation and the core rejects it; the
	/// eighth and ninth (sub-tick nanoseconds) truncate to ticks.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	public static unsafe Verdict<DateTimeOffset> Timestamp(ReadOnlySpan<byte> utf8)
	{
		RawTimestamp value = default;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_timestamp(ptr, (nuint)utf8.Length, &value, &fault);
		return code == 0 ? ToDateTimeOffset(value) : Failed<DateTimeOffset>(code, fault);
	}

	/// <inheritdoc cref="Timestamp(ReadOnlySpan{byte})"/>
	/// <param name="input">The raw scalar text.</param>
	public static Verdict<DateTimeOffset> Timestamp(ReadOnlySpan<char> input)
	{
		byte[]? rented = null;
		try
		{
			return Timestamp(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented));
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <summary>
	/// Casts an integer Unix-epoch value under a caller-declared unit to a UTC
	/// <see cref="DateTimeOffset"/>. Negatives (pre-1970) are allowed; a fractional or
	/// non-integer value is <see cref="CastFailure.Malformed"/>; outside the 0001–9999
	/// window is <see cref="CastFailure.OutOfRange"/>. Sub-tick nanoseconds truncate.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="precision">The declared unit. Never <see cref="UnixPrecision.Unspecified"/>.</param>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="precision"/> is undefined — a caller bug, not a data verdict.</exception>
	public static unsafe Verdict<DateTimeOffset> Unix(ReadOnlySpan<byte> utf8, UnixPrecision precision)
	{
		GuardPrecision(precision);
		RawTimestamp value = default;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_unix(ptr, (nuint)utf8.Length, (uint)precision, &value, &fault);
		return code == 0 ? ToDateTimeOffset(value) : Failed<DateTimeOffset>(code, fault);
	}

	/// <inheritdoc cref="Unix(ReadOnlySpan{byte}, UnixPrecision)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="precision">The declared unit. Never <see cref="UnixPrecision.Unspecified"/>.</param>
	public static Verdict<DateTimeOffset> Unix(ReadOnlySpan<char> input, UnixPrecision precision)
	{
		byte[]? rented = null;
		try
		{
			return Unix(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented), precision);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <summary>
	/// Casts a strict ISO 8601 <c>yyyy-MM-dd</c> calendar date to a <see cref="DateOnly"/>.
	/// Anything time-bearing or non-ISO is <see cref="CastFailure.Malformed"/>; year 0000 is
	/// <see cref="CastFailure.OutOfRange"/>.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	public static unsafe Verdict<DateOnly> Date(ReadOnlySpan<byte> utf8)
	{
		RawDate value = default;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_date(ptr, (nuint)utf8.Length, &value, &fault);
		return code == 0
			? new DateOnly(value.Year, value.Month, value.Day)
			: Failed<DateOnly>(code, fault);
	}

	/// <inheritdoc cref="Date(ReadOnlySpan{byte})"/>
	/// <param name="input">The raw scalar text.</param>
	public static Verdict<DateOnly> Date(ReadOnlySpan<char> input)
	{
		byte[]? rented = null;
		try
		{
			return Date(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented));
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <summary>
	/// Casts a separated calendar date — three digit fields joined by one consistent
	/// separator (<c>/</c>, <c>-</c>, or <c>.</c>) — under the caller-declared
	/// <see cref="DateOrder"/> to a <see cref="DateOnly"/>: <c>1/7/2026</c> is January 7th
	/// or July 1st only because <paramref name="order"/> said which. The year field is four
	/// digits wherever the order puts it (two-digit years mean century guessing, which
	/// never happens — <see cref="CastFailure.Malformed"/>); the order-less
	/// <see cref="Date(ReadOnlySpan{byte})"/> overload stays strict ISO.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="order">The declared field order. Never <see cref="DateOrder.Unspecified"/>.</param>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="order"/> is undefined — a caller bug, not a data verdict.</exception>
	public static unsafe Verdict<DateOnly> Date(ReadOnlySpan<byte> utf8, DateOrder order)
	{
		GuardOrder(order);
		RawDate value = default;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_date_ordered(ptr, (nuint)utf8.Length, (uint)order, &value, &fault);
		return code == 0
			? new DateOnly(value.Year, value.Month, value.Day)
			: Failed<DateOnly>(code, fault);
	}

	/// <inheritdoc cref="Date(ReadOnlySpan{byte}, DateOrder)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="order">The declared field order. Never <see cref="DateOrder.Unspecified"/>.</param>
	public static Verdict<DateOnly> Date(ReadOnlySpan<char> input, DateOrder order)
	{
		byte[]? rented = null;
		try
		{
			return Date(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented), order);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <summary>
	/// Casts an ISO 8601 24-hour time-of-day — <c>HH:mm</c>, <c>HH:mm:ss</c>, or
	/// <c>HH:mm:ss.f{1..9}</c> — to a <see cref="TimeOnly"/>. Midnight and
	/// <c>23:59:59.999…</c> are real clock readings, so this door has no range failure.
	/// Sub-tick nanoseconds truncate.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	public static unsafe Verdict<TimeOnly> Time(ReadOnlySpan<byte> utf8)
	{
		ulong nanos = 0;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_time(ptr, (nuint)utf8.Length, &nanos, &fault);
		return code == 0 ? new TimeOnly((long)(nanos / 100)) : Failed<TimeOnly>(code, fault);
	}

	/// <inheritdoc cref="Time(ReadOnlySpan{byte})"/>
	/// <param name="input">The raw scalar text.</param>
	public static Verdict<TimeOnly> Time(ReadOnlySpan<char> input)
	{
		byte[]? rented = null;
		try
		{
			return Time(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented));
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <summary>
	/// Casts a duration in any of three cleanly-partitioned shapes to a <see cref="TimeSpan"/>:
	/// an ISO 8601 duration restricted to fixed components (<c>P2W</c>, <c>P1DT6H30M15.5S</c> —
	/// years/months are not fixed durations and are <see cref="CastFailure.Malformed"/>), the
	/// invariant colon form (<c>[-][d.]hh:mm[:ss[.f]]</c>), or protobuf JSON seconds
	/// (<c>3.5s</c>). Beyond ±10,000 years is <see cref="CastFailure.OutOfRange"/>. Sub-tick
	/// nanoseconds truncate.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	public static unsafe Verdict<TimeSpan> Duration(ReadOnlySpan<byte> utf8)
	{
		RawDuration value = default;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_duration(ptr, (nuint)utf8.Length, &value, &fault);
		return code == 0
			? new TimeSpan(value.Seconds * TimeSpan.TicksPerSecond + value.Nanos / 100)
			: Failed<TimeSpan>(code, fault);
	}

	/// <inheritdoc cref="Duration(ReadOnlySpan{byte})"/>
	/// <param name="input">The raw scalar text.</param>
	public static Verdict<TimeSpan> Duration(ReadOnlySpan<char> input)
	{
		byte[]? rented = null;
		try
		{
			return Duration(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented));
		}
		finally
		{
			Recycle(rented);
		}
	}

	static DateTimeOffset ToDateTimeOffset(in RawTimestamp timestamp) =>
		new(UnixEpochTicks + timestamp.Seconds * TimeSpan.TicksPerSecond + timestamp.Nanos / 100,
			TimeSpan.Zero);

	static void GuardOrder(DateOrder order)
	{
		if (order is not (DateOrder.YearMonthDay or DateOrder.MonthDayYear or DateOrder.DayMonthYear))
			throw new ArgumentOutOfRangeException(nameof(order), order,
				"Order must be YearMonthDay, MonthDayYear, or DayMonthYear.");
	}

	static void GuardPrecision(UnixPrecision precision)
	{
		if (precision is not (UnixPrecision.Seconds or UnixPrecision.Milliseconds
			or UnixPrecision.Microseconds or UnixPrecision.Nanoseconds))
			throw new ArgumentOutOfRangeException(nameof(precision), precision,
				"Precision must be Seconds, Milliseconds, Microseconds, or Nanoseconds.");
	}
}
