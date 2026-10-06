import Foundation
import HyperCastCore

/// Allocation-lean scalar casts — booleans, numerics (integers, reals, exact decimals),
/// UUIDs, temporals — calling directly into the native `hypercast` core through
/// `@convention(c)` function pointers. The core is a static library linked into the
/// executable on every platform this package builds for (`HyperCastCore`, a SwiftPM binary
/// target), so there is nothing to find, open or deploy at run time. Every door returns a
/// ``Verdict``: the value, or a ``Fault`` with a closed reason and the offending byte span.
/// No door throws — bad data is a ``Fault``, and the `throws` on each door is kept from
/// when macOS and Windows loaded a shared library that could fail to load — and a
/// precondition failure means a caller bug, never data.
///
/// Door names mirror the native ABI (`i32`, `f64`, `decimal`, `timestamp`, …) so the polyglot surface
/// reads identically across bindings. Swift-flavored fidelity: `UInt8`–`UInt64` are native
/// (no widening games), `Duration` carries the core's nanoseconds exactly, and
/// `DateComponents` keeps date/time-of-day digit-perfect; `Date` (the instant lingua
/// franca) is a `Double` of seconds, so sub-microsecond fidelity degrades toward the
/// window's edges — stated, not hidden. Foundation's `Decimal` holds every value the
/// decimal door produces exactly, and the door's result is canonical — exact trailing
/// fraction zeros trimmed, so the scale is minimal — which is precisely what `Decimal`
/// represents: nothing is lost between the core and the presentation.
public enum Cast {
    private typealias PlainFn =
        @convention(c) (
            UnsafePointer<UInt8>?, UInt, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?
        ) -> Int32
    private typealias NumericFn =
        @convention(c) (
            UnsafePointer<UInt8>?, UInt, UnsafeRawPointer?, UnsafeMutableRawPointer?,
            UnsafeMutableRawPointer?
        ) -> Int32
    private typealias UnixFn =
        @convention(c) (
            UnsafePointer<UInt8>?, UInt, UInt32, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?
        ) -> Int32
    private typealias VersionFn = @convention(c) () -> UInt32
    // The typed doors read a double the caller holds instead of text.
    private typealias TypedFn =
        @convention(c) (
            Double, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?
        ) -> Int32
    private typealias TypedDeclaredFn =
        @convention(c) (
            Double, UInt32, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?
        ) -> Int32

    // The native exports as one table, so every door reaches them the same way. A class:
    // a reference is one retain where a struct of function pointers would be copied.
    // Immutable once built, and C function pointers carry no state of their own.
    private final class LoadedLibrary: Sendable {
        let bool: PlainFn
        let i8: NumericFn
        let i16: NumericFn
        let i32: NumericFn
        let i64: NumericFn
        let u8: NumericFn
        let u16: NumericFn
        let u32: NumericFn
        let u64: NumericFn
        let f32: NumericFn
        let f64: NumericFn
        let decimal: NumericFn
        let uuid: PlainFn
        let timestamp: PlainFn
        let unix: UnixFn
        // cast_excel_serial shares the unix ABI shape too.
        let excelSerial: UnixFn
        let date: PlainFn
        // cast_date_ordered and cast_datetime share the unix ABI shape (ptr, len, u32, out, fault).
        let dateOrdered: UnixFn
        let dateTime: UnixFn
        let time: PlainFn
        let duration: PlainFn
        let decimalFromF64: TypedFn
        let excelSerialFromF64: TypedDeclaredFn
        let excelTime: TypedFn
        let excelDuration: TypedFn
        let version: VersionFn

        init(
            bool: PlainFn,
            i8: NumericFn, i16: NumericFn, i32: NumericFn, i64: NumericFn,
            u8: NumericFn, u16: NumericFn, u32: NumericFn, u64: NumericFn,
            f32: NumericFn, f64: NumericFn, decimal: NumericFn, uuid: PlainFn, timestamp: PlainFn,
            unix: UnixFn, excelSerial: UnixFn, date: PlainFn, dateOrdered: UnixFn, dateTime: UnixFn,
            time: PlainFn, duration: PlainFn,
            decimalFromF64: TypedFn, excelSerialFromF64: TypedDeclaredFn, excelTime: TypedFn,
            excelDuration: TypedFn, version: VersionFn
        ) {
            self.bool = bool
            self.i8 = i8; self.i16 = i16; self.i32 = i32; self.i64 = i64
            self.u8 = u8; self.u16 = u16; self.u32 = u32; self.u64 = u64
            self.f32 = f32; self.f64 = f64; self.decimal = decimal
            self.uuid = uuid
            self.timestamp = timestamp; self.unix = unix; self.excelSerial = excelSerial
            self.date = date; self.dateOrdered = dateOrdered; self.dateTime = dateTime
            self.time = time; self.duration = duration
            self.decimalFromF64 = decimalFromF64; self.excelSerialFromF64 = excelSerialFromF64
            self.excelTime = excelTime; self.excelDuration = excelDuration
            self.version = version
        }
    }

