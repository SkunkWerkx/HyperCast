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

/// Caller-declared numeric notation for the integer and real doors. The core carries no
/// culture data — a binding maps its platform's culture to these three fields (Svartalfheim
/// leaned on `IFormatProvider` here; currency symbols, the one notation that truly needs
/// culture tables, are deliberately dropped).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumFormat {
    /// The declared decimal separator (`.` invariant, `,` in eurozone-style text).
    pub decimal_sep: char,
    /// The declared digit-group separator (`,` invariant, `.` or U+00A0 elsewhere).
    /// Must differ from `decimal_sep`; the doors check the decimal separator first, so a
    /// caller passing equal separators gets decimal semantics — the FFI layer rejects the
    /// combination outright as a contract violation.
    pub group_sep: char,
    /// Bitwise OR of the `GROUPING`/`PARENS`/`EXPONENT`/`RADIX_PREFIX`/`PERCENT` flags.
    pub flags: u32,
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
    /// Every flag set.
    pub const ALL: u32 =
        Self::GROUPING | Self::PARENS | Self::EXPONENT | Self::RADIX_PREFIX | Self::PERCENT;

    /// The invariant profile — `.` decimal, `,` grouping, every lenience on. This is what
    /// the FFI doors use when the caller passes a null format.
    pub const INVARIANT: NumFormat =
        NumFormat { decimal_sep: '.', group_sep: ',', flags: Self::ALL };

    pub(crate) fn allows(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
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
