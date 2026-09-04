package io.github.skunkwerkx.hypercast;

/**
 * The twenty {@code cast_*} exports of the native core, named once so both interop paths
 * — the FFM downcalls in {@link Cast} and the GraalWasm calls in {@link WasmBackend} — key
 * off the same list. The ordinal is what the wasm backend indexes its resolved exports by.
 */
enum Door {
    BOOL("cast_bool"),
    I8("cast_i8"),
    I16("cast_i16"),
    I32("cast_i32"),
    I64("cast_i64"),
    U8("cast_u8"),
    U16("cast_u16"),
    U32("cast_u32"),
    U64("cast_u64"),
    F32("cast_f32"),
    F64("cast_f64"),
    UUID("cast_uuid"),
    TIMESTAMP("cast_timestamp"),
    UNIX("cast_unix"),
    EXCEL_SERIAL("cast_excel_serial"),
    DATE("cast_date"),
    DATE_ORDERED("cast_date_ordered"),
    DATETIME("cast_datetime"),
    TIME("cast_time"),
    DURATION("cast_duration");

    private final String symbol;

    Door(String symbol) {
        this.symbol = symbol;
    }

    /** The C-ABI export name, as {@code ffi.rs} spells it. */
    String symbol() {
        return symbol;
    }
}
