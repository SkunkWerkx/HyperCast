package io.github.skunkwerkx.hypercast.interop;

import io.github.skunkwerkx.hypercast.CastFailure;
import io.github.skunkwerkx.hypercast.Fault;
import io.github.skunkwerkx.hypercast.NumFormat;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.UUID;

/**
 * The native core's C ABI values, read and written: every out-value layout a door fills, the
 * 32-byte numeric format it reads, the verdict code and span of a fault, and the packed
 * version word. Public for a library that carries HyperCast's verdicts across a C ABI of its
 * own (HyperTabular does) — it reads these out of its own buffers through exactly the
 * conversions every {@link io.github.skunkwerkx.hypercast.Cast} door uses, so a value it hands
 * out is the value the door would have.
 *
 * <p>Each reader takes the segment and the byte offset of one value, which must be aligned
 * for the layout's widest field (8 bytes for the 16-byte values). The {@code *_BYTES}
 * constants are each layout's size, the stride of an array of them.
 */
public final class NativeValues {
    /** {@code {i64 seconds, i32 nanos}} and 4 bytes of padding — protobuf's {@code Timestamp}. */
    public static final long TIMESTAMP_BYTES = 16;

    /** {@code {u16 year, u8 month, u8 day}}. */
    public static final long DATE_BYTES = 4;

    /** A date, 4 bytes of padding, then {@code u64} nanoseconds since midnight. */
    public static final long CIVIL_BYTES = 16;

    /** {@code u64} nanoseconds since midnight. */
    public static final long TIME_BYTES = 8;

    /** {@code {i64 seconds, i32 nanos}}, same-signed, and 4 bytes of padding — protobuf's {@code Duration}. */
    public static final long DURATION_BYTES = 16;

    /** {@code {u64 lo, u32 hi, u8 scale, u8 negative}} and 2 bytes of padding: a 96-bit magnitude and its scale. */
    public static final long DECIMAL_BYTES = 16;

    /** 16 bytes in RFC 9562 order. */
    public static final long UUID_BYTES = 16;

    /**
     * {@code {u32 decimal_sep, u32 group_sep, u32 flags, u32 currency_len, u8[16] currency}} —
     * the symbol's UTF-8 bytes inline, zero-padded.
     */
    public static final long FORMAT_BYTES = 32;

    /** A fault's span: {@code {u32 offset, u32 length}}, in bytes of the input. */
    public static final long FAULT_BYTES = 8;

    // A layout is read through a VarHandle, and one that is not a constant in a native image
    // costs ~70 ns a load there instead of one instruction: this class is initialized at
    // image build time (the jar's native-image.properties says so) for that reason.
    private static final ValueLayout.OfLong BIG_ENDIAN_LONG = ValueLayout.JAVA_LONG.withOrder(ByteOrder.BIG_ENDIAN);

    private NativeValues() {}

    /**
     * An instant: the timestamp, Unix and Excel-serial doors' value.
     *
     * @param segment the segment holding the value
     * @param at the value's byte offset
     * @return the instant, at full nanosecond fidelity
     */
    public static Instant instant(MemorySegment segment, long at) {
        return Instant.ofEpochSecond(segment.get(ValueLayout.JAVA_LONG, at), segment.get(ValueLayout.JAVA_INT, at + 8));
    }

    /**
     * A calendar date: the date doors' value.
     *
     * @param segment the segment holding the value
     * @param at the value's byte offset
     * @return the date
     */
    public static LocalDate date(MemorySegment segment, long at) {
        return LocalDate.of(
                Short.toUnsignedInt(segment.get(ValueLayout.JAVA_SHORT, at)),
                segment.get(ValueLayout.JAVA_BYTE, at + 2),
                segment.get(ValueLayout.JAVA_BYTE, at + 3));
    }

    /**
     * A zone-less wall clock: the date-time doors' value.
     *
     * @param segment the segment holding the value
     * @param at the value's byte offset
     * @return the wall clock, at full nanosecond fidelity
     */
    public static LocalDateTime civil(MemorySegment segment, long at) {
        return LocalDateTime.of(date(segment, at), time(segment, at + 8));
    }

