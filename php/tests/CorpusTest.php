<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use DateTimeImmutable;
use HyperCast\Cast;
use HyperCast\DateOrder;
use HyperCast\CastFailure;
use HyperCast\Duration;
use HyperCast\Fault;
use HyperCast\NumFormat;
use HyperCast\Success;
use HyperCast\UnixPrecision;
use PHPUnit\Framework\TestCase;

/**
 * Replays the shared conformance corpus (corpus/*.json at the repository root) — the same
 * files every other binding's suite replays. Fault spans are byte offsets into the UTF-8
 * input, identical to PHP string byte offsets. Decoded with JSON_BIGINT_AS_STRING so
 * u64-scale expected values survive PHP's signed-64 int.
 */
final class CorpusTest extends TestCase
{
    private const EXPECTED_REASON = [
        'empty' => CastFailure::Empty,
        'malformed' => CastFailure::Malformed,
        'out_of_range' => CastFailure::OutOfRange,
    ];

    private static function corpus(string $name): array
    {
        $dir = __DIR__;
        while ($dir !== '/') {
            $candidate = $dir . '/corpus/' . $name;
            if (is_file($candidate)) {
                return json_decode(
                    file_get_contents($candidate),
                    true,
                    512,
                    JSON_THROW_ON_ERROR | JSON_BIGINT_AS_STRING
                );
            }
            $dir = \dirname($dir);
        }
        self::fail('corpus directory not found');
    }

    private static function formatOf(array $vector): NumFormat
    {
        if (!isset($vector['format'])) {
            return NumFormat::invariant();
        }
        return new NumFormat(
            $vector['format']['decimal_sep'],
            $vector['format']['group_sep'],
            $vector['format']['flags']
        );
    }

    private function assertVerdict(string $domain, array $vector, Success|Fault $verdict, mixed $expected): void
    {
        $input = $vector['input'];
        $expect = $vector['expect'];
        // The union consumption idiom in action — match over the closed pair.
        match (true) {
            $verdict instanceof Success => (function () use ($domain, $input, $expect, $verdict, $expected) {
                $this->assertSame('ok', $expect, "{$domain}: '{$input}' unexpectedly parsed");
                if ($expected instanceof DateTimeImmutable) {
                    $this->assertEquals($expected, $verdict->value, "{$domain}: '{$input}'");
                } else {
                    $this->assertEquals($expected, $verdict->value, "{$domain}: '{$input}'");
                }
            })(),
            $verdict instanceof Fault => (function () use ($domain, $input, $expect, $vector, $verdict) {
                $this->assertArrayHasKey($expect, self::EXPECTED_REASON,
                    "{$domain}: '{$input}' expected {$expect} but faulted");
                $this->assertSame(self::EXPECTED_REASON[$expect], $verdict->reason, "{$domain}: '{$input}'");
                if (isset($vector['fault'])) {
                    $this->assertSame($vector['fault'], [$verdict->offset, $verdict->length],
                        "{$domain}: '{$input}' fault span");
                }
            })(),
        };
    }

    public function testBooleanCorpus(): void
    {
        foreach (self::corpus('boolean.json') as $vector) {
            $this->assertVerdict('boolean', $vector, Cast::bool($vector['input']), $vector['value'] ?? null);
        }
    }

    public function testIntegerCorpus(): void
    {
        foreach (self::corpus('integer.json') as $vector) {
            $door = $vector['type'];
            $verdict = Cast::$door($vector['input'], self::formatOf($vector));
            $expected = $vector['value'] ?? null;
            if ($door === 'u64' && \is_string($expected)) {
                // Beyond PHP_INT_MAX the carrier is the two's-complement bit pattern —
                // compare through the unsigned renderer, the documented consumption path.
                if ($verdict instanceof Success) {
                    $this->assertSame($expected, sprintf('%u', $verdict->value),
                        "integer: '{$vector['input']}'");
                    continue;
                }
            }
            $this->assertVerdict('integer', $vector, $verdict, $expected);
        }
    }

    public function testRealCorpus(): void
    {
        foreach (self::corpus('real.json') as $vector) {
            $door = $vector['type'] === 'f32' ? 'f32' : 'f64';
            $expected = $vector['value'] ?? null;
            if ($expected !== null) {
                $expected = (float) $expected;
                if ($door === 'f32') {
                    $expected = unpack('g', pack('g', $expected))[1];
                }
            }
            $this->assertVerdict('real', $vector, Cast::$door($vector['input'], self::formatOf($vector)), $expected);
        }
    }

