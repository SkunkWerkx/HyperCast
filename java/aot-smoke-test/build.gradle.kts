// Proof that Cast's FFM downcalls survive GraalVM Native Image ahead-of-time compilation —
// mirrors csharp/HyperCast.AotSmokeTest, and full AOT is a HyperCast non-negotiable
// (docs/roadmap.md): `./gradlew :aot-smoke-test:nativeRun` under a GraalVM JAVA_HOME.
plugins {
    application
    id("org.graalvm.buildtools.native") version "1.1.10"
}

repositories {
    mavenCentral()
}

dependencies {
    implementation(rootProject)
    // -Pwasm: the same smoke test with GraalWasm on the classpath and the binary run with
    // -Dhypercast.backend=wasm, so the wasm path is proven under Native Image too — the
    // library's own reachability-metadata.json (the WasmBackend reflection entry, the
    // native/*/* resource glob) is what has to carry it, exactly as for the FFM path.
    // Off by default: the plain run keeps proving the FFM path with nothing extra linked in.
    if (project.hasProperty("wasm")) {
        runtimeOnly("org.graalvm.polyglot:polyglot:25.3.4.1")
        runtimeOnly("org.graalvm.polyglot:wasm:25.3.4.1")
    }
}

application {
    mainClass.set("io.github.skunkwerkx.hypercast.aotsmoketest.Main")
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
            if (project.hasProperty("wasm")) {
                runtimeArgs.add("-Dhypercast.backend=wasm")
            }
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
