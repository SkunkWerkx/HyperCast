//! The four detailed C exports (`cast_detailed`, `cast_detailed_from_f64`,
//! `cast_checked_detailed`, `cast_checked_detailed_from_f64`), linked from the rlib as a C
//! caller links them, held to the 1.0 exports they mirror and to the Rust `detailed` doors
//! they dispatch to.
//!
//! - **Differential:** every corpus row at its own door, and every corpus input at every
//!   door, through the old export and through `cast_detailed` with Western grouping and no
//!   date options, which must agree; the four typed exports against `cast_detailed_from_f64`;
//!   `cast_checked`/`cast_checked_from_f64` against their detailed pair. Values are compared
//!   field by field (never padding; floats by bits; byte arrays as bytes), faults and checks
//!   only on the codes that write them, and every other byte must be left as it was.
//! - **Declarations:** every row with its own format (Indian grouping included), day periods
//!   and date options through `cast_detailed`, against the Rust `detailed` door and the row.
//! - **Sentinels:** fault capacities, a null fault, an unaligned `out`, poisoned pointers a
//!   door ignores, unknown doors and parameters, refused layouts and vocabularies.
//!
//! Each call runs twice, under two fills of every out-parameter, so a byte an export writes
//! is told apart from one it left.

use core::ffi::c_void;
use hypercast::{
    CurrencySymbol, DateOptions, DayPeriods, Detail, Door, Failure, Grouping, MonthNames,
    NativeDigits, NumFormat, RawCheck, RawConstraints, RawDateOptions, RawDayPeriods,
    RawFaultDetail, RawNumFormat, RawSpan, Symbol, ValidDateOptions, ValidNumFormat, detailed,
};
use serde_json::Value;
use std::path::PathBuf;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RawFault {
    offset: u32,
    len: u32,
}

use hypercast::{CivilDateTime, Date, Decimal, Duration, Timestamp};

unsafe extern "C" {
    fn cast_bool(ptr: *const u8, len: usize, out: *mut u8, fault: *mut RawFault) -> i32;
    fn cast_char(ptr: *const u8, len: usize, out: *mut u32, fault: *mut RawFault) -> i32;
    fn cast_i8(p: *const u8, l: usize, f: *const RawNumFormat, o: *mut i8, x: *mut RawFault)
    -> i32;
    fn cast_i16(
        p: *const u8,
        l: usize,
        f: *const RawNumFormat,
        o: *mut i16,
        x: *mut RawFault,
    ) -> i32;
    fn cast_i32(
        p: *const u8,
        l: usize,
        f: *const RawNumFormat,
        o: *mut i32,
        x: *mut RawFault,
    ) -> i32;
    fn cast_i64(
        p: *const u8,
        l: usize,
        f: *const RawNumFormat,
        o: *mut i64,
        x: *mut RawFault,
    ) -> i32;
    fn cast_u8(p: *const u8, l: usize, f: *const RawNumFormat, o: *mut u8, x: *mut RawFault)
    -> i32;
    fn cast_u16(
        p: *const u8,
        l: usize,
        f: *const RawNumFormat,
        o: *mut u16,
        x: *mut RawFault,
    ) -> i32;
    fn cast_u32(
        p: *const u8,
        l: usize,
        f: *const RawNumFormat,
        o: *mut u32,
        x: *mut RawFault,
    ) -> i32;
    fn cast_u64(
        p: *const u8,
        l: usize,
        f: *const RawNumFormat,
        o: *mut u64,
        x: *mut RawFault,
    ) -> i32;
    fn cast_f32(
        p: *const u8,
        l: usize,
        f: *const RawNumFormat,
        o: *mut f32,
        x: *mut RawFault,
    ) -> i32;
    fn cast_f64(
        p: *const u8,
        l: usize,
        f: *const RawNumFormat,
        o: *mut f64,
        x: *mut RawFault,
    ) -> i32;
    fn cast_decimal(
        p: *const u8,
        l: usize,
        f: *const RawNumFormat,
        o: *mut Decimal,
        x: *mut RawFault,
    ) -> i32;
    fn cast_uuid(ptr: *const u8, len: usize, out: *mut u8, fault: *mut RawFault) -> i32;
    fn cast_timestamp(ptr: *const u8, len: usize, out: *mut Timestamp, f: *mut RawFault) -> i32;
    fn cast_unix(
        p: *const u8,
        l: usize,
        precision: u32,
        o: *mut Timestamp,
        f: *mut RawFault,
    ) -> i32;
    fn cast_excel_serial(
        p: *const u8,
        l: usize,
        epoch: u32,
        o: *mut Timestamp,
        f: *mut RawFault,
    ) -> i32;
    fn cast_date_ordered(p: *const u8, l: usize, order: u32, o: *mut Date, f: *mut RawFault)
    -> i32;
    fn cast_datetime(
        p: *const u8,
        l: usize,
        order: u32,
        periods: *const RawDayPeriods,
        o: *mut CivilDateTime,
        f: *mut RawFault,
    ) -> i32;
    fn cast_date(ptr: *const u8, len: usize, out: *mut Date, fault: *mut RawFault) -> i32;
    fn cast_time(ptr: *const u8, len: usize, out: *mut u64, fault: *mut RawFault) -> i32;
    fn cast_duration(ptr: *const u8, len: usize, out: *mut Duration, fault: *mut RawFault) -> i32;
    fn cast_text(ptr: *const u8, len: usize, out: *mut RawSpan, fault: *mut RawFault) -> i32;
    fn cast_decimal_from_f64(value: f64, out: *mut Decimal, fault: *mut RawFault) -> i32;
    fn cast_excel_serial_from_f64(
        value: f64,
        epoch: u32,
        out: *mut CivilDateTime,
        fault: *mut RawFault,
    ) -> i32;
    fn cast_excel_time(value: f64, out: *mut u64, fault: *mut RawFault) -> i32;
    fn cast_excel_duration(value: f64, out: *mut Duration, fault: *mut RawFault) -> i32;
    fn cast_checked(
        door: u32,
        param: u32,
        format: *const RawNumFormat,
        constraints: *const RawConstraints,
        enum_ptr: *const u8,
        enum_len: usize,
        ptr: *const u8,
        len: usize,
        out: *mut c_void,
        fault: *mut RawFault,
        check: *mut RawCheck,
    ) -> i32;
    fn cast_constraints_check(
        door: u32,
        param: u32,
        constraints: *const RawConstraints,
        enum_ptr: *const u8,
        enum_len: usize,
    ) -> i32;
    fn cast_checked_from_f64(
        door: u32,
        value: f64,
        constraints: *const RawConstraints,
        enum_ptr: *const u8,
        enum_len: usize,
        out: *mut c_void,
        fault: *mut RawFault,
        check: *mut RawCheck,
    ) -> i32;

    fn cast_detailed(
        door: u32,
        param: u32,
        format: *const RawNumFormat,
        periods: *const RawDayPeriods,
        date_options: *const RawDateOptions,
        vocab_ptr: *const u8,
        vocab_len: usize,
        ptr: *const u8,
        len: usize,
        out: *mut c_void,
        fault: *mut RawFaultDetail,
    ) -> i32;
    fn cast_detailed_from_f64(
        door: u32,
        param: u32,
        value: f64,
        out: *mut c_void,
        fault: *mut RawFaultDetail,
    ) -> i32;
    fn cast_checked_detailed(
        door: u32,
        param: u32,
        format: *const RawNumFormat,
        constraints: *const RawConstraints,
        enum_ptr: *const u8,
        enum_len: usize,
        ptr: *const u8,
        len: usize,
        out: *mut c_void,
        fault: *mut RawFaultDetail,
        check: *mut RawCheck,
    ) -> i32;
    fn cast_checked_detailed_from_f64(
        door: u32,
        param: u32,
        constraints: *const RawConstraints,
        enum_ptr: *const u8,
        enum_len: usize,
        value: f64,
        out: *mut c_void,
        fault: *mut RawFaultDetail,
        check: *mut RawCheck,
    ) -> i32;
}

// ---------------------------------------------------------------------------------------
// What an export wrote.

/// The shape of the value an export writes, read field by field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Bool,
    Char,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Uuid,
    Timestamp,
    Date,
    Nanos,
    Duration,
    Span,
    Decimal,
    Civil,
}

impl Kind {
    fn size(self) -> usize {
        match self {
            Kind::Bool | Kind::I8 | Kind::U8 => 1,
            Kind::I16 | Kind::U16 => 2,
            Kind::Char | Kind::I32 | Kind::U32 | Kind::F32 | Kind::Date => 4,
            Kind::I64 | Kind::U64 | Kind::F64 | Kind::Nanos | Kind::Span => 8,
            Kind::Uuid | Kind::Timestamp | Kind::Duration | Kind::Decimal | Kind::Civil => 16,
        }
    }

