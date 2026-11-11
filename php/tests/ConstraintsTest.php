<?php

declare(strict_types=1);

namespace HyperCast\Tests;

use HyperCast\Cast;
use HyperCast\CastFailure;
use HyperCast\Checked;
use HyperCast\Constraints;
use HyperCast\Decimal;
use HyperCast\DetailedCast;
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
        yield 'trim off the text door' => [1, static fn () => new Constraints(Door::I32, trim: true)];
        yield 'caseSensitive off the text door' => [1, static fn () => new Constraints(
            Door::F64,
            caseSensitive: true,
        )];
        yield 'a rounding at the text door' => [1, static fn () => new Constraints(
            Door::Text,
            multipleOf: 1,
            rounding: Rounding::Floor,
        )];
        yield 'multipleOf not positive' => [5, static fn () => new Constraints(Door::I32, multipleOf: '-1')];
        yield 'rounding without multipleOf' => [6, static fn () => new Constraints(
            Door::I32,
            rounding: Rounding::Floor,
        )];
        yield 'a rounding to a fractional step at an integer door' => [8, static fn () => new Constraints(
            Door::I32,
            multipleOf: '0.5',
            rounding: Rounding::Floor,
        )];
        yield 'an enum entry that is not UTF-8' => [9, static fn () => new Constraints(Door::Text, enum: ["\xff"])];
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

    /** @return iterable<string, array{\Closure(): Constraints, string, Checked|Violation}> */
    public static function openApiDeclarations(): iterable
    {
        yield 'a keyword for another type is ignored' => [
            static fn () => new Constraints(Door::I32, minLength: 5, maxLength: 1),
            '7',
            new Checked(7),
        ];
        yield 'minimum on text is ignored' => [
            static fn () => new Constraints(Door::Text, minimum: 10, maxLength: 3),
            'abc',
            new Checked('abc'),
        ];
        yield 'minimum with exclusiveMinimum, each its own keyword' => [
            static fn () => new Constraints(Door::I32, minimum: 1, exclusiveMinimum: 5),
            '3',
            new Violation(Keyword::ExclusiveMinimum, 3, 0, 1),
        ];
        yield 'maximum with exclusiveMaximum' => [
            static fn () => new Constraints(Door::I32, maximum: 10, exclusiveMaximum: 5),
            '5',
            new Violation(Keyword::ExclusiveMaximum, 5, 0, 1),
        ];
        yield 'crossed bounds, which no value meets' => [
            static fn () => new Constraints(Door::I32, minimum: 2, maximum: 1),
            '2',
            new Violation(Keyword::Maximum, 2, 0, 1),
        ];
        yield 'minLength above maxLength' => [
            static fn () => new Constraints(Door::Text, minLength: 2, maxLength: 1),
            'ab',
            new Violation(Keyword::MaxLength, 'ab', 0, 2),
        ];
        yield 'a bound past the door\'s range' => [
            static fn () => new Constraints(Door::U8, maximum: '256'),
            '255',
            new Checked(255),
        ];
        yield 'a duplicate entry, the first match winning' => [
            static fn () => new Constraints(Door::Text, enum: ['I', 'i']),
            'i',
            new Checked('I', false, 0),
        ];
        yield 'an empty enum, which every value violates' => [
            static fn () => new Constraints(Door::I32, enum: []),
            '1',
            new Violation(Keyword::Enum, 1, 0, 1),
        ];
        yield 'an empty text entry' => [
            static fn () => new Constraints(Door::Text, enum: ['Male', '']),
            'male',
            new Checked('Male', false, 0),
        ];
        yield 'an entry trim leaves unmatchable' => [
            static fn () => new Constraints(Door::Text, trim: true, enum: [' Male ']),
            ' Male ',
            new Violation(Keyword::Enum, 'Male', 1, 4),
        ];
        yield 'a fractional entry at an integer door' => [
            static fn () => new Constraints(Door::I32, enum: ['1.5', 2]),
            '2',
            new Checked(2),
        ];
        yield 'an enum at a float door, compared exactly' => [
            static fn () => new Constraints(Door::F64, enum: ['0.5', '1.25']),
            '1.250',
            new Checked(1.25),
        ];
        yield 'an enum at the decimal door' => [
            static fn () => new Constraints(Door::Decimal, enum: ['0.5']),
            '0.25',
            new Violation(Keyword::Enum, new Decimal('25', 2, false), 0, 4),
        ];
        yield 'a strict fractional multipleOf at an integer door' => [
            static fn () => new Constraints(Door::I32, multipleOf: '0.5'),
            '3',
            new Checked(3),
        ];
    }

    /**
     * Every declaration OpenAPI 3.2 allows is taken, and judges values: the core is the one
     * judge of a declaration, and it refuses only what cannot mean anything.
     *
     * @param \Closure(): Constraints $declare
     */
    #[DataProvider('openApiDeclarations')]
    public function testEveryDeclarationOpenApiAllowsIsTaken(
        \Closure $declare,
        string $input,
        Checked|Violation $want,
    ): void {
        $constraints = $declare();
        $verdict = $constraints->door === Door::Text
            ? Cast::checkedText($input, $constraints)
            : Cast::checked($input, NumFormat::invariant(), $constraints);
        self::assertEquals($want, $verdict);
    }

    /**
     * A number in a text `enum` is a valid entry that no string equals (JSON Schema's `enum`
     * is exact equality across types): declared, kept as given, and never matched.
     */
    public function testANumberInATextEnumNeverMatches(): void
    {
        $half = new Decimal('25', 1, false);
        $sex = new Constraints(Door::Text, enum: [1, 'One', $half, 'Two']);
        self::assertSame([1, 'One', $half, 'Two'], $sex->enum);
        self::assertSame("\0\0\0\0", substr($sex->packedEnum(), 0, 4));
        self::assertEquals(new Violation(Keyword::Enum, '1', 0, 1), Cast::checkedText('1', $sex));
        self::assertEquals(new Violation(Keyword::Enum, '2.5', 0, 3), Cast::checkedText('2.5', $sex));
        self::assertEquals(new Checked('One', false, 1), Cast::checkedText('one', $sex));
        self::assertEquals(new Checked('Two', false, 3), Cast::checkedText('TWO', $sex));
        // Control: the same entries as strings match.
        self::assertEquals(
            new Checked('1', false, 0),
            Cast::checkedText('1', new Constraints(Door::Text, enum: ['1', 'One']))
        );
    }

    /**
     * A text `enum` folds case unless `caseSensitive`, which compares byte for byte, for a
     * vocabulary whose case carries meaning.
     */
    public function testCaseSensitiveMatchesByteForByte(): void
    {
        $folded = new Constraints(Door::Text, enum: ['mm', 'Mm']);
        self::assertFalse($folded->caseSensitive);
        self::assertEquals(new Checked('mm', false, 0), Cast::checkedText('MM', $folded));
        self::assertEquals(new Checked('mm', false, 0), Cast::checkedText('Mm', $folded));
        $exact = new Constraints(Door::Text, caseSensitive: true, enum: ['mm', 'Mm']);
        self::assertTrue($exact->caseSensitive);
        self::assertEquals(new Checked('Mm', false, 1), Cast::checkedText('Mm', $exact));
        self::assertEquals(new Violation(Keyword::Enum, 'MM', 0, 2), Cast::checkedText('MM', $exact));
        self::assertEquals(
            new Violation(Keyword::Enum, 'MM', 0, 2),
            DetailedCast::checkedText('MM', $exact)
        );
    }

    /**
     * A violation carries the value exactly as the door cast it, in the door's own PHP type,
     * never the rounded one: the input is the type, and breaks the schema.
     */
    public function testAViolationCarriesTheValueAsCast(): void
    {
        $invariant = NumFormat::invariant();
        // A rounding past what the type holds is `multipleOf`'s violation, carrying the value.
        $ceiling = new Constraints(Door::I8, multipleOf: 5, rounding: Rounding::Ceiling);
        self::assertEquals(new Violation(Keyword::MultipleOf, 127, 0, 3), Cast::checked('127', $invariant, $ceiling));
        self::assertEquals(
            new Violation(Keyword::MultipleOf, 127, 0, 3),
            DetailedCast::checked('127', $invariant, $ceiling)
        );
        // Rounded to 10, then above the maximum: the value is the one cast, 8, not 10.
        $rounded = new Constraints(Door::I32, maximum: 9, multipleOf: 5, rounding: Rounding::Midpoint);
        self::assertEquals(new Violation(Keyword::Maximum, 8, 1, 1), Cast::checked(' 8', $invariant, $rounded));
        // A u64 as its bit pattern, as `Cast::u64` delivers it.
        $u64 = Cast::checked('18446744073709551615', $invariant, new Constraints(Door::U64, maximum: 1));
        self::assertInstanceOf(Violation::class, $u64);
        self::assertSame(-1, $u64->value);
        // A decimal as the `Decimal` cast, a float as the float, both from a decoded number too.
        self::assertEquals(
            new Violation(Keyword::Minimum, new Decimal('125', 2, false), 0, 4),
            Cast::checked('1.25', $invariant, new Constraints(Door::Decimal, minimum: 2))
        );
        self::assertEquals(
            new Violation(Keyword::Minimum, 1.5, 0, 0),
            Cast::checkedFromFloat(1.5, new Constraints(Door::F64, minimum: 2))
        );
        // The text door's value is the judged text, after `trim`, never an entry.
        self::assertEquals(
            new Violation(Keyword::MaxLength, 'abc', 2, 3),
            Cast::checkedText("  abc\t", new Constraints(Door::Text, maxLength: 2, trim: true))
        );
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
     * Bounds, `multipleOf` and numeric `enum` entries are an int, a string or a `Decimal`, and
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
                => 'Argument #12 ($enum) must hold only int, string or HyperCast\\Decimal entries; entry 1 is float',
            'new HyperCast\\Constraints(HyperCast\\Door::Text, enum: ["a", 1.0])'
                => 'Argument #12 ($enum) must hold only strings, or int or HyperCast\\Decimal entries, '
                    . 'which never match, at HyperCast\\Door::Text; entry 1 is float',
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
        self::assertEquals(new Violation(Keyword::Enum, 'Femal', 2, 5), $violation);
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
     * `Cast::optional()` takes a checked verdict as it takes a plain one: only an `Empty` fault
     * becomes `null`; a `Checked`, a `Violation` and any other fault come back as they were.
     */
    public function testOptionalTakesACheckedVerdictAndLetsAViolationThrough(): void
    {
        $sex = new Constraints(Door::Text, trim: true, enum: ['Female', 'Male']);
        self::assertNull(Cast::optional(Cast::checkedText('   ', $sex)));
        $violation = Cast::checkedText('x', $sex);
        self::assertSame($violation, Cast::optional($violation));
        self::assertEquals(new Violation(Keyword::Enum, 'x', 0, 1), Cast::optional($violation));
        $checked = Cast::checkedText('male', $sex);
        self::assertSame($checked, Cast::optional($checked));
        self::assertEquals(new Checked('Male', false, 1), Cast::optional($checked));
        $malformed = Cast::checkedText("\xFF", $sex);
        self::assertEquals(new Fault(CastFailure::Malformed, 0, 1), Cast::optional($malformed));
    }

    /**
     * A PHP string is bytes, so text that is not UTF-8 (a lone surrogate's three bytes, a stray
     * byte) reaches every door as it is, and the plain door's verdict is the checked door's, span
     * included: `Malformed` at its bytes, never a replacement standing for them.
     */
    public function testInvalidUtf8IsTheSameFaultAtPlainAndCheckedDoors(): void
    {
        $i32 = new Constraints(Door::I32);
        $format = new NumFormat('.', ',', NumFormat::ALL, "\u{FFFD}");
        foreach (["\xED\xA0\x805", "5\xED\xBF\xBF", "12\xFF3", "\xEF\xBF\xBD5\xC0", "\x80"] as $text) {
            $plain = Cast::i32($text, $format);
            self::assertInstanceOf(Fault::class, $plain, bin2hex($text));
            self::assertSame(CastFailure::Malformed, $plain->reason, bin2hex($text));
            self::assertEquals(Cast::checked($text, $format, $i32), $plain, bin2hex($text));
        }
        self::assertEquals(new Fault(CastFailure::Malformed, 0, 3), Cast::i32("\xED\xA0\x805", $format));
        $text = new Constraints(Door::Text);
        foreach (["\xED\xA0\x80", "a\xFFb", "\xC3"] as $bytes) {
            $plain = Cast::text($bytes);
            self::assertInstanceOf(Fault::class, $plain, bin2hex($bytes));
            self::assertEquals(Cast::checkedText($bytes, $text), $plain, bin2hex($bytes));
        }
        self::assertEquals(new Fault(CastFailure::Malformed, 1, 1), Cast::text("a\xFFb"));
    }

    /**
     * `Keyword` is open: a code this binding does not name yet is kept as the bare int, named
     * `keyword N`, and every named case's OpenAPI spelling is its name, first letter lowered.
     */
    public function testAKeywordThisBindingDoesNotNameIsKept(): void
    {
        $unknown = new Violation(9, 'x', 0, 1);
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
            self::assertSame($names[$i], (new Violation($keyword, null, 0, 0))->keywordName());
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
            new Violation(Keyword::MultipleOf, $sum, 0, 0),
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
        self::assertEquals(new Violation(Keyword::Enum, 'other', 0, 5), Cast::checkedText('other', $sex));

        $codes = [10, '20'];
        foreach ($codes as &$code) {
        }
        $tens = new Constraints(Door::I32, enum: $codes);
        $code = 30;
        unset($code);
        self::assertEquals([new Decimal('10', 0, false), new Decimal('20', 0, false)], $tens->enum);
        self::assertEquals(new Checked(20), Cast::checked('20', NumFormat::invariant(), $tens));
        self::assertEquals(
            new Violation(Keyword::Enum, 30, 0, 2),
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
            'a:1:{i:0;d:7.5;}' => 'enum must hold only strings, or int or HyperCast\\Decimal entries, '
                . 'which never match, at HyperCast\\Door::Text; entry 0 is float',
        ];
        $originals = ['s:9:"minLength";i:-1;' => 's:9:"minLength";N;', 'a:1:{i:0;d:7.5;}' => 'a:1:{i:0;s:1:"a";}'];
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
        self::assertEquals($built, new Violation(1, 9, 0, 1));
        self::assertSame(Keyword::Minimum, (new Violation(1, 9, 0, 1))->keyword);
        self::assertSame(9, (new Violation(9, 9, 1, 1))->keyword);
        self::assertSame(1, (new Violation(9, 9, 1, 1))->offset);
    }

    /**
     * `packed()` is the core's 104-byte `RawConstraints`, each of the four bounds in a slot of
     * its own, and `packedEnum()` the list beside it.
     */
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
                'size' => 104,
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
        $decimals = static fn (string $packed): array => array_map(
            static fn (int $at): array => unpack('Qlo/Lhi/Cscale/Cnegative', $packed, $at),
            [24, 40, 56, 72, 88]
        );
        $four = new Constraints(
            Door::Decimal,
            minimum: 1,
            exclusiveMinimum: 2,
            maximum: 3,
            exclusiveMaximum: 4,
            multipleOf: '0.5',
        );
        self::assertSame(1 | 2 | 4 | 8 | 16, unpack('L', $four->packed(), 4)[1]);
        self::assertSame(
            [
                ['lo' => 1, 'hi' => 0, 'scale' => 0, 'negative' => 0],
                ['lo' => 3, 'hi' => 0, 'scale' => 0, 'negative' => 0],
                ['lo' => 5, 'hi' => 0, 'scale' => 1, 'negative' => 0],
                ['lo' => 2, 'hi' => 0, 'scale' => 0, 'negative' => 0],
                ['lo' => 4, 'hi' => 0, 'scale' => 0, 'negative' => 0],
            ],
            $decimals($four->packed())
        );
        self::assertSame(
            1 | 4 | 128 | 256 | 512,
            unpack('L', (new Constraints(
                Door::Text,
                minimum: 0,
                maximum: 1,
                trim: true,
                caseSensitive: true,
                enum: ['a'],
            ))->packed(), 4)[1]
        );
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
