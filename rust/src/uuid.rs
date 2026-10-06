//! The UUID door. Svartalfheim's `GuidParser` semantics: strip a leading case-insensitive
//! `urn:uuid:` / `GUID:` / `UUID:` prefix, then accept every format .NET's `Guid.TryParse`
//! does — D (hyphenated), N (32 hex), B (braced), P (parenthesized), X (hex struct) — with
//! HyperCast's own deterministic strictness inside X (no interior whitespace).
//!
//! Output is 16 bytes in RFC 9562 order (the order the text reads in). Platform byte-order
//! games — .NET `Guid`'s little-endian first three fields, SQL Server sort order — stay in
//! the bindings, exactly where HyperUuid already put them.

use crate::integer::char_len_at;
use crate::verdict::{Fault, trim};

const PREFIXES: [&[u8]; 3] = [b"urn:uuid:", b"guid:", b"uuid:"];

/// Casts UUID text to 16 bytes in RFC 9562 order. Empty ⇒ `Empty`; unrecognized ⇒
/// `Malformed` at the first offending byte (or spanning the token for structural failures).
pub fn cast_uuid(input: impl AsRef<[u8]>) -> Result<[u8; 16], Fault> {
    let input = input.as_ref();
    let (outer, outer_start) = trim(input);
    if outer.is_empty() {
        return Err(Fault::EMPTY);
    }

    let (mut text, mut start) = (outer, outer_start);
    // Every prefix starts with u/g; a hex digit never does, so the common unprefixed
    // shapes skip the three case-insensitive comparisons entirely.
    if matches!(text.first().map(|&byte| byte | 0x20), Some(b'u' | b'g')) {
        for prefix in PREFIXES {
            if let Some((head, rest)) = text.split_at_checked(prefix.len())
                && head.eq_ignore_ascii_case(prefix)
            {
                let (stripped, inner_start) = trim(rest);
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

    match text {
        [b'{', b'0', x, ..] if x | 0x20 == b'x' => parse_x(text, start),
        [b'{', ..] => parse_wrapped(text, start, b'}'),
        [b'(', ..] => parse_wrapped(text, start, b')'),
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
///
/// Every caller has already checked the length, so the pair is always there; it is read with
/// `get` all the same, because a bounds check the optimizer has to prove away is a panic
/// path whenever it does not: a consumer's build on Rust 1.88, or on stable without
/// `codegen-units = 1`, kept one here and `cast_uuid` failed the no-panic proof. A pair that
/// is somehow absent reads as a non-hex byte.
#[inline]
fn hex_pair(text: &[u8], at: usize, start: usize) -> Result<u8, Fault> {
    let nibble = |at: usize| text.get(at).map_or(0xFF, |&byte| HEX[byte as usize]);
    let hi = nibble(at);
    let lo = nibble(at + 1);
    if hi | lo == 0xFF {
        let bad = if hi == 0xFF { at } else { at + 1 };
        return Err(Fault::malformed(start + bad, char_len_at(text, bad)));
    }
    Ok((hi << 4) | lo)
}

/// The 16 hex-pair positions of the D format `dddddddd-dddd-dddd-dddd-dddddddddddd`.
const D_PAIRS: [usize; 16] = [0, 2, 4, 6, 9, 11, 14, 16, 19, 21, 24, 26, 28, 30, 32, 34];

/// D format: `dddddddd-dddd-dddd-dddd-dddddddddddd`.
fn parse_d(text: &[u8], start: usize) -> Result<[u8; 16], Fault> {
    let Ok(d) = <&[u8; 36]>::try_from(text) else {
        return Err(Fault::malformed(start, text.len()));
    };
    for hyphen in [8usize, 13, 18, 23] {
        if d[hyphen] != b'-' {
            return Err(Fault::malformed(start + hyphen, char_len_at(text, hyphen)));
        }
    }
    // The 32 digits as four little-endian words of eight: the first group, the two groups
    // of four on either side of each middle hyphen joined, and the last eight digits.
    let word =
        |at: usize| d.get(at..at + 8).and_then(|w| w.try_into().ok()).map(u64::from_le_bytes);
    let half = |at: usize| {
        u64::from(d.get(at..at + 4).and_then(|w| w.try_into().ok()).map_or(0, u32::from_le_bytes))
    };
    let words = [
        word(0).unwrap_or(0),
        half(9) | half(14) << 32,
        half(19) | half(24) << 32,
        word(28).unwrap_or(0),
    ];
    decode_words(words).ok_or_else(|| first_bad_pair(text, &D_PAIRS, start))
}

/// N format: 32 bare hex digits.
fn parse_n(text: &[u8], start: usize) -> Result<[u8; 16], Fault> {
    let Ok(n) = <&[u8; 32]>::try_from(text) else {
        return Err(Fault::malformed(start, text.len()));
    };
    let (words, _) = n.as_chunks::<8>();
    let mut packed = [0u64; 4];
    for (slot, word) in packed.iter_mut().zip(words) {
        *slot = u64::from_le_bytes(*word);
    }
    decode_words(packed).ok_or_else(|| first_bad_pair(text, &N_PAIRS, start))
}

/// The 16 hex-pair positions of the N format: every other byte of 32.
const N_PAIRS: [usize; 16] = [0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30];

/// Decodes 32 hex digits held as four little-endian words of eight into the UUID's 16
/// bytes, or `None` when any digit is not hex.
///
/// SWAR: each word is checked and converted eight digits at a time with plain integer
/// arithmetic, no table and no branch per digit, and the four results are joined in
/// registers and stored once. The table loop this replaced wrote sixteen single bytes the
/// caller then read back as one 16-byte value — a store the CPU cannot forward.
#[inline(always)]
fn decode_words(words: [u64; 4]) -> Option<[u8; 16]> {
    let mut value = 0u128;
    let mut valid = true;
    for (index, word) in words.into_iter().enumerate() {
        let (bytes, ok) = decode8(word);
        valid &= ok;
        value |= u128::from(bytes) << (32 * index);
    }
    valid.then(|| value.to_le_bytes())
}

/// Eight hex digits (a little-endian word, first digit in the low byte) to their four
/// bytes (first byte in the low byte), and whether all eight were hex digits.
#[inline(always)]
fn decode8(word: u64) -> (u32, bool) {
    const ONES: u64 = 0x0101_0101_0101_0101;
    const HIGH: u64 = 0x8080_8080_8080_8080;
    // Bytewise `byte >= floor`, as each byte's high bit, for bytes below 0x80: setting the
    // high bit first means the subtraction never borrows across bytes.
    let at_least = |x: u64, floor: u8| ((x | HIGH) - ONES * u64::from(floor)) & HIGH;
    let lower = word | (ONES * 0x20);
    let digit = at_least(word, b'0') & !at_least(word, b'9' + 1);
    let letter = at_least(lower, b'a') & !at_least(lower, b'f' + 1);
    let valid = word & HIGH == 0 && (digit | letter) == HIGH;
    // '0'-'9' end in their value; 'A'-'F' and 'a'-'f' have bit 6 set and end in value - 9.
    let nibbles = (word & (ONES * 0x0F)) + ((word >> 6) & ONES) * 9;
    // Each even byte takes its digit as the high nibble and the next as the low one...
    let pairs = ((nibbles << 4) | (nibbles >> 8)) & 0x00FF_00FF_00FF_00FF;
    // ...and the four pair bytes close ranks.
    let pairs = (pairs | pairs >> 8) & 0x0000_FFFF_0000_FFFF;
    ((pairs | pairs >> 16) as u32, valid)
}

/// The fault for text [`decode_words`] refused: the first pair at `pairs` holding a non-hex
/// byte, read again one pair at a time. Only reached on bad input.
#[cold]
fn first_bad_pair(text: &[u8], pairs: &[usize; 16], start: usize) -> Fault {
    pairs
        .iter()
        .find_map(|&at| hex_pair(text, at, start).err())
        .unwrap_or(Fault::malformed(start, text.len()))
}

/// B and P formats: a D-format UUID wrapped in `{}` or `()`.
fn parse_wrapped(text: &[u8], start: usize, close: u8) -> Result<[u8; 16], Fault> {
    if text.len() != 38 {
        return Err(Fault::malformed(start, text.len()));
    }
    let Some(inner) = text.get(1..37).filter(|_| text.last() == Some(&close)) else {
        return Err(Fault::malformed(start + 37, char_len_at(text, 37)));
    };
    parse_d(inner, start + 1)
}

/// X format: `{0xdddddddd,0xdddd,0xdddd,{0xdd,0xdd,0xdd,0xdd,0xdd,0xdd,0xdd,0xdd}}`,
/// each component 1 to its full width in hex digits, no interior whitespace.
fn parse_x(text: &[u8], start: usize) -> Result<[u8; 16], Fault> {
    let mut out = [0u8; 16];
    let mut i = 0;
    let expect = |wanted: u8, i: &mut usize| -> Result<(), Fault> {
        match text.get(*i) {
            Some(&byte) if byte == wanted => {
                *i += 1;
                Ok(())
            }
            Some(_) => Err(Fault::malformed(start + *i, char_len_at(text, *i))),
            None => Err(Fault::malformed(start, text.len())),
        }
    };

    // Reads `0x` + 1..=max_digits hex digits, big-endian into out[at..at + width].
    let component =
        |i: &mut usize, out: &mut [u8; 16], at: usize, width: usize| -> Result<(), Fault> {
            let prefixed = matches!(text.get(*i..), Some([b'0', x, ..]) if x | 0x20 == b'x');
            if !prefixed {
                let bad = (*i).min(text.len().saturating_sub(1));
                return Err(Fault::malformed(start + bad, char_len_at(text, bad)));
            }
            *i += 2;
            let mut value: u64 = 0;
            let mut digits = 0;
            while let Some(nibble) = text.get(*i).and_then(|&byte| hex(byte)) {
                if digits == width * 2 {
                    return Err(Fault::malformed(start + *i, char_len_at(text, *i)));
                }
                value = (value << 4) | nibble as u64;
                digits += 1;
                *i += 1;
            }
            if digits == 0 {
                let bad = (*i).min(text.len().saturating_sub(1));
                return Err(Fault::malformed(start + bad, char_len_at(text, bad)));
            }
            for (slot, byte) in out.iter_mut().skip(at).take(width).enumerate() {
                *byte = (value >> ((width - 1 - slot) * 8)) as u8;
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
        return Err(Fault::malformed(start + i, char_len_at(text, i)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pair-by-pair decoder the SWAR one replaced, kept as its reference.
    fn by_pairs(text: &[u8], pairs: &[usize; 16], start: usize) -> Result<[u8; 16], Fault> {
        let mut out = [0u8; 16];
        for (slot, &at) in out.iter_mut().zip(pairs) {
            *slot = hex_pair(text, at, start)?;
        }
        Ok(out)
    }

    /// Every byte value at every digit position of a D and an N UUID, mixed-case digits
    /// around it: the SWAR decoder must give the reference's bytes or its exact fault.
    #[test]
    fn swar_decode_matches_pairwise_for_every_byte_at_every_position() {
        let d = *b"0aB1c2D3-e4F5-a6b7-C8d9-E0f1A2b3C4d5";
        let n = *b"0aB1c2D3e4F5a6b7C8d9E0f1A2b3C4d5";
        for position in 0..36 {
            if D_PAIRS.iter().all(|&at| at != position && at + 1 != position) {
                continue;
            }
            for byte in 0..=255u8 {
                let mut text = d;
                text[position] = byte;
                assert_eq!(
                    parse_d(&text, 3),
                    by_pairs(&text, &D_PAIRS, 3),
                    "D, byte {byte:#04x} at {position}"
                );
            }
        }
        for position in 0..32 {
            for byte in 0..=255u8 {
                let mut text = n;
                text[position] = byte;
                assert_eq!(
                    parse_n(&text, 3),
                    by_pairs(&text, &N_PAIRS, 3),
                    "N, byte {byte:#04x} at {position}"
                );
            }
        }
    }

    /// Two bad digits: the fault is the first, as the pairwise reader reports it.
    #[test]
    fn swar_decode_faults_at_the_first_bad_digit() {
        let text = b"0aB1c2D3-e4F5-a6b7-C8d9-E0f1A2bXCgd5";
        assert_eq!(parse_d(text, 0), by_pairs(text, &D_PAIRS, 0));
        assert_eq!(parse_d(text, 0), Err(Fault::malformed(31, 1)));
    }

    /// The decoded bytes are the `uuid` crate's for a spread of values.
    #[test]
    fn swar_decode_matches_uuid_crate() {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..10_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let value = u128::from(state) << 64 | u128::from(state.rotate_left(29) ^ 0xA5A5);
            let reference = uuid::Uuid::from_u128(value);
            let hyphenated = reference.hyphenated().to_string();
            let simple = reference.simple().to_string().to_uppercase();
            assert_eq!(cast_uuid(&hyphenated), Ok(*reference.as_bytes()), "{hyphenated}");
            assert_eq!(cast_uuid(&simple), Ok(*reference.as_bytes()), "{simple}");
        }
    }
}
