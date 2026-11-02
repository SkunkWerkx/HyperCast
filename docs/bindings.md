# Binding parity

One row per concept, one column per language, each cell the public spelling of that concept
in that package, so drift between the Rust template and the ten bindings shows at a glance.
The C ABI itself is the Export column of [Doors](#doors) and `include/hypercast.h`.
A change that adds a door or a concept adds its row here in the same change.

## Doors

The 31 `cast_*` exports of `rust/src/ffi.rs`, in export order by group. Every binding door
also takes the text first (or the `f64` for the four typed doors) and returns that
language's verdict (see [Verdict](#verdict)); the checked doors return the checked verdict
of [Constraints](#constraints) instead.

| Export | Rust | C# | Java | Go | Swift | PHP | Ruby | Python | TypeScript | R | C++ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `cast_bool` | `cast_bool` | `Cast.Boolean` | `Cast.bool` | `Bool` | `Cast.bool(_:)` | `Cast::bool` | `HyperCast.bool` | `cast_bool` | `castBool` | `cast_bool` | `cast_bool` |
| `cast_char` | `cast_char` | `Cast.Char` | `Cast.character` [^java-char] | `Char` | `Cast.char(_:)` | `Cast::char` | `HyperCast.char` | `cast_char` | `castChar` | `cast_char` | `cast_char` |
| `cast_i8` | `cast_i8` | `Cast.SByte` | `Cast.i8` | `I8` | `Cast.i8(_:format:)` | `Cast::i8` | `HyperCast.i8` | `cast_i8` | `castI8` | `cast_i8` | `cast_i8` |
| `cast_i16` | `cast_i16` | `Cast.Int16` | `Cast.i16` | `I16` | `Cast.i16(_:format:)` | `Cast::i16` | `HyperCast.i16` | `cast_i16` | `castI16` | `cast_i16` | `cast_i16` |
| `cast_i32` | `cast_i32` | `Cast.Int32` | `Cast.i32` | `I32` | `Cast.i32(_:format:)` | `Cast::i32` | `HyperCast.i32` | `cast_i32` | `castI32` | `cast_i32` | `cast_i32` |
| `cast_i64` | `cast_i64` | `Cast.Int64` | `Cast.i64` | `I64` | `Cast.i64(_:format:)` | `Cast::i64` | `HyperCast.i64` | `cast_i64` | `castI64` | `cast_i64` | `cast_i64` |
| `cast_u8` | `cast_u8` | `Cast.Byte` | `Cast.u8` | `U8` | `Cast.u8(_:format:)` | `Cast::u8` | `HyperCast.u8` | `cast_u8` | `castU8` | `cast_u8` | `cast_u8` |
| `cast_u16` | `cast_u16` | `Cast.UInt16` | `Cast.u16` | `U16` | `Cast.u16(_:format:)` | `Cast::u16` | `HyperCast.u16` | `cast_u16` | `castU16` | `cast_u16` | `cast_u16` |
| `cast_u32` | `cast_u32` | `Cast.UInt32` | `Cast.u32` | `U32` | `Cast.u32(_:format:)` | `Cast::u32` | `HyperCast.u32` | `cast_u32` | `castU32` | `cast_u32` | `cast_u32` |
| `cast_u64` | `cast_u64` | `Cast.UInt64` | `Cast.u64` | `U64` | `Cast.u64(_:format:)` | `Cast::u64` | `HyperCast.u64` | `cast_u64` | `castU64` | `cast_u64` | `cast_u64` |
| `cast_f32` | `cast_f32` | `Cast.Single` | `Cast.f32` | `F32` | `Cast.f32(_:format:)` | `Cast::f32` | `HyperCast.f32` | `cast_f32` | `castF32` | `cast_f32` | `cast_f32` |
| `cast_f64` | `cast_f64` | `Cast.Double` | `Cast.f64` | `F64` | `Cast.f64(_:format:)` | `Cast::f64` | `HyperCast.f64` | `cast_f64` | `castF64` | `cast_f64` | `cast_f64` |
| `cast_decimal` | `cast_decimal` | `Cast.Decimal` | `Cast.decimal` | `Exact` [^go-names] | `Cast.decimal(_:format:)` | `Cast::decimal` | `HyperCast.decimal` | `cast_decimal` | `castDecimal` | `cast_decimal` | `cast_decimal` |
| `cast_uuid` | `cast_uuid` | `Cast.Uuid` | `Cast.uuid` | `Uuid` | `Cast.uuid(_:)` | `Cast::uuid`; `Cast::uuidBytes` for the 16 raw bytes | `HyperCast.uuid` | `cast_uuid` | `castUuid` | `cast_uuid` | `cast_uuid` |
| `cast_timestamp` | `cast_timestamp` | `Cast.Timestamp` | `Cast.timestamp` | `Timestamp` | `Cast.timestamp(_:)` | `Cast::timestamp` | `HyperCast.timestamp` | `cast_timestamp` | `castTimestamp` | `cast_timestamp` | `cast_timestamp` |
| `cast_unix` | `cast_unix(input, precision)` | `Cast.Unix(input, precision)` | `Cast.unix(text, precision)` | `Unix(text, precision)` | `Cast.unix(_:precision:)` | `Cast::unix($text, $precision)` | `HyperCast.unix(text, precision)` | `cast_unix(text, precision)` | `castUnix(text, precision)` | `cast_unix(x, precision)` | `cast_unix(text, precision)` |
| `cast_excel_serial` | `cast_excel_serial(input, epoch)` | `Cast.ExcelSerial(input, epoch)` | `Cast.excelSerial(text, epoch)` | `ExcelSerial(text, epoch)` | `Cast.excelSerial(_:epoch:)` | `Cast::excelSerial($text, $epoch)` | `HyperCast.excel_serial(text, epoch)` | `cast_excel_serial(text, epoch)` | `castExcelSerial(text, epoch)` | `cast_excel_serial(x, epoch)` | `cast_excel_serial(text, epoch)` |
| `cast_date` | `cast_date` | `Cast.Date(input)` | `Cast.date(text)` | `DateOnly` [^go-names] | `Cast.date(_:)` | `Cast::date($text)` [^folded] | `HyperCast.date(text)` [^folded] | `cast_date(text)` [^folded] | `castDate(text)` [^folded] | `cast_date(x)` [^folded] | `cast_date(text)` |
| `cast_date_ordered` | `cast_date_ordered(input, order)` | `Cast.Date(input, order)` (overload) | `Cast.date(text, order)` (overload) | `DateOnlyOrdered(text, order)` | `Cast.date(_:order:)` (overload) | `Cast::date($text, $order)` [^folded] | `HyperCast.date(text, order)` [^folded] | `cast_date(text, order)` [^folded] | `castDate(text, order)` [^folded] | `cast_date(x, order)` [^folded] | `cast_date(text, order)` (overload) |
| `cast_datetime` | `cast_datetime(input, order)` | `Cast.DateTime(input, order)` | `Cast.dateTime(text, order)` | `DateTime(text, order)` | `Cast.dateTime(_:order:)` | `Cast::datetime($text, $order)` | `HyperCast.datetime(text, order)` | `cast_datetime(text, order)` | `castDatetime(text, format)` (a `DateOrder` or a `DateFormat`) | `cast_datetime(x, order, am = "", pm = "")` | `cast_datetime(text, order)`, and an overload taking a `DateFormat` |
| `cast_time` | `cast_time` | `Cast.Time` | `Cast.time` | `TimeOfDay` [^go-names] | `Cast.time(_:)` | `Cast::time` | `HyperCast.time` | `cast_time` | `castTime` | `cast_time` | `cast_time` |
| `cast_duration` | `cast_duration` | `Cast.Duration` | `Cast.duration` | `Span` [^go-names] | `Cast.duration(_:)` | `Cast::duration` | `HyperCast.duration` | `cast_duration` | `castDuration` | `cast_duration` | `cast_duration` |
| `cast_decimal_from_f64` | `decimal_from_f64` [^typed-reason] | `Cast.DecimalFromDouble` | `Cast.decimalFromDouble` | `ExactFromFloat64` | `Cast.decimalFromDouble(_:)` | `Cast::decimalFromFloat` | `HyperCast.decimal_from_float` | `cast_decimal_from_float` | `castDecimalFromNumber` | `cast_decimal_from_double` | `cast_decimal_from_double` |
| `cast_excel_serial_from_f64` | `excel_serial(serial, epoch)` [^typed-reason] [^excel-serial-name] | `Cast.ExcelSerialFromDouble(value, epoch)` | `Cast.excelSerialFromDouble(value, epoch)` | `ExcelSerialFromFloat64(value, epoch)` | `Cast.excelSerialFromDouble(_:epoch:)` | `Cast::excelSerialFromFloat($value, $epoch)` | `HyperCast.excel_serial_from_float(value, epoch)` | `cast_excel_serial_from_float(value, epoch)` | `castExcelSerialFromNumber(value, epoch)` | `cast_excel_serial_from_double(x, epoch)` | `cast_excel_serial_from_double(value, epoch)` |
| `cast_excel_time` | `excel_time` [^typed-reason] | `Cast.ExcelTime` | `Cast.excelTime` | `ExcelTime` | `Cast.excelTime(_:)` | `Cast::excelTime` | `HyperCast.excel_time` | `cast_excel_time` | `castExcelTime` | `cast_excel_time` | `cast_excel_time` |
| `cast_excel_duration` | `excel_duration` [^typed-reason] | `Cast.ExcelDuration` | `Cast.excelDuration` | `ExcelDuration` | `Cast.excelDuration(_:)` | `Cast::excelDuration` | `HyperCast.excel_duration` | `cast_excel_duration` | `castExcelDuration` | `cast_excel_duration` | `cast_excel_duration` |
| `cast_checked` | `cast_checked(input, &format, &constraints)` | `Cast.Int32(input, format, constraints)` … (an overload of each numeric door; `Cast.Numeric<T>`) [^checked-doors] | `Cast.i32(text, format, constraints)` … (an overload of each numeric door, over `String`, `byte[]` and `MemorySegment`) [^checked-doors] | `Check(text, format, constraints)` | `Cast.i32(_:format:constraints:)` … (an overload of each numeric door) [^checked-doors] | `Cast::checked($text, $format, $constraints)` | `HyperCast.checked(text, format, constraints)` | `cast_checked(text, fmt, constraints)` | `castChecked(text, constraints, format?)` | `cast_checked(x, format, constraints)` | `cast_i32(text, format, constraints)` … (an overload of each numeric door) [^checked-doors] |
| `cast_checked_from_f64` | `checked_from_f64(value, &constraints)` | `Cast.Int32FromDouble(value, constraints)` … | `Cast.i32FromDouble(value, constraints)` … | `CheckFromFloat64(value, constraints)` | `Cast.i32FromDouble(_:constraints:)` … | `Cast::checkedFromFloat($value, $constraints)` | `HyperCast.checked_from_float(value, constraints)` | `cast_checked_from_float(value, constraints)` | `castCheckedFromNumber(value, constraints)` | `cast_checked_from_double(x, constraints)` | `cast_i32_from_double(value, constraints)` … |
| `cast_constraints_check` | `check_constraints(door, &raw, entries)` → `Result<(), Refusal>` (`Constraints::numeric` and `::text` run it) | — (inside `Constraints<T>`'s constructor) | — (inside `Constraints.Builder.build()`) | — (inside `NewConstraints`) | — (inside `Constraints.init`) | — (inside `new Constraints`) | `HyperCast.constraints_check(**keywords)` → the code (`Constraints.new` runs it) | `cast_constraints_check(door, **keywords)` → the code (`Constraints(…)` runs it) | `castConstraintsCheck(spec)` → the code (`Constraints.declare` runs it) | — (inside `hypercast_constraints()`) | — (inside `Constraints<T>::declare`) |
| `cast_text` | `cast_text(input)`; checked: `cast_text_checked(input, &constraints)` | `Cast.Text(input)`; checked: `Cast.Text(input, constraints)` [^csharp-text] | `Cast.text(text)`; checked: `Cast.text(text, constraints)` | `String(text)`; checked: `CheckString(text, constraints)` | `Cast.text(_:)`; checked: `Cast.text(_:constraints:)` | `Cast::text($text)`; checked: `Cast::checkedText($text, $constraints)` | `HyperCast.text(text)`; checked: `HyperCast.text(text, constraints)` | `cast_text(text)`; checked: `cast_text(text, constraints)` | `castText(text)`; checked: `castText(text, constraints)` | `cast_text(x)`; checked: `cast_text(x, constraints)` | `cast_text(text)`; checked: `cast_text(text, constraints)` |
| `cast_declared_decimal` | `decimal_for_declaration(text)` → `Result<Decimal, Refusal>` | — (inside `ConstraintValue` and `Constraints<T>`) | — (inside `Constraints.Builder`) | — (inside the `Minimum`, `MultipleOf` and `Enum` rules) | — (inside `ConstraintValue` and `Constraints.init`) | — (inside `new Constraints`) | `HyperCast.declared_decimal(text)` → `Decimal` | `cast_declared_decimal(text)` → `decimal.Decimal` | `castDeclaredDecimal(text)` → `Declaration<string>` | — (inside `hypercast_constraints()`) | `ConstraintValue::decimal()` → `std::optional<Decimal>` |

The integer, real and decimal doors take a `NumFormat` second (`format`, or `fmt` in
Python); it is optional in TypeScript, where leaving it out means `NumFormat.invariant()`, and
required in R and C++. No Swift door throws.

### Doors beyond the ABI

| Concept | Rust | C# | Java | Go | Swift | PHP | Ruby | Python | TypeScript | R | C++ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| One numeric door generic over the target | — | `Cast.Numeric<T>` (`T : struct, INumber<T>`) | — [^generic] | `Numeric[V Number, T Text]` | `Cast.numeric<T: NumericCastTarget>(_:format:)` | — [^generic] | — [^generic] | — [^generic] | — [^generic] | — [^generic] | — [^cpp-generic] |
| One door generic over every target [^scalar] | — | `Cast.Scalar<T>` (`T : struct`; `NotSupportedException` otherwise) | — [^generic] | `Scalar[V ScalarTarget, T Text]` | `Cast.scalar<T: ScalarCastTarget>(_:format:)` | — [^generic] | — [^generic] | — [^generic] | — [^generic] | — [^generic] | — [^cpp-generic] |
| Remaining typed doors (a number already held) | `i8_from_f64` … `u64_from_f64`, `f32_from_f64`, `bool_from_f64`, `unix_from_f64` | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] |
| Shortest digits of an `f64` | `shortest_digits` → `Option<ShortestDigits>` | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] |
| Civil ↔ instant, stated UTC | `CivilDateTime::assume_utc`, `Timestamp::utc_civil` | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] |
| Window constants | `MIN_TIMESTAMP_SECONDS`, `MAX_TIMESTAMP_SECONDS`, `MAX_DURATION_SECONDS` | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] | — [^rust-only] |
| Format as it crosses the ABI (92 bytes) [^layout] | `RawNumFormat` (`resolve()`, `to_le_bytes`) | `Interop.RawNumFormat` (`NumFormat.ToRaw()`) | `NativeValues.writeFormat` (`FORMAT_BYTES`) | `RawNumFormat` (`NumFormat.Raw()`) | `Interop.RawNumFormat` (`rawFormat(_:)`) | `NumFormat::packed()` | `NumFormat#packed` | `NumFormat.packed` | `NumFormat#packed` (`Uint8Array`) | `num_format()$packed` (`raw`) | `HyperCastRawNumFormat` (`NumFormat::raw()`) |
| Day periods as they cross the ABI (40 bytes) [^layout] | `RawDayPeriods` (`resolve()`) | `Interop.RawDayPeriods` (`DateFormat.ToRaw()`) | `NativeValues.writeDayPeriods` (`DAY_PERIODS_BYTES`) | `RawDayPeriods` (`DateFormat.Raw()`) | `Interop.RawDayPeriods` (`rawDayPeriods(_:)`) | `DayPeriods::packed()` | — (Fiddle scratch) | — (calls the Rust API) | — (calls the Rust API) | — (packed inside `cast_datetime`) | `HyperCastRawDayPeriods` (`DateFormat::raw()`) |

