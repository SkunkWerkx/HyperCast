namespace HyperCast;

public static partial class Cast
{
	/// <summary>
	/// Every text door behind one generic entry point, for a caller that is itself generic
	/// over the target type (a serializer's converter, a parsing gateway) and would otherwise
	/// keep its own <c>typeof</c> table mirroring this class. <typeparamref name="T"/> picks
	/// the door; the verdict is exactly the one that door returns:
	/// <list type="table">
	/// <listheader><term><typeparamref name="T"/></term><description>Door</description></listheader>
	/// <item><term><see cref="bool"/></term><description><see cref="Boolean(ReadOnlySpan{byte})"/></description></item>
	/// <item><term><see cref="sbyte"/> … <see cref="ulong"/>, <see cref="float"/>, <see cref="double"/>, <see cref="decimal"/></term><description>the numeric doors, as <see cref="Numeric{T}(ReadOnlySpan{byte}, NumFormat)"/></description></item>
	/// <item><term><see cref="char"/></term><description><see cref="Char(ReadOnlySpan{byte})"/></description></item>
	/// <item><term><see cref="Guid"/></term><description><see cref="Uuid(ReadOnlySpan{byte})"/></description></item>
	/// <item><term><see cref="DateOnly"/></term><description><see cref="Date(ReadOnlySpan{byte})"/>, strict ISO <c>yyyy-MM-dd</c> (no <see cref="DateOrder"/>)</description></item>
	/// <item><term><see cref="TimeOnly"/></term><description><see cref="Time(ReadOnlySpan{byte})"/></description></item>
	/// <item><term><see cref="DateTimeOffset"/></term><description><see cref="Timestamp(ReadOnlySpan{byte})"/>, RFC 3339 with the zone mandatory</description></item>
	/// <item><term><see cref="System.DateTime"/></term><description><see cref="Timestamp(ReadOnlySpan{byte})"/>, projected to <see cref="DateTimeOffset.UtcDateTime"/> (<see cref="DateTimeKind.Utc"/>)</description></item>
	/// <item><term><see cref="TimeSpan"/></term><description><see cref="Duration(ReadOnlySpan{byte})"/></description></item>
	/// </list>
	/// The <c>typeof</c> tests fold per instantiation, so a given <typeparamref name="T"/>
	/// costs one direct call, with no boxing and no reflection (NativeAOT-safe).
	/// </summary>
	/// <remarks>
	/// <para>
	/// <see cref="System.DateTime"/> is <b>not</b> <see cref="DateTime(ReadOnlySpan{byte}, DateOrder)"/>'s
	/// civil door: that one needs a declared <see cref="DateOrder"/> and returns
	/// <see cref="DateTimeKind.Unspecified"/>, and a generic caller has no order to declare. The
	/// one reading that needs no declaration is the RFC 3339 instant, so here
	/// <see cref="System.DateTime"/> means that instant in UTC. A caller that wants the civil
	/// reading special-cases <see cref="System.DateTime"/> and calls the civil door itself.
	/// </para>
	/// <para>
	/// <paramref name="format"/> is read only by the numeric doors; every other door ignores
	/// it.
	/// </para>
	/// </remarks>
	/// <typeparam name="T">The target type — one of the rows above.</typeparam>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared numeric notation, for the numeric targets.</param>
	/// <exception cref="NotSupportedException">
	/// No door reads <typeparamref name="T"/> (<see cref="Int128"/>, <see cref="Half"/>, an enum,
	/// a user type) — thrown before any native call, so a caller can try this door first and
	/// fall back on the exception or, better, on a type check of its own made once.
	/// </exception>
	public static Verdict<T> Scalar<T>(ReadOnlySpan<byte> utf8, NumFormat format) where T : struct
	{
		if (typeof(T) == typeof(bool)) return Fold<bool, T>(Boolean(utf8));
		if (typeof(T) == typeof(sbyte)) return Fold<sbyte, T>(SByte(utf8, format));
		if (typeof(T) == typeof(short)) return Fold<short, T>(Int16(utf8, format));
		if (typeof(T) == typeof(int)) return Fold<int, T>(Int32(utf8, format));
		if (typeof(T) == typeof(long)) return Fold<long, T>(Int64(utf8, format));
		if (typeof(T) == typeof(byte)) return Fold<byte, T>(Byte(utf8, format));
		if (typeof(T) == typeof(ushort)) return Fold<ushort, T>(UInt16(utf8, format));
		if (typeof(T) == typeof(uint)) return Fold<uint, T>(UInt32(utf8, format));
		if (typeof(T) == typeof(ulong)) return Fold<ulong, T>(UInt64(utf8, format));
		if (typeof(T) == typeof(float)) return Fold<float, T>(Single(utf8, format));
		if (typeof(T) == typeof(double)) return Fold<double, T>(Double(utf8, format));
		if (typeof(T) == typeof(decimal)) return Fold<decimal, T>(Decimal(utf8, format));
		if (typeof(T) == typeof(char)) return Fold<char, T>(Char(utf8));
		if (typeof(T) == typeof(Guid)) return Fold<Guid, T>(Uuid(utf8));
		if (typeof(T) == typeof(DateOnly)) return Fold<DateOnly, T>(Date(utf8));
		if (typeof(T) == typeof(TimeOnly)) return Fold<TimeOnly, T>(Time(utf8));
		if (typeof(T) == typeof(DateTimeOffset)) return Fold<DateTimeOffset, T>(Timestamp(utf8));
		if (typeof(T) == typeof(System.DateTime)) return Fold<System.DateTime, T>(Utc(Timestamp(utf8)));
		if (typeof(T) == typeof(TimeSpan)) return Fold<TimeSpan, T>(Duration(utf8));
		throw Unsupported<T>();
	}

	/// <inheritdoc cref="Scalar{T}(ReadOnlySpan{byte}, NumFormat)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared numeric notation, for the numeric targets.</param>
	public static Verdict<T> Scalar<T>(ReadOnlySpan<char> input, NumFormat format) where T : struct
	{
		// char keeps its own UTF-16 door, whose one-code-unit rule a transcode would lose.
		if (typeof(T) == typeof(char))
			return Fold<char, T>(Char(input));
		if (!HasScalarDoor<T>())
			throw Unsupported<T>();
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Scalar<T>(utf8, format), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	static bool HasScalarDoor<T>() =>
		typeof(T) == typeof(bool)
		|| typeof(T) == typeof(sbyte) || typeof(T) == typeof(short) || typeof(T) == typeof(int) || typeof(T) == typeof(long)
		|| typeof(T) == typeof(byte) || typeof(T) == typeof(ushort) || typeof(T) == typeof(uint) || typeof(T) == typeof(ulong)
		|| typeof(T) == typeof(float) || typeof(T) == typeof(double) || typeof(T) == typeof(decimal)
		|| typeof(T) == typeof(char) || typeof(T) == typeof(Guid)
		|| typeof(T) == typeof(DateOnly) || typeof(T) == typeof(TimeOnly)
		|| typeof(T) == typeof(DateTimeOffset) || typeof(T) == typeof(System.DateTime) || typeof(T) == typeof(TimeSpan);

	static NotSupportedException Unsupported<T>() =>
		new($"No HyperCast door casts text to {typeof(T)}.");

	static Verdict<System.DateTime> Utc(Verdict<DateTimeOffset> verdict)
	{
		if (verdict.TryGetValue(out Success<DateTimeOffset> success))
			return success.Value.UtcDateTime;
		verdict.TryGetValue(out Fault fault);
		return new(fault);
	}
}
