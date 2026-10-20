//! The verdict vocabulary — the closed set of ways a cast can fail, the fault that carries
//! it, and the caller-declared numeric format. Ported from Svartalfheim's `ParseFailure` /
//! `Failure` pair, adjusted for the ABI: the reason crosses as an integer code, and instead
//! of capturing the offending text (which would allocate), a fault points back into the
//! caller's own input buffer as a byte span.

/// The closed set of reasons a cast can fail. Adding a member is a deliberate breaking
/// change: every exhaustive match over this enum — in the core and in every binding's
/// discriminated union — must be updated together.
///
/// `0` is reserved for "Ok" at the ABI and is never a `Reason`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// Required input was empty or whitespace. Bindings surface this as "absent" (null/None)
    /// on their optional doors — the core has one door per type, not a required/optional pair.
    Empty = 1,
    /// Input was present but not recognizable as the target type.
    Malformed = 2,
    /// Input was well-formed but the value falls outside the target's representable range —
    /// `"256"` for a u8, a timestamp past 9999-12-31, `1e400` for an f64.
    OutOfRange = 3,
}

/// The failure case of a cast: a closed reason plus the offending byte span, indexed into
/// the caller's original (untrimmed) input. `Empty` faults carry a zero span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fault {
    /// The closed-set reason.
    pub reason: Reason,
    /// Byte offset of the offending span in the caller's input.
    pub offset: u32,
    /// Byte length of the offending span.
    pub len: u32,
}

impl Reason {
    /// The reason whose ABI verdict code is `code` (`1` empty, `2` malformed, `3` out of
    /// range), or `None` for any other value — `0`, the code for success, included.
    pub const fn from_code(code: u32) -> Option<Reason> {
        Some(match code {
            1 => Reason::Empty,
            2 => Reason::Malformed,
            3 => Reason::OutOfRange,
            _ => return None,
        })
    }
}

impl core::fmt::Display for Reason {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Reason::Empty => "empty",
            Reason::Malformed => "malformed",
            Reason::OutOfRange => "out of range",
        })
    }
}

impl core::fmt::Display for Fault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.reason {
            Reason::Empty => f.write_str("empty input"),
            reason => {
                write!(f, "{reason} input at bytes {}..{}", self.offset, self.offset + self.len)
            }
        }
    }
}

/// A `Fault` composes with ordinary Rust error handling (`?` into `Box<dyn Error>`,
/// `anyhow`, `thiserror` chains) — the same first-class citizenship every binding's fault
/// type has in its own platform's error culture.
impl core::error::Error for Fault {}

impl Fault {
    pub(crate) const EMPTY: Fault = Fault { reason: Reason::Empty, offset: 0, len: 0 };

    pub(crate) fn malformed(offset: usize, len: usize) -> Fault {
        Fault { reason: Reason::Malformed, offset: clamp(offset), len: clamp(len) }
    }

    pub(crate) fn out_of_range(offset: usize, len: usize) -> Fault {
        Fault { reason: Reason::OutOfRange, offset: clamp(offset), len: clamp(len) }
    }
}

fn clamp(value: usize) -> u32 {
    value.try_into().unwrap_or(u32::MAX)
}

/// A short declared symbol, held inline so a [`NumFormat`] stays `Copy` and crosses the
/// ABI as plain bytes: a separator, a sign, a percent symbol (`Symbol<8>`) or a day-period
/// designator (`Symbol<16>`). Up to `N` bytes of UTF-8, which is wide enough for every
/// value a real culture declares: a separator of one or two code points (`.`, U+202F, an
/// LTR mark and a comma), a minus sign of up to two (U+2212; an RTL mark and a hyphen),
/// `a. m.`. Empty is [`Symbol::NONE`]: nothing declared, so the core keeps its default for
/// the field.
///
/// The one rule is that a symbol never contains an ASCII digit — a digit is a digit before
/// it is anything else, so such a symbol could never be matched, and a sign made of one
/// would read `15` as a signed `5`. [`CurrencySymbol`] keeps its own stricter rule.
///
/// ABI layout: the `N` bytes, then the length, `N + 1` bytes with no alignment — a
/// [`NumFormat`] is read across the C ABI in place, so this is the shape it crosses in.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Symbol<const N: usize> {
    bytes: [u8; N],
    len: u8,
}

/// Two symbols are equal when they declare the same bytes; what a layout read across the
/// ABI holds past the length is no part of the symbol.
impl<const N: usize> PartialEq for Symbol<N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<const N: usize> Eq for Symbol<N> {}

impl<const N: usize> Symbol<N> {
    /// The inline capacity, in UTF-8 bytes.
    pub const MAX_BYTES: usize = N;

    /// Nothing declared.
    pub const NONE: Symbol<N> = Symbol { bytes: [0; N], len: 0 };

