<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use HyperCast\Cast;
use HyperCast\CastFailure;
use HyperCast\DetailedCast;
use HyperCast\DetailedFault;
use HyperCast\Fault;
use HyperCast\FaultDetail;
use HyperCast\NumFormat;
use HyperCast\Success;
use PHPUnit\Framework\Attributes\DataProvider;
use PHPUnit\Framework\Attributes\RequiresPhpExtension;
use PHPUnit\Framework\TestCase;

/**
 * Item 3, strict grouping: the 1.1 `NumFormat` declares its grouping policy beside the style
 * flags, strict Western unless the caller says otherwise, and the bridges read the platform's
 * own number pattern.
 */
final class GroupingTest extends TestCase
{
    public function testThePolicyConstantsAreTheCoresBits(): void
    {
        self::assertSame(0, NumFormat::GROUPING_LEGACY);
        self::assertSame(1 << 8, NumFormat::GROUPING_WESTERN);
        self::assertSame(2 << 8, NumFormat::GROUPING_INDIAN);
        self::assertSame(3 << 8, NumFormat::GROUPING_POLICY);
    }

    public function testTheInvariantFormatIsStrictWestern(): void
    {
        foreach ([NumFormat::invariant(), NumFormat::detect(), new NumFormat('.', ',', NumFormat::ALL)] as $format) {
            self::assertSame(NumFormat::GROUPING_WESTERN, $format->grouping);
        }
        // `12,34` was a value through the 1.0 invariant format; it is a fault now, at the group.
        $fault = Cast::i32('12,34', NumFormat::invariant());
        self::assertEquals(new Fault(CastFailure::Malformed, 2, 3), $fault);
        $detailed = DetailedCast::i32('12,34', NumFormat::invariant());
        self::assertInstanceOf(DetailedFault::class, $detailed);
        self::assertSame(FaultDetail::GroupSize, $detailed->detail);
        self::assertEquals(new Success(1234), Cast::i32('1,234', NumFormat::invariant()));
        self::assertEquals(new Success(1234567), Cast::i32('1,234,567', NumFormat::invariant()));
        self::assertEquals(new Success(1234), Cast::i32('1234', NumFormat::invariant()));
        // The same at the float and decimal doors, and in a fraction.
        self::assertEquals(new Fault(CastFailure::Malformed, 2, 3), Cast::f64('12,34', NumFormat::invariant()));
        self::assertInstanceOf(Fault::class, Cast::decimal('12,34', NumFormat::invariant()));
        self::assertInstanceOf(Fault::class, Cast::f64('1.234,5', NumFormat::invariant()));
    }

    public function testTheLegacyOptionKeepsTheOldRule(): void
    {
        $legacy = new NumFormat('.', ',', NumFormat::ALL, grouping: NumFormat::GROUPING_LEGACY);
        self::assertEquals(new Success(1234), Cast::i32('12,34', $legacy));
        self::assertEquals(new Success(1234), Cast::i32('1,2,3,4', $legacy));
        $viaWith = NumFormat::invariant()->withGrouping(NumFormat::GROUPING_LEGACY);
        self::assertEquals($legacy, $viaWith);
        self::assertEquals(new Success(1234), Cast::i32('12,34', $viaWith));
    }

    public function testTheIndianOption(): void
    {
        $indian = new NumFormat('.', ',', NumFormat::ALL, grouping: NumFormat::GROUPING_INDIAN);
        self::assertEquals(new Success(1234567), Cast::i32('12,34,567', $indian));
        self::assertEquals(new Success(1234567), Cast::i32('1234567', $indian));
        self::assertEquals(new Success(-12345.5), Cast::f64('-12,345.5', $indian));
        self::assertInstanceOf(Fault::class, Cast::i32('1,234,567', $indian));
        self::assertInstanceOf(Fault::class, Cast::i32('123,4567', $indian));
        self::assertEquals(new Success(1234567), Cast::i32('12,34,567', NumFormat::invariant()->withGrouping(512)));
    }

