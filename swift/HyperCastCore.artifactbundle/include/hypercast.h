// The C ABI of the hypercast core (https://github.com/SkunkWerkx/HyperCast): the 36 functions
// the shared and static libraries export, which every binding calls through, and the
// #[repr(C)] types they read and write. hypercast_version() is the library's version.
//
// Each type is the Rust core's type of the same name behind the HyperCast prefix, and its
// comments are that type's Rust documentation, so the names in them are Rust's:
// `NumFormat::CURRENCY` there is HyperCastNumFormat_CURRENCY here, `Symbol<8>`
// HyperCastSymbol_8, and `Symbol::NONE` a symbol whose length is 0.
//
// Every door reads `len` bytes of UTF-8 at `ptr`, never dereferenced when `len` is 0, so a
// null `ptr` is fine for empty text. It returns 0 with the value written to `out`, or the
// verdict's reason: 1 empty, 2 malformed, 3 out of range, with the offending byte span
// written to `fault` when `fault` is not null. The span indexes the caller's own input, and
// offset + len never exceeds `len`. -1 is a contract violation, a caller bug rather than a
// data verdict: an undefined precision, epoch or date-order code, or a format or day-period
// layout the core refuses. Only success writes `out`, and only 1, 2 or 3 writes `fault`.
//
// A null `format` means the invariant number format: `.` decimal and `,` group separators,
// the HyperCastNumFormat_ALL leniences, nothing else declared. A null `periods` means the
// English AM/PM only. Otherwise each is a size-tagged layout, 4-byte aligned and read in
// place: `size` says how many bytes the caller built (the layout's _SIZE for all of it), the
// core reads exactly those and lets a field past them declare nothing, so a layout can grow
// without breaking an older caller. A size under the layout's _MIN_SIZE or past its _SIZE is
// a contract violation. A symbol is its UTF-8 bytes followed by its length, 0 declaring
// nothing.
//
// cast_bool writes one byte, 1 or 0; cast_char a Unicode scalar value; cast_uuid sixteen
// bytes in RFC 9562 order; cast_time nanoseconds since midnight. The four typed doors at the
// end (cast_decimal_from_f64, cast_excel_serial_from_f64, cast_excel_time and
// cast_excel_duration) read a double instead of text and keep the same contract; with no
// text for a span to index, their fault span is always empty.
//
// The validation constraints come last. cast_text is the text door: it writes the span of
// valid UTF-8 input. cast_checked and cast_checked_from_f64 judge a door's value by a
// declaration, a HyperCastRawConstraints (8-byte aligned, `size` exactly
// HyperCastRawConstraints_SIZE; any other size, or a null pointer, is -1) and an `enum`
// list of `enum_len` bytes beside it, read only under HyperCastRawConstraints_HAS_ENUM
// (set, an empty list is an enum with no entries, refused; clear, a list passed anyway is a
// malformed layout): at an integer door, Decimals holding whole numbers (at any scale: 7.0
// is 7) in their native-endian 16-byte layout; at the text door, entries each a
// little-endian uint32_t byte length followed by that many UTF-8 bytes, unpadded. They add
// return code 4, a violation of a declared rule, whose span covers the value and whose
// keyword is written to the nullable `check`; at a float door the float written is judged
// too, against each bound read as the door reads it. cast_constraints_check judges a
// declaration once and returns 0 or the code of the rule it breaks, 1 to 13, never -1; its
// duplicate search compares every pair of entries, so its time grows with the square of the
// list's length (slow for a long list, never a fault). cast_declared_decimal converts a
// bound, multipleOf or integer `enum` entry written in invariant notation into a
// HyperCastDecimal, the one conversion every binding calls, and returns 0 or 8,
// cast_constraints_check's code for a value no door can hold.
//
// The four detailed exports come last: cast_detailed, cast_detailed_from_f64,
// cast_checked_detailed and cast_checked_detailed_from_f64 reach the doors above by their
// door code, with the parameter a door declares (0 where it declares none), and write a
// HyperCastRawFaultDetail, whose `detail` names what failed (a code a caller doesn't know is
// still a failure with a usable reason and span). They judge a format by 1.1's rules, so
// flag bit 8 (a policy field of 1 or 3) and any other undefined flag bit are -1 there, while
// every other export ignores flag bits 8 and up and parses with Western grouping. The
// date doors take HyperCastRawDateOptions and a month vocabulary through cast_detailed only.
// Every argument is validated before anything is written, and -1 writes nothing. Reading by
// 1.1's grouping rules needs a 1.1 core: a 1.0 core has no detailed exports, ignores the policy
// bits and reads a group separator between any two digits, so before relying on either check
// that the core has the same major version and a minor at least the one the caller needs, the
// patch ignored: `(hypercast_version() >> 16) == 1 &&
// ((hypercast_version() >> 8) & 0xFF) >= 1`.

#ifndef HYPERCAST_H
#define HYPERCAST_H

// Generated by cbindgen from rust/src: `HYPERCAST_BLESS=1 cargo test --test header` in
// rust/ rewrites it. Edit the doc comments there, never this file.

#include <stdbool.h>
#include <stdint.h>

// The fault span as it crosses the ABI — the reason travels as the return code.
typedef struct HyperCastRawFault {
  // Where the offending bytes start, as an offset into the caller's input.
  uint32_t offset;
  // How many bytes they run; 0 for a typed door, which has no text to index.
  uint32_t len;
} HyperCastRawFault;

// A caller-declared set of native digits, read beside ASCII `0`–`9`: the ten code points
// from a declared zero up (`٠`, U+0660, declares the Arabic-Indic digits; `０`, U+FF10,
// the fullwidth ones), the way ICU and Java's `DecimalFormatSymbols` describe a locale's
// digits. [`NativeDigits::NONE`] declares none, and ASCII digits are always read.
//
// Every decimal digit block Unicode defines starts at a code point whose ten successors
// share its UTF-8 prefix, which is what lets a digit be recognised from its last byte;
// [`new`](Self::new) refuses a zero for which that does not hold, and any ASCII one.
//
// ABI layout: the zero's code point as a `u32`, `0` for `NONE`.
typedef uint32_t HyperCastNativeDigits;
// No native digits declared.
#define HyperCastNativeDigits_NONE 0

// A short declared symbol, held inline so a [`NumFormat`] stays `Copy` and crosses the
// ABI as plain bytes: a separator, a sign, a percent symbol (`Symbol<8>`) or a day-period
// designator (`Symbol<16>`). Up to `min(N, 255)` bytes of UTF-8, wide enough for
// every value a real culture declares: a separator of one or two code points (`.`, U+202F, an
// LTR mark and a comma), a minus sign of up to two (U+2212; an RTL mark and a hyphen),
// `a. m.`. Empty is [`Symbol::NONE`]: nothing declared, so the core keeps its default for
// the field.
//
// The one rule is that a symbol never contains an ASCII digit — a digit is a digit before
// it is anything else, so such a symbol could never be matched, and a sign made of one
// would read `15` as a signed `5`. [`CurrencySymbol`] keeps its own stricter rule.
//
// ABI layout: the `N` bytes, then the length, `N + 1` bytes with no alignment — a
// [`NumFormat`] is read across the C ABI in place, so this is the shape it crosses in.
typedef struct HyperCastSymbol_8 {
  // The symbol's UTF-8 bytes; those past `len` are no part of it.
  uint8_t bytes[8];
  // How many of `bytes` the symbol is.
  uint8_t len;
} HyperCastSymbol_8;