    /// Declares `symbol`. `None` when it is empty, longer than [`MAX_BYTES`](Self::MAX_BYTES)
    /// bytes, or contains an ASCII digit.
    #[inline]
    pub const fn new(symbol: &str) -> Option<Symbol<N>> {
        let source = symbol.as_bytes();
        if source.is_empty() || source.len() > N {
            return None;
        }
        let mut bytes = [0u8; N];
        let mut i = 0;
        while i < source.len() {
            if source[i].is_ascii_digit() {
                return None;
            }
            bytes[i] = source[i];
            i += 1;
        }
        Some(Symbol { bytes, len: source.len() as u8 })
    }

    /// One character as a symbol, unchecked: the separators [`NumFormat::new`] takes as
    /// `char`s. A digit declared this way is inert (the digit scan reads it first).
    #[inline]
    pub(crate) const fn from_char(symbol: char) -> Symbol<N> {
        let code = symbol as u32;
        let mut bytes = [0u8; N];
        let len = if code < 0x80 {
            bytes[0] = code as u8;
            1
        } else if code < 0x800 {
            bytes[0] = 0xC0 | (code >> 6) as u8;
            bytes[1] = 0x80 | (code & 0x3F) as u8;
            2
        } else if code < 0x10000 {
            bytes[0] = 0xE0 | (code >> 12) as u8;
            bytes[1] = 0x80 | ((code >> 6) & 0x3F) as u8;
            bytes[2] = 0x80 | (code & 0x3F) as u8;
            3
        } else {
            bytes[0] = 0xF0 | (code >> 18) as u8;
            bytes[1] = 0x80 | ((code >> 12) & 0x3F) as u8;
            bytes[2] = 0x80 | ((code >> 6) & 0x3F) as u8;
            bytes[3] = 0x80 | (code & 0x3F) as u8;
            4
        };
        Symbol { bytes, len }
    }

    /// Whether a symbol read across the ABI keeps [`new`](Self::new)'s rules — a length of
    /// `1..=N`, no ASCII digit, well-formed UTF-8 — and, with `flush`, has no ASCII
    /// whitespace in it either (a sign or a percent symbol sits flush against the digits).
    /// Every numeric cast through the C ABI checks two separators, so the common one-byte
    /// symbol is decided without a loop or a call, and UTF-8 is checked only once a byte is
    /// outside ASCII.
    #[inline]
    pub(crate) fn check(&self, flush: bool) -> bool {
        let len = usize::from(self.len);
        if len == 0 || len > N {
            return false;
        }
        if len == 1 {
            let byte = self.bytes[0];
            return byte < 0x80 && !byte.is_ascii_digit() && !(flush && byte.is_ascii_whitespace());
        }
        let symbol = self.as_bytes();
        let mut ascii = true;
        for &byte in symbol {
            if byte.is_ascii_digit() || (flush && byte.is_ascii_whitespace()) {
                return false;
            }
            ascii &= byte < 0x80;
        }
        ascii || utf8_ok(symbol)
    }

    /// [`check`](Self::check), with nothing declared passing too.
    #[inline]
    pub(crate) fn check_optional(&self, flush: bool) -> bool {
        self.len == 0 || self.check(flush)
    }

    /// The symbol's UTF-8 bytes — empty for [`NONE`](Self::NONE).
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        // `len` never exceeds N (`new` checks); `get` says so without a bounds check.
        self.bytes.get(..usize::from(self.len)).unwrap_or_default()
    }

    /// The symbol as text — empty for [`NONE`](Self::NONE).
    pub fn as_str(&self) -> &str {
        // SAFETY: every constructor stores the bytes of a `&str` or one encoded `char`,
        // `NONE` stores nothing, and a symbol read across the ABI is only handed out once
        // `check` has seen its bytes are UTF-8 (`RawNumFormat::resolve`).
        unsafe { str::from_utf8_unchecked(self.as_bytes()) }
    }

    /// True when nothing is declared.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The symbol's length in bytes.
    pub(crate) fn len(&self) -> usize {
        usize::from(self.len)
    }

    /// The one byte of a single-byte (so ASCII) symbol — what the fast lanes read.
    #[inline]
    pub(crate) fn single_ascii(&self) -> Option<u8> {
        if self.len == 1 { self.bytes.first().copied() } else { None }
    }

    /// Whether the symbol sits at `text[at..]`. Never true for [`NONE`](Self::NONE).
    #[inline]
    pub(crate) fn matches(&self, text: &[u8], at: usize) -> bool {
        self.len != 0 && text.get(at..).is_some_and(|rest| rest.starts_with(self.as_bytes()))
    }

    /// Whether the symbol ends `text`. Never true for [`NONE`](Self::NONE).
    #[inline]
    pub(crate) fn ends(&self, text: &[u8]) -> bool {
        self.len != 0 && text.ends_with(self.as_bytes())
    }
}

