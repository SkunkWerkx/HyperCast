//! UTF-8 validation for the text door: simdutf8's SIMD check where the CPU has one, and
//! `core::str::from_utf8` everywhere else and on every failure, so the fault span is always
//! core's.
//!
//! simdutf8 is taken with its `std` feature off, even when ours is on: it then picks an
//! implementation by compile-time target features only, so an x86_64 build would never use
//! SIMD unless built for AVX2. This module detects AVX2 and SSE 4.2 itself instead, with
//! `cpuid` and `xgetbv` from `core::arch` (the questions HyperTabular's delimited engine
//! asks), and caches the answer in one atomic byte. aarch64 has NEON on every target this
//! crate ships; wasm32 has simd128 only when built with it. Bare metal (`target_os = "none"`)
//! and UEFI never take the SIMD path: they are soft-float targets, where vector code either
//! doesn't compile or would clobber state the OS never agreed to save. Every other target, and
//! every input under 64 bytes (simdutf8's chunk, where it would call core itself), takes
//! core's check alone.
//!
//! Only simdutf8's `basic` check is used, which answers valid or not. Its `compat` check
//! reports where the error is, but through a path the no-panic proof refuses; core's error,
//! taken on a second pass, gives the same answer by construction.

/// `input` as text, or `core::str::from_utf8`'s error for it.
#[inline]
pub(crate) fn from_utf8(input: &[u8]) -> Result<&str, core::str::Utf8Error> {
    checked(input, fast_valid)
}

/// The check, with the fast validator given: `from_utf8` passes the one this target has, and
/// the tests pass each x86_64 level in turn.
#[inline(always)]
fn checked(input: &[u8], fast: impl FnOnce(&[u8]) -> bool) -> Result<&str, core::str::Utf8Error> {
    #[cfg(any(
        all(target_arch = "x86_64", not(target_os = "none"), not(target_os = "uefi")),
        all(
            target_arch = "aarch64",
            target_feature = "neon",
            not(target_os = "none"),
            not(target_os = "uefi")
        ),
        all(target_arch = "wasm32", target_feature = "simd128"),
    ))]
    if input.len() >= SIMD_MIN && fast(input) {
        // SAFETY: `fast` answered true, so `input` is well-formed UTF-8.
        return Ok(unsafe { core::str::from_utf8_unchecked(input) });
    }
    #[cfg(not(any(
        all(target_arch = "x86_64", not(target_os = "none"), not(target_os = "uefi")),
        all(
            target_arch = "aarch64",
            target_feature = "neon",
            not(target_os = "none"),
            not(target_os = "uefi")
        ),
        all(target_arch = "wasm32", target_feature = "simd128"),
    )))]
    let _ = fast;
    core::str::from_utf8(input)
}

/// Below this many bytes simdutf8 would call `core::str::from_utf8` itself.
#[cfg(any(
    all(target_arch = "x86_64", not(target_os = "none"), not(target_os = "uefi")),
    all(
        target_arch = "aarch64",
        target_feature = "neon",
        not(target_os = "none"),
        not(target_os = "uefi")
    ),
    all(target_arch = "wasm32", target_feature = "simd128"),
))]
const SIMD_MIN: usize = 64;

/// The x86_64 levels: no SIMD check, SSE 4.2, AVX2.
#[cfg(any(
    test,
    all(
        target_arch = "x86_64",
        not(target_feature = "avx2"),
        not(target_env = "sgx"),
        not(target_os = "none"),
        not(target_os = "uefi")
    )
))]
mod levels {
    pub(super) const NONE: u8 = 1;
    pub(super) const SSE42: u8 = 2;
    pub(super) const AVX2: u8 = 3;

