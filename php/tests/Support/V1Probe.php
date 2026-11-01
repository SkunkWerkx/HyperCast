<?php

declare(strict_types=1);

namespace HyperCast\Tests\Support;

use HyperCast\Cast;
use HyperCast\DayPeriods;
use HyperCast\NumFormat;

/**
 * What 1.0's PHP package did, reduced to JSON: the recordings under tests/fixtures/v1 were
 * made with this class against 1.0's userland classes (tools/capture-v1-fixtures.php), and
 * the parity tests run the same code against the extension and compare. It uses nothing but
 * the `HyperCast\` API, so it runs against either.
 *
 * Every value is reduced by {@see reduce()} to something JSON carries exactly: a float as
 * its `var_export`, a string that is not UTF-8 as its hex, an object as its class and public
 * properties, a date as its full text and zone, an enum case as its name.
 */
final class V1Probe
{
    /** The classes of 1.0's public API, by short name under `HyperCast\`. */
    public const CLASSES = [
        'Cast', 'Success', 'Fault', 'CastFailure', 'DateOrder', 'ExcelEpoch', 'UnixPrecision',
        'NumFormat', 'DayPeriods', 'Decimal', 'Duration',
        'Interop\\NativeValues', 'Interop\\NativePlatform',
    ];

    /**
     * The public surface of each class, from reflection: kind, modifiers, parent and
     * interfaces, constants and enum cases, public methods with their parameters, and public
     * properties with their types.
     *
     * @return array<string, mixed>
     */
    public static function surface(): array
    {
        $surface = [];
        foreach (self::CLASSES as $short) {
            $rc = new \ReflectionClass('HyperCast\\' . $short);
            $entry = [
                'kind' => $rc->isEnum() ? 'enum' : ($rc->isInterface() ? 'interface' : 'class'),
                'final' => $rc->isFinal(),
                'readonly' => $rc->isReadOnly(),
                'instantiable' => $rc->isInstantiable(),
                'interfaces' => $rc->getInterfaceNames(),
                'parent' => $rc->getParentClass() ? $rc->getParentClass()->getName() : null,
                'constants' => [],
                'cases' => [],
                'methods' => [],
                'properties' => [],
            ];
            sort($entry['interfaces']);
            foreach ($rc->getReflectionConstants() as $const) {
                if ($const->isEnumCase()) {
                    $case = $const->getValue();
                    $entry['cases'][$const->getName()] = $case instanceof \BackedEnum ? $case->value : null;
                } else {
                    $entry['constants'][$const->getName()] = $const->getValue();
                }
            }
            foreach ($rc->getMethods() as $m) {
                if ($m->getDeclaringClass()->getName() !== $rc->getName() || !$m->isPublic()) {
                    continue;
                }
                $params = [];
                foreach ($m->getParameters() as $p) {
                    $default = null;
                    if ($p->isDefaultValueAvailable()) {
                        $default = $p->isDefaultValueConstant()
                            ? $p->getDefaultValueConstantName()
                            : var_export($p->getDefaultValue(), true);
                    }
                    $params[] = [
                        'name' => $p->getName(),
                        'type' => $p->getType() === null ? null : (string) $p->getType(),
                        'optional' => $p->isOptional(),
                        'default' => $default,
                        'byRef' => $p->isPassedByReference(),
                    ];
                }
                $entry['methods'][$m->getName()] = [
                    'static' => $m->isStatic(),
                    'params' => $params,
                    'return' => $m->getReturnType() === null ? null : (string) $m->getReturnType(),
                ];
            }
            if (!$rc->isEnum()) {
                foreach ($rc->getProperties(\ReflectionProperty::IS_PUBLIC) as $p) {
                    $entry['properties'][$p->getName()] = [
                        'type' => $p->getType() === null ? null : (string) $p->getType(),
                        'readonly' => $p->isReadOnly(),
                        'static' => $p->isStatic(),
                        'hasDefault' => $p->hasDefaultValue(),
                    ];
                }
            }
            ksort($entry['constants']);
            ksort($entry['methods']);
            ksort($entry['properties']);
            $surface[$short] = $entry;
        }
        return $surface;
    }

