<?php

declare(strict_types=1);

namespace HyperCast\Interop;

use DateTimeImmutable;
use FFI;
use HyperCast\CastFailure;
use HyperCast\Fault;
use HyperCast\NumFormat;

/**
 * The native core's C ABI values, presented: every out-value a door fills, built from the
 * fields the core writes; the 32-byte numeric format it reads; the verdict code and span of a
 * fault; and the packed version word. Public for a package that carries HyperCast's verdicts
 * across a C ABI of its own (HyperTabular does) — it reads the fields out of its own buffers
 * and builds each value through exactly the conversion every {@see \HyperCast\Cast} door
 * uses, so a value it hands out is the value the door would have.
 *
 * A Decimal is built by {@see \HyperCast\Decimal::fromLimbs()}, a span by the
 * {@see \HyperCast\Duration} constructor; both are public already.
 */
final class NativeValues
{
    private static ?bool $fastInstants = null;

    /** Static-only — never instantiated. */
    private function __construct()
    {
    }

    /**
     * An instant — the timestamp, Unix and Excel-serial doors' value — from the core's
     * protobuf pair, on PHP 8.4's createFromTimestamp/setMicrosecond where there is one and
     * the date-string route below it.
     *
     * @param int $seconds whole seconds from the Unix epoch
     * @param int $nanos the nanoseconds after them, 0–999,999,999
     * @return DateTimeImmutable the UTC instant, nanoseconds truncated to microseconds (PHP's ceiling)
     */
    public static function instant(int $seconds, int $nanos): DateTimeImmutable
    {
        return self::at($seconds, intdiv($nanos, 1000));
    }

    /**
     * A calendar date — the date doors' value — as UTC midnight of that day: PHP has no
     * date-only type, and the UTC label is a carrier artifact, not data.
     *
     * @param int $year the year
     * @param int $month the month, 1–12
     * @param int $day the day of the month
     * @return DateTimeImmutable midnight of the date, labeled UTC
     */
    public static function date(int $year, int $month, int $day): DateTimeImmutable
    {
        return self::at(self::epochSeconds($year, $month, $day), 0);
    }

    /**
     * A zone-less wall clock — the date-time doors' value — on a UTC-labeled carrier: PHP
     * has no zone-less datetime type, so no zone was read and none was applied.
     *
     * @param int $year the year
     * @param int $month the month, 1–12
     * @param int $day the day of the month
     * @param int $nanosOfDay nanoseconds since that day's midnight
     * @return DateTimeImmutable the wall clock, labeled UTC, nanoseconds truncated to microseconds
     */
    public static function civil(int $year, int $month, int $day, int $nanosOfDay): DateTimeImmutable
    {
        return self::at(
            self::epochSeconds($year, $month, $day) + intdiv($nanosOfDay, 1_000_000_000),
            intdiv($nanosOfDay % 1_000_000_000, 1000)
        );
    }

    /**
     * A UUID — the uuid door's value — as PHP's lingua franca, the lowercase hyphenated
     * string.
     *
     * @param string $bytes the 16 bytes, in RFC 9562 order
     * @return string the canonical text
     */
    public static function uuid(string $bytes): string
    {
        $hex = bin2hex($bytes);
        return substr($hex, 0, 8) . '-' . substr($hex, 8, 4) . '-' . substr($hex, 12, 4)
            . '-' . substr($hex, 16, 4) . '-' . substr($hex, 20, 12);
    }

    /**
     * The fault a nonzero verdict code and its byte span name.
     *
     * @param int $code the verdict code: 1 empty, 2 malformed, 3 out of range
     * @param int $offset the span's byte offset
     * @param int $length the span's byte length
     * @return Fault the fault
     * @throws \LogicException $code names no reason — a binding bug, not data
     */
    public static function fault(int $code, int $offset, int $length): Fault
    {
        $reason = CastFailure::tryFrom($code)
            ?? throw new \LogicException("{$code} is not a verdict reason code — a binding bug, please report it");
        return new Fault($reason, $offset, $length);
    }

    /**
     * A native library's packed version word as "major.minor.patch".
     *
     * @param int $packed major << 16 | minor << 8 | patch, as a *_version() export returns it
     * @return string the version
     */
    public static function version(int $packed): string
    {
        return sprintf('%d.%d.%d', $packed >> 16, ($packed >> 8) & 0xFF, $packed & 0xFF);
    }

    /**
     * Writes a format into a native format struct — fields decimal_sep, group_sep, flags,
     * currency_len and currency[16], the core's layout — as every numeric door does: the
     * currency bytes are written as the full 16, zero-padded, so a struct reused for another
     * format keeps no tail. The format was validated when it was built.
     *
     * @param NumFormat $format the declared format
     * @param FFI\CData $target the struct to write
     * @return void
     */
    public static function writeFormat(NumFormat $format, FFI\CData $target): void
    {
        [$decimal, $group] = $format->codePoints();
        $target->decimal_sep = $decimal;
        $target->group_sep = $group;
        $target->flags = $format->flags;
        $target->currency_len = \strlen($format->currency);
        $bytes = NumFormat::CURRENCY_MAX_BYTES;
        FFI::memcpy($target->currency, str_pad($format->currency, $bytes, "\0"), $bytes);
    }

    /**
     * The UTC instant at $seconds plus $micros.
     *
     * @param int $seconds whole seconds from the Unix epoch
     * @param int $micros microseconds after them
     * @return DateTimeImmutable the instant
     */
    private static function at(int $seconds, int $micros): DateTimeImmutable
    {
        self::$fastInstants ??= method_exists(DateTimeImmutable::class, 'createFromTimestamp')
            && method_exists(DateTimeImmutable::class, 'setMicrosecond');
        if (self::$fastInstants) {
            $instant = DateTimeImmutable::createFromTimestamp($seconds);
            return $micros === 0 ? $instant : $instant->setMicrosecond($micros);
        }
        $instant = new DateTimeImmutable("@{$seconds}");
        return $micros === 0 ? $instant : $instant->modify("+{$micros} microseconds");
    }

    /**
     * Epoch seconds at midnight of a civil date — Hinnant's days_from_civil, the same math
     * the core itself uses.
     *
     * @param int $year the civil year
     * @param int $month the civil month
     * @param int $day the civil day
     * @return int seconds since the epoch at that date's midnight
     */
    private static function epochSeconds(int $year, int $month, int $day): int
    {
        $shifted = $month <= 2 ? $year - 1 : $year;
        $era = intdiv($shifted >= 0 ? $shifted : $shifted - 399, 400);
        $yearOfEra = $shifted - $era * 400;
        $dayOfYear = intdiv(153 * ($month + ($month > 2 ? -3 : 9)) + 2, 5) + $day - 1;
        $dayOfEra = $yearOfEra * 365 + intdiv($yearOfEra, 4) - intdiv($yearOfEra, 100) + $dayOfYear;
        return ($era * 146_097 + $dayOfEra - 719_468) * 86_400;
    }
}