/// Whether `bytes` are well-formed UTF-8 — `core::str::from_utf8`'s answer, for the few
/// bytes of a symbol read across the ABI, without the call: a numeric cast through the C
/// ABI checks its format's symbols every time, and a culture's currency symbol is outside
/// ASCII more often than not. Overlong forms, surrogates and code points past U+10FFFF are
/// refused, exactly as there.
#[inline]
fn utf8_ok(bytes: &[u8]) -> bool {
    let mut i = 0;
    while let Some(&lead) = bytes.get(i) {
        let (len, floor) = match lead {
            0x00..=0x7F => (1, 0),
            0xC2..=0xDF => (2, 0x80),
            0xE0..=0xEF => (3, 0x800),
            0xF0..=0xF4 => (4, 0x10000),
            _ => return false,
        };
        let Some(sequence) = bytes.get(i..i + len) else {
            return false;
        };
        let mut code = if len == 1 { u32::from(lead) } else { u32::from(lead) & (0x7F >> len) };
        for &trail in sequence.iter().skip(1) {
            if trail & 0xC0 != 0x80 {
                return false;
            }
            code = (code << 6) | u32::from(trail & 0x3F);
        }
        if code < floor || code > 0x10FFFF || (0xD800..=0xDFFF).contains(&code) {
            return false;
        }
        i += len;
    }
    true
}

impl<const N: usize> core::fmt::Display for Symbol<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A caller-declared currency symbol, held inline so a [`NumFormat`] stays `Copy` and
/// crosses the ABI as plain bytes. Up to [`CurrencySymbol::MAX_BYTES`] bytes of UTF-8 —
/// wide enough for every symbol a real culture declares (`$`, `€`, `kr.`, `CHF`, `R$`,
/// `руб.`). Empty is [`CurrencySymbol::NONE`]: nothing declared, nothing matched.
///
/// The symbol is matched whole, at the edges of the numeric body — leading (before or
/// after a sign) or trailing, with optional ASCII whitespace between it and the digits —
/// and only while [`NumFormat::CURRENCY`] is set. It never participates in the digit scan,
/// so a symbol that happens to contain a separator character (`kr.` under `.` grouping)
/// is fine.
///
/// ABI layout: the 16 bytes, then the length, 17 bytes with no alignment, as a [`Symbol`].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CurrencySymbol {
    bytes: [u8; Self::MAX_BYTES],
    len: u8,
}

/// Two symbols are equal when they declare the same bytes; what a layout read across the
/// ABI holds past the length is no part of the symbol.
impl PartialEq for CurrencySymbol {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for CurrencySymbol {}

impl CurrencySymbol {
    /// The inline capacity, in UTF-8 bytes.
    pub const MAX_BYTES: usize = 16;

    /// No symbol declared.
    pub const NONE: CurrencySymbol = CurrencySymbol { bytes: [0; Self::MAX_BYTES], len: 0 };

    /// Declares `symbol`. `None` when it is empty, longer than [`MAX_BYTES`](Self::MAX_BYTES)
    /// bytes, or contains an ASCII digit or ASCII whitespace — those would collide with the
    /// digit scan and the trimming the doors do around the symbol, so they are a caller
    /// bug, not a symbol.
    pub const fn new(symbol: &str) -> Option<CurrencySymbol> {
        let source = symbol.as_bytes();
        if source.is_empty() || source.len() > Self::MAX_BYTES {
            return None;
        }
        let mut bytes = [0u8; Self::MAX_BYTES];
        let mut i = 0;
        while i < source.len() {
            let byte = source[i];
            if byte.is_ascii_digit() || byte.is_ascii_whitespace() {
                return None;
            }
            bytes[i] = byte;
            i += 1;
        }
        Some(CurrencySymbol { bytes, len: source.len() as u8 })
    }

    /// Whether a symbol read across the ABI keeps [`new`](Self::new)'s rules, nothing
    /// declared passing too: a length of at most 16, no ASCII digit or whitespace, and
    /// well-formed UTF-8, which is checked only once a byte is outside ASCII.
    #[inline]
    pub(crate) fn check(&self) -> bool {
        let len = usize::from(self.len);
        if len == 0 {
            return true;
        }
        if len > Self::MAX_BYTES {
            return false;
        }
        let symbol = self.as_bytes();
        let mut ascii = true;
        for &byte in symbol {
            if byte.is_ascii_digit() || byte.is_ascii_whitespace() {
                return false;
            }
            ascii &= byte < 0x80;
        }
        ascii || utf8_ok(symbol)
    }

    /// The symbol's UTF-8 bytes — empty for [`NONE`](Self::NONE).
    pub fn as_bytes(&self) -> &[u8] {
        // `len` never exceeds MAX_BYTES (`new` checks); `get` says so without a bounds check.
        self.bytes.get(..usize::from(self.len)).unwrap_or_default()
    }

    /// The symbol as text — empty for [`NONE`](Self::NONE).
    pub fn as_str(&self) -> &str {
        // SAFETY: `new` only ever stores the bytes of a `&str`, and `NONE` stores none.
        unsafe { str::from_utf8_unchecked(self.as_bytes()) }
    }

