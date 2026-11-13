import Foundation

/// The lenience flags of ``NumFormat`` — bit-for-bit the native core's flags, bits 0 to 7.
/// A style mask never carries the grouping policy: that is ``Grouping``, declared beside it.
public struct NumStyles: OptionSet, Hashable, Sendable {
    /// The flag bits, exactly as the native format's `flags` field carries them.
    public let rawValue: UInt32

    /// Wraps raw flag bits. A bit no flag below names is carried through here, and a
    /// ``NumFormat`` refuses it: the core accepts no undefined flag.
    public init(rawValue: UInt32) {
        self.rawValue = rawValue
    }

    /// The bits the eight flags below define.
    static let definedBits: UInt32 = 0xFF

    /// Permit the declared group separator between digits, in the group sizes the format's
    /// ``Grouping`` declares.
    public static let grouping = NumStyles(rawValue: 1)
    /// Permit accounting parentheses as negation: `(1,234)` is -1234.
    public static let parentheses = NumStyles(rawValue: 1 << 1)
    /// Permit exponent notation. Integer doors reject a negative exponent.
    public static let exponent = NumStyles(rawValue: 1 << 2)
    /// Permit `0x`/`&H`/`0b` two's-complement radix prefixes (`0xFF` is -1 for an i8).
    public static let radixPrefixes = NumStyles(rawValue: 1 << 3)
    /// Permit a trailing `%` — or the declared ``NumFormat/percentSymbol``, read beside it —
    /// dividing by 100. Real and decimal doors only.
    public static let percent = NumStyles(rawValue: 1 << 4)
    /// Resolve the `.`/`,` roles per input from structure instead of the declared
    /// separators (which are ignored while this flag is set, the currency ones included). Detection, not sniffing: a
    /// repeated separator is grouping (`1.234.567,89`); with both present the rightmost is
    /// the decimal; a single separator with a non-3-digit right run is the decimal
    /// (`3,1415`); with exactly 3 digits right, only a `0` integer part proves decimal
    /// (`0,785`). Genuinely ambiguous input (`12.185`, `1,000`) is malformed at the
    /// separator, never guessed.
    public static let separatorDetect = NumStyles(rawValue: 1 << 5)
    /// Permit the declared ``NumFormat/currencySymbol`` at either edge of the numeric body
    /// — leading, before or after a sign (`$5`, `-$5`, `$ -5`), or trailing (`5 €`,
    /// `1.234,50 kr.`) — once, with optional white space between symbol and digits;
    /// accounting parentheses wrap the symbol along with the digits (`($5)`). With no
    /// symbol declared the flag matches nothing and changes nothing; a symbol declared
    /// without this flag is malformed at the symbol. Integer, real and decimal doors.
    public static let currency = NumStyles(rawValue: 1 << 6)
    /// Permit the sign after the digits (`5-`, `5 -`, `5 €-`, `5- €`, `2.5%-`), the way a
    /// culture whose negative pattern is `n-` writes it: ASCII `-`/`+` or the declared
    /// signs, once, with optional white space before it, and never beside a leading
    /// sign or inside accounting parentheses. Not part of ``all``: a trailing sign is a
    /// culture's own pattern, so ``NumFormat/from(locale:)`` sets this from the locale's
    /// negative pattern rather than assuming it.
    public static let trailingSign = NumStyles(rawValue: 1 << 7)
    /// Every lenience on — every one there was before ``trailingSign``, which is declared
    /// from a culture's pattern rather than assumed (`separatorDetect` is a separator
    /// policy, not a lenience, and is deliberately not included either).
    public static let all: NumStyles = [
        .grouping, .parentheses, .exponent, .radixPrefixes, .percent, .currency,
    ]
}

/// How a ``NumFormat`` reads group separators: the grouping policy, the core's own values in the
/// native format's `flags`. It judges the groups of a numeral that uses the declared group
/// separator, and only while ``NumStyles/grouping`` is on: with it off, a group separator stays
/// forbidden under every policy. Separator detection (``NumStyles/separatorDetect``) is
/// independent of it, and no policy resolves an ambiguous `1,234`.
///
/// `flags`' value 256 (bit 8) is retired permanently and never reused, so no case holds it and a
/// stale `351` is refused; a future policy takes a new bit from bit 10 up. Shapes neither policy
/// reads are malformed under a declared format; separator detection infers them instead, and can
/// read `1,2345` as 1.2345: `docs/grouping.md`.
public enum Grouping: UInt32, Sendable, CaseIterable {
    /// Groups of exactly three digits counted leftward from the decimal separator, the
    /// leftmost of one to three digits and not starting with a zero: `1,234,567` is read,
    /// `1,23,456`, `12,34`, `1234,567` and `0,125` are malformed at the group. A numeral with no separator stays valid. The
    /// default.
    case western = 0
    /// Indian grouping, declared only when it is meant: the group next to the decimal
    /// separator has three digits, every group left of it has two, the leftmost one or two
    /// (`12,34,567`).
    case indian = 0x200

    /// The mask of the two policy bits in the native `flags`.
    static let mask: UInt32 = 0x300
    /// The retired bit, refused wherever a word is judged.
    static let retired: UInt32 = 0x100
}