    public function testAStrictFormatWithGroupingOffForbidsTheSeparator(): void
    {
        $format = new NumFormat('.', ',', NumFormat::ALL & ~NumFormat::GROUPING);
        self::assertSame(NumFormat::GROUPING_WESTERN, $format->grouping);
        self::assertInstanceOf(Fault::class, Cast::i32('1,234', $format));
        self::assertEquals(new Success(1234), Cast::i32('1234', $format));
    }

    public function testWithGroupingReplacesThePolicyAndKeepsTheRest(): void
    {
        $styles = NumFormat::ALL | NumFormat::TRAILING_SIGN;
        $german = new NumFormat(',', '.', $styles, '€', '', '', "\u{2212}", '', '', '');
        $indian = $german->withGrouping(NumFormat::GROUPING_INDIAN);
        self::assertSame(NumFormat::GROUPING_INDIAN, $indian->grouping);
        self::assertSame(
            [',', '.', NumFormat::ALL | NumFormat::TRAILING_SIGN, '€', "\u{2212}"],
            [$indian->decimalSep, $indian->groupSep, $indian->flags, $indian->currency, $indian->minusSign]
        );
        // The policy is replaced, never OR-ed into the old one.
        $western = $indian->withGrouping(NumFormat::GROUPING_WESTERN);
        self::assertSame(NumFormat::GROUPING_WESTERN, $western->grouping);
        self::assertEquals($german, $western);
        self::assertSame(NumFormat::GROUPING_LEGACY, $western->withGrouping(NumFormat::GROUPING_LEGACY)->grouping);
        $this->expectException(\InvalidArgumentException::class);
        $western->withGrouping(NumFormat::GROUPING_POLICY);
    }

    /** @return iterable<string, array{int}> */
    public static function badPolicies(): iterable
    {
        yield 'policy 11' => [NumFormat::GROUPING_POLICY];
        yield 'a style bit' => [1];
        yield 'the bit above the policy' => [1 << 10];
        yield 'negative' => [-256];
        yield 'both policy bits and more' => [NumFormat::GROUPING_POLICY | 1];
    }

    #[DataProvider('badPolicies')]
    public function testAPolicyThatIsNoPolicyIsRefused(int $policy): void
    {
        try {
            new NumFormat('.', ',', NumFormat::ALL, grouping: $policy);
            self::fail('accepted');
        } catch (\InvalidArgumentException $e) {
            self::assertSame(
                'Grouping policy must be NumFormat::GROUPING_LEGACY, NumFormat::GROUPING_WESTERN or '
                    . "NumFormat::GROUPING_INDIAN; got {$policy}",
                $e->getMessage()
            );
        }
    }

    /** @return iterable<string, array{int}> */
    public static function badFlags(): iterable
    {
        yield 'a policy in the flags' => [NumFormat::ALL | NumFormat::GROUPING_WESTERN];
        yield 'bit 10' => [NumFormat::ALL | (1 << 10)];
        yield 'bit 31' => [NumFormat::ALL | (1 << 31)];
        yield 'negative' => [-1];
    }

    #[DataProvider('badFlags')]
    public function testFlagsThatCarryMoreThanStylesAreRefused(int $flags): void
    {
        // A style mask never carries a policy, and no undefined bit passes (1.0 ignored them).
        try {
            new NumFormat('.', ',', $flags);
            self::fail('accepted');
        } catch (\InvalidArgumentException $e) {
            $hint = ($flags & NumFormat::GROUPING_POLICY) !== 0
                ? '; the grouping policy is the `grouping` argument'
                : '';
            self::assertSame(
                "Flags must hold only the styles NumFormat declares, 0 through 255; got {$flags}{$hint}",
                $e->getMessage()
            );
        }
    }

    public function testThePackedLayoutCarriesThePolicyInItsFlagsWord(): void
    {
        $flagsWord = static fn (NumFormat $f): int => unpack('V', substr($f->packed(), 4, 4))[1];
        self::assertSame(NumFormat::ALL | NumFormat::GROUPING_WESTERN, $flagsWord(NumFormat::invariant()));
        self::assertSame(
            NumFormat::ALL | NumFormat::GROUPING_INDIAN,
            $flagsWord(NumFormat::invariant()->withGrouping(NumFormat::GROUPING_INDIAN))
        );
        self::assertSame(NumFormat::ALL, $flagsWord(NumFormat::invariant()->withGrouping(NumFormat::GROUPING_LEGACY)));
        // `flags` itself is what the caller gave.
        self::assertSame(NumFormat::ALL, NumFormat::invariant()->flags);
    }

