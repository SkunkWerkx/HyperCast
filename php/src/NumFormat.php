<?php

declare(strict_types=1);

namespace HyperCast;

/**
 * Caller-declared numeric notation for the integer and real doors. The native core carries
 * no culture data — a call site parsing culture-sensitive text declares its format out
 * loud ({@see NumFormat::invariant()}, or a constructed instance); there is no default,
 * the same stance every binding in this repo takes. Equal separators are a caller bug
 * (InvalidArgumentException), never a verdict.
 */
final readonly class NumFormat
{
    /** Permit the group separator between digits (sizes not validated — between digits is the rule). */
    public const GROUPING = 1;
    /** Permit accounting parentheses as negation: (1,234) is -1234. */
    public const PARENTHESES = 1 << 1;
    /** Permit exponent notation. Integer doors reject a negative exponent. */
    public const EXPONENT = 1 << 2;
    /** Permit 0x/&H/0b two's-complement radix prefixes (0xFF is -1 for an i8). */
    public const RADIX_PREFIXES = 1 << 3;
    /** Permit a trailing %, dividing by 100. Real doors only. */
    public const PERCENT = 1 << 4;
    /**
     * Resolve the ./, roles per input from structure instead of the declared separators
     * (which are ignored while this flag is set). Detection, not sniffing: a repeated
     * separator is grouping ("1.234.567,89"); with both present the rightmost is the
     * decimal; a single separator with a non-3-digit right run is the decimal ("3,1415");
     * with exactly 3 digits right, only a 0 integer part proves decimal ("0,785").
     * Genuinely ambiguous input ("12.185", "1,000") is a Malformed Fault at the
     * separator, never guessed.
     */
    public const SEPARATOR_DETECT = 1 << 5;
    /** Every lenience on (SEPARATOR_DETECT is a separator policy, deliberately excluded). */
    public const ALL = self::GROUPING | self::PARENTHESES | self::EXPONENT | self::RADIX_PREFIXES | self::PERCENT;

    /**
     * Declares a format: single-character separators, distinct from each other, plus the
     * lenience flags. A malformed format fails loudly as the caller bug it is.
     *
     * @param string $decimalSep the decimal separator, exactly one character
     * @param string $groupSep the group separator, exactly one character
     * @param int $flags the lenience flags (GROUPING | PARENTHESES | ...)
     */
    public function __construct(
        public string $decimalSep,
        public string $groupSep,
        public int $flags,
    ) {
        if (mb_strlen($decimalSep, 'UTF-8') !== 1 || mb_strlen($groupSep, 'UTF-8') !== 1) {
            throw new \InvalidArgumentException('Separators must be single characters');
        }
        if ($decimalSep === $groupSep) {
            throw new \InvalidArgumentException(
                "Decimal and group separators must differ; both are '{$decimalSep}'"
            );
        }
    }

    /**
     * The invariant profile — '.' decimal, ',' grouping, every lenience on.
     *
     * @return self the shared invariant instance
     */
    public static function invariant(): self
    {
        static $invariant = null;
        return $invariant ??= new self('.', ',', self::ALL);
    }

    /**
     * The detection profile — every lenience on, ./, roles resolved per input by
     * SEPARATOR_DETECT's structural rules.
     *
     * @return self the shared detection instance
     */
    public static function detect(): self
    {
        static $detect = null;
        return $detect ??= new self('.', ',', self::ALL | self::SEPARATOR_DETECT);
    }

    /**
     * The separators as Unicode code points, the core's own field encoding.
     *
     * @return array{int, int} decimal separator code point, then group separator
     */
    public function codePoints(): array
    {
        return [
            mb_ord($this->decimalSep, 'UTF-8'),
            mb_ord($this->groupSep, 'UTF-8'),
        ];
    }
}
