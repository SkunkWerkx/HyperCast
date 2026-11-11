package io.github.skunkwerkx.hypercast;

import io.github.skunkwerkx.hypercast.interop.NativePlatform;
import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.Arrays;
import java.util.UUID;

/**
 * The JNI path to the core: the JDK 21 base of the multi-release jar, used on 21 through 24
 * and wherever the {@code META-INF/versions/25} tree is not read. A library
 * ({@code libhypercast_jni}) holds the core inside it and registers the natives below when it
 * loads, one per C export; every call returns the core's verdict code raw, and {@link Cast}
 * builds the verdict. The JDK 25 tree has a class of the same name and the same members that
 * reaches the core through FFM instead.
 *
 * <p>Every door crosses through one of the four detailed exports ({@code cast_detailed} and its
 * three siblings), which alone take the grouping policy a {@link NumFormat} declares, date
 * options and a month vocabulary, and report what failed; the 1.0 natives below stay declared,
 * one per export, as {@code check_exports.py} holds every site to the whole ABI, and nothing
 * calls them. A library without the detailed natives is a 1.0 library, which would ignore a strict
 * grouping policy, and the count check in {@link Library} fails its load.
 *
 * <p>A verdict comes back through the thread's {@link Scratch}: the value as two
 * {@code long}s in {@code out[0..2]} on success (and from a checked door on a violation, the
 * value as cast), the fault's span in {@code out[2..4]} on a fault or a violation, from a
 * checked door its check in {@code out[4..7]}, and a fault's detail code in
 * {@code out[7]} (java/jni/src/lib.rs's {@code Words} and {@code check_words} have each
 * encoding, and the readers below are their other half). A checked door's {@code enum} list crosses as the direct buffer its
 * {@link Constraints} holds, so it is neither pinned nor copied. A {@code byte[]} crosses pinned for the core call alone when it
 * is at most 64 KiB, and copied into native memory above that; a {@link MemorySegment} is
 * copied into a {@code byte[]} first, since JNI reaches no segment. Nothing here touches
 * {@code java.lang.foreign} on the way to a {@code byte[]} or {@link String} door.
 */
final class NativeCore {
    private NativeCore() {}

    /** The library's base name: {@code libhypercast_jni.so}, {@code hypercast_jni.dll}. */
    private static final String LIBRARY = "hypercast_jni";

    // The natives, registered by JNI_OnLoad on this very class. Their names and descriptors
    // are the library's table, java/jni/src/lib.rs. Reached only through the members below,
    // which touch Library first.
    private static native int castBool(byte[] in, long[] out);

    private static native int castChar(byte[] in, long[] out);

    private static native int castI8(byte[] in, byte[] format, long[] out);

    private static native int castI16(byte[] in, byte[] format, long[] out);

    private static native int castI32(byte[] in, byte[] format, long[] out);

    private static native int castI64(byte[] in, byte[] format, long[] out);

    private static native int castU8(byte[] in, byte[] format, long[] out);

    private static native int castU16(byte[] in, byte[] format, long[] out);

    private static native int castU32(byte[] in, byte[] format, long[] out);

    private static native int castU64(byte[] in, byte[] format, long[] out);

    private static native int castF32(byte[] in, byte[] format, long[] out);

    private static native int castF64(byte[] in, byte[] format, long[] out);

    private static native int castDecimal(byte[] in, byte[] format, long[] out);

    private static native int castUuid(byte[] in, long[] out);

    private static native int castTimestamp(byte[] in, long[] out);

    private static native int castUnix(byte[] in, int precision, long[] out);

    private static native int castExcelSerial(byte[] in, int epoch, long[] out);

    private static native int castDateOrdered(byte[] in, int order, long[] out);

    private static native int castDatetime(byte[] in, int order, byte[] periods, long[] out);

    private static native int castDate(byte[] in, long[] out);

    private static native int castTime(byte[] in, long[] out);

    private static native int castDuration(byte[] in, long[] out);

    private static native int castDecimalFromF64(double value, long[] out);

    private static native int castExcelSerialFromF64(double value, int epoch, long[] out);

    private static native int castExcelTime(double value, long[] out);

    private static native int castExcelDuration(double value, long[] out);

    private static native int hypercastVersion();

    private static native int castChecked(
            int door, byte[] format, byte[] constraints, ByteBuffer entries, byte[] in, long[] out);

    private static native int castCheckedFromF64(
            int door, double value, byte[] constraints, ByteBuffer entries, long[] out);