/// Caller-declared numeric notation for the integer, real and decimal doors. The native
/// core carries no culture data — a call site parsing culture-sensitive text declares its
/// format out loud (``invariant``, or ``from(locale:)``); there is no default argument, the
/// same stance every binding in this repo takes. Every field is a short UTF-8 string, which
/// is exactly the core's own field: the format is laid out once, when it is declared, and
/// the core reads that layout in place.
///
/// The two number separators and the lenience flags are the format; everything else is a
/// further spelling, declared, never looked up, and read *beside* the ASCII one, which the
/// core always reads: a declared minus sign does not stop `-` being read, a declared percent
/// symbol does not stop `%`, native digits do not stop `0`–`9`. A format that declares none
/// of them reads exactly what it did before they existed.
public struct NumFormat: Hashable, Sendable {
    /// The text between the integer and fraction digits (`.` in ``invariant``, `,` in
    /// eurozone text): 1 to 8 UTF-8 bytes, with no ASCII digit or white space in it.
    public var decimalSeparator: String { symbol(at: 12, capacity: Self.maxSymbolBytes) }
    /// The text permitted between digits while ``NumStyles/grouping`` is in ``styles``
    /// (`,` in ``invariant``; `.`, a narrow no-break space or `, ` elsewhere). Same rules
    /// as ``decimalSeparator``, except that it may be or contain white space (French and
    /// Swedish group with a no-break space), and never equal to it. A group declared as a
    /// space, U+00A0 or U+202F reads as any of the three.
    public var groupSeparator: String { symbol(at: 21, capacity: Self.maxSymbolBytes) }
    /// The leniences this format permits. Never carries the grouping policy: that is
    /// ``grouping``.
    public var styles: NumStyles { NumStyles(rawValue: raw.1 & NumStyles.definedBits) }
    /// How this format reads group separators. Western unless declared otherwise.
    public var grouping: Grouping { Grouping(rawValue: raw.1 & Grouping.mask) ?? .western }
    /// The declared currency symbol (`$`, `€`, `kr.`, `CHF`), honored only while
    /// ``NumStyles/currency`` is in ``styles``; empty declares none. At most
    /// 16 UTF-8 bytes, with no ASCII digit in it and no white space at either end — those
    /// would collide with the digit scan and the trimming the doors do around the symbol.
    /// White space inside is fine (`F CFA`), and a space, U+00A0 and U+202F there match
    /// each other.
    public var currencySymbol: String { symbol(at: 75, capacity: Self.maxCurrencySymbolBytes) }
    /// The decimal separator a currency amount is read with, in place of
    /// ``decimalSeparator``: when ``NumStyles/currency`` is in ``styles``, a
    /// ``currencySymbol`` is declared and the symbol is in the text, at either edge — the
    /// way Foundation's number parsing switches to a locale's monetary separators when it
    /// sees the symbol (`en_NL` writes a decimal comma in numbers and a decimal point in
    /// prices). Empty keeps ``decimalSeparator``; otherwise its rules apply.
    /// Ignored under ``NumStyles/separatorDetect``, where detection governs the whole token.
    public var currencyDecimalSeparator: String { symbol(at: 30, capacity: Self.maxSymbolBytes) }
    /// The group separator of a currency amount, likewise; empty keeps ``groupSeparator``.
    /// The pair in force for a currency amount must differ, as the number pair must.
    public var currencyGroupSeparator: String { symbol(at: 39, capacity: Self.maxSymbolBytes) }
    /// A further spelling of the minus sign, read wherever ASCII `-` is — ahead of the
    /// digits, before or after a leading currency symbol, and after the digits under
    /// ``NumStyles/trailingSign`` (U+2212 in Estonian and Basque; Arabic-script locales
    /// write an Arabic letter mark ahead of `-`). Empty declares none; `-` is read whatever
    /// is declared. 1 to 8 UTF-8 bytes, with no ASCII digit or white space in it. Two
    /// signs in one number, in either spelling, are malformed at the second.
    public var minusSign: String { symbol(at: 48, capacity: Self.maxSymbolBytes) }
    /// A further spelling of the plus sign, likewise beside ASCII `+`.
    public var plusSign: String { symbol(at: 57, capacity: Self.maxSymbolBytes) }
    /// A further spelling of the percent symbol, read as a trailing `%` is while
    /// ``NumStyles/percent`` is in ``styles`` (`٪` in Arabic-script locales, `％` fullwidth).
    /// Empty declares none; `%` is read whatever is declared. Same rules as the signs. Real
    /// and decimal doors only — an integer door never reads a percent.
    public var percentSymbol: String { symbol(at: 66, capacity: Self.maxSymbolBytes) }
    /// The zero of a set of native digits read beside ASCII `0`–`9` wherever digits are —
    /// the integer part, the fraction, the exponent; a radix-prefixed body stays ASCII —
    /// as the ten code points from it up: U+0660 declares the Arabic-Indic `٠`–`٩`, U+FF10
    /// the fullwidth `０`–`９`. `nil` declares none. Not ASCII, and the start of a digit
    /// block: a code point whose value modulo 64 is at most 54, which every decimal-digit
    /// block Unicode defines satisfies.
    public var digitZero: Unicode.Scalar? { raw.2 == 0 ? nil : Unicode.Scalar(raw.2) }

