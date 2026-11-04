<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use HyperCast\Tests\Support\CoercionMatrix;
use PHPUnit\Framework\Attributes\DataProvider;
use PHPUnit\Framework\TestCase;

/**
 * Every public parameter takes its arguments as 1.0's userland methods did: weak-mode callers
 * get PHP's standard coercion, with its deprecations and warnings; strict callers get the
 * `TypeError`s, worded as for a userland function (`..., called in F on line L`). The expected
 * outcomes are 1.0's own, recorded by tools/capture-v1-fixtures.php --coercion on each minor.
 */
final class CoercionTest extends TestCase
{
    private static string $scratch;

    /** @var array<string, array<string, array<string, array<string, mixed>>>> */
    private static array $actual;

    public static function setUpBeforeClass(): void
    {
        self::$scratch = sys_get_temp_dir() . '/hypercast-coercion-test-' . getmypid();
        if (!is_dir(self::$scratch)) {
            mkdir(self::$scratch, 0700, true);
        }
        $flags = JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR | JSON_PRESERVE_ZERO_FRACTION;
        self::$actual = json_decode(json_encode(CoercionMatrix::run(self::$scratch), $flags), true, flags: $flags);
    }

    public static function tearDownAfterClass(): void
    {
        array_map('unlink', glob(self::$scratch . '/*') ?: []);
        rmdir(self::$scratch);
    }

    /**
     * Every minor's recording: the engine's own messages (a deprecation's wording, PHP 8.5's
     * NAN warnings) differ between minors, so 1.0 was recorded on each.
     *
     * @return array<string, array<string, mixed>>
     */
    private static function captures(): array
    {
        $json = (string) file_get_contents(__DIR__ . '/fixtures/v1/coercion.json');
        return json_decode($json, true, flags: JSON_THROW_ON_ERROR)['captures'];
    }

    /**
     * 1.0's recording on the running PHP minor.
     *
     * @return array<string, array<string, array<string, array<string, mixed>>>>
     */
    private static function expected(): array
    {
        $minor = PHP_MAJOR_VERSION . '.' . PHP_MINOR_VERSION;
        $captures = self::captures();
        if (!isset($captures[$minor])) {
            throw new \RuntimeException("coercion.json holds no recording of 1.0 on PHP {$minor}");
        }
        return $captures[$minor]['matrix'];
    }

    /** @return iterable<string, array{string, string}> */
    public static function modesAndCases(): iterable
    {
        foreach (self::expected() as $mode => $cases) {
            foreach (array_keys($cases) as $case) {
                yield "{$mode}: {$case}" => [$mode, $case];
            }
        }
    }

    #[DataProvider('modesAndCases')]
    public function testEachInputIsTakenAs1Point0TookIt(string $mode, string $case): void
    {
        $expected = self::expected()[$mode][$case];
        $this->assertSame(array_keys($expected), array_keys(self::$actual[$mode][$case] ?? []));
        foreach ($expected as $input => $outcome) {
            if (self::debugEngineWarnsTwice($mode, $case, $input)) {
                $outcome['errors'] = [...$outcome['errors'], ...$outcome['errors']];
            }
            $outcome = self::refusedFlags($case, self::widened($case, $outcome));
            $actual = self::$actual[$mode][$case][$input];
            if ($case === 'NumFormat::fromLocaleconv $conv' && $input === 'null') {
                $actual = self::withGroupingRestored($actual);
            }
            $actual = self::withoutDefaultGrouping($actual, $case === 'NumFormat::fromLocaleconv $conv' ? 0 : 256);
            $this->assertSame($outcome, $actual, "{$mode}, {$case}, input {$input}");
        }
    }

    /**
     * The one declared edit: `Cast::optional()` takes a checked verdict too, so the type its
     * `TypeError` names widens, additively; every input it refused, it still refuses.
     *
     * @param array<string, mixed> $outcome
     * @return array<string, mixed>
     */
    private static function widened(string $case, array $outcome): array
    {
        if ($case !== 'Cast::optional $verdict' || !isset($outcome['message'])) {
            return $outcome;
        }
        $outcome['message'] = str_replace(
            'must be of type HyperCast\\Success|HyperCast\\Fault,',
            'must be of type HyperCast\\Success|HyperCast\\Checked|HyperCast\\Fault|HyperCast\\Violation,',
            $outcome['message'],
            $count
        );
        self::assertSame(1, $count, $case);
        return $outcome;
    }

