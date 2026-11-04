import Foundation
import HyperCastCore

// The checked doors: each numeric door, and the text door, judged by a door-bound
// Constraints<T> after it parses, through cast_checked_detailed, cast_checked_detailed_from_f64
// and the text door of cast_detailed, which report a failure's detail beside its span.
// The declaration itself is judged once, by cast_constraints_check, when it is built, its
// decimal text converted by cast_declared_decimal. Spans are UTF-8 bytes, as the core counts
// them and as every Fault here already is, so nothing is remapped.
extension Cast {
    // What a checked door reports beside its value, the native `RawCheck`: the keyword, the
    // adjusted flag and the enum index, three u32s. Internal so the suite pins it to the header.
    typealias CheckScratch = (UInt32, UInt32, UInt32)
    // The native `RawSpan` the text door writes: an offset and a length in bytes.
    typealias SpanScratch = (UInt32, UInt32)
    // The header's sizes for the layouts above and the declaration's, which the suite pins
    // each restated layout to.
    static var headerRawConstraintsSize: Int { Int(HyperCastRawConstraints_SIZE) }
    static var headerRawCheckSize: Int { Int(HyperCastRawCheck_SIZE) }
    static var headerRawSpanSize: Int { Int(HyperCastRawSpan_SIZE) }
    static var headerRawFaultDetailSize: Int { Int(HyperCastRawFaultDetail_SIZE) }
    static var headerRawDateOptionsSize: Int { Int(HyperCastRawDateOptions_SIZE) }
    static var headerRawDateOptionsMinSize: Int { Int(HyperCastRawDateOptions_MIN_SIZE) }
    // The header's grouping policy bits — legacy, Western, Indian and the mask — and its
    // date-option bits, in the order Grouping and DateOptions restate them, which the suite pins.
    static var headerGroupingBits: [UInt32] {
        [
            HyperCastNumFormat_GROUPING_LEGACY, HyperCastNumFormat_GROUPING_WESTERN,
            HyperCastNumFormat_GROUPING_INDIAN, HyperCastNumFormat_GROUPING_POLICY,
        ].map { UInt32($0) }
    }
    // The header's flag bits, in the order Constraints restates them, which the suite pins.
    static var headerConstraintFlags: [UInt32] {
        [
            HyperCastRawConstraints_HAS_MINIMUM, HyperCastRawConstraints_HAS_EXCLUSIVE_MINIMUM,
            HyperCastRawConstraints_HAS_MAXIMUM, HyperCastRawConstraints_HAS_EXCLUSIVE_MAXIMUM,
            HyperCastRawConstraints_HAS_MULTIPLE_OF, HyperCastRawConstraints_HAS_MIN_LENGTH,
            HyperCastRawConstraints_HAS_MAX_LENGTH, HyperCastRawConstraints_TRIM, HyperCastRawConstraints_HAS_ENUM,
        ].map { UInt32($0) }
    }

    /// Judges a declaration once, for `door`: 0, or the core's refusal code.
    static func checkConstraints(door: UInt32, raw: Interop.RawConstraints, packed: [UInt8]) -> Int32 {
        var raw = raw
        return withUnsafeBytes(of: &raw) { declared in
            packed.withUnsafeBufferPointer { entries in
                cast_constraints_check(
                    door, 0, .init(OpaquePointer(declared.baseAddress)), entries.baseAddress, UInt(entries.count))
            }
        }
    }

    // A check before the core writes one: no keyword, not adjusted, no enum entry.
    private static var noCheck: CheckScratch { (0, 0, UInt32.max) }

