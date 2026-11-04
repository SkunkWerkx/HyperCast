/// What failed inside a ``Fault``'s span: the native core's detail vocabulary, codes 1 to 32,
/// a small shared set with precise meanings (`groupSize` is a group with the wrong digit
/// count, `digitExpected` a digit field cut short). The ``Fault/reason`` stays the broad
/// classification and the span says where; the detail says what, mainly for the person
/// diagnosing rejected data.
///
/// Published codes keep their numeric values and meanings. New codes may appear in a minor
/// release, so a detail is open, like ``Keyword``: a value with static members rather than an
/// enum, so a `switch` over one needs a `default` arm and a code added later breaks no one. A
/// code this binding doesn't name yet keeps its ``rawValue`` (its ``name`` is `nil`, and it
/// compares equal to the named member once a later release names it). A fault with an
/// unpublished detail still has a usable reason and span.
///
/// ```swift
/// if case .fault(let fault) = Cast.i32("12,34", format: .invariant) {
///     switch fault.detail {
///     case .groupSize?: print("a group has the wrong number of digits")
///     case let detail?: print("failed: \(detail)")  // `group_size`, `digit_expected`, …
///     case nil: break  // a fault built by hand has no detail
///     }
/// }
/// ```
public struct FaultDetail: RawRepresentable, Hashable, Sendable, CustomStringConvertible {
    /// The native core's code: 1 `blank` … 32 `not_a_scalar`, or the code of a detail this binding
    /// doesn't name.
    public let rawValue: UInt32

    /// The detail whose native code is `rawValue`. Any code is kept, 0 and the unpublished
    /// ones included: decoding never fails and never traps.
    public init(rawValue: UInt32) {
        self.rawValue = rawValue
    }

    /// The detail whose native code is `code`; ``init(rawValue:)`` without the failable shape.
    public init(code: UInt32) {
        self.init(rawValue: code)
    }

