<?php

declare(strict_types=1);

namespace HyperCast;

/**
 * The date system an Excel serial number is expressed in. Spreadsheets carry no marker for
 * this — it is a workbook-level setting — so the caller states it, the same way
 * UnixPrecision and DateOrder are declared rather than guessed. Values match the native
 * core's discriminants.
 */
enum ExcelEpoch: int
{
    /** The Windows default: serial 1 is 1900-01-01, and serial 60 is a February 29th that never existed. */
    case Y1900 = 1;
    /** The legacy Macintosh system, still selectable today: serial 0 is 1904-01-01, no phantom day. */
    case Y1904 = 2;
}
