package io.github.skunkwerkx.hypercast.interop;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.skunkwerkx.hypercast.CastFailure;
import io.github.skunkwerkx.hypercast.DateOrder;
import io.github.skunkwerkx.hypercast.ExcelEpoch;
import io.github.skunkwerkx.hypercast.Fault;
import io.github.skunkwerkx.hypercast.NumFormat;
import io.github.skunkwerkx.hypercast.UnixPrecision;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.Optional;
import java.util.UUID;
import org.junit.jupiter.api.Test;

/**
 * The public interop surface a library carrying HyperCast's verdicts across its own C ABI
 * reads with: each value read back from bytes laid out as the core lays them out, each code
 * decoded to exactly the constant it names.
 */
class NativeValuesTest {

    @Test
    void everyCodeDecodesToTheConstantItNamesAndNothingElseDecodes() {
        for (UnixPrecision precision : UnixPrecision.values()) {
            assertEquals(Optional.of(precision), UnixPrecision.fromCode(precision.code()));
        }
        for (DateOrder order : DateOrder.values()) {
            assertEquals(Optional.of(order), DateOrder.fromCode(order.code()));
        }
        for (ExcelEpoch epoch : ExcelEpoch.values()) {
            assertEquals(Optional.of(epoch), ExcelEpoch.fromCode(epoch.code()));
        }
        for (CastFailure reason : CastFailure.values()) {
            assertEquals(Optional.of(reason), CastFailure.fromCode(reason.code()));
        }
        for (int outside : new int[] {0, 5, -1, Integer.MAX_VALUE}) {
            assertTrue(UnixPrecision.fromCode(outside).isEmpty());
            assertTrue(DateOrder.fromCode(outside).isEmpty());
            assertTrue(ExcelEpoch.fromCode(outside).isEmpty());
            assertTrue(CastFailure.fromCode(outside).isEmpty());
        }
    }

    @Test
    void aFaultKeepsItsSpanAndACodeThatNamesNoReasonIsRefused() {
        Fault<Integer> fault = NativeValues.fault(2, 3, 4);
        assertEquals(new Fault<Integer>(CastFailure.MALFORMED, 3, 4), fault);
        assertThrows(IllegalStateException.class, () -> NativeValues.fault(0, 0, 0));
        assertThrows(IllegalStateException.class, () -> NativeValues.fault(7, 0, 0));
    }

    @Test
    void versionsUnpack() {
        assertEquals("0.6.2", NativeValues.version(0x00_06_02));
        assertEquals("1.2.3", NativeValues.version(0x01_02_03));
    }

    @Test
    void valuesReadBackAtAnyAlignedOffset() {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment segment = arena.allocate(64, 8);
            long at = 16;

            segment.set(ValueLayout.JAVA_LONG, at, 1_767_366_245L);
            segment.set(ValueLayout.JAVA_INT, at + 8, 123_456_789);
            assertEquals(Instant.parse("2026-01-02T15:04:05.123456789Z"), NativeValues.instant(segment, at));

            segment.set(ValueLayout.JAVA_SHORT, at, (short) 2026);
            segment.set(ValueLayout.JAVA_BYTE, at + 2, (byte) 1);
            segment.set(ValueLayout.JAVA_BYTE, at + 3, (byte) 7);
            assertEquals(LocalDate.of(2026, 1, 7), NativeValues.date(segment, at));
            segment.set(ValueLayout.JAVA_LONG, at + 8, 54_245_123_456_789L);
            assertEquals(LocalDateTime.of(2026, 1, 7, 15, 4, 5, 123_456_789), NativeValues.civil(segment, at));
            assertEquals(LocalTime.of(15, 4, 5, 123_456_789), NativeValues.time(segment, at + 8));

            segment.set(ValueLayout.JAVA_LONG, at, -1L);
            segment.set(ValueLayout.JAVA_INT, at + 8, -500_000_000);
            assertEquals(Duration.ofMillis(-1_500), NativeValues.duration(segment, at));

            segment.set(ValueLayout.JAVA_LONG, at, 12_345L);
            segment.set(ValueLayout.JAVA_INT, at + 8, 0);
            segment.set(ValueLayout.JAVA_BYTE, at + 12, (byte) 2);
            segment.set(ValueLayout.JAVA_BYTE, at + 13, (byte) 1);
            assertEquals(new BigDecimal("-123.45"), NativeValues.decimal(segment, at));

            byte[] rfc = {
                0x55,
                0x0e,
                (byte) 0x84,
                0x00,
                (byte) 0xe2,
                (byte) 0x9b,
                0x41,
                (byte) 0xd4,
                (byte) 0xa7,
                0x16,
                0x44,
                0x66,
                0x55,
                0x44,
                0x00,
                0x00
            };
            MemorySegment.copy(rfc, 0, segment, ValueLayout.JAVA_BYTE, at, rfc.length);
            assertEquals(UUID.fromString("550e8400-e29b-41d4-a716-446655440000"), NativeValues.uuid(segment, at));
        }
    }

    @Test
    void aFormatWritesTheCoresLayoutAndLeavesNoTailOfAnEarlierOne() {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment segment = arena.allocate(NativeValues.FORMAT_BYTES + 4, 4);
            NativeValues.writeFormat(new NumFormat('.', ',', NumFormat.STYLE_ALL, "US$"), segment, 4);
            NativeValues.writeFormat(new NumFormat(',', '.', NumFormat.STYLE_ALL, "€"), segment, 4);
            assertEquals(',', segment.get(ValueLayout.JAVA_INT, 4));
            assertEquals('.', segment.get(ValueLayout.JAVA_INT, 8));
            assertEquals(NumFormat.STYLE_ALL, segment.get(ValueLayout.JAVA_INT, 12));
            byte[] euro = "€".getBytes(StandardCharsets.UTF_8);
            assertEquals(euro.length, segment.get(ValueLayout.JAVA_INT, 16));
            byte[] symbol = segment.asSlice(20, 16).toArray(ValueLayout.JAVA_BYTE);
            byte[] expected = new byte[16];
            System.arraycopy(euro, 0, expected, 0, euro.length);
            assertArrayEquals(expected, symbol);
        }
    }

    @Test
    void theLibraryIsNamedForTheCoreAskedFor() {
        assertEquals(
                "/native/linux-x64/libhypertabular.so",
                NativePlatform.resolve("hypertabular", "Linux", "amd64", false).resourcePath());
        assertEquals(
                "/native/win-arm64/hypercast.dll",
                NativePlatform.resolve("hypercast", "Windows 11", "aarch64", false)
                        .resourcePath());
    }
}
