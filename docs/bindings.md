# Binding parity

One row per concept, one column per language, each cell the public spelling of that concept
in that package, so drift between the Rust template and the seven bindings shows at a glance.
A change that adds a door or a concept adds its row here in the same change.

## Doors

The 26 `cast_*` exports of `rust/src/ffi.rs`, in export order by group. Every binding door
also takes the text first (or the `f64` for the four typed doors) and returns that
language's verdict (see [Verdict](#verdict)).

| Export | Rust | C# | Java | Go | Swift | PHP | Ruby | Python |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `cast_bool` | `cast_bool` | `Cast.Boolean` | `Cast.bool` | `Bool` | `Cast.bool(_:)` | `Cast::bool` | `HyperCast.bool` | `cast_bool` |
| `cast_char` | `cast_char` | `Cast.Char` | `Cast.character` [^java-char] | `Char` | `Cast.char(_:)` | `Cast::char` | `HyperCast.char` | `cast_char` |
| `cast_i8` | `cast_i8` | `Cast.SByte` | `Cast.i8` | `I8` | `Cast.i8(_:format:)` | `Cast::i8` | `HyperCast.i8` | `cast_i8` |
| `cast_i16` | `cast_i16` | `Cast.Int16` | `Cast.i16` | `I16` | `Cast.i16(_:format:)` | `Cast::i16` | `HyperCast.i16` | `cast_i16` |
| `cast_i32` | `cast_i32` | `Cast.Int32` | `Cast.i32` | `I32` | `Cast.i32(_:format:)` | `Cast::i32` | `HyperCast.i32` | `cast_i32` |
| `cast_i64` | `cast_i64` | `Cast.Int64` | `Cast.i64` | `I64` | `Cast.i64(_:format:)` | `Cast::i64` | `HyperCast.i64` | `cast_i64` |
| `cast_u8` | `cast_u8` | `Cast.Byte` | `Cast.u8` | `U8` | `Cast.u8(_:format:)` | `Cast::u8` | `HyperCast.u8` | `cast_u8` |
| `cast_u16` | `cast_u16` | `Cast.UInt16` | `Cast.u16` | `U16` | `Cast.u16(_:format:)` | `Cast::u16` | `HyperCast.u16` | `cast_u16` |
| `cast_u32` | `cast_u32` | `Cast.UInt32` | `Cast.u32` | `U32` | `Cast.u32(_:format:)` | `Cast::u32` | `HyperCast.u32` | `cast_u32` |
| `cast_u64` | `cast_u64` | `Cast.UInt64` | `Cast.u64` | `U64` | `Cast.u64(_:format:)` | `Cast::u64` | `HyperCast.u64` | `cast_u64` |
| `cast_f32` | `cast_f32` | `Cast.Single` | `Cast.f32` | `F32` | `Cast.f32(_:format:)` | `Cast::f32` | `HyperCast.f32` | `cast_f32` |
| `cast_f64` | `cast_f64` | `Cast.Double` | `Cast.f64` | `F64` | `Cast.f64(_:format:)` | `Cast::f64` | `HyperCast.f64` | `cast_f64` |
| `cast_decimal` | `cast_decimal` | `Cast.Decimal` | `Cast.decimal` | `Exact` [^go-names] | `Cast.decimal(_:format:)` | `Cast::decimal` | `HyperCast.decimal` | `cast_decimal` |
| `cast_uuid` | `cast_uuid` | `Cast.Uuid` | `Cast.uuid` | `Uuid` | `Cast.uuid(_:)` | `Cast::uuid`; `Cast::uuidBytes` for the 16 raw bytes | `HyperCast.uuid` | `cast_uuid` |
| `cast_timestamp` | `cast_timestamp` | `Cast.Timestamp` | `Cast.timestamp` | `Timestamp` | `Cast.timestamp(_:)` | `Cast::timestamp` | `HyperCast.timestamp` | `cast_timestamp` |
| `cast_unix` | `cast_unix(input, precision)` | `Cast.Unix(input, precision)` | `Cast.unix(text, precision)` | `Unix(text, precision)` | `Cast.unix(_:precision:)` | `Cast::unix($text, $precision)` | `HyperCast.unix(text, precision)` | `cast_unix(text, precision)` |
| `cast_excel_serial` | `cast_excel_serial(input, epoch)` | `Cast.ExcelSerial(input, epoch)` | `Cast.excelSerial(text, epoch)` | `ExcelSerial(text, epoch)` | `Cast.excelSerial(_:epoch:)` | `Cast::excelSerial($text, $epoch)` | `HyperCast.excel_serial(text, epoch)` | `cast_excel_serial(text, epoch)` |
| `cast_date` | `cast_date` | `Cast.Date(input)` | `Cast.date(text)` | `DateOnly` [^go-names] | `Cast.date(_:)` | `Cast::date($text)` [^folded] | `HyperCast.date(text)` [^folded] | `cast_date(text)` [^folded] |
| `cast_date_ordered` | `cast_date_ordered(input, order)` | `Cast.Date(input, order)` (overload) | `Cast.date(text, order)` (overload) | `DateOnlyOrdered(text, order)` | `Cast.date(_:order:)` (overload) | `Cast::date($text, $order)` [^folded] | `HyperCast.date(text, order)` [^folded] | `cast_date(text, order)` [^folded] |
| `cast_datetime` | `cast_datetime(input, order)` | `Cast.DateTime(input, order)` | `Cast.dateTime(text, order)` | `DateTime(text, order)` | `Cast.dateTime(_:order:)` | `Cast::datetime($text, $order)` | `HyperCast.datetime(text, order)` | `cast_datetime(text, order)` |
| `cast_time` | `cast_time` | `Cast.Time` | `Cast.time` | `TimeOfDay` [^go-names] | `Cast.time(_:)` | `Cast::time` | `HyperCast.time` | `cast_time` |
| `cast_duration` | `cast_duration` | `Cast.Duration` | `Cast.duration` | `Span` [^go-names] | `Cast.duration(_:)` | `Cast::duration` | `HyperCast.duration` | `cast_duration` |
| `cast_decimal_from_f64` | `decimal_from_f64` [^typed-reason] | `Cast.DecimalFromDouble` | `Cast.decimalFromDouble` | `ExactFromFloat64` | `Cast.decimalFromDouble(_:)` | `Cast::decimalFromFloat` | `HyperCast.decimal_from_float` | `cast_decimal_from_float` |
| `cast_excel_serial_from_f64` | `excel_serial(serial, epoch)` [^typed-reason] [^excel-serial-name] | `Cast.ExcelSerialFromDouble(value, epoch)` | `Cast.excelSerialFromDouble(value, epoch)` | `ExcelSerialFromFloat64(value, epoch)` | `Cast.excelSerialFromDouble(_:epoch:)` | `Cast::excelSerialFromFloat($value, $epoch)` | `HyperCast.excel_serial_from_float(value, epoch)` | `cast_excel_serial_from_float(value, epoch)` |
| `cast_excel_time` | `excel_time` [^typed-reason] | `Cast.ExcelTime` | `Cast.excelTime` | `ExcelTime` | `Cast.excelTime(_:)` | `Cast::excelTime` | `HyperCast.excel_time` | `cast_excel_time` |
| `cast_excel_duration` | `excel_duration` [^typed-reason] | `Cast.ExcelDuration` | `Cast.excelDuration` | `ExcelDuration` | `Cast.excelDuration(_:)` | `Cast::excelDuration` | `HyperCast.excel_duration` | `cast_excel_duration` |