// A caller-declared currency symbol, held inline so a [`NumFormat`] stays `Copy` and
// crosses the ABI as plain bytes. Up to [`CurrencySymbol::MAX_BYTES`] bytes of UTF-8 —
// wide enough for every symbol a real culture declares (`$`, `€`, `kr.`, `CHF`, `R$`,
// `руб.`). Empty is [`CurrencySymbol::NONE`]: nothing declared, nothing matched.
//
// The symbol is matched whole, at the edges of the numeric body — leading (before or
// after a sign) or trailing, with optional white space between it and the digits —
// and only while [`NumFormat::CURRENCY`] is set. It never participates in the digit scan,
// so a symbol that happens to contain a separator character (`kr.` under `.` grouping)
// is fine.
//
// White space may sit inside a symbol (CLDR's `F CFA`, written with U+202F), never at its
// edges, where the doors ignore it. Inside, `' '`, U+00A0 and U+202F stand for each other
// in the input, one character for one: `F CFA` declared with any of them reads `F CFA`
// written with any of them. Every other character, other white space included, matches
// exactly; a run of spaces never matches one, and the space is never optional.
//
// ABI layout: the 16 bytes, then the length, 17 bytes with no alignment, as a [`Symbol`].
typedef struct HyperCastCurrencySymbol {
  // The symbol's UTF-8 bytes; those past `len` are no part of it. Sixteen, as
  // [`MAX_BYTES`](Self::MAX_BYTES) says, written as a literal so the C header can spell it.
  uint8_t bytes[16];
  // How many of `bytes` the symbol is.
  uint8_t len;
} HyperCastCurrencySymbol;
// The inline capacity, in UTF-8 bytes.
#define HyperCastCurrencySymbol_MAX_BYTES 16

// Caller-declared numeric notation for the integer and real doors. The core carries no
// culture data — a binding maps its platform's culture to these fields (Svartalfheim
// leaned on `IFormatProvider` here). Everything past the separators and the flags is
// declared, never looked up, and every one of those fields defaults to declaring nothing,
// which reads exactly as the invariant culture: ASCII signs and percent, ASCII digits,
// the number separators for a currency amount.
//
// ABI layout: the format crosses the C ABI in place, so this is `#[repr(C)]` in exactly
// the field order below — `flags` and the digit zero as two `u32`s, then seven
// [`Symbol<8>`]s of nine bytes each and the [`CurrencySymbol`]'s seventeen, 88 bytes with
// 4-byte alignment; [`RawNumFormat`](crate::RawNumFormat) puts a size word ahead of it.
typedef struct HyperCastNumFormat {
  // Bitwise OR of the `GROUPING`/`PARENS`/`EXPONENT`/`RADIX_PREFIX`/`PERCENT`/`CURRENCY`/
  // `TRAILING_SIGN` flags, plus the `SEPARATOR_DETECT` policy and, in bits 8 and 9, the
  // grouping policy: no bits for `GROUPING_WESTERN`, `GROUPING_INDIAN` when declared.
  uint32_t flags;
  // Native digits read beside ASCII `0`–`9`. [`NativeDigits::NONE`] declares none.
  HyperCastNativeDigits digits;
  // The declared decimal separator (`.` invariant, `,` in eurozone-style text): one or
  // two characters, with no white space in them (a numeral admits white space only as a
  // group separator). An empty symbol matches nothing, so no fraction can be written.
  struct HyperCastSymbol_8 decimal_sep;
  // The declared digit-group separator (`,` invariant, `.` or U+00A0 elsewhere). Must
  // differ from `decimal_sep`; the doors check the decimal separator first, so a caller
  // passing equal separators gets decimal semantics — the FFI layer rejects the
  // combination outright as a contract violation.
  struct HyperCastSymbol_8 group_sep;
  // The decimal separator of a currency amount — read in place of `decimal_sep` when the
  // declared symbol is present in the text, as the BCL switches to a culture's
  // `CurrencyDecimalSeparator` when it sees the symbol, and with no white space in it
  // either. [`Symbol::NONE`] keeps `decimal_sep`.
  struct HyperCastSymbol_8 currency_decimal_sep;
  // The group separator of a currency amount, likewise. [`Symbol::NONE`] keeps `group_sep`.
  struct HyperCastSymbol_8 currency_group_sep;
  // A further spelling of the minus sign, read wherever ASCII `-` is (U+2212 in Estonian
  // and Basque). ASCII `-` is always read; [`Symbol::NONE`] declares no other.
  struct HyperCastSymbol_8 minus_sign;
  // A further spelling of the plus sign, likewise beside ASCII `+`.
  struct HyperCastSymbol_8 plus_sign;
  // A further spelling of the percent symbol, read as a trailing `%` is while
  // [`PERCENT`](Self::PERCENT) is set (`٪` in Arabic-script cultures, `％` fullwidth).
  // ASCII `%` is always read.
  struct HyperCastSymbol_8 percent;
  // The declared currency symbol, honored only while [`CURRENCY`](Self::CURRENCY) is
  // set. [`CurrencySymbol::NONE`] declares nothing.
  struct HyperCastCurrencySymbol currency;
} HyperCastNumFormat;
// Permit `group_sep` between digits. The grouping policy
// ([`GROUPING_WESTERN`](Self::GROUPING_WESTERN), the default, or
// [`GROUPING_INDIAN`](Self::GROUPING_INDIAN)) judges every group, HyperCast's own
// deterministic rule where .NET's `AllowThousands` deferred to per-culture quirks.
#define HyperCastNumFormat_GROUPING (1 << 0)
// Permit accounting parentheses as negation: `(1,234)` is -1234.
#define HyperCastNumFormat_PARENS (1 << 1)
// Permit exponent notation: `1e3`, `2.5E-3`. Integer doors reject a negative exponent —
// a decimal point is never accepted on an integer, in any disguise.
#define HyperCastNumFormat_EXPONENT (1 << 2)
// Permit the culture-insensitive radix prefixes `0x`/`&H` (hex) and `0b` (binary),
// read as the two's-complement bit pattern (`0xFF` is -1 for an i8). Integer doors only.
#define HyperCastNumFormat_RADIX_PREFIX (1 << 3)
// Permit a trailing `%` (or the declared [`percent`](Self::percent) symbol), dividing
// by 100 (`50%` is 0.5). Real doors only.
#define HyperCastNumFormat_PERCENT (1 << 4)
// Resolve the `.`/`,` roles per input from structure instead of the declared fields
// (which are ignored while this flag is set). Detection, not sniffing — only
// structurally unambiguous inputs resolve, by exactly these rules: a separator
// appearing twice or more is grouping (`1.234.567,89`); when both appear, the
// rightmost is the decimal (`1,234.5` vs `1.234,5`); a single separator with a
// non-3-digit run to its right is the decimal (`3,1415`); a single separator with
// exactly 3 digits right is the decimal only when the integer part is exactly `0`
// (`0,785` is 0.785 — `0785` would be no number at all). Everything else — `12.185`,
// `1,000` — is genuinely ambiguous between dialects and comes back `Malformed` at the
// undecidable separator, never guessed. Only the numeral counts: a declared symbol's
// separator is no separator (`Rs.1,234` is as ambiguous as `1,234`), and declared native
// digits are digits (`١,٢٣٤` too). Covers the `.`/`,` pair only; space/NBSP grouping still
// needs a declared format.
#define HyperCastNumFormat_SEPARATOR_DETECT (1 << 5)
// Permit the declared [`currency`](Self::currency) symbol at either edge of the
// numeric body — leading (`$5`, `-$5`, `$ -5`) or trailing (`5 €`, `1.234,50 kr.`),
// once, with optional white space between symbol and digits; accounting
// parentheses wrap the symbol along with the digits (`($5)`, `(1.234,50 kr.)`). With
// [`CurrencySymbol::NONE`] declared the flag matches nothing and changes nothing.
// Integer and real doors.
#define HyperCastNumFormat_CURRENCY (1 << 6)
// Permit the sign after the digits (`5-`, `5 -`, `5 €-`, `5- €`), the way a culture
// whose negative pattern is `n-` writes it and .NET's `AllowTrailingSign` reads it:
// ASCII `-`/`+` or the declared signs, once, with optional white space before it,
// and never beside a leading sign or inside accounting parentheses. Not part of
// [`ALL`](Self::ALL): a bridge sets it from the culture's own negative pattern.
#define HyperCastNumFormat_TRAILING_SIGN (1 << 7)
// The default leniences — every lenience flag there was before `TRAILING_SIGN`, which
// is declared from a culture's pattern rather than assumed ([`SEPARATOR_DETECT`](Self::SEPARATOR_DETECT)
// is a separator *policy*, not a lenience, and is deliberately not included either).
#define HyperCastNumFormat_ALL (((((HyperCastNumFormat_GROUPING | HyperCastNumFormat_PARENS) | HyperCastNumFormat_EXPONENT) | HyperCastNumFormat_RADIX_PREFIX) | HyperCastNumFormat_PERCENT) | HyperCastNumFormat_CURRENCY)
// Grouping policy `00`, Western, what a format that records no policy has: the C ABI's
// null format, a 1.0 layout, a 1.0 serialized payload, [`new`](Self::new) with no policy
// bits, [`INVARIANT`](Self::INVARIANT) and [`DETECT`](Self::DETECT). Once a numeral uses a
// group separator, its integer part is groups of exactly three digits counted leftward from
// the decimal separator (or the numeral's end), the leftmost group one to three and not
// starting with a zero (`0,125` and `007,000` are `Malformed`). An ungrouped numeral stays
// valid (`0125`); grouping after the decimal separator or in an exponent never is. Digits
// are counted as digits, native ones included, never bytes, and the rule holds for the
// currency group separator and each space-like separator alike. A policy only judges
// groups: with [`GROUPING`](Self::GROUPING) off, a group separator stays forbidden.
#define HyperCastNumFormat_GROUPING_WESTERN 0
// Grouping policy `10`, Indian (lakh and crore), only when declared: as
// [`GROUPING_WESTERN`](Self::GROUPING_WESTERN), but the group next to the decimal
// separator has three digits, every group to its left two, and the leftmost one or two
// (`12,34,567`).
#define HyperCastNumFormat_GROUPING_INDIAN (2 << 8)
// The policy field, bits 8 and 9, within `flags`. Only `00` and `10` are policies. Value 256
// (bit 8) is retired permanently and never reused, so an old `351` can never gain a meaning:
// [`check`](Self::check) and [`validate`](Self::validate) refuse `01` and `11`, and a future
// policy, such as four-digit grouping, takes a new bit from bit 10 up. Set a policy with
// [`with_grouping`](Self::with_grouping), which can't name a bad one. [`new`](Self::new),
// [`with_separators`](Self::with_separators) and the public fields can, because 1.0 gave them
// a plain `u32`: a plain door reads such a format Western, and
// [`check`](Self::check) and [`validate`](Self::validate) are what refuse it.
#define HyperCastNumFormat_GROUPING_POLICY (3 << 8)