    // Linked in: the C declarations are the table, and there is nothing to find or open.
    private static let library = LoadedLibrary(
        bool: cast_bool,
        i8: cast_i8, i16: cast_i16, i32: cast_i32, i64: cast_i64,
        u8: cast_u8, u16: cast_u16, u32: cast_u32, u64: cast_u64,
        f32: cast_f32, f64: cast_f64,
        decimal: cast_decimal,
        uuid: cast_uuid,
        timestamp: cast_timestamp,
        unix: cast_unix,
        excelSerial: cast_excel_serial,
        date: cast_date,
        dateOrdered: cast_date_ordered,
        dateTime: cast_datetime,
        time: cast_time,
        duration: cast_duration,
        decimalFromF64: cast_decimal_from_f64,
        excelSerialFromF64: cast_excel_serial_from_f64,
        excelTime: cast_excel_time,
        excelDuration: cast_excel_duration,
        version: hypercast_version)

    // `throws` only so every door keeps one shape; the core is always there.
    private static func loaded() throws -> LoadedLibrary {
        library
    }

    /// Whether the native core is usable. Always `true`: the core is linked into the
    /// executable on every platform this package builds for, and a platform with no
    /// prebuilt core fails to compile rather than at run time. Kept for callers that gated
    /// on it when macOS and Windows loaded a shared library.
    public static var isAvailable: Bool { true }

    private static func fault(_ code: Int32, _ raw: UnsafeRawBufferPointer) -> Fault {
        precondition(code != -1, "libhypercast reported a contract violation — a binding bug, please report it")
        return Interop.fault(
            code: code,
            offset: raw.load(fromByteOffset: 0, as: UInt32.self),
            length: raw.load(fromByteOffset: 4, as: UInt32.self))
    }

    // The scratch every door needs, on the stack: 16 bytes for the widest out-value (a
    // timestamp or a civil date-time — two UInt64s so the core's i64/u64 stores land
    // 8-aligned) and 8 for the fault span. These used to be three heap `[UInt8]` arrays
    // per call (four with the format), which was most of what a door cost; a tuple of
    // fixed-width integers has no heap existence at all. Internal, not private, so the test
    // suite can pin both sizes to the ones rust/src/abi.rs and ffi.rs pin.
    typealias OutScratch = (UInt64, UInt64)
    typealias FaultScratch = (UInt32, UInt32)
    // The native `RawNumFormat`, under the name the test suite pins its size by.
    typealias RawNumFormat = Interop.RawNumFormat

    private static func inputPointer(_ utf8: UnsafeRawBufferPointer) -> UnsafePointer<UInt8>? {
        // An empty buffer may carry a nil base address; the ABI never dereferences at len 0.
        utf8.baseAddress?.assumingMemoryBound(to: UInt8.self)
    }

    /// Runs a culture-insensitive door over the caller's bytes, the verdict assembled from
    /// whichever half of the scratch the code says is live.
    private static func plainDoor<T>(
        _ fn: PlainFn, _ utf8: UnsafeRawBufferPointer, read: (UnsafeRawBufferPointer) -> T
    ) -> Verdict<T> {
        var out: OutScratch = (0, 0)
        var faultOut: FaultScratch = (0, 0)
        let code = withUnsafeMutableBytes(of: &out) { outRaw in
            withUnsafeMutableBytes(of: &faultOut) { faultRaw in
                fn(inputPointer(utf8), UInt(utf8.count), outRaw.baseAddress, faultRaw.baseAddress)
            }
        }
        if code == 0 {
            return .success(withUnsafeBytes(of: &out, read))
        }
        return .fault(withUnsafeBytes(of: &faultOut) { fault(code, $0) })
    }