## Inputs

| Concept | Rust | C# | Java | Go | Swift | PHP | Ruby | Python | TypeScript | R | C++ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Text | `impl AsRef<[u8]>` (`&str`, `String`) | `ReadOnlySpan<char>` (a `string` converts) | `String` | `string` (the `Text` constraint) | `String` | `string` | `String`, any encoding (transcoded to UTF-8) | `str` | `string` (the `Text` type; `null` and `undefined` are `empty`) | a `character` vector (a factor is read as its levels; `NA` is missing) | `Text`: anything that converts to `std::string_view`, or `std::u8string_view` under C++20 |
| UTF-8 bytes | `impl AsRef<[u8]>` (`&[u8]`, `Vec<u8>`) | `ReadOnlySpan<byte>` overload | `byte[]` and `MemorySegment` overloads | `[]byte` (the `Text` constraint) | `[UInt8]` and `UnsafeRawBufferPointer` overloads | — (a `string` is already bytes) | a binary (`ASCII-8BIT`) `String` | `bytes` | `Uint8Array` (a `Buffer` is one) | a list of `raw` vectors (a bare `raw` vector is one input; `NULL` is missing) | `Text(const std::uint8_t *data, std::size_t size)` |
| Typed-door number | `f64` | `double` | `double` | `float64` | `Double` | `float` | `Float()`-convertible: `Integer`, `Float`, `Rational` | `float` | `number` | a numeric vector (`NA` is missing) | `double` |
| Fault span units | UTF-8 bytes | chars for `ReadOnlySpan<char>`, bytes for `ReadOnlySpan<byte>` | chars for `String`, bytes otherwise | bytes | bytes | bytes | characters, or bytes for a binary `String` | code points for `str`, bytes for `bytes` | UTF-16 code units for a `string`, bytes for a `Uint8Array` | characters for a character vector, bytes for raw vectors, a `"bytes"` string, or an unmarked string in the C locale | bytes |