    /// The check a CPU can take, from what cpuid and xgetbv answered: AVX2 when leaf 7 exists
    /// and has AVX2, leaf 1 has AVX and OSXSAVE, and XCR0 says the OS saves SSE and AVX state;
    /// else SSE 4.2 when leaf 1 has it; else none. Pure, so it is tested on made-up registers
    /// as well as on the CPU at hand.
    pub(super) const fn level(max_leaf: u32, leaf1_ecx: u32, leaf7_ebx: u32, xcr0: u64) -> u8 {
        let sse42 = leaf1_ecx & (1 << 20) != 0;
        let osxsave = leaf1_ecx & (1 << 27) != 0;
        let avx = leaf1_ecx & (1 << 28) != 0;
        let avx2 = max_leaf >= 7 && leaf7_ebx & (1 << 5) != 0;
        if avx2 && avx && osxsave && xcr0 & 0b110 == 0b110 {
            AVX2
        } else if sse42 {
            SSE42
        } else {
            NONE
        }
    }
}

/// x86_64 built without AVX2: which SIMD check this CPU has, asked once.
#[cfg(all(
    target_arch = "x86_64",
    not(target_feature = "avx2"),
    not(target_env = "sgx"),
    not(target_os = "none"),
    not(target_os = "uefi")
))]
mod x86 {
    use super::levels::{AVX2, SSE42, level};
    use core::sync::atomic::{AtomicU8, Ordering};

    /// Not yet asked: the cache's first value.
    const UNKNOWN: u8 = 0;

    static LEVEL: AtomicU8 = AtomicU8::new(UNKNOWN);

    /// Whether `input` is well-formed UTF-8, by the best check this CPU has.
    #[inline]
    pub(super) fn fast_valid(input: &[u8]) -> bool {
        let mut level = LEVEL.load(Ordering::Relaxed);
        if level == UNKNOWN {
            level = detect();
            LEVEL.store(level, Ordering::Relaxed);
        }
        fast_valid_at(input, level)
    }

    /// Whether `input` is well-formed UTF-8, by the check for `level`, which must be at most
    /// what [`detect`] answers on this CPU.
    #[inline]
    pub(super) fn fast_valid_at(input: &[u8], level: u8) -> bool {
        match level {
            // SAFETY: `level` came from `detect`, which saw AVX2 in cpuid and its state
            // enabled in XCR0 (or, in the tests, is at most what `detect` answered).
            AVX2 => unsafe { simdutf8::basic::imp::x86::avx2::validate_utf8(input) }.is_ok(),
            // SAFETY: as above, for SSE 4.2 in cpuid.
            SSE42 => unsafe { simdutf8::basic::imp::x86::sse42::validate_utf8(input) }.is_ok(),
            _ => core::str::from_utf8(input).is_ok(),
        }
    }

    /// This CPU's level: its cpuid and xgetbv answers, read only where they exist, through
    /// [`level`].
    #[cold]
    pub(super) fn detect() -> u8 {
        use core::arch::x86_64::{__cpuid, __cpuid_count};
        // SAFETY: cpuid exists on every x86_64 CPU (unsafe only before Rust 1.94).
        #[allow(unused_unsafe)]
        let (max, leaf1_ecx) = unsafe { (__cpuid(0).eax, __cpuid(1).ecx) };
        // SAFETY: leaf 7 exists when `max >= 7` (unsafe only before Rust 1.94).
        #[allow(unused_unsafe)]
        let leaf7_ebx = if max >= 7 { unsafe { __cpuid_count(7, 0) }.ebx } else { 0 };
        let xcr0 = if leaf1_ecx & (1 << 27) != 0 {
            // SAFETY: OSXSAVE is set, so xgetbv is available and the OS manages XCR0.
            unsafe { xgetbv0() }
        } else {
            0
        };
        level(max, leaf1_ecx, leaf7_ebx, xcr0)
    }

    /// XCR0, the state the OS saves on a context switch.
    ///
    /// # Safety
    /// The CPU has XSAVE and the OS has set OSXSAVE.
    #[target_feature(enable = "xsave")]
    unsafe fn xgetbv0() -> u64 {
        // SAFETY: per the function contract.
        unsafe { core::arch::x86_64::_xgetbv(0) }
    }
}

