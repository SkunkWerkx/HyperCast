//! The parts of the C ABI that are Rust API too: the version word every binding probes, and
//! [`NumFormat`] as it crosses the boundary. They live outside `ffi` because that module is
//! the `exports` feature — the 26 `cast_*` symbols — and a crate that links this one as an
//! rlib to export a C ABI of its own takes the core with `exports` off, so its library
//! carries none of them, while still declaring a numeric column in exactly this layout.

use crate::verdict::{
    CivilDateTime, CurrencySymbol, Date, Decimal, Duration, NumFormat, Timestamp,
};

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
    RawNumFormat => (32, 4),
    Decimal => (16, 8),
    Timestamp => (16, 8),
    Date => (4, 2),
    CivilDateTime => (16, 8),
    Duration => (16, 8),
}

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

/// [`NumFormat`] as it crosses the ABI: separators as Unicode code points, the currency
/// symbol as `currency_len` UTF-8 bytes held inline in `currency` (32 bytes in all,
/// 4-byte alignment). Where an export takes a pointer to one, null means
/// [`NumFormat::INVARIANT`].
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawNumFormat {
    /// The decimal separator, as a Unicode code point.
    pub decimal_sep: u32,
    /// The digit-group separator, as a Unicode code point.
    pub group_sep: u32,
    /// The [`NumFormat`] flags.
    pub flags: u32,
    /// How many bytes of `currency` are the symbol; `0` declares none.
    pub currency_len: u32,
    /// The currency symbol's UTF-8 bytes, `currency_len` of them.
    pub currency: [u8; CurrencySymbol::MAX_BYTES],
}

impl RawNumFormat {
    /// The [`NumFormat`] this declares, or `None` for a contract violation — a caller bug,
    /// not a data verdict: a code point that is no `char`, equal separators, or a currency
    /// that is not a valid [`CurrencySymbol`] (longer than 16 bytes, not UTF-8, or carrying
    /// an ASCII digit or whitespace).
    pub fn resolve(&self) -> Option<NumFormat> {
        let decimal_sep = char::from_u32(self.decimal_sep)?;
        let group_sep = char::from_u32(self.group_sep)?;
        if decimal_sep == group_sep {
            return None;
        }
        let currency_len = self.currency_len as usize;
        let currency = if currency_len == 0 {
            CurrencySymbol::NONE
        } else {
            let bytes = self.currency.get(..currency_len)?;
            CurrencySymbol::new(core::str::from_utf8(bytes).ok()?)?
        };
        Some(NumFormat { decimal_sep, group_sep, flags: self.flags, currency })
    }

    /// The 32 bytes in the layout's own order, little-endian — what a binding hands a C ABI
    /// that takes a format, and what Ruby's and Python's `NumFormat.packed` are.
    pub fn to_le_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        let words = [self.decimal_sep, self.group_sep, self.flags, self.currency_len];
        for (slot, word) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(words) {
            *slot = word.to_le_bytes();
        }
        if let Some(currency) = bytes.get_mut(16..) {
            currency.copy_from_slice(&self.currency);
        }
        bytes
    }

    /// The layout read back from [`to_le_bytes`](Self::to_le_bytes)'s 32 bytes. Nothing is
    /// checked here: [`resolve`](Self::resolve) says whether the format they spell is one.
    pub fn from_le_bytes(bytes: [u8; 32]) -> RawNumFormat {
        let word = |at: usize| {
            let mut word = [0u8; 4];
            if let Some(source) = bytes.get(at..at + 4) {
                word.copy_from_slice(source);
            }
            u32::from_le_bytes(word)
        };
        let mut currency = [0u8; CurrencySymbol::MAX_BYTES];
        if let Some(source) = bytes.get(16..) {
            currency.copy_from_slice(source);
        }
        RawNumFormat {
            decimal_sep: word(0),
            group_sep: word(4),
            flags: word(8),
            currency_len: word(12),
            currency,
        }
    }
}

impl From<NumFormat> for RawNumFormat {
    /// The layout a format crosses the ABI in — [`resolve`](RawNumFormat::resolve)'s inverse.
    fn from(format: NumFormat) -> RawNumFormat {
        let symbol = format.currency.as_str().as_bytes();
        let mut currency = [0u8; CurrencySymbol::MAX_BYTES];
        if let Some(slot) = currency.get_mut(..symbol.len()) {
            slot.copy_from_slice(symbol);
        }
        RawNumFormat {
            decimal_sep: u32::from(format.decimal_sep),
            group_sep: u32::from(format.group_sep),
            flags: format.flags,
            currency_len: symbol.len() as u32,
            currency,
        }
    }
}
