<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use HyperCast\Cast;
use HyperCast\CastFailure;
use HyperCast\Checked;
use HyperCast\Constraints;
use HyperCast\DetailedCast;
use HyperCast\DetailedFault;
use HyperCast\Door;
use HyperCast\Fault;
use HyperCast\FaultDetail;
use HyperCast\NumFormat;
use HyperCast\Success;
use HyperCast\Violation;
use PHPUnit\Framework\TestCase;

/**
 * Item 4, fault detail: every detailed failure carries the raw code and a named value with an
 * `Unknown` member; the 1.0 `Fault` is left exactly as it was, and the detail rides a separate
 * result path.
 */
final class DetailTest extends TestCase
{
    public function testEveryPublishedCodeIsACaseAndNoOtherIs(): void
    {
        $cases = [];
        foreach (FaultDetail::cases() as $case) {
            $cases[$case->value] = $case->name;
        }
        ksort($cases);
        self::assertSame(0, array_key_first($cases));
        self::assertSame('Unknown', $cases[0]);
        self::assertSame(range(0, 32), array_keys($cases));
        $named = array_flip(array_map(
            static fn (string $name): string => str_replace('_', '', $name),
            array_keys(CorpusTest::DETAIL_CODES)
        ));
        foreach (CorpusTest::DETAIL_CODES as $name => $code) {
            self::assertSame(
                strtolower(str_replace('_', '', $name)),
                strtolower($cases[$code]),
                "code {$code} is {$name}"
            );
        }
        self::assertCount(32, $named);
    }

    public function testADetailedFaultIsBuiltFromTheRawCode(): void
    {
        $fault = new DetailedFault(CastFailure::Malformed, 2, 3, 11);
        self::assertSame(
            [CastFailure::Malformed, 2, 3, 11, FaultDetail::GroupSize],
            [$fault->reason, $fault->offset, $fault->length, $fault->code, $fault->detail]
        );
        self::assertEquals(new Fault(CastFailure::Malformed, 2, 3), $fault->toFault());
    }

    public function testAnUnknownCodeNeverThrowsAndTheReasonAndSpanStayUsable(): void
    {
        foreach ([999, 33, 0, 4294967295, PHP_INT_MAX] as $code) {
            $fault = new DetailedFault(CastFailure::OutOfRange, 5, 7, $code);
            self::assertSame(FaultDetail::Unknown, $fault->detail, "code {$code}");
            self::assertSame($code, $fault->code);
            self::assertSame([CastFailure::OutOfRange, 5, 7], [$fault->reason, $fault->offset, $fault->length]);
            self::assertEquals(new Fault(CastFailure::OutOfRange, 5, 7), $fault->toFault());
            // And it round-trips through the engine's own serialization.
            self::assertEquals($fault, unserialize(serialize($fault)));
        }
        // Published codes beside it keep their names.
        self::assertSame(FaultDetail::TooLarge, (new DetailedFault(CastFailure::OutOfRange, 0, 1, 25))->detail);
    }

    public function testADetailedDoorReportsTheWinningFaultAndItsDetail(): void
    {
        $fault = DetailedCast::i32('12a', NumFormat::invariant());
        self::assertInstanceOf(DetailedFault::class, $fault);
        self::assertSame(
            [CastFailure::Malformed, 2, 1, FaultDetail::UnexpectedCharacter, 2],
            [$fault->reason, $fault->offset, $fault->length, $fault->detail, $fault->code]
        );
        self::assertEquals(new Success(7), DetailedCast::i32('7', NumFormat::invariant()));
        $empty = DetailedCast::i32('  ', NumFormat::invariant());
        self::assertInstanceOf(DetailedFault::class, $empty);
        self::assertSame([CastFailure::Empty, FaultDetail::Blank], [$empty->reason, $empty->detail]);
        $big = DetailedCast::i8('128', NumFormat::invariant());
        self::assertSame([CastFailure::OutOfRange, FaultDetail::TooLarge], [$big->reason, $big->detail]);
        $bad = DetailedCast::uuid("\xff");
        self::assertSame(FaultDetail::InvalidUtf8, $bad->detail);
        // `Cast::optional` has its detailed twin: only an Empty fault becomes null.
        self::assertNull(DetailedCast::optional($empty));
        self::assertSame($fault, DetailedCast::optional($fault));
    }

