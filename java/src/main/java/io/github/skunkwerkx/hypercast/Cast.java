package io.github.skunkwerkx.hypercast;

import io.github.skunkwerkx.hypercast.interop.NativePlatform;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.Locale;
import java.util.Objects;
import java.util.UUID;
import java.util.function.IntUnaryOperator;

/**
 * Allocation-lean scalar casts — booleans, numerics, UUIDs, temporals — calling directly
 * into the native core in-process: through JNI on JDK 21 to 24, and through
 * {@code java.lang.foreign} (FFM) downcalls on JDK 25 and later, from one multi-release jar.
 * Every door returns a {@link Verdict}: the value, or a {@link Fault} with a closed reason and
 * the offending byte span. Never throws on bad input — the only exceptions here are caller
 * bugs (a {@code null} argument, a malformed {@link NumFormat} or {@link DateTimeFormat}),
 * never data.
 *
 * <p>Door names mirror the native ABI ({@code i32}, {@code f64}, {@code timestamp}, …) so
 * the polyglot surface reads identically across bindings. Each door takes a {@link String}
 * (UTF-8-encoded into a fresh {@code byte[]}), raw UTF-8 {@code byte[]} — the native
 * contract, and the form whose {@link Fault} offsets need no mapping — or a
 * {@link MemorySegment} view of UTF-8 bytes, heap or native, for a caller that already
 * holds one buffer of many values (a mapped file, a direct buffer, one line of a CSV) and
 * wants to cast a slice of it without copying it out first. {@code java.lang.foreign} is a
 * preview API on JDK 21, so the {@link MemorySegment} doors are there for code compiled with
 * preview enabled; on the JNI path a segment's bytes are copied to the heap first.
 *
 * <p>Temporal doors come out at {@code java.time}'s full fidelity: {@link Instant},
 * {@link LocalTime}, and {@link Duration} all carry nanoseconds, so unlike the C# binding
 * (ticks) nothing truncates — the JVM is the one platform that keeps every digit the core
 * parses.
 *
 * <p>The native libraries ride inside the jar under {@code /native/{rid}/} and are picked by
 * platform at runtime (see {@link NativePlatform}): the core, which JDK 25 and later call
 * through FFM, and the JNI library built from it, which JDK 21 to 24 load. On JDK 25 the
 * input crosses without a copy: every downcall is linked
 * {@link java.lang.foreign.Linker.Option#critical(boolean) critical(true)}, so the caller's
 * own heap array or segment is handed to the native side directly. On the JNI path a
 * {@code byte[]} of up to 64 KiB is pinned for the core call alone, and a longer one is
 * copied into native memory, so a parse never holds off the garbage collector for long.
 * What remains per call is the verdict record, the boxed value it carries, and (for the
 * {@link String} doors) the UTF-8 encode; the Rust core itself never allocates.
 *
 * <p>Nothing loads until the first door is called. A consumer with a managed fallback gates
 * on {@link #isAvailable()} first — the one probe that never throws — because the doors do
 * not fall back: a core that failed to load is thrown from every door as the failure it was.
 */
public final class Cast {
    private Cast() {}

    /**
     * Whether the core resolved: the bundled platform library loaded, and every export this
     * binding was built against was found in it. Probed once, on first call, and cached;
     * never throws. A library that will not open, a jar built without a core for this
     * platform, an export missing from an older core — all come back {@code false}. This is
     * what a consumer with a managed fallback gates on before the first door: the doors
     * themselves throw the load failure they hit rather than fall back, because a door that
     * quietly answered from somewhere else would be a verdict lie. {@code true} exactly when
     * {@link #nativeVersion()} succeeds.
     *
     * @return {@code true} when the core is loaded and callable
     */
    public static boolean isAvailable() {
        return Availability.AVAILABLE;
    }

    // Its own holder so the answer is computed once and cached without Cast's own init
    // depending on the load. The probe is the version export: the cheapest crossing there
    // is, and reaching it proves the core initialized — every export resolved.
    private static final class Availability {
        static final boolean AVAILABLE = probe();

        private Availability() {}

        private static boolean probe() {
            try {
                nativeVersion();
                return true;
            } catch (RuntimeException | LinkageError unavailable) {
                // The core failing to initialize surfaces as ExceptionInInitializerError (and
                // NoClassDefFoundError on every later touch) — LinkageErrors, whatever was
                // underneath: the loader's own IllegalStateException, an unopenable
                // library's IllegalArgumentException or UnsatisfiedLinkError, a missing
                // export's NoSuchElementException. Nothing else is expected, and nothing else
                // is swallowed.
                return false;
            }
        }
    }

    /**
     * The version of the core this process actually loaded — the bundled platform library —
     * as {@code major.minor.patch}, decoded from the core's own
     * {@code hypercast_version} export. The probe a host uses to prove the library it
     * resolved is the one this binding was built against, before making the first cast.
     * Takes nothing and touches nothing; the only way it fails is the core not having
     * loaded, which it reports as the load failure itself.
     *
     * @return the loaded core's version as {@code "major.minor.patch"}
     */
    public static String nativeVersion() {
        int packed = NativeCore.coreVersion();
        return (packed >>> 16) + "." + ((packed >>> 8) & 0xFF) + "." + (packed & 0xFF);
    }

