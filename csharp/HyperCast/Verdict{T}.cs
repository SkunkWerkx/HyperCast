using System.Runtime.CompilerServices;

namespace HyperCast;

/// <summary>
/// The outcome of a cast: exactly one of <see cref="Success{T}"/> or <see cref="Fault"/>,
/// as a native C# union — the failure reason travels in the type, never as an exception.
/// </summary>
/// <remarks>
/// <para>
/// <b>Pattern matching unwraps to the case types.</b> Match against
/// <see cref="Success{T}"/> or <see cref="Fault"/> — a two-arm switch over both case types
/// is exhaustive, and with this repo's build settings (CS8509 as error, see
/// <c>csharp/Directory.Build.props</c>) an unhandled disposition is a compile failure.
/// Consumers are encouraged to elevate CS8509 the same way.
/// </para>
/// <para>
/// <b>Do not use <c>default(Verdict&lt;T&gt;)</c>.</b> Like
/// <c>default(ImmutableArray&lt;T&gt;)</c>, a defaulted value is malformed by construction:
/// union well-formedness requires its <see cref="Value"/> to be null, so it matches neither
/// case and an exhaustive switch throws <see cref="SwitchExpressionException"/> at first
/// consumption.
/// </para>
/// <para>
/// Hand-implements the union pattern (rather than a shorthand <c>union</c> declaration) so
/// both cases are stored inline and nothing boxes on either path — the same trade
/// Svartalfheim's <c>Result&lt;T&gt;</c> documents. The compiler routes pattern matching
/// through <see cref="TryGetValue(out Success{T})"/> / <see cref="TryGetValue(out Fault)"/>;
/// only a direct read of <see cref="Value"/> boxes.
/// </para>
/// </remarks>
/// <typeparam name="T">The cast value's type — always a value type in this library.</typeparam>
[Union]
public readonly record struct Verdict<T> : IUnion where T : struct
{
	enum State : byte
	{
		Default = 0,
		Success = 1,
		Fault = 2
	}

	readonly Success<T> _success;
	readonly Fault _fault;
	readonly State _state;

	/// <summary>Creates a successful verdict. Also reachable as an implicit union conversion.</summary>
	/// <param name="value">The success case.</param>
	public Verdict(Success<T> value)
	{
		_success = value;
		_state = State.Success;
	}

	/// <summary>Creates a failed verdict. Also reachable as an implicit union conversion.</summary>
	/// <param name="value">The conversion fault.</param>
	/// <exception cref="ArgumentOutOfRangeException">
	/// <paramref name="value"/> carries the <see cref="CastFailure.Unspecified"/> sentinel —
	/// a <c>default(Fault)</c> smuggled in as a case value.
	/// </exception>
	public Verdict(Fault value)
	{
		if (value.Reason == CastFailure.Unspecified)
			throw new ArgumentOutOfRangeException(nameof(value), value.Reason,
				"Fault must carry a real reason; default(Fault) is not a valid case value.");
		_fault = value;
		_state = State.Fault;
	}

	/// <summary>Wraps a cast value as the success case via plain assignment.</summary>
	/// <param name="value">The cast value.</param>
	public static implicit operator Verdict<T>(T value) => new(new Success<T>(value));

	/// <summary>
	/// The boxed case contents, or <see langword="null"/> for a defaulted value.
	/// Pattern matching does not read this property; a direct read boxes.
	/// </summary>
	public object? Value =>
		_state switch
		{
			State.Success => _success,
			State.Fault => _fault,
			_ => null,
		};

	/// <summary><see langword="true"/> unless this value was defaulted rather than constructed.</summary>
	public bool HasValue => _state != State.Default;

	/// <summary>Retrieves the success case without boxing.</summary>
	/// <param name="value">The success case when present; default otherwise.</param>
	/// <returns><see langword="true"/> if this verdict is the success case.</returns>
	public bool TryGetValue(out Success<T> value)
	{
		value = _success;
		return _state == State.Success;
	}

	/// <summary>Retrieves the failure case without boxing.</summary>
	/// <param name="value">The fault when present; default otherwise.</param>
	/// <returns><see langword="true"/> if this verdict is the failure case.</returns>
	public bool TryGetValue(out Fault value)
	{
		value = _fault;
		return _state == State.Fault;
	}

	/// <summary>Renders "Success(value)", "Fault(Reason @ offset+length)", or "Default(invalid)".</summary>
	public override string ToString() =>
		_state switch
		{
			State.Success => $"Success({_success.Value})",
			State.Fault => $"Fault({_fault.Reason} @ {_fault.Offset}+{_fault.Length})",
			_ => "Default(invalid)",
		};
}