    private static native int castConstraintsCheck(int door, byte[] constraints, ByteBuffer entries);

    private static native int castText(byte[] in, long[] out);

    private static native int castDeclaredDecimal(byte[] in, long[] out);

    private static native int castDetailed(
            int door,
            int param,
            byte[] format,
            byte[] periods,
            byte[] dateOptions,
            ByteBuffer vocabulary,
            byte[] in,
            long[] out);

    private static native int castDetailedFromF64(int door, int param, double value, long[] out);

    private static native int castCheckedDetailed(
            int door, byte[] format, byte[] constraints, ByteBuffer entries, byte[] in, long[] out);

    private static native int castCheckedDetailedFromF64(
            int door, double value, byte[] constraints, ByteBuffer entries, long[] out);

    private static native int jniNatives();

    /**
     * The library, loaded once in this holder's class init. A platform this jar ships no build
     * for, or a library that will not load (a temp directory mounted noexec, say), fails this
     * class's init with the reason, and {@link Cast#isAvailable()} says so. A library whose
     * natives are not the ones this class declares fails the same way, as a 1.0 library does: it
     * implements 33 of the 37. Nothing loads until a
     * member below needs the core: every one touches this holder first.
     */
    private static final class Library {
        private Library() {}

        static {
            load(NativePlatform.current(LIBRARY));
            int implemented = jniNatives();
            int declared = 0;
            for (Method method : NativeCore.class.getDeclaredMethods()) {
                if (Modifier.isNative(method.getModifiers())) {
                    declared++;
                }
            }
            if (implemented != declared) {
                throw new IllegalStateException(
                        "libhypercast_jni implements " + implemented + " natives; this binding declares " + declared);
            }
        }

        /** Forces the load. The call itself is the point; there is nothing to return. */
        static void ensure() {}

        private static void load(NativePlatform.Target target) {
            if (target == null) {
                // The same words as the FFM path's: no build of either library exists here.
                throw new IllegalStateException(NativePlatform.missing("hypercast", null));
            }
            try (InputStream resource = NativeCore.class.getResourceAsStream(target.resourcePath())) {
                if (resource == null) {
                    throw new IllegalStateException(NativePlatform.missing(LIBRARY, target));
                }
                String libraryFileName = target.libraryFileName();
                String extension = libraryFileName.substring(libraryFileName.lastIndexOf('.'));
                Path tmp = Files.createTempFile(LIBRARY, extension);
                tmp.toFile().deleteOnExit();
                Files.copy(resource, tmp, StandardCopyOption.REPLACE_EXISTING);
                // JNI_OnLoad registers the natives here; a table that does not match the
                // declarations above surfaces as the NoSuchMethodError it left pending.
                System.load(tmp.toAbsolutePath().toString());
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
        }
    }

    /** Which path reached the core, for the diagnostics; loads nothing. */
    static String backend() {
        return "jni";
    }

    /**
     * Per-thread scratch: the {@code long}s a native writes its verdict into, and the
     * format and designator layouts as the library reads them, each kept under an identity
     * memo the way the FFM tree keeps its segments. Doors never nest, so no call can observe
     * another's scratch mid-flight; nothing is shared between threads, so no door needs
     * locking.
     */
    static final class Scratch {
        // out[0..2] the value (a checked door's on a violation too), out[2..4] a fault's span,
        // out[4..7] a checked door's check, out[7] a fault's detail code.
        final long[] out = new long[8];
        private final byte[] format = new byte[FORMAT_BYTES];
        private NumFormat formatKey;
        private final byte[] periods = new byte[PERIODS_BYTES];
        private DateTimeFormat periodsKey;
        private final byte[] dateOptions = new byte[DATE_OPTIONS_BYTES];
        private DateOptions dateOptionsKey;

        private Scratch() {}

        byte[] format(NumFormat declared) {
            // Formats are reused constants in practice, so an identity check skips the
            // encode on the overwhelming majority of calls.
            if (formatKey != declared) {
                packFormat(declared, format);
                formatKey = declared;
            }
            return format;
        }

        // null when nothing is declared — the DateOrder overloads, or a DateTimeFormat with no
        // designators — which the library hands the core as its null.
        byte[] periods(DateTimeFormat declared) {
            if (declared == null || declared.englishOnly()) {
                return null;
            }
            if (periodsKey != declared) {
                packPeriods(declared, periods);
                periodsKey = declared;
            }
            return periods;
        }

