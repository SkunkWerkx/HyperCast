<?php

declare(strict_types=1);

/*
 * The suite's bootstrap: the autoloader, then the core freshness check. Without
 * HYPERCAST_NATIVE_LIBRARY the suite loads the libraries committed under src/native/, which
 * stage-native-binaries.yml refreshes only at release, so after a core change a plain
 * phpunit run would test the core as it was at the last staging. The check refuses to run
 * the suite when this checkout's rust/ differs from the commit those libraries were built
 * from. CI builds its own core and copies it over src/native/ before the suite, and sets CI,
 * so the check is skipped there; so it is wherever it cannot tell (no git, no staging
 * commit, a shallow clone without its source commit), and when src/native/ itself has
 * working-tree changes, since then someone wrote fresh libraries into it.
 */

require __DIR__ . '/../vendor/autoload.php';

// Runs git in $dir and returns its trimmed standard output, or null when it did not exit 0;
// $status is its exit code (-1 when git did not run). Closures, not functions: a file that
// runs the check may not also declare symbols (PSR-1).
$git = static function (string $dir, array $args, ?int &$status = null): ?string {
    $null = PHP_OS_FAMILY === 'Windows' ? 'NUL' : '/dev/null';
    $process = @proc_open(
        array_merge(['git'], $args),
        [0 => ['file', $null, 'r'], 1 => ['pipe', 'w'], 2 => ['pipe', 'w']],
        $pipes,
        $dir,
    );
    if (!is_resource($process)) {
        $status = -1;
        return null;
    }
    $out = stream_get_contents($pipes[1]);
    stream_get_contents($pipes[2]);
    fclose($pipes[1]);
    fclose($pipes[2]);
    $status = proc_close($process);
    return $status === 0 ? trim((string) $out) : null;
};

// Why the suite would run against a stale core, or null when the core is fresh or the check
// cannot tell.
$staleCoreMessage = static function () use ($git): ?string {
    $tree = 'php/src/native';
    if ((string) getenv('CI') !== '' || (string) getenv('HYPERCAST_NATIVE_LIBRARY') !== '') {
        return null;
    }
    if (!function_exists('proc_open')) {
        return null;
    }
    $top = $git(__DIR__, ['rev-parse', '--show-toplevel']);
    if ($top === null || $top === '') {
        return null;
    }
    $staging = $git($top, ['log', '-1', '--format=%H', '--grep=^chore: stage native binaries', 'HEAD']);
    if ($staging === null || $staging === '') {
        return null;
    }
    $trailer = $git($top, ['log', '-1', '--format=%(trailers:key=Release-Source-SHA,valueonly)', $staging]);
    $source = trim(explode("\n", (string) $trailer)[0]);
    if ($source === '') {
        $subject = (string) $git($top, ['log', '-1', '--format=%s', $staging]);
        if (preg_match('/\(([0-9a-f]{40})\)\s*$/', $subject, $match) !== 1) {
            return null;
        }
        $source = $match[1];
    }
    if ($git($top, ['cat-file', '-e', $source . '^{commit}']) === null) {
        return null;
    }
    $git($top, ['diff', '--quiet', 'HEAD', '--', $tree, ':(exclude,glob)' . $tree . '/**/README.md'], $status);
    if ($status !== 0) {
        return null;
    }
    $git($top, ['diff', '--quiet', $source, '--', 'rust/src', 'rust/Cargo.toml', 'rust/Cargo.lock'], $status);
    $stale = $status === 1;
    if (!$stale) {
        $untracked = $git($top, ['ls-files', '--others', '--exclude-standard', '--', 'rust/src']);
        $stale = $untracked !== null && $untracked !== '';
    }
    if (!$stale) {
        return null;
    }
    $library = match (PHP_OS_FAMILY) {
        'Darwin' => 'libhypercast.dylib',
        'Windows' => 'hypercast.dll',
        default => 'libhypercast.so',
    };
    return $tree . ' was built from rust/ at ' . substr($source, 0, 12)
        . ", and this checkout's rust/ differs from it, so these tests would run against a stale core."
        . " Build the checkout's own with .github/scripts/local-core.sh (FORGE=<path> if the"
        . ' SkunkWerkx/.github checkout is not ../.github or ../HyperForge), then run'
        . ' HYPERCAST_NATIVE_LIBRARY=' . $top . '/rust/target/release/' . $library . ' vendor/bin/phpunit instead;'
        . ' CI builds its own core and skips this check.';
};

$stale = $staleCoreMessage();
if ($stale !== null) {
    fwrite(STDERR, $stale . PHP_EOL);
    exit(1);
}
