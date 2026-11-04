package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.DataInputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.jar.Attributes;
import java.util.jar.JarEntry;
import java.util.jar.JarFile;
import java.util.jar.JarOutputStream;
import java.util.jar.Manifest;
import java.util.zip.ZipEntry;
import org.junit.jupiter.api.Test;

/**
 * Which path loaded, and the load-time properties of both: each runs on the floor JDK (JNI) and
 * on 25 (FFM), the expected backend and feature coming from the build.
 */
class BackendTest {
    private static String expectBackend() {
        return System.getProperty("hypercast.expectBackend");
    }

    private static Path jar() throws Exception {
        return Path.of(
                Cast.class.getProtectionDomain().getCodeSource().getLocation().toURI());
    }

    private static Path testClasses() throws Exception {
        return Path.of(BackendTest.class
                .getProtectionDomain()
                .getCodeSource()
                .getLocation()
                .toURI());
    }

    private static String java() {
        return ProcessHandle.current().info().command().orElse("java");
    }

    /** Runs a child JVM to completion, output merged; fails the test on a hang. */
    private static String run(int expectedExit, List<String> command) throws Exception {
        var log = Files.createTempFile("hypercast-backend", ".log");
        try {
            var process = new ProcessBuilder(command)
                    .redirectErrorStream(true)
                    .redirectOutput(log.toFile())
                    .start();
            if (!process.waitFor(120, TimeUnit.SECONDS)) {
                process.destroyForcibly();
                throw new AssertionError("child JVM did not finish in 120 s: " + command);
            }
            var output = Files.readString(log, StandardCharsets.UTF_8);
            assertEquals(expectedExit, process.exitValue(), output);
            return output;
        } finally {
            Files.deleteIfExists(log);
        }
    }

    @Test
    void theExpectedPathLoaded() throws Exception {
        assertTrue(Cast.isAvailable());
        assertEquals(expectBackend(), NativeCore.backend());
        assertEquals(
                Integer.parseInt(System.getProperty("hypercast.expectFeature")),
                Runtime.version().feature());
        long natives = Arrays.stream(NativeCore.class.getDeclaredMethods())
                .filter(m -> Modifier.isNative(m.getModifiers()))
                .count();
        // One native per C export (36, the four detailed ones included), and jniNatives, the count
        // the library checks against.
        assertEquals("jni".equals(expectBackend()) ? 37 : 0, natives);
        int major;
        try (InputStream in = NativeCore.class.getResourceAsStream("NativeCore.class")) {
            var data = new DataInputStream(in);
            data.readInt(); // 0xCAFEBABE
            data.readUnsignedShort(); // minor
            major = data.readUnsignedShort();
        }
        assertEquals("jni".equals(expectBackend()) ? 65 : 69, major);
    }

    // The attribute stripped, as a shading plugin does: the JDK then reads only the base, so
    // 25 runs the JNI path too. Correct, slower, and its warning names System::load.
    @Test
    void aStrippedJarRunsJniCorrectlyOn25() throws Exception {
        var dir = Files.createTempDirectory("hypercast-stripped");
        var stripped = dir.resolve("stripped.jar");
        try {
            try (var in = new JarFile(jar().toFile());
                    OutputStream file = Files.newOutputStream(stripped);
                    var out = new JarOutputStream(file)) {
                var manifest = new Manifest(in.getManifest());
                manifest.getMainAttributes().remove(new Attributes.Name("Multi-Release"));
                out.putNextEntry(new ZipEntry(JarFile.MANIFEST_NAME));
                manifest.write(out);
                out.closeEntry();
                for (var entries = in.entries(); entries.hasMoreElements(); ) {
                    JarEntry entry = entries.nextElement();
                    if (entry.getName().equals(JarFile.MANIFEST_NAME)) {
                        continue;
                    }
                    out.putNextEntry(new ZipEntry(entry.getName()));
                    if (!entry.isDirectory()) {
                        try (InputStream body = in.getInputStream(entry)) {
                            body.transferTo(out);
                        }
                    }
                    out.closeEntry();
                }
            }
            // No --enable-native-access: on 24+ the warning is what the README promises.
            var output = run(
                    0,
                    List.of(
                            java(),
                            "-cp",
                            stripped + java.io.File.pathSeparator + testClasses(),
                            StrippedMain.class.getName()));
            assertTrue(output.contains("backend: jni"), output);
            assertTrue(output.contains("i32: Success[value=42]"), output);
            assertTrue(output.contains("dateTime: Success[value=2026-01-07T15:04]"), output);
            if (Runtime.version().feature() >= 24) {
                assertTrue(output.contains("java.lang.System::load"), output);
            }
        } finally {
            Files.deleteIfExists(stripped);
            Files.deleteIfExists(dir);
        }
    }

    static final class StrippedMain {
        public static void main(String[] args) {
            System.out.println("backend: " + NativeCore.backend());
            System.out.println("i32: " + Cast.i32("42", NumFormat.INVARIANT));
            System.out.println("dateTime: "
                    + Cast.dateTime(
                            "7/1/2026 3:04 p. m.", new DateTimeFormat(DateOrder.DAY_MONTH_YEAR, "a. m.", "p. m.")));
        }
    }