    // The format laid out as the native `RawNumFormat` carries it, the size word included,
    // and the whole of the value: every field above reads out of it, so a format is 92 plain
    // bytes — a door call, and the locale cache, copy it with no string to retain, and two
    // formats that declare the same thing are the same bytes (the padding is always zero).
    let raw: Interop.RawNumFormat

    /// A symbol field of the layout as text: the bytes up to the length after the field. At
    /// most sixteen bytes, so the string is always small and nothing is allocated.
    private func symbol(at offset: Int, capacity: Int) -> String {
        withUnsafeBytes(of: raw) { bytes in
            String(decoding: bytes[offset..<(offset + Int(bytes[offset + capacity]))], as: UTF8.self)
        }
    }

    /// The inline capacity of the native format's currency field, in UTF-8 bytes.
    static let maxCurrencySymbolBytes = Interop.currencyMaxBytes
    /// The inline capacity of every other symbol field — the separators, the signs and the
    /// percent symbol — in UTF-8 bytes.
    static let maxSymbolBytes = Interop.symbolMaxBytes

    /// Declares a format from its separators and leniences, and whatever else the culture
    /// spells differently from ASCII. `grouping` is how group separators are read and defaults to
    /// ``Grouping/western``: `12,34` and `1,23,456` are malformed at the group, where before 1.1
    /// they read as 1234 and 123456 (declare ``Grouping/indian`` for `1,23,456`). Every field after `grouping` defaults to nothing declared, which reads
    /// exactly the ASCII default.
    ///
    /// - Precondition: `styles` sets no bit the core doesn't define (a grouping policy is
    ///   declared with `grouping`, never inside `styles`); the separators are each 1 to 8 UTF-8 bytes with no ASCII digit and
    ///   differ from each other; the currency separators are empty or the same, and the
    ///   pair in force differs; neither decimal separator, the currency one included,
    ///   contains white space (Unicode `White_Space`, U+00A0 included); the signs and the
    ///   percent symbol are empty or 1 to 8 bytes with no ASCII digit or white space;
    ///   `currencySymbol` is at most 16 UTF-8 bytes with no ASCII digit and no white space
    ///   at either end;
    ///   `digitZero` is `nil` or a non-ASCII scalar whose value modulo 64 is at most 54. Any
    ///   of these is a caller bug, not a data verdict — the core refuses the same layouts as
    ///   a contract violation, and checking here means it never sees one from well-typed
    ///   code.
    public init(
        decimalSeparator: String, groupSeparator: String, styles: NumStyles, grouping: Grouping = .western,
        currencySymbol: String = "", currencyDecimalSeparator: String = "", currencyGroupSeparator: String = "",
        minusSign: String = "", plusSign: String = "", percentSymbol: String = "", digitZero: Unicode.Scalar? = nil
    ) {
        let flags = styles.rawValue | grouping.rawValue
        precondition(Self.refusal(ofFlags: flags) == nil, Self.refusal(ofFlags: flags) ?? "")
        precondition(
            styles.rawValue & Grouping.mask == 0, "A grouping policy is declared with Grouping, never in the styles.")
        precondition(
            Self.isDeclarable(separator: decimalSeparator),
            "Decimal separator '\(decimalSeparator)' must be 1 to \(Self.maxSymbolBytes) UTF-8 bytes "
                + "with no ASCII digit")
        precondition(
            Self.isDeclarable(separator: groupSeparator),
            "Group separator '\(groupSeparator)' must be 1 to \(Self.maxSymbolBytes) UTF-8 bytes with no ASCII digit")
        precondition(
            decimalSeparator != groupSeparator,
            "Decimal and group separators must differ; both are '\(decimalSeparator)'")
        precondition(
            currencyDecimalSeparator.isEmpty || Self.isDeclarable(separator: currencyDecimalSeparator),
            "Currency decimal separator '\(currencyDecimalSeparator)' must be 1 to \(Self.maxSymbolBytes) UTF-8 bytes "
                + "with no ASCII digit")
        precondition(
            currencyGroupSeparator.isEmpty || Self.isDeclarable(separator: currencyGroupSeparator),
            "Currency group separator '\(currencyGroupSeparator)' must be 1 to \(Self.maxSymbolBytes) UTF-8 bytes "
                + "with no ASCII digit")
        let currencyDecimal = currencyDecimalSeparator.isEmpty ? decimalSeparator : currencyDecimalSeparator
        let currencyGroup = currencyGroupSeparator.isEmpty ? groupSeparator : currencyGroupSeparator
        precondition(
            currencyDecimal != currencyGroup,
            "Currency decimal and group separators must differ; both are '\(currencyDecimal)'")
        precondition(
            Self.isDeclarable(decimalSeparator: decimalSeparator),
            "Decimal separator '\(decimalSeparator)' must not contain white space")
        precondition(
            currencyDecimalSeparator.isEmpty || Self.isDeclarable(decimalSeparator: currencyDecimalSeparator),
            "Currency decimal separator '\(currencyDecimalSeparator)' must not contain white space")
        precondition(
            Self.isDeclarable(sign: minusSign),
            "Minus sign '\(minusSign)' must be 1 to \(Self.maxSymbolBytes) UTF-8 bytes with no ASCII digit "
                + "or white space")
        precondition(
            Self.isDeclarable(sign: plusSign),
            "Plus sign '\(plusSign)' must be 1 to \(Self.maxSymbolBytes) UTF-8 bytes with no ASCII digit "
                + "or white space")
        precondition(
            Self.isDeclarable(sign: percentSymbol),
            "Percent symbol '\(percentSymbol)' must be 1 to \(Self.maxSymbolBytes) UTF-8 bytes with no ASCII digit "
                + "or white space")
        precondition(
            Self.isDeclarable(currencySymbol: currencySymbol),
            "Currency symbol '\(currencySymbol)' must be at most \(Self.maxCurrencySymbolBytes) UTF-8 bytes "
                + "with no ASCII digit, and must not start or end with white space")
        precondition(
            digitZero.map(Self.isDeclarable(digitZero:)) ?? true,
            "Digit zero U+\(String(digitZero?.value ?? 0, radix: 16, uppercase: true)) must be non-ASCII and start a "
                + "digit block (its value modulo 64 at most 54)")
        // The layout, field by field at the offsets rust/src/abi.rs pins: the three words
        // are stored natively, as the core reads them; a symbol is its bytes then its length.
        var raw = Interop.zeroRawFormat
        withUnsafeMutableBytes(of: &raw) { bytes in
            bytes.storeBytes(of: UInt32(Interop.rawFormatSize), toByteOffset: 0, as: UInt32.self)
            bytes.storeBytes(of: styles.rawValue | grouping.rawValue, toByteOffset: 4, as: UInt32.self)
            bytes.storeBytes(of: digitZero?.value ?? 0, toByteOffset: 8, as: UInt32.self)
            Interop.store(decimalSeparator, in: bytes, at: 12, capacity: Self.maxSymbolBytes)
            Interop.store(groupSeparator, in: bytes, at: 21, capacity: Self.maxSymbolBytes)
            Interop.store(currencyDecimalSeparator, in: bytes, at: 30, capacity: Self.maxSymbolBytes)
            Interop.store(currencyGroupSeparator, in: bytes, at: 39, capacity: Self.maxSymbolBytes)
            Interop.store(minusSign, in: bytes, at: 48, capacity: Self.maxSymbolBytes)
            Interop.store(plusSign, in: bytes, at: 57, capacity: Self.maxSymbolBytes)
            Interop.store(percentSymbol, in: bytes, at: 66, capacity: Self.maxSymbolBytes)
            Interop.store(currencySymbol, in: bytes, at: 75, capacity: Self.maxCurrencySymbolBytes)
        }
        self.raw = raw
    }

