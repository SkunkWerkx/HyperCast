<?php

/**
 * Records 1.0's PHP behaviour (the ext-ffi userland package) as fixtures, for the parity
 * tests of the extension. 1.0's classes are loaded from `<v1-src>`, a copy of php/src from the
 * v1.0.0 tag, extracted without touching the checkout:
 *
 *   git archive v1.0.0 php/src | tar -x -C <scratch>      # <v1-src> is <scratch>/php/src
 *
 * and 1.0 loads the core named by HYPERCAST_NATIVE_LIBRARY, the library committed beside them
 * for this platform (<v1-src>/native/<rid>/). Two modes, run from php/ after `composer install`:
 *
 *   HYPERCAST_NATIVE_LIBRARY=<lib> php tools/capture-v1-fixtures.php <v1-src> [outdir]
 *
 * writes provenance.json, surface.json, doors.json, formats.json, formatters.json (ext-intl),
 * interop.json and objects.json; and
 *
 *   HYPERCAST_NATIVE_LIBRARY=<lib> php tools/capture-v1-fixtures.php --coercion <v1-src> [outdir]
 *
 * adds the running PHP minor's recording to coercion.json (tests/Support/CoercionMatrix.php
 * says what it holds; the engine's own messages differ between minors, so each minor has its
 * own). The commit is HYPERCAST_V1_COMMIT, or the v1.0.0 tag's commit in this checkout; it
 * must be the commit archived.
 *
 * Fixtures are add-only: the first mode refuses to overwrite a fixture, and the second refuses
 * a minor coercion.json already holds. They record 1.0 and are never edited; where the
 * extension answers differently by design, tests/Support/V1Differences.php says so.
 */

declare(strict_types=1);

// A script, run directly: its helper closures sit beside the steps that use them.
// phpcs:disable PSR1.Files.SideEffects.FoundWithSymbols

use HyperCast\Cast;
use HyperCast\Tests\Support\CoercionMatrix;
use HyperCast\Tests\Support\V1Probe;

$fail = static function (string $message): never {
    fwrite(STDERR, $message . PHP_EOL);
    exit(1);
};

/** JSON, pretty to `$inline` levels deep and one line per value below that. */
$encode = static function (mixed $data, int $inline, int $depth = 0) use (&$encode): string {
    $flags = JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR | JSON_PRESERVE_ZERO_FRACTION;
    if ($depth >= $inline || !\is_array($data) || $data === []) {
        return json_encode($data, $flags);
    }
    $pad = str_repeat('    ', $depth + 1);
    $list = array_is_list($data);
    $lines = [];
    foreach ($data as $key => $value) {
        $name = $list ? '' : json_encode((string) $key, $flags) . ': ';
        $lines[] = $pad . $name . $encode($value, $inline, $depth + 1);
    }
    [$open, $close] = $list ? ['[', ']'] : ['{', '}'];
    return $open . "\n" . implode(",\n", $lines) . "\n" . str_repeat('    ', $depth) . $close;
};

$args = array_slice($argv, 1);
$coercion = ($args[0] ?? null) === '--coercion';
if ($coercion) {
    array_shift($args);
}
$v1Src = realpath($args[0] ?? '');
if ($v1Src === false || !is_file($v1Src . '/Cast.php')) {
    $fail('usage: php tools/capture-v1-fixtures.php [--coercion] <v1-src> [outdir], <v1-src> a 1.0 php/src tree');
}
$library = getenv('HYPERCAST_NATIVE_LIBRARY');
if (!\is_string($library) || $library === '' || !is_file($library)) {
    $fail('HYPERCAST_NATIVE_LIBRARY must name the 1.0 library to load');
}
$library = (string) realpath($library);
if (extension_loaded('hypercast')) {
    $fail('the hypercast extension is loaded; 1.0 is recorded without it');
}

