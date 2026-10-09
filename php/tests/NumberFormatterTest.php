<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use HyperCast\Cast;
use HyperCast\Decimal;
use HyperCast\Fault;
use HyperCast\NumFormat;
use HyperCast\Success;
use PHPUnit\Framework\Attributes\RequiresPhpExtension;
use PHPUnit\Framework\TestCase;

/**
 * intl's NumberFormatter as the numeric doors' format: PHP's per-object locale formatting,
 * where NumFormat::fromLocaleconv() reads process state. ext-intl is optional for the
 * package, so every test that builds a formatter requires it, and the last test proves the
 * doors need nothing from it — it runs where intl is absent (CI's Alpine images, which add
 * ext-ffi and nothing else) and is skipped where it is loaded.
 */
final class NumberFormatterTest extends TestCase
{
    /**
     * @return array<string, array{string, string, string, string, float}>
     */
    public static function locales(): array
    {
        return [
            // locale => decimal separator, group separator, currency symbol, text, value
            'en_US' => ['en_US', '.', ',', '$', '1,234.5'],
            'de_DE' => ['de_DE', ',', '.', '€', '1.234,5'],
            // ICU groups French with U+202F NARROW NO-BREAK SPACE: three bytes, one character.
            'fr_FR' => ['fr_FR', ',', "\u{202F}", '€', "1\u{202F}234,5"],
        ];
    }

    #[RequiresPhpExtension('intl')]
    #[\PHPUnit\Framework\Attributes\DataProvider('locales')]
    public function testFromNumberFormatterReadsTheFormattersSymbols(
        string $locale,
        string $decimalSep,
        string $groupSep,
        string $currency,
        string $text,
    ): void {
        $formatter = new \NumberFormatter($locale, \NumberFormatter::DECIMAL);
        $format = NumFormat::fromNumberFormatter($formatter);
        $this->assertSame([$decimalSep, $groupSep, NumFormat::ALL, $currency], [
            $format->decimalSep, $format->groupSep, $format->flags, $format->currency,
        ]);
        $this->assertEquals(new Success(1234.5), Cast::f64($text, $format));
    }

    #[RequiresPhpExtension('intl')]
    #[\PHPUnit\Framework\Attributes\DataProvider('locales')]
    public function testEveryNumericDoorTakesAFormatterAsItsFormat(
        string $locale,
        string $decimalSep,
        string $groupSep,
        string $currency,
        string $text,
    ): void {
        $formatter = new \NumberFormatter($locale, \NumberFormatter::DECIMAL);
        $format = NumFormat::fromNumberFormatter($formatter);
        $grouped = "1{$groupSep}234";
        // The formatter and the format it declares give the same verdict at every door.
        foreach (['i16', 'i32', 'i64', 'u16', 'u32', 'u64', 'f32', 'f64', 'decimal'] as $door) {
            $this->assertEquals(Cast::$door($grouped, $format), Cast::$door($grouped, $formatter), "$locale $door");
        }
        $this->assertEquals(new Success(1234), Cast::i32($grouped, $formatter));
        $this->assertEquals(new Success(1234.5), Cast::f64($text, $formatter));
        $this->assertEquals(Cast::decimal($text, $format), Cast::decimal($text, $formatter));
        $this->assertInstanceOf(Decimal::class, Cast::decimal($text, $formatter)->value);
        // The narrow widths, where 1,234 is out of range: still the format's verdict.
        foreach (['i8', 'u8'] as $door) {
            $this->assertEquals(Cast::$door($grouped, $format), Cast::$door($grouped, $formatter), "$locale $door");
            $this->assertInstanceOf(Fault::class, Cast::$door($grouped, $formatter));
        }
        // And the currency symbol it carries.
        $this->assertEquals(new Success(-5.0), Cast::f64("-{$currency}5", $formatter));
    }

    #[RequiresPhpExtension('intl')]
    public function testAFormattersChangedSymbolsAreReadAgain(): void
    {
        // A formatter is mutable, and the doors remember the format each one declared: the
        // remembered format must not outlive the symbols it was made from.
        $formatter = new \NumberFormatter('en_US', \NumberFormatter::DECIMAL);
        $this->assertEquals(new Success(1234.5), Cast::f64('1,234.5', $formatter));
        $formatter->setSymbol(\NumberFormatter::DECIMAL_SEPARATOR_SYMBOL, ',');
        $formatter->setSymbol(\NumberFormatter::GROUPING_SEPARATOR_SYMBOL, '.');
        $this->assertEquals(new Success(1234.5), Cast::f64('1.234,5', $formatter));
        $this->assertInstanceOf(Fault::class, Cast::f64('1,234.5', $formatter));
    }

    #[RequiresPhpExtension('intl')]
    public function testASymbolTheCoreCannotCarryIsACallerBug(): void
    {
        // Through a formatter exactly as through the constructor: never a verdict.
        $formatter = new \NumberFormatter('en_US', \NumberFormatter::DECIMAL);
        $formatter->setSymbol(\NumberFormatter::GROUPING_SEPARATOR_SYMBOL, '.');
        $this->expectException(\InvalidArgumentException::class);
        Cast::i32('1.234', $formatter);
    }

    public function testTheDoorsNeedNothingFromIntl(): void
    {
        if (\extension_loaded('intl')) {
            $this->markTestSkipped('ext-intl is loaded here; this runs where it is absent (CI\'s Alpine images)');
        }
        // NumberFormatter appears in the doors' signatures, and an unloaded class in a union
        // type costs nothing until something is passed as one.
        $this->assertFalse(class_exists(\NumberFormatter::class, false));
        $german = new NumFormat(',', '.', NumFormat::ALL, '€');
        $this->assertEquals(new Success(1234), Cast::i32('1.234', $german));
        $this->assertEquals(new Success(1234.5), Cast::f64('1.234,5 €', $german));
        $this->assertEquals(new Success(1234.5), Cast::f64('1,234.5', NumFormat::invariant()));
    }
}
