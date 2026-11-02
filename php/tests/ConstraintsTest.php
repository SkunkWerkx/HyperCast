<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use HyperCast\Cast;
use HyperCast\CastFailure;
use HyperCast\Checked;
use HyperCast\Constraints;
use HyperCast\Decimal;
use HyperCast\Door;
use HyperCast\Fault;
use HyperCast\Keyword;
use HyperCast\NumFormat;
use HyperCast\Rounding;
use HyperCast\Tests\Support\ChildProcess;
use HyperCast\Violation;
use PHPUnit\Framework\Attributes\DataProvider;
use PHPUnit\Framework\TestCase;

/**
 * The binding's own contracts for the validation constraints, beyond what the corpus replays
 * (CorpusTest): what a declaration accepts, how it is refused, and what the checked doors
 * hand back.
 */
final class ConstraintsTest extends TestCase
{
    use ChildProcess;

    /** @return iterable<string, array{int, \Closure(): Constraints}> */
    public static function refusals(): iterable
    {
        yield 'keyword not for the door' => [1, static fn () => new Constraints(Door::Text, minimum: 1)];
        yield 'minimum with exclusiveMinimum' => [2, static fn () => new Constraints(
            Door::I32,
            minimum: 1,
            exclusiveMinimum: 1,
        )];
        yield 'maximum with exclusiveMaximum' => [3, static fn () => new Constraints(
            Door::I32,
            maximum: 1,
            exclusiveMaximum: 1,
        )];
        yield 'lower not below upper' => [4, static fn () => new Constraints(Door::I32, minimum: 2, maximum: 1)];
        yield 'multipleOf not positive' => [5, static fn () => new Constraints(Door::I32, multipleOf: '-1')];
        yield 'rounding without multipleOf' => [6, static fn () => new Constraints(
            Door::I32,
            rounding: Rounding::Floor,
        )];
        yield 'minLength above maxLength' => [7, static fn () => new Constraints(
            Door::Text,
            minLength: 2,
            maxLength: 1,
        )];
        yield 'not held by the door' => [8, static fn () => new Constraints(Door::U8, maximum: '256')];
        yield 'an enum entry that is not UTF-8' => [9, static fn () => new Constraints(Door::Text, enum: ["\xff"])];
        yield 'a duplicate enum entry' => [10, static fn () => new Constraints(Door::Text, enum: ['I', 'i'])];
        yield 'an empty integer enum' => [12, static fn () => new Constraints(Door::I32, enum: [])];
        yield 'an empty text enum' => [12, static fn () => new Constraints(Door::Text, trim: true, enum: [])];
        yield 'an enum at a door that takes none, first' => [1, static fn () => new Constraints(Door::F64, enum: [])];
        yield 'an empty text entry' => [13, static fn () => new Constraints(Door::Text, enum: ['Male', ''])];
        yield 'an entry trim leaves unmatchable' => [13, static fn () => new Constraints(
            Door::Text,
            trim: true,
            enum: [' Male '],
        )];
    }

    /**
     * Every refusal the core can be asked for from PHP is an `\InvalidArgumentException`, its
     * code the core's, its message that code's own. Code 11, a malformed layout, cannot be
     * asked for: `Rounding` is an enum and the binding writes every layout.
     *
     * @param \Closure(): Constraints $declare
     */
    #[DataProvider('refusals')]
    public function testEveryRefusalThrowsItsOwnMessage(int $code, \Closure $declare): void
    {
        try {
            $declare();
            self::fail('not refused');
        } catch (\InvalidArgumentException $e) {
            self::assertSame(\InvalidArgumentException::class, $e::class);
            self::assertSame($code, $e->getCode());
            self::assertSame(CorpusTest::refusalMessage($code), $e->getMessage());
        }
    }

    public function testTheDoorIsRequired(): void
    {
        $this->expectException(\ArgumentCountError::class);
        new Constraints(...[]);
    }

