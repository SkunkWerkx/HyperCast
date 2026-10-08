//! The char door. Reads one Unicode scalar from text, either verbatim or as a declared code
//! point — the grammar is Svartalfheim's `CharParser`, ported.
//!
//! 1. **Verbatim:** an input that is exactly one UTF-8 scalar is that scalar, checked
//!    *before* trimming, so `" "` is a space, not `Empty`, and `"6"` is the digit six,
//!    not U+0006.
//! 2. Otherwise leading and trailing ASCII whitespace is ignored and the text must be
//!    exactly one code-point spelling, ASCII case-insensitive on the prefix: decimal `65`,
//!    `U+0041`, `0x41`, `&H41`, or an HTML numeric entity `&#65;` / `&#x41;` (the closing
//!    `;` is required).
//!
//! The wrong shape is `Malformed` at the first offending character, or over the whole token
//! when the text simply ends too early (`U+`, `&#65`). A well-shaped spelling naming no
//! scalar — past `U+10FFFF`, or a surrogate `U+D800..=U+DFFF` — is `OutOfRange` over the
//! token. Culture-insensitive by nature, so no [`NumFormat`](crate::NumFormat) is accepted.

use crate::integer::char_len_at;
use crate::verdict::{Fault, trim};

/// Casts char text. Empty or whitespace-only input (other than a single whitespace
/// character, which is verbatim) ⇒ `Empty`.
pub fn cast_char(input: impl AsRef<[u8]>) -> Result<char, Fault> {
    let input = input.as_ref();
    if input.is_empty() {
        return Err(Fault::EMPTY);
    }
    if let Some(scalar) = single_scalar(input) {
        return Ok(scalar);
    }
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }
    let token = Fault::malformed(start, text.len());
    // The offending character at `at` (relative to `text`), or the whole token when the
    // text ran out before the grammar was satisfied.
    let offending = |at: usize| match text.get(at) {
        Some(_) => Fault::malformed(start + at, char_len_at(text, at)),
        None => token,
    };

    let (radix, digits_at, entity) = match text {
        [b'u' | b'U', b'+', ..] | [b'0', b'x' | b'X', ..] | [b'&', b'h' | b'H', ..] => {
            (16, 2, false)
        }
        [b'&', b'#', b'x' | b'X', ..] => (16, 3, true),
        [b'&', b'#', ..] => (10, 2, true),
        [b'&', ..] => return Err(offending(1)),
        _ => (10, 0, false),
    };

    let body = text.get(digits_at..).unwrap_or_default();
    let mut value = 0u32;
    let mut digits = 0usize;
    for &byte in body {
        let Some(digit) = digit(byte, radix) else { break };
        // Saturating keeps an absurdly long spelling above U+10FFFF instead of wrapping
        // back into range; the range check below then reports it.
        value = value.saturating_mul(radix).saturating_add(digit);
        digits += 1;
    }
    let after = digits_at + digits;
    if digits == 0 {
        return Err(offending(after));
    }
    if entity {
        match text.get(after) {
            Some(b';') if after + 1 == text.len() => {}
            Some(b';') => return Err(offending(after + 1)),
            _ => return Err(offending(after)),
        }
    } else if after != text.len() {
        return Err(offending(after));
    }
    char::from_u32(value).ok_or(Fault::out_of_range(start, text.len()))
}

/// The scalar `input` encodes when it is exactly one well-formed UTF-8 scalar.
fn single_scalar(input: &[u8]) -> Option<char> {
    if input.len() > 4 {
        return None;
    }
    let mut chars = core::str::from_utf8(input).ok()?.chars();
    match (chars.next(), chars.next()) {
        (Some(scalar), None) => Some(scalar),
        _ => None,
    }
}

fn digit(byte: u8, radix: u32) -> Option<u32> {
    let value = match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' if radix == 16 => byte - b'a' + 10,
        b'A'..=b'F' if radix == 16 => byte - b'A' + 10,
        _ => return None,
    };
    Some(u32::from(value))
}
