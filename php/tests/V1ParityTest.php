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
 *
 * Where the white-space change answers differently by design, fixtures/v1/divergences.json
 * says so: an object keyed "<fixture>:<key>" (the key is a formatter's label, or "<group>/<case>"
 * in the other fixtures), each value {"answer": the new recorded answer, "reason": why}. A
 * covered key is compared with the entry's answer instead of 1.0's, so a divergence is pinned,
 * not skipped; and testEveryDivergenceIsUsed fails on an entry no key touches.
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
        $expected = self::flatten(self::fixture($name));
        $actual = self::flatten(self::normalise(V1Probe::$name()));
        self::assertSame(array_keys($expected), array_keys($actual), $name);
        foreach ($expected as $key => $outcome) {
            self::assertSame(self::answer($name, $key, $outcome), $actual[$key], "{$name}: {$key}");
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
            self::assertSame(self::answer('formatters', (string) $label, $case), $actual, (string) $label);
        }
    }

    /**
     * Each divergence names a key a fixture holds and answers differently from 1.0, and says
     * why. (The key's own comparison is the tests above, which read the entry.)
     */
    public function testEveryDivergenceIsUsed(): void
    {
        $holds = [];
        foreach (self::divergences() as $id => $entry) {
            self::assertSame(['answer', 'reason'], array_keys($entry), $id);
            self::assertNotSame('', trim((string) $entry['reason']), $id);
            [$name, $key] = explode(':', $id, 2);
            self::assertContains($name, ['doors', 'formats', 'formatters', 'interop', 'objects'], $id);
            $holds[$name] ??= $name === 'formatters'
                ? self::fixture($name)
                : self::flatten(self::fixture($name));
            self::assertArrayHasKey($key, $holds[$name], "{$id}: no key of {$name} is this");
            self::assertNotSame($holds[$name][$key], $entry['answer'], "{$id}: the entry says what 1.0 said");
        }
    }

    /** What a key is compared with: the divergence's answer where one covers it, else 1.0's. */
    private static function answer(string $name, string $key, mixed $recorded): mixed
    {
        return self::divergences()["{$name}:{$key}"]['answer'] ?? $recorded;
    }

    /** @return array<string, array{answer: mixed, reason: string}> */
    private static function divergences(): array
    {
        static $divergences = null;
        return $divergences ??= self::fixture('divergences');
    }

    /**
     * A recording's groups of cases as one list of cases, "<group>/<case>", so a divergence
     * covers one case and not the group around it.
     *
     * @param array<string, mixed> $recording
     * @return array<string, mixed>
     */
    private static function flatten(array $recording): array
    {
        $cases = [];
        foreach ($recording as $group => $inGroup) {
            foreach ($inGroup as $case => $outcome) {
                $cases["{$group}/{$case}"] = $outcome;
            }
        }
        return $cases;
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
