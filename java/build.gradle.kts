plugins {
    `java-library`
    `maven-publish`
}

group = "io.github.buvinghausen"
// CI overrides this (0.1.0-ci.<run_number>) via HYPERCAST_VERSION so repeated manual
// workflow_dispatch runs during testing don't collide with an already-published version.
version = System.getenv("HYPERCAST_VERSION") ?: "0.1.0"

repositories {
    mavenCentral()
}

dependencies {
    testImplementation(platform("org.junit:junit-bom:6.1.3"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testImplementation("com.google.code.gson:gson:2.11.0")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

// Local dev loop, mirroring the C# binding's csproj copy of the freshly-built core: when
// the Rust cdylib exists in-repo (a release-profile cargo build in ../rust), stage it as
// the classpath resource /native/{rid}/{lib} the loader expects. CI overlays every
// platform's build into the same layout before packaging.
val nativeRid = run {
    val osName = System.getProperty("os.name").lowercase()
    val isArm = System.getProperty("os.arch").lowercase().let { it.contains("aarch64") || it.contains("arm") }
    when {
        osName.contains("win") -> if (isArm) "win-arm64" else "win-x64"
        osName.contains("mac") || osName.contains("darwin") -> if (isArm) "osx-arm64" else "osx-x64"
        else -> if (isArm) "linux-arm64" else "linux-x64"
    }
}

val stageNativeLibrary = tasks.register<Copy>("stageNativeLibrary") {
    from("../rust/target/release") {
        include("libhypercast.so", "libhypercast.dylib", "hypercast.dll")
    }
    into(layout.buildDirectory.dir("generated-resources/native/$nativeRid"))
}

sourceSets.main {
    resources.srcDir(layout.buildDirectory.dir("generated-resources"))
}

tasks.processResources {
    dependsOn(stageNativeLibrary)
}

tasks.test {
    useJUnitPlatform()
    // Cast's FFM downcalls are a "restricted method" — silences the runtime warning today
    // and avoids them being blocked outright in a future JDK.
    jvmArgs("--enable-native-access=ALL-UNNAMED")
}

java {
    // 22 is the floor: java.lang.foreign is stable, non-preview only from JDK 22 (JEP 454)
    // onward, and the Verdict union's whole point — sealed interface + record patterns +
    // exhaustive switch, Java's native discriminated union — is stable since 21. The same
    // reasoning that put .NET 11 under the C# binding, at Java's own version numbers.
    sourceCompatibility = JavaVersion.VERSION_22
    targetCompatibility = JavaVersion.VERSION_22
    withSourcesJar()
}

publishing {
    publications {
        create<MavenPublication>("maven") {
            from(components["java"])
            pom {
                name.set("hypercast")
                description.set(
                    "Allocation-free scalar parsing — booleans, numerics, UUIDs, temporals — " +
                        "as a sealed-interface Verdict union (value or reason + offending span, " +
                        "never an exception), FFM bindings straight into a native Rust core " +
                        "(libhypercast). No runtime bridge, no reflection, no extra dependency."
                )
                url.set("https://github.com/SkunkWerkx/HyperCast")
                licenses {
                    license {
                        name.set("MIT")
                    }
                }
            }
        }
    }
    repositories {
        maven {
            name = "GitHubPackages"
            url = uri("https://maven.pkg.github.com/SkunkWerkx/HyperCast")
            credentials {
                username = System.getenv("GITHUB_ACTOR")
                password = System.getenv("GITHUB_TOKEN")
            }
        }
    }
}
