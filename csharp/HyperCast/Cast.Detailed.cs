using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using HyperCast.Interop;

namespace HyperCast;

// Every door reaches the core through the four detailed exports of 1.1: they take the 1.1
// format (grouping policy in the flags word, undefined bits refused), write a RawFaultDetail
// (the span and what failed), and read the date options and month vocabulary the date doors
// declare. The 1.0 exports above stay declared: they are the core's own surface, and the
// export-parity check demands every one, but no door uses them. A 1.0 core has no
// detailed export, so a door on one throws EntryPointNotFoundException and never parses with
// 1.0's rules — which is also how IsAvailable tells a core below 1.1.
public static partial class Cast
{
	/// <summary>The door codes of the core's <c>Door</c>, which the detailed exports take.</summary>
	static class DoorCodes
	{
		public const uint Bool = 1;
		public const uint I8 = 2;
		public const uint I16 = 3;
		public const uint I32 = 4;
		public const uint I64 = 5;
		public const uint U8 = 6;
		public const uint U16 = 7;
		public const uint U32 = 8;
		public const uint U64 = 9;
		public const uint F32 = 10;
		public const uint F64 = 11;
		public const uint Uuid = 12;
		public const uint Timestamp = 13;
		public const uint Unix = 14;
		public const uint Date = 15;
		public const uint Time = 16;
		public const uint Duration = 17;
		public const uint Text = 18;
		public const uint Decimal = 19;
		public const uint DateOrdered = 20;
		public const uint DateTime = 21;
		public const uint ExcelSerial = 22;
		public const uint Char = 23;
	}