## Values

What a successful door returns. Ranges and fidelity follow each binding's README.

| Door | Rust | C# | Java | Go | Swift | PHP | Ruby | Python | TypeScript | R | C++ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| boolean | `bool` | `bool` | `Boolean` | `bool` | `Bool` | `bool` | `true`/`false` | `bool` | `boolean` | logical | `bool` |
| char | `char` | `char` [^utf16-char] | `Character` [^utf16-char] | `rune` | `Unicode.Scalar` | the scalar's UTF-8 `string` | one-character UTF-8 `String` | one-character `str` | one-code-point `string` | one-character string [^r-nul] | `char32_t` |
| i8, i16, i32, i64 | `i8` … `i64` | `sbyte`, `short`, `int`, `long` | `Byte`, `Short`, `Integer`, `Long` | `int8` … `int64` | `Int8` … `Int64` | `int` | `Integer` | `int` | `number`, `number`, `number`, `bigint` | integer, integer, double, character (the exact decimal) | `std::int8_t` … `std::int64_t` |
| u8, u16, u32, u64 | `u8` … `u64` | `byte`, `ushort`, `uint`, `ulong` | `Integer`, `Integer`, `Long`, `Long` (u64 as the two's-complement bit pattern) | `uint8` … `uint64` | `UInt8` … `UInt64` | `int` (u64 as the two's-complement bit pattern) | `Integer` (u64 the true value) | `int` (u64 the true value) | `number`, `number`, `number`, `bigint` (u64 the true value) | integer, integer, double, character (u64 the true value as its exact decimal) | `std::uint8_t` … `std::uint64_t` |
| f32, f64 | `f32`, `f64` | `float`, `double` | `Float`, `Double` | `float32`, `float64` | `Float`, `Double` | `float` | `Float` | `float` | `number` (an `f32` widened exactly) | double | `float`, `double` |
| decimal, decimal from `f64` | `Decimal` {`lo`, `hi`, `scale`, `negative`} | `decimal` | `BigDecimal` | `Decimal` {`Lo`, `Hi`, `Scale`, `Negative`} | Foundation `Decimal` | `HyperCast\Decimal` {`magnitude` (string), `scale`, `negative`} | `HyperCast::Decimal` {`magnitude`, `scale`, `negative`} | `decimal.Decimal` | canonical `string` (`'1234.5'`) | canonical character (`"1234.5"`) | `Decimal` {`lo`, `hi`, `scale`, `negative`} (`to_chars`) |
| uuid | `[u8; 16]`, RFC 9562 order | `Guid` | `UUID` | `uuid.UUID` (`github.com/google/uuid`) | `UUID` | lowercase hyphenated `string` (`uuidBytes`: 16-byte `string`) | lowercase hyphenated `String` | `uuid.UUID` | lowercase hyphenated `string` | lowercase hyphenated character | `Uuid` {`bytes`}, RFC 9562 order |
| timestamp, unix, excel serial (text) | `Timestamp` {`seconds`, `nanos`} | `DateTimeOffset` | `Instant` | UTC `time.Time` | `Date` | UTC `DateTimeImmutable` (microseconds) | UTC `Time` | UTC-aware `datetime.datetime` (microseconds) | `Timestamp` {`seconds`, `nanos`} | UTC `POSIXct`, with exact `seconds` and `nanos` columns | `Timestamp` {`seconds`, `nanos`} |
| date, date ordered | `Date` {`year`, `month`, `day`} | `DateOnly` | `LocalDate` | `Date` {`Year`, `Month`, `Day`} | `DateComponents` | `DateTimeImmutable` at UTC midnight | `Date` | `datetime.date` | `CalendarDate` {`year`, `month`, `day`} | `Date` | `Date` {`year`, `month`, `day`} |
| datetime, excel serial from `f64` | `CivilDateTime` {`date`, `nanos_of_day`} | `DateTime`, `DateTimeKind.Unspecified` | `LocalDateTime` | `CivilDateTime` {`Date`, `TimeOfDay`} | `DateComponents` | `DateTimeImmutable` labeled UTC | `DateTime` at offset `+00:00` | naive `datetime.datetime` | `CivilDateTime` {`year`, `month`, `day`, `hour`, `minute`, `second`, `nanos`} | character wall clock (`"2026-01-07 15:04:05.123"`), with exact `date` and `nanos_of_day` columns | `CivilDateTime` {`date`, `nanos_of_day`} |
| time, excel time | `u64` nanoseconds since midnight | `TimeOnly` | `LocalTime` | `time.Duration` since midnight | `DateComponents` | `int` nanoseconds since midnight | `Integer` nanoseconds since midnight | `datetime.time` | `TimeOfDay` {`hour`, `minute`, `second`, `nanos`} | `difftime` seconds since midnight, with an exact `nanos` column | `std::chrono::nanoseconds` since midnight |
| duration, excel duration | `Duration` {`seconds`, `nanos`} | `TimeSpan` | `java.time.Duration` | `Duration` {`Seconds`, `Nanos`} (`AsDuration()`) | `Duration` | `HyperCast\Duration` {`seconds`, `nanos`} | `Rational` seconds | `datetime.timedelta` | `Duration` {`seconds`, `nanos`} | `difftime` in seconds, with exact `seconds` and `nanos` columns | `Duration` {`seconds`, `nanos`} |

C#'s `DateTimeOffset`, `TimeOnly` and `TimeSpan` resolve to 100 ns ticks; PHP and Python
truncate instants to microseconds. Each binding's README states its own fidelity.

## Verdict

