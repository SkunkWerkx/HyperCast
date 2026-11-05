// Proof that Cast's FFM downcalls survive GraalVM Native Image ahead-of-time compilation —
// mirrors csharp/HyperCast.AotSmokeTest, and full AOT is a HyperCast non-negotiable
// (docs/roadmap.md): `./gradlew :aot-smoke-test:nativeRun` under a GraalVM JAVA_HOME.
// `./gradlew :aot-smoke-test:run` is the same program on an ordinary JVM, and
// `:aot-smoke-test:runFloor` the same on the floor JDK, where the jar's JNI base loads
// instead. The image keeps only the FFM path and no JNI library.
plugins {
    application
    id("org.graalvm.buildtools.native") version "1.1.18"
}

repositories {
    mavenCentral()
}

dependencies {
    implementation(project(":"))
}

application {
    mainClass.set("io.github.skunkwerkx.hypercast.aotsmoketest.Main")
    // `:aot-smoke-test:run` is the same program on an ordinary JVM, with the flag a consumer
    // passes (README.md's "Native access").
    applicationDefaultJvmArgs = listOf("--enable-native-access=ALL-UNNAMED")
}

// The library's own floor, from the root project so the two cannot drift. Gradle and every
// compiler stay on 25 (a JDK 21 javac rejects the library's java25 tree), so only the
// launcher below runs on the floor.
val jdkFloor = rootProject.extra["jdkFloor"] as Int

// Compiled for the floor the way the library's base is, -source/-target rather than
// --release, and for the same reason: this program drives the MemorySegment doors, and
// java.lang.foreign is a preview API in 21's signatures. runFloor running it on 21 is the
// proof that nothing newer slipped in.
java {
    sourceCompatibility = JavaVersion.toVersion(jdkFloor)
    targetCompatibility = JavaVersion.toVersion(jdkFloor)
}

tasks.withType<JavaCompile>().configureEach {
    options.compilerArgs.add("-Xlint:-options")
}

// `run` on the floor JDK, through the toolchain: the jar's JNI base end to end, the same
// launcher as the library's testFloor.
tasks.register<JavaExec>("runFloor") {
    group = "application"
    description = "Runs the smoke test on the floor JDK ($jdkFloor)."
    classpath = sourceSets.main.get().runtimeClasspath
    mainClass = application.mainClass
    jvmArgs("--enable-native-access=ALL-UNNAMED")
    javaLauncher = javaToolchains.launcherFor {
        languageVersion = JavaLanguageVersion.of(jdkFloor)
    }
}

graalvmNative {
    binaries {
        named("main") {
            // Same restricted-method opt-in as the library's own test task, plus a build
            // report so a failed reachability/linking analysis is diagnosable.
            buildArgs.add("--enable-native-access=ALL-UNNAMED")
            buildArgs.add("-H:+ReportExceptionStackTraces")
            // Deliberately NO `resources { includedPatterns.add("native/.*") }` here.
            //
            // Native Image doesn't embed classpath resources by default, so without that
            // glob somewhere, Cast's getResourceAsStream("/native/{rid}/{lib}") finds
            // nothing at runtime — the binary builds clean and dies on first call. It used
            // to live here, which meant this smoke test proved only that *this project*
            // could be configured to work, never that a consumer's own native-image build
            // could. Found by building a real native image against the published 0.0.1 jar
            // from Maven Central: it failed with "classpath resource not found" while this
            // test was green.
            //
            // The globs now ship inside the library, in its own reachability-metadata.json
            // alongside the FFM downcall signatures (the core library's three names, never
            // the JNI library's), so a consumer inherits them with zero configuration. This test is the thing that proves that, and it can only prove
            // it by not configuring it itself.
        }
    }
}
