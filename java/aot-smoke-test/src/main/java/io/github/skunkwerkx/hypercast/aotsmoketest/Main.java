package io.github.skunkwerkx.hypercast.aotsmoketest;

import io.github.skunkwerkx.hypercast.Cast;
import io.github.skunkwerkx.hypercast.CastFailure;
import io.github.skunkwerkx.hypercast.Fault;
import io.github.skunkwerkx.hypercast.NumFormat;
import io.github.skunkwerkx.hypercast.Success;
import io.github.skunkwerkx.hypercast.UnixPrecision;
import io.github.skunkwerkx.hypercast.Verdict;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalTime;
import java.util.UUID;

/**
 * Exercises a door from every family — including the sealed union's exhaustive-switch
 * consumption — as a GraalVM Native Image binary. Exit code 0 only if every cast lands as
 * expected.
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
     * Exercises a door from every family plus the union switch and exits non-zero on the
     * first wrong answer.
     *
     * @param args ignored
     */
    public static void main(String[] args) {
        // Which interop path this binary took — "native" (FFM) or "wasm" (GraalWasm) — so a
        // -Dhypercast.backend=wasm run is visibly proving the path it claims to.
        System.out.println("backend: " + Cast.backend());
        check("bool", Cast.bool("enabled"), true);
        check("i32", Cast.i32("(1,234)", NumFormat.INVARIANT), -1234);
        check("f64", Cast.f64("25.5%", NumFormat.INVARIANT), 0.255);
        check("uuid", Cast.uuid("urn:uuid:01020304-0506-0708-090a-0b0c0d0e0f10"),
                UUID.fromString("01020304-0506-0708-090a-0b0c0d0e0f10"));
        check("timestamp", Cast.timestamp("2026-01-02T15:04:05.123456789+05:00"),
                Instant.parse("2026-01-02T10:04:05.123456789Z"));
        check("unix", Cast.unix("1700000000", UnixPrecision.SECONDS), Instant.ofEpochSecond(1_700_000_000L));
        check("date", Cast.date("2026-01-02"), LocalDate.of(2026, 1, 2));
        check("time", Cast.time("15:04:05"), LocalTime.of(15, 4, 5));
        check("duration", Cast.duration("P1DT6H"), Duration.ofHours(30));

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