| Concept | Rust | C# | Java | Go | Swift | PHP | Ruby | Python | TypeScript | R | C++ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Verdict type | `Result<T, Fault>` | `Verdict<T>` (`[Union]` record struct) | `Verdict<T>` (sealed interface) | `(T, *Fault)` | `Verdict<T>` (enum) | `Success\|Fault` | `Success` or `Fault` (no named union) | `Verdict[T]` (`Success[T] \| Fault`) | `Verdict<T>` (`Success<T> \| Failure`, discriminated on `ok`) | `hypercast_verdict` (a `data.frame` subclass, one row per element, `ok` the discriminant) | `Verdict<T>` (class template; `to_variant()`, and `std::expected<T, Fault>` under C++23) |
| Success case | `Ok(value)` | `Success<T>` (`Value`) | `Success<T>` (`value()`) | a `nil` `*Fault` | `.success(value)` | `Success` (`value`) | `HyperCast::Success` (`value`) | `Success` (`value`) | `Success<T>` (`ok: true`, `value`) | a row whose `ok` is `TRUE` (`value`) | `ok()` (`value()`, `value_if()`) |
| Fault case | `Err(Fault)` | `Fault` | `Fault<T>` | a non-`nil` `*Fault` (implements `error`) | `.fault(Fault)` | `Fault` | `HyperCast::Fault` | `Fault` | `Failure` (`ok: false`, `fault`: `Fault`) | a row whose `ok` is `FALSE` | `Fault` (`fault()`, `fault_if()`) |
| Typed-door verdict | `Result<T, Reason>` [^typed-reason] | `Fault` with `Offset` and `Length` 0 | `Fault` with `offset()` and `length()` 0 | `Fault` with `Offset` and `Length` 0 | `Fault` with `offset` and `length` 0 | `Fault` with `offset` and `length` 0 | `Fault` with `offset` and `length` 0 | `Fault` with `offset` and `length` 0 | `Fault` with `offset` and `length` 0 | a row with `offset` and `length` 0 | `Fault` with `offset` and `length` 0 |
| Fault reason | `reason` | `Reason` | `reason()` | `Reason` | `reason` | `reason` | `reason` | `reason` | `reason` | `reason` (factor column) | `reason` |
| Fault offset | `offset` (`u32`) | `Offset` (`int`) | `offset()` (`int`) | `Offset` (`int`) | `offset` (`Int`) | `offset` (`int`) | `offset` | `offset` | `offset` (`number`) | `offset` (integer column) | `offset` (`std::size_t`) |
| Fault length | `len` (`u32`) | `Length` (`int`) | `length()` (`int`) | `Length` (`int`) | `length` (`Int`) | `length` (`int`) | `length` | `length` | `length` (`number`) | `length` (integer column) | `length` (`std::size_t`) |
| Failure reasons | `Reason` | `CastFailure` [^unspecified] | `CastFailure` | `CastFailure` | `CastFailure` | `CastFailure` | Symbols (`HyperCast::REASONS` maps the codes) | `CastFailure` | `CastFailure` (a string-literal union; the `CastFailure` object names the three) | levels of the `reason` factor | `CastFailure` (`enum class`) |
| Empty | `Reason::Empty` | `CastFailure.Empty` | `CastFailure.EMPTY` | `Empty` | `.empty` | `CastFailure::Empty` | `:empty` | `CastFailure.EMPTY` | `'empty'` (`CastFailure.Empty`) | `"empty"` | `CastFailure::empty` |
| Malformed | `Reason::Malformed` | `CastFailure.Malformed` | `CastFailure.MALFORMED` | `Malformed` | `.malformed` | `CastFailure::Malformed` | `:malformed` | `CastFailure.MALFORMED` | `'malformed'` (`CastFailure.Malformed`) | `"malformed"` | `CastFailure::malformed` |
| Out of range | `Reason::OutOfRange` | `CastFailure.OutOfRange` | `CastFailure.OUT_OF_RANGE` | `OutOfRange` | `.outOfRange` | `CastFailure::OutOfRange` | `:out_of_range` | `CastFailure.OUT_OF_RANGE` | `'outOfRange'` (`CastFailure.OutOfRange`) | `"out_of_range"` | `CastFailure::out_of_range` |
| Optional (Empty as absent) | `optional(verdict)` → `Result<Option<T>, Fault>` | `Cast.Optional(verdict)` → `Verdict<T>?` | `Cast.optional(verdict)` → `Optional<Verdict<T>>` | — | `Cast.optional(_:)` → `Verdict<T>?` | `Cast::optional($verdict)` → `Success\|Fault\|null` | `HyperCast.optional(verdict)` → `nil` | `optional(verdict)` → `None` | `optional(verdict)` → `null` | `optional(x)` → missing rows (`ok` `NA`) | `optional(verdict)` → `std::nullopt` |

## Constraints

Validation constraints: the OpenAPI Schema Object's keywords, declared once for a door and
judged by the core after the door parses. The plain doors and their verdicts are unchanged.

