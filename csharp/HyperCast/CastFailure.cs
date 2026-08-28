namespace HyperCast;

/// <summary>
/// The closed set of reasons a cast can fail — the native core's verdict codes, verbatim.
/// Adding a member is a deliberate breaking change: every exhaustive switch over this enum
/// becomes a build error until updated, in every binding at once.
/// </summary>
public enum CastFailure : byte
{
	/// <summary>Sentinel CLR default — never produced by any cast path (0 is "Ok" at the ABI).</summary>
	Unspecified = 0,

	/// <summary>Required input was empty or whitespace. The <c>*Optional</c> doors surface this as absent.</summary>
	Empty = 1,

	/// <summary>Input was present but not recognizable as the target type.</summary>
	Malformed = 2,

	/// <summary>
	/// Input was well-formed but the value falls outside the target's representable range —
	/// <c>"256"</c> for a <see cref="byte"/>, a timestamp past 9999-12-31, <c>1e400</c> for a
	/// <see cref="double"/>.
	/// </summary>
	OutOfRange = 3
}
