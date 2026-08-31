<?php

declare(strict_types=1);

namespace HyperCast;

/**
 * A signed span of time as the protobuf pair — whole seconds plus same-signed nanoseconds.
 * PHP's DateInterval is calendar-shaped and sub-second-hostile, so the pair is the honest
 * carrier (the same call Go's binding makes); {@see toSeconds()} converts to float when
 * approximate seconds are enough.
 */
final readonly class Duration
{
    /**
     * Carries the pair verbatim from the native core — no normalization, no validation.
     *
     * @param int $seconds whole seconds, signed
     * @param int $nanos same-signed nanoseconds, |nanos| < 1e9
     */
    public function __construct(
        public int $seconds,
        public int $nanos,
    ) {
    }

    /**
     * Approximate float seconds — convenient, and lossy exactly the way floats are.
     *
     * @return float the span in seconds, to float precision
     */
    public function toSeconds(): float
    {
        return $this->seconds + $this->nanos / 1_000_000_000;
    }
}