    /**
     * Each door over inputs that reach its value, its faults and its edges, reduced.
     *
     * @return array<string, array<string, mixed>>
     */
    public static function doors(): array
    {
        $inv = NumFormat::invariant();
        $de = new NumFormat(',', '.', NumFormat::ALL, '€');
        $numeric = ['0', '-1', '127', '128', '-129', '255', '256', '65535', '4294967295', '9223372036854775807',
            '9223372036854775808', '18446744073709551615', '-9223372036854775808', '(1,234)', '1,234.5', '1e3',
            '0xFF', '0b101', '50%', '$5', '  42  ', '', '   ', 'abc', '1.5', '1e400', '-0', '0.1',
            '79228162514264337593543950335', '79228162514264337593543950336', '1.00000000000000000000000000001',
            "\xff", 'NaN', '1,2,3'];
        $doors = [];
        foreach (['i8', 'i16', 'i32', 'i64', 'u8', 'u16', 'u32', 'u64', 'f32', 'f64', 'decimal'] as $door) {
            foreach ($numeric as $text) {
                $doors[$door][self::key($text)] = self::run(static fn () => Cast::$door($text, $inv));
            }
            $doors["{$door} de"]['1.234,5 €'] = self::run(static fn () => Cast::$door('1.234,5 €', $de));
            $doors["{$door} de"]['-12'] = self::run(static fn () => Cast::$door('-12', $de));
        }
        foreach (['yes', 'No', 'T', '0', 'enabled', 'checked', 'out', 'maybe', '', ' true '] as $text) {
            $doors['bool'][self::key($text)] = self::run(static fn () => Cast::bool($text));
        }
        $chars = ['A', ' ', 'é', '😀', 'U+00E9', '&#233;', '&#xE9;', '0x41', '&H41', '65', 'U+D800', 'U+110000', 'AB',
            ''];
        foreach ($chars as $text) {
            $doors['char'][self::key($text)] = self::run(static fn () => Cast::char($text));
        }
        $uuids = ['018bcfe5-6800-7000-8000-000000000001', '{018BCFE5-6800-7000-8000-000000000001}',
            '018bcfe568007000800000000000000a', 'urn:uuid:018bcfe5-6800-7000-8000-000000000001',
            '(018bcfe5-6800-7000-8000-000000000001)',
            '{0x018bcfe5,0x6800,0x7000,{0x80,0x00,0x00,0x00,0x00,0x00,0x00,0x01}}',
            'not-a-uuid', ''];
        foreach ($uuids as $text) {
            $doors['uuid'][self::key($text)] = self::run(static fn () => Cast::uuid($text));
            $doors['uuidBytes'][self::key($text)] = self::run(static fn () => Cast::uuidBytes($text));
        }
        $timestamps = ['2026-01-02T15:04:05.123456789+05:00', '1970-01-01T00:00:00Z', '1969-12-31T23:59:59.999999Z',
            '0001-01-01T00:00:00Z', '9999-12-31T23:59:59.999999999Z', '1900-02-28T12:00:00.5-01:30',
            '2026-01-02T15:04:05', '2026-13-01T00:00:00Z', ''];
        foreach ($timestamps as $text) {
            $doors['timestamp'][self::key($text)] = self::run(static fn () => Cast::timestamp($text));
        }
        foreach (\HyperCast\UnixPrecision::cases() as $precision) {
            foreach (['1767348245123', '-1', '0', '-1767348245123456', '99999999999999999999', 'x', ''] as $text) {
                $doors["unix {$precision->name}"][self::key($text)] =
                    self::run(static fn () => Cast::unix($text, $precision));
            }
        }
        foreach (\HyperCast\ExcelEpoch::cases() as $epoch) {
            foreach (['45292.75', '60', '61', '59.5', '0', '-1', '1e6', '2958465.99999999', 'x', ''] as $text) {
                $doors["excelSerial {$epoch->name}"][self::key($text)] =
                    self::run(static fn () => Cast::excelSerial($text, $epoch));
            }
            foreach ([45292.75, 60.0, 61.0, 0.0, -1.0, NAN, INF, 0.123456789123, 2958465.999999999] as $value) {
                $doors["excelSerialFromFloat {$epoch->name}"][self::key($value)] =
                    self::run(static fn () => Cast::excelSerialFromFloat($value, $epoch));
            }
        }
        $dates = ['2026-01-07', '1/7/2026', '7.1.2026', '2026/1/7', '31/12/1999', '2/29/2024', '2/29/2023',
            '0001-01-01', '9999-12-31', '-0001-01-01', ''];
        foreach ($dates as $text) {
            $doors['date'][self::key($text)] = self::run(static fn () => Cast::date($text));
            foreach (\HyperCast\DateOrder::cases() as $order) {
                $doors["date {$order->name}"][self::key($text)] = self::run(static fn () => Cast::date($text, $order));
            }
        }
        $periods = new DayPeriods('vorm.', 'nachm.');
        $datetimes = ['1/7/2026 3:04 PM', '2026-01-07 15:04:05', '2026-01-07T15:04:05.123456789', 'PM 3:04 1/7/2026',
            '1/7/2026 3:04 nachm.', '1/7/2026 12:00 AM', '1/7/2026', '1/7/2026 25:00', '0001-01-01 00:00:00.000000001',
            ''];
        foreach ($datetimes as $text) {
            foreach (\HyperCast\DateOrder::cases() as $order) {
                $doors["datetime {$order->name}"][self::key($text)] =
                    self::run(static fn () => Cast::datetime($text, $order));
                $doors["datetime {$order->name} periods"][self::key($text)] =
                    self::run(static fn () => Cast::datetime($text, $order, $periods));
            }
        }
        foreach (['00:00', '23:59:59.999999999', '12:34:56.5', '24:00', '1:2', 'x', ''] as $text) {
            $doors['time'][self::key($text)] = self::run(static fn () => Cast::time($text));
        }
        $durations = ['PT1H', 'P1DT2H', '-PT0.5S', '01:02:03', '-1:00:00.25', '1.5s', '-0.000000001s', 'P1M', 'x', ''];
        foreach ($durations as $text) {
            $doors['duration'][self::key($text)] = self::run(static fn () => Cast::duration($text));
        }
        $floats = [0.1, 0.30000000000000004, -1.5, 0.0, -0.0, 1e20, 1e-30, 7.9228162514264337593543950335e28, 1e29,
            NAN, INF, -INF, 123456789.125];
        foreach ($floats as $value) {
            $doors['decimalFromFloat'][self::key($value)] = self::run(static fn () => Cast::decimalFromFloat($value));
            $doors['excelTime'][self::key($value)] = self::run(static fn () => Cast::excelTime($value));
            $doors['excelDuration'][self::key($value)] = self::run(static fn () => Cast::excelDuration($value));
        }
        $doors['optional']['Empty'] = self::run(static fn () => Cast::optional(Cast::i32('', $inv)));
        $doors['optional']['Malformed'] = self::run(static fn () => Cast::optional(Cast::i32('x', $inv)));
        $doors['optional']['Success'] = self::run(static fn () => Cast::optional(Cast::i32('5', $inv)));
        $doors['nativeVersion']['()'] = self::run(
            static fn () => preg_match('/^\d+\.\d+\.\d+$/', Cast::nativeVersion())
        );
        return $doors;
    }