    /// Declares a format from one-character separators — the shape every format declared
    /// before separators could be wider than a character, kept for a caller holding
    /// scalars. Everything else is nothing declared; see ``init(decimalSeparator:groupSeparator:styles:currencySymbol:currencyDecimalSeparator:currencyGroupSeparator:minusSign:plusSign:percentSymbol:digitZero:)``
    /// for the rules; `grouping` defaults to ``Grouping/western`` as there.
    public init(
        decimalSeparator: Unicode.Scalar, groupSeparator: Unicode.Scalar, styles: NumStyles,
        grouping: Grouping = .western, currencySymbol: String = ""
    ) {
        self.init(
            decimalSeparator: String(decimalSeparator), groupSeparator: String(groupSeparator), styles: styles,
            grouping: grouping, currencySymbol: currencySymbol)
    }

    /// This format with `styles` in place of its leniences: its grouping policy, separators and
    /// every other field kept as they are.
    ///
    /// - Precondition: `styles` sets no bit the core doesn't define, as ``init(decimalSeparator:groupSeparator:styles:grouping:currencySymbol:currencyDecimalSeparator:currencyGroupSeparator:minusSign:plusSign:percentSymbol:digitZero:)``.
    public func withStyles(_ styles: NumStyles) -> NumFormat {
        let refusal = Self.refusal(ofFlags: styles.rawValue)
        precondition(refusal == nil, refusal ?? "")
        precondition(
            styles.rawValue & Grouping.mask == 0, "A grouping policy is declared with Grouping, never in the styles.")
        return replacingFlags(styles.rawValue | grouping.rawValue)
    }

    /// This format with `grouping` in place of its policy (replaced, never combined with the
    /// current one): its leniences, separators and every other field kept as they are.
    public func withGrouping(_ grouping: Grouping) -> NumFormat {
        replacingFlags(styles.rawValue | grouping.rawValue)
    }

    private func replacingFlags(_ flags: UInt32) -> NumFormat {
        var raw = raw
        withUnsafeMutableBytes(of: &raw) { $0.storeBytes(of: flags, toByteOffset: 4, as: UInt32.self) }
        return NumFormat(raw: raw)
    }

    private init(raw: Interop.RawNumFormat) {
        self.raw = raw
    }

    /// Two formats are equal when they declare the same thing, which is when their layouts
    /// are the same bytes.
    public static func == (lhs: NumFormat, rhs: NumFormat) -> Bool {
        withUnsafeBytes(of: lhs.raw) { left in withUnsafeBytes(of: rhs.raw) { right in left.elementsEqual(right) } }
    }

    /// Hashes what ``==`` compares: the layout's bytes.
    public func hash(into hasher: inout Hasher) {
        withUnsafeBytes(of: raw) { hasher.combine(bytes: $0) }
    }

