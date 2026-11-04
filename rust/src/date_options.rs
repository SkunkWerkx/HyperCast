//! Opt-in date and time shapes for the [`detailed`](crate::detailed) date doors.
//!
//! The doors keep today's shape rules unless the caller passes options: a caller reading
//! strict RFC 3339 may rely on a space separator or `+00` being refused. Four shapes can be
//! opened, each at the door that reads it:
//!
//! | Option | Door | Accepts |
//! |---|---|---|
//! | [`COMPACT_DATE`](DateOptions::COMPACT_DATE) | `cast_date` | `20260107` (eight ASCII digits) |
//! | [`SPACE_SEPARATOR`](DateOptions::SPACE_SEPARATOR) | `cast_timestamp` | one space in place of `T` |
//! | [`HOUR_OFFSET`](DateOptions::HOUR_OFFSET) | `cast_timestamp` | `±hh` offsets (`+00`, `-05`) |
//! | a pivot | `cast_date_ordered`, `cast_datetime` | two-digit years |
//! | [`MonthNames`] | `cast_date_ordered`, `cast_datetime` | declared month names |
//!
//! [`DateOptions::COMMON_FORMS`] turns on the three flags together and works at every date
//! door unchanged: an option a door doesn't read is ignored. It never supplies a pivot or a
//! vocabulary. A declaration is validated once, by [`DateOptions::validate`], into the
//! [`ValidDateOptions`] the doors take; a refusal is the caller's [`DeclarationError`], never a
//! [`Failure`](crate::Failure).
//!
//! ```
//! use hypercast::{DateOptions, DateOrder, MonthNames, detailed};
//!
//! let names = [
//!     (1, "Jan"), (2, "Feb"), (3, "Mar"), (4, "Apr"), (5, "May"), (6, "Jun"),
//!     (7, "Jul"), (8, "Aug"), (9, "Sep"), (9, "Sept"), (10, "Oct"), (11, "Nov"), (12, "Dec"),
//! ];
//! let options = DateOptions::NONE
//!     .with_pivot(2029)
//!     .validate(Some(MonthNames::from_entries(&names)))
//!     .unwrap();
//! let date = detailed::cast_date_ordered("07-Jan-26", DateOrder::DayMonthYear, Some(&options));
//! assert_eq!(date.map(|d| (d.year, d.month, d.day)), Ok((2026, 1, 7)));
//! ```

use core::fmt;

use crate::text::{eq_folded, fold, is_white_space, white_space_len};
use crate::verdict::DeclarationError;

/// The month names a name entry may hold, in bytes.
const MAX_NAME_BYTES: usize = 64;

/// The bytes of one packed entry before its name: the month byte and the `u32` length.
const ENTRY_HEAD: usize = 5;

/// The options that open the date doors' extra shapes. Built from the constants below, and
/// judged by [`validate`](Self::validate): an option bit this core doesn't define, or a pivot
/// outside 100–9999, is refused there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DateOptions {
    flags: u32,
    pivot: u16,
}

impl DateOptions {
    /// `cast_date` also reads `yyyyMMdd`, eight ASCII digits.
    pub const COMPACT_DATE: u32 = 1;
    /// `cast_timestamp` also reads a single space where the `T` goes.
    pub const SPACE_SEPARATOR: u32 = 2;
    /// `cast_timestamp` also reads an offset of `±hh`, with no minutes.
    pub const HOUR_OFFSET: u32 = 4;
    /// Every flag this core defines.
    const DEFINED: u32 = Self::COMPACT_DATE | Self::SPACE_SEPARATOR | Self::HOUR_OFFSET;

    /// No option: every door reads exactly the shapes it always has.
    pub const NONE: DateOptions = DateOptions { flags: 0, pivot: 0 };