    /**
     * NumFormat's and DayPeriods' constructors, factories and packed layouts: what each
     * declares, and the caller bugs each refuses.
     *
     * @return array<string, array<string, mixed>>
     */
    public static function formats(): array
    {
        $out = [];
        foreach (self::formatArguments() as $label => $args) {
            $out['NumFormat::__construct'][$label] = self::run(
                static fn () => self::numFormat(new NumFormat(...$args))
            );
        }
        foreach (self::periodArguments() as $label => $args) {
            $out['DayPeriods::__construct'][$label] = self::run(static function () use ($args): array {
                $periods = new DayPeriods(...$args);
                return ['am' => $periods->am, 'pm' => $periods->pm, 'packed' => bin2hex($periods->packed())];
            });
        }
        foreach (self::localeconvArguments() as $label => $conv) {
            $out['NumFormat::fromLocaleconv'][$label] =
                self::run(static fn () => self::numFormat(NumFormat::fromLocaleconv($conv)));
        }
        $out['NumFormat::invariant']['()'] = self::run(static fn () => self::numFormat(NumFormat::invariant()));
        $out['NumFormat::detect']['()'] = self::run(static fn () => self::numFormat(NumFormat::detect()));
        $out['NumFormat::invariant']['shared'] = NumFormat::invariant() === NumFormat::invariant();
        $out['NumFormat::detect']['shared'] = NumFormat::detect() === NumFormat::detect();
        return $out;
    }