    /// Runs a checked door over the caller's bytes; `format` is nil at the text door.
    private static func checked<D>(
        _ utf8: UnsafeRawBufferPointer, _ format: NumFormat?, _ constraints: Constraints<D>,
        _ out: inout OutScratch, _ faultOut: inout DetailScratch, _ check: inout CheckScratch
    ) -> Int32 {
        var rawFormat = format?.raw ?? Interop.zeroRawFormat
        var declared = constraints.raw
        return withUnsafeMutableBytes(of: &out) { outRaw in
            withUnsafeMutableBytes(of: &faultOut) { faultRaw in
                withUnsafeMutableBytes(of: &check) { checkRaw in
                    withUnsafeBytes(of: &rawFormat) { formatRaw in
                        withUnsafeBytes(of: &declared) { declaredRaw in
                            constraints.packed.withUnsafeBufferPointer { entries in
                                cast_checked_detailed(
                                    constraints.door, 0,
                                    format == nil ? nil : .init(OpaquePointer(formatRaw.baseAddress)),
                                    .init(OpaquePointer(declaredRaw.baseAddress)), entries.baseAddress,
                                    UInt(entries.count), inputPointer(utf8), UInt(utf8.count), outRaw.baseAddress,
                                    .init(OpaquePointer(faultRaw.baseAddress)),
                                    .init(OpaquePointer(checkRaw.baseAddress)))
                            }
                        }
                    }
                }
            }
        }
    }

    private static func checkedDoor<T>(
        _ utf8: UnsafeRawBufferPointer, _ format: NumFormat, _ constraints: Constraints<T>,
        read: (UnsafeRawBufferPointer) -> T
    ) -> CheckedVerdict<T> {
        var out: OutScratch = (0, 0)
        var faultOut = Interop.freshRawFaultDetail
        var check = noCheck
        let code = checked(utf8, format, constraints, &out, &faultOut, &check)
        guard code == 0 else { return .rejection(rejection(code, faultOut, check)) }
        return .success(found(withUnsafeBytes(of: &out, read), check))
    }

    private static func checkedFromDouble<T>(
        _ value: Double, _ constraints: Constraints<T>, read: (UnsafeRawBufferPointer) -> T
    ) -> CheckedVerdict<T> {
        var out: OutScratch = (0, 0)
        var faultOut = Interop.freshRawFaultDetail
        var check = noCheck
        var declared = constraints.raw
        let code = withUnsafeMutableBytes(of: &out) { outRaw in
            withUnsafeMutableBytes(of: &faultOut) { faultRaw in
                withUnsafeMutableBytes(of: &check) { checkRaw in
                    withUnsafeBytes(of: &declared) { declaredRaw in
                        constraints.packed.withUnsafeBufferPointer { entries in
                            cast_checked_detailed_from_f64(
                                constraints.door, 0, .init(OpaquePointer(declaredRaw.baseAddress)),
                                entries.baseAddress, UInt(entries.count), value, outRaw.baseAddress,
                                .init(OpaquePointer(faultRaw.baseAddress)), .init(OpaquePointer(checkRaw.baseAddress)))
                        }
                    }
                }
            }
        }
        guard code == 0 else { return .rejection(rejection(code, faultOut, check)) }
        return .success(found(withUnsafeBytes(of: &out, read), check))
    }

    /// A success: the value, whether rounding changed it, and the matched text entry's index.
    /// An integer `enum` has no index; the value is the answer.
    private static func found<T>(_ value: T, _ check: CheckScratch) -> Checked<T> {
        Checked(value: value, adjusted: check.1 != 0, enumIndex: check.2 == UInt32.max ? nil : Int(check.2))
    }

    /// The rejection a nonzero code names: 4 a violation of the keyword the check carries,
    /// 1 to 3 the door's own fault, both spanning UTF-8 bytes of the input.
    private static func rejection(_ code: Int32, _ span: DetailScratch, _ check: CheckScratch) -> Rejection {
        precondition(code != -1, "libhypercast reported a contract violation — a binding bug, please report it")
        guard code == 4 else {
            return .fault(Interop.fault(code: code, offset: span.1, length: span.2, detail: span.3))
        }
        guard let keyword = Keyword(rawValue: check.0) else {
            preconditionFailure("libhypercast reported a violation of no keyword — a binding bug, please report it")
        }
        return .violation(Violation(keyword: keyword, offset: Int(span.1), length: Int(span.2)))
    }

    // MARK: - checked i8

