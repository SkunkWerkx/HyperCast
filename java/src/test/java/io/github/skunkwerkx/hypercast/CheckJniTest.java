package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.lang.foreign.MemorySegment;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.time.LocalDateTime;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import org.junit.jupiter.api.Test;

/**
 * {@code -Xcheck:jni} prints its findings natively, past every Gradle listener, so a suite that
 * merely runs under the flag cannot fail on them. This runs every shape of the JNI library's
 * calls, on both sides of the 64 KiB pinning bound, in a child JVM of its own and reads the
 * output. On 25 the same program runs the FFM path, where the flag has nothing to check.
 */
class CheckJniTest {
    @Test
    void noCheckJniWarningUnderAllocationPressure() throws Exception {
        var java = ProcessHandle.current().info().command().orElse("java");
        var log = Files.createTempFile("hypercast-checkjni", ".log");
        try {
            var process = new ProcessBuilder(
                            java,
                            "--enable-native-access=ALL-UNNAMED",
                            "-Xcheck:jni",
                            "-cp",
                            System.getProperty("java.class.path"),
                            PressureMain.class.getName())
                    .redirectErrorStream(true)
                    .redirectOutput(log.toFile())
                    .start();
            if (!process.waitFor(300, TimeUnit.SECONDS)) {
                process.destroyForcibly();
                throw new AssertionError("the -Xcheck:jni child did not finish in 300 s");
            }
            var output = Files.readString(log, StandardCharsets.UTF_8);
            assertEquals(0, process.exitValue(), output);
            assertTrue(output.contains("checked"), output);
            // HotSpot words its findings three ways: "WARNING in native method" (a call made with
            // an exception pending, say), "Warning: Calling other JNI functions in the scope of
            // Get/ReleasePrimitiveArrayCritical", and "FATAL ERROR in native method" (which also
            // fails the exit code).
            assertFalse(output.contains("WARNING"), output);
            assertFalse(output.contains("Warning:"), output);
            assertFalse(output.contains("FATAL ERROR"), output);
        } finally {
            Files.deleteIfExists(log);
        }
    }

    /** Every call shape, pinned and copied, while another thread churns the heap. */
    static final class PressureMain {
        private static void expect(Object want, Object got) {
            if (!want.equals(got)) {
                throw new AssertionError(want + " != " + got);
            }
        }

        public static void main(String[] args) throws Exception {
            var stop = new AtomicBoolean();
            var churn = new Thread(() -> {
                byte[] keep = null;
                while (!stop.get()) {
                    keep = new byte[64 * 1024];
                }
                if (keep == null) {
                    throw new IllegalStateException();
                }
            });
            churn.setDaemon(true);
            churn.start();
            // Designators beyond English, so the civil door carries its layout across.
            DateTimeFormat spanish = new DateTimeFormat(DateOrder.DAY_MONTH_YEAR, "a. m.", "p. m.");
            // A declaration with an enum list, so the checked doors carry every layout and the
            // list's direct buffer across.
            Constraints<Integer> codes = Constraints.i32().enumOf("42", "7").build();
            Constraints<String> sex =
                    Constraints.text().trim(true).enumOf("Female", "Male").build();
            Constraints<BigDecimal> cash =
                    Constraints.decimal().multipleOf("0.05").build();
            try {
                for (int pad : new int[] {0, 64 * 1024 - 4, 64 * 1024, 1 << 20}) {
                    String blank = " ".repeat(pad);
                    byte[] number = (blank + "42").getBytes(StandardCharsets.UTF_8);
                    // The critical side of the bound repeatedly, so a collection lands inside some.
                    for (int i = 0; i < (pad > 64 * 1024 ? 4 : 200); i++) {
                        expect(new Success<>(42), Cast.i32(number, NumFormat.INVARIANT));
                        expect(new Success<>(new BigDecimal("42")), Cast.decimal(number, NumFormat.INVARIANT));
                        expect(new Success<>(true), Cast.bool(blank + "true"));
                        expect(
                                new Success<>(LocalDateTime.of(2026, 1, 7, 15, 4)),
                                Cast.dateTime(blank + "7/1/2026 3:04 p. m.", spanish));
                        expect(new Success<>(42L), Cast.i64(MemorySegment.ofArray(number), NumFormat.INVARIANT));
                        expect(
                                new Checked<>(42, false, java.util.OptionalInt.empty()),
                                Cast.i32(number, NumFormat.INVARIANT, codes));
                        expect(
                                new Checked<>("Male", false, java.util.OptionalInt.of(1)),
                                Cast.text(blank + "MALE", sex));
                        expect(new Success<>("42"), Cast.text("42"));
                    }
                    expect(
                            new Violation<BigDecimal>(Keyword.MULTIPLE_OF, new BigDecimal("42.01"), pad, 5),
                            Cast.decimal(
                                    (blank + "42.01").getBytes(StandardCharsets.UTF_8), NumFormat.INVARIANT, cash));
                    // A fault past the bound: its span is in the caller's bytes, whichever side.
                    byte[] bad = (blank + "x").getBytes(StandardCharsets.UTF_8);
                    expect(new Fault<Integer>(CastFailure.MALFORMED, pad, 1), Cast.i32(bad, NumFormat.INVARIANT));
                }
                expect(new Success<>(new BigDecimal("0.5")), Cast.decimalFromDouble(0.5));
                expect(new Checked<>(7, false, java.util.OptionalInt.empty()), Cast.i32FromDouble(7, codes));
                expect(Cast.nativeVersion(), Cast.nativeVersion());
            } finally {
                stop.set(true);
            }
            System.out.println("checked");
        }
    }
}
