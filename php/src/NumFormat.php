<?php

declare(strict_types=1);

namespace HyperCast;

/**
 * Caller-declared numeric notation for the integer, real and decimal doors. The native core
 * carries no culture data — a call site parsing culture-sensitive text declares its format
 * out loud ({@see NumFormat::invariant()}, or a constructed instance); there is no default,
 * the same stance every binding in this repo takes. Equal separators, or a currency symbol
 * the core cannot carry, are a caller bug (InvalidArgumentException), never a verdict.
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
    /**
     * Permit the declared currency symbol once, leading ("$5", "-$5", "$ -5") or trailing
     * ("5 €", "1.234,50 kr."), with optional ASCII whitespace between it and the digits;
     * accounting parentheses wrap symbol and digits together ("($5)"). With no symbol
     * declared the flag matches nothing. A symbol declared without this flag is Malformed
     * at the symbol, never silently ignored.
     */
    public const CURRENCY = 1 << 6;
    /** Every lenience on (SEPARATOR_DETECT is a separator policy, deliberately excluded). */
    public const ALL = self::GROUPING | self::PARENTHESES | self::EXPONENT | self::RADIX_PREFIXES | self::PERCENT
        | self::CURRENCY;

    /** The inline capacity the core gives a currency symbol, in UTF-8 bytes. */
    public const CURRENCY_MAX_BYTES = 16;

    /**
     * Declares a format: single-character separators, distinct from each other, the
     * lenience flags, and optionally the currency symbol CURRENCY honors — up to 16 bytes
     * of UTF-8 ("$", "€", "kr.", "CHF", "R$", "руб.") with no ASCII digit or whitespace,
     * since those would collide with the digit scan and the trimming around the symbol.
     * A malformed format fails loudly as the caller bug it is.
     *
     * @param string $decimalSep the decimal separator, exactly one character
     * @param string $groupSep the group separator, exactly one character
     * @param int $flags the lenience flags (GROUPING | PARENTHESES | ...)
     * @param string $currency the currency symbol; '' (the default) declares none
     */
    public function __construct(
        public string $decimalSep,
        public string $groupSep,
        public int $flags,
        public string $currency = '',
    ) {
        if (mb_strlen($decimalSep, 'UTF-8') !== 1 || mb_strlen($groupSep, 'UTF-8') !== 1) {
            throw new \InvalidArgumentException('Separators must be single characters');
        }
        if ($decimalSep === $groupSep) {
            throw new \InvalidArgumentException(
                "Decimal and group separators must differ; both are '{$decimalSep}'"
            );
        }
        if ($currency !== '') {
            if (\strlen($currency) > self::CURRENCY_MAX_BYTES || !mb_check_encoding($currency, 'UTF-8')) {
                throw new \InvalidArgumentException(
                    'Currency symbol must be at most ' . self::CURRENCY_MAX_BYTES . ' bytes of UTF-8'
                );
            }
            // The core's own rule: ASCII digits and ASCII whitespace (space, \t, \n, \f, \r)
            // are the scan's and the trim's business, so a symbol may not carry them.
            if (strpbrk($currency, "0123456789 \t\n\f\r") !== false) {
                throw new \InvalidArgumentException(
                    "Currency symbol must not contain an ASCII digit or whitespace; got '{$currency}'"
                );
            }
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
     * Declares the notation the platform's locale data describes — the counterpart of the
     * C# binding's `NumFormat.From(CultureInfo)` and Python's `from_localeconv`. Reads
     * `decimal_point`, `thousands_sep` and `currency_symbol` from `$conv`, or from
     * {@see localeconv()} when null, defaulting to '.', ',' and no symbol wherever the field
     * is empty (the C locale reports an empty thousands separator), with every lenience on.
     *
     * PHP's `localeconv()` reflects `setlocale(LC_NUMERIC | LC_MONETARY)` *process* state —
     * shared across every request in the worker, and nothing this library controls — so a
     * caller that knows its notation should declare it explicitly through the constructor;
     * this factory is for the caller that genuinely wants "whatever the process locale
     * says", the same stance the C# doc takes on `CultureInfo.CurrentCulture`.
     *
     * @param array<string, mixed>|null $conv a localeconv()-shaped array, or null to read localeconv()
     * @return self the format the locale data describes, every lenience on
     */
    public static function fromLocaleconv(?array $conv = null): self
    {
        $conv ??= localeconv();
        $decimalSep = $conv['decimal_point'] ?? '';
        $groupSep = $conv['thousands_sep'] ?? '';
        $currency = $conv['currency_symbol'] ?? '';
        return new self(
            $decimalSep === '' ? '.' : $decimalSep,
            $groupSep === '' ? ',' : $groupSep,
            self::ALL,
            \is_string($currency) ? $currency : ''
        );
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