    /// [`COMPACT_DATE`](Self::COMPACT_DATE), [`SPACE_SEPARATOR`](Self::SPACE_SEPARATOR) and
    /// [`HOUR_OFFSET`](Self::HOUR_OFFSET) together: the shapes real feeds mix with RFC 3339.
    /// No pivot and no vocabulary; an option a door doesn't read is ignored.
    pub const COMMON_FORMS: DateOptions = DateOptions { flags: Self::DEFINED, pivot: 0 };

    /// These option bits, no pivot. A bit this core doesn't define is kept, to be refused by
    /// [`validate`](Self::validate): the form a C caller's `flags` word arrives in.
    pub const fn from_flags(flags: u32) -> DateOptions {
        DateOptions { flags, pivot: 0 }
    }

    /// These options with a pivot: the last year of the 100-year window a two-digit `yy` is
    /// read in, the one year in `[pivot−99, pivot]` that ends in `yy`. `0` is no pivot, and
    /// then a two-digit year is a width fault. [`validate`](Self::validate) accepts 100–9999,
    /// so the window stays within years 1–9999.
    pub const fn with_pivot(self, pivot: u16) -> DateOptions {
        DateOptions { pivot, ..self }
    }

    /// The option bits.
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    /// The pivot, or `0` for none.
    pub const fn pivot(&self) -> u16 {
        self.pivot
    }

    /// The options and the month vocabulary, if any, validated for the detailed date doors:
    /// the [`DeclarationError`] naming what is wrong when an option bit isn't defined (first),
    /// the pivot isn't 0 or 100–9999 (second), or the vocabulary breaks a rule (last). The
    /// vocabulary is judged in full here, on every call; nothing is kept between calls.
    ///
    /// The vocabulary is independent of the options: `DateOptions::NONE` with names reads
    /// names and nothing else.
    pub fn validate<'a>(
        self,
        months: Option<MonthNames<'a>>,
    ) -> Result<ValidDateOptions<'a>, DeclarationError> {
        if self.flags & !Self::DEFINED != 0 {
            return Err(DeclarationError::DateFlags);
        }
        if self.pivot != 0 && !(100..=9999).contains(&self.pivot) {
            return Err(DeclarationError::DatePivot);
        }
        if let Some(months) = &months {
            months.judge().map_err(DeclarationError::MonthNames)?;
        }
        Ok(ValidDateOptions { flags: self.flags, pivot: self.pivot, months })
    }
}

/// [`DateOptions`] and their vocabulary after [`DateOptions::validate`] accepted them: what the
/// [`detailed`](crate::detailed) date doors take, so a declaration they could not honour never
/// reaches them.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct ValidDateOptions<'a> {
    flags: u32,
    pivot: u16,
    months: Option<MonthNames<'a>>,
}

impl<'a> ValidDateOptions<'a> {
    /// No option and no vocabulary: the doors' own shapes.
    pub const NONE: ValidDateOptions<'a> = ValidDateOptions { flags: 0, pivot: 0, months: None };

    /// The option bits.
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    /// The pivot, or `0` for none.
    pub const fn pivot(&self) -> u16 {
        self.pivot
    }

    /// The month vocabulary, if one was declared.
    pub const fn months(&self) -> Option<&MonthNames<'a>> {
        self.months.as_ref()
    }

    /// Whether `flag` ([`DateOptions::COMPACT_DATE`] and the others) is on.
    #[inline(always)]
    pub(crate) const fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
}

impl Default for ValidDateOptions<'_> {
    fn default() -> Self {
        Self::NONE
    }
}

/// Which rule a month vocabulary breaks. The rules are the vocabulary's whole contract: each
/// entry names a month 1–12; every month has an entry; no name is empty, over 64 bytes, or
/// holds an ASCII digit, `/`, `-` or white space (`.` is fine anywhere); and no two names
/// fold to the same spelling, even for one month.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonthNamesFault {
    /// A packed entry is cut short, or bytes trail the last whole entry.
    Layout,
    /// An entry's month is outside 1–12.
    Month,
    /// An entry's name is not UTF-8.
    Utf8,
    /// An entry's name is empty, or longer than 64 bytes.
    NameLength,
    /// An entry's name holds an ASCII digit, `/`, `-` or white space.
    Character,
    /// A month has no entry.
    MonthMissing,
    /// Two names fold to the same spelling.
    Duplicate,
}

