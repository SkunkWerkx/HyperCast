<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use DateTimeImmutable;
use HyperCast\Cast;
use HyperCast\CastFailure;
use HyperCast\DateOrder;
use HyperCast\DayPeriods;
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

    /** The char door's carrier is the scalar's UTF-8, every width of it. */
    public function testCharIsTheScalarsUtf8(): void
    {
        $this->assertSame('A', Cast::char('U+0041')->value);
        $this->assertSame("\u{E9}", Cast::char('&#233;')->value);
        $this->assertSame("\u{20AC}", Cast::char('0x20AC')->value);
        $this->assertSame("\u{1F600}", Cast::char('&#x1F600;')->value);
        $this->assertSame("\u{1F600}", Cast::char("\u{1F600}")->value);
        $this->assertSame(' ', Cast::char(' ')->value);
        $this->assertEquals(new Fault(CastFailure::Malformed, 2, 2), Cast::char("U+\u{E9}9"));
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
        $this->assertInstanceOf(Success::class, $verdict);
        $this->assertSame(hex2bin('0102030405060708090a0b0c0d0e0f10'), $verdict->value);
        $this->assertInstanceOf(Fault::class, Cast::uuidBytes('not-a-uuid'));
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

    public function testDeclaredSignsAreReadBesideTheAsciiOnes(): void
    {
        // Estonian's and Basque's minus is U+2212: declared, it is read; '-' still is too.
        $format = new NumFormat('.', ',', NumFormat::ALL, '$', minusSign: "\u{2212}", plusSign: "\u{FF0B}");
        $this->assertEquals(new Success(-5), Cast::i32("\u{2212}5", $format));
        $this->assertEquals(new Success(-5), Cast::i32('-5', $format));
        $this->assertEquals(new Success(5), Cast::i32("\u{FF0B}5", $format));
        $this->assertEquals(new Success(-5.0), Cast::f64("\u{2212}\$5", $format));
        $this->assertEquals(new Success(-5.0), Cast::f64("\$\u{2212}5", $format));
        // Two signs is Malformed at the later one, whichever spelling each is.
        $this->assertEquals(new Fault(CastFailure::Malformed, 3, 1), Cast::i32("\u{2212}-5", $format));
        // Undeclared, U+2212 is just three bytes the grammar does not know.
        $this->assertEquals(new Fault(CastFailure::Malformed, 0, 3), Cast::i32("\u{2212}5", NumFormat::invariant()));
    }

    public function testTrailingSignIsOptIn(): void
    {
        $trailing = new NumFormat('.', ',', NumFormat::ALL | NumFormat::TRAILING_SIGN, '€', minusSign: "\u{2212}");
        $this->assertEquals(new Success(-5), Cast::i32('5-', $trailing));
        $this->assertEquals(new Success(-5), Cast::i32('5 -', $trailing));
        $this->assertEquals(new Success(5), Cast::i32('5+', $trailing));
        $this->assertEquals(new Success(-5), Cast::i32('5 €-', $trailing));
        $this->assertEquals(new Success(-5), Cast::i32("5 €\u{2212}", $trailing));
        $this->assertEquals(new Success(-5), Cast::i32('5- €', $trailing));
        $this->assertEquals(new Success(-0.025), Cast::f64('2.5%-', $trailing));
        $this->assertEquals(new Success(new Decimal('25', 1, true)), Cast::decimal('2.5-', $trailing));
        // Never beside a leading sign, never inside parentheses, and never without the flag:
        // ALL does not include it, since a culture's negative pattern decides.
        $this->assertEquals(new Fault(CastFailure::Malformed, 2, 1), Cast::i32('-5-', $trailing));
        $this->assertEquals(new Fault(CastFailure::Malformed, 2, 1), Cast::i32('(5-)', $trailing));
        $this->assertEquals(new Fault(CastFailure::Malformed, 1, 1), Cast::i32('5-', NumFormat::invariant()));
        $this->assertSame(0, NumFormat::ALL & NumFormat::TRAILING_SIGN);
    }

    public function testCurrencyAmountsReadWithTheCurrencySeparators(): void
    {
        // A locale that writes a number with ',' and '.' but an amount the other way round:
        // the currency separators apply exactly when the symbol is found in the text.
        $format = new NumFormat(',', '.', NumFormat::ALL, '€', currencyDecimalSep: '.', currencyGroupSep: ',');
        $this->assertEquals(new Success(1234.5), Cast::f64('€1,234.50', $format));
        $this->assertEquals(new Success(1234.5), Cast::f64('1,234.50 €', $format));
        $this->assertEquals(new Success(1234.5), Cast::f64('1.234,50', $format));
        $this->assertEquals(new Fault(CastFailure::Malformed, 5, 1), Cast::f64('1,234.50', $format));
        $this->assertEquals(new Fault(CastFailure::Malformed, 4, 1), Cast::i32('€1.234', $format));
        // One declared alone falls back to its number counterpart for the other.
        $halfDeclared = new NumFormat(',', "\u{A0}", NumFormat::ALL, '€', currencyGroupSep: '.');
        $this->assertEquals(new Success(1234.5), Cast::f64('1.234,50 €', $halfDeclared));
        $this->assertEquals(new Success(1234.5), Cast::f64("1\u{A0}234,50", $halfDeclared));
        // Under detection the structure governs the whole token and the pair is ignored.
        $detect = new NumFormat(',', '.', NumFormat::ALL | NumFormat::SEPARATOR_DETECT, '€', '.', ',');
        $this->assertEquals(new Success(1234.5), Cast::f64('€1.234,50', $detect));
    }

    public function testDeclaredPercentAndNativeDigits(): void
    {
        $arabic = new NumFormat("\u{66B}", "\u{66C}", NumFormat::ALL, digitZero: "\u{660}", percent: "\u{66A}");
        $this->assertSame("\u{660}", $arabic->digitZero);
        // The ten code points from the zero, beside ASCII, in the integer part, the fraction
        // and the exponent; the two scripts may even mix.
        $this->assertEquals(
            new Success(1234.5),
            Cast::f64("\u{661}\u{66C}\u{662}\u{663}\u{664}\u{66B}\u{665}", $arabic)
        );
        $this->assertEquals(new Success(125.0), Cast::f64("\u{661}\u{662}\u{66B}\u{665}e\u{661}", $arabic));
        $this->assertEquals(new Success(123), Cast::i32("12\u{663}", $arabic));
        $this->assertEquals(new Success(-123), Cast::i32("-\u{661}\u{662}\u{663}", $arabic));
        // The declared percent beside '%', on the real and decimal doors only.
        $this->assertEquals(new Success(0.5), Cast::f64("\u{665}\u{660}\u{66A}", $arabic));
        $this->assertEquals(new Success(0.5), Cast::f64('50%', $arabic));
        $this->assertEquals(new Success(new Decimal('5', 1, false)), Cast::decimal("50\u{66A}", $arabic));
        $this->assertInstanceOf(Fault::class, Cast::i32("50\u{66A}", $arabic));
        // Undeclared, a native digit is Malformed at its first byte.
        $this->assertEquals(
            new Fault(CastFailure::Malformed, 0, 2),
            Cast::i32("\u{661}\u{662}", NumFormat::invariant())
        );
        // Radix-prefixed digits stay ASCII.
        $this->assertInstanceOf(Fault::class, Cast::i32("0x\u{661}", $arabic));
        $fullwidth = new NumFormat('.', ',', NumFormat::ALL, digitZero: "\u{FF10}", percent: "\u{FF05}");
        $this->assertEquals(new Success(0.5), Cast::f64("\u{FF15}\u{FF10}\u{FF05}", $fullwidth));
    }

    public function testSeparatorsMayBeTwoCharacters(): void
    {
        $spaced = new NumFormat('.', ', ', NumFormat::ALL);
        $this->assertEquals(new Success(1234), Cast::i32('1, 234', $spaced));
        $this->assertEquals(new Fault(CastFailure::Malformed, 1, 1), Cast::i32('1,234', $spaced));
        $swiss = new NumFormat('.', "' ", NumFormat::ALL);
        $this->assertEquals(new Success(1234.5), Cast::f64("1' 234.5", $swiss));
        // Eight bytes is the ceiling: two four-byte characters fit, a ninth byte does not.
        $this->assertSame("\u{1F600}\u{1F600}", (new NumFormat("\u{1F600}\u{1F600}", ',', 0))->decimalSep);
        $this->expectException(\InvalidArgumentException::class);
        new NumFormat("\u{1F600}\u{1F600}.", ',', 0);
    }

    public function testDayPeriodsAreReadBesideAmPm(): void
    {
        $german = new DayPeriods('vorm.', 'nachm.');
        $this->assertSame(['vorm.', 'nachm.'], [$german->am, $german->pm]);
        $afternoon = Cast::datetime('1/7/2026 3:04 nachm.', DateOrder::Dmy, $german);
        $this->assertInstanceOf(Success::class, $afternoon);
        $this->assertSame('2026-07-01 15:04:00', $afternoon->value->format('Y-m-d H:i:s'));
        $clock = static fn (Success|Fault $verdict): string => $verdict->value->format('H:i');
        $this->assertSame('03:04', $clock(Cast::datetime('1/7/2026 3:04 vorm.', DateOrder::Dmy, $german)));
        // English stays read beside the declared pair, and ahead of the clock too.
        $this->assertSame('15:04', $clock(Cast::datetime('1/7/2026 3:04 PM', DateOrder::Dmy, $german)));
        $this->assertSame('15:04', $clock(Cast::datetime('1/7/2026 pm 3:04', DateOrder::Dmy)));
        // Korean writes the period ahead of the clock, with or without a space.
        $korean = new DayPeriods("\u{C624}\u{C804}", "\u{C624}\u{D6C4}");
        $this->assertSame('15:04', $clock(Cast::datetime("2026-01-07 \u{C624}\u{D6C4} 3:04", DateOrder::Ymd, $korean)));
        $this->assertSame('15:04', $clock(Cast::datetime("2026-01-07 \u{C624}\u{D6C4}3:04", DateOrder::Ymd, $korean)));
        $midnight = Cast::datetime("2026-01-07 \u{C624}\u{C804} 12:00", DateOrder::Ymd, $korean);
        $this->assertSame('00:00', $clock($midnight));
        // An internal space is a designator's own business (Spanish).
        $spanish = new DayPeriods('a. m.', 'p. m.');
        $this->assertSame('15:04', $clock(Cast::datetime('1/7/2026 3:04 p. m.', DateOrder::Mdy, $spanish)));
        // Undeclared, a culture's designator is Malformed where it starts — the existing
        // signature, exactly as before.
        $this->assertEquals(
            new Fault(CastFailure::Malformed, 13, 1),
            Cast::datetime('1/7/2026 3:04 nachm.', DateOrder::Dmy)
        );
        // Two markers is Malformed at the second.
        $this->assertEquals(
            new Fault(CastFailure::Malformed, 16, 1),
            Cast::datetime('1/7/2026 PM 3:04 PM', DateOrder::Mdy)
        );
    }

    public function testDayPeriodsScratchFollowsTheDeclaredPair(): void
    {
        // The identity memo rewrites the native struct per DayPeriods instance; a longer
        // pair after a shorter one, then none at all, each read as declared.
        $german = new DayPeriods('vorm.', 'nachm.');
        $spanish = new DayPeriods('a. m.', 'p. m.');
        $this->assertInstanceOf(Success::class, Cast::datetime('1/7/2026 3:04 p. m.', DateOrder::Mdy, $spanish));
        $this->assertInstanceOf(Success::class, Cast::datetime('1/7/2026 3:04 nachm.', DateOrder::Mdy, $german));
        $this->assertInstanceOf(Fault::class, Cast::datetime('1/7/2026 3:04 p. m.', DateOrder::Mdy, $german));
        $this->assertInstanceOf(Fault::class, Cast::datetime('1/7/2026 3:04 nachm.', DateOrder::Mdy));
        $this->assertInstanceOf(Success::class, Cast::datetime('1/7/2026 3:04 p. m.', DateOrder::Mdy, $spanish));
    }

    public function testDayPeriodValidationIsACallerBug(): void
    {
        foreach (['', ' am', 'am ', "pm\t", 'a1', "\xFF", str_repeat('오', 6)] as $bad) {
            try {
                new DayPeriods($bad, 'pm');
                $this->fail("designator '{$bad}' should have been rejected");
            } catch (\InvalidArgumentException) {
                $this->addToAssertionCount(1);
            }
        }
        // Sixteen bytes exactly is the ceiling, not over it; an internal space is fine.
        $sixteen = str_repeat('오', 5) . 'x';
        $this->assertSame($sixteen, (new DayPeriods($sixteen, 'p. m.'))->am);
    }

    public function testSymbolValidationIsACallerBug(): void
    {
        $cases = [
            'a minus sign with whitespace' => ['minusSign' => ' -'],
            'a plus sign with a digit' => ['plusSign' => '+1'],
            'a percent symbol with whitespace' => ['percent' => "%\t"],
            'an over-long minus sign' => ['minusSign' => str_repeat("\u{2212}", 3)],
            'ill-formed UTF-8 in a sign' => ['minusSign' => "\xC3"],
            'a currency decimal separator with a digit' => ['currencyDecimalSep' => '0'],
            'an over-long currency group separator' => ['currencyGroupSep' => str_repeat(' ', 9)],
            'a currency pair that collides' => ['currencyDecimalSep' => ','],
            'a currency pair that collides the other way' => ['currencyGroupSep' => '.'],
            'an ASCII digit zero' => ['digitZero' => '0'],
            'a digit zero of two characters' => ['digitZero' => "\u{660}\u{661}"],
            'a digit zero that starts no block' => ['digitZero' => "\u{FF3F}"],
        ];
        foreach ($cases as $name => $arguments) {
            try {
                new NumFormat('.', ',', NumFormat::ALL, ...$arguments);
                $this->fail("{$name} should have been rejected");
            } catch (\InvalidArgumentException) {
                $this->addToAssertionCount(1);
            }
        }
        // A currency pair declared in full may differ from the number pair in any way.
        $swapped = new NumFormat('.', ',', NumFormat::ALL, '€', ',', '.');
        $this->assertSame([',', '.'], [$swapped->currencyDecimalSep, $swapped->currencyGroupSep]);
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

    public function testFromLocaleconvReadsTheRestOfTheNotation(): void
    {
        // An et_EE-shaped localeconv(): U+2212 for the minus sign, a monetary group
        // separator unlike the number one, and the sign after the amount (n_sign_posn 2).
        $estonian = NumFormat::fromLocaleconv([
            'decimal_point' => ',',
            'thousands_sep' => "\u{A0}",
            'currency_symbol' => '€',
            'mon_decimal_point' => ',',
            'mon_thousands_sep' => '.',
            'negative_sign' => "\u{2212}",
            'positive_sign' => '+',
            'n_sign_posn' => 2,
        ]);
        $this->assertSame(NumFormat::ALL | NumFormat::TRAILING_SIGN, $estonian->flags);
        $this->assertSame("\u{2212}", $estonian->minusSign);
        // The monetary decimal point equals the number one, so it declares nothing; the
        // ASCII plus sign likewise.
        $this->assertSame(
            ['', '.', ''],
            [$estonian->currencyDecimalSep, $estonian->currencyGroupSep, $estonian->plusSign]
        );
        $this->assertEquals(new Success(-1234.5), Cast::f64("\u{2212}1\u{A0}234,5", $estonian));
        $this->assertEquals(new Success(-1234.5), Cast::f64("1.234,50 €\u{2212}", $estonian));
        $this->assertEquals(new Success(-1234.5), Cast::f64("-1\u{A0}234,5", $estonian));
        // n_sign_posn 4 is the other trailing placement; 1 and 3 lead; CHAR_MAX is unset.
        $trailing = static fn (int $posn): int => NumFormat::fromLocaleconv(['n_sign_posn' => $posn])->flags
            & NumFormat::TRAILING_SIGN;
        $this->assertSame(NumFormat::TRAILING_SIGN, $trailing(4));
        $this->assertSame(0, $trailing(1));
        $this->assertSame(0, $trailing(127));
        // The C locale's empty everything, and the ASCII signs, declare nothing further.
        $c = NumFormat::fromLocaleconv(['negative_sign' => '-', 'positive_sign' => '', 'mon_decimal_point' => '']);
        $this->assertSame(
            ['', '', '', ''],
            [$c->minusSign, $c->plusSign, $c->currencyDecimalSep, $c->currencyGroupSep]
        );
        // A monetary decimal point that is the number group separator, with no monetary
        // group separator reported: the empty one takes the other of the pair, the number
        // pair's own rule, rather than colliding.
        $crossed = NumFormat::fromLocaleconv([
            'decimal_point' => ',',
            'thousands_sep' => '.',
            'mon_decimal_point' => '.',
        ]);
        $this->assertSame(['.', ','], [$crossed->currencyDecimalSep, $crossed->currencyGroupSep]);
    }

    public function testFromLocaleconvNeverInventsACollidingSeparator(): void
    {
        // A comma-decimal locale that reports no thousands separator: the default ',' would
        // collide with the declared decimal, so grouping takes the other of the pair.
        $comma = NumFormat::fromLocaleconv(['decimal_point' => ',', 'thousands_sep' => '']);
        $this->assertSame([',', '.'], [$comma->decimalSep, $comma->groupSep]);
        $this->assertEquals(new Success(1234.5), Cast::f64('1.234,5', $comma));
        // The mirror image, for a hand-built array: no decimal point beside a '.' group.
        $point = NumFormat::fromLocaleconv(['decimal_point' => '', 'thousands_sep' => '.']);
        $this->assertSame([',', '.'], [$point->decimalSep, $point->groupSep]);
        // A declared pair is never rewritten, and a declared collision is still a caller bug.
        $swiss = NumFormat::fromLocaleconv(['decimal_point' => '.', 'thousands_sep' => "'"]);
        $this->assertSame(['.', "'"], [$swiss->decimalSep, $swiss->groupSep]);
        $this->expectException(\InvalidArgumentException::class);
        NumFormat::fromLocaleconv(['decimal_point' => ',', 'thousands_sep' => ',']);
    }

    public function testSeparatorsCrossAsTheirBytesAtEveryUtf8Width(): void
    {
        // One, two, three and four UTF-8 bytes, each in its field with its length after it.
        foreach (['.', '·', "\u{202F}", "\u{1F600}", "\u{10FFFF}"] as $char) {
            $packed = (new NumFormat($char, "\u{D7FF}", 0))->packed();
            $this->assertSame(str_pad($char, 8, "\0") . \chr(\strlen($char)), substr($packed, 12, 9), bin2hex($char));
            $this->assertSame("\u{D7FF}\0\0\0\0\0\x03", substr($packed, 21, 9));
        }
        // A narrow no-break space is what fr_FR really groups with; it reaches the core intact.
        $french = new NumFormat(',', "\u{202F}", NumFormat::ALL);
        $this->assertEquals(new Success(1234.5), Cast::f64("1\u{202F}234,5", $french));
    }

    public function testSeparatorsMustBeWellFormedUtf8OfAtMostEightBytes(): void
    {
        foreach (['', "\xFF", "\xC3", "\xE2\x82", "\xC0\xAF", "\xED\xA0\x80", '1', str_repeat('.', 9)] as $bad) {
            try {
                new NumFormat($bad, ',', NumFormat::ALL);
                $this->fail('separator ' . bin2hex($bad) . ' should have been rejected');
            } catch (\InvalidArgumentException) {
                $this->addToAssertionCount(1);
            }
        }
    }

    public function testIsAvailableAnswersFalseWhenFfiIsDisabled(): void
    {
        // ffi.enable=0 refuses the FFI API even on the CLI, which is exactly what a
        // restricted web SAPI looks like from inside the binding.
        $this->assertSame('unavailable ffi', self::probe(\dirname(__DIR__) . '/src', '-d', 'ffi.enable=0'));
    }

    public function testIsAvailableAnswersFalseWhenTheExtensionIsMissing(): void
    {
        // -n drops every ini file, and with them a shared ext-ffi. A PHP with ext-ffi
        // compiled in statically has nothing to drop, so there is nothing to prove there.
        $answer = self::probe(\dirname(__DIR__) . '/src', '-n');
        if (str_ends_with($answer, ' ffi')) {
            $this->markTestSkipped('ext-ffi is compiled into this PHP and cannot be unloaded');
        }
        $this->assertSame('unavailable no-ffi', $answer);
    }

    public function testIsAvailableAnswersFalseWhenTheLibraryIsMissing(): void
    {
        // A copy of the binding with no native/ directory beside it, far from any cargo
        // build the development fallback could find.
        $root = sys_get_temp_dir() . '/hypercast-probe-' . bin2hex(random_bytes(6));
        $src = $root . '/php/src';
        mkdir($src, 0o777, true);
        try {
            foreach (glob(\dirname(__DIR__) . '/src/*.php') as $file) {
                copy($file, $src . '/' . basename($file));
            }
            $this->assertSame('unavailable ffi', self::probe($src));
        } finally {
            array_map('unlink', glob($src . '/*.php'));
            rmdir($src);
            rmdir($root . '/php');
            rmdir($root);
        }
    }

    public function testDeclaredShapesAreTheSizesTheCorePins(): void
    {
        // The cdef restates rust/src/verdict.rs's and ffi.rs's #[repr(C)] shapes by hand;
        // rust/src/abi.rs pins their sizes, and the declarations have to land on the same.
        Cast::nativeVersion();
        $ffi = (new \ReflectionProperty(Cast::class, 'ffi'))->getValue();
        $pinned = [
            'hc_fault' => 8,
            'hc_pair' => 16,
            'hc_date' => 4,
            'hc_civil' => 16,
            'hc_decimal' => 16,
            'hc_format' => 92,
            'hc_periods' => 40,
        ];
        foreach ($pinned as $type => $size) {
            self::assertSame($size, \FFI::sizeof($ffi->type($type)), $type);
        }
    }

    public function testDateOrderDisambiguatesLikeTheCulturesDo(): void
    {
        // The canonical ambiguity: 1/7/2026 is January 7th under en-US's month-first short
        // dates and July 1st under en-GB's day-first ones — resolved only by declaration.
        $enUs = Cast::date('1/7/2026', DateOrder::Mdy);
        $enGb = Cast::date('1/7/2026', DateOrder::Dmy);
        $this->assertInstanceOf(Success::class, $enUs);
        $this->assertInstanceOf(Success::class, $enGb);
        $this->assertSame('2026-01-07', $enUs->value->format('Y-m-d'));
        $this->assertSame('2026-07-01', $enGb->value->format('Y-m-d'));
        // Undeclared, the door stays strict ISO — the ambiguity is never guessed at.
        $undeclared = Cast::date('1/7/2026');
        $this->assertInstanceOf(Fault::class, $undeclared);
        $this->assertSame(CastFailure::Malformed, $undeclared->reason);
    }

    public function testDateTimeReadsTheMessyCivilShapes(): void
    {
        // The AM/PM world, zone-less: the UTC label on the carrier is an artifact, not
        // data — no zone was read and none was applied.
        $enUs = Cast::datetime('1/7/2026 3:04 PM', DateOrder::Mdy);
        $enGb = Cast::datetime('1/7/2026 3:04 PM', DateOrder::Dmy);
        $this->assertInstanceOf(Success::class, $enUs);
        $this->assertInstanceOf(Success::class, $enGb);
        $this->assertSame('2026-01-07 15:04:00', $enUs->value->format('Y-m-d H:i:s'));
        $this->assertSame('2026-07-01 15:04:00', $enGb->value->format('Y-m-d H:i:s'));
        // A zone suffix is not this door's business — timestamp() is the instant door.
        $this->assertInstanceOf(Fault::class, Cast::datetime('1/7/2026 15:04:05Z', DateOrder::Mdy));
    }

    /**
     * Runs tests/fixtures/probe.php in a child PHP — the only way to observe a process in
     * which the binding cannot load — and returns what it printed.
     */
    private static function probe(string $src, string ...$phpFlags): string
    {
        // Without the dev-loop override, or the probe would load the library it names.
        $env = array_diff_key(getenv(), ['HYPERCAST_NATIVE_LIBRARY' => true]);
        $process = proc_open(
            [PHP_BINARY, ...$phpFlags, __DIR__ . '/fixtures/probe.php', $src],
            [1 => ['pipe', 'w'], 2 => ['pipe', 'w']],
            $pipes,
            null,
            $env
        );
        self::assertIsResource($process);
        $stdout = stream_get_contents($pipes[1]);
        $stderr = stream_get_contents($pipes[2]);
        self::assertSame(0, proc_close($process), "probe failed: {$stdout}{$stderr}");
        return $stdout;
    }

    /** The crate's own manifest version — walked up from here the way CorpusTest finds corpus/. */
    private static function crateVersion(): string
    {
        $dir = __DIR__;
        // Stop when dirname() stops moving, not at '/': a Windows root is 'C:\\', never '/'.
        for ($parent = \dirname($dir); $parent !== $dir; $dir = $parent, $parent = \dirname($dir)) {
            $candidate = $dir . '/rust/Cargo.toml';
            if (is_file($candidate)) {
                self::assertSame(
                    1,
                    preg_match('/^version\s*=\s*"([^"]+)"/m', file_get_contents($candidate), $match),
                    'rust/Cargo.toml carries no package version'
                );
                return $match[1];
            }
        }
        self::fail('rust/Cargo.toml not found');
    }
}
