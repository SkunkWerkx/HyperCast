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
    // Cast's FFM downcalls are a "restricted method" — the JMH-forked JVM needs the same
    // opt-in the library's own test task already sets.
    jvmArgsAppend.add("--enable-native-access=ALL-UNNAMED")
}
