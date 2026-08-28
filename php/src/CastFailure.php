<?php

declare(strict_types=1);

namespace HyperCast;

/**
 * The closed set of reasons a cast can fail — the native core's verdict codes, verbatim,
 * as a real PHP backed enum. Adding a member is a deliberate breaking change: every match
 * over this enum must be updated, in every binding at once.
 */
enum CastFailure: int
{
    /** Required input was empty or whitespace. {@see Cast::optional()} surfaces this as null. */
    case Empty = 1;

    /** Input was present but not recognizable as the target type. */
    case Malformed = 2;

    /** Well-formed but outside the target's range — "256" for a u8, 1e400 for an f64. */
    case OutOfRange = 3;
}