// [`NumFormat`] as it crosses the ABI — the size-tagged layout of 0.9.0: a size word, then
// the format itself, byte for byte, 92 bytes with 4-byte alignment. The core reads the
// caller's struct in place, so a cast pays for a check of it and no copy. Every symbol is
// its bytes followed by its length (nine bytes for a separator, a sign or the percent
// symbol, seventeen for the currency symbol); a length of `0` declares nothing and the
// core keeps its default for that field, except the two number separators, which a caller
// always declares. Where an export takes a pointer to one, null means
// [`NumFormat::INVARIANT`].
//
// | Offset | Field | Bytes |
// | --- | --- | --- |
// | 0 | `size`, a `u32` | 4 |
// | 4 | `flags`, a `u32` | 4 |
// | 8 | the native digits' zero code point, a `u32`; `0` for none | 4 |
// | 12 | `decimal_sep`: 8 bytes of UTF-8, then its length | 9 |
// | 21 | `group_sep` | 9 |
// | 30 | `currency_decimal_sep`; length `0` keeps `decimal_sep` | 9 |
// | 39 | `currency_group_sep`; length `0` keeps `group_sep` | 9 |
// | 48 | `minus_sign`; length `0` declares none beside ASCII `-` | 9 |
// | 57 | `plus_sign`; likewise beside `+` | 9 |
// | 66 | `percent`; likewise beside `%` | 9 |
// | 75 | `currency`: 16 bytes of UTF-8, then its length; `0` declares none | 17 |
//
// `size` is first so the layout can grow without another coordinated change: a caller
// states how many bytes it built, the core reads exactly those and defaults the rest, and a
// size the core does not know (under 30, or past this layout) is a contract violation. The
// 0.8.0 layout was 32 bytes with the separators as code points at its head; it has no size
// field and is not read.
typedef struct HyperCastRawNumFormat {
  // `size_of::<RawNumFormat>()` as the caller built it: [`RawNumFormat::SIZE`].
  uint32_t size;
  // The format, in place.
  struct HyperCastNumFormat format;
} HyperCastRawNumFormat;
// The layout's size in bytes, which `size` carries: a literal, so the C header carries
// it too, held to `size_of` beside the layout pins.
#define HyperCastRawNumFormat_SIZE 92
// The smallest `size` the core reads: through the group separator's length byte, since
// a format with no number separators is no format.
#define HyperCastRawNumFormat_MIN_SIZE 30

