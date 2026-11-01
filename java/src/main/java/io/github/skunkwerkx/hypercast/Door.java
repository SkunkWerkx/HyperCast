package io.github.skunkwerkx.hypercast;

/**
 * Which door a {@link NativeCore} call is for: one id per text and typed export, grouped by
 * the ABI shape that carries it. Both {@code NativeCore} trees switch on these, the JNI one to
 * pick its native and the FFM one to pick the export's address, and this class has one copy
 * in the jar's base that both read, so an id cannot mean one door on one JDK and another on
 * the next. A {@code static final int} is folded into its caller at compile time, which is
 * why the ids live here and not in either tree.
 */
final class Door {
    private Door() {}

    // (text) — NativeCore.plain
    static final int BOOL = 0;
    static final int CHAR = 1;
    static final int UUID = 2;
    static final int TIMESTAMP = 3;
    static final int DATE = 4;
    static final int TIME = 5;
    static final int DURATION = 6;

    // (text, format) — NativeCore.numeric
    static final int I8 = 10;
    static final int I16 = 11;
    static final int I32 = 12;
    static final int I64 = 13;
    static final int U8 = 14;
    static final int U16 = 15;
    static final int U32 = 16;
    static final int U64 = 17;
    static final int F32 = 18;
    static final int F64 = 19;
    static final int DECIMAL = 20;

    // (text, declared code) — NativeCore.declared
    static final int UNIX = 30;
    static final int EXCEL_SERIAL = 31;
    static final int DATE_ORDERED = 32;

    // (double) — NativeCore.typed
    static final int DECIMAL_FROM_F64 = 40;
    static final int EXCEL_TIME = 41;
    static final int EXCEL_DURATION = 42;

    // (double, declared code) — NativeCore.typedDeclared
    static final int EXCEL_SERIAL_FROM_F64 = 50;

    // The civil date-time door is the one shape with a single door: NativeCore.civil.

    /** The failure a door id a shape does not carry is: a binding bug, never data. */
    static IllegalArgumentException unknown(int door) {
        return new IllegalArgumentException("door " + door + " is not of this shape — a binding bug, please report it");
    }
}