    /** No declaration is read as "no rules": one never built is the engine's `Error`. */
    public function testAnUnbuiltDeclarationIsNeverNoRules(): void
    {
        $unbuilt = (new \ReflectionClass(Constraints::class))->newInstanceWithoutConstructor();
        foreach (
            [
                static fn () => Cast::checked('1', NumFormat::invariant(), $unbuilt),
                static fn () => Cast::checkedFromFloat(1.0, $unbuilt),
                static fn () => Cast::checkedText('x', $unbuilt),
                static fn () => $unbuilt->packed(),
            ] as $call
        ) {
            try {
                $call();
                self::fail('an unbuilt declaration was read');
            } catch (\Error $e) {
                self::assertSame(\Error::class, $e::class);
                self::assertSame(
                    'Typed property HyperCast\\Constraints::$door must not be accessed before initialization',
                    $e->getMessage()
                );
            }
        }
    }

    /**
     * Bounds, `multipleOf` and integer `enum` entries are an int, a string or a `Decimal`, and
     * never a float or a bool; lengths are an int, never a bool or a numeric string. The check is
     * exact in weak mode too, where PHP would otherwise coerce them.
     */
    public function testBoundsAndLengthsAreNeverCoerced(): void
    {
        $refused = [
            'new HyperCast\\Constraints(HyperCast\\Door::F64, minimum: 0.5)'
                => 'Argument #2 ($minimum) must be of type HyperCast\\Decimal|string|int|null, float given',
            'new HyperCast\\Constraints(HyperCast\\Door::F64, multipleOf: true)'
                => 'Argument #6 ($multipleOf) must be of type HyperCast\\Decimal|string|int|null, true given',
            'new HyperCast\\Constraints(HyperCast\\Door::Text, minLength: true)'
                => 'Argument #8 ($minLength) must be of type ?int, true given',
            'new HyperCast\\Constraints(HyperCast\\Door::Text, maxLength: "3")'
                => 'Argument #9 ($maxLength) must be of type ?int, string given',
            'new HyperCast\\Constraints(HyperCast\\Door::I32, enum: [1, 2.0])'
                => 'Argument #11 ($enum) must hold only int, string or HyperCast\\Decimal entries; entry 1 is float',
            'new HyperCast\\Constraints(HyperCast\\Door::Text, enum: ["a", 1])'
                => 'Argument #11 ($enum) must hold only strings at HyperCast\\Door::Text; entry 1 is int',
        ];
        foreach (['', 'declare(strict_types=1);'] as $mode) {
            foreach ($refused as $code => $message) {
                $out = self::child(
                    ['-n', '-d', 'extension=' . self::ext()],
                    "{$mode} try { {$code}; echo 'accepted'; } catch (\\TypeError \$e) { echo \$e->getMessage(); }"
                );
                $want = "HyperCast\\Constraints::__construct(): {$message}";
                self::assertStringStartsWith($want, $out, "{$mode} {$code}");
            }
        }
    }

    public function testALengthOutsideThirtyTwoBitsIsAValueError(): void
    {
        $this->expectException(\ValueError::class);
        $this->expectExceptionMessage(
            'HyperCast\\Constraints::__construct(): Argument #9 ($maxLength) must be between 0 and 4294967295'
        );
        new Constraints(Door::Text, maxLength: -1);
    }

    /** Each decimal is kept as the exact `Decimal` the core read, whatever form declared it. */
    public function testBoundsAreKeptAsTheDecimalTheCoreRead(): void
    {
        $byString = new Constraints(Door::Decimal, minimum: '0.50', multipleOf: '0.05');
        $byDecimal = new Constraints(
            Door::Decimal,
            minimum: new Decimal('5', 1, false),
            multipleOf: new Decimal('5', 2, false),
        );
        self::assertEquals($byString, $byDecimal);
        self::assertEquals(new Decimal('5', 1, false), $byString->minimum);
        self::assertEquals(new Decimal('9', 0, true), (new Constraints(Door::I8, maximum: -9))->maximum);
        self::assertNull($byString->maximum);
        self::assertSame(Rounding::Strict, $byString->rounding);
        self::assertEquals([new Decimal('7', 0, false)], (new Constraints(Door::I32, enum: ['7.0']))->enum);
        self::assertNull((new Constraints(Door::Text, enum: null))->enum);
        // Untrimmed, edge white space is part of the entry, declared and matched as given.
        $spaced = new Constraints(Door::Text, enum: [' Male ']);
        self::assertSame([' Male '], $spaced->enum);
        self::assertSame(256, unpack('L', $spaced->packed(), 4)[1] & 256);
        self::assertSame(0, unpack('L', (new Constraints(Door::Text))->packed(), 4)[1] & 256);
        self::assertEquals(new Checked(' Male ', false, 0), Cast::checkedText(' MALE ', $spaced));
    }

