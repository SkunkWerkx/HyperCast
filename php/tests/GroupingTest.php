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
use PHPUnit\Framework\Attributes\RequiresPhp;
use PHPUnit\Framework\Attributes\RequiresPhpExtension;
use PHPUnit\Framework\TestCase;

/**
 * Item 3, grouping policies: the 1.1 `NumFormat` declares its grouping policy beside the style
 * flags, Western unless the caller says otherwise, and the bridges read the platform's
 * own number pattern.
 */
final class GroupingTest extends TestCase
{
    public function testThePolicyConstantsAreTheCoresBits(): void
    {
        self::assertSame(0, NumFormat::GROUPING_WESTERN);
        self::assertSame(2 << 8, NumFormat::GROUPING_INDIAN);
        self::assertSame(3 << 8, NumFormat::GROUPING_POLICY);
    }

    public function testTheInvariantFormatIsWestern(): void
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

    public function testAPolicyIsChosenInBothDirections(): void
    {
        $indian = NumFormat::invariant()->withGrouping(NumFormat::GROUPING_INDIAN);
        self::assertEquals(new Success(123456), Cast::i32('1,23,456', $indian));
        self::assertEquals(new Fault(CastFailure::Malformed, 1, 4), Cast::i32('1,234,567', $indian));
        $back = $indian->withGrouping(NumFormat::GROUPING_WESTERN);
        self::assertEquals(NumFormat::invariant(), $back);
        self::assertEquals(new Fault(CastFailure::Malformed, 1, 3), Cast::i32('1,23,456', $back));
        self::assertEquals(new Success(1234567), Cast::i32('1,234,567', $back));
        // What a plan's column does with a bridge whose numbers are Indian and currency Western.
        $ml = NumFormat::fromLocaleconv([
            'decimal_point' => '.', 'thousands_sep' => ',', 'currency_symbol' => '₹',
            'grouping' => [3, 2, 0], 'mon_grouping' => [3, 0],
        ]);
        self::assertSame(NumFormat::GROUPING_INDIAN, $ml->grouping);
        self::assertInstanceOf(Fault::class, Cast::f64('₹1,234,567.50', $ml));
        self::assertEquals(
            new Success(1234567.5),
            Cast::f64('₹1,234,567.50', $ml->withGrouping(NumFormat::GROUPING_WESTERN)),
        );
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

    public function testAFormatWithGroupingOffForbidsTheSeparator(): void
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
                'Grouping policy must be NumFormat::GROUPING_WESTERN or '
                    . "NumFormat::GROUPING_INDIAN; got {$policy}",
                $e->getMessage()
            );
        }
    }

    /** @return iterable<string, array{int}> */
    public static function badFlags(): iterable
    {
        yield 'a policy in the flags' => [NumFormat::ALL | NumFormat::GROUPING_INDIAN];
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
        self::assertSame(NumFormat::ALL, $flagsWord(NumFormat::invariant()));
        self::assertSame(
            NumFormat::ALL | NumFormat::GROUPING_INDIAN,
            $flagsWord(NumFormat::invariant()->withGrouping(NumFormat::GROUPING_INDIAN))
        );
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

    public function testAFormatSerializedBy1Point0IsWestern(): void
    {
        // 1.0's object has ten properties and no policy: what records no policy is Western, and
        // `grouping` reads so rather than as an unset property.
        $format = unserialize(self::v1Payload(95));
        self::assertInstanceOf(NumFormat::class, $format);
        self::assertSame(95, $format->flags);
        self::assertSame(NumFormat::GROUPING_WESTERN, $format->grouping);
        self::assertSame(95, unpack('V', substr($format->packed(), 4, 4))[1]);
        self::assertEquals(new Fault(CastFailure::Malformed, 1, 2), Cast::i32('1,2,345', $format));
        self::assertEquals(new Fault(CastFailure::Malformed, 1, 3), Cast::i32('1,23,456', $format));
        self::assertEquals(new Success(1234567), Cast::i32('1,234,567', $format));
        // Serialized again it is a 1.1 object with the policy it was read with.
        $again = unserialize(serialize($format));
        self::assertSame([95, NumFormat::GROUPING_WESTERN], [$again->flags, $again->grouping]);
    }

    /** @return iterable<string, array{int}> a `flags` holding bit 8, which is refused */
    public static function retiredFlags(): iterable
    {
        yield 'the stale 351' => [351];
        yield 'the stale 863' => [863];
        yield 'bit 8 alone' => [256];
        yield 'bits 8 and 9' => [768];
        // 1.0 stored any integer; one whose bits include 8 is refused with the rest.
        yield 'minus one' => [-1];
        yield 'a low word of ones' => [0x1FF];
    }

    /**
     * @return iterable<string, array{int, string}> a `flags` holding no bit 8 but something that
     *   isn't a style: refused at `unserialize` as Go's JSON refuses it where it is decoded
     */
    public static function otherFlags(): iterable
    {
        yield 'the Indian bit, beside the styles' => [607, 'grouping property'];
        yield 'the Indian bit alone' => [512, 'grouping property'];
        yield 'bit 10' => [1119, 'bits 0 to 7'];
        yield 'bit 40' => [95 | 1 << 40, 'bits 0 to 7'];
    }

    #[DataProvider('otherFlags')]
    public function testASerializedFlagsHoldingAnythingButStylesIsRefusedAtUnserialize(int $flags, string $why): void
    {
        try {
            unserialize(self::v1Payload($flags));
            self::fail("flags {$flags} unserialized");
        } catch (\InvalidArgumentException $e) {
            self::assertStringContainsString($why, $e->getMessage());
        }
        // Past `unserialize` (reflection writes the readonly property's slot), packed() refuses it.
        $format = new NumFormat('.', ',', NumFormat::ALL);
        $clone = (new \ReflectionClass(NumFormat::class))->newInstanceWithoutConstructor();
        foreach (
            ['decimalSep', 'groupSep', 'currency', 'currencyDecimalSep', 'currencyGroupSep', 'minusSign',
            'plusSign', 'percent', 'digitZero', 'grouping'] as $name
        ) {
            (new \ReflectionProperty(NumFormat::class, $name))->setRawValueWithoutLazyInitialization(
                $clone,
                $format->$name
            );
        }
        (new \ReflectionProperty(NumFormat::class, 'flags'))->setRawValueWithoutLazyInitialization($clone, $flags);
        $this->expectException(\RuntimeException::class);
        $clone->packed();
    }

    #[DataProvider('retiredFlags')]
    public function testASerializedFlagsHoldingBitEightIsRefusedAtUnserialize(int $flags): void
    {
        try {
            unserialize(self::v1Payload($flags));
            self::fail("flags {$flags} unserialized");
        } catch (\InvalidArgumentException $e) {
            self::assertStringContainsString('bit 8', $e->getMessage());
        }
        // Past `unserialize`, packed() refuses it as well.
        $this->expectException(\InvalidArgumentException::class);
        new NumFormat('.', ',', $flags);
    }

    /** @return iterable<string, array{int}> */
    public static function craftedPolicies(): iterable
    {
        yield 'a style bit' => [7];
        yield 'policy 11' => [3 << 8];
        yield 'the retired bit 8' => [1 << 8];
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
                'Grouping policy must be NumFormat::GROUPING_WESTERN or '
                    . "NumFormat::GROUPING_INDIAN; got {$grouping}",
                $e->getMessage()
            );
        }
    }

    // The reflection call that writes a readonly slot from outside arrived in PHP 8.4.
    #[RequiresPhp('>= 8.4.0')]
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
        foreach ([NumFormat::GROUPING_WESTERN, NumFormat::GROUPING_INDIAN] as $policy) {
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
        self::assertSame([223, NumFormat::GROUPING_WESTERN], [$format->flags, $format->grouping]);
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
     * The policy a bridge declares follows the number pattern: all Indian is Indian, anything else
     * Western. GROUPING is cleared only when no pattern groups.
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
        // Both patterns, as the C library gives them: the last size repeats.
        self::assertSame($indian, $declared(['grouping' => [3, 2], 'mon_grouping' => [3, 2]]));
        self::assertSame($indian, $declared(['grouping' => [3, 2, 0], 'mon_grouping' => [3, 2, self::charMax()]]));
        self::assertSame($western, $declared(['grouping' => [3], 'mon_grouping' => [3]]));
        self::assertSame($western, $declared(['grouping' => [3, 3], 'mon_grouping' => [3, 0]]));
        // Number Indian, currency Western, and the other way about: the number pattern decides.
        self::assertSame($indian, $declared(['grouping' => [3, 2], 'mon_grouping' => [3]]));
        self::assertSame($western, $declared(['grouping' => [3], 'mon_grouping' => [3, 2]]));
        // Any other shape is Western.
        self::assertSame($western, $declared(['grouping' => [2], 'mon_grouping' => [2]]));
        self::assertSame($western, $declared(['grouping' => [3], 'mon_grouping' => [2]]));
        self::assertSame($western, $declared(['grouping' => [2, 3], 'mon_grouping' => [2, 3]]));
        self::assertSame($western, $declared(['grouping' => [3, 3, 2], 'mon_grouping' => [3, 3, 2]]));
        // One pattern that does not group: the flag stays, and only a number pattern is Indian.
        self::assertSame($indian, $declared(['grouping' => [3, 2], 'mon_grouping' => []]));
        self::assertSame($western, $declared(['grouping' => [], 'mon_grouping' => [3]]));
        self::assertSame($western, $declared(['grouping' => [], 'mon_grouping' => [3, 2]]));
        self::assertSame($western, $declared(['grouping' => [], 'mon_grouping' => [2]]));
        // Neither groups (the C locale): GROUPING is cleared, the policy is moot.
        self::assertSame([NumFormat::GROUPING_WESTERN, false], $declared(['grouping' => [], 'mon_grouping' => []]));
        // A key the array lacks says nothing; with nothing said the format is Western.
        self::assertSame($indian, $declared(['grouping' => [3, 2]]));
        self::assertSame($western, $declared(['mon_grouping' => [3]]));
        self::assertSame($western, $declared([]));
        self::assertSame($western, $declared(['grouping' => 'x']));
        self::assertSame($western, $declared(['grouping' => ['3', 2]]));
        $indianFormat = NumFormat::fromLocaleconv($conv + ['grouping' => [3, 2], 'mon_grouping' => [3, 2]]);
        self::assertEquals(new Success(1234567), Cast::i32('12,34,567', $indianFormat));
        self::assertInstanceOf(Fault::class, Cast::i32('1,234,567', $indianFormat));
        $mixed = NumFormat::fromLocaleconv($conv + ['grouping' => [3, 2], 'mon_grouping' => [3]]);
        self::assertInstanceOf(Fault::class, Cast::i32('1,234,567', $mixed));
        self::assertEquals(new Success(1234567), Cast::i32('12,34,567', $mixed));
        // A locale whose last group isn't three digits is refused, not misread: glibc's `cmn_TW`
        // family groups in fours and its `unm_US` ends in twos.
        $fours = NumFormat::fromLocaleconv($conv + ['grouping' => [4, 0], 'mon_grouping' => [4, 0]]);
        self::assertSame(NumFormat::GROUPING_WESTERN, $fours->grouping);
        self::assertEquals(new Fault(CastFailure::Malformed, 1, 5), Cast::f64('1,2345.00', $fours));
        self::assertEquals(new Success(2345.0), Cast::f64('2345.00', $fours));
        $unm = NumFormat::fromLocaleconv($conv + ['grouping' => [2, 2, 2, 3, 0], 'mon_grouping' => [2, 2, 2, 3, 0]]);
        self::assertSame(NumFormat::GROUPING_WESTERN, $unm->grouping);
        self::assertEquals(new Fault(CastFailure::Malformed, 1, 3), Cast::f64('9,99.00', $unm));
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
        $expected = $shape($groups($conv['grouping'] ?? null)) === 'indian'
            ? NumFormat::GROUPING_INDIAN
            : NumFormat::GROUPING_WESTERN;
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

    /** What a formatter's own group sizes are: `none`, `western`, `indian` or `other`. */
    private static function shape(\NumberFormatter $formatter): string
    {
        $primary = $formatter->getAttribute(\NumberFormatter::GROUPING_SIZE);
        $secondary = $formatter->getAttribute(\NumberFormatter::SECONDARY_GROUPING_SIZE);
        if ($formatter->getAttribute(\NumberFormatter::GROUPING_USED) === 0 || $primary <= 0) {
            return 'none';
        }
        $secondary = $secondary === 0 ? $primary : $secondary;
        return match ([$primary, $secondary]) {
            [3, 3] => 'western',
            [3, 2] => 'indian',
            default => 'other',
        };
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
                // Where the writer's pattern isn't the policy (`ml`'s currency), it is refused.
                if (!\in_array(self::shape($formatter), [self::policyShape($decimal), 'none'], true)) {
                    continue;
                }
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
     * For every value ICU formats in a locale in a pattern of the policy the bridge declares, the
     * format never refuses it because of its grouping: where the text with its group separators
     * taken out reads, the grouped text reads too, to the same value. (Other symbols a locale
     * writes, a bidi mark beside a currency symbol, say, are not grouping.) Another pattern, `tok`'s
     * groups of two or `ml`'s Western currency, is refused rather than misread.
     */
    private static function policyShape(\NumberFormatter $decimal): string
    {
        return self::shape($decimal) === 'indian' ? 'indian' : 'western';
    }

    private function assertGroupingNeverRejectsItsOutput(string $locale): void
    {
        ['decimal' => $decimal, 'currency' => $currency] = self::styles($locale);
        if ($decimal->getPattern() === '#,#0.###') {
            self::assertInstanceOf(Fault::class, Cast::f64((string) $decimal->format(1234567.0), $decimal), $locale);
            return;
        }
        $symbols = [\NumberFormatter::GROUPING_SEPARATOR_SYMBOL, \NumberFormatter::MONETARY_GROUPING_SEPARATOR_SYMBOL];
        foreach ([$decimal, $currency] as $at => $formatter) {
            if (!\in_array(self::shape($formatter), [self::policyShape($decimal), 'none'], true)) {
                continue;
            }
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
        // Number Indian, currency Western: Indian, whichever style is asked. ICU releases move
        // these (ta's currency is Indian in some), so each is judged only where this ICU's patterns
        // still disagree; testMixedNumberAndCurrencyPatterns holds the rule on patterns of its own.
        foreach (['ml', 'ml_IN', 'or', 'or_IN', 'mr', 'mr_IN', 'ta', 'ta_IN', 'ta_LK'] as $locale) {
            $formatters = self::styles($locale);
            $shapes = [self::shape($formatters['decimal']), self::shape($formatters['currency'])];
            if ($shapes !== ['indian', 'western']) {
                continue;
            }
            foreach ($formatters as $style => $formatter) {
                self::assertSame($indian, self::declared($formatter), "{$locale} {$style}");
            }
        }
        // Another shape: Toki Pona groups by two (`#,#0.###`), in the ICU releases that have it.
        foreach (['tok', 'tok_001'] as $locale) {
            $formatters = self::styles($locale);
            if ($formatters['decimal']->getPattern() !== '#,#0.###') {
                continue;
            }
            foreach ($formatters as $style => $formatter) {
                self::assertSame($western, self::declared($formatter), "{$locale} {$style}");
            }
        }
        // Neither pattern groups (en_US_POSIX's `0.######` and `¤ 0.00`): GROUPING is cleared.
        self::assertSame('none', self::shape(new \NumberFormatter('en_US_POSIX', \NumberFormatter::DECIMAL)));
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
        // The number pattern decides, a formatter's own where it is a number's.
        self::assertSame($western, $with('en_US', $decimal, '#,##0.###'));
        self::assertSame($western, $with('en_IN', $decimal, '#,##0.###'));
        self::assertSame($indian, $with('en_IN', $decimal, '#,##,##0.###'));
        self::assertSame($indian, $with('en_US', $decimal, '#,##,##0.###'));
        self::assertSame($western, $with('en_US', $currency, '¤#,##,##0.00'));
        // Another shape is Western; a currency's never moves the number's.
        self::assertSame($western, $with('en_US', $decimal, '#,#0.###'));
        self::assertSame($indian, $with('en_IN', $currency, '¤#,#0.00'));
        // Pattern with no grouping beside one that groups: the flag stays.
        self::assertSame($western, $with('en_IN', $decimal, '0.###'));
        self::assertSame($indian, $with('en_IN', $currency, '¤0.00'));
        self::assertSame($western, $with('en_US', $decimal, '0.###'));
        self::assertSame($western, $with('en_US', $currency, '¤0.00'));
        self::assertSame($western, $with('tok', $decimal, '0.###'));
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
        self::assertInstanceOf(Fault::class, Cast::i32('12,34,567', $india));
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
                'ml_IN', 'mr_IN', 'or_IN', 'ta_IN', 'ta_LK', 'ml', 'ta', 'en_US_POSIX', 'en_BG',
            ] as $locale
        ) {
            $this->assertReadsItsOwnOutput($locale);
        }
    }

    #[RequiresPhpExtension('intl')]
    public function testTheBridgeReadsEveryLocalesOwnOutputAndDeclaresWhatItsPatternsSay(): void
    {
        $locales = 0;
        $mixed = [];
        foreach (\ResourceBundle::getLocales('') as $locale) {
            ['decimal' => $decimal, 'currency' => $currency] = self::styles($locale);
            $shapes = [self::shape($decimal), self::shape($currency)];
            $expected = [
                $shapes[0] === 'indian' ? NumFormat::GROUPING_INDIAN : NumFormat::GROUPING_WESTERN,
                $shapes !== ['none', 'none'],
            ];
            foreach (self::styles($locale) as $style => $formatter) {
                self::assertSame($expected, self::declared($formatter), "{$locale} {$style}");
            }
            if ($shapes === ['indian', 'western']) {
                $mixed[] = $locale;
            }
            $this->assertGroupingNeverRejectsItsOutput($locale);
            $locales++;
        }
        self::assertGreaterThan(500, $locales);
        // ICU releases move the rest (ta_LK, tok); this one disagrees in every ICU the suite runs on.
        self::assertContains('ml_IN', $mixed);
        // Percent patterns are outside the rule (owner's ruling): in `bn` the percent pattern
        // `#,##0%` is Western beside an Indian number pattern, and the bridge declares Indian
        // for all three styles, reading the number and currency patterns only.
        self::assertSame(
            [NumFormat::GROUPING_INDIAN, true],
            self::declared(new \NumberFormatter('bn', \NumberFormatter::PERCENT))
        );
    }
}
