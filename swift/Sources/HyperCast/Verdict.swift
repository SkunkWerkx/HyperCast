/// The closed set of reasons a cast can fail — the native core's verdict codes, verbatim.
/// Adding a member is a deliberate breaking change: every exhaustive switch over this enum
/// must be updated, in every binding at once.
public enum CastFailure: Int32, Equatable, Sendable {
    /// Required input was empty or whitespace. ``Cast/optional(_:)`` surfaces this as `nil`.
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
public struct Fault: Equatable, Sendable {
    public let reason: CastFailure
    public let offset: Int
    public let length: Int

    public init(reason: CastFailure, offset: Int, length: Int) {
        self.reason = reason
        self.offset = offset
        self.length = length
    }
}

/// The outcome of a cast: exactly one of a value or a ``Fault`` — and in Swift the union is
/// the language's own bread and butter: an enum with associated values, where an exhaustive
/// switch is *mandatory*, not opt-in. Of every binding in this repo, this is the strongest
/// form of the guarantee: C# needs CS8509 elevated, Java needs the sealed hierarchy, Python
/// needs a type checker — Swift's compiler refuses an unhandled disposition out of the box.
///
/// ```swift
/// switch Cast.i32("(1,234)", format: .invariant) {
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
