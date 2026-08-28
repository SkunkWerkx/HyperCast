<?php

declare(strict_types=1);

namespace HyperCast;

use DateTimeImmutable;
use DateTimeZone;
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
 * PHP-flavored fidelity, stated honestly: int is 64-bit signed, so u64 comes back as the
 * two's-complement bit pattern (render with sprintf('%u', ...)); DateTimeImmutable tops
 * out at microseconds, so the core's nanoseconds truncate by three digits on the instant
 * doors; time-of-day is an exact int of nanoseconds since midnight; durations come back as
 * the protobuf pair ({@see Duration}) because DateInterval can't carry them.
 */
final class Cast
{
    private static ?FFI $ffi = null;

    private function __construct()
    {
    }

    /**
     * Presents a verdict optionally: an Empty fault becomes null (PHP's absent),
     * everything else flows through untouched.
     */
    public static function optional(Success|Fault $verdict): Success|Fault|null
    {
        return $verdict instanceof Fault && $verdict->reason === CastFailure::Empty ? null : $verdict;
    }

    /**
     * Casts boolean text: true/false plus the conventions untrusted sources actually send
     * (t/f, yes/no, y/n, 1/0, on/off, enabled/disabled, active/inactive,
     * checked/unchecked, in/out), ASCII case-insensitive.
     */
    public static function bool(string $text): Success|Fault
    {
        return self::plain('cast_bool', $text, 'uint8_t', static fn($out) => $out->cdata !== 0);
    }

    /**
     * Integer doors: the target type's own range, declared grouping, accounting parens,
     * non-negative exponent, and 0x/&H/0b two's-complement radix prefixes.
     */
    public static function i8(string $text, NumFormat $format): Success|Fault
    {
        return self::numeric('cast_i8', $text, $format, 'int8_t', static fn($out) => $out->cdata);
    }

    public static function i16(string $text, NumFormat $format): Success|Fault
    {
        return self::numeric('cast_i16', $text, $format, 'int16_t', static fn($out) => $out->cdata);
    }

    public static function i32(string $text, NumFormat $format): Success|Fault
    {
        return self::numeric('cast_i32', $text, $format, 'int32_t', static fn($out) => $out->cdata);
    }

    public static function i64(string $text, NumFormat $format): Success|Fault
    {
        return self::numeric('cast_i64', $text, $format, 'int64_t', static fn($out) => $out->cdata);
    }

    public static function u8(string $text, NumFormat $format): Success|Fault
    {
        return self::numeric('cast_u8', $text, $format, 'uint8_t', static fn($out) => $out->cdata);
    }

    public static function u16(string $text, NumFormat $format): Success|Fault
    {
        return self::numeric('cast_u16', $text, $format, 'uint16_t', static fn($out) => $out->cdata);
    }

    public static function u32(string $text, NumFormat $format): Success|Fault
    {
        return self::numeric('cast_u32', $text, $format, 'uint32_t', static fn($out) => $out->cdata);
    }

    /**
     * u64 comes back as PHP int's two's-complement bit pattern (PHP has no unsigned 64) —
     * render with sprintf('%u', ...), the same carrier choice as the Java binding.
     */
    public static function u64(string $text, NumFormat $format): Success|Fault
    {
        return self::numeric('cast_u64', $text, $format, 'uint64_t', static fn($out) => $out->cdata);
    }

    /** Real doors: finite values only, declared separators, parens, exponent, percent. */
    public static function f32(string $text, NumFormat $format): Success|Fault
    {
        return self::numeric('cast_f32', $text, $format, 'float', static fn($out) => $out->cdata);
    }

    public static function f64(string $text, NumFormat $format): Success|Fault
    {
        return self::numeric('cast_f64', $text, $format, 'double', static fn($out) => $out->cdata);
    }

    /**
     * Casts UUID text — all five .NET Guid formats (D/N/B/P/X) plus urn:uuid:/GUID:/UUID:
     * prefixes — to PHP's UUID lingua franca: the lowercase hyphenated string.
     */
    public static function uuid(string $text): Success|Fault
    {
        return self::rawOut('cast_uuid', $text, 16, static function (string $bytes) {
            $hex = bin2hex($bytes);
            return sprintf(
                '%s-%s-%s-%s-%s',
                substr($hex, 0, 8),
                substr($hex, 8, 4),
                substr($hex, 12, 4),
                substr($hex, 16, 4),
                substr($hex, 20, 12)
            );
        });
    }

    /**
     * Casts an RFC 3339 instant — zone mandatory — to a UTC DateTimeImmutable.
     * Sub-microsecond nanoseconds truncate (PHP's ceiling).
     */
    public static function timestamp(string $text): Success|Fault
    {
        return self::rawOut('cast_timestamp', $text, 16, self::instant(...));
    }