    /** 1.0's own serialization of a plain format, byte for byte, with `$flags` in the flags slot. */
    private static function v1Payload(int $flags): string
    {
        return 'O:19:"HyperCast\NumFormat":10:{s:10:"decimalSep";s:1:".";s:8:"groupSep";s:1:",";s:5:"flags";i:'
            . $flags . ';s:8:"currency";s:0:"";s:18:"currencyDecimalSep";s:0:"";s:16:"currencyGroupSep";s:0:"";'
            . 's:9:"minusSign";s:0:"";s:8:"plusSign";s:0:"";s:7:"percent";s:0:"";s:9:"digitZero";s:0:"";}';
    }

    /** @return iterable<string, array{int, int}> flags in the payload, and the styles 1.0 read from them */
    public static function oneOhPayloads(): iterable
    {
        yield 'every lenience' => [95, 95];
        // 1.0 stored any integer and read its low 32 bits, ignoring all but the styles.
        yield 'minus one' => [-1, 255];
        yield 'a policy bit 1.0 ignored' => [351, 95];
    }

    #[DataProvider('oneOhPayloads')]
    public function testAFormatSerializedBy1Point0KeepsItsMeaning(int $flags, int $styles): void
    {
        // 1.0's object has ten properties and no policy: it parses as 1.0 did (legacy grouping,
        // the styles of its low byte), so a stored one never changes meaning when the extension
        // is upgraded, and its `grouping` reads as legacy rather than as an unset property.
        $format = unserialize(self::v1Payload($flags));
        self::assertInstanceOf(NumFormat::class, $format);
        self::assertSame($flags, $format->flags, 'the property is what 1.0 stored');
        self::assertSame(NumFormat::GROUPING_LEGACY, $format->grouping);
        self::assertSame($styles, unpack('V', substr($format->packed(), 4, 4))[1]);
        // Legacy grouping: a group separator anywhere between two digits.
        self::assertEquals(new Success(1234), Cast::i32('1,2,3,4', $format));
        // Serialized again it is a 1.1 object with the policy it was read with.
        $again = unserialize(serialize($format));
        self::assertSame([$flags, NumFormat::GROUPING_LEGACY], [$again->flags, $again->grouping]);
    }

    public function testAMinusOneFlagsFormatReadsEveryStyleAsOnePointZeroDid(): void
    {
        $format = unserialize(self::v1Payload(-1));
        // The low byte is every style: radix prefixes, exponents, and separator detection, which
        // makes `1,234` ambiguous as it did (DowngradeTest holds this to a real 1.0 core).
        self::assertEquals(new Success(255), Cast::i32('0xFF', $format));
        self::assertEquals(new Success(1500.0), Cast::f64('1.5e3', $format));
        self::assertEquals(new Success(1234), Cast::i32('1,2,3,4', $format));
        self::assertEquals(new Fault(CastFailure::Malformed, 1, 1), Cast::i32('1,234', $format));
        // No contract violation: the door answers, as for any format.
        self::assertInstanceOf(Fault::class, Cast::i32('x', $format));
        // A new format is never built from such flags.
        $this->expectException(\InvalidArgumentException::class);
        new NumFormat('.', ',', -1);
    }

    /** @return iterable<string, array{int}> */
    public static function craftedPolicies(): iterable
    {
        yield 'a style bit' => [7];
        yield 'policy 11' => [3 << 8];
        yield 'minus one' => [-1];
        yield 'bit 10' => [1 << 10];
    }

