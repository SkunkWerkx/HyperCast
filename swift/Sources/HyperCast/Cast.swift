import Foundation
import HyperCastCore

/// Allocation-lean scalar casts — booleans, numerics (integers, reals, exact decimals),
/// UUIDs, temporals — calling directly into the native `hypercast` core through the
/// detailed exports. The core is a static library linked into the
/// executable on every platform this package builds for (`HyperCastCore`, a SwiftPM binary
/// target), so there is nothing to find, open or deploy at run time. Every door returns a
/// ``Verdict``: the value, or a ``Fault`` with a closed reason, the offending byte span and
/// the ``FaultDetail`` of what failed there; a
/// checked door, judged by ``Constraints``, a ``CheckedVerdict``, whose rejection is that
/// ``Fault`` or a ``Violation``. No door throws — bad data is a ``Fault`` or a ``Violation`` —
/// and a precondition failure means a caller bug, never data.
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
    typealias PlainFn =
        @convention(c) (
            UnsafePointer<UInt8>?, UInt, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?
        ) -> Int32
    typealias NumericFn =
        @convention(c) (
            UnsafePointer<UInt8>?, UInt, UnsafeRawPointer?, UnsafeMutableRawPointer?,
            UnsafeMutableRawPointer?
        ) -> Int32
    typealias UnixFn =
        @convention(c) (
            UnsafePointer<UInt8>?, UInt, UInt32, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?
        ) -> Int32
    // cast_datetime: the unix shape with the declared day periods (null = English AM/PM
    // only) after the order.
    typealias DateTimeFn =
        @convention(c) (
            UnsafePointer<UInt8>?, UInt, UInt32, UnsafeRawPointer?, UnsafeMutableRawPointer?,
            UnsafeMutableRawPointer?
        ) -> Int32
    typealias VersionFn = @convention(c) () -> UInt32
    // cast_checked and cast_checked_from_f64: the declaration, its enum list, the input or the
    // double, then out, fault and check.
    typealias CheckedFn =
        @convention(c) (
            UInt32, UInt32, UnsafeRawPointer?, UnsafeRawPointer?, UnsafePointer<UInt8>?, UInt,
            UnsafePointer<UInt8>?, UInt, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?,
            UnsafeMutableRawPointer?
        ) -> Int32
    typealias CheckedDoubleFn =
        @convention(c) (
            UInt32, Double, UnsafeRawPointer?, UnsafePointer<UInt8>?, UInt, UnsafeMutableRawPointer?,
            UnsafeMutableRawPointer?, UnsafeMutableRawPointer?
        ) -> Int32
    // The typed doors read a double the caller holds instead of text.
    typealias TypedFn =
        @convention(c) (
            Double, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?
        ) -> Int32
    typealias TypedDeclaredFn =
        @convention(c) (
            Double, UInt32, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?
        ) -> Int32

    // The 1.0 exports as one table. The doors below reach the core through the detailed exports
    // (`cast_detailed` and its three siblings), which carry the fault's detail and the 1.1
    // grouping policy; this table is the exports a 1.0 core had, which read Western and name no
    // detail, kept so the suite can hold the two to each other (the doors agree with them on a
    // Western format) and so the export check finds every symbol this package links. A class: a
    // reference is one retain where a struct of function pointers would be copied. Immutable
    // once built, and C function pointers carry no state of their own.
    final class OneDotZeroExports: Sendable {
        let bool: PlainFn
        let char: PlainFn
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
        // cast_date_ordered shares the unix ABI shape (ptr, len, u32, out, fault).
        let dateOrdered: UnixFn
        let dateTime: DateTimeFn
        let time: PlainFn
        let duration: PlainFn
        let decimalFromF64: TypedFn
        let excelSerialFromF64: TypedDeclaredFn
        let excelTime: TypedFn
        let excelDuration: TypedFn
        let version: VersionFn
        let text: PlainFn
        let checked: CheckedFn
        let checkedFromF64: CheckedDoubleFn

        init(
            bool: PlainFn, char: PlainFn,
            i8: NumericFn, i16: NumericFn, i32: NumericFn, i64: NumericFn,
            u8: NumericFn, u16: NumericFn, u32: NumericFn, u64: NumericFn,
            f32: NumericFn, f64: NumericFn, decimal: NumericFn, uuid: PlainFn, timestamp: PlainFn,
            unix: UnixFn, excelSerial: UnixFn, date: PlainFn, dateOrdered: UnixFn, dateTime: DateTimeFn,
            time: PlainFn, duration: PlainFn,
            decimalFromF64: TypedFn, excelSerialFromF64: TypedDeclaredFn, excelTime: TypedFn,
            excelDuration: TypedFn, version: VersionFn, text: PlainFn, checked: CheckedFn,
            checkedFromF64: CheckedDoubleFn
        ) {
            self.bool = bool
            self.char = char
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
            self.text = text; self.checked = checked; self.checkedFromF64 = checkedFromF64
        }
    }

    // Linked in: the C declarations are the table, and there is nothing to find or open.
    // The header types each door's `out`, `format`, `periods` and `fault` (HyperCastDecimal,
    // HyperCastRawFault, ...), while the doors here fill and read them as raw bytes through
    // the shapes above, so each entry passes its raw pointers on as the typed ones, which
    // are the same pointers in the C ABI. Each is a C function of its own that calls the door.
    static let oneDotZero = OneDotZeroExports(
        bool: { cast_bool($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3))) },
        char: { cast_char($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3))) },
        i8: { cast_i8($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        i16: { cast_i16($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        i32: { cast_i32($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        i64: { cast_i64($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        u8: { cast_u8($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        u16: { cast_u16($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        u32: { cast_u32($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        u64: { cast_u64($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        f32: { cast_f32($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        f64: { cast_f64($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        decimal: { cast_decimal($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        uuid: { cast_uuid($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3))) },
        timestamp: { cast_timestamp($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3))) },
        unix: { cast_unix($0, $1, $2, .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        excelSerial: { cast_excel_serial($0, $1, $2, .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        date: { cast_date($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3))) },
        dateOrdered: { cast_date_ordered($0, $1, $2, .init(OpaquePointer($3)), .init(OpaquePointer($4))) },
        dateTime: {
            cast_datetime($0, $1, $2, .init(OpaquePointer($3)), .init(OpaquePointer($4)), .init(OpaquePointer($5)))
        },
        time: { cast_time($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3))) },
        duration: { cast_duration($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3))) },
        decimalFromF64: { cast_decimal_from_f64($0, .init(OpaquePointer($1)), .init(OpaquePointer($2))) },
        excelSerialFromF64: { cast_excel_serial_from_f64($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3))) },
        excelTime: { cast_excel_time($0, .init(OpaquePointer($1)), .init(OpaquePointer($2))) },
        excelDuration: { cast_excel_duration($0, .init(OpaquePointer($1)), .init(OpaquePointer($2))) },
        version: { hypercast_version() },
        text: { cast_text($0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3))) },
        checked: {
            cast_checked(
                $0, $1, .init(OpaquePointer($2)), .init(OpaquePointer($3)), $4, $5, $6, $7, $8,
                .init(OpaquePointer($9)), .init(OpaquePointer($10)))
        },
        checkedFromF64: {
            cast_checked_from_f64(
                $0, $1, .init(OpaquePointer($2)), $3, $4, $5, .init(OpaquePointer($6)), .init(OpaquePointer($7)))
        })

    /// Whether the native core is usable. Always `true`: the core is linked into the
    /// executable on every platform this package builds for, and a platform with no
    /// prebuilt core fails to compile rather than at run time. It keeps the probe every other
    /// binding has under the same name, where the answer can be `false`.
    public static var isAvailable: Bool { true }

    // The door codes of the detailed exports: `Door`'s numbering in rust/src/door.rs, which
    // every binding dispatching through them restates. A door that declares something (a Unix
    // precision, a date order, an Excel epoch) takes it as the `param` beside the code; every
    // other door takes 0.
    enum DoorCode {
        static let bool: UInt32 = 1
        static let i8: UInt32 = 2
        static let i16: UInt32 = 3
        static let i32: UInt32 = 4
        static let i64: UInt32 = 5
        static let u8: UInt32 = 6
        static let u16: UInt32 = 7
        static let u32: UInt32 = 8
        static let u64: UInt32 = 9
        static let f32: UInt32 = 10
        static let f64: UInt32 = 11
        static let uuid: UInt32 = 12
        static let timestamp: UInt32 = 13
        static let unix: UInt32 = 14
        static let date: UInt32 = 15
        static let time: UInt32 = 16
        static let duration: UInt32 = 17
        static let text: UInt32 = 18
        static let decimal: UInt32 = 19
        static let dateOrdered: UInt32 = 20
        static let dateTime: UInt32 = 21
        static let excelSerial: UInt32 = 22
        static let char: UInt32 = 23
    }

    // The 1.0 fault: the span alone.
    static func fault(_ code: Int32, _ raw: UnsafeRawBufferPointer) -> Fault {
        precondition(code != -1, "libhypercast reported a contract violation — a binding bug, please report it")
        return Interop.fault(
            code: code,
            offset: raw.load(fromByteOffset: 0, as: UInt32.self),
            length: raw.load(fromByteOffset: 4, as: UInt32.self))
    }

    // The detailed fault: the span and the detail code, behind the capacity word the caller wrote.
    static func detailedFault(_ code: Int32, _ raw: UnsafeRawBufferPointer) -> Fault {
        precondition(code != -1, "libhypercast reported a contract violation — a binding bug, please report it")
        return Interop.fault(code: code, faultDetail: raw)
    }

    // The scratch every door needs, on the stack: 16 bytes for the widest out-value (a
    // timestamp or a civil date-time — two UInt64s so the core's i64/u64 stores land
    // 8-aligned) and 16 for the detailed fault (its capacity word, the span and the detail).
    // A tuple of fixed-width integers has no heap
    // existence at all, where three heap `[UInt8]` arrays per call (four with the format)
    // would be most of what a door costs. Internal, not private, so the test
    // suite can pin both sizes to the ones rust/src/abi.rs and ffi.rs pin.
    typealias OutScratch = (UInt64, UInt64)
    typealias FaultScratch = (UInt32, UInt32)
    typealias DetailScratch = Interop.RawFaultDetail
    // The native `RawNumFormat` and `RawDayPeriods`, under the names the test suite pins
    // their sizes by. A format's layout is computed when it is declared; a door copies it
    // onto the stack and hands the core a pointer, which it reads in place.
    typealias RawNumFormat = Interop.RawNumFormat
    typealias RawDayPeriods = Interop.RawDayPeriods
    typealias RawDateOptions = Interop.RawDateOptions

    static func inputPointer(_ utf8: UnsafeRawBufferPointer) -> UnsafePointer<UInt8>? {
        // An empty buffer may carry a nil base address; the ABI never dereferences at len 0.
        utf8.baseAddress?.assumingMemoryBound(to: UInt8.self)
    }

    /// The general call into `cast_detailed`: every declaration optional, the core's verdict
    /// code returned as it came (−1, a refused declaration, included). A declaration a door
    /// ignores is passed null and never read.
    static func callDetailed(
        door: UInt32, param: UInt32, _ utf8: UnsafeRawBufferPointer,
        format: NumFormat? = nil, periods: DateFormat? = nil, options: DateOptions? = nil,
        out: inout OutScratch, fault: inout DetailScratch
    ) -> Int32 {
        var rawFormat = format?.raw ?? Interop.zeroRawFormat
        var rawPeriods = periods?.raw ?? Interop.zeroRawDayPeriods
        var rawOptions: RawDateOptions = options?.raw ?? (0, 0, 0)
        let declaresPeriods = periods?.declaresPeriods ?? false
        let vocabulary = options?.months?.packed ?? []
        return withUnsafeMutableBytes(of: &out) { outRaw in
            withUnsafeMutableBytes(of: &fault) { faultRaw in
                withUnsafeBytes(of: &rawFormat) { formatRaw in
                    withUnsafeBytes(of: &rawPeriods) { periodsRaw in
                        withUnsafeBytes(of: &rawOptions) { optionsRaw in
                            vocabulary.withUnsafeBufferPointer { vocab in
                                cast_detailed(
                                    door, param,
                                    format == nil ? nil : .init(OpaquePointer(formatRaw.baseAddress)),
                                    declaresPeriods ? .init(OpaquePointer(periodsRaw.baseAddress)) : nil,
                                    options == nil ? nil : .init(OpaquePointer(optionsRaw.baseAddress)),
                                    vocab.baseAddress, UInt(vocab.count), inputPointer(utf8), UInt(utf8.count),
                                    outRaw.baseAddress, .init(OpaquePointer(faultRaw.baseAddress)))
                            }
                        }
                    }
                }
            }
        }
    }

    /// Whether the core accepts a packed month vocabulary: a probe call at the date-order door
    /// with nothing else declared. −1 is a vocabulary the core refuses; any other code, a read
    /// or a fault of the probe's own text, is an accepted one.
    static func vocabularyIsAccepted(_ packed: [UInt8]) -> Bool {
        var out: OutScratch = (0, 0)
        var fault = Interop.freshRawFaultDetail
        let probe = Array("2026-01-07".utf8)
        let code = probe.withUnsafeBytes { text in
            packed.withUnsafeBufferPointer { vocab in
                withUnsafeMutableBytes(of: &out) { outRaw in
                    withUnsafeMutableBytes(of: &fault) { faultRaw in
                        cast_detailed(
                            DoorCode.dateOrdered, DateOrder.yearMonthDay.rawValue, nil, nil, nil, vocab.baseAddress,
                            UInt(vocab.count), inputPointer(text), UInt(text.count), outRaw.baseAddress,
                            .init(OpaquePointer(faultRaw.baseAddress)))
                    }
                }
            }
        }
        return code != -1
    }

    /// Runs a culture-insensitive door over the caller's bytes, the verdict assembled from
    /// whichever half of the scratch the code says is live.
    private static func plainDoor<T>(
        _ door: UInt32, _ utf8: UnsafeRawBufferPointer, read: (UnsafeRawBufferPointer) -> T
    ) -> Verdict<T> {
        var out: OutScratch = (0, 0)
        var faultOut = Interop.freshRawFaultDetail
        let code = withUnsafeMutableBytes(of: &out) { outRaw in
            withUnsafeMutableBytes(of: &faultOut) { faultRaw in
                cast_detailed(
                    door, 0, nil, nil, nil, nil, 0, inputPointer(utf8), UInt(utf8.count), outRaw.baseAddress,
                    .init(OpaquePointer(faultRaw.baseAddress)))
            }
        }
        if code == 0 {
            return .success(withUnsafeBytes(of: &out, read))
        }
        return .fault(withUnsafeBytes(of: &faultOut) { detailedFault(code, $0) })
    }

    private static func numericDoor<T>(
        _ door: UInt32, _ utf8: UnsafeRawBufferPointer, _ format: NumFormat,
        read: (UnsafeRawBufferPointer) -> T
    ) -> Verdict<T> {
        var rawFormat = format.raw
        var out: OutScratch = (0, 0)
        var faultOut = Interop.freshRawFaultDetail
        let code = withUnsafeMutableBytes(of: &out) { outRaw in
            withUnsafeMutableBytes(of: &faultOut) { faultRaw in
                withUnsafeBytes(of: &rawFormat) { formatRaw in
                    cast_detailed(
                        door, 0, .init(OpaquePointer(formatRaw.baseAddress)), nil, nil, nil, 0, inputPointer(utf8),
                        UInt(utf8.count), outRaw.baseAddress, .init(OpaquePointer(faultRaw.baseAddress)))
                }
            }
        }
        if code == 0 {
            return .success(withUnsafeBytes(of: &out, read))
        }
        return .fault(withUnsafeBytes(of: &faultOut) { detailedFault(code, $0) })
    }

    /// The (door, param) shape — a declared precision, epoch or field order — and the date
    /// doors' declarations: day periods and date options (with their month names) where the
    /// door reads them.
    static func parameterizedDoor<T>(
        _ door: UInt32, _ param: UInt32, _ utf8: UnsafeRawBufferPointer,
        periods: DateFormat? = nil, options: DateOptions? = nil,
        read: (UnsafeRawBufferPointer) -> T
    ) -> Verdict<T> {
        var out: OutScratch = (0, 0)
        var faultOut = Interop.freshRawFaultDetail
        let code = callDetailed(
            door: door, param: param, utf8, periods: periods, options: options, out: &out, fault: &faultOut)
        if code == 0 {
            return .success(withUnsafeBytes(of: &out, read))
        }
        return .fault(withUnsafeBytes(of: &faultOut) { detailedFault(code, $0) })
    }

    /// The (door, param) shape without a declaration: a declared precision, epoch or field
    /// order.
    private static func unixDoor<T>(
        _ door: UInt32, _ utf8: UnsafeRawBufferPointer, _ param: UInt32,
        read: (UnsafeRawBufferPointer) -> T
    ) -> Verdict<T> {
        var out: OutScratch = (0, 0)
        var faultOut = Interop.freshRawFaultDetail
        let code = withUnsafeMutableBytes(of: &out) { outRaw in
            withUnsafeMutableBytes(of: &faultOut) { faultRaw in
                cast_detailed(
                    door, param, nil, nil, nil, nil, 0, inputPointer(utf8), UInt(utf8.count), outRaw.baseAddress,
                    .init(OpaquePointer(faultRaw.baseAddress)))
            }
        }
        if code == 0 {
            return .success(withUnsafeBytes(of: &out, read))
        }
        return .fault(withUnsafeBytes(of: &faultOut) { detailedFault(code, $0) })
    }

    /// The date-time shape: a declared order, then the declared day periods — the format's
    /// layout on the stack, or null when nothing is declared, which is exactly the
    /// order-only door's call — and the date options, when the caller passes them.
    static func dateTimeDoor<T>(
        _ utf8: UnsafeRawBufferPointer, _ order: DateOrder, _ format: DateFormat?, _ options: DateOptions?,
        read: (UnsafeRawBufferPointer) -> T
    ) -> Verdict<T> {
        parameterizedDoor(DoorCode.dateTime, order.rawValue, utf8, periods: format, options: options, read: read)
    }

    /// The typed shape: a double instead of text, then the declared epoch when the door
    /// takes one. There is no text for a fault's span to index, so the core leaves it empty.
    private static func typedDoor<T>(
        _ door: UInt32, _ param: UInt32, _ value: Double, read: (UnsafeRawBufferPointer) -> T
    ) -> Verdict<T> {
        var out: OutScratch = (0, 0)
        var faultOut = Interop.freshRawFaultDetail
        let code = withUnsafeMutableBytes(of: &out) { outRaw in
            withUnsafeMutableBytes(of: &faultOut) { faultRaw in
                cast_detailed_from_f64(
                    door, param, value, outRaw.baseAddress, .init(OpaquePointer(faultRaw.baseAddress)))
            }
        }
        if code == 0 {
            return .success(withUnsafeBytes(of: &out, read))
        }
        return .fault(withUnsafeBytes(of: &faultOut) { detailedFault(code, $0) })
    }

    /// A native Swift `String` already stores contiguous UTF-8, so `withUTF8` hands the
    /// door a view of the string's own bytes — no `Array(text.utf8)` copy, which used to
    /// be one heap allocation and a memcpy on every `String` door.
    static func withUTF8<T>(_ text: String, _ body: (UnsafeRawBufferPointer) -> T) -> T {
        var text = text
        return text.withUTF8 { body(UnsafeRawBufferPointer($0)) }
    }

    // MARK: - boolean

    /// Casts boolean text: `true`/`false` plus the conventions untrusted sources actually
    /// send (`t/f`, `yes/no`, `y/n`, `1/0`, `on/off`, `enabled/disabled`,
    /// `active/inactive`, `checked/unchecked`, `in/out`), ASCII case-insensitive.
    public static func bool(_ text: String) -> Verdict<Bool> {
        withUTF8(text) { bool($0) }
    }

    /// See ``bool(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func bool(_ utf8: [UInt8]) -> Verdict<Bool> {
        utf8.withUnsafeBytes { bool($0) }
    }

    /// See ``bool(_:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func bool(_ utf8: UnsafeRawBufferPointer) -> Verdict<Bool> {
        plainDoor(DoorCode.bool, utf8) { $0.load(as: UInt8.self) != 0 }
    }

    // MARK: - char

    /// Casts char text to one `Unicode.Scalar`, verbatim or as a declared code point. An
    /// input that is exactly one scalar is that scalar, checked before any trimming — so
    /// `" "` is a space and `"6"` the digit six. Otherwise white space (Unicode `White_Space`,
    /// U+00A0 included) is trimmed and the text must be exactly one code-point spelling, ASCII
    /// case-insensitive on the prefix: decimal `65`, `U+0041`, `0x41`, `&H41`, or an HTML
    /// numeric entity `&#65;` / `&#x41;` (the `;` required). A spelling past `U+10FFFF` or
    /// naming a surrogate is `.outOfRange`. Every scalar fits `Unicode.Scalar`, so nothing
    /// narrows here.
    public static func char(_ text: String) -> Verdict<Unicode.Scalar> {
        withUTF8(text) { char($0) }
    }

    /// See ``char(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func char(_ utf8: [UInt8]) -> Verdict<Unicode.Scalar> {
        utf8.withUnsafeBytes { char($0) }
    }

    /// See ``char(_:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func char(_ utf8: UnsafeRawBufferPointer) -> Verdict<Unicode.Scalar> {
        plainDoor(DoorCode.char, utf8, read: Interop.scalar)
    }

    // MARK: - integers (the type's own range; grouping, parens, exponent, radix prefixes, currency per format)

    /// Casts integer text to a signed 8-bit value under the declared format: the type's own
    /// range, declared grouping, accounting parentheses, non-negative exponent,
    /// `0x`/`&H`/`0b` two's-complement radix prefixes, and the declared currency symbol.
    public static func i8(_ text: String, format: NumFormat) -> Verdict<Int8> {
        withUTF8(text) { i8($0, format: format) }
    }

    /// See ``i8(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i8(_ utf8: [UInt8], format: NumFormat) -> Verdict<Int8> {
        utf8.withUnsafeBytes { i8($0, format: format) }
    }

    /// See ``i8(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func i8(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Int8> {
        numericDoor(DoorCode.i8, utf8, format) { $0.load(as: Int8.self) }
    }

    /// Casts integer text to a signed 16-bit value. Notation rules as
    /// ``i8(_:format:)-swift.type.method``.
    public static func i16(_ text: String, format: NumFormat) -> Verdict<Int16> {
        withUTF8(text) { i16($0, format: format) }
    }

    /// See ``i16(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i16(_ utf8: [UInt8], format: NumFormat) -> Verdict<Int16> {
        utf8.withUnsafeBytes { i16($0, format: format) }
    }

    /// See ``i16(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func i16(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Int16> {
        numericDoor(DoorCode.i16, utf8, format) { $0.load(as: Int16.self) }
    }

    /// Casts integer text to a signed 32-bit value. Notation rules as
    /// ``i8(_:format:)-swift.type.method``.
    public static func i32(_ text: String, format: NumFormat) -> Verdict<Int32> {
        withUTF8(text) { i32($0, format: format) }
    }

    /// See ``i32(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i32(_ utf8: [UInt8], format: NumFormat) -> Verdict<Int32> {
        utf8.withUnsafeBytes { i32($0, format: format) }
    }

    /// See ``i32(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func i32(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Int32> {
        numericDoor(DoorCode.i32, utf8, format) { $0.load(as: Int32.self) }
    }

    /// Casts integer text to a signed 64-bit value. Notation rules as
    /// ``i8(_:format:)-swift.type.method``.
    public static func i64(_ text: String, format: NumFormat) -> Verdict<Int64> {
        withUTF8(text) { i64($0, format: format) }
    }

    /// See ``i64(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func i64(_ utf8: [UInt8], format: NumFormat) -> Verdict<Int64> {
        utf8.withUnsafeBytes { i64($0, format: format) }
    }

    /// See ``i64(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func i64(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Int64> {
        numericDoor(DoorCode.i64, utf8, format) { $0.load(as: Int64.self) }
    }

    /// Casts integer text to an unsigned 8-bit value. Notation rules as
    /// ``i8(_:format:)-swift.type.method``.
    public static func u8(_ text: String, format: NumFormat) -> Verdict<UInt8> {
        withUTF8(text) { u8($0, format: format) }
    }

    /// See ``u8(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u8(_ utf8: [UInt8], format: NumFormat) -> Verdict<UInt8> {
        utf8.withUnsafeBytes { u8($0, format: format) }
    }

    /// See ``u8(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func u8(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<UInt8> {
        numericDoor(DoorCode.u8, utf8, format) { $0.load(as: UInt8.self) }
    }

    /// Casts integer text to an unsigned 16-bit value. Notation rules as
    /// ``i8(_:format:)-swift.type.method``.
    public static func u16(_ text: String, format: NumFormat) -> Verdict<UInt16> {
        withUTF8(text) { u16($0, format: format) }
    }

    /// See ``u16(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u16(_ utf8: [UInt8], format: NumFormat) -> Verdict<UInt16> {
        utf8.withUnsafeBytes { u16($0, format: format) }
    }

    /// See ``u16(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func u16(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<UInt16> {
        numericDoor(DoorCode.u16, utf8, format) { $0.load(as: UInt16.self) }
    }

    /// Casts integer text to an unsigned 32-bit value. Notation rules as
    /// ``i8(_:format:)-swift.type.method``.
    public static func u32(_ text: String, format: NumFormat) -> Verdict<UInt32> {
        withUTF8(text) { u32($0, format: format) }
    }

    /// See ``u32(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u32(_ utf8: [UInt8], format: NumFormat) -> Verdict<UInt32> {
        utf8.withUnsafeBytes { u32($0, format: format) }
    }

    /// See ``u32(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func u32(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<UInt32> {
        numericDoor(DoorCode.u32, utf8, format) { $0.load(as: UInt32.self) }
    }

    /// Casts integer text to an unsigned 64-bit value — natively unsigned, no widening games.
    /// Notation rules as ``i8(_:format:)-swift.type.method``.
    public static func u64(_ text: String, format: NumFormat) -> Verdict<UInt64> {
        withUTF8(text) { u64($0, format: format) }
    }

    /// See ``u64(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func u64(_ utf8: [UInt8], format: NumFormat) -> Verdict<UInt64> {
        utf8.withUnsafeBytes { u64($0, format: format) }
    }

    /// See ``u64(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func u64(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<UInt64> {
        numericDoor(DoorCode.u64, utf8, format) { $0.load(as: UInt64.self) }
    }

    // MARK: - reals (finite only; separators, parens, exponent, percent, currency per format)

    /// Casts real text to a `Float` under the declared format: finite values only, declared
    /// separators and grouping, parentheses, exponent, trailing percent (`50%` is 0.5), and
    /// the declared currency symbol.
    public static func f32(_ text: String, format: NumFormat) -> Verdict<Float> {
        withUTF8(text) { f32($0, format: format) }
    }

    /// See ``f32(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func f32(_ utf8: [UInt8], format: NumFormat) -> Verdict<Float> {
        utf8.withUnsafeBytes { f32($0, format: format) }
    }

    /// See ``f32(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func f32(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Float> {
        numericDoor(DoorCode.f32, utf8, format) { $0.load(as: Float.self) }
    }

    /// Casts real text to a `Double`. Notation rules as ``f32(_:format:)-swift.type.method``.
    public static func f64(_ text: String, format: NumFormat) -> Verdict<Double> {
        withUTF8(text) { f64($0, format: format) }
    }

    /// See ``f64(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func f64(_ utf8: [UInt8], format: NumFormat) -> Verdict<Double> {
        utf8.withUnsafeBytes { f64($0, format: format) }
    }

    /// See ``f64(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func f64(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Double> {
        numericDoor(DoorCode.f64, utf8, format) { $0.load(as: Double.self) }
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
    public static func decimal(_ text: String, format: NumFormat) -> Verdict<Decimal> {
        withUTF8(text) { decimal($0, format: format) }
    }

    /// See ``decimal(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func decimal(_ utf8: [UInt8], format: NumFormat) -> Verdict<Decimal> {
        utf8.withUnsafeBytes { decimal($0, format: format) }
    }

    /// See ``decimal(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func decimal(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Decimal> {
        numericDoor(DoorCode.decimal, utf8, format, read: Interop.decimal)
    }

    // MARK: - generic numeric (for a caller that is itself generic over the target)

    /// Casts numeric text to whichever of the eleven numeric targets `T` is — the door a
    /// caller that is itself generic over its target reaches for. Resolved statically:
    /// `Int8`/`Int16`/`Int32`/`Int64`, `UInt8`/`UInt16`/`UInt32`/`UInt64`, `Float`,
    /// `Double` and `Decimal`, each through its own concrete door under that door's rules.
    /// ``NumericCastTarget`` is closed to exactly those eleven, so any other `T` is refused
    /// by the compiler, not at run time.
    public static func numeric<T: NumericCastTarget>(_ text: String, format: NumFormat) -> Verdict<T> {
        withUTF8(text) { numeric($0, format: format) }
    }

    /// See ``numeric(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func numeric<T: NumericCastTarget>(_ utf8: [UInt8], format: NumFormat) -> Verdict<T> {
        utf8.withUnsafeBytes { numeric($0, format: format) }
    }

    /// See ``numeric(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func numeric<T: NumericCastTarget>(_ utf8: UnsafeRawBufferPointer, format: NumFormat)
        -> Verdict<T>
    {
        T.castNumeric(utf8, format: format)
    }

    // MARK: - generic scalar (every door a type alone can choose)

    /// Casts text to whichever ``ScalarCastTarget`` `T` is — the door a caller that is itself
    /// generic over its target reaches for, across every door `T` alone picks out: `Bool`,
    /// the eleven ``NumericCastTarget`` types, `UUID`, `Date` (the RFC 3339
    /// ``timestamp(_:)-swift.type.method`` door, zone mandatory), `Duration` and
    /// `Unicode.Scalar` (the ``char(_:)-swift.type.method`` door), each through its own
    /// concrete door under that door's rules. `format` is read by the numeric doors only.
    ///
    /// `DateComponents` is not a target: the date, time and date-time doors all return it,
    /// so the type cannot say which was meant — call that door by name. Resolved
    /// statically, so any other `T` is refused by the compiler, not at run time.
    public static func scalar<T: ScalarCastTarget>(_ text: String, format: NumFormat) -> Verdict<T> {
        withUTF8(text) { scalar($0, format: format) }
    }

    /// See ``scalar(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func scalar<T: ScalarCastTarget>(_ utf8: [UInt8], format: NumFormat) -> Verdict<T> {
        utf8.withUnsafeBytes { scalar($0, format: format) }
    }

    /// See ``scalar(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func scalar<T: ScalarCastTarget>(_ utf8: UnsafeRawBufferPointer, format: NumFormat)
        -> Verdict<T>
    {
        T.castScalar(utf8, format: format)
    }

    // MARK: - uuid

    /// Casts UUID text — all five .NET `Guid` formats (D/N/B/P/X) plus
    /// `urn:uuid:`/`GUID:`/`UUID:` prefixes — to a Foundation `UUID`.
    public static func uuid(_ text: String) -> Verdict<UUID> {
        withUTF8(text) { uuid($0) }
    }

    /// See ``uuid(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func uuid(_ utf8: [UInt8]) -> Verdict<UUID> {
        utf8.withUnsafeBytes { uuid($0) }
    }

    /// See ``uuid(_:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func uuid(_ utf8: UnsafeRawBufferPointer) -> Verdict<UUID> {
        plainDoor(DoorCode.uuid, utf8, read: Interop.uuid)
    }

    // MARK: - temporals

    /// Casts an RFC 3339 instant — zone **mandatory** — to a `Date`, normalized to UTC.
    /// `Date` is a `Double` of seconds, so sub-microsecond fidelity degrades toward the
    /// 0001/9999 window edges; the digit-perfect `DateComponents` that
    /// ``time(_:)-swift.type.method`` returns has no counterpart for instants in Foundation,
    /// and that trade is stated rather than hidden.
    public static func timestamp(_ text: String) -> Verdict<Date> {
        withUTF8(text) { timestamp($0) }
    }

    /// See ``timestamp(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func timestamp(_ utf8: [UInt8]) -> Verdict<Date> {
        utf8.withUnsafeBytes { timestamp($0) }
    }

    /// See ``timestamp(_:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func timestamp(_ utf8: UnsafeRawBufferPointer) -> Verdict<Date> {
        plainDoor(DoorCode.timestamp, utf8, read: Interop.instant)
    }

    /// Casts an integer Unix-epoch value under a caller-declared unit to a `Date`.
    public static func unix(_ text: String, precision: UnixPrecision) -> Verdict<Date> {
        withUTF8(text) { unix($0, precision: precision) }
    }

    /// See ``unix(_:precision:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func unix(_ utf8: [UInt8], precision: UnixPrecision) -> Verdict<Date> {
        utf8.withUnsafeBytes { unix($0, precision: precision) }
    }

    /// See ``unix(_:precision:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func unix(_ utf8: UnsafeRawBufferPointer, precision: UnixPrecision) -> Verdict<Date> {
        unixDoor(DoorCode.unix, utf8, precision.rawValue, read: Interop.instant)
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
    public static func excelSerial(_ text: String, epoch: ExcelEpoch) -> Verdict<Date> {
        withUTF8(text) { excelSerial($0, epoch: epoch) }
    }

    /// See ``excelSerial(_:epoch:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func excelSerial(_ utf8: [UInt8], epoch: ExcelEpoch) -> Verdict<Date> {
        utf8.withUnsafeBytes { excelSerial($0, epoch: epoch) }
    }

    /// See ``excelSerial(_:epoch:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func excelSerial(_ utf8: UnsafeRawBufferPointer, epoch: ExcelEpoch) -> Verdict<Date> {
        unixDoor(DoorCode.excelSerial, utf8, epoch.rawValue, read: Interop.instant)
    }

    /// Casts a strict ISO 8601 `yyyy-MM-dd` calendar date to `DateComponents` (year, month,
    /// day) — digit-perfect, no calendar or zone attached, exactly what the core parsed.
    public static func date(_ text: String) -> Verdict<DateComponents> {
        withUTF8(text) { date($0) }
    }

    /// See ``date(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func date(_ utf8: [UInt8]) -> Verdict<DateComponents> {
        utf8.withUnsafeBytes { date($0) }
    }

    /// See ``date(_:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func date(_ utf8: UnsafeRawBufferPointer) -> Verdict<DateComponents> {
        plainDoor(DoorCode.date, utf8, read: Interop.date)
    }

    /// Casts a separated calendar date — three digit fields joined by one consistent
    /// separator (`/`, `-`, or `.`) — under the caller-declared ``DateOrder`` to
    /// `DateComponents`: `1/7/2026` is January 7th or July 1st only because `order` said
    /// which. The year field is four digits wherever the order puts it (two-digit years
    /// mean century guessing, which never happens — malformed); the order-less
    /// ``date(_:)-swift.type.method`` overload stays strict ISO.
    public static func date(_ text: String, order: DateOrder) -> Verdict<DateComponents> {
        withUTF8(text) { date($0, order: order) }
    }

    /// See ``date(_:order:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func date(_ utf8: [UInt8], order: DateOrder) -> Verdict<DateComponents> {
        utf8.withUnsafeBytes { date($0, order: order) }
    }

    /// See ``date(_:order:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func date(_ utf8: UnsafeRawBufferPointer, order: DateOrder) -> Verdict<DateComponents> {
        unixDoor(DoorCode.dateOrdered, utf8, order.rawValue, read: Interop.date)
    }

    /// Casts a zone-less civil date-time — the shape untrusted feeds actually send
    /// (`1/7/2026 3:04 PM`, `2026-01-07 15:04:05`) — under the caller-declared
    /// ``DateOrder`` to `DateComponents` (year through nanosecond, no zone). The date part
    /// follows ``date(_:order:)-swift.type.method``'s grammar; the optional time part (one
    /// space or `T` after the date) is 24-hour `h:mm[:ss[.f+]]` or 12-hour with an
    /// `AM`/`PM` marker after the clock or ahead of it (`3:04 PM`, `PM 3:04`), ASCII
    /// case-insensitive; absent, the time is midnight. A culture's own designators
    /// (`오후`, `nachm.`) are read only when declared — ``dateTime(_:format:)-swift.type.method``.
    /// No zone is read and none is invented — the text named no instant, so no `timeZone`
    /// component is set; fusing one is the caller's job
    /// (``timestamp(_:)-swift.type.method`` stays the strict RFC 3339 instant door).
    public static func dateTime(_ text: String, order: DateOrder) -> Verdict<DateComponents> {
        withUTF8(text) { dateTime($0, order: order) }
    }

    /// See ``dateTime(_:order:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func dateTime(_ utf8: [UInt8], order: DateOrder) -> Verdict<DateComponents> {
        utf8.withUnsafeBytes { dateTime($0, order: order) }
    }

    /// See ``dateTime(_:order:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func dateTime(_ utf8: UnsafeRawBufferPointer, order: DateOrder) -> Verdict<DateComponents> {
        dateTimeDoor(utf8, order, nil, nil, read: Interop.civil)
    }

    /// Casts a zone-less civil date-time under a declared ``DateFormat``: the field order
    /// of ``dateTime(_:order:)-swift.type.method``, plus the day-period designators the
    /// text's culture writes beside a 12-hour clock — `1/7/2026 3:04 nachm.`,
    /// `2026-01-07 오후 3:04` — read after the clock or ahead of it, with any run of white space or none between,
    /// beside the English `AM`/`PM`, which are read whatever is declared. A format that
    /// declares no designator is exactly the order-only door.
    public static func dateTime(_ text: String, format: DateFormat) -> Verdict<DateComponents> {
        withUTF8(text) { dateTime($0, format: format) }
    }

    /// See ``dateTime(_:format:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func dateTime(_ utf8: [UInt8], format: DateFormat) -> Verdict<DateComponents> {
        utf8.withUnsafeBytes { dateTime($0, format: format) }
    }

    /// See ``dateTime(_:format:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func dateTime(_ utf8: UnsafeRawBufferPointer, format: DateFormat) -> Verdict<DateComponents> {
        dateTimeDoor(utf8, format.order, format, nil, read: Interop.civil)
    }

    /// Casts an ISO 24-hour time-of-day to `DateComponents` (hour, minute, second,
    /// nanosecond) — full nanosecond fidelity.
    public static func time(_ text: String) -> Verdict<DateComponents> {
        withUTF8(text) { time($0) }
    }

    /// See ``time(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func time(_ utf8: [UInt8]) -> Verdict<DateComponents> {
        utf8.withUnsafeBytes { time($0) }
    }

    /// See ``time(_:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func time(_ utf8: UnsafeRawBufferPointer) -> Verdict<DateComponents> {
        plainDoor(DoorCode.time, utf8, read: Interop.time)
    }

    /// Casts a duration (ISO 8601 fixed components, invariant colon form, or protobuf JSON
    /// seconds) to Swift's `Duration` — attosecond-backed, so the core's nanoseconds carry
    /// exactly, both signs.
    public static func duration(_ text: String) -> Verdict<Duration> {
        withUTF8(text) { duration($0) }
    }

    /// See ``duration(_:)-swift.type.method``; input as raw UTF-8 bytes.
    public static func duration(_ utf8: [UInt8]) -> Verdict<Duration> {
        utf8.withUnsafeBytes { duration($0) }
    }

    /// See ``duration(_:)-swift.type.method``; input as a raw view of UTF-8 bytes —
    /// the primitive the `String` and `[UInt8]` forms wrap, for a caller already holding a
    /// buffer (or a slice of one) to cast out of without copying.
    public static func duration(_ utf8: UnsafeRawBufferPointer) -> Verdict<Duration> {
        plainDoor(DoorCode.duration, utf8, read: Interop.duration)
    }

    // MARK: - typed doors (a number the caller already holds, not text)

    /// Reads a number the caller already holds — the `Double` a workbook stores for a
    /// numeric cell — as the exact `Decimal` it names: the shortest decimal that rounds back
    /// to the double, the digits a spreadsheet writes for it. So `0.1` is one tenth, not the
    /// binary fraction nearest it, and `0.1 + 0.2` is `0.30000000000000004`. The result is
    /// canonical, as ``decimal(_:format:)-swift.type.method``'s is. NaN is `.malformed`; an
    /// infinity, a magnitude past 2⁹⁶ − 1 or more than 28 places is `.outOfRange` — the digits
    /// are never cut to fit. A typed door's ``Fault`` has no span: its offset and length are 0.
    public static func decimalFromDouble(_ value: Double) -> Verdict<Decimal> {
        typedDoor(DoorCode.decimal, 0, value, read: Interop.decimal)
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
    public static func excelSerialFromDouble(_ value: Double, epoch: ExcelEpoch) -> Verdict<DateComponents> {
        typedDoor(DoorCode.excelSerial, epoch.rawValue, value, read: Interop.civil)
    }

    /// Reads the fraction of an Excel serial the caller already holds as a `Double` as a time
    /// of day, in the components ``time(_:)-swift.type.method`` returns: `0.75` and
    /// `45292.75` are both 18:00. Snapped as the serial door snaps, and a fraction that snaps
    /// to a whole day is midnight. A negative, NaN or infinite serial is `.malformed`;
    /// one past 9999-12-31 is `.outOfRange`.
    public static func excelTime(_ value: Double) -> Verdict<DateComponents> {
        typedDoor(DoorCode.time, 0, value, read: Interop.time)
    }

    /// Reads a number of days the caller already holds as a `Double` — what an elapsed-time
    /// format (`[h]:mm:ss`) stores — as a `Duration`: `1.5` is a day and twelve hours, and a
    /// negative span is negative, its size snapped as a serial's time. NaN or an infinity is
    /// `.malformed`; beyond ±10,000 years is `.outOfRange`.
    public static func excelDuration(_ value: Double) -> Verdict<Duration> {
        typedDoor(DoorCode.duration, 0, value, read: Interop.duration)
    }

    /// Presents a verdict optionally: an ``CastFailure/empty`` fault becomes `nil` (Swift's
    /// absent), everything else flows through untouched.
    public static func optional<T>(_ verdict: Verdict<T>) -> Verdict<T>? {
        if case .fault(let fault) = verdict, fault.reason == .empty {
            return nil
        }
        return verdict
    }

    /// Presents a checked verdict optionally: an ``CastFailure/empty`` fault becomes `nil`,
    /// everything else, a ``Violation`` included, flows through untouched.
    public static func optional<T>(_ verdict: CheckedVerdict<T>) -> CheckedVerdict<T>? {
        if case .rejection(.fault(let fault)) = verdict, fault.reason == .empty {
            return nil
        }
        return verdict
    }

    // MARK: - the native library itself

    /// The version of the native `libhypercast` linked into this executable, as
    /// `major.minor.patch` — the library's own answer (`hypercast_version`), not this
    /// package's tag — so a caller can prove the two agree before the first cast and name
    /// the mismatch when they don't.
    public static func nativeVersion() -> String {
        Interop.version(oneDotZero.version())
    }
}

