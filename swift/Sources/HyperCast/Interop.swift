import Foundation

/// The native core's C ABI, for a package that carries HyperCast's verdicts across a C ABI of
/// its own (HyperTabular does): the value layouts the core writes, read through exactly the
/// conversions every ``Cast`` door applies, the 32-byte numeric format it reads, the verdict
/// code and span of a fault, and the packed version word. A value read out of another
/// library's buffer through these is the value the door of the same name would have returned.
///
/// Each reader takes a view of one value's bytes, starting at the value, aligned for its
/// widest field (8 bytes for the 16-byte values).
public enum Interop {
    /// The native `RawNumFormat`, 32 bytes: decimal separator, group separator, flags and
    /// currency length as `u32`s at 0/4/8/12, then the symbol's 16 UTF-8 bytes at 16 —
    /// carried as two words whose in-memory bytes are the symbol's, in order.
    public typealias RawNumFormat = (UInt32, UInt32, UInt32, UInt32, UInt64, UInt64)

    /// The inline capacity of the native format's currency field, in UTF-8 bytes.
    public static let currencyMaxBytes = 16

    /// A format laid out as the core reads it — what every numeric door hands the core. The
    /// format was validated when it was declared.
    public static func rawFormat(_ format: NumFormat) -> RawNumFormat {
        (
            format.decimalSeparator.value, format.groupSeparator.value, format.styles.rawValue,
            format.currencyLength, format.currencyLow, format.currencyHigh
        )
    }

    /// 2⁶⁴ — the weight of the core's high word, exact in `Decimal`.
    private static let highWordWeight = Decimal(UInt64.max) + Decimal(1)

    /// A decimal: `lo: u64` at 0, `hi: u32` at 8, `scale: u8` at 12, `negative: u8` at 13;
    /// value = ±(hi·2⁶⁴ + lo) × 10⁻ˢᶜᵃˡᵉ. The magnitude is at most 2⁹⁶ − 1 (29 digits), so
    /// the arithmetic stays inside `Decimal`'s 38-digit mantissa and is exact: the decimal
    /// doors' value.
    public static func decimal(_ raw: UnsafeRawBufferPointer) -> Decimal {
        let lo = raw.load(fromByteOffset: 0, as: UInt64.self)
        let hi = raw.load(fromByteOffset: 8, as: UInt32.self)
        let scale = raw.load(fromByteOffset: 12, as: UInt8.self)
        let negative = raw.load(fromByteOffset: 13, as: UInt8.self) != 0
        let magnitude = hi == 0 ? Decimal(lo) : Decimal(hi) * highWordWeight + Decimal(lo)
        return Decimal(sign: negative ? .minus : .plus, exponent: -Int(scale), significand: magnitude)
    }

    /// A UUID: 16 bytes in RFC 9562 order, which is `uuid_t`'s tuple layout exactly: the uuid
    /// door's value.
    public static func uuid(_ raw: UnsafeRawBufferPointer) -> UUID {
        UUID(uuid: raw.load(as: uuid_t.self))
    }

    /// An instant: `seconds: i64` at 0, `nanos: i32` at 8. A `Date` is a `Double` of seconds,
    /// so sub-microsecond fidelity degrades toward the window's edges: the timestamp, Unix
    /// and Excel-serial doors' value.
    public static func instant(_ raw: UnsafeRawBufferPointer) -> Date {
        let seconds = raw.load(fromByteOffset: 0, as: Int64.self)
        let nanos = raw.load(fromByteOffset: 8, as: Int32.self)
        return Date(timeIntervalSince1970: Double(seconds) + Double(nanos) / 1_000_000_000)
    }

    /// A calendar date: `year: u16` at 0, `month`/`day: u8` at 2/3, as year, month and day
    /// components with no calendar or zone attached: the date doors' value.
    public static func date(_ raw: UnsafeRawBufferPointer) -> DateComponents {
        DateComponents(
            year: Int(raw.load(fromByteOffset: 0, as: UInt16.self)),
            month: Int(raw.load(fromByteOffset: 2, as: UInt8.self)),
            day: Int(raw.load(fromByteOffset: 3, as: UInt8.self)))
    }

    /// A zone-less wall clock: a date at 0 and the nanoseconds of its day as `u64` at 8, as
    /// year-through-nanosecond components with no zone: the date-time doors' value.
    public static func civil(_ raw: UnsafeRawBufferPointer) -> DateComponents {
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

    /// A time of day: nanoseconds since midnight as `u64`, as hour-through-nanosecond
    /// components: the time doors' value.
    public static func time(_ raw: UnsafeRawBufferPointer) -> DateComponents {
        let nanosOfDay = raw.load(as: UInt64.self)
        let (secondOfDay, nano) = nanosOfDay.quotientAndRemainder(dividingBy: 1_000_000_000)
        let (hour, rest) = secondOfDay.quotientAndRemainder(dividingBy: 3_600)
        let (minute, second) = rest.quotientAndRemainder(dividingBy: 60)
        return DateComponents(
            hour: Int(hour), minute: Int(minute), second: Int(second), nanosecond: Int(nano))
    }

    /// A span — the protobuf pair: `seconds: i64` at 0, same-signed `nanos: i32` at 8,
    /// carried exactly: the duration doors' value.
    public static func duration(_ raw: UnsafeRawBufferPointer) -> Duration {
        let seconds = raw.load(fromByteOffset: 0, as: Int64.self)
        let nanos = raw.load(fromByteOffset: 8, as: Int32.self)
        return Duration.seconds(seconds) + .nanoseconds(Int64(nanos))
    }

    /// The fault a nonzero verdict code and its byte span name.
    ///
    /// - Precondition: `code` is a reason (1 empty, 2 malformed, 3 out of range) — anything
    ///   else is a binding bug, not data.
    public static func fault(code: Int32, offset: UInt32, length: UInt32) -> Fault {
        guard let reason = CastFailure(rawValue: code) else {
            preconditionFailure("\(code) is not a verdict reason code — a binding bug, please report it")
        }
        return Fault(reason: reason, offset: Int(offset), length: Int(length))
    }

    /// A native library's packed version word — `major << 16 | minor << 8 | patch`, as a
    /// `*_version()` export returns it — as `major.minor.patch`.
    public static func version(_ packed: UInt32) -> String {
        "\(packed >> 16).\(packed >> 8 & 0xFF).\(packed & 0xFF)"
    }
}