    /**
     * What a NumberFormatter reads as: for each formatter set, the symbols and attributes a
     * formatter answered, and what 1.0 declared from them, through `fromNumberFormatter()` and
     * through a door. The test replays the answers through {@see RecordedFormatter}, so the
     * recording doesn't depend on the ICU version that made it.
     *
     * @return array<string, array<string, mixed>>
     */
    public static function formatters(): array
    {
        $out = [];
        $locales = ['en_US', 'de_DE', 'de_AT', 'fr_FR', 'fr_CH', 'sv_SE', 'ar_EG', 'fa_IR', 'he_IL', 'hi_IN', 'nl_NL',
            'ja_JP', 'es_ES', 'pt_BR', 'ru_RU', 'et_EE', 'de_CH', 'bn_BD'];
        foreach ($locales as $locale) {
            $styles = ['DECIMAL' => \NumberFormatter::DECIMAL, 'CURRENCY' => \NumberFormatter::CURRENCY];
            foreach ($styles as $name => $style) {
                $formatter = new \NumberFormatter($locale, $style);
                $out["{$locale} {$name}"] = self::formatterCase(RecordedFormatter::record($formatter));
            }
        }
        $odd = [
            'a trailing negative pattern' => ['symbols' => [0 => '.', 1 => ','], 'attributes' => [2 => '', 3 => '-']],
            'a negative prefix and suffix' => ['symbols' => [0 => '.', 1 => ','], 'attributes' => [2 => '(', 3 => ')']],
            'an over-long separator' => ['symbols' => [0 => '.........', 1 => ',']],
            'an over-long currency symbol' => ['symbols' => [0 => '.', 1 => ',', 8 => 'ABCDEFGHIJKLMNOPQ']],
            'equal separators' => ['symbols' => [0 => ',', 1 => ',']],
            'empty separators' => ['symbols' => [0 => '', 1 => '']],
            'a whitespace sign' => ['symbols' => [6 => '- ']],
            'a digit in a symbol' => ['symbols' => [3 => '%1']],
            'an ASCII zero digit' => ['symbols' => [4 => '0']],
            'a zero that starts no block' => ['symbols' => [4 => 'é']],
            'monetary separators equal to the number ones' => ['symbols' => [0 => ',', 1 => '.', 10 => ',', 17 => '.']],
            'a monetary group equal to the decimal' => ['symbols' => [0 => ',', 1 => ' ', 17 => ',']],
            'a monetary decimal equal to the group' => ['symbols' => [0 => '.', 1 => ',', 10 => ',']],
            'getSymbol failing' => ['symbols' => [0 => false, 1 => false, 8 => false], 'attributes' => [2 => false]],
        ];
        foreach ($odd as $label => $answers) {
            $out[$label] = self::formatterCase($answers);
        }
        return $out;
    }