// An exact decimal — a sign, a 96-bit unsigned magnitude, and a base-10 scale — the
// shape .NET's `decimal` stores natively and Java's `BigDecimal`, Python's `Decimal` and
// Ruby's `BigDecimal` build from directly. The value is `(-1)^negative × (hi·2⁶⁴ + lo) ×
// 10⁻ˢᶜᵃˡᵉ`. Casts produce canonical values: exact trailing fraction zeros are trimmed,
// the scale is minimal, and zero has scale 0 and no negative sign. Casts never round:
// text carrying more nonzero precision than 96 bits and 28 places can hold is
// `OutOfRange`. The public fields also permit directly constructed, noncanonical
// values; display preserves their declared scale and sign without clamping.
// ABI layout: `lo` at offset 0, `hi` at 8, `scale` at 12, `negative` at 13,
// 16 bytes with 8-byte alignment.
typedef struct HyperCastDecimal {
  // The low 64 bits of the magnitude.
  uint64_t lo;
  // The high 32 bits of the magnitude; the magnitude never exceeds 2⁹⁶ − 1.
  uint32_t hi;
  // Base-10 scale: the number of places the magnitude is shifted right. Casts
  // produce `0..=28`, with no trailing magnitude zeros at a positive scale.
  // This public ABI representation can also be constructed directly; display
  // handles every scale byte without clamping or changing its value.
  uint8_t scale;
  // True for a negative value. Casts never produce a negative zero.
  bool negative;
} HyperCastDecimal;

// A point in time, independent of any time zone or calendar — protobuf's
// `google.protobuf.Timestamp` layout exactly, so every binding folds it into its platform
// instant type without precision games.
//
// Range: `0001-01-01T00:00:00Z` (seconds -62_135_596_800) through
// `9999-12-31T23:59:59.999999999Z` (seconds 253_402_300_799). `nanos` counts forward from
// `seconds` and is always in `0..=999_999_999`, even for instants before the epoch.
typedef struct HyperCastTimestamp {
  // Seconds since the Unix epoch `1970-01-01T00:00:00Z`.
  int64_t seconds;
  // Non-negative sub-second nanoseconds, `0..=999_999_999`.
  int32_t nanos;
} HyperCastTimestamp;

// A calendar date with no time or zone — protobuf's `google.type.Date` field layout.
// Years `1..=9999`, proleptic Gregorian.
typedef struct HyperCastDate {
  // Year, `1..=9999`.
  uint16_t year;
  // Month, `1..=12`.
  uint8_t month;
  // Day of month, `1..=days_in_month(year, month)`.
  uint8_t day;
} HyperCastDate;

// A short declared symbol, held inline so a [`NumFormat`] stays `Copy` and crosses the
// ABI as plain bytes: a separator, a sign, a percent symbol (`Symbol<8>`) or a day-period
// designator (`Symbol<16>`). Up to `min(N, 255)` bytes of UTF-8, wide enough for
// every value a real culture declares: a separator of one or two code points (`.`, U+202F, an
// LTR mark and a comma), a minus sign of up to two (U+2212; an RTL mark and a hyphen),
// `a. m.`. Empty is [`Symbol::NONE`]: nothing declared, so the core keeps its default for
// the field.
//
// The one rule is that a symbol never contains an ASCII digit — a digit is a digit before
// it is anything else, so such a symbol could never be matched, and a sign made of one
// would read `15` as a signed `5`. [`CurrencySymbol`] keeps its own stricter rule.
//
// ABI layout: the `N` bytes, then the length, `N + 1` bytes with no alignment — a
// [`NumFormat`] is read across the C ABI in place, so this is the shape it crosses in.
typedef struct HyperCastSymbol_16 {
  // The symbol's UTF-8 bytes; those past `len` are no part of it.
  uint8_t bytes[16];
  // How many of `bytes` the symbol is.
  uint8_t len;
} HyperCastSymbol_16;

// Caller-declared day-period designators for [`cast_datetime_with`]: the words a culture
// writes for the two halves of a 12-hour clock (`오전`/`오후`, `vorm.`/`nachm.`, `a. m.`/
// `p. m.`), read beside the English `AM`/`PM`, which are always read. Declared, never
// looked up, like every other piece of culture here; a binding's bridge fills them from
// its platform's data. Up to 16 UTF-8 bytes each, with no ASCII digit and no white
// space at either end (the grammar around a designator reads that); inside stays allowed.
//
// ABI layout: the two designators in place, 34 bytes with no alignment, each its 16 bytes
// then its length; [`RawDayPeriods`](crate::RawDayPeriods) puts a size word ahead of them.
typedef struct HyperCastDayPeriods {
  // The designator of the first half of the day; [`Symbol::NONE`] declares none beside `AM`.
  struct HyperCastSymbol_16 am;
  // The designator of the second half; [`Symbol::NONE`] declares none beside `PM`.
  struct HyperCastSymbol_16 pm;
} HyperCastDayPeriods;

// [`DayPeriods`] as they cross the ABI beside a declared date order: a size word, then
// the designators in place — the AM one's 16 bytes of UTF-8 and its length, then the PM
// one's — 40 bytes with 4-byte alignment, the last two padding. A length of `0` declares
// nothing; where an export takes a pointer to one, null means [`DayPeriods::ENGLISH`] —
// the English `AM`/`PM`, which are read whatever is declared.
//
// | Offset | Field | Bytes |
// | --- | --- | --- |
// | 0 | `size`, a `u32` | 4 |
// | 4 | `am`: 16 bytes of UTF-8, then its length | 17 |
// | 21 | `pm` | 17 |
// | 38 | padding | 2 |
typedef struct HyperCastRawDayPeriods {
  // `size_of::<RawDayPeriods>()` as the caller built it: [`RawDayPeriods::SIZE`].
  uint32_t size;
  // The designators, in place.
  struct HyperCastDayPeriods periods;
} HyperCastRawDayPeriods;
// The layout's size in bytes, which `size` carries: a literal, so the C header carries
// it too, held to `size_of` beside the layout pins.
#define HyperCastRawDayPeriods_SIZE 40
// The smallest `size` the core reads: the size word alone, which declares nothing.
#define HyperCastRawDayPeriods_MIN_SIZE 4

// A civil (wall-clock) date and time with **no zone** — exactly what zone-less text like
// `1/7/2026 3:04 PM` actually names. Deliberately not a [`Timestamp`]: without a zone
// there is no instant, and inventing one (assuming UTC, say) would be a silent value
// error of up to ±14 hours. Fusing a zone is the caller's job, per the module doctrine.
// ABI layout: the [`Date`] fields at offset 0, `nanos_of_day` at offset 8 (after C
// struct padding).
typedef struct HyperCastCivilDateTime {
  // The calendar date.
  struct HyperCastDate date;
  // Nanoseconds since that date's midnight, `0..86_400_000_000_000`.
  uint64_t nanos_of_day;
} HyperCastCivilDateTime;

// A signed span of time — protobuf's `google.protobuf.Duration` layout exactly.
// `seconds` is bounded to ±315_576_000_000 (±10,000 years); `nanos` carries the same sign
// as `seconds` (unlike [`Timestamp`], whose nanos always count forward).
typedef struct HyperCastDuration {
  // Whole seconds, `-315_576_000_000..=315_576_000_000`.
  int64_t seconds;
  // Sub-second nanoseconds, `-999_999_999..=999_999_999`, same sign as `seconds`.
  int32_t nanos;
} HyperCastDuration;

