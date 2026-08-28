<?php

declare(strict_types=1);

namespace HyperCast;

/**
 * The success case of a verdict: a cast value. The verdict union is PHP's own idiom —
 * a `Success|Fault` union type consumed with `instanceof` or `match(true)`; PHP has no
 * compiler-checked exhaustiveness, so the closed pair is enforced by both classes being
 * final and every door's return type declaring the union.
 */
final readonly class Success
{
    public function __construct(public mixed $value)
    {
    }
}