$root = dirname(__DIR__);
$repo = dirname($root);
// 1.0's classes, ahead of anything else: HyperCast\ from <v1-src>, HyperCast\Interop\ beneath it.
spl_autoload_register(static function (string $class) use ($v1Src): void {
    if (preg_match('/^HyperCast\\\\((?:Interop\\\\)?\w+)$/', $class, $m) === 1) {
        $file = $v1Src . '/' . str_replace('\\', '/', $m[1]) . '.php';
        if (is_file($file)) {
            require $file;
        }
    }
}, true, true);
require $root . '/vendor/autoload.php';

$outDir = $args[1] ?? $root . '/tests/fixtures/v1';
if (!is_dir($outDir) && !mkdir($outDir, 0777, true)) {
    $fail("cannot create {$outDir}");
}

$commit = getenv('HYPERCAST_V1_COMMIT');
if ($commit === false || $commit === '') {
    $commit = trim((string) shell_exec('git -C ' . escapeshellarg($repo) . ' rev-parse v1.0.0^{commit} 2>/dev/null'));
}
if (preg_match('/^[0-9a-f]{40}$/', $commit) !== 1) {
    $fail('cannot tell which commit <v1-src> came from; set HYPERCAST_V1_COMMIT');
}
// Where the library came from, relative to the archived tree, so no machine's path is recorded.
$relative = static function (string $path) use ($v1Src): string {
    $tree = dirname($v1Src, 2);
    return str_starts_with($path, $tree . '/') ? substr($path, \strlen($tree) + 1) : basename($path);
};
$provenance = [
    'tool' => 'php/tools/capture-v1-fixtures.php' . ($coercion ? ' --coercion' : ''),
    'classes' => 'php/src at source_sha (git archive), autoloaded',
    'library' => $relative($library),
    'library_sha256' => hash_file('sha256', $library),
    'native_version' => Cast::nativeVersion(),
    'source_sha' => $commit,
    'php_version' => PHP_VERSION,
    'os' => PHP_OS_FAMILY,
    'intl' => extension_loaded('intl') ? INTL_ICU_VERSION : null,
];

if ($coercion) {
    $target = $outDir . '/coercion.json';
    $minor = PHP_MAJOR_VERSION . '.' . PHP_MINOR_VERSION;
    $fixture = is_file($target)
        ? json_decode((string) file_get_contents($target), true, flags: JSON_THROW_ON_ERROR)
        : ['captures' => []];
    if (isset($fixture['captures'][$minor])) {
        $fail("{$target} already holds PHP {$minor}; fixtures are add-only");
    }
    $scratch = sys_get_temp_dir() . '/hypercast-coercion-' . getmypid();
    if (!is_dir($scratch) && !mkdir($scratch, 0700, true)) {
        $fail("cannot create {$scratch}");
    }
    $matrix = CoercionMatrix::run($scratch);
    array_map('unlink', glob($scratch . '/*') ?: []);
    rmdir($scratch);
    $fixture['captures'][$minor] = ['provenance' => $provenance, 'matrix' => $matrix];
    ksort($fixture['captures']);
    file_put_contents($target, $encode($fixture, 6) . "\n");
    fwrite(STDOUT, "PHP {$minor} added to {$target}\n");
    exit(0);
}

if (!extension_loaded('intl')) {
    $fail('the first mode records formatters.json, which needs ext-intl');
}
$files = [
    'provenance.json' => [$provenance, 1],
    'surface.json' => [V1Probe::surface(), 6],
    'doors.json' => [V1Probe::doors(), 2],
    'formats.json' => [V1Probe::formats(), 2],
    'formatters.json' => [V1Probe::formatters(), 2],
    'interop.json' => [V1Probe::interop(), 2],
    'objects.json' => [V1Probe::objects(), 2],
];
foreach (array_keys($files) as $name) {
    if (is_file($outDir . '/' . $name)) {
        $fail("{$outDir}/{$name} exists; fixtures are add-only");
    }
}
foreach ($files as $name => [$data, $inline]) {
    file_put_contents($outDir . '/' . $name, $encode($data, $inline) . "\n");
}
fwrite(STDOUT, 'fixtures written to ' . $outDir . PHP_EOL);
