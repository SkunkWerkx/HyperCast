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

    private struct LoadedLibrary {
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
        let date: PlainFn
        // cast_date_ordered shares the unix ABI shape (ptr, len, u32, out, fault).
        let dateOrdered: UnixFn
        let time: PlainFn
        let duration: PlainFn
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
            date: try plain("cast_date"),
            dateOrdered: unsafeBitCast(try library.symbol("cast_date_ordered"), to: UnixFn.self),
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

    /// Runs a culture-insensitive door: `outBytes` of scratch for the value, 8 for the
    /// fault span, the verdict assembled from whichever the code says is live.
    private static func plainDoor<T>(
        _ door: (LoadedLibrary) -> PlainFn, _ utf8: [UInt8], outBytes: Int,
        read: (UnsafeRawBufferPointer) -> T
    ) throws -> Verdict<T> {
        let fn = door(try loaded())
        var out = [UInt8](repeating: 0, count: outBytes)
        var faultOut = [UInt8](repeating: 0, count: 8)
        let code = out.withUnsafeMutableBytes { outRaw in
            faultOut.withUnsafeMutableBytes { faultRaw in
                utf8.withUnsafeBufferPointer { input in
                    fn(input.baseAddress, UInt(utf8.count), outRaw.baseAddress, faultRaw.baseAddress)
                }
            }
        }
        if code == 0 {
            return .success(out.withUnsafeBytes(read))
        }
        return .fault(faultOut.withUnsafeBytes { fault(code, $0) })
    }

    private static func numericDoor<T>(
        _ door: (LoadedLibrary) -> NumericFn, _ utf8: [UInt8], _ format: NumFormat,
        outBytes: Int, read: (UnsafeRawBufferPointer) -> T
    ) throws -> Verdict<T> {
        let fn = door(try loaded())
        var rawFormat = [UInt8](repeating: 0, count: 12)
        rawFormat.withUnsafeMutableBytes { raw in
            raw.storeBytes(of: format.decimalSeparator.value, toByteOffset: 0, as: UInt32.self)
            raw.storeBytes(of: format.groupSeparator.value, toByteOffset: 4, as: UInt32.self)
            raw.storeBytes(of: format.styles.rawValue, toByteOffset: 8, as: UInt32.self)
        }
        var out = [UInt8](repeating: 0, count: outBytes)
        var faultOut = [UInt8](repeating: 0, count: 8)
        let code = out.withUnsafeMutableBytes { outRaw in
            faultOut.withUnsafeMutableBytes { faultRaw in
                utf8.withUnsafeBufferPointer { input in
                    rawFormat.withUnsafeBytes { formatRaw in
                        fn(input.baseAddress, UInt(utf8.count), formatRaw.baseAddress,
                           outRaw.baseAddress, faultRaw.baseAddress)
                    }
                }
            }
        }
        if code == 0 {
            return .success(out.withUnsafeBytes(read))
        }
        return .fault(faultOut.withUnsafeBytes { fault(code, $0) })
    }

    // MARK: - boolean

    /// Casts boolean text: `true`/`false` plus the conventions untrusted sources actually
    /// send (`t/f`, `yes/no`, `y/n`, `1/0`, `on/off`, `enabled/disabled`,
    /// `active/inactive`, `checked/unchecked`, `in/out`), ASCII case-insensitive.
    public static func bool(_ text: String) throws -> Verdict<Bool> {
        try bool(Array(text.utf8))
    }

    /// See ``bool(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func bool(_ utf8: [UInt8]) throws -> Verdict<Bool> {
        try plainDoor({ $0.bool }, utf8, outBytes: 1) { $0.load(as: UInt8.self) != 0 }
    }

    // MARK: - integers (the type's own range; grouping, parens, exponent, radix prefixes per format)

    public static func i8(_ text: String, format: NumFormat) throws -> Verdict<Int8> {
        try i8(Array(text.utf8), format: format)
    }

    public static func i8(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<Int8> {
        try numericDoor({ $0.i8 }, utf8, format, outBytes: 1) { $0.load(as: Int8.self) }
    }

    public static func i16(_ text: String, format: NumFormat) throws -> Verdict<Int16> {
        try i16(Array(text.utf8), format: format)
    }

    public static func i16(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<Int16> {
        try numericDoor({ $0.i16 }, utf8, format, outBytes: 2) { $0.load(as: Int16.self) }
    }

    public static func i32(_ text: String, format: NumFormat) throws -> Verdict<Int32> {
        try i32(Array(text.utf8), format: format)
    }

    public static func i32(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<Int32> {
        try numericDoor({ $0.i32 }, utf8, format, outBytes: 4) { $0.load(as: Int32.self) }
    }

    public static func i64(_ text: String, format: NumFormat) throws -> Verdict<Int64> {
        try i64(Array(text.utf8), format: format)
    }

    public static func i64(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<Int64> {
        try numericDoor({ $0.i64 }, utf8, format, outBytes: 8) { $0.load(as: Int64.self) }
    }

    public static func u8(_ text: String, format: NumFormat) throws -> Verdict<UInt8> {
        try u8(Array(text.utf8), format: format)
    }

    public static func u8(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<UInt8> {
        try numericDoor({ $0.u8 }, utf8, format, outBytes: 1) { $0.load(as: UInt8.self) }
    }

    public static func u16(_ text: String, format: NumFormat) throws -> Verdict<UInt16> {
        try u16(Array(text.utf8), format: format)
    }

    public static func u16(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<UInt16> {
        try numericDoor({ $0.u16 }, utf8, format, outBytes: 2) { $0.load(as: UInt16.self) }
    }

    public static func u32(_ text: String, format: NumFormat) throws -> Verdict<UInt32> {
        try u32(Array(text.utf8), format: format)
    }

    public static func u32(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<UInt32> {
        try numericDoor({ $0.u32 }, utf8, format, outBytes: 4) { $0.load(as: UInt32.self) }
    }

    public static func u64(_ text: String, format: NumFormat) throws -> Verdict<UInt64> {
        try u64(Array(text.utf8), format: format)
    }

    public static func u64(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<UInt64> {
        try numericDoor({ $0.u64 }, utf8, format, outBytes: 8) { $0.load(as: UInt64.self) }
    }

    // MARK: - reals (finite only; separators, parens, exponent, percent per format)

    public static func f32(_ text: String, format: NumFormat) throws -> Verdict<Float> {
        try f32(Array(text.utf8), format: format)
    }

    public static func f32(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<Float> {
        try numericDoor({ $0.f32 }, utf8, format, outBytes: 4) { $0.load(as: Float.self) }
    }

    public static func f64(_ text: String, format: NumFormat) throws -> Verdict<Double> {
        try f64(Array(text.utf8), format: format)
    }

    public static func f64(_ utf8: [UInt8], format: NumFormat) throws -> Verdict<Double> {
        try numericDoor({ $0.f64 }, utf8, format, outBytes: 8) { $0.load(as: Double.self) }
    }

    // MARK: - uuid

    /// Casts UUID text — all five .NET `Guid` formats (D/N/B/P/X) plus
    /// `urn:uuid:`/`GUID:`/`UUID:` prefixes — to a Foundation `UUID`.
    public static func uuid(_ text: String) throws -> Verdict<UUID> {
        try uuid(Array(text.utf8))
    }

    /// See ``uuid(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func uuid(_ utf8: [UInt8]) throws -> Verdict<UUID> {
        try plainDoor({ $0.uuid }, utf8, outBytes: 16) { raw in
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
        try timestamp(Array(text.utf8))
    }

    /// See ``timestamp(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func timestamp(_ utf8: [UInt8]) throws -> Verdict<Date> {
        try plainDoor({ $0.timestamp }, utf8, outBytes: 16, read: instant)
    }

    /// Casts an integer Unix-epoch value under a caller-declared unit to a `Date`.
    public static func unix(_ text: String, precision: UnixPrecision) throws -> Verdict<Date> {
        try unix(Array(text.utf8), precision: precision)
    }

    /// See ``unix(_:precision:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func unix(_ utf8: [UInt8], precision: UnixPrecision) throws -> Verdict<Date> {
        let fn = try loaded().unix
        var out = [UInt8](repeating: 0, count: 16)
        var faultOut = [UInt8](repeating: 0, count: 8)
        let code = out.withUnsafeMutableBytes { outRaw in
            faultOut.withUnsafeMutableBytes { faultRaw in
                utf8.withUnsafeBufferPointer { input in
                    fn(input.baseAddress, UInt(utf8.count), precision.rawValue,
                       outRaw.baseAddress, faultRaw.baseAddress)
                }
            }
        }
        if code == 0 {
            return .success(out.withUnsafeBytes(instant))
        }
        return .fault(faultOut.withUnsafeBytes { fault(code, $0) })
    }

    private static func instant(_ raw: UnsafeRawBufferPointer) -> Date {
        let seconds = raw.load(fromByteOffset: 0, as: Int64.self)
        let nanos = raw.load(fromByteOffset: 8, as: Int32.self)
        return Date(timeIntervalSince1970: Double(seconds) + Double(nanos) / 1_000_000_000)
    }

    /// Casts a strict ISO 8601 `yyyy-MM-dd` calendar date to `DateComponents` (year, month,
    /// day) — digit-perfect, no calendar or zone attached, exactly what the core parsed.
    public static func date(_ text: String) throws -> Verdict<DateComponents> {
        try date(Array(text.utf8))
    }

    /// See ``date(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func date(_ utf8: [UInt8]) throws -> Verdict<DateComponents> {
        try plainDoor({ $0.date }, utf8, outBytes: 4) { raw in
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
        try date(Array(text.utf8), order: order)
    }

    /// See ``date(_:order:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func date(_ utf8: [UInt8], order: DateOrder) throws -> Verdict<DateComponents> {
        let fn = try loaded().dateOrdered
        var out = [UInt8](repeating: 0, count: 4)
        var faultOut = [UInt8](repeating: 0, count: 8)
        let code = out.withUnsafeMutableBytes { outRaw in
            faultOut.withUnsafeMutableBytes { faultRaw in
                utf8.withUnsafeBufferPointer { input in
                    fn(input.baseAddress, UInt(utf8.count), order.rawValue,
                       outRaw.baseAddress, faultRaw.baseAddress)
                }
            }
        }
        if code == 0 {
            return .success(out.withUnsafeBytes { raw in
                DateComponents(
                    year: Int(raw.load(fromByteOffset: 0, as: UInt16.self)),
                    month: Int(raw.load(fromByteOffset: 2, as: UInt8.self)),
                    day: Int(raw.load(fromByteOffset: 3, as: UInt8.self)))
            })
        }
        return .fault(faultOut.withUnsafeBytes { fault(code, $0) })
    }

    /// Casts an ISO 24-hour time-of-day to `DateComponents` (hour, minute, second,
    /// nanosecond) — full nanosecond fidelity.
    public static func time(_ text: String) throws -> Verdict<DateComponents> {
        try time(Array(text.utf8))
    }

    /// See ``time(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func time(_ utf8: [UInt8]) throws -> Verdict<DateComponents> {
        try plainDoor({ $0.time }, utf8, outBytes: 8) { raw in
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
        try duration(Array(text.utf8))
    }

    /// See ``duration(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func duration(_ utf8: [UInt8]) throws -> Verdict<Duration> {
        try plainDoor({ $0.duration }, utf8, outBytes: 16) { raw in
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
