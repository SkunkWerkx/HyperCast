import Foundation
import HyperCastCore

/// The closed set of types a ``Constraints`` can be bound to, one per checked door: `Int8`
/// to `Int64`, `UInt8` to `UInt64`, `Float`, `Double`, `Decimal` and `String` (the text door).
/// The conformances are this binding's to declare, and a caller's own type has no door, so
/// conforming anything else is unsupported (declaring constraints for it is a precondition
/// failure).
public protocol CheckedCastTarget {}

extension Int8: CheckedCastTarget {}
extension Int16: CheckedCastTarget {}
extension Int32: CheckedCastTarget {}
extension Int64: CheckedCastTarget {}
extension UInt8: CheckedCastTarget {}
extension UInt16: CheckedCastTarget {}
extension UInt32: CheckedCastTarget {}
extension UInt64: CheckedCastTarget {}
extension Float: CheckedCastTarget {}
extension Double: CheckedCastTarget {}
extension Decimal: CheckedCastTarget {}
extension String: CheckedCastTarget {}

/// A bound, step or `enum` entry of ``Constraints``, as the caller wrote it: an exact
/// `Decimal` (an integer literal is one), or text. Never a binary float — there is no float
/// literal and no initializer from `Double`, so `0.1` cannot be declared inexactly by
/// accident; write `"0.1"`.
///
/// Text at a numeric door is a decimal in invariant notation — an optional sign, digits with
/// an optional `.` and fraction, an optional exponent (`"0.000001"`, `"-90"`, `"1e-6"`) —
/// converted exactly by the native core's own conversion, as a `Decimal` is through its
/// `description`: a value that does not fit 96 bits at a scale of 28 or less is refused
/// rather than rounded. Text at the text door is the `enum` entry itself. Foundation's
/// `Decimal(0.1)` goes through a `Double`; build a `Decimal` from text
/// (`Decimal(string: "0.1")`), or pass the text.
public struct ConstraintValue: Equatable, Sendable, ExpressibleByStringLiteral, ExpressibleByIntegerLiteral,
    CustomStringConvertible
{
    enum Kind: Equatable, Sendable {
        case number(Decimal)
        case text(String)
    }

    let kind: Kind

    /// An exact decimal.
    public init(_ number: Decimal) {
        kind = .number(number)
    }

    /// Text: a decimal in invariant notation at a numeric door, an entry at the text door.
    public init(_ text: String) {
        kind = .text(text)
    }

    /// Text, from a string literal: `"0.000001"`, or `"Male"` at the text door.
    public init(stringLiteral text: String) {
        kind = .text(text)
    }

    /// A whole number, from an integer literal: `-90`.
    public init(integerLiteral number: Int) {
        kind = .number(Decimal(number))
    }

    /// The decimal's invariant text, or the text as written.
    public var description: String {
        switch kind {
        case .number(let number): number.description
        case .text(let text): text
        }
    }
}

/// A declaration of validation constraints — the OpenAPI Schema Object's keywords — bound to
/// the door its type names: `Constraints<Double>` for the checked
/// ``Cast/f64(_:format:constraints:)-swift.type.method``, `Constraints<String>` for
/// ``Cast/text(_:constraints:)-swift.type.method``. Immutable, and judged once, by the native
/// core, when it is built: every valid OpenAPI 3.2 declaration is accepted, unsatisfiable ones
/// included (crossed bounds, an empty `enum`), and one the core refuses is a caller bug, so
/// every checked cast after that only judges data.
///
/// The numeric doors (`Int8` to `UInt64`, `Float`, `Double` and `Decimal`) read `minimum`,
/// `exclusiveMinimum`, `maximum`, `exclusiveMaximum` (four independent keywords, any of them
/// together), `multipleOf` with a ``Rounding``, and `enum`; the text door (`String`)
/// `minLength`, `maxLength` and `enum`, with two extensions of its own, `trim` and
/// `caseSensitive`. A keyword for the other type is ignored, as JSON Schema ignores it. Bounds,
/// steps and numeric entries are ``ConstraintValue``s — an exact `Decimal` or its text, never a
/// binary float.
///
/// The value is judged in the core, in this order, the first failure winning: the door casts
/// it (a ``Fault``, exactly as the plain door's: the input is not the type); text is trimmed of
/// Unicode `White_Space` when `trim` says so; `multipleOf`, on the exact decimal, rounding when
/// declared; the bounds, on the rounded value; the lengths, in code points; `enum` (numbers
/// compared exactly; text under Unicode simple case folding unless `caseSensitive`, delivering
/// the declared entry). A broken rule is a ``Violation`` carrying the value as cast.
///
/// ```swift
/// let latitude = Constraints<Double>(minimum: -90, maximum: 90, multipleOf: "0.000001", rounding: .midpoint)
/// let sex = Constraints<String>(trim: true, enum: ["Female", "Male", "Other"])
/// ```
public struct Constraints<T: CheckedCastTarget>: Sendable {
    /// The native door code this declaration is bound to.
    let door: UInt32
    /// The declaration in the core's 104-byte layout.
    let raw: Interop.RawConstraints
    /// The `enum` list as the core reads it, empty for none (the layout's flag `256` says
    /// whether one was declared).
    let packed: [UInt8]
    /// At the text door, the declared entries, so a match hands back the caller's own string.
    let entries: [String]?
    /// The `enum` as declared, for ``enum``.
    let declaredEnum: [ConstraintValue]?