    #[DataProvider('craftedPolicies')]
    public function testACraftedGroupingIsRefusedAsTheConstructorRefusesIt(int $grouping): void
    {
        // A 1.1 payload carrying a `grouping` that names no policy: never ORed into the styles.
        $payload = 'O:19:"HyperCast\NumFormat":11:{s:10:"decimalSep";s:1:".";s:8:"groupSep";s:1:",";s:5:"flags";i:95;'
            . 's:8:"currency";s:0:"";s:18:"currencyDecimalSep";s:0:"";s:16:"currencyGroupSep";s:0:"";'
            . 's:9:"minusSign";s:0:"";s:8:"plusSign";s:0:"";s:7:"percent";s:0:"";s:9:"digitZero";s:0:"";'
            . 's:8:"grouping";i:' . $grouping . ';}';
        try {
            unserialize($payload);
            self::fail("grouping {$grouping} unserialized");
        } catch (\InvalidArgumentException $e) {
            self::assertSame(
                'Grouping policy must be NumFormat::GROUPING_LEGACY, NumFormat::GROUPING_WESTERN or '
                    . "NumFormat::GROUPING_INDIAN; got {$grouping}",
                $e->getMessage()
            );
        }
    }

    public function testNothingOutsideThePolicyBitsReachesTheStyles(): void
    {
        // Past `unserialize` (reflection writes the readonly property's slot), packed() still
        // refuses a policy that is none rather than ORing it into the flags.
        $format = new NumFormat('.', ',', NumFormat::ALL);
        $slot = new \ReflectionProperty(NumFormat::class, 'grouping');
        $clone = (new \ReflectionClass(NumFormat::class))->newInstanceWithoutConstructor();
        foreach (
            ['decimalSep', 'groupSep', 'flags', 'currency', 'currencyDecimalSep', 'currencyGroupSep', 'minusSign',
            'plusSign', 'percent', 'digitZero'] as $name
        ) {
            (new \ReflectionProperty(NumFormat::class, $name))->setRawValueWithoutLazyInitialization(
                $clone,
                $format->$name
            );
        }
        $slot->setRawValueWithoutLazyInitialization($clone, 7);
        $this->expectException(\RuntimeException::class);
        $clone->packed();
    }

    public function testThePolicyOfAOneOneFormatSurvivesUnserialize(): void
    {
        foreach ([NumFormat::GROUPING_LEGACY, NumFormat::GROUPING_WESTERN, NumFormat::GROUPING_INDIAN] as $policy) {
            $format = new NumFormat('.', ',', NumFormat::ALL, grouping: $policy);
            self::assertSame($policy, unserialize(serialize($format))->grouping);
        }
    }

    public function testTheOneOhRecordingOfAFormatUnserializes(): void
    {
        $path = __DIR__ . '/fixtures/v1/objects.json';
        $objects = json_decode((string) file_get_contents($path), true, flags: JSON_THROW_ON_ERROR);
        $recorded = $objects['NumFormat']['serialize'];
        $format = unserialize($recorded);
        self::assertInstanceOf(NumFormat::class, $format);
        self::assertSame([223, NumFormat::GROUPING_LEGACY], [$format->flags, $format->grouping]);
        self::assertSame('kr.', $format->currency);
    }

    /** The C `CHAR_MAX` that ends a `localeconv()` grouping: 127, or 255 where `char` is unsigned. */
    private static function charMax(): int
    {
        $unsignedChar = ['aarch64', 'arm64', 'armv7l', 'ppc64le', 's390x'];
        $unsigned = PHP_OS_FAMILY === 'Linux' && \in_array(php_uname('m'), $unsignedChar, true);
        return $unsigned ? 255 : 127;
    }

