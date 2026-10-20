//! The parts of the C ABI that are Rust API too: the version word every binding probes, and
//! [`NumFormat`] and the day-period designators as they cross the boundary. They live
//! outside `ffi` because that module is the `exports` feature — the 26 `cast_*` symbols —
//! and a crate that links this one as an rlib to export a C ABI of its own takes the core
//! with `exports` off, so its library carries none of them, while still declaring a
//! numeric column in exactly this layout.

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
}

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
};

/// This library's version, packed `major << 16 | minor << 8 | patch` from the crate's own
/// manifest — so a host can prove the library it loaded is the one its binding was built
/// against before making the first cast, and can name the mismatch when it isn't. Takes
/// nothing, touches nothing: the cheapest possible "did the native library resolve" probe.
#[cfg_attr(feature = "exports", unsafe(no_mangle))]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
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
    /// The layout's size in bytes, which `size` carries.
    pub const SIZE: u32 = size_of::<RawNumFormat>() as u32;

    /// The smallest `size` the core reads: through the group separator's length byte, since
    /// a format with no number separators is no format.
    pub const MIN_SIZE: u32 = 30;

    /// Every byte zero: no flags, nothing declared, and no number separators — not a
    /// format on its own, but what [`resolve`](Self::resolve) fills a shorter caller's
    /// layout out to.
    pub const ZERO: RawNumFormat = RawNumFormat { size: Self::SIZE, format: NumFormat::EMPTY };

    /// The [`NumFormat`] this declares, in place, or `None` for a contract violation — a
    /// caller bug, not a data verdict: a `size` the core does not read, or a format
    /// [`NumFormat::check`] refuses (a missing or over-long separator, equal separators, a
    /// symbol with an ASCII digit in it or that is not UTF-8, whitespace in a sign, the
    /// percent or the currency symbol, a digit zero that starts no digit block).
    #[inline]
    pub fn resolve(&self) -> Option<&NumFormat> {
        if self.size < Self::MIN_SIZE || self.size > Self::SIZE || !self.format.check() {
            return None;
        }
        Some(&self.format)
    }

    /// The 92 bytes in the layout's own order, little-endian — what a binding hands a C ABI
    /// that takes a format, and what Ruby's and Python's `NumFormat.packed` are.
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
    /// The layout's size in bytes, which `size` carries.
    pub const SIZE: u32 = size_of::<RawDayPeriods>() as u32;

    /// The smallest `size` the core reads: the size word alone, which declares nothing.
    pub const MIN_SIZE: u32 = 4;

    /// Nothing declared.
    pub const ZERO: RawDayPeriods =
        RawDayPeriods { size: Self::SIZE, periods: DayPeriods::ENGLISH };

    /// The [`DayPeriods`] this declares, in place, or `None` for a contract violation: a
    /// `size` the core does not read, or a designator that is not valid (longer than 16
    /// bytes, not UTF-8, carrying an ASCII digit, or starting or ending in ASCII whitespace,
    /// which the grammar around it already reads).
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
