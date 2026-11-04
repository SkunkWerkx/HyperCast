package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
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
import java.util.Map;
import java.util.UUID;
import java.util.function.Supplier;
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
     * The grouping policy a format declares: {@code "grouping"} is {@code "legacy"},
     * {@code "western"} or {@code "indian"}, and absent means the policy bits the row's raw
     * {@code flags} carry, which for every older row is legacy: the core's and the C ABI's
     * meaning, not the constructors' new default.
     */
    private static Grouping groupingOf(JsonObject format) {
        if (format.has("grouping")) {
            return switch (format.get("grouping").getAsString()) {
                case "legacy" -> Grouping.LEGACY;
                case "western" -> Grouping.WESTERN;
                case "indian" -> Grouping.INDIAN;
                default -> throw new AssertionError("unknown grouping " + format.get("grouping"));
            };
        }
        return Grouping.of(format.get("flags").getAsInt() & Grouping.POLICY_MASK);
    }

    /**
     * The format a vector declares: the two separators and the flags always, every other
     * field only where the vector names it — absent is not declared, exactly as the core's
     * own replay reads it. The policy is {@link #groupingOf} (legacy for a row declaring none);
     * the styles are the flags without it, and a flag bit that is no style is the
     * constructor's refusal.
     */
    private static NumFormat formatOf(JsonObject vector) {
        if (!vector.has("format")) {
            return NumFormat.INVARIANT.withGrouping(Grouping.LEGACY);
        }
        JsonObject format = vector.getAsJsonObject("format");
        String digitZero = symbolOf(format, "digit_zero");
        Grouping grouping = groupingOf(format);
        int styles = format.get("flags").getAsInt() & ~Grouping.POLICY_MASK;
        return new NumFormat(
                        format.get("decimal_sep").getAsString(),
                        format.get("group_sep").getAsString(),
                        styles,
                        symbolOf(format, "currency"),
                        symbolOf(format, "currency_decimal_sep"),
                        symbolOf(format, "currency_group_sep"),
                        symbolOf(format, "minus_sign"),
                        symbolOf(format, "plus_sign"),
                        symbolOf(format, "percent"),
                        digitZero.isEmpty() ? 0 : digitZero.codePointAt(0))
                .withGrouping(grouping);
    }

    /** The detail vocabulary, restated: name to code, the spec's table. */
    private static final Map<String, Integer> DETAIL_CODES = detailCodes();

    private static Map<String, Integer> detailCodes() {
        String[] names = {
            "blank",
            "unexpected_character",
            "digit_expected",
            "delimiter_expected",
            "trailing_text",
            "no_digits",
            "sign_misplaced",
            "parentheses_unbalanced",
            "mark_repeated",
            "group_misplaced",
            "group_size",
            "group_in_fraction",
            "ambiguous_separator",
            "fraction_not_allowed",
            "field_width",
            "wrong_length",
            "component_missing",
            "not_recognized",
            "unknown_unit",
            "invalid_utf8",
            "too_long",
            "not_finite",
            "not_whole",
            "negative_not_allowed",
            "too_large",
            "too_small",
            "too_precise",
            "date_field_out_of_range",
            "time_field_out_of_range",
            "outside_window",
            "nonexistent_date",
            "not_a_scalar"
        };
        Map<String, Integer> codes = new java.util.LinkedHashMap<>();
        for (int i = 0; i < names.length; i++) {
            codes.put(names[i], i + 1);
        }
        return codes;
    }

    /** A failure row's detail, named and by its raw code; every failure row must name one. */
    private static void assertDetail(String label, JsonObject vector, FaultDetail detail) {
        assertTrue(vector.has("detail"), label + ": a failure row names its detail");
        String name = vector.get("detail").getAsString();
        assertTrue(DETAIL_CODES.containsKey(name), label + ": unknown detail " + name);
        assertEquals(name, detail.name(), label + ": detail");
        assertEquals(DETAIL_CODES.get(name), detail.code(), label + ": detail code");
        assertTrue(detail.isKnown(), label + ": a published detail is known");
    }

    /** The options a vector declares: flag names, a pivot, and a month vocabulary; or null. */
    private static DateOptions optionsOf(JsonObject vector) {
        if (!vector.has("options")) {
            return null;
        }
        JsonObject options = vector.getAsJsonObject("options");
        int flags = 0;
        if (options.has("flags")) {
            for (JsonElement name : options.getAsJsonArray("flags")) {
                flags |= switch (name.getAsString()) {
                    case "compact_date" -> DateOptions.COMPACT_DATE;
                    case "space_separator" -> DateOptions.SPACE_SEPARATOR;
                    case "hour_offset" -> DateOptions.HOUR_OFFSET;
                    case "common_forms" -> DateOptions.COMMON_FORMS.flags();
                    default -> throw new AssertionError("unknown option flag " + name);
                };
            }
        }
        return new DateOptions(
                flags, options.has("pivot") ? options.get("pivot").getAsInt() : 0);
    }

    /** The month vocabulary a vector declares, through the builder; or null. */
    private static MonthNames monthsOf(JsonObject vector) {
        if (!vector.has("options") || !vector.getAsJsonObject("options").has("months")) {
            return null;
        }
        MonthNames.Builder builder = MonthNames.builder();
        for (JsonElement entry : vector.getAsJsonObject("options").getAsJsonArray("months")) {
            builder.add(
                    entry.getAsJsonArray().get(0).getAsInt(),
                    entry.getAsJsonArray().get(1).getAsString());
        }
        return builder.build();
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

    private static CastFailure reasonOf(String expect, String label) {
        return switch (expect) {
            case "empty" -> CastFailure.EMPTY;
            case "malformed" -> CastFailure.MALFORMED;
            case "out_of_range" -> CastFailure.OUT_OF_RANGE;
            default -> fail(label + ": expected '" + expect + "' but faulted");
        };
    }

    /** The same row through the detailed path: value, reason and span as above, and the detail. */
    private static <T> void assertDetailed(String domain, JsonObject vector, DetailedVerdict<T> verdict, T expected) {
        String input = vector.get("input").getAsString();
        String expect = vector.get("expect").getAsString();
        String label = domain + ": '" + input + "'";
        switch (verdict) {
            case DetailedSuccess<T> success -> {
                assertEquals("ok", expect, label + " unexpectedly parsed (detailed)");
                assertSame(expected, success.value(), label + " (detailed)");
            }
            case DetailedFault<T> fault -> {
                assertEquals(reasonOf(expect, label), fault.reason(), label + " (detailed)");
                if (vector.has("fault")) {
                    JsonArray span = vector.getAsJsonArray("fault");
                    assertEquals(span.get(0).getAsInt(), fault.offset(), label + " detailed fault offset");
                    assertEquals(span.get(1).getAsInt(), fault.length(), label + " detailed fault length");
                }
                assertDetail(label, vector, fault.detail());
            }
        }
    }

    /**
     * One row through the plain door (unless it declares date options, which only the detailed
     * path takes) and through the detailed door, which must agree on everything and add the
     * detail.
     */
    private static <T> void both(
            String domain,
            JsonObject vector,
            Supplier<Verdict<T>> plain,
            Supplier<DetailedVerdict<T>> detailed,
            T expected) {
        if (!vector.has("options")) {
            assertVerdict(domain, vector, plain.get(), expected);
        }
        assertDetailed(domain, vector, detailed.get(), expected);
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
     * input — the one place a Java verdict departs from the core's, by design. The trim is
     * the test's own, by code point over the string and counted in UTF-8 bytes, against this
     * binding's restated {@link WhiteSpace} table.
     */
    @Test
    void charCorpus() {
        for (JsonElement element : corpus("char.json")) {
            JsonObject vector = element.getAsJsonObject().deepCopy();
            Character expected = null;
            if (vector.has("value")) {
                int scalar = vector.get("value").getAsInt();
                if (scalar > Character.MAX_VALUE) {
                    int[] token = trimmedToken(vector.get("input").getAsString());
                    vector.addProperty("expect", "out_of_range");
                    JsonArray span = new JsonArray();
                    span.add(token[0]);
                    span.add(token[1]);
                    vector.add("fault", span);
                    // One char cannot hold the scalar the core accepted: too large for the type.
                    vector.addProperty("detail", "too_large");
                } else {
                    expected = (char) scalar;
                }
            }
            byte[] input = inputBytes(vector);
            both("char", vector, () -> Cast.character(input), () -> DetailedCast.character(input), expected);
        }
    }

    /** The input less its white-space edges: the token's UTF-8 byte offset and length. */
    private static int[] trimmedToken(String input) {
        int[] points = input.codePoints().toArray();
        int first = 0;
        int last = points.length;
        while (first < last && WhiteSpace.is(points[first])) {
            first++;
        }
        while (last > first && WhiteSpace.is(points[last - 1])) {
            last--;
        }
        return new int[] {
            new String(points, 0, first).getBytes(StandardCharsets.UTF_8).length,
            new String(points, first, last - first).getBytes(StandardCharsets.UTF_8).length
        };
    }

    /** The message fragment this binding gives each rule of {@code formats.json}. */
    private static String ruleMessage(String rule) {
        return switch (rule) {
            case "length" -> "must be at most";
            case "digit" -> "must not contain an ASCII digit";
            case "equal_separators" -> "must differ";
            case "white_space" -> "must not contain white space";
            case "designator_edge", "currency_edge" -> "must not start or end with white space";
            case "grouping_policy" -> "names no grouping policy";
            case "unknown_flag", "flags" -> "undefined bits";
            case "pivot" -> "Pivot must be";
            case "layout" -> "month vocabulary is malformed";
            case "month" -> "must be 1 to 12";
            case "utf8" -> "must be valid UTF-8";
            case "name_length" -> "UTF-8 bytes";
            case "character" -> "must not contain an ASCII digit, '/', '-' or white space";
            case "month_missing" -> "needs at least one name";
            case "duplicate" -> "fold to the same spelling";
            default -> throw new AssertionError("formats: unknown rule " + rule);
        };
    }

    /** A vocabulary's entries as the core packs them: month byte, little-endian length, bytes. */
    private static byte[] packedMonths(JsonObject vector) {
        java.io.ByteArrayOutputStream packed = new java.io.ByteArrayOutputStream();
        for (JsonElement entry : vector.getAsJsonArray("months")) {
            byte[] name = entry.getAsJsonArray().get(1).getAsString().getBytes(StandardCharsets.UTF_8);
            packed.write(entry.getAsJsonArray().get(0).getAsInt());
            packed.writeBytes(java.nio.ByteBuffer.allocate(4)
                    .order(java.nio.ByteOrder.LITTLE_ENDIAN)
                    .putInt(name.length)
                    .array());
            packed.writeBytes(name);
        }
        if (vector.has("append_bytes")) {
            for (JsonElement b : vector.getAsJsonArray("append_bytes")) {
                packed.write(b.getAsInt());
            }
        }
        return packed.toByteArray();
    }

    /** The options a {@code date_options} row declares: flag names or raw bits, and a pivot. */
    private static void declareDateOptions(JsonObject vector) {
        int flags = 0;
        if (vector.has("flag_bits")) {
            flags = vector.get("flag_bits").getAsInt();
        } else {
            for (JsonElement name : vector.getAsJsonArray("flags")) {
                flags |= switch (name.getAsString()) {
                    case "compact_date" -> DateOptions.COMPACT_DATE;
                    case "space_separator" -> DateOptions.SPACE_SEPARATOR;
                    case "hour_offset" -> DateOptions.HOUR_OFFSET;
                    case "common_forms" -> DateOptions.COMMON_FORMS.flags();
                    default -> throw new AssertionError("unknown option flag " + name);
                };
            }
        }
        // A pivot past an int is as refused as one past 9999, never wrapped into range.
        long pivot = vector.has("pivot") ? vector.get("pivot").getAsLong() : 0;
        new DateOptions(flags, (int) Math.min(pivot, Integer.MAX_VALUE));
    }

    /**
     * A {@code month_names} row, built through {@link MonthNames#ofPacked} from the bytes the
     * row spells and, where it appends none, through the builder as well; both must refuse a
     * row with the one rule's message, and an accepted vocabulary packs back to its bytes.
     */
    private static void declareMonthNames(JsonObject vector) {
        byte[] packed = packedMonths(vector);
        MonthNames names = MonthNames.ofPacked(packed);
        assertArrayEquals(packed, names.packed());
        if (!vector.has("append_bytes")) {
            MonthNames.Builder builder = MonthNames.builder();
            for (JsonElement entry : vector.getAsJsonArray("months")) {
                builder.add(
                        entry.getAsJsonArray().get(0).getAsInt(),
                        entry.getAsJsonArray().get(1).getAsString());
            }
            assertEquals(names, builder.build());
        }
    }

    /**
     * {@code corpus/formats.json}: the table this binding restates holds to every
     * {@code white_space} row (and to no code point the corpus does not name true), and each
     * declaration row is built through the public constructors — a {@code refused} row throws
     * the message of the rule that wins, the ordering rows included.
     */
    @Test
    void formatsCorpus() {
        java.util.Set<Integer> expectedWhiteSpace = new java.util.TreeSet<>();
        java.util.Set<String> rules = new java.util.TreeSet<>();
        int[] rows = new int[4];
        JsonArray vectors = corpus("formats.json");
        for (JsonElement element : vectors) {
            JsonObject vector = element.getAsJsonObject();
            String kind = vector.get("kind").getAsString();
            if (kind.equals("white_space")) {
                String codePoint = vector.get("code_point").getAsString();
                int scalar = Integer.parseInt(codePoint.substring(2), 16);
                boolean expected = vector.get("white_space").getAsBoolean();
                assertEquals(expected, WhiteSpace.is(scalar), codePoint);
                if (expected) {
                    expectedWhiteSpace.add(scalar);
                }
                rows[0]++;
                continue;
            }
            Runnable declare = switch (kind) {
                case "num_format" -> () -> formatOf(vector);
                case "day_periods" ->
                    () -> new DateTimeFormat(DateOrder.YEAR_MONTH_DAY, symbolOf(vector, "am"), symbolOf(vector, "pm"));
                case "date_options" -> () -> declareDateOptions(vector);
                case "month_names" -> () -> declareMonthNames(vector);
                default -> throw new AssertionError("formats: unknown kind " + kind);
            };
            String label = kind + " " + vector;
            if (vector.get("expect").getAsString().equals("refused")) {
                String rule = vector.get("rule").getAsString();
                IllegalArgumentException refused = assertThrows(IllegalArgumentException.class, declare::run, label);
                assertTrue(refused.getMessage().contains(ruleMessage(rule)), label + ": " + refused.getMessage());
                rules.add(rule);
            } else {
                assertDoesNotThrow(declare::run, label);
            }
            rows[kind.equals("num_format") ? 1 : kind.equals("day_periods") ? 2 : 3]++;
        }
        for (int scalar = 0; scalar <= Character.MAX_CODE_POINT; scalar++) {
            assertEquals(
                    expectedWhiteSpace.contains(scalar), WhiteSpace.is(scalar), "U+" + Integer.toHexString(scalar));
        }
        assertEquals(25, expectedWhiteSpace.size());
        assertEquals(
                java.util.Set.of(
                        "length",
                        "digit",
                        "equal_separators",
                        "white_space",
                        "designator_edge",
                        "currency_edge",
                        "grouping_policy",
                        "unknown_flag",
                        "flags",
                        "pivot",
                        "layout",
                        "month",
                        "utf8",
                        "name_length",
                        "character",
                        "month_missing",
                        "duplicate"),
                rules);
        assertEquals(vectors.size(), rows[0] + rows[1] + rows[2] + rows[3]);
        assertTrue(rows[1] > 0 && rows[2] > 0 && rows[3] > 0, rows[1] + "/" + rows[2] + "/" + rows[3]);
    }

    @Test
    void booleanCorpus() {
        for (JsonElement element : corpus("boolean.json")) {
            JsonObject vector = element.getAsJsonObject();
            Boolean expected = vector.has("value") ? vector.get("value").getAsBoolean() : null;
            byte[] input = inputBytes(vector);
            both("boolean", vector, () -> Cast.bool(input), () -> DetailedCast.bool(input), expected);
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
                    both(
                            "integer",
                            vector,
                            () -> Cast.i8(input, format),
                            () -> DetailedCast.i8(input, format),
                            value == null ? null : value.getAsByte());
                case "i16" ->
                    both(
                            "integer",
                            vector,
                            () -> Cast.i16(input, format),
                            () -> DetailedCast.i16(input, format),
                            value == null ? null : value.getAsShort());
                case "i32" ->
                    both(
                            "integer",
                            vector,
                            () -> Cast.i32(input, format),
                            () -> DetailedCast.i32(input, format),
                            value == null ? null : value.getAsInt());
                case "i64" ->
                    both(
                            "integer",
                            vector,
                            () -> Cast.i64(input, format),
                            () -> DetailedCast.i64(input, format),
                            value == null ? null : value.getAsLong());
                case "u8" ->
                    both(
                            "integer",
                            vector,
                            () -> Cast.u8(input, format),
                            () -> DetailedCast.u8(input, format),
                            value == null ? null : value.getAsInt());
                case "u16" ->
                    both(
                            "integer",
                            vector,
                            () -> Cast.u16(input, format),
                            () -> DetailedCast.u16(input, format),
                            value == null ? null : value.getAsInt());
                case "u32" ->
                    both(
                            "integer",
                            vector,
                            () -> Cast.u32(input, format),
                            () -> DetailedCast.u32(input, format),
                            value == null ? null : value.getAsLong());
                case "u64" ->
                    both(
                            "integer",
                            vector,
                            () -> Cast.u64(input, format),
                            () -> DetailedCast.u64(input, format),
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
                    both(
                            "real",
                            vector,
                            () -> Cast.f32(input, format),
                            () -> DetailedCast.f32(input, format),
                            value == null ? null : (float) value.getAsDouble());
                case "f64" ->
                    both(
                            "real",
                            vector,
                            () -> Cast.f64(input, format),
                            () -> DetailedCast.f64(input, format),
                            value == null ? null : value.getAsDouble());
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
            byte[] bytes = inputBytes(vector);
            NumFormat format = formatOf(vector);
            Verdict<BigDecimal> verdict = Cast.decimal(bytes, format);
            both("decimal", vector, () -> verdict, () -> DetailedCast.decimal(bytes, format), expected);
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
            byte[] input = inputBytes(vector);
            UUID want = expected;
            both("uuid", vector, () -> Cast.uuid(input), () -> DetailedCast.uuid(input), want);
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
            byte[] input = inputBytes(vector);
            DateOptions options = optionsOf(vector);
            both(
                    "timestamp",
                    vector,
                    () -> Cast.timestamp(input),
                    () -> options == null ? DetailedCast.timestamp(input) : DetailedCast.timestamp(input, options),
                    expectedInstant(vector));
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
            byte[] input = inputBytes(vector);
            both(
                    "unix",
                    vector,
                    () -> Cast.unix(input, precision),
                    () -> DetailedCast.unix(input, precision),
                    expectedInstant(vector));
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
            byte[] input = inputBytes(vector);
            both(
                    "excel_serial",
                    vector,
                    () -> Cast.excelSerial(input, epoch),
                    () -> DetailedCast.excelSerial(input, epoch),
                    expectedInstant(vector));
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
            byte[] input = inputBytes(vector);
            DateOptions options = optionsOf(vector);
            both(
                    "date",
                    vector,
                    () -> Cast.date(input),
                    () -> options == null ? DetailedCast.date(input) : DetailedCast.date(input, options),
                    expected);
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
            byte[] input = inputBytes(vector);
            DateOptions options = optionsOf(vector);
            MonthNames months = monthsOf(vector);
            both(
                    "date_order",
                    vector,
                    () -> Cast.date(input, order),
                    () -> options == null
                            ? DetailedCast.date(input, order)
                            : months == null
                                    ? DetailedCast.date(input, order, options)
                                    : DetailedCast.date(input, order, options, months),
                    expected);
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
            DateOptions options = optionsOf(vector);
            MonthNames months = monthsOf(vector);
            if (format.englishOnly()) {
                both(
                        "datetime",
                        vector,
                        () -> Cast.dateTime(input, order),
                        () -> options == null
                                ? DetailedCast.dateTime(input, order)
                                : months == null
                                        ? DetailedCast.dateTime(input, order, options)
                                        : DetailedCast.dateTime(input, order, options, months),
                        expected);
                assertEquals(Cast.dateTime(input, order), Cast.dateTime(input, format));
                assertEquals(
                        DetailedCast.dateTime(input, order, DateOptions.NONE),
                        DetailedCast.dateTime(input, format, DateOptions.NONE));
            } else {
                both(
                        "datetime",
                        vector,
                        () -> Cast.dateTime(input, format),
                        () -> options == null
                                ? DetailedCast.dateTime(input, format)
                                : months == null
                                        ? DetailedCast.dateTime(input, format, options)
                                        : DetailedCast.dateTime(input, format, options, months),
                        expected);
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
            byte[] input = inputBytes(vector);
            both("time", vector, () -> Cast.time(input), () -> DetailedCast.time(input), expected);
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
            byte[] input = inputBytes(vector);
            both("duration", vector, () -> Cast.duration(input), () -> DetailedCast.duration(input), expected);
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
                    assertDetailed("typed decimal", vector, DetailedCast.decimalFromDouble(value), expected);
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
                    assertDetailed(
                            "typed excel_serial", vector, DetailedCast.excelSerialFromDouble(value, epoch), expected);
                    yield cast;
                }
                case "excel_time" -> {
                    LocalTime expected = vector.has("nanos")
                            ? LocalTime.ofNanoOfDay(vector.get("nanos").getAsLong())
                            : null;
                    Verdict<LocalTime> cast = Cast.excelTime(value);
                    assertVerdict("typed excel_time", vector, cast, expected);
                    assertDetailed("typed excel_time", vector, DetailedCast.excelTime(value), expected);
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
                    assertDetailed("typed excel_duration", vector, DetailedCast.excelDuration(value), expected);
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

    /**
     * A detailed checked door by name: the typed methods of {@link DetailedCast} are the same
     * eleven shapes four times over, so the replay finds them by reflection, which also fails the
     * row if the door is missing.
     */
    @SuppressWarnings("unchecked")
    private static <T> DetailedCheckedVerdict<T> detailed(String name, Class<?>[] leading, Object... arguments) {
        Class<?>[] types = java.util.Arrays.copyOf(leading, leading.length + 1);
        types[leading.length] = Constraints.class;
        try {
            return (DetailedCheckedVerdict<T>)
                    DetailedCast.class.getMethod(name, types).invoke(null, arguments);
        } catch (ReflectiveOperationException e) {
            throw new AssertionError("DetailedCast." + name + java.util.Arrays.toString(types), e);
        }
    }

    /**
     * A detailed checked verdict as the checked verdict it projects to, which {@link
     * #assertChecked} judges, plus the detail of a parse failure. A violation names none.
     */
    private static <T> void assertDetailedChecked(
            String label,
            JsonObject vector,
            DetailedCheckedVerdict<T> verdict,
            java.util.function.IntUnaryOperator units,
            java.util.function.Function<JsonElement, T> expected) {
        CheckedVerdict<T> plain = switch (verdict) {
            case DetailedChecked<T> ok -> new Checked<>(ok.value(), ok.adjusted(), ok.enumIndex());
            case DetailedFault<T> fault -> fault.toFault();
            case DetailedViolation<T> violation -> violation.toViolation();
        };
        assertChecked(label + " (detailed)", vector, plain, units, expected);
        if (verdict instanceof DetailedFault<T> fault) {
            assertDetail(label, vector, fault.detail());
        }
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
        String name = vector.get("door").getAsString();
        if (vector.has("from_f64")) {
            double value = fromF64(vector.get("from_f64"));
            assertChecked(label, vector, door.fromDouble(value, constraints), u -> u, expected);
            assertDetailedChecked(
                    label,
                    vector,
                    detailed(name + "FromDouble", new Class<?>[] {double.class}, value, constraints),
                    u -> u,
                    expected);
            return;
        }
        String text = vector.get("input").getAsString();
        byte[] bytes = text.getBytes(StandardCharsets.UTF_8);
        NumFormat format = formatOf(vector);
        assertChecked(label, vector, door.bytes(bytes, format, constraints), u -> u, expected);
        assertDetailedChecked(
                label,
                vector,
                detailed(name, new Class<?>[] {byte[].class, NumFormat.class}, bytes, format, constraints),
                u -> u,
                expected);
        assertDetailedChecked(
                label + " (segment)",
                vector,
                detailed(
                        name,
                        new Class<?>[] {MemorySegment.class, NumFormat.class},
                        MemorySegment.ofArray(bytes),
                        format,
                        constraints),
                u -> u,
                expected);
        assertDetailedChecked(
                label + " (string)",
                vector,
                detailed(name, new Class<?>[] {String.class, NumFormat.class}, text, format, constraints),
                u -> chars(bytes, u),
                expected);
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
            assertDetailedChecked(label, vector, DetailedCast.text(invalid, constraints), u -> u, value);
            assertDetailedChecked(
                    label + " (segment)",
                    vector,
                    DetailedCast.text(MemorySegment.ofArray(invalid), constraints),
                    u -> u,
                    value);
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
        assertDetailedChecked(label, vector, DetailedCast.text(bytes, constraints), u -> u, value);
        assertDetailedChecked(
                label + " (segment)",
                vector,
                DetailedCast.text(MemorySegment.ofArray(bytes), constraints),
                u -> u,
                value);
        assertDetailedChecked(
                label + " (string)", vector, DetailedCast.text(text, constraints), u -> chars(bytes, u), value);
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
        JsonArray vectors = corpus("constraints.json");
        for (JsonElement element : vectors) {
            JsonObject vector = element.getAsJsonObject();
            String door = vector.get("door").getAsString();
            seen.add(vector.get("expect").getAsString().equals("refused") ? "refused" : door);
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
            rows++;
        }
        assertEquals(
                new java.util.TreeSet<>(java.util.List.of(
                        "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "decimal", "text",
                        "refused")),
                seen);
        // Every row the file holds was replayed to the end: the count comes from the file.
        assertTrue(rows > 0, "constraints.json holds no rows");
        assertEquals(vectors.size(), rows);
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
        JsonArray vectors = corpus("declarations.json");
        for (JsonElement element : vectors) {
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
        // Every row the file holds, counted from the file, and each kind present.
        assertEquals(vectors.size(), accepted + refused + bytes);
        assertTrue(accepted > 0 && refused > 0 && bytes > 0, accepted + "/" + refused + "/" + bytes);
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