    /// Casts integer text to a signed 8-bit value under the declared format, as
    /// ``i8(_:format:)-swift.type.method`` does, then judges it by `constraints`.
    public static func i8(_ text: String, format: NumFormat, constraints: Constraints<Int8>)
        -> CheckedVerdict<Int8>
    {
        withUTF8(text) { i8($0, format: format, constraints: constraints) }
    }

    /// See ``i8(_:format:constraints:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i8(_ utf8: [UInt8], format: NumFormat, constraints: Constraints<Int8>)
        -> CheckedVerdict<Int8>
    {
        utf8.withUnsafeBytes { i8($0, format: format, constraints: constraints) }
    }

    /// See ``i8(_:format:constraints:)-swift.type.method``; input as a raw view of UTF-8
    /// bytes, the primitive the `String` and `[UInt8]` forms wrap.
    public static func i8(_ utf8: UnsafeRawBufferPointer, format: NumFormat, constraints: Constraints<Int8>)
        -> CheckedVerdict<Int8>
    {
        checkedDoor(utf8, format, constraints) { $0.load(as: Int8.self) }
    }

    /// Judges a number the caller already holds — a JSON parser's `Double`, a spreadsheet's
    /// numeric cell — at the i8 door by `constraints`, on its sign and the shortest decimal
    /// that names it: the judgement ``i8(_:format:constraints:)-swift.type.method`` makes of
    /// that text. NaN is ``CastFailure/malformed``; an infinity is ``CastFailure/outOfRange`` at
    /// a float or decimal door and ``CastFailure/malformed`` at an integer door. With no text to
    /// point into, every fault and violation has an empty span.
    public static func i8FromDouble(_ value: Double, constraints: Constraints<Int8>) -> CheckedVerdict<Int8> {
        checkedFromDouble(value, constraints) { $0.load(as: Int8.self) }
    }

    // MARK: - checked i16

    /// Casts text as ``i16(_:format:)-swift.type.method`` does, then judges the value by
    /// `constraints`.
    public static func i16(_ text: String, format: NumFormat, constraints: Constraints<Int16>)
        -> CheckedVerdict<Int16>
    {
        withUTF8(text) { i16($0, format: format, constraints: constraints) }
    }

    /// See ``i16(_:format:constraints:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i16(_ utf8: [UInt8], format: NumFormat, constraints: Constraints<Int16>)
        -> CheckedVerdict<Int16>
    {
        utf8.withUnsafeBytes { i16($0, format: format, constraints: constraints) }
    }

    /// See ``i16(_:format:constraints:)-swift.type.method``; input as a raw view of UTF-8
    /// bytes, the primitive the `String` and `[UInt8]` forms wrap.
    public static func i16(_ utf8: UnsafeRawBufferPointer, format: NumFormat, constraints: Constraints<Int16>)
        -> CheckedVerdict<Int16>
    {
        checkedDoor(utf8, format, constraints) { $0.load(as: Int16.self) }
    }

    /// Judges a number the caller already holds — a JSON parser's `Double`, a spreadsheet's
    /// numeric cell — at the i16 door by `constraints`, on its sign and the shortest decimal
    /// that names it: the judgement ``i16(_:format:constraints:)-swift.type.method`` makes of
    /// that text. NaN is ``CastFailure/malformed``; an infinity is ``CastFailure/outOfRange`` at
    /// a float or decimal door and ``CastFailure/malformed`` at an integer door. With no text to
    /// point into, every fault and violation has an empty span.
    public static func i16FromDouble(_ value: Double, constraints: Constraints<Int16>) -> CheckedVerdict<Int16> {
        checkedFromDouble(value, constraints) { $0.load(as: Int16.self) }
    }

    // MARK: - checked i32

    /// Casts text as ``i32(_:format:)-swift.type.method`` does, then judges the value by
    /// `constraints`.
    public static func i32(_ text: String, format: NumFormat, constraints: Constraints<Int32>)
        -> CheckedVerdict<Int32>
    {
        withUTF8(text) { i32($0, format: format, constraints: constraints) }
    }

    /// See ``i32(_:format:constraints:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i32(_ utf8: [UInt8], format: NumFormat, constraints: Constraints<Int32>)
        -> CheckedVerdict<Int32>
    {
        utf8.withUnsafeBytes { i32($0, format: format, constraints: constraints) }
    }