    /**
     * One formatter set: the answers, `fromNumberFormatter()`'s format, and two doors through it.
     *
     * @param array{symbols?: array<int, string|false>, attributes?: array<int, string|false>} $answers
     * @return array<string, mixed>
     */
    public static function formatterCase(array $answers): array
    {
        $formatter = RecordedFormatter::replay($answers);
        $sample = $formatter->sample();
        return [
            'answers' => RecordedFormatter::encode($answers),
            'sample' => self::reduce($sample),
            'format' => self::run(static fn () => self::numFormat(NumFormat::fromNumberFormatter($formatter))),
            'f64' => self::run(static fn () => Cast::f64($sample, $formatter)),
            'decimal' => self::run(static fn () => Cast::decimal($sample, $formatter)),
            'i32 grouped' => self::run(static fn () => Cast::i32('1' . $formatter->symbol(1) . '234', $formatter)),
        ];
    }

    /**
     * Interop's values and the platform table.
     *
     * @return array<string, array<string, mixed>>
     */
    public static function interop(): array
    {
        $values = \HyperCast\Interop\NativeValues::class;
        $platform = \HyperCast\Interop\NativePlatform::class;
        $out = [];
        $instants = [[0, 0], [1767348245, 123456789], [-1, 999999999], [-62135596800, 0], [253402300799, 999999999],
            [-1, 1], [1, 999]];
        foreach ($instants as $args) {
            $out['instant'][implode(',', $args)] = self::run(static fn () => $values::instant(...$args));
        }
        $dates = [[2026, 1, 7], [1, 1, 1], [9999, 12, 31], [1969, 12, 31], [2024, 2, 29], [0, 3, 1], [-1, 12, 31]];
        foreach ($dates as $args) {
            $out['date'][implode(',', $args)] = self::run(static fn () => $values::date(...$args));
        }
        $civils = [[2026, 1, 7, 54240000000000], [1, 1, 1, 1], [9999, 12, 31, 86399999999999], [1969, 12, 31, 999],
            [2024, 2, 29, 86400000000000]];
        foreach ($civils as $args) {
            $out['civil'][implode(',', $args)] = self::run(static fn () => $values::civil(...$args));
        }
        foreach (['0123456789abcdeffedcba9876543210', '00000000000000000000000000000000', 'ffff', ''] as $hex) {
            $out['uuid'][$hex] = self::run(static fn () => $values::uuid((string) hex2bin($hex)));
        }
        foreach ([[1, 0, 0], [2, 3, 4], [3, 7, 1], [0, 0, 0], [4, 1, 1], [-1, 0, 0]] as $args) {
            $out['fault'][implode(',', $args)] = self::run(static fn () => $values::fault(...$args));
        }
        foreach ([0x010000, 0x000901, 0x0A0B0C, 0, -1, 0x1FFFFFF] as $packed) {
            $out['version'][(string) $packed] = self::run(static fn () => $values::version($packed));
        }
        $machines = [['Linux', 'x86_64', false], ['Linux', 'aarch64', true], ['Linux', 'AMD64', false],
            ['Darwin', 'arm64', false], ['Darwin', 'x86_64', false], ['Windows', 'ARM64', false],
            ['Windows', 'x86', false], ['Linux', 'armv7l', false], ['BSD', 'amd64', false], ['Linux', 'ARM64', true]];
        foreach ($machines as [$os, $machine, $musl]) {
            foreach ([8, 4] as $size) {
                $label = "{$os} {$machine}" . ($musl ? ' musl' : '') . " {$size}";
                $out['resolve'][$label] =
                    self::run(static fn () => $platform::resolve('hypercast', $os, $machine, $size, $musl));
            }
        }
        $out['resolve']['another base name'] =
            self::run(static fn () => $platform::resolve('hypertabular', 'Linux', 'x86_64', 8, true));
        return $out;
    }

