<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use HyperCast\Tests\Support\ChildProcess;
use HyperCast\Tests\Support\V1Probe;
use PHPUnit\Framework\TestCase;

/**
 * stubs/hypercast.stub.php, the IDE and static-analysis stubs, declares the extension's API
 * exactly, and is what tools/generate-stubs.php makes from the current source.
 */
final class StubTest extends TestCase
{
    use ChildProcess;

    /**
     * Runs everywhere, without cargo-php: the committed stub, loaded alone in a PHP without the
     * extension, has the loaded extension's surface (V1Probe::surface(), the record 1.0 was held
     * to, over 1.0's classes, the validation constraints' and 1.1's detail and date options'),
     * declaration for declaration.
     * This is what holds the value classes, which the tool writes by hand, to the extension.
     */
    public function testTheStubDeclaresTheLoadedSurface(): void
    {
        $root = \dirname(__DIR__);
        $classes = 'array_merge(HyperCast\Tests\Support\V1Probe::CLASSES, '
            . 'HyperCast\Tests\Support\V1Probe::CONSTRAINT_CLASSES, '
            . 'HyperCast\Tests\Support\V1Probe::DETAIL_CLASSES)';
        $stubbed = self::child(
            ['-n'],
            'require ' . var_export($root . '/vendor/autoload.php', true) . ';'
            . 'require ' . var_export($root . '/stubs/hypercast.stub.php', true) . ';'
            . "echo json_encode(HyperCast\\Tests\\Support\\V1Probe::surface({$classes}), JSON_THROW_ON_ERROR);"
        );
        $loaded = json_encode(
            V1Probe::surface([...V1Probe::CLASSES, ...V1Probe::CONSTRAINT_CLASSES, ...V1Probe::DETAIL_CLASSES]),
            JSON_THROW_ON_ERROR
        );
        self::assertSame(
            json_decode($loaded, true, flags: JSON_THROW_ON_ERROR),
            json_decode($stubbed, true, flags: JSON_THROW_ON_ERROR)
        );
    }

    public function testTheStubDeclaresEveryHyperCastClassAndNoOther(): void
    {
        $stub = (string) file_get_contents(\dirname(__DIR__) . '/stubs/hypercast.stub.php');
        preg_match_all('/^\s*namespace\s+([\w\\\\]+)\s*\{(.*?)^}/ms', $stub, $blocks, PREG_SET_ORDER);
        $declared = [];
        foreach ($blocks as [, $namespace, $body]) {
            preg_match_all('/^\s*(?:final\s+)?(?:readonly\s+)?(?:class|enum|interface)\s+(\w+)/m', $body, $names);
            foreach ($names[1] as $name) {
                $declared[] = $namespace . '\\' . $name;
            }
        }
        sort($declared);
        $loaded = array_values(array_filter(
            [...get_declared_classes(), ...get_declared_interfaces()],
            static fn (string $name): bool => str_starts_with($name, 'HyperCast\\')
                && (new \ReflectionClass($name))->isInternal(),
        ));
        sort($loaded);
        self::assertSame($loaded, $declared);
    }

    public function testStubIsCurrent(): void
    {
        if (PHP_OS_FAMILY === 'Windows') {
            self::markTestSkipped('cargo-php generates stubs on Linux and macOS only');
        }
        if (getenv('HYPERCAST_CHECK_STUBS') !== '1') {
            self::markTestSkipped('set HYPERCAST_CHECK_STUBS=1 to regenerate and compare the stubs');
        }
        $root = \dirname(__DIR__);
        $generated = tempnam(sys_get_temp_dir(), 'hypercast-stub');
        self::assertIsString($generated);
        try {
            $process = proc_open(
                [PHP_BINARY, $root . '/tools/generate-stubs.php', '--out', $generated],
                [1 => ['pipe', 'w'], 2 => ['pipe', 'w']],
                $pipes,
                $root
            );
            self::assertIsResource($process);
            $output = stream_get_contents($pipes[1]) . stream_get_contents($pipes[2]);
            fclose($pipes[1]);
            fclose($pipes[2]);
            self::assertSame(0, proc_close($process), "generate-stubs.php failed:\n{$output}");
            self::assertFileEquals($generated, $root . '/stubs/hypercast.stub.php');
        } finally {
            unlink($generated);
        }
    }
}
