package io.github.skunkwerkx.hypercast;

/**
 * Which door a {@link NativeCore} call is for: the core's own door code, the one {@code Door}
 * numbers in {@code rust/src/door.rs} and every detailed export takes. Both {@code NativeCore}
 * trees pass it straight through, and this class has one copy in the jar's base that both
 * read, so an id cannot mean one door on one JDK and another on the next. A
 * {@code static final int} is folded into its caller at compile time, which is why the ids
 * live here and not in either tree.
 *
 * <p>A door that declares something carries it beside the id: the Unix precision, the date
 * order, the Excel epoch ({@code param}). The typed doors reuse the ids of the text doors they
 * read the same type for ({@link #DECIMAL}, {@link #TIME}, {@link #DURATION},
 * {@link #EXCEL_SERIAL}); which export they reach is the {@code NativeCore} method's shape, not
 * the id's.
 */
final class Door {
    private Door() {}

    // (text) — NativeCore.plain
    static final int BOOL = 1;
    static final int CHAR = 23;
    static final int UUID = 12;
    static final int TIMESTAMP = 13;
    static final int DATE = 15;
    static final int TIME = 16;
    static final int DURATION = 17;
    static final int TEXT = 18;

    // (text, format) — NativeCore.numeric
    static final int I8 = 2;
    static final int I16 = 3;
    static final int I32 = 4;
    static final int I64 = 5;
    static final int U8 = 6;
    static final int U16 = 7;
    static final int U32 = 8;
    static final int U64 = 9;
    static final int F32 = 10;
    static final int F64 = 11;
    static final int DECIMAL = 19;

    // (text, declared code) — NativeCore.declared
    static final int UNIX = 14;
    static final int DATE_ORDERED = 20;
    static final int EXCEL_SERIAL = 22;

    // (double) — NativeCore.typed
    static final int DECIMAL_FROM_F64 = DECIMAL;
    static final int EXCEL_TIME = TIME;
    static final int EXCEL_DURATION = DURATION;

    // (double, declared code) — NativeCore.typedDeclared
    static final int EXCEL_SERIAL_FROM_F64 = EXCEL_SERIAL;

    // The civil date-time door is the one shape with a single door: NativeCore.civil.
    static final int DATETIME = 21;
}
