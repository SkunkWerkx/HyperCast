import Foundation

/// Opt-in date and time shapes, which the date doors accept beside their strict ones: the
/// shapes a door refuses unless the caller asks. Without options every door keeps its
/// shape rules (a caller using strict RFC 3339 may rely on a space separator or `+00` being
/// refused).
///
/// | Option | Door | Accepts |
/// | --- | --- | --- |
/// | ``Flags/compactDate`` | ``Cast/date(_:options:)-swift.type.method`` | `20260107` (eight ASCII digits) |
/// | ``Flags/spaceSeparator`` | ``Cast/timestamp(_:options:)-swift.type.method`` | a single space in place of `T` |
/// | ``Flags/hourOffset`` | ``Cast/timestamp(_:options:)-swift.type.method`` | `±hh` offsets (`+00`, `-05`) |
/// | ``pivot`` | the order doors | two-digit years |
/// | ``months`` | the order doors | declared month names in the month field |
///
/// A known option a door doesn't read is ignored by it.
public struct DateOptions: Hashable, Sendable {
    /// The option bits, as the native `RawDateOptions` carries them.
    public struct Flags: OptionSet, Hashable, Sendable {
        /// The bits, exactly as the native options' `flags` word carries them.
        public let rawValue: UInt32

        /// Wraps raw option bits. A bit no option below names is carried through here, and
        /// ``DateOptions`` refuses it: the core accepts no undefined option.
        public init(rawValue: UInt32) {
            self.rawValue = rawValue
        }

        /// `20260107`: eight ASCII digits as `yyyyMMdd`, at the date door.
        public static let compactDate = Flags(rawValue: 1)
        /// A single space in place of the `T` of an RFC 3339 instant, at the timestamp door.
        public static let spaceSeparator = Flags(rawValue: 1 << 1)
        /// `±hh` offsets (`+00`, `-05`) beside `±hh:mm`, at the timestamp door.
        public static let hourOffset = Flags(rawValue: 1 << 2)
        /// The three shapes above together. It supplies no pivot and no month names.
        public static let commonForms: Flags = [.compactDate, .spaceSeparator, .hourOffset]

        /// The bits the options above define.
        static let definedBits: UInt32 = 0b111
    }

    /// The shapes this declaration opens.
    public let flags: Flags
    /// The last year of the 100-year window a two-digit year is read in (`26` is 2026 under
    /// 2029, and `30` is 1930), 100 through 9999; `nil` for none, where a two-digit year is
    /// malformed. Four-digit years are unaffected.
    public let pivot: Int?
    /// Declared month names read in the month field of the order doors (`07-Jan-26`), or `nil`
    /// for none. Independent of ``flags``: it may accompany no flag at all.
    public let months: MonthNames?

    /// Declares options. Every field defaults to nothing declared: today's shape rules.
    ///
    /// - Precondition: `flags` sets no bit the core doesn't define, and `pivot` is `nil` or
    ///   100 through 9999 — a caller bug, not a data verdict, as the core refuses the same
    ///   declarations.
    public init(flags: Flags = [], pivot: Int? = nil, months: MonthNames? = nil) {
        // Under 100 or past the ABI's u32 is refused as the core would: the sentinel 1 stands
        // for any such value, and `nil`, not 0, is how none is declared.
        let word = pivot.map { (100...9999).contains($0) ? UInt32($0) : 1 } ?? 0
        let refusal = Self.refusal(flags: flags.rawValue, pivot: word)
        precondition(refusal == nil, refusal ?? "")
        self.flags = flags
        self.pivot = pivot
        self.months = months
    }

    /// No options: every door's own shape rules, no pivot, no month names.
    public static let none = DateOptions()

    /// ``Flags/commonForms``: compact dates, a space separator and hour offsets, with no pivot
    /// and no month names.
    public static let commonForms = DateOptions(flags: .commonForms)

    /// The core's rules for the option bits and pivot: a bit no option defines, or a pivot other
    /// than 0 (none) or 100 through 9999, is refused. `nil` when declarable, else the message
    /// the initializer traps with.
    static func refusal(flags: UInt32, pivot: UInt32) -> String? {
        if flags & ~Flags.definedBits != 0 {
            return
                "Date option bits 0x\(String(flags & ~Flags.definedBits, radix: 16, uppercase: true)) are not defined; "
                + "the options are compactDate, spaceSeparator and hourOffset."
        }
        if pivot != 0 && !(100...9999).contains(pivot) {
            return "The pivot \(pivot) must be 100 through 9999 (the last year of the window), or nil for none."
        }
        return nil
    }

