import Foundation

/// The lenience flags of ``NumFormat`` — bit-for-bit the native core's flags.
public struct NumStyles: OptionSet, Sendable {
    public let rawValue: UInt32

    public init(rawValue: UInt32) {
        self.rawValue = rawValue
    }

    /// Permit the declared group separator between digits (sizes not validated — between
    /// digits is the rule).
    public static let grouping = NumStyles(rawValue: 1)
    /// Permit accounting parentheses as negation: `(1,234)` is -1234.
    public static let parentheses = NumStyles(rawValue: 1 << 1)
    /// Permit exponent notation. Integer doors reject a negative exponent.
    public static let exponent = NumStyles(rawValue: 1 << 2)
    /// Permit `0x`/`&H`/`0b` two's-complement radix prefixes (`0xFF` is -1 for an i8).
    public static let radixPrefixes = NumStyles(rawValue: 1 << 3)
    /// Permit a trailing `%`, dividing by 100. Real and decimal doors only.
    public static let percent = NumStyles(rawValue: 1 << 4)
    /// Resolve the `.`/`,` roles per input from structure instead of the declared
    /// separators (which are ignored while this flag is set). Detection, not sniffing: a
    /// repeated separator is grouping (`1.234.567,89`); with both present the rightmost is
    /// the decimal; a single separator with a non-3-digit right run is the decimal
    /// (`3,1415`); with exactly 3 digits right, only a `0` integer part proves decimal
    /// (`0,785`). Genuinely ambiguous input (`12.185`, `1,000`) is malformed at the
    /// separator, never guessed.
    public static let separatorDetect = NumStyles(rawValue: 1 << 5)
    /// Permit the declared ``NumFormat/currencySymbol`` at either edge of the numeric body
    /// — leading, before or after a sign (`$5`, `-$5`, `$ -5`), or trailing (`5 €`,
    /// `1.234,50 kr.`) — once, with optional ASCII whitespace between symbol and digits;
    /// accounting parentheses wrap the symbol along with the digits (`($5)`). With no
    /// symbol declared the flag matches nothing and changes nothing; a symbol declared
    /// without this flag is malformed at the symbol. Integer, real and decimal doors.
    public static let currency = NumStyles(rawValue: 1 << 6)
    /// Every lenience on (`separatorDetect` is a separator policy, not a lenience, and is
    /// deliberately not included).
    public static let all: NumStyles = [
        .grouping, .parentheses, .exponent, .radixPrefixes, .percent, .currency,
    ]
}

/// Caller-declared numeric notation for the integer, real and decimal doors. The native
/// core carries no culture data — a call site parsing culture-sensitive text declares its
/// format out loud (``invariant``, or ``from(locale:)``); there is no default argument, the
/// same stance every binding in this repo takes. Separators are Unicode scalars — exactly
/// the core's code-point fields; the currency symbol is the one field that needs a culture
/// table to fill in, and it is declared, never looked up.
public struct NumFormat: Equatable, Sendable {
    public let decimalSeparator: Unicode.Scalar
    public let groupSeparator: Unicode.Scalar
    public let styles: NumStyles
    /// The declared currency symbol (`$`, `€`, `kr.`, `CHF`), honored only while
    /// ``NumStyles/currency`` is in ``styles``; empty declares none. At most
    /// 16 UTF-8 bytes, with no ASCII digit or ASCII whitespace in it — those would collide
    /// with the digit scan and the trimming the doors do around the symbol.
    public let currencySymbol: String

    // The symbol packed the way the native `RawNumFormat` carries it — its UTF-8 length
    // and the 16 inline bytes as two little-endian words — computed once here, so a door
    // call copies three integers onto the stack instead of walking the string.
    let currencyLength: UInt32
    let currencyLow: UInt64
    let currencyHigh: UInt64

    /// The inline capacity of the native format's currency field, in UTF-8 bytes.
    static let maxCurrencySymbolBytes = 16

    /// - Precondition: the separators differ, and `currencySymbol` is at most 16 UTF-8
    ///   bytes with no ASCII digit or ASCII whitespace — either is a caller bug, not a data
    ///   verdict.
    public init(
        decimalSeparator: Unicode.Scalar, groupSeparator: Unicode.Scalar, styles: NumStyles,
        currencySymbol: String = ""
    ) {
        precondition(
            decimalSeparator != groupSeparator,
            "Decimal and group separators must differ; both are '\(decimalSeparator)'")
        precondition(
            Self.isDeclarable(currencySymbol: currencySymbol),
            "Currency symbol '\(currencySymbol)' must be at most \(Self.maxCurrencySymbolBytes) UTF-8 bytes "
                + "with no ASCII digit or whitespace")
        self.decimalSeparator = decimalSeparator
        self.groupSeparator = groupSeparator
        self.styles = styles
        self.currencySymbol = currencySymbol
        var low: UInt64 = 0
        var high: UInt64 = 0
        for (index, byte) in currencySymbol.utf8.enumerated() {
            if index < 8 {
                low |= UInt64(byte) << (8 * UInt64(index))
            } else {
                high |= UInt64(byte) << (8 * UInt64(index - 8))
            }
        }
        // `.littleEndian` so the words' in-memory bytes are the symbol's bytes in order on
        // any host, which is what the native side reads them as.
        currencyLength = UInt32(currencySymbol.utf8.count)
        currencyLow = low.littleEndian
        currencyHigh = high.littleEndian
    }

