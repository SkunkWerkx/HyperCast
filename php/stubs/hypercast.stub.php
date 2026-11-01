<?php

/**
 * Stubs for the hypercast extension, for IDEs and static analysis. Generated: don't edit.
 * Regenerate from php/ with `php tools/generate-stubs.php`, which runs
 * `cargo php stubs --stdout` (cargo-php 0.2.1) on the debug build of the extension and
 * completes its output (the tool says how).
 */

namespace HyperCast {
    /**
     * The doors: each casts a string and returns a `Success` holding the value or a `Fault`
     * naming the reason and the byte span that offended.
     *
     * PHP-flavoured fidelity: `int` is 64-bit signed, so u64 carries the two's-complement bit
     * pattern (render with `sprintf('%u', ...)`); `DateTimeImmutable` tops out at microseconds,
     * so the core's nanoseconds truncate by three digits on the instant doors; time of day is an
     * exact `int` of nanoseconds since midnight; durations come back as the protobuf pair
     * (`Duration`) and decimals as the core's exact triple (`Decimal`).
     */
    final class Cast {
        /**
         * Not instantiable: every door is static.
         */
        private function __construct() {}

        /**
         * `true`/`false` and their common spellings.
         *
         * @param string $text
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function bool(string $text): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * One Unicode scalar value, as its UTF-8 string.
         *
         * @param string $text
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function char(string $text): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\DateOrder|null $order
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function date(string $text, ?\HyperCast\DateOrder $order = null): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\DateOrder $order
         * @param \HyperCast\DayPeriods|null $periods
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function datetime(string $text, \HyperCast\DateOrder $order, ?\HyperCast\DayPeriods $periods = null): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\NumFormat|\NumberFormatter $format
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function decimal(string $text, \HyperCast\NumFormat|\NumberFormatter $format): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param float $value
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function decimalFromFloat(float $value): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function duration(string $text): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param float $value
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function excelDuration(float $value): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\ExcelEpoch $epoch
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function excelSerial(string $text, \HyperCast\ExcelEpoch $epoch): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param float $value
         * @param \HyperCast\ExcelEpoch $epoch
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function excelSerialFromFloat(float $value, \HyperCast\ExcelEpoch $epoch): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * Nanoseconds since midnight.
         *
         * @param float $value
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function excelTime(float $value): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\NumFormat|\NumberFormatter $format
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function f32(string $text, \HyperCast\NumFormat|\NumberFormatter $format): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\NumFormat|\NumberFormatter $format
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function f64(string $text, \HyperCast\NumFormat|\NumberFormatter $format): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\NumFormat|\NumberFormatter $format
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function i16(string $text, \HyperCast\NumFormat|\NumberFormatter $format): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\NumFormat|\NumberFormatter $format
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function i32(string $text, \HyperCast\NumFormat|\NumberFormatter $format): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\NumFormat|\NumberFormatter $format
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function i64(string $text, \HyperCast\NumFormat|\NumberFormatter $format): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\NumFormat|\NumberFormatter $format
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function i8(string $text, \HyperCast\NumFormat|\NumberFormatter $format): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * The core's version, `major.minor.patch`.
         *
         * @return string
         */
        public static function nativeVersion(): string {}

        /**
         * The verdict, or `null` for an `Empty` fault.
         *
         * @param \HyperCast\Success|\HyperCast\Fault $verdict
         * @return \HyperCast\Success|\HyperCast\Fault|null
         */
        public static function optional(\HyperCast\Success|\HyperCast\Fault $verdict): \HyperCast\Success|\HyperCast\Fault|null {}

