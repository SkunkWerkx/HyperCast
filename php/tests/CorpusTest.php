<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use DateTimeImmutable;
use HyperCast\Cast;
use HyperCast\Checked;
use HyperCast\Constraints;
use HyperCast\DateOrder;
use HyperCast\CastFailure;
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
            return NumFormat::invariant();
        }
        $format = $vector['format'];
        return new NumFormat(
            $format['decimal_sep'],
            $format['group_sep'],
            $format['flags'],
            $format['currency'] ?? '',
            $format['currency_decimal_sep'] ?? '',
            $format['currency_group_sep'] ?? '',
            $format['minus_sign'] ?? '',
            $format['plus_sign'] ?? '',
            $format['percent'] ?? '',
            $format['digit_zero'] ?? ''
        );
    }

    /** The day periods a datetime vector declares ("am" and "pm", together or not at all). */
    private static function periodsOf(array $vector): ?DayPeriods
    {
        return isset($vector['am']) ? new DayPeriods($vector['am'], $vector['pm']) : null;
    }

    private function assertVerdict(string $domain, array $vector, Success|Fault $verdict, mixed $expected): void
    {
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
            $this->assertVerdict('boolean', $vector, Cast::bool($vector['input']), $vector['value'] ?? null);
        }
    }

    public function testCharCorpus(): void
    {
        foreach (self::corpus('char.json') as $vector) {
            $verdict = Cast::char($vector['input']);
            if ($verdict instanceof Success) {
                // The corpus pins the code point; decode the door's UTF-8 back to compare.
                $verdict = new Success(self::codePoint($verdict->value));
            }
            $this->assertVerdict('char', $vector, $verdict, $vector['value'] ?? null);
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
            $verdict = Cast::$door($vector['input'], self::formatOf($vector));
            $expected = $vector['value'] ?? null;
            if ($door === 'u64' && \is_string($expected)) {
                // Beyond PHP_INT_MAX the carrier is the two's-complement bit pattern —
                // compare through the unsigned renderer, the documented consumption path.
                if ($verdict instanceof Success) {
                    $this->assertSame(
                        $expected,
                        sprintf('%u', $verdict->value),
                        "integer: '{$vector['input']}'"
                    );
                    continue;
                }
            }
            $this->assertVerdict('integer', $vector, $verdict, $expected);
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
            $verdict = Cast::$door($vector['input'], self::formatOf($vector));
            if ($verdict instanceof Success && $expected !== null) {
                self::assertSame(
                    bin2hex(pack('e', $expected)),
                    bin2hex(pack('e', $verdict->value)),
                    "real: '{$vector['input']}' bits"
                );
            }
            $this->assertVerdict('real', $vector, $verdict, $expected);
        }
    }

    public function testDecimalCorpus(): void
    {
        foreach (self::corpus('decimal.json') as $vector) {
            $input = $vector['input'];
            $verdict = Cast::decimal($input, self::formatOf($vector));
            if (!$verdict instanceof Success) {
                $this->assertVerdict('decimal', $vector, $verdict, null);
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
            $this->assertVerdict(
                'timestamp',
                $vector,
                Cast::timestamp($vector['input']),
                self::expectedInstant($vector)
            );
        }
    }

    public function testUnixCorpus(): void
    {
        foreach (self::corpus('unix.json') as $vector) {
            $verdict = Cast::unix($vector['input'], UnixPrecision::from($vector['precision']));
            $this->assertVerdict('unix', $vector, $verdict, self::expectedInstant($vector));
        }
    }

    public function testExcelSerialCorpus(): void
    {
        foreach (self::corpus('excel_serial.json') as $vector) {
            $verdict = Cast::excelSerial($vector['input'], ExcelEpoch::from($vector['epoch']));
            $this->assertVerdict('excel_serial', $vector, $verdict, self::expectedInstant($vector));
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
            $verdict = Cast::datetime($vector['input'], $order, self::periodsOf($vector));
            $this->assertVerdict('datetime', $vector, $verdict, $expected);
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
                    $expected = $vector['nanos'] ?? null;
                    break;
                case 'excel_duration':
                    $verdict = Cast::excelDuration($value);
                    $expected = isset($vector['seconds']) ? new Duration($vector['seconds'], $vector['nanos']) : null;
                    break;
                default:
                    self::fail("typed: unknown door {$door}");
            }
            $this->assertVerdict("typed {$door}", $vector, $verdict, $expected);
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
        2 => 'minimum together with exclusiveMinimum',
        3 => 'maximum together with exclusiveMaximum',
        4 => 'the lower bound is not below the upper bound',
        5 => 'multipleOf is not positive',
        6 => 'a rounding without multipleOf',
        7 => 'minLength is above maxLength',
        8 => 'a bound, step or enum entry the door cannot hold',
        9 => 'an enum entry that is not UTF-8',
        10 => 'a duplicate enum entry',
        11 => 'a malformed constraints layout',
        12 => 'an enum with no entries',
        13 => 'a text enum entry no input can match',
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
                $verdict = Cast::checkedFromFloat(self::spelledFloat($vector['from_f64']), $constraints);
            } elseif (isset($vector['input_bytes'])) {
                $seen['input_bytes'] = true;
                $verdict = Cast::checkedText(pack('C*', ...$vector['input_bytes']), $constraints);
            } elseif ($door === Door::Text) {
                $verdict = Cast::checkedText($vector['input'], $constraints);
            } else {
                $verdict = Cast::checked($vector['input'], self::formatOf($vector), $constraints);
            }
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
            $want = $vector['value'];
            $value = $verdict->value;
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
                case 'text':
                    self::assertSame($want, $value, $label);
                    if ($verdict->enumIndex !== null) {
                        // The caller's own declared entry, by index.
                        self::assertSame($constraints->enum[$verdict->enumIndex], $value, "{$label}: declared entry");
                    }
                    break;
                case 'u64':
                    self::assertSame((string) $want, sprintf('%u', $value), $label);
                    break;
                default:
                    self::assertSame($want, $value, $label);
            }
            return;
        }
        if ($expect === 'violation') {
            self::assertInstanceOf(Violation::class, $verdict, $label);
            self::assertSame($vector['keyword'], $verdict->keywordName(), $label);
            self::assertInstanceOf(Keyword::class, $verdict->keyword, $label);
            self::assertSame($vector['keyword'], lcfirst($verdict->keyword->name), $label);
            self::assertSame($vector['fault'], [$verdict->offset, $verdict->length], "{$label}: span");
            return;
        }
        self::assertInstanceOf(Fault::class, $verdict, $label);
        self::assertSame(self::EXPECTED_REASON[$expect], $verdict->reason, $label);
        self::assertSame($vector['fault'], [$verdict->offset, $verdict->length], "{$label}: span");
    }

    /**
     * Replays corpus/declarations.json through the conversion every bound, step and integer
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
}
