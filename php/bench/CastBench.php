<?php

declare(strict_types=1);

namespace HyperCast\Bench;

use DateTimeImmutable;
use HyperCast\Cast;
use HyperCast\DateOrder;
use HyperCast\NumFormat;
use PhpBench\Attributes as Bench;

/**
 * phpbench pairs: hypercast doors vs PHP's closest parse — mirrors the other bindings'
 * harnesses. Honesty notes baked in: intval()/floatval() have no grouping knob and no
 * failure signal at all (bad input silently becomes 0 — a different contract than a
 * verdict); DateInterval speaks ISO-8601 durations but rejects negatives and fractions;
 * PHP has no stdlib UUID parser, so that door runs unopposed.
 */
#[Bench\BeforeMethods('warmUp')]
#[Bench\Iterations(5)]
#[Bench\Revs(1000)]
#[Bench\OutputTimeUnit('microseconds', precision: 3)]
final class CastBench
{
    private const INT = '1234567';
    private const INT_GROUPED = '1,234,567';
    private const FLOAT = '12345.6789';
    private const UUID = '01020304-0506-0708-090a-0b0c0d0e0f10';
    private const TIMESTAMP = '2026-01-02T15:04:05.123456+05:00';
    private const ISO_SPAN = 'PT1H30M15.5S';

    private NumFormat $invariant;

    public function warmUp(): void
    {
        $this->invariant = NumFormat::invariant();
        Cast::bool('true');
    }

    public function benchCastBool(): void
    {
        Cast::bool('true');
    }

    public function benchCastI32(): void
    {
        Cast::i32(self::INT, $this->invariant);
    }

    public function benchCastI32Grouped(): void
    {
        Cast::i32(self::INT_GROUPED, $this->invariant);
    }

    public function benchIntval(): void
    {
        // No grouping knob, and no failure signal — "abc" silently becomes 0.
        \intval(self::INT);
    }

    public function benchCastF64(): void
    {
        Cast::f64(self::FLOAT, $this->invariant);
    }

    public function benchFloatval(): void
    {
        \floatval(self::FLOAT);
    }

    public function benchCastUuid(): void
    {
        Cast::uuid(self::UUID);
    }

    public function benchCastTimestamp(): void
    {
        Cast::timestamp(self::TIMESTAMP);
    }

    public function benchDateTimeImmutableParse(): void
    {
        new DateTimeImmutable(self::TIMESTAMP);
    }

    public function benchCastDuration(): void
    {
        Cast::duration(self::ISO_SPAN);
    }

    public function benchDateInterval(): void
    {
        // ISO-8601 durations, but integer components only and no negatives — the closest
        // PHP has; PT1H30M15S here vs our PT1H30M15.5S is as close as its grammar goes.
        new \DateInterval('PT1H30M15S');
    }
    public function benchCastDateTimeMessy(): void
    {
        Cast::datetime('1/7/2026 3:04 PM', DateOrder::Mdy);
    }

    public function benchDateTimeImmutableCreateFromFormat(): void
    {
        // The PHP parse that accepts the same text: an explicit format, since
        // new DateTimeImmutable() reads "1/7/2026" as m/d/Y by its own convention only
        // when the separators say so — this is the like-for-like.
        DateTimeImmutable::createFromFormat('n/j/Y g:i A', '1/7/2026 3:04 PM');
    }

    public function benchCastDateOrdered(): void
    {
        Cast::date('1/7/2026', DateOrder::Mdy);
    }

    public function benchCastF64Detect(): void
    {
        Cast::f64('1.234.567,89', NumFormat::detect());
    }

    public function benchCastF64Declared(): void
    {
        // Hoisted, not constructed per call: a fresh NumFormat would measure object
        // construction *and* defeat Cast's identity memo on the packed format struct.
        Cast::f64('1.234.567,89', self::eurozone());
    }

    private static function eurozone(): NumFormat
    {
        static $eurozone = null;
        return $eurozone ??= new NumFormat(',', '.', NumFormat::ALL);
    }

}