    /// Declares the constraints and has the native core judge them for `T`'s door.
    ///
    /// - Parameters:
    ///   - minimum: The smallest value allowed.
    ///   - exclusiveMinimum: A value every value must be above.
    ///   - maximum: The largest value allowed.
    ///   - exclusiveMaximum: A value every value must be below.
    ///   - multipleOf: The positive step every value must be a multiple of.
    ///   - rounding: What a value that is not a multiple becomes; anything but
    ///     ``Rounding/strict`` needs `multipleOf`.
    ///   - minLength: The fewest code points the text may hold.
    ///   - maxLength: The most code points the text may hold.
    ///   - trim: Trim leading and trailing Unicode `White_Space` from the text before judging it.
    ///   - caseSensitive: Match a text `enum` byte for byte instead of under simple case
    ///     folding, for a vocabulary whose case carries meaning (`mm` and `Mm`).
    ///   - entries: The values allowed: numbers at a numeric door (compared exactly to the
    ///     value; one the type cannot hold never matches), strings at the text door (matched
    ///     under simple case folding unless `caseSensitive`; a match delivers the entry as
    ///     declared). With duplicates, the first match wins. `nil` declares no `enum`; an
    ///     empty list declares one no value matches.
    ///
    /// - Precondition: the core accepts the declaration. A refusal is a contract violation, a
    ///   caller bug rather than a data verdict, and traps with a message naming the rule it
    ///   broke (`Constraints refused (code 8): a bound, step or enum entry the door cannot
    ///   hold.`): `trim` or `caseSensitive` at a numeric door; a rounding with `multipleOf` at
    ///   the text door; a `multipleOf` not positive; a rounding without `multipleOf`; a bound,
    ///   step or entry no decimal holds (text that is no decimal, or one past 96 bits or a scale
    ///   of 28), or a rounding at an integer door with a fractional `multipleOf`. Also a length
    ///   below zero or past `UInt32.max`, and a `T` with no checked door. A number among the text
    ///   door's entries is valid and never matches, as JSON Schema's exact equality says.
    public init(
        minimum: ConstraintValue? = nil,
        exclusiveMinimum: ConstraintValue? = nil,
        maximum: ConstraintValue? = nil,
        exclusiveMaximum: ConstraintValue? = nil,
        multipleOf: ConstraintValue? = nil,
        rounding: Rounding = .strict,
        minLength: Int? = nil,
        maxLength: Int? = nil,
        trim: Bool = false,
        caseSensitive: Bool = false,
        enum entries: [ConstraintValue]? = nil
    ) {
        do {
            self = try Self.declare(
                minimum: minimum, exclusiveMinimum: exclusiveMinimum, maximum: maximum,
                exclusiveMaximum: exclusiveMaximum, multipleOf: multipleOf, rounding: rounding,
                minLength: minLength, maxLength: maxLength, trim: trim, caseSensitive: caseSensitive,
                enum: entries)
        } catch {
            preconditionFailure(error.message)
        }
    }

    private init(
        door: UInt32, raw: Interop.RawConstraints, packed: [UInt8], entries: [String]?,
        declaredEnum: [ConstraintValue]?
    ) {
        self.door = door
        self.raw = raw
        self.packed = packed
        self.entries = entries
        self.declaredEnum = declaredEnum
    }

    /// A declaration the binding or the core refused: the core's refusal code, or `nil` for a
    /// rule the binding enforces before the core sees the declaration, and the message the
    /// public initializer traps with.
    struct Refused: Error {
        let code: Int32?
        let message: String
    }

