<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use DateTimeImmutable;
use HyperCast\Cast;
use HyperCast\CastFailure;
use HyperCast\DateOrder;
use HyperCast\Decimal;
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

    /** The bytes door hands back the sixteen RFC-ordered octets, nothing rendered. */
    public function testUuidBytesAreTheRfcOrderedSixteen(): void
    {
        $verdict = Cast::uuidBytes('urn:uuid:01020304-0506-0708-090A-0B0C0D0E0F10');
        self::assertInstanceOf(Success::class, $verdict);
        self::assertSame(hex2bin('0102030405060708090a0b0c0d0e0f10'), $verdict->value);
        self::assertInstanceOf(Fault::class, Cast::uuidBytes('not-a-uuid'));
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

    public function testDeclaredCurrencySymbol(): void
    {
        $usd = new NumFormat('.', ',', NumFormat::ALL, '$');
        $this->assertSame('$', $usd->currency);
        $this->assertEquals(new Success(1234), Cast::i32('$1,234', $usd));
        $this->assertEquals(new Success(-5), Cast::i32('-$5', $usd));
        $this->assertEquals(new Success(-5), Cast::i32('$ -5', $usd));
        $this->assertEquals(new Success(-5), Cast::i32('($5)', $usd));
        $this->assertEquals(new Success(1234.5), Cast::f64('$1,234.50', $usd));
        // Trailing, multi-byte, and a symbol that happens to contain the group separator.
        $danish = new NumFormat(',', '.', NumFormat::ALL, 'kr.');
        $this->assertEquals(new Success(1234.5), Cast::f64('1.234,50 kr.', $danish));
        $euro = new NumFormat(',', '.', NumFormat::ALL, '€');
        $this->assertEquals(new Success(-1234), Cast::i32('(1.234 €)', $euro));
        // The symbol is accepted once; a second is data the grammar doesn't know.
        $this->assertInstanceOf(Fault::class, Cast::i32('$$5', $usd));
        // Without a declared symbol, the flag matches nothing — and "$" is just Malformed.
        $this->assertInstanceOf(Fault::class, Cast::i32('$5', NumFormat::invariant()));
    }

    public function testDeclaredCurrencyWithoutTheFlagIsMalformedAtTheSymbol(): void
    {
        $flagOff = new NumFormat('.', ',', NumFormat::ALL & ~NumFormat::CURRENCY, '$');
        $this->assertEquals(new Fault(CastFailure::Malformed, 0, 1), Cast::i32('$5', $flagOff));
        $this->assertEquals(new Fault(CastFailure::Malformed, 0, 1), Cast::f64('$5', $flagOff));
        $this->assertEquals(new Fault(CastFailure::Malformed, 0, 1), Cast::decimal('$5', $flagOff));
    }

    public function testFormatScratchClearsAShorterSymbolAfterALongerOne(): void
    {
        // The identity memo rewrites the packed struct per NumFormat instance; a 3-byte
        // symbol followed by a 1-byte one must leave no stale tail behind the new length.
        $danish = new NumFormat(',', '.', NumFormat::ALL, 'kr.');
        $usd = new NumFormat('.', ',', NumFormat::ALL, '$');
        $this->assertEquals(new Success(5), Cast::i32('5 kr.', $danish));
        $this->assertEquals(new Success(5), Cast::i32('$5', $usd));
        $this->assertInstanceOf(Fault::class, Cast::i32('5 kr.', $usd));
        $this->assertEquals(new Success(5), Cast::i32('5 kr.', $danish));
    }

    public function testCurrencySymbolValidationIsACallerBug(): void
    {
        foreach (['$5', 'US D', "kr\t", str_repeat('€', 6), "\xFF"] as $bad) {
            try {
                new NumFormat('.', ',', NumFormat::ALL, $bad);
                $this->fail("currency '{$bad}' should have been rejected");
            } catch (\InvalidArgumentException) {
                $this->addToAssertionCount(1);
            }
        }
        // Sixteen bytes exactly is the ceiling, not over it.
        $sixteen = str_repeat('€', 5) . 'k';
        $this->assertSame($sixteen, (new NumFormat('.', ',', NumFormat::ALL, $sixteen))->currency);
    }

    public function testDecimalIsExactAndTheScaleIsCanonical(): void
    {
        // Exact trailing zeros in the fraction are trimmed, so the scale is minimal.
        $verdict = Cast::decimal('1.10', NumFormat::invariant());
        $this->assertEquals(new Success(new Decimal('11', 1, false)), $verdict);
        $this->assertSame('1.1', (string) $verdict->value);
        $this->assertEquals($verdict, Cast::decimal('1.1', NumFormat::invariant()));
        $this->assertEquals($verdict, Cast::decimal('1.1000', NumFormat::invariant()));
        $this->assertEquals(new Success(new Decimal('1', 1, false)), Cast::decimal('0.1', NumFormat::invariant()));
        $this->assertEquals(new Success(new Decimal('1', 0, false)), Cast::decimal('1.0000', NumFormat::invariant()));
        // Only zeros are ever dropped: a whole number keeps its digits.
        $this->assertEquals(new Success(new Decimal('100', 0, false)), Cast::decimal('100', NumFormat::invariant()));
        $accounting = Cast::decimal('(1,234.50)', NumFormat::invariant());
        $this->assertEquals(new Success(new Decimal('12345', 1, true)), $accounting);
        $this->assertSame('-1234.5', (string) $accounting->value);
        $this->assertEquals(new Success(new Decimal('5', 1, false)), Cast::decimal('50%', NumFormat::invariant()));
        $this->assertSame('-0.025', (string) Cast::decimal('(2.5)%', NumFormat::invariant())->value);
        // Excess precision is a verdict, never a rounding.
        $tooPrecise = Cast::decimal('0.' . str_repeat('1', 29), NumFormat::invariant());
        $this->assertInstanceOf(Fault::class, $tooPrecise);
        $this->assertSame(CastFailure::OutOfRange, $tooPrecise->reason);
    }

    public function testDecimalZeroIsNeverNegative(): void
    {
        // Zero is scale 0 as well as never negative.
        $this->assertEquals(new Success(new Decimal('0', 0, false)), Cast::decimal('-0.00', NumFormat::invariant()));
        $this->assertSame('0', (string) Cast::decimal('-0.00', NumFormat::invariant())->value);
        $this->assertSame('0', (string) Cast::decimal('(0)', NumFormat::invariant())->value);
    }

    public function testDecimalMagnitudeRidesBeyondPhpIntAsDigits(): void
    {
        // 2^96 - 1: hi is all ones and lo's bit pattern is PHP's -1 — the two-limb renderer
        // must produce the digits without float and without signed-int wraparound.
        $max = '79228162514264337593543950335';
        $this->assertEquals(new Success(new Decimal($max, 0, false)), Cast::decimal($max, NumFormat::invariant()));
        $this->assertSame($max, Decimal::fromLimbs(-1, 0xFFFFFFFF, 0, false)->magnitude);
        // Exactly 2^64: lo is zero, hi is one.
        $this->assertSame('18446744073709551616', Decimal::fromLimbs(0, 1, 0, false)->magnitude);
        $this->assertEquals(
            new Success(new Decimal('18446744073709551616', 0, false)),
            Cast::decimal('18446744073709551616', NumFormat::invariant())
        );
        // Just past PHP_INT_MAX: hi is zero but lo's sign bit is set.
        $this->assertSame('9223372036854775808', Decimal::fromLimbs(PHP_INT_MIN, 0, 0, false)->magnitude);
        $this->assertSame('0', Decimal::fromLimbs(0, 0, 0, false)->magnitude);
        // With a scale the rendering pads the way the corpus pins it.
        $this->assertSame('-792281625142643375935439.50335', (string) new Decimal($max, 5, true));
        $this->assertSame('0.0000000000000000000000000001', (string) new Decimal('1', 28, false));
    }

    public function testDecimalToFloatIsTheLossyConvenience(): void
    {
        $this->assertSame(1234.5, Cast::decimal('1,234.50', NumFormat::invariant())->value->toFloat());
        $this->assertSame(-0.025, (new Decimal('25', 3, true))->toFloat());
    }

    public function testNativeVersionNamesTheLoadedLibrary(): void
    {
        $this->assertSame(self::crateVersion(), Cast::nativeVersion());
    }

    public function testIsAvailableIsTheNonThrowingProbe(): void
    {
        $this->assertTrue(Cast::isAvailable());
        // Cached and idempotent — the second answer is the first, no reload.
        $this->assertTrue(Cast::isAvailable());
    }

    public function testFromLocaleconvReadsThePlatformShape(): void
    {
        // A de_DE-shaped localeconv(): comma decimal, point grouping, the euro.
        $german = NumFormat::fromLocaleconv([
            'decimal_point' => ',',
            'thousands_sep' => '.',
            'currency_symbol' => '€',
        ]);
        $this->assertSame([',', '.', NumFormat::ALL, '€'], [
            $german->decimalSep, $german->groupSep, $german->flags, $german->currency,
        ]);
        $this->assertEquals(new Success(1234.5), Cast::f64('1.234,50 €', $german));
        // The C locale: empty thousands_sep and currency_symbol fall back to ',' and none.
        $c = NumFormat::fromLocaleconv(['decimal_point' => '.', 'thousands_sep' => '', 'currency_symbol' => '']);
        $this->assertSame(['.', ',', NumFormat::ALL, ''], [$c->decimalSep, $c->groupSep, $c->flags, $c->currency]);
        $this->assertEquals(new Success(1234), Cast::i32('1,234', $c));
        // Missing keys default the same way as empty ones.
        $bare = NumFormat::fromLocaleconv([]);
        $this->assertSame(['.', ',', ''], [$bare->decimalSep, $bare->groupSep, $bare->currency]);
    }

    /** The crate's own manifest version — walked up from here the way CorpusTest finds corpus/. */
    private static function crateVersion(): string
    {
        $dir = __DIR__;
        while ($dir !== '/') {
            $candidate = $dir . '/rust/Cargo.toml';
            if (is_file($candidate)) {
                self::assertSame(
                    1,
                    preg_match('/^version\s*=\s*"([^"]+)"/m', file_get_contents($candidate), $match),
                    'rust/Cargo.toml carries no package version'
                );
                return $match[1];
            }
            $dir = \dirname($dir);
        }
        self::fail('rust/Cargo.toml not found');
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
