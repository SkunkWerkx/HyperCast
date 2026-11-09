<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use FFI;
use HyperCast\Cast;
use HyperCast\Interop\NativeValues;
use HyperCast\NumFormat;
use HyperCast\Success;
use PHPUnit\Framework\TestCase;

/**
 * The downgrade guard (spec, item 3). Policy bits select behaviour; they cannot negotiate a
 * library version, and a 1.0 core ignores them and reads every format by 1.0's rules. The extension
 * links its own core, so only code that loads a C library itself (`Interop\NativeValues`, which
 * HyperTabular's PHP binding uses) can meet an old one. Its guard is two checks, neither a
 * hard-coded version: the FFI handle must declare the detailed exports a core that reads the
 * policy has, and `hypercast_version()` must not be older than this extension's.
 *
 * The 1.0 core is HYPERCAST_V1_LIBRARY: a build of tag v1.0.0 (the Release has no platform
 * assets), which CI makes the same way. The test is skipped without it.
 */
final class DowngradeTest extends TestCase
{
    private const DECLARATIONS = 'typedef struct { uint8_t bytes[8]; uint8_t len; } symbol8;'
        . 'typedef struct { uint8_t bytes[16]; uint8_t len; } symbol16;'
        . 'typedef struct { uint32_t size; uint32_t flags; uint32_t digit_zero; symbol8 decimal_sep;'
        . ' symbol8 group_sep; symbol8 currency_decimal_sep; symbol8 currency_group_sep; symbol8 minus_sign;'
        . ' symbol8 plus_sign; symbol8 percent; symbol16 currency; } format;'
        . 'typedef struct { uint32_t offset; uint32_t len; } fault;'
        . 'uint32_t hypercast_version(void);'
        . 'int32_t cast_i32(const uint8_t *ptr, uintptr_t len, const format *format, int32_t *out, fault *fault);';

    private static function oldCore(): FFI
    {
        if (!\extension_loaded('ffi')) {
            self::markTestSkipped('ext-ffi is not loaded');
        }
        $path = getenv('HYPERCAST_V1_LIBRARY');
        if (!\is_string($path) || !is_file($path)) {
            self::markTestSkipped('HYPERCAST_V1_LIBRARY names no 1.0 library (a build of tag v1.0.0)');
        }
        return FFI::cdef(self::DECLARATIONS, $path);
    }

    private static function parse(FFI $core, FFI\CData $format, string $text): ?int
    {
        $out = $core->new('int32_t');
        $fault = $core->new('fault');
        $input = $core->new('uint8_t[' . max(1, \strlen($text)) . ']');
        FFI::memcpy($input, $text, \strlen($text));
        $code = $core->cast_i32($input, \strlen($text), FFI::addr($format), FFI::addr($out), FFI::addr($fault));
        return $code === 0 ? $out->cdata : null;
    }

    /** The 1.0 core's answer for `$text`: the value, or `[offset, length]` of its fault. */
    private static function parseWith(FFI $core, FFI\CData $format, string $text): int|array
    {
        $out = $core->new('int32_t');
        $fault = $core->new('fault');
        $input = $core->new('uint8_t[' . max(1, \strlen($text)) . ']');
        FFI::memcpy($input, $text, \strlen($text));
        $code = $core->cast_i32($input, \strlen($text), FFI::addr($format), FFI::addr($out), FFI::addr($fault));
        return $code === 0 ? $out->cdata : [$fault->offset, $fault->len];
    }

    public function testAFormatAgainstAOneOhCoreIsRefusedNotParsedByOneOhsRules(): void
    {
        $core = self::oldCore();
        self::assertSame('1.0.0', NativeValues::version($core->hypercast_version()));

        // The hazard the guard exists for: written unguarded, the format is read by the 1.0 core by
        // 1.0's rules, and `12,34` is a value.
        $unguarded = $core->new('format');
        NativeValues::writeFormat(NumFormat::invariant(), $unguarded);
        self::assertSame(1234, self::parse($core, $unguarded, '12,34'));
        // ...while this extension's own core refuses it.
        self::assertSame(\HyperCast\CastFailure::Malformed, Cast::i32('12,34', NumFormat::invariant())->reason);

        // Guarded by the handle: refused, and nothing written.
        $target = $core->new('format');
        try {
            NativeValues::writeFormat(NumFormat::invariant(), $target, $core);
            self::fail('a format was written for a 1.0 core');
        } catch (\RuntimeException $e) {
            self::assertStringContainsString('does not declare cast_detailed', $e->getMessage());
        }
        self::assertSame(0, $target->size);
        foreach ([NumFormat::GROUPING_WESTERN, NumFormat::GROUPING_INDIAN] as $policy) {
            try {
                NativeValues::writeFormat(NumFormat::invariant()->withGrouping($policy), $target, $core);
                self::fail("policy {$policy} was written for a 1.0 core");
            } catch (\RuntimeException) {
                self::assertSame(0, $target->size);
            }
        }
    }

