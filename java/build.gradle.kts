import org.gradle.external.javadoc.StandardJavadocDocletOptions

plugins {
    `java-library`
    `maven-publish`
    id("com.vanniktech.maven.publish") version "0.37.0"
}

// io.github.skunkwerkx — the SkunkWerkx org's own Central-Support-approved Portal namespace,
// proven live by HyperUuid's v0.1.0 publish (HyperUuid's very first publish went out under
// an interim personal-account namespace; this repo never needs that step — it starts on the
// org coordinate directly).
group = "io.github.skunkwerkx"
// CI overrides this (0.1.0-ci.<run_number>) via HYPERCAST_VERSION so repeated manual
// workflow_dispatch runs during testing don't collide with an already-published version —
// the real Maven Central publish (release.yml, tag-triggered) never sets that env var, so
// it always uses this committed version as-is.
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

// Ships the license text and this binding's README inside the jar, under META-INF/ (the
// conventional home for both). Gradle copies from anywhere on disk, so the repo root's
// LICENSE is referenced directly — no local copy, unlike the gem and the wheel, whose
// packers both reject a parent path outright. The POM's <licenses> block stays the
// machine-readable declaration; this is the text itself, for consumers who vendor the jar.
tasks.jar {
    metaInf {
        from("../LICENSE")
        from("README.md")
    }
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

// javadoc's own doclint already flags a missing comment/@param/@return as a WARNING by
// default — -Xwerror promotes those warnings to build-failing errors, so an undocumented
// public member can't ship silently. Central Portal requires a javadoc jar for every
// artifact anyway (see mavenPublishing below), so this is enforcing a real publish
// prerequisite, not just style.
tasks.javadoc {
    (options as StandardJavadocDocletOptions).addBooleanOption("Xwerror", true)
}

// mavenPublishing {} (com.vanniktech.maven.publish) owns the "maven" publication itself —
// sources/javadoc jars, POM, and the Central Portal repository target all come from here, not
// from a manually created MavenPublication (that would collide: the plugin creates one named
// "maven" too). publishToMavenCentral() targets the new Central Publisher Portal, not the
// dead OSSRH/Nexus staging API. Credentials (mavenCentralUsername/mavenCentralPassword, from
// the Central Portal's own token generator — not a raw Sonatype account password) and the
// signing key come from ORG_GRADLE_PROJECT_-prefixed env vars in CI,
// ~/.gradle/gradle.properties locally; neither lives in this file.
mavenPublishing {
    publishToMavenCentral()
    signAllPublications()

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
                url.set("https://opensource.org/license/mit")
                distribution.set("repo")
            }
        }
        developers {
            developer {
                id.set("buvinghausen")
                name.set("Brian Buvinghausen")
                url.set("https://github.com/buvinghausen/")
            }
        }
        scm {
            url.set("https://github.com/SkunkWerkx/HyperCast")
            connection.set("scm:git:git://github.com/SkunkWerkx/HyperCast.git")
            developerConnection.set("scm:git:ssh://git@github.com/SkunkWerkx/HyperCast.git")
        }
    }
}

publishing {
    repositories {
        // This repo's GitHub Packages Maven registry (private by default, repo-scoped —
        // github.com/SkunkWerkx/HyperCast/packages). Credentials come from CI's own
        // GITHUB_ACTOR/GITHUB_TOKEN; empty locally, which only matters if you actually run
        // `./gradlew publish` (publishToMavenLocal doesn't touch this repository). Independent
        // of mavenPublishing {} above — this stays as a second, separate target on the same
        // "maven" publication, not a competing one.
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
