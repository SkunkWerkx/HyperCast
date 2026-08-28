package io.github.buvinghausen.hypercast.benchmarks;

import io.github.buvinghausen.hypercast.Cast;
import io.github.buvinghausen.hypercast.NumFormat;
import io.github.buvinghausen.hypercast.Verdict;
import java.nio.charset.StandardCharsets;
import java.text.NumberFormat;
import java.text.ParseException;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalTime;
import java.time.OffsetDateTime;
import java.time.format.DateTimeFormatter;
import java.util.Locale;
import java.util.UUID;
import java.util.concurrent.TimeUnit;
import org.openjdk.jmh.annotations.Benchmark;
import org.openjdk.jmh.annotations.BenchmarkMode;
import org.openjdk.jmh.annotations.Mode;
import org.openjdk.jmh.annotations.OutputTimeUnit;
import org.openjdk.jmh.annotations.Scope;
import org.openjdk.jmh.annotations.State;

/**
 * Each pair pits a HyperCast door (FFM crossing included) against the closest JDK parse
 * with equivalent settings declared — {@link NumberFormat} where grouping needs the JDK's
 * locale machinery, {@code java.time} parsers where that's simply how the JDK door works.
 * String-door numbers include the UTF-8 encode; the {@code byte[]} rows show the raw
 * crossing.
 */
@State(Scope.Benchmark)
@BenchmarkMode(Mode.AverageTime)
@OutputTimeUnit(TimeUnit.NANOSECONDS)
public class CastBenchmarks {
    private static final NumFormat INVARIANT = NumFormat.INVARIANT;
    private static final NumberFormat JDK_GROUPED = NumberFormat.getIntegerInstance(Locale.US);
    private static final DateTimeFormatter ISO_OFFSET = DateTimeFormatter.ISO_OFFSET_DATE_TIME;

    private final String boolText = "true";
    private final String intText = "1,234,567";
    private final String doubleText = "12345.6789";
    private final String uuidText = "01020304-0506-0708-090a-0b0c0d0e0f10";
    private final String timestampText = "2026-01-02T15:04:05.123456789Z";
    private final String offsetTimestampText = "2026-01-02T15:04:05.123456789+05:00";
    private final String isoDurationText = "PT1H30M15.5S";
    private final String timeText = "15:04:05.123456789";
    private final byte[] timestampBytes = timestampText.getBytes(StandardCharsets.UTF_8);
    private final byte[] intBytes = intText.getBytes(StandardCharsets.UTF_8);

    @Benchmark
    public Verdict<Boolean> castBool() {
        return Cast.bool(boolText);
    }

    @Benchmark
    public boolean jdkParseBoolean() {
        // Never fails by design — anything not "true" is silently false. Not equivalent
        // machinery, printed for scale only.
        return Boolean.parseBoolean(boolText);
    }

    @Benchmark
    public Verdict<Integer> castI32Grouped() {
        return Cast.i32(intText, INVARIANT);
    }

    @Benchmark
    public Verdict<Integer> castI32GroupedBytes() {
        return Cast.i32(intBytes, INVARIANT);
    }

    @Benchmark
    public Number jdkNumberFormatGrouped() throws ParseException {
        // The JDK's own grouped-integer door. Stateful and not thread-safe, unlike Cast.
        return JDK_GROUPED.parse(intText);
    }

    @Benchmark
    public Verdict<Double> castF64() {
        return Cast.f64(doubleText, INVARIANT);
    }

    @Benchmark
    public double jdkParseDouble() {
        return Double.parseDouble(doubleText);
    }

    @Benchmark
    public Verdict<UUID> castUuid() {
        return Cast.uuid(uuidText);
    }

    @Benchmark
    public UUID jdkUuidFromString() {
        return UUID.fromString(uuidText);
    }

    @Benchmark
    public Verdict<Instant> castTimestamp() {
        return Cast.timestamp(timestampText);
    }

    @Benchmark
    public Verdict<Instant> castTimestampBytes() {
        return Cast.timestamp(timestampBytes);
    }

    @Benchmark
    public Instant jdkInstantParse() {
        return Instant.parse(timestampText);
    }

    @Benchmark
    public Verdict<Instant> castTimestampOffset() {
        return Cast.timestamp(offsetTimestampText);
    }

    @Benchmark
    public Instant jdkOffsetDateTimeParse() {
        return OffsetDateTime.parse(offsetTimestampText, ISO_OFFSET).toInstant();
    }

    @Benchmark
    public Verdict<Duration> castDurationIso() {
        return Cast.duration(isoDurationText);
    }

    @Benchmark
    public Duration jdkDurationParse() {
        return Duration.parse(isoDurationText);
    }

    @Benchmark
    public Verdict<LocalTime> castTime() {
        return Cast.time(timeText);
    }

    @Benchmark
    public LocalTime jdkLocalTimeParse() {
        return LocalTime.parse(timeText);
    }
}
