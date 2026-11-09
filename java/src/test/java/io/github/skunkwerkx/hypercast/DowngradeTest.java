package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.skunkwerkx.hypercast.interop.NativePlatform;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;

/**
 * A policy is bits in a flags word, and a 1.0 core silently ignores them, so a request against
 * one would be parsed by 1.0's rule and answer wrongly instead of loudly. The guard is
 * structural: this binding resolves the four detailed exports (FFM) or the natives that wrap them
 * (JNI) as it loads, and a library without them fails the load. This runs a child JVM whose
 * classpath puts a real 1.0 library in front of the jar's own, and shows that {@link
 * Cast#isAvailable()} says no, that a request is a load failure and never a verdict, and,
 * as the control, that the same child with the jar's own libraries answers.
 *
 * <p>Tagged {@code downgrade}: the 1.0 libraries are not in the jar. {@code ./gradlew
 * testDowngrade testDowngradeFloor -Phypercast.v1Core=<a build of tag v1.0.0's libhypercast>
 * -Phypercast.preDetailedCore=<a core with the 1.0-era exports and no detailed ones>
 * -Phypercast.v1Jni=<a libhypercast_jni built from a core without the detailed exports>}. A
 * missing property fails the test: a downgrade test that quietly passes proves nothing.
 */
@Tag("downgrade")
class DowngradeTest {
    /** The v1.0.0 core, which predates even the checked exports: refused at the first export it lacks. */
    @Test
    void aRequestAgainstTheV10CoreIsRejectedNeverParsedByItsRule() throws Exception {
        String older = runAgainst("hypercast.v1Core");
        if (Runtime.version().feature() >= 25) {
            assertTrue(older.contains("no export cast_"), older);
        } else {
            assertTrue(older.contains(JNI_MISMATCH), older);
        }
    }

    /**
     * A core with every 1.0 export and none of the four detailed ones (the item-2 core): the guard's
     * own reason, a missing {@code cast_detailed*} symbol on FFM, the JNI native-count mismatch on JNI.
     */
    @Test
    void aCoreWithoutTheDetailedExportsIsRejectedForThatReason() throws Exception {
        String older = runAgainst("hypercast.preDetailedCore");
        if (Runtime.version().feature() >= 25) {
            assertTrue(older.contains("no export cast_detailed"), older);
        } else {
            assertTrue(older.contains(JNI_MISMATCH), older);
        }
    }

    private static final String JNI_MISMATCH = "implements 33 natives; this binding declares 37";

    // The child with `core` (the named property's library) and the 1.0-era JNI library in front of
    // the jar's, then the control: the same child, the jar's own libraries.
    private static String runAgainst(String coreProperty) throws Exception {
        String core = System.getProperty(coreProperty);
        String jni = System.getProperty("hypercast.v1Jni");
        assertNotNull(core, "-P" + coreProperty + " names the older core library");
        assertNotNull(jni, "-Phypercast.v1Jni names the 1.0-era JNI library");
        Path dir = Files.createTempDirectory("hypercast-v10");
        try {
            NativePlatform.Target coreTarget = NativePlatform.current("hypercast");
            NativePlatform.Target jniTarget = NativePlatform.current("hypercast_jni");
            Path native_ = Files.createDirectories(dir.resolve("native").resolve(coreTarget.rid()));
            Files.copy(
                    Path.of(core), native_.resolve(coreTarget.libraryFileName()), StandardCopyOption.REPLACE_EXISTING);
            Files.copy(Path.of(jni), native_.resolve(jniTarget.libraryFileName()), StandardCopyOption.REPLACE_EXISTING);

            String older = run(dir);
            assertTrue(older.contains("available=false"), older);
            assertTrue(older.contains("plain=rejected "), older);
            assertTrue(older.contains("detailed=rejected "), older);
            // Whatever a request returned is wrong, a Fault as much as a Success: against an older
            // core nothing may answer. The reason is named by the root cause, asserted by the caller.
            assertFalse(older.contains("=returned "), older);

            // The control: the same child, the jar's own libraries, answers the request.
            String current = run(null);
            assertTrue(current.contains("available=true"), current);
            assertTrue(current.contains("plain=returned Fault[reason=MALFORMED, offset=2, length=3]"), current);
            assertTrue(
                    current.contains("detailed=returned DetailedFault[reason=MALFORMED, detail=group_size"), current);
            return older;
        } finally {
            try (var files = Files.walk(dir)) {
                files.sorted(java.util.Comparator.reverseOrder())
                        .forEach(p -> p.toFile().delete());
            }
        }
    }

    private static String run(Path first) throws IOException, InterruptedException {
        String launcher = ProcessHandle.current().info().command().orElse("java");
        String classpath =
                (first == null ? "" : first + java.io.File.pathSeparator) + System.getProperty("java.class.path");
        Path log = Files.createTempFile("hypercast-downgrade", ".log");
        try {
            Process process = new ProcessBuilder(
                            launcher, "--enable-native-access=ALL-UNNAMED", "-cp", classpath, Child.class.getName())
                    .redirectErrorStream(true)
                    .redirectOutput(log.toFile())
                    .start();
            if (!process.waitFor(120, TimeUnit.SECONDS)) {
                process.destroyForcibly();
                throw new AssertionError("the child JVM did not finish in 120 s");
            }
            String output = Files.readString(log, StandardCharsets.UTF_8);
            assertEquals(0, process.exitValue(), output);
            return output;
        } finally {
            Files.deleteIfExists(log);
        }
    }

    /** The numeric request, as a caller makes it, and what became of it. */
    static final class Child {
        public static void main(String[] args) {
            System.out.println("available=" + Cast.isAvailable());
            System.out.println("plain=" + attempt(() -> Cast.i32("12,34", NumFormat.INVARIANT)));
            System.out.println("detailed=" + attempt(() -> DetailedCast.i32("12,34", NumFormat.INVARIANT)));
        }

        private static String attempt(java.util.function.Supplier<Object> request) {
            try {
                return "returned " + request.get();
            } catch (Throwable t) {
                // The cause chain, so a test can name the reason and not just that it failed.
                StringBuilder chain = new StringBuilder("rejected");
                for (Throwable cause = t; cause != null; cause = cause.getCause()) {
                    chain.append(" <- ")
                            .append(cause.getClass().getName())
                            .append(": ")
                            .append(cause.getMessage());
                }
                return chain.toString();
            }
        }
    }
}
