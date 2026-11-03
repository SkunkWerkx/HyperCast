<?php

/**
 * Regenerates stubs/hypercast.stub.php, the IDE and static-analysis stubs for the extension:
 *
 *   php tools/generate-stubs.php [--out <file>]
 *
 * from php/ on Linux or macOS, with cargo-php CARGO_PHP_VERSION installed
 * (`cargo install cargo-php --version <it> --locked`) and a C compiler on PATH. Steps:
 *
 * 1. Builds the extension's debug library, `cargo rustc --crate-type cdylib --features php
 *    --target-dir target/php` in rust/ (on Linux with `-- -C link-arg=-Wl,-z,lazy`, below):
 *    ext-php-rs exports the description cargo-php reads (`ext_php_rs_describe_module_v2`) only
 *    from a debug build. Release builds are untouched.
 * 2. Runs `cargo php stubs --stdout` on it, preloading a library that defines the PHP symbols
 *    the extension references but cargo-php doesn't stand in for (its own list covers
 *    ext-php-rs's bindings only): its data, and on x86-64 Linux its functions too, which Rust
 *    calls through the GOT, so they must resolve when the library loads, `-z lazy` or not.
 * 3. Applies REWRITES (what cargo-php's view of the builders can't show), each of which must
 *    match exactly as often as recorded, and appends VALUE_CLASSES (the classes registered by
 *    hand, which cargo-php can't see).
 *
 * tests/StubTest.php runs this into a temporary file and compares it with the committed stub,
 * and loads the committed stub without the extension to hold it to the extension's surface.
 */

declare(strict_types=1);

// A script, run directly: its helper functions sit beside the steps that use them.
// phpcs:disable PSR1.Files.SideEffects.FoundWithSymbols

const CARGO_PHP_VERSION = '0.2.1';

/**
 * PHP symbols the extension imports beyond cargo-php's stand-ins: data as null pointers,
 * functions as bodies that never run (stub generation calls none of them).
 */
const SYMBOLS_C = <<<'C'
    void *spl_ce_InvalidArgumentException, *spl_ce_LogicException, *spl_ce_RuntimeException;
    void *zend_empty_string, *zend_one_char_string[256], *zend_post_startup_cb;
    void *zend_ce_unhandled_match_error;
    int object_init_ex(void) { return -1; }
    void *zend_declare_typed_property(void) { return 0; }
    void zend_update_property(void) {}
    int zend_verify_scalar_type_hint(void) { return 0; }
    C;

/** [search, replace, occurrences]. */
const REWRITES = [
    // ext-php-rs's argument info names a union of classes in one string, which cargo-php
    // prefixes with one backslash, and spells a nullable union with `?`, which PHP refuses.
    // `optional()`, the one nullable answer, takes and gives a plain or a checked verdict.
    [
        '?\\HyperCast\\Success|HyperCast\\Checked|HyperCast\\Fault|HyperCast\\Violation {',
        '\\HyperCast\\Success|\\HyperCast\\Checked|\\HyperCast\\Fault|\\HyperCast\\Violation|null {',
        1,
    ],
    [
        '\\HyperCast\\Success|HyperCast\\Checked|HyperCast\\Fault|HyperCast\\Violation',
        '\\HyperCast\\Success|\\HyperCast\\Checked|\\HyperCast\\Fault|\\HyperCast\\Violation',
        3,
    ],
    ['\\HyperCast\\Success|HyperCast\\Fault', '\\HyperCast\\Success|\\HyperCast\\Fault', 54],
    [
        '\\HyperCast\\Checked|HyperCast\\Fault|HyperCast\\Violation',
        '\\HyperCast\\Checked|\\HyperCast\\Fault|\\HyperCast\\Violation',
        6,
    ],
    ['\\HyperCast\\NumFormat|NumberFormatter', '\\HyperCast\\NumFormat|\\NumberFormatter', 24],
    // The classes are registered final; cargo-php's stubs leave the modifier out.
    ["    class Cast {\n", "    final class Cast {\n", 1],
    ["    class NativePlatform {\n", "    final class NativePlatform {\n", 1],
    ["    class NativeValues {\n", "    final class NativeValues {\n", 1],
];