    private static func numericDoor<T>(
        _ fn: NumericFn, _ utf8: UnsafeRawBufferPointer, _ format: NumFormat,
        read: (UnsafeRawBufferPointer) -> T
    ) -> Verdict<T> {
        var rawFormat = Interop.rawFormat(format)
        var out: OutScratch = (0, 0)
        var faultOut: FaultScratch = (0, 0)
        let code = withUnsafeMutableBytes(of: &out) { outRaw in
            withUnsafeMutableBytes(of: &faultOut) { faultRaw in
                withUnsafeBytes(of: &rawFormat) { formatRaw in
                    fn(
                        inputPointer(utf8), UInt(utf8.count), formatRaw.baseAddress,
                        outRaw.baseAddress, faultRaw.baseAddress)
                }
            }
        }
        if code == 0 {
            return .success(withUnsafeBytes(of: &out, read))
        }
        return .fault(withUnsafeBytes(of: &faultOut) { fault(code, $0) })
    }

    /// The (ptr, len, u32, out, fault) shape — a declared precision, epoch or field order.
    private static func unixDoor<T>(
        _ fn: UnixFn, _ utf8: UnsafeRawBufferPointer, _ discriminant: UInt32,
        read: (UnsafeRawBufferPointer) -> T
    ) -> Verdict<T> {
        var out: OutScratch = (0, 0)
        var faultOut: FaultScratch = (0, 0)
        let code = withUnsafeMutableBytes(of: &out) { outRaw in
            withUnsafeMutableBytes(of: &faultOut) { faultRaw in
                fn(
                    inputPointer(utf8), UInt(utf8.count), discriminant, outRaw.baseAddress,
                    faultRaw.baseAddress)
            }
        }
        if code == 0 {
            return .success(withUnsafeBytes(of: &out, read))
        }
        return .fault(withUnsafeBytes(of: &faultOut) { fault(code, $0) })
    }

    /// The typed shape: a double instead of text, then the declared epoch when the door
    /// takes one. There is no text for a fault's span to index, so the core leaves it empty.
    private static func typedDoor<T>(
        _ value: Double, _ call: (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Int32,
        read: (UnsafeRawBufferPointer) -> T
    ) -> Verdict<T> {
        var out: OutScratch = (0, 0)
        var faultOut: FaultScratch = (0, 0)
        let code = withUnsafeMutableBytes(of: &out) { outRaw in
            withUnsafeMutableBytes(of: &faultOut) { faultRaw in
                call(outRaw.baseAddress, faultRaw.baseAddress)
            }
        }
        if code == 0 {
            return .success(withUnsafeBytes(of: &out, read))
        }
        return .fault(withUnsafeBytes(of: &faultOut) { fault(code, $0) })
    }

    /// A native Swift `String` already stores contiguous UTF-8, so `withUTF8` hands the
    /// door a view of the string's own bytes — no `Array(text.utf8)` copy, which used to
    /// be one heap allocation and a memcpy on every `String` door.
    private static func withUTF8<T>(_ text: String, _ body: (UnsafeRawBufferPointer) throws -> T) rethrows -> T {
        var text = text
        return try text.withUTF8 { try body(UnsafeRawBufferPointer($0)) }
    }

    // MARK: - boolean

    /// Casts boolean text: `true`/`false` plus the conventions untrusted sources actually
    /// send (`t/f`, `yes/no`, `y/n`, `1/0`, `on/off`, `enabled/disabled`,
    /// `active/inactive`, `checked/unchecked`, `in/out`), ASCII case-insensitive.
    public static func bool(_ text: String) throws -> Verdict<Bool> {
        try withUTF8(text) { try bool($0) }
    }

    /// See ``bool(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func bool(_ utf8: [UInt8]) throws -> Verdict<Bool> {
        try utf8.withUnsafeBytes { try bool($0) }
    }