/// The closed set of targets ``Cast/numeric(_:format:)-swift.type.method`` resolves over:
/// exactly the eleven types the concrete numeric doors produce. The conformances are this
/// binding's to declare — one per door, below — and a caller's own type has no door to
/// route to, so conforming anything else is unsupported.
public protocol NumericCastTarget {
    /// Routes to this type's own door. ``Cast/numeric(_:format:)-swift.type.method`` is the
    /// entry point; this is the hook it dispatches through.
    static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Self>
}

extension Int8: NumericCastTarget {
    /// ``Cast/i8(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Int8> {
        Cast.i8(utf8, format: format)
    }
}

extension Int16: NumericCastTarget {
    /// ``Cast/i16(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Int16> {
        Cast.i16(utf8, format: format)
    }
}

extension Int32: NumericCastTarget {
    /// ``Cast/i32(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Int32> {
        Cast.i32(utf8, format: format)
    }
}

extension Int64: NumericCastTarget {
    /// ``Cast/i64(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Int64> {
        Cast.i64(utf8, format: format)
    }
}

extension UInt8: NumericCastTarget {
    /// ``Cast/u8(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<UInt8> {
        Cast.u8(utf8, format: format)
    }
}

extension UInt16: NumericCastTarget {
    /// ``Cast/u16(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<UInt16> {
        Cast.u16(utf8, format: format)
    }
}

