package io.github.skunkwerkx.hypercast.aotsmoketest;

import io.github.skunkwerkx.hypercast.Cast;
import io.github.skunkwerkx.hypercast.CastFailure;
import io.github.skunkwerkx.hypercast.DateOrder;
import io.github.skunkwerkx.hypercast.ExcelEpoch;
import io.github.skunkwerkx.hypercast.Fault;
import io.github.skunkwerkx.hypercast.NumFormat;
import io.github.skunkwerkx.hypercast.Success;
import io.github.skunkwerkx.hypercast.UnixPrecision;
import io.github.skunkwerkx.hypercast.Verdict;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.Locale;
import java.util.UUID;

/**
 * Exercises every door — and the sealed union's exhaustive-switch consumption — as a
 * GraalVM Native Image binary ({@code ./gradlew :aot-smoke-test:nativeRun};
 * {@code :aot-smoke-test:run} is the same program on an ordinary JVM). Exit code 0 only if
 * every cast lands as expected.
 */
public final class Main {
    private Main() {}

    private static int failures;

    private static <T> void check(String name, Verdict<T> verdict, T expected) {
        switch (verdict) {
            case Success<T> success when success.value().equals(expected) ->
                    System.out.println("ok   " + name + " = " + success.value());
            case Success<T> success -> {
                System.out.println("FAIL " + name + ": " + success.value() + " (expected " + expected + ")");
                failures++;
            }
            case Fault<T> fault -> {
                System.out.println("FAIL " + name + ": " + fault + " (expected " + expected + ")");
                failures++;
            }
        }
    }

    /**
     * Exercises the probe, every door, each input form and the union switch, prints every
     * answer, and exits non-zero if any of them was wrong.
     *
     * @param args ignored
     */
    public static void main(String[] args) {
        // Which interop path this binary took — "native" (FFM) or "wasm" (GraalWasm) — so a
        // -Dhypercast.backend=wasm run is visibly proving the path it claims to.
        // The non-throwing gate first: under AOT a missing resource registration would make
        // the core unloadable, and this is the probe a consumer would notice that through.
        System.out.println("available: " + Cast.isAvailable());
        if (!Cast.isAvailable()) {
            failures++;
        }
        System.out.println("backend: " + Cast.backend());
        // The version probe is the one export linked without the critical option, so it
        // needs its own downcall signature in reachability-metadata.json — this is what
        // proves the registration reached the binary.
        String version = Cast.nativeVersion();
        System.out.println("version: " + version);
        if (!version.matches("\\d+\\.\\d+\\.\\d+")) {
            System.out.println("FAIL version: " + version + " (expected major.minor.patch)");
            failures++;
        }
        // All twenty-one doors, each through its String form.
        check("bool", Cast.bool("enabled"), true);
        check("i8", Cast.i8("-128", NumFormat.INVARIANT), (byte) -128);
        check("i16", Cast.i16("-32,768", NumFormat.INVARIANT), (short) -32768);
        check("i32", Cast.i32("(1,234)", NumFormat.INVARIANT), -1234);
        check("i64", Cast.i64("9223372036854775807", NumFormat.INVARIANT), Long.MAX_VALUE);
        check("u8", Cast.u8("255", NumFormat.INVARIANT), 255);
        check("u16", Cast.u16("65535", NumFormat.INVARIANT), 65535);
        check("u32", Cast.u32("4294967295", NumFormat.INVARIANT), 4_294_967_295L);
        // u64::MAX arrives as the two's-complement bit pattern.
        check("u64", Cast.u64("18446744073709551615", NumFormat.INVARIANT), -1L);
        check("f32", Cast.f32("1.5", NumFormat.INVARIANT), 1.5f);
        check("f64", Cast.f64("25.5%", NumFormat.INVARIANT), 0.255);
        check("decimal", Cast.decimal("$1,234.50", NumFormat.from(Locale.US)), new BigDecimal("1234.5"));
        check("uuid", Cast.uuid("urn:uuid:01020304-0506-0708-090a-0b0c0d0e0f10"),
                UUID.fromString("01020304-0506-0708-090a-0b0c0d0e0f10"));
        check("timestamp", Cast.timestamp("2026-01-02T15:04:05.123456789+05:00"),
                Instant.parse("2026-01-02T10:04:05.123456789Z"));
        check("unix", Cast.unix("1700000000", UnixPrecision.SECONDS), Instant.ofEpochSecond(1_700_000_000L));
        check("excelSerial", Cast.excelSerial("45292.75", ExcelEpoch.Y1900), Instant.parse("2024-01-01T18:00:00Z"));
        check("date", Cast.date("2026-01-02"), LocalDate.of(2026, 1, 2));
        check("date (ordered)", Cast.date("1/7/2026", DateOrder.MONTH_DAY_YEAR), LocalDate.of(2026, 1, 7));
        check("dateTime", Cast.dateTime("1/7/2026 3:04 PM", DateOrder.MONTH_DAY_YEAR),
                LocalDateTime.of(2026, 1, 7, 15, 4));
        check("time", Cast.time("15:04:05"), LocalTime.of(15, 4, 5));
        check("duration", Cast.duration("P1DT6H"), Duration.ofHours(30));

        // The other two input forms, once per ABI shape rather than once per door — the
        // overloads differ only in how the bytes arrive. A byte[] and a heap slice both
        // cross pinned, which is the heap-access half of the critical downcall
        // registration; a native segment crosses as the address it already is.
        byte[] line = "(1,234)|1700000000|true".getBytes(StandardCharsets.UTF_8);
        check("i32 (byte[])", Cast.i32("(1,234)".getBytes(StandardCharsets.UTF_8), NumFormat.INVARIANT), -1234);
        MemorySegment whole = MemorySegment.ofArray(line);
        check("i32 (heap slice)", Cast.i32(whole.asSlice(0, 7), NumFormat.INVARIANT), -1234);
        check("unix (heap slice)", Cast.unix(whole.asSlice(8, 10), UnixPrecision.SECONDS),
                Instant.ofEpochSecond(1_700_000_000L));
        check("bool (heap slice)", Cast.bool(whole.asSlice(19, 4)), true);
        try (Arena arena = Arena.ofConfined()) {
            // allocateFrom appends a NUL terminator; the door gets exactly the text's bytes.
            MemorySegment offHeap = arena.allocateFrom("42.5");
            check("f64 (native segment)", Cast.f64(offHeap.asSlice(0, 4), NumFormat.INVARIANT), 42.5);
        }

        // The exhaustive two-arm switch must survive AOT too.
        String disposition = switch (Cast.i32("not-a-number", NumFormat.INVARIANT)) {
            case Success<Integer> s -> "unexpected ok " + s.value();
            case Fault<Integer> f -> "fault " + f.reason() + " @ " + f.offset() + "+" + f.length();
        };
        System.out.println("union switch: " + disposition);
        if (!disposition.startsWith("fault " + CastFailure.MALFORMED)) {
            failures++;
        }

        System.out.println(failures == 0 ? "AOT smoke test passed." : "AOT smoke test FAILED (" + failures + ").");
        System.exit(failures == 0 ? 0 : 1);
    }
}
