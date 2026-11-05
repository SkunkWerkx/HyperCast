// Local-dev-only JMH benchmarks — mirrors rust/benches (Criterion) and
// csharp/HyperCast.Benchmarks (BenchmarkDotNet): each door against the closest java.time /
// java.lang parse with equivalent settings declared. Run by hand with `./gradlew :benchmarks:jmh`.
plugins {
    java
    id("me.champeau.jmh") version "0.7.4"
}

repositories {
    mavenCentral()
}

dependencies {
    implementation(project(":"))
}

val jdkFloor = rootProject.extra["jdkFloor"] as Int

// Compiled for the library's floor the way its base is (-source/-target, not --release: the
// slices are MemorySegments, a preview API in 21's signatures).
java {
    sourceCompatibility = JavaVersion.toVersion(jdkFloor)
    targetCompatibility = JavaVersion.toVersion(jdkFloor)
}

tasks.withType<JavaCompile>().configureEach {
    options.compilerArgs.add("-Xlint:-options")
}

// The jmh jar bundles the library's classes; without this attribute the JDK reads only the
// jar's base and a 25 JVM silently benchmarks the JNI path. The @Setup check fails on that.
tasks.named<Jar>("jmhJar") {
    manifest {
        attributes("Multi-Release" to "true")
    }
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
    // Cast's FFM downcalls (and on 24, the JNI path's System.load) are "restricted
    // methods" — the JMH-forked JVM needs the same opt-in the library's own test task sets.
    jvmArgsAppend.add("--enable-native-access=ALL-UNNAMED")
    // -PjmhJdk=<feature>: fork the benchmark JVMs on that JDK through the toolchain (the
    // floor, say), the Gradle JVM staying on 25. The default is the Gradle JVM itself.
    if (project.hasProperty("jmhJdk")) {
        val launcher = javaToolchains.launcherFor {
            languageVersion = JavaLanguageVersion.of(project.property("jmhJdk").toString().toInt())
        }
        jvm.set(launcher.map { it.executablePath.asFile.absolutePath })
    }
    // -PjmhInclude=<regex>: a subset of the suite, JMH's own include syntax.
    if (project.hasProperty("jmhInclude")) {
        includes.set(listOf(project.property("jmhInclude") as String))
    }
}

// A benchmark that throws (the @Setup backend check, say) leaves JMH printing the exception and
// the build green: the plugin's failOnError hands JMH `-foe 1`, which JMH reads as false, and it
// exposes no way to pass a raw `-foe true`. A run that measured nothing writes a results file
// with no rows, so the task fails on that.
tasks.named("jmh") {
    val results = layout.buildDirectory.file("results/jmh/results.txt")
    doLast {
        val rows = results.get().asFile.readLines().count { Regex("""^\S+\s+(avgt|thrpt|sample|ss)\s+\d+""").containsMatchIn(it) }
        check(rows > 0) { "jmh measured nothing; a benchmark failed (see the exception above)" }
    }
}