/** classes.rs registers these in MINIT as plain PHP classes, outside cargo-php's view. */
const VALUE_CLASSES = <<<'PHP'

    namespace HyperCast {
        /**
         * A door's successful verdict: the value it cast.
         */
        final readonly class Success {
            public function __construct(public mixed $value) {}
        }

        /**
         * A door's failed verdict: why, and the byte span of the input at fault.
         */
        final readonly class Fault {
            public function __construct(
                public \HyperCast\CastFailure $reason,
                public int $offset,
                public int $length,
            ) {}
        }

        /**
         * An exact decimal: the magnitude's digits, shifted right by `scale` places.
         */
        final readonly class Decimal implements \Stringable {
            public function __construct(
                public string $magnitude,
                public int $scale,
                public bool $negative,
            ) {}

            /**
             * From the C ABI's 96-bit magnitude, in two limbs.
             */
            public static function fromLimbs(int $lo, int $hi, int $scale, bool $negative): \HyperCast\Decimal {}

            public function __toString(): string {}

            /**
             * The nearest float, which may lose digits.
             */
            public function toFloat(): float {}
        }

        /**
         * A signed span of time; `nanos` carries the same sign as `seconds`.
         */
        final readonly class Duration {
            public function __construct(
                public int $seconds,
                public int $nanos,
            ) {}

            public function toSeconds(): float {}
        }

        /**
         * The day-period designators `datetime()` reads beside the English AM and PM.
         */
        final readonly class DayPeriods {
            public const MAX_BYTES = 16;
            public const PACKED_BYTES = 40;

            public function __construct(
                public string $am,
                public string $pm,
            ) {}

            /**
             * The C ABI's `hc_periods` layout.
             */
            public function packed(): string {}
        }

        /**
         * A declared number format: separators, notation flags and symbols.
         */
        final readonly class NumFormat {
            public const GROUPING = 1;
            public const PARENTHESES = 2;
            public const EXPONENT = 4;
            public const RADIX_PREFIXES = 8;
            public const PERCENT = 16;
            public const SEPARATOR_DETECT = 32;
            public const CURRENCY = 64;
            public const TRAILING_SIGN = 128;
            public const ALL = 95;
            public const SYMBOL_MAX_BYTES = 8;
            public const CURRENCY_MAX_BYTES = 16;
            public const PACKED_BYTES = 92;

            public function __construct(
                public string $decimalSep,
                public string $groupSep,
                public int $flags,
                public string $currency = '',
                public string $currencyDecimalSep = '',
                public string $currencyGroupSep = '',
                public string $minusSign = '',
                public string $plusSign = '',
                public string $percent = '',
                public string $digitZero = '',
            ) {}

            /**
             * `.` and `,` with every notation: one instance per request.
             */
            public static function invariant(): \HyperCast\NumFormat {}

            /**
             * The invariant format that also detects a comma decimal: one instance per request.
             */
            public static function detect(): \HyperCast\NumFormat {}

            /**
             * From a `localeconv()` array, the current locale's when `$conv` is null.
             */
            public static function fromLocaleconv(?array $conv = null): \HyperCast\NumFormat {}

            /**
             * From an ext-intl formatter's symbols and negative pattern.
             */
            public static function fromNumberFormatter(\NumberFormatter $formatter): \HyperCast\NumFormat {}

            /**
             * The C ABI's `hc_format` layout.
             */
            public function packed(): string {}
        }

        /**
         * Validation constraints, the OpenAPI Schema Object's keywords, bound to one door and
         * judged by the core when built: a declaration that cannot mean anything at its door is
         * an `\InvalidArgumentException` whose code is the core's refusal code. Bounds and
         * `multipleOf` are an int, a string in invariant notation or a `Decimal`, never a float
         * (in weak mode too), and are kept as the exact `Decimal` the core read.
         */
        final readonly class Constraints {
            public const PACKED_BYTES = 72;

            public \HyperCast\Door $door;
            public ?\HyperCast\Decimal $minimum;
            public ?\HyperCast\Decimal $exclusiveMinimum;
            public ?\HyperCast\Decimal $maximum;
            public ?\HyperCast\Decimal $exclusiveMaximum;
            public ?\HyperCast\Decimal $multipleOf;
            public \HyperCast\Rounding $rounding;
            public ?int $minLength;
            public ?int $maxLength;
            public bool $trim;
            public ?array $enum;

            public function __construct(
                \HyperCast\Door $door,
                int|string|\HyperCast\Decimal|null $minimum = null,
                int|string|\HyperCast\Decimal|null $exclusiveMinimum = null,
                int|string|\HyperCast\Decimal|null $maximum = null,
                int|string|\HyperCast\Decimal|null $exclusiveMaximum = null,
                int|string|\HyperCast\Decimal|null $multipleOf = null,
                \HyperCast\Rounding $rounding = \HyperCast\Rounding::Strict,
                ?int $minLength = null,
                ?int $maxLength = null,
                bool $trim = false,
                ?array $enum = null,
            ) {}

            /**
             * The C ABI's `RawConstraints` layout, native-endian.
             */
            public function packed(): string {}

            /**
             * The `enum` list as the C ABI packs it beside `packed()`, empty for none.
             */
            public function packedEnum(): string {}
        }

        /**
         * A checked door's success: the value (rounded to `multipleOf` when the declaration
         * rounds; with a text `enum`, the caller's own declared entry), whether rounding changed
         * it, and the index of the text `enum` entry it matched.
         */
        final readonly class Checked {
            public function __construct(
                public mixed $value,
                public bool $adjusted = false,
                public ?int $enumIndex = null,
            ) {}
        }

        /**
         * A value that parsed and broke a declared rule: the keyword (the bare code for one this
         * binding does not name yet) and the byte span of the input.
         */
        final readonly class Violation {
            public function __construct(
                public \HyperCast\Keyword|int $keyword,
                public int $offset,
                public int $length,
            ) {}

            /**
             * The keyword's OpenAPI spelling (`exclusiveMinimum`), or `keyword N`.
             */
            public function keywordName(): string {}
        }
    }

    PHP;

