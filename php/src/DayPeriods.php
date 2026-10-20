<?php

declare(strict_types=1);

namespace HyperCast;

/**
 * Caller-declared day-period designators for the datetime door — the words a culture
 * writes for the two halves of a 12-hour clock ('vorm.'/'nachm.', '오전'/'오후',
 * 'a. m.'/'p. m.'), declared beside the {@see DateOrder} the same way, and read beside the
 * English AM/PM, which are always read: a declared designator is matched byte for byte,
 * after the clock ("3:04 nachm.", an optional space before it) or ahead of it ("오후 3:04",
 * an optional space after), where AM/PM match ASCII case-insensitively in the same two
 * places. Nothing declared — the door's default — is exactly the English pair. Each
 * designator is up to 16 bytes of UTF-8 with no ASCII digit and no ASCII whitespace at
 * either end (the grammar around it reads that; an internal space is fine, as in Spanish's
 * 'a. m.'); anything else is a caller bug (InvalidArgumentException), never a verdict.
 * Like DateOrder there is no locale bridge: PHP's stdlib carries no day-period data, and
 * ext-intl's IntlDateFormatter formats one without exposing it as a symbol — declare the
 * pair the text actually speaks.
 */
final readonly class DayPeriods
{
    /** The inline capacity the core gives a designator, in UTF-8 bytes. */
    public const MAX_BYTES = 16;
    /** The size of the packed layout {@see packed()} builds and the core reads. */
    public const PACKED_BYTES = 40;

    /**
     * Declares both designators. A malformed one fails loudly as the caller bug it is.
     *
     * @param string $am the first half's designator ('vorm.', '오전', 'a. m.')
     * @param string $pm the second half's designator ('nachm.', '오후', 'p. m.')
     */
    public function __construct(
        public string $am,
        public string $pm,
    ) {
        self::checkDesignator($am, 'AM designator');
        self::checkDesignator($pm, 'PM designator');
    }

    /**
     * The designators in the layout the core reads: 40 bytes — a size word, then each
     * designator as its bytes zero-padded to 16 followed by its length, then two bytes of
     * padding. What {@see \HyperCast\Interop\NativeValues::writeDayPeriods()} copies into
     * the native scratch. Built from fields the constructor has already validated.
     *
     * @return string the 40 packed bytes
     */
    public function packed(): string
    {
        return pack('V', self::PACKED_BYTES)
            . str_pad($this->am, self::MAX_BYTES, "\0") . \chr(\strlen($this->am))
            . str_pad($this->pm, self::MAX_BYTES, "\0") . \chr(\strlen($this->pm))
            . "\0\0";
    }

    /**
     * The core's rules for one designator, which it checks again on every cast:
     * well-formed UTF-8 of one to 16 bytes, no ASCII digit, and no ASCII whitespace (space,
     * \t, \n, \f, \r) as its first or last byte. PCRE's /u mode is the UTF-8 authority, as
     * in NumFormat; it fails the match outright on ill-formed UTF-8.
     *
     * @param string $designator the designator to check
     * @param string $role the designator's name in the exception
     * @return void
     * @throws \InvalidArgumentException When the core could not carry the designator.
     */
    private static function checkDesignator(string $designator, string $role): void
    {
        if ($designator === '' || \strlen($designator) > self::MAX_BYTES || preg_match('//u', $designator) !== 1) {
            throw new \InvalidArgumentException("{$role} must be one to " . self::MAX_BYTES . ' bytes of UTF-8');
        }
        if (strpbrk($designator, '0123456789') !== false) {
            throw new \InvalidArgumentException("{$role} must not contain an ASCII digit; got '{$designator}'");
        }
        if (trim($designator, " \t\n\f\r") !== $designator) {
            throw new \InvalidArgumentException(
                "{$role} must not start or end with ASCII whitespace; got '{$designator}'"
            );
        }
    }
}