#[cfg(all(
    target_arch = "x86_64",
    not(target_feature = "avx2"),
    not(target_env = "sgx"),
    not(target_os = "none"),
    not(target_os = "uefi")
))]
use x86::fast_valid;

/// Whether `input` is well-formed UTF-8, by the check this target was built with.
#[cfg(any(
    all(
        target_arch = "x86_64",
        any(target_feature = "avx2", target_env = "sgx"),
        not(target_os = "none"),
        not(target_os = "uefi")
    ),
    all(
        target_arch = "aarch64",
        target_feature = "neon",
        not(target_os = "none"),
        not(target_os = "uefi")
    ),
    all(target_arch = "wasm32", target_feature = "simd128"),
))]
#[inline]
fn fast_valid(input: &[u8]) -> bool {
    simdutf8::basic::from_utf8(input).is_ok()
}

/// No SIMD check on this target: `checked` never calls this.
#[cfg(not(any(
    all(target_arch = "x86_64", not(target_os = "none"), not(target_os = "uefi")),
    all(
        target_arch = "aarch64",
        target_feature = "neon",
        not(target_os = "none"),
        not(target_os = "uefi")
    ),
    all(target_arch = "wasm32", target_feature = "simd128"),
)))]
#[inline]
fn fast_valid(_: &[u8]) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::levels::{AVX2, NONE, SSE42, level};
    use super::{checked, from_utf8};
    use crate::{Reason, cast_text};

    /// What the check answers for `input`, under `at`: `None` is the shipped path (detection
    /// and its cache); `Some(level)` is that x86_64 level, given per call so that tests running
    /// in parallel never share it.
    fn answer(input: &[u8], at: Option<u8>) -> Result<(*const u8, usize), (usize, Option<usize>)> {
        let result = match at {
            None => from_utf8(input),
            #[cfg(all(
                target_arch = "x86_64",
                not(target_feature = "avx2"),
                not(target_env = "sgx"),
                not(target_os = "none"),
                not(target_os = "uefi")
            ))]
            Some(level) => checked(input, |input| super::x86::fast_valid_at(input, level)),
            #[allow(unreachable_patterns)]
            Some(_) => checked(input, super::fast_valid),
        };
        // Compared as positions, never printed as text: a wrong `Ok` holds ill-formed UTF-8.
        result.map(|text| (text.as_ptr(), text.len())).map_err(|e| (e.valid_up_to(), e.error_len()))
    }

    /// The door's validation must answer exactly as core does: the same `&str` (pointer and
    /// length) or the same error.
    fn same(input: &[u8], at: Option<u8>) {
        let core = core::str::from_utf8(input)
            .map(|text| (text.as_ptr(), text.len()))
            .map_err(|e| (e.valid_up_to(), e.error_len()));
        assert_eq!(answer(input, at), core, "{at:?} {input:02x?}");
    }

    /// Runs `check` under every check the target can take: on x86_64 built without AVX2, each
    /// level this CPU has, given per call; then, everywhere, the shipped path.
    fn every_level(mut check: impl FnMut(Option<u8>)) {
        #[cfg(all(
            target_arch = "x86_64",
            not(target_feature = "avx2"),
            not(target_env = "sgx"),
            not(target_os = "none"),
            not(target_os = "uefi")
        ))]
        {
            let best = super::x86::detect();
            for level in [NONE, SSE42, AVX2] {
                if level <= best {
                    check(Some(level));
                }
            }
        }
        check(None);
    }

    #[test]
    fn the_level_follows_the_registers() {
        const SSE42_BIT: u32 = 1 << 20;
        const OSXSAVE: u32 = 1 << 27;
        const AVX: u32 = 1 << 28;
        const AVX2_BIT: u32 = 1 << 5;
        const ALL: u32 = SSE42_BIT | OSXSAVE | AVX;
        // Full AVX2: leaf 7 present, AVX and OSXSAVE, and the OS saves SSE and AVX state.
        assert_eq!(level(7, ALL, AVX2_BIT, 0b111), AVX2);
        assert_eq!(level(13, ALL, AVX2_BIT, 0b110), AVX2);
        // AVX2 in cpuid but the OS saves no YMM state (XCR0 bit 2), or nothing at all.
        assert_eq!(level(7, ALL, AVX2_BIT, 0b011), SSE42);
        assert_eq!(level(7, ALL, AVX2_BIT, 0b100), SSE42);
        assert_eq!(level(7, ALL, AVX2_BIT, 0), SSE42);
        // No OSXSAVE, no AVX, or leaf 7 absent (its register is then garbage, not an answer).
        assert_eq!(level(7, SSE42_BIT | AVX, AVX2_BIT, 0b111), SSE42);
        assert_eq!(level(7, SSE42_BIT | OSXSAVE, AVX2_BIT, 0b111), SSE42);
        assert_eq!(level(6, ALL, AVX2_BIT, 0b111), SSE42);
        // AVX but not AVX2.
        assert_eq!(level(7, ALL, 0, 0b111), SSE42);
        // SSE 4.2 alone.
        assert_eq!(level(1, SSE42_BIT, 0, 0), SSE42);
        // No SSE 4.2: no SIMD check, whatever else is set but AVX2's whole condition.
        assert_eq!(level(7, 0, 0, 0), NONE);
        assert_eq!(level(1, 0, 0, 0), NONE);
        assert_eq!(level(7, OSXSAVE | AVX, 0, 0b111), NONE);
        assert_eq!(level(7, OSXSAVE | AVX, AVX2_BIT, 0b011), NONE);
    }

    #[cfg(all(
        target_arch = "x86_64",
        not(target_feature = "avx2"),
        not(target_env = "sgx"),
        not(target_os = "none"),
        not(target_os = "uefi")
    ))]
    #[test]
    fn detection_agrees_with_std() {
        let want = if std::arch::is_x86_feature_detected!("avx2") {
            AVX2
        } else if std::arch::is_x86_feature_detected!("sse4.2") {
            SSE42
        } else {
            NONE
        };
        assert_eq!(super::x86::detect(), want);
    }

    /// Valid and invalid pieces, the ones a SIMD check gets wrong first: cut and overlong
    /// sequences, surrogates, code points past U+10FFFF, lone continuations.
    const PIECES: &[&[u8]] = &[
        b"a",
        b"\x7f",
        "é".as_bytes(),
        "€".as_bytes(),
        "𝄞".as_bytes(),
        "\u{10FFFF}".as_bytes(),
        "\u{FFFD}".as_bytes(),
        b"\x80",
        b"\xbf",
        b"\xc0\x80",
        b"\xc1\xbf",
        b"\xc2",
        b"\xe0\x80\x80",
        b"\xe0\xa0",
        b"\xed\xa0\x80",
        b"\xed\xbf\xbf",
        b"\xef\xbf",
        b"\xf0\x80\x80\x80",
        b"\xf0\x90\x80",
        b"\xf4\x90\x80\x80",
        b"\xf5\x80\x80\x80",
        b"\xf8\x88\x80\x80\x80",
        b"\xfc\x84\x80\x80\x80\x80",
        b"\xfe",
        b"\xff",
    ];

    /// The bytes worth trying in each position of a sequence.
    const EDGES: [u8; 16] = [
        0x00, 0x41, 0x7f, 0x80, 0x8f, 0x90, 0x9f, 0xa0, 0xbf, 0xc0, 0xc2, 0xdf, 0xe0, 0xef, 0xf4,
        0xff,
    ];

    #[test]
    fn answers_as_core_at_63_64_and_65_bytes() {
        every_level(|lv| {
            for len in [63, 64, 65] {
                let ascii = [b'a'; 65];
                same(&ascii[..len], lv);
                for piece in PIECES {
                    for at in [0, len.saturating_sub(piece.len()), len - 1, 31, 32] {
                        let mut input = [b'a'; 72];
                        let end = (at + piece.len()).min(input.len());
                        input[at..end].copy_from_slice(&piece[..end - at]);
                        same(&input[..len], lv);
                    }
                }
            }
        });
    }

    #[test]
    fn a_bad_last_byte_is_seen_at_every_length_from_64() {
        // A check that skipped the input's last byte would pass all of these.
        every_level(|lv| {
            for len in 64..=200 {
                for last in [&b"\x80"[..], b"\xff", b"\xc3", b"\xe2\x82", b"\xf0\x9f\x98"] {
                    let mut input = [b'a'; 200];
                    input[len - last.len()..len].copy_from_slice(last);
                    let input = &input[..len];
                    assert!(answer(input, lv).is_err(), "{len} {last:02x?}");
                    same(input, lv);
                }
            }
        });
    }

    #[test]
    fn every_two_and_edge_three_and_four_byte_sequence_across_the_chunk_boundary() {
        every_level(|lv| {
            let mut base = [b'a'; 128];
            "é€𝄞".bytes().zip(base.iter_mut()).for_each(|(b, slot)| *slot = b);
            for at in [61, 62, 63, 64, 65] {
                for pair in 0..=u16::MAX {
                    let mut input = base;
                    input[at..at + 2].copy_from_slice(&pair.to_be_bytes());
                    same(&input, lv);
                    same(&input[..at + 2], lv);
                }
                for lead in 0xc0..=0xff_u8 {
                    for b2 in EDGES {
                        for b3 in EDGES {
                            let mut input = base;
                            input[at..at + 3].copy_from_slice(&[lead, b2, b3]);
                            same(&input, lv);
                            same(&input[..at + 3], lv);
                            if lead >= 0xf0 {
                                for b4 in EDGES {
                                    input[at + 3] = b4;
                                    same(&input, lv);
                                    same(&input[..at + 4], lv);
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    #[test]
    fn answers_as_core_on_random_text() {
        // xorshift64*: deterministic, no dependency.
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_f491_4f6c_dd1d)
        };
        let mut input = [0u8; 600];
        every_level(|lv| {
            for _ in 0..4_000 {
                let len = match next() % 4 {
                    0 => (next() % 600) as usize,
                    _ => 40 + (next() % 160) as usize,
                };
                let mut at = 0;
                while at < len {
                    let piece: &[u8] = match next() % 8 {
                        0 => PIECES[(next() % PIECES.len() as u64) as usize],
                        1 => &[(next() & 0xff) as u8],
                        2..=4 => PIECES[(next() % 7) as usize],
                        _ => b"plain text, ",
                    };
                    let end = (at + piece.len()).min(len);
                    input[at..end].copy_from_slice(&piece[..end - at]);
                    at = end;
                }
                same(&input[..len], lv);
            }
        });
    }

    #[test]
    fn the_door_spans_the_first_bad_sequence_past_the_chunk() {
        // The door's fault, at the SIMD lengths: a lone continuation at 64 spans one byte, a
        // sequence cut by the end of the input spans to the end. Read through `err()`, since a
        // wrong `Ok` holds ill-formed UTF-8 and must not be printed.
        let fault = |input: &[u8]| cast_text(input).err().map(|f| (f.reason, f.offset, f.len));
        let mut input = [b'a'; 70];
        input[64] = 0x80;
        assert_eq!(fault(&input), Some((Reason::Malformed, 64, 1)));
        let mut input = [b'a'; 66];
        input[64..].copy_from_slice(b"\xe2\x82");
        assert_eq!(fault(&input), Some((Reason::Malformed, 64, 2)));
        let text = "€".repeat(30);
        assert_eq!(cast_text(text.as_bytes()), Ok(text.as_str()));
    }
}
