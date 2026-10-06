using System.Runtime.CompilerServices;

namespace HyperCast.Interop;

/// <summary>
/// The native core's C ABI vocabulary — declared-option discriminants, verdict codes, the
/// packed version word and the time-of-day and UUID values — for a library that carries
/// HyperCast's verdicts across a C ABI of its own. The Cast doors use exactly these, so a
/// verdict read out of another library's buffer is presented as a Cast door would present it.
/// </summary>
public static class Abi
{
	/// <summary>The precision's ABI discriminant, checked.</summary>
	/// <param name="precision">The declared precision.</param>
	/// <param name="paramName">The caller's parameter, for the exception.</param>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="precision"/> is not a defined precision.</exception>
	public static uint Code(UnixPrecision precision, [CallerArgumentExpression(nameof(precision))] string? paramName = null) =>
		precision is UnixPrecision.Seconds or UnixPrecision.Milliseconds or UnixPrecision.Microseconds or UnixPrecision.Nanoseconds
			? (uint)precision
			: throw new ArgumentOutOfRangeException(paramName, precision,
				"Precision must be Seconds, Milliseconds, Microseconds, or Nanoseconds.");

	/// <summary>The order's ABI discriminant, checked.</summary>
	/// <param name="order">The declared order.</param>
	/// <param name="paramName">The caller's parameter, for the exception.</param>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="order"/> is not a defined order.</exception>
	public static uint Code(DateOrder order, [CallerArgumentExpression(nameof(order))] string? paramName = null) =>
		order is DateOrder.YearMonthDay or DateOrder.MonthDayYear or DateOrder.DayMonthYear
			? (uint)order
			: throw new ArgumentOutOfRangeException(paramName, order,
				"Order must be YearMonthDay, MonthDayYear, or DayMonthYear.");

	/// <summary>The date system's ABI discriminant, checked.</summary>
	/// <param name="epoch">The declared date system.</param>
	/// <param name="paramName">The caller's parameter, for the exception.</param>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="epoch"/> is not a defined date system.</exception>
	public static uint Code(ExcelEpoch epoch, [CallerArgumentExpression(nameof(epoch))] string? paramName = null) =>
		epoch is ExcelEpoch.Y1900 or ExcelEpoch.Y1904
			? (uint)epoch
			: throw new ArgumentOutOfRangeException(paramName, epoch, "Epoch must be Y1900 or Y1904.");

	/// <summary>The precision whose ABI discriminant is <paramref name="code"/>, or <see langword="null"/>.</summary>
	/// <param name="code">The discriminant: 1 seconds … 4 nanoseconds.</param>
	public static UnixPrecision? UnixPrecisionFrom(uint code) =>
		code is >= 1 and <= 4 ? (UnixPrecision)code : null;

	/// <summary>The order whose ABI discriminant is <paramref name="code"/>, or <see langword="null"/>.</summary>
	/// <param name="code">The discriminant: 1 year-month-day, 2 month-day-year, 3 day-month-year.</param>
	public static DateOrder? DateOrderFrom(uint code) =>
		code is >= 1 and <= 3 ? (DateOrder)code : null;

	/// <summary>The date system whose ABI discriminant is <paramref name="code"/>, or <see langword="null"/>.</summary>
	/// <param name="code">The discriminant: 1 the 1900 system, 2 the 1904 system.</param>
	public static ExcelEpoch? ExcelEpochFrom(uint code) =>
		code is 1 or 2 ? (ExcelEpoch)code : null;

	/// <summary>The reason whose verdict code is <paramref name="code"/>, or <see langword="null"/> — 0, success, included.</summary>
	/// <param name="code">The verdict code: 1 empty, 2 malformed, 3 out of range.</param>
	public static CastFailure? ReasonFrom(uint code) =>
		code is >= 1 and <= 3 ? (CastFailure)code : null;

	/// <summary>The fault a nonzero verdict code and its span name.</summary>
	/// <param name="code">The verdict code: 1 empty, 2 malformed, 3 out of range.</param>
	/// <param name="span">The span, in bytes of the input.</param>
	/// <exception cref="InvalidOperationException"><paramref name="code"/> is no reason — a binding bug, not data.</exception>
	public static Fault ToFault(uint code, RawFault span) =>
		ReasonFrom(code) is { } reason
			? new Fault(reason, (int)span.Offset, (int)span.Length)
			: throw new InvalidOperationException($"{code} is not a verdict reason code — a binding bug, please report it.");

	/// <summary>A time of day, as the core writes it (nanoseconds since midnight), sub-tick nanoseconds truncated.</summary>
	/// <param name="nanosOfDay">Nanoseconds since midnight.</param>
	public static TimeOnly ToTimeOnly(ulong nanosOfDay) => new((long)(nanosOfDay / 100));

	/// <summary>A UUID, as the core writes it: 16 bytes in RFC 9562 (big-endian) order.</summary>
	/// <param name="rfc9562">The 16 bytes.</param>
	/// <exception cref="ArgumentException"><paramref name="rfc9562"/> is not 16 bytes.</exception>
	public static Guid ToGuid(ReadOnlySpan<byte> rfc9562) => new(rfc9562, bigEndian: true);

	/// <summary>A native library's packed version word — <c>major &lt;&lt; 16 | minor &lt;&lt; 8 | patch</c> — as a <see cref="Version"/>.</summary>
	/// <param name="packed">The word, as a <c>*_version()</c> export returns it.</param>
	public static Version ToVersion(uint packed) =>
		new((int)(packed >> 16), (int)((packed >> 8) & 0xFF), (int)(packed & 0xFF));

	/// <summary>
	/// Asks a native library for its version, or <see langword="null"/> when it did not
	/// load — the probe <see cref="Cast.IsAvailable"/> and <see cref="Cast.NativeVersion"/>
	/// rest on, for any library's <c>*_version()</c> export. Only the failures of finding
	/// and binding the library are caught; anything else propagates.
	/// </summary>
	/// <param name="version">Calls the library's version export.</param>
	public static Version? ProbeVersion(Func<uint> version)
	{
		ArgumentNullException.ThrowIfNull(version);
		try
		{
			return ToVersion(version());
		}
		catch (Exception e) when (e is DllNotFoundException or EntryPointNotFoundException
			or BadImageFormatException or PlatformNotSupportedException or TypeInitializationException)
		{
			return null;
		}
	}
}