    /// See ``i32(_:format:constraints:)-swift.type.method``; input as a raw view of UTF-8
    /// bytes, the primitive the `String` and `[UInt8]` forms wrap.
    public static func i32(_ utf8: UnsafeRawBufferPointer, format: NumFormat, constraints: Constraints<Int32>)
        -> CheckedVerdict<Int32>
    {
        checkedDoor(utf8, format, constraints) { $0.load(as: Int32.self) }
    }

    /// Judges a number the caller already holds — a JSON parser's `Double`, a spreadsheet's
    /// numeric cell — at the i32 door by `constraints`, on its sign and the shortest decimal
    /// that names it: the judgement ``i32(_:format:constraints:)-swift.type.method`` makes of
    /// that text. NaN is ``CastFailure/malformed``; an infinity is ``CastFailure/outOfRange`` at
    /// a float or decimal door and ``CastFailure/malformed`` at an integer door. With no text to
    /// point into, every fault and violation has an empty span.
    public static func i32FromDouble(_ value: Double, constraints: Constraints<Int32>) -> CheckedVerdict<Int32> {
        checkedFromDouble(value, constraints) { $0.load(as: Int32.self) }
    }

    // MARK: - checked i64

    /// Casts text as ``i64(_:format:)-swift.type.method`` does, then judges the value by
    /// `constraints`.
    public static func i64(_ text: String, format: NumFormat, constraints: Constraints<Int64>)
        -> CheckedVerdict<Int64>
    {
        withUTF8(text) { i64($0, format: format, constraints: constraints) }
    }

    /// See ``i64(_:format:constraints:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i64(_ utf8: [UInt8], format: NumFormat, constraints: Constraints<Int64>)
        -> CheckedVerdict<Int64>
    {
        utf8.withUnsafeBytes { i64($0, format: format, constraints: constraints) }
    }

    /// See ``i64(_:format:constraints:)-swift.type.method``; input as a raw view of UTF-8
    /// bytes, the primitive the `String` and `[UInt8]` forms wrap.
    public static func i64(_ utf8: UnsafeRawBufferPointer, format: NumFormat, constraints: Constraints<Int64>)
        -> CheckedVerdict<Int64>
    {
        checkedDoor(utf8, format, constraints) { $0.load(as: Int64.self) }
    }

    /// Judges a number the caller already holds — a JSON parser's `Double`, a spreadsheet's
    /// numeric cell — at the i64 door by `constraints`, on its sign and the shortest decimal
    /// that names it: the judgement ``i64(_:format:constraints:)-swift.type.method`` makes of
    /// that text. NaN is ``CastFailure/malformed``; an infinity is ``CastFailure/outOfRange`` at
    /// a float or decimal door and ``CastFailure/malformed`` at an integer door. With no text to
    /// point into, every fault and violation has an empty span.
    public static func i64FromDouble(_ value: Double, constraints: Constraints<Int64>) -> CheckedVerdict<Int64> {
        checkedFromDouble(value, constraints) { $0.load(as: Int64.self) }
    }

    // MARK: - checked u8

    /// Casts text as ``u8(_:format:)-swift.type.method`` does, then judges the value by
    /// `constraints`.
    public static func u8(_ text: String, format: NumFormat, constraints: Constraints<UInt8>)
        -> CheckedVerdict<UInt8>
    {
        withUTF8(text) { u8($0, format: format, constraints: constraints) }
    }

    /// See ``u8(_:format:constraints:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u8(_ utf8: [UInt8], format: NumFormat, constraints: Constraints<UInt8>)
        -> CheckedVerdict<UInt8>
    {
        utf8.withUnsafeBytes { u8($0, format: format, constraints: constraints) }
    }

    /// See ``u8(_:format:constraints:)-swift.type.method``; input as a raw view of UTF-8
    /// bytes, the primitive the `String` and `[UInt8]` forms wrap.
    public static func u8(_ utf8: UnsafeRawBufferPointer, format: NumFormat, constraints: Constraints<UInt8>)
        -> CheckedVerdict<UInt8>
    {
        checkedDoor(utf8, format, constraints) { $0.load(as: UInt8.self) }
    }

