<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use HyperCast\Cast;
use HyperCast\CastFailure;
use HyperCast\DateOptions;
use HyperCast\DateOrder;
use HyperCast\DetailedCast;
use HyperCast\DetailedFault;
use HyperCast\Fault;
use HyperCast\FaultDetail;
use HyperCast\MonthNames;
use HyperCast\Success;
use PHPUnit\Framework\TestCase;

/**
 * Item 5, date shapes: every option is opt-in, the profile never supplies a pivot or a
 * vocabulary, and a vocabulary is caller-built.
 */
final class DateOptionsTest extends TestCase
{
    private static function french(): MonthNames
    {
        return new MonthNames([
            1 => 'janv.', 2 => 'févr.', 3 => 'mars', 4 => 'avr.', 5 => 'mai', 6 => 'juin', 7 => 'juil.',
            8 => 'août', 9 => ['sept.', 'sep.'], 10 => 'oct.', 11 => 'nov.', 12 => 'déc.',
        ]);
    }

    private static function english(): MonthNames
    {
        return new MonthNames([
            1 => 'Jan', 2 => 'Feb', 3 => 'Mar', 4 => 'Apr', 5 => 'May', 6 => 'Jun', 7 => 'Jul', 8 => 'Aug',
            9 => ['Sep', 'Sept'], 10 => 'Oct', 11 => 'Nov', 12 => 'Dec',
        ]);
    }

    private static function day(Success|Fault $verdict): string
    {
        self::assertInstanceOf(Success::class, $verdict);
        return $verdict->value->format('Y-m-d');
    }

    public function testTheConstants(): void
    {
        self::assertSame(
            [1, 2, 4],
            [DateOptions::COMPACT_DATE, DateOptions::SPACE_SEPARATOR, DateOptions::HOUR_OFFSET]
        );
        self::assertSame(
            DateOptions::COMPACT_DATE | DateOptions::SPACE_SEPARATOR | DateOptions::HOUR_OFFSET,
            DateOptions::COMMON_FORMS
        );
        $none = new DateOptions();
        self::assertSame([0, 0], [$none->flags, $none->pivot]);
        $profile = new DateOptions(DateOptions::COMMON_FORMS);
        self::assertSame([7, 0], [$profile->flags, $profile->pivot], 'the profile supplies no pivot');
    }

    public function testEveryOptionIsOptIn(): void
    {
        // Without options each door keeps today's shape rules.
        self::assertInstanceOf(Fault::class, Cast::date('20260107'));
        self::assertInstanceOf(Fault::class, Cast::timestamp('2026-01-07 15:04:05Z'));
        self::assertInstanceOf(Fault::class, Cast::timestamp('2026-01-07T15:04:05+00'));
        self::assertInstanceOf(Fault::class, Cast::date('07-Jan-2026', DateOrder::Dmy));
        self::assertInstanceOf(Fault::class, Cast::date('26/01/07', DateOrder::Ymd));
        // Each option opens its own shape.
        $compact = new DateOptions(DateOptions::COMPACT_DATE);
        self::assertSame('2026-01-07', self::day(Cast::date('20260107', null, $compact)));
        self::assertInstanceOf(
            Success::class,
            Cast::timestamp('2026-01-07 15:04:05Z', new DateOptions(DateOptions::SPACE_SEPARATOR))
        );
        self::assertInstanceOf(
            Success::class,
            Cast::timestamp('2026-01-07T15:04:05-05', new DateOptions(DateOptions::HOUR_OFFSET))
        );
        self::assertInstanceOf(
            Fault::class,
            Cast::timestamp('2026-01-07 15:04:05+00', new DateOptions(DateOptions::SPACE_SEPARATOR))
        );
        // The profile opens all three, and works unchanged at every date door: a known option a
        // door has no use for is ignored.
        $profile = new DateOptions(DateOptions::COMMON_FORMS);
        self::assertInstanceOf(Success::class, Cast::timestamp('2026-01-07 15:04:05+00', $profile));
        self::assertSame('2026-01-07', self::day(Cast::date('20260107', null, $profile)));
        self::assertInstanceOf(Fault::class, Cast::date('20260107', DateOrder::Ymd, $profile));
        $detailed = DetailedCast::datetime('20260107', DateOrder::Ymd, null, $profile);
        self::assertInstanceOf(DetailedFault::class, $detailed);
    }