    /// True when no symbol is declared.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// A caller-declared set of native digits, read beside ASCII `0`–`9`: the ten code points
/// from a declared zero up (`٠`, U+0660, declares the Arabic-Indic digits; `０`, U+FF10,
/// the fullwidth ones), the way ICU and Java's `DecimalFormatSymbols` describe a locale's
/// digits. [`NativeDigits::NONE`] declares none, and ASCII digits are always read.
///
/// Every decimal digit block Unicode defines starts at a code point whose ten successors
/// share its UTF-8 prefix, which is what lets a digit be recognised from its last byte;
/// [`new`](Self::new) refuses a zero for which that does not hold, and any ASCII one.
///
/// ABI layout: the zero's code point as a `u32`, `0` for `NONE`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeDigits {
    zero: u32,
}

impl NativeDigits {
    /// No native digits declared.
    pub const NONE: NativeDigits = NativeDigits { zero: 0 };

    /// Declares the ten digits starting at `zero`. `None` for an ASCII `zero`, or one whose
    /// ten successors do not share its UTF-8 prefix (a code point whose value modulo 64 is
    /// past 54 — no Unicode digit block starts at one).
    pub const fn new(zero: char) -> Option<NativeDigits> {
        let code = zero as u32;
        if code < 0x80 || code % 64 > 54 {
            return None;
        }
        Some(NativeDigits { zero: code })
    }

    /// The declared zero, or `None` for [`NONE`](Self::NONE).
    pub fn zero(&self) -> Option<char> {
        if self.zero == 0 { None } else { char::from_u32(self.zero) }
    }

    /// The ABI's spelling: the zero's code point, `0` for [`NONE`](Self::NONE).
    pub(crate) const fn zero_code(&self) -> u32 {
        self.zero
    }

    /// [`zero_code`](Self::zero_code)'s inverse, unchecked: what a layout read back as
    /// bytes holds until [`check`](Self::check) has seen it.
    pub(crate) const fn from_code(zero: u32) -> NativeDigits {
        NativeDigits { zero }
    }

    /// True when no native digits are declared.
    pub fn is_empty(&self) -> bool {
        self.zero == 0
    }

    /// Whether a set read across the ABI is one [`new`](Self::new) would declare, nothing
    /// declared passing too.
    #[inline]
    pub(crate) fn check(&self) -> bool {
        self.zero == 0 || char::from_u32(self.zero).and_then(NativeDigits::new).is_some()
    }

    /// The native digit at `text[at..]`, as its value and byte length.
    #[inline]
    pub(crate) fn read(&self, text: &[u8], at: usize) -> Option<(u8, usize)> {
        if self.zero == 0 {
            return None;
        }
        let lead = Symbol::<4>::from_char(char::from_u32(self.zero)?);
        let len = usize::from(lead.len);
        let candidate = text.get(at..at + len)?;
        let (prefix, last) = candidate.split_at_checked(len - 1)?;
        if prefix != lead.bytes.get(..len - 1)? {
            return None;
        }
        let zero = *lead.bytes.get(len - 1)?;
        let value = last.first()?.wrapping_sub(zero);
        if value <= 9 { Some((value, len)) } else { None }
    }
}

/// Caller-declared numeric notation for the integer and real doors. The core carries no
/// culture data — a binding maps its platform's culture to these fields (Svartalfheim
/// leaned on `IFormatProvider` here). Everything past the separators and the flags is
/// declared, never looked up, and every one of those fields defaults to declaring nothing,
/// which reads exactly as the invariant culture: ASCII signs and percent, ASCII digits,
/// the number separators for a currency amount.
///
/// ABI layout: the format crosses the C ABI in place, so this is `#[repr(C)]` in exactly
/// the field order below — `flags` and the digit zero as two `u32`s, then seven
/// [`Symbol<8>`]s of nine bytes each and the [`CurrencySymbol`]'s seventeen, 88 bytes with
/// 4-byte alignment; [`RawNumFormat`](crate::RawNumFormat) puts a size word ahead of it.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumFormat {
    /// Bitwise OR of the `GROUPING`/`PARENS`/`EXPONENT`/`RADIX_PREFIX`/`PERCENT`/`CURRENCY`/
    /// `TRAILING_SIGN` flags, plus the `SEPARATOR_DETECT` policy.
    pub flags: u32,
    /// Native digits read beside ASCII `0`–`9`. [`NativeDigits::NONE`] declares none.
    pub digits: NativeDigits,
    /// The declared decimal separator (`.` invariant, `,` in eurozone-style text): one or
    /// two characters. An empty symbol matches nothing, so no fraction can be written.
    pub decimal_sep: Symbol<8>,
    /// The declared digit-group separator (`,` invariant, `.` or U+00A0 elsewhere). Must
    /// differ from `decimal_sep`; the doors check the decimal separator first, so a caller
    /// passing equal separators gets decimal semantics — the FFI layer rejects the
    /// combination outright as a contract violation.
    pub group_sep: Symbol<8>,
    /// The decimal separator of a currency amount — read in place of `decimal_sep` when the
    /// declared symbol is present in the text, as the BCL switches to a culture's
    /// `CurrencyDecimalSeparator` when it sees the symbol. [`Symbol::NONE`] keeps
    /// `decimal_sep`.
    pub currency_decimal_sep: Symbol<8>,
    /// The group separator of a currency amount, likewise. [`Symbol::NONE`] keeps `group_sep`.
    pub currency_group_sep: Symbol<8>,
    /// A further spelling of the minus sign, read wherever ASCII `-` is (U+2212 in Estonian
    /// and Basque). ASCII `-` is always read; [`Symbol::NONE`] declares no other.
    pub minus_sign: Symbol<8>,
    /// A further spelling of the plus sign, likewise beside ASCII `+`.
    pub plus_sign: Symbol<8>,
    /// A further spelling of the percent symbol, read as a trailing `%` is while
    /// [`PERCENT`](Self::PERCENT) is set (`٪` in Arabic-script cultures, `％` fullwidth).
    /// ASCII `%` is always read.
    pub percent: Symbol<8>,
    /// The declared currency symbol, honored only while [`CURRENCY`](Self::CURRENCY) is
    /// set. [`CurrencySymbol::NONE`] declares nothing.
    pub currency: CurrencySymbol,
}

