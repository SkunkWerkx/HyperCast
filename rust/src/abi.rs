//! The parts of the C ABI that are Rust API too: the version word every binding probes,
//! [`NumFormat`] and the day-period designators as they cross the boundary, the
//! validation constraints' layouts ([`RawConstraints`], [`RawCheck`], [`RawSpan`]), and the
//! detailed exports' fault and date options ([`RawFaultDetail`], [`RawDateOptions`]). They
//! live outside `ffi` because that module is the `exports` feature — the 36 symbols, the
//! version probe and the 35 `cast_*` exports — and a crate that links this one as an rlib to export a
//! C ABI of its own takes the core with `exports` off, so its library carries none of them,
//! while still reading the version and declaring a numeric column in exactly this layout.

use crate::temporal::DayPeriods;
use crate::verdict::{CivilDateTime, Date, Decimal, Duration, NativeDigits, NumFormat, Timestamp};

// The size and alignment of every shape that crosses the ABI, pinned here because the
// bindings restate them by hand (C# structs, Go and PHP C declarations, ctypes in
// cdylib_smoke.py, Swift scratch tuples) and each of those asserts the same numbers. A
// layout change fails to compile here first, and these numbers are what every binding's
// assertion has to move to. `RawFault` is pinned beside its definition in ffi.rs.
//
// Only where a u64 is 8-aligned, which is every target a binding ships for. Where it is
// not (i686's SysV ABI), the shapes are other sizes and no binding runs there, so the Rust
// API still builds.
macro_rules! pin_layout {
    ($($ty:ty => ($size:literal, $align:literal)),+ $(,)?) => {$(
        const _: () = assert!(
            align_of::<u64>() != 8 || (size_of::<$ty>() == $size && align_of::<$ty>() == $align)
        );
    )+};
}

pin_layout! {
    RawNumFormat => (92, 4),
    NumFormat => (88, 4),
    RawDayPeriods => (40, 4),
    DayPeriods => (34, 1),
    Decimal => (16, 8),
    Timestamp => (16, 8),
    Date => (4, 2),
    CivilDateTime => (16, 8),
    Duration => (16, 8),
    RawConstraints => (104, 8),
}

// Every target, not only where a u64 is 8-aligned: none of these layouts holds a u64.
const _: () = assert!(
    RawNumFormat::SIZE as usize == size_of::<RawNumFormat>()
        && RawDayPeriods::SIZE as usize == size_of::<RawDayPeriods>()
        && RawCheck::SIZE as usize == size_of::<RawCheck>()
        && align_of::<RawCheck>() == 4
        && RawSpan::SIZE as usize == size_of::<RawSpan>()
        && align_of::<RawSpan>() == 4
        && RawFaultDetail::SIZE as usize == size_of::<RawFaultDetail>()
        && align_of::<RawFaultDetail>() == 4
        && RawDateOptions::SIZE as usize == size_of::<RawDateOptions>()
        && align_of::<RawDateOptions>() == 4
);
const _: () =
    assert!(align_of::<u64>() != 8 || RawConstraints::SIZE as usize == size_of::<RawConstraints>());

// The offsets the bindings restate, pinned the same way: each symbol's bytes come first
// and its length byte last, with nothing between one field and the next.
const _: () = {
    assert!(core::mem::offset_of!(RawNumFormat, format) == 4);
    assert!(core::mem::offset_of!(NumFormat, flags) == 0);
    assert!(core::mem::offset_of!(NumFormat, digits) == 4);
    assert!(core::mem::offset_of!(NumFormat, decimal_sep) == 8);
    assert!(core::mem::offset_of!(NumFormat, group_sep) == 17);
    assert!(core::mem::offset_of!(NumFormat, currency_decimal_sep) == 26);
    assert!(core::mem::offset_of!(NumFormat, currency_group_sep) == 35);
    assert!(core::mem::offset_of!(NumFormat, minus_sign) == 44);
    assert!(core::mem::offset_of!(NumFormat, plus_sign) == 53);
    assert!(core::mem::offset_of!(NumFormat, percent) == 62);
    assert!(core::mem::offset_of!(NumFormat, currency) == 71);
    assert!(core::mem::offset_of!(RawDayPeriods, periods) == 4);
    assert!(core::mem::offset_of!(DayPeriods, pm) == 17);
    assert!(core::mem::offset_of!(RawCheck, adjusted) == 4);
    assert!(core::mem::offset_of!(RawCheck, enum_index) == 8);
    assert!(core::mem::offset_of!(RawSpan, len) == 4);
    assert!(core::mem::offset_of!(RawFaultDetail, offset) == 4);
    assert!(core::mem::offset_of!(RawFaultDetail, len) == 8);
    assert!(core::mem::offset_of!(RawFaultDetail, detail) == 12);
    assert!(core::mem::offset_of!(RawDateOptions, flags) == 4);
    assert!(core::mem::offset_of!(RawDateOptions, pivot) == 8);
};