    // JNI refuses one library in two class loaders, so each loader extracts its own copy.
    @Test
    void twoClassLoadersEachLoadTheCore() throws Exception {
        URL url = jar().toUri().toURL();
        try (var a = new URLClassLoader(new URL[] {url}, ClassLoader.getPlatformClassLoader());
                var b = new URLClassLoader(new URL[] {url}, ClassLoader.getPlatformClassLoader())) {
            var versions = new ArrayList<String>();
            for (ClassLoader loader : List.of(a, b)) {
                Class<?> cast = loader.loadClass(Cast.class.getName());
                assertFalse(cast == Cast.class);
                Method available = cast.getMethod("isAvailable");
                Method version = cast.getMethod("nativeVersion");
                assertEquals(true, available.invoke(null));
                versions.add((String) version.invoke(null));
            }
            assertEquals(versions.get(0), versions.get(1));
        }
    }

    @Test
    void theModulePathLoadsTheExpectedBackend() throws Exception {
        var output = run(
                0,
                List.of(
                        java(),
                        "--module-path",
                        jar().toString(),
                        "--add-modules",
                        "io.github.skunkwerkx.hypercast",
                        "--enable-native-access=io.github.skunkwerkx.hypercast",
                        "-cp",
                        testClasses().toString(),
                        "io.github.skunkwerkx.hypercast.probe.ModuleMain"));
        assertTrue(output.contains("backend: " + expectBackend()), output);
        assertTrue(output.contains("module: io.github.skunkwerkx.hypercast"), output);
        assertFalse(output.contains("WARNING"), output);
    }

    @Test
    void withoutACoreTheProbeAnswersFalseAndTheDoorsThrowTheLoadFailure() throws Exception {
        // A second JVM with the jar's classes but no native library: the README's contract is
        // that isAvailable() answers false there and never throws, and that a door throws
        // ExceptionInInitializerError, then NoClassDefFoundError, rather than anything wrapped
        // around them. On either path.
        var output = run(
                0,
                List.of(
                        java(),
                        "--enable-native-access=ALL-UNNAMED",
                        "-cp",
                        System.getProperty("hypercast.noNativeJar") + java.io.File.pathSeparator + testClasses(),
                        NoCore.class.getName()));
        assertTrue(
                output.contains("doors: java.lang.ExceptionInInitializerError java.lang.NoClassDefFoundError"), output);
        assertTrue(output.contains("probe: false"), output);
        assertTrue(output.contains("backend: " + expectBackend()), output);
    }

    static final class NoCore {
        private static String door() {
            try {
                Cast.i32("42", NumFormat.INVARIANT);
                return "returned";
            } catch (Throwable e) {
                return e.getClass().getName();
            }
        }

        public static void main(String[] args) {
            System.out.println("doors: " + door() + " " + door());
            System.out.println("probe: " + Cast.isAvailable());
            System.out.println("backend: " + NativeCore.backend());
        }
    }

    @Test
    void aRacingFirstTouchLoadsOnce() throws Exception {
        var tmp = Files.createTempDirectory("hypercast-race");
        try {
            var output = run(
                    0,
                    List.of(
                            java(),
                            "--enable-native-access=ALL-UNNAMED",
                            "-Djava.io.tmpdir=" + tmp,
                            "-cp",
                            System.getProperty("java.class.path"),
                            RaceMain.class.getName()));
            assertTrue(output.contains("all 32 doors succeeded"), output);
            assertTrue(output.contains("extracted: 1"), output);
        } finally {
            try (var left = Files.list(tmp)) {
                for (Path p : (Iterable<Path>) left::iterator) {
                    Files.deleteIfExists(p);
                }
            }
            Files.deleteIfExists(tmp);
        }
    }

    /** 32 threads, one latch, a different door first on each; then counts the extracted libraries. */
    static final class RaceMain {
        public static void main(String[] args) throws Exception {
            var gate = new CountDownLatch(1);
            var failures = new java.util.concurrent.ConcurrentLinkedQueue<Throwable>();
            byte[] utf8 = "42".getBytes(StandardCharsets.UTF_8);
            List<Runnable> doors = List.of(
                    () -> Cast.bool("true"),
                    () -> Cast.character("x"),
                    () -> Cast.i32("42", NumFormat.INVARIANT),
                    () -> Cast.u64(utf8, NumFormat.INVARIANT),
                    () -> Cast.f64(java.lang.foreign.MemorySegment.ofArray(utf8), NumFormat.INVARIANT),
                    () -> Cast.decimal("4.2", java.util.Locale.GERMANY),
                    () -> Cast.uuid("f1e2d3c4-b5a6-9788-8968-5a4b3c2d1e0f"),
                    () -> Cast.timestamp("2026-01-07T15:04:05Z"),
                    () -> Cast.unix("1767798245", UnixPrecision.values()[0]),
                    () -> Cast.date("2026-01-07"),
                    () -> Cast.dateTime("1/7/2026 3:04 PM", DateOrder.MONTH_DAY_YEAR),
                    () -> Cast.time("15:04"),
                    () -> Cast.duration("PT1H"),
                    () -> Cast.excelTime(0.5),
                    Cast::nativeVersion,
                    Cast::isAvailable);
            var threads = new ArrayList<Thread>();
            for (int i = 0; i < 32; i++) {
                Runnable door = doors.get(i % doors.size());
                Thread t = new Thread(() -> {
                    try {
                        gate.await();
                        door.run();
                    } catch (Throwable e) {
                        failures.add(e);
                    }
                });
                threads.add(t);
                t.start();
            }
            gate.countDown();
            for (Thread t : threads) {
                t.join();
            }
            if (!failures.isEmpty()) {
                failures.peek().printStackTrace();
                System.exit(1);
            }
            long extracted;
            try (var files = Files.list(Path.of(System.getProperty("java.io.tmpdir")))) {
                extracted = files.filter(p -> p.getFileName().toString().startsWith("hypercast"))
                        .count();
            }
            System.out.println("all 32 doors succeeded");
            System.out.println("extracted: " + extracted);
        }
    }
}