        // null when nothing is declared, which the library hands the core as its null.
        byte[] dateOptions(DateOptions declared) {
            if (declared == null || declared.isNone()) {
                return null;
            }
            if (dateOptionsKey != declared) {
                packDateOptions(declared, dateOptions);
                dateOptionsKey = declared;
            }
            return dateOptions;
        }
    }

    private static final ThreadLocal<Scratch> SCRATCH = ThreadLocal.withInitial(Scratch::new);

    /** This thread's scratch. */
    static Scratch scratch() {
        return SCRATCH.get();
    }

    /** The core's packed version, {@code major << 16 | minor << 8 | patch}. */
    static int coreVersion() {
        Library.ensure();
        return hypercastVersion();
    }

    // --- calls: each returns the core's code raw ---

    // Every text door is cast_detailed: `door` is the core's code and `param` what it declares.
    // A layout that is not declared crosses as null, which the library hands the core as its
    // null; the core reads each declaration only at the doors it is for.
    private static int detailed(
            Scratch s,
            int door,
            int param,
            byte[] format,
            byte[] periods,
            byte[] dateOptions,
            ByteBuffer vocabulary,
            byte[] in) {
        Library.ensure();
        return castDetailed(door, param, format, periods, dateOptions, vocabulary, in, s.out);
    }

    /** A text door with nothing declared beside the text. */
    static int plain(Scratch s, int door, byte[] in) {
        return detailed(s, door, 0, null, null, null, null, in);
    }

    static int plain(Scratch s, int door, MemorySegment in) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return plain(s, door, bytes(in));
    }

    /** A numeric door, under the declared format. */
    static int numeric(Scratch s, int door, byte[] in, NumFormat format) {
        Library.ensure();
        return detailed(s, door, 0, s.format(format), null, null, null, in);
    }

