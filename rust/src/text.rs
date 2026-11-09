//! The text door's pieces: UTF-8 validation, trimming, length in code points and the
//! simple case folding that `enum` matching uses.
//!
//! Text is never normalized: `é` as one code point and as `e` plus U+0301 are different
//! text, and a length counts code points, not graphemes.

use crate::detail::Failure;
use crate::fold_table::FOLD;
use crate::site::Site;
use crate::verdict::Fault;

/// The 25 code points with the Unicode `White_Space` property (PropList.txt, Unicode
/// 17.0.0), in code point order. U+200B, ZERO WIDTH SPACE, is not one of them.
pub(crate) const WHITE_SPACE: [char; 25] = [
    '\u{0009}', '\u{000A}', '\u{000B}', '\u{000C}', '\u{000D}', '\u{0020}', '\u{0085}', '\u{00A0}',
    '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}',
    '\u{2007}', '\u{2008}', '\u{2009}', '\u{200A}', '\u{2028}', '\u{2029}', '\u{202F}', '\u{205F}',
    '\u{3000}',
];

/// Whether `c` has the Unicode `White_Space` property: the 25 code points of PropList.txt,
/// which is what every door trims and what a number or a date may not contain inside.
/// U+200B, U+FEFF and U+180E are not white space.
pub fn is_white_space(c: char) -> bool {
    WHITE_SPACE.contains(&c)
}

/// Whether `b` is one of the ASCII white-space bytes: the five controls, U+000B and space.
const fn is_ascii_white_space(b: u8) -> bool {
    matches!(b, 0x09..=0x0D | 0x20)
}

/// The byte length of the non-ASCII white-space code point that `seq` starts with, or `0`.
/// Overlong and cut encodings match nothing.
const fn encoded_white_space_prefix(seq: &[u8]) -> usize {
    match seq {
        [0xC2, 0x85 | 0xA0, ..] => 2,
        [0xE1, 0x9A, 0x80, ..]
        | [0xE2, 0x80, 0x80..=0x8A | 0xA8 | 0xA9 | 0xAF, ..]
        | [0xE2, 0x81, 0x9F, ..]
        | [0xE3, 0x80, 0x80, ..] => 3,
        _ => 0,
    }
}

/// The byte length of the white-space code point that starts `bytes`, or `0`.
#[inline]
pub(crate) const fn white_space_len(bytes: &[u8]) -> usize {
    match bytes {
        [first, ..] if is_ascii_white_space(*first) => 1,
        _ => encoded_white_space_prefix(bytes),
    }
}

/// The byte length of the white-space code point that ends `bytes`, or `0`.
#[inline]
pub(crate) const fn white_space_len_back(bytes: &[u8]) -> usize {
    match bytes {
        [.., last] if is_ascii_white_space(*last) => 1,
        [.., a, b] if encoded_white_space_prefix(&[*a, *b]) == 2 => 2,
        [.., a, b, c] if encoded_white_space_prefix(&[*a, *b, *c]) == 3 => 3,
        _ => 0,
    }
}

/// The byte length of the space-like character that starts `bytes`, or `0`: `' '`, U+00A0
/// or U+202F, the three that real data mixes and that stand for each other in a declared
/// group separator and inside a declared currency symbol.
#[inline]
pub(crate) const fn space_like_len(bytes: &[u8]) -> usize {
    match bytes {
        [b' ', ..] => 1,
        [0xC2, 0xA0, ..] => 2,
        [0xE2, 0x80, 0xAF, ..] => 3,
        _ => 0,
    }
}

/// The byte length of the space-like character that ends `bytes`, or `0`.
#[inline]
pub(crate) const fn space_like_len_back(bytes: &[u8]) -> usize {
    match bytes {
        [.., b' '] => 1,
        [.., 0xC2, 0xA0] => 2,
        [.., 0xE2, 0x80, 0xAF] => 3,
        _ => 0,
    }
}

/// The byte length of the run of white space at the start of `bytes`.
#[inline]
pub(crate) fn skip_white_space(bytes: &[u8]) -> usize {
    let mut rest = bytes;
    loop {
        let step = white_space_len(rest);
        match rest.get(step..) {
            Some(tail) if step != 0 => rest = tail,
            _ => return bytes.len().saturating_sub(rest.len()),
        }
    }
}

/// The byte length of the run of white space at the end of `bytes`.
#[inline]
pub(crate) fn skip_white_space_back(bytes: &[u8]) -> usize {
    let mut rest = bytes;
    loop {
        let step = white_space_len_back(rest);
        match rest.len().checked_sub(step).and_then(|keep| rest.get(..keep)) {
            Some(head) if step != 0 => rest = head,
            _ => return bytes.len().saturating_sub(rest.len()),
        }
    }
}

/// Whether any white-space code point occurs anywhere in `bytes`. Bytes that are not
/// valid UTF-8 never match, since only whole encodings do.
pub(crate) const fn contains_white_space(bytes: &[u8]) -> bool {
    let mut rest = bytes;
    while let [first, tail @ ..] = rest {
        if is_ascii_white_space(*first) || encoded_white_space_prefix(rest) != 0 {
            return true;
        }
        rest = tail;
    }
    false
}