| Concept | Rust | C# | Java | Go | Swift | PHP | Ruby | Python | TypeScript | R | C++ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Constraints type, and how it is built | `Constraints<'e, T>` (`T: Numeric` or `Text`): `Constraints::numeric(NumericRules, Option<&[i128]>)`, `Constraints::text(TextRules, Option<&[&str]>)` (`None` no `enum`, `Some(&[])` refused, 12), the rules built as `NumericRules::new().minimum(…)` … `.multiple_of(…, rounding)` and `TextRules::new().min_length(…)` … `.trim(true)`, each keyword with a `maybe_` twin taking an `Option`; `minimum` with `exclusive_minimum` (or the maximums) refused, 2 (3) | `Constraints<T>`: `new Constraints<T>(minimum:, exclusiveMinimum:, maximum:, exclusiveMaximum:, multipleOf:, rounding:, minLength:, maxLength:, trim:, @enum:)` (`@enum: null` no `enum`, `@enum: []` refused, 12) | `Constraints<T>`: a builder per door (`Constraints.i32()` … `Constraints.text()`), `.minimum(…)` … `.enumOf(…)`, `.build()` (no `enumOf` no `enum`, `enumOf(new String[0])` refused, 12) | `Constraints[V]`: `NewConstraints[V](rules...)` or `MustConstraints[V](rules...)`, with the rules `Minimum`, `ExclusiveMinimum`, `Maximum`, `ExclusiveMaximum`, `MultipleOf`, `WithRounding`, `MinLength`, `MaxLength`, `Trim`, `Enum` (no `Enum` rule no `enum`, `Enum()` refused, 12) | `Constraints<T>(minimum:, exclusiveMinimum:, maximum:, exclusiveMaximum:, multipleOf:, rounding:, minLength:, maxLength:, trim:, enum:)` (`enum: nil` no `enum`, `enum: []` refused, 12) | `new Constraints(Door::F64, minimum:, exclusiveMinimum:, maximum:, exclusiveMaximum:, multipleOf:, rounding:, minLength:, maxLength:, trim:, enum:)` (`enum: null` no `enum`, `enum: []` refused, 12) | `HyperCast::Constraints.new(door: :f64, minimum:, exclusive_minimum:, maximum:, exclusive_maximum:, multiple_of:, rounding:, min_length:, max_length:, trim:, enum:)` (`enum: nil` no `enum`, `enum: []` refused, 12) | `Constraints(Door.F64, minimum=, exclusive_minimum=, maximum=, exclusive_maximum=, multiple_of=, rounding=, min_length=, max_length=, trim=, enum=)` (`enum=None` no `enum`, `enum=[]` refused, 12) | `Constraints.declare({ door: Door.F64, minimum, exclusiveMinimum, maximum, exclusiveMaximum, multipleOf, rounding, minLength, maxLength, trim, enum })` (a `ConstraintsSpec`; `enum` left out no `enum`, `enum: []` refused, 12) | `hypercast_constraints("f64", minimum =, exclusive_minimum =, maximum =, exclusive_maximum =, multiple_of =, rounding =, min_length =, max_length =, trim =, enum =)` (`enum = NULL` no `enum`, `enum = character(0)` refused, 12) | `Constraints<T>::declare(fields, entries, storage)`, `Fields` {`minimum`, `exclusive_minimum`, `maximum`, `exclusive_maximum`, `multiple_of`, `rounding`, `min_length`, `max_length`, `trim`} (`declare(fields)` no `enum`, `declare(fields, {}, storage)` refused, 12) |
| Declared number (bound, `multipleOf`, integer `enum` entry) | `Decimal` (`Bound::Inclusive` or `Exclusive`; `i128` entries) | `ConstraintValue` (a `decimal`, or its invariant text) | `BigDecimal` or `String` | `ConstraintValue` (`string`, `Decimal` or a Go integer) | `ConstraintValue` (a `Decimal`, text or an integer literal) | `int`, `string` or `Decimal` | `Integer`, `BigDecimal`, `String` or `Decimal` | `decimal.Decimal`, `int` or `str` | `string` or `bigint` | a string or an R integer | `ConstraintValue` (a `Decimal`, an integer or text) |
| Refusal (a declaration that cannot mean anything at its door) | `Err(Refusal)` from `Constraints::numeric` and `::text` (`Refusal::code()`) | `ArgumentException` naming the rule (`ArgumentOutOfRangeException` for a negative length) | `IllegalArgumentException` from `build()` | `NewConstraints` returns an `error`; `MustConstraints` panics | a trap (`preconditionFailure`) naming the rule | `\InvalidArgumentException`, its code the core's refusal code | `ArgumentError` | `ValueError` | `Constraints.declare` returns a `Declaration`: `Success` or a `Refusal` {`ok: false`, `message`} | an error naming the rule | `Declared<Constraints<T>>`: `value()`, or `refusal()` → `Refusal` {`code`} (`message()`) |
| Checked verdict | `Result<Checked<T>, Rejection>` | `CheckedVerdict<T>` (a union of `Checked<T>` and `Rejection`) | `CheckedVerdict<T>` (sealed interface: `Checked<T>`, `Fault<T>`, `Violation<T>`) | `(Checked[V], Rejection)` (a `nil` `Rejection` is success) | `CheckedVerdict<T>` (enum: `.success(Checked<T>)`, `.rejection(Rejection)`) | `Checked\|Fault\|Violation` | `Checked`, `Fault` or `Violation` (no named union) | `CheckedVerdict[T]` (`Checked[T] \| Fault \| Violation`) | `CheckedVerdict<T>` (`Checked<T> \| Failure \| Violated`, discriminated on `ok`) | `hypercast_checked_verdict` (a `data.frame` subclass, one row per element, `ok` the discriminant) | `Checked<T>` (class template; `to_variant()`, and `to_expected()` under C++23) |
| Success case (value, adjusted, enum index) | `Checked<T>` {`value`, `adjusted`, `enum_index: Option<u32>`} | `Checked<T>` (`Value`, `Adjusted`, `EnumIndex` `int?`) | `Checked<T>` (`value()`, `adjusted()`, `enumIndex()` `OptionalInt`) | `Checked[V]` {`Value`, `Adjusted`, `EnumIndex` (`int`, `-1` for none)} | `Checked<T>` (`value`, `adjusted`, `enumIndex` `Int?`) | `Checked` (`value`, `adjusted`, `enumIndex` `?int`) | `HyperCast::Checked` (`value`, `adjusted`, `enum_index`) | `Checked` (`value`, `adjusted`, `enum_index`) | `Checked<T>` (`ok: true`, `value`, `adjusted`, `enumIndex` `number \| null`) | a row whose `ok` is `TRUE` (`value`, `adjusted`, `enum_index` counted from 1, `NA` for none) | `CheckedValue<T>` {`value`, `adjusted`, `enum_index` `std::optional<std::uint32_t>`} |
| Rejection (a fault or a violation) | `Rejection` (`Fault(Fault)`, `Violation(Violation)`) | `Rejection` (a union of `Fault` and `Violation`) | `Rejection<T>` (sealed interface: `Fault<T>`, `Violation<T>`) | `Rejection` (sealed interface: `*Fault`, `*Violation`) | `Rejection` (enum: `.fault(Fault)`, `.violation(Violation)`) | — (`Fault` or `Violation` directly) | — (`Fault` or `Violation` directly) | `Rejection` (`Fault \| Violation`) | `Rejection` (`Failure \| Violated`) | — (a row with `reason` set is a fault, one with `keyword` set a violation) | `Rejection` (`fault_if()`, `violation_if()`, `to_variant()`) |
| Violation (keyword, offset, length) | `Violation` {`keyword`, `offset` (`u32`), `len` (`u32`)} | `Violation(Keyword, Offset, Length)` | `Violation<T>` (`keyword()`, `offset()`, `length()`) | `*Violation` {`Keyword`, `Offset`, `Length`} (implements `error`) | `Violation` (`keyword`, `offset`, `length`) | `Violation` (`keyword`, `offset`, `length`; `keywordName()`) | `HyperCast::Violation` (`keyword`, `offset`, `length`) | `Violation` (`keyword`, `offset`, `length`) | `Violated` {`ok: false`, `violation`: `Violation` {`keyword`, `offset`, `length`}} | a row whose `ok` is `FALSE` with `keyword`, `offset` and `length` set; `violations()` lists them | `Violation` {`keyword`, `offset`, `length`} |
| Violation span units | UTF-8 bytes | chars for `ReadOnlySpan<char>` and `string`, bytes for `ReadOnlySpan<byte>` | chars for `String`, bytes otherwise | bytes | bytes | bytes | characters, or bytes for a binary `String` | code points for `str`, bytes for `bytes` | UTF-16 code units for a `string`, bytes for a `Uint8Array` | as a plain verdict's | bytes |
| Keyword (open: code and OpenAPI name) | `Keyword` (`#[non_exhaustive]` enum, `Minimum` … `Enum`; `code()`, `name()`) | `Keyword` (`Keyword.Minimum` … `Keyword.Enum`; `Code`, `Name`, `Keyword.FromCode`) | `Keyword` (`Keyword.MINIMUM` … `Keyword.ENUM`; `code()`, `name()`) | `Keyword` (`KeywordMinimum` … `KeywordEnum`; `String()`) | `Keyword` (`.minimum` … `.enum`; `rawValue`, `name`) | `Keyword` (enum, `Keyword::Minimum` … `Keyword::Enum`) | `Keyword` (`Keyword::MINIMUM` … `Keyword::ENUM`; `code`, `name`) | `Keyword` (`Keyword.MINIMUM` … `Keyword.ENUM`; `code`, `name`) | `Keyword` (string literals, `Keyword.Minimum` … `Keyword.Enum`) | the `keyword` column, OpenAPI's spelling | `Keyword` (`enum class`, `Keyword::minimum` … `Keyword::enum_`; `name(keyword)`) |
| Keyword not named by this binding | — (a wildcard arm; the enum is `#[non_exhaustive]`) | `Name` is `"keyword N"` | `name()` is `"keyword N"` | `String()` is `"keyword N"` | `name` is `"keyword N"` | `Violation::$keyword` is the bare `int`; `keywordName()` is `"keyword N"` | `name` is `"keyword N"` | `Keyword(N)`, `name` is `"keyword N"` | `"keyword N"` | `"keyword N"` | `name(keyword)` is `"keyword N"` |
| Rounding (strict, midpoint, floor, ceiling) | `Rounding::Strict`, `Midpoint`, `Floor`, `Ceiling` | `Rounding.Strict`, `Midpoint`, `Floor`, `Ceiling` | `Rounding.STRICT`, `MIDPOINT`, `FLOOR`, `CEILING` | `Strict`, `Midpoint`, `Floor`, `Ceiling` (`WithRounding`) | `.strict`, `.midpoint`, `.floor`, `.ceiling` | `Rounding::Strict`, `Midpoint`, `Floor`, `Ceiling` | `:strict`, `:midpoint`, `:floor`, `:ceiling` | `Rounding.STRICT`, `MIDPOINT`, `FLOOR`, `CEILING` | `Rounding.Strict`, `Midpoint`, `Floor`, `Ceiling` | `"strict"`, `"midpoint"`, `"floor"`, `"ceiling"` | `Rounding::strict`, `midpoint`, `floor`, `ceiling` |
| Optional (Empty as absent) on a checked verdict | — (`optional` takes a plain verdict) | — | — | — | — | — | `HyperCast.optional(verdict)` → `nil` | `optional(verdict)` → `None` | `optional(verdict)` → `null` | `optional(x)` → missing rows (`ok` `NA`) | `optional(checked)` → `std::nullopt` |

A violation is a value that parsed and broke a rule, never a failure reason and never absent:
every `optional` above turns only an `Empty` fault into absent. An enum index counts from 0
everywhere except R, which counts from 1.

## NumFormat