    /**
     * A declared edit: `NumFormat`'s `flags` holds the styles only, bits 0 to 255 (1.0 ignored
     * every other bit, and 1.1 refuses them, spec item 3), so the two integers 1.0 accepted
     * outside that range are refused now: the negative one and the one past 32 bits. Every
     * other input, and every other message, is 1.0's.
     *
     * @param array<string, mixed> $outcome
     * @return array<string, mixed>
     */
    private static function refusedFlags(string $case, array $outcome): array
    {
        $flags = $outcome['result']['flags'] ?? null;
        if ($case !== 'NumFormat::__construct $flags' || !\is_int($flags) || ($flags >= 0 && $flags <= 255)) {
            return $outcome;
        }
        $outcome['exception'] = 'InvalidArgumentException';
        // A policy spelled in the flags (bits 8 and 9) is named the `grouping` argument's.
        $hint = ($flags & 768) !== 0 ? '; the grouping policy is the `grouping` argument' : '';
        $outcome['message'] = "Flags must hold only the styles NumFormat declares, 0 through 255; got {$flags}{$hint}";
        unset($outcome['result']);
        return $outcome;
    }

    /**
     * A declared edit: `fromLocaleconv(null)` reads the process locale, and the `C` locale's
     * `localeconv()` has an empty `grouping` and `mon_grouping`: neither pattern groups, so the
     * bridge clears `GROUPING` (and declares the default policy), where 1.0 left every lenience
     * on. The one flag is put back before the rest of the dump is held to 1.0's.
     *
     * @param array<string, mixed> $outcome
     * @return array<string, mixed>
     */
    private static function withGroupingRestored(array $outcome): array
    {
        $format = $outcome['result'] ?? null;
        if (\is_array($format) && ($format['grouping'] ?? null) === 256 && ($format['flags'] & 1) === 0) {
            $outcome['result']['flags'] |= 1;
            $outcome['result']['grouping'] = 0;
        }
        return $outcome;
    }

    /**
     * The other declared edit: 1.1's `NumFormat` has a `grouping` property, which a format built
     * without a policy holds as the default, strict Western; a format `fromLocaleconv()` builds
     * from an array that says nothing of grouping is legacy (nothing known of the platform's
     * patterns), which is the 1.0 rule. Dropped from a dump where it holds that default, so every
     * other field is still held to 1.0's; any other policy stays and fails.
     *
     * @param array<string, mixed> $outcome
     * @return array<string, mixed>
     */
    private static function withoutDefaultGrouping(array $outcome, int $default): array
    {
        if (
            ($outcome['class'] ?? null) === 'HyperCast\\NumFormat'
            && ($outcome['grouping'] ?? null) === $default
        ) {
            unset($outcome['grouping']);
        }
        foreach ($outcome as $key => $value) {
            if (\is_array($value)) {
                $outcome[$key] = self::withoutDefaultGrouping($value, $default);
            }
        }
        return $outcome;
    }

    /**
     * The one place a debug (ZEND_DEBUG) PHP differs: before an internal function runs, it
     * checks the arguments against their declared types itself, and on 8.5 that check raises
     * the "unexpected NAN value was coerced to bool" warning for a NAN given to a `bool`
     * parameter, which the call then raises again. 1.0's userland method was never checked
     * that way.
     */
    private static function debugEngineWarnsTwice(string $mode, string $case, string $input): bool
    {
        return ZEND_DEBUG_BUILD && PHP_VERSION_ID >= 80500 && $mode === 'weak' && $input === 'NAN'
            && \in_array($case, ['Decimal::__construct $negative', 'Decimal::fromLimbs $negative',
                'NativePlatform::resolve $musl'], true);
    }

    public function testTheFixtureCoversEveryMinorCaseAndInputInBothModes(): void
    {
        $this->assertSame(['8.3', '8.4', '8.5'], array_keys(self::captures()));
        foreach (self::captures() as $capture) {
            $this->assertSame(['weak', 'strict'], array_keys($capture['matrix']));
            foreach ($capture['matrix'] as $cases) {
                $this->assertSame(array_keys(CoercionMatrix::cases()), array_keys($cases));
                foreach ($cases as $case => $inputs) {
                    $this->assertSame(array_keys(CoercionMatrix::inputs()), array_keys($inputs), $case);
                }
            }
        }
    }

    public function testTheMatrixTellsTheModesApart(): void
    {
        // Guards the harness itself: if strictness weren't the caller file's, the two modes
        // would record the same outcomes.
        $weak = self::$actual['weak']['Duration::__construct $seconds']['numeric string'];
        $strict = self::$actual['strict']['Duration::__construct $seconds']['numeric string'];
        $this->assertSame(['result' => ['class' => 'HyperCast\\Duration', 'seconds' => 7, 'nanos' => 0]], $weak);
        $this->assertSame('TypeError', $strict['exception'] ?? null);
    }
}