    /// The defined fields of a value of this kind at `bytes`: never padding, floats by bits.
    fn fields(self, bytes: &[u8]) -> Vec<u128> {
        assert!(bytes.len() >= self.size());
        let p = bytes.as_ptr();
        // SAFETY: `bytes` holds `size()` bytes an export wrote as a value of this kind.
        unsafe {
            match self {
                Kind::Bool => vec![u128::from(p.read())],
                Kind::U8 => vec![u128::from(p.read())],
                Kind::I8 => vec![i128::from(p.cast::<i8>().read()) as u128],
                Kind::I16 => vec![i128::from(p.cast::<i16>().read_unaligned()) as u128],
                Kind::U16 => vec![u128::from(p.cast::<u16>().read_unaligned())],
                Kind::Char | Kind::U32 => vec![u128::from(p.cast::<u32>().read_unaligned())],
                Kind::I32 => vec![i128::from(p.cast::<i32>().read_unaligned()) as u128],
                Kind::I64 => vec![i128::from(p.cast::<i64>().read_unaligned()) as u128],
                Kind::U64 | Kind::Nanos => vec![u128::from(p.cast::<u64>().read_unaligned())],
                Kind::F32 => vec![u128::from(p.cast::<u32>().read_unaligned())],
                Kind::F64 => vec![u128::from(p.cast::<u64>().read_unaligned())],
                Kind::Uuid => vec![u128::from_ne_bytes(bytes[..16].try_into().unwrap())],
                Kind::Timestamp => timestamp(p.cast::<Timestamp>().read_unaligned()),
                Kind::Date => date(p.cast::<Date>().read_unaligned()),
                Kind::Duration => duration(p.cast::<Duration>().read_unaligned()),
                Kind::Span => {
                    let span = p.cast::<RawSpan>().read_unaligned();
                    vec![span.offset.into(), span.len.into()]
                }
                Kind::Decimal => {
                    // The sign byte is read as a byte: a `bool` read of a byte the export did
                    // not write would be undefined.
                    let lo = p.cast::<u64>().read_unaligned();
                    let hi = p.add(8).cast::<u32>().read_unaligned();
                    vec![lo.into(), hi.into(), p.add(12).read().into(), p.add(13).read().into()]
                }
                Kind::Civil => civil(p.cast::<CivilDateTime>().read_unaligned()),
            }
        }
    }
}

fn timestamp(t: Timestamp) -> Vec<u128> {
    vec![t.seconds as u128, t.nanos as u128]
}

fn date(d: Date) -> Vec<u128> {
    vec![d.year.into(), d.month.into(), d.day.into()]
}

fn duration(d: Duration) -> Vec<u128> {
    vec![d.seconds as u128, d.nanos as u128]
}

fn civil(c: CivilDateTime) -> Vec<u128> {
    let mut fields = date(c.date);
    fields.push(c.nanos_of_day.into());
    fields
}

fn decimal(d: Decimal) -> Vec<u128> {
    vec![d.lo.into(), d.hi.into(), d.scale.into(), u128::from(d.negative)]
}

/// The kind a text door's export writes.
fn text_kind(door: Door) -> Kind {
    match door {
        Door::Bool => Kind::Bool,
        Door::Char => Kind::Char,
        Door::I8 => Kind::I8,
        Door::I16 => Kind::I16,
        Door::I32 => Kind::I32,
        Door::I64 => Kind::I64,
        Door::U8 => Kind::U8,
        Door::U16 => Kind::U16,
        Door::U32 => Kind::U32,
        Door::U64 => Kind::U64,
        Door::F32 => Kind::F32,
        Door::F64 => Kind::F64,
        Door::Uuid => Kind::Uuid,
        Door::Timestamp | Door::Unix(_) | Door::ExcelSerial(_) => Kind::Timestamp,
        Door::Date | Door::DateOrdered(_) => Kind::Date,
        Door::Time => Kind::Nanos,
        Door::Duration => Kind::Duration,
        Door::Text => Kind::Span,
        Door::Decimal => Kind::Decimal,
        Door::DateTime(_) => Kind::Civil,
        other => panic!("no text door {other:?}"),
    }
}

/// The kind a typed door's export writes.
fn typed_kind(door: Door) -> Kind {
    match door {
        Door::Decimal => Kind::Decimal,
        Door::ExcelSerial(_) => Kind::Civil,
        Door::Time => Kind::Nanos,
        Door::Duration => Kind::Duration,
        other => panic!("no typed C door {other:?}"),
    }
}

/// What one export call did: its code, the value it wrote, and what it wrote to `fault` and
/// `check`. `None` is a parameter left exactly as it was.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Outcome {
    code: i32,
    value: Option<Vec<u128>>,
    span: Option<(u32, u32)>,
    detail: Option<u32>,
    check: Option<RawCheck>,
}

const FILLS: [u8; 2] = [0xA5, 0x5A];
const OUT_AT: usize = 3;

fn fill_word(fill: u8) -> u32 {
    u32::from_ne_bytes([fill; 4])
}

/// The two runs of one call agree, and what each left alone is still its fill.
fn agreed(runs: Vec<Outcome>, label: &str) -> Outcome {
    assert_eq!(runs[0], runs[1], "{label}: the two fills disagree");
    runs.into_iter().next().unwrap()
}

/// Calls a 1.0 export with an aligned `out` and a `RawFault`.
fn old(kind: Kind, label: &str, call: impl Fn(*mut u8, *mut RawFault) -> i32) -> Outcome {
    old_checked(kind, label, |out, fault, _| call(out, fault))
}

/// Calls a 1.0 export with an aligned `out`, a `RawFault` and a `RawCheck`.
fn old_checked(
    kind: Kind,
    label: &str,
    call: impl Fn(*mut u8, *mut RawFault, *mut RawCheck) -> i32,
) -> Outcome {
    let runs = FILLS
        .iter()
        .map(|&fill| {
            let mut out = [u64::from_ne_bytes([fill; 8]); 6];
            let word = fill_word(fill);
            let mut fault = RawFault { offset: word, len: word };
            let mut check = RawCheck { keyword: word, adjusted: word, enum_index: word };
            let code = call(out.as_mut_ptr().cast(), &mut fault, &mut check);
            // SAFETY: plain bytes.
            let bytes: [u8; 48] = unsafe { core::mem::transmute(out) };
            outcome(code, kind, &bytes, 0, fill, fault_written(&fault, word), None, &check, label)
        })
        .collect();
    agreed(runs, label)
}

fn fault_written(fault: &RawFault, word: u32) -> Option<(u32, u32)> {
    (*fault != RawFault { offset: word, len: word }).then_some((fault.offset, fault.len))
}

/// Calls a detailed export with `out` at an odd address and a 16-byte `RawFaultDetail`.
fn new(kind: Kind, label: &str, call: impl Fn(*mut c_void, *mut RawFaultDetail) -> i32) -> Outcome {
    new_checked(kind, label, |out, fault, _| call(out, fault))
}

/// Calls a detailed checked export with `out` at an odd address, a `RawFaultDetail` and a
/// `RawCheck`.
fn new_checked(
    kind: Kind,
    label: &str,
    call: impl Fn(*mut c_void, *mut RawFaultDetail, *mut RawCheck) -> i32,
) -> Outcome {
    let runs = FILLS
        .iter()
        .map(|&fill| {
            let mut out = [fill; 48];
            let word = fill_word(fill);
            let mut fault = RawFaultDetail { size: 16, offset: word, len: word, detail: word };
            let mut check = RawCheck { keyword: word, adjusted: word, enum_index: word };
            // SAFETY: `OUT_AT` leaves room for any value.
            let code = call(unsafe { out.as_mut_ptr().add(OUT_AT) }.cast(), &mut fault, &mut check);
            assert_eq!(fault.size, 16, "{label}: the core wrote `size`");
            let untouched = RawFaultDetail { size: 16, offset: word, len: word, detail: word };
            let (span, detail) = if fault == untouched {
                (None, None)
            } else {
                (Some((fault.offset, fault.len)), Some(fault.detail))
            };
            outcome(code, kind, &out, OUT_AT, fill, span, Some(detail), &check, label)
        })
        .collect();
    agreed(runs, label)
}

/// One run's outcome, after checking that `out` was written only on success and only where
/// the value goes, and `check` only on 0 and 4.
#[allow(clippy::too_many_arguments)]
fn outcome(
    code: i32,
    kind: Kind,
    out: &[u8],
    at: usize,
    fill: u8,
    span: Option<(u32, u32)>,
    detail: Option<Option<u32>>,
    check: &RawCheck,
    label: &str,
) -> Outcome {
    let word = fill_word(fill);
    let check_written =
        (*check != RawCheck { keyword: word, adjusted: word, enum_index: word }).then_some(*check);
    let value_at = at..at + kind.size();
    let outside_untouched = out
        .iter()
        .enumerate()
        .all(|(i, &byte)| byte == fill || (matches!(code, 0 | 4) && value_at.contains(&i)));
    assert!(outside_untouched, "{label}: code {code} wrote `out` where it must not: {out:?}");
    // A success and a violation both write the value as cast.
    let value = matches!(code, 0 | 4).then(|| kind.fields(&out[value_at]));
    match code {
        0 => assert_eq!(span, None, "{label}: success wrote `fault`"),
        1..=4 => assert!(span.is_some(), "{label}: code {code} wrote no fault"),
        -1 => {
            assert_eq!(span, None, "{label}: a contract violation wrote `fault`");
            assert_eq!(check_written, None, "{label}: a contract violation wrote `check`");
        }
        other => panic!("{label}: code {other}"),
    }
    if (1..=3).contains(&code) {
        assert_eq!(check_written, None, "{label}: a failure wrote `check`");
    }
    Outcome { code, value, span, detail: detail.flatten(), check: check_written }
}

// ---------------------------------------------------------------------------------------
// The calls.

/// Every door `cast_detailed` takes, with each parameter it declares: 30 pairs.
fn every_text_door() -> Vec<(u32, u32)> {
    let mut doors = Vec::new();
    for code in 1..=23 {
        let params: &[u32] = match code {
            14 => &[1, 2, 3, 4],
            20 | 21 => &[1, 2, 3],
            22 => &[1, 2],
            _ => &[0],
        };
        doors.extend(params.iter().map(|&param| (code, param)));
    }
    doors
}