// The constraint layout's words come first, then its three decimals, each 8-aligned with
// nothing between: where a u64 is 8-aligned, as the size pin above.
const _: () = assert!(
    align_of::<u64>() != 8
        || (core::mem::offset_of!(RawConstraints, flags) == 4
            && core::mem::offset_of!(RawConstraints, rounding) == 8
            && core::mem::offset_of!(RawConstraints, min_length) == 12
            && core::mem::offset_of!(RawConstraints, max_length) == 16
            && core::mem::offset_of!(RawConstraints, _reserved) == 20
            && core::mem::offset_of!(RawConstraints, lower) == 24
            && core::mem::offset_of!(RawConstraints, upper) == 40
            && core::mem::offset_of!(RawConstraints, multiple_of) == 56
            && core::mem::offset_of!(RawConstraints, exclusive_lower) == 72
            && core::mem::offset_of!(RawConstraints, exclusive_upper) == 88)
);

/// This library's version, packed `major << 16 | minor << 8 | patch` from the crate's own
/// manifest — so a host can prove the library it loaded is the one its binding was built
/// against before making the first cast, and can name the mismatch when it isn't. Takes
/// nothing, touches nothing: the cheapest possible "did the native library resolve" probe.
/// The `exports` feature exports it to C under this name (ffi.rs, which cbindgen can read).
#[inline]
pub extern "C" fn hypercast_version() -> u32 {
    const fn field(text: &str) -> u32 {
        let bytes = text.as_bytes();
        let mut value = 0u32;
        let mut i = 0;
        while i < bytes.len() {
            value = value * 10 + (bytes[i] - b'0') as u32;
            i += 1;
        }
        value
    }
    const VERSION: u32 = (field(env!("CARGO_PKG_VERSION_MAJOR")) << 16)
        | (field(env!("CARGO_PKG_VERSION_MINOR")) << 8)
        | field(env!("CARGO_PKG_VERSION_PATCH"));
    VERSION
}

/// [`NumFormat`] as it crosses the ABI — the size-tagged layout of 0.9.0: a size word, then
/// the format itself, byte for byte, 92 bytes with 4-byte alignment. The core reads the
/// caller's struct in place, so a cast pays for a check of it and no copy. Every symbol is
/// its bytes followed by its length (nine bytes for a separator, a sign or the percent
/// symbol, seventeen for the currency symbol); a length of `0` declares nothing and the
/// core keeps its default for that field, except the two number separators, which a caller
/// always declares. Where an export takes a pointer to one, null means
/// [`NumFormat::INVARIANT`].
///
/// | Offset | Field | Bytes |
/// | --- | --- | --- |
/// | 0 | `size`, a `u32` | 4 |
/// | 4 | `flags`, a `u32` | 4 |
/// | 8 | the native digits' zero code point, a `u32`; `0` for none | 4 |
/// | 12 | `decimal_sep`: 8 bytes of UTF-8, then its length | 9 |
/// | 21 | `group_sep` | 9 |
/// | 30 | `currency_decimal_sep`; length `0` keeps `decimal_sep` | 9 |
/// | 39 | `currency_group_sep`; length `0` keeps `group_sep` | 9 |
/// | 48 | `minus_sign`; length `0` declares none beside ASCII `-` | 9 |
/// | 57 | `plus_sign`; likewise beside `+` | 9 |
/// | 66 | `percent`; likewise beside `%` | 9 |
/// | 75 | `currency`: 16 bytes of UTF-8, then its length; `0` declares none | 17 |
///
/// `size` is first so the layout can grow without another coordinated change: a caller
/// states how many bytes it built, the core reads exactly those and defaults the rest, and a
/// size the core does not know (under 30, or past this layout) is a contract violation. The
/// 0.8.0 layout was 32 bytes with the separators as code points at its head; it has no size
/// field and is not read.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawNumFormat {
    /// `size_of::<RawNumFormat>()` as the caller built it: [`RawNumFormat::SIZE`].
    pub size: u32,
    /// The format, in place.
    pub format: NumFormat,
}

