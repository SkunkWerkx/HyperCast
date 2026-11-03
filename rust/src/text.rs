//! The text door's pieces: UTF-8 validation, trimming, length in code points and the
//! simple case folding that `enum` matching uses.
//!
//! Text is never normalized: `é` as one code point and as `e` plus U+0301 are different
//! text, and a length counts code points, not graphemes.

use crate::fold_table::FOLD;
use crate::verdict::{Fault, Reason};

/// The 25 code points with the Unicode `White_Space` property (PropList.txt, Unicode
/// 17.0.0), in code point order. U+200B, ZERO WIDTH SPACE, is not one of them.
const WHITE_SPACE: [char; 25] = [
    '\u{0009}', '\u{000A}', '\u{000B}', '\u{000C}', '\u{000D}', '\u{0020}', '\u{0085}', '\u{00A0}',
    '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}',
    '\u{2007}', '\u{2008}', '\u{2009}', '\u{200A}', '\u{2028}', '\u{2029}', '\u{202F}', '\u{205F}',
    '\u{3000}',
];

fn is_white_space(c: char) -> bool {
    WHITE_SPACE.contains(&c)
}

/// The text door: the input as `&str`, untrimmed. Invalid UTF-8 is `Malformed`, spanning
/// the first invalid sequence (to the end of the input when it is cut short); empty input
/// is `Empty`.
pub fn cast_text(input: &[u8]) -> Result<&str, Fault> {
    if input.is_empty() {
        return Err(Fault { reason: Reason::Empty, offset: 0, len: 0 });
    }
    core::str::from_utf8(input).map_err(|e| {
        let offset = e.valid_up_to();
        let len = match e.error_len() {
            Some(len) => len,
            None => input.len().saturating_sub(offset),
        };
        Fault::malformed(offset, len)
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
pub(crate) fn fold(c: char) -> char {
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
