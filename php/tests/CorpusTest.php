<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use DateTimeImmutable;
use HyperCast\Cast;
use HyperCast\Checked;
use HyperCast\Constraints;
use HyperCast\DateOptions;
use HyperCast\DateOrder;
use HyperCast\CastFailure;
use HyperCast\DetailedCast;
use HyperCast\DetailedFault;
use HyperCast\FaultDetail;
use HyperCast\MonthNames;
use HyperCast\DayPeriods;
use HyperCast\Decimal;
use HyperCast\Door;
use HyperCast\ExcelEpoch;
use HyperCast\Duration;
use HyperCast\Fault;
use HyperCast\Keyword;
use HyperCast\NumFormat;
use HyperCast\Rounding;
use HyperCast\Success;
use HyperCast\UnixPrecision;
use HyperCast\Violation;
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
        // Stop when dirname() stops moving, not at '/': a Windows root is 'C:\\', never '/'.
        for ($parent = \dirname($dir); $parent !== $dir; $dir = $parent, $parent = \dirname($dir)) {
            $candidate = $dir . '/corpus/' . $name;
            if (is_file($candidate)) {
                return json_decode(
                    file_get_contents($candidate),
                    true,
                    512,
                    JSON_THROW_ON_ERROR | JSON_BIGINT_AS_STRING
                );
            }
        }
        self::fail('corpus directory not found');
    }

    /**
     * The format a vector declares: the separators and flags, and every optional symbol
     * the "format" object may carry — absent is undeclared, which is '' here.
     */
    private static function formatOf(array $vector): NumFormat
    {
        if (!isset($vector['format'])) {
            // A row that declares nothing is INVARIANT, which is Western.
            return NumFormat::invariant();
        }
        $format = $vector['format'];
        // A row that declares no policy is Western; a refusal row
        // spells a policy inside `flags`, which the constructor takes apart: the style bits,
        // and the policy field beside them.
        $policy = match ($format['grouping'] ?? 'western') {
            'western' => NumFormat::GROUPING_WESTERN,
            'indian' => NumFormat::GROUPING_INDIAN,
        } | ($format['flags'] & NumFormat::GROUPING_POLICY);
        return new NumFormat(
            $format['decimal_sep'],
            $format['group_sep'],
            $format['flags'] & ~NumFormat::GROUPING_POLICY,
            $format['currency'] ?? '',
            $format['currency_decimal_sep'] ?? '',
            $format['currency_group_sep'] ?? '',
            $format['minus_sign'] ?? '',
            $format['plus_sign'] ?? '',
            $format['percent'] ?? '',
            $format['digit_zero'] ?? '',
            $policy,
        );
    }

    /** The published codes, by the corpus's names: the spec's table, never renumbered. */
    public const DETAIL_CODES = [
        'blank' => 1, 'unexpected_character' => 2, 'digit_expected' => 3, 'delimiter_expected' => 4,
        'trailing_text' => 5, 'no_digits' => 6, 'sign_misplaced' => 7, 'parentheses_unbalanced' => 8,
        'mark_repeated' => 9, 'group_misplaced' => 10, 'group_size' => 11, 'group_in_fraction' => 12,
        'ambiguous_separator' => 13, 'fraction_not_allowed' => 14, 'field_width' => 15, 'wrong_length' => 16,
        'component_missing' => 17, 'not_recognized' => 18, 'unknown_unit' => 19, 'invalid_utf8' => 20,
        'too_long' => 21, 'not_finite' => 22, 'not_whole' => 23, 'negative_not_allowed' => 24,
        'too_large' => 25, 'too_small' => 26, 'too_precise' => 27, 'date_field_out_of_range' => 28,
        'time_field_out_of_range' => 29, 'outside_window' => 30, 'nonexistent_date' => 31, 'not_a_scalar' => 32,
    ];

    /** A `FaultDetail` case as the corpus spells it. */
    private static function detailName(FaultDetail $detail): string
    {
        return strtolower((string) preg_replace('/(?<!^)[A-Z]/', '_$0', $detail->name));
    }

    /**
     * The detailed door's answer beside the plain door's: the same value, or the same fault with
     * the row's detail, by name and by raw code.
     */
    private function assertDetailed(
        string $domain,
        array $vector,
        Success|Fault $plain,
        Success|DetailedFault $detailed
    ): void {
        $input = $vector['input'] ?? json_encode($vector);
        if ($plain instanceof Success) {
            $this->assertEquals($plain, $detailed, "{$domain}: '{$input}' detailed value");
            return;
        }
        $this->assertInstanceOf(DetailedFault::class, $detailed, "{$domain}: '{$input}' detailed fault");
        $this->assertEquals($plain, $detailed->toFault(), "{$domain}: '{$input}' detailed fault is the plain one");
        $this->assertArrayHasKey('detail', $vector, "{$domain}: '{$input}' pins no detail");
        $this->assertArrayHasKey($vector['detail'], self::DETAIL_CODES, "{$domain}: '{$input}' detail name");
        $this->assertSame($vector['detail'], self::detailName($detailed->detail), "{$domain}: '{$input}' detail");
        $this->assertSame(self::DETAIL_CODES[$vector['detail']], $detailed->code, "{$domain}: '{$input}' code");
    }

    /** The day periods a datetime vector declares ("am" and "pm", together or not at all). */
    private static function periodsOf(array $vector): ?DayPeriods
    {
        return isset($vector['am']) ? new DayPeriods($vector['am'], $vector['pm']) : null;
    }

    private function assertVerdict(
        string $domain,
        array $vector,
        Success|Fault $verdict,
        mixed $expected,
        Success|DetailedFault $detailed
    ): void {
        $this->assertDetailed($domain, $vector, $verdict, $detailed);
        $input = $vector['input'];
        $expect = $vector['expect'];
        // The union consumption idiom in action — match over the closed pair.
        match (true) {
            $verdict instanceof Success => (function () use ($domain, $input, $expect, $verdict, $expected) {
                $this->assertSame('ok', $expect, "{$domain}: '{$input}' unexpectedly parsed");
                $this->assertEquals($expected, $verdict->value, "{$domain}: '{$input}'");
            })(),
            $verdict instanceof Fault => (function () use ($domain, $input, $expect, $vector, $verdict) {
                $this->assertArrayHasKey(
                    $expect,
                    self::EXPECTED_REASON,
                    "{$domain}: '{$input}' expected {$expect} but faulted"
                );
                $this->assertSame(self::EXPECTED_REASON[$expect], $verdict->reason, "{$domain}: '{$input}'");
                if (isset($vector['fault'])) {
                    $this->assertSame(
                        $vector['fault'],
                        [$verdict->offset, $verdict->length],
                        "{$domain}: '{$input}' fault span"
                    );
                }
            })(),
        };
    }

    public function testBooleanCorpus(): void
    {
        foreach (self::corpus('boolean.json') as $vector) {
            $this->assertVerdict(
                'boolean',
                $vector,
                Cast::bool($vector['input']),
                $vector['value'] ?? null,
                DetailedCast::bool($vector['input'])
            );
        }
    }

    public function testCharCorpus(): void
    {
        foreach (self::corpus('char.json') as $vector) {
            $verdict = Cast::char($vector['input']);
            $detailed = DetailedCast::char($vector['input']);
            if ($verdict instanceof Success) {
                // The corpus pins the code point; decode the door's UTF-8 back to compare.
                $verdict = new Success(self::codePoint($verdict->value));
                $detailed = new Success(self::codePoint($detailed->value));
            }
            $this->assertVerdict('char', $vector, $verdict, $vector['value'] ?? null, $detailed);
        }
    }

    /**
     * Decodes one UTF-8 scalar — the corpus's integer shape for a char value.
     *
     * @param string $utf8 exactly one UTF-8-encoded scalar
     * @return int its code point
     */
    private static function codePoint(string $utf8): int
    {
        $bytes = \array_values(\unpack('C*', $utf8));
        $lead = $bytes[0];
        [$scalar, $trail] = match (true) {
            $lead < 0x80 => [$lead, 0],
            $lead < 0xE0 => [$lead & 0x1F, 1],
            $lead < 0xF0 => [$lead & 0x0F, 2],
            default => [$lead & 0x07, 3],
        };
        self::assertCount($trail + 1, $bytes, 'char: one scalar');
        for ($i = 1; $i <= $trail; $i++) {
            $scalar = $scalar << 6 | $bytes[$i] & 0x3F;
        }
        return $scalar;
    }

    public function testIntegerCorpus(): void
    {
        foreach (self::corpus('integer.json') as $vector) {
            $door = $vector['type'];
            $format = self::formatOf($vector);
            $verdict = Cast::$door($vector['input'], $format);
            $detailed = DetailedCast::$door($vector['input'], $format);
            $expected = $vector['value'] ?? null;
            if ($door === 'u64' && \is_string($expected)) {
                // Beyond PHP_INT_MAX the carrier is the two's-complement bit pattern —
                // compare through the unsigned renderer, the documented consumption path.
                if ($verdict instanceof Success) {
                    $this->assertDetailed('integer', $vector, $verdict, $detailed);
                    $this->assertSame(
                        $expected,
                        sprintf('%u', $verdict->value),
                        "integer: '{$vector['input']}'"
                    );
                    continue;
                }
            }
            $this->assertVerdict('integer', $vector, $verdict, $expected, $detailed);
        }
    }

    /**
     * The real doors, each value compared by its bits: zero has one form, and the core never
     * returns a negative zero, which `==` would let through.
     */
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
            $format = self::formatOf($vector);
            $verdict = Cast::$door($vector['input'], $format);
            $detailed = DetailedCast::$door($vector['input'], $format);
            if ($verdict instanceof Success && $expected !== null) {
                self::assertSame(
                    bin2hex(pack('e', $expected)),
                    bin2hex(pack('e', $verdict->value)),
                    "real: '{$vector['input']}' bits"
                );
            }
            $this->assertVerdict('real', $vector, $verdict, $expected, $detailed);
        }
    }

    public function testDecimalCorpus(): void
    {
        foreach (self::corpus('decimal.json') as $vector) {
            $input = $vector['input'];
            $format = self::formatOf($vector);
            $verdict = Cast::decimal($input, $format);
            $detailed = DetailedCast::decimal($input, $format);
            $this->assertDetailed('decimal', $vector, $verdict, $detailed);
            if (!$verdict instanceof Success) {
                $this->assertVerdict('decimal', $vector, $verdict, null, $detailed);
                continue;
            }
            $this->assertSame('ok', $vector['expect'], "decimal: '{$input}' unexpectedly parsed");
            $decimal = $verdict->value;
            $this->assertInstanceOf(Decimal::class, $decimal);
            // The triple exactly as the core hands it over, then the canonical rendering.
            $this->assertSame(
                [(string) $vector['magnitude'], $vector['scale'], $vector['negative']],
                [$decimal->magnitude, $decimal->scale, $decimal->negative],
                "decimal: '{$input}' triple"
            );
            $this->assertSame($vector['value'], (string) $decimal, "decimal: '{$input}' canonical text");
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
            $this->assertVerdict(
                'uuid',
                $vector,
                Cast::uuid($vector['input']),
                $expected,
                DetailedCast::uuid($vector['input'])
            );
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

    /** The option names a corpus row spells, as the bits they set. */
    private const OPTION_FLAGS = ['compact_date' => 1, 'space_separator' => 2, 'hour_offset' => 4, 'common_forms' => 7];

    /**
     * A vocabulary as a row lists it, `[month, name]` pairs (aliases are separate pairs), grouped
     * by month for the constructor.
     *
     * @param list<array{int, string}> $pairs
     */
    private static function monthNamesOf(array $pairs): MonthNames
    {
        $months = [];
        foreach ($pairs as [$month, $name]) {
            $months[$month][] = $name;
        }
        return new MonthNames($months);
    }

    /**
     * The options and the vocabulary a row declares: none for a row with no `options`; an
     * options object only when a flag or a pivot is declared (a vocabulary reads without one).
     *
     * @return array{?DateOptions, ?MonthNames}
     */
    private static function optionsOf(array $vector): array
    {
        $declared = $vector['options'] ?? null;
        if ($declared === null) {
            return [null, null];
        }
        $flags = 0;
        foreach ($declared['flags'] as $name) {
            $flags |= self::OPTION_FLAGS[$name];
        }
        $pivot = $declared['pivot'] ?? 0;
        $options = $flags === 0 && $pivot === 0 ? null : new DateOptions($flags, $pivot);
        $months = isset($declared['months']) ? self::monthNamesOf($declared['months']) : null;
        return [$options, $months];
    }

    public function testTimestampCorpus(): void
    {
        foreach (self::corpus('timestamp.json') as $vector) {
            [$options] = self::optionsOf($vector);
            $this->assertVerdict(
                'timestamp',
                $vector,
                Cast::timestamp($vector['input'], $options),
                self::expectedInstant($vector),
                DetailedCast::timestamp($vector['input'], $options)
            );
        }
    }

    public function testUnixCorpus(): void
    {
        foreach (self::corpus('unix.json') as $vector) {
            $verdict = Cast::unix($vector['input'], UnixPrecision::from($vector['precision']));
            $detailed = DetailedCast::unix($vector['input'], UnixPrecision::from($vector['precision']));
            $this->assertVerdict('unix', $vector, $verdict, self::expectedInstant($vector), $detailed);
        }
    }

    public function testExcelSerialCorpus(): void
    {
        foreach (self::corpus('excel_serial.json') as $vector) {
            $verdict = Cast::excelSerial($vector['input'], ExcelEpoch::from($vector['epoch']));
            $detailed = DetailedCast::excelSerial($vector['input'], ExcelEpoch::from($vector['epoch']));
            $this->assertVerdict('excel_serial', $vector, $verdict, self::expectedInstant($vector), $detailed);
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
            [$options, $months] = self::optionsOf($vector);
            $this->assertVerdict(
                'date',
                $vector,
                Cast::date($vector['input'], null, $options, $months),
                $expected,
                DetailedCast::date($vector['input'], null, $options, $months)
            );
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
            [$options, $months] = self::optionsOf($vector);
            $this->assertVerdict(
                'date_order',
                $vector,
                Cast::date($vector['input'], $order, $options, $months),
                $expected,
                DetailedCast::date($vector['input'], $order, $options, $months)
            );
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
            [$options, $months] = self::optionsOf($vector);
            $periods = self::periodsOf($vector);
            $verdict = Cast::datetime($vector['input'], $order, $periods, $options, $months);
            $detailed = DetailedCast::datetime($vector['input'], $order, $periods, $options, $months);
            $this->assertVerdict('datetime', $vector, $verdict, $expected, $detailed);
        }
    }

    public function testTimeCorpus(): void
    {
        foreach (self::corpus('time.json') as $vector) {
            $this->assertVerdict(
                'time',
                $vector,
                Cast::time($vector['input']),
                $vector['nanos'] ?? null,
                DetailedCast::time($vector['input'])
            );
        }
    }

    public function testDurationCorpus(): void
    {
        foreach (self::corpus('duration.json') as $vector) {
            $expected = isset($vector['seconds'])
                ? new Duration($vector['seconds'], $vector['nanos'])
                : null;
            $this->assertVerdict(
                'duration',
                $vector,
                Cast::duration($vector['input']),
                $expected,
                DetailedCast::duration($vector['input'])
            );
        }
    }

    /**
     * The typed doors, by bits: unpack('E', hex2bin(...)) rebuilds each double exactly —
     * NAN and INF included, which JSON cannot spell. A typed door's Fault has no span.
     */
    public function testTypedCorpus(): void
    {
        $doors = [];
        foreach (self::corpus('typed.json') as $vector) {
            $value = unpack('E', hex2bin($vector['bits']))[1];
            $door = $vector['door'];
            $doors[$door] = true;
            $expected = null;
            switch ($door) {
                case 'decimal':
                    $verdict = Cast::decimalFromFloat($value);
                    $detailed = DetailedCast::decimalFromFloat($value);
                    if ($verdict instanceof Success) {
                        $decimal = $verdict->value;
                        $this->assertSame(
                            [(string) $vector['magnitude'], $vector['scale'], $vector['negative']],
                            [$decimal->magnitude, $decimal->scale, $decimal->negative],
                            "typed decimal: '{$vector['input']}' triple"
                        );
                        $this->assertSame($vector['value'], (string) $decimal, "typed decimal: '{$vector['input']}'");
                        $expected = $decimal;
                    }
                    break;
                case 'excel_serial':
                    $verdict = Cast::excelSerialFromFloat($value, ExcelEpoch::from($vector['epoch']));
                    $detailed = DetailedCast::excelSerialFromFloat($value, ExcelEpoch::from($vector['epoch']));
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
                    break;
                case 'excel_time':
                    $verdict = Cast::excelTime($value);
                    $detailed = DetailedCast::excelTime($value);
                    $expected = $vector['nanos'] ?? null;
                    break;
                case 'excel_duration':
                    $verdict = Cast::excelDuration($value);
                    $detailed = DetailedCast::excelDuration($value);
                    $expected = isset($vector['seconds']) ? new Duration($vector['seconds'], $vector['nanos']) : null;
                    break;
                default:
                    self::fail("typed: unknown door {$door}");
            }
            $this->assertVerdict("typed {$door}", $vector, $verdict, $expected, $detailed);
            if ($verdict instanceof Fault) {
                $this->assertSame([0, 0], [$verdict->offset, $verdict->length], "typed {$door}: span");
            }
        }
        $this->assertEqualsCanonicalizing(
            ['decimal', 'excel_serial', 'excel_time', 'excel_duration'],
            array_keys($doors)
        );
    }

    private const DOORS = [
        'i8' => Door::I8, 'i16' => Door::I16, 'i32' => Door::I32, 'i64' => Door::I64,
        'u8' => Door::U8, 'u16' => Door::U16, 'u32' => Door::U32, 'u64' => Door::U64,
        'f32' => Door::F32, 'f64' => Door::F64, 'decimal' => Door::Decimal, 'text' => Door::Text,
    ];

    private const ROUNDINGS = [
        'strict' => Rounding::Strict,
        'midpoint' => Rounding::Midpoint,
        'floor' => Rounding::Floor,
        'ceiling' => Rounding::Ceiling,
    ];

    /** The core's `Refusal` Display, by code: the words every binding's refusal carries. */
    private const REFUSALS = [
        1 => 'a constraint the door does not take',
        5 => 'multipleOf is not positive',
        6 => 'a rounding without multipleOf',
        8 => 'a bound, step or enum entry the door cannot hold',
        9 => 'an enum entry that is not UTF-8',
        11 => 'a malformed constraints layout',
    ];

    /** The message a refused declaration throws with, one per code. */
    public static function refusalMessage(int $code): string
    {
        return "Constraints refused (code {$code}): " . self::REFUSALS[$code] . '.';
    }

    /**
     * A vector's declaration through the public constructor, every keyword it names whether
     * or not its door takes it, as the corpus spells them (bounds and entries as strings).
     */
    public static function constraintsOf(Door $door, array $c): Constraints
    {
        return new Constraints(
            $door,
            minimum: $c['minimum'] ?? null,
            exclusiveMinimum: $c['exclusiveMinimum'] ?? null,
            maximum: $c['maximum'] ?? null,
            exclusiveMaximum: $c['exclusiveMaximum'] ?? null,
            multipleOf: $c['multipleOf'] ?? null,
            rounding: self::ROUNDINGS[$c['rounding'] ?? 'strict'],
            minLength: $c['minLength'] ?? null,
            maxLength: $c['maxLength'] ?? null,
            trim: $c['trim'] ?? false,
            caseSensitive: $c['caseSensitive'] ?? false,
            enum: $c['enum'] ?? null,
        );
    }

    /** A `from_f64` the corpus spells as text, because JSON cannot: NaN, the infinities, -0. */
    private static function spelledFloat(int|float|string $value): float
    {
        return match ($value) {
            'NaN' => NAN,
            'Infinity' => INF,
            '-Infinity' => -INF,
            '-0' => -0.0,
            default => (float) $value,
        };
    }

    /**
     * Replays corpus/constraints.json, every row, through the public `Constraints` constructor
     * (so bounds, steps and entries cross this binding's own conversion): a refused row throws
     * `\InvalidArgumentException` with the refusal code as its code and its own message; text
     * rows go through `checkedText()` (`input_bytes` as the raw bytes PHP strings are),
     * numeric rows through `checked()`, `from_f64` rows through `checkedFromFloat()`. Spans
     * are bytes, as every fault's.
     */
    public function testConstraintsCorpus(): void
    {
        $seen = [];
        $vectors = self::corpus('constraints.json');
        $replayed = 0;
        foreach ($vectors as $vector) {
            $door = self::DOORS[$vector['door']];
            $label = "constraints {$vector['door']} " . json_encode($vector['constraints']) . ' '
                . json_encode($vector['input'] ?? $vector['from_f64'] ?? $vector['input_bytes'] ?? null);
            if ($vector['expect'] === 'refused') {
                $seen['refused'] = true;
                try {
                    self::constraintsOf($door, $vector['constraints']);
                    self::fail("{$label}: not refused");
                } catch (\InvalidArgumentException $e) {
                    self::assertSame(\InvalidArgumentException::class, $e::class, $label);
                    self::assertSame($vector['refusal'], $e->getCode(), $label);
                    self::assertSame(self::refusalMessage($vector['refusal']), $e->getMessage(), $label);
                }
                $replayed++;
                continue;
            }
            $seen[$vector['door']] = true;
            $constraints = self::constraintsOf($door, $vector['constraints']);
            if (\array_key_exists('from_f64', $vector)) {
                $seen['from_f64'] = true;
                $value = self::spelledFloat($vector['from_f64']);
                $verdict = Cast::checkedFromFloat($value, $constraints);
                $detailed = DetailedCast::checkedFromFloat($value, $constraints);
            } elseif (isset($vector['input_bytes'])) {
                $seen['input_bytes'] = true;
                $bytes = pack('C*', ...$vector['input_bytes']);
                $verdict = Cast::checkedText($bytes, $constraints);
                $detailed = DetailedCast::checkedText($bytes, $constraints);
            } elseif ($door === Door::Text) {
                $verdict = Cast::checkedText($vector['input'], $constraints);
                $detailed = DetailedCast::checkedText($vector['input'], $constraints);
            } else {
                $format = self::formatOf($vector);
                $verdict = Cast::checked($vector['input'], $format, $constraints);
                $detailed = DetailedCast::checked($vector['input'], $format, $constraints);
            }
            $this->assertCheckedDetailed($label, $vector, $verdict, $detailed);
            $this->assertChecked($label, $vector, $constraints, $verdict);
            $replayed++;
        }
        // Every row was replayed: the count is the file's, never a literal.
        self::assertSame(\count($vectors), $replayed);
        ksort($seen);
        $expected = [...array_keys(self::DOORS), 'from_f64', 'input_bytes', 'refused'];
        sort($expected);
        self::assertSame($expected, array_keys($seen));
    }

    /**
     * The detailed checked door beside the plain one: a `Checked` or a `Violation` as it was
     * (a violation carries no detail), a failure with the row's detail.
     */
    private function assertCheckedDetailed(
        string $label,
        array $vector,
        Checked|Fault|Violation $plain,
        Checked|DetailedFault|Violation $detailed
    ): void {
        if (!$plain instanceof Fault) {
            self::assertEquals($plain, $detailed, "{$label}: detailed");
            return;
        }
        self::assertInstanceOf(DetailedFault::class, $detailed, "{$label}: detailed fault");
        self::assertEquals($plain, $detailed->toFault(), "{$label}: detailed fault is the plain one");
        self::assertSame($vector['detail'], self::detailName($detailed->detail), "{$label}: detail");
        self::assertSame(self::DETAIL_CODES[$vector['detail']], $detailed->code, "{$label}: code");
    }

    private function assertChecked(
        string $label,
        array $vector,
        Constraints $constraints,
        Checked|Fault|Violation $verdict
    ): void {
        $expect = $vector['expect'];
        if ($expect === 'ok') {
            self::assertInstanceOf(Checked::class, $verdict, $label);
            self::assertSame($vector['adjusted'], $verdict->adjusted, "{$label}: adjusted");
            self::assertSame($vector['enum_index'] ?? null, $verdict->enumIndex, "{$label}: enum index");
            self::assertValue($label, $vector, $verdict->value);
            if ($vector['door'] === 'text' && $verdict->enumIndex !== null) {
                // The caller's own declared entry, by index.
                self::assertSame($constraints->enum[$verdict->enumIndex], $verdict->value, "{$label}: declared entry");
            }
            return;
        }
        if ($expect === 'violation') {
            self::assertInstanceOf(Violation::class, $verdict, $label);
            self::assertSame($vector['keyword'], $verdict->keywordName(), $label);
            self::assertInstanceOf(Keyword::class, $verdict->keyword, $label);
            self::assertSame($vector['keyword'], lcfirst($verdict->keyword->name), $label);
            self::assertSame($vector['fault'], [$verdict->offset, $verdict->length], "{$label}: span");
            // The value as the door cast it, asserted as a success's is.
            self::assertValue("{$label}: violation", $vector, $verdict->value);
            return;
        }
        self::assertInstanceOf(Fault::class, $verdict, $label);
        self::assertSame(self::EXPECTED_REASON[$expect], $verdict->reason, $label);
        self::assertSame($vector['fault'], [$verdict->offset, $verdict->length], "{$label}: span");
    }

    /**
     * A row's `value` against the value a checked door delivered, a `Checked`'s or a
     * `Violation`'s: a float by its bits (an f32 as the f32 the row names), a `Decimal` by its
     * canonical text, a u64 by its unsigned spelling, anything else exactly.
     */
    private static function assertValue(string $label, array $vector, mixed $value): void
    {
        $want = $vector['value'];
        switch ($vector['door']) {
            case 'f32':
            case 'f64':
                $want = (float) $want;
                if ($vector['door'] === 'f32') {
                    $want = unpack('g', pack('g', $want))[1];
                }
                self::assertIsFloat($value, $label);
                self::assertSame(bin2hex(pack('e', $want)), bin2hex(pack('e', $value)), "{$label}: bits");
                break;
            case 'decimal':
                self::assertInstanceOf(Decimal::class, $value, $label);
                self::assertSame($want, (string) $value, $label);
                break;
            case 'u64':
                self::assertSame((string) $want, sprintf('%u', $value), $label);
                break;
            default:
                self::assertSame($want, $value, $label);
        }
    }

    /**
     * Replays corpus/declarations.json through the conversion every bound, step and numeric
     * `enum` entry crosses: a `Constraints` maximum at the decimal door, read back as the
     * `Decimal` the constructor keeps and from `packed()`, whose bytes 40 to 56 are the raw
     * triple the core reads (scale included). A refused row is code 8.
     */
    public function testDeclarationsCorpus(): void
    {
        $counts = ['ok' => 0, 'refused' => 0, 'bytes' => 0];
        $vectors = self::corpus('declarations.json');
        foreach ($vectors as $vector) {
            if (isset($vector['input_bytes'])) {
                $counts['bytes']++;
                $text = pack('C*', ...$vector['input_bytes']);
            } else {
                $text = $vector['input'];
            }
            if ($vector['expect'] === 'refused') {
                $counts['refused'] += isset($vector['input_bytes']) ? 0 : 1;
                self::assertSame(8, $vector['refusal']);
                try {
                    new Constraints(Door::Decimal, maximum: $text);
                    self::fail("declarations: '{$text}' not refused");
                } catch (\InvalidArgumentException $e) {
                    self::assertSame(8, $e->getCode(), $text);
                    self::assertSame(self::refusalMessage(8), $e->getMessage(), $text);
                }
                continue;
            }
            $counts['ok']++;
            $want = [(string) $vector['magnitude'], $vector['scale'], $vector['negative']];
            $constraints = new Constraints(Door::Decimal, maximum: $text);
            $kept = $constraints->maximum;
            self::assertSame($want, [$kept->magnitude, $kept->scale, $kept->negative], "declarations: '{$text}'");
            self::assertSame($vector['value'], (string) $kept, "declarations: '{$text}'");
            $raw = unpack('Qlo/Lhi/Cscale/Cnegative', substr($constraints->packed(), 40, 16));
            $limbs = Decimal::fromLimbs($raw['lo'], $raw['hi'], $raw['scale'], $raw['negative'] === 1);
            self::assertSame(
                $want,
                [$limbs->magnitude, $limbs->scale, $limbs->negative],
                "declarations: '{$text}' packed"
            );
        }
        // Every row was replayed, and every kind is present: counts from the file, never literals.
        self::assertSame(\count($vectors), array_sum($counts));
        self::assertNotContains(0, $counts);
    }

    /** The fields of a `num_format` row that can break a rule, in the order the extension checks them. */
    private const SYMBOL_FIELDS = [
        'decimal_sep' => ['Decimal separator', 8, false],
        'group_sep' => ['Group separator', 8, false],
        'currency_decimal_sep' => ['Currency decimal separator', 8, false],
        'currency_group_sep' => ['Currency group separator', 8, false],
        'minus_sign' => ['Minus sign', 8, true],
        'plus_sign' => ['Plus sign', 8, true],
        'percent' => ['Percent symbol', 8, true],
        'currency' => ['Currency symbol', 16, true],
    ];

    /** The 25 `White_Space` code points, as the inside of a PCRE class. */
    private const WHITE_SPACE_CLASS = '\x{9}-\x{D}\x{20}\x{85}\x{A0}\x{1680}\x{2000}-\x{200A}'
        . '\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}';

    /** Finds the field a row means to break. */
    private const WHITE_SPACE = '/[' . self::WHITE_SPACE_CLASS . ']/u';

    /** The same, at either end of a designator. */
    private const WHITE_SPACE_EDGE = '/^[' . self::WHITE_SPACE_CLASS . ']|[' . self::WHITE_SPACE_CLASS . ']\z/u';

    /**
     * The message the extension gives each rule, for the field a refused row breaks it in:
     * length, digit and white space name the field; the equal separators name their pair; a
     * currency symbol's white space is its own rule (at an edge only), and so is a designator's.
     * A sign, percent or currency symbol that holds a digit keeps 1.0's wording, "an ASCII digit or
     * whitespace", whichever it also holds.
     *
     * @param array<string, mixed> $format
     */
    private static function numFormatMessage(array $format, string $rule): string
    {
        $declared = static fn (string $field): string => $format[$field] ?? '';
        $find = static function (callable $breaks) use ($declared): string {
            foreach (array_keys(self::SYMBOL_FIELDS) as $field) {
                if ($breaks($declared($field), self::SYMBOL_FIELDS[$field])) {
                    return $field;
                }
            }
            self::fail('no field breaks the rule');
        };
        switch ($rule) {
            case 'length':
                $field = $find(static fn (string $v, array $f): bool => \strlen($v) > $f[1]);
                [$role, $max] = self::SYMBOL_FIELDS[$field];
                return "{$role} must be one to {$max} bytes of UTF-8";
            case 'digit':
                $field = $find(static fn (string $v): bool => preg_match('/[0-9]/', $v) === 1);
                [$role, , $flush] = self::SYMBOL_FIELDS[$field];
                $words = $flush ? 'an ASCII digit or whitespace' : 'an ASCII digit';
                return "{$role} must not contain {$words}; got '{$declared($field)}'";
            case 'equal_separators':
                if ($declared('decimal_sep') === $declared('group_sep')) {
                    return "Decimal and group separators must differ; both are '{$declared('decimal_sep')}'";
                }
                $both = $declared('currency_decimal_sep');
                return "Currency decimal and group separators must differ; both are '{$both}'";
            case 'currency_edge':
                return "Currency symbol must not start or end with white space; got '{$declared('currency')}'";
            case 'grouping_policy':
                return 'Grouping policy must be NumFormat::GROUPING_WESTERN or '
                    . 'NumFormat::GROUPING_INDIAN; got ' . ($format['flags'] & NumFormat::GROUPING_POLICY);
            case 'unknown_flag':
                return 'Flags must hold only the styles NumFormat declares, 0 through 255; got '
                    . ($format['flags'] & ~NumFormat::GROUPING_POLICY);
            case 'white_space':
                $field = $find(
                    static fn (string $v, array $f): bool => !str_contains($f[0], 'Group')
                        && preg_match(self::WHITE_SPACE, $v) === 1
                );
                return self::SYMBOL_FIELDS[$field][0] . " must not contain white space; got '{$declared($field)}'";
        }
        self::fail("unknown rule {$rule}");
    }

    /**
     * Replays corpus/formats.json's declaration rows through the public constructors: a
     * refused row throws `\InvalidArgumentException` with the winning rule's message (so the
     * ordering rows pin the order: length, digit, equal separators, white space); an ok row
     * builds. The `white_space` rows are skipped: PHP calls the core's `is_white_space`, so
     * there is no table of its own to hold to them.
     */
    public function testFormatsCorpus(): void
    {
        $replayed = ['num_format' => 0, 'day_periods' => 0, 'date_options' => 0, 'month_names' => 0];
        $rules = [];
        foreach (self::corpus('formats.json') as $vector) {
            $kind = $vector['kind'];
            if ($kind === 'white_space') {
                continue;
            }
            $label = $kind . ' ' . json_encode($vector, JSON_UNESCAPED_UNICODE);
            if ($kind === 'num_format') {
                $replayed[$kind]++;
                if ($vector['expect'] === 'ok') {
                    self::assertInstanceOf(NumFormat::class, self::formatOf($vector), $label);
                    continue;
                }
                $rules[$vector['rule']] = true;
                try {
                    self::formatOf($vector);
                    self::fail("{$label}: not refused");
                } catch (\InvalidArgumentException $e) {
                    $message = self::numFormatMessage($vector['format'], $vector['rule']);
                    self::assertSame($message, $e->getMessage(), $label);
                }
                continue;
            }
            if ($kind === 'date_options') {
                $replayed[$kind]++;
                $this->replayDateOptions($vector, $label, $rules);
                continue;
            }
            if ($kind === 'month_names') {
                $replayed[$kind]++;
                $this->replayMonthNames($vector, $label, $rules);
                continue;
            }
            self::assertSame('day_periods', $kind, $label);
            $replayed[$kind]++;
            // A row may declare one designator; PHP's constructor takes both, so the other
            // is a plain one that breaks nothing.
            $am = $vector['am'] ?? 'AM';
            $pm = $vector['pm'] ?? 'PM';
            if ($vector['expect'] === 'ok') {
                $periods = new DayPeriods($am, $pm);
                self::assertSame([$am, $pm], [$periods->am, $periods->pm], $label);
                continue;
            }
            $rules[$vector['rule']] = true;
            $role = preg_match(self::WHITE_SPACE_EDGE, $am) === 1 ? 'AM' : 'PM';
            $bad = $role === 'AM' ? $am : $pm;
            try {
                new DayPeriods($am, $pm);
                self::fail("{$label}: not refused");
            } catch (\InvalidArgumentException $e) {
                $message = "{$role} designator must not start or end with white space; got '{$bad}'";
                self::assertSame($message, $e->getMessage(), $label);
            }
        }
        self::assertNotContains(0, $replayed);
        ksort($rules);
        self::assertSame(
            [
                'character', 'currency_edge', 'designator_edge', 'digit', 'duplicate', 'equal_separators', 'flags',
                'grouping_policy', 'layout', 'length', 'month', 'month_missing', 'name_length', 'pivot',
                'unknown_flag', 'utf8', 'white_space',
            ],
            array_keys($rules)
        );
    }

    /** The message each rule of a refused `DateOptions` carries. */
    private static function dateOptionsMessage(string $rule, int $flags, int $pivot): string
    {
        return $rule === 'flags'
            ? "Date option flags must name only COMPACT_DATE, SPACE_SEPARATOR and HOUR_OFFSET; got {$flags}"
            : "Date pivot must be 0 (none) or 100 through 9999; got {$pivot}";
    }

    /** The message each rule of a refused vocabulary carries. */
    private const MONTH_NAMES_MESSAGES = [
        'layout' => 'Month names are cut short or have bytes after the last entry',
        'month' => 'A month name must belong to a month from 1 to 12',
        'utf8' => 'A month name must be valid UTF-8',
        'name_length' => 'A month name must be one to 64 bytes',
        'character' => "A month name must not contain an ASCII digit, '/', '-' or white space",
        'month_missing' => 'Every month from 1 to 12 needs at least one name',
        'duplicate' => 'Two month names fold to the same spelling',
    ];

    /** @param array<string, true> $rules */
    private function replayDateOptions(array $vector, string $label, array &$rules): void
    {
        $flags = $vector['flag_bits'] ?? 0;
        foreach ($vector['flags'] ?? [] as $name) {
            $flags |= self::OPTION_FLAGS[$name];
        }
        $pivot = $vector['pivot'] ?? 0;
        if ($vector['expect'] === 'ok') {
            $options = new DateOptions($flags, $pivot);
            self::assertSame([$flags, $pivot], [$options->flags, $options->pivot], $label);
            return;
        }
        $rules[$vector['rule']] = true;
        try {
            new DateOptions($flags, $pivot);
            self::fail("{$label}: not refused");
        } catch (\InvalidArgumentException $e) {
            self::assertSame(self::dateOptionsMessage($vector['rule'], $flags, $pivot), $e->getMessage(), $label);
        }
    }

    /**
     * A vocabulary row through both ways in: the constructor, and the packed layout (the one way
     * to spell a cut-short entry or a trailing byte). Both refuse with the rule's message, or both
     * read the same names.
     *
     * @param array<string, true> $rules
     */
    private function replayMonthNames(array $vector, string $label, array &$rules): void
    {
        $packed = '';
        foreach ($vector['months'] as [$month, $name]) {
            $packed .= pack('CV', $month, \strlen($name)) . $name;
        }
        $packed .= pack('C*', ...($vector['append_bytes'] ?? []));
        $builders = ['packed' => static fn (): MonthNames => MonthNames::fromPacked($packed)];
        if (!isset($vector['append_bytes'])) {
            $builders['constructor'] = fn (): MonthNames => self::monthNamesOf($vector['months']);
        }
        $read = [];
        foreach ($builders as $way => $build) {
            try {
                $names = $build();
            } catch (\InvalidArgumentException $e) {
                self::assertSame('refused', $vector['expect'], "{$label} ({$way}): refused");
                $rules[$vector['rule']] = true;
                self::assertSame(self::MONTH_NAMES_MESSAGES[$vector['rule']], $e->getMessage(), "{$label} ({$way})");
                continue;
            }
            self::assertSame('ok', $vector['expect'], "{$label} ({$way}): not refused");
            $read[$way] = $names->months;
            self::assertEquals(
                MonthNames::fromPacked($names->packed()),
                $names,
                "{$label} ({$way}): packed round trip"
            );
        }
        if (\count($read) === 2) {
            self::assertSame($read['packed'], $read['constructor'], "{$label}: the two ways read the same names");
        }
    }

    /**
     * The plain doors keep their verdict exactly: `Success(value)` or `Fault(reason, offset,
     * length)` over the three `CastFailure` cases, and every plain door still declares that
     * pair. The checked doors are additive.
     */
    public function testThePlainVerdictKeepsItsShape(): void
    {
        $params = static fn (string $class): array => array_map(
            static fn (\ReflectionParameter $p): string => $p->getType() . ' $' . $p->getName(),
            (new \ReflectionMethod($class, '__construct'))->getParameters(),
        );
        self::assertSame(['mixed $value'], $params(Success::class));
        self::assertSame(['HyperCast\\CastFailure $reason', 'int $offset', 'int $length'], $params(Fault::class));
        self::assertSame(
            ['Empty' => 1, 'Malformed' => 2, 'OutOfRange' => 3],
            array_column(
                array_map(static fn (CastFailure $c): array => [$c->name, $c->value], CastFailure::cases()),
                1,
                0
            )
        );
        $plain = ['bool', 'char', 'i8', 'i16', 'i32', 'i64', 'u8', 'u16', 'u32', 'u64', 'f32', 'f64', 'decimal',
            'uuid', 'uuidBytes', 'timestamp', 'unix', 'excelSerial', 'date', 'datetime', 'time', 'duration',
            'decimalFromFloat', 'excelSerialFromFloat', 'excelTime', 'excelDuration', 'text'];
        foreach ($plain as $door) {
            self::assertSame(
                'HyperCast\\Success|HyperCast\\Fault',
                (string) (new \ReflectionMethod(Cast::class, $door))->getReturnType(),
                $door
            );
        }
        foreach (['checked', 'checkedFromFloat', 'checkedText'] as $door) {
            self::assertSame(
                'HyperCast\\Checked|HyperCast\\Fault|HyperCast\\Violation',
                (string) (new \ReflectionMethod(Cast::class, $door))->getReturnType(),
                $door
            );
        }
        self::assertSame(Success::class, Cast::i32('1', NumFormat::invariant())::class);
        self::assertSame(Fault::class, Cast::i32('x', NumFormat::invariant())::class);
        self::assertSame(Success::class, Cast::text('x')::class);
        self::assertSame(Fault::class, Cast::text("\xff")::class);
    }

    public function testGroupingCorpus(): void
    {
        $holds = static function (array $want, string $text, NumFormat $format, string $label): void {
            $verdicts = [Cast::i64($text, $format), Cast::f64($text, $format), Cast::decimal($text, $format)];
            foreach ($verdicts as $verdict) {
                if (isset($want['value'])) {
                    self::assertInstanceOf(Success::class, $verdict, "{$label} {$text}");
                    $value = is_object($verdict->value) ? (string) $verdict->value : $verdict->value;
                    self::assertEquals($want['value'], $value, "{$label} {$text}");
                } else {
                    self::assertInstanceOf(Fault::class, $verdict, "{$label} {$text}");
                    self::assertSame(
                        [CastFailure::Malformed, $want['fault'][0], $want['fault'][1]],
                        [$verdict->reason, $verdict->offset, $verdict->length],
                        "{$label} {$text}"
                    );
                }
            }
            if (!isset($want['value'])) {
                $detailed = DetailedCast::i64($text, $format);
                self::assertSame(FaultDetail::GroupSize, $detailed->detail, "{$label} {$text}");
            }
        };
        $invariant = NumFormat::invariant();
        $conv = ['decimal_point' => '.', 'thousands_sep' => ','];
        $shapes = 0;
        foreach (self::corpus('grouping.json') as $row) {
            switch ($row['kind']) {
                case 'shape':
                    $shapes++;
                    foreach (
                        [
                            'invariant' => $invariant,
                            'constructor' => new NumFormat('.', ',', NumFormat::ALL),
                            'declared' => $invariant->withGrouping(NumFormat::GROUPING_WESTERN),
                            'indian to western' => $invariant->withGrouping(NumFormat::GROUPING_INDIAN)
                                ->withGrouping(NumFormat::GROUPING_WESTERN),
                            'bridge threes' => NumFormat::fromLocaleconv($conv + ['grouping' => [3, 0]]),
                            'bridge fours' => NumFormat::fromLocaleconv($conv + ['grouping' => [4, 0]]),
                            'bridge indian currency' => NumFormat::fromLocaleconv(
                                $conv + ['grouping' => [3, 0], 'mon_grouping' => [3, 2, 0]]
                            ),
                            'a 1.0 payload' => unserialize(serialize($invariant)),
                        ] as $label => $format
                    ) {
                        $holds($row['western'], $row['input'], $format, $label);
                    }
                    foreach (
                        [
                            'declared indian' => new NumFormat(
                                '.',
                                ',',
                                NumFormat::ALL,
                                grouping: NumFormat::GROUPING_INDIAN
                            ),
                            'western to indian' => $invariant->withGrouping(NumFormat::GROUPING_INDIAN),
                            'bridge indian' => NumFormat::fromLocaleconv($conv + ['grouping' => [3, 2, 0]]),
                            'bridge ml' => NumFormat::fromLocaleconv(
                                $conv + ['grouping' => [3, 2, 0], 'mon_grouping' => [3, 0]]
                            ),
                        ] as $label => $format
                    ) {
                        $holds($row['indian'], $row['input'], $format, $label);
                    }
                    break;
                case 'detect':
                    $holds($row, $row['input'], NumFormat::detect(), 'detect');
                    break;
                case 'refused_flags':
                    $flags = $row['flags'];
                    $policy = $flags & NumFormat::GROUPING_POLICY;
                    foreach (
                        [
                            static fn () => new NumFormat('.', ',', $flags),
                            static fn () => new NumFormat('.', ',', NumFormat::ALL, grouping: $policy),
                            static fn () => $invariant->withGrouping($policy),
                            static fn () => unserialize(
                                str_replace('s:5:"flags";i:95;', "s:5:\"flags\";i:{$flags};", serialize($invariant))
                            ),
                        ] as $entry => $build
                    ) {
                        try {
                            $build();
                            self::fail("flags {$flags}, entry point {$entry}");
                        } catch (\InvalidArgumentException) {
                            self::addToAssertionCount(1);
                        }
                    }
                    break;
                case 'accepted_flags':
                    $flags = $row['flags'];
                    $policy = $flags & NumFormat::GROUPING_POLICY;
                    $made = new NumFormat('.', ',', $flags & ~NumFormat::GROUPING_POLICY, grouping: $policy);
                    self::assertSame($flags, unpack('V', substr($made->packed(), 4, 4))[1]);
                    self::assertEquals($made, unserialize(serialize($made)));
                    break;
                default:
                    self::fail("unknown kind {$row['kind']}");
            }
        }
        self::assertGreaterThanOrEqual(18, $shapes);
    }
}