    public function testADeclarationHandedToTheWrongDoorIsACallerBug(): void
    {
        $text = new Constraints(Door::Text, maxLength: 3);
        $number = new Constraints(Door::F64, maximum: 1);
        $cases = [
            'Constraints for HyperCast\\Door::Text are judged by HyperCast\\Cast::checkedText()' => [
                static fn () => Cast::checked('1', NumFormat::invariant(), $text),
                static fn () => Cast::checkedFromFloat(1.0, $text),
            ],
            'Constraints for HyperCast\\Door::F64 are judged by HyperCast\\Cast::checked()' => [
                static fn () => Cast::checkedText('1', $number),
            ],
        ];
        foreach ($cases as $message => $calls) {
            foreach ($calls as $call) {
                try {
                    $call();
                    self::fail('a declaration for another door was judged');
                } catch (\InvalidArgumentException $e) {
                    self::assertSame($message, $e->getMessage());
                }
            }
        }
    }

    /** A text `enum` match is the caller's own declared entry, with its index. */
    public function testATextEnumMatchIsTheDeclaredEntry(): void
    {
        $sex = new Constraints(Door::Text, trim: true, enum: ['Female', 'Male', 'Other']);
        self::assertEquals(new Checked('Male', false, 1), Cast::checkedText('  MALE ', $sex));
        $violation = Cast::checkedText('  Femal ', $sex);
        self::assertEquals(new Violation(Keyword::Enum, 2, 5), $violation);
        // No enum: the judged text, trimmed when declared.
        $trimmed = Cast::checkedText(" ab\u{3000}", new Constraints(Door::Text, trim: true));
        self::assertEquals(new Checked('ab'), $trimmed);
        self::assertEquals(new Checked(' ab '), Cast::checkedText(' ab ', new Constraints(Door::Text)));
        // Trimmed to nothing is empty, a plain `Fault` that `optional()` reads.
        $empty = Cast::checkedText("\u{A0} ", new Constraints(Door::Text, trim: true));
        self::assertEquals(new Fault(CastFailure::Empty, 0, 0), $empty);
        self::assertNull(Cast::optional($empty));
    }

    /**
     * `Keyword` is open: a code this binding does not name yet is kept as the bare int, named
     * `keyword N`, and every named case's OpenAPI spelling is its name, first letter lowered.
     */
    public function testAKeywordThisBindingDoesNotNameIsKept(): void
    {
        $unknown = new Violation(9, 0, 1);
        self::assertSame(9, $unknown->keyword);
        self::assertSame('keyword 9', $unknown->keywordName());
        $matched = match ($unknown->keyword) {
            Keyword::Minimum => 'minimum',
            default => 'some other rule',
        };
        self::assertSame('some other rule', $matched);
        $names = ['minimum', 'exclusiveMinimum', 'maximum', 'exclusiveMaximum', 'multipleOf', 'minLength',
            'maxLength', 'enum'];
        foreach (Keyword::cases() as $i => $keyword) {
            self::assertSame($i + 1, $keyword->value);
            self::assertSame($names[$i], (new Violation($keyword, 0, 0))->keywordName());
            self::assertSame($names[$i], lcfirst($keyword->name));
        }
    }

    public function testTheEnumsCarryTheCoresCodes(): void
    {
        $codes = static fn (array $cases): array => array_combine(
            array_map(static fn (\BackedEnum $c): string => $c->name, $cases),
            array_map(static fn (\BackedEnum $c): int => $c->value, $cases),
        );
        self::assertSame(
            ['I8' => 2, 'I16' => 3, 'I32' => 4, 'I64' => 5, 'U8' => 6, 'U16' => 7, 'U32' => 8, 'U64' => 9,
                'F32' => 10, 'F64' => 11, 'Decimal' => 19, 'Text' => 18],
            $codes(Door::cases())
        );
        self::assertSame(
            ['Strict' => 0, 'Midpoint' => 1, 'Floor' => 2, 'Ceiling' => 3],
            $codes(Rounding::cases())
        );
    }