    /**
     * The value classes under PHP's generic object machinery: serialization, export,
     * comparison, clone, and the refusals a readonly class gives.
     *
     * @return array<string, array<string, mixed>>
     */
    public static function objects(): array
    {
        $samples = self::samples();
        $out = [];
        foreach ($samples as $label => $object) {
            $class = $object::class;
            $row = [
                'serialize' => self::text(serialize($object)),
                'unserialize' => self::run(static fn () => unserialize(serialize($object)) == $object),
                'var_export' => self::text(var_export($object, true)),
                'json_encode' => self::run(static fn () => json_encode($object, JSON_THROW_ON_ERROR)),
                'print_r' => self::text(print_r($object, true)),
                'array' => self::reduce((array) $object),
                'clone' => self::run(static fn () => self::reduce(clone $object) === self::reduce($object)),
                'string' => self::run(static fn () => (string) $object),
                'isset missing' => isset($object->missing),
            ];
            $first = array_key_first(get_object_vars($object));
            $row['write'] = self::run(static function () use ($object, $first): mixed {
                $object->$first = $object->$first;
                return 'written';
            });
            $row['unset'] = self::run(static function () use ($object, $first): mixed {
                unset($object->$first);
                return 'unset';
            });
            $row['dynamic'] = self::run(static function () use ($object): mixed {
                $object->dynamic = 1;
                return 'written';
            });
            $row['construct again'] = self::run(static function () use ($object): mixed {
                $object->__construct(...array_values(get_object_vars($object)));
                return 'constructed';
            });
            $row['without constructor'] = self::run(static function () use ($class): mixed {
                $bare = (new \ReflectionClass($class))->newInstanceWithoutConstructor();
                return self::reduce((array) $bare);
            });
            $out[$label] = $row;
        }
        $pairs = [
            'Success equal' => [new \HyperCast\Success(5), new \HyperCast\Success(5)],
            'Success loose' => [new \HyperCast\Success(5), new \HyperCast\Success('5')],
            'Success differ' => [new \HyperCast\Success(5), new \HyperCast\Success(6)],
            'Fault differ' => [$samples['Fault'], new \HyperCast\Fault(\HyperCast\CastFailure::Malformed, 2, 1)],
            'Decimal equal' => [new \HyperCast\Decimal('11', 1, false), \HyperCast\Decimal::fromLimbs(11, 0, 1, false)],
            'Duration differ' => [new \HyperCast\Duration(1, 0), new \HyperCast\Duration(0, 999999999)],
            'NumFormat equal' => [NumFormat::invariant(), new NumFormat('.', ',', NumFormat::ALL)],
            'NumFormat differ' => [NumFormat::invariant(), NumFormat::detect()],
            'Success vs Fault' => [new \HyperCast\Success(5), $samples['Fault']],
        ];
        foreach ($pairs as $label => [$a, $b]) {
            $out['compare'][$label] = [
                '==' => $a == $b,
                '<=>' => self::run(static fn () => $a <=> $b),
            ];
        }
        return $out;
    }

    /**
     * One instance of each value class.
     *
     * @return array<string, object>
     */
    public static function samples(): array
    {
        return [
            'Success int' => new \HyperCast\Success(42),
            'Success null' => new \HyperCast\Success(null),
            'Success string' => new \HyperCast\Success("a\0b"),
            'Success float' => new \HyperCast\Success(1.5),
            'Success date' => new \HyperCast\Success(new \DateTimeImmutable('2026-01-02T03:04:05.123456Z')),
            'Success decimal' => new \HyperCast\Success(new \HyperCast\Decimal('12345', 1, true)),
            'Fault' => new \HyperCast\Fault(\HyperCast\CastFailure::OutOfRange, 3, 4),
            'Decimal' => new \HyperCast\Decimal('79228162514264337593543950335', 28, false),
            'Decimal zero' => \HyperCast\Decimal::fromLimbs(0, 0, 0, false),
            'Duration' => new \HyperCast\Duration(-1, -500000000),
            'NumFormat' => new NumFormat(
                ',',
                ' ',
                NumFormat::ALL | NumFormat::TRAILING_SIGN,
                'kr.',
                '',
                '.',
                "\u{2212}",
                '',
                '٪',
                '٠',
            ),
            'DayPeriods' => new DayPeriods('a. m.', 'p. m.'),
        ];
    }