impl NumFormat {
    /// Permit `group_sep` between digits (group sizes are not validated — a separator must
    /// simply sit between two digits, HyperCast's own deterministic rule where .NET's
    /// `AllowThousands` deferred to per-culture quirks).
    pub const GROUPING: u32 = 1 << 0;
    /// Permit accounting parentheses as negation: `(1,234)` is -1234.
    pub const PARENS: u32 = 1 << 1;
    /// Permit exponent notation: `1e3`, `2.5E-3`. Integer doors reject a negative exponent —
    /// a decimal point is never accepted on an integer, in any disguise.
    pub const EXPONENT: u32 = 1 << 2;
    /// Permit the culture-insensitive radix prefixes `0x`/`&H` (hex) and `0b` (binary),
    /// read as the two's-complement bit pattern (`0xFF` is -1 for an i8). Integer doors only.
    pub const RADIX_PREFIX: u32 = 1 << 3;
    /// Permit a trailing `%` (or the declared [`percent`](Self::percent) symbol), dividing
    /// by 100 (`50%` is 0.5). Real doors only.
    pub const PERCENT: u32 = 1 << 4;
    /// Resolve the `.`/`,` roles per input from structure instead of the declared fields
    /// (which are ignored while this flag is set). Detection, not sniffing — only
    /// structurally unambiguous inputs resolve, by exactly these rules: a separator
    /// appearing twice or more is grouping (`1.234.567,89`); when both appear, the
    /// rightmost is the decimal (`1,234.5` vs `1.234,5`); a single separator with a
    /// non-3-digit run to its right is the decimal (`3,1415`); a single separator with
    /// exactly 3 digits right is the decimal only when the integer part is exactly `0`
    /// (`0,785` is 0.785 — `0785` would be no number at all). Everything else — `12.185`,
    /// `1,000` — is genuinely ambiguous between dialects and comes back `Malformed` at the
    /// undecidable separator, never guessed. Covers the `.`/`,` pair only; space/NBSP
    /// grouping still needs a declared format.
    pub const SEPARATOR_DETECT: u32 = 1 << 5;
    /// Permit the declared [`currency`](Self::currency) symbol at either edge of the
    /// numeric body — leading (`$5`, `-$5`, `$ -5`) or trailing (`5 €`, `1.234,50 kr.`),
    /// once, with optional ASCII whitespace between symbol and digits; accounting
    /// parentheses wrap the symbol along with the digits (`($5)`, `(1.234,50 kr.)`). With
    /// [`CurrencySymbol::NONE`] declared the flag matches nothing and changes nothing.
    /// Integer and real doors.
    pub const CURRENCY: u32 = 1 << 6;
    /// Permit the sign after the digits (`5-`, `5 -`, `5 €-`, `5- €`), the way a culture
    /// whose negative pattern is `n-` writes it and .NET's `AllowTrailingSign` reads it:
    /// ASCII `-`/`+` or the declared signs, once, with optional ASCII whitespace before it,
    /// and never beside a leading sign or inside accounting parentheses. Not part of
    /// [`ALL`](Self::ALL): a bridge sets it from the culture's own negative pattern.
    pub const TRAILING_SIGN: u32 = 1 << 7;
    /// The default leniences — every lenience flag there was before `TRAILING_SIGN`, which
    /// is declared from a culture's pattern rather than assumed ([`SEPARATOR_DETECT`](Self::SEPARATOR_DETECT)
    /// is a separator *policy*, not a lenience, and is deliberately not included either).
    pub const ALL: u32 = Self::GROUPING
        | Self::PARENS
        | Self::EXPONENT
        | Self::RADIX_PREFIX
        | Self::PERCENT
        | Self::CURRENCY;

