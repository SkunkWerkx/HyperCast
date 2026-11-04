/// The closed set of reasons a cast can fail — the native core's verdict codes, verbatim.
/// Adding a member is a deliberate breaking change: every exhaustive switch over this enum
/// must be updated, in every binding at once.
public enum CastFailure: Int32, Equatable, Sendable {
    /// Required input was empty or white space. ``Cast/optional(_:)`` surfaces this as `nil`.
    case empty = 1
    /// Input was present but not recognizable as the target type.
    case malformed = 2
    /// Well-formed but outside the target's range — `"256"` for a u8, `1e400` for an f64.
    case outOfRange = 3
}

/// The failure case of a ``Verdict``: a closed conversion reason plus the offending span,
/// pointing back into the caller's own input. Nothing is captured — rendering a diagnostic
/// (slicing the offending text out of the input) is the caller's choice.
///
/// `offset` and `length` are byte offsets into the UTF-8 representation of the input —
/// exactly what the `[UInt8]` doors received, identical to `String.utf8` offsets.
///
/// A fault a door returns also says what failed there: ``detail``, with its raw ``detailCode``.
/// Equality is the fault's reason and span and nothing else, as it always was, so a fault
/// built by hand with ``init(reason:offset:length:)`` equals the door's fault for the same
/// failure; compare ``detailCode`` to compare details.
public struct Fault: Equatable, Sendable {
    /// Why the cast failed.
    public let reason: CastFailure
    /// Where the offending span starts, in UTF-8 bytes from the start of the input.
    public let offset: Int
    /// How many UTF-8 bytes the offending span covers.
    public let length: Int
    /// The core's detail code for what failed in the span: 1 or more on a door's fault, 0 on a
    /// fault built without one.
    public let detailCode: UInt32

    /// What failed in the span: the ``FaultDetail`` (``FaultDetail/name`` is `nil` for a code
    /// this binding doesn't name), or `nil` for a fault built by hand without one.
    public var detail: FaultDetail? { detailCode == 0 ? nil : FaultDetail(code: detailCode) }

    /// Builds a fault from its parts — what a test asserting an expected verdict needs; the
    /// doors are what produce one otherwise. It carries no detail.
    public init(reason: CastFailure, offset: Int, length: Int) {
        self.init(reason: reason, offset: offset, length: length, detailCode: 0)
    }

    /// Builds a fault with the raw detail code the core reported (0 for none).
    public init(reason: CastFailure, offset: Int, length: Int, detailCode: UInt32) {
        self.reason = reason
        self.offset = offset
        self.length = length
        self.detailCode = detailCode
    }

    /// Two faults are equal when their reason and span are: the detail is not part of equality.
    public static func == (lhs: Fault, rhs: Fault) -> Bool {
        lhs.reason == rhs.reason && lhs.offset == rhs.offset && lhs.length == rhs.length
    }
}

/// The outcome of a cast: exactly one of a value or a ``Fault`` — and in Swift the union is
/// the language's own bread and butter: an enum with associated values, where an exhaustive
/// switch is *mandatory*, not opt-in. Of every binding in this repo, this is the strongest
/// form of the guarantee: C# needs CS8509 elevated, Java needs the sealed hierarchy, Python
/// needs a type checker — Swift's compiler refuses an unhandled disposition out of the box.
///
/// ```swift
/// switch try Cast.i32("(1,234)", format: .invariant) {
/// case .success(let value): print("got \(value)")          // -1234, accounting negative
/// case .fault(let fault): print("\(fault.reason) at byte \(fault.offset)")
/// }   // no third case exists, and the compiler knows it
/// ```
public enum Verdict<T> {
    /// The success case: the cast value.
    case success(T)
    /// The failure case: a closed reason plus the offending byte span.
    case fault(Fault)
}

extension Verdict: Equatable where T: Equatable {}
extension Verdict: Sendable where T: Sendable {}