    /** Zero has one form: no checked door delivers a negative zero. */
    public function testZeroHasOneForm(): void
    {
        $zero = bin2hex(pack('e', 0.0));
        $step = new Constraints(Door::F64, multipleOf: '0.000001', rounding: Rounding::Midpoint);
        $rounded = Cast::checked('-0.0000004', NumFormat::invariant(), $step);
        self::assertInstanceOf(Checked::class, $rounded);
        self::assertTrue($rounded->adjusted);
        self::assertSame($zero, bin2hex(pack('e', $rounded->value)));
        foreach ([Door::F32, Door::F64] as $door) {
            $decoded = Cast::checkedFromFloat(-0.0, new Constraints($door, minimum: 0));
            self::assertInstanceOf(Checked::class, $decoded);
            self::assertSame($zero, bin2hex(pack('e', $decoded->value)), $door->name);
            $text = Cast::checked('-0', NumFormat::invariant(), new Constraints($door));
            self::assertSame($zero, bin2hex(pack('e', $text->value)), $door->name);
        }
    }

    /** `0.1 + 0.2` breaks `multipleOf: '0.1'` unrounded, and is `0.3`, adjusted, at midpoint. */
    public function testADecodedFloatIsJudgedOnItsShortestDecimal(): void
    {
        $sum = 0.1 + 0.2;
        self::assertEquals(
            new Violation(Keyword::MultipleOf, 0, 0),
            Cast::checkedFromFloat($sum, new Constraints(Door::F64, multipleOf: '0.1'))
        );
        $rounded = Cast::checkedFromFloat(
            $sum,
            new Constraints(Door::F64, multipleOf: '0.1', rounding: Rounding::Midpoint)
        );
        self::assertEquals(new Checked(0.3, true), $rounded);
    }

    /**
     * The values are PHP's own readonly objects: every write is refused, serialization keeps
     * them, and a declaration PHP built around the constructor is still judged by the core,
     * its refusal thrown by the door that reads it.
     */
    public function testTheValuesAreReadonlyAndSerializable(): void
    {
        $constraints = new Constraints(Door::Text, trim: true, enum: ['a']);
        try {
            $constraints->trim = false;
            self::fail('a readonly property was written');
        } catch (\Error $e) {
            self::assertSame('Cannot modify readonly property HyperCast\\Constraints::$trim', $e->getMessage());
        }
        try {
            $constraints->__construct(Door::Text);
            self::fail('a built declaration was built again');
        } catch (\Error $e) {
            self::assertSame('Cannot modify readonly property HyperCast\\Constraints::$door', $e->getMessage());
        }
        $copy = unserialize(serialize($constraints));
        self::assertEquals($constraints, $copy);
        self::assertEquals(Cast::checkedText(' A ', $constraints), Cast::checkedText(' A ', $copy));
        $tampered = unserialize(str_replace(
            'E:19:"HyperCast\\Door:Text";',
            'E:18:"HyperCast\\Door:F64";',
            serialize(new Constraints(Door::Text, trim: true))
        ));
        self::assertSame(Door::F64, $tampered->door);
        try {
            Cast::checkedFromFloat(1.0, $tampered);
            self::fail('a refused declaration was judged');
        } catch (\InvalidArgumentException $e) {
            self::assertSame(1, $e->getCode());
            self::assertSame(CorpusTest::refusalMessage(1), $e->getMessage());
        }
    }

    /**
     * `enum` entries PHP holds by reference (a `foreach` by reference leaves them so) are read
     * as their values, and the declaration keeps the values: writing the caller's variable
     * afterwards changes neither the declaration nor its verdicts.
     */
    public function testEnumEntriesHeldByReferenceAreReadAsTheirValues(): void
    {
        $words = ['Female', 'Male'];
        foreach ($words as &$word) {
        }
        $sex = new Constraints(Door::Text, enum: $words);
        $word = 'Other';
        unset($word);
        self::assertSame(['Female', 'Other'], $words);
        self::assertSame(['Female', 'Male'], $sex->enum);
        self::assertEquals(new Checked('Male', false, 1), Cast::checkedText('male', $sex));
        self::assertEquals(new Violation(Keyword::Enum, 0, 5), Cast::checkedText('other', $sex));

        $codes = [10, '20'];
        foreach ($codes as &$code) {
        }
        $tens = new Constraints(Door::I32, enum: $codes);
        $code = 30;
        unset($code);
        self::assertEquals([new Decimal('10', 0, false), new Decimal('20', 0, false)], $tens->enum);
        self::assertEquals(new Checked(20), Cast::checked('20', NumFormat::invariant(), $tens));
        self::assertEquals(
            new Violation(Keyword::Enum, 0, 2),
            Cast::checked('30', NumFormat::invariant(), $tens)
        );
    }