    /**
     * The constructor argument lists recorded for NumFormat.
     *
     * @return array<string, list<mixed>>
     */
    private static function formatArguments(): array
    {
        $flags = NumFormat::ALL;
        return [
            'invariant' => ['.', ',', $flags],
            'comma decimal' => [',', '.', $flags],
            'two-character group' => ['.', "'\u{00A0}", $flags],
            'eight-byte separator' => ['.', str_repeat('\'', 8), $flags],
            'nine-byte separator' => ['.', str_repeat('\'', 9), $flags],
            'empty decimal' => ['', ',', $flags],
            'empty group' => ['.', '', $flags],
            'equal separators' => [',', ',', $flags],
            'digit separator' => ['1', ',', $flags],
            'space separator' => ['.', ' ', $flags],
            'ill-formed separator' => ["\xC3", ',', $flags],
            'surrogate separator' => ["\xED\xA0\x80", ',', $flags],
            'currency' => ['.', ',', $flags, '$'],
            'currency with a space' => ['.', ',', $flags, 'kr .'],
            'currency of 16 bytes' => ['.', ',', $flags, str_repeat('€', 5) . 'x'],
            'currency of 17 bytes' => ['.', ',', $flags, str_repeat('x', 17)],
            'currency with a digit' => ['.', ',', $flags, 'X1'],
            'currency separators' => ['.', ',', $flags, '€', ',', '.'],
            'currency separators equal' => ['.', ',', $flags, '€', ',', ','],
            'currency decimal equal to the group' => ['.', ',', $flags, '', ','],
            'currency group equal to the decimal' => ['.', ',', $flags, '', '', '.'],
            'currency decimal with a digit' => ['.', ',', $flags, '', '9'],
            'signs' => ['.', ',', $flags, '', '', '', "\u{2212}", '＋'],
            'sign with whitespace' => ['.', ',', $flags, '', '', '', "\u{2212} "],
            'sign with a tab' => ['.', ',', $flags, '', '', '', '', "+\t"],
            'percent' => ['.', ',', $flags, '', '', '', '', '', '٪'],
            'percent with a digit' => ['.', ',', $flags, '', '', '', '', '', '%0'],
            'digit zero' => ['.', ',', $flags, '', '', '', '', '', '', '٠'],
            'fullwidth digit zero' => ['.', ',', $flags, '', '', '', '', '', '', '０'],
            'ASCII digit zero' => ['.', ',', $flags, '', '', '', '', '', '', '0'],
            'two-character digit zero' => ['.', ',', $flags, '', '', '', '', '', '', '٠٠'],
            'digit zero past a block' => ['.', ',', $flags, '', '', '', '', '', '', "\u{0677}"],
            'ill-formed digit zero' => ['.', ',', $flags, '', '', '', '', '', '', "\xD9"],
            'negative flags' => ['.', ',', -1],
            'flags past 32 bits' => ['.', ',', 1 << 40],
            'no flags' => ['.', ',', 0],
        ];
    }

    /**
     * The constructor argument lists recorded for DayPeriods.
     *
     * @return array<string, list<mixed>>
     */
    private static function periodArguments(): array
    {
        return [
            'German' => ['vorm.', 'nachm.'],
            'Korean' => ['오전', '오후'],
            'Spanish' => ['a. m.', 'p. m.'],
            'empty' => ['', 'PM'],
            'sixteen bytes' => [str_repeat('a', 16), 'p'],
            'seventeen bytes' => [str_repeat('a', 17), 'p'],
            'a digit' => ['a1', 'p'],
            'leading space' => [' am', 'pm'],
            'trailing tab' => ['am', "pm\t"],
            'ill-formed' => ["\xC3", 'pm'],
        ];
    }