The integer, real and decimal doors take a `NumFormat` second (`format`, or `fmt` in
Python). Every Swift door is `throws` [^swift-throws].

### Doors beyond the ABI

| Concept | Rust | C# | Java | Go | Swift | PHP | Ruby | Python |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| One numeric door generic over the target | — | `Cast.Numeric<T>` (`T : struct, INumber<T>`) | — [^generic] | `Numeric[V Number, T Text]` | `Cast.numeric<T: NumericCastTarget>(_:format:)` | — [^generic] | — [^generic] | — [^generic] |
| One door generic over every target [^scalar] | — | `Cast.Scalar<T>` (`T : struct`; `NotSupportedException` otherwise) | — [^generic] | `Scalar[V ScalarTarget, T Text]` | `Cast.scalar<T: ScalarCastTarget>(_:format:)` | — [^generic] | — [^generic] | — [^generic] |
| Remaining typed doors (a number already held) | `i8_from_f64` … `u64_from_f64`, `f32_from_f64`, `bool_from_f64`, `unix_from_f64` | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] |
| Shortest digits of an `f64` | `shortest_digits` → `Option<ShortestDigits>` | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] |
| Civil ↔ instant, stated UTC | `CivilDateTime::assume_utc`, `Timestamp::utc_civil` | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] |
| Window constants | `MIN_TIMESTAMP_SECONDS`, `MAX_TIMESTAMP_SECONDS`, `MAX_DURATION_SECONDS` | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] |
| Format as it crosses the ABI (92 bytes) [^layout] | `RawNumFormat` (`resolve()`, `to_le_bytes`) | `Interop.RawNumFormat` (`NumFormat.ToRaw()`) | `NativeValues.writeFormat` (`FORMAT_BYTES`) | `RawNumFormat` (`NumFormat.Raw()`) | `Interop.RawNumFormat` (`rawFormat(_:)`) | `NumFormat::packed()` | `NumFormat#packed` | `NumFormat.packed` |
| Day periods as they cross the ABI (40 bytes) [^layout] | `RawDayPeriods` (`resolve()`) | `Interop.RawDayPeriods` (`DateFormat.ToRaw()`) | `NativeValues.writeDayPeriods` (`DAY_PERIODS_BYTES`) | `RawDayPeriods` (`DateFormat.Raw()`) | `Interop.RawDayPeriods` (`rawDayPeriods(_:)`) | `DayPeriods::packed()` | — (Fiddle scratch) | — (calls the Rust API) |

## Inputs

| Concept | Rust | C# | Java | Go | Swift | PHP | Ruby | Python |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Text | `impl AsRef<[u8]>` (`&str`, `String`) | `ReadOnlySpan<char>` (a `string` converts) | `String` | `string` (the `Text` constraint) | `String` | `string` | `String`, any encoding (transcoded to UTF-8) | `str` |
| UTF-8 bytes | `impl AsRef<[u8]>` (`&[u8]`, `Vec<u8>`) | `ReadOnlySpan<byte>` overload | `byte[]` and `MemorySegment` overloads | `[]byte` (the `Text` constraint) | `[UInt8]` and `UnsafeRawBufferPointer` overloads | — (a `string` is already bytes) | a binary (`ASCII-8BIT`) `String` | `bytes` |
| Typed-door number | `f64` | `double` | `double` | `float64` | `Double` | `float` | `Float()`-convertible: `Integer`, `Float`, `Rational` | `float` |
| Fault span units | UTF-8 bytes | chars for `ReadOnlySpan<char>`, bytes for `ReadOnlySpan<byte>` | chars for `String`, bytes otherwise | bytes | bytes | bytes | characters, or bytes for a binary `String` | code points for `str`, bytes for `bytes` |

