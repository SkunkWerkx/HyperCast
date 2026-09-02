package io.github.skunkwerkx.hypercast.benchmarks;

import io.github.skunkwerkx.hypercast.Cast;
import io.github.skunkwerkx.hypercast.DateOrder;
import io.github.skunkwerkx.hypercast.NumFormat;
import io.github.skunkwerkx.hypercast.Verdict;
import java.lang.foreign.MemorySegment;
import java.nio.charset.StandardCharsets;
import java.text.NumberFormat;
import java.text.ParseException;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
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
    private final byte[] uuidBytes = uuidText.getBytes(StandardCharsets.UTF_8);
    // The round-three shape: one buffer holding many values, each door handed a heap slice.
    private final MemorySegment line = MemorySegment.ofArray(
            (intText + "|" + timestampText).getBytes(StandardCharsets.UTF_8));
    private final MemorySegment intSlice = line.asSlice(0, intBytes.length);
    private final MemorySegment timestampSlice = line.asSlice(intBytes.length + 1, timestampBytes.length);

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
    public Verdict<Integer> castI32GroupedSlice() {
        return Cast.i32(intSlice, INVARIANT);
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
    public Verdict<UUID> castUuidBytes() {
        return Cast.uuid(uuidBytes);
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
    public Verdict<Instant> castTimestampSlice() {
        return Cast.timestamp(timestampSlice);
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
    // --- the declared-order doors, vs the JDK pattern formatter that could accept the
    // same text. This is the messy-feed shape the doors exist for: "1/7/2026 3:04 PM"
    // has no java.time parser of its own, only a hand-built formatter.

    private static final DateTimeFormatter US_DATE_TIME =
            DateTimeFormatter.ofPattern("M/d/yyyy h:mm a", Locale.US);
    private static final DateTimeFormatter US_DATE = DateTimeFormatter.ofPattern("M/d/yyyy", Locale.US);

    private final String messyDateTimeText = "1/7/2026 3:04 PM";
    private final String isoDateTimeText = "2026-01-07T15:04:05";
    private final String messyDateText = "1/7/2026";

    @Benchmark
    public Verdict<LocalDateTime> castDateTimeMessy() {
        return Cast.dateTime(messyDateTimeText, DateOrder.MONTH_DAY_YEAR);
    }

    @Benchmark
    public LocalDateTime jdkPatternDateTimeParse() {
        return LocalDateTime.parse(messyDateTimeText, US_DATE_TIME);
    }

    @Benchmark
    public Verdict<LocalDateTime> castDateTimeIso() {
        return Cast.dateTime(isoDateTimeText, DateOrder.YEAR_MONTH_DAY);
    }

    @Benchmark
    public LocalDateTime jdkLocalDateTimeParse() {
        return LocalDateTime.parse(isoDateTimeText);
    }

    @Benchmark
    public Verdict<LocalDate> castDateOrdered() {
        return Cast.date(messyDateText, DateOrder.MONTH_DAY_YEAR);
    }

    @Benchmark
    public LocalDate jdkPatternDateParse() {
        return LocalDate.parse(messyDateText, US_DATE);
    }

    // --- separator detection, vs the same text under a declared eurozone format and vs
    // the JDK's own locale machinery. Detection's cost is one extra scan for '.'/','.

    private static final NumFormat EUROZONE =
            new NumFormat(',', '.', NumFormat.STYLE_ALL);
    private static final NumberFormat JDK_GERMAN = NumberFormat.getInstance(Locale.GERMANY);

    private final String euroNumberText = "1.234.567,89";

    @Benchmark
    public Verdict<Double> castF64Detect() {
        return Cast.f64(euroNumberText, NumFormat.DETECT);
    }

    @Benchmark
    public Verdict<Double> castF64Declared() {
        return Cast.f64(euroNumberText, EUROZONE);
    }

    @Benchmark
    public Number jdkNumberFormatGerman() throws ParseException {
        return JDK_GERMAN.parse(euroNumberText);
    }

}