    /**
     * A time of day: the time doors' value.
     *
     * @param segment the segment holding the value
     * @param at the value's byte offset
     * @return the time, at full nanosecond fidelity
     */
    public static LocalTime time(MemorySegment segment, long at) {
        return LocalTime.ofNanoOfDay(segment.get(ValueLayout.JAVA_LONG, at));
    }

    /**
     * A span: the duration doors' value.
     *
     * @param segment the segment holding the value
     * @param at the value's byte offset
     * @return the duration, at full nanosecond fidelity
     */
    public static Duration duration(MemorySegment segment, long at) {
        // Duration.ofSeconds normalizes the core's same-signed nanos adjustment correctly.
        return Duration.ofSeconds(segment.get(ValueLayout.JAVA_LONG, at), segment.get(ValueLayout.JAVA_INT, at + 8));
    }

    /**
     * An exact decimal: the decimal doors' value.
     *
     * @param segment the segment holding the value
     * @param at the value's byte offset
     * @return the decimal, exactly
     */
    public static BigDecimal decimal(MemorySegment segment, long at) {
        // BigInteger takes its magnitude big-endian, so the two words are laid out high word
        // first; the core never hands back a negative zero, so the signum needs no zero check.
        byte[] magnitude = new byte[12];
        ByteBuffer.wrap(magnitude)
                .putInt(segment.get(ValueLayout.JAVA_INT, at + 8))
                .putLong(segment.get(ValueLayout.JAVA_LONG, at));
        int signum = segment.get(ValueLayout.JAVA_BYTE, at + 13) != 0 ? -1 : 1;
        return new BigDecimal(new BigInteger(signum, magnitude), segment.get(ValueLayout.JAVA_BYTE, at + 12));
    }

    /**
     * A UUID: the uuid door's value.
     *
     * @param segment the segment holding the value
     * @param at the value's byte offset
     * @return the UUID
     */
    public static UUID uuid(MemorySegment segment, long at) {
        // RFC 9562 byte order is exactly UUID's msb/lsb decomposition: two big-endian longs.
        return new UUID(segment.get(BIG_ENDIAN_LONG, at), segment.get(BIG_ENDIAN_LONG, at + 8));
    }

    /**
     * Writes a format in the core's layout, the whole {@link #FORMAT_BYTES} of it: the
     * symbol field is zeroed past the symbol, so a segment reused for another format keeps no
     * tail. The format was validated when it was built.
     *
     * @param format the declared format
     * @param segment the segment to write into
     * @param at the byte offset to write at, 4-byte aligned
     */
    public static void writeFormat(NumFormat format, MemorySegment segment, long at) {
        segment.set(ValueLayout.JAVA_INT, at, format.decimalSeparator());
        segment.set(ValueLayout.JAVA_INT, at + 4, format.groupSeparator());
        segment.set(ValueLayout.JAVA_INT, at + 8, format.styles());
        byte[] symbol = format.currencySymbol().getBytes(StandardCharsets.UTF_8);
        segment.set(ValueLayout.JAVA_INT, at + 12, symbol.length);
        segment.asSlice(at + 16, 16).fill((byte) 0);
        MemorySegment.copy(symbol, 0, segment, ValueLayout.JAVA_BYTE, at + 16, symbol.length);
    }

    /**
     * The fault a nonzero verdict code and its span name.
     *
     * @param code the verdict code: 1 empty, 2 malformed, 3 out of range
     * @param offset the span's byte offset
     * @param length the span's byte length
     * @param <T> the type the cast was for
     * @return the fault
     * @throws IllegalStateException {@code code} is no reason — a binding bug, not data
     */
    public static <T> Fault<T> fault(int code, int offset, int length) {
        CastFailure reason = CastFailure.fromCode(code)
                .orElseThrow(() -> new IllegalStateException(
                        code + " is not a verdict reason code — a binding bug, please report it"));
        return new Fault<>(reason, offset, length);
    }

    /**
     * A native library's packed version word as {@code major.minor.patch}.
     *
     * @param packed {@code major << 16 | minor << 8 | patch}, as a {@code *_version()} export returns it
     * @return the version
     */
    public static String version(int packed) {
        return (packed >>> 16) + "." + ((packed >>> 8) & 0xFF) + "." + (packed & 0xFF);
    }
}