    static int numeric(Scratch s, int door, MemorySegment in, NumFormat format) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return numeric(s, door, bytes(in), format);
    }

    /** A text door with one declared code: the Unix unit, the Excel epoch, the date order. */
    static int declared(Scratch s, int door, byte[] in, int code) {
        return detailed(s, door, code, null, null, null, null, in);
    }

    static int declared(Scratch s, int door, MemorySegment in, int code) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return declared(s, door, bytes(in), code);
    }

    /** The civil date-time door: the order, and the designators or {@code null} for none. */
    static int civil(Scratch s, byte[] in, int order, DateTimeFormat format) {
        Library.ensure();
        return detailed(s, Door.DATETIME, order, null, s.periods(format), null, null, in);
    }

    static int civil(Scratch s, MemorySegment in, int order, DateTimeFormat format) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return civil(s, bytes(in), order, format);
    }

    /**
     * A date door under {@link DateOptions} and a month vocabulary, either {@code null} for
     * none: the timestamp, date, ordered date and civil date-time doors. {@code param} is the
     * order at the last two; {@code format} the designators at the civil door only.
     */
    static int dated(
            Scratch s, int door, int param, byte[] in, DateTimeFormat format, DateOptions options, MonthNames months) {
        Library.ensure();
        return detailed(
                s,
                door,
                param,
                null,
                s.periods(format),
                s.dateOptions(options),
                months == null ? null : months.vocabulary(),
                in);
    }

    static int dated(
            Scratch s,
            int door,
            int param,
            MemorySegment in,
            DateTimeFormat format,
            DateOptions options,
            MonthNames months) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return dated(s, door, param, bytes(in), format, options, months);
    }

    /** A typed door: a {@code double} the caller holds. */
    static int typed(Scratch s, int door, double value) {
        Library.ensure();
        return castDetailedFromF64(door, 0, value, s.out);
    }

    /** A typed door with one declared code: the Excel serial and its epoch. */
    static int typedDeclared(Scratch s, int door, double value, int code) {
        Library.ensure();
        return castDetailedFromF64(door, code, value, s.out);
    }

    /**
     * A checked door: {@code door} is the core's door code ({@link Constraints#I32},
     * {@link Constraints#TEXT}, ...), {@code format} the declared notation or {@code null} (the
     * text door reads none), {@code constraints} the declaration already bound to the door.
     */
    static int checked(Scratch s, int door, byte[] in, NumFormat format, Constraints<?> constraints) {
        Library.ensure();
        byte[] declared = format == null ? null : s.format(format);
        return castCheckedDetailed(door, declared, constraints.raw(), constraints.entries(), in, s.out);
    }

    static int checked(Scratch s, int door, MemorySegment in, NumFormat format, Constraints<?> constraints) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return checked(s, door, bytes(in), format, constraints);
    }

    /** A checked door over a {@code double} a source already decoded. */
    static int checkedFromF64(Scratch s, int door, double value, Constraints<?> constraints) {
        Library.ensure();
        return castCheckedDetailedFromF64(door, value, constraints.raw(), constraints.entries(), s.out);
    }

    /**
     * The core's judgement of a month vocabulary, 1 when it is whole and -1 when the core
     * refuses it: the vocabulary is judged before the (empty) input is read, so an acceptable
     * one reads nothing and answers {@code empty}.
     */
    static int monthNamesCheck(ByteBuffer vocabulary) {
        Library.ensure();
        return castDetailed(Door.DATE_ORDERED, 1, null, null, null, vocabulary, new byte[0], new long[8]);
    }

    /** The core's judgement of a declaration: 0, or the refusal code. */
    static int constraintsCheck(int door, byte[] raw, ByteBuffer entries) {
        Library.ensure();
        return castConstraintsCheck(door, raw, entries);
    }

    /** The text door: the span of valid UTF-8 input. */
    static int text(Scratch s, byte[] in) {
        return detailed(s, Door.TEXT, 0, null, null, null, null, in);
    }

    static int text(Scratch s, MemorySegment in) {
        if (tooLong(in)) {
            return tooLong(s);
        }
        return text(s, bytes(in));
    }

    /** The core's conversion of a declared decimal: 0 with the value, or 8. */
    static int declaredDecimal(Scratch s, byte[] utf8) {
        Library.ensure();
        return castDeclaredDecimal(utf8, s.out);
    }

    // Java's spans are ints, so a segment longer than Integer.MAX_VALUE bytes is out_of_range
    // (too_long) at every door, spanning the most a span holds, before the core is asked: no
    // value or fault it could report past there would fit, and nothing past there is read.
    private static boolean tooLong(MemorySegment in) {
        return in.byteSize() > Integer.MAX_VALUE;
    }

    private static int tooLong(Scratch s) {
        s.out[2] = 0;
        s.out[3] = Integer.MAX_VALUE;
        s.out[7] = TOO_LONG;
        return OUT_OF_RANGE;
    }

    private static final int OUT_OF_RANGE = 3;
    private static final int TOO_LONG = 21;

    // JNI reaches no segment, so its bytes cross as a heap copy.
    private static byte[] bytes(MemorySegment in) {
        return in.toArray(ValueLayout.JAVA_BYTE);
    }

    // --- readers: the value the last successful call on `s` produced, or its fault ---

    static boolean bool(Scratch s) {
        return s.out[0] != 0;
    }

    /** The Unicode scalar the char door read, {@code 0..0x10FFFF}. */
    static int scalar(Scratch s) {
        return (int) s.out[0];
    }

    static byte i8(Scratch s) {
        return (byte) s.out[0];
    }

    static short i16(Scratch s) {
        return (short) s.out[0];
    }

    static int i32(Scratch s) {
        return (int) s.out[0];
    }

    static long i64(Scratch s) {
        return s.out[0];
    }

    static int u8(Scratch s) {
        return (int) s.out[0];
    }

    static int u16(Scratch s) {
        return (int) s.out[0];
    }

    static long u32(Scratch s) {
        return s.out[0];
    }

    /** The two's-complement bit pattern, as Java has no unsigned 64-bit primitive. */
    static long u64(Scratch s) {
        return s.out[0];
    }

    static float f32(Scratch s) {
        return Float.intBitsToFloat((int) s.out[0]);
    }

    static double f64(Scratch s) {
        return Double.longBitsToDouble(s.out[0]);
    }

    static BigDecimal decimal(Scratch s) {
        // The 96-bit magnitude big-endian, high word first; the core never hands back a
        // negative zero, so the signum needs no zero check.
        long high = s.out[1];
        byte[] magnitude = new byte[12];
        ByteBuffer.wrap(magnitude).putInt((int) high).putLong(s.out[0]);
        int scale = (int) (high >>> 32) & 0xFF;
        int signum = ((high >>> 40) & 1) != 0 ? -1 : 1;
        return new BigDecimal(new BigInteger(signum, magnitude), scale);
    }

    static UUID uuid(Scratch s) {
        return new UUID(s.out[0], s.out[1]);
    }

    static Instant instant(Scratch s) {
        return Instant.ofEpochSecond(s.out[0], s.out[1]);
    }

    static LocalDate date(Scratch s) {
        return date(s.out[0]);
    }

    static LocalDateTime dateTime(Scratch s) {
        return LocalDateTime.of(date(s.out[0]), LocalTime.ofNanoOfDay(s.out[1]));
    }

    static LocalTime time(Scratch s) {
        return LocalTime.ofNanoOfDay(s.out[0]);
    }

    static Duration duration(Scratch s) {
        // Duration.ofSeconds normalizes the core's same-signed nanos adjustment correctly.
        return Duration.ofSeconds(s.out[0], s.out[1]);
    }

    /** The text door's span: the byte offset of the (trimmed) text. */
    static int spanOffset(Scratch s) {
        return (int) s.out[0];
    }

    /** The text door's span: the byte length of the (trimmed) text. */
    static int spanLength(Scratch s) {
        return (int) s.out[1];
    }

    /** A checked door's keyword code on a violation; 0 otherwise. */
    static int checkKeyword(Scratch s) {
        return (int) s.out[4];
    }

    /** Whether a checked door's rounding changed the value. */
    static boolean checkAdjusted(Scratch s) {
        return s.out[5] != 0;
    }

    /** The text {@code enum} entry a checked door matched, or -1 for none. */
    static int checkEnumIndex(Scratch s) {
        return (int) s.out[6];
    }

    static int faultOffset(Scratch s) {
        return (int) s.out[2];
    }

    static int faultLength(Scratch s) {
        return (int) s.out[3];
    }

    /** The detail code of the fault the last failed call wrote: 1 or more, or 0 for a violation. */
    static int faultDetail(Scratch s) {
        return (int) s.out[7];
    }

    // year << 16 | month << 8 | day
    private static LocalDate date(long packed) {
        return LocalDate.of((int) (packed >>> 16) & 0xFFFF, (int) (packed >>> 8) & 0xFF, (int) packed & 0xFF);
    }

    // --- the layouts the library copies onto its stack, in this platform's byte order ---

    /** {@code RawNumFormat}: the size word, the flags, the digit zero, then the symbols. */
    private static final int FORMAT_BYTES = 92;

    /** {@code RawDayPeriods}: the size word, then the two designators, then padding. */
    private static final int PERIODS_BYTES = 40;

    /** {@code RawDateOptions}: the size word, the option flags and the pivot. */
    private static final int DATE_OPTIONS_BYTES = 12;

    // The whole layout, zero past each symbol: the memo means a shorter symbol after a
    // longer one must not leave the tail behind. The format was validated when it was built.
    private static void packFormat(NumFormat format, byte[] into) {
        Arrays.fill(into, (byte) 0);
        ByteBuffer buffer = ByteBuffer.wrap(into).order(ByteOrder.nativeOrder());
        buffer.putInt(FORMAT_BYTES).putInt(format.flags()).putInt(format.digitZero());
        symbol(buffer, format.decimalSeparator(), 8);
        symbol(buffer, format.groupSeparator(), 8);
        symbol(buffer, format.currencyDecimalSeparator(), 8);
        symbol(buffer, format.currencyGroupSeparator(), 8);
        symbol(buffer, format.minusSign(), 8);
        symbol(buffer, format.plusSign(), 8);
        symbol(buffer, format.percentSymbol(), 8);
        symbol(buffer, format.currencySymbol(), 16);
    }

    private static void packDateOptions(DateOptions options, byte[] into) {
        ByteBuffer.wrap(into)
                .order(ByteOrder.nativeOrder())
                .putInt(DATE_OPTIONS_BYTES)
                .putInt(options.flags())
                .putInt(options.pivot());
    }

    private static void packPeriods(DateTimeFormat format, byte[] into) {
        Arrays.fill(into, (byte) 0);
        ByteBuffer buffer = ByteBuffer.wrap(into).order(ByteOrder.nativeOrder());
        buffer.putInt(PERIODS_BYTES);
        symbol(buffer, format.amDesignator(), 16);
        symbol(buffer, format.pmDesignator(), 16);
    }

    // One symbol field: its UTF-8 bytes in a field `capacity` bytes wide (already zeroed),
    // then its length.
    private static void symbol(ByteBuffer buffer, String symbol, int capacity) {
        byte[] utf8 = symbol.getBytes(StandardCharsets.UTF_8);
        int at = buffer.position();
        buffer.put(utf8).position(at + capacity).put((byte) utf8.length);
    }
}
