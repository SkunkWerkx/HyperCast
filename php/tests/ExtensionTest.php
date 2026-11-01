<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use HyperCast\Cast;
use HyperCast\NumFormat;
use HyperCast\Tests\Support\ChildProcess;
use PHPUnit\Framework\TestCase;

/**
 * The extension as PHP sees it: its name and version, its per-request shared formats, and
 * that it needs neither ext-ffi nor ext-intl.
 */
final class ExtensionTest extends TestCase
{
    use ChildProcess;

    public function testTheExtensionReportsTheCratesVersion(): void
    {
        self::assertTrue(\extension_loaded('hypercast'));
        $manifest = (string) file_get_contents(\dirname(__DIR__, 2) . '/rust/Cargo.toml');
        self::assertSame(1, preg_match('/^version\s*=\s*"([^"]+)"/m', $manifest, $match));
        self::assertSame($match[1], phpversion('hypercast'));
        self::assertSame($match[1], Cast::nativeVersion());
    }

    public function testTheSharedFormatsAreOneInstanceEachPerRequest(): void
    {
        // As 1.0's function-static caches held them.
        self::assertSame(NumFormat::invariant(), NumFormat::invariant());
        self::assertSame(NumFormat::detect(), NumFormat::detect());
        self::assertNotSame(NumFormat::invariant(), NumFormat::detect());
    }

    public function testItLoadsAndCastsWithNoOtherExtension(): void
    {
        // -n drops every ini file, and with them a shared ext-ffi and ext-intl (a PHP that
        // builds them in keeps them, so there it shows only that -n loads nothing else).
        $out = self::child(
            ['-n', '-d', 'extension=' . self::ext()],
            'echo HyperCast\Cast::f64("1.234,5", new HyperCast\NumFormat(",", ".", HyperCast\NumFormat::ALL))->value;'
        );
        self::assertSame('1234.5', $out);
    }
}
