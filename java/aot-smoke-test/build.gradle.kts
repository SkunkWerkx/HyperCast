// Proof that Cast's FFM downcalls survive GraalVM Native Image ahead-of-time compilation —
// mirrors csharp/HyperCast.AotSmokeTest, and full AOT is a HyperCast non-negotiable
// (docs/roadmap.md): `./gradlew :aot-smoke-test:nativeRun` under a GraalVM JAVA_HOME.
plugins {
    application
    id("org.graalvm.buildtools.native") version "1.1.10"
}

dependencies {
    implementation(rootProject)
}

application {
    mainClass.set("io.github.buvinghausen.hypercast.aotsmoketest.Main")
}

java {
    sourceCompatibility = JavaVersion.VERSION_22
    targetCompatibility = JavaVersion.VERSION_22
}

graalvmNative {
    binaries {
        named("main") {
            // Same restricted-method opt-in as the library's own test task, plus a build
            // report so a failed reachability/linking analysis is diagnosable.
            buildArgs.add("--enable-native-access=ALL-UNNAMED")
            buildArgs.add("-H:+ReportExceptionStackTraces")
            // Native Image doesn't embed classpath resources by default — without this,
            // Cast's getResourceAsStream("/native/{rid}/{lib}") finds nothing at runtime.
            resources {
                includedPatterns.add("native/.*")
            }
        }
    }
}