    /// See ``bool(_:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func bool(_ utf8: UnsafeRawBufferPointer) throws -> Verdict<Bool> {
        plainDoor(try loaded().bool, utf8) { $0.load(as: UInt8.self) != 0 }
    }

    // MARK: - integers (the type's own range; grouping, parens, exponent, radix prefixes, currency per format)

    /// Casts integer text to a signed 8-bit value under the declared format: the type's own
    /// range, declared grouping, accounting parentheses, non-negative exponent,
    /// `0x`/`&H`/`0b` two's-complement radix prefixes, and the declared currency symbol.
    public static func i8(_ text: String, format: NumFormat) throws -> Verdict<Int8> {
        try withUTF8(text) { try i8($0, format: format) }
    }

    /// See ``i8(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i8(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<Int8> {
        try utf8.withUnsafeBytes { try i8($0, format: format) }
    }

    /// See ``i8(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func i8(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Int8> {
        numericDoor(try loaded().i8, utf8, format) { $0.load(as: Int8.self) }
    }

    /// Casts integer text to a signed 16-bit value. Notation rules as
    /// ``i8(_:format:)-swift.type.method``.
    public static func i16(_ text: String, format: NumFormat) throws -> Verdict<Int16> {
        try withUTF8(text) { try i16($0, format: format) }
    }

    /// See ``i16(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i16(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<Int16> {
        try utf8.withUnsafeBytes { try i16($0, format: format) }
    }

    /// See ``i16(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func i16(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Int16> {
        numericDoor(try loaded().i16, utf8, format) { $0.load(as: Int16.self) }
    }

    /// Casts integer text to a signed 32-bit value. Notation rules as
    /// ``i8(_:format:)-swift.type.method``.
    public static func i32(_ text: String, format: NumFormat) throws -> Verdict<Int32> {
        try withUTF8(text) { try i32($0, format: format) }
    }

    /// See ``i32(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i32(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<Int32> {
        try utf8.withUnsafeBytes { try i32($0, format: format) }
    }

    /// See ``i32(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func i32(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Int32> {
        numericDoor(try loaded().i32, utf8, format) { $0.load(as: Int32.self) }
    }

    /// Casts integer text to a signed 64-bit value. Notation rules as
    /// ``i8(_:format:)-swift.type.method``.
    public static func i64(_ text: String, format: NumFormat) throws -> Verdict<Int64> {
        try withUTF8(text) { try i64($0, format: format) }
    }

    /// See ``i64(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i64(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<Int64> {
        try utf8.withUnsafeBytes { try i64($0, format: format) }
    }

    /// See ``i64(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func i64(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Int64> {
        numericDoor(try loaded().i64, utf8, format) { $0.load(as: Int64.self) }
    }

    /// Casts integer text to an unsigned 8-bit value. Notation rules as
    /// ``i8(_:format:)-swift.type.method``.
    public static func u8(_ text: String, format: NumFormat) throws -> Verdict<UInt8> {
        try withUTF8(text) { try u8($0, format: format) }
    }

    /// See ``u8(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u8(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<UInt8> {
        try utf8.withUnsafeBytes { try u8($0, format: format) }
    }

    /// See ``u8(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func u8(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<UInt8> {
        numericDoor(try loaded().u8, utf8, format) { $0.load(as: UInt8.self) }
    }

    /// Casts integer text to an unsigned 16-bit value. Notation rules as
    /// ``i8(_:format:)-swift.type.method``.
    public static func u16(_ text: String, format: NumFormat) throws -> Verdict<UInt16> {
        try withUTF8(text) { try u16($0, format: format) }
    }

    /// See ``u16(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u16(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<UInt16> {
        try utf8.withUnsafeBytes { try u16($0, format: format) }
    }

    /// See ``u16(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func u16(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<UInt16> {
        numericDoor(try loaded().u16, utf8, format) { $0.load(as: UInt16.self) }
    }

    /// Casts integer text to an unsigned 32-bit value. Notation rules as
    /// ``i8(_:format:)-swift.type.method``.
    public static func u32(_ text: String, format: NumFormat) throws -> Verdict<UInt32> {
        try withUTF8(text) { try u32($0, format: format) }
    }

    /// See ``u32(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u32(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<UInt32> {
        try utf8.withUnsafeBytes { try u32($0, format: format) }
    }

    /// See ``u32(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func u32(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<UInt32> {
        numericDoor(try loaded().u32, utf8, format) { $0.load(as: UInt32.self) }
    }

    /// Casts integer text to an unsigned 64-bit value — natively unsigned, no widening games.
    /// Notation rules as ``i8(_:format:)-swift.type.method``.
    public static func u64(_ text: String, format: NumFormat) throws -> Verdict<UInt64> {
        try withUTF8(text) { try u64($0, format: format) }
    }

    /// See ``u64(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u64(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<UInt64> {
        try utf8.withUnsafeBytes { try u64($0, format: format) }
    }

    /// See ``u64(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func u64(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<UInt64> {
        numericDoor(try loaded().u64, utf8, format) { $0.load(as: UInt64.self) }
    }

    // MARK: - reals (finite only; separators, parens, exponent, percent, currency per format)

    /// Casts real text to a `Float` under the declared format: finite values only, declared
    /// separators and grouping, parentheses, exponent, trailing percent (`50%` is 0.5), and
    /// the declared currency symbol.
    public static func f32(_ text: String, format: NumFormat) throws -> Verdict<Float> {
        try withUTF8(text) { try f32($0, format: format) }
    }

    /// See ``f32(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func f32(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<Float> {
        try utf8.withUnsafeBytes { try f32($0, format: format) }
    }

    /// See ``f32(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func f32(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Float> {
        numericDoor(try loaded().f32, utf8, format) { $0.load(as: Float.self) }
    }

    /// Casts real text to a `Double`. Notation rules as ``f32(_:format:)-swift.type.method``.
    public static func f64(_ text: String, format: NumFormat) throws -> Verdict<Double> {
        try withUTF8(text) { try f64($0, format: format) }
    }

    /// See ``f64(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func f64(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<Double> {
        try utf8.withUnsafeBytes { try f64($0, format: format) }
    }

    /// See ``f64(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func f64(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Double> {
        numericDoor(try loaded().f64, utf8, format) { $0.load(as: Double.self) }
    }

    // MARK: - decimal (exact; same grammar as the reals, never rounds)

    /// Casts decimal text to Foundation's `Decimal` — exactly. Same grammar as
    /// ``f64(_:format:)-swift.type.method`` (declared separators and grouping, parentheses,
    /// exponent, percent, currency), but no float is ever formed: `0.1` is one tenth and
    /// `50%` is exactly `0.5`. The core carries a sign, a 96-bit magnitude and a base-10
    /// scale of 0 through 28 — the shape .NET's `decimal` stores — and never rounds: text
    /// with more precision than that is ``CastFailure/outOfRange``, not approximated. The
    /// result is canonical: exact trailing fraction zeros are trimmed, so `1.10`, `1.1` and
    /// `1.1000` are all magnitude 11 at scale 1, and zero is scale 0 and never negative —
    /// exactly what Foundation's `Decimal` represents, and its 38-digit mantissa holds every
    /// such value exactly.
    public static func decimal(_ text: String, format: NumFormat) throws -> Verdict<Decimal> {
        try withUTF8(text) { try decimal($0, format: format) }
    }

    /// See ``decimal(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func decimal(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<Decimal> {
        try utf8.withUnsafeBytes { try decimal($0, format: format) }
    }

    /// See ``decimal(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func decimal(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Decimal> {
        numericDoor(try loaded().decimal, utf8, format, read: Interop.decimal)
    }

    // MARK: - generic numeric (for a caller that is itself generic over the target)

    /// Casts numeric text to whichever of the eleven numeric targets `T` is — the door a
    /// caller that is itself generic over its target reaches for. Resolved statically:
    /// `Int8`/`Int16`/`Int32`/`Int64`, `UInt8`/`UInt16`/`UInt32`/`UInt64`, `Float`,
    /// `Double` and `Decimal`, each through its own concrete door under that door's rules.
    /// ``NumericCastTarget`` is closed to exactly those eleven, so any other `T` is refused
    /// by the compiler, not at run time.
    public static func numeric<T: NumericCastTarget>(_ text: String, format: NumFormat) throws -> Verdict<T> {
        try withUTF8(text) { try numeric($0, format: format) }
    }

    /// See ``numeric(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func numeric<T: NumericCastTarget>(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<T> {
        try utf8.withUnsafeBytes { try numeric($0, format: format) }
    }

    /// See ``numeric(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func numeric<T: NumericCastTarget>(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws
        -> Verdict<T>
    {
        try T.castNumeric(utf8, format: format)
    }

    // MARK: - uuid

    /// Casts UUID text — all five .NET `Guid` formats (D/N/B/P/X) plus
    /// `urn:uuid:`/`GUID:`/`UUID:` prefixes — to a Foundation `UUID`.
    public static func uuid(_ text: String) throws -> Verdict<UUID> {
        try withUTF8(text) { try uuid($0) }
    }

    /// See ``uuid(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func uuid(_ utf8: [UInt8]) throws -> Verdict<UUID> {
        try utf8.withUnsafeBytes { try uuid($0) }
    }

    /// See ``uuid(_:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func uuid(_ utf8: UnsafeRawBufferPointer) throws -> Verdict<UUID> {
        plainDoor(try loaded().uuid, utf8, read: Interop.uuid)
    }

    // MARK: - temporals

    /// Casts an RFC 3339 instant — zone **mandatory** — to a `Date`, normalized to UTC.
    /// `Date` is a `Double` of seconds, so sub-microsecond fidelity degrades toward the
    /// 0001/9999 window edges; the digit-perfect `DateComponents` that
    /// ``time(_:)-swift.type.method`` returns has no counterpart for instants in Foundation,
    /// and that trade is stated rather than hidden.
    public static func timestamp(_ text: String) throws -> Verdict<Date> {
        try withUTF8(text) { try timestamp($0) }
    }

    /// See ``timestamp(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func timestamp(_ utf8: [UInt8]) throws -> Verdict<Date> {
        try utf8.withUnsafeBytes { try timestamp($0) }
    }

    /// See ``timestamp(_:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func timestamp(_ utf8: UnsafeRawBufferPointer) throws -> Verdict<Date> {
        plainDoor(try loaded().timestamp, utf8, read: Interop.instant)
    }

    /// Casts an integer Unix-epoch value under a caller-declared unit to a `Date`.
    public static func unix(_ text: String, precision: UnixPrecision) throws -> Verdict<Date> {
        try withUTF8(text) { try unix($0, precision: precision) }
    }

    /// See ``unix(_:precision:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func unix(_ utf8: [UInt8], precision: UnixPrecision) throws -> Verdict<Date> {
        try utf8.withUnsafeBytes { try unix($0, precision: precision) }
    }

    /// See ``unix(_:precision:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func unix(_ utf8: UnsafeRawBufferPointer, precision: UnixPrecision) throws -> Verdict<Date> {
        unixDoor(try loaded().unix, utf8, precision.rawValue, read: Interop.instant)
    }

    /// Casts an Excel date serial under a caller-declared ``ExcelEpoch`` to a `Date`. The
    /// whole part counts days from the system's own day zero and the fraction is the time
    /// of day, so `45292.75` is 2024-01-01T18:00:00Z. A cell carries no zone and none is
    /// invented.
    ///
    /// The 1900 system contains a day that never existed: serial `60` is 1900-02-29, kept
    /// deliberately because Lotus 1-2-3 wrongly treated 1900 as a leap year and Excel
    /// copied the bug for file compatibility. It is `.outOfRange` here — the same verdict
    /// ``date(_:)-swift.type.method`` gives the text `1900-02-29` — so every serial above
    /// it is shifted one day against a naive count.
    public static func excelSerial(_ text: String, epoch: ExcelEpoch) throws -> Verdict<Date> {
        try withUTF8(text) { try excelSerial($0, epoch: epoch) }
    }

    /// See ``excelSerial(_:epoch:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func excelSerial(_ utf8: [UInt8], epoch: ExcelEpoch) throws -> Verdict<Date> {
        try utf8.withUnsafeBytes { try excelSerial($0, epoch: epoch) }
    }

    /// See ``excelSerial(_:epoch:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func excelSerial(_ utf8: UnsafeRawBufferPointer, epoch: ExcelEpoch) throws -> Verdict<Date> {
        unixDoor(try loaded().excelSerial, utf8, epoch.rawValue, read: Interop.instant)
    }

    /// Casts a strict ISO 8601 `yyyy-MM-dd` calendar date to `DateComponents` (year, month,
    /// day) — digit-perfect, no calendar or zone attached, exactly what the core parsed.
    public static func date(_ text: String) throws -> Verdict<DateComponents> {
        try withUTF8(text) { try date($0) }
    }

    /// See ``date(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func date(_ utf8: [UInt8]) throws -> Verdict<DateComponents> {
        try utf8.withUnsafeBytes { try date($0) }
    }

    /// See ``date(_:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func date(_ utf8: UnsafeRawBufferPointer) throws -> Verdict<DateComponents> {
        plainDoor(try loaded().date, utf8, read: Interop.date)
    }

    /// Casts a separated calendar date — three digit fields joined by one consistent
    /// separator (`/`, `-`, or `.`) — under the caller-declared ``DateOrder`` to
    /// `DateComponents`: `1/7/2026` is January 7th or July 1st only because `order` said
    /// which. The year field is four digits wherever the order puts it (two-digit years
    /// mean century guessing, which never happens — malformed); the order-less
    /// ``date(_:)-swift.type.method`` overload stays strict ISO.
    public static func date(_ text: String, order: DateOrder) throws -> Verdict<DateComponents> {
        try withUTF8(text) { try date($0, order: order) }
    }

    /// See ``date(_:order:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func date(_ utf8: [UInt8], order: DateOrder) throws -> Verdict<DateComponents> {
        try utf8.withUnsafeBytes { try date($0, order: order) }
    }

    /// See ``date(_:order:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func date(_ utf8: UnsafeRawBufferPointer, order: DateOrder) throws -> Verdict<DateComponents> {
        unixDoor(try loaded().dateOrdered, utf8, order.rawValue, read: Interop.date)
    }

    /// Casts a zone-less civil date-time — the shape untrusted feeds actually send
    /// (`1/7/2026 3:04 PM`, `2026-01-07 15:04:05`) — under the caller-declared
    /// ``DateOrder`` to `DateComponents` (year through nanosecond, no zone). The date part
    /// follows ``date(_:order:)-swift.type.method``'s grammar; the optional time part (one
    /// space or `T` after the date) is 24-hour `h:mm[:ss[.f+]]` or 12-hour with an
    /// `AM`/`PM` marker; absent, the time is midnight. No zone is read and none is
    /// invented — the text named no instant, so no `timeZone` component is set; fusing one
    /// is the caller's job (``timestamp(_:)-swift.type.method`` stays the strict RFC 3339
    /// instant door).
    public static func dateTime(_ text: String, order: DateOrder) throws -> Verdict<DateComponents> {
        try withUTF8(text) { try dateTime($0, order: order) }
    }

    /// See ``dateTime(_:order:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func dateTime(_ utf8: [UInt8], order: DateOrder) throws -> Verdict<DateComponents> {
        try utf8.withUnsafeBytes { try dateTime($0, order: order) }
    }

    /// See ``dateTime(_:order:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func dateTime(_ utf8: UnsafeRawBufferPointer, order: DateOrder) throws -> Verdict<DateComponents> {
        unixDoor(try loaded().dateTime, utf8, order.rawValue, read: Interop.civil)
    }

    /// Casts an ISO 24-hour time-of-day to `DateComponents` (hour, minute, second,
    /// nanosecond) — full nanosecond fidelity.
    public static func time(_ text: String) throws -> Verdict<DateComponents> {
        try withUTF8(text) { try time($0) }
    }

    /// See ``time(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func time(_ utf8: [UInt8]) throws -> Verdict<DateComponents> {
        try utf8.withUnsafeBytes { try time($0) }
    }

    /// See ``time(_:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func time(_ utf8: UnsafeRawBufferPointer) throws -> Verdict<DateComponents> {
        plainDoor(try loaded().time, utf8, read: Interop.time)
    }

    /// Casts a duration (ISO 8601 fixed components, invariant colon form, or protobuf JSON
    /// seconds) to Swift's `Duration` — attosecond-backed, so the core's nanoseconds carry
    /// exactly, both signs.
    public static func duration(_ text: String) throws -> Verdict<Duration> {
        try withUTF8(text) { try duration($0) }
    }

    /// See ``duration(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func duration(_ utf8: [UInt8]) throws -> Verdict<Duration> {
        try utf8.withUnsafeBytes { try duration($0) }
    }

    /// See ``duration(_:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func duration(_ utf8: UnsafeRawBufferPointer) throws -> Verdict<Duration> {
        plainDoor(try loaded().duration, utf8, read: Interop.duration)
    }

    // MARK: - typed doors (a number the caller already holds, not text)

    /// Reads a number the caller already holds — the `Double` a workbook stores for a
    /// numeric cell — as the exact `Decimal` it names: the shortest decimal that rounds back
    /// to the double, the digits a spreadsheet writes for it. So `0.1` is one tenth, not the
    /// binary fraction nearest it, and `0.1 + 0.2` is `0.30000000000000004`. The result is
    /// canonical, as ``decimal(_:format:)-swift.type.method``'s is. NaN is `.malformed`; an
    /// infinity, a magnitude past 2⁹⁶ − 1 or more than 28 places is `.outOfRange` — the digits
    /// are never cut to fit. A typed door's ``Fault`` has no span: its offset and length are 0.
    public static func decimalFromDouble(_ value: Double) throws -> Verdict<Decimal> {
        let fn = try loaded().decimalFromF64
        return typedDoor(value, { fn(value, $0, $1) }, read: Interop.decimal)
    }

    /// Reads an Excel date serial the caller already holds as a `Double` under a declared
    /// ``ExcelEpoch`` — the twin of ``excelSerial(_:epoch:)-swift.type.method`` for a workbook
    /// reader that has the cell's number and no text — as the zone-less wall clock the cell
    /// holds, in the `DateComponents` ``dateTime(_:order:)-swift.type.method`` returns, its
    /// fraction snapped: the time with the fewest fractional-second digits that the same
    /// double stores, so Excel's 23:59:59 is read on the second rather than the nanoseconds of
    /// float noise the double carries. The 1900 system's phantom serial `60`, a
    /// serial below the system's first day and one past 9999-12-31 are `.outOfRange`; a
    /// negative, NaN or infinite serial is `.malformed`.
    public static func excelSerialFromDouble(_ value: Double, epoch: ExcelEpoch) throws -> Verdict<DateComponents> {
        let fn = try loaded().excelSerialFromF64
        return typedDoor(value, { fn(value, epoch.rawValue, $0, $1) }, read: Interop.civil)
    }

    /// Reads the fraction of an Excel serial the caller already holds as a `Double` as a time
    /// of day, in the components ``time(_:)-swift.type.method`` returns: `0.75` and
    /// `45292.75` are both 18:00. Snapped as the serial door snaps, and a fraction that snaps
    /// to a whole day is midnight. A negative, NaN or infinite serial is `.malformed`;
    /// one past 9999-12-31 is `.outOfRange`.
    public static func excelTime(_ value: Double) throws -> Verdict<DateComponents> {
        let fn = try loaded().excelTime
        return typedDoor(value, { fn(value, $0, $1) }, read: Interop.time)
    }

    /// Reads a number of days the caller already holds as a `Double` — what an elapsed-time
    /// format (`[h]:mm:ss`) stores — as a `Duration`: `1.5` is a day and twelve hours, and a
    /// negative span is negative, its size snapped as a serial's time. NaN or an infinity is
    /// `.malformed`; beyond ±10,000 years is `.outOfRange`.
    public static func excelDuration(_ value: Double) throws -> Verdict<Duration> {
        let fn = try loaded().excelDuration
        return typedDoor(value, { fn(value, $0, $1) }, read: Interop.duration)
    }

    /// Presents a verdict optionally: an ``CastFailure/empty`` fault becomes `nil` (Swift's
    /// absent), everything else flows through untouched.
    public static func optional<T>(_ verdict: Verdict<T>) -> Verdict<T>? {
        if case .fault(let fault) = verdict, fault.reason == .empty {
            return nil
        }
        return verdict
    }

    // MARK: - the native library itself

    /// The version of the native `libhypercast` linked into this executable, as
    /// `major.minor.patch` — the library's own answer (`hypercast_version`), not this
    /// package's tag — so a caller can prove the two agree before the first cast and name
    /// the mismatch when they don't. Never throws in practice; `throws` is kept from when
    /// macOS and Windows loaded a shared library at run time.
    public static func nativeVersion() throws -> String {
        Interop.version(try loaded().version())
    }
}