// A declaration of validation constraints as it crosses the ABI, bound to a door by the
// call that takes it: a size word, the flags that say which fields are read, then the
// five decimals, 104 bytes with 8-byte alignment. Pointer-free, so the layout is the same
// on 32-bit and 64-bit targets; an `enum` list travels beside it. Built from a
// [`Constraints`](crate::Constraints) by [`raw`](crate::Constraints::raw), and judged by
// [`check_constraints`](crate::check_constraints).
//
// | Offset | Field | Bytes |
// | --- | --- | --- |
// | 0 | `size`, a `u32` | 4 |
// | 4 | `flags`, a `u32` | 4 |
// | 8 | `rounding`: `0` strict, `1` midpoint, `2` floor, `3` ceiling | 4 |
// | 12 | `min_length`, read under [`HAS_MIN_LENGTH`](Self::HAS_MIN_LENGTH) | 4 |
// | 16 | `max_length`, read under [`HAS_MAX_LENGTH`](Self::HAS_MAX_LENGTH) | 4 |
// | 20 | `_reserved`, always `0` | 4 |
// | 24 | `lower`: `minimum`, read under [`HAS_MINIMUM`](Self::HAS_MINIMUM) | 16 |
// | 40 | `upper`: `maximum`, read under [`HAS_MAXIMUM`](Self::HAS_MAXIMUM) | 16 |
// | 56 | `multiple_of`, read under [`HAS_MULTIPLE_OF`](Self::HAS_MULTIPLE_OF) | 16 |
// | 72 | `exclusive_lower`: `exclusiveMinimum`, read under [`HAS_EXCLUSIVE_MINIMUM`](Self::HAS_EXCLUSIVE_MINIMUM) | 16 |
// | 88 | `exclusive_upper`: `exclusiveMaximum`, read under [`HAS_EXCLUSIVE_MAXIMUM`](Self::HAS_EXCLUSIVE_MAXIMUM) | 16 |
//
// Each keyword has a field of its own, so all four bounds may be declared together, as
// OpenAPI allows.
//
// A `size` other than [`SIZE`](Self::SIZE), an unknown flag bit, an unknown rounding code,
// a nonzero `_reserved`, or (read from a caller's memory across the C ABI) a decimal's sign
// byte other than `0` or `1` is a malformed layout. The `enum` list is read only under
// [`HAS_ENUM`](Self::HAS_ENUM).
typedef struct HyperCastRawConstraints {
  // `size_of::<RawConstraints>()` as the caller built it: [`RawConstraints::SIZE`].
  uint32_t size;
  // Which constraints are declared: the `HAS_*` bits, [`TRIM`](Self::TRIM) and
  // [`CASE_SENSITIVE`](Self::CASE_SENSITIVE).
  uint32_t flags;
  // How a value that is not a multiple of `multiple_of` is treated: `0` strict, `1`
  // midpoint (ties to even), `2` floor, `3` ceiling.
  uint32_t rounding;
  // The fewest code points the text may hold.
  uint32_t min_length;
  // The most code points the text may hold.
  uint32_t max_length;
  // Always `0`.
  uint32_t _reserved;
  // `minimum`: the value may equal it.
  struct HyperCastDecimal lower;
  // `maximum`: the value may equal it.
  struct HyperCastDecimal upper;
  // The step every value must be a multiple of.
  struct HyperCastDecimal multiple_of;
  // `exclusiveMinimum`: the value must exceed it.
  struct HyperCastDecimal exclusive_lower;
  // `exclusiveMaximum`: the value must be below it.
  struct HyperCastDecimal exclusive_upper;
} HyperCastRawConstraints;
// The layout's size in bytes, which `size` carries.
#define HyperCastRawConstraints_SIZE 104
// `lower`, the `minimum`, is read.
#define HyperCastRawConstraints_HAS_MINIMUM 1
// `exclusive_lower`, the `exclusiveMinimum`, is read.
#define HyperCastRawConstraints_HAS_EXCLUSIVE_MINIMUM 2
// `upper`, the `maximum`, is read.
#define HyperCastRawConstraints_HAS_MAXIMUM 4
// `exclusive_upper`, the `exclusiveMaximum`, is read.
#define HyperCastRawConstraints_HAS_EXCLUSIVE_MAXIMUM 8
// `multiple_of` and `rounding` are read.
#define HyperCastRawConstraints_HAS_MULTIPLE_OF 16
// `min_length` is read.
#define HyperCastRawConstraints_HAS_MIN_LENGTH 32
// `max_length` is read.
#define HyperCastRawConstraints_HAS_MAX_LENGTH 64
// Text is trimmed of Unicode `White_Space` before it is judged.
#define HyperCastRawConstraints_TRIM 128
// An `enum` list is declared and read: the list beside the layout, which may be empty
// (a declaration [`check_constraints`](crate::check_constraints) refuses). Clear, no
// list is read, and one passed anyway is a malformed layout.
#define HyperCastRawConstraints_HAS_ENUM 256
// A text `enum` matches exactly, byte for byte, instead of under simple case folding,
// for a vocabulary whose case carries meaning (`mm` and `Mm`). The text door only.
#define HyperCastRawConstraints_CASE_SENSITIVE 512

// What a checked door reports beside its value, 12 bytes with 4-byte alignment.
typedef struct HyperCastRawCheck {
  // On a violation, the keyword broken (`1` minimum … `8` enum); `0` otherwise.
  uint32_t keyword;
  // On success, `1` when rounding changed the value, else `0`.
  uint32_t adjusted;
  // On a success at the text door that matched an `enum` entry, that entry's index;
  // `u32::MAX` otherwise.
  uint32_t enum_index;
} HyperCastRawCheck;
// The layout's size in bytes.
#define HyperCastRawCheck_SIZE 12

// A span of the caller's own input, 8 bytes with 4-byte alignment: what the text door
// writes in place of a copy.
typedef struct HyperCastRawSpan {
  // The byte offset where the span starts.
  uint32_t offset;
  // The span's length in bytes.
  uint32_t len;
} HyperCastRawSpan;
// The layout's size in bytes.
#define HyperCastRawSpan_SIZE 8

// The date options as they cross the ABI: a size word, the option bits
// ([`DateOptions::COMPACT_DATE`](crate::DateOptions::COMPACT_DATE) and the others), and the
// two-digit-year pivot, 12 bytes with 4-byte alignment.
//
// | Offset | Field | Bytes |
// | --- | --- | --- |
// | 0 | `size`, a `u32` | 4 |
// | 4 | `flags`, the option bits | 4 |
// | 8 | `pivot`, `0` for none, else 100–9999 | 4 |
//
// `size` is [`MIN_SIZE`](Self::MIN_SIZE) (the size and flags words; the pivot is then `0`,
// none) or [`SIZE`](Self::SIZE); any other size, a flag bit this core doesn't define, or a
// pivot other than `0` or 100–9999 is a contract violation. A null pointer is no options. The
// core never writes it.
typedef struct HyperCastRawDateOptions {
  // The bytes the caller built: [`RawDateOptions::SIZE`] or [`RawDateOptions::MIN_SIZE`].
  uint32_t size;
  // The option bits.
  uint32_t flags;
  // The last year of the window a two-digit year is read in; `0` for none.
  uint32_t pivot;
} HyperCastRawDateOptions;
// The layout's size in bytes, which `size` carries.
#define HyperCastRawDateOptions_SIZE 12
// The smallest `size` the core reads: the size and flags words, with no pivot.
#define HyperCastRawDateOptions_MIN_SIZE 8