    /// The invariant profile — `.` decimal, `,` grouping, every default lenience on, nothing
    /// else declared. This is what the FFI doors use when the caller passes a null format.
    pub const INVARIANT: NumFormat = NumFormat::new('.', ',', Self::ALL);

    /// Every field empty, the number separators included — not a format on its own (an
    /// empty separator matches nothing, and the ABI refuses it), but the bytes a layout
    /// read across the ABI starts from.
    pub(crate) const EMPTY: NumFormat = NumFormat {
        flags: 0,
        digits: NativeDigits::NONE,
        decimal_sep: Symbol::NONE,
        group_sep: Symbol::NONE,
        currency_decimal_sep: Symbol::NONE,
        currency_group_sep: Symbol::NONE,
        minus_sign: Symbol::NONE,
        plus_sign: Symbol::NONE,
        percent: Symbol::NONE,
        currency: CurrencySymbol::NONE,
    };

    /// The detection profile — every default lenience on, `.`/`,` roles resolved per input
    /// by [`SEPARATOR_DETECT`](Self::SEPARATOR_DETECT)'s structural rules.
    pub const DETECT: NumFormat = NumFormat::new('.', ',', Self::ALL | Self::SEPARATOR_DETECT);

    /// Declares the separators and flags and nothing else — the three-field constructor
    /// every binding's own format type maps onto. A separator that is an ASCII digit is
    /// inert: the digit scan reads it first.
    #[inline]
    pub const fn new(decimal_sep: char, group_sep: char, flags: u32) -> NumFormat {
        NumFormat {
            flags,
            digits: NativeDigits::NONE,
            decimal_sep: Symbol::from_char(decimal_sep),
            group_sep: Symbol::from_char(group_sep),
            currency_decimal_sep: Symbol::NONE,
            currency_group_sep: Symbol::NONE,
            minus_sign: Symbol::NONE,
            plus_sign: Symbol::NONE,
            percent: Symbol::NONE,
            currency: CurrencySymbol::NONE,
        }
    }

    /// [`new`](Self::new) with separators of one or two characters each (`Symbol::NONE`
    /// takes the invariant `.` or `,`), for a culture whose separator is wider than one.
    pub const fn with_separators(
        decimal_sep: Symbol<8>,
        group_sep: Symbol<8>,
        flags: u32,
    ) -> NumFormat {
        let mut format = NumFormat::new('.', ',', flags);
        if decimal_sep.len != 0 {
            format.decimal_sep = decimal_sep;
        }
        if group_sep.len != 0 {
            format.group_sep = group_sep;
        }
        format
    }

    /// The same format with `currency` declared. Pair with [`CURRENCY`](Self::CURRENCY)
    /// in `flags` (already part of [`ALL`](Self::ALL)) for the symbol to be honored.
    pub const fn with_currency(self, currency: CurrencySymbol) -> NumFormat {
        NumFormat { currency, ..self }
    }

    /// The same format with the separators a currency amount is read with, each
    /// [`Symbol::NONE`] keeping the number separator it stands in for.
    pub const fn with_currency_separators(
        self,
        decimal_sep: Symbol<8>,
        group_sep: Symbol<8>,
    ) -> NumFormat {
        NumFormat { currency_decimal_sep: decimal_sep, currency_group_sep: group_sep, ..self }
    }

    /// The same format with further spellings of the minus and plus signs declared, each
    /// [`Symbol::NONE`] declaring none.
    pub const fn with_signs(self, minus_sign: Symbol<8>, plus_sign: Symbol<8>) -> NumFormat {
        NumFormat { minus_sign, plus_sign, ..self }
    }

    /// The same format with a further spelling of the percent symbol declared.
    pub const fn with_percent(self, percent: Symbol<8>) -> NumFormat {
        NumFormat { percent, ..self }
    }

    /// The same format with native digits declared beside ASCII.
    pub const fn with_digits(self, digits: NativeDigits) -> NumFormat {
        NumFormat { digits, ..self }
    }