extension UInt32: NumericCastTarget {
    /// ``Cast/u32(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<UInt32> {
        Cast.u32(utf8, format: format)
    }
}

extension UInt64: NumericCastTarget {
    /// ``Cast/u64(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<UInt64> {
        Cast.u64(utf8, format: format)
    }
}

extension Float: NumericCastTarget {
    /// ``Cast/f32(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Float> {
        Cast.f32(utf8, format: format)
    }
}

extension Double: NumericCastTarget {
    /// ``Cast/f64(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Double> {
        Cast.f64(utf8, format: format)
    }
}

extension Decimal: NumericCastTarget {
    /// ``Cast/decimal(_:format:)-swift.type.method``, reached generically.
    public static func castNumeric(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Decimal> {
        Cast.decimal(utf8, format: format)
    }
}

/// The closed set of targets ``Cast/scalar(_:format:)-swift.type.method`` resolves over:
/// every type exactly one door produces — `Bool`, the eleven ``NumericCastTarget`` types,
/// `UUID`, `Date`, `Duration` and `Unicode.Scalar`. `DateComponents` is left out because
/// three doors return it. The conformances are this binding's to declare, below, and a
/// caller's own type has no door to route to, so conforming anything else is unsupported.
public protocol ScalarCastTarget {
    /// Routes to this type's own door. ``Cast/scalar(_:format:)-swift.type.method`` is the
    /// entry point; this is the hook it dispatches through.
    static func castScalar(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Self>
}

extension ScalarCastTarget where Self: NumericCastTarget {
    /// The numeric target's own door, as ``Cast/numeric(_:format:)-swift.type.method`` reaches it.
    public static func castScalar(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Self> {
        castNumeric(utf8, format: format)
    }
}

extension Int8: ScalarCastTarget {}
extension Int16: ScalarCastTarget {}
extension Int32: ScalarCastTarget {}
extension Int64: ScalarCastTarget {}
extension UInt8: ScalarCastTarget {}
extension UInt16: ScalarCastTarget {}
extension UInt32: ScalarCastTarget {}
extension UInt64: ScalarCastTarget {}
extension Float: ScalarCastTarget {}
extension Double: ScalarCastTarget {}
extension Decimal: ScalarCastTarget {}

extension Bool: ScalarCastTarget {
    /// ``Cast/bool(_:)-swift.type.method``, reached generically; `format` is not read.
    public static func castScalar(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Bool> {
        Cast.bool(utf8)
    }
}

extension UUID: ScalarCastTarget {
    /// ``Cast/uuid(_:)-swift.type.method``, reached generically; `format` is not read.
    public static func castScalar(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<UUID> {
        Cast.uuid(utf8)
    }
}

extension Date: ScalarCastTarget {
    /// ``Cast/timestamp(_:)-swift.type.method`` — RFC 3339, zone mandatory — reached
    /// generically; `format` is not read.
    public static func castScalar(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Date> {
        Cast.timestamp(utf8)
    }
}

extension Duration: ScalarCastTarget {
    /// ``Cast/duration(_:)-swift.type.method``, reached generically; `format` is not read.
    public static func castScalar(_ utf8: UnsafeRawBufferPointer, format: NumFormat) -> Verdict<Duration> {
        Cast.duration(utf8)
    }
}

extension Unicode.Scalar: ScalarCastTarget {
    /// ``Cast/char(_:)-swift.type.method``, reached generically; `format` is not read.
    public static func castScalar(_ utf8: UnsafeRawBufferPointer, format: NumFormat)
        -> Verdict<Unicode.Scalar>
    {
        Cast.char(utf8)
    }
}