        /**
         * Nanoseconds since midnight.
         *
         * @param string $text
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function time(string $text): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function timestamp(string $text): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\NumFormat|\NumberFormatter $format
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function u16(string $text, \HyperCast\NumFormat|\NumberFormatter $format): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\NumFormat|\NumberFormatter $format
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function u32(string $text, \HyperCast\NumFormat|\NumberFormatter $format): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * A `uint64_t` read into a PHP int, as 1.0 read it: above `PHP_INT_MAX` it wraps negative.
         *
         * @param string $text
         * @param \HyperCast\NumFormat|\NumberFormatter $format
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function u64(string $text, \HyperCast\NumFormat|\NumberFormatter $format): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\NumFormat|\NumberFormatter $format
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function u8(string $text, \HyperCast\NumFormat|\NumberFormatter $format): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * @param string $text
         * @param \HyperCast\UnixPrecision $precision
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function unix(string $text, \HyperCast\UnixPrecision $precision): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * The canonical lowercase hyphenated form.
         *
         * @param string $text
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function uuid(string $text): \HyperCast\Success|\HyperCast\Fault {}

        /**
         * The 16 bytes, RFC 9562 order.
         *
         * @param string $text
         * @return \HyperCast\Success|\HyperCast\Fault
         */
        public static function uuidBytes(string $text): \HyperCast\Success|\HyperCast\Fault {}
    }

    /**
     * Why a cast failed.
     */
    enum CastFailure: int {
      case Empty = 1;
      case Malformed = 2;
      case OutOfRange = 3;
    }

    /**
     * The order of a date's numeric fields.
     */
    enum DateOrder: int {
      case Ymd = 1;
      case Mdy = 2;
      case Dmy = 3;
    }

    /**
     * Excel's two date systems.
     */
    enum ExcelEpoch: int {
      case Y1900 = 1;
      case Y1904 = 2;
    }

    /**
     * The unit of a Unix timestamp.
     */
    enum UnixPrecision: int {
      case Seconds = 1;
      case Milliseconds = 2;
      case Microseconds = 3;
      case Nanoseconds = 4;
    }
}

namespace HyperCast\Interop {
    /**
     * Maps the running process to the RID-style directory and file name of a native library
     * (`native/{rid}/`), and finds the library to load: for a package that ships a SkunkWerkx
     * core's builds that way and loads them with ext-ffi (HyperTabular loads `hypertabular`).
     */
    final class NativePlatform {
        /**
         * Not instantiable: every helper is static.
         */
        private function __construct() {}

        /**
         * Where a native library for the running platform is: the `$override` environment
         * variable's path, else `<sourceDir>/native/<rid>/<library>`, else a repository build.
         *
         * @param string $baseName
         * @param string $sourceDir
         * @param string $override
         * @return string
         */
        public static function libraryPath(string $baseName, string $sourceDir, string $override): string {}

        /**
         * `[rid, libraryName]` for a declared platform.
         *
         * @param string $baseName
         * @param string $osFamily
         * @param string $machine
         * @param int $intSize
         * @param bool $musl
         * @return array
         */
        public static function resolve(string $baseName, string $osFamily, string $machine, int $intSize, bool $musl): array {}

        /**
         * `[rid, libraryName]` for the running PHP.
         *
         * @param string $baseName
         * @return array
         */
        public static function ridAndLibraryName(string $baseName): array {}
    }

    /**
     * The core's C ABI values, presented, for a package that carries HyperCast's verdicts across
     * a C ABI of its own (HyperTabular does): it reads the core's fields out of its own buffers
     * and builds each value through the conversion every `Cast` door uses. A `Decimal` is built
     * by `Decimal::fromLimbs()`, a span by the `Duration` constructor.
     */
    final class NativeValues {
        /**
         * Not instantiable: every helper is static.
         */
        private function __construct() {}

        /**
         * A civil date and time of day, read as UTC.
         *
         * @param int $year
         * @param int $month
         * @param int $day
         * @param int $nanosOfDay
         * @return \DateTimeImmutable
         */
        public static function civil(int $year, int $month, int $day, int $nanosOfDay): \DateTimeImmutable {}

        /**
         * Midnight UTC of a civil date.
         *
         * @param int $year
         * @param int $month
         * @param int $day
         * @return \DateTimeImmutable
         */
        public static function date(int $year, int $month, int $day): \DateTimeImmutable {}

        /**
         * A `Fault` from the C ABI's verdict code and span.
         *
         * @param int $code
         * @param int $offset
         * @param int $length
         * @return \HyperCast\Fault
         */
        public static function fault(int $code, int $offset, int $length): \HyperCast\Fault {}

        /**
         * A UTC instant from Unix seconds and nanoseconds (truncated to microseconds).
         *
         * @param int $seconds
         * @param int $nanos
         * @return \DateTimeImmutable
         */
        public static function instant(int $seconds, int $nanos): \DateTimeImmutable {}