    /**
     * A format serialized under 1.0 means what the real 1.0 core made of it: the extension's
     * answers match the 1.0 core's, over the same inputs, for the flags that hold styles alone. A
     * stored `flags` holding anything else is refused at `unserialize` (GroupingTest).
     */
    public function testAFormatSerializedUnderOnePointZeroReadsAsTheOnePointZeroCoreReadsIt(): void
    {
        $core = self::oldCore();
        $inputs = [
            '1,234', '1234', '(1,234)', '1,234-', '0xFF', '1e3', '$5', '5%', '', 'x', '١٢',
        ];
        foreach ([95, 0] as $flags) {
            $payload = 'O:19:"HyperCast\NumFormat":10:{s:10:"decimalSep";s:1:".";s:8:"groupSep";s:1:",";s:5:"flags";i:'
                . $flags . ';s:8:"currency";s:0:"";s:18:"currencyDecimalSep";s:0:"";s:16:"currencyGroupSep";s:0:"";'
                . 's:9:"minusSign";s:0:"";s:8:"plusSign";s:0:"";s:7:"percent";s:0:"";s:9:"digitZero";s:0:"";}';
            $format = unserialize($payload);
            // What 1.0 handed the core: the layout with the integer as the flags.
            $raw = $core->new('format');
            $packed = $format->packed();
            FFI::memcpy($raw, substr($packed, 0, 4) . pack('V', $flags & 0xFFFFFFFF) . substr($packed, 8), 92);
            foreach ($inputs as $input) {
                $want = self::parseWith($core, $raw, $input);
                $got = Cast::i32($input, $format);
                $label = "flags {$flags}, " . json_encode($input);
                self::assertSame($want, $got instanceof Success ? $got->value : [$got->offset, $got->length], $label);
            }
        }
    }

    /**
     * The shapes 1.0 read and this extension refuses, the same format at each: a real 1.0 core
     * reads them, including an Indian shape with no policy declared.
     */
    public function testRefusalsAreThisCoresNotOnePointZeros(): void
    {
        $core = self::oldCore();
        $western = NumFormat::invariant();
        $raw = $core->new('format');
        FFI::memcpy($raw, $western->packed(), 92);
        foreach (['0,125' => 125, '12,34' => 1234, '1234,567' => 1234567, '1,23,456' => 123456] as $input => $was) {
            self::assertSame($was, self::parseWith($core, $raw, (string) $input), (string) $input);
            self::assertSame(\HyperCast\CastFailure::Malformed, Cast::i32((string) $input, $western)->reason);
        }
    }

    /** The extension's packed version, as `hypercast_version()` packs it. */
    private static function extensionVersion(): int
    {
        [$major, $minor, $patch] = array_map('intval', explode('.', Cast::nativeVersion()));
        return $major << 16 | $minor << 8 | $patch;
    }

    /**
     * A stand-in library with the detailed export and, unless `$version` is null, a
     * `hypercast_version()` reporting it; its FFI handle declares what it has.
     */
    private static function standIn(string $dir, ?int $version, string $name): FFI
    {
        $library = $dir . '/' . $name . (PHP_OS_FAMILY === 'Darwin' ? '.dylib' : '.so');
        $source = "int cast_detailed(void) { return -1; }\n"
            . ($version === null ? '' : sprintf("unsigned hypercast_version(void) { return 0x%06Xu; }\n", $version));
        file_put_contents($dir . '/' . $name . '.c', $source);
        exec(
            'cc -shared -fPIC -o ' . escapeshellarg($library) . ' ' . escapeshellarg($dir . '/' . $name . '.c'),
            $out,
            $status
        );
        if ($status !== 0) {
            self::markTestSkipped('no C compiler');
        }
        return FFI::cdef(
            'int cast_detailed(void);' . ($version === null ? '' : 'unsigned hypercast_version(void);'),
            $library
        );
    }

