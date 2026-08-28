//! The UUID door. Svartalfheim's `GuidParser` semantics: strip a leading case-insensitive
//! `urn:uuid:` / `GUID:` / `UUID:` prefix, then accept every format .NET's `Guid.TryParse`
//! does — D (hyphenated), N (32 hex), B (braced), P (parenthesized), X (hex struct) — with
//! HyperCast's own deterministic strictness inside X (no interior whitespace).
//!
//! Output is 16 bytes in RFC 9562 order (the order the text reads in). Platform byte-order
//! games — .NET `Guid`'s little-endian first three fields, SQL Server sort order — stay in
//! the bindings, exactly where HyperUuid already put them.

use crate::integer::char_len;
use crate::verdict::{trim, Fault};

const PREFIXES: [&[u8]; 3] = [b"urn:uuid:", b"guid:", b"uuid:"];

/// Casts UUID text to 16 bytes in RFC 9562 order. Empty ⇒ `Empty`; unrecognized ⇒
/// `Malformed` at the first offending byte (or spanning the token for structural failures).
pub fn cast_uuid(input: &[u8]) -> Result<[u8; 16], Fault> {
    let (outer, outer_start) = trim(input);
    if outer.is_empty() {
        return Err(Fault::EMPTY);
    }

    let (mut text, mut start) = (outer, outer_start);
    // Every prefix starts with u/g; a hex digit never does, so the common unprefixed
    // shapes skip the three case-insensitive comparisons entirely.
    if matches!(text[0] | 0x20, b'u' | b'g') {
        for prefix in PREFIXES {
            if text.len() >= prefix.len() && text[..prefix.len()].eq_ignore_ascii_case(prefix) {
                let (stripped, inner_start) = trim(&text[prefix.len()..]);
                start += prefix.len() + inner_start;
                text = stripped;
                break;
            }
        }
    }
    if text.is_empty() {
        // A bare prefix ("GUID:") is present-but-unrecognizable, not absent.
        return Err(Fault::malformed(outer_start, outer.len()));
    }

    match text[0] {
        b'{' if text.len() >= 3 && text[1] == b'0' && (text[2] | 0x20) == b'x' => {
            parse_x(text, start)
        }
        b'{' => parse_wrapped(text, start, b'}'),
        b'(' => parse_wrapped(text, start, b')'),
        _ if text.len() == 32 => parse_n(text, start),
        _ => parse_d(text, start),
    }
}

/// Hex nibble lookup — `0xFF` marks a non-hex byte, so a pair's validity is one branch on
/// `hi | lo` instead of two `Option` chains per nibble.
static HEX: [u8; 256] = {
    let mut table = [0xFFu8; 256];
    let mut byte = 0usize;
    while byte < 256 {
        table[byte] = match byte as u8 {
            b'0'..=b'9' => byte as u8 - b'0',
            b'a'..=b'f' => byte as u8 - b'a' + 10,
            b'A'..=b'F' => byte as u8 - b'A' + 10,
            _ => 0xFF,
        };
        byte += 1;
    }
    table
};

fn hex(byte: u8) -> Option<u8> {
    match HEX[byte as usize] {
        0xFF => None,
        value => Some(value),
    }
}

/// Decodes the hex pair at `at` into one output byte, faulting on the exact bad byte.
#[inline]
fn hex_pair(text: &[u8], at: usize, start: usize) -> Result<u8, Fault> {
    let hi = HEX[text[at] as usize];
    let lo = HEX[text[at + 1] as usize];
    if hi | lo == 0xFF {
        let bad = if hi == 0xFF { at } else { at + 1 };
        return Err(Fault::malformed(start + bad, char_len(text[bad])));
    }
    Ok((hi << 4) | lo)
}

/// The 16 hex-pair positions of the D format `dddddddd-dddd-dddd-dddd-dddddddddddd`.
const D_PAIRS: [usize; 16] = [0, 2, 4, 6, 9, 11, 14, 16, 19, 21, 24, 26, 28, 30, 32, 34];

