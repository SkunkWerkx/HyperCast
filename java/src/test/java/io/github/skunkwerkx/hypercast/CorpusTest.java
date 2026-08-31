package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.fail;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
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
        throw new IllegalStateException("corpus directory not found above " + Path.of("").toAbsolutePath());
    }

    private static JsonArray corpus(String name) {
        try {
            return JsonParser.parseString(Files.readString(CORPUS.resolve(name))).getAsJsonArray();
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    private static byte[] inputBytes(JsonObject vector) {
        return vector.get("input").getAsString().getBytes(StandardCharsets.UTF_8);
    }

    private static NumFormat formatOf(JsonObject vector) {
        if (!vector.has("format")) {
            return NumFormat.INVARIANT;
        }
        JsonObject format = vector.getAsJsonObject("format");
        return new NumFormat(
                format.get("decimal_sep").getAsString().charAt(0),
                format.get("group_sep").getAsString().charAt(0),
                format.get("flags").getAsInt());
    }

    private static <T> void assertVerdict(String domain, JsonObject vector, Verdict<T> verdict, T expected) {
        String input = vector.get("input").getAsString();
        String expect = vector.get("expect").getAsString();
        // The union payoff in action: two case arms, no default, exhaustive over the sealed set.
        switch (verdict) {
            case Success<T> success -> {
                assertEquals("ok", expect, domain + ": '" + input + "' unexpectedly parsed");
                assertEquals(expected, success.value(), domain + ": '" + input + "'");
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
                case "i8" -> assertVerdict("integer", vector, Cast.i8(input, format),
                        value == null ? null : value.getAsByte());
                case "i16" -> assertVerdict("integer", vector, Cast.i16(input, format),
                        value == null ? null : value.getAsShort());
                case "i32" -> assertVerdict("integer", vector, Cast.i32(input, format),
                        value == null ? null : value.getAsInt());
                case "i64" -> assertVerdict("integer", vector, Cast.i64(input, format),
                        value == null ? null : value.getAsLong());
                case "u8" -> assertVerdict("integer", vector, Cast.u8(input, format),
                        value == null ? null : value.getAsInt());
                case "u16" -> assertVerdict("integer", vector, Cast.u16(input, format),
                        value == null ? null : value.getAsInt());
                case "u32" -> assertVerdict("integer", vector, Cast.u32(input, format),
                        value == null ? null : value.getAsLong());
                case "u64" -> assertVerdict("integer", vector, Cast.u64(input, format),
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
                case "f32" -> assertVerdict("real", vector, Cast.f32(input, format),
                        value == null ? null : (float) value.getAsDouble());
                case "f64" -> assertVerdict("real", vector, Cast.f64(input, format),
                        value == null ? null : value.getAsDouble());
                default -> fail("real: unknown type " + vector.get("type"));
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
                ? Instant.ofEpochSecond(vector.get("seconds").getAsLong(), vector.get("nanos").getAsLong())
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
                    ? Duration.ofSeconds(vector.get("seconds").getAsLong(), vector.get("nanos").getAsLong())
                    : null;
            assertVerdict("duration", vector, Cast.duration(inputBytes(vector)), expected);
        }
    }
}