    /**
     * A `Constraints` `unserialize()` built with values the constructor would not take is the
     * caller's bug, an `\InvalidArgumentException` from the door that reads it.
     */
    public function testATamperedDeclarationIsACallerBug(): void
    {
        $serialized = serialize(new Constraints(Door::Text, enum: ['a']));
        $cases = [
            's:9:"minLength";i:-1;' => 'minLength must be between 0 and 4294967295',
            'a:1:{i:0;i:7;}' => 'enum must hold only strings at HyperCast\\Door::Text; entry 0 is int',
        ];
        $originals = ['s:9:"minLength";i:-1;' => 's:9:"minLength";N;', 'a:1:{i:0;i:7;}' => 'a:1:{i:0;s:1:"a";}'];
        foreach ($cases as $tampered => $why) {
            $copy = unserialize(str_replace($originals[$tampered], $tampered, $serialized));
            self::assertInstanceOf(Constraints::class, $copy);
            try {
                Cast::checkedText('a', $copy);
                self::fail("{$tampered} was judged");
            } catch (\InvalidArgumentException $e) {
                self::assertSame(\InvalidArgumentException::class, $e::class);
                self::assertSame(
                    "not a declaration HyperCast\\Constraints::__construct accepts: {$why}",
                    $e->getMessage()
                );
            }
        }
    }

    /** A code this binding names is its `Keyword` case however the `Violation` was built. */
    public function testAViolationKeepsANamedCodeAsItsCase(): void
    {
        $built = Cast::checked('9', NumFormat::invariant(), new Constraints(Door::I32, minimum: 10));
        self::assertEquals($built, new Violation(1, 0, 1));
        self::assertSame(Keyword::Minimum, (new Violation(1, 0, 1))->keyword);
        self::assertSame(9, (new Violation(9, 1, 1))->keyword);
        self::assertSame(1, (new Violation(9, 1, 1))->offset);
    }

    /** `packed()` is the core's 72-byte `RawConstraints`, `packedEnum()` the list beside it. */
    public function testThePackedLayouts(): void
    {
        $constraints = new Constraints(
            Door::I32,
            minimum: -5,
            maximum: 10,
            multipleOf: 5,
            rounding: Rounding::Ceiling,
            enum: [5, '10'],
        );
        $packed = $constraints->packed();
        self::assertSame(Constraints::PACKED_BYTES, \strlen($packed));
        $head = unpack('Lsize/Lflags/Lrounding/LminLength/LmaxLength/Lreserved', $packed);
        self::assertSame(
            [
                'size' => 72,
                'flags' => 1 | 4 | 16 | 256,
                'rounding' => 3,
                'minLength' => 0,
                'maxLength' => 0,
                'reserved' => 0,
            ],
            $head
        );
        $lower = unpack('Qlo/Lhi/Cscale/Cnegative', $packed, 24);
        self::assertSame(['lo' => 5, 'hi' => 0, 'scale' => 0, 'negative' => 1], $lower);
        self::assertSame(32, \strlen($constraints->packedEnum()));
        self::assertSame(
            ['lo' => 10, 'hi' => 0, 'scale' => 0, 'negative' => 0],
            unpack('Qlo/Lhi/Cscale/Cnegative', $constraints->packedEnum(), 16)
        );
        $text = new Constraints(Door::Text, enum: ['ab', 'é']);
        self::assertSame(pack('V', 2) . 'ab' . pack('V', 2) . 'é', $text->packedEnum());
        self::assertSame('', (new Constraints(Door::Text))->packedEnum());
    }

    /** Plain `text()` is the plain verdict: the input, unjudged and untrimmed. */
    public function testPlainTextIsThePlainVerdict(): void
    {
        self::assertEquals(new \HyperCast\Success(' Male '), Cast::text(' Male '));
        self::assertEquals(new Fault(CastFailure::Empty, 0, 0), Cast::text(''));
        self::assertEquals(new Fault(CastFailure::Malformed, 2, 1), Cast::text("ab\xffc"));
    }
}
