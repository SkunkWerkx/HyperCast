<?php

declare(strict_types=1);

namespace HyperCast;

/**
 * The failure case of a verdict: a closed conversion reason plus the offending span, as
 * byte offsets into the UTF-8 input (PHP strings are raw bytes — no mapping needed).
 * Nothing is captured — slicing the offending text out of the input is the caller's choice.
 */
final readonly class Fault
{
    public function __construct(
        public CastFailure $reason,
        public int $offset,
        public int $length,
    ) {
    }
}
