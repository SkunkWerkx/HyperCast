<?php

declare(strict_types=1);

namespace HyperCast;

use DateTimeImmutable;
use FFI;
use HyperCast\Interop\NativePlatform;
use HyperCast\Interop\NativeValues;

/**
 * Allocation-lean scalar casts — booleans, numerics (integer, real and exact decimal),
 * UUIDs, temporals — calling directly into the native libhypercast shared library via
 * PHP's built-in ext-ffi, no runtime bridge and no Composer dependency. Every door returns
 * the `Success|Fault` verdict union; never an exception for bad data — an exception here
 * is a caller bug (a malformed NumFormat), never data. {@see nativeVersion()} names the
 * library that actually loaded.
 *
 * Door names mirror the native ABI (i32, f64, timestamp, ...) so the polyglot surface
 * reads identically across bindings. PHP strings are raw bytes, so inputs cross verbatim
 * and fault offsets need no mapping.
 *
 * Performance shape, measured not assumed: PHP's raw ext-ffi call floor is ~105 ns —
 * already extension-class — so every avoidable nanosecond here was wrapper, and the
 * wrapper is written accordingly: doors are flat (one FFI call, no helper or closure
 * indirection on the hot path), out-params are typed cdef structs read as fields (no
 * string round trips), scratch CData is allocated once (PHP's request model makes static
 * scratch safe), and instants build through createFromTimestamp/setMicrosecond on PHP
 * 8.4+ instead of a date-string parse (older PHP falls back automatically).
 *
 * PHP-flavored fidelity, stated honestly: int is 64-bit signed, so u64 carries the
 * two's-complement bit pattern (render with sprintf('%u', ...)); DateTimeImmutable tops
 * out at microseconds, so the core's nanoseconds truncate by three digits on the instant
 * doors; time-of-day is an exact int of nanoseconds since midnight; durations come back as
 * the protobuf pair ({@see Duration}) because DateInterval can't carry them; decimals come
 * back as the core's exact triple ({@see Decimal}) because PHP has no decimal type at all.
 */
final class Cast
{
    private static ?FFI $ffi = null;
    private static ?FFI\CData $out16 = null;
    private static ?FFI\CData $outPair = null;
    private static ?FFI\CData $outDate = null;
    private static ?FFI\CData $outCivil = null;
    private static ?FFI\CData $outI64 = null;
    private static ?FFI\CData $outReal = null;
    private static ?FFI\CData $outDecimal = null;
    private static ?FFI\CData $fault = null;
    private static ?FFI\CData $format = null;
    // Pre-taken addresses: FFI auto-decays arrays to pointers but not scalars/structs, and
    // FFI::addr() per call would be a fresh CData allocation on the hot path.
    private static ?FFI\CData $outPairPtr = null;
    private static ?FFI\CData $outDatePtr = null;
    private static ?FFI\CData $outCivilPtr = null;
    private static ?FFI\CData $outI64Ptr = null;
    private static ?FFI\CData $outRealPtr = null;
    private static ?FFI\CData $outDecimalPtr = null;
    private static ?FFI\CData $faultPtr = null;
    private static ?FFI\CData $formatPtr = null;
    private static ?NumFormat $formatKey = null;
    private static ?bool $available = null;

    /** Static-only facade — never instantiated. */
    private function __construct()
    {
    }

    /**
     * Presents a verdict optionally: an Empty fault becomes null (PHP's absent),
     * everything else flows through untouched.
     *
     * @param Success|Fault $verdict the verdict to present
     * @return Success|Fault|null null for an Empty fault; the untouched verdict otherwise
     */
    public static function optional(Success|Fault $verdict): Success|Fault|null
    {
        return $verdict instanceof Fault && $verdict->reason === CastFailure::Empty ? null : $verdict;
    }

    /**
     * The cold path: assembles a Fault from the scratch span, or reports a binding bug.
     *
     * @param int $rc the native failure code
     * @return Fault the assembled fault
     */
    private static function fail(int $rc): Fault
    {
        if ($rc === -1) {
            throw new \RuntimeException(
                'hypercast: libhypercast reported a contract violation — a binding bug, please report it'
            );
        }
        return NativeValues::fault($rc, self::$fault->offset, self::$fault->length);
    }

