<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use DateTimeImmutable;
use HyperCast\Cast;
use HyperCast\CastFailure;
use HyperCast\DateOrder;
use HyperCast\Duration;
use HyperCast\Fault;
use HyperCast\NumFormat;
use HyperCast\Success;
use HyperCast\UnixPrecision;
use PHPUnit\Framework\TestCase;

/**
 * Binding-level behavior the corpus can't express: union consumption, PHP-flavored
 * fidelity (microsecond DateTimeImmutable, the u64 bit-pattern carrier, the Duration
 * pair), and the caller-bug guards.
 */
final class CastTest extends TestCase
{
    public function testMatchConsumesTheUnion(): void
    {
        $verdict = Cast::i32('(1,234)', NumFormat::invariant());
        $rendered = match (true) {
            $verdict instanceof Success => "ok {$verdict->value}",
            $verdict instanceof Fault => "fault {$verdict->reason->name}",
        };
        $this->assertSame('ok -1234', $rendered);
    }

    public function testFaultSpanPointsAtTheOffendingByte(): void
    {
        $this->assertEquals(
            new Fault(CastFailure::Malformed, 4, 1),
            Cast::i32('  12x4', NumFormat::invariant())
        );
    }

    public function testDeclaredSeparators(): void
    {
        $eurozone = new NumFormat(',', '.', NumFormat::ALL);
        $this->assertEquals(new Success(1234.5), Cast::f64('1.234,5', $eurozone));
        $french = new NumFormat(',', ' ', NumFormat::ALL);
        $this->assertEquals(new Success(1234.5), Cast::f64('1 234,5', $french));
    }

    public function testU64CarriesTheBitPattern(): void
    {
        $verdict = Cast::u64('18446744073709551615', NumFormat::invariant());
        $this->assertInstanceOf(Success::class, $verdict);
        $this->assertSame(-1, $verdict->value);
        $this->assertSame('18446744073709551615', sprintf('%u', $verdict->value));
    }

    public function testUuidMatchesTheCanonicalShape(): void
    {
        $this->assertEquals(
            new Success('01020304-0506-0708-090a-0b0c0d0e0f10'),
            Cast::uuid('urn:uuid:01020304-0506-0708-090A-0B0C0D0E0F10')
        );
    }

    public function testTimestampTruncatesToMicroseconds(): void
    {
        $expected = (new DateTimeImmutable('@1767348245'))->modify('+123456 microseconds');
        $this->assertEquals(new Success($expected), Cast::timestamp('2026-01-02T15:04:05.123456789+05:00'));
    }

    public function testUnixMapsTheDeclaredPrecision(): void
    {
        $this->assertEquals(
            new Success(new DateTimeImmutable('@-1')),
            Cast::unix('-1', UnixPrecision::Seconds)
        );
    }

    public function testDurationPair(): void
    {
        $this->assertEquals(new Success(new Duration(1, 500_000_000)), Cast::duration('PT1.5S'));
        $this->assertEquals(new Success(new Duration(-1, -500_000_000)), Cast::duration('-1.5s'));
        $verdict = Cast::duration('315576000000s');
        $this->assertInstanceOf(Success::class, $verdict);
        $this->assertSame(315_576_000_000, $verdict->value->seconds);
    }

    public function testOptionalPresentsEmptyAsNull(): void
    {
        $this->assertNull(Cast::optional(Cast::i32('   ', NumFormat::invariant())));
        $this->assertEquals(new Success(42), Cast::optional(Cast::i32('42', NumFormat::invariant())));
    }

    public function testEqualSeparatorsAreACallerBug(): void
    {
        $this->expectException(\InvalidArgumentException::class);
        new NumFormat('.', '.', NumFormat::ALL);
    }
    public function testDateOrderDisambiguatesLikeTheCulturesDo(): void
    {
        // The canonical ambiguity: 1/7/2026 is January 7th under en-US's month-first short
        // dates and July 1st under en-GB's day-first ones — resolved only by declaration.
        $enUs = Cast::date('1/7/2026', DateOrder::Mdy);
        $enGb = Cast::date('1/7/2026', DateOrder::Dmy);
        self::assertInstanceOf(Success::class, $enUs);
        self::assertInstanceOf(Success::class, $enGb);
        self::assertSame('2026-01-07', $enUs->value->format('Y-m-d'));
        self::assertSame('2026-07-01', $enGb->value->format('Y-m-d'));
        // Undeclared, the door stays strict ISO — the ambiguity is never guessed at.
        $undeclared = Cast::date('1/7/2026');
        self::assertInstanceOf(Fault::class, $undeclared);
        self::assertSame(CastFailure::Malformed, $undeclared->reason);
    }

    public function testDateTimeReadsTheMessyCivilShapes(): void
    {
        // The AM/PM world, zone-less: the UTC label on the carrier is an artifact, not
        // data — no zone was read and none was applied.
        $enUs = Cast::datetime('1/7/2026 3:04 PM', DateOrder::Mdy);
        $enGb = Cast::datetime('1/7/2026 3:04 PM', DateOrder::Dmy);
        self::assertInstanceOf(Success::class, $enUs);
        self::assertInstanceOf(Success::class, $enGb);
        self::assertSame('2026-01-07 15:04:00', $enUs->value->format('Y-m-d H:i:s'));
        self::assertSame('2026-07-01 15:04:00', $enGb->value->format('Y-m-d H:i:s'));
        // A zone suffix is not this door's business — timestamp() is the instant door.
        self::assertInstanceOf(Fault::class, Cast::datetime('1/7/2026 15:04:05Z', DateOrder::Mdy));
    }

}