/// D format: `dddddddd-dddd-dddd-dddd-dddddddddddd`.
fn parse_d(text: &[u8], start: usize) -> Result<[u8; 16], Fault> {
    if text.len() != 36 {
        return Err(Fault::malformed(start, text.len()));
    }
    for hyphen in [8usize, 13, 18, 23] {
        if text[hyphen] != b'-' {
            return Err(Fault::malformed(start + hyphen, char_len(text[hyphen])));
        }
    }
    let mut out = [0u8; 16];
    for (slot, &at) in out.iter_mut().zip(&D_PAIRS) {
        *slot = hex_pair(text, at, start)?;
    }
    Ok(out)
}

/// N format: 32 bare hex digits.
fn parse_n(text: &[u8], start: usize) -> Result<[u8; 16], Fault> {
    let mut out = [0u8; 16];
    for (slot, at) in out.iter_mut().zip((0..32).step_by(2)) {
        *slot = hex_pair(text, at, start)?;
    }
    Ok(out)
}

/// B and P formats: a D-format UUID wrapped in `{}` or `()`.
fn parse_wrapped(text: &[u8], start: usize, close: u8) -> Result<[u8; 16], Fault> {
    if text.len() != 38 {
        return Err(Fault::malformed(start, text.len()));
    }
    if *text.last().unwrap() != close {
        return Err(Fault::malformed(start + 37, char_len(text[37])));
    }
    parse_d(&text[1..37], start + 1)
}

/// X format: `{0xdddddddd,0xdddd,0xdddd,{0xdd,0xdd,0xdd,0xdd,0xdd,0xdd,0xdd,0xdd}}`,
/// each component 1 to its full width in hex digits, no interior whitespace.
fn parse_x(text: &[u8], start: usize) -> Result<[u8; 16], Fault> {
    let mut out = [0u8; 16];
    let mut i = 0;
    let expect = |wanted: u8, i: &mut usize| -> Result<(), Fault> {
        if *i < text.len() && text[*i] == wanted {
            *i += 1;
            Ok(())
        } else if *i < text.len() {
            Err(Fault::malformed(start + *i, char_len(text[*i])))
        } else {
            Err(Fault::malformed(start, text.len()))
        }
    };

    // Reads `0x` + 1..=max_digits hex digits, big-endian into out[at..at + width].
    let component =
        |i: &mut usize, out: &mut [u8; 16], at: usize, width: usize| -> Result<(), Fault> {
            if *i + 2 > text.len()
                || text[*i] != b'0'
                || (text[*i + 1] | 0x20) != b'x'
            {
                let bad = (*i).min(text.len().saturating_sub(1));
                return Err(Fault::malformed(start + bad, char_len(text[bad])));
            }
            *i += 2;
            let mut value: u64 = 0;
            let mut digits = 0;
            while *i < text.len()
                && let Some(nibble) = hex(text[*i])
            {
                if digits == width * 2 {
                    return Err(Fault::malformed(start + *i, char_len(text[*i])));
                }
                value = (value << 4) | nibble as u64;
                digits += 1;
                *i += 1;
            }
            if digits == 0 {
                let bad = (*i).min(text.len().saturating_sub(1));
                return Err(Fault::malformed(start + bad, char_len(text[bad])));
            }
            for slot in 0..width {
                out[at + slot] = (value >> ((width - 1 - slot) * 8)) as u8;
            }
            Ok(())
        };

    expect(b'{', &mut i)?;
    component(&mut i, &mut out, 0, 4)?;
    expect(b',', &mut i)?;
    component(&mut i, &mut out, 4, 2)?;
    expect(b',', &mut i)?;
    component(&mut i, &mut out, 6, 2)?;
    expect(b',', &mut i)?;
    expect(b'{', &mut i)?;
    for slot in 0..8 {
        component(&mut i, &mut out, 8 + slot, 1)?;
        if slot < 7 {
            expect(b',', &mut i)?;
        }
    }
    expect(b'}', &mut i)?;
    expect(b'}', &mut i)?;
    if i != text.len() {
        return Err(Fault::malformed(start + i, char_len(text[i])));
    }
    Ok(out)
}
