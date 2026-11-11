package io.github.skunkwerkx.hypercast.aotsmoketest;

import io.github.skunkwerkx.hypercast.Cast;
import io.github.skunkwerkx.hypercast.CastFailure;
import io.github.skunkwerkx.hypercast.Checked;
import io.github.skunkwerkx.hypercast.CheckedVerdict;
import io.github.skunkwerkx.hypercast.Constraints;
import io.github.skunkwerkx.hypercast.DateOptions;
import io.github.skunkwerkx.hypercast.DateOrder;
import io.github.skunkwerkx.hypercast.DateTimeFormat;
import io.github.skunkwerkx.hypercast.DetailedCast;
import io.github.skunkwerkx.hypercast.DetailedChecked;
import io.github.skunkwerkx.hypercast.DetailedCheckedVerdict;
import io.github.skunkwerkx.hypercast.DetailedFault;
import io.github.skunkwerkx.hypercast.DetailedSuccess;
import io.github.skunkwerkx.hypercast.DetailedVerdict;
import io.github.skunkwerkx.hypercast.DetailedViolation;
import io.github.skunkwerkx.hypercast.ExcelEpoch;
import io.github.skunkwerkx.hypercast.Fault;
import io.github.skunkwerkx.hypercast.Grouping;
import io.github.skunkwerkx.hypercast.MonthNames;
import io.github.skunkwerkx.hypercast.NumFormat;
import io.github.skunkwerkx.hypercast.Success;
import io.github.skunkwerkx.hypercast.UnixPrecision;
import io.github.skunkwerkx.hypercast.Verdict;
import io.github.skunkwerkx.hypercast.Violation;
import java.io.DataInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.reflect.Modifier;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.Arrays;
import java.util.Locale;
import java.util.UUID;

/**
 * Exercises every door — and the sealed union's exhaustive-switch consumption — as a
 * GraalVM Native Image binary ({@code ./gradlew :aot-smoke-test:nativeRun};
 * {@code :aot-smoke-test:run} is the same program on an ordinary JVM, where JDK 25 takes the
 * jar's FFM path, and {@code :aot-smoke-test:runFloor} on the floor JDK, where its JNI base
 * loads). Exit code 0 only if every cast lands as expected.
 */
public final class Main {
    private Main() {}

    private static int failures;

