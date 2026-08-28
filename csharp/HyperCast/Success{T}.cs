namespace HyperCast;

/// <summary>The success case of <see cref="Verdict{T}"/>: a cast value.</summary>
/// <typeparam name="T">The cast value's type — always a value type in this library.</typeparam>
/// <param name="Value">The cast value.</param>
public readonly record struct Success<T>(T Value) where T : struct;