// A detailed export's fault: the span and the [`Detail`](crate::Detail) code, behind a size
// word the caller writes, 16 bytes with 4-byte alignment.
//
// | Offset | Field | Bytes |
// | --- | --- | --- |
// | 0 | `size`, a `u32`, written by the caller | 4 |
// | 4 | `offset`, where the span starts in the caller's input | 4 |
// | 8 | `len`, the span's length in bytes | 4 |
// | 12 | `detail`, a [`Detail`](crate::Detail) code, or `0` for a checked violation | 4 |
//
// `size` says how many bytes the caller provides: at least [`SIZE`](Self::SIZE) and a
// multiple of 4, or the export refuses the call (−1) and writes nothing. A larger size, from
// a caller built against a later layout, is accepted: this core writes bytes 4 to 16 and
// never a byte past them, so such a caller sets any field beyond those to its neutral value
// itself. The core never writes `size`, and writes the rest only on codes 1 to 3 (and on 4
// at the checked exports); a null pointer is allowed and nothing is written.
typedef struct HyperCastRawFaultDetail {
  // The bytes the caller provides: [`RawFaultDetail::SIZE`] or more, a multiple of 4.
  uint32_t size;
  // Where the offending bytes start, as an offset into the caller's input.
  uint32_t offset;
  // How many bytes they run; 0 for a typed door, which has no text to index.
  uint32_t len;
  // What failed: a [`Detail`](crate::Detail) code, 1 or more; `0` on a checked violation.
  // A code this binding doesn't know is still a failure with a usable reason and span.
  uint32_t detail;
} HyperCastRawFaultDetail;
// The layout's size in bytes, the least `size` the core accepts.
#define HyperCastRawFaultDetail_SIZE 16

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

// This library's version, packed `major << 16 | minor << 8 | patch` from the crate's own
// manifest — so a host can prove the library it loaded is the one its binding was built
// against before making the first cast, and can name the mismatch when it isn't. Takes
// nothing, touches nothing: the cheapest possible "did the native library resolve" probe.
uint32_t hypercast_version(void);

// Casts boolean text at `ptr`/`len` into `out`: 1 for true, 0 for false.
int32_t cast_bool(const uint8_t *ptr, uintptr_t len, uint8_t *out, struct HyperCastRawFault *fault);

// Casts char text at `ptr`/`len` into `out` as a Unicode scalar value (never a surrogate,
// never past U+10FFFF).
int32_t cast_char(const uint8_t *ptr, uintptr_t len, uint32_t *out, struct HyperCastRawFault *fault);

// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
// outside `int8_t` is out of range.
int32_t cast_i8(const uint8_t *ptr, uintptr_t len, const struct HyperCastRawNumFormat *format, int8_t *out, struct HyperCastRawFault *fault);

// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
// outside `int16_t` is out of range.
int32_t cast_i16(const uint8_t *ptr, uintptr_t len, const struct HyperCastRawNumFormat *format, int16_t *out, struct HyperCastRawFault *fault);

// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
// outside `int32_t` is out of range.
int32_t cast_i32(const uint8_t *ptr, uintptr_t len, const struct HyperCastRawNumFormat *format, int32_t *out, struct HyperCastRawFault *fault);

// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
// outside `int64_t` is out of range.
int32_t cast_i64(const uint8_t *ptr, uintptr_t len, const struct HyperCastRawNumFormat *format, int64_t *out, struct HyperCastRawFault *fault);

// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
// outside `uint8_t` is out of range.
int32_t cast_u8(const uint8_t *ptr, uintptr_t len, const struct HyperCastRawNumFormat *format, uint8_t *out, struct HyperCastRawFault *fault);

// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
// outside `uint16_t` is out of range.
int32_t cast_u16(const uint8_t *ptr, uintptr_t len, const struct HyperCastRawNumFormat *format, uint16_t *out, struct HyperCastRawFault *fault);

// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
// outside `uint32_t` is out of range.
int32_t cast_u32(const uint8_t *ptr, uintptr_t len, const struct HyperCastRawNumFormat *format, uint32_t *out, struct HyperCastRawFault *fault);

// Casts integer text at `ptr`/`len` under the declared `format` into `out`; a value
// outside `uint64_t` is out of range.
int32_t cast_u64(const uint8_t *ptr, uintptr_t len, const struct HyperCastRawNumFormat *format, uint64_t *out, struct HyperCastRawFault *fault);

// Casts real text at `ptr`/`len` under the declared `format` into `out`; a magnitude
// beyond `float`'s finite range is out of range.
// Never writes a negative zero: `-0`, `(0)` and an underflow like `-1e-400` are `0.0`.
int32_t cast_f32(const uint8_t *ptr, uintptr_t len, const struct HyperCastRawNumFormat *format, float *out, struct HyperCastRawFault *fault);

// Casts real text at `ptr`/`len` under the declared `format` into `out`; a magnitude
// beyond `double`'s finite range is out of range.
// Never writes a negative zero: `-0`, `(0)` and an underflow like `-1e-400` are `0.0`.
int32_t cast_f64(const uint8_t *ptr, uintptr_t len, const struct HyperCastRawNumFormat *format, double *out, struct HyperCastRawFault *fault);

// Casts decimal text at `ptr`/`len` under the declared `format` into `out`, exact and
// canonical; a magnitude past 96 bits, or more fractional precision than 28 places hold
// once exact trailing zeros are trimmed, is out of range. Never rounds.
int32_t cast_decimal(const uint8_t *ptr, uintptr_t len, const struct HyperCastRawNumFormat *format, struct HyperCastDecimal *out, struct HyperCastRawFault *fault);

// Casts UUID text at `ptr`/`len` into the 16 bytes at `out`, RFC 9562 order.
int32_t cast_uuid(const uint8_t *ptr, uintptr_t len, uint8_t *out, struct HyperCastRawFault *fault);

// Casts an RFC 3339 instant at `ptr`/`len` into `out`.
int32_t cast_timestamp(const uint8_t *ptr, uintptr_t len, struct HyperCastTimestamp *out, struct HyperCastRawFault *fault);

// Casts an integer Unix-epoch value at `ptr`/`len` under the declared `precision`
// (1 seconds, 2 milliseconds, 3 microseconds, 4 nanoseconds — anything else is a contract
// violation) into `out`.
int32_t cast_unix(const uint8_t *ptr, uintptr_t len, uint32_t precision, struct HyperCastTimestamp *out, struct HyperCastRawFault *fault);

// Casts an Excel date serial at `ptr`/`len` under the declared `epoch` (1 the 1900 system,
// 2 the 1904 system — anything else is a contract violation) into `out`.
int32_t cast_excel_serial(const uint8_t *ptr, uintptr_t len, uint32_t epoch, struct HyperCastTimestamp *out, struct HyperCastRawFault *fault);

