<?php

/**
 * The culture sweep: what `NumFormat::fromNumberFormatter()` and `fromLocaleconv()` declare for
 * every locale this PHP can reach, as JSON, so two builds (a base and a change) can be compared.
 *
 *   php -d extension=<build> tools/culture-sweep.php [out.json]
 *   php tools/culture-sweep.php --compare base.json change.json
 *
 * The sweep walks every ICU locale in the decimal, currency and percent styles, and every
 * system locale `locale -a` lists. A locale is a finding when a build throws where the base did
 * not or loses a symbol. What the grouping rule moves is not a finding: the policy a locale
 * declares (Western, or Indian where its number pattern is) and whether `GROUPING` stays on
 * (cleared only where neither groups). The compare mode counts those and lists the cleared
 * locales.
 */

declare(strict_types=1);

// A script, run directly: its helper functions sit beside the steps that use them.
// phpcs:disable PSR1.Files.SideEffects.FoundWithSymbols

/** @return array<string, mixed> */
function describe(callable $build): array
{
    try {
        $format = $build();
    } catch (\Throwable $e) {
        return ['threw' => $e::class . ': ' . $e->getMessage()];
    }
    $fields = get_object_vars($format);
    $grouping = $fields['grouping'] ?? null;
    unset($fields['grouping']);
    // The GROUPING flag is the grouping rule's, reported beside the policy, not a symbol.
    $on = ($fields['flags'] & 1) === 1;
    $fields['flags'] &= ~1;
    return ['fields' => $fields, 'grouping' => $grouping, 'grouping_on' => $on];
}

/** Prints what changed between two sweeps; the exit status is 1 when a finding is not the grouping default. */
function compare(string $basePath, string $changePath): int
{
    $base = json_decode((string) file_get_contents($basePath), true, 512, JSON_THROW_ON_ERROR);
    $change = json_decode((string) file_get_contents($changePath), true, 512, JSON_THROW_ON_ERROR);
    $findings = 0;
    foreach (['icu', 'system'] as $kind) {
        foreach ($base[$kind] as $key => $was) {
            $now = $change[$kind][$key] ?? null;
            if ($now === null) {
                echo "FINDING {$kind} {$key}: missing from the change\n";
                $findings++;
            } elseif (isset($now['threw']) && !isset($was['threw'])) {
                echo "FINDING {$kind} {$key}: newly throws: {$now['threw']}\n";
                $findings++;
            } elseif (
                ($was['fields'] ?? null) !== ($now['fields'] ?? null)
                || ($was['threw'] ?? null) !== ($now['threw'] ?? null)
            ) {
                echo "FINDING {$kind} {$key}: the declared symbols differ\n";
                $findings++;
            }
        }
        foreach (array_diff_key($change[$kind], $base[$kind]) as $key => $_) {
            echo "FINDING {$kind} {$key}: absent from the base\n";
            $findings++;
        }
    }
    $declared = [];
    $cleared = [];
    foreach ($change as $kind => $entries) {
        foreach ($entries as $key => $now) {
            $policy = match ($now['grouping'] ?? null) {
                0 => 'Western',
                512 => 'Indian',
                default => 'none',
            };
            $on = ($now['grouping_on'] ?? true) ? 'GROUPING on' : 'GROUPING cleared';
            $declared["{$kind}: {$policy}, {$on}"] = ($declared["{$kind}: {$policy}, {$on}"] ?? 0) + 1;
            if (!($now['grouping_on'] ?? true)) {
                $cleared[] = "{$kind} {$key}";
            }
        }
    }
    echo 'locales compared: icu ' . \count($base['icu']) . ', system ' . \count($base['system']) . "\n";
    echo "findings besides the grouping rule: {$findings}\n";
    ksort($declared);
    foreach ($declared as $what => $count) {
        echo "  {$count}  {$what}\n";
    }
    echo 'GROUPING cleared: ' . implode(', ', $cleared) . "\n";
    return $findings === 0 ? 0 : 1;
}

if (($argv[1] ?? null) === '--compare') {
    exit(compare($argv[2] ?? '', $argv[3] ?? ''));
}

$sweep = ['icu' => [], 'system' => []];
if (extension_loaded('intl')) {
    $styles = [
        'decimal' => \NumberFormatter::DECIMAL,
        'currency' => \NumberFormatter::CURRENCY,
        'percent' => \NumberFormatter::PERCENT,
    ];
    foreach (\ResourceBundle::getLocales('') as $locale) {
        foreach ($styles as $name => $style) {
            $formatter = new \NumberFormatter($locale, $style);
            $sweep['icu']["{$locale} {$name}"] = ['pattern' => $formatter->getPattern()]
                + describe(static fn () => \HyperCast\NumFormat::fromNumberFormatter($formatter));
        }
    }
}
exec('locale -a 2>/dev/null', $names);
foreach ($names as $name) {
    if (!str_contains($name, 'UTF-8') && !str_contains($name, 'utf8')) {
        continue;
    }
    if (setlocale(LC_ALL, $name) === false) {
        continue;
    }
    $conv = localeconv();
    $sweep['system'][$name] = [
        'lc_grouping' => $conv['grouping'] ?? null,
        'lc_mon_grouping' => $conv['mon_grouping'] ?? null,
    ] + describe(static fn () => \HyperCast\NumFormat::fromLocaleconv($conv));
}
setlocale(LC_ALL, 'C');
$json = json_encode($sweep, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR);
if (isset($argv[1])) {
    file_put_contents($argv[1], $json . "\n");
} else {
    echo $json, "\n";
}