impl RawNumFormat {
    /// The layout's size in bytes, which `size` carries: a literal, so the C header carries
    /// it too, held to `size_of` beside the layout pins.
    pub const SIZE: u32 = 92;

    /// The smallest `size` the core reads: through the group separator's length byte, since
    /// a format with no number separators is no format.
    pub const MIN_SIZE: u32 = 30;

    /// Every byte zero: no flags, nothing declared, and no number separators — not a
    /// format on its own, but what [`resolve`](Self::resolve) fills a shorter caller's
    /// layout out to.
    pub const ZERO: RawNumFormat = RawNumFormat { size: Self::SIZE, format: NumFormat::EMPTY };

    /// The [`NumFormat`] this declares, in place, or `None` for a contract violation — a
    /// caller bug, not a data verdict: a `size` the core does not read, or a format
    /// [`NumFormat::check`] refuses (an undefined `flags` bit or the retired bit 8, which
    /// the 1.0 exports cut away before asking; a missing or over-long separator, equal separators, a
    /// symbol with an ASCII digit in it or that is not UTF-8, white space in a sign, the
    /// percent or the currency symbol or in either decimal separator, a digit zero that
    /// starts no digit block).
    #[inline]
    pub fn resolve(&self) -> Option<&NumFormat> {
        if self.size < Self::MIN_SIZE || self.size > Self::SIZE || !self.format.check() {
            return None;
        }
        Some(&self.format)
    }

    /// The 92 bytes in the layout's own order, little-endian on every host: the portable
    /// form Ruby's and Python's `NumFormat.packed` are, and what a native extension reads
    /// back with [`from_le_bytes`](Self::from_le_bytes) (PHP's does). Not what a C export
    /// takes: the exports read a `RawNumFormat` in place, in the host's own order, so on a
    /// big-endian host its three words must be swapped first.
    pub fn to_le_bytes(&self) -> [u8; 92] {
        let mut bytes = [0u8; 92];
        let mut at = 0;
        let mut put = |source: &[u8]| {
            if let Some(slot) = bytes.get_mut(at..at + source.len()) {
                slot.copy_from_slice(source);
            }
            at += source.len();
        };
        let format = &self.format;
        put(&self.size.to_le_bytes());
        put(&format.flags.to_le_bytes());
        put(&format.digits.zero_code().to_le_bytes());
        for symbol in [
            format.decimal_sep.as_bytes(),
            format.group_sep.as_bytes(),
            format.currency_decimal_sep.as_bytes(),
            format.currency_group_sep.as_bytes(),
            format.minus_sign.as_bytes(),
            format.plus_sign.as_bytes(),
            format.percent.as_bytes(),
        ] {
            let mut field = [0u8; 9];
            if let Some(slot) = field.get_mut(..symbol.len()) {
                slot.copy_from_slice(symbol);
            }
            field[8] = symbol.len() as u8;
            put(&field);
        }
        let symbol = format.currency.as_bytes();
        let mut field = [0u8; 17];
        if let Some(slot) = field.get_mut(..symbol.len()) {
            slot.copy_from_slice(symbol);
        }
        field[16] = symbol.len() as u8;
        put(&field);
        bytes
    }

    /// The layout read back from [`to_le_bytes`](Self::to_le_bytes)'s 92 bytes. Nothing is
    /// checked here: [`resolve`](Self::resolve) says whether the format they spell is one.
    pub fn from_le_bytes(bytes: [u8; 92]) -> RawNumFormat {
        let mut raw = RawNumFormat::ZERO;
        // The symbols are plain bytes in the layout's own order on every host; only the
        // three words are endian-sensitive, and they are decoded as little-endian after.
        // SAFETY: `RawNumFormat` is `#[repr(C)]`, 92 bytes of `u32`s and `u8`s with no
        // padding and no invalid bit patterns (a symbol's length is checked by `resolve`,
        // never trusted), so any 92 bytes are a valid value of it.
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), (&raw mut raw).cast::<u8>(), 92) };
        let word = |at: usize| {
            let mut word = [0u8; 4];
            if let Some(source) = bytes.get(at..at + 4) {
                word.copy_from_slice(source);
            }
            u32::from_le_bytes(word)
        };
        raw.size = word(0);
        raw.format.flags = word(4);
        raw.format.digits = NativeDigits::from_code(word(8));
        raw
    }
}

