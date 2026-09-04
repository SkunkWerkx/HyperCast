<?php

declare(strict_types=1);

namespace HyperCast;

/**
 * An exact decimal as the core's triple — sign, 96-bit magnitude, base-10 scale — the shape
 * .NET's `decimal` stores natively. PHP has no decimal type of its own and `int` is signed
 * 64-bit, so the magnitude rides as a decimal digit string (rendered from the core's two
 * limbs with integer arithmetic — no float ever touches it) and the scale is canonical:
 * exact trailing zeros in the fraction are trimmed by the core, so "1.10", "1.1" and
 * "1.1000" are all magnitude 11 at scale 1, zero is always scale 0, and {@see __toString()}
 * renders "1.1" for every one of them. Zero dependencies — bcmath, GMP or ext-decimal can
 * all consume the string form when arithmetic is wanted; {@see toFloat()} is the lossy
 * convenience.
 */
final readonly class Decimal implements \Stringable
{
    /**
     * Carries the triple verbatim from the native core — no normalization, no validation.
     *
     * @param string $magnitude the unsigned magnitude as decimal digits, at most 2^96 - 1
     * @param int $scale base-10 places the magnitude is shifted right, 0..=28
     * @param bool $negative true for a negative value; zero is never negative
     */
    public function __construct(
        public string $magnitude,
        public int $scale,
        public bool $negative,
    ) {
    }

    /**
     * Builds from the core's out-struct fields: the 96-bit magnitude as its low 64 and high
     * 32 bits. `$lo` arrives as PHP's signed int carrying the unsigned bit pattern, exactly
     * as the u64 door presents it, and is rendered without ever going through float.
     *
     * @param int $lo the magnitude's low 64 bits (unsigned bit pattern in a signed int)
     * @param int $hi the magnitude's high 32 bits
     * @param int $scale base-10 places the magnitude is shifted right, 0..=28
     * @param bool $negative true for a negative value
     * @return self the decimal carrying the triple
     */
    public static function fromLimbs(int $lo, int $hi, int $scale, bool $negative): self
    {
        if ($hi === 0 && $lo >= 0) {
            return new self((string) $lo, $scale, $negative);
        }
        // Three 32-bit limbs, most significant first; repeated division by 10^9 peels off
        // nine digits per pass. Each step's accumulator is below 2^62, so it stays inside
        // PHP's signed 64-bit int.
        $limbs = [$hi, ($lo >> 32) & 0xFFFFFFFF, $lo & 0xFFFFFFFF];
        $chunks = [];
        while ($limbs[0] !== 0 || $limbs[1] !== 0 || $limbs[2] !== 0) {
            $remainder = 0;
            foreach ($limbs as $i => $limb) {
                $accumulator = ($remainder << 32) | $limb;
                $limbs[$i] = intdiv($accumulator, 1_000_000_000);
                $remainder = $accumulator % 1_000_000_000;
            }
            $chunks[] = $remainder;
        }
        $digits = (string) array_pop($chunks);
        while ($chunks !== []) {
            $digits .= sprintf('%09d', array_pop($chunks));
        }
        return new self($digits, $scale, $negative);
    }

    /**
     * The canonical text — sign, whole part, then exactly `$scale` fraction digits
     * ("1234.5", "-0.025", "0") — the same rendering the shared corpus pins as `value`.
     * The core's scale is already minimal, so nothing is trimmed here.
     *
     * @return string the canonical decimal text
     */
    public function __toString(): string
    {
        $digits = $this->magnitude;
        if ($this->scale === 0) {
            return $this->negative ? "-{$digits}" : $digits;
        }
        if (\strlen($digits) <= $this->scale) {
            $digits = str_pad($digits, $this->scale + 1, '0', STR_PAD_LEFT);
        }
        $split = \strlen($digits) - $this->scale;
        $text = substr($digits, 0, $split) . '.' . substr($digits, $split);
        return $this->negative ? "-{$text}" : $text;
    }

    /**
     * Approximate float — convenient, and lossy exactly the way floats are.
     *
     * @return float the value to double precision
     */
    public function toFloat(): float
    {
        return (float) $this->__toString();
    }
}