    /**
     * Re-stores the declared format only when it actually changes (identity check). The
     * currency bytes are always written as the full 16, zero-padded — the memo means a
     * shorter symbol can follow a longer one into the same scratch, and the core's contract
     * is zero-padding beyond `currency_len`.
     *
     * @param NumFormat $format the caller-declared numeric notation
     * @return void
     */
    private static function declare(NumFormat $format): void
    {
        if (self::$formatKey !== $format) {
            NativeValues::writeFormat($format, self::$format);
            self::$formatKey = $format;
        }
    }

    /**
     * Casts boolean text: true/false plus the conventions untrusted sources actually send
     * (t/f, yes/no, y/n, 1/0, on/off, enabled/disabled, active/inactive,
     * checked/unchecked, in/out), ASCII case-insensitive.
     *
     * @param string $text the text to cast
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function bool(string $text): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_bool($text === '' ? null : $text, \strlen($text), self::$out16, self::$faultPtr);
        return $rc === 0 ? new Success(self::$out16[0] !== 0) : self::fail($rc);
    }

    /**
     * Casts char text: an input that is exactly one UTF-8 scalar is that scalar, checked
     * before trimming (" " is a space, "6" the digit); otherwise one declared code point,
     * ASCII case-insensitive on the prefix — decimal 65, U+0041, 0x41, &H41, &#65; or
     * &#x41;. A code point past U+10FFFF, or a surrogate, is OutOfRange.
     *
     * @param string $text the text to cast
     * @return Success|Fault the verdict: a Success carrying the scalar as a UTF-8 string, or a Fault
     */
    public static function char(string $text): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        self::$outI64->cdata = 0;
        $rc = $ffi->cast_char($text === '' ? null : $text, \strlen($text), self::$outI64Ptr, self::$faultPtr);
        return $rc === 0 ? new Success(self::utf8(self::$outI64->cdata)) : self::fail($rc);
    }

    /**
     * UTF-8 for a scalar the core already range-checked — by hand, so the door needs no
     * mbstring or intl.
     *
     * @param int $scalar a Unicode scalar value
     * @return string its UTF-8 encoding
     */
    private static function utf8(int $scalar): string
    {
        if ($scalar < 0x80) {
            return \chr($scalar);
        }
        if ($scalar < 0x800) {
            return \chr(0xC0 | $scalar >> 6) . \chr(0x80 | $scalar & 0x3F);
        }
        if ($scalar < 0x10000) {
            return \chr(0xE0 | $scalar >> 12) . \chr(0x80 | $scalar >> 6 & 0x3F) . \chr(0x80 | $scalar & 0x3F);
        }
        return \chr(0xF0 | $scalar >> 18) . \chr(0x80 | $scalar >> 12 & 0x3F)
            . \chr(0x80 | $scalar >> 6 & 0x3F) . \chr(0x80 | $scalar & 0x3F);
    }

    /**
     * Integer doors: the target type's own range, declared grouping, accounting parens,
     * non-negative exponent, and 0x/&H/0b two's-complement radix prefixes. Every width
     * funnels through one zeroed 64-bit scratch slot (the supported RIDs are all
     * little-endian); narrow signed widths sign-extend on readback. Each door is written
     * out flat — one literal FFI call, no shared helper — the same rule the real doors
     * follow, so no width pays a dynamic symbol lookup or a string match to find its shift.
     *
     * @param string $text the text to cast
     * @param NumFormat $format the caller-declared numeric notation
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function i8(string $text, NumFormat $format): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        self::declare($format);
        self::$outI64->cdata = 0;
        $rc = $ffi->cast_i8(
            $text === '' ? null : $text,
            \strlen($text),
            self::$formatPtr,
            self::$outI64Ptr,
            self::$faultPtr
        );
        if ($rc !== 0) {
            return self::fail($rc);
        }
        $raw = self::$outI64->cdata;
        return new Success($raw << 56 >> 56);
    }

    /**
     * Casts integer text to a signed 16-bit value. Notation rules as {@see i8()}.
     *
     * @param string $text the text to cast
     * @param NumFormat $format the caller-declared numeric notation
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function i16(string $text, NumFormat $format): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        self::declare($format);
        self::$outI64->cdata = 0;
        $rc = $ffi->cast_i16(
            $text === '' ? null : $text,
            \strlen($text),
            self::$formatPtr,
            self::$outI64Ptr,
            self::$faultPtr
        );
        if ($rc !== 0) {
            return self::fail($rc);
        }
        $raw = self::$outI64->cdata;
        return new Success($raw << 48 >> 48);
    }

    /**
     * Casts integer text to a signed 32-bit value. Notation rules as {@see i8()}.
     *
     * @param string $text the text to cast
     * @param NumFormat $format the caller-declared numeric notation
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function i32(string $text, NumFormat $format): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        self::declare($format);
        self::$outI64->cdata = 0;
        $rc = $ffi->cast_i32(
            $text === '' ? null : $text,
            \strlen($text),
            self::$formatPtr,
            self::$outI64Ptr,
            self::$faultPtr
        );
        if ($rc !== 0) {
            return self::fail($rc);
        }
        $raw = self::$outI64->cdata;
        return new Success($raw << 32 >> 32);
    }

    /**
     * Casts integer text to a signed 64-bit value. Notation rules as {@see i8()}.
     *
     * @param string $text the text to cast
     * @param NumFormat $format the caller-declared numeric notation
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function i64(string $text, NumFormat $format): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        self::declare($format);
        self::$outI64->cdata = 0;
        $rc = $ffi->cast_i64(
            $text === '' ? null : $text,
            \strlen($text),
            self::$formatPtr,
            self::$outI64Ptr,
            self::$faultPtr
        );
        return $rc === 0 ? new Success(self::$outI64->cdata) : self::fail($rc);
    }

    /**
     * Casts integer text to an unsigned 8-bit value. Notation rules as {@see i8()}.
     *
     * @param string $text the text to cast
     * @param NumFormat $format the caller-declared numeric notation
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function u8(string $text, NumFormat $format): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        self::declare($format);
        self::$outI64->cdata = 0;
        $rc = $ffi->cast_u8(
            $text === '' ? null : $text,
            \strlen($text),
            self::$formatPtr,
            self::$outI64Ptr,
            self::$faultPtr
        );
        return $rc === 0 ? new Success(self::$outI64->cdata) : self::fail($rc);
    }

    /**
     * Casts integer text to an unsigned 16-bit value. Notation rules as {@see i8()}.
     *
     * @param string $text the text to cast
     * @param NumFormat $format the caller-declared numeric notation
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function u16(string $text, NumFormat $format): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        self::declare($format);
        self::$outI64->cdata = 0;
        $rc = $ffi->cast_u16(
            $text === '' ? null : $text,
            \strlen($text),
            self::$formatPtr,
            self::$outI64Ptr,
            self::$faultPtr
        );
        return $rc === 0 ? new Success(self::$outI64->cdata) : self::fail($rc);
    }

    /**
     * Casts integer text to an unsigned 32-bit value. Notation rules as {@see i8()}.
     *
     * @param string $text the text to cast
     * @param NumFormat $format the caller-declared numeric notation
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function u32(string $text, NumFormat $format): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        self::declare($format);
        self::$outI64->cdata = 0;
        $rc = $ffi->cast_u32(
            $text === '' ? null : $text,
            \strlen($text),
            self::$formatPtr,
            self::$outI64Ptr,
            self::$faultPtr
        );
        return $rc === 0 ? new Success(self::$outI64->cdata) : self::fail($rc);
    }

    /**
     * u64 comes back as PHP int's two's-complement bit pattern (PHP has no unsigned 64) —
     * render with sprintf('%u', ...), the same carrier choice as the Java binding.
     *
     * @param string $text the text to cast
     * @param NumFormat $format the caller-declared numeric notation
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function u64(string $text, NumFormat $format): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        self::declare($format);
        self::$outI64->cdata = 0;
        $rc = $ffi->cast_u64(
            $text === '' ? null : $text,
            \strlen($text),
            self::$formatPtr,
            self::$outI64Ptr,
            self::$faultPtr
        );
        return $rc === 0 ? new Success(self::$outI64->cdata) : self::fail($rc);
    }

    /**
     * Casts real text to an IEEE single (widened losslessly on readback): finite values
     * only, declared separators and grouping, parens, exponent, and trailing percent.
     *
     * @param string $text the text to cast
     * @param NumFormat $format the caller-declared numeric notation
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function f32(string $text, NumFormat $format): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        self::declare($format);
        $rc = $ffi->cast_f32(
            $text === '' ? null : $text,
            \strlen($text),
            self::$formatPtr,
            self::$outRealPtr,
            self::$faultPtr
        );
        return $rc === 0 ? new Success(self::$outReal->f32) : self::fail($rc);
    }

    /**
     * Casts real text to an IEEE double. Notation rules as {@see f32()}.
     *
     * @param string $text the text to cast
     * @param NumFormat $format the caller-declared numeric notation
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function f64(string $text, NumFormat $format): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        self::declare($format);
        $rc = $ffi->cast_f64(
            $text === '' ? null : $text,
            \strlen($text),
            self::$formatPtr,
            self::$outRealPtr,
            self::$faultPtr
        );
        return $rc === 0 ? new Success(self::$outReal->f64) : self::fail($rc);
    }

    /**
     * Casts decimal text to an exact {@see Decimal} — sign, 96-bit magnitude, base-10
     * scale 0..=28 — under the same grammar and NumFormat as the real doors. Never rounds:
     * only exact trailing zeros in the fraction are dropped, so the scale is canonical
     * ("1.10", "1.1" and "1.1000" are all 11 at scale 1; zero is always scale 0), and text
     * carrying more precision than the triple can hold is OutOfRange, not silently
     * approximated.
     *
     * @param string $text the text to cast
     * @param NumFormat $format the caller-declared numeric notation
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function decimal(string $text, NumFormat $format): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        self::declare($format);
        $rc = $ffi->cast_decimal(
            $text === '' ? null : $text,
            \strlen($text),
            self::$formatPtr,
            self::$outDecimalPtr,
            self::$faultPtr
        );
        return $rc === 0
            ? new Success(Decimal::fromLimbs(
                self::$outDecimal->lo,
                self::$outDecimal->hi,
                self::$outDecimal->scale,
                self::$outDecimal->negative !== 0
            ))
            : self::fail($rc);
    }

    /**
     * Casts UUID text — all five .NET Guid formats (D/N/B/P/X) plus urn:uuid:/GUID:/UUID:
     * prefixes — to PHP's UUID lingua franca: the lowercase hyphenated string.
     *
     * @param string $text the text to cast
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function uuid(string $text): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_uuid($text === '' ? null : $text, \strlen($text), self::$out16, self::$faultPtr);
        if ($rc !== 0) {
            return self::fail($rc);
        }
        return new Success(NativeValues::uuid(FFI::string(self::$out16, 16)));
    }

    /**
     * Casts UUID text — the same grammar as {@see uuid()} — to its 16 RFC 9562-ordered
     * bytes as a binary string, for a BINARY(16) column bind or a wire format. Skips the
     * hex encoding and hyphen assembly {@see uuid()} does to build the canonical string;
     * when bytes are the destination, that string is work you did not ask for.
     *
     * @param string $text the text to cast
     * @return Success|Fault the verdict: a Success carrying the 16 raw bytes, or a Fault
     */
    public static function uuidBytes(string $text): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_uuid($text === '' ? null : $text, \strlen($text), self::$out16, self::$faultPtr);
        return $rc === 0 ? new Success(FFI::string(self::$out16, 16)) : self::fail($rc);
    }

    /**
     * Builds the scratch pair's instant — createFromTimestamp/setMicrosecond on PHP 8.4+,
     * the date-string fallback below it.
     *
     * @return DateTimeImmutable the UTC instant, nanoseconds truncated to microseconds
     */
    private static function instant(): DateTimeImmutable
    {
        return NativeValues::instant(self::$outPair->seconds, self::$outPair->nanos);
    }

    /**
     * Casts an RFC 3339 instant — zone mandatory — to a UTC DateTimeImmutable.
     * Sub-microsecond nanoseconds truncate (PHP's ceiling).
     *
     * @param string $text the text to cast
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function timestamp(string $text): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_timestamp($text === '' ? null : $text, \strlen($text), self::$outPairPtr, self::$faultPtr);
        return $rc === 0 ? new Success(self::instant()) : self::fail($rc);
    }

    /**
     * Casts an integer Unix-epoch value under a caller-declared unit to a UTC
     * DateTimeImmutable.
     *
     * @param string $text the text to cast
     * @param UnixPrecision $precision the declared unit of the epoch value
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function unix(string $text, UnixPrecision $precision): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_unix(
            $text === '' ? null : $text,
            \strlen($text),
            $precision->value,
            self::$outPairPtr,
            self::$faultPtr
        );
        return $rc === 0 ? new Success(self::instant()) : self::fail($rc);
    }

    /**
     * Casts an Excel date serial under a caller-declared epoch to a UTC DateTimeImmutable.
     * The whole part counts days from the system's own day zero and the fraction is the
     * time of day, so "45292.75" is 2024-01-01T18:00:00Z. A cell carries no zone and none
     * is invented.
     *
     * The 1900 system contains a day that never existed: serial 60 is 1900-02-29, kept
     * deliberately because Lotus 1-2-3 wrongly treated 1900 as a leap year and Excel copied
     * the bug for file compatibility. It is OutOfRange here — the same verdict date() gives
     * the text "1900-02-29" — so every serial above it is shifted one day against a naive
     * count, which is the arithmetic hand-rolled conversions get wrong.
     *
     * @param string $text the text to cast
     * @param ExcelEpoch $epoch the declared date system
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function excelSerial(string $text, ExcelEpoch $epoch): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_excel_serial(
            $text === '' ? null : $text,
            \strlen($text),
            $epoch->value,
            self::$outPairPtr,
            self::$faultPtr
        );
        return $rc === 0 ? new Success(self::instant()) : self::fail($rc);
    }

    /**
     * Casts a calendar date to a UTC DateTimeImmutable at midnight (PHP has no date-only
     * type). With no order declared: the strict ISO 8601 yyyy-MM-dd form only. With a
     * declared DateOrder, also the separated forms — "1/7/2026" is January 7th (Mdy, the
     * en-US order) or July 1st (Dmy, the en-GB order) only because the caller said which.
     * Built from epoch arithmetic — Hinnant's days_from_civil, the same math the core
     * itself uses — not a date-string parse.
     *
     * @param string $text the text to cast
     * @param DateOrder|null $order the declared field order; null keeps the strict ISO door
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function date(string $text, ?DateOrder $order = null): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $order === null
            ? $ffi->cast_date($text === '' ? null : $text, \strlen($text), self::$outDatePtr, self::$faultPtr)
            : $ffi->cast_date_ordered(
                $text === '' ? null : $text,
                \strlen($text),
                $order->value,
                self::$outDatePtr,
                self::$faultPtr
            );
        if ($rc !== 0) {
            return self::fail($rc);
        }
        return new Success(NativeValues::date(self::$outDate->year, self::$outDate->month, self::$outDate->day));
    }

    /**
     * Casts a zone-less civil date-time — the shape untrusted feeds actually send
     * ("1/7/2026 3:04 PM", "2026-01-07 15:04:05") — under the caller-declared DateOrder.
     * The date part follows date()'s declared-order grammar; the optional time part (one
     * space or T after the date) is 24-hour h:mm[:ss[.f]] or 12-hour with an AM/PM
     * marker; absent, the time is midnight. PHP has no zone-less datetime type, so the
     * civil value rides a UTC-labeled DateTimeImmutable — the label is a carrier
     * artifact, not data: no zone was read and none was applied, and fusing a real zone
     * is the caller's job (timestamp() stays the strict RFC 3339 instant door).
     * Sub-microsecond nanoseconds truncate (PHP's ceiling).
     *
     * @param string $text the text to cast
     * @param DateOrder $order the declared field order
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function datetime(string $text, DateOrder $order): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_datetime(
            $text === '' ? null : $text,
            \strlen($text),
            $order->value,
            self::$outCivilPtr,
            self::$faultPtr
        );
        return $rc === 0 ? new Success(self::civil()) : self::fail($rc);
    }

    /**
     * The civil date-time in the scratch out-param on its UTC-labeled carrier (see
     * datetime() for why the label is not data), nanoseconds truncated to microseconds.
     *
     * @return DateTimeImmutable the civil value, labeled UTC
     */
    private static function civil(): DateTimeImmutable
    {
        return NativeValues::civil(
            self::$outCivil->year,
            self::$outCivil->month,
            self::$outCivil->day,
            self::$outCivil->nanos
        );
    }

    /**
     * Casts an ISO 24-hour time-of-day to an exact int of nanoseconds since midnight
     * (PHP has no time-only type; the integer keeps every digit).
     *
     * @param string $text the text to cast
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function time(string $text): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_time($text === '' ? null : $text, \strlen($text), self::$outI64Ptr, self::$faultPtr);
        return $rc === 0 ? new Success(self::$outI64->cdata) : self::fail($rc);
    }

    /**
     * Casts a duration (ISO 8601 fixed components, invariant colon form, or protobuf JSON
     * seconds) to the protobuf pair — see {@see Duration} for why not DateInterval.
     *
     * @param string $text the text to cast
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function duration(string $text): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_duration($text === '' ? null : $text, \strlen($text), self::$outPairPtr, self::$faultPtr);
        return $rc === 0
            ? new Success(new Duration(self::$outPair->seconds, self::$outPair->nanos))
            : self::fail($rc);
    }

    /**
     * Reads a number a caller already holds as the exact {@see Decimal} it names: the
     * shortest decimal that rounds back to the float, the digits a spreadsheet writes for
     * it — 0.1 is magnitude 1 at scale 1, not the binary fraction nearest it, and 0.1 + 0.2
     * is 0.30000000000000004. NAN is Malformed; INF, a magnitude past 2^96 - 1 or more than
     * 28 places is OutOfRange. A typed door's Fault has no span: offset and length are 0.
     *
     * @param float $value the number to read
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function decimalFromFloat(float $value): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_decimal_from_f64($value, self::$outDecimalPtr, self::$faultPtr);
        return $rc === 0
            ? new Success(Decimal::fromLimbs(
                self::$outDecimal->lo,
                self::$outDecimal->hi,
                self::$outDecimal->scale,
                self::$outDecimal->negative !== 0
            ))
            : self::fail($rc);
    }

    /**
     * Reads an Excel serial number under a caller-declared epoch as the zone-less civil
     * date-time it names — the twin of excelSerial() for a number a workbook reader already
     * holds, on the same UTC-labeled carrier datetime() uses (the label is not data). The
     * 1900 system's phantom serial 60 is OutOfRange; a negative, NAN or INF serial is
     * Malformed. Sub-microsecond nanoseconds truncate.
     *
     * @param float $value the serial
     * @param ExcelEpoch $epoch the declared date system
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function excelSerialFromFloat(float $value, ExcelEpoch $epoch): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_excel_serial_from_f64($value, $epoch->value, self::$outCivilPtr, self::$faultPtr);
        return $rc === 0 ? new Success(self::civil()) : self::fail($rc);
    }

    /**
     * Reads the fraction of an Excel serial number as an exact int of nanoseconds since
     * midnight, as time() does; 0.75 and 45292.75 are both 18:00.
     *
     * @param float $value the serial
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function excelTime(float $value): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_excel_time($value, self::$outI64Ptr, self::$faultPtr);
        return $rc === 0 ? new Success(self::$outI64->cdata) : self::fail($rc);
    }

    /**
     * Reads a number of days as the protobuf pair duration() returns: 1.5 is 129,600
     * seconds.
     *
     * @param float $value the number of days
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function excelDuration(float $value): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_excel_duration($value, self::$outPairPtr, self::$faultPtr);
        return $rc === 0
            ? new Success(new Duration(self::$outPair->seconds, self::$outPair->nanos))
            : self::fail($rc);
    }

    /**
     * Whether libhypercast resolved for this platform and exports the ABI this binding was
     * built against — the probe a consumer with a fallback gates on. Attempts the same load
     * every door makes, but never throws: a missing ext-ffi, an `ffi.enable` setting that
     * restricts FFI for this SAPI, a missing or unloadable library, an unsupported platform,
     * or a stale library that lacks a symbol this binding declares all answer false. The
     * answer is cached for the request; true exactly when {@see nativeVersion()} succeeds.
     *
     * @return bool true when every door can be called; false when the first one would throw
     */
    public static function isAvailable(): bool
    {
        if (self::$available !== null) {
            return self::$available;
        }
        try {
            self::nativeVersion();
            return self::$available = true;
        } catch (\Throwable) {
            return self::$available = false;
        }
    }

    /**
     * The version of the native library that actually loaded, as "major.minor.patch" —
     * the core's own manifest version, read through the zero-argument probe it exports, so
     * a host can prove the library it resolved is the one this binding was written against
     * before making the first cast. Throws when no library resolves; {@see isAvailable()}
     * is the non-throwing form.
     *
     * @return string the native library's semantic version as "major.minor.patch"
     */
    public static function nativeVersion(): string
    {
        $ffi = self::$ffi ?? self::load();
        return NativeValues::version($ffi->hypercast_version());
    }

    /**
     * The cdef load plus the static scratch allocations every door reuses — once per
     * request: PHP's statics reset between requests, so under a web SAPI the declarations
     * are bound again on each request's first door (the OS keeps the library itself mapped
     * for the worker's lifetime).
     *
     * @return FFI the bound library handle
     */
    private static function load(): FFI
    {
        // HYPERCAST_NATIVE_LIBRARY names a library to load instead of the staged one, so the
        // suite runs against a core built from the checkout without replacing committed files
        // (.github/scripts/local-core.sh builds one and prints it).
        $path = NativePlatform::libraryPath('hypercast', __DIR__, 'HYPERCAST_NATIVE_LIBRARY');

        $numeric = '(const char *ptr, size_t len, const void *format, void *out, void *fault)';
        $plain = '(const char *ptr, size_t len, void *out, void *fault)';
        self::$ffi = FFI::cdef(
            'typedef struct { uint32_t offset; uint32_t length; } hc_fault;'
            . 'typedef struct { int64_t seconds; int32_t nanos; } hc_pair;'
            . 'typedef struct { uint16_t year; uint8_t month; uint8_t day; } hc_date;'
            . 'typedef struct { uint16_t year; uint8_t month; uint8_t day; uint32_t pad;'
            . ' uint64_t nanos; } hc_civil;'
            . 'typedef union { float f32; double f64; } hc_real;'
            . 'typedef struct { uint64_t lo; uint32_t hi; uint8_t scale; uint8_t negative;'
            . ' uint8_t pad[2]; } hc_decimal;'
            . 'typedef struct { uint32_t decimal_sep; uint32_t group_sep; uint32_t flags; uint32_t currency_len;'
            . ' uint8_t currency[16]; } hc_format;'
            . 'uint32_t hypercast_version(void);'
            . "int cast_bool{$plain};"
            . "int cast_char{$plain};"
            . "int cast_i8{$numeric}; int cast_i16{$numeric}; int cast_i32{$numeric}; int cast_i64{$numeric};"
            . "int cast_u8{$numeric}; int cast_u16{$numeric}; int cast_u32{$numeric}; int cast_u64{$numeric};"
            . "int cast_f32{$numeric}; int cast_f64{$numeric}; int cast_decimal{$numeric};"
            . "int cast_uuid{$plain};"
            . "int cast_timestamp{$plain};"
            . 'int cast_unix(const char *ptr, size_t len, uint32_t precision, void *out, void *fault);'
            . 'int cast_excel_serial(const char *ptr, size_t len, uint32_t epoch, void *out, void *fault);'
            . "int cast_date{$plain};"
            . 'int cast_date_ordered(const char *ptr, size_t len, uint32_t order, void *out, void *fault);'
            . 'int cast_datetime(const char *ptr, size_t len, uint32_t order, void *out, void *fault);'
            . "int cast_time{$plain};"
            . "int cast_duration{$plain};"
            . 'int cast_decimal_from_f64(double value, void *out, void *fault);'
            . 'int cast_excel_serial_from_f64(double value, uint32_t epoch, void *out, void *fault);'
            . 'int cast_excel_time(double value, void *out, void *fault);'
            . 'int cast_excel_duration(double value, void *out, void *fault);',
            $path
        );
        self::$out16 = self::$ffi->new('uint8_t[16]');
        self::$outPair = self::$ffi->new('hc_pair');
        self::$outDate = self::$ffi->new('hc_date');
        self::$outCivil = self::$ffi->new('hc_civil');
        self::$outI64 = self::$ffi->new('int64_t');
        self::$outReal = self::$ffi->new('hc_real');
        self::$outDecimal = self::$ffi->new('hc_decimal');
        self::$fault = self::$ffi->new('hc_fault');
        self::$format = self::$ffi->new('hc_format');
        self::$outPairPtr = FFI::addr(self::$outPair);
        self::$outDatePtr = FFI::addr(self::$outDate);
        self::$outCivilPtr = FFI::addr(self::$outCivil);
        self::$outI64Ptr = FFI::addr(self::$outI64);
        self::$outRealPtr = FFI::addr(self::$outReal);
        self::$outDecimalPtr = FFI::addr(self::$outDecimal);
        self::$faultPtr = FFI::addr(self::$fault);
        self::$formatPtr = FFI::addr(self::$format);
        return self::$ffi;
    }
}