impl From<NumFormat> for RawNumFormat {
    /// The layout a format crosses the ABI in — [`resolve`](RawNumFormat::resolve)'s inverse.
    fn from(format: NumFormat) -> RawNumFormat {
        RawNumFormat { size: Self::SIZE, format }
    }
}

/// [`DayPeriods`] as they cross the ABI beside a declared date order: a size word, then
/// the designators in place — the AM one's 16 bytes of UTF-8 and its length, then the PM
/// one's — 40 bytes with 4-byte alignment, the last two padding. A length of `0` declares
/// nothing; where an export takes a pointer to one, null means [`DayPeriods::ENGLISH`] —
/// the English `AM`/`PM`, which are read whatever is declared.
///
/// | Offset | Field | Bytes |
/// | --- | --- | --- |
/// | 0 | `size`, a `u32` | 4 |
/// | 4 | `am`: 16 bytes of UTF-8, then its length | 17 |
/// | 21 | `pm` | 17 |
/// | 38 | padding | 2 |
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawDayPeriods {
    /// `size_of::<RawDayPeriods>()` as the caller built it: [`RawDayPeriods::SIZE`].
    pub size: u32,
    /// The designators, in place.
    pub periods: DayPeriods,
}

impl RawDayPeriods {
    /// The layout's size in bytes, which `size` carries: a literal, so the C header carries
    /// it too, held to `size_of` beside the layout pins.
    pub const SIZE: u32 = 40;

    /// The smallest `size` the core reads: the size word alone, which declares nothing.
    pub const MIN_SIZE: u32 = 4;

    /// Nothing declared.
    pub const ZERO: RawDayPeriods =
        RawDayPeriods { size: Self::SIZE, periods: DayPeriods::ENGLISH };

    /// The [`DayPeriods`] this declares, in place, or `None` for a contract violation: a
    /// `size` the core does not read, or a designator that is not valid (longer than 16
    /// bytes, not UTF-8, carrying an ASCII digit, or starting or ending in white space, which
    /// the grammar around it already reads).
    #[inline]
    pub fn resolve(&self) -> Option<&DayPeriods> {
        if self.size < Self::MIN_SIZE || self.size > Self::SIZE || !self.periods.check() {
            return None;
        }
        Some(&self.periods)
    }
}

impl From<DayPeriods> for RawDayPeriods {
    /// The layout the designators cross the ABI in — [`resolve`](RawDayPeriods::resolve)'s
    /// inverse.
    fn from(periods: DayPeriods) -> RawDayPeriods {
        RawDayPeriods { size: Self::SIZE, periods }
    }
}

