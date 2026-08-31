<?php

declare(strict_types=1);

namespace HyperCast;

/**
 * The caller-declared field order of a separated calendar date — no guessing, ever:
 * "1/7/2026" is January 7th (Mdy, the en-US order) or July 1st (Dmy, the en-GB order)
 * only because the caller said which. Values match the native core's discriminants.
 * PHP's stdlib carries no per-locale date-pattern source (that's ext-intl territory), so
 * unlike NumFormat there is no locale bridge here — declare the order the text actually
 * speaks.
 */
enum DateOrder: int
{
    case Ymd = 1;
    case Mdy = 2;
    case Dmy = 3;
}
