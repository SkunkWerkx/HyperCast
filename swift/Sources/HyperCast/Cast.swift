import Foundation

/// Allocation-lean scalar casts — booleans, numerics, UUIDs, temporals — calling directly
/// into the native `libhypercast` shared library via `dlopen`/`dlsym` plus
/// `@convention(c)` function-pointer casts (see `DynamicLibrary.swift`). Every door returns
/// a ``Verdict``: the value, or a ``Fault`` with a closed reason and the offending byte
/// span. Never throws for bad data — a `throws` here means the native library itself
/// couldn't load, and a precondition failure means a caller bug, never data.
///
/// Door names mirror the native ABI (`i32`, `f64`, `timestamp`, …) so the polyglot surface
/// reads identically across bindings. Swift-flavored fidelity: `UInt8`–`UInt64` are native
/// (no widening games), `Duration` carries the core's nanoseconds exactly, and
/// `DateComponents` keeps date/time-of-day digit-perfect; `Date` (the instant lingua
/// franca) is a `Double` of seconds, so sub-microsecond fidelity degrades toward the
/// window's edges — stated, not hidden.
public enum Cast {
    private typealias PlainFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?
    ) -> Int32
    private typealias NumericFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt, UnsafeRawPointer?, UnsafeMutableRawPointer?,
        UnsafeMutableRawPointer?
    ) -> Int32
    private typealias UnixFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt, UInt32, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?
    ) -> Int32

    // A class, deliberately: `loaded()` used to copy this 21-field struct out of the
    // `Result` on every single door call. A reference is one retain.
    private final class LoadedLibrary {
        let library: DynamicLibrary
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

        init(library: DynamicLibrary, bool: PlainFn, i8: NumericFn, i16: NumericFn, i32: NumericFn,
             i64: NumericFn, u8: NumericFn, u16: NumericFn, u32: NumericFn, u64: NumericFn,
             f32: NumericFn, f64: NumericFn, uuid: PlainFn, timestamp: PlainFn, unix: UnixFn,
             excelSerial: UnixFn, date: PlainFn, dateOrdered: UnixFn, dateTime: UnixFn,
             time: PlainFn, duration: PlainFn) {
            self.library = library
            self.bool = bool
            self.i8 = i8; self.i16 = i16; self.i32 = i32; self.i64 = i64
            self.u8 = u8; self.u16 = u16; self.u32 = u32; self.u64 = u64
            self.f32 = f32; self.f64 = f64
            self.uuid = uuid
            self.timestamp = timestamp; self.unix = unix; self.excelSerial = excelSerial
            self.date = date; self.dateOrdered = dateOrdered; self.dateTime = dateTime
            self.time = time; self.duration = duration
        }
    }

    // Swift initializes `static let`s lazily, exactly once, thread-safely; failures are
    // captured in a `Result` and re-surfaced as a normal `throws` from `loaded()`.
    private static let loadResult: Result<LoadedLibrary, Swift.Error> = Result { try load() }

    private static func load() throws -> LoadedLibrary {
        let library = try DynamicLibrary(path: try extractNativeLibrary())
        func plain(_ name: String) throws -> PlainFn {
            unsafeBitCast(try library.symbol(name), to: PlainFn.self)
        }
        func numeric(_ name: String) throws -> NumericFn {
            unsafeBitCast(try library.symbol(name), to: NumericFn.self)
        }
        return LoadedLibrary(
            library: library,
            bool: try plain("cast_bool"),
            i8: try numeric("cast_i8"), i16: try numeric("cast_i16"),
            i32: try numeric("cast_i32"), i64: try numeric("cast_i64"),
            u8: try numeric("cast_u8"), u16: try numeric("cast_u16"),
            u32: try numeric("cast_u32"), u64: try numeric("cast_u64"),
            f32: try numeric("cast_f32"), f64: try numeric("cast_f64"),
            uuid: try plain("cast_uuid"),
            timestamp: try plain("cast_timestamp"),
            unix: unsafeBitCast(try library.symbol("cast_unix"), to: UnixFn.self),
            excelSerial: unsafeBitCast(try library.symbol("cast_excel_serial"), to: UnixFn.self),
            date: try plain("cast_date"),
            dateOrdered: unsafeBitCast(try library.symbol("cast_date_ordered"), to: UnixFn.self),
            dateTime: unsafeBitCast(try library.symbol("cast_datetime"), to: UnixFn.self),
            time: try plain("cast_time"),
            duration: try plain("cast_duration"))
    }

    private static func loaded() throws -> LoadedLibrary {
        switch loadResult {
        case .success(let library): return library
        case .failure(let error): throw error
        }
    }

    /// Extracts this platform's bundled native library (an SPM resource) to a temp file —
    /// HyperUuid's approach, verbatim; the temp file is deliberately never removed.
    private static func extractNativeLibrary() throws -> String {
        let libNameURL = URL(fileURLWithPath: NativePlatform.libraryFileName)
        guard
            let resourceURL = Bundle.module.url(
                forResource: libNameURL.deletingPathExtension().lastPathComponent,
                withExtension: libNameURL.pathExtension,
                subdirectory: "NativeLibs/\(NativePlatform.rid)"
            )
        else {
            throw DynamicLibraryError.openFailed(
                path: "NativeLibs/\(NativePlatform.rid)/\(NativePlatform.libraryFileName)",
                reason: "resource not found (unsupported platform, or this package was built without a native library for it)"
            )
        }
        let data = try Data(contentsOf: resourceURL)
        let tempURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("libhypercast-\(UUID().uuidString)")
            .appendingPathExtension(libNameURL.pathExtension)
        try data.write(to: tempURL)
        return tempURL.path
    }

    private static func fault(_ code: Int32, _ raw: UnsafeRawBufferPointer) -> Fault {
        precondition(code != -1, "libhypercast reported a contract violation — a binding bug, please report it")
        guard let reason = CastFailure(rawValue: code) else {
            preconditionFailure("libhypercast returned unknown verdict code \(code)")
        }
        return Fault(
            reason: reason,
            offset: Int(raw.load(fromByteOffset: 0, as: UInt32.self)),
            length: Int(raw.load(fromByteOffset: 4, as: UInt32.self)))
    }

    // The scratch every door needs, on the stack: 16 bytes for the widest out-value (a
    // timestamp or a civil date-time — two UInt64s so the core's i64/u64 stores land
    // 8-aligned) and 8 for the fault span. These used to be three heap `[UInt8]` arrays
    // per call (four with the format), which was most of what a door cost; a tuple of
    // fixed-width integers has no heap existence at all.
    private typealias OutScratch = (UInt64, UInt64)
    private typealias FaultScratch = (UInt32, UInt32)

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
        var rawFormat: (UInt32, UInt32, UInt32) = (
            format.decimalSeparator.value, format.groupSeparator.value, format.styles.rawValue)
        var out: OutScratch = (0, 0)
        var faultOut: FaultScratch = (0, 0)
        let code = withUnsafeMutableBytes(of: &out) { outRaw in
            withUnsafeMutableBytes(of: &faultOut) { faultRaw in
                withUnsafeBytes(of: &rawFormat) { formatRaw in
                    fn(inputPointer(utf8), UInt(utf8.count), formatRaw.baseAddress,
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
                fn(inputPointer(utf8), UInt(utf8.count), discriminant, outRaw.baseAddress,
                   faultRaw.baseAddress)
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

    // MARK: - integers (the type's own range; grouping, parens, exponent, radix prefixes per format)

    /// Casts integer text to a signed 8-bit value under the declared format: the type's own
    /// range, declared grouping, accounting parentheses, non-negative exponent, and
    /// `0x`/`&H`/`0b` two's-complement radix prefixes.
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

    // MARK: - reals (finite only; separators, parens, exponent, percent per format)

    /// Casts real text to a `Float` under the declared format: finite values only, declared
    /// separators and grouping, parentheses, exponent, and trailing percent (`50%` is 0.5).
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
        plainDoor(try loaded().uuid, utf8) { raw in
            // uuid_t's tuple layout is the RFC byte order exactly.
            UUID(uuid: raw.load(as: uuid_t.self))
        }
    }

    // MARK: - temporals

    /// Casts an RFC 3339 instant — zone **mandatory** — to a `Date`, normalized to UTC.
    /// `Date` is a `Double` of seconds, so sub-microsecond fidelity degrades toward the
    /// 0001/9999 window edges; ``timeComponents``-style exactness isn't available for
    /// instants in Foundation, and that trade is stated rather than hidden.
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
        plainDoor(try loaded().timestamp, utf8, read: instant)
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
        unixDoor(try loaded().unix, utf8, precision.rawValue, read: instant)
    }

    /// Casts an Excel date serial under a caller-declared ``ExcelEpoch`` to a `Date`. The
    /// whole part counts days from the system's own day zero and the fraction is the time
    /// of day, so `45292.75` is 2024-01-01T18:00:00Z. A cell carries no zone and none is
    /// invented.
    ///
    /// The 1900 system contains a day that never existed: serial `60` is 1900-02-29, kept
    /// deliberately because Lotus 1-2-3 wrongly treated 1900 as a leap year and Excel
    /// copied the bug for file compatibility. It is `.malformed` here — the same verdict
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
        unixDoor(try loaded().excelSerial, utf8, epoch.rawValue, read: instant)
    }

    private static func instant(_ raw: UnsafeRawBufferPointer) -> Date {
        let seconds = raw.load(fromByteOffset: 0, as: Int64.self)
        let nanos = raw.load(fromByteOffset: 8, as: Int32.self)
        return Date(timeIntervalSince1970: Double(seconds) + Double(nanos) / 1_000_000_000)
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
        plainDoor(try loaded().date, utf8) { raw in
            DateComponents(
                year: Int(raw.load(fromByteOffset: 0, as: UInt16.self)),
                month: Int(raw.load(fromByteOffset: 2, as: UInt8.self)),
                day: Int(raw.load(fromByteOffset: 3, as: UInt8.self)))
        }
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
        unixDoor(try loaded().dateOrdered, utf8, order.rawValue) { raw in
            DateComponents(
                year: Int(raw.load(fromByteOffset: 0, as: UInt16.self)),
                month: Int(raw.load(fromByteOffset: 2, as: UInt8.self)),
                day: Int(raw.load(fromByteOffset: 3, as: UInt8.self)))
        }
    }

    /// Casts a zone-less civil date-time — the shape untrusted feeds actually send
    /// (`1/7/2026 3:04 PM`, `2026-01-07 15:04:05`) — under the caller-declared
    /// ``DateOrder`` to `DateComponents` (year through nanosecond, no zone). The date part
    /// follows ``date(_:order:)-swift.type.method``'s grammar; the optional time part (one
    /// space or `T` after the date) is 24-hour `h:mm[:ss[.f{1..9}]]` or 12-hour with an
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
        unixDoor(try loaded().dateTime, utf8, order.rawValue) { raw in
            let nanos = raw.load(fromByteOffset: 8, as: UInt64.self)
            let secondOfDay = nanos / 1_000_000_000
            return DateComponents(
                year: Int(raw.load(fromByteOffset: 0, as: UInt16.self)),
                month: Int(raw.load(fromByteOffset: 2, as: UInt8.self)),
                day: Int(raw.load(fromByteOffset: 3, as: UInt8.self)),
                hour: Int(secondOfDay / 3_600),
                minute: Int(secondOfDay % 3_600 / 60),
                second: Int(secondOfDay % 60),
                nanosecond: Int(nanos % 1_000_000_000))
        }
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
        plainDoor(try loaded().time, utf8) { raw in
            let nanosOfDay = raw.load(as: UInt64.self)
            let (secondOfDay, nano) = nanosOfDay.quotientAndRemainder(dividingBy: 1_000_000_000)
            let (hour, rest) = secondOfDay.quotientAndRemainder(dividingBy: 3_600)
            let (minute, second) = rest.quotientAndRemainder(dividingBy: 60)
            return DateComponents(
                hour: Int(hour), minute: Int(minute), second: Int(second), nanosecond: Int(nano))
        }
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
        plainDoor(try loaded().duration, utf8) { raw in
            let seconds = raw.load(fromByteOffset: 0, as: Int64.self)
            let nanos = raw.load(fromByteOffset: 8, as: Int32.self)
            return Duration.seconds(seconds) + .nanoseconds(Int64(nanos))
        }
    }

    /// Presents a verdict optionally: an ``CastFailure/empty`` fault becomes `nil` (Swift's
    /// absent), everything else flows through untouched.
    public static func optional<T>(_ verdict: Verdict<T>) -> Verdict<T>? {
        if case .fault(let fault) = verdict, fault.reason == .empty {
            return nil
        }
        return verdict
    }
}
