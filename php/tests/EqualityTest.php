<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use HyperCast\DateOptions;
use HyperCast\MonthNames;
use HyperCast\NumFormat;
use PHPUnit\Framework\TestCase;

/**
 * A declaration equals (`==`) one that declares the same thing, however it was built, as the
 * Rust core's `==` holds.
 */
final class EqualityTest extends TestCase
{
    public function testAFormatEqualsOneDeclaringTheSameThing(): void
    {
        $invariant = NumFormat::invariant();
        $built = [
            new NumFormat('.', ',', NumFormat::ALL),
            new NumFormat('.', ',', NumFormat::ALL, grouping: NumFormat::GROUPING_WESTERN),
            $invariant->withGrouping(NumFormat::GROUPING_INDIAN)->withGrouping(NumFormat::GROUPING_WESTERN),
        ];
        foreach ($built as $format) {
            self::assertTrue($format == $invariant);
        }
        self::assertFalse($invariant == $invariant->withGrouping(NumFormat::GROUPING_INDIAN));
        self::assertFalse($invariant == new NumFormat('.', ',', NumFormat::ALL, '$'));
        self::assertFalse($invariant == NumFormat::detect());
    }

    public function testDateOptionsAndAVocabularyEqualOnesDeclaringTheSameThing(): void
    {
        self::assertTrue(new DateOptions(0, 2050) == new DateOptions(pivot: 2050));
        self::assertFalse(new DateOptions(0, 2050) == new DateOptions(0, 2049));
        $months = self::months('janvier');
        self::assertTrue(MonthNames::fromPacked($months->packed()) == $months);
        self::assertTrue(new MonthNames([1 => ['janvier']] + self::rest()) == $months);
        self::assertFalse(self::months('january') == $months);
    }

    private static function months(string $first): MonthNames
    {
        return new MonthNames([1 => $first] + self::rest());
    }

    /** @return array<int, string> */
    private static function rest(): array
    {
        $rest = [];
        for ($month = 2; $month <= 12; $month++) {
            $rest[$month] = 'm' . \chr(96 + $month);
        }
        return $rest;
    }
}