    /**
     * The policy and GROUPING flag a bridge declares are the owner's rule: every pattern that
     * groups Western is strict Western, all Indian strict Indian, any other shape or a
     * disagreement is legacy, and GROUPING is cleared only when no pattern groups.
     */
    public function testFromLocaleconvReadsBothGroupingArrays(): void
    {
        $conv = ['decimal_point' => '.', 'thousands_sep' => ','];
        $declared = static function (array $extra) use ($conv): array {
            $format = NumFormat::fromLocaleconv($conv + $extra);
            return [$format->grouping, ($format->flags & NumFormat::GROUPING) !== 0];
        };
        $western = [NumFormat::GROUPING_WESTERN, true];
        $indian = [NumFormat::GROUPING_INDIAN, true];
        $legacy = [NumFormat::GROUPING_LEGACY, true];
        // Both patterns, as the C library gives them: the last size repeats.
        self::assertSame($indian, $declared(['grouping' => [3, 2], 'mon_grouping' => [3, 2]]));
        self::assertSame($indian, $declared(['grouping' => [3, 2, 0], 'mon_grouping' => [3, 2, self::charMax()]]));
        self::assertSame($western, $declared(['grouping' => [3], 'mon_grouping' => [3]]));
        self::assertSame($western, $declared(['grouping' => [3, 3], 'mon_grouping' => [3, 0]]));
        // Number Indian, currency Western: the platform's own output could not all be read.
        self::assertSame($legacy, $declared(['grouping' => [3, 2], 'mon_grouping' => [3]]));
        self::assertSame($legacy, $declared(['grouping' => [3], 'mon_grouping' => [3, 2]]));
        // Any other shape.
        self::assertSame($legacy, $declared(['grouping' => [2], 'mon_grouping' => [2]]));
        self::assertSame($legacy, $declared(['grouping' => [3], 'mon_grouping' => [2]]));
        self::assertSame($legacy, $declared(['grouping' => [2, 3], 'mon_grouping' => [2, 3]]));
        self::assertSame($legacy, $declared(['grouping' => [3, 3, 2], 'mon_grouping' => [3, 3, 2]]));
        // One pattern that does not group leaves the other's shape, and the flag stays.
        self::assertSame($indian, $declared(['grouping' => [3, 2], 'mon_grouping' => []]));
        self::assertSame($western, $declared(['grouping' => [], 'mon_grouping' => [3]]));
        self::assertSame($legacy, $declared(['grouping' => [], 'mon_grouping' => [2]]));
        // Neither groups (the C locale): GROUPING is cleared, the policy is moot.
        self::assertSame([NumFormat::GROUPING_WESTERN, false], $declared(['grouping' => [], 'mon_grouping' => []]));
        // A key the array lacks has no vote; with no vote at all nothing is known: legacy.
        self::assertSame($indian, $declared(['grouping' => [3, 2]]));
        self::assertSame($western, $declared(['mon_grouping' => [3]]));
        self::assertSame($legacy, $declared([]));
        self::assertSame($legacy, $declared(['grouping' => 'x']));
        self::assertSame($legacy, $declared(['grouping' => ['3', 2]]));
        $indianFormat = NumFormat::fromLocaleconv($conv + ['grouping' => [3, 2], 'mon_grouping' => [3, 2]]);
        self::assertEquals(new Success(1234567), Cast::i32('12,34,567', $indianFormat));
        self::assertInstanceOf(Fault::class, Cast::i32('1,234,567', $indianFormat));
        $legacyFormat = NumFormat::fromLocaleconv($conv + ['grouping' => [3, 2], 'mon_grouping' => [3]]);
        self::assertEquals(new Success(1234567), Cast::i32('1,234,567', $legacyFormat));
        self::assertEquals(new Success(1234567), Cast::i32('12,34,567', $legacyFormat));
        $none = NumFormat::fromLocaleconv($conv + ['grouping' => [], 'mon_grouping' => []]);
        self::assertSame(0, $none->flags & NumFormat::GROUPING);
        self::assertInstanceOf(Fault::class, Cast::i32('1,234', $none));
        self::assertEquals(new Success(1234), Cast::i32('1234', $none));
    }

    public function testTheCurrentLocalesGroupingIsRead(): void
    {
        $conv = localeconv();
        $format = NumFormat::fromLocaleconv();
        $groups = static fn (mixed $sizes): array => array_values(array_filter(
            \is_array($sizes) ? $sizes : [],
            static fn (mixed $size): bool => $size !== 0 && $size !== self::charMax()
        ));
        $shape = static fn (array $sizes): string => match (true) {
            $sizes === [] => 'none',
            $sizes === array_fill(0, \count($sizes), 3) => 'western',
            $sizes[0] === 3 && \count($sizes) > 1 && array_unique(\array_slice($sizes, 1)) === [2] => 'indian',
            default => 'other',
        };
        $shapes = [$shape($groups($conv['grouping'] ?? null)), $shape($groups($conv['mon_grouping'] ?? null))];
        $expected = match (true) {
            $shapes === ['none', 'none'] => NumFormat::GROUPING_WESTERN,
            \count(array_unique(array_diff($shapes, ['none']))) === 1
                && array_values(array_diff($shapes, ['none']))[0] === 'western' => NumFormat::GROUPING_WESTERN,
            \count(array_unique(array_diff($shapes, ['none']))) === 1
                && array_values(array_diff($shapes, ['none']))[0] === 'indian' => NumFormat::GROUPING_INDIAN,
            default => NumFormat::GROUPING_LEGACY,
        };
        self::assertSame($expected, $format->grouping);
    }