const HEADER = <<<'PHP'
    <?php

    /**
     * Stubs for the hypercast extension, for IDEs and static analysis. Generated: don't edit.
     * Regenerate from php/ with `php tools/generate-stubs.php`, which runs
     * `cargo php stubs --stdout` (cargo-php %s) on the debug build of the extension and
     * completes its output (the tool says how).
     */

    PHP;

function fail(string $message): never
{
    fwrite(STDERR, $message . PHP_EOL);
    exit(1);
}

/** Runs `$command` in `$cwd` with `$env` added; returns stdout, or fails with stderr. */
function run(array $command, string $cwd, array $env = []): string
{
    $process = proc_open(
        $command,
        [1 => ['pipe', 'w'], 2 => ['pipe', 'w']],
        $pipes,
        $cwd,
        $env === [] ? null : [...getenv(), ...$env]
    );
    if (!is_resource($process)) {
        fail('could not start ' . $command[0]);
    }
    $stdout = (string) stream_get_contents($pipes[1]);
    $stderr = (string) stream_get_contents($pipes[2]);
    fclose($pipes[1]);
    fclose($pipes[2]);
    if (proc_close($process) !== 0) {
        fail(implode(' ', $command) . " failed:\n" . $stderr . $stdout);
    }
    return $stdout;
}

if (PHP_OS_FAMILY === 'Windows') {
    fail('cargo-php generates stubs on Linux and macOS only');
}

$root = dirname(__DIR__);
$rust = dirname($root) . '/rust';
$out = $root . '/stubs/hypercast.stub.php';
if (($argv[1] ?? null) === '--out' && isset($argv[2])) {
    $out = $argv[2];
} elseif (count($argv) > 1) {
    fail('usage: php tools/generate-stubs.php [--out <file>]');
}

$version = trim(run(['cargo', 'php', '--version'], $rust));
if (!str_starts_with($version, 'cargo-php ' . CARGO_PHP_VERSION . ' ')) {
    fail("cargo-php " . CARGO_PHP_VERSION . " is required (its output is the stub's format), found: {$version}");
}

$darwin = PHP_OS_FAMILY === 'Darwin';
$build = ['cargo', 'rustc', '--quiet', '--crate-type', 'cdylib', '--features', 'php', '--target-dir', 'target/php'];
// Rust links with `-z now` on Linux, which binds every function when the library loads, and
// cargo-php stands in only for ext-php-rs's bindings; `-z lazy` defers the rest, as on macOS.
// The flag reaches only this crate's link, so the cached dependencies are reused.
$linux = $darwin ? [] : ['--', '-C', 'link-arg=-Wl,-z,lazy'];
run([...$build, ...$linux], $rust);
$library = $rust . '/target/php/debug/' . ($darwin ? 'libhypercast.dylib' : 'libhypercast.so');

$scratch = sys_get_temp_dir() . '/hypercast-stubs-' . getmypid();
if (!is_dir($scratch) && !mkdir($scratch, 0700, true)) {
    fail("cannot create {$scratch}");
}
// `fail()` exits, which skips `finally`; a shutdown function runs on every way out.
register_shutdown_function(static function () use ($scratch): void {
    foreach (glob($scratch . '/*') ?: [] as $file) {
        unlink($file);
    }
    if (is_dir($scratch)) {
        rmdir($scratch);
    }
});
$symbols = $scratch . '/symbols' . ($darwin ? '.dylib' : '.so');
file_put_contents($scratch . '/symbols.c', SYMBOLS_C . "\n");
run(['cc', '-shared', '-fPIC', '-o', $symbols, $scratch . '/symbols.c'], $scratch);
$preload = $darwin ? ['DYLD_INSERT_LIBRARIES' => $symbols] : ['LD_PRELOAD' => $symbols];
$stubs = run(['cargo', 'php', 'stubs', '--stdout', $library], $rust, $preload);

$prefix = "<?php\n\n// Stubs for hypercast\n\n";
if (!str_starts_with($stubs, $prefix)) {
    fail("cargo-php's output no longer starts with the expected lines:\n" . substr($stubs, 0, 200));
}
$stubs = substr($stubs, strlen($prefix));
foreach (REWRITES as [$search, $replace, $expected]) {
    $count = substr_count($stubs, $search);
    if ($count !== $expected) {
        fail("rewrite of '{$search}' expects {$expected} matches, found {$count}");
    }
    $stubs = str_replace($search, $replace, $stubs);
}
$stubs = sprintf(HEADER, CARGO_PHP_VERSION) . "\n" . rtrim($stubs, "\n") . "\n" . VALUE_CLASSES;
if (file_put_contents($out, $stubs) === false) {
    fail("cannot write {$out}");
}