/// A declaration of validation constraints as it crosses the ABI, bound to a door by the
/// call that takes it: a size word, the flags that say which fields are read, then the
/// five decimals, 104 bytes with 8-byte alignment. Pointer-free, so the layout is the same
/// on 32-bit and 64-bit targets; an `enum` list travels beside it. Built from a
/// [`Constraints`](crate::Constraints) by [`raw`](crate::Constraints::raw), and judged by
/// [`check_constraints`](crate::check_constraints).
///
/// | Offset | Field | Bytes |
/// | --- | --- | --- |
/// | 0 | `size`, a `u32` | 4 |
/// | 4 | `flags`, a `u32` | 4 |
/// | 8 | `rounding`: `0` strict, `1` midpoint, `2` floor, `3` ceiling | 4 |
/// | 12 | `min_length`, read under [`HAS_MIN_LENGTH`](Self::HAS_MIN_LENGTH) | 4 |
/// | 16 | `max_length`, read under [`HAS_MAX_LENGTH`](Self::HAS_MAX_LENGTH) | 4 |
/// | 20 | `_reserved`, always `0` | 4 |
/// | 24 | `lower`: `minimum`, read under [`HAS_MINIMUM`](Self::HAS_MINIMUM) | 16 |
/// | 40 | `upper`: `maximum`, read under [`HAS_MAXIMUM`](Self::HAS_MAXIMUM) | 16 |
/// | 56 | `multiple_of`, read under [`HAS_MULTIPLE_OF`](Self::HAS_MULTIPLE_OF) | 16 |
/// | 72 | `exclusive_lower`: `exclusiveMinimum`, read under [`HAS_EXCLUSIVE_MINIMUM`](Self::HAS_EXCLUSIVE_MINIMUM) | 16 |
/// | 88 | `exclusive_upper`: `exclusiveMaximum`, read under [`HAS_EXCLUSIVE_MAXIMUM`](Self::HAS_EXCLUSIVE_MAXIMUM) | 16 |
///
/// Each keyword has a field of its own, so all four bounds may be declared together, as
/// OpenAPI allows.
///
/// A `size` other than [`SIZE`](Self::SIZE), an unknown flag bit, an unknown rounding code,
/// a nonzero `_reserved`, or (read from a caller's memory across the C ABI) a decimal's sign
/// byte other than `0` or `1` is a malformed layout. The `enum` list is read only under
/// [`HAS_ENUM`](Self::HAS_ENUM).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawConstraints {
    /// `size_of::<RawConstraints>()` as the caller built it: [`RawConstraints::SIZE`].
    pub size: u32,
    /// Which constraints are declared: the `HAS_*` bits, [`TRIM`](Self::TRIM) and
    /// [`CASE_SENSITIVE`](Self::CASE_SENSITIVE).
    pub flags: u32,
    /// How a value that is not a multiple of `multiple_of` is treated: `0` strict, `1`
    /// midpoint (ties to even), `2` floor, `3` ceiling.
    pub rounding: u32,
    /// The fewest code points the text may hold.
    pub min_length: u32,
    /// The most code points the text may hold.
    pub max_length: u32,
    /// Always `0`.
    pub _reserved: u32,
    /// `minimum`: the value may equal it.
    pub lower: Decimal,
    /// `maximum`: the value may equal it.
    pub upper: Decimal,
    /// The step every value must be a multiple of.
    pub multiple_of: Decimal,
    /// `exclusiveMinimum`: the value must exceed it.
    pub exclusive_lower: Decimal,
    /// `exclusiveMaximum`: the value must be below it.
    pub exclusive_upper: Decimal,
}

impl RawConstraints {
    /// The layout's size in bytes, which `size` carries.
    pub const SIZE: u32 = 104;

    /// `lower`, the `minimum`, is read.
    pub const HAS_MINIMUM: u32 = 1;
    /// `exclusive_lower`, the `exclusiveMinimum`, is read.
    pub const HAS_EXCLUSIVE_MINIMUM: u32 = 2;
    /// `upper`, the `maximum`, is read.
    pub const HAS_MAXIMUM: u32 = 4;
    /// `exclusive_upper`, the `exclusiveMaximum`, is read.
    pub const HAS_EXCLUSIVE_MAXIMUM: u32 = 8;
    /// `multiple_of` and `rounding` are read.
    pub const HAS_MULTIPLE_OF: u32 = 16;
    /// `min_length` is read.
    pub const HAS_MIN_LENGTH: u32 = 32;
    /// `max_length` is read.
    pub const HAS_MAX_LENGTH: u32 = 64;
    /// Text is trimmed of Unicode `White_Space` before it is judged.
    pub const TRIM: u32 = 128;
    /// An `enum` list is declared and read: the list beside the layout, which may be empty
    /// (a declaration [`check_constraints`](crate::check_constraints) refuses). Clear, no
    /// list is read, and one passed anyway is a malformed layout.
    pub const HAS_ENUM: u32 = 256;
    /// A text `enum` matches exactly, byte for byte, instead of under simple case folding,
    /// for a vocabulary whose case carries meaning (`mm` and `Mm`). The text door only.
    pub const CASE_SENSITIVE: u32 = 512;

    /// Nothing declared: the size, and every other byte zero.
    pub const ZERO: RawConstraints = {
        const ZERO: Decimal = Decimal { lo: 0, hi: 0, scale: 0, negative: false };
        RawConstraints {
            size: Self::SIZE,
            flags: 0,
            rounding: 0,
            min_length: 0,
            max_length: 0,
            _reserved: 0,
            lower: ZERO,
            upper: ZERO,
            multiple_of: ZERO,
            exclusive_lower: ZERO,
            exclusive_upper: ZERO,
        }
    };
}

