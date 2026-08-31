using System.Buffers;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;

namespace HyperCast;

/// <summary>
/// Allocation-free scalar casts — booleans, numerics, UUIDs, temporals — calling directly
/// into the native <c>libhypercast</c> shared library via source-generated P/Invoke. Every
/// door returns a <see cref="Verdict{T}"/> (the value, or a <see cref="Fault"/> with a
/// closed reason and the offending span) and never throws on bad input; the only exceptions
/// here are caller bugs (a malformed <see cref="NumFormat"/>, an undefined
/// <see cref="UnixPrecision"/>), never data.
/// </summary>
/// <remarks>
/// <para>
/// Each door takes UTF-8 bytes (<see cref="ReadOnlySpan{Byte}"/> — the native contract,
/// zero-copy) or UTF-16 (<see cref="ReadOnlySpan{Char}"/>/<see cref="string"/>, encoded into
/// a stack buffer with an <see cref="ArrayPool{T}"/> fallback, HyperUuid's proven pattern).
/// Nothing allocates on either the success or the failure path.
/// </para>
/// <para>
/// Temporal doors present the core's protobuf-shaped values at .NET fidelity:
/// <see cref="DateTimeOffset"/>/<see cref="TimeOnly"/>/<see cref="TimeSpan"/> resolve to
/// 100 ns ticks, so sub-tick nanoseconds truncate (the core carries full nanosecond
/// fidelity; .NET's clock types don't).
/// </para>
/// <para>
/// AOT/trimming friendly: <see cref="LibraryImportAttribute"/> is source-generated (no
/// runtime reflection), so this type publishes cleanly under <c>PublishAot</c>. Needs the
/// platform-specific native binary. No separate <c>browser-wasm</c> build anymore —
/// HyperUuid's single-assembly pattern, ported home: every native entry point is declared
/// twice, once against <c>"hypercast"</c> (dlopen on every real native platform), once
/// against <c>"*"</c> (resolves against the current module — the only thing that works for
/// a statically-linked WASM native), sharing the same
/// <see cref="LibraryImportAttribute.EntryPoint"/>. <see cref="OperatingSystem.IsBrowser"/>
/// picks the right one at the call site — a real runtime check the .NET linker knows how to
/// constant-fold per publish target, so a trimmed build still ships only the reachable
/// branch.
/// </para>
/// </remarks>
[SkipLocalsInit] // the UTF-16 doors stackalloc a 512-byte transcode buffer per call; without
                 // this the JIT zeroes it every time — a measured double-digit-ns tax on
                 // every char-door cast. No local here is read before it's written.
public static partial class Cast
{
	/// <summary>UTF-16 doors encode through a stack buffer of this size before renting.</summary>
	const int Utf8StackBytes = 512;

	[StructLayout(LayoutKind.Sequential)]
	internal readonly struct RawNumFormat(uint decimalSep, uint groupSep, uint flags)
	{
		public readonly uint DecimalSep = decimalSep;
		public readonly uint GroupSep = groupSep;
		public readonly uint Flags = flags;
	}

	[StructLayout(LayoutKind.Sequential)]
	readonly struct RawFault
	{
		public readonly uint Offset;
		public readonly uint Length;
	}

	[StructLayout(LayoutKind.Sequential)]
	readonly struct RawTimestamp
	{
		public readonly long Seconds;
		public readonly int Nanos;
	}

	[StructLayout(LayoutKind.Sequential)]
	readonly struct RawDate
	{
		public readonly ushort Year;
		public readonly byte Month;
		public readonly byte Day;
	}

	[StructLayout(LayoutKind.Sequential)]
	readonly struct RawDuration
	{
		public readonly long Seconds;
		public readonly int Nanos;
	}