    /// `blank`: empty, or only white space.
    public static let blank = FaultDetail(rawValue: 1)
    /// `unexpected_character`: a character the position doesn't allow, where it allows several things.
    public static let unexpectedCharacter = FaultDetail(rawValue: 2)
    /// `digit_expected`: a numeral or digit field is under way, or a fixed position needs a digit,
    /// and another character or the end is found.
    public static let digitExpected = FaultDetail(rawValue: 3)
    /// `delimiter_expected`: a required mark (`-`, `:`, `T`, `}`, `0x`, `;`) is missing or different.
    public static let delimiterExpected = FaultDetail(rawValue: 4)
    /// `trailing_text`: the value is complete, but more text follows.
    public static let trailingText = FaultDetail(rawValue: 5)
    /// `no_digits`: the numeral is absent entirely: only a sign, an affix or a bare radix prefix.
    public static let noDigits = FaultDetail(rawValue: 6)
    /// `sign_misplaced`: a sign where none may stand.
    public static let signMisplaced = FaultDetail(rawValue: 7)
    /// `parentheses_unbalanced`: `(` without `)`, or `( )`.
    public static let parenthesesUnbalanced = FaultDetail(rawValue: 8)
    /// `mark_repeated`: a once-only mark appears twice (a second decimal separator).
    public static let markRepeated = FaultDetail(rawValue: 9)
    /// `group_misplaced`: a group separator not between digits, or an empty group.
    public static let groupMisplaced = FaultDetail(rawValue: 10)
    /// `group_size`: a group with the wrong digit count under a strict grouping policy.
    public static let groupSize = FaultDetail(rawValue: 11)
    /// `group_in_fraction`: grouping after the decimal separator or in an exponent.
    public static let groupInFraction = FaultDetail(rawValue: 12)
    /// `ambiguous_separator`: separator detection can't tell `.` from `,`.
    public static let ambiguousSeparator = FaultDetail(rawValue: 13)
    /// `fraction_not_allowed`: a fraction where the type is whole.
    public static let fractionNotAllowed = FaultDetail(rawValue: 14)
    /// `field_width`: a delimited field has the wrong number of digits.
    public static let fieldWidth = FaultDetail(rawValue: 15)
    /// `wrong_length`: a fixed-shape token, recognised by its total length, has the wrong length.
    public static let wrongLength = FaultDetail(rawValue: 16)
    /// `component_missing`: a required value-level part is absent (a zone, seconds, a duration unit).
    public static let componentMissing = FaultDetail(rawValue: 17)
    /// `not_recognized`: not one of the door's accepted words or values.
    public static let notRecognized = FaultDetail(rawValue: 18)
    /// `unknown_unit`: an unknown or misplaced duration unit.
    public static let unknownUnit = FaultDetail(rawValue: 19)
    /// `invalid_utf8`: the bytes at the failure site don't decode as UTF-8.
    public static let invalidUtf8 = FaultDetail(rawValue: 20)
    /// `too_long`: past an implementation limit.
    public static let tooLong = FaultDetail(rawValue: 21)
    /// `not_finite`: a non-finite value: NaN or an infinity.
    public static let notFinite = FaultDetail(rawValue: 22)
    /// `not_whole`: a non-integral double at a whole-number door.
    public static let notWhole = FaultDetail(rawValue: 23)
    /// `negative_not_allowed`: negative where the type has no negatives.
    public static let negativeNotAllowed = FaultDetail(rawValue: 24)
    /// `too_large`: the magnitude exceeds what the target type represents.
    public static let tooLarge = FaultDetail(rawValue: 25)
    /// `too_small`: below a signed type's minimum.
    public static let tooSmall = FaultDetail(rawValue: 26)
    /// `too_precise`: more fractional digits than the type holds.
    public static let tooPrecise = FaultDetail(rawValue: 27)
    /// `date_field_out_of_range`: year 0, month 13, a day past the month's end.
    public static let dateFieldOutOfRange = FaultDetail(rawValue: 28)
    /// `time_field_out_of_range`: an hour, minute, second or offset out of range.
    public static let timeFieldOutOfRange = FaultDetail(rawValue: 29)
    /// `outside_window`: a representable instant or duration past the supported window.
    public static let outsideWindow = FaultDetail(rawValue: 30)
    /// `nonexistent_date`: a day that never existed (Excel's phantom 1900-02-29).
    public static let nonexistentDate = FaultDetail(rawValue: 31)
    /// `not_a_scalar`: a code point above U+10FFFF, or a surrogate.
    public static let notAScalar = FaultDetail(rawValue: 32)

    /// The native code, as ``rawValue``.
    public var code: UInt32 { rawValue }

    private static let published = [
        "blank",
        "unexpected_character",
        "digit_expected",
        "delimiter_expected",
        "trailing_text",
        "no_digits",
        "sign_misplaced",
        "parentheses_unbalanced",
        "mark_repeated",
        "group_misplaced",
        "group_size",
        "group_in_fraction",
        "ambiguous_separator",
        "fraction_not_allowed",
        "field_width",
        "wrong_length",
        "component_missing",
        "not_recognized",
        "unknown_unit",
        "invalid_utf8",
        "too_long",
        "not_finite",
        "not_whole",
        "negative_not_allowed",
        "too_large",
        "too_small",
        "too_precise",
        "date_field_out_of_range",
        "time_field_out_of_range",
        "outside_window",
        "nonexistent_date",
        "not_a_scalar",
    ]

    /// The published spelling: `blank`, `unexpected_character`, … `not_a_scalar`; `nil` for a
    /// code this binding does not name.
    public var name: String? {
        let index = Int(rawValue) - 1
        return Self.published.indices.contains(index) ? Self.published[index] : nil
    }

    /// Whether this binding names the code (1 to 32).
    public var isPublished: Bool { name != nil }

    /// The ``name``, or `detail N` for a code this binding does not name.
    public var description: String { name ?? "detail \(rawValue)" }
}