    /** Builds a door's value out of the scratch its successful call wrote. */
    private interface Reader<T> {
        T read(NativeCore.Scratch scratch);
    }

    private static <T> Verdict<T> verdict(int code, NativeCore.Scratch scratch, Reader<T> reader) {
        return code == 0 ? new Success<>(reader.read(scratch)) : failed(code, scratch);
    }

    private static <T> Verdict<T> failed(int code, NativeCore.Scratch scratch) {
        if (code == -1) {
            throw new IllegalStateException(
                    "libhypercast reported a contract violation — a binding bug, please report it");
        }
        CastFailure reason = CastFailure.fromCode(code)
                .orElseThrow(() -> new IllegalStateException(
                        code + " is not a verdict reason code — a binding bug, please report it"));
        return new Fault<>(reason, NativeCore.faultOffset(scratch), NativeCore.faultLength(scratch));
    }

    // The door shapes, each over both input forms. A null argument is refused here, named,
    // before anything crosses, so both paths answer it alike.
    private static <T> Verdict<T> plain(int door, byte[] utf8, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.plain(scratch, door, utf8), scratch, reader);
    }

    private static <T> Verdict<T> plain(int door, MemorySegment utf8, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.plain(scratch, door, utf8), scratch, reader);
    }

    private static <T> Verdict<T> numeric(int door, byte[] utf8, NumFormat format, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        Objects.requireNonNull(format, "format");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.numeric(scratch, door, utf8, format), scratch, reader);
    }

    private static <T> Verdict<T> numeric(int door, MemorySegment utf8, NumFormat format, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        Objects.requireNonNull(format, "format");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.numeric(scratch, door, utf8, format), scratch, reader);
    }

    private static <T> Verdict<T> declared(int door, byte[] utf8, int code, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.declared(scratch, door, utf8, code), scratch, reader);
    }

    private static <T> Verdict<T> declared(int door, MemorySegment utf8, int code, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.declared(scratch, door, utf8, code), scratch, reader);
    }

    // `format` is null for the DateOrder overloads: no designators, a null pointer to the core.
    private static Verdict<LocalDateTime> civil(byte[] utf8, DateOrder order, DateTimeFormat format) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.civil(scratch, utf8, order.code(), format), scratch, NativeCore::dateTime);
    }

    private static Verdict<LocalDateTime> civil(MemorySegment utf8, DateOrder order, DateTimeFormat format) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.civil(scratch, utf8, order.code(), format), scratch, NativeCore::dateTime);
    }

    private static <T> Verdict<T> typed(int door, double value, Reader<T> reader) {
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.typed(scratch, door, value), scratch, reader);
    }

    private static byte[] utf8(String text) {
        return Objects.requireNonNull(text, "text").getBytes(StandardCharsets.UTF_8);
    }

    // A fault's span is byte offsets into the UTF-8 the core saw. For a String door that is
    // the wrong unit whenever the encoding came out longer than the String — non-ASCII input
    // — so the span is rebased to UTF-16 code units, and text.substring(offset, offset +
    // length) names the same characters the core faulted at. Length equality is the whole
    // ASCII check; the walk below runs only on a non-ASCII failure.
    private static <T> Verdict<T> chars(Verdict<T> verdict, String text, byte[] utf8) {
        if (utf8.length == text.length() || !(verdict instanceof Fault<T> fault)) {
            return verdict;
        }
        int start = charIndex(utf8, fault.offset());
        int end = charIndex(utf8, fault.offset() + fault.length());
        return new Fault<>(fault.reason(), start, end - start);
    }

    // How many UTF-16 code units the first byteOffset UTF-8 bytes decode to: one per ASCII
    // or lead byte, two for the lead of a four-byte sequence (a supplementary code point is
    // a surrogate pair), none for a continuation byte.
    private static int charIndex(byte[] utf8, int byteOffset) {
        int units = 0;
        for (int i = 0, n = Math.min(byteOffset, utf8.length); i < n; i++) {
            int b = utf8[i] & 0xFF;
            if ((b & 0xC0) != 0x80) {
                units += b >= 0xF0 ? 2 : 1;
            }
        }
        return units;
    }

    /**
     * Presents a verdict optionally: an {@link CastFailure#EMPTY} fault becomes absent
     * ({@link java.util.Optional#empty()}); every other outcome flows through untouched —
     * each binding maps absence to its platform's own idiom, and Java's is {@code Optional}.
     *
     * @param <T> the verdict's value type
     * @param verdict the verdict to present
     * @return empty for an EMPTY fault; the untouched verdict otherwise
     */
    public static <T> java.util.Optional<Verdict<T>> optional(Verdict<T> verdict) {
        Objects.requireNonNull(verdict, "verdict");
        return verdict instanceof Fault<T> fault && fault.reason() == CastFailure.EMPTY
                ? java.util.Optional.empty()
                : java.util.Optional.of(verdict);
    }

    // --- boolean ---

    /**
     * Casts boolean text: {@code true}/{@code false} plus the numeric and natural-language
     * conventions untrusted sources actually send ({@code t}/{@code f}, {@code yes}/{@code no},
     * {@code y}/{@code n}, {@code 1}/{@code 0}, {@code on}/{@code off},
     * {@code enabled}/{@code disabled}, {@code active}/{@code inactive},
     * {@code checked}/{@code unchecked}, {@code in}/{@code out}), ASCII case-insensitive.
     * Culture-insensitive — no {@link NumFormat}.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Boolean> bool(String text) {
        byte[] utf8 = utf8(text);
        return chars(bool(utf8), text, utf8);
    }

    /**
     * See {@link #bool(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Boolean> bool(byte[] utf8) {
        return plain(Door.BOOL, utf8, NativeCore::bool);
    }

    /**
     * See {@link #bool(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Boolean> bool(MemorySegment utf8) {
        return plain(Door.BOOL, utf8, NativeCore::bool);
    }

    // --- char ---

    /**
     * Casts char text: one character, either verbatim — the input is exactly one character,
     * read <em>before</em> trimming, so {@code " "} is a space and {@code "6"} is the digit
     * six — or a declared code point, ASCII-whitespace-trimmed and ASCII case-insensitive on
     * the prefix: decimal {@code 65}, {@code U+0041}, {@code 0x41}, {@code &H41}, or an HTML
     * numeric entity {@code &#65;} / {@code &#x41;} (the closing {@code ;} required).
     * Culture-insensitive — no {@link NumFormat}.
     *
     * <p>A code point past {@code U+10FFFF} or in the surrogate range is
     * {@link CastFailure#OUT_OF_RANGE}, as is any scalar above {@code U+FFFF}: a supplementary
     * character is a surrogate pair, not one {@code char}, so it faults over the trimmed input
     * rather than being split. A {@code String} of exactly one UTF-16 code unit is that
     * {@code char} as-is, a lone surrogate included, without crossing into the core.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Character> character(String text) {
        if (Objects.requireNonNull(text, "text").length() == 1) {
            return new Success<>(text.charAt(0));
        }
        byte[] utf8 = utf8(text);
        return chars(character(utf8), text, utf8);
    }

    /**
     * See {@link #character(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Character> character(byte[] utf8) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return character(NativeCore.plain(scratch, Door.CHAR, utf8), scratch, utf8.length, i -> utf8[i]);
    }

    /**
     * See {@link #character(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Character> character(MemorySegment utf8) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return character(
                NativeCore.plain(scratch, Door.CHAR, utf8),
                scratch,
                (int) utf8.byteSize(),
                i -> utf8.get(ValueLayout.JAVA_BYTE, i));
    }

    // `byteAt` reads the input the core saw, for the one verdict built here rather than there.
    private static Verdict<Character> character(
            int code, NativeCore.Scratch scratch, int len, IntUnaryOperator byteAt) {
        if (code != 0) {
            return failed(code, scratch);
        }
        int scalar = NativeCore.scalar(scratch);
        if (scalar <= Character.MAX_VALUE) {
            return new Success<>((char) scalar);
        }
        // A supplementary scalar the core accepted but one char cannot hold: out of range
        // over the token the core read, the input less its ASCII whitespace edges.
        int start = 0;
        int end = len;
        while (start < end && asciiWhitespace(byteAt.applyAsInt(start))) {
            start++;
        }
        while (end > start && asciiWhitespace(byteAt.applyAsInt(end - 1))) {
            end--;
        }
        return new Fault<>(CastFailure.OUT_OF_RANGE, start, end - start);
    }

    // The core's trim set: Rust's u8::is_ascii_whitespace (no vertical tab).
    private static boolean asciiWhitespace(int b) {
        return b == ' ' || b == '\t' || b == '\n' || b == '\f' || b == '\r';
    }

    // --- integers ---

    /**
     * Casts integer text to a signed 8-bit value under the declared format: the type's own
     * range, declared grouping, accounting parentheses, non-negative exponent ({@code 1e3}
     * is 1000; a decimal point is never accepted), and {@code 0x}/{@code &H}/{@code 0b}
     * two's-complement radix prefixes ({@code 0xFF} is -1).
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Byte> i8(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(i8(utf8, format), text, utf8);
    }

    /**
     * See {@link #i8(String, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Byte> i8(String text, Locale locale) {
        return i8(text, NumFormat.from(locale));
    }

    /**
     * See {@link #i8(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Byte> i8(byte[] utf8, NumFormat format) {
        return numeric(Door.I8, utf8, format, NativeCore::i8);
    }

    /**
     * See {@link #i8(byte[], NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Byte> i8(byte[] utf8, Locale locale) {
        return i8(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #i8(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Byte> i8(MemorySegment utf8, NumFormat format) {
        return numeric(Door.I8, utf8, format, NativeCore::i8);
    }

    /**
     * See {@link #i8(MemorySegment, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Byte> i8(MemorySegment utf8, Locale locale) {
        return i8(utf8, NumFormat.from(locale));
    }

    /**
     * Casts integer text to a signed 16-bit value. Notation rules as {@link #i8(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Short> i16(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(i16(utf8, format), text, utf8);
    }

    /**
     * See {@link #i16(String, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Short> i16(String text, Locale locale) {
        return i16(text, NumFormat.from(locale));
    }

    /**
     * See {@link #i16(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Short> i16(byte[] utf8, NumFormat format) {
        return numeric(Door.I16, utf8, format, NativeCore::i16);
    }

    /**
     * See {@link #i16(byte[], NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Short> i16(byte[] utf8, Locale locale) {
        return i16(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #i16(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Short> i16(MemorySegment utf8, NumFormat format) {
        return numeric(Door.I16, utf8, format, NativeCore::i16);
    }

    /**
     * See {@link #i16(MemorySegment, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Short> i16(MemorySegment utf8, Locale locale) {
        return i16(utf8, NumFormat.from(locale));
    }

    /**
     * Casts integer text to a signed 32-bit value. Notation rules as {@link #i8(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> i32(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(i32(utf8, format), text, utf8);
    }

    /**
     * See {@link #i32(String, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> i32(String text, Locale locale) {
        return i32(text, NumFormat.from(locale));
    }

    /**
     * See {@link #i32(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> i32(byte[] utf8, NumFormat format) {
        return numeric(Door.I32, utf8, format, NativeCore::i32);
    }

    /**
     * See {@link #i32(byte[], NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> i32(byte[] utf8, Locale locale) {
        return i32(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #i32(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> i32(MemorySegment utf8, NumFormat format) {
        return numeric(Door.I32, utf8, format, NativeCore::i32);
    }

    /**
     * See {@link #i32(MemorySegment, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> i32(MemorySegment utf8, Locale locale) {
        return i32(utf8, NumFormat.from(locale));
    }

    /**
     * Casts integer text to a signed 64-bit value. Notation rules as {@link #i8(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> i64(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(i64(utf8, format), text, utf8);
    }

    /**
     * See {@link #i64(String, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> i64(String text, Locale locale) {
        return i64(text, NumFormat.from(locale));
    }

    /**
     * See {@link #i64(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> i64(byte[] utf8, NumFormat format) {
        return numeric(Door.I64, utf8, format, NativeCore::i64);
    }

    /**
     * See {@link #i64(byte[], NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> i64(byte[] utf8, Locale locale) {
        return i64(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #i64(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> i64(MemorySegment utf8, NumFormat format) {
        return numeric(Door.I64, utf8, format, NativeCore::i64);
    }

    /**
     * See {@link #i64(MemorySegment, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> i64(MemorySegment utf8, Locale locale) {
        return i64(utf8, NumFormat.from(locale));
    }

    /**
     * Casts integer text to an unsigned 8-bit value, widened to {@code int} ({@code 0..255})
     * — Java has no unsigned primitives. Notation rules as {@link #i8(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u8(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(u8(utf8, format), text, utf8);
    }

    /**
     * See {@link #u8(String, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> u8(String text, Locale locale) {
        return u8(text, NumFormat.from(locale));
    }

    /**
     * See {@link #u8(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u8(byte[] utf8, NumFormat format) {
        return numeric(Door.U8, utf8, format, NativeCore::u8);
    }

    /**
     * See {@link #u8(byte[], NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> u8(byte[] utf8, Locale locale) {
        return u8(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #u8(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u8(MemorySegment utf8, NumFormat format) {
        return numeric(Door.U8, utf8, format, NativeCore::u8);
    }

    /**
     * See {@link #u8(MemorySegment, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> u8(MemorySegment utf8, Locale locale) {
        return u8(utf8, NumFormat.from(locale));
    }

    /**
     * Casts integer text to an unsigned 16-bit value, widened to {@code int} ({@code 0..65535}).
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u16(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(u16(utf8, format), text, utf8);
    }

    /**
     * See {@link #u16(String, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> u16(String text, Locale locale) {
        return u16(text, NumFormat.from(locale));
    }

    /**
     * See {@link #u16(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u16(byte[] utf8, NumFormat format) {
        return numeric(Door.U16, utf8, format, NativeCore::u16);
    }

    /**
     * See {@link #u16(byte[], NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> u16(byte[] utf8, Locale locale) {
        return u16(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #u16(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Integer> u16(MemorySegment utf8, NumFormat format) {
        return numeric(Door.U16, utf8, format, NativeCore::u16);
    }

    /**
     * See {@link #u16(MemorySegment, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Integer> u16(MemorySegment utf8, Locale locale) {
        return u16(utf8, NumFormat.from(locale));
    }

    /**
     * Casts integer text to an unsigned 32-bit value, widened to {@code long}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u32(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(u32(utf8, format), text, utf8);
    }

    /**
     * See {@link #u32(String, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> u32(String text, Locale locale) {
        return u32(text, NumFormat.from(locale));
    }

    /**
     * See {@link #u32(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u32(byte[] utf8, NumFormat format) {
        return numeric(Door.U32, utf8, format, NativeCore::u32);
    }

    /**
     * See {@link #u32(byte[], NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> u32(byte[] utf8, Locale locale) {
        return u32(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #u32(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u32(MemorySegment utf8, NumFormat format) {
        return numeric(Door.U32, utf8, format, NativeCore::u32);
    }

    /**
     * See {@link #u32(MemorySegment, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> u32(MemorySegment utf8, Locale locale) {
        return u32(utf8, NumFormat.from(locale));
    }

    /**
     * Casts integer text to an unsigned 64-bit value, carried as {@code long}'s
     * two's-complement bit pattern — render with {@link Long#toUnsignedString(long)} and
     * compare with {@link Long#compareUnsigned(long, long)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u64(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(u64(utf8, format), text, utf8);
    }

    /**
     * See {@link #u64(String, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> u64(String text, Locale locale) {
        return u64(text, NumFormat.from(locale));
    }

    /**
     * See {@link #u64(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u64(byte[] utf8, NumFormat format) {
        return numeric(Door.U64, utf8, format, NativeCore::u64);
    }

    /**
     * See {@link #u64(byte[], NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> u64(byte[] utf8, Locale locale) {
        return u64(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #u64(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Long> u64(MemorySegment utf8, NumFormat format) {
        return numeric(Door.U64, utf8, format, NativeCore::u64);
    }

    /**
     * See {@link #u64(MemorySegment, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Long> u64(MemorySegment utf8, Locale locale) {
        return u64(utf8, NumFormat.from(locale));
    }

    // --- reals ---

    /**
     * Casts real text to {@code float} under the declared format: finite values only
     * ({@code NaN}/{@code Infinity} literals are {@link CastFailure#MALFORMED}, overflow to
     * infinity is {@link CastFailure#OUT_OF_RANGE}), declared separators and grouping,
     * accounting parentheses, exponent, and trailing percent ({@code 50%} is 0.5).
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Float> f32(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(f32(utf8, format), text, utf8);
    }

    /**
     * See {@link #f32(String, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Float> f32(String text, Locale locale) {
        return f32(text, NumFormat.from(locale));
    }

    /**
     * See {@link #f32(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Float> f32(byte[] utf8, NumFormat format) {
        return numeric(Door.F32, utf8, format, NativeCore::f32);
    }

    /**
     * See {@link #f32(byte[], NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Float> f32(byte[] utf8, Locale locale) {
        return f32(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #f32(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Float> f32(MemorySegment utf8, NumFormat format) {
        return numeric(Door.F32, utf8, format, NativeCore::f32);
    }

    /**
     * See {@link #f32(MemorySegment, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Float> f32(MemorySegment utf8, Locale locale) {
        return f32(utf8, NumFormat.from(locale));
    }

    /**
     * Casts real text to {@code double}. Notation rules as {@link #f32(String, NumFormat)}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Double> f64(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(f64(utf8, format), text, utf8);
    }

    /**
     * See {@link #f64(String, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Double> f64(String text, Locale locale) {
        return f64(text, NumFormat.from(locale));
    }

    /**
     * See {@link #f64(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Double> f64(byte[] utf8, NumFormat format) {
        return numeric(Door.F64, utf8, format, NativeCore::f64);
    }

    /**
     * See {@link #f64(byte[], NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Double> f64(byte[] utf8, Locale locale) {
        return f64(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #f64(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Double> f64(MemorySegment utf8, NumFormat format) {
        return numeric(Door.F64, utf8, format, NativeCore::f64);
    }

    /**
     * See {@link #f64(MemorySegment, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<Double> f64(MemorySegment utf8, Locale locale) {
        return f64(utf8, NumFormat.from(locale));
    }

    // --- decimal ---

    /**
     * Casts decimal text to a {@link BigDecimal}, exactly and canonically: no {@code double}
     * is ever formed on the way, so {@code 0.1} is one tenth and {@code 50%} is exactly
     * {@code 0.5}; exact trailing zeros in the fraction are trimmed, so the scale is minimal
     * ({@code 1.10}, {@code 1.1} and {@code 1.1000} all come out with a scale of 1, and
     * {@code 100} stays {@code 100} — integer zeros are never touched). Zero is scale 0 and
     * never negative. Notation rules as {@link #f32(String, NumFormat)}. The door never
     * rounds — nothing but a zero is ever dropped, so a magnitude past
     * 2<sup>96</sup>&minus;1, or more fractional precision than 28 places can hold once exact
     * trailing zeros are shed, is {@link CastFailure#OUT_OF_RANGE}, not a silent
     * approximation.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<BigDecimal> decimal(String text, NumFormat format) {
        byte[] utf8 = utf8(text);
        return chars(decimal(utf8, format), text, utf8);
    }

    /**
     * See {@link #decimal(String, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<BigDecimal> decimal(String text, Locale locale) {
        return decimal(text, NumFormat.from(locale));
    }

    /**
     * See {@link #decimal(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<BigDecimal> decimal(byte[] utf8, NumFormat format) {
        return numeric(Door.DECIMAL, utf8, format, NativeCore::decimal);
    }

    /**
     * See {@link #decimal(byte[], NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<BigDecimal> decimal(byte[] utf8, Locale locale) {
        return decimal(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #decimal(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8
     * bytes — heap or native — crossing without a copy. Slice a larger buffer to cast one
     * value out of it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<BigDecimal> decimal(MemorySegment utf8, NumFormat format) {
        return numeric(Door.DECIMAL, utf8, format, NativeCore::decimal);
    }

    /**
     * See {@link #decimal(MemorySegment, NumFormat)}, with the notation the locale's — separators, signs,
     * percent and currency symbols, native digits, every default lenience on — as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<BigDecimal> decimal(MemorySegment utf8, Locale locale) {
        return decimal(utf8, NumFormat.from(locale));
    }

    // --- uuid ---

    /**
     * Casts UUID text to a {@link UUID}: every format .NET's {@code Guid} accepts (D, N, B,
     * P, X), after stripping a case-insensitive {@code urn:uuid:}/{@code GUID:}/{@code UUID:}
     * prefix. Culture-insensitive.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<UUID> uuid(String text) {
        byte[] utf8 = utf8(text);
        return chars(uuid(utf8), text, utf8);
    }

    /**
     * See {@link #uuid(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<UUID> uuid(byte[] utf8) {
        return plain(Door.UUID, utf8, NativeCore::uuid);
    }

    /**
     * See {@link #uuid(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<UUID> uuid(MemorySegment utf8) {
        return plain(Door.UUID, utf8, NativeCore::uuid);
    }

    // --- temporals ---

    /**
     * Casts an RFC 3339 instant — {@code yyyy-MM-ddTHH:mm:ss[.f+](Z|±hh:mm)}, zone
     * <b>mandatory</b> — to an {@link Instant}, normalized to UTC at full nanosecond
     * fidelity. A zone-less or space-separated form is {@link CastFailure#MALFORMED}; an
     * instant outside 0001-01-01 to 9999-12-31 UTC is {@link CastFailure#OUT_OF_RANGE}.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> timestamp(String text) {
        byte[] utf8 = utf8(text);
        return chars(timestamp(utf8), text, utf8);
    }

    /**
     * See {@link #timestamp(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> timestamp(byte[] utf8) {
        return plain(Door.TIMESTAMP, utf8, NativeCore::instant);
    }

    /**
     * See {@link #timestamp(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> timestamp(MemorySegment utf8) {
        return plain(Door.TIMESTAMP, utf8, NativeCore::instant);
    }

    /**
     * Casts an integer Unix-epoch value under a caller-declared unit to an {@link Instant}.
     * Negatives (pre-1970) are allowed; a fractional or non-integer value is
     * {@link CastFailure#MALFORMED}; outside the 0001–9999 window is
     * {@link CastFailure#OUT_OF_RANGE}.
     *
     * @param text the text to cast
     * @param precision the declared unit of the epoch value
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> unix(String text, UnixPrecision precision) {
        byte[] utf8 = utf8(text);
        return chars(unix(utf8, precision), text, utf8);
    }

    /**
     * See {@link #unix(String, UnixPrecision)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param precision the declared unit of the epoch value
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> unix(byte[] utf8, UnixPrecision precision) {
        return declared(
                Door.UNIX, utf8, Objects.requireNonNull(precision, "precision").code(), NativeCore::instant);
    }

    /**
     * See {@link #unix(String, UnixPrecision)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @param precision the declared unit of the epoch value
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> unix(MemorySegment utf8, UnixPrecision precision) {
        return declared(
                Door.UNIX, utf8, Objects.requireNonNull(precision, "precision").code(), NativeCore::instant);
    }

    /**
     * Casts an Excel date serial under a caller-declared {@link ExcelEpoch} to an
     * {@link Instant}. The whole part counts days from the system's own day zero and the
     * fraction is the time of day, so {@code 45292.75} is 2024-01-01T18:00:00Z. A
     * spreadsheet cell carries no zone and none is invented.
     *
     * <p>The 1900 system contains a day that never existed: serial {@code 60} is
     * 1900-02-29, kept deliberately because Lotus 1-2-3 wrongly treated 1900 as a leap year
     * and Excel copied the bug for file compatibility. It is {@link CastFailure#OUT_OF_RANGE}
     * here — the same verdict {@link #date(String)} gives the text {@code 1900-02-29} — so
     * every serial above it is shifted one day against a naive count, which is the
     * arithmetic hand-rolled conversions get wrong.
     *
     * @param text the text to cast
     * @param epoch the declared date system
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> excelSerial(String text, ExcelEpoch epoch) {
        byte[] utf8 = utf8(text);
        return chars(excelSerial(utf8, epoch), text, utf8);
    }

    /**
     * See {@link #excelSerial(String, ExcelEpoch)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param epoch the declared date system
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> excelSerial(byte[] utf8, ExcelEpoch epoch) {
        return declared(
                Door.EXCEL_SERIAL, utf8, Objects.requireNonNull(epoch, "epoch").code(), NativeCore::instant);
    }

    /**
     * See {@link #excelSerial(String, ExcelEpoch)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @param epoch the declared date system
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Instant> excelSerial(MemorySegment utf8, ExcelEpoch epoch) {
        return declared(
                Door.EXCEL_SERIAL, utf8, Objects.requireNonNull(epoch, "epoch").code(), NativeCore::instant);
    }

    /**
     * Casts a strict ISO 8601 {@code yyyy-MM-dd} calendar date to a {@link LocalDate}.
     * Anything time-bearing or non-ISO is {@link CastFailure#MALFORMED}; year 0000, month
     * 00 or 13+, or a day the month does not have is {@link CastFailure#OUT_OF_RANGE}, at
     * that field.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(String text) {
        byte[] utf8 = utf8(text);
        return chars(date(utf8), text, utf8);
    }

    /**
     * See {@link #date(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(byte[] utf8) {
        return plain(Door.DATE, utf8, NativeCore::date);
    }

    /**
     * See {@link #date(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(MemorySegment utf8) {
        return plain(Door.DATE, utf8, NativeCore::date);
    }

    /**
     * Casts a separated calendar date — three digit fields joined by one consistent
     * separator ({@code /}, {@code -}, or {@code .}) — under the caller-declared
     * {@link DateOrder} to a {@link LocalDate}: {@code 1/7/2026} is January 7th or July 1st
     * only because {@code order} said which. The year field is four digits wherever the
     * order puts it (two-digit years mean century guessing, which never happens —
     * {@link CastFailure#MALFORMED}); the order-less {@link #date(String)} overload stays
     * strict ISO.
     *
     * @param text the text to cast
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(String text, DateOrder order) {
        byte[] utf8 = utf8(text);
        return chars(date(utf8, order), text, utf8);
    }

    /**
     * See {@link #date(String, DateOrder)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(byte[] utf8, DateOrder order) {
        return declared(
                Door.DATE_ORDERED, utf8, Objects.requireNonNull(order, "order").code(), NativeCore::date);
    }

    /**
     * See {@link #date(String, DateOrder)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDate> date(MemorySegment utf8, DateOrder order) {
        return declared(
                Door.DATE_ORDERED, utf8, Objects.requireNonNull(order, "order").code(), NativeCore::date);
    }

    /**
     * Casts a zone-less civil date-time — the shape untrusted feeds actually send
     * ({@code 1/7/2026 3:04 PM}, {@code 2026-01-07 15:04:05}) — under the caller-declared
     * {@link DateOrder} to a {@link LocalDateTime} at full nanosecond fidelity. The date
     * part follows {@link #date(String, DateOrder)}'s grammar; the optional time part (one
     * space or {@code T} after the date) is 24-hour {@code h:mm[:ss[.f+]]} or 12-hour
     * with an {@code AM}/{@code PM} marker, ASCII case-insensitive, after the clock or
     * ahead of it ({@code PM 3:04}); absent, the time is midnight. A culture's own
     * designators ({@code 오후 3:04}, {@code 3:04 p. m.}) are read only when declared, by
     * {@link #dateTime(String, DateTimeFormat)}. No zone is read and none is invented —
     * the text named no instant, which is exactly what {@link LocalDateTime} says; fusing a
     * zone is the caller's job ({@link #timestamp(String)} stays the strict RFC 3339
     * instant door).
     *
     * @param text the text to cast
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDateTime> dateTime(String text, DateOrder order) {
        byte[] utf8 = utf8(text);
        return chars(dateTime(utf8, order), text, utf8);
    }

    /**
     * See {@link #dateTime(String, DateOrder)}, with the day-period designators the format
     * declares read beside {@code AM}/{@code PM}: {@code 2026-01-07 오후 3:04} is 15:04 under
     * {@code new DateTimeFormat(DateOrder.YEAR_MONTH_DAY, "오전", "오후")}, and
     * {@link CastFailure#MALFORMED} at the designator under the order alone.
     *
     * @param text the text to cast
     * @param format the declared field order and designators
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDateTime> dateTime(String text, DateTimeFormat format) {
        byte[] utf8 = utf8(text);
        return chars(dateTime(utf8, format), text, utf8);
    }

    /**
     * See {@link #dateTime(String, DateOrder)}, with the notation the locale's — its
     * short-date field order and its own day-period designators — as
     * {@link DateTimeFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose date formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<LocalDateTime> dateTime(String text, Locale locale) {
        return dateTime(text, DateTimeFormat.from(locale));
    }

    /**
     * See {@link #dateTime(String, DateOrder)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDateTime> dateTime(byte[] utf8, DateOrder order) {
        return civil(utf8, Objects.requireNonNull(order, "order"), null);
    }

    /**
     * See {@link #dateTime(String, DateTimeFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the declared field order and designators
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDateTime> dateTime(byte[] utf8, DateTimeFormat format) {
        return civil(utf8, Objects.requireNonNull(format, "format").order(), format);
    }

    /**
     * See {@link #dateTime(byte[], DateOrder)}, with the notation the locale's — its
     * short-date field order and its own day-period designators — as
     * {@link DateTimeFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose date formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<LocalDateTime> dateTime(byte[] utf8, Locale locale) {
        return dateTime(utf8, DateTimeFormat.from(locale));
    }

    /**
     * See {@link #dateTime(String, DateOrder)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDateTime> dateTime(MemorySegment utf8, DateOrder order) {
        return civil(utf8, Objects.requireNonNull(order, "order"), null);
    }

    /**
     * See {@link #dateTime(String, DateTimeFormat)}; input as a {@link MemorySegment} view of
     * UTF-8 bytes — heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the declared field order and designators
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDateTime> dateTime(MemorySegment utf8, DateTimeFormat format) {
        return civil(utf8, Objects.requireNonNull(format, "format").order(), format);
    }

    /**
     * See {@link #dateTime(MemorySegment, DateOrder)}, with the notation the locale's — its
     * short-date field order and its own day-period designators — as
     * {@link DateTimeFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose date formatting declares the notation; pass
     *     {@code Locale.getDefault(Locale.Category.FORMAT)} for the default
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static Verdict<LocalDateTime> dateTime(MemorySegment utf8, Locale locale) {
        return dateTime(utf8, DateTimeFormat.from(locale));
    }

    /**
     * Casts an ISO 8601 24-hour time-of-day — {@code HH:mm}, {@code HH:mm:ss}, or
     * {@code HH:mm:ss.f+} — to a {@link LocalTime} at full nanosecond fidelity,
     * {@code 00:00} through {@code 23:59:59.999999999}. A well-formed hour past 23 or minute
     * or second past 59 ({@code 24:00}, {@code 15:04:60}) is
     * {@link CastFailure#OUT_OF_RANGE}, at that field.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalTime> time(String text) {
        byte[] utf8 = utf8(text);
        return chars(time(utf8), text, utf8);
    }

    /**
     * See {@link #time(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalTime> time(byte[] utf8) {
        return plain(Door.TIME, utf8, NativeCore::time);
    }

    /**
     * See {@link #time(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalTime> time(MemorySegment utf8) {
        return plain(Door.TIME, utf8, NativeCore::time);
    }

    /**
     * Casts a duration in any of three cleanly-partitioned shapes to a {@link Duration} at
     * full nanosecond fidelity: an ISO 8601 duration restricted to fixed components
     * ({@code P2W}, {@code P1DT6H30M15.5S} — years/months are not fixed durations and are
     * {@link CastFailure#MALFORMED}), the invariant colon form
     * ({@code [-][d.]hh:mm[:ss[.f]]}), or protobuf JSON seconds ({@code 3.5s}). Beyond
     * ±10,000 years is {@link CastFailure#OUT_OF_RANGE}.
     *
     * @param text the text to cast
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Duration> duration(String text) {
        byte[] utf8 = utf8(text);
        return chars(duration(utf8), text, utf8);
    }

    /**
     * See {@link #duration(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Duration> duration(byte[] utf8) {
        return plain(Door.DURATION, utf8, NativeCore::duration);
    }

    /**
     * See {@link #duration(String)}; input as a {@link MemorySegment} view of UTF-8 bytes —
     * heap or native — crossing without a copy.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Duration> duration(MemorySegment utf8) {
        return plain(Door.DURATION, utf8, NativeCore::duration);
    }

    // --- typed doors: a number the caller already holds ---

    /**
     * Reads a number the caller already holds — the {@code double} a workbook stores for a
     * numeric cell — as the exact {@link BigDecimal} it names: the shortest decimal that rounds
     * back to the double, the digits a spreadsheet writes for it. So {@code 0.1} is one tenth,
     * not the binary fraction {@code new BigDecimal(0.1)} spells out, and {@code 0.1 + 0.2} is
     * {@code 0.30000000000000004}. The result is canonical, as
     * {@link #decimal(String, NumFormat)}'s is. {@code NaN} is {@link CastFailure#MALFORMED};
     * an infinity, a magnitude past 2<sup>96</sup>&minus;1 or more than 28 places is
     * {@link CastFailure#OUT_OF_RANGE} — the digits are never cut to fit. A typed door's
     * {@link Fault} has no span: its offset and length are 0.
     *
     * @param value the number to read
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<BigDecimal> decimalFromDouble(double value) {
        return typed(Door.DECIMAL_FROM_F64, value, NativeCore::decimal);
    }

    /**
     * Reads an Excel date serial the caller already holds as a {@code double} under a declared
     * {@link ExcelEpoch} — the twin of {@link #excelSerial(String, ExcelEpoch)} for a workbook
     * reader that has the cell's number and no text. The result is the zone-less wall clock
     * the cell holds, a {@link LocalDateTime} as {@link #dateTime(String, DateOrder)} returns,
     * its fraction snapped: the time with the fewest fractional-second digits that the same double
     * stores, so Excel's 23:59:59 is read on the second rather than the nanoseconds of float
     * noise the double carries. The 1900 system's phantom serial
     * {@code 60}, a serial below the system's first day and one past 9999-12-31 are
     * {@link CastFailure#OUT_OF_RANGE}; a negative, NaN or infinite serial is
     * {@link CastFailure#MALFORMED}.
     *
     * @param value the serial
     * @param epoch the declared date system
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalDateTime> excelSerialFromDouble(double value, ExcelEpoch epoch) {
        int code = Objects.requireNonNull(epoch, "epoch").code();
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(
                NativeCore.typedDeclared(scratch, Door.EXCEL_SERIAL_FROM_F64, value, code),
                scratch,
                NativeCore::dateTime);
    }

    /**
     * Reads the fraction of an Excel serial the caller already holds as a {@code double} as a
     * {@link LocalTime} at full nanosecond fidelity: {@code 0.75} and {@code 45292.75} are
     * both 18:00. Snapped as the serial door snaps, and a fraction that snaps to a whole day is
     * midnight. A negative, NaN or infinite serial is {@link CastFailure#MALFORMED}; one
     * past 9999-12-31 is {@link CastFailure#OUT_OF_RANGE}.
     *
     * @param value the serial
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<LocalTime> excelTime(double value) {
        return typed(Door.EXCEL_TIME, value, NativeCore::time);
    }

    /**
     * Reads a number of days the caller already holds as a {@code double} — what an
     * elapsed-time format ({@code [h]:mm:ss}) stores — as a {@link Duration} at full
     * nanosecond fidelity: {@code 1.5} is a day and twelve hours, and a negative span is
     * negative, its size snapped as a serial's time. NaN or an infinity is
     * {@link CastFailure#MALFORMED}; beyond ±10,000 years is {@link CastFailure#OUT_OF_RANGE}.
     *
     * @param value the number of days
     * @return the verdict: a {@link Success} carrying the cast value, or a {@link Fault}
     */
    public static Verdict<Duration> excelDuration(double value) {
        return typed(Door.EXCEL_DURATION, value, NativeCore::duration);
    }
}