/// The closed set of targets ``Cast/numeric(_:format:)-swift.type.method`` resolves over:
/// exactly the eleven types the concrete numeric doors produce. The conformances are this
/// binding's to declare — one per door, below — and a caller's own type has no door to
/// route to, so conforming anything else is unsupported.
public protocol NumericCastTarget {
    /// Routes to this type's own door. ``Cast/numeric(_:format:)-swift.type.method`` is the
    /// entry point; this is the hook it dispatches through.
    static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Self>
}

extension Int8: NumericCastTarget {
    /// ``Cast/i8(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Int8> {
        try Cast.i8(utf8, format: format)
    }
}

extension Int16: NumericCastTarget {
    /// ``Cast/i16(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Int16> {
        try Cast.i16(utf8, format: format)
    }
}

extension Int32: NumericCastTarget {
    /// ``Cast/i32(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Int32> {
        try Cast.i32(utf8, format: format)
    }
}

extension Int64: NumericCastTarget {
    /// ``Cast/i64(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Int64> {
        try Cast.i64(utf8, format: format)
    }
}

extension UInt8: NumericCastTarget {
    /// ``Cast/u8(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<UInt8> {
        try Cast.u8(utf8, format: format)
    }
}

extension UInt16: NumericCastTarget {
    /// ``Cast/u16(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<UInt16> {
        try Cast.u16(utf8, format: format)
    }
}

extension UInt32: NumericCastTarget {
    /// ``Cast/u32(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<UInt32> {
        try Cast.u32(utf8, format: format)
    }
}

extension UInt64: NumericCastTarget {
    /// ``Cast/u64(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<UInt64> {
        try Cast.u64(utf8, format: format)
    }
}

extension Float: NumericCastTarget {
    /// ``Cast/f32(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Float> {
        try Cast.f32(utf8, format: format)
    }
}

extension Double: NumericCastTarget {
    /// ``Cast/f64(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Double> {
        try Cast.f64(utf8, format: format)
    }
}

extension Decimal: NumericCastTarget {
    /// ``Cast/decimal(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) throws -> Verdict<Decimal> {
        try Cast.decimal(utf8, format: format)
    }
}