    /// Judges a number the caller already holds — a JSON parser's `Double`, a spreadsheet's
    /// numeric cell — at the u8 door by `constraints`, on its sign and the shortest decimal
    /// that names it: the judgement ``u8(_:format:constraints:)-swift.type.method`` makes of
    /// that text. NaN is ``CastFailure/malformed``; an infinity is ``CastFailure/outOfRange`` at
    /// a float or decimal door and ``CastFailure/malformed`` at an integer door. With no text to
    /// point into, every fault and violation has an empty span.
    public static func u8FromDouble(_ value: Double, constraints: Constraints<UInt8>) -> CheckedVerdict<UInt8> {
        checkedFromDouble(value, constraints) { $0.load(as: UInt8.self) }
    }

    // MARK: - checked u16

    /// Casts text as ``u16(_:format:)-swift.type.method`` does, then judges the value by
    /// `constraints`.
    public static func u16(_ text: String, format: NumFormat, constraints: Constraints<UInt16>)
        -> CheckedVerdict<UInt16>
    {
        withUTF8(text) { u16($0, format: format, constraints: constraints) }
    }

    /// See ``u16(_:format:constraints:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u16(_ utf8: [UInt8], format: NumFormat, constraints: Constraints<UInt16>)
        -> CheckedVerdict<UInt16>
    {
        utf8.withUnsafeBytes { u16($0, format: format, constraints: constraints) }
    }

    /// See ``u16(_:format:constraints:)-swift.type.method``; input as a raw view of UTF-8
    /// bytes, the primitive the `String` and `[UInt8]` forms wrap.
    public static func u16(_ utf8: UnsafeRawBufferPointer, format: NumFormat, constraints: Constraints<UInt16>)
        -> CheckedVerdict<UInt16>
    {
        checkedDoor(utf8, format, constraints) { $0.load(as: UInt16.self) }
    }

    /// Judges a number the caller already holds — a JSON parser's `Double`, a spreadsheet's
    /// numeric cell — at the u16 door by `constraints`, on its sign and the shortest decimal
    /// that names it: the judgement ``u16(_:format:constraints:)-swift.type.method`` makes of
    /// that text. NaN is ``CastFailure/malformed``; an infinity is ``CastFailure/outOfRange`` at
    /// a float or decimal door and ``CastFailure/malformed`` at an integer door. With no text to
    /// point into, every fault and violation has an empty span.
    public static func u16FromDouble(_ value: Double, constraints: Constraints<UInt16>) -> CheckedVerdict<UInt16> {
        checkedFromDouble(value, constraints) { $0.load(as: UInt16.self) }
    }

    // MARK: - checked u32

    /// Casts text as ``u32(_:format:)-swift.type.method`` does, then judges the value by
    /// `constraints`.
    public static func u32(_ text: String, format: NumFormat, constraints: Constraints<UInt32>)
        -> CheckedVerdict<UInt32>
    {
        withUTF8(text) { u32($0, format: format, constraints: constraints) }
    }

    /// See ``u32(_:format:constraints:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u32(_ utf8: [UInt8], format: NumFormat, constraints: Constraints<UInt32>)
        -> CheckedVerdict<UInt32>
    {
        utf8.withUnsafeBytes { u32($0, format: format, constraints: constraints) }
    }

    /// See ``u32(_:format:constraints:)-swift.type.method``; input as a raw view of UTF-8
    /// bytes, the primitive the `String` and `[UInt8]` forms wrap.
    public static func u32(_ utf8: UnsafeRawBufferPointer, format: NumFormat, constraints: Constraints<UInt32>)
        -> CheckedVerdict<UInt32>
    {
        checkedDoor(utf8, format, constraints) { $0.load(as: UInt32.self) }
    }