	[LibraryImport("hypercast", EntryPoint = "cast_bool")]
	private static unsafe partial int cast_bool_native(byte* ptr, nuint len, byte* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_bool")]
	private static unsafe partial int cast_bool_browser(byte* ptr, nuint len, byte* value, RawFault* fault);
	private static unsafe int cast_bool(byte* ptr, nuint len, byte* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_bool_browser(ptr, len, value, fault) : cast_bool_native(ptr, len, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_i8")]
	private static unsafe partial int cast_i8_native(byte* ptr, nuint len, RawNumFormat* format, sbyte* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_i8")]
	private static unsafe partial int cast_i8_browser(byte* ptr, nuint len, RawNumFormat* format, sbyte* value, RawFault* fault);
	private static unsafe int cast_i8(byte* ptr, nuint len, RawNumFormat* format, sbyte* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_i8_browser(ptr, len, format, value, fault) : cast_i8_native(ptr, len, format, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_i16")]
	private static unsafe partial int cast_i16_native(byte* ptr, nuint len, RawNumFormat* format, short* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_i16")]
	private static unsafe partial int cast_i16_browser(byte* ptr, nuint len, RawNumFormat* format, short* value, RawFault* fault);
	private static unsafe int cast_i16(byte* ptr, nuint len, RawNumFormat* format, short* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_i16_browser(ptr, len, format, value, fault) : cast_i16_native(ptr, len, format, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_i32")]
	private static unsafe partial int cast_i32_native(byte* ptr, nuint len, RawNumFormat* format, int* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_i32")]
	private static unsafe partial int cast_i32_browser(byte* ptr, nuint len, RawNumFormat* format, int* value, RawFault* fault);
	private static unsafe int cast_i32(byte* ptr, nuint len, RawNumFormat* format, int* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_i32_browser(ptr, len, format, value, fault) : cast_i32_native(ptr, len, format, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_i64")]
	private static unsafe partial int cast_i64_native(byte* ptr, nuint len, RawNumFormat* format, long* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_i64")]
	private static unsafe partial int cast_i64_browser(byte* ptr, nuint len, RawNumFormat* format, long* value, RawFault* fault);
	private static unsafe int cast_i64(byte* ptr, nuint len, RawNumFormat* format, long* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_i64_browser(ptr, len, format, value, fault) : cast_i64_native(ptr, len, format, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_u8")]
	private static unsafe partial int cast_u8_native(byte* ptr, nuint len, RawNumFormat* format, byte* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_u8")]
	private static unsafe partial int cast_u8_browser(byte* ptr, nuint len, RawNumFormat* format, byte* value, RawFault* fault);
	private static unsafe int cast_u8(byte* ptr, nuint len, RawNumFormat* format, byte* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_u8_browser(ptr, len, format, value, fault) : cast_u8_native(ptr, len, format, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_u16")]
	private static unsafe partial int cast_u16_native(byte* ptr, nuint len, RawNumFormat* format, ushort* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_u16")]
	private static unsafe partial int cast_u16_browser(byte* ptr, nuint len, RawNumFormat* format, ushort* value, RawFault* fault);
	private static unsafe int cast_u16(byte* ptr, nuint len, RawNumFormat* format, ushort* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_u16_browser(ptr, len, format, value, fault) : cast_u16_native(ptr, len, format, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_u32")]
	private static unsafe partial int cast_u32_native(byte* ptr, nuint len, RawNumFormat* format, uint* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_u32")]
	private static unsafe partial int cast_u32_browser(byte* ptr, nuint len, RawNumFormat* format, uint* value, RawFault* fault);
	private static unsafe int cast_u32(byte* ptr, nuint len, RawNumFormat* format, uint* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_u32_browser(ptr, len, format, value, fault) : cast_u32_native(ptr, len, format, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_u64")]
	private static unsafe partial int cast_u64_native(byte* ptr, nuint len, RawNumFormat* format, ulong* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_u64")]
	private static unsafe partial int cast_u64_browser(byte* ptr, nuint len, RawNumFormat* format, ulong* value, RawFault* fault);
	private static unsafe int cast_u64(byte* ptr, nuint len, RawNumFormat* format, ulong* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_u64_browser(ptr, len, format, value, fault) : cast_u64_native(ptr, len, format, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_f32")]
	private static unsafe partial int cast_f32_native(byte* ptr, nuint len, RawNumFormat* format, float* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_f32")]
	private static unsafe partial int cast_f32_browser(byte* ptr, nuint len, RawNumFormat* format, float* value, RawFault* fault);
	private static unsafe int cast_f32(byte* ptr, nuint len, RawNumFormat* format, float* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_f32_browser(ptr, len, format, value, fault) : cast_f32_native(ptr, len, format, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_f64")]
	private static unsafe partial int cast_f64_native(byte* ptr, nuint len, RawNumFormat* format, double* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_f64")]
	private static unsafe partial int cast_f64_browser(byte* ptr, nuint len, RawNumFormat* format, double* value, RawFault* fault);
	private static unsafe int cast_f64(byte* ptr, nuint len, RawNumFormat* format, double* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_f64_browser(ptr, len, format, value, fault) : cast_f64_native(ptr, len, format, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_uuid")]
	private static unsafe partial int cast_uuid_native(byte* ptr, nuint len, byte* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_uuid")]
	private static unsafe partial int cast_uuid_browser(byte* ptr, nuint len, byte* value, RawFault* fault);
	private static unsafe int cast_uuid(byte* ptr, nuint len, byte* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_uuid_browser(ptr, len, value, fault) : cast_uuid_native(ptr, len, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_timestamp")]
	private static unsafe partial int cast_timestamp_native(byte* ptr, nuint len, RawTimestamp* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_timestamp")]
	private static unsafe partial int cast_timestamp_browser(byte* ptr, nuint len, RawTimestamp* value, RawFault* fault);
	private static unsafe int cast_timestamp(byte* ptr, nuint len, RawTimestamp* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_timestamp_browser(ptr, len, value, fault) : cast_timestamp_native(ptr, len, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_unix")]
	private static unsafe partial int cast_unix_native(byte* ptr, nuint len, uint precision, RawTimestamp* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_unix")]
	private static unsafe partial int cast_unix_browser(byte* ptr, nuint len, uint precision, RawTimestamp* value, RawFault* fault);
	private static unsafe int cast_unix(byte* ptr, nuint len, uint precision, RawTimestamp* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_unix_browser(ptr, len, precision, value, fault) : cast_unix_native(ptr, len, precision, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_date")]
	private static unsafe partial int cast_date_native(byte* ptr, nuint len, RawDate* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_date")]
	private static unsafe partial int cast_date_browser(byte* ptr, nuint len, RawDate* value, RawFault* fault);
	private static unsafe int cast_date(byte* ptr, nuint len, RawDate* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_date_browser(ptr, len, value, fault) : cast_date_native(ptr, len, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_time")]
	private static unsafe partial int cast_time_native(byte* ptr, nuint len, ulong* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_time")]
	private static unsafe partial int cast_time_browser(byte* ptr, nuint len, ulong* value, RawFault* fault);
	private static unsafe int cast_time(byte* ptr, nuint len, ulong* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_time_browser(ptr, len, value, fault) : cast_time_native(ptr, len, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_duration")]
	private static unsafe partial int cast_duration_native(byte* ptr, nuint len, RawDuration* value, RawFault* fault);
	[LibraryImport("*", EntryPoint = "cast_duration")]
	private static unsafe partial int cast_duration_browser(byte* ptr, nuint len, RawDuration* value, RawFault* fault);
	private static unsafe int cast_duration(byte* ptr, nuint len, RawDuration* value, RawFault* fault) =>
		OperatingSystem.IsBrowser() ? cast_duration_browser(ptr, len, value, fault) : cast_duration_native(ptr, len, value, fault);

	const long UnixEpochTicks = 621_355_968_000_000_000L;

	static ReadOnlySpan<byte> Utf8(ReadOnlySpan<char> chars, Span<byte> stack, ref byte[]? rented)
	{
		var maxBytes = Encoding.UTF8.GetMaxByteCount(chars.Length);
		var target = maxBytes <= stack.Length
			? stack
			: (rented = ArrayPool<byte>.Shared.Rent(maxBytes)).AsSpan();
		var written = Encoding.UTF8.GetBytes(chars, target);
		return target[..written];
	}

	static void Recycle(byte[]? rented)
	{
		if (rented is not null)
			ArrayPool<byte>.Shared.Return(rented);
	}

	static Verdict<T> Failed<T>(int code, in RawFault fault) where T : struct =>
		code == -1
			? throw new InvalidOperationException(
				"libhypercast reported a contract violation — a binding bug, please report it.")
			: new(new Fault((CastFailure)code, (int)fault.Offset, (int)fault.Length));

	/// <summary>
	/// Presents a verdict optionally: an <see cref="CastFailure.Empty"/> fault becomes
	/// absent (<see langword="null"/>); every other outcome flows through untouched. The
	/// composition-side answer to Svartalfheim's per-door <c>ParseOptional</c> pairs.
	/// </summary>
	/// <typeparam name="T">The cast value's type.</typeparam>
	/// <param name="verdict">The verdict from any door.</param>
	public static Verdict<T>? Optional<T>(Verdict<T> verdict) where T : struct =>
		verdict.TryGetValue(out Fault fault) && fault.Reason == CastFailure.Empty
			? null
			: verdict;

	/// <summary>
	/// Casts boolean text: <c>true</c>/<c>false</c> plus the numeric and natural-language
	/// conventions untrusted sources actually send (<c>t</c>/<c>f</c>, <c>yes</c>/<c>no</c>,
	/// <c>y</c>/<c>n</c>, <c>1</c>/<c>0</c>, <c>on</c>/<c>off</c>,
	/// <c>enabled</c>/<c>disabled</c>, <c>active</c>/<c>inactive</c>,
	/// <c>checked</c>/<c>unchecked</c>, <c>in</c>/<c>out</c>), ASCII case-insensitive.
	/// Culture-insensitive — no <see cref="NumFormat"/>.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	public static unsafe Verdict<bool> Boolean(ReadOnlySpan<byte> utf8)
	{
		byte value = 0;
		RawFault fault = default;
		int code;
		fixed (byte* ptr = utf8)
			code = cast_bool(ptr, (nuint)utf8.Length, &value, &fault);
		return code == 0 ? value != 0 : Failed<bool>(code, fault);
	}

	/// <inheritdoc cref="Boolean(ReadOnlySpan{byte})"/>
	/// <param name="input">The raw scalar text.</param>
	public static Verdict<bool> Boolean(ReadOnlySpan<char> input)
	{
		byte[]? rented = null;
		try
		{
			return Boolean(Utf8(input, stackalloc byte[Utf8StackBytes], ref rented));
		}
		finally
		{
			Recycle(rented);
		}
	}
}