/// The 1.0 export of a text door on `input`.
fn old_text(
    door: Door,
    param: u32,
    format: *const RawNumFormat,
    periods: *const RawDayPeriods,
    input: &[u8],
    label: &str,
) -> Outcome {
    let (p, l) = (input.as_ptr(), input.len());
    old(text_kind(door), label, |out, fault| unsafe {
        match door {
            Door::Bool => cast_bool(p, l, out, fault),
            Door::Char => cast_char(p, l, out.cast(), fault),
            Door::I8 => cast_i8(p, l, format, out.cast(), fault),
            Door::I16 => cast_i16(p, l, format, out.cast(), fault),
            Door::I32 => cast_i32(p, l, format, out.cast(), fault),
            Door::I64 => cast_i64(p, l, format, out.cast(), fault),
            Door::U8 => cast_u8(p, l, format, out.cast(), fault),
            Door::U16 => cast_u16(p, l, format, out.cast(), fault),
            Door::U32 => cast_u32(p, l, format, out.cast(), fault),
            Door::U64 => cast_u64(p, l, format, out.cast(), fault),
            Door::F32 => cast_f32(p, l, format, out.cast(), fault),
            Door::F64 => cast_f64(p, l, format, out.cast(), fault),
            Door::Decimal => cast_decimal(p, l, format, out.cast(), fault),
            Door::Uuid => cast_uuid(p, l, out, fault),
            Door::Timestamp => cast_timestamp(p, l, out.cast(), fault),
            Door::Unix(_) => cast_unix(p, l, param, out.cast(), fault),
            Door::ExcelSerial(_) => cast_excel_serial(p, l, param, out.cast(), fault),
            Door::Date => cast_date(p, l, out.cast(), fault),
            Door::DateOrdered(_) => cast_date_ordered(p, l, param, out.cast(), fault),
            Door::DateTime(_) => cast_datetime(p, l, param, periods, out.cast(), fault),
            Door::Time => cast_time(p, l, out.cast(), fault),
            Door::Duration => cast_duration(p, l, out.cast(), fault),
            Door::Text => cast_text(p, l, out.cast(), fault),
            other => panic!("no door {other:?}"),
        }
    })
}

/// What `cast_detailed` is handed beside the door and the text.
#[derive(Clone, Copy)]
struct Declared<'a> {
    format: *const RawNumFormat,
    periods: *const RawDayPeriods,
    options: *const RawDateOptions,
    vocab: (*const u8, usize),
    _life: core::marker::PhantomData<&'a ()>,
}

impl Declared<'_> {
    const NONE: Declared<'static> = Declared {
        format: core::ptr::null(),
        periods: core::ptr::null(),
        options: core::ptr::null(),
        vocab: (core::ptr::null(), 0),
        _life: core::marker::PhantomData,
    };
}

fn new_text(
    kind: Kind,
    code: u32,
    param: u32,
    d: Declared<'_>,
    input: &[u8],
    label: &str,
) -> Outcome {
    new(kind, label, |out, fault| unsafe {
        cast_detailed(
            code,
            param,
            d.format,
            d.periods,
            d.options,
            d.vocab.0,
            d.vocab.1,
            input.as_ptr(),
            input.len(),
            out,
            fault,
        )
    })
}

/// The Rust `detailed` door's verdict as an export's fields.
fn rust_text(
    door: Door,
    input: &[u8],
    format: &ValidNumFormat,
    periods: &DayPeriods,
    options: Option<&ValidDateOptions<'_>>,
) -> Result<Vec<u128>, Failure> {
    use detailed as d;
    let whole = |v: i128| vec![v as u128];
    Ok(match door {
        Door::Bool => vec![u128::from(d::cast_bool(input)?)],
        Door::Char => vec![u128::from(u32::from(d::cast_char(input)?))],
        Door::I8 => whole(d::cast_i8(input, format)?.into()),
        Door::I16 => whole(d::cast_i16(input, format)?.into()),
        Door::I32 => whole(d::cast_i32(input, format)?.into()),
        Door::I64 => whole(d::cast_i64(input, format)?.into()),
        Door::U8 => whole(d::cast_u8(input, format)?.into()),
        Door::U16 => whole(d::cast_u16(input, format)?.into()),
        Door::U32 => whole(d::cast_u32(input, format)?.into()),
        Door::U64 => whole(d::cast_u64(input, format)?.into()),
        Door::F32 => vec![d::cast_f32(input, format)?.to_bits().into()],
        Door::F64 => vec![d::cast_f64(input, format)?.to_bits().into()],
        Door::Decimal => decimal(d::cast_decimal(input, format)?),
        Door::Uuid => vec![u128::from_ne_bytes(d::cast_uuid(input)?)],
        Door::Timestamp => timestamp(d::cast_timestamp(input, options)?),
        Door::Unix(precision) => timestamp(d::cast_unix(input, precision)?),
        Door::ExcelSerial(epoch) => timestamp(d::cast_excel_serial(input, epoch)?),
        Door::Date => date(d::cast_date(input, options)?),
        Door::DateOrdered(order) => date(d::cast_date_ordered(input, order, options)?),
        Door::DateTime(order) => civil(d::cast_datetime_with(input, order, periods, options)?),
        Door::Time => vec![d::cast_time(input)?.into()],
        Door::Duration => duration(d::cast_duration(input)?),
        Door::Text => vec![0, d::cast_text(input)?.len() as u128],
        other => panic!("no door {other:?}"),
    })
}

/// A Rust verdict as the outcome a detailed export reports for it.
fn as_outcome(verdict: Result<Vec<u128>, Failure>) -> Outcome {
    match verdict {
        Ok(value) => Outcome { code: 0, value: Some(value), span: None, detail: None, check: None },
        Err(failure) => Outcome {
            code: failure.reason as i32,
            value: None,
            span: Some((failure.offset, failure.len)),
            detail: Some(failure.detail.code()),
            check: None,
        },
    }
}

/// The 1.0 outcome with the detail the detailed export added: what the two must agree on.
fn with_detail(old: &Outcome, new: &Outcome) -> Outcome {
    if (1..=3).contains(&new.code) {
        let detail = new.detail.expect("a failure names its detail");
        assert!(Detail::from_code(detail).is_some(), "detail {detail} is a published code");
    }
    Outcome { detail: new.detail, ..old.clone() }
}

// ---------------------------------------------------------------------------------------
// The corpus.

fn corpus(name: &str) -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus").join(name);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn input_of(row: &Value) -> Vec<u8> {
    match (row.get("input"), row.get("input_bytes")) {
        (_, Some(bytes)) => {
            bytes.as_array().unwrap().iter().map(|b| b.as_u64().unwrap() as u8).collect()
        }
        (Some(text), _) => text.as_str().unwrap().as_bytes().to_vec(),
        _ => panic!("no input"),
    }
}

/// The door files and the door each row is replayed at.
const DOOR_FILES: [&str; 15] = [
    "boolean.json",
    "char.json",
    "integer.json",
    "real.json",
    "decimal.json",
    "uuid.json",
    "timestamp.json",
    "unix.json",
    "excel_serial.json",
    "date.json",
    "time.json",
    "duration.json",
    "date_order.json",
    "datetime.json",
    "constraints.json",
];

/// The door a row names, as `(code, param)`.
fn row_door(file: &str, row: &Value) -> (u32, u32) {
    let number = |field: &str| row[field].as_u64().unwrap() as u32;
    match file {
        "boolean.json" => (1, 0),
        "char.json" => (23, 0),
        "integer.json" | "real.json" => (named_door(row["type"].as_str().unwrap()), 0),
        "decimal.json" => (19, 0),
        "uuid.json" => (12, 0),
        "timestamp.json" => (13, 0),
        "unix.json" => (14, number("precision")),
        "excel_serial.json" => (22, number("epoch")),
        "date.json" => (15, 0),
        "time.json" => (16, 0),
        "duration.json" => (17, 0),
        "date_order.json" => (20, number("order")),
        "datetime.json" => (21, number("order")),
        "constraints.json" => (named_door(row["door"].as_str().unwrap()), 0),
        other => panic!("{other}"),
    }
}

fn named_door(name: &str) -> u32 {
    match name {
        "i8" => 2,
        "i16" => 3,
        "i32" => 4,
        "i64" => 5,
        "u8" => 6,
        "u16" => 7,
        "u32" => 8,
        "u64" => 9,
        "f32" => 10,
        "f64" => 11,
        "text" => 18,
        "decimal" => 19,
        other => panic!("door {other}"),
    }
}

fn grouping_of(format: &Value) -> u32 {
    match format.get("grouping").and_then(Value::as_str) {
        None => format["flags"].as_u64().unwrap() as u32 & NumFormat::GROUPING_POLICY,
        Some("western") => NumFormat::GROUPING_WESTERN,
        Some("indian") => NumFormat::GROUPING_INDIAN,
        Some(other) => panic!("grouping {other}"),
    }
}

/// The format a row declares, `None` for none (the invariant one).
fn format_of(row: &Value) -> Option<NumFormat> {
    let format = row.get("format")?;
    let symbol = |field: &str| -> Symbol<8> {
        format.get(field).and_then(Value::as_str).map_or(Symbol::NONE, |t| Symbol::new(t).unwrap())
    };
    let currency = format
        .get("currency")
        .and_then(Value::as_str)
        .map_or(CurrencySymbol::NONE, |t| CurrencySymbol::new(t).unwrap());
    let digits = format
        .get("digit_zero")
        .and_then(Value::as_str)
        .map_or(NativeDigits::NONE, |z| NativeDigits::new(z.chars().next().unwrap()).unwrap());
    Some(
        NumFormat::with_separators(
            symbol("decimal_sep"),
            symbol("group_sep"),
            format["flags"].as_u64().unwrap() as u32,
        )
        .with_currency(currency)
        .with_currency_separators(symbol("currency_decimal_sep"), symbol("currency_group_sep"))
        .with_signs(symbol("minus_sign"), symbol("plus_sign"))
        .with_percent(symbol("percent"))
        .with_digits(digits)
        .with_grouping(Grouping::from_code(grouping_of(format)).expect("a policy")),
    )
}

