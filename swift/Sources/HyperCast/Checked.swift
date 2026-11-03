/// The rule a ``Violation`` broke, spelled the OpenAPI way (``name`` is `exclusiveMinimum`,
/// not `ExclusiveMinimum`), carried as the native core's code.
///
/// Open to additions: `format` and `pattern` may join in a minor release, so a keyword is a
/// value with static members rather than an enum, and a `switch` over one needs a `default`
/// arm. A keyword this binding does not know yet keeps its code, and its ``name`` is
/// `keyword N`.
///
/// ```swift
/// switch violation.keyword {
/// case .minimum, .maximum: "out of bounds"
/// case .enum: "not one of the allowed values"
/// default: "breaks \(violation.keyword)"  // keywords are open
/// }
/// ```
public struct Keyword: RawRepresentable, Hashable, Sendable, CustomStringConvertible {
    /// The native core's code: 1 `minimum` … 8 `enum`.
    public let rawValue: UInt32

    /// The keyword whose native code is `rawValue`, or `nil` for 0, which names none. Any
    /// other code is kept, so a keyword a newer core reports survives the trip.
    public init?(rawValue: UInt32) {
        guard rawValue != 0 else { return nil }
        self.rawValue = rawValue
    }

    private init(code: UInt32) {
        rawValue = code
    }

    /// `minimum`: the value is below it.
    public static let minimum = Keyword(code: 1)
    /// `exclusiveMinimum`: the value is at or below it.
    public static let exclusiveMinimum = Keyword(code: 2)
    /// `maximum`: the value is above it.
    public static let maximum = Keyword(code: 3)
    /// `exclusiveMaximum`: the value is at or above it.
    public static let exclusiveMaximum = Keyword(code: 4)
    /// `multipleOf`: the value is not a multiple, under ``Rounding/strict``.
    public static let multipleOf = Keyword(code: 5)
    /// `minLength`: the text has fewer code points.
    public static let minLength = Keyword(code: 6)
    /// `maxLength`: the text has more code points.
    public static let maxLength = Keyword(code: 7)
    /// `enum`: the value matches no entry.
    public static let `enum` = Keyword(code: 8)

    /// The OpenAPI spelling: `minimum`, `exclusiveMinimum`, `maximum`, `exclusiveMaximum`,
    /// `multipleOf`, `minLength`, `maxLength`, `enum`; `keyword N` for a code this binding does
    /// not name.
    public var name: String {
        switch rawValue {
        case 1: "minimum"
        case 2: "exclusiveMinimum"
        case 3: "maximum"
        case 4: "exclusiveMaximum"
        case 5: "multipleOf"
        case 6: "minLength"
        case 7: "maxLength"
        case 8: "enum"
        default: "keyword \(rawValue)"
        }
    }

    /// The ``name``.
    public var description: String { name }
}

/// A value that parsed and broke a declared rule: the ``Keyword`` and the value's span, in the
/// same UTF-8 byte units as a ``Fault``'s. A from-double door has no text to point into, so
/// its span is empty.
public struct Violation: Equatable, Sendable {
    /// The rule the value broke.
    public let keyword: Keyword
    /// Where the value starts, in UTF-8 bytes from the start of the input.
    public let offset: Int
    /// How many UTF-8 bytes the value covers.
    public let length: Int

    /// Builds a violation from its parts — what a test asserting an expected verdict needs;
    /// the checked doors are what produce one otherwise.
    public init(keyword: Keyword, offset: Int, length: Int) {
        self.keyword = keyword
        self.offset = offset
        self.length = length
    }
}

/// Why a checked door gave no value: closed, exactly one of two. A ``Fault``, the very type
/// every plain door returns (it did not parse), or a ``Violation`` (it parsed and broke a
/// declared rule). A parse failure keeps its own closed lexicon, and a broken rule is a
/// different type, not a fourth ``CastFailure``.
public enum Rejection: Equatable, Sendable {
    /// It did not parse: the plain door's own fault, unchanged.
    case fault(Fault)
    /// It parsed and broke a declared rule.
    case violation(Violation)
}

/// What a checked door delivers on success: the value, whether rounding changed it, and, at
/// the text door with an `enum`, which entry it matched.
public struct Checked<T> {
    /// The value: rounded to the declared `multipleOf` when the declaration rounds, and at the
    /// text door with an `enum` the caller's own declared entry.
    public let value: T
    /// `true` only when rounding changed the value.
    public let adjusted: Bool
    /// At the text door, the index of the `enum` entry the value matched; `nil` otherwise. An
    /// integer `enum` has no index: the value is the answer.
    public let enumIndex: Int?

    /// Builds a success from its parts — what a test asserting an expected verdict needs; the
    /// checked doors are what produce one otherwise.
    public init(value: T, adjusted: Bool, enumIndex: Int?) {
        self.value = value
        self.adjusted = adjusted
        self.enumIndex = enumIndex
    }
}

extension Checked: Equatable where T: Equatable {}
extension Checked: Sendable where T: Sendable {}

/// The outcome of a checked cast: a ``Checked`` value, or a ``Rejection`` — a ``Fault`` or a
/// ``Violation``. Like ``Verdict``, an enum whose exhaustive `switch` the compiler demands,
/// and none of the three outcomes needs a `default`:
///
/// ```swift
/// switch Cast.text(" MALE ", constraints: sex) {
/// case .success(let checked): print(checked.value)  // Male, the declared entry
/// case .rejection(.fault(let fault)): print("\(fault.reason) at byte \(fault.offset)")
/// case .rejection(.violation(let violation)): print("breaks \(violation.keyword)")
/// }
/// ```
///
/// The plain doors keep returning ``Verdict``; this type is the checked doors' alone.
public enum CheckedVerdict<T> {
    /// The value, judged and found to keep every declared rule.
    case success(Checked<T>)
    /// It did not parse, or it broke a declared rule.
    case rejection(Rejection)
}

extension CheckedVerdict: Equatable where T: Equatable {}
extension CheckedVerdict: Sendable where T: Sendable {}

/// What a value that is not a multiple of the declared `multipleOf` becomes. Every mode but
/// ``strict`` needs a `multipleOf`, and rounding happens on the exact decimal, before the
/// bounds judge the result.
public enum Rounding: Sendable, CaseIterable {
    /// Not a multiple is a ``Keyword/multipleOf`` violation. The default.
    case strict
    /// Rounded to the nearest multiple, a tie to the even one.
    case midpoint
    /// Rounded down, toward negative infinity.
    case floor
    /// Rounded up, toward positive infinity.
    case ceiling

    /// The native core's rounding code.
    var code: UInt32 {
        switch self {
        case .strict: 0
        case .midpoint: 1
        case .floor: 2
        case .ceiling: 3
        }
    }
}
