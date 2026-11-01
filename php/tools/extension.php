<?php

declare(strict_types=1);

// The extension `composer test` and `composer bench` load: the local cargo build (`cargo
// php-ext` in rust/) unless HYPERCAST_PHP_EXTENSION names another, which CI does. Resolved to
// an absolute path and exported back into HYPERCAST_PHP_EXTENSION, so child processes load
// the same file. Exits with a message when it is not built.

function hypercast_extension(string $root): string
{
    $path = getenv('HYPERCAST_PHP_EXTENSION');
    if (!\is_string($path) || $path === '') {
        $library = match (PHP_OS_FAMILY) {
            'Darwin' => 'libhypercast.dylib',
            'Windows' => 'hypercast.dll',
            default => 'libhypercast.so',
        };
        $path = $root . '/../rust/target/php/release/' . $library;
    }
    $resolved = realpath($path);
    if ($resolved === false || !is_file($resolved)) {
        fwrite(
            STDERR,
            "the hypercast extension is not built: {$path} not found (run cargo php-ext in rust/)" . PHP_EOL
        );
        exit(1);
    }
    putenv('HYPERCAST_PHP_EXTENSION=' . $resolved);
    return $resolved;
}

/** Runs $command from $root with this process's standard streams, and exits with its status. */
function hypercast_run(array $command, string $root, string $what): never
{
    $process = proc_open($command, [0 => STDIN, 1 => STDOUT, 2 => STDERR], $pipes, $root);
    if (!\is_resource($process)) {
        fwrite(STDERR, "could not start {$what}" . PHP_EOL);
        exit(1);
    }
    exit(proc_close($process));
}
