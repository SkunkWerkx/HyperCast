#![deny(missing_docs)]
//! The link-time proof behind the `no-panic` feature. `#[no_panic]` only reports a function
//! when a release-optimized binary that calls it is linked, and a cdylib does not count: it
//! drops every function no export reaches. So this calls each annotated native once, with
//! inputs the optimizer cannot see through, and building it is the whole check. It needs
//! unwind, because under abort a panic path is invisible to `no-panic`:
//!
//!     CARGO_PROFILE_RELEASE_PANIC=unwind cargo build --release --locked --example no_panic --features no-panic
//!
//! A panic path left in any of them fails the link with `ERROR[no-panic]` naming the
//! function. Running the binary proves nothing: the calls sit behind a condition that is
//! never true.

use core::hint::black_box;
use core::ptr::null_mut;

fn main() {
    if std::env::args().len() > 1000 {
        let env = black_box(null_mut());
        let class = black_box(null_mut());
        let input = black_box(null_mut());
        let format = black_box(null_mut());
        let out = black_box(null_mut());
        let code = black_box(0_i32);
        let value = black_box(0.0_f64);
        unsafe {
            black_box(hypercast_jni::native_cast_bool(env, class, input, out));
            black_box(hypercast_jni::native_cast_char(env, class, input, out));
            black_box(hypercast_jni::native_cast_i8(
                env, class, input, format, out,
            ));
            black_box(hypercast_jni::native_cast_i16(
                env, class, input, format, out,
            ));
            black_box(hypercast_jni::native_cast_i32(
                env, class, input, format, out,
            ));
            black_box(hypercast_jni::native_cast_i64(
                env, class, input, format, out,
            ));
            black_box(hypercast_jni::native_cast_u8(
                env, class, input, format, out,
            ));
            black_box(hypercast_jni::native_cast_u16(
                env, class, input, format, out,
            ));
            black_box(hypercast_jni::native_cast_u32(
                env, class, input, format, out,
            ));
            black_box(hypercast_jni::native_cast_u64(
                env, class, input, format, out,
            ));
            black_box(hypercast_jni::native_cast_f32(
                env, class, input, format, out,
            ));
            black_box(hypercast_jni::native_cast_f64(
                env, class, input, format, out,
            ));
            black_box(hypercast_jni::native_cast_decimal(
                env, class, input, format, out,
            ));
            black_box(hypercast_jni::native_cast_uuid(env, class, input, out));
            black_box(hypercast_jni::native_cast_timestamp(env, class, input, out));
            black_box(hypercast_jni::native_cast_unix(
                env, class, input, code, out,
            ));
            black_box(hypercast_jni::native_cast_excel_serial(
                env, class, input, code, out,
            ));
            black_box(hypercast_jni::native_cast_date_ordered(
                env, class, input, code, out,
            ));
            black_box(hypercast_jni::native_cast_datetime(
                env, class, input, code, format, out,
            ));
            black_box(hypercast_jni::native_cast_date(env, class, input, out));
            black_box(hypercast_jni::native_cast_time(env, class, input, out));
            black_box(hypercast_jni::native_cast_duration(env, class, input, out));
            black_box(hypercast_jni::native_cast_decimal_from_f64(
                env, class, value, out,
            ));
            black_box(hypercast_jni::native_cast_excel_serial_from_f64(
                env, class, value, code, out,
            ));
            black_box(hypercast_jni::native_cast_excel_time(
                env, class, value, out,
            ));
            black_box(hypercast_jni::native_cast_excel_duration(
                env, class, value, out,
            ));
            black_box(hypercast_jni::native_hypercast_version(env, class));
            let entries = black_box(null_mut());
            let constraints = black_box(null_mut());
            black_box(hypercast_jni::native_cast_checked(
                env,
                class,
                code,
                format,
                constraints,
                entries,
                input,
                out,
            ));
            black_box(hypercast_jni::native_cast_checked_from_f64(
                env,
                class,
                code,
                value,
                constraints,
                entries,
                out,
            ));
            black_box(hypercast_jni::native_cast_constraints_check(
                env,
                class,
                code,
                constraints,
                entries,
            ));
            black_box(hypercast_jni::native_cast_text(env, class, input, out));
            black_box(hypercast_jni::native_cast_declared_decimal(
                env, class, input, out,
            ));
            black_box(hypercast_jni::native_jni_natives(env, class));
            black_box(hypercast_jni::JNI_OnLoad(
                black_box(null_mut()),
                black_box(null_mut()),
            ));
        }
    }
}
