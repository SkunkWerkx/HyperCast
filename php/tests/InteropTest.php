<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use FFI;
use HyperCast\Cast;
use HyperCast\CastFailure;
use HyperCast\DayPeriods;
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

    /** The C declarations a host restates, exactly as Cast.php's cdef has them. */
    private static function layouts(): FFI
    {
        return FFI::cdef(
            'typedef struct { uint8_t bytes[8]; uint8_t len; } symbol8;'
            . 'typedef struct { uint8_t bytes[16]; uint8_t len; } symbol16;'
            . 'typedef struct { uint32_t size; uint32_t flags; uint32_t digit_zero; symbol8 decimal_sep;'
            . ' symbol8 group_sep; symbol8 currency_decimal_sep; symbol8 currency_group_sep; symbol8 minus_sign;'
            . ' symbol8 plus_sign; symbol8 percent; symbol16 currency; } format;'
            . 'typedef struct { uint32_t size; symbol16 am; symbol16 pm; uint8_t pad[2]; } periods;'
        );
    }

    public function testAFormatWritesTheCoresLayoutAndLeavesNoTailOfAnEarlierOne(): void
    {
        $ffi = self::layouts();
        $target = $ffi->new('format');
        NativeValues::writeFormat(
            new NumFormat(', ', '.', NumFormat::ALL, 'US$', "\u{66B}", "\u{66C}", "\u{2212}", "\u{FF0B}", "\u{66A}"),
            $target
        );
        NativeValues::writeFormat(new NumFormat(',', '.', NumFormat::ALL, '€'), $target);
        $this->assertSame(92, $target->size);
        $this->assertSame(NumFormat::ALL, $target->flags);
        $this->assertSame(0, $target->digit_zero);
        $this->assertSame(1, $target->decimal_sep->len);
        $this->assertSame(',' . str_repeat("\0", 7), FFI::string($target->decimal_sep->bytes, 8));
        $this->assertSame('.', \chr($target->group_sep->bytes[0]));
        foreach (['currency_decimal_sep', 'currency_group_sep', 'minus_sign', 'plus_sign', 'percent'] as $symbol) {
            $this->assertSame(0, $target->$symbol->len, $symbol);
            $this->assertSame(str_repeat("\0", 8), FFI::string($target->$symbol->bytes, 8), $symbol);
        }
        $this->assertSame(3, $target->currency->len);
        $this->assertSame("€" . str_repeat("\0", 13), FFI::string($target->currency->bytes, 16));
    }

    public function testTheLayoutsAreTheSizesAndOffsetsTheCorePins(): void
    {
        // rust/src/abi.rs pins RawNumFormat at 92 bytes and RawDayPeriods at 40, both
        // 4-aligned, and every field's offset; the packed strings land each field there.
        $ffi = self::layouts();
        $this->assertSame([92, 4], [FFI::sizeof($ffi->type('format')), FFI::alignof($ffi->type('format'))]);
        $this->assertSame([40, 4], [FFI::sizeof($ffi->type('periods')), FFI::alignof($ffi->type('periods'))]);
        $this->assertSame(92, NumFormat::PACKED_BYTES);
        $this->assertSame(40, DayPeriods::PACKED_BYTES);
        $format = new NumFormat(
            "\u{66B}",
            ', ',
            NumFormat::ALL | NumFormat::TRAILING_SIGN,
            'kr.',
            '.',
            "\u{A0}",
            "\u{2212}",
            "\u{FF0B}",
            "\u{66A}",
            "\u{660}"
        );
        $packed = $format->packed();
        $this->assertSame(92, \strlen($packed));
        $this->assertSame([92, NumFormat::ALL | NumFormat::TRAILING_SIGN, 0x660], array_values(unpack('V3', $packed)));
        $fields = [
            12 => ["\u{66B}", 8],
            21 => [', ', 8],
            30 => ['.', 8],
            39 => ["\u{A0}", 8],
            48 => ["\u{2212}", 8],
            57 => ["\u{FF0B}", 8],
            66 => ["\u{66A}", 8],
            75 => ['kr.', 16],
        ];
        foreach ($fields as $offset => [$symbol, $capacity]) {
            $this->assertSame(
                str_pad($symbol, $capacity, "\0") . \chr(\strlen($symbol)),
                substr($packed, $offset, $capacity + 1),
                "offset {$offset}"
            );
        }
        // The same bytes through the typed struct: each field read back where the C
        // declaration puts it, and the struct's own bytes are the packed string.
        $target = $ffi->new('format');
        NativeValues::writeFormat($format, $target);
        $this->assertSame(0x660, $target->digit_zero);
        $this->assertSame('kr.', FFI::string($target->currency->bytes, 3));
        $this->assertSame(3, $target->minus_sign->len);
        $this->assertSame($packed, FFI::string($ffi->cast('char *', FFI::addr($target)), 92));
        $base = $ffi->cast('uint8_t *', FFI::addr($target));
        $this->assertSame(48, $ffi->cast('uint8_t *', FFI::addr($target->minus_sign)) - $base);
        $this->assertSame(75, $ffi->cast('uint8_t *', FFI::addr($target->currency)) - $base);

        $periods = new DayPeriods('a. m.', "\u{C624}\u{D6C4}");
        $packed = $periods->packed();
        $this->assertSame(40, \strlen($packed));
        $this->assertSame(40, unpack('V', $packed)[1]);
        $this->assertSame("a. m.\0\0\0\0\0\0\0\0\0\0\0\x05", substr($packed, 4, 17));
        $this->assertSame("\u{C624}\u{D6C4}" . str_repeat("\0", 10) . "\x06", substr($packed, 21, 17));
        $this->assertSame("\0\0", substr($packed, 38, 2));
        $target = $ffi->new('periods');
        NativeValues::writeDayPeriods(new DayPeriods('vorm.', 'nachm.'), $target);
        NativeValues::writeDayPeriods($periods, $target);
        $this->assertSame(40, $target->size);
        $this->assertSame([5, 6], [$target->am->len, $target->pm->len]);
        $this->assertSame($packed, FFI::string($ffi->cast('char *', FFI::addr($target)), 40));
        $base = $ffi->cast('uint8_t *', FFI::addr($target));
        $this->assertSame(21, $ffi->cast('uint8_t *', FFI::addr($target->pm)) - $base);
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
