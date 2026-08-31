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
    /// Permit a trailing `%`, dividing by 100. Real doors only.
    public static let percent = NumStyles(rawValue: 1 << 4)
    /// Every lenience on.
    /// Resolve the `.`/`,` roles per input from structure instead of the declared
    /// separators (which are ignored while this flag is set). Detection, not sniffing: a
    /// repeated separator is grouping (`1.234.567,89`); with both present the rightmost is
    /// the decimal; a single separator with a non-3-digit right run is the decimal
    /// (`3,1415`); with exactly 3 digits right, only a `0` integer part proves decimal
    /// (`0,785`). Genuinely ambiguous input (`12.185`, `1,000`) is malformed at the
    /// separator, never guessed.
    public static let separatorDetect = NumStyles(rawValue: 1 << 5)
    /// Every lenience on (`separatorDetect` is a separator policy, not a lenience, and is
    /// deliberately not included).
    public static let all: NumStyles = [.grouping, .parentheses, .exponent, .radixPrefixes, .percent]
}

/// Caller-declared numeric notation for the integer and real doors. The native core carries
/// no culture data — a call site parsing culture-sensitive text declares its format out
/// loud (``invariant``, or ``from(locale:)``); there is no default argument, the same
/// stance every binding in this repo takes. Separators are Unicode scalars — exactly the
/// core's code-point fields.
public struct NumFormat: Equatable, Sendable {
    public let decimalSeparator: Unicode.Scalar
    public let groupSeparator: Unicode.Scalar
    public let styles: NumStyles

    /// - Precondition: the separators differ — equal separators are a caller bug, not a
    ///   data verdict.
    public init(decimalSeparator: Unicode.Scalar, groupSeparator: Unicode.Scalar, styles: NumStyles) {
        precondition(
            decimalSeparator != groupSeparator,
            "Decimal and group separators must differ; both are '\(decimalSeparator)'")
        self.decimalSeparator = decimalSeparator
        self.groupSeparator = groupSeparator
        self.styles = styles
    }

    /// The invariant profile — `.` decimal, `,` grouping, every lenience on.
    public static let invariant = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all)

    /// The detection profile — every lenience on, `.`/`,` roles resolved per input by
    /// ``NumStyles/separatorDetect``'s structural rules.
    public static let detect = NumFormat(
        decimalSeparator: ".", groupSeparator: ",", styles: [.all, .separatorDetect])

    /// Derives a format from a locale's separators (first scalar of each), with every
    /// lenience on. Falls back to the invariant separators where the locale doesn't say.
    public static func from(locale: Locale) -> NumFormat {
        let decimal = locale.decimalSeparator?.unicodeScalars.first ?? "."
        let group = locale.groupingSeparator?.unicodeScalars.first ?? ","
        return NumFormat(decimalSeparator: decimal, groupSeparator: group, styles: .all)
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
