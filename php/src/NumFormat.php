<?php

declare(strict_types=1);

namespace HyperCast;

/**
 * Caller-declared numeric notation for the integer, real and decimal doors. The native core
 * carries no culture data — a call site parsing culture-sensitive text declares its format
 * out loud ({@see NumFormat::invariant()}, a constructed instance, or one of the locale
 * factories); there is no default, the same stance every binding in this repo takes.
 *
 * The two number separators are required; everything else is optional, and every optional
 * symbol is a further spelling read *beside* the ASCII one, which is always read: declaring
 * U+2212 as the minus sign does not stop '-' parsing, declaring '٠' as the digit zero does
 * not stop '0', so nothing a format parsed before stops parsing when a symbol is added. A
 * symbol the core cannot carry — equal separators, an ASCII digit inside a symbol,
 * whitespace in a sign — is a caller bug (InvalidArgumentException), never a verdict.
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
    /**
     * Permit a trailing %, dividing by 100, and the declared percent symbol beside it. Real
     * and decimal doors only; an integer door never reads a percent.
     */
    public const PERCENT = 1 << 4;
    /**
     * Resolve the ./, roles per input from structure instead of the declared separators
     * (which are ignored while this flag is set, the currency separators included).
     * Detection, not sniffing: a repeated separator is grouping ("1.234.567,89"); with
     * both present the rightmost is the decimal; a single separator with a non-3-digit
     * right run is the decimal ("3,1415"); with exactly 3 digits right, only a 0 integer
     * part proves decimal ("0,785"). Genuinely ambiguous input ("12.185", "1,000") is a
     * Malformed Fault at the separator, never guessed.
     */
    public const SEPARATOR_DETECT = 1 << 5;
    /**
     * Permit the declared currency symbol once, leading ("$5", "-$5", "$ -5") or trailing
     * ("5 €", "1.234,50 kr."), with optional ASCII whitespace between it and the digits;
     * accounting parentheses wrap symbol and digits together ("($5)"). With no symbol
     * declared the flag matches nothing. A symbol declared without this flag is Malformed
     * at the symbol, never silently ignored. When the symbol is found, the digits are read
     * with the currency separators where those are declared.
     */
    public const CURRENCY = 1 << 6;
    /**
     * Permit a sign after the digits — "5-", "5 -", "5+", "5 €-", "5- €", "2.5%-" — the way
     * some cultures write a negative amount, the declared signs read beside ASCII. Never
     * beside a leading sign ("-5-" is Malformed at the second) nor inside parentheses.
     * Deliberately not part of ALL: the locale factories set it from the locale's own
     * negative pattern, and a hand-built format declares it when its text speaks that way.
     */
    public const TRAILING_SIGN = 1 << 7;
    /**
     * Every lenience on (SEPARATOR_DETECT is a separator policy and TRAILING_SIGN a
     * culture's negative pattern, both deliberately excluded).
     */
    public const ALL = self::GROUPING | self::PARENTHESES | self::EXPONENT | self::RADIX_PREFIXES | self::PERCENT
        | self::CURRENCY;

    /** The inline capacity the core gives a separator, a sign or the percent symbol, in UTF-8 bytes. */
    public const SYMBOL_MAX_BYTES = 8;
    /** The inline capacity the core gives a currency symbol, in UTF-8 bytes. */
    public const CURRENCY_MAX_BYTES = 16;
    /** The size of the packed layout {@see packed()} builds and the core reads. */
    public const PACKED_BYTES = 92;

    /**
     * Declares a format: the two number separators, distinct from each other and each one
     * or two characters (up to 8 bytes of UTF-8 — ", " and "' " are separators real
     * cultures use), the lenience flags, and optionally every further symbol the text may
     * carry. Each optional symbol declares nothing when '' (the default), and a declared one
     * is read beside its ASCII counterpart, never instead of it. The core's rules, enforced
     * here so that a bad format fails loudly as the caller bug it is: no symbol contains an
     * ASCII digit (a digit is a digit before it is anything else); a sign and the percent
     * symbol contain no ASCII whitespace (they sit flush against the digits); the currency
     * symbol, up to 16 bytes ("$", "€", "kr.", "CHF", "R$", "руб."), contains neither; the
     * two currency separators in force — each declared one, else the number separator it
     * stands in for — differ from each other; and the digit zero is one non-ASCII character
     * that starts a Unicode digit block.
     *
     * @param string $decimalSep the decimal separator, one or two characters
     * @param string $groupSep the group separator, one or two characters
     * @param int $flags the lenience flags (GROUPING | PARENTHESES | ...)
     * @param string $currency the currency symbol; '' (the default) declares none
     * @param string $currencyDecimalSep the decimal separator of a currency amount; '' keeps $decimalSep
     * @param string $currencyGroupSep the group separator of a currency amount; '' keeps $groupSep
     * @param string $minusSign a further spelling of the minus sign (U+2212, say); '' declares none beside '-'
     * @param string $plusSign a further spelling of the plus sign; '' declares none beside '+'
     * @param string $percent a further spelling of the percent symbol ('٪'); '' declares none beside '%'
     * @param string $digitZero the zero of a set of native digits read beside 0-9 ('٠' for the
     *        Arabic-Indic digits, '０' for the fullwidth ones); '' declares none
     */
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
    ) {
        self::checkSymbol($decimalSep, 'Decimal separator', self::SYMBOL_MAX_BYTES, false);
        self::checkSymbol($groupSep, 'Group separator', self::SYMBOL_MAX_BYTES, false);
        if ($decimalSep === $groupSep) {
            throw new \InvalidArgumentException(
                "Decimal and group separators must differ; both are '{$decimalSep}'"
            );
        }
        if ($currencyDecimalSep !== '') {
            self::checkSymbol($currencyDecimalSep, 'Currency decimal separator', self::SYMBOL_MAX_BYTES, false);
        }
        if ($currencyGroupSep !== '') {
            self::checkSymbol($currencyGroupSep, 'Currency group separator', self::SYMBOL_MAX_BYTES, false);
        }
        $currencyDecimal = $currencyDecimalSep === '' ? $decimalSep : $currencyDecimalSep;
        $currencyGroup = $currencyGroupSep === '' ? $groupSep : $currencyGroupSep;
        if ($currencyDecimal === $currencyGroup) {
            throw new \InvalidArgumentException(
                "Currency decimal and group separators must differ; both are '{$currencyDecimal}'"
            );
        }
        if ($minusSign !== '') {
            self::checkSymbol($minusSign, 'Minus sign', self::SYMBOL_MAX_BYTES, true);
        }
        if ($plusSign !== '') {
            self::checkSymbol($plusSign, 'Plus sign', self::SYMBOL_MAX_BYTES, true);
        }
        if ($percent !== '') {
            self::checkSymbol($percent, 'Percent symbol', self::SYMBOL_MAX_BYTES, true);
        }
        if ($currency !== '') {
            self::checkSymbol($currency, 'Currency symbol', self::CURRENCY_MAX_BYTES, true);
        }
        if ($digitZero !== '') {
            // PCRE's /u mode is the UTF-8 authority here rather than ext-mbstring: PCRE is part
            // of every PHP build, so the package's only extension requirement stays ext-ffi.
            // /u fails the match outright on ill-formed UTF-8, so "exactly one character" also
            // means "exactly one well-formed code point".
            if (preg_match('/\A.\z/us', $digitZero) !== 1) {
                throw new \InvalidArgumentException('Digit zero must be exactly one character');
            }
            // The core's rule: every Unicode digit block starts at a code point whose ten
            // successors share its UTF-8 prefix (its value modulo 64 is at most 54), which is
            // what lets a digit be told from its last byte; and ASCII digits are read already.
            $code = self::codePoint($digitZero);
            if ($code < 0x80 || $code % 64 > 54) {
                throw new \InvalidArgumentException(
                    "Digit zero must start a Unicode digit block ('٠', '०', '０'); got '{$digitZero}'"
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
     * An empty separator never collides with the declared one: a comma-decimal locale that
     * reports no thousands separator gets '.' for grouping rather than a second ',' (and,
     * symmetrically, an empty decimal point beside a '.' group becomes ',').
     *
     * The rest of the locale's notation carries over too: `mon_decimal_point` and
     * `mon_thousands_sep` become the currency separators where they differ from the number
     * ones (a currency amount is then read with them, as the locale writes it);
     * `negative_sign` and `positive_sign` are declared as further spellings of the signs
     * where they are not the ASCII ones (U+2212 for Estonian and Basque, for instance), the
     * ASCII signs staying readable beside them; and an `n_sign_posn` of 2 or 4 — the sign
     * after the amount — sets TRAILING_SIGN. `localeconv()` carries no percent symbol and no
     * native digits, so those stay undeclared.
     *
     * PHP's `localeconv()` reflects `setlocale(LC_NUMERIC | LC_MONETARY)` *process* state —
     * shared across every request in the worker, and nothing this library controls — so a
     * caller that knows its notation should declare it explicitly through the constructor;
     * this factory is for the caller that genuinely wants "whatever the process locale
     * says", the same stance the C# doc takes on `CultureInfo.CurrentCulture`.
     *
     * Every lenience is on, radix prefixes and percent included; narrow the flags, or
     * construct a NumFormat outright, for anything stricter.
     *
     * @param array<string, mixed>|null $conv a localeconv()-shaped array, or null to read localeconv()
     * @return self the format the locale data describes, every lenience on
     */
    public static function fromLocaleconv(?array $conv = null): self
    {
        $conv ??= localeconv();
        $text = static fn (string $key): string => \is_string($conv[$key] ?? null) ? $conv[$key] : '';
        $signPosition = $conv['n_sign_posn'] ?? null;
        return self::fromSymbols(
            $text('decimal_point'),
            $text('thousands_sep'),
            $text('currency_symbol'),
            $text('mon_decimal_point'),
            $text('mon_thousands_sep'),
            $text('negative_sign'),
            $text('positive_sign'),
            '',
            '',
            $signPosition === 2 || $signPosition === 4
        );
    }

    /**
     * Declares the notation an intl `NumberFormatter` describes — PHP's per-object locale
     * formatting, where {@see fromLocaleconv()} reads process-global state. Reads the
     * formatter's decimal and grouping separators and its currency symbol, with every
     * lenience on, and fills an empty field the way {@see fromLocaleconv()} does. Every
     * numeric door also takes a `NumberFormatter` in place of a `NumFormat` and calls this.
     *
     * The rest of ICU's data for the locale carries over too: the monetary separators
     * (`MONETARY_SEPARATOR_SYMBOL`, `MONETARY_GROUPING_SEPARATOR_SYMBOL`) become the
     * currency separators where they differ from the number ones — Austrian German groups
     * a number with a no-break space and an amount with '.'; the minus, plus and percent
     * symbols (`MINUS_SIGN_SYMBOL`, `PLUS_SIGN_SYMBOL`, `PERCENT_SYMBOL`) are declared as
     * further spellings where they are not the ASCII ones — U+2212 for Swedish, Finnish
     * and Estonian, a bidi mark and a hyphen for Arabic and Hebrew — the ASCII ones staying
     * readable beside them; the zero digit (`ZERO_DIGIT_SYMBOL`) declares the locale's
     * native digits where it is not '0' — '٠' for Egyptian Arabic, '۰' for Persian; and a
     * negative pattern with a suffix and no prefix (`NEGATIVE_SUFFIX`, `NEGATIVE_PREFIX`:
     * `#,##0.###;#,##0.###-`) sets TRAILING_SIGN.
     *
     * A formatter's symbols are whatever ICU's data for its locale says, so a symbol the
     * core cannot carry (a separator of more than 8 bytes, say, or a currency symbol of
     * more than 16) is an InvalidArgumentException here exactly as it would be through the
     * constructor. Requires ext-intl; nothing else in this package does.
     *
     * Every lenience is on, radix prefixes and percent included; narrow the flags, or
     * construct a NumFormat outright, for anything stricter.
     *
     * @param \NumberFormatter $formatter the formatter whose symbols declare the notation
     * @return self the format the formatter describes, every lenience on
     * @throws \InvalidArgumentException When the formatter's symbols cannot be declared.
     */
    public static function fromNumberFormatter(\NumberFormatter $formatter): self
    {
        $symbol = static function (int $which) use ($formatter): string {
            $text = $formatter->getSymbol($which);
            return \is_string($text) ? $text : '';
        };
        $negativePrefix = $formatter->getTextAttribute(\NumberFormatter::NEGATIVE_PREFIX);
        $negativeSuffix = $formatter->getTextAttribute(\NumberFormatter::NEGATIVE_SUFFIX);
        $trailingSign = \is_string($negativeSuffix) && $negativeSuffix !== ''
            && ($negativePrefix === '' || $negativePrefix === false);
        return self::fromSymbols(
            $symbol(\NumberFormatter::DECIMAL_SEPARATOR_SYMBOL),
            $symbol(\NumberFormatter::GROUPING_SEPARATOR_SYMBOL),
            $symbol(\NumberFormatter::CURRENCY_SYMBOL),
            $symbol(\NumberFormatter::MONETARY_SEPARATOR_SYMBOL),
            $symbol(\NumberFormatter::MONETARY_GROUPING_SEPARATOR_SYMBOL),
            $symbol(\NumberFormatter::MINUS_SIGN_SYMBOL),
            $symbol(\NumberFormatter::PLUS_SIGN_SYMBOL),
            $symbol(\NumberFormatter::PERCENT_SYMBOL),
            $symbol(\NumberFormatter::ZERO_DIGIT_SYMBOL),
            $trailingSign
        );
    }

    /**
     * The shared tail of both locale factories. An empty number separator takes its
     * invariant default unless the other separator already holds that character, in which
     * case it takes the other of the ./, pair. A symbol that is its ASCII default ('-',
     * '+', '%', '0') or, for a currency separator, the number separator it would stand in
     * for, declares nothing: the core reads those anyway, and a format with nothing further
     * declared takes the core's shortest check. A currency separator left empty beside one
     * that collides with it takes the other of the ./, pair, the number pair's own rule.
     * Every lenience is on.
     *
     * @param string $decimalSep the locale's decimal separator, or '' for none
     * @param string $groupSep the locale's group separator, or '' for none
     * @param string $currency the locale's currency symbol, or '' for none
     * @param string $currencyDecimalSep the locale's monetary decimal separator, or '' for none
     * @param string $currencyGroupSep the locale's monetary group separator, or '' for none
     * @param string $minusSign the locale's minus sign, or '' for none
     * @param string $plusSign the locale's plus sign, or '' for none
     * @param string $percent the locale's percent symbol, or '' for none
     * @param string $digitZero the locale's zero digit, or '' for none
     * @param bool $trailingSign whether the locale writes the sign after a negative amount
     * @return self the declared format
     */
    private static function fromSymbols(
        string $decimalSep,
        string $groupSep,
        string $currency,
        string $currencyDecimalSep,
        string $currencyGroupSep,
        string $minusSign,
        string $plusSign,
        string $percent,
        string $digitZero,
        bool $trailingSign,
    ): self {
        if ($decimalSep === '') {
            $decimalSep = $groupSep === '.' ? ',' : '.';
        }
        if ($groupSep === '') {
            $groupSep = $decimalSep === ',' ? '.' : ',';
        }
        if ($currencyDecimalSep === $decimalSep) {
            $currencyDecimalSep = '';
        }
        if ($currencyGroupSep === $groupSep) {
            $currencyGroupSep = '';
        }
        if ($currencyDecimalSep === '' && $currencyGroupSep === $decimalSep) {
            $currencyDecimalSep = $currencyGroupSep === ',' ? '.' : ',';
        } elseif ($currencyGroupSep === '' && $currencyDecimalSep === $groupSep) {
            $currencyGroupSep = $currencyDecimalSep === ',' ? '.' : ',';
        }
        return new self(
            $decimalSep,
            $groupSep,
            self::ALL | ($trailingSign ? self::TRAILING_SIGN : 0),
            $currency,
            $currencyDecimalSep,
            $currencyGroupSep,
            $minusSign === '-' ? '' : $minusSign,
            $plusSign === '+' ? '' : $plusSign,
            $percent === '%' ? '' : $percent,
            $digitZero === '0' ? '' : $digitZero,
        );
    }

    /**
     * The format in the layout the core reads: 92 bytes — a size word, the flags, the digit
     * zero's code point, then each symbol as its bytes zero-padded to its capacity followed
     * by its length (the seven 8-byte symbols, then the 16-byte currency symbol), all
     * little-endian. What {@see \HyperCast\Interop\NativeValues::writeFormat()} copies
     * into the native scratch, and what a host carrying this format across a C ABI of its
     * own hands the core. Built from fields the constructor has already validated.
     *
     * @return string the 92 packed bytes
     */
    public function packed(): string
    {
        $digitZero = $this->digitZero === '' ? 0 : self::codePoint($this->digitZero);
        return pack('V3', self::PACKED_BYTES, $this->flags, $digitZero)
            . self::field($this->decimalSep, self::SYMBOL_MAX_BYTES)
            . self::field($this->groupSep, self::SYMBOL_MAX_BYTES)
            . self::field($this->currencyDecimalSep, self::SYMBOL_MAX_BYTES)
            . self::field($this->currencyGroupSep, self::SYMBOL_MAX_BYTES)
            . self::field($this->minusSign, self::SYMBOL_MAX_BYTES)
            . self::field($this->plusSign, self::SYMBOL_MAX_BYTES)
            . self::field($this->percent, self::SYMBOL_MAX_BYTES)
            . self::field($this->currency, self::CURRENCY_MAX_BYTES);
    }

    /**
     * One symbol in the layout's shape: its bytes zero-padded to the field's capacity,
     * then its length — the shape {@see DayPeriods::packed()} gives a designator too.
     *
     * @param string $symbol the symbol's UTF-8, at most $capacity bytes; '' for none
     * @param int $capacity the field's capacity in bytes
     * @return string the capacity + 1 bytes
     */
    private static function field(string $symbol, int $capacity): string
    {
        return str_pad($symbol, $capacity, "\0") . \chr(\strlen($symbol));
    }

    /**
     * The core's rules for one symbol, which it checks again on every cast: well-formed
     * UTF-8 of one to $maxBytes bytes with no ASCII digit, and — for a sign, the percent
     * symbol and the currency symbol, which sit flush against the digits or are trimmed
     * around — no ASCII whitespace (space, \t, \n, \f, \r). PCRE's /u mode is the UTF-8
     * authority (see the constructor); it fails the match outright on ill-formed UTF-8.
     *
     * @param string $symbol the symbol to check
     * @param string $role the symbol's name in the exception
     * @param int $maxBytes the field's capacity in bytes
     * @param bool $flush whether ASCII whitespace is refused too
     * @return void
     * @throws \InvalidArgumentException When the core could not carry the symbol.
     */
    private static function checkSymbol(string $symbol, string $role, int $maxBytes, bool $flush): void
    {
        if ($symbol === '' || \strlen($symbol) > $maxBytes || preg_match('//u', $symbol) !== 1) {
            throw new \InvalidArgumentException("{$role} must be one to {$maxBytes} bytes of UTF-8");
        }
        if (strpbrk($symbol, $flush ? "0123456789 \t\n\f\r" : '0123456789') !== false) {
            throw new \InvalidArgumentException(
                $flush
                    ? "{$role} must not contain an ASCII digit or whitespace; got '{$symbol}'"
                    : "{$role} must not contain an ASCII digit; got '{$symbol}'"
            );
        }
    }

    /**
     * Decodes one UTF-8 character to its code point. The constructor has already proven
     * the string is exactly one well-formed character, so the byte length alone names the
     * encoding form and no validation is repeated here.
     *
     * @param string $char exactly one well-formed UTF-8 character
     * @return int its Unicode code point
     */
    private static function codePoint(string $char): int
    {
        $lead = \ord($char[0]);
        return match (\strlen($char)) {
            1 => $lead,
            2 => ($lead & 0x1F) << 6 | (\ord($char[1]) & 0x3F),
            3 => ($lead & 0x0F) << 12 | (\ord($char[1]) & 0x3F) << 6 | (\ord($char[2]) & 0x3F),
            4 => ($lead & 0x07) << 18 | (\ord($char[1]) & 0x3F) << 12 | (\ord($char[2]) & 0x3F) << 6
                | (\ord($char[3]) & 0x3F),
        };
    }
}
