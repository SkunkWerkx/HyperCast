<?php

declare(strict_types=1);

namespace HyperCast\Tests\Support;

/**
 * A NumberFormatter that answers `getSymbol()` and `getTextAttribute()` from a recording
 * instead of from ICU, so a formatter recorded on one machine reads the same on another,
 * whatever its ICU version. Needs ext-intl, as any NumberFormatter does.
 */
final class RecordedFormatter extends \NumberFormatter
{
    /** The symbols a formatter is asked for, as `getSymbol()` numbers them. */
    public const SYMBOLS = [0, 1, 3, 4, 6, 7, 8, 10, 17];

    /** The text attributes it is asked for: NEGATIVE_PREFIX and NEGATIVE_SUFFIX. */
    public const ATTRIBUTES = [2, 3];

    /** What an answer the recording leaves out is: en_US's. */
    private const DEFAULT_SYMBOLS = [0 => '.', 1 => ',', 3 => '%', 4 => '0', 6 => '-', 7 => '+', 8 => '$', 10 => '.',
        17 => ','];
    private const DEFAULT_ATTRIBUTES = [2 => '-', 3 => ''];

    /** @var array<int, string|false> */
    private array $symbols = self::DEFAULT_SYMBOLS;

    /** @var array<int, string|false> */
    private array $attributes = self::DEFAULT_ATTRIBUTES;

    private string $sampleText = '-1,234.5';

    /**
     * What a real formatter answers, and the text it formats -1234.5 as.
     *
     * @return array{symbols: array<int, string|false>, attributes: array<int, string|false>, sample: string}
     */
    public static function record(\NumberFormatter $formatter): array
    {
        $symbols = [];
        foreach (self::SYMBOLS as $symbol) {
            $symbols[$symbol] = $formatter->getSymbol($symbol);
        }
        $attributes = [];
        foreach (self::ATTRIBUTES as $attribute) {
            $attributes[$attribute] = $formatter->getTextAttribute($attribute);
        }
        return ['symbols' => $symbols, 'attributes' => $attributes, 'sample' => (string) $formatter->format(-1234.5)];
    }

    /**
     * A formatter answering `$answers`, en_US's answer for anything they leave out.
     *
     * @param array{symbols?: array<int, string|false>, attributes?: array<int, string|false>, sample?: string} $answers
     */
    public static function replay(array $answers): self
    {
        $formatter = new self('en_US', \NumberFormatter::DECIMAL);
        $formatter->symbols = ($answers['symbols'] ?? []) + self::DEFAULT_SYMBOLS;
        $formatter->attributes = ($answers['attributes'] ?? []) + self::DEFAULT_ATTRIBUTES;
        $formatter->sampleText = $answers['sample'] ?? $formatter->sampleText;
        return $formatter;
    }

    /**
     * The answers as the fixture holds them.
     *
     * @param array<string, mixed> $answers
     * @return array<string, mixed>
     */
    public static function encode(array $answers): array
    {
        return $answers;
    }

    public function getSymbol(int $symbol): string|false
    {
        return $this->symbols[$symbol] ?? false;
    }

    public function getTextAttribute(int $attribute): string|false
    {
        return $this->attributes[$attribute] ?? false;
    }

    /** The recorded text of -1234.5. */
    public function sample(): string
    {
        return $this->sampleText;
    }

    /** One recorded symbol, '' for false. */
    public function symbol(int $symbol): string
    {
        $text = $this->symbols[$symbol] ?? false;
        return \is_string($text) ? $text : '';
    }
}
