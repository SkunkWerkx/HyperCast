<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use HyperCast\Tests\Support\V1Probe;
use PHPUnit\Framework\TestCase;

/**
 * The extension's public surface against 1.0's, recorded by tools/capture-v1-fixtures.php
 * into fixtures/v1/surface.json with the same code (V1Probe::surface()): every class, enum,
 * constant, case, method, parameter, default and property, after the edits `expected()`
 * declares: the one removal php/README.md lists under "Differences from 1.0", and the checked
 * doors the validation constraints add to `Cast` (every 1.0 member is unchanged).
 */
final class SurfaceTest extends TestCase
{
    public function testSurfaceMatchesV1(): void
    {
        $expected = self::expected();
        $actual = self::normalise(V1Probe::surface());
        $differences = self::differences($expected, $actual);
        self::assertSame([], $differences, "the surface differs from 1.0's:\n" . implode("\n", $differences));
        self::assertSame($expected, $actual);
    }

    /**
     * 1.0's static classes had private constructors, which surface.json (public members only)
     * records only as not instantiable.
     */
    public function testStaticClassesAreNotInstantiable(): void
    {
        foreach (['Cast', 'Interop\\NativeValues', 'Interop\\NativePlatform'] as $short) {
            $class = 'HyperCast\\' . $short;
            self::assertTrue((new \ReflectionMethod($class, '__construct'))->isPrivate(), $class);
            try {
                new $class();
                self::fail("{$class} was instantiated");
            } catch (\Error $e) {
                self::assertSame(\Error::class, $e::class);
                self::assertSame("Call to private {$class}::__construct() from scope " . self::class, $e->getMessage());
            }
        }
    }

    public function testEveryClassIsTheExtensions(): void
    {
        foreach ([...V1Probe::CLASSES, ...V1Probe::CONSTRAINT_CLASSES] as $short) {
            $class = new \ReflectionClass('HyperCast\\' . $short);
            self::assertTrue($class->isInternal(), $short);
            self::assertSame('hypercast', $class->getExtensionName(), $short);
        }
    }

    /** surface.json with the declared edits applied: these and nothing else. */
    private static function expected(): array
    {
        $path = __DIR__ . '/fixtures/v1/surface.json';
        $surface = json_decode((string) file_get_contents($path), true, 512, JSON_THROW_ON_ERROR);
        // Cast::isAvailable() is removed: the extension is either loaded or the classes don't
        // exist, so there is nothing left for it to report.
        self::assertArrayHasKey('isAvailable', $surface['Cast']['methods']);
        unset($surface['Cast']['methods']['isAvailable']);
        // The validation constraints' doors: plain text, and the three checked doors.
        $param = static fn (string $name, string $type): array =>
            ['name' => $name, 'type' => $type, 'optional' => false, 'default' => null, 'byRef' => false];
        $checked = 'HyperCast\\Checked|HyperCast\\Fault|HyperCast\\Violation';
        $constraints = $param('constraints', 'HyperCast\\Constraints');
        $surface['Cast']['methods'] += [
            'text' => [
                'static' => true,
                'params' => [$param('text', 'string')],
                'return' => 'HyperCast\\Success|HyperCast\\Fault',
            ],
            'checked' => [
                'static' => true,
                'params' => [
                    $param('text', 'string'),
                    $param('format', 'HyperCast\\NumFormat|NumberFormatter'),
                    $constraints,
                ],
                'return' => $checked,
            ],
            'checkedFromFloat' => [
                'static' => true,
                'params' => [$param('value', 'float'), $constraints],
                'return' => $checked,
            ],
            'checkedText' => [
                'static' => true,
                'params' => [$param('text', 'string'), $constraints],
                'return' => $checked,
            ],
        ];
        // optional() takes a checked verdict too: its parameter and answer widen, additively.
        $any = 'HyperCast\\Success|HyperCast\\Checked|HyperCast\\Fault|HyperCast\\Violation';
        $optional = &$surface['Cast']['methods']['optional'];
        self::assertSame('HyperCast\\Success|HyperCast\\Fault', $optional['params'][0]['type']);
        $optional['params'][0]['type'] = $any;
        $optional['return'] = "{$any}|null";
        unset($optional);
        ksort($surface['Cast']['methods']);
        return self::normalise($surface);
    }

    /** Both sides through the capture's JSON encoding, so they compare as the same types. */
    private static function normalise(array $surface): array
    {
        $flags = JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR | JSON_PRESERVE_ZERO_FRACTION;
        return json_decode(json_encode($surface, $flags), true, 512, JSON_THROW_ON_ERROR);
    }

    /**
     * Each difference, named: a class or member added or removed, or a member that changed.
     *
     * @return list<string>
     */
    private static function differences(array $expected, array $actual): array
    {
        $out = [];
        foreach (array_diff_key($expected, $actual) as $class => $_) {
            $out[] = "{$class}: removed";
        }
        foreach (array_diff_key($actual, $expected) as $class => $_) {
            $out[] = "{$class}: added";
        }
        foreach (array_intersect_key($expected, $actual) as $class => $want) {
            $have = $actual[$class];
            foreach (['kind', 'final', 'readonly', 'instantiable', 'interfaces', 'parent'] as $key) {
                if ($want[$key] !== $have[$key]) {
                    $out[] = "{$class}: {$key} " . self::show($want[$key]) . ' became ' . self::show($have[$key]);
                }
            }
            foreach (['constants' => '', 'cases' => '', 'methods' => '()', 'properties' => ''] as $key => $suffix) {
                $sep = $key === 'properties' ? '::$' : '::';
                foreach (array_diff_key($want[$key], $have[$key]) as $name => $_) {
                    $out[] = "{$class}{$sep}{$name}{$suffix}: removed";
                }
                foreach (array_diff_key($have[$key], $want[$key]) as $name => $_) {
                    $out[] = "{$class}{$sep}{$name}{$suffix}: added";
                }
                foreach (array_intersect_key($want[$key], $have[$key]) as $name => $member) {
                    if ($member !== $have[$key][$name]) {
                        $out[] = "{$class}{$sep}{$name}{$suffix}: " . self::show($member)
                            . ' became ' . self::show($have[$key][$name]);
                    }
                }
            }
        }
        return $out;
    }

    private static function show(mixed $value): string
    {
        return json_encode($value, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR);
    }
}