        /**
         * 16 bytes as the canonical lowercase hyphenated text.
         *
         * @param string $bytes
         * @return string
         */
        public static function uuid(string $bytes): string {}

        /**
         * `hypercast_version()`'s packed version as `major.minor.patch`.
         *
         * @param int $packed
         * @return string
         */
        public static function version(int $packed): string {}

        /**
         * Writes a set of day periods' packed layout into an `hc_periods` (ext-ffi).
         *
         * @param \HyperCast\DayPeriods $periods
         * @param \FFI\CData $target
         * @return void
         */
        public static function writeDayPeriods(\HyperCast\DayPeriods $periods, \FFI\CData $target): void {}

        /**
         * Writes a format's packed layout into an `hc_format` (ext-ffi).
         *
         * @param \HyperCast\NumFormat $format
         * @param \FFI\CData $target
         * @return void
         */
        public static function writeFormat(\HyperCast\NumFormat $format, \FFI\CData $target): void {}
    }
}

namespace HyperCast {
    /**
     * A door's successful verdict: the value it cast.
     */
    final readonly class Success {
        public function __construct(public mixed $value) {}
    }

    /**
     * A door's failed verdict: why, and the byte span of the input at fault.
     */
    final readonly class Fault {
        public function __construct(
            public \HyperCast\CastFailure $reason,
            public int $offset,
            public int $length,
        ) {}
    }

    /**
     * An exact decimal: the magnitude's digits, shifted right by `scale` places.
     */
    final readonly class Decimal implements \Stringable {
        public function __construct(
            public string $magnitude,
            public int $scale,
            public bool $negative,
        ) {}

        /**
         * From the C ABI's 96-bit magnitude, in two limbs.
         */
        public static function fromLimbs(int $lo, int $hi, int $scale, bool $negative): \HyperCast\Decimal {}

        public function __toString(): string {}

        /**
         * The nearest float, which may lose digits.
         */
        public function toFloat(): float {}
    }

    /**
     * A signed span of time; `nanos` carries the same sign as `seconds`.
     */
    final readonly class Duration {
        public function __construct(
            public int $seconds,
            public int $nanos,
        ) {}

        public function toSeconds(): float {}
    }

    /**
     * The day-period designators `datetime()` reads beside the English AM and PM.
     */
    final readonly class DayPeriods {
        public const MAX_BYTES = 16;
        public const PACKED_BYTES = 40;

        public function __construct(
            public string $am,
            public string $pm,
        ) {}

        /**
         * The C ABI's `hc_periods` layout.
         */
        public function packed(): string {}
    }

    /**
     * A declared number format: separators, notation flags and symbols.
     */
    final readonly class NumFormat {
        public const GROUPING = 1;
        public const PARENTHESES = 2;
        public const EXPONENT = 4;
        public const RADIX_PREFIXES = 8;
        public const PERCENT = 16;
        public const SEPARATOR_DETECT = 32;
        public const CURRENCY = 64;
        public const TRAILING_SIGN = 128;
        public const ALL = 95;
        public const SYMBOL_MAX_BYTES = 8;
        public const CURRENCY_MAX_BYTES = 16;
        public const PACKED_BYTES = 92;

        public function __construct(
            public string $decimalSep,
            public string $groupSep,
            public int $flags,
            public string $currency = '',
            public string $currencyDecimalSep = '',
            public string $currencyGroupSep = '',
            public string $minusSign = '',
            public string $plusSign = '',
            public string $percent = '',
            public string $digitZero = '',
        ) {}

        /**
         * `.` and `,` with every notation: one instance per request.
         */
        public static function invariant(): \HyperCast\NumFormat {}

        /**
         * The invariant format that also detects a comma decimal: one instance per request.
         */
        public static function detect(): \HyperCast\NumFormat {}

        /**
         * From a `localeconv()` array, the current locale's when `$conv` is null.
         */
        public static function fromLocaleconv(?array $conv = null): \HyperCast\NumFormat {}

        /**
         * From an ext-intl formatter's symbols and negative pattern.
         */
        public static function fromNumberFormatter(\NumberFormatter $formatter): \HyperCast\NumFormat {}

        /**
         * The C ABI's `hc_format` layout.
         */
        public function packed(): string {}
    }
}