	[LibraryImport("hypercast", EntryPoint = "cast_detailed")]
	private static unsafe partial int cast_detailed_native(uint door, uint param, RawNumFormat* format, RawDayPeriods* periods, RawDateOptions* options, byte* vocab, nuint vocabLen, byte* ptr, nuint len, void* value, RawFaultDetail* fault);
	[LibraryImport("*", EntryPoint = "cast_detailed")]
	private static unsafe partial int cast_detailed_browser(uint door, uint param, RawNumFormat* format, RawDayPeriods* periods, RawDateOptions* options, byte* vocab, nuint vocabLen, byte* ptr, nuint len, void* value, RawFaultDetail* fault);
	[LibraryImport("__Internal", EntryPoint = "cast_detailed")]
	private static unsafe partial int cast_detailed_internal(uint door, uint param, RawNumFormat* format, RawDayPeriods* periods, RawDateOptions* options, byte* vocab, nuint vocabLen, byte* ptr, nuint len, void* value, RawFaultDetail* fault);
	private static unsafe int cast_detailed(uint door, uint param, RawNumFormat* format, RawDayPeriods* periods, RawDateOptions* options, byte* vocab, nuint vocabLen, byte* ptr, nuint len, void* value, RawFaultDetail* fault) =>
		OperatingSystem.IsBrowser() ? cast_detailed_browser(door, param, format, periods, options, vocab, vocabLen, ptr, len, value, fault)
			: OperatingSystem.IsIOS() ? cast_detailed_internal(door, param, format, periods, options, vocab, vocabLen, ptr, len, value, fault)
			: cast_detailed_native(door, param, format, periods, options, vocab, vocabLen, ptr, len, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_detailed_from_f64")]
	private static unsafe partial int cast_detailed_from_f64_native(uint door, uint param, double number, void* value, RawFaultDetail* fault);
	[LibraryImport("*", EntryPoint = "cast_detailed_from_f64")]
	private static unsafe partial int cast_detailed_from_f64_browser(uint door, uint param, double number, void* value, RawFaultDetail* fault);
	[LibraryImport("__Internal", EntryPoint = "cast_detailed_from_f64")]
	private static unsafe partial int cast_detailed_from_f64_internal(uint door, uint param, double number, void* value, RawFaultDetail* fault);
	private static unsafe int cast_detailed_from_f64(uint door, uint param, double number, void* value, RawFaultDetail* fault) =>
		OperatingSystem.IsBrowser() ? cast_detailed_from_f64_browser(door, param, number, value, fault)
			: OperatingSystem.IsIOS() ? cast_detailed_from_f64_internal(door, param, number, value, fault)
			: cast_detailed_from_f64_native(door, param, number, value, fault);

	[LibraryImport("hypercast", EntryPoint = "cast_checked_detailed")]
	private static unsafe partial int cast_checked_detailed_native(uint door, uint param, RawNumFormat* format, RawConstraints* constraints, byte* enumPtr, nuint enumLen, byte* ptr, nuint len, void* value, RawFaultDetail* fault, RawCheck* check);
	[LibraryImport("*", EntryPoint = "cast_checked_detailed")]
	private static unsafe partial int cast_checked_detailed_browser(uint door, uint param, RawNumFormat* format, RawConstraints* constraints, byte* enumPtr, nuint enumLen, byte* ptr, nuint len, void* value, RawFaultDetail* fault, RawCheck* check);
	[LibraryImport("__Internal", EntryPoint = "cast_checked_detailed")]
	private static unsafe partial int cast_checked_detailed_internal(uint door, uint param, RawNumFormat* format, RawConstraints* constraints, byte* enumPtr, nuint enumLen, byte* ptr, nuint len, void* value, RawFaultDetail* fault, RawCheck* check);
	private static unsafe int cast_checked_detailed(uint door, uint param, RawNumFormat* format, RawConstraints* constraints, byte* enumPtr, nuint enumLen, byte* ptr, nuint len, void* value, RawFaultDetail* fault, RawCheck* check) =>
		OperatingSystem.IsBrowser() ? cast_checked_detailed_browser(door, param, format, constraints, enumPtr, enumLen, ptr, len, value, fault, check)
			: OperatingSystem.IsIOS() ? cast_checked_detailed_internal(door, param, format, constraints, enumPtr, enumLen, ptr, len, value, fault, check)
			: cast_checked_detailed_native(door, param, format, constraints, enumPtr, enumLen, ptr, len, value, fault, check);

	[LibraryImport("hypercast", EntryPoint = "cast_checked_detailed_from_f64")]
	private static unsafe partial int cast_checked_detailed_from_f64_native(uint door, uint param, RawConstraints* constraints, byte* enumPtr, nuint enumLen, double number, void* value, RawFaultDetail* fault, RawCheck* check);
	[LibraryImport("*", EntryPoint = "cast_checked_detailed_from_f64")]
	private static unsafe partial int cast_checked_detailed_from_f64_browser(uint door, uint param, RawConstraints* constraints, byte* enumPtr, nuint enumLen, double number, void* value, RawFaultDetail* fault, RawCheck* check);
	[LibraryImport("__Internal", EntryPoint = "cast_checked_detailed_from_f64")]
	private static unsafe partial int cast_checked_detailed_from_f64_internal(uint door, uint param, RawConstraints* constraints, byte* enumPtr, nuint enumLen, double number, void* value, RawFaultDetail* fault, RawCheck* check);
	private static unsafe int cast_checked_detailed_from_f64(uint door, uint param, RawConstraints* constraints, byte* enumPtr, nuint enumLen, double number, void* value, RawFaultDetail* fault, RawCheck* check) =>
		OperatingSystem.IsBrowser() ? cast_checked_detailed_from_f64_browser(door, param, constraints, enumPtr, enumLen, number, value, fault, check)
			: OperatingSystem.IsIOS() ? cast_checked_detailed_from_f64_internal(door, param, constraints, enumPtr, enumLen, number, value, fault, check)
			: cast_checked_detailed_from_f64_native(door, param, constraints, enumPtr, enumLen, number, value, fault, check);

	// A text door: the declarations it reads (null for those it does not), the input, and where the
	// value and the fault go. `fault` is stated at the size the layout needs, as RawFaultDetail.Empty.
	[MethodImpl(MethodImplOptions.AggressiveInlining)]
	static unsafe int Detailed(uint door, uint param, RawNumFormat* format, RawDayPeriods* periods,
		ReadOnlySpan<byte> utf8, void* value, RawFaultDetail* fault)
	{
		fixed (byte* ptr = utf8)
			return cast_detailed(door, param, format, periods, null, null, 0, ptr, (nuint)utf8.Length, value, fault);
	}

	// A date door under options: the options when something is declared (null otherwise) and the
	// vocabulary beside them, which the core judges in full on every call.
	static unsafe int Detailed(uint door, uint param, RawDayPeriods* periods, in DateOptions options,
		ReadOnlySpan<byte> utf8, void* value, RawFaultDetail* fault)
	{
		var declared = options.TryToRaw(out var raw);
		var months = options.Months is { } names ? names.Packed : default;
		fixed (byte* vocab = months)
		fixed (byte* ptr = utf8)
			return cast_detailed(door, param, null, periods, declared ? &raw : null, vocab, (nuint)months.Length,
				ptr, (nuint)utf8.Length, value, fault);
	}

	// Whether the core refuses a vocabulary, asked of the ordered date door with nothing to parse:
	// a declaration is judged before the input, so a valid one answers "empty" and a refused one -1.
	internal static unsafe bool RefusesMonthNames(ReadOnlySpan<byte> packed)
	{
		var fault = RawFaultDetail.Empty;
		RawDate value = default;
		fixed (byte* vocab = packed)
			return cast_detailed(DoorCodes.DateOrdered, (uint)DateOrder.YearMonthDay, null, null, null, vocab, (nuint)packed.Length,
				null, 0, &value, &fault) == -1;
	}
}