    /**
     * The localeconv() arrays recorded for fromLocaleconv().
     *
     * @return array<string, array<string, mixed>|null>
     */
    private static function localeconvArguments(): array
    {
        return [
            'C locale' => ['decimal_point' => '.', 'thousands_sep' => '', 'currency_symbol' => ''],
            'de_DE' => ['decimal_point' => ',', 'thousands_sep' => '.', 'currency_symbol' => '€',
                'mon_decimal_point' => ',', 'mon_thousands_sep' => '.', 'negative_sign' => '-', 'positive_sign' => '',
                'n_sign_posn' => 1],
            'comma decimal, no group' => ['decimal_point' => ',', 'thousands_sep' => ''],
            'no decimal, dot group' => ['decimal_point' => '', 'thousands_sep' => '.'],
            'nothing' => [],
            'trailing sign 2' => ['n_sign_posn' => 2, 'negative_sign' => "\u{2212}"],
            'trailing sign 4' => ['n_sign_posn' => 4],
            'sign position as a string' => ['n_sign_posn' => '2'],
            'monetary group equal to the decimal' => ['decimal_point' => ',', 'thousands_sep' => ' ',
                'mon_thousands_sep' => ','],
            'monetary decimal equal to the group' => ['decimal_point' => '.', 'thousands_sep' => ',',
                'mon_decimal_point' => ','],
            'non-string fields' => ['decimal_point' => 1, 'thousands_sep' => null, 'currency_symbol' => ['x']],
            'over-long currency' => ['currency_symbol' => str_repeat('x', 17)],
            'plus sign' => ['positive_sign' => '+', 'negative_sign' => '-'],
        ];
    }

    /**
     * A NumFormat as its public fields and packed layout.
     *
     * @return array<string, mixed>
     */
    public static function numFormat(NumFormat $format): array
    {
        $fields = self::reduce(get_object_vars($format));
        $fields['packed'] = bin2hex($format->packed());
        return $fields;
    }

    /**
     * Runs `$call`, reducing what it returned, or the class and message of what it threw,
     * with every PHP error it raised.
     *
     * @return array<string, mixed>
     */
    public static function run(\Closure $call, ?string $file = null): array
    {
        $raised = [];
        set_error_handler(static function (int $level, string $message) use (&$raised): bool {
            $raised[] = self::levelName($level) . ': ' . $message;
            return true;
        });
        try {
            $outcome = ['result' => self::reduce($call())];
        } catch (\Throwable $e) {
            $message = $e->getMessage();
            if ($file !== null) {
                $message = str_replace($file, '{caller}', $message);
            }
            $outcome = ['exception' => $e::class, 'message' => self::text($message)];
        } finally {
            restore_error_handler();
        }
        if ($raised !== []) {
            $outcome['errors'] = $raised;
        }
        return $outcome;
    }

    /** A value as JSON carries it exactly. */
    public static function reduce(mixed $value, int $depth = 0): mixed
    {
        if ($depth > 8) {
            return '(deeper)';
        }
        return match (true) {
            $value === null, \is_bool($value), \is_int($value) => $value,
            \is_float($value) => 'float ' . var_export($value, true),
            \is_string($value) => self::text($value),
            \is_array($value) => array_map(static fn (mixed $v): mixed => self::reduce($v, $depth + 1), $value),
            $value instanceof \UnitEnum => 'enum ' . $value::class . '::' . $value->name,
            $value instanceof \DateTimeInterface => 'date ' . $value::class . ' '
                . $value->format('Y-m-d\TH:i:s.uP') . ' ' . $value->getTimezone()->getName()
                . ' type ' . ((array) $value)['timezone_type'],
            \is_object($value) => ['class' => self::className($value)]
                + array_map(static fn (mixed $v): mixed => self::reduce($v, $depth + 1), get_object_vars($value)),
            default => get_debug_type($value),
        };
    }

    /** A string as JSON carries it: itself when it is UTF-8, else its bytes as hex. */
    public static function text(string $text): string
    {
        return preg_match('//u', $text) === 1 ? $text : 'bytes ' . bin2hex($text);
    }

    /** An array key for an input. */
    private static function key(mixed $input): string
    {
        return \is_string($input) ? 'text ' . self::text($input) : 'float ' . var_export($input, true);
    }

    /** A class name without an anonymous class's location. */
    private static function className(object $object): string
    {
        return explode("\0", $object::class)[0];
    }

    private static function levelName(int $level): string
    {
        return match ($level) {
            E_WARNING, E_USER_WARNING => 'Warning',
            E_NOTICE, E_USER_NOTICE => 'Notice',
            E_DEPRECATED, E_USER_DEPRECATED => 'Deprecated',
            default => "E_{$level}",
        };
    }
}