// Casts a separated calendar date at `ptr`/`len` under the caller-declared field `order`
// (1 year-month-day, 2 month-day-year, 3 day-month-year — anything else is a contract
// violation) into `out`.
int32_t cast_date_ordered(const uint8_t *ptr, uintptr_t len, uint32_t order, struct HyperCastDate *out, struct HyperCastRawFault *fault);

// Casts a zone-less civil date-time at `ptr`/`len` under the caller-declared field
// `order` (1 year-month-day, 2 month-day-year, 3 day-month-year — anything else is a
// contract violation) and the declared day-period designators at `periods` (null ⇒ the
// English `AM`/`PM` only; an invalid designator is a contract violation) into `out`.
int32_t cast_datetime(const uint8_t *ptr, uintptr_t len, uint32_t order, const struct HyperCastRawDayPeriods *periods, struct HyperCastCivilDateTime *out, struct HyperCastRawFault *fault);

// Casts a strict `yyyy-MM-dd` date at `ptr`/`len` into `out`.
int32_t cast_date(const uint8_t *ptr, uintptr_t len, struct HyperCastDate *out, struct HyperCastRawFault *fault);

// Casts an ISO 24-hour time-of-day at `ptr`/`len` into `out` as nanoseconds since
// midnight.
int32_t cast_time(const uint8_t *ptr, uintptr_t len, uint64_t *out, struct HyperCastRawFault *fault);

// Casts a duration at `ptr`/`len` (ISO 8601, invariant colon form, or protobuf JSON
// seconds) into `out`.
int32_t cast_duration(const uint8_t *ptr, uintptr_t len, struct HyperCastDuration *out, struct HyperCastRawFault *fault);

// Reads the number `value` into `out` as the shortest decimal that names it, canonical as
// `cast_decimal`'s. NaN is malformed; an infinity, a magnitude past 2^96 - 1 or more than
// 28 places is out of range.
int32_t cast_decimal_from_f64(double value, struct HyperCastDecimal *out, struct HyperCastRawFault *fault);

// Reads the Excel serial `value` under the declared `epoch` (1 the 1900 system, 2 the 1904
// system — anything else is a contract violation) into `out` as the zone-less wall clock
// it names, its fraction snapped to the fewest fractional-second digits that store as
// this same double.
int32_t cast_excel_serial_from_f64(double value, uint32_t epoch, struct HyperCastCivilDateTime *out, struct HyperCastRawFault *fault);

// Reads the time of day of the Excel serial `value` into `out` as nanoseconds since
// midnight, snapped as `cast_excel_serial_from_f64` snaps. A negative, NaN or infinite
// value is malformed; a serial past either date system's last day is out of range.
int32_t cast_excel_time(double value, uint64_t *out, struct HyperCastRawFault *fault);

// Reads `value` days into `out` as a duration, snapped as `cast_excel_serial_from_f64`
// snaps a time. NaN or an infinity is malformed; past ±10,000 years is out of range.
int32_t cast_excel_duration(double value, struct HyperCastDuration *out, struct HyperCastRawFault *fault);

// Casts the text at `ptr`/`len` through door `door` (with `param`, the precision, order or
// date system a door declares; the door numbering is `Door`'s, 4 I32, 11 F64, 18 Text, 19
// Decimal) under the declared `format` (null for the invariant one; ignored by Text), then
// judges the value by `constraints` and the `enum` list at `enum_ptr`/`enum_len` (integer
// entries native-endian `Decimal`s, 16 bytes each; text entries a little-endian `u32` length
// followed by UTF-8 bytes). Returns 0 with the value written to `out`, the type the door
// writes (a `RawSpan` of the input for Text, which with an `enum` is the matched input, or 3
// when that span would end past `UINT32_MAX` bytes); 1, 2 or 3, the door's own fault, the
// input not the type (`out` untouched); or 4, a value of the type that breaks a declared
// rule, written to `out` exactly as cast (never rounded: `multipleOf`'s rounding is part of
// the judgement; for Text, the span of the judged text), with `fault` spanning it. A
// rounding past what the type holds is 4, `multipleOf`. At a float door the float written is
// judged too, against each bound read as the door reads it (to the nearest float, ties to
// even), so a value inside a bound whose float lands on or past it is a violation of that
// bound.
// Codes 1 to 4 write `fault`; 0 and 4 write `out`, and `check` when it is not null
// (`keyword` on 4, `adjusted` and `enum_index` on 0). -1 is a contract violation: a null
// `constraints`, a `size` other than `HyperCastRawConstraints_SIZE`, a door that is not
// numeric or Text, a format the core refuses, or a declaration `cast_constraints_check`
// refuses. With duplicate `enum` entries, the first matching entry wins. `out` need not be
// aligned.
int32_t cast_checked(uint32_t door, uint32_t param, const struct HyperCastRawNumFormat *format, const struct HyperCastRawConstraints *constraints, const uint8_t *enum_ptr, uintptr_t enum_len, const uint8_t *ptr, uintptr_t len, void *out, struct HyperCastRawFault *fault, struct HyperCastRawCheck *check);

// Judges the number `value`, which a source already decoded (a JSON parser's double, a
// spreadsheet's numeric cell), at the numeric door `door` by `constraints` and the integer
// `enum` list at `enum_ptr`/`enum_len`, on its sign and shortest round-trip decimal: the
// same judgement `cast_checked` makes of that text. `-0.0` is zero, written at a float door
// as `0.0`. NaN is malformed; an infinity is out of range at a float or decimal door and
// malformed at an integer door, as the typed doors judge it. With no text to index, every
// fault and violation has an empty span. The codes and what each writes are
// `cast_checked`'s, the delivered float's judgement included (at the `F32` door, the
// double's digits are judged, then the narrowed float), and a door that is not numeric is
// a contract violation.
int32_t cast_checked_from_f64(uint32_t door, double value, const struct HyperCastRawConstraints *constraints, const uint8_t *enum_ptr, uintptr_t enum_len, void *out, struct HyperCastRawFault *fault, struct HyperCastRawCheck *check);

// Judges a declaration, `constraints` and the `enum` list at `enum_ptr`/`enum_len` (read
// only under `HAS_ENUM`; numeric entries native-endian `Decimal`s, 16 bytes each; text
// entries a little-endian `u32` length followed by UTF-8 bytes), against door `door` with
// `param`, once, when a binding builds it. Returns 0 when every rule holds, otherwise the
// first rule it breaks, in code order. Every valid OpenAPI 3.2 declaration is accepted,
// unsatisfiable ones included (crossed bounds, duplicate or empty `enum`s), and a keyword
// for another type is ignored, as JSON Schema ignores it. The codes: 1 a door that takes no
// constraint (or a door code that names no door), `TRIM` or `CASE_SENSITIVE` at a numeric
// door, or a rounding with `multipleOf` at the text door; 5 multipleOf not positive; 6 a
// rounding without multipleOf; 8 a bound, step or numeric entry at a scale past 28, a text
// entry past `u32::MAX` bytes, or a rounding at an integer door with a fractional
// multipleOf; 9 an enum entry that is not UTF-8; 11 a malformed layout. Codes 2, 3, 4, 7,
// 10, 12 and 13 are retired and never returned. A malformed layout is any of: a null
// `constraints`; a `size` other than `HyperCastRawConstraints_SIZE` or a sign byte other
// than 0 or 1 in any of its five decimals (these two judged before everything else, since
// the layout cannot be read past them); an unknown flag bit, rounding code or nonzero
// `_reserved`; a sign byte other than 0 or 1 in a numeric entry; a packed list cut short (a
// text entry's length word or bytes, or a 16-byte numeric entry); or a non-empty list
// without `HAS_ENUM`. Never -1.
int32_t cast_constraints_check(uint32_t door, uint32_t param, const struct HyperCastRawConstraints *constraints, const uint8_t *enum_ptr, uintptr_t enum_len);

