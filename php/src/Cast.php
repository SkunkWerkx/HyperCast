<?php

declare(strict_types=1);

namespace HyperCast;

use DateTimeImmutable;
use FFI;

/**
 * Allocation-lean scalar casts — booleans, numerics, UUIDs, temporals — calling directly
 * into the native libhypercast shared library via PHP's built-in ext-ffi, no runtime
 * bridge and no Composer dependency. Every door returns the `Success|Fault` verdict union;
 * never an exception for bad data — an exception here is a caller bug (a malformed
 * NumFormat), never data.
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
 * the protobuf pair ({@see Duration}) because DateInterval can't carry them.
 */
final class Cast
{
    private static ?FFI $ffi = null;
    private static ?FFI\CData $out16 = null;
    private static ?FFI\CData $outPair = null;
    private static ?FFI\CData $outDate = null;
    private static ?FFI\CData $outI64 = null;
    private static ?FFI\CData $outReal = null;
    private static ?FFI\CData $fault = null;
    private static ?FFI\CData $format = null;
    // Pre-taken addresses: FFI auto-decays arrays to pointers but not scalars/structs, and
    // FFI::addr() per call would be a fresh CData allocation on the hot path.
    private static ?FFI\CData $outPairPtr = null;
    private static ?FFI\CData $outDatePtr = null;
    private static ?FFI\CData $outI64Ptr = null;
    private static ?FFI\CData $outRealPtr = null;
    private static ?FFI\CData $faultPtr = null;
    private static ?NumFormat $formatKey = null;
    private static bool $fastInstants = false;

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
        return new Fault(CastFailure::from($rc), self::$fault->offset, self::$fault->length);
    }

    /**
     * Re-stores the declared format only when it actually changes (identity check).
     *
     * @param NumFormat $format the caller-declared numeric notation
     * @return void
     */
    private static function declare(NumFormat $format): void
    {
        if (self::$formatKey !== $format) {
            [$decimal, $group] = $format->codePoints();
            self::$format[0] = $decimal;
            self::$format[1] = $group;
            self::$format[2] = $format->flags;
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
     * Integer doors: the target type's own range, declared grouping, accounting parens,
     * non-negative exponent, and 0x/&H/0b two's-complement radix prefixes. Every width
     * funnels through one zeroed 64-bit scratch slot (the supported RIDs are all
     * little-endian); narrow signed widths sign-extend on readback.
     *
     * @param string $text the text to cast
     * @param NumFormat $format the caller-declared numeric notation
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function i8(string $text, NumFormat $format): Success|Fault
    {
        return self::integer('cast_i8', $text, $format);
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
        return self::integer('cast_i16', $text, $format);
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
        return self::integer('cast_i32', $text, $format);
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
        return self::integer('cast_i64', $text, $format);
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
        return self::integer('cast_u8', $text, $format);
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
        return self::integer('cast_u16', $text, $format);
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
        return self::integer('cast_u32', $text, $format);
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
        return self::integer('cast_u64', $text, $format);
    }

    /**
     * The shared body of the integer doors: one zeroed 64-bit scratch slot, one FFI call.
     *
     * @param string $symbol the native door symbol
     * @param string $text the text to cast
     * @param NumFormat $format the caller-declared numeric notation
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    private static function integer(string $symbol, string $text, NumFormat $format): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        self::declare($format);
        self::$outI64->cdata = 0;
        $rc = $ffi->{$symbol}(
            $text === '' ? null : $text, \strlen($text), self::$format, self::$outI64Ptr, self::$faultPtr
        );
        if ($rc !== 0) {
            return self::fail($rc);
        }
        $raw = self::$outI64->cdata;
        return new Success(match ($symbol) {
            'cast_i8' => $raw << 56 >> 56,
            'cast_i16' => $raw << 48 >> 48,
            'cast_i32' => $raw << 32 >> 32,
            default => $raw,
        });
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
            $text === '' ? null : $text, \strlen($text), self::$format, self::$outRealPtr, self::$faultPtr
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
            $text === '' ? null : $text, \strlen($text), self::$format, self::$outRealPtr, self::$faultPtr
        );
        return $rc === 0 ? new Success(self::$outReal->f64) : self::fail($rc);
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
        $hex = bin2hex(FFI::string(self::$out16, 16));
        return new Success(
            substr($hex, 0, 8) . '-' . substr($hex, 8, 4) . '-' . substr($hex, 12, 4)
            . '-' . substr($hex, 16, 4) . '-' . substr($hex, 20)
        );
    }

    /**
     * Builds the scratch pair's instant — createFromTimestamp/setMicrosecond on PHP 8.4+,
     * the date-string fallback below it.
     *
     * @return DateTimeImmutable the UTC instant, nanoseconds truncated to microseconds
     */
    private static function instant(): DateTimeImmutable
    {
        $seconds = self::$outPair->seconds;
        $micros = intdiv(self::$outPair->nanos, 1000);
        if (self::$fastInstants) {
            $instant = DateTimeImmutable::createFromTimestamp($seconds);
            return $micros === 0 ? $instant : $instant->setMicrosecond($micros);
        }
        $instant = new DateTimeImmutable("@{$seconds}");
        return $micros === 0 ? $instant : $instant->modify("+{$micros} microseconds");
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
            $text === '' ? null : $text, \strlen($text), $precision->value, self::$outPairPtr, self::$faultPtr
        );
        return $rc === 0 ? new Success(self::instant()) : self::fail($rc);
    }

    /**
     * Casts a strict ISO 8601 yyyy-MM-dd calendar date to a UTC DateTimeImmutable at
     * midnight (PHP has no date-only type). Built from epoch arithmetic — Hinnant's
     * days_from_civil, the same math the core itself uses — not a date-string parse.
     *
     * @param string $text the text to cast
     * @return Success|Fault the verdict: a Success carrying the cast value, or a Fault
     */
    public static function date(string $text): Success|Fault
    {
        $ffi = self::$ffi ?? self::load();
        $rc = $ffi->cast_date($text === '' ? null : $text, \strlen($text), self::$outDatePtr, self::$faultPtr);
        if ($rc !== 0) {
            return self::fail($rc);
        }
        $year = self::$outDate->year;
        $month = self::$outDate->month;
        $day = self::$outDate->day;
        $shifted = $month <= 2 ? $year - 1 : $year;
        $era = intdiv($shifted >= 0 ? $shifted : $shifted - 399, 400);
        $yearOfEra = $shifted - $era * 400;
        $dayOfYear = intdiv(153 * ($month + ($month > 2 ? -3 : 9)) + 2, 5) + $day - 1;
        $dayOfEra = $yearOfEra * 365 + intdiv($yearOfEra, 4) - intdiv($yearOfEra, 100) + $dayOfYear;
        $seconds = ($era * 146_097 + $dayOfEra - 719_468) * 86_400;
        return new Success(
            self::$fastInstants
                ? DateTimeImmutable::createFromTimestamp($seconds)
                : new DateTimeImmutable("@{$seconds}")
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
     * One-time cdef load plus the static scratch allocations every door reuses.
     *
     * @return FFI the bound library handle
     */
    private static function load(): FFI
    {
        [$rid, $libName] = NativePlatform::ridAndLibraryName();
        $path = __DIR__ . "/native/{$rid}/{$libName}";
        if (!is_file($path)) {
            // Development loop: fall back to the in-repo cargo build, exactly what the
            // other bindings' local staging does.
            $repoBuild = \dirname(__DIR__, 2) . "/rust/target/release/{$libName}";
            if (is_file($repoBuild)) {
                $path = $repoBuild;
            }
        }
        if (!is_file($path)) {
            throw new \RuntimeException(
                "hypercast: {$path} not found (unsupported platform, or this package was built "
                . 'without a native library for it)'
            );
        }

        $numeric = '(const char *ptr, size_t len, const void *format, void *out, void *fault)';
        $plain = '(const char *ptr, size_t len, void *out, void *fault)';
        self::$ffi = FFI::cdef(
            'typedef struct { uint32_t offset; uint32_t length; } hc_fault;'
            . 'typedef struct { int64_t seconds; int32_t nanos; } hc_pair;'
            . 'typedef struct { uint16_t year; uint8_t month; uint8_t day; } hc_date;'
            . 'typedef union { float f32; double f64; } hc_real;'
            . "int cast_bool{$plain};"
            . "int cast_i8{$numeric}; int cast_i16{$numeric}; int cast_i32{$numeric}; int cast_i64{$numeric};"
            . "int cast_u8{$numeric}; int cast_u16{$numeric}; int cast_u32{$numeric}; int cast_u64{$numeric};"
            . "int cast_f32{$numeric}; int cast_f64{$numeric};"
            . "int cast_uuid{$plain};"
            . "int cast_timestamp{$plain};"
            . 'int cast_unix(const char *ptr, size_t len, uint32_t precision, void *out, void *fault);'
            . "int cast_date{$plain};"
            . "int cast_time{$plain};"
            . "int cast_duration{$plain};",
            $path
        );
        self::$out16 = self::$ffi->new('uint8_t[16]');
        self::$outPair = self::$ffi->new('hc_pair');
        self::$outDate = self::$ffi->new('hc_date');
        self::$outI64 = self::$ffi->new('int64_t');
        self::$outReal = self::$ffi->new('hc_real');
        self::$fault = self::$ffi->new('hc_fault');
        self::$format = self::$ffi->new('uint32_t[3]');
        self::$outPairPtr = FFI::addr(self::$outPair);
        self::$outDatePtr = FFI::addr(self::$outDate);
        self::$outI64Ptr = FFI::addr(self::$outI64);
        self::$outRealPtr = FFI::addr(self::$outReal);
        self::$faultPtr = FFI::addr(self::$fault);
        self::$fastInstants = method_exists(DateTimeImmutable::class, 'createFromTimestamp')
            && method_exists(DateTimeImmutable::class, 'setMicrosecond');
        return self::$ffi;
    }
}