## Values

What a successful door returns. Ranges and fidelity follow each binding's README.

| Door | Rust | C# | Java | Go | Swift | PHP | Ruby | Python |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| boolean | `bool` | `bool` | `Boolean` | `bool` | `Bool` | `bool` | `true`/`false` | `bool` |
| char | `char` | `char` [^utf16-char] | `Character` [^utf16-char] | `rune` | `Unicode.Scalar` | the scalar's UTF-8 `string` | one-character UTF-8 `String` | one-character `str` |
| i8, i16, i32, i64 | `i8` … `i64` | `sbyte`, `short`, `int`, `long` | `Byte`, `Short`, `Integer`, `Long` | `int8` … `int64` | `Int8` … `Int64` | `int` | `Integer` | `int` |
| u8, u16, u32, u64 | `u8` … `u64` | `byte`, `ushort`, `uint`, `ulong` | `Integer`, `Integer`, `Long`, `Long` (u64 as the two's-complement bit pattern) | `uint8` … `uint64` | `UInt8` … `UInt64` | `int` (u64 as the two's-complement bit pattern) | `Integer` (u64 the true value) | `int` (u64 the true value) |
| f32, f64 | `f32`, `f64` | `float`, `double` | `Float`, `Double` | `float32`, `float64` | `Float`, `Double` | `float` | `Float` | `float` |
| decimal, decimal from `f64` | `Decimal` {`lo`, `hi`, `scale`, `negative`} | `decimal` | `BigDecimal` | `Decimal` {`Lo`, `Hi`, `Scale`, `Negative`} | Foundation `Decimal` | `HyperCast\Decimal` {`magnitude` (string), `scale`, `negative`} | `HyperCast::Decimal` {`magnitude`, `scale`, `negative`} | `decimal.Decimal` |
| uuid | `[u8; 16]`, RFC 9562 order | `Guid` | `UUID` | `uuid.UUID` (`github.com/google/uuid`) | `UUID` | lowercase hyphenated `string` (`uuidBytes`: 16-byte `string`) | lowercase hyphenated `String` | `uuid.UUID` |
| timestamp, unix, excel serial (text) | `Timestamp` {`seconds`, `nanos`} | `DateTimeOffset` | `Instant` | UTC `time.Time` | `Date` | UTC `DateTimeImmutable` (microseconds) | UTC `Time` | UTC-aware `datetime.datetime` (microseconds) |
| date, date ordered | `Date` {`year`, `month`, `day`} | `DateOnly` | `LocalDate` | `Date` {`Year`, `Month`, `Day`} | `DateComponents` | `DateTimeImmutable` at UTC midnight | `Date` | `datetime.date` |
| datetime, excel serial from `f64` | `CivilDateTime` {`date`, `nanos_of_day`} | `DateTime`, `DateTimeKind.Unspecified` | `LocalDateTime` | `CivilDateTime` {`Date`, `TimeOfDay`} | `DateComponents` | `DateTimeImmutable` labeled UTC | `DateTime` at offset `+00:00` | naive `datetime.datetime` |
| time, excel time | `u64` nanoseconds since midnight | `TimeOnly` | `LocalTime` | `time.Duration` since midnight | `DateComponents` | `int` nanoseconds since midnight | `Integer` nanoseconds since midnight | `datetime.time` |
| duration, excel duration | `Duration` {`seconds`, `nanos`} | `TimeSpan` | `java.time.Duration` | `Duration` {`Seconds`, `Nanos`} (`AsDuration()`) | `Duration` | `HyperCast\Duration` {`seconds`, `nanos`} | `Rational` seconds | `datetime.timedelta` |

C#'s `DateTimeOffset`, `TimeOnly` and `TimeSpan` resolve to 100 ns ticks; PHP and Python
truncate instants to microseconds. Each binding's README states its own fidelity.

## Verdict

