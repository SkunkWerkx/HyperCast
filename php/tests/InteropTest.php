<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use FFI;
use HyperCast\Cast;
use HyperCast\CastFailure;
use HyperCast\Fault;
use HyperCast\Interop\NativePlatform;
use HyperCast\Interop\NativeValues;
use HyperCast\NumFormat;
use HyperCast\Success;
use PHPUnit\Framework\TestCase;

/**
 * The public interop surface a package carrying HyperCast's verdicts across its own C ABI
 * builds values with: each one as the door of the same name presents it.
 */
final class InteropTest extends TestCase
{
    public function testValuesPresentAsTheDoorsPresentThem(): void
    {
        $door = Cast::timestamp('2026-01-02T15:04:05.123456789Z');
        $this->assertInstanceOf(Success::class, $door);
        $this->assertEquals($door->value, NativeValues::instant(1_767_366_245, 123_456_789));

        $date = Cast::date('2026-01-07');
        $this->assertInstanceOf(Success::class, $date);
        $this->assertEquals($date->value, NativeValues::date(2026, 1, 7));

        $civil = Cast::datetime('2026-01-07 15:04:05.123456789', \HyperCast\DateOrder::Ymd);
        $this->assertInstanceOf(Success::class, $civil);
        $this->assertEquals($civil->value, NativeValues::civil(2026, 1, 7, 54_245_123_456_789));

        $this->assertSame(
            '550e8400-e29b-41d4-a716-446655440000',
            NativeValues::uuid(hex2bin('550e8400e29b41d4a716446655440000'))
        );
    }

    public function testAFaultKeepsItsSpanAndACodeThatNamesNoReasonIsRefused(): void
    {
        $this->assertEquals(new Fault(CastFailure::Malformed, 3, 4), NativeValues::fault(2, 3, 4));
        $this->expectException(\LogicException::class);
        NativeValues::fault(7, 0, 0);
    }

    public function testVersionsUnpack(): void
    {
        $this->assertSame('0.6.2', NativeValues::version(0x00_06_02));
        $this->assertSame('1.2.3', NativeValues::version(0x01_02_03));
    }

    public function testAFormatWritesTheCoresLayoutAndLeavesNoTailOfAnEarlierOne(): void
    {
        $ffi = FFI::cdef(
            'typedef struct { uint32_t decimal_sep; uint32_t group_sep; uint32_t flags;'
            . ' uint32_t currency_len; uint8_t currency[16]; } format;'
        );
        $target = $ffi->new('format');
        NativeValues::writeFormat(new NumFormat('.', ',', NumFormat::ALL, 'US$'), $target);
        NativeValues::writeFormat(new NumFormat(',', '.', NumFormat::ALL, '€'), $target);
        $this->assertSame(\ord(','), $target->decimal_sep);
        $this->assertSame(\ord('.'), $target->group_sep);
        $this->assertSame(NumFormat::ALL, $target->flags);
        $this->assertSame(3, $target->currency_len);
        $this->assertSame("€" . str_repeat("\0", 13), FFI::string($target->currency, 16));
    }

    public function testTheLibraryIsNamedForTheCoreAskedFor(): void
    {
        $this->assertSame(
            ['linux-x64', 'libhypertabular.so'],
            NativePlatform::resolve('hypertabular', 'Linux', 'x86_64', 8, false)
        );
        $this->assertSame(
            ['win-x64', 'hypercast.dll'],
            NativePlatform::resolve('hypercast', 'Windows', 'ARM64', 8, false)
        );
    }
}
