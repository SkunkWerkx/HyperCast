package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.lang.foreign.MemorySegment;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.UUID;
import org.junit.jupiter.api.Test;

/**
 * Replays the shared conformance corpus ({@code corpus/*.json} at the repository root) —
 * the same files the Rust core's and C# binding's suites replay — through this binding's
 * UTF-8 doors. Fault spans are byte offsets into the UTF-8 input, which is exactly what
 * these doors receive, so span assertions hold verbatim. This is the byte-for-byte polyglot
 * contract: a vector that fails here is a break in the promise, not just a failing test.
 */
final class CorpusTest {
    private static final Path CORPUS = findCorpusDirectory();

    private static Path findCorpusDirectory() {
        for (Path dir = Path.of("").toAbsolutePath(); dir != null; dir = dir.getParent()) {
            Path corpus = dir.resolve("corpus");
            if (Files.isDirectory(corpus)) {
                return corpus;
            }
        }
        throw new IllegalStateException(
                "corpus directory not found above " + Path.of("").toAbsolutePath());
    }

    private static JsonArray corpus(String name) {
        try {
            return JsonParser.parseString(Files.readString(CORPUS.resolve(name)))
                    .getAsJsonArray();
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    private static byte[] inputBytes(JsonObject vector) {
        return vector.get("input").getAsString().getBytes(StandardCharsets.UTF_8);
    }

    /**
     * The format a vector declares: the two separators and the flags always, every other
     * field only where the vector names it — absent is not declared, exactly as the core's
     * own replay reads it.
     */
    private static NumFormat formatOf(JsonObject vector) {
        if (!vector.has("format")) {
            return NumFormat.INVARIANT;
        }
        JsonObject format = vector.getAsJsonObject("format");
        String digitZero = symbolOf(format, "digit_zero");
        return new NumFormat(
                format.get("decimal_sep").getAsString(),
                format.get("group_sep").getAsString(),
                format.get("flags").getAsInt(),
                symbolOf(format, "currency"),
                symbolOf(format, "currency_decimal_sep"),
                symbolOf(format, "currency_group_sep"),
                symbolOf(format, "minus_sign"),
                symbolOf(format, "plus_sign"),
                symbolOf(format, "percent"),
                digitZero.isEmpty() ? 0 : digitZero.codePointAt(0));
    }

    private static String symbolOf(JsonObject object, String key) {
        return object.has(key) ? object.get(key).getAsString() : "";
    }

    private static <T> void assertVerdict(String domain, JsonObject vector, Verdict<T> verdict, T expected) {
        String input = vector.get("input").getAsString();
        String expect = vector.get("expect").getAsString();
        // The union payoff in action: two case arms, no default, exhaustive over the sealed set.
        switch (verdict) {
            case Success<T> success -> {
                assertEquals("ok", expect, domain + ": '" + input + "' unexpectedly parsed");
                assertSame(expected, success.value(), domain + ": '" + input + "'");
            }
            case Fault<T> fault -> {
                CastFailure reason = switch (expect) {
                    case "empty" -> CastFailure.EMPTY;
                    case "malformed" -> CastFailure.MALFORMED;
                    case "out_of_range" -> CastFailure.OUT_OF_RANGE;
                    default -> fail(domain + ": '" + input + "' expected '" + expect + "' but faulted");
                };
                assertEquals(reason, fault.reason(), domain + ": '" + input + "'");
                if (vector.has("fault")) {
                    JsonArray span = vector.getAsJsonArray("fault");
                    assertEquals(span.get(0).getAsInt(), fault.offset(), domain + ": '" + input + "' fault offset");
                    assertEquals(span.get(1).getAsInt(), fault.length(), domain + ": '" + input + "' fault length");
                }
            }
        }
    }

    /**
     * Values compare as the core wrote them: a float or double by its bits, so zero, which the
     * core writes in one form, must arrive unsigned (boxed {@code equals} would also tell
     * {@code -0.0} apart, but by a rule Java could change for NaN; the bits are the contract),
     * and a {@link BigDecimal} by value and scale, which {@code equals} already compares.
     */
    private static void assertSame(Object expected, Object actual, String label) {
        if (expected instanceof Double d && actual instanceof Double a) {
            assertEquals(
                    Long.toHexString(Double.doubleToRawLongBits(d)),
                    Long.toHexString(Double.doubleToRawLongBits(a)),
                    label + " bits");
        } else if (expected instanceof Float f && actual instanceof Float a) {
            assertEquals(
                    Integer.toHexString(Float.floatToRawIntBits(f)),
                    Integer.toHexString(Float.floatToRawIntBits(a)),
                    label + " bits");
        } else {
            assertEquals(expected, actual, label);
        }
    }

    /**
     * The corpus pins a char as its code point. A {@code char} holds only the BMP, so a
     * supplementary scalar the core accepts is this binding's out-of-range over the trimmed
     * input — the one place a Java verdict departs from the core's, by design.
     */
    @Test
    void charCorpus() {
        for (JsonElement element : corpus("char.json")) {
            JsonObject vector = element.getAsJsonObject().deepCopy();
            Character expected = null;
            if (vector.has("value")) {
                int scalar = vector.get("value").getAsInt();
                if (scalar > Character.MAX_VALUE) {
                    byte[] bytes = inputBytes(vector);
                    int start = 0;
                    int end = bytes.length;
                    while (start < end && " \t\n\f\r".indexOf(bytes[start]) >= 0) {
                        start++;
                    }
                    while (end > start && " \t\n\f\r".indexOf(bytes[end - 1]) >= 0) {
                        end--;
                    }
                    vector.addProperty("expect", "out_of_range");
                    JsonArray span = new JsonArray();
                    span.add(start);
                    span.add(end - start);
                    vector.add("fault", span);
                } else {
                    expected = (char) scalar;
                }
            }
            assertVerdict("char", vector, Cast.character(inputBytes(vector)), expected);
        }
    }

    @Test
    void booleanCorpus() {
        for (JsonElement element : corpus("boolean.json")) {
            JsonObject vector = element.getAsJsonObject();
            Boolean expected = vector.has("value") ? vector.get("value").getAsBoolean() : null;
            assertVerdict("boolean", vector, Cast.bool(inputBytes(vector)), expected);
        }
    }

    @Test
    void integerCorpus() {
        for (JsonElement element : corpus("integer.json")) {
            JsonObject vector = element.getAsJsonObject();
            byte[] input = inputBytes(vector);
            NumFormat format = formatOf(vector);
            JsonElement value = vector.get("value");
            switch (vector.get("type").getAsString()) {
                case "i8" ->
                    assertVerdict("integer", vector, Cast.i8(input, format), value == null ? null : value.getAsByte());
                case "i16" ->
                    assertVerdict(
                            "integer", vector, Cast.i16(input, format), value == null ? null : value.getAsShort());
                case "i32" ->
                    assertVerdict("integer", vector, Cast.i32(input, format), value == null ? null : value.getAsInt());
                case "i64" ->
                    assertVerdict("integer", vector, Cast.i64(input, format), value == null ? null : value.getAsLong());
                case "u8" ->
                    assertVerdict("integer", vector, Cast.u8(input, format), value == null ? null : value.getAsInt());
                case "u16" ->
                    assertVerdict("integer", vector, Cast.u16(input, format), value == null ? null : value.getAsInt());
                case "u32" ->
                    assertVerdict("integer", vector, Cast.u32(input, format), value == null ? null : value.getAsLong());
                case "u64" ->
                    assertVerdict(
                            "integer",
                            vector,
                            Cast.u64(input, format),
                            value == null ? null : value.getAsBigInteger().longValue());
                default -> fail("integer: unknown type " + vector.get("type"));
            }
        }
    }

    @Test
    void realCorpus() {
        for (JsonElement element : corpus("real.json")) {
            JsonObject vector = element.getAsJsonObject();
            byte[] input = inputBytes(vector);
            NumFormat format = formatOf(vector);
            JsonElement value = vector.get("value");
            switch (vector.get("type").getAsString()) {
                case "f32" ->
                    assertVerdict(
                            "real",
                            vector,
                            Cast.f32(input, format),
                            value == null ? null : (float) value.getAsDouble());
                case "f64" ->
                    assertVerdict("real", vector, Cast.f64(input, format), value == null ? null : value.getAsDouble());
                default -> fail("real: unknown type " + vector.get("type"));
            }
        }
    }

    @Test
    void decimalCorpus() {
        for (JsonElement element : corpus("decimal.json")) {
            JsonObject vector = element.getAsJsonObject();
            String input = vector.get("input").getAsString();
            BigDecimal expected = null;
            if (vector.has("magnitude")) {
                // The triple the core hands back — sign, 96-bit magnitude, scale — is
                // exactly BigDecimal's own decomposition, so it is pinned field by field...
                BigInteger magnitude = new BigInteger(vector.get("magnitude").getAsString());
                expected = new BigDecimal(
                        vector.get("negative").getAsBoolean() ? magnitude.negate() : magnitude,
                        vector.get("scale").getAsInt());
                // ...and the canonical text is the cross-binding check: same value, same scale.
                BigDecimal canonical = new BigDecimal(vector.get("value").getAsString());
                assertEquals(0, canonical.compareTo(expected), "decimal: '" + input + "' canonical value");
                assertEquals(canonical.scale(), expected.scale(), "decimal: '" + input + "' canonical scale");
            }
            Verdict<BigDecimal> verdict = Cast.decimal(inputBytes(vector), formatOf(vector));
            assertVerdict("decimal", vector, verdict, expected);
            if (verdict instanceof Success<BigDecimal> success) {
                assertEquals(
                        expected.unscaledValue(),
                        success.value().unscaledValue(),
                        "decimal: '" + input + "' magnitude");
                assertEquals(expected.scale(), success.value().scale(), "decimal: '" + input + "' scale");
                assertEquals(expected.signum(), success.value().signum(), "decimal: '" + input + "' sign");
            }
        }
    }

    @Test
    void uuidCorpus() {
        for (JsonElement element : corpus("uuid.json")) {
            JsonObject vector = element.getAsJsonObject();
            UUID expected = null;
            if (vector.has("value")) {
                String hex = vector.get("value").getAsString();
                expected = new UUID(
                        Long.parseUnsignedLong(hex.substring(0, 16), 16),
                        Long.parseUnsignedLong(hex.substring(16, 32), 16));
            }
            assertVerdict("uuid", vector, Cast.uuid(inputBytes(vector)), expected);
        }
    }

    private static Instant expectedInstant(JsonObject vector) {
        return vector.has("seconds")
                ? Instant.ofEpochSecond(
                        vector.get("seconds").getAsLong(), vector.get("nanos").getAsLong())
                : null;
    }

    @Test
    void timestampCorpus() {
        for (JsonElement element : corpus("timestamp.json")) {
            JsonObject vector = element.getAsJsonObject();
            assertVerdict("timestamp", vector, Cast.timestamp(inputBytes(vector)), expectedInstant(vector));
        }
    }

    @Test
    void unixCorpus() {
        for (JsonElement element : corpus("unix.json")) {
            JsonObject vector = element.getAsJsonObject();
            UnixPrecision precision = switch (vector.get("precision").getAsInt()) {
                case 1 -> UnixPrecision.SECONDS;
                case 2 -> UnixPrecision.MILLISECONDS;
                case 3 -> UnixPrecision.MICROSECONDS;
                case 4 -> UnixPrecision.NANOSECONDS;
                default -> throw new IllegalStateException("unix: unknown precision " + vector.get("precision"));
            };
            assertVerdict("unix", vector, Cast.unix(inputBytes(vector), precision), expectedInstant(vector));
        }
    }

    @Test
    void excelSerialCorpus() {
        for (JsonElement element : corpus("excel_serial.json")) {
            JsonObject vector = element.getAsJsonObject();
            ExcelEpoch epoch = switch (vector.get("epoch").getAsInt()) {
                case 1 -> ExcelEpoch.Y1900;
                case 2 -> ExcelEpoch.Y1904;
                default -> throw new IllegalStateException("excel_serial: unknown epoch " + vector.get("epoch"));
            };
            assertVerdict("excel_serial", vector, Cast.excelSerial(inputBytes(vector), epoch), expectedInstant(vector));
        }
    }

    @Test
    void dateCorpus() {
        for (JsonElement element : corpus("date.json")) {
            JsonObject vector = element.getAsJsonObject();
            LocalDate expected = vector.has("year")
                    ? LocalDate.of(
                            vector.get("year").getAsInt(),
                            vector.get("month").getAsInt(),
                            vector.get("day").getAsInt())
                    : null;
            assertVerdict("date", vector, Cast.date(inputBytes(vector)), expected);
        }
    }

    @Test
    void dateOrderCorpus() {
        for (JsonElement element : corpus("date_order.json")) {
            JsonObject vector = element.getAsJsonObject();
            DateOrder order = switch (vector.get("order").getAsInt()) {
                case 1 -> DateOrder.YEAR_MONTH_DAY;
                case 2 -> DateOrder.MONTH_DAY_YEAR;
                case 3 -> DateOrder.DAY_MONTH_YEAR;
                default -> throw new AssertionError("date_order: unknown order");
            };
            LocalDate expected = vector.has("year")
                    ? LocalDate.of(
                            vector.get("year").getAsInt(),
                            vector.get("month").getAsInt(),
                            vector.get("day").getAsInt())
                    : null;
            assertVerdict("date_order", vector, Cast.date(inputBytes(vector), order), expected);
        }
    }

    @Test
    void dateTimeCorpus() {
        for (JsonElement element : corpus("datetime.json")) {
            JsonObject vector = element.getAsJsonObject();
            DateOrder order = switch (vector.get("order").getAsInt()) {
                case 1 -> DateOrder.YEAR_MONTH_DAY;
                case 2 -> DateOrder.MONTH_DAY_YEAR;
                case 3 -> DateOrder.DAY_MONTH_YEAR;
                default -> throw new AssertionError("datetime: unknown order");
            };
            LocalDateTime expected = vector.has("year")
                    ? LocalDateTime.of(
                            LocalDate.of(
                                    vector.get("year").getAsInt(),
                                    vector.get("month").getAsInt(),
                                    vector.get("day").getAsInt()),
                            LocalTime.ofNanoOfDay(vector.get("nanos_of_day").getAsLong()))
                    : null;
            byte[] input = inputBytes(vector);
            // A vector declaring designators goes through the DateTimeFormat door; one that
            // declares none through the DateOrder door it always used, which must agree with
            // a DateTimeFormat declaring none — the same null pointer either way.
            DateTimeFormat format = new DateTimeFormat(order, symbolOf(vector, "am"), symbolOf(vector, "pm"));
            if (format.englishOnly()) {
                assertVerdict("datetime", vector, Cast.dateTime(input, order), expected);
                assertEquals(Cast.dateTime(input, order), Cast.dateTime(input, format));
            } else {
                assertVerdict("datetime", vector, Cast.dateTime(input, format), expected);
            }
        }
    }

    @Test
    void timeCorpus() {
        for (JsonElement element : corpus("time.json")) {
            JsonObject vector = element.getAsJsonObject();
            LocalTime expected = vector.has("nanos")
                    ? LocalTime.ofNanoOfDay(vector.get("nanos").getAsLong())
                    : null;
            assertVerdict("time", vector, Cast.time(inputBytes(vector)), expected);
        }
    }

    @Test
    void durationCorpus() {
        for (JsonElement element : corpus("duration.json")) {
            JsonObject vector = element.getAsJsonObject();
            Duration expected = vector.has("seconds")
                    ? Duration.ofSeconds(
                            vector.get("seconds").getAsLong(),
                            vector.get("nanos").getAsLong())
                    : null;
            assertVerdict("duration", vector, Cast.duration(inputBytes(vector)), expected);
        }
    }

    /**
     * The typed doors, by bits: each double is rebuilt exactly from its IEEE 754 pattern — NaN
     * and the infinities included, which JSON cannot spell. A typed door's fault has no span.
     */
    @Test
    void typedCorpus() {
        java.util.Set<String> doors = new java.util.TreeSet<>();
        for (JsonElement element : corpus("typed.json")) {
            JsonObject vector = element.getAsJsonObject();
            double value = Double.longBitsToDouble(
                    Long.parseUnsignedLong(vector.get("bits").getAsString(), 16));
            String door = vector.get("door").getAsString();
            doors.add(door);
            Verdict<?> verdict = switch (door) {
                case "decimal" -> {
                    BigDecimal expected = null;
                    if (vector.has("magnitude")) {
                        BigInteger magnitude =
                                new BigInteger(vector.get("magnitude").getAsString());
                        expected = new BigDecimal(
                                vector.get("negative").getAsBoolean() ? magnitude.negate() : magnitude,
                                vector.get("scale").getAsInt());
                        assertEquals(
                                vector.get("value").getAsString(), expected.toPlainString(), "typed decimal: value");
                    }
                    Verdict<BigDecimal> cast = Cast.decimalFromDouble(value);
                    assertVerdict("typed decimal", vector, cast, expected);
                    yield cast;
                }
                case "excel_serial" -> {
                    ExcelEpoch epoch = vector.get("epoch").getAsInt() == 1 ? ExcelEpoch.Y1900 : ExcelEpoch.Y1904;
                    LocalDateTime expected = vector.has("year")
                            ? LocalDateTime.of(
                                    LocalDate.of(
                                            vector.get("year").getAsInt(),
                                            vector.get("month").getAsInt(),
                                            vector.get("day").getAsInt()),
                                    LocalTime.ofNanoOfDay(
                                            vector.get("nanos_of_day").getAsLong()))
                            : null;
                    Verdict<LocalDateTime> cast = Cast.excelSerialFromDouble(value, epoch);
                    assertVerdict("typed excel_serial", vector, cast, expected);
                    yield cast;
                }
                case "excel_time" -> {
                    LocalTime expected = vector.has("nanos")
                            ? LocalTime.ofNanoOfDay(vector.get("nanos").getAsLong())
                            : null;
                    Verdict<LocalTime> cast = Cast.excelTime(value);
                    assertVerdict("typed excel_time", vector, cast, expected);
                    yield cast;
                }
                case "excel_duration" -> {
                    Duration expected = vector.has("seconds")
                            ? Duration.ofSeconds(
                                    vector.get("seconds").getAsLong(),
                                    vector.get("nanos").getAsLong())
                            : null;
                    Verdict<Duration> cast = Cast.excelDuration(value);
                    assertVerdict("typed excel_duration", vector, cast, expected);
                    yield cast;
                }
                default -> throw new AssertionError("typed: unknown door " + door);
            };
            if (verdict instanceof Fault<?> fault) {
                assertEquals(0, fault.offset(), "typed " + door + ": span offset");
                assertEquals(0, fault.length(), "typed " + door + ": span length");
            }
        }
        assertEquals(java.util.Set.of("decimal", "excel_serial", "excel_time", "excel_duration"), doors);
    }

    // --- the validation constraints ---

    /** The core's Refusal, as Rust displays it: the wording every binding's message carries. */
    private static final String[] REFUSALS = {
        "",
        "a constraint the door does not take",
        "minimum together with exclusiveMinimum",
        "maximum together with exclusiveMaximum",
        "the lower bound is not below the upper bound",
        "multipleOf is not positive",
        "a rounding without multipleOf",
        "minLength is above maxLength",
        "a bound, step or enum entry the door cannot hold",
        "an enum entry that is not UTF-8",
        "a duplicate enum entry",
        "a malformed constraints layout",
        "an enum with no entries",
        "a text enum entry no input can match",
    };

    static String refusalMessage(int code) {
        return "Constraints refused (code " + code + "): " + REFUSALS[code] + ".";
    }

    /** Every keyword a vector names, whether or not its door takes it, through the public builder. */
    private static <T> Constraints<T> declared(Constraints.Builder<T> builder, JsonObject c) {
        if (c.has("minimum")) {
            builder.minimum(c.get("minimum").getAsString());
        }
        if (c.has("exclusiveMinimum")) {
            builder.exclusiveMinimum(c.get("exclusiveMinimum").getAsString());
        }
        if (c.has("maximum")) {
            builder.maximum(c.get("maximum").getAsString());
        }
        if (c.has("exclusiveMaximum")) {
            builder.exclusiveMaximum(c.get("exclusiveMaximum").getAsString());
        }
        if (c.has("multipleOf")) {
            builder.multipleOf(c.get("multipleOf").getAsString());
        }
        if (c.has("rounding")) {
            builder.rounding(
                    switch (c.get("rounding").getAsString()) {
                        case "strict" -> Rounding.STRICT;
                        case "midpoint" -> Rounding.MIDPOINT;
                        case "floor" -> Rounding.FLOOR;
                        case "ceiling" -> Rounding.CEILING;
                        default -> throw new AssertionError("unknown rounding " + c.get("rounding"));
                    });
        }
        if (c.has("minLength")) {
            builder.minLength(c.get("minLength").getAsInt());
        }
        if (c.has("maxLength")) {
            builder.maxLength(c.get("maxLength").getAsInt());
        }
        if (c.has("trim")) {
            builder.trim(c.get("trim").getAsBoolean());
        }
        if (c.has("enum")) {
            JsonArray entries = c.getAsJsonArray("enum");
            String[] values = new String[entries.size()];
            for (int i = 0; i < values.length; i++) {
                values[i] = entries.get(i).getAsString();
            }
            builder.enumOf(values);
        }
        return builder.build();
    }

    private static String label(JsonObject vector) {
        String row = "constraints " + vector.get("door").getAsString() + " " + vector.get("constraints");
        if (vector.has("input")) {
            return row + " '" + vector.get("input").getAsString() + "'";
        }
        if (vector.has("from_f64")) {
            return row + " from_f64 " + vector.get("from_f64");
        }
        return vector.has("input_bytes") ? row + " bytes " + vector.get("input_bytes") : row;
    }

    /** A refused row throws with its code's message; any other row is declared. */
    private static <T> Constraints<T> declare(Constraints.Builder<T> builder, JsonObject vector) {
        JsonObject c = vector.getAsJsonObject("constraints");
        if (!vector.get("expect").getAsString().equals("refused")) {
            return declared(builder, c);
        }
        int code = vector.get("refusal").getAsInt();
        IllegalArgumentException refused =
                assertThrows(IllegalArgumentException.class, () -> declared(builder, c), label(vector));
        assertEquals(refusalMessage(code), refused.getMessage(), label(vector));
        return null;
    }

    private static double fromF64(JsonElement value) {
        if (value.getAsJsonPrimitive().isNumber()) {
            return value.getAsDouble();
        }
        return switch (value.getAsString()) {
            case "-0" -> -0.0;
            case "NaN" -> Double.NaN;
            case "Infinity" -> Double.POSITIVE_INFINITY;
            case "-Infinity" -> Double.NEGATIVE_INFINITY;
            default -> throw new AssertionError("unknown from_f64 " + value);
        };
    }

    /** One numeric door's checked overloads: text over a String, bytes and a segment, and a double. */
    private interface NumericDoor<T> {
        CheckedVerdict<T> text(String text, NumFormat format, Constraints<T> constraints);

        CheckedVerdict<T> bytes(byte[] utf8, NumFormat format, Constraints<T> constraints);

        CheckedVerdict<T> segment(MemorySegment utf8, NumFormat format, Constraints<T> constraints);

        CheckedVerdict<T> fromDouble(double value, Constraints<T> constraints);
    }

    private static <T> void numericRow(
            JsonObject vector,
            Constraints.Builder<T> builder,
            NumericDoor<T> door,
            java.util.function.Function<JsonElement, T> expected) {
        Constraints<T> constraints = declare(builder, vector);
        if (constraints == null) {
            return;
        }
        String label = label(vector);
        if (vector.has("from_f64")) {
            assertChecked(
                    label, vector, door.fromDouble(fromF64(vector.get("from_f64")), constraints), u -> u, expected);
            return;
        }
        String text = vector.get("input").getAsString();
        byte[] bytes = text.getBytes(StandardCharsets.UTF_8);
        NumFormat format = formatOf(vector);
        assertChecked(label, vector, door.bytes(bytes, format, constraints), u -> u, expected);
        assertChecked(
                label + " (segment)",
                vector,
                door.segment(MemorySegment.ofArray(bytes), format, constraints),
                u -> u,
                expected);
        assertChecked(
                label + " (string)", vector, door.text(text, format, constraints), u -> chars(bytes, u), expected);
    }

    // A corpus byte offset into the UTF-8 input as the String door's char offset.
    private static int chars(byte[] utf8, int bytes) {
        return new String(utf8, 0, bytes, StandardCharsets.UTF_8).length();
    }

    private static void textRow(JsonObject vector) {
        Constraints<String> constraints = declare(Constraints.text(), vector);
        if (constraints == null) {
            return;
        }
        String label = label(vector);
        java.util.function.Function<JsonElement, String> value = JsonElement::getAsString;
        if (vector.has("input_bytes")) {
            JsonArray raw = vector.getAsJsonArray("input_bytes");
            byte[] invalid = new byte[raw.size()];
            for (int i = 0; i < invalid.length; i++) {
                invalid[i] = (byte) raw.get(i).getAsInt();
            }
            assertChecked(label, vector, Cast.text(invalid, constraints), u -> u, value);
            assertChecked(
                    label + " (segment)",
                    vector,
                    Cast.text(MemorySegment.ofArray(invalid), constraints),
                    u -> u,
                    value);
            return;
        }
        String text = vector.get("input").getAsString();
        byte[] bytes = text.getBytes(StandardCharsets.UTF_8);
        assertChecked(label, vector, Cast.text(bytes, constraints), u -> u, value);
        assertChecked(
                label + " (segment)", vector, Cast.text(MemorySegment.ofArray(bytes), constraints), u -> u, value);
        CheckedVerdict<String> fromString = Cast.text(text, constraints);
        assertChecked(label + " (string)", vector, fromString, u -> chars(bytes, u), value);
        if (fromString instanceof Checked<String> ok && ok.enumIndex().isPresent()) {
            // A match hands back the caller's own declared string, the very instance.
            assertTrue(ok.value() == constraints.entry(ok.enumIndex().getAsInt()), label + ": the declared instance");
        }
    }

    // `units` re-expresses a corpus byte offset in the door's own units.
    private static <T> void assertChecked(
            String label,
            JsonObject vector,
            CheckedVerdict<T> verdict,
            java.util.function.IntUnaryOperator units,
            java.util.function.Function<JsonElement, T> expected) {
        String expect = vector.get("expect").getAsString();
        if (verdict instanceof Checked<T> ok) {
            assertEquals("ok", expect, label + ": unexpectedly passed as " + verdict);
            assertSame(expected.apply(vector.get("value")), ok.value(), label + ": value");
            assertEquals(vector.get("adjusted").getAsBoolean(), ok.adjusted(), label + ": adjusted");
            assertEquals(
                    vector.has("enum_index")
                            ? java.util.OptionalInt.of(vector.get("enum_index").getAsInt())
                            : java.util.OptionalInt.empty(),
                    ok.enumIndex(),
                    label + ": enum_index");
            return;
        }
        JsonArray span = vector.getAsJsonArray("fault");
        int offset = units.applyAsInt(span.get(0).getAsInt());
        int length = units.applyAsInt(span.get(0).getAsInt() + span.get(1).getAsInt()) - offset;
        // Exhaustive over the sealed Rejection: no default arm.
        switch ((Rejection<T>) verdict) {
            case Violation<T> violation -> {
                assertEquals("violation", expect, label + ": " + verdict);
                assertEquals(
                        vector.get("keyword").getAsString(), violation.keyword().name(), label + ": keyword");
                assertEquals(offset, violation.offset(), label + ": violation offset");
                assertEquals(length, violation.length(), label + ": violation length");
            }
            case Fault<T> fault -> {
                CastFailure reason = switch (expect) {
                    case "empty" -> CastFailure.EMPTY;
                    case "malformed" -> CastFailure.MALFORMED;
                    case "out_of_range" -> CastFailure.OUT_OF_RANGE;
                    default -> fail(label + ": expected '" + expect + "' but faulted " + fault);
                };
                assertEquals(reason, fault.reason(), label);
                assertEquals(offset, fault.offset(), label + ": fault offset");
                assertEquals(length, fault.length(), label + ": fault length");
            }
        }
    }

    /**
     * The checked doors ({@code corpus/constraints.json}), every row: each declaration is built
     * through the public {@link Constraints} builder from the corpus's own text (so bounds,
     * steps and integer entries cross the core's conversion, {@code cast_declared_decimal}),
     * and a {@code refused} row must throw with its code's message. Every other row is replayed
     * through the door's {@code byte[]}, {@link MemorySegment} and {@link String} overloads
     * (spans re-expressed in chars), or its {@code FromDouble} one; the byte rows, which no
     * string can carry, through the first two alone. Floats compare by bits.
     */
    @Test
    void constraintsCorpus() {
        java.util.Set<String> seen = new java.util.TreeSet<>();
        int rows = 0;
        for (JsonElement element : corpus("constraints.json")) {
            JsonObject vector = element.getAsJsonObject();
            String door = vector.get("door").getAsString();
            seen.add(vector.get("expect").getAsString().equals("refused") ? "refused" : door);
            rows++;
            switch (door) {
                case "i8" ->
                    numericRow(
                            vector,
                            Constraints.i8(),
                            new NumericDoor<>() {
                                public CheckedVerdict<Byte> text(String t, NumFormat f, Constraints<Byte> c) {
                                    return Cast.i8(t, f, c);
                                }

                                public CheckedVerdict<Byte> bytes(byte[] t, NumFormat f, Constraints<Byte> c) {
                                    return Cast.i8(t, f, c);
                                }

                                public CheckedVerdict<Byte> segment(MemorySegment t, NumFormat f, Constraints<Byte> c) {
                                    return Cast.i8(t, f, c);
                                }

                                public CheckedVerdict<Byte> fromDouble(double v, Constraints<Byte> c) {
                                    return Cast.i8FromDouble(v, c);
                                }
                            },
                            JsonElement::getAsByte);
                case "i16" ->
                    numericRow(
                            vector,
                            Constraints.i16(),
                            new NumericDoor<>() {
                                public CheckedVerdict<Short> text(String t, NumFormat f, Constraints<Short> c) {
                                    return Cast.i16(t, f, c);
                                }

                                public CheckedVerdict<Short> bytes(byte[] t, NumFormat f, Constraints<Short> c) {
                                    return Cast.i16(t, f, c);
                                }

                                public CheckedVerdict<Short> segment(
                                        MemorySegment t, NumFormat f, Constraints<Short> c) {
                                    return Cast.i16(t, f, c);
                                }

                                public CheckedVerdict<Short> fromDouble(double v, Constraints<Short> c) {
                                    return Cast.i16FromDouble(v, c);
                                }
                            },
                            JsonElement::getAsShort);
                case "i32" ->
                    numericRow(
                            vector,
                            Constraints.i32(),
                            new NumericDoor<>() {
                                public CheckedVerdict<Integer> text(String t, NumFormat f, Constraints<Integer> c) {
                                    return Cast.i32(t, f, c);
                                }

                                public CheckedVerdict<Integer> bytes(byte[] t, NumFormat f, Constraints<Integer> c) {
                                    return Cast.i32(t, f, c);
                                }

                                public CheckedVerdict<Integer> segment(
                                        MemorySegment t, NumFormat f, Constraints<Integer> c) {
                                    return Cast.i32(t, f, c);
                                }

                                public CheckedVerdict<Integer> fromDouble(double v, Constraints<Integer> c) {
                                    return Cast.i32FromDouble(v, c);
                                }
                            },
                            JsonElement::getAsInt);
                case "i64" ->
                    numericRow(
                            vector,
                            Constraints.i64(),
                            new NumericDoor<>() {
                                public CheckedVerdict<Long> text(String t, NumFormat f, Constraints<Long> c) {
                                    return Cast.i64(t, f, c);
                                }

                                public CheckedVerdict<Long> bytes(byte[] t, NumFormat f, Constraints<Long> c) {
                                    return Cast.i64(t, f, c);
                                }

                                public CheckedVerdict<Long> segment(MemorySegment t, NumFormat f, Constraints<Long> c) {
                                    return Cast.i64(t, f, c);
                                }

                                public CheckedVerdict<Long> fromDouble(double v, Constraints<Long> c) {
                                    return Cast.i64FromDouble(v, c);
                                }
                            },
                            JsonElement::getAsLong);
                case "u8" ->
                    numericRow(
                            vector,
                            Constraints.u8(),
                            new NumericDoor<>() {
                                public CheckedVerdict<Integer> text(String t, NumFormat f, Constraints<Integer> c) {
                                    return Cast.u8(t, f, c);
                                }

                                public CheckedVerdict<Integer> bytes(byte[] t, NumFormat f, Constraints<Integer> c) {
                                    return Cast.u8(t, f, c);
                                }

                                public CheckedVerdict<Integer> segment(
                                        MemorySegment t, NumFormat f, Constraints<Integer> c) {
                                    return Cast.u8(t, f, c);
                                }

                                public CheckedVerdict<Integer> fromDouble(double v, Constraints<Integer> c) {
                                    return Cast.u8FromDouble(v, c);
                                }
                            },
                            JsonElement::getAsInt);
                case "u16" ->
                    numericRow(
                            vector,
                            Constraints.u16(),
                            new NumericDoor<>() {
                                public CheckedVerdict<Integer> text(String t, NumFormat f, Constraints<Integer> c) {
                                    return Cast.u16(t, f, c);
                                }

                                public CheckedVerdict<Integer> bytes(byte[] t, NumFormat f, Constraints<Integer> c) {
                                    return Cast.u16(t, f, c);
                                }

                                public CheckedVerdict<Integer> segment(
                                        MemorySegment t, NumFormat f, Constraints<Integer> c) {
                                    return Cast.u16(t, f, c);
                                }

                                public CheckedVerdict<Integer> fromDouble(double v, Constraints<Integer> c) {
                                    return Cast.u16FromDouble(v, c);
                                }
                            },
                            JsonElement::getAsInt);
                case "u32" ->
                    numericRow(
                            vector,
                            Constraints.u32(),
                            new NumericDoor<>() {
                                public CheckedVerdict<Long> text(String t, NumFormat f, Constraints<Long> c) {
                                    return Cast.u32(t, f, c);
                                }

                                public CheckedVerdict<Long> bytes(byte[] t, NumFormat f, Constraints<Long> c) {
                                    return Cast.u32(t, f, c);
                                }

                                public CheckedVerdict<Long> segment(MemorySegment t, NumFormat f, Constraints<Long> c) {
                                    return Cast.u32(t, f, c);
                                }

                                public CheckedVerdict<Long> fromDouble(double v, Constraints<Long> c) {
                                    return Cast.u32FromDouble(v, c);
                                }
                            },
                            JsonElement::getAsLong);
                case "u64" ->
                    numericRow(
                            vector,
                            Constraints.u64(),
                            new NumericDoor<>() {
                                public CheckedVerdict<Long> text(String t, NumFormat f, Constraints<Long> c) {
                                    return Cast.u64(t, f, c);
                                }

                                public CheckedVerdict<Long> bytes(byte[] t, NumFormat f, Constraints<Long> c) {
                                    return Cast.u64(t, f, c);
                                }

                                public CheckedVerdict<Long> segment(MemorySegment t, NumFormat f, Constraints<Long> c) {
                                    return Cast.u64(t, f, c);
                                }

                                public CheckedVerdict<Long> fromDouble(double v, Constraints<Long> c) {
                                    return Cast.u64FromDouble(v, c);
                                }
                            },
                            v -> v.getAsBigInteger().longValue());
                case "f32" ->
                    numericRow(
                            vector,
                            Constraints.f32(),
                            new NumericDoor<>() {
                                public CheckedVerdict<Float> text(String t, NumFormat f, Constraints<Float> c) {
                                    return Cast.f32(t, f, c);
                                }

                                public CheckedVerdict<Float> bytes(byte[] t, NumFormat f, Constraints<Float> c) {
                                    return Cast.f32(t, f, c);
                                }

                                public CheckedVerdict<Float> segment(
                                        MemorySegment t, NumFormat f, Constraints<Float> c) {
                                    return Cast.f32(t, f, c);
                                }

                                public CheckedVerdict<Float> fromDouble(double v, Constraints<Float> c) {
                                    return Cast.f32FromDouble(v, c);
                                }
                            },
                            v -> (float) v.getAsDouble());
                case "f64" ->
                    numericRow(
                            vector,
                            Constraints.f64(),
                            new NumericDoor<>() {
                                public CheckedVerdict<Double> text(String t, NumFormat f, Constraints<Double> c) {
                                    return Cast.f64(t, f, c);
                                }

                                public CheckedVerdict<Double> bytes(byte[] t, NumFormat f, Constraints<Double> c) {
                                    return Cast.f64(t, f, c);
                                }

                                public CheckedVerdict<Double> segment(
                                        MemorySegment t, NumFormat f, Constraints<Double> c) {
                                    return Cast.f64(t, f, c);
                                }

                                public CheckedVerdict<Double> fromDouble(double v, Constraints<Double> c) {
                                    return Cast.f64FromDouble(v, c);
                                }
                            },
                            JsonElement::getAsDouble);
                case "decimal" ->
                    numericRow(
                            vector,
                            Constraints.decimal(),
                            new NumericDoor<>() {
                                public CheckedVerdict<BigDecimal> text(
                                        String t, NumFormat f, Constraints<BigDecimal> c) {
                                    return Cast.decimal(t, f, c);
                                }

                                public CheckedVerdict<BigDecimal> bytes(
                                        byte[] t, NumFormat f, Constraints<BigDecimal> c) {
                                    return Cast.decimal(t, f, c);
                                }

                                public CheckedVerdict<BigDecimal> segment(
                                        MemorySegment t, NumFormat f, Constraints<BigDecimal> c) {
                                    return Cast.decimal(t, f, c);
                                }

                                public CheckedVerdict<BigDecimal> fromDouble(double v, Constraints<BigDecimal> c) {
                                    return Cast.decimalFromDouble(v, c);
                                }
                            },
                            v -> new BigDecimal(v.getAsString()));
                case "text" -> textRow(vector);
                default -> throw new AssertionError("constraints: unknown door " + door);
            }
        }
        assertEquals(
                new java.util.TreeSet<>(java.util.List.of(
                        "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "decimal", "text",
                        "refused")),
                seen);
        assertEquals(331, rows);
    }

    /**
     * The declaration conversion ({@code corpus/declarations.json}): each input crosses the
     * builder's string path, as the {@code maximum} of a declaration at the decimal door, which
     * holds any decimal, and the triple the core converted is asserted exactly, scale included.
     * A {@code refused} row throws with code 8. The {@code input_bytes} rows are invalid UTF-8,
     * which no Java string carries (a lone surrogate, the nearest a string comes, is pinned in
     * {@link ConstraintsTest}), so they are counted and skipped here.
     */
    @Test
    void declarationsCorpus() {
        int accepted = 0;
        int refused = 0;
        int bytes = 0;
        for (JsonElement element : corpus("declarations.json")) {
            JsonObject vector = element.getAsJsonObject();
            if (vector.has("input_bytes")) {
                bytes++;
                continue;
            }
            String input = vector.get("input").getAsString();
            if (vector.get("expect").getAsString().equals("refused")) {
                assertEquals(8, vector.get("refusal").getAsInt(), input);
                IllegalArgumentException e = assertThrows(
                        IllegalArgumentException.class,
                        () -> Constraints.decimal().maximum(input).build(),
                        input);
                assertEquals(refusalMessage(8), e.getMessage(), input);
                refused++;
                continue;
            }
            BigDecimal maximum =
                    Constraints.decimal().maximum(input).build().maximum().orElseThrow();
            BigInteger magnitude = new BigInteger(vector.get("magnitude").getAsString());
            assertEquals(magnitude, maximum.unscaledValue().abs(), input + ": magnitude");
            assertEquals(vector.get("scale").getAsInt(), maximum.scale(), input + ": scale");
            assertEquals(vector.get("negative").getAsBoolean(), maximum.signum() < 0, input + ": sign");
            assertEquals(vector.get("value").getAsString(), maximum.toPlainString(), input + ": value");
            accepted++;
        }
        assertEquals(java.util.List.of(33, 30, 3), java.util.List.of(accepted, refused, bytes));
    }

    /**
     * The plain doors' verdict keeps exactly its shape: validation constraints are additive, so
     * {@link Verdict}'s two cases, {@link Success}'s and {@link Fault}'s components and
     * {@link CastFailure}'s constants are pinned, and no door without a {@link Constraints}
     * answers in anything but a {@link Verdict}. {@link Fault} joins {@link Rejection} as the
     * checked doors' parse failure, the very record the plain doors return.
     */
    @Test
    void thePlainVerdictKeepsItsShape() {
        java.util.function.Function<Class<?>, java.util.List<String>> components =
                type -> java.util.Arrays.stream(type.getRecordComponents())
                        .map(c -> c.getGenericType().getTypeName() + " " + c.getName())
                        .toList();
        assertEquals(
                java.util.List.of(Success.class, Fault.class),
                java.util.List.of(Verdict.class.getPermittedSubclasses()));
        assertEquals(java.util.List.of("T value"), components.apply(Success.class));
        assertEquals(
                java.util.List.of("io.github.skunkwerkx.hypercast.CastFailure reason", "int offset", "int length"),
                components.apply(Fault.class));
        assertEquals(java.util.List.of(Verdict.class), java.util.List.of(Success.class.getInterfaces()));
        assertEquals(java.util.List.of(Verdict.class, Rejection.class), java.util.List.of(Fault.class.getInterfaces()));
        assertEquals(
                java.util.List.of("EMPTY=1", "MALFORMED=2", "OUT_OF_RANGE=3"),
                java.util.Arrays.stream(CastFailure.values())
                        .map(r -> r.name() + "=" + r.code())
                        .toList());
        int plain = 0;
        for (java.lang.reflect.Method method : Cast.class.getMethods()) {
            if (method.getDeclaringClass() != Cast.class
                    || method.getName().equals("isAvailable")
                    || method.getName().equals("nativeVersion")
                    || method.getName().equals("optional")) {
                continue;
            }
            boolean checked =
                    java.util.Arrays.asList(method.getParameterTypes()).contains(Constraints.class);
            assertEquals(
                    checked ? CheckedVerdict.class : Verdict.class, method.getReturnType(), method.toGenericString());
            plain += checked ? 0 : 1;
        }
        // 1.0.0's 109 plain doors, plus the three text doors.
        assertEquals(112, plain);
    }
}