| Concept | Rust | C# | Java | Go | Swift | PHP | Ruby | Python |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Verdict type | `Result<T, Fault>` | `Verdict<T>` (`[Union]` record struct) | `Verdict<T>` (sealed interface) | `(T, *Fault)` | `Verdict<T>` (enum) | `Success\|Fault` | `Success` or `Fault` (no named union) | `Verdict[T]` (`Success[T] \| Fault`) |
| Success case | `Ok(value)` | `Success<T>` (`Value`) | `Success<T>` (`value()`) | a `nil` `*Fault` | `.success(value)` | `Success` (`value`) | `HyperCast::Success` (`value`) | `Success` (`value`) |
| Fault case | `Err(Fault)` | `Fault` | `Fault<T>` | a non-`nil` `*Fault` (implements `error`) | `.fault(Fault)` | `Fault` | `HyperCast::Fault` | `Fault` |
| Typed-door verdict | `Result<T, Reason>` [^typed-reason] | `Fault` with `Offset` and `Length` 0 | `Fault` with `offset()` and `length()` 0 | `Fault` with `Offset` and `Length` 0 | `Fault` with `offset` and `length` 0 | `Fault` with `offset` and `length` 0 | `Fault` with `offset` and `length` 0 | `Fault` with `offset` and `length` 0 |
| Fault reason | `reason` | `Reason` | `reason()` | `Reason` | `reason` | `reason` | `reason` | `reason` |
| Fault offset | `offset` (`u32`) | `Offset` (`int`) | `offset()` (`int`) | `Offset` (`int`) | `offset` (`Int`) | `offset` (`int`) | `offset` | `offset` |
| Fault length | `len` (`u32`) | `Length` (`int`) | `length()` (`int`) | `Length` (`int`) | `length` (`Int`) | `length` (`int`) | `length` | `length` |
| Failure reasons | `Reason` | `CastFailure` [^unspecified] | `CastFailure` | `CastFailure` | `CastFailure` | `CastFailure` | Symbols (`HyperCast::REASONS` maps the codes) | `CastFailure` |
| Empty | `Reason::Empty` | `CastFailure.Empty` | `CastFailure.EMPTY` | `Empty` | `.empty` | `CastFailure::Empty` | `:empty` | `CastFailure.EMPTY` |
| Malformed | `Reason::Malformed` | `CastFailure.Malformed` | `CastFailure.MALFORMED` | `Malformed` | `.malformed` | `CastFailure::Malformed` | `:malformed` | `CastFailure.MALFORMED` |
| Out of range | `Reason::OutOfRange` | `CastFailure.OutOfRange` | `CastFailure.OUT_OF_RANGE` | `OutOfRange` | `.outOfRange` | `CastFailure::OutOfRange` | `:out_of_range` | `CastFailure.OUT_OF_RANGE` |
| Optional (Empty as absent) | `optional(verdict)` → `Result<Option<T>, Fault>` | `Cast.Optional(verdict)` → `Verdict<T>?` | `Cast.optional(verdict)` → `Optional<Verdict<T>>` | — | `Cast.optional(_:)` → `Verdict<T>?` | `Cast::optional($verdict)` → `Success\|Fault\|null` | `HyperCast.optional(verdict)` → `nil` | `optional(verdict)` → `None` |

## NumFormat