    private static <T> void check(String name, Verdict<T> verdict, T expected) {
        switch (verdict) {
            case Success<T> success
            when success.value().equals(expected) -> System.out.println("ok   " + name + " = " + success.value());
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

    private static <T> void checked(String name, CheckedVerdict<T> verdict, String expected) {
        String actual = switch (verdict) {
            case Checked<T> c -> "checked " + c.value();
            case Violation<T> v -> "violation " + v.keyword() + " " + v.value() + " @ " + v.offset() + "+" + v.length();
            case Fault<T> f -> "fault " + f.reason() + " @ " + f.offset() + "+" + f.length();
        };
        if (actual.equals(expected)) {
            System.out.println("ok   " + name + " = " + actual);
        } else {
            System.out.println("FAIL " + name + ": " + actual + " (expected " + expected + ")");
            failures++;
        }
    }

    private static <T> void detailed(String name, DetailedVerdict<T> verdict, String expected) {
        String actual = switch (verdict) {
            case DetailedSuccess<T> s -> "ok " + s.value();
            case DetailedFault<T> f -> "fault " + f.reason() + " " + f.detail() + " @ " + f.offset() + "+" + f.length();
        };
        report(name, actual, expected);
    }

    private static <T> void detailedChecked(String name, DetailedCheckedVerdict<T> verdict, String expected) {
        String actual = switch (verdict) {
            case DetailedChecked<T> c -> "checked " + c.value();
            case DetailedViolation<T> v ->
                "violation " + v.keyword() + " " + v.value() + " @ " + v.offset() + "+" + v.length();
            case DetailedFault<T> f -> "fault " + f.detail() + " @ " + f.offset() + "+" + f.length();
        };
        report(name, actual, expected);
    }

    private static void report(String name, String actual, String expected) {
        if (actual.equals(expected)) {
            System.out.println("ok   " + name + " = " + actual);
        } else {
            System.out.println("FAIL " + name + ": " + actual + " (expected " + expected + ")");
            failures++;
        }
    }

    /**
     * Exercises the probe, every door, each input form and the union switch, prints every
     * answer, and exits non-zero if any of them was wrong.
     *
     * @param args ignored
     */
    public static void main(String[] args) {
        // The non-throwing gate first: under AOT a missing resource registration would make
        // the core unloadable, and this is the probe a consumer would notice that through.
        System.out.println("available: " + Cast.isAvailable());
        if (!Cast.isAvailable()) {
            failures++;
        }
        requireExpectedBackend();
        if (System.getProperty("org.graalvm.nativeimage.imagecode") != null) {
            requireNoJniLibraryEmbedded();
        }
        // The version probe is the one export linked without the critical option, so it
        // needs its own downcall signature in reachability-metadata.json — this is what
        // proves the registration reached the binary.
        String version = Cast.nativeVersion();
        System.out.println("version: " + version);
        if (!version.matches("\\d+\\.\\d+\\.\\d+")) {
            System.out.println("FAIL version: " + version + " (expected major.minor.patch)");
            failures++;
        }
        // All twenty-six doors: each text door through its String form, the four typed doors
        // through a double.
        check("bool", Cast.bool("enabled"), true);
        check("character", Cast.character("U+00E9"), '\u00e9');
        check("i8", Cast.i8("-128", NumFormat.INVARIANT), (byte) -128);
        check("i16", Cast.i16("-32,768", NumFormat.INVARIANT), (short) -32768);
        // A Locale overload: the format derived from the JDK's locale data rather than declared.
        // en-US, the locale the image is built in and so the one it carries without
        // -H:IncludeLocales (see the README's AOT section).
        check("f64 (en-US locale)", Cast.f64("1,234.5", Locale.US), 1234.5);
        check("i32", Cast.i32("(1,234)", NumFormat.INVARIANT), -1234);
        // A format with the 0.9.0 fields declared, crossing in the 92-byte layout.
        check(
                "i32 (declared minus, native digits)",
                Cast.i32(
                        "\u2212\u0661,\u0662\u0663\u0664",
                        NumFormat.INVARIANT.withSigns("\u2212", "").withDigitZero(0x0660)),
                -1234);
        check("i64", Cast.i64("9223372036854775807", NumFormat.INVARIANT), Long.MAX_VALUE);
        check("u8", Cast.u8("255", NumFormat.INVARIANT), 255);
        check("u16", Cast.u16("65535", NumFormat.INVARIANT), 65535);
        check("u32", Cast.u32("4294967295", NumFormat.INVARIANT), 4_294_967_295L);
        // u64::MAX arrives as the two's-complement bit pattern.
        check("u64", Cast.u64("18446744073709551615", NumFormat.INVARIANT), -1L);
        check("f32", Cast.f32("1.5", NumFormat.INVARIANT), 1.5f);
        check("f64", Cast.f64("25.5%", NumFormat.INVARIANT), 0.255);
        check("decimal", Cast.decimal("$1,234.50", NumFormat.from(Locale.US)), new BigDecimal("1234.5"));
        check(
                "uuid",
                Cast.uuid("urn:uuid:01020304-0506-0708-090a-0b0c0d0e0f10"),
                UUID.fromString("01020304-0506-0708-090a-0b0c0d0e0f10"));
        check(
                "timestamp",
                Cast.timestamp("2026-01-02T15:04:05.123456789+05:00"),
                Instant.parse("2026-01-02T10:04:05.123456789Z"));
        check("unix", Cast.unix("1700000000", UnixPrecision.SECONDS), Instant.ofEpochSecond(1_700_000_000L));
        check("excelSerial", Cast.excelSerial("45292.75", ExcelEpoch.Y1900), Instant.parse("2024-01-01T18:00:00Z"));
        check("date", Cast.date("2026-01-02"), LocalDate.of(2026, 1, 2));
        check("date (ordered)", Cast.date("1/7/2026", DateOrder.MONTH_DAY_YEAR), LocalDate.of(2026, 1, 7));
        check(
                "dateTime",
                Cast.dateTime("1/7/2026 3:04 PM", DateOrder.MONTH_DAY_YEAR),
                LocalDateTime.of(2026, 1, 7, 15, 4));
        // The same door with a culture's designators declared: the one ABI shape that takes a
        // declared order and a pointer, crossing with the 40-byte layout where the line above
        // crossed with NULL — its own downcall signature in reachability-metadata.json.
        check(
                "dateTime (declared designators)",
                Cast.dateTime("1/7/2026 3:04 p. m.", new DateTimeFormat(DateOrder.MONTH_DAY_YEAR, "a. m.", "p. m.")),
                LocalDateTime.of(2026, 1, 7, 15, 4));
        check("time", Cast.time("15:04:05"), LocalTime.of(15, 4, 5));
        check("duration", Cast.duration("P1DT6H"), Duration.ofHours(30));
        // The typed doors, one per shape: their downcalls take a double, which is its own
        // registration in reachability-metadata.json.
        check("decimalFromDouble", Cast.decimalFromDouble(0.1 + 0.2), new BigDecimal("0.30000000000000004"));
        check(
                "excelSerialFromDouble",
                Cast.excelSerialFromDouble(45292.75, ExcelEpoch.Y1900),
                LocalDateTime.of(2024, 1, 1, 18, 0));
        check("excelTime", Cast.excelTime(0.75), LocalTime.of(18, 0));
        check("excelDuration", Cast.excelDuration(1.5), Duration.ofHours(36));

        // The other two input forms, once per ABI shape rather than once per door — the
        // overloads differ only in how the bytes arrive. A byte[] and a heap slice both
        // cross pinned, which is the heap-access half of the critical downcall
        // registration; a native segment crosses as the address it already is.
        byte[] line = "(1,234)|1700000000|true".getBytes(StandardCharsets.UTF_8);
        check("i32 (byte[])", Cast.i32("(1,234)".getBytes(StandardCharsets.UTF_8), NumFormat.INVARIANT), -1234);
        MemorySegment whole = MemorySegment.ofArray(line);
        check("i32 (heap slice)", Cast.i32(whole.asSlice(0, 7), NumFormat.INVARIANT), -1234);
        check(
                "unix (heap slice)",
                Cast.unix(whole.asSlice(8, 10), UnixPrecision.SECONDS),
                Instant.ofEpochSecond(1_700_000_000L));
        check("bool (heap slice)", Cast.bool(whole.asSlice(19, 4)), true);
        try (Arena arena = Arena.ofConfined()) {
            // By hand rather than allocateFrom, which JDK 21 lacks: the floor runs this too.
            byte[] text = "42.5|".getBytes(StandardCharsets.UTF_8);
            MemorySegment offHeap = arena.allocate(text.length);
            MemorySegment.copy(text, 0, offHeap, ValueLayout.JAVA_BYTE, 0, text.length);
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

        // The constraint doors: build() judges the declaration once and converts each bound
        // through the core's declared decimal, then the checked door and its double form run
        // under it, and the three-arm switch over the checked verdict must survive AOT.
        Constraints<Double> latitude =
                Constraints.f64().minimum("-90").maximum("90").build();
        checked("f64 checked", Cast.f64("45.5", NumFormat.INVARIANT, latitude), "checked 45.5");
        checked(
                "f64 checked (violation)",
                Cast.f64("91", NumFormat.INVARIANT, latitude),
                "violation maximum 91.0 @ 0+2");
        checked("f64FromDouble checked", Cast.f64FromDouble(-91.0, latitude), "violation minimum -91.0 @ 0+0");
        Constraints<String> sex =
                Constraints.text().trim(true).enumOf("M", "F", "X").build();
        checked("text checked", Cast.text(" F ", sex), "checked F");

        // The strict default, and the detailed path's four downcall shapes: a text door with
        // date options and a month vocabulary, a typed door, and the checked pair.
        report(
                "i32 (Western)",
                String.valueOf(Cast.i32("12,34", NumFormat.INVARIANT)),
                "Fault[reason=MALFORMED, offset=2, length=3]");
        check("i32 (Indian)", Cast.i32("1,23,456", NumFormat.INVARIANT.withGrouping(Grouping.INDIAN)), 123456);
        detailed("i32 detailed", DetailedCast.i32("12,34", NumFormat.INVARIANT), "fault MALFORMED group_size @ 2+3");
        detailed("date (common forms)", DetailedCast.date("20260107", DateOptions.COMMON_FORMS), "ok 2026-01-07");
        MonthNames english = MonthNames.builder()
                .add(1, "Jan")
                .add(2, "Feb")
                .add(3, "Mar")
                .add(4, "Apr")
                .add(5, "May")
                .add(6, "Jun")
                .add(7, "Jul")
                .add(8, "Aug")
                .add(9, "Sep")
                .add(10, "Oct")
                .add(11, "Nov")
                .add(12, "Dec")
                .build();
        detailed(
                "date (month names, pivot)",
                DetailedCast.date("07-Jan-26", DateOrder.DAY_MONTH_YEAR, new DateOptions(0, 2029), english),
                "ok 2026-01-07");
        detailed("excelTime detailed", DetailedCast.excelTime(Double.NaN), "fault MALFORMED not_finite @ 0+0");
        detailedChecked(
                "f64 checked detailed",
                DetailedCast.f64("91", NumFormat.INVARIANT, latitude),
                "violation maximum 91.0 @ 0+2");
        detailedChecked(
                "f64FromDouble checked detailed",
                DetailedCast.f64FromDouble(-91.0, latitude),
                "violation minimum -91.0 @ 0+0");

        System.out.println(failures == 0 ? "AOT smoke test passed." : "AOT smoke test FAILED (" + failures + ").");
        System.exit(failures == 0 ? 0 : 1);
    }

    // The path the JDK picked from the multi-release jar: FFM on 25 and later, the JNI base
    // below that, told apart by NativeCore's natives and, on a JVM, its class file major.
    private static void requireExpectedBackend() {
        boolean ffm = Runtime.version().feature() >= 25;
        String expected = ffm ? "ffm" : "jni";
        Class<?> core;
        try {
            core = Class.forName("io.github.skunkwerkx.hypercast.NativeCore");
        } catch (ClassNotFoundException e) {
            throw new AssertionError("NativeCore is not loadable", e);
        }
        long natives = Arrays.stream(core.getDeclaredMethods())
                .filter(m -> Modifier.isNative(m.getModifiers()))
                .count();
        require(natives == (ffm ? 0 : 37), expected + " expected, but NativeCore declares " + natives + " natives");
        String major = "n/a (image)";
        if (System.getProperty("org.graalvm.nativeimage.imagecode") == null) {
            try (InputStream in = core.getResourceAsStream("NativeCore.class")) {
                require(in != null, "NativeCore.class is not readable as a resource");
                DataInputStream data = new DataInputStream(in);
                data.readInt();
                data.readUnsignedShort();
                int found = data.readUnsignedShort();
                require(found == (ffm ? 69 : 65), expected + " expected, but NativeCore is class file major " + found);
                major = Integer.toString(found);
            } catch (IOException e) {
                throw new AssertionError("NativeCore.class is unreadable", e);
            }
        }
        System.out.println("backend: " + expected + " (" + natives + " natives, class major " + major + ")");
    }

    // The image must not carry the JNI library: only the FFM path survives into it (the base
    // NativeCore is unreachable), and the resource globs cover the core library alone. The
    // JNI library names are the three the Java side resolves, for each of the eight RIDs.
    private static void requireNoJniLibraryEmbedded() {
        String[] rids = {
            "win-x64",
            "win-arm64",
            "osx-x64",
            "osx-arm64",
            "linux-x64",
            "linux-arm64",
            "linux-musl-x64",
            "linux-musl-arm64"
        };
        String[] names = {"libhypercast_jni.so", "libhypercast_jni.dylib", "hypercast_jni.dll"};
        for (String rid : rids) {
            for (String name : names) {
                require(
                        Main.class.getResource("/native/" + rid + "/" + name) == null,
                        "the image embeds the JNI library /native/" + rid + "/" + name);
            }
        }
        System.out.println("image: no JNI library embedded");
    }

    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }
}
