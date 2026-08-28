<?php

declare(strict_types=1);

namespace HyperCast;

/**
 * Maps the running PHP_OS_FAMILY/machine to the RID-style directory (matching the other
 * bindings' runtimes/{rid}/native/ / native/{rid}/ convention) and native library filename.
 */
final class NativePlatform
{
    /** @return array{string, string} [rid, libraryFileName] */
    public static function ridAndLibraryName(): array
    {
        $machine = strtolower(php_uname('m'));
        $isArm = str_contains($machine, 'arm') || str_contains($machine, 'aarch64');

        return match (PHP_OS_FAMILY) {
            'Windows' => [$isArm ? 'win-arm64' : 'win-x64', 'hypercast.dll'],
            'Darwin' => [$isArm ? 'osx-arm64' : 'osx-x64', 'libhypercast.dylib'],
            'Linux' => [$isArm ? 'linux-arm64' : 'linux-x64', 'libhypercast.so'],
            default => throw new \RuntimeException(
                'hypercast: unsupported platform PHP_OS_FAMILY=' . PHP_OS_FAMILY
            ),
        };
    }

    private function __construct()
    {
    }
}
