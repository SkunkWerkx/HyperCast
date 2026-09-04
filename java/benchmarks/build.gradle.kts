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
    // GraalWasm on the benchmark classpath only, so `./gradlew :benchmarks:jmh -Pwasm` can
    // run the same suite through the wasm32-wasip1 module — the README's WebAssembly table
    // is that run against the plain one. Absent the property nothing here loads it.
    jmh("org.graalvm.polyglot:polyglot:25.3.4.1")
    jmh("org.graalvm.polyglot:wasm:25.3.4.1")
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
    // -Pwasm: the same suite through the GraalWasm backend (see the dependencies above).
    // The short-warmup reasoning above does not hold on this path: under a GraalVM JDK
    // Truffle compiles the guest's own code at runtime, and the first seconds of a door
    // measure the interpreter and the compiler rather than the door, so the wasm run
    // warms up longer and measures longer.
    if (project.hasProperty("wasm")) {
        jvmArgsAppend.add("-Dhypercast.backend=wasm")
        jvmArgsAppend.add("-Dpolyglot.engine.WarnInterpreterOnly=false")
        warmupIterations.set(10)
        warmup.set("2s")
        iterations.set(10)
        timeOnIteration.set("2s")
    }
    // -PjmhInclude=<regex>: a subset of the suite, JMH's own include syntax.
    if (project.hasProperty("jmhInclude")) {
        includes.set(listOf(project.property("jmhInclude") as String))
    }
}
