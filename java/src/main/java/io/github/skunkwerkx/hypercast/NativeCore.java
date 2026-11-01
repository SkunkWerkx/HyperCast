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
 * <p>A verdict comes back through the thread's {@link Scratch}: the value as two
 * {@code long}s in {@code out[0..2]} on success, the fault's span in {@code out[2..4]} on a
 * fault (java/jni/src/lib.rs's {@code Words} has each type's encoding, and the readers
 * below are its other half). A {@code byte[]} crosses pinned for the core call alone when it
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

    private static native int jniNatives();

    /**
     * The library, loaded once in this holder's class init. A platform this jar ships no build
     * for, or a library that will not load (a temp directory mounted noexec, say), fails this
     * class's init with the reason, and {@link Cast#isAvailable()} says so. A library whose
     * natives are not the ones this class declares fails the same way. Nothing loads until a
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
     * Per-thread scratch: the four {@code long}s a native writes its verdict into, and the
     * format and designator layouts as the library reads them, each kept under an identity
     * memo the way the FFM tree keeps its segments. Doors never nest, so no call can observe
     * another's scratch mid-flight; nothing is shared between threads, so no door needs
     * locking.
     */
    static final class Scratch {
        final long[] out = new long[4];
        private final byte[] format = new byte[FORMAT_BYTES];
        private NumFormat formatKey;
        private final byte[] periods = new byte[PERIODS_BYTES];
        private DateTimeFormat periodsKey;

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

    /** A text door with nothing declared beside the text. */
    static int plain(Scratch s, int door, byte[] in) {
        Library.ensure();
        long[] out = s.out;
        return switch (door) {
            case Door.BOOL -> castBool(in, out);
            case Door.CHAR -> castChar(in, out);
            case Door.UUID -> castUuid(in, out);
            case Door.TIMESTAMP -> castTimestamp(in, out);
            case Door.DATE -> castDate(in, out);
            case Door.TIME -> castTime(in, out);
            case Door.DURATION -> castDuration(in, out);
            default -> throw Door.unknown(door);
        };
    }

    static int plain(Scratch s, int door, MemorySegment in) {
        return plain(s, door, bytes(in));
    }

    /** A numeric door, under the declared format. */
    static int numeric(Scratch s, int door, byte[] in, NumFormat format) {
        Library.ensure();
        byte[] declared = s.format(format);
        long[] out = s.out;
        return switch (door) {
            case Door.I8 -> castI8(in, declared, out);
            case Door.I16 -> castI16(in, declared, out);
            case Door.I32 -> castI32(in, declared, out);
            case Door.I64 -> castI64(in, declared, out);
            case Door.U8 -> castU8(in, declared, out);
            case Door.U16 -> castU16(in, declared, out);
            case Door.U32 -> castU32(in, declared, out);
            case Door.U64 -> castU64(in, declared, out);
            case Door.F32 -> castF32(in, declared, out);
            case Door.F64 -> castF64(in, declared, out);
            case Door.DECIMAL -> castDecimal(in, declared, out);
            default -> throw Door.unknown(door);
        };
    }

    static int numeric(Scratch s, int door, MemorySegment in, NumFormat format) {
        return numeric(s, door, bytes(in), format);
    }

    /** A text door with one declared code: the Unix unit, the Excel epoch, the date order. */
    static int declared(Scratch s, int door, byte[] in, int code) {
        Library.ensure();
        long[] out = s.out;
        return switch (door) {
            case Door.UNIX -> castUnix(in, code, out);
            case Door.EXCEL_SERIAL -> castExcelSerial(in, code, out);
            case Door.DATE_ORDERED -> castDateOrdered(in, code, out);
            default -> throw Door.unknown(door);
        };
    }

    static int declared(Scratch s, int door, MemorySegment in, int code) {
        return declared(s, door, bytes(in), code);
    }

    /** The civil date-time door: the order, and the designators or {@code null} for none. */
    static int civil(Scratch s, byte[] in, int order, DateTimeFormat format) {
        Library.ensure();
        return castDatetime(in, order, s.periods(format), s.out);
    }

    static int civil(Scratch s, MemorySegment in, int order, DateTimeFormat format) {
        return civil(s, bytes(in), order, format);
    }

    /** A typed door: a {@code double} the caller holds. */
    static int typed(Scratch s, int door, double value) {
        Library.ensure();
        long[] out = s.out;
        return switch (door) {
            case Door.DECIMAL_FROM_F64 -> castDecimalFromF64(value, out);
            case Door.EXCEL_TIME -> castExcelTime(value, out);
            case Door.EXCEL_DURATION -> castExcelDuration(value, out);
            default -> throw Door.unknown(door);
        };
    }

    /** A typed door with one declared code: the Excel serial and its epoch. */
    static int typedDeclared(Scratch s, int door, double value, int code) {
        Library.ensure();
        if (door != Door.EXCEL_SERIAL_FROM_F64) {
            throw Door.unknown(door);
        }
        return castExcelSerialFromF64(value, code, s.out);
    }

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

    static int faultOffset(Scratch s) {
        return (int) s.out[2];
    }

    static int faultLength(Scratch s) {
        return (int) s.out[3];
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

    // The whole layout, zero past each symbol: the memo means a shorter symbol after a
    // longer one must not leave the tail behind. The format was validated when it was built.
    private static void packFormat(NumFormat format, byte[] into) {
        Arrays.fill(into, (byte) 0);
        ByteBuffer buffer = ByteBuffer.wrap(into).order(ByteOrder.nativeOrder());
        buffer.putInt(FORMAT_BYTES).putInt(format.styles()).putInt(format.digitZero());
        symbol(buffer, format.decimalSeparator(), 8);
        symbol(buffer, format.groupSeparator(), 8);
        symbol(buffer, format.currencyDecimalSeparator(), 8);
        symbol(buffer, format.currencyGroupSeparator(), 8);
        symbol(buffer, format.minusSign(), 8);
        symbol(buffer, format.plusSign(), 8);
        symbol(buffer, format.percentSymbol(), 8);
        symbol(buffer, format.currencySymbol(), 16);
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