/// The day periods a datetime row declares, `None` for none (the English ones).
fn periods_of(row: &Value) -> Option<DayPeriods> {
    row.get("am")?;
    let designator = |field: &str| -> Symbol<16> {
        row.get(field).and_then(Value::as_str).map_or(Symbol::NONE, |t| Symbol::new(t).unwrap())
    };
    Some(DayPeriods::new(designator("am"), designator("pm")))
}

fn flag_bits_of(options: &Value) -> u32 {
    if let Some(bits) = options.get("flag_bits") {
        return u32::try_from(bits.as_u64().unwrap()).unwrap();
    }
    options.get("flags").map_or(0, |flags| {
        flags.as_array().unwrap().iter().fold(0, |bits, flag| {
            bits | match flag.as_str().unwrap() {
                "compact_date" => DateOptions::COMPACT_DATE,
                "space_separator" => DateOptions::SPACE_SEPARATOR,
                "hour_offset" => DateOptions::HOUR_OFFSET,
                "common_forms" => DateOptions::COMMON_FORMS.flags(),
                other => panic!("flag {other}"),
            }
        })
    })
}

/// A row's pivot as the C ABI carries it, a `u32`: never truncated to fit.
fn pivot_of(options: &Value) -> u32 {
    options.get("pivot").map_or(0, |p| u32::try_from(p.as_u64().unwrap()).unwrap())
}

fn pack_months(options: &Value) -> Option<Vec<u8>> {
    let months = options.get("months")?.as_array().unwrap();
    let mut packed = Vec::new();
    for pair in months {
        let name = pair[1].as_str().unwrap();
        packed.push(pair[0].as_u64().unwrap() as u8);
        packed.extend_from_slice(&(name.len() as u32).to_le_bytes());
        packed.extend_from_slice(name.as_bytes());
    }
    Some(packed)
}

fn reason_code(expect: &str) -> i32 {
    match expect {
        "ok" => 0,
        "empty" => 1,
        "malformed" => 2,
        "out_of_range" => 3,
        "violation" => 4,
        other => panic!("expect {other}"),
    }
}

fn detail_named(name: &str) -> u32 {
    (1..=32)
        .filter_map(Detail::from_code)
        .find(|detail| detail.name() == name)
        .unwrap_or_else(|| panic!("detail {name}"))
        .code()
}

/// Asserts what a row pins: its code, its span and its detail.
fn assert_row(row: &Value, got: &Outcome, label: &str) {
    assert_eq!(got.code, reason_code(row["expect"].as_str().unwrap()), "{label}: code");
    if let Some(span) = row.get("fault") {
        let span = (span[0].as_u64().unwrap() as u32, span[1].as_u64().unwrap() as u32);
        assert_eq!(got.span, Some(span), "{label}: span");
    }
    match row.get("detail") {
        Some(name) => {
            assert_eq!(got.detail, Some(detail_named(name.as_str().unwrap())), "{label}: detail")
        }
        None if got.code == 4 => assert_eq!(got.detail, Some(0), "{label}: a violation's detail"),
        None => assert_eq!(got.detail, None, "{label}: no detail"),
    }
}

// ---------------------------------------------------------------------------------------
// The differential.

/// Every row of every door file at its own door, through the 1.0 export and `cast_detailed`
/// with the row's format at Western grouping, its day periods and no date options, and then
/// every corpus input at every door with nothing declared. The detailed outcome must be the
/// old one with a detail. Where the row declares nothing the old export ignores (no grouping
/// policy, no options), its own detail and span are asserted too.
#[test]
fn every_old_text_export_agrees_with_cast_detailed() {
    let mut inputs = std::collections::BTreeSet::new();
    let mut rows = 0;
    for file in DOOR_FILES {
        for row in corpus(file) {
            if row.get("input").is_none() && row.get("input_bytes").is_none() {
                continue;
            }
            let input = input_of(&row);
            inputs.insert(input.clone());
            if file == "constraints.json" {
                continue;
            }
            let (code, param) = row_door(file, &row);
            let door = Door::from_code(code, param).unwrap();
            let label = format!("{file} {row}");
            let declared_format = format_of(&row);
            let western =
                declared_format.map(|f| RawNumFormat::from(f.with_grouping(Grouping::Western)));
            let format = western.as_ref().map_or(core::ptr::null(), |f| f as *const _);
            let periods = periods_of(&row).map(RawDayPeriods::from);
            let periods = periods.as_ref().map_or(core::ptr::null(), |p| p as *const _);
            let before = old_text(door, param, format, periods, &input, &label);
            let declared = Declared { format, periods, ..Declared::NONE };
            let after = new_text(text_kind(door), code, param, declared, &input, &label);
            assert_eq!(with_detail(&before, &after), after, "{label}");
            let western_row = declared_format.is_none_or(|f| {
                f.flags & NumFormat::GROUPING_POLICY == NumFormat::GROUPING_WESTERN
            });
            if western_row && row.get("options").is_none() {
                assert_row(&row, &after, &label);
            }
            rows += 1;
        }
    }
    assert!(rows > 1500, "replayed only {rows} rows");
    // Text that is not UTF-8 where a door fails: the detail is `invalid_utf8` there.
    for bytes in [&b"1\xFF"[..], b"\xC3", b"12\xE2\x82", b"\xFF", b"2026-01-0\xFF", b"U+\xC3"] {
        inputs.insert(bytes.to_vec());
    }
    let invariant = ValidNumFormat::INVARIANT;
    let mut calls = 0;
    for input in &inputs {
        for (code, param) in every_text_door() {
            let door = Door::from_code(code, param).unwrap();
            let label = format!("door {code}/{param} {:?}", String::from_utf8_lossy(input));
            let null = core::ptr::null();
            let before = old_text(door, param, null, core::ptr::null(), input, &label);
            let after = new_text(text_kind(door), code, param, Declared::NONE, input, &label);
            assert_eq!(with_detail(&before, &after), after, "{label}");
            let rust = rust_text(door, input, &invariant, &DayPeriods::ENGLISH, None);
            assert_eq!(after, as_outcome(rust), "{label}: the Rust door");
            calls += 1;
        }
    }
    assert!(calls > 30_000, "cross-fed only {calls} calls");
}

/// Every row with its own declarations through `cast_detailed`: its format at its own grouping
/// policy, its day periods, its date options and vocabulary. The export must report what the
/// Rust `detailed` door reports, and what the row pins. A vocabulary with no flags and no pivot
/// travels without a `RawDateOptions` (the two are independent).
#[test]
fn cast_detailed_reads_every_declaration_as_the_rust_doors_do() {
    let mut indian = 0;
    let mut optioned = 0;
    for file in DOOR_FILES {
        if file == "constraints.json" {
            continue;
        }
        for row in corpus(file) {
            let input = input_of(&row);
            let (code, param) = row_door(file, &row);
            let door = Door::from_code(code, param).unwrap();
            let label = format!("{file} {row}");
            let format = format_of(&row).unwrap_or(NumFormat::INVARIANT);
            let raw_format = RawNumFormat::from(format);
            let periods = periods_of(&row).unwrap_or(DayPeriods::ENGLISH);
            let raw_periods = RawDayPeriods::from(periods);
            let options = row.get("options");
            let packed = options.and_then(pack_months);
            let flags = options.map_or(0, flag_bits_of);
            let pivot = options.map_or(0, pivot_of);
            let raw_options = RawDateOptions { size: RawDateOptions::SIZE, flags, pivot };
            let declared = Declared {
                format: &raw_format,
                periods: &raw_periods,
                options: if flags == 0 && pivot == 0 { core::ptr::null() } else { &raw_options },
                vocab: packed.as_ref().map_or((core::ptr::null(), 0), |v| (v.as_ptr(), v.len())),
                _life: core::marker::PhantomData,
            };
            let got = new_text(text_kind(door), code, param, declared, &input, &label);

            let months = packed.as_deref().map(|bytes| MonthNames::from_packed(bytes).unwrap());
            let valid = options.map(|_| {
                let pivot = u16::try_from(pivot).expect("a door row's pivot fits");
                DateOptions::from_flags(flags).with_pivot(pivot).validate(months).unwrap()
            });
            let want = as_outcome(rust_text(
                door,
                &input,
                &format.validate().unwrap(),
                &periods,
                valid.as_ref(),
            ));
            assert_eq!(got, want, "{label}");
            assert_row(&row, &got, &label);
            indian += usize::from(format.flags & NumFormat::GROUPING_POLICY != 0);
            optioned += usize::from(options.is_some());
        }
    }
    assert!(indian > 50 && optioned > 150, "indian {indian}, optioned {optioned}");
}

/// The doubles `cast_detailed_from_f64` is tried with: every `typed.json` row's, and edges.
fn doubles() -> Vec<f64> {
    let mut values: Vec<f64> = corpus("typed.json")
        .iter()
        .map(|row| f64::from_bits(u64::from_str_radix(row["bits"].as_str().unwrap(), 16).unwrap()))
        .collect();
    values.extend([
        0.0,
        -0.0,
        0.5,
        -1.0,
        59.0,
        60.0,
        61.0,
        1e300,
        -1e300,
        f64::MIN_POSITIVE / 2.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        2958465.999999,
        2958466.0,
    ]);
    values
}

/// The typed C door of a `typed.json` row, as `(code, param)`.
fn typed_door(row: &Value) -> (u32, u32) {
    match row["door"].as_str().unwrap() {
        "decimal" => (19, 0),
        "excel_serial" => (22, row["epoch"].as_u64().unwrap() as u32),
        "excel_time" => (16, 0),
        "excel_duration" => (17, 0),
        other => panic!("typed door {other}"),
    }
}