    /// Judges a number the caller already holds — a JSON parser's `Double`, a spreadsheet's
    /// numeric cell — at the u32 door by `constraints`, on its sign and the shortest decimal
    /// that names it: the judgement ``u32(_:format:constraints:)-swift.type.method`` makes of
    /// that text. NaN is ``CastFailure/malformed``; an infinity is ``CastFailure/outOfRange`` at
    /// a float or decimal door and ``CastFailure/malformed`` at an integer door. With no text to
    /// point into, every fault and violation has an empty span.
    public static func u32FromDouble(_ value: Double, constraints: Constraints<UInt32>) -> CheckedVerdict<UInt32> {
        checkedFromDouble(value, constraints) { $0.load(as: UInt32.self) }
    }

    // MARK: - checked u64

    /// Casts text as ``u64(_:format:)-swift.type.method`` does, then judges the value by
    /// `constraints`.
    public static func u64(_ text: String, format: NumFormat, constraints: Constraints<UInt64>)
        -> CheckedVerdict<UInt64>
    {
        withUTF8(text) { u64($0, format: format, constraints: constraints) }
    }

    /// See ``u64(_:format:constraints:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u64(_ utf8: [UInt8], format: NumFormat, constraints: Constraints<UInt64>)
        -> CheckedVerdict<UInt64>
    {
        utf8.withUnsafeBytes { u64($0, format: format, constraints: constraints) }
    }

    /// See ``u64(_:format:constraints:)-swift.type.method``; input as a raw view of UTF-8
    /// bytes, the primitive the `String` and `[UInt8]` forms wrap.
    public static func u64(_ utf8: UnsafeRawBufferPointer, format: NumFormat, constraints: Constraints<UInt64>)
        -> CheckedVerdict<UInt64>
    {
        checkedDoor(utf8, format, constraints) { $0.load(as: UInt64.self) }
    }

    /// Judges a number the caller already holds — a JSON parser's `Double`, a spreadsheet's
    /// numeric cell — at the u64 door by `constraints`, on its sign and the shortest decimal
    /// that names it: the judgement ``u64(_:format:constraints:)-swift.type.method`` makes of
    /// that text. NaN is ``CastFailure/malformed``; an infinity is ``CastFailure/outOfRange`` at
    /// a float or decimal door and ``CastFailure/malformed`` at an integer door. With no text to
    /// point into, every fault and violation has an empty span.
    public static func u64FromDouble(_ value: Double, constraints: Constraints<UInt64>) -> CheckedVerdict<UInt64> {
        checkedFromDouble(value, constraints) { $0.load(as: UInt64.self) }
    }

    // MARK: - checked f32

    /// Casts text as ``f32(_:format:)-swift.type.method`` does, then judges the value by
    /// `constraints`.
    public static func f32(_ text: String, format: NumFormat, constraints: Constraints<Float>)
        -> CheckedVerdict<Float>
    {
        withUTF8(text) { f32($0, format: format, constraints: constraints) }
    }

    /// See ``f32(_:format:constraints:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func f32(_ utf8: [UInt8], format: NumFormat, constraints: Constraints<Float>)
        -> CheckedVerdict<Float>
    {
        utf8.withUnsafeBytes { f32($0, format: format, constraints: constraints) }
    }

    /// See ``f32(_:format:constraints:)-swift.type.method``; input as a raw view of UTF-8
    /// bytes, the primitive the `String` and `[UInt8]` forms wrap.
    public static func f32(_ utf8: UnsafeRawBufferPointer, format: NumFormat, constraints: Constraints<Float>)
        -> CheckedVerdict<Float>
    {
        checkedDoor(utf8, format, constraints) { $0.load(as: Float.self) }
    }

    /// Judges a number the caller already holds — a JSON parser's `Double`, a spreadsheet's
    /// numeric cell — at the f32 door by `constraints`, on its sign and the shortest decimal
    /// that names it: the judgement ``f32(_:format:constraints:)-swift.type.method`` makes of
    /// that text. NaN is ``CastFailure/malformed``; an infinity is ``CastFailure/outOfRange`` at
    /// a float or decimal door and ``CastFailure/malformed`` at an integer door. With no text to
    /// point into, every fault and violation has an empty span.
    public static func f32FromDouble(_ value: Double, constraints: Constraints<Float>) -> CheckedVerdict<Float> {
        checkedFromDouble(value, constraints) { $0.load(as: Float.self) }
    }

