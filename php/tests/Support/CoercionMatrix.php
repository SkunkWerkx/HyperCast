<?php

declare(strict_types=1);

namespace HyperCast\Tests\Support;

/**
 * The argument coercion matrix: the public parameters of the API, each called from a
 * weak-mode file and from a `declare(strict_types=1)` file with a fixed set of inputs,
 * recording what each call returned or threw and every warning or deprecation it raised.
 *
 * Strictness belongs to the file a call is written in, so the calls live in generated caller
 * files ({@see callerSource()}), one per mode, and this class only drives them. The same code
 * recorded 1.0's behaviour (tools/capture-v1-fixtures.php --coercion, into
 * tests/fixtures/v1/coercion.json) and checks the extension against it (tests/CoercionTest.php).
 *
 * Every parameter type the API declares is covered by at least one case. Left out: the doors'
 * `$text` beyond one per signature (every door declares the same `string $text`), and the
 * parameters whose inputs need ext-ffi or ext-intl or name this platform's files, which the
 * other tests cover (InteropTest, NumberFormatterTest, NativePlatformTest).
 */
final class CoercionMatrix
{
    /**
     * Each case: the method and parameter under test, and the call, as PHP source, taking the
     * input as `$v`.
     *
     * @return array<string, string>
     */
    public static function cases(): array
    {
        $c = '\HyperCast\Cast';
        $f = '\HyperCast\NumFormat';
        $inv = "{$f}::invariant()";
        $v = '\HyperCast\Interop\NativeValues';
        $p = '\HyperCast\Interop\NativePlatform';
        return [
            'Cast::bool $text' => "{$c}::bool(\$v)",
            'Cast::char $text' => "{$c}::char(\$v)",
            'Cast::i32 $text' => "{$c}::i32(\$v, {$inv})",
            'Cast::i32 $format' => "{$c}::i32('7', \$v)",
            'Cast::u64 $format' => "{$c}::u64('7', \$v)",
            'Cast::f64 $format' => "{$c}::f64('7', \$v)",
            'Cast::decimal $text' => "{$c}::decimal(\$v, {$inv})",
            'Cast::decimal $format' => "{$c}::decimal('7', \$v)",
            'Cast::uuid $text' => "{$c}::uuid(\$v)",
            'Cast::uuidBytes $text' => "{$c}::uuidBytes(\$v)",
            'Cast::timestamp $text' => "{$c}::timestamp(\$v)",
            'Cast::unix $text' => "{$c}::unix(\$v, \\HyperCast\\UnixPrecision::Seconds)",
            'Cast::unix $precision' => "{$c}::unix('7', \$v)",
            'Cast::excelSerial $epoch' => "{$c}::excelSerial('7', \$v)",
            'Cast::date $text' => "{$c}::date(\$v)",
            'Cast::date $order' => "{$c}::date('1/7/2026', \$v)",
            'Cast::datetime $order' => "{$c}::datetime('1/7/2026 3:04 PM', \$v)",
            'Cast::datetime $periods' => "{$c}::datetime('1/7/2026 3:04 PM', \\HyperCast\\DateOrder::Mdy, \$v)",
            'Cast::time $text' => "{$c}::time(\$v)",
            'Cast::duration $text' => "{$c}::duration(\$v)",
            'Cast::decimalFromFloat $value' => "{$c}::decimalFromFloat(\$v)",
            'Cast::excelSerialFromFloat $value' => "{$c}::excelSerialFromFloat(\$v, \\HyperCast\\ExcelEpoch::Y1900)",
            'Cast::excelSerialFromFloat $epoch' => "{$c}::excelSerialFromFloat(7.5, \$v)",
            'Cast::excelTime $value' => "{$c}::excelTime(\$v)",
            'Cast::excelDuration $value' => "{$c}::excelDuration(\$v)",
            'Cast::optional $verdict' => "{$c}::optional(\$v)",
            'Success::__construct $value' => 'new \HyperCast\Success($v)',
            'Fault::__construct $reason' => 'new \HyperCast\Fault($v, 1, 2)',
            'Fault::__construct $offset' => 'new \HyperCast\Fault(\HyperCast\CastFailure::Empty, $v, 2)',
            'Decimal::__construct $magnitude' => 'new \HyperCast\Decimal($v, 1, false)',
            'Decimal::__construct $scale' => "new \\HyperCast\\Decimal('12', \$v, false)",
            'Decimal::__construct $negative' => "new \\HyperCast\\Decimal('12', 1, \$v)",
            'Decimal::fromLimbs $lo' => '\HyperCast\Decimal::fromLimbs($v, 0, 1, false)',
            'Decimal::fromLimbs $negative' => '\HyperCast\Decimal::fromLimbs(12, 0, 1, $v)',
            'Duration::__construct $seconds' => 'new \HyperCast\Duration($v, 0)',
            'Duration::__construct $nanos' => 'new \HyperCast\Duration(0, $v)',
            'DayPeriods::__construct $am' => "new \\HyperCast\\DayPeriods(\$v, 'PM')",
            'NumFormat::__construct $decimalSep' => "new {$f}(\$v, ',', 0)",
            'NumFormat::__construct $flags' => "new {$f}('.', ',', \$v)",
            'NumFormat::__construct $currency' => "new {$f}('.', ',', 0, \$v)",
            'NumFormat::__construct $digitZero' => "new {$f}('.', ',', 0, digitZero: \$v)",
            'NumFormat::fromLocaleconv $conv' => "{$f}::fromLocaleconv(\$v)",
            'NumFormat::fromNumberFormatter $formatter' => "{$f}::fromNumberFormatter(\$v)",
            'NativeValues::instant $seconds' => "{$v}::instant(\$v, 0)",
            'NativeValues::date $year' => "{$v}::date(\$v, 1, 1)",
            'NativeValues::civil $nanosOfDay' => "{$v}::civil(2026, 1, 1, \$v)",
            'NativeValues::uuid $bytes' => "{$v}::uuid(\$v)",
            'NativeValues::fault $code' => "{$v}::fault(\$v, 0, 0)",
            'NativeValues::version $packed' => "{$v}::version(\$v)",
            'NativeValues::writeFormat $format' => "{$v}::writeFormat(\$v, null)",
            'NativePlatform::resolve $baseName' => "{$p}::resolve(\$v, 'Linux', 'x86_64', 8, false)",
            'NativePlatform::resolve $machine' => "{$p}::resolve('hypercast', 'Linux', \$v, 8, false)",
            'NativePlatform::resolve $intSize' => "{$p}::resolve('hypercast', 'Linux', 'x86_64', \$v, false)",
            'NativePlatform::resolve $musl' => "{$p}::resolve('hypercast', 'Linux', 'x86_64', 8, \$v)",
        ];
    }