fn old_typed(door: Door, param: u32, value: f64, label: &str) -> Outcome {
    old(typed_kind(door), label, |out, fault| unsafe {
        match door {
            Door::Decimal => cast_decimal_from_f64(value, out.cast(), fault),
            Door::ExcelSerial(_) => cast_excel_serial_from_f64(value, param, out.cast(), fault),
            Door::Time => cast_excel_time(value, out.cast(), fault),
            Door::Duration => cast_excel_duration(value, out.cast(), fault),
            other => panic!("{other:?}"),
        }
    })
}

fn new_typed(code: u32, param: u32, value: f64, label: &str) -> Outcome {
    let kind = typed_kind(Door::from_code(code, param).unwrap());
    new(kind, label, |out, fault| unsafe { cast_detailed_from_f64(code, param, value, out, fault) })
}

fn rust_typed(door: Door, value: f64) -> Result<Vec<u128>, Failure> {
    Ok(match door {
        Door::Decimal => decimal(detailed::decimal_from_f64(value)?),
        Door::ExcelSerial(epoch) => civil(detailed::excel_serial(value, epoch)?),
        Door::Time => vec![detailed::excel_time(value)?.into()],
        Door::Duration => duration(detailed::excel_duration(value)?),
        other => panic!("{other:?}"),
    })
}

/// The four typed exports against `cast_detailed_from_f64`, and both against the Rust
/// `detailed` typed doors: every `typed.json` row at its door (its pinned detail asserted),
/// then every double at every typed door.
#[test]
fn the_typed_exports_agree_with_cast_detailed_from_f64() {
    for row in corpus("typed.json") {
        let value = f64::from_bits(u64::from_str_radix(row["bits"].as_str().unwrap(), 16).unwrap());
        let (code, param) = typed_door(&row);
        let door = Door::from_code(code, param).unwrap();
        let label = format!("typed {row}");
        let before = old_typed(door, param, value, &label);
        let after = new_typed(code, param, value, &label);
        assert_eq!(with_detail(&before, &after), after, "{label}");
        assert_eq!(after, as_outcome(rust_typed(door, value)), "{label}: the Rust door");
        assert_row(&row, &after, &label);
    }
    for value in doubles() {
        for (code, param) in [(19, 0), (22, 1), (22, 2), (16, 0), (17, 0)] {
            let door = Door::from_code(code, param).unwrap();
            let label = format!("typed {code}/{param} {value:?}");
            let before = old_typed(door, param, value, &label);
            let after = new_typed(code, param, value, &label);
            assert_eq!(with_detail(&before, &after), after, "{label}");
            assert_eq!(after, as_outcome(rust_typed(door, value)), "{label}: the Rust door");
        }
    }
}

/// A constraints row's declaration in its ABI form, every keyword it names.
fn declaration(door: u32, c: &Value) -> Option<(RawConstraints, Vec<u8>)> {
    use RawConstraints as Raw;
    let dec =
        |field: &str| c.get(field).map(|v| hypercast::decimal_for_declaration(v.as_str().unwrap()));
    let mut raw = Raw::ZERO;
    for (field, flag, slot) in [
        ("minimum", Raw::HAS_MINIMUM, 0),
        ("exclusiveMinimum", Raw::HAS_EXCLUSIVE_MINIMUM, 1),
        ("maximum", Raw::HAS_MAXIMUM, 2),
        ("exclusiveMaximum", Raw::HAS_EXCLUSIVE_MAXIMUM, 3),
    ] {
        if let Some(value) = dec(field) {
            raw.flags |= flag;
            *(match slot {
                0 => &mut raw.minimum,
                1 => &mut raw.exclusive_minimum,
                2 => &mut raw.maximum,
                _ => &mut raw.exclusive_maximum,
            }) = value.ok()?;
        }
    }
    if let Some(value) = dec("multipleOf") {
        raw.flags |= Raw::HAS_MULTIPLE_OF;
        raw.multiple_of = value.ok()?;
    }
    raw.rounding = match c.get("rounding").and_then(Value::as_str) {
        Some("midpoint") => 1,
        Some("floor") => 2,
        Some("ceiling") => 3,
        _ => 0,
    };
    if let Some(length) = c.get("minLength") {
        raw.flags |= Raw::HAS_MIN_LENGTH;
        raw.min_length = length.as_u64().unwrap() as u32;
    }
    if let Some(length) = c.get("maxLength") {
        raw.flags |= Raw::HAS_MAX_LENGTH;
        raw.max_length = length.as_u64().unwrap() as u32;
    }
    if c.get("trim").and_then(Value::as_bool) == Some(true) {
        raw.flags |= Raw::TRIM;
    }
    if c.get("caseSensitive").and_then(Value::as_bool) == Some(true) {
        raw.flags |= Raw::CASE_SENSITIVE;
    }
    let mut entries = Vec::new();
    if c.get("enum").is_some() {
        raw.flags |= Raw::HAS_ENUM;
    }
    for entry in c.get("enum").and_then(Value::as_array).into_iter().flatten() {
        let entry = entry.as_str().unwrap();
        if door == 18 {
            entries.extend_from_slice(&(entry.len() as u32).to_le_bytes());
            entries.extend_from_slice(entry.as_bytes());
        } else {
            let d = hypercast::decimal_for_declaration(entry).ok()?;
            entries.extend_from_slice(&d.lo.to_ne_bytes());
            entries.extend_from_slice(&d.hi.to_ne_bytes());
            entries.extend_from_slice(&[d.scale, d.negative.into(), 0, 0]);
        }
    }
    Some((raw, entries))
}

fn from_f64_of(row: &Value) -> Option<f64> {
    row.get("from_f64").map(|value| match value.as_str() {
        Some("-0") => -0.0,
        Some("NaN") => f64::NAN,
        Some("Infinity") => f64::INFINITY,
        Some("-Infinity") => f64::NEG_INFINITY,
        Some(other) => panic!("from_f64 {other}"),
        None => value.as_f64().unwrap(),
    })
}

/// `cast_checked` and `cast_checked_from_f64` against their detailed pair over every
/// `constraints.json` row, refused declarations included (tried with the text `0` and the
/// double `0`): the same code, value, span and check, and a detail on every parse failure (the
/// row's) and `0` on every violation.
#[test]
fn the_checked_exports_agree_with_their_detailed_pair() {
    let mut rows = 0;
    for row in corpus("constraints.json") {
        let door = named_door(row["door"].as_str().unwrap());
        let Some((raw, entries)) = declaration(door, &row["constraints"]) else { continue };
        let label = format!("constraints {row}");
        let kind = text_kind(Door::from_code(door, 0).unwrap());
        let declared = format_of(&row);
        let western = declared.map(|f| RawNumFormat::from(f.with_grouping(Grouping::Western)));
        let format = western.as_ref().map_or(core::ptr::null(), |f| f as *const _);
        let indian = declared.filter(|f| f.flags & NumFormat::GROUPING_POLICY != 0);
        let (e, n) = (entries.as_ptr(), entries.len());
        let refused = row["expect"] == "refused";
        let value = from_f64_of(&row).or(refused.then_some(0.0));
        if let Some(value) = value {
            let before = old_checked(kind, &label, |out, fault, check| unsafe {
                cast_checked_from_f64(door, value, &raw, e, n, out.cast(), fault, check)
            });
            let after = new_checked(kind, &label, |out, fault, check| unsafe {
                cast_checked_detailed_from_f64(door, 0, &raw, e, n, value, out, fault, check)
            });
            assert_checked_pair(&before, &after, &label);
            if !refused {
                assert_row(&row, &after, &label);
            }
        }
        if value.is_none() || refused {
            let text = if refused { b"0".to_vec() } else { input_of(&row) };
            let (p, l) = (text.as_ptr(), text.len());
            let before = old_checked(kind, &label, |out, fault, check| unsafe {
                cast_checked(door, 0, format, &raw, e, n, p, l, out.cast(), fault, check)
            });
            let after = new_checked(kind, &label, |out, fault, check| unsafe {
                cast_checked_detailed(door, 0, format, &raw, e, n, p, l, out, fault, check)
            });
            assert_checked_pair(&before, &after, &label);
            match indian.map(RawNumFormat::from) {
                // The Indian rows, tested apart: the detailed pair under the row's own policy.
                Some(indian) => {
                    let after = new_checked(kind, &label, |out, fault, check| unsafe {
                        cast_checked_detailed(door, 0, &indian, &raw, e, n, p, l, out, fault, check)
                    });
                    assert_row(&row, &after, &label);
                }
                None if !refused => assert_row(&row, &after, &label),
                None => {}
            }
        }
        rows += 1;
    }
    assert!(rows > 300, "replayed only {rows} rows");
}

fn assert_checked_pair(before: &Outcome, after: &Outcome, label: &str) {
    match after.code {
        4 => assert_eq!(after.detail, Some(0), "{label}: a violation's detail is 0"),
        1..=3 => assert!(after.detail.is_some_and(|d| d >= 1), "{label}: a failure's detail"),
        _ => assert_eq!(after.detail, None, "{label}"),
    }
    assert_eq!(Outcome { detail: after.detail, ..before.clone() }, *after, "{label}");
}

// ---------------------------------------------------------------------------------------
// Sentinels.

/// A pointer to nothing: any read through it faults, so passing it where an export must not
/// look proves the export didn't.
fn poisoned<T>() -> *const T {
    core::ptr::without_provenance(16)
}