    pub(crate) fn allows(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    /// Whether a format read across the ABI is one this API would declare: each number
    /// separator one to eight bytes with no ASCII digit, the two different; each currency
    /// separator the same or empty, and the pair in force different; each sign and the
    /// percent symbol the same or empty and with no ASCII whitespace; the currency symbol
    /// a [`CurrencySymbol`] or empty; the digits a [`NativeDigits`] or none. Anything else
    /// is a caller bug, not a data verdict, and the C ABI reports it as one.
    #[inline]
    pub fn check(&self) -> bool {
        if !self.decimal_sep.check(false) || !self.group_sep.check(false) {
            return false;
        }
        // Equal separators, decided on the first byte for nearly every format: a slice
        // comparison is a call, and this runs on every cast through the C ABI.
        if self.decimal_sep.bytes[0] == self.group_sep.bytes[0]
            && self.decimal_sep.len == self.group_sep.len
            && self.decimal_sep == self.group_sep
        {
            return false;
        }
        // Everything below is declared by few formats; one word of their lengths and the
        // digit zero together decides the lot for the rest.
        let rest = u32::from(self.currency_decimal_sep.len)
            | u32::from(self.currency_group_sep.len)
            | u32::from(self.minus_sign.len)
            | u32::from(self.plus_sign.len)
            | u32::from(self.percent.len)
            | self.digits.zero;
        if rest == 0 {
            return self.currency.check();
        }
        if !self.currency_decimal_sep.check_optional(false)
            || !self.currency_group_sep.check_optional(false)
        {
            return false;
        }
        let (decimal, group) = self.separators(true);
        decimal != group
            && self.minus_sign.check_optional(true)
            && self.plus_sign.check_optional(true)
            && self.percent.check_optional(true)
            && self.currency.check()
            && self.digits.check()
    }

    /// The separators a token is read with: the currency ones where they are declared and
    /// the symbol was found, the number ones otherwise.
    #[inline]
    pub(crate) fn separators(&self, currency: bool) -> (&Symbol<8>, &Symbol<8>) {
        if currency {
            let decimal = if self.currency_decimal_sep.is_empty() {
                &self.decimal_sep
            } else {
                &self.currency_decimal_sep
            };
            let group = if self.currency_group_sep.is_empty() {
                &self.group_sep
            } else {
                &self.currency_group_sep
            };
            (decimal, group)
        } else {
            (&self.decimal_sep, &self.group_sep)
        }
    }

    /// Resolves this format's `.`/`,` roles for `text` (trimmed, offset `start` in the
    /// caller's input) under [`SEPARATOR_DETECT`](Self::SEPARATOR_DETECT)'s rules,
    /// returning a concrete declared-separator format for the ordinary engines to run
    /// with. The currency separators are resolved along with the number ones: detection
    /// governs the whole token. The one fault detection itself can produce: `Malformed` at
    /// a structurally undecidable separator.
    pub(crate) fn resolve_detected(&self, text: &[u8], start: usize) -> Result<NumFormat, Fault> {
        let flags = self.flags & !Self::SEPARATOR_DETECT;
        let (mut dots, mut commas) = (0usize, 0usize);
        let (mut last_dot, mut last_comma) = (0usize, 0usize);
        for (i, &byte) in text.iter().enumerate() {
            match byte {
                b'.' => (dots, last_dot) = (dots + 1, i),
                b',' => (commas, last_comma) = (commas + 1, i),
                _ => {}
            }
        }
        let (decimal_sep, group_sep) = match (dots, commas) {
            (0, 0) => ('.', ','),
            (_, 0) => resolve_single_sep(text, start, last_dot, dots, '.', ',')?,
            (0, _) => resolve_single_sep(text, start, last_comma, commas, ',', '.')?,
            _ if last_dot > last_comma => ('.', ','),
            _ => (',', '.'),
        };
        Ok(NumFormat {
            flags,
            decimal_sep: Symbol::from_char(decimal_sep),
            group_sep: Symbol::from_char(group_sep),
            currency_decimal_sep: Symbol::NONE,
            currency_group_sep: Symbol::NONE,
            ..*self
        })
    }
}

/// An exact decimal — a sign, a 96-bit unsigned magnitude, and a base-10 scale — the
/// shape .NET's `decimal` stores natively and Java's `BigDecimal`, Python's `Decimal` and
/// Ruby's `BigDecimal` build from directly. The value is `(-1)^negative × (hi·2⁶⁴ + lo) ×
/// 10⁻ˢᶜᵃˡᵉ`, in canonical form — exact trailing zeros in the fraction are trimmed, so the
/// scale is the smallest that represents the value and zero is scale 0. Never rounded:
/// text carrying more nonzero precision than 96 bits and 28 places can hold is
/// `OutOfRange`, not silently approximated. ABI layout: `lo` at offset 0, `hi` at 8,
/// `scale` at 12, `negative` at 13, 16 bytes with 8-byte alignment.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decimal {
    /// The low 64 bits of the magnitude.
    pub lo: u64,
    /// The high 32 bits of the magnitude; the magnitude never exceeds 2⁹⁶ − 1.
    pub hi: u32,
    /// Base-10 scale, `0..=28`: the number of places the magnitude is shifted right. Minimal
    /// by construction — the magnitude never ends in a zero while the scale is positive.
    pub scale: u8,
    /// True for a negative value. Zero is never negative.
    pub negative: bool,
}

impl Decimal {
    /// The magnitude as one integer.
    pub fn magnitude(&self) -> u128 {
        (u128::from(self.hi) << 64) | u128::from(self.lo)
    }
}