    /**
     * The inputs every parameter is called with.
     *
     * @return array<string, mixed>
     */
    public static function inputs(): array
    {
        return [
            'int' => 7,
            'negative int' => -1,
            'int beyond 32 bits' => 1 << 40,
            'numeric string' => '7',
            'numeric string, leading whitespace' => ' 7',
            'numeric string, trailing whitespace' => '7 ',
            'integral float string' => '7.0',
            'fractional float string' => '7.5',
            'exponent string' => '1e3',
            'leading-numeric string' => '7abc',
            'non-numeric string' => 'abc',
            'empty string' => '',
            'integral float' => 7.0,
            'fractional float' => 7.5,
            'float beyond int' => 1.0e20,
            'NAN' => NAN,
            'INF' => INF,
            'true' => true,
            'false' => false,
            'null' => null,
            'array' => [],
            'locale array' => ['decimal_point' => ',', 'thousands_sep' => '.'],
            'stdClass' => new \stdClass(),
            'Stringable "7"' => new class {
                public function __toString(): string
                {
                    return '7';
                }
            },
            'DateTimeImmutable' => new \DateTimeImmutable('2025-06-15T12:34:56.789999Z'),
            'CastFailure::Malformed' => \HyperCast\CastFailure::Malformed,
            'DateOrder::Dmy' => \HyperCast\DateOrder::Dmy,
            'UnixPrecision::Milliseconds' => \HyperCast\UnixPrecision::Milliseconds,
            'ExcelEpoch::Y1904' => \HyperCast\ExcelEpoch::Y1904,
            'NumFormat::detect()' => \HyperCast\NumFormat::detect(),
            'DayPeriods' => new \HyperCast\DayPeriods('vorm.', 'nachm.'),
            'Success' => new \HyperCast\Success(7),
            'Fault' => new \HyperCast\Fault(\HyperCast\CastFailure::Empty, 0, 0),
        ];
    }

    /**
     * A caller file: returns `[case => closure]`, each closure making its case's call from this
     * file, so with this file's strictness.
     */
    public static function callerSource(bool $strict): string
    {
        $source = "<?php\n\n" . ($strict ? "declare(strict_types=1);\n\n" : '') . "return [\n";
        foreach (self::cases() as $case => $call) {
            $source .= '    ' . var_export($case, true) . " => static fn(mixed \$v): mixed => {$call},\n";
        }
        return $source . "];\n";
    }

    /**
     * Runs every case of both modes, the caller files written into `$dir`, and returns the
     * matrix: `[mode][case][input] => outcome`.
     *
     * @return array<string, array<string, array<string, array<string, mixed>>>>
     */
    public static function run(string $dir): array
    {
        $matrix = [];
        foreach (['weak' => false, 'strict' => true] as $mode => $strict) {
            $file = $dir . "/{$mode}-caller.php";
            file_put_contents($file, self::callerSource($strict));
            // Messages name the file as PHP resolved it (macOS's /var is /private/var).
            $file = (string) realpath($file);
            $calls = require $file;
            foreach ($calls as $case => $call) {
                foreach (self::inputs() as $label => $input) {
                    $matrix[$mode][$case][$label] = V1Probe::run(static fn () => $call($input), $file);
                }
            }
        }
        return $matrix;
    }
}
