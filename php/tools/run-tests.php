<?php

declare(strict_types=1);

// `composer test`: runs PHPUnit in a PHP that has the extension loaded (tools/extension.php
// says which). No shell substitution, so it works on Windows too.

require __DIR__ . '/extension.php';

$root = \dirname(__DIR__);
$path = hypercast_extension($root);
hypercast_run(
    [PHP_BINARY, '-d', 'extension=' . $path, $root . '/vendor/bin/phpunit', ...\array_slice($argv, 1)],
    $root,
    'phpunit'
);