/// `cast_detailed` at `code`/`param` with `declared`, into a 48-byte `out` and a fault of
/// `size` bytes; returns the code, `out` and the fault buffer.
fn raw_call(
    code: u32,
    param: u32,
    declared: Declared<'_>,
    input: &[u8],
    fault_size: u32,
) -> (i32, [u8; 48], [u32; 8]) {
    let mut out = [0xA5u8; 48];
    let mut fault = [0xC3C3_C3C3u32; 8];
    fault[0] = fault_size;
    // SAFETY: every pointer is live for the call, `out` has room for any value, and `fault`
    // holds 32 aligned bytes.
    let rc = unsafe {
        cast_detailed(
            code,
            param,
            declared.format,
            declared.periods,
            declared.options,
            declared.vocab.0,
            declared.vocab.1,
            input.as_ptr(),
            input.len(),
            out.as_mut_ptr().add(1).cast(),
            fault.as_mut_ptr().cast(),
        )
    };
    (rc, out, fault)
}

fn untouched_fault(size: u32) -> [u32; 8] {
    let mut fault = [0xC3C3_C3C3u32; 8];
    fault[0] = size;
    fault
}

/// A refusal writes nothing: −1, `out` and `fault` as they were.
#[track_caller]
fn assert_refused(code: u32, param: u32, declared: Declared<'_>, input: &[u8], what: &str) {
    let (rc, out, fault) = raw_call(code, param, declared, input, 16);
    assert_eq!(rc, -1, "{what}");
    assert_eq!(out, [0xA5; 48], "{what}: `out` written");
    assert_eq!(fault, untouched_fault(16), "{what}: `fault` written");
}

#[test]
fn a_fault_capacity_under_16_or_not_a_multiple_of_4_is_refused() {
    for size in [0, 4, 12, 15, 17, 18, 19, 21] {
        for input in [&b"12a"[..], b"12"] {
            let (rc, out, fault) = raw_call(4, 0, Declared::NONE, input, size);
            assert_eq!(rc, -1, "size {size}");
            assert_eq!(out, [0xA5; 48], "size {size}: `out` written");
            assert_eq!(fault, untouched_fault(size), "size {size}: `fault` written");
        }
    }
    // The other three exports judge it the same way, before anything else.
    for size in [0, 12, 15, 17] {
        let mut fault = untouched_fault(size);
        let mut out = [0xA5u8; 48];
        let mut check = RawCheck { keyword: 7, adjusted: 7, enum_index: 7 };
        let raw = RawConstraints::ZERO;
        // SAFETY: every pointer is live for its call.
        let codes = unsafe {
            [
                cast_detailed_from_f64(
                    19,
                    0,
                    f64::NAN,
                    out.as_mut_ptr().cast(),
                    fault.as_mut_ptr().cast(),
                ),
                cast_checked_detailed(
                    4,
                    0,
                    core::ptr::null(),
                    &raw,
                    core::ptr::null(),
                    0,
                    b"x".as_ptr(),
                    1,
                    out.as_mut_ptr().cast(),
                    fault.as_mut_ptr().cast(),
                    &mut check,
                ),
                cast_checked_detailed_from_f64(
                    4,
                    0,
                    &raw,
                    core::ptr::null(),
                    0,
                    1.5,
                    out.as_mut_ptr().cast(),
                    fault.as_mut_ptr().cast(),
                    &mut check,
                ),
            ]
        };
        assert_eq!(codes, [-1, -1, -1], "size {size}");
        assert_eq!(out, [0xA5; 48]);
        assert_eq!(fault, untouched_fault(size));
        assert_eq!(check, RawCheck { keyword: 7, adjusted: 7, enum_index: 7 });
    }
}

#[test]
fn a_larger_fault_is_written_in_bytes_4_to_16_only() {
    for size in [16, 20, 32] {
        let (rc, out, fault) = raw_call(4, 0, Declared::NONE, b" 12a", size);
        assert_eq!(rc, 2);
        assert_eq!(out, [0xA5; 48], "a failure writes no value");
        let mut want = untouched_fault(size);
        want[1..4].copy_from_slice(&[3, 1, Detail::UnexpectedCharacter.code()]);
        assert_eq!(fault, want, "size {size}");
    }
    // A success writes no byte of it.
    let (rc, _, fault) = raw_call(4, 0, Declared::NONE, b"12", 32);
    assert_eq!(rc, 0);
    assert_eq!(fault, untouched_fault(32));
}

#[test]
fn a_null_fault_still_returns_the_code() {
    let mut out = [0xA5u8; 8];
    let call = |input: &[u8], out: &mut [u8; 8]| unsafe {
        cast_detailed(
            4,
            0,
            core::ptr::null(),
            core::ptr::null(),
            core::ptr::null(),
            core::ptr::null(),
            0,
            input.as_ptr(),
            input.len(),
            out.as_mut_ptr().cast(),
            core::ptr::null_mut(),
        )
    };
    assert_eq!(call(b"12a", &mut out), 2);
    assert_eq!(out, [0xA5; 8]);
    assert_eq!(call(b"", &mut out), 1);
    assert_eq!(call(b"99999999999", &mut out), 3);
    assert_eq!(call(b"-7", &mut out), 0);
    assert_eq!(i32::from_ne_bytes(out[..4].try_into().unwrap()), -7);
    // SAFETY: every pointer is live for its call.
    let typed = unsafe {
        cast_detailed_from_f64(16, 0, -1.0, out.as_mut_ptr().cast(), core::ptr::null_mut())
    };
    assert_eq!(typed, 2);
}

#[test]
fn out_is_written_unaligned_at_every_offset() {
    for at in 0..8 {
        let mut out = [0u8; 32];
        let mut fault = RawFaultDetail { size: 16, offset: 0, len: 0, detail: 0 };
        let input = b"2026-01-07T15:04:05.5Z";
        // SAFETY: every pointer is live for the call, and `out` has room at `at`.
        let rc = unsafe {
            cast_detailed(
                13,
                0,
                core::ptr::null(),
                core::ptr::null(),
                core::ptr::null(),
                core::ptr::null(),
                0,
                input.as_ptr(),
                input.len(),
                out.as_mut_ptr().add(at).cast(),
                &mut fault,
            )
        };
        assert_eq!(rc, 0);
        assert_eq!(
            Kind::Timestamp.fields(&out[at..]),
            timestamp(hypercast::cast_timestamp(input).unwrap())
        );
    }
}

#[test]
fn pointers_a_door_ignores_are_never_read() {
    let poison = Declared {
        format: poisoned(),
        periods: poisoned(),
        options: poisoned(),
        vocab: (poisoned(), 64),
        _life: core::marker::PhantomData,
    };
    // A date door never reads a format or periods; `cast_date` and `cast_timestamp` never a
    // vocabulary.
    let (rc, ..) =
        raw_call(15, 0, Declared { options: core::ptr::null(), ..poison }, b"2026-01-07", 16);
    assert_eq!(rc, 0, "cast_date");
    let (rc, ..) = raw_call(
        13,
        0,
        Declared { options: core::ptr::null(), ..poison },
        b"2026-01-07T00:00:00Z",
        16,
    );
    assert_eq!(rc, 0, "cast_timestamp");
    let (rc, ..) = raw_call(
        20,
        3,
        Declared { options: core::ptr::null(), vocab: (core::ptr::null(), 0), ..poison },
        b"7/1/2026",
        16,
    );
    assert_eq!(rc, 0, "cast_date_ordered");
    // `i32` reads none of periods, options and vocabulary, and a null vocabulary with a length
    // is no refusal where no vocabulary is read.
    let (rc, out, _) = raw_call(4, 0, Declared { format: core::ptr::null(), ..poison }, b"12", 16);
    assert_eq!((rc, &out[1..5]), (0, &12i32.to_ne_bytes()[..]), "cast_i32");
    let (rc, ..) = raw_call(
        4,
        0,
        Declared { format: core::ptr::null(), vocab: (core::ptr::null(), 9), ..poison },
        b"12",
        16,
    );
    assert_eq!(rc, 0, "cast_i32, a null vocabulary with a length");
    let (rc, ..) = raw_call(
        15,
        0,
        Declared { vocab: (core::ptr::null(), 9), ..Declared::NONE },
        b"2026-01-07",
        16,
    );
    assert_eq!(rc, 0, "cast_date, a null vocabulary with a length");
    // Every door but those that read them, with all four poisoned.
    for (code, param) in every_text_door() {
        let door = Door::from_code(code, param).unwrap();
        let mut declared = poison;
        if door.is_numeric() {
            declared.format = core::ptr::null();
        }
        if matches!(door, Door::DateTime(_)) {
            declared.periods = core::ptr::null();
        }
        if matches!(door, Door::Date | Door::Timestamp | Door::DateOrdered(_) | Door::DateTime(_)) {
            declared.options = core::ptr::null();
        }
        if matches!(door, Door::DateOrdered(_) | Door::DateTime(_)) {
            declared.vocab = (core::ptr::null(), 0);
        }
        let (rc, ..) = raw_call(code, param, declared, b"1", 16);
        assert!((0..=3).contains(&rc), "door {code}/{param}: {rc}");
    }
    // The checked text door never reads a format.
    let raw = RawConstraints::ZERO;
    let mut out = [0u8; 16];
    let mut fault = RawFaultDetail { size: 16, offset: 0, len: 0, detail: 0 };
    let mut check = RawCheck { keyword: 0, adjusted: 0, enum_index: 0 };
    // SAFETY: every pointer read is live; `format` is never read at the text door.
    let rc = unsafe {
        cast_checked_detailed(
            18,
            0,
            poisoned(),
            &raw,
            core::ptr::null(),
            0,
            b"abc".as_ptr(),
            3,
            out.as_mut_ptr().cast(),
            &mut fault,
            &mut check,
        )
    };
    assert_eq!(rc, 0);
}