    // MARK: - checked f64

    /// Casts text as ``f64(_:format:)-swift.type.method`` does, then judges the value by
    /// `constraints`.
    public static func f64(_ text: String, format: NumFormat, constraints: Constraints<Double>)
        -> CheckedVerdict<Double>
    {
        withUTF8(text) { f64($0, format: format, constraints: constraints) }
    }

    /// See ``f64(_:format:constraints:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func f64(_ utf8: [UInt8], format: NumFormat, constraints: Constraints<Double>)
        -> CheckedVerdict<Double>
    {
        utf8.withUnsafeBytes { f64($0, format: format, constraints: constraints) }
    }

    /// See ``f64(_:format:constraints:)-swift.type.method``; input as a raw view of UTF-8
    /// bytes, the primitive the `String` and `[UInt8]` forms wrap.
    public static func f64(_ utf8: UnsafeRawBufferPointer, format: NumFormat, constraints: Constraints<Double>)
        -> CheckedVerdict<Double>
    {
        checkedDoor(utf8, format, constraints) { $0.load(as: Double.self) }
    }

    /// Judges a number the caller already holds — a JSON parser's `Double`, a spreadsheet's
    /// numeric cell — at the f64 door by `constraints`, on its sign and the shortest decimal
    /// that names it: the judgement ``f64(_:format:constraints:)-swift.type.method`` makes of
    /// that text. NaN is ``CastFailure/malformed``; an infinity is ``CastFailure/outOfRange`` at
    /// a float or decimal door and ``CastFailure/malformed`` at an integer door. With no text to
    /// point into, every fault and violation has an empty span.
    public static func f64FromDouble(_ value: Double, constraints: Constraints<Double>) -> CheckedVerdict<Double> {
        checkedFromDouble(value, constraints) { $0.load(as: Double.self) }
    }

    // MARK: - checked decimal

    /// Casts text as ``decimal(_:format:)-swift.type.method`` does, then judges the value by
    /// `constraints`.
    public static func decimal(_ text: String, format: NumFormat, constraints: Constraints<Decimal>)
        -> CheckedVerdict<Decimal>
    {
        withUTF8(text) { decimal($0, format: format, constraints: constraints) }
    }

    /// See ``decimal(_:format:constraints:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func decimal(_ utf8: [UInt8], format: NumFormat, constraints: Constraints<Decimal>)
        -> CheckedVerdict<Decimal>
    {
        utf8.withUnsafeBytes { decimal($0, format: format, constraints: constraints) }
    }

    /// See ``decimal(_:format:constraints:)-swift.type.method``; input as a raw view of UTF-8
    /// bytes, the primitive the `String` and `[UInt8]` forms wrap.
    public static func decimal(_ utf8: UnsafeRawBufferPointer, format: NumFormat, constraints: Constraints<Decimal>)
        -> CheckedVerdict<Decimal>
    {
        checkedDoor(utf8, format, constraints, read: Interop.decimal)
    }

    /// Judges a number the caller already holds — a JSON parser's `Double`, a spreadsheet's
    /// numeric cell — at the decimal door by `constraints`, on its sign and the shortest decimal
    /// that names it: the judgement ``decimal(_:format:constraints:)-swift.type.method`` makes of
    /// that text. NaN is ``CastFailure/malformed``; an infinity is ``CastFailure/outOfRange`` at
    /// a float or decimal door and ``CastFailure/malformed`` at an integer door. With no text to
    /// point into, every fault and violation has an empty span.
    public static func decimalFromDouble(_ value: Double, constraints: Constraints<Decimal>) -> CheckedVerdict<Decimal>
    {
        checkedFromDouble(value, constraints, read: Interop.decimal)
    }

    // MARK: - text