    /// The core's rule for the `flags` word, which the 1.1 check applies to every declaration:
    /// the retired bit 8 names no policy, and a bit no flag or policy defines (bit 10 and up) is
    /// refused. `nil` when the word is declarable, else the message
    /// the constructor traps with.
    static func refusal(ofFlags flags: UInt32) -> String? {
        if flags & Grouping.retired != 0 {
            return "Bit 8 names no grouping policy; declare Grouping.western or .indian."
        }
        if flags & ~(NumStyles.definedBits | Grouping.mask) != 0 {
            return
                "Style bits 0x\(String(flags & ~(NumStyles.definedBits | Grouping.mask), radix: 16, uppercase: true)) "
                + "are not defined; the styles are bits 0 to 7, and a grouping policy is declared with Grouping."
        }
        return nil
    }

    /// The native format's rule for a number separator: 1 to 8 UTF-8 bytes with no ASCII
    /// digit anywhere in it — a digit is a digit before it is anything else, so such a
    /// separator could never be matched.
    static func isDeclarable(separator symbol: String) -> Bool {
        (1...maxSymbolBytes).contains(symbol.utf8.count) && !symbol.utf8.contains(where: isAsciiDigit)
    }

    /// The native format's rule for a decimal separator, the currency one included: a
    /// separator's rules, and no white space either. A numeral admits white space only as a
    /// declared group separator, so a decimal separator holding any could never be matched.
    static func isDeclarable(decimalSeparator symbol: String) -> Bool {
        isDeclarable(separator: symbol) && !containsWhiteSpace(symbol)
    }

    /// The native format's rule for a sign or the percent symbol, which sit flush against
    /// the digits: a separator's rules, and no white space either. Empty (none declared)
    /// passes.
    static func isDeclarable(sign symbol: String) -> Bool {
        symbol.isEmpty || (isDeclarable(separator: symbol) && !containsWhiteSpace(symbol))
    }

    /// The native format's own rule for a currency symbol: no more than 16 UTF-8 bytes, no
    /// ASCII digit, and no white space at either edge — inside is fine (CLDR writes the CFA
    /// franc `F` U+202F `CFA`). Empty (no symbol) passes.
    static func isDeclarable(currencySymbol symbol: String) -> Bool {
        let scalars = symbol.unicodeScalars
        return symbol.utf8.count <= maxCurrencySymbolBytes && !symbol.utf8.contains(where: isAsciiDigit)
            && !(scalars.first.map(isWhiteSpace) ?? false) && !(scalars.last.map(isWhiteSpace) ?? false)
    }

    /// The native format's rule for a digit zero: not ASCII, and a code point whose ten
    /// successors share its UTF-8 prefix (its value modulo 64 at most 54), which is how the
    /// core recognises a native digit from its last byte and what every decimal-digit block
    /// Unicode defines satisfies.
    static func isDeclarable(digitZero zero: Unicode.Scalar) -> Bool {
        zero.value >= 0x80 && zero.value % 64 <= 54
    }

    static func isAsciiDigit(_ byte: UInt8) -> Bool {
        (0x30...0x39).contains(byte)
    }

    /// Whether `scalar` has the Unicode `White_Space` property: the 25 code points of
    /// PropList.txt (Unicode 17.0.0), which is what the core trims and what a number or a
    /// date may not hold inside. U+200B, U+FEFF and U+180E are not white space. Held to the
    /// corpus's `formats.json`.
    static func isWhiteSpace(_ scalar: Unicode.Scalar) -> Bool {
        switch scalar.value {
        case 0x0009...0x000D, 0x0020, 0x0085, 0x00A0, 0x1680, 0x2000...0x200A, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000:
            true
        default:
            false
        }
    }

    static func containsWhiteSpace(_ text: String) -> Bool {
        text.unicodeScalars.contains(where: isWhiteSpace)
    }

    /// The invariant profile — `.` decimal, `,` grouping, every lenience on, Western
    /// grouping, nothing else declared. (Before 1.1 it read a group separator anywhere between
    /// two digits, `12,34` and `1,23,456` included: Western refuses both, and Indian reads the
    /// second.)
    public static let invariant = NumFormat(decimalSeparator: ".", groupSeparator: ",", styles: .all)

    /// The detection profile — every lenience on, `.`/`,` roles resolved per input by
    /// ``NumStyles/separatorDetect``'s structural rules, and Western grouping.
    public static let detect = NumFormat(
        decimalSeparator: ".", groupSeparator: ",", styles: [.all, .separatorDetect])