#[test]
fn doors_and_parameters_outside_an_export_are_refused_before_any_write() {
    let none = Declared::NONE;
    for (code, param, what) in [
        (0, 0, "door 0"),
        (24, 0, "door 24"),
        (99, 0, "door 99"),
        (u32::MAX, 0, "door u32::MAX"),
        (4, 1, "a parameter at i32"),
        (1, 7, "a parameter at bool"),
        (23, 1, "a parameter at char"),
        (18, 1, "a parameter at text"),
        (14, 0, "precision 0"),
        (14, 5, "precision 5"),
        (20, 0, "order 0"),
        (20, 4, "order 4"),
        (21, 4, "order 4 at datetime"),
        (22, 0, "epoch 0"),
        (22, 3, "epoch 3"),
    ] {
        assert_refused(code, param, none, b"1", what);
    }
    // Review Focus 3: a door valid for one export passed to another.
    let refused_typed = |code: u32, param: u32| {
        let mut out = [0xA5u8; 32];
        let mut fault = untouched_fault(16);
        // SAFETY: every pointer is live for the call.
        let rc = unsafe {
            cast_detailed_from_f64(
                code,
                param,
                1.0,
                out.as_mut_ptr().cast(),
                fault.as_mut_ptr().cast(),
            )
        };
        assert_eq!(rc, -1, "cast_detailed_from_f64 door {code}/{param}");
        assert_eq!(out, [0xA5; 32]);
        assert_eq!(fault, untouched_fault(16));
    };
    refused_typed(18, 0);
    for (code, param) in every_text_door() {
        if ![(19, 0), (22, 1), (22, 2), (16, 0), (17, 0)].contains(&(code, param)) {
            refused_typed(code, param);
        }
    }
    for (code, param) in [(0, 0), (24, 0), (19, 1), (16, 1), (17, 2), (22, 0), (22, 3)] {
        refused_typed(code, param);
    }
    // The checked pair: numeric doors and Text only (from f64, numeric only), no parameter.
    let raw = RawConstraints::ZERO;
    for (code, param, text) in [
        (1, 0, true),
        (12, 0, true),
        (23, 0, true),
        (24, 0, true),
        (4, 1, true),
        (18, 1, true),
        (18, 0, false),
        (1, 0, false),
        (16, 0, false),
        (4, 1, false),
    ] {
        let mut out = [0xA5u8; 32];
        let mut fault = untouched_fault(16);
        let mut check = RawCheck { keyword: 7, adjusted: 7, enum_index: 7 };
        // SAFETY: every pointer is live for the call.
        let rc = unsafe {
            if text {
                cast_checked_detailed(
                    code,
                    param,
                    core::ptr::null(),
                    &raw,
                    core::ptr::null(),
                    0,
                    b"1".as_ptr(),
                    1,
                    out.as_mut_ptr().cast(),
                    fault.as_mut_ptr().cast(),
                    &mut check,
                )
            } else {
                cast_checked_detailed_from_f64(
                    code,
                    param,
                    &raw,
                    core::ptr::null(),
                    0,
                    1.0,
                    out.as_mut_ptr().cast(),
                    fault.as_mut_ptr().cast(),
                    &mut check,
                )
            }
        };
        assert_eq!(rc, -1, "checked door {code}/{param} text {text}");
        assert_eq!((out, fault), ([0xA5; 32], untouched_fault(16)));
        assert_eq!(check, RawCheck { keyword: 7, adjusted: 7, enum_index: 7 });
    }
}

#[test]
fn the_detailed_exports_judge_a_format_by_the_1_1_rules() {
    for flags in [
        NumFormat::ALL | NumFormat::GROUPING_POLICY,
        NumFormat::ALL | 1 << 10,
        NumFormat::ALL | 1 << 31,
    ] {
        let raw =
            RawNumFormat { size: RawNumFormat::SIZE, format: NumFormat::new('.', ',', flags) };
        let declared = Declared { format: &raw, ..Declared::NONE };
        for code in [4, 11, 19] {
            assert_refused(code, 0, declared, b"12,34", &format!("flags {flags:#x} at {code}"));
        }
        let mut out = [0xA5u8; 32];
        let mut fault = untouched_fault(16);
        let mut check = RawCheck { keyword: 7, adjusted: 7, enum_index: 7 };
        let constraints = RawConstraints::ZERO;
        // SAFETY: every pointer is live for the call.
        let rc = unsafe {
            cast_checked_detailed(
                4,
                0,
                &raw,
                &constraints,
                core::ptr::null(),
                0,
                b"12".as_ptr(),
                2,
                out.as_mut_ptr().cast(),
                fault.as_mut_ptr().cast(),
                &mut check,
            )
        };
        assert_eq!(rc, -1, "cast_checked_detailed, flags {flags:#x}");
        assert_eq!((out, fault), ([0xA5; 32], untouched_fault(16)));
    }
    // A layout size the core doesn't read.
    let short = RawNumFormat { size: 29, format: NumFormat::INVARIANT };
    assert_refused(4, 0, Declared { format: &short, ..Declared::NONE }, b"1", "size 29");
    // A stale bit 8 is refused, with or without the rest of `ALL`.
    for flags in [NumFormat::ALL | 1 << 8, 1 << 8] {
        let stale = RawNumFormat::from(NumFormat { flags, ..NumFormat::INVARIANT });
        assert_refused(4, 0, Declared { format: &stale, ..Declared::NONE }, b"1", "stale bit 8");
    }
    // The policy is honoured: `12,34` is a group of the wrong size.
    let western = RawNumFormat::from(NumFormat::INVARIANT);
    let (rc, _, fault) =
        raw_call(4, 0, Declared { format: &western, ..Declared::NONE }, b"12,34", 16);
    assert_eq!((rc, fault[1], fault[2], fault[3]), (2, 2, 3, Detail::GroupSize.code()));
    // So is a first group that starts with a zero, at an integer and a real door alike.
    for code in [4, 11] {
        let (rc, _, fault) =
            raw_call(code, 0, Declared { format: &western, ..Declared::NONE }, b"-0,125", 16);
        assert_eq!((rc, fault[1], fault[2], fault[3]), (2, 1, 1, Detail::GroupSize.code()));
    }
}

/// Review Focus 4: an old export given a 1.1 flag ignores it.
#[test]
fn an_old_export_ignores_a_1_1_flag() {
    let raw = RawNumFormat::from(NumFormat::INVARIANT.with_grouping(Grouping::Indian));
    let mut out = 0i32;
    let mut fault = RawFault { offset: 9, len: 9 };
    // SAFETY: every pointer is live for the call.
    let rc = unsafe { cast_i32(b"1,234,567".as_ptr(), 9, &raw, &mut out, &mut fault) };
    assert_eq!((rc, out), (0, 1234567));
    assert_eq!(fault, RawFault { offset: 9, len: 9 });
}

fn date_options(size: u32, flags: u32, pivot: u32) -> RawDateOptions {
    RawDateOptions { size, flags, pivot }
}

#[test]
fn date_options_are_read_at_sizes_8_and_12_only() {
    let dmy = |options: &RawDateOptions, input: &[u8]| {
        raw_call(20, 3, Declared { options, ..Declared::NONE }, input, 16)
    };
    for size in [0, 4, 7, 9, 10, 11, 13, 16, 24] {
        let options = date_options(size, 0, 0);
        for (code, param) in [(20, 3), (21, 3), (15, 0), (13, 0)] {
            assert_refused(
                code,
                param,
                Declared { options: &options, ..Declared::NONE },
                b"2026-01-07",
                &format!("size {size} at {code}"),
            );
        }
    }
    // Size 8 stops before the pivot: what lies past it is never read.
    let short = date_options(8, 0, 2029);
    let (rc, _, fault) = dmy(&short, b"07/01/26");
    assert_eq!((rc, fault[3]), (2, Detail::FieldWidth.code()), "no pivot at size 8");
    let full = date_options(12, 0, 2029);
    let (rc, out, _) = dmy(&full, b"07/01/26");
    assert_eq!((rc, Kind::Date.fields(&out[1..])), (0, vec![2026, 1, 7]));
    // A size-8 layout's flags are read.
    let compact = date_options(8, DateOptions::COMPACT_DATE, 0);
    let (rc, ..) =
        raw_call(15, 0, Declared { options: &compact, ..Declared::NONE }, b"20260107", 16);
    assert_eq!(rc, 0);
    // Flags and pivots outside the rules, at a door that reads them: refused.
    for (flags, pivot) in [(8, 0), (1 << 31, 0), (0, 1), (0, 99), (0, 10_000), (0, 65_535)] {
        let options = date_options(12, flags, pivot);
        assert_refused(
            15,
            0,
            Declared { options: &options, ..Declared::NONE },
            b"2026-01-07",
            &format!("flags {flags} pivot {pivot}"),
        );
    }
    // A pivot past u16 is refused as itself, never truncated: 65536 + 2029 would otherwise
    // read as 2029.
    for pivot in [65_536, 65_536 + 2029, u32::MAX] {
        let options = date_options(12, 0, pivot);
        assert_refused(
            20,
            3,
            Declared { options: &options, ..Declared::NONE },
            b"07/01/26",
            &format!("pivot {pivot}"),
        );
    }
    // Pivots at the window's edges.
    for (pivot, input, year) in
        [(100, &b"07/01/00"[..], 100), (100, b"07/01/01", 1), (9999, b"07/01/99", 9999)]
    {
        let options = date_options(12, 0, pivot);
        let (rc, out, _) = dmy(&options, input);
        assert_eq!((rc, Kind::Date.fields(&out[1..])[0]), (0, year), "pivot {pivot}");
    }
}

fn english() -> Vec<u8> {
    let names =
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mut packed = Vec::new();
    for (i, name) in names.iter().enumerate() {
        packed.push(i as u8 + 1);
        packed.extend_from_slice(&(name.len() as u32).to_le_bytes());
        packed.extend_from_slice(name.as_bytes());
    }
    packed
}

