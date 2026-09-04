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
            reason => write!(
                f,
                "{reason} input at bytes {}..{}",
                self.offset,
                self.offset + self.len
            ),
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurrencySymbol {
    bytes: [u8; Self::MAX_BYTES],
    len: u8,
}

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

    /// The symbol's UTF-8 bytes — empty for [`NONE`](Self::NONE).
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
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

/// Caller-declared numeric notation for the integer and real doors. The core carries no
/// culture data — a binding maps its platform's culture to these fields (Svartalfheim
/// leaned on `IFormatProvider` here). The currency symbol is the one field that needs a
/// culture table to fill in; it is declared, never looked up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumFormat {
    /// The declared decimal separator (`.` invariant, `,` in eurozone-style text).
    pub decimal_sep: char,
    /// The declared digit-group separator (`,` invariant, `.` or U+00A0 elsewhere).
    /// Must differ from `decimal_sep`; the doors check the decimal separator first, so a
    /// caller passing equal separators gets decimal semantics — the FFI layer rejects the
    /// combination outright as a contract violation.
    pub group_sep: char,
    /// Bitwise OR of the `GROUPING`/`PARENS`/`EXPONENT`/`RADIX_PREFIX`/`PERCENT`/`CURRENCY`
    /// flags, plus the `SEPARATOR_DETECT` policy.
    pub flags: u32,
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
    /// Permit a trailing `%`, dividing by 100 (`50%` is 0.5). Real doors only.
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
    /// Every lenience flag set (the declared-separator flags — [`SEPARATOR_DETECT`](Self::SEPARATOR_DETECT)
    /// is a separator *policy*, not a lenience, and is deliberately not included).
    pub const ALL: u32 = Self::GROUPING
        | Self::PARENS
        | Self::EXPONENT
        | Self::RADIX_PREFIX
        | Self::PERCENT
        | Self::CURRENCY;

    /// The invariant profile — `.` decimal, `,` grouping, every lenience on, no currency
    /// symbol declared. This is what the FFI doors use when the caller passes a null format.
    pub const INVARIANT: NumFormat = NumFormat {
        decimal_sep: '.',
        group_sep: ',',
        flags: Self::ALL,
        currency: CurrencySymbol::NONE,
    };

    /// The detection profile — every lenience on, `.`/`,` roles resolved per input by
    /// [`SEPARATOR_DETECT`](Self::SEPARATOR_DETECT)'s structural rules.
    pub const DETECT: NumFormat = NumFormat {
        decimal_sep: '.',
        group_sep: ',',
        flags: Self::ALL | Self::SEPARATOR_DETECT,
        currency: CurrencySymbol::NONE,
    };

    /// Declares the separators and flags with no currency symbol — the three-field
    /// constructor every binding's own format type maps onto.
    pub const fn new(decimal_sep: char, group_sep: char, flags: u32) -> NumFormat {
        NumFormat { decimal_sep, group_sep, flags, currency: CurrencySymbol::NONE }
    }

    /// The same format with `currency` declared. Pair with [`CURRENCY`](Self::CURRENCY)
    /// in `flags` (already part of [`ALL`](Self::ALL)) for the symbol to be honored.
    pub const fn with_currency(self, currency: CurrencySymbol) -> NumFormat {
        NumFormat { currency, ..self }
    }

    pub(crate) fn allows(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    /// Resolves this format's `.`/`,` roles for `text` (trimmed, offset `start` in the
    /// caller's input) under [`SEPARATOR_DETECT`](Self::SEPARATOR_DETECT)'s rules,
    /// returning a concrete declared-separator format for the ordinary engines to run
    /// with. The one fault detection itself can produce: `Malformed` at a structurally
    /// undecidable separator.
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
        Ok(NumFormat { decimal_sep, group_sep, flags, currency: self.currency })
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
    let right = text[at + 1..].iter().take_while(|byte| byte.is_ascii_digit()).count();
    if right != 3 {
        return Ok((sep, other));
    }
    let left = text[..at].iter().rev().take_while(|byte| byte.is_ascii_digit()).count();
    if left == 1 && text[at - 1] == b'0' {
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