    /// The whole of the public initializer but the trap, so a suite can see a refusal.
    static func declare(
        minimum: ConstraintValue? = nil,
        exclusiveMinimum: ConstraintValue? = nil,
        maximum: ConstraintValue? = nil,
        exclusiveMaximum: ConstraintValue? = nil,
        multipleOf: ConstraintValue? = nil,
        rounding: Rounding = .strict,
        minLength: Int? = nil,
        maxLength: Int? = nil,
        trim: Bool = false,
        caseSensitive: Bool = false,
        enum entries: [ConstraintValue]? = nil
    ) throws(Refused) -> Self {
        let door = try doorOf()
        var flags: UInt32 = 0
        var lower: Cast.OutScratch = (0, 0)
        var upper: Cast.OutScratch = (0, 0)
        var exclusiveLower: Cast.OutScratch = (0, 0)
        var exclusiveUpper: Cast.OutScratch = (0, 0)
        var step: Cast.OutScratch = (0, 0)
        if let minimum {
            lower = try declared(minimum)
            flags |= hasMinimum
        }
        if let exclusiveMinimum {
            exclusiveLower = try declared(exclusiveMinimum)
            flags |= hasExclusiveMinimum
        }
        if let maximum {
            upper = try declared(maximum)
            flags |= hasMaximum
        }
        if let exclusiveMaximum {
            exclusiveUpper = try declared(exclusiveMaximum)
            flags |= hasExclusiveMaximum
        }
        if let multipleOf {
            step = try declared(multipleOf)
            flags |= hasMultipleOf
        }
        let fewest = try length(minLength, "minLength")
        let most = try length(maxLength, "maxLength")
        if minLength != nil { flags |= hasMinLength }
        if maxLength != nil { flags |= hasMaxLength }
        if trim { flags |= trimFlag }
        if caseSensitive { flags |= caseSensitiveFlag }
        if entries != nil { flags |= hasEnum }
        var raw = Interop.zeroRawConstraints
        withUnsafeMutableBytes(of: &raw) { bytes in
            bytes.storeBytes(of: UInt32(Interop.rawConstraintsSize), toByteOffset: 0, as: UInt32.self)
            bytes.storeBytes(of: flags, toByteOffset: 4, as: UInt32.self)
            bytes.storeBytes(of: rounding.code, toByteOffset: 8, as: UInt32.self)
            bytes.storeBytes(of: fewest, toByteOffset: 12, as: UInt32.self)
            bytes.storeBytes(of: most, toByteOffset: 16, as: UInt32.self)
            bytes.storeBytes(of: lower, toByteOffset: 24, as: Cast.OutScratch.self)
            bytes.storeBytes(of: upper, toByteOffset: 40, as: Cast.OutScratch.self)
            bytes.storeBytes(of: step, toByteOffset: 56, as: Cast.OutScratch.self)
            bytes.storeBytes(of: exclusiveLower, toByteOffset: 72, as: Cast.OutScratch.self)
            bytes.storeBytes(of: exclusiveUpper, toByteOffset: 88, as: Cast.OutScratch.self)
        }
        var packed: [UInt8] = []
        var texts: [String]?
        if let entries {
            (packed, texts) = try pack(entries, at: door)
        }
        let code = Cast.checkConstraints(door: door, raw: raw, packed: packed)
        guard code == 0 else { throw Refused(code: code, message: refusalMessage(code)) }
        return Self(door: door, raw: raw, packed: packed, entries: texts, declaredEnum: entries)
    }

    /// The declared `minimum`, or `nil`.
    public var minimum: Decimal? { bound(Self.hasMinimum, at: 24) }
    /// The declared `exclusiveMinimum`, or `nil`.
    public var exclusiveMinimum: Decimal? { bound(Self.hasExclusiveMinimum, at: 72) }
    /// The declared `maximum`, or `nil`.
    public var maximum: Decimal? { bound(Self.hasMaximum, at: 40) }
    /// The declared `exclusiveMaximum`, or `nil`.
    public var exclusiveMaximum: Decimal? { bound(Self.hasExclusiveMaximum, at: 88) }
    /// The declared `multipleOf`, or `nil`.
    public var multipleOf: Decimal? { bound(Self.hasMultipleOf, at: 56) }

    /// What a value that is not a multiple of ``multipleOf`` becomes.
    public var rounding: Rounding {
        let code = word(at: 8)
        return Rounding.allCases.first { $0.code == code } ?? .strict
    }