#[test]
fn the_vocabulary_is_independent_of_the_options_and_judged_on_every_call() {
    let names = english();
    let vocab = (names.as_ptr(), names.len());
    // With null options, a vocabulary opens month names and nothing else.
    let (rc, out, _) = raw_call(20, 3, Declared { vocab, ..Declared::NONE }, b"7-Jan-2026", 16);
    assert_eq!((rc, Kind::Date.fields(&out[1..])), (0, vec![2026, 1, 7]));
    let (rc, ..) = raw_call(20, 3, Declared { vocab, ..Declared::NONE }, b"7-Jan-26", 16);
    assert_eq!(rc, 2, "no pivot without options");
    let (rc, out, _) =
        raw_call(21, 2, Declared { vocab, ..Declared::NONE }, b"Jan/7/2026 3:04 PM", 16);
    assert_eq!((rc, Kind::Civil.fields(&out[1..])[1]), (0, 1));
    // A null pointer with a length is refused before any dereference.
    for (code, param) in [(20, 3), (21, 1)] {
        assert_refused(
            code,
            param,
            Declared { vocab: (core::ptr::null(), 1), ..Declared::NONE },
            b"7-Jan-2026",
            "a null vocabulary with a length",
        );
    }
    // A length of 0 is no vocabulary, whatever the pointer.
    let (rc, ..) =
        raw_call(20, 3, Declared { vocab: (poisoned(), 0), ..Declared::NONE }, b"7/1/2026", 16);
    assert_eq!(rc, 0);
    let (rc, ..) =
        raw_call(20, 3, Declared { vocab: (poisoned(), 0), ..Declared::NONE }, b"7-Jan-2026", 16);
    assert_eq!(rc, 2);
    // A vocabulary the rules refuse, each way.
    let mut cut = names.clone();
    cut.pop();
    let mut missing = names.clone();
    missing.truncate(names.len() - 8);
    let mut duplicate = names.clone();
    duplicate.extend_from_slice(&[1, 3, 0, 0, 0, b'J', b'A', b'N']);
    let mut digit = names.clone();
    digit.extend_from_slice(&[1, 2, 0, 0, 0, b'J', b'1']);
    let mut month = names.clone();
    month.extend_from_slice(&[13, 1, 0, 0, 0, b'X']);
    for (bad, what) in [
        (&cut, "cut short"),
        (&missing, "a month missing"),
        (&duplicate, "a folded duplicate"),
        (&digit, "a digit"),
        (&month, "month 13"),
    ] {
        let options = date_options(12, 0, 2029);
        for options in [core::ptr::null(), &options as *const _] {
            assert_refused(
                20,
                3,
                Declared { options, vocab: (bad.as_ptr(), bad.len()), ..Declared::NONE },
                b"7-Jan-2026",
                what,
            );
        }
        // Where no vocabulary is read, it isn't judged either.
        let (rc, ..) = raw_call(
            15,
            0,
            Declared { vocab: (bad.as_ptr(), bad.len()), ..Declared::NONE },
            b"2026-01-07",
            16,
        );
        assert_eq!(rc, 0, "{what} at cast_date");
    }
}

#[test]
fn a_violation_writes_detail_0_and_a_check_with_no_index() {
    let mut raw = RawConstraints::ZERO;
    raw.flags = RawConstraints::HAS_MAXIMUM;
    raw.maximum = hypercast::decimal_for_declaration("10").unwrap();
    let input = b"  42 ";
    let mut out = [0xA5u8; 16];
    let mut fault = RawFaultDetail { size: 16, offset: 9, len: 9, detail: 9 };
    let mut check = RawCheck { keyword: 9, adjusted: 9, enum_index: 9 };
    // SAFETY: every pointer is live for the call.
    let rc = unsafe {
        cast_checked_detailed(
            4,
            0,
            core::ptr::null(),
            &raw,
            core::ptr::null(),
            0,
            input.as_ptr(),
            input.len(),
            out.as_mut_ptr().cast(),
            &mut fault,
            &mut check,
        )
    };
    assert_eq!(rc, 4);
    // The value as cast, written beside the keyword; nothing past it.
    assert_eq!(i32::from_ne_bytes(out[..4].try_into().unwrap()), 42);
    assert_eq!(out[4..], [0xA5; 12]);
    assert_eq!(fault, RawFaultDetail { size: 16, offset: 2, len: 2, detail: 0 });
    let maximum = hypercast::Keyword::Maximum.code();
    assert_eq!(check, RawCheck { keyword: maximum, adjusted: 0, enum_index: u32::MAX });

    let mut fault = RawFaultDetail { size: 16, offset: 9, len: 9, detail: 9 };
    let mut check = RawCheck { keyword: 9, adjusted: 9, enum_index: 9 };
    // SAFETY: every pointer is live for the call.
    let rc = unsafe {
        cast_checked_detailed_from_f64(
            11,
            0,
            &raw,
            core::ptr::null(),
            0,
            42.5,
            out.as_mut_ptr().cast(),
            &mut fault,
            &mut check,
        )
    };
    assert_eq!(rc, 4);
    assert_eq!(fault, RawFaultDetail { size: 16, offset: 0, len: 0, detail: 0 });
    assert_eq!(check, RawCheck { keyword: maximum, adjusted: 0, enum_index: u32::MAX });
}

#[test]
fn door_23_is_the_char_door() {
    assert_eq!(Door::from_code(23, 0), Some(Door::Char));
    assert_eq!(Door::Char.code(), 23);
    for input in [&b"U+00E9"[..], b"x", b"&#233;", b"U+D800", b"", b"ab"] {
        let label = format!("{:?}", String::from_utf8_lossy(input));
        let before = old_text(Door::Char, 0, core::ptr::null(), core::ptr::null(), input, &label);
        let after = new_text(Kind::Char, 23, 0, Declared::NONE, input, &label);
        assert_eq!(with_detail(&before, &after), after, "{label}");
        let want = as_outcome(rust_text(
            Door::Char,
            input,
            &ValidNumFormat::INVARIANT,
            &DayPeriods::ENGLISH,
            None,
        ));
        assert_eq!(after, want, "{label}");
    }
}

#[test]
fn the_layouts_are_pinned() {
    assert_eq!(size_of::<RawFaultDetail>(), 16);
    assert_eq!(align_of::<RawFaultDetail>(), 4);
    assert_eq!(RawFaultDetail::SIZE, 16);
    assert_eq!(size_of::<RawDateOptions>(), 12);
    assert_eq!(align_of::<RawDateOptions>(), 4);
    assert_eq!((RawDateOptions::SIZE, RawDateOptions::MIN_SIZE), (12, 8));
    assert_eq!(core::mem::offset_of!(RawFaultDetail, offset), 4);
    assert_eq!(core::mem::offset_of!(RawFaultDetail, len), 8);
    assert_eq!(core::mem::offset_of!(RawFaultDetail, detail), 12);
    assert_eq!(core::mem::offset_of!(RawDateOptions, flags), 4);
    assert_eq!(core::mem::offset_of!(RawDateOptions, pivot), 8);
}

/// A checked failure is classified at its span as the Rust `detailed` doors classify it: bytes
/// that don't decode there are `invalid_utf8`.
#[test]
fn a_checked_failure_is_classified_at_its_span() {
    let raw = RawConstraints::ZERO;
    for (input, door) in [(&b"1\xFF"[..], 4), (b"12\xE2\x82", 11), (b"\xC3", 19), (b"\xFFab", 18)] {
        let label = format!("{input:?} at {door}");
        let kind = text_kind(Door::from_code(door, 0).unwrap());
        let got = new_checked(kind, &label, |out, fault, check| unsafe {
            cast_checked_detailed(
                door,
                0,
                core::ptr::null(),
                &raw,
                core::ptr::null(),
                0,
                input.as_ptr(),
                input.len(),
                out,
                fault,
                check,
            )
        });
        let want = rust_text(
            Door::from_code(door, 0).unwrap(),
            input,
            &ValidNumFormat::INVARIANT,
            &DayPeriods::ENGLISH,
            None,
        )
        .unwrap_err();
        assert_eq!(want.detail, Detail::InvalidUtf8, "{label}");
        assert_eq!(got, as_outcome(Err(want)), "{label}");
    }
}

/// Door 23 is `Char` to the detailed exports only: every 1.0 export that takes a door code
/// still names no door there, refused as 1.0 refused it, before `format` is read.
#[test]
fn the_1_0_exports_still_know_no_door_23() {
    let raw = RawConstraints::ZERO;
    for door in [23, 24] {
        let mut out = [0xA5u8; 32];
        let mut fault = RawFault { offset: 7, len: 7 };
        let mut check = RawCheck { keyword: 7, adjusted: 7, enum_index: 7 };
        // SAFETY: every pointer read is live; `format` is poisoned and must not be read.
        let codes = unsafe {
            [
                cast_checked(
                    door,
                    0,
                    poisoned(),
                    &raw,
                    core::ptr::null(),
                    0,
                    b"x".as_ptr(),
                    1,
                    out.as_mut_ptr().cast(),
                    &mut fault,
                    &mut check,
                ),
                cast_checked_from_f64(
                    door,
                    1.0,
                    &raw,
                    core::ptr::null(),
                    0,
                    out.as_mut_ptr().cast(),
                    &mut fault,
                    &mut check,
                ),
            ]
        };
        assert_eq!(codes, [-1, -1], "door {door}");
        assert_eq!(out, [0xA5; 32]);
        assert_eq!(fault, RawFault { offset: 7, len: 7 });
        assert_eq!(check, RawCheck { keyword: 7, adjusted: 7, enum_index: 7 });
        // SAFETY: every pointer is live for the call.
        let refusal = unsafe { cast_constraints_check(door, 0, &raw, core::ptr::null(), 0) };
        assert_eq!(refusal, 1, "cast_constraints_check door {door}: names no door");
    }
}