    /**
     * What a formatter's bridge declares: its policy, and whether GROUPING is on.
     *
     * @return array{int, bool}
     */
    private static function declared(\NumberFormatter $formatter): array
    {
        $format = NumFormat::fromNumberFormatter($formatter);
        return [$format->grouping, ($format->flags & NumFormat::GROUPING) !== 0];
    }

    /** The three styles of a locale. */
    private static function styles(string $locale): array
    {
        return [
            'decimal' => new \NumberFormatter($locale, \NumberFormatter::DECIMAL),
            'currency' => new \NumberFormatter($locale, \NumberFormatter::CURRENCY),
            'percent' => new \NumberFormatter($locale, \NumberFormatter::PERCENT),
        ];
    }

    /**
     * The values ICU itself formats, read back through the bridge's format and through the door
     * handed the formatter: a decimal, and a currency amount (the symbol is the formatter's).
     */
    private function assertReadsItsOwnOutput(string $locale): void
    {
        ['decimal' => $decimal, 'currency' => $currency] = self::styles($locale);
        foreach ([1234567.0, -1234567.0, 12.0, 1234.0, 1234567.5, 0.5] as $value) {
            foreach ([$decimal, $currency] as $style => $formatter) {
                // A fraction only where the style has the digit for it (a currency may have none).
                if ($value !== floor($value) && $formatter->getAttribute(\NumberFormatter::MAX_FRACTION_DIGITS) < 1) {
                    continue;
                }
                $text = $formatter->format($value);
                $label = "{$locale} " . ($style === 0 ? 'decimal' : 'currency') . ' ' . json_encode($text);
                foreach ([$decimal, $currency] as $reader) {
                    $verdict = Cast::f64((string) $text, $reader);
                    self::assertInstanceOf(Success::class, $verdict, $label . ' ' . json_encode($verdict));
                    self::assertEquals($value, $verdict->value, $label);
                }
                $verdict = Cast::f64((string) $text, NumFormat::fromNumberFormatter($formatter));
                self::assertEquals(new Success($value), $verdict, $label);
            }
        }
    }

    /**
     * For every value ICU formats in a locale, the bridge's format never refuses it because of its
     * grouping: where the text with its group separators taken out reads, the grouped text reads
     * too, to the same value. (Other symbols a locale writes, a bidi mark beside a currency
     * symbol, say, are not grouping, and are the same before and after this change.)
     */
    private function assertGroupingNeverRejectsItsOutput(string $locale): void
    {
        ['decimal' => $decimal, 'currency' => $currency] = self::styles($locale);
        $symbols = [\NumberFormatter::GROUPING_SEPARATOR_SYMBOL, \NumberFormatter::MONETARY_GROUPING_SEPARATOR_SYMBOL];
        foreach ([$decimal, $currency] as $at => $formatter) {
            // The group separator the style formats with: the number's, or the money's.
            $separators = array_filter([$formatter->getSymbol($symbols[$at])]);
            foreach ([1234567.0, -1234567.0, 12345.0, 123.0] as $value) {
                $text = (string) $formatter->format($value);
                $ungrouped = str_replace($separators, '', $text);
                foreach ([$decimal, $currency] as $reader) {
                    $plain = Cast::f64($ungrouped, $reader);
                    if ($plain instanceof Success) {
                        self::assertEquals($plain, Cast::f64($text, $reader), "{$locale} " . json_encode($text));
                    }
                }
            }
        }
    }