impl fmt::Display for MonthNamesFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MonthNamesFault::Layout => "an entry is cut short, or bytes trail the last one",
            MonthNamesFault::Month => "an entry's month is outside 1 to 12",
            MonthNamesFault::Utf8 => "an entry's name is not UTF-8",
            MonthNamesFault::NameLength => "an entry's name is empty or longer than 64 bytes",
            MonthNamesFault::Character => {
                "an entry's name holds an ASCII digit, a slash, a hyphen or white space"
            }
            MonthNamesFault::MonthMissing => "a month has no name",
            MonthNamesFault::Duplicate => "two names fold to the same spelling",
        })
    }
}

impl core::error::Error for MonthNamesFault {}

/// Where a vocabulary's entries live: as the C ABI packs them, or as Rust values.
#[derive(Clone, Copy, Debug)]
enum Source<'a> {
    Packed(&'a [u8]),
    Entries(&'a [(u8, &'a str)]),
}

/// Declared month names for the date doors that read a month field: `janvier`, `Sep` and
/// `Sept`, `janv.`. Caller-built: no platform vocabulary is looked up.
///
/// Each entry maps a name to a month 1–12, and aliases are separate entries. Names match under
/// simple case folding, and the longest entry that is followed by the separator the date is
/// using wins. Punctuation is part of a name and never stripped: with `janv.` declared and `.`
/// as the separator, `7.janv..2026` reads, while `7.janv.2026` doesn't unless `janv` is also
/// declared.
///
/// A vocabulary is built over a packed buffer ([`from_packed`](Self::from_packed), the layout
/// the C ABI takes) or over Rust values ([`from_entries`](Self::from_entries), `const`), and is
/// judged by [`DateOptions::validate`]; [`pack`](Self::pack) writes the packed layout.
///
/// The packed layout: each entry is a month byte (1–12), a little-endian `u32` byte length,
/// then that many UTF-8 bytes, with no padding.
#[derive(Clone, Copy, Debug)]
pub struct MonthNames<'a> {
    source: Source<'a>,
    /// Whether [`from_packed`](Self::from_packed) already judged every rule.
    judged: bool,
}