    /** Casts an integer Unix-epoch value under a caller-declared unit to a UTC DateTimeImmutable. */
    public static function unix(string $text, UnixPrecision $precision): Success|Fault
    {
        $out = self::ffi()->new('uint8_t[16]');
        $fault = self::ffi()->new('uint32_t[2]');
        $rc = self::ffi()->cast_unix(
            $text === '' ? null : $text,
            \strlen($text),
            $precision->value,
            $out,
            $fault
        );
        return self::verdict($rc, $fault, fn() => self::instant(FFI::string($out, 16)));
    }

    /**
     * Casts a strict ISO 8601 yyyy-MM-dd calendar date to a UTC DateTimeImmutable at
     * midnight (PHP has no date-only type).
     */
    public static function date(string $text): Success|Fault
    {
        return self::rawOut('cast_date', $text, 4, static function (string $bytes) {
            ['year' => $year, 'month' => $month, 'day' => $day] =
                unpack('vyear/Cmonth/Cday', $bytes);
            return new DateTimeImmutable(
                sprintf('%04d-%02d-%02d 00:00:00', $year, $month, $day),
                new DateTimeZone('UTC')
            );
        });
    }

    /**
     * Casts an ISO 24-hour time-of-day to an exact int of nanoseconds since midnight
     * (PHP has no time-only type; the integer keeps every digit).
     */
    public static function time(string $text): Success|Fault
    {
        return self::rawOut('cast_time', $text, 8, static fn(string $bytes) => unpack('Pnanos', $bytes)['nanos']);
    }

    /**
     * Casts a duration (ISO 8601 fixed components, invariant colon form, or protobuf JSON
     * seconds) to the protobuf pair — see {@see Duration} for why not DateInterval.
     */
    public static function duration(string $text): Success|Fault
    {
        return self::rawOut('cast_duration', $text, 16, static function (string $bytes) {
            ['seconds' => $seconds, 'nanos' => $nanos] = unpack('qseconds/lnanos', $bytes);
            return new Duration($seconds, $nanos);
        });
    }

    private static function instant(string $bytes): DateTimeImmutable
    {
        ['seconds' => $seconds, 'nanos' => $nanos] = unpack('qseconds/lnanos', $bytes);
        $base = new DateTimeImmutable("@{$seconds}");
        $micros = intdiv($nanos, 1000);
        return $micros === 0 ? $base : $base->modify("+{$micros} microseconds");
    }

    private static function verdict(int $rc, $fault, callable $read): Success|Fault
    {
        if ($rc === 0) {
            return new Success($read());
        }
        if ($rc === -1) {
            throw new \RuntimeException(
                'hypercast: libhypercast reported a contract violation — a binding bug, please report it'
            );
        }
        return new Fault(CastFailure::from($rc), $fault[0], $fault[1]);
    }

    private static function plain(string $symbol, string $text, string $outType, callable $read): Success|Fault
    {
        $out = self::ffi()->new($outType);
        $fault = self::ffi()->new('uint32_t[2]');
        $rc = self::ffi()->{$symbol}($text === '' ? null : $text, \strlen($text), FFI::addr($out), $fault);
        return self::verdict($rc, $fault, static fn() => $read($out));
    }

    private static function rawOut(string $symbol, string $text, int $outBytes, callable $read): Success|Fault
    {
        $out = self::ffi()->new("uint8_t[{$outBytes}]");
        $fault = self::ffi()->new('uint32_t[2]');
        $rc = self::ffi()->{$symbol}($text === '' ? null : $text, \strlen($text), $out, $fault);
        return self::verdict($rc, $fault, static fn() => $read(FFI::string($out, $outBytes)));
    }

    private static function numeric(
        string $symbol,
        string $text,
        NumFormat $format,
        string $outType,
        callable $read,
    ): Success|Fault {
        [$decimal, $group] = $format->codePoints();
        $raw = self::ffi()->new('uint32_t[3]');
        $raw[0] = $decimal;
        $raw[1] = $group;
        $raw[2] = $format->flags;
        $out = self::ffi()->new($outType);
        $fault = self::ffi()->new('uint32_t[2]');
        $rc = self::ffi()->{$symbol}(
            $text === '' ? null : $text,
            \strlen($text),
            $raw,
            FFI::addr($out),
            $fault
        );
        return self::verdict($rc, $fault, static fn() => $read($out));
    }

    private static function ffi(): FFI
    {
        if (self::$ffi !== null) {
            return self::$ffi;
        }

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
        return self::$ffi = FFI::cdef(
            "int cast_bool{$plain};"
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
    }
}
