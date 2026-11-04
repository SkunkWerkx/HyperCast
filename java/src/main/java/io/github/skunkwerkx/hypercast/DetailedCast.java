package io.github.skunkwerkx.hypercast;

import io.github.skunkwerkx.hypercast.interop.NativeValues;
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
import java.util.OptionalInt;
import java.util.UUID;
import java.util.function.IntUnaryOperator;

/**
 * The detailed path: every door of {@link Cast}, with the failure's {@link FaultDetail}. Each
 * door answers a {@link DetailedVerdict} (or, with {@link Constraints}, a
 * {@link DetailedCheckedVerdict}): the same value, reason and span the {@link Cast} door gives,
 * and on a failure what failed, as the core names it ({@code group_size}, {@code too_large},
 * {@code digit_expected}). It is a separate class with its own result types because
 * {@link Verdict} and {@link Rejection} are sealed: a detail on {@link Fault}, or a third case,
 * would break the exhaustive switches written against them. {@link Cast}, {@link Verdict},
 * {@link Fault} and every other 1.0 type are exactly what they were.
 *
 * <p>The doors are named and shaped as {@link Cast}'s, over a {@link String}, raw UTF-8
 * {@code byte[]} or {@link MemorySegment}, and spans are in the units {@link Fault} documents.
 * A numeric door takes a {@link NumFormat} (or a {@link Locale}, as {@link NumFormat#from(Locale)}
 * derives it), whose {@link Grouping} the core enforces.
 *
 * <p>This is also where the opt-in date shapes arrive: {@link DateOptions} and
 * {@link MonthNames} are accepted by the date doors here and nowhere else, as the core takes
 * them only through its detailed exports. A door handed neither keeps today's shape rules.
 * {@link DateOptions#COMMON_FORMS} is safe at every date door; an option that means nothing to a
 * door is ignored there.
 *
 * <p>A detail this binding does not name, one a later core publishes, is not an error: the
 * {@link FaultDetail} keeps its code, and the reason and span are as usable as any.
 */
public final class DetailedCast {
    private DetailedCast() {}

    /** Builds a door's value out of the scratch its successful call wrote. */
    private interface Reader<T> {
        T read(NativeCore.Scratch scratch);
    }

    private static <T> DetailedVerdict<T> verdict(int code, NativeCore.Scratch scratch, Reader<T> reader) {
        return code == 0 ? new DetailedSuccess<>(reader.read(scratch)) : fault(code, scratch);
    }

    // The fault a nonzero code names, at the span and with the detail the call wrote. -1 is a
    // contract violation, which every door's own checks keep from happening: a binding bug,
    // never data.
    private static <T> DetailedFault<T> fault(int code, NativeCore.Scratch scratch) {
        if (code == -1) {
            throw new IllegalStateException(
                    "libhypercast reported a contract violation — a binding bug, please report it");
        }
        return NativeValues.detailedFault(
                code,
                NativeCore.faultOffset(scratch),
                NativeCore.faultLength(scratch),
                NativeCore.faultDetail(scratch));
    }

