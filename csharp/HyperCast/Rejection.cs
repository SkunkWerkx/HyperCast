using System.Runtime.CompilerServices;

namespace HyperCast;

/// <summary>
/// The failure case of <see cref="CheckedVerdict{T}"/>: exactly one of <see cref="Fault"/>
/// (it did not parse — the type every plain door already returns, unchanged) or
/// <see cref="Violation"/> (it parsed and broke a declared rule), as a native C# union. The
/// two are the only kinds of failure: parsing keeps its own lexicon, and a rule is never a
/// fifth <see cref="CastFailure"/>.
/// </summary>
/// <remarks>
/// <para>
/// A two-arm switch over <see cref="Fault"/> and <see cref="Violation"/> is exhaustive. Without
/// C# 15, use <see cref="Match{TResult}"/>, <see cref="Switch"/> or the
/// <see cref="TryGetValue(out Fault)"/> / <see cref="TryGetValue(out Violation)"/> pair, as on
/// <see cref="Verdict{T}"/>.
/// </para>
/// <para>
/// <b>Do not use <c>default(Rejection)</c>.</b> As with <see cref="Verdict{T}"/>, a defaulted
/// value is neither case.
/// </para>
/// </remarks>
[Union]
public readonly record struct Rejection : IUnion
{
	enum State : byte
	{
		Default = 0,
		Fault = 1,
		Violation = 2
	}

	readonly Fault _fault;
	readonly Violation _violation;
	readonly State _state;

	/// <summary>Creates a parse failure. Also reachable as an implicit union conversion.</summary>
	/// <param name="value">The conversion fault.</param>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="value"/> carries the <see cref="CastFailure.Unspecified"/> sentinel.</exception>
	public Rejection(Fault value)
	{
		if (value.Reason == CastFailure.Unspecified)
			throw new ArgumentOutOfRangeException(nameof(value), value.Reason,
				"Fault must carry a real reason; default(Fault) is not a valid case value.");
		_fault = value;
		_state = State.Fault;
	}

	/// <summary>Creates a rule failure. Also reachable as an implicit union conversion.</summary>
	/// <param name="value">The violation.</param>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="value"/> names no keyword (code 0, a <c>default(Violation)</c>).</exception>
	public Rejection(Violation value)
	{
		if (value.Keyword.Code == 0)
			throw new ArgumentOutOfRangeException(nameof(value), value.Keyword.Code,
				"Violation must name a keyword; default(Violation) is not a valid case value.");
		_violation = value;
		_state = State.Violation;
	}

	/// <summary>
	/// The boxed case contents, or <see langword="null"/> for a defaulted value.
	/// Pattern matching does not read this property; a direct read boxes.
	/// </summary>
	public object? Value =>
		_state switch
		{
			State.Fault => _fault,
			State.Violation => _violation,
			_ => null,
		};

	/// <summary><see langword="true"/> unless this value was defaulted rather than constructed.</summary>
	public bool HasValue => _state != State.Default;

	/// <summary>Retrieves the parse failure without boxing.</summary>
	/// <param name="value">The fault when present; default otherwise.</param>
	/// <returns><see langword="true"/> if this rejection is a fault.</returns>
	public bool TryGetValue(out Fault value)
	{
		value = _fault;
		return _state == State.Fault;
	}

	/// <summary>Retrieves the rule failure without boxing.</summary>
	/// <param name="value">The violation when present; default otherwise.</param>
	/// <returns><see langword="true"/> if this rejection is a violation.</returns>
	public bool TryGetValue(out Violation value)
	{
		value = _violation;
		return _state == State.Violation;
	}

	/// <summary>Maps this rejection through the delegate for its case: the union switch, for a consumer without C# 15.</summary>
	/// <typeparam name="TResult">What both delegates return.</typeparam>
	/// <param name="fault">Called with a parse failure.</param>
	/// <param name="violation">Called with a rule failure.</param>
	/// <returns>The return value of the delegate that ran.</returns>
	/// <exception cref="ArgumentNullException"><paramref name="fault"/> or <paramref name="violation"/> is <see langword="null"/>.</exception>
	/// <exception cref="InvalidOperationException">This is <c>default(Rejection)</c>, which is neither case.</exception>
	public TResult Match<TResult>(Func<Fault, TResult> fault, Func<Violation, TResult> violation)
	{
		ArgumentNullException.ThrowIfNull(fault);
		ArgumentNullException.ThrowIfNull(violation);
		return _state switch
		{
			State.Fault => fault(_fault),
			State.Violation => violation(_violation),
			_ => throw Defaulted(),
		};
	}

	/// <summary>Runs the action for this rejection's case: the union switch statement, for a consumer without C# 15.</summary>
	/// <param name="fault">Run with a parse failure.</param>
	/// <param name="violation">Run with a rule failure.</param>
	/// <exception cref="ArgumentNullException"><paramref name="fault"/> or <paramref name="violation"/> is <see langword="null"/>.</exception>
	/// <exception cref="InvalidOperationException">This is <c>default(Rejection)</c>, which is neither case.</exception>
	public void Switch(Action<Fault> fault, Action<Violation> violation)
	{
		ArgumentNullException.ThrowIfNull(fault);
		ArgumentNullException.ThrowIfNull(violation);
		switch (_state)
		{
			case State.Fault:
				fault(_fault);
				break;
			case State.Violation:
				violation(_violation);
				break;
			default:
				throw Defaulted();
		}
	}

	static InvalidOperationException Defaulted() =>
		new("default(Rejection) is neither a fault nor a violation; construct a rejection instead.");

	/// <summary>Renders "Fault(Reason @ offset+length)", "Violation(keyword @ offset+length)", or "Default(invalid)".</summary>
	public override string ToString() =>
		_state switch
		{
			State.Fault => $"Fault({_fault.Reason} @ {_fault.Offset}+{_fault.Length})",
			State.Violation => $"Violation({_violation.Keyword} @ {_violation.Offset}+{_violation.Length})",
			_ => "Default(invalid)",
		};
}