    #[RequiresPhpExtension('intl')]
    public function testFromNumberFormatterReadsBothPatterns(): void
    {
        $western = [NumFormat::GROUPING_WESTERN, true];
        $indian = [NumFormat::GROUPING_INDIAN, true];
        $legacy = [NumFormat::GROUPING_LEGACY, true];
        foreach (['en_US', 'de_DE', 'fr_FR', 'es_ES', 'en_BG', 'ja_JP'] as $locale) {
            foreach (self::styles($locale) as $style => $formatter) {
                self::assertSame($western, self::declared($formatter), "{$locale} {$style}");
            }
        }
        // Indian in both patterns, and a percent formatter is judged by its locale's number and
        // currency patterns, not its own.
        foreach (['en_IN', 'hi_IN', 'bn_IN', 'gu_IN', 'te_IN'] as $locale) {
            foreach (self::styles($locale) as $style => $formatter) {
                self::assertSame($indian, self::declared($formatter), "{$locale} {$style}");
            }
        }
        // The disagreeing locales: number Indian, currency Western. Legacy, whichever style is asked.
        foreach (['ml', 'ml_IN', 'or', 'or_IN', 'mr', 'mr_IN', 'ta', 'ta_IN', 'ta_LK'] as $locale) {
            foreach (self::styles($locale) as $style => $formatter) {
                self::assertSame($legacy, self::declared($formatter), "{$locale} {$style}");
            }
        }
        // Another shape: Toki Pona groups by two (`#,#0.###`).
        foreach (['tok', 'tok_001'] as $locale) {
            $formatters = self::styles($locale);
            self::assertSame('#,#0.###', $formatters['decimal']->getPattern());
            foreach ($formatters as $style => $formatter) {
                self::assertSame($legacy, self::declared($formatter), "{$locale} {$style}");
            }
        }
        // Neither pattern groups (en_US_POSIX's `0.######` and `¤ 0.00`): GROUPING is cleared.
        foreach (self::styles('en_US_POSIX') as $style => $formatter) {
            self::assertSame([NumFormat::GROUPING_WESTERN, false], self::declared($formatter), "POSIX {$style}");
        }
        $posix = NumFormat::fromNumberFormatter(new \NumberFormatter('en_US_POSIX', \NumberFormatter::DECIMAL));
        self::assertInstanceOf(Fault::class, Cast::i32('1,234', $posix));
        self::assertEquals(new Success(1234), Cast::i32('1234', $posix));
    }

    #[RequiresPhpExtension('intl')]
    public function testMixedNumberAndCurrencyPatterns(): void
    {
        // One formatter's own pattern replaces its style's; the other style is its locale's.
        $with = static function (string $locale, int $style, string $pattern): array {
            $formatter = new \NumberFormatter($locale, $style);
            $formatter->setPattern($pattern);
            return self::declared($formatter);
        };
        $decimal = \NumberFormatter::DECIMAL;
        $currency = \NumberFormatter::CURRENCY;
        $western = [NumFormat::GROUPING_WESTERN, true];
        $indian = [NumFormat::GROUPING_INDIAN, true];
        $legacy = [NumFormat::GROUPING_LEGACY, true];
        // Western + Western; Western + Indian; Indian + Indian; Indian + Western.
        self::assertSame($western, $with('en_US', $decimal, '#,##0.###'));
        self::assertSame($legacy, $with('en_IN', $decimal, '#,##0.###'));
        self::assertSame($indian, $with('en_IN', $decimal, '#,##,##0.###'));
        self::assertSame($legacy, $with('en_US', $decimal, '#,##,##0.###'));
        self::assertSame($legacy, $with('en_US', $currency, '¤#,##,##0.00'));
        // Western + another shape, and another shape beside Indian.
        self::assertSame($legacy, $with('en_US', $decimal, '#,#0.###'));
        self::assertSame($legacy, $with('en_IN', $currency, '¤#,#0.00'));
        // Indian + none, Western + none: the flag stays, the pattern that groups decides.
        self::assertSame($indian, $with('en_IN', $decimal, '0.###'));
        self::assertSame($indian, $with('en_IN', $currency, '¤0.00'));
        self::assertSame($western, $with('en_US', $decimal, '0.###'));
        self::assertSame($western, $with('en_US', $currency, '¤0.00'));
        // Another shape + none is still another shape.
        self::assertSame($legacy, $with('tok', $decimal, '0.###'));
        // A secondary size of zero repeats the primary: one comma is groups of three.
        self::assertSame($western, $with('en_US', $decimal, '#,##0.###'));
        // The grouping read is the integer part's, whatever surrounds it.
        self::assertSame($indian, $with('en_IN', $decimal, "'a,b' #,##,##0.###;(#,##0)"));
        self::assertSame($indian, $with('en_IN', $currency, "\u{200F}¤ #,##,##0.00;¤-#,##,##0.00"));
        // A user's change to a formatter is read again at the door.
        $india = new \NumberFormatter('en_IN', \NumberFormatter::DECIMAL);
        self::assertEquals(new Success(1234567), Cast::i32('12,34,567', $india));
        self::assertInstanceOf(Fault::class, Cast::i32('1,234,567', $india));
        $india->setPattern('#,##0.###');
        self::assertEquals(new Success(1234567), Cast::i32('1,234,567', $india));
        self::assertEquals(new Success(1234567), Cast::i32('12,34,567', $india));
    }