impl<'a> MonthNames<'a> {
    /// A vocabulary over these `(month, name)` entries. Not judged here: a rule a vocabulary
    /// breaks is reported by [`DateOptions::validate`].
    pub const fn from_entries(entries: &'a [(u8, &'a str)]) -> MonthNames<'a> {
        MonthNames { source: Source::Entries(entries), judged: false }
    }

    /// A vocabulary read from its packed layout, judged in full: the
    /// [`DeclarationError`] says which rule the bytes break. The caller owns the buffer;
    /// the vocabulary borrows it.
    pub fn from_packed(bytes: &'a [u8]) -> Result<MonthNames<'a>, DeclarationError> {
        let names = MonthNames { source: Source::Packed(bytes), judged: false };
        names.judge().map_err(DeclarationError::MonthNames)?;
        Ok(MonthNames { judged: true, ..names })
    }

    /// Packs the vocabulary into `out` in the layout [`from_packed`](Self::from_packed) reads,
    /// and returns the bytes it takes, as [`Constraints::pack_enum`](crate::Constraints::pack_enum)
    /// does: when `out` is shorter than that, nothing is written and the length is still
    /// returned, so a caller sizes its buffer and calls again. `None` when the packed list
    /// would take more than `usize::MAX` bytes, or holds a name too long for its length word
    /// (a vocabulary [`validate`](DateOptions::validate) refuses); nothing is written then
    /// either. Never allocates.
    pub fn pack(&self, out: &mut [u8]) -> Option<usize> {
        let len = match self.source {
            Source::Packed(bytes) => bytes.len(),
            Source::Entries(entries) => entries.iter().try_fold(0usize, |total, &(_, name)| {
                u32::try_from(name.len()).ok()?;
                total.checked_add(name.len())?.checked_add(ENTRY_HEAD)
            })?,
        };
        let Some(out) = out.get_mut(..len) else {
            return Some(len);
        };
        match self.source {
            Source::Packed(bytes) => out.copy_from_slice(bytes),
            Source::Entries(entries) => {
                let mut rest = out;
                for &(month, name) in entries {
                    // `len` counted every byte written here and refused a name past `u32::MAX`,
                    // so neither split comes up short.
                    let Some((head, tail)) = rest.split_at_mut_checked(ENTRY_HEAD) else { break };
                    let Some((bytes, tail)) = tail.split_at_mut_checked(name.len()) else { break };
                    let [first, length @ ..] = head else { break };
                    *first = month;
                    length.copy_from_slice(&(name.len() as u32).to_le_bytes());
                    bytes.copy_from_slice(name.as_bytes());
                    rest = tail;
                }
            }
        }
        Some(len)
    }

    /// The entries in order, each as its month byte and name bytes; an entry cut short ends
    /// the walk with the [`Layout`](MonthNamesFault::Layout) fault.
    fn entries(&self) -> EntryIter<'a> {
        match self.source {
            Source::Packed(bytes) => EntryIter::Packed(bytes),
            Source::Entries(entries) => EntryIter::Entries(entries.iter()),
        }
    }

    /// Every rule, in the order an entry meets them; then each month's entry.
    ///
    /// The duplicate rule compares each name with every earlier one under folding. A key of
    /// each name's folded spelling, kept for the first [`KEYED`] entries, settles most pairs
    /// without a comparison: equal spellings have equal keys, so only a pair whose keys match,
    /// or an entry past the kept ones, is compared in full.
    fn judge(&self) -> Result<(), MonthNamesFault> {
        if self.judged {
            return Ok(());
        }
        let mut seen: u16 = 0;
        let mut keys = [0u32; KEYED];
        for (index, entry) in self.entries().enumerate() {
            let (month, name) = entry?;
            if !(1..=12).contains(&month) {
                return Err(MonthNamesFault::Month);
            }
            let Ok(text) = core::str::from_utf8(name) else {
                return Err(MonthNamesFault::Utf8);
            };
            if text.is_empty() || text.len() > MAX_NAME_BYTES {
                return Err(MonthNamesFault::NameLength);
            }
            if text
                .chars()
                .any(|c| c.is_ascii_digit() || matches!(c, '/' | '-') || is_white_space(c))
            {
                return Err(MonthNamesFault::Character);
            }
            let key = folded_key(text);
            let kept = keys.get(..index).unwrap_or(&keys);
            if kept.contains(&key) || index > KEYED {
                let mut earlier = self.entries().take(index).flatten().enumerate();
                let compared = |&(at, _): &(usize, _)| keys.get(at).is_none_or(|&k| k == key);
                if earlier.any(|pair| compared(&pair) && eq_folded(pair.1.1, name)) {
                    return Err(MonthNamesFault::Duplicate);
                }
            }
            if let Some(slot) = keys.get_mut(index) {
                *slot = key;
            }
            seen |= 1u16.wrapping_shl(u32::from(month));
        }
        if seen & 0x1FFE != 0x1FFE {
            return Err(MonthNamesFault::MonthMissing);
        }
        Ok(())
    }

    /// The month of the longest declared name at `text[at..]` that is followed by `follow`,
    /// and the bytes of text it matches, or `None`. Names compare under simple case folding,
    /// one code point to one, so a match's length in `text` can differ from the name's.
    pub(crate) fn read(&self, text: &[u8], at: usize, follow: Follow) -> Option<(u32, usize)> {
        let mut best: Option<(u32, usize)> = None;
        for (month, name) in self.entries().flatten() {
            let Some(len) = match_name(text, at, name) else { continue };
            let Some(&after) = text.get(at.saturating_add(len)) else { continue };
            let eligible = match follow {
                Follow::Any => matches!(after, b'/' | b'-' | b'.'),
                Follow::Separator(separator) => after == separator,
            };
            if eligible && best.is_none_or(|(_, longest)| len > longest) {
                best = Some((u32::from(month), len));
            }
        }
        best
    }

    /// Whether `word` is exactly one declared name, under the same folding.
    pub(crate) fn names(&self, word: &[u8]) -> bool {
        self.entries().flatten().any(|(_, name)| eq_folded(name, word))
    }
}