| Concept | Rust | C# | Java | Go | Swift | PHP | Ruby | Python | TypeScript | R | C++ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Type | `NumFormat` | `NumFormat` (record struct) | `NumFormat` (record) | `NumFormat` (struct) | `NumFormat` (struct) | `NumFormat` (readonly class) | `HyperCast::NumFormat` (`Data`) | `NumFormat` | `NumFormat` (class, no public constructor) | `hypercast_num_format` (S3 list) | `NumFormat` (class over the 92-byte `HyperCastRawNumFormat`) |
| Construct | `NumFormat::new(decimal_sep, group_sep, flags)`, `with_separators(..)` | `new NumFormat(decimalSeparator, groupSeparator, styles[, currencySymbol])` with init-only properties | `new NumFormat(decimalSeparator, groupSeparator, styles[, currencySymbol])`, `with*` | `NumFormat{DecimalSep, GroupSep, Styles, Currency, ...}` literal | `NumFormat(decimalSeparator:groupSeparator:styles:currencySymbol:...)` | `new NumFormat($decimalSep, $groupSep, $flags, $currency = '', ...)` (named arguments) | `NumFormat.new(decimal_sep:, group_sep:, flags:, currency: "", ...)`, `with` | `NumFormat(decimal_sep, group_sep, flags, currency="", *, ...)` | `NumFormat.declare({ decimalSep, groupSep, flags, currency?, ... })` → `Declaration<NumFormat>` (`Success<NumFormat> \| Refusal`) | `num_format(decimal_sep = ".", group_sep = ",", flags = num_styles[["all"]], currency = "", ...)` | `NumFormat::declare(Fields{decimal_sep, group_sep, styles, currency, ...})` → `std::optional<NumFormat>` |
| Invariant | `NumFormat::INVARIANT` (also `Default`) | `NumFormat.Invariant` | `NumFormat.INVARIANT` | `Invariant` | `NumFormat.invariant` | `NumFormat::invariant()` | `NumFormat::INVARIANT` | `NumFormat.INVARIANT` | `NumFormat.invariant()` (also what a left-out format means) | `num_format()` | `NumFormat::invariant()` |
| Detect | `NumFormat::DETECT` | `NumFormat.Detect` | `NumFormat.DETECT` | `Detect` | `NumFormat.detect` | `NumFormat::detect()` | `NumFormat::DETECT` | `NumFormat.DETECT` | `NumFormat.detect()` | `num_format_detect()` | `NumFormat::detect()` |
| Decimal separator [^symbol] | `decimal_sep` (`Symbol<8>`) | `DecimalSeparator` (`string`) | `decimalSeparator()` (`String`) | `DecimalSep` (`string`) | `decimalSeparator` (`String`) | `decimalSep` (`string`) | `decimal_sep` | `decimal_sep` | `decimalSep` (`string`) | `decimal_sep` | `decimal_sep` (`std::string_view`) |
| Group separator [^symbol] | `group_sep` (`Symbol<8>`) | `GroupSeparator` (`string`) | `groupSeparator()` (`String`) | `GroupSep` (`string`) | `groupSeparator` (`String`) | `groupSep` (`string`) | `group_sep` | `group_sep` | `groupSep` (`string`) | `group_sep` | `group_sep` (`std::string_view`) |
| Currency separators (read once the declared symbol is found; empty keeps the number pair) | `currency_decimal_sep`, `currency_group_sep` (`with_currency_separators`) | `CurrencyDecimalSeparator`, `CurrencyGroupSeparator` | `currencyDecimalSeparator()`, `currencyGroupSeparator()` | `CurrencyDecimalSep`, `CurrencyGroupSep` | `currencyDecimalSeparator`, `currencyGroupSeparator` | `currencyDecimalSep`, `currencyGroupSep` | `currency_decimal_sep`, `currency_group_sep` | `currency_decimal_sep`, `currency_group_sep` | `currencyDecimalSep`, `currencyGroupSep` | `currency_decimal_sep`, `currency_group_sep` | `currency_decimal_sep`, `currency_group_sep` |
| Declared minus and plus signs (read beside ASCII) | `minus_sign`, `plus_sign` (`with_signs`) | `MinusSign`, `PlusSign` | `minusSign()`, `plusSign()` | `MinusSign`, `PlusSign` | `minusSign`, `plusSign` | `minusSign`, `plusSign` | `minus_sign`, `plus_sign` | `minus_sign`, `plus_sign` | `minusSign`, `plusSign` | `minus_sign`, `plus_sign` | `minus_sign`, `plus_sign` |
| Declared percent symbol (read beside `%`) | `percent` (`with_percent`) | `PercentSymbol` | `percentSymbol()` | `Percent` | `percentSymbol` | `percent` | `percent` | `percent` | `percent` | `percent` | `percent` |
| Native digits (the zero of a digit block, read beside ASCII) | `digits` (`NativeDigits`, `with_digits`) | `DigitZero` (`Rune?`) | `digitZero()` (`int` code point, 0 = none) | `DigitZero` (`rune`, 0 = none) | `digitZero` (`Unicode.Scalar?`) | `digitZero` (`string`, `''` = none) | `digit_zero` (`""` = none) | `digit_zero` (`""` = none) | `digitZero` (`string`, `''` = none) | `digit_zero` (`""` = none) | `digit_zero` (`char32_t`, 0 = none) |
| Styles field | `flags` (`u32`) | `Styles` (`NumStyles`) | `styles()` (`int`) | `Styles` (`NumStyles`) | `styles` (`NumStyles`) | `flags` (`int`) | `flags` | `flags` | `flags` (`number`) | `flags` (integer) | `styles` (`NumStyles`) |
| Currency symbol field | `currency` (`CurrencySymbol`) | `CurrencySymbol` (`string`) | `currencySymbol()` (`String`) | `Currency` (`string`) | `currencySymbol` (`String`) | `currency` (`string`) | `currency` | `currency` | `currency` (`string`) | `currency` | `currency` (`std::string_view`) |
| Declare a symbol | `.with_currency(symbol)`, `CurrencySymbol::new(&str)` → `Option<CurrencySymbol>` | constructor argument | constructor argument | `Currency:` field | `currencySymbol:` argument | `$currency` argument | `currency:` keyword | `currency` argument | `currency` spec field | `currency` argument | `Fields::currency` |
| No symbol | `CurrencySymbol::NONE` | `""` | `""` | `""` | `""` | `''` | `""` | `""` | `''` (or left out of the spec) | `""` | `""` (an empty `std::string_view`) |
| Symbol limit (16 UTF-8 bytes) | `CurrencySymbol::MAX_BYTES` | `NumFormat.MaxCurrencyBytes` | — (private) | `CurrencyMaxBytes` | `Interop.currencyMaxBytes` | `NumFormat::CURRENCY_MAX_BYTES` | `NumFormat::CURRENCY_MAX_BYTES` | — | — | — | `HyperCastCurrencySymbol_MAX_BYTES` (`hypercast.h`) |
| Separator and sign limit (8 UTF-8 bytes) | `Symbol::<8>::MAX_BYTES` [^symbol-n] | `NumFormat.MaxSymbolBytes` | — (private) | `SymbolMaxBytes` | `Interop.symbolMaxBytes` | `NumFormat::SYMBOL_MAX_BYTES` | `NumFormat::SYMBOL_MAX_BYTES` | — | — | — | — |
| Platform culture bridge (fills every field above) [^bridge-fields] | — [^no-bridge] | `NumFormat.From(CultureInfo)`, `From(IFormatProvider)`, `From(NumberFormatInfo)` | `NumFormat.from(Locale)`, `from(DecimalFormat)` | — [^no-bridge] | `NumFormat.from(locale:)`, `from(formatter:)` | `NumFormat::fromLocaleconv(?array $conv = null)`, `NumFormat::fromNumberFormatter(\NumberFormatter)` | `NumFormat.from_i18n(hash, currency_format:)` (I18n `number.format` and `number.currency.format` Hashes) [^no-bridge] | `NumFormat.from_localeconv(conv=None)` | — [^ts-no-bridge] | `num_format_from_localeconv(conv = Sys.localeconv())` | — [^cpp-no-bridge] |
| A numeric door taking the platform's own format [^platform-format] | — | an `IFormatProvider? provider` overload of every numeric door, `Numeric<T>` and `Scalar<T>`; `null` is the current culture | a `Locale` overload of every numeric door | — [^no-bridge] | a `locale:` overload of every numeric door, `numeric` and `scalar` | `NumFormat\|\NumberFormatter` as the format argument | `separator:`, `delimiter:`, `unit:` keywords on every numeric door (`NumFormat#override`) | — [^python-format] | — [^ts-no-bridge] | — [^python-format] | — [^cpp-no-bridge] |

### Styles