    /// Derives a format from a locale's number symbols, every lenience on — read the way
    /// Foundation's own number parsing reads them, through a `NumberFormatter` in the
    /// decimal style, and the locale's numbering system: the separators and the currency
    /// symbol, as before, and now the minus, plus and percent signs, the separators a
    /// currency amount is written with, the negative pattern (a suffix sign, as an `n-`
    /// culture writes, sets ``NumStyles/trailingSign``) and the native digits. See
    /// ``from(formatter:)`` for what is read from where.
    ///
    /// A symbol the locale spells as the ASCII default is not declared, since the core reads
    /// that anyway; nor is one the native format cannot carry (each field has its rules), nor
    /// the digits of a numbering system this binding has no zero for — a locale is process or
    /// user state, and no cast should trap on it. The separators fall back to the invariant
    /// ones where the locale does not say. ICU's symbols are declared as spelled, bidi marks
    /// included (an Arabic-script locale's minus is an Arabic letter mark then `-`, which is
    /// what its own formatter writes), so a locale's own formatted output parses under its
    /// format, and so does the plain ASCII spelling.
    ///
    /// The grouping policy follows the locale's number pattern (see ``from(formatter:)``): all
    /// Indian is Indian, anything else Western, with ``NumStyles/grouping`` cleared only when
    /// neither the number nor the currency pattern groups. The patterns are read, never the
    /// locale's name.
    ///
    /// Every lenience is on, radix prefixes and percent included; narrow the styles, or
    /// declare a `NumFormat` outright, for anything stricter. Deriving builds a
    /// `NumberFormatter`, which is tens of microseconds on Foundation; a hot loop derives
    /// once and passes the format.
    public static func from(locale: Locale) -> NumFormat {
        let formatter = NumberFormatter()
        formatter.locale = locale
        formatter.numberStyle = .decimal
        return from(formatter: formatter)
    }

    /// Derives a format from a `NumberFormatter` a caller already configured — the user's
    /// own, overrides and all — as ``from(locale:)`` does from the locale's: its
    /// `decimalSeparator` and `groupingSeparator`, `currencySymbol`, `minusSign`, `plusSign`
    /// and `percentSymbol`, `currencyDecimalSeparator` and `currencyGroupingSeparator` (each
    /// declared only where it differs from the number one, which is what the core keeps for
    /// an empty field), a non-empty `negativeSuffix` with an empty `negativePrefix` as
    /// ``NumStyles/trailingSign`` (the `n-` and `n -` patterns; parentheses are their own
    /// lenience), and its locale's `numberingSystem` for ``digitZero`` — `latn` declares
    /// none, `arab`, `arabext`, `fullwide`, `thai`, `mymr`, `khmr`, `laoo`, `tibt`, `mong`
    /// and the Indic scripts declare their block, anything else declares none. Meant for a
    /// decimal-style formatter: in the currency style the suffixes are the symbol, not a sign.
    ///
    /// **Grouping.** One format has one policy and one ``NumStyles/grouping`` flag for number and
    /// currency values alike. The policy follows this formatter's own number pattern
    /// (`groupingSize`, `secondaryGroupingSize` and `usesGroupingSeparator`). A pattern groups when
    /// `usesGroupingSeparator` is on and the primary size is above 0; a secondary size of 0 repeats
    /// the primary one (`NumberFormatter`'s own meaning), so (3, 0) groups by threes and Toki Pona's
    /// (2, 0) by twos.
    ///
    /// - a group of three then twos (`#,##,##0`): ``Grouping/indian``;
    /// - anything else: ``Grouping/western``, so `tok`'s grouped output is refused, never misread
    ///   (`docs/grouping.md`), and a locale with a Western number and an Indian currency pattern
    ///   is Western;
    /// - ``NumStyles/grouping`` is cleared only when neither the number pattern nor the currency
    ///   pattern of the formatter's locale groups (`en_US_POSIX`), and then a group separator is
    ///   malformed.
    ///
    /// When only one of the two patterns groups (`en_BG`'s currency doesn't), the flag stays for
    /// the other: the format can't prohibit grouping in a currency amount alone, so an ungrouped
    /// value stays valid and a grouped one the platform never writes is read. A currency pattern
    /// of the other policy is refused until the column says ``withGrouping(_:)``. The currency
    /// pattern comes from a second `NumberFormatter` on the formatter's locale, so deriving costs
    /// two.
    public static func from(formatter: NumberFormatter) -> NumFormat {
        func separator(_ symbol: String?, accepting: (String) -> Bool, orInvariant fallback: String) -> String {
            symbol.flatMap { accepting($0) ? $0 : nil } ?? fallback
        }
        // A sign or the percent symbol is a further spelling: ASCII is read regardless, so
        // declaring it would say nothing.
        func spelling(_ symbol: String?, beside ascii: String) -> String {
            symbol.flatMap { $0 != ascii && isDeclarable(sign: $0) ? $0 : nil } ?? ""
        }
        // The currency separators are read in place of the number ones, so an equal one is
        // the number one and declaring it would say nothing either.
        func currencySeparator(_ symbol: String?, accepting: (String) -> Bool, beside number: String) -> String {
            symbol.flatMap { $0 != number && accepting($0) ? $0 : nil } ?? ""
        }
        let decimal = separator(
            formatter.decimalSeparator, accepting: isDeclarable(decimalSeparator:), orInvariant: ".")
        var group = separator(formatter.groupingSeparator, accepting: isDeclarable(separator:), orInvariant: ",")
        if group == decimal {
            group = decimal == "," ? "." : ","
        }
        // A pair that collapses to the same symbol is no pair.
        var currencyDecimal = currencySeparator(
            formatter.currencyDecimalSeparator, accepting: isDeclarable(decimalSeparator:), beside: decimal)
        var currencyGroup = currencySeparator(
            formatter.currencyGroupingSeparator, accepting: isDeclarable(separator:), beside: group)
        if (currencyDecimal.isEmpty ? decimal : currencyDecimal) == (currencyGroup.isEmpty ? group : currencyGroup) {
            currencyDecimal = ""
            currencyGroup = ""
        }
        let currency = formatter.currencySymbol.flatMap { isDeclarable(currencySymbol: $0) ? $0 : nil } ?? ""
        var styles = NumStyles.all
        if (formatter.negativePrefix ?? "").isEmpty && !(formatter.negativeSuffix ?? "").isEmpty {
            styles.insert(.trailingSign)
        }
        let currencyFormatter = NumberFormatter()
        currencyFormatter.locale = formatter.locale
        currencyFormatter.numberStyle = .currency
        let (grouping, groups) = Self.grouping(
            number: GroupingShape(of: formatter), currency: GroupingShape(of: currencyFormatter))
        if !groups {
            styles.remove(.grouping)
        }
        return NumFormat(
            decimalSeparator: decimal, groupSeparator: group, styles: styles, grouping: grouping,
            currencySymbol: currency, currencyDecimalSeparator: currencyDecimal, currencyGroupSeparator: currencyGroup,
            minusSign: spelling(formatter.minusSign, beside: "-"),
            plusSign: spelling(formatter.plusSign, beside: "+"),
            percentSymbol: spelling(formatter.percentSymbol, beside: "%"),
            digitZero: digitZero(ofNumberingSystem: formatter.locale.numberingSystem.identifier))
    }

