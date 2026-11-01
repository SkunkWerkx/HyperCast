<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use HyperCast\Tests\Support\V1Probe;
use PHPUnit\Framework\Attributes\DataProvider;
use PHPUnit\Framework\Attributes\RequiresPhpExtension;
use PHPUnit\Framework\TestCase;

/**
 * Everything 1.0 answered, answered the same: each door over its inputs, every NumFormat and
 * DayPeriods construction, the interop helpers, and the value objects' behaviour (serialize,
 * var_export, json_encode, clone, comparison, and every readonly refusal). The fixtures are
 * 1.0's own recordings (tools/capture-v1-fixtures.php), and the same code (V1Probe) records
 * the extension here.
 */
final class V1ParityTest extends TestCase
{
    /** @return iterable<string, array{string}> */
    public static function recordings(): iterable
    {
        foreach (['doors', 'formats', 'interop', 'objects'] as $name) {
            yield $name => [$name];
        }
    }

    #[DataProvider('recordings')]
    public function testTheExtensionAnswersAs1Point0Did(string $name): void
    {
        $expected = self::fixture($name);
        $actual = self::normalise(V1Probe::$name());
        self::assertSame(array_keys($expected), array_keys($actual), $name);
        foreach ($expected as $key => $outcome) {
            self::assertSame($outcome, $actual[$key], "{$name}: {$key}");
        }
    }

    /**
     * NumberFormatter formats: each recorded formatter's answers replayed (so the comparison
     * holds whatever ICU this PHP links), through `fromNumberFormatter()` and three doors.
     */
    #[RequiresPhpExtension('intl')]
    public function testFormattersAreReadAs1Point0ReadThem(): void
    {
        $expected = self::fixture('formatters');
        self::assertNotEmpty($expected);
        foreach ($expected as $label => $case) {
            $actual = self::normalise(V1Probe::formatterCase($case['answers']));
            self::assertSame($case, $actual, (string) $label);
        }
    }

    /** @return array<string, mixed> */
    private static function fixture(string $name): array
    {
        $json = (string) file_get_contents(__DIR__ . "/fixtures/v1/{$name}.json");
        return json_decode($json, true, 512, JSON_THROW_ON_ERROR);
    }

    /** Through the capture's JSON encoding, so both sides compare as the same types. */
    private static function normalise(array $data): array
    {
        $flags = JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR | JSON_PRESERVE_ZERO_FRACTION;
        return json_decode(json_encode($data, $flags), true, 512, JSON_THROW_ON_ERROR);
    }
}