/// The entries whose [`folded_key`] [`MonthNames::judge`] keeps, on the stack.
const KEYED: usize = 64;

/// A hash (FNV-1a) of `name`'s spelling under simple case folding: names equal under folding
/// have equal keys.
fn folded_key(name: &str) -> u32 {
    name.chars().map(fold).fold(0x811C_9DC5, |key, c| (key ^ c as u32).wrapping_mul(0x0100_0193))
}

/// What must follow a month name for the entry to be eligible.
#[derive(Clone, Copy)]
pub(crate) enum Follow {
    /// Any of the date separators `/`, `-` and `.`: the name is the date's first field, and
    /// the separator after it is the one the date uses.
    Any,
    /// This separator, the date's own.
    Separator(u8),
}

/// The bytes of `text` from `at` that `name` matches under simple case folding, or `None`.
fn match_name(text: &[u8], at: usize, name: &[u8]) -> Option<usize> {
    let name = core::str::from_utf8(name).ok()?;
    let mut end = at;
    for expected in name.chars() {
        let (found, width) = decode(text.get(end..)?)?;
        if fold(found) != fold(expected) {
            return None;
        }
        end = end.checked_add(width)?;
    }
    end.checked_sub(at)
}

/// The scalar value at the start of `bytes` and the bytes it takes, or `None` for anything
/// that isn't one whole, well-formed UTF-8 sequence.
fn decode(bytes: &[u8]) -> Option<(char, usize)> {
    let width = match *bytes.first()? {
        0x00..=0x7F => 1,
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => return None,
    };
    let c = core::str::from_utf8(bytes.get(..width)?).ok()?.chars().next()?;
    Some((c, width))
}

/// The bytes of the word at `text[at..]`: up to the next date separator, white space or the
/// end. Empty when the field starts at one of them.
pub(crate) fn word_len(text: &[u8], at: usize) -> usize {
    let mut len = 0;
    while let Some(rest) = text.get(at.saturating_add(len)..) {
        match rest.first() {
            None | Some(b'/' | b'-' | b'.') => break,
            Some(_) if white_space_len(rest) != 0 => break,
            Some(_) => len += 1,
        }
    }
    len
}

/// A vocabulary's entries, one at a time.
enum EntryIter<'a> {
    Packed(&'a [u8]),
    Entries(core::slice::Iter<'a, (u8, &'a str)>),
}

impl<'a> Iterator for EntryIter<'a> {
    type Item = Result<(u8, &'a [u8]), MonthNamesFault>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            EntryIter::Entries(entries) => {
                entries.next().map(|&(month, name)| Ok((month, name.as_bytes())))
            }
            EntryIter::Packed(rest) => {
                let (&month, tail) = rest.split_first()?;
                let entry = (|| {
                    let (length, tail) = tail.split_first_chunk::<4>()?;
                    let length = usize::try_from(u32::from_le_bytes(*length)).ok()?;
                    tail.split_at_checked(length)
                })();
                match entry {
                    Some((name, tail)) => {
                        *rest = tail;
                        Some(Ok((month, name)))
                    }
                    None => {
                        *rest = &[];
                        Some(Err(MonthNamesFault::Layout))
                    }
                }
            }
        }
    }
}
