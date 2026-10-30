using BenchmarkDotNet.Running;
using HyperCast.Benchmarks;

BenchmarkSwitcher.FromAssembly(typeof(CastBenchmarks).Assembly).Run(args);