    /// The shape of a platform's grouping pattern.
    enum GroupingShape: Equatable {
        /// The pattern doesn't group.
        case none
        /// Groups of three.
        case western
        /// A group of three, then twos.
        case indian
        /// Any other group sizes.
        case other

        /// Reads a `NumberFormatter`'s pattern: it groups when it uses a grouping separator with a
        /// primary size above 0, and a secondary size of 0 repeats the primary one.
        init(of formatter: NumberFormatter) {
            guard formatter.usesGroupingSeparator, formatter.groupingSize > 0 else {
                self = .none
                return
            }
            let primary = formatter.groupingSize
            let secondary = formatter.secondaryGroupingSize == 0 ? primary : formatter.secondaryGroupingSize
            switch (primary, secondary) {
            case (3, 3): self = .western
            case (3, 2): self = .indian
            default: self = .other
            }
        }
    }

    /// The one policy and the one `grouping` flag a number pattern and a currency pattern share:
    /// the number pattern's all-Indian is Indian, anything else Western, and the flag is cleared
    /// only when neither groups.
    static func grouping(number: GroupingShape, currency: GroupingShape) -> (grouping: Grouping, groups: Bool) {
        (number == .indian ? .indian : .western, number != .none || currency != .none)
    }

    /// The zero digit of a numbering system, by ICU's identifier, for the systems whose ten
    /// digits are one block of code points — which is every decimal one Unicode defines;
    /// `latn` is ASCII and declares nothing, `hanidec` is not a block, and an algorithmic
    /// system has no digits. The common ones; anything else declares none.
    static func digitZero(ofNumberingSystem identifier: String) -> Unicode.Scalar? {
        switch identifier {
        case "arab": "\u{0660}"
        case "arabext": "\u{06F0}"
        case "beng": "\u{09E6}"
        case "deva": "\u{0966}"
        case "fullwide": "\u{FF10}"
        case "gujr": "\u{0AE6}"
        case "guru": "\u{0A66}"
        case "khmr": "\u{17E0}"
        case "knda": "\u{0CE6}"
        case "laoo": "\u{0ED0}"
        case "mlym": "\u{0D66}"
        case "mong": "\u{1810}"
        case "mymr": "\u{1040}"
        case "orya": "\u{0B66}"
        case "tamldec": "\u{0BE6}"
        case "telu": "\u{0C66}"
        case "thai": "\u{0E50}"
        case "tibt": "\u{0F20}"
        default: nil
        }
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
        return from(formatter: formatter)
    }

