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
    /**
     * Carries the reason and span verbatim from the native core's fault out-param.
     *
     * @param CastFailure $reason the closed-set conversion reason
     * @param int $offset byte offset of the offending span in the input
     * @param int $length byte length of the offending span; zero for Empty
     */
    public function __construct(
        public CastFailure $reason,
        public int $offset,
        public int $length,
    ) {
    }
}
