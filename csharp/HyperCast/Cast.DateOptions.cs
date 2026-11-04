using System.Runtime.CompilerServices;
using HyperCast.Interop;

namespace HyperCast;

// The date doors under DateOptions: the shapes, pivot and month names a caller opts into. Each is
// the door of the same name with its options declared, through cast_detailed; the doors without
// options keep today's shape rules.
public static partial class Cast
{
	/// <summary>
	/// Casts a strict ISO 8601 <c>yyyy-MM-dd</c> calendar date as <see cref="Date(ReadOnlySpan{byte})"/> does, and, with <see cref="DateShapes.CompactDate"/>, also <c>20260107</c> (eight ASCII digits as <c>yyyyMMdd</c>). Any other option is ignored here.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="options">The shapes, pivot and month names to read beyond the strict shapes; <c>default</c> declares nothing.</param>
	/// <exception cref="ArgumentException"><paramref name="options"/> carries an undefined shape bit — a caller bug, not a data verdict.</exception>
	/// <exception cref="ArgumentOutOfRangeException">The pivot is neither 0 nor 100 to 9999, or an order is undefined — a caller bug, not a data verdict.</exception>
	[OverloadResolutionPriority(-1)]
	public static unsafe Verdict<DateOnly> Date(ReadOnlySpan<byte> utf8, DateOptions options)
	{
		RawDate value = default;
		var fault = RawFaultDetail.Empty;
		var code = Detailed(DoorCodes.Date, 0, null, options, utf8, &value, &fault);
		return code == 0 ? value.ToDateOnly() : Failed<DateOnly>(code, fault);
	}

	/// <inheritdoc cref="Date(ReadOnlySpan{byte}, DateOptions)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="options">The shapes, pivot and month names to read beyond the strict shapes; <c>default</c> declares nothing.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<DateOnly> Date(ReadOnlySpan<char> input, DateOptions options)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Date(utf8, options), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <summary>
	/// Casts a separated calendar date as <see cref="Date(ReadOnlySpan{byte}, DateOrder)"/> does, with a two-digit year read through <see cref="DateOptions.Pivot"/> and a month name through <see cref="DateOptions.Months"/> in the month position the <paramref name="order"/> selects (<c>07-Jan-26</c>, <c>7-Jan-2026</c>). The shapes are ignored here.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="order">The declared field order. Never <see cref="DateOrder.Unspecified"/>.</param>
	/// <param name="options">The shapes, pivot and month names to read beyond the strict shapes; <c>default</c> declares nothing.</param>
	/// <exception cref="ArgumentException"><paramref name="options"/> carries an undefined shape bit — a caller bug, not a data verdict.</exception>
	/// <exception cref="ArgumentOutOfRangeException">The pivot is neither 0 nor 100 to 9999, or an order is undefined — a caller bug, not a data verdict.</exception>
	public static unsafe Verdict<DateOnly> Date(ReadOnlySpan<byte> utf8, DateOrder order, DateOptions options)
	{
		var declared = Abi.Code(order);
		RawDate value = default;
		var fault = RawFaultDetail.Empty;
		var code = Detailed(DoorCodes.DateOrdered, declared, null, options, utf8, &value, &fault);
		return code == 0 ? value.ToDateOnly() : Failed<DateOnly>(code, fault);
	}

	/// <inheritdoc cref="Date(ReadOnlySpan{byte}, DateOrder, DateOptions)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="order">The declared field order. Never <see cref="DateOrder.Unspecified"/>.</param>
	/// <param name="options">The shapes, pivot and month names to read beyond the strict shapes; <c>default</c> declares nothing.</param>
	public static Verdict<DateOnly> Date(ReadOnlySpan<char> input, DateOrder order, DateOptions options)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Date(utf8, order, options), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <summary>
	/// Casts an RFC 3339 instant as <see cref="Timestamp(ReadOnlySpan{byte})"/> does, and, with <see cref="DateShapes.SpaceSeparator"/>, a single space in place of <c>T</c>, and with <see cref="DateShapes.HourOffset"/>, <c>±hh</c> offsets. The zone stays mandatory.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="options">The shapes, pivot and month names to read beyond the strict shapes; <c>default</c> declares nothing.</param>
	/// <exception cref="ArgumentException"><paramref name="options"/> carries an undefined shape bit — a caller bug, not a data verdict.</exception>
	/// <exception cref="ArgumentOutOfRangeException">The pivot is neither 0 nor 100 to 9999, or an order is undefined — a caller bug, not a data verdict.</exception>
	public static unsafe Verdict<DateTimeOffset> Timestamp(ReadOnlySpan<byte> utf8, DateOptions options)
	{
		RawTimestamp value = default;
		var fault = RawFaultDetail.Empty;
		var code = Detailed(DoorCodes.Timestamp, 0, null, options, utf8, &value, &fault);
		return code == 0 ? value.ToDateTimeOffset() : Failed<DateTimeOffset>(code, fault);
	}