    // The door shapes, each over both input forms. A null argument is refused here, named,
    // before anything crosses, so both paths answer it alike.
    private static <T> DetailedVerdict<T> plain(int door, byte[] utf8, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.plain(scratch, door, utf8), scratch, reader);
    }

    private static <T> DetailedVerdict<T> plain(int door, MemorySegment utf8, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.plain(scratch, door, utf8), scratch, reader);
    }

    private static <T> DetailedVerdict<T> numeric(int door, byte[] utf8, NumFormat format, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        Objects.requireNonNull(format, "format");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.numeric(scratch, door, utf8, format), scratch, reader);
    }

    private static <T> DetailedVerdict<T> numeric(int door, MemorySegment utf8, NumFormat format, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        Objects.requireNonNull(format, "format");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.numeric(scratch, door, utf8, format), scratch, reader);
    }

    private static <T> DetailedVerdict<T> declared(int door, byte[] utf8, int code, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.declared(scratch, door, utf8, code), scratch, reader);
    }

    private static <T> DetailedVerdict<T> declared(int door, MemorySegment utf8, int code, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.declared(scratch, door, utf8, code), scratch, reader);
    }

    // `format` is null at every door but the civil one, and at that one for a DateOrder.
    private static <T> DetailedVerdict<T> dated(
            int door,
            int param,
            byte[] utf8,
            DateTimeFormat format,
            DateOptions options,
            MonthNames months,
            Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        Objects.requireNonNull(options, "options");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.dated(scratch, door, param, utf8, format, options, months), scratch, reader);
    }

    private static <T> DetailedVerdict<T> dated(
            int door,
            int param,
            MemorySegment utf8,
            DateTimeFormat format,
            DateOptions options,
            MonthNames months,
            Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        Objects.requireNonNull(options, "options");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.dated(scratch, door, param, utf8, format, options, months), scratch, reader);
    }

    private static <T> DetailedVerdict<T> typed(int door, double value, Reader<T> reader) {
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(NativeCore.typed(scratch, door, value), scratch, reader);
    }

    // A fault's span is byte offsets into the UTF-8 the core saw. For a String door that is the
    // wrong unit whenever the encoding came out longer than the String, so the span is rebased
    // to UTF-16 code units, as Cast rebases a Fault's.
    private static <T> DetailedVerdict<T> chars(DetailedVerdict<T> verdict, String text, byte[] utf8) {
        if (utf8.length == text.length() || !(verdict instanceof DetailedFault<T> fault)) {
            return verdict;
        }
        int start = Cast.charIndex(utf8, fault.offset());
        int end = Cast.charIndex(utf8, fault.offset() + fault.length());
        return new DetailedFault<>(fault.reason(), fault.detail(), start, end - start);
    }

    /**
     * Presents a detailed verdict optionally: an {@link CastFailure#EMPTY} fault becomes absent
     * ({@link java.util.Optional#empty()}); every other outcome flows through untouched.
     *
     * @param <T> the verdict's value type
     * @param verdict the verdict to present
     * @return empty for an EMPTY fault; the untouched verdict otherwise
     */
    public static <T> java.util.Optional<DetailedVerdict<T>> optional(DetailedVerdict<T> verdict) {
        Objects.requireNonNull(verdict, "verdict");
        return verdict instanceof DetailedFault<T> fault && fault.reason() == CastFailure.EMPTY
                ? java.util.Optional.empty()
                : java.util.Optional.of(verdict);
    }

    /**
     * Presents a checked detailed verdict optionally: an {@link CastFailure#EMPTY} fault becomes
     * absent ({@link java.util.Optional#empty()}); every other outcome, a
     * {@link DetailedViolation} included, flows through untouched.
     *
     * @param <T> the verdict's value type
     * @param verdict the checked verdict to present
     * @return empty for an EMPTY fault; the untouched verdict otherwise
     */
    public static <T> java.util.Optional<DetailedCheckedVerdict<T>> optional(DetailedCheckedVerdict<T> verdict) {
        Objects.requireNonNull(verdict, "verdict");
        return verdict instanceof DetailedFault<T> fault && fault.reason() == CastFailure.EMPTY
                ? java.util.Optional.empty()
                : java.util.Optional.of(verdict);
    }

    /**
     * Presents a fault optionally, as {@link #optional(DetailedVerdict)} does. A
     * {@link DetailedFault} is both a {@link DetailedVerdict} and a {@link DetailedCheckedVerdict};
     * this overload keeps a call on an expression typed {@code DetailedFault} unambiguous.
     *
     * @param <T> the verdict's value type
     * @param fault the fault to present
     * @return empty for an EMPTY fault; the untouched fault otherwise
     */
    public static <T> java.util.Optional<DetailedVerdict<T>> optional(DetailedFault<T> fault) {
        return optional((DetailedVerdict<T>) fault);
    }

    // --- char ---

    /**
     * Casts char text, as {@link Cast#character(String)} does: one character verbatim or a
     * declared code point; a scalar above {@code U+FFFF} is {@link CastFailure#OUT_OF_RANGE}
     * with the detail {@link FaultDetail#TOO_LARGE}, since one {@code char} cannot hold it.
     *
     * @param text the text to cast
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Character> character(String text) {
        if (Objects.requireNonNull(text, "text").length() == 1) {
            return new DetailedSuccess<>(text.charAt(0));
        }
        byte[] utf8 = Cast.utf8(text);
        return chars(character(utf8), text, utf8);
    }

    /**
     * See {@link #character(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Character> character(byte[] utf8) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return character(NativeCore.plain(scratch, Door.CHAR, utf8), scratch, utf8.length, i -> utf8[i]);
    }

    /**
     * See {@link #character(String)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Character> character(MemorySegment utf8) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        return character(
                NativeCore.plain(scratch, Door.CHAR, utf8),
                scratch,
                (int) utf8.byteSize(),
                i -> utf8.get(ValueLayout.JAVA_BYTE, i));
    }

    // `byteAt` reads the input the core saw, for the one verdict built here rather than there.
    private static DetailedVerdict<Character> character(
            int code, NativeCore.Scratch scratch, int len, IntUnaryOperator byteAt) {
        if (code != 0) {
            return fault(code, scratch);
        }
        int scalar = NativeCore.scalar(scratch);
        if (scalar <= Character.MAX_VALUE) {
            return new DetailedSuccess<>((char) scalar);
        }
        // A supplementary scalar the core accepted but one char cannot hold: out of range over
        // the token the core read, the input less its white-space edges, and too large for the
        // type.
        int start = 0;
        int end = len;
        for (int step; (step = WhiteSpace.lengthAt(byteAt, start, end)) > 0; ) {
            start += step;
        }
        for (int step; (step = WhiteSpace.lengthBefore(byteAt, start, end)) > 0; ) {
            end -= step;
        }
        return new DetailedFault<>(CastFailure.OUT_OF_RANGE, FaultDetail.TOO_LARGE, start, end - start);
    }

    // --- boolean ---

    /**
     * Casts boolean text, as {@link Cast#bool(String)} does.
     *
     * @param text the text to cast
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Boolean> bool(String text) {
        byte[] utf8 = Cast.utf8(text);
        return chars(bool(utf8), text, utf8);
    }

    /**
     * See {@link #bool(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Boolean> bool(byte[] utf8) {
        return plain(Door.BOOL, utf8, NativeCore::bool);
    }

    /**
     * See {@link #bool(String)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Boolean> bool(MemorySegment utf8) {
        return plain(Door.BOOL, utf8, NativeCore::bool);
    }

    // --- numerics: integers, reals and decimals under a declared format ---

    /**
     * Casts numeric text to an 8-bit signed integer under the declared format, as
     * {@link Cast#i8(String, NumFormat)} does, with the detail of a failure. The format's
     * {@link Grouping} is enforced: under the default, {@code 12,34} is
     * {@link FaultDetail#GROUP_SIZE}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Byte> i8(String text, NumFormat format) {
        byte[] utf8 = Cast.utf8(text);
        return chars(i8(utf8, format), text, utf8);
    }

    /**
     * See {@link #i8(String, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Byte> i8(String text, Locale locale) {
        return i8(text, NumFormat.from(locale));
    }

    /**
     * See {@link #i8(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Byte> i8(byte[] utf8, NumFormat format) {
        return numeric(Door.I8, utf8, format, NativeCore::i8);
    }

    /**
     * See {@link #i8(byte[], NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Byte> i8(byte[] utf8, Locale locale) {
        return i8(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #i8(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Byte> i8(MemorySegment utf8, NumFormat format) {
        return numeric(Door.I8, utf8, format, NativeCore::i8);
    }

    /**
     * See {@link #i8(MemorySegment, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Byte> i8(MemorySegment utf8, Locale locale) {
        return i8(utf8, NumFormat.from(locale));
    }

    /**
     * Casts numeric text to a 16-bit signed integer under the declared format, as
     * {@link Cast#i16(String, NumFormat)} does, with the detail of a failure. The format's
     * {@link Grouping} is enforced: under the default, {@code 12,34} is
     * {@link FaultDetail#GROUP_SIZE}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Short> i16(String text, NumFormat format) {
        byte[] utf8 = Cast.utf8(text);
        return chars(i16(utf8, format), text, utf8);
    }

    /**
     * See {@link #i16(String, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Short> i16(String text, Locale locale) {
        return i16(text, NumFormat.from(locale));
    }

    /**
     * See {@link #i16(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Short> i16(byte[] utf8, NumFormat format) {
        return numeric(Door.I16, utf8, format, NativeCore::i16);
    }

    /**
     * See {@link #i16(byte[], NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Short> i16(byte[] utf8, Locale locale) {
        return i16(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #i16(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Short> i16(MemorySegment utf8, NumFormat format) {
        return numeric(Door.I16, utf8, format, NativeCore::i16);
    }

    /**
     * See {@link #i16(MemorySegment, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Short> i16(MemorySegment utf8, Locale locale) {
        return i16(utf8, NumFormat.from(locale));
    }

    /**
     * Casts numeric text to a 32-bit signed integer under the declared format, as
     * {@link Cast#i32(String, NumFormat)} does, with the detail of a failure. The format's
     * {@link Grouping} is enforced: under the default, {@code 12,34} is
     * {@link FaultDetail#GROUP_SIZE}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Integer> i32(String text, NumFormat format) {
        byte[] utf8 = Cast.utf8(text);
        return chars(i32(utf8, format), text, utf8);
    }

    /**
     * See {@link #i32(String, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Integer> i32(String text, Locale locale) {
        return i32(text, NumFormat.from(locale));
    }

    /**
     * See {@link #i32(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Integer> i32(byte[] utf8, NumFormat format) {
        return numeric(Door.I32, utf8, format, NativeCore::i32);
    }

    /**
     * See {@link #i32(byte[], NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Integer> i32(byte[] utf8, Locale locale) {
        return i32(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #i32(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Integer> i32(MemorySegment utf8, NumFormat format) {
        return numeric(Door.I32, utf8, format, NativeCore::i32);
    }

    /**
     * See {@link #i32(MemorySegment, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Integer> i32(MemorySegment utf8, Locale locale) {
        return i32(utf8, NumFormat.from(locale));
    }

    /**
     * Casts numeric text to a 64-bit signed integer under the declared format, as
     * {@link Cast#i64(String, NumFormat)} does, with the detail of a failure. The format's
     * {@link Grouping} is enforced: under the default, {@code 12,34} is
     * {@link FaultDetail#GROUP_SIZE}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Long> i64(String text, NumFormat format) {
        byte[] utf8 = Cast.utf8(text);
        return chars(i64(utf8, format), text, utf8);
    }

    /**
     * See {@link #i64(String, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Long> i64(String text, Locale locale) {
        return i64(text, NumFormat.from(locale));
    }

    /**
     * See {@link #i64(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Long> i64(byte[] utf8, NumFormat format) {
        return numeric(Door.I64, utf8, format, NativeCore::i64);
    }

    /**
     * See {@link #i64(byte[], NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Long> i64(byte[] utf8, Locale locale) {
        return i64(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #i64(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Long> i64(MemorySegment utf8, NumFormat format) {
        return numeric(Door.I64, utf8, format, NativeCore::i64);
    }

    /**
     * See {@link #i64(MemorySegment, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Long> i64(MemorySegment utf8, Locale locale) {
        return i64(utf8, NumFormat.from(locale));
    }

    /**
     * Casts numeric text to an 8-bit unsigned integer under the declared format, as
     * {@link Cast#u8(String, NumFormat)} does, with the detail of a failure. The format's
     * {@link Grouping} is enforced: under the default, {@code 12,34} is
     * {@link FaultDetail#GROUP_SIZE}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Integer> u8(String text, NumFormat format) {
        byte[] utf8 = Cast.utf8(text);
        return chars(u8(utf8, format), text, utf8);
    }

    /**
     * See {@link #u8(String, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Integer> u8(String text, Locale locale) {
        return u8(text, NumFormat.from(locale));
    }

    /**
     * See {@link #u8(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Integer> u8(byte[] utf8, NumFormat format) {
        return numeric(Door.U8, utf8, format, NativeCore::u8);
    }

    /**
     * See {@link #u8(byte[], NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Integer> u8(byte[] utf8, Locale locale) {
        return u8(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #u8(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Integer> u8(MemorySegment utf8, NumFormat format) {
        return numeric(Door.U8, utf8, format, NativeCore::u8);
    }

    /**
     * See {@link #u8(MemorySegment, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Integer> u8(MemorySegment utf8, Locale locale) {
        return u8(utf8, NumFormat.from(locale));
    }

    /**
     * Casts numeric text to a 16-bit unsigned integer under the declared format, as
     * {@link Cast#u16(String, NumFormat)} does, with the detail of a failure. The format's
     * {@link Grouping} is enforced: under the default, {@code 12,34} is
     * {@link FaultDetail#GROUP_SIZE}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Integer> u16(String text, NumFormat format) {
        byte[] utf8 = Cast.utf8(text);
        return chars(u16(utf8, format), text, utf8);
    }

    /**
     * See {@link #u16(String, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Integer> u16(String text, Locale locale) {
        return u16(text, NumFormat.from(locale));
    }

    /**
     * See {@link #u16(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Integer> u16(byte[] utf8, NumFormat format) {
        return numeric(Door.U16, utf8, format, NativeCore::u16);
    }

    /**
     * See {@link #u16(byte[], NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Integer> u16(byte[] utf8, Locale locale) {
        return u16(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #u16(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Integer> u16(MemorySegment utf8, NumFormat format) {
        return numeric(Door.U16, utf8, format, NativeCore::u16);
    }

    /**
     * See {@link #u16(MemorySegment, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Integer> u16(MemorySegment utf8, Locale locale) {
        return u16(utf8, NumFormat.from(locale));
    }

    /**
     * Casts numeric text to a 32-bit unsigned integer under the declared format, as
     * {@link Cast#u32(String, NumFormat)} does, with the detail of a failure. The format's
     * {@link Grouping} is enforced: under the default, {@code 12,34} is
     * {@link FaultDetail#GROUP_SIZE}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Long> u32(String text, NumFormat format) {
        byte[] utf8 = Cast.utf8(text);
        return chars(u32(utf8, format), text, utf8);
    }

    /**
     * See {@link #u32(String, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Long> u32(String text, Locale locale) {
        return u32(text, NumFormat.from(locale));
    }

    /**
     * See {@link #u32(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Long> u32(byte[] utf8, NumFormat format) {
        return numeric(Door.U32, utf8, format, NativeCore::u32);
    }

    /**
     * See {@link #u32(byte[], NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Long> u32(byte[] utf8, Locale locale) {
        return u32(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #u32(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Long> u32(MemorySegment utf8, NumFormat format) {
        return numeric(Door.U32, utf8, format, NativeCore::u32);
    }

    /**
     * See {@link #u32(MemorySegment, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Long> u32(MemorySegment utf8, Locale locale) {
        return u32(utf8, NumFormat.from(locale));
    }

    /**
     * Casts numeric text to a 64-bit unsigned integer under the declared format, as
     * {@link Cast#u64(String, NumFormat)} does, with the detail of a failure. The format's
     * {@link Grouping} is enforced: under the default, {@code 12,34} is
     * {@link FaultDetail#GROUP_SIZE}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Long> u64(String text, NumFormat format) {
        byte[] utf8 = Cast.utf8(text);
        return chars(u64(utf8, format), text, utf8);
    }

    /**
     * See {@link #u64(String, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Long> u64(String text, Locale locale) {
        return u64(text, NumFormat.from(locale));
    }

    /**
     * See {@link #u64(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Long> u64(byte[] utf8, NumFormat format) {
        return numeric(Door.U64, utf8, format, NativeCore::u64);
    }

    /**
     * See {@link #u64(byte[], NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Long> u64(byte[] utf8, Locale locale) {
        return u64(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #u64(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Long> u64(MemorySegment utf8, NumFormat format) {
        return numeric(Door.U64, utf8, format, NativeCore::u64);
    }

    /**
     * See {@link #u64(MemorySegment, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Long> u64(MemorySegment utf8, Locale locale) {
        return u64(utf8, NumFormat.from(locale));
    }

    /**
     * Casts numeric text to a 32-bit float under the declared format, as
     * {@link Cast#f32(String, NumFormat)} does, with the detail of a failure. The format's
     * {@link Grouping} is enforced: under the default, {@code 12,34} is
     * {@link FaultDetail#GROUP_SIZE}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Float> f32(String text, NumFormat format) {
        byte[] utf8 = Cast.utf8(text);
        return chars(f32(utf8, format), text, utf8);
    }

    /**
     * See {@link #f32(String, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Float> f32(String text, Locale locale) {
        return f32(text, NumFormat.from(locale));
    }

    /**
     * See {@link #f32(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Float> f32(byte[] utf8, NumFormat format) {
        return numeric(Door.F32, utf8, format, NativeCore::f32);
    }

    /**
     * See {@link #f32(byte[], NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Float> f32(byte[] utf8, Locale locale) {
        return f32(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #f32(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Float> f32(MemorySegment utf8, NumFormat format) {
        return numeric(Door.F32, utf8, format, NativeCore::f32);
    }

    /**
     * See {@link #f32(MemorySegment, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Float> f32(MemorySegment utf8, Locale locale) {
        return f32(utf8, NumFormat.from(locale));
    }

    /**
     * Casts numeric text to a 64-bit float under the declared format, as
     * {@link Cast#f64(String, NumFormat)} does, with the detail of a failure. The format's
     * {@link Grouping} is enforced: under the default, {@code 12,34} is
     * {@link FaultDetail#GROUP_SIZE}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Double> f64(String text, NumFormat format) {
        byte[] utf8 = Cast.utf8(text);
        return chars(f64(utf8, format), text, utf8);
    }

    /**
     * See {@link #f64(String, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Double> f64(String text, Locale locale) {
        return f64(text, NumFormat.from(locale));
    }

    /**
     * See {@link #f64(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Double> f64(byte[] utf8, NumFormat format) {
        return numeric(Door.F64, utf8, format, NativeCore::f64);
    }

    /**
     * See {@link #f64(byte[], NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Double> f64(byte[] utf8, Locale locale) {
        return f64(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #f64(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Double> f64(MemorySegment utf8, NumFormat format) {
        return numeric(Door.F64, utf8, format, NativeCore::f64);
    }

    /**
     * See {@link #f64(MemorySegment, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<Double> f64(MemorySegment utf8, Locale locale) {
        return f64(utf8, NumFormat.from(locale));
    }

    /**
     * Casts numeric text to an exact decimal under the declared format, as
     * {@link Cast#decimal(String, NumFormat)} does, with the detail of a failure. The format's
     * {@link Grouping} is enforced: under the default, {@code 12,34} is
     * {@link FaultDetail#GROUP_SIZE}.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<BigDecimal> decimal(String text, NumFormat format) {
        byte[] utf8 = Cast.utf8(text);
        return chars(decimal(utf8, format), text, utf8);
    }

    /**
     * See {@link #decimal(String, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<BigDecimal> decimal(String text, Locale locale) {
        return decimal(text, NumFormat.from(locale));
    }

    /**
     * See {@link #decimal(String, NumFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<BigDecimal> decimal(byte[] utf8, NumFormat format) {
        return numeric(Door.DECIMAL, utf8, format, NativeCore::decimal);
    }

    /**
     * See {@link #decimal(byte[], NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<BigDecimal> decimal(byte[] utf8, Locale locale) {
        return decimal(utf8, NumFormat.from(locale));
    }

    /**
     * See {@link #decimal(String, NumFormat)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<BigDecimal> decimal(MemorySegment utf8, NumFormat format) {
        return numeric(Door.DECIMAL, utf8, format, NativeCore::decimal);
    }

    /**
     * See {@link #decimal(MemorySegment, NumFormat)}, with the notation the locale's, as
     * {@link NumFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose number formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<BigDecimal> decimal(MemorySegment utf8, Locale locale) {
        return decimal(utf8, NumFormat.from(locale));
    }

    // --- uuid ---

    /**
     * Casts a UUID, as {@link Cast#uuid(String)} does.
     *
     * @param text the text to cast
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<UUID> uuid(String text) {
        byte[] utf8 = Cast.utf8(text);
        return chars(uuid(utf8), text, utf8);
    }

    /**
     * See {@link #uuid(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<UUID> uuid(byte[] utf8) {
        return plain(Door.UUID, utf8, NativeCore::uuid);
    }

    /**
     * See {@link #uuid(String)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<UUID> uuid(MemorySegment utf8) {
        return plain(Door.UUID, utf8, NativeCore::uuid);
    }

    // --- timestamp ---

    /**
     * Casts an RFC 3339 instant, as {@link Cast#timestamp(String)} does.
     *
     * @param text the text to cast
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Instant> timestamp(String text) {
        byte[] utf8 = Cast.utf8(text);
        return chars(timestamp(utf8), text, utf8);
    }

    /**
     * See {@link #timestamp(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Instant> timestamp(byte[] utf8) {
        return plain(Door.TIMESTAMP, utf8, NativeCore::instant);
    }

    /**
     * See {@link #timestamp(String)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Instant> timestamp(MemorySegment utf8) {
        return plain(Door.TIMESTAMP, utf8, NativeCore::instant);
    }

    /**
     * Casts an RFC 3339 instant under opt-in shapes: {@link DateOptions#SPACE_SEPARATOR} reads a
     * single space in place of {@code T}, and {@link DateOptions#HOUR_OFFSET} reads {@code ±hh}
     * offsets; every other option is ignored at this door. Otherwise as
     * {@link #timestamp(String)}.
     *
     * @param text the text to cast
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Instant> timestamp(String text, DateOptions options) {
        byte[] utf8 = Cast.utf8(text);
        return chars(timestamp(utf8, options), text, utf8);
    }

    /**
     * See {@link #timestamp(String, DateOptions)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Instant> timestamp(byte[] utf8, DateOptions options) {
        return dated(Door.TIMESTAMP, 0, utf8, null, options, null, NativeCore::instant);
    }

    /**
     * See {@link #timestamp(String, DateOptions)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Instant> timestamp(MemorySegment utf8, DateOptions options) {
        return dated(Door.TIMESTAMP, 0, utf8, null, options, null, NativeCore::instant);
    }

    // --- temporals ---

    /**
     * Casts an integer Unix-epoch value under a declared unit, as
     * {@link Cast#unix(String, UnixPrecision)} does.
     *
     * @param text the text to cast
     * @param precision the declared unit of the epoch value
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Instant> unix(String text, UnixPrecision precision) {
        byte[] utf8 = Cast.utf8(text);
        return chars(unix(utf8, precision), text, utf8);
    }

    /**
     * See {@link #unix(String, UnixPrecision)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param precision the declared unit of the epoch value
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Instant> unix(byte[] utf8, UnixPrecision precision) {
        return declared(
                Door.UNIX, utf8, Objects.requireNonNull(precision, "precision").code(), NativeCore::instant);
    }

    /**
     * See {@link #unix(String, UnixPrecision)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param precision the declared unit of the epoch value
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Instant> unix(MemorySegment utf8, UnixPrecision precision) {
        return declared(
                Door.UNIX, utf8, Objects.requireNonNull(precision, "precision").code(), NativeCore::instant);
    }

    /**
     * Casts an Excel date serial under a declared date system, as
     * {@link Cast#excelSerial(String, ExcelEpoch)} does.
     *
     * @param text the text to cast
     * @param epoch the declared date system
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Instant> excelSerial(String text, ExcelEpoch epoch) {
        byte[] utf8 = Cast.utf8(text);
        return chars(excelSerial(utf8, epoch), text, utf8);
    }

    /**
     * See {@link #excelSerial(String, ExcelEpoch)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param epoch the declared date system
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Instant> excelSerial(byte[] utf8, ExcelEpoch epoch) {
        return declared(
                Door.EXCEL_SERIAL, utf8, Objects.requireNonNull(epoch, "epoch").code(), NativeCore::instant);
    }

    /**
     * See {@link #excelSerial(String, ExcelEpoch)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param epoch the declared date system
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Instant> excelSerial(MemorySegment utf8, ExcelEpoch epoch) {
        return declared(
                Door.EXCEL_SERIAL, utf8, Objects.requireNonNull(epoch, "epoch").code(), NativeCore::instant);
    }

    // --- calendar date ---

    /**
     * Casts a strict ISO 8601 calendar date, as {@link Cast#date(String)} does.
     *
     * @param text the text to cast
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(String text) {
        byte[] utf8 = Cast.utf8(text);
        return chars(date(utf8), text, utf8);
    }

    /**
     * See {@link #date(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(byte[] utf8) {
        return plain(Door.DATE, utf8, NativeCore::date);
    }

    /**
     * See {@link #date(String)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(MemorySegment utf8) {
        return plain(Door.DATE, utf8, NativeCore::date);
    }

    /**
     * Casts a calendar date under opt-in shapes: {@link DateOptions#COMPACT_DATE} reads
     * {@code yyyyMMdd}, eight ASCII digits; every other option is ignored at this door.
     * Otherwise as {@link #date(String)}.
     *
     * @param text the text to cast
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(String text, DateOptions options) {
        byte[] utf8 = Cast.utf8(text);
        return chars(date(utf8, options), text, utf8);
    }

    /**
     * See {@link #date(String, DateOptions)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(byte[] utf8, DateOptions options) {
        return dated(Door.DATE, 0, utf8, null, options, null, NativeCore::date);
    }

    /**
     * See {@link #date(String, DateOptions)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(MemorySegment utf8, DateOptions options) {
        return dated(Door.DATE, 0, utf8, null, options, null, NativeCore::date);
    }

    // --- separated dates ---

    /**
     * Casts a separated calendar date under a declared field order, as
     * {@link Cast#date(String, DateOrder)} does.
     *
     * @param text the text to cast
     * @param order the declared field order
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(String text, DateOrder order) {
        byte[] utf8 = Cast.utf8(text);
        return chars(date(utf8, order), text, utf8);
    }

    /**
     * Casts a separated calendar date under a declared field order and opt-in shapes:
     * the {@link DateOptions#pivot() pivot} reads a two-digit year, and the other options are
     * ignored at this door. Otherwise as {@link #date(String, DateOrder)}.
     *
     * @param text the text to cast
     * @param order the declared field order
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(String text, DateOrder order, DateOptions options) {
        byte[] utf8 = Cast.utf8(text);
        return chars(date(utf8, order, options), text, utf8);
    }

    /**
     * Casts a separated calendar date under a declared field order, opt-in shapes and a
     * {@link MonthNames month vocabulary}: a declared name stands for the month field, in the
     * position the order selects ({@code 07-Jan-26} with a pivot, {@code 7-Jan-2026} without).
     * Otherwise as {@link #date(String, DateOrder, DateOptions)}.
     *
     * @param text the text to cast
     * @param order the declared field order
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @param months the declared month names
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(
            String text, DateOrder order, DateOptions options, MonthNames months) {
        byte[] utf8 = Cast.utf8(text);
        return chars(date(utf8, order, options, months), text, utf8);
    }

    /**
     * See {@link #date(String, DateOrder)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(byte[] utf8, DateOrder order) {
        return declared(
                Door.DATE_ORDERED, utf8, Objects.requireNonNull(order, "order").code(), NativeCore::date);
    }

    /**
     * See {@link #date(String, DateOrder, DateOptions)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param order the declared field order
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(byte[] utf8, DateOrder order, DateOptions options) {
        return dated(
                Door.DATE_ORDERED,
                Objects.requireNonNull(order, "order").code(),
                utf8,
                null,
                options,
                null,
                NativeCore::date);
    }

    /**
     * See {@link #date(String, DateOrder, DateOptions, MonthNames)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param order the declared field order
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @param months the declared month names
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(
            byte[] utf8, DateOrder order, DateOptions options, MonthNames months) {
        return dated(
                Door.DATE_ORDERED,
                Objects.requireNonNull(order, "order").code(),
                utf8,
                null,
                options,
                Objects.requireNonNull(months, "months"),
                NativeCore::date);
    }

    /**
     * See {@link #date(String, DateOrder)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(MemorySegment utf8, DateOrder order) {
        return declared(
                Door.DATE_ORDERED, utf8, Objects.requireNonNull(order, "order").code(), NativeCore::date);
    }

    /**
     * See {@link #date(String, DateOrder, DateOptions)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param order the declared field order
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(MemorySegment utf8, DateOrder order, DateOptions options) {
        return dated(
                Door.DATE_ORDERED,
                Objects.requireNonNull(order, "order").code(),
                utf8,
                null,
                options,
                null,
                NativeCore::date);
    }

    /**
     * See {@link #date(String, DateOrder, DateOptions, MonthNames)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param order the declared field order
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @param months the declared month names
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDate> date(
            MemorySegment utf8, DateOrder order, DateOptions options, MonthNames months) {
        return dated(
                Door.DATE_ORDERED,
                Objects.requireNonNull(order, "order").code(),
                utf8,
                null,
                options,
                Objects.requireNonNull(months, "months"),
                NativeCore::date);
    }

    // --- civil date-times ---

    /**
     * Casts a zone-less civil date-time under a declared field order, as
     * {@link Cast#dateTime(String, DateOrder)} does.
     *
     * @param text the text to cast
     * @param order the declared field order
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(String text, DateOrder order) {
        byte[] utf8 = Cast.utf8(text);
        return chars(dateTime(utf8, order), text, utf8);
    }

    /**
     * Casts a zone-less civil date-time under a declared field order and designators, as
     * {@link Cast#dateTime(String, DateTimeFormat)} does.
     *
     * @param text the text to cast
     * @param format the declared field order and designators
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(String text, DateTimeFormat format) {
        byte[] utf8 = Cast.utf8(text);
        return chars(dateTime(utf8, format), text, utf8);
    }

    /**
     * See {@link #dateTime(String, DateTimeFormat)}, with the notation the locale's, as
     * {@link DateTimeFormat#from(Locale)} derives it.
     *
     * @param text the text to cast
     * @param locale the locale whose date formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<LocalDateTime> dateTime(String text, Locale locale) {
        return dateTime(text, DateTimeFormat.from(locale));
    }

    /**
     * Casts a zone-less civil date-time under a declared field order and opt-in shapes: the
     * {@link DateOptions#pivot() pivot} reads a two-digit year, and the other options are
     * ignored at this door. Otherwise as {@link #dateTime(String, DateOrder)}.
     *
     * @param text the text to cast
     * @param order the declared field order
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(String text, DateOrder order, DateOptions options) {
        byte[] utf8 = Cast.utf8(text);
        return chars(dateTime(utf8, order, options), text, utf8);
    }

    /**
     * Casts a zone-less civil date-time under a declared field order, opt-in shapes and a
     * {@link MonthNames month vocabulary}, as {@link #date(String, DateOrder, DateOptions, MonthNames)}
     * reads a date. Otherwise as {@link #dateTime(String, DateOrder, DateOptions)}.
     *
     * @param text the text to cast
     * @param order the declared field order
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @param months the declared month names
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(
            String text, DateOrder order, DateOptions options, MonthNames months) {
        byte[] utf8 = Cast.utf8(text);
        return chars(dateTime(utf8, order, options, months), text, utf8);
    }

    /**
     * Casts a zone-less civil date-time under a declared field order and designators and opt-in shapes: the
     * {@link DateOptions#pivot() pivot} reads a two-digit year, and the other options are
     * ignored at this door. Otherwise as {@link #dateTime(String, DateTimeFormat)}.
     *
     * @param text the text to cast
     * @param format the declared field order and designators
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(String text, DateTimeFormat format, DateOptions options) {
        byte[] utf8 = Cast.utf8(text);
        return chars(dateTime(utf8, format, options), text, utf8);
    }

    /**
     * Casts a zone-less civil date-time under a declared field order and designators, opt-in shapes and a
     * {@link MonthNames month vocabulary}, as {@link #date(String, DateOrder, DateOptions, MonthNames)}
     * reads a date. Otherwise as {@link #dateTime(String, DateTimeFormat, DateOptions)}.
     *
     * @param text the text to cast
     * @param format the declared field order and designators
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @param months the declared month names
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(
            String text, DateTimeFormat format, DateOptions options, MonthNames months) {
        byte[] utf8 = Cast.utf8(text);
        return chars(dateTime(utf8, format, options, months), text, utf8);
    }

    /**
     * See {@link #dateTime(String, DateOrder)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(byte[] utf8, DateOrder order) {
        return dated(
                Door.DATETIME,
                Objects.requireNonNull(order, "order").code(),
                utf8,
                null,
                DateOptions.NONE,
                null,
                NativeCore::dateTime);
    }

    /**
     * See {@link #dateTime(String, DateTimeFormat)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the declared field order and designators
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(byte[] utf8, DateTimeFormat format) {
        return dated(
                Door.DATETIME,
                Objects.requireNonNull(format, "format").order().code(),
                utf8,
                format,
                DateOptions.NONE,
                null,
                NativeCore::dateTime);
    }

    /**
     * See {@link #dateTime(byte[], DateTimeFormat)}, with the notation the locale's, as
     * {@link DateTimeFormat#from(Locale)} derives it.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param locale the locale whose date formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<LocalDateTime> dateTime(byte[] utf8, Locale locale) {
        return dateTime(utf8, DateTimeFormat.from(locale));
    }

    /**
     * See {@link #dateTime(String, DateOrder, DateOptions)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param order the declared field order
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(byte[] utf8, DateOrder order, DateOptions options) {
        return dated(
                Door.DATETIME,
                Objects.requireNonNull(order, "order").code(),
                utf8,
                null,
                options,
                null,
                NativeCore::dateTime);
    }

    /**
     * See {@link #dateTime(String, DateOrder, DateOptions, MonthNames)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param order the declared field order
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @param months the declared month names
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(
            byte[] utf8, DateOrder order, DateOptions options, MonthNames months) {
        return dated(
                Door.DATETIME,
                Objects.requireNonNull(order, "order").code(),
                utf8,
                null,
                options,
                Objects.requireNonNull(months, "months"),
                NativeCore::dateTime);
    }

    /**
     * See {@link #dateTime(String, DateTimeFormat, DateOptions)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the declared field order and designators
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(byte[] utf8, DateTimeFormat format, DateOptions options) {
        return dated(
                Door.DATETIME,
                Objects.requireNonNull(format, "format").order().code(),
                utf8,
                format,
                options,
                null,
                NativeCore::dateTime);
    }

    /**
     * See {@link #dateTime(String, DateTimeFormat, DateOptions, MonthNames)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the declared field order and designators
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @param months the declared month names
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(
            byte[] utf8, DateTimeFormat format, DateOptions options, MonthNames months) {
        return dated(
                Door.DATETIME,
                Objects.requireNonNull(format, "format").order().code(),
                utf8,
                format,
                options,
                Objects.requireNonNull(months, "months"),
                NativeCore::dateTime);
    }

    /**
     * See {@link #dateTime(String, DateOrder)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param order the declared field order
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(MemorySegment utf8, DateOrder order) {
        return dated(
                Door.DATETIME,
                Objects.requireNonNull(order, "order").code(),
                utf8,
                null,
                DateOptions.NONE,
                null,
                NativeCore::dateTime);
    }

    /**
     * See {@link #dateTime(String, DateTimeFormat)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the declared field order and designators
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(MemorySegment utf8, DateTimeFormat format) {
        return dated(
                Door.DATETIME,
                Objects.requireNonNull(format, "format").order().code(),
                utf8,
                format,
                DateOptions.NONE,
                null,
                NativeCore::dateTime);
    }

    /**
     * See {@link #dateTime(MemorySegment, DateTimeFormat)}, with the notation the locale's, as
     * {@link DateTimeFormat#from(Locale)} derives it.
     *
     * @param utf8 the UTF-8 input bytes
     * @param locale the locale whose date formatting declares the notation
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     * @throws NullPointerException if {@code locale} is null
     */
    public static DetailedVerdict<LocalDateTime> dateTime(MemorySegment utf8, Locale locale) {
        return dateTime(utf8, DateTimeFormat.from(locale));
    }

    /**
     * See {@link #dateTime(String, DateOrder, DateOptions)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param order the declared field order
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(MemorySegment utf8, DateOrder order, DateOptions options) {
        return dated(
                Door.DATETIME,
                Objects.requireNonNull(order, "order").code(),
                utf8,
                null,
                options,
                null,
                NativeCore::dateTime);
    }

    /**
     * See {@link #dateTime(String, DateOrder, DateOptions, MonthNames)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param order the declared field order
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @param months the declared month names
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(
            MemorySegment utf8, DateOrder order, DateOptions options, MonthNames months) {
        return dated(
                Door.DATETIME,
                Objects.requireNonNull(order, "order").code(),
                utf8,
                null,
                options,
                Objects.requireNonNull(months, "months"),
                NativeCore::dateTime);
    }

    /**
     * See {@link #dateTime(String, DateTimeFormat, DateOptions)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the declared field order and designators
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(
            MemorySegment utf8, DateTimeFormat format, DateOptions options) {
        return dated(
                Door.DATETIME,
                Objects.requireNonNull(format, "format").order().code(),
                utf8,
                format,
                options,
                null,
                NativeCore::dateTime);
    }

    /**
     * See {@link #dateTime(String, DateTimeFormat, DateOptions, MonthNames)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the declared field order and designators
     * @param options the opt-in shapes; {@link DateOptions#NONE} for none
     * @param months the declared month names
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> dateTime(
            MemorySegment utf8, DateTimeFormat format, DateOptions options, MonthNames months) {
        return dated(
                Door.DATETIME,
                Objects.requireNonNull(format, "format").order().code(),
                utf8,
                format,
                options,
                Objects.requireNonNull(months, "months"),
                NativeCore::dateTime);
    }

    // --- time ---

    /**
     * Casts an ISO 8601 24-hour time of day, as {@link Cast#time(String)} does.
     *
     * @param text the text to cast
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalTime> time(String text) {
        byte[] utf8 = Cast.utf8(text);
        return chars(time(utf8), text, utf8);
    }

    /**
     * See {@link #time(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalTime> time(byte[] utf8) {
        return plain(Door.TIME, utf8, NativeCore::time);
    }

    /**
     * See {@link #time(String)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalTime> time(MemorySegment utf8) {
        return plain(Door.TIME, utf8, NativeCore::time);
    }

    // --- duration ---

    /**
     * Casts a duration, as {@link Cast#duration(String)} does.
     *
     * @param text the text to cast
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Duration> duration(String text) {
        byte[] utf8 = Cast.utf8(text);
        return chars(duration(utf8), text, utf8);
    }

    /**
     * See {@link #duration(String)}; input as raw UTF-8 bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Duration> duration(byte[] utf8) {
        return plain(Door.DURATION, utf8, NativeCore::duration);
    }

    /**
     * See {@link #duration(String)}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Duration> duration(MemorySegment utf8) {
        return plain(Door.DURATION, utf8, NativeCore::duration);
    }

    // --- typed doors: a number the caller already holds ---

    /**
     * Reads a {@code double} as the exact decimal it names, as {@link Cast#decimalFromDouble(double)} does.
     *
     * @param value the number to read
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<BigDecimal> decimalFromDouble(double value) {
        return typed(Door.DECIMAL_FROM_F64, value, NativeCore::decimal);
    }

    /**
     * Reads an Excel date serial held as a {@code double}, as {@link Cast#excelSerialFromDouble(double, ExcelEpoch)} does.
     *
     * @param value the serial
     * @param epoch the declared date system
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalDateTime> excelSerialFromDouble(double value, ExcelEpoch epoch) {
        int code = Objects.requireNonNull(epoch, "epoch").code();
        NativeCore.Scratch scratch = NativeCore.scratch();
        return verdict(
                NativeCore.typedDeclared(scratch, Door.EXCEL_SERIAL_FROM_F64, value, code),
                scratch,
                NativeCore::dateTime);
    }

    /**
     * Reads the fraction of an Excel serial held as a {@code double}, as {@link Cast#excelTime(double)} does.
     *
     * @param value the serial
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<LocalTime> excelTime(double value) {
        return typed(Door.EXCEL_TIME, value, NativeCore::time);
    }

    /**
     * Reads a number of days held as a {@code double} as a {@link Duration}, as {@link Cast#excelDuration(double)} does.
     *
     * @param value the number of days
     * @return the verdict: a {@link DetailedSuccess} carrying the cast value, or a {@link DetailedFault}
     */
    public static DetailedVerdict<Duration> excelDuration(double value) {
        return typed(Door.EXCEL_DURATION, value, NativeCore::duration);
    }

    // --- the checked doors: a door judged by declared validation constraints ---

    /** The core's verdict on a checked door: the value and its check, a violation, or a fault. */
    private static <T> DetailedCheckedVerdict<T> judged(int code, NativeCore.Scratch scratch, Reader<T> reader) {
        return switch (code) {
            case 0 -> new DetailedChecked<>(reader.read(scratch), NativeCore.checkAdjusted(scratch), index(scratch));
            case 4 ->
                new DetailedViolation<>(
                        new Keyword(NativeCore.checkKeyword(scratch)),
                        NativeCore.faultOffset(scratch),
                        NativeCore.faultLength(scratch));
            default -> fault(code, scratch);
        };
    }

    // The text entry matched, or empty: the core's UINT32_MAX, an integer enum's included.
    private static OptionalInt index(NativeCore.Scratch scratch) {
        int index = NativeCore.checkEnumIndex(scratch);
        return index == -1 ? OptionalInt.empty() : OptionalInt.of(index);
    }

    private static <T> DetailedCheckedVerdict<T> checked(
            int door, String name, byte[] utf8, NumFormat format, Constraints<T> constraints, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        Objects.requireNonNull(format, "format");
        Objects.requireNonNull(constraints, "constraints").require(door, name);
        NativeCore.Scratch scratch = NativeCore.scratch();
        return judged(NativeCore.checked(scratch, door, utf8, format, constraints), scratch, reader);
    }

    private static <T> DetailedCheckedVerdict<T> checked(
            int door, String name, MemorySegment utf8, NumFormat format, Constraints<T> constraints, Reader<T> reader) {
        Objects.requireNonNull(utf8, "utf8");
        Objects.requireNonNull(format, "format");
        Objects.requireNonNull(constraints, "constraints").require(door, name);
        NativeCore.Scratch scratch = NativeCore.scratch();
        return judged(NativeCore.checked(scratch, door, utf8, format, constraints), scratch, reader);
    }

    // Over a String: encoded WTF-8, so a lone surrogate reaches the core as the invalid
    // sequence it is, and every span re-expressed in chars.
    private static <T> DetailedCheckedVerdict<T> checked(
            int door, String name, String text, NumFormat format, Constraints<T> constraints, Reader<T> reader) {
        byte[] utf8 = Cast.wtf8(text);
        return checkedChars(checked(door, name, utf8, format, constraints, reader), text, utf8);
    }

    private static <T> DetailedCheckedVerdict<T> checkedFromDouble(
            int door, String name, double value, Constraints<T> constraints, Reader<T> reader) {
        Objects.requireNonNull(constraints, "constraints").require(door, name);
        NativeCore.Scratch scratch = NativeCore.scratch();
        return judged(NativeCore.checkedFromF64(scratch, door, value, constraints), scratch, reader);
    }

    // A checked verdict's span rebased to chars, as chars(DetailedVerdict, ...) rebases a fault's.
    private static <T> DetailedCheckedVerdict<T> checkedChars(
            DetailedCheckedVerdict<T> verdict, String text, byte[] utf8) {
        if (utf8.length == text.length()) {
            return verdict;
        }
        return switch (verdict) {
            case DetailedChecked<T> ok -> ok;
            case DetailedFault<T> fault -> {
                int start = Cast.charIndex(utf8, fault.offset());
                int end = Cast.charIndex(utf8, fault.offset() + fault.length());
                yield new DetailedFault<>(fault.reason(), fault.detail(), start, end - start);
            }
            case DetailedViolation<T> violation -> {
                int start = Cast.charIndex(utf8, violation.offset());
                int end = Cast.charIndex(utf8, violation.offset() + violation.length());
                yield new DetailedViolation<>(violation.keyword(), start, end - start);
            }
        };
    }

    /**
     * Casts numeric text to an 8-bit signed integer under the declared {@code format}, as
     * {@link Cast#i8(String, NumFormat, Constraints)} does, then judges it by {@code constraints}:
     * a {@link DetailedChecked}, or a {@link DetailedRejection}, the plain detailed door's own
     * {@link DetailedFault} (with its {@link FaultDetail}) or the {@link DetailedViolation} of a
     * declared rule, spanning the value in chars.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#i8()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Byte> i8(String text, NumFormat format, Constraints<Byte> constraints) {
        return checked(Constraints.I8, "i8", text, format, constraints, NativeCore::i8);
    }

    /**
     * See {@link #i8(String, NumFormat, Constraints)}; input as raw UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#i8()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Byte> i8(byte[] utf8, NumFormat format, Constraints<Byte> constraints) {
        return checked(Constraints.I8, "i8", utf8, format, constraints, NativeCore::i8);
    }

    /**
     * See {@link #i8(String, NumFormat, Constraints)}; input as a {@link MemorySegment} view of UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#i8()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Byte> i8(MemorySegment utf8, NumFormat format, Constraints<Byte> constraints) {
        return checked(Constraints.I8, "i8", utf8, format, constraints, NativeCore::i8);
    }

    /**
     * Judges a number a source already decoded at the {@code i8} door by {@code constraints}, as
     * {@link Cast#i8FromDouble(double, Constraints)} does. Every fault and violation has an empty span.
     *
     * @param value the number to judge
     * @param constraints the declaration, built by {@link Constraints#i8()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Byte> i8FromDouble(double value, Constraints<Byte> constraints) {
        return checkedFromDouble(Constraints.I8, "i8", value, constraints, NativeCore::i8);
    }

    /**
     * Casts numeric text to a 16-bit signed integer under the declared {@code format}, as
     * {@link Cast#i16(String, NumFormat, Constraints)} does, then judges it by {@code constraints}:
     * a {@link DetailedChecked}, or a {@link DetailedRejection}, the plain detailed door's own
     * {@link DetailedFault} (with its {@link FaultDetail}) or the {@link DetailedViolation} of a
     * declared rule, spanning the value in chars.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#i16()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Short> i16(String text, NumFormat format, Constraints<Short> constraints) {
        return checked(Constraints.I16, "i16", text, format, constraints, NativeCore::i16);
    }

    /**
     * See {@link #i16(String, NumFormat, Constraints)}; input as raw UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#i16()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Short> i16(byte[] utf8, NumFormat format, Constraints<Short> constraints) {
        return checked(Constraints.I16, "i16", utf8, format, constraints, NativeCore::i16);
    }

    /**
     * See {@link #i16(String, NumFormat, Constraints)}; input as a {@link MemorySegment} view of UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#i16()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Short> i16(
            MemorySegment utf8, NumFormat format, Constraints<Short> constraints) {
        return checked(Constraints.I16, "i16", utf8, format, constraints, NativeCore::i16);
    }

    /**
     * Judges a number a source already decoded at the {@code i16} door by {@code constraints}, as
     * {@link Cast#i16FromDouble(double, Constraints)} does. Every fault and violation has an empty span.
     *
     * @param value the number to judge
     * @param constraints the declaration, built by {@link Constraints#i16()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Short> i16FromDouble(double value, Constraints<Short> constraints) {
        return checkedFromDouble(Constraints.I16, "i16", value, constraints, NativeCore::i16);
    }

    /**
     * Casts numeric text to a 32-bit signed integer under the declared {@code format}, as
     * {@link Cast#i32(String, NumFormat, Constraints)} does, then judges it by {@code constraints}:
     * a {@link DetailedChecked}, or a {@link DetailedRejection}, the plain detailed door's own
     * {@link DetailedFault} (with its {@link FaultDetail}) or the {@link DetailedViolation} of a
     * declared rule, spanning the value in chars.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#i32()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Integer> i32(String text, NumFormat format, Constraints<Integer> constraints) {
        return checked(Constraints.I32, "i32", text, format, constraints, NativeCore::i32);
    }

    /**
     * See {@link #i32(String, NumFormat, Constraints)}; input as raw UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#i32()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Integer> i32(byte[] utf8, NumFormat format, Constraints<Integer> constraints) {
        return checked(Constraints.I32, "i32", utf8, format, constraints, NativeCore::i32);
    }

    /**
     * See {@link #i32(String, NumFormat, Constraints)}; input as a {@link MemorySegment} view of UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#i32()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Integer> i32(
            MemorySegment utf8, NumFormat format, Constraints<Integer> constraints) {
        return checked(Constraints.I32, "i32", utf8, format, constraints, NativeCore::i32);
    }

    /**
     * Judges a number a source already decoded at the {@code i32} door by {@code constraints}, as
     * {@link Cast#i32FromDouble(double, Constraints)} does. Every fault and violation has an empty span.
     *
     * @param value the number to judge
     * @param constraints the declaration, built by {@link Constraints#i32()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Integer> i32FromDouble(double value, Constraints<Integer> constraints) {
        return checkedFromDouble(Constraints.I32, "i32", value, constraints, NativeCore::i32);
    }

    /**
     * Casts numeric text to a 64-bit signed integer under the declared {@code format}, as
     * {@link Cast#i64(String, NumFormat, Constraints)} does, then judges it by {@code constraints}:
     * a {@link DetailedChecked}, or a {@link DetailedRejection}, the plain detailed door's own
     * {@link DetailedFault} (with its {@link FaultDetail}) or the {@link DetailedViolation} of a
     * declared rule, spanning the value in chars.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#i64()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Long> i64(String text, NumFormat format, Constraints<Long> constraints) {
        return checked(Constraints.I64, "i64", text, format, constraints, NativeCore::i64);
    }

    /**
     * See {@link #i64(String, NumFormat, Constraints)}; input as raw UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#i64()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Long> i64(byte[] utf8, NumFormat format, Constraints<Long> constraints) {
        return checked(Constraints.I64, "i64", utf8, format, constraints, NativeCore::i64);
    }

    /**
     * See {@link #i64(String, NumFormat, Constraints)}; input as a {@link MemorySegment} view of UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#i64()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Long> i64(
            MemorySegment utf8, NumFormat format, Constraints<Long> constraints) {
        return checked(Constraints.I64, "i64", utf8, format, constraints, NativeCore::i64);
    }

    /**
     * Judges a number a source already decoded at the {@code i64} door by {@code constraints}, as
     * {@link Cast#i64FromDouble(double, Constraints)} does. Every fault and violation has an empty span.
     *
     * @param value the number to judge
     * @param constraints the declaration, built by {@link Constraints#i64()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Long> i64FromDouble(double value, Constraints<Long> constraints) {
        return checkedFromDouble(Constraints.I64, "i64", value, constraints, NativeCore::i64);
    }

    /**
     * Casts numeric text to an 8-bit unsigned integer under the declared {@code format}, as
     * {@link Cast#u8(String, NumFormat, Constraints)} does, then judges it by {@code constraints}:
     * a {@link DetailedChecked}, or a {@link DetailedRejection}, the plain detailed door's own
     * {@link DetailedFault} (with its {@link FaultDetail}) or the {@link DetailedViolation} of a
     * declared rule, spanning the value in chars.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#u8()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Integer> u8(String text, NumFormat format, Constraints<Integer> constraints) {
        return checked(Constraints.U8, "u8", text, format, constraints, NativeCore::u8);
    }

    /**
     * See {@link #u8(String, NumFormat, Constraints)}; input as raw UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#u8()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Integer> u8(byte[] utf8, NumFormat format, Constraints<Integer> constraints) {
        return checked(Constraints.U8, "u8", utf8, format, constraints, NativeCore::u8);
    }

    /**
     * See {@link #u8(String, NumFormat, Constraints)}; input as a {@link MemorySegment} view of UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#u8()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Integer> u8(
            MemorySegment utf8, NumFormat format, Constraints<Integer> constraints) {
        return checked(Constraints.U8, "u8", utf8, format, constraints, NativeCore::u8);
    }

    /**
     * Judges a number a source already decoded at the {@code u8} door by {@code constraints}, as
     * {@link Cast#u8FromDouble(double, Constraints)} does. Every fault and violation has an empty span.
     *
     * @param value the number to judge
     * @param constraints the declaration, built by {@link Constraints#u8()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Integer> u8FromDouble(double value, Constraints<Integer> constraints) {
        return checkedFromDouble(Constraints.U8, "u8", value, constraints, NativeCore::u8);
    }

    /**
     * Casts numeric text to a 16-bit unsigned integer under the declared {@code format}, as
     * {@link Cast#u16(String, NumFormat, Constraints)} does, then judges it by {@code constraints}:
     * a {@link DetailedChecked}, or a {@link DetailedRejection}, the plain detailed door's own
     * {@link DetailedFault} (with its {@link FaultDetail}) or the {@link DetailedViolation} of a
     * declared rule, spanning the value in chars.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#u16()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Integer> u16(String text, NumFormat format, Constraints<Integer> constraints) {
        return checked(Constraints.U16, "u16", text, format, constraints, NativeCore::u16);
    }

    /**
     * See {@link #u16(String, NumFormat, Constraints)}; input as raw UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#u16()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Integer> u16(byte[] utf8, NumFormat format, Constraints<Integer> constraints) {
        return checked(Constraints.U16, "u16", utf8, format, constraints, NativeCore::u16);
    }

    /**
     * See {@link #u16(String, NumFormat, Constraints)}; input as a {@link MemorySegment} view of UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#u16()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Integer> u16(
            MemorySegment utf8, NumFormat format, Constraints<Integer> constraints) {
        return checked(Constraints.U16, "u16", utf8, format, constraints, NativeCore::u16);
    }

    /**
     * Judges a number a source already decoded at the {@code u16} door by {@code constraints}, as
     * {@link Cast#u16FromDouble(double, Constraints)} does. Every fault and violation has an empty span.
     *
     * @param value the number to judge
     * @param constraints the declaration, built by {@link Constraints#u16()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Integer> u16FromDouble(double value, Constraints<Integer> constraints) {
        return checkedFromDouble(Constraints.U16, "u16", value, constraints, NativeCore::u16);
    }

    /**
     * Casts numeric text to a 32-bit unsigned integer under the declared {@code format}, as
     * {@link Cast#u32(String, NumFormat, Constraints)} does, then judges it by {@code constraints}:
     * a {@link DetailedChecked}, or a {@link DetailedRejection}, the plain detailed door's own
     * {@link DetailedFault} (with its {@link FaultDetail}) or the {@link DetailedViolation} of a
     * declared rule, spanning the value in chars.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#u32()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Long> u32(String text, NumFormat format, Constraints<Long> constraints) {
        return checked(Constraints.U32, "u32", text, format, constraints, NativeCore::u32);
    }

    /**
     * See {@link #u32(String, NumFormat, Constraints)}; input as raw UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#u32()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Long> u32(byte[] utf8, NumFormat format, Constraints<Long> constraints) {
        return checked(Constraints.U32, "u32", utf8, format, constraints, NativeCore::u32);
    }

    /**
     * See {@link #u32(String, NumFormat, Constraints)}; input as a {@link MemorySegment} view of UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#u32()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Long> u32(
            MemorySegment utf8, NumFormat format, Constraints<Long> constraints) {
        return checked(Constraints.U32, "u32", utf8, format, constraints, NativeCore::u32);
    }

    /**
     * Judges a number a source already decoded at the {@code u32} door by {@code constraints}, as
     * {@link Cast#u32FromDouble(double, Constraints)} does. Every fault and violation has an empty span.
     *
     * @param value the number to judge
     * @param constraints the declaration, built by {@link Constraints#u32()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Long> u32FromDouble(double value, Constraints<Long> constraints) {
        return checkedFromDouble(Constraints.U32, "u32", value, constraints, NativeCore::u32);
    }

    /**
     * Casts numeric text to a 64-bit unsigned integer under the declared {@code format}, as
     * {@link Cast#u64(String, NumFormat, Constraints)} does, then judges it by {@code constraints}:
     * a {@link DetailedChecked}, or a {@link DetailedRejection}, the plain detailed door's own
     * {@link DetailedFault} (with its {@link FaultDetail}) or the {@link DetailedViolation} of a
     * declared rule, spanning the value in chars.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#u64()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Long> u64(String text, NumFormat format, Constraints<Long> constraints) {
        return checked(Constraints.U64, "u64", text, format, constraints, NativeCore::u64);
    }

    /**
     * See {@link #u64(String, NumFormat, Constraints)}; input as raw UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#u64()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Long> u64(byte[] utf8, NumFormat format, Constraints<Long> constraints) {
        return checked(Constraints.U64, "u64", utf8, format, constraints, NativeCore::u64);
    }

    /**
     * See {@link #u64(String, NumFormat, Constraints)}; input as a {@link MemorySegment} view of UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#u64()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Long> u64(
            MemorySegment utf8, NumFormat format, Constraints<Long> constraints) {
        return checked(Constraints.U64, "u64", utf8, format, constraints, NativeCore::u64);
    }

    /**
     * Judges a number a source already decoded at the {@code u64} door by {@code constraints}, as
     * {@link Cast#u64FromDouble(double, Constraints)} does. Every fault and violation has an empty span.
     *
     * @param value the number to judge
     * @param constraints the declaration, built by {@link Constraints#u64()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Long> u64FromDouble(double value, Constraints<Long> constraints) {
        return checkedFromDouble(Constraints.U64, "u64", value, constraints, NativeCore::u64);
    }

    /**
     * Casts numeric text to a 32-bit float under the declared {@code format}, as
     * {@link Cast#f32(String, NumFormat, Constraints)} does, then judges it by {@code constraints}:
     * a {@link DetailedChecked}, or a {@link DetailedRejection}, the plain detailed door's own
     * {@link DetailedFault} (with its {@link FaultDetail}) or the {@link DetailedViolation} of a
     * declared rule, spanning the value in chars.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#f32()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Float> f32(String text, NumFormat format, Constraints<Float> constraints) {
        return checked(Constraints.F32, "f32", text, format, constraints, NativeCore::f32);
    }

    /**
     * See {@link #f32(String, NumFormat, Constraints)}; input as raw UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#f32()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Float> f32(byte[] utf8, NumFormat format, Constraints<Float> constraints) {
        return checked(Constraints.F32, "f32", utf8, format, constraints, NativeCore::f32);
    }

    /**
     * See {@link #f32(String, NumFormat, Constraints)}; input as a {@link MemorySegment} view of UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#f32()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Float> f32(
            MemorySegment utf8, NumFormat format, Constraints<Float> constraints) {
        return checked(Constraints.F32, "f32", utf8, format, constraints, NativeCore::f32);
    }

    /**
     * Judges a number a source already decoded at the {@code f32} door by {@code constraints}, as
     * {@link Cast#f32FromDouble(double, Constraints)} does. Every fault and violation has an empty span.
     *
     * @param value the number to judge
     * @param constraints the declaration, built by {@link Constraints#f32()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Float> f32FromDouble(double value, Constraints<Float> constraints) {
        return checkedFromDouble(Constraints.F32, "f32", value, constraints, NativeCore::f32);
    }

    /**
     * Casts numeric text to a 64-bit float under the declared {@code format}, as
     * {@link Cast#f64(String, NumFormat, Constraints)} does, then judges it by {@code constraints}:
     * a {@link DetailedChecked}, or a {@link DetailedRejection}, the plain detailed door's own
     * {@link DetailedFault} (with its {@link FaultDetail}) or the {@link DetailedViolation} of a
     * declared rule, spanning the value in chars.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#f64()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Double> f64(String text, NumFormat format, Constraints<Double> constraints) {
        return checked(Constraints.F64, "f64", text, format, constraints, NativeCore::f64);
    }

    /**
     * See {@link #f64(String, NumFormat, Constraints)}; input as raw UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#f64()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Double> f64(byte[] utf8, NumFormat format, Constraints<Double> constraints) {
        return checked(Constraints.F64, "f64", utf8, format, constraints, NativeCore::f64);
    }

    /**
     * See {@link #f64(String, NumFormat, Constraints)}; input as a {@link MemorySegment} view of UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#f64()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Double> f64(
            MemorySegment utf8, NumFormat format, Constraints<Double> constraints) {
        return checked(Constraints.F64, "f64", utf8, format, constraints, NativeCore::f64);
    }

    /**
     * Judges a number a source already decoded at the {@code f64} door by {@code constraints}, as
     * {@link Cast#f64FromDouble(double, Constraints)} does. Every fault and violation has an empty span.
     *
     * @param value the number to judge
     * @param constraints the declaration, built by {@link Constraints#f64()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<Double> f64FromDouble(double value, Constraints<Double> constraints) {
        return checkedFromDouble(Constraints.F64, "f64", value, constraints, NativeCore::f64);
    }

    /**
     * Casts numeric text to an exact decimal under the declared {@code format}, as
     * {@link Cast#decimal(String, NumFormat, Constraints)} does, then judges it by {@code constraints}:
     * a {@link DetailedChecked}, or a {@link DetailedRejection}, the plain detailed door's own
     * {@link DetailedFault} (with its {@link FaultDetail}) or the {@link DetailedViolation} of a
     * declared rule, spanning the value in chars.
     *
     * @param text the text to cast
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#decimal()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<BigDecimal> decimal(
            String text, NumFormat format, Constraints<BigDecimal> constraints) {
        return checked(Constraints.DECIMAL, "decimal", text, format, constraints, NativeCore::decimal);
    }

    /**
     * See {@link #decimal(String, NumFormat, Constraints)}; input as raw UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#decimal()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<BigDecimal> decimal(
            byte[] utf8, NumFormat format, Constraints<BigDecimal> constraints) {
        return checked(Constraints.DECIMAL, "decimal", utf8, format, constraints, NativeCore::decimal);
    }

    /**
     * See {@link #decimal(String, NumFormat, Constraints)}; input as a {@link MemorySegment} view of UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param format the caller-declared numeric notation
     * @param constraints the declaration, built by {@link Constraints#decimal()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<BigDecimal> decimal(
            MemorySegment utf8, NumFormat format, Constraints<BigDecimal> constraints) {
        return checked(Constraints.DECIMAL, "decimal", utf8, format, constraints, NativeCore::decimal);
    }

    /**
     * Judges a number a source already decoded at the {@code decimal} door by {@code constraints}, as
     * {@link Cast#decimalFromDouble(double, Constraints)} does. Every fault and violation has an empty span.
     *
     * @param value the number to judge
     * @param constraints the declaration, built by {@link Constraints#decimal()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<BigDecimal> decimalFromDouble(
            double value, Constraints<BigDecimal> constraints) {
        return checkedFromDouble(Constraints.DECIMAL, "decimal", value, constraints, NativeCore::decimal);
    }

    // --- the text door ---

    // The text door's value from bytes the core accepted: the span, decoded.
    private static String spanned(byte[] utf8, NativeCore.Scratch scratch) {
        return new String(utf8, NativeCore.spanOffset(scratch), NativeCore.spanLength(scratch), StandardCharsets.UTF_8);
    }

    private static String spanned(MemorySegment utf8, NativeCore.Scratch scratch) {
        byte[] span = utf8.asSlice(NativeCore.spanOffset(scratch), NativeCore.spanLength(scratch))
                .toArray(ValueLayout.JAVA_BYTE);
        return new String(span, StandardCharsets.UTF_8);
    }

    // The span of the caller's own string: the string itself when it is all of it.
    private static String spanned(String text, byte[] utf8, NativeCore.Scratch scratch) {
        int offset = NativeCore.spanOffset(scratch);
        int start = Cast.charIndex(utf8, offset);
        int end = Cast.charIndex(utf8, offset + NativeCore.spanLength(scratch));
        return text.substring(start, end);
    }

    /**
     * Casts text through the text door, as {@link Cast#text(String)} does: valid UTF-8 that is
     * not empty, untrimmed. A lone surrogate is {@link FaultDetail#INVALID_UTF8}, spanning it in
     * chars; empty input is {@link FaultDetail#BLANK}.
     *
     * @param text the text to cast
     * @return the verdict: a {@link DetailedSuccess} carrying the text, or a {@link DetailedFault}
     */
    public static DetailedVerdict<String> text(String text) {
        byte[] utf8 = Cast.wtf8(text);
        NativeCore.Scratch scratch = NativeCore.scratch();
        int code = NativeCore.text(scratch, utf8);
        return code == 0
                ? new DetailedSuccess<>(spanned(text, utf8, scratch))
                : chars(DetailedCast.<String>fault(code, scratch), text, utf8);
    }

    /**
     * See {@link #text(String)}; input as raw UTF-8 bytes, decoded into the value.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the text, or a {@link DetailedFault}
     */
    public static DetailedVerdict<String> text(byte[] utf8) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        int code = NativeCore.text(scratch, utf8);
        return code == 0 ? new DetailedSuccess<>(spanned(utf8, scratch)) : fault(code, scratch);
    }

    /**
     * See {@link #text(byte[])}; input as a {@link MemorySegment} view of UTF-8 bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @return the verdict: a {@link DetailedSuccess} carrying the text, or a {@link DetailedFault}
     */
    public static DetailedVerdict<String> text(MemorySegment utf8) {
        Objects.requireNonNull(utf8, "utf8");
        NativeCore.Scratch scratch = NativeCore.scratch();
        int code = NativeCore.text(scratch, utf8);
        return code == 0 ? new DetailedSuccess<>(spanned(utf8, scratch)) : fault(code, scratch);
    }

    /**
     * Casts text through the text door, then judges it by {@code constraints}, as
     * {@link Cast#text(String, Constraints)} does. A violation spans the (trimmed) text, in chars.
     *
     * @param text the text to cast
     * @param constraints the declaration, built by {@link Constraints#text()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<String> text(String text, Constraints<String> constraints) {
        byte[] utf8 = Cast.wtf8(text);
        Objects.requireNonNull(constraints, "constraints").require(Constraints.TEXT, "text");
        NativeCore.Scratch scratch = NativeCore.scratch();
        int code = NativeCore.checked(scratch, Constraints.TEXT, utf8, null, constraints);
        return checkedChars(
                judged(code, scratch, s -> entryOr(constraints, s, () -> spanned(text, utf8, s))), text, utf8);
    }

    /**
     * See {@link #text(String, Constraints)}; input as raw UTF-8 bytes, spans in bytes.
     *
     * @param utf8 the raw UTF-8 input bytes
     * @param constraints the declaration, built by {@link Constraints#text()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<String> text(byte[] utf8, Constraints<String> constraints) {
        Objects.requireNonNull(utf8, "utf8");
        Objects.requireNonNull(constraints, "constraints").require(Constraints.TEXT, "text");
        NativeCore.Scratch scratch = NativeCore.scratch();
        int code = NativeCore.checked(scratch, Constraints.TEXT, utf8, null, constraints);
        return judged(code, scratch, s -> entryOr(constraints, s, () -> spanned(utf8, s)));
    }

    /**
     * See {@link #text(byte[], Constraints)}; input as a {@link MemorySegment} view of UTF-8
     * bytes.
     *
     * @param utf8 the UTF-8 input bytes
     * @param constraints the declaration, built by {@link Constraints#text()}
     * @return the verdict: a {@link DetailedChecked}, a {@link DetailedFault} or a {@link DetailedViolation}
     * @throws IllegalArgumentException {@code constraints} were built for another door
     */
    public static DetailedCheckedVerdict<String> text(MemorySegment utf8, Constraints<String> constraints) {
        Objects.requireNonNull(utf8, "utf8");
        Objects.requireNonNull(constraints, "constraints").require(Constraints.TEXT, "text");
        NativeCore.Scratch scratch = NativeCore.scratch();
        int code = NativeCore.checked(scratch, Constraints.TEXT, utf8, null, constraints);
        return judged(code, scratch, s -> entryOr(constraints, s, () -> spanned(utf8, s)));
    }

    // The declared entry an enum matched, the caller's own instance; otherwise the span.
    private static String entryOr(
            Constraints<String> constraints, NativeCore.Scratch scratch, java.util.function.Supplier<String> span) {
        int index = NativeCore.checkEnumIndex(scratch);
        return index == -1 ? span.get() : constraints.entry(index);
    }
}