/// What a checked door reports beside its value, 12 bytes with 4-byte alignment.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawCheck {
    /// On a violation, the keyword broken (`1` minimum … `8` enum); `0` otherwise.
    pub keyword: u32,
    /// On success, `1` when rounding changed the value, else `0`.
    pub adjusted: u32,
    /// On a success at the text door that matched an `enum` entry, that entry's index;
    /// `u32::MAX` otherwise.
    pub enum_index: u32,
}

impl RawCheck {
    /// The layout's size in bytes.
    pub const SIZE: u32 = 12;
}

/// A span of the caller's own input, 8 bytes with 4-byte alignment: what the text door
/// writes in place of a copy.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawSpan {
    /// The byte offset where the span starts.
    pub offset: u32,
    /// The span's length in bytes.
    pub len: u32,
}

impl RawSpan {
    /// The layout's size in bytes.
    pub const SIZE: u32 = 8;
}

/// A detailed export's fault: the span and the [`Detail`](crate::Detail) code, behind a size
/// word the caller writes, 16 bytes with 4-byte alignment.
///
/// | Offset | Field | Bytes |
/// | --- | --- | --- |
/// | 0 | `size`, a `u32`, written by the caller | 4 |
/// | 4 | `offset`, where the span starts in the caller's input | 4 |
/// | 8 | `len`, the span's length in bytes | 4 |
/// | 12 | `detail`, a [`Detail`](crate::Detail) code, or `0` for a checked violation | 4 |
///
/// `size` says how many bytes the caller provides: at least [`SIZE`](Self::SIZE) and a
/// multiple of 4, or the export refuses the call (−1) and writes nothing. A larger size, from
/// a caller built against a later layout, is accepted: this core writes bytes 4 to 16 and
/// never a byte past them, so such a caller sets any field beyond those to its neutral value
/// itself. The core never writes `size`, and writes the rest only on codes 1 to 3 (and on 4
/// at the checked exports); a null pointer is allowed and nothing is written.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawFaultDetail {
    /// The bytes the caller provides: [`RawFaultDetail::SIZE`] or more, a multiple of 4.
    pub size: u32,
    /// Where the offending bytes start, as an offset into the caller's input.
    pub offset: u32,
    /// How many bytes they run; 0 for a typed door, which has no text to index.
    pub len: u32,
    /// What failed: a [`Detail`](crate::Detail) code, 1 or more; `0` on a checked violation.
    /// A code this binding doesn't know is still a failure with a usable reason and span.
    pub detail: u32,
}

impl RawFaultDetail {
    /// The layout's size in bytes, the least `size` the core accepts.
    pub const SIZE: u32 = 16;
}

/// The date options as they cross the ABI: a size word, the option bits
/// ([`DateOptions::COMPACT_DATE`](crate::DateOptions::COMPACT_DATE) and the others), and the
/// two-digit-year pivot, 12 bytes with 4-byte alignment.
///
/// | Offset | Field | Bytes |
/// | --- | --- | --- |
/// | 0 | `size`, a `u32` | 4 |
/// | 4 | `flags`, the option bits | 4 |
/// | 8 | `pivot`, `0` for none, else 100–9999 | 4 |
///
/// `size` is [`MIN_SIZE`](Self::MIN_SIZE) (the size and flags words; the pivot is then `0`,
/// none) or [`SIZE`](Self::SIZE); any other size, a flag bit this core doesn't define, or a
/// pivot other than `0` or 100–9999 is a contract violation. A null pointer is no options. The
/// core never writes it.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawDateOptions {
    /// The bytes the caller built: [`RawDateOptions::SIZE`] or [`RawDateOptions::MIN_SIZE`].
    pub size: u32,
    /// The option bits.
    pub flags: u32,
    /// The last year of the window a two-digit year is read in; `0` for none.
    pub pivot: u32,
}

impl RawDateOptions {
    /// The layout's size in bytes, which `size` carries.
    pub const SIZE: u32 = 12;

    /// The smallest `size` the core reads: the size and flags words, with no pivot.
    pub const MIN_SIZE: u32 = 8;
}
