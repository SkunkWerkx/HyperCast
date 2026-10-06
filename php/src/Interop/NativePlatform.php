<?php

declare(strict_types=1);

namespace HyperCast\Interop;

/**
 * Maps the running process to the RID-style directory (matching the other bindings'
 * runtimes/{rid}/native/ / native/{rid}/ convention) and file name of a native library, and
 * finds the library to load. Every SkunkWerkx core ships the same way — a package carrying
 * each platform's build under native/{rid}/ — so this is public for any of them: HyperCast
 * loads `hypercast` with it, HyperTabular `hypertabular`.
 */
final class NativePlatform
{
    /** Non-instantiable — static resolution only. */
    private function __construct()
    {
    }

    /**
     * This process's directory and file name for a library.
     *
     * @param string $baseName the library's base name — `hypercast` for libhypercast.so and hypercast.dll
     * @return array{0: string, 1: string} [$rid, $libraryFileName]
     */
    public static function ridAndLibraryName(string $baseName): array
    {
        return self::resolve(
            $baseName,
            PHP_OS_FAMILY,
            php_uname('m'),
            PHP_INT_SIZE,
            PHP_OS_FAMILY === 'Linux' && self::isMusl()
        );
    }

    /**
     * The mapping itself, as a pure function of what the process reports, so the whole table
     * is testable from any one platform.
     *
     * Windows is `win-x64` whatever the hardware: PHP has never shipped a native Windows
     * ARM64 build, so on ARM hardware it is an x64 process under emulation — while
     * php_uname('m') reports the *machine* ("ARM64"), which is not what a DLL has to match.
     * Everywhere else the machine string is matched exactly (case-insensitively), so an
     * architecture no library is built for is a clear error here rather than a
     * wrong-architecture dlopen failure later.
     *
     * @param string $baseName the library's base name
     * @param string $osFamily PHP_OS_FAMILY
     * @param string $machine php_uname('m')
     * @param int $intSize PHP_INT_SIZE — 8 for the 64-bit process every bundled library needs
     * @param bool $musl whether the process runs on musl libc (Linux only)
     * @return array{0: string, 1: string} [$rid, $libraryFileName]
     */
    public static function resolve(string $baseName, string $osFamily, string $machine, int $intSize, bool $musl): array
    {
        if ($intSize !== 8) {
            throw new \RuntimeException("{$baseName}: unsupported platform — a 64-bit PHP is required");
        }
        if ($osFamily === 'Windows') {
            return ['win-x64', "{$baseName}.dll"];
        }

        $arch = match (strtolower($machine)) {
            'x86_64', 'amd64' => 'x64',
            'aarch64', 'arm64' => 'arm64',
            default => throw new \RuntimeException(
                "{$baseName}: unsupported platform — no native library for architecture '{$machine}'"
            ),
        };

        return match ($osFamily) {
            'Darwin' => ["osx-{$arch}", "lib{$baseName}.dylib"],
            'Linux' => [($musl ? 'linux-musl-' : 'linux-') . $arch, "lib{$baseName}.so"],
            default => throw new \RuntimeException(
                "{$baseName}: unsupported platform PHP_OS_FAMILY={$osFamily}"
            ),
        };
    }

    /**
     * The library to load: the build staged for this platform under $sourceDir/native/{rid}/;
     * or, for the development loop, the file the environment variable $override names (a
     * core built from a checkout, loaded without replacing committed files); or, when nothing
     * is staged, the in-repo cargo build beside $sourceDir — exactly what the other
     * bindings' local staging does.
     *
     * @param string $baseName the library's base name
     * @param string $sourceDir the package's php/src directory
     * @param string $override the environment variable naming a library to load instead
     * @return string the path to load
     * @throws \RuntimeException no library resolves (an unsupported platform, or a package built without one)
     */
    public static function libraryPath(string $baseName, string $sourceDir, string $override): string
    {
        [$rid, $libName] = self::ridAndLibraryName($baseName);
        $path = "{$sourceDir}/native/{$rid}/{$libName}";
        $named = getenv($override);
        if (\is_string($named) && $named !== '') {
            $path = $named;
        } elseif (!is_file($path)) {
            $repoBuild = \dirname($sourceDir, 2) . "/rust/target/release/{$libName}";
            if (is_file($repoBuild)) {
                $path = $repoBuild;
            }
        }
        if (!is_file($path)) {
            throw new \RuntimeException(
                "{$baseName}: {$path} not found (unsupported platform, or this package was built "
                . 'without a native library for it)'
            );
        }
        return $path;
    }

    /**
     * Whether this process runs on musl libc (Alpine) rather than glibc: true when the
     * process has a musl loader mapped. The same rule every binding uses; an unreadable
     * /proc/self/maps means glibc.
     *
     * @return bool true on musl
     */
    private static function isMusl(): bool
    {
        $maps = @file_get_contents('/proc/self/maps');
        return $maps !== false && (str_contains($maps, 'ld-musl-') || str_contains($maps, 'libc.musl-'));
    }
}