    public function testACheckedViolationCarriesNoDetail(): void
    {
        $constraints = new Constraints(Door::I32, maximum: 10);
        $violation = DetailedCast::checked('11', NumFormat::invariant(), $constraints);
        self::assertEquals(Cast::checked('11', NumFormat::invariant(), $constraints), $violation);
        self::assertInstanceOf(Violation::class, $violation);
        $failure = DetailedCast::checked('x', NumFormat::invariant(), $constraints);
        self::assertInstanceOf(DetailedFault::class, $failure);
        self::assertSame(FaultDetail::UnexpectedCharacter, $failure->detail);
        self::assertInstanceOf(Checked::class, DetailedCast::checked('9', NumFormat::invariant(), $constraints));
        $nan = DetailedCast::checkedFromFloat(NAN, $constraints);
        self::assertInstanceOf(DetailedFault::class, $nan);
        self::assertSame(FaultDetail::NotFinite, $nan->detail);
    }

    /** The 1.0 surface, used exactly as 1.0's code used it. */
    public function testThe1Point0FaultIsExactlyAsItWas(): void
    {
        $fault = new Fault(CastFailure::Malformed, 2, 1);
        // The constructor: three positional arguments, nothing else.
        self::assertSame(
            ['HyperCast\\CastFailure $reason', 'int $offset', 'int $length'],
            array_map(
                static fn (\ReflectionParameter $p): string => $p->getType() . ' $' . $p->getName(),
                (new \ReflectionMethod(Fault::class, '__construct'))->getParameters()
            )
        );
        self::assertSame(['reason', 'offset', 'length'], array_keys(get_object_vars($fault)));
        // Equality, loose and strict, as 1.0's callers wrote it.
        self::assertTrue($fault == new Fault(CastFailure::Malformed, 2, 1));
        self::assertFalse($fault == new Fault(CastFailure::Malformed, 2, 2));
        self::assertEquals($fault, Cast::i32('12a', NumFormat::invariant()));
        self::assertSame(
            'O:15:"HyperCast\Fault":3:{s:6:"reason";E:31:"HyperCast\CastFailure:Malformed";'
                . 's:6:"offset";i:2;s:6:"length";i:1;}',
            serialize($fault)
        );
        self::assertEquals($fault, unserialize(serialize($fault)));
        // The match idiom over the closed pair, deconstructing by property.
        $door = Cast::i32('12a', NumFormat::invariant());
        $text = match (true) {
            $door instanceof Success => "ok {$door->value}",
            $door instanceof Fault => "{$door->reason->name} at {$door->offset}+{$door->length}",
        };
        self::assertSame('Malformed at 2+1', $text);
        // A plain door never returns the detailed type; its return type is still the closed pair.
        self::assertSame(Fault::class, $door::class);
        self::assertSame(
            'HyperCast\\Success|HyperCast\\Fault',
            (string) (new \ReflectionMethod(Cast::class, 'i32'))->getReturnType()
        );
        // A detailed fault never equals a 1.0 fault (a different class), so equality is never
        // changed for either.
        self::assertFalse($fault == new DetailedFault(CastFailure::Malformed, 2, 1, 2));
    }

    public function testEveryPlainDoorHasADetailedTwinOfTheSameShape(): void
    {
        $plain = (new \ReflectionClass(Cast::class))->getMethods(\ReflectionMethod::IS_PUBLIC);
        $twins = 0;
        foreach ($plain as $method) {
            if ($method->isConstructor() || $method->getName() === 'nativeVersion') {
                continue;
            }
            self::assertTrue(method_exists(DetailedCast::class, $method->getName()), $method->getName());
            $twin = new \ReflectionMethod(DetailedCast::class, $method->getName());
            // The same parameters; the one verdict `optional()` takes names the detailed fault.
            $params = static fn (\ReflectionMethod $m): array => array_map(
                static fn (\ReflectionParameter $p): string => str_replace(
                    'HyperCast\\DetailedFault',
                    'HyperCast\\Fault',
                    $p->getType() . ' $' . $p->getName()
                ),
                $m->getParameters()
            );
            self::assertSame($params($method), $params($twin), $method->getName());
            self::assertSame(
                str_replace('HyperCast\\Fault', 'HyperCast\\DetailedFault', (string) $method->getReturnType()),
                (string) $twin->getReturnType(),
                $method->getName()
            );
            $twins++;
        }
        self::assertGreaterThanOrEqual(30, $twins);
    }

    public function testADetailedFaultIsReadonlyAndFinal(): void
    {
        $class = new \ReflectionClass(DetailedFault::class);
        self::assertTrue($class->isFinal());
        self::assertTrue($class->isReadOnly());
        $fault = new DetailedFault(CastFailure::Malformed, 0, 1, 2);
        $this->expectException(\Error::class);
        // @phpstan-ignore-next-line
        $fault->code = 3;
    }
}