// Casts the text at `ptr`/`len` into `out` as the span of the whole input, once it is
// valid UTF-8: the text door, untrimmed. Invalid UTF-8 is malformed, spanning the first
// invalid sequence; empty input is empty; input past `UINT32_MAX` bytes, which a span
// cannot carry, is out of range.
int32_t cast_text(const uint8_t *ptr, uintptr_t len, struct HyperCastRawSpan *out, struct HyperCastRawFault *fault);

// Converts a bound, `multipleOf` or integer `enum` entry written in invariant notation (an
// optional sign, digits, an optional `.` and digits, an optional exponent; ASCII only, no
// white space of any kind) at `ptr`/`len` into a `Decimal` at `out`, exactly: trailing zeros
// dropped, zero unsigned. Returns 0 with `out` written, or 8 (a value no door can hold: a magnitude
// past 96 bits or a scale past 28, or text not in invariant notation, invalid UTF-8
// included), leaving `out` untouched; 8 is `cast_constraints_check`'s code for the same
// refusal. -1 is a contract violation: a null `out`, or a null `ptr` with a non-zero `len`.
// Every binding converts a declaration's strings through this one export, so none restates
// the conversion.
int32_t cast_declared_decimal(const uint8_t *ptr, uintptr_t len, struct HyperCastDecimal *out);

// Casts the text at `ptr`/`len` through door `door`, reporting a failure's detail. `door` is
// `Door`'s code (1 bool, 2 to 9 the integers `int8_t` … `uint64_t`, 10 float, 11 double, 12
// uuid, 13 timestamp, 14 unix, 15 date, 16 time, 17 duration, 18 text, 19 decimal, 20
// date_ordered, 21 datetime, 22 excel_serial, 23 char) and `param` what the door declares,
// numbered as its own export numbers it (14's precision, 20's and 21's order, 22's epoch),
// and `0` at every other door. `out` receives exactly what that door's own export writes
// (`cast_text`'s `RawSpan` at 18), and need not be aligned.
//
// Each declaration is read only by the doors it is for, and a pointer a door ignores is
// never read: `format` by the numeric doors 2 to 11 and 19 (null for the invariant format,
// Western grouping), judged by 1.1's rules, so bit 8 and any other `flags` bit 1.1 doesn't
// define are refused; `periods` by 21 (null for English); `date_options` by 13, 15,
// 20 and 21 (null for none); and the month vocabulary at `vocab_ptr`/`vocab_len` by 20 and 21
// only, packed as each entry's month byte (1 to 12), a little-endian `uint32_t` byte length,
// then that many UTF-8 bytes, a length of 0 meaning none whatever the pointer. The
// vocabulary is independent of `date_options`: with null options it opens month names and
// nothing else. It is judged in full on every call.
//
// Returns 0 with the value written to `out`; 1, 2 or 3, the door's own fault, with `offset`,
// `len` and `detail` written to a non-null `fault`. -1 is a contract violation and writes
// nothing: a `fault` whose `size` is under 16 or not a multiple of 4, an unknown door, a
// `param` the door doesn't take, or a declaration the door reads and the core refuses
// (a `date_options` size other than 8 or 12, an undefined option bit, a pivot other than 0
// or 100 to 9999, a null `vocab_ptr` with a nonzero `vocab_len`, a vocabulary its rules
// refuse).
int32_t cast_detailed(uint32_t door, uint32_t param, const struct HyperCastRawNumFormat *format, const struct HyperCastRawDayPeriods *periods, const struct HyperCastRawDateOptions *date_options, const uint8_t *vocab_ptr, uintptr_t vocab_len, const uint8_t *ptr, uintptr_t len, void *out, struct HyperCastRawFaultDetail *fault);

// Reads the double `value` through typed door `door`, reporting a failure's detail: the
// doors with a typed export, 19 (`cast_decimal_from_f64`, a `Decimal`), 22 with `param` the
// epoch (`cast_excel_serial_from_f64`, a `CivilDateTime`), 16 (`cast_excel_time`, a
// `uint64_t`) and 17 (`cast_excel_duration`, a `Duration`), each as that export reads it.
// `param` is `0` but at 22. `out` need not be aligned. A failure's span is empty, with no
// text to index. Returns as `cast_detailed` does; -1 for any other door, a `param` the door
// doesn't take, or a `fault` too small.
int32_t cast_detailed_from_f64(uint32_t door, uint32_t param, double value, void *out, struct HyperCastRawFaultDetail *fault);

// `cast_checked`, reporting a failure's detail: the same doors (the numeric ones and 18,
// text), declaration, `enum` list and `out`, with `param` `0` (no checked door declares one)
// and `format` judged by 1.1's rules (null for the invariant format; never read at 18).
// Returns 0 with the value written to `out` and `check` written (`keyword` 0, `adjusted`,
// `enum_index`); 1, 2 or 3, the door's own fault, with `offset`, `len` and `detail` written
// to `fault`; or 4, a violation of a declared rule, with `fault` spanning the value and
// `detail` 0, and `check` holding the `keyword`, `adjusted` 0 and `enum_index`
// `UINT32_MAX`. `fault` and `check` may be null. -1 writes nothing: what `cast_checked`
// refuses, a `fault` whose `size` is under 16 or not a multiple of 4, or a nonzero `param`.
int32_t cast_checked_detailed(uint32_t door, uint32_t param, const struct HyperCastRawNumFormat *format, const struct HyperCastRawConstraints *constraints, const uint8_t *enum_ptr, uintptr_t enum_len, const uint8_t *ptr, uintptr_t len, void *out, struct HyperCastRawFaultDetail *fault, struct HyperCastRawCheck *check);

// `cast_checked_from_f64`, reporting a failure's detail: the numeric doors, `param` `0`, and
// the codes and writes of `cast_checked_detailed`, every span empty. -1 writes nothing: what
// `cast_checked_from_f64` refuses, a `fault` too small, or a nonzero `param`.
int32_t cast_checked_detailed_from_f64(uint32_t door, uint32_t param, const struct HyperCastRawConstraints *constraints, const uint8_t *enum_ptr, uintptr_t enum_len, double value, void *out, struct HyperCastRawFaultDetail *fault, struct HyperCastRawCheck *check);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* HYPERCAST_H */
