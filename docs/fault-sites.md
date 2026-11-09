# Fault sites

Generated from `rust/src/site.rs` by `docs_fault_sites_is_current` in `rust/src/tests.rs`; never edit it by hand (`HYPERCAST_BLESS=1 cargo test --lib docs_fault_sites` in `rust/` rewrites it).

Every place a door fails, with the reason and detail it reports. A detailed door reports `invalid_utf8` instead wherever the bytes at the failure's span don't decode as UTF-8. The corpus reaches every site but two kinds, as `every_site_is_reached` checks: an **unreachable** site is a defensive check that no input reaches, for the reason given; an **impractical** site is reached only by inputs no corpus row can carry (megabytes, or past 4 GiB), and the named unit test in `rust/src/tests.rs` reaches it instead, under the same check.

| Site | File | Reason | Detail | Condition |
|---|---|---|---|---|
| `BoolBlank` | `boolean.rs` | `empty` | `blank` | The trimmed input is empty. |
| `BoolNotRecognized` | `boolean.rs` | `malformed` | `not_recognized` | The trimmed input is not one of the lexicon's words. |
| `CharEmpty` | `character.rs` | `empty` | `blank` | The input is empty. |
| `CharBlank` | `character.rs` | `empty` | `blank` | The input is only white space, and not one white-space character (which is verbatim). |
| `CharAmpersandMark` | `character.rs` | `malformed` | `delimiter_expected` | `&` is followed by something other than `#` or `H`, or by nothing. |
| `CharNotASpelling` | `character.rs` | `malformed` | `unexpected_character` | No prefix, and the token doesn't start with a decimal digit: not a code-point spelling. |
| `CharDigitExpected` | `character.rs` | `malformed` | `digit_expected` | A prefix (`U+`, `0x`, `&H`, `&#`, `&#x`) is followed by a character that is not a digit. |
| `CharBarePrefix` | `character.rs` | `malformed` | `no_digits` | A bare prefix (`U+`, `0x`, `&H`, `&#`, `&#x`) with nothing after it: no numeral at all, as a bare `0x` is at the integer doors; the span is the whole token. |
| `CharEntityUnterminated` | `character.rs` | `malformed` | `delimiter_expected` | An entity's digits end the input without its `;`; the span is the whole token. |
| `CharEntityStray` | `character.rs` | `malformed` | `unexpected_character` | An entity's digits are followed by a character other than `;`. |
| `CharEntityTrailing` | `character.rs` | `malformed` | `trailing_text` | More text follows an entity's `;`. |
| `CharAfterDigits` | `character.rs` | `malformed` | `unexpected_character` | A non-entity spelling's digits are followed by another character. |
| `CharNotAScalar` | `character.rs` | `out_of_range` | `not_a_scalar` | The spelling names a code point above U+10FFFF or a surrogate. |
| `TextEmpty` | `text.rs` | `empty` | `blank` | The input is empty. |
| `TextInvalidUtf8` | `text.rs` | `malformed` | `invalid_utf8` | The input is not UTF-8; the span is the first invalid sequence. |
| `DetectAmbiguous` | `verdict.rs` | `malformed` | `ambiguous_separator` | Separator detection finds a single `.` or `,` with exactly three digits after it and an integer part other than `0`. |
| `FramePercentOnly` | `integer.rs` | `malformed` | `no_digits` | Only a percent symbol (and a trailing sign) at a real or decimal door. |
| `FrameParensUnclosed` | `integer.rs` | `malformed` | `parentheses_unbalanced` | `(` without a closing `)`. |
| `FrameParensEmpty` | `integer.rs` | `malformed` | `parentheses_unbalanced` | Accounting parentheses with nothing inside. |
| `FrameCurrencyOnlyLeading` | `integer.rs` | `malformed` | `no_digits` | A leading currency symbol (and sign) with nothing after it. |
| `FrameCurrencyOnlyTrailing` | `integer.rs` | `malformed` | `no_digits` | A trailing currency symbol with nothing before it (a symbol that starts with a sign). |
| `FrameSignAfterSymbolAndPercent` | `integer.rs` | `malformed` | `sign_misplaced` | A sign after a trailing currency symbol and another after the percent; the span is the inner one. |
| `FrameSignBeforeSymbolInParens` | `integer.rs` | `malformed` | `sign_misplaced` | A sign ahead of a leading currency symbol inside accounting parentheses (`(-$5)`); the span is that sign. |
| `FrameSecondSignAfterSymbol` | `integer.rs` | `malformed` | `sign_misplaced` | A sign ahead of a leading currency symbol and another after it (`-$-5`); the span is the second. |
| `FrameSignInParens` | `integer.rs` | `malformed` | `sign_misplaced` | A sign inside accounting parentheses (`(-5)`). |
| `FrameTrailingSignTwice` | `integer.rs` | `malformed` | `sign_misplaced` | A trailing sign on a token that is already signed or in parentheses. |
| `FramePostSignTwice` | `integer.rs` | `malformed` | `sign_misplaced` | A sign after the percent on a token that is already signed or in parentheses. |
| `IntBlank` | `integer.rs` | `empty` | `blank` | The trimmed input is empty. |
| `IntFastNegative` | `integer.rs` | `out_of_range` | `negative_not_allowed` | The plain-digits fast path reads a negative value at an unsigned door. |
| `IntFastTooLarge` | `integer.rs` | `out_of_range` | `too_large` | The plain-digits fast path reads a value above the type's maximum. |
| `IntFastTooSmall` | `integer.rs` | `out_of_range` | `too_small` | The plain-digits fast path reads a value below a signed type's minimum. |
| `RadixNoDigits` | `integer.rs` | `malformed` | `no_digits` | A radix prefix (`0x`, `&H`, `0b`) with no digits after it. |
| `RadixDigitExpected` | `integer.rs` | `malformed` | `digit_expected` | A character after a radix prefix that is not a digit of that radix. |
| `RadixTooLarge` | `integer.rs` | `out_of_range` | `too_large` | A radix bit pattern wider than the type. |
| `IntLaneNegative` | `integer.rs` | `out_of_range` | `negative_not_allowed` | The lenient lane reads a negative value at an unsigned door. |
| `IntLaneTooLarge` | `integer.rs` | `out_of_range` | `too_large` | The lenient lane reads a value above the type's maximum. |
| `IntLaneTooSmall` | `integer.rs` | `out_of_range` | `too_small` | The lenient lane reads a value below a signed type's minimum. |
| `IntDecimalPoint` | `integer.rs` | `malformed` | `fraction_not_allowed` | A decimal separator at an integer door. |
| `IntGroupMisplaced` | `integer.rs` | `malformed` | `group_misplaced` | A group separator not between two digits. |
| `IntGroupSize` | `integer.rs` | `malformed` | `group_size` | A group its grouping policy refuses: the wrong number of digits, or a first group that starts with a zero. The span is the group, with the separator before it unless it is the leftmost. |
| `IntNegativeExponent` | `integer.rs` | `malformed` | `fraction_not_allowed` | A negative exponent at an integer door; the span is its `-`. |
| `IntExponentDigitExpected` | `integer.rs` | `malformed` | `digit_expected` | An `e` (and its sign) with no exponent digit after it; the span is the `e`. |
| `IntAfterExponentSign` | `integer.rs` | `malformed` | `sign_misplaced` | A sign after the exponent's digits. |
| `IntGroupInExponent` | `integer.rs` | `malformed` | `group_in_fraction` | A group separator after the exponent's digits. |
| `IntAfterExponent` | `integer.rs` | `malformed` | `unexpected_character` | Another character after the exponent's digits. |
| `IntSignMisplaced` | `integer.rs` | `malformed` | `sign_misplaced` | A second leading sign, or a sign inside the numeral. |
| `IntNotFinite` | `integer.rs` | `malformed` | `not_finite` | The token, less its frame, is `nan`, `inf` or `infinity` (ASCII case-insensitive); the span is its first letter. |
| `IntUnexpected` | `integer.rs` | `malformed` | `unexpected_character` | Any other character the numeral doesn't allow: a letter, an undeclared symbol, `%`, `e` with exponents off, interior white space. |
| `IntNoDigits` | `integer.rs` | `malformed` | `no_digits` | Only a sign: no digit at all. |
| `IntOverflowNegative` | `integer.rs` | `out_of_range` | `negative_not_allowed` | The magnitude overflows 128 bits, negative, at an unsigned door. |
| `IntOverflowTooLarge` | `integer.rs` | `out_of_range` | `too_large` | The magnitude overflows 128 bits, positive. |
| `IntOverflowTooSmall` | `integer.rs` | `out_of_range` | `too_small` | The magnitude overflows 128 bits, negative, at a signed door. |
| `IntScaledNegative` | `integer.rs` | `out_of_range` | `negative_not_allowed` | The exponent scales the value past 128 bits, negative, at an unsigned door. |
| `IntScaledTooLarge` | `integer.rs` | `out_of_range` | `too_large` | The exponent scales the value past 128 bits, positive. |
| `IntScaledTooSmall` | `integer.rs` | `out_of_range` | `too_small` | The exponent scales the value past 128 bits, negative, at a signed door. |
| `IntRangeNegative` | `integer.rs` | `out_of_range` | `negative_not_allowed` | The engine reads a negative value at an unsigned door. |
| `IntRangeTooLarge` | `integer.rs` | `out_of_range` | `too_large` | The engine reads a value above the type's maximum. |
| `IntRangeTooSmall` | `integer.rs` | `out_of_range` | `too_small` | The engine reads a value below a signed type's minimum. |
| `RealBlank` | `real.rs` | `empty` | `blank` | The trimmed input is empty. |
| `RealUnparsed` | `real.rs` | `malformed` | `unexpected_character` | The conversion declines the text the engine wrote, which is always in its shape. **Unreachable:** the engine only writes `[-]digits[.digits][e[+\|-]digits]` with a digit, which the conversion always reads. |
| `RealEngineTooLarge` | `real.rs` | `out_of_range` | `too_large` | A magnitude past the type's finite range, read by the engine. |
| `RealTooLarge` | `real.rs` | `out_of_range` | `too_large` | A magnitude past the type's finite range, read by the plain shape or the lenient lane. |
| `RealMarkRepeated` | `real.rs` | `malformed` | `mark_repeated` | A second decimal separator. |
| `RealGroupInFraction` | `real.rs` | `malformed` | `group_in_fraction` | A group separator after the decimal separator. |
| `RealGroupMisplaced` | `real.rs` | `malformed` | `group_misplaced` | A group separator not between two digits. |
| `RealGroupSize` | `real.rs` | `malformed` | `group_size` | A group its grouping policy refuses: the wrong number of digits, or a first group that starts with a zero. The span is the group, with the separator before it unless it is the leftmost. |
| `RealExponentDigitExpected` | `real.rs` | `malformed` | `digit_expected` | An `e` (and its sign) with no exponent digit after it; the span is the `e`. |
| `RealAfterExponentSign` | `real.rs` | `malformed` | `sign_misplaced` | A sign after the exponent's digits. |
| `RealGroupInExponent` | `real.rs` | `malformed` | `group_in_fraction` | A group separator after the exponent's digits. |
| `RealAfterExponent` | `real.rs` | `malformed` | `unexpected_character` | Another character after the exponent's digits. |
| `RealSignMisplaced` | `real.rs` | `malformed` | `sign_misplaced` | A second leading sign, or a sign inside the numeral. |
| `RealNotFinite` | `real.rs` | `malformed` | `not_finite` | The token, less its frame, is `nan`, `inf` or `infinity` (ASCII case-insensitive); the span is its first letter. |
| `RealUnexpected` | `real.rs` | `malformed` | `unexpected_character` | Any other character the numeral doesn't allow: a letter, an undeclared symbol, `e` with exponents off, interior white space. |
| `RealNoDigits` | `real.rs` | `malformed` | `no_digits` | Only a sign: no digit at all. |
| `RealTooLong` | `real.rs` | `malformed` | `too_long` | The numeral normalizes to more than 256 bytes. |
| `DecimalBlank` | `decimal.rs` | `empty` | `blank` | The trimmed input is empty. |
| `DecimalWholeTooLarge` | `decimal.rs` | `out_of_range` | `too_large` | The whole part of the magnitude overflows 128 bits, or a fraction digit lands on a whole part already past 96 bits. |
| `DecimalFractionTooPrecise` | `decimal.rs` | `out_of_range` | `too_precise` | A nonzero fraction digit no longer fits 96 bits. |
| `DecimalNoDigits` | `decimal.rs` | `malformed` | `no_digits` | The invariant text has no digit. **Unreachable:** the plain shape and the engine's text always carry a digit. |
| `DecimalExponentDigitExpected` | `decimal.rs` | `malformed` | `digit_expected` | The invariant text's exponent has no digit. **Unreachable:** the plain shape and the engine's text always carry an exponent digit. |
| `DecimalTrailing` | `decimal.rs` | `malformed` | `trailing_text` | The invariant text has more after the numeral. **Unreachable:** the plain shape and the engine's text are read to their end. |
| `DecimalShiftTooLarge` | `decimal.rs` | `out_of_range` | `too_large` | The exponent shifts the magnitude past 96 bits. |
| `DecimalTooLarge` | `decimal.rs` | `out_of_range` | `too_large` | The magnitude, its exact zeros shed, is past 96 bits. |
| `DecimalTooPrecise` | `decimal.rs` | `out_of_range` | `too_precise` | The scale, its exact zeros shed, is past 28. |
| `WholeNotFinite` | `typed.rs` | `malformed` | `not_finite` | A NaN or an infinity at a whole-number door. |
| `WholeNotWhole` | `typed.rs` | `malformed` | `not_whole` | A finite double with a fractional part at a whole-number door. |
| `WholeUnrepresentable` | `typed.rs` | `malformed` | `not_finite` | An integral double from 2^53 up has no shortest digits. **Unreachable:** every finite nonzero double has shortest digits. |
| `TypedIntNegative` | `typed.rs` | `out_of_range` | `negative_not_allowed` | A negative whole number at an unsigned integer door. |
| `TypedIntTooLarge` | `typed.rs` | `out_of_range` | `too_large` | A whole number above the integer type's maximum. |
| `TypedIntTooSmall` | `typed.rs` | `out_of_range` | `too_small` | A whole number below a signed integer type's minimum. |
| `F32TypedNan` | `typed.rs` | `malformed` | `not_finite` | NaN at the `f32` door. |
| `F32TypedInfinite` | `typed.rs` | `out_of_range` | `not_finite` | An infinity at the `f32` door. |
| `F32TypedTooLarge` | `typed.rs` | `out_of_range` | `too_large` | A finite double past `f32::MAX`. |
| `BoolTypedNotFinite` | `typed.rs` | `malformed` | `not_finite` | NaN or an infinity at the `bool` door. |
| `BoolTypedNotRecognized` | `typed.rs` | `malformed` | `not_recognized` | A finite double other than `0` or `1` at the `bool` door. |
| `DecimalTypedNan` | `typed.rs` | `malformed` | `not_finite` | NaN at the decimal door. |
| `DecimalTypedInfinite` | `typed.rs` | `out_of_range` | `not_finite` | An infinity at the decimal door. |
| `DecimalTypedShiftTooLarge` | `typed.rs` | `out_of_range` | `too_large` | The shortest digits shifted by their exponent overflow 128 bits. |
| `DecimalTypedTooLarge` | `typed.rs` | `out_of_range` | `too_large` | The magnitude is past 96 bits. |
| `DecimalTypedTooPrecise` | `typed.rs` | `out_of_range` | `too_precise` | The scale is past 28. |
| `CheckedF64Nan` | `checked.rs` | `malformed` | `not_finite` | NaN at the checked `f64` door. |
| `CheckedF64Infinite` | `checked.rs` | `out_of_range` | `not_finite` | An infinity at the checked `f64` door. |
| `CheckedSpanTooLong` | `checked.rs` | `out_of_range` | `too_long` | A success span that ends past `u32::MAX` bytes. **Impractical:** only an input past 4 GiB reaches it; reached by the unit test `checked_span_too_long`. Where `usize` is 32 bits (wasm32) no length can pass `u32::MAX`, so the site is structurally unreachable there, which the coverage test asserts. |
| `CheckedMultipleTooLong` | `checked.rs` | `out_of_range` | `too_large` | `multipleOf` rounding yields a multiple too long to hold. **Unreachable:** a declared step is never zero, and a value a door accepts rounds to at most 337 digits (an `f64` near its maximum at a step of scale 28), within the 340 `Rounded` holds. |
| `CheckedRoundedTooLarge` | `checked.rs` | `out_of_range` | `too_large` | `multipleOf` rounding yields a multiple past what the door's type holds. |
| `CheckedRoundedTooSmall` | `checked.rs` | `out_of_range` | `too_small` | `multipleOf` rounding yields a multiple below a signed integer type's minimum. |
| `CheckedTextBlank` | `checked.rs` | `empty` | `blank` | The text door's input is white space only, under `trim`. |
| `UuidBlank` | `uuid.rs` | `empty` | `blank` | The trimmed input is empty. |
| `UuidBarePrefix` | `uuid.rs` | `malformed` | `component_missing` | A prefix (`urn:uuid:`, `GUID:`, `UUID:`) with nothing after it. |
| `UuidHexDigit` | `uuid.rs` | `malformed` | `digit_expected` | A character that is not a hex digit where one is required. |
| `UuidWrongLength` | `uuid.rs` | `malformed` | `wrong_length` | An unwrapped token of neither 32 nor 36 characters. |
| `UuidHyphen` | `uuid.rs` | `malformed` | `delimiter_expected` | A D-format hyphen is missing or different. |
| `UuidNWrongLength` | `uuid.rs` | `malformed` | `wrong_length` | An N-format token of other than 32 characters. **Unreachable:** the N format is chosen only for 32 characters. |
| `UuidNoBadPair` | `uuid.rs` | `malformed` | `digit_expected` | The digits fail to decode but no pair holds a non-hex byte. **Unreachable:** the decoder refuses exactly the pairs holding a non-hex byte. |
| `UuidWrappedWrongLength` | `uuid.rs` | `malformed` | `wrong_length` | A braced or parenthesized token of other than 38 characters. |
| `UuidWrappedClose` | `uuid.rs` | `malformed` | `delimiter_expected` | A braced or parenthesized token without its closing mark. |
| `UuidXDelimiter` | `uuid.rs` | `malformed` | `delimiter_expected` | An X-format mark (`{`, `,`, `}`) is different. |
| `UuidXEnded` | `uuid.rs` | `malformed` | `delimiter_expected` | The X format ends before a mark; the span is the whole token. |
| `UuidXPrefix` | `uuid.rs` | `malformed` | `delimiter_expected` | An X-format component without its `0x`. |
| `UuidXWidth` | `uuid.rs` | `malformed` | `field_width` | An X-format component with more digits than its width. |
| `UuidXDigitExpected` | `uuid.rs` | `malformed` | `digit_expected` | An X-format component's `0x` with no digit after it. |
| `UuidXTrailing` | `uuid.rs` | `malformed` | `trailing_text` | More text after the X format's closing `}`. |
| `FractionDigitExpected` | `temporal.rs` | `malformed` | `digit_expected` | A decimal mark with no digit after it; the span is the mark. |
| `DigitRunTooLong` | `temporal.rs` | `malformed` | `too_long` | A digit run past eighteen digits; the span is the nineteenth. |
| `DateYearShort` | `temporal.rs` | `malformed` | `wrong_length` | The ISO date's year is too short to read. **Unreachable:** the ISO date is read only from ten bytes. |
| `DateYearDigits` | `temporal.rs` | `malformed` | `digit_expected` | The ISO date's year is not four digits. |
| `DateFirstHyphen` | `temporal.rs` | `malformed` | `delimiter_expected` | The ISO date's first `-` is missing or different. |
| `DateMonthDigits` | `temporal.rs` | `malformed` | `digit_expected` | The ISO date's month is not two digits. |
| `DateSecondHyphen` | `temporal.rs` | `malformed` | `delimiter_expected` | The ISO date's second `-` is missing or different. |
| `DateDayDigits` | `temporal.rs` | `malformed` | `digit_expected` | The ISO date's day is not two digits. |
| `DateYearRange` | `temporal.rs` | `out_of_range` | `date_field_out_of_range` | Year 0000. |
| `DateMonthRange` | `temporal.rs` | `out_of_range` | `date_field_out_of_range` | A month outside 1–12. |
| `DateDayRange` | `temporal.rs` | `out_of_range` | `date_field_out_of_range` | A day the month doesn't have. |
| `ClockHourRange` | `temporal.rs` | `out_of_range` | `time_field_out_of_range` | An hour outside the clock's range. |
| `ClockMinuteRange` | `temporal.rs` | `out_of_range` | `time_field_out_of_range` | A minute past 59. |
| `ClockSecondRange` | `temporal.rs` | `out_of_range` | `time_field_out_of_range` | A second past 59 (a leap second included). |
| `DateBlank` | `temporal.rs` | `empty` | `blank` | The trimmed input is empty. |
| `DateWrongLength` | `temporal.rs` | `malformed` | `wrong_length` | The trimmed input is not ten characters long. |
| `OrderedFieldDigitExpected` | `temporal.rs` | `malformed` | `digit_expected` | A date field with no digit. |
| `OrderedFieldWidth` | `temporal.rs` | `malformed` | `field_width` | A date field of five or more digits. |
| `OrderedFirstSeparator` | `temporal.rs` | `malformed` | `delimiter_expected` | The first separator is not `/`, `-` or `.`, or is missing. |
| `OrderedSecondSeparator` | `temporal.rs` | `malformed` | `delimiter_expected` | The second separator is missing or differs from the first. |
| `OrderedMonthWidth` | `temporal.rs` | `malformed` | `field_width` | The month field is more than two digits. |
| `OrderedDayWidth` | `temporal.rs` | `malformed` | `field_width` | The day field is more than two digits. |
| `OrderedYearWidth` | `temporal.rs` | `malformed` | `field_width` | The year field is not four digits (nor two, under a pivot). |
| `OrderedMonthNotRecognized` | `temporal.rs` | `malformed` | `not_recognized` | A month field that starts with a word, under a vocabulary, that is not exactly one of its names; the span is the word. |
| `OrderedMonthDelimiter` | `temporal.rs` | `malformed` | `delimiter_expected` | A month field that is exactly a declared name, under a vocabulary, with a different mark or none after it; the span is that character, empty at the end. |
| `OrderedBlank` | `temporal.rs` | `empty` | `blank` | The trimmed input is empty (`cast_date_ordered`). |
| `OrderedTrailing` | `temporal.rs` | `malformed` | `trailing_text` | More text after a complete date (`cast_date_ordered`). |
| `DatetimeBlank` | `temporal.rs` | `empty` | `blank` | The trimmed input is empty. |
| `DatetimeSeparator` | `temporal.rs` | `malformed` | `delimiter_expected` | A character other than a space or `T` after the date. |
| `CivilHourDigitExpected` | `temporal.rs` | `malformed` | `digit_expected` | The hour has no digit. |
| `CivilHourWidth` | `temporal.rs` | `malformed` | `field_width` | The hour has more than two digits. |
| `CivilMinuteDigits` | `temporal.rs` | `malformed` | `digit_expected` | The minutes are not two digits. |
| `CivilSecondDigits` | `temporal.rs` | `malformed` | `digit_expected` | The seconds are not two digits. |
| `CivilMinutesMissing` | `temporal.rs` | `malformed` | `component_missing` | A bare hour with neither minutes nor a day-period marker; the span is the hour. |
| `DatetimeTrailing` | `temporal.rs` | `malformed` | `trailing_text` | More text after a complete time. |
| `TimeBlank` | `temporal.rs` | `empty` | `blank` | The trimmed input is empty. |
| `TimeTrailing` | `temporal.rs` | `malformed` | `trailing_text` | More text after a complete time. |
| `TimeHourDigits` | `temporal.rs` | `malformed` | `digit_expected` | The hour is not two digits. |
| `TimeColon` | `temporal.rs` | `malformed` | `delimiter_expected` | The `:` after the hour is missing or different. |
| `TimeMinuteDigits` | `temporal.rs` | `malformed` | `digit_expected` | The minutes are not two digits. |
| `TimeSecondDigits` | `temporal.rs` | `malformed` | `digit_expected` | The seconds are not two digits. |
| `TimestampBlank` | `temporal.rs` | `empty` | `blank` | The trimmed input is empty. |
| `TimestampTooShort` | `temporal.rs` | `malformed` | `wrong_length` | The trimmed input is shorter than a date and its `T`. |
| `TimestampT` | `temporal.rs` | `malformed` | `delimiter_expected` | The `T` after the date is missing or different. |
| `TimestampTimeDigit` | `temporal.rs` | `malformed` | `digit_expected` | The time's first non-digit, ahead of its seconds, is where a digit must be; the span is the whole time. |
| `TimestampTimeColon` | `temporal.rs` | `malformed` | `delimiter_expected` | The time's first wrong character, ahead of its seconds, is where a `:` must be; the span is the whole time. |
| `TimestampSecondsMissing` | `temporal.rs` | `malformed` | `component_missing` | `HH:mm` ends the input or meets a zone: the seconds are missing; the span is the whole time. |
| `TimestampZoneMissing` | `temporal.rs` | `malformed` | `component_missing` | No zone after the time. |
| `TimestampTrailingAfterZ` | `temporal.rs` | `malformed` | `trailing_text` | More text after `Z`. |
| `TimestampOffsetHourDigits` | `temporal.rs` | `malformed` | `digit_expected` | The offset's hours are not two digits. |
| `TimestampOffsetColon` | `temporal.rs` | `malformed` | `delimiter_expected` | The offset's `:` is missing or different. |
| `TimestampOffsetMinuteDigits` | `temporal.rs` | `malformed` | `digit_expected` | The offset's minutes are not two digits. |
| `TimestampTrailingAfterOffset` | `temporal.rs` | `malformed` | `trailing_text` | More text after the offset. |
| `TimestampZoneUnexpected` | `temporal.rs` | `malformed` | `unexpected_character` | A character after the time that starts neither a fraction nor a zone. |
| `TimestampOffsetHourRange` | `temporal.rs` | `out_of_range` | `time_field_out_of_range` | The offset's hours past 23. |
| `TimestampOffsetMinuteRange` | `temporal.rs` | `out_of_range` | `time_field_out_of_range` | The offset's minutes past 59. |
| `TimestampOutsideWindow` | `temporal.rs` | `out_of_range` | `outside_window` | A real instant outside 0001-01-01 through 9999-12-31 UTC. |
| `UnixBlank` | `temporal.rs` | `empty` | `blank` | The trimmed input is empty. |
| `UnixNoDigits` | `temporal.rs` | `malformed` | `no_digits` | Only a sign. |
| `UnixFraction` | `temporal.rs` | `malformed` | `fraction_not_allowed` | A `.` in the count. |
| `UnixSignMisplaced` | `temporal.rs` | `malformed` | `sign_misplaced` | A second sign. |
| `UnixUnexpected` | `temporal.rs` | `malformed` | `unexpected_character` | Any other non-digit in the count. |
| `UnixOverflow` | `temporal.rs` | `out_of_range` | `too_large` | The count overflows 128 bits. |
| `UnixOutsideWindow` | `temporal.rs` | `out_of_range` | `outside_window` | The instant is outside the timestamp window. |
| `UnixTypedOverflow` | `temporal.rs` | `out_of_range` | `too_large` | The typed count overflows 128 bits. |
| `UnixTypedOutsideWindow` | `temporal.rs` | `out_of_range` | `outside_window` | The typed count's instant is outside the timestamp window. |
| `SerialNotFinite` | `temporal.rs` | `malformed` | `not_finite` | NaN or an infinity as a serial. |
| `SerialNegative` | `temporal.rs` | `malformed` | `negative_not_allowed` | A negative serial. |
| `SerialPastLastDay` | `temporal.rs` | `out_of_range` | `outside_window` | A serial past either system's last day. |
| `SerialPhantom` | `temporal.rs` | `out_of_range` | `nonexistent_date` | The 1900 system's phantom serial 60 (`1900-02-29`). |
| `SerialOutsideWindow` | `temporal.rs` | `out_of_range` | `outside_window` | A serial below the system's first day, or past `9999-12-31`. |
| `DurationTypedNotFinite` | `temporal.rs` | `malformed` | `not_finite` | NaN or an infinity as a duration in days. |
| `DurationTypedOutsideWindow` | `temporal.rs` | `out_of_range` | `outside_window` | A duration in days past ±10,000 years. |
| `DurationTypedSnappedOutsideWindow` | `temporal.rs` | `out_of_range` | `outside_window` | A duration in days whose snapped size is past ±10,000 years. **Unreachable:** a span inside the window snaps by at most a few microseconds, never past a whole second. |
| `SerialTextBlank` | `temporal.rs` | `empty` | `blank` | The trimmed input is empty. |
| `SerialTextNegative` | `temporal.rs` | `malformed` | `negative_not_allowed` | A `-` ahead of the serial. |
| `SerialTextPlus` | `temporal.rs` | `malformed` | `sign_misplaced` | A `+` ahead of the serial: a sign where none may stand (only a `-` is a negative). |
| `SerialTextDigitExpected` | `temporal.rs` | `malformed` | `digit_expected` | Another character where the serial's first digit must be. |
| `SerialTextFractionDigit` | `temporal.rs` | `malformed` | `digit_expected` | A `.` with no digit after it; the span is the `.`. |
| `SerialTextTrailing` | `temporal.rs` | `malformed` | `trailing_text` | White space after the serial, with more text after it. |
| `SerialTextUnexpected` | `temporal.rs` | `malformed` | `unexpected_character` | Any other character after the serial's digits. |
| `SerialTextOverflow` | `temporal.rs` | `out_of_range` | `too_large` | The whole days overflow 64 bits. |
| `SerialTextPhantom` | `temporal.rs` | `out_of_range` | `nonexistent_date` | The 1900 system's phantom serial 60 (`1900-02-29`). |
| `SerialTextOutsideWindow` | `temporal.rs` | `out_of_range` | `outside_window` | A serial below the system's first day, or past `9999-12-31`. |
| `SerialTextInstantOutsideWindow` | `temporal.rs` | `out_of_range` | `outside_window` | The serial's instant is outside the timestamp window. **Unreachable:** every serial the day check accepts, with any fraction, is inside the window. |
| `DurationBlank` | `temporal.rs` | `empty` | `blank` | The trimmed input is empty. |
| `DurationOutsideWindow` | `temporal.rs` | `out_of_range` | `outside_window` | Whole seconds past ±10,000 years. |
| `IsoTRepeated` | `temporal.rs` | `malformed` | `mark_repeated` | A second `T` in an ISO duration. |
| `IsoDigitExpected` | `temporal.rs` | `malformed` | `digit_expected` | A character where an ISO component's number must start. |
| `IsoUnitMissing` | `temporal.rs` | `malformed` | `component_missing` | An ISO component's number with no unit; the span is the number. |
| `IsoUnknownUnit` | `temporal.rs` | `malformed` | `unknown_unit` | An unknown or misplaced ISO unit (years, months before `T`, a time unit before `T`). |
| `IsoFractionNotAllowed` | `temporal.rs` | `malformed` | `fraction_not_allowed` | A fraction on an ISO unit other than seconds; the span is the component. |
| `IsoOverflow` | `temporal.rs` | `out_of_range` | `too_large` | The ISO total overflows 128 bits of nanoseconds. **Impractical:** a component is at most 18 digits of weeks, so overflowing takes some 280,000 components, megabytes of input; reached by the unit test `iso_overflow`. |
| `IsoNoComponent` | `temporal.rs` | `malformed` | `component_missing` | An ISO duration with no component, or a `T` with no time component. |
| `ColonFirstDigitExpected` | `temporal.rs` | `malformed` | `digit_expected` | The colon form's first field has no digit. |
| `ColonHourDigitExpected` | `temporal.rs` | `malformed` | `digit_expected` | The hours after a day part have no digit. |
| `ColonHourWidth` | `temporal.rs` | `malformed` | `field_width` | The hours after a day part have more than two digits. |
| `ColonFirstWidth` | `temporal.rs` | `malformed` | `field_width` | The hours, with no day part, have more than two digits. |
| `ColonColon` | `temporal.rs` | `malformed` | `delimiter_expected` | The `:` after the hours is missing or different. |
| `ColonMinuteDigitExpected` | `temporal.rs` | `malformed` | `digit_expected` | The minutes have no digit. |
| `ColonMinuteWidth` | `temporal.rs` | `malformed` | `field_width` | The minutes have more than two digits. |
| `ColonSecondDigitExpected` | `temporal.rs` | `malformed` | `digit_expected` | The seconds have no digit. |
| `ColonSecondWidth` | `temporal.rs` | `malformed` | `field_width` | The seconds have more than two digits. |
| `ColonTrailing` | `temporal.rs` | `malformed` | `trailing_text` | More text after a complete colon duration. |
| `ColonFieldRange` | `temporal.rs` | `out_of_range` | `time_field_out_of_range` | Hours past 23, or minutes or seconds past 59. |
| `ProtobufUnitMissing` | `temporal.rs` | `malformed` | `component_missing` | A token that doesn't end in `s` (not a duration at all, or one missing its unit). |
| `ProtobufNoDigits` | `temporal.rs` | `malformed` | `no_digits` | Only `s`, or a sign and `s`. |
| `ProtobufDigitExpected` | `temporal.rs` | `malformed` | `digit_expected` | Something other than a digit where the seconds must start. |
| `ProtobufTrailing` | `temporal.rs` | `malformed` | `trailing_text` | An `s` ends a complete duration and more text follows. |
| `ProtobufUnexpected` | `temporal.rs` | `malformed` | `unexpected_character` | Any other character after the seconds. |

232 sites: 11 unreachable, 2 impractical.