| Concept | Rust | C# | Java | Go | Swift | PHP | Ruby | Python |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Type | `NumFormat` | `NumFormat` (record struct) | `NumFormat` (record) | `NumFormat` (struct) | `NumFormat` (struct) | `NumFormat` (readonly class) | `HyperCast::NumFormat` (`Data`) | `NumFormat` |
| Construct | `NumFormat::new(decimal_sep, group_sep, flags)`, `with_separators(..)` | `new NumFormat(decimalSeparator, groupSeparator, styles[, currencySymbol])` with init-only properties | `new NumFormat(decimalSeparator, groupSeparator, styles[, currencySymbol])`, `with*` | `NumFormat{DecimalSep, GroupSep, Styles, Currency, ...}` literal | `NumFormat(decimalSeparator:groupSeparator:styles:currencySymbol:...)` | `new NumFormat($decimalSep, $groupSep, $flags, $currency = '', ...)` (named arguments) | `NumFormat.new(decimal_sep:, group_sep:, flags:, currency: "", ...)`, `with` | `NumFormat(decimal_sep, group_sep, flags, currency="", *, ...)` |
| Invariant | `NumFormat::INVARIANT` (also `Default`) | `NumFormat.Invariant` | `NumFormat.INVARIANT` | `Invariant` | `NumFormat.invariant` | `NumFormat::invariant()` | `NumFormat::INVARIANT` | `NumFormat.INVARIANT` |
| Detect | `NumFormat::DETECT` | `NumFormat.Detect` | `NumFormat.DETECT` | `Detect` | `NumFormat.detect` | `NumFormat::detect()` | `NumFormat::DETECT` | `NumFormat.DETECT` |
| Decimal separator (1–2 chars) [^symbol] | `decimal_sep` (`Symbol<8>`) | `DecimalSeparator` (`string`) | `decimalSeparator()` (`String`) | `DecimalSep` (`string`) | `decimalSeparator` (`String`) | `decimalSep` (`string`) | `decimal_sep` | `decimal_sep` |
| Group separator (1–2 chars) [^symbol] | `group_sep` (`Symbol<8>`) | `GroupSeparator` (`string`) | `groupSeparator()` (`String`) | `GroupSep` (`string`) | `groupSeparator` (`String`) | `groupSep` (`string`) | `group_sep` | `group_sep` |
| Currency separators (read once the declared symbol is found; empty keeps the number pair) | `currency_decimal_sep`, `currency_group_sep` (`with_currency_separators`) | `CurrencyDecimalSeparator`, `CurrencyGroupSeparator` | `currencyDecimalSeparator()`, `currencyGroupSeparator()` | `CurrencyDecimalSep`, `CurrencyGroupSep` | `currencyDecimalSeparator`, `currencyGroupSeparator` | `currencyDecimalSep`, `currencyGroupSep` | `currency_decimal_sep`, `currency_group_sep` | `currency_decimal_sep`, `currency_group_sep` |
| Declared minus and plus signs (read beside ASCII) | `minus_sign`, `plus_sign` (`with_signs`) | `MinusSign`, `PlusSign` | `minusSign()`, `plusSign()` | `MinusSign`, `PlusSign` | `minusSign`, `plusSign` | `minusSign`, `plusSign` | `minus_sign`, `plus_sign` | `minus_sign`, `plus_sign` |
| Declared percent symbol (read beside `%`) | `percent` (`with_percent`) | `PercentSymbol` | `percentSymbol()` | `Percent` | `percentSymbol` | `percent` | `percent` | `percent` |
| Native digits (the zero of a digit block, read beside ASCII) | `digits` (`NativeDigits`, `with_digits`) | `DigitZero` (`Rune?`) | `digitZero()` (`int` code point, 0 = none) | `DigitZero` (`rune`, 0 = none) | `digitZero` (`Unicode.Scalar?`) | `digitZero` (`string`, `''` = none) | `digit_zero` (`""` = none) | `digit_zero` (`""` = none) |
| Styles field | `flags` (`u32`) | `Styles` (`NumStyles`) | `styles()` (`int`) | `Styles` (`NumStyles`) | `styles` (`NumStyles`) | `flags` (`int`) | `flags` | `flags` |
| Currency symbol field | `currency` (`CurrencySymbol`) | `CurrencySymbol` (`string`) | `currencySymbol()` (`String`) | `Currency` (`string`) | `currencySymbol` (`String`) | `currency` (`string`) | `currency` | `currency` |
| Declare a symbol | `.with_currency(symbol)`, `CurrencySymbol::new(&str)` → `Option<CurrencySymbol>` | constructor argument | constructor argument | `Currency:` field | `currencySymbol:` argument | `$currency` argument | `currency:` keyword | `currency` argument |
| No symbol | `CurrencySymbol::NONE` | `""` | `""` | `""` | `""` | `''` | `""` | `""` |
| Symbol limit (16 UTF-8 bytes) | `CurrencySymbol::MAX_BYTES` | `NumFormat.MaxCurrencyBytes` | — (private) | `CurrencyMaxBytes` | `Interop.currencyMaxBytes` | `NumFormat::CURRENCY_MAX_BYTES` | `NumFormat::CURRENCY_MAX_BYTES` | — |
| Separator and sign limit (8 UTF-8 bytes) | `Symbol::<8>::MAX_BYTES` [^symbol-n] | `NumFormat.MaxSymbolBytes` | — (private) | `SymbolMaxBytes` | `Interop.symbolMaxBytes` | `NumFormat::SYMBOL_MAX_BYTES` | `NumFormat::SYMBOL_MAX_BYTES` | — |
| Platform culture bridge (fills every field above) [^bridge-fields] | — [^no-bridge] | `NumFormat.From(CultureInfo)`, `From(IFormatProvider)`, `From(NumberFormatInfo)` | `NumFormat.from(Locale)`, `from(DecimalFormat)` | — [^no-bridge] | `NumFormat.from(locale:)`, `from(formatter:)` | `NumFormat::fromLocaleconv(?array $conv = null)`, `NumFormat::fromNumberFormatter(\NumberFormatter)` | `NumFormat.from_i18n(hash, currency_format:)` (I18n `number.format` and `number.currency.format` Hashes) [^no-bridge] | `NumFormat.from_localeconv(conv=None)` |
| A numeric door taking the platform's own format [^platform-format] | — | an `IFormatProvider? provider` overload of every numeric door, `Numeric<T>` and `Scalar<T>`; `null` is the current culture | a `Locale` overload of every numeric door | — [^no-bridge] | a `locale:` overload of every numeric door, `numeric` and `scalar` | `NumFormat\|\NumberFormatter` as the format argument | `separator:`, `delimiter:`, `unit:` keywords on every numeric door (`NumFormat#override`) | — [^python-format] |

### Styles