| Flag | Rust | C# | Java | Go | Swift | PHP | Ruby | Python | TypeScript | R | C++ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Flags type | `u32` constants on `NumFormat` | `NumStyles` (`[Flags]` enum) | `int` constants on `NumFormat` | `NumStyles` | `NumStyles` (`OptionSet`) | `int` constants on `NumFormat` | `Integer` constants on `HyperCast` | `int` class attributes on `NumFormat` | `NumStyles` (numeric enum; `flags` is a `number`) | `num_styles` (named integer vector; `flags` also takes the names) | `NumStyles` (`enum class` with `\|`, `&`, `^`, `~`) |
| None | `0` | `NumStyles.None` | `0` | `0` | `[]` | `0` | `0` | `0` | `NumStyles.None` | `0L` | `NumStyles::none` |
| Grouping (`1`) | `NumFormat::GROUPING` | `NumStyles.Grouping` | `NumFormat.STYLE_GROUPING` | `Grouping` | `.grouping` | `NumFormat::GROUPING` | `HyperCast::GROUPING` | `NumFormat.GROUPING` | `NumStyles.Grouping` | `num_styles[["grouping"]]` | `NumStyles::grouping` |
| Parentheses (`1 << 1`) | `NumFormat::PARENS` | `NumStyles.Parentheses` | `NumFormat.STYLE_PARENTHESES` | `Parentheses` | `.parentheses` | `NumFormat::PARENTHESES` | `HyperCast::PARENTHESES` | `NumFormat.PARENTHESES` | `NumStyles.Parentheses` | `num_styles[["parentheses"]]` | `NumStyles::parentheses` |
| Exponent (`1 << 2`) | `NumFormat::EXPONENT` | `NumStyles.Exponent` | `NumFormat.STYLE_EXPONENT` | `Exponent` | `.exponent` | `NumFormat::EXPONENT` | `HyperCast::EXPONENT` | `NumFormat.EXPONENT` | `NumStyles.Exponent` | `num_styles[["exponent"]]` | `NumStyles::exponent` |
| Radix prefixes (`1 << 3`) | `NumFormat::RADIX_PREFIX` | `NumStyles.RadixPrefixes` | `NumFormat.STYLE_RADIX_PREFIXES` | `RadixPrefixes` | `.radixPrefixes` | `NumFormat::RADIX_PREFIXES` | `HyperCast::RADIX_PREFIXES` | `NumFormat.RADIX_PREFIXES` | `NumStyles.RadixPrefixes` | `num_styles[["radix_prefixes"]]` | `NumStyles::radix_prefixes` |
| Percent (`1 << 4`) | `NumFormat::PERCENT` | `NumStyles.Percent` | `NumFormat.STYLE_PERCENT` | `Percent` | `.percent` | `NumFormat::PERCENT` | `HyperCast::PERCENT` | `NumFormat.PERCENT` | `NumStyles.Percent` | `num_styles[["percent"]]` | `NumStyles::percent` |
| Separator detect (`1 << 5`) | `NumFormat::SEPARATOR_DETECT` | `NumStyles.SeparatorDetect` | `NumFormat.STYLE_SEPARATOR_DETECT` | `SeparatorDetect` | `.separatorDetect` | `NumFormat::SEPARATOR_DETECT` | `HyperCast::SEPARATOR_DETECT` | `NumFormat.SEPARATOR_DETECT` | `NumStyles.SeparatorDetect` | `num_styles[["separator_detect"]]` | `NumStyles::separator_detect` |
| Currency (`1 << 6`) | `NumFormat::CURRENCY` | `NumStyles.Currency` | `NumFormat.STYLE_CURRENCY` | `CurrencySymbol` | `.currency` | `NumFormat::CURRENCY` | `HyperCast::CURRENCY` | `NumFormat.CURRENCY` | `NumStyles.Currency` | `num_styles[["currency"]]` | `NumStyles::currency` |
| Trailing sign (`1 << 7`; not in All; set by each bridge from the culture's negative pattern) | `NumFormat::TRAILING_SIGN` | `NumStyles.TrailingSign` | `NumFormat.STYLE_TRAILING_SIGN` | `TrailingSign` | `.trailingSign` | `NumFormat::TRAILING_SIGN` | `HyperCast::TRAILING_SIGN` | `NumFormat.TRAILING_SIGN` | `NumStyles.TrailingSign` | `num_styles[["trailing_sign"]]` | `NumStyles::trailing_sign` |
| All leniences (not separator detect) | `NumFormat::ALL` | `NumStyles.All` | `NumFormat.STYLE_ALL` | `AllStyles` | `.all` | `NumFormat::ALL` | `HyperCast::ALL_STYLES` | `NumFormat.ALL` | `NumStyles.All` | `num_styles[["all"]]` | `NumStyles::all` |

## Declared enums

| Concept | Rust | C# | Java | Go | Swift | PHP | Ruby | Python | TypeScript | R | C++ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Unix precision | `UnixPrecision` | `UnixPrecision` [^unspecified] | `UnixPrecision` | `UnixPrecision` | `UnixPrecision` | `UnixPrecision` | Symbols, keys of `HyperCast::UNIX_PRECISIONS` | `UnixPrecision` | `UnixPrecision` (numeric enum) | strings, the `precision` argument of `cast_unix` | `UnixPrecision` (`enum class`) |
| Seconds (`1`) | `UnixPrecision::Seconds` | `UnixPrecision.Seconds` | `UnixPrecision.SECONDS` | `Seconds` | `.seconds` | `UnixPrecision::Seconds` | `:seconds` | `UnixPrecision.SECONDS` | `UnixPrecision.Seconds` | `"seconds"` | `UnixPrecision::seconds` |
| Milliseconds (`2`) | `UnixPrecision::Millis` | `UnixPrecision.Milliseconds` | `UnixPrecision.MILLISECONDS` | `Milliseconds` | `.milliseconds` | `UnixPrecision::Milliseconds` | `:milliseconds` | `UnixPrecision.MILLISECONDS` | `UnixPrecision.Milliseconds` | `"milliseconds"` | `UnixPrecision::milliseconds` |
| Microseconds (`3`) | `UnixPrecision::Micros` | `UnixPrecision.Microseconds` | `UnixPrecision.MICROSECONDS` | `Microseconds` | `.microseconds` | `UnixPrecision::Microseconds` | `:microseconds` | `UnixPrecision.MICROSECONDS` | `UnixPrecision.Microseconds` | `"microseconds"` | `UnixPrecision::microseconds` |
| Nanoseconds (`4`) | `UnixPrecision::Nanos` | `UnixPrecision.Nanoseconds` | `UnixPrecision.NANOSECONDS` | `Nanoseconds` | `.nanoseconds` | `UnixPrecision::Nanoseconds` | `:nanoseconds` | `UnixPrecision.NANOSECONDS` | `UnixPrecision.Nanoseconds` | `"nanoseconds"` | `UnixPrecision::nanoseconds` |
| Excel epoch | `ExcelEpoch` | `ExcelEpoch` [^unspecified] | `ExcelEpoch` | `ExcelEpoch` | `ExcelEpoch` | `ExcelEpoch` | Symbols, keys of `HyperCast::EXCEL_EPOCHS` | `ExcelEpoch` | `ExcelEpoch` (numeric enum) | strings, the `epoch` argument of the Excel doors | `ExcelEpoch` (`enum class`) |
| 1900 system (`1`) | `ExcelEpoch::Y1900` | `ExcelEpoch.Y1900` | `ExcelEpoch.Y1900` | `Excel1900` | `.y1900` | `ExcelEpoch::Y1900` | `:y1900` | `ExcelEpoch.Y1900` | `ExcelEpoch.Y1900` | `"y1900"` | `ExcelEpoch::y1900` |
| 1904 system (`2`) | `ExcelEpoch::Y1904` | `ExcelEpoch.Y1904` | `ExcelEpoch.Y1904` | `Excel1904` | `.y1904` | `ExcelEpoch::Y1904` | `:y1904` | `ExcelEpoch.Y1904` | `ExcelEpoch.Y1904` | `"y1904"` | `ExcelEpoch::y1904` |
| Date order | `DateOrder` | `DateOrder` [^unspecified] | `DateOrder` | `DateOrder` | `DateOrder` | `DateOrder` | Symbols, keys of `HyperCast::DATE_ORDERS` | `DateOrder` | `DateOrder` (numeric enum) | strings, the `order` argument of `cast_date` and `cast_datetime` | `DateOrder` (`enum class`) |
| Year, month, day (`1`) | `DateOrder::YearMonthDay` | `DateOrder.YearMonthDay` | `DateOrder.YEAR_MONTH_DAY` | `YearMonthDay` | `.yearMonthDay` | `DateOrder::Ymd` | `:year_month_day` | `DateOrder.YEAR_MONTH_DAY` | `DateOrder.YearMonthDay` | `"year_month_day"` | `DateOrder::year_month_day` |
| Month, day, year (`2`) | `DateOrder::MonthDayYear` | `DateOrder.MonthDayYear` | `DateOrder.MONTH_DAY_YEAR` | `MonthDayYear` | `.monthDayYear` | `DateOrder::Mdy` | `:month_day_year` | `DateOrder.MONTH_DAY_YEAR` | `DateOrder.MonthDayYear` | `"month_day_year"` | `DateOrder::month_day_year` |
| Day, month, year (`3`) | `DateOrder::DayMonthYear` | `DateOrder.DayMonthYear` | `DateOrder.DAY_MONTH_YEAR` | `DayMonthYear` | `.dayMonthYear` | `DateOrder::Dmy` | `:day_month_year` | `DateOrder.DAY_MONTH_YEAR` | `DateOrder.DayMonthYear` | `"day_month_year"` | `DateOrder::day_month_year` |
| Date order from the platform locale | — [^no-bridge] | `DateOrders.From(CultureInfo)` | `DateOrder.from(Locale)` | — [^no-bridge] | `DateOrder.from(locale:)` | — [^no-bridge] | — [^no-bridge] | — | — [^ts-no-bridge] | — [^no-bridge] | — [^cpp-no-bridge] |
| Day-period designators beside the order (the culture's `AM`/`PM`, read beside English) [^periods] | `DayPeriods::new(am, pm)` with `cast_datetime_with` | `new DateFormat(order, amDesignator, pmDesignator)` with `Cast.DateTime` | `DateTimeFormat(order, am, pm)` with `Cast.dateTime` | `DateFormat{Order, AM, PM}` with `DateTimeWith` | `DateFormat(order:amSymbol:pmSymbol:)` with `Cast.dateTime(_:format:)` | `new DayPeriods($am, $pm)`, third argument of `Cast::datetime` | `am:`, `pm:` keywords on `HyperCast.datetime` | `am=`, `pm=` keyword-only on `cast_datetime` | `DateFormat.declare({ order, am?, pm? })` → `Declaration<DateFormat>`, with `castDatetime` | `am`, `pm` arguments of `cast_datetime` | `DateFormat::declare(order, am_symbol, pm_symbol)` → `std::optional<DateFormat>`, with `cast_datetime` |
| Day periods from the platform locale | — [^no-bridge] | `DateFormat.From(CultureInfo)`, `From(IFormatProvider)`, `From(DateTimeFormatInfo)`, and an `IFormatProvider?` overload of `Cast.DateTime` | `DateTimeFormat.from(Locale)`, and a `Locale` overload of `Cast.dateTime` | — [^no-bridge] | `DateFormat.from(locale:)`, and `Cast.dateTime(_:locale:)` | — [^no-bridge] | — [^no-bridge] | — | — [^ts-no-bridge] | — [^no-bridge] | — [^cpp-no-bridge] |

## The native library

| Concept | Rust | C# | Java | Go | Swift | PHP | Ruby | Python | TypeScript | R | C++ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Version probe (`hypercast_version`) | `hypercast_version()` → `u32`, packed `major << 16 \| minor << 8 \| patch` | `Cast.NativeVersion` → `Version?` | `Cast.nativeVersion()` → `String` | `NativeVersion()` → `string` | `Cast.nativeVersion()` → `String` | `Cast::nativeVersion()` → `string` | `HyperCast.native_version` → `String` | `native_version()` → `str` | `nativeVersion()` → `string` | `hypercast_native_version()` → `package_version` | `native_version()` → `std::string_view` |
| Availability, never throws | — [^rust-probe] | `Cast.IsAvailable` | `Cast.isAvailable()` | `Available()`, always `true` [^linked] | `Cast.isAvailable`, always `true` [^linked] | `extension_loaded('hypercast')` [^php-probe] | `HyperCast.available?` | — | `isAvailable()` | — [^r-probe] | `is_available()`, always `true` [^linked] |
| Load failure | — | — | — | `LoadError()`, always `nil` [^linked] | — | — | — | — | `ERR_HYPERCAST_UNAVAILABLE`, thrown by every door, its `cause` the loader's error (`isHyperCastError(error)`) | — | — |
| Backend in use | — | — | — | — | — | — | `HyperCast::BACKEND` → `:native` or `:fiddle` | — | — | — | — |

Rust is the crate's own API (the `rust/src/lib.rs` re-exports). The other names are
relative to C# namespace `HyperCast`, Java package `io.github.skunkwerkx.hypercast`, Go
package `hypercast`, Swift module `HyperCast`, PHP namespace `HyperCast`, Ruby module
`HyperCast`, Python package `hypercast`, the npm package `@skunkwerkx/hypercast` (named
exports), the R package `hypercast` (exported functions) and the C++ namespace `hypercast`
(`include/hypercast.hpp`). `—` means no counterpart; a footnote says why
when the absence is deliberate.

[^checked-doors]: A checked door is the plain numeric door's overload taking a `Constraints`
    after the format, and answers a checked verdict; the plain door's signature and verdict
    are unchanged. Go, PHP, Ruby, Python, TypeScript and R name theirs apart.

[^csharp-text]: C#'s plain text door answers a `CheckedVerdict<string>`, since
    `Verdict<T>` does not hold a `string`; with nothing declared its rejection is always a
    `Fault`. Every other binding's plain text door answers its ordinary verdict.

[^go-names]: Go names a door after what it returns where Go or the package already owns
    the ABI word: `Exact` and `Span` because the result types are `Decimal` and `Duration`,
    and `DateOnly`, `DateOnlyOrdered` and `TimeOfDay` for what `cast_date`,
    `cast_date_ordered` and `cast_time` return (`go/cast.go` package doc).

[^folded]: PHP, Ruby, Python, TypeScript and R fold `cast_date` and `cast_date_ordered` into one entry
    point whose order argument is optional (`?DateOrder $order = null`, `order = nil`,
    `order: DateOrder | None = None`, `order?: DateOrder`, `order = NULL`): absent, the door is
    strict ISO `yyyy-MM-dd` (`cast_date`); present, it is `cast_date_ordered`. C#, Java, Swift
    and C++ keep one name and overload it; Rust and Go keep two names.

[^typed-reason]: Rust's typed doors return a bare `Reason`, since a double has no text to
    index. The C ABI exports and every binding return their ordinary fault with an empty
    span (offset 0, length 0) instead (CHANGELOG, 0.7.0).

[^excel-serial-name]: The Rust twin of `cast_excel_serial` for a held `f64` is
    `excel_serial`; the export is `cast_excel_serial_from_f64` and the bindings follow the
    export's naming (`FromDouble`, `FromFloat64`, `FromFloat`, `FromNumber`, `from_float`,
    `from_double`).

[^generic]: Not in Java, which has no generics over primitives, nor in the dynamic bindings (PHP,
    Ruby, Python, R), nor in TypeScript, whose types are erased before a door runs, so nothing
    could pick the door from `T`: there the per-width doors are the idiom (`docs/roadmap.md`,
    CHANGELOG). Rust callers have the concrete doors.

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
    bridge; R is in the same position, bridging `num_format()` from `Sys.localeconv()`, which
    carries no date order (`r/R/num_format.R`). The same holds for day-period designators: no
    locale data in Go, Ruby or PHP's standard library names a culture's `AM`/`PM`, and neither
    Python's `localeconv()` nor R's `Sys.localeconv()` carries any, so those five declare them
    by hand, as they do the order.

[^layout]: The format crosses the C ABI as a size-tagged, 92-byte layout, the
    `#[repr(C)]` `NumFormat` itself behind a size word, read in place; day periods cross as a
    40-byte `RawDayPeriods` after the date order, null for English only. `rust/src/abi.rs`
    has the offset table every binding restates and pins in its interop tests.

[^symbol]: 1 to 8 UTF-8 bytes, containing no ASCII digit; the two
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
    the culture declares carries over — its signs, a trailing-sign pattern, the
    separators of a currency amount, its percent symbol, its digits — each read beside the
    ASCII default, never instead of it; the
    [culture fidelity sweep](roadmap.md#culture-fidelity--the-sweep-built-090) has every row.

[^python-format]: Python's and R's standard libraries have no per-object locale: Python's `locale` is
    process-global, as R's `Sys.localeconv()` is, and `NumFormat.from_localeconv` and
    `num_format_from_localeconv()` already read it, so the doors keep taking a `NumFormat`.

[^rust-probe]: The crate is the core, so there is nothing to load and nothing to probe;
    `hypercast_version()` is re-exported for a crate that fronts its own C ABI.

[^linked]: Go compiles the core in as Go source and Swift links it statically, and C++ links
    it at build time, so it is always there; the probes are there for code written against the
    load probe every binding carries.

[^cpp-generic]: C++ has no template door: the header mirrors the C ABI's names one for one
    (`include/hypercast.hpp`, "Names mirror the C ABI's"), so a caller holding a generic `T`
    picks the concrete door itself.

[^r-nul]: An R string cannot hold U+0000, which the core accepts, so `cast_char` reports it as
    `out_of_range` over the input less its ASCII whitespace edges, as C# and Java report a
    scalar their UTF-16 `char` cannot hold (`r/R/doors.R`).

[^ts-no-bridge]: The TypeScript binding ships no bridge from `Intl.NumberFormat` or `Intl.DateTimeFormat`:
    formats, orders and day periods are declared through `NumFormat.declare`, the
    `DateOrder` enum and `DateFormat.declare` only (`typescript/index.d.ts`).

[^cpp-no-bridge]: The C++ header reads no culture: no bridge from `std::locale` (`std::numpunct`,
    `std::time_get::date_order`, `std::moneypunct`) and no door overload taking one. A
    caller maps its own culture to `NumFormat::Fields`, a `DateOrder` and
    `DateFormat::declare` (`include/hypercast.hpp`).

[^r-probe]: The R package links the core statically into its own shared object, so `library(hypercast)`
    succeeding means the core is there, as Python's import does;
    `hypercast_native_version()` is the one probe (`r/R/native.R`).

[^php-probe]: PHP's own check for a loaded extension: without it the classes do not exist, so there is
    nothing to call a probe on; `Cast::nativeVersion()` is the version (`php/README.md`,
    "Checking it is loaded").
