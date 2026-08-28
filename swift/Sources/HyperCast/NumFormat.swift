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

    /// Derives a format from a locale's separators (first scalar of each), with every
    /// lenience on. Falls back to the invariant separators where the locale doesn't say.
    public static func from(locale: Locale) -> NumFormat {
        let decimal = locale.decimalSeparator?.unicodeScalars.first ?? "."
        let group = locale.groupingSeparator?.unicodeScalars.first ?? ","
        return NumFormat(decimalSeparator: decimal, groupSeparator: group, styles: .all)
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