    public function testAnUndefinedFlagBitIsRefused(): void
    {
        foreach ([8, 16, 1 << 31, -1] as $bits) {
            try {
                new DateOptions($bits);
                self::fail("{$bits} accepted");
            } catch (\InvalidArgumentException $e) {
                self::assertSame(
                    "Date option flags must name only COMPACT_DATE, SPACE_SEPARATOR and HOUR_OFFSET; got {$bits}",
                    $e->getMessage()
                );
            }
        }
    }

    public function testThePivotIsZeroOrOneHundredToNinetyNineNinetyNine(): void
    {
        foreach ([0, 100, 2029, 9999] as $pivot) {
            self::assertSame($pivot, (new DateOptions(0, $pivot))->pivot);
        }
        foreach ([1, 99, 10000, 65535, 65536, -1, PHP_INT_MAX] as $pivot) {
            try {
                new DateOptions(0, $pivot);
                self::fail("{$pivot} accepted");
            } catch (\InvalidArgumentException $e) {
                self::assertSame("Date pivot must be 0 (none) or 100 through 9999; got {$pivot}", $e->getMessage());
            }
        }
    }

    public function testThePivotReadsTwoDigitYears(): void
    {
        $options = new DateOptions(0, 2029);
        self::assertSame('2026-01-07', self::day(Cast::date('07/01/26', DateOrder::Dmy, $options)));
        self::assertSame('2007-01-26', self::day(Cast::date('07/01/26', DateOrder::Ymd, $options)));
        self::assertSame('1930-01-07', self::day(Cast::date('07/01/30', DateOrder::Dmy, $options)));
        // No pivot, no two-digit years; four-digit years never move.
        $without = Cast::date('07/01/26', DateOrder::Dmy);
        self::assertInstanceOf(Fault::class, $without);
        self::assertSame('2026-01-07', self::day(Cast::date('07/01/2026', DateOrder::Dmy, $options)));
    }

    public function testMonthNamesReadInTheFieldTheOrderSelects(): void
    {
        $options = new DateOptions(0, 2029);
        self::assertSame('2026-01-07', self::day(Cast::date('07-Jan-26', DateOrder::Dmy, $options, self::english())));
        self::assertSame('2026-01-07', self::day(Cast::date('7-jan-2026', DateOrder::Dmy, null, self::english())));
        self::assertSame('2026-09-07', self::day(Cast::date('7-Sept-2026', DateOrder::Dmy, null, self::english())));
        self::assertSame('2026-09-07', self::day(Cast::date('Sep-7-2026', DateOrder::Mdy, null, self::english())));
        // Without the pivot, a two-digit year is still refused, naming the field.
        $short = DetailedCast::date('07-Jan-26', DateOrder::Dmy, null, self::english());
        self::assertInstanceOf(DetailedFault::class, $short);
        self::assertSame(FaultDetail::FieldWidth, $short->detail);
        // A vocabulary never overrides the declared order.
        self::assertInstanceOf(Fault::class, Cast::date('Jan-07-2026', DateOrder::Dmy, null, self::english()));
        // An unknown name is not recognised, spanning the word.
        $unknown = DetailedCast::date('7-Xyz-2026', DateOrder::Dmy, null, self::english());
        self::assertInstanceOf(DetailedFault::class, $unknown);
        self::assertSame([CastFailure::Malformed, 2, 3, FaultDetail::NotRecognized], [
            $unknown->reason, $unknown->offset, $unknown->length, $unknown->detail,
        ]);
        // The datetime door reads names and the pivot too.
        $civil = Cast::datetime('07-Jan-26 3:04 PM', DateOrder::Dmy, null, $options, self::english());
        self::assertInstanceOf(Success::class, $civil);
        self::assertSame('2026-01-07 15:04:00', $civil->value->format('Y-m-d H:i:s'));
    }

