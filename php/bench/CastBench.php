<?php

declare(strict_types=1);

namespace HyperCast\Bench;

use DateTimeImmutable;
use HyperCast\Cast;
use HyperCast\DateOrder;
use HyperCast\DayPeriods;
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
    private NumFormat $enUs;
    private NumFormat $cultured;
    private \NumberFormatter $enUsFormatter;
    private DayPeriods $german;

    public function warmUp(): void
    {
        $this->invariant = NumFormat::invariant();
        // ext-intl is optional for the package but required for these two rows.
        $this->enUsFormatter = new \NumberFormatter('en_US', \NumberFormatter::DECIMAL);
        $this->enUs = NumFormat::fromNumberFormatter($this->enUsFormatter);
        // Every optional symbol declared: the core's full check of a format, where the
        // invariant one is decided on its separators alone.
        $this->cultured = new NumFormat(
            ',',
            '.',
            NumFormat::ALL | NumFormat::TRAILING_SIGN,
            '€',
            '.',
            ',',
            "\u{2212}",
            "\u{FF0B}",
            "\u{66A}",
            "\u{660}"
        );
        $this->german = new DayPeriods('vorm.', 'nachm.');
        Cast::bool('true');
    }

    public function benchCastBool(): void
    {
        Cast::bool('true');
    }

    public function benchCastChar(): void
    {
        Cast::char('U+00E9');
    }

    public function benchCastI32(): void
    {
        Cast::i32(self::INT, $this->invariant);
    }

    public function benchCastI32Grouped(): void
    {
        Cast::i32(self::INT_GROUPED, $this->invariant);
    }

    /** The same grouped text under a format derived once from an intl formatter. */
    public function benchCastI32GroupedFromFormatter(): void
    {
        Cast::i32(self::INT_GROUPED, $this->enUs);
    }

    /** The formatter itself passed to the door, which declares its symbols per call. */
    public function benchCastI32GroupedFormatter(): void
    {
        Cast::i32(self::INT_GROUPED, $this->enUsFormatter);
    }

    /** The same grouped text under a format with every optional symbol declared. */
    public function benchCastI32GroupedCultured(): void
    {
        Cast::i32('1.234.567', $this->cultured);
    }

    /** A negative written the way that format's culture writes one: trailing, U+2212. */
    public function benchCastI32TrailingMinus(): void
    {
        Cast::i32("1.234.567 \u{2212}", $this->cultured);
    }

    /** The rejection path: a fault leaves the fast lanes for the engine that owns its span. */
    public function benchCastI32Malformed(): void
    {
        Cast::i32('1,234x567', $this->invariant);
    }

    /** PHP's own validating integer parse, failing on the same mistake. */
    public function benchFilterVarIntMalformed(): void
    {
        \filter_var('1234x567', \FILTER_VALIDATE_INT);
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

    /** The same civil shape with a declared day-period pair, read in the culture's own words. */
    public function benchCastDateTimeMessyDayPeriods(): void
    {
        Cast::datetime('1/7/2026 3:04 nachm.', DateOrder::Dmy, $this->german);
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