    #[RequiresPhpExtension('intl')]
    public function testTheBridgeReadsThePlatformsOwnFormattedValues(): void
    {
        // Locales whose currency text starts with a bidi mark (ar_EG, fa_IR) are left to the
        // never-rejects-for-grouping check over every locale below: that mark is no grouping, and
        // 1.0 did not read it either.
        foreach (
            [
                'en_US', 'en_IN', 'hi_IN', 'de_DE', 'fr_FR', 'de_CH', 'ru_RU', 'bn_BD', 'ja_JP',
                'ml_IN', 'mr_IN', 'or_IN', 'ta_IN', 'ta_LK', 'ml', 'ta', 'tok', 'tok_001', 'en_US_POSIX', 'en_BG',
            ] as $locale
        ) {
            $this->assertReadsItsOwnOutput($locale);
        }
    }

    #[RequiresPhpExtension('intl')]
    public function testTheBridgeReadsEveryLocalesOwnOutputAndDeclaresWhatItsPatternsSay(): void
    {
        $shape = static function (\NumberFormatter $f): string {
            $primary = $f->getAttribute(\NumberFormatter::GROUPING_SIZE);
            $secondary = $f->getAttribute(\NumberFormatter::SECONDARY_GROUPING_SIZE);
            if ($f->getAttribute(\NumberFormatter::GROUPING_USED) === 0 || $primary <= 0) {
                return 'none';
            }
            $secondary = $secondary === 0 ? $primary : $secondary;
            return match ([$primary, $secondary]) {
                [3, 3] => 'western',
                [3, 2] => 'indian',
                default => 'other',
            };
        };
        $locales = 0;
        $legacy = [];
        foreach (\ResourceBundle::getLocales('') as $locale) {
            ['decimal' => $decimal, 'currency' => $currency] = self::styles($locale);
            $shapes = [$shape($decimal), $shape($currency)];
            $voting = array_values(array_unique(array_diff($shapes, ['none'])));
            $expected = match (true) {
                $voting === [] => [NumFormat::GROUPING_WESTERN, false],
                $voting === ['western'] => [NumFormat::GROUPING_WESTERN, true],
                $voting === ['indian'] => [NumFormat::GROUPING_INDIAN, true],
                default => [NumFormat::GROUPING_LEGACY, true],
            };
            foreach (self::styles($locale) as $style => $formatter) {
                self::assertSame($expected, self::declared($formatter), "{$locale} {$style}");
            }
            if ($expected[0] === NumFormat::GROUPING_LEGACY) {
                $legacy[] = $locale;
            }
            $this->assertGroupingNeverRejectsItsOutput($locale);
            $locales++;
        }
        self::assertGreaterThan(500, $locales);
        foreach (['ml_IN', 'ta_LK', 'tok', 'tok_001'] as $locale) {
            self::assertContains($locale, $legacy);
        }
        // Percent patterns are outside the rule (owner's ruling): in `bn` the percent pattern
        // `#,##0%` is Western beside an Indian number pattern, and the bridge declares Indian
        // for all three styles, reading the number and currency patterns only.
        self::assertSame(
            [NumFormat::GROUPING_INDIAN, true],
            self::declared(new \NumberFormatter('bn', \NumberFormatter::PERCENT))
        );
    }
}