    /**
     * The binding-against-core check, on stand-in libraries that declare the detailed export and
     * report a version: compatible is the extension's major version and a minor version no older,
     * the patch ignored. It is what rejects a 1.0 core once a release bumps both.
     */
    public function testTheCoreMustShareTheExtensionsMajorAndHaveAMinorNoOlder(): void
    {
        if (!\extension_loaded('ffi') || PHP_OS_FAMILY === 'Windows') {
            self::markTestSkipped('needs ext-ffi and a C compiler');
        }
        $own = self::extensionVersion();
        [$major, $minor, $patch] = [$own >> 16, $own >> 8 & 0xFF, $own & 0xFF];
        $dir = sys_get_temp_dir() . '/hypercast-downgrade-' . getmypid();
        mkdir($dir, 0700, true);
        try {
            $layouts = FFI::cdef(self::DECLARATIONS);
            $target = $layouts->new('format');
            $cases = [
                'the same version' => [$own, true],
                'a newer patch' => [$major << 16 | $minor << 8 | $patch + 1, true],
                'a newer minor' => [$major << 16 | $minor + 1 << 8, true],
                'a newer minor and an older patch' => [$major << 16 | $minor + 1 << 8 | 0, true],
                'an older major' => [$major - 1 << 16 | 0x09 << 8, false],
                'a newer major' => [$major + 1 << 16 | $minor << 8 | $patch, false],
                'a newer major and a newer minor' => [$major + 1 << 16 | $minor + 1 << 8, false],
            ];
            if ($patch > 0) {
                $cases['an older patch'] = [$major << 16 | $minor << 8 | $patch - 1, true];
            }
            if ($minor > 0) {
                $cases['an older minor'] = [$major << 16 | $minor - 1 << 8 | 0xFF, false];
            }
            $n = 0;
            foreach ($cases as $label => [$version, $accepted]) {
                $core = self::standIn($dir, $version, 'core' . $n++);
                $target->size = 0;
                try {
                    NativeValues::writeFormat(NumFormat::invariant(), $target, $core);
                    self::assertTrue($accepted, "{$label}: a format was written");
                    self::assertSame(92, $target->size, $label);
                } catch (\RuntimeException $e) {
                    self::assertFalse($accepted, "{$label}: {$e->getMessage()}");
                    self::assertStringContainsString(
                        'which does not match this extension, ' . NativeValues::version($own),
                        $e->getMessage(),
                        $label
                    );
                    self::assertSame(0, $target->size, $label);
                }
            }
        } finally {
            array_map('unlink', glob($dir . '/*') ?: []);
            rmdir($dir);
        }
    }

    /** The rule alone, over versions an extension has not reached yet. */
    public function testTheCompatibilityRule(): void
    {
        $v = static fn (int $major, int $minor, int $patch): int => $major << 16 | $minor << 8 | $patch;
        $extension = $v(1, 1, 7);
        self::assertTrue(NativeValues::compatible($v(1, 1, 7), $extension));
        self::assertTrue(NativeValues::compatible($v(1, 1, 5), $extension), 'an older patch');
        self::assertTrue(NativeValues::compatible($v(1, 1, 255), $extension), 'a newer patch');
        self::assertTrue(NativeValues::compatible($v(1, 2, 0), $extension), 'a newer minor');
        self::assertFalse(NativeValues::compatible($v(1, 0, 9), $extension), 'an older minor');
        self::assertFalse(NativeValues::compatible($v(1, 0, 255), $extension), 'an older minor, any patch');
        self::assertFalse(NativeValues::compatible($v(2, 1, 7), $extension), 'a newer major');
        self::assertFalse(NativeValues::compatible($v(2, 9, 0), $extension), 'a newer major and minor');
        self::assertFalse(NativeValues::compatible($v(0, 9, 0), $extension), 'an older major');
        self::assertFalse(NativeValues::compatible($v(0, 255, 255), $extension), 'an older major, newer minor');
        self::assertTrue(NativeValues::compatible(self::extensionVersion()), 'this extension\'s own version');
        self::assertFalse(NativeValues::compatible($v(0, 9, 0)));
    }

    public function testAHandleWithoutHypercastVersionIsRefusedByTheGuardNotTheEngine(): void
    {
        if (!\extension_loaded('ffi') || PHP_OS_FAMILY === 'Windows') {
            self::markTestSkipped('needs ext-ffi and a C compiler');
        }
        $dir = sys_get_temp_dir() . '/hypercast-downgrade-' . getmypid() . '-v';
        mkdir($dir, 0700, true);
        try {
            $core = self::standIn($dir, null, 'unversioned');
            $layouts = FFI::cdef(self::DECLARATIONS);
            $target = $layouts->new('format');
            try {
                NativeValues::writeFormat(NumFormat::invariant(), $target, $core);
                self::fail('a format was written for a core of unknown version');
            } catch (\RuntimeException $e) {
                // The guard's own refusal, not the engine's FFI\Exception left pending.
                self::assertSame(\RuntimeException::class, $e::class);
                self::assertStringContainsString('does not declare hypercast_version', $e->getMessage());
                self::assertNull($e->getPrevious());
            }
            self::assertSame(0, $target->size);
            // Nothing is left pending: the next call runs clean.
            self::assertFalse(FFI::isNull(FFI::addr($target)));
        } finally {
            array_map('unlink', glob($dir . '/*') ?: []);
            rmdir($dir);
        }
    }

    public function testWithoutAHandleTheCallerOwnsTheCheck(): void
    {
        if (!\extension_loaded('ffi')) {
            self::markTestSkipped('ext-ffi is not loaded');
        }
        $layouts = FFI::cdef(self::DECLARATIONS);
        $target = $layouts->new('format');
        NativeValues::writeFormat(NumFormat::invariant(), $target);
        self::assertSame(NumFormat::ALL, $target->flags);
    }
}