| Flag | Rust | C# | Java | Go | Swift | PHP | Ruby | Python |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Flags type | `u32` constants on `NumFormat` | `NumStyles` (`[Flags]` enum) | `int` constants on `NumFormat` | `NumStyles` | `NumStyles` (`OptionSet`) | `int` constants on `NumFormat` | `Integer` constants on `HyperCast` | `int` class attributes on `NumFormat` |
| None | `0` | `NumStyles.None` | `0` | `0` | `[]` | `0` | `0` | `0` |
| Grouping (`1`) | `NumFormat::GROUPING` | `NumStyles.Grouping` | `NumFormat.STYLE_GROUPING` | `Grouping` | `.grouping` | `NumFormat::GROUPING` | `HyperCast::GROUPING` | `NumFormat.GROUPING` |
| Parentheses (`1 << 1`) | `NumFormat::PARENS` | `NumStyles.Parentheses` | `NumFormat.STYLE_PARENTHESES` | `Parentheses` | `.parentheses` | `NumFormat::PARENTHESES` | `HyperCast::PARENTHESES` | `NumFormat.PARENTHESES` |
| Exponent (`1 << 2`) | `NumFormat::EXPONENT` | `NumStyles.Exponent` | `NumFormat.STYLE_EXPONENT` | `Exponent` | `.exponent` | `NumFormat::EXPONENT` | `HyperCast::EXPONENT` | `NumFormat.EXPONENT` |
| Radix prefixes (`1 << 3`) | `NumFormat::RADIX_PREFIX` | `NumStyles.RadixPrefixes` | `NumFormat.STYLE_RADIX_PREFIXES` | `RadixPrefixes` | `.radixPrefixes` | `NumFormat::RADIX_PREFIXES` | `HyperCast::RADIX_PREFIXES` | `NumFormat.RADIX_PREFIXES` |
| Percent (`1 << 4`) | `NumFormat::PERCENT` | `NumStyles.Percent` | `NumFormat.STYLE_PERCENT` | `Percent` | `.percent` | `NumFormat::PERCENT` | `HyperCast::PERCENT` | `NumFormat.PERCENT` |
| Separator detect (`1 << 5`) | `NumFormat::SEPARATOR_DETECT` | `NumStyles.SeparatorDetect` | `NumFormat.STYLE_SEPARATOR_DETECT` | `SeparatorDetect` | `.separatorDetect` | `NumFormat::SEPARATOR_DETECT` | `HyperCast::SEPARATOR_DETECT` | `NumFormat.SEPARATOR_DETECT` |
| Currency (`1 << 6`) | `NumFormat::CURRENCY` | `NumStyles.Currency` | `NumFormat.STYLE_CURRENCY` | `CurrencySymbol` | `.currency` | `NumFormat::CURRENCY` | `HyperCast::CURRENCY` | `NumFormat.CURRENCY` |
| Trailing sign (`1 << 7`; not in All; set by each bridge from the culture's negative pattern) | `NumFormat::TRAILING_SIGN` | `NumStyles.TrailingSign` | `NumFormat.STYLE_TRAILING_SIGN` | `TrailingSign` | `.trailingSign` | `NumFormat::TRAILING_SIGN` | `HyperCast::TRAILING_SIGN` | `NumFormat.TRAILING_SIGN` |
| All leniences (not separator detect) | `NumFormat::ALL` | `NumStyles.All` | `NumFormat.STYLE_ALL` | `AllStyles` | `.all` | `NumFormat::ALL` | `HyperCast::ALL_STYLES` | `NumFormat.ALL` |

## Declared enums

| Concept | Rust | C# | Java | Go | Swift | PHP | Ruby | Python |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Unix precision | `UnixPrecision` | `UnixPrecision` [^unspecified] | `UnixPrecision` | `UnixPrecision` | `UnixPrecision` | `UnixPrecision` | Symbols, keys of `HyperCast::UNIX_PRECISIONS` | `UnixPrecision` |
| Seconds (`1`) | `UnixPrecision::Seconds` | `UnixPrecision.Seconds` | `UnixPrecision.SECONDS` | `Seconds` | `.seconds` | `UnixPrecision::Seconds` | `:seconds` | `UnixPrecision.SECONDS` |
| Milliseconds (`2`) | `UnixPrecision::Millis` | `UnixPrecision.Milliseconds` | `UnixPrecision.MILLISECONDS` | `Milliseconds` | `.milliseconds` | `UnixPrecision::Milliseconds` | `:milliseconds` | `UnixPrecision.MILLISECONDS` |
| Microseconds (`3`) | `UnixPrecision::Micros` | `UnixPrecision.Microseconds` | `UnixPrecision.MICROSECONDS` | `Microseconds` | `.microseconds` | `UnixPrecision::Microseconds` | `:microseconds` | `UnixPrecision.MICROSECONDS` |
| Nanoseconds (`4`) | `UnixPrecision::Nanos` | `UnixPrecision.Nanoseconds` | `UnixPrecision.NANOSECONDS` | `Nanoseconds` | `.nanoseconds` | `UnixPrecision::Nanoseconds` | `:nanoseconds` | `UnixPrecision.NANOSECONDS` |
| Excel epoch | `ExcelEpoch` | `ExcelEpoch` [^unspecified] | `ExcelEpoch` | `ExcelEpoch` | `ExcelEpoch` | `ExcelEpoch` | Symbols, keys of `HyperCast::EXCEL_EPOCHS` | `ExcelEpoch` |
| 1900 system (`1`) | `ExcelEpoch::Y1900` | `ExcelEpoch.Y1900` | `ExcelEpoch.Y1900` | `Excel1900` | `.y1900` | `ExcelEpoch::Y1900` | `:y1900` | `ExcelEpoch.Y1900` |
| 1904 system (`2`) | `ExcelEpoch::Y1904` | `ExcelEpoch.Y1904` | `ExcelEpoch.Y1904` | `Excel1904` | `.y1904` | `ExcelEpoch::Y1904` | `:y1904` | `ExcelEpoch.Y1904` |
| Date order | `DateOrder` | `DateOrder` [^unspecified] | `DateOrder` | `DateOrder` | `DateOrder` | `DateOrder` | Symbols, keys of `HyperCast::DATE_ORDERS` | `DateOrder` |
| Year, month, day (`1`) | `DateOrder::YearMonthDay` | `DateOrder.YearMonthDay` | `DateOrder.YEAR_MONTH_DAY` | `YearMonthDay` | `.yearMonthDay` | `DateOrder::Ymd` | `:year_month_day` | `DateOrder.YEAR_MONTH_DAY` |
| Month, day, year (`2`) | `DateOrder::MonthDayYear` | `DateOrder.MonthDayYear` | `DateOrder.MONTH_DAY_YEAR` | `MonthDayYear` | `.monthDayYear` | `DateOrder::Mdy` | `:month_day_year` | `DateOrder.MONTH_DAY_YEAR` |
| Day, month, year (`3`) | `DateOrder::DayMonthYear` | `DateOrder.DayMonthYear` | `DateOrder.DAY_MONTH_YEAR` | `DayMonthYear` | `.dayMonthYear` | `DateOrder::Dmy` | `:day_month_year` | `DateOrder.DAY_MONTH_YEAR` |
| Date order from the platform locale | — [^no-bridge] | `DateOrders.From(CultureInfo)` | `DateOrder.from(Locale)` | — [^no-bridge] | `DateOrder.from(locale:)` | — [^no-bridge] | — [^no-bridge] | — |
| Day-period designators beside the order (the culture's `AM`/`PM`, read beside English) [^periods] | `DayPeriods::new(am, pm)` with `cast_datetime_with` | `new DateFormat(order, amDesignator, pmDesignator)` with `Cast.DateTime` | `DateTimeFormat(order, am, pm)` with `Cast.dateTime` | `DateFormat{Order, AM, PM}` with `DateTimeWith` | `DateFormat(order:amSymbol:pmSymbol:)` with `Cast.dateTime(_:format:)` | `new DayPeriods($am, $pm)`, third argument of `Cast::datetime` | `am:`, `pm:` keywords on `HyperCast.datetime` | `am=`, `pm=` keyword-only on `cast_datetime` |
| Day periods from the platform locale | — [^no-bridge] | `DateFormat.From(CultureInfo)`, `From(IFormatProvider)`, `From(DateTimeFormatInfo)`, and an `IFormatProvider?` overload of `Cast.DateTime` | `DateTimeFormat.from(Locale)`, and a `Locale` overload of `Cast.dateTime` | — [^no-bridge] | `DateFormat.from(locale:)`, and `Cast.dateTime(_:locale:)` | — [^no-bridge] | — [^no-bridge] | — |

## The native library

| Concept | Rust | C# | Java | Go | Swift | PHP | Ruby | Python |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Version probe (`hypercast_version`) | `hypercast_version()` → `u32`, packed `major << 16 \| minor << 8 \| patch` | `Cast.NativeVersion` → `Version?` | `Cast.nativeVersion()` → `String` | `NativeVersion()` → `string` | `Cast.nativeVersion()` → `String` | `Cast::nativeVersion()` → `string` | `HyperCast.native_version` → `String` | `native_version()` → `str` |
| Availability, never throws | — [^rust-probe] | `Cast.IsAvailable` | `Cast.isAvailable()` | `Available()`, always `true` [^linked] | `Cast.isAvailable`, always `true` [^linked] | `Cast::isAvailable()` | `HyperCast.available?` | — |
| Load failure | — | — | — | `LoadError()`, always `nil` [^linked] | — | — | — | — |
| Backend in use | — | — | — | — | — | — | `HyperCast::BACKEND` → `:native` or `:fiddle` | `BACKEND`, always `"native"` |

Rust is the crate's own API (the `rust/src/lib.rs` re-exports). The other names are
relative to C# namespace `HyperCast`, Java package `io.github.skunkwerkx.hypercast`, Go
package `hypercast`, Swift module `HyperCast`, PHP namespace `HyperCast`, Ruby module
`HyperCast` and Python package `hypercast`. `—` means no counterpart; a footnote says why
when the absence is deliberate.

[^go-names]: Go names a door after what it returns where Go or the package already owns
    the ABI word: `Exact` and `Span` because the result types are `Decimal` and `Duration`,
    and `DateOnly`, `DateOnlyOrdered` and `TimeOfDay` for what `cast_date`,
    `cast_date_ordered` and `cast_time` return (`go/cast.go` package doc).

[^folded]: PHP, Ruby and Python fold `cast_date` and `cast_date_ordered` into one entry
    point whose order argument is optional (`?DateOrder $order = null`, `order = nil`,
    `order: DateOrder | None = None`): absent, the door is strict ISO `yyyy-MM-dd`
    (`cast_date`); present, it is `cast_date_ordered`. C#, Java and Swift keep one name and
    overload it; Rust and Go keep two names.

[^typed-reason]: Rust's typed doors return a bare `Reason`, since a double has no text to
    index. The C ABI exports and every binding return their ordinary fault with an empty
    span (offset 0, length 0) instead (CHANGELOG, 0.7.0).

[^excel-serial-name]: The Rust twin of `cast_excel_serial` for a held `f64` is
    `excel_serial`; the export is `cast_excel_serial_from_f64` and the bindings follow the
    export's `FromDouble`/`FromFloat64`/`from_float` naming.

[^swift-throws]: Swift's doors and `nativeVersion()` are `throws` only so call sites written
    when macOS and Windows loaded a shared library keep compiling; the core is linked in and
    they do not throw (`swift/Sources/HyperCast/Cast.swift`).

[^generic]: Not in Java, which has no generics over primitives, nor in the dynamic
    bindings, where the per-width doors are the idiom (`docs/roadmap.md`, CHANGELOG). Rust
    callers have the concrete doors.

[^scalar]: One entry point that picks the door from the requested type, for a caller
    holding a generic `T` (#21). `format` is read only by the numeric doors. Each binding
    covers the types that name exactly one door: C#'s table covers `bool`, the numerics,
    `Guid`, `DateOnly`, `TimeOnly`, `DateTimeOffset`, `TimeSpan`, `char` and `DateTime`.
    `DateTime` reads RFC 3339 and comes out as `.UtcDateTime` (`Kind = Utc`), not through
    the civil `Cast.DateTime` door, which needs a declared order. Go leaves out `rune`
    (it is `int32`, the integer door), `time.Duration` (what `TimeOfDay` returns) and
    `CivilDateTime` (needs an order), and the union constraint makes any of them a compile
    error. Swift leaves out `DateComponents`, which the date, time and date-time doors
    all return.

[^java-char]: `char` is a Java keyword, so the door is `character`.

[^utf16-char]: A UTF-16 `char` holds no scalar past `U+FFFF`, so C# and Java return such a
    scalar as `OutOfRange`, over the input less its ASCII whitespace edges, in the
    caller's units. A string of exactly one UTF-16 unit, including a lone surrogate, is
    returned as it is, without calling the core.

[^rust-only]: Only four typed doors cross the C ABI, the ones a workbook reader needs most
    (CHANGELOG, 0.7.0); the rest of the typed family and the conversion helpers are
    Rust API only.

[^unspecified]: C#'s enums also carry `Unspecified = 0`, the CLR default, which no cast
    produces and every door rejects.

[^no-bridge]: The Rust core carries no culture data. Go and Ruby have no locale data in
    their standard libraries (`docs/roadmap.md`). PHP's standard library has no per-locale
    date pattern, so it bridges `NumFormat` from `localeconv()` but has no `DateOrder`
    bridge (`php/src/DateOrder.php`). The same holds for day-period designators: no locale
    data in Go, Ruby or PHP's standard library names a culture's `AM`/`PM`, and Python's
    `localeconv()` carries none, so those four declare them by hand, as they do the order.

[^layout]: Since 0.9.0 the format crosses the C ABI as a size-tagged, 92-byte layout, the
    `#[repr(C)]` `NumFormat` itself behind a size word, read in place; day periods cross as a
    40-byte `RawDayPeriods` after the date order, null for English only. `rust/src/abi.rs`
    has the offset table every binding restates and pins in its interop tests.

[^symbol]: One or two characters, up to 8 UTF-8 bytes, containing no ASCII digit; the two
    must differ. A format read across the ABI that breaks a rule is a contract violation,
    which each binding reports as its platform's argument error before the call.

[^symbol-n]: Rust's generic `Symbol<N>` accepts at most `min(N, 255)` UTF-8 bytes, because
    its length crosses the ABI as one byte. The 8- and 16-byte symbols every binding carries
    are well inside that.

[^bridge-fields]: Each bridge declares a symbol only where the platform's own differs from
    what the core reads anyway (ASCII `-`, `+`, `%`, `0`, and currency separators equal to
    the number ones); the reading is identical either way, and the check stays on its fast
    path. The C#, Java and Swift bridges never throw on their platform's data: a symbol or
    designator the core would refuse (over its bytes, an ASCII digit in it, whitespace where
    none may be) is left undeclared, and a culture whose two separators coincide groups with
    the other ASCII one. PHP's `fromNumberFormatter` raises `InvalidArgumentException` for a
    symbol it cannot carry, as its constructor does, by its own documented decision. Trailing-sign lenience comes from the culture's negative pattern: .NET's
    `NumberNegativePattern`, ICU's negative suffix with an empty prefix (Java, Swift, PHP's
    `NumberFormatter`), `n_sign_posn` 2 or 4 from `localeconv()` (PHP, Python), and a
    `negative_format` with the sign after `%n` in Rails.

[^periods]: A declared designator is read exactly as spelled, ahead of the clock or after
    it, beside the English `AM`/`PM`, which are read case-insensitively. Each is 1 to 16
    UTF-8 bytes with no ASCII digit and no ASCII whitespace at either end (`a. m.` is fine).
    The plain `DateOrder` door is unchanged in every binding and reads English only.

[^platform-format]: So a caller holding its platform's own culture or format object passes it
    straight through instead of mapping it first; each maps to the core's `NumFormat` with
    every lenience on, as the bridge above does, and a `NumFormat` remains the way to declare
    anything stricter. The shapes follow each platform: C# matches `TryParse(s, provider)`
    and `ISpanParsable<T>` (a `null` provider is the current culture, as there, and the
    overloads rank below the `NumFormat` ones, so `default` still means `NumFormat`); Java
    and Swift take a non-null `Locale` (pass `Locale.getDefault(Locale.Category.FORMAT)` or
    `.current`), each cached per locale; PHP takes intl's per-object `\NumberFormatter`
    (intl stays optional); Ruby takes Rails' `number.format` vocabulary as keywords. Swift's
    cache is keyed by locale identifier for an explicit locale, and skipped for `.current`
    and `.autoupdatingCurrent`, so a user's own number-format overrides on Apple platforms
    are honored; Java's Native Image carries only the locales named at build time
    (`-H:IncludeLocales`). In every binding, and through the bridges above too, everything
    the culture declares carries over since 0.9.0 — its signs, a trailing-sign pattern, the
    separators of a currency amount, its percent symbol, its digits — each read beside the
    ASCII default, never instead of it; the
    [culture fidelity sweep](roadmap.md#culture-fidelity--the-sweep-built-090) has every row.

[^python-format]: Python's standard library has no per-object locale: `locale` is
    process-global, which `NumFormat.from_localeconv` already reads, so the doors keep taking
    a `NumFormat`.

[^rust-probe]: The crate is the core, so there is nothing to load and nothing to probe;
    `hypercast_version()` is re-exported for a crate that fronts its own C ABI.

[^linked]: Go and Swift link the core in statically, so it is always there; the probes are
    kept for code written against the load probe every binding carries.
