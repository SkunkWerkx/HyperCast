<?php

declare(strict_types=1);

if (!extension_loaded('hypercast')) {
    fwrite(STDERR, 'the hypercast extension is not loaded; run composer test' . PHP_EOL);
    exit(1);
}

require __DIR__ . '/../vendor/autoload.php';