/// Renders the canonical text form — `-1234.5`, no trailing fraction zeros — the same string the
/// conformance corpus pins as `value`, so any binding's host decimal can be checked against
/// it by parsing.
impl core::fmt::Display for Decimal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // 39 digits covers u128; one more for a leading zero when scale >= digit count.
        let mut digits = [b'0'; 40];
        let mut magnitude = self.magnitude();
        let mut len = 0;
        while magnitude > 0 || len == 0 {
            digits[39 - len] = b'0' + (magnitude % 10) as u8;
            magnitude /= 10;
            len += 1;
        }
        let scale = usize::from(self.scale);
        while len <= scale {
            len += 1; // pad with leading zeros so at least one digit precedes the point
        }
        let text = &digits[40 - len..];
        let (whole, fraction) = text.split_at(len - scale);
        if self.negative {
            f.write_str("-")?;
        }
        // SAFETY: only ASCII digits were written.
        f.write_str(unsafe { str::from_utf8_unchecked(whole) })?;
        if scale > 0 {
            f.write_str(".")?;
            f.write_str(unsafe { str::from_utf8_unchecked(fraction) })?;
        }
        Ok(())
    }
}

/// The single-separator-kind arm of separator detection: repeated ⇒ grouping; a
/// non-3-digit right run ⇒ decimal; a 3-digit right run with a `0` integer part ⇒
/// decimal; anything else is undecidable.
fn resolve_single_sep(
    text: &[u8],
    start: usize,
    at: usize,
    count: usize,
    sep: char,
    other: char,
) -> Result<(char, char), Fault> {
    if count >= 2 {
        return Ok((other, sep));
    }
    // `at` always indexes the separator; were it ever past the end, the declared roles stand.
    let Some((before, [_, after @ ..])) = text.split_at_checked(at) else {
        return Ok((sep, other));
    };
    let right = after.iter().take_while(|byte| byte.is_ascii_digit()).count();
    if right != 3 {
        return Ok((sep, other));
    }
    let left = before.iter().rev().take_while(|byte| byte.is_ascii_digit()).count();
    if left == 1 && before.last() == Some(&b'0') {
        return Ok((sep, other));
    }
    Err(Fault::malformed(start + at, 1))
}

impl Default for NumFormat {
    fn default() -> Self {
        Self::INVARIANT
    }
}

/// A point in time, independent of any time zone or calendar — protobuf's
/// `google.protobuf.Timestamp` layout exactly, so every binding folds it into its platform
/// instant type without precision games.
///
/// Range: `0001-01-01T00:00:00Z` (seconds -62_135_596_800) through
/// `9999-12-31T23:59:59.999999999Z` (seconds 253_402_300_799). `nanos` counts forward from
/// `seconds` and is always in `0..=999_999_999`, even for instants before the epoch.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timestamp {
    /// Seconds since the Unix epoch `1970-01-01T00:00:00Z`.
    pub seconds: i64,
    /// Non-negative sub-second nanoseconds, `0..=999_999_999`.
    pub nanos: i32,
}

/// A calendar date with no time or zone — protobuf's `google.type.Date` field layout.
/// Years `1..=9999`, proleptic Gregorian.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    /// Year, `1..=9999`.
    pub year: u16,
    /// Month, `1..=12`.
    pub month: u8,
    /// Day of month, `1..=days_in_month(year, month)`.
    pub day: u8,
}

/// A civil (wall-clock) date and time with **no zone** — exactly what zone-less text like
/// `1/7/2026 3:04 PM` actually names. Deliberately not a [`Timestamp`]: without a zone
/// there is no instant, and inventing one (assuming UTC, say) would be a silent value
/// error of up to ±14 hours. Fusing a zone is the caller's job, per the module doctrine.
/// ABI layout: the [`Date`] fields at offset 0, `nanos_of_day` at offset 8 (after C
/// struct padding).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CivilDateTime {
    /// The calendar date.
    pub date: Date,
    /// Nanoseconds since that date's midnight, `0..86_400_000_000_000`.
    pub nanos_of_day: u64,
}

/// A signed span of time — protobuf's `google.protobuf.Duration` layout exactly.
/// `seconds` is bounded to ±315_576_000_000 (±10,000 years); `nanos` carries the same sign
/// as `seconds` (unlike [`Timestamp`], whose nanos always count forward).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Duration {
    /// Whole seconds, `-315_576_000_000..=315_576_000_000`.
    pub seconds: i64,
    /// Sub-second nanoseconds, `-999_999_999..=999_999_999`, same sign as `seconds`.
    pub nanos: i32,
}

/// Trims leading and trailing ASCII whitespace, returning the trimmed slice and the byte
/// offset it starts at in the original input — the offset every fault span is rebased by,
/// so spans always index the caller's own buffer.
pub(crate) fn trim(input: &[u8]) -> (&[u8], usize) {
    let stripped = input.trim_ascii_start();
    let start = input.len() - stripped.len();
    (stripped.trim_ascii_end(), start)
}
