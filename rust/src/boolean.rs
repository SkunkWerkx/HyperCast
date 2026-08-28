//! The boolean door. Extends the bare `true`/`false` pair with the numeric and
//! natural-language conventions untrusted sources actually send — the lexicon is
//! Svartalfheim's `BooleanParser`, verbatim.
//!
//! Recognized true values: `true`, `t`, `yes`, `y`, `1`, `on`, `enabled`, `active`,
//! `checked`, `in`. Recognized false values: `false`, `f`, `no`, `n`, `0`, `off`,
//! `disabled`, `inactive`, `unchecked`, `out`. Matching is ASCII case-insensitive;
//! leading and trailing ASCII whitespace is ignored. Culture-insensitive by nature —
//! no [`NumFormat`](crate::NumFormat) is accepted.

use crate::verdict::{trim, Fault};

/// Casts boolean text. Empty or whitespace input ⇒ `Empty`; unrecognized input ⇒
/// `Malformed` spanning the trimmed token.
pub fn cast_bool(input: impl AsRef<[u8]>) -> Result<bool, Fault> {
    let input = input.as_ref();
    let (text, start) = trim(input);
    if text.is_empty() {
        return Err(Fault::EMPTY);
    }
    // Length dispatch first: no lexeme pair shares a length and a truth value, so each arm
    // compares at most a couple of fixed-width candidates — no scratch buffer, no fold loop.
    // (`| 0x20` is a no-op on the digit arms: 0x30–0x39 already carry that bit.)
    let value = match text.len() {
        1 => match text[0] | 0x20 {
            b't' | b'y' | b'1' => Some(true),
            b'f' | b'n' | b'0' => Some(false),
            _ => None,
        },
        2 if text.eq_ignore_ascii_case(b"on") || text.eq_ignore_ascii_case(b"in") => Some(true),
        2 if text.eq_ignore_ascii_case(b"no") => Some(false),
        3 if text.eq_ignore_ascii_case(b"yes") => Some(true),
        3 if text.eq_ignore_ascii_case(b"off") || text.eq_ignore_ascii_case(b"out") => {
            Some(false)
        }
        4 if text.eq_ignore_ascii_case(b"true") => Some(true),
        5 if text.eq_ignore_ascii_case(b"false") => Some(false),
        6 if text.eq_ignore_ascii_case(b"active") => Some(true),
        7 if text.eq_ignore_ascii_case(b"enabled") || text.eq_ignore_ascii_case(b"checked") => {
            Some(true)
        }
        8 if text.eq_ignore_ascii_case(b"disabled") || text.eq_ignore_ascii_case(b"inactive") => {
            Some(false)
        }
        9 if text.eq_ignore_ascii_case(b"unchecked") => Some(false),
        _ => None,
    };
    value.ok_or_else(|| Fault::malformed(start, text.len()))
}
