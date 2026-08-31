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
    // Full-length: the README's first table came from a deliberately shortened run with
    // error bars too wide to publish as receipts. Two forks (JIT variance between JVM
    // instances is real), 5 warmup + 10 measurement iterations, 1s each — plenty of ops
    // per iteration at nanosecond scale, and ~15 minutes for the whole suite.
    warmupIterations.set(5)
    iterations.set(10)
    fork.set(2)
    timeOnIteration.set("1s")
    warmup.set("1s")
    // Cast's FFM downcalls are a "restricted method" — the JMH-forked JVM needs the same
    // opt-in the library's own test task already sets.
    jvmArgsAppend.add("--enable-native-access=ALL-UNNAMED")
}