    public function testUuidCorpus(): void
    {
        foreach (self::corpus('uuid.json') as $vector) {
            $expected = null;
            if (isset($vector['value'])) {
                $hex = $vector['value'];
                $expected = sprintf(
                    '%s-%s-%s-%s-%s',
                    substr($hex, 0, 8),
                    substr($hex, 8, 4),
                    substr($hex, 12, 4),
                    substr($hex, 16, 4),
                    substr($hex, 20, 12)
                );
            }
            $this->assertVerdict('uuid', $vector, Cast::uuid($vector['input']), $expected);
        }
    }

    private static function expectedInstant(array $vector): ?DateTimeImmutable
    {
        if (!isset($vector['seconds'])) {
            return null;
        }
        $base = new DateTimeImmutable("@{$vector['seconds']}");
        $micros = intdiv($vector['nanos'], 1000);
        return $micros === 0 ? $base : $base->modify("+{$micros} microseconds");
    }

    public function testTimestampCorpus(): void
    {
        foreach (self::corpus('timestamp.json') as $vector) {
            $this->assertVerdict('timestamp', $vector, Cast::timestamp($vector['input']),
                self::expectedInstant($vector));
        }
    }

    public function testUnixCorpus(): void
    {
        foreach (self::corpus('unix.json') as $vector) {
            $verdict = Cast::unix($vector['input'], UnixPrecision::from($vector['precision']));
            $this->assertVerdict('unix', $vector, $verdict, self::expectedInstant($vector));
        }
    }

    public function testDateCorpus(): void
    {
        foreach (self::corpus('date.json') as $vector) {
            $expected = null;
            if (isset($vector['year'])) {
                $expected = new DateTimeImmutable(
                    sprintf('%04d-%02d-%02d 00:00:00', $vector['year'], $vector['month'], $vector['day']),
                    new \DateTimeZone('UTC')
                );
            }
            $this->assertVerdict('date', $vector, Cast::date($vector['input']), $expected);
        }
    }

    public function testDateOrderCorpus(): void
    {
        foreach (self::corpus('date_order.json') as $vector) {
            $order = DateOrder::from($vector['order']);
            $expected = null;
            if (isset($vector['year'])) {
                $expected = new DateTimeImmutable(
                    sprintf('%04d-%02d-%02d 00:00:00', $vector['year'], $vector['month'], $vector['day']),
                    new \DateTimeZone('UTC')
                );
            }
            $this->assertVerdict('date_order', $vector, Cast::date($vector['input'], $order), $expected);
        }
    }

    public function testDateTimeCorpus(): void
    {
        foreach (self::corpus('datetime.json') as $vector) {
            $order = DateOrder::from($vector['order']);
            $expected = null;
            if (isset($vector['year'])) {
                $secondOfDay = intdiv($vector['nanos_of_day'], 1_000_000_000);
                $micros = intdiv($vector['nanos_of_day'] % 1_000_000_000, 1000);
                $expected = new DateTimeImmutable(
                    sprintf(
                        '%04d-%02d-%02d %02d:%02d:%02d.%06d',
                        $vector['year'],
                        $vector['month'],
                        $vector['day'],
                        intdiv($secondOfDay, 3600),
                        intdiv($secondOfDay % 3600, 60),
                        $secondOfDay % 60,
                        $micros
                    ),
                    new \DateTimeZone('UTC')
                );
            }
            $this->assertVerdict('datetime', $vector, Cast::datetime($vector['input'], $order), $expected);
        }
    }

    public function testTimeCorpus(): void
    {
        foreach (self::corpus('time.json') as $vector) {
            $this->assertVerdict('time', $vector, Cast::time($vector['input']), $vector['nanos'] ?? null);
        }
    }

    public function testDurationCorpus(): void
    {
        foreach (self::corpus('duration.json') as $vector) {
            $expected = isset($vector['seconds'])
                ? new Duration($vector['seconds'], $vector['nanos'])
                : null;
            $this->assertVerdict('duration', $vector, Cast::duration($vector['input']), $expected);
        }
    }
}