    /// Casts text as text: the input itself, once it is valid UTF-8, untrimmed. Empty input is
    /// ``CastFailure/empty``. A Swift `String` is always valid UTF-8, so this form is
    /// `.success` with the caller's own string for anything but `""`.
    public static func text(_ text: String) -> Verdict<String> {
        withUTF8(text) { plainText($0, whole: text) }
    }

    /// See ``text(_:)-swift.type.method``; input as raw UTF-8 bytes, where invalid UTF-8 is
    /// ``CastFailure/malformed``, spanning the first invalid sequence.
    public static func text(_ utf8: [UInt8]) -> Verdict<String> {
        utf8.withUnsafeBytes { plainText($0, whole: nil) }
    }

    /// See ``text(_:)-swift.type.method``; input as a raw view of UTF-8 bytes, the primitive
    /// the `String` and `[UInt8]` forms wrap.
    public static func text(_ utf8: UnsafeRawBufferPointer) -> Verdict<String> {
        plainText(utf8, whole: nil)
    }

    /// Casts text as text, then judges it by `constraints`: trimmed of Unicode `White_Space`
    /// when `trim` says so, its length counted in code points, and an `enum` matched under
    /// Unicode simple case folding — a match delivers the caller's own declared entry, and its
    /// index. Spans are UTF-8 bytes into the input, as a fault's are.
    public static func text(_ text: String, constraints: Constraints<String>) -> CheckedVerdict<String> {
        withUTF8(text) { checkedText($0, whole: text, constraints) }
    }

    /// See ``text(_:constraints:)-swift.type.method``; input as raw UTF-8 bytes, where invalid
    /// UTF-8 is ``CastFailure/malformed``, spanning the first invalid sequence.
    public static func text(_ utf8: [UInt8], constraints: Constraints<String>) -> CheckedVerdict<String> {
        utf8.withUnsafeBytes { checkedText($0, whole: nil, constraints) }
    }

    /// See ``text(_:constraints:)-swift.type.method``; input as a raw view of UTF-8 bytes, the
    /// primitive the `String` and `[UInt8]` forms wrap.
    public static func text(_ utf8: UnsafeRawBufferPointer, constraints: Constraints<String>)
        -> CheckedVerdict<String>
    {
        checkedText(utf8, whole: nil, constraints)
    }

    private static func plainText(_ utf8: UnsafeRawBufferPointer, whole: String?) -> Verdict<String> {
        var spanOut: OutScratch = (0, 0)
        var faultOut = Interop.freshRawFaultDetail
        let code = callDetailed(door: DoorCode.text, param: 0, utf8, out: &spanOut, fault: &faultOut)
        precondition(code != -1, "libhypercast reported a contract violation — a binding bug, please report it")
        guard code == 0 else {
            return .fault(withUnsafeBytes(of: &faultOut) { detailedFault(code, $0) })
        }
        let span = withUnsafeBytes(of: &spanOut) { $0.load(as: SpanScratch.self) }
        return .success(slice(utf8, span, whole: whole))
    }

    private static func checkedText(
        _ utf8: UnsafeRawBufferPointer, whole: String?, _ constraints: Constraints<String>
    ) -> CheckedVerdict<String> {
        var out: OutScratch = (0, 0)
        var faultOut = Interop.freshRawFaultDetail
        var check = noCheck
        let code = checked(utf8, nil, constraints, &out, &faultOut, &check)
        guard code == 0 else { return .rejection(rejection(code, faultOut, check)) }
        if check.2 != UInt32.max {
            return .success(found(constraints.entry(check.2), check))
        }
        let span = withUnsafeBytes(of: &out) { $0.load(as: SpanScratch.self) }
        return .success(found(slice(utf8, span, whole: whole), check))
    }

    /// The span of the input the core judged to be the text: the caller's own string when it
    /// is all of it, otherwise those bytes, which the core proved are valid UTF-8.
    private static func slice(_ utf8: UnsafeRawBufferPointer, _ span: SpanScratch, whole: String?) -> String {
        let offset = Int(span.0)
        let length = Int(span.1)
        if let whole, offset == 0, length == utf8.count {
            return whole
        }
        return String(decoding: utf8[offset..<(offset + length)], as: UTF8.self)
    }
}
