// Local-dev-only JMH benchmarks — mirrors rust/benches (Criterion) and
// csharp/HyperCast.Benchmarks (BenchmarkDotNet): each door against the closest java.time /
// java.lang parse with equivalent settings declared. Run by hand with `./gradlew :benchmarks:jmh`.
plugins {
    java
    id("me.champeau.jmh") version "0.7.3"
}

repositories {
    mavenCentral()
}

dependencies {
    implementation(rootProject)
}

java {
    sourceCompatibility = JavaVersion.VERSION_22
    targetCompatibility = JavaVersion.VERSION_22
}

jmh {
    warmupIterations.set(3)
    iterations.set(5)
    fork.set(1)
    // HyperUuid's start settings, ported: the iteration *counts* are meaningless without
    // the durations below — JMH's default is 10 seconds per iteration, which is how a suite
    // this size turns into a quarter of an hour producing numbers that converged in the
    // second warmup. One second per iteration still samples tens of thousands of operations
    // at these speeds, and this workload is an unusually safe candidate for a short run:
    // every benchmark is an FFM downcall over caller bytes across a native boundary, so the
    // JIT effects a long warmup exists to out-wait — dead-code elimination, constant
    // folding, hoisting the call out of the loop — cannot apply to it. The README's 0.2.0
    // table came from a 2-fork, 5+10 run of the same suite; raise these again only if a
    // benchmark measuring pure Java work is ever added, where those hazards are real.
    warmupForks.set(0)
    timeOnIteration.set("1s")
    warmup.set("1s")
    // Allocation per call is a receipt here, not a claim — the same role [MemoryDiagnoser]
    // plays for the C# suite. gc.alloc.rate.norm is the B/op column the README quotes.
    profilers.set(listOf("gc"))
    // Cast's FFM downcalls are a "restricted method" — the JMH-forked JVM needs the same
    // opt-in the library's own test task already sets.
    jvmArgsAppend.add("--enable-native-access=ALL-UNNAMED")
}
