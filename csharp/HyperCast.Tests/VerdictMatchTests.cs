namespace HyperCast.Tests;

/// <summary>
/// <see cref="Verdict{T}.Match{TResult}"/> and <see cref="Verdict{T}.Switch"/>: the path a
/// consumer without C# 15's unions has, on both frameworks. Each takes both cases, so a missing
/// one is a compile error, and each refuses a defaulted verdict the way the union switch does.
/// </summary>
public sealed class VerdictMatchTests
{
	[Fact]
	public void Match_calls_success_for_a_success() =>
		Cast.Int32("42", NumFormat.Invariant).Match(s => s.Value, f => -1).ShouldBe(42);

	[Fact]
	public void Match_calls_fault_for_a_fault() =>
		Cast.Int32("abc", NumFormat.Invariant).Match(s => CastFailure.Unspecified, f => f.Reason).ShouldBe(CastFailure.Malformed);

	[Fact]
	public void Switch_runs_exactly_one_action()
	{
		int successes = 0, faults = 0;
		Cast.Int32("42", NumFormat.Invariant).Switch(_ => successes++, _ => faults++);
		(successes, faults).ShouldBe((1, 0));

		successes = faults = 0;
		Cast.Int32("abc", NumFormat.Invariant).Switch(_ => successes++, _ => faults++);
		(successes, faults).ShouldBe((0, 1));
	}

	[Fact]
	public void Match_and_Switch_throw_on_a_defaulted_verdict()
	{
		var calls = 0;
		Should.Throw<InvalidOperationException>(() => default(Verdict<int>).Match(_ => ++calls, _ => ++calls))
			.Message.ShouldContain("default(Verdict<T>)");
		Should.Throw<InvalidOperationException>(() => default(Verdict<int>).Switch(_ => calls++, _ => calls++))
			.Message.ShouldContain("default(Verdict<T>)");
		calls.ShouldBe(0);
	}

	[Fact]
	public void Match_and_Switch_reject_null_delegates()
	{
		var verdict = Cast.Int32("42", NumFormat.Invariant);
		Should.Throw<ArgumentNullException>(() => verdict.Match<int>(null!, _ => 0)).ParamName.ShouldBe("success");
		Should.Throw<ArgumentNullException>(() => verdict.Match(_ => 0, null!)).ParamName.ShouldBe("fault");
		Should.Throw<ArgumentNullException>(() => verdict.Switch(null!, _ => { })).ParamName.ShouldBe("success");
		Should.Throw<ArgumentNullException>(() => verdict.Switch(_ => { }, null!)).ParamName.ShouldBe("fault");
	}
}