    /// The declared `minLength`, or `nil`.
    public var minLength: Int? { flags & Self.hasMinLength != 0 ? Int(word(at: 12)) : nil }
    /// The declared `maxLength`, or `nil`.
    public var maxLength: Int? { flags & Self.hasMaxLength != 0 ? Int(word(at: 16)) : nil }
    /// Whether text is trimmed of Unicode `White_Space` before it is judged.
    public var trim: Bool { flags & Self.trimFlag != 0 }
    /// Whether a text `enum` matches byte for byte rather than under simple case folding.
    public var caseSensitive: Bool { flags & Self.caseSensitiveFlag != 0 }
    /// The declared `enum`, each entry as it was passed, in order, duplicates kept; empty for an
    /// empty `enum`, which every value violates, and `nil` when none is declared.
    public var `enum`: [ConstraintValue]? { declaredEnum }

    /// The declared text entry at `index`, the caller's own string.
    func entry(_ index: UInt32) -> String {
        guard let entries, index < entries.count else {
            preconditionFailure(
                "libhypercast matched enum entry \(index), which was never declared — a binding bug, please report it")
        }
        return entries[Int(index)]
    }

    private var flags: UInt32 { word(at: 4) }

    private func word(at offset: Int) -> UInt32 {
        withUnsafeBytes(of: raw) { $0.load(fromByteOffset: offset, as: UInt32.self) }
    }

    private func bound(_ flag: UInt32, at offset: Int) -> Decimal? {
        guard flags & flag != 0 else { return nil }
        return withUnsafeBytes(of: raw) { Interop.decimal(UnsafeRawBufferPointer(rebasing: $0[offset..<offset + 16])) }
    }

    // The flag bits, restated from the header; internal so the suite pins them to it.
    static var hasMinimum: UInt32 { 1 }
    static var hasExclusiveMinimum: UInt32 { 2 }
    static var hasMaximum: UInt32 { 4 }
    static var hasExclusiveMaximum: UInt32 { 8 }
    static var hasMultipleOf: UInt32 { 16 }
    static var hasMinLength: UInt32 { 32 }
    static var hasMaxLength: UInt32 { 64 }
    static var trimFlag: UInt32 { 128 }
    static var hasEnum: UInt32 { 256 }
    static var caseSensitiveFlag: UInt32 { 512 }
    private static var textDoor: UInt32 { 18 }

    /// The core's door code for `T`.
    private static func doorOf() throws(Refused) -> UInt32 {
        if T.self == Int8.self { return 2 }
        if T.self == Int16.self { return 3 }
        if T.self == Int32.self { return 4 }
        if T.self == Int64.self { return 5 }
        if T.self == UInt8.self { return 6 }
        if T.self == UInt16.self { return 7 }
        if T.self == UInt32.self { return 8 }
        if T.self == UInt64.self { return 9 }
        if T.self == Float.self { return 10 }
        if T.self == Double.self { return 11 }
        if T.self == String.self { return textDoor }
        if T.self == Decimal.self { return 19 }
        throw Refused(code: nil, message: "No checked door casts to \(T.self).")
    }

    /// A bound, step or numeric entry through the core's own conversion: the exact decimal, or
    /// the core's refusal 8.
    private static func declared(_ value: ConstraintValue) throws(Refused) -> Cast.OutScratch {
        let text: String =
            switch value.kind {
            case .number(let number): number.description
            case .text(let text): text
            }
        guard let decimal = declaredDecimal(text) else {
            throw Refused(code: 8, message: refusalMessage(8))
        }
        return decimal
    }

    /// The core's conversion of a declaration's decimal text (`cast_declared_decimal`), the
    /// one conversion every binding calls: the decimal in the core's 16-byte layout, or `nil`
    /// for text no door can hold (the core's 8).
    static func declaredDecimal(_ text: String) -> Cast.OutScratch? {
        Cast.withUTF8(text) { declaredDecimal($0) }
    }

    /// The same conversion over bytes, which a `String` cannot make invalid UTF-8.
    static func declaredDecimal(_ utf8: UnsafeRawBufferPointer) -> Cast.OutScratch? {
        var decimal: Cast.OutScratch = (0, 0)
        let code = withUnsafeMutableBytes(of: &decimal) { out in
            cast_declared_decimal(Cast.inputPointer(utf8), UInt(utf8.count), .init(OpaquePointer(out.baseAddress)))
        }
        switch code {
        case 0: return decimal
        case 8: return nil
        default: preconditionFailure("libhypercast reported a contract violation — a binding bug, please report it")
        }
    }

