namespace HyperCast;

/// <summary>
/// The declared unit of a Unix-epoch value. There is no magnitude guessing — the caller
/// states the unit, so a bare number is never silently interpreted as seconds or
/// milliseconds. Values match the native core's discriminants.
/// </summary>
public enum UnixPrecision : uint
{
	/// <summary>Sentinel CLR default — never a valid precision; rejected by the Unix door.</summary>
	Unspecified = 0,

	/// <summary>Seconds since 1970-01-01T00:00:00Z.</summary>
	Seconds = 1,

	/// <summary>Milliseconds since the epoch.</summary>
	Milliseconds = 2,

	/// <summary>Microseconds since the epoch.</summary>
	Microseconds = 3,

	/// <summary>Nanoseconds since the epoch.</summary>
	Nanoseconds = 4
}
