<?php

declare(strict_types=1);

namespace HyperCast;

/**
 * The declared unit of a Unix-epoch value — no magnitude guessing, ever. Values match the
 * native core's discriminants.
 */
enum UnixPrecision: int
{
    case Seconds = 1;
    case Milliseconds = 2;
    case Microseconds = 3;
    case Nanoseconds = 4;
}