/// The text door: the input as `&str`, untrimmed. Invalid UTF-8 is `Malformed`, spanning
/// the first invalid sequence (to the end of the input when it is cut short); empty input
/// is `Empty`.
pub fn cast_text(input: &[u8]) -> Result<&str, Fault> {
    text(input).map_err(Failure::fault)
}

/// The text door's one parse, for both entry points.
pub(crate) fn text(input: &[u8]) -> Result<&str, Failure> {
    if input.is_empty() {
        return Err(Failure::spanless(Site::TextEmpty));
    }
    crate::utf8::from_utf8(input).map_err(|e| {
        let offset = e.valid_up_to();
        let len = match e.error_len() {
            Some(len) => len,
            None => input.len().saturating_sub(offset),
        };
        Failure::at(Site::TextInvalidUtf8, offset, len)
    })
}

/// The text without leading and trailing `White_Space`, and the byte offset where it
/// starts in `text`.
pub(crate) fn trim_white_space(text: &str) -> (&str, usize) {
    let rest = text.trim_start_matches(is_white_space);
    (rest.trim_end_matches(is_white_space), text.len() - rest.len())
}

/// The length in Unicode code points, saturating at `u32::MAX`.
pub(crate) fn code_points(text: &str) -> u32 {
    u32::try_from(text.chars().count()).unwrap_or(u32::MAX)
}

/// Simple case folding (`C` and `S` rows of `CaseFolding.txt`): one code point to one,
/// never Turkic, never full (`ß` stays `ß`).
#[inline]
pub(crate) fn fold(c: char) -> char {
    // On ASCII the table is `to_ascii_lowercase` (`fold_is_ascii_lowercase_on_ascii`), so the
    // common case needs no search.
    if c.is_ascii() {
        return c.to_ascii_lowercase();
    }
    fold_searched(c)
}

/// [`fold`] by searching the table, for any code point.
fn fold_searched(c: char) -> char {
    match FOLD.binary_search_by_key(&(c as u32), |&(from, _)| from) {
        Ok(i) => FOLD.get(i).and_then(|&(_, to)| char::from_u32(to)).unwrap_or(c),
        Err(_) => c,
    }
}

/// Whether two texts are equal under simple case folding, compared code point by code
/// point in step without allocating. Callers validate UTF-8; bytes that are not valid
/// compare unequal.
pub(crate) fn eq_folded(a: &[u8], b: &[u8]) -> bool {
    let (Ok(a), Ok(b)) = (core::str::from_utf8(a), core::str::from_utf8(b)) else {
        return false;
    };
    a.chars().map(fold).eq(b.chars().map(fold))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::Reason;

    #[test]
    fn cast_text_validates_utf8() {
        let bad = cast_text(b"ab\xFFc").unwrap_err();
        assert_eq!((bad.reason, bad.offset, bad.len), (Reason::Malformed, 2, 1));
        let cut = cast_text(b"ab\xE2\x82").unwrap_err();
        assert_eq!((cut.reason, cut.offset, cut.len), (Reason::Malformed, 2, 2));
        assert_eq!(cast_text(b"").unwrap_err().reason, Reason::Empty);
        assert_eq!(cast_text(b" a "), Ok(" a "));
    }

    #[test]
    fn trim_covers_white_space() {
        assert_eq!(trim_white_space("\u{00A0}Male\u{3000}"), ("Male", 2));
        assert_eq!(trim_white_space("\u{200B}x"), ("\u{200B}x", 0));
        assert_eq!(trim_white_space(" \t\n "), ("", 4));
        for c in WHITE_SPACE {
            let mut buf = [0u8; 4];
            assert_eq!(trim_white_space(c.encode_utf8(&mut buf)).0, "");
        }
    }

    #[test]
    fn lengths_count_code_points() {
        assert_eq!(code_points("👍🏽"), 2);
        assert_eq!(code_points("e\u{301}"), 2);
        assert_eq!(code_points("ab"), 2);
        assert_eq!(code_points(""), 0);
    }

    #[test]
    fn fold_is_ascii_lowercase_on_ascii() {
        for c in (0u8..0x80).map(char::from) {
            assert_eq!(fold(c), fold_searched(c), "{c:?}");
        }
    }

    #[test]
    fn folding_is_simple_and_not_turkic() {
        assert!(eq_folded(b"MALE", b"male"));
        assert!(eq_folded("ΣΑΣ".as_bytes(), "σας".as_bytes()));
        assert!(!eq_folded("I".as_bytes(), "ı".as_bytes()));
        assert!(!eq_folded("i".as_bytes(), "İ".as_bytes()));
        assert!(!eq_folded(b"STRASSE", "straße".as_bytes()));
        assert!(!eq_folded(b"a\xFF", b"a\xFF"));
        assert!(!eq_folded(b"ab", b"abc"));
    }
}