    /// The native format's own rule for a symbol: no more than 16 UTF-8 bytes, and no
    /// ASCII digit or ASCII whitespace anywhere in it. Empty (no symbol) passes.
    static func isDeclarable(currencySymbol symbol: String) -> Bool {
        symbol.utf8.count <= maxCurrencySymbolBytes
            && !symbol.utf8.contains { byte in
                (0x30...0x39).contains(byte) || byte == 0x20 || (0x09...0x0D).contains(byte)
            }
    }

    /// The invariant profile — `.` decimal, `,` grouping, every lenience on, no currency
    /// symbol declared.
    public static let invariant = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all)

    /// The detection profile — every lenience on, `.`/`,` roles resolved per input by
    /// ``NumStyles/separatorDetect``'s structural rules.
    public static let detect = NumFormat(
        decimalSeparator: ".", groupSeparator: ",", styles: [.all, .separatorDetect])

    /// Derives a format from a locale's separators (first scalar of each) and its currency
    /// symbol, with every lenience on. Falls back to the invariant separators where the
    /// locale doesn't say, and to no symbol where it names none — or names one the native
    /// format cannot carry (see ``currencySymbol``), since a locale is process/user state
    /// and no cast should trap on it.
    public static func from(locale: Locale) -> NumFormat {
        let decimal = locale.decimalSeparator?.unicodeScalars.first ?? "."
        let group = locale.groupingSeparator?.unicodeScalars.first ?? ","
        let currency = locale.currencySymbol.flatMap { isDeclarable(currencySymbol: $0) ? $0 : nil } ?? ""
        return NumFormat(
            decimalSeparator: decimal, groupSeparator: group, styles: .all, currencySymbol: currency)
    }
}

/// The caller-declared field order of a separated calendar date — no guessing, ever:
/// `1/7/2026` is January 7th (``monthDayYear``, the en-US order) or July 1st
/// (``dayMonthYear``, the en-GB order) only because the caller said which. Values match
/// the native core's discriminants.
public enum DateOrder: UInt32, Sendable {
    /// Year, month, day — ISO's order with any accepted separator.
    case yearMonthDay = 1
    /// Month, day, year — the en-US short-date order (`1/7/2026` is January 7th).
    case monthDayYear = 2
    /// Day, month, year — the en-GB/most-of-the-world order (`1/7/2026` is July 1st).
    case dayMonthYear = 3

    /// Derives the field order from a locale's own short-date pattern: the first of
    /// `y`/`M`/`d` to appear decides (quoted literals skipped). Prefer declaring
    /// explicitly when the text's origin is known — a locale is process/user state; the
    /// text's dialect is a property of the text. Falls back to ``yearMonthDay`` if the
    /// pattern names none of the fields.
    public static func from(locale: Locale) -> DateOrder {
        let formatter = DateFormatter()
        formatter.locale = locale
        formatter.dateStyle = .short
        formatter.timeStyle = .none
        var inLiteral = false
        for ch in formatter.dateFormat ?? "" {
            if ch == "'" {
                inLiteral.toggle()
                continue
            }
            if inLiteral { continue }
            switch ch {
            case "y", "u": return .yearMonthDay
            case "M", "L": return .monthDayYear
            case "d": return .dayMonthYear
            default: break
            }
        }
        return .yearMonthDay
    }
}

/// The declared unit of a Unix-epoch value — no magnitude guessing, ever. Values match the
/// native core's discriminants.
public enum UnixPrecision: UInt32, Sendable {
    case seconds = 1
    case milliseconds = 2
    case microseconds = 3
    case nanoseconds = 4
}

/// The date system an Excel serial number is expressed in. Spreadsheets carry no marker for
/// this — it is a workbook-level setting — so the caller states it, the same way
/// ``UnixPrecision`` and ``DateOrder`` are declared rather than guessed. Values match the
/// native core's discriminants.
public enum ExcelEpoch: UInt32, Sendable {
    /// The Windows default: serial `1` is 1900-01-01, and serial `60` is a February 29th
    /// that never existed.
    case y1900 = 1
    /// The legacy Macintosh system, still selectable today: serial `0` is 1904-01-01, with
    /// no phantom day anywhere in it.
    case y1904 = 2
}
