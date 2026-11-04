package io.github.skunkwerkx.hypercast;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.lang.foreign.MemorySegment;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.util.Locale;
import org.junit.jupiter.api.Test;

/**
 * Every reference parameter of every public door refuses {@code null} with a {@link
 * NullPointerException} named for the parameter, before any native call, so the JNI path (where
 * a null array would reach the library) and the FFM path answer alike.
 */
class NullArgumentTest {
    private static Object valid(Class<?> type) {
        if (type == String.class) {
            return "1";
        }
        if (type == byte[].class) {
            return "1".getBytes(StandardCharsets.UTF_8);
        }
        if (type == MemorySegment.class) {
            return MemorySegment.ofArray("1".getBytes(StandardCharsets.UTF_8));
        }
        if (type == NumFormat.class) {
            return NumFormat.INVARIANT;
        }
        if (type == DateTimeFormat.class) {
            return DateTimeFormat.from(Locale.US);
        }
        if (type == Locale.class) {
            return Locale.US;
        }
        if (type == UnixPrecision.class) {
            return UnixPrecision.values()[0];
        }
        if (type == ExcelEpoch.class) {
            return ExcelEpoch.values()[0];
        }
        if (type == DateOrder.class) {
            return DateOrder.values()[0];
        }
        if (type == Verdict.class) {
            return new Success<>(1);
        }
        if (type == CheckedVerdict.class) {
            return new Checked<>(1, false, java.util.OptionalInt.empty());
        }
        if (type == Fault.class) {
            return new Fault<>(CastFailure.MALFORMED, 0, 1);
        }
        if (type == DateOptions.class) {
            return DateOptions.NONE;
        }
        if (type == MonthNames.class) {
            return DetailedCastTest.english();
        }
        if (type == DetailedVerdict.class) {
            return new DetailedSuccess<>(1);
        }
        if (type == DetailedCheckedVerdict.class) {
            return new DetailedChecked<>(1, false, java.util.OptionalInt.empty());
        }
        if (type == DetailedFault.class) {
            return new DetailedFault<>(CastFailure.MALFORMED, FaultDetail.BLANK, 0, 1);
        }
        if (type == double.class) {
            return 1.0;
        }
        if (type == Constraints.class) {
            // Any door's: every null is refused before the door binding is checked.
            return Constraints.text().build();
        }
        throw new AssertionError("no valid value for " + type);
    }

    private static String expectedName(Class<?> type) {
        if (type == String.class) {
            return "text";
        }
        if (type == byte[].class || type == MemorySegment.class) {
            return "utf8";
        }
        if (type == NumFormat.class || type == DateTimeFormat.class) {
            return "format";
        }
        if (type == Locale.class) {
            return "locale";
        }
        if (type == UnixPrecision.class) {
            return "precision";
        }
        if (type == ExcelEpoch.class) {
            return "epoch";
        }
        if (type == DateOrder.class) {
            return "order";
        }
        if (type == Verdict.class || type == CheckedVerdict.class || type == Fault.class) {
            return "verdict";
        }
        if (type == Constraints.class) {
            return "constraints";
        }
        if (type == DateOptions.class) {
            return "options";
        }
        if (type == MonthNames.class) {
            return "months";
        }
        if (type == DetailedVerdict.class || type == DetailedCheckedVerdict.class || type == DetailedFault.class) {
            return "verdict";
        }
        throw new AssertionError("no expected name for " + type);
    }

    // The one that would reach the library on the JNI path: a null array.
    @Test
    void nullBytesIsANullPointerException() {
        NullPointerException e =
                assertThrows(NullPointerException.class, () -> Cast.i32((byte[]) null, NumFormat.INVARIANT));
        assertEquals("utf8", e.getMessage());
    }

    @Test
    void everyReferenceParameterRefusesNull() throws Exception {
        // A new public door with a reference parameter changes this count on purpose.
        assertEquals(312, refusals(Cast.class));
    }

    /** The detailed path's doors refuse null in the same words, parameter for parameter. */
    @Test
    void everyReferenceParameterOfTheDetailedPathRefusesNull() throws Exception {
        assertEquals(387, refusals(DetailedCast.class));
    }

    private static int refusals(Class<?> doors) throws Exception {
        int checked = 0;
        for (Method m : doors.getDeclaredMethods()) {
            if (!Modifier.isPublic(m.getModifiers()) || !Modifier.isStatic(m.getModifiers())) {
                continue;
            }
            Class<?>[] types = m.getParameterTypes();
            for (int i = 0; i < types.length; i++) {
                if (types[i].isPrimitive()) {
                    continue;
                }
                Object[] args = new Object[types.length];
                for (int j = 0; j < types.length; j++) {
                    args[j] = j == i ? null : valid(types[j]);
                }
                InvocationTargetException e = assertThrows(
                        InvocationTargetException.class,
                        () -> m.invoke(null, args),
                        m.toGenericString() + " with null at " + i);
                assertTrue(
                        e.getCause() instanceof NullPointerException,
                        m.toGenericString() + " arg " + i + " threw " + e.getCause());
                assertEquals(expectedName(types[i]), e.getCause().getMessage(), m.toGenericString() + " arg " + i);
                checked++;
            }
        }
        return checked;
    }
}
