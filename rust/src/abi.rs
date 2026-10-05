//! The parts of the C ABI that are Rust API too: the version word every binding probes, and
//! [`NumFormat`] as it crosses the boundary. They live outside `ffi` because that module is
//! the `exports` feature — the 25 `cast_*` symbols — and a crate that links this one as an
//! rlib to export a C ABI of its own takes the core with `exports` off, so its library
//! carries none of them, while still declaring a numeric column in exactly this layout.

use crate::verdict::{CurrencySymbol, NumFormat};

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
}