    /// The options laid out as the native `RawDateOptions` carries them.
    var raw: Interop.RawDateOptions {
        (UInt32(Interop.rawDateOptionsSize), flags.rawValue, UInt32(pivot ?? 0))
    }
}

/// Caller-declared month names for the date doors that read a month field: `janvier`, `Sep` and
/// `Sept`, `janv.`. Built by the caller and never looked up from the platform: aliases,
/// abbreviations and grammatical forms are the caller's decision.
///
/// Each entry maps a name to a month, 1 through 12, and aliases are separate entries. Names
/// match under simple case folding, and the longest entry followed by the separator the date
/// is using wins. Punctuation is part of a name and never stripped: with `janv.` declared and
/// `.` as the separator, `7.janv..2026` reads, while `7.janv.2026` doesn't unless `janv` is
/// declared too.
///
/// A name is 1 to 64 UTF-8 bytes with no ASCII digit, `/`, `-` or white space (`.` is fine
/// anywhere), every month has at least one name, and no two names fold to the same spelling.
/// The vocabulary is packed once, as the core reads it: each entry a month byte, a
/// little-endian `UInt32` byte length, then the UTF-8 bytes, with no padding.
public struct MonthNames: Hashable, Sendable {
    /// The vocabulary in the native packed layout, which a door hands the core as it is.
    public let packed: [UInt8]

    /// Declares the vocabulary from `(month, name)` entries.
    ///
    /// - Precondition: the entries obey the rules above — a caller bug, not a data verdict,
    ///   as the core refuses the same vocabularies.
    public init(_ entries: [(month: Int, name: String)]) {
        do {
            self = try Self.declare(entries)
        } catch {
            preconditionFailure(error.message)
        }
    }

    private init(packed: [UInt8]) {
        self.packed = packed
    }

    /// A vocabulary the binding or the core refused, and the message the public initializer
    /// traps with.
    struct Refused: Error {
        let message: String
    }

    /// The whole of the public initializer but the trap, so a suite can see a refusal. Each
    /// rule the binding can name is judged here, in the order an entry meets them; the one
    /// that needs the core's case folding (two names folding to the same spelling) is the
    /// core's own judgement, so no folding table is restated.
    static func declare(_ entries: [(month: Int, name: String)]) throws(Refused) -> MonthNames {
        var seen = Set<Int>()
        var packed: [UInt8] = []
        for entry in entries {
            guard (1...12).contains(entry.month) else {
                throw Refused(message: "The month \(entry.month) must be 1 through 12.")
            }
            let bytes = Array(entry.name.utf8)
            guard (1...64).contains(bytes.count) else {
                throw Refused(message: "The month name '\(entry.name)' must be 1 to 64 UTF-8 bytes.")
            }
            guard
                !bytes.contains(where: {
                    NumFormat.isAsciiDigit($0) || $0 == UInt8(ascii: "/") || $0 == UInt8(ascii: "-")
                }),
                !NumFormat.containsWhiteSpace(entry.name)
            else {
                throw Refused(
                    message:
                        "The month name '\(entry.name)' holds an ASCII digit, a slash, a hyphen or white space.")
            }
            seen.insert(entry.month)
            packed.append(UInt8(entry.month))
            withUnsafeBytes(of: UInt32(bytes.count).littleEndian) { packed.append(contentsOf: $0) }
            packed.append(contentsOf: bytes)
        }
        guard seen.count == 12 else {
            let missing = (1...12).first { !seen.contains($0) } ?? 0
            throw Refused(message: "The month \(missing) has no name; every month needs at least one.")
        }
        guard Cast.vocabularyIsAccepted(packed) else {
            throw Refused(
                message:
                    "The core refuses the month names: with the month, length and character rules kept, "
                    + "that is two names that fold to the same spelling, which only its case folding judges.")
        }
        return MonthNames(packed: packed)
    }
}
