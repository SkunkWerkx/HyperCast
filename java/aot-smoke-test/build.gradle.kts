// Proof that Cast's FFM downcalls survive GraalVM Native Image ahead-of-time compilation —
// mirrors csharp/HyperCast.AotSmokeTest, and full AOT is a HyperCast non-negotiable
// (docs/roadmap.md): `./gradlew :aot-smoke-test:nativeRun` under a GraalVM JAVA_HOME.
plugins {
    application
    id("org.graalvm.buildtools.native") version "1.1.14"
}

repositories {
    mavenCentral()
}

dependencies {
    implementation(rootProject)
}

application {
    mainClass.set("io.github.skunkwerkx.hypercast.aotsmoketest.Main")
    // `:aot-smoke-test:run` is the same program on an ordinary JVM, with the flag a consumer
    // passes (README.md's "Native access").
    applicationDefaultJvmArgs = listOf("--enable-native-access=ALL-UNNAMED")
}

java {
    sourceCompatibility = JavaVersion.VERSION_25
    targetCompatibility = JavaVersion.VERSION_25
}

// The library's own floor, enforced the same way it is there.
tasks.withType<JavaCompile>().configureEach {
    options.release = 25
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
            // The glob now ships inside the library, in its own reachability-metadata.json
            // alongside the FFM downcall signatures, so a consumer inherits it with zero
            // configuration. This test is the thing that proves that, and it can only prove
            // it by not configuring it itself.
        }
    }
}