    /// ``from(locale:)``'s reading of a formatter's short-date pattern. The formatter is
    /// the caller's: ``DateFormat/from(locale:)`` reads the day periods off the same one.
    static func from(formatter: DateFormatter) -> DateOrder {
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

/// Caller-declared civil date-time notation for ``Cast/dateTime(_:format:)-swift.type.method``:
/// the field ``order`` of the date part, and the day-period designators a culture writes
/// beside a 12-hour clock (`오전`/`오후`, `vorm.`/`nachm.`, `a. m.`/`p. m.`), read beside the
/// English `AM`/`PM`, which are read whatever is declared — after the clock (`3:04 nachm.`)
/// or ahead of it (`오후 3:04`), with any run of white space, or none, between. A declared designator matches its exact
/// bytes; the English pair matches ASCII case-insensitively. Declared, never looked up, the
/// way ``NumFormat`` is: ``from(locale:)`` fills it from the locale's own data, and
/// ``Cast/dateTime(_:order:)-swift.type.method`` is this with nothing declared.
public struct DateFormat: Hashable, Sendable {
    /// The field order of the date part.
    public let order: DateOrder
    /// The designator of the first half of the day; empty declares none beside `AM`. 1 to 16
    /// UTF-8 bytes, with no ASCII digit and no white space at either end — the grammar
    /// around a designator reads that; a space inside is fine (`a. m.`).
    public var amSymbol: String { designator(at: 4) }
    /// The designator of the second half of the day; empty declares none beside `PM`. Same
    /// rules as ``amSymbol``.
    public var pmSymbol: String { designator(at: 21) }

    // The designators laid out as the native `RawDayPeriods` carries them, and the whole of
    // the value beside the order: both designators read out of it, so a format is plain
    // bytes. A door hands the core a pointer to a copy, or null when nothing is declared.
    let raw: Interop.RawDayPeriods

    /// A designator field of the layout as text: the bytes up to the length after the field.
    private func designator(at offset: Int) -> String {
        withUnsafeBytes(of: raw) { bytes in
            String(decoding: bytes[offset..<(offset + Int(bytes[offset + Self.maxDesignatorBytes]))], as: UTF8.self)
        }
    }

    /// The inline capacity of a native designator field, in UTF-8 bytes.
    static let maxDesignatorBytes = Interop.designatorMaxBytes

    /// Declares the order and, optionally, the designators.
    ///
    /// - Precondition: each designator is empty or 1 to 16 UTF-8 bytes with no ASCII digit
    ///   and no white space at either end — a caller bug otherwise, not a data verdict.
    public init(order: DateOrder, amSymbol: String = "", pmSymbol: String = "") {
        precondition(
            Self.isDeclarable(designator: amSymbol),
            "AM designator '\(amSymbol)' must be 1 to \(Self.maxDesignatorBytes) UTF-8 bytes with no ASCII digit and "
                + "no white space at either end")
        precondition(
            Self.isDeclarable(designator: pmSymbol),
            "PM designator '\(pmSymbol)' must be 1 to \(Self.maxDesignatorBytes) UTF-8 bytes with no ASCII digit and "
                + "no white space at either end")
        self.order = order
        var raw = Interop.zeroRawDayPeriods
        withUnsafeMutableBytes(of: &raw) { bytes in
            bytes.storeBytes(of: UInt32(Interop.rawDayPeriodsSize), toByteOffset: 0, as: UInt32.self)
            Interop.store(amSymbol, in: bytes, at: 4, capacity: Self.maxDesignatorBytes)
            Interop.store(pmSymbol, in: bytes, at: 21, capacity: Self.maxDesignatorBytes)
        }
        self.raw = raw
    }

    /// Whether either designator is declared (its length byte is nonzero) — when neither
    /// is, the door passes the core null, exactly the order-only door.
    var declaresPeriods: Bool {
        withUnsafeBytes(of: raw) { bytes in bytes[20] != 0 || bytes[37] != 0 }
    }

    /// Two formats are equal when they declare the same thing, which is when their order and
    /// their layouts' bytes are the same.
    public static func == (lhs: DateFormat, rhs: DateFormat) -> Bool {
        lhs.order == rhs.order
            && withUnsafeBytes(of: lhs.raw) { left in
                withUnsafeBytes(of: rhs.raw) { right in left.elementsEqual(right) }
            }
    }

    /// Hashes what ``==`` compares: the order and the layout's bytes.
    public func hash(into hasher: inout Hasher) {
        hasher.combine(order)
        withUnsafeBytes(of: raw) { hasher.combine(bytes: $0) }
    }

    /// The native rule for a designator: 1 to 16 UTF-8 bytes, no ASCII digit, and no white
    /// space at either end. Empty (none declared) passes.
    static func isDeclarable(designator symbol: String) -> Bool {
        guard !symbol.isEmpty else { return true }
        let scalars = symbol.unicodeScalars
        return symbol.utf8.count <= maxDesignatorBytes && !symbol.utf8.contains(where: NumFormat.isAsciiDigit)
            && !NumFormat.isWhiteSpace(scalars.first!) && !NumFormat.isWhiteSpace(scalars.last!)
    }

    /// Derives the format from a locale's own data: the field order of its short-date
    /// pattern, as ``DateOrder/from(locale:)`` reads it, and its day-period designators, a
    /// `DateFormatter`'s `amSymbol` and `pmSymbol`. The pair is declared together or not at
    /// all: a locale whose designators the native field cannot carry (longer than 16 bytes)
    /// declares none, and its text is read with the English `AM`/`PM` only —
    /// a locale is process/user state, and no cast should trap on it. Deriving builds a
    /// `DateFormatter`, which is tens of microseconds on Foundation; a hot loop derives once
    /// and passes the format.
    public static func from(locale: Locale) -> DateFormat {
        let formatter = DateFormatter()
        formatter.locale = locale
        let order = DateOrder.from(formatter: formatter)
        let am = formatter.amSymbol ?? ""
        let pm = formatter.pmSymbol ?? ""
        guard !am.isEmpty, !pm.isEmpty, isDeclarable(designator: am), isDeclarable(designator: pm) else {
            return DateFormat(order: order)
        }
        return DateFormat(order: order, amSymbol: am, pmSymbol: pm)
    }
}

/// The declared unit of a Unix-epoch value — no magnitude guessing, ever. Values match the
/// native core's discriminants.
public enum UnixPrecision: UInt32, Sendable {
    /// Whole seconds since the Unix epoch.
    case seconds = 1
    /// Milliseconds since the Unix epoch.
    case milliseconds = 2
    /// Microseconds since the Unix epoch.
    case microseconds = 3
    /// Nanoseconds since the Unix epoch.
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