    public function testADottedNameWithTheDotSeparatorAndAPivot(): void
    {
        $options = new DateOptions(0, 2029);
        $french = self::french();
        self::assertSame('2026-01-07', self::day(Cast::date('7.janv..26', DateOrder::Dmy, $options, $french)));
        self::assertSame('2026-01-07', self::day(Cast::date('7.JANV..2026', DateOrder::Dmy, $options, $french)));
        $malformed = DetailedCast::date('7.janv.26', DateOrder::Dmy, $options, $french);
        self::assertInstanceOf(DetailedFault::class, $malformed);
        self::assertSame(CastFailure::Malformed, $malformed->reason);
        self::assertSame(FaultDetail::NotRecognized, $malformed->detail);
        // Declared without the dot, `janv` reads the other spelling.
        $both = new MonthNames([
            1 => ['janv', 'janv.'], 2 => 'févr.', 3 => 'mars', 4 => 'avr.', 5 => 'mai', 6 => 'juin', 7 => 'juil.',
            8 => 'août', 9 => 'sept.', 10 => 'oct.', 11 => 'nov.', 12 => 'déc.',
        ]);
        self::assertSame('2026-01-07', self::day(Cast::date('7.janv.2026', DateOrder::Dmy, null, $both)));
    }

    public function testAVocabularyReadsWithoutOptions(): void
    {
        // Independent of the options: names alone, no pivot.
        self::assertSame('2026-01-07', self::day(Cast::date('7-Jan-2026', DateOrder::Dmy, null, self::english())));
        self::assertEquals(
            Cast::date('7-Jan-2026', DateOrder::Dmy, new DateOptions(), self::english()),
            Cast::date('7-Jan-2026', DateOrder::Dmy, null, self::english())
        );
        // And a door that ignores it never looks at it.
        self::assertSame('2026-01-07', self::day(Cast::date('2026-01-07', null, null, self::english())));
    }

    public function testTheVocabularyRulesAndTheirMessages(): void
    {
        $full = [
            1 => 'Jan', 2 => 'Feb', 3 => 'Mar', 4 => 'Apr', 5 => 'May', 6 => 'Jun', 7 => 'Jul', 8 => 'Aug',
            9 => 'Sep', 10 => 'Oct', 11 => 'Nov', 12 => 'Dec',
        ];
        $refused = static function (array $months, string $message) use ($full): void {
            try {
                new MonthNames($months + $full);
                self::fail("accepted: {$message}");
            } catch (\InvalidArgumentException $e) {
                self::assertSame($message, $e->getMessage());
            }
        };
        $refused([13 => 'Extra'], 'A month name must belong to a month from 1 to 12');
        $refused([0 => 'Zero'], 'A month name must belong to a month from 1 to 12');
        $refused([1 => ['Jan', 'JAN']], 'Two month names fold to the same spelling');
        $refused([1 => 'Ja1'], "A month name must not contain an ASCII digit, '/', '-' or white space");
        $refused([1 => 'Ja n'], "A month name must not contain an ASCII digit, '/', '-' or white space");
        $refused([1 => 'Ja/n'], "A month name must not contain an ASCII digit, '/', '-' or white space");
        $refused([1 => ''], 'A month name must be one to 64 bytes');
        $refused([1 => str_repeat('a', 65)], 'A month name must be one to 64 bytes');
        $refused([1 => "\xff"], 'A month name must be valid UTF-8');
        try {
            new MonthNames([1 => 'Jan']);
            self::fail('a missing month was accepted');
        } catch (\InvalidArgumentException $e) {
            self::assertSame('Every month from 1 to 12 needs at least one name', $e->getMessage());
        }
        // `.` is allowed anywhere, and a 64-byte name stands.
        self::assertInstanceOf(MonthNames::class, new MonthNames([1 => ['.Jan.', str_repeat('a', 64)]] + $full));
        // What a name or list must be.
        $this->expectException(\TypeError::class);
        new MonthNames([1 => 5] + $full);
    }

    public function testThePackedLayout(): void
    {
        $names = self::english();
        $packed = $names->packed();
        // A month byte, a little-endian u32 length, then the bytes: Jan first.
        self::assertSame("\x01\x03\x00\x00\x00Jan", substr($packed, 0, 8));
        self::assertEquals($names, MonthNames::fromPacked($packed));
        $this->expectException(\InvalidArgumentException::class);
        MonthNames::fromPacked($packed . "\x01");
    }

    public function testAVocabularyIsReadonly(): void
    {
        $names = self::english();
        self::assertSame(['Sep', 'Sept'], $names->months[9]);
        $this->expectException(\Error::class);
        // @phpstan-ignore-next-line
        $names->months = [];
    }
}