	/// <inheritdoc cref="Timestamp(ReadOnlySpan{byte}, DateOptions)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="options">The shapes, pivot and month names to read beyond the strict shapes; <c>default</c> declares nothing.</param>
	public static Verdict<DateTimeOffset> Timestamp(ReadOnlySpan<char> input, DateOptions options)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(Timestamp(utf8, options), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <summary>
	/// Casts a zone-less civil date-time as <see cref="DateTime(ReadOnlySpan{byte}, DateOrder)"/> does, with the date read as <see cref="Date(ReadOnlySpan{byte}, DateOrder, DateOptions)"/> reads it: a pivot for two-digit years, month names. The shapes are ignored here.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="order">The declared field order. Never <see cref="DateOrder.Unspecified"/>.</param>
	/// <param name="options">The shapes, pivot and month names to read beyond the strict shapes; <c>default</c> declares nothing.</param>
	/// <exception cref="ArgumentException"><paramref name="options"/> carries an undefined shape bit — a caller bug, not a data verdict.</exception>
	/// <exception cref="ArgumentOutOfRangeException">The pivot is neither 0 nor 100 to 9999, or an order is undefined — a caller bug, not a data verdict.</exception>
	public static unsafe Verdict<System.DateTime> DateTime(ReadOnlySpan<byte> utf8, DateOrder order, DateOptions options)
	{
		var declared = Abi.Code(order);
		RawCivil value = default;
		var fault = RawFaultDetail.Empty;
		var code = Detailed(DoorCodes.DateTime, declared, null, options, utf8, &value, &fault);
		return code == 0 ? value.ToDateTime() : Failed<System.DateTime>(code, fault);
	}

	/// <inheritdoc cref="DateTime(ReadOnlySpan{byte}, DateOrder, DateOptions)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="order">The declared field order. Never <see cref="DateOrder.Unspecified"/>.</param>
	/// <param name="options">The shapes, pivot and month names to read beyond the strict shapes; <c>default</c> declares nothing.</param>
	public static Verdict<System.DateTime> DateTime(ReadOnlySpan<char> input, DateOrder order, DateOptions options)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(DateTime(utf8, order, options), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}

	/// <summary>
	/// <see cref="DateTime(ReadOnlySpan{byte}, DateOrder, DateOptions)"/> under a declared <see cref="DateFormat"/>: its
	/// <see cref="DateFormat.Order"/> and its day-period designators, as
	/// <see cref="DateTime(ReadOnlySpan{byte}, DateFormat)"/> reads them.
	/// </summary>
	/// <param name="utf8">The raw scalar text as UTF-8 bytes.</param>
	/// <param name="format">The declared order and designators. Its order is never <see cref="DateOrder.Unspecified"/>.</param>
	/// <param name="options">The pivot and month names to read; <c>default</c> declares nothing.</param>
	/// <exception cref="ArgumentOutOfRangeException">The format's order is undefined, or the pivot is neither 0 nor 100 to 9999 — a caller bug, not a data verdict.</exception>
	/// <exception cref="ArgumentException">A designator the core cannot read, or an undefined shape bit — a caller bug, not a data verdict.</exception>
	[OverloadResolutionPriority(-1)]
	public static unsafe Verdict<System.DateTime> DateTime(ReadOnlySpan<byte> utf8, DateFormat format, DateOptions options)
	{
		var declared = Abi.Code(format.Order, nameof(format));
		var periods = format.ToRaw();
		RawCivil value = default;
		var fault = RawFaultDetail.Empty;
		var code = Detailed(DoorCodes.DateTime, declared, format.HasDesignators ? &periods : null, options, utf8, &value, &fault);
		return code == 0 ? value.ToDateTime() : Failed<System.DateTime>(code, fault);
	}

	/// <inheritdoc cref="DateTime(ReadOnlySpan{byte}, DateFormat, DateOptions)"/>
	/// <param name="input">The raw scalar text.</param>
	/// <param name="format">The declared order and designators. Its order is never <see cref="DateOrder.Unspecified"/>.</param>
	/// <param name="options">The pivot and month names to read; <c>default</c> declares nothing.</param>
	[OverloadResolutionPriority(-1)]
	public static Verdict<System.DateTime> DateTime(ReadOnlySpan<char> input, DateFormat format, DateOptions options)
	{
		byte[]? rented = null;
		try
		{
			var utf8 = Utf8(input, stackalloc byte[Utf8StackBytes], ref rented);
			return Remap(DateTime(utf8, format, options), utf8, input.Length);
		}
		finally
		{
			Recycle(rented);
		}
	}
}
