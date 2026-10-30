// .NET 10's BCL has neither type C# 15's compiler looks for on a union, so net10.0 builds
// declare them here, internal: not public surface, never in a consumer's way, and absent
// from the net11.0 assembly, which takes the BCL's. Goes when net10.0 does.
#if !NET11_0_OR_GREATER
namespace System.Runtime.CompilerServices
{
	[AttributeUsage(AttributeTargets.Class | AttributeTargets.Struct, Inherited = false)]
	sealed class UnionAttribute : Attribute;
	interface IUnion { object? Value { get; } }
}
#endif