    private static func length(_ value: Int?, _ name: String) throws(Refused) -> UInt32 {
        guard let value else { return 0 }
        guard let length = UInt32(exactly: value) else {
            throw Refused(code: nil, message: "\(name) must be from 0 to \(UInt32.max); it is \(value).")
        }
        return length
    }

    /// The `enum` list packed for the door: at the text door each entry a little-endian
    /// 32-bit byte length then its UTF-8 bytes, with the declared strings kept; elsewhere
    /// 16-byte decimals in native byte order.
    private static func pack(_ values: [ConstraintValue], at door: UInt32) throws(Refused) -> ([UInt8], [String]?) {
        var packed: [UInt8] = []
        guard door == textDoor else {
            packed.reserveCapacity(values.count * 16)
            for value in values {
                withUnsafeBytes(of: try declared(value)) { packed.append(contentsOf: $0) }
            }
            return (packed, nil)
        }
        var texts: [String] = []
        texts.reserveCapacity(values.count)
        for (index, value) in values.enumerated() {
            // A number is a valid entry that no string equals (JSON Schema's `enum` is exact
            // equality across types): packed as an empty entry, which no input matches (empty
            // input is the door's `.empty` fault), so every other entry keeps its index.
            guard case .text(let text) = value.kind else {
                withUnsafeBytes(of: UInt32(0).littleEndian) { packed.append(contentsOf: $0) }
                texts.append("")
                continue
            }
            guard let count = UInt32(exactly: text.utf8.count) else {
                throw Refused(code: nil, message: "Enum entry \(index) is longer than \(UInt32.max) UTF-8 bytes.")
            }
            withUnsafeBytes(of: count.littleEndian) { packed.append(contentsOf: $0) }
            packed.append(contentsOf: text.utf8)
            texts.append(text)
        }
        return (packed, texts)
    }

    /// One message per refusal code, worded as the Rust core's `Refusal` displays it.
    static func refusalMessage(_ code: Int32) -> String {
        let rule =
            switch code {
            case 1: "a constraint the door does not take"
            case 5: "multipleOf is not positive"
            case 6: "a rounding without multipleOf"
            case 8: "a bound, step or enum entry the door cannot hold"
            case 9: "an enum entry that is not UTF-8"
            case 11: "a malformed constraints layout"
            default: "a rule this binding does not name"
            }
        return "Constraints refused (code \(code)): \(rule)."
    }
}

extension Interop {
    /// The native `RawConstraints`, 104 bytes, 8-aligned, which the checked doors read in
    /// place: a size word (104) at 0, the flags at 4 (`1` minimum, `2` exclusiveMinimum, `4`
    /// maximum, `8` exclusiveMaximum, `16` multipleOf, `32` minLength, `64` maxLength, `128`
    /// trim, `256` an `enum` declared, its list read beside the layout, `512` caseSensitive),
    /// the rounding code at 8 (`0` strict, `1` midpoint, `2` floor, `3` ceiling), `minLength`
    /// at 12, `maxLength` at 16, a reserved zero word at 20, then five decimals in the layout
    /// ``decimal(_:)`` reads, one per keyword: `minimum` at 24, `maximum` at 40, `multipleOf`
    /// at 56, `exclusiveMinimum` at 72 and `exclusiveMaximum` at 88. Carried as thirteen
    /// `UInt64` words whose in-memory bytes are the layout's.
    public typealias RawConstraints = (
        UInt64, UInt64, UInt64, UInt64, UInt64, UInt64, UInt64, UInt64, UInt64, UInt64, UInt64, UInt64, UInt64
    )

    /// The size of the native constraints layout, in bytes, which its first word carries.
    public static let rawConstraintsSize = 104

    static let zeroRawConstraints: RawConstraints = (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)

    /// A declaration laid out as the core reads it — what every checked door hands the core,
    /// beside ``packedEnum(_:)``. The core judged it when it was declared.
    public static func rawConstraints<T>(_ constraints: Constraints<T>) -> RawConstraints {
        constraints.raw
    }

    /// A declaration's `enum` list as the core reads it beside ``rawConstraints(_:)`` (whose
    /// flag `256` says whether one was declared), empty for none: at a numeric door, 16-byte
    /// decimals in native byte order; at the text door, each entry a little-endian 32-bit byte
    /// length followed by its UTF-8 bytes.
    public static func packedEnum<T>(_ constraints: Constraints<T>) -> [UInt8] {
        constraints.packed
    }
}
