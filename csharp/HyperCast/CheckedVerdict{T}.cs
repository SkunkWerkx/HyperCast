using System.Runtime.CompilerServices;

namespace HyperCast;

/// <summary>
/// The outcome of a checked cast — a door with declared <see cref="Constraints{T}"/>:
/// exactly one of <see cref="Checked{T}"/> (the value obeys every rule) or
/// <see cref="Rejection"/> (a <see cref="Fault"/> or a <see cref="Violation"/>), as a native
/// C# union beside <see cref="Verdict{T}"/>, which the plain doors keep returning. Nothing is
/// ever thrown for data.
/// </summary>
/// <remarks>
/// <para>
/// A two-arm switch over <see cref="Checked{T}"/> and <see cref="Rejection"/> is exhaustive,
/// and so is the nested one over the rejection's <see cref="Fault"/> and
/// <see cref="Violation"/>. Without C# 15, use <see cref="Match{TResult}"/>,
/// <see cref="Switch"/> or the <see cref="TryGetValue(out Checked{T})"/> /
/// <see cref="TryGetValue(out Rejection)"/> pair, as on <see cref="Verdict{T}"/>.
/// </para>
/// <para>
/// <b>Do not use <c>default(CheckedVerdict&lt;T&gt;)</c>.</b> As with <see cref="Verdict{T}"/>, a
/// defaulted value is neither case.
/// </para>
/// </remarks>
/// <typeparam name="T">The door's type: a number, or <see cref="string"/> for the text door.</typeparam>
[Union]
public readonly record struct CheckedVerdict<T> : IUnion where T : notnull
{
	enum State : byte
	{
		Default = 0,
		Checked = 1,
		Rejection = 2
	}

	readonly Checked<T> _checked;
	readonly Rejection _rejection;
	readonly State _state;

	/// <summary>Creates a successful verdict. Also reachable as an implicit union conversion.</summary>
	/// <param name="value">The success case.</param>
	public CheckedVerdict(Checked<T> value)
	{
		_checked = value;
		_state = State.Checked;
	}

	/// <summary>Creates a failed verdict. Also reachable as an implicit union conversion.</summary>
	/// <param name="value">The rejection.</param>
	/// <exception cref="ArgumentOutOfRangeException"><paramref name="value"/> is a <c>default(Rejection)</c>.</exception>
	public CheckedVerdict(Rejection value)
	{
		if (!value.HasValue)
			throw new ArgumentOutOfRangeException(nameof(value), "default(Rejection) is not a valid case value.");
		_rejection = value;
		_state = State.Rejection;
	}

	/// <summary>
	/// The boxed case contents, or <see langword="null"/> for a defaulted value.
	/// Pattern matching does not read this property; a direct read boxes.
	/// </summary>
	public object? Value =>
		_state switch
		{
			State.Checked => _checked,
			State.Rejection => _rejection,
			_ => null,
		};

	/// <summary><see langword="true"/> unless this value was defaulted rather than constructed.</summary>
	public bool HasValue => _state != State.Default;

	/// <summary>Retrieves the success case without boxing.</summary>
	/// <param name="value">The success case when present; default otherwise.</param>
	/// <returns><see langword="true"/> if this verdict is the success case.</returns>
	public bool TryGetValue(out Checked<T> value)
	{
		value = _checked;
		return _state == State.Checked;
	}

	/// <summary>Retrieves the failure case without boxing.</summary>
	/// <param name="value">The rejection when present; default otherwise.</param>
	/// <returns><see langword="true"/> if this verdict is the failure case.</returns>
	public bool TryGetValue(out Rejection value)
	{
		value = _rejection;
		return _state == State.Rejection;
	}

	/// <summary>Maps this verdict through the delegate for its case: the union switch, for a consumer without C# 15.</summary>
	/// <typeparam name="TResult">What both delegates return.</typeparam>
	/// <param name="success">Called with the success case.</param>
	/// <param name="rejection">Called with the failure case.</param>
	/// <returns>The return value of the delegate that ran.</returns>
	/// <exception cref="ArgumentNullException"><paramref name="success"/> or <paramref name="rejection"/> is <see langword="null"/>.</exception>
	/// <exception cref="InvalidOperationException">This is <c>default(CheckedVerdict&lt;T&gt;)</c>, which is neither case.</exception>
	public TResult Match<TResult>(Func<Checked<T>, TResult> success, Func<Rejection, TResult> rejection)
	{
		ArgumentNullException.ThrowIfNull(success);
		ArgumentNullException.ThrowIfNull(rejection);
		return _state switch
		{
			State.Checked => success(_checked),
			State.Rejection => rejection(_rejection),
			_ => throw Defaulted(),
		};
	}

	/// <summary>Runs the action for this verdict's case: the union switch statement, for a consumer without C# 15.</summary>
	/// <param name="success">Run with the success case.</param>
	/// <param name="rejection">Run with the failure case.</param>
	/// <exception cref="ArgumentNullException"><paramref name="success"/> or <paramref name="rejection"/> is <see langword="null"/>.</exception>
	/// <exception cref="InvalidOperationException">This is <c>default(CheckedVerdict&lt;T&gt;)</c>, which is neither case.</exception>
	public void Switch(Action<Checked<T>> success, Action<Rejection> rejection)
	{
		ArgumentNullException.ThrowIfNull(success);
		ArgumentNullException.ThrowIfNull(rejection);
		switch (_state)
		{
			case State.Checked:
				success(_checked);
				break;
			case State.Rejection:
				rejection(_rejection);
				break;
			default:
				throw Defaulted();
		}
	}

	static InvalidOperationException Defaulted() =>
		new("default(CheckedVerdict<T>) is neither a success nor a rejection; construct a verdict instead.");

	/// <summary>
	/// Renders "Checked(value)" (with ", adjusted" and ", enum N" when they apply), the
	/// rejection's own rendering, or "Default(invalid)".
	/// </summary>
	public override string ToString() =>
		_state switch
		{
			State.Checked => $"Checked({_checked.Value}{(_checked.Adjusted ? ", adjusted" : "")}{(_checked.EnumIndex is { } index ? $", enum {index}" : "")})",
			State.Rejection => _rejection.ToString(),
			_ => "Default(invalid)",
		};
}
